# Linked-Stage Plugins

Plugins in this directory run at the `$(Linked)` stage, after the final
binary is produced.  They operate on the binary file path via `Bin$`.

**2026-07-21:** New stage in the granular pipeline expansion.  See
`docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md`.

## Current plugins

(none)

## Writing a Linked plugin

Use binary operations on the explicit `Bin$` target:

```brief
$(Linked) {
    Bin$.Run$("strip --strip-unnecessary {{path}}");
};
```

See [`docs/architecture/features/plugins.md`](../../docs/architecture/features/plugins.md).
