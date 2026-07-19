// ── Type Checker ───────────────────────────────────────────────────────
// 2026-07-12: Phase 2.4 — Expression/statement type inference.
// The main type-checking pass: infers types for all expressions and
// validates that operations are well-typed.
//
// Key responsibilities:
// - Expr::Call with # suffix → look up get_intrinsic_signature()
// - BinaryOp/UnaryOp → resolve op via get_operator_intrinsic()
// - Literals → validate formatting compatibility
// - Variables → look up in scope

mod validate;
pub use validate::*;

use crate::analysis::provenance::Provenance;
use crate::ast::*;
use crate::errors::{SyntaxError, TypeError};
use crate::intrinsic_signatures::{get_intrinsic_signature, ReturnKind, Signature};
use crate::type_universe::{get_operator_intrinsic, TypeUniverse};
use std::collections::HashMap;

/// Type-check context: variable bindings and type universe.
pub struct TypecheckContext<'a> {
    pub bindings: HashMap<String, Type>,
    /// 2026-07-18: Names of state-level bindings (top-level let, state fields,
    /// txn parameters). Used by is_mutable_location to distinguish mutable
    /// state fields from immutable let-bindings for PtrConst inference.
    pub state_keys: std::collections::HashSet<String>,
    pub universe: &'a TypeUniverse,
}

impl<'a> TypecheckContext<'a> {
    pub fn new(universe: &'a TypeUniverse) -> Self {
        TypecheckContext {
            bindings: HashMap::new(),
            state_keys: std::collections::HashSet::new(),
            universe,
        }
    }

    /// 2026-07-18: Check if a variable name refers to a mutable state location.
    /// Used by AddrOf to decide Ptr<T> (mutable) vs Ptr<const T> (immutable).
    pub fn is_mutable_location(&self, name: &str) -> bool {
        self.state_keys.contains(name)
    }
}

/// Infer the type of an expression in the given context.
/// Infer the type of an expression. Returns both the type and provenance.
/// 2026-07-18: Phase 2 — Thread Provenance through type inference for
/// pointer tracking and dangling-borrow detection.
pub fn infer_expression(expr: &Expr, ctx: &mut TypecheckContext) -> Result<(Type, Provenance), TypeError> {
    match expr {
        // ── Literals ────────────────────────────────────────────
        Expr::Decimal(_) => Ok((Type::int(), Provenance::Unknown)),
        Expr::Float(_) => Ok((Type::float(), Provenance::Unknown)),
        Expr::Bool(_) => Ok((Type::bool_(), Provenance::Unknown)),
        Expr::Quoted(_) => Ok((Type::string(), Provenance::Unknown)),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => {
            let ty = ctx.bindings
                .get(name)
                .cloned()
                .ok_or_else(|| TypeError::UndefinedVariable {
                    name: name.clone(),
                    available: ctx.bindings.keys().cloned().collect(),
                })?;
            Ok((ty, Provenance::Known(name.clone())))
        }

        // ── Calls ───────────────────────────────────────────────
        Expr::Call(name, args, _) => {
            infer_call(name, args, ctx).map(|ty| (ty, Provenance::Unknown))
        }

        // ── Binary operators ─────────────────────────────────────
        Expr::BinaryOp(kind, lhs, rhs) => {
            infer_binary_op(kind, lhs, rhs, ctx).map(|ty| (ty, Provenance::Unknown))
        }

        // ── Unary operators ──────────────────────────────────────
        Expr::UnaryOp(kind, expr) => {
            infer_unary_op(kind, expr, ctx).map(|ty| (ty, Provenance::Unknown))
        }

        // ── Other expressions ────────────────────────────────────
        Expr::Block(stmts) => {
            for stmt in stmts {
                infer_statement(stmt, ctx)?;
            }
            Ok((Type::void(), Provenance::Unknown))
        }
        Expr::If(cond, then, else_) => {
            infer_if(cond, then, else_, ctx).map(|ty| (ty, Provenance::Unknown))
        }
        Expr::Tuple(elems) => {
            let types: Result<Vec<Type>, _> =
                elems.iter().map(|e| infer_expression(e, ctx).map(|(t, _)| t)).collect();
            Ok((Type::Tuple(types?), Provenance::Unknown))
        }
        Expr::List(elems) => {
            if let Some(first) = elems.first() {
                let (elem_ty, _) = infer_expression(first, ctx)?;
                Ok((Type::Applied("List".into(), vec![elem_ty]), Provenance::Unknown))
            } else {
                Ok((Type::Applied("List".into(), vec![Type::int()]), Provenance::Unknown))
            }
        }
        Expr::Lambda(params, body) => {
            for param in params {
                ctx.bindings.insert(param.clone(), Type::int());
            }
            let (ret_ty, _) = infer_expression(body, ctx)?;
            Ok((Type::Function(
                params.iter().map(|_| Type::int()).collect(),
                Box::new(ret_ty),
            ), Provenance::Unknown))
        }
        Expr::Field(obj, name) => {
            let (obj_ty, obj_prov) = infer_expression(obj, ctx)?;
            Ok((obj_ty, Provenance::FieldAccess {
                base: Box::new(obj_prov),
                field: name.clone(),
            }))
        }
        Expr::Index(obj, index) => {
            let (_, obj_prov) = infer_expression(obj, ctx)?;
            let (_, idx_prov) = infer_expression(index, ctx)?;
            Ok((Type::int(), Provenance::Index {
                base: Box::new(obj_prov),
                index: Box::new(idx_prov),
            }))
        }
        Expr::Cast(expr, target_ty) => {
            let (_, prov) = infer_expression(expr, ctx)?;
            Ok((target_ty.clone(), prov))
        }
        Expr::IsType(expr, _ty) => {
            let (_, prov) = infer_expression(expr, ctx)?;
            Ok((Type::bool_(), prov))
        }
        Expr::Within(expr, scope) => {
            let (_, _) = infer_expression(scope, ctx)?;
            infer_expression(expr, ctx)
        }
        Expr::DerivationBlock(_) => Ok((Type::void(), Provenance::Unknown)),
        // 2026-07-17: Address-of: &expr returns a Ptr to the inner type.
        // Provenance carries through so the backend can distinguish
        // mutable state borrows from immutable local borrows.
        // 2026-07-18: Const inference — &local_var returns Ptr<const T>
        // (read-only), while &state_field returns Ptr<T> (mutable).
        Expr::AddrOf(inner) => {
            let (inner_ty, inner_prov) = infer_expression(inner, ctx)?;
            let ptr_ty = if let Expr::Identifier(name) = inner.as_ref() {
                if ctx.is_mutable_location(name) {
                    Type::ptr(inner_ty)
                } else {
                    Type::ptr_const(inner_ty)
                }
            } else {
                // 2026-07-18: Compound expressions (Field, Index) on state
                // are still mutable — fall back to Ptr<T>.
                Type::ptr(inner_ty)
            };
            Ok((ptr_ty, inner_prov))
        }
        // 2026-07-15: Dereference: *ptr returns the pointee type (strip outer Ptr).
        Expr::Deref(inner) => {
            let (inner_ty, inner_prov) = infer_expression(inner, ctx)?;
            match inner_ty {
                Type::Ptr(pointee) => Ok(((*pointee).clone(), Provenance::Deref(Box::new(inner_prov)))),
                _ => Err(TypeError::InvalidOperation {
                    operation: format!("cannot dereference non-pointer type '{}'", inner_ty),
                    type_name: inner_ty.to_string(),
                }),
            }
        }
        Expr::PropertyGet(name) => Err(TypeError::UndefinedVariable {
            name: name.clone(),
            available: vec![],
        }),
        Expr::FormattingAnnotation(_) => Ok((Type::void(), Provenance::Unknown)),
        Expr::Match(expr, arms) => {
            infer_match(expr, arms, ctx).map(|ty| (ty, Provenance::Unknown))
        }
        // 2026-07-19: Plugin-intercept calls are resolved by Front or Mid
        // stage plugins. If unresolved at Front, they pass through typecheck
        // with Void type and are resolved at Mid stage. If still unresolved
        // after Mid, codegen panics with a clear error.
        Expr::PluginIntercept { .. } => Ok((Type::void(), Provenance::Unknown)),
    }
}

/// 2026-07-18: Convenience wrapper — infer type without provenance.
pub fn infer_type_only(expr: &Expr, ctx: &mut TypecheckContext) -> Result<Type, TypeError> {
    infer_expression(expr, ctx).map(|(ty, _)| ty)
}

/// Infer the type of a function/intrinsic call.
fn infer_call(name: &str, args: &[Expr], ctx: &mut TypecheckContext) -> Result<Type, TypeError> {
    // Intrinsic call (ends with #): look up signature
    if name.ends_with('#') {
        let sig = get_intrinsic_signature(name).ok_or_else(|| {
            let found: Vec<String> = args
                .iter()
                .filter_map(|a| infer_type_only(a, ctx).ok())
                .map(|t| format!("{}", t))
                .collect();
            TypeError::InvalidOperation {
                operation: format!("call to unknown intrinsic '{}'({})", name, found.join(", ")),
                type_name: "intrinsic".into(),
            }
        })?;
        return infer_intrinsic_call(&sig, args, ctx);
    }

    // User function call
    for arg in args {
        infer_type_only(arg, ctx)?;
    }
    Ok(Type::int())
}

/// Type-check an intrinsic call against its signature.
fn infer_intrinsic_call(
    sig: &Signature,
    args: &[Expr],
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    // 2026-07-14: Empty parameters means type-inferred — skip count check
    if !sig.parameters.is_empty() && args.len() != sig.parameters.len() {
        return Err(TypeError::TypeMismatch {
            expected: format!("{} parameters", sig.parameters.len()),
            found: format!("{} arguments", args.len()),
            context: format!("call to '{}'", sig.name),
        });
    }
    for (i, (_, param_ty)) in sig.parameters.iter().enumerate() {
        let arg_ty = infer_type_only(&args[i], ctx)?;
        // Simplified type compatibility check
        if format!("{}", arg_ty) != format!("{}", param_ty) {
            return Err(TypeError::TypeMismatch {
                expected: format!("{}", param_ty),
                found: format!("{}", arg_ty),
                context: format!("parameter {} of '{}'", i, sig.name),
            });
        }
    }
    // 2026-07-15: ReturnKind replaces return_type: Option<Type>
    Ok(match &sig.return_kind {
        ReturnKind::Native("Int") => Type::int(),
        ReturnKind::Native("Float") => Type::float(),
        ReturnKind::Native("Bool") => Type::bool_(),
        ReturnKind::Inferred => {
            // Infer from first argument's type
            args.first().map(|a| infer_type_only(a, ctx)).unwrap_or(Ok(Type::int()))?
        }
        ReturnKind::Exact(t) => t.clone(),
        _ => Type::int(), // fallback for unknown Native kinds
    })
}

/// Infer the type of a binary operation.
fn infer_binary_op(
    kind: &BinaryOpKind,
    lhs: &Expr,
    rhs: &Expr,
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    let lhs_ty = infer_type_only(lhs, ctx)?;
    let rhs_ty = infer_type_only(rhs, ctx)?;

    // Check type compatibility
    let lhs_str = format!("{}", lhs_ty);
    let rhs_str = format!("{}", rhs_ty);

    // Determine result type
    let result_ty = match kind {
        BinaryOpKind::Eq
        | BinaryOpKind::Neq
        | BinaryOpKind::Lt
        | BinaryOpKind::Gt
        | BinaryOpKind::Le
        | BinaryOpKind::Ge => Type::bool_(),
        BinaryOpKind::And | BinaryOpKind::Or => Type::bool_(),
        _ => {
            // Arithmetic/bitwise: return LHS type
            // Check via operator resolution
            let rune = format!("{}", kind);
            // 2026-07-18: Phase 0 — use universe-driven dispatch with
            // builtin fallback (get_operator_intrinsic already chains to
            // builtin_operator_binding when no universe property exists).
            let binding = get_operator_intrinsic(ctx.universe, &rune, &lhs_ty)
                .or_else(|| get_operator_intrinsic(ctx.universe, &rune, &rhs_ty));
            match binding {
                Some(_) => lhs_ty.clone(),
                None => {
                    return Err(TypeError::InvalidOperation {
                        operation: format!("'{}'", kind),
                        type_name: lhs_str,
                    });
                }
            }
        }
    };

    if kind.is_comparison() || kind.is_logical() {
        if lhs_str != rhs_str {
            return Err(TypeError::TypeMismatch {
                expected: lhs_str,
                found: rhs_str,
                context: format!("binary op '{}'", kind),
            });
        }
    }

    Ok(result_ty)
}

/// Infer the type of a unary operation.
fn infer_unary_op(
    kind: &UnaryOpKind,
    expr: &Expr,
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    let ty = infer_type_only(expr, ctx)?;
    match kind {
        UnaryOpKind::Neg => {
            let type_str = format!("{}", ty);
            if type_str == "Int" || type_str == "Float" {
                Ok(ty)
            } else {
                Err(TypeError::InvalidOperation {
                    operation: "negate".into(),
                    type_name: type_str,
                })
            }
        }
        UnaryOpKind::Not | UnaryOpKind::BitNot => {
            Ok(ty) // Simplified
        }
    }
}

/// Infer the type of an if expression.
fn infer_if(
    cond: &Expr,
    then: &Expr,
    else_: &Option<Box<Expr>>,
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    let cond_ty = infer_type_only(cond, ctx)?;
    let cond_str = format!("{}", cond_ty);
    if cond_str != "Bool" {
        return Err(TypeError::TypeMismatch {
            expected: "Bool".into(),
            found: cond_str,
            context: "if condition".into(),
        });
    }
    let then_ty = infer_type_only(then, ctx)?;
    if let Some(else_) = else_ {
        let else_ty = infer_type_only(else_, ctx)?;
        // Both branches should return the same type
        let then_str = format!("{}", then_ty);
        let else_str = format!("{}", else_ty);
        if then_str != else_str {
            return Err(TypeError::TypeMismatch {
                expected: then_str,
                found: else_str,
                context: "if/else branches".into(),
            });
        }
        Ok(then_ty)
    } else {
        Ok(then_ty)
    }
}

/// Infer the type of a match expression.
fn infer_match(
    expr: &Expr,
    arms: &[MatchArm],
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    let _matched_ty = infer_type_only(expr, ctx)?;
    if let Some(first) = arms.first() {
        infer_type_only(&first.body, ctx)
    } else {
        Ok(Type::void())
    }
}

/// Infer the type of a statement.
pub fn infer_statement(stmt: &Statement, ctx: &mut TypecheckContext) -> Result<(), TypeError> {
    match stmt {
        Statement::Let { name, ty, expr, .. } => {
            let inferred = if let Some(expr) = expr {
                infer_type_only(expr, ctx)?
            } else {
                Type::int()
            };
            let resolved = ty.clone().unwrap_or(inferred);
            ctx.bindings.insert(name.clone(), resolved);
            Ok(())
        }
        Statement::Assign(lhs, rhs) => {
            // 2026-07-18: Write-through guard — reject *p = val when p is Ptr<const T>.
            if let Expr::Deref(ptr) = lhs {
                if let Ok((ptr_ty, _)) = infer_expression(ptr, ctx) {
                    if matches!(ptr_ty, Type::PtrConst(_)) {
                        return Err(TypeError::InvalidOperation {
                            operation: "write through const pointer (*p = val)".into(),
                            type_name: format!("{}", ptr_ty),
                        });
                    }
                }
            }
            infer_type_only(lhs, ctx)?;
            infer_type_only(rhs, ctx)?;
            Ok(())
        }
        Statement::Term(val) | Statement::TermBang(val) => {
            if let Some(val) = val {
                infer_type_only(val, ctx)?;
            }
            Ok(())
        }
        Statement::Return(val) => {
            if let Some(val) = val {
                infer_type_only(val, ctx)?;
            }
            Ok(())
        }
        Statement::Expression(expr) => {
            infer_type_only(expr, ctx)?;
            Ok(())
        }
        Statement::If(cond, then, else_) => {
            infer_type_only(cond, ctx)?;
            for stmt in then {
                infer_statement(stmt, ctx)?;
            }
            for stmt in else_ {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::Guarded(cond, body) => {
            infer_type_only(cond, ctx)?;
            for stmt in body {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::Block(stmts) => {
            for stmt in stmts {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::Escape(_) => Ok(()),
        Statement::Foreach { item, list, body } => {
            let list_ty = infer_type_only(list, ctx)?;
            // Element type: assume List<T> has element T
            ctx.bindings.insert(item.clone(), Type::int());
            for stmt in body {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::TrgBinding { instance, .. } => {
            infer_type_only(instance, ctx)?;
            Ok(())
        }
        Statement::InlineAsm { .. } => Ok(()),
        Statement::SyncBlock(body) => {
            for stmt in body {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::MetadataAssignment(_, _) => Ok(()),
    }
}

/// Type-check a complete program.
pub fn check_program(items: &[TopLevel], universe: &TypeUniverse) -> Result<(), Vec<TypeError>> {
    // 2026-07-14: Pre-collect state variable bindings from top-level `let`
    // so they are visible to all transactions and definitions.
    let state_bindings: HashMap<String, Type> = items.iter().filter_map(|item| {
        match item {
            TopLevel::Statement(stmt) => {
                if let Statement::Let { name, ty, .. } = stmt.as_ref() {
                    return ty.clone().map(|t| (name.clone(), t));
                }
            }
            // 2026-07-15: Also collect top-level const so it's visible in txn bodies
            TopLevel::Constant(c) => {
                return Some((c.name.clone(), c.ty.clone()));
            }
            _ => {}
        }
        None
    }).collect();

    let mut errors = Vec::new();
    for item in items {
        if let Err(e) = check_top_level(item, universe, &state_bindings) {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Type-check a top-level item.
fn check_top_level(item: &TopLevel, universe: &TypeUniverse, state_bindings: &HashMap<String, Type>) -> Result<(), TypeError> {
    let mut ctx = TypecheckContext::new(universe);
    // 2026-07-14: Inject state variable bindings so transactions/defns can reference them.
    for (name, ty) in state_bindings {
        ctx.bindings.insert(name.clone(), ty.clone());
        ctx.state_keys.insert(name.clone());
    }
    // 2026-07-18: Txn parameters are also mutable state locations (they hold
    // state field values within the txn body).
    if let TopLevel::Transaction(txn) = item {
        for (name, _) in &txn.parameters {
            ctx.state_keys.insert(name.clone());
        }
    }
    match item {
        TopLevel::Definition(defn) => {
            for (name, ty) in &defn.parameters {
                ctx.bindings.insert(name.clone(), ty.clone());
            }
            for stmt in &defn.body {
                infer_statement(stmt, &mut ctx)?;
            }
            Ok(())
        }
        TopLevel::Transaction(txn) => {
            for (name, ty) in &txn.parameters {
                ctx.bindings.insert(name.clone(), ty.clone());
            }
            for stmt in &txn.body {
                infer_statement(stmt, &mut ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── BinaryOpKind helpers ───────────────────────────────────────────────

impl BinaryOpKind {
    fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinaryOpKind::Eq
                | BinaryOpKind::Neq
                | BinaryOpKind::Lt
                | BinaryOpKind::Gt
                | BinaryOpKind::Le
                | BinaryOpKind::Ge
        )
    }

    fn is_logical(&self) -> bool {
        matches!(self, BinaryOpKind::And | BinaryOpKind::Or)
    }
}


