## Dispatch Collapse: Reactive Before State

**What**: Optimized reactive transaction dispatch so that preconditions evaluate
against the pre-tick state, allowing the compiler to skip entire transaction
bodies when no guard can trigger.

**Why it matters**: In programs with many reactive transactions, the old
dispatch evaluated guards after state updates, forcing every transaction to
at least peek at the new state. With collapse, transactions whose preconditions
can be statically disproven are elided entirely from the dispatch loop.

**How**: The dispatch analysis builds a dependency graph between state fields
and transaction guards. When a tick updates field X, the compiler walks the
graph to find only the transactions whose guards reference X. All others are
skipped without evaluation. The key insight: guard expressions reference the
prior-state (`@field`), which the `#!exit` pragma evaluates against snapshot
values, not the in-progress tick.

**Before/After**: In programs with 10+ reactive transactions and 2-3 active
per tick, dispatch overhead dropped from O(N) to O(K) where K is the number
of guards referencing the changed field.
