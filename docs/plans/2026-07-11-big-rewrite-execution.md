# Pure Bits Refactor — The Big Rewrite

**Plan doc:** `docs/plans/2026-07-11-pure-bits-refactor.md`  
**Execution doc (this file):** `docs/plans/2026-07-11-big-rewrite-execution.md`  
**Thesis:** `docs/architecture/bits-thesis.md`  
**Status:** Execution — post-compaction reference

---

## Summary

Rewrite the 12 most deeply-nested files in the compiler **in place or as
split modules**, enforcing max 2 nesting depth throughout, using Bits-only
`Value::Bits(Vec<u8>)`, and implementing the three-token-form architecture
(`Quoted`, `Bare`, `Decimal`) with the `formatting <~` codec property.

After this rewrite:
- No file exceeds 3000 lines (breaking the 14K `interpreter.rs` monolith)
- Every function has max 2 nesting levels (guard clauses, `?`, extracted helpers)
- No `Value::Int`, `Value::Float`, `Value::String`, `Value::Bool`, `Value::Char`, `Value::Ptr` exist anywhere in `src/`
- `Expr::String` is renamed to `Expr::Quoted(Vec<u8>)`; `Expr::Integer` to `Expr::Decimal(i64)`
- Codecs use `formatting <~ Quoted | Bare | Decimal` (capitalized identifiers, not strings)
- The `@` prefix lexer rule converts any token to `Expr::Quoted(raw_bytes)`
- The type checker validates `formatting` on assignment, not hardcoded type names
- All tests pass; all benchmarks match

---

## Non-Negotiable Rules

Every function in every file touched by this rewrite must follow these rules.
Any code that violates them must be rewritten before merge.

### Rule 1: Max 2 Nesting Depth

No function body may exceed 2 levels of indentation. Arrow-shaped code is
forbidden. Use these patterns instead:

```rust
// ✅ GOOD: Guard clause + extracted helper
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 { return None; }
    Some(result)
}

// ❌ BAD: 4 levels deep
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {
        if let Some(result) = val.as_i64() {
            if result > 0 {
                return Some(result);
            }
        }
    }
    None
}
```

| ✅ Allowed | ❌ Forbidden |
|---|---|
| `let val = expr?;` | `if let Some(v) = opt { if let Ok(x) = v { ... } }` |
| `if !condition { return; }` | `if cond { if other { ... } else { ... } }` |
| `let result = match expr { A => a, B => b };` | `match x { A => match y { B => ..., C => ... } }` |
| Helper functions extracted from match arms | Match arms with >10 lines inline |
| `for item in items { helper(item)?; }` | `for item in items { if let Some(x) = item { ... } }` |

### Rule 2: Bits-Only Value Usage

`Value::Int`, `Value::Float`, `Value::String`, `Value::Char`,
`Value::Bool`, `Value::Ptr` have been DELETED. They do not exist in the
Value enum. Every representational value is `Value::Bits(Vec<u8>)`.

Construct using:
```rust
Value::Bits(i64_to_bits(n))           // from i64
Value::Bits(f64_to_bits(f))           // from f64
Value::Bits(s.into_bytes())           // from String
Value::Bits(vec![1u8])                // from bool true
Value::Bits((c as u32).to_le_bytes()) // from char
```

Extract using:
```rust
value_as_i64(&val)          // Bits → Option<i64>
value_as_f64(&val)          // Bits → Option<f64>
value_as_bool(&val)         // Bits → Option<bool>
String::from_UTF8_lossy(b)  // Bits → Cow<str> (FFI string params)
```

### Rule 3: `formatting <~` Uses Identifiers, Not Strings

```rust
formatting <~ Quoted    // ✅ correct — frontend-intrinsic identifier
formatting <~ "quoted"  // ❌ wrong — old string form, removed
```

The three recognized identifiers: `Quoted`, `Bare`, `Decimal`.

### Rule 4: `Expr::String` → `Expr::Quoted`, `Expr::Integer` → `Expr::Decimal`

These renames are required. All existing AST matches on `Expr::String` must
become `Expr::Quoted`. All existing matches on `Expr::Integer` must become
`Expr::Decimal`.

---

## Files to Rewrite — Order & Module Structure

### 1. `src/features/binary_op.rs` (320 lines → ~150 lines)

**Current state:** 50+ dead match arms for typed variants (`(Add, Value::Int(a), Value::Int(b))`).
Most are unreachable since literals produce `Value::Bits`.

**Rewrite:** Strip all typed arms. Keep only:
- `try_bits_dispatch()` — property lookup for Bits operands (already written)
- `legacy_normalize()` — temporary shim for untyped contexts (removable after audit)
- `ty_bits_concat()` helper — concatenates two `Value::Bits` as UTF-8 (for String+String etc.)
- `_ =>` error catch-all
- All Char×Int/Int×Char/Char×Char comparison arms are dead — delete them

**Flat code requirement:** The `evaluate()` function must be exactly:
1. Evaluate left and right via `ctx.eval_expr()` (2 calls, flat)
2. `if let Some(result) = try_bits_dispatch(...) { return result; }` (guard clause)
3. `legacy_normalize()` left and right (2 calls, flat)
4. `match (self.kind, &l, &r) { ... }` — one level, each arm is `value => Value::Bits(...)` or `value => Err(...)`

### 2. `src/features/unary_op.rs` (148 lines → ~80 lines)

Same pattern. Strip typed arms. Keep only:
- `try_unary_bits_dispatch()` — property lookup
- `(Neg, Value::Bits(b))` — inline negation via `bits_to_i64`
- `(Not, Value::Bits(b))` — inline not via `b.first().map_or(true, |x| *x == 0)`
- `_ =>` error catch-all

### 3. `src/features/projection.rs` (297 lines)

**Current state:** Already flat-ish but has dead `Value::Int`/`Value::String` arms.

**Rewrite:** Split into:
- `src/features/projection/mod.rs` — `Projection::evaluate()` dispatch
- `src/features/projection/scalar.rs` — Size, Bytes, Alignment, Popcount,
  LeadingZeros, TrailingZeros, Absolute, BitReverse, Width, Type,
  BitRange — each is a `pub fn` returning `Result<Value>`, max 2 levels
- `src/features/projection/collection.rs` — Front, Back, Contains, IsEmpty,
  Top, AsStack, AsQueue — each flat, using `value_as_i64`

### 4. `src/ffi/types.rs` (200 lines) + `src/ffi/dynamic.rs` (200 lines)

**Rewrite:** Strip all typed variant matches. `from_interpreter_value` and
`to_interpreter_value` work exclusively with `Value::Bits(bytes)`, using
byte-width to determine the FFI type (4 bytes = u32, 8 bytes = u64/f64, etc.).

**Flat code:** Each conversion is a single `match bytes.len() { 1 => ..., 4 => ..., 8 => ... }`.

### 5. `src/ffi/registry.rs` (1507 lines)

**Current state:** Monolithic FFI dispatch. ~40 impl functions for strings,
encoding, JSON, HTTP, file I/O — all scattered in one file with deep `if let
Value::String(s)` chains.

**Rewrite:** Split into 4 files:
- `src/ffi/strings.rs` — concat, contains, starts_with, ends_with, replace,
  trim, split, find, string_append, string_concat_all, string_size
- `src/ffi/encoding.rs` — to_string, to_int, to_float, parse_int,
  parse_float, to_binary, to_hex
- `src/ffi/json.rs` — json_is_string, json_is_object, json_serialize,
  json_deserialize, json_format
- `src/ffi/io.rs` — file_read, file_write, http_get, http_post, exec_cmd

Each impl function takes `&[Value]` and returns `Result<Value>`. Inside each:
```
let bits = match &args[0] { Value::Bits(b) => b, _ => return Err(...) };
// operate on bytes — use String::from_UTF8_lossy() where strings needed
Ok(Value::Bits(result_bytes))
```

No function exceeds 2 levels. No `if let Value::String(s)` — that variant
doesn't exist.

### 6. `src/interpreter.rs` (14,648 lines → 6 files, each < 3000 lines)

**The main monolith.** Split into:

| New file | Contents | Approx LOC |
|---|---|---|
| `src/interpreter/mod.rs` | `Value` enum, `PartialEq`, `Display`, `VirtualHeap`, `execute_intrinsic()`, `value_as_i64/ f64/ bool`, `i64_to_bits`, `f64_to_bits`, re-exports of sub-modules | 400 |
| `src/interpreter/eval.rs` | `eval_expr()` — top-level dispatch. One match arm per `Expr` variant. Each arm body that exceeds 5 lines is extracted into a named helper function (e.g. `eval_arrow`, `eval_cast`, `eval_call`) | 3000 |
| `src/interpreter/intrinsics.rs` | The intrinsic dispatch match block (~3160-5600 in old file). `execute_intrinsic(name, args)` — one arm per intrinsic name, each arm flat | 3000 |
| `src/interpreter/ffi.rs` | FFI dispatch functions that were inline in the old eval. `dispatch_ffi(name, args)` — delegates to `ffi/strings.rs`, `ffi/encoding.rs`, etc. | 2500 |
| `src/interpreter/cells.rs` | Cell/thread logic: `tick_persistent_cells`, `register_persistent_cell`, cell convergence, thread management | 2000 |
| `src/interpreter/casts.rs` | `eval_cast()` — type conversion dispatch. `cast_to_int`, `cast_to_float`, `cast_to_string`, `cast_to_bool`, `cast_to_char`, `cast_to_ptr`, identity casts | 800 |

**Flat code requirement for eval.rs:**
```
pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Integer(n) => Ok(Value::Bits(i64_to_bits(*n))),
        Expr::Float64(f) => Ok(Value::Bits(f64_to_bits(*f))),
        Expr::String(s) => Ok(Value::Bits(s.as_bytes().to_vec())),
        Expr::Identifier(name) => self.eval_identifier(name),
        Expr::Add(l, r) => BinaryOpExpr::new(Add, *l, *r).evaluate(self),
        Expr::Call(fn_name, args) => self.eval_call(fn_name, args),
        Expr::Block(stmts, last) => self.eval_block(stmts, last),
        // ... one line per variant. No inline logic exceeding 5 lines.
        // Complex variants call named helpers: self.eval_arrow(...)
    }
}
```

Each helper function (`eval_call`, `eval_identifier`, `eval_arrow`, etc.)
is independently testable, flat (max 2 levels), and has a doc comment.

### 7. `src/type_universe.rs` (2012 lines)

**Rewrite:** Split into:
- `src/type_universe/mod.rs` — `TypeUniverse` struct, `new()`, `get()`, re-exports
- `src/type_universe/resolve.rs` — `resolve_type_def()`, `apply_binding()`, phase processing
- `src/type_universe/validate.rs` — `validate_primitives()`
- `src/type_universe/operators.rs` — `get_operator_intrinsic()`, `get_formatting_type()`

### 8. `src/main.rs` (5241 lines)

**Rewrite:** Fix `value_to_expr()` and `resolve_deferred_literals()` to
use Bits-only matching. The `resolve_deferred_literals` function is
already flat (max 2 levels). `value_to_expr` needs the match arms updated
from `Value::Int(i)` → `value_as_i64`, `Value::String(s)` →
`String::from_UTF8_lossy`, etc.

### 9. Token Form Refactor (AST rename pass)

After all files above compile, apply `Expr::String → Expr::Quoted` and
`Expr::Integer → Expr::Decimal` across the ENTIRE codebase:

1. Rename the variants in `src/ast.rs`
2. Fix all match/construction sites — build will reveal every one (~200 hits)
3. The `Expr::Integer` → `Expr::Decimal` rename is mechanical (same backing type)
4. The `Expr::String` → `Expr::Quoted` rename changes the backing type from `String` to `Vec<u8>`. All sites must change from `s.to_string()` to `String::from_UTF8_lossy(b)`
5. Add `@` lexer rule in `src/lexer.rs` — reads the next token, produces `Expr::Quoted(raw_source_bytes)`

### 10. `formatting <~` Property Implementation

1. Add `formatting: Option<Vec<String>>` to `CodecDeclaration` in `src/ast.rs`
2. Parse `formatting <~ Quoted | Bare | Decimal` and `formatting <~ [Quoted, Bare, Decimal]` in `parse_codec_decl()` in `src/parser.rs`
3. Add `get_formatting_types(ty_name, universe) -> Vec<&str>` to `TypeUniverse` that resolves a type → its codec → the `formatting` property
4. In the type checker's assignment validation: before allowing `Expr::Quoted` to be assigned to `T`, check `get_formatting_types(T)` contains `"Quoted"`
5. Remove old hardcoded `ty == Type::string() || ty == Type::int()` checks

### 11. Test Files

After all production code compiles, fix test assertions in:

| File | Fix |
|---|---|
| `src/backend/llvm/tests.rs` | `Value::Int(n)` → `Value::Bits(n.to_le_bytes().to_vec())` |
| `src/features/stmt/assignment.rs` | Same |
| `src/features/macros/macro_.rs` | Same |
| `src/interpreter.rs` test blocks | All deleted-variant references → Bits equivalents |
| `src/proof_engine.rs` | Same |
| `src/symbolic.rs` | Same |

---

## Execution Order

All phases are independent of each-other. Do NOT try to fix 27 errors
incrementally — rewrite whole files at once.

```
Phase 0: Rewrite binary_op.rs (+ unary_op.rs) — 2 files, ~1 hour
Phase 1: Rewrite projection/ — split into mod + scalar + collection — 1 hour
Phase 2: Rewrite ffi/types.rs + ffi/dynamic.rs — 30 min
Phase 3: Split ffi/registry.rs into 4 modules — 2 hours
Phase 4: Split interpreter.rs into 6 modules — 4-5 hours
Phase 5: Split type_universe.rs into 4 modules — 1 hour
Phase 6: Fix main.rs value_to_expr — 30 min
Phase 7: Token form rename (Expr::String→Quoted, Expr::Integer→Decimal) — 2 hours
Phase 8: formatting <~ property — parser + type checker — 1 hour
Phase 9: Fix test files — 1 hour
Phase 10: Intrinsic enum removal (Phase 8G) — 2 hours (post-rewrite, builds on Phase 8F)
```

Each phase:
1. `cargo build` — must pass with 0 errors
2. `cargo test --lib` — must pass
3. No warnings from the new code

---

## Verification

```
cargo build                                          # 0 errors, 0 warnings
cargo test --lib                                     # all pass (exact count unchanged)
cargo build --release                                # no warnings
bash benchmarks/build_and_bench.sh --correctness    # all benchmarks match

# Audit: no deleted Value variants remain in production code
grep -rn "Value::Int\|Value::Float\|Value::String\|Value::Bool\|Value::Char\|Value::Ptr" src/ --include="*.rs" | grep -v "/archive/" | grep -v "dead " | grep -v "// deprecated" || echo "CLEAN"

# Audit: no Expr::String or Expr::Integer remain (except in archive)
grep -rn "Expr::String(" src/interpreter.rs src/features/ src/ffi/ src/typechecker.rs src/main.rs src/parser.rs | grep -v "// " || echo "CLEAN"
```

---

## Phase 8G: `#` Intrinsic Architecture

**Replaced by `docs/plans/2026-07-12-intrinsic-architecture.md`.**

This phase has been redesigned. The `#` suffix is now a first-class lexical
character in identifiers — `Sqrt#(x)` parses as `Expr::Call("Sqrt#", [x])`
with no `Intrinsic` enum, no `Expr::IntrinsicCall` AST node, no `inop`.

See the new plan document for the full 12-step execution:
[`docs/plans/2026-07-12-intrinsic-architecture.md`](./2026-07-12-intrinsic-architecture.md)

---

## What Success Looks Like

After this rewrite, the codebase is:
- **Flat:** Every function has max 2 nesting levels. No arrow-shaped code.
- **Bits-only:** `Value::Bits(Vec<u8>)` is the only representational Value variant.
- **Token-aware:** `Expr::Quoted`, `Expr::Decimal`, `Expr::Identifier(Bareword)` are the three literal forms.
- **Property-driven:** `formatting <~ Quoted | Bare | Decimal` on codecs controls which types accept which literals.
- **No name magic:** The type checker doesn't check `Type::Custom("String")` — it checks the codec's `formatting` property.
- **Modular:** No file exceeds 3000 lines. `interpreter.rs` is 6 files. `ffi/registry.rs` is 4 files.
- **Tested:** Full test suite passes. Benchmarks match.

---

## Reference: Design Decisions (Post-Compaction Context)

### Three Token Forms (Compiler Axioms)

| Form | AST node | Source | `formatting <~` value |
|---|---|---|---|
| QuotedValue | `Expr::Quoted(Vec<u8>)` | `"..."` | `Quoted` |
| DecimalValue | `Expr::Decimal(i64)` | `[0-9]+`, `[0-9]+\.[0-9]+` | `Decimal` |
| Bareword | `Expr::Identifier(String)` | `[a-zA-Z][a-zA-Z0-9_]*` | `Bare` |

These are compiler-intrinsic — the lexer must produce SOMETHING, and these
three forms are the complete syntactic surface for literal values.

### `@` Prefix Modifier

`@` before any token produces `Expr::Quoted(raw_source_bytes)`. The lexer
reads the next token, discards semantic interpretation, and wraps the
raw source bytes as a QuotedValue. This disambiguates variable vs. custom
literal: `let x: Color = @FF00FF` is unambiguously a literal.

### `formatting <~` Property

A codec declares which token forms it accepts:

```briev
codec HexCodec {
    formatting <~ Bare;
    parse      <~ parse_hex;
};
```

Valid values are the identifiers `Quoted`, `Bare`, `Decimal` (capitalized,
not quoted strings). Multiple forms: `formatting <~ [Quoted, Bare]`.

### Identifier-vs-String Rule

- **Identifier values** (`formatting <~ Quoted`) → frontend must understand
  this to compile the program. The type checker matches on it.
- **String values** (`llvm <~ "%String"`) → opaque to frontend. Only the
  backend interprets it.

### `op ParseFrom*` Is Not Used

The token form dispatch goes through `formatting <~` on the codec, not
through `op ParseFromBareword`. The `op` system is for runtime operations
(`op Add`, `op Drop`). Literal parsing is a compile-time property, not a
runtime operator. The codec's `parse <~` handler converts the token text
to Bits at compile time.

### Bootstrap Codec Structure

```briev
codec DefaultQuoted { formatting <~ Quoted; parse <~ identity; };
codec DefaultDecimal { formatting <~ Decimal; parse <~ parse_decimal; };
codec DefaultBare { formatting <~ Bare; parse <~ parse_bare; };

type Int    : Bits { codec <~ DefaultDecimal; bytes <~ 8; llvm <~ "i64"; };
type Float  : Bits { codec <~ DefaultDecimal; bytes <~ 8; llvm <~ "double"; };
type String : Bits { codec <~ DefaultQuoted; bytes <~ 24; llvm <~ "%String"; };
```

No name-based magic. `String` accepts `"..."` because `DefaultQuoted`
declares `formatting <~ Quoted`, not because the type is named `String`.

### What the `formatting` System Replaces

| Old (hardcoded) | New (property-driven) |
|---|---|
| `Type::Custom("Int")` matches `Expr::Integer` | `DefaultDecimal.formatting <~ Decimal` |
| `Type::Custom("String")` matches `Expr::String` | `DefaultQuoted.formatting <~ Quoted` |
| Type checker checks type names | Type checker checks codec properties |

The old hardcoded rules are REMOVED, not supplemented. The type checker
must use the `formatting` property exclusively for token assignments.

---

### Compiler Primitives

These are the capitalized, PascalCase identifiers that the compiler
recognizes in specific metadata contexts. They are NEVER user-definable
and NEVER appear in string quotes.

**Token form primitives** (used in `formatting <~`):

| Primitive | Meaning |
|---|---|
| `Quoted` | Accepts `"..."` token form |
| `Bare` | Accepts bare identifier token form (`FF00FF`) |
| `Decimal` | Accepts numeric token form (`42`, `3.14`) |

**Operator primitives** are now handled by the `#` intrinsic architecture.
See `docs/plans/2026-07-12-intrinsic-architecture.md` for the full design.
The `intrinsic_op <~` convention has been replaced by the `#` suffix on
function identifiers — `Sqrt#(x)` parses as a standard `Expr::Call`.
