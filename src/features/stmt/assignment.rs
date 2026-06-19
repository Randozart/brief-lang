// ── Statement::Assignment — Assignment Statement ──────────────────
//
// Phase 2: Pattern B feature struct + 5 trait stubs.
// DEFERRED: Actual dispatch migration to Phase 4.

use crate::ast::{Expr, Hashtag, Statement, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::parser::Parser;
use crate::typechecker::TypeChecker;

pub struct AssignmentStmt {
    pub lhs: Expr,
    pub expr: Expr,
    pub timeout: Option<(Expr, crate::ast::TimeUnit)>,
    pub modifiers: Vec<Hashtag>,
}

impl StmtTypecheck for AssignmentStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> {
        Ok(())
    }
}

impl StmtEval for AssignmentStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("AssignmentStmt not yet dispatched".into()))
    }
}

impl StmtCodegenLLVM for AssignmentStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}


impl StmtCodegenWebstack for AssignmentStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assignment_stmt_construct() {
        let stmt = AssignmentStmt {
            lhs: Expr::Identifier("x".into()),
            expr: Expr::Integer(42),
            timeout: None,
            modifiers: vec![],
        };
        assert!(matches!(stmt.lhs, Expr::Identifier(_)));
        assert_eq!(stmt.expr, Expr::Integer(42));
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_assignment_construct() {
        let stmt = AssignmentStmt {
            lhs: Expr::Identifier("x".into()),
            expr: Expr::Integer(42),
            timeout: None,
            modifiers: vec![],
        };
        assert!(matches!(stmt.lhs, Expr::Identifier(_)));
    }
}
