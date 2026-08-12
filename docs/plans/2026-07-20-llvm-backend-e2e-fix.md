# Plan: LLVM Backend End-to-End Fix + DRY Cleanup + Integration Tests

Date: 2026-07-20
Status: Active
Driver: @agent

## Summary

The type system has been rebuilt (hashword protocol, normalizer, protocol graph,
BFS Cast resolution). The LLVM backend compiles (0 warnings, 931/932 lib tests
pass) but has known issues:

1. One lib test fails (`test_float_binary_add` — float width without universe)
2. 7 integration test crates fail to compile (2 active, 5 dead backends)
3. 58+ hand-rolled `getelementptr %State` sites remain instead of centralized helpers
4. `phi.rs` + `function.rs` are entirely dead code (placeholder backedge logic)
5. Triplicate type resolution (`lower_type`, `TypedRegister::llvm`, `llvm_type`)
6. `<-` operator silently no-ops for non-ringbuf types with OperatorDefs
7. Baseline worktree at `../briev-compiler-baseline` (commit `8a827db1`) available

## Prerequisites

```bash
# Read this document before starting
# Baseline comparison available via:
bash benchmarks/compare_baseline.sh <benchmark_name>
```

## Step 1: Remove dead code

### 1a: Remove `phi.rs` (45 lines)

`emit_forward_phis` and `emit_backedge_phis` are defined but never called anywhere
in the codebase. The `emit_backedge_phis` function has a placeholder `add` instruction
pretending to be a phi patch — this would mislead any engineer into thinking
backedge phi patching is broken. Real phi logic is inline in `loop_engine/counter.rs`
and `loop_engine/ssa.rs`.

- Delete `src/backend/llvm/phi.rs`
- Remove `pub mod phi;` from `src/backend/llvm/mod.rs`

### 1b: Remove `function.rs` (67 lines)

`FunctionState` struct is ONLY imported by `phi.rs` (line 5). With `phi.rs` removed,
`function.rs` has no consumers.

- Delete `src/backend/llvm/function.rs`
- Remove `pub mod function;` from `src/backend/llvm/mod.rs`

### 1c: Verify nothing else depends on these

```bash
grep -rn "function::FunctionState\|phi::emit_" src/
# Should show no results after removal
cargo build  # Should succeed
cargo test --lib  # All 932 tests pass
```

## Step 2: Fix `test_float_binary_add` (the 1 failing lib test)

### Root cause

`emit_binop_from_config` in `emit_expr.rs:730` calls `template_for_op` which
computes `float_llvm` from `bytes`. Without a TypeUniverse loaded, `resolve_arg_bytes`
defaults to `bytes = 8`, producing `fadd fast double` instead of `fadd fast float`.

### Fix: Option B (cleanest)

In `template_for_op` (`intrinsics.rs:131`), determine `float_llvm` from the
`llvm_ty` string directly instead of from `bytes`:

```rust
let float_llvm = match llvm_ty {
    "float" | "half" | "bfloat" => "float",
    "double" => "double",
    _ if bytes <= 4 => "float",
    _ => "double",
};
```

This way `template_for_op` is self-contained regardless of universe availability.

### Verification

```bash
cargo test --lib -- backend::llvm::tests::test_float_binary_add  # PASS
```

## Step 3: Fix active integration tests

### 3a: Fix `tests/test_contract.rs` (convert to proper `#[test]`)

Current: `fn main()` binary with `println!` instead of assertions.

Change to proper `#[test]` functions using the new Parser API.

Steps:
1. Change `fn main()` to individual `#[test]` functions
2. Add `use briev_compiler::lexer::tokenize;`
3. Change `Parser::new(s1)` to `Parser::new(tokenize(s1).unwrap(), s1)`
4. Change `parser.parse()` to `parser.parse_program()`
5. Change `prog.items[N]` to `prog[N]`
6. Replace `println!` with `assert!` / `assert_eq!`

### 3b: Fix `tests/pointer_trickery_test.rs` (rewrite as source-based test)

This test uses old-style AST constructors (`Expr::Add`, `Expr::Integer`, `Expr::Eq`,
`Expr::Bool`, `Expr::Cast`, `Expr::IntrinsicCall`, `Intrinsic::VolatileLoad`,
`Value::Ptr`, `Value::Bool`). All of these have been removed/modernized.

**Strategy**: Rewrite to use new-style AST constructors:

| Old | New |
|-----|-----|
| `Expr::Add(l, r)` | `Expr::BinaryOp(BinaryOpKind::Add, l, r)` |
| `Expr::Integer(n)` | `Expr::Decimal(n)` |
| `Expr::Eq(l, r)` | `Expr::BinaryOp(BinaryOpKind::Eq, l, r)` |
| `Expr::Ge(l, r)` | `Expr::BinaryOp(BinaryOpKind::Ge, l, r)` |
| `Expr::Lt(l, r)` | `Expr::BinaryOp(BinaryOpKind::Lt, l, r)` |
| `Expr::BitAnd(l, r)` | `Expr::BinaryOp(BinaryOpKind::BitAnd, l, r)` |
| `Expr::BitXor(l, r)` | `Expr::BinaryOp(BinaryOpKind::BitXor, l, r)` |
| `Value::Ptr(addr)` | `Value::Int(addr)` |
| `Value::Bool(b)` | `Value::Int(if b { 1 } else { 0 })` |
| `Expr::IntrinsicCall { intrinsic: Intrinsic::VolatileLoad, args }` | `Expr::Call("Load#".into(), args)` |
| `Expr::IntrinsicCall { intrinsic: Intrinsic::VolatileStore, args }` | `Expr::Call("Store#".into(), args)` |

Also update `parse_and_init` to use new Parser API.

### 3c: Stub `tests/fuzz_backend.rs` as dead-backend test

This test uses `AArch64Backend` (dead) + `Desugarer` (removed). Per AGENTS.md
"Dead Backends — zero fixes". Replace content with placeholder.

## Step 4: Fix dead backend integration tests for shared API changes

When shared API changes (e.g., `Parser::new()` signature, `Program` removal) break
dead backend tests, use `#[allow(unused_variables)]`, `_ => {}`, or `todo!()`
with comment `// dead backend`.

Files: `tests/test_aarch64.rs`, `tests/test_verilog.rs`, `tests/test_vhdl.rs`,
`tests/test_x86_64.rs`, `tests/test_wasm.rs`

**Strategy**: The Parser API change affects ALL these files. Quickest fix:
- Add `use briev_compiler::lexer::tokenize;`
- Change `Parser::new(source)` to `Parser::new(tokenize(source).unwrap(), source)`
- Change `.parse()` to `.parse_program()`
- Change `Program` type references to `Vec<TopLevel>`

## Step 5: Clean up triplicate type resolution

`TypedRegister::llvm()` in `mod.rs` is a duplicate of `llvm_type()` with
subtle differences (`Bool → "i1"` instead of `Bool → "i8"`).

### Fix: Remove `TypedRegister::llvm()`

1. Find callers: `grep -rn "\.llvm()" src/backend/llvm/`
2. Replace `reg.llvm()` with `self.llvm_type(&reg.ty)` in each caller
3. Remove the method definition

## Step 6: DRY — Migrate hand-rolled GEP+load/store to centralized helpers

Centralized helpers exist at `helpers.rs:2031-2092`:
- `emit_state_gep()` — returns GEP register name
- `emit_state_load_i64()` — GEP + load, returns (reg, briev_type)
- `emit_state_store_i64()` — GEP + store
- `emit_state_load_i64_by_idx()` — by field index
- `emit_state_store_i64_by_idx()` — by field index

### Migration order (simplest first):

- 6a: `emit_stmt.rs` (3 sites)
- 6b: `emit_expr.rs` (2 sites)
- 6c: `mod.rs` (9 sites)
- 6d: `emit_toplevel.rs` (~14 sites)
- 6e: `helpers.rs` (6 non-DRY sites)
- 6f: `loop_engine/` (~27 sites)

For each site, replace:
```rust
// OLD pattern:
let ptr = backend.fun.gen_reg();
writeln!(out, "{}{} = getelementptr %State, ptr %state, i32 0, i32 {}", indent, ptr, idx).ok();
writeln!(out, "{}store i64 {}, ptr {}", indent, val, ptr).ok();

// NEW:
backend.emit_state_store_i64_by_idx(out, indent, idx, &val);
```

## Step 7: Fix `<-` operator for non-ringbuf types

In `emit_strategy_fn_call` (`emit_stmt.rs:293`), the function calls
`backend.ctx.ringbuf_inline.get(var_name)?` — if the target type has an
`InsertAt`/`ExtractFrom` OperatorDef but isn't a ringbuf inline type, the
`?` returns `None` and the operator silently does nothing.

**Fix**: Add fallback for non-ringbuf types. When `ringbuf_inline` doesn't
have an entry, compute the handle from the variable's value register.

## Step 8: Validate against baseline worktree

```bash
bash benchmarks/build_and_bench.sh --correctness
bash benchmarks/compare_baseline.sh nbody_sym
bash benchmarks/compare_baseline.sh memory_loop
bash benchmarks/build_and_bench.sh --runtime --optimizer
```

## Commit sequence

| # | Commit message | Verification |
|---|---------------|--------------|
| 1 | `Remove dead phi.rs and function.rs — unused placeholder backedge logic` | `cargo test --lib` |
| 2 | `Fix float op width when no TypeUniverse — use llvm_ty string not bytes` | `cargo test --lib` |
| 3 | `Convert test_contract to proper #[test] with new Parser API` | `cargo test --test test_contract` |
| 4 | `Rewrite pointer_trickery_test with new-style AST constructors` | `cargo test --test pointer_trickery_test` |
| 5 | `Fix dead backend integration tests for shared Parser API change` | `cargo test --tests` |
| 6 | `Remove duplicate TypedRegister::llvm() — use llvm_type() consistently` | `cargo test --lib` |
| 7 | `DRY emit_stmt.rs — use emit_state_store/load_i64 helpers` | `cargo test --lib` |
| 8 | `DRY emit_expr.rs — use emit_state_gep helper` | `cargo test --lib` |
| 9 | `DRY mod.rs — use emit_state_store/load_i64_by_idx helpers` | `cargo test --lib` |
| 10 | `DRY emit_toplevel.rs — use emit_state_gep helper` | `cargo test --lib` |
| 11 | `DRY helpers.rs — migrate remaining hand-rolled GEPs` | `cargo test --lib` |
| 12 | `DRY loop_engine — use emit_state_gep in SSA/counter` | `cargo test --lib` |
| 13 | `Fix <- operator fallback for non-ringbuf OperatorDefs` | `cargo test --lib` + benchmarks |
| 14 | `Verify all benchmarks correct vs baseline` | `bash benchmarks/build_and_bench.sh` |

## Pre-commit verification

```bash
cargo build                                          # no warnings
cargo test --lib                                     # all 932+ pass
cargo test --tests -- --include-ignored              # active integration tests pass
bash benchmarks/build_and_bench.sh --correctness     # all benchmarks correct
bash benchmarks/compare_baseline.sh nbody_sym        # no regression
bash benchmarks/compare_baseline.sh memory_loop      # no regression
```
