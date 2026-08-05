# Completion Report — Plugin/Macro Rework + Bits Model (2026-08-01)

**Branch:** `feat/plugin-macro-rework` → merged into `main` at `b00c9681`
**Plan:** `docs/plans/2026-08-01-plugin-macro-rework.md`
**Scope:** 17 commits (`f546af1c`..`b00c9681`), 142 files, +5379/−1457
**Final state:** 1359 tests green, release build clean, Praetor no new
diagnostics, zero benchmark regressions.

---

## 1. Goal

Part A of the plan rebuilt the CLI/macro surface (replacing the removed `[#]`
entry marker, adding `print!`/`println!`/`get_env!`/`entry!`/`args!` plugins,
the concurrency gate, and flat scripting). Part B (the **bits model**) rebuilt
the String representation as a protocol member whose value is a `ptr` to a
length-prefixed `[len][bytes]` buffer, retiring the SSO/fat-pointer machinery
and the legacy String types. Both parts are complete and merged.

## 2. Commits

| Commit | Phase | What |
|--------|-------|------|
| `f00496a6` | 0 | FFI audit — no print/env regression; baseline table, guard test, harness fixes |
| `8962a2a1` | 1 | lowercase `print!`/`get_env!` macros + `{}/{{}}` formatting; B0-aligned String fixes; plan Part B |
| `7dceefb7` | B0 | bits model — String = `ptr` to `[len][bytes]` everywhere; flexible/fixed width rule; sound `!range` |
| `0efccf63` | — | Praetor: record removal of the broken pre-commit hook (file-target no-op) |
| `ba1d02b4` | B1 | content equality + `#String` bitwise defaults (deref operands) |
| `be67baa6` | 2 | `[#]` entry-point marker removed |
| `a8a4f421` | 3a | CLI argv runtime capture (`main(i32,ptr)` + globals + `__argv_*`) |
| `6fb929a8` | 3b | `entry!`/`args!` plugin + malformed-i8-range fix |
| `9dd1398d` | 3c | concurrency gate (rule #21) |
| `0984f47c` | 4 | flat-scripting plugin (one-shot opening node) |
| `36589b6b` | 5 | docs, SPEC, highlighter, final benchmark table |
| `4452ae3d` | B2 | content-view casts + encoding door (`#String↔#Bit`) |
| `30922fc6` | B3 | length-op dispatch (Size = char count, Bytes = header read) |
| `9106bb51` | B4a | retire SSO layer + `is_string_like` structural heuristic |
| `c5ae8b78` | B4b | retire legacy String types (StaticString/UTF8View/SmallString64) |
| `fc8f594a` | B4c | docs to bits model + `!>` metadata syntax + benchmark note |
| `b00c9681` | — | Phase 5 fix: commit the `!`-suffix front-end plugin doc section |

## 3. Part A — CLI / Macro Surface

- **`print!`/`println!`/`get_env!`/`get_env_int!`** plugins (Parsed stage):
  Rust-style format strings, `{}`/`{n}`/`{{}}`, direct `__print_*` FFI calls.
- **`[#]` removed** (Phase 2): a syntax error with a clear message; the
  entry-point marker is replaced by `entry!`/`args!`.
- **CLI argv capture** (3a): every loop-engine `main` is
  `main(i32 %argc, ptr %argv)` storing into `@__briv_argc`/`@__briv_argv`;
  runtime helpers `__argv_count/__argv_get/__argv_has/__argv_value/
  __argv_command`; `lib/std/cli.bv`.
- **`entry!`/`args!`** (3b): one-shot CLI subcommand guards
  (`entry_cmd() == "<cmd>" && !__entry_<cmd>_done` + flip), snapshot arg
  fields, synthesized wrappers for non-reactive `defn` entry points.
- **Concurrency gate** (3c): any eligible-but-unclassified reactive pair is a
  hard compile error (rule #21). `check_satisfiable` detects the
  subcommand-dispatch UNSAT pattern; `sync<group> node` parsing added;
  benchmarks audited (`async node` / `sync<counters>`).
- **Flat scripting** (4): `defn main()` and bare top-level lets run exactly
  once via a synthesized `node __script_main` (fixes the dead `briv_main`).

## 4. Part B — Bits Model

- **String = `ptr` to `[len][bytes]`** everywhere (B0): casting graph
  (`#String` → `ptr`), `protocol_llvm_type`, literals, state adapters, FFI.
  No `{ i64, i64 }`/`i128` String claim remains. State slot = one i64 word.
- **Flexible/fixed width rule**: `Int`/`UInt`/`String` are one machine word
  (derived from `int_bits`); `Int32`/`Int64` are absolute; String's primordial
  is flexible like Int/UInt.
- **Content equality + bitwise** (B1): `Eq`/`Ne` compare payload bytes via
  `briv_str_eq`; `& | ^ ~` operate on content via
  `briv_str_band/bor/bxor/bnot`.
- **Content view + encoding door** (B2): `#String → #Bit` = buffer address
  (`ptrtoint`); `#Bit → #String` = UTF8 wrap via `briv_bits_to_str`
  (header materialized by construction); `CastFrom(#Bit)` overrides.
- **Length ops** (B3): `x.^Len` = UTF8 char count (`briv_char_len`);
  `x.^^Bytes` = O(1) header read.
- **Legacy retirement** (B4): SSO layer + `is_string_like` (structural
  heuristic) + StaticString/UTF8View/SmallString64 deleted. `grep` over
  `src/` + `lib/std` returns zero for all of them.
- **`!>` metadata syntax**: the `<~` metadata form was removed (writing `<~`
  is a parse error); `!> key: value;` is the sole form. SPEC grammar, learn-
  briv, and architecture docs updated.

## 5. Bugs found & fixed (all logged in BUGS.md)

1. **Malformed `!range` on narrow fields** (B0/B3b): `!{ i64 0, i64 256 }` on
   a `load i8` (Bool/UInt8/Int8) crashed clang — range bounds must match the
   load width; vacuous ranges are now skipped.
2. **SSO tag bit corrupted String addresses** (B1): OR-1 static tag on literal
   stores made `briv_str_eq` read a misaligned header.
3. **`async node` prefix dropped the flag** (3c): explicitly-async nodes were
   never classified.
4. **Reflect-read String field eliminated as dead** (B3): `collect_identifiers`
   missed `Expr::Reflect`, so a reflect-only String `let` vanished from %State.
5. **HashWord categories kept the `#` prefix** (B2): casts to/from `#Bit`
   found no base lanes and silently fell through to invalid LLVM coercion.
6. **Concat result tagged as i64** (B4a): the OR-2 temp bit broke `ptr`
   consumers (`__print_str`); now returns the untagged ptr.
7. **`briv_main` dead code** (Phase 4): `defn main` was emitted but never
   invoked; the script plugin fixed it.
8. **Broken pre-commit hook** (Praetor): `--target <file>` silently no-op'd;
   removed per the no-hook decision.

## 6. Benchmark results

The final runtime table (`benchmarks/results/2026-08-01-plugin-rework-final.md`)
shows **zero MISMATCH and zero regression >0.05 ratio** vs the pre-rework
baseline (commit `f546af1c`). All deltas are run-to-run noise (±0.03); fasta
improved (−.07). The B4 SSO/legacy-type removals are compile-time-only (the
`feature_sso_strings` flag was always off), so runtime numbers are unchanged.

## 7. Known follow-ups (deferred, not in this plan)

- Subtype String-literal coercion: `let r: Latin1String = "..."` needs an
  implicit cast (the typechecker does not coerce String literals to `#String`
  subtypes); explicit casts fire the `CastFrom(#Bit)` override path.
- `Slice<T>` content ops (fat-pointer sequence views) remain a separate
  concern (B-D8 defers them).
- The encoding registry (`config/encodings.toml`) still exists for protocol
  variants; its interplay with the now-bare String type is untested for
  non-UTF8 variants.
