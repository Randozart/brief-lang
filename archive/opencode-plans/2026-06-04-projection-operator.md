# `:>` Projection Operator — Plan

**Date:** 2026-06-04
**Status:** Design final, ready to implement
**Contract:** No magic strings. Unique syntax. Type-independent dispatch.

## Motivation

The existing `Expr::ListLen` variant is a magic node — it only exists because
`len(list)` or `list.len()` was special-cased. Both syntaxes relied on hardcoded
string matching against `"len"`, violating the No Magic principle.

The `:>` projection operator eliminates this by making length/size/pointer
queries a **first-class, syntax-level operation** with zero string matching.
The parser produces `Expr::Projection { source, target }` directly.

## The Token

Add `Token::ColonGreaterThan` to the lexer. This is a 2-character token `:>`
that the lexer recognizes as a single unit. It is unambiguous:
- `:` alone opens type annotations
- `>` alone is comparison
- `:>` is the projection operator; can never appear in any other context

## AST Changes

### New Enum: `ProjectionTarget` (file: `src/ast.rs`, insert after line ~309)
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectionTarget {
    Size,       // element count (length)
    Bytes,      // byte footprint
    Ptr,        // base address
    Alignment,  // memory alignment
    Range,      // (min, max) value bounds from range.rs
}
```

### Removed: `Expr::ListLen` (file: `src/ast.rs`, line 368)
Delete `ListLen(Box<Expr>)` — it is fully subsumed by Projection.

### New: `Expr::Projection` (file: `src/ast.rs`, insert replacing line 368)
```rust
/// Compile-time metadata projection: `expr :> Size`
Projection {
    source: Box<Expr>,
    target: ProjectionTarget,
},
```

## Lexer Changes (file: `src/lexer.rs`)

Add token variant ~line 263 (after ArrowLeft):
```rust
ColonGreaterThan,  // :>
```

In the lexer's main token scanning loop, after `Colon` is matched, peek
at the next character. If `>`, consume it and emit `ColonGreaterThan`.
Otherwise emit `Colon`.

## Parser Changes (file: `src/parser.rs`)

In `parse_postfix` (or similar), after parsing a primary expression:
1. Check for `Token::ColonGreaterThan`
2. If present, consume it and parse a `ProjectionTarget`
3. The target is a reserved identifier (`Size`, `Bytes`, `Ptr`, `Alignment`, `Range`)
4. Emit `Expr::Projection { source, target }`

No new parse-time type checks. The parser is type-agnostic — it just
produces the AST node.

## Typechecker Changes (file: `src/typechecker.rs`)

In `infer_expression`, add a match arm for `Expr::Projection`:
1. Infer the source expression's type
2. Match on `ProjectionTarget`:
   - `Size` → valid for List, Vector, String, any collection → `Type::Int`
   - `Bytes` → valid for any type → `Type::Int`
   - `Ptr` → valid for any allocated/stateful variable → `Type::Int`
   - `Alignment` → valid for any type → `Type::Int`
   - `Range` → valid for Int, Float → `Type::Tuple(Type::Int, Type::Int)`
3. If target is invalid for source type, emit type error

**Remove** the UFCS resolution pass added previously (the `resolve_len_calls`
block with `HashSet<String>` name matching). It is obsolete.

## Interpreter Changes (file: `src/interpreter.rs`)

Add evaluation for `Expr::Projection` (~line ~1850):
- `Size` on a `Value::List` → `Value::Integer(list.len())`
- `Bytes` on any value → `Value::Integer(size_in_memory(value))`
- `Ptr` on a list → `Value::Integer(list_ptr_address)` (or 0 for simulation)
- `Alignment` → `Value::Integer(8)` (default alignment)
- `Range` → `Value::List([min, max])` from range analysis if available, else `(-∞, +∞)`

## LLVM Backend (file: `src/backend/llvm.rs`)

### Codegen for `Expr::Projection` (~line ~3100, replacing `ListLen` handler)

| Target | Codegen |
|--------|---------|
| `Size` on `List<T>` | `load i64* %slot_1` (length field of 2-slot header) |
| `Size` on static `Vector<T,N>` | `add i64 0, N` (compile-time constant) |
| `Size` on scalar | `add i64 0, 1` |
| `Bytes` on any type | `add i64 0, sizeof(T)` (compile-time) |
| `Ptr` on `List<T>` | `load i64* %slot_0` (data pointer) |
| `Ptr` on stack var | `ptrtoint %alloca to i64` |
| `Ptr` on MMIO field | `inttoptr i64 ${address}` |
| `Alignment` | `add i64 0, N` (compile-time) |
| `Range` | Return `(min, max)` as two i64 constants |

### Remove `ListLen` codegen (~line 2657)
The `ListLen` stub that returns 0 should be deleted and replaced by the above.

## Transition Graph (file: `src/analysis/transition_graph.rs`)

### `extract_bounded_pre` (~line 99-190)

Add match arms for `Expr::Projection { target: Size }`:

```rust
ExtractBoundedPre arm for Gt/Ge:
  (Expr::Projection { source, target: Size }, Expr::Integer(n)) => {
    if let Some(name) = expr_name(source) {
      Some(BoundedPre { var: name, dir: Decreasing, bound_literal: Some(*n) })
    }
  }
```

Remove the `Expr::ListLen` arms added in the previous session (now obsolete).

### `detect_collection_drain` (~line 528)

No changes needed — it already matches `ArrowMut`/`ArrowDiscard` structures
directly, and the increment detection is decoupled from the Size projection.

## Build Order

### Phase 1: Preparatory cleanup (no functional changes)
1. Add `Token::ColonGreaterThan` to lexer
2. Add `ProjectionTarget` enum to `ast.rs`
3. Add `Expr::Projection` variant to `ast.rs`
4. Update all match arms across the codebase to handle the new variant
   - All 13 match sites: parser, interpreter, typechecker, transition_graph,
     llvm.rs, rust.rs, webstack.rs, annotator.rs, dataflow.rs, proof_engine.sys,
     symbolic.rs, desugarer.rs, fuzzer/ast_generator.rs
5. Delete `Expr::ListLen` variant
6. Update all match arms that matched `ListLen` — replace with `Projection { .. }` or delete as appropriate

### Phase 2: The lexer/parser
7. In lexer: after `Colon` token, peek for `>` → emit `ColonGreaterThan`
8. In parser: `parse_postfix` checks `ColonGreaterThan` → parse target → emit `Expr::Projection`

### Phase 3: Semantics and codegen
9. Typechecker: `infer_expression` for `Projection`
10. Interpreter: eval for `Projection`
11. LLVM backend: codegen for all 5 projection targets
12. Remove the UFCS hack from typechecker (`resolve_len_calls` block + HashSet import)

### Phase 4: Analysis
13. `extract_bounded_pre`: add `Projection(Size)` arm, remove `ListLen` arm
14. `detect_collection_drain`: verify unchanged

## Files and Line Numbers (initial state)

| File | Lines | Change |
|------|-------|--------|
| `src/lexer.rs:263` | +1 | Add `ColonGreaterThan` to Token enum |
| `src/lexer.rs:280-400` | +5 | Peek-for-`>` logic in lexer loop |
| `src/ast.rs:308` | +11 | Add `ProjectionTarget` enum |
| `src/ast.rs:368` | -1 | Delete `ListLen(Box<Expr>)` |
| `src/ast.rs:368` | +6 | Add `Projection { source, target }` |
| `src/parser.rs:~1200` | +25 | Parse `:>` in postfix |
| `src/typechecker.rs:~1196` | +30 | `infer_expression` for `Projection` |
| `src/typechecker.rs:634-710` | -80 | Delete UFCS resolution block |
| `src/typechecker.rs:28` | -1 | Remove `HashSet` import (no longer needed) |
| `src/interpreter.rs:~1850` | +50 | Eval for `Projection` |
| `src/backend/llvm.rs:~2657` | -5 | Delete `ListLen` stub |
| `src/backend/llvm.rs:~3100` | +40 | Codegen for all 5 projection targets |
| `src/analysis/transition_graph.rs:99-190` | +20 / -20 | Replace `ListLen` with `Projection(Size)` |
| ±10 other files | ~60 total | Add catch-all `_ => {}` or `Projection` match arms |

**Total:** ~150 lines added, ~106 removed (net ~+44).

## Pointer Safety Model

The `Range` projection + SMT range analyzer enables deterministic safe pointer
arithmetic. When a developer writes:

```brief
let ptr = buffer :> Ptr;
let element = ptr + (idx * 8);
```

The compiler's `range.rs` analysis proves `proven_max(idx) * 8 < buffer :> Bytes`.
If proven: raw `getelementptr` with no bounds check. If unprovable: compile-time
safety violation error.

This is a strict improvement over:
- **C**: No bounds metadata, all pointer arithmetic unchecked
- **Rust**: Requires `unsafe { }`, no SMT prover, developer responsible
- **SPARK Ada**: Bans pointer arithmetic entirely

Brief proves pointer safety statically using data the compiler already has
(ranges from contracts + allocation sizes from types), without runtime checks
and without unsafe blocks.

## Existing Changes to Preserve (from this session)

The following changes are architecturally sound and should be kept:

1. `extract_valid_bounded_pre` — prevents picking immutable bound vars from
   `And` clauses. Critical fix.
2. `simplify_body` + `simplify_expr` — algebraic cancellation. Enables P4.
3. `detect_popcount_decay` — bit-clear pattern detection. Pure additive.
4. `detect_collection_drain` — collection pop detection. Pure additive.
5. Extended `detect_increments` for interval bounds (`Sub(Add(var, R1), R2)`).
6. `detect_lexicographic_ranking` — multi-variable Or-condition ranking.

All of these feed into the existing `BoundedPre` + `IncrementInfo` mechanism
and are independent of the `ListLen` → `Projection` migration.

## What To Remove

1. `Expr::ListLen` — delete entirely, replace with `Projection`
2. UFCS resolution block in typechecker — the `resolve_len_calls` hack with
   `HashSet<String>` name matching. Delete. It was always wrong.
3. `Expr::ListLen` arms in `extract_bounded_pre` — replace with `Projection(Size)`
4. All `Expr::ListLen` match arms in backends — replace with `Projection(Size)`
   catch-all or delete
