<!-- 2026-06-15. Transaction body instruction reordering pass. -->

# Instruction Reordering for ILP

## Purpose
Reorder transaction body statements to maximize instruction-level parallelism (ILP). Independent statements (no read/write conflicts) are grouped together so LLVM's out-of-order scheduler can issue them simultaneously.

## Motivation
A transaction body like:

```briv
node compute [ready][done] {
    &x = a + b;      // independent
    &y = c + d;      // independent — can fire in parallel with x
    &z = x * y;      // dependent on x AND y — must wait
};
```

Modern CPUs extract ILP via out-of-order execution, but the window is narrow (~352 µops on Zen 4, ~512 on Golden Cove). Briv has more information than LLVM — contracts expose precise read/write sets — so reordering at the Briv IR level gives LLVM better material to schedule.

## Dependency Analysis

For each statement, we compute a `ReadWriteSet`:

```
struct ReadWriteSet {
    reads: HashSet<String>,   // variables read
    writes: HashSet<String>,  // variables written
}
```

Dependencies are classified:

| Type | Meaning | Condition |
|------|---------|-----------|
| **RAW** (true dependency) | i writes x, j reads x | `writes_i ∩ reads_j ≠ ∅` |
| **WAW** (output dependency) | i writes x, j writes x | `writes_i ∩ writes_j ≠ ∅` |
| **WAR** (anti-dependency) | i reads x, j writes x | `reads_i ∩ writes_j ≠ ∅` |

Statements with no dependency between them can be reordered freely.

## Algorithm

1. **Compute `ReadWriteSet`** for each statement via `rw_set_of(stmt)`
2. **Build dependency DAG**: `build_dependency_graph(sets)` — edges go from i to j when i must come before j
3. **Kahn's topological sort**: `topological_sort(body, deps)` — emits statements in dependency order, grouping independent ones together, returning a `(Vec<Statement>, has_cycle)` tuple. If a cycle is detected (unexpected — suggests a transaction with cyclic assignments), unscheduled statements are appended in original order and a compiler warning is emitted.

## Implementation

`src/backend/llvm/reorder.rs`:

```
┌─────────────┐     ┌──────────────────┐     ┌──────────────┐
│ rw_set_of   │ ──→ │ build_dependency  │ ──→ │ topological   │
│ (per stmt)  │     │ graph            │     │ sort (Kahn's)│
└─────────────┘     └──────────────────┘     └──────────────┘
                                                    ↓
                                           ┌──────────────┐
                                           │ reordered     │
                                           │ Vec<Statement>│
                                           └──────────────┘
```

### Key helper functions

| Function | Purpose |
|----------|---------|
| `rw_set_of(stmt)` | Extract read/write identifiers from any Statement variant |
| `collect_write_target(expr)` | Extract write target from LHS expressions |
| `collect_reads_from_expr(expr)` | Recursively collect variable reads from expressions |
| `build_dependency_graph(sets)` | Build RAW/WAW/WAR dependency DAG |
| `topological_sort(body, deps)` | Kahn's algorithm for maximum ILP |

## Integration

Called from `emit_transaction` in `emit_toplevel.rs`:

```rust
let (reordered, has_cycle) = super::reorder::reorder_body_statements(&txn.body);
if has_cycle {
    self.warnings.push(format!("Warning: dependency cycle ..."));
}
for s in &reordered { self.emit_stmt(out, s, "  "); }
```

Applied to both the `assume_shape` and standard transaction emission paths.

## Tests

| Test | What it verifies |
|------|-----------------|
| `test_reorder_independent_assignments` | `x = a+b; y = c+d;` — no conflict, both emitted |
| `test_reorder_dependent_assignments` | `x = a+b; y = x+1;` — y must come after x |
| `test_reorder_chain` | `a=1; b=a+1; c=b+1;` — full chain preserved |

## Future Work

- **`noalias` GEP annotations**: after reordering, annotate GEP instructions with `!noalias` metadata so LLVM's alias analysis can schedule across field accesses
- **SLP hazard integration**: run reordering BEFORE SLP hazard analysis so the hazard analyzer sees the final instruction order
