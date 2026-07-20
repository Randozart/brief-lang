# Protocol and Meld Architecture — Final Specification

**Date:** 2026-07-20  
**Status:** Foundational  
**Supersedes:** CTD/ALU/TOML architecture, category inference (2026-07-19)

---

## Core Principles

1. **The sole primitive is Bits.** Every type is a Bits subtype with layout + ops.
2. **A type is its fields + its ops.** Metadata is an optimization hint, not identity.
3. **Hashwords are backend directives, not category tags.** `#Int` means "backend, emit your native integer addition." A type never *belongs* to a category — it *interacts* with one through its ops.
4. **No TOML config.** Hashwords replace `llvm-ops.toml` and `ctd-llvm-mappings.toml`. Only `config/targets.toml` remains.
5. **UTF-8 is the universal default for all files.** `.bv` and `.ebv` both default to UTF-8. ASCII is explicit opt-in via `#String<ascii>`.
6. **Cast chains are inlined and optimized by LLVM.** An ASCIIString → UTF-8 → ASCIIString conversion (both directions through the protocol) folds to a no-op for the ASCII byte range.

---

## Layer 1: Bits (the only primitive)

```brief
// Exists implicitly. No layout. Only bitwise ops.
op And(#Bits, #Bits);
op Or(#Bits, #Bits);
op Xor(#Bits, #Bits);
op Not(#Bits);
op Shl(#Bits, #Bits);
op Shr(#Bits, #Bits);
```

Every type implicitly inherits `Cast(#Bits)` — the ability to reinterpret its bytes. This is the universal fallback for all type conversions.

---

## Layer 2: Layout from structure

A type's layout is determined by its fields:

```brief
type Meters { data: Bits<64>; };           // bytes=8, llvm_type="i64"
type ASCIIString {                          // bytes=16, llvm_type="{ i64, i64 }"
    data: Bits<64>;                         // pointer to bytes
    len: Bits<64>;                          // byte count
};
type Packed {
    a: Bits<16>;                            // 2 bytes
    b: Bits<48>;                            // 6 bytes
};                                          // total: bytes=8, llvm_type="{ i64 }" or "[8 x i8]"
```

No `bytes <~`, `alignment <~`, `ctd <~`, or `alu <~` needed.

---

## Layer 3: Ops with hashwords

Ops declare how a type interacts with backend categories:

```brief
type Bfloat16 {
    data: Bits<16>;
    op Add(#Float, #Float) = bfloat_add(#L, #R);
    op Mul(#Float, #Float) = bfloat_mul(#L, #R);
    op Cast(#Bits);                         // implicit — explicit override optional
};

type Posit32 {
    data: Bits<32>;
    op Add(Posit32) = posit32_add(#L, #R);  // no hashword — custom function
    op Mul(Posit32) = posit32_mul(#L, #R);
    op Cast(#Bits);
};
```

| Op form | Meaning | Emission |
|---|---|---|
| `op Add(#Int, #Int)` | Backend, use your intrinsic integer add | `add i64 %a, %b` |
| `op Add(#Float, #Float) = fn(#L, #R)` | Override backend's float add with custom fn | `call @fn` (auto-alwaysinline) |
| `op Add(Posit32) = fn(#L, #R)` | Custom op for this type only | `call @fn` (auto-alwaysinline) |
| `op Parse(#Category)` | Compile-time identity — literal IS the protocol | No emission (identity, zero-cost) |
| `op Parse(Bare/Decimal/Quoted)` | Literal construction from source text | Emit call to bound defn (auto-alwaysinline) |

Functions bound to ops via `= fn(...)` are automatically emitted with `alwaysinline`.

---

## Layer 4: Protocol variants

Hashwords can be parameterized by protocol variant:

```brief
#String<utf8>         // UTF-8 encoding (default for all files)
#String<ascii>        // ASCII (explicit opt-in)
#Float<ieee754>       // IEEE 754 (default)
#Char<unicode>        // Unicode scalar (default)
```

Bare hashwords resolve to their default variant at parse time:

| Hashword | Resolved to |
|---|---|
| `#String` | `#String<utf8>` |
| `#Float` | `#Float<ieee754>` |
| `#Char` | `#Char<unicode>` |
| `#Int`, `#Bool`, `#Bits` | (no variants) |

UTF-8 is the universal default for ALL files (both `.bv` and `.ebv`).
ASCII is an explicit opt-in via `#String<ascii>`. No file-extension-based
default switching.

---

## Layer 5: Protocol shapes — the common language

Each hashword category has a well-defined protocol shape that types must
produce and consume through `CastTo`/`CastFrom`:

| Category | Protocol shape | Production | Consumption |
|---|---|---|---|
| `#String` | UTF-8 byte sequence | `op CastTo(#String)` — emit UTF-8 bytes | `op CastFrom(#String)` — accept UTF-8 bytes |
| `#Int` | Two's complement `i64` | `op CastTo(#Int)` — emit integer value | `op CastFrom(#Int)` — accept integer value |
| `#Float` | IEEE 754 binary32/64 | `op CastTo(#Float)` — emit float bytes | `op CastFrom(#Float)` — accept float bytes |
| `#Bool` | `i1` (single bit) | `op CastTo(#Bool)` | `op CastFrom(#Bool)` |
| `#Char` | Unicode scalar (`i32`) | `op CastTo(#Char)` | `op CastFrom(#Char)` |
| `#Bits` | Raw `iN` | (implicit) | (implicit) |

Conversion happens through the `CastTo`/`CastFrom` pair directly — there is
no intermediate currency type. The compiler inlines both ops and LLVM
eliminates any redundant transformations:

```brief
inline defn any_string_to_ascii(source: #String) -> ASCIIString {
    let bytes = source :> CastTo(#Bits);        // raw bytes
    let result = ASCIIString::from_bytes(bytes); // construct from bytes
    result
};
```

When `CastTo(#String)` and `CastFrom(#String)` are both inlined, LLVM sees:

```
Source.CastTo(#String) → emit UTF-8 bytes
Target.CastFrom(#String) → consume UTF-8 bytes
→ For ASCII-range data: both sides produce/consume identical bytes
→ LLVM's InstCombine eliminates the pair
```

---

## Layer 6: Cast#() intrinsic and resolution

`Cast#()` is a **compiler intrinsic** — not user-declarable. When the
programmer writes `(TargetType)source`, the compiler internally emits
`Cast#(source, TargetType)`, which runs the resolution pipeline.

### User-declarable ops

| Op | Direction | Example | When it fires |
|---|---|---|---|
| `op CastTo(#String)` | Source **→** Protocol | `Latin1 → UTF-8` | Source emits UTF-8 bytes |
| `op CastFrom(#String)` | Protocol **→** Source | `UTF-8 → ASCII` | Target accepts UTF-8 bytes |
| `op Cast(ConcreteType)` | Source **→** Concrete | `Posit32 → Int` | Direct type-to-type (no protocol) |

`CastTo` and `CastFrom` are always oriented toward the `#Category` protocol:

```brief
type Latin1String {
    op CastTo(#String) = latin1_to_utf8(#L);      // Latin1 → UTF-8 bytes
    op CastFrom(#String) = utf8_to_latin1(#L);     // UTF-8 bytes → Latin1
};
```

`Cast(ConcreteType)` is for direct paths between concrete types, bypassing
protocols. No `To`/`From` needed because both sides are concrete:

```brief
type Posit32 {
    op Cast(Int) = posit32_to_int(#L);             // Posit32 → Int
    op Cast(Float) = posit32_to_float(#L);          // Posit32 → Float
};
```

### Resolution pipeline

`Cast#(source, Target)` runs:

```
┌─────────────────────────────────────────────────────┐
│ 1. meld Source <-> Target exists?                   │
│    └─ Apply declared operations (inline or fn call)  │
│       → Zero-cost. Validated by 5-layer cascade.    │
├─────────────────────────────────────────────────────┤
│ 2. op Cast(Target) declared on Source?              │
│    └─ Direct type-to-type conversion. Auto-inlined. │
├─────────────────────────────────────────────────────┤
│ 3. Protocol path via CastTo/CastFrom?               │
│    └─ Source.CastTo(#Category) →                   │
│       Target.CastFrom(#Category)                    │
│       Both inlined. LLVM optimizes the chain.        │
│       Example: Latin1String → #String → ASCIIString │
│       Latin1.CastTo decodes Latin-1 to Char         │
│       ASCIIString.CastFrom encodes Char to ASCII    │
├─────────────────────────────────────────────────────┤
│ 4. Implicit CastTo(#Bits) + CastFrom(#Bits)         │
│    └─ Raw bytes. Always available.                  │
│       Same width → bitcast (no-op)                  │
│       Different width → lshr/and/shl/or remap       │
└─────────────────────────────────────────────────────┘
```

### CastTo / CastFrom optimization

When `Source.CastTo(#String)` and `Target.CastFrom(#String)` are both declared
and inlined, LLVM sees:

```llvm
; Source.CastTo(#String) — e.g., Latin1 to UTF-8
%char = zext i8 %byte to i32   ; Latin-1 byte → Unicode scalar
; ... (potential multi-byte encoding for >127)

; Target.CastFrom(#String) — e.g., UTF-8 to ASCII
%byte_out = trunc i32 %char to i8  ; Unicode scalar → ASCII byte
```

LLVM's `InstCombine` eliminates the `zext`/`trunc` pair for values in the
overlapping range (0-127). For values >127, the Latin-1 `CastTo` must emit
multi-byte UTF-8, and ASCII's `CastFrom` rejects them — correct behavior
preserved.

The protocol path goes through the `CastTo`/`CastFrom` pair directly — no
intermediate type, no runtime allocation. The protocol shape (UTF-8 bytes,
IEEE 754 float, etc.) is a compile-time contract, not a runtime object.

---

## Layer 7: Meld — explicit structural equivalence

Meld expresses: **"I, the programmer, assert that these two types are
structurally equivalent (possibly with simple transformations)."**

```brief
meld Meters -> Kilometers {
    data = #L / 1000;
};

meld String -> PyString {
    data = pyobj_from_utf8(#L);
};
```

Meld is NOT:

| NOT meld | Handled by |
|---|---|
| Protocol membership | Hashwords + protocol ops |
| Category-level conversion | `Cast(#Category)` + protocol currencies |
| Raw byte reinterpretation | `Cast(#Bits)` (implicit) |

Meld IS:

| IS meld | What it enables |
|---|---|
| Explicit structural assertion | `meld A -> B` with inline ops or function calls |
| Implicit conversion in Brief code | Pass `Meters` where `Kilometers` expected — meld fires |
| FFI bridge | Pass `String` where `PyString` expected — meld fires |
| Validated by 5-layer cascade | L1: Structural → L2: Bit-permutation → L3: Unit-vector → L4: Symbolic → L5: SMT |

Meld validation prevents incorrect structural assertions. If the programmer
claims `Meters` maps to `Kilometers` by dividing, the validator checks that
the result fits and the transformation is invertible.

---

## Layer 8: Protocol shapes (backend contract)

| Hashword | Protocol shape | Required ops |
|---|---|---|
| `#Int` | `i64` | Add, Sub, Mul, Div, CastTo(#Bits), CastFrom(#Bits) |
| `#Float` | IEEE 754 binary32/64 | Add, Sub, Mul, Div, Sqrt, CastTo(Float64), CastFrom(Float64), CastTo(#Bits), CastFrom(#Bits) |
| `#Bool` | `i1` (stored i8) | And, Or, Not, CastTo(#Bits), CastFrom(#Bits) |
| `#Char` | Unicode scalar (i32) | CastTo(#Int), CastFrom(#Int), Eq, Lt |
| `#String` | UTF-8 byte sequence | CastTo(#Char), CastFrom(#Char), Extract(#Char), InsertAt(#Char), Concat(#String), :> Size, CastTo(#Bits), CastFrom(#Bits) |
| `#Bits` | Raw `iN` | And, Or, Xor, Not, Shl, Shr, CastTo(iN), CastFrom(iN) |

Every backend MUST be able to translate these protocol shapes. A backend that
cannot represent `i64` decomposes it into smaller units but still provides
the `#Int` protocol ops.

---

## Layer 9: Type parameter constraints

```brief
type HashMap<K: #String, V> {
    buckets: Bits<64>;
    len: Bits<64>;
    cap: Bits<64>;
    op Insert(V) = hashmap_insert(#L, #R1, #R2);
    op Get(K) -> V = hashmap_get(#L, #R);
};
```

`K: #String` is a **protocol satisfaction check**. The typechecker verifies
that `K` implements the `#String` protocol ops (`Extract(#Char)`,
`InsertAt(#Char)`, `Concat(#String)`, `:> Size`, `Cast(#Bits)`).
If yes, the constraint is satisfied.

At instantiation `HashMap<ASCIIString, Int>`:
- `K` = ASCIIString → checked against `#String` protocol → pass
- `V` = Int → no constraint → pass

---

## Layer 10: Parse — compile-time literal construction

Types declare how they are constructed from source text at compile time
using `op Parse` declarations:

```brief
type Int <: Bits {
    op Add(#Int, #Int);
    op Parse(#Int);           // compile-time identity — literal IS an Int
    op Parse(Decimal);        // "42" constructs an Int via conversion fn
};

type HexColor {
    data: Bits<24>;
    op Parse(Bare) = parse_hex_color(#L);   // "FF00FF" → color
    op Cast(#Bits);
};
```

### Parse forms

| Op form | Meaning | Conversion function? |
|---|---|---|
| `op Parse(#Category)` | Compile-time identity — target IS the protocol | No — literal bytes are already valid |
| `op Parse(Bare)` | Construct from bareword identifier `FF00FF` | Yes — required |
| `op Parse(Decimal)` | Construct from numeric literal `42` or `3.14` | Yes — required |
| `op Parse(Quoted)` | Construct from quoted string `"..."` | Yes — required |

`Parse(#Category)` is a hashword op — it declares identity with the protocol,
so no conversion function is needed. `Parse(Bare/Decimal/Quoted)` are concrete
form ops that ALWAYS require a conversion function, because the compiler has
no intrinsic knowledge of how barewords, numbers, or quoted strings map to
arbitrary type values.

### Parse resolution pipeline

When the compiler encounters a literal expression assigned to type `T`:

```
1. Determine the literal's syntactic form (Bare, Decimal, or Quoted)
2. Does T declare op Parse(#Category) where #Category matches the
   literal's protocol? → Use identity (zero-cost, no emission)
3. Does T declare op Parse(Form) with a conversion function?
   → Call the inlined defn at compile time
4. Does T's parent (via <:) have a Parse op? → Check inheritance
5. No match → compile error: "type T does not accept Decimal literals"
```

### Parse + Cast interaction

After Parse constructs a value, `Cast#()` may fire if the parsed value's
type doesn't match the target expression's expected type:

```brief
type MyInt { data: Bits<64>;
    op Parse(Decimal) = myint_from_decimal(#L);
    op Cast(#Int) = myint_to_int(#L);
};
let x: Int = 42;  // Parse(Decimal) → MyInt, then Cast(#Int) → Int
```

The compiler folds Parse + Cast into a direct conversion when both are inlined.

### Round-trip verification

For every type that declares both a Parse op and a produce op (`CastTo` or
`Cast(#Category)`), the compiler performs symbolic execution at compile time:

```
// For every op Parse(Form) = fn(#L):
//   1. Apply fn to a representative test literal → value
//   2. Apply op Cast(#Category) or custom produce → back to bytes
//   3. Assert: original test literal == step 2 result
```

If the round-trip fails, the compiler emits a warning (or error in strict mode)
with the exact input and output shown:

```
warning: Parse → Cast round-trip failed for type 'HexColor'
  Parse('FF00FF') → 0xFF00FF
  Cast(#String) → '00FF00' (expected 'FF00FF')
```

Verification uses the protocol's `CastTo`/`CastFrom` pair directly:

```brief
// Parse: "FF00FF" → HexColor via parse_hex_color
// Cast: HexColor → #String via hex_color_to_string
// Round-trip: hex_color_to_string(parse_hex_color("FF00FF")) == "FF00FF"
```

### Replacement of `formatting <~`

The `formatting <~` metadata property and the `codec { ... }` declaration form
are superseded by `op Parse`:

| Old mechanism | Replaced by |
|---|---|
| `formatting <~ Bare` + `parse <~ parse_hex` | `op Parse(Bare) = parse_hex(#L)` |
| `formatting <~ Decimal` + `parse <~ parse_fn` | `op Parse(Decimal) = fn(#L)` |
| `formatting <~ Quoted` + `parse <~ identity` | `op Parse(#String)` or `op Parse(Quoted) = fn(#L)` |
| `DefaultQuoted` codec class | Inline `op Parse` on each type |

The `op Parse` system is additive with respect to the `op` system — it uses
the same operator declaration syntax, the same `#L` positional marker, and
the same `alwaysinline` semantics for bound functions.

---

## Resolved Design Questions (Parse + Protocol System)

### Q1: Error boundaries — what if round-trip verification fails?

The test value and expected/actual results are shown with source location.
The user can suppress via `#[allow(protocol_roundtrip)]` annotation on the
type — but only after explicitly acknowledging the deviation. Suppression
without acknowledgement is a compile error.

### Q2: Dispatch priority — concrete vs hashword overloads

Concrete type wins over hashword. `op Add(ConcreteType, ConcreteType)` fires
before `op Add(#String, #String)` because concrete is a more specific match.
Same principle applies to Parse: `op Parse(Decimal) = fn(#L)` fires before
`op Parse(#Int)` for numeric literals, because the concrete form is more
specific.

### Q3: Protocol variant directions — CastTo utf8, CastFrom ascii?

Valid. A transcoder type may `CastFrom(#String<utf8>)` and
`CastTo(#String<ascii>)` — that's a meaningful conversion.
The round-trip test handles it: `CastFrom(utf8) → CastTo(ascii)` is tested
separately from `CastFrom(ascii) → CastTo(utf8)`.

### Q4: How tolerant is round-trip equality? Structural or literal?

Structural equality, not literal bytes. `"XIV"` and `"xiv"` are the same
RomanNumeral if `Parse(Bare)` is case-insensitive by design. The comparison
uses the type's own `Eq` op if available, falling back to byte equality.

### Q5: Module caching — duplicated work across modules?

Both, but cached. The round-trip test runs when a type is first defined.
When another module imports it, the result is cached by type name + SHA-256
of the op implementation. No duplicated work.

### Q6: Write-only types — Parse without CastTo?

Valid. A type can be constructed from a literal but never serialized back.
The round-trip test simply doesn't run for directions that aren't declared.
Useful for opaque handles, capabilities, or types that exist only
temporarily within a computation.

### Q7: Parse inheritance — does `<: pass Parse ops?

Yes. Parse ops follow the same inheritance rules as other ops.
If `ASCIIString <: String` and `String` declares `op Parse(#String)`,
`ASCIIString` inherits it — correct because ASCII IS valid UTF-8.
A subtype may override with its own `op Parse(Form) = fn(#L)`.

---

## Implementation Status

| Layer | Status | Files changed |
|---|---|---|
| 1. Bits as sole primitive | ✅ Implicit | Already foundational |
| 2. Layout from structure | ✅ Complete | normalizer.rs, type_universe |
| 3. Ops with hashwords | ✅ Complete | parser, OperatorDef |
| 4. Protocol variants | ✅ Complete | HashWordVariant, utf8 default |
| 5. Protocol currencies | ⏳ Specified | Need protocol ops in stdlib |
| 6. Cast resolution | ⏳ Specified | Need Cast resolution in typechecker |
| 7. Meld (explicit) | ⏳ Specified | Need updated meld parser + validation |
| 8. Protocol shapes | ✅ Documented | casting-protocol.md |
| 9. Type param constraints | ✅ Complete | compile.rs validate_constraints |
| 10. Parse protocol | ⏳ Specified | Need op Parse parser + resolution |
| 10a. Round-trip verification | ⏳ Specified | Need symbolic exec of Parse→Cast→Produce |

---

## Files Changed (Future Implementation)

| File | Change |
|---|---|
| `src/type_universe/operators.rs` | Add Cast resolution with priority: meld → op Cast → protocol graph → #Bits |
| `src/backend/llvm/emit_expr.rs` | Expr::Cast handler uses Cast resolution |
| `src/parser/definitions.rs` | Update meld parser for inline expr ops |
| `src/analysis/meld_validation.rs` | Keep 5-layer cascade, update for new meld semantics |
| `src/type_universe/mod.rs` | Keep universe.melds, update find_meld for implicit conversion |
| `lib/std/types/bootstrap.bv` | Add protocol ops (Extract, InsertAt, etc.) for String |
| `lib/std/core/ring_buffer.bv` | Already updated (op InsertAt(T) = ring_push(#L,#R)) |
| `src/parser/definitions.rs` | Parse `op Parse(#Category)` and `op Parse(Bare/Decimal/Quoted)` |
| `src/type_universe/operators.rs` | Parse op resolution in literal construction |
| `src/backend/llvm/emit_expr.rs` | Expr::Quoted/Decimal/Bare dispatch through Parse ops |
| `src/interpreter/mod.rs` | Compile-time literal construction via Parse ops |
| `lib/std/types/bootstrap.bv` | Add `op Parse(#Int)`, `op Parse(#Float)`, `op Parse(#String)` |
