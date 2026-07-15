# Post-Stage Plugins

Plugins in this directory run at the `$(Post)` stage, after code generation.
They validate or transform the generated IR (`.ll`, `.mlir`, `.ts`).

## Current plugins

- `validate-trg.bv` — warns on unresolved dynamic trigger references

## Writing a Post plugin

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
