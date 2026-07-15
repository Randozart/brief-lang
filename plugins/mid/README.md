# Mid-Stage Plugins

Plugins in this directory run at the `$(Mid)` stage, after type-checking
and before code generation. They operate on the type-checked AST and can
perform semantic analysis, contract validation, or transformation.

## Current plugins

(none)

## Writing a Mid plugin

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
