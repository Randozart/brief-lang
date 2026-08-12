# Phase 15 — Library Mode Completion

**Date:** 2026-07-11
**Status:** Plan — pre-implementation
**Depends on:** Phases 0–14 (Extensible Types → Derivation & Synthesis →
`.dbvl` archive); GLUE v2 (`docs/plans/2026-07-10-glue-v2-ffi-unification.md`)
**Proposal by:** [@revred](https://github.com/revred) — review of Briev's
FFI export pipeline identified five gaps between the existing infrastructure
(`library_mode`, `emit_library_shim`, bindgen) and a consumable C-callable
library. This plan closes those gaps.
**Supersedes:** `docs/plans/2026-07-10-library-mode-emit-llvm.md` — the older
plan covered `--library` flag wiring and `.ll` emission; Phase 15 extends it
through `.o`/`.a` packaging, bindgen completeness, type marshaling, and
end-to-end testing.

---

## Overview

Briev's `--library` mode currently emits a `.ll` file with `#export` wrappers
and `__briev_init_state`, but stops there. The external reviewer identified
five gaps that prevent shipping a consumable C-callable library:

| # | Gap | Source |
|---|-----|--------|
| 1 | `.ll` → `.o` → `.a`/`.so` packaging missing | External review |
| 2 | `__briev_init_state`/`__glue_release` not in generated headers | External review |
| 3 | `Bool`/`String` not marshaled at the FFI boundary | External review |
| 4 | No end-to-end C driver test | External review |
| 5 | `#export` is a pragma, not a keyword | Internal design |

This plan also adopts the `export` keyword as the canonical syntax (replacing
`#export` pragma), and organizes all library-mode work as Phase 15 — the
natural continuation after derivation Phases 8–14.

### What we are taking from @revred's review

The external review identified the exact boundary where Briev's existing
infrastructure (which we built) stops and the consumable artifact begins.
We adopt their analysis of the five gaps, their proposed v1 scope (pure
`defn`s only, no transactions), and their acceptance criterion (a C program
links the archive and gets correct results). The implementation details
(marshal via `zext`/`trunc` for `Bool`, `getelementptr`/`bitcast` for
`String`, `ar rcs` for static libs) are our own design within Briev's
existing backend conventions.

---

## CLI Model

```bash
# The three output modes, fully orthogonal:

# 1. Archive for decoupled backends (Phase 12)
briev compile main.bv --archive build/main.dbvl

# 2. Library for C/foreign linking (Phase 15)
briev build --library main.bv --out out/
  → out/main.ll       (LLVM IR — already planned)
  → out/main.o        (llc -filetype=obj — Phase 15.1)
  → out/libmain.a     (ar rcs — Phase 15.2)
  → out/briev_types.h (bindgen + init_state/release — Phase 15.3)

# 3. GLUE bridge metadata (GLUE v2)
briev export main.bv rust --out out/
  → out/bridge-exports.dbvl
```

---

## Step 15.0 — `export` keyword (replaces `#export` pragma)

**File:** `src/lexer.rs`, `src/parser.rs`, `src/ast.rs`

**What:** Add `export` as a first-class keyword. `export defn` becomes a
distinct syntactic form rather than a `#export` annotation on a plain `defn`.

**Lexer:** Add `Token::Export` keyword.

**New AST variant:**
```rust
/// A top-level export declaration wrapping an exportable definition.
/// 2026-07-11: Phase 15.0
pub struct Export {
    /// The exported definition or transaction.
    pub inner: Box<TopLevel>,
    /// Optional external name (defaults to inner's name).
    pub export_name: Option<String>,
}
```

Add to `TopLevel` enum:
```rust
Export(Export),
```

**Parser:** In `parse_top_level()`:
```rust
Token::Export => {
    self.advance();
    let defn = self.parse_definition()?;
    Ok(TopLevel::Export(Export {
        inner: Box::new(TopLevel::Definition(defn)),
        export_name: None,
    }))
}
```

**Syntax:**
```briev
export defn add(a: Int, b: Int) -> Int { term a + b; };
```

**Migration:** The old `#export` pragma continues to work during a
deprecation window. The parser recognizes both forms and emits a warning
for `#export`.

**Tests:**
- `test_parse_export_keyword_defn`: `export defn f() -> Int { term 0; };`
- `test_parse_export_keyword_multiple`: Multiple exports
- `test_parse_export_old_pragma_warning`: `#export defn ...` → warning emitted
- `test_ast_export_variant`: Export struct constructed and queried correctly

---

## Step 15.1 — `llc -filetype=obj` after `.ll` emission in `--library` mode

**File:** `src/main.rs` (handler around line 2695)

**What:** After `--library` emits the `.ll` file, run `llc` to produce a
`.o` file instead of returning early.

**Current code (line 2695):**
```rust
if library_mode {
    return Ok(output_file);  // stops at .ll
}
```

**New code:**
```rust
if library_mode {
    let obj_path = out_dir.join(format!("{}.o", stem));
    let mut llc = std::process::Command::new("llc");
    llc.args(["-filetype=obj", "-O2", "--relocation-model=pic"])
        .arg("-o")
        .arg(&obj_path)
        .arg(&output_file);
    let status = llc.status()
        .map_err(|e| format!("llc failed: {}", e))?;
    if !status.success() {
        return Err("llc returned non-zero exit status".into());
    }
    println!("  Library object: {}", obj_path.display());
    return Ok(obj_path);
}
```

**Why `-O2` not `-O3`:** The existing library-mode plan notes that `-O3`
can eliminate external globals like `@stdout`. We match the existing comment.

**Tests:**
- Manual: `briev build --library test.bv --out /tmp/lib` → produces `.ll`
  AND `.o`
- `test_library_mode_obj_exists`: After compile, `.o` file exists and is
  non-empty
- `test_library_mode_llc_fails`: If `llc` not on PATH, error message is clear

---

## Step 15.2 — Static library packaging (`ar rcs`)

**File:** `src/main.rs` (extends Step 15.1 handler)

**What:** After `.o` is produced, run `ar rcs lib<stem>.a <stem>.o` to
produce a static library. Optionally support `-shared` for `.so`.

```rust
if library_mode {
    // Step 15.1: .ll → .o
    let obj_path = out_dir.join(format!("{}.o", stem));
    // ... llc invocation ...

    // Step 15.2: .o → .a (static) or .so (shared)
    let lib_path = out_dir.join(format!("lib{}.a", stem));
    let mut ar = std::process::Command::new("ar");
    ar.args(["rcs"])
        .arg(&lib_path)
        .arg(&obj_path);
    let status = ar.status()
        .map_err(|e| format!("ar failed: {}", e))?;
    if !status.success() {
        return Err("ar returned non-zero exit status".into());
    }
    println!("  Library archive: {}", lib_path.display());

    // Optional: .o → .so
    // (gated behind `--shared` flag)
    // let mut cc = std::process::Command::new("cc");
    // cc.args(["-shared", "-o"])
    //     .arg(out_dir.join(format!("lib{}.so", stem)))
    //     .arg(&obj_path)
    //     .arg("-lm");

    return Ok(lib_path);
}
```

**Test artifact:** C driver links with `-L. -l<stem>` and succeeds.

**Tests:**
- Manual: `briev build --library test.bv --out /tmp/lib` → `.a` exists
- `test_library_mode_ar_fails`: If `ar` not on PATH, graceful error
- `test_library_mode_a_has_correct_symbols`: `nm libtest.a | grep add`
  contains the expected export symbol

---

## Step 15.3 — `__briev_init_state` and `__glue_release` in generated headers

**File:** `src/backend/bindgen.rs`

**What:** The C header (`briev_types.h`), Rust bindings (`briev_bindings.rs`),
and Python stubs (`briev_bindings.py`) currently emit function declarations
for exported definitions with a `struct BrievState* state` first parameter,
but they do not declare `__briev_init_state` or `__glue_release`. Add them.

**C header additions (`emit_c_header`):**
```rust
// After the #include guard, before struct definitions:
out.push_str("// ── Runtime State Handle ──\n");
out.push_str("#ifndef BRIEV_STATE_DEFINED\n");
out.push_str("#define BRIEV_STATE_DEFINED\n");
out.push_str("typedef struct BrievState BrievState;\n");
out.push_str("#endif\n\n");
out.push_str("// Initialize the Briev runtime state. Returns an opaque handle.\n");
out.push_str("// Must be called before any exported function.\n");
out.push_str("BrievState* __briev_init_state(void);\n\n");
out.push_str("// Release the Briev runtime state.\n");
out.push_str("void __glue_release(BrievState* state);\n\n");
```

**Rust additions (`emit_rust_bindings`):**
```rust
out.push_str("extern \"C\" {\n");
out.push_str("    pub fn __briev_init_state() -> *mut c_void;\n");
out.push_str("    pub fn __glue_release(state: *mut c_void);\n");
out.push_str("}\n");
```

**Python additions (`emit_python_stubs`):**
```python
_lib.__briev_init_state.argtypes = []
_lib.__briev_init_state.restype = ctypes.c_void_p
_lib.__glue_release.argtypes = [ctypes.c_void_p]
_lib.__glue_release.restype = None
```

**Also:** Change the first parameter from the current `struct BrievState* state`
to use the opaque typedef. The consumer should not need to know the struct
layout — just the opaque handle.

**Tests:**
- `test_bindgen_c_header_has_init_release`: Generated C header contains
  `__briev_init_state` and `__glue_release`
- `test_bindgen_rust_has_init_release`: Generated Rust bindings have both
- `test_bindgen_python_has_init_release`: Generated Python stubs have both
- `test_bindgen_c_typedef_opaque`: `BrievState` is declared as `typedef struct BrievState BrievState;` not a struct definition

---

## Step 15.4 — Type marshaling at the FFI boundary

**File:** `src/backend/llvm/emit_toplevel.rs` (export wrapper emission),
`src/backend/bindgen.rs` (type maps)

**What:** The LLVM export wrapper (the `define dso_local` shim that forwards
from the C-ABI entry point to the inner definition) must marshal types
between C calling convention and Briev's internal representation. Bindgen's
type maps must agree with the wrapper's emitted IR.

### Bool: `i1` ↔ `uint8_t`

Briev's internal `Bool` type is `i1` (1-bit integer in LLVM). The C ABI
passes `_Bool`/`uint8_t` as `i8`. The export wrapper must `zext`/`trunc`:

```llvm
; Before calling the inner function:
;   (param comes in as i8, trunc to i1)
%inner_val = trunc i8 %param to i1
; After returning from the inner function:
;   (result is i1, zext to i8 for C caller)
%result = zext i1 %inner_ret to i8
ret i8 %result
```

**In `emit_toplevel.rs`**, when emitting the export wrapper's parameter
list and return handling, check the type:

```rust
fn emit_export_wrapper(
    out: &mut String,
    defn: &Definition,
    export_name: &str,
    ctx: &CompilerContext,
) {
    // ... emit header ...
    for (i, (name, ty)) in defn.parameters.iter().enumerate() {
        match ty {
            Type::Custom(t) if t == "Bool" => {
                // C ABI passes as i8 — trunc to i1 for Briev
                let i1_reg = format!("%inner_{}", name);
                writeln!(out, "  {} = trunc i8 %{} to i1", i1_reg, name).ok();
                // Use i1_reg as the argument to the inner call
            }
            // ...
        }
    }
    // On return:
    if defn.output_type == Type::Custom("Bool".to_string()) {
        writeln!(out, "  %result_i8 = zext i1 %inner_ret to i8").ok();
        writeln!(out, "  ret i8 %result_i8").ok();
    }
}
```

### String: `{ptr, len}` struct ↔ `const char*`

Briev's internal `String` is a `%String` struct `{ i64, i64, i8 }` (ptr, len,
codec). C callers expect `const char*` (a null-terminated `i8*`). The export
wrapper must extract the `.ptr` field and pass/return it as `i8*`.

```llvm
; For a String parameter coming from C (i8*):
;   — This requires copying/constructing a %String struct
; For a String return going to C:
;   — Extract .ptr and return as i8*
```

**For exports:** Return type marshaling:
```llvm
; Briev function returned %String (i64 handle)
%str_ptr = inttoptr i64 %briev_ret to %String*
%c_str_ptr = getelementptr %String, %String* %str_ptr, i32 0, i32 0
%c_char_ptr = bitcast i64* %c_str_ptr to i8*
ret i8* %c_char_ptr
```

**For imports (`frgn`):** (Covered by existing GLUE + Phase 11 sad-path)

**Bindgen alignment:** Update `type_to_c` in `bindgen.rs`:
- `Bool` → `uint8_t` (already correct at line 183; verify wrapper emits
  `zext`/`trunc`)
- `String` → `const char*` for function declarations (currently emits
  `struct BrievString` at line 185 — change to `const char*` for the
  export API, keep `struct BrievString` for GLUE meld paths that need
  the full struct)

**Reject complex types at compile time:** If an `export defn` has `List`,
`Data`, or user-defined struct types in its signature, emit a compile error:
`"export of <type> is not yet supported. Only Int, Float, Bool, Char, and
String are supported in v1."` This matches the v1 scope.

**Tests:**
- `test_export_bool_marshal`: Compile `export defn f(b: Bool) -> Bool`,
  verify LLVM IR contains `trunc i8` and `zext i1`
- `test_export_string_marshal`: Compile `export defn f(s: String) -> String`,
  verify LLVM IR contains `getelementptr` and `bitcast` for `.ptr`
- `test_export_rejects_complex_types`: `export defn f(l: List<Int>)` →
  compile error
- `test_bindgen_string_is_const_char_ptr`: C header shows `const char*`
  not `struct BrievString`

---

## Step 15.5 — End-to-end C driver integration test

**New file:** `tests/library_mode/` directory

**What:** A C program that links a compiled Briev library and calls an
exported function, verifying correct results. Guarded by toolchain
availability (skip if `llc`/`ar`/`cc` not found, not `#[ignore]`).

**Test structure:**

```c
// tests/library_mode/c_driver.c
#include "briev_types.h"
#include <stdio.h>

int main() {
    BrievState* state = __briev_init_state();
    int64_t result = add(state, 2, 3);
    printf("add(2, 3) = %ld\n", result);
    __glue_release(state);

    if (result != 5) {
        fprintf(stderr, "Expected 5, got %ld\n", result);
        return 1;
    }
    return 0;
}
```

**Rust integration test:**

```rust
#[test]
fn test_library_mode_c_driver() {
    // Skip if llc/ar/cc not on PATH
    if !has_tool("llc") || !has_tool("ar") || !has_tool("cc") {
        eprintln!("skipping: llc, ar, or cc not found");
        return;
    }

    // 1. Compile the test .bv file in library mode
    let bv_source = r#"
        export defn add(a: Int, b: Int) -> Int {
            term a + b;
        };
    "#;
    let dir = tempdir();
    let bv_path = dir.path().join("test.bv");
    fs::write(&bv_path, bv_source).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_briev"))
        .args(["build", "--library"])
        .arg(&bv_path)
        .arg("--out")
        .arg(dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "briev build --library failed");

    // 2. Verify artifacts exist
    assert!(dir.path().join("libtest.a").exists());
    assert!(dir.path().join("briev_types.h").exists());

    // 3. Compile C driver
    // (writes c_driver.c, compiles with cc, links libtest.a, runs)
    // ... (omitted for brevity — see test source)

    // 4. Assert output
    // let output = std::process::Command::new("./c_driver")
    //     .output().unwrap();
    // assert_eq!(String::from_UTF8_lossy(&output.stdout).trim(), "add(2, 3) = 5");
}
```

**Tests:**
- `test_library_mode_c_driver`: Full pipeline: `.bv` → `.a` + header → C
  driver → correct result
- `test_library_mode_c_driver_string`: String roundtrip through FFI boundary
- `test_library_mode_c_driver_bool`: Bool roundtrip through FFI boundary

---

## Integration with Existing Plans

### Dependency on Derivation Plan (Phase 12)

Phase 15 can run in parallel with or independently of Phase 12's `.dbvl`
archive. The `--library` mode consumes definitions directly from the AST,
not from the archive. When the archive is available, `--library` could
alternatively read from `.dbvl` — that's a future optimization, not a
dependency.

### Dependency on GLUE v2

The `bridge-exports.dbvl` metadata from GLUE v2 and the `--library` output
are complementary: the foreign build system uses the `.dbvl` to generate
its bindings and the `.a`/`.so` to link. Phase 15 does not replace
`bridge-exports.dbvl` — it produces the linkable artifact that the GLUE
metadata describes.

### Relationship to #export → export migration

Step 15.0 adds the `export` keyword. The GLUE v2 plan's existing detection
of exported functions (via modifier scan) is updated to also recognize the
new `TopLevel::Export` variant during a transition window. After the
transition, `#export` support is removed.

---

## Golden Rules

1. **Flat control flow**: Max 2 levels deep. Use guard clauses.
2. **Tests or it doesn't exist**: Every new code path needs tests.
3. **Doc comments on every definition**: Every new struct, fn, trait needs `///`.
4. **Rationale comments**: Every change site gets a
   `// 2026-07-11: Phase 15.N — <description>` comment.
5. **No weakening existing paths**: The old `#export` pragma continues to
   work during the deprecation window.
6. **Additive only**: New match arms only. The `_ => return None;`
   fallthrough must remain unchanged.
7. **Continuous commits**: Commit after every logical step.
   `cargo test --lib` before every commit.

---

## Testing Strategy Summary

| Step | Focus | Test count delta |
|------|-------|-----------------|
| 15.0 | `export` keyword | ~+10 (lexer, parser, AST, deprecation warning) |
| 15.1 | `.ll` → `.o` via `llc` | ~+5 (obj file exists, llc error handling) |
| 15.2 | `.o` → `.a` via `ar` | ~+5 (archive exists, symbol verification) |
| 15.3 | init/release in bindgen | ~+8 (C, Rust, Python headers) |
| 15.4 | Type marshaling | ~+15 (Bool IR, String IR, reject complex, bindgen alignment) |
| 15.5 | C driver test | ~+5 (full pipeline, Bool roundtrip, String roundtrip) |

---

## Documentation Updates

| Doc | What |
|-----|------|
| `docs/architecture/features/export.md` | `export` keyword syntax, library mode CLI, type marshaling rules |
| `docs/architecture/glue-pipeline.md` | Add library-mode path alongside GLUE export path |
| `docs/architecture/features/bindgen.md` | Generated header API: `BrievState`, `__briev_init_state`, `__glue_release` |

---

## Risk Register

| Risk | Mitigation |
|------|------------|
| `llc`/`ar`/`cc` not on user's PATH | Graceful skip with clear error message; tests skip automatically |
| String marshaling produces wrong results for non-UTF8 or embedded nulls | v1 documents that strings are null-terminated C strings; full `{ptr, len}` support deferred to v2 |
| Bool as `uint8_t` contradicts existing C callers expecting `i1` | The LLVM ABI for `_Bool` is `i8`; `i1` is not a valid C ABI parameter type |
| `#export` deprecation breaks existing code | Compiler emits warning but still compiles; removal scheduled for next major version |
