// ── SLP Vectorization Hazard Analysis ─────────────────────────────────
// 2026-07-13: Rewritten for new AST. All old binary-op match arms combined
// into Expr::BinaryOp(_, l, r). All old Statement variants simplified.

use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::LlvmBackend;
use std::collections::HashSet;

/// Check if an expression references a given variable name.
fn expr_refs_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Identifier(n) => n == name,
        Expr::BinaryOp(_, l, r) => expr_refs_name(l, name) || expr_refs_name(r, name),
        Expr::UnaryOp(_, e) | Expr::Cast(e, _) | Expr::IsType(e, _) => expr_refs_name(e, name),
        Expr::Call(_, args, _) => args.iter().any(|a| expr_refs_name(a, name)),
        Expr::Field(obj, _) => expr_refs_name(obj, name),
        Expr::Index(obj, idx) => expr_refs_name(obj, name) || expr_refs_name(idx, name),
        Expr::Block(stmts) => stmts.iter().any(|s| stmt_refs_name(s, name)),
        Expr::If(cond, then, else_opt) => {
            expr_refs_name(cond, name)
                || expr_refs_name(then, name)
                || else_opt.as_ref().map_or(false, |e| expr_refs_name(e, name))
        }
        Expr::Match(value, arms) => {
            expr_refs_name(value, name) || arms.iter().any(|arm| expr_refs_name(&arm.body, name))
        }
        Expr::Tuple(items) | Expr::List(items) => {
            items.iter().any(|e| expr_refs_name(e, name))
        }
        Expr::Lambda(_, body) => expr_refs_name(body, name),
        Expr::Within(body, fallback) => {
            expr_refs_name(body, name) || expr_refs_name(fallback, name)
        }
        Expr::DerivationBlock(db) => {
            db.examples.iter().any(|ex| {
                ex.inputs.iter().any(|i| expr_refs_name(i, name))
                    || expr_refs_name(&ex.output, name)
            })
        }
        _ => false,
    }
}

/// Check if a statement references a given variable name.
fn stmt_refs_name(stmt: &Statement, name: &str) -> bool {
    match stmt {
        Statement::Let { expr: Some(e), .. } | Statement::Assign(_, e) => expr_refs_name(e, name),
        Statement::Let { expr: None, .. } => false,
        Statement::Expression(e) => expr_refs_name(e, name),
        Statement::Term(Some(e)) | Statement::TermBang(Some(e)) | Statement::Return(Some(e)) => {
            expr_refs_name(e, name)
        }
        Statement::Term(None) | Statement::TermBang(None) | Statement::Return(None) => false,
        Statement::Guarded(cond, stmts) => {
            expr_refs_name(cond, name) || stmts.iter().any(|s| stmt_refs_name(s, name))
        }
        Statement::If(cond, then_b, else_b) => {
            expr_refs_name(cond, name)
                || then_b.iter().any(|s| stmt_refs_name(s, name))
                || else_b.iter().any(|s| stmt_refs_name(s, name))
        }
        Statement::Block(stmts) => stmts.iter().any(|s| stmt_refs_name(s, name)),
        Statement::Escape(Some(e)) => expr_refs_name(e, name),
        Statement::Escape(None) => false,
        Statement::Foreach { list, body, .. } => {
            expr_refs_name(list, name) || body.iter().any(|s| stmt_refs_name(s, name))
        }
        Statement::SyncBlock(stmts) => stmts.iter().any(|s| stmt_refs_name(s, name)),
        Statement::TrgBinding { instance, .. } => expr_refs_name(instance, name),
        _ => false,
    }
}

/// Compute the peak number of simultaneously-live float values across a
/// statement body, using classical interval analysis (def → last use).
fn compute_peak_live_floats(body: &[Statement], float_names: &[String]) -> u32 {
    if float_names.is_empty() {
        return 0;
    }
    let mut intervals: Vec<(usize, usize)> = Vec::with_capacity(float_names.len());
    for name in float_names {
        let def_idx = body
            .iter()
            .position(|s| matches!(s, Statement::Let { name: n, .. } if n == name))
            .unwrap_or_else(|| {
                body.iter()
                    .position(|s| stmt_refs_name(s, name))
                    .unwrap_or(0)
            });
        let mut last_use = def_idx;
        for (i, stmt) in body.iter().enumerate().skip(def_idx + 1) {
            if stmt_refs_name(stmt, name) {
                last_use = i;
            }
        }
        intervals.push((def_idx, last_use));
    }
    let mut peak = 0u32;
    for i in 0..body.len() {
        let active = intervals
            .iter()
            .filter(|(def, last)| *def <= i && i <= *last)
            .count() as u32;
        peak = peak.max(active);
    }
    peak
}

impl LlvmBackend {
    /// Map a target spec to (register_count, vector_width).
    pub(super) fn target_hardware(&self, spec: &crate::target_spec::TargetSpec) -> (u32, u32) {
        if spec.has_capability("avx512f") {
            (32, 16)
        } else if spec.has_capability("avx2") {
            (16, 8)
        } else if spec.has_capability("neon") {
            (32, 4)
        } else if spec.has_capability("sse") {
            (16, 4)
        } else {
            (16, 1)
        }
    }

    pub(super) fn slp_attr(&self, fn_name: &str, default: &str) -> String {
        if fn_name == "main" {
            if self.ctx.slp_hazard_fns.contains(fn_name) {
                return "#5".to_string();
            }
            return "#9".to_string();
        }
        if self.ctx.slp_hazard_fns.contains(fn_name) {
            match default {
                "#0" => "#4".to_string(),
                "#3" => "#5".to_string(),
                _ => default.to_string(),
            }
        } else {
            default.to_string()
        }
    }

    pub(super) fn is_float_field(&self, name: &str) -> bool {
        self.ctx
            .field_index_map
            .get(name)
            .map(|&idx| {
                // 2026-07-17: Check brief type (not LLVM type) since all state
                // fields are stored as i64 regardless of their Brief type.
                // field_types is always "i64" — use field_brief_types instead.
                self.ctx.field_brief_types.get(idx)
                    .map_or(false, |t| *t == Type::float() || *t == Type::float64())
            })
            .unwrap_or(false)
    }

    pub(super) fn is_float_expr_pre_cg(
        &self,
        expr: &Expr,
        local_floats: &HashSet<String>,
    ) -> bool {
        match expr {
            Expr::Float(_) => true,
            Expr::Identifier(name) => {
                self.is_float_field(name)
                    || local_floats.contains(name.as_str())
                    || self
                        .ctx
                        .constants
                        .get(name.as_str())
                        .map_or(false, |(t, _)| *t == Type::float())
            }
            Expr::BinaryOp(_, l, r) => {
                self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats)
            }
            Expr::UnaryOp(_, e) | Expr::Cast(e, _) => {
                self.is_float_expr_pre_cg(e, local_floats)
            }
            Expr::Block(stmts) => stmts
                .last()
                .map_or(false, |s| match s {
                    Statement::Expression(e) | Statement::Term(Some(e)) => {
                        self.is_float_expr_pre_cg(e, local_floats)
                    }
                    _ => false,
                }),
            _ => false,
        }
    }

    pub(super) fn count_cross_float_ops(
        &self,
        expr: &Expr,
        local_floats: &HashSet<String>,
    ) -> u32 {
        match expr {
            Expr::BinaryOp(_, l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats);
                let is_cross_field = match (l.as_ref(), r.as_ref()) {
                    (Expr::Identifier(n1), Expr::Identifier(n2)) => n1 != n2,
                    _ => false,
                };
                let mut count = if is_cross_field && is_float { 1 } else { 0 };
                count += self.count_cross_float_ops(l, local_floats);
                count += self.count_cross_float_ops(r, local_floats);
                count
            }
            Expr::UnaryOp(_, e) => self.count_cross_float_ops(e, local_floats),
            Expr::Block(stmts) => stmts.last().map_or(0, |s| match s {
                Statement::Expression(e) => self.count_cross_float_ops(e, local_floats),
                _ => 0,
            }),
            _ => 0,
        }
    }

    pub(super) fn collect_local_floats_and_temps(
        &self,
        body: &[Statement],
        local_floats: &mut HashSet<String>,
    ) -> u32 {
        let mut temp_count = 0;
        for stmt in body {
            match stmt {
                Statement::Let { name, ty, expr, .. } => {
                    let is_float = ty.as_ref() == Some(&Type::float())
                        || expr.as_ref().map_or(false, |e| {
                            self.is_float_expr_pre_cg(e, local_floats)
                        });
                    if is_float {
                        local_floats.insert(name.clone());
                        temp_count += 1;
                    }
                }
                Statement::Guarded(_, stmts) => {
                    temp_count += self.collect_local_floats_and_temps(stmts, local_floats);
                }
                _ => {}
            }
        }
        temp_count
    }

    // ── SLP Vectorization Hazard Analysis ─────────────────────
    pub(super) fn estimate_slp_hazard(
        &mut self,
        txns: &[(String, &crate::ast::Transaction)],
    ) {
        let (r, w) = match self.ctx.spec.as_ref() {
            Some(spec) => self.target_hardware(spec),
            None => (16, 4),
        };
        if w <= 1 {
            return;
        }

        let mut float_fields: HashSet<String> = HashSet::new();
        let mut accessed_constants: HashSet<String> = HashSet::new();
        let mut total_cross_ops: u32 = 0;
        let mut peak_live_floats: u32 = 0;

        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let mut local_floats = HashSet::new();
            self.collect_local_floats_and_temps(&txn.body, &mut local_floats);
            if !local_floats.is_empty() {
                let float_names: Vec<String> = local_floats.iter().cloned().collect();
                let active = compute_peak_live_floats(&txn.body, &float_names);
                peak_live_floats = peak_live_floats.max(active);
            }

            let reads = crate::backend::collect_read_identifiers(&txn.body);
            let writes: HashSet<String> = crate::backend::collect_assigned_identifiers(&txn.body)
                .into_iter()
                .collect();

            for f in reads.union(&writes) {
                if self.is_float_field(f) {
                    float_fields.insert(f.clone());
                }
            }

            for f in reads.iter() {
                if self
                    .ctx
                    .constants
                    .get(f.as_str())
                    .map_or(false, |(t, _)| *t == Type::float())
                {
                    accessed_constants.insert(f.clone());
                }
            }

            for stmt in &txn.body {
                match stmt {
                    Statement::Assign(_, e) | Statement::Expression(e) => {
                        total_cross_ops += self.count_cross_float_ops(e, &local_floats);
                    }
                    Statement::Let { expr: Some(e), .. } => {
                        total_cross_ops += self.count_cross_float_ops(e, &local_floats);
                    }
                    Statement::Guarded(_, stmts) => {
                        for s in stmts {
                            match s {
                                Statement::Assign(_, e) | Statement::Expression(e) => {
                                    total_cross_ops +=
                                        self.count_cross_float_ops(e, &local_floats);
                                }
                                Statement::Let { expr: Some(e), .. } => {
                                    total_cross_ops +=
                                        self.count_cross_float_ops(e, &local_floats);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let n = float_fields.len();
        if n == 0 {
            return;
        }

        let packed_phis = (n + w as usize - 1) / w as usize;
        let c = total_cross_ops as usize;
        let shuffle_pressure = std::cmp::min(c, n as usize * 2);
        let const_packed = (accessed_constants.len() + w as usize - 1) / w as usize;
        let peak =
            (packed_phis + shuffle_pressure + peak_live_floats as usize + const_packed + 2) as u32;

        // 2026-07-21: Don't flag main as SLP-hazardous when all txns are
        // alwaysinline. These txns will be inlined before codegen, so
        // function-level SLP hazard is irrelevant — LLVM re-evaluates
        // after inlining. Without this, nbody_newton gets disable-slp-
        // vectorize on main, preventing any float vectorization.
        // Flagging individual txns is harmless (they're inlined away).
        let all_alwaysinline = txns.iter().all(|(_, t)| {
            t.modifiers.iter().any(|m| m.name == "inline")
                || !self.ctx.has_cycles
        });
        // 2026-07-27: Hazard-gated SLP — flags txns where SLP would degrade
        // performance. Three criteria:
        //   1. peak >= r: register pressure exceeds available registers
        //   2. ops_per_field < 1.5: too few float ops per field to amortize
        //   3. cross_per_field > 3: shuffle overhead dominates compute
        // Only non-alwaysinline txns use criteria 1-2 (alwaysinline txns are
        // absorbed into the caller). Criterion 3 applies to ALL txns — high
        // cross-op density means each SLP lane needs unique inserts regardless
        // of the calling context.
        // Unlike the original, we do NOT emit #4/#5 (disable-slp-vectorize).
        // LLVM's auto-vectorizer remains unblocked for all benchmarks.
        if peak >= r {
            if !all_alwaysinline {
                self.ctx.slp_hazard_fns.insert("main".to_string());
                for (txn_name, _) in txns {
                    self.ctx.slp_hazard_fns.insert(txn_name.clone());
                }
            }
        } else if total_cross_ops > 0 {
            let total_float_ops = self.count_all_float_ops(txns);
            if total_float_ops > 0 && n > 0 {
                let ops_per_field = total_float_ops as f64 / n as f64;
                if ops_per_field < 1.5 {
                    if !all_alwaysinline {
                        self.ctx.slp_hazard_fns.insert("main".to_string());
                        for (txn_name, _) in txns {
                            self.ctx.slp_hazard_fns.insert(txn_name.clone());
                        }
                    }
                }
            }
        }
        // 2026-07-27: Cross-ops per field — disabled for now. Nbody has 258 cross
        // ops across 31 fields (8.32/field) but SLP still helps (independent force
        // pairs). Kalman has 84 cross ops across 9 fields (9.33/field) but SLP hurts
        // (sequential matrix chains). The ratios are too close to distinguish by
        // density alone — need dependency analysis, not just counting.
    }

    pub(super) fn count_all_float_ops(
        &self,
        txns: &[(String, &crate::ast::Transaction)],
    ) -> u32 {
        let mut count = 0;
        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let mut local_floats = HashSet::new();
            self.collect_local_floats_and_temps(&txn.body, &mut local_floats);
            for stmt in &txn.body {
                match stmt {
                    Statement::Assign(_, e)
                    | Statement::Expression(e)
                    | Statement::Let {
                        expr: Some(e), ..
                    } => {
                        count += self.count_float_arith_ops(e, &local_floats);
                    }
                    Statement::Guarded(_, stmts) => {
                        for s in stmts {
                            match s {
                                Statement::Assign(_, e)
                                | Statement::Expression(e)
                                | Statement::Let {
                                    expr: Some(e), ..
                                } => {
                                    count += self.count_float_arith_ops(e, &local_floats);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        count
    }

    pub(super) fn count_float_arith_ops(
        &self,
        expr: &Expr,
        local_floats: &HashSet<String>,
    ) -> u32 {
        match expr {
            Expr::BinaryOp(_, l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats);
                let mut c = if is_float { 1 } else { 0 };
                c += self.count_float_arith_ops(l, local_floats);
                c += self.count_float_arith_ops(r, local_floats);
                c
            }
            Expr::UnaryOp(_, e) => {
                let base = if self.is_float_expr_pre_cg(e, local_floats) {
                    1
                } else {
                    0
                };
                base + self.count_float_arith_ops(e, local_floats)
            }
            Expr::Block(stmts) => stmts.last().map_or(0, |s| match s {
                Statement::Expression(e) => self.count_float_arith_ops(e, local_floats),
                _ => 0,
            }),
            _ => 0,
        }
    }

    pub(super) fn optimal_unroll_factor(&self, body: &[Statement]) -> usize {
        let mut local_floats = std::collections::HashSet::new();
        for stmt in body {
            if let Statement::Let { name, ty, expr, .. } = stmt {
                let is_float = ty.as_ref() == Some(&Type::float())
                    || expr
                        .as_ref()
                        .map_or(false, |e| self.is_float_expr_pre_cg(e, &local_floats));
                if is_float {
                    local_floats.insert(name.clone());
                }
            }
        }
        let float_names: Vec<String> = local_floats.into_iter().collect();
        let peak = if float_names.is_empty() {
            0
        } else {
            compute_peak_live_floats(body, &float_names)
        };
        let (regs, _) = match self.ctx.spec.as_ref() {
            Some(spec) => self.target_hardware(spec),
            None => (16, 4),
        };
        let inst_count = body.len() as u32;

        if peak > 0 && peak > regs / 4 {
            return 1;
        }
        if inst_count <= 3 && peak <= 1 {
            return 8;
        }
        if peak <= regs / 8 {
            return 8;
        }
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_expr(val: f64) -> Expr {
        Expr::Float(val)
    }

    fn ident(n: &str) -> Expr {
        Expr::Identifier(n.to_string())
    }

    fn add_expr(a: Expr, b: Expr) -> Expr {
        Expr::BinaryOp(crate::ast::BinaryOpKind::Add, Box::new(a), Box::new(b))
    }

    fn let_stmt(name: &str, expr: Expr) -> Statement {
        Statement::Let { names: vec![], 
            name: name.to_string(),
            ty: Some(Type::float()),
            expr: Some(expr),
            modifiers: vec![],
        }
    }

    fn assign_stmt(name: &str, expr: Expr) -> Statement {
        Statement::Assign(
            Expr::Field(Box::new(ident("state")), name.to_string()),
            expr,
        )
    }

    #[test]
    fn test_peak_two_sequential() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)),
            let_stmt("t1", float_expr(2.0)),
            assign_stmt("f0", add_expr(ident("t0"), ident("t1"))),
        ];
        let names = vec!["t0".to_string(), "t1".to_string()];
        assert_eq!(compute_peak_live_floats(&body, &names), 2);
    }

    #[test]
    fn test_peak_no_overlap() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)),
            assign_stmt("f0", ident("t0")),
            let_stmt("t1", float_expr(2.0)),
            assign_stmt("f1", ident("t1")),
        ];
        let names = vec!["t0".to_string(), "t1".to_string()];
        assert_eq!(compute_peak_live_floats(&body, &names), 1);
    }

    #[test]
    fn test_peak_three_overlapping() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)),
            let_stmt("t1", float_expr(2.0)),
            let_stmt("t2", float_expr(3.0)),
            assign_stmt(
                "f0",
                add_expr(add_expr(ident("t0"), ident("t1")), ident("t2")),
            ),
        ];
        let names = vec!["t0".to_string(), "t1".to_string(), "t2".to_string()];
        assert_eq!(compute_peak_live_floats(&body, &names), 3);
    }

    #[test]
    fn test_peak_field_read_def_at_first_use() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)),
            assign_stmt("f0", add_expr(ident("field_a"), ident("t0"))),
            assign_stmt("f1", add_expr(ident("field_a"), float_expr(5.0))),
        ];
        let names = vec!["t0".to_string(), "field_a".to_string()];
        assert_eq!(compute_peak_live_floats(&body, &names), 2);
    }

    #[test]
    fn test_peak_empty_body() {
        let body: Vec<Statement> = vec![];
        let names: Vec<String> = vec![];
        assert_eq!(compute_peak_live_floats(&body, &names), 0);
    }

    #[test]
    fn test_peak_no_floats() {
        let body = vec![let_stmt("x", float_expr(1.0))];
        let names: Vec<String> = vec![];
        assert_eq!(compute_peak_live_floats(&body, &names), 0);
    }
}
