# `<:` Subtype Projection Operator — Phase E+F Plan

**Date:** 2026-06-08 23:00
**Status:** Approved for implementation
**Supersedes:** Phase E (query engine on native values) + Phase F (DBVL append/performance)

---

## 1. Core Syntax

### Collection Projection
Source type: `List<T>`, `HashMap<K,V>`, or any iterable collection.

```
let result <: source {
    FILTER(.predicate);
    GROUP(.key);
    COUNT;
};
```

### String Projection
Source type: `String`. Pattern is a compile-time constant string or literal.

```
let (user, domain) <: email["^([a-z]+)@(.+)$"];
```

Alternate form:
```
const pat = "^([a-z]+)@(.+)$";
let (user, domain) <: email[pat];
```

---

## 2. Allowed Operations

### Collection Source

| Op | Signature | Semantics | Output type |
|----|-----------|-----------|-------------|
| `FILTER(.expr)` | `T -> Bool` | Keep elements where predicate is true | `List<T>` |
| `MAP(.expr)` | `T -> U` | Transform each element | `List<U>` |
| `SORT(.expr)` | `T -> Ord` | Sort by key expression | `List<T>` |
| `LIMIT(N)` | `Int` | Take first N elements | `List<T>` |
| `SKIP(N)` | `Int` | Skip first N elements | `List<T>` |
| `UNIQUE` | — | Remove adjacent duplicates | `List<T>` |
| `JOIN(other, .key)` | `T -> K, U -> K` | Merge with another collection | `List<(T,U)>` |
| `GROUP(.key)` | `T -> K` | Group by key (must be followed by aggregate) | `Map<K, List<T>>` |
| `COUNT` | — | Count elements (aggregate, terminal) | `Int` |
| `SUM(.field)` | `T -> Int/Float` | Sum of field (aggregate, terminal) | `Int/Float` |
| `AVG(.field)` | `T -> Int/Float` | Average of field (aggregate, terminal) | `Float` |
| `MIN(.field)` | `T -> Ord` | Minimum of field (aggregate, terminal) | `typeof(.field)` |
| `MAX(.field)` | `T -> Ord` | Maximum of field (aggregate, terminal) | `typeof(.field)` |

### String Source

| Op | Syntax | Semantics | Output type |
|----|--------|-----------|-------------|
| `MATCH` | `string["pattern"]` | Compile regex to DFA, extract capture groups | `Bool` (0 groups), `String` (1), `Tuple` (N) |

---

## 3. Return Type Rules

The last operation in the block determines the return type:

**Aggregates are terminal** — collapse to scalar:
```
let count <: items { FILTER(.active); COUNT; };         // Int
let total <: items { SUM(.price); };                    // Int/Float
let avg   <: items { AVG(.score); };                    // Float
let early <: items { MIN(.timestamp); };                // typeof(.timestamp)
```

**Non-aggregates return a collection:**
```
let active <: items { FILTER(.active); };               // List<Item>
let names  <: items { MAP(.name); SORT(.name); };      // List<String>
```

**GROUP changes the result shape** — aggregate applies per group:
```
let per_region <: items { GROUP(.region); COUNT; };     // Map<String, Int>
```

Compiler enforces: aggregates must be the **last op** in the block. No op after an aggregate.

---

## 4. Parser Grammar

### Collection Projection
```bnf
projection_decl ::= "let" ident "<:" expression "{" projection_op ";" { projection_op ";" } "}" ";"
projection_op    ::= "FILTER" "(" expression ")"
                   | "MAP" "(" expression ")"
                   | "SORT" "(" expression ")"
                   | "LIMIT" "(" integer ")"
                   | "SKIP" "(" integer ")"
                   | "UNIQUE"
                   | "JOIN" "(" expression "," expression ")"
                   | "GROUP" "(" expression ")"
                   | "COUNT"
                   | "SUM" "(" expression ")"
                   | "AVG" "(" expression ")"
                   | "MIN" "(" expression ")"
                   | "MAX" "(" expression ")"
```

### String Projection
```bnf
string_projection ::= "let" tuple_ident "<:" expression "[" expression "]" ";"
tuple_ident       ::= ident | "(" ident { "," ident } ")"
```

---

## 5. AST Changes

### New types in `src/ast.rs`

```rust
pub enum Expr {
    // ... existing variants ...
    SubtypeProjection {
        source: Box<Expr>,
        ops: Vec<SubtypeOp>,
    },
}

pub enum SubtypeOp {
    Filter(Expr),
    Map(Expr),
    Sort(Expr, SortDir),
    Limit(usize),
    Skip(usize),
    Unique,
    Join { other: String, key: Expr },
    Group(Expr),
    Count,
    Sum(Expr),
    Avg(Expr),
    Min(Expr),
    Max(Expr),
    Match(Expr),  // regex pattern for string projection
}

pub enum SortDir {
    Asc,
    Desc,
}
```

### Removal

Remove `ProjectionTarget::Match` from the `ProjectionTarget` enum — superseded by `SubtypeOp::Match` in the string projection form.

---

## 6. Interpreter Design

### Collection Projection — Single-Pass Fusion

All non-aggregate ops run in a single fused pass over the source:

```
for item in source {
    if FILTER predicate fails, skip
    if MAP, transform
    if SORT, buffer for post-sort
    if GROUP, route to accumulator bucket
    if LIMIT, break after N items
    if SKIP, skip first N items
}
if SORT, sort buffered results
if aggregate (COUNT/SUM/etc), compute from group buckets
```

No intermediate heap allocations between ops.

### String Projection — DFA Execution

```
let dfa = compile_regex(pattern);  // at compile time (AST)
let result = dfa_execute(dfa, string);  // at runtime, O(n)
```

---

## 7. LLVM Backend

- **Collection ops**: Deferred — initially emit via interpreter fallback for correctness, then optimize later.
- **String `[pattern]`**: Deferred — same approach.
- Remove existing `ProjectionTarget::Match` stubs from all backends.

---

## 8. Phase F Hook: DBVL Large-File Indexing

No new syntax. A large `.dbvl` imported via `import "data.dbvl" as data` creates a `Value::LazyMap` that:
- Stores key → byte offset index in memory (~4 bytes/entry) on first import
- Parses individual lines lazily when accessed by key
- `<:` FILTER on indexed key field uses seek-based range scan instead of full parse

The `<:` compiler detects indexed collections and generates the optimized code path.

---

## 9. Implementation Steps (ordered)

| Step | File(s) | Description |
|------|---------|-------------|
| 1 | `src/ast.rs` | Add `SubtypeProjection` + `SubtypeOp` enum, remove `ProjectionTarget::Match` |
| 2 | `src/parser.rs` | Parse collection projection `let x <: src { ops; }` |
| 3 | `src/parser.rs` | Parse string projection `let x <: src[pattern]` |
| 4 | `src/typechecker.rs` | Validate source type against ops |
| 5 | `src/desugarer.rs` | Add pass-through match arm |
| 6 | `src/interpreter.rs` | Implement fused projection evaluation |
| 7 | `src/backend/*` (all 10) | Remove `:> Match` stubs |
| 8 | `src/dbrief/eval.rs` | **Delete** — superseded |
| 9 | `src/dbrief/mod.rs` | Remove `pub use eval::*` |
| 10 | `tests/` | Parser + interpreter tests |

---

## 10. Obsolete Code Removal

| File | Lines | Reason |
|------|-------|--------|
| `src/dbrief/eval.rs` | 612 | Query engine operated on old `DbriefLiteral` — superseded by `<:` on native Value |
| `ProjectionTarget::Match` in `ast.rs` | ~5 | Superseded by `SubtypeOp::Match` for string `<:[...]` |
| `interpreter.rs` `:> Match` match arm | ~15 | Removed |
| All backends `:> Match` match arms | ~10 each (10 backends = ~100) | Removed (were stubs) |

---

## 11. Test Plan

| Test | Description |
|------|-------------|
| `test_projection_filter` | FILTER keeps matching elements |
| `test_projection_map` | MAP transforms elements |
| `test_projection_filter_limit` | FILTER + LIMIT fuses to early break |
| `test_projection_sort` | SORT orders elements |
| `test_projection_group_count` | GROUP + COUNT groups and counts |
| `test_projection_group_sum` | GROUP + SUM sums per group |
| `test_projection_unique` | UNIQUE removes adjacent duplicates |
| `test_projection_join` | JOIN merges two collections |
| `test_projection_string_match` | MATCH extracts capture groups |
| `test_projection_string_match_bool` | MATCH without groups returns Bool |
| `test_projection_type_error` | FILTER on String source errors |
| `test_projection_match_error` | MATCH on List source errors |
| `test_projection_aggregate_must_be_last` | Op after COUNT errors |
| `test_no_more_projectin_target_match` | `:> Match` no longer exists |
| `test_eval_deleted` | `dbrief::eval` module removed |
