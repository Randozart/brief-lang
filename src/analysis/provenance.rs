use crate::ast::{Expr, Statement};

/// Describes where a pointer value originates from — used to detect
/// dangling borrows and to refine parallel-txn write sets.
#[derive(Debug, Clone, PartialEq)]
pub enum Provenance {
    /// Points to a named variable or state field.
    Known(String),
    /// Points to a field of a known base: `base.field`.
    FieldAccess {
        base: Box<Provenance>,
        field: String,
    },
    /// Points to an index into a known base: `base[idx]`.
    Index {
        base: Box<Provenance>,
        index: Box<Provenance>,
    },
    /// Points to the target of a dereference: `*ptr`.
    Deref(Box<Provenance>),
    /// Provenance cannot be determined (opaque source, FFI, or complex expr).
    Unknown,
}

/// Infer the provenance of an expression. This is used alongside type
/// inference during typechecking to track where pointer values come from.
pub fn infer_provenance(expr: &Expr) -> Provenance {
    match expr {
        Expr::Identifier(name) => Provenance::Known(name.clone()),
        Expr::PriorState(name) => Provenance::Known(name.clone()),
        Expr::Field(base, field) => Provenance::FieldAccess {
            base: Box::new(infer_provenance(base)),
            field: field.clone(),
        },
        Expr::ListIndex(base, _) => Provenance::Index {
            base: Box::new(infer_provenance(base)),
            index: Box::new(Provenance::Unknown),
        },
        Expr::AddrOf(inner) => infer_provenance(inner),
        Expr::Deref(ptr) => deref_provenance(ptr),
        _ => Provenance::Unknown,
    }
}

/// Compute the provenance of a dereference expression.
/// `*p` → deref(p's provenance)
/// `*(*p)` → just p's provenance (double deref collapses)
pub fn deref_provenance(ptr: &Expr) -> Provenance {
    let inner = infer_provenance(ptr);
    match inner {
        Provenance::Unknown => Provenance::Unknown,
        Provenance::Deref(in_inner) => *in_inner,
        _ => Provenance::Deref(Box::new(inner)),
    }
}

/// Check if a provenance refers to a local (non-state) variable.
/// Used to detect dangling pointers: storing the address of a local
/// into a state field creates a dangling reference.
pub fn is_local_provenance(prov: &Provenance) -> bool {
    match prov {
        Provenance::Known(_) => {
            // All Known at this stage are considered state-addressable.
            // A refined version would check a set of local variable names.
            false
        }
        Provenance::FieldAccess { base, .. } | Provenance::Index { base, .. } => {
            is_local_provenance(base)
        }
        Provenance::Deref(_) => false,
        Provenance::Unknown => false,
    }
}

/// Extract (target, source) from a pointer-to-pointer assignment.
/// Returns `Some((lhs_target, rhs_expr))` for statements like:
/// `&state_ptr = &local_var;`  —  `AddrOf(state_ptr) = AddrOf(local_var)`
pub fn extract_ptr_assign(stmt: &Statement) -> Option<(&Expr, &Expr)> {
    if let Statement::Assignment { lhs, expr, .. } = stmt {
        if let Expr::AddrOf(lhs_target) = lhs {
            return Some((lhs_target, expr));
        }
    }
    None
}

/// Build a diagnostic warning about a dangling pointer assignment.
pub fn build_dangling_warning(target: &Expr, source: &Expr) -> String {
    let target_name = match target {
        Expr::Identifier(n) => n.as_str(),
        _ => "<expression>",
    };
    let source_name = match source {
        Expr::AddrOf(inner) => match inner.as_ref() {
            Expr::Identifier(n) => n.as_str(),
            _ => "<expression>",
        },
        Expr::Identifier(n) => n.as_str(),
        _ => "<expression>",
    };
    format!(
        "warning: storing pointer to local '{}' in state field '{}' may dangle. \
         Store the value, not the pointer, or ensure the local outlives the state reference",
        source_name, target_name
    )
}

/// Scan transaction body assignments for dangling pointer patterns.
/// Emits warnings when a pointer to a local variable is stored in a
/// state field via `&target = &source`.
pub fn check_dangling_ptrs(body: &[Statement]) -> Vec<String> {
    let mut warnings = Vec::new();
    for stmt in body {
        let Some((target, source)) = extract_ptr_assign(stmt) else { continue; };
        let source_prov = infer_provenance(source);
        if is_local_provenance(&source_prov) {
            warnings.push(build_dangling_warning(target, source));
        }
    }
    warnings
}

/// Collect variable names referenced in an expression.
fn collect_var_names(expr: &Expr) -> Vec<String> {
    let mut vars = Vec::new();
    let mut work = vec![expr];
    while let Some(e) = work.pop() {
        match e {
            Expr::Identifier(n) => vars.push(n.clone()),
            Expr::AddrOf(inner) | Expr::Deref(inner)
            | Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) | Expr::Cast(inner, _) => {
                work.push(inner);
            }
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
            | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
            | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::And(a, b)
            | Expr::Or(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
            | Expr::Shl(a, b) | Expr::Shr(a, b) | Expr::Concat(a, b) => {
                work.push(b);
                work.push(a);
            }
            Expr::Field(obj, _) | Expr::ListIndex(obj, _) => { work.push(obj); }
            Expr::Call(_, args) => { work.extend(args.iter().rev()); }
            _ => {}
        }
    }
    vars
}

/// Collect variable names written to by assignment statements in a body.
fn collect_write_vars(body: &[Statement]) -> Vec<String> {
    let mut vars = Vec::new();
    for stmt in body {
        if let Statement::Assignment { lhs, .. } = stmt {
            if let Some(name) = lhs.as_var_name() {
                vars.push(name.to_string());
            }
        }
        if let Statement::Let { name, .. } = stmt {
            vars.push(name.clone());
        }
    }
    vars
}

/// Check that every reactive transaction can converge on its own.
/// A reactive txn must modify at least one variable in its pre-condition;
/// otherwise it depends entirely on other txns to unblock it, which is
/// a convergence footgun.
///
/// Returns (severity, message) pairs. When no other txn modifies any
/// pre-condition variable and no convergng txn depends on it, the
/// txn will run forever — this is a hard error, not a warning.
pub fn check_convergence_safety(
    txn_name: &str,
    is_reactive: bool,
    pre: &Expr,
    body: &[Statement],
    all_txns: &std::collections::HashMap<String, crate::ast::Transaction>,
) -> Vec<(&'static str, String)> {
    let mut results = Vec::new();
    if !is_reactive { return results; }
    if matches!(pre, Expr::Bool(true)) { return results; }

    let pre_vars = collect_var_names(pre);
    let write_vars = collect_write_vars(body);

    let modifies_own_pre = pre_vars.iter().any(|pv| write_vars.contains(pv));
    if !modifies_own_pre {
        let pre_str = pre_vars.join(", ");
        let write_str = if write_vars.is_empty() {
            "nothing".to_string()
        } else {
            write_vars.join(", ")
        };

        // Check if ANY other txn writes to any pre-condition variable.
        let other_modifies = pre_vars.iter().any(|pv| {
            all_txns.iter().any(|(other_name, other_txn)| {
                other_name != txn_name && {
                    let other_writes = collect_write_vars(&other_txn.body);
                    other_writes.contains(pv)
                }
            })
        });

        let msg = format!(
            "reactive txn '{}' has precondition [{}] but body writes to [{}]. \
             The txn does not modify any precondition variable and depends on other \
             txns to satisfy its convergence. If no other txn changes the precondition \
             to false, this txn will run forever.",
            txn_name, pre_str, write_str
        );

        if other_modifies {
            // Another txn may eventually unblock this one — soft warning.
            results.push(("warning", msg));
        } else {
            // No txn modifies any pre-condition variable — guaranteed infinite loop.
            results.push(("error", msg));
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provenance_identifier() {
        let prov = infer_provenance(&Expr::Identifier("x".to_string()));
        assert_eq!(prov, Provenance::Known("x".to_string()));
    }

    #[test]
    fn test_provenance_addr_of_identifier() {
        let expr = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        let prov = infer_provenance(&expr);
        assert_eq!(prov, Provenance::Known("x".to_string()));
    }

    #[test]
    fn test_provenance_addr_of_field() {
        let inner = Expr::Field(
            Box::new(Expr::Identifier("obj".to_string())),
            "field".to_string(),
        );
        let expr = Expr::AddrOf(Box::new(inner));
        let prov = infer_provenance(&expr);
        assert_eq!(prov, Provenance::FieldAccess {
            base: Box::new(Provenance::Known("obj".to_string())),
            field: "field".to_string(),
        });
    }

    #[test]
    fn test_provenance_deref() {
        let ptr = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        let expr = Expr::Deref(Box::new(ptr));
        let prov = infer_provenance(&expr);
        // deref(addr_of(x)) → deref(Known("x")) → Deref(Known("x"))
        assert_eq!(prov, Provenance::Deref(Box::new(Provenance::Known("x".to_string()))));
    }

    #[test]
    fn test_provenance_double_deref() {
        // *(*p) → p's provenance (double deref collapses to Known("p"))
        // Because *p has provenance Deref(Known("p")), then *(that) is:
        // deref_provenance sees Deref(Known("p")) → unwraps to Known("p")
        let inner_ptr = Expr::AddrOf(Box::new(Expr::Identifier("p".to_string())));
        let deref_inner = Expr::Deref(Box::new(inner_ptr));
        let expr = Expr::Deref(Box::new(deref_inner));
        let prov = infer_provenance(&expr);
        assert_eq!(prov, Provenance::Known("p".to_string()));
    }

    #[test]
    fn test_extract_ptr_assign_simple() {
        let stmt = Statement::Assignment {
            lhs: Expr::AddrOf(Box::new(Expr::Identifier("dst".to_string()))),
            expr: Expr::AddrOf(Box::new(Expr::Identifier("src".to_string()))),
            timeout: None,
            modifiers: vec![],
        };
        let (target, source) = extract_ptr_assign(&stmt).unwrap();
        assert_eq!(target.as_var_name(), Some("dst"));
        assert!(matches!(source, Expr::AddrOf(inner) if inner.as_var_name() == Some("src")));
    }

    #[test]
    fn test_extract_ptr_assign_non_ptr_returns_none() {
        let stmt = Statement::Assignment {
            lhs: Expr::Identifier("x".to_string()),
            expr: Expr::Decimal(42),
            timeout: None,
            modifiers: vec![],
        };
        assert!(extract_ptr_assign(&stmt).is_none());
    }

    #[test]
    fn test_dangling_warning_message() {
        let target = Expr::Identifier("state_field".to_string());
        let source = Expr::AddrOf(Box::new(Expr::Identifier("local_var".to_string())));
        let msg = build_dangling_warning(&target, &source);
        assert!(msg.contains("local_var"));
        assert!(msg.contains("state_field"));
        assert!(msg.contains("may dangle"));
    }

    #[test]
    fn test_check_dangling_ptrs_no_warning_on_normal_assign() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Decimal(42),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let warnings = check_dangling_ptrs(&body);
        assert!(warnings.is_empty());
    }
}
