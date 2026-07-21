# Verified-Stage Plugins

Plugins in this directory run at the `$(Verified)` stage, after protocol
verification and before code generation.  They operate on the verified AST
and can perform semantic validation, contract checks, or prepare annotations.

**2026-07-21:** Replaces the old `post/` directory (which was renamed to
`generated/` for IR-stage plugins).  See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.

## Current plugins

- `validate-trg.bv` — Warns on unresolved dynamic trigger references

## Writing a Verified plugin

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
