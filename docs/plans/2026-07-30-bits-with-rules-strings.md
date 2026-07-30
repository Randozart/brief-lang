# Bits with Rules: String as Bit-Derived Protocol Type

**Date**: 2026-07-30
**Author**: Agent
**Status**: Plan (ready for implementation)

---

## Abstract

This plan refactors the Brief compiler's string model from a struct-based fat-pointer
(`{data: Int, len: Int}` subscribing to `#String`) to a Bit-derivation model where
`String` IS raw bytes (Bit) with UTF-8 interpretation rules layered via the `#String`
protocol. It introduces `Slice<T>` as the canonical variable-length byte-view type,
replaces `UTF8View` entirely, and establishes the "read string as Int" fast path as
a first-class protocol operation.

---

## Background: The Bits Thesis

Every Brief type is ultimately a lens over raw bit layout. `Int` is 64 bits with
two's-complement semantics. `Float` is 32 bits with IEEE 754 semantics. `String`
should be N bytes with UTF-8 semantics — not a struct containing a pointer and
length, but the bytes themselves, where the pointer+length is an implementation
detail of the *container* that holds the bits.

### Before (current architecture)

```brief
type String: #String {
    data: Int;      // pointer to UTF-8 bytes
    len: Int;       // byte length
    !> alignment: 8;
    !> encoding: "UTF-8";
    prop Size: chars(#L);
    prop Bytes: byte_len(#L);
};
```

Problems:
- String is defined as a struct, not as a bit sequence
- `{data, len}` fat pointer is exposed as fields
- `is_string_like()` structural check (2 Int fields) is a fragile heuristic
- UTF8View duplicates the fat-pointer shape with a different name
- Name-based dispatch in 9+ Rust match arms (`name == "String"`)

### After (target architecture)

```brief
type String: #String Bit {
    op CastTo(#Bit) = string_get_content_bytes(#L);   // deref fat ptr → Slice<Bit>
    op CastTo(#Int) = string_parse_to_int(#L);         // semantic: "123" → 123
    op CastFrom(#Int) = string_from_int(#L);           // semantic: 123 → "123"
    prop Size: chars(#L);
    prop Bytes: byte_len(#L);
};
```

Key principles:
1. **Implicit Bit**: All types derive from Bit by default. `type String: #String`
   is sugar for `type String: Bit #String`.
2. **Physical vs semantic casts**: `CastTo(#Bit)` always returns literal memory
   bytes (via `Slice<Bit>`); `CastTo(#AnyOther)` is a semantic conversion.
3. **No symmetry required**: CastTo and CastFrom are independent; round-trip
   verification is opt-in.
4. **Shape derives LLVM**: Struct-like types get their LLVM type from field shapes,
   not from metadata or primordial entries.

---

## Architecture

### Two Cast Families

```
┌──────────────────────────────────────────────────────────────────┐
│                        Cast Resolution Pipeline                    │
│                                                                    │
│  Cast(expr, TargetType)                                           │
│    │                                                              │
│    ├── Physical path (TargetType is #Bit or Bit-derived):         │
│    │   → Emit literal memory bytes at the value's address         │
│    │   → For String: deref fat pointer, return Slice<Bit>         │
│    │   → For Int: reinterpret i64 register as 8 bytes             │
│    │                                                              │
│    └── Semantic path (TargetType is any other protocol):          │
│        → 1. Direct op Cast(Target) on source                      │
│        → 2. CastTo(#Cat) → CastFrom(#Cat) protocol path           │
│        → 3. Meld shuffle (structural bit remapping)               │
│        → 4. #Bits bitcast (ONLY when target IS #Bit)              │
│        → ✗ Error if no path found (no implicit #Bits fallback     │
│            for semantic casts)                                    │
└──────────────────────────────────────────────────────────────────┘
```

### String Representation (LLVM IR)

SSO enabled:
```
%String = type { i64, i64 }
  handle[0]: bits 0-2 = tag, bits 3-63 = inline data (≤6 bytes) or ptr
  handle[1]: byte length
```

SSO disabled:
```
%String = type ptr  (opaque pointer to heap buffer)
```

`CastTo(#Bit)` emission:
- **SSO short** (≤6 bytes): `lshr i64 %handle, 3` → `trunc i64 to i32` (or i8/i16)
- **SSO/legacy heap**: `and i64 %handle, -8` → `inttoptr to ptr` → dereference

### Slice<T> as Primordial Bootstrap Type

```brief
type Slice<T> {
    data: Ptr<T>;
    len: Int;
    prop Size: len;
};
```

- **LLVM type**: derived from field shapes → `{ ptr, i64 }`
- **No primordial entry needed** — shape derivation is automatic
- **No `llvm_type` metadata** — the `llvm_type()` function computes `{ ptr, i64 }`
  by recursively mapping field types
- **Replaces UTF8View entirely** — same fat-pointer shape, no separate type

### Zero-Cost Ptr Reinterpretation

Since LLVM 15+ uses opaque `ptr` everywhere, `Ptr<Bit>` and `Ptr<Int32>` are
the same LLVM type. Casting between them emits no instructions:

```brief
let content: Slice<Bit> = s.CastTo(#Bit);   // → %v = extractvalue {ptr,i64} %s, 0
let p32: Ptr<Int32> = content.data as Ptr<Int32>;  // → no LLVM instruction
let val: Int32 = load<Int32>(p32);  // → load i32, ptr %v
```

At LLVM IR:
```llvm
%ptr = extractvalue { ptr, i64 } %slice, 0
%val = load i32, ptr %ptr
```

Two instructions. The Ptr cast is zero cost.

---

## Implementation Plan (8 Phases)

### Phase 0: Inject `Cast.#Bit` When `base == "Bit"`

**Objective**: Make the implicit Bit derivation visible to the protocol graph.

**Rationale**: `type String: #String` already defaults to `base: "Bit"` in the
normalizer, but `Cast.#Bit` is not injected. Without it, `find_cast_path` cannot
find the `String → #Bits` edge through the property bag (only through the BFS
hardcoded injection at `layout_optimizer.rs:297-302`, which is a secondary
mechanism). Normalizing this makes the protocol graph self-consistent.

**Files**:
- `src/backend/llvm/normalizer.rs:336-368` — in the Cast.# injection loop,
  after injecting `Cast.#<Protocol>` from `td.protocol`, also inject `Cast.#Bit`
  when `base == "Bit"`.

**Changes** (normalizer.rs, after line 368):
```rust
// 2026-07-30: Also inject Cast.#Bit when base is Bit.
// Every type is implicitly Bit-derived; the protocol graph needs this edge
// so find_cast_path can resolve String → #Bits without the hardcoded BFS fallback.
if rt.base == "Bit" && !rt.properties.contains_key("Cast.#Bit") {
    rt.properties.insert(
        "Cast.#Bit".to_string(),
        PropertyValue::Bool(true),
    );
}
```

**Test**: After normalization, every type with `base: "Bit"` has `Cast.#Bit`
property set. `find_cast_path("String", "#Bits")` finds the String → #Bits edge
through the property bag, not just the BFS fallback.

**Verification**:
```rust
let universe = /* normalized */;
let rt = universe.get("String").unwrap();
assert!(rt.properties.contains_key("Cast.#Bit"));
```

---

### Phase 1: Add `Slice<T>` as Bootstrap Type + Struct Shape Derivation

**Objective**: Define `Slice<T>` in stdlib with zero metadata, derive LLVM type
from field shapes automatically.

**Rationale**: Adding `Type::Slice` as an AST variant would require changes to
every match arm over `Type`. Instead, `Slice<T>` is `Type::Applied("Slice", [T])`,
resolved through the universe like any generic. Its LLVM type is derived from
its fields, not from `llvm_type` metadata or primordial entries.

**Files**:
1. `lib/std/types/bootstrap.bv` — add Slice<T> declaration
2. `src/backend/llvm/emit_toplevel.rs` — add struct shape derivation in `llvm_type()`
3. `src/backend/llvm/helpers.rs` — ensure `is_protocol_member` handles Applied types
4. `src/analysis/layout_optimizer.rs` — ensure `find_cast_path` handles Slice<T>

#### Step 1.1: Bootstrap Declaration

`lib/std/types/bootstrap.bv`:
```brief
// 2026-07-30: Slice<T> — fat-pointer view over contiguous elements.
// LLVM type derived from field shapes: { ptr, i64 }.
// No llvm_type metadata, no primordial entry.
type Slice<T> {
    data: Ptr<T>;
    len: Int;
    prop Size: len;
};
```

#### Step 1.2: Struct Shape Derivation in `llvm_type()`

`src/backend/llvm/emit_toplevel.rs` — add before the universe query (between
current line 298 and line 303):

```rust
// 2026-07-30: Struct-like types derive LLVM type from field shapes.
// This handles Slice<T> (fields: { Ptr<T>, Int } → { ptr, i64 }),
// List<T>, and any future struct type without requiring llvm_type metadata
// or primordial entries.
if let Some(rt) = self.ctx.type_universe.as_ref()
    .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
{
    if !rt.fields.is_empty()
        && !rt.properties.contains_key("llvm_type")
    {
        let field_tys: Vec<String> = rt.fields.iter()
            .map(|(_, fty)| self.llvm_type(fty))
            .collect();
        return format!("{{ {} }}", field_tys.join(", "));
    }
}
```

**Crucial ordering**: This check must come AFTER the SSO/SVO/String/UTF8View
checks (which have explicit metadata or primordial `llvm_type`) but BEFORE the
universe fallback (`rt_llvm_type` → `fallback_llvm_type`). This ensures:
- String (has primordial `llvm_type: "{ i64, i64 }"`) keeps its SSO behavior
- UTF8View (has hardcoded check) keeps its fat-pointer behavior
- Slice<T> (no llvm_type, has fields) gets { ptr, i64 } from shape

#### Step 1.3: Handle Applied Types in `is_protocol_member`

`src/backend/llvm/helpers.rs` — `is_protocol_member()` currently uses `universe_key()`
which already handles `Applied("Slice", _)` → `Some("Slice")`. No change needed,
but verify with a test.

#### Step 1.4: `find_cast_path` for Applied Types

`src/analysis/layout_optimizer.rs` — `find_cast_path` uses `universe.get(&current)`
which already works with `"Slice"`. No change needed.

**Test**: A Brief program using `Slice<Bit>` compiles to correct LLVM IR:
```
%Slice = type { ptr, i64 }
```

---

### Phase 2: Split Physical vs Semantic Cast Paths

**Objective**: Unify the two cast pipelines (`Expr::Cast` hardcoded dispatch in
`emit_expr.rs` and `Cast#` protocol pipeline in `intrinsics.rs`) into one
consistent resolution with clear physical/semantic separation.

**Rationale**: Currently, `Expr::Cast` in `emit_expr.rs` uses hardcoded LLVM
type dispatch (lines 544-616) that bypasses the protocol pipeline entirely.
The `Cast#` intrinsic in `intrinsics.rs` has a 4-step protocol pipeline (lines
1005-1067). They produce different results for the same inputs. Unifying them
eliminates a class of bugs.

**Files**:
1. `src/backend/llvm/emit_expr.rs:544-616` — rewrite `Expr::Cast` handler
2. `src/backend/llvm/intrinsics.rs:1005-1067` — modify `emit_intrinsic_cast`
3. `src/backend/llvm/helpers.rs:686-694` — remove `cast_string_to_int`

#### Step 2.1: Define `resolve_cast()` Shared Dispatcher

`src/backend/llvm/emit_expr.rs` — extract a shared function:

```rust
// 2026-07-30: Unified cast resolution. Called by both Expr::Cast and
// Cast# intrinsic. Two paths:
//   Physical: target is #Bit or Bit-derived → literal memory bytes
//   Semantic: target is any other protocol → protocol pipeline
fn resolve_cast(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    src: BTypedRegister,
    target_ty: &Type,
    indent: &str,
) -> BTypedRegister {
    // Physical path: CastTo(#Bit) → literal memory bytes
    if backend.is_protocol_member(target_ty, "#Bit") {
        return resolve_physical_cast(backend, out, v, &src, indent);
    }
    // Semantic path: delegate to protocol pipeline
    // Step 1: direct op Cast(Target) on source type
    if let Some(result) = try_direct_cast(backend, out, v, &src, target_ty, indent) {
        return result;
    }
    // Step 2: CastTo(#Cat) → CastFrom(#Cat) protocol path
    if let Some(result) = try_protocol_path_cast(backend, out, v, &src, target_ty, indent) {
        return result;
    }
    // Step 3: meld shuffle
    if let Some(result) = try_meld_shuffle(backend, out, v, &src, target_ty, indent) {
        return result;
    }
    // Step 4: #Bits bitcast ONLY when target is #Bit
    if backend.is_protocol_member(target_ty, "#Bit") {
        return emit_bitcast(backend, out, v, &src, target_ty, indent);
    }
    // No path found — compile error
    panic!("No cast path from {:?} to {:?}", src.ty, target_ty);
}
```

#### Step 2.2: Physical Cast: `CastTo(#Bit)` → Literal Memory Bytes

`src/backend/llvm/emit_expr.rs`:

```rust
// 2026-07-30: Emit literal memory bytes at the value's address.
// For fixed-size types (Int, Float), this is a bitcast of the register.
// For pointer-like types (String, Slice), this dereferences the pointer.
fn resolve_physical_cast(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    src: &BTypedRegister,
    indent: &str,
) -> BTypedRegister {
    // Check if the source type has a custom CastTo(#Bit) operator
    if let Some(name) = type_name_str(&src.ty) {
        if let Some(impl_args) = find_cast_impl(backend, &name, "CastTo") {
            return emit_simple_call(backend, out, v, src, &impl_args, indent);
        }
    }
    // Fallback: bitcast the register bytes (for fixed-size types)
    let src_ll = backend.llvm_type(&src.ty);
    writeln!(out, "{}{} = bitcast {} {} to i64", indent, v, src_ll, src.name).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}
```

#### Step 2.3: Semantic Cast Steps

`src/backend/llvm/emit_expr.rs` — extract the existing logic from `intrinsics.rs:1021-1067`
into `try_direct_cast`, `try_protocol_path_cast`, `try_meld_shuffle`, `emit_bitcast`.

#### Step 2.4: Remove Hardcoded String Casts

`src/backend/llvm/helpers.rs:686-694` — remove `cast_string_to_int`. String→Int
semantic conversion now goes through the protocol pipeline (Step 2: `CastTo(#Int)`
on String).

`src/backend/llvm/emit_expr.rs:579-583` — remove the `is_string_chain(expr)` +
`__str_to_int` path. Delegate to `resolve_cast()`.

#### Step 2.5: Constrain Step 4 (#Bits bitcast) to Physical Only

`src/backend/llvm/intrinsics.rs:1064-1066` — change the unconditional #Bits bitcast
to only fire when target is #Bit:

```rust
// Step 4: Implicit Cast(#Bits) — raw bitcast.
// Only applies when target IS #Bits, not as a fallback for failed semantic casts.
if self.is_protocol_member(&src.ty, "#Bit")
    || ty_is_bit_target(&target_ll)
{
    writeln!(out, "{}{} = bitcast {} {} to {}", indent, v, src_ll, src.name, target_ll).ok();
    return BTypedRegister { name: v.to_string(), ty: src.ty.clone() };
}
return BTypedRegister { name: v.to_string(), ty: Type::int() }; // error sentinel
```

**Test**: `(Int) "123"` → parses via protocol pipeline → emits `call @parse_to_int`.
`(Bit) "123"` → physical path → emits `lshr+trunc` or `extractvalue`. `(Int) "abc"`
→ compile error (no valid parse path).

---

### Phase 3: Rewrite String as `type String: #String Bit`

**Objective**: Replace the struct-based String definition with Bit-derivation
model using explicit CastTo/CastFrom operators.

**Files**:
1. `lib/std/types/bootstrap.bv` — String declaration
2. `lib/std/string.bv` — operator implementations
3. `src/backend/llvm/normalizer.rs` — ensure operator_defs are registered
4. `src/backend/llvm/intrinsics.rs` — wire CastTo(#Bit) and CastTo(#Int) emission

#### Step 3.1: Bootstrap Declaration

`lib/std/types/bootstrap.bv`:

```brief
// 2026-07-30: Bit-derivation model. String IS raw bytes, not {data, len} struct.
// The fat pointer {ptr, len} is the container; CastTo(#Bit) dereferences to content.
// CastTo(#Int) and CastFrom(#Int) are semantic conversions (parse/format).
type String: #String Bit {
    op CastTo(#Bit) = string_get_content_bytes(#L);
    op CastTo(#Int) = string_parse_to_int(#L);
    op CastFrom(#Int) = string_from_int(#L);
    prop Size: chars(#L);
    prop Bytes: byte_len(#L);
};
```

**Note**: The `!> alignment: 8` and `!> encoding: "UTF-8"` metadata is dropped.
Alignment is derived from the fat-pointer struct shape. Encoding is the protocol's
job (`#String<UTF8>` by default).

#### Step 3.2: Operator Implementations

`lib/std/string.bv` — add defn bodies:

```brief
// 2026-07-30: Extract content bytes from String fat pointer.
// SSO: inline bytes from handle[0] bits[3..63]
// Heap: bytes at ptr (handle[0] with tag bits masked)
defn string_get_content_bytes(s: String) -> Slice<Bit> {
    // The backend has a fast path for this (emit_cast_to_bit_for_string).
    // This stdlib definition is for the interpreter and as reference.
    let ptr: Ptr<Bit> = __builtin_extract_string_ptr(s);
    let len: Int = __builtin_extract_string_len(s);
    Slice<Bit> { data: ptr, len: len }
};

// 2026-07-30: Parse string content as integer.
// "123" → 123. "a" → fails at compile-time for literals,
// returns Err or panics at runtime.
defn string_parse_to_int(s: String) -> Int {
    __builtin_string_to_int(s)
};

// 2026-07-30: Format integer as UTF-8 string.
// 123 → "123".
defn string_from_int(n: Int) -> String {
    __builtin_int_to_string(n)
};
```

#### Step 3.3: Backend Emission for CastTo(#Bit)

`src/backend/llvm/intrinsics.rs` — add `emit_cast_to_bit_for_string`:

```rust
// 2026-07-30: Emit CastTo(#Bit) for String type.
// SSO short: lshr i64 %handle, 3 (strip tag), return as Bit sequence
// SSO heap: and i64 %handle, -8 (mask tag), inttoptr, return Slice<Bit>
fn emit_cast_to_bit_for_string(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    handle: &str, len_reg: &str, indent: &str,
) -> BTypedRegister {
    if backend.feature_sso_strings {
        // SSO path — try inline first, fall back to heap
        let is_sso = backend.fun.gen_reg();
        writeln!(out, "{}{} = and i64 {}, 1", indent, is_sso, handle).ok();
        let sso_block = backend.fun.gen_reg();
        let heap_block = backend.fun.gen_reg();
        writeln!(out, "{}{} = icmp eq i64 {}, 1", indent, sso_block, is_sso).ok();
        // ... branch on SSO vs heap, extract bytes accordingly
    }
    // Legacy (non-SSO): handle is ptrtoint, inttoptr back, return Slice<Bit>
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, handle).ok();
    emit_slice_constructor(backend, out, v, &ptr, len_reg, indent)
}

// 2026-07-30: Construct a Slice<Bit> value { ptr, i64 } in LLVM IR.
fn emit_slice_constructor(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    ptr: &str, len: &str, indent: &str,
) -> BTypedRegister {
    let t0 = backend.fun.gen_reg();
    writeln!(out, "{}{} = insertvalue {{ ptr, i64 }} undef, ptr {}, 0", indent, t0, ptr).ok();
    let t1 = backend.fun.gen_reg();
    writeln!(out, "{}{} = insertvalue {{ ptr, i64 }} %{}, i64 {}, 1", indent, t1, t0, len).ok();
    BTypedRegister {
        name: t1,
        ty: Type::Applied("Slice".to_string(), vec![Type::bits(8)]),
    }
}
```

#### Step 3.4: Backend Emission for CastTo(#Int)

`src/backend/llvm/emit_expr.rs` — the semantic cast path now routes through
the protocol pipeline. When `CastTo(#Int)` is resolved:

1. The protocol pipeline finds `op CastTo(#Int)` on String (from bootstrap.bv operator_defs)
2. `emit_simple_call` emits `call i64 @string_parse_to_int(ptr %handle)`
3. For fast-path constant strings (string literals), the interpreter evaluates
   `string_parse_to_int` at compile time

---

### Phase 4: Wire `CastTo(#Bit)` → Slice<Bit> Emission

**Objective**: Connect the operator declaration in bootstrap.bv to the backend
emission path.

**Files**:
1. `src/backend/llvm/intrinsics.rs` — register `string_get_content_bytes` as recognized
2. `src/backend/llvm/emit_expr.rs` — physical cast path catches CastTo(#Bit)

#### Step 4.1: Register the Operator

The normalizer processes `op CastTo(#Bit) = string_get_content_bytes(#L)` from
bootstrap.bv and stores it in the operator_defs table. The backend's
`find_cast_impl("String", "CastTo")` finds it.

No change needed in the normalizer — it already processes operator_defs.

#### Step 4.2: Emit Through the Physical Cast Path

When `resolve_physical_cast` is called with `String → #Bit`:

1. `type_name_str(&src.ty)` → `Some("String")`
2. `find_cast_impl(backend, "String", "CastTo")` → finds `string_get_content_bytes`
3. `emit_simple_call(backend, out, v, src, &impl_args, indent)` → emits
   `call { ptr, i64 } @string_get_content_bytes({ i64, i64 } %handle)`

But we want the backend to INLINE this call, not emit a function call. The
`emit_cast_to_bit_for_string` function produces better LLVM IR (lshr, and,
inttoptr, extractvalue — no call instruction).

**Solution**: Register `string_get_content_bytes` as a recognized intrinsic-like
operation that the backend can inline:

```rust
// In the physical cast resolution, check for known fast paths:
fn resolve_physical_cast(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    src: &BTypedRegister,
    indent: &str,
) -> BTypedRegister {
    // Check for known fast-path types
    if let Some(name) = type_name_str(&src.ty) {
        match name.as_str() {
            "String" => return emit_cast_to_bit_for_string(backend, out, v, src, indent),
            "Slice" => return emit_cast_to_bit_for_slice(backend, out, v, src, indent),
            _ => {
                // Generic: call the CastTo(#Bit) operator if declared
                if let Some(impl_args) = find_cast_impl(backend, &name, "CastTo") {
                    return emit_simple_call(backend, out, v, src, &impl_args, indent);
                }
            }
        }
    }
    // Fallback: bitcast register
    let src_ll = backend.llvm_type(&src.ty);
    writeln!(out, "{}{} = bitcast {} {} to i64", indent, v, src_ll, src.name).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}
```

---

### Phase 5: Purge Name-Based Dispatch + Remove UTF8View

**Objective**: Zero `name == "String"` and `name == "UTF8View"` matches in Rust
code. Remove UTF8View as a separate type.

**Files to modify**:

| # | File | Line | Current | Replace With |
|---|------|------|---------|--------------|
| 1 | `interpreter/casts.rs` | 28 | `name == "String"` | `is_protocol_member(ty, "#String")` |
| 2 | `interpreter/casts.rs` | 80 | `name == "String"` | Check `Cast.#String` property |
| 3 | `type_universe/mod.rs` | 253 | `is_string_like()` | Remove — detect via `Cast.#String` |
| 4 | `emit_toplevel.rs` | 252-255 | `name == "UTF8View"` | Remove (UTF8View deleted) |
| 5 | `emit_toplevel.rs` | 280-281 | `name == "String"` | `is_protocol_member(ty, "#String")` |
| 6 | `emit_toplevel.rs` | 294 | `name == "String"` | `is_protocol_member(ty, "#String")` |
| 7 | `helpers.rs` | 686-694 | `cast_string_to_int` | Remove (Phase 2 handles it) |
| 8 | `emit_expr.rs` | 579-583 | `is_string_chain(expr)` | Remove (Phase 2 handles it) |
| 9 | `mod.rs` | 1594 | Comment about UTF8View | Update comment |

#### Step 5.1: Remove `is_string_like()`

`src/type_universe/mod.rs:253-262`:

```rust
// REMOVED 2026-07-30: Structural string detection replaced by protocol membership.
// Use is_protocol_member(ty, "#String") instead.
```

The consumers of `is_string_like()`:
- `emit_toplevel.rs:284` — SSO String check: replace with `is_protocol_member(ty, "#String")`
- Any other callers: `git grep is_string_like` to find and replace

#### Step 5.2: Remove UTF8View

`lib/std/types/bootstrap.bv` — remove UTF8View declaration:
```brief
// REMOVED 2026-07-30: Replaced by Slice<Bit>. CastTo(#Bit) on String
// returns Slice<Bit> which serves the same view role.
```

`lib/std/types/utf8view.bv` — move functions to `lib/std/slice.bv`:
- `memcmp` → method on Slice<Bit>
- `UTF8_find` → method on Slice<Bit>
- `UTF8_validate` → method on Slice<Bit>
- `UTF8view_len` → remove (use Slice.Size property)
- `UTF8view_eq` → `Slice.eq`

`src/backend/llvm/emit_toplevel.rs:252-255` — remove UTF8View check:
```rust
// REMOVED 2026-07-30: UTF8View is replaced by Slice<Bit>.
// Struct shape derivation handles Slice<Bit> → { ptr, i64 }.
```

`benchmarks/utf8_ops.bv` — update imports from `"std/types/UTF8view.bv"` to
Slice-based operations.

#### Step 5.3: Replace `name == "String"` with Protocol Checks

Systematically replace all remaining name-based String matches:

```rust
// BEFORE:
if let Type::Custom(name) = ty {
    if name == "String" { ... }
}

// AFTER:
if self.is_protocol_member(ty, "#String") { ... }
```

**Validation**: After all changes:
```bash
git grep -n '== "String"' src/   # → zero results
git grep -n '"UTF8View"' src/    # → zero results
```

---

### Phase 6: Decouple Round-Trip Verification

**Objective**: Make CastTo/CastFrom round-trip verification opt-in, not default.

**Rationale**: With independent CastTo and CastFrom operations (e.g.,
`String.CastTo(#Int)` = parse, `Int.CastTo(#String)` = format), there is no
requirement that `CastFrom(CastTo(x)) == x`. The protocol round-trip check
(`protocol_graph.rs:303-369`) currently fails unless this holds.

**Files**:
1. `src/ast/top.rs` — add `roundtrip_verified` field to ProtocolDef
2. `src/analysis/protocol_graph.rs` — make verification conditional
3. `lib/std/protocols.bv` — add `!> roundtrip: true` where symmetry is required

#### Step 6.1: Add Metadata

`src/ast/top.rs` — ProtocolDef struct:

```rust
pub struct ProtocolDef {
    pub name: String,
    pub category: String,
    pub contract: Option<Contract>,
    pub cast_edges: Vec<CastEdge>,
    pub cross_ops: Vec<OperatorDef>,
    pub span: Option<Span>,
    // 2026-07-30: Round-trip verification is opt-in.
    // Set !> roundtrip: true in the protocol declaration to enable
    // CastFrom(CastTo(x)) == x proofs. Default: no symmetry assumed.
    pub roundtrip_verified: bool,
}
```

#### Step 6.2: Conditional Verification

`src/analysis/protocol_graph.rs` — `verify_protocol_roundtrip`:

```rust
pub fn verify_protocol_roundtrip(
    pd: &ProtocolDef,
    items: &[TopLevel],
) -> Result<(), String> {
    if !pd.roundtrip_verified {
        return Ok(());  // Round-trip verification opt-out
    }
    // ... existing verification logic ...
}
```

#### Step 6.3: Enable for Encoding Conversions

`lib/std/protocols.bv`:

```brief
proto ASCII: #String {
    !> roundtrip: true;  // ASCII ↔ UTF8 IS symmetric
    CastTo(#String<UTF8>) = ascii_to_utf8(#L);
    CastFrom(#String<UTF8>) = utf8_to_ascii(#L);
};

proto UTF16: #String {
    !> roundtrip: true;  // UTF16 ↔ UTF8 IS symmetric
    CastTo(#String<UTF8>) = utf16_to_utf8(#L);
    CastFrom(#String<UTF8>) = utf8_to_utf16(#L);
};
```

---

### Phase 7: Convenience Operators + Benchmarks

**Objective**: Add `as_int32()` convenience operator on String. Implement the
"fast parser trick" benchmark. Verify IR quality.

**Files**:
1. `lib/std/string.bv` — add `as_int32`
2. `benchmarks/string_swift_parse.bv` — HTTP method routing benchmark
3. `benchmarks/string_swift_parse_c.c` — C reference
4. `benchmarks/build_and_bench.sh` — register the new benchmark

#### Step 7.1: Convenience Operators

`lib/std/string.bv`:

```brief
// 2026-07-30: Read first 4 bytes as Int32 (fast parser trick).
// SSO short: 2 LLVM instructions (lshr + trunc)
// SSO heap:  3 LLVM instructions (and + inttoptr + load)
// Legacy:    2 LLVM instructions (inttoptr + load)
defn as_int32(s: String) -> Int32 {
    let content: Slice<Bit> = s.CastTo(#Bit);
    let p: Ptr<Int32> = content.data as Ptr<Int32>;
    load<Int32>(p)
};

// 2026-07-30: Read first 8 bytes as Int64.
defn as_int64(s: String) -> Int64 {
    let content: Slice<Bit> = s.CastTo(#Bit);
    let p: Ptr<Int64> = content.data as Ptr<Int64>;
    load<Int64>(p)
};
```

#### Step 7.2: Benchmark

`benchmarks/string_swift_parse.bv`:

```brief
import "std/string.bv";

// HTTP method routing via single Int32 comparison.
// Branchless: 1 load + 3 cmp + conditional moves.
defn parse_method(line: String) -> Int {
    let magic: Int32 = line.as_int32();
    when magic & 0x00FFFFFF == 0x00544547 { term 1; };  // "GET"
    when magic == 0x20544547 { term 2; };                 // "GET "
    when magic == 0x54534f50 { term 3; };                 // "POST"
    when magic == 0x20545550 { term 4; };                 // "PUT "
    when magic == 0x454c4544 { term 5; };                 // "DELE"
    term 0;
};

let N: Int = GetEnvInt#("BOUND");
let test_str: String = "GET /api/v1/users HTTP/1.1\n";

node bench [i < N][i == N] {
    let method: Int = parse_method(test_str);
    i = i + 1;
    term! -> PrintInt#(method);
};
```

#### Step 7.3: Verify LLVM IR

```bash
# Compile the benchmark
./target/release/briefc compile benchmarks/string_swift_parse.bv --llvm
# Check that the hot loop emits load i32, not a function call
grep 'load i32' string_swift_parse.ll
# Expected: %val = load i32, ptr %ptr  (single instruction)
```

---

### Phase 8: Documentation

**Docs to update**:
- `docs/architecture/hash-words.md` — document CastTo(#Bit) vs semantic cast distinction
- `docs/architecture/intrinsics-vs-stdlib.md` — add Slice<T> as "stdlib type, zero compiler changes"
- `spec/SPEC.md` — update String type definition syntax
- `AGENTS.md` — add Bit-derivation convention, struct shape derivation rule
- `BUGS.md` — log any issues found during implementation

**Rationale comments** at every modified code site: `// 2026-07-30: <reason>`

---

## Execution Order & Dependency Graph

```
Phase 0 (inject Cast.#Bit) — no dependencies
  │
  ▼
Phase 1 (Slice<T> + struct shape) — depends on Phase 0 for Cast.#Bit injection
  │
  ▼
Phase 2 (split cast paths) — depends on Phase 1 for Slice<Bit> type
  │
  ├──────────────────────────────────┐
  ▼                                  ▼
Phase 3 (String rewrite)         Phase 5 (purge name dispatch)
(depends on Phase 2)             (can parallel with Phase 3)
  │                                  │
  ├──────────────────────────────────┘
  ▼
Phase 4 (CastTo(#Bit) emission) — depends on Phase 3
  │
  ▼
Phase 6 (decouple round-trip) — independent, can run after Phase 3
  │
  ▼
Phase 7 (benchmarks) — depends on Phase 4
  │
  ▼
Phase 8 (docs) — after all phases
```

Parallelization: Phases 3 and 5 can run in parallel. Phase 6 can start after
Phase 3. Phases 0 and 1 must complete before Phase 2.

---

## Benchmark Baseline

Before any changes, run the full benchmark suite and record results:

```bash
bash benchmarks/build_and_bench.sh --runtime
bash benchmarks/build_and_bench.sh --optimizer
bash benchmarks/build_and_bench.sh --correctness
```

New benchmark to add: `string_swift_parse`. Compare against C reference.

Expected improvement:
- String→Int parsing: from `call @__str_to_int` (function call) to inlined
  protocol pipeline (direct parse in same function)
- Byte-level string operations: from `UTF8View.func(ptr, len)` to
  `Slice<Bit>.func(ptr, len)` — same runtime, cleaner type
- Read-as-Int: previously required unsafe pointer casting workaround, now
  a first-class protocol operation

---

## Per-Phase Commit Checklist

Each phase MUST pass before committing:

1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. `git status` and `git diff` — only intended files staged
4. Rationale comments added at every modified code site
5. Architecture docs updated if API contracts changed
6. BUGS.md updated with any bugs found during the phase
7. Phase-specific behavioral tests pass
8. Regression guard: verify no existing match arms removed, no existing
   optimization paths weakened

---

## Regression Guard

Before and after each phase:

1. **Inspect every match arm** in modified functions. Silent regressions come
   from removed arms, not added ones.
2. **Verify optimized IR** — run the relevant benchmarks and compare against
   the pre-change numbers.
3. **Check git history** for the ACTUAL evolution of any function being refactored.
4. **Map ALL benchmarks** — every optimization decision affects all benchmarks.

### Known Risks

- **Phase 2 (cast path split)**: High risk. The two existing cast pipelines
  (`Expr::Cast` and `Cast#`) may have subtle behavioral differences. After
  unification, existing programs that relied on one path vs the other may break.
  **Mitigation**: Add explicit test cases for every cast scenario before changing
  the pipeline. Run `--correctness` on all benchmarks after Phase 2.

- **Phase 3 (String rewrite)**: Medium risk. Changing the String type definition
  affects all code that imports or uses String. The `CastTo(#Bit)` and
  `CastTo(#Int)` operators must be defined before existing code that uses
  `(Int) string_val` or `(Bit) string_val` can work.
  **Mitigation**: Implement Phase 2 first (split cast paths), then define the
  operators in stdlib, then update the backend.

- **Phase 5 (purge name dispatch)**: Low risk if done incrementally. Each
  replacement can be tested independently.
  **Mitigation**: One match arm at a time, with a test after each.

---

## Per-Commit Checklist Template

```bash
# Before every commit:
cargo test --lib
cargo build
git status && git diff --stat
# Verify no name-based String matches remain in changed files
git diff --unified=0 | grep '== "String"' | grep '^+' && echo "REGRESSION: name match" || echo "clean"
# Commit with message describing what and why
git add <intended files only>
git commit -m "YYYY-MM-DD: Phase N — <description>

<what changed, why, what pattern it targets>"
```

---

## Plan Directives Compliance

1. **FLAT CONTROL FLOW**: All added functions use guard clauses and early returns.
   Max 2 nesting levels. No arrowhead code.

2. **COMMENT THE CODE**: Every modified or added code site has a `// 2026-07-30:`
   rationale comment explaining intent, not mechanics. Never delete rationale
   comments from refactored code — rewrite them to explain the new structure.

3. **UPDATE ALL EXAMPLES**: When syntax changes, update every example file
   (`examples/`, `lib/std/`, `benchmarks/`) that used the old syntax.

4. **DOCUMENTATION IS CODE**: Update `docs/architecture/`, `docs/features/`,
   and inline `///` doc comments in the same commit as the code change.

5. **BEHAVIORAL TESTS, NOT LITERAL TESTS**: Every new feature has unit tests
   that assert behavioral outcomes — not literal IR snapshots or implementation
   details. A test must pass after refactoring if the behavior is preserved.

---

## Open Questions (Resolved)

1. **Q**: Should `Slice<T>` be a Type::Slice AST variant or `Type::Applied`?
   **A**: `Type::Applied("Slice", [T])`. No new variant needed. The universe
   resolves "Slice" → ResolvedType with fields, and `llvm_type()` derives
   `{ ptr, i64 }` from shape.

2. **Q**: Should UTF8View be removed?
   **A**: Yes. `Slice<Bit>` serves the same role. Functions migrate from
   `utf8view.bv` to `slice.bv`.

3. **Q**: Should CastTo/CastFrom round-trip verification be mandatory?
   **A**: No. Opt-in via `!> roundtrip: true` metadata. Default is
   independent operations.

4. **Q**: Should `bytes` field in ResolvedType be removed?
   **A**: Not in this plan. The `bytes` field is transitional design baggage
   but removing it is a separate cleanup. This plan's struct shape derivation
   does not depend on `bytes`.
