# Decision Log & Implementation Plan
**2026-05-27**

## Language Decisions

### Tuple Field Access: `tuple.N`
- **Decision**: `tuple.0`, `tuple.1` — dot-integer syntax
- **Rationale**: Field access with numeric field names. No different from `struct.field`. Position `0` is traceable to the tuple's type declaration `(String, Int)`.
- **Rejected**: `first(tuple)` (mystical built-in), `tuple[0]` (index semantics mismatch), destructuring-only (overly restrictive)

### Logical OR: `||`
- **Decision**: `||` operator in expressions, mapping to `Expr::Or`
- **Rationale**: Briev already has `Expr::And` with `&&`. Symmetric operator for OR is idiomatic.
- **Status**: Lexer has `OpOr` token, parser has `Expr::Or` — just needs `||` wired through `parse_or_expr`

### List Concatenation: `++`
- **Decision**: `++` operator for list concatenation
- **Rationale**: Common functional language idiom. Distinct from arithmetic `+`. Briev lists are immutable, so `++` produces a new list.
- **Status**: New parser construct needed.

### Rejected: `::` path separator, `*` dereference
- **Not Briev features**. `Box::new(x)` and `*expr` in proof_engine.bv are Rust-isms that leaked in during porting. Fix: `Box(x)` and `expr`.

## Implementation Order

1. Add `||` to parser (smallest change, already partially supported)
2. Add `.N` tuple field access to parser
3. Add `++` list concat to lexer + parser
4. Fix Briev source files (Rust-ism removal): proof_engine.bv, main.bv, typechecker.bv, call_graph.bv, range.bv, parser.bv
