# LSP & SPEC Handoff — State Audit

**Created:** 2026-05-27
**Tests baseline:** 269 passing

---

## Part 1: LSP Current State

### File: `src/lsp.rs` — 905 lines

### Registered Capabilities (what LSP advertises)
1. ✅ `textDocumentSync` (openClose + Full change)
2. ✅ `hoverProvider`
3. ✅ `definitionProvider`
4. ✅ `documentSymbolProvider`
5. ✅ `workspaceSymbolProvider`
6. ✅ `completionProvider` (trigger chars: `.`, `#`)

### Request Handlers Implemented
- `textDocument/hover` — returns type/markdown info
- `textDocument/definition` — go-to-definition within same file
- `textDocument/completion` — static keyword list
- `textDocument/documentSymbol` — document outline
- `workspace/symbol` — cross-document symbol search

### Notification Handlers
- `textDocument/didOpen` — parses document, sends diagnostics
- `textDocument/didChange` — re-parses, sends diagnostics

### Diagnostics Sources
- `Parser::new().parse()` — syntax errors
- `TypeChecker::new().check_program()` — type errors
- `ProofEngine::new().verify_program()` — proof errors
- All sent via `textDocument/publishDiagnostics`

---

### Confirmed Bugs

#### Bug 1: Strict Mode Not Wired
- **Location**: `lsp.rs:321-370` `run_type_check()`
- **Problem**: Uses `Parser::new(&source)` and `ProofEngine::new()` — never calls `with_strict_mode(true)` even for `.sbv`/`.sebv` files
- **Claimed in ROADMAP.md**: ✅ C1 "done"
- **Actual**: ❌ Source shows no strict mode
- **Fix**: Add URI extension check (`.sbv`/`.sebv` → strict), pass to Parser + ProofEngine

#### Bug 2: Single-Error Parser Short-Circuit
- **Location**: `src/parser.rs` `parse_body()`, `parse_statement()`
- **Problem**: First parse error returns `Err` — no more errors reported
- **Impact**: LSP only shows one diagnostic even when multiple errors exist
- **Fix**: Add statement-level error recovery (`sync_to_next_stmt()`) that skips to `;` and continues collecting

#### Bug 3: O(n) Symbol Lookups
- **Location**: `lsp.rs:602-670` `handle_hover()`, `handle_definition()`
- **Problem**: Scans `program.items` linearly on every hover/definition request
- **Fix**: Build `HashMap<String, &TopLevel>` once in `check_document()`, reuse for all queries

### Planned But Not Started

| Feature | ROADMAP ID | Dependencies |
|---------|-----------|-------------|
| Ghost text (inlay hints) | C6 | Phase 0 analysis module |
| Semantic highlighting (`semanticTokens`) | C7 | None |
| Auto-launch VS Code extension | C4 | Config exists, manifest needs update |

---

## Part 2: SPEC v0.14 Gaps

### Spec vs Implementation Cross-Reference

| Spec § | Feature | Implemented? | Where | Notes |
|--------|---------|-------------|-------|-------|
| **§10** | `alka {}` / `alka! {}` blocks | ❌ | Neither AST | Need `StmtAlka` variant in both Brief and Rust ASTs |
| **§11.1** | `#tag(expr)` value hashtags | ❌ | Parser | Only `#tag` and `#!tag` — parenthesized value not parsed |
| **§11.2** | `#on_exit { ... };` block pragmas | ❌ | Neither AST | Need parsing support + proof engine verification |
| **§11.4 pos 5** | Per-body hashtags before variant | ❌ | Parser | `[pre] #tag { body }` syntax not supported |
| **§12** | Multi-body dispatch `[pre]{body}[pre]{body};` | ❌ | Neither AST | Major feature — multiple bodies per txn/defn/struct |
| **§12.3** | Type/struct discriminant variants | ❌ | Neither AST | `type GPU { ... } [has_ce] { + ce_engine };` |
| **§12.3** | `+member` / `-member` differential syntax | ❌ | Neither AST | |
| **§13** | Dynamic `@ expr` address binding | ❌ | Parser | Only literal `@0xADDR` — no expression support |

### Spec Features That ARE Implemented (cross-check passed)
- `#tag` / `#!tag` hashtag modifiers — ✅ (both parsers)
- `#!A|B|C` fallback chains — ✅ (both parsers)
- `#[target]tag` scoped tags — ✅ (both parsers)
- Hashtag positions: `let`, `term`, `&` assignments, after `}` — ✅
- Strict mode contract verification — ✅
- Backend registry hashtag validation — ✅ (`validate_hashtags()`)

---

## Part 3: Language Changes Since SPEC Was Written

Changes that exist in the compiler but are not in the SPEC:

1. **`++` list concatenation operator** — `Expr::Concat` variant added to Rust AST and parser. Lexer has `PlusPlus` token. Not documented in spec.
2. **`.N` tuple field access** — `tuple.0` syntax accepted in parser postfix expressions. Not documented.
3. **`[true][true]` hard error** — parser rejects both-trivial contracts at parse time regardless of strict mode. Spec says "in strict mode" but implementation is unconditional.
4. **`rstruct` keyword** — Rendered Struct: `rstruct Name { field: Type, txn method() { } }`. Parser accepts it. Not in SPEC.
5. **`render` keyword** — `render Name { ... }` view body. Parsed as `TopLevel::RenderBlock`. Not in SPEC.
6. **`ForAll`/`Exists` expression nodes** — `forall x { expr }` / `exists x { expr }`. Rust parser handles them. Not in SPEC.
7. **`Type::ContractBound`** — `Type[expr]` syntax for constrained types. Not in SPEC.
8. **`Type::TypeVar`** — type variables (`'a`, `'T`). Not in SPEC.

---

## Part 4: Recommended Fix Order

### LSP (ordered by impact)
| Priority | Fix | Effort |
|----------|-----|--------|
| P0 | **Bug 1**: Wire strict mode in `run_type_check()` | 30 min |
| P0 | **Bug 3**: Build symbol cache on `check_document()`, use in hover/definition | 1 session |
| P1 | **Bug 2**: Add statement-level error recovery to parser | 1 session |
| P1 | Add `alka` keyword to lexer + AST (matching existing `asm` support) | 30 min |
| P2 | Ghost text (inlay hints) | 2-3 sessions |
| P2 | Semantic highlighting | 2 sessions |

### SPEC (ordered by significance)
| Priority | Task | Effort |
|----------|------|--------|
| P0 | Document `++`, `.N`, `[true][true]` error, `rstruct`, `render`, `ForAll`/`Exists`, `TypeVar`, `ContractBound` | 1 session |
| P1 | Implement multi-body dispatch (§12) — biggest language gap | 2-3 weeks |
| P2 | Implement `alka` blocks (§10), `#on_exit` (§11.2), `#tag(expr)` (§11.1) | 1 session each |
| P3 | Implement dynamic `@` (§13), discriminant variants (§12.3) | 1 week |
 
