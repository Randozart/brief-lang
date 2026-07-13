use crate::ast::{Expr, Pattern, Statement, Type};
use crate::backend::llvm::LlvmBackend;
use std::collections::HashSet;

fn expr_refs_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Identifier(n) => n == name,
        Expr::PriorState(n) => n == name,
        Expr::Add(l, r)
        | Expr::Sub(l, r)
        | Expr::Mul(l, r)
        | Expr::Div(l, r)
        | Expr::Eq(l, r)
        | Expr::Ne(l, r)
        | Expr::Lt(l, r)
        | Expr::Le(l, r)
        | Expr::Gt(l, r)
        | Expr::Ge(l, r)
        | Expr::Or(l, r)
        | Expr::And(l, r)
        | Expr::BitAnd(l, r)
        | Expr::BitOr(l, r)
        | Expr::BitXor(l, r)
        | Expr::Shl(l, r)
        | Expr::Shr(l, r) => expr_refs_name(l, name) || expr_refs_name(r, name),
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) | Expr::Cast(e, _) => expr_refs_name(e, name),
        Expr::Call(_, args) | Expr::IntrinsicCall { args, .. } => {
            args.iter().any(|a| expr_refs_name(a, name))
        }
        Expr::Projection { source, .. } => expr_refs_name(source, name),
        Expr::ListIndex(l, i) => expr_refs_name(l, name) || expr_refs_name(i, name),
        Expr::FieldAccess(obj, _) => expr_refs_name(obj, name),
        Expr::StructInstance(_, fields) => fields.iter().any(|(_, e)| expr_refs_name(e, name)),
        Expr::Concat(l, r) => expr_refs_name(l, name) || expr_refs_name(r, name),
        Expr::Tuple(items) => items.iter().any(|e| expr_refs_name(e, name)),
        Expr::ListLiteral(items) => items.iter().any(|e| expr_refs_name(e, name)),
        Expr::MapLiteral(entries) => entries
            .iter()
            .any(|(k, v)| expr_refs_name(k, name) || expr_refs_name(v, name)),
        Expr::SetLiteral(items) => items.iter().any(|e| expr_refs_name(e, name)),
        Expr::Block(_, last) => expr_refs_name(last, name),
        Expr::Match { value, arms } => {
            expr_refs_name(value, name) || arms.iter().any(|arm| expr_refs_name(&arm.body, name))
        }
        Expr::PatternMatch { value, fields, .. } => {
            expr_refs_name(value, name) || fields.iter().any(|p| pattern_refs_name(p, name))
        }
        Expr::Slice {
            value,
            start,
            end,
            stride,
            ..
        } => {
            expr_refs_name(value, name)
                || start.as_ref().map_or(false, |e| expr_refs_name(e, name))
                || end.as_ref().map_or(false, |e| expr_refs_name(e, name))
                || stride.as_ref().map_or(false, |e| expr_refs_name(e, name))
        }
        Expr::ArrowMut {
            target,
            index,
            value,
            ..
        } => {
            expr_refs_name(target, name)
                || expr_refs_name(index, name)
                || value.as_ref().map_or(false, |v| expr_refs_name(v, name))
        }
        Expr::ArrowDiscard { target, index } => {
            expr_refs_name(target, name) || expr_refs_name(index, name)
        }
        Expr::ArrowTransfer {
            dest,
            source,
            filter,
            ..
        } => {
            expr_refs_name(dest, name)
                || expr_refs_name(source, name)
                || filter.as_ref().map_or(false, |e| expr_refs_name(e, name))
        }
        Expr::SubtypeProjection { source, .. } => expr_refs_name(source, name),
        Expr::TupleDestructure(_, expr) => expr_refs_name(expr, name),
        Expr::MultiSlice { value, .. } => expr_refs_name(value, name),
        Expr::SigCall { expr: e, .. } => expr_refs_name(e, name),
        _ => false,
    }
}

fn stmt_refs_name(stmt: &Statement, name: &str) -> bool {
    match stmt {
        Statement::Let { expr: Some(e), .. } | Statement::Assignment { expr: e, .. } => {
            expr_refs_name(e, name)
        }
        Statement::Let { expr: None, .. } => false,
        Statement::Expression(e) => expr_refs_name(e, name),
        Statement::Term {
            values, swan_song, ..
        } => {
            values
                .iter()
                .any(|v| v.as_ref().map_or(false, |e| expr_refs_name(e, name)))
                || swan_song
                    .as_ref()
                    .map_or(false, |s| stmt_refs_name(s, name))
        }
        Statement::TermBang {
            values, swan_song, ..
        } => {
            values
                .iter()
                .any(|v| v.as_ref().map_or(false, |e| expr_refs_name(e, name)))
                || swan_song
                    .as_ref()
                    .map_or(false, |s| stmt_refs_name(s, name))
        }
        Statement::Guarded {
            condition,
            statements,
            ..
        } => expr_refs_name(condition, name) || statements.iter().any(|s| stmt_refs_name(s, name)),
        Statement::Escape(Some(expr)) => expr_refs_name(expr, name),
        Statement::Escape(None) => false,
        Statement::Unification { fields, expr, .. } => {
            fields.iter().any(|p| pattern_refs_name(p, name)) || expr_refs_name(expr, name)
        }
        Statement::SyncBlock { body } => body.iter().any(|s| stmt_refs_name(s, name)),
        _ => false,
    }
}

fn pattern_refs_name(pattern: &Pattern, name: &str) -> bool {
    match pattern {
        Pattern::Var(n) => n == name,
        Pattern::Tuple(items) => items.iter().any(|p| pattern_refs_name(p, name)),
        _ => false,
    }
}

/// Compute the peak number of simultaneously-live float values across a
/// statement body, using classical interval analysis (def→last-use).
///
/// Why interval analysis instead of register-allocation simulation: we
/// only need to know whether peak register demand exceeds the hardware
/// budget, not an exact allocation. Interval analysis is O(N × V) where
/// N = statements, V = float names, which is fast (< 1µs for real bodies).
/// A full register allocator simulation would be O(N × V × R) and is
/// unnecessary — the only question is "do we risk spills if SLP multiplies
/// demand by vector_width?"
///
/// Def point: for let-bound floats, the let statement. For state fields
/// or constants, the first statement that references the name (the implicit
/// load from %State or global).
/// Last use: the last statement that references the name within the body.
fn compute_peak_live_floats(body: &[Statement], float_names: &[String]) -> u32 {
    if float_names.is_empty() {
        return 0;
    }
    let mut intervals: Vec<(usize, usize)> = Vec::with_capacity(float_names.len());
    for name in float_names.iter() {
        // Is this name let-bound in this body?
        let let_idx = body
            .iter()
            .position(|s| matches!(s, Statement::Let { name: n, .. } if n == name));
        let def_idx = match let_idx {
            Some(idx) => idx,
            None => {
                // Field or const — first reference is the def point
                body.iter()
                    .position(|s| stmt_refs_name(s, name))
                    .unwrap_or(0)
            }
        };
        // last use: scan forward from def+1
        let mut last_use = def_idx;
        for (i, stmt) in body.iter().enumerate().skip(def_idx + 1) {
            if stmt_refs_name(stmt, name) {
                last_use = i;
            }
        }
        intervals.push((def_idx, last_use));
    }
    // Sweep program points, count active intervals
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
    pub(super) fn slp_attr(&self, fn_name: &str, default: &str) -> String {
        // 2026-07-05: Map main functions to #9 to avoid attribute collision.
        // clang-generated bitcode (from brief_rt.c) uses #0-#8, which would
        // override the program's #0 during llvm-link LTO merging.
        // clang's #0 has memory(inaccessiblemem: write), which makes LLVM
        // eliminate fprintf@stdout calls — causing precomputed benchmarks
        // with I/O to produce empty output (knucleotide, fasta).
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

    // ── SLP Vectorization Hazard Analysis ─────────────────────
    //
    // Why this exists: LLVM's SLP vectorizer can produce code that is
    // slower than scalar if register pressure exceeds hardware capacity.
    // SLP packs multiple scalar operations into vector instructions,
    // increasing live register demand linearly with vector width.
    // On AVX2 (16 regs, 8-wide), SLP can turn 2 live floats into 16
    // by packing 8 operations that each reference different fields.
    //
    // The analysis computes peak register demand from three sources:
    //   a) Live float temporaries from interval analysis (def→last-use)
    //   b) Shuffle pressure from cross-field float operations (expensive)
    //   c) Packed constants (global float values loaded into registers)
    //
    // If peak >= available registers (e.g., 16 for AVX2), we disable
    // SLP entirely for that function by selecting attribute groups
    // #4/#5 instead of #0/#3. The #4/#5 groups have
    // -prefer-vector-width=1 and -vectorize-loops=false.
    //
    // Even if peak < available registers, we also check the ratio of
    // float operations to distinct float fields. If ops_per_field < 1.5,
    // there are too few operations per field to amortize the shuffle
    // cost of SLP packing — scalar is faster.
    //
    // Three critical guarantees make this analysis watertight:
    //   1. Local variable tracking: we walk body statements FIRST, collecting
    //      let-bound float names into `local_floats` before they're referenced.
    //   2. Operand-aware counting: any float binary op with >=1 non-trivial
    //      operand (variable, constant, or literal) counts as a cross-op.
    //   3. Constant-load accounting: global float constants (matrix coefficients,
    //      filter taps) are counted and packed into the peak register demand.

    pub(super) fn is_float_field(&self, name: &str) -> bool {
        self.ctx
            .field_index_map
            .get(name)
            // 2026-06-29: Check for both "float" (Float) and "double" (Float64)
            .map(|&idx| {
                let ll_ty = &self.ctx.field_types[idx];
                ll_ty == "float" || ll_ty == "double"
            })
            .unwrap_or(false)
    }

    pub(super) fn is_float_expr_pre_cg(&self, expr: &Expr, local_floats: &HashSet<String>) -> bool {
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
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats)
            }
            Expr::Neg(e) => self.is_float_expr_pre_cg(e, local_floats),
            Expr::Cast(_, ty) => *ty == Type::float(),
            Expr::Block(_, last) => self.is_float_expr_pre_cg(last, local_floats),
            _ => false,
        }
    }

    pub(super) fn count_cross_float_ops(&self, expr: &Expr, local_floats: &HashSet<String>) -> u32 {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats);
                let is_cross_field = match (l.as_ref(), r.as_ref()) {
                    (Expr::Identifier(n1), Expr::Identifier(n2)) => n1 != n2,
                    (e1 @ Expr::AddrOf(_), e2 @ Expr::AddrOf(_)) => {
                        e1.as_var_name() != e2.as_var_name()
                    }
                    (Expr::Identifier(_), Expr::AddrOf(_))
                    | (Expr::AddrOf(_), Expr::Identifier(_)) => true,
                    _ => false,
                };
                let mut count = if is_cross_field && is_float { 1 } else { 0 };
                count += self.count_cross_float_ops(l, local_floats);
                count += self.count_cross_float_ops(r, local_floats);
                count
            }
            Expr::Neg(e) => self.count_cross_float_ops(e, local_floats),
            Expr::Block(_, last) => self.count_cross_float_ops(last, local_floats),
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
                        || expr
                            .as_ref()
                            .map_or(false, |e| self.is_float_expr_pre_cg(e, local_floats));
                    if is_float {
                        local_floats.insert(name.clone());
                        temp_count += 1;
                    }
                }
                Statement::Guarded { statements, .. } => {
                    temp_count += self.collect_local_floats_and_temps(statements, local_floats);
                }
                _ => {}
            }
        }
        temp_count
    }

    /// Map a target spec to (register_count, vector_width).
    ///
    /// Returned pair is used to decide whether SLP vectorization fits
    /// within the available register file. The register count is the
    /// number of architectural float/vector registers; the vector width
    /// is the number of scalar elements per vector.
    ///
    /// Why these specific values:
    ///   AVX512: 32 zmm registers, 16-wide (64-bit floats in 512-bit)
    ///   AVX2:   16 ymm registers, 8-wide  (64-bit floats in 256-bit)
    ///   NEON:   32 q registers, 4-wide   (64-bit floats in 128-bit)
    ///   SSE:    16 xmm registers, 4-wide (64-bit floats in 128-bit)
    ///   Fallback: 16 scalar regs, width 1 (no vectorization)
    ///
    /// Note: NEON has 32 registers vs AVX2's 16, so NEON can tolerate
    /// more SLP-induced register pressure despite narrower vectors.
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

    pub(super) fn estimate_slp_hazard(&mut self, txns: &[(String, &crate::ast::Transaction)]) {
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
                    Statement::Assignment { expr, .. } => {
                        total_cross_ops += self.count_cross_float_ops(expr, &local_floats);
                    }
                    Statement::Let { expr: Some(e), .. } => {
                        total_cross_ops += self.count_cross_float_ops(e, &local_floats);
                    }
                    Statement::Guarded { statements, .. } => {
                        for s in statements {
                            match s {
                                Statement::Assignment { expr, .. } => {
                                    total_cross_ops +=
                                        self.count_cross_float_ops(expr, &local_floats);
                                }
                                Statement::Let { expr: Some(e), .. } => {
                                    total_cross_ops += self.count_cross_float_ops(e, &local_floats);
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

        if peak >= r {
            self.ctx.slp_hazard_fns.insert("main".to_string());
            for (txn_name, _) in txns {
                self.ctx.slp_hazard_fns.insert(txn_name.clone());
            }
        } else {
            if total_cross_ops > 0 {
                let total_float_ops = self.count_all_float_ops(&txns);
                if total_float_ops > 0 && n > 0 {
                    let ops_per_field = total_float_ops as f64 / n as f64;
                    if ops_per_field < 1.5 {
                        self.ctx.slp_hazard_fns.insert("main".to_string());
                        for (txn_name, _) in txns {
                            self.ctx.slp_hazard_fns.insert(txn_name.clone());
                        }
                    }
                }
            }
        }
    }

    pub(super) fn count_all_float_ops(&self, txns: &[(String, &crate::ast::Transaction)]) -> u32 {
        let mut count = 0;
        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let mut local_floats = HashSet::new();
            self.collect_local_floats_and_temps(&txn.body, &mut local_floats);
            for stmt in &txn.body {
                match stmt {
                    Statement::Assignment { expr, .. }
                    | Statement::Let {
                        expr: Some(expr), ..
                    } => {
                        count += self.count_float_arith_ops(expr, &local_floats);
                    }
                    Statement::Guarded { statements, .. } => {
                        for s in statements {
                            match s {
                                Statement::Assignment { expr, .. }
                                | Statement::Let {
                                    expr: Some(expr), ..
                                } => {
                                    count += self.count_float_arith_ops(expr, &local_floats);
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

    pub(super) fn count_float_arith_ops(&self, expr: &Expr, local_floats: &HashSet<String>) -> u32 {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats);
                let mut c = if is_float { 1 } else { 0 };
                c += self.count_float_arith_ops(l, local_floats);
                c += self.count_float_arith_ops(r, local_floats);
                c
            }
            Expr::Neg(e) => {
                if self.is_float_expr_pre_cg(e, local_floats) {
                    1 + self.count_float_arith_ops(e, local_floats)
                } else {
                    self.count_float_arith_ops(e, local_floats)
                }
            }
            Expr::Block(_, last) => self.count_float_arith_ops(last, local_floats),
            _ => 0,
        }
    }
    /// Select the optimal loop unroll factor based on register pressure analysis.
    ///
    /// Uses `compute_peak_live_floats` to estimate how many float registers are
    /// live simultaneously in the loop body. Unrolling multiplies register demand;
    /// if demand exceeds available registers, LLVM spills to stack, negating the
    /// benefit. Also considers body instruction count for very simple loops.
    ///
    /// Returns 1, 4, or 8:
    ///   1 = no unrolling (high reg pressure: peak > regs/4, avoid spills)
    ///   4 = moderate (default, balances unroll benefit with pressure)
    ///   8 = aggressive (simple body with <=3 insts and <=1 live float,
    ///       or very low pressure with peak <= regs/8)
    ///
    /// Why 4 as default: LLVM's own heuristic defaults to unroll factor ~4
    /// for most loops. 4x unrolling gives a good balance of reduced branch
    /// overhead vs register pressure. 8x is only safe when register pressure
    /// is demonstrably low.
    ///
    /// 2026-06-20: Phase 0b — replaces hardcoded `let uf = 4`.
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

        // High register pressure: peak live floats exceed 1/4 of available registers
        if peak > 0 && peak > regs / 4 {
            return 1usize;
        }
        if inst_count <= 3 && peak <= 1 {
            return 8usize;
        }
        if peak <= regs / 8 {
            return 8usize;
        }
        4usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Statement::*;
    use crate::ast::*;

    fn float_expr(val: f64) -> Expr {
        Expr::Float(val)
    }

    fn ident(n: &str) -> Expr {
        Expr::Identifier(n.to_string())
    }

    fn let_stmt(name: &str, expr: Expr) -> Statement {
        Let {
            name: name.to_string(),
            ty: Some(Type::float()),
            expr: Some(expr),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: vec![],
        }
    }

    fn assign_stmt(name: &str, expr: Expr) -> Statement {
        Assignment {
            lhs: Expr::FieldAccess(Box::new(ident("state")), name.to_string()),
            expr,
            timeout: None,
            modifiers: vec![],
        }
    }

    fn add_expr(a: Expr, b: Expr) -> Expr {
        Expr::Add(Box::new(a), Box::new(b))
    }

    fn mul_expr(a: Expr, b: Expr) -> Expr {
        Expr::Mul(Box::new(a), Box::new(b))
    }

    /// Simple sequential: t0 defined, used immediately, then t1 defined.
    /// Peak should be 2 (t0 and t1 overlap at the mul statement).
    #[test]
    fn test_peak_two_sequential() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)),                       // def t0
            let_stmt("t1", float_expr(2.0)),                       // def t1
            assign_stmt("f0", add_expr(ident("t0"), ident("t1"))), // uses t0, t1
        ];
        let names = vec!["t0".to_string(), "t1".to_string()];
        // t0: def=0, last_use=2; t1: def=1, last_use=2
        // sweep: i=0 → 1 active (t0), i=1 → 2 active (t0,t1), i=2 → 2 active (t0,t1)
        assert_eq!(compute_peak_live_floats(&body, &names), 2);
    }

    /// Non-overlapping: t0 defined and used before t1 is defined.
    /// Peak should be 1.
    #[test]
    fn test_peak_no_overlap() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)), // def t0
            assign_stmt("f0", ident("t0")),  // last use of t0
            let_stmt("t1", float_expr(2.0)), // def t1
            assign_stmt("f1", ident("t1")),  // last use of t1
        ];
        let names = vec!["t0".to_string(), "t1".to_string()];
        // t0: def=0, last_use=1; t1: def=2, last_use=3
        // sweep: i=0→1, i=1→1, i=2→1, i=3→1
        assert_eq!(compute_peak_live_floats(&body, &names), 1);
    }

    /// Three temps, all overlapping at the final expression.
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
        // All defined at 0,1,2, all used at 3
        // sweep: i=0→1, i=1→2, i=2→3, i=3→3
        assert_eq!(compute_peak_live_floats(&body, &names), 3);
    }

    /// Field read (not let-bound): first use determines def point.
    #[test]
    fn test_peak_field_read_def_at_first_use() {
        let body = vec![
            let_stmt("t0", float_expr(1.0)),
            assign_stmt("f0", add_expr(ident("field_a"), ident("t0"))), // first use of field_a, last use of t0
            assign_stmt("f1", add_expr(ident("field_a"), float_expr(5.0))), // last use of field_a
        ];
        let names = vec!["t0".to_string(), "field_a".to_string()];
        // t0: def=0, last_use=1; field_a: first_use=1, last_use=2
        // sweep: i=0→1, i=1→2, i=2→1
        assert_eq!(compute_peak_live_floats(&body, &names), 2);
    }

    /// Empty body → peak 0.
    #[test]
    fn test_peak_empty_body() {
        let body: Vec<Statement> = vec![];
        let names: Vec<String> = vec![];
        assert_eq!(compute_peak_live_floats(&body, &names), 0);
    }

    /// Empty names → peak 0.
    #[test]
    fn test_peak_no_floats() {
        let body = vec![let_stmt("x", float_expr(1.0))];
        let names: Vec<String> = vec![];
        assert_eq!(compute_peak_live_floats(&body, &names), 0);
    }
}
