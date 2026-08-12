# Plugin-Driven Bridge: Python C Extension via Briev

**Date:** 2026-07-23
**Status:** Plan → Implementation

## Goal

Remove all hardcoded Python knowledge from the LLVM backend.
The generator `$defn` emits a C file with `PyMethodDef[]`,
`PyModuleDef`, `PyInit_*`, and `_pybridge_*` wrappers, compiles
it with `ShellCmd$`, and injects the `.o` into the link step
via `InsertObject$`. Zero Rust changes per language.

## Changes

| File | Change |
|------|--------|
| `src/plugin/mod.rs` | Add `extra_objects: Vec<PathBuf>` |
| `src/macros/eval.rs` | Add `InsertObject$` handler |
| `src/compile.rs` | Merge `pm.extra_objects` into linker |
| `lib/glue/generator.bv` | Add `build_py_module` `$defn` |
| `src/backend/llvm/mod.rs` | Remove `emit_module_metadata()` |
