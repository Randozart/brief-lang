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

use crate::ast::*;
use crate::errors::{SyntaxError, TypeError};
use crate::intrinsic_signatures::{get_intrinsic_signature, ReturnKind, Signature};
use crate::type_universe::{builtin_operator_binding, TypeUniverse};
use std::collections::HashMap;

/// Type-check context: variable bindings and type universe.
pub struct TypecheckContext<'a> {
    pub bindings: HashMap<String, Type>,
    pub universe: &'a TypeUniverse,
}

impl<'a> TypecheckContext<'a> {
    pub fn new(universe: &'a TypeUniverse) -> Self {
        TypecheckContext {
            bindings: HashMap::new(),
            universe,
        }
    }
}

/// Infer the type of an expression in the given context.
pub fn infer_expression(expr: &Expr, ctx: &mut TypecheckContext) -> Result<Type, TypeError> {
    match expr {
        // ── Literals ────────────────────────────────────────────
        Expr::Decimal(_) => Ok(Type::int()),
        Expr::Float(_) => Ok(Type::float()),
        Expr::Bool(_) => Ok(Type::bool_()),
        Expr::Quoted(_) => Ok(Type::string()),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => {
            ctx.bindings
                .get(name)
                .cloned()
                .ok_or_else(|| TypeError::UndefinedVariable {
                    name: name.clone(),
                    available: ctx.bindings.keys().cloned().collect(),
                })
        }

        // ── Calls ───────────────────────────────────────────────
        Expr::Call(name, args) => infer_call(name, args, ctx),

        // ── Binary operators ─────────────────────────────────────
        Expr::BinaryOp(kind, lhs, rhs) => infer_binary_op(kind, lhs, rhs, ctx),

        // ── Unary operators ──────────────────────────────────────
        Expr::UnaryOp(kind, expr) => infer_unary_op(kind, expr, ctx),

        // ── Other expressions ────────────────────────────────────
        Expr::Block(stmts) => {
            for stmt in stmts {
                infer_statement(stmt, ctx)?;
            }
            Ok(Type::void())
        }
        Expr::If(cond, then, else_) => infer_if(cond, then, else_, ctx),
        Expr::Tuple(elems) => {
            let types: Result<Vec<Type>, _> =
                elems.iter().map(|e| infer_expression(e, ctx)).collect();
            Ok(Type::Tuple(types?))
        }
        Expr::List(elems) => {
            if let Some(first) = elems.first() {
                let elem_ty = infer_expression(first, ctx)?;
                Ok(Type::Applied("List".into(), vec![elem_ty]))
            } else {
                Ok(Type::Applied("List".into(), vec![Type::int()]))
            }
        }
        Expr::Lambda(params, body) => {
            for param in params {
                ctx.bindings.insert(param.clone(), Type::int());
            }
            let ret_ty = infer_expression(body, ctx)?;
            Ok(Type::Function(
                params.iter().map(|_| Type::int()).collect(),
                Box::new(ret_ty),
            ))
        }
        Expr::Field(obj, _name) => {
            // Simplified: just return the object's type
            infer_expression(obj, ctx)
        }
        Expr::Index(obj, index) => {
            let _obj_ty = infer_expression(obj, ctx)?;
            let _idx_ty = infer_expression(index, ctx)?;
            // Simplified: assume index returns element type
            Ok(Type::int())
        }
        Expr::Cast(expr, target_ty) => {
            infer_expression(expr, ctx)?;
            Ok(target_ty.clone())
        }
        Expr::IsType(expr, _ty) => {
            infer_expression(expr, ctx)?;
            Ok(Type::bool_())
        }
        Expr::Within(expr, scope) => {
            infer_expression(scope, ctx)?;
            infer_expression(expr, ctx)
        }
        Expr::DerivationBlock(_) => Ok(Type::void()),
        // 2026-07-15: Dereference: *ptr returns the pointee type (strip outer Ptr).
        Expr::Deref(inner) => {
            let inner_ty = infer_expression(inner, ctx)?;
            match inner_ty {
                Type::Ptr(pointee) => Ok((*pointee).clone()),
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
        Expr::FormattingAnnotation(_) => Ok(Type::void()),
        Expr::Match(expr, arms) => infer_match(expr, arms, ctx),
    }
}

/// Infer the type of a function/intrinsic call.
fn infer_call(name: &str, args: &[Expr], ctx: &mut TypecheckContext) -> Result<Type, TypeError> {
    // Intrinsic call (ends with #): look up signature
    if name.ends_with('#') {
        let sig = get_intrinsic_signature(name).ok_or_else(|| {
            let found: Vec<String> = args
                .iter()
                .filter_map(|a| infer_expression(a, ctx).ok())
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
        infer_expression(arg, ctx)?;
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
        let arg_ty = infer_expression(&args[i], ctx)?;
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
            args.first().map(|a| infer_expression(a, ctx)).unwrap_or(Ok(Type::int()))?
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
    let lhs_ty = infer_expression(lhs, ctx)?;
    let rhs_ty = infer_expression(rhs, ctx)?;

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
            let binding = builtin_operator_binding(&rune, &lhs_ty)
                .or_else(|| builtin_operator_binding(&rune, &rhs_ty));
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
    let ty = infer_expression(expr, ctx)?;
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
    let cond_ty = infer_expression(cond, ctx)?;
    let cond_str = format!("{}", cond_ty);
    if cond_str != "Bool" {
        return Err(TypeError::TypeMismatch {
            expected: "Bool".into(),
            found: cond_str,
            context: "if condition".into(),
        });
    }
    let then_ty = infer_expression(then, ctx)?;
    if let Some(else_) = else_ {
        let else_ty = infer_expression(else_, ctx)?;
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
    let _matched_ty = infer_expression(expr, ctx)?;
    if let Some(first) = arms.first() {
        infer_expression(&first.body, ctx)
    } else {
        Ok(Type::void())
    }
}

/// Infer the type of a statement.
pub fn infer_statement(stmt: &Statement, ctx: &mut TypecheckContext) -> Result<(), TypeError> {
    match stmt {
        Statement::Let { name, ty, expr, .. } => {
            let inferred = if let Some(expr) = expr {
                infer_expression(expr, ctx)?
            } else {
                Type::int()
            };
            let resolved = ty.clone().unwrap_or(inferred);
            ctx.bindings.insert(name.clone(), resolved);
            Ok(())
        }
        Statement::Assign(lhs, rhs) => {
            infer_expression(lhs, ctx)?;
            infer_expression(rhs, ctx)?;
            Ok(())
        }
        Statement::Term(val) | Statement::TermBang(val) => {
            if let Some(val) = val {
                infer_expression(val, ctx)?;
            }
            Ok(())
        }
        Statement::Return(val) => {
            if let Some(val) = val {
                infer_expression(val, ctx)?;
            }
            Ok(())
        }
        Statement::Expression(expr) => {
            infer_expression(expr, ctx)?;
            Ok(())
        }
        Statement::If(cond, then, else_) => {
            infer_expression(cond, ctx)?;
            for stmt in then {
                infer_statement(stmt, ctx)?;
            }
            for stmt in else_ {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::Guarded(cond, body) => {
            infer_expression(cond, ctx)?;
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
            let list_ty = infer_expression(list, ctx)?;
            // Element type: assume List<T> has element T
            ctx.bindings.insert(item.clone(), Type::int());
            for stmt in body {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::TrgBinding { instance, .. } => {
            infer_expression(instance, ctx)?;
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
        if let TopLevel::Statement(stmt) = item {
            if let Statement::Let { name, ty, .. } = stmt.as_ref() {
                return ty.clone().map(|t| (name.clone(), t));
            }
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


