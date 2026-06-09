<!-- 2026-06-09 -->

# Backend Strategy

## Principle

Backend codegen is extracted into feature files via per-backend traits.
Each backend is a separate trait so changing VHDL emission never
recompiles LLVM codegen.

## LLVM Backend (Pragmatic Extraction)

The LLVM backend (`src/backend/llvm.rs`, 7,799 lines) has optimizations
deeply interwoven with codegen. Strategy:

| Component | Lines | Approach |
|-----------|-------|----------|
| `emit_expr` match arms | 622 | ✅ Extract into feature `ExprCodegenLLVM` impls |
| `emit_stmt` match arms | 385 | ✅ Extract into statement feature files |
| `simplify()` call | 5 | ✅ Hoist to pre-pass before codegen |
| Peephole in `emit_binop`/`emit_fcmp` | 50 | ⚠️ Move to pre-pass or keep in helper |
| Optimization decision tree | 898 | ⚠️ Extract to `llvm_optimizer.rs` |
| Folded loop engine (4 fns) | 700 | ⛔ Keep centralized |
| SSA mode + pre-extraction | 150 | ⛔ Keep centralized |
| Parallel reactor + dispatch | 260 | ⛔ Keep centralized |
| `LlvmBackend` struct (48 fields) | 78 | ⛔ Keep as-is — context shared by features |
| Tests (86 tests) | 2,837 | ⛔ Keep intact |

Result: `llvm.rs` shrinks from 7,799 to ~3,000 lines.

## VHDL Backend

(`src/backend/vhdl.rs`, 1,261 lines) — expression emission extracted
into feature `ExprCodegenVHDL` impls. Optimizations deferred until
LLVM pattern is proven.

## Webstack Backend

(`src/backend/webstack.rs`, 2,230 lines) — expression emission extracted
into feature `ExprCodegenWebstack` impls. Optimizations deferred until
LLVM pattern is proven.
