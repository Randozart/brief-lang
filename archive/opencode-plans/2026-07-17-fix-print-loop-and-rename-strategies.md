# Fix print_loop + Rename Legacy Strategy IDs

## Part 1: Fix print_loop (compound AND precondition)

### Root Cause

`print_loop` precondition is `[bound > 0 && ops < bound]` — a compound AND.
`extract_bounded_pre` at `transition_graph.rs:268-270` handles AND by trying left
first, then right:

```rust
Expr::BinaryOp(BinaryOpKind::And, l, r) => {
    extract_bounded_pre(l).or_else(|| extract_bounded_pre(r))
}
```

For `bound > 0 && ops < bound`:
- Left `bound > 0` → matches `Gt(Identifier, Decimal)` → `BoundedPre { var: "bound", bound_literal: Some(0) }`
- Returns immediately, never tries right `ops < bound`

Then `extract_valid_bounded_pre` checks `inc.var == bp.var` → `"ops" != "bound"` → returns `None`.
Since `bounded_pre` is `None`, `foldable` is false → falls through to broken SSA path.

### Fix

Change the AND handler to try BOTH sides and prefer the one that looks like a
variable-to-variable comparison (bound_literal is None), which is the real
counter loop condition:

```rust
Expr::BinaryOp(BinaryOpKind::And, l, r) => {
    let left = extract_bounded_pre(l);
    let right = extract_bounded_pre(r);
    match (left, right) {
        (Some(lp), Some(rp)) => {
            // Prefer the one with a variable bound (bound_literal = None).
            // Variable-to-variable (e.g. ops < bound) is the real counter loop;
            // the literal bound (e.g. bound > 0) is just a guard.
            if lp.bound_literal.is_some() { rp } else { lp }
        }
        (Some(lp), None) => Some(lp),
        (None, Some(rp)) => Some(rp),
        (None, None) => None,
    }
}
```

This correctly selects `ops < bound` (bound_literal = None) over `bound > 0`
(bound_literal = Some(0)).

### File Changed

- `src/analysis/transition_graph.rs` — AND handler in `extract_bounded_pre`

### Tests

- `cargo test --lib` — all 913+ tests pass
- `print_loop BOUND=5` — produces empty output (matches C)
- Full correctness suite — print_loop goes from MISMATCH to MATCH

---

## Part 2: Rename Legacy Strategy IDs

### Mapping

| Old Name | New Name | Used In |
|----------|----------|---------|
| A000 | `EmitPureCounterFold` | 8 occurrences in mod.rs, helpers.rs, tests.rs |
| A005 (bare) | `EmitAdaptive` | 4 occurrences in mod.rs (comments only) |
| A005a | `EmitInlineSsa` | 12 occurrences in mod.rs, context.rs, emit_toplevel.rs |
| A005b | `EmitMemoryCounter` | 4 occurrences in mod.rs, counter.rs |
| A005c | `EmitPerFieldPhi` | 18 occurrences in mod.rs, context.rs, counter.rs |
| A005d | (removed, 2 historical comments) | `// A005d removed` → `// Removed memory loop variant` |
| A005e | `EmitHybridCounterPhi` | 4 occurrences in mod.rs, counter.rs |
| A006 | `EmitSequentialSsa` | 7 occurrences in mod.rs |

### What Changes

**Comments only** — no functional code changes. The strategy names appear in:
1. Section header comments (e.g. `// ── Precomputation check (A000) ──`)
2. Decision-tree comments (e.g. `// A005c is selected for dense-write...`)
3. `self.warnings.push(format!("info: txn '{}' dispatched via ..."))` — these
   are the most visible; they appear in compiler output

### File Changed

- `src/backend/llvm/mod.rs` — all 50+ occurrences in comments and warnings
- `src/backend/llvm/context.rs` — 5 occurrences in comments
- `src/backend/llvm/helpers.rs` — 1 doc comment
- `src/backend/llvm/emit_toplevel.rs` — 1 comment
- `src/backend/llvm/tests.rs` — 1 comment
- `src/backend/llvm/loop_engine/counter.rs` — 5 occurrences in doc comments + section headers
- `src/backend/llvm/loop_engine/mod.rs` — 1 doc comment

### Tests

- `cargo test --lib` — all 913+ tests pass (no functional change)
- Only comment words changed, no test assertions match these strings

---

## Total Change Summary

| File | Change |
|------|--------|
| `src/analysis/transition_graph.rs` | Fix AND handler in `extract_bounded_pre` |
| `src/backend/llvm/mod.rs` | Rename A000/A005a/A005b/A005c/A005d/A005e/A006 in comments + warnings |
| `src/backend/llvm/context.rs` | Rename A005a/A005c in comments |
| `src/backend/llvm/helpers.rs` | Rename A000 in doc comment |
| `src/backend/llvm/emit_toplevel.rs` | Rename A005a in comment |
| `src/backend/llvm/tests.rs` | Rename A000 in comment |
| `src/backend/llvm/loop_engine/counter.rs` | Rename A005b/A005c/A005e in doc comments |
| `src/backend/llvm/loop_engine/mod.rs` | Rename A005e in doc comment |
