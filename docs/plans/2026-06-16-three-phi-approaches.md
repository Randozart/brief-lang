# Three φ Approaches — Plan for 2026-06-16

## Problem

The folded SSA loop path in `loop_engine.rs::emit_folded_loop` currently uses
`load %State, %State* %slot` / `store %State, %State* %slot` round-trips. This
prevents SROA from decomposing `%State` into scalar registers — every field
access goes through the alloca.

Replacing the alloca slot with a `phi %State [%init, %pre], [%next, %latch]`
was tried and **crashed** because the phi had fewer entries than the number
of predecessor blocks. The guards (`[count == N-1] { term! -> ... }`) create
else-blocks that also branch to the loop header, adding predecessors the phi
doesn't account for.

## Three Approaches

### Approach A — Count-ALL Pred then Emit

Pre-scan the `simplified_body` before emitting to count exactly how many
blocks will branch to `case_hdr`. Reserve that many phi entries.

**How**: Walk every statement in the body. Each non-terminal statement maps
to 1 back-edge block. Each `Guarded` maps to 1 extra back-edge (the else block).
Total preds = 1 (pre-header) + body_preds.

Emit phi with `count_preds` entries. Use an array of `(BB, Buf)` placeholder
pairs. As each block is emitted, `replace` one placeholder.

**Risks**:
- Statement-to-block mapping is fragile. A single statement may emit 0 or 2+
  blocks depending on its structure.
- If the mapping is wrong, phi entries don't match predecessors → verifier
  rejects or LLVM crashes.

**Advantage**: One `phi %State` gives LLVM the full struct, and SROA may
decompose it into field-level phis automatically.

### Approach B — Per-Field φ Nodes

Instead of a single `phi %State`, emit 17 independent `phi i64` nodes — one
per field of `%State`. Use `insertelement` / `extractelement` (or
`insertvalue` / `extractvalue`) in the body to construct the struct on the fly.

**Predecessors**: Each field phi has exactly 2 entries: `[init_val, %pre]`
and `[new_val, %latch]`.

**The trick**: The `%latch` block is a **dedicated single back-edge block**.
ALL paths through the body converge to `%latch`. There is no direct branch
from any body block to `case_hdr` — only to `%latch`, which branches to
`case_hdr`.

This means:
1. At loop header: 17 `phi i64` nodes, each with 2 entries.
2. Emit body blocks, each terminating with `br %latch`.
3. `%latch` block: use `insertvalue` on the phis to build `%State`, store to
   `%state_slot` (for post-loop stores), then `br %case_hdr`.

**Struct reconstruction at latch**: Build `%State.new = insertelement %State
zeroinitializer, %f0, 0`, then repeat for each field. Store to `%state_slot`.

**Risks**:
- 17 insertvalue chains at the latch may cause register pressure.
- SROA might already decompose the struct from the alloca slot; this approach
  may not be faster than the current alloca approach.
- Adding a dedicated latch block means the back-edge always takes a detour
  through `%latch`, potentially confusing LLVM's loop analysis.

**Advantage**: No predecessor-counting. Each phi has exactly 2 entries.

### Approach C — Latch-Block Restructuring (Hybrid)

Emit the body using the **original `alloca %State` approach** (which handles
any number of guards/predecessors), then add a **dedicated latch block**
at the end that:
1. Loads from `%state_slot`
2. Stores back to `%state_slot` (this is the "back-edge")
3. Branches to `case_hdr`

The latch block forces the back-edge through a single path. The loop header
phi (if any) would only need 2 entries.

**Actually**: This doesn't use a phi at all. The alloca IS the phi — it's
memory, not an SSA value. This is what we already have.

**Variation C2**: Add a SCALAR-PHI approach. After emitting the body with
allocas, replace each `load %State` → `extractvalue %State, i` + `store`
with 17 individual scalars and `phi i64` per field. This is a post-emission
IR rewrite pass.

**Risks**:
- IR rewrite is fragile. LLVM's `replaceAllUsesWith` doesn't work well
  with phi nodes.
- Too complex for the gain.

## Decision Criteria

For each approach, measure:
1. `fannkuch_redux` ratio vs C
2. `mandelbrot` ratio vs C
3. Does LLVM verify pass?
4. Does clang crash?
5. Implementation complexity (lines changed)

## Implementation Order

1. Start with **Approach A** (count-all preds) — it's the simplest fix to the
   existing phi experiment. If pred counting is precise enough, this gives us
   the `phi %State` without restructuring.

2. If A fails: **Approach B** (per-field phis with dedicated latch block).
   This adds a new block but ensures exactly 2 predecessors per phi.

3. Fallback to multi-pass.

## Predecessor Inference — How to Count

Every statement in the `simplified_body` generates at least 1 block that
branches to `case_hdr`:

| Statement type | Blocks that branch to case_hdr |
|---|---|
| `Let` | 1 (the next-block) |
| `Assignment` | 1 (the next-block) |
| `Expression` | 1 (the next-block) |
| `Guarded { condition, statements }` | 2 (the next-block, plus the else-block) |
| `Guarded` nested in Guarded | 3+ (recursive) |

The pre-header block (`case_pre`) is always 1 predecessor.

Total preds = 1 (pre) + count_backedge_blocks(body).

For each Guarded statement, the else-block adds exactly 1 predecessor.
This is independent of the Guarded's own statements — the else-block just
unconditionally branches to `case_hdr`.

So: preds = 1 + count_non_guarded_stmts(body) + count_guard_blocks(body)

Where count_guard_blocks = number of Guarded statements in the body.

This is deterministic and knowsable before emission starts.

---

Datetime: 2026-06-16T22:00:00-05:00
