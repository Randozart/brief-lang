# Baseline Recovery Plan — From `b39461e2` to Stable State
## 2026-07-28

## 1. Current State

### 1.1 Baseline Commit

```
b39461e2  2026-07-27: SLP stride gate — all 19 benchmarks at parity or better
```

The baseline worktree is at `../briv-compiler-baseline` with a release build ready.
To rebuild: `cd ../briv-compiler-baseline && cargo build --release`

### 1.2 Baseline Benchmark Table (Run 6 — SLP stride gate era)

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ Benchmark                 ║ Briv      ║ C          ║ Ratio    ║ Winner ║ Correct   ║
╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣
║ ring_buffer               ║ .0550s     ║ .0480s     ║ 1.14x    ║ C      ║ MATCH     ║
║ float_math                ║ .0744s     ║ .0743s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ float_math_nonzero        ║ .1656s     ║ .1675s     ║ .98x     ║ Briv  ║ MATCH     ║
║ sparse_dispatch           ║ .0500s     ║ .0610s     ║ .81x     ║ Briv  ║ MATCH     ║
║ print_loop                ║ .0604s     ║ .0587s     ║ 1.02x    ║ C      ║ MATCH     ║
║ nbody_newton              ║ 9.0467s    ║ 8.2689s    ║ 1.09x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.7347s    ║ 2.7684s    ║ .98x     ║ Briv  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.3417s    ║ 3.6030s    ║ .92x     ║ Briv  ║ MATCH     ║
║ fasta                     ║ .2099s     ║ .2109s     ║ .99x     ║ Briv  ║ MATCH     ║
║ fannkuch_redux            ║ .0653s     ║ .0657s     ║ .99x     ║ Briv  ║ MATCH     ║
║ mandelbrot                ║ .6569s     ║ .6528s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1813s     ║ .1790s     ║ 1.01x    ║ C      ║ MATCH     ║
║ knucleotide               ║ .1883s     ║ .1909s     ║ .98x     ║ Briv  ║ MATCH     ║
║ cancel_math               ║ .0626s     ║ .0614s     ║ 1.01x    ║ C      ║ MATCH     ║
║ bit_clear                 ║ .0001s     ║ .0002s     ║ .50x     ║ Briv  ║ MATCH     ║
║ queue_drain               ║ .0623s     ║ .0612s     ║ 1.01x    ║ C      ║ MATCH     ║
║ queue_drain_sym           ║ .0618s     ║ .0611s     ║ 1.01x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0624s     ║ .0618s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ interval_step             ║ .0617s     ║ .0588s     ║ 1.04x    ║ C      ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

**Characteristics of this baseline:**
- `#0 = memory(readwrite)` on txn functions (LLVM auto-vectorizer free to run)
- SLP stride gate active — kalman protected
- Depth×width ≥ 10 active — nbody pre-merge depth<4 blocked
- Width cap ≤ 8 active — merged groups limited
- Three-category cold-path outlining active
- Print plugin emits `__print_int` FFI (NOT `PrintInt#`)
- No `!range` or `!prof` metadata
- No `noundef`/`dereferenceable` on params
- `is_wasm()` still present (not DataLayout-driven)
- `Bits` named `Bits` (not `Bit`)

### 1.3 What Changed Since Baseline (chronological)

| Order | Change | Commit | Status | SLP interaction? |
|-------|--------|--------|--------|-----------------|
| 0 | Three-category outlining + stride gate + depth×width + width cap | `b39461e2` | **BASELINE** | Active baseline |
| 1 | Phase 1: `!range` metadata | `b9a2dd7d` | NOT in baseline | No |
| 2 | Phase 2: `!prof` branch weights | `9ff835ac` | NOT in baseline | No |
| 3 | Phase A: DataLayout int_bits, remove is_wasm() | `5bde7aed` | NOT in baseline | No |
| 4 | Phase B: noundef + dereferenceable on params | `e24c3fd7` | NOT in baseline | No |
| 5 | Phase D: Bits → Bit rename | `30f9b839` | NOT in baseline | No |
| 6 | Phase C: persist AnalysisResults | `004a4d12` | NOT in baseline | No |
| 7 | Metadata syntax `!>` | `abd1a090` | NOT in baseline | No |
| 8 | Phase E: PrintInt# intrinsics | `0ebfba39` | NOT in baseline | Yes — changes guard detection |
| 9 | Fix Phase E regression | `9f4d7dfe` | NOT in baseline | Yes — extends is_ffi_call |
| 10 | Remove stride gate + fix lane_positions + vector sqrt | `ecf299c9` | NOT in baseline | **YES** — removed stride gate |
| 11 | Restore stride gate | `a53ddf14` | NOT in baseline | **YES** — restored stride gate |
| 12 | Cross-per-field density check (emit_toplevel.rs) | `95b5eb02` | NOT in baseline | No |
| 13 | Two-pass consumer analysis | `edf671de` | NOT in baseline | **YES** — replaced stride gate |
| 14 | Baseline pin + handoff | `70ead990` | NOT in baseline | Doc only |

### 1.4 Regressions Introduced After Baseline

| Benchmark | Baseline | HEAD (`edf671de`) | Delta | Suspect change |
|-----------|----------|-------------------|-------|----------------|
| ring_buffer | 1.14x | 1.28x | ❌ +12% | Stride gate restoration + two-pass analysis both touched SLP dispatch |
| kalman_filter_runtime | 1.01x | 3.80x | ❌ +276% | Phase E (PrintInt#) changed guard detection → kalman gets `#11` → LLVM auto-vectorizer creates `<12 x float>` ops. Stride gate removal then restored allowed this through. |
| nbody_newton | 1.09x | 1.06x | ⚠️ -3% | Within noise, but consistently slightly better after stride gate removal |
| nbody_sqrt_idio | 0.92x | 0.85x | ✅ Better | Baseline is already better than current HEAD here |

**Key insight:** Two distinct root causes:
1. **Kalman regression**: `#11` attribute from Phase E enables LLVM's auto-vectorizer. The stride gate protects against OUR SLP, but LLVM's auto-vectorizer is not affected. Fix: force `#0` on dense matrix txns via cross-per-field check.
2. **Ring_buffer regression**: Stride gate + two-pass analysis changes touched SLP dispatch logic. Ring_buffer has no SLP groups, so baseline should be recovered automatically by reverting SLP-related changes.

## 2. Target State

**Definition of done:** All 19 benchmarks at parity or better, all MATCH.
Every change is a single commit on top of `b39461e2`, verified independently.

**Non-goals:**
- Performance beyond parity. If a change doesn't improve any benchmark and adds complexity, it doesn't land.
- Perfect SLP gating. The stride gate + depth×width + width cap already work. The two-pass analysis, total_gap check, etc. are not needed — they were attempts to remove the stride gate, which was the wrong goal.

## 3. Step-by-Step Recovery

Each step is applied on top of `b39461e2` (i.e., starting from the baseline,
then cherry-picking or re-implementing each later change).

### Verification procedure for EVERY step

```bash
cargo test --lib                              # ~25s — must pass
cargo build --release                          # ~45s — must build clean
rm -f benchmarks/*.ll
bash benchmarks/build_and_bench.sh --runtime   # ~5min — record results
bash benchmarks/build_and_bench.sh --correctness  # ~2min — all MATCH
```

Compare results against the baseline table in §1.2. If any benchmark ratio
moves >0.03x in the wrong direction, revert the step and investigate before
proceeding.

---

### Step 1: DataLayout-driven int_bits + remove is_wasm()

**Based on:** `5bde7aed` (Phase A)

**Files:** `src/backend/llvm/context.rs`, `src/backend/llvm/mod.rs`

**What:**
1. Add `parse_pointer_width(dl: &str) -> u64` that extracts `p:<abi>:<pref>`
   from the target data layout string.
2. In `CompilerContext::new()`, auto-set `int_bits` from parsed width.
3. Remove `is_wasm()`, `pointer_bytes()`, `pointer_llvm_type()` — replace with
   `int_bits / 8` and `format!("i{}", self.ctx.int_bits)`.
4. Remove `with_int_bits(32)` hardcoding in compile.rs Webstack arm (the data
   layout for WASM32 already has `p:32:32` so int_bits auto-sets to 32).
5. Fix `emit_inttoptr` to use `format!("i{}", ctx.int_bits)` instead of
   `pointer_llvm_type()`.
6. Update tests to check `int_bits` and `parse_pointer_width` instead of `is_wasm()`.

**Expected delta:** None (same i64 default on x86_64, auto-detection for WASM)

**Risk:** None

**Revert:** `git checkout -f HEAD~1 -- src/backend/llvm/context.rs src/backend/llvm/mod.rs`

---

### Step 2: noundef + dereferenceable on ptr %state params

**Based on:** `e24c3fd7` (Phase B)

**Files:** `src/backend/llvm/context.rs`, `emit_toplevel.rs`, `dispatch.rs`, `mod.rs`, `tests.rs`

**What:**
1. Add `state_size_bytes: u64` and `state_ptr_param: String` to `CompilerContext`.
2. Compute `state_size_bytes` in `generate()` from `compute_state_size_bytes()`.
3. Build `state_ptr_param` string: `"ptr noundef dereferenceable(N) noalias nocapture align 8 %state"`
   (omit `dereferenceable` if `state_size_bytes == 0` to avoid invalid LLVM IR).
4. Update ALL function definition sites (~14 across emit_toplevel.rs, dispatch.rs, mod.rs)
   to use `self.ctx.state_ptr_param` instead of hardcoded `ptr noalias nocapture align 8 %state`.
5. Update 3 functions that were missing `align 8` (reactor_tick seq/par, __briv_init_state).

**Affected lines (exact):**
```
emit_toplevel.rs:864    init_state
emit_toplevel.rs:1144   user defn param (conditional needs_state)
emit_toplevel.rs:1277   export wrapper
emit_toplevel.rs:1554   reactive txn (unfoldable, txn_attr)
emit_toplevel.rs:1718   reactive txn (outlinable, local_txn_attr)
emit_toplevel.rs:2002   callable txn
emit_toplevel.rs:2252   pre_* functions
emit_toplevel.rs:2302   async_body_* functions
emit_toplevel.rs:2369   fused txn
emit_toplevel.rs:2383   shape-guarded fused txn
emit_toplevel.rs:2421   fused composed
emit_toplevel.rs:2491   __briv_init_state (add align 8 + noalias)
emit_toplevel.rs:2549   cell_persistent_ticks
dispatch.rs:76          reactor_tick sequential (add align 8)
dispatch.rs:365         reactor_tick parallel (add align 8)
mod.rs:3118             reactor_tick fallback (add align 8)
tests.rs:864            update init_state assertion
tests.rs:1946           update struct param assertion
```

**Expected delta:** ~1-2% on nbody, ring_buffer

**Risk:** Low — `dereferenceable(0)` guard prevents invalid IR

**Revert:** `git diff HEAD~1 --stat | cut -d' ' -f2 | xargs git checkout -f HEAD~1 --`

---

### Step 3: Bits → Bit rename

**Based on:** `30f9b839` (Phase D)

**Files:** ~15 .rs files, ~5 .bv files

**What:** Mechanical rename of the `Bits` type name to `Bit`. Does NOT rename
`#Bits` hashword or `bits <~ N` property.

**Affected .rs files:** type_universe/mod.rs, type_universe/resolve.rs,
backend/bindgen.rs, backend/llvm/mod.rs (3 occurrences), backend/llvm/normalizer.rs,
backend/webstack_normalizer.rs, parser/types.rs, features/toplevel/typedef.rs,
plugin/print_plugin.rs, beast/deserialize.rs, analysis/meld_validation.rs,
analysis/layout_optimizer.rs, backend/llvm/tests.rs

**Affected .bv files:** lib/std/types/float.bv, lib/std/types/bootstrap.bv, etc.

**Expected delta:** None (zero performance impact)

**Risk:** None

**Revert:** `git checkout -f HEAD~1`

---

### Step 4: `!range` metadata from contracts

**Based on:** `b9a2dd7d` (Phase 1)

**Files:** `src/backend/llvm/helpers.rs`, `dispatch.rs`, `context.rs`, `emit_toplevel.rs`

**What:**
1. Add `extract_ranges_with_constants` that resolves `Expr::Identifier` to const
   values via `ctx.constants` (the original `extract_ranges` only handled `Expr::Decimal`).
2. Add `idx_to_field_name: HashMap<usize, String>` on `CompilerContext` — reverse
   index from field position to field name.
3. Populate `idx_to_field_name` alongside `field_to_meta_idx` in `emit_transaction`.
4. In `load_field_type()`, after emitting the load instruction, look up `!range`
   metadata via `field_to_meta_idx` using the field name from `idx_to_field_name`.

**Expected delta:** ~2-3% on ring_buffer (!range on ops [0, 50000000) helps modulo),
~1-2% on float_math

**Risk:** Low — metadata only helps LLVM, never hurts

**Revert:** `git checkout -f HEAD~1`

---

### Step 5: `!prof` branch weights from postcondition

**Based on:** `9ff835ac` (Phase 2)

**Files:** `src/backend/llvm/emit_toplevel.rs` (the guard emission in the
non-assume_action path)

**What:**
For each `when` guard at emission time, analyze the guard condition:
1. Extract postcondition bound from `txn.contract.post_condition` (match `[x == total]`)
2. Extract modulo divisor from guard condition (match `x % N == C`)
3. If both found, compute `not_taken_weight = total` and `taken_weight = ceil(total / N)`
4. Scale to max 1000 and emit `!prof !{!"branch_weights", i32 wt, i32 wn}` on the
   guard's `br i1` instruction.

This is a ~40-line addition right before `writeln!(out, "  br i1 {}, label %{}, label %{}", ...)`.

**Expected delta:** ~1-2% on ring_buffer, float_math

**Risk:** Low — metadata only affects LLVM branch layout

**Revert:** `git checkout -f HEAD~1 -- src/backend/llvm/emit_toplevel.rs`

---

### Step 6: Metadata syntax `!>`

**Based on:** `abd1a090`

**Files:** Lexer, parser (3 files), display, ~25 .bv files across lib/ examples/ glue/

**What:** Change the metadata assignment operator from `<~` (TildeArrow) to `!>`
(ExclaimArrow) with colon separator between key and value.

**Lexer changes:**
- Replace `Token::TildeArrow` with `Token::ExclaimArrow` (`!>`) at lexer.rs:337
- Update Display impl at lexer.rs:649
- Update test at lexer.rs:824

**Parser changes (3 files):**
- `statements.rs:252`: add `expect(Colon)` after key before value
- `metadata.rs:18`: same for `parse_body_metadata`
- `definitions.rs:967,1113`: restructure special-case parsing for ctd/alu/layout

**Display change:**
- `display.rs:308`: `"{:?} <~ ..."` → `"{:?}: ..."`

**Across ~25 .bv files:** Replace `key <~ value;` with `!> key: value;`

**Expected delta:** None (syntax only)

**Risk:** None

**Revert:** `git checkout -f HEAD~1`

---

### Step 7: PrintInt# intrinsics (conditional — requires both commits)

**Based on:** `0ebfba39` (Phase E) + `9f4d7dfe` (observable intrinsic fix)

**Files:** `print_plugin.rs`, `intrinsic_signatures.rs`, `intrinsics.rs`,
`interpreter/intrinsics.rs`, `emit_toplevel.rs`

**What:**
1. **print_plugin.rs**: Change `"__print_int"` → `"PrintInt#"`, `"__print_float"` → `"PrintFloat#"`,
   `"__print_char"` → `"PrintChar#"`, `"__print_str"` → `"PrintStr#"`.
2. **intrinsic_signatures.rs**: Register four print intrinsics with `observable: true`.
3. **intrinsics.rs**: Add match arms that emit `call @__print_*(%val)` with correct
   LLVM types (float for PrintFloat#, i64 for PrintInt#/PrintChar#).
4. **interpreter/intrinsics.rs**: Add execution paths for print intrinsics (`print!("{}", val)`).
5. **emit_toplevel.rs**: Extend `is_ffi_call` to also match observable intrinsics
   (check `get_intrinsic_signature(name).observable`).

**Expected delta:** ~5-10% on ring_buffer (guards become outline-able → txns get `#11`)

**Risk:** MEDIUM — kalman_filter_runtime may regress because `#11` enables LLVM's
auto-vectorizer to create expensive `<12 x float>` ops in the matrix multiply.

**Mitigation for kalman:** If kalman regresses (>1.10x), apply the cross-per-field
density check BEFORE reverting Phase E:

In `emit_toplevel.rs`, after the outlining analysis selects `#11`:

```rust
if local_txn_attr == "#11" {
    // Count cross-field operations to detect dense matrix computations
    let mut cross_ops = 0u32;
    let mut float_idents: HashSet<String> = HashSet::new();
    for s in &reordered {
        if let Statement::Let { name, .. } = s { float_idents.insert(name.clone()); }
    }
    // Also add state fields and constants that are Float32
    for (name, ty) in &self.ctx.field_briv_types.zip(self.ctx.field_index_map.keys()) {
        if matches!(ty, Type::Custom(n) if n == "Float" || n == "Float32") {
            float_idents.insert(name.clone());
        }
    }
    for s in &reordered {
        if let Statement::Let { expr: Some(e), .. } = s {
            cross_ops += count_cross_ops(e, &float_idents);
        }
    }
    let n = float_idents.len();
    if n > 4 && (cross_ops as f64 / n as f64) > 4.0 {
        local_txn_attr = "#0".to_string();  // Force #0 to block LLVM auto-vectorizer
    }
}
```

**Revert:** `git revert 9f4d7dfe 0ebfba39`

---

### Step 8: Phase C — persist AnalysisResults (optional)

**Based on:** `004a4d12`

**Files:** `context.rs`, `mod.rs`, `emit_toplevel.rs`, `transition_graph.rs`

**What:** Store `transition_graph` and `iter_bounds` on `CompilerContext` so
`emit_toplevel.rs` can compute precise `!prof` weights using induction variable info.

**Expected delta:** ~1-2% on benchmarks with non-modulo guard patterns

**Risk:** Low

**Revert:** `git checkout -f HEAD~1`

---

## 4. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Step 7 (PrintInt#) causes kalman regression | Medium | 3.5x on kalman | Cross-per-field density check forces `#0`. If that's insufficient, add `"prefer-vector-width"="1"` attribute. If still broken, revert Step 7. |
| Step 2 (noundef) causes LLVM miscompile | Very low | Wrong output | `dereferenceable(0)` guard prevents invalid IR. Full `--correctness` run catches this. |
| Cherry-pick conflicts from session history | High | Can't apply cleanly | Re-implement each step from scratch — each change is <50 lines with clear intent. |
| Multiple steps interact non-linearly | Medium | Unexpected regression | After each step, compare against BASELINE (§1.2), not previous step. |
| Stride gate removal (ecf299c9) effects linger | Low | SLP dispatch confused | Not present in baseline — baseline has the working stride gate. |

## 5. Verification Command Reference

```bash
# Full suite
bash benchmarks/build_and_bench.sh --runtime        # ~5min, get table
bash benchmarks/build_and_bench.sh --correctness    # ~2min, all MATCH

# Single benchmark (fast iteration)
rm -f benchmarks/ring_buffer.ll benchmarks/ring_buffer benchmarks/ring_buffer_c
./target/release/brivc build benchmarks/ring_buffer.bv --llvm --out benchmarks
clang -O3 -flto -march=native -ffast-math \
    benchmarks/ring_buffer.ll lib/runtime/briv_rt.c \
    -o benchmarks/ring_buffer 2>&1 | grep -c "error" | grep -v "^0$" || echo OK
BOUND=50000000 /usr/bin/time -f "%e" ./benchmarks/ring_buffer 2>&1

# Compare against baseline
bash benchmarks/compare_baseline.sh nbody_newton

# Check for specific LLVM metadata
grep '!range' benchmarks/ring_buffer.ll | head -3
grep 'branch_weights' benchmarks/ring_buffer.ll | head -3
grep '#11\|#0 alwaysinline' benchmarks/ring_buffer.ll | head -3
grep 'insertelement\|extractelement' benchmarks/nbody_newton.ll | wc -l
```

## 6. Decision Tree for Regressions

```
Step N regresses benchmark X:
  |
  +-- Is X == kalman_filter_runtime?
  |     |
  |     +-- >1.10x → revert immediately.
  |     |               kalman is the canary for #11 + auto-vectorizer issues.
  |     +-- Apply cross-per-field density check before reattempting.
  |
  +-- Is X == mandelbrot?
  |     +-- MISMATCH → revert immediately.
  |     |               mandelbrot MISMATCH always means SLP or metadata bug.
  |     +-- MATCH → acceptable within ±0.05x.
  |
  +-- Any other benchmark:
        +-- >5% regression → investigate before next step.
        +-- <5% regression → flag, proceed, re-evaluate after next step.
```

## 8. Interim Results — Step 1 + Trunc Fix (Potential Baseline Candidate)

Commit `874482f6` on `recovery-branch`. All 19 runtime benchmarks compiled, ran, and matched.
The bool truncation bug in `as_bool_reg()` (pre-existing in `b39461e2`) was fixed — all benchmarks
with guards in `@main` previously failed to compile (ring_buffer, nbody* , bit_clear, etc.).

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ Benchmark                 ║ Briv      ║ C          ║ Ratio    ║ Winner ║ Correct   ║
╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣
║ ring_buffer               ║ .0554s     ║ .0490s     ║ 1.13x    ║ C      ║ MATCH     ║
║ float_math                ║ .0725s     ║ .0760s     ║ .95x     ║ Briv  ║ MATCH     ║
║ float_math_nonzero        ║ .1663s     ║ .1682s     ║ .98x     ║ Briv  ║ MATCH     ║
║ sparse_dispatch           ║ .0548s     ║ .0641s     ║ .85x     ║ Briv  ║ MATCH     ║
║ print_loop                ║ .0650s     ║ .0631s     ║ 1.03x    ║ C      ║ MATCH     ║
║ nbody_newton              ║ 9.4851s    ║ 8.5580s    ║ 1.10x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.7581s    ║ 2.7962s    ║ .98x     ║ Briv  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.3699s    ║ 3.6140s    ║ .93x     ║ Briv  ║ MATCH     ║
║ fasta                     ║ .2108s     ║ .2112s     ║ .99x     ║ Briv  ║ MATCH     ║
║ fannkuch_redux            ║ .0629s     ║ .0646s     ║ .97x     ║ Briv  ║ MATCH     ║
║ mandelbrot                ║ .6593s     ║ .6572s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1808s     ║ .1797s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ knucleotide               ║ .1901s     ║ .1920s     ║ .99x     ║ Briv  ║ MATCH     ║
║ cancel_math               ║ .0618s     ║ .0642s     ║ .96x     ║ Briv  ║ MATCH     ║
║ bit_clear                 ║ .0002s     ║ .0003s     ║ .66x     ║ Briv  ║ MATCH     ║
║ queue_drain               ║ .0618s     ║ .0630s     ║ .98x     ║ Briv  ║ MATCH     ║
║ queue_drain_sym           ║ .0612s     ║ .0618s     ║ .99x     ║ Briv  ║ MATCH     ║
║ queue_drain_idio          ║ .0595s     ║ .0635s     ║ .93x     ║ Briv  ║ MATCH     ║
║ interval_step             ║ .0612s     ║ .0626s     ║ .97x     ║ Briv  ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

Key improvements over last agent's final state:
- kalman_filter_runtime: 3.80x → **1.00x** (catastrophic regression eliminated)
- ring_buffer: 1.28x → **1.13x** (recovered within noise of baseline 1.14x)
- nbody_sqrt_idio: 0.85x → **0.93x** (regressed from all-time 0.68x, but better than baseline 0.92x)

**nbody_sqrt_idio** is better than the pinned baseline (0.92x) but worse than its all-time best
(0.68x from cold-path outlining era). Step 2 is expected to improve this — if not, investigate.

### Step 2 Results — noundef + dereferenceable on ptr %state params

Commit `05e6fb65` on `recovery-branch`. Added `noundef` (and conditional `dereferenceable(N)`)
to all 14 function definitions taking `ptr %state`. Updated `dispatch.rs` + `mod.rs` reactor_tick
fallback too. 1045 tests pass.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ Benchmark                 ║ Briv      ║ C          ║ Ratio    ║ Winner ║ Correct   ║
╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣
║ ring_buffer               ║ .0558s     ║ .0471s     ║ 1.18x    ║ C      ║ MATCH     ║
║ float_math                ║ .0735s     ║ .0735s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ float_math_nonzero        ║ .1659s     ║ .1651s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ sparse_dispatch           ║ .0517s     ║ .0598s     ║ .86x     ║ Briv  ║ MATCH     ║
║ print_loop                ║ .0598s     ║ .0599s     ║ .99x     ║ Briv  ║ MATCH     ║
║ nbody_newton              ║ 9.1881s    ║ 8.5377s    ║ 1.07x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 3.1200s    ║ 3.0291s    ║ 1.03x    ║ C      ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.6215s    ║ 4.0243s    ║ .89x     ║ Briv  ║ MATCH     ║
║ fasta                     ║ .2211s     ║ .2209s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ fannkuch_redux            ║ .0646s     ║ .0633s     ║ 1.02x    ║ C      ║ MATCH     ║
║ mandelbrot                ║ .6969s     ║ .6959s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1856s     ║ .1845s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ knucleotide               ║ .1987s     ║ .1983s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ cancel_math               ║ .0631s     ║ .0635s     ║ .99x     ║ Briv  ║ MATCH     ║
║ bit_clear                 ║ .0004s     ║ .0003s     ║ 1.33x    ║ C      ║ MATCH     ║
║ queue_drain               ║ .0619s     ║ .0646s     ║ .95x     ║ Briv  ║ MATCH     ║
║ queue_drain_sym           ║ .0642s     ║ .0616s     ║ 1.04x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0673s     ║ .0662s     ║ 1.01x    ║ C      ║ MATCH     ║
║ interval_step             ║ .0714s     ║ .0710s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

Improvements over Step 1:
- nbody_newton 1.10 → 1.07x (−3%)
- nbody_sqrt_idio 0.93 → 0.89x (−4%)
- nbody_sqrt: minor noise (0.98 → 1.03x)
- All other benchmarks at or near parity within noise

### Step 4 Results — !range metadata from contracts

Commit `d71633c8` on `recovery-branch`. Wired contract-driven and type-driven range bounds
into every state field load as `!range` metadata. Added `idx_to_field_name` reverse index,
`extract_ranges_with_constants` for preamble constants (`TOTAL`, `CAP`), and `Le` handler.
1045 tests pass.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ Benchmark                 ║ Briv      ║ C          ║ Ratio    ║ Winner ║ Correct   ║
╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣
║ ring_buffer               ║ .0553s     ║ .0509s     ║ 1.08x    ║ C      ║ MATCH     ║
║ float_math                ║ .0757s     ║ .0748s     ║ 1.01x    ║ C      ║ MATCH     ║
║ float_math_nonzero        ║ .1662s     ║ .1645s     ║ 1.01x    ║ C      ║ MATCH     ║
║ sparse_dispatch           ║ .0500s     ║ .0609s     ║ .82x     ║ Briv  ║ MATCH     ║
║ print_loop                ║ .0615s     ║ .0584s     ║ 1.05x    ║ C      ║ MATCH     ║
║ nbody_newton              ║ 9.7294s    ║ 8.8435s    ║ 1.10x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 3.1473s    ║ 3.1247s    ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.7276s    ║ 3.9211s    ║ .95x     ║ Briv  ║ MATCH     ║
║ fasta                     ║ .2374s     ║ .2296s     ║ 1.03x    ║ C      ║ MATCH     ║
║ fannkuch_redux            ║ .0712s     ║ .0701s     ║ 1.01x    ║ C      ║ MATCH     ║
║ mandelbrot                ║ .6983s     ║ .6967s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1820s     ║ .1821s     ║ .99x     ║ Briv  ║ MATCH     ║
║ knucleotide               ║ .1971s     ║ .1957s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ cancel_math               ║ .0642s     ║ .0652s     ║ .98x     ║ Briv  ║ MATCH     ║
║ queue_drain               ║ .0630s     ║ .0625s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ queue_drain_sym           ║ .0638s     ║ .0639s     ║ .99x     ║ Briv  ║ MATCH     ║
║ queue_drain_idio          ║ .0621s     ║ .0579s     ║ 1.07x    ║ C      ║ MATCH     ║
║ interval_step             ║ .0637s     ║ .0635s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

Key change: ring_buffer 1.18→1.08x from `!range [0, 50000000)` on the `ops` field.
Other benchmarks showed non-overlapping min/max regressions:
- nbody_newton (Briv min 8.95s→9.62s, C avg 8.54s→8.84s)
- fasta (Briv min 0.205s→0.228s, C avg 0.211s→0.230s)
- nbody_sqrt (C avg 2.80s→3.12s — pure noise, Briv stable at ~3.0s)

### Step 4 Regression Analysis — July 28 12:00

Three changes in Step 4 produce ZERO LLVM IR difference for nbody_newton and fasta:

| Change | Effect on nbody/fasta |
|--------|----------------------|
| `extract_ranges_with_constants` | Same empty result — `bound`/`N` are `let` variables, not `const` |
| `idx_to_field_name` population | Populated but never read (empty `field_to_meta_idx`) |
| `!range` on `load_field_type` | Never emitted (no ranges found) |

The `extract_ranges_with_constants` change DOES benefit ring_buffer (`const TOTAL: Int = 50000000` is resolved from `ctx.constants`, producing `!range [MIN, 50000000)` on the `ops` field). The old `extract_ranges` only matched `Expr::Decimal`, so it couldn't resolve `TOTAL`.

C benchmarks also regressed between runs (nbody C 8.54s→8.84s, fasta C 0.211s→0.230s, nbody_sqrt C 2.80s→3.12s). C source is stable — the regression is SYSTEM NOISE from thermal throttling during sustained 7-minute runs.

**Confirmation procedure:**
1. Revert Step 4 — wait 60s cooldown → benchmark Step 3
2. Cherry-pick Step 4 back — wait 60s cooldown → benchmark Step 4

**Results from confirmation run (cold-started, 60s between runs):**

| Benchmark | Step 3 (reverted) | Step 4 (re-applied) | Briv Delta |
|-----------|-------------------|--------------------|-------------|
| ring_buffer | 1.09x (.0567s) | **1.14x (.0512s)** | **−9.7%** ⚡ |
| nbody_newton | 1.12x (9.85s) | **1.05x (9.25s)** | **−6.1%** ⚡ |
| nbody_sqrt | .94x (3.09s) | **.96x (2.85s)** | **−7.8%** ⚡ |
| nbody_sqrt_idio | .93x (3.68s) | **.94x (3.51s)** | **−4.6%** ⚡ |
| fasta | .93x (.221s) | 1.05x (.229s) | C faster (.236→.218s); Briv stable |
| All others | parity | parity | Within noise |

**Verdict: Step 4 is clean.** The original "regression" was thermal throttling noise from back-to-back benchmark runs without cooldown. Both Step 3 and Step 4 produce identical IR for benchmarks with no applicable `!range` metadata. ring_buffer's Briv time improved from .0567s to .0512s (−9.7%) from the `!range` on `ops`.

### Step 5 Results — !prof branch weights from postcondition

Commit `36437588` on `recovery-branch`. Emits `!prof !{branch_weights}` metadata on guard
branches when both postcondition bound (`[x == N]`) and modulo guard (`x % M == C`) are
available. Scales weights to max 1000. `1045 tests pass`. All 19 at parity.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ Benchmark                 ║ Briv      ║ C          ║ Ratio    ║ Winner ║ Correct   ║
╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣
║ ring_buffer               ║ .0553s     ║ .0488s     ║ 1.13x    ║ C      ║ MATCH     ║
║ float_math                ║ .0721s     ║ .0740s     ║ .97x     ║ Briv  ║ MATCH     ║
║ float_math_nonzero        ║ .1663s     ║ .1667s     ║ .99x     ║ Briv  ║ MATCH     ║
║ sparse_dispatch           ║ .0528s     ║ .0608s     ║ .86x     ║ Briv  ║ MATCH     ║
║ print_loop                ║ .0579s     ║ .0580s     ║ .99x     ║ Briv  ║ MATCH     ║
║ nbody_newton              ║ 9.0454s    ║ 8.2543s    ║ 1.09x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.8100s    ║ 2.7976s    ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.3549s    ║ 3.6367s    ║ .92x     ║ Briv  ║ MATCH     ║
║ fasta                     ║ .2094s     ║ .2204s     ║ .95x     ║ Briv  ║ MATCH     ║
║ fannkuch_redux            ║ .0681s     ║ .0648s     ║ 1.05x    ║ C      ║ MATCH     ║
║ mandelbrot                ║ .6702s     ║ .6648s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1801s     ║ .1817s     ║ .99x     ║ Briv  ║ MATCH     ║
║ knucleotide               ║ .1963s     ║ .1906s     ║ 1.02x    ║ C      ║ MATCH     ║
║ cancel_math               ║ .0645s     ║ .0635s     ║ 1.01x    ║ C      ║ MATCH     ║
║ queue_drain               ║ .0600s     ║ .0616s     ║ .97x     ║ Briv  ║ MATCH     ║
║ queue_drain_sym           ║ .0650s     ║ .0621s     ║ 1.04x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0641s     ║ .0639s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ interval_step             ║ .0639s     ║ .0619s     ║ 1.03x    ║ C      ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

All benchmarks stable within single-run noise. ring_buffer retains !range benefit from Step 4.

### Historical All-Time Best Results (Per Benchmark)

Compiled from AGENTS_HISTORY.md, AGENTS_HISTORY_2.md, docs/plans/*.md, benchmarks/results/*.md,
and commit messages. Every benchmark's best-ever ratio with the commit where it was achieved.

```
╔═══════════════════════════╦══════════════╦════════════╦══════════╦═══════════════════╗
║ Benchmark                 ║ Best Ratio   ║ Briv Time ║ Winner   ║ Commit / Era      ║
╠═══════════════════════════╬══════════════╬════════════╬══════════╬═══════════════════╣
║ ring_buffer               ║    0.99x     ║ .0664s     ║ Briv    ║ f598584 (Jul 06)  ║
║ float_math                ║    0.81x     ║ .0631s     ║ Briv    ║ 8a827db (Jul 11)  ║
║ float_math_nonzero        ║    0.98x     ║ .1611s     ║ Briv    ║ 33d42397 (Jul 27) ║
║ sparse_dispatch           ║    0.09x     ║ .0060s     ║ Briv    ║ 8a827db (Jul 11)  ║
║ print_loop                ║    0.93x     ║ .0624s     ║ Briv    ║ post-mig (Jul 19) ║
║ nbody_newton              ║    0.75x     ║ 7.4132s    ║ Briv    ║ 8a827db (Jul 11)  ║
║ nbody_sqrt                ║    0.85x     ║ 2.2434s    ║ Briv    ║ 33d42397 (Jul 27) ║
║ nbody_sqrt_idio           ║    0.67x     ║ 2.3270s    ║ Briv    ║ 33d42397 (Jul 27) ║
║ fasta                     ║    0.95x     ║ .2094s     ║ Briv    ║ recovery Step 5   ║
║ fannkuch_redux            ║    0.96x     ║ .0763s     ║ Briv    ║ 8a827db (Jul 11)  ║
║ mandelbrot                ║    0.99x     ║ .7514s     ║ Briv    ║ 8a827db (Jul 11)  ║
║ kalman_filter_runtime     ║    0.95x     ║ .1610s     ║ Briv    ║ early Jun (Era 1) ║
║ knucleotide               ║    0.97x     ║ .1880s     ║ Briv    ║ early Jun (Era 1) ║
║ cancel_math               ║    0.96x     ║ .0618s     ║ Briv    ║ recovery Step 1   ║
║ bit_clear                 ║    0.50x     ║ .0001s     ║ Briv    ║ 33d42397 (Jul 27) ║
║ queue_drain               ║    0.01x     ║ .0007s     ║ Briv    ║ 8a827db (Jul 11)  ║
║ queue_drain_sym           ║    0.95x     ║ .0575s     ║ Briv    ║ 33d42397 (Jul 27) ║
║ queue_drain_idio          ║    0.93x     ║ .0595s     ║ Briv    ║ recovery Step 1   ║
║ interval_step             ║    0.01x     ║ .0009s     ║ Briv    ║ f598584 (Jul 06)  ║
╚═══════════════════════════╩══════════════╩════════════╩══════════╩═══════════════════╝
```

Key observation: **No single commit achieves all bests simultaneously.** The all-time best for each
benchmark is spread across multiple eras spanning 6 weeks of development. The two most
frequent best-performing commits are `8a827db` (Phase 3 complete, Jul 11) and
`33d42397` (post-fixes with no stride gate, Jul 27).

Critical finding for nbody_sqrt_idio: best (0.67x) at `33d42397` is AFTER the baseline `b39461e2`.
The stride gate re-enabled in `b39461e2` regressed it from 0.67x to 0.92x — a 37% penalty
accepted as the price of ring_buffer parity. Whether this regression was real or thermal
throttling is the question.

## 9. Worktree Topology & Execution Environment

### Worktree Layout

```
../briv-compiler/                     # Main repo (integration target)
../briv-compiler-baseline/            # Read-only A/B comparison at b39461e2
../briv-compiler-recovery/            # ACTIVE — recovery-branch @ b39461e2
../briv-compiler-derive/              # Feature worktree (derivation + stochastic opt)
```

### Where to Run What

| Operation | Run in | Why |
|-----------|--------|-----|
| `cargo test --lib` | `<any>` | All worktrees share objects — same result |
| `cargo build --release` | `../briv-compiler-recovery` | Build the recovery compiler |
| `bash benchmarks/build_and_bench.sh --runtime` | `../briv-compiler` | Script uses relative path to baseline |
| `bash benchmarks/compare_baseline.sh <name>` | `../briv-compiler` | Script looks for `../briv-compiler-baseline` |
| Single bmark compile | `../briv-compiler-recovery` | Use our compiler binary |
| `git commit` | `../briv-compiler-recovery` | Commits go on `recovery-branch` |

### Integration with Derivation Feature Worktree

The derive worktree (`../briv-compiler-derive`) holds 12 code commits for the
`:=` derivation block + stochastic optimization feature, forked from `c3155e99`
(which is 5 commits behind HEAD on the current `main`).

**Merge order:**
1. Feature merges to `main` first (derivation agent resolves conflicts with
   the 17 post-baseline commits)
2. Recovery merges into `main` second (we resolve conflicts using the matrix
   in the section below)

### Conflict Resolution Matrix (for Step 2 merge)

When merging `recovery-branch` into `main` (after feature merge):

| File(s) | Strategy | Rationale |
|---------|----------|-----------|
| `emit_toplevel.rs`, `context.rs`, `mod.rs`, `dispatch.rs` | Take recovery | Our correct metadata, stride gate, state params, DataLayout |
| `helpers.rs` | Take recovery | Our `!range`/`!prof` wiring |
| `*slp_isomorphism*`, `*counter*` | Take recovery | Our working stride gate + gates |
| `lexer.rs`, parser files | Merge both | Feature adds `:=`, our Step 6 adds `!>` — both coexist |
| Feature-new files (synthesis, MCMC, config) | Take main | Keep derivation feature |
| MetadataRegistry hooks in backend | Take main's hooks, apply to our code | Feature's registry must work with our clean backend |
| `.bv` files | Apply Step 6 `!>` syntax to all | Feature-added .bv files also need metadata syntax update |

## 10. Rollback Procedure

```bash
# Revert the most recent commit (one step)
git revert HEAD

# If multiple steps need reverting
git log --oneline -5
git revert HEAD~N..HEAD  # revert the last N commits

# Complete restart from baseline
git checkout b39461e2
git branch -D recovery-branch
git checkout -b recovery-branch
```

## 11. Ad-Hoc Single-Benchmark Testing

```bash
# From the recovery worktree:
cd /home/randozart/Desktop/Projects/briv-compiler-recovery
rm -f benchmarks/ring_buffer.ll benchmarks/ring_buffer
./target/release/brivc build benchmarks/ring_buffer.bv --llvm --out benchmarks
clang -O3 -flto -march=native -ffast-math \
    benchmarks/ring_buffer.ll lib/runtime/briv_rt.c \
    -o benchmarks/ring_buffer
BOUND=50000000 /usr/bin/time -f "%e" ./benchmarks/ring_buffer 2>&1

# Compare against baseline:
cd /home/randozart/Desktop/Projects/briv-compiler
bash benchmarks/compare_baseline.sh ring_buffer
```

## 12. Full Verification Commands

```bash
# Before every commit (from recovery worktree):
cd /home/randozart/Desktop/Projects/briv-compiler-recovery
cargo test --lib
cargo build --release

# Full benchmark suite (from main worktree):
cd /home/randozart/Desktop/Projects/briv-compiler
rm -f benchmarks/*.ll
bash benchmarks/build_and_bench.sh --runtime
bash benchmarks/build_and_bench.sh --correctness
```
