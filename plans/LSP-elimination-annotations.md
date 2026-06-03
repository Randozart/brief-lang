# LSP Inline Annotations for Dead-Field Elimination

## Goal
In the LSP, show visual annotations (dimmed text, strikethrough, or inlay hints)
next to source lines whose stores are eliminated by dead-field analysis or
pure-counter fold. The programmer sees *why* and *what* the compiler removed.

## Requirements

### 1. Source-location threading
Every `Statement` in the AST needs a `SourceLoc` (file, line, column, span length)
that maps back to the original source text. Currently `Statement` has no source
location info.

New field on `Statement::Assignment`, `Statement::Let`, etc.:
```rust
pub struct SourceLoc {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub span: u32,  // length in characters
}
```

### 2. Machine-readable diagnostic codes
A002 and A003 currently emit formatted strings. The LSP needs structured data:
- `code: "A002"`, `message: "field 'x1' stores eliminated"`, `range: {file, line, col}`
- `code: "A003"`, `message: "txn folded to O(1)"`, `range: {file, line, col}`

Add a structured diagnostic type that the LSP handler can query.

### 3. LSP diagnostic channel
Add an `lsp_diagnostics: Vec<LspDiagnostic>` field to `LlvmBackend`, populated
alongside the string warnings in `generate()`. The LSP handler reads this after
compilation.

```rust
pub struct LspDiagnostic {
    pub code: String,         // "A002", "A003"
    pub severity: String,     // "info"
    pub message: String,
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub span: u32,
}
```

### 4. IDE rendering
- **A002 (dead-field)**: Dim the entire assignment line `&x1 = b0 * input;`
- **A003 (folded txn)**: Add inlay hint `// folded to O(1) — 50M iterations eliminated`
  after the txn declaration line

### 5. Implementation order
1. Add `SourceLoc` to all `Statement` variants in the parser
2. Thread source locations through `ReactorNode` body statements
3. Add `LspDiagnostic` collection in `generate()` alongside existing warnings
4. Add LSP handler query endpoint `textDocument/codeLens` or `textDocument/diagnostic`
5. Test with ring_buffer.bv, const_heavy.bv, iir_filter.bv

### 6. Backward compatibility
String warnings (`self.warnings`) remain unchanged for CLI users.
`--no-dead-info` also suppresses LSP diagnostics.
