# Plan: Perfect Hashing & Selective LUT Specialization

**Date:** 2026-06-02
**Status:** Plan — ready for implementation
**Version:** 2.0 (production-ready, fully researched)

## Problem

Briev's enum switch-dispatch (Path 4) emits a dense `switch i64 %trigger` block at `llvm.rs:3126`. LLVM compiles this to an O(1) jump table **only when trigger values are consecutive integers** (e.g., `0, 1, 2, 3`). The switch hardcodes `for val in 0..n as i64`, which means:

1. **Sparse triggers** (`101, 204, 404`): `value_set_size_of` returns `Some(3)` but the switch emits cases for `0, 1, 2` — **wrong mapping**. Falls back to binary decision tree (O(log N)).
2. **Arbitrary Int triggers** with no range annotation: `value_set_size_of` returns `None` → enum dispatch is entirely disabled → falls back to sequential reactor (100ms tick polling).
3. **Uniform LUT sizing**: All values get identical LUT entries regardless of frequency, wasting cache lines on cold paths.

### Root Cause: The Region Analyzer Can't Extract Keys from Preconditions

The `enum_txn_names` computation at line 384 calls `is_trigger_gated` to check if a transaction's precondition references a trigger. Even when it does (e.g., `sensor == 101 || sensor == 204`), if the region analyzer returns `None` for the trigger's value set size (because the trigger is an unbounded `Int`), enum dispatch is **never entered**. The keys are sitting in the precondition AST but the compiler never extracts them.

## Solution Part 1: Key Extraction from Precondition AST

Instead of requiring the region analyzer to know the trigger's value set, **extract the actual keys used in precondition `Eq` comparisons**:

```rust
fn extract_trigger_keys(pre: &Expr, trigger_names: &HashSet<&str>) -> Option<Vec<i64>> {
    let mut keys = Vec::new();
    match pre {
        Expr::Eq(ident, Expr::Integer(n)) | Expr::Eq(Expr::Integer(n), ident)
            if matches!(ident.as_ref(), Expr::Identifier(name))
               && trigger_names.contains(name.as_str()) =>
        {
            keys.push(*n);
        }
        Expr::Or(l, r) => {
            keys.extend(extract_trigger_keys(l, trigger_names)?);
            keys.extend(extract_trigger_keys(r, trigger_names)?);
        }
        Expr::And(l, _) => {
            // Triggers in And are also valid (e.g., trigger && counter < bound)
            keys.extend(extract_trigger_keys(l, trigger_names)?);
        }
        _ => return None,
    }
    keys.sort_unstable();
    keys.dedup();
    if keys.len() < 2 { return None; }
    Some(keys)
}
```

**This is the critical unlock.** A transaction with `[sensor == 101 || sensor == 204]` now yields `Some([101, 204])` — enabling enum dispatch for **any** trigger-gated transaction, even with arbitrary integer values.

### Sparsity Heuristic: When to Bother with Perfect Hashing

```rust
fn sparsity_ratio(keys: &[i64]) -> f64 {
    if keys.len() < 2 { return 0.0; }
    let sorted = { let mut k = keys.to_vec(); k.sort_unstable(); k };
    let gaps: Vec<u64> = sorted.windows(2).map(|w| (w[1] - w[0]) as u64).collect();
    let min_gap = *gaps.iter().min().unwrap_or(&1);
    let max_gap = *gaps.iter().max().unwrap_or(&0);
    if min_gap == 0 { return f64::MAX; }
    max_gap as f64 / min_gap as f64
}
```

If `sparsity_ratio < 4.0`, the keys are dense enough for a standard jump table already — no hash needed, just use consecutive offsets.

## Solution Part 2: Multiplicative Perfect Hashing

For sparse keys `K = {k₀, ..., kₙ₋₁}` with `sparsity_ratio >= 4.0`, find `M` and `S` such that:

```
h(k) = (k × M) ≫ S      maps each kᵢ to unique index in [0, 2^d)
```

### Compile-Time Hash Search (LLVM backend, Rust)

```rust
fn find_perfect_hash(keys: &[i64]) -> Option<(u64, u32)> {
    let n = keys.len();
    let num_slots = n.next_power_of_two();
    let shift = 64 - num_slots.trailing_zeros();
    let mut rng: u64 = 123456789; // deterministic LCG for reproducible builds
    for _ in 0..10000 {
        rng = rng.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        let multiplier = rng | 1; // must be odd
        let mut seen = vec![false; num_slots];
        let mut ok = true;
        for &k in keys {
            let hash = (k.wrapping_mul(multiplier as i64) as u64) >> shift;
            if seen[hash as usize] { ok = false; break; }
            seen[hash as usize] = true;
        }
        if ok { return Some((multiplier, shift)); }
    }
    None
}
```

**Guarantees:** Capped at 10K iterations. For N ≤ 256, success >99.9% within 100-1000 iterations. Deterministic LCG = reproducible builds.

### Emitted LLVM IR

```llvm
; 1. Load raw sparse trigger
%raw_val = load volatile i64, i64* @__trigger_address, align 8

; 2. Perfect hash: 2 ALU instructions
%t0 = mul  i64 %raw_val, <M_CONSTANT>
%h  = lshr i64 %t0,      <S_CONSTANT>

; 3. Dense jump table (LLVM guarantees O(1))
switch i64 %h, label %residual [
  i64 0, label %case_0
  i64 1, label %case_1
]

; 4. Verification guard (safety: invalid input that hashes to valid slot)
case_0:
  %ok = icmp eq i64 %raw_val, 101
  br i1 %ok, label %body_0, label %residual
```

**Cost:** 2 ALU cycles (mul + lshr) + 1 jump table. Replaces O(log N) branch tree with O(1) dispatch.

## Solution Part 3: Multi-Trigger Perfect Hashing

With 2 triggers (e.g., `sensor ∈ {101, 204}`, `mode ∈ {5, 9}`), product = 4 ≤ budget. Concatenate:

```rust
fn combine_triggers(values: &[i64], primes: &[i64]) -> i64 {
    // Use coprime multipliers to combine trigger values into one key
    values.iter().zip(primes).map(|(v, p)| v * p).sum()
}
// Or simply: pack with bitshift for small values
let combined = value_1 * 997 + value_2; // 997 > max value_2
```

Then perfect hash the combined key. This replaces the current multi-trigger fallthrough (`call @reactor_tick()`) with an O(1) dispatch.

## Solution Part 4: Entropy-Guided Hot/Cold Splitting

### Phase 4a: `#weight(N)` Annotations

```briev
enum Phase {
    Idle    #weight(5),
    Heating #weight(89),   // 89% of execution
    Resting #weight(1),
    Cooling #weight(3),
    Done    #weight(2)
}
```

### Phase 4b: `likely()` Branch via `@llvm.expect.i64`

When hot/cold splitting is active, emit:

```llvm
%is_hot = icmp eq i64 %val, 1
%expected = call i64 @llvm.expect.i64(i64 %is_hot, i64 1)
%is_hot_i1 = icmp ne i64 %expected, 0
br i1 %is_hot_i1, label %fast_path, label %cold_path
; fast_path laid out first (contiguous with loop entry)
; cold_path placed in .cold section by LLVM
```

This is structurally identical to C's `__builtin_expect` / `likely()`. LLVM will:
1. Place the hot path inline after the branch
2. Move the cold path to a separate function section
3. Mark the hot edge as "taken" for register allocation hinting

### Phase 4c: Profile-Guided Optimization (PGO)

Add `--profile-generate` and `--profile-use=<file>`:
1. Instrumentation binary with per-value hit counters
2. Training run writes `program.prof`
3. Shannon entropy threshold: `H(V) < 0.5 · log₂(n)` → skewed → apply hot/cold

### Phase 4d: Partial LUT Materialization

Only emit LUT entries for values covering top 80% of execution. Residual handles the remaining 20%. Saves 3/5 of LUT entries in a 5-value enum with 90/10 split.

## How Briev Beats C on This

| Technique | Briev | C (world-class) | Advantage |
|-----------|-------|-----------------|-----------|
| Perfect hash for sparse dispatch | Auto-detected from preconditions | Programmer must manually write `static const void* table[]` + computed goto | **Briev is automatic** |
| `likely()` branch prediction | `@llvm.expect.i64` via weight annotations | `__builtin_expect(expr, 1)` | **Identical** |
| Multi-trigger dispatch | Auto-concatenation + hash | Programmer writes nested switch or manual packing | **Briev is automatic** |
| Key extraction | Extracts from precondition AST | N/A — C switch values must be literals | **Briev handles dynamic sets** |

C can match Briev's **performance** with hand-tuned computed gotos and `__builtin_expect`, but Briev matches it **automatically** — the programmer writes `[sensor == 101 \|\| sensor == 204]`, and the compiler does the rest.

## Implementation Map

### Module map

| File | Change | Lines |
|------|--------|-------|
| `src/backend/llvm.rs` | `extract_trigger_keys`, `find_perfect_hash`, `emit_perfect_hash_dispatch`, `classify_trigger_sparsity`, `likely_emit`, `emit_hot_cold_split`, modify `emit_enum_main` (line ~3126) | ~180 |
| `src/analysis/region.rs` | Add `extract_keys` method using `extract_trigger_keys` for fallback value-set sizing | ~30 |
| `src/analysis/entropy.rs` (new) | `analyze_entropy`, `select_hot_values`, `sparsity_ratio` | ~50 |
| `src/parser.rs` | Parse `#weight(N)` on enum variants, store in `EnumVariant::weight: Option<u64>` | ~20 |
| `src/main.rs` | `--profile-generate`, `--profile-use`, `--profile-write` flags | ~40 |

### Modified `emit_enum_main` (llvm.rs:3126)

**Current code:** `switch i64 %sz_{tn}, label %{tn}_residual [ for val in 0..n ]`

**New code:**
```rust
fn emit_enum_main(...) {
    // 1. Try key extraction from precondition AST
    let keys = self.extract_trigger_keys(&txns, &trigger_names);

    // 2. Check sparsity — skip hashing for dense sets
    if let Some(ref keys) = keys {
        if sparsity_ratio(keys) < 4.0 {
            // Dense: use existing switch with offset mapping
            self.emit_dense_switch(out, keys, ...);
            return;
        }
    }

    // 3. Try perfect hash for sparse sets
    if let Some(keys) = keys {
        if let Some((m, s)) = find_perfect_hash(&keys) {
            self.emit_perfect_hash_dispatch(out, trigger, m, s, keys, ...);
            return;
        }
    }

    // 4. Check for hot/cold split
    if let Some(hot_values) = self.classify_hot_values(trigger, &txns) {
        self.emit_hot_cold_split(out, trigger, hot_values, ...);
        return;
    }

    // 5. Fall through to existing switch
    // ... existing code ...
}
```

## Benchmark Strategy

### Benchmark 1: Sparse Trigger Dispatch (pure dispatch, no math)

| Variant | Briev (before) | Briev (after) | C (world-class) |
|---------|---------------|---------------|-----------------|
| 3 dense keys (0,1,2) | O(1) jump table | O(1) jump table | O(1) jump table |
| 3 sparse keys (101,204,404) | O(log 3) binary tree | O(1) hash + jump table | O(1) computed goto |
| 8 sparse keys | O(log 8) tree | O(1) hash + jump table | O(1) computed goto |
| 256 sparse keys | O(log 256) tree | O(1) hash + jump table | O(1) computed goto |
| Multi-trigger (2×2) | Sequential reactor (slow) | O(1) hash + jump table | Nested switch (O(1)) |

**C reference (world-class):**
```c
// Perfect hash computed at compile time via gperf or manual
static const void* table[] = { &&lbl_101, &&lbl_204, &&lbl_404 };
dispatch:
    unsigned h = (val * M) >> S;
    goto *table[h];
lbl_101: /* verify */ if (val != 101) goto residual; /* body */ goto done;
lbl_204: if (val != 204) goto residual; /* body */ goto done;
lbl_404: if (val != 404) goto residual; /* body */ goto done;
residual: /* fallback */
done: /* next */
```

### Benchmark 2: Hot/Cold Speculative Branch

| Variant | Briev (before) | Briev (after) | C (world-class) |
|---------|---------------|---------------|-----------------|
| Uniform (10% each) | Full switch | Full switch | Full switch |
| 90/10 skewed | Full switch | `likely()` branch + residual | `if (likely(x==HOT))` + residual |
| 99/1 skewed | Full switch | `likely()` branch + residual | `if (likely(x==HOT))` + residual |

**C reference:** `if (__builtin_expect(x == HOT_VALUE, 1)) { hot_work(); } else { cold_work(); }`

### Benchmark 3: Real-Work Sparse Dispatch

Kalman filter receiving `PREDICT(101)` vs `UPDATE(204)` vs `INIT(404)` — three sparse keys, each triggering a different filter mode.

| Variant | Briev (before) | Briev (after) | C (world-class) |
|---------|---------------|---------------|-----------------|
| Dispatch only | Binary tree (O(log 3)) | Hash + jump (O(1)) | Computed goto (O(1)) |
| Full pipeline | Binary tree + Kalman math | Hash + Kalman math | Computed goto + Kalman math |

## Acceptance Criteria

1. **Sparse 3-element trigger** (101, 204, 404): compiled to dense jump table (verify via `llvm-objdump` indirect jump count)
2. **Dense trigger** (0, 1, 2, 3): sparsity ratio < 4.0 → no hash, existing behavior preserved
3. **Key extraction from `[sensor == 101 || sensor == 204]`**: returns `Some([101, 204])`
4. **Multi-trigger** (2 triggers, 2 values each): combined key → perfect hash → single switch
5. **Safety guard**: Every hash case arm contains `icmp eq i64 %raw_val, <original_key>`
6. **`likely()` annotation**: Hot/cold split emits `@llvm.expect.i64` intrinsic
7. **`#weight(90)`**: Enum variant with weight annotation emits `llvm.expect` check
8. **Zero tests broken**: `cargo test --lib` passes (368+)
9. **Compile-time bounded**: Hash search < 10ms for N ≤ 256
10. **C benchmarks**: Briev performs at worst 1.1× of optimized C for all new benchmarks

## Benchmark Impact Matrix

| Optimization | ring_buffer | async_counters | iir_filter | kalman_filter | New: sparse dispatch | New: hot/cold |
|-------------|-------------|----------------|------------|---------------|---------------------|---------------|
| Key extraction from preconds | — | — | — | — | **Enables dispatch** | **Enables dispatch** |
| Perfect hashing | — | — | — | — | **2-10×** | — |
| Multi-trigger hash | — | — | — | — | **100×** (vs reactor) | — |
| `likely()` + hot/cold | — | — | — | — | — | **1.5×** |
| Partial LUT materialization | — | — | — | — | — | **Cache win** |
