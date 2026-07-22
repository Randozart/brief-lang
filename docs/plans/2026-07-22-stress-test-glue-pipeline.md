# Stress-Test the GLUE/FFI Pipeline via `brief export`

**Date:** 2026-07-22
**Status:** Complete — all three gaps resolved
**Applies to:** `src/glue/export.rs`, `lib/glue.toml`, `src/library.rs`

---

## Goal

Run `brief export pp-types.bv rust --out /tmp/pp-bridge` end-to-end and have it
produce a working Rust crate that Rust can call with `brief_pp_type_bits("42")`
and get back `"Bits(42)"`.

Each failure reveals a gap in the GLUE pipeline. Fix each gap, repeat, until
the bridge holds.

---

## Architecture: TOML-Driven Language Generation

The key insight: **the export command must not know about specific languages.**
Every target language is configured entirely through `lib/glue.toml`. The TOML
provides:

1. **Type mapping** (`c_type_map`) — Brief types to foreign ABI types
2. **Calling convention** (`calling_convention`) — `"lto"` for LLVM-linked,
   `"c_abi"` for dynamic FFI
3. **Output templates** — file names and their content templates, using
   `{{mustache}}` substitution for bridge function metadata

The Rust export code becomes a generic template engine:
```
Read lib/glue.toml → Extract bridge info → Load templates →
Substitute {{variables}} → Write files → Compile .ll → Build .o → Package
```

No Rust code branches on language. Adding a new language = adding a TOML section.

---

## Template Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `{{bridge_name}}` | Name of the bridge | `"pp-types"` |
| `{{name}}` | Function name | `"brief_pp_type_bits"` |
| `{{params}}` | Parameter list (name: Type) | `"n: &str"` |
| `{{ffi_params}}` | FFI parameter list | `"state: *mut c_void, n: i64"` |
| `{{ffi_call_args}}` | FFI call arguments | `"state, n_as_i64"` |
| `{{args}}` | Argument list (names only) | `"n"` |
| `{{return}}` | Return type | `"String"` |
| `{{c_return}}` | C ABI return type | `"i64"` |
| `{{exports}}` | All exports (used in block templates) | repeated body |
| `{{ffi_decls}}` | All FFI declarations | repeated body |
| `{{param_type_list}}` | Type list for param declaration | `"n: &str"` |

Each export in `{{exports}}` or `{{ffi_decls}}` block has access to the
per-function variables above.

---

## Execution Order

### Step 1 — Add templates to `lib/glue.toml`

Add `[language.templates]` sections for `rust` and `python` with the output
file patterns and their content. Templates use `{{mustache}}` for substitution.
The `{{exports}}` block variable repeats for each exported function.

### Step 2 — Make `run_export_cli` generic

Replace the `match language { "python" => ..., "rust" => ... }` hardcoded
generators with generic template processing:

1. Read templates from `glue_target.templates` (from TOML)
2. For each template file, substitute `{{variables}}` across all exports
3. Write each file to the output directory
4. The LLVM IR generation + compilation + packaging stays the same

### Step 3 — Fix `generate_with_exports` to use the full LLVM backend

Currently `library::generate_with_exports` emits `ret i64 0` stubs. Change it
to compile the bridge with the full LLVM backend, producing real function bodies.
Then add C-ABI export wrappers on top.

### Step 4 — Test end-to-end

```bash
brief export pp-types.bv rust --out /tmp/pp-bridge
cd /tmp/pp-bridge/pp-types-bridge
cargo build
```

The generated crate should compile and produce a binary that can call
the exported Brief functions.

---

## What Each Gap Tests

| Gap | What's tested | Success Criteria |
|-----|---------------|-----------------|
| **1** | TOML-driven template generation works | `brief export` produces files without hardcoded branches |
| **2** | Full backend codegen for exports | Generated `.ll` has real function bodies, not `ret i64 0` |
| **3** | Generated Rust crate compiles | `cargo build` in the output dir succeeds |
| **4** | Bridge functions return correct results | `brief_pp_type_bits("42")` returns `"Bits(42)"` |

The stress test is the pipeline itself. Each gap we discover and fix proves
the architecture is sound.
