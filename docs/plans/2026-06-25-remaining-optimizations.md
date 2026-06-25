# Remaining Optimization Work — Cell + Wire + Performance

**Date:** 2026-06-25
**Status:** Plan (awaiting execution)

Supersedes `docs/plans/2026-06-24-cell-deferred-items.md` (now historical).
Supersedes `docs/plans/2026-06-11-remaining-gaps.md` (now historical).

---

## Current baseline

- **1288 tests passing**, 0 failing
- Cell-to-cell wires working in interpreter + LLVM (non-threaded only)
- Two-pass convergence loop fixes HashMap ordering bug
- `is_wake = true` default with `#nowake` modifier implemented
- Timing watchdog Phases 1-5 complete
- Temporal fallback (`~?` operator) complete

---

## Item 1 — Sub-State GEP Optimization

**Files:** `src/backend/llvm/mod.rs`, `src/backend/llvm/emit_toplevel.rs`, `src/backend/llvm/emit_expr.rs`, `src/backend/llvm/loop_engine.rs`
**Effort:** ~70 lines across 4 files
**Priority:** Medium

### Problem

Persistent cell fields live as flat prefixed slots in `%State`:

```
%State = type { i8, i8, i64, i64, i64, ... }
                ^      ^     ^    ^
                |      |    cell$adder$b
                |      |   cell$adder$sum
                |      cell$adder$done
                boot_done
```

`%State` grows linearly with cell definitions. Cell field GEPs go through the same path as program state — no type-level distinction, LLVM can't alias-analyze them separately.

### Goal

Emit `%CellState.<name>` per cell, pass `%CellState*` to tick functions:

```
%CellState.adder = type { i64, i64 }
%CellState.filter = type { i64, i8 }
```

### Changes

| File | Lines | Change |
|------|-------|--------|
| `mod.rs` | +30 | `build_field_index` emits `%CellState.*` types, skips persistent cell fields in `%State` |
| `emit_toplevel.rs` | +20 | `emit_cell_thread` and `emit_persistent_cell_ticks` use `%CellState.*` param instead of `%State*` |
| `emit_expr.rs` | +10 | CellCall codegen uses separate GEP path for cell fields |
| `loop_engine.rs` | +10 | Main loop allocates `%CellState.*` for cell threads, not `%State` |

### Risks

CellCall convergence loop in `emit_expr.rs` uses `self.state_reg_name` and `self.field_index_map` for GEPs — needs different path for `%CellState.*`. The flat approach works correctly today. This is purely structural (no correctness win, only alias analysis improvement).

---

## Item 2 — Threaded Cell Wire Propagation

**Files:** `src/backend/llvm/emit_toplevel.rs`, `src/backend/llvm/mod.rs`
**Effort:** ~50 lines
**Priority:** Low (blocker: Item 1 is prerequisite)

### Problem

Today, `emit_persistent_cell_ticks` propagates wires only from non-threaded source cells. Threaded cells write outputs to `@chan_val_<cell>_<port>` channel globals, but no code reads those globals and writes the value to the target cell's param slot.

### Goal

After the main loop's channel-drain pass, scan wire targets whose source is a threaded cell. Read `@chan_val_<from_cell>_<from_port>` (volatile load to get latest), then GEP+store into the target cell's slot.

### Changes

| File | Lines | Change |
|------|-------|--------|
| `emit_toplevel.rs` | +30 | After channel-drain in `@reactor_tick`, emit GEP+load from channel global + GEP+store to target |
| `mod.rs` | +20 | Scan wires where `from_cell` is threaded, emit volatile load from channel global |

### Risks

Channel globals hold the LAST value published by the threaded cell's convergence pass. If the threaded cell hasn't published yet (first tick), the main loop reads stale data. This is acceptable — same semantics as non-threaded cells that haven't produced output yet.

---

## Item 3 — Performance: Memcpy Round-Trip SROA

**Files:** `src/backend/llvm/loop_engine.rs`, `src/analysis/dataflow.rs`
**Effort:** ~100 lines
**Priority:** High (direct benchmark impact)

### Problem

Every tick emits 17 separate GEP+load pairs (read old state) + 17 GEP+store pairs (write new state). LLVM's SROA promotes `alloca` but not function arguments (`%State*`). The tick loop operates on a pointer argument, so 17 fields survive as memory operations instead of phi nodes.

Current benchmark gap: fannkuch_redux 1.84x behind C, float_math_nonzero 1.10x behind C.

### Goal

At tick start: `alloca %State` + `@llvm.memcpy` from `%state` → alloca. Operate on alloca via GEP+load+store. At tick end: `@llvm.memcpy` from alloca → `%state`. LLVM inlines the memcpy, SROA sees the alloca, promotes all 17 fields to phi.

**Expected result:** fannkuch 1.84x → ~1.1x.

### Changes

| File | Lines | Change |
|------|-------|--------|
| `loop_engine.rs` | +60 | alloca + memcpy preamble at tick start, memcpy epilogue at tick end. Redirect GEP operations to use alloca instead of `%state` |
| `dataflow.rs` | +30 | Expose read-field set per transaction (for Item 4) |
| `loop_engine.rs` | +10 | Skip prior-state load for write-only fields (Item 4, piggybacks on dataflow change) |

### Ordering with Item 1

Item 1 (sub-state GEP) removes cell fields from `%State`, which means fewer fields survive as memory ops after SROA. The two optimizations are complementary: Item 1 reduces the number of fields in `%State`, Item 3 ensures the remaining fields get promoted. Do Item 1 first, then Item 3.

---

## Item 4 — Prior-State Elision (Minor)

**Files:** `src/analysis/dataflow.rs`, `src/backend/llvm/loop_engine.rs`
**Effort:** ~40 lines
**Priority:** Low

### Goal

Skip `@` prior-state load for fields that are write-only in a given tick. If a field is written but never read (no precondition references it), don't emit the prior-state GEP+load.

**Expected gain:** ~0.5%

Piggybacks on the dataflow changes from Item 3.

---

## Item 5 — CIRCT Transaction Body Synthesis

**Files:** `src/backend/circt.rs`
**Effort:** ~100 lines
**Priority:** Lowest

### Current State

CIRCT backend emits `hw.instance` for `Expr::CellCall` and registers cell fields as `seq.firreg` registers. Transaction bodies are opaque — no `comb.and`/`comb.add` MLIR ops.

### Remaining Work

| Item | Status |
|------|--------|
| `Expr::IntrinsicCall` → `comb` operations | Not started |
| Cell state fields → `seq.firreg` with reset values | Partially done |
| `Statement::Assignment` → combinational wiring | Not started |
| `Contract` precondition → `assert` or `when` guard | Not started |

No priority — CIRCT is the least-active backend. Only worth doing if hardware synthesis becomes active.

---

## Implementation Order

1. **Sub-state GEP** — unblocks cleaner threading, reduces `%State` size, pure refactor
2. **Memcpy round-trip SROA** — highest performance impact (potential 1.84x→1.1x)
3. **Prior-state elision** — small gain, piggybacks on Item 2 dataflow
4. **Threaded wire propagation** — requires Item 1 for clean layout
5. **CIRCT body synthesis** — lowest priority

Items 1 and 2 are independent of each other and could be parallelized.
Item 4 depends on Item 1. Items 3 and 5 are independent of everything.
