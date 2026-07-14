// 2026-07-12: Phase 0.2 — New architecture AST module.
// Replaces the old src/ast.rs with a clean split.
// Key differences from the old AST:
// - No Intrinsic enum (use #-name dispatch via execute_intrinsic())
// - No Expr::IntrinsicCall (use Expr::Call with # suffix)
// - No InopDeclaration or TopLevel::Inop
// - No "feature" types (BinaryOpExpr, CallExpr, etc.) — unified Expr variants only
// - Added Contract.is_entry for [#] entry points
// - Added TopLevel::Export for export defn
// - Added Statement::Guarded (already existed in old AST)

mod display;
mod expr;
pub mod top;
mod types;

pub use display::*;
pub use expr::*;
pub use top::*;
pub use types::*;
