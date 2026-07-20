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

## Layer 5: Protocol currencies (the common language)

Each hashword category has a universal currency for cross-type conversion.
Types declare `CastTo(#Category)` to produce the currency, and
`CastFrom(#Category)` to consume it:

| Category | Currency | Protocol ops required |
|---|---|---|
| `#String` | `Char` (Unicode scalar, i32) | `CastTo(#Char)`, `CastFrom(#Char)`, `Extract(#Char)`, `InsertAt(#Char)`, `:> Size` |
| `#Char` | `#Int` (Unicode scalar value) | `CastTo(#Int)`, `CastFrom(#Int)`, `Eq(#Char)`, `Lt(#Char)` |
| `#Float` | `Float64` (IEEE 754 double) | `CastTo(Float64)`, `CastFrom(Float64)` |
| `#Int` | `#Bits` (raw integer) | `CastTo(#Bits)`, `CastFrom(#Bits)`, `Add(#Int)`, `Sub(#Int)` |
| `#Bits` | (itself) | (implicit) |

A conversion function between two `#String` types:

```brief
inline defn any_string_to_ascii(source: #String) -> ASCIIString {
    let len = source :> Size;
    let result = ASCIIString::alloc(len);
    let mut i = 0;
    do {
        let c: Char = source :> Extract(i);         // decode to universal currency
        result :> InsertAt(i, c);                    // encode from universal currency
        i = i + 1;
    } while i < len;
    result
};
```

When both sides operate on ASCII-range data, LLVM inlines the `Extract`/`InsertAt`
and eliminates the intermediate `Char`:

```
Extract: load i8 %src, %idx → zext i8 to i32    (made Char)
InsertAt: trunc i32 to i8 → store i8 %dest, %idx  (unmade Char)
→ LLVM sees: load i8, store i8  (identity eliminated)
```

---

## Layer 6: Cast#() intrinsic and resolution

`Cast#()` is a **compiler intrinsic** — not user-declarable. When the
programmer writes `(TargetType)source`, the compiler internally emits
`Cast#(source, TargetType)`, which runs the resolution pipeline.

### User-declarable ops

| Op | Direction | Example | When it fires |
|---|---|---|---|
| `op CastTo(#String)` | Source **→** Protocol | `Latin1 → UTF-8` | Source produces protocol currency |
| `op CastFrom(#String)` | Protocol **→** Source | `UTF-8 → ASCII` | Target consumes protocol currency |
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

The protocol path never goes through a **third type** — it goes through the
protocol currency (`Char`, `Float64`), which is a compile-time abstraction,
not a runtime allocation.

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

| Hashword | Protocol shape | Required ops | Universal currency |
|---|---|---|---|
| `#Int` | `i64` | Add, Sub, Mul, Div, CastTo(#Bits), CastFrom(#Bits) | `#Bits` |
| `#Float` | IEEE 754 binary32/64 | Add, Sub, Mul, Div, Sqrt, CastTo(Float64), CastFrom(Float64), CastTo(#Bits), CastFrom(#Bits) | `Float64` |
| `#Bool` | `i1` (stored i8) | And, Or, Not, CastTo(#Bits), CastFrom(#Bits) | `#Bits` |
| `#Char` | Unicode scalar (i32) | CastTo(#Int), CastFrom(#Int), Eq, Lt | `#Int` |
| `#String` | UTF-8 byte sequence | CastTo(#Char), CastFrom(#Char), Extract(#Char), InsertAt(#Char), Concat(#String), :> Size, CastTo(#Bits), CastFrom(#Bits) | `Char` |
| `#Bits` | Raw `iN` | And, Or, Xor, Not, Shl, Shr, CastTo(iN), CastFrom(iN) | (itself) |

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
