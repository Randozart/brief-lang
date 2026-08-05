// ── Backend Normalizer — Shared Helpers ───────────────────────────────
// 2026-07-14: Walks the AST and attaches backend-specific annotations.
// Shared across all backend normalizers. Max 2 nesting depth.

use std::collections::HashSet;
use crate::ast::*;

/// A collected intrinsic call from the AST.
#[derive(Debug, Clone)]
pub struct IntrinsicCall {
    pub name: String,
}

/// Walk the AST and collect all Expr::Call where name ends with '#'.
pub fn collect_intrinsic_calls(items: &[TopLevel]) -> Vec<IntrinsicCall> {
    let mut calls = Vec::new();
    for item in items {
        walk_toplevel(item, &mut |e| {
            if let Expr::Call(name, _, _) = e {
                if name.ends_with('#') {
                    calls.push(IntrinsicCall { name: name.clone() });
                }
            }
        });
    }
    calls
}

/// Validate that every intrinsic call in the program is in the supported set.
pub fn validate_intrinsics(
    items: &[TopLevel],
    supported: &HashSet<String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for call in collect_intrinsic_calls(items) {
        if !supported.contains(&call.name) {
            errors.push(format!("intrinsic '{}' is not supported by this backend", call.name));
        }
    }
    errors
}

/// Walk a TopLevel item, calling `f` on every Expr encountered.
fn walk_toplevel<F>(item: &TopLevel, f: &mut F)
where F: FnMut(&Expr) {
    match item {
        TopLevel::Definition(d) => walk_statements(&d.body, f),
        TopLevel::Transaction(t) => walk_statements(&t.body, f),
        TopLevel::StateDecl(_) | TopLevel::Trigger(_) => {}
        TopLevel::Constant(c) => f(&c.expr),
        _ => {}
    }
}

/// Walk a list of Statements, calling `f` on every Expr encountered.
fn walk_statements<F>(stmts: &[Statement], f: &mut F)
where F: FnMut(&Expr) {
    for stmt in stmts {
        match stmt {
            Statement::Assign(lhs, rhs) => { f(lhs); f(rhs); }
            Statement::Expression(e) => f(e),
            Statement::Term(Some(e)) => f(e),
            Statement::ExitProgram(Some(e)) => f(e),
            Statement::Guarded(_, body) => walk_statements(body, f),
            Statement::Block(body) => walk_statements(body, f),
            Statement::If(_, then_, else_) => {
                walk_statements(then_, f);
                walk_statements(else_, f);
            }
            _ => {}
        }
    }
}
