use crate::ast::Expr;
use std::collections::HashMap;

// ── Helpers ────────────────────────────────────────────────────────

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

// ── Structural hash with boost hash_combine ────────────────────────
// No external dependencies — pure integer arithmetic.
// Collisions are benign (skip a rewrite, never wrong output).

const HASH_INIT: u64 = 0xcbf29ce484222325; // FNV offset basis

fn combine(h: u64, v: u64) -> u64 {
    h ^ (v.wrapping_add(0x9e3779b97f4a7c15)).wrapping_add(h.wrapping_shl(6)).wrapping_add(h.wrapping_shr(2))
}

fn hash_u64(h: u64, v: u64) -> u64 { combine(h, v) }
fn hash_str(h: u64, s: &str) -> u64 {
    let mut h = h;
    for b in s.bytes() {
        h = combine(h, b as u64);
    }
    h
}

fn structural_hash(expr: &Expr) -> u64 {
    let mut h = HASH_INIT;
    structural_hash_into(expr, &mut h);
    h
}

fn structural_hash_into(expr: &Expr, h: &mut u64) {
    // 2026-06-27: Normalize new-style BinaryOp/UnaryOp to old variants
    // so hash is consistent regardless of expression representation.
    if let Some(norm) = expr.normalize_to_old() {
        return structural_hash_into(&norm, h);
    }
    match expr {
        // ── Leaf variants ───────────────────────────────────────
        Expr::Integer(n) => { *h = combine(*h, 1); *h = combine(*h, *n as u64); }
        Expr::Float(f) => { *h = combine(*h, 2); *h = combine(*h, f.to_bits()); }
        Expr::Bool(b) => { *h = combine(*h, 3); *h = combine(*h, if *b { 1 } else { 0 }); }
        Expr::Char(c) => { *h = combine(*h, 4); *h = combine(*h, *c as u64); }
        Expr::String(s) => { *h = combine(*h, 5); *h = hash_str(*h, s); }
        Expr::Identifier(id) => { *h = combine(*h, 6); *h = hash_str(*h, id); }
        Expr::Term => { *h = combine(*h, 7); }
        Expr::OwnedRef(id) => { *h = combine(*h, 8); *h = hash_str(*h, id); }
        Expr::PriorState(id) => { *h = combine(*h, 9); *h = hash_str(*h, id); }
        Expr::Ellipsis => { *h = combine(*h, 10); }
        Expr::TypeRef(t) => { *h = combine(*h, 11); *h = hash_str(*h, t); }

        // ── Binary ops ──────────────────────────────────────────
        Expr::Add(l, r) => { *h = combine(*h, 20); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Sub(l, r) => { *h = combine(*h, 21); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Mul(l, r) => { *h = combine(*h, 22); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Div(l, r) => { *h = combine(*h, 23); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Mod(l, r) => { *h = combine(*h, 24); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::And(l, r) => { *h = combine(*h, 25); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Or(l, r) => { *h = combine(*h, 26); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Eq(l, r) => { *h = combine(*h, 27); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Ne(l, r) => { *h = combine(*h, 28); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Lt(l, r) => { *h = combine(*h, 29); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Le(l, r) => { *h = combine(*h, 30); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Gt(l, r) => { *h = combine(*h, 31); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Ge(l, r) => { *h = combine(*h, 32); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::BitAnd(l, r) => { *h = combine(*h, 33); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::BitOr(l, r) => { *h = combine(*h, 34); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::BitXor(l, r) => { *h = combine(*h, 35); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Shl(l, r) => { *h = combine(*h, 36); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Shr(l, r) => { *h = combine(*h, 37); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::Concat(l, r) => { *h = combine(*h, 38); structural_hash_into(l, h); structural_hash_into(r, h); }
        Expr::ListIndex(l, r) => { *h = combine(*h, 39); structural_hash_into(l, h); structural_hash_into(r, h); }

        // ── Unary ops ───────────────────────────────────────────
        Expr::Not(inner) => { *h = combine(*h, 40); structural_hash_into(inner, h); }
        Expr::Neg(inner) => { *h = combine(*h, 41); structural_hash_into(inner, h); }
        Expr::BitNot(inner) => { *h = combine(*h, 42); structural_hash_into(inner, h); }

        // ── Variadic ────────────────────────────────────────────
        Expr::Call(name, args) => {
            *h = combine(*h, 50); *h = hash_str(*h, name);
            for a in args { structural_hash_into(a, h); }
        }
        Expr::IntrinsicCall { intrinsic, args } => {
            *h = combine(*h, 51); *h = hash_str(*h, &format!("{:?}", intrinsic));
            for a in args { structural_hash_into(a, h); }
        }
        Expr::ListLiteral(items) => {
            *h = combine(*h, 52);
            for a in items { structural_hash_into(a, h); }
        }
        Expr::Tuple(items) => {
            *h = combine(*h, 53);
            for a in items { structural_hash_into(a, h); }
        }
        Expr::MapLiteral(items) => {
            *h = combine(*h, 54);
            for (k, v) in items { structural_hash_into(k, h); structural_hash_into(v, h); }
        }
        Expr::SetLiteral(items) => {
            *h = combine(*h, 55);
            for a in items { structural_hash_into(a, h); }
        }
        Expr::StructInstance(name, fields) => {
            *h = combine(*h, 56); *h = hash_str(*h, name);
            for (fn_, fv) in fields { *h = hash_str(*h, fn_); structural_hash_into(fv, h); }
        }
        Expr::ObjectLiteral(fields) => {
            *h = combine(*h, 57);
            for (fn_, fv) in fields { *h = hash_str(*h, fn_); structural_hash_into(fv, h); }
        }
        Expr::FieldAccess(obj, field) => {
            *h = combine(*h, 58); structural_hash_into(obj, h); *h = hash_str(*h, field);
        }
        Expr::Cast(expr, ty) => {
            *h = combine(*h, 59); structural_hash_into(expr, h); *h = combine(*h, format!("{:?}", ty).len() as u64);
        }
        Expr::Projection { source, target } => {
            *h = combine(*h, 60); structural_hash_into(source, h); *h = hash_str(*h, &format!("{:?}", target));
        }

        // ── Complex: fall back to Debug hash ────────────────────
        _ => {
            *h = combine(*h, 255);
            for b in format!("{:?}", expr).bytes() {
                *h = combine(*h, b as u64);
            }
        }
    }
}

// ── SimplifyCache ──────────────────────────────────────────────────

pub struct SimplifyCache {
    map: HashMap<u64, Expr>,
    pub nodes_processed: u64,
    budget: u64,
}

impl SimplifyCache {
    pub fn new(budget: u64) -> Self {
        Self { map: HashMap::new(), nodes_processed: 0, budget }
    }

    pub fn has_budget(&self) -> bool {
        self.nodes_processed < self.budget
    }
}

// ── Bottom-up simplify with cache ──────────────────────────────────

/// Simplify an expression using bottom-up rewriting with a hash-cons cache.
/// Returns `None` if the budget is exceeded (caller should use the original expression).
pub fn simplify_cached(expr: &Expr, cache: &mut SimplifyCache) -> Option<Expr> {
    // 2026-06-27: Normalize new-style BinaryOp/UnaryOp to old variants
    // so simplification rules apply regardless of expression representation.
    if let Some(norm) = expr.normalize_to_old() {
        return simplify_cached(&norm, cache);
    }

    if !cache.has_budget() {
        return None;
    }

    let h = structural_hash(expr);
    if let Some(cached) = cache.map.get(&h) {
        return Some(cached.clone());
    }

    cache.nodes_processed += 1;

    let result = match expr {
        // ── Binary ops ──────────────────────────────────────────
        Expr::Add(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sr) { sl }
            else if is_zero(&sl) { sr }
            else if let Expr::Sub(a, b) = &sl {
                if exprs_equal(b, &sr) { *a.clone() }
                else { Expr::Add(Box::new(sl), Box::new(sr)) }
            } else { Expr::Add(Box::new(sl), Box::new(sr)) }
        }

        Expr::Sub(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sr) { sl }
            else if exprs_equal(&sl, &sr) { Expr::Integer(0) }
            else if let Expr::Add(a, b) = &sl {
                if exprs_equal(b, &sr) { *a.clone() }
                else if exprs_equal(a, &sr) { *b.clone() }
                else { Expr::Sub(Box::new(sl), Box::new(sr)) }
            } else { Expr::Sub(Box::new(sl), Box::new(sr)) }
        }

        Expr::Mul(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sl) || is_zero(&sr) { Expr::Integer(0) }
            else if is_one(&sr) { sl }
            else if is_one(&sl) { sr }
            else { Expr::Mul(Box::new(sl), Box::new(sr)) }
        }

        Expr::Div(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_one(&sr) { sl }
            else { Expr::Div(Box::new(sl), Box::new(sr)) }
        }

        Expr::And(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_bool_false(&sr) || is_bool_false(&sl) { Expr::Bool(false) }
            else if is_bool_true(&sr) { sl }
            else if is_bool_true(&sl) { sr }
            else if exprs_equal(&sl, &sr) { sl }
            else { Expr::And(Box::new(sl), Box::new(sr)) }
        }

        Expr::Or(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_bool_true(&sr) || is_bool_true(&sl) { Expr::Bool(true) }
            else if is_bool_false(&sr) { sl }
            else if is_bool_false(&sl) { sr }
            else if exprs_equal(&sl, &sr) { sl }
            else { Expr::Or(Box::new(sl), Box::new(sr)) }
        }

        Expr::BitAnd(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sl) || is_zero(&sr) { Expr::Integer(0) }
            else { Expr::BitAnd(Box::new(sl), Box::new(sr)) }
        }

        Expr::BitOr(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sr) { sl }
            else if is_zero(&sl) { sr }
            else { Expr::BitOr(Box::new(sl), Box::new(sr)) }
        }

        Expr::BitXor(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sr) { sl }
            else if is_zero(&sl) { sr }
            else { Expr::BitXor(Box::new(sl), Box::new(sr)) }
        }

        Expr::Shl(l, r) | Expr::Shr(l, r) => {
            let sl = simplify_cached(l, cache)?;
            let sr = simplify_cached(r, cache)?;
            if is_zero(&sr) { sl }
            else {
                let op = match expr {
                    Expr::Shl(_, _) => Expr::Shl,
                    _ => Expr::Shr,
                };
                op(Box::new(sl), Box::new(sr))
            }
        }

        // ── Unary ops ───────────────────────────────────────────
        Expr::Not(inner) => {
            let si = simplify_cached(inner, cache)?;
            if let Expr::Not(i) = &si { *i.clone() }
            else { Expr::Not(Box::new(si)) }
        }

        Expr::Neg(inner) => {
            let si = simplify_cached(inner, cache)?;
            if let Expr::Neg(i) = &si { *i.clone() }
            else { Expr::Neg(Box::new(si)) }
        }

        // ── Variadic / complex: simplify children ───────────────
        Expr::Call(name, args) => {
            let mut new_args = Vec::with_capacity(args.len());
            for a in args {
                new_args.push(simplify_cached(a, cache)?);
            }
            Expr::Call(name.clone(), new_args)
        }

        Expr::IntrinsicCall { intrinsic, args } => {
            let mut new_args = Vec::with_capacity(args.len());
            for a in args {
                new_args.push(simplify_cached(a, cache)?);
            }
            Expr::IntrinsicCall { intrinsic: intrinsic.clone(), args: new_args }
        }

        Expr::ListLiteral(items) => {
            let mut new_items = Vec::with_capacity(items.len());
            for a in items {
                new_items.push(simplify_cached(a, cache)?);
            }
            Expr::ListLiteral(new_items)
        }

        Expr::Tuple(items) => {
            let mut new_items = Vec::with_capacity(items.len());
            for a in items {
                new_items.push(simplify_cached(a, cache)?);
            }
            Expr::Tuple(new_items)
        }

        Expr::MapLiteral(items) => {
            let mut new_items = Vec::with_capacity(items.len());
            for (k, v) in items {
                let sk = simplify_cached(k, cache)?;
                let sv = simplify_cached(v, cache)?;
                new_items.push((sk, sv));
            }
            Expr::MapLiteral(new_items)
        }

        Expr::SetLiteral(items) => {
            let mut new_items = Vec::with_capacity(items.len());
            for a in items {
                new_items.push(simplify_cached(a, cache)?);
            }
            Expr::SetLiteral(new_items)
        }

        Expr::StructInstance(name, fields) => {
            let mut new_fields = Vec::with_capacity(fields.len());
            for (fn_, fv) in fields {
                new_fields.push((fn_.clone(), simplify_cached(fv, cache)?));
            }
            Expr::StructInstance(name.clone(), new_fields)
        }

        Expr::ObjectLiteral(fields) => {
            let mut new_fields = Vec::with_capacity(fields.len());
            for (fn_, fv) in fields {
                new_fields.push((fn_.clone(), simplify_cached(fv, cache)?));
            }
            Expr::ObjectLiteral(new_fields)
        }

        Expr::TupleDestructure(names, expr) => {
            let se = simplify_cached(expr, cache)?;
            Expr::TupleDestructure(names.clone(), Box::new(se))
        }

        Expr::FieldAccess(obj, field) => {
            let so = simplify_cached(obj, cache)?;
            Expr::FieldAccess(Box::new(so), field.clone())
        }

        Expr::Cast(inner, ty) => {
            let si = simplify_cached(inner, cache)?;
            Expr::Cast(Box::new(si), ty.clone())
        }

        Expr::Block(stmts, expr) => {
            Expr::Block(stmts.clone(), Box::new(simplify_cached(expr, cache)?))
        }

        Expr::Match { value, arms } => {
            let sv = simplify_cached(value, cache)?;
            let mut new_arms = Vec::with_capacity(arms.len());
            for arm in arms {
                new_arms.push(crate::ast::MatchArm {
                    pattern: arm.pattern.clone(),
                    guard: None,
                    body: Box::new(simplify_cached(&arm.body, cache)?),
                });
            }
            Expr::Match { value: Box::new(sv), arms: new_arms }
        }

        // ── Leaf: clone as-is ───────────────────────────────────
        _ => expr.clone(),
    };

    cache.map.insert(h, result.clone());
    Some(result)
}

// ── Top-level API (non-cached, creates temporary cache) ───────────

pub fn simplify(expr: &Expr) -> Expr {
    let mut cache = SimplifyCache::new(u64::MAX);
    simplify_cached(expr, &mut cache).unwrap_or_else(|| expr.clone())
}

// ── Program-level simplification ───────────────────────────────────

pub fn simplify_program(program: &mut crate::ast::Program, budget: u64) {
    let mut cache = SimplifyCache::new(budget);
    for item in &mut program.items {
        use crate::ast::TopLevel;
        match item {
            TopLevel::Definition(defn) => {
                for stmt in &mut defn.body {
                    simplify_stmt(stmt, &mut cache);
                }
            }
            TopLevel::Transaction(txn) => {
                for stmt in &mut txn.body {
                    simplify_stmt(stmt, &mut cache);
                }
            }
            TopLevel::Constant(c) => {
                // 2026-06-27: Apply simplification to constant initializer expressions.
                c.expr = simplify_cached(&c.expr, &mut cache).unwrap_or_else(|| c.expr.clone());
            }
            _ => {}
        }
        if !cache.has_budget() { break; }
    }
}

fn simplify_stmt(stmt: &mut crate::ast::Statement, cache: &mut SimplifyCache) {
    use crate::ast::Statement;
    match stmt {
        Statement::Assignment { expr, .. } => {
            if let Some(s) = simplify_cached(expr, cache) { *expr = s; }
        }
        Statement::Let { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                if let Some(s) = simplify_cached(e, cache) { *e = s; }
            }
        }
        Statement::Guarded { condition, statements, .. } => {
            if let Some(s) = simplify_cached(condition, cache) { *condition = s; }
            for st in statements { simplify_stmt(st, cache); }
        }
        Statement::Term { values, swan_song, .. } => {
            for v in values.iter_mut().flatten() {
                if let Some(s) = simplify_cached(v, cache) { *v = s; }
            }
            if let Some(swan) = swan_song.as_mut() { simplify_stmt(swan, cache); }
        }
        Statement::TermBang { swan_song, .. } => {
            if let Some(swan) = swan_song.as_mut() { simplify_stmt(swan, cache); }
        }
        Statement::Expression(e) => {
            if let Some(s) = simplify_cached(e, cache) { *e = s; }
        }
        Statement::Unification { expr, .. } => {
            if let Some(s) = simplify_cached(expr, cache) { *expr = s; }
        }
        Statement::Escape(e) => {
            if let Some(inner) = e.as_mut() {
                if let Some(s) = simplify_cached(inner, cache) { *inner = s; }
            }
        }
        Statement::LocalTrigger { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                if let Some(s) = simplify_cached(e, cache) { *e = s; }
            }
        }
        Statement::SyncBlock { body } => {
            for st in body { simplify_stmt(st, cache); }
        }
        _ => {}
    }
}

// ── Tests ──────────────────────────────────────────────────────────

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

    #[test]
    fn test_deep_or_chain_linear_time() {
        let expr = "abcdefghijklmnopqrstuvwxyz".chars().fold(
            Expr::Bool(false),
            |acc, c| Expr::Or(Box::new(acc), Box::new(id(&c.to_string()))),
        );
        let mut cache = SimplifyCache::new(10000);
        let _result = simplify_cached(&expr, &mut cache);
        assert!(
            cache.nodes_processed < 10000,
            "nodes_processed={} exceeded limit, likely O(6^n) blowup",
            cache.nodes_processed
        );
    }

    #[test]
    fn test_cache_hit() {
        let inner = Expr::Add(Box::new(id("a")), Box::new(Expr::Integer(0)));
        let outer = Expr::Add(
            Box::new(Expr::Add(Box::new(inner.clone()), Box::new(id("b")))),
            Box::new(inner),
        );
        let mut cache = SimplifyCache::new(1000);
        let result = simplify_cached(&outer, &mut cache);
        assert!(result.is_some());
        // a+0 simplified once (2 nodes for inner), plus outer nodes (add, add+b, a): ~6 max
        assert!(cache.nodes_processed <= 10, "too many nodes: {}", cache.nodes_processed);
    }

    #[test]
    fn test_add_sub_cancel() {
        let expr = Expr::Sub(
            Box::new(Expr::Add(Box::new(id("a")), Box::new(id("b")))),
            Box::new(id("a")),
        );
        let result = simplify(&expr);
        assert_eq!(result, id("b"));
    }

    #[test]
    fn test_sub_add_cancel() {
        let expr = Expr::Add(
            Box::new(Expr::Sub(Box::new(id("a")), Box::new(id("b")))),
            Box::new(id("b")),
        );
        let result = simplify(&expr);
        assert_eq!(result, id("a"));
    }

    #[test]
    fn test_double_neg_mul() {
        let expr = Expr::Neg(Box::new(Expr::Neg(Box::new(id("x")))));
        let result = simplify(&expr);
        assert_eq!(result, id("x"));
    }

    #[test]
    fn test_or_true_simplifies() {
        let expr = Expr::Or(Box::new(id("x")), Box::new(Expr::Bool(true)));
        let result = simplify(&expr);
        assert_eq!(result, Expr::Bool(true));
    }

    #[test]
    fn test_and_false_simplifies() {
        let expr = Expr::And(Box::new(id("x")), Box::new(Expr::Bool(false)));
        let result = simplify(&expr);
        assert_eq!(result, Expr::Bool(false));
    }
}
