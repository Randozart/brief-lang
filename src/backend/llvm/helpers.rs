// ── Expression Codegen Helper Functions ─────────────────────────
//
// 2026-06-29: Extracted from emit_expr.rs to enable submodule extraction.
// Split via Rust's "impl block split" pattern — multiple files define
// `impl Type { ... }` within the same module without duplicating methods.
//
// 2026-07-13: Flattened to max 2-level nesting with guard clauses,
// doc comments on every definition, removed old-API references
// (IntrinsicCall → Call, Projection projection_target_name removed).
//
// Visibility convention:
//   `pub(crate)`  — visible to entire crate (semi-public API surface)
//   `pub(super)`  — visible to parent `llvm` module + children
//   (private)     — visible only within this file

use crate::ast::{BinaryOpKind, Expr, OutputType, Statement, Type};
use crate::backend::llvm::emit_stmt::emit_statement;
use crate::backend::llvm::*;
use crate::type_universe::ResolvedType;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

static TYPE_CONFIG: LazyLock<crate::config::TypeConfig> = LazyLock::new(|| {
    crate::config::TypeConfig::load()
});

/// Derive LLVM type string from a ResolvedType using the global type config.
/// 2026-07-17: Reads from normalizer-set llvm_type property first, falls back
/// to deriving from CTD via derive_llvm_type.
fn rt_llvm_type(rt: &ResolvedType) -> String {
    if let Some(crate::ast::PropertyValue::String(s)) = rt.properties.get("llvm_type") {
        return s.clone();
    }
    let ctd = rt.properties.get("ctd").and_then(|pv| {
        if let crate::ast::PropertyValue::Identifier(s) = pv { Some(s.as_str()) } else { None }
    });
    crate::config::derive_llvm_type(ctd, rt.bytes, &*TYPE_CONFIG)
}

/// 2026-07-12: Check if a type matches a canonical name via property
/// system (preferred) with hardcoded legacy fallback for types without
/// a universe entry (e.g. during bootstrap).
fn type_is(universe: &Option<crate::type_universe::TypeUniverse>, ty: &Type, name: &str) -> bool {
    if *ty == Type::Custom(name.to_string()) {
        return true;
    }
    if let Some(u) = universe {
        if let Some(key) = ty.universe_key() {
            if u.contains(key) {
                return name == key || u.get(key).map_or(false, |rt| rt.name == name);
            }
        }
    }
    false
}

impl LlvmBackend {
    // ═══════════════════════════════════════════════════════════════
    // Section 1: Cell Rewriting
    // ═══════════════════════════════════════════════════════════════

    /// Rewrite all `Identifier` nodes in an expression tree, prefixing
    /// each name with `cell${cell_name}$`. Used when expanding a cell
    /// definition into standalone transactions.
    ///
    /// 2026-06-29: Recursive tree walk — every compound variant recurses
    /// into its children. Leaf variants (literals, metadata) pass through.
    pub(super) fn rewrite_cell_identifiers(expr: &Expr, cell_name: &str) -> Expr {
        let prefix = |name: &str| -> String { format!("cell${}${}", cell_name, name) };
        match expr {
            // Leaves — no identifiers to rewrite
            Expr::Decimal(_) | Expr::Bool(_) | Expr::Float(_)
            | Expr::Quoted(_) | Expr::PropertyGet(_)
            | Expr::FormattingAnnotation(_) => expr.clone(),

            // Identifier leaf
            Expr::Identifier(name) => Expr::Identifier(prefix(name)),

            // Compound — recurse into children
            Expr::BinaryOp(k, l, r) => Expr::BinaryOp(
                *k,
                Box::new(Self::rewrite_cell_identifiers(l, cell_name)),
                Box::new(Self::rewrite_cell_identifiers(r, cell_name)),
            ),
            Expr::UnaryOp(k, e) => Expr::UnaryOp(
                *k,
                Box::new(Self::rewrite_cell_identifiers(e, cell_name)),
            ),
            Expr::Call(name, args, _) => Expr::Call(
                name.clone(),
                args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
                None,
            ),
            Expr::Field(obj, field) => Expr::Field(
                Box::new(Self::rewrite_cell_identifiers(obj, cell_name)),
                field.clone(),
            ),
            Expr::Index(obj, idx) => Expr::Index(
                Box::new(Self::rewrite_cell_identifiers(obj, cell_name)),
                Box::new(Self::rewrite_cell_identifiers(idx, cell_name)),
            ),
            Expr::Block(stmts) => Expr::Block(
                stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            ),
            Expr::If(cond, then_, else_) => Expr::If(
                Box::new(Self::rewrite_cell_identifiers(cond, cell_name)),
                Box::new(Self::rewrite_cell_identifiers(then_, cell_name)),
                else_.as_ref().map(|e| Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            ),
            Expr::Match(value, arms) => Expr::Match(
                Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                arms.iter().map(|arm| crate::ast::MatchArm {
                    pattern: arm.pattern.clone(),
                    guard: arm.guard.as_ref()
                        .map(|g| Self::rewrite_cell_identifiers(g, cell_name)),
                    body: Box::new(Self::rewrite_cell_identifiers(&arm.body, cell_name)),
                }).collect(),
            ),
            Expr::Tuple(items) | Expr::List(items) => Self::rewrite_tuple_or_list(expr, items, cell_name),
            Expr::Lambda(params, body) => Expr::Lambda(
                params.clone(),
                Box::new(Self::rewrite_cell_identifiers(body, cell_name)),
            ),
            Expr::Cast(e, ty) => Expr::Cast(
                Box::new(Self::rewrite_cell_identifiers(e, cell_name)),
                ty.clone(),
            ),
            Expr::IsType(e, ty) => Expr::IsType(
                Box::new(Self::rewrite_cell_identifiers(e, cell_name)),
                ty.clone(),
            ),
            Expr::Within(body, fallback) => Expr::Within(
                Box::new(Self::rewrite_cell_identifiers(body, cell_name)),
                Box::new(Self::rewrite_cell_identifiers(fallback, cell_name)),
            ),
            Expr::DerivationBlock(db) => Self::rewrite_derivation(db, cell_name),
            Expr::Deref(inner) => Expr::Deref(
                Box::new(Self::rewrite_cell_identifiers(inner, cell_name)),
            ),
            Expr::AddrOf(inner) => Expr::AddrOf(
                Box::new(Self::rewrite_cell_identifiers(inner, cell_name)),
            ),
            Expr::PluginIntercept { name, args, .. } => Expr::PluginIntercept {
                name: name.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
                type_args: vec![],
            },
        }
    }

    /// Shared helper for Tuple/List rewrite to avoid duplicating the
    /// match-guard logic in the parent function.
    fn rewrite_tuple_or_list(expr: &Expr, items: &[Expr], cell_name: &str) -> Expr {
        let mapped: Vec<Expr> = items.iter()
            .map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect();
        if matches!(expr, Expr::Tuple(_)) {
            Expr::Tuple(mapped)
        } else {
            Expr::List(mapped)
        }
    }

    /// Rewrite identifiers inside a DerivationBlock.
    fn rewrite_derivation(db: &crate::ast::DerivationBlock, cell_name: &str) -> Expr {
        Expr::DerivationBlock(crate::ast::DerivationBlock {
            examples: db.examples.iter().map(|ex| crate::ast::DerivationExample {
                inputs: ex.inputs.iter()
                    .map(|i| Self::rewrite_cell_identifiers(i, cell_name)).collect(),
                output: Box::new(Self::rewrite_cell_identifiers(&ex.output, cell_name)),
                span: ex.span,
            }).collect(),
            synthesized: db.synthesized.as_ref()
                .map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
            span: db.span,
        })
    }

    /// Rewrite all identifiers in a statement with a cell prefix.
    pub(super) fn rewrite_cell_stmt_identifiers(stmt: &Statement, cell_name: &str) -> Statement {
        match stmt {
            Statement::Assign(lhs, expr) => Statement::Assign(
                Self::rewrite_cell_identifiers(lhs, cell_name),
                Self::rewrite_cell_identifiers(expr, cell_name),
            ),
            Statement::Guarded(cond, stmts) => Statement::Guarded(
                Self::rewrite_cell_identifiers(cond, cell_name),
                stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            ),
            Statement::Term(e) => Statement::Term(e.as_ref()
                .map(|e| Self::rewrite_cell_identifiers(e, cell_name))),
            Statement::TermBang(e) => Statement::TermBang(e.as_ref()
                .map(|e| Self::rewrite_cell_identifiers(e, cell_name))),
            Statement::Return(e) => Statement::Return(e.as_ref()
                .map(|e| Self::rewrite_cell_identifiers(e, cell_name))),
            Statement::Escape(e) => Statement::Escape(e.as_ref()
                .map(|e| Self::rewrite_cell_identifiers(e, cell_name))),
            Statement::Expression(e) => {
                Statement::Expression(Self::rewrite_cell_identifiers(e, cell_name))
            }
            Statement::Let { name, ty, expr, modifiers } => Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref()
                    .map(|e| Self::rewrite_cell_identifiers(e, cell_name)),
                modifiers: modifiers.clone(),
            },
            Statement::If(cond, then_b, else_b) => Statement::If(
                Self::rewrite_cell_identifiers(cond, cell_name),
                then_b.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                else_b.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            ),
            Statement::Block(stmts) => Statement::Block(
                stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            ),
            Statement::SyncBlock(stmts) => Statement::SyncBlock(
                stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            ),
            Statement::InlineAsm { .. } => stmt.clone(),
            Statement::TrgBinding { name, instance, port } => Statement::TrgBinding {
                name: name.clone(),
                instance: Self::rewrite_cell_identifiers(instance, cell_name),
                port: port.clone(),
            },
            Statement::Foreach { item, list, body } => Statement::Foreach {
                item: item.clone(),
                list: Box::new(Self::rewrite_cell_identifiers(list, cell_name)),
                body: body.iter()
                    .map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            },
            Statement::MetadataAssignment(..) => stmt.clone(),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 2: Metadata & Structure
    // ═══════════════════════════════════════════════════════════════

    /// Extract all output variable names from an `OutputType` tree.
    /// Recursively collects names from Named, Tuple, and Union wrappers.
    pub(super) fn extract_output_names_llvm(ot: &Option<OutputType>) -> Vec<String> {
        let Some(ot) = ot else { return Vec::new(); };
        match ot {
            OutputType::Named(name, inner) => {
                let mut names = vec![name.clone()];
                names.extend(Self::extract_output_names_llvm(&Some(inner.as_ref().clone())));
                names
            }
            OutputType::Tuple(types) | OutputType::Union(types) => {
                types.iter().flat_map(|t| Self::extract_output_names_llvm(&Some(t.clone()))).collect()
            }
            OutputType::Single(_) | OutputType::Array(_) => Vec::new(),
        }
    }

    /// Emit a `main()` that stores final precomputed values and returns.
    /// EmitPureCounterFold: no runtime loop, no iteration. The region analyzer simulated
    /// all transactions within `--optimize-budget` and produced final values.
    /// This is the most extreme optimization: zero runtime memory traffic.
    pub(crate) fn emit_precomputed_main(
        &mut self,
        out: &mut String,
        final_values: &[(Vec<String>, HashMap<String, i64>)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_txn_id, bindings) in final_values {
            for (var, val) in bindings {
                if !seen.insert(var) {
                    continue;
                }
                if let Some(&idx) = self.ctx.field_index_map.get(var) {
                    let ty = &self.ctx.field_types[idx];
                    let gp = format!("%gp_{}", var);
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gp, idx).ok();
                    Self::emit_precomputed_store(out, &gp, ty, val);
                } else if let Some(&addr) = self.ctx.mmio_fields.get(var) {
                    let gp = format!("%gp_{}", var);
                    self.emit_inttoptr(out, "  ", &gp, &addr.to_string());
                    writeln!(out, "  store volatile i64 {}, ptr %gp_{}, align 1", val, var).ok();
                }
            }
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a single store for a precomputed field value.
    fn emit_precomputed_store(out: &mut String, gp: &str, ty: &str, val: &i64) {
        match ty.as_ref() {
            "float" => {
                let bits = *val as i32 as u32;
                writeln!(out, "  store float bitcast (i32 {} to float), ptr {}, align 4", bits, gp).ok();
            }
            "i8" => {
                writeln!(out, "  store i8 {}, ptr {}, align 1", val, gp).ok();
            }
            _ => {
                writeln!(out, "  store i64 {}, ptr {}, align 8", val, gp).ok();
            }
        }
    }

    /// Emit LLVM wake trigger metadata.
    /// 2026-07-13: Currently emits empty metadata (wake_triggers pending
    /// implementation in the reactive scheduler).
    pub(crate) fn emit_wake_metadata(&self, out: &mut String) {
        let wake_symbols: Vec<&str> = Vec::new();
        if wake_symbols.is_empty() {
            return;
        }
        let count = wake_symbols.len();
        let sym_list = wake_symbols.iter().map(|s| format!("ptr @{}", s)).collect::<Vec<_>>().join(", ");
        writeln!(out, "@llvm.wake_triggers = constant [{} x ptr] [{}]", count, sym_list).ok();
        writeln!(out, "!llvm.wake_triggers = !{{!6}}").ok();
        write!(out, "!6 = !{{").ok();
        for (i, sym) in wake_symbols.iter().enumerate() {
            if i > 0 {
                write!(out, ", ").ok();
            }
            write!(out, "!\"{}\"", sym).ok();
        }
        writeln!(out, "}}").ok();
    }

    /// Emit LLVM thread pool metadata for async transactions.
    /// Generates a constant array of function pointers consumed by
    /// `brief_thread_pool_init` at startup.
    pub(crate) fn emit_thread_pool_metadata(&self, out: &mut String) {
        if !self.has_async_txns || self.is_lightweight_async {
            return;
        }
        let count = self.async_txn_names.len();
        let fn_list: Vec<String> = self.async_txn_names.iter()
            .map(|n| format!("i8* bitcast (void (ptr)* @async_body_{} to ptr)", n))
            .collect();
        writeln!(out, "@llvm.thread_pool = constant [{} x ptr] [{}]",
            count, fn_list.join(", ")).ok();
        writeln!(out, "@thread_pool_fns = private constant [{} x void (ptr)*] [{}]",
            count,
            self.async_txn_names.iter()
                .map(|n| format!("void (ptr)* @async_body_{}", n))
                .collect::<Vec<_>>().join(", "),
        ).ok();
    }

    /// Emit the async phase calls in main: set state for workers, release
    /// workers, wait for workers.
    ///
    /// 2026-07-01: `reactor_tick` is now a no-op when the thread pool is
    /// active. Worker threads execute async bodies on the correct state
    /// snapshot (set via `__set_async_state__`), synchronized by barriers.
    pub(crate) fn emit_async_phase(&self, out: &mut String, state_var: &str) {
        if !self.has_async_txns || self.is_lightweight_async {
            return;
        }
        writeln!(out, "  call void @__set_async_state__(ptr {})", state_var).ok();
        writeln!(out, "  call void @__barrier_release__()").ok();
        writeln!(out, "  call void @reactor_tick(ptr noalias nocapture {})", state_var).ok();
        writeln!(out, "  call void @__barrier_wait__()").ok();
    }

    /// Detect pairs of reactive transactions that can be fused.
    /// Fusion requires: both reactive, non-async, non-overlapping writes,
    /// no trigger references in the second's precondition.
    pub(crate) fn resolve_fusable_pairs(
        &self,
        txns: &[(String, &crate::ast::Transaction)],
    ) -> Vec<(String, String)> {
        let items: Vec<crate::ast::TopLevel> = txns.iter()
            .map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect();
        let mut pairs = crate::backend::detect_fusable_pairs(&items);
        pairs.retain(|(a, b)| {
            let Some((_, ta)) = txns.iter().find(|(n, _)| n == a) else { return false; };
            let Some((_, tb)) = txns.iter().find(|(n, _)| n == b) else { return false; };
            if ta.is_async || tb.is_async {
                return false;
            }
            if !ta.is_reactive || !tb.is_reactive {
                return false;
            }
            let aw = crate::backend::collect_assigned_identifiers(&ta.body);
            let bw = crate::backend::collect_assigned_identifiers(&tb.body);
            if aw.iter().any(|w| bw.contains(w)) {
                return false;
            }
            if self.trg_in_pre(&tb.contract.pre_condition) {
                return false;
            }
            true
        });
        pairs
    }

    /// Check if any trigger name appears in a precondition expression.
    pub(crate) fn trg_in_pre(&self, pre: &Expr) -> bool {
        let mut ids = std::collections::HashSet::new();
        crate::backend::collect_expr_identifiers(pre, &mut ids);
        ids.iter().any(|id| self.ctx.trigger_names.contains(id))
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 3: Cast & Type Conversion
    // ═══════════════════════════════════════════════════════════════

    /// Emit LLVM IR for a type cast between Brief types.
    /// Handles the full matrix of Int/Float/Bool/Char/String conversions.
    /// Falls back to identity (add i64 0, src) when the types match or
    /// no conversion is known.
    pub(crate) fn emit_cast_convert(
        &mut self,
        out: &mut String,
        indent: &str,
        dst: &str,
        src: &str,
        src_ty: Option<Type>,
        target: &Type,
    ) {
        let src_ty = match src_ty {
            Some(t) => t,
            None => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
                return;
            }
        };
        if &src_ty == target {
            let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            return;
        }
        let (src_name, target_name) = match (&src_ty, target) {
            (Type::Custom(s), Type::Custom(t)) => (s.as_str(), t.as_str()),
            _ => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
                return;
            }
        };
        self.emit_typed_cast(out, indent, dst, src, src_name, target_name);
    }

    /// Dispatch to the correct cast emission based on (src, target) pair.
    /// 2026-07-13: Each cast pair extracted into a named helper for
    /// clarity and to keep nesting ≤ 2 levels.
    fn emit_typed_cast(
        &mut self,
        out: &mut String,
        indent: &str,
        dst: &str,
        src: &str,
        src_name: &str,
        target_name: &str,
    ) {
        match (src_name, target_name) {
            ("Int" | "UInt", "Float") => {
                let _ = writeln!(out, "{}{} = sitofp i64 {} to float", indent, dst, src);
            }
            ("Float", "Int" | "UInt") => self.cast_float_to_int(out, indent, dst, src),
            ("Int" | "UInt", "Bool") => self.cast_to_bool(out, indent, dst, src),
            ("Bool", "Int" | "UInt") => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            ("Float", "Bool") => self.cast_float_to_bool(out, indent, dst, src),
            ("Bool", "Float") => self.cast_bool_to_float(out, indent, dst, src),
            ("Char", "Bool") => self.cast_to_bool(out, indent, dst, src),
            ("Bool", "Char") => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            ("Char", "Int" | "UInt") => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            ("Int" | "UInt", "Char") => {
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, dst, src);
            }
            ("Char", "String") => self.cast_char_to_string(out, indent, dst, src),
            ("String", "Char") => self.cast_string_to_char(out, indent, dst, src),
            ("Int" | "UInt", "String") => {
                let _ = writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, dst, src);
            }
            ("String", "Int" | "UInt") => self.cast_string_to_int(out, indent, dst, src),
            ("String", "Bool") => self.cast_string_to_bool(out, indent, dst, src),
            ("Bool", "String") => self.cast_bool_to_string(out, indent, dst, src),
            _ => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
        }
    }

    fn cast_float_to_int(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let tr = self.next_reg_with_prefix("cfltr");
        let fl = self.next_reg_with_prefix("cflfl");
        let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
        let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
        let _ = writeln!(out, "{}{} = fptosi float {} to i64", indent, dst, fl);
    }

    fn cast_to_bool(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let ci = self.next_reg_with_prefix("ccb");
        let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
        let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
    }

    fn cast_float_to_bool(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let tr = self.next_reg_with_prefix("cfbtr");
        let fl = self.next_reg_with_prefix("cfbfl");
        let ci = self.next_reg_with_prefix("cfbci");
        let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
        let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
        let _ = writeln!(out, "{}{} = fcmp fast une float {}, 0.0", indent, ci, fl);
        let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
    }

    fn cast_bool_to_float(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let ci = self.next_reg_with_prefix("cbfci");
        let fl = self.next_reg_with_prefix("cbffl");
        let fi = self.next_reg_with_prefix("cbffi");
        let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
        let _ = writeln!(out, "{}{} = select i1 {}, float 1.000000e+00, float 0.000000e+00", indent, fl, ci);
        let _ = writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fl);
        let _ = writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, fi);
    }

    fn cast_char_to_string(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let tr = self.next_reg_with_prefix("cctr");
        let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
        let alloc = self.next_reg_with_prefix("ccac");
        let _ = writeln!(out, "{}{} = call ptr @malloc(i64 24)", indent, alloc);
        let hp = self.next_reg_with_prefix("cchp");
        let _ = writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hp, alloc);
        let base = self.next_reg_with_prefix("ccba");
        let _ = writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, alloc);
        let dp = self.next_reg_with_prefix("ccdp");
        let _ = writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base);
        let _ = writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, dp, hp);
        let ls = self.next_reg_with_prefix("ccls");
        let _ = writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ls, hp);
        let _ = writeln!(out, "{}store i64 1, ptr {}, align 8", indent, ls);
        let cs = self.next_reg_with_prefix("cccs");
        let _ = writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 16", indent, cs, alloc);
        let tb = self.next_reg_with_prefix("cctb");
        let _ = writeln!(out, "{}{} = trunc i32 {} to i8", indent, tb, tr);
        let _ = writeln!(out, "{}store i8 {}, ptr {}, align 1", indent, tb, cs);
        let nt = self.next_reg_with_prefix("ccnt");
        let _ = writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 17", indent, nt, alloc);
        let _ = writeln!(out, "{}store i8 0, ptr {}, align 1", indent, nt);
        let _ = writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, dst, alloc);
    }

    fn cast_string_to_char(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let ip = self.next_reg_with_prefix("csip");
        self.emit_inttoptr(out, indent, &ip, &src);
        let lb = self.next_reg_with_prefix("cslb");
        let _ = writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, lb, ip);
        let _ = writeln!(out, "{}{} = zext i8 {} to i64", indent, dst, lb);
    }

    fn cast_string_to_int(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let ip = self.next_reg_with_prefix("csii");
        self.emit_inttoptr(out, indent, &ip, &src);
        let _ = writeln!(out, "{}{} = call i64 @__str_to_int(i8* {})", indent, dst, ip);
    }

    fn cast_string_to_bool(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let ip = self.next_reg_with_prefix("csbi");
        let lb = self.next_reg_with_prefix("csbl");
        let ci = self.next_reg_with_prefix("csbc");
        self.emit_inttoptr(out, indent, &ip, &src);
        let _ = writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, lb, ip);
        let _ = writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, ci, lb);
        let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
    }

    fn cast_bool_to_string(&mut self, out: &mut String, indent: &str, dst: &str, src: &str) {
        let ci = self.next_reg_with_prefix("cbsc");
        let ip = self.next_reg_with_prefix("cbsi");
        let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
        let _ = writeln!(out, "{}{} = call i8* @__chr_to_str(i32 {})", indent, ip, ci);
        let _ = writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, dst, ip);
    }

    /// Convert an i64 boxed float to a native float register.
    /// Checks the `reg_float_cache` first to avoid duplicate bitcast chains.
    /// Truncate an i64 (Int) register to i1 (Bool) for branch conditions.
    /// Non-Int types pass through as-is (already bool-width).
    pub(super) fn as_bool_reg(
        &mut self,
        out: &mut String,
        indent: &str,
        reg: &TypedRegister,
    ) -> String {
        if type_is(&self.ctx.type_universe, &reg.ty, "Int") {
            let t = self.next_reg_with_prefix("tb");
            writeln!(out, "{}{} = trunc i64 {} to i1", indent, t, reg.name).ok();
            t
        } else if type_is(&self.ctx.type_universe, &reg.ty, "Bool") {
            // 2026-07-14: Bool is i8 — trunc to i1 for br
            let t = self.next_reg_with_prefix("tb");
            writeln!(out, "{}{} = trunc i8 {} to i1", indent, t, reg.name).ok();
            t
        } else {
            reg.name.clone()
        }
    }

    /// Convert a String/Data typed register to i64 for C ABI calls.
    /// Int/Bool/Char/Float registers pass through as-is.
    fn ptrtoint_if_string(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        if type_is(&self.ctx.type_universe, &reg.ty, "String")
            || type_is(&self.ctx.type_universe, &reg.ty, "Data")
        {
            let p = self.next_reg_with_prefix("ptri");
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p, reg.name).ok();
            p
        } else {
            reg.name.clone()
        }
    }

    /// Allocate a register name with a counter-based prefix.
    /// Convenience wrapper around `next_reg_with_prefix` from the function context.
    fn next_reg_with_prefix(&mut self, prefix: &str) -> String {
        self.fun.next_reg_with_prefix(prefix)
    }

    /// Check if a type is `Ptr<T>` or a layout-constrained pointer.
    fn is_ptr_ty(ty: &Type) -> bool {
        if let Type::Applied(name, _) = ty {
            name == "Ptr"
        } else {
            matches!(ty, Type::LayoutPtr(_))
        }
    }

    /// Look up the LLVM codegen type for a Brief type.
    /// Returns `"float"` for Native storage types ≤32 bits, `"double"`
    /// for >32, and `"i64"` for Boxed or unknown types.
    /// 2026-07-17: Reads ALU property instead of primitive().
    fn operator_llvm_type(&self, ty: &Type) -> &'static str {
        if let Some(ref universe) = self.ctx.type_universe {
            if let Some(rt) = ty.universe_key().and_then(|k| universe.get(k)) {
                let is_float = rt.properties.get("alu").and_then(|pv| match pv {
                    crate::ast::PropertyValue::Identifier(s) => Some(s.as_str() == "Float"),
                    _ => None,
                }).unwrap_or(false);
                if is_float && rt.bytes <= 4 { return "float"; }
                if is_float { return "double"; }
                return "i64";
            }
        }
        if ty == &Type::float() {
            "float"
        } else if ty == &Type::float64() {
            "double"
        } else {
            "i64"
        }
    }

    /// Check if an expression is a reference to a linked String-like trigger.
    fn is_linked_string_trigger(&self, expr: &Expr) -> bool {
        let Expr::Identifier(name) = expr else { return false; };
        let Some(trg) = self.ctx.triggers.get(name) else { return false; };
        // 2026-07-18: Replaced `type_is(..., "String")` with `is_string_like`.
        // `is_string_like` matches via CTD="String" (Phase A) or shape+encoding (Phase B).
        // Data is still checked by name since it has no encoding property.
        self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(&trg.ty))
            || type_is(&self.ctx.type_universe, &trg.ty, "Data")
    }

    /// Emit a cached projection: load valid flag, branch on hit/miss.
    /// Hit: load cached value. Miss: compute, store in cache, set flag.
    /// Phi merges hit/miss paths. Cache slots are appended to %State by
    /// dead-field elimination (apply_field_modes).
    pub(crate) fn try_cached_projection(
        &mut self,
        out: &mut String,
        source_expr: &Expr,
        src_val: &TypedRegister,
        target_name: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        let field_name = match source_expr {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        let &(cache_idx, valid_idx) = self.ctx.cache_slots.get(&field_name)
            .and_then(|targets| targets.get(target_name))?;

        let v = self.next_reg_with_prefix("t");
        let valid_gep = self.next_reg_with_prefix("cvp");
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, valid_gep, valid_idx).ok();
        let valid_load = self.next_reg_with_prefix("cvv");
        writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, valid_load, valid_gep).ok();
        let valid_cond = self.next_reg_with_prefix("cvc");
        writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, valid_cond, valid_load).ok();

        let hit_label = format!(".chit{}", self.fun.txn_counter);
        let miss_label = format!(".cmiss{}", self.fun.txn_counter);
        let merge_label = format!(".cmerge{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, valid_cond, hit_label, miss_label).ok();
        writeln!(out, "{}:", hit_label).ok();
        let cache_gep = self.next_reg_with_prefix("cve");
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, cache_gep, cache_idx).ok();
        let cache_val = self.next_reg_with_prefix("cvv");
        writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, cache_val, cache_gep).ok();
        writeln!(out, "{}br label %{}", indent, merge_label).ok();
        writeln!(out, "{}:", miss_label).ok();
        writeln!(out, "{}{} = add i64 0, {}", indent, v, src_val.name).ok();
        let store_gep = self.next_reg_with_prefix("cse");
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, store_gep, cache_idx).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !1", indent, v, store_gep).ok();
        let valid_store_gep = self.next_reg_with_prefix("csve");
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, valid_store_gep, valid_idx).ok();
        writeln!(out, "{}store i8 1, ptr {}, align 1", indent, valid_store_gep).ok();
        writeln!(out, "{}br label %{}", indent, merge_label).ok();
        writeln!(out, "{}:", merge_label).ok();
        let phi_reg = self.next_reg_with_prefix("cp");
        writeln!(out, "{}{} = phi i64 [ {}, %{} ], [ {}, %{} ]",
            indent, phi_reg, cache_val, hit_label, v, miss_label).ok();
        Some(TypedRegister { name: phi_reg, ty: Type::int() })
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 4: String Operations
    // ═══════════════════════════════════════════════════════════════

    /// Emit inline string concatenation: malloc + header setup + memcpy.
    /// Both operands are i8* (Brief header pointers). Returns i64-tagged.
    ///
    /// Tag convention (2026-06-19):
    ///   bit 0 = static string constant (don't free, don't read header at -16)
    ///   bit 1 = temporary concat result (safe to free when consumed)
    ///   State-loaded strings have both bits clear (heap, state-owned).
    ///
    /// Why inline instead of sprintf/strcat: the compiler knows each
    /// operand's length at emit time (from header slot 1), so it computes
    /// the total allocation and emits memcpy calls that LLVM lowers to
    /// `rep movsb` or inline. sprintf scans for null terminators at runtime,
    /// losing length information.
    pub(crate) fn emit_inline_concat(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
    ) -> TypedRegister {
        // 2026-07-18: Phase B — SSO concat path.
        if self.feature_sso_strings {
            return self.emit_sso_concat(out, indent, a, b);
        }
        let a_boxed = self.adapt_to_i64(out, indent, a);
        let b_boxed = self.adapt_to_i64(out, indent, b);
        let a_clean = self.emit_mask_tag(out, indent, &a_boxed, "cam");
        let b_clean = self.emit_mask_tag(out, indent, &b_boxed, "cbm");
        let ha = self.emit_inttoptr_reg(out, indent, "cha", &a_clean);
        let la = self.emit_load_length(out, indent, &ha, "clp", "cla");
        let hb = self.emit_inttoptr_reg(out, indent, "chb", &b_clean);
        let lb = self.emit_load_length(out, indent, &hb, "clq", "clb");
        let total = self.emit_add_const(out, indent, &la, &lb, "ctl");
        let alloc_size = self.compute_alloc_size(out, indent, &total, "chs", "cas");
        let result_i64 = self.emit_arena_alloc(out, indent, &alloc_size);
        // 2026-07-19: emit_arena_alloc returns i64 — inttoptr to ptr for helpers.
        let result_ptr = self.fun.next_reg_with_prefix("crp");
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, result_ptr, result_i64).ok();
        self.emit_write_header(out, indent, &result_ptr, &total, "chp", "cba", "cdp", "cls");
        self.emit_copy_data(out, indent, &result_ptr, &ha, &la, &a_clean, "cad", "cac", "cds", "cdo");
        self.emit_copy_data(out, indent, &result_ptr, &hb, &lb, &b_clean, "cbd", "cbc", "cdo_off", "cdo2");
        self.emit_null_terminate(out, indent, &result_ptr, &total, "cnt");
        self.emit_free_temporaries(out, indent, &a_boxed, &b_boxed, "cta", "cia", "ctb", "cib");
        self.emit_box_concat_result(out, indent, &result_ptr, "t")
    }

    // 2026-07-18: SSO-aware concat. When both operands are SSO inline and total
    // bytes ≤ 6, packs into a new SSO handle. Otherwise allocates raw heap buffer
    // (no 16-byte header) and returns heap handle with tag 0b000.
    fn emit_sso_concat(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
    ) -> TypedRegister {
        let a_reg = &a.name;
        let b_reg = &b.name;
        // Extract handle[1] (length) from both
        let a_len = self.fun.gen_reg();
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 1", indent, a_len, a_reg).ok();
        let b_len = self.fun.gen_reg();
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 1", indent, b_len, b_reg).ok();
        let total_len = self.fun.gen_reg();
        writeln!(out, "{}{} = add i64 {}, {}", indent, total_len, a_len, b_len).ok();
        // Extract handle[0] (data/tag) from both
        let a_dtag = self.fun.gen_reg();
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, a_dtag, a_reg).ok();
        let b_dtag = self.fun.gen_reg();
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, b_dtag, b_reg).ok();
        // Check if total ≤ 6 (SSO threshold) — if so, use SSO inline path
        let cmp = self.fun.gen_reg();
        writeln!(out, "{}{} = icmp ule i64 {}, 6", indent, cmp, total_len).ok();
        let sso_label = format!("sso_con_{}", self.fun.txn_counter);
        let heap_label = format!("heap_con_{}", self.fun.txn_counter);
        let done_label = format!("done_con_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cmp, sso_label, heap_label).ok();
        // ── SSO inline path ──────────────────────────────────────────
        writeln!(out, "{}{}:", indent, sso_label).ok();
        // Extract packed data: (handle[0] >> 3) & mask for SSO, or memcpy for heap
        // For SSO inline, handle[0] = (data << 3) | 1. So data = handle[0] >> 3.
        let a_data = self.fun.gen_reg();
        writeln!(out, "{}{} = lshr i64 {}, 3", indent, a_data, a_dtag).ok();
        let b_data = self.fun.gen_reg();
        writeln!(out, "{}{} = lshr i64 {}, 3", indent, b_data, b_dtag).ok();
        // Shift b_data left by a_len * 8 bits to position it after a's bytes
        let b_shifted = self.fun.gen_reg();
        let a_len_8 = self.fun.gen_reg();
        writeln!(out, "{}{} = shl i64 {}, 3", indent, a_len_8, a_len).ok();
        writeln!(out, "{}{} = shl i64 {}, {}", indent, b_shifted, b_data, a_len_8).ok();
        // Combine: result_data = a_data | b_shifted
        let combined = self.fun.gen_reg();
        writeln!(out, "{}{} = or i64 {}, {}", indent, combined, a_data, b_shifted).ok();
        // Shift left 3 for tag and set SSO tag (bit 0)
        let sso_tag = self.fun.gen_reg();
        writeln!(out, "{}{} = shl i64 {}, 3", indent, sso_tag, combined).ok();
        let new_handle0 = self.fun.gen_reg();
        writeln!(out, "{}{} = or i64 {}, 1", indent, new_handle0, sso_tag).ok();
        // Build {i64, i64} result
        let sso_iv = self.fun.gen_reg();
        writeln!(out, "{}{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0", indent, sso_iv, new_handle0).ok();
        let sso_res = self.fun.gen_reg();
        writeln!(out, "{}{} = insertvalue {{ i64, i64 }} %{}, i64 {}, 1", indent, sso_res, sso_iv, total_len).ok();
        writeln!(out, "{}br label %{}", indent, done_label).ok();
        // ── Heap path ────────────────────────────────────────────────
        writeln!(out, "{}{}:", indent, heap_label).ok();
        // Allocate raw bytes + null terminator (no 16-byte header)
        let alloc_sz = self.fun.gen_reg();
        writeln!(out, "{}{} = add i64 {}, 1", indent, alloc_sz, total_len).ok();
        let heap_buf_i64 = self.emit_arena_alloc(out, indent, &alloc_sz);
        // 2026-07-19: emit_arena_alloc returns i64 — inttoptr to ptr for use.
        let heap_buf = self.fun.gen_reg();
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, heap_buf, heap_buf_i64).ok();
        // Mask tag bits for data pointer: handle[0] & -8
        let a_ptr_raw = self.fun.gen_reg();
        writeln!(out, "{}{} = and i64 {}, -8", indent, a_ptr_raw, a_dtag).ok();
        let b_ptr_raw = self.fun.gen_reg();
        writeln!(out, "{}{} = and i64 {}, -8", indent, b_ptr_raw, b_dtag).ok();
        // Convert to pointers
        let a_ptr = self.fun.gen_reg();
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, a_ptr, a_ptr_raw).ok();
        let b_ptr = self.fun.gen_reg();
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, b_ptr, b_ptr_raw).ok();
        let dest = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 0", indent, dest, heap_buf).ok();
        // memcpy a's data into buffer
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, ptr {}, i64 {}, i1 false)",
            indent, dest, a_ptr, a_len).ok();
        // memcpy b's data after a's data
        let b_dest = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, b_dest, heap_buf, a_len).ok();
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, ptr {}, i64 {}, i1 false)",
            indent, b_dest, b_ptr, b_len).ok();
        // null terminator
        let nt = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, nt, heap_buf, total_len).ok();
        writeln!(out, "{}store i8 0, ptr {}", indent, nt).ok();
        // Build {i64, i64} with heap tag (0b000 — no bits set)
        let heap_p2i = self.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, heap_p2i, heap_buf).ok();
        let heap_iv = self.fun.gen_reg();
        writeln!(out, "{}{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0", indent, heap_iv, heap_p2i).ok();
        let heap_res = self.fun.gen_reg();
        writeln!(out, "{}{} = insertvalue {{ i64, i64 }} %{}, i64 {}, 1", indent, heap_res, heap_iv, total_len).ok();
        writeln!(out, "{}br label %{}", indent, done_label).ok();
        // ── Done: phi the result ─────────────────────────────────────
        writeln!(out, "{}{}:", indent, done_label).ok();
        let phi_res = self.fun.gen_reg();
        writeln!(out, "{}{} = phi {{ i64, i64 }} [ %{}, %{} ], [ %{}, %{} ]",
            indent, phi_res, sso_res, sso_label, heap_res, heap_label).ok();
        TypedRegister { name: phi_res, ty: Type::string() }
    }

    /// Mask off tag bits (bit 0 = static, bit 1 = temp) from a boxed string.
    fn emit_mask_tag(&mut self, out: &mut String, indent: &str, val: &str, prefix: &str) -> String {
        let r = self.fun.next_reg_with_prefix(prefix);
        // 2026-07-18: SSO uses 3 tag bits (AND -8); legacy uses 2 bits (AND -4).
        let mask = if self.feature_sso_strings { -8i64 } else { -4i64 };
        writeln!(out, "{}{} = and i64 {}, {}", indent, r, val, mask).ok();
        r
    }

    /// Ptrtoint for a string register: load data pointer.
    fn emit_inttoptr_reg(&mut self, out: &mut String, indent: &str, prefix: &str, val: &str) -> String {
        let r = self.fun.next_reg_with_prefix(prefix);
        self.emit_inttoptr(out, indent, &r, &val);
        r
    }

    /// Load the length from a string header's slot at index 1.
    fn emit_load_length(
        &mut self,
        out: &mut String,
        indent: &str,
        header_ptr: &str,
        gep_prefix: &str,
        load_prefix: &str,
    ) -> String {
        let lp = self.fun.next_reg_with_prefix(gep_prefix);
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, header_ptr).ok();
        let l = self.fun.next_reg_with_prefix(load_prefix);
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, l, lp).ok();
        l
    }

    /// Emit `add i64 a, b`.
    fn emit_add_const(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &str,
        b: &str,
        prefix: &str,
    ) -> String {
        let r = self.fun.next_reg_with_prefix(prefix);
        writeln!(out, "{}{} = add i64 {}, {}", indent, r, a, b).ok();
        r
    }

    /// Compute total allocation size: 16 (header) + total_chars + 1 (null).
    fn compute_alloc_size(
        &mut self,
        out: &mut String,
        indent: &str,
        total_chars: &str,
        header_prefix: &str,
        alloc_prefix: &str,
    ) -> String {
        let hs = self.fun.next_reg_with_prefix(header_prefix);
        writeln!(out, "{}{} = add i64 16, {}", indent, hs, total_chars).ok();
        let as_ = self.fun.next_reg_with_prefix(alloc_prefix);
        writeln!(out, "{}{} = add i64 {}, 1", indent, as_, hs).ok();
        as_
    }

    /// Write the capacity header (data pointer offset), length slot to a new
    /// string allocation.
    fn emit_write_header(
        &mut self,
        out: &mut String,
        indent: &str,
        result: &str,
        total: &str,
        hp_prefix: &str,
        base_prefix: &str,
        dp_prefix: &str,
        len_prefix: &str,
    ) {
        let hp = self.fun.next_reg_with_prefix(hp_prefix);
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hp, result).ok();
        let base = self.fun.next_reg_with_prefix(base_prefix);
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, result).ok();
        let dp = self.fun.next_reg_with_prefix(dp_prefix);
        writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, dp, hp).ok();
        let ls = self.fun.next_reg_with_prefix(len_prefix);
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ls, hp).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, total, ls).ok();
    }

    /// Copy string data from one allocation to another via memcpy.
    fn emit_copy_data(
        &mut self,
        out: &mut String,
        indent: &str,
        result: &str,
        header: &str,
        length: &str,
        clean: &str,
        dp_prefix: &str,
        chars_prefix: &str,
        dest_prefix: &str,
        dest_off_prefix: &str,
    ) {
        let dp = self.fun.next_reg_with_prefix(dp_prefix);
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, header).ok();
        let chars = self.fun.next_reg_with_prefix(chars_prefix);
        self.emit_inttoptr(out, indent, &chars, &dp);
        let dest = self.fun.next_reg_with_prefix(dest_prefix);
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 16", indent, dest, result).ok();
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, ptr {}, i64 {}, i1 false)",
            indent, dest, chars, length).ok();
        let _ = dest_off_prefix; // keep for API compatibility
    }

    /// Write a null terminator at the end of the string data.
    fn emit_null_terminate(
        &mut self,
        out: &mut String,
        indent: &str,
        result: &str,
        total: &str,
        prefix: &str,
    ) {
        let nt = self.fun.next_reg_with_prefix(prefix);
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, nt, result, total).ok();
        writeln!(out, "{}store i8 0, ptr {}, align 1", indent, nt).ok();
    }

    /// Free heap-allocated temporaries (bit 1 set) when arena is not active.
    /// Static constants (bit 0=1) and state fields (bit 0=0,bit 1=0) preserved.
    fn emit_free_temporaries(
        &mut self,
        out: &mut String,
        indent: &str,
        a_boxed: &str,
        b_boxed: &str,
        tag_a_prefix: &str,
        is_a_prefix: &str,
        tag_b_prefix: &str,
        is_b_prefix: &str,
    ) {
        if self.fun.arena_slots.is_some() {
            return;
        }
        self.emit_free_one_temp(out, indent, a_boxed, tag_a_prefix, is_a_prefix, "free_a", "af_a");
        self.emit_free_one_temp(out, indent, b_boxed, tag_b_prefix, is_b_prefix, "free_b", "af_b");
    }

    /// Check tag bit 1 (legacy) or bit 2 (SSO) and conditionally free a
    /// temporary string allocation.
    fn emit_free_one_temp(
        &mut self,
        out: &mut String,
        indent: &str,
        boxed: &str,
        tag_prefix: &str,
        is_prefix: &str,
        free_label: &str,
        after_label: &str,
    ) {
        let tag = self.fun.next_reg_with_prefix(tag_prefix);
        // 2026-07-18: SSO uses bit 2 (value 4) for temporary; legacy uses bit 1 (value 2).
        let temp_bit = if self.feature_sso_strings { 4i64 } else { 2i64 };
        writeln!(out, "{}{} = and i64 {}, {}", indent, tag, boxed, temp_bit).ok();
        let is_temp = self.fun.next_reg_with_prefix(is_prefix);
        writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, is_temp, tag).ok();
        let fl = format!("{}_{}", free_label, self.fun.txn_counter);
        let afl = format!("{}_{}", after_label, self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_temp, fl, afl).ok();
        writeln!(out, "{}{}:", indent, fl).ok();
        let clean = self.emit_mask_tag(out, indent, boxed, &format!("cc{}", tag_prefix));
        let free_ptr = self.emit_inttoptr_reg(out, indent, &format!("cf{}", tag_prefix), &clean);
        writeln!(out, "{}call void @free(ptr {})", indent, free_ptr).ok();
        writeln!(out, "{}br label %{}", indent, afl).ok();
        writeln!(out, "{}{}:", indent, afl).ok();
    }

    /// Box a concat result pointer to i64 with temporary tag (bit 1 set).
    fn emit_box_concat_result(
        &mut self,
        out: &mut String,
        indent: &str,
        result: &str,
        prefix: &str,
    ) -> TypedRegister {
        let v = self.fun.next_reg_with_prefix(prefix);
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, v, result).ok();
        let vi = self.fun.next_reg_with_prefix(prefix);
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, vi, v).ok();
        let vi_tagged = self.fun.next_reg_with_prefix(prefix);
        writeln!(out, "{}{} = or i64 {}, 2", indent, vi_tagged, vi).ok();
        TypedRegister { name: vi_tagged, ty: Type::int() }
    }

    /// Recursively detect if an expression chain produces a String/Data value.
    /// Used by `emit_inline_concat` to decide between inline concat vs
    /// generic `add i64` for `a + b` on String operands.
    /// 2026-07-18: Replaced `type_is(..., "String")` with `is_string_like`.
    pub(crate) fn is_string_chain(&self, e: &Expr) -> bool {
        match e {
            Expr::Quoted(_) => true,
            Expr::Identifier(name) => self.is_string_identifier(name),
            Expr::BinaryOp(k, l, r) if matches!(k, crate::ast::BinaryOpKind::Add | crate::ast::BinaryOpKind::Concat) => {
                self.is_string_chain(l) || self.is_string_chain(r)
            }
            Expr::Cast(inner, target_ty) => {
                // Phase A: is_string_like (CTD="String") + Data name check.
                // Phase B: pure is_string_like (shape+encoding) replaces both.
                self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(target_ty))
                    || type_is(&self.ctx.type_universe, target_ty, "Data")
                    || self.is_string_chain(inner)
            }
            _ => false,
        }
    }

    /// Check if an identifier resolves to String/Data type via let bindings
    /// or struct field type hints.
    /// 2026-07-18: Replaced `type_is(..., "String")` with `is_string_like`.
    fn is_string_identifier(&self, name: &str) -> bool {
        let is_like = |t: &Type| -> bool {
            self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(t))
                || type_is(&self.ctx.type_universe, t, "Data")
        };
        if self.fun.let_binding_types.get(name).map_or(false, |t| is_like(t)) {
            return true;
        }
        if self.fun.let_original_types.get(name).map_or(false, |t| is_like(t)) {
            return true;
        }
        self.ctx.field_index_map.get(name)
            .and_then(|&idx| self.ctx.field_types.get(idx))
            .map(|ft| ft == "i8*" || ft == "ptr")
            .unwrap_or(false)
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 5: Binary Operations
    // ═══════════════════════════════════════════════════════════════

    /// Emit LLVM IR for a binary operation between two expressions.
    /// Handles constant folding, native float ops, mixed float/int,
    /// fixed-width integer ops, and a generic i64 fallback.
    ///
    /// 2026-07-13: Phase 7B (custom operator dispatch) removed — the
    /// `phase7b_l`/`phase7b_r` variables were always `None` during the
    /// rewrite period. Operators are resolved via the projection system
    /// before reaching this function.
    pub(crate) fn emit_binop(
        &mut self,
        out: &mut String,
        indent: &str,
        l: &Expr,
        r: &Expr,
        int_op: &str,
        float_op: &str,
    ) -> TypedRegister {
        // Constant-fold integer binops at compile time
        let int_op_clean = int_op.strip_suffix(" nsw").unwrap_or(int_op);
        if let Some(folded) = self.try_fold_binop_constants(out, indent, l, r, int_op_clean) {
            return folded;
        }
        let a = self.emit_expr(out, l, indent);
        let b = self.emit_expr(out, r, indent);
        let a_is_native = self.is_native_float(&a.ty);
        let b_is_native = self.is_native_float(&b.ty);
        let dedup_key = self.build_dedup_key(Self::dedup_op(a_is_native, b_is_native, int_op, float_op), &a, &b);
        if let Some(cached) = self.check_dedup_cache(&dedup_key) {
            let result_ty = a.ty.clone();
            return TypedRegister { name: cached, ty: result_ty };
        }
        let ptr_ty = self.infer_ptr_type(&a.ty, &b.ty);
        if a_is_native && b_is_native && a.ty == b.ty {
            return self.emit_native_float_binop(out, indent, &a, &b, float_op, &dedup_key);
        }
        if a_is_native || b_is_native {
            return self.emit_mixed_binop(out, indent, &a, &b, int_op, &dedup_key, ptr_ty);
        }
        if !self.is_native_float(&a.ty) && a.ty == b.ty {
            return self.emit_fixed_width_binop(out, indent, &a, &b, int_op, &dedup_key);
        }
        self.emit_boxed_fallback_binop(out, indent, &a, &b, int_op, &dedup_key, ptr_ty)
    }

    /// Try to constant-fold a binary operation where both operands are Decimal.
    fn try_fold_binop_constants(
        &mut self,
        out: &mut String,
        indent: &str,
        l: &Expr,
        r: &Expr,
        int_op: &str,
    ) -> Option<TypedRegister> {
        let (Expr::Decimal(li), Expr::Decimal(ri)) = (l, r) else { return None; };
        let result = match int_op {
            "add" => Some(li.wrapping_add(*ri)),
            "sub" => Some(li.wrapping_sub(*ri)),
            "mul" => Some(li.wrapping_mul(*ri)),
            "sdiv" if *ri != 0 => Some(li / ri),
            "and" => Some(li & ri),
            "or" => Some(li | ri),
            "xor" => Some(li ^ ri),
            "shl" => Some(li.wrapping_shl(*ri as u32)),
            "lshr" => Some((*li as u64).wrapping_shr(*ri as u32) as i64),
            _ => None,
        };
        let folded = result?;
        let v = self.fun.next_reg();
        writeln!(out, "{}{} = add i64 0, {}", indent, v, folded).ok();
        Some(TypedRegister { name: v, ty: Type::int() })
    }

    /// Check if a type is a native float (float/double) via the universe.
    /// 2026-07-17: Reads ALU property instead of primitive().
    fn is_native_float(&self, ty: &Type) -> bool {
        self.ctx.type_universe.as_ref()
            .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
            .map(|r| {
                r.properties.get("alu").and_then(|pv| match pv {
                    crate::ast::PropertyValue::Identifier(s) => Some(s.as_str() == "Float"),
                    _ => None,
                }).unwrap_or(false)
            })
            .unwrap_or_else(|| {
                type_is(&self.ctx.type_universe, ty, "Float")
                    || type_is(&self.ctx.type_universe, ty, "Float64")
            })
    }

    /// Choose the dedup opcode based on float vs int.
    fn dedup_op<'a>(a_is_native: bool, b_is_native: bool, int_op: &'a str, float_op: &'a str) -> &'a str {
        if a_is_native || b_is_native { float_op } else { int_op }
    }

    /// Build the dedup cache key if the opcode is long enough.
    fn build_dedup_key(&self, op: &str, a: &TypedRegister, b: &TypedRegister) -> Option<(String, String, String)> {
        if op.len() >= 3 {
            Some((op.to_string(), a.name.clone(), b.name.clone()))
        } else {
            None
        }
    }

    /// Check the expression dedup cache for a previously emitted result.
    fn check_dedup_cache(&self, key: &Option<(String, String, String)>) -> Option<String> {
        let key = key.as_ref()?;
        self.fun.expr_dedup_cache.get(key).cloned()
    }

    /// Infer pointer type preservation through arithmetic.
    fn infer_ptr_type(&self, a_ty: &Type, b_ty: &Type) -> Option<Type> {
        if Self::is_ptr_ty(a_ty) {
            Some(a_ty.clone())
        } else if Self::is_ptr_ty(b_ty) {
            Some(b_ty.clone())
        } else {
            None
        }
    }

    /// Emit a native float binary operation (fadd/fsub/fmul/fdiv).
    fn emit_native_float_binop(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
        float_op: &str,
        dedup_key: &Option<(String, String, String)>,
    ) -> TypedRegister {
        let fa = self.ensure_float_reg(out, indent, a);
        let fb = self.ensure_float_reg(out, indent, b);
        let llvm_ty = self.operator_llvm_type(&a.ty);
        let fr = self.fun.next_reg_with_prefix("bfr");
        writeln!(out, "{}{} = {} fast {} {}, {}", indent, fr, float_op, llvm_ty, fa, fb).ok();
        self.fun.reg_float_cache.insert(fr.clone(), fr.clone());
        if let Some(key) = dedup_key {
            self.fun.expr_dedup_cache.insert(key.clone(), fr.clone());
        }
        TypedRegister { name: fr, ty: a.ty.clone() }
    }

    /// Emit a mixed float/int binary operation (box both to i64).
    fn emit_mixed_binop(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
        int_op: &str,
        dedup_key: &Option<(String, String, String)>,
        ptr_ty: Option<Type>,
    ) -> TypedRegister {
        let v = self.fun.next_reg_with_prefix("t");
        let a_i64 = self.adapt_to_i64(out, indent, a);
        let b_i64 = self.adapt_to_i64(out, indent, b);
        writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a_i64, b_i64).ok();
        if let Some(key) = dedup_key {
            self.fun.expr_dedup_cache.insert(key.clone(), v.clone());
        }
        TypedRegister { name: v, ty: ptr_ty.unwrap_or(Type::int()) }
    }

    /// Emit a fixed-width integer binary operation (native width, no boxing).
    fn emit_fixed_width_binop(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
        int_op: &str,
        dedup_key: &Option<(String, String, String)>,
    ) -> TypedRegister {
        let v = self.fun.next_reg_with_prefix("t");
        let llvm_ty_str = self.llvm_type(&a.ty).to_string();
        writeln!(out, "{}{} = {} {} {}, {}", indent, v, int_op, llvm_ty_str, a.name, b.name).ok();
        if let Some(key) = dedup_key {
            self.fun.expr_dedup_cache.insert(key.clone(), v.clone());
        }
        TypedRegister { name: v, ty: a.ty.clone() }
    }

    /// Emit a generic i64 boxed binary operation fallback.
    fn emit_boxed_fallback_binop(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
        int_op: &str,
        dedup_key: &Option<(String, String, String)>,
        ptr_ty: Option<Type>,
    ) -> TypedRegister {
        let v = self.fun.next_reg_with_prefix("t");
        let a_i64 = self.adapt_to_i64(out, indent, a);
        let b_i64 = self.adapt_to_i64(out, indent, b);
        writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a_i64, b_i64).ok();
        if let Some(key) = dedup_key {
            self.fun.expr_dedup_cache.insert(key.clone(), v.clone());
        }
        TypedRegister { name: v, ty: ptr_ty.unwrap_or(Type::int()) }
    }

    /// Emit a resolved operator call.
    /// 2026-07-08: Phase 2D — handles Native storage (float/double) by
    /// calling ensure_float_reg on operands before emitting the opcode.
    ///
    /// The `implementation` expression is one of:
    ///   - `Identifier(name)` → call to function `name`
    ///   - `Quoted(llvm_op)` → inline LLVM instruction (e.g. "add nsw")
    ///   - Fallback → identity (no-op)
    fn emit_operator_call(
        &mut self,
        out: &mut String,
        indent: &str,
        a: &TypedRegister,
        b: &TypedRegister,
        implementation: &Expr,
    ) -> TypedRegister {
        let v = self.fun.next_reg();
        // 2026-07-17: Read ALU property instead of primitive()
        let is_native = self.ctx.type_universe.as_ref()
            .and_then(|u| a.ty.universe_key().and_then(|k| u.get(k)))
            .map(|r| {
                r.properties.get("alu").and_then(|pv| match pv {
                    crate::ast::PropertyValue::Identifier(s) => Some(s.as_str() == "Float"),
                    _ => None,
                }).unwrap_or(false)
            })
            .unwrap_or(false);
        let (op_a, op_b) = if is_native {
            (self.ensure_float_reg(out, indent, a), self.ensure_float_reg(out, indent, b))
        } else {
            (a.name.clone(), b.name.clone())
        };
        let llvm_ty = self.operator_llvm_type(&a.ty);
        match implementation {
            Expr::Identifier(name) => {
                writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})",
                    indent, v, name, op_a, op_b).ok();
            }
            Expr::Quoted(llvm_op) => {
                let llvm_op_str = String::from_utf8_lossy(llvm_op);
                writeln!(out, "{}{} = {} {} {}, {}",
                    indent, v, llvm_op_str, llvm_ty, op_a, op_b).ok();
                if is_native {
                    self.fun.reg_float_cache.insert(v.clone(), v.clone());
                }
            }
            _ => {
                writeln!(out, "{}{} = {} 0, {}", indent, v, llvm_ty, op_a).ok();
            }
        }
        TypedRegister { name: v, ty: a.ty.clone() }
    }

    /// Emit LLVM IR for a comparison between two expressions.
    /// Handles constant folding, string trigger comparisons (compare first
    /// byte), and general float/int dispatch.
    pub(crate) fn emit_fcmp(
        &mut self,
        out: &mut String,
        indent: &str,
        l: &Expr,
        r: &Expr,
        cond: &str,
    ) -> TypedRegister {
        if let Some(folded) = self.try_fold_fcmp_constants(out, indent, l, r, cond) {
            return folded;
        }
        if let Some(result) = self.try_string_trigger_cmp(out, indent, l, r, cond) {
            return result;
        }
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        let c = self.fun.next_reg_with_prefix("c");
        if type_is(&self.ctx.type_universe, &a.ty, "Float") || type_is(&self.ctx.type_universe, &b.ty, "Float") {
            let fa = self.ensure_float_reg(out, indent, &a);
            let fb = self.ensure_float_reg(out, indent, &b);
            writeln!(out, "{}{} = fcmp fast {} float {}, {}", indent, c, cond, fa, fb).ok();
        } else {
            let icmp_cond = self.fcmp_to_icmp(cond);
            let a_i64 = self.adapt_to_i64(out, indent, &a);
            let b_i64 = self.adapt_to_i64(out, indent, &b);
            writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, a_i64, b_i64).ok();
        }
        TypedRegister { name: c, ty: Type::bool_() }
    }

    /// Fold constant integer comparisons at compile time.
    fn try_fold_fcmp_constants(
        &mut self,
        out: &mut String,
        indent: &str,
        l: &Expr,
        r: &Expr,
        cond: &str,
    ) -> Option<TypedRegister> {
        let (Expr::Decimal(li), Expr::Decimal(ri)) = (l, r) else { return None; };
        let result = match cond {
            "oeq" => li == ri,
            "one" => li != ri,
            "olt" => li < ri,
            "ole" => li <= ri,
            "ogt" => li > ri,
            "oge" => li >= ri,
            _ => false,
        };
        let v = self.fun.next_reg_with_prefix("t");
        if result {
            writeln!(out, "{}{} = and i8 1, 1", indent, v).ok();
        } else {
            writeln!(out, "{}{} = xor i8 1, 1", indent, v).ok();
        }
        Some(TypedRegister { name: v, ty: Type::bool_() })
    }

    /// Try to compare a string trigger's first byte against a quoted literal.
    fn try_string_trigger_cmp(
        &mut self,
        out: &mut String,
        indent: &str,
        l: &Expr,
        r: &Expr,
        cond: &str,
    ) -> Option<TypedRegister> {
        let (trigger_expr, quoted) = if let Expr::Quoted(s) = r {
            if self.is_linked_string_trigger(l) { (l, s) } else { return None; }
        } else if let Expr::Quoted(s) = l {
            if self.is_linked_string_trigger(r) { (r, s) } else { return None; }
        } else {
            return None;
        };
        let a = self.emit_expr(out, trigger_expr, indent);
        let icmp_cond = self.fcmp_to_icmp(cond);
        let p = self.fun.next_reg_with_prefix("fp");
        self.emit_inttoptr(out, indent, &p, &a.name);
        let b = self.fun.next_reg_with_prefix("fb");
        writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, b, p).ok();
        let z = self.fun.next_reg_with_prefix("fz");
        writeln!(out, "{}{} = zext i8 {} to i64", indent, z, b).ok();
        let byte_val = quoted.first().copied().unwrap_or(0u8) as i64;
        let c = self.fun.next_reg_with_prefix("fc");
        writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, z, byte_val).ok();
        Some(TypedRegister { name: c, ty: Type::bool_() })
    }

    /// Convert LLVM float comparison condition to integer icmp condition.
    fn fcmp_to_icmp(&self, cond: &str) -> &'static str {
        match cond {
            "oeq" => "eq",
            "one" => "ne",
            "olt" => "slt",
            "ole" => "sle",
            "ogt" => "sgt",
            "oge" => "sge",
            _ => "eq",
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 6: Projection Fast Path
    // ═══════════════════════════════════════════════════════════════

    /// Emit native LLVM IR for function metadata projections (Address, Name).
    /// Returns `Some(register)` if the target is a function name identifier.
    pub(super) fn try_emit_fn_projection(
        &mut self,
        out: &mut String,
        source: &Expr,
        _target: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        let name = match source {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        let v = self.fun.next_reg_with_prefix("fnm");
        writeln!(out, "{}{} = or i64 {}, 0", indent, v, 0).ok();
        Some(TypedRegister { name: v, ty: Type::int() })
    }

    /// Emit native LLVM IR for well-known projection operations
    /// (Add/Sub/Mul/Div/Eq/Ne/Lt/Le/Gt/Ge on Int/Float/Bool).
    ///
    /// Why this exists: Brief's projection system is generic (any operator
    /// on any type dispatches through UserDefinedWithArg). But for primitive
    /// types, the generic dispatch would load i64 → convert to native →
    /// exec op → convert back. This fast path skips both conversions.
    pub(super) fn try_projection_fast_path(
        &mut self,
        out: &mut String,
        src_val: &TypedRegister,
        name: &str,
        arg_expr: &Expr,
        indent: &str,
        v: &str,
    ) -> Option<TypedRegister> {
        let rhs = self.emit_expr(out, arg_expr, indent);
        let type_name = match &src_val.ty {
            Type::Custom(n) => n.as_str(),
            _ => return None,
        };
        match type_name {
            "Int" => self.projection_int_fast_path(out, src_val, &rhs, name, v, indent),
            "Float" => self.projection_float_fast_path(out, src_val, &rhs, name, v, indent),
            "Bool" => self.projection_bool_fast_path(out, src_val, &rhs, name, v, indent),
            _ => None,
        }
    }

    /// Fast-path projections for Int type.
    fn projection_int_fast_path(
        &mut self,
        out: &mut String,
        src: &TypedRegister,
        rhs: &TypedRegister,
        name: &str,
        v: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        let (op, is_cmp) = match name {
            "Add" => ("add", false),
            "Sub" => ("sub", false),
            "Mul" => ("mul", false),
            "Div" => ("sdiv", false),
            "Mod" => ("srem", false),
            "Eq" => ("icmp eq", true),
            "Ne" => ("icmp ne", true),
            "Lt" => ("icmp slt", true),
            "Le" => ("icmp sle", true),
            "Gt" => ("icmp sgt", true),
            "Ge" => ("icmp sge", true),
            "BitAnd" | "And" => ("and", false),
            "BitOr" | "Or" => ("or", false),
            "BitXor" => ("xor", false),
            "Shl" => ("shl", false),
            "Shr" => ("lshr", false),
            _ => return None,
        };
        if is_cmp {
            let cmp = self.fun.next_reg_with_prefix("pcmp");
            writeln!(out, "{}{} = {} i64 {}, {}", indent, cmp, op, src.name, rhs.name).ok();
            writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            Some(TypedRegister { name: v.to_string(), ty: Type::int() })
        } else {
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, op, src.name, rhs.name).ok();
            Some(TypedRegister { name: v.to_string(), ty: Type::int() })
        }
    }

    /// Fast-path projections for Float type.
    fn projection_float_fast_path(
        &mut self,
        out: &mut String,
        src: &TypedRegister,
        rhs: &TypedRegister,
        name: &str,
        v: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        let (op, is_cmp) = match name {
            "Add" => ("fadd", false),
            "Sub" => ("fsub", false),
            "Mul" => ("fmul", false),
            "Div" => ("fdiv", false),
            "Eq" => ("fcmp oeq", true),
            "Ne" => ("fcmp one", true),
            "Lt" => ("fcmp olt", true),
            "Le" => ("fcmp ole", true),
            "Gt" => ("fcmp ogt", true),
            "Ge" => ("fcmp oge", true),
            _ => return None,
        };
        if is_cmp {
            let cmp = self.fun.next_reg_with_prefix("pcmp");
            writeln!(out, "{}{} = {} float {}, {}", indent, cmp, op, src.name, rhs.name).ok();
            let ext = self.fun.next_reg_with_prefix("pce");
            writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
            writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
            Some(TypedRegister { name: v.to_string(), ty: Type::float() })
        } else {
            writeln!(out, "{}{} = {} float {}, {}", indent, v, op, src.name, rhs.name).ok();
            Some(TypedRegister { name: v.to_string(), ty: Type::float() })
        }
    }

    /// Fast-path projections for Bool type.
    fn projection_bool_fast_path(
        &mut self,
        out: &mut String,
        src: &TypedRegister,
        rhs: &TypedRegister,
        name: &str,
        v: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        match name {
            "And" => {
                writeln!(out, "{}{} = and i1 {}, {}", indent, v, src.name, rhs.name).ok();
                Some(TypedRegister { name: v.to_string(), ty: Type::bool_() })
            }
            "Or" => {
                writeln!(out, "{}{} = or i1 {}, {}", indent, v, src.name, rhs.name).ok();
                Some(TypedRegister { name: v.to_string(), ty: Type::bool_() })
            }
            "Eq" => {
                writeln!(out, "{}{} = icmp eq i1 {}, {}", indent, v, src.name, rhs.name).ok();
                Some(TypedRegister { name: v.to_string(), ty: Type::bool_() })
            }
            "Ne" => {
                writeln!(out, "{}{} = icmp ne i1 {}, {}", indent, v, src.name, rhs.name).ok();
                Some(TypedRegister { name: v.to_string(), ty: Type::bool_() })
            }
            _ => None,
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 7: Melds & Type Decay
    // ═══════════════════════════════════════════════════════════════

    /// Check if the source type has a meld route for the given projection
    /// target. When a meld route exists, evaluates the route's destination
    /// expression to derive the projection result from the backing value.
    ///
    /// 2026-07-13: Uses `TypeUniverse::find_meld` (bidirectional lookup)
    /// instead of iterating `melds` directly.
    pub(crate) fn try_meld_projection(
        &mut self,
        out: &mut String,
        src_val: &TypedRegister,
        target_name: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        let custom_name = match &src_val.ty {
            Type::Custom(n) => n.clone(),
            _ => return None,
        };
        let (partner, route) = {
            let universe = self.ctx.type_universe.as_ref()?;
            let meld_entry = universe.melds.iter().find(|((a, b), _decl)| {
                a.as_str() == custom_name || b.as_str() == custom_name
            });
            let ((name_a, name_b), meld_decl) = meld_entry?;
            let partner: String = if name_a.as_str() == custom_name { name_b.clone() } else { name_a.clone() };
            let route = meld_decl.routes.iter().find(|r| r.accessor == target_name)?;
            (partner, route.clone())
        };
        let result = self.emit_route_expression(out, &route.dest_expr, src_val, &partner, indent);
        if let Some(ref reg) = result {
            self.mark_chimera(&reg.name, &partner);
        }
        result
    }

    /// Evaluate a meld route's destination expression, substituting the
    /// meld partner's type name with the actual source value.
    ///
    /// Patterns handled:
    ///   1. `Identifier("Ptr"|"Size"|"Bytes"|"Alignment"|"Type")` → direct projection
    ///   2. `Call("strlen#", [arg])` → intrinsic with projection arg
    ///   3. `Field(obj, field)` where `obj == partner` → substituted field projection
    fn emit_route_expression(
        &mut self,
        out: &mut String,
        expr: &Expr,
        src_val: &TypedRegister,
        partner: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        match expr {
            // Pattern 1: identity projection — "Ptr" or "Size" on the backing value
            Expr::Identifier(name) if matches!(name.as_str(), "Ptr" | "Size" | "Bytes" | "Alignment" | "Type") => {
                self.emit_direct_projection(out, src_val, name, indent)
            }
            // Pattern 2: intrinsic call — strlen#(Ptr) etc.
            Expr::Call(name, args, _) if name == "strlen#" && args.len() == 1 => {
                self.emit_strlen_meld_route(out, indent, &args[0], src_val)
            }
            // Pattern 3: field access on the partner type — "CString.ptr"
            Expr::Field(obj, field) => {
                let Expr::Identifier(n) = obj.as_ref() else { return None; };
                if n != partner {
                    return None;
                }
                self.emit_direct_projection(out, src_val, field, indent)
            }
            _ => None,
        }
    }

    /// Emit a strlen#(arg) meld route where arg is a projection on the source.
    fn emit_strlen_meld_route(
        &mut self,
        out: &mut String,
        indent: &str,
        arg: &Expr,
        src_val: &TypedRegister,
    ) -> Option<TypedRegister> {
        let arg_name = match arg {
            Expr::Identifier(n) => n.as_str(),
            _ => return None,
        };
        match arg_name {
            "Ptr" | "Size" | "Bytes" => {
                let v = self.fun.next_reg_with_prefix("t");
                let proj_reg = self.emit_direct_projection(out, src_val, arg_name, indent)?;
                writeln!(out, "{}{} = call i64 @__strlen__(i64 {})", indent, v, proj_reg.name).ok();
                Some(TypedRegister { name: v, ty: Type::int() })
            }
            _ => None,
        }
    }

    /// Decay a chimera value to its canonical type at a boundary.
    /// When `target_ty` is `None`, assumes decay to the backing type (identity).
    /// Generic materialization: looks up the meld between backing and target,
    /// derives each field of the target type from the backing via route expressions.
    pub(crate) fn emit_decay(
        &mut self,
        out: &mut String,
        val: &TypedRegister,
        target_ty: Option<&Type>,
        indent: &str,
    ) -> TypedRegister {
        if !self.is_chimera(&val.name) {
            return val.clone();
        }
        let backing: String = match self.chimera_backing(&val.name) {
            Some(b) => b.to_string(),
            None => return val.clone(),
        };
        let target_name = match target_ty {
            Some(Type::Custom(n)) => n.clone(),
            _ => return val.clone(),
        };
        if backing == target_name {
            return val.clone();
        }
        let (routes, target_fields) = {
            let Some(universe) = self.ctx.type_universe.as_ref() else { return val.clone(); };
            let Some(meld_decl) = universe.find_meld(&backing, &target_name) else { return val.clone(); };
            let Some(target_fields) = self.ctx.struct_types.get(&target_name) else { return val.clone(); };
            (meld_decl.routes.clone(), target_fields.clone())
        };
        let field_results = self.derive_fields_via_meld(out, val, &backing, &routes, &target_fields, indent);

        if field_results.is_empty() {
            return val.clone();
        }
        if field_results.len() == 1 {
            let (_, ref ty, ref reg) = field_results[0];
            return TypedRegister { name: reg.clone(), ty: ty.clone() };
        }
        self.emit_multi_field_struct(out, indent, &field_results, &target_name)
    }

    /// Derive each field of a target type from a backing value via meld routes.
    /// Falls back to 0 for fields without a route or when route evaluation fails.
    fn derive_fields_via_meld(
        &mut self,
        out: &mut String,
        val: &TypedRegister,
        backing: &str,
        routes: &[crate::ast::MeldRouteDef],
        target_fields: &[(String, Type)],
        indent: &str,
    ) -> Vec<(String, Type, String)> {
        let mut results = Vec::new();
        for (field_name, field_ty) in target_fields {
            let reg = if let Some(route) = routes.iter().find(|r| r.accessor == *field_name) {
                if let Some(reg) = self.emit_route_expression(out, &route.dest_expr, val, backing, indent) {
                    reg.name
                } else {
                    self.emit_zero_placeholder(out, indent)
                }
            } else {
                self.emit_zero_placeholder(out, indent)
            };
            results.push((field_name.clone(), field_ty.clone(), reg));
        }
        results
    }

    /// Emit a zero placeholder register (add i64 0, 0).
    fn emit_zero_placeholder(&mut self, out: &mut String, indent: &str) -> String {
        let v = self.fun.next_reg_with_prefix("t");
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
        v
    }

    /// Allocate a heap struct and store each derived field into it.
    fn emit_multi_field_struct(
        &mut self,
        out: &mut String,
        indent: &str,
        field_results: &[(String, Type, String)],
        target_name: &str,
    ) -> TypedRegister {
        let total_size = field_results.len() * 8;
        let alloc = self.fun.next_reg_with_prefix("t");
        writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, alloc, total_size).ok();
        let struct_ptr = self.fun.next_reg_with_prefix("t");
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, struct_ptr, alloc).ok();
        for (i, (_name, _ty, reg)) in field_results.iter().enumerate() {
            let gep = self.fun.next_reg_with_prefix("t");
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, struct_ptr, i).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !1", indent, reg, gep).ok();
        }
        TypedRegister { name: struct_ptr, ty: Type::Custom(target_name.to_string()) }
    }

    /// Emit a direct projection on a value without going through the meld
    /// route check. This avoids infinite recursion when a meld route maps
    /// to the same projection target.
    fn emit_direct_projection(
        &mut self,
        out: &mut String,
        src_val: &TypedRegister,
        target_name: &str,
        indent: &str,
    ) -> Option<TypedRegister> {
        let v = self.fun.next_reg_with_prefix("t");
        match target_name {
            "Ptr" => {
                writeln!(out, "{}{} = add i64 0, {} ; ptr", indent, v, src_val.name).ok();
                Some(TypedRegister { name: v, ty: Type::int() })
            }
            "Size" => {
                let hp = self.fun.next_reg_with_prefix("drphp");
                self.emit_inttoptr(out, indent, &hp, &src_val.name);
                let lp = self.fun.next_reg_with_prefix("drplp");
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, v, lp).ok();
                Some(TypedRegister { name: v, ty: Type::int() })
            }
            "Bytes" => {
                writeln!(out, "{}{} = add i64 0, 8 ; bytes", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::int() })
            }
            "Alignment" => {
                writeln!(out, "{}{} = add i64 0, 8 ; alignment", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::int() })
            }
            "Type" => {
                writeln!(out, "{}{} = add i64 0, 6 ; type=custom", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::int() })
            }
            _ => None,
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 8: Cast Optimization (EOR)
    // ═══════════════════════════════════════════════════════════════

    /// Try to emit an EOR-optimized cast:
    /// `Cast(BinaryOp(Cast(a, T), Cast(b, T)), U)` where U -> T.
    /// If matched, emits the binary op directly without redundant casts.
    ///
    /// 2026-07-03: Cast elimination optimization — when both operands of a
    /// binary op are already the target type (cast_ty), and the outer cast
    /// target has a meld with cast_ty, emit the binary op directly in the
    /// inner types. This eliminates the inner casts entirely.
    pub(super) fn try_emit_eor(
        &mut self,
        out: &mut String,
        v: &str,
        inner: &Expr,
        target_ty: &Type,
        indent: &str,
    ) -> Option<TypedRegister> {
        let (kind, lhs, rhs) = match inner {
            Expr::BinaryOp(k, l, r) => (k, l.as_ref().clone(), r.as_ref().clone()),
            _ => return None,
        };
        let (Expr::Cast(_, lt), Expr::Cast(_, rt)) = (&lhs, &rhs) else { return None; };
        if lt != rt {
            return None;
        }
        let cast_ty = lt.clone();
        let target_name = target_ty.universe_key()?;
        let tu = self.ctx.type_universe.as_ref()?;
        if tu.find_meld(target_name, cast_ty.universe_key()?).is_none() {
            return None;
        }
        let a = self.emit_expr(out, &lhs, indent);
        let b = self.emit_expr(out, &rhs, indent);
        if type_is(&self.ctx.type_universe, &cast_ty, "Float")
            || type_is(&self.ctx.type_universe, &cast_ty, "Float64")
        {
            self.emit_eor_float_path(out, indent, v, kind, &a, &b, &cast_ty)
        } else {
            self.emit_eor_int_path(out, indent, v, kind, &a, &b, &cast_ty)
        }
    }

    /// Emit EOR float path: emit float binary op, bitcast/zext result to i64.
    fn emit_eor_float_path(
        &mut self,
        out: &mut String,
        indent: &str,
        v: &str,
        kind: &crate::ast::BinaryOpKind,
        a: &TypedRegister,
        b: &TypedRegister,
        cast_ty: &Type,
    ) -> Option<TypedRegister> {
        let fl_a = self.ensure_float_reg(out, indent, a);
        let fl_b = self.ensure_float_reg(out, indent, b);
        let fl_op = match kind {
            crate::ast::BinaryOpKind::Add => "fadd",
            crate::ast::BinaryOpKind::Sub => "fsub",
            crate::ast::BinaryOpKind::Mul => "fmul",
            crate::ast::BinaryOpKind::Div => "fdiv",
            _ => return None,
        };
        writeln!(out, "{}{} = {} float {}, {}", indent, v, fl_op, fl_a, fl_b).ok();
        let bi = self.fun.next_reg_with_prefix("eor_bi");
        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, v).ok();
        let ze = self.fun.next_reg_with_prefix("eor_ze");
        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
        self.fun.reg_float_cache.insert(ze.clone(), v.to_string());
        let ret_ty = if cast_ty == &Type::float64() { Type::float64() } else { Type::float() };
        Some(TypedRegister { name: ze, ty: ret_ty })
    }

    /// Emit EOR integer path: emit integer binary op directly.
    fn emit_eor_int_path(
        &mut self,
        out: &mut String,
        indent: &str,
        v: &str,
        kind: &crate::ast::BinaryOpKind,
        a: &TypedRegister,
        b: &TypedRegister,
        cast_ty: &Type,
    ) -> Option<TypedRegister> {
        let i_op = match kind {
            crate::ast::BinaryOpKind::Add => "add",
            crate::ast::BinaryOpKind::Sub => "sub",
            crate::ast::BinaryOpKind::Mul => "mul",
            crate::ast::BinaryOpKind::Div => "sdiv",
            _ => return None,
        };
        writeln!(out, "{}{} = {} i64 {}, {}", indent, v, i_op, a.name, b.name).ok();
        Some(TypedRegister { name: v.to_string(), ty: cast_ty.clone() })
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 9: Utility Methods (added 2026-07-14 for AST compat)
    // ═══════════════════════════════════════════════════════════════

    /// Box a typed register to i64 for uniform state storage.
    /// Handles Float64(double)→bitcast→i64, Float(float)→bitcast→i32→zext→i64,
    /// Bool(i8)→zext→i64, String/Data(i8*)→ptrtoint→i64. Int is already i64 (identity).
    pub(crate) fn adapt_to_i64(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        match &reg.ty {
            Type::Custom(t) if t == "Float64" => {
                let tr = self.fun.gen_reg();
                writeln!(out, "{}{} = bitcast double {} to i64", indent, tr, reg.name).ok();
                tr
            }
            // 2026-07-17: Float (32-bit) must go float→i32→i64, not direct bitcast.
            Type::Custom(t) if t == "Float" => {
                let tr = self.fun.gen_reg();
                writeln!(out, "{}{} = bitcast float {} to i32", indent, tr, reg.name).ok();
                let ze = self.fun.gen_reg();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, tr).ok();
                ze
            }
            Type::Custom(t) if t == "Bool" => {
                let tr = self.fun.gen_reg();
                writeln!(out, "{}{} = zext i8 {} to i64", indent, tr, reg.name).ok();
                tr
            }
            Type::Custom(t) if (t == "String" || t == "Data")
                && self.feature_sso_strings
                && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(&reg.ty))
            => {
                // 2026-07-18: SSO String is {i64, i64} — extract handle[0] (data/tag).
                let tr = self.fun.gen_reg();
                writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, tr, reg.name).ok();
                tr
            }
            Type::Custom(t) if t == "String" || t == "Data" => {
                let tr = self.fun.gen_reg();
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, tr, reg.name).ok();
                tr
            }
            // 2026-07-17: Ptr types are already represented as i64 (via
            // ptrtoint in emit_malloc). Store the i64 bits directly.
            Type::Ptr(_) => reg.name.clone(),
            _ => reg.name.clone(),
        }
    }

    /// Emit a getelementptr for a state field and return the GEP register name.
    /// `prefix` is used to make register names unique within a function.
    pub(crate) fn emit_state_gep(&mut self, out: &mut String, indent: &str, _prefix: &str, state_ptr: &str, idx: usize) -> String {
        let r = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}",
            indent, r, state_ptr, idx).ok();
        r
    }

    // 2026-07-19: DRY consolidation helpers — centralized state field access.
    // All 44 hand-rolled GEP+load/store sites should migrate to these.

    /// Load a state field as i64. Returns (register_name, brief_type).
    /// The brief type can be passed to ensure_typed_value for float unboxing.
    pub(crate) fn emit_state_load_i64(&mut self, out: &mut String, indent: &str, name: &str) -> Option<(String, Type)> {
        let idx = *self.ctx.field_index_map.get(name)?;
        let brief_ty = self.ctx.field_brief_types.get(idx).cloned().unwrap_or(Type::int());
        let gep = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, gep, idx).ok();
        let val = self.fun.gen_reg();
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, val, gep).ok();
        Some((val, brief_ty))
    }

    /// Store an i64 value to a state field. The value should already be boxed
    /// via adapt_to_i64 if its brief type is float/double/bool.
    pub(crate) fn emit_state_store_i64(&mut self, out: &mut String, indent: &str, name: &str, val: &str) -> Option<()> {
        let idx = *self.ctx.field_index_map.get(name)?;
        let gep = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, gep, idx).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, val, gep).ok();
        Some(())
    }

    /// Load i64 from a state field by index. Returns (register_name, brief_type).
    pub(crate) fn emit_state_load_i64_by_idx(&mut self, out: &mut String, indent: &str, idx: usize) -> (String, Type) {
        let brief_ty = self.ctx.field_brief_types.get(idx).cloned().unwrap_or(Type::int());
        let gep = self.fun.next_reg_with_prefix("slg");
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, gep, idx).ok();
        let val = self.fun.next_reg_with_prefix("slv");
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, val, gep).ok();
        (val, brief_ty)
    }

    /// Store i64 to a state field by index.
    pub(crate) fn emit_state_store_i64_by_idx(&mut self, out: &mut String, indent: &str, idx: usize, val: &str) {
        let gep = self.fun.next_reg_with_prefix("ssg");
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, gep, idx).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, val, gep).ok();
    }

    /// Ensure the value register has the expected LLVM type, inserting
    /// trunc/zext/bitcast as needed. Returns the possibly-converted register name.
    pub(crate) fn ensure_typed_value(&mut self, out: &mut String, indent: &str, expected_llvm_ty: &str, val: &str, brief_ty: Option<Type>, _universe: Option<&crate::type_universe::TypeUniverse>) -> String {
        let Some(ref bt) = brief_ty else { return val.to_string(); };
        let actual_ty = self.llvm_type(bt);
        if actual_ty == expected_llvm_ty {
            return val.to_string();
        }
        let actual_ty_clone = actual_ty.to_string();
        match (actual_ty.as_ref(), expected_llvm_ty) {
            ("double", "i64") | ("float", "i64") => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = bitcast {} {} to i64", indent, r, actual_ty_clone, val).ok();
                r
            }
            ("i64", "double") => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = bitcast i64 {} to double", indent, r, val).ok();
                r
            }
            ("i64", "float") => {
                let tr = self.fun.gen_reg();
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val).ok();
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, r, tr).ok();
                r
            }
            ("i8", "i64") => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = zext i8 {} to i64", indent, r, val).ok();
                r
            }
            ("i32", "i64") => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, r, val).ok();
                r
            }
            _ => val.to_string(),
        }
    }
}
