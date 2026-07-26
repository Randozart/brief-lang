# SSO String Handle — Full Compiler Migration

**Date:** 2026-07-18
**Status:** Plan
**See also:**
  - `docs/plans/2026-07-18-allocation-strategy-system.md` (Alloc# intrinsic, arena types)
  - `docs/plans/2026-07-18-string-encoding-alloc-and-provenance.md` (encoding registry, fat pointer provenance)
  - `docs/plans/2026-07-18-phase4-execution-graph-strategy-selection.md` (analysis pass, now committed)

---

## Executive Summary

String is currently a `Type::Custom("String")` with ~40 hardcoded `"String"` match arms across the compiler. The LLVM type is `i8*`, the state field is 1 × `i64` (a ptrtoint'd pointer), and every string header is heap-allocated (16 bytes header + `len` bytes data + null terminator).

**Two changes, one migration:**

### A. Type system: struct_fields on ResolvedType + is_string_like

Add `fields: Vec<(String, Type)>` to `ResolvedType`. String becomes a 2-field struct (`data: Int, len: Int`) identified by shape+encoding rather than name. The ~40 hardcoded `"String"` match arms are replaced by a single `is_string_like()` helper:

```rust
fn is_string_like(ty: &Type, universe: &TypeUniverse) -> bool {
    let rt = match ty { Type::Custom(n) => universe.types.get(n), _ => None };
    let Some(rt) = rt else { return false; };
    rt.fields.len() == 2
        && rt.fields[0].1 == Type::int()
        && rt.fields[1].1 == Type::int()
        && rt.properties.contains_key("encoding")
}
```

A user-defined `type MyString { data: Int; len: Int; encoding <~ "UTF-8"... }` gets the same SSO treatment as `type String` — no compiler changes needed.

### B. SSO handle: 6 bytes inline, heap otherwise

The handle is `{ i64, i64 }`:
- **SSO** (≤6 bytes): field[0] = tag(3 bits) | data(48 bits) + 13 zero bits. field[1] = len.
- **Heap** (>6 bytes): field[0] = tag(3 bits) | ptr(61 bits). field[1] = len.
- **Heap layout changes**: no more 16-byte header. Just raw bytes + null terminator at the pointer. Length lives in handle[1].
- **Tag scheme**: lower 3 bits of field[0]. 001 = SSO, 010 = static, 100 = temp, 000 = heap.

| Metric | Current | SSO |
|--------|---------|-----|
| Handle width | 1 × i64 | 2 × i64 |
| SSO capacity | 0 bytes | 6 bytes inline |
| Heap header | 16 bytes | 0 bytes |
| State field | 1 × i64 | 2 × i64 |
| Function ABI | 1 register | 2 registers |

---

## Files

### New files

| File | Purpose |
|------|---------|
| None | All changes are modifications |

### Modified files (complete list)

| File | Phase | What changes |
|------|-------|-------------|
| `src/ast/types.rs` | A | Add `ResolvedType.fields` struct field |
| `src/type_universe/mod.rs` | A | Add `fields` to `ResolvedType`, update seed table for String primordial |
| `src/type_universe/resolve.rs` | A | `resolve_type` preserves `fields` from universe lookup |
| `src/backend/llvm/normalizer.rs` | A | `register_typedefs` populates `fields` from TypeDef.slots instead of flattening to `slot.*` properties |
| `src/backend/llvm/types.rs` | B | `lower_custom_type` reads `rt.fields` → LLVM struct type |
| `src/backend/llvm/mod.rs` | A | Remove hardcoded `"String"` from `trg_llvm_storage_ty`, `llvm_type()` |
| `src/backend/llvm/mod.rs:1632-1639` | A | Replace `ctx.struct_types` population with read from TypeUniverse |
| `src/backend/llvm/mod.rs:822-831` | B | `push_field_type` generic: pushes N × i64 from `fields.len()` |
| `src/backend/llvm/helpers.rs` | B | `adapt_to_i64` → struct extractvalue. `is_string_chain` → `is_string_like`. |
| `src/backend/llvm/helpers.rs:749-773` | B | `emit_inline_concat` — SSO path for ≤6 byte total |
| `src/backend/llvm/helpers.rs:919-944` | B | `emit_free_temporaries` — tag check updated (lower 3 bits, AND -8) |
| `src/backend/llvm/emit_expr.rs:148-150` | B | String literal emission: SSO (shl+or) vs heap |
| `src/backend/llvm/emit_toplevel.rs` | B | `emit_fn_body` — String param boxing. `declare_state_type` — auto from fields. |
| `src/backend/llvm/intrinsics.rs` | B | `emit_len` — reads handle[1]. `emit_alloc` for string buffers. |
| `src/backend/llvm/intrinsics.rs:489-500` | B | Length# — extract handle[1] for string-like types with encoding property |
| `src/backend/llvm/mod.rs:2023-2035` | B | String globals: >6 bytes keep existing format, ≤6 use SSO literal |
| `src/backend/llvm/mod.rs:477-485` | B | `trg_llvm_storage_ty` — generic via fields |
| `src/compile.rs` | B | Add `--feature sso-strings` flag |
| `src/interpreter/intrinsics.rs` | C | String representation |
| `src/interpreter/value.rs` | C | String in interpreter |
| `lib/runtime/brief_rt.c` | C | `brief_str_to_c` — handle SSO + new heap layout |
| `lib/std/types/bootstrap.bv` | B | Update String type (fields, bytes=16, no llvm) |
| All `examples/*.bv` | B | Verify compilation with new String layout |
| All `benchmarks/*.bv` | B | Verify correctness, re-run baseline |

---

## Prerequisite: Op Dispatch Architecture

Before touching String layout, the op dispatch must be cleaned up. Currently there are **three disconnected systems** doing the same thing:

| System | What it does | How it stores the mapping |
|--------|-------------|---------------------------|
| `bootstrap.bv` `op Add(Int) -> Int = "add nsw"` | Declares operator implementations | Property bag `"op.Add" => "add nsw"` — **never consumed** |
| `builtin_operator_binding()` (operators.rs) | Validates ops during typechecking | Hardcoded Rust match: `("Int", "Add") => "AddI64#"` |
| `emit_expr.rs:emit_binop` | Emits LLVM IR for binary ops | Hardcoded match `BinaryOpKind::Add => "add nsw i64 %a, %b"` |

None of them communicate. The property bag strings are dead code. The typechecker has a separate hardcoded table from the backend.

### Target architecture

**bootstrap.bv** uses generic backend-agnostic op identifiers:

```brief
type Int : Bits {
    op Add(Int) -> Int = "int.add";
    op Sub(Int) -> Int = "int.sub";
    op Eq(Int) -> Bool = "int.eq";
    // ...
};
```

The op declaration is parsed into a dedicated `ops: HashMap<String, String>` on `OpDeclaration` and `ResolvedType` — NOT stored in the property bag. The generic identifier `"int.add"` is backend-agnostic and maps to concrete IR via `config/llvm-ops.toml`:

```toml
["int.add".Int]
8 = "add nsw {L}, {R}"
```

This approach:

| Property | Before | After |
|----------|--------|-------|
| Generic across backends | No — LLVM syntax in .bv files | Yes — .bv has backend-agnostic `"int.add"` |
| Typechecker source of truth | Hardcoded Rust table | Universe `ops` declarations |
| Backend dispatch | Hardcoded `BinaryOpKind` matches | `llvm-ops.toml` lookup by generic identifier |
| bootstrap.bv properties | `"op.Add" => "add nsw"` (dead code) | `ops: HashMap` (live) |
| `builtin_operator_binding` | Required for typechecking | Removed — universe drives all |
| Config overridability | None — baked into bootstrap.bv | Full — `config/llvm-ops.toml` per backend |

### Phase 0: Op dispatch cleanup — changes

| File | Change |
|------|--------|
| `src/ast/top.rs` | Add `ops: HashMap<String, OpDecl>` to `TypeDefBody` (separate from `metadata`) |
| `src/ast/types.rs` | Add `OpDecl { name, params, result, generic_id }` type. Add `ops: HashMap<String, String>` to `ResolvedType`. |
| `src/ast/expr.rs` | Remove old-style `Expr::Add`, `Expr::Mul` etc. (already unnormalized?) — verify only `BinaryOp`/`UnaryOp` exist |
| `src/parser/definitions.rs` | Parse `op Add(Int) -> Int = "int.add"` into `OpDecl` with `generic_id: "int.add"`, not into `metadata["op.Add"]` |
| `src/type_universe/mod.rs` | Add `ops: HashMap<String, String>` to `ResolvedType` (op_name → generic_identifier) |
| `src/type_universe/operators.rs` | `get_operator_intrinsic` reads from `rt.ops` table. `builtin_operator_binding` removed — universe drives all dispatch |
| `src/typechecker/mod.rs` | Replace `builtin_operator_binding` call with `get_operator_intrinsic` universe lookup |
| `src/backend/llvm/emit_expr.rs` | `emit_binop` looks up generic identifier in `type_universe`, maps to LLVM IR via `OpConfig::lookup(op_id, type_name, bytes)` |
| `src/backend/llvm/normalizer.rs` | Update `keep` set: remove `slot.*` (replaced by `fields`), remove `op.*` (replaced by `ops` table). Keep `encoding` for `is_string_like`. |
| `config/llvm-ops.toml` | Restructure keys from `"op.Add"` to `"int.add"`, include per-type+bytes templates |
| `lib/std/types/bootstrap.bv` | Change all `op ... = "llvm syntax"` to `op ... = "generic.id"` |
| `lib/std/types/*.bv` | Same migration for all type-specific ops |

### Phase 0 migration strategy (additive to avoid regressions)

This is a large refactoring. To keep tests passing at each step:

1. **Add `ops` field** to `ResolvedType` (parallel to `properties`). TypeDefBody gains `ops: Vec<OpDecl>`.
2. **Parser writes both**: `metadata["op.Add"] = "int.add"` (for backward compat) AND `ops["Add"] = OpDecl { generic_id: "int.add", ... }` (new structure).
3. **`get_operator_intrinsic` reads from `ops`** first, falls back to `properties["op.Add"]`.
4. **Typechecker switches** from `builtin_operator_binding` to `get_operator_intrinsic`.
5. **Backend switches** from `BinaryOpKind` matches to `OpConfig::lookup(op_id, type_name, bytes)`.
6. **Remove** old `builtin_operator_binding`, `metadata["op.*"]`, `BinaryOpKind` hardcoded matches.
7. **Verify**: all tests pass at each step.

### Impact on SSO plan

The op cleanup is a prerequisite for phases B-D of the SSO migration because:
- The SSO string `Concat` and `Eq` ops need proper dispatch through the universe
- bootstrap.bv String type needs clean op declarations without LLVM syntax
- `is_string_like` replaces name-based dispatch; op dispatch is the parallel replacement for expression-based dispatch

Without this cleanup, the SSO migration would add yet another layer of ad-hoc dispatch on top of the existing three.

## Encoding Config Cleanup

Currently `encoding_registry.rs` has a split: PascalCase names (UTF8, ASCII, Latin1, etc.) are hardcoded with `char_width`, while quoted names fall through to `config/encodings.toml`. This is the same anti-pattern as the op dispatch — compiler knowledge of encoding semantics that belongs in stdlib.

### Target: all encodings are config-driven

```
encoding <~ "UTF-8"     → config/encodings.toml (char_width=0)
encoding <~ "shift_jis" → config/encodings.toml (char_width=0)
encoding <~ "ASCII"     → config/encodings.toml (char_width=1)
```

No more PascalCase hardcoded table. The config specifies how the compiler emits `Index#` and `Length#`:

```toml
[encoding.UTF-8]
char_width = 0
ops.index_at  = "std.encoding.UTF8.index_at"
ops.char_len  = "std.encoding.UTF8.char_count"

[encoding.ASCII]
char_width = 1
# No ops needed — fixed-width, compiler emits direct GEP
```

Dispatch logic in the compiler:

- `char_width > 0` → `Index#(s, i)` emits `GEP s, i * char_width` (O(1))
- `char_width == 0 + ops.index_at` → emits `call @std.encoding.UTF8.index_at(ptr %s, i64 %i)`
- `char_width == 0 + no ops` → delegate to runtime scan (conservative default)

`Length#` follows the same pattern: `char_len` op from config, or byte-length from handle[1].

### Changes

| File | Change |
|------|--------|
| `src/encoding_registry.rs` | Remove `hardcoded_encodings()`. All lookups go through config. Add `ops` map to `EncodingInfo`. |
| `config/encodings.toml` | Add UTF-8, ASCII, Latin1, UTF-16, UTF-32 entries. Each with `char_width` and optional `ops`. |
| `src/backend/llvm/helpers.rs` | `emit_len` — if `is_string_like` and encoding has `char_len` op, emit that call; else read handle[1] byte length. |
| `src/backend/llvm/intrinsics.rs` | `emit_index_at` — if `char_width > 0` emit GEP; if `char_width == 0` + op, emit call; else runtime loop. |
| `lib/std/types/bootstrap.bv` | String `encoding <~ "UTF-8"` (quoted, not PascalCase) |

### No intrinsic encodings

The compiler knows zero encoding semantics. Everything is:
1. `char_width` for SSO capacity and GEP eligibility
2. `ops.*` for stdlib function names to call at runtime

This aligns with the "stdlib is the extension mechanism" principle (Golden Rule 13).

---

## Implementation Plan

### Phase A: Type system foundation (struct_fields + is_string_like)

**Goal:** No behavioral change. ResolvedType gains `fields`, the primordial gets String with fields, all `"String"` match arms that can be driven by shape+encoding are replaced by `is_string_like()`. Everything still compiles to the same IR.

#### A1. Add fields to ResolvedType

```rust
// type_universe/mod.rs
pub struct ResolvedType {
    pub name: String,
    pub base: String,
    pub bytes: u64,
    pub alignment: u64,
    pub properties: HashMap<String, PropertyValue>,
    /// 2026-07-18: Struct field declarations. Populated from TypeDef.body.slots
    /// by the normalizer. For the String primordial, seeded with [("data", Int), ("len", Int)].
    /// Codegen uses this to determine LLVM struct type and ABI width.
    pub fields: Vec<(String, Type)>,
}
```

Seed the String primordial with fields:

```rust
// In seed_primordial_types(), add:
("String", 16, 8, "String", Some(vec![
    ("data".into(), Type::int()),
    ("len".into(), Type::int()),
])),
```

Add `encoding <~ "UTF-8"` to the String primordial's initial properties.

#### A2. Update normalizer

In `register_typedefs`, preserve `TypeDef.slots` as `ResolvedType.fields` instead of flattening to `slot.*` properties:

```rust
// Before:
for slot in &td.body.slots {
    properties.insert(format!("slot.{}", slot.name), PropertyValue::Identifier(slot.ty.to_string()));
}

// After:
let fields: Vec<(String, Type)> = td.body.slots.iter()
    .map(|s| (s.name.clone(), s.ty.clone()))
    .collect();
// slot.* properties no longer inserted — fields list replaces them
```

#### A3. Define is_string_like

```rust
// In helpers.rs or a shared location:
pub fn is_string_like(ty: &Type, universe: &TypeUniverse) -> bool {
    let rt = match ty {
        Type::Custom(n) => universe.types.get(n),
        _ => None,
    };
    let Some(rt) = rt else { return false; };
    if rt.fields.len() != 2 { return false; }
    if rt.fields[0].1 != Type::int() { return false; }
    if rt.fields[1].1 != Type::int() { return false; }
    rt.properties.contains_key("encoding")
}
```

#### A4. Replace hardcoded "String" matches with is_string_like

| Site | Current | Replacement |
|------|---------|-------------|
| `types.rs:lower_custom_type` | `"String" => "ptr"` | `rt.fields → "{ i64, i64 }"` |
| `mod.rs:trg_llvm_storage_ty` | `if t == "String" { "i8*" }` | `is_string_like → LLVM struct type` |
| `helpers.rs:adapt_to_i64` | `ptrtoint i8* %val to i64` | `extractvalue { i64, i64 }, 0` |
| `helpers.rs:is_string_chain` | `type_is(..., "String")` | `is_string_like` |
| `emit_toplevel.rs:fallback_llvm_type` | `"String" => "ptr"` | `is_string_like → struct type` |
| `emit_toplevel.rs:emit_fn_body` | `ptrtoint ptr` boxing | `extractvalue` from struct param |
| `intrinsics.rs:emit_len` | slot-0 load | read handle[1] for string-like |
| `memory_spec.rs` | `"String" => 24` | `fields → 16` |

Each site gets the `is_string_like` check instead of `name == "String"`. **Flat control flow**: guard clause, early return, fallthrough.

#### A5. Update LLVM backend struct_types

The existing `ctx.struct_types: HashMap<String, Vec<(String, Type)>>` (populated from TypeDef.slots at mod.rs:1632) becomes redundant — read from `TypeUniverse` instead. Keep it temporarily with a `// TEMP` comment, remove in Phase C.

#### A6. Update bootstrap.bv

```brief
type String {
    data: Int;
    len: Int;
    bytes <~ 16;
    alignment <~ 8;
    tbaa <~ "String";
};
```

Removed:
- `: Bits` (default)
- `ptr: Ptr<UInt8>` → `data: Int`
- `codec: UInt8` (encoding registry)
- `llvm <~ "%String"` (derived from fields)
- `bytes <~ 24` → `bytes <~ 16`

Note: `bootstrap.bv` sets `bytes` for memory accounting (memory_spec.rs reads this) and `tbaa` for alias analysis. The field declarations (`data: Int; len: Int;`) are informational (the primordial already carries them) — the normalizer reconciles the TypeDef with the primordial during register().

#### A7. Derive bytes from fields when present

Currently `bytes` is set explicitly on every type, even when it can be computed from field types. With `fields` now available, the compiler should derive `bytes` from fields when present, with explicit metadata as override.

Rules:
- **Struct types** (fields non-empty): `bytes = sum of each field's resolved byte size`. E.g. String with `[("data", Int), ("len", Int)]` → `bytes = 8 + 8 = 16`.
- **Scalar types** (fields empty): Use explicit `bytes` from metadata or primordial table.
- **Override**: Explicit `bytes <~ N` in metadata takes precedence over field-derived value.

Changes:

| Site | Change |
|------|--------|
| `seed_primordial_types()` | String primordial: derive `bytes` from `fields` (16) instead of hardcoded 24. Scalar primordials keep explicit `bytes`. |
| `register_typedefs` | After populating `fields`, compute `bytes` from field types if no explicit `bytes` metadata. Remove the `bytes` metadata lookup — it's an override, not the primary source. |
| `mod.rs:1613-1628` (Struct registration) | Already computes bytes from fields — keep as-is. |
| `memory_spec.rs:estimate_type_size` | Still hardcoded `"String" => 24` — update to read from universe (or remove special case, use generic field-based computation). |

This eliminates the redundant explicit `bytes` for struct-like types while keeping it for scalars where it's the only source of truth.

#### Test gate

```
cargo test --lib  → 918 pass (no behavior change)
```

### Phase B: SSO codegen (flag-gated)

Add `--feature sso-strings` to `BuildOptions`. When ON:

#### B1. LLVM type becomes { i64, i64 }

`lower_custom_type` already returns `{ i64, i64 }` for String (from Phase A4). When `--feature sso-strings` is OFF, override this back to `"ptr"` via the feature flag. When ON, let it flow.

#### B2. String literal emission

```rust
fn string_literal(backend, out, v, bytes, indent) -> TypedRegister {
    if !backend.feature_sso_strings { return emit_heap_literal(...); }
    if bytes.len() > 6 { return emit_heap_literal(...); }
    emit_sso_literal(...)  // shl + or, no heap
}
```

SSO literal: pack bytes into u64, `shl 3` + `or 0b001`. Handle[1] = `len`. Return `{ i64, i64 }` ssa value.

#### B3. State field layout

`push_field_type` reads `fields.len()` — for String it's 2, so pushes 2 × `i64`. The existing `field_brief_types` records `Type::Custom("String")` for both slots. State access automatically widens to 2 slots.

#### B4. Function ABI

String params arrive as `{ i64, i64 }`. No boxing/unboxing needed — they're already structs. `extractvalue` to read fields, `insertvalue` to construct returns.

For `frgn` calls, String is still passed as `i8*` (C ABI compatibility). The feature flag gates a shim: extract `data` field (which is the pointer for heap-backed strings), `inttoptr` to `i8*`.

#### B5. Concat

```rust
fn concat(backend, out, a, b) -> TypedRegister {
    if !backend.feature_sso_strings { return heap_concat(a, b); }
    let total_len = a_len + b_len;
    if total_len <= 6 { return sso_concat(a, b); }
    heap_concat(a, b)  // no 16-byte header, just raw bytes + null
}
```

#### B6. Tag scheme

`emit_mask_tag` changes from `AND -4` (bits 0-1) to `AND -8` (bits 0-2). Check feature flag to select.

#### Test gate

```
cargo test --lib  → 918 pass with flag OFF
cargo test --lib  → 918 pass with flag ON  (new tests for SSO paths)
```

### Phase C: C runtime + interpreter

#### C1. C runtime

`brief_str_to_c` checks tag:
- `001` → extract 6 bytes from field[0] >> 3, malloc, copy
- `000/010/100` → mask tag, ptr = field[0] & ~7, len = field[1], memcpy from ptr

Heap layout change: no more 16-byte header. Pointer points directly to raw UTF-8. Length is in handle[1].

#### C2. Interpreter

String representation updates to 2-field handle. `Length#` reads field[1].

### Phase D: Flag removal

Set `--feature sso-strings` default to `true`. After stabilization, remove the flag and old code paths.

---

## Testing Strategy

### Phase A tests (structural, no behavior change)

| Test | What it asserts |
|------|----------------|
| `test_resolved_type_fields` | ResolvedType.fields is populated from TypeDef.slots |
| `test_string_primordial_has_fields` | String primordial has `[("data", Int), ("len", Int)]` |
| `test_is_string_like_string` | `is_string_like` returns true for `Type::Custom("String")` |
| `test_is_string_like_custom` | `is_string_like` returns true for user type with same shape+encoding |
| `test_lower_string_to_struct` | With struct fields, `lower_custom_type` returns `{ i64, i64 }` |

### Phase B tests (SSO paths)

| Test | What it asserts |
|------|----------------|
| `test_sso_literal_short` | ≤6 byte literal → `shl`+`or`, no `@malloc` |
| `test_sso_literal_long` | >6 byte literal → heap allocation |
| `test_sso_state_2slot` | String state field → 2 × i64 slots |
| `test_sso_concat_short_short` | "abc"+"def" ≤6 total → SSO result |
| `test_sso_concat_short_long` | "a"+"hello world" >6 → heap |
| `test_sso_empty_string` | "" → SSO with len=0 |
| `test_sso_tag_mask` | `AND -8` instead of `AND -4` |
| `test_sso_all_918_pass_on` | All existing tests pass with `--feature sso-strings` |

### Phase C tests (C runtime + interpreter)

| Test | What it asserts |
|------|----------------|
| `test_c_runtime_sso` | `brief_str_to_c` handles SSO handle |
| `test_c_runtime_heap` | `brief_str_to_c` handles heap handle (new no-header layout) |
| `test_interpreter_string` | Interpreter round-trip with new handle format |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **FFI breakage** — `frgn fn(s: String)` changes ABI | Certain | High | Keep `i8*` at C boundary with feature-flag shim. Old binaries won't link — recompile required. |
| **String equality** breaks from spare bits | Medium | High | SSO construction must zero bits 51-63 of field[0]. Enforce in `emit_sso_literal`. |
| **Concat SSO path** misses common case | Low | Low | Falls through to heap concat — correct but suboptimal. |
| **~40 is_string_like replacements** miss a site | Medium | Medium | Coverage: compile all examples after Phase A. Any missed site won't compile (type mismatch). |
| **bootstrap.bv change** breaks existing programs | Low | High | `type String` in bootstrap.bv is the canonical definition. Programs importing `std/types` get the new layout. |
| **Interpreter divergence** from LLVM codegen | Medium | Medium | Interpreter is reference — fix it to match. |

---

## Documentation

### Inline doc comments

| Site | Comment |
|------|---------|
| `ResolvedType.fields` | `// 2026-07-18: Struct field layout. Drives LLVM type lowering, state field width, ABI.` |
| `is_string_like()` | `// 2026-07-18: Shape+encoding check replaces ~40 hardcoded "String" matches.` |
| `seed_primordial_types` String entry | `// 2026-07-18: String is a 2-field struct ([data, len]) with encoding property.` |
| `emit_sso_literal` | `// 2026-07-18: SSO handle — ≤6 bytes inline, tag=0b001 in lower 3 bits.` |
| `emit_mask_tag` | `// 2026-07-18: Tag scheme — lower 3 bits. AND -8 to mask.` |

### Architecture docs

| Document | What changes |
|----------|-------------|
| `docs/architecture/features/string-encoding-and-fat-pointer.md` | Full rewrite: SSO layout, struct_fields-driven lowering, is_string_like, heap simplification |
