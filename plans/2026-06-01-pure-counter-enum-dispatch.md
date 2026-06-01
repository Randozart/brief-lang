# Plan: Pure-Counter Fold Elimination for Enum Dispatch

**Date:** 2026-06-01
**Status:** Planned → In Progress

## Problem

The enum dispatch path (Path 4) emits a while-loop for every bounded-counter txn,
even when the txn body is a pure increment (`is_pure_body = true`). For
ring_buffer's `work` txn (`ops = ops + 1`), this produces 50M iterations — each
doing GEP → load → icmp → br → inlined add+store → br.

The standard reactor path (Path 2) already has this optimization:
- `is_pure_body` → `emit_folded_pure_counter(...)` → `store i64 N, i64* %ops` (O(1))
- otherwise → `emit_folded_main(...)` → `while (ops < N) work()` (O(N))

But `emit_folded_pure_counter` emits a **full `@main()` function** — it's only
usable when the program IS the single folded loop. The enum dispatch path needs
a lightweight variant: just the GEP + store instruction pair, inline in the case arm.

## Solution

Add a companion map `enum_fold_pure` alongside `enum_fold_params` that carries
`is_pure_body: bool` and `total_value: Option<i64>` per txn. In
`emit_case_folded_loops`, for pure txns with a known total, emit GEP + store
instead of the while-loop.

## Implementation

### Step 1: Build `enum_fold_pure` map in `generate()`

After `enum_fold_params` is built (line ~738), build a parallel map:

```rust
let enum_fold_pure: HashMap<String, (bool, Option<i64>)> = {
    let mut m = HashMap::new();
    for txn_name in &enum_txn_names {
        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
            let total_val = bp.bound_var.and_then(|bv| {
                field_initializers.get(bv).and_then(|e| e.as_ref())
                    .and_then(|e| if let Expr::Integer(n) = e { Some(*n) } else { None })
                    .or_else(|| constants.get(bv).and_then(|(_, e)|
                        if let Expr::Integer(n) = e { Some(*n) } else { None }
                    ))
            });
            m.insert(txn_name.clone(), (node.is_pure_body, total_val));
        }
    }
    m
};
```

Total value resolution (reuses the same logic as lines 634-642):
1. Try `field_initializers` for the bound_var (state field with initial value)
2. Fall back to `constants` for the bound_var (constant like `N = 50000000`)
3. If neither yields a value, `total_value = None` → skip optimization

### Step 2: Pass to `emit_enum_main`

Add parameter `fold_pure: Option<&HashMap<String, (bool, Option<i64>)>>`.
Update the call site at line ~725.

### Step 3: Modify `emit_case_folded_loops` closure

In the multi-txn loop (line 2449-2453), before calling `emit_folded_loop`:

```rust
if let Some(fp) = fold_pure {
    if let Some(&(pure, tv)) = fp.get(ptxn_name) {
        if pure {
            if let Some(tv) = tv {
                writeln!(out, "  %pc_{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", sub_prefix, pci).ok();
                writeln!(out, "  store i64 {}, i64* %pc_{}, align 8", tv, sub_prefix).ok();
                return; // skip the folded loop entirely
            }
        }
    }
}
emit_folded_loop(out, ptxn_name, pci, pti, ptcn_ref, &sub_prefix);
```

Same logic for the single-txn fallback (line 2456).

### Step 4: Handle `all_internal_lookup` priority

The existing `all_internal_lookup` (for composed chains) already provides the
pure-counter shortcut at a HIGHER level — it runs BEFORE calling
`emit_case_folded_loops`. No conflict.

## Edge Cases

| Case | Behavior |
|------|----------|
| Bound is a state field (dynamic) | `total_value = None` → while-loop |
| Txn has side effects (not pure) | `is_pure_body = false` → while-loop |
| Multi-txn (async_counters) | Each pure txn gets store; impure gets loop |
| Trigger value = 0 (inactive) | Case arm still stores N — exit check fires immediately |
| Single-value trigger | Same logic via the `single_value` path (line 2467) |

## Expected Results

```
ring_buffer:     0.11s  →  ~0.001s  (110× speedup)
async_counters:  0.11s  →  ~0.001s  (110× speedup)
iir_filter:      0.15s  →  0.15s    (no change — already folded)
precompute_sum:  0.00s  →  0.00s    (no change — already precomputed)
```

## Tests

- `test_enum_pure_counter`: pure txn in enum dispatch emits `store i64 N` instead
  of while-loop
- Update `test_exit_in_enum_main` / `test_exit_in_enum_hybrid_wake`: adjust
  assertions for eliminated loop

## Files Changed

- `src/backend/llvm.rs` only (~30 lines net)
