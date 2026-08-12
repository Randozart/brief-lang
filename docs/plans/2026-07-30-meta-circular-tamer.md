# Meta-Circular Tamer: Briev Compiling Through Itself to Native

## Stdlib Extraction Principle

**Every general-purpose utility produced by this work must land in `lib/std/`, not
in `lib/tamer/`.** The tamer is a stress test, not a library. If a function could
be useful to a program that has nothing to do with VM bytecode or LLVM IR — a
binary parser, a string splitter, a hex encoder — it belongs in stdlib.

Concretely:

| If you write this | And it's general | It goes here |
|---|---|---|
| `read_u32_le` from a Ptr | Yes — any binary format needs this | `lib/std/binary.bv` |
| `exec_op` dispatch for the VM | No — tamer-specific VM logic | `lib/tamer/vm.bv` |
| `string_split` on a delimiter | Yes — no relation to tamer | `lib/std/string.bv` |
| `vm_loop` convergence txn | No — tamer's inner interpreter | `lib/tamer/vm.bv` |
| `int_to_string` for LLVM IR | Yes — any formatting code uses this | `lib/std/string.bv` |
| `codegen_walk_definition` | No — tamer's LLVM emitter | `lib/tamer/codegen.bv` |

**Every phase must identify stdlib candidates before writing tamer-specific
code.** The review question is: "Could someone use this without importing
`lib/tamer/`?" If yes, extract it.

This is not optional — it is the mechanism by which stdlib reaches 100% native
and the tamer pipeline proves Briev's general-purpose fitness.

## Overview

The meta-circular tamer is a stress test: a Briev program (the tamer VM interpreter)
compiled to `.lair` bytecode by the Rust `brievc`, loaded into the C tamer VM, that
then interprets a user's Briev program and emits LLVM IR to produce a native binary.

It answers three questions:

1. **Can Briev function as a systems language?** The tamer does pointer arithmetic,
   struct field access, bytecode parsing, bitwise operations, and FFI calls — all in
   Briev, running inside a VM, inside a C host.

2. **Can the compilation pipeline survive a round-trip through itself?** The user's
   program goes typed AST → beastpack → Briev deserializer → LLVM IR emission → clang.
   Every step but clang is in Briev. If this works, the path to full self-hosting is
   clear.

3. **Are our abstractions general enough?** If the tamer works, then any program that
   can be expressed in Briev can be compiled to a native binary through the same
   pipeline. No special cases, no intrinsic knowledge of specific types.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ brievc bounty my_program.bv                                     │
│                                                                 │
│  my_program.bv                                                  │
│       │                                                        │
│       ▼ compile_to_typed                                        │
│  typed AST + TypeUniverse                                       │
│       │                                                        │
│       ├────► serialize → .beastpack (gzipped BEAST text)       │
│       │                                                        │
│       └────► VmBackend → .user.lair (user's program bytecode)  │
│                                                                 │
│  lib/tamer/main.bv (pre-compiled tamer)                         │
│       │                                                        │
│       └────► .lair (tamer VM interpreter bytecode)              │
│                                                                 │
│  Assembler: .lair + .user.lair + .beastpack + manifest         │
│       │                                                        │
│       ▼ .bounty file                                            │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼ tamer my_program.bounty
┌─────────────────────────────────────────────────────────────────┐
│  C VM (tamer/main.c + interp.c):                                │
│                                                                 │
│  1. Parse .bounty → sections                                    │
│  2. Register host functions (ids 0-5)                           │
│     host_log(0), host_cpuid(1), host_os_abi(2),                │
│     host_llvm_emit(3), host_llvm_flush(4), host_invoke_clang(5) │
│  3. Load .lair (tamer VM interpreter bytecode)                  │
│  4. Find function `tame` by name                                │
│  5. Push args: (user_lair_ptr, user_lair_len,                   │
│                 beastpack_ptr, beastpack_len, output_dir)        │
│  6. vm_execute(tame_fn_idx)                                     │
│       │                                                         │
│       ▼ Briev tamer (running inside C VM)                       │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ tame() validates headers, computes buffer sizes          │    │
│  │ vm_loop() interprets user's .user.lair bytecode          │    │
│  │   │                                                      │    │
│  │   ▼ For each expression in user's program:               │    │
│  │   - HCALL host_llvm_emit → C writes to IR buffer        │    │
│  │   - Control flow (jmp/jz/jnz)                           │    │
│  │   - Arithmetic, memory ops                              │    │
│  │                                                         │    │
│  │   When done: HCALL host_llvm_flush → .ll file           │    │
│  │   HCALL host_invoke_clang → native binary               │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  7. Return exit code                                            │
└─────────────────────────────────────────────────────────────────┘
```

## Why This Is Hard

The VM backend (`src/backend/vm/emit_expr.rs`) compiles Briev source to `.lair`
bytecode. The C interpreter (`tamer/interp.c`) executes `.lair` bytecode. The
Briev tamer (`lib/tamer/vm.bv`) interprets `.lair` bytecode **inside** the C
interpreter — a VM inside a VM.

The user's program, when executed inside the Briev tamer's inner VM, must emit
LLVM IR text via HCALL to the C host. This means the user's program's bytecode
contains instructions that the Briev tamer understands, and when it encounters
an HCALL instruction with a specific host function ID, it relays that call to
the C host.

**The critical insight**: The user's program doesn't know about LLVM IR. The
Briev tamer's codegen pass reads the beastpack (serialized AST) and walks the
AST tree, emitting LLVM IR for each node. The user's `.lair` bytecode is just
the tamer's data — the tamer interprets it as a compiled representation of the
user's logic, and uses that to drive codegen decisions.

But for an MVP, we don't need the full beastpack → LLVM IR pipeline in Briev.
We can take a simpler approach: **the user's `.lair` bytecode, when executed by
the Briev tamer's inner VM, performs the codegen via HCALL**. Each opcode in
the inner VM maps to an LLVM IR emission strategy:

- `PUSH_I64 42` → `host_llvm_emit("store i64 42, i64* %sp\n")`
- `ADD` → `host_llvm_emit("add i64 %a, %b\n")`
- etc.

This is the **interpreter-as-codegen** approach: the inner VM doesn't just
compute values — it emits LLVM IR that represents those computations. The inner
VM's "stack" becomes the LLVM value tracker, and each opcode generates the
corresponding LLVM instruction.

For the MVP, we start with the simplest possible subset and expand.

## Phase 0: Fix the Broken Plumbing (5 days)

### 0a. Fix Ptr arithmetic codegen in LLVM backend (1 day)

`*(ptr + offset)` produces invalid LLVM IR: `add nsw i64 %ac0, %offset` then
`load i64, ptr %i64_result` — the second operand to `load` must be `ptr`, not `i64`.

**This is not a GEP bug.** The LLVM backend uses an i64-centric internal
representation for all values, including pointers. Ptr parameters are converted
from `ptr` to `i64` at function entry via `ptrtoint` (emit_toplevel.rs:1188,
rationale comment at line 1184). This is intentional — it keeps all SSA values
as `i64` and lets LLVM's optimizer eliminate the round-trip. The GEP path at
emit_expr.rs:1946 is dead code by design (config dispatch intercepts first).

**Root cause**: `Expr::Deref` (and the store path in `Statement::Assignment`)
use the i64 register directly as `ptr` in `load`/`store` without an intervening
`inttoptr`. The `Expr::Index` handler (emit_expr.rs:491-520) and the loop
engine's Index store (counter.rs:828-841) show the correct pattern:
`inttoptr` → `GEP`/`load`/`store`.

**Three sites need the fix** (all the same: emit `inttoptr` before using the
register as ptr):

| Site | File:Line | Pattern |
|------|-----------|---------|
| Deref load | `src/backend/llvm/emit_expr.rs:718` | `load ..., ptr %i64_reg` → insert `inttoptr` then `load ..., ptr %new_ptr` |
| Deref store (main) | `src/backend/llvm/emit_stmt.rs:122` | `store ..., ptr %i64_reg` → insert `inttoptr` |
| Deref store (loop) | `src/backend/llvm/loop_engine/counter.rs:846` | `store ..., ptr %i64_reg` → insert `inttoptr` |

The canonical helper `emit_inttoptr` at `mod.rs:1178` generates the correct
pointer-width instruction using `int_bits` from the target data layout.

This blocks native compilation of the tamer via `brievc build lib/tamer/main.bv`.

**Test update**: `test_struct_param_ptrtoint_at_entry` in `tests.rs:1992`
asserts the ptrtoint behavior (which is correct and stays). No test exists for
the Deref load path — add one that verifies `*(ptr + 0)` produces valid IR with
the `inttoptr` + `load` pattern.

### 0b. Pre-compile the tamer to .lair (1 day)

`src/main.rs:run_bounty()` must embed the tamer's `.lair`, not the user's
program. Add a step to pre-compile `lib/tamer/main.bv` to `.lair`:

```rust
// Compile the tamer itself to .lair bytecode
eprintln!("[bounty] Compiling tamer VM interpreter to .lair...");
let tamer_source = std::fs::read_to_string("lib/tamer/main.bv")?;
let (tamer_items, tamer_universe) = compile::compile_to_typed(
    "lib/tamer/main.bv", &tamer_source, &tamer_opts)?;
let mut vm = VmBackend::new();
let tamer_lair = vm.generate(&tamer_items, &tamer_universe);
```

### 0c. Add SECTION_USER_LAIR to bounty format (1 day)

`src/bounty/mod.rs` — add section type constant:

```rust
pub const SECTION_LAIR: u8 = 1;        // tamer VM interpreter bytecode
pub const SECTION_BEASTPACK: u8 = 2;   // user's serialized typed AST
pub const SECTION_MANIFEST: u8 = 3;    // JSON manifest
pub const SECTION_USER_LAIR: u8 = 4;   // user's program .lair bytecode
```

Update `run_bounty()` to write 4 sections instead of 3. Update `write_bounty()`
to accept an optional user `.lair`. (Or add a `write_bounty_full` function.)

**Why 4 sections**: The tamer needs both:
- Its own `.lair` (to execute)
- The user's `.lair` (as data to interpret)

If we only put one `.lair` in the bounty, the C tamer can't distinguish "this is
the code to run" from "this is the data to process."

### 0d. Fix C tamer entry point: call tame() by name, not hardcoded index (1 day)

`tamer/main.c` calls `vm_execute(&vm, 0)` — assumes function 0 is the entry
point. The tamer `.lair` may have functions in any order (imports, support
functions, then `tame`). We need `vm_find_function(vm, "tame")` that walks the
function table and matches by name.

Add to `tamer/interp.h`:

```c
// Find a function by name. Returns index or -1 if not found.
int vm_find_function(VmState* vm, const char* name);
```

Add to `tamer/interp.c`:

```c
int vm_find_function(VmState* vm, const char* name) {
    for (size_t i = 0; i < vm->function_count; i++) {
        const char* fn_name = vm->string_table + vm->function_table[i].name_idx;
        if (strcmp(fn_name, name) == 0) {
            return (int)i;
        }
    }
    return -1;
}
```

Update `tamer/main.c` to:

```c
// Load .lair (the tamer VM interpreter)
vm_load_lair(&vm, tamer_lair, tamer_lair_size);
// Find tame function
int tame_idx = vm_find_function(&vm, "tame");
if (tame_idx < 0) { error("tame function not found"); }
// Push arguments: user_lair, beastpack, output_dir
vm_push_args(&vm, user_lair_ptr, user_lair_len,
             beastpack_ptr, beastpack_len, output_dir_ptr);
// Call tame
uint64_t result = vm_execute(&vm, tame_idx);
```

### 0e. Wire up from run_bounty to the 4-section bounty (1 day)

Update `src/main.rs` to:
1. Compile user's .bv to typed AST
2. Obfuscate and serialize as beastpack
3. Compile user's typed AST to .lair via `VmBackend::generate()`
4. Pre-compile `lib/tamer/main.bv` to .lair
5. Call `write_bounty_full(tamer_lair, user_lair, beastpack, manifest)`

## Phase 1: Briev-Side Tamer Completeness (7 days)

The Briev tamer (`lib/tamer/`) must handle all opcodes that the VM backend
actually emits. Current gaps:

### 1a. Add missing opcodes to vm.bv (2 days)

The VM backend emits opcodes that the Briev tamer's `vm.bv` doesn't handle:

From `src/backend/vm/assembler.rs`:
```
OP_OVER   (0x04) — used by some expression patterns
OP_ROT    (0x05) — used by ternary expressions
OP_DIV_S  (0x09), OP_REM_S (0x0A) — integer division/remainder
OP_NE     (0x12) — not-equal (emit_expr line 86)
OP_LT_S   (0x13) through OP_GE_S (0x16) — comparisons
OP_TRACE  (0x1A) — debug tracing
OP_ALLOC  (0x91) — heap allocation
OP_LOAD_OFF (0x92) — struct field load with offset
OP_STORE_OFF (0x93) — struct field store with offset
OP_PUSH_STR (0xB0) — string constant
```

Cross-reference `emit_expr.rs` match arms against `vm.bv` opcode handlers.
Every opcode that the Rust VM backend can emit must have a corresponding arm
in the Briev VM interpreter's `exec_op` match.

### 1b. Wire HCALL dispatch through to C host functions (2 days)

The Briev tamer's `exec_op` handles `OP_HCALL`:

```briev
0x71 => { // hcall
    let host_id = read_u32(bc, pc + 1);
    // HCall dispatching — placeholder for now
    term pc + 5;
};
```

This is a no-op — it skips the HCALL without calling the host. The Briev tamer
executes inside the C VM, so an HCALL from the Briev tamer must trigger an HCALL
in the outer C VM. But the outer C VM's HCALL mechanism expects the args to be on
the outer VM's stack, not the inner VM's stack.

**Solution**: When the Briev tamer's inner VM encounters an HCALL, it must:
1. Copy the inner VM's stack frame (args) to a buffer
2. Signal to the outer C VM to dispatch the host function
3. Copy the result back

This is the **meta-circular relay**. The outer C VM's `host_log` function hooks
into the inner VM's state to read arguments. The mechanism:

The Briev tamer's `exec_op` for HCALL:
```briev
0x71 => { // hcall
    let host_id = read_u32(bc, pc + 1);
    // Copy args from stack to a relay buffer
    // (stored in a fixed struct field that the outer C host can read)
    // Signal the outer C VM via a sentinel return code
    // The outer C VM intercepts the sentinel, reads the relay buffer,
    // dispatches the host function, writes the result back.
    // The inner VM resumes with the result on its stack.
    term -2; // sentinel: "HCALL pending, invoke host"
};
```

The C VM's `vm_execute` loop detects the sentinel return, reads the relay
buffer from the inner VM's state (passed as a Ptr<Frame> or similar), and
dispatches to the registered host function.

**Alternative (simpler for MVP)**: Instead of a relay buffer, the inner VM
copies args to a fixed offset in its own locals/stack that the outer C host
knows about. The outer host reads them directly from the inner VM's data array.
This avoids needing the inner VM to have special "relay" struct support —
just pointer arithmetic on its stack array.

### 1c. DAG buffer sizing (1 day)

`lib/tamer/analyze.bv` already has `compute_buffer_sizes()` that computes
minimum stack/locals/frame sizes. The current `main.bv` uses fixed-size struct
arrays (`VMStack { data: Int[1024] }`). Verify these bounds hold for real
programs by running the DAG analysis against actual compiled `.lair` files.

If bounds need adjustment, make them dynamic by taking the analysis results
and sizing struct arrays at the Briev level. (Briev doesn't support runtime-sized
fixed arrays, so this may require a `MAX_*` constant that's verified against
the DAG analysis.)

### 1d. Loader completeness (1 day)

`lib/tamer/loader.bv` has `read_u8/i8/u16/i16/u32/i64`. Verify these match
the `.lair` header format in `tamer/interp.h`:
- Function table entry: 20 bytes (name_idx u32 + bc_off u64 + bc_len u32 + local_count u16 + arg_count u16)
- Header offset layout (offsets at bytes 16, 24, 32, 40, 48, 56, 64, 72, 80, 88)

The current `main.bv` reads fields directly by word offset (`word4 = *(data + 4)`).
This is incorrect — the header has byte offsets, not word (8-byte) offsets. The
`.lair` header uses byte offsets for section pointers, and the tamer must read
them as 8-byte values at specific byte positions.

### 1f. Handle `TopLevel::AsmFn` in VmBackend (1 day)

`src/backend/vm/emit_toplevel.rs` does not handle `TopLevel::AsmFn` — it falls
through to `_ => {}` and the asm function is silently dropped from the `.lair`.

Asm functions can't execute in the VM (they're native assembly for specific ISAs),
but they must still be **reachable by name** for the inner VM's call dispatch.
Register them as host functions:

```rust
TopLevel::AsmFn(af) => {
    let id = self.host_fn_ids.len() as u32;
    self.asm.register_host_fn(&af.name, id);
    self.host_fn_ids.insert(af.name.clone(), id);
}
```

This lets the VM bytecode path call the asm function through HCALL. The C host
receives the call and executes it natively. The inner VM never needs to interpret
assembly — it delegates to the outer runtime.

### 1e. Add function table walking to loader (1 day)

`lib/tamer/main.bv` reads `fn_table` entries by direct arithmetic. Extract into
named loader helpers for clarity and correctness:

```briev
defn fn_bc_offset(ft: Ptr<Int>, idx: Int) -> Int { ... }
defn fn_bc_len(ft: Ptr<Int>, idx: Int) -> Int { ... }
defn fn_local_count(ft: Ptr<Int>, idx: Int) -> Int { ... }
defn fn_arg_count(ft: Ptr<Int>, idx: Int) -> Int { ... }
defn fn_name_idx(ft: Ptr<Int>, idx: Int) -> Int { ... }
```

Currently duplicated between `analyze.bv` and the VM loop in `vm.bv`. Extract
into `loader.bv`.

**Stdlib extraction**: `read_u8`/`read_i8`/`read_u16`/`read_i16`/`read_u32`/`read_i32`/
`read_u64`/`read_i64` are general binary format parsers — they belong in
`lib/std/binary.bv`, not `lib/tamer/loader.bv`. Move them to stdlib and import
from there. The `fn_bc_offset`/`fn_bc_len`/etc. helpers are `.lair`-format-specific
and stay in `lib/tamer/`.

## Phase 2: Beastpack Deserialization in Briev (10 days)

### 2a. Beastpack header parsing (2 days)

The beastpack binary format from `src/beastpack/serialize.rs`:

```
Offset  Size  Field
0       10    Magic: "BEASTPACK\0"
10      4     Version (u32 LE)
14      8     Obfuscation seed (u64 LE)
22      4     Flags (u32 LE)
26      4     Reserved (zero)
30      8     Data size (u64 LE)
38      N     Gzipped BEAST text
38+N    32    Blake3 checksum
```

Add to `lib/tamer/beastpack.bv`:

```briev
struct BeastpackHeader {
    magic: Int[10];
    version: Int;
    seed: Int;
    flags: Int;
    data_size: Int;
};

defn parse_beastpack_header(data: Ptr<Int>, len: Int) -> Result<BeastpackHeader, Int> { ... }
defn verify_checksum(data: Ptr<Int>, len: Int) -> Bool { ... }
```

**Stdlib extraction**: `read_u32_le_from_ptr`, `read_u64_le_from_ptr`, and
`read_bytes_from_ptr` are general binary utilities. They belong in
`lib/std/binary.bv`. If `verify_checksum` uses Blake3, that belongs in
`lib/std/hash/blake3.bv` (or wherever the hash library lives). Only the
`BeastpackHeader` struct and `parse_beastpack_header` wrapper are
tamer-specific and stay in `lib/tamer/`.

### 2b. Gzip decompression (3 days)

The beastpack data is gzip-compressed. The Briev tamer needs to decompress it
before parsing the BEAST text.

**Option A**: Use the inner VM's `SysCall#` to invoke an external decompressor
(zcat, gunzip). Simple but introduces a process-spawn dependency.

**Option B**: Implement gzip decompression in Briev. This is ~200 lines of Briev
for inflate + CRC32. Feasible but adds complexity to the tamer.

**Option C**: Don't compress the beastpack in MVP mode. Set `FLAG_COMPRESSED = 0`
and emit plaintext BEAST. This is the simplest path.

For MVP, use Option C. Set `run_bounty()` to serialize without compression:

```rust
// In beastpack::serialize, skip compression for MVP
let flags = 0;  // FLAG_COMPRESSED not set
let text = to_beast(&clean, universe);
// Write text directly without gzip
```

### 2c. BEAST text parser in Briev (5 days)

The BEAST format is an S-expression tree. From `src/beast/sexpr.rs`, the
grammar is:

```
sexpr   ::= "(" atom* ")" | atom
atom    ::= integer | string | keyword
integer ::= ["-"] digit+
string  ::= "\"" [^"]* "\""
keyword ::= ":" identifier
```

The BEAST deserializer (`src/beast/deserialize.rs`) converts S-expressions into
`Vec<TopLevel>` + `TypeUniverse`. The serialized format looks like:

```
(:program
  (:definition "main"
    (:params)
    (:return :int)
    (:body
      (:term (:decimal 42)))))
```

Implement a BEAST parser in `lib/tamer/beast_parse.bv` that produces an AST
data structure:

```briev
enum BeastExpr {
    Decimal(Int),
    String(String),
    Keyword(String),
    List(List<BeastExpr>),
};

defn parse_beast_text(text: Ptr<Int>, len: Int) -> Result<BeastExpr, Int> { ... }
```

The parser must handle:
- Nested S-expressions
- String escaping
- Negative integers
- Comments (if any — check BEAST format documentation)

This is the heaviest single piece of the plan. The BEAST format has many
expression types — for MVP, only parse the subset that the user's program uses:
`defn`, `term`, `decimal`, `identifier`, `binary_op`, `unary_op`, `call`.

**Stdlib extraction**: The S-expression parser is inherently general — it knows
nothing about Briev's AST shape. It consumes text and produces `BeastExpr` trees.
This goes in `lib/std/sexpr.bv`. Only the `BeastExpr`-to-Briev-AST mapping is
tamer-specific and stays in `lib/tamer/`.

### 2d. Preserve AsmFn and verification chains in beastpack (3 days)

**Problem**: The beastpack currently drops both `TopLevel::AsmFn` nodes and
`Definition.derivation.chain` (the `:=` verification chain). The BEAST
serializer has a debug-format fallthrough for `AsmFn` (unparseable), and
`from_beast()` hardcodes `derivation: None`. Full round-trip loss.

**Why this matters**: The `.bounty` is a platform-independent distribution
format. A `popcount` function with `asm<x86_64>` and `asm<aarch64>` variants
must carry ALL variants in the beastpack. The tamer selects the right one at
install time based on the target architecture — it cannot re-run the Rust
compiler's verification chain.

**Serialized form** (changes to `src/beast/serialize.rs`):

```
(asmfn "x86_64" "popcount_asm"
  (params (param "x" Int))
  (outputs Int)
  (body "popcnt {result}, {x}"))

(defn "popcount"
  (params (param "x" Int))
  (outputs Int)
  (chain (ref "popcount_asm") (ref "popcount_ref"))
  (body (term (binary-op :bit-and (call "popcount_ref" (ident "x")) (decimal 127)))))
```

**Changes required**:

| File | Change |
|------|--------|
| `src/beast/serialize.rs` | Add `emit_asm_fn()` — serialize `AsmFn` as structured S-expression |
| `src/beast/serialize.rs` | Add derivation/chain serialization to `emit_definition()` |
| `src/beast/deserialize.rs` | Add `parse_asm_fn()` — reconstruct `TopLevel::AsmFn` |
| `src/beast/deserialize.rs` | Add derivation/chain parsing to `parse_definition()`, remove `derivation: None` hack |
| `src/beastpack/obfuscate.rs` | `collect_toplevel_names()` must descend into `AsmFn` params and `DerivationBlock.chain` |
| `src/beastpack/strip.rs` | Add explicit `TopLevel::AsmFn` passthrough arm (for clarity, no functional change) |

**Implementation notes**:

The `chain` field contains `Vec<ChainSegment>`, where each segment is either
`Ref(String)` or `Derivation(Box<DerivationBlock>)`. Serialize as:
```
(chain (ref "asm_x86") (ref "asm_arm") (ref "ref_impl"))
```

The `AsmFn` target field is a string like `"x86_64"`, `"aarch64"`. The tamer
matches this against `host_arch()` output from the C host.

**Stdlib extraction**: No new stdlib — this is purely Rust-side BEAST
serialization changes.

## Phase 3: LLVM IR Emission in Briev (14 days)

### 3a. Define a minimal LLVM IR subset (2 days)

For MVP, support only:
- Module header (`target triple = "x86_64-unknown-linux-gnu"`)
- Function declarations (`define i64 @name(i64 %arg0)`)
- Basic block labels (`entry:`)
- Load/store to stack slots (`alloca i64`, `store i64 %x, i64* %ptr`, `load i64, i64* %ptr`)
- Integer arithmetic (`add i64 %a, %b`, `sub`, `mul`)
- Return (`ret i64 %val`)
- Integer literals as `i64` constants

This subset is sufficient for programs like:
```briev
defn add(a: Int, b: Int) -> Int { term a + b; }
defn main() -> Int { term add(40, 2); }
```

### 3b. LLVM IR string builder in Briev (3 days)

Create `lib/tamer/llvm_emit.bv` that builds LLVM IR strings:

```briev
struct LlvmModule {
    declarations: String;
    functions: String;
};

defn llvm_module_new() -> LlvmModule { ... }
defn llvm_emit_function_header(mod: Ptr<LlvmModule>, name: String, args: List<String>) { ... }
defn llvm_emit_ret(mod: Ptr<LlvmModule>, val: String) { ... }
defn llvm_emit_add(mod: Ptr<LlvmModule>, dest: String, lhs: String, rhs: String) { ... }
defn llvm_emit_alloca(mod: Ptr<LlvmModule>, dest: String) { ... }
defn llvm_emit_store(mod: Ptr<LlvmModule>, val: String, ptr: String) { ... }
defn llvm_emit_load(mod: Ptr<LlvmModule>, dest: String, ptr: String) { ... }
defn llvm_flush_to_host(mod: Ptr<LlvmModule>, output_path: Ptr<Int>) -> Int { ... }
```

Each `emit_*` function appends to the module's internal string buffers using
the `host_llvm_emit(text_ptr)` HCALL.

**Stdlib extraction**: The `LlvmModule` struct and its `append_*` methods are
just a string builder with a specific pattern. If they use `StringBuilder`
internally, no new stdlib is needed. If they do something novel (e.g., automatic
register numbering, label deduplication), extract those helpers into
`lib/std/string_builder.bv` before building `LlvmModule` on top.

### 3c. Beastwalk: walk the deserialized AST and emit LLVM IR (7 days)

Create `lib/tamer/codegen.bv`:

```briev
defn compile_beast_to_llvm(ast: BeastExpr) -> Result<String, Int> {
    // Top-level: (:program (...))
    // Walk each (:definition ...) and emit LLVM function
    ...
};
```

The codegen walker maps each BEAST expression to LLVM IR:

| BEAST form | LLVM IR |
|-----------|---------|
| `(:definition "f" (:params (:param "x" :int)) (:body ...))` | `define i64 @f(i64 %x) { entry: ... }` |
| `(:term (:decimal 42))` | `ret i64 42` |
| `(:term (:identifier "x"))` | `ret i64 %x` |
| `(:binary-op :add (:decimal 1) (:decimal 2))` | `%t0 = add i64 1, 2` |
| `(:call "g" (:decimal 5))` | `%t0 = call i64 @g(i64 5)` |

For MVP, skip structured types, contracts, transactions/nodes, guards, and
generics. Only `defn` with straight-line arithmetic code.

### 3d. Wire LLVM IR emission into tame() (2 days)

Update `lib/tamer/main.bv` so `tame()`:

1. Parses beastpack header (Phase 2a)
2. Decompresses/dispatches BEAST text (Phase 2b)
3. Parses BEAST text to AST (Phase 2c)
4. Walks AST to emit LLVM IR via HCALL (Phase 3c)
5. Calls `host_llvm_flush` to write the `.ll` file
6. Calls `host_invoke_clang` to compile to native binary
7. Returns exit code

The `tame()` signature stays the same:
```briev
export defn tame(lair_data: Ptr<Int>, lair_len: Int,
                 beastpack_data: Ptr<Int>, beastpack_len: Int) -> Int
```

For MVP, ignore `lair_data`/`lair_len` — the VM bytecode interpretation path
(interpreting user's `.lair` in the inner VM) is deferred to a follow-up phase.
The first working path uses the beastpack directly:
beastpack → BEAST text → AST → LLVM IR → clang.

### 3e. Inner VM codegen mode (deferred to Phase 5)

The full meta-circular path (inner VM interprets user's `.lair` and generates
LLVM IR from bytecode) is deferred. The beastpack path is the MVP.

### 3f. Target-aware asm variant selection in the tamer (3 days)

The tamer's codegen walker must select the right implementation from a
verification chain based on the target architecture. The C host provides
two HCALL functions for querying target info:

```
host_cpuid(id 1)   → returns bitmask of CPU features
host_os_abi(id 2)  → returns OS identifier (0 = Linux)
```

**Selection algorithm** in `lib/tamer/codegen.bv`:

```briev
defn select_variant(chain: List<ChainSegment>) -> CodegenPath {
    // Walk chain in priority order (same as := declaration)
    for segment in chain {
        match segment {
            ChainSegment::AsmFn(af) => {
                // Query target arch
                let arch = host_arch();  // via host_os_abi HCALL
                if arch_matches(af.target, arch) {
                    term CodegenPath::Asm(af);
                }
            },
            ChainSegment::Ref(name) => {
                // Resolve reference and recurse
                let resolved = resolve_ref(name);
                match resolved {
                    ChainSegment::AsmFn(af) => { /* check target */ },
                    ChainSegment::Definition(d) => {
                        term CodegenPath::Briev(d);
                    },
                }
            },
        };
    };
    // No match — error (should not happen with a Briev fallback)
    term CodegenPath::Error;
};

defn host_arch() -> String {
    // Call host_os_abi, map to arch string
    let abi: Int = hcall_host_os_abi();
    when abi == 0 { term "x86_64"; };
    // ... more as backends are added
    term "unknown";
};

defn arch_matches(target: String, current: String) -> Bool {
    // Exact match for now. Future: family matching
    // (x86_64 matches amd64, x86, etc.)
    term target == current;
};
```

**Integration with `compile_beast_to_llvm()`**:

When the codegen walker encounters a `defn` with a `(chain ...)` field, it
calls `select_variant(chain)` and emits the selected variant. The chain is
resolved at compile time (in the tamer, not in the Rust compiler), so the
same `.bounty` file produces different native code on different targets.

```briev
defn compile_beast_to_llvm(ast: BeastExpr) -> Result<String, Int> {
    for item in ast {
        match item {
            BeastExpr::List(list) => {
                let tag = list[0];
                when tag == Keyword("definition") {
                    let chain = parse_chain(item);
                    if len(chain) > 0 {
                        // Multi-variant: select by target
                        let variant = select_variant(chain);
                        match variant {
                            CodegenPath::Asm(af)  => emit_asm_fn(af),
                            CodegenPath::Briev(d) => emit_briev_fn(d),
                        };
                    } else {
                        // Single definition: emit directly
                        emit_briev_definition(item);
                    };
                };
            },
        };
    };
};
```

### 3g. LLVM call asm emission for AsmFn (2 days)

When the tamer selects an `asm<>` variant, it must emit LLVM inline assembly:

```briev
defn emit_asm_fn(mod: Ptr<LlvmModule>, af: AsmFn) {
    // Emit: define i64 @name(i64 %arg0) {
    //   entry:
    //   %r0 = call i64 asm "popcnt $0, $1", "=r,r"(i64 %arg0)
    //   ret i64 %r0
    // }

    let fn_sig = format_string("define {} @{}(",
        llvm_type(af.ret_type), af.name);
    for (i, (_, ty)) in enumerate(af.params) {
        if i > 0 { append_string(fn_sig, ", "); };
        append_string(fn_sig, format_string("{} %arg{}", llvm_type(ty), i));
    };
    append_string(fn_sig, ") local_unnamed_addr #8 {");
    emit_to_host(fn_sig);

    emit_to_host("  entry:");

    // Build constraint string
    let constraint = "=r";
    for _ in af.params {
        constraint = constraint + ",r";
    };
    constraint = constraint + ",~{dirflag},~{fpsr},~{flags}";

    // Build operand list
    let mut args: String;
    for (i, (_, ty)) in enumerate(af.params) {
        if i > 0 { args = args + ", "; };
        args = args + format_string("{} %arg{}", llvm_type(ty), i);
    };

    // Substitute placeholders {result} → $0, {param} → $1, $2, ...
    let asm_text = substitute_placeholders(af.body[0], af.params);

    // Emit call asm instruction
    if af.ret_type != Void {
        emit_to_host(format_string(
            "  %r0 = call {} asm \"{}\", \"{}\"({})",
            llvm_type(af.ret_type), asm_text, constraint, args));
        emit_to_host(format_string("  ret {} %r0", llvm_type(af.ret_type)));
    } else {
        emit_to_host(format_string(
            "  call void asm sideeffect \"{}\", \"{}\"({})",
            asm_text, constraint, args));
        emit_to_host("  ret void");
    };

    emit_to_host("}");
};
```

**Placeholder substitution**:

The `asm<>` body uses `{result}` and `{param_name}` as placeholders. The
LLVM emitter substitutes these with `$0` (output) and `$N` (input):

```briev
defn substitute_placeholders(instr: String, params: List<(String, Type)>) -> String {
    let result = string_replace(instr, "{result}", "$0");
    let mut offset = 1;  // $1, $2, ... for input params
    for (name, _) in params {
        result = string_replace(result, format_string("{{{}}}", name),
                                 format_string("${}", offset));
        offset = offset + 1;
    };
    term result;
};
```

**Multiple instructions**: If the asm body has multiple strings, emit them as
separate `call asm` instructions linked by SSA registers — or use `\n` joining
for a single `call asm sideeffect` with multiple lines. For MVP, join with `\n`
and emit as a single instruction.

## Phase 4: Everything In Between — Stdlib & Compiler Fixes (7 days)

These are prerequisites discovered during development. Per the Stdlib Extraction
Principle, every general utility in this phase goes to `lib/std/`, not to a
tamer-specific module. This phase closes the gaps that would otherwise force
tamer code to import from non-standard paths.

### 4a. Fix os/fs.bv corruption (1 day)

`lib/std/os/fs.bv` has duplicated lines — `const` is missing the `c` on every
other line. Rewrite the file.

### 4b. Add missing string operations to stdlib (3 days)

The BEAST parser and LLVM emitter need:
- `string_split` — tokenizing BEAST text
- `string_trim` — removing whitespace
- `string_starts_with`, `string_ends_with` — keyword matching
- `string_join` — building LLVM IR strings
- `int_to_string` — converting register numbers, constants to text
- `string_replace` — string substitution

Currently these are either FFI-only or missing. Port to native Briev or add
as `#` intrinsics.

### 4c. Add HexParse, DecParse to stdlib for beastpack header parsing (1 day)

The beastpack header contains binary integers. Add:
- `parse_u32_le(data: Ptr<Int>, offset: Int) -> Int`
- `parse_u64_le(data: Ptr<Int>, offset: Int) -> Int`

These read little-endian multi-byte values from a Ptr<Int> data buffer.

### 4d. Blob/byte array utilities for binary data parsing (2 days)

The beastpack header and BEAST text both require reading from flat byte buffers
(Ptr<Int> interpreted as bytes). Add:
- `read_byte(ptr: Ptr<Int>, offset: Int) -> Int`
- `read_bytes(ptr: Ptr<Int>, offset: Int, count: Int) -> Ptr<Int>`
- `ptr_as_string(ptr: Ptr<Int>, len: Int) -> String`

(Some may already exist in `lib/tamer/loader.bv` — consolidate into stdlib.)

## Phase 5: Testing (5 days)

Each stdlib module extracted during Phases 1-4 must have its own tests in the
appropriate location — not as tamer tests. The tamer-specific integration tests
cover the pipeline. The stdlib tests cover the utilities.

### 5a. Unit tests for the Briev tamer (2 days)

Each `lib/tamer/` module must have tests:

- `test_beastpack_header` — parse a known beastpack header
- `test_beast_parse_simple` — parse `(:term (:decimal 42))`
- `test_beast_parse_nested` — parse `(:binary-op :add ...)`
- `test_llvm_emit_add` — emit `%t0 = add i64 1, 2` to string
- `test_codegen_simple_defn` — compile `defn f() -> Int { term 42; }` to LLVM IR
- `test_codegen_binary_op` — compile `defn add(a, b) { term a + b; }`

### 5b. End-to-end bounty → tamer → native binary (2 days)

Behavioral test using the C test harness (`tamer/tests/`):

```c
// Build a .bounty with a known program
// Run the C tamer's VM with the bounty
// Verify a native binary is produced
// Execute the binary and check output
```

Add a new test `tamer/tests/test_bounty_pipeline.c`:

```c
int main(void) {
    // 1. Create a minimal .bounty in memory:
    //    - tamer.lair (pre-compiled from lib/tamer/main.bv)
    //    - user.lair (pre-compiled from a test program like add(5,7))
    //    - beastpack (serialized AST of the test program)
    //    - manifest
    // 2. Run tamer pipeline on the .bounty
    // 3. Verify output binary exists and produces correct result
}
```

### 5c. Regression: verify existing tamer tests still pass (1 day)

The existing `test_briev_tamer.c` loads `combined.lair` and calls `tame()` at
hardcoded index 71. After adding `vm_find_function`, update the test to use
name-based lookup instead.

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Briev string operations too slow for BEAST parsing | Medium | High | Optimize hot paths; pre-allocate string builders; batch HCALLs |
| HCALL relay between inner/outer VM too slow | Medium | Medium | Batch emissions; reduce relay frequency |
| BEAST format changes break tamer | Low | High | Pin version; add format validation |
| Gzip decompression in Briev too complex | Medium | Medium | Skip compression for MVP; use SysCall# for v1 |
| Inner VM (tamer) stack overflow from real programs | Low | High | DAG buffer sizing handles this; increase struct array sizes |
| LLVM IR emission produces incorrect IR | Medium | High | Test each emission pattern against clang; run clang -S for validation |
| `vm_find_function` loads wrong function | Low | Medium | Test with known .lair; verify by function count + name |
| Byte order or alignment mismatch between Rust VM backend and Briev tamer | Medium | High | Auto-generate opcode constants from a shared source; add assertion tests |
| The Briev language itself has a bug that blocks compilation | Medium | Critical | Test tamer compiles with current `brievc` first; fix bugs as found |
| String building in Briev causes memory explosion | Medium | Medium | Use streaming emission via HCALL rather than building giant strings |
| Utilities duplicated in tamer-private code instead of stdlib | Medium | Low | Phase-by-phase audit; enforce Stdlib Extraction Principle in review |
| `os/fs.bv` corruption causes subtle bugs in file I/O | High | Medium | Rewrite entirely in Phase 4; add tests |
| AsmFn variants missing from beastpack breaks cross-target compilation | High | High | Phase 2d adds round-trip serialization; test with popcount_chain.bv |
| Target arch detection via HCALL too simplistic | Medium | Medium | Start with exact string match; evolve to family matching (x86_64 ↔ amd64) |
| LLVM `call asm` constraint syntax errors | Medium | High | Test each emission against `clang -S -c`; validate constraint strings match LLVM docs |

## Timeline Summary

| Phase | Effort | Stdlib extracted | Deliverable |
|-------|--------|-----------------|-------------|
| Phase 0: Plumbing | 5 days | None | `brievc bounty` produces correct `.bounty`; C tamer calls `tame()` by name |
| Phase 1: Tamer completeness | 8 days | `binary.bv` (read_u* from Ptr) | All opcodes handled; HCALL relay works; AsmFn registered in VM bytecode |
| Phase 2: Beastpack in Briev | 13 days | `binary.bv` (extend), `sexpr.bv`, `hash/blake3.bv` | Beastpack header parsed; BEAST text parsed; AsmFn+chain round-trip in BEAST |
| Phase 3: LLVM IR emission | 19 days | `string_builder.bv` (extend) | AST → LLVM IR; asm variant selection; `call asm` emission; clang produces binary |
| Phase 4: Stdlib gaps | 7 days | `string.bv` (extend), `os/fs.bv` (fix) | String ops, binary parsing, `os/fs.bv` fixed |
| Phase 5: Testing | 5 days | All extracted modules get tests | Unit tests, end-to-end bounty test, regression test |

**Total: ~57 days (12 weeks)** — with ~6 new/extended stdlib modules created en route

### MVP Milestone (after Phase 0 + 1 + simplified Phase 2 + 3a-c + 5)

**~25 days** to get `brievc bounty add.bv && tamer add.bounty` producing a
working binary that computes `3 + 4` and exits with code 7.

This uses:
- Beastpack without compression
- Only `defn` with `term`, `let`, binary ops, and function calls
- Direct beastpack → LLVM IR (no inner VM interpretation)
- Fixed-size struct arrays for all buffers
- Stdlib gains: `binary.bv`, `sexpr.bv`, extended `string.bv` and `string_builder.bv`

### Full Milestone (all phases)

**~57 days** to get the meta-circular pipeline working:
- Inner VM interprets user's `.lair` bytecode
- LLVM IR emitted per-opcode during interpretation
- Full program compilation (not just straight-line arithmetic)
- Beastpack preserves `asm<>` variants and `:=` verification chains
- Tamer selects the right asm variant for the target architecture at install time
- `popcount_chain.bv` compiles to native code with `popcnt` on x86, `cnt` on ARM, Briev fallback elsewhere
- `os/fs.bv` fixed, stdlib complete, all tests passing

## Documentation

After each phase, update:

- `docs/architecture/overview.md` — Bounty pipeline section with correct architecture
- `docs/architecture/bounty-format.md` (new) — `.bounty` format spec with all 4 section types
- `docs/architecture/tamer-meta-circular.md` (new) — Meta-circular architecture description
- `tamer/interp.h` — API comments for `vm_find_function`
- `src/bounty/mod.rs` — Doc comment update for 4-section bounty
- `src/beast/serialize.rs` — `emit_asm_fn()` doc comment; `emit_definition()` derivation serialization docs
- `src/beast/deserialize.rs` — `parse_asm_fn()` doc comment; derivation/chain parsing docs
- `src/beastpack/obfuscate.rs` — `AsmFn` and `DerivationBlock` handling doc comments
- `lib/tamer/codegen.bv` — Target-aware selection and `call asm` emission rationale comments
- `src/main.rs` — Comments in `run_bounty` explaining the two-lair architecture
- `lib/tamer/` — All files get 2026-07-30 rationale comments

## Exceptions to Standards

This plan follows AGENTS.md Plan Directives throughout:
- ✓ **Flat control flow** — all pseudocode uses early returns and guard clauses
- ✓ **Comment the code** — every new/modified site gets a YYYY-MM-DD rationale
- ✓ **Behavioral tests, not literal tests** — the end-to-end test asserts "binary exits with code 7", not "IR contains `add i64`"
- ✓ **Documentation is code** — architecture docs updated in same commit as code
- ✓ **No type name matching** — all Briev type queries go through protocol membership, not name strings
- ✓ **DRY consolidation** — beastpack parsing, LLVM emission helpers are extracted into shared functions (not duplicated across phases)
- ✓ **Full provenance** — every temporary solution flagged with `TEMP: 2026-07-30:` and describes the path to permanence
- ✓ **Target-aware dispatch** — asm variant selection queries the host at install time, avoiding hardcoded architecture checks in the tamer
- ✓ **Verification chains are configuration, not compiler knowledge** — the tamer doesn't know about `popcnt` vs `cnt`; it reads variants from beastpack data and selects by target match
