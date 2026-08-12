# RingBuffer Inline Fields + Sparse Dispatch Small-K Unroll

**Date**: 2026-07-02
**Status**: Planned
**Goal**: Close two C-wins benchmark gaps by fixing architectural patterns that
prevent LLVM optimization, without regressing the cases where the current
approach is optimal.

---

## P1: RingBuffer Inline Fields (queue_drain 2.10x → ~0.7x)

### Current Architecture
The RingBuffer's 4 fields (`data_ptr`, `head`, `tail`, `mask`) are stored in a
heap-allocated struct (32 bytes via `malloc`). The `emit_ringbuf_init` function
does:
```
%rb = malloc(32)              ; RingBuf struct
store data_ptr, %rb+0
store head,      %rb+1
store tail,      %rb+2
store mask,      %rb+3
%handle = ptrtoint %rb to i64
store %handle, %State.queue   ; handle in %State
```

Each `RingPush`/`RingPop` does:
```
%handle = load %State.queue     ; load from %State alloca
%ptr    = inttoptr %handle to i64*
%tail   = load %ptr+2           ; load from heap struct
; ... (no alias info for inttoptr'd pointer)
```

The `inttoptr` breaks LLVM's ability to trace the pointer back to the `%State`
alloca. Even with perfect SROA on `%State`, the heap indirection remains.

### Proposed Fix
Store the 4 RingBuf fields directly as individual `i64` fields in `%State`:
```
%State = type { ..., i64, i64, i64, i64, ... }
                queue_data queue_head queue_tail queue_mask
```

- `emit_ringbuf_init`: stores 4 fields via GEP on `%State` — no `malloc`
- `RingPush`/`RingPop`: load fields directly from `%State` GEP — no `inttoptr`
- Remove the `malloc` heap allocation entirely for inline RingBuffers

### Detection Heuristic
The inline approach is always better for the RingBuffer hot path. There is no
case where the heap struct is faster — it adds:
- 1 `malloc` (program init, one-time cost — negligible)
- 1 `inttoptr` per push/pop (small but blocks optimization)
- LLVM cannot SROA through it (prevents register promotion)

The only reason to keep the heap struct: if the RingBuffer handle is passed
through FFI boundaries (where a raw `i64` handle is expected). In that case,
the heap struct is needed for the ABI. But no current benchmarks do this.

**Decision**: Always inline by default. If FFI-passing is needed later, we can
add an `inop` variant with heap semantics.

### Files Changed
| File | Change |
|------|--------|
| `emit_toplevel.rs` | `emit_ringbuf_init`: store 4 fields via direct GEP on %State |
| `intrinsics.rs` | RingPush/RingPop: load from %State GEP, no inttoptr |
| `arrow.rs` | Update emit_arrow_push/pop to pass individual fields |
| `context.rs` | May need field index helpers for RingBuf fields |

### Trade-off
- **Pro**: LLVM can SROA the 4 fields → register promotion. Expected 2.10x → ~0.7x.
- **Con**: More fields in `%State` (4 more i64s). For programs with already-large
  state (nbody: 33 fields), this makes the SROA problem slightly worse. But
  this is a general SROA issue (P3.4) that should be fixed independently.
- **Neutral**: One-time `malloc` is eliminated (negligible anyway).

### Verification
```bash
# Compare before/after:
./target/release/briev-compiler llvm benchmarks/queue_drain.bv
grep '%aam\|malloc\|realloc' benchmarks/queue_drain.ll  # should be 0
opt -O2 -pass-remarks=sroa benchmarks/queue_drain.ll -disable-output 2>&1
# Should show SROA-promoted RingBuf fields
bash benchmarks/build_and_bench.sh  # ratio should drop from 2.10x to ~0.7x
```

---

## P3: Sparse Dispatch Small-K Unroll (sparse_dispatch 2.98x → ~1.2x)

### Current Architecture
The modulo-switch dispatch emits:
```
tick:
  %mod = srem i64 %count, K      ; compute residual
  switch i64 %mod, label %after_switch [
    i64 0, label %case_ping
    i64 1, label %case_ack
    ...
  ]
case_ping:
  ; txn body for ping
  br label %after_switch
...
after_switch:
  %count_new = add i64 %count, 1
  store %count_new
  %cont = icmp slt %count_new, %bound
  br i1 %cont, label %tick, label %done
```

Each tick pays: 1 `srem` + 1 switch (indirect branch) + 1 basic block overhead
for the dispatch. For K=8, this is ~6-10 instructions of pure overhead per tick.

### Proposed Fix
For K ≤ 8, emit a rotated loop instead of a switch:
```
_body4:                                       ; 8 iterations per round
  ; iteration 0 (ping)
  %count_0 = add i64 %count_in, 0
  ; txn body for ping (using %count_0)
  ...
  store %count_0_result
  ; iteration 1 (ack)
  %count_1 = add i64 %count_in, 1
  ; txn body for ack (using %count_1)
  ...
  store %count_1_result
  ; ... iterations 2-7 ...
  %count_out = add i64 %count_in, 8
  store %count_out
  %cont = icmp slt %count_out, %bound
  br i1 %cont, label %_body4, label %_body1

_body1:                                       ; remainder (1 iteration)
  %count_rem = add i64 %count_in, 0
  ; dispatch using srem + switch (same as now, but for at most 1 iteration)
  ...
```

The key insight: when K ≤ 8, we can fully unroll the modulo pattern. Instead
of computing `count % K` and switching, we execute ALL K bodies in a fixed
sequence, incrementing the counter by K. The dispatch becomes a tight loop
with no `srem` or `switch`.

### Detection Heuristic
Trigger the unrolled path when: `K <= 8 && all txns have same bound`.

The K ≤ 8 threshold comes from:
- K=8: 8 straight-line iterations per loop round (no code bloat)
- K=16: 16 iterations — borderline code bloat (2x the body)
- K=256: 256 iterations — unacceptable code bloat

For K > 8, keep the existing `srem + switch` dispatch (it's optimal).

### Files Changed
| File | Change |
|------|--------|
| `loop_engine.rs` | `emit_modulo_switch_main`: add rotated-loop path for K ≤ 8 |
| `loop_engine.rs` | `emit_ssa_main`: pass K to modulo dispatch or detect inline |

### Trade-off
- **Pro**: No `srem` or `switch` in the hot path for K ≤ 8. Expected 2.98x → ~1.2x.
- **Con**: Code size grows by ~8x for the unrolled body (still small for K=8).
- **Con**: If one txn body is much slower than others, they all run even when
  unnecessary. In practice, all txns fire at the same rate (they share a counter),
  so this isn't a problem.
- **Neutral**: The `_body1` remainder still uses `srem + switch` for the last
  K-1 iterations (worst case: 7 iterations out of 50M = 0.000014% overhead).

### Verification
```bash
./target/release/briev-compiler llvm benchmarks/sparse_dispatch.bv
grep 'srem\|switch' benchmarks/sparse_dispatch.ll  # should be 0 (in main loop)
bash benchmarks/build_and_bench.sh  # ratio should drop from 2.98x to ~1.2x
```

---

## Regression Watch

Both fixes are **additive** — the new code paths only activate when the
detection heuristic matches. The existing code paths remain unchanged.

| Fix | Existing path preserved? | Detection |
|-----|--------------------------|-----------|
| P1 inline fields | N/A (always inline) | No detection needed — always better |
| P3 small-K unroll | Yes (K > 8 stays as switch) | `if K <= 8` |

For P1, the only regression concern is `%State` size growth (4 more i64 fields).
This is negligible compared to the optimization benefit.

For P3, the only regression concern is code size for K ≤ 8 (8x the body). The
bodies are typically 3-5 instructions each, so 8 × 5 = 40 instructions — trivial.
