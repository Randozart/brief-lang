# Brief Optimization Plan

**Created:** 2026-05-27
**Status:** Ready for implementation
**Tests baseline:** 269 passing

---

## 1. Dual-Mode Architecture

Brief supports two build modes:

### Full Mode (`brief build`, default)
Current behavior. Idiomatic, debuggable target-native output. All 10 backends unchanged. Suitable for development, debugging, and cyclic code that cannot be pre-scheduled.

### Optimized Mode (`brief build -O`)
Thin DAG emission. The shared pipeline pre-computes everything (call graph, parameter ranges, dataflow, fusable pairs, peephole transformations, memory overlay, guard caching) and backends receive pre-resolved acyclic instruction sequences. Output is smaller and faster but unreadable.

### Gating
Only acyclic subgraphs are optimized. `CallGraph::is_acyclic()` guards each transaction. Cyclic code passes through full mode. Mixed programs get mixed output — per-transaction optimization. The optimizer skips:
- Transactions in cycles
- Transactions with dynamic dispatch
- Transactions with `volatile` hashtag
- Transactions with external `frgn` calls that have side effects

---

## 2. Wire Up Dead Analysis Code

Currently ~350 lines of shared analysis infrastructure is fully implemented but never called.

### 2.1 `analyze_program()` — Entry point
- **Location:** `src/backend/mod.rs:24`
- **Status:** Defined, never called
- **Work:** Call from `main.rs` dispatch for both modes. Return `(CallGraph, ParameterRanges)`.
- **Praetor:** Add `/// Intent:` comment

### 2.2 Thread analysis results into backends
All 10 `generate()` methods need new signatures accepting `&CallGraph` + `&ParameterRanges`. Start with the optimized path — in full mode, pass empty/defaults if backends don't use them.

### 2.3 `detect_fusable_pairs()`
- **Location:** `src/backend/mod.rs:246`
- **Status:** Defined, never called
- **Work:** Call in optimized mode. Fused transactions become atomic. Post(A) implying pre(B) means A+B execute as one unit with no interleaving check.

### 2.4 `DataflowAnalyzer`
- **Location:** `src/analysis/dataflow.rs:22`
- **Status:** Defined, never called (uses `&'static Program` — needs fix to eliminate static lifetime)
- **Work:** Fix `&'static` → proper lifetime. Wire into optimized pipeline for use-before-set and weight-load ordering detection.

### 2.5 Peephole optimizer
- **Status:** Pseudocode only in `OPTIMIZATIONS.md`
- **Work:** Implement as shared pipeline pass. Pattern-matches common suboptimal sequences and rewrites them. Called in optimized mode after all other analysis.

### 2.6 Memory overlay + guard caching
- **Status:** Per-backend in c.rs, rust.rs only
- **Work:** Extract to shared pipeline. Called once, results passed to backends. Removes duplication across 10 backends.

---

## 3. Fix Brief Self-Hosted Bugs

These bugs prevent Brief from compiling itself. Fixing them is a prerequisite for the self-hosted compiler to be a viable development platform.

### 3.1 `backend_aarch64.bv` — undefined `NOT()` call
- **Location:** `lib/compiler/backends/backend_aarch64.bv:507`
- **Bug:** `let inverted_mask = NOT(field.mask);` — `NOT` is an instruction variant, not a function
- **Fix:** Inline bitwise NOT: `let inverted_mask = !field.mask;` or define a helper.

### 3.2 `range.bv` — non-existent AST constructors
- **Location:** `lib/compiler/range.bv`
- **Bug:** Uses `IdentExpr`, `IntLit`, `AndExpr`, `GtExpr`, `GeExpr`, `LtExpr`, `LeExpr` — none exist in `ast.bv`
- **Fix:** Replace with actual AST types: `ExprVar`, `ExprInt`, `ExprBinOp("&&")`, `ExprBinOp(">")`, etc.

### 3.3 `wasm.bv`, `vhdl.bv`, `verilog.bv` — pseudo-code
- **Bug:** Rust-flavored pseudo-code. Uses `transaction`, `vec!`, `HashMap::new()`, `format!`, `&mut`, `&ctx`, `||`, pattern matching with `=>`, `use std::*`.
- **Fix:** Full rewrite in valid Brief. These are 360-500 lines each.

### 3.4 `rust.bv:210`, `c.bv:197` — Rust syntax
- **Bug:** `String::new()` — Rust method syntax
- **Fix:** Replace with Brief-idiomatic string construction.

### 3.5 `parser.bv` — unimplemented dispatch
- **Functions:** `parse_render()`, `parse_rstruct()`, `parse_foreign_signature()` — called from `parse_top_level`, never implemented
- **Fix:** Implement these to parse `render {}`, `rstruct {}`, and `frgn` declarations.

### 3.6 `main.bv` — backend stubs
- **Backends:** `compile_to_wasm`, `compile_to_webstack`, `generate_rust` — all emit `"Hello from Brief"` only
- **Fix:** Real implementations matching the Brief self-hosted backend files.

---

## 4. Praetor Compliance (Incremental)

### Strategy
Fix intent comments on every file we touch, plus spillover to adjacent files. Track the delta per session.

### Current State
- 1,675 unproven diagnostics total
- ~217 fixed (src/backend/)
- ~1,458 remaining
- Pre-commit hook blocks on any diagnostic

### Pattern
```
/// Intent: [one-line description of what this function does]
///   Precondition: [what must be true before, or "None"]
///   Postcondition: [what is guaranteed after, or "None"]
pub fn ...
```

### Datalog Rule 1 (Phase C)
Functions accessing private/secret/password data must call authenticate/authorize/login first. Currently 0 fixes applied. Address after Phase B intent work is substantially complete.

---

## 5. Parser Completeness (Stretch)

### Rust Parser — AST nodes defined but not parsable
- `Expr::ListLen(Expr)`
- `Expr::ForAll { var, expr }`
- `Expr::Exists { var, expr }`
- `Expr::Block(Vec<Stmt>, Box<Expr>)` — parser produces Vec<Statement>, never wraps in Expr
- `Type::ContractBound(Type, Expr)`
- `Type::TypeVar(String)`
- `Type::Sig(String)`
- `Type::Enum(String)`
- `TopLevel::Stylesheet(String)`
- `TopLevel::SvgComponent { name, content }`

### Brief Parser — AST nodes defined but not parsable
- `ExprTuple(List<Expr>)`
- `ExprFieldAccess(Expr, String)`
- `ExprIndex(Expr, Expr)`
- `ExprSlice { start, end, stride, mask }`
- `ExprMultiSlice { coords, mask }`
- `ExprCast(Expr, Type)`
- `ExprForAll(String, Expr)`
- `ExprExists(String, Expr)`
- `StmtAsm(String, List<String>)`

### Missing from both (language design gap)
- `if/else` — Brief token.bv has `KeywordIf`/`KeywordElse`, Rust lexer.rs does not
- `while` loops — no tokens in either
- `for` loops — no tokens in either
- `loop` (infinite) — no tokens in either
- `match/switch` — Rust lexer has `Match`, Brief tokens do not
- `bank` — both lexers have it, no parser handler
- `stage` / `on` — both lexers have them, used only inside `trg` parsing
- Closures/lambdas — no tokens in either

---

## 6. Implementation Order

| Priority | Task | Depends On | Effort | Status |
|----------|------|-----------|--------|--------|
| P0 | Wire `analyze_program()` in `main.rs` | None | 1 session | ✅ DONE |
| P0 | Thread CallGraph + ParameterRanges into backends | P0 above | 1-2 sessions | ✅ DONE |
| P0 | Fix `range.bv` AST constructors | None | 1 session | ✅ DONE |
| P0 | Fix `backend_aarch64.bv` NOT() call | None | Minutes | ✅ DONE |
| P0 | Fix `rust.bv`/`c.bv` Rust syntax | None | Minutes | ✅ DONE |
| P0 | Implement `parse_render/rstruct/foreign_signature` | None | 1-2 sessions | ✅ DONE |
| P0 | Create `AnalysisResults` struct + `--optimize` flag | P0 above | 1 session | ✅ DONE |
| P0 | Fix `DataflowAnalyzer` static lifetime | P0 above | 1 session | ✅ DONE |
| P1 | Wire `detect_fusable_pairs()` in -O path | P0 above | 1 session | PENDING |
| P1 | Fix `main.bv` backend stubs | None | 1 session | PENDING |
| P1 | Praetor intent comments on touched files | Every task | Ongoing | PENDING |
| P2 | Implement peephole optimizer | P1 above | 1-2 sessions | ✅ DONE |
| P2 | Extract memory overlay + guard caching to shared | P0 above | 1-2 sessions | ✅ DONE |
| P2 | Rewrite `wasm.bv`, `vhdl.bv`, `verilog.bv` | P0 above | 2-3 sessions | PENDING |
| P3 | Rust parser: missing AST nodes (ListLen, ForAll, Exists, Block, ContractBound, TypeVar, Sig, Enum) | None | 2-3 sessions | ✅ DONE |
| P3 | Brief parser: missing expr types (Tuple, FieldAccess, etc.) | None | 2-3 sessions | PENDING |
| P3 | Language design: if/else, loops, match | CLAUDE.md discussion | Varies | PENDING |

---

## 7. Commit Log

- `15742b2` (2026-05-27): Fix Brief bugs (NOT(), range.bv, String::new()), implement stub parsers, wire analyze_program into all backends
- `current` (2026-05-27): Implement shared peephole optimizer (constant folding, redundant elimination, guard simplification). Extract MemoryOverlay + GuardTracker to shared pipeline. Both wired into --optimize path.
- `c2728c5` (2026-05-27): [same as above - amend for final extraction]

---