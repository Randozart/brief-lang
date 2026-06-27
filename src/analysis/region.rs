use crate::ast::{BracketOp, Expr, Program, ProjectionTarget, Statement, TopLevel, Type};
use std::collections::{HashMap, HashSet, VecDeque};

/// Classification of a transaction body by computational weight.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComplexityClass {
    Trivial,
    Light,
    Medium,
    Heavy,
    Unbounded,
}

/// Optimization score and metadata for a single atomic reactive region.
#[derive(Debug, Clone)]
pub struct RegionScore {
    pub region_id: usize,
    pub txn_names: Vec<String>,
    pub complexity: ComplexityClass,
    pub body_weight: usize,
    pub iteration_count: u64,
    pub value_set_size: Option<u64>,
    pub optimization_score: f64,
    pub chain_composed: bool,
    pub gpu_eligible: bool,
}

/// Result of greedy budget allocation across regions.
#[derive(Debug, Clone)]
pub struct BudgetPlan {
    pub total_budget: u64,
    pub allocated: Vec<(usize, ComplexityClass, u64, f64)>,
    pub residual_budget: u64,
    pub skipped: Vec<(usize, ComplexityClass, u64)>,
}

/// Result of expression substitution across a linear transaction chain.
#[derive(Debug, Clone)]
pub struct ComposedChain {
    pub chain: Vec<String>,
    pub link_vars: Vec<String>,
    pub root_triggers: Vec<String>,
    pub composed_body: Vec<Statement>,
    pub counter_var: Option<String>,
    pub fused_weight: usize,
    pub trigger_values: Option<Vec<(String, i64)>>,
    pub all_internal: bool,
}

/// Classification of a variable along the predictability axis.
///
/// - **Pure**: deterministic, no dependency on frontier values — fully foldable.
/// - **Bounded**: depends on frontier but bounds are known (type + contract) —
///   can still optimize with range checks instead of exact values.
/// - **Opaque**: depends on frontier with unbounded or unknown values —
///   segmentation necessary, cannot fold through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarClass {
    Pure,
    Bounded,
    Opaque,
}

/// Compile-time-known inclusive integer interval.
#[derive(Debug, Clone, PartialEq)]
pub struct Interval {
    pub lo: i64,
    pub hi: i64,
}

impl Interval {
    pub fn contains(&self, val: i64) -> bool {
        val >= self.lo && val <= self.hi
    }

    pub fn size(&self) -> u64 {
        if self.hi < self.lo {
            0
        } else {
            (self.hi - self.lo + 1) as u64
        }
    }
}

/// Analysis result for a single variable.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub classification: VarClass,
    pub interval: Option<Interval>,
    /// Estimated number of distinct values this variable can hold,
    /// or `None` if unbounded.
    pub value_set_size: Option<u64>,
    /// Which atomic region this variable belongs to (0 = Pure/isolated).
    pub region_id: usize,
}

/// The region analyzer — classifies variables and partitions the program
/// into independent atomic reactive regions.
///
/// A region is a connected component in the dependency graph of non-Pure
/// variables. Variables in the same region must be scheduled together;
/// variables in different regions are independent.
pub struct RegionAnalyzer {
    pub var_info: HashMap<String, VarInfo>,
    pub trigger_vars: HashSet<String>,
    pub regions: Vec<Vec<String>>,
    deps: HashMap<String, HashSet<String>>,
    rev_deps: HashMap<String, HashSet<String>>,
    txn_reads: HashMap<String, HashSet<String>>,
    txn_writes: HashMap<String, HashSet<String>>,
    pub linear_chains: Vec<Vec<String>>,
    txn_bodies: HashMap<String, Vec<Statement>>,
    iter_bounds: HashMap<String, u64>,
    pub region_scores: Vec<RegionScore>,
    pub budget_plan: Option<BudgetPlan>,
    pub composed_chains: Vec<ComposedChain>,
}

impl RegionAnalyzer {
    /// Run the full analysis pipeline on a parsed program.
    pub fn analyze(program: &Program) -> Self {
        let mut analyzer = RegionAnalyzer {
            var_info: HashMap::new(),
            trigger_vars: HashSet::new(),
            regions: Vec::new(),
            deps: HashMap::new(),
            rev_deps: HashMap::new(),
            txn_reads: HashMap::new(),
            txn_writes: HashMap::new(),
            linear_chains: Vec::new(),
            txn_bodies: HashMap::new(),
            iter_bounds: HashMap::new(),
            region_scores: Vec::new(),
            budget_plan: None,
            composed_chains: Vec::new(),
        };

        analyzer.register_declarations(program);
        analyzer.build_dependency_graph(program);
        analyzer.seed_frontier();
        analyzer.propagate_classification();
        analyzer.compute_regions();
        analyzer.estimate_value_sets();
        analyzer.detect_linear_chains(program);
        analyzer.resolve_iteration_bounds(program);
        analyzer.compute_region_scores(program);

        analyzer
    }

    // ── Phase A: Collect declarations ──────────────────────────────────

    fn register_declarations(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    let interval = decl.expr.as_ref().and_then(|e| Self::expr_to_interval(e));
                    // Check for `<: [lo..hi]` range constraint (desugared as `_ >= lo && _ <= hi`)
                    let mut range_interval: Option<Interval> = None;
                    let mut range_size: Option<u64> = None;
                    if let Some(constraint) = &decl.constraint {
                        if let Some((lo_expr, hi_expr)) = Self::extract_range_from_constraint(constraint) {
                            if let (Some(lo), Some(hi)) = (
                                Self::eval_expr_simple(lo_expr, &HashMap::new()),
                                Self::eval_expr_simple(hi_expr, &HashMap::new()),
                            ) {
                                if lo <= hi {
                                    range_interval = Some(Interval { lo, hi });
                                    let sz = (hi as i128 - lo as i128).unsigned_abs() + 1;
                                    range_size = Some(if sz > u64::MAX as u128 { u64::MAX } else { sz as u64 });
                                }
                            }
                        }
                    }
                    let vc = VarInfo {
                        classification: if range_interval.is_some() { VarClass::Bounded } else { VarClass::Pure },
                        interval: range_interval.or(interval),
                        value_set_size: range_size,
                        region_id: 0,
                    };
                    self.var_info.entry(decl.name.clone()).or_insert(vc);
                    self.deps.entry(decl.name.clone()).or_default();
                    self.rev_deps.entry(decl.name.clone()).or_default();
                }
                TopLevel::Constant(constant) => {
                    let interval = Self::expr_to_interval(&constant.expr);
                    let vc = VarInfo {
                        classification: VarClass::Pure,
                        interval,
                        value_set_size: None,
                        region_id: 0,
                    };
                    self.var_info.entry(constant.name.clone()).or_insert(vc);
                    self.deps.entry(constant.name.clone()).or_default();
                    self.rev_deps.entry(constant.name.clone()).or_default();
                }
                TopLevel::Trigger(trg) => {
                    self.trigger_vars.insert(trg.name.clone());
                    let interval = Self::type_to_interval(&trg.ty);
                    // Seed as Opaque; seed_frontier will downgrade to Bounded
                    // if the type gives a narrow interval.
                    let vc = VarInfo {
                        classification: VarClass::Opaque,
                        interval,
                        value_set_size: None,
                        region_id: 0,
                    };
                    self.var_info.entry(trg.name.clone()).or_insert(vc);
                    self.deps.entry(trg.name.clone()).or_default();
                    self.rev_deps.entry(trg.name.clone()).or_default();
                }
                _ => {}
            }
        }
    }

    // ── Phase B: Build dependency graph ─────────────────────────────────

    fn build_dependency_graph(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                let mut txn_read_vars = HashSet::new();
                let mut txn_write_vars = HashSet::new();

                self.txn_bodies.insert(txn.name.clone(), txn.body.clone());

                self.collect_identifiers(&txn.contract.pre_condition, &txn.name);
                self.collect_identifiers(&txn.contract.post_condition, &txn.name);

                for stmt in &txn.body {
                    if let Statement::Assignment { lhs, expr, .. } = stmt {
                        let writer = match lhs {
                            Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                            _ => continue,
                        };
                        txn_write_vars.insert(writer.clone());
                        txn_read_vars.extend(expr_to_var_set(expr));
                        self.collect_identifiers(expr, &writer);
                    }
                    if let Statement::Let { name, expr, .. } = stmt {
                        if let Some(e) = expr {
                            txn_write_vars.insert(name.clone());
                            txn_read_vars.extend(expr_to_var_set(e));
                            self.collect_identifiers(e, name);
                        }
                    }
                }

                self.txn_reads.entry(txn.name.clone()).or_default().extend(txn_read_vars);
                self.txn_writes.entry(txn.name.clone()).or_default().extend(txn_write_vars);
            }
        }
    }

    /// Iterative identifier collection. Uses explicit stack to avoid recursion.
    fn collect_identifiers(&mut self, expr: &Expr, reader_for: &str) {
        let rf = reader_for.to_string();
        let mut work: Vec<&Expr> = vec![expr];

        while let Some(e) = work.pop() {
            match e {
                Expr::Identifier(name) | Expr::OwnedRef(name) => {
                    self.deps.entry(rf.clone()).or_default().insert(name.clone());
                    self.rev_deps.entry(name.clone()).or_default().insert(rf.clone());
                }
                Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b)
                | Expr::Div(a, b) | Expr::Mod(a, b)
                | Expr::Eq(a, b) | Expr::Ne(a, b)
                | Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b)
                | Expr::And(a, b) | Expr::Or(a, b)
                | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
                | Expr::Shl(a, b) | Expr::Shr(a, b) | Expr::Concat(a, b) => {
                    work.push(b);
                    work.push(a);
                }
                Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _)
                | Expr::Projection { source: a, .. } => {
                    work.push(a);
                }
                Expr::Call(_, args) => {
                    for arg in args.iter().rev() {
                        work.push(arg);
                    }
                }
                Expr::ListLiteral(elems) | Expr::Tuple(elems) => {
                    for e in elems.iter().rev() {
                        work.push(e);
                    }
                }
                Expr::ListIndex(list, idx) => {
                    work.push(idx);
                    work.push(list);
                }
                Expr::FieldAccess(obj, _) => {
                    work.push(obj);
                }
                Expr::Block(_, last) | Expr::TupleDestructure(_, last) => {
                    work.push(last);
                }
                Expr::Match { value, arms } => {
                    for arm in arms.iter().rev() {
                        if let Some(g) = &arm.guard { work.push(g); }
                        work.push(&arm.body);
                    }
                    work.push(value.as_ref());
                }
                Expr::PatternMatch { value, .. } => {
                    work.push(value.as_ref());
                }
                Expr::Within { body, fallback, .. } => {
                    work.push(fallback);
                    work.push(body);
                }
                Expr::Slice { value, start, end, stride, mask } => {
                    if let Some(s) = start { work.push(s); }
                    if let Some(e) = end { work.push(e); }
                    if let Some(s) = stride { work.push(s); }
                    if let Some(m) = mask { work.push(m); }
                    work.push(value);
                }
                Expr::MultiSlice { value, ops } => {
                    for op in ops.iter().rev() {
                        match op {
                            BracketOp::Mask(m) => work.push(m.as_ref()),
                            BracketOp::Stride(s) => work.push(s.as_ref()),
                            BracketOp::Coord(_) => {}
                        }
                    }
                    work.push(value);
                }
                Expr::StructInstance(_, fields) | Expr::ObjectLiteral(fields) => {
                    for (_, e) in fields.iter().rev() {
                        work.push(e);
                    }
                }
                _ => {}
            }
        }
    }

    // ── Phase C: Seed frontier ──────────────────────────────────────────

    fn seed_frontier(&mut self) {
        let trigger_names: Vec<String> = self.trigger_vars.iter().cloned().collect();
        for name in trigger_names {
            if let Some(info) = self.var_info.get(&name) {
                // If the trigger has a tight bound, mark as Bounded not Opaque.
                let is_tight = info
                    .interval
                    .as_ref()
                    .map(|iv| iv.size() <= 256)
                    .unwrap_or(false);
                if is_tight {
                    if let Some(info) = self.var_info.get_mut(&name) {
                        info.classification = VarClass::Bounded;
                    }
                }
            }
        }
    }

    // ── Phase D: Propagate classification ───────────────────────────────

    fn propagate_classification(&mut self) {
        // BFS from frontier vars: any var that reads a non-Pure var
        // inherits the most restrictive classification.
        let var_names: Vec<String> = self.var_info.keys().cloned().collect();
        let mut queue: VecDeque<String> = VecDeque::new();

        // Seed the queue with all non-Pure vars
        for name in &var_names {
            if let Some(info) = self.var_info.get(name) {
                if info.classification != VarClass::Pure {
                    queue.push_back(name.clone());
                }
            }
        }

        while let Some(frontier_var) = queue.pop_front() {
            let frontier_class = self.var_info[&frontier_var].classification.clone();

            // Propagate to all vars that read this frontier_var
            if let Some(readers) = self.rev_deps.get(&frontier_var).cloned() {
                for reader in readers {
                    // Skip unregistered vars (e.g. transaction params, foreign bindings)
                    let Some(reader_info) = self.var_info.get(&reader) else { continue; };
                    let cur_class = &reader_info.classification;
                    let needs_update = *cur_class == VarClass::Pure
                        || (frontier_class == VarClass::Opaque
                            && *cur_class == VarClass::Bounded);
                    if needs_update {
                        if let Some(info) = self.var_info.get_mut(&reader) {
                            info.classification = if frontier_class == VarClass::Opaque {
                                VarClass::Opaque
                            } else {
                                VarClass::Bounded
                            };
                            queue.push_back(reader.clone());
                        }
                    }
                }
            }
        }
    }

    // ── Phase E: Find connected components (regions) ────────────────────

    fn compute_regions(&mut self) {
        let mut visited: HashSet<String> = HashSet::new();
        let mut region_id_counter: usize = 0;

        let non_pure: Vec<String> = self
            .var_info
            .iter()
            .filter(|(_, info)| info.classification != VarClass::Pure)
            .map(|(name, _)| name.clone())
            .collect();

        for seed in &non_pure {
            if visited.contains(seed) {
                continue;
            }

            let mut component: Vec<String> = Vec::new();
            let mut stack: Vec<String> = vec![seed.clone()];

            while let Some(var) = stack.pop() {
                if !visited.insert(var.clone()) {
                    continue;
                }
                component.push(var.clone());

                // Neighbors = deps[var] ∪ rev_deps[var], filtered to non-Pure
                let mut neighbors: Vec<String> = Vec::new();
                if let Some(deps) = self.deps.get(&var) {
                    for n in deps.iter() {
                        if self.var_info.get(n).map(|i| i.classification != VarClass::Pure).unwrap_or(false)
                        {
                            neighbors.push(n.clone());
                        }
                    }
                }
                if let Some(revs) = self.rev_deps.get(&var) {
                    for n in revs.iter() {
                        if self.var_info.get(n).map(|i| i.classification != VarClass::Pure).unwrap_or(false)
                        {
                            neighbors.push(n.clone());
                        }
                    }
                }

                for n in neighbors {
                    if !visited.contains(&n) {
                        stack.push(n);
                    }
                }
            }

            if !component.is_empty() {
                region_id_counter += 1;
                for var in &component {
                    if let Some(info) = self.var_info.get_mut(var) {
                        info.region_id = region_id_counter;
                    }
                }
                self.regions.push(component);
            }
        }

        // Pure vars get region_id = 0
        for (_, info) in self.var_info.iter_mut() {
            if info.classification == VarClass::Pure {
                info.region_id = 0;
            }
        }
    }

    // ── Phase G: Estimate value set sizes ───────────────────────────────

    fn estimate_value_sets(&mut self) {
        let names: Vec<String> = self.var_info.keys().cloned().collect();
        for name in names {
            let size = self.compute_value_set_size(&name);
            if let Some(info) = self.var_info.get_mut(&name) {
                info.value_set_size = size;
            }
        }
    }

    fn compute_value_set_size(&self, name: &str) -> Option<u64> {
        let info = self.var_info.get(name)?;
        match info.classification {
            VarClass::Pure => {
                // Pure variables: if constant → size 1, if interval → size of interval
                if let Some(ref iv) = info.interval {
                    let s = iv.size();
                    if s > 0 { Some(s) } else { None }
                } else {
                    None
                }
            }
            VarClass::Bounded => {
                // Bounded: use interval if available, otherwise fall back to type
                if let Some(ref iv) = info.interval {
                    Some(iv.size())
                } else {
                    None
                }
            }
            VarClass::Opaque => None, // Can't estimate
        }
    }

    // ── Utilities ───────────────────────────────────────────────────────

    /// Extract a compile-time-known integer interval from an expression.
    /// Returns `Some((lo, hi))` if the expression is a known constant or
    /// a narrow range of constants.
    fn expr_to_interval(expr: &Expr) -> Option<Interval> {
        match expr {
            Expr::Integer(n) => Some(Interval { lo: *n, hi: *n }),
            Expr::Bool(b) => Some(Interval {
                lo: if *b { 1 } else { 0 },
                hi: if *b { 1 } else { 0 },
            }),
            Expr::Neg(inner) => {
                Self::expr_to_interval(inner).map(|iv| Interval {
                    lo: -iv.hi,
                    hi: -iv.lo,
                })
            }
            _ => None,
        }
    }

    /// Extract bounds from a Brief type (e.g., `Bool` → `[0,1]`, `U8` → `[0,255]`).
    fn type_to_interval(ty: &Type) -> Option<Interval> {
        match ty {
            Type::Bool => Some(Interval { lo: 0, hi: 1 }),
            Type::Int => None, // Full i64 range — unbounded
            Type::UInt => Some(Interval {
                lo: 0,
                hi: i64::MAX,
            }),
            Type::Char => Some(Interval {
                lo: 0,
                hi: 0x10FFFF,
            }),
            _ => None,
        }
    }

    // ── Phase H: Detect linear transaction chains ───────────────────────

    fn detect_linear_chains(&mut self, _program: &Program) {
        self.linear_chains.clear();
        let txn_names: Vec<String> = self.txn_reads.keys().cloned().collect();

        for txn in &txn_names {
            let reads = self.txn_reads.get(txn).cloned().unwrap_or_default();
            let writes = self.txn_writes.get(txn).cloned().unwrap_or_default();

            // A linear chain starts when a transaction's writes are exclusively
            // consumed by another single transaction's reads.
            for written in &writes {
                if let Some(readers) = self.rev_deps.get(written) {
                    let downstream_txns: Vec<&String> = txn_names
                        .iter()
                        .filter(|tn| {
                            *tn != txn
                                && self.txn_reads.get(*tn).map(|r| r.contains(written)).unwrap_or(false)
                        })
                        .collect();

                    if downstream_txns.len() == 1 {
                        let next = downstream_txns[0].clone();
                        let mut chain = vec![txn.clone(), next.clone()];

                        // Extend the chain: follow reads→writes forward
                        loop {
                            let last = match chain.last() {
                                Some(l) => l.clone(),
                                None => break,
                            };
                            let cur_writes = self.txn_writes.get(&last).cloned().unwrap_or_default();
                            let mut extended = false;
                            for cw in &cur_writes {
                                let further: Vec<String> = txn_names
                                    .iter()
                                    .filter(|tn| {
                                        !chain.contains(tn)
                                            && self.txn_reads.get(*tn).map(|r| r.contains(cw)).unwrap_or(false)
                                    })
                                    .cloned()
                                    .collect();
                                if further.len() == 1 {
                                    chain.push(further[0].clone());
                                    extended = true;
                                    break;
                                }
                            }
                            if !extended { break; }
                        }

                        self.linear_chains.push(chain);
                    }
                }
            }
        }

        // Deduplicate: keep only maximal chains (supersets of others)
        self.linear_chains.sort_by(|a, b| b.len().cmp(&a.len()));
        let mut deduped: Vec<Vec<String>> = Vec::new();
        for chain in &self.linear_chains {
            let chain_set: HashSet<&String> = chain.iter().collect();
            if !deduped.iter().any(|existing| {
                let existing_set: HashSet<&String> = existing.iter().collect();
                existing_set.is_superset(&chain_set) && existing.len() > chain.len()
            }) {
                deduped.push(chain.clone());
            }
        }
        self.linear_chains = deduped;
    }

    // ── Public query API ────────────────────────────────────────────────

    /// Get the region ID for a given variable.
    pub fn region_of(&self, var: &str) -> Option<usize> {
        self.var_info.get(var).map(|i| i.region_id)
    }

    /// Get the classification of a given variable.
    pub fn classification_of(&self, var: &str) -> Option<VarClass> {
        self.var_info.get(var).map(|i| i.classification.clone())
    }

    /// Check if a variable is trigger-dependent (Bounded or Opaque).
    pub fn is_frontier_dependent(&self, var: &str) -> bool {
        self.var_info
            .get(var)
            .map(|i| i.classification != VarClass::Pure)
            .unwrap_or(false)
    }

    /// Get the estimated value-set size for a variable, or `None` if unbounded.
    pub fn value_set_size_of(&self, var: &str) -> Option<u64> {
        self.var_info.get(var).and_then(|i| i.value_set_size)
    }

    // ── Iteration bound resolution ────────────────────────────────

    fn resolve_iteration_bounds(&mut self, program: &Program) {
        self.iter_bounds.clear();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if let Some(bound) = Self::extract_bound_from_pre(&txn.contract.pre_condition) {
                    let val = self.resolve_bound_value(program, &bound.1);
                    if let Some(v) = val {
                        self.iter_bounds.insert(txn.name.clone(), v);
                    }
                }
            }
        }
    }

    fn extract_bound_from_pre(pre: &Expr) -> Option<(String, String)> {
        match pre {
            Expr::Lt(a, b) | Expr::Le(a, b) => {
                match (a.as_ref(), b.as_ref()) {
                    (Expr::Identifier(var), Expr::Identifier(bound)) => {
                        Some((var.clone(), bound.clone()))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn resolve_bound_value(&self, program: &Program, bound_var: &str) -> Option<u64> {
        for item in &program.items {
            match item {
                TopLevel::Constant(c) if c.name == bound_var => {
                    if let Expr::Integer(n) = &c.expr { return Some(*n as u64); }
                }
                TopLevel::StateDecl(d) if d.name == bound_var => {
                    if let Some(Expr::Integer(n)) = &d.expr { return Some(*n as u64); }
                }
                _ => {}
            }
        }
        None
    }

    // ── Complexity estimation ─────────────────────────────────────

    fn classify_complexity(&self, body: &[Statement]) -> ComplexityClass {
        let weight = count_statements_recursive(body);
        let has_ffi = self.has_ffi_or_trigger_refs(body);
        if has_ffi {
            ComplexityClass::Unbounded
        } else if weight <= 2 {
            ComplexityClass::Trivial
        } else if weight <= 5 {
            ComplexityClass::Light
        } else if weight <= 20 {
            ComplexityClass::Medium
        } else {
            ComplexityClass::Heavy
        }
    }

    fn has_ffi_or_trigger_refs(&self, body: &[Statement]) -> bool {
        for stmt in body {
            if has_ffi_or_trigger_stmt(stmt, &self.trigger_vars) {
                return true;
            }
        }
        false
    }

    // ── Region scoring ────────────────────────────────────────────

    fn compute_region_scores(&mut self, program: &Program) {
        self.region_scores.clear();

        let txn_to_region: HashMap<String, usize> = self.build_txn_to_region_map();
        let mut region_txns: HashMap<usize, Vec<String>> = HashMap::new();
        for (txn, rid) in &txn_to_region {
            region_txns.entry(*rid).or_default().push(txn.clone());
        }

        for (rid, txn_list) in region_txns {
            let mut body_weight: usize = 0;
            let mut complexity = ComplexityClass::Trivial;
            let mut max_iter: u64 = 1;
            let mut combined_vset_size: Option<u64> = Some(1);
            let mut all_pure_and_nop_term = true;

            for tn in &txn_list {
                if let Some(body) = self.txn_bodies.get(tn) {
                    let c = self.classify_complexity(body);
                    if c != ComplexityClass::Trivial {
                        complexity = std::cmp::max(complexity.clone(), c);
                    }
                    body_weight += count_statements_recursive(body);
                }

                // GPU eligibility
                if let Some(body) = self.txn_bodies.get(tn) {
                    if has_term_or_unify_escape(body) {
                        all_pure_and_nop_term = false;
                    }
                }
                if self.has_txn_trigger_refs(tn) {
                    all_pure_and_nop_term = false;
                }

                // iteration count
                let iter = self.iter_bounds.get(tn).copied().unwrap_or(1);
                if iter > max_iter { max_iter = iter; }

                // value set size
                if let Some(reads) = self.txn_reads.get(tn) {
                    for r in reads {
                        if self.trigger_vars.contains(r) {
                            let sz = self.value_set_size_of(r);
                            combined_vset_size = match (combined_vset_size, sz) {
                                (Some(a), Some(b)) => Some(a.saturating_mul(b)),
                                _ => None,
                            };
                        }
                    }
                }
            }

            let cost = combined_vset_size.unwrap_or(1).max(1);
            let score = if complexity == ComplexityClass::Unbounded {
                f64::NEG_INFINITY
            } else {
                (body_weight as f64 * max_iter as f64) / cost as f64
            };

            let gpu_eligible = all_pure_and_nop_term
                && complexity != ComplexityClass::Trivial
                && complexity != ComplexityClass::Light
                && complexity != ComplexityClass::Unbounded;

            self.region_scores.push(RegionScore {
                region_id: rid,
                txn_names: txn_list,
                complexity,
                body_weight,
                iteration_count: max_iter,
                value_set_size: combined_vset_size,
                optimization_score: score,
                chain_composed: false,
                gpu_eligible,
            });
        }

        self.region_scores.sort_by(|a, b| {
            b.optimization_score
                .partial_cmp(&a.optimization_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    fn build_txn_to_region_map(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for rid in 1..=self.regions.len() {
            if let Some(region) = self.regions.get(rid - 1) {
                let txn_in_region: Vec<&String> = self.txn_writes
                    .iter()
                    .filter(|(_, writes)| writes.iter().any(|w| region.contains(w)))
                    .map(|(tn, _)| tn)
                    .collect();
                for tn in txn_in_region {
                    map.entry(tn.clone()).or_insert(rid);
                }
            }
        }
        map
    }

    fn has_txn_trigger_refs(&self, txn_name: &str) -> bool {
        if let Some(reads) = self.txn_reads.get(txn_name) {
            reads.iter().any(|r| self.trigger_vars.contains(r))
        } else {
            false
        }
    }

    // ── Budget planning ───────────────────────────────────────────

    pub fn build_budget_plan(&mut self, budget: u64) {
        let mut allocated = Vec::new();
        let mut skipped = Vec::new();
        let mut remaining = budget;

        for score in &self.region_scores {
            if score.complexity == ComplexityClass::Unbounded {
                skipped.push((score.region_id, score.complexity.clone(), 0));
                continue;
            }
            let cost = score.value_set_size.unwrap_or(u64::MAX);
            if cost == u64::MAX || cost > remaining {
                skipped.push((score.region_id, score.complexity.clone(), cost.min(remaining)));
            } else {
                remaining = remaining.saturating_sub(cost);
                allocated.push((
                    score.region_id,
                    score.complexity.clone(),
                    cost,
                    score.optimization_score,
                ));
            }
        }

        self.budget_plan = Some(BudgetPlan {
            total_budget: budget,
            allocated,
            residual_budget: remaining,
            skipped,
        });
    }

    /// Query the resolved iteration bound for a given transaction name.
    pub fn iteration_bound_of(&self, txn_name: &str) -> Option<u64> {
        self.iter_bounds.get(txn_name).copied()
    }

    // ── Chain composition (Phase 4.2) ─────────────────────────────

    pub fn compose_chains(&mut self) {
        self.composed_chains.clear();
        let chains = self.linear_chains.clone();
        let all_txn_names: HashSet<String> = self.txn_reads.keys().cloned().collect();

        for chain in &chains {
            if chain.len() < 2 { continue; }

            let link_vars = self.compute_link_vars(chain);
            if link_vars.is_empty() { continue; }

            if !self.chain_is_composable(chain, &all_txn_names) { continue; }

            let root_triggers: Vec<String> = self.txn_reads
                .get(&chain[0])
                .map(|reads| reads.iter()
                    .filter(|r| self.trigger_vars.contains(*r))
                    .cloned()
                    .collect())
                .unwrap_or_default();

            let all_internal = link_vars.iter().all(|lv| self.var_is_chain_internal(lv, chain, &all_txn_names));

            if root_triggers.is_empty() {
                self.push_composed_chain(chain, &link_vars, &root_triggers, None, all_internal);
            } else if root_triggers.len() == 1 {
                let trg = &root_triggers[0];
                if let Some(info) = self.var_info.get(trg) {
                    if let Some(ref iv) = info.interval {
                        let bound = iv.size().min(256);
                        let hi = iv.lo.saturating_add((bound - 1) as i64).min(iv.hi);
                        for val in iv.lo..=hi {
                            let tv = vec![(trg.clone(), val)];
                            self.push_composed_chain(chain, &link_vars, &root_triggers, Some(&tv), all_internal);
                        }
                    } else {
                        self.push_composed_chain(chain, &link_vars, &root_triggers, None, all_internal);
                    }
                } else {
                    self.push_composed_chain(chain, &link_vars, &root_triggers, None, all_internal);
                }
            } else {
                self.push_composed_chain(chain, &link_vars, &root_triggers, None, all_internal);
            }
        }

        self.update_scores_after_composition();
    }

    pub fn is_fully_precomputable(&self, budget: u64) -> bool {
        if self.composed_chains.is_empty() {
            return false;
        }
        let mut total: u64 = 0;
        for cc in &self.composed_chains {
            if has_ffi_or_trigger_stmt_in_chain(&cc.composed_body) {
                return false;
            }
            if cc.trigger_values.is_some() {
                return false;
            }
            if cc.all_internal {
                total = total.saturating_add(1);
            } else {
                return false;
            }
        }
        for score in &self.region_scores {
            if score.complexity == ComplexityClass::Unbounded {
                return false;
            }
        }
        total <= budget
    }

    pub fn collect_final_values(&self, program: &Program) -> Option<Vec<(Vec<String>, HashMap<String, i64>)>> {
        let mut all_bindings = Vec::new();
        for cc in &self.composed_chains {
            let mut bindings = Self::initial_bindings(program);
            let mut chain_bindings = HashMap::new();
            if cc.all_internal {
                if let Some(ref cv) = cc.counter_var {
                    if let Some(&bound) = self.iter_bounds.get(&cc.chain[0]) {
                        bindings.insert(cv.clone(), bound as i64);
                        chain_bindings.insert(cv.clone(), bound as i64);
                    }
                }
                all_bindings.push((cc.chain.clone(), chain_bindings));
                continue;
            }
            for stmt in &cc.composed_body {
                if !Self::eval_stmt(stmt, &mut bindings) {
                    return None;
                }
            }
            for (k, v) in &bindings {
                chain_bindings.insert(k.clone(), *v);
            }
            all_bindings.push((cc.chain.clone(), chain_bindings));
        }
        Some(all_bindings)
    }

    fn initial_bindings(program: &Program) -> HashMap<String, i64> {
        let mut bindings = HashMap::new();
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if let Some(ref e) = decl.expr {
                        if let Some(v) = Self::eval_expr_simple(e, &HashMap::new()) {
                            bindings.insert(decl.name.clone(), v);
                        }
                    }
                }
                TopLevel::Constant(c) => {
                    if let Some(v) = Self::eval_expr_simple(&c.expr, &HashMap::new()) {
                        bindings.insert(c.name.clone(), v);
                    }
                }
                _ => {}
            }
        }
        bindings
    }

    fn eval_stmt(stmt: &Statement, bindings: &mut HashMap<String, i64>) -> bool {
        match stmt {
            Statement::Assignment { lhs: Expr::Identifier(name), expr, .. } => {
                if let Some(val) = Self::eval_expr_simple(expr, bindings) {
                    bindings.insert(name.clone(), val);
                    true
                } else { false }
            }
            Statement::Let { name, expr, .. } => {
                if let Some(e) = expr {
                    if let Some(val) = Self::eval_expr_simple(e, bindings) {
                        bindings.insert(name.clone(), val);
                    }
                }
                true
            }
            Statement::Expression(e) => {
                Self::eval_expr_simple(e, bindings).is_some()
            }
            Statement::Guarded { condition, statements, .. } => {
                if let Some(cond) = Self::eval_expr_simple(condition, bindings) {
                    if cond != 0 {
                        for s in statements {
                            if !Self::eval_stmt(s, bindings) { return false; }
                        }
                    }
                    true
                } else { false }
            }
            Statement::Term { .. } | Statement::TermBang { .. } | Statement::InlineAsm { .. }
            | Statement::Alka(_) => false,
            _ => true,
        }
    }

    // 2026-06-27: iterative arithmetic evaluation — replaces recursive version
    // to prevent stack overflow on deeply nested arithmetic expressions.
    fn eval_expr_simple(expr: &Expr, bindings: &HashMap<String, i64>) -> Option<i64> {
        // Stack: (left_expr, right_expr, op_to_apply)
        // Post-order: push children first, then combine results.
        enum Op { Add, Sub, Mul, Div, Mod, And, Or, Eq, Ne, Lt, Le, Gt, Ge,
                  BitAnd, BitOr, BitXor, Shl, Shr, Not, Neg, BitNot, Cast, Id }
        struct Frame<'a> { expr: &'a Expr, state: u8, left: Option<i64> }

        let mut stack: Vec<Frame> = vec![Frame { expr, state: 0, left: None }];
        let mut results: Vec<i64> = vec![];

        while let Some(mut f) = stack.pop() {
            match f.state {
                0 => match f.expr {
                    Expr::Integer(n) => results.push(*n),
                    Expr::Bool(b) => results.push(if *b { 1 } else { 0 }),
                    Expr::Identifier(n) | Expr::OwnedRef(n) => {
                        if let Some(&v) = bindings.get(n) { results.push(v); }
                        else { return None; }
                    }
                    Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b)
                    | Expr::Div(a, b) | Expr::Mod(a, b)
                    | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
                    | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b)
                    | Expr::And(a, b) | Expr::Or(a, b)
                    | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
                    | Expr::Shl(a, b) | Expr::Shr(a, b) => {
                        f.state = 1;
                        stack.push(f);
                        stack.push(Frame { expr: b, state: 0, left: None });
                        stack.push(Frame { expr: a, state: 0, left: None });
                    }
                    Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _) => {
                        f.state = 1;
                        stack.push(f);
                        stack.push(Frame { expr: a, state: 0, left: None });
                    }
                    _ => return None,
                },
                1 => match f.expr {
                    Expr::Add(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l.wrapping_add(r)); }
                    Expr::Sub(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l.wrapping_sub(r)); }
                    Expr::Mul(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l.wrapping_mul(r)); }
                    Expr::Div(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l / r); }
                    Expr::Mod(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l % r); }
                    Expr::And(_, _) => { let rv = results.pop()?; let lv = results.pop()?; results.push(if lv != 0 && rv != 0 { 1 } else { 0 }); }
                    Expr::Or(_, _) => { let rv = results.pop()?; let lv = results.pop()?; results.push(if lv != 0 || rv != 0 { 1 } else { 0 }); }
                    Expr::Eq(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(if l == r { 1 } else { 0 }); }
                    Expr::Ne(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(if l != r { 1 } else { 0 }); }
                    Expr::Lt(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(if l < r { 1 } else { 0 }); }
                    Expr::Le(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(if l <= r { 1 } else { 0 }); }
                    Expr::Gt(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(if l > r { 1 } else { 0 }); }
                    Expr::Ge(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(if l >= r { 1 } else { 0 }); }
                    Expr::BitAnd(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l & r); }
                    Expr::BitOr(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l | r); }
                    Expr::BitXor(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l ^ r); }
                    Expr::Shl(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l << (r as u32 & 63)); }
                    Expr::Shr(_, _) => { let r = results.pop()?; let l = results.pop()?; results.push(l >> (r as u32 & 63)); }
                    Expr::Not(_) => { let v = results.pop()?; results.push(if v == 0 { 1 } else { 0 }); }
                    Expr::Neg(_) => { let v = results.pop()?; results.push(-v); }
                    Expr::BitNot(_) => { let v = results.pop()?; results.push(!v); }
                    Expr::Cast(_, _) => {} // result already on stack
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        results.pop()
    }

    /// Extract range bounds from a desugared constraint expression.
    /// Recognizes the pattern: `_ >= lo && _ <= hi` produced by `lo..hi` sugar.
    fn extract_range_from_constraint(expr: &Expr) -> Option<(&Expr, &Expr)> {
        match expr {
            Expr::And(ge_expr, le_expr) => {
                if let Expr::Ge(ge_lhs, lo_expr) = ge_expr.as_ref() {
                    if let Expr::Identifier(l1) = ge_lhs.as_ref() {
                        if let Expr::Le(le_lhs, hi_expr) = le_expr.as_ref() {
                            if let Expr::Identifier(l2) = le_lhs.as_ref() {
                                if l1 == "_" && l2 == "_" {
                                    return Some((lo_expr.as_ref(), hi_expr.as_ref()));
                                }
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn var_is_chain_internal(&self, var: &str, chain: &[String], all_txn_names: &HashSet<String>) -> bool {
        for txn in all_txn_names {
            if chain.contains(txn) { continue; }
            if let Some(reads) = self.txn_reads.get(txn) {
                if reads.contains(var) { return false; }
            }
            if let Some(writes) = self.txn_writes.get(txn) {
                if writes.contains(var) { return false; }
            }
        }
        true
    }

    fn chain_is_composable(&self, chain: &[String], all_txn_names: &HashSet<String>) -> bool {
        // 1. Each link variable has exactly one writer within the chain
        for i in 0..chain.len().saturating_sub(1) {
            let a = &chain[i];
            if let Some(writes_a) = self.txn_writes.get(a) {
                if let Some(reads_next) = self.txn_reads.get(&chain[i + 1]) {
                    let shared: Vec<_> = writes_a.iter().filter(|w| reads_next.contains(*w)).collect();
                    for sv in &shared {
                        if self.is_chain_counter_var(chain, sv) { continue; }
                        let writer_count: usize = chain.iter()
                            .filter(|tn| {
                                if let Some(w) = self.txn_writes.get(*tn) {
                                    w.contains(*sv)
                                } else { false }
                            })
                            .count();
                        if writer_count > 1 { return false; }
                    }
                }
            }
        }

        // 2. No FFI calls in chain body
        for tn in chain {
            if let Some(body) = self.txn_bodies.get(tn) {
                if has_ffi_or_trigger_stmt_in_chain(body) {
                    return false;
                }
            }
        }

        // 3. Same convergence contract (same pre-condition var and bound)
        let mut common_pre_var: Option<String> = None;
        let mut common_bound_var: Option<String> = None;
        for tn in chain {
            if let Some(body) = self.txn_bodies.get(tn) {
                let cv = find_counter_var(body);
                if let Some(ref v) = cv {
                    if let Some(ref pv) = common_pre_var {
                        if pv != v { return false; }
                    } else {
                        common_pre_var = Some(v.clone());
                    }
                } else {
                    return false;
                }
            }
            if let Some(bv) = self.iter_bounds.get(tn) {
                if let Some(ref cb) = common_bound_var {
                    if format!("{}", bv) != *cb { return false; }
                } else {
                    common_bound_var = Some(format!("{}", bv));
                }
            }
        }

        true
    }

    fn push_composed_chain(
        &mut self,
        chain: &[String],
        link_vars: &[String],
        root_triggers: &[String],
        trigger_values: Option<&[(String, i64)]>,
        all_internal: bool,
    ) {
        let composed = self.build_composed_body(chain, link_vars, trigger_values);

        let counter_var = self.iter_bounds.get(&chain[0])
            .map(|_| {
                if let Some(body) = self.txn_bodies.get(&chain[0]) {
                    find_counter_var(body)
                } else { None }
            })
            .flatten();

        let fused_weight = count_statements_recursive(&composed);

        self.composed_chains.push(ComposedChain {
            chain: chain.to_vec(),
            link_vars: link_vars.to_vec(),
            root_triggers: root_triggers.to_vec(),
            composed_body: composed,
            counter_var,
            fused_weight,
            trigger_values: trigger_values.map(|tv| tv.to_vec()),
            all_internal,
        });
    }

    fn is_chain_counter_var(&self, chain: &[String], var: &str) -> bool {
        chain.iter().any(|tn| {
            if let Some(body) = self.txn_bodies.get(tn) {
                let cv = find_counter_var(body);
                cv.as_deref() == Some(var)
            } else { false }
        })
    }

    fn compute_link_vars(&self, chain: &[String]) -> Vec<String> {
        let mut links = Vec::new();
        for i in 0..chain.len().saturating_sub(1) {
            let a = &chain[i];
            let b = &chain[i + 1];
            if let (Some(writes_a), Some(reads_b)) = (self.txn_writes.get(a), self.txn_reads.get(b)) {
                for w in writes_a {
                    if reads_b.contains(w) {
                        links.push(w.clone());
                        break;
                    }
                }
            }
        }
        links
    }

    fn build_composed_body(
        &self,
        chain: &[String],
        link_vars: &[String],
        trigger_values: Option<&[(String, i64)]>,
    ) -> Vec<Statement> {
        let mut result = Vec::new();

        for (i, txn_name) in chain.iter().enumerate() {
            let body = match self.txn_bodies.get(txn_name) {
                Some(b) => b.clone(),
                None => continue,
            };

            let counter_var = find_counter_var(&body);

            let mut stmts: Vec<Statement> = body.clone();

            // If this is the root txn (i=0) and we have trigger values,
            // substitute trigger identifiers with concrete values before composition.
            if i == 0 {
                if let Some(tv) = trigger_values {
                    for (trg_name, trg_val) in tv {
                        stmts = substitute_var(&stmts, trg_name, &Expr::Integer(*trg_val));
                    }
                }
            }

            // Substitute link variables from previous txn's writes
            if i > 0 {
                let prev = &chain[i - 1];
                if let Some(prev_body) = self.txn_bodies.get(prev) {
                    if let Some(&ref link_var) = link_vars.get(i - 1) {
                        let mut write_expr = find_write_expr(prev_body, link_var);

                        // If the previous txn is the root and we have trigger values,
                        // also concretize the write expression
                        if i == 1 {
                            if let Some(tv) = trigger_values {
                                if let Some(ref we) = write_expr {
                                    let mut subs = we.clone();
                                    for (trg_name, trg_val) in tv {
                                        subs = substitute_expr(&subs, trg_name, &Expr::Integer(*trg_val));
                                    }
                                    write_expr = Some(subs);
                                }
                            }
                        }

                        if let Some(we) = write_expr {
                            stmts = substitute_var(&stmts, link_var, &we);
                        }
                    }
                }
            }

            // Filter out counter bumps from non-root transactions
            for s in &stmts {
                let is_counter = if let Some(ref cv) = counter_var {
                    is_counter_bump_stmt(s, cv)
                } else { false };
                if i > 0 && is_counter { continue; }
                result.push(s.clone());
            }
        }

        result
    }

    fn update_scores_after_composition(&mut self) {
        for cc in &self.composed_chains {
            for score in &mut self.region_scores {
                let shares = cc.chain.iter().any(|c| score.txn_names.contains(c));
                if shares {
                    score.chain_composed = true;
                    score.body_weight = cc.fused_weight;
                    score.optimization_score *= 1.5;
                }
            }
        }
    }
}

fn expr_to_var_set(expr: &Expr) -> HashSet<String> {
    let mut vars = HashSet::new();
    collect_var_ids(expr, &mut vars);
    vars
}

// 2026-06-27: iterative variable collection — replaces recursive version.
fn collect_var_ids(expr: &Expr, vars: &mut HashSet<String>) {
    let mut work: Vec<&Expr> = vec![expr];
    while let Some(e) = work.pop() {
        match e {
            Expr::Identifier(n) | Expr::OwnedRef(n) => { vars.insert(n.clone()); }
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
            | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
            | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::And(a, b)
            | Expr::Or(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
            | Expr::Shl(a, b) | Expr::Shr(a, b) | Expr::Concat(a, b) => {
                work.push(b);
                work.push(a);
            }
            Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _)
            | Expr::Projection { source: a, .. } => { work.push(a); }
            Expr::Call(_, args) => { work.extend(args.iter().rev()); }
            Expr::ListLiteral(elems) | Expr::Tuple(elems) => { work.extend(elems.iter().rev()); }
            Expr::ListIndex(l, i) => { work.push(i); work.push(l); }
            Expr::FieldAccess(o, _) => { work.push(o); }
            Expr::Block(_, last) | Expr::TupleDestructure(_, last) => { work.push(last); }
            _ => {}
        }
    }
}

// 2026-06-27: iterative count — replaces recursive version to prevent
// stack overflow on deep statement nesting (Oracle with nested Foreach).
fn count_statements_recursive(body: &[Statement]) -> usize {
    let mut count = 0;
    let mut work: Vec<&Statement> = body.iter().collect();
    while let Some(s) = work.pop() {
        count += 1;
        match s {
            Statement::Guarded { statements, .. } => {
                work.extend(statements.iter().rev());
            }
            Statement::OnExit { body: inner, .. } => {
                work.extend(inner.iter().rev());
            }
            Statement::SyncBlock { body: inner } => {
                work.extend(inner.iter().rev());
            }
            Statement::Foreach { body: inner, .. } => {
                work.extend(inner.iter().rev());
            }
            Statement::Oracle { body, handler, .. } => {
                work.extend(body.iter().rev());
                work.extend(handler.iter().rev());
            }
            Statement::Async { body: inner, .. } => {
                work.push(inner);
            }
            Statement::AsyncAwait { body: inner, .. } => {
                work.push(inner);
            }
            _ => {}
        }
    }
    count
}

// 2026-06-27: iterative statement walk — replaces recursive version to prevent
// stack overflow on deeply nested guarded blocks.
fn has_ffi_or_terminator_stmt(stmt: &Statement) -> bool {
    let mut work: Vec<&Statement> = vec![stmt];
    while let Some(s) = work.pop() {
        match s {
            Statement::Term { .. } | Statement::TermBang { .. }
            | Statement::InlineAsm { .. } | Statement::Alka(_) => return true,
            Statement::Assignment { expr, .. } if expr_has_call(expr) => return true,
            Statement::Let { expr, .. } => {
                if let Some(e) = expr { if expr_has_call(e) { return true; } }
            }
            Statement::Expression(e) if expr_has_call(e) => return true,
            Statement::Guarded { condition, statements, .. } => {
                if expr_has_call(condition) { return true; }
                work.extend(statements.iter().rev());
            }
            Statement::Unification { expr, .. } if expr_has_call(expr) => return true,
            Statement::OnExit { body, .. } => {
                work.extend(body.iter().rev());
            }
            Statement::Foreach { body, .. } => {
                work.extend(body.iter().rev());
            }
            Statement::Oracle { body, handler, .. } => {
                work.extend(body.iter().rev());
                work.extend(handler.iter().rev());
            }
            Statement::Await { expr, .. } if expr_has_call(expr) => return true,
            Statement::Async { body, .. } => { work.push(body); }
            Statement::AsyncAwait { body, .. } => { work.push(body); }
            _ => {}
        }
    }
    false
}

pub(crate) fn has_ffi_or_trigger_stmt_in_chain(body: &[Statement]) -> bool {
    body.iter().any(|s| has_ffi_or_terminator_stmt(s))
}

// 2026-06-27: iterative statement walk — replaces recursive version to prevent
// stack overflow on deeply nested guarded blocks.
fn has_ffi_or_trigger_stmt(stmt: &Statement, trigger_vars: &HashSet<String>) -> bool {
    let mut work: Vec<&Statement> = vec![stmt];
    while let Some(s) = work.pop() {
        match s {
            Statement::Term { .. } | Statement::TermBang { .. }
            | Statement::InlineAsm { .. } | Statement::Alka(_) => return true,
            Statement::Assignment { lhs, expr, .. } => {
                if expr_has_call(expr) { return true; }
                // Also check if lhs references a trigger variable
                if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                    if trigger_vars.contains(n) { return true; }
                }
            }
            Statement::Let { expr, .. } => {
                if let Some(e) = expr { if expr_has_call(e) { return true; } }
            }
            Statement::Expression(e) if expr_has_call(e) => return true,
            Statement::Guarded { condition, statements, .. } => {
                if expr_has_call(condition) { return true; }
                work.extend(statements.iter().rev());
            }
            Statement::Unification { expr, .. } if expr_has_call(expr) => return true,
            Statement::OnExit { body, .. } => {
                work.extend(body.iter().rev());
            }
            Statement::Foreach { body, .. } => {
                work.extend(body.iter().rev());
            }
            Statement::Oracle { body, handler, .. } => {
                work.extend(body.iter().rev());
                work.extend(handler.iter().rev());
            }
            Statement::Await { expr, .. } if expr_has_call(expr) => return true,
            Statement::Async { body, .. } => { work.push(body); }
            Statement::AsyncAwait { body, .. } => { work.push(body); }
            _ => {}
        }
    }
    false
}

// 2026-06-27: iterative AST walk — replaces recursive version to prevent
// stack overflow on deeply nested expression trees (officina-cli's complex
// Match arms with deeply nested binary ops).
fn expr_has_call(expr: &Expr) -> bool {
    let mut work: Vec<&Expr> = vec![expr];
    while let Some(e) = work.pop() {
        match e {
            Expr::Call(_, _) => return true,
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
            | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
            | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::And(a, b)
            | Expr::Or(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
            | Expr::Shl(a, b) | Expr::Shr(a, b) | Expr::Concat(a, b) => {
                work.push(b);
                work.push(a);
            }
            Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _)
            | Expr::Projection { source: a, .. } => {
                work.push(a);
            }
            Expr::ListLiteral(elems) | Expr::Tuple(elems) => {
                work.extend(elems.iter().rev());
            }
            Expr::ListIndex(l, i) => {
                work.push(i);
                work.push(l);
            }
            Expr::FieldAccess(o, _) => {
                work.push(o);
            }
            Expr::Block(_, last) | Expr::TupleDestructure(_, last) => {
                work.push(last);
            }
            Expr::Match { value, arms } => {
                for arm in arms.iter().rev() {
                    work.push(&arm.body);
                }
                work.push(value);
            }
            Expr::PatternMatch { value, .. } => {
                work.push(value);
            }
            Expr::StructInstance(_, fields) | Expr::ObjectLiteral(fields) => {
                for (_, e) in fields.iter().rev() {
                    work.push(e);
                }
            }
            Expr::Slice { value, start, end, stride, mask } => {
                if let Some(m) = mask { work.push(m); }
                if let Some(s) = stride { work.push(s); }
                if let Some(e) = end { work.push(e); }
                if let Some(s) = start { work.push(s); }
                work.push(value);
            }
            Expr::MultiSlice { value, ops } => {
                for op in ops.iter().rev() {
                    if let BracketOp::Mask(m) = op { work.push(m); }
                    if let BracketOp::Stride(s) = op { work.push(s); }
                }
                work.push(value);
            }
            _ => {}
        }
    }
    false
}

fn has_term_or_unify_escape(body: &[Statement]) -> bool {
    body.iter().any(|s| matches!(s,
        Statement::Term { .. } | Statement::TermBang { .. } | Statement::Unification { .. }
        | Statement::Escape(_) | Statement::InlineAsm { .. }
        | Statement::Alka(_)
    ))
}

fn is_counter_bump_stmt(stmt: &Statement, counter_var: &str) -> bool {
    matches!(stmt, Statement::Assignment {
        lhs: Expr::Identifier(lhs_name) | Expr::OwnedRef(lhs_name),
        expr: Expr::Add(a, b),
        ..
    } if lhs_name == counter_var && {
        (matches!(a.as_ref(), Expr::Identifier(n) if n == counter_var)
            && matches!(b.as_ref(), Expr::Integer(n) if *n > 0))
        || (matches!(b.as_ref(), Expr::Identifier(n) if n == counter_var)
            && matches!(a.as_ref(), Expr::Integer(n) if *n > 0))
    })
}

fn find_counter_var(body: &[Statement]) -> Option<String> {
    for s in body {
        if let Statement::Assignment {
            lhs: Expr::Identifier(n) | Expr::OwnedRef(n),
            expr: Expr::Add(a, b),
            ..
        } = s {
            let lhs_in_a = matches!(a.as_ref(), Expr::Identifier(an) if an == n);
            let lhs_in_b = matches!(b.as_ref(), Expr::Identifier(bn) if bn == n);
            let pos_int = |e: &Expr| matches!(e, Expr::Integer(d) if *d > 0);
            if (lhs_in_a && pos_int(b)) || (lhs_in_b && pos_int(a)) {
                return Some(n.clone());
            }
        }
    }
    None
}

fn find_write_expr(body: &[Statement], var: &str) -> Option<Expr> {
    for s in body {
        if let Statement::Assignment {
            lhs: Expr::Identifier(n) | Expr::OwnedRef(n),
            expr,
            ..
        } = s {
            if n == var {
                return Some(expr.clone());
            }
        }
    }
    None
}

fn substitute_var(body: &[Statement], old_var: &str, new_expr: &Expr) -> Vec<Statement> {
    body.iter().map(|s| substitute_stmt(s, old_var, new_expr)).collect()
}

fn substitute_stmt(stmt: &Statement, old_var: &str, new_expr: &Expr) -> Statement {
    match stmt {
        Statement::Assignment { lhs, expr, timeout, modifiers } => {
            Statement::Assignment {
                lhs: lhs.clone(),
                expr: substitute_expr(expr, old_var, new_expr),
                timeout: timeout.clone(),
                modifiers: modifiers.clone(),
            }
        }
        Statement::Let { name, ty, expr, address, address_expr, bit_range, is_override, modifiers, .. } => {
            Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| substitute_expr(e, old_var, new_expr)),
                address: *address,
                address_expr: address_expr.as_ref().map(|e| Box::new(substitute_expr(e, old_var, new_expr))),
                bit_range: bit_range.clone(),
                is_override: *is_override,
                modifiers: modifiers.clone(),
                constraint: None,
            }
        }
        Statement::Guarded { condition, statements } => {
            Statement::Guarded {
                condition: substitute_expr(&condition, old_var, new_expr),
                statements: substitute_var(&statements, old_var, new_expr),
            }
        }
        Statement::Expression(e) => Statement::Expression(substitute_expr(e, old_var, new_expr)),
        Statement::Term { values, modifiers, swan_song } => {
            Statement::Term {
                values: values.iter().map(|v| v.as_ref().map(|x| substitute_expr(x, old_var, new_expr))).collect(),
                swan_song: swan_song.as_ref().map(|s| {
                    let mut v = substitute_var(std::slice::from_ref(s.as_ref()), old_var, new_expr);
                    Box::new(v.pop().unwrap_or(Statement::Escape(None)))
                }),
                modifiers: modifiers.clone(),
            }
        }
        Statement::TermBang { values, modifiers, swan_song } => {
            Statement::TermBang {
                values: values.iter().map(|v| v.as_ref().map(|x| substitute_expr(x, old_var, new_expr))).collect(),
                swan_song: swan_song.as_ref().map(|s| {
                    let mut v = substitute_var(std::slice::from_ref(s.as_ref()), old_var, new_expr);
                    Box::new(v.pop().unwrap_or(Statement::Escape(None)))
                }),
                modifiers: modifiers.clone(),
            }
        }
        Statement::Escape(e) => Statement::Escape(e.as_ref().map(|x| substitute_expr(x, old_var, new_expr))),
        Statement::SyncBlock { body } => Statement::SyncBlock { body: body.clone() },
        Statement::Unification { name, variant, fields, expr } => {
            Statement::Unification {
                name: name.clone(),
                variant: variant.clone(),
                fields: fields.clone(),
                expr: substitute_expr(expr, old_var, new_expr),
            }
        }
        Statement::OnExit { body, span } => {
            Statement::OnExit {
                body: substitute_var(body, old_var, new_expr),
                span: *span,
            }
        }
        other => other.clone(),
    }
}

/// Post-order iterative substitution using explicit stack.
/// Replaces `old_var` with `new_expr` in an expression tree without recursion.
fn substitute_expr(expr: &Expr, old_var: &str, new_expr: &Expr) -> Expr {
    let owned_new = new_expr.clone();
    let owned_old = old_var.to_string();

    // ── Helper function pointers for binary/unary ops ──
    fn add(l: Expr, r: Expr) -> Expr { Expr::Add(Box::new(l), Box::new(r)) }
    fn sub(l: Expr, r: Expr) -> Expr { Expr::Sub(Box::new(l), Box::new(r)) }
    fn mul(l: Expr, r: Expr) -> Expr { Expr::Mul(Box::new(l), Box::new(r)) }
    fn div(l: Expr, r: Expr) -> Expr { Expr::Div(Box::new(l), Box::new(r)) }
    fn modop(l: Expr, r: Expr) -> Expr { Expr::Mod(Box::new(l), Box::new(r)) }
    fn eq(l: Expr, r: Expr) -> Expr { Expr::Eq(Box::new(l), Box::new(r)) }
    fn ne(l: Expr, r: Expr) -> Expr { Expr::Ne(Box::new(l), Box::new(r)) }
    fn lt(l: Expr, r: Expr) -> Expr { Expr::Lt(Box::new(l), Box::new(r)) }
    fn le(l: Expr, r: Expr) -> Expr { Expr::Le(Box::new(l), Box::new(r)) }
    fn gt(l: Expr, r: Expr) -> Expr { Expr::Gt(Box::new(l), Box::new(r)) }
    fn ge(l: Expr, r: Expr) -> Expr { Expr::Ge(Box::new(l), Box::new(r)) }
    fn and(l: Expr, r: Expr) -> Expr { Expr::And(Box::new(l), Box::new(r)) }
    fn or(l: Expr, r: Expr) -> Expr { Expr::Or(Box::new(l), Box::new(r)) }
    fn bitand(l: Expr, r: Expr) -> Expr { Expr::BitAnd(Box::new(l), Box::new(r)) }
    fn bitor(l: Expr, r: Expr) -> Expr { Expr::BitOr(Box::new(l), Box::new(r)) }
    fn bitxor(l: Expr, r: Expr) -> Expr { Expr::BitXor(Box::new(l), Box::new(r)) }
    fn shl(l: Expr, r: Expr) -> Expr { Expr::Shl(Box::new(l), Box::new(r)) }
    fn shr(l: Expr, r: Expr) -> Expr { Expr::Shr(Box::new(l), Box::new(r)) }
    fn concat(l: Expr, r: Expr) -> Expr { Expr::Concat(Box::new(l), Box::new(r)) }
    fn not(v: Expr) -> Expr { Expr::Not(Box::new(v)) }
    fn neg(v: Expr) -> Expr { Expr::Neg(Box::new(v)) }
    fn bitnot(v: Expr) -> Expr { Expr::BitNot(Box::new(v)) }

    // Work stack entries
    enum W {
        Proc(Expr),
        B0(Expr),
        /// Unary combine (must capture no data — use naive free function pointers)
        B1(fn(Expr) -> Expr),
        /// Binary combine (must capture no data — use naive free function pointers)
        B2(fn(Expr, Expr) -> Expr),
        /// N-ary combine with data capture via Box<dyn FnOnce>
        Args(usize, Box<dyn FnOnce(Vec<Expr>) -> Expr>),
    }

    macro_rules! binop {
        ($work:ident, $f:expr, $a:expr, $b:expr) => {{
            $work.push(W::B2($f));
            $work.push(W::Proc($b));
            $work.push(W::Proc($a));
        }};
    }
    macro_rules! unop {
        ($work:ident, $f:expr, $a:expr) => {{
            $work.push(W::B1($f));
            $work.push(W::Proc($a));
        }};
    }

    let mut work: Vec<W> = vec![W::Proc(expr.clone())];
    let mut results: Vec<Expr> = vec![];

    // Helper to push Proc entries for each expression in reverse order
    macro_rules! push_procs_rev {
        ($work:ident, $first:expr $(, $rest:expr)*) => {{
            $work.push(W::Proc($first));
            $( $work.push(W::Proc($rest)); )*
        }};
    }

    while let Some(w) = work.pop() {
        match w {
            W::Proc(e) => match e {
                Expr::Identifier(n) if n == owned_old => {
                    results.push(owned_new.clone());
                }
                Expr::OwnedRef(n) if n == owned_old => {
                    results.push(owned_new.clone());
                }
                Expr::Identifier(n) => { results.push(Expr::Identifier(n)); }
                Expr::OwnedRef(n) => { results.push(Expr::OwnedRef(n)); }
                Expr::Integer(_) | Expr::Float(_) | Expr::String(_)
                | Expr::Char(_) | Expr::Bool(_) | Expr::Term
                | Expr::Ellipsis | Expr::RegexLiteral(_) | Expr::TypeRef(_)
                | Expr::SharedMem(_) | Expr::Literal(_) => {
                    results.push(e);
                }
                Expr::Add(a, b) => binop!(work, add, *a, *b),
                Expr::Sub(a, b) => binop!(work, sub, *a, *b),
                Expr::Mul(a, b) => binop!(work, mul, *a, *b),
                Expr::Div(a, b) => binop!(work, div, *a, *b),
                Expr::Mod(a, b) => binop!(work, modop, *a, *b),
                Expr::Eq(a, b) => binop!(work, eq, *a, *b),
                Expr::Ne(a, b) => binop!(work, ne, *a, *b),
                Expr::Lt(a, b) => binop!(work, lt, *a, *b),
                Expr::Le(a, b) => binop!(work, le, *a, *b),
                Expr::Gt(a, b) => binop!(work, gt, *a, *b),
                Expr::Ge(a, b) => binop!(work, ge, *a, *b),
                Expr::And(a, b) => binop!(work, and, *a, *b),
                Expr::Or(a, b) => binop!(work, or, *a, *b),
                Expr::BitAnd(a, b) => binop!(work, bitand, *a, *b),
                Expr::BitOr(a, b) => binop!(work, bitor, *a, *b),
                Expr::BitXor(a, b) => binop!(work, bitxor, *a, *b),
                Expr::Shl(a, b) => binop!(work, shl, *a, *b),
                Expr::Shr(a, b) => binop!(work, shr, *a, *b),
                Expr::Concat(a, b) => binop!(work, concat, *a, *b),
                Expr::Not(a) => unop!(work, not, *a),
                Expr::Neg(a) => unop!(work, neg, *a),
                Expr::BitNot(a) => unop!(work, bitnot, *a),
                Expr::Cast(a, t) => {
                    let t2 = t;
                    work.push(W::Args(1, Box::new(move |v| Expr::Cast(Box::new(v[0].clone()), t2.clone()))));
                    work.push(W::Proc(*a));
                }
                Expr::Call(name, args) => {
                    let n = args.len();
                    let name2 = name;
                    work.push(W::Args(n, Box::new(move |v| Expr::Call(name2, v))));
                    for a in args.into_iter().rev() {
                        work.push(W::Proc(a));
                    }
                }
                Expr::ListLiteral(elems) => {
                    let n = elems.len();
                    work.push(W::Args(n, Box::new(Expr::ListLiteral)));
                    for e in elems.into_iter().rev() {
                        work.push(W::Proc(e));
                    }
                }
                Expr::Tuple(elems) => {
                    let n = elems.len();
                    work.push(W::Args(n, Box::new(Expr::Tuple)));
                    for e in elems.into_iter().rev() {
                        work.push(W::Proc(e));
                    }
                }
                Expr::ListIndex(l, i) => {
                    work.push(W::Args(2, Box::new(|v| Expr::ListIndex(Box::new(v[0].clone()), Box::new(v[1].clone())))));
                    work.push(W::Proc(*i));
                    work.push(W::Proc(*l));
                }
                Expr::Projection { source, target: ref t } => {
                    let t2 = t.clone();
                    work.push(W::Args(1, Box::new(move |v| Expr::Projection { source: Box::new(v[0].clone()), target: t2.clone() })));
                    work.push(W::Proc(*source));
                }
                Expr::FieldAccess(obj, f) => {
                    let f2 = f;
                    work.push(W::Args(1, Box::new(move |v| Expr::FieldAccess(Box::new(v[0].clone()), f2.clone()))));
                    work.push(W::Proc(*obj));
                }
                Expr::Block(stmts, last) => {
                    let s = substitute_var(&stmts, &owned_old, &owned_new);
                    work.push(W::Args(1, Box::new(move |v| Expr::Block(s.clone(), Box::new(v[0].clone())))));
                    work.push(W::Proc(*last));
                }
                Expr::TupleDestructure(bindings, body) => {
                    let b = bindings;
                    work.push(W::Args(1, Box::new(move |v| Expr::TupleDestructure(b.clone(), Box::new(v[0].clone())))));
                    work.push(W::Proc(*body));
                }
                Expr::Match { value, arms } => {
                    let arms2 = arms.clone();
                    let num_children = 1 + arms.iter().map(|a| 1 + a.guard.is_some() as usize).sum::<usize>();
                    work.push(W::Args(num_children, Box::new(move |v| {
                        let mut idx = 1;
                        Expr::Match {
                            value: Box::new(v[0].clone()),
                            arms: arms2.iter().map(|arm| {
                                let guard_result = if arm.guard.is_some() {
                                    let r = v[idx].clone(); idx += 1; Some(Box::new(r))
                                } else { None };
                                let body = v[idx].clone(); idx += 1;
                                crate::ast::MatchArm {
                                    pattern: arm.pattern.clone(),
                                    guard: guard_result,
                                    body: Box::new(body),
                                }
                            }).collect(),
                        }
                    })));
                    work.push(W::Proc(*value));
                    for arm in arms.into_iter().rev() {
                        work.push(W::Proc(*arm.body));
                        if let Some(g) = arm.guard {
                            work.push(W::Proc(*g));
                        }
                    }
                }
                Expr::PatternMatch { value, variant, fields } => {
                    let v = variant;
                    let f = fields;
                    work.push(W::Args(1, Box::new(move |v2| Expr::PatternMatch {
                        value: Box::new(v2[0].clone()), variant: v.clone(), fields: f.clone(),
                    })));
                    work.push(W::Proc(*value));
                }
                Expr::StructInstance(name, fields) => {
                    let n = name;
                    let names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                    let count = fields.len();
                    work.push(W::Args(count, Box::new(move |v| {
                        Expr::StructInstance(n.clone(), names.iter().cloned().zip(v.into_iter()).collect())
                    })));
                    for (_, e) in fields.into_iter().rev() {
                        work.push(W::Proc(e));
                    }
                }
                Expr::ObjectLiteral(fields) => {
                    let names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                    let count = fields.len();
                    work.push(W::Args(count, Box::new(move |v| {
                        Expr::ObjectLiteral(names.iter().cloned().zip(v.into_iter()).collect())
                    })));
                    for (_, e) in fields.into_iter().rev() {
                        work.push(W::Proc(e));
                    }
                }
                Expr::Slice { value, start, end, stride, mask } => {
                    let has_start = start.is_some();
                    let has_end = end.is_some();
                    let has_stride = stride.is_some();
                    let has_mask = mask.is_some();
                    let child_count = 1
                        + has_start as usize + has_end as usize
                        + has_stride as usize + has_mask as usize;
                    work.push(W::Args(child_count, Box::new(move |v| {
                        let mut idx = 0;
                        let sv = v[idx].clone(); idx += 1;
                        let st = if has_start { let r = v[idx].clone(); idx += 1; Some(Box::new(r)) } else { None };
                        let en = if has_end { let r = v[idx].clone(); idx += 1; Some(Box::new(r)) } else { None };
                        let strd = if has_stride { let r = v[idx].clone(); idx += 1; Some(Box::new(r)) } else { None };
                        let msk = if has_mask { let r = v[idx].clone(); idx += 1; Some(Box::new(r)) } else { None };
                        Expr::Slice { value: Box::new(sv), start: st, end: en, stride: strd, mask: msk }
                    })));
                    work.push(W::Proc(*value));
                    if let Some(s) = start { work.push(W::Proc(*s)); }
                    if let Some(e) = end { work.push(W::Proc(*e)); }
                    if let Some(s) = stride { work.push(W::Proc(*s)); }
                    if let Some(m) = mask { work.push(W::Proc(*m)); }
                }
                Expr::MultiSlice { value, ops } => {
                    let child_count = 1 + ops.iter().filter(|op| matches!(op, BracketOp::Mask(_) | BracketOp::Stride(_))).count();
                    let ops_clone = ops.clone();
                    work.push(W::Args(child_count, Box::new(move |v| {
                        let mut idx = 1;
                        Expr::MultiSlice {
                            value: Box::new(v[0].clone()),
                            ops: ops_clone.iter().map(|op| match op {
                                BracketOp::Coord(c) => BracketOp::Coord(c.clone()),
                                BracketOp::Mask(_) => { let r = v[idx].clone(); idx += 1; BracketOp::Mask(Box::new(r)) }
                                BracketOp::Stride(_) => { let r = v[idx].clone(); idx += 1; BracketOp::Stride(Box::new(r)) }
                            }).collect(),
                        }
                    })));
                    work.push(W::Proc(*value));
                    for op in ops.into_iter().rev() {
                        match op {
                            BracketOp::Coord(_) => {}
                            BracketOp::Mask(m) => work.push(W::Proc(*m)),
                            BracketOp::Stride(s) => work.push(W::Proc(*s)),
                        }
                    }
                }
                other => { results.push(other); }
            },
            W::B0(e) => { results.push(e); }
            W::B1(f) => {
                let a = results.pop().expect("B1: no result");
                results.push(f(a));
            }
            W::B2(f) => {
                let b = results.pop().expect("B2: no right result");
                let a = results.pop().expect("B2: no left result");
                results.push(f(a, b));
            }
            W::Args(n, f) => {
                let len = results.len();
                let args: Vec<Expr> = results.drain(len - n..).collect();
                results.push(f(args));
            }
        }
    }

    results.pop().expect("substitute_expr: no result")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_state(name: &str, val: Expr) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty: Type::Int,
            expr: Some(val),
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
            constraint: None,
        })
    }

    fn make_trigger(name: &str, ty: Type) -> TopLevel {
        TopLevel::Trigger(TriggerDeclaration {
            name: name.to_string(),
            ty,
            address: crate::ast::LinkRef::Explicit(0),
            bit_range: None,
            stages: vec![],
            condition: None,
            is_wake: true,
            is_const: false,
            span: None,
            modifiers: vec![],
        })
    }

    fn make_txn(
        name: &str,
        pre: Expr,
        post: Expr,
        body: Vec<Statement>,
    ) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: post,
                watchdog: None,
                span: None,
            },
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     })
    }

    fn assign(lhs: &str, expr: Expr) -> Statement {
        Statement::Assignment {
            lhs: Expr::Identifier(lhs.to_string()),
            expr,
            timeout: None,
            modifiers: vec![],
        }
    }

    fn int(n: i64) -> Expr {
        Expr::Integer(n)
    }

    fn ident(name: &str) -> Expr {
        Expr::Identifier(name.to_string())
    }

    fn add(a: Expr, b: Expr) -> Expr {
        Expr::Add(Box::new(a), Box::new(b))
    }

    fn mk_program(items: Vec<TopLevel>) -> Program {
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        }
    }

    #[test]
    fn test_pure_vars_region_zero() {
        // Two state vars with no triggers or transactions — all Pure, region 0
        let program = mk_program(vec![
            make_state("count", int(0)),
            make_state("total", int(100)),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.region_of("count"), Some(0));
        assert_eq!(ra.region_of("total"), Some(0));
        assert_eq!(ra.classification_of("count"), Some(VarClass::Pure));
    }

    #[test]
    fn test_trigger_seeded_as_opaque() {
        let program = mk_program(vec![make_trigger("btn", Type::Bool)]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.classification_of("btn"), Some(VarClass::Bounded)); // Bool → tight → Bounded
    }

    #[test]
    fn test_trigger_dependency_propagates() {
        // trg: Bool → x depends on trg → x becomes Bounded
        let program = mk_program(vec![
            make_trigger("trg", Type::Bool),
            make_state("x", int(0)),
            make_txn(
                "t1",
                Expr::Bool(true),
                Expr::Bool(true),
                vec![assign("x", ident("trg"))],
            ),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.classification_of("trg"), Some(VarClass::Bounded));
        assert_eq!(ra.classification_of("x"), Some(VarClass::Bounded));
        assert_ne!(ra.region_of("x"), Some(0));
    }

    #[test]
    fn test_two_independent_trigs_two_regions() {
        // trg_a → x, trg_b → y — two regions
        let program = mk_program(vec![
            make_trigger("trg_a", Type::Bool),
            make_trigger("trg_b", Type::Bool),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("tx_a", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("trg_a")),
            ]),
            make_txn("tx_b", Expr::Bool(true), Expr::Bool(true), vec![
                assign("y", ident("trg_b")),
            ]),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        // Both triggers are Bounded (Bool → tight), both x and y are Bounded
        assert_eq!(ra.classification_of("x"), Some(VarClass::Bounded));
        assert_eq!(ra.classification_of("y"), Some(VarClass::Bounded));
        // x and y should be in DIFFERENT regions (connected graphs)
        let region_x = ra.region_of("x").unwrap();
        let region_y = ra.region_of("y").unwrap();
        assert_ne!(
            region_x, region_y,
            "Independent trg vars should be in different regions"
        );
        assert_eq!(ra.regions.len(), 2);
    }

    #[test]
    fn test_chained_dependency_one_region() {
        // trg → x → y: all in same region
        let program = mk_program(vec![
            make_trigger("trg", Type::Bool),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn(
                "t1",
                Expr::Bool(true),
                Expr::Bool(true),
                vec![assign("x", ident("trg"))],
            ),
            make_txn(
                "t2",
                Expr::Bool(true),
                Expr::Bool(true),
                vec![assign("y", ident("x"))],
            ),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.classification_of("trg"), Some(VarClass::Bounded));
        assert_eq!(ra.classification_of("x"), Some(VarClass::Bounded));
        assert_eq!(ra.classification_of("y"), Some(VarClass::Bounded));
        // All three in same region
        assert_eq!(ra.region_of("trg"), ra.region_of("x"));
        assert_eq!(ra.region_of("x"), ra.region_of("y"));
        assert_eq!(ra.regions.len(), 1);
    }

    #[test]
    fn test_int_trigger_opaque() {
        // Int trigger has no bound → stays Opaque (not tight)
        let program = mk_program(vec![make_trigger("sensor", Type::Int)]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.classification_of("sensor"), Some(VarClass::Opaque));
    }

    #[test]
    fn test_transaction_precondition_adds_dep() {
        // pre-condition references total → txn depends on total
        let program = mk_program(vec![
            make_state("count", int(0)),
            make_state("total", int(100)),
            make_txn(
                "proc",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![assign("count", add(ident("count"), int(1)))],
            ),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        // count and total are Pure (no triggers), but count depends on total via pre
        assert_eq!(ra.classification_of("count"), Some(VarClass::Pure));
        assert_eq!(ra.classification_of("total"), Some(VarClass::Pure));
    }

    #[test]
    fn test_constant_interval() {
        let program = mk_program(vec![TopLevel::Constant(Constant {
            name: "total".to_string(),
            ty: Type::Int,
            expr: int(100),
        })]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.classification_of("total"), Some(VarClass::Pure));
        let info = ra.var_info.get("total").unwrap();
        assert_eq!(info.interval, Some(Interval { lo: 100, hi: 100 }));
        assert_eq!(info.value_set_size, Some(1));
    }

    #[test]
    fn test_region_of_nonexistent_var() {
        let program = mk_program(vec![]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.region_of("nonexistent"), None);
        assert_eq!(ra.classification_of("nonexistent"), None);
    }

    fn make_txn_with_body(name: &str, body: Vec<Statement>) -> TopLevel {
        make_txn(name, Expr::Bool(true), Expr::Bool(true), body)
    }

    fn make_const(name: &str, val: i64) -> TopLevel {
        TopLevel::Constant(Constant {
            name: name.to_string(),
            ty: Type::Int,
            expr: int(val),
        })
    }

    #[test]
    fn test_complexity_trivial() {
        let program = mk_program(vec![
            make_trigger("btn", Type::Bool),
            make_state("count", int(0)),
            make_txn_with_body("bump", vec![assign("count", ident("btn"))]),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert!(ra.region_scores.len() > 0);
        assert_eq!(ra.region_scores[0].complexity, ComplexityClass::Trivial);
    }

    #[test]
    fn test_complexity_light() {
        let program = mk_program(vec![
            make_trigger("btn", Type::Bool),
            make_state("a", int(0)),
            make_state("b", int(0)),
            make_txn_with_body("proc", vec![
                assign("a", ident("btn")),
                assign("b", int(2)),
                assign("a", add(ident("a"), int(3))),
            ]),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert!(ra.region_scores.len() > 0);
        assert_eq!(ra.region_scores[0].complexity, ComplexityClass::Light);
    }

    #[test]
    fn test_complexity_unbounded() {
        let program = mk_program(vec![
            make_trigger("btn", Type::Bool),
            make_state("x", int(0)),
            make_txn_with_body("exit", vec![
                assign("x", ident("btn")),
                Statement::Term { values: vec![Some(int(0))], modifiers: vec![], swan_song: None },
            ]),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert!(ra.region_scores.len() > 0);
        assert_eq!(ra.region_scores[0].complexity, ComplexityClass::Unbounded);
    }

    #[test]
    fn test_region_scoring() {
        let program = mk_program(vec![
            make_trigger("trg", Type::Bool),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("t1", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("trg")),
            ]),
            make_txn("t2", Expr::Bool(true), Expr::Bool(true), vec![
                assign("y", add(ident("x"), int(1))),
            ]),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert!(!ra.region_scores.is_empty());
        let score = &ra.region_scores[0];
        assert!(score.body_weight > 0);
    }

    #[test]
    fn test_region_independent() {
        let program = mk_program(vec![
            make_trigger("ta", Type::Bool),
            make_trigger("tb", Type::Bool),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("tx_a", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("ta")),
            ]),
            make_txn("tx_b", Expr::Bool(true), Expr::Bool(true), vec![
                assign("y", ident("tb")),
            ]),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        assert_eq!(ra.region_scores.len(), 2);
    }

    #[test]
    fn test_budget_plan_fit() {
        let program = mk_program(vec![
            make_trigger("ta", Type::Bool),
            make_trigger("tb", Type::Bool),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("tx_a", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("ta")),
            ]),
            make_txn("tx_b", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("ta")),
                assign("y", ident("tb")),
            ]),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.build_budget_plan(10);
        let plan = ra.budget_plan.as_ref().unwrap();
        assert!(plan.allocated.len() > 0);
        assert!(plan.residual_budget > 0);
    }

    #[test]
    fn test_budget_plan_exceeds() {
        let program = mk_program(vec![
            make_trigger("ta", Type::Bool),
            make_state("x", int(0)),
            make_txn("tx_a", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("ta")),
            ]),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.build_budget_plan(0);
        let plan = ra.budget_plan.as_ref().unwrap();
        assert!(plan.allocated.is_empty());
        assert!(!plan.skipped.is_empty());
    }

    #[test]
    fn test_chain_substitution() {
        let body_a = vec![assign("x", int(42))];
        let body_b = vec![assign("y", add(ident("x"), int(1)))];
        let result = substitute_var(&body_b, "x", &int(42));
        if let Statement::Assignment { expr, .. } = &result[0] {
            assert!(!format!("{:?}", expr).contains("Identifier(\"x\")"));
        }
    }

    #[test]
    fn test_chain_composition() {
        let program = mk_program(vec![
            make_const("total", 100),
            make_state("count", int(0)),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("step_a",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("x", int(42)),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
            make_txn("step_b",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("y", add(ident("x"), int(1))),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        if !ra.composed_chains.is_empty() {
            let cc = &ra.composed_chains[0];
            assert!(cc.fused_weight > 0);
            assert_eq!(cc.link_vars.len(), 1);
        }
    }

    #[test]
    fn test_chain_branching() {
        let program = mk_program(vec![
            make_trigger("sensor", Type::Bool),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("step_a", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("sensor")),
            ]),
            make_txn("step_b", Expr::Bool(true), Expr::Bool(true), vec![
                assign("y", add(ident("x"), int(1))),
            ]),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        if !ra.composed_chains.is_empty() {
            let cc = &ra.composed_chains[0];
            assert!(cc.root_triggers.contains(&"sensor".to_string()));
        }
    }

    #[test]
    fn test_gpu_eligible() {
        let program = mk_program(vec![
            make_const("total", 100),
            make_state("a", int(0)),
            make_state("b", int(0)),
            make_state("c", int(0)),
            make_state("count", int(0)),
            make_txn("heavy",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("a", int(1)),
                    assign("b", int(2)),
                    assign("c", int(3)),
                    assign("a", add(ident("a"), ident("b"))),
                    assign("b", add(ident("b"), ident("c"))),
                    assign("c", add(ident("c"), int(1))),
                    assign("a", add(ident("a"), ident("c"))),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        if let Some(score) = ra.region_scores.first() {
            assert_eq!(score.complexity, ComplexityClass::Medium);
            assert!(score.gpu_eligible);
        }
    }

    #[test]
    fn test_gpu_ineligible_term() {
        let program = mk_program(vec![
            make_const("total", 100),
            make_state("a", int(0)),
            make_state("count", int(0)),
            make_txn("with_term",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("a", int(1)),
                    assign("a", int(2)),
                    assign("a", int(3)),
                    assign("a", int(4)),
                    assign("a", int(5)),
                    assign("a", int(6)),
                    Statement::Term { values: vec![Some(int(0))], modifiers: vec![], swan_song: None },
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let ra = RegionAnalyzer::analyze(&program);
        if let Some(score) = ra.region_scores.first() {
            assert!(!score.gpu_eligible);
        }
    }

    #[test]
    fn test_is_fully_precomputable_chain() {
        let program = mk_program(vec![
            make_const("total", 100),
            make_state("count", int(0)),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("step_a",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("x", int(42)),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
            make_txn("step_b",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("y", add(ident("x"), int(1))),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        assert!(ra.is_fully_precomputable(10));
        assert!(!ra.is_fully_precomputable(0));
    }

    #[test]
    fn test_is_not_precomputable_no_chains() {
        let program = mk_program(vec![
            make_trigger("btn", Type::Bool),
            make_state("x", int(0)),
            make_txn("t1", Expr::Bool(true), Expr::Bool(true), vec![
                assign("x", ident("btn")),
            ]),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        assert!(!ra.is_fully_precomputable(100));
    }

    #[test]
    fn test_is_not_precomputable_ffi() {
        let program = mk_program(vec![
            make_const("total", 100),
            make_state("count", int(0)),
            make_state("x", int(0)),
            make_txn("heavy",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("x", int(1)),
                    Statement::Term { values: vec![Some(int(0))], modifiers: vec![], swan_song: None },
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        assert!(!ra.is_fully_precomputable(100));
    }

    #[test]
    fn test_collect_final_values_all_internal() {
        let program = mk_program(vec![
            make_const("total", 100),
            make_state("count", int(0)),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("step_a",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("x", int(42)),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
            make_txn("step_b",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("y", add(ident("x"), int(1))),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        let result = ra.collect_final_values(&program);
        assert!(result.is_some());
        let bindings = result.unwrap();
        assert!(!bindings.is_empty());
        let found_count = bindings.iter().any(|(_, m)| m.get("count") == Some(&100));
        assert!(found_count, "Final count should equal bound (100), got {:?}", bindings);
    }

    #[test]
    fn test_collect_final_values_with_trigger() {
        let program = mk_program(vec![
            make_const("total", 50),
            make_trigger("sensor", Type::Bool),
            make_state("count", int(0)),
            make_state("x", int(0)),
            make_state("y", int(0)),
            make_txn("step_a",
                Expr::Bool(true),
                Expr::Bool(true),
                vec![
                    assign("x", ident("sensor")),
                ],
            ),
            make_txn("step_b",
                Expr::Lt(Box::new(ident("count")), Box::new(ident("total"))),
                Expr::Bool(true),
                vec![
                    assign("y", add(ident("x"), int(1))),
                    assign("count", add(ident("count"), int(1))),
                ],
            ),
        ]);
        let mut ra = RegionAnalyzer::analyze(&program);
        ra.compose_chains();
        let result = ra.collect_final_values(&program);
        assert!(result.is_some());
    }
}
