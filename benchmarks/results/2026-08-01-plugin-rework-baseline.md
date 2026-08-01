# Baseline + FFI Audit — plugin/macro rework Phase 0

**Date:** 2026-08-01
**Commit:** `f546af1c` (== `d6c6c818` for benchmarks; the plan doc commit adds no code)
**Worktree:** `../brief-compiler-plugin-rework`, branch `feat/plugin-macro-rework`
**Plan:** `docs/plans/2026-08-01-plugin-macro-rework.md`
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Raw output:** `/tmp/plugin_rework_baseline.log`
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`
**Baseline reference worktree:** `../brief-compiler-baseline` at `d6c6c818`

## 1. Baseline results (rule #11 — clean `cargo build --release` + harness)

5 iterations per benchmark, nanosecond-precision fork+exec timing.

| Benchmark | Brief | C | Ratio | Winner | Correct |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|
| ring_buffer | .0516s | .0442s | 1.16× | C | MATCH |
| float_math | .0436s | .0720s | .60× | Brief | MATCH |
| float_math_nonzero | .1590s | .1662s | .95× | Brief | MATCH |
| sparse_dispatch | .0502s | .0615s | .81× | Brief | MATCH |
| print_loop | .0362s | .0609s | .59× | Brief | MATCH |
| nbody_newton | 7.6315s | 9.1267s | .83× | Brief | MATCH |
| nbody_sqrt | 2.4004s | 3.1238s | .76× | Brief | MATCH |
| nbody_sqrt_idio | 2.9454s | 3.9006s | .75× | Brief | MATCH |
| fasta | .2422s | .2267s | 1.06× | C | MATCH |
| fannkuch_redux | .0604s | .0641s | .94× | Brief | MATCH |
| mandelbrot | .7117s | .6932s | 1.02× | C | MATCH |
| kalman_filter_runtime | .1522s | .1780s | .85× | Brief | MATCH |
| knucleotide | .1894s | .1911s | .99× | Brief | MATCH |
| cancel_math | .0503s | .0590s | .85× | Brief | MATCH |
| bit_clear | .0005s | .0004s | 1.25× | C | MATCH |
| interval_step | .0597s | .0596s | 1.00× | ~tie | MATCH |
| telemetry_stream | .1919s | .1987s | .96× | Brief | MATCH |
| pid_control | .3432s | .3486s | .98× | Brief | MATCH |
| matrix_pipeline | .4630s | .7407s | .62× | Brief | MATCH |
| accumulator_flush | .1072s | .1489s | .71× | Brief | MATCH |
| sweep_sparse | .2190s | .1530s | 1.43× | C | MATCH |
| sweep_mid | .2602s | .2343s | 1.11× | C | MATCH |
| sweep_dense | .3957s | .2627s | 1.50× | C | MATCH |
| bridge_glue | done | — | — | — | SKIP |
| bridge_multi | done | — | — | — | PASS |

**Zero MISMATCH.** These match the countdown-era levels (`2026-07-31-countdown-loop.md`):
print_loop .59×, kalman .85×, float_math_nonzero .95× — the FFI-facing benchmarks
are at or above parity. The only >1.2× losses (sweep_* 1.11–1.50×, ring_buffer
1.16×, fasta 1.06×) are loop-shape/density/pointer-boxing concerns, **not** the
print/env FFI paths this plan touches; they belong to the frontend-driven-dispatch
workstream.

## 2. FFI audit findings (rule #19 — evidence from the ACTUAL generated IR)

### 2.1 Print FFI — direct, inlinable, no indirection. No regression found.

`float_math.ll` hot-loop tail:

```llvm
%t182 = call i64 @__print_float(float %t183)      ; one value per PrintLn!
%t190 = call i64 @__print_char(i64 %t191)         ; newline
```

`print_loop.ll`:

```llvm
%t42 = call i64 @__print_int(i64 %__cp_ops)
%t44 = call i64 @__print_char(i64 %t45)
```

The runtime helpers (`brief_rt.c:178-199`) are `always_inline` and the harness
links `.ll` + `brief_rt.c` with `-O3 -flto`, so these inline into `main`'s hot
loop. This matches the "native era" emission (intrinsics.rs emits the same
`call i64 @__print_*` shapes since 0ebfba39). print_loop improved 1.03×→.59×
with the countdown loop (9d7a2404) — already on main.

### 2.2 Env FFI — direct i64 calls at module init. Correct ABI.

`float_math.ll` (module init, not hot loop):

```llvm
%t0 = call i64 @__getenv_int(ptr %t2)       ; brief_rt.c: `__getenv_int(int64_t key_bstr)`
%t0 = call ptr @__getenv_brief(ptr %t2)
```

`ptr` is the 8-byte ABI equivalent of the C `int64_t` param. brief_rt.c
`__getenv_int`/`__getenv_brief` (lines 137-160) decode the length-prefixed Brief
string → C string → `getenv`. Called once at init for `let bound = get_env_int("BOUND")`.

### 2.3 LATENT INCONSISTENCY (documented, NOT fixed — out of phase scope)

The frgn **declare** emission uses `protocol_llvm_type` (`mod.rs:366`), which
returns `{ i64, i64 }` for String-shaped types **unconditionally**:

```llvm
declare { i64, i64 } @__getenv_brief({ i64, i64 }) #6
declare i64 @__getenv_int({ i64, i64 }) #6
```

The **call-site** uses `llvm_type` (`emit_toplevel.rs:269`), which **respects
`feature_sso_strings`** (default **off** → `ptr`). With SSO off the declare is
wrong-but-dead: LLVM creates a separate implicit `ptr`-typed function for the
call, which resolves to the C symbol; the wrong `{i64,i64}` declare is
GC-section-dropped. With SSO on, both resolve `{i64,i64}` and the
`extractvalue {i64,i64} ..., 0` + `inttoptr` shim applies (emit_expr.rs:1851-1883).

**Impact today:** none (calls match the C ABI; benchmarks are correct). **Risk:**
if anything ever calls through the declared `{i64,i64}` prototype, the ABI breaks.
**Fix (deferred to a follow-up):** make `protocol_llvm_type` honor
`feature_sso_strings` so the declare agrees with the call — a one-line feature
gate, additive, no semantic change. Logged in BUGS.md.

### 2.4 No hot-loop `.c`-source frgn benchmark in the active suite

`git grep frgn benchmarks/*.bv`: `meld-bridge.bv` and `gpu/saxpy/saxpy.bv` use
`.c`/`#Link` frgns but are **excluded** from `BENCHMARKS` in the harness
(`meld-bridge` — no `.bv` build path; `gpu/*` — no `.bv`). fasta mentions frgn
only in a comment. The FFI hot-loop surface of the active runtime suite IS the
`__print_*` intrinsic path. Tier-1 `frgn` Inline resolution for `.c/.cpp/.cxx/.rs`
is confirmed ahead of the GLUE-bridge check (`frgn_dispatch.rs:177`), so `.c`
frgns never route through the bridge on the LLVM backend.

### 2.5 Bridge path never selected for native sources (confirmed)

Dispatch order in `resolve_single_frgn` (`frgn_dispatch.rs`): `#Web` →
`#System`/`#Link` → empty-ext error → **`.c/.cpp/.cxx/.rs` → Inline** →
GLUE-language → Bridge → `.o/.so/.a` → Inline → Unsupported. Native sources can
never reach the Bridge path on LLVM. (Bridge is exercised only by
`benchmarks/bridge/` Python-based harnesses — SKIP/PASS, not timed.)

### 2.6 ptr↔i64 round-trips are folded by LLVM (no perf impact)

`get_env` wrapper defn shows `ptrtoint`→`inttoptr` pairs (the `("ptr","i64")`
coerce arm, emit_expr.rs:1791-1796). These are identity folds for LLVM;
irrelevant to the measured numbers.

### 2.7 Dead duplicate intrinsic arms (latent cleanup)

`intrinsics.rs:65-90` and `:94-97` both match `"PrintInt#"`/`"PrintFloat#"`/
`"PrintChar#"`/`"PrintStr#"`; the second set (via `emit_external_call`) is
unreachable. Harmless today; the format rework (Phase 1) will remove the dead
arm.

## 3. Regression target + guard

The guard test `backend::llvm::tests::test_print_plugin_emits_direct_ffi_calls`
(tests.rs, appended 2026-08-01) pins the contract for the entire rework:

- `PrintLn!` rewrites to `PrintInt#` + `PrintChar#`.
- The backend emits **direct** `call i64 @__print_int(...)` and
  `call i64 @__print_char(...)`.
- No `bridge_` indirection appears in the IR.

Phase 1 renames `PrintLn!`→`println!`; if the new format rewrite ever stops
emitting the direct call sequence, this test fails. `cargo test --lib --`
currently green (1311 tests).

## 4. Harness fix (pre-existing bug)

`benchmarks/compare_baseline.sh:34` referenced the stale binary name
`brief-compiler` (renamed to `briefc` long ago) — baseline builds would fail.
Fixed to `briefc`. Also `build_and_bench.sh` `bridge_multi` node bench failed
under `set -e` (missing koffi), aborting the suite before the summary; now
guarded with `|| echo SKIP`. Both are robustness fixes, not number-gaming.

## 5. Conclusion

**No native-FFI performance regression is present in the print/env paths.** The
emitted IR for print/env FFI is direct `call @__print_*`/`call @__getenv_*` with
no bridge indirection; the two latent issues (§2.3 declare mismatch, §2.7 dead
arms) are correctness-adjacent cleanups for follow-up, not perf regressions. The
plugin/macro rework (Phases 1-5) can proceed without an FFI perf blocker.
