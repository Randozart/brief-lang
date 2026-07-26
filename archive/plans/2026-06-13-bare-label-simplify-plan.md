# Plan: Fix Bare Label Bug + Rewrite Equality Saturation

Date: 2026-06-13
Status: Active

---

## Part 1: Bare Label Bug — Diagnosis & Fix

### Root Cause

**Syntactic bug in emit_expr.rs**: label definitions use a `%` prefix, but LLVM IR
requires label definitions WITHOUT `%` and label references WITH `%`.

```
// ❌ WRONG: LLVM interprets %mdef4 as a value reference
%mdef4:

// ✅ CORRECT: label definition
mdef4:

// ✅ CORRECT: branch target reference
br label %mdef4
```

The backend has always been wrong here. `opt` (the LLVM optimizer) rejects this
with `expected '=' after instruction name` because `%mdef4` looks like the start
of `%mdef4 = add i64 0, ...` and the `:` is unexpected.

The interpreter-only path never runs `opt`, so this bug was latent — only exposed
when `brief build` was switched to use the LLVM backend (previous session).

### Affected Sites (6 label definitions in emit_expr.rs)

| Line | Current (wrong) | Correct |
|------|-----------------|---------|
| 656 | `%marm<N>:` | `marm<N>:` |
| 673 | `%mdef<N>:` | `mdef<N>:` |
| 681 | `%mmerge<N>:` | `mmerge<N>:` |
| 735 | `%s_hdr<N>:` | `s_hdr<N>:` |
| 739 | `%s_body<N>:` | `s_body<N>:` |
| 759 | `%s_done<N>:` | `s_done<N>:` |

### Secondary Issues Found in Same Area

1. **Dead `br` after `unreachable`** (emit_expr.rs ~677-680):
   When match has no wildcard arm, emits `unreachable` (terminator) then
   immediately emits `br label %mmerge` (another terminator). The `br` is
   dead code. Fix: move the `br` inside the `if let Some(wildcard)` arm.

2. **`terminated` flag leak** (emit_stmt.rs ~374-420):
   After a `Guarded` block whose body sets `self.terminated = true`, the flag
   is not restored to `prev_terminated`. Currently:
   ```rust
   if !self.terminated {
       self.terminated = prev_terminated; // restored only on fall-through path
   }
   // When self.terminated is true: NOT restored — leaks!
   ```
   Fix: always restore `self.terminated = prev_terminated` unconditionally.

3. **No defensive guard on `post:` label** (emit_toplevel.rs ~457):
   Minor — valid Brief always ends bodies with `term`. Not fixing this cycle.

---

## Part 2: Simplify Pass — Complete Rewrite

### Design Goals

1. **O(n) complexity** — each node visited exactly once, no fixpoint loop
2. **Separate optimization pass** — runs on AST before codegen, not inline
3. **`--prod` / `--release` gated** — only enabled for production builds; disabled in `--dev`
4. **Memoized via hash-cons cache** — identical subexpressions simplified once
5. **Independent budget** — `--simplify-budget <N>` limits total nodes processed

### Algorithm: Bottom-Up Rewriting with Hash-Cons Cache

```
fn simplify(expr) -> Expr:
    h = structural_hash(expr)
    if cache[h]: return cache[h]

    result = match expr:
        Binary(op, l, r) =>
            sl = simplify(l)          // children first
            sr = simplify(r)
            try_rewrite(op, sl, sr) ?? Binary(sl, sr)

        Unary(op, inner) =>
            si = simplify(inner)
            try_rewrite(op, si) ?? Unary(si)

        Variadic(children) =>
            op(children.map(simplify))

        Leaf => expr.clone()

    cache[h] = result
    return result
```

**No fixpoint loop** — each node visited exactly once. Bottom-up handles all
existing rewrites because children are simplified before parent rewriting.

### Complexity

| Current (broken) | Proposed |
|---|---|
| Mutual recursion: `simplify ↔ simplify_pass` | Single pass |
| 5 fixpoint iterations × 2 children = 10× per level | Children simplified once |
| `format!("{:?}")` comparisons per iteration | Structural hash (u64) |
| No cache | Hash-cons: each unique subexpr once |

### Hash-Cons Cache

```rust
struct SimplifyCache {
    map: HashMap<u64, Expr>,
    nodes_processed: u64,
    budget: u64,
}
```

Key: structural hash of `Expr` (discriminant + recursive hash of children).
Same subexpression anywhere in the tree → same hash → simplified once.

### try_rewrite Rule Set (preserved from existing code)

All identity/cancellation rules carry over identically:
- `x + 0 → x`, `0 + x → x`
- `x - 0 → x`, `x - x → 0`
- `(a+b) - b → a`, `(a+b) - a → b`, `(a-b) + b → a`
- `x * 0 → 0`, `0 * x → 0`, `x * 1 → x`, `1 * x → x`
- `x / 1 → x`
- `x & 0 → 0`, `0 & x → 0`
- `x | 0 → x`, `0 | x → x`
- `x ^ 0 → x`, `0 ^ x → x`
- `x << 0 → x`, `x >> 0 → x`
- `true && x → x`, `x && true → x`, `x && x → x`
- `false || x → x`, `x || false → x`, `x || x → x`
- `!!x → x`, `--x → x`

### Pipeline Integration

```
Source → Parse → Resolve → Typecheck → [SIMPLIFY if --prod] → Codegen
```

SIMPLIFY phase is pure AST→AST. No backend changes.

### CLI Flags

| Flag | Effect |
|------|--------|
| `--dev` (default) | budget=256, simplify=OFF |
| `--prod` / `--release` | budget=MAX, simplify=ON |
| `--optimize-budget <N>` | overrides budget |
| `--simplify-budget <N>` | max nodes for simplify pass (prod default: MAX, dev: 0) |
| `--no-simplify` | disable simplify regardless |

### Testing

1. All 8 existing tests pass with bottom-up algorithm
2. Deep chain test: 26-term `||` terminates in linear time
3. Cache hit test: shared subexpressions produce one simplification
4. All 22 benchmarks produce same output (--dev vs --prod)
5. officina.bv: --dev and --prod both compile successfully

---

## Implementation Order

1. Fix 6 `%` prefixes in emit_expr.rs
2. Fix `unreachable` + `br` dead code in emit_expr.rs
3. Fix `terminated` flag leak in emit_stmt.rs
4. **Verify**: `cargo build && brief build officina.bv` passes `opt`
5. Implement `--dev`/`--prod`/`--release`/`--simplify-budget` in main.rs
6. Rewrite equality_saturation.rs with bottom-up + hash-cons
7. Wire simplify pass into compile pipeline
8. Unit tests + regression
9. Benchmarks verification
10. Documentation
