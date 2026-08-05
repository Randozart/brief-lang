# IR Determinism & Benchmark Strategy

Date: 2026-07-06
Status: Plan

## 1. State of Benchmark Determinism

### Loop bounds (18 benchmarks audited)

| Source | Bound type | Mechanism | Precomputable? |
|--------|-----------|-----------|----------------|
| 17/18 | **Runtime** | `__get_env_int("BOUND")` | No — FFI opaque |
| bit_clear.bv | **Compile-time** | `const initial_reg = 0x7FFFFFFFFFFFFFFF` | Yes — 63 iterations by design |

All runtime-bound benchmarks are already "the same between runs and between C
and Briv" and "not so deterministic that the compiler can predict at compile
time." No changes needed for the benchmark sources.

### What's left: IR non-determinism

The same Briv source compiled twice produces different LLVM IR because Rust's
`HashMap` iterates in non-deterministic order (SipHash random seed per process).
This causes ~5-10% benchmark-to-benchmark performance variation on the same
machine — not from CPU throttling but from different machine code each compile.

Repeated identical-compiler runs of `nbody_sqrt_idio` show a range of 2.87s to
3.14s (9% swing) despite identical source, identical BOUND, and stable C times.

## 2. Root Sites of Non-Determinism

All in `src/backend/llvm/loop_engine.rs`:

| Site | Line (approx) | HashMap | Effect |
|------|--------------|---------|--------|
| Phi header emission | 988-996 | `field_index_map` | Order of `%phi_<name>` declarations |
| Phi reg assignment | 1013-1033 | `vector_phi_groups` | Which fields get which phi register |
| ssa_old setup | 1150-1163 | `phi_field_regs` | Order of `ssa_old_float_regs` inserts |
| Backedge latch | 1191-1197 | `backedge_field_regs` | Order of `%be_<name> = fadd 0.0` ops |
| Commit block | 1634-1635 | `last_val_temps` | Order of `store %phi, ptr %temp` |
| post-loop loads | 2523-2529 | `done_needs_fields` | Order of `load` in done: block |

## 3. Fix Strategy

Sort every critical HashMap/Set iteration by field name at the collection site.
Use `.iter().sorted_by_key(|(k, _)| k.clone())` or collect into `Vec` and sort.

### Rule of thumb

Sort when the iteration produces LLVM IR instructions whose order differs
across compilations. Don't sort when the iteration is only used for lookup
(no emission).

### Affected functions

1. `emit_countable_setup_phis_and_header` (line ~988):
   `all_fields` → sort by `name` before phi emission

2. `emit_countable_latch` (line ~1191):
   `backedge_entries` → sort by `name` before backedge emission

3. `phi_regs_to_ssa_old` (line ~1150):
   `phi_field_regs` → sort by `name` before ssa_old inserts

4. Commit block (line ~1634):
   `last_val_temps` → sort by `field_name` before store emission

5. `load_last_val_temps` (line ~2523):
   `done_needs_fields` → sort by `field_name` before load emission

### Implementation pattern

```rust
// Before:
for (name, phi_reg) in &self.fun.phi_field_regs {

// After:
let mut sorted_regs: Vec<(String, String)> = self.fun.phi_field_regs.iter()
    .map(|(k, v)| (k.clone(), v.clone()))
    .collect();
sorted_regs.sort_by_key(|(k, _)| k.clone());
for (name, phi_reg) in &sorted_regs {
```

## 4. Verification

### After each change
1. `cargo build --release` — no warnings
2. `cargo test --lib` — all 1403 pass

### After all changes
3. Compile a benchmark twice, `diff` the IR — should be byte-identical
   (modulo register numbering from `txn_counter` which is sequential and
   deterministic given the same AST traversal order)
4. `bash benchmarks/build_and_bench.sh --runtime` — all MATCH
5. Run twice: ratios should be within ±1% instead of current ±9%

## 5. Risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| Sorting changes phi order → different LLVM optimization | Performance could shift across the board | Run full suite before/after; compare ratios |
| Sorting adds O(n log n) per callite | Negligible for n ≤ 100 fields | 31 fields for nbody, log2(31) ≈ 5 comparisons |
| Chunk layout changes from deterministic order | Same as above | Same mitigation |

## 6. Secondary: bit_clear.bv precomputability

`bit_clear.bv` has `const initial_reg = 0x7FFFFFFFFFFFFFFF` and iterates 63
times. With `--optimize-budget 256`, the compiler can fold up to 256 simulation
steps. Since 63 < 256, the entire loop CAN be precomputed. Currently it takes
0.0006s suggesting it IS precomputed.

This is by design — bit_clear is a synthetic correctness test, not a runtime
benchmark. It should remain compile-time constant. The benchmark harness
detects precomputed binaries by `.text` size ratio and skips timing.

If we wanted to prevent precomputation, we'd change it to use
`__get_env_int("BOUND")` — but this would make it a runtime benchmark with
unnecessary overhead (63 iterations would be dominated by FFI overhead).

**Decision**: Keep bit_clear as-is. It's already handled by the harness.

## 7. Implementation Order

1. Sort `all_fields` in `emit_countable_setup_phis_and_header`
2. Sort `phi_field_regs` in `phi_regs_to_ssa_old`
3. Sort `backedge_entries` in `emit_countable_latch`
4. Sort `last_val_temps` in commit block
5. Sort `done_needs_fields` in `load_last_val_temps`
6. Build, test, IR diff
7. Benchmark run × 2 to verify consistency
8. Document in architecture/overview.md

## Implementation Result

Applied in commit `139c345`. 6 sorting sites, 25 lines added, 6 modified.
All 1403 tests pass. All 22 benchmarks MATCH (0 MISMATCH).

nbody_sqrt_idio at **.65x** (2.67s) — nearly identical to Config A's
poison-backedge performance (.64x, 2.60s) but with COMPLETELY CORRECT
values. Deterministic phi ordering improves LLVM's SROA decomposition
vs Config B's non-deterministic run-to-run average (.70x–.81x).

### Not fixed (out of scope)

Global constant ordering and metadata indexing still use HashMap iteration
in the parser, type checker, and import resolver. Full IR determinism would
require sorting at ~20 additional sites. The remaining non-determinism is
primarily in constant alias vs definition selection and `!range` metadata
indexing — neither affects benchmark performance.
