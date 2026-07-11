# `--library` Mode: Reusable LLVM IR Module for GLUE

**Date:** 2026-07-10
**Status:** Plan — pre-implementation
**Depends on:** GLUE v2 FFI Unification (commit `45a5b1b`, plan `2026-07-10-glue-v2-ffi-unification.md`)
**Superseded by:** `docs/plans/2026-07-11-library-mode-completion.md` — Phase 15
of the overall roadmap extends this plan with `.o`/`.a` packaging, bindgen
completeness, type marshaling, and end-to-end testing.

---

## 1. Goal

Add a `--library` flag to `brief build` that produces a reusable LLVM IR module
(`.ll`) with `#export` wrappers and an initializer, but **no main function**.
This `.ll` module is the compiled artifact that foreign build systems consume
as part of the GLUE protocol:

```
brief build --library bridge.bv --out ./out
  → out/bridge.ll   (library-mode LLVM IR — #export wrappers + __brief_init_state)
```

Combined with:
```
brief export bridge.bv rust --out ./out
  → out/bridge-bridge/bridge-exports.dbvl  (metadata for the foreign build system)
```

The foreign build system (e.g., Rust `build.rs`) reads `bridge-exports.dbvl` to
generate bindings, and `llc bridge.ll` to produce a linkable `.o`.

---

## 2. Current State

- `brief build --llvm` already produces a `.ll` file and exits before linking
  (line 3818-3839 of `src/main.rs`)
- `library_mode` is a `bool` field on `CompilerContext` (line 60 of
  `src/backend/llvm/context.rs`), set via `with_library_mode()` (line 947 of
  `src/backend/llvm/mod.rs`)
- `emit_library_shim()` exists in `src/backend/llvm/emit_toplevel.rs:1971` —
  emits `__brief_init_state()` (allocates `%State`, returns ptr) and
  `__glue_release()` (no-op placeholder), skips `main()` function
- **No CLI flag sets `library_mode`** — it is only available programmatically
- `--emit-bindings` flag (line 1733 of `src/main.rs`) generates C/Rust/Python
  bindings from `#export` definitions via `src/backend/bindgen.rs`, but this is
  a separate path from the GLUE export pipeline

---

## 3. Changes Required

### 3.1 CLI flag: `--library`

Add `--library` to the `build` subcommand argument parser (around line 3776 of
`src/main.rs`). Model it on the existing `--llvm` flag:

```rust
let mut emit_llvm = false;
let mut library_mode = false;

// In the arg loop:
} else if arg == "--library" {
    library_mode = true;
    i += 1;
}
```

### 3.2 `run_llvm_compile` — accept `library_mode` parameter

Add `library_mode: bool` to `run_llvm_compile`'s 20+ parameter list (line 2327).
Thread it through to the backend:

```rust
fn run_llvm_compile(
    file_path: &PathBuf,
    out_dir: Option<&Path>,
    library_mode: bool,
    // ... existing params unchanged ...
```

At the backend creation site (line 2594):
```rust
let mut llvm_backend = crate::backend::llvm::LlvmBackend::new()
    ...
    .with_library_mode(library_mode);
```

### 3.3 Handle `--library` path in `build` subcommand

Similar to the `--llvm` handler (line 3818), add a `--library` handler that:

1. Calls `run_llvm_compile` with `library_mode=true`
2. After successful IR emission, prints the output path
3. Exits early (before linking code runs)

```rust
if library_mode {
    let result = run_llvm_compile(&path, out, true, ...);
    match result {
        Ok(ll_path) => {
            println!("  Library LLVM IR: {}", ll_path.display());
            std::process::exit(0);
        }
        Err(e) => { eprintln!("{}", e); std::process::exit(1); }
    }
}
```

### 3.4 Flat control flow

All changes must follow the max-2-levels nesting rule:

```rust
// ✅ Correct: guard clause
if !library_mode {
    // Normal compilation — backend emits main()
}
// library_mode path — backend already emitted init_state + exports

// ✅ Correct: flat
if !emit_llvm && !library_mode {
    return; // short-circuit when not producing a binary
}
```

No arrowhead code. No nesting beyond 2 levels.

### 3.5 What the output `.ll` contains

In library mode, `LlvmBackend::generate()` emits:

1. **Header**: `ModuleID`, `datalayout`, `triple` (from `emit_header`)
2. **Declares**: `llvm.assume`, `malloc`, `free`, foreign functions
3. **Globals**: string constants, constant aggregates
4. **`%State` type**: struct with all state fields
5. **`#export` wrappers**: `define dso_local <ret> @<export_name>(ptr %state, <params>)`
6. **init_state**: `define dso_local i64 @__brief_init_state()` — allocates
   `%State`, initializes to zero/null, returns pointer as `i64`
7. **`__glue_release`**: `define dso_local void @__glue_release(i64)` — no-op
8. **No main function**: reactor loop, SSA main, thread pool metadata are all absent

---

## 4. Files Changed

| File | Lines changed | Change |
|------|---------------|--------|
| `src/main.rs` (arg parse) | ~8 | Add `--library` flag detection |
| `src/main.rs` (handler) | ~20 | `--library` path: call `run_llvm_compile` with `library_mode=true`, exit early |
| `src/main.rs` (fn sig) | ~2 | Add `library_mode: bool` param to `run_llvm_compile` |
| `src/main.rs` (backend) | ~1 | `.with_library_mode(library_mode)` at backend creation |
| `src/main.rs` (usage) | ~2 | Add `--library` to usage strings |
| `docs/plans/2026-07-10-library-mode-emit-llvm.md` | ~100 | This plan document |

No changes to the LLVM backend itself — all infrastructure already exists.

---

## 5. Tests

- `cargo test --lib` — all 1448 tests must pass
- Manual: `brief build --library examples/glue-macro.bv --out /tmp/glue-test`
  → verifies `.ll` contains `@__brief_init_state` and no `@main`
- Manual: `brief build --library examples/glue-macro.bv --llvm` → should be
  rejected or ignored (mutually exclusive with `--llvm`)

---

## 6. Future Work (Out of Scope)

- **`bridge-exports.dbvl` auto-generation** as part of `--library` — currently
  handled separately by `brief export`
- **Library mode for transactions** — `#export` on callable `txn` items
  (currently only `defn` items get export wrappers)
- **Configurable target triple** — `emit_header` hardcodes
  `x86_64-unknown-linux-gnu`; library mode may want a different triple
