# Code Review Fixes — Round 2 (2026-06-02)

## Scope
Address 5 active bugs/cleanup items from codebase review, plus revert clang codegen experiment.

## Steps

### 0. Revert clang → llc in main.rs
- `src/main.rs:1923`: `Command::new("clang")` → `Command::new("llc")`
- `src/main.rs:1924`: `-O3 -march=native -ffast-math -c -x ir` → `-filetype=obj -O3 --mcpu=native`
- Lines 1893, 1898 (comment), 1935: match fallback messages

### 1. __find_from char-boundary fix
- **File**: `lib/ffi/native/src/lib.rs:370`
- **Change**: Add `|| !s.is_char_boundary(start_idx)` to guard
- **Risk**: None — same pattern already used in `__contains_at`

### 2. Dbrief pipeline terminal operations
- **File**: `src/dbrief/eval.rs`
- **Changes**:
  - `apply_operation`: add match arms for all terminal ops (Count → QueryResult::Count, Sum → Aggregated(Sum), etc.)
  - `execute_pipeline`: refactor loop to work with `QueryResult` step-state
  - Missing arms: Sum, Avg, Min, Max, First, Last, Join, LeftJoin → return `Err("not yet implemented")`
- **Risk**: Medium — changes pipeline state machine

### 3. Dataflow extract_ids_recursive completeness
- **File**: `src/analysis/dataflow.rs:92-116`
- **Change**: Replace `_ => {}` with exhaustive arms for all expr variants
- **Risk**: Low — strictly adds coverage

### 4. Protocol verifier postcondition check
- **File**: `src/analysis/protocol.rs:34-44`
- **Change**: Add `extract_postcondition_writes()` + `txn_writes` map, check writes instead of preconditions
- **Risk**: Low — currently non-functional

### 5. Parser keyword deduplication
- **File**: `src/parser.rs:4617-4860`
- **Change**: Extract `is_keyword_identifier()` helper + `parse_keyword_as_expr()` method
- **Risk**: Medium — parser hot path

### Execution order
0 → 1 → 2 → 3 → 4 → 5, re-running `cargo test --lib` after each.
