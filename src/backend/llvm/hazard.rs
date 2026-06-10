use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::LlvmBackend;
use std::collections::HashSet;

impl LlvmBackend {
    pub(super) fn slp_attr(&self, fn_name: &str, default: &str) -> String {
        if self.slp_hazard_fns.contains(fn_name) {
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
    // Three critical guarantees make this analysis watertight:
    //   1. Local variable tracking: we walk body statements FIRST, collecting
    //      let-bound float names into `local_floats` before they're referenced.
    //   2. Operand-aware counting: any float binary op with ≥1 non-trivial
    //      operand (variable, constant, or literal) counts as a cross-op.
    //   3. Constant-load accounting: global float constants (matrix coefficients,
    //      filter taps) are counted and packed into the peak register demand.

    pub(super) fn is_float_field(&self, name: &str) -> bool {
        self.field_index_map.get(name)
            .map(|&idx| self.field_types[idx] == "float")
            .unwrap_or(false)
    }

    pub(super) fn is_float_expr_pre_cg(&self, expr: &Expr, local_floats: &HashSet<String>) -> bool {
        match expr {
            Expr::Float(_) => true,
            Expr::Identifier(name) | Expr::OwnedRef(name) => {
                self.is_float_field(name)
                    || local_floats.contains(name.as_str())
                    || self.constants.get(name.as_str()).map_or(false, |(t, _)| *t == Type::Float)
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.is_float_expr_pre_cg(l, local_floats) || self.is_float_expr_pre_cg(r, local_floats)
            }
            Expr::Neg(e) => self.is_float_expr_pre_cg(e, local_floats),
            Expr::Cast(_, ty) => *ty == Type::Float,
            Expr::Block(_, last) => self.is_float_expr_pre_cg(last, local_floats),
            _ => false,
        }
    }

    pub(super) fn count_cross_float_ops(&self, expr: &Expr, local_floats: &HashSet<String>) -> u32 {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats) || self.is_float_expr_pre_cg(r, local_floats);
                let is_cross_field = match (l.as_ref(), r.as_ref()) {
                    (Expr::Identifier(n1), Expr::Identifier(n2)) | (Expr::OwnedRef(n1), Expr::OwnedRef(n2)) => n1 != n2,
                    (Expr::Identifier(_), Expr::OwnedRef(n)) | (Expr::OwnedRef(n), Expr::Identifier(_)) => true,
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

    pub(super) fn collect_local_floats_and_temps(&self, body: &[Statement], local_floats: &mut HashSet<String>) -> u32 {
        let mut temp_count = 0;
        for stmt in body {
            match stmt {
                Statement::Let { name, ty, expr, .. } => {
                    let is_float = ty.as_ref() == Some(&Type::Float)
                        || expr.as_ref().map_or(false, |e| self.is_float_expr_pre_cg(e, local_floats));
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
        let (r, w) = match self.spec.as_ref() {
            Some(spec) => self.target_hardware(spec),
            None => (16, 4),
        };
        if w <= 1 {
            return;
        }

        let mut float_fields: HashSet<String> = HashSet::new();
        let mut accessed_constants: HashSet<String> = HashSet::new();
        let mut total_cross_ops: u32 = 0;
        let mut max_float_temps: u32 = 0;

        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let mut local_floats = HashSet::new();
            let temps = self.collect_local_floats_and_temps(&txn.body, &mut local_floats);
            max_float_temps = max_float_temps.max(temps);

            let reads = crate::backend::collect_read_identifiers(&txn.body);
            let writes: HashSet<String> =
                crate::backend::collect_assigned_identifiers(&txn.body)
                    .into_iter().collect();

            for f in reads.union(&writes) {
                if self.is_float_field(f) {
                    float_fields.insert(f.clone());
                }
            }

            for f in reads.iter() {
                if self.constants.get(f.as_str()).map_or(false, |(t, _)| *t == Type::Float) {
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
                                    total_cross_ops += self.count_cross_float_ops(expr, &local_floats);
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
        let peak = (packed_phis + shuffle_pressure + max_float_temps as usize + const_packed + 2) as u32;

        if peak >= r {
            self.slp_hazard_fns.insert("main".to_string());
            for (txn_name, _) in txns {
                self.slp_hazard_fns.insert(txn_name.clone());
            }
        } else {
            if total_cross_ops > 0 {
                let total_float_ops = self.count_all_float_ops(&txns);
                if total_float_ops > 0 && n > 0 {
                    let ops_per_field = total_float_ops as f64 / n as f64;
                    if ops_per_field < 1.5 {
                        self.slp_hazard_fns.insert("main".to_string());
                        for (txn_name, _) in txns {
                            self.slp_hazard_fns.insert(txn_name.clone());
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
                    Statement::Assignment { expr, .. } | Statement::Let { expr: Some(expr), .. } => {
                        count += self.count_float_arith_ops(expr, &local_floats);
                    }
                    Statement::Guarded { statements, .. } => {
                        for s in statements {
                            match s {
                                Statement::Assignment { expr, .. } | Statement::Let { expr: Some(expr), .. } => {
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
}
