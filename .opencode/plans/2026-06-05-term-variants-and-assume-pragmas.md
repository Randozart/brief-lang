# Plan: Term Swan-Song, TermBang, Assume-Event, Assume-Shape

**Date**: 2026-06-05
**Status**: Implementation-ready
**Tests**: All additive — existing 435 tests unchanged

## Overview

Four language features that extend the terminator and pragma systems:

1. **`term -> swan_song;`** — Commit action on postcondition acceptance
2. **`term!` + `term! -> cleanup;`** — Program exit with centralized exit block
3. **`#assume_event(trigger_name)`** — Liveness fairness constraint for external-trigger loops
4. **`#assume_shape(guard_expr, escape|run|exit)`** — Runtime guard with fast-path/slow-path codegen

Design principles:
- All four are **additive** — new match arms only, no modification of existing optimization paths
- `->` token already exists (`Token::Arrow` at `src/lexer.rs:260`)
- `#` token already exists with `parse_hashtag_modifiers` infrastructure
- `term!` follows `frgn!` token pattern (`Token::FrgnBang` at `src/lexer.rs:100`)
- Interpreter is reference — implement there first before LLVM backend

## Phase 1: `term -> swan_song;`

### Semantics
- `term;` is a symbolic compile-time checkpoint — triggers "postcondition satisfied?" verification
- `term -> swan_song;` is a **commit action**: `swan_song` executes **only if** postcondition is accepted
- Swan song is NOT a loop increment — it fires once on acceptance, not every tick
- In the LLVM backend, swan song goes into the postcondition-satisfied branch after the `icmp` check

### AST (`src/ast.rs:559-562`)
```rust
Term {
    values: Vec<Option<Expr>>,
    swan_song: Option<Box<Statement>>,   // NEW
    modifiers: Vec<Hashtag>,
}
```

This is a breaking change to the struct — every `Statement::Term { values, modifiers }` destructuring must be updated to `{ values, modifiers, .. }` or include `swan_song`. Fortunately, most match arms use `{ .. }` or `{ values, .. }` which already ignore unknown fields. Only arms that explicitly name `modifiers` need updating.

### Parser (`src/parser.rs:3486-3491`)
After parsing `term` and its output expressions, before expecting `;`:
1. Check for `Token::Arrow` (`->`)
2. If found, call `parse_statement()` to get the swan song
3. Wrap as `Some(Box::new(swan_song))`
4. Expect `;`

### Interpreter (`src/interpreter.rs:766`)
In the `Statement::Term` eval path:
1. Evaluate first output as before (sets `return_value`)
2. If swan song present AND postcondition accepted: `exec_stmt(swan_song)`

### LLVM Backend (`src/backend/llvm.rs:2273`)
In the postcondition commit block (after `icmp` check):
- If swan song present: emit swan song statements before `ret`
- If no swan song: unchanged

### Transition Graph (`src/analysis/transition_graph.rs:687`)
- `statement_contains_ffi` must recurse into swan song (if present)

### Docs
- `spec/SPEC.md`: Add `term -> statement` to §2.5 (Statements grammar), add semantics in §5.4.2
- `spec/LANGUAGE-TUTORIAL.md`: Add subsection on commit actions in Part 9 (Advanced Patterns)

## Phase 2: `term!` + `term! -> cleanup;`

### Semantics
- `term!` is **program termination** — compiles to a direct branch to a centralized exit block
- `term! -> cleanup;` executes cleanup before termination
- Cleanup is a commit action (same as swan song) — only fires on postcondition acceptance
- In the interpreter, `term!` sets `return_value` then signals program exit (stops execution)
- Centralized `exit_block` in `emit_main` handles cleanup sequence + `ret i32 0`

### Lexer (`src/lexer.rs:78`, adjacent to `Term`)
```rust
#[token("term!")]
#[token("TERM!")]
TermBang,
```

### AST (`src/ast.rs:563-566`)
```rust
TermBang {
    values: Vec<Option<Expr>>,
    swan_song: Option<Box<Statement>>,   // cleanup action
    modifiers: Vec<Hashtag>,
}
```

### Parser
Handle `Token::TermBang` same as `Token::Term` but constructs `Statement::TermBang`:
1. Parse optional output expressions
2. Check for `->` for optional swan song
3. Expect `;`

### All Match Arms (~40 across ~22 files)

Files with `Statement::Term` match arms that need `Statement::TermBang` counterparts:

| File | Lines | Strategy |
|------|-------|----------|
| `src/interpreter.rs` | 364, 766 | Execute swan song, set return_value, then exit |
| `src/backend/llvm.rs` | 93, 2248, 2273, 3894, 3915, 4125, 4141 | 93: collect strings; 2248: skip in fused compose; 2273: emit swan song + br exit_block; 3894/3915: filter in SSA loop; 4125/4141: filter in dispatch |
| `src/backend/mod.rs` | 162 | Collect modifiers |
| `src/backend/rust.rs` | 436 | Emit `return;` |
| `src/backend/x86_64.rs` | 452 | Comment + pending_cleanup |
| `src/backend/aarch64.rs` | 453 | Comment + pending_cleanup |
| `src/backend/webstack.rs` | 1338 | Comment + pending_cleanup |
| `src/backend/cobol.rs` | 452 | `STOP RUN.` |
| `src/backend/vhdl.rs` | 833 | Comment |
| `src/backend/verilog.rs` | 760, 1543 | Return expr / comment |
| `src/backend/wasm.rs` | 61, 800, 905 | Comment / return |
| `src/backend/c.rs` | 846 | Comment |
| `src/typechecker.rs` | 801 | Validate outputs |
| `src/annotator.rs` | 62, 333 | Format output |
| `src/proof_engine.rs` | 879, 1631, 2785, 2818, 2894, 3033 | Terminates = true, collect variables |
| `src/analysis/region.rs` | 994, 1295, 1304, 1327, 1388, 1473 | Is terminator, counts, substitution |
| `src/analysis/transition_graph.rs` | 687, 871 | FFI check |
| `src/analysis/dataflow.rs` | 227 | Extract IDs |
| `src/reactor.rs` | 259 | TermSuccess |
| `src/desugarer.rs` | 366, 396, 484 | Has term with expr |
| `src/symbolic.rs` | 429 | Path ends |
| `src/assertion_verify.rs` | 119 | Check provable |
| `src/fuzzing/ast_generator.rs` | 162, 309, 310, 612 | Generate terminator |
| `src/parser.rs` | 3491, 5513 | Construct terminator |
| `src/annotator.rs` | 333 | Format |

**Conservative strategy**: Every `Statement::Term { .. }` or `Statement::Term { values, .. }` arm gets a `Statement::TermBang { .. }` or `Statement::TermBang { values, .. }` arm that behaves identically. Only the LLVM backend and interpreter get special treatment.

### LLVM: Centralized Exit Block (`src/backend/llvm.rs:3717`)
```llvm
exit_block:
  ; cleanup sequence
  ; (populated by term! swan songs)
  ret i32 0
```
- `term!` without swan song: `br label %exit_block`
- `term!` with swan song: emit swan song, then `br label %exit_block`

### Docs
- `spec/SPEC.md`: Add `term!` to §2.5 (Statements grammar), add `exit_block` semantics in §5.4.2
- `spec/LANGUAGE-TUTORIAL.md`: Add subsection on `term!` in Part 9

## Phase 3: `#assume_event(trigger_name)`

### Semantics
- `#assume_event(stdin_ready)` declares that `stdin_ready` **will** fire eventually
- The proof engine treats this as a fairness constraint — can prove termination for external-trigger loops
- Compiler emits no new code — purely a proof-engine/analysis hint
- Uses existing `Hashtag { name: "assume_event", value: Some("stdin_ready") }` infrastructure

### Parser (`src/parser.rs:608-640`)
Currently, `Token::Hash` (`#`) is NOT handled at the top level. Only `#![...]`, `#pragma`, `#!`, `#!pragma` are.
- Add `Token::Hash` handling in `parse_top_level` loop:
  1. Call `parse_hashtag_modifiers()` to get `Vec<Hashtag>`
  2. Store modifiers in a `pending_attrs` field or similar
  3. When the next `TopLevel` is parsed (e.g., transaction or definition), attach the stored modifiers

### Transition Graph / Proof Engine (`src/analysis/transition_graph.rs`)
- In termination analysis: when a txn has `#assume_event(trg_name)`, add `trg_name` to "guaranteed-to-fire" set
- The `has_wake_triggers` logic already tracks triggers — extend to accept guaranteed triggers as sufficient for termination proof

### No LLVM codegen change
The proof engine just relaxes its liveness constraints. The backend never needs to know.

### Docs
- `spec/SPEC.md`: Add `#assume_event` to §6 (Compiler Pragmas)
- `spec/LANGUAGE-TUTORIAL.md`: Mention in Part 10 (Performance & Optimization)

## Phase 4: `#assume_shape(guard_expr, escape|run|exit)`

### Semantics
- `#assume_shape(packet :> PaymentTxn, escape)` declares that `packet` is expected to match `PaymentTxn` shape
- Compiler generates a runtime guard: `if !(guard) { rollback_action }`
- **Fast path** (guard passes): optimized body — compiler assumes shape, strips runtime type checks
- **Slow path** (guard fails): `escape` = skip txn, `run` = execute with checks, `exit` = terminate

### Parser
Parsed via the same top-level `#` mechanism as Phase 3. The value string `"packet :> PaymentTxn, escape"` is stored as-is in `Hashtag.value`. Analysis stage splits on `, ` (last comma-space).

### Analysis (`src/analysis/transition_graph.rs`)
- Extract guard expression and rollback action from `Hashtag.value`
- Validate rollback is one of: `escape`, `run`, `exit`
- Mark txn for fast-path/slow-path codegen

### LLVM Backend (`src/backend/llvm.rs`, new block, ~80 lines)
```llvm
entry:
  %shape_ok = call @evaluate_guard(%state)
  br i1 %shape_ok, label %fast_path, label %slow_path

fast_path:
  ; emit body with assumption (no runtime type checks)
  ; term swan song commit
  ret void

slow_path:
  switch action {
    escape: br %reactor_tick       ; skip this txn
    run:    call @body_with_checks ; fall through
    exit:   call @__exit(1)
            unreachable
  }
```

The fast path body is the same transaction body but with the compiler's knowledge that the shape guard holds — dead branch elimination, type check stripping, etc.

### Docs
- `spec/SPEC.md`: Add `#assume_shape` to §6 (Compiler Pragmas), describe guard/fast-path/slow-path behavior
- `spec/LANGUAGE-TUTORIAL.md`: Add example in Part 9

## File Change Summary

| File | Phase | Type | Change |
|------|-------|------|--------|
| `src/lexer.rs` | 2 | Add | `Token::TermBang` variant + `#[token("term!")]` |
| `src/ast.rs` | 1,2 | Modify/Add | `Term::swan_song` field, `TermBang` variant |
| `src/parser.rs` | 1,2,3 | Modify | `->` after term, `TermBang` handling, top-level `#` |
| `src/interpreter.rs` | 1,2 | Modify | Swan song eval, TermBang exit |
| `src/backend/llvm.rs` | 1,2,4 | Modify | Commit block, exit_block, assume_shape |
| `src/backend/*.rs` (9 files) | 2 | Modify | `TermBang` match arms |
| `src/backend/mod.rs` | 2 | Modify | `TermBang` match arm |
| `src/typechecker.rs` | 2 | Modify | `TermBang` validation |
| `src/annotator.rs` | 1,2 | Modify | Swan song + TermBang format |
| `src/proof_engine.rs` | 2 | Modify | 6 match arms |
| `src/analysis/region.rs` | 2 | Modify | 6 match arms |
| `src/analysis/transition_graph.rs` | 1,3,4 | Modify | FFI recurse, assume_event, assume_shape |
| `src/analysis/dataflow.rs` | 2 | Modify | 1 match arm |
| `src/reactor.rs` | 2 | Modify | 1 match arm |
| `src/desugarer.rs` | 1,2 | Modify | Swan song recurs, TermBang arms |
| `src/symbolic.rs` | 2 | Modify | 1 match arm |
| `src/assertion_verify.rs` | 2 | Modify | 1 match arm |
| `src/fuzzing/ast_generator.rs` | 2 | Modify | TermBang generation |
| `spec/SPEC.md` | 1,2,3,4 | Modify | Grammar + semantics |
| `spec/LANGUAGE-TUTORIAL.md` | 1,2,3,4 | Modify | Examples + explanation |

## Verification
- `cargo test --lib` after each phase
- All 435 existing tests must pass unchanged
- Add new tests for:
  - `term -> swan_song` parsing + interpreter (3 tests)
  - `term!` parsing + interpreter (3 tests)
  - `#assume_event` parsing (2 tests)
  - `#assume_shape` parsing + analysis (3 tests)
