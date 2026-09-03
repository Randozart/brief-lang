<!-- 2026-06-09 -->

# Benchmark Strategy

## Philosophy

Benchmarks exist to find flaws in Briev, not to produce flattering numbers.

A benchmark that reports `0.0002s vs 0.088s` (440x Briev win) is a diagnostic
signal — it tells you the compiler folded the loop. The compiler is correct.
The benchmark is measuring the wrong thing.

The same applies in reverse: a benchmark that only reaches parity when the Briev
program uses a strategy keyword (`seq`, `vol`, `pack`, …) or a non-idiomatic
shape is surfacing a **default-codegen gap**, not a win. The compiler must pick
the most efficient strategy for every program automatically (Golden Rule 2
"MAXIMUM EFFICIENT DEFAULT", AGENTS.md); keywords exist for intended
behaviour, never to win on speed. A modifier-beaten default is a compiler bug
— fix the default, don't credit the modifier.

### Symmetric by Default

Every Briev benchmark must compute the **same output** as its C reference for
the same input. If Briev's idiomatic approach to a problem differs
fundamentally from C's (different data structures, different control flow,
different algorithm), create **two** benchmarks:

| Variant | Intent |
|---------|--------|
| **Symmetric** (`_sym`) | Mirrors C's approach step-for-step using Briev features. Answers: "Given the same algorithm, does Briev's throughput match C's?" |
| **Idiomatic** (`_idio`) | Uses Briev-native patterns (contract-proven loops, reactive transactions, etc.) to achieve the same semantic result. Answers: "Given the same semantic goal, can Briev's optimizer find a better path?" |

Both must produce identical output for the same input. Neither claims to be
the single "fair" comparison — both are tagged with their intent.

When a bug is discovered in a benchmark (e.g. wrong output), fix the bug.
Do not create two variants unless the approaches genuinely differ. Hillel
Wayne's observation about `queue_drain` is the canonical example: the C
and Briev versions were computing the same result through different
algorithms, making the comparison asymmetric. The fix is not to hobble
either version, but to create a symmetric pair and label them.

## The Two Categories

Every benchmark belongs to exactly one category:

| Category | Tag | What it measures | Criteria |
|----------|-----|------------------|----------|
| **Runtime** | `--runtime` | Throughput of compiled code | FFI call or observable intrinsic in the hot loop body. LLVM cannot eliminate the loop. |
| **Optimizer** | `--optimizer` | Compile-time folding power | All `const` inputs + no FFI in the hot loop. LLVM may eliminate the loop. Both Briev and C must produce identical outputs. |

A benchmark cannot be both. If it has no observable side effects in its hot
loop, it is an optimizer benchmark — runtime timing is meaningless.

### Runtime Benchmarks (14)

| Benchmark | What it tests | Loop structure |
|-----------|--------------|----------------|
| `fasta` | FFI `putchar` in hot loop, convergent loop | `__putchar(seed % 26 + 97)` each iteration |
| `fannkuch_redux` | 12-field rotation, SROA scalar decomposition | Pure integer math, FFI only at exit |
| `mandelbrot` | Complex arithmetic + escape tracking | FFI `__print_int` every 5M iters |
| `knucleotide` | 64-field guarded dispatch, compiler switch-gen | FFI `__print_int` every 5M iters |
| `float_math` | Float arrays at contract-proven scale | FFI `__print_float` every 5M iters |
| `float_math_nonzero` | Float with nonzero preconditions | FFI `__print_float` every 5M iters |
| `kalman_filter_runtime` | Kalman filter with runtime bound | FFI `__print_float` every 5M iters |
| `nbody_newton` | Newton-iteration sqrt pipeline | FFI `__print_float` at exit only (runtime bound prevents fold) |
| `nbody_sqrt` | Hardware sqrt via FFI | FFI `__sqrtf` in hot loop |
| `sparse_dispatch` | Sparse conditional dispatch | FFI `__print_int` every 5M iters |
| `cancel_math` | Expression cancellation patterns | FFI `__print_int` every 5M iters |
| `bit_clear` | Bitwise ops + popcount | FFI `__print_int` conditionally (prevents fold) |
| `queue_drain` | Collection push/pop + dispatch | FFI `__print_int` every 5M iters |
| `interval_step` | Interval arithmetic stepping | FFI `__print_int` every 5M iters |
| `ring_buffer` | Folded while-loop with periodic output | FFI `__print_int` every 5M iters |

Sources: `async_counters`, `async_counters_runtime`, `iir_filter_runtime`,
`precompute_sum_runtime`, `ring_buffer_runtime` also runtime-bound
(use `__get_env_int` for loop bound).

### Optimizer Benchmarks (3)

| Benchmark | What it tests | Why foldable |
|-----------|--------------|--------------|
| `iir_filter` | Folded while-loop counter convergence | All `const` inputs + pure float math |
| `precompute_sum` | Compile-time complete evaluation | All `const` inputs, no FFI |
| `const_heavy` | Many constant operands folded | All `const` inputs, no FFI |

These emit correct LLVM IR and produce correct results. The timing is
irrelevant — they are `precompute_ok`.

## How the Harness Decides

After compilation, the harness (`build_and_bench.sh`) inspects the emitted
IR or binary for evidence of an observable loop:

1. **Binary size check**: If Briev `.text` < 25% of C `.text`, flag as
   `precompute_ok` (compiler folded the loop).
2. **IR check**: If the emitted `main()` contains `ret i32 0` and no loop
   back-edge, flag as `precompute_ok`.
3. **Runtime timing**: Only performed for non-precompute_ok benchmarks.

```
=== iir_filter ===
  briev:     1 KB  (precompute_ok — skip runtime)
  c:        36 KB
  briev out: <correctness: match>
  c out:     <correctness: match>
```

## Correctness Verification

Every benchmark, regardless of category, must produce the same output as
its C reference when given the same input. The harness runs both with
`BOUND=5` and compares stdout.

## New Benchmarks

New benchmarks should follow the `fasta` pattern: an FFI call in the hot loop
body that produces observable output. The canonical set from AGENTS.md:

| Benchmark | Pattern | Status |
|-----------|---------|--------|
| `fasta` | FFI output in hot loop | ✅ Done |
| `fannkuch-redux` | 12-field rotation SROA | ✅ Done |
| `mandelbrot` | Complex arithmetic | ✅ Done |
| `knucleotide` | 64-field guarded dispatch | ✅ Done |
| `spectral-norm` | Float arrays, contract-proven scale | 📝 Planned |
| `binary-trees` | Index-based tree walk, struct pool | 📝 Planned |

## CLI Flags

```
bash benchmarks/build_and_bench.sh               # all (current behavior)
bash benchmarks/build_and_bench.sh --runtime     # runtime only
bash benchmarks/build_and_bench.sh --optimizer   # optimizer only
bash benchmarks/build_and_bench.sh --correctness # output verification only
```

## Anti-Overfit Doctrine (locked, 2026-09-02 — user)

**Ward against over-tuning; keep the language powerful enough that a
programmer reaches the same performance with general syntax.**

The GPU (and CPU) optimization arcs anchor on real workloads — currently
LLM-shaped GEMV/GEMM against the llama.cpp/ggml anchors. That is a
BENCHMARK-TARGET choice, never a backend-design constraint. The doctrine
that keeps the machinery general while the race stays honest:

1. **Shape tiers, not workload tiers.** Strategy tiers (`GemmPlan`,
   cooperative rows, coopmat fragments) may recognize loop STRUCTURE —
   decomposition, reduction, indexed store — never workload names, type
   names, or benchmark text (rule 1/15). A tier is selected by shape +
   device capability, never by what the program is "for".
2. **The general path is always the correctness reference.** Every tiered
   fast path must be validated against the general lowering's output
   (the f16 arc's pattern: the naive tier proved the tensor tier's
   expected numerics before the tensor tier was trusted).
3. **A general-syntax programmer gets the tier's performance.** Rule 2's
   GPU corollary: if reaching the fast tier requires a strategy keyword,
   a rewrite, or a luckier loop shape, that is a compiler bug — fix the
   shape analysis to recognize the general form, never demand the
   special one.
4. **VERDICT discipline.** A rung that loses is a VERDICT entry, not a
   silent revert (O3: ~5% on GEMV — rejected, kept as infrastructure
   only where generally useful).
5. **Watch the under-served side.** Over-tuning shows up as small /
   irregular shapes degrading while the benchmark shape wins. Shape-robust
   guards (the 256³ dispatch quirk) are generality work, not cleanup.

## GPU Benchmark Portfolio (2026-09-02)

The GPU lane's benchmark set, chosen so no single access pattern (or
workload family) dominates the tuning signal — the anti-overfit doctrine
applied to the benchmark suite itself:

| benchmark | pattern | what it measures | status |
|-----------|---------|------------------|--------|
| `gemv.abv` + `gemv_bench` | row reduction, co-op rows | memory + subgroup ops | ✅ ledger (M1, 0.199ms) |
| `gemm.abv` + `gemm_bench` | tiled matmul f32 | shared-mem staging, compute | ✅ ledger (M2.1) |
| `gemm_h.abv` + `gemm_h_bench` | tensor matmul f16 | coopmat fragments, fp32 accumulate | ✅ ledger (14.3 TFLOP/s, past anchor) |
| `saxpy.abv` + `saxpy_bench` | pure elementwise | achieved DRAM bandwidth | ✅ RESOLVED (n_fields=3 harness typo; 219.7 GB/s full-transport) |
| `reduce.abv` + `reduce_bench` | two-stage reduction | bandwidth + combination | ✅ 0.771ms / 69 GB/s — latency-bound; subgroup-coop target |
| `gather_8.abv` + `gather_bench` | strided gather (32B stride) | random-read throughput | ✅ 1.881ms / 69.5 GB/s — prefetcher penalty vs saxpy's stream |
| `softmax.abv` + `softmax_bench` | elementwise Exp# | transcendenal compute+memory | ✅ 0.455ms / 278.9 GB/s (Exp# via GLSL.std.450) |
| `nbody_force.bv` | N-body central force | non-AI compute generality | ✅ pre-existing |
| `pairs.abv` | 2D dispatch infra | grid geometry | ✅ infra |
| stencil (neighbor + boundary) | local windows | needs guarded bodies or .bv+accel lane | 📝 deferred — SPIR-V rejects when/[]; gather fills the non-contiguous slot |
| `ray.abv` + `ray_bench` | per-pixel raytracer | divergence + transcendental shading | ✅ 1.01e-04 gate, 0.246ms, 8437 Mrays/s = 436× CPU (plan 2026-09-02-graphics-ray-and-images) |

Doctrine notes:
- Every entry gates on correctness BEFORE timing; the C harness is
  seed → launch → download → double-reference compare.
- The generated runner's field table is the layout authority for every
  harness (the 256³ lesson: harness offsets derived from args, blob
  arrays literal-sized — mismatch reads as a "correctness failure").
- Benchmarks exist to find flaws: reduce exposed the serial-FADD
  latency bound (O6's target); saxpy exposed the 4th-field store
  visibility issue (BUGS.md, open).
