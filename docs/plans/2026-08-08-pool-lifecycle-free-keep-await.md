# Pool Lifecycle: free/keep/await + capacity accounting (cells phase)

**Date:** 2026-08-08 · **Status:** Planned · **Ties to:** SPEC §12.2
(spawn/await/free/keep), §16.6 (dependent bounds);
`docs/plans/2026-08-07-object-instance-pools.md` (OPEN: "cells — spawn/await/
keep/free lifecycle beyond the row allocator")

## The problem

The object-instance-pool migration shipped spawn + static/dependent capacity
(`58f89b02`). The lifecycle ops the SPEC promises — `free`, `keep`, `await` —
are not coherent with the pool allocator, and the capacity analysis has three
latent correctness bugs that surface the moment a program uses more than one
node or actually frees.

## The three capacity bugs (root cause first)

### BUG 1 — `free`/`keep` decrement the wrong key (a no-op that pretends to work)

`spawn_pool.rs:221-224`:

```rust
Statement::FreeHint(name) | Statement::KeepHint(name) => {
    let entry = live.entry(name.clone()).or_insert(0);   // name = "h"
    *entry = (*entry - multiplier).max(0);
}
```

spawn keys by **base** (`live.entry(type_name)` → `"Counter"`); free/keep key by
**variable name** (`"h"`). `free h;` decrements a `"h"` entry that nothing else
touches — the `Counter` pool count is never reduced. It is a silent no-op.
Today this is "safe by accident" (capacity never shrinks), but it is dead wrong
logic: the docstring's "maximum concurrent live instances (spawns minus frees)"
is false, and any future "fix" that keys by base **without adding row
reclamation** would introduce a heap overflow (see BUG 2).

### BUG 2 — capacity is `max` across nodes, but the allocator is monotonic

`merge_max` (`spawn_pool.rs:343`) takes the elementwise `max` over transaction
live counts. But `__spawn_next_<base>` (`emit_expr.rs:1256`) is a single
monotonic counter shared by ALL nodes that spawn that base — it never
decrements. Two nodes each spawning `Counter`:

```briev
node a [ticksA < 3][ticksA == 3] { let h = spawn Counter(); ticksA = ticksA + 1; };
node b [ticksB < 5][ticksB == 5] { let h = spawn Counter(); ticksB = ticksB + 1; };
```

`live` = max(3,5) = 5 → column `[6 x i64]`. The counter runs 1..8 across both
nodes → rows 6,7,8 write past the column → **heap/static-column corruption**.

The counter's max value is the **sum** of all firing counts that reach it, not
the max. With a monotonic allocator and no reclamation, capacity must be the
TOTAL lifetime spawn count per base.

### BUG 3 — dependent buffer is off-by-one (row 0)

`emit_dependent_pool_buffers` (`emit_toplevel.rs:908-930`):
`rows = static_rows + Σ (multiplier × bound)`. The allocator counter starts at
**1** (`__spawn_next_<base>` init = 1; row 0 is the static instance). For a
pure dependent pool, `static_rows = 0`, so `rows = bound`. The counter then
writes rows `1..bound` — the last row (`bound`) is index `bound`, but the
malloc holds `bound` elements (indices `0..bound-1`) → **one-past-the-end
write**. spr.bv (BOUND=5) printed `22222` only because malloc rounded the
chunk; valgrind would flag it.

Static columns get this right (`live + 1`). The dependent path must be
`static_rows + Σterms + 1`.

## The design decision: monotonic-allocator capacity vs row reclamation

The SPEC (§12.2) calls `free`/`keep`/`await` a **lifecycle**: "`free task`
requests cancellation/stop and runs defer cleanup", "silently dropping a live
handle is an error". Two coherent end-states:

**Option A — capacity = total lifetime spawns, free/keep/await are
consumption/ownership directives only (no row reuse).**
- Simplest, provably inexhaustible (counter ≤ total).
- `free`/`keep`/`await` mark the handle consumed (typechecker already does);
  the backend emits no row reclamation.
- Cost: a pool that spawns 1M over a program's life (freeing each after use)
  is sized to 1M rows even though only a handful are live at once. Matches
  today's static-column reality; the "cells" open item stays open.

**Option B — free-list reclamation: `free h` returns the row; `spawn` pops a
free row before bumping the counter.**
- Capacity = max concurrent live (smaller pools, SPEC-faithful lifecycle).
- Cost: a `__free_next_<base>[capacity]` column (or liveness bitmask) per
  base, free-list head slot, and the spawn/free codegen changes. The capacity
  analysis must track a running MAX of live (spawns minus frees within the
  body), not a total.

**Decision: BOTH, staged.** This plan ships Option A's correctness fixes now
(the three bugs — they are real memory-corruption bugs independent of which
option wins), then Option B's reclamation as the lifecycle that makes
`free`/`await` meaningful. Rationale: Bug 2 and Bug 3 are corruption bugs that
exist in the current monotonic design regardless; fixing capacity accounting
first keeps the compiler correct for ALL programs today, and Option B builds
cleanly on the corrected total-capacity as its upper bound.

## Phase 1 — fix capacity accounting (correctness)

### 1a. `free`/`keep` stop pretending to shrink the pool

`spawn_pool.rs`: remove the `FreeHint`/`KeepHint` decrement from the `live`
accounting. `free h;` / `keep h;` become consumption/ownership directives —
they consume the handle's typechecker binding (already done) and DO NOT reduce
capacity. Rationale comment: the allocator is monotonic; without reclamation
(Phase 2) a free never returns a row, so a capacity reduction here would
under-allocate. The walk still visits `free`/`keep` args (they can reference
spawn args? no — keep the arm, no live effect).

### 1b. capacity = total spawn count per base (sum across nodes)

`merge_max` → `merge_total` (elementwise **sum**). `capacities[base]` = sum of
all `live[base]` across every transaction/definition — the counter's true max.
Update the docstring (`predictably inexhaustible`) to "total lifetime spawn
count; the monotonic allocator counter never exceeds it; a pool is sized to
the sum of every bounded firing context that spawns it."

Keep the existing tests; add:
- two-node same-base spawn test → capacity = sum (e.g. 3 + 5 = 8).
- the `free h;` no-op test: `[ticks < N]` spawn + free + spawn still sizes to
  the spawn count (free does not shrink).

### 1c. dependent buffer +1

`emit_dependent_pool_buffers`: `rows = static_rows + Σterms + 1` (row 0 is
the static instance the counter skips). Verify with the existing spr.bv +
valgrind / ASAN build; add a unit assertion on the emitted size expression
(`add i64 ... , 1`).

### 1d. retire the boxed obj-instance fallback for instances

`emit_member_body` (`emit_expr.rs:2037`): the `prefix: None` arm
(`self_binding = Some((type_name, inttoptr recv))`) is the retired boxed path.
Today every obj instance member call arrives with a prefix (instance_prefix_for
resolves spawned handles + unpacked instances), so the None arm should only
fire for genuine `struct` values. Audit the 12 `self_binding` sites; where a
site can only be reached by struct values keep it, and where it is an
instance-only path, delete it + its test. Do NOT touch struct-literal paths.

## Phase 2 — the lifecycle: await + row reclamation (cells)

### 2a. `await h` statement, end-to-end

- **Lexer:** `Token::Await` exists.
- **Parser:** `await <identifier>;` → `Statement::Await(String)` (mirror
  FreeHint shape; also an expression form `let r = await h;`? — keep statement
  form first, SPEC shows `let result = await task;` so support the let-bound
  form too: `Expr::Await(Box<Expr>)`).
- **Typechecker:** consume the handle like `free` (mutable + not already dead);
  `await h` yields the instance's declared output type (SPEC 12.2 "returns the
  callable's declared result"). For a pool instance with no output member,
  output = Void.
- **Interpreter:** `await h` marks the handle dead; returns the instance's
  declared result (Void for no output). Reference for codegen.
- **Codegen:** consume the handle (no-op for a monotonic pool until 2b) + mark
  the row free if reclamation exists.

### 2b. free-list reclamation (the cells)

- Per base: a `__free_head_<base>` i64 slot (init = 0) + a `__free_next_<base>`
  `[capacity x i64]` column (init = 0), both ALWAYS-live. `free h;`/`await h`
  pushes the row: `next[row] = head; head = row`. `spawn` pops: `if head != 0
  { row = head; head = next[row]; } else { row = counter++; }`. Row 0 (the
  static instance) is never freed — head init 0 doubles as "empty" because row
  0 is reserved.
- Capacity analysis gains a **peak-live** pass (running max of
  spawns−frees within each body, summed across nodes) so pools can shrink to
  the true concurrent maximum. Option B's correctness proof: total-capacity is
  the upper bound, peak-live the tight bound; reclamation makes both sound.
- Interpreter reference: free/await return the row; spawn reuses it.

### 2c. linear ownership diagnostics

`keep h` transfers ownership to the enclosing boundary (SPEC 12.2) — after a
`keep`, the current node must not free/read the handle. `await` consumes.
Enforce with the existing consumed_locals machinery (extend to Await/Keep).

## Phase 3 — liveness wiring + docs (cleanup)

- Field-liveness scan walks member bodies so unpacked instance slots prune
  when dead (drop the ALWAYS-live blanket, `mod.rs:4626`). Gate: dead instance
  state is eliminated; live instances unchanged.
- Update `docs/architecture/` + SPEC §12.2 notes + this plan's progress
  section; update the 2026-08-07 plan's OPEN (cells → done).

## Phase 1 shipped 2026-08-08

- **1a (Bug 1):** `free h;`/`keep h;` no longer decrement the pool count
  (`spawn_pool.rs`) — the monotonic allocator never reclaims, so the decrement
  (keyed by var name, not base) was both a no-op AND a trap: keying it by base
  would under-allocate. `free`/`keep` are consumption directives only.
- **1b (Bug 2):** `merge_max` → `merge_total` — capacity per base is the SUM
  across all nodes (one shared monotonic counter), not the max. Two nodes
  spawning the same base get `[a+b+1 x T]` columns, never `[max(a,b)+1]`.
- **1c (Bug 3):** `emit_dependent_pool_buffers` adds the `+1` for row 0 — the
  last spawned row was writing one-past-the-end (masked by malloc slop).
- **1d:** boxed obj-instance fallback guarded — a genuine POOL instance (has
  `{base}.`-prefixed instance slots) reaching `self_binding` panics loudly;
  stdlib collection objs (List/RingBuffer, boxed struct addresses) keep the
  boxed path.

### Three additional pre-existing bugs found while verifying Bug 2

Verification exposed pre-existing dispatch/codegen bugs (not pool-related,
fixed under "bug in a touched file" rule):

1. **Literal-bound countdown loops ran once.** `emit_countable_load_bound`
   fell back to `add i64 0, 1` for `[ticks < N]` with a literal N — every
   literal-bound countdown (spawn pools included) silently under-ran. Threaded
   `bound_literal` through all four live emitters
   (`emit_countable_main`/`emit_folded_main`/`emit_countable_countdown_main`/
   `emit_version_dag_main`) + `emit_folded_multi_main`. spc3 (three firings,
   spawn+inc each) went from `2` to `222`.
2. **Multi-txn fold selected for disjoint counters.** `multi_foldable` folded
   any set of async bounded txns into ONE loop driven by a single counter
   (hardcoded slot 0) — two nodes with different counters ran once then
   dropped. Now gated on all async txns sharing the SAME counter var + bound;
   the fold call derives the shared driver instead of `0/None/None`.
3. **Synthetic exit referenced an undefined global.** `program_convergence`
   emitted `ticks >= __lit__ticksA` (a synthetic name with no backing global)
   for literal bounds → `@__lit__ticksA` undefined in the reactor exit check.
   `counter_ge_bounds` now carries the bound as an `Expr` (`Decimal` for
   literals, `Identifier` for field/const bounds).

Verified: 1666 lib tests (new: free/keep no-op ×2, cross-node sum,
convergence literal-value, dependent +1 + literal-bound assertions in the
spawn member test) + full `--runtime` benchmark suite all MATCH (ring_buffer
1.07x vs 1.18x baseline — improved, not regressed). The two-node spawn RUNTIME
path still needs the async/sync reactor dispatch fixes (empty reactor_tick,
thread-pool convergence) — pre-existing, tracked for Phase 2's cells work.

## Verification

- `cargo test --lib` green at every phase (existing 1662 + new tests).
- New unit tests: 1b cross-node sum, 1a free no-op, 1c dependent +1,
  2a await consume/result, 2b spawn-reuses-freed-row + free-then-spawn cycle,
  2c keep-after-free is a compile error.
- spr.bv under BOUND=5/50: output unchanged (`22222` / 50×`2`), now with the
  +1 fix; run under ASAN (`-fsanitize=address`) to prove the OOB is gone.
- A two-node spawn program compiled + run under ASAN (Bug 2 regression).
- Backend unit tests (`src/backend/llvm/tests.rs`): assert the emitted malloc
  size includes the `+ 1`, and (2b) the spawn path pops the free-list first.
- Benchmarks: enemy_swarm + linked_list samples (no regression — spawn is not
  on their hot paths, but confirm the analysis change doesn't perturb
  state layout).
- Praetor `--warn` on changed dirs; no new diagnostics.
- Per-commit: `cargo test --lib`; commit each phase separately.

## Reference map

| Change | File |
|--------|------|
| free/keep no-op + total-sum + docstring | `src/analysis/spawn_pool.rs` |
| dependent +1 | `src/backend/llvm/emit_toplevel.rs` (`emit_dependent_pool_buffers`) |
| Await AST/parse/typecheck/interp/codegen | `src/ast/`, `src/parser/statements.rs`, `src/typechecker/mod.rs`, `src/interpreter/eval.rs`, `src/backend/llvm/emit_stmt.rs` |
| free-list columns + spawn/free codegen | `src/backend/llvm/mod.rs` (register), `src/backend/llvm/emit_expr.rs` (Spawn), `src/backend/llvm/emit_stmt.rs` (FreeHint/Await) |
| peak-live analysis | `src/analysis/spawn_pool.rs` |
| boxed instance audit | `src/backend/llvm/emit_expr.rs` (`emit_member_body`, `self_binding` sites), `src/backend/llvm/emit_stmt.rs` |
| liveness walk of member bodies | `src/backend/llvm/mod.rs` (`apply_field_modes` region) |
