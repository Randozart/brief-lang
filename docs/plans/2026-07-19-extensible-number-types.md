# Extensible Number Types — LLVM Backend

**Date:** 2026-07-19
**Author:** Brief Compiler Team
**Status:** Plan — ready for implementation

## Problem

Adding a new numeric type (Bfloat16, Posit32, Decimal64) requires hardcoded changes in ~7 backend code sites that match type names instead of reading type properties. The normalizer already stamps `llvm_type` on every `ResolvedType`, but the backend doesn't trust it — it has fallback name-matching that bypasses the normalizer.

Separately, the stdlib uses `llvm <~ "float"` to set the LLVM type directly on primitive types. This means the normalizer's `llvm_type` computation is redundant for these types — it's overridden by the property that `llvm <~` produces. This is backward: the config should drive the LLVM type, not the stdlib.

## Core Insight

**Replace CTD-driven dispatch with category-driven dispatch.** The normalizer infers a `category` from a type's structure (fields + properties), then derives `llvm_type` from category + CTD + config/TOML. All backend code checks `category` instead of matching type names. The normalizer is the sole authority for `llvm_type`.

### Category Inference Rules

| Category | Detected by | Example types |
|----------|------------|---------------|
| `"String"` | 2 Int fields (`data`, `len`) + `encoding` property | String, UTF8View, StaticString, SmallString64, MyString |
| `"Float"` | `alu <~ "Float"` property | Float, Float64, Bfloat16, FP16 |
| `"Bits"` | Everything else | Int, Bool, Char, Data, Posit32, Decimal64 |

### llvm_type Derivation (in order of priority)

1. **Explicit override**: If the type has an explicit `llvm` property (set by user code via `llvm <~ "..."`), validate it and use it directly. Reject invalid LLVM types with a clear error.
2. **Category-driven**:
   - `"Float"` category → `derive_llvm_type(CTD, bytes, config)` looks up config/ctd-llvm-mappings.toml
   - `"String"` category → `"{ i64, i64 }"` (two-field struct)
   - `"Bits"` category → `derive_llvm_type(CTD, bytes, config)`; if CTD not found, fall back to `"i{N*8}"`

### Explicit `llvm` Validation

If a type sets `llvm <~ "some_type"`, the normalizer must validate it against a set of known LLVM type strings:
- Native floats: `"half"`, `"bfloat"`, `"float"`, `"double"`, `"fp128"`
- Native integers: `"i1"`, `"i8"`, `"i16"`, `"i32"`, `"i64"`, `"i128"`
- Pointers: `"ptr"`
- Structs: `"{ ... }"` (syntax check only — full validation deferred to LLVM)
- Vectors: `"<N x type>"` (syntax check only)

If the value doesn't match any known pattern, the normalizer returns an error:
```
error: invalid LLVM type '{invalid}' for type 'MyType'
```

### Architecture Shift

```
Before:  CTD("Float") → ctd_to_llvm("Float", 4) → "float" → fadd float
After:   category("Float") → derive_llvm_type("Float", 4, config) → "float" → fadd float

Before:  type name "String" || ad-hoc fields check → "{ i64, i64 }"
After:   category("String") → "{ i64, i64 }" (always from structure)

Before:  stdlib says `llvm <~ "float"` → overrides normalizer
After:   stdlib says nothing about LLVM → normalizer derives from config
```

## Implementation Plan

### Phase 1: Category Inference + Generalize Dispatch

This phase has 10 sub-tasks (1a–1j). Each is a small, testable change. Order matters — do them sequentially.

#### 1a. Normalizer: category inference + config-driven llvm_type

**File: `src/backend/llvm/normalizer.rs`**

**Changes:**

1. Add `infer_category()` function:
```rust
/// 2026-07-19: Infer a type's category from its structure and properties.
/// "String" — 2 Int fields + encoding property (structural duck test)
/// "Float"  — ALU property says "Float"
/// "Bits"   — default for everything else
fn infer_category(rt: &ResolvedType) -> &'static str {
    // String-like: two Int fields (data, len) + encoding property.
    // This catches user-defined types like MyString that walk like String.
    if rt.fields.len() == 2
        && rt.fields[0].1 == Type::int()
        && rt.fields[1].1 == Type::int()
        && rt.properties.contains_key("encoding")
    {
        return "String";
    }
    // Float-like: ALU says Float. The ALU property is set by the
    // primordial type system or by explicit `alu <~ "Float"` in .bv.
    if let Some(PropertyValue::Identifier(s)) = rt.properties.get("alu") {
        if s == "Float" { return "Float"; }
    }
    // Bits: default for all other types (Int, Bool, Char, Data, Posit32, etc.)
    "Bits"
}
```

2. Add explicit `llvm` validation function:
```rust
/// 2026-07-19: Validate that a user-provided LLVM type string is valid.
/// Returns Ok(()) if the type string is syntactically valid, Err with
/// a descriptive message otherwise.
fn validate_explicit_llvm(llvm_val: &str) -> Result<(), String> {
    // Native float types
    match llvm_val {
        "half" | "bfloat" | "float" | "double" | "fp128"
        | "x86_fp80" | "ppc_fp128" => return Ok(()),
        _ => {}
    }
    // Native integer types: i1, i8, i16, i32, i64, i128, i256
    if llvm_val.starts_with('i') {
        let bits = &llvm_val[1..];
        if bits.parse::<u64>().is_ok() {
            return Ok(());
        }
    }
    // Pointer
    if llvm_val == "ptr" {
        return Ok(());
    }
    // Void
    if llvm_val == "void" {
        return Ok(());
    }
    // Simple struct check: { type, type, ... }
    if llvm_val.starts_with('{') && llvm_val.ends_with('}') {
        return Ok(());  // syntax check only
    }
    // Vector check: <N x type>
    if llvm_val.starts_with('<') && llvm_val.contains("x ") && llvm_val.ends_with('>') {
        return Ok(());  // syntax check only
    }
    Err(format!("invalid LLVM type '{}': expected a known LLVM type (float, double, iN, ptr, half, bfloat, ...)", llvm_val))
}
```

3. Restructure the main normalize loop — remove llvm_type computation from the first pass, add a second pass after `register_typedefs()`:

```rust
// 2026-07-19: Pass 1 — ALU × CTD validation only.
// llvm_type and category are computed in Pass 2 (after register_typedefs).
for rt in universe.types.values_mut() {
    let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.as_str()),
        _ => None,
    });
    let alu = rt.properties.get("alu").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.as_str()),
        _ => None,
    });
    if let (Some(a), Some(c)) = (alu, ctd) {
        if let Err(e) = validate_alu_ctd(a, c) {
            return Err(e);
        }
    }
    // Keep layout parsing (unchanged from existing code)
    if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
        let cleaned = layout_str.strip_prefix('<').unwrap_or(layout_str);
        if let Ok(pat) = crate::beast::layout::parse_layout_pattern(cleaned) {
            attach_layout_fields(rt, &pat);
        }
    }
}
```

4. Add a new Pass 2 after `register_typedefs()`:

```rust
// 2026-07-19: Pass 2 — Infer category and compute llvm_type for ALL types.
// This runs after register_typedefs so struct types have their fields set.
for rt in universe.types.values_mut() {
    let category = infer_category(rt);
    rt.properties.insert("category".into(), PropertyValue::String(category.into()));

    let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
        PropertyValue::Identifier(s) => Some(s.as_str()),
        _ => None,
    });

    // 2026-07-19: Check for explicit user-provided llvm override first.
    // Stdlib types never set this — it's for user custom types only.
    let explicit_llvm = rt.properties.get("llvm").and_then(|pv| match pv {
        PropertyValue::String(s) => Some(s.as_str()),
        _ => None,
    });

    let llvm_ty = if let Some(llvm_val) = explicit_llvm {
        // Validate user-provided LLVM type string
        validate_explicit_llvm(llvm_val)?;
        llvm_val.to_string()
    } else {
        // Derive from category + CTD + config
        match category {
            "Float" => derive_llvm_type(ctd, rt.bytes, &prim_config),
            "String" => "{ i64, i64 }".to_string(),
            _ => derive_llvm_type(ctd, rt.bytes, &prim_config),
        }
    };
    rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
}
```

5. In `register_typedefs()`, remove the llvm_type computation (lines 350-360) — it will be set by Pass 2. Remove the inheritance of llvm (only inherit ctd, alu, encoding).

6. Add `"category"`, `"tbaa_parent"` to the metadata keep list (line 103):
```rust
let keep: HashSet<String> = ["ctd", "alu", "category", "llvm_type", "encoding",
    "layout", "svo", "tbaa_parent", "op.InsertAt", "op.ExtractFrom"]
    .iter().map(|s| s.to_string()).collect();
```

Note: `"llvm"` (the user-facing property) is NOT kept — it's consumed by the normalizer and replaced with `"llvm_type"` (the derived value).

7. Update the doc comment at the top to explain the new architecture.

**Verification:** `cargo test --lib` passes. The existing `ctd_to_llvm` tests still pass (function kept for backward compat). All types in the universe now have `category` and `llvm_type` set.

---

#### 1a-ii. Remove `llvm <~` from stdlib .bv files

**Files:**
- `lib/std/types/bootstrap.bv` (13 instances: lines 13, 34, 54, 75, 95, 116, 136, 157, 177, 198, 219, 233, 289)
- `lib/std/types/float.bv` (2 instances: lines 14, 26)

Remove every `llvm <~ "..."` line from these files. The normalizer now derives `llvm_type` from CTD + bytes + config, so these are redundant.

**Rationale:** Stdlib describes what types ARE (semantics, structure), not how they're REPRESENTED in one backend. The config/TOML files map (CTD, bytes) → LLVM type for all backends. This is the "Intrinsics Before Frgn" principle applied to type representation.

---

#### 1a-iii. Update config/ctd-llvm-mappings.toml

**File: `config/ctd-llvm-mappings.toml`**

Add missing entries so all primordial types resolve correctly:

```toml
[ctd.UInt]
1 = "i8"
2 = "i16"
4 = "i32"
8 = "i64"

[ctd.Char]
4 = "i32"

[ctd.Data]
8 = "ptr"

[ctd.Void]
0 = "void"
```

Also add entries for future types (will be used in Phase 2):
```toml
[ctd.Bfloat16]
2 = "bfloat"

[ctd.Float16]
2 = "half"
```

---

#### 1b. Generalize `TypedRegister::llvm()`

**File: `src/backend/llvm/mod.rs`, lines 319-335**

Change return type from `&'static str` to `String`. Replace hardcoded type-name matching with a universe query that reads the `llvm_type` property:

```rust
impl TypedRegister {
    pub fn llvm(&self, universe: Option<&TypeUniverse>) -> String {
        // 2026-07-19: Query the universe for the stamped llvm_type property.
        // This handles all categories (Float, String, Bits) uniformly.
        if let Some(u) = universe {
            if let Some(key) = self.ty.universe_key() {
                if let Some(rt) = u.get(key) {
                    if let Some(PropertyValue::String(s)) = rt.properties.get("llvm_type") {
                        return s.clone();
                    }
                }
            }
        }
        // Fallback: no universe or type not found — use type-based defaults.
        match &self.ty {
            Type::Custom(t) if t == "Bool" => "i1",
            _ => "i64",
        }
    }
}
```

Wait — this changes the signature. All callers need updating. Let me count callers...

Actually, I should grep for all call sites of `TypedRegister::llvm()` first. Let me do that in the implementation phase.

The fallback for Bool should be "i1" (LLVM's boolean type), not "i8" (C bool). Actually, currently Bool maps to "i1" in TypedRegister::llvm(). Let me check...

Looking at the existing code:
```rust
if self.ty == Type::bool_() { "i1" }
```

Yes, "i1" for Bool. The fallback should keep this.

---

#### 1c. Fix `fallback_llvm_type()`

**File: `src/backend/llvm/emit_toplevel.rs`, lines 290-308**

Currently maps both `"Float"` AND `"Float64"` to `"double"` — a known bug for Float32.

After Phase 1a, the normalizer's `llvm_type` property is authoritative. This function should be simplified to just read the normalizer-stamped property, or if that's not available, use bytes:

```rust
fn fallback_llvm_type(ty: &Type) -> &'static str {
    // 2026-07-19: This is only reached when the universe query fails.
    // All properly normalized types have llvm_type from the normalizer.
    match ty {
        Type::Custom(t) if t == "Bool" => "i8",
        Type::Custom(t) if t == "Char" => "i32",
        Type::String { ... } => "ptr",
        Type::Bits(w) => match w { 1=>"i1", 8=>"i8", 16=>"i16", 32=>"i32", 64=>"i64", _=>"i64" },
        _ => "i64",
    }
}
```

Remove the `"Float" | "Float64" => "double"` arm. The normalizer handles these correctly now:
- Float (CTD="Float", bytes=4) → config lookup → "float"
- Float64 (CTD="Double", bytes=8) → "double"

---

#### 1d. Generalize `adapt_to_i64()`

**File: `src/backend/llvm/helpers.rs`, lines 1983-2022**

Replace the 4-type-name match with a helper that reads `llvm_type` from the resolved type and dispatches:

```rust
pub(crate) fn adapt_to_i64(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
    // 2026-07-19: Generalized boxing — reads llvm_type from universe.
    // Handles any LLVM type: float, double, bfloat, half, i8, i16, i32, i64, etc.
    match &reg.ty {
        Type::Custom(t) if t == "String" || t == "Data" => {
            // SSO String: extract the data pointer (first i64 field)
            if self.ctx.config.sso_strings {
                let sso_reg = self.fun.gen_reg();
                writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, sso_reg, reg.name).ok();
                return sso_reg;
            }
            // Legacy String: ptrtoint i8* to i64
            let pti = self.fun.gen_reg();
            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, pti, reg.name).ok();
            pti
        }
        Type::Ptr(_) => reg.name.clone(),
        _ => {
            // Read llvm_type from universe to determine boxing strategy
            let llvm_ty = self.llvm_type(&reg.ty);
            let bytes = /* resolve bytes */ 8;
            box_to_i64(&reg.name, &llvm_ty, bytes, &mut self.fun, out, indent)
        }
    }
}
```

The `box_to_i64` helper:
```rust
/// 2026-07-19: Box any native LLVM value to i64 for state storage.
/// - float/double/half/bfloat: bitcast then zext if needed
/// - i8/i16/i32: zext to i64
/// - i64: identity
/// - ptr: ptrtoint to i64
fn box_to_i64(reg: &str, llvm_ty: &str, bytes: u64, fun: &mut FunctionContext, out: &mut String, indent: &str) -> String {
    if bytes == 8 && !is_float_llvm_type(llvm_ty) {
        return reg.to_string();  // Already i64 or i64-equivalent
    }
    match llvm_ty {
        "double" => {
            let r = fun.gen_reg();
            writeln!(out, "{}{} = bitcast double {} to i64", indent, r, reg).ok();
            r
        }
        "float" => {
            let t = fun.gen_reg();
            let r = fun.gen_reg();
            writeln!(out, "{}{} = bitcast float {} to i32", indent, t, reg).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, r, t).ok();
            r
        }
        "bfloat" | "half" => {
            let t = fun.gen_reg();
            let r = fun.gen_reg();
            writeln!(out, "{}{} = bitcast {} {} to i16", indent, t, llvm_ty, reg).ok();
            writeln!(out, "{}{} = zext i16 {} to i64", indent, r, t).ok();
            r
        }
        _ if llvm_ty.starts_with('i') && bytes < 8 => {
            let r = fun.gen_reg();
            writeln!(out, "{}{} = zext {} {} to i64", indent, r, llvm_ty, reg).ok();
            r
        }
        "ptr" => {
            let r = fun.gen_reg();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, r, reg).ok();
            r
        }
        _ => {
            // Fallback: try bitcast
            let r = fun.gen_reg();
            writeln!(out, "{}{} = bitcast {} {} to i64", indent, r, llvm_ty, reg).ok();
            r
        }
    }
}
```

Similarly, add `unbox_from_i64()` for the inverse operation.

---

#### 1e. Generalize `TypeConverter` in builder.rs

**File: `src/backend/llvm/builder.rs`, lines 526-618**

The `TypeConverter::box_to_i64()` and `unbox_from_i64()` methods have the same hardcoded float/bool patterns. Replace with calls to the generalized helpers from 1d (or share the same helper functions).

---

#### 1f. Float intrinsic dispatch

**File: `src/backend/llvm/intrinsics.rs`, lines 88-117**

Replace the `is_float_unary = matches!(...)` type-name check with a category check:

```rust
// 2026-07-19: Check category instead of type name.
// Any type with category "Float" gets the native LLVM float intrinsic.
let is_float_op = back.ctx.type_universe.as_ref()
    .and_then(|u| arg_regs[0].ty.universe_key().and_then(|k| u.get(k)))
    .map(|rt| {
        rt.properties.get("category").and_then(|pv| match pv {
            PropertyValue::String(s) => Some(s == "Float"),
            _ => None,
        }).unwrap_or(false)
    })
    .unwrap_or(false);
```

If `is_float_op`, query the config for the specific LLVM intrinsic suffix based on the type's `llvm_type` property. For a "float" type, use `.f32`. For "double", use `.f64`. For "bfloat", use `.bf16`.

---

#### 1g. Float literal emission

**File: `src/backend/llvm/emit_expr.rs`, lines 38-52**

Currently always emits as `f32`. Change to emit based on the target type's `llvm_type`:

```rust
Expr::Float(f) => {
    // 2026-07-19: Determine the float type from context or explicit type annotation.
    // If no type info, default to Float32.
    let target_llvm_ty = /* resolve from type context or "float" */;
    match target_llvm_ty {
        "float" => {
            // Current path: f32 literal
            let h = float_to_llvm_hex(*f);
            let hex_reg = self.fun.gen_reg();
            let flt_reg = self.fun.gen_reg();
            writeln!(out, "{}{} = add i32 0, {}", indent, hex_reg, h).ok();
            writeln!(out, "{}{} = bitcast i32 {} to float", indent, flt_reg, hex_reg).ok();
            writeln!(out, "{}{} = fadd float 0.0, {}", indent, v, flt_reg).ok();
            TypedRegister { name: v, ty: Type::float() }
        }
        "double" => {
            // f64 literal
            let h = float64_to_llvm_hex(*f);
            let hex_reg = self.fun.gen_reg();
            let flt_reg = self.fun.gen_reg();
            writeln!(out, "{}{} = add i64 0, {}", indent, hex_reg, h).ok();
            writeln!(out, "{}{} = bitcast i64 {} to double", indent, flt_reg, hex_reg).ok();
            writeln!(out, "{}{} = fadd double 0.0, {}", indent, v, flt_reg).ok();
            TypedRegister { name: v, ty: Type::float64() }
        }
        "half" => { /* same pattern with i16 + bitcast to half */ }
        "bfloat" => { /* same pattern with i16 + bitcast to bfloat */ }
        _ => { /* error: can't emit literal for non-float type */ }
    }
}
```

---

#### 1h. `ensure_float_reg()` generalization

**File: `src/backend/llvm/emit_toplevel.rs`, lines 366-387**

Change from type-name matching to reading `llvm_type`:

```rust
pub(super) fn ensure_float_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
    // 2026-07-19: Generalize to handle any float-like LLVM type.
    // Promote smaller float types to double for C variadic printf compat.
    let llvm_ty = /* read from universe */;
    match llvm_ty {
        "float" => {
            // Float32 → fpext to double (C variadic promotion)
            if let Some(cached) = self.fun.reg_float_cache.get(&reg.name) {
                return cached.clone();
            }
            let promoted = self.fun.gen_reg();
            writeln!(out, "{}{} = fpext float {} to double", indent, promoted, reg.name).ok();
            self.fun.reg_float_cache.insert(reg.name.clone(), promoted.clone());
            promoted
        }
        "bfloat" | "half" => {
            // Bfloat16/FP16 → fpext to double
            let promoted = self.fun.gen_reg();
            writeln!(out, "{}{} = fpext {} {} to double", indent, promoted, llvm_ty, reg.name).ok();
            promoted
        }
        _ => reg.name.clone()  // Already double or unknown — use as-is
    }
}
```

---

#### 1i. TBAA node mapping

**File: `src/backend/llvm/mod.rs`, lines 505-534**

Replace the hardcoded `"float" | "double" => "Float"` with a lookup of the type's CTD or `tbaa` property from the universe:

```rust
pub(super) fn tbaa_node(ty_str: &str, universe: Option<&TypeUniverse>) -> i32 {
    if let Some(u) = universe {
        // 2026-07-19: Look up the type by its llvm_type string in the universe.
        // If found, use its CTD (or tbaa property). Otherwise fall back.
        let group = u.types.iter()
            .find(|(_, rt)| {
                rt.properties.get("llvm_type")
                    .and_then(|pv| if let PropertyValue::String(s) = pv { Some(s.as_str()) } else { None })
                    == Some(ty_str)
            })
            .and_then(|(_, rt)| {
                rt.properties.get("tbaa").or_else(|| rt.properties.get("ctd"))
                    .and_then(|pv| if let PropertyValue::Identifier(s) = pv { Some(s.as_str()) } else { None })
            })
            .unwrap_or("Int");
        // ... calculate position in sorted groups ...
    } else {
        // No universe: hardcoded fallback
        match ty_str {
            "i64" => 1, "i8" => 2, "i32" => 3,
            "ptr" => 4, "float" | "double" => 5,
            _ => 1,
        }
    }
}
```

---

#### 1j. Phi float unboxing

**File: `src/backend/llvm/emit_expr.rs`, lines 91-109**

Replace inline `Type::float64()`/`Type::float()` checks with a call to the generalized `unbox_from_i64()` helper:

```rust
// 2026-07-19: Generalized phi unboxing — reads llvm_type from universe.
let unboxed = self.unbox_phi_value(out, indent, &phi_reg, &brief_ty);
```

Where `unbox_phi_value` uses the same `unbox_from_i64()` helper from 1d.

---

#### 1k. `operator_llvm_type()` and `is_native_float()` — already ALU-based

**File: `src/backend/llvm/helpers.rs`, lines 640-659 and 1201-1214**

These already use the ALU property. They should be updated to check `category = "Float"` instead of `alu = "Float"` for consistency, but this is not strictly required — they already work correctly for all float types.

---

### Phase 2: Native Float Types via Config

After Phase 1, adding a native LLVM float type requires only:
1. A type definition in stdlib (or user code)
2. Entries in config/ctd-llvm-mappings.toml
3. Entries in config/llvm-ops.toml for the operations

No Rust code changes beyond Phase 1.

**Stdlib types to add:**

```brief
// lib/std/types/float.bv
type Bfloat16 <: Bits {
    bytes <~ 2; alignment <~ 2;
    alu <~ "Float";
    tbaa_parent <~ "Float";
    commuting <~ true;
    op Add ~> "float.add";
    op Sub ~> "float.sub";
    op Mul ~> "float.mul";
    // ... etc
};
```

**Config entries:**

```toml
[op.Add.Bfloat16."2"]
template = "fadd fast bfloat %a, %b"
[op.Sub.Bfloat16."2"]
template = "fsub fast bfloat %a, %b"
[op.Mul.Bfloat16."2"]
template = "fmul fast bfloat %a, %b"
[op.Div.Bfloat16."2"]
template = "fdiv fast bfloat %a, %b"
```

**Builder additions:**

```rust
// src/backend/llvm/builder.rs
pub enum LlvmType {
    I1, I8, I16, I32, I64, I128,
    Float, Double, Half, BFloat, FP128,
    Ptr, Void,
}
```

Add `Display`, `as_int_ty()`, `size_bytes()` for the new variants.

---

### Phase 3: Auto-Inline + Auxlib (Post-Phase-2 Feature)

#### How auto-inline works

When the op template references a function name (e.g., `call i32 @Posit32_add`):
1. The normalizer detects that `Posit32_add` is referenced in an op template
2. It looks up the function definition in the universe (it's a `defn` in stdlib)
3. When emitting the function, it adds LLVM's `alwaysinline` attribute
4. LLVM's inliner at `-O3` eliminates the call, leaving only the integer ops

**No `inline` keyword needed.** The relationship is implicit: if a function is referenced by an op template, it's auto-inlined.

#### Implementation

In the normalizer, after all types and functions are registered:
1. Scan all op templates for function call references
2. Build a set of "auto-inline" function names
3. Pass this set to the codegen phase
4. In `emit_toplevel.rs`, when emitting a function definition:
   ```rust
   if auto_inline_fns.contains(&func_name) {
       writeln!(out, "define ... @{}() alwaysinline {{", func_name)
   }
   ```

#### Posit32 demonstration

```brief
type Posit32 <: Bits {
    bytes <~ 4; alignment <~ 4;
    op Add ~> "Posit32_add";
    op Mul ~> "Posit32_mul";
};

defn Posit32_add(a: UInt32, b: UInt32) -> UInt32 {
    // Pure bit manipulation — no Posit32-specific operations
    // (implemented in std/aux/Posit32.bv)
};
```

Config:
```toml
[op.Add.Posit32."4"]
template = "call i32 @Posit32_add(i32 %a, i32 %b)"
[op.Mul.Posit32."4"]
template = "call i32 @Posit32_mul(i32 %a, i32 %b)"
```

---

### Phase 4: Target Capability Awareness (Post-Phase-2 Feature)

`config/targets.toml` extended with per-target native float type lists. The normalizer checks if the target supports a given type natively and can fall back to a wider type if not.

---

### Phase 5: Hierarchical TBAA (Post-Phase-2 Feature)

TBAA tree changes from flat `!N = !{!"Type", !0}` to parent-based hierarchy:
```
!2 = !{!"Float", !0}
!3 = !{!"Float32", !2}   # Float32 IS A Float
!4 = !{!"Bfloat16", !2}  # Bfloat16 IS A Float
```

Controlled by `tbaa_parent <~ "Float"` in type definitions.

---

## Documentation Updates

### Architecture docs to update:
- `docs/architecture/backend-type-dispatch.md` — Replace CTD-based dispatch docs with category-based dispatch. Document `category` inference rules.
- `docs/architecture/intrinsics-vs-stdlib.md` — Update to note that `llvm_type` is derived by normalizer, not set in stdlib.
- `docs/plans/2026-07-19-extensible-number-types.md` — This file (the plan document itself).

### Code comments to add at each modified site:

Every modified code site must have a rational comment:
```
// 2026-07-19: [why this change exists]
// [what pattern it targets, what problem it solves]
```

Specific comments are specified in each sub-task above.

---

## Test Plan

All tests must be **behavioral** (assert correctness), not **literal** (no IR snapshot matching):

### Phase 1 tests:
1. Normalizer stamps `category` on every type — run pass, inspect universe
2. Float types (Float, Float64) get correct `llvm_type` without `llvm <~` in stdlib
3. String-like types get `category = "String"` and `llvm_type = "{ i64, i64 }"`
4. Int types get correct `llvm_type` from config
5. All existing benchmarks produce identical output to C reference

### Phase 2 tests:
6. Bfloat16 type compiles to IR with `bfloat` LLVM type
7. Bfloat16 arithmetic emits `fadd bfloat` etc.
8. FP16 → `half` in IR
9. FP128 → `fp128` in IR

### Phase 3 tests:
10. Posit32 function gets `alwaysinline` attribute in IR
11. Posit32 arithmetic produces correct results (interpreter)
12. After `opt -O3`, the inline call barrier is eliminated

---

## Rationale Comments Map

| File | Line(s) | Comment |
|------|---------|---------|
| `normalizer.rs` | new | `infer_category()` doc: structural inference rules |
| `normalizer.rs` | Pass 2 loop | category-driven llvm_type, explicit llvm validation |
| `mod.rs:319` | `TypedRegister::llvm()` | reads llvm_type from universe |
| `emit_toplevel.rs:290` | `fallback_llvm_type()` | simplified: Float/Float64 handled by normalizer |
| `helpers.rs:1983` | `adapt_to_i64()` | generalized via llvm_type |
| `helpers.rs:new` | `box_to_i64/unbox_from_i64` | centralized boxing helpers |
| `intrinsics.rs:88` | float intrinsic dispatch | category check replaces type-name matching |
| `emit_expr.rs:38` | float literal | per-llvm_type literal emission |
| `emit_toplevel.rs:366` | `ensure_float_reg()` | generalized for any float llvm_type |
| `mod.rs:505` | `tbaa_node()` | category-driven TBAA lookup |
| `emit_expr.rs:91` | phi unboxing | generalized unbox helper |
