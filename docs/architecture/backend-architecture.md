# LLVM Backend Architecture Guide

**Audience:** AI coding agents and new contributors who need to make correct changes to the LLVM backend without violating the type system's architectural invariants.

**Status:** Living document — updated as the backend evolves.

---

## 1. Core Architecture

### 1.1 Three-Layer Lifetime Model

The backend state is strictly stratified into three lifetimes to prevent state leakage:

| Layer | Type | Scope | Mutability | Contents |
|-------|------|-------|------------|----------|
| **CompilerContext** | `context.rs` | Global (entire compilation) | Read-only during codegen | AST definitions, FFI signatures, target specs, field layouts, casting graph |
| **FunctionContext** | `context.rs` | Per-function/transaction | Mutable per-function | SSA registers (`gen_reg`), phi nodes, type caches, loop state |
| **LlvmBackend** | `mod.rs` | Orchestrator | Delegates to modules | `generate()`, dispatch, helper functions |

**Rules:**
- Never add transient, function-scoped compilation variables to `CompilerContext`.
- All registers must be requested via `self.fun.gen_reg()`. Manual register arithmetic is forbidden.
- `CompilerContext` is read-only during codegen. If you need mutable per-function state, add it to `FunctionContext`.

### 1.2 Module Map

| File | Purpose |
|------|---------|
| `mod.rs` | Entry point (`generate()`), field index assignment, dispatch, helper functions |
| `emit_toplevel.rs` | Top-level IR emission: `@init_state`, `@main`, struct type declarations, runtime declares |
| `emit_expr.rs` | Expression emission: arithmetic, casts, calls, loads, stores |
| `emit_stmt.rs` | Statement emission: assignments, let bindings, control flow |
| `helpers.rs` | Common helpers: `adapt_to_i64`, `load_field_type`, `store_field_type`, `llvm_type` |
| `context.rs` | `CompilerContext` and `FunctionContext` struct definitions |
| `normalizer.rs` | LLVM type resolution and struct layout computation |
| `intrinsics.rs` | Intrinsic call emission (`Sqrt#`, `Malloc#`, etc.) |
| `vector_phi.rs` | Vector phi group detection and emission |
| `loop_engine/` | Loop emission strategies (counter.rs, ssa.rs) |
| `tests.rs` | Backend unit tests |

### 1.3 Code Generation Flow

```
generate(items)
  │
  ├─ build_field_index(items)        — Assign state slot indices from let declarations
  │     Every let var → (field_index, field_type, brief_type)
  │     Synthetic fields (cycle_count, arena_ptr) appended after
  │
  ├─ declare_struct_types(&mut out)  — Emit %SmallString64, %StaticString, %String, %UTF8View
  │     Also emit universe-registered struct types (skipping hardcoded names to avoid duplicates)
  │
  ├─ emit_declares(&mut out)         — Declare @llvm.* intrinsics + runtime functions
  │
  ├─ emit_main_or_bootup(&mut out)   — Emit @main and/or __brief_init_state
  │     │
  │     ├─ emit_init_state()         — Function that writes initial values to %State
  │     │     Called from @main's entry block before the loop
  │     │
  │     └─ [Loop emission]           — One of:
  │           • emit_folded_main()     — Inline SSA (small states, dense writes)
  │           • emit_countable_main()  — Per-field phi nodes (default)
  │           • emit_countable_batched_main() — Outer/inner loop (guards detected)
  │
  └─ Metadata + function epilogue
```

## 2. The Golden Rule: Never Match Type Names

This is the most important rule in the backend. Violations produce bugs that are hard to find and fix.

### 2.1 Why Name Matching Is Wrong

Types are **protocol + metadata** (`docs/architecture/bits-thesis.md`). A type has no canonical layout. Its LLVM representation is derived at codegen time by the casting graph from `(protocol, bytes, variant)`.

```rust
// WRONG — matches type name directly:
Type::Custom(name) if name == "String" => { /* emit SSO string */ }

// RIGHT — queries protocol membership:
let (cat, _) = graph.type_to_protocol(universe, ty);
if cat == "String" { /* emit SSO string */ }
```

**Why this matters:**
1. A user-defined `type MyString: #String` would NOT match `name == "String"` but WOULD match `Cast.#String`. Programs using custom String-like types silently produce wrong codegen.
2. Every name match is a place where type system evolution must be manually tracked.
3. The backend was refactored to protocol-based dispatch (Phases 0-3). Name matches are leftovers.

### 2.2 The Protocol Dispatch Chain

```
Normalizer (normalizer.rs)
  │  Injects Cast.#<Category> properties during universe registration
  │  e.g., type String: #String → { "Cast.#String": true }
  ▼
Casting Graph (graph.rs)
  │  type_to_protocol(universe, ty) — queries Cast.#<Category> properties
  │  resolve_llvm_type(universe, ty, int_bits) — resolves LLVM type string
  ▼
LLVM Backend
  │  self.protocol_of(ty) (see §2.3) — NEVER matches type names
  ▼
Code Generation
```

### 2.3 The One Valid Query Pattern

```rust
// Access the casting graph's type_to_protocol result:
fn protocol_of(&self, ty: &Type) -> &str {
    let Some(graph) = self.ctx.casting_graph.as_ref() else { return "Bit" };
    let Some(univ) = self.ctx.type_universe.as_ref() else { return "Bit" };
    graph.type_to_protocol(univ, ty).0
}
```

### 2.4 Exceptions (Permitted by Rule 18)

These `Type` variants are compiler constructs that can be matched directly:

| Type variant | Rule | Reason |
|-------------|------|--------|
| `Type::Ptr(_)` | 18a | Compiler construct — not stored in universe |
| `Type::Bits(N)` | 18a | Width construct — compiler-internal |
| `Type::Vector(_, _)` | 18a | SIMD construct — compiler-internal |

Everything else — `Type::Custom(s)`, `Type::Applied(_, _)` — must go through protocol queries.

### 2.5 TBAA Exception (Rule 18c)

The `tbaa_node` function in `mod.rs` matches LLVM IR type strings (`"i64"`, `"float"`, `"ptr"`), not Brief type names. This is permitted because it operates on LLVM's type system, not Brief's.

### 2.6 Audit Checklist

Before committing any change to the LLVM backend:
```bash
# Must return ZERO results:
grep -rn 'Type::Custom.*if.*==.*"' src/backend/llvm/ | grep -v 'tests\.rs\|tb aa\|Int\b\|Float\b\|Bool\b'
```

Any match that is not `Int`, `Float`, or `Bool` (the bootstrap exceptions) is a violation.

## 3. Casting Graph

### 3.1 Architecture

The casting graph (`src/casting/graph.rs`) has 64 base protocol lanes:

```
Bit — root protocol (compiler axiom)
  ├── Int      → i64       — integer ALU
  ├── UInt     → i64       — unsigned integer ALU  
  ├── Float    → float/double — float ALU
  ├── String   → {i64,i64}  — SSO or heap string
  ├── Bool     → i8         — boolean comparison
  ├── Char     → i32        — unicode scalar
  └── Data     → ptr        — opaque pointer
```

Each protocol has a hardcoded direct lane to every other protocol. Protocol variants (`#String<UTF8>`, `#Float<IEEE754>`) are expressed through variant edges.

### 3.2 Key Functions

| Function | Location | What it does |
|----------|----------|-------------|
| `type_to_protocol(universe, ty)` | `graph.rs:492` | Returns `(category, variant)` from `Cast.#<Category>` properties |
| `resolve_llvm_type(universe, ty, int_bits)` | `graph.rs:540` | Returns LLVM type string from `(protocol, bytes)` |
| `is_protocol_member(ty, "#String")` | Via `type_to_protocol` | Checks protocol membership |
| `find_path(graph, src, dst)` | `graph.rs:??` | BFS for cast path between protocols |

### 3.3 How `type_to_protocol` Works

```rust
// Simplified from graph.rs:492
pub fn type_to_protocol(&self, universe: &TypeUniverse, ty: &Type) -> (String, String) {
    match ty {
        Type::Bits(_) | Type::Void => ("Bit".to_string(), String::new()),
        Type::HashWord(name) => return (name.clone(), String::new()),
        _ => {} // fall through to universe lookup
    }
    let key = ty.universe_key().and_then(|k| universe.get(k));
    let rt = key?;
    // Priority: Float → UInt → Int → String → Bool → Char → Data → Bit
    if rt.properties.contains_key("Cast.#Float") { ("Float", "") }
    else if rt.properties.contains_key("Cast.#UInt") { ("UInt", "") }
    else if rt.properties.contains_key("Cast.#Int") { ("Int", "") }
    else if rt.properties.contains_key("Cast.#String") { ("String", "") }
    else if rt.properties.contains_key("Cast.#Bool") { ("Bool", "") }
    else if rt.properties.contains_key("Cast.#Char") { ("Char", "") }
    else if rt.properties.contains_key("Cast.#Data") { ("Data", "") }
    else { ("Bit", "") }
}
```

**Never matches type names.** Always queries `Cast.#<Category>` properties injected by the normalizer.

## 4. State Fields and the %State Struct

### 4.1 How State Fields Work

Every `let` declaration at the top level of a `.bv` file becomes a state field:

```brief
let bound: Int = GetEnvInt!("BOUND");    // state field at index 0
let count: Int = 0;                       // state field at index 1
let bx0: Float32 = 0.0f32;               // state field at index 2
```

The `build_field_index` function in `mod.rs` assigns indices based on declaration order (or SoA-reordered order). Key data structures:

| Structure | Type | Description |
|-----------|------|-------------|
| `field_index_map` | `HashMap<String, usize>` | Field name → state slot index |
| `field_types` | `Vec<String>` | LLVM type per slot (e.g., `"i64"`, `"float"`, `"double"`) |
| `field_brief_types` | `Vec<Type>` | Brief type per slot |
| `idx_to_field_name` | `HashMap<usize, String>` | Reverse: index → field name |

### 4.2 Field Type Storage

All state fields are stored as `i64` in `%State` (from `push_field_type`, `mod.rs:918`). The `adapt_to_i64` / `ensure_typed_value` functions handle conversion between `i64` and the field's natural type at load/store time.

Exception: Float fields (`Cast.#Float` types) are sometimes stored as their native `float`/`double` type in `%State`. The `declare_state_type` function uses `protocol_llvm_type` which returns the native type.

### 4.3 Accessing State Fields

```rust
// Load from state (with !range metadata if available):
let (reg_name, brief_type) = self.emit_state_load_i64_by_idx(out, "  ", field_idx);

// Store to state:
self.emit_state_store_i64_by_idx(out, "  ", field_idx, &value_reg);
```

**Never use GEP directly.** The `emit_state_load/store_i64_by_idx` functions handle the correct GEP generation, load/store emission, and metadata attachment.

### 4.4 Struct Type Declarations

The `declare_struct_types` function emits required LLVM struct type declarations:

```llvm
%SmallString64 = type { i64, i64, i64, i64, i64, i64, i64, i64, i64 }
%StaticString = type { i64, i64 }
%String = type { i64, i64 }
%UTF8View = type { i64, i64 }
```

These must always be present. Without them, clang 18.1.3's LICM `sinkRegion` pass segfaults. Universe-registered struct types are also emitted here, but any type matching the hardcoded names above is skipped to prevent duplicate declarations.

## 5. Loop Emission Strategies

### 5.1 Dispatch Logic (`mod.rs`)

When a convergent transaction (node/txn) is emitted, the dispatch selects one of three strategies:

```
Entry → build_field_index → ... → dispatch:
  1. Pure counter fold — if no written non-counter state fields: O(1) single store
  2. Inline SSA — if write_density >= 0.5 AND total_fields < 8: insertvalue chain
  3. PerFieldPhi — default: per-field phi nodes
     (may be further split into batch-loop if hoistable guards detected)
```

### 5.2 PerFieldPhi (Default)

`counter.rs :: emit_countable_main`

Each written state field gets its own phi node in the loop header:

```llvm
.cm_header:
  %cmc = phi i64 [ %init_count, %entry ], [ %next_count, %.cm_latch ]
  %ppf228 = phi float [ %init_bx0, %entry ], [ %pbf193, %.cm_latch ]
  %ppf229 = phi float [ %init_by0, %entry ], [ %pbf194, %.cm_latch ]
  ; ... one phi per written field ...
  ;; body accesses values through phi registers
  ;; latch computes backedge values
```

Key data structures:

| Structure | Type | Description |
|-----------|------|-------------|
| `phi_field_regs` | `HashMap<String, String>` | Field name → phi register name |
| `backedge_field_regs` | `HashMap<String, String>` | Field name → backedge register name |
| `pending_phi_backedge` | `HashMap<String, String>` | Computed backedge values (written by body) |
| `last_val_temps` | `HashMap<String, String>` | Last computed value for each let-binding |
| `last_val_types` | `HashMap<String, Type>` | Type for each last computed value |

### 5.3 Batch-Loop Optimization

When the loop body contains hoistable `when` guards (periodic prints with `PrintLn!`, termination checks), the dispatch splits the loop:

```
Outer loop (.oh_* → .ox_* → .done_*):
  Tracks batches, checks termination
  Inner loop (.inner_* → .il_* → .inner_exit_*):
    Pure compute — no branches, no function calls
    Runs for batch_size iterations
```

**Where guards must be emitted:** In `.inner_exit_124` — this block is dominated by `.inner_124` (where phi registers are valid). Never emit guards in `.ox_124` (the outer body) because that block is NOT dominated by the inner loop's body blocks, and let-binding registers from the inner loop are invalid there.

**Identifier remapping:** When guards are hoisted, let-binding references (e.g., `energy`) are remapped to their corresponding state fields (e.g., `last_energy`) via `let_to_field` map built from `Statement::Assign(let_name, Expr::Identifier(field_name))` patterns.

### 5.4 Loop Metadata

Loop vectorization metadata is emitted on the latch branch:

```llvm
br label %.cm_header, !llvm.loop !100
!100 = !{!100, !101, !102}
!101 = !{!"llvm.loop.vectorize.enable", i1 true}
```

To force vectorization, add metadata here. Note that LLVM's loop vectorizer cannot if-convert branches containing opaque function calls (`call @PrintInt#`, etc.). Only pure-compute loops (no function calls in the body) can be vectorized.

## 6. Pointer Handling

### 6.1 Internal Representation

`Ptr<T>` values are stored as `i64` in `%State` (via `ptrtoint` at emission time). When used in load/store/call instructions, they must be converted back to LLVM `ptr` via `inttoptr`:

```llvm
%t0_p = call ptr @malloc(i64 %size)    ; Malloc returns ptr
%t0 = ptrtoint ptr %t0_p to i64         ; Convert to i64 for state storage
; ... later, when using the pointer:
%t_ptr = inttoptr i64 %t0 to ptr         ; Convert back before load
%val = load i64, ptr %t_ptr               ; Load through pointer
```

### 6.2 `!invariant.load` on Ptr Fields

`Ptr<T>` state fields carry `!invariant.load` metadata on their loads (added in `helpers.rs:2635`). This tells LICM the pointer value never changes after initialization, enabling hoisting of the pointer load to the loop preheader.

### 6.3 Where `inttoptr` Is Added

- `emit_expr.rs: Deref` — before loading through a Ptr value
- `emit_stmt.rs: Deref store` — before storing through a Ptr value  
- `counter.rs: Deref store` — same pattern in loop body stores
- `emit_expr.rs: emit_user_call` — Ptr arguments before function calls
- `emit_binop_from_config` — preserves Ptr return type for Ptr+Int Add/Sub

## 7. Common Pitfalls Checklist

Before merging any backend change, verify:

- [ ] **No type name matching** — `git grep 'Type::Custom.*if.*=="' src/backend/llvm/` returns zero results (except for `Int`, `Float`, `Bool` bootstrap exceptions)
- [ ] **No hardcoded String/Data/Char/UInt** — use `is_protocol_member(ty, "#String")` or `protocol_of(ty) == "String"`
- [ ] **`llvm_type()` used correctly** — never hardcode `"i64"` or `"float"` as type strings
- [ ] **Phi register use limited to the loop body** — phi registers from `.inner_124` are invalid in `.ox_124`; use state loads instead
- [ ] **Registers via `gen_reg()`** — no hand-written `%tN` register names
- [ ] **`push_field_type` overrides to `"i64"`** — all state fields are i64 unless float
- [ ] **~New field added to `FunctionContext`?** Must be initialized in the default constructor
- [ ] **HashMap iteration determinism** — every HashMap iteration that emits IR must be sorted by key first
- [ ] **`declare_struct_types` has the hardcoded four** — `SmallString64`, `StaticString`, `String`, `UTF8View` must always be emitted

## 8. Testing and Verification

```bash
# Unit tests
cargo test --lib

# Benchmark correctness
bash benchmarks/build_and_bench.sh --correctness

# Runtime performance
bash benchmarks/build_and_bench.sh --runtime

# Compare against baseline worktree
bash benchmarks/compare_baseline.sh <benchmark_name>

# Audit for name matching violations
grep -rn 'Type::Custom.*if.*==.*"' src/backend/llvm/ | grep -v 'tests\.rs\|Int\b\|Float\b\|Bool\b'
```

## 9. Quick Reference: Protocol Categories vs LLVM Types

| Protocol | Default LLVM type | Width (bytes) |
|----------|-------------------|:-------------:|
| `#Bit` | `i64` | 8 |
| `#Int` | `i64` | 8 |
| `#UInt` | `i64` | 8 |
| `#Float` | `double` (default 64-bit) | 8 |
| `#Float32` | `float` | 4 |
| `#String` | `{ i64, i64 }` | 16 |
| `#Bool` | `i8` | 1 |
| `#Char` | `i32` | 4 |
| `#Data` | `ptr` | 8 |

These are resolved by `resolve_llvm_type()` in the casting graph. Never hardcode them.

## 10. Adding a New Protocol Type

1. Define the type in stdlib `.bv` with protocol membership:
   ```brief
   type MyType: #String { !> bytes: 16; op CastTo(#Int): my_parse(#L); };
   ```
2. If a new protocol category is needed, add a lane in `graph.rs::new()`:
   ```rust
   self.set_lane("MyProto", "Bit", LaneKind::Bitcast);
   ```
3. Add normalizer injection for `Cast.#MyProto` in `normalizer.rs`.
4. Add protocol arm in `type_to_protocol` priority chain.
5. Add LLVM type resolution in `resolve_llvm_type`.
6. **No name-based matching in the backend.**
