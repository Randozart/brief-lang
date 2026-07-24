# Doc Comments, Block Comments, and Compound Operators

**Date:** 2026-07-24
**Status:** Plan → Implementation

## Features

| Feature | Effort | Files |
|---------|--------|-------|
| `*=` / `/=` compound assignment | ~15 min | `lexer.rs`, `statements.rs` |
| `/* */` block comments | ~10 min | `lexer.rs` |
| `///` / `//!` doc comments | ~half day | `lexer.rs`, `top.rs`, `definitions.rs`, `parser/mod.rs` |

---

## Phase 1: `*=` and `/=` (implementation first)

Lexer tokens `#[token("*=")] StarEq` and `#[token("/=")] SlashEq`.
Parser arms mapping to `BinaryOpKind::Mul` / `BinaryOpKind::Div`.
Evaluator already handles Mul/Div.

## Phase 2: `/* */` block comments

Single Logos skip pattern. No nesting (C-style).

## Phase 3: Doc comments

### Lexer

`/// text` → `DocComment("text")` token (not skipped — captured as a token)
`//! text` → `DocCommentBang("text")` token

### AST

New fields:
- `Definition.doc: Option<String>`
- `Transaction.doc: Option<String>`
- `Struct.doc: Option<String>`
- `Enum.doc: Option<String>`
- `CellDef.doc: Option<String>`
- `ForeignBinding.doc: Option<String>`
- `CompileTimeDefn`, `CompileTimeTxn`: same
- `TopLevel::FileDoc(String)` for `//!` before any item

### Parser

Before each top-level parse, check if current token is DocComment.
If so, store the text and attach it to the next parsed definition.
DocCommentBang before any item → `TopLevel::FileDoc`.

### Extraction

`FileDoc` extracted by `extract_inline_stage_blocks` and stored on
`PluginManager.file_doc` (not codegen'd).
Doc comments on `$defn`/`$txn` preserved in stage block extraction.
