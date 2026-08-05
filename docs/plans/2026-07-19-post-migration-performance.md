# Post-Migration Performance Optimization

**Date:** 2026-07-19
**Prerequisite:** Intrinsic migration + stabilization complete (23/24 MATCH)
**Gaps:** nbody_newton 2.2× regression, UTF8_ops MISMATCH, HashMap non-determinism

---

## Priority 1: HashMap Determinism (6 sites)

**Pattern for all:** collect keys, sort, iterate sorted.

| File | Line | HashMap | Fix |
|------|------|---------|-----|
| `emit_toplevel.rs` | 640 | `cell_state_types` | `.keys().collect()` → `.sort()` → iterate |
| `emit_toplevel.rs` | 989 | `cache_slots` | Same pattern |
| `emit_toplevel.rs` | 2174 | `cell_defs` | `.iter().collect()` → `.sort_by_key(\|_, n\| n)` |
| `mod.rs` | 2126 | `constants` | `.iter().collect()` → `.sort()` |
| `mod.rs` | 2158 | `constants` | Same (second iteration) |
| `mod.rs` | 2355 | `cell_defs` | `.values().collect()` → `.sort_by_key(\|d\| d.name)` |

## Priority 2: Arena Allocator for Reactive Txns (UTF8_ops Tier 1)

**Problem:** `emit_transaction` in `emit_toplevel.rs` has two paths — the callable-txn path calls `emit_arena_init`, the standard reactive path does not. Every `Alloc#(8)` falls through to `@malloc(8)`.

**Fix:** Add `emit_arena_init` to the standard path (~line 1428) before the body emission, and `emit_arena_fini` (already present at line 1459). Also set `is_static_bound` when the precondition is `x < N` with a known bound.

**Impact:** 50-100× speedup for UTF8_ops.

## Priority 3: Auto-Inlining Small Callable Txns (UTF8_ops Tier 2)

**Problem:** `memcmp_loop` is a callable txn that iterates byte-by-byte. It's emitted as a separate function without `alwaysinline`, so LLVM can't optimize across the call boundary.

**Fix:** In `emit_callable_txn`, if the function meets these criteria:
- Has few parameters (< 8)
- Has no frgn calls
- Is not async
- Has a small body (< 20 statements)

Then emit `alwaysinline` on the function. This is better than manual `#inline` annotations — the compiler detects the pattern automatically.

**Impact:** 5-10× speedup for memcmp-heavy workloads.

## Priority 4: Constant-Length Fast Path for memcmp (UTF8_ops Tier 3)

**Problem:** `memcmp(a, b, 8)` compares 8 bytes one at a time via `Load#(addr, 1)`. Each byte load goes through the full LLVM intrinsic path.

**Fix:** In `lib/std/types/UTF8view.bv`, add a fast path for `len == 8` that loads both values as `i64` via `Load#(a, 8)` and compares with a single `icmp`. This is safe because SSO strings store inline data packed as i64.

## Priority 5: Native Float Types in State (nbody_newton)

**Problem:** All state fields are emitted as `i64` in `%State`. Float32 fields need 4× instructions per access (GEP→load→trunc→bitcast).

**Fix:** The chunk allocator (`emit_inline_init_stores` / chunk type emission) should use the field's Briv type to determine the LLVM type. Float32 → `float`, Float64 → `double`, Int → `i64`. This restores the Phase 3 behavior where `%StateChunk0 = type { i64, i64, float, float, ... }`.

**More complex — needs careful handling of:** struct field alignment, GEP index calculations, and the `adapt_to_i64` boxing/unboxing in phi backedge handling.
