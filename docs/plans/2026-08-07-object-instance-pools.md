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
