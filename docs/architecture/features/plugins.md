# Plugin Architecture & Prelude Design

**2026-07-15**: Phase 2 decisions. Supersedes `macro.md`.

## How $ intrinsics work

`$` intrinsics (e.g. `InsertRegistryImport$("std/prelude.bv")`) are Rust
functions that operate directly on `&mut Vec<TopLevel>` (AST) and
`&mut TypeUniverse` — no text-level expansion. A `$(Front)` block's body
is a sequence of `$` calls; when the stage runs, each call dispatches to
the corresponding `execute_$intrinsic()` Rust function inline.

## Prelude per extension

Not all backends need the same types. The prelude is a system plugin
(`plugins/front/prelude.bv`) that injects stdlib imports into user code.
Different extensions get different preludes:

| Extension | Backend | Plugin | Stdlib entry point |
|-----------|---------|--------|--------------------|
| `.bv` | LLVM | `prelude` | `std/prelude.bv` — Int, Bool, Float, String, collections, Option, Result |
| `.ebv` | LLVM embedded | `prelude` | same |
| `.rbv` | Webstack | `prelude` | same |
| `.abv` | GPU | `prelude` | same |
| `.cbv` | CIRCT | `prelude-hw` | `std/hardware.bv` — Cell, Wire, Register, Bit, etc. |

The prelude plugin file is just a `$(Front @ highest)` block that calls
`InsertRegistryImport$()`. System plugins import what they need manually.

## Plugin naming

System plugins discovered from `plugins/{stage}/<name>.bv` are registered
with the name `<name>` (the file stem, no prefix/suffix). This lets
`config/targets.toml` reference them by simple name in the `plugins` list.

## BVIR path not needed for in-process plugins

The BVIR serialize→external→deserialize path is only for the legacy
`--plugin` CLI flag (external executables). In-process plugins via
`PluginManager` work directly on AST/IR in memory.

## Webstack output

The webstack backend emits HTML + CSS + SVG + TypeScript, not WASM.
`.rbv` uses it.
