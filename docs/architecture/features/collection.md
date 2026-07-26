# Universal Bracket Syntax — The Partition Operator

**Date added:** 2026-06-11
**Status:** Implementation complete (Phases 1-5); Tuple index & IsEmpty added 2026-06-20

## Purpose

Bracket syntax (`[]`) is Brief's **Partition Operator** — it segments any value
into addressable sub-ranges. Every value decomposes to visual `Char` fragments
under bracket operations. Bracket operations select, filter, stride, or
transform these fragments. The result reconstructs to the original type.

## Syntax

```brief
// Coord access
digits[0..3]                    // First 3 chars of Int -> Int
str[0]                          // First char of String -> Char

// Stride
digits[::2]                     // Every other char -> Int
list[::3]                       // Every 3rd element -> List

// Mask (predicate)
digits[; == '5']                // Keep only digit '5' -> Int
list[; _.active]                // Keep active elements -> List

// Regex mask
digits[; @"[15]"]               // Keep digits matching regex -> Int
str[; @"^[hw]"]                 // Keep strings starting with h/w -> List

// String coord -> regex (type-directed desugar on atomic types)
val["pattern"]                  // Desugars to val[; @"pattern"]
                                // Only for non-collection atomic types

// Arrow operations
&x[; == '5'] = '7'             // Replace matching chars -> Int
<- &x[; == '5']                 // Remove matching chars -> Int
```

## Fragment Decomposition

| Type | Fragment Type | Example | Fragment sequence |
|------|---------------|---------|------------------|
| `Int` | `Char` | `15561` | `['1','5','5','6','1']` |
| `Float` | `Char` | `3.14` | `['3','.','1','4']` |
| `Bool` | `Char` | `true` | `['t','r','u','e']` |
| `Char` | `Char` | `'a'` | `['a']` |
| `String` | `Char` | `"hi"` | `['h','i']` |
| `List<T>` | `T` | `[1,2,3]` | `[1,2,3]` |
| `Tuple` | element | `(1, "a")` | `[1, "a"]` |
| `HashMap<K,V>` | `(K,V)` | `{"a":1}` | `[("a",1)]` |
| `Struct` | field `(String,Value)` | `{x:5}` | `[("x",5)]` |

## Evaluation

### ListIndexExpr (`val[idx]`)

`src/features/collection.rs` — `ListIndexExpr::evaluate()`

1. Evaluate `value` expression → `Value`
2. Evaluate `index` expression → `Int`
3. Dispatch on value type:
   - `Value::List(items)`: `items[idx]` — bounds checked
   - `Value::Tuple(items)`: `items[idx]` — bounds checked
   - `Value::DbvlTable`: string-keyed lookup
4. Returns the element at the given index.

### SliceExpr (`val[start..end;stride;mask]`)

`src/features/collection.rs` — `SliceExpr::evaluate()`

1. Evaluate `value` expression → `Value`
2. If `Value::String`: slicing on character positions (existing behavior)
3. If atomic (Int/Float/Bool/Char): decompose via `decompose_atomic_to_chars()`
   → apply start/end/stride/mask → reconstruct via `reconstruct_from_chars()`
4. If `Value::List`: element slicing (existing behavior)

### MultiSliceExpr (`val[coord;coord;stride;mask]`)

`src/features/collection.rs` — `MultiSliceExpr::evaluate()`

1. Evaluate `value` → `Value`
2. If atomic: decompose to `Vec<Value::Char>`, apply ops sequentially,
   reconstruct via `reconstruct_from_chars()`. Early return.
3. If non-atomic: coordinate indexing, stride, mask on list or tuple.
   Tuple elements are indexed identically to list elements; stride/mask
   on a Tuple produce another Tuple (preserving the tuple type).

### Type-Directed Desugar

When exactly one `BracketOp::Coord(Index(expr))` is present and `expr`
evaluates to `Value::String` on an atomic type, it desugars to a per-element
regex filter:

```brief
15561["[15]"]  →  decompose ['1','5','5','6','1']
               →  keep ['1','5','5','1'] (chars matching [15])
               →  reconstruct Int(1551)
```

Implementation: in `MultiSliceExpr::evaluate()`, before the main ops loop,
check if `ops.len() == 1` and `ops[0]` is `Coord(Index(string_expr))`.
If so, compile the string as a DFA and apply per-char regex filtering.

### Regex Literal: `@"pattern"`

`Expr::RegexLiteral(String)` evaluated via `crate::analysis::dfa::compile_to_dfa()`
→ `Value::Regex(RegexPattern)`. The DFA compiler converts the regex to a
Thompson NFA, then powerset-constructs a DFA. At runtime, `execute_dfa()`
walks the DFA table in O(n) with zero allocation.

### Regex in BracketOp::Mask

`eval_mask_condition()` (in `collection.rs`) handles three result types:
- `Value::Bool(b)` — direct boolean test
- `Value::Regex(ref dfa)` — apply DFA to stringified element
- `Value::String(ref pattern)` — compile string to DFA on the fly

## Codegen

### LLVM Backend

- `Expr::RegexLiteral` — treated as a string constant (emits global string ptr)
- `Expr::Slice` / `Expr::MultiSlice` on atomic types: passthrough for
  coord-only, stub for stride/mask
  (atomic bracket codegen returns 0 for stride/mask ops)
- `Expr::Slice` / `Expr::MultiSlice` on list/tuple types: pointer-based
  access (existing behavior, unchanged). Tuple memory layout matches List
  (`[data_ptr, len, elem0, ...]`) so existing GEP-based indexing works
  without modification.
- `Expr::ListIndex` on tuples: handled by same GEP path as List — no
  backend change needed.

## Key Files

| File | Responsibility |
|------|---------------|
| `src/features/collection.rs` | `decompose_atomic_to_chars()`, `reconstruct_from_chars()`, `eval_mask_condition()`, `SliceExpr::evaluate()`, `MultiSliceExpr::evaluate()` |
| `src/analysis/dfa.rs` | `compile_to_dfa()`, `execute_dfa()` — compile-time regex compilation |
| `src/interpreter.rs` | `Value::Regex` variant, `Expr::RegexLiteral` eval |
| `src/ast.rs` | `Expr::RegexLiteral(String)`, `Value::Regex(RegexPattern)` |
| `src/parser.rs` | `@"..."` regex literal parsing, state decl address loop skip |
| `src/typechecker.rs` | Type inference for slice/multislice/regex literal, `Type::Tuple` indexing |
| `src/backend/llvm/emit_expr.rs` | Atomic type passthrough in MultiSlice/Slice |
| `src/backend/llvm/emit_toplevel.rs` | `check_insert_strategy()` — LLVM InsertAt strategy dispatch |
| `src/features/arrow.rs` | Strategy-aware push/pop via `lookup_insert_strategy()`/`lookup_extract_strategy()` |

## InsertAt / ExtractFrom Strategy Synthesis (D-3)

When a TypeDef defines `InsertAt = "strategy"` or `ExtractFrom = "strategy"`, the compiler uses these bindings to dispatch arrow operations differently.

### Known Strategies

| Strategy | InsertAt | ExtractFrom | Behavior |
|----------|----------|-------------|---------|
| `append` | ✅ default | — | Push to end of list |
| `prepend` | ✅ | — | Insert at position 0, shift right |
| `sorted` | ✅ | — | Binary search insert (LLVM: append, interpreter: append via fallthrough) |
| `pop` | — | ✅ default | Return and remove last element |
| `shift` / `head` | — | ✅ | Return and remove first element |
| `tail` | — | ✅ | Alias for pop |
| `hash` | ✅ | ✅ | Hash-based insert/remove |

### Defining Custom Strategies

```brief
type Queue : List {
    InsertAt    = "append";
    ExtractFrom = "shift";
};

type Stack : List {
    InsertAt    = "prepend";
    ExtractFrom = "pop";
};
```
