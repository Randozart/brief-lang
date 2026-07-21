# Optimized-Stage Plugins

Plugins in this directory run at the `$(Optimized)` stage, after backend
optimizations. They operate on the optimized IR text and can perform
final validation, instrumentation, or target-specific fixups.

**2026-07-21:** Renamed from `back/` to `optimized/` as part of the granular
pipeline expansion.  See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.

## Current plugins

(none)

## Writing an Optimized plugin

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
