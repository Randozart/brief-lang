# Optimized-Stage Plugins

Plugins in this directory run at the `$(Optimized)` stage, after backend
optimizations and before linking.  They operate on the optimized IR text.

**2026-07-21:** Replaces the old `$(Back)` stage.  See
`docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.

## Current plugins

(none)

## Writing an Optimized plugin

Same as Generated-stage: text operations on the implicit `Ir$` target.  See
[`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
