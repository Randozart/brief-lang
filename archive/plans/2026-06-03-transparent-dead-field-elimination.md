# Transparent Dead-Field Elimination — Info Diagnostics (A002/A003)

## Problem
The compiler silently eliminates dead fields and folds pure-counter loops to O(1).
Benchmark wins like iir_filter (0.001s vs C 0.107s) and const_heavy (0.001s vs C 0.048s)
are unexplainable to the programmer — they look like cheating. The user must guess
what the compiler eliminated and why.

## Solution
Always-on info diagnostics that tell the programmer exactly what was eliminated
and why, with a `--no-dead-info` flag to suppress them.

## Implementation

### Phase 0 — Export liveness info from transition graph
- Add `live_fields: HashSet<String>` to `ReactorTransitionGraph`
- Computed during `build()`, accessible from `generate()`
- Enables backend to emit diagnostics without re-running the analysis

### Phase 1 — A002: Dead-field info per txn
- For each `ReactorNode`, compute `dead_fields = write_set - live_fields`
- Emit `info:` line for each dead field:
  ```
  info: field 'x1' written by txn 'process' is never read — stores eliminated
  ```
- Fires for every txn where non-counter writes are dead, regardless of fold status

### Phase 2 — A003: Pure-counter fold info
- When `node.is_effectively_pure == true`, emit:
  ```
  info: txn 'process' folded to O(1) store — 50000000 iterations eliminated
  info: dead fields: x1, x2, y1, y2
  info: counter 'count' retains its store (the only live write)
  ```

### Phase 3 — `--no-dead-info` flag
- New field `dead_info_disabled: bool` on `LlvmBackend` (default `false`)
- Builder method `with_dead_info_disabled()`
- CLI flag `--no-dead-info` parsed in `main.rs` `run_llvm_compile()`
- When set, suppress all A002/A003 output

### Phase 4 — LSP support (future)
- Current diagnostics use strings with `info:`, `note:`, `help:` formatting
- LSP needs source-location-annotated diagnostics (file, line, column, range)
- Requires threading `SourceLoc` through `ReactorNode` body statements
- This plan does not implement LSP; it sets the diagnostic shape so LSP can parse it

## Files Changed
- `src/analysis/transition_graph.rs` — Add `live_fields` to `ReactorTransitionGraph`
- `src/backend/llvm.rs` — Add `dead_info_disabled`, `with_dead_info_disabled()`, emit A002/A003
- `src/main.rs` — Parse `--no-dead-info`, pass to backend

## Verification
- `cargo test --lib` — all 372 tests pass
- `const_heavy.bv` produces A002/A003 info (acc dead, count folded)
- `iir_filter.bv` produces A002/A003 info (x1/x2/y1/y2 dead, count folded)
- `float_math.bv` produces NO A003 (no fold — bounded_pre fails) but A002 may fire
- `--no-dead-info` suppresses all above
