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

## 9. Fragile Strategies — What Breaks If You Change It

The backend has several interacting subsystems. A change in one area can silently break another. This section documents each fragile subsystem, what it depends on, and what breaks if those dependencies are violated.

### 9.1 Batch-Loop Optimization (emit_countable_batched_main)

**What it does:** Splits a convergence loop into an outer structural loop and an inner pure-compute loop when hoistable `when` guards are detected.

**Dependencies:**
- `loop_peeling.rs::split_hoistable()` — detects guards in the body
- `loop_peeling.rs::extract_batch_size_from_guards()` — extracts batch_size from `when count % N == 0` conditions
- `counter.rs::emit_countable_batched_main()` — emits the two-loop structure

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Emit outer guards in `.ox_` block instead of `.inner_exit_` | **Instruction does not dominate all uses** — phi registers from `.inner_124` are invalid in `.ox_124`. Use `.inner_exit_124` which IS dominated by `.inner_124`. |
| Skip the `let_to_field` remapping | **`@energy` undefined global** — guard bodies reference let-bindings (`energy`) that aren't state fields. Must be remapped to their state field equivalent (`last_energy`). |
| Clear `last_val_temps` before guard emission | Identifiers resolve to globals instead of phi registers. Guards must see phi registers, not state loads. |
| Remove the `write_set` from `emit_countable_body` call | State field assignments in guards write to phi backedge tables that don't exist in the exit block. |
| Change `batch_size` computation | Wrong batch count — inner loop runs incorrect number of iterations, producing wrong results. |

**The dominance tree for the batch loop:**

```
.oh_124 [outer header]
  │
  └──→ .inner_124 [inner header — phis ARE valid here and in .inner_exit_124]
         │
         ├──→ .il_124 [inner latch — computed values, NOT valid in outer blocks]
         │      │
         │      └──→ back to .inner_124
         │
         └──→ .inner_exit_124 [guard emission — phi registers from .inner_124 ARE valid]
                │
                └──→ .ox_124 [termination check — load from %State only]
                       │
                       ├──→ .done_124 [exit — load from %State only]
                       └──→ .ol_124 [outer latch]
                              │
                              └──→ back to .oh_124
```

**Key invariant:** The only registers valid in `.inner_exit_124` are:
- PHI registers defined in `.inner_124` (all fields in `phi_field_regs`)
- Registers computed inside `.inner_exit_124` itself
- Registers loaded from `%State`

Registers from `.il_124` (computed body values) are NOT valid in `.inner_exit_124` because `.il_124` does not dominate `.inner_exit_124`.

### 9.2 PerFieldPhi Register Tracking

**What it does:** Each written state field gets a phi node. The body writes to `pending_phi_backedge`, the latch reads it for the backedge identity copy.

**Dependencies:**
- `phi_field_regs` — maps field name → phi register name (set in header)
- `backedge_field_regs` — maps field name → backedge register name (set in header)
- `pending_phi_backedge` — maps field name → computed value (set by body)
- `last_val_temps` — maps let-binding name → register name (set by body)

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Add a field to `write_set` without a corresponding phi | **Undefined value** in the backedge — LLVM picks `undef` for the missing phi. |
| Remove a field from `write_set` that's still assigned | **Silent value loss** — the assignment writes to `pending_phi_backedge` but no phi picks it up. The next iteration reads the old value. |
| Clear `phi_field_regs` without clearing `pending_phi_backedge` | **Dominance violation** — stale entries make the body reference registers that no longer exist. |
| Mix up `phi_field_regs` and `backedge_field_regs` in the latch | **Wrong backedge value** — the phi selects the wrong predecessor register. |
| Emit `fadd float 0.0` instead of `add i64 0` for integer fields | **LLVM IR type error** — float operation on i64 type. |
| Forget to sort `sorted_fields` | **Non-deterministic IR** — HashMap iteration order changes every compilation, producing ~9% performance variation between runs. |

### 9.3 SoA Field Reorder (analysis/soa_reorder.rs)

**What it does:** Reorders state field declarations from AoS to SoA layout before `build_field_index` assigns indices.

**Dependencies:**
- Runs between `items` extraction and `build_field_index` in `generate()`
- Uses `parse_numeric_prefix` to detect indexed families (bx0, bx1, etc.)
- Proves data independence between family members

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Move SoA reorder after `build_field_index` | **Wrong indices** — field_index_map is already built with AoS layout. All GEP offsets shift. |
| Skip the independence proof | **Wrong results** — if `bx0` references `bx1` (through let-bindings), reordering changes computation order. |
| Change the field name prefix detection | **Fields not grouped** — renamed fields (e.g., `body0_x`) produce wrong prefixes. |
| Sort `non_float_indices` differently | **Non-deterministic output** — non-field items move around, changing the IR structure. |

### 9.4 Brief-Level LICM (analysis/licm.rs)

**What it runs:** Before the dispatch, identites loop-invariant let-bindings and prepends them to the body.

**Dependencies:**
- Runs after `hoist_terminating_guard` but before the dispatch
- Depends on `write_set` to determine which identifiers are variant

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Remove the `state_fields` parameter check | **Local variables hoisted as loop-invariant** — a let-binding `step = dt * 0.5` uses `dt` (not a state field) so it's not in `write_set`. Without checking `state_fields`, the function thinks `dt` is invariant (it IS, but because it's a const, not because it's a state field). Actually this is correct — `dt` IS invariant. The bug would be if a LOCAL variable that changes each iteration is NOT in `state_fields` — the function would incorrectly mark it as invariant. |
| Hoist expressions containing `Expr::Call` | **Side effects lost** — function calls inside invariant-looking expressions are hoisted, changing execution count. |
| Hoist after the dispatch instead of before | **Different dispatch decisions** — if a hoisted binding changes the body structure, the dispatch might select a different strategy. |

### 9.5 Hoist Terminating Guard (hoist_terminating_guard)

**What it does:** Removes `Term(..)` / `TermBang(..)` statements from the body and places the last guard's body in `post_hoist` if it contains `TermBang`.

**Dependencies:**
- Runs before the dispatch and before the batch-loop detection
- Must be called before `split_hoistable` because it removes the termination guard from the body

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Swap order with `split_hoistable` | **Guard body emitted in inner loop** — the termination guard's TermBang body (containing `PrintLn!`) ends up in the inner loop, blocking if-conversion. |
| Don't check for `TermBang` in the last guard | **Terminating print never fires** — the guard body isn't hoisted to `post_hoist`. |
| Don't remove `Term(..)` from the body | **`emit_countable_body` hits the catch-all** — `Statement::Term(None)` falls through to `_ => {}` and is silently dropped. |

### 9.6 Vector Phi Emission (Dormant — DO NOT RE-ENABLE WITHOUT RESEARCH)

**What it is:** The dispatch selects a "VectorPhi" path when `detect_vector_groups` finds groups and `total_fields > 14`. However, `counter.rs:213-221` immediately clears all vector phi state, so the actual emission is PerFieldPhi.

**Why it's dormant:** Vector phi emission was disabled because:
1. `<2 x float>` groups added extract/insert overhead that dwarfed register-pressure benefit
2. `<8 x float>` groups triggered AVX lane-crossing latency
3. Naming-based grouping was a fragile heuristic
4. The SoA reorder pass + SLP vectorizer achieves the same result without explicit vector phis

**What breaks if re-enabled without fixing the root cause:**

| Risk | Why |
|------|-----|
| `extractelement_cache` used without clearing | Same extract value used across iterations — stale values. |
| `field_to_phi` conflicts with `phi_field_regs` | Two sets of phi nodes for the same field — which one does the body read? |
| Vector phis + batch-loop interaction | The batch loop's inner exit stores phi values to %State. Vector phi values are `<N x float>` — storing them would require deconstructing the vector. |

### 9.7 `push_field_type` i64 Override

**What it does:** Forces all state field LLVM types to `"i64"` regardless of their Brief type.

**Dependencies:**
- `field_types` — used by `emit_state_load_i64_by_idx` / `emit_state_store_i64_by_idx` for GEP+load/store
- `field_brief_types` — used by `llvm_type()` / `protocol_llvm_type()` for protocol-based type resolution

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Remove the i64 override | **%State struct layout changes** — float fields would be `float` instead of `i64`. All GEP offsets stay the same (the struct is reified by `declare_state_type` which uses `protocol_llvm_type`), but the load/store type would mismatch. |
| Change the override to `float` for float fields | **`push_field_type` comment says "always i64"** — code paths that `load i64` from state would get wrong types. The `adapt_to_i64` path expects `i64` loads. |

### 9.8 `!invariant.load` on Ptr Fields

**What it does:** Adds `!invariant.load` metadata to all loads of `Type::Ptr(_)` state fields.

**Dependency:** Asserts that `Ptr<T>` fields are assigned exactly once (at init) and never reassigned.

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Apply `!invariant.load` to non-Ptr fields | **Stale values** — if a field IS modified, LLVM caches the initial load value forever. |
| Remove the `!invariant.load` from Ptr fields | **One extra GEP+load per iteration per Ptr field** — LICM doesn't hoist the pointer load, adding ~4 cycles per iteration. |

### 9.9 `declare_struct_types` Hardcoded Skip Set

**What it does:** Emits `%SmallString64`, `%StaticString`, `%String`, `%UTF8View` as hardcoded type declarations, then skips them during the universe iteration to avoid duplicates.

**Dependency:** The skip set must always include ALL names emitted in the hardcoded block.

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Add a name to the hardcoded block without adding it to the skip set | **Clang error: `redefinition of type '%NewType'`** — duplicate type declaration. |
| Remove a name from the skip set without removing it from the hardcoded block | Same — duplicate declaration. |
| Remove a name from the hardcoded block | **If the universe doesn't have it, clang LICM may segfault** — missing struct type triggers `sinkRegion` crash in clang 18.1.3. |

### 9.10 Constant Float Emission Pattern

**What it does:** Emits `float` literals as a single `bitcast i32 <hex> to float` instruction instead of the old `add i32 0, N` + `bitcast` + `fadd float 0.0` sequence.

**Dependency:** All `float` literals go through `emit_expr.rs::Expr::Float`. The `float_to_llvm_hex` function converts the f32 value to its i32 bit pattern.

**What breaks if changed carelessly:**

| Change | Failure mode |
|--------|--------------|
| Revert to the old pattern | **~2100 extra IR instructions per nbody compilation** — no runtime impact but more work for LLVM's optimizer. |
| Use `float <value>` directly instead of `bitcast` | **LLVM verifier error** — high-precision float literals like `"0.001660076642744037"` have more significant digits than f32 can represent. The bitcast from hex avoids this. |

## 10. Quick Reference: Protocol Categories vs LLVM Types

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
