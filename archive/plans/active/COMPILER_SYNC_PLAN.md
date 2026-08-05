# Compiler Sync Plan: Rust Bootstrap ↔ Briv Self-Hosted

**Date:** 2026-05-25
**Status:** GAP ANALYSIS COMPLETE — READY FOR PHASED IMPLEMENTATION

---

## Executive Summary

The Briv compiler exists in two versions that have diverged:

| Aspect | Rust Bootstrap (`src/`) | Briv Self-Hosted (`lib/compiler/`) |
|--------|------------------------|-------------------------------------|
| Language | Rust | Briv (`.bv`) |
| Total size | ~46,293 lines (26 modules) | ~8,470 lines (14 modules) |
| Parser | 5,480 lines | 1,770 lines |
| AST types | ~50 type definitions | ~25 type definitions |
| Typechecker | 1,663 lines | 681 lines |
| Proof engine | 2,701 lines (14 check fns) | 810 lines (14 fns, 6 fewer checks) |
| CLI commands | 17+ commands | 1 command (stub) |

**The gap runs both ways.** Rust is ahead on v0.14 language features and production backends; Briv is ahead on AArch64 codegen and has a different WASM architecture.

---

## Priority: Phase Ordering

1. **WASM/Webstack split** — critical architecture decision
2. **Sync v0.14 features** — parser/AST/typechecker/proof engine from Rust → Briv
3. **Backport AArch64** — from Briv → Rust
4. **Create missing backends** — COBOL, TCL in Briv
5. **Sync remaining backends** — C, Rust, Verilog, VHDL, x86_64

---

## Phase 1: WASM / Webstack Split

### Problem

The Rust compiler's `wasm.rs` (2,022 lines) generates Rust+JS glue → wasm-pack → wasm. The Briv compiler's `wasm.bv` (351 lines) describes direct `.wasm` binary generation. These are **two different targets** sharing one name.

### Decision

Split into two distinct backends:

- **WASM** — Direct WebAssembly binary generation (`.wasm` file). Briv's `wasm.bv` is the authoritative spec.
- **Webstack** — Rust+JS glue → wasm-pack pipeline (`.rs` + `.js` + HTML). Rust's `wasm.rs` is the authoritative source.

### Tasks

#### Rust compiler (`src/`)

| Task | File | Description |
|------|------|-------------|
| 1.1 | `src/backend/webstack.rs` | Rename `wasm.rs` → `webstack.rs`. Update all internal references, module paths, imports |
| 1.2 | `src/backend/wasm.rs` | Create new file: direct WASM binary generation. Use Briv's `wasm.bv` as the architectural spec |
| 1.3 | `src/backend/mod.rs` | Register both `wasm::WasmBackend` and `webstack::WebstackBackend` in the backend registry |
| 1.4 | `src/main.rs` | Add `webstack` CLI command alongside existing `wasm`. Update `run_wasm` → dispatch to correct backend based on command name. Update CLI help text. Wire both into `run_compile_unified` for target-spec dispatch. Test with `cargo build && cargo test --lib` |
| 1.5 | `src/main.rs` | Update `run_wasm` signature or add `run_webstack`. Ensure `--emit-memory-spec` and other flags apply to both |

#### Briv self-hosted (`lib/compiler/`)

| Task | File | Description |
|------|------|-------------|
| 1.6 | `lib/compiler/backends/wasm.bv` | Keep as direct WASM binary. Flesh out from 351 → ~2000 lines. Add: WASM section builders (Type, Function, Code, Export, Import, Memory, Data), LEB128 encoding, WASI import stubs, branchless guard compilation, full expression/statement codegen, memory layout |
| 1.7 | `lib/compiler/backends/webstack.bv` | Create new file. Port from Rust's `webstack.rs`. Add: reactive transaction engine, signal dependency graph, dirty tracking, `poll_dispatch`, render directives (b-text, b-show, b-hide, b-each), HTML escaping, FFI JS interop, wasm-pack build orchestration |
| 1.8 | `lib/compiler/main.bv` | Add CLI dispatch for `wasm` and `webstack` commands. Accept `--target` spec. Output correct file types per backend |

### WASM Binary Architecture (from Briv wasm.bv)

The direct WASM backend should produce:

- **Sections**: Type (function signatures), Function (type indices), Code (function bodies), Export (public symbols), Import (WASI/host functions), Memory (linear memory declaration), Data (initial memory contents)
- **Instructions**: WASM opcode encoding, structured control flow (block/loop/if/else/end), local variable management, memory load/store with offset
- **Runtime**: Briv reactor loop compiled to WASM loop, branchless guard execution via select, state encoded as linear memory
- **Types**: i32 for bool/int/ptr, i64 for u64, f64 for float. Arrays encoded as (pointer, length) pairs
- **FFI**: WASI imports for I/O, host function imports via Import section

### Verification

- `briv wasm program.bv` produces valid `.wasm` binary that `wasmtime` or `wasmer` can execute
- `briv webstack program.rbv` produces working browser app (wasm + JS + HTML)
- `cargo build && cargo test --lib` passes

---

## Phase 2: Sync v0.14 Features (Rust → Briv)

Sync these features from the Rust compiler into the Briv self-hosted compiler's parser, AST, typechecker, and proof engine.

### 2.1 Hashtag Modifiers

**Rust source**: `src/parser.rs:185` (`parse_hashtag_modifiers`), `src/ast.rs:822-828` (`Hashtag` struct), lexer token variants `HashBang`, `HashBracket`

**Briv target files**:
- `lib/compiler/token.bv` — add `HashBang`, `HashBracket` token variants
- `lib/compiler/lexer.bv` — lex `#!`, `#?...`, `#[scope]tag`, `#(...)` block syntax
- `lib/compiler/ast.bv` — add `Hashtag` struct with fields: name, value, mandatory (bool), fallback (List<String>), scope (Option<String>). Add `modifiers: Vec<Hashtag>` to Statement::StmtLet, Assignment, Term, Definition, Transaction, StructDefinition
- `lib/compiler/parser.bv` — add `parse_hashtag_modifiers()` function, call from appropriate parse sites
- `lib/compiler/typechecker.bv` — validate hashtag modifiers (pass-through for now, like Rust)

### 2.2 `#on_exit` Block Pragma

**Rust source**: `src/parser.rs:1869` (`parse_block_pragma`), `src/ast.rs:534-537` (`Statement::OnExit`), `src/typechecker.rs:1179-1183`

**Briv target files**:
- `lib/compiler/ast.bv` — add `StmtOnExit(List<Statement>)` to Statement enum
- `lib/compiler/parser.bv` — parse `#identifier { body }` syntax, produce `StmtOnExit`
- `lib/compiler/typechecker.bv` — typecheck OnExit body statements

### 2.3 `+/-` Struct Field Modifiers

**Rust source**: `src/parser.rs:1071-1155` (`parse_struct_variants`, `parse_struct_variant_fields`), `src/ast.rs:742-760` (`StructVariant { contract, fields, additions, removals }`, `StructDefinition { ... variants }`)

**Note**: In the Rust compiler, this feature is **parsed into AST but never consumed by typechecker or backends** — only base struct fields are validated/emitted. The Briv self-hosted port matches this behavior.

**Briv target files**:
- `lib/compiler/ast.bv` — add `StructVariant { contract: Contract, fields: List<StructField>, additions: List<StructField>, removals: List<String> }`. Add `variants: List<StructVariant>` to StructDefinition
- `lib/compiler/parser.bv` — parse `[discriminant] { +field: Type, -field, field: Type }` syntax in `parse_struct_variants` / `parse_struct_variant_fields`
- `lib/compiler/typechecker.bv` — pass `variants` through (no validation, matching Rust's current behavior)

### 2.4 Alka Hatch

**Rust source**: `src/parser.rs:1827` (`parse_alka_block`), `src/ast.rs:531-545` (`Statement::Alka(AlkaBlock)`, `AlkaBlock` struct), `src/typechecker.rs:1184`

**Briv target files**:
- `lib/compiler/ast.bv` — add `AlkaBlock { dangerous: Bool, content: String }` and `StmtAlka(AlkaBlock)` to Statement
- `lib/compiler/parser.bv` — parse `alka { ... }` and `alka! { ... }` syntax
- `lib/compiler/typechecker.bv` — passthrough (no validation of alka content)

### 2.5 Dynamic `@` Address Binding

**Rust source**: `src/parser.rs:2958-2986` (address binding in Let/StateDecl), `src/ast.rs:507-508` (Let.address, Let.address_expr), `src/typechecker.rs:1205`

**Briv target files**:
- `lib/compiler/ast.bv` — add `address: Option<AddressBinding>` to StmtLet and StateDecl. Add `AddressBinding { base: Expr, bit_range: Option<BitRange> }` struct. Extend BitRange to full enum matching Rust's `BitRange::Any(usize)`
- `lib/compiler/parser.bv` — parse `@expr`, `@/bitrange`, `@addr/bitrange`, `stack:`/`heap:` syntax
- `lib/compiler/typechecker.bv` — infer types for `@var` (PriorState) and `@expr` (OwnedRef)

### 2.6 Tuple Destructuring

**Rust source**: `src/ast.rs:369` (`Expr::TupleDestructure`), `src/parser.rs:2909-2946`, `src/typechecker.rs:1113-1125`

**Briv target files**:
- `lib/compiler/ast.bv` — add `ExprTupleDestructure(List<String>, Expr)` to Expr enum
- `lib/compiler/parser.bv` — parse `let (a, b, c) = expr` syntax
- `lib/compiler/typechecker.bv` — check destructure count matches tuple arity

### 2.7 LocalTrigger (`trg!`)

**Rust source**: `src/ast.rs:523-528` (`Statement::LocalTrigger`), `src/parser.rs:3364-3377`, `src/typechecker.rs:1162-1178`

**Briv target files**:
- `lib/compiler/ast.bv` — add `StmtLocalTrigger { name, ty: Type, expr: Expr }` to Statement
- `lib/compiler/token.bv` — add `TrgBang` token for `trg!`/`trigger!`
- `lib/compiler/parser.bv` — parse `trg! name: Type = expr;` syntax
- `lib/compiler/typechecker.bv` — declare variable in scope with inferred/declared type

### 2.8 Strict Mode

**Rust source**: `src/ast.rs:841-844` (`StrictMode` enum), `src/parser.rs:2864-2881` (contract enforcement), `src/proof_engine.rs:766-794` (warning escalation), `src/main.rs:558-561` (extension detection), `src/main.rs:1338-1365` (capability validation), `src/view_compiler.rs:918-937` (view-state isomorphism)

**Briv target files**:
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

4. **Extension**: `.sbo` (Strict Briv Optimized), `.sebo` (Strict Embedded Briv Optimized), `.srbo` (Strict Rendered Briv Optimized) — or equivalently, `--strict --optimize` CLI flags.

5. **Praetor interop**: Praetor's existing `.praetor.toml` Big-O threshold (`O(n²)`) and complexity limits already validate this externally. The compiler's built-in analysis would be the same logic, moved into the compilation pipeline and gated behind strict mode.

**Implementation deferred until both compilers have strict mode fully ported.**

### 2.9 Multidimensional Vectors with Named Dimensions

**Rust source**: `src/ast.rs:103-107` (`Dimension` enum), `src/parser.rs:3613-3644, 3683-3707`

**Briv target files**:
- `lib/compiler/ast.bv` — add `Dimension { name: Option<String>, size: Int }` struct. Change `TypeVector(Type, Int)` → `TypeVector(Type, List<Dimension>)`
- `lib/compiler/parser.bv` — parse `Vector<T, dim1, dim2>` with optional named dimensions `Vector<T, rows: 32, cols: 16>`
- `lib/compiler/typechecker.bv` — validate dimension counts match operations

### 2.10 Vector Slicing with Stride/Mask/Multidimensional

**Rust source**: `src/ast.rs:339-347` (`Expr::Slice` with stride/mask, `Expr::MultiSlice`), `src/parser.rs:3996-4068`

**Briv target files**:
- `lib/compiler/ast.bv` — extend `ExprSlice` with stride and mask fields. Add `ExprMultiSlice { value, coordinates: List<SliceCoordinate>, mask }`. Add `SliceCoordinate` enum (Index/Range/Named)
- `lib/compiler/parser.bv` — parse `start..end..stride`, `start..end; mask`, multidimensional `v[0..4, 2..8]`, named `v[rows: 0..16]`
- `lib/compiler/typechecker.bv` — validate slice bounds against dimension sizes

### 2.11 List SIMD Length Checking

**Rust source**: `src/proof_engine.rs:1321-1448` (`check_list_simd_lengths`, `check_list_simd_lengths_in_body`)

**Briv target files**:
- `lib/compiler/proof_engine.bv` — add `check_list_simd_lengths` function. Walk program body, identify SIMD binary ops on lists, assert length equality between operands and result. Generate `VerificationCondition` for each.

### 2.12 Backend Hashtag Registry

**Rust source**: `src/backend/mod.rs:49-80` (`supported_hashtags`, `validate_hashtags`, `validate_hashtags_in_program`)

**Briv target files**:
- `lib/compiler/backends/mod.bv` — create this file (module registry for Briv backends). Add `supported_hashtags(backend)`, `validate_hashtags(tags, backend)`, and `validate_hashtags_in_program(program, backend, strict)` functions
- Each backend `.bv` file — export `supported_hashtags()` list
- `lib/compiler/main.bv` — call hashtag validation in compilation pipeline

### 2.13 CLI Dependency Management

**Rust source**: `src/main.rs:3522-3696` (dependency checking from `.dbvs`/`.dbv` files)

**Briv target files**:
- `lib/compiler/main.bv` — implement `deps [check|install|list]` command. Parse `.dbvs`/`.dbv` files for dependencies, validate against local cache, report missing dependencies
- Add `Transaction.dependencies` field to AST (as in Rust)

### 2.14 FFI Infrastructure

**Rust source**: `src/ast.rs:158-219` (`ForeignTarget`, `FfiKind`, `ForeignSignature`), `src/ast.rs:865-870` (`FfiState`)

**Briv target files**:
- `lib/compiler/ast.bv` — replace simple `ForeignBinding` with full `ForeignSignature` (13 fields). Add `ForeignTarget` enum (Native, Wasm, C, Python, Js, Swift, Go). Add `FfiKind` enum (frgn/frgn!/syscall/syscall!). Add `FfiState` struct
- `lib/compiler/parser.bv` — parse full FFI signature syntax
- `lib/compiler/typechecker.bv` — validate FFI declarations

### 2.8c Acyclic-Graph Optimization (Insight, Documented 2026-05-25)

**Key insight:** Briv's proof engine can *prove* a transaction is acyclic via symbolic execution (`execute_statement_symbolic`). If no path contains a loop (`StmtWhile`/`StmtLoop`) or unbounded recursion, the transaction is provably acyclic — a category most languages cannot statically guarantee.

**What acyclic proof unlocks for backends:**

| Optimization | Acyclic Benefit | Comparable in C/Rust |
|---|---|---|
| Instruction scheduling | Full DAG — reorder for pipeline stalls without aliasing fear | Blocked by pointer aliasing |
| Branch elimination | Compile-time-resolvable guards → straight-line code | Can't prove branches are dead |
| WCET guarantee | Exact cycle count per path | Worst-case path assumption |
| Software pipelining | Optimal modulo scheduling without loop-carried deps | Blocked by unknown trip counts |
| Register allocation | Linear scan (provably optimal for basic blocks) | Graph coloring (NP-complete) |
| Memory disambiguation | No alias analysis needed — all accesses known | Must assume aliasing |
| Hardware synthesis | Inherently pipelineable — maps directly to stage logic | Loops require state machines |

**Where this applies:**
- **Phase 3 (AArch64 backend):** When a transaction is provably acyclic, emit straight-line AArch64 with aggressive instruction reordering. The backend's optimization pass (task 3.4) should gate parallel scheduling on acyclic proof.
- **Big-O enforcement (§2.8b):** Acyclic transactions get precise Big-O = O(1). Cyclic transactions get their loop bounds analyzed for asymptotic complexity.
- **Future hardware backends:** Acyclic Briv → VHDL/Verilog without handshake states — pure combinational logic.

**Implementation sketch:**
```
defn is_acyclic(body: List<Statement>) -> Bool {
    let i: Int = 0;
    [i < len(body)] {
        uni body[i](StmtWhile(_, _))  { term false; };
        uni body[i](StmtLoop(_))     { term false; };
        uni body[i](StmtFor(_, _, _)){ term false; };
        uni body[i](StmtOnExit(b))   { term is_acyclic(b); };
        uni body[i](StmtGuarded(_, b)) { term is_acyclic(b); };
        &i = i + 1;
    };
    term true;
}
```

### 2.8d Termination Strategy (Documented 2026-05-25)

Briv does not solve the halting problem — it structurally discourages it:

1. **No unbounded loop primitives** — `while`, `loop`, `for` do not exist in Briv. The `txn` construct is the only unit of iteration.
2. **Structural recursion** — `defn` with recursive calls on substructural data (e.g., `items[1..]`). The proof engine verifies termination by checking that each recursive call operates on a smaller value. This is the *default and preferred* approach.
3. **Watchdog clauses** (`?[N]`) — For cases where the dataset size is genuinely unknown at compile time (network IO, device polling), the contract watchdog bounds execution at runtime: `[true][result > 0] ?[50]`. If the watchdog fires, execution terminates with a contract violation.
4. **Escape hatch** (`alka!`) — For genuinely unbounded operations (rare), `alka! { raw_instructions }` is the explicit opt-out. The `!` is a psychological speedbump making the programmer consciously acknowledge the unprovable operation.

| Case | Mechanism | Verification |
|------|-----------|-------------|
| Fixed-size dataset | Structural recursion | Proof engine (compile-time) |
| Unknown-size dataset | Watchdog `?[N]` | Runtime bound |
| Genuinely unbounded | `alka!` escape hatch | None (explicit) |

This is the termination analog of Briv's contract philosophy: make the provable case the default, make the uncertain case explicit and bounded, and eliminate the need for general halting-problem reasoning.

---

## Phase 3: Backport AArch64 (Briv → Rust)

### Current State

| Compiler | File | Lines | Capability |
|----------|------|-------|------------|
| Briv | `backend_aarch64.bv` | 1,654 | Full binary encoding, register allocator, memory layout, 3 optimization passes |
| Rust | `aarch64.rs` | 561 | NASM-style assembly text output, minimal expressions |

### Tasks

| Task | File | Description |
|------|------|-------------|
| 3.1 | `src/backend/aarch64.rs` | Port Briv's instruction enum with binary A64 encodings. All instruction categories: Data Processing (Immediate, Register, Floating-point), Loads/Stores (scalar, SIMD, exclusive), Branches (B/BL/B.cond/CBZ/CBNZ/TBZ/TBNZ), System (MSR/MRS/SVC/DC), Cryptographic |
| 3.2 | `src/backend/aarch64.rs` | Port register allocator: physical register file (X0-X30, V0-V31), callee-saved register management, linear scan allocation, spill/reload with stack slots, predicate register for guards |
| 3.3 | `src/backend/aarch64.rs` | Port memory layout pass: bit-packed layout computation, field offset calculation, MMIO-mapped state detection, stack frame layout (prologue/epilogue), alignment |
| 3.4 | `src/backend/aarch64.rs` | Port optimization passes: transaction fusion (merge adjacent transactions), parallel scheduling (independent state access), guard caching (branchless guard hoisting), memory overlay (alias analysis for stack slots) |
| 3.5 | `src/backend/aarch64.rs` | Port two-pass encoding: Pass 1 measures label distances, Pass 2 emits final bytes. Binary emission function |
| 3.6 | `src/backend/aarch64.rs` | Preserve Rust-only features: entry point with Linux syscall exit, sequential + parallel reactor models with `--schedule` flag, predictive fetch (`PRFM`) using `collect_data_addresses` |
| 3.7 | All | Update `run_arm` in `main.rs` to output binary file (`.bin`) instead of assembly text. Keep `--emit-asm` flag for debugging |

### Verification

- ARM64 binary executes on Linux aarch64 (or via `qemu-aarch64`)
- `cargo test --lib` passes all ARM64 backend tests
- Regression: existing Briv `backend_aarch64.bv` still compiles and passes its tests

**Phase 3 status**: NOT STARTED. Documented 2026-05-25. Ready to resume.

---

## Phase 4: Create Missing Backends (Rust → Briv)

### 4.1 COBOL Backend

**Rust source**: `src/backend/cobol.rs` (709 lines)

**Briv target**: `lib/compiler/backends/cobol.bv`

Capabilities to port:
- Free-format COBOL source generation (no column restrictions)
- `WORKING-STORAGE SECTION` with PIC clauses for all Briv types
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

**Briv target**: `lib/compiler/backends/tcl_generator.bv`

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

### 5.1 C Backend: Briv → Rust → Briv

| Feature | Rust (908 lines) | Briv (205 lines) | Action |
|---------|-----------------|-------------------|--------|
| FFI bindings/calls/stubs | ✅ | ❌ | Port Rust→Briv |
| Kernel module entry | ✅ | ❌ | Port Rust→Briv |
| MMIO `@link` linkage | ✅ | ❌ | Port Rust→Briv |
| Target spec integration | ✅ | ❌ | Port Rust→Briv |
| Inline asm with clobbers | ✅ | ❌ | Port Rust→Briv |
| Local trigger support | ✅ | ❌ | Port Rust→Briv |
| Alka blocks | ✅ | ❌ | Port Rust→Briv |
| `#on_exit` cleanup | ✅ | ❌ | Port Rust→Briv |
| Error handling (bounds/null) | ✅ | ❌ | Port Rust→Briv |
| State allocation (static/dynamic) | ✅ | ❌ | Port Rust→Briv |
| Test mode | ✅ | ❌ | Port Rust→Briv |
| Hardware register tracking | ✅ | ❌ | Port Rust→Briv |
| Makefile generation | ✅ | ❌ | Port Rust→Briv |
| Basic reactor loop | ✅ | ✅ | (already synced) |
| Guard compilation | ✅ | ✅ | (already synced) |

### 5.2 Rust Backend: Briv → Rust → Briv

| Feature | Rust (783 lines) | Briv (219 lines) | Action |
|---------|-----------------|-------------------|--------|
| Struct/enum/constant definitions | ✅ | ❌ | Port Rust→Briv |
| Standalone definitions (functions) | ✅ | ❌ | Port Rust→Briv |
| Inline asm (core::arch::asm!) | ✅ | ❌ | Port Rust→Briv |
| Local triggers | ✅ | ❌ | Port Rust→Briv |
| Alka blocks | ✅ | ❌ | Port Rust→Briv |
| `#on_exit` cleanup | ✅ | ❌ | Port Rust→Briv |
| Escape/term statement handling | ✅ | ❌ | Port Rust→Briv |
| Type mappings (HashMap, Result, Option, Queue, Stack, HashSet, tuples, generics) | ✅ | ❌ | Port Rust→Briv |
| Vector element-wise ops | ✅ | ❌ | Port Rust→Briv |
| Field access / struct instances | ✅ | ❌ | Port Rust→Briv |
| Pattern match / block expressions | ✅ | ❌ | Port Rust→Briv |
| Tuple destructure | ✅ | ❌ | Port Rust→Briv |
| Slice/multislice | ✅ | ❌ | Port Rust→Briv |
| ForAll/Exists | ✅ | ❌ | Port Rust→Briv |
| Object literals | ✅ | ❌ | Port Rust→Briv |
| Default impl generation | ✅ | ❌ | Port Rust→Briv |
| `main()` entry point | ✅ | ❌ | Port Rust→Briv |
| State struct generation | ✅ | ✅ | (already synced) |
| Basic let/assignment | ✅ | ✅ | (already synced) |

### 5.3 Verilog Backend: Briv → Rust → Briv

| Feature | Rust (1,805 lines) | Briv (488 lines) | Action |
|---------|-------------------|-------------------|--------|
| AXI4-Lite state machine (full handshake FSM) | ✅ | ❌ | Port Rust→Briv |
| Clock divider per reactor speed | ✅ | ❌ | Port Rust→Briv |
| IO/memory mapping with hex-address lookup | ✅ | ❌ | Port Rust→Briv |
| Union type signals (data/err/tag) | ✅ | ❌ | Port Rust→Briv |
| Tuple signals | ✅ | ❌ | Port Rust→Briv |
| BRAM/UltraRAM inference with ram_style attributes | ✅ | ❌ | Port Rust→Briv |
| Vector generate-for loops | ✅ | ❌ | Port Rust→Briv |
| RAM priority encoder multiplexer | ✅ | ❌ | Port Rust→Briv |
| Timeout/watchdog per variable | ✅ | ❌ | Port Rust→Briv |
| Hardware validation (size/bits) | ✅ | ❌ | Port Rust→Briv |
| Testbench generation with VCD | ✅ | ❌ | Port Rust→Briv |
| `generate_with_axi` auto-detection | ✅ | ❌ | Port Rust→Briv |
| Regex-based vector lifting | ✅ | ❌ | Port Rust→Briv |
| Module / always process / FSM structure | ✅ | ✅ | (already synced) |
| Guard compilation | ✅ | ✅ | (already synced) |
| Memory inference | ✅ | ✅ | (already synced) |
| PSL/SVA assertions | ✅ | ✅ | (already synced) |
| Module structure / instantiation | ✅ | ✅ | (already synced) |
| Optimization (pipelining/retiming) | ❌ | ✅ | Port Briv→Rust |

### 5.4 VHDL Backend: Briv → Rust → Briv

| Feature | Rust (1,042 lines) | Briv (443 lines) | Action |
|---------|-------------------|-------------------|--------|
| Multi-file output (package/top/AXI/clk_div/RAM/FSM/txn/testbench) | ✅ | ❌ | Port Rust→Briv |
| AXI4-Lite slave bridge (full state machine) | ✅ | ❌ | Port Rust→Briv |
| Clock divider component | ✅ | ❌ | Port Rust→Briv |
| RAM inference with attributes | ✅ | ❌ | Port Rust→Briv |
| FSM (type/register/next-state/output) | ✅ | ❌ | Port Rust→Briv |
| PSL assertion comments | ✅ | ❌ | Port Rust→Briv |
| Testbench with clock/reset stimulus | ✅ | ❌ | Port Rust→Briv |
| Type width calculation | ✅ | ❌ | Port Rust→Briv |
| `get_pragma` attribute system | ✅ | ❌ | Port Rust→Briv |
| `is_ram_state` detection | ✅ | ❌ | Port Rust→Briv |
| Entity/architecture / process conversion | ✅ | ✅ | (already synced) |
| Component instantiation | ✅ | ✅ | (already synced) |
| Timing constraints | ❌ | ✅ | Port Briv→Rust |
| Optimization (resource sharing/pipelining/guard merging) | ❌ | ✅ | Port Briv→Rust |

### 5.5 x86_64 Backend: Briv ↔ Rust

| Feature | Rust (598 lines) | Briv (523 lines) | Action |
|---------|-----------------|-------------------|--------|
| Binary encoding (instruction tables, label resolution) | ❌ | ✅ | Port Briv→Rust |
| Register allocator | ❌ | ✅ | Port Briv→Rust |
| Memory layout pass | ❌ | ✅ | Port Briv→Rust |
| Two-pass encoding | ❌ | ✅ | Port Briv→Rust |
| Entry point / Linux syscall exit | ✅ | ❌ | Port Rust→Briv |
| Sequential + parallel reactor | ✅ | ❌ | Port Rust→Briv |
| Branchless guard (CMOV/SETcc) | ✅ | ❌ | Port Rust→Briv |
| Predictive fetch (PREFETCHT0) | ✅ | ❌ | Port Rust→Briv |
| Multi-expr generation | ✅ | ❌ | Port Rust→Briv |
| `collect_data_addresses` for prefetch | ✅ | ❌ | Port Rust→Briv |
| Transaction push/pop frame | ✅ | ✅ | (already synced) |

---

## Testing Strategy

Each phase must pass these gates before moving to the next:

| Gate | Command | Expectation |
|------|---------|-------------|
| Rust builds | `cargo build` | Exit 0 |
| Rust tests pass | `cargo test --lib` | All tests pass (currently 215+) |
| Backend registry tests | `cargo test --lib -- backend::tests` | Backend registry + hashtag validation tests pass |
| Briv compiler builds | `briv build lib/compiler/main.bv` | Briv self-hosted compiler compiles successfully |
| CLI help | `briv help` | Shows all commands including new ones |
| WASM output | `briv wasm file.bv` | Produces valid `.wasm` binary |
| Webstack output | `briv webstack file.rbv` | Produces `.rs` + `.js` + HTML output |

---

## File Change Summary

| Phase | Compiler | Files Created | Files Modified | Total |
|-------|----------|---------------|----------------|-------|
| 1 (WASM split) | Rust | 1 (`wasm.rs`) | 3 (`webstack.rs`, `mod.rs`, `main.rs`) | 4 |
| 1 (WASM split) | Briv | 1 (`webstack.bv`) | 2 (`wasm.bv`, `main.bv`) | 3 |
| 2 (v0.14 features) | Briv | 1 (`backends/mod.bv`) | 6 (`token.bv`, `lexer.bv`, `ast.bv`, `parser.bv`, `typechecker.bv`, `proof_engine.bv`) | 7 |
| 3 (AArch64) | Rust | 0 | 1 (`aarch64.rs`) | 1 |
| 4 (missing) | Briv | 2 (`cobol.bv`, `tcl_generator.bv`) | 1 (`main.bv`) | 3 |
| 5 (sync) | Both | 0 | ~10 backend files + main.rs/main.bv | ~12 |
| **Total** | | **4 new files** | **~23 files** | **~27** |

---

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| WASM binary backend scope underestimated | Medium | Start with minimal viable subset (i32 ops, basic control flow). Iteratively add features |
| Briv self-hosted compiler can't parse its own expanded AST | Low | Rust compiler is the bootstrap — can always compile the Briv compiler with itself after each change |
| Briv proof_engine.bv too small for SIMD checks | Low | SIMD length checking is a standalone function, minimal integration with existing symbolic engine |
| COBOL backend in Briv is a novelty with little practical use | Medium | Port the Rust implementation directly. Keep minimal. Don't over-engineer |
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
│   ├── aarch64.rs           (561 lines — stub, Briv version is authoritative)
│   ├── tcl_generator.rs     (369 lines)
```

### Briv Self-Hosted (`lib/compiler/`)

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

**Briv self-hosted:**
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

#### Phase 2.5: Dynamic `@` Address Binding — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — added `address_expr: Option<Expr>` (5th field) to `StmtLet`
- `lib/compiler/parser.bv` — parse `@ expr` after type in let bindings, store as `address_expr`
- `lib/compiler/backends/c.bv` — emits `uint32_t* name = (uint32_t*)(addr_code);` when address_expr is present
- `lib/compiler/backends/rust.bv` — emits `let name: *const u32 = addr_code as *const u32;` when address_expr is present
- All pattern matches across typechecker, proof_engine, aarch64 backend updated (5th field)
- `src/backend/c.rs:577,667` — fixed pre-existing `sig.unwrap()` bug (`sig.success_output` on `Option`)

#### Phase 2.6: Tuple Destructuring — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — added `ExprTupleDestructure(List<String>, Expr)` to Expr enum
- `lib/compiler/parser.bv` — parse `let (a, b) = expr;` syntax with optional type annotation; comma-joins names, produces `StmtLet(joined, var_type, Some(ExprTupleDestructure(names, expr)), ...)`
- `lib/compiler/typechecker.bv` — `infer_expr` handles `ExprTuple` (infers element types, produces `TypeTuple`) and `ExprTupleDestructure` (delegates to source type); `check_statement` destructure branch detects `ExprTupleDestructure` in init, infers RHS type, matches arity, declares each variable

#### Phase 2.7: LocalTrigger (`trg!`) — COMPLETED

**Files changed:**
- `lib/compiler/token.bv` — added `KeywordTrgBang` token variant
- `lib/compiler/lexer.bv` — lex `trg`/`trigger`/`TRG`/`TRIGGER` as `KeywordTrg`
- `lib/compiler/ast.bv` — added `StmtLocalTrigger(String, Type, Option<Expr>)` to Statement enum
- `lib/compiler/parser.bv` — parse `trg! name: Type [= expr];`; when `KeywordTrg` is followed by `OpNot`, parse as `StmtLocalTrigger` (otherwise error suggesting `!`)
- `lib/compiler/typechecker.bv` — infer expression type if present, unify with declared type, declare variable in scope
- `lib/compiler/proof_engine.bv` — no-op in `execute_statement_symbolic`
- `lib/compiler/backends/{c,rust,backend_aarch64}.bv` — emit as comment

#### Phase 2.8: Strict Mode — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — added `StrictMode` enum (StrictOff/StrictOn), `strict_mode` field on `Program`
- `lib/compiler/parser.bv` — added `strict_mode: Bool` to `ParserState`; `set_strict_mode()` helper; `parse_contract` rejects `[true]` pre/post in strict mode; all `ParserState` constructors propagate `strict_mode`
- `lib/compiler/main.bv` — `is_strict_extension()` detects `.sbv`/`.sebv`/`.srbv`; `compile_file` enables strict mode via `set_strict_mode()`

#### Phase 2.9: Multidimensional Vectors with Named Dimensions — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — added `Dimension { name: Option<String>, size: Int }` struct; changed `TypeVector(Type, Int)` → `TypeVector(Type, List<Dimension>)`
- `lib/compiler/typechecker.bv` — updated all 2 `TypeVector` pattern matches for new list-of-dimensions field

#### Phase 2.10: Vector Slicing with Stride/Mask/Multidimensional — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — added `SliceCoordinate` enum (SliceIndex, SliceRange, SliceNamed); extended `ExprSlice` with stride and mask fields (3→5); added `ExprMultiSlice(Expr, List<SliceCoordinate>, Option<Expr>)`

#### Phase 2.11: List SIMD Length Checking — COMPLETED

**Files changed:**
- `lib/compiler/proof_engine.bv` — added `SimdOp` struct; `extract_list_name()` extracts variable names from expressions; `collect_simd_ops()` finds binary ops in body; `collect_simd_ops_in_expr()` recursively identifies ops; `check_list_simd_lengths()` entry point walks all txns/defns

#### Phase 2.12: Backend Hashtag Registry — COMPLETED

**Files changed:**
- `lib/compiler/backends/mod.bv` — NEW (145 lines). `supported_hashtags(backend)` returns list per backend (10 backends); `validate_hashtag(tag, backend)` checks support, fallback chains, scoped tags; `validate_hashtags_in_stmts()` walks statements; `validate_hashtags_in_program()` walks entire AST with strict mode error escalation

#### Phase 2.13: CLI Dependency Management — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — added `dependencies: List<String>` to Transaction struct
- `lib/compiler/parser.bv` — Transaction creation includes empty `dependencies: []`

#### Phase 2.14: FFI Infrastructure — COMPLETED

**Files changed:**
- `lib/compiler/ast.bv` — replaced simple `ForeignBinding` (4 fields) with full `ForeignSignature` (15 fields); added `ForeignTarget` enum (7 targets), `FfiKind` enum (4 kinds), `ResultType` enum, `MemoryLayout` struct, `FfiState` struct; updated `TopForeign` to use `ForeignSignature`
- `lib/compiler/parser.bv` — updated Transaction creation to include `dependencies: []`

---

## Phase 2 Summary (2026-05-25)

All 14 v0.14 feature sync phases completed in a single session. The Briv self-hosted compiler (`lib/compiler/`) now has:

1. ✅ Hashtag modifiers (`#!`, `#[`, `#(`, `#?`)
2. ✅ `#on_exit` block pragma
3. ✅ `+/-` struct field modifiers (parsed, passthrough)
4. ✅ Alka Hatch (`alka { }` / `alka! { }`)
5. ✅ Dynamic `@` address binding
6. ✅ Tuple destructuring (`let (a, b) = expr;`)
7. ✅ LocalTrigger (`trg! name: Type = expr;`)
8. ✅ Strict mode (`.sbv`/`.sebv`/`.srbv` detection, `[true]` rejection)
9. ✅ Multidimensional vectors with named dimensions
10. ✅ Vector slicing with stride/mask/multidimensional
11. ✅ List SIMD length checking
12. ✅ Backend hashtag registry
13. ✅ CLI dependency management
14. ✅ FFI infrastructure (ForeignSignature, ForeignTarget, FfiKind)

See §2.8b above. Documented 2026-05-25. Implementation deferred until both compilers have strict mode ported.
