// ── BVIR Serializer ─────────────────────────────────────────────────────
// 2026-07-14: Walk Vec<TopLevel> + TypeUniverse → .bvir S-expression text.
// Every function is max 2 levels. Extract helpers for deeper logic.

use std::fmt::Write;
use crate::ast::*;
use crate::type_universe::{ResolvedType, TypeUniverse};
use super::sexpr::{to_string, Atom, SExpr};

/// Serialize a compiled program to BVIR S-expression text.
pub fn to_bvir(items: &[TopLevel], universe: &TypeUniverse) -> String {
    let mut exprs: Vec<SExpr> = Vec::new();
    for rt in universe.types.values() {
        exprs.push(emit_universe(rt));
    }
    for item in items {
        exprs.push(emit_toplevel(item));
    }
    to_string(&SExpr::List(exprs))
}

fn emit_universe(rt: &ResolvedType) -> SExpr {
    let mut children: Vec<SExpr> = vec![atom(&rt.name)];
    children.push(list(&[atom("bytes"), atom(&rt.bytes.to_string())]));
    children.push(list(&[atom("alignment"), atom(&rt.alignment.to_string())]));
    if !rt.properties.is_empty() {
        let mut props: Vec<SExpr> = vec![atom("properties")];
        for (k, v) in &rt.properties {
            props.push(list(&[atom(k), pv_to_sexpr(v)]));
        }
        children.push(SExpr::List(props));
    }
    SExpr::List(children)
}

fn emit_toplevel(item: &TopLevel) -> SExpr {
    match item {
        TopLevel::Definition(d) => emit_definition(d),
        TopLevel::Transaction(t) => emit_transaction(t),
        TopLevel::StateDecl(s) => emit_statedecl(s),
        TopLevel::Trigger(t) => emit_trigger(t),
        TopLevel::Constant(c) => emit_constant(c),
        TopLevel::TypeDef(t) => emit_typedef(t),
        _ => list(&[atom("toplevel"), atom(&format!("{:?}", item))]),
    }
}

fn emit_definition(d: &Definition) -> SExpr {
    let mut children: Vec<SExpr> = vec![atom("defn"), atom(&d.name), emit_params(&d.parameters)];
    children.push(emit_outputs(&d.outputs));
    // 2026-07-15: Serialize contract (including is_entry) for BVIR round-trip
    children.push(emit_contract(&d.contract));
    for (k, v) in &d.metadata {
        children.push(list(&[atom("metadata"), atom(k), pv_to_sexpr(v)]));
    }
    children.push(list(&[atom("body")]));
    for s in &d.body {
        children.push(emit_statement(s));
    }
    SExpr::List(children)
}

fn emit_transaction(t: &Transaction) -> SExpr {
    let mut children: Vec<SExpr> = vec![atom("txn"), atom(&t.name)];
    if t.is_reactive { children.push(atom(":reactive")); }
    if t.is_async { children.push(atom(":async")); }
    children.push(emit_params(&t.parameters));
    children.push(emit_contract(&t.contract));
    for (k, v) in &t.metadata {
        children.push(list(&[atom("metadata"), atom(k), pv_to_sexpr(v)]));
    }
    children.push(list(&[atom("body")]));
    for s in &t.body {
        children.push(emit_statement(s));
    }
    SExpr::List(children)
}

fn emit_contract(c: &Contract) -> SExpr {
    // 2026-07-15: (entry) preserves is_entry through BVIR round-trip
    let mut children = vec![atom("contract")];
    if c.is_entry {
        children.push(atom("entry"));
    }
    children.push(list(&[atom("pre"), emit_expr(&c.pre_condition)]));
    children.push(list(&[atom("post"), emit_expr(&c.post_condition)]));
    SExpr::List(children)
}

fn emit_statedecl(s: &StateDecl) -> SExpr {
    let mut children: Vec<SExpr> = vec![atom("state"), atom(&s.name), emit_type(&s.ty)];
    SExpr::List(children)
}

fn emit_trigger(t: &Trigger) -> SExpr {
    let mut children: Vec<SExpr> = vec![atom("trigger"), atom(&t.name)];
    children.push(list(&[atom("port"), atom(&t.port)]));
    SExpr::List(children)
}

fn emit_constant(c: &Constant) -> SExpr {
    list(&[atom("constant"), atom(&c.name), emit_type(&c.ty), emit_expr(&c.expr)])
}

fn emit_typedef(t: &TypeDef) -> SExpr {
    let mut children: Vec<SExpr> = vec![atom("typedef"), atom(&t.name)];
    children.push(list(&[atom("base"), emit_expr(&t.base)]));
    let mut slots: Vec<SExpr> = vec![atom("slots")];
    for slot in &t.body.slots {
        slots.push(list(&[atom("slot"), atom(&slot.name), emit_type(&slot.ty)]));
    }
    children.push(SExpr::List(slots));
    for (k, v) in &t.body.metadata {
        children.push(list(&[atom("metadata"), atom(k), pv_to_sexpr(v)]));
    }
    SExpr::List(children)
}

fn emit_statement(s: &Statement) -> SExpr {
    match s {
        Statement::Assign(lhs, rhs) => {
            list(&[atom("assign"), emit_expr(lhs), emit_expr(rhs)])
        }
        Statement::Let { name, expr, .. } => {
            let mut children = vec![atom("let"), atom(name)];
            if let Some(e) = expr {
                children.push(emit_expr(e));
            }
            SExpr::List(children)
        }
        Statement::Term(e) => {
            match e {
                Some(e) => list(&[atom("term"), emit_expr(e)]),
                None => list(&[atom("term")]),
            }
        }
        Statement::TermBang(e) => {
            match e {
                Some(e) => list(&[atom("term!"), emit_expr(e)]),
                None => list(&[atom("term!")]),
            }
        }
        Statement::Return(e) => {
            match e {
                Some(e) => list(&[atom("return"), emit_expr(e)]),
                None => list(&[atom("return")]),
            }
        }
        Statement::Expression(e) => list(&[atom("expr"), emit_expr(e)]),
        Statement::Guarded(cond, body) => {
            let mut children = vec![atom("guarded"), emit_expr(cond)];
            children.push(list(&[atom("body")]));
            for s in body { children.push(emit_statement(s)); }
            SExpr::List(children)
        }
        Statement::If(cond, then, els) => {
            let mut children = vec![atom("if"), emit_expr(cond)];
            children.push(list(&[atom("then")]));
            for s in then { children.push(emit_statement(s)); }
            if !els.is_empty() {
                children.push(list(&[atom("else")]));
                for s in els { children.push(emit_statement(s)); }
            }
            SExpr::List(children)
        }
        Statement::Block(body) => {
            let mut children = vec![atom("block")];
            for s in body { children.push(emit_statement(s)); }
            SExpr::List(children)
        }
        Statement::MetadataAssignment(key, val) => {
            list(&[atom("metadata"), atom(key), pv_to_sexpr(val)])
        }
        _ => list(&[atom("statement"), atom(&format!("{:?}", s))]),
    }
}

fn emit_expr(e: &Expr) -> SExpr {
    match e {
        Expr::Decimal(n) => SExpr::Atom(Atom::Int(*n)),
        Expr::Float(f) => SExpr::Atom(Atom::Float(*f)),
        Expr::Bool(b) => SExpr::Atom(Atom::Bool(*b)),
        Expr::Quoted(bytes) => {
            let s = String::from_utf8_lossy(bytes).to_string();
            list(&[atom("string"), atom(&s)])
        }
        Expr::Identifier(name) => list(&[atom("ident"), atom(name)]),
        Expr::Call(name, args) => {
            let mut children = vec![atom("call"), atom(name)];
            for a in args { children.push(emit_expr(a)); }
            SExpr::List(children)
        }
        Expr::BinaryOp(kind, l, r) => {
            list(&[atom("binop"), atom(&format!("{:?}", kind)), emit_expr(l), emit_expr(r)])
        }
        Expr::UnaryOp(kind, inner) => {
            list(&[atom("unop"), atom(&format!("{:?}", kind)), emit_expr(inner)])
        }
        Expr::Field(obj, name) => {
            list(&[atom("field"), emit_expr(obj), atom(name)])
        }
        Expr::Index(obj, idx) => {
            list(&[atom("index"), emit_expr(obj), emit_expr(idx)])
        }
        Expr::Tuple(items) => {
            let mut children = vec![atom("tuple")];
            for item in items { children.push(emit_expr(item)); }
            SExpr::List(children)
        }
        Expr::List(items) => {
            let mut children = vec![atom("list")];
            for item in items { children.push(emit_expr(item)); }
            SExpr::List(children)
        }
        Expr::Cast(expr, ty) => {
            list(&[atom("cast"), emit_expr(expr), emit_type(ty)])
        }
        Expr::Deref(inner) => {
            list(&[atom("deref"), emit_expr(inner)])
        }
        Expr::AddrOf(inner) => {
            list(&[atom("addrof"), emit_expr(inner)])
        }
        Expr::If(cond, t, f) => {
            let mut children = vec![atom("if"), emit_expr(cond), emit_expr(t)];
            if let Some(fe) = f { children.push(emit_expr(fe)); }
            SExpr::List(children)
        }
        _ => atom(&format!("{:?}", e)),
    }
}

fn emit_type(ty: &Type) -> SExpr {
    atom(&format!("{}", ty))
}

fn emit_params(params: &[(String, Type)]) -> SExpr {
    let mut children = vec![atom("params")];
    for (name, ty) in params {
        children.push(list(&[atom("param"), atom(name), emit_type(ty)]));
    }
    SExpr::List(children)
}

fn emit_outputs(outputs: &[Type]) -> SExpr {
    let mut children = vec![atom("outputs")];
    for ty in outputs {
        children.push(emit_type(ty));
    }
    SExpr::List(children)
}

fn pv_to_sexpr(pv: &PropertyValue) -> SExpr {
    match pv {
        PropertyValue::Identifier(s) => atom(s),
        PropertyValue::String(s) => atom(s),
        PropertyValue::Int(n) => SExpr::Atom(Atom::Int(*n)),
        PropertyValue::Float(f) => SExpr::Atom(Atom::Float(*f)),
        PropertyValue::Bool(b) => SExpr::Atom(Atom::Bool(*b)),
        PropertyValue::List(items) => {
            let mut children = vec![atom("list")];
            for item in items { children.push(pv_to_sexpr(item)); }
            SExpr::List(children)
        }
    }
}

fn atom(s: &str) -> SExpr {
    SExpr::Atom(Atom::String(s.to_string()))
}

fn list(items: &[SExpr]) -> SExpr {
    SExpr::List(items.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvir::from_bvir;

    #[test]
    fn test_roundtrip_simple() {
        let items = vec![
            TopLevel::StateDecl(StateDecl {
                name: "counter".into(), ty: Type::int(), span: None,
            }),
        ];
        let universe = TypeUniverse::new();
        let ir = to_bvir(&items, &universe);
        let (restored, _) = from_bvir(&ir).unwrap();
        assert_eq!(items.len(), restored.len());
        match (&items[0], &restored[0]) {
            (TopLevel::StateDecl(a), TopLevel::StateDecl(b)) => {
                assert_eq!(a.name, b.name);
            }
            _ => panic!("expected StateDecl"),
        }
    }

    #[test]
    fn test_serialize_deref() {
        let expr = Expr::Deref(Box::new(Expr::Identifier("ptr".into())));
        let sexpr = emit_expr(&expr);
        let s = to_string(&sexpr);
        assert!(s.contains("deref"));
        assert!(s.contains("ptr"));
    }

    #[test]
    fn test_contract_entry_roundtrip() {
        // 2026-07-15: Verify is_entry survives BVIR serialize/deserialize
        let entry_contract = Contract {
            pre_condition: Expr::Bool(true),
            post_condition: Expr::Bool(true),
            is_entry: true,
            watchdog: None,
            span: None,
        };
        let items = vec![
            TopLevel::Definition(Definition {
                name: "main".into(),
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![Type::int()],
                contract: entry_contract,
                body: vec![],
                metadata: std::collections::HashMap::new(),
                derivation: None,
                modifiers: vec![],
                annotations: vec![],
                span: None,
            }),
        ];
        let universe = TypeUniverse::new();
        let ir = to_bvir(&items, &universe);
        let (restored, _) = from_bvir(&ir).unwrap();
        assert_eq!(items.len(), restored.len());
        match &restored[0] {
            TopLevel::Definition(d) => {
                assert!(d.contract.is_entry, "is_entry must survive round-trip");
            }
            _ => panic!("expected Definition"),
        }
    }

    #[test]
    fn test_roundtrip_deref() {
        let expr = Expr::Deref(Box::new(Expr::Identifier("x".into())));
        let sexpr = emit_expr(&expr);
        let s = to_string(&sexpr);
        let tokens = crate::bvir::sexpr::tokenize(&s).unwrap();
        let parsed = crate::bvir::sexpr::parse(&tokens).unwrap();
        let restored = crate::bvir::deserialize::parse_expr(&parsed).unwrap();
        assert_eq!(expr, restored);
    }
}
