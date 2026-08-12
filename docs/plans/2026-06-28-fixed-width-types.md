# Fixed-Width Types + Adaptive Int

**Date:** 2026-06-28
**Status:** Active

## Overview

Add explicit fixed-width integer and float types to Briev's type system, with Rust-like literal suffixes, context inference, and no implicit widening. Then add an analysis pass that makes `Int` and `Float` adaptive — inferring the minimal width from contracts and usage.

---

## Type Alias Table

All keyword sets are interchangeable in source code:

| Aliases | AST variant | LLVM type | Signed? | Width |
|---------|-------------|-----------|---------|-------|
| `Int`, `Int64`, `I64` | `Type::Int` | `i64` | Signed | 64-bit |
| `Int32`, `I32` | `Type::Int32` | `i32` | Signed | 32-bit |
| `Int16`, `I16` | `Type::Int16` | `i16` | Signed | 16-bit |
| `Int8`, `I8` | `Type::Int8` | `i8` | Signed | 8-bit |
| `UInt`, `UInt64`, `U64` | `Type::UInt` | `i64` | Unsigned | 64-bit |
| `UInt32`, `U32` | `Type::UInt32` | `i32` | Unsigned | 32-bit |
| `UInt16`, `U16` | `Type::UInt16` | `i16` | Unsigned | 16-bit |
| `UInt8`, `U8` | `Type::UInt8` | `i8` | Unsigned | 8-bit |
| `Float`, `Float32`, `F32` | `Type::Float` | `float` | N/A | 32-bit |
| `Double`, `Float64`, `F64` | `Type::Float64` | `double` | N/A | 64-bit |

**Key:** `Int`/`UInt` remain the canonical names for 64-bit. `I64`/`UInt64` are aliases.

## Literal Suffixes

| Suffix | Example | Resolved type |
|--------|---------|---------------|
| (none) | `42` | Context-inferred, defaults to `Int` |
| `i8` | `42i8` | `Int8` |
| `i16` | `42i16` | `Int16` |
| `i32` | `42i32` | `Int32` |
| `i64` | `42i64` | `Int` (alias) |
| `u8` | `42u8` | `UInt8` |
| `u16` | `42u16` | `UInt16` |
| `u32` | `42u32` | `UInt32` |
| `u64` | `42u64` | `UInt` (alias) |
| (none) | `3.14` | Context-inferred, defaults to `Float` |
| `f32` | `3.14f32` | `Float` |
| `f64` | `3.14f64` | `Float64` |

No bare `f`/`d` suffixes — only explicit `f32`/`f64`. The lexer avoids ambiguity by consuming the suffix as part of the token.

## Inference Rules

- **Unsuffixed literals** (`42`, `3.14`) create type variables resolved from context (Rust-like). Ambiguous context → default to `Int`/`Float` (backward compatible).
- **Suffixed literals** (`42i32`, `3.14f64`) are fixed to their explicit type.
- **`let x: I8 = 42;`** — `42` inferred as `I8` from context (no suffix needed).
- **`let x: Float64 = 3.14;`** — `3.14` inferred as `Float64` from context (no suffix needed).

## Coercion Rules

- **No implicit widening or narrowing** between numeric types. `I8 + I16` → compile error.
- **All integer↔integer** pairs castable via `as`: `x as I16`, `x as I32`, etc.
- **All integer↔float** pairs castable via `as`: `x as Float`, `x as I32`, etc.
- **Float↔Float64** castable via `as`: `x as Float64`, `x as Float`.
- **Binary ops** require same type: `I32 + I32 → I32`, `Float + Float → Float`, `Int8 + UInt8` → error.

## Type Universe Integration

Primitive types remain as AST enum variants (`Type::Int8`, `Type::Float64`, etc.) — they are NOT registered in `TypeUniverse`. The type universe handles only user-defined `TypeDef` declarations.

A unified width/alignment query on `Type` itself provides the bridge:

```rust
impl Type {
    pub fn bit_width(&self) -> Option<u64> {
        match self {
            Type::Int8 | Type::UInt8 => Some(8),
            Type::Int16 | Type::UInt16 => Some(16),
            Type::Int32 | Type::UInt32 => Some(32),
            Type::Int | Type::UInt | Type::Int64 | Type::UInt64 => Some(64),
            Type::Float => Some(32),
            Type::Float64 => Some(64),
            _ => None,
        }
    }
}
```

The TypeUniverse's `resolve_type_def` can reference this when a TypeDef's base is `"I32"` — it maps back to the AST variant's known width.

---

## Implementation: Phase A — Explicit Types

### Step 1: Lexer (`src/lexer.rs`)

Current integer token:
```rust
#[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
Integer(i64),
```

New suffixed tokens (8 regex patterns):
```rust
#[regex(r"[0-9]+i8",  |lex| lex.slice().trim_end_matches("i8").parse().ok())]  IntegerI8(i64),
#[regex(r"[0-9]+i16", |lex| lex.slice().trim_end_matches("i16").parse().ok())] IntegerI16(i64),
#[regex(r"[0-9]+i32", |lex| lex.slice().trim_end_matches("i32").parse().ok())] IntegerI32(i64),
#[regex(r"[0-9]+i64", |lex| lex.slice().trim_end_matches("i64").parse().ok())] IntegerI64(i64),
#[regex(r"[0-9]+u8",  |lex| lex.slice().trim_end_matches("u8").parse().ok())]  IntegerU8(i64),
#[regex(r"[0-9]+u16", |lex| lex.slice().trim_end_matches("u16").parse().ok())] IntegerU16(i64),
#[regex(r"[0-9]+u32", |lex| lex.slice().trim_end_matches("u32").parse().ok())] IntegerU32(i64),
#[regex(r"[0-9]+u64", |lex| lex.slice().trim_end_matches("u64").parse().ok())] IntegerU64(i64),
```

Current float token:
```rust
#[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse().ok())]
Float(f64),
```

New suffixed float tokens:
```rust
#[regex(r"[0-9]+\.[0-9]+f32", |lex| lex.slice().trim_end_matches("f32").parse().ok())] Float32(f64),
#[regex(r"[0-9]+\.[0-9]+f64", |lex| lex.slice().trim_end_matches("f64").parse().ok())] Float64(f64),
```

New type keyword tokens (18 new):
```rust
#[token("Int8")]    Token::TypeInt8
#[token("I8")]      Token::TypeI8
#[token("Int16")]   Token::TypeInt16
#[token("I16")]     Token::TypeI16
#[token("Int32")]   Token::TypeInt32
#[token("I32")]     Token::TypeI32
#[token("Int64")]   Token::TypeInt64
#[token("I64")]     Token::TypeI64
#[token("UInt8")]   Token::TypeUInt8
#[token("U8")]      Token::TypeU8
#[token("UInt16")]  Token::TypeUInt16
#[token("U16")]     Token::TypeU16
#[token("UInt32")]  Token::TypeUInt32
#[token("U32")]     Token::TypeU32
#[token("UInt64")]  Token::TypeUInt64
#[token("U64")]     Token::TypeU64
#[token("Float32")] Token::TypeFloat32
#[token("F32")]     Token::TypeF32
#[token("Float64")] Token::TypeFloat64
#[token("F64")]     Token::TypeF64
#[token("Double")]  Token::TypeDouble
```

`#[token("Float")] Token::TypeFloat` already exists.

### Step 2: AST (`src/ast.rs`)

Add to `Type` enum:
```rust
Int8, Int16, Int32, Int64,
UInt8, UInt16, UInt32, UInt64,
Float64,
```

Add expression variant for explicit Float64 literals:
```rust
Float64(f64),
```

`Expr::Integer(i64)` stays unchanged — unsuffixed integer literals remain `Type::Int`. `Expr::Float(f64)` stays for unsuffixed and f32-suffixed.

Add `Type::bit_width()` method:
```rust
impl Type {
    pub fn bit_width(&self) -> Option<u64> { ... }
    pub fn is_signed(&self) -> Option<bool> { ... }
}
```

### Step 3: Parser (`src/parser.rs`)

Map suffixed integer tokens to `Expr::Integer` with the appropriate type:
```rust
// In expression parsing:
Token::IntegerI8(n) => { let e = Expr::Integer(n); e.set_type(Type::Int8); e }
Token::IntegerI32(n) => { let e = Expr::Integer(n); e.set_type(Type::Int32); e }
// etc.
```

Map suffixed float tokens:
```rust
Token::Float32(f) => Expr::Float(f),     // same as unsuffixed
Token::Float64(f) => Expr::Float64(f),   // explicit 64-bit
```

Map type keywords to their AST `Type` variants (in `parse_type`):
```rust
Token::TypeI8 | Token::TypeInt8 => Type::Int8,
Token::TypeI16 | Token::TypeInt16 => Type::Int16,
// ... all 22 keywords mapped to 9 AST variants
```

### Step 4: Typechecker (`src/typechecker.rs`)

**Literal inference:**
- `Expr::Integer(n)` in `infer_expression` → currently `Type::Int`. Keep this for unsuffixed.
- For suffixed: the type is already set by the parser, return it directly.
- `LiteralExpr::Integer(_)` → `Type::Int` (unsuffixed default, resolved by context inference).
- `Expr::Float64(_)` → `Type::Float64`.

**Binary ops (`binary_op_type`):**
- `(Int, Int)` → `Int` (unchanged)
- `(Int8, Int8)` → `Int8`
- `(Int32, Int32)` → `Int32`
- `(UInt8, UInt8)` → `UInt8`
- `(Float, Float)` → `Float` (unchanged)
- `(Float64, Float64)` → `Float64`
- Mixed widths → type error. No implicit widening.

**Casts (`is_cast_valid`):**
Add all integer↔integer width pairs:
```rust
(Type::Int8, Type::Int16) | (Type::Int16, Type::Int8) |
(Type::Int8, Type::Int32) | (Type::Int32, Type::Int8) |
// ... all 9×9 = 81 pairs
```

Simplify with a helper: `is_integral(ty) && is_integral(dst)` → true. Same for float:
`is_float_type(ty) && is_float_type(dst)` → true. Cross-family: `is_integral(ty) && is_float_type(dst)` → true.

**`common_type` / type unification:**
- `Int` and `Int8` etc. do NOT unify (different types).
- Unsuffixed literals use type variables that unify with context.

**Signed vs unsigned arithmetic:**
- `Int*` types → signed arithmetic (`sdiv`, `srem`, `icmp slt`)
- `UInt*` types → unsigned arithmetic (`udiv`, `urem`, `icmp ult`)
- `is_signed()` helper on `Type` carries this distinction through codegen.

### Step 5: Interpreter (`src/interpreter.rs`)

- `Expr::Float64(f)` → `Value::Float(f)` (interpreter stores all floats as f64).
- Integer widths: all `Value::Int(i64)` internally (interpreter already uses i64).
- Type distinctions are compile-time only in the interpreter.
- `try_eval_cfloat64`: fold float constants in pure f64 (no f32 truncation).
- `try_eval_cintN`: fold integer constants with overflow checked at the correct width.

### Step 6: LLVM Backend

**Type mapping (`mod.rs:200-208`):**
```rust
match ty {
    Type::Int8 => "i8",
    Type::Int16 => "i16",
    Type::Int32 => "i32",
    Type::Int | Type::Int64 => "i64",
    Type::UInt8 => "i8",
    Type::UInt16 => "i16",
    Type::UInt32 => "i32",
    Type::UInt | Type::UInt64 => "i64",
    Type::Float => "float",
    Type::Float64 => "double",
    // ...
}
```

**Integer literal emission:**
Suffixed integer literals (`42i8`, `42i32` etc.) emit the appropriate LLVM immediate:
- `add i8 0, 42` for I8
- `add i32 0, 42` for I32
- etc.

**Integer arithmetic (`emit_binop`):**
Dispatch on the LLVM type string:
- `i8` → `add i8`, `sub i8`, `mul i8`
- `i16` → `add i16`, `sub i16`, `mul i16`
- etc.
- Signed: `sdiv`/`srem`/`icmp slt`
- Unsigned: `udiv`/`urem`/`icmp ult`
- Dispatch via the existing `TypedRegister.ty` → `llvm_type()` → emitted instruction.

**Float literal emission:**
- `Expr::Float(f)` → `bitcast i32 {f32_bits} to float` (unchanged)
- `Expr::Float64(f)` → `bitcast i64 {f64_bits} to double` (new)
- Add `float64_to_llvm_hex(f: f64) -> String`.

**Float arithmetic:**
- `Type::Float` → `fadd fast float`, `fmul fast float` etc. (unchanged)
- `Type::Float64` → `fadd fast double`, `fmul fast double` etc. (new)

**Intrinsics:**
- `sqrt#(f: Float)` → `@llvm.sqrt.f32` (unchanged)
- `sqrt#(f: Float64)` → `@llvm.sqrt.f64` (new)
- Same for `sin#`, `cos#`, `pow#`, `fabs#`, `ceil#`, `floor#`.

**Print intrinsic:**
- `print_float#(f: Float)` → `fpext float → double` then `fprintf %.9f` (unchanged)
- `print_float#(f: Float64)` → direct `fprintf %.9f` with double (skip fpext)

**State struct fields:**
- Emit appropriate-width LLVM types for struct fields based on field's declared type.
- `I8` field → `i8` in the `%State` struct (was always `i64`).
- This is the big optimization: smaller state structs, better cache behavior.

**Projection fast path (`loop_engine.rs`):**
- `field_types` stores LLVM type strings like `"i8"`, `"i16"`, `"i32"`, `"i64"`, `"float"`, `"double"`.
- `ensure_float_reg` → handle both `float` (f32) and `double` (f64).
- `ensure_int_reg` → handle all integer widths.

### Step 7: Other Backends

**Webstack (`webstack.rs`):**
- All integer types → JS `number` (same as `Int`).
- `Type::Float64` → JS `number` (same as `Float`).

**CIRCT (`circt.rs`):**
- Map new types to MLIR types (`si8`, `si16`, `si32`, `si64`, `f64`).

**GPU/SPIR-V (`gpu.rs`):**
- `is_gpu_safe_intrinsic` → allow `Float64` intrinsics.
- Print buffer sizing for f64 vs f32.

### Step 8: Benchmarks & Stdlib

**Benchmarks:**
- nbody_newton, nbody_sqrt: `const pi: Float64 = 3.141592653589793f64` for full f64 precision matching C.
- All benchmarks using `Float` stay unchanged (still f32, backward compatible).

**Stdlib (`lib/std/types.bv`):**
- Export all type aliases as pass-throughs.

### Step 9: Tests

- **Lexer:** 10 new suffix token patterns.
- **Parser:** Suffix parsing + 22 type keyword mappings.
- **Typechecker:** Same-type binary ops, cross-type cast validation, signed vs unsigned arithmetic, context inference.
- **Interpreter:** `Expr::Float64` evaluation, `try_eval_cfloat64`.
- **LLVM backend:** f64 literal emission, `i8`/`i16`/`i32` arithmetic, signed vs unsigned comparisons, intrinsics dispatch per width.
- **Regression:** All 1300+ existing tests must still pass.

---

## Implementation: Phase B — Adaptive Int/Float

Phase B is a **pure analysis pass** that runs between typechecking and codegen. It transforms `Type::Int` and `Type::Float` into concrete widths by analyzing contracts and usage.

### Inference Engine (`src/analysis/width_inference.rs`)

New file. Walks the typed AST and resolves adaptive types:

```
// Input:  typed AST where some nodes are still Type::Int / Type::Float
// Output: typed AST where all Type::Int / Type::Float are replaced with
//         concrete Type::I8..I64 / Type::Float or Type::Float64

For each expression/variable of adaptive type:
  1. Collect constraints:
     - Contract bounds: [count < 100] → max 99 → 7 bits → I8
     - Explicit casts: x as I32 → needs at least I32
     - FFI signatures: frgn fn expects Int32 → needs I32
     - Binary ops with concrete operand: x + y where y: I16 → x needs I16
  2. Compute minimum safe width:
     - Start with constraint-set minimum (cast/FFI/op requirements)
     - If contract provides a tighter bound, narrow if safe
     - Otherwise: default to I64 / Float (safe, backward compatible)
  3. Replace Type::Int / Type::Float in AST
```

**Key properties:**
- **Monotonic:** Constraints only ever demand ≥ a minimum width. Contracts can prove ≤ a maximum. The resolved width is `max(minimum_required, proven_maximum)`.
- **Conservative:** No contract → I64/`Float`. Unsafe code cannot slip through.
- **Non-breaking:** All existing code compiles identically (backward compatible, `Int` was I64 before, still I64 when unconstrained).

**No codegen changes needed:** Phase A's codegen already handles all widths. Phase B just changes what type annotation the codegen sees.

---

## Verification

```bash
cargo test --lib                         # all tests pass
cargo build --release                    # no warnings
BOUND=5 timeout 10 benchmarks/nbody_newton   # matches C output
BOUND=5 timeout 10 benchmarks/nbody_sqrt     # matches C output
bash benchmarks/build_and_bench.sh --runtime  # no parsing errors
bash benchmarks/build_and_bench.sh --correctness  # all MATCH
```

---

## Ordering

1. **Step 1: Lexer** — 10 new suffix tokens + 22 new type keywords
2. **Step 2: AST** — 9 new Type variants + 1 new Expr variant + `bit_width()`
3. **Step 3: Parser** — token-to-type mappings
4. **Step 4: Typechecker** — inference, binary ops, casts, signed/unsigned
5. **Step 5: Interpreter** — `Expr::Float64`, `try_eval_cfloat64`
6. **Step 6: LLVM backend** — type mapping, codegen, intrinsics, state fields
7. **Step 7: Other backends** — webstack, CIRCT, GPU
8. **Step 8: Benchmarks + stdlib** — nbody to Float64, type exports
9. **Step 9: Tests** — coverage for all of the above
10. **Phase B: Width inference pass** — new analysis file, no codegen changes
