# Borrow-Inspired LLVM Attribute Optimizations

## Status Summary

Recent work (2026-07-04) implemented two borrow-checker-inspired optimization
concepts at the codegen level:
- **`parallel_safe_body`** (`context.rs:287`): Per-field phi loops detect when
  all `&` assignments in a body use independent old values, enabling SIMD
  vectorization without semantic change.
- **`done_needs_fields`** (`context.rs:304`): Per-field liveness for the `done:`
  block, eliminating dead stores for fields not read post-loop.
- **A005d removed** (`mod.rs:2149`): Chunk allocas (≤15 fields per chunk)
  allow per-field phi loops for ALL field counts — SROA decomposes each chunk
  independently, making the memory-based fallback obsolete.

These were implemented. This plan covers the remaining borrow-inspired
opportunities: **richer LLVM attributes** that the compiler's existing analysis
information (read/write sets, contract purity, field liveness, type universe,
Ptr<T> type system) already proves, but does not yet emit.

## Guiding Principles

### Additive Only — Never Modify Existing Paths

Every optimization below emits NEW attribute groups, new metadata, or new
annotations. Existing `#0`–`#6` attribute groups and their associated IR
patterns remain untouched. The `_ => return None;` fallthrough pattern in
optimization-disabled functions is preserved.

The existing loop body emission paths (Path A for zero-body-stores, Path B for
swan-song body stores) are NOT modified. Both paths must continue to work
identically before and after these changes.

### Commenting Mandate

Every code change must include a comment explaining:
```
// 2026-07-04: <what this attribute signals to LLVM>
// <which optimization pass it unlocks>
// <why we can prove the attribute holds in Brief (which analysis provides it)>
// <which other paths exist for the same feature (other attribute groups,
//  other function types) and why each path is chosen>
```

Comments are placed at:
1. The attribute group definition (`mod.rs:2572`)
2. The function signature emission site (`emit_toplevel.rs`)
3. The analysis that proves the attribute (`context.rs` field docs)
4. The dispatch point that chooses which attribute group to use

### Flat Control Flow (Max 2 Levels)

Every function must be ≤2 levels deep. Where attribute selection requires
branching, extract into a named helper:
```rust
// 2026-07-04: Choose the correct attribute group for a @pre_* function.
// Uses #7 (readonly) because @pre_* only reads %State — enables CSE
// of redundant precondition checks. Falls back to #0 for expressions
// with side effects (rare: frgn in precondition).
fn pre_attribute_group(&self) -> &str { ... }
```

### Regression Prevention

Before each change:
1. `cargo test --lib` — all tests pass with the current IR
2. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks produce
   correct output
3. Verify the generated `.ll` file contains the new attributes
4. Run benchmark suite and compare ratios against pre-change baseline

After each change:
1. Verify no dead backends (verilog.rs, vhdl.rs, c.rs, rust.rs, cobol.rs,
   x86_64.rs, aarch64.rs, wasm.rs, tcl_generator.rs) are affected. If a shared
   API change propagates mechanically, use `#[allow(unused_variables)]`,
   `_ => {}`, or `todo!()` with a comment `// dead backend`.
2. Check that the new attributes appear in `opt -O3 -pass-remarks-missed=...`
   diagnostics as expected (fewer missed SROA/GVN/vectorization opportunities).

---

## Optimization 1: `readonly` on `@pre_*` Functions

### Problem

Every `@pre_*` function reads `%State` but never writes it. Currently they all
use attribute group `#0` (`mustprogress nofree norecurse nosync nounwind willreturn`),
which does NOT include `readonly` or `memory(readonly)`. LLVM must conservatively
assume `@pre_*` may write to memory, preventing:

- **CSE**: Destroys common subexpression information (e.g., both an SSA loop and
  a memory loop dispatch path call `@pre_*` with the same state — LLVM cannot
  merge the two calls).
- **Load hoisting**: Loads before a `@pre_*` call cannot be reordered past it.
- **Dead store elimination**: Stores before a `@pre_*` call cannot be eliminated
  because LLVM cannot prove the store is dead across the call.

### Fix

Add a new attribute group `#7` with `memory(readonly)` and use it for `@pre_*`
function signatures:

```llvm
attributes #7 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(readonly)
}
```

In `emit_pre_function` (`emit_toplevel.rs:1525`), replace `#0` with `#7`:
```llvm
define internal i1 @pre_{name}(ptr noalias nocapture align 8 %state) #7 {
```

### Safety (Proof)

The `@pre_*` function body is compiled from a Brief precondition expression
(`txn.contract.pre_condition`). Precondition expressions:
- Cannot contain `Statement::Assignment` (no `&x = ...`)
- Cannot contain arrow mutations (`<- push`, `<- pop`)
- Cannot call functions with side effects (BILD/inop bodies are excluded)
- Only read state via `Expr::Identifier` → GEP+load from `%state`

### Code Paths

| Path | Attribute Group | When Selected |
|------|----------------|---------------|
| Current | `#0` (no readonly) | All `@pre_*` functions |
| New | `#7` (with `memory(readonly)`) | All `@pre_*` functions (preconditions are always read-only) |

No runtime dispatch needed — every `@pre_*` function gets `#7`.

### Impact

Lets LLVM:
1. **CSE**: If two dispatch paths call `@pre_same` with the same `%state` data,
   LLVM merges them: `%a = call @pre_same(ptr %state); %b = call @pre_same(ptr %state)`
   → `%a = call @pre_same(ptr %state); %b = add i1 %a, 0` (GVN eliminates `%b`).
2. **Load hoisting**: Any load emitted before a `@pre_*` call is movable past it.
3. **DSE**: Dead stores before a `@pre_*` call are eliminated.

For programs with multiple dispatch paths (SSA + memory + folded checks),
this eliminates redundant precondition evaluation at compile time.

---

## Optimization 2: `argmemonly` on Function Attribute Groups

### Problem

Brief functions only access memory through pointer arguments (`%state` and,
for GPU kernels, `%in_buf`/`%out_buf`). They never read or write global
memory, memory returned from `malloc`, or memory reachable through non-pointer
global values. However, the current attribute groups use `memory(readwrite)`,
which tells LLVM "may read/write any memory anywhere."

LLVM's `argmemonly` attribute says "this function only accesses memory that
is pointed to by its pointer arguments." This is a weaker (and therefore
more optimizable) constraint than `memory(readwrite)`.

Without `argmemonly`:
- Calls act as barrier to all memory operations, not just `%state` operations
- Stack allocas that are proven not to escape to the called function cannot be
  promoted past the call
- Two independent `%state`-only calls cannot be reordered past unrelated loads

### Fix

Add `memory(argmemonly: readwrite)` variant attribute groups for functions
that access memory exclusively through `%state`.

New groups:
```llvm
attributes #8 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(argmemonly: readwrite)
}
attributes #9 = {
    nofree norecurse nosync nounwind memory(argmemonly: readwrite)
}
attributes #10 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(argmemonly: readonly)
}
```

Assignment:
- `#8` → replaces `#0` for definitions that only access `%state` (all of them)
- `#9` → replaces `#3` for `@main` (only accesses `%state` through calls)
- `#10` → for `@pre_*` combined with `memory(readonly)` and `argmemonly`

### Safety (Proof)

Brief's execution model:
- No global variables (except `@link` triggers, which are `load volatile`)
- Arena allocator (bump pointer returned from `malloc`, not global state)
- All state is in `%State` struct, passed as function parameter
- Collection operations (`<- push`, `<- pop`) operate on state fields or
  temporaries — never on globals

Exception: `@link` triggers are `external global` which are read via
`load volatile`. These are NOT accessed through `%state`. But `argmemonly`
says "only accesses memory through pointer args" — since `@link` triggers
are accessed through their global address (not through `%state`), this would
be violated if the function reads triggers.

**Decision**: Do NOT apply `argmemonly` to functions that read `@link` triggers.
Only apply to:
- `@defn` functions (no trigger access)
- `@callable_txn` functions (no trigger access)
- `@pre_*` functions (no trigger access)

Reactive txns (`@reactor_tick`, `@async_body_*`) may access triggers and
should NOT get `argmemonly` — they keep `memory(readwrite)`.

### Code Paths

| Path | Attribute Group | Functions |
|------|----------------|-----------|
| Current | `#0` (`memory(readwrite)`) | All definitions, pre_*, callable txns |
| New | `#8` (`memory(argmemonly: readwrite)`) | Definitions and callable txns |
| New | `#10` (`memory(argmemonly: readonly)`) | `@pre_*` functions (combines with readonly) |
| Unchanged | `#2` (`memory(readwrite)`) | `@reactor_tick` (may access triggers) |
| Unchanged | `#3` (`memory(readwrite)`) | `@main` (may access triggers via reactor tick) |

Dispatch: check `self.ctx.link_triggers.is_empty()` (or a per-function flag)
to select `argmemonly` vs full `memory(readwrite)`.

### Impact

LLVM gains:
1. **Alloca promotion** across call boundaries: `alloca i64` proved local to
   the calling function can be promoted through `@pre_*` calls.
2. **Load/store reordering**: Independent memory operations that don't go
   through `%state` can be reordered past `argmemonly` calls.
3. **Redundant load elimination**: If two `argmemonly` calls don't alias with
   non-`%state` loads, those loads are CSE-able across the call.

---

## Optimization 3: `readonly`/`nocapture` on Scalar Function Parameters

### Problem

Function parameters beyond `%state` get zero LLVM attributes:
```llvm
define i64 @brief_main(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1)
```

These parameters:
- Are never written to (Brief has no mutable parameters — `&x = ...` only mutates
  state fields, not parameters)
- Never escape (no pointer parameters at all for defns — all scalars)
- Are used at most once (no aliasing concerns)

Without attributes, LLVM conservatively assumes every call may write to
parameters (through invisible references) and may capture them.

### Fix

Emit `readonly` on every non-`%state` parameter for definitions and callable
transactions:

```llvm
define i64 @brief_main(ptr noalias nocapture align 8 %state,
    i64 nofree nosync readonly %arg0,
    i64 nofree nosync readonly %arg1)
```

### Safety (Proof)

- `defn` parameters are immutable: the function body is a pure expression
  tree with no `&` assignments to parameters.
- `txn` parameters are immutable: only state fields can be mutated via `&`.
- Parameters are scalar `i64` values: no pointer indirection, no aliasing.

Exception: `Ptr<T>` parameters (opaque pointer addresses carried as `i64`) are
also immutable at the Brief level — the function cannot modify the pointed-to
memory except through explicit `volatile_store#(ptr, val)` intrinsics, which
are side-effect calls, not parameter mutation.

### Code Paths

| Path | Parameter Attributes | When Selected |
|------|--------------------|---------------|
| Current | None | All defn/txn parameters |
| New | `nofree nosync readonly` | All non-`%state` parameters of defns and callable txns |
| Unchanged | None | Reactive txn parameters (reactive txns have no extra params) |

No runtime dispatch — every defn/txn non-`%state` parameter gets these
attributes. The change is in the emission loop at `emit_toplevel.rs:984-986`
and `emit_toplevel.rs:1318-1320`.

### Impact

Enables:
1. **Argument elimination**: If a parameter is unused, LLVM eliminates it
   entirely (requires `readonly` + `nocapture` to prove no observable effect).
2. **GVN across call sites**: Two calls to the same function with the same
   argument values can be CSE'd (requires `readonly` to prove the function
   doesn't modify the argument).
3. **Inlining cost**: Lower penalty for inlining (no hidden writes through
   parameter).
4. **Argument promotion**: A constant argument can be folded into the function
   body (requires `readonly` + `nofree` to prove it's not mutated).

---

## Optimization 4: Per-Field `!invariant.load` Metadata

### Problem

In a per-field phi loop (A005c), every tick reads ALL state fields at the
loop header. But many fields are **read-only** within a given transaction
body — they appear in the `read_set` but not the `write_set`. LLVM does not
know this and must assume every field may be modified by every iteration.

With `!invariant.load` metadata on the phi's initial load from `%State`, LLVM
learns that the load result is invariant for the duration of the loop.

### Fix

In `emit_countable_setup_phis_and_header` (`loop_engine.rs:966`), when loading
initial field values from `%State`, check if the field is in the write set.
If not, emit `!invariant.load` metadata on the load:

```llvm
; non-write-field load:
%init_f = load i64, ptr %gep_f, align 8, !invariant.load !{}
```

For write-set fields (read-write or write-only), omit the metadata — the
load is not invariant because the loop body may modify it.

### Implementation

1. Add `write_set: &HashSet<String>` parameter to
   `emit_countable_setup_phis_and_header`.
2. In the initial-load loop (line 981-986), check `!write_set.contains(name)`:
3. Update the call site in `emit_countable_main` to pass `write_set`.

### Impact

For benchmarks with many read-only fields:
- **nbody_newton**: 31 fields, ~30 read-only per iteration. Each loads with
  `!invariant.load` → LLVM's LICM hoists them out of the loop entirely
  (they become loop-invariant). The phi carries the initial value for all
  iterations with zero subsequent memory traffic.
- The main gain is for Path B loops (swan song paths) where stores are still
  emitted — the read-only fields' initial loads become hoistable out of the
  loop, eliminating one load per read-only field per iteration.

---

## Optimization 5: `!alias.scope` for Async Transaction Isolation

### Problem

The proof engine (`proof_engine.rs:2951`) verifies that async transactions
have non-overlapping write sets. But LLVM does not know this — both async
txns receive the same `%state` pointer, so LLVM conservatively assumes they
may alias.

### Fix

Emit `!alias.scope` and `!noalias` metadata on concurrent async calls:

```llvm
call void @async_body_A(ptr noalias nocapture %state_A), !alias.scope !{!7}, !noalias !{!8}
call void @async_body_B(ptr noalias nocapture %state_B), !alias.scope !{!8}, !noalias !{!7}
```

Only emitted when:
1. Multiple async txns run in parallel
2. The proof engine confirmed no read/write overlap
3. Neither txn accesses `@link` trigger globals

---

## Optimization 6: `!noalias` Between Ptr<T> Accesses and `%State`

### Problem

`Ptr<T>` values are opaque integers (`i64`) in Brief's type system. When used
with `volatile_load#(ptr)` or `volatile_store#(ptr, val)`, the backend emits
an `inttoptr i64 %addr to ptr` to get an LLVM pointer, then loads/stores
through it (`expr/intrinsics.rs:1884,1930`).

The resulting LLVM pointer has **no aliasing relationship** with the `%state`
pointer. LLVM conservatively assumes it MAY alias `%state` — so a volatile
load through a `Ptr<T>` is a barrier to all `%State` loads and stores,
preventing:
- **Load reordering**: A `%State` field load after a `volatile_load#` cannot
  be hoisted before it.
- **Store sinking**: A `%State` field store before a `volatile_store#` cannot
  be sunk past it.
- **Dead store elimination**: A dead `%State` store cannot be eliminated if a
  `volatile_load#` follows.

### Fix

Add `!noalias !{!StateScope}` metadata on the `volatile_load`/`volatile_store`
instruction and define a TBAA scope for `%State`:

```llvm
%ptr = inttoptr i64 %addr to ptr
%val = load volatile i64, ptr %ptr, !noalias !{!StateScope}
```

### Safety (Proof)

`Ptr<T>` values are opaque integer addresses. They are:
- Created by `inttoptr` from contract-proven integer literals or `__rt_` FFI
  calls — NEVER from `%State` pointers (no `ptrtoint %state` in the backend)
- Passed through `i64` registers, not LLVM pointers

Therefore, a `Ptr<T>` value can NEVER alias with any `%State` field.

### Code Paths

| Path | Metadata | When Selected |
|------|----------|---------------|
| Current | None | All `volatile_load#` / `volatile_store#` |
| New | `!noalias !{!StateScope}` | Every volatile memory access through Ptr<T> |

---

## Optimization 7: Cast-Unwrapping for `ptr_field as Int` Range Metadata

### Problem

`Ptr<T>` fields in contracts are written as `ptr_field as Int` (an
`Expr::Cast` wrapping the identifier). But `extract_ranges` (dispatch.rs:140)
and `emit_precondition_check` (emit_toplevel.rs:1482) only match bare
`Expr::Identifier(name)`. The contract falls through to `@llvm.assume`
instead of producing `!range` metadata.

### Fix

Add cast-unwrapping to `extract_ranges_inner` and `emit_precondition_check`:

```rust
fn unwrap_cast_to_ident(e: &Expr) -> Option<String> {
    match e {
        Expr::Cast(inner, Type::Int) => match inner.as_ref() {
            Expr::Identifier(n) => Some(n.clone()),
            _ => None,
        },
        Expr::Identifier(n) => Some(n.clone()),
        _ => None,
    }
}
```

### Impact

For programs using Ptr<T> with address-range contracts:
- `[uart_ptr as Int >= 0x3F8 && uart_ptr as Int < 0x400]` now produces
  `!range {0x3F8, 0x400}` — LLVM can optimize bounds checks.
- Non-Ptr fields continue to work identically (the `Expr::Identifier` match
  path is unchanged).

---

## Optimization 8: `dereferenceable(N)` for Ptr<T> Parameters

### Problem

When a function receives a `Ptr<T>` parameter (passed as `i64`), LLVM has no
information about the validity or size of the pointed-to memory. Without
`dereferenceable(N)`:
- LLVM cannot speculate a load through the pointer before the bounds check
- LLVM cannot hoist loads through the pointer out of loops
- LLVM inserts unnecessary null checks (even though Brief pointers are never
  null)

### Fix

On Ptr<T> function parameters, emit `dereferenceable(N)` where `N` is the
pointee byte size from the type universe:

```llvm
; For fn(ptr: Ptr<UInt8>) — dereferenceable(1) from pointer_pointee_layout:
define i64 @fn(ptr noalias nocapture align 8 %state,
    i64 nofree nosync readonly dereferenceable(1) %arg0)
```

### Safety (Proof)

The type universe provides `pointer_pointee_layout()` (type_universe.rs:771)
which returns `(bytes, alignment)` for `Ptr<T>` types. This gives a minimum
dereferenceable size from the type alone.

Brief guarantees:
1. Ptr<T> values come from contract-proven sources (hardware addresses,
   arena allocations, FFI returns)
2. The arena allocator is scoped to the tick (reset each loop iteration)
3. Hardware MMIO addresses are statically valid

### Code Paths

| Path | `dereferenceable` | When Selected |
|------|-------------------|---------------|
| Current | None | All Ptr<T> parameters |
| New | `dereferenceable(pointee_bytes)` | Ptr<T> parameters with universal type info |
| Unchanged | None | Non-Ptr parameters (scalars, collections) |

---

## Optimization 9: Universe-Driven `!range` for Narrow Types

### Problem

The type universe knows the byte size of every type. For narrow types like
`UInt8` (1 byte, range `[0, 256)`), the compiler could automatically emit
`!range {0, 256}` on field loads — without any contract precondition. This
is information the type system provides for free.

Currently, `!range` metadata is driven solely by contract preconditions
(`dispatch.rs:132-145`).

### Fix

After extracting contract-driven ranges, add type-driven ranges for narrow
types by querying the universe for the field's type byte size:

```rust
for (field_name, &idx) in &self.ctx.field_index_map {
    let ty = &self.ctx.field_types[idx];
    if let Some((lo, hi)) = TypeUniverse::type_range_for_llvm_type(ty) {
        let mi = range_meta.len();
        range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, lo, hi));
        self.ctx.field_to_meta_idx.insert(field_name.clone(), mi);
    }
}
```

### Impact

- **UInt8 fields**: `!range {0, 256}` eliminates bounds checks
- **UInt16 fields**: `!range {0, 65536}` eliminates bounds checks
- **Char fields**: `!range {0, 1114112}` lets LLVM optimize character ops
- **No contract needed**: Pure type-driven — fires for every field

---

## Optimization 10: Universe-Driven `align_of()` and `ensure_typed_value()`

### Problem

Two key LLVM backend functions use hardcoded matches instead of querying the
type universe:

**`align_of()`** (`emit_toplevel.rs:469-477`): Hardcoded to return 8 for
unknown types, which is wrong for custom types with smaller alignment.

**`ensure_typed_value()`** (`emit_stmt.rs:120-147`): Does NOT query the
universe's `unbox_op` — relies on hardcoded LLVM type string matching.

### Fix

**Universe-driven `align_of()`** — query universe first:

```rust
pub(super) fn align_of(&self, ty: &str) -> u32 {
    if let Some(u) = &self.ctx.type_universe {
        for rt in u.types.values() {
            if rt.llvm_type == ty { return rt.alignment as u32; }
        }
    }
    match ty { "i64" | "double" => 8, "float" | "i32" => 4, ... }
}
```

**Universe-driven `ensure_typed_value()`** — use `unbox_op` from the
universe instead of hardcoded LLVM string matching.

---

## Already Implemented (Summary)

| Feature | Location | What It Does |
|---------|----------|-------------|
| `parallel_safe_body` | `context.rs:287`, `emit_stmt.rs:59-68` | Keep old phi regs when each field is mutated at most once — enables SIMD |
| `done_needs_fields` | `context.rs:304`, `emit_stmt.rs:52-57` | Only store fields that swan-songs read |
| `needs_state_stores_in_body` | `context.rs:263`, `loop_engine.rs:1246` | Path A (no stores) vs Path B (stores for swan song) |
| A005d removal | `mod.rs:2149` | Per-field phi loop for ALL field counts |
| TBAA metadata | `mod.rs:2600-2617` | Per-type alias analysis for Int/Bool/Float in %State |

---

## Implementation Order (Executing Now)

1. **`readonly` on `@pre_*`** — new `#7` group, `memory(readonly)` on pre_ functions
2. **`readonly` on defn/txn scalar params** — `nofree nosync readonly` on every `%argN`
3. **`!noalias` between Ptr<T> and `%State`** — `!noalias !{!StateScope}` on volatile accesses
4. **Cast-unwrapping for `ptr_field as Int`** — range metadata for Ptr<T> contracts
5. **Universe-driven `!range` for narrow types** — automatic range from type byte size
6. **`dereferenceable(N)` for Ptr<T> params** — from `pointer_pointee_layout()`
7. **`argmemonly` attribute groups** — `#8`/`#9`/`#10` for defns/pre_/main
8. **Universe-driven `align_of()` and `ensure_typed_value()`**
9. **Per-field `!invariant.load`** — read-only field optimization
10. **`!alias.scope` for async txns** — memory barrier elimination
