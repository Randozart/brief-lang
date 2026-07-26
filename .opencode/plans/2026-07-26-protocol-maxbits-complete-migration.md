# Complete Migration: Protocol+Maxbits-Driven Type System

**Date:** 2026-07-26
**Status:** Planned
**Audit:** Every `Type::Custom(t) if t == "X"` pattern in the compiler and glue code.

## Principle

A type's LLVM representation, ABI width, boxing behavior, and protocol
category are derived from its `ResolvedType` in the `TypeUniverse` — never
from matching on its Brief type name string. The `ResolvedType` contains:

| Source | Field | Example |
|--------|-------|---------|
| PRIMORDIALS | `llvm_type` property | `"float"`, `"i32"` |
| PRIMORDIALS | `max_bits`, `min_bits` | `(32, 32)`, `(0, 64)` |
| Normalizer | `Cast.#<Protocol>` property | `Cast.#Float = true` |
| Normalizer | `category` property | `"Float"`, `"Int"` |
| Decl metadata | `bits`, `maxbits`, `minbits` | user-specified overrides |

## Status Quo Audit

Every file in `src/backend/llvm/` and `src/glue/` was searched for:
- `if s == "Int"`, `if s == "Float"`, etc. on `Type::Custom`
- `type_is(..., "X")` calls
- `is_native_float()` usage
- `*ty == Type::float()` enum comparisons (bootstrap — justified)

**Total sites found: ~200**, of which ~140 need conversion (the rest are
bootstrap primitives or test data). They group into 5 functional clusters:

### Cluster A: Type-to-LLVM mapping (~30 sites)
These take a Brief type and return an LLVM type string. They all duplicate
the same information that already exists in the primordial `llvm_type`
property.

| Function | File | Lines | Current approach |
|----------|------|-------|-----------------|
| `protocol_llvm_type()` | `mod.rs` | 496-523 | 20-line name match table |
| `lower_custom_type()` | `types.rs` | 29-43 | Name match table |
| `type_size()` | `types.rs` | 46-70 | Name match → byte size |
| `fallback_llvm_type()` | `emit_toplevel.rs` | 223-242 | Name match table |
| constant emission (×3) | `mod.rs` | 2278-2337 | 3× repeated name match |
| TBAA `ty_str` group | `mod.rs` | 535-544 | LLVM type → group (justified) |

### Cluster B: Dispatch on type category (~50 sites)
These branch behavior based on whether a type is Float, Int, Bool, String,
or Data. The branch logic itself is correct — the dispatch mechanism is wrong.

| Function | File | Lines | Types checked |
|----------|------|-------|---------------|
| `type_is()` callers (17) | `helpers.rs` | 734-2598 | Int, Bool, Float, Float64, String, Data |
| `is_native_float()` | `helpers.rs` | 1641-1659 | Float, Float64 (partially protocol) |
| `emit_fcmp()` | `helpers.rs` | 1906-1931 | Float |
| `emit_typed_cast()` | `helpers.rs` | 573-609 | Int, UInt, Float, Bool, Char, String |
| `emit_projection_fast_path()` | `helpers.rs` | 2072-2075 | Int, Float, Bool |
| `emit_trg_load_finish()` | `emit_toplevel.rs` | 559-579 | Bool, Int, UInt, Float, Char, String, Data |
| defn param boxing | `emit_toplevel.rs` | 1219-1259 | Bool, Char, String, Data, Float |
| txn param boxing | `emit_toplevel.rs` | 1648-1701 | Bool, Char, String, Data, Float |
| `resolve_bild_type()` | `mod.rs` | 3585-3608 | Already universe-driven — correct |
| `is_bool_type()` | `types.rs` | 80-86 | Bool |

### Cluster C: i64 conversion / boxing (~25 sites)
These convert between a type's native LLVM representation and the uniform
i64 representation used for state storage, function arguments, and triggers.

| Function | File | Lines | Types matched |
|----------|------|-------|---------------|
| `adapt_to_i64()` | `helpers.rs` | 2675-2739 | Float64, Float, Bool, String, Data, Ptr, Int, UInt |
| `box_to_i64_fallback()` | `builder.rs` | 544-572 | Bool, String, Data, Float, Float64, Int8-UInt32 |
| `unbox_from_i64_fallback()` | `builder.rs` | 588-618 | Bool, String, Data, Float, Float64, Int8-UInt32 |
| `ensure_typed_value()` | `helpers.rs` | 2881-2934 | (LLVM type pairs — justified, operates on LLVM types) |

### Cluster D: SSO/heap allocation heuristics (~10 sites)
These determine memory representation for compound types.

| Function | File | Lines | Types matched |
|----------|------|-------|---------------|
| `type_is_heap_allocated()` | `mod.rs` | 1604-1613 | UTF8View, StaticString, SmallString64, Data, List, HashMap, HashSet, Stack, Queue, StringBuilder |
| `push_field_type()` | `mod.rs` | 944 | UTF8View |
| `is_string_like` callers | `helpers.rs` | 1400-1425 | String, Data (with LLVM type string) |

### Cluster E: GLUE/web FFI (~15 sites)
These generate marshalling code for cross-language FFI bridges.

| Function | File | Lines | Types matched |
|----------|------|-------|---------------|
| `param_name_from_type()` | `web_generator.rs` | 488-498 | Ptr, String, Int, Float, Bool, Element, CanvasContext |
| `frgn_marshal_in()` | `web_generator.rs` | 509-521 | String, Bool, Element, CanvasContext |
| `frgn_marshal_out()` | `web_generator.rs` | 531-544 | Element, CanvasContext, String, Bool |
| `format_type()` | `export.rs` | 199-209 | Int, Float, Bool, Char, String, Data |

---

## Phase 0 — Fix metadata semantics in the normalizer

**File:** `src/backend/llvm/normalizer.rs`, lines 155-267

**Problem:** The normalizer reads all metadata keys (`bits`, `maxbits`,
`minbits`) into a single `bytes` value, then sets `min_bits = max_bits =
bytes * 8`. This conflates the semantics:

| Metadata | Intended | Current normalizer |
|----------|----------|-------------------|
| `bits <~ 32` | exact: min=32, max=32 | min=32, max=32 ✅ |
| `maxbits <~ 32` | ceiling: min=0, max=32 | min=32, max=32 ❌ |
| `minbits <~ 64` | floor: min=64, max=primordial | min=64, max=64 ❌ |
| (none) | primordial values | min=64, max=64 ❌ |

**Fix:** Read each metadata key independently in `register_typedefs`:

```rust
let primordial = universe.get(&td.name);
let prim_min = primordial.map(|p| p.min_bits).unwrap_or(0);
let prim_max = primordial.map(|p| p.max_bits).unwrap_or(64);

let exact_bits = td.body.metadata.get("bits")
    .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
let ceiling = td.body.metadata.get("maxbits")
    .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
let floor = td.body.metadata.get("minbits")
    .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });

let (min_bits, max_bits) = if let Some(bits) = exact_bits {
    (bits, bits)
} else if let Some(fp) = td.body.metadata.get("minbits") {
    // minbits with no maxbits: floor only, max from primordial
    let f = floor.unwrap_or(prim_min);
    (f, prim_max.max(f))
} else {
    (floor.unwrap_or(prim_min), ceiling.unwrap_or(prim_max).max(floor.unwrap_or(0)))
};

// bytes is derived from max_bits for simple types, or from slots/layout for compound types.
// If slots or layout provides a larger size, that wins — but min_bits/max_bits stay.
let bytes = max_bits.div_ceil(8);
```

**Remove** the primordial-preservation code from the 2026-07-26 commit (the
`if let Some(prim) = universe.get(&td.name)` block) — it's now redundant
because primordial values are read directly as defaults.

**Impact on `push_field_type`:**
- `bits <~ 32` → exact (min=max=32) → native `"i32"` in %State
- `maxbits <~ 32` → ceiling (min=0, max=32) → flexible → `"i64"` in %State
- `minbits <~ 64` → floor (min=64, max=64 for Int) → exact → native `"i64"` in %State
- (none) → primordial Float → `"float"` in %State (min=32, max=32, exact)

---

## Phase 1 — Remove the narrowing pass

**Delete:** `src/optimizer/narrow_int.rs` (268 lines).

**Remove module declaration:** `src/optimizer/mod.rs` line 1: `pub mod narrow_int`.

**Remove from compile.rs:**
- Lines 458-462: Remove the `narrow_types()` call and its result variable
- Remove `narrow_bindings` parameter from `codegen()` call (line 568)
- Remove `narrow_bindings` parameter from `codegen()` function signature (line 873)
- Remove `.with_narrow_bindings(narrow_bindings)` from all three backend invocations (lines 901, 940, 959)

**Remove from context.rs:**
- `CompilerContext.narrow_bindings` field (line 132) and its initialization (line 249)
- `FunctionContext.narrowed` field (line 306) and its initialization (line 585)

**Remove from mod.rs:**
- Doc comment reference to narrowing (line 489)
- `with_narrow_bindings()` builder method (lines 1074-1077)

**Simplify emit_toplevel.rs:**
- Lines 1129-1131: Remove `self.fun.narrowed = ...` loading for definitions
- Lines 1341-1343: Remove `self.fun.narrowed = ...` loading for transactions
- Lines 301-328 (`llvm_type()` function): Remove the entire narrowing branching for Int/UInt. The `Int`/`UInt` match arm is deleted — the type falls through to the standard universe lookup, which reads the primordial `llvm_type` property and returns `"i64"`.

**Simplify emit_toplevel.rs — trigger-type warning (line 2253):**
The trigger-type guard `Type::Custom(__t) if __t == "Bool" || ...` uses
hardcoded name matching. This is also addressed in Phase 2b (dispatch
refactoring), but the narrowing branch removal is independent.

**Remove dead truncation code:**
- `emit_stmt.rs:215-219`: Remove the `trunc i64 to iN` for narrowed return types in `Statement::Term` — with no narrowing, `fn_ret_ty` is always `"i64"` for Int/UInt
- `emit_stmt.rs:249-254`: Remove the trunc code for narrowed returns in `Statement::Return` — same reasoning

**Remove dead ret_ty override:**
- `emit_expr.rs:1841-1845`: Remove the `ret_ty` override based on `int_ty != "i64"` — never triggered because `binop_int_type()` always returns `"i64"`

**Clean up comments:**
- `emit_expr.rs:1916-1966`: Remove "narrowing pass controls width" comments

**Keep:**
- `binop_int_type()` (emit_expr.rs:1699-1706) — already returns `"i64"`, no change
- `adapt_to_i64()` widening — still needed for exact types (Int32 → i32)
- counter `sext` in `counter.rs:404-412` — still needed for exact int counters
- `min_bits`/`max_bits` on `ResolvedType` — still the foundation of protocol+maxbits

---

## Phase 2a — Refactor core type-to-LLVM mapping

### 2a.1 `protocol_llvm_type()` — `mod.rs:496-523`

**Current:** 20-line name-match table mapping "Int"→"i64", "Float"→"float", etc.
**Fix:** Make it universe-driven, accepting an optional `&TypeUniverse`.

```rust
/// 2026-07-26: Returns the LLVM type for a Brief type, driven by the
/// type's primordial llvm_type property + maxbits. No name matching.
pub fn protocol_llvm_type(ty: &Type, universe: Option<&TypeUniverse>) -> String {
    if let Some(ref u) = universe {
        if let Some(rt) = ty.universe_key().and_then(|k| u.get(k)) {
            if let Some(PropertyValue::String(s)) = rt.properties.get("llvm_type") {
                // For #Float protocol types, width is driven by maxbits.
                if matches!(s.as_str(), "half" | "float" | "double" | "bfloat" | "fp128" | "x86_fp80") {
                    if rt.max_bits <= 32 { return "float".to_string(); }
                    if rt.max_bits <= 64 { return "double".to_string(); }
                    return "i64".to_string();  // fp128 stored lossily
                }
                return s.clone();
            }
        }
    }
    // Fallback for pre-normalization contexts: match known primitive names.
    match ty {
        Type::Ptr(_) => "ptr".to_string(),
        Type::Custom(s) => match s.as_str() {
            "Float64" | "Double" => "double",
            "Float" | "Float32" | "Half" => "float",
            "Int8" | "UInt8" | "Bool" => "i8",
            "Int16" | "UInt16" => "i16",
            "Int32" | "UInt32" | "Char" => "i32",
            "Int64" | "UInt64" | "Int" | "UInt" => "i64",
            "String" | "Data" | "Bytes" => "ptr",
            _ => "i64",
        },
        _ => "i64",
    }
}
```

This preserves a fallback for pre-normalization contexts (where the universe
isn't populated yet) but uses the universe-driven path whenever possible.

**Update all callers** — there are ~30 call sites. Most already have access
to a universe reference (via `self.ctx.type_universe` or similar). The call
changes from `protocol_llvm_type(&ty)` to
`protocol_llvm_type(&ty, self.ctx.type_universe.as_ref())`.

### 2a.2 `lower_custom_type()` — `types.rs:29-43`

**Current:** Name-match table duplicating `protocol_llvm_type()`.
**Fix:** Delegate to `protocol_llvm_type()`:

```rust
fn lower_custom_type(name: &str, universe: Option<&TypeUniverse>) -> String {
    protocol_llvm_type(&Type::Custom(name.to_string()), universe)
}
```

### 2a.3 `type_size()` — `types.rs:46-70`

**Current:** Name-match table mapping type names to byte sizes.
**Fix:** Compute from `ResolvedType.bytes` via universe, or from
`protocol_llvm_type()` → `llvm_type_byte_size()`:

```rust
fn type_size(name: &str, universe: Option<&TypeUniverse>) -> u64 {
    if let Some(ref u) = universe {
        if let Some(rt) = u.get(name) {
            return rt.bytes;
        }
    }
    // Fallback: derive from LLVM type
    let llvm_ty = lower_custom_type(name, None);
    llvm_type_byte_size(&llvm_ty) as u64
}
```

### 2a.4 `fallback_llvm_type()` — `emit_toplevel.rs:223-242`

**Current:** Name-match table duplicating `protocol_llvm_type()`.
**Fix:** Delegate to `protocol_llvm_type()`:

```rust
fn fallback_llvm_type(ty: &Type) -> String {
    protocol_llvm_type(ty, None)
}
```

### 2a.5 Constant emission (×3) — `mod.rs:2278-2337`

**Current:** Three nearly-identical name-match blocks that emit LLVM IR for
constant values of different types.
**Fix:** Each block calls `protocol_llvm_type()` with the universe:

```rust
let llvm_ty = protocol_llvm_type(ty, self.ctx.type_universe.as_ref());
match llvm_ty.as_str() {
    "i64" => { /* i64 constant */ }
    "i32" => { /* i32 constant */ }
    "i8" => { /* i8 constant */ }
    "float" => { /* float constant */ }
    "double" => { /* double constant */ }
    _ => { /* fallback */ }
}
```

Note: this still has LLVM-type matching (`"i64"`, `"float"`, etc.), which is
correct — these are LLVM IR types, not Brief type names. The dispatch is on
the LLVM representation, which is what the emission needs.

### 2a.6 Update `llvm_type()` in `emit_toplevel.rs`

After removing the narrowing branch (Phase 1), `llvm_type()` for `Int`/`UInt`
falls through to the main function body which already handles universe
lookup. No additional change needed for this function.

---

## Phase 2b — Refactor type-category dispatch

### 2b.1 Add protocol membership helper

Add a method on `LlvmBackend`:

```rust
/// 2026-07-26: Check if a type implements a protocol by looking for
/// Cast.<protocol> in its ResolvedType properties. No name matching.
fn is_protocol_member(&self, ty: &Type, protocol: &str) -> bool {
    let prop_key = if protocol.starts_with('#') {
        format!("Cast.{}", protocol)
    } else {
        format!("Cast.#{}", protocol)
    };
    self.ctx.type_universe.as_ref()
        .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
        .map(|rt| rt.properties.contains_key(&prop_key))
        .unwrap_or(false)
}
```

Also add a static version for contexts without `&self`:

```rust
fn is_protocol_member_static(universe: &Option<TypeUniverse>, ty: &Type, protocol: &str) -> bool {
    // same logic
}
```

### 2b.2 Replace `type_is()` callers

**Delete** the `type_is()` function entirely (helpers.rs:39-51).

Replace each call site:

| Before | After |
|--------|-------|
| `type_is(&self.ctx.type_universe, &a.ty, "Float")` | `self.is_protocol_member(&a.ty, "#Float")` |
| `type_is(&self.ctx.type_universe, &reg.ty, "Int")` | `self.is_protocol_member(&reg.ty, "#Int")` |
| `type_is(&self.ctx.type_universe, &reg.ty, "Bool")` | `self.is_protocol_member(&reg.ty, "#Bool")` |
| `type_is(&self.ctx.type_universe, &reg.ty, "String")` | `self.is_protocol_member(&reg.ty, "#String")` |
| `type_is(&self.ctx.type_universe, &reg.ty, "Data")` | `self.is_protocol_member(&reg.ty, "#Data")` |

### 2b.3 Refactor `is_native_float()` — `helpers.rs:1641-1659`

**Current:** Checks `category` property (protocol-driven) with fallback to
`type_is("Float" || "Float64")` (name-based).
**Fix:** Remove the name-based fallback. The protocol check is sufficient:

```rust
fn is_native_float(&self, ty: &Type) -> bool {
    self.ctx.type_universe.as_ref()
        .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
        .map(|rt| {
            rt.properties.get("category")
                .and_then(|pv| match pv {
                    PropertyValue::String(s) => Some(s == "Float"),
                    _ => None,
                })
                .unwrap_or(false)
                || rt.properties.contains_key("Cast.#Float")
        })
        .unwrap_or(false)
}
```

The fallback `|| rt.properties.contains_key("Cast.#Float")` handles types
where the normalizer set `Cast.#Float` but not the `category` property.

### 2b.4 Refactor `emit_fcmp()` — `helpers.rs:1906-1931`

**Current:** `type_is(..., "Float")` to decide between `fcmp` and `icmp`.
**Fix:** `self.is_protocol_member(&a.ty, "#Float")`.

### 2b.5 Refactor `emit_typed_cast()` — `helpers.rs:573-609`

**Current:** Matches `("Int" | "UInt", "Float")` etc. by name.
**Fix:** Check protocol membership for each source/target type pair:

```rust
let src_proto = self.protocol_category(&src_ty);
let tgt_proto = self.protocol_category(&tgt_ty);
match (src_proto.as_deref(), tgt_proto.as_deref()) {
    (Some("#Int") | Some("#UInt"), Some("#Float")) => { /* sitofp */ }
    (Some("#Float"), Some("#Int") | Some("#UInt")) => { /* fptosi */ }
    ...
}
```

Where `protocol_category()` returns the hashword-category from the type's
properties (e.g., `"#Int"`, `"#Float"`, `"#Bool"`).

### 2b.6 Refactor `emit_projection_fast_path()` — `helpers.rs:2072-2075`

**Current:** Dispatches on `"Int"`, `"Float"`, `"Bool"` by name.
**Fix:** Use `self.is_protocol_member()`:

```rust
if self.is_protocol_member(ty, "#Float") {
    self.projection_float_fast_path(...)
} else if self.is_protocol_member(ty, "#Bool") {
    self.projection_bool_fast_path(...)
} else if self.is_protocol_member(ty, "#Int") {
    self.projection_int_fast_path(...)
}
```

### 2b.7 Refactor `emit_trg_load_finish()` — `emit_toplevel.rs:559-579`

**Current:** Name-match on Bool, Int, UInt, Float, Char, String, Data.
**Fix:** Protocol membership:

```rust
if self.is_protocol_member(&rt.ty, "#Bool") { /* zext i8 to i64 */ }
else if self.is_protocol_member(&rt.ty, "#Int") { /* pass through */ }
else if self.is_protocol_member(&rt.ty, "#Float") { /* bitcast */ }
else if self.is_protocol_member(&rt.ty, "#Char") { /* zext i32 to i64 */ }
else if self.is_protocol_member(&rt.ty, "#String") { /* ptrtoint */ }
else if self.is_protocol_member(&rt.ty, "#Data") { /* ptrtoint */ }
```

### 2b.8 Refactor defn/txn param boxing — `emit_toplevel.rs:1219-1259`

**Current:** 4× repeated name-match pattern for Bool/Char/String/Data/Float.
**Fix:** Protocol membership, same pattern as 2b.7:

```rust
// One handler for boxing parameters before defn/txn calls
fn box_param_to_i64(&self, ty: &Type, reg: &str) -> String {
    if self.is_protocol_member(ty, "#Bool") { ... zext i8 ... }
    else if self.is_protocol_member(ty, "#Char") { ... zext i32 ... }
    else if self.is_protocol_member(ty, "#String") { ... ptrtoint ... }
    else if self.is_protocol_member(ty, "#Float") { ... bitcast float to i32, zext to i64 ... }
    else { reg.to_string() }  // Int, Ptr: passthrough
}
```

This replaces the duplicated match blocks at lines 1219-1226, 1255, 1655-1662,
and 1697.

### 2b.9 Refactor `emit_trg_global_decl()` — `emit_toplevel.rs:2253`

**Current:** Name-match on Bool, Int, UInt, Char, String, Data to determine
which types are valid trigger inputs.
**Fix:** Check protocol membership or a config-driven "is_trigger_allowed"
property on the ResolvedType:

```rust
let supported = self.is_protocol_member(&decl.ty, "#Bool")
    || self.is_protocol_member(&decl.ty, "#Int")
    || self.is_protocol_member(&decl.ty, "#Char")
    || self.is_protocol_member(&decl.ty, "#String");
```

### 2b.10 Refactor `is_bool_type()` — `types.rs:80-86`

**Current:** `name == "Bool"` on both `Custom` and `Applied` variants.
**Fix:** Protocol membership check if universe is available; fallback to
name matching for pre-universe contexts:

```rust
fn is_bool_type(ty: &Type, universe: Option<&TypeUniverse>) -> bool {
    if let Some(ref u) = universe {
        return is_protocol_member_static(u, ty, "#Bool");
    }
    matches!(ty, Type::Custom(name) if name == "Bool")
}
```

---

## Phase 2c — Refactor i64 conversion / boxing

### 2c.1 `adapt_to_i64()` — `helpers.rs:2675-2739`

**Current:** Name-match on `"Float64"`, `"Float"`, `"Bool"`, `"String"`,
`"Data"`, `"Int"`, `"UInt"`.
**Fix:** Protocol membership + maxbits:

```rust
pub(crate) fn adapt_to_i64(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
    // Check #Float protocol: convert float/double to i64
    if self.is_protocol_member(&reg.ty, "#Float") {
        let maxbits = self.ctx.type_universe.as_ref()
            .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
            .map(|rt| rt.max_bits)
            .unwrap_or(32);
        if maxbits > 32 {
            // double → bitcast to i64
            let tr = self.fun.gen_reg();
            writeln!(out, "{}{} = bitcast double {} to i64", indent, tr, reg.name).ok();
            return tr;
        } else {
            // float → bitcast to i32 → zext to i64
            let tr = self.fun.gen_reg();
            writeln!(out, "{}{} = bitcast float {} to i32", indent, tr, reg.name).ok();
            let ze = self.fun.gen_reg();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, tr).ok();
            return ze;
        }
    }
    // Check #Bool protocol: zext i8 to i64
    if self.is_protocol_member(&reg.ty, "#Bool") {
        let tr = self.fun.gen_reg();
        writeln!(out, "{}{} = zext i8 {} to i64", indent, tr, reg.name).ok();
        return tr;
    }
    // Check #String/#Data protocol: ptrtoint or extractvalue
    if self.is_protocol_member(&reg.ty, "#String") || self.is_protocol_member(&reg.ty, "#Data") {
        if self.feature_sso_strings && self.ctx.type_universe.as_ref()
            .map_or(false, |u| u.is_string_like(&reg.ty))
        {
            let tr = self.fun.gen_reg();
            writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, tr, reg.name).ok();
            return tr;
        } else {
            let tr = self.fun.gen_reg();
            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, tr, reg.name).ok();
            return tr;
        }
    }
    // Check #Int protocol: widen if narrower than i64
    if self.is_protocol_member(&reg.ty, "#Int") {
        let llvm_ty = self.llvm_type(&reg.ty);
        if llvm_ty != "i64" && llvm_ty.starts_with('i') {
            let tr = self.fun.gen_reg();
            let is_unsigned = self.is_protocol_member(&reg.ty, "#UInt");
            if is_unsigned {
                writeln!(out, "{}{} = zext {} {} to i64", indent, tr, llvm_ty, reg.name).ok();
            } else {
                writeln!(out, "{}{} = sext {} {} to i64", indent, tr, llvm_ty, reg.name).ok();
            }
            return tr;
        }
    }
    // Ptr: already i64 (identity)
    reg.name.clone()
}
```

Note: The `Type::Ptr(_)` match is still justified — Ptr is a compiler
construct, not a user-defined type.

### 2c.2 `box_to_i64_fallback()` / `unbox_from_i64_fallback()` — `builder.rs:544-618`

Same pattern as 2c.1 — replace name matches with protocol membership.

### 2c.3 `ensure_typed_value()` — `helpers.rs:2881-2934`

**Current:** Matches on LLVM type pairs (`("double", "i64")`, `("i64", "float")`,
`("i8", "i64")`, `("i32", "i64")`). These are LLVM type conversions, not Brief
type name matches — the dispatch is on LLVM IR types. **No change needed.**

---

## Phase 2d — Refactor SSO/heap allocation heuristics

### 2d.1 `type_is_heap_allocated()` — `mod.rs:1604-1613`

**Current:** Name-match on UTF8View, StaticString, SmallString64, Data,
List, HashMap, HashSet, Stack, Queue, StringBuilder.
**Fix:** Use the already-existing universe methods `is_string_like()` and
`is_vector_like()`, plus a new `is_container_like()` for heap-allocated
types:

```rust
fn type_is_heap_allocated(&self, ty: &Type) -> bool {
    self.ctx.type_universe.as_ref().map_or(false, |u| {
        u.is_string_like(ty)
        || u.is_vector_like(ty)
        || ty.universe_key().and_then(|k| u.get(k))
            .map(|rt| {
                rt.properties.contains_key("Cast.#HeapAllocated")
                || rt.properties.get("storage").and_then(|pv| match pv {
                    PropertyValue::String(s) => Some(s == "heap"),
                    _ => None,
                }).unwrap_or(false)
            })
            .unwrap_or(false)
    })
}
```

### 2d.2 `push_field_type()` — `mod.rs:944`

**Current:** Name-match on `"UTF8View"`.
**Fix:** Use `is_string_like()`:

```rust
if self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty))
    || matches!(ty, Type::Custom(name) if name == "UTF8View")
```

Note: The fallback to name matching for `"UTF8View"` is needed for contexts
where the universe might not have the string-like property set yet. This
is justified as a bootstrap fallback.

---

## Phase 3 — Refactor GLUE/web FFI

### 3a. `web_generator.rs:param_name_from_type()` — lines 488-498

**Current:** Name-match on Ptr, String, Int, Float, Bool, Element, CanvasContext.
**Fix:** Read the type's protocol category and look up the corresponding
`wasm_abi` from the GLUE config:

```rust
fn param_name_from_type(&self, ty: &crate::ast::Type, idx: usize,
    glue_targets: &LanguageEntry) -> String
{
    if matches!(ty, Type::Ptr(_)) {
        return format!("ptr{}", idx);
    }
    if let Some(cat) = self.protocol_category(ty) {
        if let Some(entry) = glue_targets.protocols.get(cat) {
            return format!("{}_{}", entry.native, idx);
        }
    }
    format!("arg{}", idx)
}
```

Where `protocol_category()` returns the type's protocol hashword
(e.g., `"#Int"`, `"#String"`) by scanning `Cast.#*` properties:

```rust
fn protocol_category(&self, ty: &Type) -> Option<String> {
    self.ctx.type_universe.as_ref()
        .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
        .and_then(|rt| {
            rt.properties.keys()
                .find_map(|k| k.strip_prefix("Cast.#").map(|s| format!("#{}", s)))
        })
}
```

### 3b. `web_generator.rs:frgn_marshal_in()` — lines 509-521

**Current:** Name-match on String, Bool, Element, CanvasContext.
**Fix:** Protocol membership + config lookup:

```rust
fn frgn_marshal_in(&self, inputs: &[(String, ast::Type)], param_names: &[String],
    glue_targets: &LanguageEntry) -> Vec<String>
{
    let mut stmts = Vec::new();
    for (i, (_, ty)) in inputs.iter().enumerate() {
        let pn = &param_names[i];
        // String: read string from WASM memory
        if self.is_protocol_member(ty, "#String") {
            stmts.push(format!("const {} = this._readString({});", pn, pn));
        }
        // Bool: nonzero check
        else if self.is_protocol_member(ty, "#Bool") {
            stmts.push(format!("const {} = {} !== 0;", pn, pn));
        }
        // Handle types (Element, CanvasContext, etc.)
        else if self.is_protocol_member(ty, "#Handle") {
            stmts.push(format!("const {} = this._handles[{}];", pn, pn));
        }
        // Int, Float, Char: pass through as-is
    }
    stmts
}
```

### 3c. `web_generator.rs:frgn_marshal_out()` — lines 531-544

**Current:** Name-match on Element, CanvasContext, String, Bool.
**Fix:** Same pattern as 3b.

### 3d. `export.rs:format_type()` — lines 199-209

**Current:** Name-match on Int, Float, Bool, Char, String, Data.
**Fix:** Universe-driven:

```rust
fn format_type(ty: &crate::ast::Type, universe: Option<&TypeUniverse>) -> String {
    if let Some(ref u) = universe {
        if let Some(rt) = ty.universe_key().and_then(|k| u.get(k)) {
            // Use explicit export_name property if set, otherwise the type's own name
            return rt.properties.get("export_name")
                .and_then(|pv| match pv { PropertyValue::String(s) => Some(s.clone()), _ => None })
                .unwrap_or_else(|| rt.name.clone());
        }
    }
    // Fallback for pre-universe contexts
    match ty {
        Type::Custom(name) => name.clone(),
        _ => format!("{:?}", ty),
    }
}
```

---

## Phase 4 — Clean up test code and dead backends

### 4.1 Test code

The ~80 test sites that use `Type::int()`, `Type::float()`, etc. for
constructing test ASTs are fine — they're constructing data, not dispatching.
**No change needed.**

### 4.2 Dead backends (verilog.rs, vhdl.rs, c.rs, rust.rs, cobol.rs,
x86_64.rs, aarch64.rs, wasm.rs, tcl_generator.rs)

Per AGENTS.md: "Do not modify for any reason." If a shared API change
mechanically breaks a dead backend, use `#[allow(unused_variables)]`,
`_ => {}`, or `todo!()` with a comment `// dead backend`.
**No change needed.**

---

## Files changed (complete list)

| Phase | File | Change type | Lines |
|-------|------|-------------|-------|
| 0 | `src/backend/llvm/normalizer.rs` | Modify | 162-224 |
| 1 | `src/optimizer/narrow_int.rs` | **Delete** | 1-268 |
| 1 | `src/optimizer/mod.rs` | Modify | 1 |
| 1 | `src/compile.rs` | Modify | 458-462, 568, 873, 901, 940, 959 |
| 1 | `src/backend/llvm/context.rs` | Modify | 132, 249, 306, 585 |
| 1 | `src/backend/llvm/mod.rs` | Modify | 489, 1074-1077 |
| 1 | `src/backend/llvm/emit_toplevel.rs` | Modify | 301-328, 1129-1131, 1341-1343 |
| 1 | `src/backend/llvm/emit_stmt.rs` | Modify | 215-219, 249-254 |
| 1 | `src/backend/llvm/emit_expr.rs` | Modify | 1841-1845 |
| 2a | `src/backend/llvm/mod.rs` | Modify | 496-523 |
| 2a | `src/backend/llvm/types.rs` | Modify | 29-70 |
| 2a | `src/backend/llvm/emit_toplevel.rs` | Modify | 223-242 |
| 2b | `src/backend/llvm/helpers.rs` | Modify | 39-51, 573-609, 734-838, 1641-1659, 1906-1931, 2072-2075 |
| 2b | `src/backend/llvm/emit_toplevel.rs` | Modify | 559-579, 1219-1259, 1648-1701, 2253 |
| 2b | `src/backend/llvm/types.rs` | Modify | 80-86 |
| 2c | `src/backend/llvm/helpers.rs` | Modify | 2675-2739 |
| 2c | `src/backend/llvm/builder.rs` | Modify | 544-618 |
| 2d | `src/backend/llvm/mod.rs` | Modify | 944, 1604-1613 |
| 3a | `src/glue/web_generator.rs` | Modify | 488-548 |
| 3b | `src/glue/export.rs` | Modify | 199-209 |

---

## Verification

1. `cargo build` after each phase — zero new warnings
2. `cargo test --lib` — all 1035 tests pass
3. `bash benchmarks/build_and_bench.sh --runtime` — all benchmarks MATCH
4. `git grep 'Type::Custom.*==.*"' src/backend/llvm src/glue` — zero results
5. `type_is` function is deleted
