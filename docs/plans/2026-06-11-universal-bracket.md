# Universal Bracket Syntax: SIMD Protocol for All Types

**Date:** 2026-06-11 22:00 UTC

## Overview

Extend bracket syntax (`[]`) to work universally on **every type**, not just
collections and strings. Every value is treated as a sequence of visual
fragments. Bracket operations select, filter, stride, or transform these
fragments. The result reconstructs to the original type.

This makes SIMD vectorization a natural consequence of bracket operations —
uniform element-wise transforms are trivially vectorizable.

## Core Principle

> **Every value is a sequence of visual fragments. Bracket syntax selects
> or transforms those fragments. The result reconstructs to the original type.**

The "visual representation" is the canonical display form of the value.
For example, `15561` decomposes to `"15561"`, whose fragments are
`[Char('1'), '5', '5', '6', '1']`.

## Fragment Decomposition Table

| Type | Fragment Type | Fragment Source | Read-only? |
|------|--------------|----------------|------------|
| `String` | `Char` | Characters | No (ArrowAssign works) |
| `List<T>` | `T` | Elements | No |
| `HashMap<K,V>` | `(K,V)` entry | Entries | No |
| `HashSet<T>` | `T` | Elements | No |
| `Stack<T>` | `T` | Elements | No |
| `Queue<T>` | `T` | Elements | No |
| `Tuple` | element types | Elements | Yes |
| `Struct` | `(String, Value)` | Fields | Yes |
| `Int` | `Char` | Visual digits | Yes |
| `Float` | `Char` | Visual repr | Yes |
| `Bool` | `Char` | Visual repr | Yes |
| `Char` | `Char` | Itself | Yes |

## Dispatch Rules

### Coord access

| Value type | `val[coord]` | Example |
|-----------|-------------|---------|
| Collection | Index into elements | `list[3]` → 4th element |
| Atomic (Int/Float/Bool/Char) | Index into visual chars | `15561[0]` → `Char('1')` |

### Filter with predicate

| Value type | `val[;pred]` | Example |
|-----------|-------------|---------|
| Collection | Filter elements by predicate | `list[;_ > 5]` |
| Atomic | Filter visual chars | `15561[;=='5']` → `Char('1')`, `'6'`, `'1'` → `Int(161)` |

### String/regex in brackets

| Value type | `val[string_expr]` (bare, no `;`) |
|-----------|-----------------------------------|
| Collection | Coord access (existing behavior — key lookup for HashMap, error for List with string index) |
| Atomic (Int/Float/Bool/Char/String) | Desugars to `val[;@"string_expr"]` — regex filter on stringified value |

The desugar rule: if brackets contain exactly one argument, that argument is a
string literal or a `const` string variable, and the value type is **not** a
collection → treat as `[;@"pattern"]` (regex filter).

**Collection exception**: `map["key"]` on `HashMap<String,V>` remains a key
lookup. Collection bracket access is never implicitly regex.

### `@"pattern"` — regex literal

A new expression form producing a `Value::Regex`. Used in filter position
(`[;@"pattern"]`) to match against the stringified representation of each
fragment (for per-element) or the whole value (for multi-fragment patterns).

**DFA compilation**: Compile-time-known `@"..."` literals are compiled to a
deterministic finite automaton via the existing `analysis::dfa` module at parse
time. Runtime regex becomes O(n) table walk with zero allocation.

### Arrow assignment with bracket

| Construct | Meaning | Example |
|-----------|---------|---------|
| `&x[;pred] = val` | SIMD: replace all matching fragments with val | `&n[;=='5'] = '7'` on `15561` → `17761` |
| `<- &x[;pred]` | Remove all matching fragments | `<- &n[;=='5']` on `15561` → `161` |
| `&x[;@"re"] = val` | Regex-level replace on stringified | `&s[;@"\d+"] = "N"` |
| `<- &x[;@"re"]` | Remove regex matches | `<- &s[;@"\s+"]` |

The compiler recognizes uniform filter+assign patterns as SIMD candidates:
- Target type is a contiguous sequence (String, List<Int>, digit chars of Int)
- Predicate is element-wise (scalar comparison, not multi-fragment regex)
- Assignment value is uniform for all matching elements

## Regex Literal Syntax

```
@"pattern"   → Expr::RegexLiteral(String)    // new AST variant
Regex        → Value::Regex(Regex)            // new Value variant
```

The `@` token already exists in the lexer. The parser distinguishes:
- `@` + identifier → `Expr::PriorState` (existing)
- `@` + string literal → `Expr::RegexLiteral` (new)

Zero ambiguity: `@` is always followed by *exactly one token* — ident or string.

## Type-Directed Desugar Rule

When `val[expr]` contains exactly one `BracketOp::Coord` whose expression is a
string literal or `const` string identifier:

1. If `val`'s type is a collection → keep as Coord (existing key lookup)
2. If `val`'s type is atomic (Int/Float/Bool/Char/String) → desugar to
   `val[;@"string_expr"]`

## Implementation Phases

### Phase 1: Infrastructure
1. Add `Expr::RegexLiteral(String)` to AST
2. Add `Value::Regex(Regex)` to interpreter Value enum
3. Wire DFA compiler (`analysis/dfa.rs`) into `Expr::RegexLiteral` evaluation
4. Add parser support for `@"..."` token

### Phase 2: Bracket on Atomic Types
5. Extend `evaluate_slice` / `evaluate_multi_slice` in interpreter to handle
   Int/Float/Bool/Char decomposition to `Char` fragments
6. Extend `handle_arrow_mut` and `handle_arrow_discard` for atomic type filter/assign
7. Implement result reconstruction: filtered chars → parse back to original type

### Phase 3: Regex in Bracket Filter
8. BracketOp::Mask evaluates to Value::Regex → apply regex filter on stringified value
9. DFA-based filter on String, fallback to regex crate for non-compile-time patterns

### Phase 4: Type-Directed Desugar
10. Parser: detect single string argument in bracket on non-collection type → `BracketOp::Mask(RegexLiteral(...))`
11. Update typechecker to recognize the desugar

### Phase 5: LLVM Backend
12. SIMD vectorization pass for uniform filter+assign on contiguous sequences
13. Bracket op codegen for atomic types (digit decomposition via div/mod)
14. Regex DFA inline as tight O(n) loop over char buffer

## Key Design Decisions

- **Fragments are `Char` for all atomic types**: uniform decomposition, no
  dual-type ambiguity. `15561[0]` is `Char('1')`, not `Int(1)`. Cast if needed.
- **Regex at string level**: `@"561"` matches against the stringified whole
  value, not per-element. This enables multi-character pattern matching.
- **Collection exception**: string-in-brackets on collections is always key
  lookup, never regex. Explicit `[;@"pat"]` for regex filter on collections.
- **DFA at compile time**: the existing `analysis::dfa` compiler makes
  compile-time-known regex patterns zero-allocation.
- **NO MAGIC preserved**: all dispatch is type-based (Value variant), not
  string-name matching. Bracket ops already dispatch on value type.

## Files Affected

- `src/ast.rs` — `Expr::RegexLiteral`, `Value::Regex`
- `src/lexer.rs` — no change needed (`@` already tokenized)
- `src/parser.rs` — `@"..."` parsing, type-directed desugar
- `src/interpreter.rs` — regex eval, atomic type bracket ops, reconstruction
- `src/backend/llvm/emit_expr.rs` — regex literal codegen
- `src/backend/llvm/emit_toplevel.rs` — DFA table emission
- `src/analysis/dfa.rs` — already built, wire into parser/interpreter
- `src/features/collection.rs` — bracket dispatch for new types
- `spec/SPEC.md` — update spec
