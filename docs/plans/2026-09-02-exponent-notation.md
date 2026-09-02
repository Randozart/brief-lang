# Plan: exponent float literals — `1.0e-8`, `1e5` (C-style maximal munch)

**2026-09-02.** Small syntax completion arc. Companion to
`2026-09-02-fundamental-parent-membership.md` (landed same day).

## The immediate use case (why now)

1. **The f16 tensor arc (`.abv` work, M2.2) needs these literals.** The
   Float16 precision contract admits a literal only when it round-trips
   through 16 bits exactly — and the interesting boundary values are
   exponent-shaped: the f16 subnormal underflow wall (`6.0e-8`), the
   smallest exact f16 value (2⁻²⁴ = `5.9604644775390625e-8`), epsilon-scale
   tolerances (`1.0e-4`). In plain decimal these are unwritable or
   unreadable (`0.000000059604644775390625`). The precision-gate tests had
   to dodge the gap with `0.0001`; the subnormal boundary remains
   unexercised until this lands.
2. **C benchmark parity.** Numeric C code (the benchmark references the
   GPU arc races against) uses exponent literals throughout; every port
   hand-rewrites them.
3. **The highlighter already promises it.**
   `syntax-highlighter/syntaxes/briev.tmLanguage.json` float pattern:
   `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?` — the editor surface has treated
   `1.0e-8` as valid; the lexer never delivered. Docs/impl mismatch.

## Design (user-approved: C-style bare form included)

- Dot form: `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?` — `1.0e-8`, `1.0E+5`.
- Bare form: `[0-9]+[eE][+-]?[0-9]+` — `1e5`, `1e-8` (C-style).
- **Maximal munch is the rule** (as in C): `1e+5` is a float literal,
  never `1e + 5`. No repo source reads it the old way (swept:
  zero `.bv` or embedded-test hits for digit-`e`-digit adjacency).
- Fallbacks stay exact: `1e` → Decimal + Ident (regex needs digits after
  `e`); `1e + 5` (spaced) → Decimal + Ident + ops; `1.0-8` → subtraction;
  `0x1e5` → hex (bare-float regex cannot cross the `x`).
- Zero parser changes: `lex.slice().parse::<f64>()` handles exponents
  natively; `Expr::Float(f64)` carries the value, so literal admission
  (`float_literal_fits`), backend hex emission, and the interpreter
  consume it unchanged.

## Changes

1. `src/lexer.rs` — the two regexes (logos longest-match picks the bare
   form over Decimal).
2. Lexer tests — single-token forms (`1e5`, `1e-8`, `1e+8`, `1E5`,
   `1.0e-8`) + fallbacks above.
3. Typechecker — extend the Float16 precision test: `6.0e-8` rejected
   (subnormal underflow), `5.9604644775390625e-8` admitted (2⁻²⁴, exact).
4. Highlighter — add the bare-exponent float pattern BEFORE the integer
   pattern (tmLanguage is first-match-wins, else `1` of `1e5` highlights
   as int). Check dbriev/rbv/beast grammars for float patterns; mirror
   where present.
5. SPEC §16.1 — normative: "decimal floats with an optional exponent,
   C-style maximal munch (`1.0e-8`, `1e5`)". Tutorial: brief mention.
6. Gates: full lib tests, gemm_h.abv build unchanged, f32 gemv
   spirv-val untouched.

## Resume point — the .abv work after this

This arc unblocks the numeric-literal surface the GPU track needs. The
next `.abv` session resumes at the M2.2 ledger
(`docs/plans/2026-08-31-vitriol-gemm-comparison.md`,
`docs/plans/2026-09-01-m2-tensor-cores.md`):

- The f16 tensor device fault (writes ~25% then dies; y-fill proves the
  fault is driver/NVVM-side) — needs vendor tooling (nsight-compute
  capture, DXC cross-check). The knob `spirv_coopmat` stays OFF until
  root-caused.
- The tiled-f16 spirv-val gap (Function-storage f16 local) — recorded in
  the membership plan's out-of-scope ledger; same blocked surface.
- With exponent literals: the f16 kernel verification surface can
  finally express subnormal-boundary expectations exactly (2⁻²⁴ etc.),
  both in the typechecker's admission gate and in kernel output checks.
- ggml anchor stands: 10.9ms / 12.6 TFLOP/s same-GPU; M2.1 tiled f32 at
  25.3ms / 5.25 TFLOP/s; GEMV M1 at 0.205ms (beats the anchor).

---

## Result (2026-09-02, same session — landed)

- Lexer: dot+exponent and bare C-style float regexes (single commit with
  the tests; logos longest-match verified against every fallback).
- Tests: `exponent_float_literals_are_single_tokens` (9 forms) +
  `exponent_fallbacks_unchanged` (1e, spaced 1e + 5, 1.0-8, 0x1e5,
  1.0expr) + the Float16 subnormal boundary (`6.0e-8` rejected,
  2⁻²⁴ = `5.9604644775390625e-8` admitted) — the boundary proof the
  membership arc could not write.
- Highlighter: bare-exponent float pattern added before the integer
  pattern in briev/dbriev/beast tmLanguage (beast restored to its
  compact formatting after a reformat-only detour).
- SPEC §16.1 normative; tutorial 01-basics mentions the form.

Gates: 2039 lib tests green · gemm_h.abv builds · Praetor clean on the
new lexer functions.
