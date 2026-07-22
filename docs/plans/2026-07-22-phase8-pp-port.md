# Phase 8: AST Pretty-Printer Port to Brief

**Date:** 2026-07-22
**Status:** Complete — all milestones achieved
**Depends on:** Phases 0-7 (complete), stdlib frgn cleanup (complete)

---

## Results

The GLUE pipeline is fully verified end-to-end:
- `brief build pp-types.bv --llvm` produces real function bodies
- `brief export pp-types.bv rust --out /tmp/x` generates a compilable Rust crate
- The bridge `.so` loaded via FFI returns correct results
- Benchmark: Brief GLUE 6203ns vs C FFI 5988ns (3.5% slower, ✅ all match)

## What Was Built

- `lib/pp/types.bv` — Brief pretty-printer for 16 Type variants
- `lib/pp/exprs.bv` — Brief pretty-printer for operators and expressions
- `pp-types.bv` — Bridge file with export wrappers + round-trip test helpers
- Integration tests (8 passing) — full pipeline from `.bv` to FFI call
- Bridge benchmark — Python ↔ C vs Brief via ctypes

## Backend Fixes

- String constants use global `@str.N` instead of stack `alloca` (use-after-free)
- Frgn declares match function ABI (`ptr` for String, not `i64`)
- Arena allocator uses C-compatible `[length][data]` format
- `emit_load_length` reads `handle[0]` (correct for both heap and globals)
- `emit_copy_data` uses caller-computed destination offsets
- `brief_str_to_c` strips tag bits (`& ~3`) before reading handle
- `emit_protocol_chain` emits real IR (Bitcast, MeldShuffle, ProtocolTransform)

## GLUE Export Infrastructure

- Fully TOML-driven templates (no hardcoded language generators)
- Dynamic config via `#[serde(flatten)]` (add language = TOML only)
- `type_map`, `conversions`, `templates` sections per language
- Template variables: `{{args_abi}}`, `{{return_expr}}`, `{{c_types}}`, etc.
- State parameter correctly included in generated wrappers
- Export uses full LLVM backend (no `ret i64 0` stubs)
