# Phase 3 — Config Migration + Rule 18 Cleanup results

**Date:** 2026-07-31
**Worktree:** FDD worktree at `../briv-compiler-fdd`, branch `feat/frontend-driven-dispatch`
**Baseline:** Phase 2 results in `2026-07-31-frontend-dispatch-phase2.md` (commit `d5ac5509`)
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`
**Raw output:** `/tmp/opencode/p3_runtime.log`

## What changed

Phase 3 (plan §8) moves the remaining hardcoded codegen knowledge out of the
compiler and cleans up Rule 18 violations / dead code.

### §8.1 `config/targets.toml` — per-target tuning (`1742f6f4`)

New `[target.<triple-prefix>]` tables (`float_registers`, `dense_compute_density`,
`vector_min_width`), loaded by the new `src/config_tuning.rs` (baked via
`include_str!` + `LazyLock`) and matched against the compiler's target_triple by
longest-prefix. Unknown prefixes fall back to the x86_64 defaults and
`generate()` warns once — silent x86 assumptions never apply to foreign targets.
`float_register_count()` now reads config instead of a hardcoded triple-prefix
match (wasm keeps its "effectively unlimited" u32::MAX cap). `TargetConfig`
made `backend`/`defaults` optional so the tuning tables coexist in the same file.

### §8.2 `config/ir-lowering.toml` — global tuning (`1742f6f4`)

`arena_min_budget`, `arena_initial_size`, `stack_threshold`,
`max_fields_per_alloca`, `sso_max_bytes`, `svo_max_elements`,
`callable_inline_weight_threshold` (SSO 6-byte default derived from the
align-8 − 2-tag-bit String handle; documented at the key). All consumers rewired;
`MAX_FIELDS_PER_ALLLOCA` const removed; `vector_min_width` threaded through
`analyze_program` → `build_loop_shapes` → `slp_isomorphism::analyze_body`.

### §8.3 Derived (not config) (`2f70365c`)

- Write masks are `u128` and the emitted width is `i128` when the program has
  >64 state fields (`write_mask_type`) — the old `idx < 64` gate silently
  dropped the 65th+ field.
- `!prof` branch-weight cap is a power of two near `i32::MAX / 2` (2^30):
  LLVM `branch_weights` are i32 and scaling keeps the sum ≤ cap with no
  overflow and less rounding (iter bounds ≤ 50M now emit direct weights).
- `type_driven_range` derives `[0, 2^(8*bytes))` from `resolved.bytes`.

### §8.4 Rule 18 cleanup — casting graph / universe (`ebbeaa03`, `25b6fd01`, `35a1790c`)

All hardcoded Briv type-name matches in `src/backend/llvm/` replaced:

| D# | Site | Now |
|----|------|-----|
| D1 | box_op fallbacks ×2 (emit_toplevel) | `is_boxed_type` / `is_boxed_int_type` / `emit_box_value_to_i64` on protocol membership (#Bool/#Char/#String/#Data/#Float-32) + canonical `Type::int()` |
| D2 | TypeConverter box/unbox (builder, test-only) | canonical bootstrap Type constructors |
| D3 | `primitive_from_name` / `resolve_bild_type` | deleted — dead code |
| D4 | `try_eval_cfloat` | caller-supplied `is_protocol_member(ty, "#Float")` |
| D5 | trigger supported-set gate | `is_boxed_int_type` + #Int/#UInt membership |
| D6 | TBAA tiebreak ×3 | shared `sort_tbaa_groups`: #Int protocol member (Cast.#Int) first |
| D7 | `llvm_type`/`emit_trg_load_finish`/`protocol_llvm_type` String/Data/Float checks | #String/#Data/#Float membership; legacy string-shaped types via structural `is_string_like` (also fixes i128 mis-derivation for 2-int-field structs); float let-param types from #Float + byte width |
| D8 | SSO String shim (emit_expr) | #String/#Data membership |
| D9 | abi.rs `is_bool_type` | `Cast.#Bool` protocol property (universe threaded) |
| D10 | helpers.rs native-float detection ×2 | `is_protocol_member(ty, "#Float")` |

Plus: cast-to-`Int` sites → canonical `Type::int()`; projection fast paths →
canonical `Type::int()/float()/bool_()`. Also seeded `Cast.#Char` and
`Cast.#String` in the type-universe primordials (the casting graph already had
Fixed LLVM types for both but the categories were unreachable, so Char/String
resolved to "Bit"/i64 in a bare universe).

`git grep 'Type::Custom.*==' src/backend/llvm/` now returns **zero**.

### §8.5 Dead code / latent bugs (`f2c0daaa`)

- E1: deleted `pre_extract_float_fields` / `pre_extract_int_fields` /
  `pre_load_all_fields` (zero callers).
- E2: SVO packed header — `(len << 32) | (cap << 32)` overlapped both fields;
  new `pack_svo_header` places cap in bits 1..32 and len in bits 32..64, with a
  round-trip test. `emit_svo_list` slot cap now reads config `svo_max_elements`.
- E3: deleted dead `emit_post_print`.
- E4: removed the always-false ringbuf-init detection stub and the unreachable
  `emit_ringbuf_init`; RingBuffer state fields use the `ringbuf_inline` expansion.
- E5: removed the unconditional `br i1 true` + unreachable `rollback:` block in
  the assume_shape path; deleted `emit_shape_guarded_body` (zero callers).
- E6: normalizer size/width/alignment fallbacks are no longer silent —
  `register_typedefs` threads `int_bits` (was a hardcoded 64) and records a
  diagnostic on the new `TypeUniverse.warnings` whenever a fallback fires; the
  LLVM backend surfaces them in its warning report.

## Runtime ratios (Briv vs C, ratio < 1 = Briv faster)

| Benchmark | Phase 3 Briv | Phase 3 ratio | Phase 2 ratio | Δ | Winner | Correct |
|-----------|--------------:|:-------------:|:-------------:|:---:|:------:|:-------:|
| ring_buffer | 0.0524s | 1.13× | 1.18× | −0.05 | C | MATCH |
| float_math | 0.0734s | 0.99× | 0.97× | +0.02 | Briv | MATCH |
| float_math_nonzero | 0.2003s | 1.21× | 1.20× | +0.01 | C | MATCH |
| sparse_dispatch | 0.0515s | 0.84× | 0.86× | −0.02 | Briv | MATCH |
| print_loop | 0.0607s | 1.03× | 1.06× | −0.03 | C | MATCH |
| nbody_newton | 6.9053s | 0.82× | 0.83× | −0.01 | Briv | MATCH |
| nbody_sqrt | 2.1862s | 0.78× | 0.78× | 0.00 | Briv | MATCH |
| nbody_sqrt_idio | 2.7251s | 0.75× | 0.76× | −0.01 | Briv | MATCH |
| fasta | 0.2088s | 0.99× | 1.01× | −0.02 | Briv | MATCH |
| fannkuch_redux | 0.0607s | 0.93× | 0.97× | −0.04 | Briv | MATCH |
| mandelbrot | 0.6778s | 1.02× | 1.03× | −0.01 | C | MATCH |
| kalman_filter_runtime | 0.2197s | 1.21× | 1.23× | −0.02 | C | MATCH |
| knucleotide | 0.1873s | 0.99× | 0.98× | +0.01 | Briv | MATCH |
| cancel_math | 0.0535s | 0.84× | 0.80× | +0.04 | Briv | MATCH |
| bit_clear | 0.0003s | 3.00× | 0× | (noise) | C | MATCH |
| queue_drain | 0.0570s | 0.93× | 0.85× | +0.08 | Briv | MATCH |
| queue_drain_sym | 0.0565s | 0.92× | 0.92× | 0.00 | Briv | MATCH |
| queue_drain_idio | 0.0564s | 0.91× | 0.93× | −0.02 | Briv | MATCH |
| interval_step | 0.0629s | 0.99× | 1.01× | −0.02 | Briv | MATCH |

**Zero MISMATCH.** All deltas are within the harness's run-to-run noise band
(±0.05–0.08× on the ~0.05s benchmarks; bit_clear times a ~0.3ms benchmark). The
per-txn memory attribute and main dispatch marker are byte-identical to Phase 2
for the sensitive set, and the emitted CODE is byte-identical (verified by
diffing generated `.ll` against the pre-§8.5 and pre-§8.4 reference builds).

## Tests

`cargo test --lib`: 1267 passed, 0 failed (was 1265 at Phase 2; +2 SVO header
round-trip tests). `cargo build`: no new warnings. Praetor clean on all changed
files.
