# Briv-in-Briv Self-Hosting Status

**Date**: 2026-05-28  
**Rust compile**: ✓ `cargo build`  
**Tests**: ✓ `cargo test --lib` — 269/269 pass  

## Objective

Make all 18 `.bv` files in `lib/compiler/` parse, type-check, and eventually compile Briv code through the self-hosted compiler.

## Parse Status

### PASS (11/18 files — parse cleanly with no errors)

| File | Notes |
|---|---|
| `ast.bv` | `All checks passed` |
| `token.bv` | No parse errors |
| `lexer.bv` | No parse errors |
| `parser.bv` | No parse errors |
| `typechecker.bv` | No parse errors |
| `proof_engine.bv` | Fixed: `check_vc` returns `Bool` not `Result`, `Some(init)` → `is_some`/`unwrap`, `txn`→`contract_txn`, `from`→`from_str`, empty guarded block removed |
| `call_graph.bv` | Fixed: contract-before-arrow syntax + body restoration |
| `range.bv` | Fixed: contract-before-arrow syntax |
| `main.bv` | No parse errors |
| `backends/mod.bv` | No parse errors |
| `backends/c.bv` | Fixed: missing `;`, `*lhs`/`*rhs` derefs removed, `txn`→`transaction_arg`/`txn_item` in params and `TopTxn` patterns |
| `backends/rust.bv` | Fixed: `*lhs`/`*rhs`/`*condition`/`*left`/`*right` derefs removed, `txn`→`txn_item` in all variable positions, missing `;`, added `less_than_str`/`greater_than_str` helpers |
| `backends/verilog.bv` | Fixed: `txn`→`transaction_arg`/`txn_item` in params and `TopTxn` patterns. Import error (expected — no deps) |
| `backends/vhdl.bv` | Fixed: `txn`→`transaction_arg` in params and body. Import error (expected) |

### FAIL — surface-level fixes might work (3 files)

| File | Error | Root Cause | Fix |
|---|---|---|---|
| `backends/backend_aarch64.bv` | `expected ']', found 'instrs' at 993:15` | `uni` inside expression `[!uni ...]` not supported | ~20 `let mut` fixed, `reg`→`reg_name`, `AND`→`&`, but `uni` as expression is deeper structural issue |
| `backends/webstack.bv` | `expected identifier, found 'Some(Ok(RBrace))' at 127:13` | `{ name: val }` object literal in `let` binding doesn't parse correctly | Already tried removing named struct prefix (`TransactionInfo {`→`{`) and inlining — still fails |
| `backends/x86_64.bv` | `expected ')', found 'Registry' at 68:21` | `reg` is `Token::Registry` keyword. Function param and many local vars use `reg` | Rename all `reg`→`reg_name` like aarch64, fix `u8`/`as u8` Rust-isms |

### FAIL — deep Rust-isms (2 files)

| File | Error Count | Issues |
|---|---|---|
| `backends/wasm.bv` | 59+ | `u8`, `u32`, `i32`, `as` type casts, `<<` bit shift. Near-rewrite needed |
| `backends/x86_64.bv` | 32+ | `u8`, `u32`, `u64`, `as` type casts. Near-rewrite needed |

## Changes Made (Phase 1 — Parse Fixes)

### Keyword conflicts fixed
- `txn` (keyword `Token::Txn`) — renamed to `transaction_arg`, `txn_item`, `contract_txn` across all `.bv` files
- `reg` (keyword `Token::Registry`) — renamed to `reg_name` in `backend_aarch64.bv`
- `from` (keyword `Token::From`) — renamed to `from_str` in `proof_engine.bv`
- `Ok`/`Err` in `check_vc` — changed return type from `Result<(), String>` to `Bool`

### Rust-isms removed
- `*lhs`/`*rhs`/`*condition`/`*left`/`*right` dereferences — just pass the variable directly
- `let mut` → `let` (functional update, not mutation)
- `.unwrap()` → `unwrap(x)` (function, not method)
- `Some(init)` in uni patterns → `init_opt.is_some()` + `unwrap(init_opt)` two-step
- `"Vec<"` → `less_than_str()` helper (avoids `<` being parsed as comparison)
- `txn` in `TopTxn(txn)` uni pattern → `TopTxn(txn_item)` (keyword in patterns)
- Missing `;` before `}` in multi-line `term` statements

### Syntax issues worked around
- Contract-after-arrow (`-> Type [pre][post]`) → contract-before-arrow (`[pre][post] -> Type`) due to Rust parser bug where both pre/post become `Bool(true)`
- Empty guarded blocks `[cond] { };` → must contain at least one statement
- `//` comments in empty blocks still count as empty (logos skips them)

## Key Discoveries

1. **Named struct literals unsupported**: `TypeName { field: val }` is NOT valid Briv syntax. Only anonymous `{ field: val }` (parsed as `Expr::ObjectLiteral`) works.
2. **`uni` is a statement, not an expression**: Cannot use `[!uni x(Pattern) = true]` as guard condition — `uni` only works as a statement (`uni ... = ...;`).
3. **Keywords can't appear in any variable position**: Not just function parameters, but also `let` bindings and `uni` pattern variables (e.g., `TopTxn(txn)` fails because `txn` is `Token::Txn`).
4. **Backend files are deeply embedded with Rust-isms**: `u8`/`u32`/`i32`/`u64` types, `as u8` casts, `<<`/`>>` bit shifts, hex handling, block comment strings `"/* expr */"`. These are non-trivial to convert.

## Next Steps

1. Fix `x86_64.bv` — same `reg` keyword as aarch64, plus `u8`/`as` casts. Quick-ish but tedious.
2. Fix `webstack.bv` — investigate `{ }` object literal parsing bug.
3. Fix `backend_aarch64.bv` — `uni` in expression context, `u32`/`u8` types.
4. Wire `main.bv` to backends (add CLI command for self-hosted compilation).
5. Fix type errors in `parser.bv`, `typechecker.bv`, `backends/mod.bv`.
6. Complete `backends/rust.bv` (add `ExprCall`, `ExprUnaryOp`, etc.).
7. Bootstrap verification.