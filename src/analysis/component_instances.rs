//! 2026-08-12 (Phase 2b3, SPEC 21.3): obj-backed components.
//!
//! Components ARE objects. `obj Name` owns the component's state slots and
//! member transactions; `render Name { ... }` is the view fragment bound to
//! that obj. The tag namespace resolves deterministically:
//!
//! 1. `<c1 />` — a declared Briv instance var (`let c1: Counter`) mounts the
//!    fragment with bindings routed to `c1.*` slots (Briv-side instance; the
//!    PROGRAM owns it).
//! 2. `<Counter />` — a component type (`obj Counter` + `render Counter`)
//!    spawns an anonymous, reactor-owned instance in a per-mount pool
//!    (`Counter.<i>.*` slots); zero-init defaults, not referenceable by code.
//! 3. else a lowercase HTML element; else an unknown-tag warning.
//!
//! Seeding is BRIV source, never Rust: `let c1: Counter = Counter { count: 5 }`
//! seeds `c1.count` from the StructLiteral. There are NO HTML props — the
//! frontend invents no state values. Every state change the frontend requests
//! (per-mount txn variants, lifecycle resets) is bound to a trg — a callable
//! transaction with a proven contract — never a direct store.
//!
//! Scope (slice 2b3): compile-time instance pools for both paths. Dynamic
//! component counts (`b-each` of components) remain a follow-up.

use crate::ast::{Expr, Statement, TopLevel, Transaction, Type};
use std::collections::{HashMap, HashSet};

/// A single component mount's per-instance view-rewrite decision — the VIEW
/// layer applies these to the raw fragment (field signals → qualified slots,
/// trigger txns → mount variants). The analysis decides WHAT; the view
/// compiler decides HOW to format the HTML.
#[derive(Debug, Clone)]
pub struct MountSpec {
    /// The component name (for the `data-instance` marker).
    pub component: String,
    /// This mount's index.
    pub index: usize,
    /// Fragment-referenced field → instance-qualified slot
    /// (`count` → `Counter.0.count`).
    pub fields: Vec<(String, String)>,
    /// Original txn name → per-mount variant (`increment` → `increment_0`).
    pub txn_variants: std::collections::HashMap<String, String>,
}

/// The result of expanding component instances in a program + view.
#[derive(Debug)]
pub struct ComponentInstancePlan {
    /// Component name → per-mount view-rewrite specs (decisions only — the
    /// view layer applies them to the raw fragment).
    pub mounts: std::collections::HashMap<String, Vec<MountSpec>>,
    /// Instance slot → Briv seed (`c1.count` → 5, from a StructLiteral). The
    /// backend seeds these into %State at init; the VALUES are Briv source.
    pub initializers: std::collections::HashMap<String, Expr>,
    /// Every per-instance mount, as `(component, index)`. The backend emits a
    /// per-instance reset so a b-when unmount re-applies the instance's seeds
    /// (remount = fresh).
    pub instances: Vec<(String, usize)>,
}

/// An obj's component-relevant surface: its state slots (name → type) and its
/// member transactions (bare-name form, self-parameterized on the slots).
struct ObjInfo {
    slots: HashMap<String, Type>,
    member_txns: HashMap<String, Transaction>,
}

/// Expand every `render Name` component into per-mount instance state.
/// Returns a DECLARATIVE plan (mount specs, initializers, instance list) and
/// appends the instance state declarations + txn variants to `items` (sourced
/// from the obj's member txns). No HTML formatting happens here — the view
/// layer consumes the specs.
pub fn expand_component_instances(
    items: &mut Vec<TopLevel>,
    view_html: &str,
) -> Result<ComponentInstancePlan, String> {
    // ── Collect render blocks + obj definitions ──────────────────────
    let render_blocks: HashMap<String, String> = items
        .iter()
        .filter_map(|item| match item {
            TopLevel::RenderBlock(rb) => Some((rb.struct_name.clone(), rb.view_html.clone())),
            _ => None,
        })
        .collect();
    let obj_defs = collect_obj_defs(items);

    let mut plan = ComponentInstancePlan {
        mounts: HashMap::new(),
        initializers: HashMap::new(),
        instances: Vec::new(),
    };
    for (component, fragment_html) in &render_blocks {
        let refs = collect_fragment_refs(fragment_html);
        // A fragment with no state fields is a static view fragment — a single
        // empty spec (the view layer mounts it shared, 2b1 behavior). A static
        // container (`render Root`) needs no obj of its own.
        if refs.fields.is_empty() && refs.txns.is_empty() {
            plan.mounts.insert(
                component.clone(),
                vec![MountSpec {
                    component: component.clone(),
                    index: 0,
                    fields: Vec::new(),
                    txn_variants: HashMap::new(),
                }],
            );
            continue;
        }
        // 2026-08-12 (2b3): components ARE objects — `render Name` requires
        // `obj Name` (the globals-based fragment form is gone).
        let Some(obj) = obj_defs.get(component) else {
            return Err(format!(
                "render '{}' requires an obj of the same name (components ARE \
                 objects): declare `obj {} {{ ... }}` with the component's \
                 state slots and member transactions",
                component, component
            ));
        };
        // 2026-08-12 (2b3): the fragment must bind ONLY the obj's own slots and
        // member txns — a reference to a non-member is never silently dead.
        let missing_fields: Vec<&String> = refs
            .fields
            .iter()
            .filter(|f| !obj.slots.contains_key(*f))
            .collect();
        if !missing_fields.is_empty() {
            return Err(format!(
                "render '{}' references field(s) {} not in obj '{}' slots",
                component,
                missing_fields
                    .iter()
                    .map(|f| format!("'{}'", f))
                    .collect::<Vec<_>>()
                    .join(", "),
                component
            ));
        }
        let missing_txns: Vec<&String> = refs
            .txns
            .iter()
            .filter(|t| !obj.member_txns.contains_key(*t))
            .collect();
        if !missing_txns.is_empty() {
            return Err(format!(
                "render '{}' triggers transaction(s) {} not members of obj '{}'",
                component,
                missing_txns
                    .iter()
                    .map(|t| format!("'{}'", t))
                    .collect::<Vec<_>>()
                    .join(", "),
                component
            ));
        }
        let mount_count = count_component_mounts(view_html, component);
        let mut per_mount: Vec<MountSpec> = Vec::with_capacity(mount_count);
        // for_each (not a `for`) keeps expand single-level for Praetor.
        (0..mount_count).for_each(|i| {
            let prefix = format!("{}.{}", component, i);
            // Instance slots = the fragment's fields ∪ every slot a variant
            // member references — all typed by the obj (never `Type::int()`
            // guessed). The qualifier maps obj-slot identifiers to their
            // instance-qualified names in the member bodies + contracts.
            let slot_set = instance_slot_set(items, obj, &refs, &prefix);
            let qualifier = |id: &str| -> Option<String> {
                if slot_set.contains(id) {
                    Some(format!("{}.{}", prefix, id))
                } else {
                    None
                }
            };
            let variant_txns = build_txn_variants(items, obj, i, &refs, &qualifier);
            // The declarative mount spec — the view layer applies it to the
            // raw fragment (no HTML formatting here).
            let fields = refs
                .fields
                .iter()
                .map(|field| (field.clone(), format!("{}.{}", prefix, field)))
                .collect();
            per_mount.push(MountSpec {
                component: component.clone(),
                index: i,
                fields,
                txn_variants: variant_txns,
            });
            plan.instances.push((component.clone(), i));
        });
        plan.mounts.insert(component.clone(), per_mount);
    }
    Ok(plan)
}

/// Collect `obj Name { slot: Type; txn member [...] {...}; }` definitions as
/// the component surface: slots (name → type) + member transactions by name.
fn collect_obj_defs(items: &[TopLevel]) -> HashMap<String, ObjInfo> {
    let mut defs = HashMap::new();
    for item in items {
        if let TopLevel::TypeDef(td) = item {
            if td.body.slots.is_empty() {
                continue;
            }
            let slots = td
                .body
                .slots
                .iter()
                .map(|s| (s.name.clone(), s.ty.clone()))
                .collect();
            let member_txns = td
                .body
                .members
                .iter()
                .filter_map(|m| match m {
                    TopLevel::Transaction(t) => Some((t.name.clone(), t.clone())),
                    _ => None,
                })
                .collect();
            defs.insert(td.name.clone(), ObjInfo { slots, member_txns });
        }
    }
    defs
}

/// Fields and transactions a fragment's directives reference.
#[derive(Default)]
struct FragmentRefs {
    /// State fields (bare names, projections stripped).
    fields: std::collections::HashSet<String>,
    /// Transaction names fired by b-trigger directives.
    txns: std::collections::HashSet<String>,
}

/// Lightweight directive scanner: pulls `b-text`/`b-show`/`b-when`/`b-bind`
/// field signals and `b-trigger`/`b-on` txn names out of a fragment's markup.
/// Single forward pass (no nested loops — Praetor loop-depth).
fn collect_fragment_refs(html: &str) -> FragmentRefs {
    let mut refs = FragmentRefs::default();
    let mut rest = html;
    while let Some(pos) = rest.find("b-") {
        let tail = &rest[pos + 2..];
        let name_end = tail.find(['=', ' ', '>', '\n']).unwrap_or(tail.len());
        let name = &tail[..name_end];
        let after_name = &tail[name_end..];
        if let Some(value) = after_name
            .find('=')
            .and_then(|eq| strip_quotes(after_name[eq + 1..].trim()))
        {
            record_fragment_ref(name, value, &mut refs);
        }
        rest = &tail[name_end.min(tail.len().saturating_sub(1)) + 1..];
    }
    refs
}

/// Record a directive attr's value into the fragment refs.
fn record_fragment_ref(name: &str, value: &str, refs: &mut FragmentRefs) {
    if name.starts_with("trigger:") || name.starts_with("on:") {
        let txn = value.split('(').next().unwrap_or("").trim();
        if !txn.is_empty() {
            refs.txns.insert(txn.to_string());
        }
        return;
    }
    if matches!(name, "show" | "when") {
        // `b-show="step == 0"` binds the root field `step` (the shim evaluates
        // the comparison); `condition_root_signal` takes the first token.
        let (root, _) = crate::view_compiler::condition_root_signal(value);
        refs.fields.insert(root.to_string());
        return;
    }
    if name == "text" || name.starts_with("bind") {
        let (root, _) = crate::view_compiler::root_signal(value);
        refs.fields.insert(root.to_string());
    }
}

/// Strip one level of `"`/`'` quotes from a directive value.
fn strip_quotes(v: &str) -> Option<&str> {
    let v = v.trim_start();
    if v.starts_with('"') || v.starts_with('\'') {
        let q = v.as_bytes()[0] as char;
        let inner = &v[1..];
        let end = inner.find(q).unwrap_or(inner.len());
        Some(&inner[..end])
    } else {
        Some(v)
    }
}

/// Count `<Name .../>` mount tags in the view for the component type `Name`.
/// The name match stops at the first non-identifier byte so `<counter1 />`
/// never counts as a `<Counter />` mount.
fn count_component_mounts(view_html: &str, component: &str) -> usize {
    let needle = format!("<{}", component.to_lowercase());
    let lower = view_html.to_lowercase();
    let mut count = 0usize;
    let mut pos = 0usize;
    while let Some(rel) = lower[pos..].find(&needle) {
        let after = pos + rel + needle.len();
        let boundary_ok = lower[after..]
            .chars()
            .next()
            .map(|c| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(true);
        if boundary_ok {
            count += 1;
        }
        pos = after;
    }
    count.max(1)
}

/// The per-mount instance slots: the fragment's fields ∪ every obj slot a
/// variant member references. Registers each as a `StateDecl` typed by the
/// obj (the ORIGINAL type is preserved — a `show: Bool` stays Bool). Returns
/// the qualified name set for the member-body rewrite qualifier.
fn instance_slot_set(
    items: &mut Vec<TopLevel>,
    obj: &ObjInfo,
    refs: &FragmentRefs,
    prefix: &str,
) -> HashSet<String> {
    let mut slots: HashSet<String> = refs.fields.clone();
    for (name, member) in &obj.member_txns {
        if refs.txns.contains(name) || txn_references_fields(member, &refs.fields) {
            collect_txn_slots(member, &obj.slots, &mut slots);
        }
    }
    for field in &slots {
        let ty = obj.slots.get(field).cloned().unwrap_or_else(Type::int);
        items.push(TopLevel::StateDecl(crate::ast::top::StateDecl {
            name: format!("{}.{}", prefix, field),
            ty,
            span: None,
        }));
    }
    slots
}

/// Add every obj slot an obj member's body/contract references to `out`.
fn collect_txn_slots(txn: &Transaction, slots: &HashMap<String, Type>, out: &mut HashSet<String>) {
    let mut hits = HashSet::new();
    for stmt in &txn.body {
        stmt_vars(stmt, &mut hits);
    }
    expr_vars(&txn.contract.pre_condition, &mut hits);
    expr_vars(&txn.contract.post_condition, &mut hits);
    out.extend(hits.into_iter().filter(|f| slots.contains_key(f)));
}

/// Build per-mount variants of the obj's member txns that write the
/// fragment's fields (or are triggered by it). Returns the variant names
/// keyed by the original member name.
fn build_txn_variants(
    items: &mut Vec<TopLevel>,
    obj: &ObjInfo,
    mount: usize,
    refs: &FragmentRefs,
    qualifier: &dyn Fn(&str) -> Option<String>,
) -> HashMap<String, String> {
    let mut variants = HashMap::new();
    // Snapshot the members to variant-ize (those triggered by the fragment or
    // referencing the fragment's fields).
    let members: Vec<(String, Transaction)> = obj
        .member_txns
        .iter()
        .filter(|(name, t)| refs.txns.contains(*name) || txn_references_fields(t, &refs.fields))
        .map(|(name, t)| (name.clone(), t.clone()))
        .collect();
    for (name, mut t) in members {
        let variant_name = format!("{}_{}", name, mount);
        // for_each keeps the body-rewrite single-level for Praetor.
        t.body.iter_mut().for_each(|stmt| rewrite_stmt(stmt, qualifier));
        let pre = rewrite_expr(&t.contract.pre_condition, qualifier);
        let post = rewrite_expr(&t.contract.post_condition, qualifier);
        // 2026-08-12 (2b3): preserve the member's signature — a member with
        // parameters (b-bind:value route) keeps them on the variant.
        let new_txn = Transaction {
            name: variant_name.clone(),
            is_reactive: false,
            is_async: false,
            type_params: t.type_params,
            parameters: t.parameters,
            output_type: t.output_type,
            outputs: t.outputs,
            contract: crate::ast::Contract {
                pre_condition: pre,
                post_condition: post,
                watchdog: None,
                explicit: true,
                span: None,
            },
            body: t.body,
            metadata: t.metadata,
            derivation: t.derivation,
            modifiers: t.modifiers,
            span: None,
            doc: None,
        };
        items.push(TopLevel::Transaction(new_txn));
        variants.insert(name, variant_name);
    }
    variants
}

/// Whether a transaction's body/contract references any of the given fields.
fn txn_references_fields(t: &Transaction, fields: &std::collections::HashSet<String>) -> bool {
    let mut hits = std::collections::HashSet::new();
    for stmt in &t.body {
        stmt_vars(stmt, &mut hits);
    }
    expr_vars(&t.contract.pre_condition, &mut hits);
    expr_vars(&t.contract.post_condition, &mut hits);
    fields.iter().any(|f| hits.contains(f))
}

/// Collect the identifiers a statement references.
fn stmt_vars(stmt: &Statement, out: &mut std::collections::HashSet<String>) {
    match stmt {
        Statement::Let { expr, .. } => {
            if let Some(e) = expr {
                expr_vars(e, out);
            }
        }
        Statement::Assign(l, r) => {
            expr_vars(l, out);
            expr_vars(r, out);
        }
        Statement::ArrowAssign { target, value, .. } => {
            if let Some(t) = target {
                expr_vars(t, out);
            }
            expr_vars(value, out);
        }
        Statement::If(c, t, e) => {
            expr_vars(c, out);
            for s in t.iter().chain(e.iter()) {
                stmt_vars(s, out);
            }
        }
        Statement::Guarded(c, b) => {
            expr_vars(c, out);
            for s in b.iter() {
                stmt_vars(s, out);
            }
        }
        Statement::Gate(c) => expr_vars(c, out),
        Statement::Block(b) => {
            for s in b.iter() {
                stmt_vars(s, out);
            }
        }
        Statement::Foreach { body, .. } => {
            for s in body.iter() {
                stmt_vars(s, out);
            }
        }
        Statement::Expression(e) => expr_vars(e, out),
        Statement::Term(Some(e)) | Statement::EndProgram(Some(e)) | Statement::Rollback(Some(e)) => {
            expr_vars(e, out)
        }
        _ => {}
    }
}

/// Collect the identifiers an expression references.
fn expr_vars(e: &Expr, out: &mut std::collections::HashSet<String>) {
    match e {
        Expr::Identifier(name) => {
            out.insert(name.clone());
        }
        Expr::BinaryOp(_, l, r) => {
            expr_vars(l, out);
            expr_vars(r, out);
        }
        Expr::UnaryOp(_, i) => expr_vars(i, out),
        Expr::Field(o, _) => expr_vars(o, out),
        Expr::MethodCall(o, _, args, _) => {
            expr_vars(o, out);
            for a in args.iter() {
                expr_vars(a, out);
            }
        }
        Expr::Index(o, i) => {
            expr_vars(o, out);
            expr_vars(i, out);
        }
        Expr::Call(_, args, _) => {
            for a in args.iter() {
                expr_vars(a, out);
            }
        }
        Expr::List(es) | Expr::Tuple(es) => {
            for x in es.iter() {
                expr_vars(x, out);
            }
        }
        Expr::AddrOf(i) | Expr::Deref(i) | Expr::Consume(i) | Expr::Await(i) | Expr::Within(i, _) => {
            expr_vars(i, out)
        }
        Expr::Block(ss) => {
            for s in ss.iter() {
                stmt_vars(s, out);
            }
        }
        Expr::If(c, t, f) => {
            expr_vars(c, out);
            expr_vars(t, out);
            if let Some(f) = f {
                expr_vars(f, out);
            }
        }
        Expr::Match(subject, arms) => {
            expr_vars(subject, out);
            for arm in arms.iter() {
                if let Some(g) = &arm.guard {
                    expr_vars(g, out);
                }
                expr_vars(&arm.body, out);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, f) in fields.iter() {
                expr_vars(f, out);
            }
        }
        Expr::Reflect(recv, _, _) => expr_vars(recv, out),
        Expr::Cast(e, _) | Expr::IsType(e, _) => expr_vars(e, out),
        _ => {}
    }
}

/// Rewrite a statement's identifiers via the qualifier.
fn rewrite_stmt(stmt: &mut Statement, qualifier: &dyn Fn(&str) -> Option<String>) {
    match stmt {
        Statement::Let { expr, .. } => {
            if let Some(e) = expr {
                rewrite_expr_mut(e, qualifier);
            }
        }
        Statement::Assign(lhs, rhs) => {
            rewrite_expr_mut(lhs, qualifier);
            rewrite_expr_mut(rhs, qualifier);
        }
        Statement::ArrowAssign { target, value, .. } => {
            if let Some(t) = target {
                rewrite_expr_mut(t, qualifier);
            }
            rewrite_expr_mut(value, qualifier);
        }
        Statement::If(cond, then_b, else_b) => {
            rewrite_expr_mut(cond, qualifier);
            for s in then_b.iter_mut() {
                rewrite_stmt(s, qualifier);
            }
            for s in else_b.iter_mut() {
                rewrite_stmt(s, qualifier);
            }
        }
        Statement::Guarded(cond, body) => {
            rewrite_expr_mut(cond, qualifier);
            for s in body.iter_mut() {
                rewrite_stmt(s, qualifier);
            }
        }
        Statement::Gate(cond) => rewrite_expr_mut(cond, qualifier),
        Statement::Block(body) => {
            for s in body.iter_mut() {
                rewrite_stmt(s, qualifier);
            }
        }
        Statement::Foreach { body, .. } => {
            for s in body.iter_mut() {
                rewrite_stmt(s, qualifier);
            }
        }
        Statement::Expression(e) => rewrite_expr_mut(e, qualifier),
        Statement::Term(Some(e)) | Statement::EndProgram(Some(e)) | Statement::Rollback(Some(e)) => {
            rewrite_expr_mut(e, qualifier)
        }
        _ => {}
    }
}

/// Rewrite an expression tree in place (identifiers → qualified).
fn rewrite_expr_mut(e: &mut Expr, qualifier: &dyn Fn(&str) -> Option<String>) {
    match e {
        Expr::Identifier(name) => {
            if let Some(q) = qualifier(name) {
                *e = Expr::Identifier(q);
            }
        }
        Expr::BinaryOp(_, l, r) => {
            rewrite_expr_mut(l, qualifier);
            rewrite_expr_mut(r, qualifier);
        }
        Expr::UnaryOp(_, i) => rewrite_expr_mut(i, qualifier),
        Expr::Field(o, _) => rewrite_expr_mut(o, qualifier),
        Expr::MethodCall(o, _, args, _) => {
            rewrite_expr_mut(o, qualifier);
            for a in args.iter_mut() {
                rewrite_expr_mut(a, qualifier);
            }
        }
        Expr::Index(o, i) => {
            rewrite_expr_mut(o, qualifier);
            rewrite_expr_mut(i, qualifier);
        }
        Expr::Call(_, args, _) => {
            for a in args.iter_mut() {
                rewrite_expr_mut(a, qualifier);
            }
        }
        Expr::List(es) | Expr::Tuple(es) => {
            for x in es.iter_mut() {
                rewrite_expr_mut(x, qualifier);
            }
        }
        Expr::AddrOf(i) | Expr::Deref(i) | Expr::Consume(i) | Expr::Await(i) | Expr::Within(i, _) => {
            rewrite_expr_mut(i, qualifier)
        }
        Expr::Block(ss) => {
            for s in ss.iter_mut() {
                rewrite_stmt(s, qualifier);
            }
        }
        Expr::If(c, t, f) => {
            rewrite_expr_mut(c, qualifier);
            rewrite_expr_mut(t, qualifier);
            if let Some(f) = f {
                rewrite_expr_mut(f, qualifier);
            }
        }
        Expr::Match(subject, arms) => {
            rewrite_expr_mut(subject, qualifier);
            for arm in arms.iter_mut() {
                if let Some(g) = &mut arm.guard {
                    rewrite_expr_mut(g, qualifier);
                }
                rewrite_expr_mut(&mut arm.body, qualifier);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, f) in fields.iter_mut() {
                rewrite_expr_mut(f, qualifier);
            }
        }
        Expr::Reflect(recv, _, _) => rewrite_expr_mut(recv, qualifier),
        Expr::Cast(e, _) | Expr::IsType(e, _) => rewrite_expr_mut(e, qualifier),
        _ => {}
    }
}

/// Rewrite an owned expression (contracts).
fn rewrite_expr(e: &Expr, qualifier: &dyn Fn(&str) -> Option<String>) -> Expr {
    let mut cloned = e.clone();
    rewrite_expr_mut(&mut cloned, qualifier);
    cloned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(src: &str) -> Result<(Vec<TopLevel>, ComponentInstancePlan), String> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        let mut items = p.parse_program().unwrap();
        let view = items
            .iter()
            .find_map(|item| match item {
                TopLevel::RenderBlock(rb) => {
                    if rb.struct_name == "Root" {
                        Some(rb.view_html.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .unwrap_or_default();
        let plan = expand_component_instances(&mut items, &view)?;
        Ok((items, plan))
    }

    #[test]
    fn two_mounts_get_independent_state() {
        let src = r#"
obj Counter {
    count: Int;
    txn increment [count < 100][true] {
        count = count + 1;
        term;
    };
};
render Counter {
    <span b-text="count">0</span>
    <button b-trigger:click="increment">+</button>
};
render Root {
    <Counter />
    <Counter />
};
"#;
        let (items, plan) = check(src).unwrap();
        let specs = plan.mounts.get("Counter").expect("Counter plan");
        assert_eq!(specs.len(), 2, "one spec per mount: {specs:?}");
        assert_eq!(
            specs[0].fields,
            vec![("count".to_string(), "Counter.0.count".to_string())],
            "mount 0 field map"
        );
        assert_eq!(
            specs[1].fields,
            vec![("count".to_string(), "Counter.1.count".to_string())],
            "mount 1 field map"
        );
        assert_eq!(
            specs[0].txn_variants.get("increment").map(|v| v.as_str()),
            Some("increment_0"),
            "mount 0 txn variant"
        );
        assert_eq!(
            specs[1].txn_variants.get("increment").map(|v| v.as_str()),
            Some("increment_1"),
            "mount 1 txn variant"
        );
        let names: Vec<String> = items.iter().filter_map(|item| match item {
            TopLevel::StateDecl(sd) => Some(sd.name.clone()),
            TopLevel::Transaction(t) => Some(format!("txn:{}", t.name)),
            _ => None,
        }).collect();
        assert!(names.contains(&"Counter.0.count".to_string()), "{names:?}");
        assert!(names.contains(&"Counter.1.count".to_string()), "{names:?}");
        assert!(names.contains(&"txn:increment_0".to_string()), "{names:?}");
        assert!(names.contains(&"txn:increment_1".to_string()), "{names:?}");
    }

    /// Slot types come from the obj's slots — a `show: Bool` instance slot is
    /// Bool, never the `Type::int()` fallback.
    #[test]
    fn slot_type_comes_from_obj() {
        let src = r#"
obj Panel {
    show: Bool;
    txn toggle [show == false][show == true] {
        show = true;
        term;
    };
};
render Panel {
    <div b-when="show">visible</div>
};
render Root {
    <Panel />
    <Panel />
};
"#;
        let (items, _plan) = check(src).unwrap();
        let states: Vec<(String, Type)> = items.iter().filter_map(|item| match item {
            TopLevel::StateDecl(sd) => Some((sd.name.clone(), sd.ty.clone())),
            _ => None,
        }).collect();
        assert!(
            states.iter().any(|(n, t)| n == "Panel.0.show" && *t == Type::bool_()),
            "instance slot keeps the obj's Bool type: {states:?}"
        );
        let txns: Vec<String> = items.iter().filter_map(|item| match item {
            TopLevel::Transaction(t) => Some(t.name.clone()),
            _ => None,
        }).collect();
        assert!(txns.contains(&"toggle_0".to_string()) && txns.contains(&"toggle_1".to_string()),
            "write-consumed member gets per-mount variants: {txns:?}");
    }

    /// `render Name` requires `obj Name` — a fragment without its obj is a
    /// compile error, never silently-mounted globals.
    #[test]
    fn render_requires_obj() {
        let src = r#"
render Counter {
    <span b-text="count">0</span>
};
render Root {
    <Counter />
};
"#;
        let err = check(src).unwrap_err();
        assert!(err.contains("requires an obj"), "{err}");
    }

    /// A fragment binding a field the obj does not own is a compile error —
    /// never a silently dead binding.
    #[test]
    fn fragment_field_must_be_obj_slot() {
        let src = r#"
obj Counter {
    count: Int;
};
render Counter {
    <span b-text="total">0</span>
};
render Root {
    <Counter />
};
"#;
        let err = check(src).unwrap_err();
        assert!(err.contains("'total'") && err.contains("slots"), "{err}");
    }

    /// A fragment triggering a txn that is not an obj member is a compile
    /// error — never an inert button.
    #[test]
    fn fragment_txn_must_be_obj_member() {
        let src = r#"
obj Counter {
    count: Int;
};
render Counter {
    <button b-trigger:click="bump">+</button>
};
render Root {
    <Counter />
};
"#;
        let err = check(src).unwrap_err();
        assert!(err.contains("'bump'") && err.contains("members"), "{err}");
    }

    /// 2026-08-12 (2b3): HTML-side spawns are reactor-owned and zero-init —
    /// no props, no Rust-invented seeds.
    #[test]
    fn html_spawns_are_zero_init() {
        let src = r#"
obj Counter {
    count: Int;
    txn increment [count < 100][true] {
        count = count + 1;
        term;
    };
};
render Counter {
    <span b-text="count">0</span>
};
render Root {
    <Counter />
    <Counter />
};
"#;
        let (_items, plan) = check(src).unwrap();
        assert!(plan.initializers.is_empty(), "no Rust-invented seeds: {:?}", plan.initializers);
        assert_eq!(
            plan.mounts.get("Counter").map(|s| s.len()).unwrap_or(0),
            2,
            "two per-mount specs"
        );
    }
}
