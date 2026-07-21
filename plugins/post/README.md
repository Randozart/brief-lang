# Verified-Stage Plugins

Plugins in this directory run at the `$(Verified)` stage, after protocol
verification and before code generation. They operate on the verified AST.

**2026-07-21:** Renamed from `post/` to `verified/` as part of the granular
pipeline expansion.  The old `$(Post)` stage for IR text is now `$(Generated)`.
See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.

## Current plugins

- `validate-trg.bv` — warns on unresolved dynamic trigger references

## Writing a Verified plugin

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
