# Sparse Dispatch Collapse — Equivalent-body modulo-switch optimization

## Baseline Results (commit 13af14a)

All 22 benchmarks MATCH. Run at BOUND=50000000, 5 iterations.

| Benchmark | Ratio | Brief | C | Correct | Notes |
|-----------|-------|-------|---|---------|-------|
| ring_buffer | .96x | .0597s | .0620s | MATCH |
| float_math | .82x | .0589s | .0713s | MATCH | BEATS C |
| float_math_nonzero | 1.02x | .1684s | .1642s | MATCH |
| **sparse_dispatch** | **1.35x** | **.0825s** | **.0611s** | **MATCH** | **TARGET** |
| print_loop | 1.01x | .0590s | .0582s | MATCH |
| nbody_newton | .68x | 5.7089s | 8.3001s | MATCH | BEATS C |
| nbody_sqrt | .85x | 2.3983s | 2.8201s | MATCH | BEATS C |
| nbody_sqrt_idio | .69x | 2.5265s | 3.6484s | MATCH | BEATS C |
| fasta | .97x | .2123s | .2174s | MATCH | BEATS C |
| fannkuch_redux | 1.09x | .0696s | .0637s | MATCH |
| mandelbrot | 1.00x | .6603s | .6571s | MATCH |
| kalman_filter | .98x | .1765s | .1783s | MATCH |
| knucleotide | .98x | .1891s | .1923s | MATCH |
| cancel_math | 1.09x | .0647s | .0593s | MATCH |
| bit_clear | 1.00x | .0006s | .0006s | MATCH |
| queue_drain | 1.02x | .0610s | .0595s | MATCH |
| queue_drain_sym | .99x | .0610s | .0611s | MATCH |
| interval_step | .01x | .0007s | .0603s | MATCH | Precomputed |

## Problem

`sparse_dispatch.bv` has 8 reactive transactions (`ping`..`stat`), each gated
by `count < total && count % 8 == N`. All 8 bodies are IDENTICAL:
```
&count = count + 1;
[count % 5000000 == 4999999] { print_int#(count + 1); };
term;
```

The compiler's `emit_modulo_rotated` emits all 8 bodies sequentially in a
rotated loop, producing 8× the LLVM IR of a single-body equivalent. C's clang
at `-O3` eliminates the empty switch entirely, producing just `count++` with a
5M-interval print guard. Clang's output has 1 body to optimize; Brief's has 8
separate basic blocks.

## Root Cause

`try_modulo_switch_dispatch` in `loop_engine.rs:1697` detects the `count % K`
pattern and routes to `emit_modulo_rotated` (K ≤ 8). The rotated loop emits:
```
_body4:
  load round_base from %State
  for N in 0..K:
    store round_base + N to %State      // infrastructure increment
    emit_body[case N]                   // body also increments count
  load count, check count < bound
  br _body4
```

This is correct but inefficient: K separate `emit_stmt` calls produce K copies
of every body instruction, each in its own anonymous block. LLVM cannot CSE
across these blocks because each has different dominance relationships.

## Fix: Modulo-Adjusted Single Body

When all K bodies are structurally identical AND the print guard follows the
pattern `count % M == M-1` with `print_int#(count + 1)` AND `M % K == 0`,
collapse to a single body with `count += K` per trip:

| Aspect | Original (per-body) | Collapsed (1 body) |
|--------|--------------------|--------------------|
| Increment | `&count = count + 1` | `&count = count + K` |
| Print guard | `count % M == M-1` | `count % M == 0` |
| Print value | `print_int#(count + 1)` | `print_int#(count)` |
| Exit check | After K bodies | After 1 body (+ loop exit) |

**Verification** (M=5000000, K=8):
- Original fires at post-inc count = 4999999, prints 5000000
- Collapsed fires at post-inc count = 5000000, prints 5000000
- 5000000 = 624999 * 8 → after 624999 trips, count = 624999*8 = 4999992,
  then +8 = 5000000. `count % 5000000 == 0` → true. Print(5000000). ✓

**When M % K != 0**: fall back to original rotated loop (no optimization).

## Implementation

### 1. `try_modulo_switch_dispatch` — detect equivalent bodies

In the existing function, after the modulo pattern is confirmed, add a check:
```rust
fn bodies_are_structurally_identical(txns: &[(String, &Transaction)]) -> Option<Vec<&Statement>> {
    let bodies: Vec<&[Statement]> = txns.iter().map(|(_, t)| t.body.as_slice()).collect();
    if bodies.iter().all(|b| b == &bodies[0]) {
        Some(bodies[0].iter().collect())  // return reference to first body
    } else {
        None
    }
}
```

This is O(K × body_size). For sparse_dispatch (K=8, small bodies), negligible.

### 2. `try_modulo_switch_dispatch` — detect transformable print guard

Check the body for the pattern:
```
&count = count + 1;
[count % M == M-1] { print_int#(count + 1); };
term;
```

When matched AND `M % K == 0`, set a flag for the collapsed path.

### 3. `emit_modulo_rotated` — collapsed emission

Add a code path after the chunk-to-monolith copy (line 2326):
```rust
if bodies_are_equivalent && can_transform {
    // Emit collapsed loop:
    //   _body4:
    //     &count = count + K;
    //     [count % M == 0] { print_int#(count); };
    //     load count, check count < bound
    //     br _body4
} else {
    // Original K-body rotated loop
}
```

The collapsed body is emitted ONCE via `emit_stmt`. The increment uses `K`
instead of `1`. The guard and print are the standard body statements,
which `emit_stmt` handles normally (the AST has already been transformed
by the analysis pass).

Wait — we can't transform the AST (it's borrowed). Instead, modify how the
body is emitted:
- Override the increment write-set to use `K` instead of `1`
- The guard check is automatically correct because the body emits
  the same AST with the same `count % M == M-1` check, but `count` now
  advances by `K` — so the effective mask changes

Actually, the transformation approach:
1. Clone the first body's statements
2. Replace `&count = count + 1` with `&count = count + K`
3. Use the modified statements for a single `emit_stmt` call

But cloning and modifying AST is complex. Simpler: emit the body statements
selectively, overriding the increment count.

### 4. Simplified approach: emit statements once, adjust increment

```rust
if collapsed {
    // Load count
    // Emit all statements of the first body except term/term!
    // Instead of body's own &count = count + 1, emit:
    //   load count, add K, store count
    // Then: check count < bound, br _body4
    // No inner loop needed — count += K per trip does the same work as
    // K bodies each doing count += 1 in the original rotated loop.
}
```

This approach avoids modifying the AST — just emit the body statements
and replace the `&count = count + 1` with a `&count = count + K` at the
infrastructure level.

## Documentation

### Code sites requiring rationale comments

1. **`try_modulo_switch_dispatch`** (line ~1697, `loop_engine.rs`):
   - Add comment documenting the equivalence detection logic
   - Document the `M % K == 0` constraint and the print-guard transformation

2. **`emit_modulo_rotated`** (line ~2243):
   - Add collapsed path comment: `// 2026-07-07: When all K bodies are`
     `// structurally identical and M % K == 0, emit a single body with`
     `// count += K instead of K copies.  The print guard is automatically`
     `// correct because count jumps by K — the modulo check `count % M ==`
     `// M-1` becomes `count % M == 0` at the reported value.`
   - Explain why the original rotated loop is still used for M % K != 0

3. **`bodies_are_structurally_identical`** (new helper):
   - `/// 2026-07-07: Returns true when all K modulo-switch transaction`
   - `/// bodies are byte-for-byte identical (same AST statements).`
   - `/// Used by the collapsed dispatch optimization in emit_modulo_rotated.`
   - `/// When true, a single body + count += K replaces K copies.`

### Architecture docs

No changes needed unless the dispatch decision tree changes significantly.
The current tree already routes to `emit_modulo_rotated`. The collapse is
an internal optimization within that function.

### Preserving existing comments

The existing rotated-loop code path remains unchanged — the `if collapsed`
branch is additive. All existing rationale comments (2026-07-05 entries
about chunk allocas, circular phi chains, etc.) stay in place.

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| `M % K != 0` cases | Cannot apply optimization | Fall back to original rotated loop |
| Bodies differ semantically | Wrong output | Only apply when byte-identical |
| Print guard not `count % M == M-1` | Cannot apply optimization | Fall back to original |
| Count-dependent body statements beyond print guard | Wrong output | Restrict transformation to bodies where the ONLY count-dependent statement is the recognized guard pattern |

## Verification

1. `cargo test --lib` — all 1403 pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all 22 MATCH
3. sparse_dispatch ratio should improve from 1.35x to ~1.0x
4. No other benchmark regresses (only sparse_dispatch uses mod-switch)
