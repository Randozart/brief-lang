# Plan: Enhanced LSP for Brief Language

## Goal
Add full Language Server Protocol (LSP) features to enable IDE integration with hover, goto definition, completions, and document symbols.

## Current State
- Existing LSP server (`src/lsp.rs`) handles: initialize, shutdown, textDocument/didOpen, textDocument/didChange
- Publishes type-checking and proof verification diagnostics
- Uses stdio for communication

## Required Changes

### 1. Configure Extension for Auto-Launch
Add language server configuration to `syntax-highlighter/package.json`:
```json
"languageServer": {
    "brief": {
        "command": "brief",
        "args": ["lsp"],
        "languages": ["brief", "rbv", "ebv"]
    }
}
```

### 2. Enhance LSP Capabilities in `src/lsp.rs`

Add handlers for:
- `textDocument/hover` → Return type information
- `textDocument/definition` → Go to definition
- `textDocument/completion` → Keyword/function suggestions
- `textDocument/documentSymbol` → Outline/structure view
- `workspace/symbol` → Global symbol search

### 3. Implement Type Info Extraction
- Parse AST to extract types, functions, variables
- Cache symbol locations for quick lookup
- Store type info for hover/definition queries

### 4. Add Completion Items
Populate from:
- Keywords: `txn`, `rct`, `let`, `const`, `sig`, `defn`, `trg`, `import`, `from`, `term`, `escape`, `async`
- Types: `Int`, `UInt`, `Float`, `String`, `Bool`, `Data`, `Void`
- Standard library functions

## Files to Modify
- `src/lsp.rs` - Add all new LSP handlers
- `syntax-highlighter/package.json` - Add languageServer config

## Difficulty
Medium (~4-6 hours)
- ~1 hour for auto-launch config
- ~3-5 hours for LSP handlers

## Verification
Run `brief lsp` and test with VS Code:
- Open `.bv` file → should get diagnostics on type/check errors
- Hover over variable → shows type
- Ctrl+Click → goes to definition
- Ctrl+Space → shows completions