# Typed-Stage Plugins

Plugins in this directory run at the `$(Typed)` stage, after type-checking
and before code generation. They operate on the type-checked AST and can
perform semantic analysis, contract validation, or transformation.

**2026-07-21:** Renamed from `mid/` to `typed/` as part of the granular
pipeline expansion.  See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.

## Current plugins

- `auto-main.bv` — Sets `[#]` entry marker on `defn main` / `txn main`
- `entry-check.bv` — Verifies at least one entry mechanism exists

## Writing a Typed plugin

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
