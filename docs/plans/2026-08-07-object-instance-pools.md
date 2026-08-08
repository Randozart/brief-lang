# Object Instance Pools — the SoA representation (architecture decision)

**Date:** 2026-08-07 · **Status:** Decided (approved) · **Ties to:** SPEC §9.5,
§12.2, §11.4; Phase 7 (const generics) + Phase 10 (spawn/await)

## The decision

**An `obj` is a naming convenience over a bundle of variables and methods. Its
instances are represented as structure-of-arrays pools unpacked into the
top-level reactor state.** Each instance is an integer id; `spawn` picks a free
id; the handle is the id; member access lowers to `MemberPool[self_id]`; member
reactive nodes operate over the pool. The current boxed model (a %State slot
holding a heap/stack struct address, members behind self-slot offsets) is
retired.

This follows the user's design intent ("obj in Briv are unpacked into top level
cross-bound values") and the spec: §9.5 "an object owns identity, lifecycle,
logical state, ports, and reactive behavior **in its parent reactor**"; §12.2
"spawn creates a persistent task or component instance and returns a linear
owned handle."

## Rationale

The boxed model proved to be an integration island: the scheduler, liveness
(`apply_field_modes`), SROA, masking, and foreach all operate on top-level
%State slots and never see boxed members. It produced the bug class we hit —
the `%Box` struct-type collapse, direct-read address resolution, member-call
segfaults (verified pre-existing at the baseline). The SoA pool puts the
members IN the top-level state, so every machinery path works uniformly.

- **Deterministic + efficient**: fixed capacity → static state layout → static
  reactive nodes. Contiguous member arrays SROA cleanly; matches the existing
  `soa_reorder` (structure-of-arrays) philosophy.
- **Reactive precision**: the scheduler tracks the member arrays; per-instance
  deps (`visible[3]` changed) are a frontend-computable refinement.
- **Cross-boundary**: an instance crossing an FFI/GLUE boundary is its id +
  member arrays — value-like, not an opaque pointer.
- **No objs are passed as values today** (verified: no defn/frgn takes an
  obj-typed param; the collection ops name the instance). Objs are top-level
  reactor units; `struct` remains the passable data-container value type.

## Design

### Representation

```
obj Button {
    visible: Bool;
    label: String;
    node render()[visible][!visible] { ... };
};
```
becomes top-level state for the pool:
```
%State:
  Button.visible : Bool[MAX]      # pool column (SoA)
  Button.label   : Ptr[MAX]
  Button.live    : Bool[MAX]      # free-list / liveness column
```
Each instance is an **id** (0..MAX-1). Member access `self.visible` lowers to
`Button.visible[self_id]`.

### Capacity

Fixed compile-time capacity, declared via the const-generic machinery already
shipped: `obj Button<MAX> { ... }` → the pool columns are `visible: Bool[MAX]`.
Capacity can also come from a top-level const. A `spawn` beyond capacity is a
compile-time/runtime "pool exhausted" error (deterministic, no re-layout).

### Top-level `let b: Box = 0`

Is **instance 0** of the pool — one representation for both static top-level
instances and spawned ones. The `let` binds the instance name to id 0; member
access `b.data` lowers to `Box.data[0]`.

### Member reactive nodes

For the first iteration, an obj's member node becomes a static node that fires
on its member-array deps and processes the changed/live ids (batch semantics).
Per-instance dep refinement is a later frontend-driven-dispatch optimization.

### `spawn` / `await` / `free` / `keep` (SPEC §12.2)

- `let h = spawn Button(label, visible)` → allocates the next free id,
  initializes the instance's member slots, returns **the id as the linear
  owned handle**.
- `await h` / `free h` / `keep h` manage the handle's lifetime; `free` zeroes
  the member slots + returns the id to the pool; the linear-ownership rules
  (§12.2) are enforced by effect/ownership analysis.

### Member self-resolution

Inside a member body, `self.visible` (and bare member names in a member
context) resolve to `Button.visible[self_id]`, where `self_id` is the instance
context (id 0 for a top-level `let`, or the handle for a spawned call).

## Migration phases

1. **Instance state building**: `build_field_index` unpacks an Applied-typed
   top-level let into prefixed member slots (substituting const dims — the
   `resolve_field_type` substitution already shipped). Member-array members use
   the top-level Vector arm (the `[5 x i64]` layout that already works for
   state arrays).
2. **Member access**: `b.data`/`b.data[i]` resolve through the standard
   top-level field-index paths (already proven working for arrays). The boxed
   self-slot + `%Box` struct + direct-read paths are retired for instances.
3. **Member nodes**: member `txn`/`node` bodies resolve bare member names
   against the instance's slots; the reactive node emission iterates the pool.
4. **Op bindings** (`<-` InsertAt/ExtractFrom): the operand's instance name
   binds the self; the op body accesses the prefixed slots.
5. **`spawn`/`await`/`free`/`keep`**: the id-pool allocator + the linear
   handle ops (§12.2), with ownership analysis.
6. **Retire the boxed obj-instance machinery** for instances (the `%Box`
   struct type, the instance-address slot, `self_binding` offsets); `struct`
   values keep the struct-literal machinery.

## Relationship to existing machinery

- **Const generics / multi-dim** (Phase 7, shipped): pool capacity + the
  member-array layout reuse `Type::Vector` + the const-param substitution.
- **`apply_field_modes` / liveness**: the pool columns are ordinary fields —
  unreferenced columns prune naturally.
- **`soa_reorder`**: the pool IS the SoA layout by construction.

## Open refinements (later)

- Per-instance reactive dep precision (frontend-driven dispatch).
- Growable pools (re-layout) — deferred; fixed capacity first.
- Obj-as-value (passing an instance handle across boundaries) — the handle (id)
  is the portable unit.

## Migration progress 2026-08-07 (late): phases 1-4 shipped

The unpacked single-instance representation works end-to-end:

- **Instance state building**: build_field_index unpacks a top-level Applied
  obj let into prefixed member slots (`st.data`, `st.len`) with the const
  args substituted (M → 5 → `[5 x i64]`). A pre-pass seeds struct_types +
  obj_type_params before build_field_index. Unpacked slots are ALWAYS-live
  (the field-liveness scan does not walk member bodies yet).
- **Init**: obj_instance_inits records the instance + its init expr; the Init
  member runs against the prefixed slots during BOTH init_state and the
  inline init stores (self_prefix = the instance name).
- **Member access**: Field/Identifier resolution routes `b.total`/`b.data` (and
  bare member names in member bodies) through the standard %State slot paths;
  array members return the slot's array ptr (indexed via the row-view path).
- **Member calls + ops**: emit_method_call / emit_strategy_member_call pass the
  instance prefix (MemberInvocation) so member bodies resolve bare names to
  the slots; the member-result term capture now fires for the prefix path.
- **Typechecker**: resolve_field_type substitutes member const dims against the
  instance args.
- Verified: 1656 tests, 75 MATCH + 1 PASS (ring_buffer / queue_drain /
  stack_push_pop / hash_ops all unpacked + correct), the Box end-to-end
  (b.total=1, b.data writes, b.set member call, op dispatch).

REMAINING: spawn/await/free/keep (id-pool allocator + linear handles, §12.2),
the SoA pool dimension (capacity), member-body liveness wiring, retiring the
boxed %Box/self-slot paths for instances.

## spawn infrastructure checkpoint 2026-08-07

`spawn Obj(args)` (SPEC §12.2) is wired end-to-end: lexer token, Expr::Spawn,
parser prefix, walker arms, the typechecker (types to Custom(type_name)), the
per-BASE pool columns (static + spawned share `{base}.{member}`), the
__spawn_next_<base> allocator counter, and the handle-aware member access
(Field/Identifier/Assign GEP the column at the handle's row; member calls
resolve a spawned handle local via instance_prefix_for). Committed d8e62e5b.
Verified: 1656 tests + 75 MATCH + 1 PASS (no regression; the Box end-to-end
= 51 on the per-base columns).

OPEN: a spawned instance's runtime member calls crash under -O3 -flto — the
inlined `h.inc()` bodies get UNROLLED by LLVM into 4 writes that advance past
the `[2 x i64]` column (addq $0x2, 0x1(%rdx)..0x3(%rdx) on a 2-element
column) inside the countdown loop. The generated IR is correct (GEPs at the
handle row); the miscompile is the loop vectorizer touching beyond the fixed
column. Next: disable the unroll for the member-body emission OR re-check the
loop-shape analysis's handling of the inlined member writes.

## Predictably-inexhaustible pools 2026-08-07 (late)

Adopted the principle: Briv has NO runtime errors — a spawn pool is PROVABLY
inexhaustible. The `obj_instance_capacity` config default is removed; the
capacity is DERIVED by a new frontend analysis:

- `src/analysis/spawn_pool.rs`: computes the maximum concurrent live instances
  per obj base (spawns minus frees, weighted by the enclosing foreach const
  range length and a reactive node's countdown firing count — `[ticks < N]`
  with a compile-time N fires N times).
- The member columns are sized to `live + 1` (row 0 is the static instance).
- An UNPROVABLE spawn (a runtime-bound loop or a never-converging node) is a
  COMPILE ERROR ("cannot statically bound..."), surfaced in both `build` and
  `check` like the termination analysis.
- AnalysisResults.spawn_pools + ctx.spawn_pools thread the proven capacities.

Verified: 1658 tests (2 spawn_pool tests: const countdown bounded, runtime
bound rejected) + 75 MATCH + 1 PASS (no regression).

OPEN (unchanged): the spawned instance's runtime member calls still crash
under -O3 -flto — LLVM unrolls the inlined h.inc() bodies into 4 writes that
advance past the column inside the countdown loop (independent of the
capacity). NEXT: address the unroll interaction.
FOLLOW-UP: the runtime-sized dependent capacity buffer for runtime-bound
spawn loops (§16.6), and cells.

## Runtime-bound dependent pools 2026-08-07 (final)

The runtime-bound spawn loop is no longer a compile error — it is a
DEPENDENT pool sized at runtime:

- `src/analysis/spawn_pool.rs`: `Firing` becomes `Static(i64)` (const
  countdown) | `Dependent(Expr)` (countdown bound is a runtime field / named
  const) | `Unprovable` (`[true]` node + spawn = error). Analyze returns
  `(capacities, dependent_terms, errors)` — `DependentTerm { multiplier,
  bound }` carries the countdown bound (nested Mul products for enclosing
  runtime-bound foreachs). 4 analysis tests.
- Backend `ctx.heap_columns: HashMap<usize, String>`: index → member-row
  LLVM type, ONLY for dependent bases. A dependent column is registered as an
  i64 SLOT holding the heap-buffer address, not a `[capacity x T]` array.
- `emit_dependent_pool_buffers` (emit_toplevel.rs): at program init, AFTER
  the bound field stores (e.g. `N = get_env_int!`) and BEFORE the static
  instance's row-0 writes, malloc each dependent base's columns to
  `(static_rows + Σ multiplier×bound) * elem_size`, and store the address in
  the slot. Provably inexhaustible: the buffer holds the sum of all proven
  runtime bounds; the allocator counter starts at row 1.
- Member read + write (self-prefix / emit_instance_column_row / Assign): if
  the column is dependent, load the slot address, inttoptr, GEP the row
  inside the buffer (element type from heap_columns). Static columns keep the
  `[capacity x T]` GEP unchanged.
- Verified: 1662 lib tests (new: spawn_pool dependent cases +
  `test_dependent_spawn_pool_heap_buffer` asserting malloc + slot round-trip +
  no static column) + scratch/spr.bv with BOUND=5 prints `22222` (each
  spawned row: Init→0, inc×2→2) under `-O3 -flto`. Benchmark samples
  (enemy_swarm .79x, linked_list .72x) unchanged — no regression.
- Committed 58f89b02.

OPEN: cells (spawn/await/keep/free lifecycle beyond the row allocator). →
See `docs/plans/2026-08-08-pool-lifecycle-free-keep-await.md` for the
lifecycle plan (three capacity-corruption bugs found: free keys by var name,
cross-node capacity is max-but-allocator-is-monotonic, dependent buffer
off-by-one; then await + free-list reclamation).
