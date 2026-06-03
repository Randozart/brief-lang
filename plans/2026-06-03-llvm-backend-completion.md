# LLVM Backend Completion Plan

**Date:** 2026-06-03
**Status:** In progress — Phase 1 complete, nbody benchmark added

## Overview

The LLVM backend at `src/backend/llvm.rs` (6024 lines) lags behind the interpreter (`src/interpreter.rs`, 2327 lines). The interpreter already supports the full Brief expression language: structs, enums, tuples, lists, hash maps, pattern matching, and block expressions. The LLVM backend either silently returns `0` or skips these AST nodes entirely.

The interpreter IS the reference implementation. If the interpreter runs it, the LLVM backend should compile it. This document lays out the exact gaps and a plan to close them — without regressing any existing optimization paths.

## Critical Constraints

### No Regression Principle
Every existing optimization path MUST continue to work unchanged:
- Path 2 (dead-field elimination + pure counter fold)
- Path 3 (compile-time precompute)
- Path 4 (enum switch-dispatch)
- Path 5 (thread pool async dispatch)
- SROA (scalar replacement of aggregates)
- SLP hazard analysis
- Constant folding/inlining/deduplication
- Folded while-loop emission
- Precondition cascade fix (dispatch-chain collapse)

### Implementation Strategy
- **Additive only**: New match arms for new Expr/TopLevel variants. Never modify existing arms.
- **Contract-preserving**: Struct/enum codegen must preserve type and contract information so existing optimizations can reason about them.
- **The interpreter is correct**: When in doubt about semantics, implement what the interpreter implements at the specified line number.

---

### Phase 1 Completion (2026-06-03)

**Status: Done.** 5 new tests, all 405 existing tests pass, 0 regressions in all 10 benchmarks.

**Changes:**
- `struct_types: HashMap<String, Vec<(String, Type)>>` added to `LlvmBackend`
- `TopLevel::Struct` handled in `generate()` — registers field layouts
- `Expr::StructInstance(typename, fields)` → `alloca i64, i64 <N>` + store per field + `ptrtoint`, returns `Type::Custom(name)`
- `Expr::ObjectLiteral(fields)` → same alloca+store+ptrtoint pattern
- `Expr::FieldAccess(obj, field_name)` → `inttoptr` + `GEP` + `load` at resolved field offset
- `let_binding_types: HashMap<String, Type>` — type propagation through `Statement::Let` so `FieldAccess(Identifier("p"), "x")` works through variable references

**Tests:**
- `test_struct_type_registered` — verifies `struct_types` populated correctly
- `test_struct_instance_emits_alloca_store_ptrtoint` — checks IR for 2-field struct construction
- `test_field_access_resolves_correct_offset` — GEP into let-bound struct instance
- `test_field_access_unknown_struct_falls_back` — fallback `add i64 0, 0` for non-struct types
- `test_object_literal_emits_alloca_store_ptrtoint` — ObjectLiteral follows same pattern

SROA/mem2reg handles scalarization — the `alloca i64` + `GEP` + `store` + `load` chain is decomposed into flat scalar registers by `opt -O2`. Zero runtime cost for the struct abstraction.

### nbody Benchmark (2026-06-03)

**Status: Added.** CLBG n-body gravity simulation, 5 bodies, 50M timesteps.

- 32 state fields (15 positions, 15 velocities, count, bound)
- 5 Newton iterations per sqrt (pure Brief Float math — no FFI)
- 10 unrolled pair interactions per tick
- C reference uses `sqrt()` from libm

---

## Priority 1: Struct Codegen (DONE)

### What the Interpreter Does
- `Expr::StructInstance(typename, fields)` → `Value::Instance { typename, fields: HashMap<String, Value> }` (line 1662)
- `Expr::FieldAccess(obj, field_name)` → looks up field by name in Instance fields (line 1648)
- `Expr::ObjectLiteral(fields)` → `Value::Instance { typename: "ObjectLiteral", fields }` (line 1672)
- `TopLevel::Struct { name, fields }` → type information consumed by typechecker

### Current LLVM Codegen
| Feature | Line | What Happens |
|---------|------|-------------|
| `TopLevel::Struct` | 427 | Silently skipped (`_ => {}`) |
| `Expr::StructInstance` | 2674 | Evaluates field exprs for side effects, returns `add i64 0, 0` |
| `Expr::ObjectLiteral` | 2675 | Same as StructInstance |
| `Expr::FieldAccess` | 2676 | Returns object pointer as-is (`add i64 0, <obj>`) |

### What's Needed

#### Phase 1a: LLVM Struct Type Generation
In `generate()`, before the `_ => {}` catch-all, add a `TopLevel::Struct` arm:
```
TopLevel::Struct(s) => {
    // Generate LLVM struct type:  %MyStruct = type { i64, i64, ... }
    // Store in self.struct_types: HashMap<String, Vec<usize>> (name → field offsets)
    // One i64 slot per field. Enums stored as ptrtoint i64.
}
```

Field order follows `StructField` declaration order. Each field occupies one `i64` slot (Brief's universal register width). Float fields use `i64` representation with `bitcast` for operations — same convention as the existing `%State` struct.

#### Phase 1b: StructInstance Emission
Replace the stub at line 2674:
1. Look up struct type in `self.struct_types`
2. `alloca i64, i64 <num_fields>`
3. For each field: `emit_expr` → `getelementptr` → `store`
4. `ptrtoint i64* → i64` (same as ListLiteral representation)

This matches the existing pattern for `ListLiteral` (line 2638) and enum constructors (line 2616). LLVM's SROA pass will scalarize the alloca → individual registers, so the runtime cost is identical to flat scalar fields after optimization.

#### Phase 1c: FieldAccess Emission
Replace the stub at line 2676:
1. `inttoptr i64 <obj> → i64*`
2. Look up field offset in `self.struct_types[name]`
3. `getelementptr i64, i64* <ptr>, i64 <offset>` → `load i64`

After SROA, the GEP + load becomes a direct reference to the scalar register.

### Non-Regression Guarantee
- Struct access in txn bodies gets SROA'd by `opt -O2` → same scalar registers as flat fields
- Dead-field elimination operates on field-level operations, not struct-level. The liveness analysis in `transition_graph.rs` already works at the field granularity.
- No changes to `emit_folded_loop`, `emit_ssa_main`, `emit_reactor`, or any existing optimization path.

---

## Priority 2: Enum Codegen

### What the Interpreter Does
- Enum constructor call (`Expr::Call` with uppercase name like `Ok(value)`) → looks up enum in state, creates `Value::Enum(name, variant, fields)` (line 999)
- `Expr::PatternMatch { value, variant, fields }` → extracts discriminant, matches variant name, binds fields (line 1682)
- `Expr::Match { value, arms }` → switch on variant, evaluate matched arm body (line 1897)
- `Statement::Unification` → pattern-match in state, execute body on match (line 579)
- `Value::Enum(enum_name, variant_name, HashMap<String, Value>)` — variant name as string, fields as HashMap

### Current LLVM Codegen
| Feature | Line | What Happens |
|---------|------|-------------|
| `TopLevel::Enum` | 427 | Silently skipped |
| Enum constructor (Call with uppercase) | 2616 | Ad-hoc: alloca with N+1 i64 slots, store discriminant at slot 0, store fields at slots 1..N, ptrtoint |
| `Expr::Match` | 2691 | ✅ switch on discriminant (`and i64 ..., 255`) with phi merge |
| `Expr::PatternMatch` | 2736 | ✅ `and i64 ..., 255` + `icmp eq` |

### What's Needed

#### Phase 2a: Enum Type Registration
In `generate()`, add a `TopLevel::Enum` arm:
```
TopLevel::Enum(e) => {
    // Register enum name → variant map
    // Each variant stores: (discriminant_index, field_count)
    // e.g., Option<T>: None→0(0), Some→1(1)
    // e.g., Result<T,E>: Err→0(1), Ok→1(1)
    self.enum_types.insert(e.name.clone(), e.variants);
}
```

Discriminant assignment: reserve `0` for error/empty variants (`None`, `Err`). Non-empty variants get `1, 2, 3, ...`. This matches the existing convention at line 2619: `let disc_val = if name == "None" || name == "Err" { 0u64 } else { 1u64 }`.

#### Phase 2b: Unified Constructor
The existing ad-hoc constructor at line 2616 is correct but scatters the convention. Refactor:
1. Check `self.enum_types` for the enum name
2. Look up the variant → discriminant_index mapping
3. Use the same `alloca i64, i64 <N+1>` + store-discriminant-at-0 pattern
4. This refactor changes nothing semantically — just moves the logic from a hardcoded check to a data-driven lookup

#### Phase 2c: Field Binding in Match Arms
The current `Expr::Match` implementation (line 2691) dispatches on discriminant but does NOT extract variant fields into named bindings. When a match arm has `Variant { name, fields }`, the field names should be bound as SSA registers pointing to `GEP i64* <ptr>, i64 <1..N>`.

For example, `match result { Ok(value) => value, Err(_) => 0 }` should:
1. Extract discriminant at slot 0
2. In the Ok arm: `GEP i64* %ptr, i64 1` → `load i64` → bind to `value`
3. All existing phi-merge logic (line 2729-2733) stays unchanged

### Non-Regression Guarantee
- The discriminant-at-slot-0 convention is already used by `Expr::PatternMatch` (line 2736-2743) and `Expr::Match` (line 2693)
- Enum dispatch and sparse_dispatch benchmarks already test this path — same LLVM IR after optimization
- The switch-dispatch optimization (Path 4) dispatches on transaction trigger values, not user enum types — no overlap

---

## Priority 3: Collection Operations

### What the Interpreter Does
Collection methods are dispatched via string matching on the function name in `Expr::Call` (interpreter lines 1034-1250):
- `list_append(list, item)` → clones list, pushes item (line 1037)
- `get(list, index)` → bounds-checked index with Option return (line 1052)
- `HashMap::new` → `Value::HashMap(HashMap::new())` (line 1065)
- `insert(map, key, value)` → clones map, inserts (line 1078)
- `get(map, key)` → looks up, returns Option (line 1085)
- `contains_key`, `remove`, `len`, `is_empty`, `keys`, `values`
- `HashSet` equivalents
- Stack operations: push, pop, clear, top
- Queue operations: enqueue, dequeue, clear, front
- StringBuilder operations: append, to_string, clear, len, is_empty

### Current LLVM Codegen
`Expr::Call` dispatch (line 2561):
1. Check `self.frgn_map` → FFI call
2. Check `self.defn_params` → defn call
3. Uppercase name → ad-hoc enum constructor (line 2616)
4. Lowercase name → `call i64 @<name>(<args>)` as defn call

Collection method names (`list_append`, `hashmap_insert`, etc.) are lowercase, so they hit path (4). If there's no defn registered, the generated LLVM IR will have a call to an undefined function → linker error.

### What's Needed

#### Phase 3a: ListLen
Replace the stub at line 2657.
**Problem**: Lists are stored as `ptrtoint i64* → i64`. There's no length information.

**Solution**: Change the list representation from `[ptr]` to `[ptr, len]`. A list occupies 2 × i64 slots: slot 0 is the data pointer, slot 1 is the element count.

- `ListLiteral`: `alloca i64, i64 <n + 2>` — allocate 2 extra slots for [header_pointer, length]
- Store elements at slots 2..n+1
- Store `n` at slot 1
- Store `ptrtoint (gep slot 2)` at slot 0 — pointer to first data element
- `ptrtoint <alloca_ptr> → i64` — return pointer to slot 0 (the header)

- `ListLen(list)`: `inttoptr i64 <list> → i64*`, `GEP i64* <ptr>, i64 1`, `load i64` → returns length
- `ListIndex(list, idx)`: `inttoptr i64 <list> → i64*`, load slot 0 for data pointer, `GEP + idx + 2` for the value

This is backward-compatible because existing benchmarks don't use ListLen. The `ListLiteral` + `ListIndex` pattern (used in self-hosting compiler) stays correct because:
- `ListLiteral` returns a pointer (ptrtoint)
- `ListIndex` takes that pointer, does inttoptr, GEPs into it

The key invariant: the pointer returned by ListLiteral ALSO points to the start of the data (slot 2, or equivalently slot 0 with +2 offset). For backward compatibility, we need `ListIndex` to work with both old (no header) and new (2-slot header) representations… but since we only have one ListLiteral implementation and no other list source, we just change both atomically.

**Alternative (simpler)**: Don't change the layout. Store the length as a separate SSA value tracked by `emit_expr` alongside the register name. But this breaks when a list is passed through function calls or stored in state. The header approach is forward-compatible.

#### Phase 3b: Slice
Replace the stub at line 2658.
Full slice `list[start..end; stride]`:
1. Compute actual start: `start.unwrap_or(0)`
2. Compute actual end: `end.unwrap_or(list_len)`
3. Compute stride: `stride.unwrap_or(1)`
4. Compute result length: `(end - start + stride - 1) / stride`
5. `alloca i64, i64 <result_len + 2>` — new list with 2-slot header
6. Loop: copy element `list[start + i*stride]` to new slot `i`
7. Return `ptrtoint <new_alloca> → i64`

The stride loop can be emitted as a simple `br` loop or as a counted `while` — LLVM's optimizer will unroll or vectorize as appropriate.

#### Phase 3c: MultiSlice
Replace the stub at line 2667.
Delegate to Slice (for Range coordinates) or ListIndex (for Index coordinates), matching the interpreter's implementation at line 1848-1895.

#### Phase 3d: Collection Method Dispatch
Add a dispatch table in `Expr::Call`:
1. Check `self.frgn_map` (existing)
2. Check collection methods (NEW):
   - `list_append` → allocate new list (length+1), copy elements, append new
   - `get` → ListIndex with bounds check, wrap in Some/None
   - `HashMap::new`, `HashSet::new`, `Stack::new`, `Queue::new` → allocate empty header
3. Check `self.defn_params` (existing)
4. Continue with existing logic

Map/Set/Stack/Queue operations are represented as Lists under the hood. The LLVM codegen mirrors this: a HashMap is a list of (key, value) pairs.

### Non-Regression Guarantee
- ListLen/Slice/MultiSlice are new operations — no existing code uses them
- The list header change (adding [ptr, len]) changes ListLiteral and ListIndex, but since both are changed atomically, all existing benchmarks continue to work
- Collection method dispatch is additive: it goes between `frgn_map` check and `defn_params` check, neither of which is touched
- Float benchmarks (kalman_filter, float_math) don't use any of these operations

---

## Priority 4: Tuple + TupleDestructure

### What the Interpreter Does
- `Expr::Tuple(exprs)` → evaluates into `Value::List(values)` (line 1815)
- `Expr::TupleDestructure(names, expr)` → destructures `Value::List` into state bindings (line 1822)

### Current LLVM Codegen
| Feature | Line | What Happens |
|---------|------|-------------|
| `Expr::Tuple` | 2672 | Evaluates sub-exprs, returns `add i64 0, 0` |
| `Expr::TupleDestructure` | 2673 | Evaluates inner, returns `add i64 0, <inner>` |

### What's Needed
Tuple representation reuses the List layout (2-slot header [ptr, len] + element slots). TupleDestructure emits `GEP` + `load` per destination name.

Tuples are purely an intermediate representation — they should not survive to final LLVM IR. After SROA and optimization, tuple construction/destruction should vanish entirely, leaving direct register-to-register assignments.

### Non-Regression Guarantee
- Tuples are not used in any current benchmark
- The implementation is a simple composition of existing List primitives

---

## Priority 5: Runtime-Sized Allocation

### What the Interpreter Does
The interpreter allocates `Vec<Value>` on the Rust heap for lists. The size is determined at evaluation time by evaluating the list literal elements.

### Current LLVM Codegen
- `Expr::ListLiteral` uses `alloca i64, i64 <n>` with a compile-time-constant `n` (line 2641)
- No mechanism for runtime-sized allocation
- Brief's philosophy: the compiler proves bounds from contracts at compile time

### What's Needed
For benchmarks like spectral-norm (which needs `float[N]` where N is a runtime parameter), there are two approaches:

#### Approach A: Fixed-Max from Contract Range
If the contract provides `0 <= N <= 8000`, the compiler can emit `alloca i64, i64 8002` — allocating the maximum. The actual used portion is N elements. This is Brief's idiomatic approach: contracts prove the bound, the compiler allocates accordingly.

This requires the range analyzer (`src/analysis/range.rs`) to propagate bounds to collection allocations. When a `ListLiteral` or other collection constructor uses an expression whose range is known from contracts, the size is the upper bound.

#### Approach B: Runtime `alloca`
LLVM supports `alloca i64, i64 %runtime_size`. This is valid when the contract doesn't provide a compile-time-known bound but the runtime size is available.

#### Recommendation
Implement Approach A first. It's Brief-idiomatic and doesn't require any runtime machinery. If a contract can't bound a collection size, that's a type-system issue — the programmer adds a contract. For spectral-norm specifically, N is read from stdin or an env var (`__get_env_int`), so the contract is `N > 0`.

The implementation: in `emit_expr`, when emitting a collection constructor that uses a runtime value for size, check `self.field_ranges` (or equivalent) for a bound on that value. Emit `alloca` with the upper bound from the range.

### Non-Regression Guarantee
- Existing benchmarks use compile-time-known ListLiteral sizes → no change
- The contract-to-size resolution is a new code path that doesn't affect existing paths
- If no range information is available, fall back to existing fixed-size behavior

---

## Priority 6: ForAll / Exists Quantification

### What the Interpreter Does
- `Expr::ForAll { var, expr }` → stub, always returns `Bool(true)` (line 1838)
- `Expr::Exists { var, expr }` → partial, checks if list is non-empty (line 1839)

### Current LLVM Codegen
| Feature | Line | What Happens |
|---------|------|-------------|
| `Expr::ForAll` | 2746 | Returns `add i64 0, 1` (always true) |
| `Expr::Exists` | 2747 | `icmp ne i64 ..., 0` (checks non-zero) |

### What's Needed
Full quantification is a bounded loop over the value range of `var`:

For `ForAll(x, predicate)`:
1. Determine the value range of `x` from contracts
2. Emit a loop over all values in range
3. Evaluate predicate for each value
4. Return `true` only if all evaluations return `true`

For `Exists(x, predicate)`:
1. Same loop structure
2. Return `true` as soon as any evaluation returns `true`

Both require the range analyzer to provide value ranges, which ties into Priority 5's contract-to-size resolution.

### Non-Regression Guarantee
- ForAll/Exists are used in contract expressions, never in hot benchmark paths
- The existing contract proof engine (`src/proof_engine.rs`) handles these at the symbolic level — the LLVM backend only emits them when they survive to runtime

---

## Priority 7: Nested Complex Types (Recursive Structs/Enums)

### What the Interpreter Does
Recursive enum types work naturally via owned values:
```
enum Tree { Node(Tree, Tree), Leaf }
```
A `Value::Enum("Tree", "Node", {"left": Value::Enum(...), "right": Value::Enum(...)})` is eagerly constructed.

### Current LLVM Codegen
The existing ad-hoc enum constructor (line 2616) uses `alloca i64, i64 <N+1>` for each enum constructor. This works for a single level of nesting (e.g., `Ok(Some(5))`), but stores nested enums as `ptrtoint i64* → i64` — a pointer to the inner enum's alloca. After the function returns, that alloca is gone.

### What's Needed
For persistent structured data that survives a function boundary, enums need storage in:
1. The `%State` global struct (for state fields)
2. A stack-allocated buffer (for local computations)
3. An arena (for long-lived heap data, if needed)

The simplest approach for Priority 7b: when an enum constructor is called inside a `StateDecl` field assignment, allocate inside `%State`. When called as a local expression inside a body, use alloca + memcpy to a scratch region.

For the binary-trees benchmark specifically:
- Tree nodes are constructed inside a transaction body
- The tree roots are stored in state fields
- During construction, local allocas suffice (after SROA, the intermediate alloca chain disappears)
- For persistence, nodes are stored in `%State` as i64 fields containing `ptrtoint` of their allocas

This is the same model as the current `ListLiteral` representation — pointers embedded in i64 state slots. LLVM's mem2reg and SROA will handle promotion to registers.

### Non-Regression Guarantee
- Recursive types do not appear in any existing benchmark
- Implementation is additive — new code paths for recursive construction/destruction
- Existing flat-field optimizations (dead-field elimination, float promotion) operate at the field granularity and don't care about pointer indirection

---

## Benchmark Impact

| Feature | CLBG Benchmark Unlocked | Complexity |
|---------|------------------------|------------|
| Struct + FieldAccess | nbody (struct-like body groupings), knucleotide | Medium |
| Enum (tagged union) | binary-trees, fannkuch-redux | Medium-High |
| ListLen + Slice | fasta (string buffer management) | Medium |
| Runtime-sized allocation | spectral-norm, mandelbrot | Medium-High |
| Tuple/TupleDestructure | Used internally by all above | Low |
| ForAll/Exists | No benchmark, but contract completeness | Low |

### Benchmarks That Work TODAY (No Changes Needed)
| Benchmark | Why |
|-----------|-----|
| nbody | 30 Float fields + 3D vector math — exact same pattern as kalman_filter |
| fasta | RNG via existing `frgn __random` + String output via `frgn __print` + StringBuilder |

nbody and fasta can be written and added to the benchmark suite immediately. They exercise the existing optimization pipeline and provide baselines before the backend improvements.

---

## Implementation Order

The implementation builds on itself — each layer depends on the previous:

1. **Struct codegen** (Phase 1) — foundational. Enum codegen builds on struct representations.
2. **Enum codegen** (Phase 2) — depends on struct codegen for variant field layouts.
3. **ListLen + Slice + MultiSlice** (Phase 3) — independent of struct/enum. Can be done in parallel with Phase 1-2.
4. **Tuple + TupleDestructure** (Phase 4) — trivial once list representation is fixed.
5. **Runtime-sized allocation** (Phase 5) — depends on list representation (Phase 3) + range analysis.
6. **ForAll + Exists** (Phase 6) — depends on range analysis (Phase 5).
7. **Nested recursive types** (Phase 7) — depends on enum codegen (Phase 2).

### Testing Strategy
Each phase includes:
- **Interpreter regression**: Run the interpreter on the new language features — it already handles them
- **LLVM IR verification**: `llc -verify-machineinstrs` catches malformed IR
- **Output equivalence**: Brief output = C reference output for the same inputs
- **Performance parity**: Brief ≥ C (never weaker)

### Existing Tests
- 400 tests pass in `cargo test --lib`
- No existing tests should break — all changes are additive match arms
- New test modules per phase, following the existing patterns in `src/backend/llvm.rs` test section
