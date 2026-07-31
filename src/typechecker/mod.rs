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
    /// 2026-07-20: Parse ops per type, for literal construction resolution.
    /// Populated from AST TypeDef bodies in check_program.
    /// Key: type_name → Vec of Parse OperatorDefs.
    parse_ops: HashMap<String, Vec<crate::ast::top::OperatorDef>>,
    /// 2026-07-27: Type parent relationships for hierarchy walking.
    /// Populated from TypeDef.parent fields in check_program.
    /// type_parents[child] == parent_name.
    type_parents: HashMap<String, String>,
    /// 2026-07-25: Function return types for user-defined functions.
    /// Populated by check_program before type-checking bodies.
    fn_return_types: HashMap<String, Type>,
    /// 2026-07-31: Regular operator declarations from TypeDef bodies
    /// (`op Add(#Float): func(#L,#R);` / `op Add(Float): ...;`), keyed by type
    /// name. Used to ALLOW mixed-type arithmetic ONLY when a cross-type /
    /// cross-protocol overload is explicitly declared — otherwise
    /// `Int * Float` is a type error (no implicit numeric coercion).
    /// Both `td.body.operators` (ProtocolDef-style OperatorDefs) and
    /// `td.body.op_bindings` (type-body `op Name(#Proto): fn(#L,#R);`) are
    /// collected; the operand type lives in OperatorDef.params or
    /// OperatorBinding.protocol_variant.
    regular_ops: HashMap<String, Vec<crate::ast::top::OperatorDef>>,
    regular_bindings: HashMap<String, Vec<crate::ast::top::OperatorBinding>>,
}

impl<'a> TypecheckContext<'a> {
    pub fn new(universe: &'a TypeUniverse) -> Self {
        TypecheckContext {
            bindings: HashMap::new(),
            state_keys: std::collections::HashSet::new(),
            universe,
            parse_ops: HashMap::new(),
            type_parents: HashMap::new(),
            fn_return_types: HashMap::new(),
            regular_ops: HashMap::new(),
            regular_bindings: HashMap::new(),
        }
    }

    /// 2026-07-31: Does a cross-type / cross-protocol operator overload exist
    /// for `rune` between `lhs` and `rhs`? A type declaring `op Add(#Float)` or
    /// `op Add(Float)` on its body authorizes `T + Float` without an explicit
    /// cast. The builtin primordials (Int, Float, …) declare NO cross-type ops,
    /// so `Int * Float` stays a type error (the type-safety guarantee).
    fn has_cross_type_overload(&self, rune: &str, lhs: &Type, rhs: &Type) -> bool {
        let Some(op_name) = crate::type_universe::operators::rune_to_op_name(rune) else {
            return false;
        };
        self.type_declares_op(lhs, op_name, rhs) || self.type_declares_op(rhs, op_name, lhs)
    }

    /// Does `ty` declare `op <op_name>` whose declared operand covers `operand`?
    fn type_declares_op(&self, ty: &Type, op_name: &str, operand: &Type) -> bool {
        let type_name = match ty {
            Type::Custom(n) => n.as_str(),
            Type::Applied(n, _) => n.as_str(),
            _ => return false,
        };
        if let Some(ops) = self.regular_ops.get(type_name) {
            if ops.iter().any(|op| {
                op.op == op_name
                    && op.params.first().map_or(false, |p| self.param_covers(p, operand))
            }) {
                return true;
            }
        }
        if let Some(bindings) = self.regular_bindings.get(type_name) {
            if bindings.iter().any(|b| {
                b.name == op_name && b.protocol_variant.as_ref().map_or(false, |v| self.variant_covers(v, operand))
            }) {
                return true;
            }
        }
        false
    }

    /// Does a declared operator parameter cover the operand type?
    /// A `#Float` hashword covers any #Float-protocol member; a concrete
    /// `Float` covers exactly that type.
    fn param_covers(&self, param: &Type, operand: &Type) -> bool {
        if param == operand {
            return true;
        }
        let Type::HashWord(hw) = param else {
            return false;
        };
        if hw == "#Bit" {
            // Universal — every type is a member of #Bit via Cast.#Bit.
            return operand.universe_key().is_some();
        }
        let prop = format!("Cast.{}", hw);
        operand
            .universe_key()
            .and_then(|k| self.universe.get(k))
            .map_or(false, |rt| rt.properties.contains_key(&prop))
    }

    /// Does a type-body op's declared variant (`#Float` or `Float`) cover the
    /// operand type?
    fn variant_covers(&self, variant: &str, operand: &Type) -> bool {
        if variant.starts_with('#') {
            let prop = format!("Cast.{}", variant);
            operand
                .universe_key()
                .and_then(|k| self.universe.get(k))
                .map_or(false, |rt| rt.properties.contains_key(&prop))
        } else {
            // Concrete type name.
            match operand {
                Type::Custom(n) => n == variant,
                Type::Applied(n, _) => n == variant,
                _ => false,
            }
        }
    }

    /// 2026-07-20: Find a Parse op on a type that could accept a literal form.
    /// form: "Decimal", "Quoted", "Bare", or a hashword category like "#Int".
    /// discriminator: optional prefix/suffix hint ("0x", "h", "bf", etc.)
    /// Returns the OperatorDef if a matching Parse op exists.
    /// Qualified ops (with pre:/suf:) win over unqualified ops.
    /// 2026-07-27: Walks the type hierarchy (type → parent → grandparent).
    pub fn find_parse_op(
        &self,
        type_name: &str,
        form: &str,
        discriminator: Option<&str>,
    ) -> Option<&crate::ast::top::OperatorDef> {
        let mut current = Some(type_name.to_string());
        while let Some(tn) = current {
            if let Some(defs) = self.parse_ops.get(&tn) {
                // 1. Exact discriminator match (pre or suf matches discriminator)
                if let Some(d) = discriminator {
                    if let Some(def) = defs.iter().find(|op| {
                        matches_form(&op.params, form)
                            && (op.pre.as_deref() == Some(d) || op.suf.as_deref() == Some(d))
                    }) {
                        return Some(def);
                    }
                }
                // 2. Qualified match (has pre: or suf:, no discriminator given)
                if discriminator.is_none() {
                    if let Some(def) = defs
                        .iter()
                        .find(|op| matches_form(&op.params, form) && (op.pre.is_some() || op.suf.is_some()))
                    {
                        return Some(def);
                    }
                }
                // 3. Unqualified match (no pre:/suf:)
                if let Some(def) = defs
                    .iter()
                    .find(|d| matches_form(&d.params, form) && d.pre.is_none() && d.suf.is_none())
                {
                    return Some(def);
                }
                // 4. Hashword identity (e.g., Parse(#Int) for Decimal)
                if let Some(def) = defs.iter().find(|d| matches_parse_identity(&d.params, form)) {
                    return Some(def);
                }
            }
            // Move to parent type
            current = self.type_parents.get(&tn).cloned();
        }
        None
    }

    /// 2026-07-20: Register Parse ops from a type's definitions.
    pub fn register_parse_ops(&mut self, type_name: &str, ops: Vec<crate::ast::top::OperatorDef>) {
        let parse_ops: Vec<_> = ops.into_iter().filter(|d| d.op == "Parse").collect();
        if !parse_ops.is_empty() {
            self.parse_ops.insert(type_name.to_string(), parse_ops);
        }
    }

    /// 2026-07-27: Register Parse bindings from a type's OperatorBinding entries.
    /// Converts OperatorBinding to OperatorDef and stores in parse_ops.
    /// Also records parent type relationship for hierarchy walking.
    pub fn register_parse_bindings(
        &mut self,
        type_name: &str,
        bindings: Vec<OperatorBinding>,
        parent: Option<&Expr>,
    ) {
        let defs: Vec<crate::ast::top::OperatorDef> = bindings
            .iter()
            .filter(|b| b.name == "Parse")
            .map(|b| crate::ast::top::OperatorDef {
                op: "Parse".to_string(),
                params: match &b.protocol_variant {
                    Some(pv) => vec![Type::Custom(pv.clone())],
                    None => vec![],
                },
                pre: b.pre.clone(),
                suf: b.suf.clone(),
                impl_args: None,
                impl_name: b.name.clone(),
                span: b.span.clone(),
            })
            .collect();
        if !defs.is_empty() {
            self.parse_ops.insert(type_name.to_string(), defs);
        }
        if let Some(Expr::Identifier(pname)) = parent {
            self.type_parents.insert(type_name.to_string(), pname.clone());
        }
    }

    /// 2026-07-18: Check if a variable name refers to a mutable state location.
    /// Used by AddrOf to decide Ptr<T> (mutable) vs Ptr<const T> (immutable).
    pub fn is_mutable_location(&self, name: &str) -> bool {
        self.state_keys.contains(name)
    }
}

/// Check if an op's params match a literal form (Decimal, Quoted, Bare, #Int, etc.)
/// Empty params = wildcard (matches all forms). Single param = exact match.
fn matches_form(params: &[Type], form: &str) -> bool {
    // 2026-07-27: Empty params means wildcard (matches any form).
    // This handles op Parse: parse_string(#L); (no protocol variant).
    if params.is_empty() {
        return true;
    }
    if params.len() != 1 {
        return false;
    }
    match &params[0] {
        Type::Custom(n) => n == form,
        Type::HashWord(s) => s.strip_prefix('#') == Some(form) || s.as_str() == form,
        Type::HashWordVariant(s, _) => s.strip_prefix('#') == Some(form) || s.as_str() == form,
        _ => false,
    }
}

/// Check if a hashword identity op matches a literal form.
/// op Parse(#Int) matches Decimal literal (Int is the protocol for numbers).
/// op Parse(#String) matches Quoted literal (String is the protocol for text).
fn matches_parse_identity(params: &[Type], form: &str) -> bool {
    if params.len() != 1 {
        return false;
    }
    let hashword_category = match &params[0] {
        Type::HashWord(s) => s.strip_prefix('#').unwrap_or(s),
        Type::HashWordVariant(s, _) => s.strip_prefix('#').unwrap_or(s),
        _ => return false,
    };
    match (hashword_category, form) {
        ("Int", "Decimal") => true,
        ("Float", "Decimal") => true,
        ("String", "Quoted") => true,
        ("Bool", "Bare") => true,
        _ => false,
    }
}

/// Infer the type of an expression in the given context.
/// Infer the type of an expression. Returns both the type and provenance.
/// 2026-07-18: Phase 2 — Thread Provenance through type inference for
/// pointer tracking and dangling-borrow detection.
pub fn infer_expression(
    expr: &Expr,
    ctx: &mut TypecheckContext,
) -> Result<(Type, Provenance), TypeError> {
    match expr {
        // ── Literals ────────────────────────────────────────────
        Expr::Decimal(_) | Expr::TaggedLiteral(_, _) => Ok((Type::int(), Provenance::Unknown)),
        Expr::Float(_) => Ok((Type::float(), Provenance::Unknown)),
        Expr::Bool(_) => Ok((Type::bool_(), Provenance::Unknown)),
        Expr::Quoted(_) | Expr::TaggedQuotedLiteral(_, _) => Ok((Type::string(), Provenance::Unknown)),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => {
            let ty =
                ctx.bindings
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
            let types: Result<Vec<Type>, _> = elems
                .iter()
                .map(|e| infer_expression(e, ctx).map(|(t, _)| t))
                .collect();
            Ok((Type::Tuple(types?), Provenance::Unknown))
        }
        Expr::List(elems) => {
            if let Some(first) = elems.first() {
                let (elem_ty, _) = infer_expression(first, ctx)?;
                Ok((
                    Type::Applied("List".into(), vec![elem_ty]),
                    Provenance::Unknown,
                ))
            } else {
                Ok((
                    Type::Applied("List".into(), vec![Type::int()]),
                    Provenance::Unknown,
                ))
            }
        }
        Expr::Lambda(params, body) => {
            for param in params {
                ctx.bindings.insert(param.clone(), Type::int());
            }
            let (ret_ty, _) = infer_expression(body, ctx)?;
            Ok((
                Type::Function(
                    params.iter().map(|_| Type::int()).collect(),
                    Box::new(ret_ty),
                ),
                Provenance::Unknown,
            ))
        }
        Expr::Field(obj, name) => {
            let (obj_ty, obj_prov) = infer_expression(obj, ctx)?;
            Ok((
                obj_ty,
                Provenance::FieldAccess {
                    base: Box::new(obj_prov),
                    field: name.clone(),
                },
            ))
        }
        Expr::Index(obj, index) => {
            let (_, obj_prov) = infer_expression(obj, ctx)?;
            let (_, idx_prov) = infer_expression(index, ctx)?;
            Ok((
                Type::int(),
                Provenance::Index {
                    base: Box::new(obj_prov),
                    index: Box::new(idx_prov),
                },
            ))
        }
        Expr::Cast(expr, target_ty) => {
            let (src_ty, prov) = infer_expression(expr, ctx)?;
            // 2026-07-26: Vector-to-Vector view cast validation.
            if let (Type::Vector(src_inner, src_dims), Type::Vector(tgt_inner, tgt_dims)) = (&src_ty, target_ty) {
                if src_inner != tgt_inner || src_dims != tgt_dims {
                    let src_bytes = type_byte_size(&src_ty);
                    let tgt_bytes = type_byte_size(target_ty);
                    if src_bytes != tgt_bytes {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("array of {} bytes", tgt_bytes),
                            found: format!("{} ({} bytes)", src_ty, src_bytes),
                            context: "view cast: source and target byte sizes must match".to_string(),
                        });
                    }
                }
            }
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
        Expr::DerivationBlock(_) | Expr::StructLiteral { .. } => Ok((Type::void(), Provenance::Unknown)),
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
                Type::Ptr(pointee) => {
                    Ok(((*pointee).clone(), Provenance::Deref(Box::new(inner_prov))))
                }
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
        Expr::Match(expr, arms) => infer_match(expr, arms, ctx).map(|ty| (ty, Provenance::Unknown)),
        // 2026-07-19: Plugin-intercept calls are resolved by Front or Mid
        // stage plugins. The typechecker passes them through with Void return;
        // the plugin is responsible for final dispatch.
        Expr::PluginIntercept { .. } => Ok((Type::void(), Provenance::Unknown)),
        Expr::Exists(_) => { unreachable!("fn? only in stage eval") },
            Expr::Slice { array, start, end, stride } => {
                let elem_ty = infer_type_only(array, ctx)?;
                if let Some(e) = start.as_deref() { infer_type_only(e, ctx)?; }
                if let Some(e) = end.as_deref() { infer_type_only(e, ctx)?; }
                if let Some(e) = stride.as_deref() { infer_type_only(e, ctx)?; }
                Ok((elem_ty, Provenance::Unknown))
            }

    }
}

/// 2026-07-20: Try to coerce a value from its inferred type to a target type
/// via Parse ops. Returns true if the value can be accepted via Parse.
/// Only works for literal expressions (Decimal, Quoted, Identifier).
/// 2026-07-27: Handles TaggedLiteral (suffix/prefix discriminators) and
/// TaggedQuotedLiteral (prefix-tagged strings like sql"...").
fn try_coerce_via_parse(
    expr: &Expr,
    arg_ty: &Type,
    target_ty: &Type,
    ctx: &TypecheckContext,
) -> bool {
    let (form, discriminator) = match expr {
        Expr::Decimal(_) => ("Decimal", None),
        Expr::Float(_) => ("Decimal", None),
        Expr::TaggedLiteral(_, tag) => ("Decimal", Some(tag.as_str())),
        Expr::Quoted(_) => ("Quoted", None),
        Expr::TaggedQuotedLiteral(_, prefix) => ("Quoted", Some(prefix.as_str())),
        Expr::Identifier(_) => ("Bare", None),
        _ => return (false),
    };
    let target_name = match target_ty {
        Type::Custom(n) => n.as_str(),
        _ => return false,
    };
    ctx.find_parse_op(target_name, form, discriminator)
        .is_some()
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

    // User function call — look up return type
    for arg in args {
        infer_type_only(arg, ctx)?;
    }
    // 2026-07-25: Look up user-defined function return types.
    if let Some(ty) = ctx.fn_return_types.get(name) {
        return Ok(ty.clone());
    }
    // 2026-07-25: Fallback — bind all destructured names to Int.
    Ok(Type::int())
}

/// Type-check an intrinsic call against its signature.
fn infer_intrinsic_call(
    sig: &Signature,
    args: &[Expr],
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    // 2026-07-14: Empty parameters means type-inferred — skip count check.
    // 2026-07-26: Variadic intrinsics (e.g., SysCall#) accept 1+ args.
    if !sig.variadic && !sig.parameters.is_empty() && args.len() != sig.parameters.len() {
        return Err(TypeError::TypeMismatch {
            expected: format!("{} parameters", sig.parameters.len()),
            found: format!("{} arguments", args.len()),
            context: format!("call to '{}'", sig.name),
        });
    }
    for (i, (_, param_ty)) in sig.parameters.iter().enumerate() {
        let arg_ty = infer_type_only(&args[i], ctx)?;
        let arg_str = format!("{}", arg_ty);
        let param_str = format!("{}", param_ty);
        if arg_str != param_str {
            // 2026-07-20: Check for Parse op coercion before reporting error
            if !try_coerce_via_parse(&args[i], &arg_ty, param_ty, ctx) {
                return Err(TypeError::TypeMismatch {
                    expected: param_str,
                    found: arg_str,
                    context: format!("parameter {} of '{}'", i, sig.name),
                });
            }
        }
    }
    // 2026-07-15: ReturnKind replaces return_type: Option<Type>
    Ok(match &sig.return_kind {
        ReturnKind::Native("Int") => Type::int(),
        ReturnKind::Native("Float") => Type::float(),
        ReturnKind::Native("Bool") => Type::bool_(),
        ReturnKind::Inferred => {
            // Infer from first argument's type
            args.first()
                .map(|a| infer_type_only(a, ctx))
                .unwrap_or(Ok(Type::int()))?
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
    } else if lhs_str != rhs_str {
        // 2026-07-31: No implicit numeric coercion — `Int * Float` is a TYPE
        // ERROR unless the LHS (or RHS) type declares a cross-type / cross-
        // protocol operator overload (`op Mul(#Float)` / `op Mul(Float)`). The
        // old behavior silently bitcast the Int to Float, producing garbage
        // (accumulator_flush `(count % 101) * 0.5` summed ~0). Add an explicit
        // `as Float` / `as Int` cast, or declare the overload. AGENTS.md:
        // "All type reinterpretations must be explicit via `as` casts."
        if !ctx.has_cross_type_overload(&format!("{}", kind), &lhs_ty, &rhs_ty) {
            return Err(TypeError::TypeMismatch {
                expected: lhs_str,
                found: rhs_str,
                context: format!(
                    "binary op '{}' — no implicit Int/Float coercion and no \
                     cross-type `op` overload; add an explicit `as` cast",
                    kind
                ),
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
        Statement::Let { name, names, ty, expr, .. } => {
            // 2026-07-25: Tuple destructuring or single let.
            if names.len() > 1 {
                return check_let_destructure(names, expr, ctx);
            }
            if names.len() == 1 && names[0] == "_" {
                // 2026-07-25: Discard binding — evaluate expr but don't bind.
                if let Some(e) = expr {
                    infer_type_only(e, ctx)?;
                }
                return Ok(());
            }
            // 2026-07-25: Single-name let: bind name or handle discard (_).
            let inferred = match expr {
                Some(e) => infer_type_only(e, ctx)?,
                None => Type::int(),
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
        Statement::Gate(cond) => {
            infer_type_only(cond, ctx)?;
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
        Statement::InlineAsm { .. } | Statement::InlineDefn(_) | Statement::InlineTxn(_) | Statement::Match { .. } => Ok(()),
        Statement::SyncBlock(body) => {
            for stmt in body {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::MetadataAssignment(_, _) => Ok(()),
    }
}

/// 2026-07-25: Type-check tuple destructuring: let (a, b) = expr;
/// Extracts each element type from the tuple and binds the names.
fn check_let_destructure(
    names: &[String],
    expr: &Option<Expr>,
    ctx: &mut TypecheckContext,
) -> Result<(), TypeError> {
    // 2026-07-25: Always bind destructured names — type is Int as fallback.
    let elem_types = match expr {
        Some(e) => match infer_type_only(e, ctx) {
            Ok(Type::Tuple(types)) => types,
            _ => vec![Type::int(); names.len()],
        },
        None => vec![Type::int(); names.len()],
    };
    let count = elem_types.len().min(names.len());
    for i in 0..count {
        ctx.bindings.insert(names[i].clone(), elem_types[i].clone());
    }
    Ok(())
}

/// 2026-07-25: Convert OutputType variants to Type.
fn output_type_to_type(ot: &OutputType) -> Type {
    match ot {
        OutputType::Single(ty) => ty.clone(),
        OutputType::Tuple(types) => {
            Type::Tuple(types.iter().map(output_type_to_type).collect())
        }
        OutputType::Union(types) => {
            Type::Union(types.iter().map(output_type_to_type).collect())
        }
        _ => Type::int(),
    }
}

/// Type-check a complete program.
pub fn check_program(items: &[TopLevel], universe: &TypeUniverse) -> Result<(), Vec<TypeError>> {
    // 2026-07-14: Pre-collect state variable bindings from top-level `let`
    // so they are visible to all transactions and definitions.
    let state_bindings: std::collections::HashMap<String, Type> = items
        .iter()
        .filter_map(|item| {
            match item {
                TopLevel::Statement(stmt) => {
                    if let Statement::Let { name, ty, .. } = stmt.as_ref() {
                        return ty.clone().map(|t| (name.clone(), t));
                    }
                }
                TopLevel::Constant(c) => {
                    return Some((c.name.clone(), c.ty.clone()));
                }
                _ => {}
            }
            None
        })
        .collect();

    let mut errors = Vec::new();

    // 2026-07-25: Pre-collect function return types for call inference.
    let fn_return_types: HashMap<String, Type> = items.iter().filter_map(|item| {
        // 2026-07-25: Extract return type from a Definition or exported Definition.
        fn defn_return_type(d: &Definition) -> Option<Type> {
            d.output_type.as_ref().and_then(|ot| match ot {
                OutputType::Single(Type::Tuple(_)) => Some(ot.all_types().into_iter().next().unwrap_or(Type::int())),
                OutputType::Single(ty) => Some(ty.clone()),
                OutputType::Tuple(types) => {
                    Some(Type::Tuple(types.iter().map(|t| output_type_to_type(t)).collect()))
                }
                _ => None,
            })
        }
        match item {
            TopLevel::Definition(d) => {
                defn_return_type(d).map(|ty| (d.name.clone(), ty))
            }
            TopLevel::Export(e) => match &*e.inner {
                TopLevel::Definition(d) => defn_return_type(d).map(|ty| (d.name.clone(), ty)),
                _ => None,
            },
            _ => None,
        }
    }).collect();

    // 2026-07-27: Pre-collect Parse bindings and type parents from ALL TypeDef items.
    let mut all_parse_bindings: HashMap<String, Vec<OperatorBinding>> = HashMap::new();
    let mut all_type_parents: HashMap<String, String> = HashMap::new();
    for item in items {
        if let TopLevel::TypeDef(td) = item {
            if !td.body.op_bindings.is_empty() {
                all_parse_bindings.insert(td.name.clone(), td.body.op_bindings.clone());
            }
            if let Some(parent) = &td.parent {
                if let Expr::Identifier(pname) = parent.as_ref() {
                    all_type_parents.insert(td.name.clone(), pname.clone());
                }
            }
        }
    }

    // 2026-07-31: Pre-collect regular operator declarations (cross-type /
    // cross-protocol overloads) from TypeDef bodies: ProtocolDef-style
    // `operators` and type-body `op_bindings`.
    let mut all_regular_ops: HashMap<String, Vec<crate::ast::top::OperatorDef>> = HashMap::new();
    let mut all_regular_bindings: HashMap<String, Vec<crate::ast::top::OperatorBinding>> = HashMap::new();
    for item in items {
        if let TopLevel::TypeDef(td) = item {
            if !td.body.operators.is_empty() {
                all_regular_ops.insert(td.name.clone(), td.body.operators.clone());
            }
            if !td.body.op_bindings.is_empty() {
                all_regular_bindings.insert(td.name.clone(), td.body.op_bindings.clone());
            }
        }
    }

    for item in items {
        if let Err(e) = check_top_level(
            item, universe, &state_bindings, &fn_return_types,
            &all_parse_bindings, &all_type_parents, &all_regular_ops, &all_regular_bindings,
        ) {
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
fn check_top_level(
    item: &TopLevel,
    universe: &TypeUniverse,
    state_bindings: &HashMap<String, Type>,
    fn_return_types: &HashMap<String, Type>,
    all_parse_bindings: &HashMap<String, Vec<OperatorBinding>>,
    all_type_parents: &HashMap<String, String>,
    all_regular_ops: &HashMap<String, Vec<crate::ast::top::OperatorDef>>,
    all_regular_bindings: &HashMap<String, Vec<crate::ast::top::OperatorBinding>>,
) -> Result<(), TypeError> {
    let mut ctx = TypecheckContext::new(universe);
    // 2026-07-27: Inject pre-collected parse bindings and type parents.
    for (type_name, bindings) in all_parse_bindings {
        ctx.register_parse_bindings(type_name, bindings.clone(), None);
    }
    ctx.type_parents = all_type_parents.clone();
    // 2026-07-31: Inject regular operator declarations for cross-type overloads.
    ctx.regular_ops = all_regular_ops.clone();
    ctx.regular_bindings = all_regular_bindings.clone();
    // 2026-07-14: Inject state variable bindings so transactions/defns can reference them.
    for (name, ty) in state_bindings {
        ctx.bindings.insert(name.clone(), ty.clone());
        ctx.state_keys.insert(name.clone());
    }
    // 2026-07-25: Inject function return types for call inference.
    for (name, ty) in fn_return_types {
        ctx.fn_return_types.insert(name.clone(), ty.clone());
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
        // 2026-07-25: Unwrap exports so exported defns are type-checked.
        TopLevel::Export(e) => check_top_level(&e.inner, universe, state_bindings, fn_return_types, all_parse_bindings, all_type_parents, all_regular_ops, all_regular_bindings),
        TopLevel::Transaction(txn) => {
            for (name, ty) in &txn.parameters {
                ctx.bindings.insert(name.clone(), ty.clone());
            }
            for stmt in &txn.body {
                infer_statement(stmt, &mut ctx)?;
            }
            Ok(())
        }
        // 2026-07-29: Validate AsmFn declaration
        TopLevel::AsmFn(asm_fn) => {
            if asm_fn.target.is_empty() {
                return Err(crate::errors::TypeError::InvalidOperation {
                    operation: "asm declaration".into(),
                    type_name: "empty target".into(),
                });
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

/// 2026-07-26: Compute byte size of a type for view-cast validation.
fn type_byte_size(ty: &Type) -> u64 {
    match ty {
        Type::Vector(inner, dims) => {
            let elem_size = type_byte_size(inner);
            let count: u64 = dims.iter().map(|d| match d {
                crate::ast::Dimension::Anonymous(n) => *n as u64,
                _ => 1,
            }).product();
            elem_size * count
        }
        Type::Custom(s) if s == "Int" || s == "Int64" || s == "Ptr" => 8,
        Type::Custom(s) if s == "Int32" || s == "Float" => 4,
        Type::Custom(s) if s == "Int16" => 2,
        Type::Custom(s) if s == "Byte" || s == "Bool" || s == "Int8" => 1,
        Type::Custom(s) if s == "Float64" => 8,
        Type::Ptr(_) => 8,
        _ => 8, // default for unknown types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(src: &str) -> Result<(), Vec<TypeError>> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&items, &universe)
    }

    /// `Int * Float` is a type error — no implicit numeric coercion.
    #[test]
    fn mixed_int_float_arithmetic_errors() {
        let src = r#"
let x: Float = 0.0;
let count: Int = 0;
node t [count < 5][count == 5] {
    x = x + (count * 0.5);
    count = count + 1;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("no implicit Int/Float coercion")),
            "expected an implicit-coercion error, got {:?}",
            err
        );
    }

    /// An explicit `as Float` cast resolves the mixed operation.
    #[test]
    fn explicit_cast_resolves_mixed_arithmetic() {
        let src = r#"
let x: Float = 0.0;
let count: Int = 0;
node t [count < 5][count == 5] {
    x = x + (count as Float) * 0.5;
    count = count + 1;
    term;
};
"#;
        assert!(check(src).is_ok(), "explicit cast must typecheck");
    }

    /// A custom type declaring `op Mul(#Int)` authorizes `Int * MyType`
    /// without a cast (a cross-protocol overload).
    #[test]
    fn cross_type_overload_allows_mixed_arithmetic() {
        let src = r#"
type MyNum : #Int {
    op Mul(#Int): func(#L, #R);
};
let count: Int = 0;
let v: MyNum = 0;
node t [count < 5][count == 5] {
    count = count * v;
    term;
};
"#;
        assert!(check(src).is_ok(), "cross-type op overload must authorize Int * MyNum");
    }

    /// Without the cross-type overload, `Int * MyNum` errors.
    #[test]
    fn missing_cross_type_overload_errors() {
        let src = r#"
type MyNum : #Int {
    op Sub(#Int): func(#L, #R);
};
let count: Int = 0;
let v: MyNum = 0;
node t [count < 5][count == 5] {
    count = count * v;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("no implicit Int/Float coercion")),
            "expected an implicit-coercion error, got {:?}",
            err
        );
    }
}
