# Collection Mutation Language Design — Implementation Report

**Date:** 2026-06-04
**Status:** Part A (complete), Part B (parser complete), Part C (complete)

## Summary

Implemented first-class collection mutation (`<-`, `...`, `@`) eliminating magic string-matching from collection operations. 430 tests pass (424 original + 6 new).

### Part A: `<-` Arrow Mutation (Complete)

**Files:** `src/lexer.rs`, `src/ast.rs`, `src/parser.rs`, `src/interpreter.rs`, `src/backend/llvm.rs`, `src/typechecker.rs`

- **Lexer**: `#[token("<-")] ArrowLeft` token
- **AST**: `ArrowDir { Push, Pop }`, `ArrowMut`, `ArrowDiscard` variants
- **Parser**: Three parsing paths (`<- &list`, `&list <- x`, `x <- &list`) + `extract_arrow_target()` + `is_valid_arrow_inner()` for `&name`/`&name[i]`/`&name.field`/`&name.field[i]`
- **Interpreter**: `ArrowMut`/`ArrowDiscard` handlers with `extract_arrow_root`/`resolve_arrow_list`/`store_arrow_list`/`eval_arrow_pos` helpers, including FieldAccess target support
- **LLVM backend**: Push/Pop/Insert/Remove via 2-slot header (load len from slot 1, store at computed position, update len)
- **Typechecker**: Removed `list_append` and `get` (List) magic signatures
- **Removed magic**: `list_append` and `get` string-match arms removed from interpreter `Expr::Call` handler

### Part B: `...` Ellipsis + `@` Dimension Specifiers (Parser Complete)

**Files:** `src/lexer.rs`, `src/ast.rs`, `src/parser.rs` + match arms in 9 files

- **AST**: `Expr::Ellipsis`, `SliceCoordinate::AtDimension { dimension, coord }`, `SliceCoordinate::Ellipsis`
- **Parser**: `...` and `@N:coord` parsing in `parse_slice_coordinate`; `peek_multidimensional_slice` detects `...` and `@` in scan loop; `parse_postfix` redirects to `parse_multi_slice` for multi-dimensional patterns
- **Match arms**: All 9 files with `SliceCoordinate`/`Expr` matches updated (annotator, dataflow, transition_graph, proof_engine, symbolic, interpreter, llvm, rust, webstack)
- **6 new parser tests**: ellipsis, at-dim, trailing-ellipsis, with AST validation

**Remaining for Part B:**
1. Ellipsis expansion pass (compile-time `...` → wildcard dimensions)
2. `@` dimension validation in type system
3. `@` in type declarations (`Vector<Int, @32:4>`)
4. Multi-dimensional interpreter resolution for `AtDimension`

### Part C: Stdlib Migration (Complete)

**Files:** `lib/std/collections.bv`, `lib/std/stack.bv`, `lib/std/queue.bv`, `lib/compiler/lexer.bv`

- `collections.bv`: `list_append` → `&items <- item`, `get` → `item <- &items`
- `stack.bv`: `push`/`pop` via `<-` instead of O(n) concat/slice
- `queue.bv`: `enqueue`/`dequeue` via `<-` instead of O(n) concat/slice
- `lexer.bv` (self-host): `list_append` → `&tokens <- tok`
- Self-hosting compiler noted as broken (magic handlers removed, deferred)

### Key Design Decisions

- `<-` chosen over `<+`, `<<`, method calls: no token conflict with `<<` (ShiftLeft), bidirectional semantics, matches existing `->`
- Collection mutation is first-class Expr variants, not disguised function calls
- `<-` targets use `&name` (owned-ref dereference) — `&queue.items` is `FieldAccess(OwnedRef("queue"), "items")`
- `list_append`/`get` removed from both interpreter and typechecker
- Stdlib `stack.bv`/`queue.bv` use O(1) `<-` mutation instead of O(n) concat/slice

## Files Modified (18 files)

| File | Changes |
|------|---------|
| `src/lexer.rs` | `ArrowLeft` token (existing) |
| `src/ast.rs` | `ArrowDir`, `ArrowMut`, `ArrowDiscard`, `Expr::Ellipsis`, `SliceCoordinate::AtDimension`, `SliceCoordinate::Ellipsis` |
| `src/parser.rs` | `<-` parsing, `extract_arrow_target`, `is_valid_arrow_inner`, `...`/`@` parsing, `peek_multidimensional_slice` detection |
| `src/interpreter.rs` | Arrow handlers + helpers (extract/store/resolve_arrow, eval_arrow_pos); removed list_append/get magic |
| `src/backend/llvm.rs` | ArrowMut/ArrowDiscard codegen via 2-slot header; match arms for Ellipsis/AtDimension |
| `src/typechecker.rs` | Removed list_append/get signatures |
| `src/annotator.rs` | Match arms for all new variants |
| `src/analysis/dataflow.rs` | Match arms for all new variants |
| `src/analysis/transition_graph.rs` | Match arms for all new variants |
| `src/proof_engine.rs` | Match arm for Expr::Ellipsis |
| `src/symbolic.rs` | Match arm for Expr::Ellipsis |
| `src/backend/rust.rs` | Match arms for SliceCoordinate::AtDimension/Ellipsis |
| `src/backend/webstack.rs` | Match arms for SliceCoordinate::AtDimension/Ellipsis |
| `lib/std/collections.bv` | Migrated to `<-` syntax |
| `lib/std/stack.bv` | Migrated to `<-` syntax |
| `lib/std/queue.bv` | Migrated to `<-` syntax |
| `lib/compiler/lexer.bv` | Migrated `list_append` → `<-` |
| `AGENTS.md` | Updated with session progress |
