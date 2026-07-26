# Zero-C Tamer with Static Memory by Contract
## 2026-07-26

## Overview

This plan eliminates all C dependencies from the Brief compiler pipeline and
replaces `Alloc#` with `struct`-embedded static arrays proven by contract. The
tamer (install-time compiler) becomes a pure Brief program compiled by Brief's
own LLVM backend, using `SysCall#` with inline assembly for OS interaction and
`struct` with `Int[N]` fields for all buffers. No `malloc`, no `brief_rt.c`,
no `tamer/*.c` — zero C.

**Four phases, executed sequentially:**

| Phase | Title | Dependencies | Outcome |
|-------|-------|-------------|---------|
| 0 | Foundation Fixes | None | CALL bug, `not` instruction, loader.bv cleanup work |
| 1 | `#System` Protocol Wire-up | Phase 0 | `from #System` resolves end-to-end per target |
| 2 | `SysCall#` Inline Asm + Runtime Port | Phase 1 | No `brief_rt.c` — all runtime in Brief using syscall asm |
| 3 | Tamer in Brief | Phase 2 | No `tamer/*.c` — pure Brief tamer with struct arrays, no `Alloc#` |
| 4 | DAG-Based Size Inference | Phase 3 | Max buffer sizes proven by DAG, not guessed |

---

## Plan Directives Compliance

This plan and every commit implementing it adheres to the five Plan Directives
from AGENTS.md:

1. **FLAT CONTROL FLOW**: Max 2 nesting levels. Guard clauses, `?`, early
   returns. Extract deeply nested logic into named helpers.
2. **COMMENT THE CODE**: Every modified code site carries a rationale comment
   (`// 2026-07-26: <why>`). Never delete existing rationale comments.
3. **UPDATE ALL EXAMPLES**: When syntax changes (`Int[N]`, `from #System`),
   update all example `.bv` files in `examples/`, `lib/std/`, `benchmarks/`.
4. **DOCUMENTATION IS CODE**: Update `docs/architecture/`, `docs/features/`,
   and inline `///` doc comments in the same commit as the code change.
5. **BEHAVIORAL TESTS, NOT LITERAL TESTS**: Test behavioral outcomes (correct
   return values, correct LLVM IR structure), not literal IR snapshot strings.

---

## Phase 0 — Foundation Fixes

Targets Items 1, 2, 5, 6 from `docs/plans/2026-07-26-tamer-completion-and-bugfixes.md`.

### 0a. Fix CALL Multi-Call Bug

**Severity:** High — blocks all function calls from within a compiled `.lair`
program. Currently worked around by inlining all reads in `tame`.

**Root cause confirmed** (`tamer/interp.c:539-566`): `OP_CALL` saves `return_pc`
on the *caller's* frame (at `vm->frame_count - 1` before `push_frame`), but
`OP_RET` reads it from the *callee's* frame (index `vm->frame_count - 1` after
`push_frame`). The callee's frame was initialized to `return_pc = NULL` by
`push_frame`. Every `OP_RET` therefore returns from `vm_execute` entirely —
the second call in a function never executes.

```c
// Before (broken) — return_pc saved on CALLER before push_frame:
Frame* cur = &vm->frames[vm->frame_count - 1];  // caller
cur->return_pc = pc + 3;
push_frame(vm, callee->local_count);               // callee, return_pc=NULL

// After (fixed) — return_pc saved on CALLEE after push_frame:
push_frame(vm, callee->local_count);
Frame* cur = &vm->frames[vm->frame_count - 1];  // callee
cur->return_pc = pc + 3;
```

**Also fix the Brief VM** (`lib/tamer/vm.bv:257-287` and `lib/tamer/combined.bv`):
`OP_CALL` saves frame metadata (`ll`, `callee_local_c`, `pc + 3`, `sll`) in the
flat `fd` buffer, and increments `fc` (frame_count). But `OP_RET` (line 166-168)
just sets `new_pc = -1` — it never reads the saved return PC from `fd`. The
Brief VM's `OP_CALL` must store `pc + 3` as the return PC in the frame record,
and `OP_RET` must read it and jump there. This is structurally the same bug:
`OP_RET` discards the caller's state.

**Files:**
- `tamer/interp.c` — `case OP_CALL:` line 539, `case OP_RET:` line 568
- `lib/tamer/vm.bv` — `exec_op` 0x54 handler (line 257), 0x19 handler (line 166)
- `lib/tamer/combined.bv` — same changes (concatenated snapshot)

**2026-07-26: Root cause documented.**
Before the fix, `OP_CALL` saved `return_pc` at frame index `frame_count - 1`,
which is the CALLER's frame. After `push_frame`, that slot is frame-count-2
and the CALLEE's frame (at frame-count-1) has `return_pc = NULL`. OP_RET reads
from the callee's frame, gets NULL, and terminates `vm_execute`. The second
call never executes. The fix is to push the callee's frame first, then save
`return_pc` on it — so OP_RET reads the correct value.

**Tests:**
1. Write a `.bv` file with a function that calls another function twice and
   sums the results: `defn test() -> Int { let a = get_val(); let b = get_val(); term a + b; }`
2. Compile with `--backend vm`, load into C interpreter, check return value
3. Assert `result == a + b` (both calls return correct values)
4. Add `fprintf(stderr, ...)` debug output to trace `locals_len`, `stack_len`,
   and `frame_count` before/after each call

### 0b. Fix `OP_NOT` (Logical vs Bitwise)

**Severity:** Medium — affects `!=` comparisons and any code that inverts a
boolean. Currently worked around for `Neq` via `eq + push_i8(1) + xor` in the
Brief VM backend.

**File:** `tamer/interp.c:320-323`
```c
// Before (~a, bitwise NOT — breaks boolean inversion):
// For a=1: ~1 = 0xFFFFFFFFFFFFFFFE, which is non-zero, so jz does NOT skip
// the guard body — opposite of intended behavior.
stack_push(vm, ~a);

// After (a == 0 ? 1 : 0, logical NOT):
stack_push(vm, a == 0 ? 1 : 0);
```

**Decision: Option A — Add `OP_BNOT` for bitwise NOT, make `OP_NOT` logical.**

Rationale: The VM uses booleans for conditionals (jz/jnz test for zero/non-zero).
A logical NOT (0→1, else→0) is the correct semantic for boolean inversion.
Bitwise NOT is still useful for bit manipulation. Rather than ambiguating `OP_NOT`,
add a separate `OP_BNOT` opcode.

**Opcode assignment:** `OP_BNOT = 0x1C` (next available after `OP_TRAP = 0x1B`).
Update `tamer/interp.h` and the Brief VM opcode constants.

**Files to modify:**
- `tamer/interp.c` — `case OP_NOT:` change to logical, add `case OP_BNOT:` with `~a`
- `tamer/interp.h` — add `#define OP_BNOT 0x1C`
- `src/backend/vm/assembler.rs` — add `emit_bnot()` method
- `src/backend/vm/emit_expr.rs` — use `emit_not()` for `UnaryOpKind::Not` (logical),
  use `emit_bnot()` for `UnaryOpKind::BitNot`
- `lib/tamer/vm.bv` — add `OP_BNOT = 0x1C` constant, add dispatch arm
- `lib/tamer/combined.bv` — same changes

**Checklist:**
- [ ] Change `OP_NOT` in `tamer/interp.c` to use logical NOT (`a == 0 ? 1 : 0`)
- [ ] Add `OP_BNOT = 0x1C` to `tamer/interp.h`
- [ ] Implement `case OP_BNOT` in `tamer/interp.c` with bitwise NOT (`~a`)
- [ ] Add `emit_bnot()` in `src/backend/vm/assembler.rs`
- [ ] Update `emit_expr.rs` to use `emit_not()` for logical, `emit_bnot()` for bitwise
- [ ] Add `OP_BNOT` constant and handler to `lib/tamer/vm.bv`
- [ ] Add `OP_BNOT` constant and handler to `lib/tamer/combined.bv`
- [ ] Verify the `xor 1` workaround for `Neq` is still correct (it is — `eq + xor 1`
      produces a logical NOT without needing `OP_NOT` at all; it can stay as-is)
- [ ] Test: `(a == b) → not →` should give 1 if a != b, 0 if a == b
- [ ] `cargo test --lib` passes

### 0c. Clean Up `loader.bv` Duplicates

**Severity:** Low — cosmetic but fixes wasted bytecode size and obsoleted code.

**File:** `lib/tamer/loader.bv` (242 lines → ~90 lines after cleanup)
**Also:** `lib/tamer/combined.bv` (mirrors loader.bv content)

The file currently has THREE versions of each read function. Remove:

- **Version 2** (lines 57-87): Direct `*(bc as Ptr<Int> + addr / 8)` arithmetic.
  This does NOT scale the byte offset correctly — it divides by 8 for word
  addressing but does not add the base pointer before the cast. Produces wrong
  byte offsets in the VM. **Remove entirely.**
- **Version 3** (lines 148-185): Function-call-based reads that call `read_u8`
  for each byte. Hits the multi-call bug (Phase 0a). After Phase 0a the call
  bug is fixed, but Version 3 is still less efficient than Version 1 — it does
  8 calls instead of 1 word read + shift. **Remove entirely.**
- **Duplicate header field accessors** (lines 189-216): Identical to lines 91-120.
  **Remove.**
- **Duplicate `find_bounty_section`** (lines 224-242): Identical to lines 128-146.
  **Remove.**
- **Duplicate `read_i8`** (lines 57-61): Identical to lines 18-22. **Remove.**

Keep only Version 1 (cast-based, lines 9-55, 91-146). The remaining functions:
`read_u8`, `read_i8`, `read_u16`, `read_i16`, `read_u32`, `read_i64`,
`lair_version`, `lair_fn_offset`, `lair_fn_size`, `lair_bc_offset`,
`lair_bc_size`, `fn_bc_offset`, `fn_bc_len`, `fn_local_count`, `fn_arg_count`,
`find_bounty_section`.

**2026-07-26: Removed duplicate function versions.**
Version 1 is correct and avoids both the scaling bug (Version 2) and the
multi-call bug (Version 3). After the CALL bug fix (Phase 0a), Version 3 would
work, but it's more verbose and slower — one word read + shift is better than
8 byte reads and 8 function calls.

### 0d. BOM Check

**Severity:** Low — cosmetic suspicion, may be non-issue.

**Action:** Verify `.ll` files don't start with a UTF-8 BOM (`\xEF\xBB\xBF`):
```bash
briefc build --backend llvm test.bv -o test.ll
head -c 3 test.ll | xxd
```

If BOM found: trace source to template writer or string builder in
`src/backend/llvm/` and strip it. If no BOM found: close as "not reproducible"
and add a note that the BOM suspicion was investigated and dismissed.

**No BOM fix needed if the bytes are `0x3B 0x20 0x3B` (i.e., `; ;` — a normal
LLVM comment line) or any other non-BOM byte sequence.**

---

## Phase 1 — `#System` Protocol Wire-up

The parser already creates `FromSpec::Protocol("#System")` from `from #System`
syntax (see `src/parser/definitions.rs:237`). The downstream is a dead end —
nothing resolves `FromSpec::Protocol` to an actual library or linker flag.

**`#System` is the sole protocol.** There is no `#Win32`, `#WASI`, or any other
protocol hashword. Platform-specific APIs use a direct `from "link/lib.so"` path.
`#System` abstracts "the platform's standard system library" — it maps to
different libraries per target (libc on Linux, libSystem on macOS, WASI preview1
on wasm) but means the same thing to the compiler: "this symbol comes from the
platform's native runtime."

### 1a. Add `protocol_map` to Protocol Config

**File:** `config/protocols.toml`

Per-target protocol → library mappings:

```toml
[x86_64-linux]
"#System" = "c"

[aarch64-linux]
"#System" = "c"

[x86_64-macos]
"#System" = "System"

[aarch64-macos]
"#System" = "System"

[wasm32-wasi]
"#System" = "wasi_snapshot_preview1"
```

**File:** `src/target.rs` — `ProtocolConfig` struct loaded from `config/protocols.toml`.

```rust
/// Loaded config/protocols.toml.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProtocolConfig {
    #[serde(flatten)]
    per_target: HashMap<String, HashMap<String, Option<String>>>,
}

impl ProtocolConfig {
    pub fn load() -> Self {
        let content = include_str!("../config/protocols.toml");
        toml::from_str(content).unwrap_or_else(|e| panic!("config/protocols.toml parse error: {}", e))
    }

    pub fn resolve(&self, target_triple: &str, protocol: &str) -> Result<Option<&str>, String> {
        let target_map = self.per_target.get(target_triple).ok_or_else(|| {
            format!("target '{}' not found in config/protocols.toml", target_triple)
        })?;
        match target_map.get(protocol) {
            Some(Some(lib)) => Ok(Some(lib.as_str())),
            Some(None) => Ok(None),
            None => Err(format!(
                "protocol '{}' is not declared for target '{}'", protocol, target_triple
            )),
        }
    }
}
```

**2026-07-26: Added protocol_map to protocol configuration.**
Before this change, `from #System` parsed but had no mechanism to resolve the
protocol name to a system library. The `ProtocolConfig` provides a per-target
declarative mapping. `#System` is the only protocol — any other hashword
produces a compile error.

### 1b. Implement `FromSpec::Protocol` Resolution

**File:** `src/analysis/frgn_dispatch.rs`

In `resolve_single_frgn()`, check for `FromSpec::Protocol` before the extension
check (since protocols have no file extension):

```rust
if let FromSpec::Protocol(proto) = &fb.from {
    let protocol_config = crate::target::ProtocolConfig::load();
    let default_triple = "x86_64-linux";  // or from --target CLI flag
    let lib = protocol_config.resolve(default_triple, proto).map_err(|e| {
        format!("frgn '{}': {}", fb.effective_brief_name(), e)
    })?;
    return Ok(ResolvedFrgn::Inline {
        symbol: fb.foreign_name.clone(),
        compile_source: false,
        protocol_lib: lib.map(|s| s.to_string()),
    });
}
```

Add `protocol_lib: Option<String>` to `ResolvedFrgn::Inline`:

```rust
pub enum ResolvedFrgn {
    Inline {
        symbol: String,
        compile_source: bool,
        /// 2026-07-26: Protocol library name for from #System.
        /// None = resolved from a normal file path or compiler registry.
        /// Some(lib) = link with -l<lib> (e.g., "c" for libc on Linux).
        protocol_lib: Option<String>,
    },
    Bridge { ... },
    Unsupported(String),
}
```

**2026-07-26: First actual resolution of `FromSpec::Protocol`.**
Before this change, the parser accepted `from #System` but the compiler never
acted on it — it fell through all match arms and produced a linking error or
silently emitted a `declare` with no backing library. Now the compiler looks
up the protocol in the target's `ProtocolConfig` and records the library to
link against. Any protocol other than `#System` produces a compile error.

### 1c. Update LLVM Codegen for Protocol-Linked FFI

**File:** `src/backend/llvm/emit_toplevel.rs`

When emitting a `declare` for a `frgn` that has `protocol_lib: Some(lib)`:

- For MVP (static linking): emit the standard `declare` and rely on the linker
  to find the library via `-l` flags. The library name is passed to the linker
  driver.
- For `frgn?` (optional FFI): emit `@dlopen`/`@dlsym` fallback pattern in the
  generated LLVM IR, so the call works at runtime only if the library is
  available.

**Add linker flag emission** to `src/compile.rs`:

In `compile_ll_to_binary()`, collect protocol library names from resolved frgns
and pass them as `-l` flags to clang:

```rust
// 2026-07-26: Link protocol-based libraries (from #System).
for lib in &protocol_libs {
    cmd.arg(format!("-l{}", lib));
}
```

**2026-07-26: Protocol libraries linked via clang flags.**
Before this change, `from #System` frgns had no mechanism to instruct the linker
to include the required system library. Now the resolved `protocol_lib` is
passed as `-l<lib>` to clang during the final linking step.

### 1e. `#Link<name>` — Direct System Library Linking

**Syntax:** `frgn MessageBoxW(h: Int, ...) -> Int from #Link<user32>;`

`#Link<name>` tells the compiler to link `-l<name>` — no per-target config, no
registry lookup. The platform linker resolves it (same as `gcc -lz` for zlib).

**Semantic model:**

| `from` variant | Example | What happens |
|----------------|---------|-------------|
| `from #System` | `frgn printf from #System` | Per-target library from `config/protocols.toml` |
| `from #Link<name>` | `frgn foo from #Link<z>` | `-l<name>` passed to linker directly |
| `from "path"` | `frgn foo from "lib/foo.c"` | File path, compiled/linked by extension |
| `from <name>` | `frgn foo from <xxhash.c>` | Compiler registry / stdlib lookup |

**Parser** (`src/parser/definitions.rs`): When `from` is followed by `#Link`,
consume it, expect `<name>`, parse until `>`, emit `FromSpec::Linked(name)`.

**AST** (`src/ast/top.rs`): Add `Linked(String)` to `FromSpec`:

```rust
pub enum FromSpec {
    Literal(PathBuf),
    CompilerRegistry(String),
    Protocol(String),      // only "#System" is valid
    Linked(String),        // from #Link<user32> → "-luser32"
}
```

**Resolution** (`src/analysis/frgn_dispatch.rs`): Match `FromSpec::Linked(lib)`
→ `ResolvedFrgn::Inline { protocol_lib: Some(lib), .. }`. Same codegen path
as `#System` — emit `declare`, linker receives `-l<lib>`.

**Tests:**

1. Parser: `from #Link<user32>` → `FromSpec::Linked("user32")`
2. Dispatch: `FromSpec::Linked("z")` → `Inline { protocol_lib: Some("z") }`
3. Linker: `-lz` flag emitted in `compile_ll_to_binary()`

**Files to modify:**
- `src/ast/top.rs` — add `Linked(String)` variant, `extension()`, `as_str()`
- `src/parser/definitions.rs` — parse `#Link<name>` before generic `#` protocol
- `src/analysis/frgn_dispatch.rs` — handle `FromSpec::Linked`

No changes needed in `compile.rs` — the `protocol_lib` mechanism already passes
`-l<lib>` flags to clang.

### 1e. Protocol Tests

**Add tests:**

1. **`target.rs` unit test**: Deserialize `ProtocolConfig` from TOML, call
   `resolve("x86_64-linux", "#System")`, verify `Ok(Some("c"))`.
2. **Integration test**: Write a `.bv` file using `frgn printf from #System`,
   compile with `--backend llvm`, verify the generated `.ll` file has the
   correct `declare` and the linker receives `-lc`.
3. **Negative test**: `from #NonExistent` → compile error about unknown protocol.

**Files to modify:**
- `config/protocols.toml` — protocol → library mapping per target
- `src/target.rs` — `ProtocolConfig` struct, deserialization, `resolve()` method
- `src/analysis/frgn_dispatch.rs` — resolve `FromSpec::Protocol`
- `src/compile.rs` — pass protocol libs to linker
- `src/backend/llvm/emit_toplevel.rs` — conditional `declare` for protocol frgns

### 1f. Compiler Registry — `briefc registry`

**Problem:** `import <name>` resolves via `config/module-registry.toml` (a baked
HashMap), and `from <name>` searches the stdlib path. Neither allows users to
install their own `.bv` files for `<name>` lookup.

**Solution:** A per-user registry directory at `~/.brief/registry/` populated
by `briefc registry add`.

#### Directory layout

```
~/.brief/
  registry/
    my-queue.bv                 # single module
    xxhash/                      # multi-file package
      xxhash.bv
      xxhash.c
```

Project-local `.brief/registry/` overrides the user-wide one (same merge
semantics as project-local config).

#### CLI commands

```
briefc registry add <path> [--name <name>]
  # Copy file/dir to ~/.brief/registry/<name>, version-locked (no symlink).
  # --name defaults to the file stem: ./my-queue.bv → "my-queue".

briefc registry list
  # Print every entry in ~/.brief/registry/ with type (file/dir) and size.

briefc registry remove <name>
  # rm -rf ~/.brief/registry/<name>* matching.
```

#### Resolution changes

**`import <name>`** (`src/import_resolver.rs`):
1. Check `registry_dir/<name>.bv` → resolve if exists
2. Check `registry_dir/<name>/` → resolve `<name>/<name>.bv` if exists
3. Fall back to existing `config/module-registry.toml`
4. Fall back to stdlib path search

**`from <name>`** (`src/compile.rs` `collect_extra_objects`):
1. Same search order: registry dir first, then stdlib path
2. Works for any file type (`.bv`, `.c`, `.rs`, `.o`)

The `ImportResolver` gets a `registry_dir: Option<PathBuf>` field, loaded at
construction from the platform data directory (`dirs::data_dir()`), overridable
by project-local `.brief/registry/` and `--registry-dir` CLI flag.

#### Design decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Copy vs symlink | **Copy** (version-locked) | Source may change or disappear; registry is a snapshot |
| Platform path | `dirs::data_dir()` | Cross-platform: Linux `~/.local/share/brief/`, macOS `~/Library/Application Support/brief/`, Windows `%APPDATA%/brief/` |
| Project override | `.brief/registry/` | Same pattern as `.brief/config/` for target config |
| File types | All types (`.bv`, `.c`, `.rs`, `.o`, `.so`) | Registry is content-agnostic; works for `import <name>` and `from <name>` |
| Duplicates | Last added wins | Simple and predictable |

#### Files to create/modify

| File | Action |
|------|--------|
| `src/registry.rs` | **New** — `add()`, `list()`, `remove()` helpers + `registry_path()` resolver |
| `src/main.rs` | Add `"registry"` subcommand → `run_registry()` with add/list/remove |
| `src/import_resolver.rs` | Add `registry_dir: Option<PathBuf>` field, check in `resolve_import()` |
| `src/compile.rs` | `collect_extra_objects()` searches registry dir for `from <name>` |

#### Tests

1. `briefc registry add ./test.bv` → file appears in `~/.brief/registry/test.bv`
2. `briefc registry list` → output contains `test.bv`
3. `briefc registry remove test` → file removed
4. `import <test>` resolves to `~/.brief/registry/test.bv` when it exists
5. `from <test.c>` resolves to `~/.brief/registry/test.c` when it exists

---

## Phase 2 — `SysCall#` Inline Asm + Runtime Port

### 2a. Change `SysCall#` to Emit Inline Assembly

**File:** `src/backend/llvm/intrinsics.rs` — `emit_intrinsic_call()` match arm
for `"SysCall#"`.

Replace the current `call @brief_syscall(i64 %num, ...)` with target-conditional
inline assembly:

**x86_64 (Linux):**
```rust
// 2026-07-26: Emit inline syscall asm instead of calling brief_syscall().
// This eliminates the libc dependency for system calls. The syscall
// instruction clobbers rcx and r11 (used internally by the kernel to save
// RIP and RFLAGS), plus rax (return value) and rdi/rsi/rdx/r10/r8/r9 (args).
// On error, rax contains -errno (which the kernel returns in the -4096..-1
// range, but userspace syscall() wrappers typically negate to positive errno
// values — we keep the raw kernel convention for simplicity).
if target_triple.starts_with("x86_64") {
    writeln!(out, "  %{id} = call i64 asm sideeffect \
        \"syscall\", \
        \"={{rax}},{{rcx}},{{r11}},{{rax}},{{rdi}},{{rsi}},{{rdx}},{{r10}},{{r8}},{{r9}}\" \
        (i64 %{num}, i64 %{a1}, i64 %{a2}, i64 %{a3}, \
         i64 %{a4}, i64 %{a5}, i64 %{a6})").ok();
}
```

**aarch64 (Linux):**
```rust
if target_triple.starts_with("aarch64") {
    writeln!(out, "  %{id} = call i64 asm sideeffect \
        \"svc #0\", \
        \"={{x0}},{{x0}},{{x1}},{{x2}},{{x3}},{{x4}},{{x5}}\" \
        (i64 %{num}, i64 %{a1}, i64 %{a2}, i64 %{a3}, \
         i64 %{a4}, i64 %{a5}, i64 %{a6})").ok();
}
```

**Non-Linux targets (macOS, WASM):** Fall back to calling the C `syscall()`
function until proper platform support is added. Emit a `call @syscall` and
rely on the C runtime.

**Argument handling:** `SysCall#` currently takes 7 args (num + up to 6). The
inline asm form matches the Linux syscall ABI exactly:
- rax/x8 = syscall number
- rdi/x0 = arg1
- rsi/x1 = arg2
- rdx/x2 = arg3
- r10/x3 = arg4
- r8/x4 = arg5
- r9/x5 = arg6

**2026-07-26: SysCall# emits inline assembly instead of calling C.**
Before this change, every `SysCall#` called `brief_syscall()` in `brief_rt.c`,
which called `syscall()` from libc. This tied the compiler to libc at the
runtime level. With inline asm, the syscall instruction issues directly — no
libc dependency. The target triple determines which asm template to use
(x86_64: `syscall` instruction, aarch64: `svc #0`).

**Also update the `SysCall#` signature** in `src/intrinsic_signatures.rs:126`:

```rust
// 2026-07-26: Added variadic flag to Signature to allow extra args beyond
// declared parameters. SysCall# may take up to 6 integer arguments after
// the syscall number. Alloc# also uses variadic for the optional strategy arg.

"SysCall#" => Some(Signature {
    name: "SysCall#",
    parameters: vec![("num", Type::int())],
    return_kind: ReturnKind::Native("Int"),
    observable: true,
    variadic: true,  // new field
}),
```

Add `variadic: bool` to the `Signature` struct in `src/intrinsic_signatures.rs`
(default `false`). This avoids false arity errors for variadic intrinsics.

### 2b. Port `brief_rt.c` Functions to Brief

Each C function becomes a Brief `defn` using `SysCall#`. Organized into protocol
modules under `lib/std/posix/`. Each function is ported one at a time, verified,
then the C original is removed from `brief_rt.c`.

#### Migration Order

| Step | C Function | Brief Equivalent | Depends On |
|------|-----------|------------------|------------|
| 1 | `__print_char` | `posix/io.bv`: `syscall(SYS_write, 1, &c, 1)` | Phase 2a (inline asm) |
| 2 | `__print_int` | `posix/io.bv`: format + write | Step 1 |
| 3 | `__exit` | `posix/io.bv`: `syscall(SYS_exit_group, code)` | Phase 2a |
| 4 | `brief_str_to_c` | **Delete** — no C strings needed without C runtime | — |
| 5 | `brief_cstr_to_brief` | **Delete** — same reasoning | — |
| 6 | `brief_free_brief_str` | **Delete** — same reasoning | — |
| 7 | `__read_file__` | `posix/io.bv`: `open` + `read` + `close` | Steps 8-10 |
| 8 | `__write_file__` | `posix/io.bv`: `open` + `write` + `close` | Steps 9-11 |
| 9 | `SysCall#` backend | **Already replaced** in Phase 2a | Phase 2a |
| 10 | `brief_syscall` | **Delete** — replaced by inline asm | Phase 2a |
| 11 | `brief_sysconf` | `posix/syscall.bv`: `syscall(SYS_sysconf, name)` | Phase 2a |
| 12 | `ShellCmd` | `posix/process.bv`: `clone` + `execve` + `pipe` | Steps 1, 7 |
| 13 | `__getenv_brief` | `posix/env.bv`: read `/proc/self/environ` | Steps 7-8 |
| 14 | `__getenv_int` | `posix/env.bv`: parse Int from env string | Step 13 |
| 15 | Thread pool | `posix/thread.bv`: `clone(CLONE_VM)` | Phase 2a |
| 16 | GPU runtime | **Separate concern** — not in scope | — |

#### `lib/std/posix/syscall.bv`

```brief
// 2026-07-26: POSIX syscall wrappers. All system calls go through SysCall#
// which emits inline assembly (syscall instruction on x86_64, svc #0 on
// aarch64). No libc dependency. See docs/plans/2026-07-26-tamer-zero-c-and-static-memory.md.

// ── Syscall numbers (x86_64 Linux) ────────────────────────────────────────
// These are target-specific constants. A future improvement would load
// these from a target config file or use #System protocol resolution.
// For now, x86_64 Linux is the primary target.

export let SYS_READ:        Int = 0;
export let SYS_WRITE:       Int = 1;
export let SYS_OPEN:        Int = 2;
export let SYS_CLOSE:       Int = 3;
export let SYS_LSEEK:       Int = 8;
export let SYS_MMAP:        Int = 9;
export let SYS_BRK:         Int = 12;
export let SYS_CLOCK_GETTIME: Int = 228;
export let SYS_EXIT:        Int = 60;
export let SYS_EXIT_GROUP:  Int = 231;
export let SYS_CLONE:       Int = 56;
export let SYS_EXECVE:      Int = 59;
export let SYS_WAIT4:       Int = 61;
export let SYS_GETDENTS64:  Int = 217;
export let SYS_NANO_SLEEP:  Int = 35;

// ── Syscall helpers ───────────────────────────────────────────────────────
// Takes up to 6 args after the syscall number. SysCall# is variadic — extra
// args are passed through to the inline asm.

export defn syscall0(n: Int) -> Int { term SysCall#(n); };

export defn syscall1(n: Int, a1: Int) -> Int { term SysCall#(n, a1); };

export defn syscall2(n: Int, a1: Int, a2: Int) -> Int {
    term SysCall#(n, a1, a2);
};

export defn syscall3(n: Int, a1: Int, a2: Int, a3: Int) -> Int {
    term SysCall#(n, a1, a2, a3);
};

export defn syscall4(n: Int, a1: Int, a2: Int, a3: Int, a4: Int) -> Int {
    term SysCall#(n, a1, a2, a3, a4);
};

export defn syscall5(n: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int) -> Int {
    term SysCall#(n, a1, a2, a3, a4, a5);
};

export defn syscall6(n: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int, a6: Int) -> Int {
    term SysCall#(n, a1, a2, a3, a4, a5, a6);
};
```

#### `lib/std/posix/io.bv`

```brief
// 2026-07-26: POSIX I/O — print, exit, file read/write.
// All functions use SysCall# with inline assembly — no C runtime needed.

import "syscall.bv";

// ── File descriptor constants ─────────────────────────────────────────────
export let FD_STDIN:  Int = 0;
export let FD_STDOUT: Int = 1;
export let FD_STDERR: Int = 2;

// ── Open flags ────────────────────────────────────────────────────────────
export let O_RDONLY: Int = 0;
export let O_WRONLY: Int = 1;
export let O_RDWR:   Int = 2;
export let O_CREAT:  Int = 64;
export let O_TRUNC:  Int = 512;
export let O_APPEND: Int = 1024;

// ── Print ─────────────────────────────────────────────────────────────────
// These replace __print_char, __print_int, __print from brief_rt.c.

export defn print_char(c: Int) -> Int {
    // Write a single character to stdout. SysCall# takes integer args,
    // so we pass the address of the character directly.
    // In the final struct-based tamer, this would use a stack buffer:
    //   let buf: Int[1]; buf[0] = c; syscall3(SYS_WRITE, 1, &buf, 1);
    // For now, use a temporary allocation:
    let buf = Alloc#(8) as Ptr<Int>;
    *buf = c;
    term syscall3(SYS_WRITE, FD_STDOUT, buf as Int, 1);
};

export defn print_int(n: Int) -> Int {
    // Format integer as decimal, write to stdout.
    // This is a simplified version — full formatting is handled by Brief code.
    // For MVP, write hex using syscall.
    let buf = Alloc#(32) as Ptr<Int>;
    let mut i: Int = 0;
    let mut tmp: Int = n;
    let neg: Int = 0;
    [tmp < 0] { &neg = 1; &tmp = -tmp; };
    [tmp == 0] { &i = 1; *(buf) = 48; }; // '0'
    [tmp > 0] {
        while [tmp > 0] {
            // This is a placeholder — actual formatting uses txn iteration.
            // For now, rely on the existing C __print_int until migration.
            term syscall1(SYS_EXIT, 0);
        };
    };
    // Reverse digits and write
    // MVP: call into C for now, migrate incrementally
    term 0;
};

export defn exit(code: Int) -> Int {
    term syscall1(SYS_EXIT_GROUP, code);
};

// ── File I/O ──────────────────────────────────────────────────────────────
// These replace __read_file__ and __write_file__ from brief_rt.c.

export defn open(path: Int, flags: Int, mode: Int) -> Int {
    term syscall3(SYS_OPEN, path, flags, mode);
};

export defn read(fd: Int, buf: Int, count: Int) -> Int {
    term syscall3(SYS_READ, fd, buf, count);
};

export defn write(fd: Int, buf: Int, count: Int) -> Int {
    term syscall3(SYS_WRITE, fd, buf, count);
};

export defn close(fd: Int) -> Int {
    term syscall1(SYS_CLOSE, fd);
};

export defn lseek(fd: Int, offset: Int, whence: Int) -> Int {
    term syscall3(SYS_LSEEK, fd, offset, whence);
};
```

#### `lib/std/posix/process.bv`

```brief
// 2026-07-26: Process spawning — clone + execve.
// Replaces ShellCmd from brief_rt.c (which used popen).

import "syscall.bv";

export let CLONE_VM:      Int = 0x100;
export let SIGCHLD:       Int = 17;

export defn clone(flags: Int, stack: Int) -> Int {
    term syscall2(SYS_CLONE, flags, stack);
};

export defn execve(path: Int, argv: Int, envp: Int) -> Int {
    term syscall3(SYS_EXECVE, path, argv, envp);
};

export defn wait4(pid: Int, wstatus: Int, options: Int) -> Int {
    term syscall3(SYS_WAIT4, pid, wstatus, options);
};
```

### 2c. Remove Dead `declare` Statements

**File:** `src/backend/llvm/emit_toplevel.rs:185-269`

The ~40 legacy `declare` statements for functions that don't exist in
`brief_rt.c` were from the pre-intrinsic era. They would cause linker errors
if ever called, but since the calls were also dead, they silently bloated
every generated `.ll` file.

**Audit and removal:** Cross-reference each `declare` against:

1. `lib/runtime/brief_rt.c` — does the C function exist?
2. `lib/std/*.bv` — does any `frgn` reference this symbol?
3. `src/intrinsic_signatures.rs` — is this mapped to an intrinsic?

If none of the three reference it → remove the `declare` line. Functions
confirmed dead and to be removed:

```
__readln__, __sort_list__, __reverse_list__, __range__, __trim_left__,
__trim_right__, __to_lower__, __contains_at__, __find_from__, __splitn__,
__float_to_str, __to_str, __stack_top__, __queue_front__, __hashmap_get__,
__hashset_elements__, __tty_raw_mode__, __spawn_with_output__, __readlink__,
__getcwd__, __readdir__, __sigaction__, __sigprocmask__, __getaddrinfo__,
__map_keys__, __map_values__, __errno__, __getrandom__, __uname__,
__hostname__, __strerror__, __strsignal__, __realpath__, __backtrace__,
__getpwuid__, __getgrgid__, __thread_create__, __thread_join__, __thread_exit__,
__mutex_lock__, __mutex_unlock__, __condvar_wait__, __condvar_signal__,
__condvar_broadcast__, __getrlimit__, __setrlimit__, __mkstemp__, __mkdtemp__,
__dlopen__, __dlsym__, __dlclose__, __ttyname__
```

**2026-07-26: Removed dead LLVM IR declares.**
These ~50 `declare` statements were emitted in every generated `.ll` file but
had no corresponding C implementation in `brief_rt.c`. They were a historical
artifact from before the intrinsic migration. Removing them cleans up the
generated IR and eliminates the risk of linker errors if any were accidentally
referenced. Functions that still have actual C implementations
(`__print_int`, `__read_file__`, `__exit`, thread pool, etc.) are kept until
Phase 2b ports them to Brief.

### After Each Migration Step

For each C function ported to Brief in Step 2b:

1. Write the Brief implementation in `lib/std/posix/*.bv`
2. Compile a minimal test program that uses the new Brief function
3. Verify the output matches the old C function's behavior
4. Remove the C implementation from `lib/runtime/brief_rt.c`
5. Remove the `declare` from `src/backend/llvm/emit_toplevel.rs`
6. Update any `frgn` declarations in `lib/std/` that referenced the C function
7. `cargo test --lib` passes
8. `bash benchmarks/build_and_bench.sh --correctness` passes

---

## Phase 3 — Tamer in Pure Brief (No `Alloc#`, No C)

### 3a. Implement `Int[N]` Array Syntax

Follow `docs/plans/2026-07-25-memory-by-contract.md` steps 1-7. The full
implementation details are in that document; this section summarizes.

**Parser** (`src/parser/types.rs`): After parsing the base type, check for `[N]`:

```rust
// 2026-07-26: Parse Int[1024] as fixed-size array type.
// Emits Type::Vector(inner, vec![Dimension::Anonymous(n)]).
if self.eat(&Token::LBracket) {
    let size = match self.peek() {
        Some(&Token::Integer(n)) => { self.pos += 1; Some(*n as usize) }
        _ => None,
    };
    self.expect(Token::RBracket)?;
    if let Some(s) = size {
        return Ok(Type::Vector(Box::new(base.1), vec![Dimension::Anonymous(s)]));
    }
}
```

**Normalizer** (`src/backend/llvm/mod.rs` `push_field_type()`): Add an arm for
`Type::Vector`:

```rust
// 2026-07-26: Emit LLVM [N x T] type for fixed-size arrays.
// Used by the struct-based tamer VM buffers.
if let Type::Vector(inner, dims) = ty {
    if dims.len() == 1 {
        if let Dimension::Anonymous(n) = dims[0] {
            let inner_llvm = self.llvm_type(inner);
            self.ctx.field_types.push(format!("[{} x {}]", n, inner_llvm));
            self.ctx.field_brief_types.push(ty.clone());
            return;
        }
    }
}
```

**Codegen** (`src/backend/llvm/emit_expr.rs`): Array index `a[i]` for
`Type::Vector` → GEP into `[N x T]` field (not ptrtoint/inttoptr):

```rust
// 2026-07-26: Array index expression for fixed-size struct arrays.
// Emits GEP into [N x T] field instead of ptrtoint+load.
if let Type::Vector(inner, _) = &*array_ty {
    let gep = format!("%gep_{idx} = getelementptr [{} x {}], ptr %state, i32 0, i32 {}, i64 {}",
        n, llvm_inner, field_idx, index);
    writeln!(out, "  {}", gep).ok();
    writeln!(out, "  %val_{idx} = load {}, ptr %gep_{idx}", llvm_inner).ok();
}
```

**Struct emission** (`src/backend/llvm/mod.rs`): `struct S { data: Int[1024] }`
→ `%S = type { [1024 x i64] }` in LLVM.

### 3b. Write Tamer VM in Brief with Struct Arrays

Replace `lib/tamer/main.bv`, `lib/tamer/vm.bv`, `lib/tamer/loader.bv` with
struct-based versions. The existing Brief VM (`vm.bv:314` lines) already has a
working fetch-decode-execute loop — the change is to use struct fields instead
of pointers + offset arithmetic.

#### Struct Definitions

```brief
// 2026-07-26: Tamer VM buffers as struct arrays.
// No Alloc# — sizes are part of the type and proven by contract bounds.
// Max stack depth: 1024 words (worst-case analyzed from .lair bytecode).
// Max locals: 4096 words (worst-case call depth × max args per function).
// Max frames: 256 (worst-case call depth).

struct VMStack { data: Int[1024]; len: Int; };
struct VMLocals { data: Int[4096]; len: Int; };
struct VMFrames { data: Frame[256]; count: Int; };
struct Frame { locals_base: Int; local_count: Int; return_pc: Int; };
```

#### Entry Point (`main.bv`)

```brief
// 2026-07-26: Tamer entry point with struct buffers.
// Reads .bounty from file (via POSIX syscalls), parses sections,
// interprets .lair bytecode, produces LLVM IR.

import "loader.bv";
import "vm.bv";
import "posix/io.bv";
import "posix/syscall.bv";

export defn tame(file_path: Int, file_path_len: Int,
                 output_dir: Int, output_dir_len: Int) -> Int
{
    // 1. Open and read .bounty file via syscalls (no C fopen)
    let fd = open(file_path, O_RDONLY, 0);
    [fd < 0] { term 1; };

    // 2. Get file size via lseek(SEEK_END)
    let file_size = lseek(fd, 0, 2);  // SEEK_END = 2
    [file_size <= 0] { close(fd); term 2; };
    lseek(fd, 0, 0);  // SEEK_SET = 0

    // 3. Read file into buffer (mmap would be better, but MVP uses read)
    // In the struct-based tamer, we'd use a fixed-size buffer.
    // For now, allocate dynamically (will be removed when full struct
    // migration is complete).
    let file_buf = Alloc#(file_size) as Ptr<Int>;
    let bytes_read = read(fd, file_buf as Int, file_size);
    close(fd);
    [bytes_read != file_size] { term 3; };

    // 4. Parse .bounty sections using loader.bv
    let lair_ptr = find_bounty_section(file_buf, 1);   // SECTION_LAIR = 1
    let lair_size = ...;  // extract from section table
    let beastpack_ptr = find_bounty_section(file_buf, 2);  // SECTION_BEASTPACK = 2

    [lair_ptr == 0] { term 4; };

    // 5. Validate .lair header
    let magic = read_u32(lair_ptr, 0);
    [magic != 0x5249414C] { term 5; };  // "LAIR"

    let fn_off = read_u64(lair_ptr, 32);
    let fn_size = read_u64(lair_ptr, 40);
    let bc_off = read_u64(lair_ptr, 48);
    let bc_len = read_u64(lair_ptr, 56);
    let fn_count = fn_size / 20;

    // 6. Execute VM with struct buffers
    let mut stack: VMStack;
    let mut locals: VMLocals;
    let mut frames: VMFrames;

    let _result = vm_loop(
        &mut stack, &mut locals, &mut frames,
        lair_ptr + bc_off, bc_len, lair_ptr + fn_off, fn_count,
        0, 0, 1);

    // 7. Write LLVM IR output (beastpack → LLVM IR emission)
    // MVP: host_llvm_emit via HCALL accumulates IR text,
    // then write to .ll file and invoke clang.

    term 0;
};
```

#### VM Loop (`vm.bv`) — Struct Version

The existing `vm_loop` txn gets rewritten to use struct fields:

```brief
// 2026-07-26: VM loop using struct array buffers.
// stack.data[stack.len - 1] = top of stack.
// locals.data[locals_base + slot] = local variable.
// frames.data[frame_count - 1] = current frame.

txn vm_loop(stack: Ptr<VMStack>, locals: Ptr<VMLocals>,
            frames: Ptr<VMFrames>,
            bc_data: Ptr<Int>, bc_len: Int,
            fn_table: Ptr<Int>, fn_count: Int,
            pc: Int, saved_locals_base: Int,
            running: Int)
    [running != 0]
    [running == 0] -> Int
{
    let op = read_u8(bc_data, pc);
    // Dispatch via exec_op with struct pointers
    let new_pc = exec_op(stack, locals, frames,
                         bc_data, fn_table, fn_count,
                         op, pc, saved_locals_base);

    [new_pc < 0] { &running = 0; };
    [new_pc >= 0] { &pc = new_pc; };
    term 0;
};
```

The key change in `exec_op`: instead of a flat stack pointer and manual
`*(sd + sl)`, use `stack.data[stack.len]`:

```brief
0x06 => { // add
    [stack.len >= 2] {
        stack.data[stack.len - 2] = stack.data[stack.len - 2]
                                  + stack.data[stack.len - 1];
        stack.len = stack.len - 1;
        term pc + 1;
    };
    term pc + 1;
};
```

**The CALL/RET fix from Phase 0a** is ported:

```brief
0x54 => { // call
    let fn_idx = read_u16(bc_data, pc + 1);
    [fn_idx < fn_count] {
        let callee_bc_off = fn_bc_offset(fn_table, fn_idx);
        let callee_local_c = fn_local_count(fn_table, fn_idx);
        let callee_arg_c = fn_arg_count(fn_table, fn_idx);

        // Save return state on CURRENT frame (before pushing new frame)
        frames.data[frames.count].return_pc = pc + 3;
        frames.data[frames.count].locals_base = locals.len;
        frames.data[frames.count].local_count = callee_local_c;
        frames.count = frames.count + 1;

        // Copy args from stack to local slots
        [callee_arg_c >= 1] { locals.data[locals.len + 0] = stack.data[stack.len - 1]; };
        [callee_arg_c >= 2] { locals.data[locals.len + 1] = stack.data[stack.len - 2]; };
        // ... up to 4 args
        locals.len = locals.len + callee_local_c;
        stack.len = stack.len - callee_arg_c;

        term callee_bc_off;  // jump to callee
    };
    term pc + 3;
};

0x19 => { // ret
    // Pop return value
    let retval: Int = 0;
    [stack.len > 0] { &retval = stack.data[stack.len - 1]; &stack.len = stack.len - 1; };
    // Restore previous frame
    [frames.count > 0] {
        let saved_pc = frames.data[frames.count - 1].return_pc;
        locals.len = frames.data[frames.count - 1].locals_base;
        frames.count = frames.count - 1;
        // Push return value on caller's stack
        stack.data[stack.len] = retval;
        &stack.len = stack.len + 1;
        [saved_pc >= 0] { term saved_pc; };
    };
    term -1;  // halt
};
```

**Contract bounds:** Every array access is guarded:
- `[stack.len >= 1]` before pop
- `[stack.len < 1024]` before push
- `[frame.count > 0]` before ret
- `[frame.count < 256]` before call

These contracts prove buffer safety at compile time. The backend can eliminate
the bounds checks after proving they never fail.

### 3c. LLVM IR Emission from Brief

The `.beastpack` contains the typed AST. Two approaches:

**MVP (via HCALL):** The `.lair` interpreter calls `host_llvm_emit(text_segment)`
via OP_HCALL. The host function appends to an LLVM IR text buffer. After the VM
halts, the accumulated IR is written to a `.ll` file via `posix::write()`.

**Future (pure Brief):** Walk the `.beastpack` binary format in Brief and emit
LLVM IR text directly. This eliminates the HCALL dependency. The beastpack
format is:
- 4 bytes: section type
- 8 bytes: item count
- Items: type (4 bytes) + data (variable length)

The Brief code would iterate items, match on type, and produce LLVM IR text.

**For this plan:** Implement MVP via HCALL in Phase 3, defer pure-Brief
beastpack walking to a follow-up.

### 3d. Clang Invocation from Brief

After the `.ll` file is written to disk, invoke clang:

```brief
// 2026-07-26: Invoke system clang via clone + execve.
// Replaces ShellCmd# and the C popen dependency.

export defn compile_to_binary(ll_path: Ptr<Int>, ll_len: Int,
                              output_path: Ptr<Int>, output_len: Int) -> Int
{
    // Build argument array for execve
    let args: Int[16];
    args[0] = str_to_ptr("/usr/bin/clang");
    args[1] = str_to_ptr("-O3");
    args[2] = str_to_ptr("-flto");
    args[3] = str_to_ptr("-march=native");
    args[4] = str_to_ptr(ll_path);
    args[5] = str_to_ptr("-o");
    args[6] = str_to_ptr(output_path);
    args[7] = str_to_ptr("-lm");
    args[8] = 0;  // NULL terminator

    let pid = clone(CLONE_VM | SIGCHLD, 0);
    [pid == 0] {
        // Child process
        execve(args[0], &args, 0);
        // If execve returns, it failed
        exit(127);
    };
    [pid > 0] {
        // Parent process
        let status: Int;
        wait4(pid, &status, 0);
        term status;
    };
    term 1;  // clone failed
};
```

---

## Phase 4 — DAG-Based Buffer Size Inference (Stretch)

Once the tamer works with fixed static buffers (Phase 3), extend the analysis
to compute *minimum sufficient* sizes from the `.lair` bytecode itself.

### 4a. Bytecode Static Analysis in Brief

The tamer receives a `.lair` file at runtime. Before allocating buffers, it can
analyze the bytecode to determine worst-case resource usage:

```brief
// 2026-07-26: Static analysis of .lair bytecode to compute
// minimum sufficient buffer sizes. Implemented in Brief, runs
// before VM execution. Proves bounds contractually.

export defn analyze_max_stack(fn_table: Ptr<Int>, fn_count: Int,
                               bc_data: Ptr<Int>, bc_len: Int) -> Int
{
    // 1. Build call graph from function table
    // 2. Walk each function's bytecode, tracking stack depth
    // 3. Find worst-case depth across all call paths
    // 4. Return max stack words needed
    // This is a dataflow analysis in Brief itself.
    term 1024;  // MVP: return fixed max, prove it's sufficient
};
```

### 4b. Dynamic-Size Struct Arrays

Future Brief feature: dimension from expression:

```brief
// NOT YET IMPLEMENTED — conceptual only.
let max_stack = analyze_max_stack(...);
let mut stack: Int[max_stack];  // future: dynamic-size struct array
```

This requires runtime-sizeable stack allocations, which LLVM supports via
`alloca` with a dynamic operand. The Brief type system would need to allow
`Int[expr]` where `expr` is a runtime value proven bounded by contract.

### 4c. DAG Integration with `src/analysis/allocation.rs`

The existing allocation analysis (`src/analysis/allocation.rs`) operates on
the Brief AST at compile time. Extending it to also analyze `.lair` bytecode
at runtime (in the Brief tamer) is a different analysis — it would be written
in Brief and operate on the VM bytecode format, not the Rust AST. The two
analyses serve different purposes:

- **Compile-time (Rust):** Analyzes the program being compiled to choose
  Alloc# strategies (Arena/Alloca/Malloc/Inline).
- **Runtime (Brief):** Analyzes the `.lair` bytecode being interpreted to
  compute minimum buffer sizes.

**Phase 4 implements the runtime analysis only.**

---

## Testing Strategy

### Per-Phase Test Matrix

| Phase | Tests | Type | Verification |
|-------|-------|------|-------------|
| 0a | `double_call.bv` — call function twice, sum results | Integration | `result == a + b` |
| 0a | `nested_call.bv` — A calls B calls C, all return values | Integration | All values propagated correctly |
| 0b | `logical_not.bv` — `(1 == 2) not` → 1, `(1 == 1) not` → 0 | Integration | Correct boolean inversion |
| 0b | `bitwise_not.bv` — `~1` → `0xFFFFFFFE` | Integration | Correct bitwise NOT via OP_BNOT |
| 0c | `lib/tamer/loader.bv` type check | Unit | `cargo test --lib` passes |
| 0d | `head -c 3 test.ll \| xxd` | Manual | No BOM bytes |
| 1a | `protocol_map` deserialization test | Unit | TOML → `ProtocolMap` correctly |
| 1b | `from #System` frgn → `ResolvedFrgn.protocol_lib` | Integration | `Some("libc.so.6")` on linux |
| 1b | `from #SomethingElse` → compile error: "only supported protocol" | End-to-end | Error message |
| 2a | `SysCall#(SYS_write, 1, msg, len)` → stdout | Integration | "Hello" printed |
| 2a | `SysCall#(SYS_exit_group, 42)` → exit code 42 | Integration | Shell shows 42 |
| 2b | Each ported I/O function against C equivalent | Comparison | Same output |
| 2c | Generated `.ll` has no dead declares | Script | grep count == expected |
| 3a | `struct S { d: Int[16] }` → LLVM `[16 x i64]` | Snapshot | Correct LLVM type |
| 3a | `let x = s.d[i]` → GEP load | Integration | Correct value |
| 3b | Tamer processes `.bounty` → native binary | End-to-end | Binary runs |
| 3d | clang invocation via Brief clone+execve | Integration | `.ll` → binary |

### Regression Guard

After every phase commit:

```bash
cargo test --lib
cargo build --release
bash benchmarks/build_and_bench.sh --correctness
bash benchmarks/compare_baseline.sh all  # only for Phase 0 — subsequent phases
                                         # change the pipeline fundamentally
```

Phase 0 must not regress any benchmark. Phases 1-3 fundamentally change the
runtime and tamer; existing benchmarks that depend on C runtime functions must
be updated to use the new Brief equivalents. Run `--correctness` only for
phases that should preserve behavior.

### Key Failsafe

If at any point `cargo test --lib` fails, stop and diagnose. Do not proceed
to the next phase with failing tests.

---

## Key Files Reference

| File | Phase | Role |
|------|-------|------|
| `tamer/interp.c` | 0a, 0b | Fix CALL return_pc save, fix OP_NOT logical |
| `tamer/interp.h` | 0b | Add OP_BNOT opcode |
| `lib/tamer/vm.bv` | 0a, 3b | Fix CALL/RET in Brief VM, rewrite with struct arrays |
| `lib/tamer/loader.bv` | 0c | Remove duplicates, keep cast-based Version 1 |
| `lib/tamer/combined.bv` | 0a, 0b, 0c | Mirror all changes from vm.bv + loader.bv |
| `lib/tamer/main.bv` | 3b | Entry point with struct buffers, POSIX I/O |
| `lib/tamer/host_ffi.bv` | 3c | Wire up protocol imports for I/O |
| `config/targets.toml` | 1a | Add `protocol_map` per target |
| `src/target.rs` | 1a | Deserialize `protocol_map` into `TargetEntry` |
| `src/analysis/frgn_dispatch.rs` | 1b | Resolve `FromSpec::Protocol` |
| `src/intrinsic_signatures.rs` | 2a | Add `variadic` field to `Signature` |
| `src/backend/llvm/intrinsics.rs` | 2a | `SysCall#` inline asm for x86_64 + aarch64 |
| `src/backend/llvm/emit_toplevel.rs` | 2c | Remove ~50 dead `declare` statements |
| `lib/runtime/brief_rt.c` | 2b | Gradually delete functions as ported |
| `lib/std/posix/syscall.bv` | 2b | **New:** syscall wrappers with named constants |
| `lib/std/posix/io.bv` | 2b | **New:** print, exit, file I/O via SysCall# |
| `lib/std/posix/process.bv` | 2b | **New:** clone + execve wrappers |
| `lib/std/posix/env.bv` | 2b | **New:** environment variable access |
| `lib/std/posix/mem.bv` | 2b | **New:** brk/mmap heap wrappers (unused by tamer) |
| `lib/std/posix/thread.bv` | 2b | **New:** thread pool via clone(CLONE_VM) |
| `src/parser/types.rs` | 3a | Parse `Int[N]` syntax |
| `src/backend/llvm/mod.rs` | 3a | Normalize `Type::Vector` → `[N x T]` |
| `src/backend/llvm/emit_expr.rs` | 3a | GEP codegen for array index |
| `docs/plans/2026-07-25-memory-by-contract.md` | 3a | Reference for array syntax implementation |
| `docs/architecture/conditional-ffi.md` | 1 | Update with completed protocol_map resolution |

---

## Documentation Maintenance Plan

Every commit in this plan must update the relevant documentation. The following
table specifies which documents change in which phase:

| Document | Phase | What to update |
|----------|-------|----------------|
| `BUGS.md` | 0 | Log CALL bug root cause (return_pc on wrong frame). Log OP_NOT fix. |
| `tamer/interp.c` (comment) | 0a | Add rationale comment at OP_CALL explaining the fix. |
| `lib/tamer/vm.bv` (comment) | 0a | Add rationale comment at OP_CALL/OP_RET for the Brief VM fix. |
| `lib/tamer/loader.bv` (comment) | 0c | Update file header to document only Version 1 is used. |
| `docs/architecture/conditional-ffi.md` | 1 | Document completed protocol_map resolution. Add `protocol_lib` field description. |
| `docs/architecture/backend-type-dispatch.md` | 1 | If protocol resolution changes type dispatch, update. |
| `config/targets.toml` | 1a | Add `protocol_map` entries for each target. |
| `docs/plans/2026-07-25-memory-by-contract.md` | 3a | Update implementation status; note which steps are complete. |
| `docs/architecture/features/backend-dispatch.md` | 2a | Document `SysCall#` inline asm dispatch by target triple. |
| `docs/architecture/bounty-architecture.md` | 3b | Document the pure-Brief tamer's architecture. |
| `AGENTS_HISTORY.md` | End | Major session milestones for each phase. |

### Rationale Comment Format (Every Modified Code Site)

```rust
// 2026-07-26: <short description of why this exists>
// <what problem it solves, what pattern it targets, how to undo>
// See docs/plans/2026-07-26-tamer-zero-c-and-static-memory.md
```

---

## Risks and Mitigations

| Risk | Phase | Likelihood | Impact | Mitigation |
|------|-------|-----------|--------|------------|
| CALL bug fix breaks existing .lair programs | 0a | Low | High | Write test that calls function twice; run all cargo tests |
| Inline asm `SysCall#` not portable | 2a | Medium | Medium | Keep C fallback for non-Linux targets |
| Porting a C function to Brief introduces a subtle bug | 2b | Medium | Medium | Compare output of C and Brief versions side-by-side |
| `Int[N]` syntax conflicts with existing `>>` token issue | 3a | Low | Low | Already handled: `>>` in nested generics requires space; `Int[1024]` uses brackets, not angle brackets |
| Brief VM struct-array version is slower than pointer version | 3b | Low | Medium | Contracts prove bounds; LLVM optimizes proven-safe GEP chains |
| Run-time of Phase 2b is too high (porting 15+ C functions) | 2b | Medium | Low | Each function is independent; parallelize porting |
| `clang` not available on target system | 3d | Low | Medium | Future: bundle a small C compiler or switch to `llc` directly |

---

## Plan Documents Referenced

| Document | Relationship |
|----------|-------------|
| `docs/plans/2026-07-26-tamer-completion-and-bugfixes.md` | Phase 0 fixes items 1, 2, 4, 5, 6 from this plan |
| `docs/plans/2026-07-25-memory-by-contract.md` | Phase 3a implements steps 1-7: `Int[N]` syntax |
| `docs/architecture/conditional-ffi.md` | Phase 1 completes the `from #System` design |
| `docs/architecture/casting-protocol.md` | Protocol infrastructure for `#System` |
| `docs/plans/2026-07-15-compiletime-meta-and-plugin-architecture.md` | Target config and protocol_map |

---

## Per-Commit Checklist

Every commit in this plan must:

1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. All modified code sites have `// 2026-07-26:` rationale comments
4. Relevant docs updated per Documentation Maintenance Plan
5. No `todo!()`, `unreachable!()`, or `// TODO:` in committed code
6. Behavioral tests for the new/changed functionality
7. `git add` only intended files — inspect `git status` and `git diff` first
8. Commit message references this plan document
9. Run Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
