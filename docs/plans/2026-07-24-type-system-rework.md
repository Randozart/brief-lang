# Type System Rework: Protocol-First, Width-Agnostic
## 2026-07-24

## Motivation

The current type system has three architectural problems:

1. **`Bits(N)` as user-facing syntax** — `Bits(8)`, `Bits(64)` expose an internal
   implementation detail. The user should think in protocols (`#Int`, `#String`),
   not byte layouts.

2. **`<:` and `:>` are confusing** — `: Bits` on type declarations reads
   backwards from how protocols work. `:>` for property access is opaque.

3. **Width is declared, not inferred** — `bytes <~ 8` forces 64-bit even when
   the actual values provably fit in 8 bits. The compiler should figure this out.

## The New Syntax

### Type declarations

```briv
// Before:
type Int : Bits { bytes <~ 8; alignment <~ 8; op Add(#Int, #Int); };
type String : Bits { bytes <~ 16; alignment <~ 8; };

// After:
type Int: #Int;
type String: #String;
type i64: Int { bits <~ 64; };
type i32: Int { bits <~ 32; };
type u5: Int { bits <~ 5; };
type MyCustom: #String;    // implements #String protocol
```

### Property access

```briv
// Before:
x .#Size
list .#Size

// After:
x.#Size
list.#Size
```

### Rules

1. **`Bits` is implicit.** Every type is backed by bits. You can't declare
   `type Foo : Bits` or `type Foo: Bits`. Bits is the universal base and
   does not appear in user syntax.

2. **Protocol is the primary classifier.** `type Int: #Int;` means "Int
   implements the #Int protocol." The backend knows how to add, subtract,
   etc. for each protocol.

3. **Derived types inherit protocol.** `type i64: Int { bits <~ 64; };`
   inherits `#Int` from `Int`. Width is the only thing that changes.

4. **Width is inferred unless explicit.** No `bits <~` in metadata means
   the compiler uses value-range analysis to pick the narrowest safe width.
   Explicit `bits <~ 64` locks it to exactly 64 bits.

5. **Types have no fixed layout.** A type like `String` does not have a
   predetermined byte layout. The optimizer picks the representation (inline SSO,
   heap-allocated, rope tree) based on the program's operation profile.
   The protocol contract (`#String`) tells the backend what operations are valid.

6. **No `<:` in type declarations.** Use `:` for both derivation and protocol.
   The right side is a protocol (`#HashWord`) or a type name (parent).

7. **No `:>` in expressions.** Replaced by `.#`.

8. **Difference-only op bodies.** The protocol body is optional. An empty body
   means "all default ops apply." A non-empty body lists only the ops that
   differ from the protocol default:
   ```briv
   type Int: #Int;                             // everything default
   type MyString: String {                      // only override what differs
       op Add(#String) = tree_concat(#L, #R);
   };
   ```

### Protocol flow

```
type declaration
  ├─ protocol (#Int, #String, #Float, …)
  │    └─ backend knows ops for this protocol
  └─ parent (optional, inherits protocol if no protocol given)
       └─ bits <~ N (optional, inferred if absent)
```

## Implementation Plan

### Phase 1: AST + Lexer + Parser

| File | Change |
|------|--------|
| `src/lexer.rs` | Add `DotHash` token (`.#`). Keep `ColonGreaterThan` for transition. |
| `src/ast/top.rs` | `TypeDef`: rename `base` → `parent`, add `protocol: Option<String>` |
| `src/ast/expr.rs` | Add `ProtocolGet` expression variant for `x.#prop` |
| `src/parser/definitions.rs` | `parse_type_body`: `:` instead of `<:`, parse protocol hashword |
| `src/parser/expressions.rs` | `parse_protocol_get` for `.#` syntax, replace `:>` handling |
| `src/parser/helpers.rs` | Remove `keyword_as_identifier` mapping for `ColonGreaterThan`? |

### Phase 2: Update TypeDef construction sites

All files that create `TypeDef { base: ... }` need to use `parent` + `protocol`.
This includes the normalizer, layout optimizer, tests, and all backends.

### Phase 3: Update all `.bv` files

- `lib/std/types/bootstrap.bv` — new syntax for all primordials
- `lib/std/*.bv` — all type declarations
- `lib/glue/*.bv` — all type declarations
- `lib/ffi/*.bv` — no type declarations (these use export defn)
- `examples/*.bv` — any with type declarations
- `benchmarks/*.bv` — any with type declarations

### Phase 4: Update property access

All `:>` usages in `.bv` files → `.#`, and the Rust handler for `:>` → `.#`.

### Phase 5: Test and validate

- `cargo test --lib` — all tests pass
- `make -C benchmarks/metropolitan run` — benchmarks still correct
