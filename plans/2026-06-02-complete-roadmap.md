# Complete Optimization & Fix Roadmap — 2026-06-02

Generated from: Two independent code reviews (Review A: LlvmBackend 7-item audit;
Review B: Shared module manual audit) + iterative refinement during implementation.

---

## Review A — LLVM Backend Optimization

| # | Category | Priority | Status | Notes |
|---|----------|----------|--------|-------|
| A1 | **alloca+SROA** — replace `phi %State` with alloca+load/store | HIGH | ✅ DONE | float_math 41× improvement |
| A2 | **fast-math flags** — `fast` on all fadd/fmul/fsub/fdiv/fcmp | MEDIUM | ✅ DONE | Compounds with SROA |
| A3 | **SLP hazard fix** — union+cross-op cap | MEDIUM | ✅ DONE | Revised formula |
| A4 | **Typed SSA** — remove i64 boxing for floats | HIGH | ⏳ PENDING | Correctness + non-SROA paths |
| A5 | **Pointer provenance** — no ptrtoint/inttoptr | LOW | ⏳ PENDING | For string-heavy programs |
| A6 | **Commutativity pattern bug** — extract_trigger_keys | LOW | ⏳ PENDING | Minor redundant pattern |
| A7 | **Per-function SLP guard + `-O3`** | **HIGH** | ⏳ **NEXT** | Unlocks `opt -O3` globally |

## Review B — Shared Module Bugs

| # | Category | Severity | Status | Notes |
|---|----------|----------|--------|-------|
| B1 | **UTF-8 slicing panic** in `lib/ffi/native/src/lib.rs` | CRITICAL | ⏳ PENDING | Runtime crash on non-ASCII |
| B2 | **Entry-point value != presence** in `src/analysis/entry_point.rs` | CRITICAL | ⏳ PENDING | Wrong compile-time evaluation |
| B3 | **Assertion false-path unsound** in `src/assertion_verify.rs` | CRITICAL | ⏳ PENDING | Soundness hole |
| B4 | **Overlap detection** in `src/analysis/cross_reference.rs` | MEDIUM | ⏳ PENDING | Only checks first decl |
| B5 | **Loop overshoot** in `src/proof_engine.rs` | INFO | ⏩ SKIP | Correct as-designed |
| B6 | **Parser duplication** in `src/parser.rs` | LOW | ⏳ PENDING | Maintainability |
| B7 | **Hardcoded addresses** in `src/analysis/address_space.rs` | MEDIUM | ⏳ PENDING | Portability |

---

## Architecture: Per-Region Optimization Control

### Problem
The current pipeline uses a global `-vectorize-slp=false` flag that disables SLP
for EVERY function if ANY function is hazardous. Combined with `opt -O2`, this
means safe functions miss both SLP vectorization AND `-O3`-gated passes.

### Solution
Use `opt -O3` / `llc -O3` globally, control SLP per-function via LLVM IR
attributes. This matches Clang's native compiler model.

### Pipeline Transformation

```
BEFORE:
  estimate_slp_hazard → llvm_extra_flags.push("-vectorize-slp=false")
  main.rs: opt -O2 ... -vectorize-slp=false
  main.rs: llc -O2
  → ALL functions lose SLP. No function gets -O3.

AFTER:
  estimate_slp_hazard → slp_hazard_fns.insert("main"), insert(txn_name)
  llvm.rs: emit @main() ... #4  (if hazardous → "disable-slp-vectorize")
  llvm.rs: emit @safe_txn() ... #0  (SLP enabled)
  main.rs: opt -O3 -S        ← -O3 with NO extra flags
  main.rs: llc -O3           ← -O3 for codegen
  → Only hazardous functions lose SLP. All others get -O3 + vectorization.
```

### What `-O3` Adds Over `-O2`

| Pass | Mechanism | Our Guard |
|------|-----------|-----------|
| `SLPVectorizePass` | Pack scalar ops → vectors | ✅ Per-function `#4`/`#5` attribute |
| `LoopVectorizePass` | SIMD for counted loops | Not guarded (safe for most) |
| `LoopLoadEliminationPass` | Forward stores → loads | Pure benefit |
| Aggressive Inliner | Merge small helpers | Pure benefit |
| Aggressive Scheduler (`llc`) | Better instruction reordering | Pure benefit |

### Precision Per-Region Hazard Control

#### Function-Level (SLP)
```llvm
define void @hazardous_fn() #4 {
  ; ... SLP disabled here only ...
}
attributes #4 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
    "disable-slp-vectorize"="true"     ; LLVM 15+
    "no-vectorize-slp"="true"           ; LLVM <15
}
```

#### Loop-Level (Loop Vectorize) — Optional
```llvm
loop_body:
  br i1 %cmp, label %loop_body, label %loop_done, !llvm.loop !1
!1 = !{!1, !2}
!2 = !{!"llvm.loop.vectorize.enable", i1 false}
```

### The Phi-Scheduling Gap (float_math_nonzero)

Current 2.32× gap (0.380s vs 0.165s) is NOT from LLVM IR optimization — it's
from codegen register allocation. The manual phi nodes create live-ranges that
span the entire loop iteration, forcing the register allocator into a suboptimal
schedule vs C's local-variable pattern.

Two-layer mitigation:
1. **`llc -O3`** — more aggressive instruction scheduling may help (try first)
2. **Alloca-based loop body** — emit `alloca` + load/store instead of phis,
   letting SROA+mem2reg produce optimal register allocation (fallback)

### Implementation Guardrails

1. **Missing `HashSet` import**: Add `use std::collections::{HashMap, HashSet};`
2. **Dual attributes**: Emit both `"disable-slp-vectorize"` and `"no-vectorize-slp"`
3. **`main()` uses `#3` sometimes**: Need `#5` group that extends `#3` with SLP-disable

### Attribute Groups

```llvm
attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn
                  memory(argmem: readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }

attributes #4 = { mustprogress nofree norecurse nosync nounwind willreturn
                  memory(argmem: readwrite)
                  "disable-slp-vectorize"="true"
                  "no-vectorize-slp"="true" }
attributes #5 = { nofree norecurse nosync nounwind memory(readwrite)
                  "disable-slp-vectorize"="true"
                  "no-vectorize-slp"="true" }
```

---

## Benchmark Targets

| Benchmark | Current (`-O2`) | Target (`-O3`) | Primary Path |
|-----------|-----------------|----------------|--------------|
| float_math | 0.011s | ~0.011s | Already O(1) |
| float_math_nonzero | 0.380s | **~0.25-0.17s** | llc -O3 + alloca fallback |
| kalman_filter | 0.71s | ~0.71s | SLP #4 guard |
| iir_filter | 0.172s | ~0.17s | Tied with C already |
| ring_buffer | 0.007s | ~0.007s | Startup noise |
| async_counters | 0.004s | ~0.004s | Tied |
| sparse_dispatch | 0.077s | ~0.077s | Startup overhead |
| const_heavy | 0.006s | ~0.006s | O(1), beats C 7× |

---

## Build Order

### Phase 1 — Per-Function SLP + `-O3` (A7)
| Step | File | Change |
|------|------|--------|
| 1.1 | `src/backend/llvm.rs` | Add `HashSet` import + `slp_hazard_fns` field |
| 1.2 | `src/backend/llvm.rs` | Rewrite `estimate_slp_hazard`: insert names instead of push flag |
| 1.3 | `src/backend/llvm.rs` | Add `slp_attr()` helper |
| 1.4 | `src/backend/llvm.rs` | Emit `#4`/`#5` attribute groups (dual attributes) |
| 1.5 | `src/backend/llvm.rs` | Update all `define @fn ... #N {` to use `slp_attr()` |
| 1.6 | `src/backend/llvm.rs` | Update 6 test assertions (check IR, not `llvm_extra_flags()`) |
| 1.7 | `src/main.rs` | `opt -O2` → `opt -O3`, `llc -O2` → `llc -O3` |
| 1.8 | `src/main.rs` | Remove `extra_flags` loop + `llvm_extra_flags()` import |
| 1.9 | — | Build → test → benchmark |

### Phase 2 — P0 Bug Fixes (B1-B4)
| Step | File | Change |
|------|------|--------|
| 2.1 | `lib/ffi/native/src/lib.rs` | UTF-8 safe `__contains_at`, `__find_from`, `__utf8_len` |
| 2.2 | `src/analysis/entry_point.rs` | Real `get_initial_value_numeric()` |
| 2.3 | `src/assertion_verify.rs` | False-path exploration for Guarded |
| 2.4 | `src/analysis/cross_reference.rs` | `decls.iter().any()` instead of `decls.first()` |

### Phase 3 — P1/P2 Items
| Step | File | Change |
|------|------|--------|
| 3.1 | `src/analysis/address_space.rs` | Load address ranges from config |
| 3.2 | `src/parser.rs` | `is_keyword_identifier()` helper |
| 3.3 | `src/backend/llvm.rs` | B6: commutativity pattern fix |
| 3.4 | `src/backend/llvm.rs` | A4: typed SSA (phased separately) |
| 3.5 | `src/backend/llvm.rs` | A5: pointer provenance |
| 3.6 | `src/backend/llvm.rs` | A7 remainder: fastcc |
| 3.7 | `src/backend/llvm.rs` | Alloca-based loop body (contingency) |
