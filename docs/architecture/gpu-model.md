# Briv GPU Model — Borrowing, Not Barriers

**Date:** 2026-07-15  
**Status:** Foundational  
**Applies to:** GPU backend, SPIR-V emission, parallel execution model

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

## How Briv Eliminates Barriers

Briv's borrowing rules guarantee **data-race freedom by construction**. If a
transaction writes to a variable, no other transaction can read or write it
simultaneously. This is enforced at compile time, not at runtime.

For GPU code, this means:

```briv
// Each work-item has a unique idx, guaranteed by precondition
// Each access is idx * stride — provably non-overlapping
txn kernel [idx < N][idx >= N] {
    Store#(out + idx * 8, Load#(a + idx * 8) + Load#(b + idx * 8));
    term;
};
```

The compiler proves:
- `idx` is unique per work-item (precondition `[idx < N]`)
- `idx * 8` gives non-overlapping addresses for distinct `idx` values
- **No two work-items access the same memory location**

Therefore no barrier is needed. The GPU backend simply:
1. Allocates N work-items
2. Gives each an `idx`
3. Lets them run independently

**No `__syncthreads()`. No `OpControlBarrier`. Zero synchronization emitted.**

---

## The Transaction-Work-Item Mapping

| Briv construct | GPU meaning |
|----------------|-------------|
| `txn kernel [idx < N][done]` | Work-item with unique idx |
| `[idx < N]` precondition | Work-item is active |
| `[idx >= N]` postcondition | Work-item terminates |
| `Load#(ptr)` | SPIR-V load instruction |
| `Store#(ptr, val)` | SPIR-V store instruction |
| `GetGlobalId#(dim)` | SPIR-V `GlobalInvocationId` |
| Term on convergence | Work-item completion |

There is **no syntactic difference** between a GPU kernel and a CPU parallel
loop. The same code compiles for both backends. The backend decides:

| Backend | Work-item dispatch |
|---------|-------------------|
| CPU (LLVM) | Parallel for-loop with thread pool |
| GPU (SPIR-V) | `OpDispatch` with N work-items |
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

## The Complete GPU Intrinsic Set

Only four `#` intrinsics are needed for GPU programming:

| Intrinsic | Purpose | GPUs without |
|-----------|---------|--------------|
| `Load#(addr, bytes)` | Read memory | CUDA: no intrinsic needed |
| `Store#(addr, val, bytes)` | Write memory | CUDA: `atomic_store` only |
| `GetGlobalId#(dim)` | Work-item index | CUDA: `blockIdx * blockDim + threadIdx` |
| `GetGlobalSize#(dim)` | Grid extent | CUDA: `gridDim * blockDim` |
| `GetLocalId#(dim)` | Local index within group | CUDA: `threadIdx` |

No `Barrier#`. No `WorkgroupSize#`. No `WorkgroupId#`. All derived values
are computed in Briv from the four primitives above.

---

## What This Means for the Backend

The SPIR-V backend (when implemented) has a trivial job:

1. **Prologue:** Emit `OpEntryPoint`, `OpVariable` for idx
2. **Body:** Emit each transaction as a SPIR-V function
3. **Load#:** `OpLoad` with pointer operand
4. **Store#:** `OpStore` with pointer and value operands
5. **GetGlobalId#:** `OpLoad` from `GlobalInvocationId` built-in
6. **Termination:** `OpReturn` on convergence

No barrier analysis. No shared memory allocation. No synchronization pass.
The borrowing rules have already done the hard work at the Briv level.
