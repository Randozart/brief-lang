# Briev GPU Model — Borrowing, Not Barriers

**Date:** 2026-07-15
**Status:** Foundational — thesis intact; reconciled 2026-08-06 for Design A
**Applies to:** GPU backend, SPIR-V emission, parallel execution model

> **Reconciliation (2026-08-06, accel plan):** the thesis — disjointness
> proven at compile time ⇒ no runtime barriers — stands. The mechanism moved
> from the `#gpu` pragma + virtual `[idx < N]` index to the **`accel` keyword
> over a real counter** (Design A, SPEC §9.7): the work-item index is a state
> field (`let i: Int = 0;` + `i = i + 1`), and the counted loop
> `[i < N][i == N]` is proven a disjoint per-`i` map. There IS a syntactic
> marker now (`accel`) — it discloses the offload instead of hiding it behind
> an opaque pragma. See `docs/architecture/gpu-offloading.md`.

---

## The Problem With Other GPGPU Models

CUDA, OpenCL, and Vulkan compute all have a fundamental problem: **the compiler
cannot prove that two work-items don't access the same memory address.** Even
with `thread_id + stride` patterns, the compiler must assume aliasing could
occur. This forces the programmer to insert explicit `__syncthreads()` barriers
between shared-memory reads and writes.

These barriers are:
1. **Error-prone** — forgetting one causes silent data races
2. **Expressive** — they serialize work-items, defeating parallelism
3. **Non-portable** — barrier placement depends on grid configuration

---

## How Briev Eliminates Barriers

Briev's borrowing rules guarantee **data-race freedom by construction**. If a
transaction writes to a variable, no other transaction can read or write it
simultaneously. This is enforced at compile time, not at runtime.

For GPU code, this means:

```briev
// Each work-item has a unique i, guaranteed by the counted-loop counter
// Each write is a slot affine in i — provably non-overlapping
let i: Int = 0;
accel node kernel [i < N][i == N] {
    out[i] = a[i] + b[i];    // per-work-item write, disjoint across i
    i = i + 1;
    term;
};
```

The compiler proves:
- `i` is unique per work-item (the counter advances 0 → N)
- `i` indexes non-overlapping slots for distinct values
- **No two work-items access the same memory location**

Therefore no barrier is needed. The backend simply:
1. Allocates N work-items
2. Gives each the counter value as its index
3. Lets them run independently

**No `__syncthreads()`. No `OpControlBarrier`. Zero synchronization emitted.**

---

## The Transaction-Work-Item Mapping

| Briev construct | GPU meaning |
|----------------|-------------|
| `accel node k [i < N][i == N]` | Work-item map over counter i |
| `[i < N]` precondition | The work-item bound (firing gate) |
| `[i == N]` postcondition | The goal — loop until all work-items done |
| `i = i + 1` | The work-item counter advance |
| `let i: Int = 0;` | Counter starts at 0 |
| `out[i] = ...` | Per-work-item disjoint write (SPIR-V store) |
| `a[i]` read | Shared read (SPIR-V load) |
| `endprogram` | Process exit after the map completes |

The `accel` keyword is the explicit offload marker (SPEC §9.7); the same
counted loop runs natively on CPU (each firing = one work-item). The backend
decides:

| Backend | Work-item dispatch |
|---------|-------------------|
| CPU (LLVM) | The counted loop runs natively (each firing = one work-item) |
| GPU (SPIR-V) | One dispatch of N work-items; the counter is `GlobalInvocationId` |
| CIRCT | Sequential unroll (one work-item at a time) |

---

## Implication: `sync(domain)` Is Not Needed in GPU Code

The `sync(domain)` construct exists for structured concurrency on CPU
(transaction ordering within a domain). On GPU, it is **a no-op** — there
are no shared-memory races to protect against because borrowing rules
prevent them.

This means the GPU backend never emits `OpControlBarrier`. The `sync`
keyword is CPU-only infrastructure.

---

## The GPU Intrinsics

The `#` intrinsics (`Load#`, `Store#`, `GetGlobalId#`, `GetGlobalSize#`,
`GetLocalId#`) exist in the stdlib (`lib/std/gpu.bv`) for explicit
pointer-based kernels. Design A kernels (SPEC §9.7) usually do not need them:
the counted loop's array writes/reads are emitted directly against the
kernel's buffer projection, and the work-item id is bound to
`GlobalInvocationId` internally.

No `Barrier#`. No `WorkgroupSize#`. No `WorkgroupId#`.

---

## What This Means for the Backend

The SPIR-V kernel emission (`src/backend/llvm/kernel.rs`) reuses the LLVM
expression/statement emitter against a kernel-scoped `%State` projection:

1. **Prologue:** `define spir_kernel void @main(ptr %state, i64 %n)` + the
   entry flag/bounds; the work-item id (`get_global_id`) is the counter
2. **Body:** the proven kernel statements emitted via the standard emitter
   (array reads/writes become GEP + load/store on the buffer projection)
3. **Scalars/constants:** read-only inputs; module-local constant globals
4. **Termination:** `unreachable` after the (noreturn) exit

No barrier analysis. No shared memory allocation. No synchronization pass.
The eligibility proof has already done the hard work at the Briev level.
