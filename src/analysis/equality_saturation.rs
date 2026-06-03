use crate::ast::Expr;

pub fn has_candidates(body: &[crate::ast::Statement]) -> bool {
    for stmt in body {
        if let crate::ast::Statement::Assignment { expr, .. } = stmt {
            if expr_has_candidates(expr) {
                return true;
            }
        }
        if let crate::ast::Statement::Guarded { condition, statements, .. } = stmt {
            if expr_has_candidates(condition) {
                return true;
            }
            for s in statements {
                if let crate::ast::Statement::Assignment { expr, .. } = s {
                    if expr_has_candidates(expr) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn expr_has_candidates(expr: &Expr) -> bool {
    match expr {
        Expr::Add(l, r) | Expr::Sub(l, r) => {
            is_zero(l) || is_zero(r) || expr_has_candidates(l) || expr_has_candidates(r)
        }
        Expr::Mul(l, r) | Expr::Div(l, r) => {
            is_zero(l) || is_zero(r) || is_one(l) || is_one(r) || expr_has_candidates(l) || expr_has_candidates(r)
        }
        Expr::Not(inner) => {
            matches!(inner.as_ref(), Expr::Not(_)) || expr_has_candidates(inner)
        }
        Expr::Neg(inner) => {
            matches!(inner.as_ref(), Expr::Neg(_)) || expr_has_candidates(inner)
        }
        Expr::And(l, r) | Expr::Or(l, r) => {
            is_bool_true(l) || is_bool_false(r) || expr_has_candidates(l) || expr_has_candidates(r)
        }
        Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r) => {
            expr_has_candidates(l) || expr_has_candidates(r)
        }
        Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r) |
        Expr::Shl(l, r) | Expr::Shr(l, r) => {
            is_zero(l) || is_zero(r) || expr_has_candidates(l) || expr_has_candidates(r)
        }
        _ => false,
    }
}

fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Integer(0)) || matches!(expr, Expr::Float(f) if *f == 0.0)
}

fn is_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Integer(1)) || matches!(expr, Expr::Float(f) if *f == 1.0)
}

fn is_bool_true(expr: &Expr) -> bool {
    matches!(expr, Expr::Bool(true))
}

fn is_bool_false(expr: &Expr) -> bool {
    matches!(expr, Expr::Bool(false))
}

fn exprs_equal(a: &Expr, b: &Expr) -> bool {
    format!("{:?}", a) == format!("{:?}", b)
}

pub fn simplify(expr: &Expr) -> Expr {
    let mut current = simplify_pass(expr);

    for _ in 0..5 {
        let next = simplify_pass(&current);
        let same = format!("{:?}", current) == format!("{:?}", next);
        if same {
            break;
        }
        current = next;
    }
    current
}

fn simplify_pass(expr: &Expr) -> Expr {
    match expr {
        Expr::Add(l, r) if is_zero(r) => simplify(l),
        Expr::Add(l, r) if is_zero(l) => simplify(r),

        Expr::Sub(l, r) if is_zero(r) => simplify(l),

        Expr::Mul(l, r) if is_one(r) => simplify(l),
        Expr::Mul(l, r) if is_one(l) => simplify(r),
        Expr::Mul(l, _) if is_zero(l) => Expr::Integer(0),
        Expr::Mul(_, r) if is_zero(r) => Expr::Integer(0),

        Expr::Div(l, r) if is_one(r) => simplify(l),

        Expr::Sub(l, r) if exprs_equal(l, r) => Expr::Integer(0),

        Expr::Not(inner) => {
            if let Expr::Not(i) = inner.as_ref() {
                simplify(i)
            } else {
                Expr::Not(Box::new(simplify(inner)))
            }
        }

        Expr::Neg(inner) => {
            if let Expr::Neg(i) = inner.as_ref() {
                simplify(i)
            } else {
                Expr::Neg(Box::new(simplify(inner)))
            }
        }

        Expr::And(l, r) if is_bool_true(r) => simplify(l),
        Expr::And(l, r) if is_bool_true(l) => simplify(r),
        Expr::And(l, r) if exprs_equal(l, r) => simplify(l),

        Expr::Or(l, r) if is_bool_false(r) => simplify(l),
        Expr::Or(l, r) if is_bool_false(l) => simplify(r),
        Expr::Or(l, r) if exprs_equal(l, r) => simplify(l),

        Expr::BitAnd(l, _) if is_zero(l) => Expr::Integer(0),
        Expr::BitAnd(_, r) if is_zero(r) => Expr::Integer(0),

        Expr::BitOr(l, r) if is_zero(r) => simplify(l),
        Expr::BitOr(l, r) if is_zero(l) => simplify(r),

        Expr::BitXor(l, r) if is_zero(r) => simplify(l),
        Expr::BitXor(l, r) if is_zero(l) => simplify(r),

        Expr::Shl(l, r) if is_zero(r) => simplify(l),
        Expr::Shr(l, r) if is_zero(r) => simplify(l),

        Expr::Sub(l, r) => {
            if let Expr::Add(a, b) = l.as_ref() {
                if exprs_equal(b, r) {
                    return simplify(a);
                }
                if exprs_equal(a, r) {
                    return simplify(b);
                }
            }
            Expr::Sub(Box::new(simplify(l)), Box::new(simplify(r)))
        }

        Expr::Add(l, r) => {
            if let Expr::Sub(a, b) = l.as_ref() {
                if exprs_equal(b, r) {
                    return simplify(a);
                }
            }
            Expr::Add(Box::new(simplify(l)), Box::new(simplify(r)))
        }

        Expr::Mul(l, r) => Expr::Mul(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Div(l, r) => Expr::Div(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::And(l, r) => Expr::And(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Or(l, r) => Expr::Or(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Eq(l, r) => Expr::Eq(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Ne(l, r) => Expr::Ne(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Lt(l, r) => Expr::Lt(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Le(l, r) => Expr::Le(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Gt(l, r) => Expr::Gt(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Ge(l, r) => Expr::Ge(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::BitAnd(l, r) => Expr::BitAnd(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::BitOr(l, r) => Expr::BitOr(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::BitXor(l, r) => Expr::BitXor(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Shl(l, r) => Expr::Shl(Box::new(simplify(l)), Box::new(simplify(r))),
        Expr::Shr(l, r) => Expr::Shr(Box::new(simplify(l)), Box::new(simplify(r))),

        _ => expr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> Expr {
        Expr::Identifier(name.to_string())
    }

    #[test]
    fn test_cancel_add_sub() {
        let expr = Expr::Sub(
            Box::new(Expr::Add(Box::new(id("a")), Box::new(Expr::Integer(5)))),
            Box::new(Expr::Integer(5)),
        );
        let result = simplify(&expr);
        assert_eq!(result, id("a"), "expected a, got {:?}", result);
    }

    #[test]
    fn test_identity_add_zero() {
        let expr = Expr::Add(Box::new(id("x")), Box::new(Expr::Integer(0)));
        let result = simplify(&expr);
        assert_eq!(result, id("x"));
    }

    #[test]
    fn test_identity_mul_one() {
        let expr = Expr::Mul(Box::new(id("x")), Box::new(Expr::Integer(1)));
        let result = simplify(&expr);
        assert_eq!(result, id("x"));
    }

    #[test]
    fn test_mul_zero() {
        let expr = Expr::Mul(Box::new(id("x")), Box::new(Expr::Integer(0)));
        let result = simplify(&expr);
        assert_eq!(result, Expr::Integer(0));
    }

    #[test]
    fn test_double_neg() {
        let expr = Expr::Not(Box::new(Expr::Not(Box::new(id("x")))));
        let result = simplify(&expr);
        assert_eq!(result, id("x"));
    }

    #[test]
    fn test_no_candidates_skips() {
        let expr = Expr::Add(Box::new(id("a")), Box::new(id("b")));
        let result = simplify(&expr);
        assert_eq!(result, Expr::Add(Box::new(id("a")), Box::new(id("b"))));
    }
}
