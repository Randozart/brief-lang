# Metadata Dispatch & Distributed Backend Verification

**Date:** 2026-07-11
**Status:** Architecture document
**Applies to:** Frontend compiler, all backends (LLVM, CIRCT, Webstack, custom)
**Key phases:** Phase 1B (property system), Phase 8G (intrinsic/inop removal),
Phase 12 (`.dbvl` archive), Phase 8A–8F (Pure Bits refactor)

---

## 1. The Core Principle

The compiler frontend does not know what instructions a backend will emit.
It does not validate register names, instruction mnemonics, FPGA block RAM
capacities, or WebAssembly opcodes. **It cannot** — it has no knowledge of
the target hardware.

Instead, the frontend guarantees only language-level properties:
- Syntax is valid
- Types are consistent
- Contracts are satisfied
- Metadata keys are valid identifiers
- Metadata values are valid property types (string, int, bool, list)

The frontend also validates metadata it has domain knowledge to verify.
For example, `alloc("Stack")` triggers escape analysis, and
`alloc(0x4000_2000)` verifies the address is a compile-time constant.
Metadata the frontend cannot validate is passed through opaquely.

Everything target-specific is delegated to the backends through
**opaque metadata strings**. A metadata value is a payload that the
frontend carries but never interprets. Each backend reads only the keys
it understands, validates them against its target, and either emits code
or reports an error.

### Validation Rules

1. **Unknown key** → silently ignored. The backend has no opinion about
   the key. Forward compatibility: new backends add keys without breaking
   existing backends.

2. **Known key + supported value** → emit code. The backend understands
   the key and the value, and can fulfill the request.

3. **Known key + unparseable/unsupported value** → **error**. The backend
   recognizes the key but cannot fulfill the value. The error message MUST
   include the key name, the value, and the reason for rejection. Example:
   `alloc("QuantumGravityZone")` produces an error because the LLVM backend
   knows the `alloc` key but doesn't recognize `"QuantumGravityZone"`.
   A future backend that does recognize it simply handles it.

This is **Distributed Metadata Validation**: verification is performed
at the layer that possesses the relevant domain knowledge, not earlier.

---

## 2. Metadata Lifecycle

```
 Source code           Parsing                 Archive                Backend
 . . . . . .    . . . . . . . . . . .    . . . . . . . . . .    . . . . . . . .
                                           
 defn add_i64     ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
   a: Int         │  Frontend    │       │  .dbvl       │       │  briev-llvm  │
   b: Int         │  parser      │──────→│  archive     │──────→│              │
 -> Int           │              │       │              │       │  reads       │
 {                │  type-checks │       │  defn add_i64│       │  llvm_instr  │
   llvm_instr     │  validates   │       │    llvm_instr│       │  llvm_asm    │
   <~ "add nsw    │  metadata    │       │    = add nsw │       │              │
     i64";        │  keys/values │       │    interpreter│      │  validates   │
   interpreter_impl│             │       │    _impl     │       │  against     │
   <~ "rust_add   │  ignores     │       │    = rust_add│       │  target      │
     i64";        │  *contents*  │       │    _i64      │       │  triple      │
 }                │  of strings  │       │              │       │              │
                  └──────────────┘       └──────────────┘       └──────────────┘
                         │                       │                       │
                    Frontend                 Archive                  Backend
                    validates:               format:                  validates:
                    • syntax                • .dbvl tagged            • llvm_instr is
                    • type checking           lines                    valid LLVM IR
                    • contracts             • one entry               • llvm_asm is
                    • metadata key            per defn                 valid x86 asm
                      is valid ident        • metadata in             • registers
                    • metadata value          {key:val}                match target
                      is valid type           format                   triple

```

### The Identifier-vs-String Rule

A metadata value is either a **frontend-intrinsic identifier** or a
**backend-intrinsic opaque string**:

| Form | Meaning | Example |
|---|---|---|
| `key <~ Identifier` | Frontend interprets this value | `formatting <~ Quoted` |
| `key <~ "string"` | Backend interprets this value; frontend carries it | `llvm <~ "%String"` |

**Rule:** If the frontend must understand the value to compile the program,
it is an identifier. If the value only matters to a backend (or the
compile-time interpreter), it is a string.

Frontend-intrinsic values use capitalized identifiers: `Quoted`, `Bare`,
`Decimal`, `Add`, `Sub`, `Drop`. These are matched by the type checker
and compile-time evaluator.

Backend-intrinsic values use quoted strings: `"%String"`, `"add nsw i64"`,
`"rdtsc"`, `"i64.const 0"`. The frontend never inspects their contents;
it carries them through to the `.dbvl` archive where backends consume them.

### Stage 1: Parsing and Frontend Validation

The frontend parser reads metadata declarations (`<~ expr;`) in function
bodies. For each declaration it:

1. **Validates the key** is a valid identifier (`llvm_instr`, `circt_op`, etc.)
2. **Validates the value** is a valid property type (String, Int, Bool, List)
3. **Checks nothing else** — the string contents are opaque

No match on key names exists in the frontend. No validation of metadata
values against target architectures. The property system (Phase 1B) stores
metadata as `HashMap<String, PropertyValue>` key-value pairs on the
`Definition` and `Transaction` AST nodes.

```rust
// Frontend validation — this is ALL it does for metadata:
fn parse_body_metadata(&mut self) -> Result<HashMap<String, PropertyValue>, SyntaxError> {
    // Parses: key <~ value;
    // Validates: key is identifier, value is literal/identifier/list
    // Stores: HashMap<String, PropertyValue>
    // Does NOT check: key names, string contents, target relevance
}
```

### Stage 2: Archive Serialization

When the frontend emits a `.dbvl` archive (Phase 12), it serializes
metadata as opaque `{key:val}` pairs in the `defn` entry:

```dbvl
defn,add_i64,a:Int|b:Int,Int,"{term a + b}",,,
  {llvm_instr:"add nsw i64" interpreter_impl:"rust_add_i64" circt_op:"comb.add"}
```

The archive format does not distinguish between "understood" and
"vendor-specific" metadata keys. All keys are carried verbatim.

### Stage 3: Backend Consumption

Each backend reads the archive and inspects only the metadata keys
it recognizes. Unknown keys are silently ignored.

```rust
// Backend dispatch — each backend checks only its own keys:
fn emit_function_call(defn: &DefnEntry, args: &[Value], ctx: &BackendContext) -> Result<()> {
    // LLVM backend checks llvm_instr, llvm_asm, llvm_asm_constraints
    if let Some(instr) = defn.get_metadata_str("llvm_asm") {
        let constraints = defn.get_metadata_str("llvm_asm_constraints")
            .unwrap_or("");
        if ctx.target_arch == "x86_64" || ctx.target_arch == "i386" {
            emit_asm_call(instr, constraints, args)?;
        } else {
            return Err(BackendError::UnsupportedAsm {
                instruction: instr.to_string(),
                target: ctx.target_triple.clone(),
            });
        }
    } else if let Some(instr) = defn.get_metadata_str("llvm_instr") {
        emit_inline_ir(instr, args)?;
    } else {
        emit_standard_call(defn, args)?;
    }

    // CIRCT backend ignores llvm_* keys entirely, checks circt_op and hls_*
    // WASM backend ignores llvm_* and circt_*, checks wasm_op
}
```

---

## 3. Metadata Key Namespace Convention

To prevent collisions between backends, metadata keys follow a
**prefix convention**:

| Prefix | Consumed by | Example |
|--------|-------------|---------|
| `alloc` | Frontend + all backends | Allocation annotation | `"Stack"`, `0x4000_2000`, `"Arena", ptr` |
| `llvm_*` | `briev-llvm` backend | `llvm_instr`, `llvm_asm`, `llvm_asm_constraints`, `llvm_entry_arg` |
| `circt_*` | `briev-circt` hardware backend | `circt_op`, `circt_module` |
| `hls_*` | `briev-circt` HLS pass | `hls_storage`, `hls_capacity` |
| `wasm_*` | `briev-webstack` backend | `wasm_op`, `wasm_module` |
| `gpu_*` | GPU backends (future) | `gpu_layout`, `gpu_workgroup_size` |
| `interpreter_*` | Compile-time interpreter | `interpreter_impl` |
| No prefix | All backends (standard Briev) | `bytes`, `alignment` |

Backends MUST NOT read keys with other backends' prefixes. For example,
`briev-llvm` must never read `circt_op`, and `briev-circt` must never
read `llvm_asm`. Unknown keys are silently ignored — this ensures
forward compatibility when new backends add new keys.

---

## 4. Distributed Validation — The Responsibility Model

### 4.1 Frontend Responsibilities

| What | Validated? | How |
|------|------------|-----|
| Metadata key is valid identifier | ✅ Yes | Parser: `key <~ value;` |
| Metadata value is valid property type | ✅ Yes | Parser: string, int, bool, list, identifier |
| Metadata key is not a Briev reserved word | ✅ Yes | Parser: reserved word check |
| Metadata value *contents* (e.g., `"add nsw i65"`) | ❌ No | Opaque — backend's job |
| Metadata key is known to any backend | ❌ No | Any key is valid; unknown keys ignored |
| Metadata is internally consistent (e.g., constr. match) | ❌ No | Backend validates its own constraints |

### 4.2 LLVM Backend Responsibilities (`briev-llvm`)

| Metadata key | Validation performed |
|--------------|---------------------|
| `llvm_asm` | Parse as LLVM `call asm` string. Validate instruction is valid for target triple (x86, ARM, RISC-V). Reject with clear error if unsupported. |
| `llvm_asm_constraints` | Parse register constraints (`"={ax}"`, `"{dx}"`). Validate registers exist on target. Validate constraint syntax matches LLVM expected format. |
| `llvm_instr` | Parse as LLVM IR instruction. Validate operands and types match surrounding IR. Reject malformed IR with source location. |
| `llvm_entry_arg` | Validate value is `"argc"` or `"argv"`. Verify the state field type matches the entry parameter (`i32` for `argc`, `ptr` for `argv`). On `main` emission, wire the entry parameter into this state field. |
| `alloc` | If value is a string, validate it's a known allocation strategy (`"Stack"`, `"Heap"`, `"Arena"`, etc.) or error. If value is an integer (physical address), validate address is in the target memory map or error. If value is a list `[strategy, ptr]`, validate the pointer is non-null. Unknown string values produce an error (known key, unparseable value). |
| Unknown keys | Silently ignored. |

### 4.3 CIRCT Backend Responsibilities (`briev-circt`)

| Metadata key | Validation performed |
|--------------|---------------------|
| `circt_op` | Validate CIRCT/MLIR operation exists in the target dialect. Emit or reject. |
| `hls_storage` | Validate storage type (`"BRAM"`, `"LUTRAM"`, `"FF"`) is available on target device. |
| `hls_capacity` | Validate requested capacity (bytes) fits within physical device limits. Report available vs. requested. |
| `llvm_*` keys | Silently ignored — not relevant to hardware synthesis. |

### 4.4 Webstack Backend Responsibilities (`briev-webstack`)

> **2026-07-26:** The webstack backend is migrating from a TS emitter to a
> WASM-first architecture. Section under review — the `GlueWebGenerator` will
> read additional metadata keys (`web_import`, `dom_binding`, `state_layout`)
> in the new pipeline. See `docs/architecture/features/rendered-briev-wasm.md`.

| Metadata key | Validation performed |
|--------------|---------------------|
| `wasm_op` | Validate WASM opcode exists. Emit as WASM bytecode instruction. |
| `web_import` | Validate import exists in wasm_runtime imports table. |
| `dom_binding` | Validate binding element/handle exists in view compiler output. |
| `llvm_*` / `circt_*` keys | Silently ignored. |

### 4.5 Compile-Time Interpreter Responsibilities

| Metadata key | Validation performed |
|--------------|---------------------|
| `interpreter_impl` | Look up function name in internal Rust dispatch table. Return runtime error if name not found. |
| All other keys | Ignored — interpreter only reads `interpreter_impl`. |

---

## 5. Example Function with Multi-Backend Metadata

```briev
// A single Briev function that compiles to three different targets.
// The frontend validates syntax and types.
// Each backend reads its own keys.

defn read_cpu_cycle() -> UInt64 {
    // LLVM backend: emit x86 RDTSC instruction
    llvm_asm <~ "rdtsc";
    llvm_asm_constraints <~ "={ax},={dx}";

    // CIRCT backend: emit cycle counter register read
    circt_op <~ "hw.cycle_count";

    // Webstack backend: WASM has no cycle counter — emit stub
    wasm_op <~ "i64.const 0";

    // Interpreter: return 0 for compile-time evaluation
    interpreter_impl <~ "emulate_cycle_count";
}
```

What each layer validates:

| Layer | Validates | Ignores |
|-------|-----------|---------|
| Frontend parser | Key `llvm_asm` is valid ident; value `"rdtsc"` is valid string | Everything else |
| `briev-llvm` | `"rdtsc"` is valid x86 instruction; registers exist | `circt_op`, `wasm_op`, `interpreter_impl` |
| `briev-circt` | `"hw.cycle_count"` is valid CIRCT op | `llvm_asm`, `wasm_op` |
| `briev-webstack` | `"i64.const 0"` is valid WASM | `llvm_asm`, `circt_op` |
| Interpreter | `"emulate_cycle_count"` exists in dispatch table | Everything else |

---

## 6. Error Reporting Model

Each backend produces errors only for metadata it recognizes, using its
own domain-specific terminology:

**LLVM backend error for invalid instruction:**
```
error[LLVM-E001]: invalid LLVM instruction in metadata
  ──> std/cpu.bv:4:17
   |
 4 |     llvm_asm <~ "rdtsc";
   |                 ^^^^^^^^
   |
   = target: aarch64-unknown-linux-gnu
   = note: the instruction 'rdtsc' is not available on this target.
   = hint: use 'mrs %0, CNTVCT_EL0' to read the cycle counter on ARM
```

**CIRCT backend error for insufficient BRAM:**
```
error[CIRCT-E002]: synthesis capacity exceeded
  ──> config/device.bv:12:17
   |
12 |     hls_capacity <~ 1048576;
   |                    ^^^^^^^^
   |
   = target: xc7z020 (Zynq-7020)
   = note: requested 1 MB BRAM, but device has only 560 KB
   = hint: reduce 'hls_capacity' or switch to a larger device
```

**Frontend does not produce either of these errors.** It cannot — it
doesn't know what a `rdtsc` is or what a `xc7z020` has in BRAM. This
is correct behavior: each error is reported by the layer that has the
domain expertise to diagnose it.

---

## 7. Adding a New Backend

A developer writing a custom backend (e.g., `briev-spirv` for Vulkan
compute shaders) follows this process:

1. **Define metadata keys** with the convention prefix:
   - `spirv_capability <~ "Int64";`
   - `spirv_instruction <~ "OpIAdd";`

2. **Read only your keys** in the backend:
   ```rust
   fn translate_defn(defn: &DefnEntry) -> Result<SpirVInstruction, SpirVError> {
       if let Some(cap) = defn.get_metadata_str("spirv_capability") {
           // Emit SPIR-V capability declaration
       }
       if let Some(instr) = defn.get_metadata_str("spirv_instruction") {
           // Emit SPIR-V instruction
       }
       // llvm_*, circt_*, wasm_* keys are silently ignored
   }
   ```

3. **Validate in your backend**, not in the frontend:
   - Check that `spirv_capability` values are valid SPIR-V capability names
   - Check that `spirv_instruction` values correspond to valid opcodes
   - Report errors in your own error format with your own error codes

4. **Benefit from frontend guarantees**:
   - Types are already checked
   - Contracts are already verified
   - Metadata keys are valid identifiers
   - Metadata values are valid property types
   - The `.dbvl` archive is well-formed

No changes to the frontend parser, AST, type-checker, or archive format
are needed to support a new backend.

---

## 8. The `llvm_asm` Inline Assembly Hatch (Specific Case)

### Syntax

```briev
defn read_cycle_counter() -> UInt64 {
    llvm_asm <~ "rdtsc";
    llvm_asm_constraints <~ "={ax},={dx}";
    llvm_asm_volatile <~ true;  // optional, defaults to true
}
```

### Frontend handling

The frontend sees three metadata key-value pairs:
- `llvm_asm`: String value — opaque
- `llvm_asm_constraints`: String value — opaque
- `llvm_asm_volatile`: Bool value — opaque

Result: serialized into `.dbvl` as:

```dbvl
defn,read_cycle_counter,,UInt64,"",
  {llvm_asm:rdtsc llvm_asm_constraints:={ax},={dx} llvm_asm_volatile:true}
```

### LLVM backend handling

```rust
fn emit_defn_call(defn: &DefnEntry, args: &[LlvmValue], ctx: &LlvmContext) -> Result<LlvmValue> {
    if let Some(asm_str) = defn.get_metadata_str("llvm_asm") {
        let constraints = defn.get_metadata_str("llvm_asm_constraints").unwrap_or("");
        let volatile = defn.get_metadata_bool("llvm_asm_volatile").unwrap_or(true);

        // Validate for target
        ctx.validate_inline_asm(asm_str, constraints)?;

        // Emit LLVM inline assembly
        let asm_call = if volatile {
            format!("call i64 asm sideeffect \"{}\", \"{}\"()", asm_str, constraints)
        } else {
            format!("call i64 asm \"{}\", \"{}\"()", asm_str, constraints)
        };
        writeln!(ctx.output, "  {} = {}", ctx.gen_reg(), asm_call)?;
        return Ok(());
    }
    // ... standard call path
}
```

### Validation performed by LLVM backend

1. Parse the asm string and constraints
2. Check that the instruction mnemonic is valid for the target triple
3. Check that register names in constraints exist on the target
4. Check that constraint syntax matches LLVM's expected format
5. Report errors with backend-specific diagnostics

### What happens on non-LLVM targets

If the same function is compiled for FPGA (CIRCT), the `llvm_asm` key is
silently ignored. The backend reads `circt_op <~ "hw.cycle_count"` instead
and emits the appropriate hardware counter. If neither key matches, the
backend reports a clear error:

```
error[CIRCT-E003]: no implementation for function 'read_cycle_counter'
  ──> std/cpu.bv:1:1
   |
 1 | defn read_cycle_counter() -> UInt64 {
   |      ^^^^^^^^^^^^^^^^^^^^
   |
   = target: xc7z020
   = note: this function has no 'circt_op' or 'hls_*' metadata
   = hint: add 'circt_op <~ "hw.cycle_count";' to this function
```

---

## 9. Backend Composition

A single `.dbvl` archive can be processed by multiple backends in sequence.
For example, a CI pipeline might:

```bash
# 1. Frontend produces archive
briev compile src/main.bv --archive build/main.dbvl

# 2. Run LLVM backend — validates llvm_* keys
briev-llvm build/main.dbvl --output build/a.out

# 3. Run CIRCT backend — validates circt_*, hls_* keys (same archive)
briev-circt build/main.dbvl --output build/design.v

# 4. Run custom compliance checker — validates pii_*, gdpr_* keys
briev-compliance build/main.dbvl --output build/compliance-report.json
```

Each backend validates only its relevant keys. No backend can fail due to
metadata intended for another backend. This is guaranteed by the key prefix
convention and the "ignore unknown keys" rule.

---

## 10. Forward Compatibility

When a new compiler version adds metadata keys, old backends simply ignore
them. When a new backend adds keys, the frontend and other backends are
unaffected. This means:

- You can update the standard library to add `wasm_op` annotations without
  breaking LLVM or CIRCT compilation
- You can add a new metadata key in a minor compiler release without a
  major version bump
- Third-party backends can coexist without coordination, as long as they
  use unique key prefixes

The only breaking change would be removing a metadata key that a backend
depends on — and that is detected at compile time by the backend itself,
not by the frontend.

---

## 11. Relationship to Other Architecture Documents

| Document | Relationship |
|----------|--------------|
| `docs/architecture/bits-thesis.md` | Establishes Bits as sole physical primitive; metadata dispatch is the mechanism that gives Bits meaning |
| `docs/architecture/features/property-system.md` | Phase 1B — defines how metadata is stored and queried on AST nodes |
| `docs/plans/2026-07-11-pure-bits-refactor.md` | Phases 8A–8F — builds `execute_intrinsic` and property-based operator dispatch that 8G completes |
| `docs/plans/2026-07-11-derivation-synthesis-comprehensive.md` | Phase 8G — removes `Intrinsic` enum, `inop`, `Expr::IntrinsicCall`; finalizes metadata-only dispatch |
| `docs/architecture/archive.md` | Phase 12 — defines `.dbvl` archive format that carries metadata to backends |
| `docs/architecture/features/backend-dispatch.md` | How backends are selected and invoked |

---

## 12. Summary

| Property | Value |
|----------|-------|
| Frontend validates | Syntax, types, contracts, metadata key/value types |
| Frontend does NOT validate | Metadata string contents, target compatibility, register names |
| Backend validates | Its own metadata keys against its target architecture |
| Unknown keys | Silently ignored by all backends |
| Error reporting | Each backend in its own domain language |
| Adding a new backend | Define new key prefix, read only your keys, validate yourself |
