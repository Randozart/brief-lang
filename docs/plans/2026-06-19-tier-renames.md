# Tier Renames: Graphic→Accelerated, Hardware Embedded→Circuit

**Date:** 2026-06-19
**Status:** Implementation plan

## Rename Summary

| Old | New | Extension change | Alternative name(s) |
|-----|-----|------------------|---------------------|
| Graphic Briev | Accelerated Briev | `.gbv` → `.abv` | "Briev Accel" |
| Hardware Embedded Briev | Circuit Briev | `.hebv` → `.cbv` | "Briev Circuit" |
| (Embedded Briev) | (unchanged) | `.ebv` | "Briev Embed" |
| (Rendered Briev) | (unchanged) | `.rbv` | "Briev Render" |
| (Data Briev) | (unchanged) | `.dbv`/`.dbvs`/`.dbvl` | "D-Briev", "Briev Data" |

## Internal naming (no change)

- `StrictMode::Gpu` — stays `Gpu` (describes GPU compilation mode, not a brand)
- `is_gpu_extension()` — stays (checks if file is `.abv` now)
- `with_gpu_mode()` — stays (same reasoning)

## Files to modify

### Source code (must compile)
- `src/main.rs` — extension checks, help text, error messages (~40 occurrences)
- `src/ast.rs` — `StrictMode::Gpu` doc comment (1 occurrence)
- `src/typechecker.rs` — validation messages referencing `.gbv` (~8 occurrences)
- `src/hardware_validator.rs` — all `.hebv`/`hebv`/`Hardware Embedded Briev` (~15 occurrences)

### Test assets
- `test_gbv.gbv` → rename to `test_abv.abv`

### Agent guidelines
- `AGENTS.md` — file types table + sugar rules + critical philosophy sections
- `CLAUDE.md` — file types table

### Documentation
- `README.md` — file type table, briev references
- `docs/reference/BRIEV_LANGUAGE_REFERENCE.md` — file type descriptions
- `docs/architecture/features/graphic-briev.md` → rename to `accelerated-briev.md` + content update
- `docs/plans/2026-06-18-graphic-briev.md` — update references
- `docs/plans/2026-06-18-gpu-io-intrinsics.md` — update references
- `spec/SPEC.md` — extension references
- `learn-briev/` — update relevant file type mentions

### Tooling
- `syntax-highlighter/package.json` — extension registrations
- `syntax-highlighter/client/extension.js` — file watcher patterns

## Execution order

1. Write plan doc
2. Update all `.rs` source files (main, ast, typechecker, hardware_validator)
3. Rename test file
4. Update agent guidelines (AGENTS.md, CLAUDE.md)
5. Rename architecture feature doc + update
6. Update remaining documentation
7. Run `cargo test --lib`
8. Run `cargo build`
