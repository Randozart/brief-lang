// ── Type Checker ───────────────────────────────────────────────────────
// 2026-07-12: Phase 2.4 — Expression/statement type inference.
// The main type-checking pass: infers types for all expressions and
// validates that operations are well-typed.
//
// Key responsibilities:
// - Expr::Call with # suffix → look up get_intrinsic_signature()
// - BinaryOp/UnaryOp → resolve op via declared ops → protocol bindings
//   (2026-08-03: protocol-category resolution, never type-name matching)
// - Literals → validate formatting compatibility
// - Variables → look up in scope

mod validate;
pub use validate::*;

use crate::analysis::provenance::Provenance;
use crate::ast::*;
use crate::errors::{SyntaxError, TypeError};
use crate::intrinsic_signatures::{get_intrinsic_signature, ReturnKind, Signature};
use crate::type_universe::{
    get_operator_intrinsic, protocol_binding, rune_to_op_name, TypeUniverse,
};
use std::collections::HashMap;

/// Type-check context: variable bindings and type universe.
pub struct TypecheckContext<'a> {
    pub bindings: HashMap<String, Type>,
    /// 2026-07-18: Names of state-level bindings (top-level let, state fields,
    /// txn parameters). Used by is_mutable_location to distinguish mutable
    /// state fields from immutable let-bindings for PtrConst inference.
    pub state_keys: std::collections::HashSet<String>,
    /// 2026-08-01 (Phase 3): locals consumed by a `~op` (`a ~= b`, `dest ~<-
    /// src`, `~<- src;`). Reading a consumed local afterward is a use-after-move
    /// compile error; reassigning it (via `=` or `let`) clears the mark.
    pub consumed_locals: std::collections::HashSet<String>,
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
    /// 2026-07-31: Struct/obj field slots, keyed by type name. Used to
    /// typecheck `Expr::Field` (`p.name`) and to resolve the receiver type
    /// for `Expr::MethodCall`.
    type_slots: HashMap<String, Vec<crate::ast::top::TypeDefSlot>>,
    /// 2026-07-31: obj member declarations (txn/defn), keyed by type name.
    /// Populated from obj bodies in check_program; used by MethodCall
    /// resolution (self-parameterized member dispatch).
    type_members: HashMap<String, Vec<crate::ast::top::TopLevel>>,
    /// 2026-07-31: obj declared type-parameter names, keyed by type name
    /// (e.g. `Stack` → ["T", "N"]). Used to substitute the receiver's concrete
    /// type args into generic member signatures at call sites.
    type_params: HashMap<String, Vec<String>>,
    /// 2026-07-31: User function/txn parameter types, keyed by name. Used to
    /// validate call arguments (Phase 2 — re-establish type validation).
    fn_param_types: HashMap<String, Vec<Type>>,
    /// 2026-07-31: The declared output type of the defn/txn currently being
    /// checked. Used to validate `term`/`term!` values.
    current_output_type: Option<Type>,
    /// 2026-07-31: Declared protocol hashwords, keyed by type name
    /// (`type MyNum : #Int` → "MyNum" → "#Int"). Used to grant numeric-protocol
    /// members literal construction.
    type_protocols: HashMap<String, String>,
    regular_bindings: HashMap<String, Vec<crate::ast::top::OperatorBinding>>,
    /// 2026-08-03 (P1.4): cross-variant op overrides from `proto` declarations
    /// (`proto C_String: #String { op Concat(#String) = cstring_concat(#L,#R) }`).
    /// Variant name → op name (e.g. "Add"/"Concat") → binding fn name. An op on
    /// a sub-protocol value prefers its variant's own op (zero cast) — "adopt
    /// whatever operations are most convenient."
    variant_cross_ops: HashMap<String, HashMap<String, String>>,
}

impl<'a> TypecheckContext<'a> {
    pub fn new(universe: &'a TypeUniverse) -> Self {
        TypecheckContext {
            bindings: HashMap::new(),
            state_keys: std::collections::HashSet::new(),
            consumed_locals: std::collections::HashSet::new(),
            universe,
            parse_ops: HashMap::new(),
            type_parents: HashMap::new(),
            fn_return_types: HashMap::new(),
            regular_ops: HashMap::new(),
            regular_bindings: HashMap::new(),
            type_slots: HashMap::new(),
            type_members: HashMap::new(),
            type_params: HashMap::new(),
            fn_param_types: HashMap::new(),
            current_output_type: None,
            type_protocols: HashMap::new(),
            variant_cross_ops: HashMap::new(),
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
    /// 2026-08-03 (operator-resolution fix): walks the `type_parents` chain, so
    /// an op declared on a parent type is inherited by its subtype (mirroring
    /// the Parse-op parent walk).
    fn type_declares_op(&self, ty: &Type, op_name: &str, operand: &Type) -> bool {
        let Some(current) = (match ty {
            Type::Custom(n) => Some(n.as_str()),
            Type::Applied(n, _) => Some(n.as_str()),
            _ => None,
        }) else {
            return false;
        };
        let mut current = current;
        loop {
            if let Some(ops) = self.regular_ops.get(current) {
                if ops.iter().any(|op| {
                    op.op == op_name
                        && op.params.first().map_or(false, |p| self.param_covers(p, operand))
                }) {
                    return true;
                }
            }
            if let Some(bindings) = self.regular_bindings.get(current) {
                if bindings.iter().any(|b| {
                    b.name == op_name && b.protocol_variant.as_ref().map_or(false, |v| self.variant_covers(v, operand))
                }) {
                    return true;
                }
            }
            match self.type_parents.get(current) {
                Some(parent) => current = parent.as_str(),
                None => return false,
            }
        }
    }

    /// Resolve the protocol hashword a custom type declares (`MyNum : #Int` →
    /// `"#Int"`), walking the `type_parents` chain until a declaration is
    /// found. None for primordials and protocol-less types. 2026-08-03: the
    /// operator-resolution fix — a subtype inherits its parent's protocol.
    fn declared_protocol_of(&self, type_name: &str) -> Option<&str> {
        let mut current = type_name;
        loop {
            if let Some(p) = self.type_protocols.get(current) {
                return Some(p.as_str());
            }
            match self.type_parents.get(current) {
                Some(parent) => current = parent.as_str(),
                None => return None,
            }
        }
    }

    /// 2026-08-03 (P1.4): the variant of a declared protocol string —
    /// `#String<C_String>` → `Some("C_String")`, `#String` → `None`.
    fn protocol_variant_of(proto: &str) -> Option<&str> {
        let b = proto.trim_start_matches('#');
        let lt = b.find('<')?;
        let variant = b[lt + 1..].trim_end_matches('>');
        if variant.is_empty() { None } else { Some(variant) }
    }

    /// The bare category of a declared protocol string —
    /// `#String<C_String>` → `"String"`, `#String` → `"String"`.
    fn protocol_category_of(proto: &str) -> &str {
        let b = proto.trim_start_matches('#');
        match b.find('<') {
            Some(lt) => &b[..lt],
            None => b,
        }
    }

    /// Protocol binding for a custom type via its declared protocol (own +
    /// parents). `MyNum : #Int` inherits `#Int`'s Add → `AddI64#`. Returns
    /// None for primordials (handled by `get_operator_intrinsic`) and
    /// protocol-less types. 2026-08-03: keyed by the bare protocol CATEGORY —
    /// never by type name.
    fn protocol_binding_for(&self, rune: &str, ty: &Type) -> Option<OpBinding> {
        let op_name = rune_to_op_name(rune)?;
        let name = match ty {
            Type::Custom(n) => n.as_str(),
            Type::Applied(n, _) => n.as_str(),
            _ => return None,
        };
        let proto = self.declared_protocol_of(name)?;
        // 2026-08-03: `+` is string concat for #String/#Data operands — resolve
        // the Concat binding (and the variant's Concat cross-op) for "+".
        let category = Self::protocol_category_of(proto);
        let effective_op = if op_name == "Add" && (category == "String" || category == "Data") {
            "Concat"
        } else {
            op_name
        };
        // 2026-08-03 (P1.4): a sub-protocol value (e.g. CStr: #String<C_String>)
        // prefers its VARIANT's own cross-op override (zero cast) over the base
        // binding. This is "adopt whatever operations are most convenient."
        if let Some(variant) = Self::protocol_variant_of(proto) {
            if let Some(fn_name) = self.variant_cross_ops.get(variant)
                .and_then(|ops| ops.get(effective_op))
            {
                return Some(OpBinding::Function(fn_name.clone()));
            }
        }
        // 2026-08-03: strip a `<variant>` suffix so a #String<C_String> type
        // resolves the base #String protocol binding (Concat, Extract, ...).
        protocol_binding(Self::protocol_category_of(proto), effective_op)
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
        self.operand_implements_protocol(operand, hw)
    }

    /// Does a type-body op's declared variant (`#Float` or `Float`) cover the
    /// operand type?
    fn variant_covers(&self, variant: &str, operand: &Type) -> bool {
        if variant.starts_with('#') {
            self.operand_implements_protocol(operand, variant)
        } else {
            // Concrete type name.
            match operand {
                Type::Custom(n) => n == variant,
                Type::Applied(n, _) => n == variant,
                _ => false,
            }
        }
    }

    /// Does `operand` implement protocol `hw` (e.g. `#Int`)? Checks the
    /// universe's `Cast.#` properties (primordials) AND the typechecker's own
    /// `type_protocols`/`type_parents` records (custom types — `MyNum : #Int`
    /// is not in the typechecker's fresh universe). 2026-08-03: protocol
    /// membership, never type-name matching.
    fn operand_implements_protocol(&self, operand: &Type, hw: &str) -> bool {
        // Universe membership (registered primordials + registered types).
        let prop = format!("Cast.{}", hw);
        if operand
            .universe_key()
            .and_then(|k| self.universe.get(k))
            .map_or(false, |rt| rt.properties.contains_key(&prop))
        {
            return true;
        }
        // Typechecker record: the type (or a parent) declares the protocol.
        let name = match operand {
            Type::Custom(n) => n.as_str(),
            Type::Applied(n, _) => n.as_str(),
            _ => return false,
        };
        self.declared_protocol_of(name) == Some(hw)
    }

    /// 2026-07-31 (A6): For `&collection <- value`, find the collection's
    /// InsertAt op binding's member first-parameter type (the element type).
    pub fn push_element_type(&self, collection: &Expr) -> Option<Type> {
        let Expr::Identifier(name) = collection else { return None; };
        let (type_name, args) = match self.bindings.get(name)? {
            Type::Custom(n) => (n.clone(), Vec::new()),
            Type::Applied(n, a) => (n.clone(), a.clone()),
            _ => return None,
        };
        let bindings = self.regular_bindings.get(&type_name)?;
        let binding = bindings.iter().find(|b| b.name == "InsertAt")?;
        let fn_name = match &binding.expr {
            Expr::Call(name, _, _) => name.clone(),
            _ => return None,
        };
        let members = self.type_members.get(&type_name)?;
        let member = members.iter().find(|m| member_name(m) == fn_name)?;
        let elem = member_params(member).into_iter().next()?;
        // 2026-08-01 (Phase 3): substitute the collection's concrete type args
        // into the generic member param (`List<Int>` InsertAt's `T` → `Int`),
        // so `queue <- count` on a List<Int> checks Int, not the bare `T`.
        let params = self.type_params.get(&type_name).cloned().unwrap_or_default();
        Some(substitute_type_params(&elem, &params, &args))
    }

    /// 2026-08-01 (Phase 3): the arrow READ/EXTRACT dispatch — a `dest <- src`
    /// where `src` is a collection with an `ExtractFrom`/`CopyFrom` op binding.
    /// Returns the member's RETURN type (what reading/extracting from the
    /// collection produces), mirroring `push_element_type` (InsertAt → param).
    pub fn extract_element_type(&self, collection: &Expr) -> Option<Type> {
        let Expr::Identifier(name) = collection else { return None; };
        let (type_name, args) = match self.bindings.get(name)? {
            Type::Custom(n) => (n.clone(), Vec::new()),
            Type::Applied(n, a) => (n.clone(), a.clone()),
            _ => return None,
        };
        let bindings = self.regular_bindings.get(&type_name)?;
        let binding = bindings
            .iter()
            .find(|b| b.name == "ExtractFrom" || b.name == "CopyFrom")?;
        let fn_name = match &binding.expr {
            Expr::Call(name, _, _) => name.clone(),
            _ => return None,
        };
        let members = self.type_members.get(&type_name)?;
        let member = members.iter().find(|m| member_name(m) == fn_name)?;
        let out = member_output(member)?;
        // 2026-08-01 (Phase 3): substitute concrete args into the generic
        // return (`Stack<Int>` pop → `T` → `Int`).
        let params = self.type_params.get(&type_name).cloned().unwrap_or_default();
        Some(substitute_type_params(&out, &params, &args))
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
        Expr::Char(_) => Ok((Type::Custom("Char".to_string()), Provenance::Unknown)),
        Expr::Float(_) => Ok((Type::float(), Provenance::Unknown)),
        Expr::Bool(_) => Ok((Type::bool_(), Provenance::Unknown)),
        Expr::Quoted(_) | Expr::TaggedQuotedLiteral(_, _) => Ok((Type::string(), Provenance::Unknown)),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => {
            // 2026-08-01 (Phase 4): a stream symbol is a compiler-known value
            // (e.g. `#StdIn` as a stream handle); it is never a user variable.
            if let Some(stream_ty) = stream_symbol_type(name) {
                return Ok((stream_ty, Provenance::Unknown));
            }
            // 2026-08-01 (Phase 3): use-after-move — a local consumed by a
            // `~op` is dead until reassigned.
            if ctx.consumed_locals.contains(name) {
                return Err(TypeError::InvalidOperation {
                    operation: "read of a consumed value".into(),
                    type_name: name.clone(),
                });
            }
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
        // 2026-07-31: Field access: p.name → the receiver's struct slot type.
        Expr::Field(obj, name) => {
            let (obj_ty, obj_prov) = infer_expression(obj, ctx)?;
            let field_ty = resolve_field_type(&obj_ty, name, ctx).ok_or_else(|| {
                TypeError::InvalidOperation {
                    operation: format!("field access '.{}'", name),
                    type_name: format!("{}", obj_ty),
                }
            })?;
            Ok((
                field_ty,
                Provenance::FieldAccess {
                    base: Box::new(obj_prov),
                    field: name.clone(),
                },
            ))
        }
        Expr::Index(obj, index) => {
            // 2026-07-31 (A4): indexing a Vector resolves the element type
            // (`f[0]` where f: Float[16] is Float, not Int).
            let (obj_ty, obj_prov) = infer_expression(obj, ctx)?;
            let (_, idx_prov) = infer_expression(index, ctx)?;
            let elem_ty = match &obj_ty {
                Type::Vector(inner, _) => (**inner).clone(),
                Type::Ptr(inner) | Type::PtrConst(inner) => (**inner).clone(),
                Type::Custom(n) if n == "String" => Type::int(),
                _ => Type::int(),
            };
            Ok((
                elem_ty,
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
        // 2026-07-31: Struct literal — `TypeName { field: e, ... }` resolves
        // to the struct type and validates the fields against its slots.
        Expr::StructLiteral { type_name, fields } => {
            // 2026-08-01 (D3): a struct literal for a GENERIC type infers as
            // the Applied form with the type params as placeholders
            // (`HashMapEntry { key, val }` in a HashMap<K, V> member → 
            // `HashMapEntry<K, V>`), so it matches a member's param type.
            let ty = match ctx.type_params.get(type_name) {
                Some(params) if !params.is_empty() => Type::Applied(
                    type_name.clone(),
                    params.iter().map(|p| Type::Custom(p.clone())).collect(),
                ),
                _ => Type::Custom(type_name.clone()),
            };
            let slots = ctx.type_slots.get(type_name).cloned().unwrap_or_default();
            // 2026-08-01 (D3): a generic struct literal (`ListBuffer<Int> { ... }`)
            // — the slot types reference type params (data: Ptr<T>). The strict
            // field check is deferred to the let-level declared-type coercion
            // (the concrete T→Int substitution); here we only validate the
            // field names exist. Collect the struct's type-param names.
            let type_param_names: Vec<String> = ctx.type_params.get(type_name)
                .cloned()
                .unwrap_or_default();
            let contains_type_param = |t: &Type| {
                fn walk(t: &Type, params: &[String]) -> bool {
                    match t {
                        Type::Custom(n) => params.iter().any(|p| p == n),
                        Type::Ptr(i) | Type::PtrConst(i) => walk(i, params),
                        Type::Vector(i, _) => walk(i, params),
                        Type::Applied(_, args) => args.iter().any(|a| walk(a, params)),
                        Type::Tuple(elems) => elems.iter().any(|e| walk(e, params)),
                        _ => false,
                    }
                }
                walk(t, &type_param_names)
            };
            let mut resolved = Vec::new();
            for (fname, fval) in fields {
                let fty = slots
                    .iter()
                    .find(|s| s.name == *fname)
                    .map(|s| s.ty.clone());
                let vty = infer_type_only(fval, ctx)?;
                if let Some(ft) = &fty {
                    if vty != *ft && !contains_type_param(ft) {
                        let coercible = try_coerce_via_parse(fval, &vty, ft, ctx);
                        if !coercible {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{}", ft),
                                found: format!("{}", vty),
                                context: format!("field '{}' of struct literal '{}'", fname, type_name),
                            });
                        }
                    }
                }
                resolved.push(());
            }
            let _ = resolved;
            Ok((ty, Provenance::Unknown))
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
        // 2026-08-01 (Phase 3): a consumed operand is read-then-destroyed.
        // Only a mutable lvalue (a variable, state field, or collection) can
        // be consumed — a literal/constant is a compile error — and consuming
        // marks the local dead (use-after-move is caught by the Identifier arm).
        Expr::Consume(inner) => {
            match inner.as_ref() {
                Expr::Decimal(_) | Expr::TaggedLiteral(_, _) | Expr::Float(_)
                | Expr::Bool(_) | Expr::Char(_)
                | Expr::Quoted(_) | Expr::TaggedQuotedLiteral(_, _) => {
                    return Err(TypeError::InvalidOperation {
                        operation: "cannot consume a constant".into(),
                        type_name: format!("{}", inner),
                    });
                }
                Expr::Identifier(name) => {
                    if !ctx.is_mutable_location(name) {
                        return Err(TypeError::InvalidOperation {
                            operation: "cannot consume a constant".into(),
                            type_name: name.clone(),
                        });
                    }
                    if ctx.consumed_locals.contains(name) {
                        return Err(TypeError::InvalidOperation {
                            operation: "consume of an already-consumed value".into(),
                            type_name: name.clone(),
                        });
                    }
                    // Infer the inner value FIRST (it must not already be
                    // consumed), then mark the local dead.
                    let ty = infer_expression(inner, ctx)?;
                    ctx.consumed_locals.insert(name.clone());
                    return Ok(ty);
                }
                _ => {}
            }
            infer_expression(inner, ctx)
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
        // 2026-07-31: Reflection: x.^Len / x.^^Size (see resolve_reflect).
        Expr::Reflect(recv, target, kind) => {
            let (recv_ty, recv_prov) = infer_expression(recv, ctx)?;
            let result_ty = resolve_reflect(&recv_ty, target, *kind)?;
            Ok((result_ty, recv_prov))
        }
        // 2026-07-31: Method call: a.m(args) — resolves the member on the
        // receiver's obj type, binds the receiver as the implicit `self`, and
        // validates the args against the member's (type-arg-substituted)
        // parameter list.
        Expr::MethodCall(recv, name, args, _) => {
            let (recv_ty, recv_prov) = infer_expression(recv, ctx)?;
            let result_ty = resolve_method_call(&recv_ty, name, args, ctx)?;
            Ok((result_ty, recv_prov))
        }
        Expr::FormattingAnnotation(_) => Ok((Type::void(), Provenance::Unknown)),
        Expr::Match(expr, arms) => infer_match(expr, arms, ctx).map(|ty| (ty, Provenance::Unknown)),
        // 2026-07-19: Plugin-intercept calls are resolved by Front or Mid
        // stage plugins. `briefc check` does not run plugins, so known
        // env-variable intercepts are typed here (they desugar to stdlib
        // `get_env`/`get_env_int` calls in the build path).
        // 2026-08-01: Phase 1 of the plugin-macro rework — only lowercase
        // macro names are recognized. PascalCase legacy names (`PrintLn!`,
        // `GetEnvInt!`, ...) are rejected with a rename hint.
        Expr::PluginIntercept { name, args, .. } => {
            for a in args {
                infer_type_only(a, ctx)?;
            }
            match name.as_str() {
                "get_env_int" => Ok((Type::int(), Provenance::Unknown)),
                "get_env" | "get_env_or_default" => Ok((Type::string(), Provenance::Unknown)),
                "print" | "println" => Ok((Type::void(), Provenance::Unknown)),
                "PrintLn" | "Print" => Err(TypeError::InvalidOperation {
                    operation: format!("plugin-intercept '{}!'", name),
                    type_name: "the lowercase macros 'println!' and 'print!' replaced 'PrintLn!' and 'Print!' — rename the call site".into(),
                }),
                "GetEnvInt" | "GetEnv" | "GetEnvOrDefault" => Err(TypeError::InvalidOperation {
                    operation: format!("plugin-intercept '{}!'", name),
                    type_name: "the lowercase macros 'get_env_int!', 'get_env!', and 'get_env_or_default!' replaced the PascalCase names — rename the call site".into(),
                }),
                _ => Err(TypeError::InvalidOperation {
                    operation: format!("plugin-intercept '{}!'", name),
                    type_name: "unresolved plugin-intercept reached the typechecker".into(),
                }),
            }
        }
        Expr::Exists(name) => Err(TypeError::InvalidOperation {
            operation: format!("compile-time existence check '{}'", name),
            type_name: "Existence checks resolve during macro evaluation, not type inference".into(),
        }),
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
        // 2026-08-01: `-3.25` is `Neg(Float)` — a negative numeric literal, so
        // it is a "Decimal" form and can coerce (e.g. to Float64) like a
        // positive literal. Without this, `let d: Float64 = -3.25` errored.
        Expr::UnaryOp(crate::ast::UnaryOpKind::Neg, inner) => {
            match inner.as_ref() {
                Expr::Float(_) | Expr::Decimal(_) | Expr::TaggedLiteral(_, _) => ("Decimal", None),
                _ => return false,
            }
        }
        _ => return (false),
    };
    let target_name = match target_ty {
        Type::Custom(n) => n.as_str(),
        // 2026-07-31 (Phase 2): Applied types use their base name for op
        // lookup (`RingBuffer<Int>` → `RingBuffer`).
        Type::Applied(n, _) => n.as_str(),
        _ => return false,
    };
    if ctx.find_parse_op(target_name, form, discriminator).is_some() {
        return true;
    }
    // 2026-07-31 (Phase 2): `op Init: init(#L, #R)` authorizes `let t: T = v`
    // construction (the collection stdlib pattern).
    let has_init = ctx
        .regular_bindings
        .get(target_name)
        .map_or(false, |b| b.iter().any(|op| op.name == "Init"));
    if has_init {
        return true;
    }
    // 2026-07-31 (Phase 2): Numeric-protocol members construct from numeric
    // literals even without an explicit Parse op (`let v: MyNum = 0` where
    // `type MyNum : #Int`). A type is numeric if it carries Cast.#Int,
    // Cast.#UInt, or Cast.#Float.
    if matches!(form, "Decimal") {
        let numeric = ["Cast.#Int", "Cast.#UInt", "Cast.#Float"];
        let is_numeric = target_ty
            .universe_key()
            .and_then(|k| ctx.universe.get(k))
            .map_or(false, |rt| numeric.iter().any(|p| rt.properties.contains_key(*p)));
        // Also honor a declared numeric protocol hashword (`type MyNum : #Int`).
        let proto_numeric = ctx
            .type_protocols
            .get(target_name)
            .map_or(false, |p| matches!(p.as_str(), "#Int" | "#UInt" | "#Float"));
        if is_numeric || proto_numeric {
            return true;
        }
    }
    false
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

    // User function call — validate args against param types, then return type.
    // 2026-07-31 (Phase 2): call arguments must match the callee's parameter
    // types — no implicit coercion (literal Parse-ops excepted).
    let param_types = ctx.fn_param_types.get(name).cloned().unwrap_or_default();
    for (i, arg) in args.iter().enumerate() {
        let arg_ty = infer_type_only(arg, ctx)?;
        if let Some(param_ty) = param_types.get(i) {
            if arg_ty != *param_ty {
                let coercible = try_coerce_via_parse(arg, &arg_ty, param_ty, ctx);
                if !coercible {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{}", param_ty),
                        found: format!("{}", arg_ty),
                        context: format!("argument {} of '{}'", i, name),
                    });
                }
            }
        }
    }
    // 2026-07-31 (Phase 2): Struct/obj constructor call — Person(args) → Person.
    // Validates the args against the type's slots (positional order).
    if ctx.type_slots.contains_key(name) {
        let slots = ctx.type_slots.get(name).cloned().unwrap_or_default();
        for (i, arg) in args.iter().enumerate() {
            let arg_ty = infer_type_only(arg, ctx)?;
            if let Some(slot) = slots.get(i) {
                if arg_ty != slot.ty {
                    let coercible = try_coerce_via_parse(arg, &arg_ty, &slot.ty, ctx);
                    if !coercible {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{}", slot.ty),
                            found: format!("{}", arg_ty),
                            context: format!("field {} of constructor '{}'", i, name),
                        });
                    }
                }
            }
        }
        return Ok(Type::Custom(name.to_string()));
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
            // 2026-08-03: CallPtr#(cb, args...) returns the cb's fn RETURN
            // type (e.g. fn(Int) -> Int → Int), not the fn value itself.
            if sig.name == "CallPtr#" {
                if let Some(Ok(Type::Function(_, ret))) = args.first().map(|a| infer_type_only(a, ctx)) {
                    return Ok(*ret);
                }
                return Ok(Type::int());
            }
            args.first()
                .map(|a| infer_type_only(a, ctx))
                .unwrap_or(Ok(Type::int()))?
        }
        ReturnKind::Exact(t) => t.clone(),
        _ => Type::int(), // fallback for unknown Native kinds
    })
}

/// Resolve the result type for an arithmetic/bitwise binary op. Returns the
/// LHS type when the op resolves (declared → protocol bindings), else an
/// InvalidOperation error. 2026-08-03: extracted from infer_binary_op so the
/// resolution chain stays flat (Praetor complexity gate).
fn arithmetic_result_ty(
    ctx: &TypecheckContext,
    kind: &BinaryOpKind,
    lhs_ty: &Type,
    rhs_ty: &Type,
    lhs_str: &str,
    rune: &str,
) -> Result<Type, TypeError> {
    if ctx.has_cross_type_overload(rune, lhs_ty, rhs_ty) {
        return Ok(lhs_ty.clone());
    }
    let binding = ctx
        .protocol_binding_for(rune, lhs_ty)
        .or_else(|| get_operator_intrinsic(ctx.universe, rune, lhs_ty))
        .or_else(|| ctx.protocol_binding_for(rune, rhs_ty))
        .or_else(|| get_operator_intrinsic(ctx.universe, rune, rhs_ty));
    match binding {
        Some(_) => Ok(lhs_ty.clone()),
        None => Err(TypeError::InvalidOperation {
            operation: format!("'{}'", kind),
            type_name: lhs_str.to_string(),
        }),
    }
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
            // 2026-08-03 (operator-resolution fix): resolution order is
            //   declared (own + parents) → protocol bindings.
            // A type declaring `op Add(#Float)`/`op Add(Float)` authorizes
            // mixed arithmetic; a custom `MyNum : #Int` with no declared op
            // inherits #Int's protocol binding (Add → AddI64#). Only the
            // protocol bindings are hardcoded — keyed by category, never by
            // type name. Same-type custom ops now resolve here too.
            arithmetic_result_ty(ctx, kind, &lhs_ty, &rhs_ty, &lhs_str, &rune)?
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
            // 2026-08-01 (Phase 3): a `let` rebinding revives a consumed name.
            if names.len() == 1 {
                ctx.consumed_locals.remove(&names[0]);
            }
            let inferred = match expr {
                Some(e) => infer_type_only(e, ctx)?,
                None => ty.clone().unwrap_or(Type::int()),
            };
            // 2026-07-31 (Phase 2): A declared type must match the inferred
            // initializer type — no implicit coercion. Literal Parse-ops
            // (`let f: Float = 5`) remain the one sanctioned path.
            if let Some(declared) = ty {
                let compatible = if inferred == *declared {
                    true
                } else {
                    // 2026-07-31: `let lb: ListBuffer<Int> = ListBuffer {...}`
                    // — a generic-struct constructor infers the bare type; the
                    // declared type application pins the type params.
                    match (declared, &inferred) {
                        (Type::Applied(dn, _), Type::Custom(inm)) => dn == inm,
                        // 2026-08-01 (D3): a generic struct literal now infers
                        // as the Applied form with type-param placeholders
                        // (`HashMapEntry<K, V>`, `ListBuffer<T>`) — same base
                        // accepts; the declared type's concrete args are pinned
                        // by the mono instantiation.
                        (Type::Applied(dn, _), Type::Applied(inm, _)) => dn == inm,
                        _ => false,
                    }
                };
                if !compatible {
                    let coercible = expr.as_ref().map_or(false, |e| {
                        try_coerce_via_parse(e, &inferred, declared, ctx)
                    });
                    if !coercible {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{}", declared),
                            found: format!("{}", inferred),
                            context: format!("let '{}'", name),
                        });
                    }
                }
            }
            let resolved = ty.clone().unwrap_or(inferred);
            ctx.bindings.insert(name.clone(), resolved);
            Ok(())
        }
        Statement::Assign(lhs, rhs) => {
            // 2026-08-01 (Phase 3): the rhs reads the OLD value (a
            // use-after-free/use-after-move is caught here), then the target
            // is REVIVED so the lhs (a reassignment) is legal — `b = 5` makes
            // a later `b` usable, while `b = b + 1` after a consume of b is an
            // error (the rhs reads a dead local).
            let rhs_ty = infer_type_only(rhs, ctx)?;
            if let Expr::Identifier(target_name) = lhs {
                ctx.consumed_locals.remove(target_name);
            }
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
            let lhs_ty = infer_expression(lhs, ctx).map(|(t, _)| t)?;
            // 2026-07-31 (A6): `&collection <- value` — the `<-` PUSH. The
            // LHS is an AddrOf of a collection with an InsertAt op binding;
            // the RHS must match the member's first parameter type (not the
            // Ptr<Collection> type a plain assignment would demand).
            if let Expr::AddrOf(inner) = lhs {
                if let Some(elem_ty) = ctx.push_element_type(inner) {
                    if rhs_ty != elem_ty {
                        let coercible = try_coerce_via_parse(rhs, &rhs_ty, &elem_ty, ctx);
                        if !coercible {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{}", elem_ty),
                                found: format!("{}", rhs_ty),
                                context: "push '<- value' into collection".into(),
                            });
                        }
                    }
                    return Ok(());
                }
            }
            // 2026-07-31 (A2): assignment must preserve the LHS type — no
            // implicit coercion (`len = "hello"` where len: Int errors).
            if lhs_ty != rhs_ty {
                let coercible = try_coerce_via_parse(rhs, &rhs_ty, &lhs_ty, ctx);
                if !coercible {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{}", lhs_ty),
                        found: format!("{}", rhs_ty),
                        context: "assignment".into(),
                    });
                }
            }
            Ok(())
        }
        Statement::ArrowAssign { target, value, .. } => {
            // 2026-08-01 (Phase 3): the arrow — `target <- value` (copy into
            // lhs) / `target ~<- value` (destructive). The dispatch finds the
            // collection by the op binding on each side:
            //   - the TARGET has an InsertAt binding → INSERT (push): the value
            //     must match the element type;
            //   - the VALUE has an ExtractFrom/CopyFrom binding → READ/EXTRACT
            //     (pop): the target must match the member's return type;
            //   - the TARGET is a stream symbol (`#StdOut`/`#StdErr`) → a write;
            //   - otherwise a plain assignment (no implicit coercion).
            let value_ty = infer_type_only(value, ctx)?;
            if let Some(t) = target {
                // 2026-08-01 (Phase 4): a stream write — the target is a
                // compiler-known stream symbol. `#StdOut` accepts any value
                // (lowered to Print#); `#StdErr` accepts a String (lowered to
                // the stderr string printer).
                if let Expr::Identifier(name) = t.as_ref() {
                    if is_stream_symbol(name) {
                        if name == "#StdErr" && value_ty != Type::string() {
                            return Err(TypeError::TypeMismatch {
                                expected: "String".into(),
                                found: format!("{}", value_ty),
                                context: "stderr stream write ('#StdErr <- value')".into(),
                            });
                        }
                        return Ok(());
                    }
                }
                // A write to the target revives a consumed name.
                if let Expr::Identifier(target_name) = t.as_ref() {
                    ctx.consumed_locals.remove(target_name);
                }
                // INSERT: the value matches the InsertAt element type (or a
                // Parse coerce). When push_element_type is unresolved (a bare
                // generic like `T`), or the value doesn't match, fall through
                // to the extract/plain checks — matching the pre-arrow behavior
                // (the old `<-` was typechecked as a plain assignment, which
                // passed RingBuffer/List via their Parse ops).
                let mut arrow_ok = false;
                if let Some(elem_ty) = ctx.push_element_type(t) {
                    if value_ty == elem_ty || try_coerce_via_parse(value, &value_ty, &elem_ty, ctx) {
                        arrow_ok = true;
                    }
                }
                if !arrow_ok {
                    if let Some(elem_ty) = ctx.extract_element_type(value) {
                        // READ/EXTRACT: `dest <- queue` / `dest ~<- queue`.
                        // The value IS the collection (value_ty = Stack<T>); the
                        // target must accept the ExtractFrom/CopyFrom return type.
                        let target_ty = infer_type_only(t, ctx)?;
                        if target_ty != elem_ty {
                            let coercible = try_coerce_via_parse(value, &value_ty, &target_ty, ctx);
                            if !coercible {
                                return Err(TypeError::TypeMismatch {
                                    expected: format!("{}", target_ty),
                                    found: format!("{}", elem_ty),
                                    context: "arrow '<-' read into target".into(),
                                });
                            }
                        }
                    } else {
                        // Plain assignment — no implicit coercion.
                        let lhs_ty = infer_type_only(t, ctx)?;
                        if lhs_ty != value_ty {
                            let coercible = try_coerce_via_parse(value, &value_ty, &lhs_ty, ctx);
                            if !coercible {
                                return Err(TypeError::TypeMismatch {
                                    expected: format!("{}", lhs_ty),
                                    found: format!("{}", value_ty),
                                    context: "arrow assignment".into(),
                                });
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        Statement::FreeHint(name) => {
            // 2026-08-01 (Phase 5): `free x;` — the backing of `x` is freed
            // here (a VERIFIED contract). The local must exist, must be
            // mutable (a constant cannot be freed), and must not already be
            // dead; afterward a read of `x` is a use-after-free compile error.
            let _ty = ctx.bindings.get(name).cloned().ok_or_else(|| TypeError::UndefinedVariable {
                name: name.clone(),
                available: ctx.bindings.keys().cloned().collect(),
            })?;
            if !ctx.is_mutable_location(name) {
                return Err(TypeError::InvalidOperation {
                    operation: "cannot free a constant".into(),
                    type_name: name.clone(),
                });
            }
            if ctx.consumed_locals.contains(name) {
                return Err(TypeError::InvalidOperation {
                    operation: "free of an already-freed value".into(),
                    type_name: name.clone(),
                });
            }
            ctx.consumed_locals.insert(name.clone());
            Ok(())
        }
        Statement::KeepHint(name) => {
            // 2026-08-01 (Phase 5): `keep x;` — suppress the scheduler's
            // auto-free. No type-level effect; the field must exist.
            if !ctx.bindings.contains_key(name) {
                return Err(TypeError::UndefinedVariable {
                    name: name.clone(),
                    available: ctx.bindings.keys().cloned().collect(),
                });
            }
            Ok(())
        }
        Statement::Term(val) | Statement::TermBang(val) => {
            if let Some(val) = val {
                let vty = infer_type_only(val, ctx)?;
                // 2026-07-31 (Phase 2): a declared return type must match the
                // term value — no implicit coercion.
                if let Some(out) = &ctx.current_output_type {
                    if vty != *out {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{}", out),
                            found: format!("{}", vty),
                            context: "term value vs declared return type".into(),
                        });
                    }
                }
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
            // 2026-07-31 (Phase 2): frgn return types — `term frgn_foo(x)`
            // must see the declared foreign return type, not the Int fallback.
            TopLevel::ForeignBinding(fb) => {
                let brief_name = fb
                    .brief_name
                    .clone()
                    .unwrap_or_else(|| fb.foreign_name.clone());
                fb.success_output
                    .first()
                    .map(|(_, ty)| (brief_name, ty.clone()))
            }
            _ => None,
        }
    }).collect();

    // 2026-07-31: Pre-collect user function/txn parameter types for call-arg
    // validation (Phase 2 — re-establish type validation).
    let fn_param_types: HashMap<String, Vec<Type>> = items
        .iter()
        .filter_map(|item| {
            let (name, params) = match item {
                TopLevel::Definition(d) => (d.name.clone(), d.parameters.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>()),
                TopLevel::Transaction(t) => (t.name.clone(), t.parameters.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>()),
                TopLevel::Export(e) => match &*e.inner {
                    TopLevel::Definition(d) => (d.name.clone(), d.parameters.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>()),
                    _ => return None,
                },
                _ => return None,
            };
            Some((name, params))
        })
        .collect();

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
    // 2026-07-31: Pre-collect struct/obj field slots and obj member
    // declarations for Expr::Field / Expr::MethodCall resolution.
    let mut all_type_slots: HashMap<String, Vec<crate::ast::top::TypeDefSlot>> = HashMap::new();
    let mut all_type_members: HashMap<String, Vec<TopLevel>> = HashMap::new();
    let mut all_type_params: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_type_protocols: HashMap<String, String> = HashMap::new();
    // 2026-08-03 (P1.4): cross-variant op overrides from proto declarations —
    // variant name → op name → binding fn (e.g. C_String → Concat →
    // cstring_concat). An op on a sub-protocol value prefers its own variant's
    // op (zero cast), falling back to the base binding via a delta cast.
    let mut all_cross_ops: HashMap<String, HashMap<String, String>> = HashMap::new();
    for item in items {
        if let TopLevel::ProtocolDef(pd) = item {
            for op in &pd.cross_ops {
                let Some(fn_name) = cross_op_fn_name(&op.impl_args) else { continue };
                all_cross_ops
                    .entry(pd.name.clone())
                    .or_default()
                    .insert(op.op.clone(), fn_name);
            }
        }
        if let TopLevel::TypeDef(td) = item {
            if !td.body.operators.is_empty() {
                all_regular_ops.insert(td.name.clone(), td.body.operators.clone());
            }
            if !td.body.op_bindings.is_empty() {
                all_regular_bindings.insert(td.name.clone(), td.body.op_bindings.clone());
            }
            if !td.body.slots.is_empty() {
                all_type_slots.insert(td.name.clone(), td.body.slots.clone());
            }
            if !td.body.members.is_empty() {
                all_type_members.insert(td.name.clone(), td.body.members.clone());
            }
            if !td.type_params.is_empty() {
                all_type_params.insert(
                    td.name.clone(),
                    td.type_params.iter().map(|p| p.name.clone()).collect(),
                );
            }
            if let Some(proto) = &td.protocol {
                all_type_protocols.insert(td.name.clone(), proto.clone());
            }
        }
        // 2026-08-01 (D3): a generic `struct ListBuffer<T>` (StaticStruct) has
        // slots too — `inner.data` on a ListBuffer<T> field must resolve. Its
        // fields become TypeDefSlots so field access + monomorphization work.
        if let TopLevel::StaticStruct(sd) = item {
            let slots: Vec<crate::ast::top::TypeDefSlot> = sd.fields
                .iter()
                .map(|(n, ty)| crate::ast::top::TypeDefSlot {
                    name: n.clone(),
                    ty: ty.clone(),
                    bit_range: None,
                })
                .collect();
            if !slots.is_empty() {
                all_type_slots.insert(sd.name.clone(), slots);
            }
            if !sd.type_params.is_empty() {
                all_type_params.insert(
                    sd.name.clone(),
                    sd.type_params.iter().map(|p| p.name.clone()).collect(),
                );
            }
        }
    }

    for item in items {
        if let Err(e) = check_top_level(
            item, universe, &state_bindings, &fn_return_types, &fn_param_types,
            &all_parse_bindings, &all_type_parents, &all_regular_ops, &all_regular_bindings,
            &all_type_slots, &all_type_members, &all_type_params, &all_type_protocols,
            &all_cross_ops,
        ) {
            errors.push(e);
        }
    }

    // 2026-07-31 (A2): Typecheck obj member bodies with `self` + slot names
    // bound. Without this, `len = "hello"` inside a member passes silently.
    for item in items {
        if let TopLevel::TypeDef(td) = item {
            if td.body.members.is_empty() {
                continue;
            }
            let self_ty = if td.type_params.is_empty() {
                Type::Custom(td.name.clone())
            } else {
                Type::Applied(
                    td.name.clone(),
                    td.type_params.iter().map(|p| Type::Custom(p.name.clone())).collect(),
                )
            };
            for member in &td.body.members {
                let mut mctx = TypecheckContext::new(universe);
                mctx.type_parents = all_type_parents.clone();
                mctx.regular_ops = all_regular_ops.clone();
                mctx.regular_bindings = all_regular_bindings.clone();
                mctx.type_slots = all_type_slots.clone();
                mctx.type_members = all_type_members.clone();
                mctx.type_params = all_type_params.clone();
                mctx.fn_param_types = fn_param_types.clone();
                mctx.type_protocols = all_type_protocols.clone();
                for (name, ty) in &fn_return_types {
                    mctx.fn_return_types.insert(name.clone(), ty.clone());
                }
                for (name, ty) in &state_bindings {
                    mctx.bindings.insert(name.clone(), ty.clone());
                    mctx.state_keys.insert(name.clone());
                }
                mctx.bindings.insert("self".into(), self_ty.clone());
                for slot in &td.body.slots {
                    mctx.bindings.insert(slot.name.clone(), slot.ty.clone());
                    mctx.state_keys.insert(slot.name.clone());
                }
                match member {
                    TopLevel::Transaction(t) => {
                        for (n, ty) in &t.parameters {
                            mctx.bindings.insert(n.clone(), ty.clone());
                        }
                        mctx.current_output_type = t.output_type.as_ref().map(output_type_to_type);
                        for stmt in &t.body {
                            if let Err(e) = infer_statement(stmt, &mut mctx) {
                                errors.push(e);
                            }
                        }
                    }
                    TopLevel::Definition(d) => {
                        for (n, ty) in &d.parameters {
                            mctx.bindings.insert(n.clone(), ty.clone());
                        }
                        mctx.current_output_type = d.output_type.as_ref().map(output_type_to_type);
                        for stmt in &d.body {
                            if let Err(e) = infer_statement(stmt, &mut mctx) {
                                errors.push(e);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Type-check a top-level item.
/// 2026-08-03 (P1.4): extract the binding function name from a cross-op's
/// `impl_args` (`= cstring_concat(#L, #R)` → "cstring_concat"). Accepts a bare
/// identifier or a call list whose first element is the function name; other
/// shapes yield None (the cross-op is skipped, the base binding is used).
fn cross_op_fn_name(impl_args: &Option<PropertyValue>) -> Option<String> {
    match impl_args {
        Some(PropertyValue::Identifier(n)) => Some(n.clone()),
        Some(PropertyValue::List(items)) => match items.first() {
            Some(PropertyValue::Identifier(n)) => Some(n.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn check_top_level(
    item: &TopLevel,
    universe: &TypeUniverse,
    state_bindings: &HashMap<String, Type>,
    fn_return_types: &HashMap<String, Type>,
    fn_param_types: &HashMap<String, Vec<Type>>,
    all_parse_bindings: &HashMap<String, Vec<OperatorBinding>>,
    all_type_parents: &HashMap<String, String>,
    all_regular_ops: &HashMap<String, Vec<crate::ast::top::OperatorDef>>,
    all_regular_bindings: &HashMap<String, Vec<crate::ast::top::OperatorBinding>>,
    all_type_slots: &HashMap<String, Vec<crate::ast::top::TypeDefSlot>>,
    all_type_members: &HashMap<String, Vec<TopLevel>>,
    all_type_params: &HashMap<String, Vec<String>>,
    all_type_protocols: &HashMap<String, String>,
    all_cross_ops: &HashMap<String, HashMap<String, String>>,
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
    // 2026-07-31: Inject struct/obj slots and members for field/method access.
    ctx.type_slots = all_type_slots.clone();
    ctx.type_members = all_type_members.clone();
    ctx.type_params = all_type_params.clone();
    ctx.fn_param_types = fn_param_types.clone();
    ctx.type_protocols = all_type_protocols.clone();
    ctx.variant_cross_ops = all_cross_ops.clone();
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
    // 2026-08-01 (Phase 3): defn/obj-member parameters are mutable locations
    // too (reassignable within the body) — a `~op` can consume them.
    if let TopLevel::Definition(defn) = item {
        for (name, _) in &defn.parameters {
            ctx.state_keys.insert(name.clone());
        }
    }
    match item {
        TopLevel::Definition(defn) => {
            for (name, ty) in &defn.parameters {
                ctx.bindings.insert(name.clone(), ty.clone());
            }
            ctx.current_output_type = defn.output_type.as_ref().map(output_type_to_type);
            for stmt in &defn.body {
                infer_statement(stmt, &mut ctx)?;
            }
            Ok(())
        }
        // 2026-07-25: Unwrap exports so exported defns are type-checked.
        TopLevel::Export(e) => check_top_level(&e.inner, universe, state_bindings, fn_return_types, fn_param_types, all_parse_bindings, all_type_parents, all_regular_ops, all_regular_bindings, all_type_slots, all_type_members, all_type_params, all_type_protocols, all_cross_ops),
        TopLevel::Transaction(txn) => {
            for (name, ty) in &txn.parameters {
                ctx.bindings.insert(name.clone(), ty.clone());
            }
            ctx.current_output_type = txn.output_type.as_ref().map(output_type_to_type);
            for stmt in &txn.body {
                infer_statement(stmt, &mut ctx)?;
            }
            Ok(())
        }
        // 2026-07-31: Top-level `let` initializers are typechecked too (Phase 2).
        TopLevel::Statement(stmt) => {
            if let Statement::Let { .. } = stmt.as_ref() {
                return infer_statement(stmt, &mut ctx);
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

/// 2026-07-31: Resolve `p.name` on a struct/obj/tuple receiver.
/// Numeric field names (`.0`, `.1`) are tuple-element indices.
fn resolve_field_type(receiver: &Type, field: &str, ctx: &TypecheckContext) -> Option<Type> {
    if let Type::Tuple(elems) = receiver {
        return field
            .parse::<usize>()
            .ok()
            .and_then(|i| elems.get(i).cloned());
    }
    let type_name = match receiver {
        Type::Custom(n) => n.as_str(),
        Type::Applied(n, _) => n.as_str(),
        _ => return None,
    };
    let slots = ctx.type_slots.get(type_name)?;
    slots
        .iter()
        .find(|s| s.name == field)
        .map(|s| s.ty.clone())
}

/// 2026-07-31: Reflection table (D1). `^` = runtime, `^^` = compile-time.
/// A target used with the wrong kind is an error; an unknown target is an
/// error. `Len`/`Ptr` are runtime; `Size`/`Bytes`/`Alignment`/`Type` are
/// compile-time (foldable).
fn resolve_reflect(
    receiver: &Type,
    target: &str,
    kind: ReflectKind,
) -> Result<Type, TypeError> {
    let is_compile_time = matches!(kind, ReflectKind::CompileTime);
    let wrong_kind = |expected: &str| {
        TypeError::InvalidOperation {
            operation: format!("reflection target '{}'", target),
            type_name: format!(
                "{} is {expected}; use '.{}'",
                target,
                if expected == "runtime" { "^" } else { "^^" }
            ),
        }
    };
    match target {
        "Len" => {
            if is_compile_time {
                return Err(wrong_kind("runtime"));
            }
            // Len is meaningful on value-carrying types (String, vectors,
            // collections); scalars have no length.
            match receiver {
                Type::Custom(n) if n == "String" => Ok(Type::int()),
                Type::Vector(..) | Type::Applied(..) | Type::Custom(_) => Ok(Type::int()),
                _ => Err(TypeError::InvalidOperation {
                    operation: format!("reflection target 'Len'"),
                    type_name: format!("type {} has no runtime length", receiver),
                }),
            }
        }
        "Ptr" => {
            if is_compile_time {
                return Err(wrong_kind("runtime"));
            }
            Ok(Type::ptr(receiver.clone()))
        }
        "Absolute" => {
            // 2026-08-04: `x.^Absolute` — absolute value. Valid on Int/Float.
            if is_compile_time {
                return Err(wrong_kind("runtime"));
            }
            Ok(receiver.clone())
        }
        "Size" => {
            if !is_compile_time {
                return Err(wrong_kind("compile-time"));
            }
            Ok(Type::int())
        }
        "Bytes" => {
            if !is_compile_time {
                return Err(wrong_kind("compile-time"));
            }
            Ok(Type::int())
        }
        "Alignment" => {
            if !is_compile_time {
                return Err(wrong_kind("compile-time"));
            }
            Ok(Type::int())
        }
        "Type" => {
            if !is_compile_time {
                return Err(wrong_kind("compile-time"));
            }
            Ok(Type::Custom("Type".into()))
        }
        _ => Err(TypeError::InvalidOperation {
            operation: format!("reflection target '{}'", target),
            type_name: "unknown reflection target — expected Len, Ptr, Absolute, Size, Bytes, Alignment, or Type".into(),
        }),
    }
}

/// 2026-07-31: Resolve `a.m(args)` — find the member on the receiver's obj
/// type, substitute the receiver's type arguments for the obj's type
/// parameters, validate each arg against the substituted parameter types, and
/// return the member's (substituted) result type.
fn resolve_method_call(
    receiver: &Type,
    name: &str,
    args: &[Expr],
    ctx: &mut TypecheckContext,
) -> Result<Type, TypeError> {
    let type_name = match receiver {
        Type::Custom(n) => n.as_str(),
        Type::Applied(n, _) => n.as_str(),
        _ => {
            return Err(TypeError::InvalidOperation {
                operation: format!("method call '.{}()'", name),
                type_name: format!("receiver type '{}' has no methods", receiver),
            });
        }
    };
    let members = ctx.type_members.get(type_name).ok_or_else(|| {
        TypeError::InvalidOperation {
            operation: format!("method call '.{}()'", name),
            type_name: format!("type '{}' has no obj members", type_name),
        }
    })?;
    let member = members
        .iter()
        .find(|m| member_name(m) == name)
        .ok_or_else(|| TypeError::InvalidOperation {
            operation: format!("method call '.{}()'", name),
            type_name: format!("type '{}' has no member '{}'", type_name, name),
        })?;
    // Build the type-argument substitution: obj's declared type params → the
    // receiver's concrete type args.
    let subst = match receiver {
        Type::Applied(_, args) => {
            let obj_type_params = obj_type_params(ctx, type_name);
            obj_type_params
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect::<HashMap<_, _>>()
        }
        _ => HashMap::new(),
    };
    let params = member_params(member);
    let out = member_output(member);
    for (i, arg) in args.iter().enumerate() {
        let arg_ty = infer_type_only(arg, ctx)?;
        let param_ty = params
            .get(i)
            .cloned()
            .map(|t| substitute_type(&t, &subst))
            .unwrap_or(Type::int());
        if arg_ty != param_ty {
            return Err(TypeError::TypeMismatch {
                expected: format!("{}", param_ty),
                found: format!("{}", arg_ty),
                context: format!("argument {} of '.{}()'", i, name),
            });
        }
    }
    Ok(out.map(|t| substitute_type(&t, &subst)).unwrap_or(Type::void()))
}

fn member_name(m: &TopLevel) -> String {
    match m {
        TopLevel::Transaction(t) => t.name.clone(),
        TopLevel::Definition(d) => d.name.clone(),
        _ => String::new(),
    }
}

fn member_params(m: &TopLevel) -> Vec<Type> {
    match m {
        TopLevel::Transaction(t) => t.parameters.iter().map(|(_, ty)| ty.clone()).collect(),
        TopLevel::Definition(d) => d.parameters.iter().map(|(_, ty)| ty.clone()).collect(),
        _ => Vec::new(),
    }
}

fn member_output(m: &TopLevel) -> Option<Type> {
    match m {
        TopLevel::Transaction(t) => t.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()),
        TopLevel::Definition(d) => d.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()),
        _ => None,
    }
}

/// 2026-08-01 (Phase 4): the compiler-known stream symbols. `#StdOut <- value`
/// and `#StdErr <- value` are stream WRITES (lowered to the print family);
/// `#StdIn` is a stream handle value (the trg read composition).
fn is_stream_symbol(name: &str) -> bool {
    matches!(name, "#StdOut" | "#StdErr" | "#StdIn")
}

/// Resolve a stream symbol's type — a system pointer handle (`#StdIn`), or
/// void for the write-only streams (`#StdOut`/`#StdErr` are only valid as
/// arrow targets, never read as values).
fn stream_symbol_type(name: &str) -> Option<Type> {
    match name {
        "#StdIn" => Some(Type::ptr(Type::int())),
        "#StdOut" | "#StdErr" => None,
        _ => None,
    }
}

/// 2026-08-01 (Phase 3): substitute a generic application's concrete type args
/// into a member signature's type-param references (`List<Int>` push's `T` →
/// `Int`). Leaves non-param types untouched; unsubstituted params stay as-is
/// (a bare `T`), which the arrow typecheck treats leniently.
fn substitute_type_params(ty: &Type, params: &[String], args: &[Type]) -> Type {
    match ty {
        Type::Custom(n) => params
            .iter()
            .position(|p| p == n)
            .and_then(|i| args.get(i).cloned())
            .unwrap_or_else(|| ty.clone()),
        Type::Ptr(i) => Type::Ptr(Box::new(substitute_type_params(i, params, args))),
        Type::PtrConst(i) => Type::PtrConst(Box::new(substitute_type_params(i, params, args))),
        Type::Vector(i, dims) => Type::Vector(Box::new(substitute_type_params(i, params, args)), dims.clone()),
        Type::Applied(n, inner) => Type::Applied(
            n.clone(),
            inner.iter().map(|a| substitute_type_params(a, params, args)).collect(),
        ),
        Type::Tuple(elems) => Type::Tuple(
            elems.iter().map(|e| substitute_type_params(e, params, args)).collect(),
        ),
        Type::Union(members) => Type::Union(
            members.iter().map(|m| substitute_type_params(m, params, args)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn obj_type_params(ctx: &TypecheckContext, type_name: &str) -> Vec<String> {
    ctx.type_params
        .get(type_name)
        .cloned()
        .unwrap_or_default()
}

/// 2026-07-31: Substitute a type's generic parameters. `Type::Custom("T")`
/// names matching a substitution key become the mapped concrete type.
pub(crate) fn substitute_type(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Custom(n) => subst.get(n).cloned().unwrap_or_else(|| ty.clone()),
        Type::Applied(n, args) => {
            let new_args = args.iter().map(|a| substitute_type(a, subst)).collect();
            Type::Applied(n.clone(), new_args)
        }
        Type::Vector(inner, dims) => Type::Vector(
            Box::new(substitute_type(inner, subst)),
            dims.iter().map(|d| match d {
                // 2026-07-31 (A8): a Named dimension whose name maps to a
                // size arg becomes an Anonymous dimension (`T[N]` with N=8 →
                // `Int[8]`).
                crate::ast::Dimension::Named(n, _) => {
                    match subst.get(n) {
                        Some(Type::Number(sz)) => crate::ast::Dimension::Anonymous(*sz as usize),
                        _ => d.clone(),
                    }
                }
                _ => d.clone(),
            }).collect(),
        ),
        Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| substitute_type(e, subst)).collect()),
        _ => ty.clone(),
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

    /// A custom `MyNum : #Int` with NO declared op still supports SAME-TYPE
    /// arithmetic — it inherits #Int's protocol binding (Add → AddI64#).
    /// 2026-08-03 (operator-resolution fix): the old name-keyed table only
    /// knew "Int", so `MyNum + MyNum` errored. Resolution is by protocol
    /// category now.
    #[test]
    fn same_type_custom_op_inherits_protocol_binding() {
        let src = r#"
type MyNum : #Int { };
let a: MyNum = 0;
let b: MyNum = 0;
node t [a < 5][a == 5] {
    let s: MyNum = a + b;
    term;
};
"#;
        assert!(check(src).is_ok(), "MyNum + MyNum must inherit #Int's Add binding");
    }

    /// A custom type with a declared `op Add(#Int)` wins for same-type use.
    #[test]
    fn same_type_custom_op_declared_binding_wins() {
        let src = r#"
type MyNum : #Int {
    op Add(#Int): func(#L, #R);
};
let a: MyNum = 0;
let b: MyNum = 0;
node t [a < 5][a == 5] {
    let s: MyNum = a + b;
    term;
};
"#;
        assert!(check(src).is_ok(), "declared op Add(#Int) must authorize MyNum + MyNum");
    }

    /// A custom type with NO protocol and NO declared op still errors on
    /// same-type arithmetic (no implicit blanket arithmetic).
    #[test]
    fn same_type_custom_op_no_protocol_errors() {
        let src = r#"
type MyNum { };
let a: MyNum = 0;
let b: MyNum = 0;
node t [a < 5][a == 5] {
    let s: MyNum = a + b;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("invalid operation")),
            "expected an invalid-operation error, got {:?}",
            err
        );
    }
    /// A subtype inherits an arithmetic op declared on its PARENT type
    /// (parent walk in type_declares_op).
    #[test]
    fn subtype_inherits_parent_declared_op() {
        let src = r#"
type Base : #Int {
    op Add(#Int): func(#L, #R);
};
type MyNum : #Int Base { };
let a: MyNum = 0;
let b: MyNum = 0;
node t [a < 5][a == 5] {
    let s: MyNum = a + b;
    term;
};
"#;
        assert!(check(src).is_ok(), "subtype must inherit the parent's declared op");
    }

    #[test]
    fn field_access_on_non_struct_errors() {
        let src = r#"
let x: Int = 5;
node probe [true][true] {
    let n: Int = x.age;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected field-access error, got: {:?}", e);
    }

    #[test]
    fn field_access_on_obj_resolves() {
        let src = r#"
obj Person { name: String; age: Int; }
let p: Person = Person("Alice", 30);
node probe [true][true] {
    let n: String = p.name;
    let a: Int = p.age;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn reflect_kind_mismatch_errors() {
        let src = r#"
let x: Int = 5;
node probe [true][true] {
    let s: Int = x.^Size;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected reflection kind-mismatch error, got: {:?}", e);
    }

    #[test]
    fn reflect_compile_time_resolves() {
        let src = r#"
let items: Int[8];
node probe [items.^^Size > 0][items == @items] {
    let sz: Int = items.^^Size;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }
    #[test]
    fn method_call_arg_mismatch_errors() {
        let src = r#"
obj Stack {
    data: Int[8];
    len: Int;
    txn push(val: Int) [len < 8][len <= 8] {
        data[len] = val;
        len = len + 1;
        term;
    };
}
let st: Stack = Stack();
node probe [true][true] {
    st.push("hello");
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected method-arg mismatch error, got: {:?}", e);
    }

    #[test]
    fn method_call_unknown_member_errors() {
        let src = r#"
obj Stack { len: Int; }
let st: Stack = Stack();
node probe [true][true] {
    st.pop();
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected unknown-method error, got: {:?}", e);
    }

    #[test]
    fn method_call_on_obj_resolves() {
        let src = r#"
obj Stack {
    data: Int[8];
    len: Int;
    txn push(val: Int) [len < 8][len <= 8] {
        data[len] = val;
        len = len + 1;
        term;
    };
    defn size() -> Int { term len; };
}
let st: Stack = Stack();
node probe [true][true] {
    st.push(5);
    let n: Int = st.size();
    term;
};
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }
    #[test]
    fn let_declared_type_mismatch_errors() {
        let src = r#"
node probe [true][true] {
    let n: Int = "hello";
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected let type-mismatch error, got: {:?}", e);
    }

    #[test]
    fn top_level_let_initializer_checked() {
        let src = r#"
let s: Int = "hello";
node probe [true][true] { term; };
"#;
        let e = check(src);
        assert!(e.is_err(), "expected top-level let type-mismatch error, got: {:?}", e);
    }

    #[test]
    fn term_must_match_declared_return() {
        let src = r#"
defn f() -> Int { term "hello"; };
"#;
        let e = check(src);
        assert!(e.is_err(), "expected term/return type-mismatch error, got: {:?}", e);
    }

    #[test]
    fn call_arg_must_match_param() {
        let src = r#"
defn takes_int(x: Int) -> Int { term x; };
node probe [true][true] {
    let r: Int = takes_int("hello");
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected call-arg type-mismatch error, got: {:?}", e);
    }

    #[test]
    fn numeric_literal_coerces_to_float() {
        // `let f: Float = 5` remains legal via numeric literal construction.
        let src = r#"
let f: Float = 5;
node probe [true][true] { term; };
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }
    #[test]
    fn generic_struct_and_constructor() {
        let src = r#"
struct ListBuffer<T> {
    data: Ptr<T>;
    cap: Int;
};
let lb: ListBuffer<Int> = ListBuffer { data: 0 as Ptr<Int>, cap: 8 };
node probe [true][true] { term; };
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn generic_enum_parses() {
        // Enum DECLARATION with type params parses (variant construction /
        // pattern-matching is part of the collections rewrite, A9).
        let src = r#"
enum Option<T> {
    Some(T),
    None
};
let o: Int = 0;
node probe [true][true] { term; };
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn struct_literal_bare_shorthand() {
        let src = r#"
struct Arena { base: Ptr<Int>; offset: Int; }
let base: Ptr<Int> = 0 as Ptr<Int>;
let offset: Int = 0;
defn mk() -> Arena { term Arena { base, offset }; };
node probe [true][true] { term; };
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn member_body_type_error_caught() {
        let src = r#"
obj Stack { len: Int; txn push(val: Int) [len < 8][len <= 8] { len = "hello"; term; }; }
let st: Stack = Stack();
node probe [true][true] { st.push(5); term; };
"#;
        let e = check(src);
        assert!(e.is_err(), "expected member-body type error, got: {:?}", e);
    }

    #[test]
    fn member_body_with_self_slots_resolves() {
        let src = r#"
obj Stack {
    data: Int[8];
    len: Int;
    txn push(val: Int) [len < 8][len <= 8] {
        data[len] = val;
        len = len + 1;
        term;
    };
}
let st: Stack = Stack();
node probe [true][true] { st.push(5); term; };
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }
}

#[cfg(test)]
mod phase3_tests {
    use super::*;

    fn check(src: &str) -> Result<(), Vec<TypeError>> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&items, &universe)
    }

    fn err_is(src: &str, needle: &str) -> bool {
        match check(src) {
            Err(errs) => errs.iter().any(|e| format!("{}", e).contains(needle)),
            Ok(()) => false,
        }
    }

    // 2026-08-01 (Phase 3): the move pass — reading a consumed local is a
    // use-after-move compile error.

    #[test]
    fn use_after_move_is_an_error() {
        let src = r#"
            defn f(a: Int, b: Int) -> Int {
                a ~= b;
                term a + b;
            };
        "#;
        assert!(err_is(src, "consumed"), "reading b after a ~= b must error");
    }

    #[test]
    fn reassignment_revives_a_consumed_local() {
        let src = r#"
            defn f(a: Int, b: Int) -> Int {
                a ~= b;
                b = 7;
                term a + b;
            };
        "#;
        assert!(check(src).is_ok(), "reassigning b revives it");
    }

    #[test]
    fn cannot_consume_a_constant() {
        let src = r#"
            defn f(a: Int) -> Int {
                term a ~+ 5;
            };
        "#;
        assert!(err_is(src, "cannot consume a constant"));
    }

    #[test]
    fn cannot_consume_a_literal_lvalue() {
        let src = r#"
            defn f() -> Int {
                let x: Int = 5;
                term x ~= 3;
            };
        "#;
        // `x ~= 3` consumes the literal 3 — a compile error.
        assert!(err_is(src, "cannot consume a constant"));
    }

    #[test]
    fn arrow_insert_typechecks() {
        // A plain-copy arrow on scalars typechecks like an assignment.
        let src = r#"
            defn f(a: Int, b: Int) -> Int {
                a <- b;
                term a;
            };
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn arrow_discard_typechecks() {
        let src = r#"
            defn f(a: Int) -> Int {
                <- a;
                term a;
            };
        "#;
        assert!(check(src).is_ok());
    }
}

#[cfg(test)]
mod phase4_tests {
    use super::*;

    fn check(src: &str) -> Result<(), Vec<TypeError>> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&items, &universe)
    }

    // 2026-08-01 (Phase 4): the stream symbols — `#StdOut` accepts any value,
    // `#StdErr` requires a String.

    #[test]
    fn stdout_stream_accepts_any_value() {
        let src = r#"
            defn f(a: Int) -> Int {
                #StdOut <- a;
                term a;
            };
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn stderr_stream_requires_string() {
        let src = r#"
            defn f(a: Int) -> Int {
                #StdErr <- a;
                term a;
            };
        "#;
        assert!(check(src).is_err(), "#StdErr <- Int must be rejected (String only)");
    }

    #[test]
    fn std_in_is_a_stream_handle_value() {
        let src = r#"
            defn f() -> Int {
                let h: Ptr<Int> = #StdIn;
                term 0;
            };
        "#;
        assert!(check(src).is_ok());
    }
}

#[cfg(test)]
mod phase5_tests {
    use super::*;

    fn check(src: &str) -> Result<(), Vec<TypeError>> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&items, &universe)
    }

    fn err_is(src: &str, needle: &str) -> bool {
        match check(src) {
            Err(errs) => errs.iter().any(|e| format!("{}", e).contains(needle)),
            Ok(()) => false,
        }
    }

    // 2026-08-01 (Phase 5): `free x;` is a verified contract — a later read
    // is a use-after-free error; reassignment revives x.

    #[test]
    fn free_then_read_is_an_error() {
        let src = r#"
            defn f(a: Int) -> Int {
                free a;
                term a;
            };
        "#;
        assert!(err_is(src, "consumed"), "reading a after free a must error");
    }

    #[test]
    fn free_then_reassign_revives() {
        let src = r#"
            defn f(a: Int) -> Int {
                free a;
                a = 7;
                term a;
            };
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn cannot_free_an_immutable_local() {
        let src = r#"
            defn f() -> Int {
                let x: Int = 5;
                free x;
                term 0;
            };
        "#;
        // `free x` on an immutable defn-local — not a mutable location.
        assert!(err_is(src, "cannot free a constant"));
    }

    #[test]
    fn keep_on_unknown_name_errors() {
        let src = r#"
            defn f() -> Int {
                keep nope;
                term 0;
            };
        "#;
        assert!(check(src).is_err(), "keep of an unknown name must error");
    }
}
