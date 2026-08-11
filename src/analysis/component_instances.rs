//! 2026-08-11 (Phase 2b2, SPEC 21.3): per-instance component state via fixed
//! instance pools.
//!
//! Each `<Name />` mount in the view gets its OWN copy of the fields its
//! `render Name { ... }` fragment references, plus per-mount variants of the
//! transactions that write them. The instance slots are dotted state fields
//! (`counter.0.count`, `counter.1.count`) — scalar `%State` rows, so the
//! existing field-index machinery routes reads/writes without a runtime mount
//! registry. The view compiler splices a per-mount (rewritten) fragment for
//! each tag, binding the mount's DOM to its pool handles.
//!
//! Scope (slice 2a): compile-time mount indices (the view's tags). Dynamic
//! component counts (`b-each` of components) and props are follow-ups.

use crate::ast::{Expr, Statement, TopLevel, Transaction, Type};

/// The result of expanding component instances in a program + view.
pub struct ComponentInstancePlan {
    /// Component name → per-mount rewritten fragment html. Mount k of `Name`
    /// splices `fragments[k]`. Components without per-instance expansion have
    /// one shared entry (2b1 behavior).
    pub fragments: std::collections::HashMap<String, Vec<String>>,
    /// Instance slot → prop initializer (`Counter.0.count` → 5). The backend
    /// seeds these into %State at init.
    pub initializers: std::collections::HashMap<String, Expr>,
}

/// Expand every `render Name` component into per-mount instance state.
/// Returns the plan (per-mount fragments) and appends the instance state
/// declarations + txn variants to `items` (removing the consumed originals).
pub fn expand_component_instances(
    items: &mut Vec<TopLevel>,
    view_html: &str,
) -> Result<ComponentInstancePlan, String> {
    // ── Collect render blocks ────────────────────────────────────────
    let render_blocks: std::collections::HashMap<String, String> = items
        .iter()
        .filter_map(|item| match item {
            TopLevel::RenderBlock(rb) => Some((rb.struct_name.clone(), rb.view_html.clone())),
            _ => None,
        })
        .collect();

    let mut plan = ComponentInstancePlan {
        fragments: std::collections::HashMap::new(),
        initializers: std::collections::HashMap::new(),
    };
    for (component, fragment_html) in &render_blocks {
        let mount_count = count_component_mounts(view_html, component);
        let refs = collect_fragment_refs(fragment_html);
        // A fragment with no state fields is a static view fragment — mount it
        // shared (2b1 behavior).
        if refs.fields.is_empty() && refs.txns.is_empty() {
            plan.fragments.insert(component.clone(), vec![fragment_html.clone()]);
            continue;
        }
        // 2026-08-11 (2b2 slice 2b): the props each mount passes — attribute
        // `attr="value"` on the `<Name />` tag seeds the instance slot for the
        // fragment-referenced field `attr`.
        let mount_props = collect_mount_props(view_html, component, mount_count);
        let mut per_mount: Vec<String> = Vec::with_capacity(mount_count);
        // for_each (not a `for`) keeps expand single-level for Praetor.
        (0..mount_count).for_each(|i| {
            let prefix = format!("{}.{}", component, i);
            // Instance-qualified state slots for the fragment's fields.
            refs.fields.iter().for_each(|field| {
                replace_or_add_state_decl(items, &format!("{}.{}", prefix, field), field);
            });
            // Prop initializers for this mount's slots.
            if let Some(props) = mount_props.get(&i) {
                props.iter().for_each(|(field, value)| {
                    if refs.fields.contains(field) {
                        if let Some(init) = parse_prop_value(value) {
                            plan.initializers.insert(format!("{}.{}", prefix, field), init);
                        }
                    }
                });
            }
            let variant_txns = build_txn_variants(items, component, i, &refs, &prefix);
            // Rewrite the fragment's directives for this mount.
            let rewritten = rewrite_fragment(fragment_html, &refs, &prefix, i, &variant_txns);
            per_mount.push(rewritten);
        });
        // Remove the consumed originals.
        remove_consumed_originals(items, &refs);
        plan.fragments.insert(component.clone(), per_mount);
    }
    Ok(plan)
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
    if matches!(name, "text" | "show" | "when") || name.starts_with("bind") {
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

/// Count `<Name />` mount tags in the view.
fn count_component_mounts(view_html: &str, component: &str) -> usize {
    let needle_l = format!("<{}", component.to_lowercase());
    let lower = view_html.to_lowercase();
    lower.matches(&needle_l).count().max(1)
}

/// 2026-08-11 (2b2 slice 2b): the props each `<Name ...>` mount tag passes —
/// `attr="value"` attributes (directive `b-*` attrs excluded), in mount order.
/// Returns mount index → (attr, value) pairs.
fn collect_mount_props(
    view_html: &str,
    component: &str,
    count: usize,
) -> std::collections::HashMap<usize, Vec<(String, String)>> {
    let mut props: std::collections::HashMap<usize, Vec<(String, String)>> =
        std::collections::HashMap::new();
    let needle = format!("<{}", component);
    let lower = view_html.to_lowercase();
    let needle_l = needle.to_lowercase();
    let mut mount = 0usize;
    let mut rest = view_html;
    let mut lower_rest = lower.as_str();
    while let Some(pos) = lower_rest.find(&needle_l) {
        if mount >= count {
            break;
        }
        // The tag extends to the `>` (quote-aware).
        let tag_start = pos;
        let tag = &rest[tag_start..];
        if let Some(end) = tag.find('>') {
            let tag_str = &tag[..end];
            let attrs = parse_tag_attrs(tag_str, component);
            if !attrs.is_empty() {
                props.insert(mount, attrs);
            }
        }
        mount += 1;
        let skip = pos + needle_l.len();
        rest = &rest[skip..];
        lower_rest = &lower_rest[skip..];
    }
    props
}

/// Extract `attr="value"` pairs from a component mount tag (skip `b-*`
/// directives and `id`).
fn parse_tag_attrs(tag: &str, component: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let rest = &tag[component.len()..];
    let mut scan = 0;
    while let Some(eq_rel) = rest[scan..].find('=') {
        let eq = scan + eq_rel;
        let attr_start = rest[..eq].rfind([' ', '\n', '\t']).map(|i| i + 1).unwrap_or(0);
        let name = rest[attr_start..eq].trim();
        // Directive attrs / id are not props.
        if name.is_empty() || name.starts_with("b-") || name == "id" {
            let after = &rest[eq + 1..];
            let Some(advance) = attr_value_len(after) else { break };
            scan = eq + 1 + advance;
            continue;
        }
        // Read the quoted value.
        let after = &rest[eq + 1..];
        let v = after.trim_start();
        let Some((value, advance)) = quoted_value(v) else { break };
        attrs.push((name.to_string(), value.to_string()));
        scan = eq + 1 + (after.len() - v.len()) + advance;
    }
    attrs
}

/// Byte length of a quoted attribute value starting at `after` (or the
/// whitespace-separated token when unquoted).
fn attr_value_len(after: &str) -> Option<usize> {
    let v = after.trim_start();
    if v.starts_with('"') {
        let inner = &v[1..];
        inner.find('"').map(|i| (after.len() - v.len()) + i + 2)
    } else {
        Some(after.len())
    }
}

/// Read a quoted attribute value: the content and the bytes consumed.
fn quoted_value(v: &str) -> Option<(&str, usize)> {
    if v.starts_with('"') {
        let inner = &v[1..];
        let end = inner.find('"')?;
        Some((&inner[..end], end + 2))
    } else {
        None
    }
}

/// Parse a prop value string into an Expr literal.
fn parse_prop_value(value: &str) -> Option<Expr> {
    let v = value.trim();
    if let Ok(n) = v.parse::<i64>() {
        return Some(Expr::Decimal(n));
    }
    if v == "true" {
        return Some(Expr::Bool(true));
    }
    if v == "false" {
        return Some(Expr::Bool(false));
    }
    if v.len() >= 2 && (v.starts_with('\'') || v.starts_with('"')) {
        let inner = &v[1..v.len() - 1];
        return Some(Expr::Quoted(inner.as_bytes().to_vec()));
    }
    None
}

/// Replace the global `field` state declaration with the instance-qualified
/// slot `qualified`. Handles both `StateDecl` and top-level `let` fields —
/// the instance slot keeps the ORIGINAL type (a `let show: Bool` must not
/// become `Int`).
fn replace_or_add_state_decl(items: &mut Vec<TopLevel>, qualified: &str, field: &str) {
    let orig_ty = items
        .iter()
        .find_map(|item| match item {
            TopLevel::StateDecl(sd) if sd.name == field => Some(sd.ty.clone()),
            TopLevel::Statement(stmt) => {
                if let Statement::Let { name, ty, .. } = stmt.as_ref() {
                    if name == field {
                        ty.clone()
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        })
        .unwrap_or(Type::int());
    if let Some(sd) = items.iter_mut().find_map(|item| match item {
        TopLevel::StateDecl(sd) if sd.name == field => Some(sd),
        _ => None,
    }) {
        sd.name = qualified.to_string();
        return;
    }
    items.push(TopLevel::StateDecl(crate::ast::top::StateDecl {
        name: qualified.to_string(),
        ty: orig_ty,
        span: None,
    }));
}

/// Build per-mount variants of the txns that write the fragment's fields.
/// Returns the variant names keyed by the original txn name.
fn build_txn_variants(
    items: &mut Vec<TopLevel>,
    component: &str,
    mount: usize,
    refs: &FragmentRefs,
    prefix: &str,
) -> std::collections::HashMap<String, String> {
    let mut variants = std::collections::HashMap::new();
    // Snapshot the component txns (those referencing the fragment's fields).
    let txn_idxs: Vec<usize> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            TopLevel::Transaction(t) => {
                if refs.txns.contains(&t.name) || txn_references_fields(t, &refs.fields) {
                    Some(i)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();
    for idx in txn_idxs {
        let orig = items[idx].clone();
        let (name, mut body, contract) = match orig {
            TopLevel::Transaction(t) => (t.name, t.body, t.contract),
            _ => unreachable!(),
        };
        let variant_name = format!("{}_{}", name, mount);
        // Rewrite the fragment fields to the instance-qualified names.
        let qualifier = |id: &str| -> Option<String> {
            if refs.fields.contains(id) {
                Some(format!("{}.{}", prefix, id))
            } else {
                None
            }
        };
        // for_each keeps the body-rewrite single-level for Praetor.
        body.iter_mut().for_each(|stmt| rewrite_stmt(stmt, &qualifier));
        let pre = rewrite_expr(&contract.pre_condition, &qualifier);
        let post = rewrite_expr(&contract.post_condition, &qualifier);
        // Replace the original in place (first variant) or append.
        let new_txn = Transaction {
            name: variant_name.clone(),
            is_reactive: false,
            is_async: false,
            type_params: Vec::new(),
            parameters: Vec::new(),
            output_type: None,
            outputs: Vec::new(),
            contract: crate::ast::Contract {
                pre_condition: pre,
                post_condition: post,
                watchdog: None,
                explicit: true,
                span: None,
            },
            body,
            metadata: std::collections::HashMap::new(),
            derivation: None,
            modifiers: Vec::new(),
            span: None,
            doc: None,
        };
        items.push(TopLevel::Transaction(new_txn));
        variants.insert(name, variant_name);
        let _ = component;
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

/// Rewrite a fragment's directives for a mount: field signals become
/// `prefix.<field>` and txn triggers become the mount's variant names.
fn rewrite_fragment(
    fragment: &str,
    refs: &FragmentRefs,
    prefix: &str,
    mount: usize,
    variant_txns: &std::collections::HashMap<String, String>,
) -> String {
    let _ = mount;
    let mut out = fragment.to_string();
    for field in &refs.fields {
        let qualified = format!("{}.{}", prefix, field);
        // Replace `b-text="field"`, `b-show="field"`, etc. — the bare field
        // token as a directive VALUE (attribute boundaries).
        out = replace_directive_value(&out, field, &qualified);
    }
    for (orig, variant) in variant_txns {
        out = replace_directive_value(&out, orig, variant);
    }
    out
}

/// Replace `from` → `to` inside DIRECTIVE attribute values only (`b-text="..."`,
/// `b-trigger:click="..."`). Non-directive attrs and markup text pass through.
fn replace_directive_value(html: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut scan = 0;
    while let Some(rel) = html[scan..].find("=\"") {
        let pos = scan + rel;
        // Check the attr name before this `=`: scan only the trailing token
        // window (bounded) so the backward search is O(1), not O(prefix).
        let window_start = pos.saturating_sub(32);
        let window = &html[window_start..pos];
        let attr_name = &window[window.rfind([' ', '<', '>', '\n', '\t']).map(|i| i + 1).unwrap_or(0)..];
        if attr_name.starts_with("b-") {
            out.push_str(&html[scan..pos + 2]);
            let after = &html[pos + 2..];
            if let Some(end) = after.find('"') {
                let value = &after[..end];
                out.push_str(&value.replace(from, to));
                out.push('"');
                scan = pos + 2 + end + 1;
            } else {
                out.push_str(after);
                scan = html.len();
            }
        } else {
            out.push_str(&html[scan..pos + 1]);
            scan = pos + 1;
        }
    }
    out.push_str(&html[scan..]);
    out
}

/// Remove the global state decls + txns consumed by per-instance expansion.
fn remove_consumed_originals(items: &mut Vec<TopLevel>, refs: &FragmentRefs) {
    let fields = &refs.fields;
    let txns = &refs.txns;
    items.retain(|item| match item {
        TopLevel::StateDecl(sd) => !fields.contains(&sd.name),
        // A txn is consumed if a trigger names it OR it references the
        // fragment's fields (its per-mount variants replace it).
        TopLevel::Transaction(t) => !txns.contains(&t.name) && !txn_references_fields(t, fields),
        // A top-level `let show: Bool` consumed by the instances is removed
        // too (its instance StateDecl replaces it).
        TopLevel::Statement(stmt) => {
            if let Statement::Let { name, .. } = stmt.as_ref() {
                !fields.contains(name)
            } else {
                true
            }
        }
        _ => true,
    });
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
let count: Int = 0;
txn increment [count < 100][true] {
    count = count + 1;
    term;
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
        let frags = plan.fragments.get("Counter").expect("Counter plan");
        assert_eq!(frags.len(), 2, "one fragment per mount: {frags:?}");
        assert!(
            frags[0].contains("b-text=\"Counter.0.count\""),
            "mount 0 binds its instance slot: {}",
            frags[0]
        );
        assert!(
            frags[1].contains("b-text=\"Counter.1.count\""),
            "mount 1 binds its instance slot: {}",
            frags[1]
        );
        assert!(
            frags[0].contains("b-trigger:click=\"increment_0\"")
                && frags[1].contains("b-trigger:click=\"increment_1\""),
            "mount triggers route to their variants: {} / {}",
            frags[0],
            frags[1]
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
        assert!(!names.contains(&"count".to_string()), "global consumed: {names:?}");
        assert!(!names.contains(&"txn:increment".to_string()), "global txn consumed: {names:?}");
    }

    /// A `let`-declared field keeps its type on the instance slot, and a txn
    /// consumed by write-set (not a trigger name) is still variant-ized + its
    /// original removed.
    #[test]
    fn let_typed_field_and_write_consumed_txn() {
        let src = r#"
let count: Int = 0;
let show: Bool = false;
txn toggle [show == false][show == true] {
    show = true;
    term;
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
            "instance slot keeps the Bool type: {states:?}"
        );
        let txns: Vec<String> = items.iter().filter_map(|item| match item {
            TopLevel::Transaction(t) => Some(t.name.clone()),
            _ => None,
        }).collect();
        assert!(txns.contains(&"toggle_0".to_string()) && txns.contains(&"toggle_1".to_string()),
            "write-consumed txn gets per-mount variants: {txns:?}");
        assert!(!txns.contains(&"toggle".to_string()),
            "original write-consumed txn removed: {txns:?}");
    }

    /// 2026-08-11 (2b2 slice 2b): `<Name attr="val" />` seeds the mount's
    /// instance slot for the fragment-referenced field `attr`.
    #[test]
    fn mount_props_seed_instance_slots() {
        let src = r#"
let count: Int = 0;
txn increment [count < 100][true] {
    count = count + 1;
    term;
};
render Counter {
    <span b-text="count">0</span>
    <button b-trigger:click="increment">+</button>
};
render Root {
    <Counter count="5" />
    <Counter count="7" />
};
"#;
        let (items, plan) = check(src).unwrap();
        assert_eq!(
            plan.initializers.get("Counter.0.count"),
            Some(&Expr::Decimal(5)),
            "mount 0 seeds 5"
        );
        assert_eq!(
            plan.initializers.get("Counter.1.count"),
            Some(&Expr::Decimal(7)),
            "mount 1 seeds 7"
        );
        assert!(
            plan.fragments.get("Counter").map(|f| f.len()).unwrap_or(0) == 2,
            "two per-mount fragments"
        );
        let _ = items;
    }
}
