// ── Statement::Assignment — Assignment Statement ──────────────────
//
// Phase 2/4: Pattern B feature struct with 4 trait implementations.

use crate::ast::{Expr, Hashtag};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};

pub struct AssignmentStmt {
    pub lhs: Expr,
    pub expr: Expr,
    pub timeout: Option<(Expr, crate::ast::TimeUnit)>,
    pub modifiers: Vec<Hashtag>,
}

impl StmtTypecheck for AssignmentStmt {
    fn typecheck(&self, _ctx: &mut crate::typechecker::TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> {
        // Phase 4: wire typecheck dispatch. The old inline path handles typechecking.
        Ok(())
    }
}

impl StmtEval for AssignmentStmt {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        let value = ctx.eval_expr(&self.expr)?;
        match &self.lhs {
            Expr::Identifier(name) | Expr::OwnedRef(name) => {
                ctx.state.insert(name.clone(), value);
            }
            Expr::ListIndex(list_expr, index_expr) => {
                let list_name = match list_expr.as_ref() {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    _ => return Err(RuntimeError::TypeMismatch("Expected identifier".to_string())),
                };
                let idx_val = ctx.eval_expr(index_expr)?;
                if let Value::Int(idx) = idx_val {
                    if let Some(target) = ctx.state.get_mut(&list_name) {
                        if let Value::List(items) = target {
                            if idx >= 0 && (idx as usize) < items.len() {
                                items[idx as usize] = value;
                            } else {
                                return Err(RuntimeError::TypeMismatch("Index out of bounds".to_string()));
                            }
                        }
                    }
                }
            }
            Expr::TupleDestructure(names, _) => {
                match value {
                    Value::Tuple(items) | Value::List(items) => {
                        for (i, name) in names.iter().enumerate() {
                            if i < items.len() && name != "_" {
                                ctx.state.insert(name.clone(), items[i].clone());
                            }
                        }
                    }
                    _ => return Err(RuntimeError::TypeMismatch("Cannot destructure non-tuple/non-list value".to_string())),
                }
            }
            _ => return Err(RuntimeError::TypeMismatch("Invalid LHS".to_string())),
        }
        Ok(())
    }
}

impl StmtCodegenLLVM for AssignmentStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {
        // Phase 4: wire LLVM codegen dispatch. The old inline path handles codegen.
    }
}

impl StmtCodegenWebstack for AssignmentStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {
        // Phase 4: wire webstack codegen dispatch. The old inline path handles codegen.
    }
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

    #[test]
    fn test_assignment_stmt_eval_simple() {
        let stmt = AssignmentStmt {
            lhs: Expr::Identifier("x".into()),
            expr: Expr::Integer(42),
            timeout: None,
            modifiers: vec![],
        };
        let mut ctx = Interpreter::new();
        stmt.evaluate(&mut ctx, &StmtDispatch).unwrap();
        let value = ctx.state.get("x").unwrap();
        assert_eq!(*value, Value::Int(42));
    }

    #[test]
    fn test_assignment_stmt_eval_list_index() {
        let stmt = AssignmentStmt {
            lhs: Expr::ListIndex(
                Box::new(Expr::Identifier("items".into())),
                Box::new(Expr::Integer(0)),
            ),
            expr: Expr::Integer(99),
            timeout: None,
            modifiers: vec![],
        };
        let mut ctx = Interpreter::new();
        ctx.state.insert("items".into(), Value::List(vec![Value::Int(1), Value::Int(2)]));
        stmt.evaluate(&mut ctx, &StmtDispatch).unwrap();
        let items = ctx.state.get("items").unwrap();
        if let Value::List(list) = items {
            assert_eq!(list[0], Value::Int(99));
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_assignment_stmt_eval_tuple_destructure() {
        let stmt = AssignmentStmt {
            lhs: Expr::TupleDestructure(vec!["a".into(), "b".into()], Box::new(Expr::Integer(0))),
            expr: Expr::Tuple(vec![Expr::Integer(10), Expr::Integer(20)]),
            timeout: None,
            modifiers: vec![],
        };
        let mut ctx = Interpreter::new();
        stmt.evaluate(&mut ctx, &StmtDispatch).unwrap();
        assert_eq!(*ctx.state.get("a").unwrap(), Value::Int(10));
        assert_eq!(*ctx.state.get("b").unwrap(), Value::Int(20));
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
