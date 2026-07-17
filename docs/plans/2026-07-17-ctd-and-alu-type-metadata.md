# CTD + ALU Type Metadata System

**Date:** 2026-07-17
**Status:** Plan — not yet implemented
**Drivers:** Benchmark regression at HEAD (all optimizer benchmarks produce `__FAIL__`)

## Problem

The `primitive` metadata field on `ResolvedType` conflates several concerns and is
inconsistently populated:

1. **Primordial sets wrong `primitive` values** — `"signed"`, `"unsigned"`,
   `"float"`, `"pointer"`, `"struct"`, `"void"`. These are backend-agnostic-ish but
   don't match the config file keys (`"Int"`, `"Float"`, etc.) or the backend
   normalizers' expectations.

2. **Primordial also sets `llvm_type`** — this is backend-specific metadata
   that doesn't belong in the frontend universe. For String, `llvm_type = "%String"`
   is the LLVM named struct type name, but the actual LLVM ABI type needed
   is `"ptr"` (heap-allocated struct passed by pointer).

3. **`derive_llvm_type()` can't produce `"ptr"` for String/Data** — it receives
   `primitive = "struct"` + `bytes = 24` and falls through to `"i{N*8}"` = `"i192"`.
   There's no way to say "this struct is heap-allocated, use `ptr` at the ABI level."

4. **LLVM normalizer defers to primordial** — it skips types that already have
   `llvm_type` (`if rt.properties.contains_key("llvm_type") { continue; }`),
   locking in the primordial's wrong values.

5. **Backend ignores normalizer** — `rt_llvm_type()` calls `derive_llvm_type()`
   instead of reading the `llvm_type` property set by the normalizer.

### Symptom

Every `defn` function with a `String` parameter emits:
```llvm
define i64 @get_env_int(ptr %state, i192 %arg0) { ... }
  %ac0 = ptrtoint ptr %arg0 to i64        ; ERROR: %arg0 is i192, not ptr!
```

`llc`/`clang` rejects the IR. All benchmarks that transitively call `get_env_int`
fail with `__FAIL__`.

## Architecture

### Three-Layer Metadata System

| Layer | Owns | Convention | Example |
|-------|------|------------|---------|
| **Primordial** (frontend) | `ctd`, `alu`, `field.*`, `bytes`, `alignment` | Backend-agnostic type semantics | `ctd = String`, `alu = Int` |
| **Normalizer** (per-backend) | Backend-specific type strings from CTD + bytes | Maps CTD → concrete type | `String` → LLVM `"ptr"` |
| **Backend** | Consumes the normalizer's annotation | Reads `llvm_type` property | `"ptr"` → used in function signatures |

### CTD (Common Type Definition)

An exhaustive, closed set of PascalCase identifiers describing what the type
*is* semantically, independent of any backend:

| CTD | Meaning | LLVM type | JS type | SPIR-V ALU |
|-----|---------|-----------|---------|------------|
| `Int` | Signed integer (size by bytes) | `i8/i16/i32/i64` | `"number"` | `Int` |
| `UInt` | Unsigned integer | `i8/i16/i32/i64` | `"number"` | `Int` |
| `Float` | 32-bit float | `"float"` | `"number"` | `Float` |
| `Double` | 64-bit float | `"double"` | `"number"` | `Float` |
| `Bool` | Boolean | `"i8"` | `"boolean"` | `Bool` |
| `Char` | Unicode codepoint | `"i32"` | `"number"` | `Int` |
| `String` | Heap-allocated string | `"ptr"` | `"string"` | `Int` |
| `Data` | Heap-allocated bytes | `"ptr"` | `"Uint8Array"` | `Int` |
| `Ptr` | Opaque pointer | `"ptr"` | `"number"` | `Ptr` |
| `Void` | No value | `"void"` | `"null"` | `Int` |

This list is EXHAUSTIVE. User-defined types may inherit a CTD from their base type
or set one explicitly via the `ctd ~> Name;` syntax.

### ALU (Arithmetic Logic Unit)

Describes what hardware unit computes with this value. Also PascalCase for
compiler-known ALUs, or lowercase-quoted for backend-specific hardware:

| ALU | Meaning | Used by |
|-----|---------|---------|
| `Int` | Integer ALU (add, sub, mul, etc.) | All integer/char/string types |
| `Float` | Floating-point ALU | Float, Double |
| `Bool` | Boolean/logical ALU | Bool, conditions |
| `"my_custom_dsp"` | Backend-specific | Only the named backend |

### Naming Convention

| Syntax | Meaning | Who reads it |
|--------|---------|-------------|
| `ctd = String` (PascalCase Identifier) | Built-in frontend-known type | All backends |
| `alu = Float` (PascalCase Identifier) | Built-in frontend-known ALU | All backends |
| `alu = "my_dsp"` (lowercase quoted String) | Opaque, backend/plugin-only | Specific backend or plugin |

### Validation Rules (enforced by LLVM normalizer)

The normalizer validates that CTD and ALU are compatible:

| CTD | Compatible ALUs | Incompatible ALUs |
|-----|----------------|-------------------|
| `Int`, `UInt`, `Char` | `Int` | `Float`, `Bool` |
| `Float`, `Double` | `Float` | `Int`, `Bool` |
| `Bool` | `Bool` | `Int`, `Float` |
| `String`, `Data` | `Int` | `Float`, `Bool` |
| `Ptr` | `Int` | `Float`, `Bool` |
| `Void` | `Int` | `Float`, `Bool` |

Quoted ALUs (lowercase) bypass validation — the backend handles those.

## Implementation Plan

### Baseline

Reference numbers from commit `8a827db` (2026-07-11, Phase 3 complete):

| Benchmark | Brief | C | Ratio | Winner | Correct |
|-----------|-------|---|-------|--------|---------|
| ring_buffer | 0.0686s | 0.0676s | 1.01x | C | MATCH |
| float_math | 0.0631s | 0.0771s | 0.81x | Brief | MATCH |
| float_math_nonzero | 0.1920s | 0.1727s | 1.11x | C | MATCH |
| sparse_dispatch | 0.0060s | 0.0657s | 0.09x | Brief | MATCH |
| print_loop | 0.0639s | 0.0670s | 0.95x | Brief | MATCH |
| nbody_newton | 7.4132s | 9.8522s | 0.75x | Brief | MATCH |
| nbody_sqrt | 3.0046s | 3.5218s | 0.85x | Brief | MATCH |
| nbody_sqrt_idio | 2.9578s | 4.3184s | 0.68x | Brief | MATCH |
| fasta | 0.2695s | 0.2636s | 1.02x | C | MATCH |
| fannkuch_redux | 0.0763s | 0.0789s | 0.96x | Brief | MATCH |
| mandelbrot | 0.7514s | 0.7538s | 0.99x | Brief | MATCH |
| kalman_filter_runtime | 0.1876s | 0.1887s | 0.99x | Brief | MATCH |
| knucleotide | 0.2093s | 0.2060s | 1.01x | C | MATCH |
| cancel_math | 0.0682s | 0.0672s | 1.01x | C | MATCH |
| bit_clear | 0.0010s | 0.0009s | 1.11x | C | MATCH |
| queue_drain | 0.0007s | 0.0632s | 0.01x | Brief | MATCH |
| queue_drain_sym | 0.0639s | 0.0672s | 0.95x | Brief | MATCH |
| queue_drain_idio | precomputed | — | — | — | SKIP |
| interval_step | 0.0009s | 0.0669s | 0.01x | Brief | MATCH |

Current HEAD: all optimizer benchmarks `__FAIL__`, runtime benchmarks fail to
compile. Target: restore to the Phase 3 baseline above.

### Steps (one commit each)

---

#### Commit 1: Replace `primitive` + `llvm_type` with `ctd` + `alu` in primordial

**File: `src/type_universe/mod.rs`**

Change the PRIMORDIALS table from 5 columns `(name, bytes, alignment, primitive, llvm_type)` to 4 columns `(name, bytes, alignment, ctd)` and add a separate ALU table:

```rust
// 2026-07-17: CTD replaces primitive+llvm_type. Each type gets a Common Type
// Definition (what it is) and an ALU (what hardware computes with it).
// These are backend-agnostic — the normalizer maps CTD to backend-specific types.
const PRIMORDIALS: &[(&str, u64, u64, &str)] = &[
    ("Int",    8, 8, "Int"),
    ("UInt",   8, 8, "UInt"),
    ("Int8",   1, 1, "Int"),
    ("UInt8",  1, 1, "UInt"),
    ("Int16",  2, 2, "Int"),
    ("UInt16", 2, 2, "UInt"),
    ("Int32",  4, 4, "Int"),
    ("UInt32", 4, 4, "UInt"),
    ("Int64",  8, 8, "Int"),
    ("UInt64", 8, 8, "UInt"),
    ("Float",  4, 4, "Float"),
    ("Float32",4, 4, "Float"),
    ("Float64",8, 8, "Double"),
    ("Double", 8, 8, "Double"),
    ("Bool",   1, 1, "Bool"),
    ("Char",   4, 4, "Char"),
    ("Data",   8, 8, "Data"),
    ("Void",   0, 0, "Void"),
];
```

Each type gets an `alu` from a parallel lookup:
```rust
/// Map a CTD to its default ALU (the ALU type that computes with it).
fn default_alu(ctd: &str) -> &'static str {
    match ctd {
        "Float" | "Double" => "Float",
        "Bool" => "Bool",
        _ => "Int",  // Int, UInt, Char, String, Data, Ptr, Void all use Int ALU
    }
}
```

In the insertion loop:
```rust
for &(name, bytes, alignment, ctd) in PRIMORDIALS {
    let mut properties = HashMap::new();
    properties.insert("ctd".into(), PropertyValue::Identifier(ctd.to_string()));
    properties.insert("alu".into(), PropertyValue::Identifier(default_alu(ctd).to_string()));
    properties.insert("alignment".into(), PropertyValue::Int(alignment as i64));
    self.types.insert(name.to_string(), ResolvedType {
        name: name.to_string(),
        base: "Bits".to_string(),
        bytes,
        alignment,
        properties,
    });
}
```

For String (the special-case block at lines 101–120):
```rust
// String — special case: heap-allocated struct with ptr+len+codec fields
// CTD = String tells the normalizer to map to "ptr" at ABI boundaries.
{
    let mut p = HashMap::new();
    p.insert("ctd".into(), PropertyValue::Identifier("String".to_string()));
    p.insert("alu".into(), PropertyValue::Identifier("Int".to_string()));
    p.insert("alignment".into(), PropertyValue::Int(8));
    p.insert("field.ptr.offset".into(), PropertyValue::Int(0));
    p.insert("field.ptr.width".into(), PropertyValue::Int(64));
    p.insert("field.len.offset".into(), PropertyValue::Int(64));
    p.insert("field.len.width".into(), PropertyValue::Int(64));
    p.insert("field.codec.offset".into(), PropertyValue::Int(128));
    p.insert("field.codec.width".into(), PropertyValue::Int(8));
    self.types.insert("String".to_string(), ResolvedType {
        name: "String".to_string(),
        base: "Bits".to_string(),
        bytes: 24,
        alignment: 8,
        properties: p,
    });
}
```

Remove the `primitive()` accessor stub — keep it for now with a deprecation
comment pointing to CTD, since other code may reference it. Actually, remove it
entirely: update all callers to read `ctd` or `alu` from properties directly.

**Impacted callers of `rt.primitive()`:**

| Location | Replace with |
|----------|-------------|
| `normalizer.rs:24` | Read `ctd` property |
| `normalizer.rs:258` | Read `ctd` property |
| `emit_toplevel.rs:15` (rt_llvm_type) | Read `llvm_type` property → fallback to `derive_llvm_type(ctd, bytes)` |
| `helpers.rs:30` (rt_llvm_type) | Same as above |
| `helpers.rs:636` (operator_llvm_type) | Read `alu` property |
| `helpers.rs:1067` (is_native_float) | Read `alu` property == `"Float"` |
| `helpers.rs:1207` (custom operator call) | Read `alu` property |
| `intrinsics.rs:97` (resolve_arg_primitive) | Read `ctd` property, rename to `resolve_arg_ctd` |
| `spirv/normalizer.rs:21` | Read `alu` property → keep `derive_alu_type` as fallback |
| `webstack_normalizer.rs:15` | Read `ctd` property |

**Doc comment updates:**
- Update the module-level doc on `seed_primordial_types` (lines 63–65) to describe
  CTD + ALU instead of primitive + llvm_type
- Add `///` doc to `default_alu()` explaining the CTD→ALU mapping
- Add rationale comment at the PRIMORDIALS table explaining why CTD replaces
  primitive + llvm_type

**Tests:**
- All existing tests must pass unchanged (this is a metadata rename — no behavior
  change if all callers are updated correctly)
- If any test constructs `ResolvedType` with `primitive` property, update it to `ctd`

---

#### Commit 2: Rename config file and update references

**File: `config/llvm-primitives.toml` → `config/ctd-llvm-mappings.toml`**

Rename file. Change section headers from `[primitive.*]` to `[ctd.*]`:

```toml
# CTD → LLVM Type Mappings
# Maps (ctd, bytes) → LLVM type string.
# Read at compile time — no Rust hardcoded tables.

[ctd.Int]
1 = "i8"
2 = "i16"
4 = "i32"
8 = "i64"

[ctd.UInt]
1 = "i8"
2 = "i16"
4 = "i32"
8 = "i64"

[ctd.Float]
2 = "half"
4 = "float"
8 = "double"

[ctd.Bool]
1 = "i8"

[ctd.Char]
4 = "i32"

[ctd.String]
8 = "ptr"

[ctd.Data]
8 = "ptr"
```

**Files referencing the old filename:**

| File | Change |
|------|--------|
| `src/config.rs:20` | `Path::new("config/llvm-primitives.toml")` → `"config/ctd-llvm-mappings.toml"` |
| `src/backend/llvm/normalizer.rs:16` | `TypeConfig::load()` → reads from new file path (handled by config.rs change) |
| `src/backend/spirv/normalizer.rs:17` | Same — handled by config.rs change |
| `src/backend/llvm/intrinsics.rs:10` | Comment reference only — update if present |

The `TypeConfig` struct itself doesn't need renaming — it's a generic "look up
type strings from a config file" mechanism. Just the file name and TOML keys change.

**`src/config.rs`:**

Update `derive_llvm_type()` — change signature to accept CTD instead of primitive:

```rust
/// Derive LLVM type string from CTD + bytes using the config file.
/// Called by the normalizer and as a fallback in the backend.
/// CTD is a PascalCase identifier (e.g., "Int", "Float", "String").
/// For CTDs with entries in ctd-llvm-mappings.toml, the config file wins.
/// For CTDs without config entries, falls back to i{N*8}.
pub fn derive_llvm_type(ctd: Option<&str>, bytes: u64, config: &TypeConfig) -> String {
    if let Some(entry) = config.lookup(ctd.unwrap_or("Int"), bytes) {
        entry.to_string()
    } else {
        format!("i{}", bytes * 8)
    }
}
```

Remove the hardcoded match arms (`Some("Int") if bytes == 8`, `Some("Float")`, etc.)
— those are now handled by the normalizer's `ctd_to_llvm()` and the config file.

Update `derive_alu_type()` similarly — or keep it as a fallback for the SPIR-V
normalizer to read the `alu` property instead. Actually, `derive_alu_type` should
be updated to match: accept CTD, derive from config. But the SPIR-V normalizer
should also read the `alu` property directly.

**`src/backend/llvm/intrinsics.rs`:**

The `OpConfig` file reference (`config/llvm-primitives.toml` in comment or import)
must be updated if present. The import `use crate::config::{OpConfig, derive_llvm_type, TypeConfig};`
at line 10 is fine — the function name `derive_llvm_type` doesn't change.

---

#### Commit 3: Fix LLVM normalizer — always compute `llvm_type` from CTD

**File: `src/backend/llvm/normalizer.rs`**

This is the core fix. The normalizer becomes the single authority for `llvm_type`.

**Remove the `continue` guard** (line 21–23).

**Add `ctd_to_llvm()`** — maps a PascalCase CTD to its LLVM type string:

```rust
// 2026-07-17: Map a frontend-known CTD to its LLVM type string.
// CTDs are PascalCase identifiers from the exhaustive primordial list.
// Unknown CTDs return None — the normalizer falls back to derive_llvm_type.
fn ctd_to_llvm(ctd: &str, bytes: u64) -> Option<&'static str> {
    match ctd {
        "Int" | "UInt" => match bytes {
            1 => Some("i8"), 2 => Some("i16"), 4 => Some("i32"), 8 => Some("i64"),
            _ => None,  // unusual byte size → let config file handle it
        },
        "Float" => Some("float"),
        "Double" => Some("double"),
        "Bool" => Some("i8"),   // storage type; backend uses i1 for registers
        "Char" => Some("i32"),
        "String" | "Data" | "Ptr" => Some("ptr"),
        "Void" => Some("void"),
        _ => None,  // user-defined CTD → fall through to derive_llvm_type
    }
}
```

**Add ALU × CTD validation:**

```rust
// 2026-07-17: Validate that a PascalCase ALU is compatible with the type's CTD.
// Quoted ALUs (lowercase strings) bypass validation — the backend handles those.
// Returns Ok(()) if compatible, Err(description) if not.
fn validate_alu_ctd(alu: &str, ctd: &str) -> Result<(), String> {
    match (alu, ctd) {
        ("Float", "Int" | "UInt" | "Bool" | "Char" | "String" | "Data" | "Ptr" | "Void") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': float hardware cannot process {} values", alu, ctd, ctd)),
        ("Bool", "Float" | "Double") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': boolean logic cannot process float values", alu, ctd)),
        ("Bool", "Int" | "UInt" | "Char" | "String" | "Data" | "Ptr" | "Void") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': boolean logic cannot process integer-like types (use ALU Int)", alu, ctd)),
        ("Int", "Float" | "Double") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': integer hardware cannot process float values (use ALU Float)", alu, ctd)),
        ("Int", "Bool") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': integer hardware cannot process boolean values (use ALU Bool)", alu, ctd)),
        _ => Ok(()),  // PascalCase known-valid combos + quoted ALUs pass
    }
}
```

**Main loop becomes:**

```rust
for rt in universe.types.values_mut() {
    // Read CTD and ALU from primordial properties
    let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.as_str()),
        _ => None,
    });
    let alu = rt.properties.get("alu").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.as_str()),
        _ => None,
    });

    // Validate ALU × CTD for built-in PascalCase identifiers
    // Quoted ALUs (lowercase strings) are backend-specific and skip validation.
    // 2026-07-17: Only validate at the normalizer level. The SPIR-V normalizer
    // reuses this validation — CIRCT ignores CTD entirely.
    if let (Some(a), Some(c)) = (alu, ctd) {
        validate_alu_ctd(a, c)?;
    }

    // Compute llvm_type from CTD
    let llvm_ty = ctd.and_then(|c| ctd_to_llvm(c, rt.bytes))
        .unwrap_or_else(|| derive_llvm_type(None, rt.bytes, &prim_config));
    rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty.to_string()));

    // Parse layout pattern and attach field annotations (unchanged)
    if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
        let cleaned = layout_str.strip_prefix('<').unwrap_or(layout_str);
        if let Ok(pat) = crate::bvir::layout::parse_layout_pattern(cleaned) {
            attach_layout_fields(rt, &pat);
        }
    }
}
```

**Update the metadata strip list** (line 80):

```rust
let keep: HashSet<String> = ["ctd", "alu", "llvm_type", "encoding", "layout"]
    .iter().map(|s| s.to_string()).collect();
```

**In `register_typedefs()`** (line 257–261):

Remove the `primitive` call. For TypeDef types, use the same CTD-based logic.
If the TypeDef has a base type with a CTD, inherit it. Otherwise fall through:

```rust
// 2026-07-17: Use CTD from the TypeDef's metadata, or from its base type,
// or fall through to derive_llvm_type.
let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
    PropertyValue::Identifier(s) => Some(s.as_str()),
    _ => None,
});
let llvm_ty = ctd.and_then(|c| ctd_to_llvm(c, rt.bytes))
    .unwrap_or_else(|| derive_llvm_type(None, rt.bytes, &prim_config));
rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
```

**Update the file-level doc comment** (lines 1–4) to describe CTD-driven mapping.

**Add rationale comments:**
- At the `continue` removal site: `// 2026-07-17: Normalizer is single authority for llvm_type — always compute from CTD`
- At `ctd_to_llvm()`: `// 2026-07-17: CTD→LLVM mapping. String/Data are heap-allocated → ptr at ABI level.`
- At `validate_alu_ctd()`: `// 2026-07-17: Validate CTD × ALU compatibility. Backend-specific ALUs skip.`

**Tests:**
- Existing tests must pass (they test behavioral outcomes, not IR snapshots)
- Add a test for `validate_alu_ctd`: verify `Float` × `String` returns Err,
  `Float` × `Double` returns Ok, `Int` × `Int` returns Ok
- Add a test for `ctd_to_llvm`: verify `("String", 24)` → `Some("ptr")`,
  `("Int", 8)` → `Some("i64")`, `("Bool", 1)` → `Some("i8")`

---

#### Commit 4: Fix `rt_llvm_type` — read from property

**File: `src/backend/llvm/emit_toplevel.rs` (line 14–16)**

```rust
/// Derive the LLVM type string for a ResolvedType.
/// First checks the normalizer-set `llvm_type` property. Falls back to
/// derive_llvm_type for types that bypassed the normalizer (tests, edge cases).
fn rt_llvm_type(rt: &ResolvedType) -> String {
    // 2026-07-17: Normalizer is the single authority. Read from property.
    if let Some(crate::ast::PropertyValue::String(s)) = rt.properties.get("llvm_type") {
        return s.clone();
    }
    // Fallback: types without normalizer annotation (test code, manual construction)
    let ctd = rt.properties.get("ctd").and_then(|pv| {
        if let crate::ast::PropertyValue::Identifier(s) = pv { Some(s.as_str()) } else { None }
    });
    crate::config::derive_llvm_type(ctd, rt.bytes, &*TYPE_CONFIG)
}
```

**File: `src/backend/llvm/helpers.rs` (line 29–31)**

Identical change — the `rt_llvm_type` helper at the top of this file is a
duplicate. Apply the same fix.

**File: `src/backend/llvm/helpers.rs` lines 636, 1067, 1207**

Replace `rt.primitive() == Some("Float")` checks with ALU-based checks:

```rust
// operator_llvm_type (line 636):
fn operator_llvm_type(&self, ty: &Type) -> &'static str {
    if let Some(ref universe) = self.ctx.type_universe {
        if let Some(rt) = ty.universe_key().and_then(|k| universe.get(k)) {
            // 2026-07-17: Read ALU property instead of primitive
            let is_float = rt.properties.get("alu").and_then(|pv| match pv {
                PropertyValue::Identifier(s) => Some(s.as_str() == "Float"),
                _ => None,
            }).unwrap_or(false);
            if is_float && rt.bytes <= 4 {
                return "float";
            }
            if is_float {
                return "double";
            }
            return "i64";
        }
    }
    // fallback omitted for brevity — same as current
}

// is_native_float (line 1067):
fn is_native_float(&self, ty: &Type) -> bool {
    self.ctx.type_universe.as_ref()
        .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
        .map(|r| {
            // 2026-07-17: Read ALU property
            r.properties.get("alu").and_then(|pv| match pv {
                PropertyValue::Identifier(s) => Some(s.as_str() == "Float"),
                _ => None,
            }).unwrap_or(false)
        })
        .unwrap_or_else(|| {
            type_is(&self.ctx.type_universe, ty, "Float")
                || type_is(&self.ctx.type_universe, ty, "Float64")
        })
}
```

The second `is_native_float` usage at line 1207 gets the same treatment.

**File: `src/backend/llvm/intrinsics.rs` (line 97)**

Rename `resolve_arg_primitive` to `resolve_arg_ctd`:

```rust
/// Resolve the CTD metadata for a typed register's type.
fn resolve_arg_ctd(backend: &LlvmBackend, reg: &BTypedRegister) -> String {
    // 2026-07-17: Read CTD from universe instead of primitive
    backend.ctx.type_universe.as_ref()
        .and_then(|u| crate::type_universe::resolve_type(u, &reg.ty))
        .and_then(|rt| rt.properties.get("ctd").and_then(|pv| {
            if let crate::ast::PropertyValue::Identifier(s) = pv { Some(s.clone()) } else { None }
        }))
        .unwrap_or_else(|| "Int".to_string())
}
```

Update all callers of `resolve_arg_primitive` to use `resolve_arg_ctd`.

---

#### Commit 5: Fix Webstack normalizer

**File: `src/backend/webstack_normalizer.rs`**

Switch from `rt.primitive()` to `rt.properties.get("ctd")`:

```rust
for rt in universe.types.values_mut() {
    // 2026-07-17: Read CTD property instead of primitive
    let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.as_str()),
        _ => None,
    });
    let js_type = match ctd {
        Some("Int") | Some("UInt") | Some("Char") => "number",
        Some("Float") | Some("Double") => "number",
        Some("Bool") => "boolean",
        Some("String") => "string",
        Some("Data") => "Uint8Array",
        _ => "object",
    };
    rt.properties.insert("js_type".into(), PropertyValue::String(js_type.into()));
}
```

Update keep list:
```rust
let keep: HashSet<String> = ["ctd", "alu", "js_type", "encoding", "bytes"]
    .iter().map(|s| s.to_string()).collect();
```

Update the module-level doc comment to describe CTD-driven JS type assignment.

---

#### Commit 6: Fix SPIR-V normalizer

**File: `src/backend/spirv/normalizer.rs`**

Switch from `derive_alu_type(prim, bytes, config)` to reading the `alu` property:

```rust
for rt in universe.types.values_mut() {
    // 2026-07-17: Read ALU from primordial property instead of deriving from primitive
    let alu = rt.properties.get("alu").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.clone()),
        PropertyValue::String(s) => Some(s.clone()),
        _ => None,
    }).unwrap_or_else(|| "Int".to_string());
    rt.properties.insert("alu".into(), PropertyValue::String(alu));
}
```

Note: The SPIR-V normalizer stores `alu` as a `String` property (not `Identifier`)
because SPIR-V codegen reads it as a string. The primordial already stores it as
`Identifier` — this conversion is fine.

Update keep list:
```rust
let keep: HashSet<String> = ["ctd", "alu", "bytes", "encoding", "is_kernel"]
    .iter().map(|s| s.to_string()).collect();
```

---

#### Commit 7: Update parser — `primitive` → `ctd` and add `alu`

**File: `src/parser/definitions.rs`**

At lines 768–773 (the `primitive` metadata slot), change to `ctd`:

```rust
if slot_name == "ctd" && self.check(&Token::TildeArrow) {
    self.advance();
    let ctd_name = self.expect_identifier()?;
    self.eat(&Token::Semicolon);
    metadata.insert("ctd".into(), PropertyValue::Identifier(ctd_name));
    continue;
}
```

Add a new `alu` slot parser after the `ctd` block:

```rust
if slot_name == "alu" && self.check(&Token::TildeArrow) {
    self.advance();
    // PascalCase identifier → known built-in ALU
    // Lowercase quoted string → backend/plugin-specific
    if self.check(&Token::Identifier) {
        let alu_name = self.expect_identifier()?;
        self.eat(&Token::Semicolon);
        metadata.insert("alu".into(), PropertyValue::Identifier(alu_name));
    } else {
        let alu_str = self.expect_string()?;
        self.eat(&Token::Semicolon);
        metadata.insert("alu".into(), PropertyValue::String(alu_str));
    }
    continue;
}
```

Update the `primitive` reference at line 872 (the second occurrence for the other
parsing path) identically. Actually, after checking lines 765–876, there are two
locations: lines 768–773 and lines 872–876. Both need the same change.

Leave a deprecation path for `primitive` — emit a warning if the old `primitive`
slot name is used, pointing to `ctd`:

```rust
if slot_name == "primitive" {
    // 2026-07-17: Deprecated — use `ctd` instead
    eprintln!("Warning: 'primitive' metadata slot is deprecated, use 'ctd' instead");
    // Still parse it for backward compat during transition
    if self.check(&Token::TildeArrow) { ... }
    continue;
}
```

Actually, the AGENTS.md says "NEVER DISCARD UNCOMMITTED WORK" and "ALWAYS FINISH."
A deprecation path adds complexity without value — this is a compiler, not a web
app. Just remove the old `primitive` parsing entirely. Any user code using
`primitive ~> ...` will get a parse error pointing them to `ctd`.

But wait — do any existing `.bv` files use `primitive ~> ...`? Let's check.

*(Checked during implementation — if any std lib or benchmark files use the old
syntax, update them as part of this commit.)*

**Update keep list in all normalizers** — already done in Commits 3, 5, 6.

---

#### Commit 8: Clean up dead code and update docs

**Remove `ResolvedType::primitive()` method** from `src/type_universe/mod.rs` (lines
32–34) — all callers have been migrated to reading `ctd` or `alu` properties.

**Update `docs/architecture/backend-type-dispatch.md`** to describe CTD + ALU
instead of primitive:

- Document the three-layer metadata system (primordial → normalizer → backend)
- List the exhaustive CTD set and their LLVM/JS/SPIR-V mappings
- Document the ALU validation rules
- Add a rationale section explaining why `primitive` was replaced

**Add `docs/architecture/ctd-and-alu.md`** — a concise reference document:

- Purpose of CTD vs ALU
- The naming convention (PascalCase vs quoted lowercase)
- The exhaustive CTD list
- How to add a new CTD (frontend change + normalizer change for each backend)
- Validation rules and error messages
- Interaction with user-defined types (`ctd ~> ...;` in `type X <: Bits { }`)

---

#### Commit 9: Verify and benchmark

1. `cargo test --lib` — all 899+ tests pass
2. `cargo build --release` — no warnings
3. `bash benchmarks/build_and_bench.sh --correctness` — all MATCH (no `__FAIL__`)
4. `bash benchmarks/build_and_bench.sh --runtime` — runtime numbers comparable to Phase 3 baseline
5. Run Praetor on new/changed files
6. Add Kani harnesses for `ctd_to_llvm()` and `validate_alu_ctd()`

### Expected Outcome

After all 9 commits:

| Benchmark | Expected | Notes |
|-----------|----------|-------|
| All optimizer benchmarks | MATCH | `get_env_int` parameter type is `ptr` not `i192` |
| All runtime benchmarks | MATCH | `llc` succeeds on generated IR |
| Runtime performance | Same as Phase 3 baseline | No codegen changes, only metadata fix |

### Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Missed a caller of `rt.primitive()` | Low | Grep catches all 10; each is explicitly handled in the plan |
| `derive_llvm_type` signature change breaks external use | Low | Only internal callers in config.rs and normalizer backends |
| SPIR-V normalizer alu property type mismatch | Low | Primordial stores `Identifier`, SPIR-V normalizer reads as String — explicit conversion |
| Webstack normalizer was already broken by `primitive` mismatch | High | This plan FIXES it (CTD values match webstack's expectations) |
| User `.bv` files use `primitive ~> ...` syntax | Low | Check lib/std/ and benchmarks/ — update if found |
