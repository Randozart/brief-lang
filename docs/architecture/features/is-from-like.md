# is, from, like — Type/Metadata Check Expressions

**Date**: 2026-06-14
**Phase**: 15

## Syntax

Three binary infix operators, all returning `Bool`:

```briev
x is Int           // true if x's runtime type is Int
x is Some          // true if x's enum variant tag is Some
x is Option<Int>   // true if x is an Option<Int>

x from Collection  // true if x's type derives from Collection (runtime)
Int from Data      // true at compile time (Int does not derive from Data)
T from Collection  // folded at monomorphization time

x like y           // true if x and y have structurally identical layouts
T like U           // true at compile time if both types have the same shape
```

## Precedence

`is`, `from`, `like` bind tighter than `==`/`!=` but looser than unary `!`:

```
!x is Some    → !(x is Some)
x is Some == true → (x is Some) == true
x from T == false → (x from T) == false
```

## AST

New types in `src/ast.rs`:

```rust
pub enum IsTarget {
    Type(Type),      // Int, Option<Int>, Foo, : derived type
    Variant(String), // Some, None, Ok, Err — resolved against LHS enum type
}
```

New `Expr` variants:

```rust
IsType(Box<Expr>, IsTarget),      // x is Int / x is Some
FromCheck(Box<Expr>, Type),       // x from Foo
Like(Box<Expr>, Box<Expr>),       // x like y
```

## Lexer

Two new tokens:

```rust
#[token("is")]
Is,
#[token("like")]
Like,
```

`Token::From` already exists (used in `import`/`frgn`/`sig`).

## Parser

### `parse_is_target()`

Parses the RHS of `is`:
- `some`, `none`, `ok`, `err` tokens → `IsTarget::Variant(name)` (case-sensitive: only lowercase)
- `parse_type()` → `IsTarget::Type(type)`
- User-defined enum variants are identifiers that parse as `Type::Enum(name)` or `Type::Custom(name)` — the typechecker resolves whether the RHS is a type or a variant of the LHS's enum type.

### Expression precedence

A new parse level between unary and equality, implemented in `parse_check()` (called from the refactored `parse_equality()`):

```
parse_expression
  → parse_or           (||)
    → parse_and        (&&)
      → parse_equality (==, !=)
        → parse_check   (is, from, like)    ← NEW
          → parse_comparison (<, >, <=, >=)
            → parse_shift  (<<, >>)
              → parse_additive (+, -)
                → ...
```

`parse_check` consumes the LHS, then loops for `is`/`from`/`like`:

```rust
fn parse_check(&mut self) -> Result<Expr, SyntaxError> {
    let mut left = self.parse_comparison()?;
    while let Some(token) = self.current_token() {
        match token {
            Ok(Token::Is) => {
                self.advance();
                let target = self.parse_is_target()?;
                left = Expr::IsType(Box::new(left), target);
            }
            Ok(Token::From) => {
                self.advance();
                let ty = self.parse_type()?;
                left = Expr::FromCheck(Box::new(left), ty);
            }
            Ok(Token::Like) => {
                self.advance();
                let right = self.parse_comparison()?;
                left = Expr::Like(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}
```

`Token::From` in expression position only matches here. In `import`/`frgn`/`sig` statements, `from` is consumed by those dedicated parse paths before reaching `parse_expression`.

### Lexer notes

`Is` and `Like` are lexer keywords (`Token::Is`, `Token::Like`). `Some` variants (`some`, `none`) are also keywords — note that `ok` and `err` are full uppercase `Ok`/`ERR` while `some` is only lowercase `some`/`SOME`. Tests must use the exact case the lexer tokenizer recognizes.

## Typechecker

All three return `Type::Bool`. Currently the typechecker's `infer_expression` returns `Type::Bool` for all three variants without deeper checking — this is correct for the initial implementation.

### `IsType(lhs, Type(ty))`

- Runtime types are determined by the interpreter's `Value` variant matching (see Interpreter section).
- The typechecker does not fold compile-time-known type checks yet.

### `IsType(lhs, Variant(v))`

- No compile-time variant resolution yet — always returns `Type::Bool`.
- Future work: resolve `v` against LHS's enum type, error if invalid.

### `FromCheck(lhs, ty)`

- No compile-time derivation chain walking yet — always returns `Type::Bool`.
- Runtime check walks the struct/enum typename at evaluation time.

### `Like(lhs, rhs)`

- No structural compatibility checking yet — always returns `Type::Bool`.
- Runtime check performs recursive structural comparison.

## Interpreter

### `IsType(val, Type(ty))`

Match `val`'s `Value` variant against `ty`:
- `Value::Int` ↔ `Type::Int`
- `Value::Float` ↔ `Type::Float`
- `Value::Bool` ↔ `Type::Bool`
- `Value::String` ↔ `Type::String`
- `Value::List(..)` ↔ `Type::Vector(..)` or generic `List<T>`
- `Value::Instance { typename, .. }` ↔ struct type name match
- `Value::Enum(ename, ..)` ↔ enum type name match

### `IsType(val, Variant(name))`

If `val` is `Value::Enum(_, variant_name, _)`: compare `variant_name` against the target name.

### `FromCheck(val, ty)`

If `val` is `Value::Instance { typename, .. }` or `Value::Enum(typename, ..)`: compare the type name against the target type (formattted with `{:?}`). Non-composite types return `false`.

### `Like(lhs, rhs)`

Recursive structural comparison:
- Same `Value` discriminant → continue
- Int/Float/Bool/String/Char: `==`
- List: same length, recursive `like` on each element
- Instance: same field count, recursive `like` on each field value (key equality enforced)
- Enum: same type and variant names, recursive `like` on payload fields
- Tuple: element-by-element
- Different types: `false`

## LLVM Backend

### `IsType`

- **Stub**: emits `add i64 0, 1 ; is type (compile-time)` — types are fully known at compile time, so the check trivially passes for any well-typed expression. The comment indicates the target variant or type name.
- **Future**: for enum discriminant checks, load the discriminant field, `icmp eq` against the variant tag, `zext` to `i64`.

### `FromCheck`

- **Stub**: emits `add i64 0, 1 ; from (compile-time)` — same reasoning as `IsType`.
- **Future**: load struct's type-id field, compare against parent type-id.

### `Like`

### `Like`

- **Delegates to `emit_fcmp`** with `oeq` — handles integer and float equality with compile-time constant folding (for integer literals). For compound types, this is a structural approximation.
- **String trigger special case**: When comparing a linked String trigger variable against a string literal, compares the trigger's byte value against the first byte of the literal (0 for `""`). This is handled before the general `emit_fcmp` logic.
- **Future**: recursive field-by-field `icmp eq` chain, `and` all results.

## Contract/Guard Usage

These expressions enable type-directed branching in contracts and guards:

```briev
defn process<T>(x: T) -> String {
    [T is Int]    { term int_to_string(x); };
    [T is String] { term x; };
    term "unknown";
};
```

At monomorphization time, `T is Int` and `T is String` fold to compile-time constants, and the dead guard body is pruned by dead-field elimination:

```briev
// monomorphized for T = Int:
defn process(x: Int) -> String {
    [true]   { term int_to_string(x); };  // live
    [false]  { /* pruned */ };
    term "unknown";  // also pruned (unreachable after true guard)
};
```

## Key Notes

- **`from` is a check, not a declaration**: The derivation declaration syntax is `struct Bar : Foo { ... }` using `<:`. The word `from` is now exclusively an infix check operator in expressions.
- **Variant name resolution**: For `x is Some`, the compiler resolves `Some` against the enum type of `x`'s type. If `x: Option<Int>`, then `Some` resolves to the first variant of `Option`. Error if no match.
- **`like` is structural, not nominal**: Two types can be `like` each other even if they have different names, as long as their field layout matches. Pointer-to-value derivation (`Int .#Ptr`) can be checked via `derived_ptr like parent_int`.
- **Lexer capitalization**: `Is` and `Like` tokens are case-insensitive. `Some`/`None` variant tokens are only `some`/`SOME`/`none`/`NONE` (lowercase). `Ok` and `Err` support capitalized forms as well (`Ok`/`OK`/`Err`/`ERR`). Tests must use the exact case the lexer tokenizer recognizes.

## Implementation Status

| Component | Status | Details |
|-----------|--------|---------|
| Lexer | ✅ | `Is`, `Like` tokens; existing `From` token |
| AST | ✅ | `IsType`, `FromCheck`, `Like` Expr variants; `IsTarget` enum |
| Parser | ✅ | `parse_check()` + `parse_is_target()` in precedence chain |
| Symbolic execution | ✅ | Returns `Unknown` for all three |
| Proof engine | ✅ | `collect_identifiers` recurses into sub-expressions |
| Typechecker | ✅ | Returns `Type::Bool`, recurses through check/ffi visitors |
| Interpreter | ✅ | Full runtime evaluation for all three |
| LLVM backend | ⚠️ | Stubs emit `add i64 0, 1` (compile-time true); `Like` delegates to `emit_fcmp` |
| Tests | 9 | 5 parser + 4 LLVM backend (794 total passing) |

### Future Work

1. **LLVM backend runtime checks**: Load discriminant for `IsType(variant)`, load type-id for `FromCheck`, recursive comparison for `Like`
2. **Typechecker compile-time folding**: Fold `42 is Int` → `true` at type-check time
3. **Variant resolution**: Resolve user-defined enum variant names (currently only `Some`/`None`/`Ok`/`Err` keywords are recognized; arbitrary identifiers fall through to type parsing)
4. **Struct `from` check**: Walk the derivation chain at compile time in the typechecker
5. **`like` structural type compatibility**: Error on incompatible types at type-check time
