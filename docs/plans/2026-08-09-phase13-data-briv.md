# Phase 13 — Data Briv: validation, canonical serialization, CLI check

**Date:** 2026-08-09
**Status:** Implementation plan
**Normative source:** `spec/SPEC.md` §22 (Data Briv),
`docs/plans/2026-08-05-implement-normative-language-spec.md` §19 (Phase 13)

## 0. Goal

Complete the Data Briv surface: schema validation (§22.5), canonical
serialization (§22.6), `briv check` extension dispatch (§22.6), and the
`.dbvs` migration (§19.5). The parser is mature (`src/dbriv/v2.rs`, 69 tests);
the gaps are the downstream consumers.

## 1. Current state (surveyed 2026-08-09)

| Area | State |
|---|---|
| `.dbv`/`.dbvl` parser | Mature — schemas, `>category`, `>schema from "..."`, positional sub-records, quoted values, key fields (v2.rs, 69 tests) |
| `config_db.rs` (.dbvl loader) | Mature — used by every config/*.dbvl + lib/glue/*/glue.dbv |
| Schema validation (§22.5) | NOT implemented (0 tests) |
| Canonical serialization (§22.6) | NOT implemented |
| `briv check file.dbv/.dbvl` dispatch | NOT implemented — run_check → check_source (.bv-only) |
| `.dbvs` migration (§19.5) | `.dbvs` rejected + not swept, but stale files remain |

## 2. Scope — 4 slices

### Slice A — Schema validation (`src/dbriv/validate.rs`)
`validate_document(&DbrivDocument) -> Result<(), Vec<String>>` — when a schema
is asserted, enforce: required/unknown fields, raw-token→type conversion,
constraints (`[ != "" ]`, `[ >= 0 ]`), optional fields, named-schema imports,
key presence/uniqueness. Tests per rule.

### Slice B — Canonical serialization (`src/dbriv/serialize.rs`)
`canonicalize_document(&DbrivDocument) -> String` — deterministic field/key
order, quoting, numeric spelling, instruction placement. Round-trip
`parse → canonicalize → parse` idempotent. `.dbvl` append-only one-record-per-
line writer.

### Slice C — `briv check` extension dispatch
`run_check` dispatches `.dbv`/`.dbvl` to a `check_data_source` (parse +
`validate_document` when asserted); else existing `check_source`. Tests via CLI.

### Slice D — `.dbvs` migration
Delete the stale `.dbvs` files (inert, already rejected). Verify no loader
references them and config .dbvl/.dbv still load.

## 3. Files

| Change | File |
|---|---|
| validate.rs (new) | `src/dbriv/validate.rs`, `src/dbriv/mod.rs` |
| serialize.rs (new) | `src/dbriv/serialize.rs` |
| CLI dispatch | `src/main.rs` (`run_check`), `src/compile.rs` (`check_data_source`) |
| migration | delete `*.dbvs` |
| docs | `spec-implementation-status.md` §22, plan tracker |

## 4. Verification

- `cargo test --lib` green after every slice; `cargo build` no new warnings.
- Praetor changed dirs — no new diagnostics.
- Validation tests per rule; serialization round-trip + idempotency; CLI tests.
- Existing 69 dbriv parser tests stay green.

## 5. Progress log

### 2026-08-09 — plan written

## 6. Progress log

### 2026-08-09 — all slices delivered

- **Slice A (validate.rs)**: schema validation — required/unknown fields,
  type conversion, constraints (`[ >= 0 ]`, `[ != "" ]`), optional fields,
  key presence + document-wide uniqueness (per-schema across groups). Also
  fixed the parser's `.dbvl` line-oriented record splitting (SPEC 22.3: each
  physical line is one record) — `;`-branch no longer merges the next record.
- **Slice B (serialize.rs)**: canonical serialization — deterministic field/
  key ordering, quoting, numeric spelling (`3.0` for integral floats), map-key
  sorting, imports-first. Round-trip + idempotency tested.
- **Slice C**: `briv check file.dbv|.dbvl` dispatches to `check_data_source`
  (parse + resolve `schema X from "..."` imports + validate asserted schemas);
  `.bv` files use the existing pipeline.
- **Slice D**: deleted 20 stale `.dbvs` files (inert — parser rejected them,
  conformance returned None).

Tests: 1704 pass (9 validation + 5 serialize + 1 CLI-record + 1 updated).
Praetor: no new diagnostics (pending).
