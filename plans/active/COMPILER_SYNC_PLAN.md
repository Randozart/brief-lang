# Compiler Sync Plan: Rust Bootstrap ↔ Brief Self-Hosted

**Date:** 2026-05-25
**Status:** GAP ANALYSIS COMPLETE — READY FOR PHASED IMPLEMENTATION

---

## Executive Summary

The Brief compiler exists in two versions that have diverged:

| Aspect | Rust Bootstrap (`src/`) | Brief Self-Hosted (`lib/compiler/`) |
|--------|------------------------|-------------------------------------|
| Language | Rust | Brief (`.bv`) |
| Total size | ~46,293 lines (26 modules) | ~8,470 lines (14 modules) |
| Parser | 5,480 lines | 1,770 lines |
| AST types | ~50 type definitions | ~25 type definitions |
| Typechecker | 1,663 lines | 681 lines |
| Proof engine | 2,701 lines (14 check fns) | 810 lines (14 fns, 6 fewer checks) |
| CLI commands | 17+ commands | 1 command (stub) |

**The gap runs both ways.** Rust is ahead on v0.14 language features and production backends; Brief is ahead on AArch64 codegen and has a different WASM architecture.

---

## Priority: Phase Ordering

1. **WASM/Webstack split** — critical architecture decision
2. **Sync v0.14 features** — parser/AST/typechecker/proof engine from Rust → Brief
3. **Backport AArch64** — from Brief → Rust
4. **Create missing backends** — COBOL, TCL in Brief
5. **Sync remaining backends** — C, Rust, Verilog, VHDL, x86_64

---

## Phase 1: WASM / Webstack Split

### Problem

The Rust compiler's `wasm.rs` (2,022 lines) generates Rust+JS glue → wasm-pack → wasm. The Brief compiler's `wasm.bv` (351 lines) describes direct `.wasm` binary generation. These are **two different targets** sharing one name.

### Decision

Split into two distinct backends:

- **WASM** — Direct WebAssembly binary generation (`.wasm` file). Brief's `wasm.bv` is the authoritative spec.
- **Webstack** — Rust+JS glue → wasm-pack pipeline (`.rs` + `.js` + HTML). Rust's `wasm.rs` is the authoritative source.

### Tasks

#### Rust compiler (`src/`)

| Task | File | Description |
|------|------|-------------|
| 1.1 | `src/backend/webstack.rs` | Rename `wasm.rs` → `webstack.rs`. Update all internal references, module paths, imports |
| 1.2 | `src/backend/wasm.rs` | Create new file: direct WASM binary generation. Use Brief's `wasm.bv` as the architectural spec |
| 1.3 | `src/backend/mod.rs` | Register both `wasm::WasmBackend` and `webstack::WebstackBackend` in the backend registry |
| 1.4 | `src/main.rs` | Add `webstack` CLI command alongside existing `wasm`. Update `run_wasm` → dispatch to correct backend based on command name. Update CLI help text. Wire both into `run_compile_unified` for target-spec dispatch. Test with `cargo build && cargo test --lib` |
| 1.5 | `src/main.rs` | Update `run_wasm` signature or add `run_webstack`. Ensure `--emit-memory-spec` and other flags apply to both |

#### Brief self-hosted (`lib/compiler/`)

| Task | File | Description |
|------|------|-------------|
| 1.6 | `lib/compiler/backends/wasm.bv` | Keep as direct WASM binary. Flesh out from 351 → ~2000 lines. Add: WASM section builders (Type, Function, Code, Export, Import, Memory, Data), LEB128 encoding, WASI import stubs, branchless guard compilation, full expression/statement codegen, memory layout |
| 1.7 | `lib/compiler/backends/webstack.bv` | Create new file. Port from Rust's `webstack.rs`. Add: reactive transaction engine, signal dependency graph, dirty tracking, `poll_dispatch`, render directives (b-text, b-show, b-hide, b-each), HTML escaping, FFI JS interop, wasm-pack build orchestration |
| 1.8 | `lib/compiler/main.bv` | Add CLI dispatch for `wasm` and `webstack` commands. Accept `--target` spec. Output correct file types per backend |

### WASM Binary Architecture (from Brief wasm.bv)

The direct WASM backend should produce:

- **Sections**: Type (function signatures), Function (type indices), Code (function bodies), Export (public symbols), Import (WASI/host functions), Memory (linear memory declaration), Data (initial memory contents)
- **Instructions**: WASM opcode encoding, structured control flow (block/loop/if/else/end), local variable management, memory load/store with offset
- **Runtime**: Brief reactor loop compiled to WASM loop, branchless guard execution via select, state encoded as linear memory
- **Types**: i32 for bool/int/ptr, i64 for u64, f64 for float. Arrays encoded as (pointer, length) pairs
- **FFI**: WASI imports for I/O, host function imports via Import section

### Verification

- `brief wasm program.bv` produces valid `.wasm` binary that `wasmtime` or `wasmer` can execute
- `brief webstack program.rbv` produces working browser app (wasm + JS + HTML)
- `cargo build && cargo test --lib` passes

---

## Phase 2: Sync v0.14 Features (Rust → Brief)

Sync these features from the Rust compiler into the Brief self-hosted compiler's parser, AST, typechecker, and proof engine.

### 2.1 Hashtag Modifiers

**Rust source**: `src/parser.rs:185` (`parse_hashtag_modifiers`), `src/ast.rs:822-828` (`Hashtag` struct), lexer token variants `HashBang`, `HashBracket`

**Brief target files**:
- `lib/compiler/token.bv` — add `HashBang`, `HashBracket` token variants
- `lib/compiler/lexer.bv` — lex `#!`, `#?...`, `#[scope]tag`, `#(...)` block syntax
- `lib/compiler/ast.bv` — add `Hashtag` struct with fields: name, value, mandatory (bool), fallback (List<String>), scope (Option<String>). Add `modifiers: Vec<Hashtag>` to Statement::StmtLet, Assignment, Term, Definition, Transaction, StructDefinition
- `lib/compiler/parser.bv` — add `parse_hashtag_modifiers()` function, call from appropriate parse sites
- `lib/compiler/typechecker.bv` — validate hashtag modifiers (pass-through for now, like Rust)

### 2.2 `#on_exit` Block Pragma

**Rust source**: `src/parser.rs:1869` (`parse_block_pragma`), `src/ast.rs:534-537` (`Statement::OnExit`), `src/typechecker.rs:1179-1183`

**Brief target files**:
- `lib/compiler/ast.bv` — add `StmtOnExit(List<Statement>)` to Statement enum
- `lib/compiler/parser.bv` — parse `#identifier { body }` syntax, produce `StmtOnExit`
- `lib/compiler/typechecker.bv` — typecheck OnExit body statements

### 2.3 `+/-` Struct Field Modifiers

**Rust source**: `src/parser.rs:1071-1155` (`parse_struct_variants`, `parse_struct_variant_fields`), `src/ast.rs:742-760` (`StructVariant { contract, fields, additions, removals }`, `StructDefinition { ... variants }`)

**Note**: In the Rust compiler, this feature is **parsed into AST but never consumed by typechecker or backends** — only base struct fields are validated/emitted. The Brief self-hosted port matches this behavior.

**Brief target files**:
- `lib/compiler/ast.bv` — add `StructVariant { contract: Contract, fields: List<StructField>, additions: List<StructField>, removals: List<String> }`. Add `variants: List<StructVariant>` to StructDefinition
- `lib/compiler/parser.bv` — parse `[discriminant] { +field: Type, -field, field: Type }` syntax in `parse_struct_variants` / `parse_struct_variant_fields`
- `lib/compiler/typechecker.bv` — pass `variants` through (no validation, matching Rust's current behavior)

### 2.4 Alka Hatch

**Rust source**: `src/parser.rs:1827` (`parse_alka_block`), `src/ast.rs:531-545` (`Statement::Alka(AlkaBlock)`, `AlkaBlock` struct), `src/typechecker.rs:1184`

**Brief target files**:
- `lib/compiler/ast.bv` — add `AlkaBlock { dangerous: Bool, content: String }` and `StmtAlka(AlkaBlock)` to Statement
- `lib/compiler/parser.bv` — parse `alka { ... }` and `alka! { ... }` syntax
- `lib/compiler/typechecker.bv` — passthrough (no validation of alka content)

### 2.5 Dynamic `@` Address Binding

**Rust source**: `src/parser.rs:2958-2986` (address binding in Let/StateDecl), `src/ast.rs:507-508` (Let.address, Let.address_expr), `src/typechecker.rs:1205`

**Brief target files**:
- `lib/compiler/ast.bv` — add `address: Option<AddressBinding>` to StmtLet and StateDecl. Add `AddressBinding { base: Expr, bit_range: Option<BitRange> }` struct. Extend BitRange to full enum matching Rust's `BitRange::Any(usize)`
- `lib/compiler/parser.bv` — parse `@expr`, `@/bitrange`, `@addr/bitrange`, `stack:`/`heap:` syntax
- `lib/compiler/typechecker.bv` — infer types for `@var` (PriorState) and `@expr` (OwnedRef)

### 2.6 Tuple Destructuring

**Rust source**: `src/ast.rs:369` (`Expr::TupleDestructure`), `src/parser.rs:2909-2946`, `src/typechecker.rs:1113-1125`

**Brief target files**:
- `lib/compiler/ast.bv` — add `ExprTupleDestructure(List<String>, Expr)` to Expr enum
- `lib/compiler/parser.bv` — parse `let (a, b, c) = expr` syntax
- `lib/compiler/typechecker.bv` — check destructure count matches tuple arity

### 2.7 LocalTrigger (`trg!`)

**Rust source**: `src/ast.rs:523-528` (`Statement::LocalTrigger`), `src/parser.rs:3364-3377`, `src/typechecker.rs:1162-1178`

**Brief target files**:
- `lib/compiler/ast.bv` — add `StmtLocalTrigger { name, ty: Type, expr: Expr }` to Statement
- `lib/compiler/token.bv` — add `TrgBang` token for `trg!`/`trigger!`
- `lib/compiler/parser.bv` — parse `trg! name: Type = expr;` syntax
- `lib/compiler/typechecker.bv` — declare variable in scope with inferred/declared type

### 2.8 Strict Mode

**Rust source**: `src/ast.rs:841-844` (`StrictMode` enum), `src/parser.rs:2864-2881` (contract enforcement), `src/proof_engine.rs:766-794` (warning escalation), `src/main.rs:558-561` (extension detection), `src/main.rs:1338-1365` (capability validation), `src/view_compiler.rs:918-937` (view-state isomorphism)

**Brief target files**:
- `lib/compiler/ast.bv` — add `StrictMode` enum (Off/Strict). Add `strict_mode` field to Program struct
- `lib/compiler/parser.bv` — detect `.sbv`/`.sebv`/`.srbv` extensions, enable strict mode. In `parse_contract`, require both pre and post conditions, forbid `[true]`
- `lib/compiler/proof_engine.bv` — add `strict_mode` field; escalate P009/P010 trivial contract warnings to hard errors in strict mode
- `lib/compiler/typechecker.bv` — validate capabilities for `.sebv` (hardware_triggers required)
- Backend files — respect strict mode when emitting contract assertions

### 2.8b Strict Mode Extension: Big-O / Complexity Enforcement (Future)

**Status**: NOT YET IMPLEMENTED in either compiler. Documented 2026-05-25.

**Design intent**: Hyper-strict mode (activated via `--strict --optimize` or `.sbo`/`.sebo`/`.srbo` extensions) adds algorithmic complexity analysis to strict mode:

1. **Big-O detection**: Compiler analyzes all function bodies for worst-case complexity. If a more efficient algorithm exists (e.g., `O(n²)` loop when `O(n)` is possible), compiler emits an error.

2. **Pragma override (`#!optimize`)**: To suppress a complexity error, the programmer must write:
   ```
   #!optimize("O(n²) is intentional — bubble sort for small n")
   ```
   The mandatory comment explaining the override is stored in the AST and checked by Praetor. The override downgrades the error to a warning. No inline `//` or bare `#` exception is permitted.

3. **No bare exceptions**: Just as Praetor forbids `// praetor:ignore`, hyper-strict forbids `# optimize` (advisory) — only `#!optimize` (mandatory with justification) is accepted.

4. **Extension**: `.sbo` (Strict Brief Optimized), `.sebo` (Strict Embedded Brief Optimized), `.srbo` (Strict Rendered Brief Optimized) — or equivalently, `--strict --optimize` CLI flags.

5. **Praetor interop**: Praetor's existing `.praetor.toml` Big-O threshold (`O(n²)`) and complexity limits already validate this externally. The compiler's built-in analysis would be the same logic, moved into the compilation pipeline and gated behind strict mode.

**Implementation deferred until both compilers have strict mode fully ported.**

### 2.9 Multidimensional Vectors with Named Dimensions

**Rust source**: `src/ast.rs:103-107` (`Dimension` enum), `src/parser.rs:3613-3644, 3683-3707`

**Brief target files**:
- `lib/compiler/ast.bv` — add `Dimension { name: Option<String>, size: Int }` struct. Change `TypeVector(Type, Int)` → `TypeVector(Type, List<Dimension>)`
- `lib/compiler/parser.bv` — parse `Vector<T, dim1, dim2>` with optional named dimensions `Vector<T, rows: 32, cols: 16>`
- `lib/compiler/typechecker.bv` — validate dimension counts match operations

### 2.10 Vector Slicing with Stride/Mask/Multidimensional

**Rust source**: `src/ast.rs:339-347` (`Expr::Slice` with stride/mask, `Expr::MultiSlice`), `src/parser.rs:3996-4068`

**Brief target files**:
- `lib/compiler/ast.bv` — extend `ExprSlice` with stride and mask fields. Add `ExprMultiSlice { value, coordinates: List<SliceCoordinate>, mask }`. Add `SliceCoordinate` enum (Index/Range/Named)
- `lib/compiler/parser.bv` — parse `start..end..stride`, `start..end; mask`, multidimensional `v[0..4, 2..8]`, named `v[rows: 0..16]`
- `lib/compiler/typechecker.bv` — validate slice bounds against dimension sizes

### 2.11 List SIMD Length Checking

**Rust source**: `src/proof_engine.rs:1321-1448` (`check_list_simd_lengths`, `check_list_simd_lengths_in_body`)

**Brief target files**:
- `lib/compiler/proof_engine.bv` — add `check_list_simd_lengths` function. Walk program body, identify SIMD binary ops on lists, assert length equality between operands and result. Generate `VerificationCondition` for each.

### 2.12 Backend Hashtag Registry

**Rust source**: `src/backend/mod.rs:49-80` (`supported_hashtags`, `validate_hashtags`, `validate_hashtags_in_program`)

**Brief target files**:
- `lib/compiler/backends/mod.bv` — create this file (module registry for Brief backends). Add `supported_hashtags(backend)`, `validate_hashtags(tags, backend)`, and `validate_hashtags_in_program(program, backend, strict)` functions
- Each backend `.bv` file — export `supported_hashtags()` list
- `lib/compiler/main.bv` — call hashtag validation in compilation pipeline

### 2.13 CLI Dependency Management

**Rust source**: `src/main.rs:3522-3696` (dependency checking from `.dbvs`/`.dbv` files)

**Brief target files**:
- `lib/compiler/main.bv` — implement `deps [check|install|list]` command. Parse `.dbvs`/`.dbv` files for dependencies, validate against local cache, report missing dependencies
- Add `Transaction.dependencies` field to AST (as in Rust)

### 2.14 FFI Infrastructure

**Rust source**: `src/ast.rs:158-219` (`ForeignTarget`, `FfiKind`, `ForeignSignature`), `src/ast.rs:865-870` (`FfiState`)

**Brief target files**:
- `lib/compiler/ast.bv` — replace simple `ForeignBinding` with full `ForeignSignature` (13 fields). Add `ForeignTarget` enum (Native, Wasm, C, Python, Js, Swift, Go). Add `FfiKind` enum (frgn/frgn!/syscall/syscall!). Add `FfiState` struct
- `lib/compiler/parser.bv` — parse full FFI signature syntax
- `lib/compiler/typechecker.bv` — validate FFI declarations

---

## Phase 3: Backport AArch64 (Brief → Rust)

### Current State

| Compiler | File | Lines | Capability |
|----------|------|-------|------------|
| Brief | `backend_aarch64.bv` | 1,654 | Full binary encoding, register allocator, memory layout, 3 optimization passes |
| Rust | `aarch64.rs` | 561 | NASM-style assembly text output, minimal expressions |

### Tasks

| Task | File | Description |
|------|------|-------------|
| 3.1 | `src/backend/aarch64.rs` | Port Brief's instruction enum with binary A64 encodings. All instruction categories: Data Processing (Immediate, Register, Floating-point), Loads/Stores (scalar, SIMD, exclusive), Branches (B/BL/B.cond/CBZ/CBNZ/TBZ/TBNZ), System (MSR/MRS/SVC/DC), Cryptographic |
| 3.2 | `src/backend/aarch64.rs` | Port register allocator: physical register file (X0-X30, V0-V31), callee-saved register management, linear scan allocation, spill/reload with stack slots, predicate register for guards |
| 3.3 | `src/backend/aarch64.rs` | Port memory layout pass: bit-packed layout computation, field offset calculation, MMIO-mapped state detection, stack frame layout (prologue/epilogue), alignment |
| 3.4 | `src/backend/aarch64.rs` | Port optimization passes: transaction fusion (merge adjacent transactions), parallel scheduling (independent state access), guard caching (branchless guard hoisting), memory overlay (alias analysis for stack slots) |
| 3.5 | `src/backend/aarch64.rs` | Port two-pass encoding: Pass 1 measures label distances, Pass 2 emits final bytes. Binary emission function |
| 3.6 | `src/backend/aarch64.rs` | Preserve Rust-only features: entry point with Linux syscall exit, sequential + parallel reactor models with `--schedule` flag, predictive fetch (`PRFM`) using `collect_data_addresses` |
| 3.7 | All | Update `run_arm` in `main.rs` to output binary file (`.bin`) instead of assembly text. Keep `--emit-asm` flag for debugging |

### Verification

- ARM64 binary executes on Linux aarch64 (or via `qemu-aarch64`)
- `cargo test --lib` passes all ARM64 backend tests
- Regression: existing Brief `backend_aarch64.bv` still compiles and passes its tests

---

## Phase 4: Create Missing Backends (Rust → Brief)

### 4.1 COBOL Backend

**Rust source**: `src/backend/cobol.rs` (709 lines)

**Brief target**: `lib/compiler/backends/cobol.bv`

Capabilities to port:
- Free-format COBOL source generation (no column restrictions)
- `WORKING-STORAGE SECTION` with PIC clauses for all Brief types
- `LINKAGE SECTION` for parameter passing
- `IDENTIFICATION DIVISION` / `PROCEDURE DIVISION` structure
- Contract enforcement via `CHECK`/`VERIFY` paragraphs (pre/post conditions)
- `old()` state capture via temporary variables
- Type mapping: Bool→PIC 1, Int→PIC S9(9) COMP, Float→COMP-2, U64→PIC 9(18), String→PIC X(n), enums→88-level condition names
- Idiomatic COBOL: `ADD TO`, `SUBTRACT FROM`, `COMPUTE`, `MOVE CORRESPONDING`
- `IF/END-IF` for guards, `EVALUATE` for multi-branch
- `GOBACK` / `EXIT PARAGRAPH` for termination
- Missing contract → `DISPLAY` + `GOBACK` with return code
- Recursion depth watchdog
- 4 unit tests (from Rust test suite)

### 4.2 TCL Generator

**Rust source**: `src/backend/tcl_generator.rs` (369 lines)

**Brief target**: `lib/compiler/backends/tcl_generator.bv`

Capabilities to port:
- Vivado project creation with part/board resolution
- IP packaging flow (create_project, set_property, synth_ip)
- AXI block design: instantiate Zynq PS, clock wizard, interconnect
- Bitstream build: synthesis, placement, routing, bitgen
- `DECREE OF EXCLUSION` reports for unconnected nets
- Memory-based parallelism: `set_param general.maxThreads` from `nproc`
- Synthesis mode: `out_of_context` vs `global`
- Called from Verilog/VHDL backend via `--tcl` flag

---

## Phase 5: Sync Remaining Backends

For each pair, merge features bidirectionally.

### 5.1 C Backend: Brief → Rust → Brief

| Feature | Rust (908 lines) | Brief (205 lines) | Action |
|---------|-----------------|-------------------|--------|
| FFI bindings/calls/stubs | ✅ | ❌ | Port Rust→Brief |
| Kernel module entry | ✅ | ❌ | Port Rust→Brief |
| MMIO `@link` linkage | ✅ | ❌ | Port Rust→Brief |
| Target spec integration | ✅ | ❌ | Port Rust→Brief |
| Inline asm with clobbers | ✅ | ❌ | Port Rust→Brief |
| Local trigger support | ✅ | ❌ | Port Rust→Brief |
| Alka blocks | ✅ | ❌ | Port Rust→Brief |
| `#on_exit` cleanup | ✅ | ❌ | Port Rust→Brief |
| Error handling (bounds/null) | ✅ | ❌ | Port Rust→Brief |
| State allocation (static/dynamic) | ✅ | ❌ | Port Rust→Brief |
| Test mode | ✅ | ❌ | Port Rust→Brief |
| Hardware register tracking | ✅ | ❌ | Port Rust→Brief |
| Makefile generation | ✅ | ❌ | Port Rust→Brief |
| Basic reactor loop | ✅ | ✅ | (already synced) |
| Guard compilation | ✅ | ✅ | (already synced) |

### 5.2 Rust Backend: Brief → Rust → Brief

| Feature | Rust (783 lines) | Brief (219 lines) | Action |
|---------|-----------------|-------------------|--------|
| Struct/enum/constant definitions | ✅ | ❌ | Port Rust→Brief |
| Standalone definitions (functions) | ✅ | ❌ | Port Rust→Brief |
| Inline asm (core::arch::asm!) | ✅ | ❌ | Port Rust→Brief |
| Local triggers | ✅ | ❌ | Port Rust→Brief |
| Alka blocks | ✅ | ❌ | Port Rust→Brief |
| `#on_exit` cleanup | ✅ | ❌ | Port Rust→Brief |
| Escape/term statement handling | ✅ | ❌ | Port Rust→Brief |
| Type mappings (HashMap, Result, Option, Queue, Stack, HashSet, tuples, generics) | ✅ | ❌ | Port Rust→Brief |
| Vector element-wise ops | ✅ | ❌ | Port Rust→Brief |
| Field access / struct instances | ✅ | ❌ | Port Rust→Brief |
| Pattern match / block expressions | ✅ | ❌ | Port Rust→Brief |
| Tuple destructure | ✅ | ❌ | Port Rust→Brief |
| Slice/multislice | ✅ | ❌ | Port Rust→Brief |
| ForAll/Exists | ✅ | ❌ | Port Rust→Brief |
| Object literals | ✅ | ❌ | Port Rust→Brief |
| Default impl generation | ✅ | ❌ | Port Rust→Brief |
| `main()` entry point | ✅ | ❌ | Port Rust→Brief |
| State struct generation | ✅ | ✅ | (already synced) |
| Basic let/assignment | ✅ | ✅ | (already synced) |

### 5.3 Verilog Backend: Brief → Rust → Brief

| Feature | Rust (1,805 lines) | Brief (488 lines) | Action |
|---------|-------------------|-------------------|--------|
| AXI4-Lite state machine (full handshake FSM) | ✅ | ❌ | Port Rust→Brief |
| Clock divider per reactor speed | ✅ | ❌ | Port Rust→Brief |
| IO/memory mapping with hex-address lookup | ✅ | ❌ | Port Rust→Brief |
| Union type signals (data/err/tag) | ✅ | ❌ | Port Rust→Brief |
| Tuple signals | ✅ | ❌ | Port Rust→Brief |
| BRAM/UltraRAM inference with ram_style attributes | ✅ | ❌ | Port Rust→Brief |
| Vector generate-for loops | ✅ | ❌ | Port Rust→Brief |
| RAM priority encoder multiplexer | ✅ | ❌ | Port Rust→Brief |
| Timeout/watchdog per variable | ✅ | ❌ | Port Rust→Brief |
| Hardware validation (size/bits) | ✅ | ❌ | Port Rust→Brief |
| Testbench generation with VCD | ✅ | ❌ | Port Rust→Brief |
| `generate_with_axi` auto-detection | ✅ | ❌ | Port Rust→Brief |
| Regex-based vector lifting | ✅ | ❌ | Port Rust→Brief |
| Module / always process / FSM structure | ✅ | ✅ | (already synced) |
| Guard compilation | ✅ | ✅ | (already synced) |
| Memory inference | ✅ | ✅ | (already synced) |
| PSL/SVA assertions | ✅ | ✅ | (already synced) |
| Module structure / instantiation | ✅ | ✅ | (already synced) |
| Optimization (pipelining/retiming) | ❌ | ✅ | Port Brief→Rust |

### 5.4 VHDL Backend: Brief → Rust → Brief

| Feature | Rust (1,042 lines) | Brief (443 lines) | Action |
|---------|-------------------|-------------------|--------|
| Multi-file output (package/top/AXI/clk_div/RAM/FSM/txn/testbench) | ✅ | ❌ | Port Rust→Brief |
| AXI4-Lite slave bridge (full state machine) | ✅ | ❌ | Port Rust→Brief |
| Clock divider component | ✅ | ❌ | Port Rust→Brief |
| RAM inference with attributes | ✅ | ❌ | Port Rust→Brief |
| FSM (type/register/next-state/output) | ✅ | ❌ | Port Rust→Brief |
| PSL assertion comments | ✅ | ❌ | Port Rust→Brief |
| Testbench with clock/reset stimulus | ✅ | ❌ | Port Rust→Brief |
| Type width calculation | ✅ | ❌ | Port Rust→Brief |
| `get_pragma` attribute system | ✅ | ❌ | Port Rust→Brief |
| `is_ram_state` detection | ✅ | ❌ | Port Rust→Brief |
| Entity/architecture / process conversion | ✅ | ✅ | (already synced) |
| Component instantiation | ✅ | ✅ | (already synced) |
| Timing constraints | ❌ | ✅ | Port Brief→Rust |
| Optimization (resource sharing/pipelining/guard merging) | ❌ | ✅ | Port Brief→Rust |

### 5.5 x86_64 Backend: Brief ↔ Rust

| Feature | Rust (598 lines) | Brief (523 lines) | Action |
|---------|-----------------|-------------------|--------|
| Binary encoding (instruction tables, label resolution) | ❌ | ✅ | Port Brief→Rust |
| Register allocator | ❌ | ✅ | Port Brief→Rust |
| Memory layout pass | ❌ | ✅ | Port Brief→Rust |
| Two-pass encoding | ❌ | ✅ | Port Brief→Rust |
| Entry point / Linux syscall exit | ✅ | ❌ | Port Rust→Brief |
| Sequential + parallel reactor | ✅ | ❌ | Port Rust→Brief |
| Branchless guard (CMOV/SETcc) | ✅ | ❌ | Port Rust→Brief |
| Predictive fetch (PREFETCHT0) | ✅ | ❌ | Port Rust→Brief |
| Multi-expr generation | ✅ | ❌ | Port Rust→Brief |
| `collect_data_addresses` for prefetch | ✅ | ❌ | Port Rust→Brief |
| Transaction push/pop frame | ✅ | ✅ | (already synced) |

---

## Testing Strategy

Each phase must pass these gates before moving to the next:

| Gate | Command | Expectation |
|------|---------|-------------|
| Rust builds | `cargo build` | Exit 0 |
| Rust tests pass | `cargo test --lib` | All tests pass (currently 215+) |
| Backend registry tests | `cargo test --lib -- backend::tests` | Backend registry + hashtag validation tests pass |
| Brief compiler builds | `brief build lib/compiler/main.bv` | Brief self-hosted compiler compiles successfully |
| CLI help | `brief help` | Shows all commands including new ones |
| WASM output | `brief wasm file.bv` | Produces valid `.wasm` binary |
| Webstack output | `brief webstack file.rbv` | Produces `.rs` + `.js` + HTML output |

---

## File Change Summary

| Phase | Compiler | Files Created | Files Modified | Total |
|-------|----------|---------------|----------------|-------|
| 1 (WASM split) | Rust | 1 (`wasm.rs`) | 3 (`webstack.rs`, `mod.rs`, `main.rs`) | 4 |
| 1 (WASM split) | Brief | 1 (`webstack.bv`) | 2 (`wasm.bv`, `main.bv`) | 3 |
| 2 (v0.14 features) | Brief | 1 (`backends/mod.bv`) | 6 (`token.bv`, `lexer.bv`, `ast.bv`, `parser.bv`, `typechecker.bv`, `proof_engine.bv`) | 7 |
| 3 (AArch64) | Rust | 0 | 1 (`aarch64.rs`) | 1 |
| 4 (missing) | Brief | 2 (`cobol.bv`, `tcl_generator.bv`) | 1 (`main.bv`) | 3 |
| 5 (sync) | Both | 0 | ~10 backend files + main.rs/main.bv | ~12 |
| **Total** | | **4 new files** | **~23 files** | **~27** |

---

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| WASM binary backend scope underestimated | Medium | Start with minimal viable subset (i32 ops, basic control flow). Iteratively add features |
| Brief self-hosted compiler can't parse its own expanded AST | Low | Rust compiler is the bootstrap — can always compile the Brief compiler with itself after each change |
| Brief proof_engine.bv too small for SIMD checks | Low | SIMD length checking is a standalone function, minimal integration with existing symbolic engine |
| COBOL backend in Brief is a novelty with little practical use | Medium | Port the Rust implementation directly. Keep minimal. Don't over-engineer |
| Breaking changes to existing CLI behavior | Medium | Never remove or change existing commands. Only add new ones. Keep `wasm` as direct binary; `webstack` is new |
| Phase 2 touches 7+ files with ~14 features — merge conflicts | Medium | Implement one feature at a time, test after each, commit between phases |

---

## Appendix: Current File Inventory

### Rust Compiler (`src/`)

```
src/
├── main.rs                  (3,751 lines — CLI dispatch, 17+ commands)
├── parser.rs                (5,480 lines — production parser)
├── ast.rs                   (911 lines — ~50 type definitions)
├── typechecker.rs           (1,663 lines — Hindley-Milner + domain checks)
├── proof_engine.rs          (2,701 lines — symbolic execution, 14 check fns)
├── lexer.rs                 (474 lines)
├── backend/
│   ├── mod.rs               (227 lines — backend registry + hashtag validation)
│   ├── wasm.rs              (2,022 lines — Webstack target, to be renamed)
│   ├── verilog.rs           (1,805 lines)
│   ├── vhdl.rs              (1,042 lines)
│   ├── c.rs                 (908 lines)
│   ├── rust.rs              (783 lines)
│   ├── cobol.rs             (709 lines)
│   ├── x86_64.rs            (598 lines)
│   ├── aarch64.rs           (561 lines — stub, Brief version is authoritative)
│   ├── tcl_generator.rs     (369 lines)
```

### Brief Self-Hosted (`lib/compiler/`)

```
lib/compiler/
├── main.bv                  (142 lines — stub, 1 command)
├── parser.bv                (1,770 lines — core language only)
├── ast.bv                   (350 lines — ~25 type definitions)
├── typechecker.bv           (681 lines — Hindley-Milner only)
├── proof_engine.bv          (810 lines — symbolic execution, 14 fns)
├── lexer.bv                 (489 lines)
├── token.bv                 (345 lines)
├── backends/
│   ├── backend_aarch64.bv   (1,654 lines — authoritative implementation)
│   ├── x86_64.bv            (523 lines — binary encoding)
│   ├── verilog.bv           (488 lines)
│   ├── vhdl.bv              (443 lines)
│   ├── wasm.bv              (351 lines — direct WASM spec, to be fleshed out)
│   ├── rust.bv              (219 lines — stub)
│   ├── c.bv                 (205 lines — stub)
│   ├── (webstack.bv)        — NEW
│   ├── (cobol.bv)           — NEW
│   ├── (tcl_generator.bv)   — NEW
│   ├── (mod.bv)             — NEW
```

---

## Progress Log

### 2026-05-25

#### Phase 1: WASM/Webstack Split — COMPLETED

**Rust compiler:**
- `src/backend/wasm.rs` → renamed to `webstack.rs`; all types/names renamed
- `src/backend/wasm.rs` — new direct WASM binary generator (LEB128, sections, expression compilation)
- `src/backend/mod.rs` — both backends registered
- `src/main.rs` — `run_wasm`/`run_webstack` split, `webstack` CLI command added
- `cargo build && cargo test --lib` — clean build, 215/215 pass

**Brief self-hosted:**
- `lib/compiler/backends/webstack.bv` — new file (signal collection, reactive engine, JS glue)
- `lib/compiler/main.bv` — updated with `--wasm`, `--webstack`, `--rust` backend dispatch

#### Phase 2.1: Hashtag Modifiers — COMPLETED

**Files changed:**
- `lib/compiler/token.bv` — added `HashBang`, `HashBracket`, `HashParen`, `HashQuestion` tokens
- `lib/compiler/lexer.bv` — `#!`, `#[`, `#(`, `#?` lexing with `#` fallback to comment
- `lib/compiler/ast.bv` — new `Hashtag` struct; `modifiers` field on `StmtLet`, `StmtAssign`, `StmtTerm`, `Definition`, `Transaction`, `StructDefinition`
- `lib/compiler/parser.bv` — `parse_hashtag_modifiers()` function, wired into all parse sites
- `lib/compiler/typechecker.bv` — updated all pattern matches for new field arities
- `lib/compiler/proof_engine.bv` — updated all pattern matches
- `lib/compiler/backends/c.bv`, `rust.bv`, `backend_aarch64.bv` — updated all pattern matches

#### Phase 2.2: `#on_exit` Block Pragma — COMPLETED

**Files changed:**
- `lib/compiler/token.bv` — added `Hash` token for `#` followed by identifier
- `lib/compiler/lexer.bv` — `#` + letter → `Hash` token (not comment)
- `lib/compiler/ast.bv` — added `StmtOnExit(List<Statement>)` to Statement enum
- `lib/compiler/parser.bv` — parse `#identifier { body };` as block pragma, produce `StmtOnExit`
- `lib/compiler/typechecker.bv` — typecheck OnExit body via `check_block`
- `lib/compiler/proof_engine.bv` — `execute_statement_symbolic`, `collect_writes`, `collect_reads` handle `StmtOnExit`
- `lib/compiler/backends/c.bv` — emit body as inline code with `/* #on_exit cleanup */` comment
- `lib/compiler/backends/rust.bv` — emit body as inline code with `// #on_exit cleanup` comment
- `lib/compiler/backends/backend_aarch64.bv` — `collect_vars_from_stmts` and `generate_statement` handle `StmtOnExit`

#### Phase 2.3: `+/-` Struct Field Modifiers — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — new `StructVariant` struct (contract, fields, additions, removals); `variants: List<StructVariant>` on `StructDefinition`
- `lib/compiler/parser.bv` — `parse_struct_variants()` / `parse_struct_variant()` / `parse_struct_variant_fields()` functions; wired into `parse_struct()` after base fields. Parses `[discriminant] { +field: Type, -field, field: Type }` syntax
- `lib/compiler/typechecker.bv` — passes `variants` through (no variant-aware validation, matching Rust's current behavior)

#### Phase 2.4: Alka Hatch — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — new `AlkaBlock` struct (dangerous, content); `StmtAlka(AlkaBlock)` in Statement enum
- `lib/compiler/parser.bv` — `token_to_string()` helper, `parse_alka_block()` function; alka/ALKA interception in `parse_statement` before expression parsing; parses `alka { ... };` and `alka! { ... };` with brace-depth tracking
- `lib/compiler/typechecker.bv` — opaque passthrough (no validation, matching Rust)
- `lib/compiler/proof_engine.bv` — passthrough in `execute_statement_symbolic` (returns state unchanged)
- `lib/compiler/backends/c.bv` — emits `/* alka: ... */` or `/* alka! ... */` comment
- `lib/compiler/backends/rust.bv` — emits `// alka {} = ...` or `// alka! {} = ...` comment
- `lib/compiler/backends/backend_aarch64.bv` — no-op in `collect_vars_from_stmts`; emits comment instruction in `generate_statement`

#### Future: Strict Mode Big-O / Complexity Extension (Planned)

See §2.8b above. Documented 2026-05-25. Implementation deferred until both compilers have strict mode ported.
