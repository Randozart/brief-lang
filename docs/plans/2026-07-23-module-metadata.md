# Generic Module Metadata Emission — Zero-Cost Cross-Language Bridges

**Date:** 2026-07-23
**Status:** Plan

---

## Goal

The LLVM backend emits C extension module init data (`PyMethodDef[]`,
`PyModuleDef`, `PyInit_<name>`) from config-driven instructions, not
hardcoded language knowledge. Adding a new language = `.bv` file +
TOML entry. Zero Rust changes.

## Architecture

```
lib/glue/python.bv
  └── InjectTypeLayout$ declares PyMethodDef, PyModuleDef struct layouts
      └── Type universe stores layout (bytes, fields, offsets)
          └── Config: [python] module_init = true
              └── LLVM backend emit_module_metadata():
                  1. Read layout from universe
                  2. Read export defn list from program
                  3. Emit global array, struct, init function
                  4. .so is now a valid Python C extension
```

## Implementation

### Step 1: TOML Config + `.bv` Layout Declarations

Add `module_init = true` to the python target in `lib/glue.toml`.

Add `InlineTypeLayout$` calls to `lib/glue/python.bv` declaring the
Python C API struct layouts so the backend knows the field offsets.

### Step 2: Pass `GlueTarget` to LLVM Backend

Add `glue_config: Option<GlueTarget>` to `CompilerContext` in
`context.rs`. Set it from `compile.rs` before `codegen()`.

### Step 3: `emit_module_metadata()` in `mod.rs`

New method gated by `module_init`. Emits:
- `@methods` — array of `PyMethodDef` structs (one per `export defn`)
- `@moduledef` — `PyModuleDef` struct referencing the method array
- `PyInit_<name>` — entry point Python's `import` looks for

### Step 4: Python can `import bridge` directly

```python
import bridge
print(bridge.add(3, 4))  # 7, no ctypes
```

## Files Touched

| File | Change |
|------|--------|
| `lib/glue.toml` | `module_init = true` for python |
| `lib/glue/python.bv` | `InjectTypeLayout$` calls |
| `src/backend/llvm/context.rs` | `glue_config` field |
| `src/backend/llvm/mod.rs` | `emit_module_metadata()` |
| `src/compile.rs` | Pass config to backend |
