# GLUE v2: FFI Unification Protocol

**Date:** 2026-07-10
**Status:** Plan — pre-implementation
**Driver:** End-to-end bidirectional FFI (Briv → foreign, foreign → Briv) with zero-copy data, no C compiler dependency, and cross-language LLVM LTO for LLVM-based targets.

---

## 1. Philosophy

The GLUE protocol (General Language Unification Engine) is Briv's strategy for
piecemeal language adoption ("strangler fig" pattern). A developer replaces one
function at a time with Briv, and the boundary is invisible to the host language.

Two integration paths exist, selected automatically by the compiler based on
target capability:

**Path A — LLVM Inlining (for Rust, C, C++, Swift, Zig):**
Briv emits LLVM IR with proper function definitions. GLUE merges Briv's IR
with the host language's IR before LLVM optimization. The result is a single
compilation unit — LLVM cross-language inlines, DCEs, constant-propagates,
and vectorizes across the former language boundary. The host linker sees only
native symbols. Briv's `#export` functions ARE native functions.

**Path B — Meld Projection (for Python, Node, JVM):**
Briv compiles to a shared library with C ABI exports. Data does NOT cross a
marshaling layer — meld declarations project Briv types onto the host
runtime's heap memory (PyObject*, v8::Value, jobject). Briv reads/writes
foreign memory through typed accessors derived from meld routes. The C ABI
is the call transport only; zero-copy comes from meld.

**Architectural invariants:**
- No C compiler in the build chain — `llc` compiles `.ll` → `.o` directly
- No `extern "C"` wrappers hand-written — GLUE generates or merges them
- No `bindgen`, `pyo3`, `neon`, or similar bindings generators
- No serialization, no marshaling, no data copying at the boundary for
  structured types

---

## 2. Architecture Changes from GLUE v1

### What stays

| Component | File(s) | Reason |
|-----------|---------|--------|
| `#export` pragma extraction | `src/glue/export.rs:extract_exports()` | Identifies boundary-crossing functions |
| `frgn` declaration extraction | `src/glue/export.rs:extract_frgns()` | Identifies imported foreign functions |
| `meld` declaration extraction | `src/glue/export.rs:extract_melds()` | Identifies type-layout compatibility proofs |
| DBVL parser | `src/glue/dbvl_reader.rs` | Briv's native data-interchange format |
| DBVS schema validator | `src/glue/dbvs_validator.rs` | Schema validation for DBVL files |
| `emit_file#()` intrinsic | `src/ast.rs` | General-purpose compile-time file output |

### What changes

| Component | File(s) | Change |
|-----------|---------|--------|
| Adapter registry | `lib/glue.dbvl` | New field layout: `language, types_module, ext, llvm_triple, c_type_map` |
| Registry schema | `lib/glue.dbvs` | Remove `macro_path` and `output_dir`, add `types_module` and `llvm_triple` |
| `AdapterEntry` struct | `src/glue/export.rs` | Remove `macro_path`, add `types_module` + `llvm_triple` |
| `find_adapter()` | `src/glue/export.rs` | Parse new field positions |
| `run_export()` | `src/glue/export.rs` | Strip `$!macro` adapter invocation. Write `bridge-exports.dbvl` instead |
| Export output | `bridge-exports.dbvl` | New format: tagged lines with `export`, `meld`, `ctype` discriminators |

### What is removed

| Component | File(s) | Rationale |
|-----------|---------|-----------|
| Rust adapter macro | `glue/adapters/rust.bv` | $!macro generated C ABI wrappers — not the target architecture |
| Python adapter macro | `glue/adapters/python.bv` | Same — replaced by direct ctypes generation from bridge-exports.dbvl |
| Node adapter macro | `glue/adapters/node.bv` | Same |
| Adapter macro invocation | `src/glue/export.rs` (~50 lines) | No more MacroContext/expand_macros() for adapter generation |
| Adapter directory | `glue/adapters/` | All three .bv files removed |
| `briv link` subcommand | `src/main.rs`, `src/glue/link.rs` | nm-based symbol analysis was a convenience, not core to the vision |

---

## 3. The Registry: `lib/glue.dbvl`

The adapter registry now records target-language metadata, not adapter macros.

### Schema (`lib/glue.dbvs`)

```
entry GlueTarget {
    language: String;       // Target language name (rust, python, etc.)
    types_module: String;   // Path to .bv declaring foreign type names for Briv's universe
    file_extension: String; // Native source extension without dot (rs, py)
    llvm_triple: String;    // LLVM target triple ("any" for non-LLVM targets)
    c_type_map: Optional<Map<String, String; pair_separator=:, value_delimiter=space, brace=required>>;
};
```

### Example entry

```
rust, glue/rust/types.bv, rs, x86_64-unknown-linux-gnu,
    {Int:int64_t Float:double Bool:bool Char:char String:cstring}
```

### Fields

- **language**: Lookup key for `briv export bridge.bv <language>`
- **types_module**: Briv source file declaring foreign type names. A bridge
  file imports this module and uses `meld` to connect Briv types to foreign types.
  Example content: `type i64; type f64; type cstring;`
- **file_extension**: Used for generated stub files if any
- **llvm_triple**: Target triple for LLVM compilation. `"any"` means this language
  does not use LLVM; interop goes through C ABI + meld projections
- **c_type_map**: Maps each Briv type name to its C ABI representation. This is
  the canonical mapping for the FFI boundary. LLVM targets derive LLVM types from
  C types (int64_t → i64). Non-LLVM targets use C types directly for the FFI boundary.
  Complex types (String, Data, List) use C struct representations defined in the
  types_module file.

### Why C types in the registry and not in meld declarations

Meld proves layout compatibility between two types in Briv's universe. The C ABI
type mapping is a separate concern: when crossing the FFI boundary, what C type
does the function signature use? This is recorded in glue.dbvl because:
1. It is the same for most targets (int64_t for Int, double for Float)
2. Foreign type names differ (Rust `i64` vs Python `ctypes.c_int64`) but the C
   ABI type is the same
3. It provides a single source of truth for the boundary contract

---

## 4. Foreign Type Modules (`glue/<lang>/types.bv`)

Each target language has a `.bv` file declaring its primitive types as names in
Briv's type universe. These are minimal — just enough for `meld` to reference
the foreign side of a compatibility proof.

```
// glue/rust/types.bv
type i64;
type f64;
type cstring;
type bool;
type char;

// glue/python/types.bv
type int64_t;
type double;
type cstring;
type bool;
type char;
```

These files are NOT adapter macros. They are pure type declarations. A bridge
file imports them and then melds:

```
import { i64 } from "glue/rust/types.bv";

meld Int <:> i64 {
    Ptr -> Ptr,
    Size -> 8,
    Bytes -> 8
};

#export add(a: Int, b: Int) -> Int { term a + b; };
```

---

## 5. Export Output: `bridge-exports.dbvl`

`briv export bridge.bv rust --out ./out` produces:

```
out/bridge.ll              (LLVM IR module — compiled bridge)
out/bridge-exports.dbvl    (metadata — exports + melds + ctype mappings)
```

### Output format

```
// bridge-exports.dbvl — auto-generated by briv export
// No schema validation needed — the consumer (build.rs, Python script)
// splits by "\n" then by "," and dispatches on the discriminator field.

// Exports: export, name, param_types_pipe_separated, return_type
export, add, Int|Int, Int
export, greet, String, String

// Melds: meld, briv_type, foreign_type, routes_semicolon_separated
meld, Int, i64, Ptr->Ptr;Size->8;Bytes->8
meld, Float, f64, Ptr->Ptr;Size->8;Bytes->8

// C type mappings: ctype, briv_type, c_type
// (injected from glue.dbvl's c_type_map for the target language)
ctype, Int, int64_t
ctype, Float, double
ctype, Bool, bool
ctype, Char, char
ctype, String, cstring
```

### Consumer: Rust build.rs

```rust
// build.rs — generated or template-based
fn main() {
    // 1. Compile bridge.ll to bridge.o
    let status = std::process::Command::new("llc")
        .args(["bridge.ll", "-o", "bridge.o", "-filetype=obj"])
        .status().expect("llc failed");

    // 2. Read bridge-exports.dbvl
    let dbvl = std::fs::read_to_string("bridge-exports.dbvl")
        .expect("bridge-exports.dbvl not found");

    // 3. Parse exports + ctype mappings
    let mut ctypes: HashMap<String, String> = HashMap::new();
    for line in dbvl.lines() {
        let parts: Vec<&str> = line.splitn(3, ',').collect();
        match parts[0] {
            "ctype" => { ctypes.insert(parts[1].into(), parts[2].into()); }
            _ => {}
        }
    }

    // 4. Generate extern "C" block
    let mut bindings = String::from("extern \"C\" {\n");
    for line in dbvl.lines() {
        let parts: Vec<&str> = line.splitn(4, ',').collect();
        if parts[0] == "export" {
            let name = parts[1];
            let param_str = parts[2];
            let ret_type = ctypes.get(parts[3]).unwrap_or(&"()".into());
            // Map param types to C types...
            let params: Vec<String> = param_str.split('|')
                .map(|t| ctypes.get(t).unwrap_or(&"()".into()).clone())
                .collect();
            bindings.push_str(&format!("    fn {}({}) -> {};\n",
                name, params.join(", "), ret_type));
        }
    }
    bindings.push_str("}");

    // 5. Write generated bindings
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{}/bridge.rs", out_dir), &bindings).unwrap();

    // 6. Link bridge.o
    println!("cargo:rustc-link-lib=static=bridge");
}
```

### Consumer: Python

```python
# bridge.py — auto-generated from bridge-exports.dbvl
import ctypes, os

_lib = ctypes.CDLL(os.path.join(os.path.dirname(__file__), 'bridge.so'))

# From ctype entries:
# Int -> int64_t -> ctypes.c_int64
# Float -> double -> ctypes.c_double
# String -> cstring -> ctypes.c_char_p

# From export entries + ctype substitutions:
_lib.add.argtypes = [ctypes.c_int64, ctypes.c_int64]
_lib.add.restype = ctypes.c_int64
```

---

## 6. Implementation Plan

All steps are additive or replacement. No existing optimization path is modified.

### Step 1: Create foreign type modules

**Files:**
- `glue/rust/types.bv` — NEW
- `glue/python/types.bv` — NEW

**Content:** Minimal type declarations (`type i64; type f64; type cstring; type bool; type char;`) for each target. These register foreign type names in Briv's type universe so `meld` can reference them.

### Step 2: Update registry files

**File: `lib/glue.dbvl`** — replace old format with new:
```
rust, glue/rust/types.bv, rs, x86_64-unknown-linux-gnu, {Int:int64_t Float:double Bool:bool Char:char String:cstring}
python, glue/python/types.bv, py, any, {Int:int64_t Float:double Bool:bool Char:char String:cstring}
```

**File: `lib/glue.dbvs`** — replace schema entry:
```
entry GlueTarget {
    language: String;
    types_module: String;
    file_extension: String;
    llvm_triple: String;
    c_type_map: Optional<Map<String, String; pair_separator=:, value_delimiter=space, brace=required>>;
};
```

### Step 3: Update `AdapterEntry` struct and `find_adapter()`

**File: `src/glue/export.rs`**

Changes to `AdapterEntry`:
- Remove: `macro_path: String`
- Remove: (implicit `output_dir` — was in registry but never used as field)
- Add: `types_module: String`
- Add: `llvm_triple: String`
- Keep: `language: String`, `file_extension: String`, `type_map: HashMap<String, String>`
  (rename `type_map` to `c_type_map` for clarity)

Field reordering in `find_adapter()`:
- Old positions: language(0), macro_path(1), file_extension(2), output_dir(3), type_map(4)
- New positions: language(0), types_module(1), file_extension(2), llvm_triple(3), c_type_map(4)

### Step 4: Strip `$!macro` adapter invocation

**File: `src/glue/export.rs`**

Remove the following from `run_export()`:
- `MacroContext` import and creation
- `collect_macro_defs()` call
- Synthetic `Expr::MacroCall` construction
- `expand_macros()` call
- `std::env::set_var("BRIV_OUTPUT_DIR", ...)` — no longer needed
- All imports related to features::macros

Replace with:
- Serialize bridge info (exports + melds) as tagged DBVL lines
- Inject c_type_map from adapter entry into the output
- Write `bridge-exports.dbvl` to the output directory

### Step 5: Write bridge-exports.dbvl output

**File: `src/glue/export.rs`**

New function `write_export_dbvl()`:
```
Input: info (BridgeInfo), c_type_map (HashMap)
Output: String (DBVL content)

Lines written:
  export, <name>, <params|pipe|separated>, <return_type>   // for each export
  meld, <from_type>, <to_type>, <routes>                    // for each meld
  ctype, <briv_type>, <c_type>                             // for each entry in c_type_map
```

### Step 6: Remove old adapter files

Delete:
- `glue/adapters/rust.bv`
- `glue/adapters/python.bv`
- `glue/adapters/node.bv`
- `glue/adapters/` directory (if empty after deletion)

### Step 7: Update tests

Update existing GLUE export tests to match new output format:
- `test_serialize_exports_dbvl_empty` → expect tagged format
- `test_serialize_exports_dbvl_single` → expect tagged format
- etc.

Add new tests:
- `test_write_export_dbvl_ctype_mapping` — ctype lines emitted correctly
- `test_write_export_dbvl_full_pipeline` — exports + melds + ctypes in correct order
- `test_find_adapter_new_format` — registry parsing works with new field positions

---

## 7. Flat Control Flow Enforcement

This plan strictly follows the project's max-2-levels nesting rule:

**Flat guard clauses:**
```rust
// ✅ Correct: flat with early returns
fn find_adapter(language: &str, dbvl_path: &Path) -> Result<AdapterEntry, String> {
    let source = fs::read_to_string(dbvl_path)
        .map_err(|e| format!("Failed to read {}: {}", dbvl_path.display(), e))?;
    let file = parse_dbvl(&source);
    for entry in &file.entries {
        let fields = match entry {
            DbvlEntry::Raw(tokens) => tokens,
            DbvlEntry::Validated { fields, .. } => fields,
        };
        if fields.len() < 5 {
            continue;
        }
        if fields[0] != language {
            continue;
        }
        let c_type_map = if fields.len() > 4 {
            parse_type_map(&fields[4])
        } else {
            HashMap::new()
        };
        return Ok(AdapterEntry {
            language: language.to_string(),
            types_module: fields[1].clone(),
            file_extension: fields[2].clone(),
            llvm_triple: fields[3].clone(),
            c_type_map,
        });
    }
    Err(format!("Adapter not found for language '{}'", language))
}
```

**No arrowhead code:**
```rust
// ✅ Correct: extracted helper + flat control flow
fn write_export_line(out: &mut String, kind: &str, fields: &[&str]) {
    let line = fields.join(",");
    out.push_str(kind);
    out.push(',');
    out.push_str(&line);
    out.push('\n');
}
```

**Forbidden patterns:**
```rust
// ❌ Wrong: nesting > 2
if let Ok(source) = fs::read_to_string(path) {
    if let Ok(file) = parse_dbvl(&source) {
        for entry in &file.entries {
            // Level 3
        }
    }
}

// ✅ Correct: flat with ?
let source = fs::read_to_string(path)?;
let file = parse_dbvl(&source);
```

---

## 8. Documentation

### Rationale comments added at each change site

**`src/glue/export.rs`** — every function that writes DBVL output:
```
// 2026-07-10: GLUE v2 output format — replaces the $!macro adapter
// invocation. Instead of running an adapter .bv macro to generate
// C ABI wrappers, we write a bridge-exports.dbvl metadata file
// that the foreign build system consumes directly. The $!macro
// approach generated Cargo.toml/build.rs/src/ that wrapped Briv
// behind C ABI; the new approach ships the LLVM IR + type metadata
// and lets the host build.rs generate bindings from the .dbvl.
// This supports both LLVM LTO (Rust) and C ABI + meld (Python).
```

**`lib/glue.dbvl`** — header comment:
```
// 2026-07-10: GLUE v2 registry format. Removed macro_path and
// output_dir; added types_module (path to foreign type declarations)
// and llvm_triple (target triple, "any" for non-LLVM targets).
// c_type_map now maps Briv types to C ABI types, not adapter-
// specific type names. The old adapter $!macro approach generated
// C ABI wrapper crates; the new approach uses LLVM IR merging
// (Path A) or meld projections (Path B) instead.
```

**`lib/glue.dbvs`** — schema comment updated to reflect new entry type.

### Architecture docs to update

- `docs/architecture/glue-pipeline.md` — update pipeline diagram to show
  `bridge.ll` + `bridge-exports.dbvl` output, remove adapter macro step
- `docs/architecture/features/backend-dispatch.md` — add note about GLUE
  selecting Path A vs Path B based on target capability

### Doc comments on new/modified definitions

- `AdapterEntry` struct: doc comment updated to explain each field's role
- `find_adapter()`: updated for new field positions
- `run_export()`: updated to describe new output format
- New `write_export_dbvl()`: full doc comment with output format spec

---

## 9. Test Plan

### Existing tests that must pass (unchanged)
- `src/glue/dbvl_reader.rs` — 7 tests (DBVL parsing)
- `src/glue/dbvs_validator.rs` — 7 tests (schema validation)
- `src/glue/export.rs` — 6 tests (serialization) — **will update**
- `src/glue/link.rs` — 3 tests (link pipeline) — **untouched, kept functional**
- All 1444 library tests

### Updated tests
- `test_serialize_exports_dbvl_empty` — now expects tagged output
- `test_serialize_exports_dbvl_single` — tagged format
- `test_serialize_exports_dbvl_multiple_params` — tagged format
- `test_serialize_exports_dbvl_multiple_separate` — tagged format
- `test_serialize_frgns_dbvl_with_intrinsic` — tagged format
- `test_serialize_melds_dbvl` — tagged format
- `test_parse_type_map` — unchanged (type map parsing same semantics)
- `test_find_adapter_new_format` — new test for new field layout

### New tests
- `test_serialize_ctype_dbvl` — ctype lines emitted as `ctype,<briv>,<c>`
- `test_write_export_dbvl_full` — end-to-end: exports + melds + ctypes interleaved
- `test_find_adapter_old_format_rejected` — old-format glue.dbvl raises error

---

## 10. File Manifest

### New files
| File | Lines | Purpose |
|------|-------|---------|
| `glue/rust/types.bv` | ~10 | Rust foreign type declarations for type universe |
| `glue/python/types.bv` | ~10 | Python foreign type declarations for type universe |
| `docs/plans/2026-07-10-glue-v2-ffi-unification.md` | ~350 | This plan document |

### Modified files
| File | Lines changed | Change |
|------|---------------|--------|
| `lib/glue.dbvl` | ~5 | New field layout |
| `lib/glue.dbvs` | ~8 | New entry schema |
| `src/glue/export.rs` | ~100 | Strip adapter macro, write bridge-exports.dbvl |

### Deleted files
| File | Rationale |
|------|-----------|
| `glue/adapters/rust.bv` | Replaced by build.rs reading bridge-exports.dbvl |
| `glue/adapters/python.bv` | Replaced by direct ctypes generation |
| `glue/adapters/node.bv` | Replaced (will re-add when Node target is active) |
| `glue/adapters/` | Empty after deletion |

---

## 11. Future Work (Covered by Phase 15)

### Phase 15 — Library Mode Completion

**Plan**: `docs/plans/2026-07-11-library-mode-completion.md`
**Proposal by**: [@revred](https://github.com/revred) — reviewed the
`--library` infrastructure and identified five gaps to a consumable
C-callable library.

The `--library` mode, `.ll` → `.o` → `.a` packaging, bindgen completeness
(`__briv_init_state`/`__glue_release`), type marshaling (`Bool`/`String`),
and the end-to-end C driver test are now specified as Phase 15 of the
overall roadmap, building on top of both this plan's `bridge-exports.dbvl`
output and the derivation plan's `.dbvl` archive format.

### Other Out-of-Scope Items

- Python meld layouts for PyLongObject, PyUnicodeObject, PyListObject — requires
  C struct definitions + meld routes, covered in a future plan.
- Rust dogfooding test — linking a Briv bridge into the Briv compiler's own
  build. Requires the Phase 15 library artifact as a prerequisite.
