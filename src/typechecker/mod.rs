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
    /// 2026-08-09 (init kind, Phase 2): runtime-seeded invariant names. Reads
    /// resolve via `bindings` (registered in make_typecheck_context); any
    /// write to one — `=` assign, `<-`/`~<-` arrow, `let` shadow, `~op`
    /// consume — is a compile error (the value is set once before
    /// `beginprogram`, immutable for the run). Distinct from `state_keys`
    /// (mutable state) and `consumed_locals` (revivable locals).
    pub init_names: std::collections::HashSet<String>,
    /// 2026-08-06 (diagnostics): every function/transaction name defined in the
    /// program (defns, txns, imported bodies). Used to validate a declared
    /// `op` binding's implementation target actually exists.
    pub defined_fns: std::collections::HashSet<String>,
    /// 2026-08-01 (Phase 3): locals consumed by a `~op` (`a ~= b`, `dest ~<-
    /// src`, `~<- src;`). Reading a consumed local afterward is a use-after-move
    /// compile error; reassigning it (via `=` or `let`) clears the mark.
    pub consumed_locals: std::collections::HashSet<String>,
    pub universe: &'a TypeUniverse,
    /// 2026-08-05 (Phase 5): the seeded protocol-casting graph, used to enforce
    /// the SPEC §8.7 rule that one written `as` traverses at most one
    /// cross-protocol edge.
    pub casting_graph: crate::casting::graph::CastingGraph,
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
    /// 2026-08-14 (generic `defn f<T>` dispatch): a defn's declared type
    /// params (e.g. `["T"]` for `defn first<T>`), so call sites can infer and
    /// substitute concrete args. Populated alongside fn_return_types.
    fn_type_params: HashMap<String, Vec<String>>,
    /// 2026-08-14 (generic `defn f<T>` dispatch): the expected type of the
    /// enclosing `let` initializer, consulted when a nullary generic
    /// (`new_stack<T>()` with no type-constraining args) needs its param bound
    /// from the annotation. Transient — set by the Let binding, cleared after.
    expected_call_type: Option<Type>,
    /// 2026-08-09 (Phase 12, SPEC §19.3): names of `optional frgn` bindings.
    /// `feature.^^Available` (a compile-time descriptor reflect) is a Bool
    /// only for these — non-optional frgns are always available.
    optional_frgns: std::collections::HashSet<String>,
    /// 2026-07-31: Regular operator declarations from TypeDef bodies
    /// (`op Add(#Float): func(#Lh,#Rh);` / `op Add(Float): ...;`), keyed by type
    /// name. Used to ALLOW mixed-type arithmetic ONLY when a cross-type /
    /// cross-protocol overload is explicitly declared — otherwise
    /// `Int * Float` is a type error (no implicit numeric coercion).
    /// Both `td.body.operators` (ProtocolDef-style OperatorDefs) and
    /// `td.body.op_bindings` (type-body `op Name(#Proto): fn(#Lh,#Rh);`) are
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
    /// (`proto C_String: #String { op Concat(#String) = cstring_concat(#Lh,#Rh) }`).
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
            init_names: std::collections::HashSet::new(),
            defined_fns: std::collections::HashSet::new(),
            consumed_locals: std::collections::HashSet::new(),
            universe,
            casting_graph: crate::casting::graph::CastingGraph::new(),
            parse_ops: HashMap::new(),
            type_parents: HashMap::new(),
            fn_return_types: HashMap::new(),
            fn_type_params: HashMap::new(),
            expected_call_type: None,
            optional_frgns: std::collections::HashSet::new(),
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

    /// 2026-08-06 (Phase 5): does `ty` declare `op <op_name>` covering
    /// `operand`, and what function implements it? Like `type_declares_op`
    /// but returns the implementation fn name (from the binding's
    /// `Call(fn, ...)` expr or the operator's `impl_args`) instead of a bool.
    /// Walks `type_parents` so a subtype inherits its parent's op.
    fn type_declares_op_binding(&self, ty: &Type, op_name: &str, operand: &Type) -> Option<String> {
        let mut current = match ty {
            Type::Custom(n) => n.as_str(),
            Type::Applied(n, _) => n.as_str(),
            _ => return None,
        };
        loop {
            if let Some(bindings) = self.regular_bindings.get(current) {
                for b in bindings {
                    if b.name != op_name {
                        continue;
                    }
                    // Coverage mirrors `type_declares_op`: only a declared
                    // protocol VARIANT (`op Add(Float)`) covers the operand.
                    // A colon-form binding (`op Add: add(#Lh, #Rh)`) has no
                    // variant — it is documentation/authorization; the
                    // category's protocol binding governs its dispatch.
                    let covers = b
                        .protocol_variant
                        .as_ref()
                        .map_or(false, |v| self.variant_covers(v, operand));
                    if covers {
                        if let Expr::Call(fn_name, _, _) = &b.expr {
                            return Some(fn_name.clone());
                        }
                    }
                }
            }
            if let Some(ops) = self.regular_ops.get(current) {
                for op in ops {
                    if op.op != op_name {
                        continue;
                    }
                    let covers = op.params.first().map_or(false, |p| self.param_covers(p, operand));
                    if covers {
                        if let Some(name) = cross_op_fn_name(&op.impl_args) {
                            return Some(name);
                        }
                    }
                }
            }
            match self.type_parents.get(current) {
                Some(parent) => current = parent.as_str(),
                None => return None,
            }
        }
    }

    /// 2026-08-06 (Phase 5): resolve a binary op to its semantic OpBinding —
    /// the chain `arithmetic_result_ty` used, returning the binding instead of
    /// dropping it. Declared ops (type-body `op Name: fn(...)`) resolve to
    /// `Function(fn)`; protocol bindings to the intrinsic. This is the single
    /// resolution chain for both typechecking and op elaboration.
    fn resolve_binary_op_binding(
        &self,
        kind: &BinaryOpKind,
        lhs: &Type,
        rhs: &Type,
    ) -> Option<OpBinding> {
        let rune = format!("{}", kind);
        let op_name = crate::type_universe::operators::rune_to_op_name(&rune)?;
        if let Some(fn_name) = self.type_declares_op_binding(lhs, &op_name, rhs) {
            return Some(OpBinding::Function(fn_name));
        }
        if let Some(binding) = self.protocol_binding_for(&rune, lhs) {
            return Some(binding);
        }
        if let Some(binding) =
            crate::type_universe::operators::get_operator_intrinsic(self.universe, &rune, lhs)
        {
            return Some(binding);
        }
        if let Some(binding) = self.protocol_binding_for(&rune, rhs) {
            return Some(binding);
        }
        crate::type_universe::operators::get_operator_intrinsic(self.universe, &rune, rhs)
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
        // 2026-08-11 (housekeeping 1b fix): the documented push form is
        // `&list <- value` — the target is `AddrOf(Identifier)`. Unwrap the
        // address-of so the InsertAt op binding resolves (the old `<-` was a
        // plain assignment, which silently never pushed).
        let inner = match collection {
            Expr::AddrOf(e) => e.as_ref(),
            other => other,
        };
        let Expr::Identifier(name) = inner else { return None; };
        let (type_name, args) = match self.bindings.get(name)? {
            Type::Custom(n) => (n.clone(), Vec::new()),
            Type::Applied(n, a) => (n.clone(), a.clone()),
            _ => return None,
        };
        let members = self.type_members.get(&type_name)?;
        // 2026-08-12 (Iterable protocol, op-as-member): the operator IS the
        // member — its first parameter is the element type, no binding
        // indirection (`op InsertAt(v: T) { … }`).
        if let Some(op_member) = operator_member(members, "InsertAt") {
            let elem = op_member.parameters.first().map(|(_, ty)| ty.clone())?;
            let params = self.type_params.get(&type_name).cloned().unwrap_or_default();
            return Some(substitute_type_params(&elem, &params, &args));
        }
        let bindings = self.regular_bindings.get(&type_name)?;
        let binding = bindings.iter().find(|b| b.name == "InsertAt")?;
        let fn_name = match &binding.expr {
            Expr::Call(name, _, _) => name.clone(),
            _ => return None,
        };
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
        // 2026-08-11 (housekeeping 1b fix): unwrap AddrOf (`&queue <- dest`)
        // like push_element_type.
        let inner = match collection {
            Expr::AddrOf(e) => e.as_ref(),
            other => other,
        };
        let Expr::Identifier(name) = inner else { return None; };
        let (type_name, args) = match self.bindings.get(name)? {
            Type::Custom(n) => (n.clone(), Vec::new()),
            Type::Applied(n, a) => (n.clone(), a.clone()),
            _ => return None,
        };
        let members = self.type_members.get(&type_name)?;
        // 2026-08-12 (Iterable protocol, op-as-member): the operator IS the
        // member — its return type is the extracted element type.
        if let Some(op_member) = operator_member(members, "ExtractFrom")
            .or_else(|| operator_member(members, "CopyFrom"))
        {
            let out = op_member.output_type.as_ref().and_then(|o| o.all_types().into_iter().next())?;
            let params = self.type_params.get(&type_name).cloned().unwrap_or_default();
            return Some(substitute_type_params(&out, &params, &args));
        }
        let bindings = self.regular_bindings.get(&type_name)?;
        let binding = bindings
            .iter()
            .find(|b| b.name == "ExtractFrom" || b.name == "CopyFrom")?;
        let fn_name = match &binding.expr {
            Expr::Call(name, _, _) => name.clone(),
            _ => return None,
        };
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
    // This handles op Parse: parse_string(#Lh); (no protocol variant).
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
/// 2026-08-05 (Phase 5): enforce that one written `as` traverses at most one
/// cross-protocol edge (SPEC §8.7). Intra-protocol refinement is free; crossing
/// more than one protocol category requires chained casts (`value as A as B`).
fn validate_cast_protocol_crossing(
    ctx: &TypecheckContext,
    src_ty: &Type,
    target_ty: &Type,
) -> Result<(), TypeError> {
    let graph = &ctx.casting_graph;
    let (src_cat, src_var) = graph.type_to_protocol(ctx.universe, src_ty);
    let (dst_cat, dst_var) = graph.type_to_protocol(ctx.universe, target_ty);
    let Some(path) = graph.find_path(&src_cat, &src_var, &dst_cat, &dst_var) else {
        return Ok(());
    };
    let cross_steps = path
        .iter()
        .filter(|s| s.src_category != s.dst_category)
        .count();
    if cross_steps > 1 {
        return Err(TypeError::InvalidOperation {
            operation: format!(
                "a single cast crosses {} protocol categories ({} → {}); \
                 chain the casts (value as A as B)",
                cross_steps, src_ty, target_ty
            ),
            type_name: src_ty.to_string(),
        });
    }
    Ok(())
}

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
        Expr::BeginProgram => Ok((Type::bool_(), Provenance::Unknown)),
        Expr::Quoted(_) => Ok((Type::string(), Provenance::Unknown)),
        // 2026-08-06 (Phase 7): `#b"..."` (TaggedQuotedLiteral prefix "b") is a
        // Data byte literal; other prefix-tagged literals are Strings.
        Expr::TaggedQuotedLiteral(_, prefix) => {
            Ok((quoted_literal_type(prefix), Provenance::Unknown))
        }

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
                // 2026-08-14 (generic `defn f<T>`): an EMPTY list in a generic
                // body adopts the declared return's element type when it is a
                // free type param — `defn empty_list<T>() -> List<T> { term
                // []; }` must be `List<T>`, not the `List<Int>` default.
                let declared = ctx.current_output_type.clone();
                if let Some(Type::Applied(base, args)) = declared {
                    if base == "List" {
                        if let Some(elem) = args.first() {
                            if matches!(elem, Type::Custom(_)) {
                                return Ok((
                                    Type::Applied("List".into(), vec![elem.clone()]),
                                    Provenance::Unknown,
                                ));
                            }
                        }
                    }
                }
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
            let (idx_ty, idx_prov) = infer_expression(index, ctx)?;
            // 2026-08-07 (Phase 7): a Bool-vector index is a MASK —
            // `data[mask]` selects the bytes at the true positions, so the
            // result is the byte-buffer container kind (Data), not the scalar
            // element type. Categories resolve via the universe (rule 18).
            // A mask is a Bool vector (`Bool[N]`) or a Bool list literal
            // (`[true, false, …]`, typed Applied("List", [Bool])).
            let idx_is_bool_vector = {
                let elem = match &idx_ty {
                    Type::Vector(inner, _) => Some(inner.as_ref()),
                    Type::Applied(_, args) => args.first(),
                    _ => None,
                };
                elem.map_or(false, |e| {
                    crate::type_universe::operators::protocol_category(ctx.universe, e)
                        .as_deref()
                        == Some("Bool")
                })
            };
            if idx_is_bool_vector {
                let obj_is_byte_buffer = matches!(obj_ty, Type::Bits(_))
                    || crate::type_universe::operators::protocol_category(ctx.universe, &obj_ty)
                        .as_deref()
                        == Some("Data")
                    || crate::type_universe::operators::protocol_category(ctx.universe, &obj_ty)
                        .as_deref()
                        == Some("String");
                if obj_is_byte_buffer {
                    return Ok((
                        Type::data(),
                        Provenance::Index {
                            base: Box::new(obj_prov),
                            index: Box::new(idx_prov),
                        },
                    ));
                }
                // A typed vector field (Int/Bool — i64-slot %State arrays;
                // Float — native `[N x float]`) or a heap List masks into a
                // heap List of its element type.
                let i64_slot_elem = match &obj_ty {
                    Type::Vector(inner, _) => Some(inner.as_ref()),
                    Type::Applied(n, args) if n == "List" => args.first(),
                    _ => None,
                };
                if let Some(elem) = i64_slot_elem {
                    let cat = crate::type_universe::operators::protocol_category(
                        ctx.universe,
                        elem,
                    );
                    if cat.as_deref() == Some("Int")
                        || cat.as_deref() == Some("Bool")
                        || cat.as_deref() == Some("Float")
                    {
                        return Ok((
                            Type::Applied("List".into(), vec![elem.clone()]),
                            Provenance::Index {
                                base: Box::new(obj_prov),
                                index: Box::new(idx_prov),
                            },
                        ));
                    }
                }
                // A Boolean mask selects elements — only byte-buffer or
                // i64-slot vector containers accept it. Anything else is a
                // hard type error (not a silent element-type fallback).
                return Err(TypeError::InvalidOperation {
                    operation: "mask index with a Boolean vector".into(),
                    type_name: format!("{}", obj_ty),
                });
            }
            let elem_ty = match &obj_ty {
                // 2026-08-07 (Phase 7): a multi-dim vector indexed once yields
                // a ROW (the remaining dims) — `data[row]` is Vector(T, [N]);
                // only the LAST index yields the element type.
                Type::Vector(inner, dims) if dims.len() > 1 => {
                    Type::Vector(Box::new((**inner).clone()), dims[1..].to_vec())
                }
                Type::Vector(inner, _) => (**inner).clone(),
                Type::Ptr(inner) | Type::PtrConst(inner) => (**inner).clone(),
                Type::Custom(n) if n == "String" => Type::int(),
                // 2026-08-04 (compiler-in-Briev): indexing a generic returns
                // its element type param — `List<String>[i]` is String, not
                // Int. General (any generic's first type param is its
                // element), so no type-name matching.
                Type::Applied(_, args) if !args.is_empty() => args[0].clone(),
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
            // 2026-08-05 (Phase 5): one written `as` traverses at most one
            // cross-protocol edge (SPEC §8.7).
            validate_cast_protocol_crossing(ctx, &src_ty, target_ty)?;
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
        // 2026-08-09 (Phase 10): `await task` — the handle's type IS the
        // task's result type (a task spawn's handle carries the defn's return
        // type, SPEC §12.2); await reads it. The handle's consumption (a later
        // use errors) is enforced by the ownership analysis.
        Expr::Await(inner) => {
            let (ty, prov) = infer_expression(inner, ctx)?;
            Ok((ty, prov))
        }
        // 2026-07-31: Reflection: x.^Length / x.^^Size (see resolve_reflect).
        Expr::Reflect(recv, target, kind) => {
            // 2026-08-09 (Phase 12, SPEC §19.3): `feature.^^Available` — a
            // compile-time descriptor Bool for an `optional frgn`. Only an
            // optional frgn name admits it (non-optional frgns are always
            // available; the reflect on a non-frgn value is an error).
            if target == "Available" && matches!(kind, ReflectKind::CompileTime) {
                let is_optional_frgn = match recv.as_ref() {
                    Expr::Identifier(n) => ctx.optional_frgns.contains(n),
                    _ => false,
                };
                if !is_optional_frgn {
                    return Err(TypeError::InvalidOperation {
                        operation: "reflection target 'Available'".into(),
                        type_name: format!(
                            "`^^Available` is valid only on an `optional frgn` name"
                        ),
                    });
                }
                return Ok((Type::bool_(), Provenance::Unknown));
            }
            // 2026-08-14 (boundary plan): `value.^^Element` — the ELEMENT type
            // of an iterable receiver as a frozen descriptor (SPEC §17.2:1430-1433).
            // Compile-time only, single-source proof form: the element type IS
            // the read op's return (`op At` Tier 2 / `op Current` Tier 1) or the
            // frozen `#String` → `Char` protocol fact — never a second derivation
            // that could drift. The expression type is Int (the folded category
            // code, exactly like `.^^Type`); the VALUE is folded at codegen.
            if target == "Element" && matches!(kind, ReflectKind::CompileTime) {
                let (recv_ty, recv_prov) = infer_expression(recv, ctx)?;
                if resolve_element_type(ctx, &recv_ty).is_none() {
                    return Err(TypeError::InvalidOperation {
                        operation: "reflection target 'Element'".into(),
                        type_name: format!(
                            "`^^Element` is valid only on an iterable receiver (a Tier-2/1 \
                             collection or a #String operand); `{recv_ty}` has no element type"
                        ),
                    });
                }
                return Ok((Type::int(), recv_prov));
            }
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
            let result_ty = resolve_method_call(recv, &recv_ty, name, args, ctx)?;
            Ok((result_ty, recv_prov))
        }
        Expr::FormattingAnnotation(_) => Ok((Type::void(), Provenance::Unknown)),
        Expr::Match(expr, arms) => infer_match(expr, arms, ctx).map(|ty| (ty, Provenance::Unknown)),
        // 2026-07-19: Plugin-intercept calls are resolved by Front or Mid
        // stage plugins. `brievc check` does not run plugins, so known
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
            // 2026-08-07 (Phase 7): an iterable integer range — `start..end`
            // / `start..=end`. Consumed by `foreach` (SPEC §11.4).
            Expr::Range { start, end, inclusive: _ } => {
                infer_type_only(start, ctx)?;
                infer_type_only(end, ctx)?;
                Ok((Type::Applied("Range".into(), vec![Type::int()]), Provenance::Unknown))
            }
            // 2026-08-07 (object instance pools): `spawn Obj(args)` creates an
            // obj instance + returns a handle of the instance type. The type
            // args come from a `let h: Obj<...> = spawn ...` annotation.
            Expr::Spawn { type_name, args, .. } => {
                for a in args {
                    infer_type_only(a, ctx)?;
                }
                // 2026-08-09 (Phase 10): `spawn defn(args)` is a TASK spawn —
                // its handle carries the defn's return type (SPEC §12.2); an
                // obj spawn keeps the obj's Custom type.
                if let Some(ty) = ctx.fn_return_types.get(type_name) {
                    Ok((ty.clone(), Provenance::Unknown))
                } else {
                    Ok((Type::Custom(type_name.clone()), Provenance::Unknown))
                }
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
    // 2026-08-11 (housekeeping 1b fix): an EMPTY list literal `[]` carries no
    // element to infer — the typechecker defaults it to List<Int>. On
    // assignment to a `List<T>` target it must coerce: an empty list is valid
    // for every element type. (The Parse-op machinery below has no form for a
    // bare `[]`.)
    if let Expr::List(elems) = expr {
        if elems.is_empty() {
            if let Type::Applied(n, _) = target_ty {
                return n == "List";
            }
            return false;
        }
    }
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
    // 2026-07-31 (Phase 2): `op Init: init(#Lh, #Rh)` authorizes `let t: T = v`
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
        let op_name = name.trim_end_matches('#');
        // 2026-08-14 (UOL §6b.2): generative op-identity dispatch — an
        // `OpName#` that names a disclosed OPERATION dispatches to the type's
        // declared op member on arg[0]. For the element-bearing collection ops
        // (`At#`/`Slice#`/`ExtractFrom#`/`CopyFrom#`) the return IS the op
        // member's output — NOT the generic "first-arg type" rule (`At#` on
        // `List<Int>` must be `Int`, the `op At` return, not `List<Int>`), so
        // this path runs FIRST for them. Arithmetic/bitwise ops (`Add#`,
        // `Eq#`, …) keep their exact signatures + template dispatch — their
        // receivers are scalars with no op members, and the generative path
        // would wrongly error. `Count#`/`InsertAt#` ride the signature path
        // (Native Int / Exact Void are already correct); `Count#` on a
        // `#String` is handled inside infer_generative_op_call via the
        // signature-free fallthrough below.
        let op_inferred = matches!(op_name,
            "At" | "Slice" | "ExtractFrom" | "CopyFrom"
            | "Count" | "Iter" | "Step" | "IsEnd" | "Current");
        if op_inferred && is_operation_identity(op_name) {
            if let Some(ty) = infer_generative_op_call(op_name, args, ctx)? {
                return Ok(ty);
            }
        }
        let sig = get_intrinsic_signature(name).ok_or_else(|| {
            // 2026-08-06 (diagnostics): the PascalCase intrinsic forms of the
            // env macros were renamed to lowercase macros — direct the user
            // instead of reporting an "unknown intrinsic".
            if matches!(name, "GetEnvInt#" | "GetEnv#" | "GetEnvOrDefault#") {
                let macro_name = match name {
                    "GetEnvInt#" => "get_env_int!",
                    "GetEnvOrDefault#" => "get_env_or_default!",
                    _ => "get_env!",
                };
                return TypeError::InvalidOperation {
                    operation: format!("call to '{}'", name),
                    type_name: format!(
                        "the lowercase macro '{}' replaced this PascalCase name — rename the call site",
                        macro_name
                    ),
                };
            }
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
    // 2026-08-14 (generic `defn f<T>` dispatch): a generic defn's params are
    // inferred at the call site and validated against the SUBSTITUTED types.
    if let Some(params) = ctx.fn_type_params.get(name).cloned() {
        if !params.is_empty() {
            if let Some(args_conc) = infer_defn_type_args(name, &params, args, ctx)? {
                for (i, arg) in args.iter().enumerate() {
                    let arg_ty = infer_type_only(arg, ctx)?;
                    if let Some(param_ty) = ctx.fn_param_types.get(name).and_then(|p| p.get(i)) {
                        let subst_ty = substitute_type_params(param_ty, &params, &args_conc);
                        if arg_ty != subst_ty && !try_coerce_via_parse(arg, &arg_ty, &subst_ty, ctx) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{}", subst_ty),
                                found: format!("{}", arg_ty),
                                context: format!("argument {} of '{}'", i, name),
                            });
                        }
                    }
                }
                if let Some(ret) = ctx.fn_return_types.get(name).cloned() {
                    return Ok(substitute_type_params(&ret, &params, &args_conc));
                }
            }
        }
    }
    let param_types = ctx.fn_param_types.get(name).cloned().unwrap_or_default();
    for (i, arg) in args.iter().enumerate() {
        let arg_ty = infer_type_only(arg, ctx)?;
        if let Some(param_ty) = param_types.get(i) {
            // 2026-08-09 (Phase 12, SPEC §18.2): the meld admission is removed —
            // only an explicit coercion path admits the pair.
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
/// 2026-08-06 (Phase 7): the type of a prefix-tagged literal — `#b` (prefix
/// "b") is a Data byte literal; other tags are Strings.
fn quoted_literal_type(prefix: &str) -> Type {
    if prefix == "b" {
        Type::Custom("Data".into())
    } else {
        Type::string()
    }
}

fn arithmetic_result_ty(    ctx: &TypecheckContext,
    kind: &BinaryOpKind,
    lhs_ty: &Type,
    rhs_ty: &Type,
    lhs_str: &str,
) -> Result<Type, TypeError> {
    let binding = ctx.resolve_binary_op_binding(kind, lhs_ty, rhs_ty);
    match binding {
        Some(_) => Ok(lhs_ty.clone()),
        None => Err(TypeError::InvalidOperation {
            operation: format!("'{}'", kind),
            type_name: lhs_str.to_string(),
        }),
    }
}

// ── Op elaboration (Phase 5) ─────────────────────────────────────────

/// 2026-08-06 (Phase 5): rewrite every `BinaryOp` whose resolved binding is a
/// declared Function into `Expr::Call(fn, [l, r])`, so lowering invokes the
/// declared implementation instead of re-dispatching by operand type. Protocol
/// intrinsics stay as BinaryOp (their category-dispatch lowering is already
/// correct and benchmark-stable). Runs after typechecking so types are known;
/// mutates `items` in place. A declared op whose implementation target is not
/// a defined function is an error (was a silent link-time undefined symbol).
pub fn elaborate_ops(items: &mut [TopLevel], universe: &TypeUniverse, env: &CheckEnv) -> Vec<TypeError> {
    let mut errors = Vec::new();
    for item in items.iter_mut() {
        let mut ctx = make_typecheck_context(env, universe);
        match item {
            TopLevel::Definition(d) => {
                for (name, ty) in &d.parameters {
                    ctx.bindings.insert(name.clone(), ty.clone());
                }
                elaborate_stmts(&mut d.body, &mut ctx, &mut errors);
                elaborate_expr(&mut d.contract.pre_condition, &mut ctx, &mut errors);
                // 2026-08-14 (`term` canonical result placeholder): bind
                // `term` to the declared output type while elaborating the
                // POST-condition — `[term == true]`, `[term.Count#() == n]`
                // typecheck with the real return type, not the Int fallback.
                let term_ty = d.output_type.as_ref().map(output_type_to_type);
                let saved = ctx.bindings.insert("term".to_string(), term_ty.unwrap_or(Type::void()));
                elaborate_expr(&mut d.contract.post_condition, &mut ctx, &mut errors);
                if let Some(prev) = saved {
                    ctx.bindings.insert("term".to_string(), prev);
                } else {
                    ctx.bindings.remove("term");
                }
            }
            TopLevel::Transaction(t) => {
                for (name, ty) in &t.parameters {
                    ctx.bindings.insert(name.clone(), ty.clone());
                }
                elaborate_stmts(&mut t.body, &mut ctx, &mut errors);
                elaborate_expr(&mut t.contract.pre_condition, &mut ctx, &mut errors);
                let term_ty = t.output_type.as_ref().map(output_type_to_type);
                let saved = ctx.bindings.insert("term".to_string(), term_ty.unwrap_or(Type::void()));
                elaborate_expr(&mut t.contract.post_condition, &mut ctx, &mut errors);
                if let Some(prev) = saved {
                    ctx.bindings.insert("term".to_string(), prev);
                } else {
                    ctx.bindings.remove("term");
                }
            }
            TopLevel::Statement(stmt) => elaborate_stmt(stmt, &mut ctx, &mut errors),
            TopLevel::Constant(c) => elaborate_expr(&mut c.expr, &mut ctx, &mut errors),
            TopLevel::Init(init) => {
                // Register the name so later statements can reference it, then
                // elaborate the seeding expr/body (like a const, but runtime).
                ctx.bindings.insert(init.name.clone(), init.ty.clone());
                if let Some(value) = &mut init.value {
                    elaborate_expr(value, &mut ctx, &mut errors);
                }
                elaborate_stmts(&mut init.body, &mut ctx, &mut errors);
            }
            _ => {}
        }
    }
    errors
}

fn elaborate_stmts(stmts: &mut [Statement], ctx: &mut TypecheckContext, errors: &mut Vec<TypeError>) {
    for stmt in stmts.iter_mut() {
        elaborate_stmt(stmt, ctx, errors);
    }
}

fn elaborate_stmt(stmt: &mut Statement, ctx: &mut TypecheckContext, errors: &mut Vec<TypeError>) {
    match stmt {
        Statement::Let { name, names, ty, expr, .. } => {
            if let Some(e) = expr {
                elaborate_expr(e, ctx, errors);
            }
            // Track the binding so later statements resolve its type (mirrors
            // infer_statement). Declared type wins; else infer the initializer.
            let bound_ty = ty.clone().or_else(|| {
                expr.as_ref().and_then(|e| infer_type_only(e, ctx).ok())
            });
            for n in std::iter::once(&*name).chain(names.iter()) {
                if let Some(t) = &bound_ty {
                    ctx.bindings.insert(n.clone(), t.clone());
                }
            }
        }
        Statement::Assign(l, r) => {
            elaborate_expr(l, ctx, errors);
            elaborate_expr(r, ctx, errors);
        }
        Statement::ArrowAssign { value, .. } => elaborate_expr(value, ctx, errors),
        Statement::Term(Some(e))
        | Statement::EndProgram(Some(e))
        | Statement::Rollback(Some(e)) => elaborate_expr(e, ctx, errors),
        Statement::Expression(e) | Statement::Gate(e) => elaborate_expr(e, ctx, errors),
        Statement::Guarded(cond, body) => {
            elaborate_expr(cond, ctx, errors);
            elaborate_stmts(body, ctx, errors);
        }
        Statement::If(cond, then, else_) => {
            elaborate_expr(cond, ctx, errors);
            elaborate_stmts(then, ctx, errors);
            elaborate_stmts(else_, ctx, errors);
        }
        Statement::Block(body) | Statement::SyncBlock(body)
        | Statement::Defer(body) | Statement::Mutex(body) => elaborate_stmts(body, ctx, errors),
        Statement::Barrier { body, .. } => elaborate_stmts(body, ctx, errors),
        Statement::Foreach { list, body, .. } => {
            elaborate_expr(list, ctx, errors);
            elaborate_stmts(body, ctx, errors);
        }
        _ => {}
    }
}

fn elaborate_expr(expr: &mut Expr, ctx: &mut TypecheckContext, errors: &mut Vec<TypeError>) {
    match expr {
        Expr::Identifier(_) => {
            // 2026-08-06 (fix): closures are real first-class values now (a
            // let-bound lambda is a heap env block address in codegen), so a
            // Function-typed binding used as a value is legal. The typechecker
            // (infer_expression) still rejects a Function value where a
            // non-Function is required.
        }
        Expr::BinaryOp(kind, l, r) => {
            let lt = infer_type_only(l, ctx).unwrap_or(Type::int());
            let rt = infer_type_only(r, ctx).unwrap_or(Type::int());
            if let Some(OpBinding::Function(fn_name)) = ctx.resolve_binary_op_binding(kind, &lt, &rt) {
                // 2026-08-06 (diagnostics): the declared op's implementation
                // target must be a defined function — a typo would otherwise
                // surface as a link-time undefined symbol.
                if !ctx.defined_fns.contains(&fn_name) {
                    errors.push(TypeError::InvalidOperation {
                        operation: format!("'{}'", kind),
                        type_name: format!(
                            "op target '{}' is not a defined function — declare it or fix the op binding",
                            fn_name
                        ),
                    });
                    elaborate_expr(l, ctx, errors);
                    elaborate_expr(r, ctx, errors);
                    return;
                }
                // Rewrite children bottom-up, then fold the op into a call to
                // the declared implementation.
                elaborate_expr(l, ctx, errors);
                elaborate_expr(r, ctx, errors);
                *expr = Expr::Call(fn_name, vec![(**l).clone(), (**r).clone()], None);
                return;
            }
            elaborate_expr(l, ctx, errors);
            elaborate_expr(r, ctx, errors);
        }
        Expr::Block(stmts) => elaborate_stmts(stmts, ctx, errors),
        Expr::If(c, t, e) => {
            elaborate_expr(c, ctx, errors);
            elaborate_expr(t, ctx, errors);
            if let Some(e) = e {
                elaborate_expr(e, ctx, errors);
            }
        }
        Expr::Match(scrut, arms) => {
            elaborate_expr(scrut, ctx, errors);
            for arm in arms.iter_mut() {
                if let Some(g) = &mut arm.guard {
                    elaborate_expr(g, ctx, errors);
                }
                elaborate_expr(&mut arm.body, ctx, errors);
            }
        }
        Expr::Call(_, args, _) => {
            for a in args.iter_mut() {
                elaborate_expr(a, ctx, errors);
            }
        }
        Expr::List(es) | Expr::Tuple(es) => {
            for e in es.iter_mut() {
                elaborate_expr(e, ctx, errors);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, f) in fields.iter_mut() {
                elaborate_expr(f, ctx, errors);
            }
        }
        Expr::Index(o, i) => {
            elaborate_expr(o, ctx, errors);
            elaborate_expr(i, ctx, errors);
        }
        Expr::Slice { array, start, end, stride } => {
            elaborate_expr(array, ctx, errors);
            for bound in [start, end, stride].into_iter().flatten() {
                elaborate_expr(bound, ctx, errors);
            }
        }
        Expr::Lambda(_, b) => elaborate_expr(b, ctx, errors),
        Expr::Cast(e, _)
        | Expr::IsType(e, _)
        | Expr::Consume(e)
        | Expr::Deref(e)
        | Expr::AddrOf(e)
        | Expr::Reflect(e, _, _)
        | Expr::Within(e, _) => elaborate_expr(e, ctx, errors),
        Expr::MethodCall(recv, _, args, _) => {
            elaborate_expr(recv, ctx, errors);
            for a in args.iter_mut() {
                elaborate_expr(a, ctx, errors);
            }
        }
        Expr::PluginIntercept { args, .. } => {
            for a in args.iter_mut() {
                elaborate_expr(a, ctx, errors);
            }
        }
        _ => {}
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
            // 2026-08-03 (operator-resolution fix): resolution order is
            //   declared (own + parents) → protocol bindings.
            // A type declaring `op Add(#Float)`/`op Add(Float)` authorizes
            // mixed arithmetic; a custom `MyNum : #Int` with no declared op
            // inherits #Int's protocol binding (Add → AddI64#). Only the
            // protocol bindings are hardcoded — keyed by category, never by
            // type name. Same-type custom ops now resolve here too.
            arithmetic_result_ty(ctx, kind, &lhs_ty, &rhs_ty, &lhs_str)?
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
            // 2026-08-09 (init kind, Phase 2): a `let` declaring an `init`
            // name would shadow the seeded invariant — reject the shadow.
            if names.iter().any(|n| ctx.init_names.contains(n)) {
                return Err(TypeError::InvalidOperation {
                    operation: format!(
                        "`let {}` shadows an `init` — the init is seeded once and \
                         immutable for the run; choose another name",
                        names.iter().find(|n| ctx.init_names.contains(*n)).unwrap()
                    ),
                    type_name: "init".into(),
                });
            }
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
                Some(e) => {
                    // 2026-08-14 (generic `defn f<T>`): a nullary generic
                    // (`new_stack<T>()`) with no type-constraining args binds
                    // its param from the let annotation (`let s: Stack<Int> =
                    // new_stack()`). Seed the expected type transiently.
                    let saved = ctx.expected_call_type.take();
                    ctx.expected_call_type = ty.clone();
                    let r = infer_type_only(e, ctx);
                    ctx.expected_call_type = saved;
                    r?
                }
                None => ty.clone().unwrap_or(Type::int()),
            };
            // 2026-08-14 (Iterable protocol, slice 5, SPEC §16.3): a list
            // literal in a BINDING must carry a collection type annotation —
            // `let xs: List<Int> = [1, 2, 3]` (the type-directed literal →
            // `op Init`/`op InsertAt`). An unconstrained `let xs = [1, 2, 3]`
            // has no ops to construct through (the compiler holds no List
            // layout); it is a compile error, never a silent fallback.
            if ty.is_none() && matches!(expr, Some(Expr::List(_))) {
                return Err(TypeError::InvalidOperation {
                    operation: "list literal in a `let` binding".into(),
                    type_name: format!(
                        "unconstrained list literal requires a collection type annotation \
                         (e.g. `let xs: List<Int> = [...]`) so it can construct through the \
                         collection's own ops"
                    ),
                });
            }
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
            // 2026-08-09 (init kind, Phase 2): an `init` is seeded exactly once
            // (its declaration) and is immutable for the run — any later write
            // to it is a compile error, not a runtime rebind.
            if let Expr::Identifier(target_name) = lhs {
                if ctx.init_names.contains(target_name) {
                    return Err(TypeError::InvalidOperation {
                        operation: format!(
                            "assignment to `{}` — an `init` is seeded once before \
                             beginprogram and is immutable for the run; declare a \
                             `let` for mutable state",
                            target_name
                        ),
                        type_name: "init".into(),
                    });
                }
            }
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
            // 2026-08-09 (Phase 12, SPEC §18.2): the meld admission is removed —
            // only an explicit coercion path admits the pair.
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
                        // 2026-08-09 (init kind, Phase 2): `init <- v` would
                        // rebind an immutable seeded value — reject it like `=`.
                        if let Expr::Identifier(target_name) = t.as_ref() {
                            if ctx.init_names.contains(target_name) {
                                return Err(TypeError::InvalidOperation {
                                    operation: format!(
                                        "arrow write to `{}` — an `init` is seeded once \
                                         before beginprogram and is immutable for the run",
                                        target_name
                                    ),
                                    type_name: "init".into(),
                                });
                            }
                        }
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
        // 2026-08-13 (layout-keywords plan Phase 4): `trap;` is a never-type —
        // it aborts, so it needs no value and unifies with any expected type
        // (SPEC §8.8).
        Statement::Trap => Ok(()),
        Statement::Term(val) | Statement::EndProgram(val) => {
            if let Some(val) = val {
                let vty = infer_type_only(val, ctx)?;
                // 2026-07-31 (Phase 2): a declared return type must match the
                // term value — no implicit coercion. A declared meld
                // (2026-08-03, P3) admits the pair instead.
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
        Statement::Rollback(_) => Ok(()),
        Statement::Foreach { item, list, body } => {
            let list_ty = infer_type_only(list, ctx)?;
            // 2026-08-12 (Iterable protocol, Tier 2, SPEC §11.4): the item
            // type is the collection's ELEMENT type — the `op At` op-as-member
            // return, substituted with the concrete generic args. Never a
            // hardcoded Int.
            let element_ty = foreach_item_type(ctx, &list_ty);
            ctx.bindings.insert(item.clone(), element_ty);
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
        Statement::Defer(body) | Statement::Mutex(body) => {
            for stmt in body {
                infer_statement(stmt, ctx)?;
            }
            Ok(())
        }
        Statement::Barrier { body, .. } => {
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
/// 2026-08-05 (Phase 6): whether a declaration's contract is required.
/// `node`/`txn`/`asm` require a present, non-trivial contract; `defn` is
/// optional; `cell` is not checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContractKind {
    Optional,
    Required,
}

/// 2026-08-05 (Phase 6): collect `(kind, declaration-name, contract)` for every
/// contract-carrying declaration, including member transactions nested inside
/// `obj`/`cell`/`type` bodies.
fn collect_contract_checks<'a>(
    item: &'a TopLevel,
    out: &mut Vec<(ContractKind, &'a str, &'a Contract)>,
) {
    match item {
        TopLevel::Definition(d) => out.push((ContractKind::Optional, &d.name, &d.contract)),
        TopLevel::Transaction(t) => out.push((ContractKind::Required, &t.name, &t.contract)),
        TopLevel::AsmFn(a) => out.push((ContractKind::Required, &a.name, &a.contract)),
        TopLevel::Export(e) => collect_contract_checks(&e.inner, out),
        TopLevel::TypeDef(t) => {
            for m in &t.body.members {
                collect_contract_checks(m, out);
            }
        }
        TopLevel::Obj(o) => {
            for t in &o.transactions {
                out.push((ContractKind::Required, &t.name, &t.contract));
            }
        }
        TopLevel::Cell(c) => {
            for t in &c.transactions {
                out.push((ContractKind::Required, &t.name, &t.contract));
            }
        }
        TopLevel::Fuzzed { item, .. } => collect_contract_checks(item, out),
        TopLevel::SyncGroup { item, .. } => collect_contract_checks(item, out),
        _ => {}
    }
}

/// 2026-08-05 (Phase 6): validate contract obligations. `node`/`txn`/`asm`
/// require a present, non-trivial contract; `defn` contracts are optional but
/// an explicit `[true][true]` is rejected everywhere.
fn check_contract_obligations(items: &[TopLevel], errors: &mut Vec<TypeError>) {
    let mut contract_checks = Vec::new();
    for item in items {
        collect_contract_checks(item, &mut contract_checks);
    }
    for (kind, name, contract) in contract_checks {
        validate_contract(kind, name, contract, errors);
    }
}

/// 2026-08-05 (Phase 5): validate trait declarations and `impl` blocks.
/// - Every `impl X` must target a declared type (coherence).
/// - A type with an explicitly asserted trait must provide the trait's
///   required functions, logical fields, and op bindings (structural
///   conformance; defaults never satisfy a trait's own requirements).
fn check_trait_declarations(items: &[TopLevel], errors: &mut Vec<TypeError>) {
    let mut traits: std::collections::HashMap<&str, &TraitDef> = std::collections::HashMap::new();
    let mut declared: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut impls: std::collections::HashMap<&str, Vec<&ImplDef>> = std::collections::HashMap::new();
    let mut type_defs: Vec<&TypeDef> = Vec::new();
    for item in items {
        match item {
            TopLevel::Trait(t) => {
                traits.insert(&t.name, t);
            }
            TopLevel::Impl(i) => {
                impls.entry(i.target.as_str()).or_default().push(i);
            }
            TopLevel::TypeDef(t) => {
                declared.insert(&t.name);
                type_defs.push(t);
            }
            TopLevel::StaticStruct(s) => {
                declared.insert(&s.name);
            }
            TopLevel::Enum(e) => {
                declared.insert(&e.name);
            }
            TopLevel::Obj(o) => {
                declared.insert(&o.name);
            }
            TopLevel::Export(e) => match e.inner.as_ref() {
                TopLevel::TypeDef(t) => {
                    declared.insert(&t.name);
                    type_defs.push(t);
                }
                _ => {}
            },
            _ => {}
        }
    }
    // Impl coherence: every impl targets a declared type.
    for (target, _) in &impls {
        if !declared.contains(*target) {
            errors.push(TypeError::InvalidOperation {
                operation: format!("impl"),
                type_name: (*target).to_string(),
            });
        }
    }
    // Structural conformance for explicitly asserted traits.
    let assertions: Vec<(&&TypeDef, &str)> = type_defs
        .iter()
        .flat_map(|t| t.traits.iter().map(move |n| (t, n.as_str())))
        .collect();
    for (t, trait_name) in assertions {
        check_trait_assertion(t, trait_name, &traits, &impls, errors);
    }
}

/// 2026-08-05 (Phase 5): verify one explicitly asserted trait on a type. The
/// type must provide each of the trait's required functions and op bindings.
fn check_trait_assertion<'a>(
    t: &TypeDef,
    trait_name: &str,
    traits: &std::collections::HashMap<&'a str, &'a TraitDef>,
    impls: &std::collections::HashMap<&str, Vec<&ImplDef>>,
    errors: &mut Vec<TypeError>,
) {
    let Some(trait_def) = traits.get(trait_name) else {
        errors.push(TypeError::UndefinedVariable {
            name: trait_name.to_string(),
            available: Vec::new(),
        });
        return;
    };
    let type_impls = impls.get(t.name.as_str()).cloned().unwrap_or_default();
    let provided_fns = type_function_names(t, type_impls.clone());
    for f in &trait_def.functions {
        if f.body.is_empty() && !provided_fns.contains(f.name.as_str()) {
            errors.push(TypeError::InvalidOperation {
                operation: format!("trait '{}' requires '{}'", trait_name, f.name),
                type_name: t.name.clone(),
            });
        }
    }
    let provided_ops = type_op_names(t, type_impls);
    for op in &trait_def.op_bindings {
        if !provided_ops.contains(&op.name) {
            errors.push(TypeError::InvalidOperation {
                operation: format!("trait '{}' requires op '{}'", trait_name, op.name),
                type_name: t.name.clone(),
            });
        }
    }
}

/// 2026-08-05 (Phase 5): the function names a type provides — its own defn
/// members plus the functions attached through `impl` blocks.
fn type_function_names(t: &TypeDef, impls: Vec<&ImplDef>) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for m in &t.body.members {
        if let TopLevel::Definition(d) = m {
            names.insert(d.name.clone());
        }
    }
    names.extend(
        impls
            .iter()
            .flat_map(|i| i.functions.iter().map(|f| f.name.clone())),
    );
    names
}

/// 2026-08-05 (Phase 5): the op binding names a type provides — its own
/// bindings plus those attached through `impl` blocks.
fn type_op_names(t: &TypeDef, impls: Vec<&ImplDef>) -> std::collections::HashSet<String> {
    let mut names: std::collections::HashSet<String> = t
        .body
        .op_bindings
        .iter()
        .map(|op| op.name.clone())
        .collect();
    names.extend(
        impls
            .iter()
            .flat_map(|i| i.op_bindings.iter().map(|op| op.name.clone())),
    );
    names
}

/// 2026-08-05 (Phase 6): a contract is a tautology when both pre and post are
/// the literal `true` expression — `true ⇒ true` holds trivially.
fn is_tautological(contract: &Contract) -> bool {
    matches!(contract.pre_condition, Expr::Bool(true))
        && matches!(contract.post_condition, Expr::Bool(true))
}

/// 2026-08-05 (Phase 6): validate one declaration's contract.
fn validate_contract(
    kind: ContractKind,
    name: &str,
    contract: &Contract,
    errors: &mut Vec<TypeError>,
) {
    if kind == ContractKind::Optional && contract.explicit && is_tautological(contract) {
        errors.push(TypeError::TautologicalContract);
        return;
    }
    if kind != ContractKind::Required {
        return;
    }
    if !contract.explicit {
        errors.push(TypeError::MissingContract {
            declaration: name.to_string(),
        });
    } else if is_tautological(contract) {
        errors.push(TypeError::TautologicalContract);
    }
}

pub fn check_program(items: &mut [TopLevel], universe: &TypeUniverse) -> Result<(), Vec<TypeError>> {
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

    // 2026-08-09 (init kind, Phase 2): init names → type, collected SEPARATELY
    // from state_bindings so init is read-only (not a mutable `state_key`) but
    // still visible to every txn/defn body that reads it.
    let init_bindings: std::collections::HashMap<String, Type> = items
        .iter()
        .filter_map(|item| match item {
            TopLevel::Init(i) => Some((i.name.clone(), i.ty.clone())),
            _ => None,
        })
        .collect();

    let mut errors = Vec::new();

    // 2026-08-09 (init kind, Phase 2): set-once — an `init` name may be
    // declared exactly once (its seeding IS the one write). A duplicate
    // declaration is a compile error, as is re-seeding via a later `init`.
    {
        let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, item) in items.iter().enumerate() {
            if let TopLevel::Init(init) = item {
                match seen.entry(init.name.as_str()) {
                    std::collections::hash_map::Entry::Occupied(_) => {
                        errors.push(TypeError::InvalidOperation {
                            operation: format!(
                                "duplicate `init` declaration for '{}' — an init is seeded \
                                 exactly once before beginprogram",
                                init.name
                            ),
                            type_name: "init".into(),
                        });
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(i);
                    }
                }
            }
        }
    }

    // 2026-08-09 (init kind, Phase 2): ordering — seeding runs before
    // beginprogram, so every `init` declaration must precede the program's
    // entry loop. An init after a beginprogram marker would seed too late.
    {
        let mut begin_seen = false;
        for item in items.iter() {
            if contains_beginprogram_in_item(item) {
                begin_seen = true;
            } else if let TopLevel::Init(init) = item {
                if begin_seen {
                    errors.push(TypeError::InvalidOperation {
                        operation: format!(
                            "`init {}` appears after a beginprogram entry — seeding must \
                             happen before beginprogram",
                            init.name
                        ),
                        type_name: "init".into(),
                    });
                }
            }
        }
    }

    // 2026-08-05 (Phase 6): contract obligations.
    // - `defn`: contract optional; an explicit [true][true] is rejected.
    // - `node`/`txn`/`asm`: contract required (present and non-trivial).
    // - `cell`: not required.
    check_contract_obligations(items, &mut errors);
    // 2026-08-05 (Phase 5): trait declarations, impl coherence, and explicit
    // trait conformance.
    check_trait_declarations(items, &mut errors);

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
            // 2026-08-03 (node bridge): transaction return types — `term
            // store_text(name)` on a CStr-returning txn must see CStr, not the
            // Int fallback (which only happened to work for Int-returning txns).
            TopLevel::Transaction(t) => {
                t.output_type.as_ref().and_then(|ot| match ot {
                    OutputType::Single(ty) => Some((t.name.clone(), ty.clone())),
                    _ => None,
                })
            }
            // 2026-07-31 (Phase 2): frgn return types — `term frgn_foo(x)`
            // must see the declared foreign return type, not the Int fallback.
            TopLevel::ForeignBinding(fb) => {
                let briev_name = fb
                    .briev_name
                    .clone()
                    .unwrap_or_else(|| fb.foreign_name.clone());
                fb.success_output
                    .first()
                    .map(|(_, ty)| (briev_name, ty.clone()))
            }
            _ => None,
        }
    }).collect();

    let optional_frgns: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            TopLevel::ForeignBinding(fb) if fb.is_optional => {
                Some(fb.briev_name.clone().unwrap_or_else(|| fb.foreign_name.clone()))
            }
            _ => None,
        })
        .collect();

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

    // 2026-08-14 (generic `defn f<T>` dispatch): a defn's declared type params
    // (e.g. `["T"]` for `defn first<T>`), for call-site inference + substitution.
    let fn_type_params: HashMap<String, Vec<String>> = items
        .iter()
        .filter_map(|item| {
            let name = match item {
                TopLevel::Definition(d) => &d.name,
                TopLevel::Transaction(t) => &t.name,
                TopLevel::Export(e) => match &*e.inner {
                    TopLevel::Definition(d) => &d.name,
                    _ => return None,
                },
                _ => return None,
            };
            let params: Vec<String> = match item {
                TopLevel::Definition(d) => d.type_params.iter().map(|p| p.name.clone()).collect(),
                TopLevel::Transaction(t) => t.type_params.iter().map(|p| p.name.clone()).collect(),
                TopLevel::Export(e) => match &*e.inner {
                    TopLevel::Definition(d) => d.type_params.iter().map(|p| p.name.clone()).collect(),
                    _ => vec![],
                },
                _ => vec![],
            };
            if params.is_empty() {
                None
            } else {
                Some((name.clone(), params))
            }
        })
        .collect();

    // 2026-08-09 (Phase 12, SPEC §18.2): the meld declaration collection is
    // removed — foreign shapes adapt through EXPLICIT protocol cast edges,
    // not an implicit meld admission; `meld_coercible` was removed.

    // 2026-07-27: Pre-collect Parse bindings and type parents from ALL TypeDef items.
    let mut all_parse_bindings: HashMap<String, Vec<OperatorBinding>> = HashMap::new();
    let mut all_type_parents: HashMap<String, String> = HashMap::new();
    for item in items.iter() {
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
    for item in items.iter() {
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

    let defined_fns: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            TopLevel::Definition(d) => Some(d.name.clone()),
            TopLevel::Transaction(t) => Some(t.name.clone()),
            _ => None,
        })
        .collect();

    let env = CheckEnv {
        state_bindings: &state_bindings,
        init_bindings: &init_bindings,
        fn_return_types: &fn_return_types,
        optional_frgns: &optional_frgns,
        fn_param_types: &fn_param_types,
        fn_type_params: &fn_type_params,
        all_parse_bindings: &all_parse_bindings,
        all_type_parents: &all_type_parents,
        all_regular_ops: &all_regular_ops,
        all_regular_bindings: &all_regular_bindings,
        all_type_slots: &all_type_slots,
        all_type_members: &all_type_members,
        all_type_params: &all_type_params,
        all_type_protocols: &all_type_protocols,
        all_cross_ops: &all_cross_ops,
        defined_fns: &defined_fns,
    };

    for item in items.iter() {
        if let Err(e) = check_top_level(item, universe, &env) {
            errors.push(e);
        }
    }

    // 2026-07-31 (A2): Typecheck obj member bodies with `self` + slot names
    // bound. Without this, `len = "hello"` inside a member passes silently.
    for item in items.iter() {
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
                for name in &optional_frgns {
                    mctx.optional_frgns.insert(name.clone());
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
                    // 2026-08-12 (Iterable protocol, op-as-member): an operator
                    // member typechecks exactly like a defn member — self +
                    // slots bound, params + output in scope.
                    TopLevel::TypeDefOperator(d) => {
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

    // 2026-08-06 (beginprogram plan): entry-loop validation — termination
    // (the goal must be provably reachable) and entry conflict (at most one
    // beginprogram node can be eligible at program start).
    let beginprogram_nodes: Vec<(&Transaction, Expr)> = items
        .iter()
        .filter_map(|item| match item {
            TopLevel::Transaction(t) => {
                beginprogram_entry(&t.contract.pre_condition).map(|entry| (t, entry))
            }
            _ => None,
        })
        .collect();
    check_beginprogram_program(&beginprogram_nodes, &mut errors);

    if errors.is_empty() {
        // 2026-08-06 (Phase 5): with types known, elaborate declared ops into
        // calls to their implementations (mutates items in place). Any error
        // (e.g. an op target that is not a defined function) fails the check.
        errors.extend(elaborate_ops(items, universe, &env));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The entry condition of a beginprogram node: the precondition with the
/// `beginprogram` marker removed (the marker takes no conditions; the node's
/// other precondition terms are the state-based entry gate). Returns None for
/// non-beginprogram nodes.
fn beginprogram_entry(pre: &Expr) -> Option<Expr> {
    match pre {
        Expr::BeginProgram => Some(Expr::Bool(true)),
        Expr::BinaryOp(BinaryOpKind::And, a, b) => {
            let a_begin = contains_beginprogram(a);
            let b_begin = contains_beginprogram(b);
            if a_begin || b_begin {
                let a_entry = if a_begin { Expr::Bool(true) } else { a.as_ref().clone() };
                let b_entry = if b_begin { Expr::Bool(true) } else { b.as_ref().clone() };
                Some(conjoin(a_entry, b_entry))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn contains_beginprogram(e: &Expr) -> bool {
    match e {
        Expr::BeginProgram => true,
        Expr::BinaryOp(BinaryOpKind::And, a, b) => {
            contains_beginprogram(a) || contains_beginprogram(b)
        }
        _ => false,
    }
}

/// 2026-08-09 (init kind, Phase 2): does a top-level item carry a
/// `beginprogram` entry marker? A transaction whose precondition contains
/// `beginprogram` is the program's entry loop — everything after it in source
/// order seeds too late for an `init`.
fn contains_beginprogram_in_item(item: &TopLevel) -> bool {
    match item {
        TopLevel::Transaction(t) => contains_beginprogram(&t.contract.pre_condition),
        TopLevel::Export(e) => contains_beginprogram_in_item(&e.inner),
        _ => false,
    }
}

fn conjoin(a: Expr, b: Expr) -> Expr {
    match (a, b) {
        (Expr::Bool(true), b) => b,
        (a, Expr::Bool(true)) => a,
        (a, b) => Expr::BinaryOp(BinaryOpKind::And, Box::new(a), Box::new(b)),
    }
}

/// Entry-loop validation: every beginprogram node's goal (postcondition) must
/// be provably reachable, and at most one beginprogram node may be eligible at
/// program start.
fn check_beginprogram_program(nodes: &[(&Transaction, Expr)], errors: &mut Vec<TypeError>) {
    for (txn, _entry) in nodes {
        if let Err(msg) = check_goal_reachable(&txn.body, &txn.contract.post_condition) {
            errors.push(TypeError::InvalidOperation {
                operation: format!(
                    "beginprogram node '{}': {}",
                    txn.name, msg
                ),
                type_name: "beginprogram".into(),
            });
        }
    }
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            if entries_may_both_hold(&nodes[i].1, &nodes[j].1) {
                errors.push(TypeError::InvalidOperation {
                    operation: format!(
                        "beginprogram nodes '{}' and '{}' have conflicting entry conditions — at most one may fire at program start",
                        nodes[i].0.name, nodes[j].0.name
                    ),
                    type_name: "beginprogram".into(),
                });
            }
        }
    }
}

/// The goal must be provably reachable: `[true]` runs exactly once; a
/// comparison over a counter (`[i == N]`, `[i >= N]`, `[i <= N]`, ...) whose
/// body advances the counter toward the goal terminates; anything else is
/// unprovable ⇒ compile error.
fn check_goal_reachable(body: &[Statement], goal: &Expr) -> Result<(), String> {
    match goal {
        Expr::Bool(true) => Ok(()),
        Expr::Bool(false) => Err("goal '[false]' is never reachable — the entry loop cannot halt".into()),
        Expr::BinaryOp(op, left, right) => counter_goal_reachable(body, op, left, right),
        _ => Err(unreachable_goal(goal)),
    }
}

/// A comparison goal over a counter is reachable when the body advances the
/// counter toward the bound (increment for an increasing goal, decrement for
/// a decreasing one).
fn counter_goal_reachable(
    body: &[Statement],
    op: &BinaryOpKind,
    left: &Expr,
    right: &Expr,
) -> Result<(), String> {
    let goal = Expr::BinaryOp(*op, Box::new(left.clone()), Box::new(right.clone()));
    let var = match left {
        Expr::Identifier(v) => v,
        _ => return Err(unreachable_goal(&goal)),
    };
    if !right_side_terminal(right) {
        return Err(unreachable_goal(&goal));
    }
    let increasing = matches!(op, BinaryOpKind::Eq | BinaryOpKind::Ge | BinaryOpKind::Gt);
    let decreasing = matches!(op, BinaryOpKind::Eq | BinaryOpKind::Le | BinaryOpKind::Lt);
    if increasing && body_advances_counter(body, var, true) {
        return Ok(());
    }
    if decreasing && body_advances_counter(body, var, false) {
        return Ok(());
    }
    Err(unreachable_goal(&goal))
}

fn unreachable_goal(goal: &Expr) -> String {
    format!(
        "goal '{}' is not provably reachable — the body must advance a counter toward it (or use '[true]' for a single pass)",
        goal
    )
}

/// Whether the right side of a goal comparison is a literal/state reference
/// (a bound the counter advances toward).
fn right_side_terminal(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Decimal(_) | Expr::Identifier(_) | Expr::Char(_)
    )
}

/// Whether the body advances `var` toward a bound: `var = var + d` for an
/// increasing goal, `var = var - d` for a decreasing one.
fn body_advances_counter(body: &[Statement], var: &str, increasing: bool) -> bool {
    body.iter().any(|stmt| match stmt {
        Statement::Assign(lhs, rhs) => match lhs {
            Expr::Identifier(n) => n == var && counter_advance(rhs, var, increasing),
            _ => false,
        },
        _ => false,
    })
}

/// `var = var ± d` with a positive literal delta in the goal's direction.
fn counter_advance(rhs: &Expr, var: &str, increasing: bool) -> bool {
    match rhs {
        Expr::BinaryOp(BinaryOpKind::Add, a, b) => {
            increasing
                && ((matches!(a.as_ref(), Expr::Identifier(v) if v == var) && literal_positive(b))
                    || (matches!(b.as_ref(), Expr::Identifier(v) if v == var)
                        && literal_positive(a)))
        }
        Expr::BinaryOp(BinaryOpKind::Sub, a, b) => {
            !increasing
                && matches!(a.as_ref(), Expr::Identifier(v) if v == var)
                && literal_positive(b)
        }
        _ => false,
    }
}

fn literal_positive(e: &Expr) -> bool {
    matches!(e, Expr::Decimal(n) if *n > 0)
}

/// Whether two beginprogram entry conditions may both hold at program start.
/// Conservative: unconditional (both `true`) or syntactically identical
/// conditions conflict; distinct conditions are assumed mutually exclusive
/// (a full satisfiability proof is a follow-up).
fn entries_may_both_hold(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Bool(true), _) | (_, Expr::Bool(true)) => true,
        _ => a == b,
    }
}

/// Type-check a top-level item.
/// 2026-08-03 (P1.4): extract the binding function name from a cross-op's
/// `impl_args` (`= cstring_concat(#Lh, #Rh)` → "cstring_concat"). Accepts a bare
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

/// 2026-08-06 (Phase 5): the pre-collected typecheck maps, bundled so
/// `check_top_level` and `elaborate_ops` share one signature (Praetor rule 4).
struct CheckEnv<'a> {
    state_bindings: &'a HashMap<String, Type>,
    /// 2026-08-09 (init kind, Phase 2): runtime-seeded invariant names → type.
    /// Reads resolve through ctx.bindings; writes are rejected by the
    /// `init_names` reassign check (the seeding is the one write).
    init_bindings: &'a HashMap<String, Type>,
    fn_return_types: &'a HashMap<String, Type>,
    optional_frgns: &'a std::collections::HashSet<String>,
    fn_param_types: &'a HashMap<String, Vec<Type>>,
    /// 2026-08-14 (generic `defn f<T>`): declared type params per defn.
    fn_type_params: &'a HashMap<String, Vec<String>>,
    all_parse_bindings: &'a HashMap<String, Vec<OperatorBinding>>,
    all_type_parents: &'a HashMap<String, String>,
    all_regular_ops: &'a HashMap<String, Vec<crate::ast::top::OperatorDef>>,
    all_regular_bindings: &'a HashMap<String, Vec<crate::ast::top::OperatorBinding>>,
    all_type_slots: &'a HashMap<String, Vec<crate::ast::top::TypeDefSlot>>,
    all_type_members: &'a HashMap<String, Vec<TopLevel>>,
    all_type_params: &'a HashMap<String, Vec<String>>,
    all_type_protocols: &'a HashMap<String, String>,
    all_cross_ops: &'a HashMap<String, HashMap<String, String>>,
    /// 2026-08-06 (diagnostics): every defined function/transaction name.
    defined_fns: &'a std::collections::HashSet<String>,
}

/// Build a typecheck context from the pre-collected maps. Shared by
/// `check_top_level` (typechecking) and `elaborate_ops` (Phase 5 op lowering).
fn make_typecheck_context<'a>(env: &CheckEnv<'a>, universe: &'a TypeUniverse) -> TypecheckContext<'a> {
    let mut ctx = TypecheckContext::new(universe);
    // 2026-07-27: Inject pre-collected parse bindings and type parents.
    for (type_name, bindings) in env.all_parse_bindings {
        ctx.register_parse_bindings(type_name, bindings.clone(), None);
    }
    ctx.type_parents = env.all_type_parents.clone();
    // 2026-07-31: Inject regular operator declarations for cross-type overloads.
    ctx.regular_ops = env.all_regular_ops.clone();
    ctx.regular_bindings = env.all_regular_bindings.clone();
    // 2026-07-31: Inject struct/obj slots and members for field/method access.
    ctx.type_slots = env.all_type_slots.clone();
    ctx.type_members = env.all_type_members.clone();
    ctx.type_params = env.all_type_params.clone();
    ctx.fn_param_types = env.fn_param_types.clone();
    ctx.fn_type_params = env.fn_type_params.clone();
    ctx.type_protocols = env.all_type_protocols.clone();
    ctx.variant_cross_ops = env.all_cross_ops.clone();
    // 2026-07-14: Inject state variable bindings so transactions/defns can reference them.
    for (name, ty) in env.state_bindings {
        ctx.bindings.insert(name.clone(), ty.clone());
        ctx.state_keys.insert(name.clone());
    }
    // 2026-08-09 (init kind, Phase 2): init names are readable everywhere but
    // NOT mutable — bind them, mark them init_names (reassign → error), and
    // keep them out of state_keys so &init yields Ptr<const T>.
    for (name, ty) in env.init_bindings {
        ctx.bindings.insert(name.clone(), ty.clone());
        ctx.init_names.insert(name.clone());
    }
    // 2026-07-25: Inject function return types for call inference.
    for (name, ty) in env.fn_return_types {
        ctx.fn_return_types.insert(name.clone(), ty.clone());
    }
    for name in env.optional_frgns {
        ctx.optional_frgns.insert(name.clone());
    }
    ctx.defined_fns = env.defined_fns.clone();
    ctx
}

fn check_top_level<'a>(
    item: &TopLevel,
    universe: &'a TypeUniverse,
    env: &CheckEnv<'a>,
) -> Result<(), TypeError> {
    let mut ctx = make_typecheck_context(env, universe);
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
        TopLevel::Export(e) => check_top_level(&e.inner, universe, env),
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
    let slot_ty = slots.iter().find(|s| s.name == field).map(|s| s.ty.clone())?;
    // 2026-08-07 (object instance pools): an Applied generic obj's member
    // keeps RAW const dims (`data: T[M]` → `Named("M", 0)`); substitute the
    // concrete args so `b.data[2]` resolves to Int (M → Number(5) → 5).
    if let Type::Applied(base, args) = receiver {
        let params = ctx.type_params.get(base).cloned().unwrap_or_default();
        let subst: std::collections::HashMap<String, Type> =
            params.into_iter().zip(args.iter().cloned()).collect();
        return Some(substitute_type(&slot_ty, &subst));
    }
    Some(slot_ty)
}

/// 2026-07-31: Reflection table (D1). `^` = runtime, `^^` = compile-time.
/// A target used with the wrong kind is an error; an unknown target is an
/// error. `Len`/`Ptr` are runtime reads; `Size`/`Bytes`/`Alignment`/`Type`/
/// `Element` are compile-time foldable descriptors. 2026-08-14 (Boxed Cat,
/// tri-partite rule): runtime `.^Size` is DELETED — the element count of a
/// collection is an operation, so its home is the `Count#` intrinsic, not a
/// reflection target (§6a/§6b of the 2026-08-14 plan). `.^^Size` (compile
/// time) keeps the vector shape.
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
        "Length" => {
            if is_compile_time {
                return Err(wrong_kind("runtime"));
            }
            // 2026-08-12 (Iterable protocol): `.^Length` is STORED-length
            // reflection (SPEC §17.1) — the String byte header, the Data byte
            // header, the Vector element count. A COLLECTION's count is
            // member-managed (`op Count`), and a computed count (a String's
            // UTF8 chars) is an intrinsic (`CharCount#`) — neither is
            // `.^Length`, so both are compile errors, never silent.
            match receiver {
                Type::Custom(n) if n == "String" || n == "Data" => Ok(Type::int()),
                Type::Vector(..) => Ok(Type::int()),
                Type::Applied(..) => Err(TypeError::InvalidOperation {
                    operation: "reflection target 'Length'".into(),
                    type_name: format!(
                        "{} has no INTRINSIC length — its count is member-managed; use `op Count`",
                        receiver
                    ),
                }),
                Type::Custom(_) => Err(TypeError::InvalidOperation {
                    operation: "reflection target 'Length'".into(),
                    type_name: format!(
                        "type {} has no intrinsic (stored) length; a computed count is an intrinsic (`CharCount#`)",
                        receiver
                    ),
                }),
                _ => Err(TypeError::InvalidOperation {
                    operation: format!("reflection target 'Length'"),
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
        // 2026-08-14 (boundary plan, SPEC §17.3): `.^Absolute` is REMOVED —
        // abs is a computed truth, so its home is the `Abs#` intrinsic, not a
        // reflection target. The deprecated alias release has passed; it is
        // now an unknown-target error (the catch-all below), directing to
        // `Abs#`. (Was: `x.^Absolute` → the receiver's type, 2026-08-04.)
        // 2026-08-14 (Boxed Cat, iterable-protocol §10.4, tri-partite rule):
        // `.^Size` (runtime) is DELETED — the element count of a collection is
        // an OPERATION, so its home is the `Count#` intrinsic, not a reflection
        // target. `.^^Size` (compile-time) keeps the vector shape and stays
        // Int. Runtime `.^Size` is a kind error, exactly like `Bytes`.
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
            // 2026-08-06 (Phase 8): `x.^^Type` is a frozen descriptor — the
            // protocol category code, an i64 (matching interpreter + codegen).
            Ok(Type::int())
        }
        _ => {
            // 2026-08-14 (boundary plan, SPEC §17.3): the removed `.^Absolute`
            // gets a targeted hint directing to `Abs#`.
            let hint = if target == "Absolute" {
                " — `.^Absolute` was removed; abs is a computed truth, use the `Abs#` intrinsic"
            } else {
                ""
            };
            Err(TypeError::InvalidOperation {
                operation: format!("reflection target '{}'", target),
                type_name: format!(
                    "unknown reflection target — expected Length, Ptr, Size, Bytes, Alignment, or Type{}",
                    hint
                ),
            })
        }
    }
}

/// 2026-07-31: Resolve `a.m(args)` — find the member on the receiver's obj
/// type, substitute the receiver's type arguments for the obj's type
/// parameters, validate each arg against the substituted parameter types, and
/// return the member's (substituted) result type.
/// 2026-08-14 (UOL §6b.2): UFCS priority — literal member wins, then a
/// generative op (`a.At#(i)` → `At#(a, i)`), then a plain top-level function
/// with the receiver prepended (`a.f(x)` → `f(a, x)`). A trailing `#` is
/// stripped for the member lookup.
fn resolve_method_call(
    recv: &Expr,
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
    let lookup_name = name.trim_end_matches('#');
    let members = ctx.type_members.get(type_name).cloned().unwrap_or_default();
    let member = members.iter().find(|m| member_name(m) == lookup_name).cloned();
    let member = match member {
        Some(m) => m,
        // 2026-08-14 (UOL §6b.2): UFCS fallback — `a.OpName#(b)` → `OpName#(a, b)`
        // (generative op or registered intrinsic), or `a.f(b)` → `f(a, b)` for a
        // plain top-level function.
        None => {
            let mut all = vec![(*recv).clone()];
            all.extend(args.iter().cloned());
            if name.ends_with('#') {
                // 2026-08-14 (UOL §6b): keep the `#` — `a.Add#(b)` resolves via
                // the intrinsic signature / generative op identity, never a
                // bare top-level function.
                return infer_call(name, &all, ctx);
            }
            if ctx.fn_return_types.contains_key(name) {
                return infer_call(name, &all, ctx);
            }
            return Err(TypeError::InvalidOperation {
                operation: format!("method call '.{}()'", name),
                type_name: format!("type '{}' has no member '{}'", type_name, name),
            });
        }
    };
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
    let params = member_params(&member);
    let out = member_output(&member);
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
        TopLevel::TypeDefOperator(d) => d.name.clone(),
        _ => String::new(),
    }
}

/// 2026-08-12 (Iterable protocol, op-as-member): find an operator member
/// (`op Name(...) { … }`) on a type by its operator name. The operator IS the
/// member — no binding RHS, no bare member-name indirection (SPEC §15.2).
/// 2026-08-14 (UOL §6b.1): the disclosed operation identities — the intrinsic
/// forms (`OpName#`) of every operation. Mirrors `operation_identities` in
/// `src/vocab.rs` and `is_operation_identity` in `src/backend/llvm/intrinsics.rs`;
/// kept in lockstep with both.
fn is_operation_identity(name: &str) -> bool {
    matches!(name,
        "Add" | "Sub" | "Mul" | "Div" | "Rem" | "Neg" | "Abs"
        | "Eq" | "Neq" | "Lt" | "Le" | "Gt" | "Ge"
        | "And" | "Or" | "Not"
        | "BitAnd" | "BitOr" | "BitXor" | "BitNot" | "Shl" | "Shr"
        | "At" | "Slice" | "InsertAt" | "ExtractFrom" | "CopyFrom"
        | "Append" | "Prepend"
        | "Count" | "Iter" | "Step" | "IsEnd" | "Current")
}

/// 2026-08-14 (UOL §6b.2): a generative `OpName#(recv, args…)` call — the
/// receiver type must declare the op member (`op At`, `op Count`, …); the
/// return type is the op member's output substituted with the concrete
/// generic args. `Some(ty)` when the op is declared; `Ok(None)` when the
/// receiver is a `#String` and the op is `Count` (its element count is the
/// `CharCount#` scan, `Native("Int")`); `Err` when the op is undeclared.
fn infer_generative_op_call(
    op_name: &str,
    args: &[Expr],
    ctx: &mut TypecheckContext,
) -> Result<Option<Type>, TypeError> {
    let recv = args.first();
    let Some(recv) = recv else {
        return Ok(None);
    };
    let recv_ty = infer_type_only(recv, ctx)?;
    // A `#String` operand has no `op Count` — its element count is the char
    // scan (CharCount#), so `Count#` on it is Int.
    if op_name == "Count" && ctx.operand_implements_protocol(&recv_ty, "#String") {
        return Ok(Some(Type::int()));
    }
    let base = match &recv_ty {
        Type::Custom(n) => n.clone(),
        Type::Applied(n, _) => n.clone(),
        _ => {
            return Err(TypeError::InvalidOperation {
                operation: format!("call to '{}#',", op_name),
                type_name: format!(
                    "receiver type '{}' has no operator members",
                    recv_ty
                ),
            });
        }
    };
    let members = ctx.type_members.get(&base).cloned().unwrap_or_default();
    let member = operator_member(&members, op_name).ok_or_else(|| {
        TypeError::InvalidOperation {
            operation: format!("call to '{}#',", op_name),
            type_name: format!(
                "`{}#` requires the receiver type '{}' to declare `op {}`",
                op_name, recv_ty, op_name
            ),
        }
    })?;
    // Infer the return from the op member's output, substituted with the
    // receiver's concrete generic args (same evidence as resolve_element_type).
    let args_of_recv = match &recv_ty {
        Type::Applied(_, a) => a.clone(),
        _ => Vec::new(),
    };
    let params = ctx.type_params.get(&base).cloned().unwrap_or_default();
    let subst: std::collections::HashMap<String, Type> =
        params.into_iter().zip(args_of_recv).collect();
    let ret = member
        .output_type
        .as_ref()
        .and_then(|o| o.all_types().into_iter().next())
        .map(|t| substitute_type(&t, &subst))
        .unwrap_or_else(Type::void);
    Ok(Some(ret))
}

fn operator_member<'a>(members: &'a [TopLevel], op: &str) -> Option<&'a Definition> {
    members.iter().find_map(|m| match m {
        TopLevel::TypeDefOperator(d) if d.name == op => Some(d),
        _ => None,
    })
}

/// 2026-08-14 (boundary plan, SPEC §17.2): the ELEMENT type of an iterable
/// receiver, single-source proof form. `Some(ty)` only for genuine iterables:
/// a `#String` operand → `Char` (frozen protocol fact), a Tier-2/1 type → the
/// read op's return substituted with the concrete generic args, a vector → the
/// inner type. `None` for everything else — a non-iterable `.^^Element` is a
/// compile error, never a silent Int. This is the SAME evidence `foreach_item_type`
/// reads for the foreach binding, so `.^^Element` and `foreach` cannot drift.
fn resolve_element_type(ctx: &TypecheckContext, ty: &Type) -> Option<Type> {
    if ctx.operand_implements_protocol(ty, "#String") {
        return Some(Type::Custom("Char".to_string()));
    }
    let (base, args) = match ty {
        Type::Custom(n) => (n.clone(), Vec::new()),
        Type::Applied(n, a) => (n.clone(), a.clone()),
        Type::Vector(inner, _) => return Some((**inner).clone()),
        _ => return None,
    };
    let members = ctx.type_members.get(&base).cloned().unwrap_or_default();
    let read = operator_member(&members, "At").or_else(|| operator_member(&members, "Current"));
    let read = read?;
    let raw = read.output_type.as_ref()?.all_types().into_iter().next()?;
    let params = ctx.type_params.get(&base).cloned().unwrap_or_default();
    let subst: std::collections::HashMap<String, Type> = params.into_iter().zip(args).collect();
    Some(substitute_type(&raw, &subst))
}

/// 2026-08-12 (Iterable protocol, Tier 2): the ELEMENT type of a `foreach`
/// iterable — the type's `op At` op-as-member return, substituted with the
/// concrete generic args (`List<String>` At → `T` → `String`). Falls back to
/// the inner type for vectors and Int for scalars/ranges — structural, never
/// a collection name. 2026-08-14 (String unification): a `#String` protocol
/// operand is `Iterable<Char>` — Char is the observed element type (SPEC
/// §17.2), a frozen protocol fact, never a name match.
fn foreach_item_type(ctx: &TypecheckContext, list_ty: &Type) -> Type {
    let (base, args) = match list_ty {
        Type::Custom(n) => (n.clone(), Vec::new()),
        Type::Applied(n, a) => (n.clone(), a.clone()),
        Type::Vector(inner, _) => return (**inner).clone(),
        _ => return Type::int(),
    };
    if ctx.operand_implements_protocol(list_ty, "#String") {
        return Type::Custom("Char".to_string());
    }
    let members = ctx.type_members.get(&base).cloned().unwrap_or_default();
    // Tier 2 first (op At), then Tier 1 (op Current) — the element type is the
    // read op's return with the concrete args substituted.
    let read = operator_member(&members, "At").or_else(|| operator_member(&members, "Current"));
    if let Some(read) = read {
        if let Some(raw) = read.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()) {
            let params = ctx.type_params.get(&base).cloned().unwrap_or_default();
            let subst: std::collections::HashMap<String, Type> =
                params.into_iter().zip(args).collect();
            return substitute_type(&raw, &subst);
        }
    }
    Type::int()
}

fn member_params(m: &TopLevel) -> Vec<Type> {
    match m {
        TopLevel::Transaction(t) => t.parameters.iter().map(|(_, ty)| ty.clone()).collect(),
        TopLevel::Definition(d) => d.parameters.iter().map(|(_, ty)| ty.clone()).collect(),
        TopLevel::TypeDefOperator(d) => d.parameters.iter().map(|(_, ty)| ty.clone()).collect(),
        _ => Vec::new(),
    }
}

fn member_output(m: &TopLevel) -> Option<Type> {
    match m {
        TopLevel::Transaction(t) => t.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()),
        TopLevel::Definition(d) => d.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()),
        TopLevel::TypeDefOperator(d) => d.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()),
        _ => None,
    }
}

/// 2026-08-01 (Phase 4): the compiler-known stream symbols. `#StdOut <- value`
/// and `#StdErr <- value` are stream WRITES (lowered to the print family);
/// `#StdIn` is a stream handle value (the trg read composition).
fn is_stream_symbol(name: &str) -> bool {
    matches!(name, "#StdOut" | "#StdErr" | "#StdIn")
}

/// 2026-08-14 (generic `defn f<T>` dispatch): infer the concrete type args of
/// a generic defn from its call-site arguments. Each parameter type is
/// unified against the corresponding argument type, binding the defn's type
/// params (`List<T>` vs `List<Int>` → `T = Int`). Returns the concrete types
/// in the defn's declared param order, or `Ok(None)` if inference is
/// inconclusive (defers to the raw-type fallback). A mismatch that is NOT a
/// type-param variance is a normal type error.
fn infer_defn_type_args(
    name: &str,
    params: &[String],
    args: &[Expr],
    ctx: &mut TypecheckContext,
) -> Result<Option<Vec<Type>>, TypeError> {
    let param_tys = ctx.fn_param_types.get(name).cloned().unwrap_or_default();
    if param_tys.len() != args.len() {
        return Ok(None);
    }
    let mut bindings: std::collections::HashMap<String, Option<Type>> =
        params.iter().map(|p| (p.clone(), None)).collect();
    for (i, arg) in args.iter().enumerate() {
        let arg_ty = infer_type_only(arg, ctx)?;
        let param_ty = &param_tys[i];
        if !unify_defn_type(param_ty, &arg_ty, &mut bindings) {
            return Err(TypeError::TypeMismatch {
                expected: format!("{}", param_ty),
                found: format!("{}", arg_ty),
                context: format!("argument {} of '{}'", i, name),
            });
        }
    }
    // The inferred concrete types in the defn's declared param order. If any
    // param went unbound (not constrained by the args), try the enclosing
    // `let` annotation's expected type — a nullary generic (`new_stack<T>()`)
    // binds its param from `let s: Stack<Int> = new_stack()`.
    let mut concrete: Vec<Type> = Vec::with_capacity(params.len());
    let mut all_bound = true;
    for p in params {
        match bindings.get(p).cloned().unwrap_or(None) {
            Some(t) => concrete.push(t),
            None => {
                if let Some(expected) = &ctx.expected_call_type {
                    if let Some(ret) = ctx.fn_return_types.get(name).cloned() {
                        // Seed the expected-type bindings with the defn's
                        // params so `K`/`V` are recognized as type params.
                        let mut eb: std::collections::HashMap<String, Option<Type>> =
                            params.iter().map(|p| (p.clone(), None)).collect();
                        if unify_defn_type(&ret, expected, &mut eb) {
                            if let Some(t) = eb.get(p).cloned().unwrap_or(None) {
                                concrete.push(t);
                                continue;
                            }
                        }
                    }
                }
                all_bound = false;
                concrete.push(Type::int());
            }
        }
    }
    if all_bound {
        Ok(Some(concrete))
    } else {
        Ok(None)
    }
}

/// Unify a defn parameter type against a concrete argument type, binding the
/// defn's type params. A param that IS a type param binds to the arg; a
/// structured param (`List<T>`) recurses; a concrete-vs-concrete equality is
/// fine; anything else is a mismatch (returns false).
fn unify_defn_type(param: &Type, arg: &Type, bindings: &mut std::collections::HashMap<String, Option<Type>>) -> bool {
    // A bare type param: `T` unifies with any concrete arg type.
    if let Type::Custom(name) = param {
        if bindings.contains_key(name) {
            let existing = bindings.get(name).cloned().unwrap_or(None);
            if existing.is_none() {
                bindings.insert(name.clone(), Some(arg.clone()));
            }
            return true;
        }
    }
    match (param, arg) {
        (Type::Custom(p), Type::Custom(a)) => p == a,
        (Type::Applied(pn, p_args), Type::Applied(an, a_args)) => {
            pn == an && p_args.len() == a_args.len()
                && p_args.iter().zip(a_args).all(|(p, a)| unify_defn_type(p, a, bindings))
        }
        (Type::Ptr(p), Type::Ptr(a)) => unify_defn_type(p, a, bindings),
        (Type::PtrConst(p), Type::PtrConst(a)) => unify_defn_type(p, a, bindings),
        (Type::Vector(p, _), Type::Vector(a, _)) => unify_defn_type(p, a, bindings),
        (Type::Tuple(p), Type::Tuple(a)) => {
            p.len() == a.len() && p.iter().zip(a).all(|(p, a)| unify_defn_type(p, a, bindings))
        }
        // 2026-08-14 (closure-typed generics): a function-typed param (`f: T
        // -> U`) unifies against a closure's inferred `Type::Function` — the
        // param type(s) and return type each unify, binding the generic params
        // (`T -> U` vs `(Int) -> Int` → `T = Int`, `U = Int`).
        (Type::Function(pp, pr), Type::Function(ap, ar)) => {
            pp.len() == ap.len()
                && pp.iter().zip(ap).all(|(p, a)| unify_defn_type(p, a, bindings))
                && unify_defn_type(pr, ar, bindings)
        }
        _ => param == arg,
    }
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
        // 2026-08-14 (closure-typed generics): a function-typed param (`f: T
        // -> U`) substitutes its param and return types — `(T) -> U` with
        // `T=Int, U=Int` becomes `(Int) -> Int`.
        Type::Function(p_params, ret) => Type::Function(
            p_params.iter().map(|p| substitute_type_params(p, params, args)).collect(),
            Box::new(substitute_type_params(ret, params, args)),
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
        // 2026-08-12 (slice 4, wasm32 maze): Ptr must descend — `Ptr<T>` with
        // {T: Int} → `Ptr<Int>`. The mono substitution (ensure_mono) builds
        // `ListBuffer<Int>`'s struct fields; without this, `inner.data`
        // resolved to the generic `Ptr<T>` and the wasm32 element width
        // couldn't be derived (the load stayed i64 while the member returned
        // i32).
        Type::Ptr(inner) => Type::Ptr(Box::new(substitute_type(inner, subst))),
        Type::PtrConst(inner) => Type::PtrConst(Box::new(substitute_type(inner, subst))),
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
        let mut items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&mut items, &universe)
    }

    /// 2026-08-12 (Iterable protocol, op-as-member): an obj with operator
    /// members typechecks — op bodies are self-parameterized (slots bound) and
    /// the `<-` arrow resolves the op-as-member element types.
    #[test]
    fn op_as_member_typechecks_and_dispatch() {
        let src = r#"
obj Counter {
    count: Int;
    op Count() -> Int { term count; };
    op At(i: Int) -> Int { term count; };
    op InsertAt(v: Int) { count = v; term; };
    op ExtractFrom() -> Int { term count; };
};
let c: Counter = Counter { count: 0 };
let v: Int = 3;
let x: Int = 0;
txn push [v == 3][c.count == 3] {
    &c <- v;
    x <- c;
};
"#;
        // A typecheck failure here means the op-as-member body was not
        // self-parameterized, or the arrow did not resolve the operator member.
        check(src).expect("op-as-member must typecheck");
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): a call to a generic defn
    /// infers the type param from the argument and substitutes it into the
    /// return type — `id(5)` returns Int, `id(1.5)` returns Float, not the
    /// free `T`. (Collection-op generics are verified end-to-end; the unit
    /// `check()` has no stdlib, so `List` isn't in type_members here.)
    #[test]
    fn generic_defn_call_infers_type_param() {
        let src = r#"
defn id<T>(x: T) -> T [true][x == x] {
    term x;
};
node probe [true][x == x] {
    let a: Int = id(5);
    term;
};
"#;
        check(src).expect("generic defn call must infer the type param");
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): two type params infer from
    /// two distinct arguments.
    #[test]
    fn generic_defn_two_type_params() {
        let src = r#"
defn id<T>(x: T) -> T [true][x == x] {
    term x;
};
node probe [true][x == x] {
    let a: Int = id(5);
    let b: Float = id(1.5);
    term;
};
"#;
        check(src).expect("two-param generic defn calls must infer");
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): an argument that does not
    /// match the generic param shape is a clean type error, not a fallback.
    #[test]
    fn generic_defn_call_mismatch_errors() {
        let src = r#"
defn only_int<T>(x: List<T>) -> T [true][x == x] {
    term At#(x, 0);
};
node probe [true][x == x] {
    let v: Int = only_int(5);
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "expected a type error for a non-List generic arg, got: {:?}", e);
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): an EMPTY list literal in a
    /// generic body adopts the declared return's element type param — `term
    /// []` in `defn empty<T>() -> List<T>` is `List<T>`, not `List<Int>`.
    #[test]
    fn generic_defn_empty_list_adopts_return_param() {
        let src = r#"
defn empty<T>() -> List<T> [true][term == []] {
    term [];
};
let a: List<Int> = [];
node probe [a == []][true] {
    term;
};
"#;
        check(src).expect("an empty list in a generic body must adopt the declared return's type param");
    }

    /// 2026-08-14 (`term` canonical result placeholder): `term` in a defn's
    /// POST-condition is bound to the declared return type — `[term == true]`
    /// on a Bool-returning defn typechecks, and a type mismatch errors.
    #[test]
    fn term_result_placeholder_binds_to_return_type() {
        let src = r#"
defn is_odd(x: Int) -> Bool [true][term == true || term == false] {
    term x % 2 == 1;
};
let b: Bool = false;
node probe [b == false][b == true] {
    term;
};
"#;
        check(src).expect("`term` in a post-condition must bind to the declared return type");
    }

    /// 2026-08-14 (`term` canonical result placeholder): a post-condition that
    /// compares `term` against a mismatched type is now a REAL error (the old
    /// `elaborate_expr` swallowed it as an Int fallback).
    #[test]
    fn term_result_placeholder_type_mismatch_errors() {
        let src = r#"
defn bad(x: Int) -> Int [true][term == true] {
    term x;
};
node probe [true][true] {
    let a: Int = bad(1);
    term;
};
"#;
        let e = check(src);
        assert!(e.is_err(), "a post-condition comparing Int `term` to Bool must error, got: {:?}", e);
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): a generic defn with a
    /// closure-typed parameter — `f: T -> T` — parses and the call site
    /// infers `T` from the argument. (A generic body that RETURNS a free-`U`
    /// closure call is the documented body-with-free-T follow-up; here the
    /// body returns the `T` param, which is Int after call-site substitution.)
    #[test]
    fn generic_defn_closure_typed_param() {
        let src = r#"
defn apply<T>(x: T, f: T -> T) -> T [true][term == x] {
    term x;
};
let v: Int = 0;
node probe [v == 0][v == 1] {
    term;
};
"#;
        check(src).expect("a closure-typed generic param must parse and infer at the call site");
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): a generic TRANSACTION
    /// (`txn loop_t<T>`) parses and dispatches like a generic defn.
    #[test]
    fn generic_txn_type_params_parse() {
        let src = r#"
txn loop_t<T>(x: T, i: Int) [i < 10][i >= 10] -> Int {
    term i;
};
let v: Int = 0;
node probe [v == 0][v == 1] {
    term;
};
"#;
        check(src).expect("a generic txn must parse and dispatch");
    }

    /// 2026-08-14 (generic `defn f<T>` dispatch): a nullary two-param generic
    /// binds BOTH params from the `let` annotation's expected type. (List is
    /// used as the target — its `[]` literal works; HashMap's `{}` empty-literal
    /// construction is a separate stdlib gap.)
    #[test]
    fn generic_defn_nullary_two_params_bind_from_expected() {
        let src = r#"
defn pair_list<K, V>() -> List<K> [true][term == []] {
    term [];
};
let m: List<String> = pair_list();
let v: Int = 0;
node probe [v == 0][v == 1] {
    term;
};
"#;
        check(src).expect("a nullary two-param generic must bind both params from the annotation");
    }

    /// 2026-08-12 (Iterable protocol): a non-operator member body still fails
    /// when a slot is mis-typed (the A2 self+slots binding is intact for the
    /// new member kind).
    #[test]
    fn op_as_member_slot_mistype_errors() {
        let src = r#"
obj Counter {
    count: Int;
    op Count() -> Int { term "nope"; };
};
"#;
        let err = check(src).unwrap_err();
        assert!(!err.is_empty(), "mis-typed op body must error");
    }

    /// 2026-08-12 (Iterable protocol, Tier 2): a `foreach` item is typed by
    /// the collection's ELEMENT type (the `op At` return, substituted) — a
    /// String-list item is String, so an Int operation on it errors.
    #[test]
    fn foreach_item_is_element_typed() {
        let ok = r#"
import <std/collections>;
let items: List<Int> = [3, 5];
let sum: Int = 0;
node main [beginprogram][true] {
    foreach(x in items) { sum = sum + x; };
};
"#;
        check(ok).expect("Int-list foreach must typecheck");
        let bad = r#"
import <std/collections>;
let items: List<Int> = [3, 5];
let acc: String = "";
node main [beginprogram][true] {
    foreach(x in items) { acc = acc + x; };
};
"#;
        let err = check(bad).unwrap_err();
        assert!(!err.is_empty(), "Int item used as String must error");
    }

    /// 2026-08-12 (Iterable protocol): the PARENLESS foreach form
    /// (`foreach x in expr { }`) parses and typechecks — the `in` keyword is
    /// the binding; the parens were redundant.
    #[test]
    fn foreach_parenless_form_parses() {
        let ok = r#"
import <std/collections>;
let items: List<Int> = [3, 5];
let sum: Int = 0;
node main [beginprogram][true] {
    foreach x in items {
        sum = sum + x;
    };
};
"#;
        check(ok).expect("parenless foreach must parse and typecheck");
    }

    /// 2026-08-12 (Iterable protocol, Tier 1): a `foreach` over a cursor
    /// collection (op Iter/op Step/op IsEnd/op Current) typechecks with the
    /// element type from the Current op's return.
    #[test]
    fn foreach_tier1_cursor_collection() {
        let ok = r#"
obj CursorList {
    data: Int[4];
    n: Int;
    op Init: init(#Lh, #Rh);
    txn init(v: Int) [true][n == 1] { data[0] = v; n = 1; };
    op Iter() -> Int { term 0; };
    op Step(i: Int) -> Int { term i + 1; };
    op IsEnd(i: Int) -> Bool { term i >= n; };
    op Current(i: Int) -> Int { term data[i]; };
};
let c: CursorList = spawn CursorList(0);
let sum: Int = 0;
node main [beginprogram][true] {
    foreach v in c {
        sum = sum + v;
    };
};
"#;
        check(ok).expect("cursor foreach must typecheck");
        let bad = r#"
obj CursorList {
    data: Int[4];
    n: Int;
    op Init: init(#Lh, #Rh);
    txn init(v: Int) [true][n == 1] { data[0] = v; n = 1; };
    op Iter() -> Int { term 0; };
    op Step(i: Int) -> Int { term i + 1; };
    op IsEnd(i: Int) -> Bool { term i >= n; };
    op Current(i: Int) -> Int { term data[i]; };
};
let c: CursorList = spawn CursorList(0);
let acc: String = "";
node main [beginprogram][true] {
    foreach v in c { acc = acc + v; };
};
"#;
        let err = check(bad).unwrap_err();
        assert!(!err.is_empty(), "Int cursor item used as String must error");
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

    /// 2026-08-11 (housekeeping 1b): `items = []` on a `List<String>` target
    /// must coerce — an empty list literal carries no element to infer, so it
    /// defaults to List<Int>; assigning it to a List<T> is valid for every T.
    #[test]
    fn empty_list_assignment_coerces_to_target_element_type() {
        let src = r#"
let items: List<String> = ["A"];
txn t [items.^Size > 0][items.^Size == 0] {
    items = [];
    term;
};
"#;
        check(src).expect("empty [] assigned to List<String> must typecheck");
    }

    #[test]
    fn mask_index_on_data_types_to_data() {
        // 2026-08-07 (Phase 7): `data[mask]` on a #Data buffer types to Data
        // (the byte-buffer container kind), not the scalar element type.
        let src = r#"
let data: Data = #b"\x01\x02\x03";
node t [true][false] {
    let masked: Data = data[[true, false, true]];
    term;
};
"#;
        check(src).expect("Data mask index should typecheck");
    }

    #[test]
    fn mask_index_on_int_vector_types_to_list() {
        // 2026-08-07 (Phase 7): `Int[N][mask]` types to List<Int> — a heap
        // container of the selected elements, not the scalar element type.
        let src = r#"
let v: Int[4];
node t [true][false] {
    let m: List<Int> = v[[true, false, true, false]];
    term;
};
"#;
        check(src).expect("Int vector mask index should typecheck to List<Int>");
    }

    #[test]
    fn mask_index_on_float_vector_types_to_list() {
        // 2026-08-07 (Phase 7): Float vector fields are contiguous `[N x
        // float]` arrays — a Boolean mask over them types to List<Float>.
        let src = r#"
let v: Float[4];
node t [true][false] {
    let m: List<Float> = v[[true, false, true, false]];
    term;
};
"#;
        check(src).expect("Float vector mask index should typecheck to List<Float>");
    }

    #[test]
    fn mask_index_on_heap_list_types_to_list() {
        // 2026-08-07 (Phase 7): a Boolean mask over a heap `List<Int>` value
        // selects its elements into a new `List<Int>`.
        let src = r#"
node t [true][false] {
    let l: List<Int> = [10, 20, 30];
    let m: List<Int> = l[[true, false, true]];
    term;
};
"#;
        check(src).expect("heap List mask index should typecheck to List<Int>");
    }

    #[test]
    fn mask_index_on_float_list_types_to_list() {
        // A heap List<Float> stores its elements as i64 bit patterns in i64
        // slots — the typed gather preserves them, so a Boolean mask over a
        // List<Float> is List<Float> (2026-08-07, Phase 7).
        let src = r#"
node t [true][false] {
    let l: List<Float> = [1.0, 2.0];
    let m: List<Float> = l[[true, false]];
    term;
};
"#;
        check(src).expect("heap List<Float> mask index should typecheck to List<Float>");
    }

    #[test]
    fn unpacked_instance_member_dims_substitute() {
        // 2026-08-07 (object instance pools): `b.data[2]` on an unpacked
        // `Box<Int, 5>` resolves the member's const dims (M → 5) so the
        // element type is Int, not the raw generic param T.
        let src = r#"
obj Box<T, M> {
    data: T[M];
    total: Int;
    op Init: init(#Lh, #Rh);
    txn init(v: T) [true][total == 1] { data[0] = v; total = 1; };
};
let b: Box<Int, 5> = 0;
node t [true][false] {
    b.data[2] = 42;
    let x: Int = b.data[2];
    term;
};
"#;
        check(src).expect("unpacked member access should typecheck with substituted dims");
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

    // ── beginprogram entry-loops ─────────────────────────────────

    #[test]
    fn beginprogram_counter_goal_typechecks() {
        // 2026-08-06 (beginprogram plan): an entry-loop whose goal is a counter
        // bound with the body advancing the counter is provably reachable.
        let src = r#"
let i: Int = 0;
node init [beginprogram && i < 4][i == 4] {
    i = i + 1;
    term;
};
"#;
        check(src).expect("counter goal must typecheck");
    }

    #[test]
    fn beginprogram_true_goal_typechecks() {
        // A `[true]` goal runs exactly once — immediately satisfied.
        let src = r#"
let i: Int = 0;
node init [beginprogram][true] {
    i = i + 1;
    term;
};
"#;
        check(src).expect("[true] goal must typecheck");
    }

    #[test]
    fn beginprogram_unreachable_goal_errors() {
        // 2026-08-06 (beginprogram plan): a goal the body does not advance
        // toward is unprovably reachable ⇒ compile error.
        let src = r#"
let i: Int = 0;
node init [beginprogram][i == 4] {
    i = 0;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("not provably reachable")),
            "expected an unreachable-goal error, got {:?}",
            err
        );
    }

    #[test]
    fn beginprogram_conflicting_entries_error() {
        // 2026-08-06 (beginprogram plan): two unconditional beginprogram nodes
        // conflict — at most one may fire at program start.
        let src = r#"
let a: Int = 0;
node one [beginprogram][a == 1] {
    a = 1;
    term;
};
node two [beginprogram][a == 2] {
    a = 2;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("conflicting entry conditions")),
            "expected a conflict error, got {:?}",
            err
        );
    }

    /// A custom type declaring `op Mul(#Int)` authorizes `Int * MyType`
    /// without a cast (a cross-protocol overload).
    #[test]
    fn cross_type_overload_allows_mixed_arithmetic() {
        let src = r#"
type MyNum : #Int {
    op Mul(#Int): func(#Lh, #Rh);
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

    /// 2026-08-06 (Phase 5): a declared variant op (`op Add(Int): my_add`)
    /// ELABORATES into a call to its implementation — the BinaryOp becomes
    /// `Expr::Call("my_add", [l, r])` after check_program.
    #[test]
    fn declared_variant_op_elaborates_to_call() {
        let src = r#"
defn my_add(a: Int, b: Int) -> Int { term (a * 3) + b; };
type MyNum : #Int {
    op Add(Int): my_add(#Lh, #Rh);
};
node start [true][false] {
    let x: MyNum = 4;
    let y: Int = 2;
    let z: MyNum = x + y;
    term;
};
"#;
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let mut items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&mut items, &universe).unwrap();
        let node = items
            .iter()
            .find_map(|i| match i {
                TopLevel::Transaction(t) if t.name == "start" => Some(t),
                _ => None,
            })
            .expect("node 'start'");
        let z_expr = node
            .body
            .iter()
            .find_map(|s| match s {
                Statement::Let { name, expr, .. } if name == "z" => expr.as_ref(),
                _ => None,
            })
            .expect("let z");
        match z_expr {
            Expr::Call(fn_name, args, _) => {
                assert_eq!(fn_name, "my_add", "declared op must lower to its function");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected Call(my_add, ...), got {other:?}"),
        }
    }

    /// 2026-08-06 (Phase 5): the bootstrap colon-form `op Add: add(#Lh, #Rh)`
    /// is documentation — `Int + Int` stays a BinaryOp (protocol intrinsic
    /// lowering), never rewritten to the undefined `add` symbol.
    #[test]
    fn colon_form_doc_binding_is_not_elaborated() {
        let src = r#"
type IntDoc : #Int {
    op Add: add(#Lh, #Rh);
};
node start [true][false] {
    let a: Int = 1;
    let b: Int = 2;
    let c: Int = a + b;
    term;
};
"#;
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let mut items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&mut items, &universe).unwrap();
        let node = items
            .iter()
            .find_map(|i| match i {
                TopLevel::Transaction(t) if t.name == "start" => Some(t),
                _ => None,
            })
            .expect("node 'start'");
        let c_expr = node
            .body
            .iter()
            .find_map(|s| match s {
                Statement::Let { name, expr, .. } if name == "c" => expr.as_ref(),
                _ => None,
            })
            .expect("let c");
        assert!(
            matches!(c_expr, Expr::BinaryOp(BinaryOpKind::Add, _, _)),
            "Int + Int must stay a BinaryOp (protocol lowering), got {c_expr:?}"
        );
    }

    // ── Diagnostics sweep (2026-08-06) ─────────────────────────────

    /// A declared op whose implementation target is not a defined function
    /// must be a typecheck error (was a silent link-time undefined symbol).
    #[test]
    fn undefined_op_target_is_an_error() {
        let src = r#"
type MyNum : #Int {
    op Add(Int): nonexistent(#Lh, #Rh);
};
node start [true][false] {
    let x: MyNum = 1;
    let y: Int = 2;
    let z: MyNum = x + y;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("not a defined function")),
            "expected an undefined-op-target error, got {err:?}"
        );
    }

    /// A closure used as a value (not a call) must be an error — codegen
    /// would otherwise silently read the placeholder register 0.
    #[test]
    fn closure_used_as_value_is_legal() {
        // 2026-08-06 (fix): a closure is a real first-class value (env block
        // address in codegen) — assigning one to a Function-typed binding is
        // legal now.
        let src = r#"
node start [true][false] {
    let f = x -> x + 1;
    let g = f;
    term;
};
"#;
        assert!(
            check(src).is_ok(),
            "closure-as-value must typecheck; the closure is a first-class value"
        );
    }

    /// The PascalCase intrinsic forms of the env macros must direct the user
    /// to the lowercase macros, not report an "unknown intrinsic".
    #[test]
    fn get_env_int_hash_guides_to_lowercase_macro() {
        let src = r#"
node start [true][false] {
    let n: Int = GetEnvInt#("BOUND");
    term Print#(n);
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("get_env_int!")),
            "expected a rename-guidance error, got {err:?}"
        );
    }

    /// Without the cross-type overload, `Int * MyNum` errors.
    #[test]
    fn missing_cross_type_overload_errors() {        let src = r#"
type MyNum : #Int {
    op Sub(#Int): func(#Lh, #Rh);
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
    op Add(#Int): func(#Lh, #Rh);
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
    op Add(#Int): func(#Lh, #Rh);
};
type MyNum : Base, #Int { };
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
node probe [p.name != ""][true] {
    let n: String = p.name;
    let a: Int = p.age;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn node_without_contract_is_rejected() {
        // 2026-08-05 (Phase 6): node/txn must declare a contract.
        let src = "let count: Int = 0;\nnode tick { count = count + 1; term; };\n";
        let e = check(src);
        assert!(
            matches!(e, Err(ref errs) if errs.iter().any(|er| matches!(er, TypeError::MissingContract { .. }))),
            "expected MissingContract, got: {:?}",
            e
        );
    }

    #[test]
    fn defn_without_contract_is_allowed() {
        // 2026-08-05 (Phase 6): defn contracts are optional.
        let src = "defn add(a: Int, b: Int) -> Int { term a + b; };\n";
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn explicit_tautology_contract_is_rejected() {
        // 2026-08-05 (Phase 6): explicit [true][true] asserts nothing.
        let src = "let done: Bool = false;\nnode tick [true][true] { done = true; term; };\n";
        let e = check(src);
        assert!(
            matches!(e, Err(ref errs) if errs.iter().any(|er| matches!(er, TypeError::TautologicalContract))),
            "expected TautologicalContract, got: {:?}",
            e
        );
    }

    #[test]
    fn type_asserting_trait_must_provide_requirements() {
        // 2026-08-05 (Phase 5): structural conformance for explicit traits.
        // `Comparable` requires `compare`; `Tagged` provides it via `impl`.
        let ok_src = r#"
trait Comparable<T> { defn compare(left: Self, right: T) -> Int; };
type Tagged: Comparable<Int> { tag: Int; };
impl Tagged { defn compare(left: Tagged, right: Int) -> Int { term 0; }; };
let done: Bool = false;
node probe [done == false][done == true] { done = true; term; };
"#;
        let e = check(ok_src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);

        // Missing requirement: `Unrelated` does not provide `compare`.
        let bad_src = r#"
trait Comparable<T> { defn compare(left: Self, right: T) -> Int; };
type Unrelated: Comparable<Int> { x: Int; };
let done: Bool = false;
node probe [done == false][done == true] { done = true; term; };
"#;
        let e = check(bad_src);
        assert!(
            matches!(e, Err(ref errs) if errs.iter().any(|er| matches!(er, TypeError::InvalidOperation { .. }))),
            "expected conformance error, got: {:?}",
            e
        );
    }

    #[test]
    fn impl_must_target_declared_type() {
        // 2026-08-05 (Phase 5): impl coherence.
        let src = "impl Ghost { defn haunt() -> Int { term 0; }; };\n";
        let e = check(src);
        assert!(
            matches!(e, Err(ref errs) if errs.iter().any(|er| matches!(er, TypeError::InvalidOperation { .. }))),
            "expected invalid impl target, got: {:?}",
            e
        );
    }

    #[test]
    fn single_cast_may_cross_one_protocol() {
        // 2026-08-05 (Phase 5): Int → Float is one cross-protocol edge (ok);
        // a path that must cross two categories in one `as` is rejected.
        let ok_src = "defn to_f(x: Int) -> Float { term x as Float; };\n";
        let e = check(ok_src);
        assert!(e.is_ok(), "single cross-protocol cast should be allowed, got: {:?}", e);
    }

    #[test]
    fn reflect_kind_mismatch_errors() {
        // `Bytes` and `Size` are compile-time-only — runtime `.^Bytes`/`.^Size`
        // are kind errors (2026-08-14 §6a: `.^Size` runtime deleted — element
        // count is the `Count#` intrinsic).
        let src = r#"
let x: Int = 5;
node probe [true][true] {
    let s: Int = x.^Bytes;
    let n: Int = x.^Size;
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
node probe [items.^^Size > 0][true] {
    let sz: Int = items.^^Size;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn reflect_element_on_iterable_resolves() {
        // 2026-08-14 (boundary plan): `.^^Element` is a compile-time frozen
        // descriptor on an iterable receiver — resolves to Int (the folded
        // category code), never an error. The iterable is declared inline
        // (stdlib `List` isn't in the test universe).
        let src = r#"
obj Items {
    data: Int[4];
    op Count() -> Int { term 4; };
    op At(i: Int) -> Int { term data[i]; };
};
let xs: Items;
let s: String = "hi";
node probe [xs.^^Element == 0][true] {
    let a: Int = xs.^^Element;
    let b: Int = s.^^Element;
    term;
};
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    #[test]
    fn reflect_element_on_non_iterable_errors() {
        // A scalar has no element type — `.^^Element` is a compile error,
        // never a silent Int.
        let src = r#"
let x: Int = 5;
node probe [true][true] {
    let e: Int = x.^^Element;
    term;
};
"#;
        let e = check(src);
        assert!(
            e.is_err(),
            "expected non-iterable `.^^Element` to error, got: {:?}",
            e
        );
    }

    #[test]
    fn absolute_reflection_removed_directs_to_abs_intrinsic() {
        // 2026-08-14 (boundary plan, SPEC §17.3): `.^Absolute` was removed —
        // abs is a computed truth, so the `Abs#` intrinsic is its home. The
        // error must direct the user to `Abs#`.
        let src = r#"
let a: Int = -7;
node probe [a < 0][true] {
    let b: Int = a.^Absolute;
    term;
};
"#;
        let e = check(src);
        match e {
            Err(errs) => {
                let msg = format!("{:?}", errs);
                assert!(
                    msg.contains("Abs#"),
                    "the removed `.^Absolute` must direct to Abs#; got: {}",
                    msg
                );
            }
            Ok(()) => panic!("expected `.^Absolute` to be rejected"),
        }
    }

    #[test]
    fn unconstrained_list_literal_requires_annotation() {
        // 2026-08-14 (Iterable protocol, slice 5, SPEC §16.3): an
        // unconstrained `let xs = [1, 2, 3]` cannot construct through ops
        // (the compiler holds no List layout) — a compile error directing to
        // the type-directed literal form.
        let src = r#"
node probe [true][true] {
    let xs = [1, 2, 3];
    term;
};
"#;
        let e = check(src);
        assert!(
            e.is_err(),
            "expected unconstrained list literal to error, got: {:?}",
            e
        );
    }

    #[test]
    fn annotated_list_literal_accepts() {
        // The type-directed literal form stays valid.
        let src = r#"
let xs: List<Int> = [1, 2, 3];
node probe [xs.^^Element == 0][true] {
    let ys: List<Int> = [4, 5];
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
node probe [st.len <= 8][true] {
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
node probe [f >= 0.0][true] { term; };
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
node probe [lb.cap >= 0][true] { term; };
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
node probe [o >= 0][true] { term; };
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
node probe [offset >= 0][true] { term; };
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
node probe [st.len <= 8][true] { st.push(5); term; };
"#;
        let e = check(src);
        assert!(e.is_ok(), "expected OK, got: {:?}", e);
    }

    // ── init kind semantics (2026-08-09, Phase 2) ─────────────────────

    #[test]
    fn init_read_is_legal_but_assign_is_an_error() {
        // An init seeds once and is immutable — reading it anywhere is fine,
        // writing to it is a compile error.
        let src = r#"
init BufSize: Int = 64;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    let x: Int = BufSize;
    fired = fired + 1;
    term;
};
"#;
        assert!(check(src).is_ok(), "reads of an init must typecheck");
        let src = r#"
init BufSize: Int = 64;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    BufSize = 128;
    fired = fired + 1;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("immutable for the run")),
            "expected init-reassign error, got {:?}",
            err
        );
    }

    #[test]
    fn init_arrow_write_is_an_error() {
        let src = r#"
init BufSize: Int = 64;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    BufSize <- 128;
    fired = fired + 1;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("immutable for the run")),
            "expected init-arrow error, got {:?}",
            err
        );
    }

    #[test]
    fn init_shadowed_by_let_is_an_error() {
        let src = r#"
init BufSize: Int = 64;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    let BufSize: Int = 8;
    fired = fired + 1;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("shadows an `init`")),
            "expected init-shadow error, got {:?}",
            err
        );
    }

    #[test]
    fn duplicate_init_declaration_is_an_error() {
        let src = r#"
init BufSize: Int = 64;
init BufSize: Int = 128;
let fired: Int = 0;
node go [fired == 0][fired == 1] { fired = fired + 1; term; };
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("duplicate `init`")),
            "expected duplicate-init error, got {:?}",
            err
        );
    }

    #[test]
    fn init_after_beginprogram_is_an_error() {
        let src = r#"
node entry [beginprogram][i == 4] {
    let i: Int = 0;
    i = i + 1;
    term;
};
init BufSize: Int = 64;
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("after a beginprogram")),
            "expected late-init error, got {:?}",
            err
        );
    }

    #[test]
    fn init_and_beginprogram_coexist_when_ordered() {
        let src = r#"
init BufSize: Int = 64;
node entry [beginprogram][i == 4] {
    let i: Int = 0;
    i = i + 1;
    let x: Int = BufSize;
    term;
};
"#;
        assert!(check(src).is_ok(), "init before beginprogram must typecheck");
    }

    #[test]
    fn init_consume_is_an_error() {
        // An init is not a mutable location — `~` consume of it is rejected.
        let src = r#"
init BufSize: Int = 64;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    let x: Int = 0;
    x ~= BufSize;
    fired = fired + 1;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("cannot consume a constant")),
            "expected init-consume error, got {:?}",
            err
        );
    }

    // ── 2026-08-09 (Phase 12, SPEC §19.3): optional-frgn .^^Available ──

    #[test]
    fn optional_frgn_available_is_bool() {
        // `feature.^^Available` on an `optional frgn` is a Bool descriptor.
        let src = r#"
optional frgn feature(x: Int) -> Int from #System;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    let avail: Bool = feature.^^Available;
    fired = fired + 1;
    term;
};
"#;
        assert!(
            check(src).is_ok(),
            "an optional-frgn `^^Available` reflect must typecheck as Bool"
        );
    }

    #[test]
    fn available_on_non_optional_frgn_is_an_error() {
        // `^^Available` is valid only on an `optional frgn` (a non-optional
        // frgn is always available).
        let src = r#"
frgn feature(x: Int) -> Int from #System;
let fired: Int = 0;
node go [fired == 0][fired == 1] {
    let avail: Bool = feature.^^Available;
    fired = fired + 1;
    term;
};
"#;
        let err = check(src).unwrap_err();
        assert!(
            err.iter().any(|e| format!("{}", e).contains("only on an `optional frgn`")),
            "non-optional ^^Available must error, got {:?}",
            err
        );
    }
}

#[cfg(test)]
mod phase3_tests {
    use super::*;

    fn check(src: &str) -> Result<(), Vec<TypeError>> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let mut items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&mut items, &universe)
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
        let mut items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&mut items, &universe)
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
        let mut items = p.parse_program().unwrap();
        let universe = crate::type_universe::TypeUniverse::new();
        check_program(&mut items, &universe)
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

/// 2026-08-06 (Phase 7): `#b"..."` is a Data byte literal; `#r"..."` a String.
#[test]
fn byte_literal_is_data_raw_string_is_string() {
    let ok = r#"
node start [true][false] {
    let b: Data = #b"\x89PNG";
    let r: String = #r"a\tb";
    term;
};
"#;
    assert!(check(ok).is_ok(), "a #b literal must type as Data");
    let bad = r#"
node start [true][false] {
    let s: String = #b"\x89";
    term;
};
"#;
    assert!(
        check(bad).is_err(),
        "#b must NOT type as String (it is a raw Data literal)"
    );
}
}
