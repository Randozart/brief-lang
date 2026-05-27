# Compiler & Transpiler Perfection Plan

**Created:** 2026-05-27  
**Status:** Ready for implementation  
**Tests baseline:** 269 passing  

---

## Problem Summary

The Brief compiler has two implementations (Rust bootstrap `src/`, ~49K lines; Brief self-hosted `lib/compiler/`, ~11K lines). Currently:

- **42/46** `lib/` files fail to parse or typecheck — the Brief self-hosted compiler cannot compile anything
- **5/12** Rust backends emit placeholder code (comments, `ret void`, `mov x0, #0` fallbacks) instead of functional output
- **9/9** Brief backends fail to parse — none can generate output
- **Syntax errors** just say what went wrong ("expected Eq, found 'LParen'") with no location context, suggestions, or token tracking
- **LSP** missing `index_map` method for symbol table, no strict mode diagnostics
- **Praetor** ~1,800 unproven diagnostics block clean commits

---

## Part 1: Fix Brief Self-Hosted Parse Errors (42 files)

### Root Cause Analysis

| Pattern | Count | Example | Fix |
|---------|-------|---------|-----|
| **1. Rust-ism `Ok(...)`/`Some(...)` wrappers** | ~12 files | `Ok(Star)`, `Ok(Cycles)` | Replace with plain values |
| **2. Missing semicolons** | ~6 files | Parser hits `RBrace` | Add `;` before closing `}` |
| **3. `[true][true]` meaningless contracts** | ~4 files | Both pre/post are `[true]` | Replace one side |
| **4. `error[B002]` type mismatches (generics)** | ~8 files | `HashSet` used where `HashSet<T>` declared | Add type params |
| **5. `error[B002]` contract-bound returns** | ~4 files | Returns `Bool` but declares `Bool[msg != ""]` | Add contract check |
| **6. `error[P008]` proof failures** | ~4 files | Post-condition unprovable | Strengthen invariants |
| **7. `error[F101/102]` FFI Result** | ~3 files | FFI call not matched | Add Result branching |
| **8. `::` paths** | ~2 files | `module::Type` | Replace with `.` |
| **9. `use`/`as`/`comptype` keywords** | ~2 files | Rust `use` at top level | Replace with `import` |
| **10. Dot method in assignment** | ~2 files | `obj.method()` where `=` | Rewrite as function call |

### Step 1.1: Fix Rust-ism patterns in lib/compiler/ core (parser.bv, range.bv, call_graph.bv, proof_engine.bv)

### Step 1.2: Fix Rust-isms in lib/compiler/backends/ (verilog.bv, backend_aarch64.bv, x86_64.bv)

### Step 1.3: Missing semicolons in typechecker.bv, shm.bv, json.bv, encoding.bv

### Step 1.4: `[true][true]` contracts in wasm_mapper.bv, c_mapper.bv, iterator.bv

### Step 1.5: Generic type mismatches and contract-bound returns in stdlib

### Step 1.6: Proof failures in token.bv, lexer.bv, collections.bv

### Step 1.7: FFI Result handling in http.bv, metro_bridge.bv

### Step 1.8: `::` paths, `use`/`as`, dot method syntax

---

## Part 2: Fix Backend Stubs (5 Rust backends)

| Backend | Problem | Fix |
|---------|---------|-----|
| **wasm.rs** | `local_index()` returns 0 | Proper local variable indexing |
| **x86_64.rs** | Comment fallbacks for calls, floats, strings | Real x86-64 codegen |
| **aarch64.rs** | Same as x86_64 | Real AArch64 codegen |
| **llvm.rs** | Reactor loop `ret void` | Implement dispatch |
| **vhdl.rs** | 8 statement types emit VHDL comments | Real RTL codegen |

---

## Part 3: Better Syntax Error Messages

| Feature | Current | Target |
|---------|---------|--------|
| Source location | None | `Error at 42:12` |
| Source snippet | None | Line + caret pointing at error |
| Token names | `'LParen'` | `'('` |
| Suggestions | None | `Did you mean just 'Star'?` |

---

## Part 4: End-to-End Transpilation Test

Write a non-trivial Brief program (binary search tree with transactions, guards, contracts, `trg!`), transpile to all 10 backends, verify output compiles/assembles/passes lint.

---

## Part 5: LSP Fixes

| Issue | Fix |
|-------|-----|
| `index_map` missing for `Vec<SymbolInfo>` | Implement or replace with HashMap |
| Strict mode diagnostics | Wire `strict` flag to publishDiagnostics |
| Ghost text / acyclicity | Wire CallGraph into hover |

---

## Part 6: Praetor Compliance

Incremental: add `/// Intent:` to every function touched. Target: ~1,800 → ~1,400.

---

## Implementation Order

| Priority | Part | Task | Depends On | Effort |
|----------|------|------|-----------|--------|
| P0 | 1.1 | Fix Ok()/Some() Rust-isms in core Brief files | None | 1 session |
| P0 | 1.2 | Fix same in Brief backends | 1.1 | 1 session |
| P0 | 1.3-1.8 | Fix remaining Brief parse errors | 1.1 | 2-3 sessions |
| P0 | 3.1-3.2 | Add line:col + source snippet to errors | None | 1 session |
| P1 | 2.1-2.3 | Fix wasm/x86_64/aarch64 stubs | None | 1-2 sessions |
| P1 | 2.4 | Fix LLVM reactor loop | None | 1 session |
| P1 | 2.5 | Fix VHDL backend | 1.2 | 2 sessions |
| P1 | 5.1 | Fix LSP index_map | None | 1 session |
| P2 | 4 | BST test + transpile to all backends | 2.1-2.5 | 1 session |
| P2 | 3.3-3.4 | Token demangling + suggestions | 3.1 | 1 session |
| P3 | 5.2-5.3 | LSP strict mode + ghost text | 5.1 | 1 session |
| P3 | 6 | Praetor incremental | All | Ongoing |