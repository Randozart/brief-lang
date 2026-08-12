# Collection Mutation + Dimension Targeting — Language Design & Implementation Plan

**Date:** 2026-06-04
**Status:** Design complete — implementation pending

## Context

This plan emerged from a discussion about Phase 5 (runtime-sized allocation) of the [LLVM Backend Completion Plan](./2026-06-03-llvm-backend-completion.md). During that discussion, we identified a deeper problem: Briev's interpreter uses string-matched function names (`"list_append"`, `"get"`, `"insert"`, etc.) to handle collection operations. This is magic. The LLVM backend cannot inherit this pattern without violating the **No Magic** principle.

What started as a discussion about `push`/`pop`/`insert`/`remove` syntax expanded into a complete rethinking of collection mutation, multi-dimensional targeting, and LINQ-style declarative data manipulation — all without a single method name or magic string match.

## Problem Statement

### What's Broken

1. **Interpreter uses string matching** (`src/interpreter.rs:1037-1163`): `if fn_name == "list_append" { ... }`, `if fn_name == "get" { ... }`, etc. Every collection operation is an opaque `Expr::Call` with a magic string.

2. **No structural mutation syntax**: Fixed-size Vectors (`Int[16]`) have `=` for element replacement and `[;>5]` for masked assignment. Dynamic Lists (`List<T>`) have no equivalent. You can read with `[i]`, slice with `[1..5]`, but you can't grow or shrink.

3. **Template standard library**: `lib/std/collections.bv` (12 lines) defines `append` and `len` as stubs with `term list;` and `term 0;` — they don't actually work. `lib/std/stack.bv` does its own `push`/`pop` using `+ [item]` concat and `[0..len()-1]` slice — both copy-based, both O(n).

4. **Method call syntax without method dispatch**: The language reference spec lists `result.validate()` and `list.length()` as valid syntax, but neither the interpreter nor the LLVM backend implements any method resolution.

5. **No way to express "push into the 3rd row of this 2D list"** or "filter all adults from this dataset into a new list" as a single fused operation.

### What Needs Solving

1. Collection mutation needs first-class syntax — no magic strings
2. The arrow operator (`<-`) must be bidirectional: push AND pop
3. Multi-dimensional tensors need concise dimension targeting
4. Filter-mask-stride syntax already exists (`[;>5]`, `[::2]`, `[0..10]`) — this just needs to compose with mutation
5. All of this must feel Briev-idiomatic: contract-driven, compiler-optimizable, zero magic

## Design: Three New Primitives

### 1. `<-` Arrow Mutation Operator

**Token:** `#[token("<-")] ArrowLeft` — new lexer token. Zero conflict with `<<` (ShiftLeft), `<=` (Le), or `->` (Arrow). Logos longest-match: `<-` (2 chars) beats `<` (1 char).

**Semantics:** One operator, direction determines operation:

| Source | Emits | Semantics |
|--------|-------|-----------|
| `&list <- x` | `Expr::ArrowMut(Push, list, full_range, x)` | Append to end |
| `x <- &list` | `Expr::ArrowMut(Pop, list, full_range)` → binds to x | Pop last |
| `<- &list` | `Expr::ArrowDiscard(list)` (statement level) | Pop last, discard |
| `&list[i] <- x` | `Expr::ArrowMut(Insert, list, i, x)` | Insert at i, shift right |
| `x <- &list[i]` | `Expr::ArrowMut(Remove, list, i)` → binds to x | Remove at i, capture |
| `<- &list[i]` | `Expr::ArrowDiscard(list, i)` (statement level) | Remove at i, discard |
| `&list[i..j] <- src` | `Expr::ArrowMut(InsertRange, list, slice, src)` | Insert src at range, shift right |
| `x <- &list[i..j]` | `Expr::ArrowMut(RemoveRange, list, slice)` → binds to x | Remove range, capture |

`=` for element replacement, `<-` for structural mutation. `&` already means mutable reference. They compose naturally.

**Discard form** (`<- &list`): The target is a statement-level expression with no binding. The ArrowDiscard variant handles this. Briev already has expression statements in the parser (`parse_statement` checks for `&` prefix to distinguish from assignment). ArrowDiscard extends this pattern.

**Type checking**: `<-` is valid only when the resolved collection type is `List<T>`. Fixed-size `Vector<T, ...>` rejects `<-` at compile time — structural mutation on a fixed-size type is a type error. Same mechanism that rejects assigning a `String` to an `Int`.

**Codegen:** Reuses the 2-slot list header from [Phase 3](./2026-06-03-llvm-backend-completion.md#phase-3-collection-operations):

- **Push**: `load len from slot 1 → GEP slot(len+2) → store elem → store len+1 at slot 1 → ptrtoint`
- **Pop**: `load len from slot 1 → GEP slot(len+1) → load for return → store len-1 at slot 1 → ptrtoint`
- **Insert/Remove**: element shifting within the same buffer. No copy, no reallocation — the compiler proves the old list is dead and reuses the buffer.

### 2. `...` Ellipsis — Dimension Fill

**Token:** `#[token("...")] Ellipsis` — new lexer token. Longest-match: `...` (3 dots) beats `..` (2 dots). A range is always `N..M`, never `N...M` — no parser ambiguity.

**Semantics:** Inside bracket context, `...` expands to the right number of `:` wildcards (`SliceCoordinate::All`) to fill unspecified dimensions. The compiler computes the expansion at parse time from the type's dimensionality.

```briev
# 4D tensor: Float[batch:64, channel:3, row:32, col:32]
tensor[..., 0]              → tensor[:, :, :, 0]     — last dim = 0
tensor[0, ...]              → tensor[0, :, :, :]     — first dim = 0
tensor[0, ..., 0..16]       → tensor[0, :, :, 0..16] — first=0, middle pass-through, last=0..16
tensor[..., 0..10, ...]     — ambiguous: rejected unless dimension context disambiguates
```

The parser needs the type available during parsing to compute the expansion. If the type is unknown (e.g., inside a template), `...` deferred to typechecking/codegen.

**When `...` is ambiguous**: `tensor[..., 0..10, ...]` has two ellipses and one explicit range — should the range target dimension 2 or 3? Resolution: **at most one `...` per bracket expression unless named dimensions disambiguate**. This is a parser rule, not a type system rule.

### 3. `@` Dimension Specifier

**Syntax:**

```
@ spec  → "@" integer (".." integer "?") ":" operation
        | "@" identifier ":" operation     (named dimension reference — existing)

operation → integer                        index
          | integer ".." integer "?"       range
          | ".." integer                   range from 0
          | "::" integer                   stride
          | ";" condition                  filter
```

**No conflict with prior state `@counter`:** `@` followed by a **digit** enters dimension-spec mode. `@` followed by an **identifier** is existing `Expr::PriorState`. The parser handles this trivially — peek at the next token.

**Examples:**

```briev
# Declaration
let x: Vector<Float, @1:64, @2:3, @3:32, @4:32>;
let x: Vector<Float, @1..3:32, @4:64>;      # dims 1-3 size 32, dim 4 size 64
let x: Vector<Float, @32:4>;                # 32 dimensions, each size 4
let emb: Vector<Float, batch:64, @1..16:768, head:12>;  # mixed

# Slicing — zero-indexed
tensor[@12: 0..16]                          # dimension 12, range 0..16
tensor[@5: ;>2]                             # dimension 5, filter >2
tensor[@1..4: 0..10, @6: 5]                # dims 1-4 range 0..10, dim 6 index 5
tensor[batch:0, @2..16: 0..32]             # mixed named + @
tensor[@5: ;>2]                            # filter dimension 5, rest pass through

# With lists
rows: List<List<Int>>
rows[@0: 3]                                 # rows[3] — first dimension, index 3
rows[@1: ;>5]                               # inner dimension, filter >5
```

**Default pass-through:** Any dimension NOT specified by `@` or explicit index gets the full range (`:` wildcard). This makes `tensor[@5: ;>2]` equivalent to "filter dimension 5, leave all others unchanged" — exactly right for massive tensors.

**AST:** `SliceCoordinate::AtDimension { dimensions: Vec<usize>, op: Box<SliceCoordinate> }`. Dimensions are zero-indexed integers. `op` is one of the existing `SliceCoordinate` variants.

**Declaration expansion:** `Vector<Float, @1..3:32, @4:64>` desugars to `Vector<Float, 32, 32, 32, 64>`. `Vector<Float, @32:4>` desugars to `Vector<Float, 4, 4, 4, ...>` (32 entries). Done at parse time, no runtime cost.

**List support:** `@` works on `List<List<Int>>` by treating the nesting depth as dimensionality. `@0` targets the outer list, `@1` targets the inner list of every element. This is semantically identical to `rows[3]` and `rows[3][;>5]` — `@` just provides positional precision when commas would be ambiguous or verbose at extreme depths.

### Composition: Unified Example

```briev
# Struct type for demonstration
struct Person {
    name: String,
    age: Int,
    city: String,
    active: Bool,
};

# Dataset — list of persons
let dataset: List<Person>;

# 1. Push filtered adults into active list — single fused loop
&active <- dataset[; age > 18 && active == true];

# 2. Pop last adult
latest <- &active;

# 3. Insert filtered NY residents, every 2nd, starting at position 10
&results[10] <- dataset[; city == "NYC"][::2];

# 4. Multi-dimensional targeting on tensors
let tensor: Vector<Float, batch:64, channel:3, row:32, col:32>;

# Filter channel 0 by value, leave everything else
tensor[@2:0; >50]                    # channel=0, filter values >50

# Remove range from dimension 12
removed <- &tensor[@12: 1..5];

# 5. 2D list-of-lists
let matrix: List<List<Int>>;

# Push into row 3
&matrix[3] <- 42;

# Pop from row 3
val <- &matrix[3];

# Push a new empty row
&matrix <- [];

# Push filtered results from column 0 of every row into a new list
&result <- matrix[@1: 0; >5];
```

## Rejected Syntax

These were considered and deliberately rejected:

### `push(list, x)` / `pop(list)` — Method Call Desugaring
**Rejected because:** this keeps the magic string-matching alive, just relocated from interpreter to parser. The function names are still magic. The parser matching `if fn_name == "push"` is the same violation as the interpreter `if fn_name == "list_append"`. Collection operations deserve first-class syntax, not disguised function calls.

### `&list <+ x` — Augmented Assignment
**Rejected because:** `<+` works in one direction (push) but not the other (pop). `x <+ &list` doesn't read as extraction. Briev needs bidirectional mutation — `<-` handles both naturally through direction.

### `list << x` — Stream Operator
**Rejected because:** `<<` is already ShiftLeft. Overloading would break all bitwise shift operations. C++ can do this because it separates stream `<<` (library overload) from bitwise `<<` (built-in). Briev has a fixed token set — no operator overloading.

### `list[1..5] = src` as InsertRange
**Rejected because:** `=` means replacement, not structural change. `&list[1..5] = src` replaces elements 1-5 with src (if lengths match). `&list[1..5] <- src` inserts src at position 1, shifting elements right. `=` and `<-` have different meanings, and the operator should communicate them.

### @ with Parentheses: `(@2..4)1..3` instead of `@2..4: 1..3`
**Rejected because:** `(` inside `[...]` is currently an expression group. Changing it to sometimes be a dimension group would require the parser to look ahead and distinguish contexts. `@` signals dimension context unambiguously without lookahead.

### `@N` as 1-indexed
**Rejected because:** Briev uses zero-indexing everywhere: `list[0]` is the first element, `mat[0, 0]` is the first cell, `SliceCoordinate::Index(0)` is position 0. Having `@1` mean "first dimension" while `[0]` means "first element" creates silent off-by-one errors. Zero-indexed everywhere, end of discussion. Named dimensions (`@batch: 0`) solve the readability concern without breaking indexing consistency.

### `forall` / `exists` Quantifier Syntax
**Dropped.** These were in the original LLVM Backend Completion Plan as Phase 6. Neither the interpreter (`Expr::ForAll` stub at line 1838) nor the LLVM backend (`Expr::ForAll` stub at line 2746) have real implementations. No benchmark uses them. They are theorem-proving features, not performance or expressiveness features. Quietly removed from the plan.

## AST Changes

### New Variants

```rust
// src/ast.rs

pub enum ArrowDir {
    Push,       // &list <- x or &list[index] <- x
    Pop,        // x <- &list or x <- &list[index]
}

pub enum Expr {
    // ... existing variants ...

    // Collection structural mutation
    ArrowMut {
        dir: ArrowDir,
        target: Box<Expr>,       // the &list identifier
        index: Box<Expr>,        // Slice expression for bracket contents (or implicit full-range)
        value: Option<Box<Expr>>, // RHS value (None for Discard form)
    },

    // Discard pop/remove: <- &list or <- &list[i]
    ArrowDiscard {
        target: Box<Expr>,
        index: Box<Expr>,
    },

    // Ellipsis placeholder — desugared in parser, should never appear in generated AST
    // (If type is unknown during parsing, it stays and is expanded in typecheck/codegen)
    Ellipsis,
}

// Extend existing SliceCoordinate
pub enum SliceCoordinate {
    Index(Box<Expr>),
    Range { start: Option<Box<Expr>>, end: Option<Box<Expr>> },
    Named { name: String, coord: Box<SliceCoordinate> },

    // NEW: Positional dimension targeting
    // "dimensions" is zero-indexed dimension positions
    // "op" is the operation to apply to those dimensions (Index, Range, All, etc.)
    // "stride" optionally applies a stride to the enumerated selections
    AtDimension {
        dimensions: Vec<usize>,      // which dimensions (may be range)
        op: Box<SliceCoordinate>,
    },
}
```

### SliceCoordinate::AtDimension Semantics

| @ Spec | AST |
|--------|-----|
| `@5: 0` | `AtDimension { dimensions: [5], op: Index(0) }` |
| `@2..4: 1..3` | `AtDimension { dimensions: [2,3,4], op: Range(1, 3) }` |
| `@5: ;>2` | `AtDimension { dimensions: [5], op: Filter(>`2``) }` |
| `@1..3: ::2` | `AtDimension { dimensions: [1,2,3], op: All, stride: 2 }` |

Multi-dimensional `@` targets: `@1..4:1..3` means "for dimensions 1 through 4, take elements 1 through 3 in each." The compiler emits nested loops.

## Lexer Changes

```rust
// src/lexer.rs — add within Token enum

#[token("<-")]
ArrowLeft,

#[token("...")]
Ellipsis,
```

`ArrowLeft` is a new operator. `Ellipsis` is a new bracket-content token. No other lexer changes.

## Parser Changes

### Arrow Parsing

In `parse_statement`, after parsing an expression that could be an LHS:

1. If the next token is `<-`, this is an arrow statement
2. If the LHS is an `&name` or `&name[...]`, emit `ArrowMut` or `ArrowDiscard`
3. The LHS is the target, the bracket contents become the `index` field
4. If the arrow operator appears after the LHS and before the value, the RHS is `value`
5. If the arrow operator appears at the start (`<- &list`), this is `ArrowDiscard`

In `parse_assignment`, when the operator is `<-` instead of `=`:

1. Same logic as above but in expression position (for `x <- &list` in non-statement context)
2. Handles the binding side: `x = &list <- val` → first evaluate `&list <- val`, then bind to x

Precedence: `<-` is tighter than `=` but looser than `..` (range). Expressions like `&list <- a + b` parse as `&list <- (a + b)` — the RHS is the entire additive expression.

### `...` Expansion

In `parse_bracket_contents`:

1. When `...` is encountered, count dimensions remaining from the type
2. If the type is a `Vector<T, d1, d2, ...>` with N dimensions:
   - Count how many explicit coordinates precede `...`
   - Count how many explicit coordinates follow `...`
   - Fill the gap with `SliceCoordinate::All` for each missing dimension
3. If the type is unknown (template context), defer to typechecking
4. If `...` appears and the type is 1D (plain List), emit an error

Rule: at most one `...` per bracket expression. Multiple `...` is a parse error unless named dimensions disambiguate the target.

### `@` Dimension Parsing

In `parse_slice_coordinate_inner` (or `parse_multi_slice`):

1. If the next token is `@`, enter dimension-spec mode
2. Consume `@`
3. Parse one or more dimension indices (integers, comma-separated or `..` range)
4. Expect `:`
5. Parse the operation (index, range, stride, filter)
6. Emit `SliceCoordinate::AtDimension { dimensions, op }`

This is a parser extension, not a new parser function. The existing `parse_slice_coordinate` already handles `Named`, `Index`, `Range` — `AtDimension` is a new variant in the same function.

### `@` in Type Declarations

In `parse_type_inner`:

1. After parsing `Vector<T,` or `Vector<`, if the next token is a dimension entry
2. Recognize `@N:M` as a shorthand for M repeated N times
3. Recognize `@N..M:S` as S repeated (M-N+1) times
4. Expand to explicit dimension entries before emitting the `Type::Vector`

No AST change for types — expansion happens entirely in the parser.

## Interpreter Changes

### New Expr Handlers

Four new arms in `eval_expr` (interpreter.rs):

```rust
Expr::ArrowMut { dir, target, index, value } => {
    // Evaluate target → get list Value
    // Evaluate index → resolve to position (int) or range (start, end)
    // Evaluate value if present
    // Match on dir:
    //   Push: clone list, push value, return new list
    //   Pop: clone list, pop, return (new list, popped value)
    //   Insert: clone list, insert at index, return new list
    //   Remove: clone list, remove at index, return (new list, removed value)
    // For range operations: slice-based insert/remove
}

Expr::ArrowDiscard { target, index } => {
    // Same as ArrowMut::Pop/Remove but discard the extracted value
    // Returns the new list
}

Expr::Ellipsis => {
    // Should never reach here — desugared in parser
    panic!("Ellipsis not desugared")
}
```

### Remove String-Matched Collection Methods

Delete lines from `Expr::Call` handler in interpreter.rs:
- `if fn_name == "list_append"` (line 1037)
- `if fn_name == "get"` (line 1052)

### SliceCoordinate::AtDimension

In `eval_slice_coordinate` or equivalent:
```rust
SliceCoordinate::AtDimension { dimensions, op } => {
    let mut result = vec![];
    for dim in dimensions {
        // For each target dimension, apply `op` to get the element(s) at that position
        // This produces a sub-view for that dimension
        let sub = evaluate_coordinate(collection, op);
        result.push(sub);
    }
    Value::List(result)  // or appropriate collection view type
}
```

## LLVM Backend Changes

### ArrowMut Codegen

In `emit_expr`, new arm for `Expr::ArrowMut`:

```rust
Expr::ArrowMut { dir, index, value } => {
    let list_ptr = ...  // evaluate target
    match dir {
        ArrowDir::Push => {
            // inttoptr list_ptr → load len from slot 1
            // GEP slot(len+2) → store value
            // store len+1 at slot 1
            // ptrtoint → return new list ptr (same pointer)
        }
        ArrowDir::Pop => {
            // inttoptr → load len from slot 1
            // GEP slot(len+1) → load element (for return value)
            // store len-1 at slot 1
            // For binding: store element into let_bindings
            // ptrtoint → return
        }
        ArrowDir::Insert => {
            // Compute insert position from index
            // Shift elements (len - pos) right by 1
            // Store value at pos
            // len += 1
        }
        ArrowDir::Remove => {
            // Load element at pos (for return)
            // Shift elements (len - pos - 1) left by 1
            // len -= 1
        }
    }
}
```

### ArrowDiscard Codegen

Same as Pop/Remove, but the extracted value is just dropped (no binding).

### ... / @ Dimension Specifiers

These are handled in the parser. The LLVM backend never sees them directly — they're desugared into explicit `SliceCoordinate` values. Multi-dimensional vectors are not yet in the LLVM backend (complex codegen, out of scope for this plan). `@` targeting on `List<List<Int>>` decomposes into nested GEP + loop patterns — Part B of this work, deferred.

## Stdlib Cleanup

After implementation, these stdlib files can use native syntax:

### `lib/std/collections.bv` — Rewrite
```briev
# Before (stub):
defn append(list: List<Int>, item: Int) -> List<Int> {
    term list;
};

# After (native):
defn append(list: List<Int>, item: Int) [true][term.len() == list.len() + 1] -> List<Int> {
    &list <- item;
    term list;
};

defn len(list: List<Int>) [true][term >= 0] -> Int {
    term list.len();
};
```

### `lib/std/stack.bv` — Rewrite
```briev
# Before: O(n) copy-based push/pop
defn push<T>(stack: Stack<T>, item: T) -> Stack<T> {
    term Stack { items: stack.items + [item] };
};

# After: O(1) arrow mutation
defn push<T>(stack: Stack<T>, item: T) -> Stack<T> {
    &stack.items <- item;
    term stack;
};
```

### Self-Hosting Compiler
Search `lib/compiler/` for any method-call patterns and replace with arrow syntax.

## Implementation Order

### Part A: Arrow Mutation (this plan — Phase 4b)

1. **Lexer**: Add `ArrowLeft` token
2. **AST**: Add `ArrowDir`, `Expr::ArrowMut`, `Expr::ArrowDiscard`
3. **Parser**: Parse `<-` in assignment and statement contexts
4. **Interpreter**: Handle `ArrowMut`/`ArrowDiscard`, remove string-matched collection methods
5. **LLVM backend**: Codegen Push/Pop/Insert/Remove using 2-slot header
6. **Stdlib**: Update `collections.bv`, `stack.bv`, `queue.bv`
7. **Tests**: Arrow mutation tests, stdlib correctness tests

### Part B: `...` + `@` Dimension Specifiers (Phase 4c)

1. **Lexer**: Add `Ellipsis` token
2. **AST**: Add `Expr::Ellipsis`, `SliceCoordinate::AtDimension`
3. **Parser**: Ellipsis expansion in bracket context, `@` dimension parsing, `@` in type declarations
4. **Type system**: `@` dimension validation (bounds, count)
5. **Interpreter**: Handle `AtDimension` in multi-dimensional slicing
6. **LLVM backend**: Multi-dimensional codegen for `@` targeting (deferred if complex)
7. **Tests**: Ellipsis expansion, `@` slicing, type declaration expansion

### Part C: Stdlib + Self-Hosting Migration

1. Audit all `.bv` files for remaining magic method calls
2. Replace with arrow syntax where applicable
3. Verify self-hosting compiler output matches interpreter output

## Impact on Existing Plans

This plan **replaces Phase 5** of the [LLVM Backend Completion Plan](./2026-06-03-llvm-backend-completion.md). The original Phase 5 (runtime-sized allocation) was blocked by a language design question: how does Briev express variable-sized collections? Arrow mutation + contract-proven bounds answers this. The compiler proves the max bound from the contract, allocates once, and the arrow operations mutate in-place within that bound.

**Phase 6** (ForAll/Exists) is **dropped**. Both interpreter and LLVM backend are stubs. No benchmark uses them. No plan to revive.

**Phase 7** (nested recursive types) remains as a future item but is no longer in the active plan. Arrow mutation on `List<List<Int>>` handles all practical nesting use cases.

## Relationship to Existing Features

| Existing Feature | Arrow `<-` Interaction |
|-----------------|----------------------|
| 2-slot list header (Phase 3) | `<` operations use header directly — no new layout |
| Masked SIMD assignment `[;>5]` | Source of insert: `&result <- data[;>5]` |
| Stride `[::2]` | Source of insert: `&result <- data[::2]` |
| Range `[0..10]` | Index for insert/remove: `&list[0..10] <- src` |
| Named dimensions `[channel:0]` | Target for arrow ops, `@` complements |
| MultiSlice | Composes with arrow ops natively |
| Struct types | Field access in filters: `data[; age > 18]` |

## Equivalence Reference

| Briev (new) | Python | Rust | LINQ (C#) |
|------------|--------|------|-----------|
| `&list <- x` | `list.append(x)` | `list.push(x)` | `list.Add(x)` |
| `x <- &list` | `x = list.pop()` | `x = list.pop()` | `list.RemoveAt(list.Count - 1)` |
| `<- &list` | `list.pop()` | `list.pop().unwrap()` | `list.RemoveAt(list.Count - 1)` |
| `&list[i] <- x` | `list.insert(i, x)` | `list.insert(i, x)` | `list.Insert(i, x)` |
| `x <- &list[i]` | `x = list.pop(i)` | `x = list.remove(i)` | `list.RemoveAt(i)` |
| `&result <- data[;>5][::2]` | `result.extend(filter(data, >5)[::2])` | multi-line for loop | `.Where(>5).Stride(2).ToList()` |
| `tensor[@5: 0..16]` | `tensor[:,:,:,0..16]` (positional) | `tensor.index_axis(5, 0..16)` | manual indexing |
| `tensor[..., 0]` | `tensor[..., 0]` | `tensor.select(Axis(4), 0)` | `tensor[:,:,:,0]` |
| `Vector<Int, @32:4>` | `np.ones((32,) + (4,)*32)` | `let x: [[i32; 4]; 32]` | N/A |

## Non-Regression Guarantee

Arrow mutation codegen is additive — new match arms in `emit_expr` for `ArrowMut`/`ArrowDiscard`. No existing optimization path is touched:
- Dead-field elimination: ArrowMut produces a new collection pointer. The old one is dead. DFE already handles this.
- Pure-body fold: If an arrow operation uses an FFI-source value, `statement_contains_ffi` keeps the loop live. Already works.
- SROA/SLP hazard: Arrow operations on scalars hit the same float/int paths as existing ops.
- Enum/struct codegen (Phases 1-4): Unaffected. No code paths shared.

## Summary Table

| Feature | Token | AST Type | Parser | Interpreter | LLVM | Stdlib |
|---------|-------|----------|--------|-------------|------|--------|
| Arrow mutation | `<-` | `ArrowMut`, `ArrowDiscard` | Assignment / statement | 4 new arms, remove magic calls | 2-slot header ops | Rewrite collections, stack, queue |
| Ellipsis | `...` | `Expr::Ellipsis` | Bracket expansion | Desugared (never reached) | Desugared (never reached) | N/A |
| @ dimensions | `@` | `SliceCoordinate::AtDimension` | Slice coordinate parsing | Multi-dim resolution | Deferred | N/A |
| @ in types | `@` | Parser-only desugar | Type parsing expansion | N/A | N/A | N/A |
