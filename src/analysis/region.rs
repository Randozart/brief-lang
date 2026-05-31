use crate::ast::{Expr, Program, Statement, TopLevel, Type};
use std::collections::{HashMap, HashSet, VecDeque};

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
        };

        analyzer.register_declarations(program);
        analyzer.build_dependency_graph(program);
        analyzer.seed_frontier();
        analyzer.propagate_classification();
        analyzer.compute_regions();
        analyzer.estimate_value_sets();
        analyzer.detect_linear_chains(program);

        analyzer
    }

    // ── Phase A: Collect declarations ──────────────────────────────────

    fn register_declarations(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    let interval = decl.expr.as_ref().and_then(|e| Self::expr_to_interval(e));
                    let vc = VarInfo {
                        classification: VarClass::Pure,
                        interval,
                        value_set_size: None,
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

                // Dependencies from pre/post conditions
                self.collect_identifiers(&txn.contract.pre_condition, &txn.name);
                self.collect_identifiers(&txn.contract.post_condition, &txn.name);

                // Dependencies from body assignments
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

                // Track per-transaction reads and writes
                self.txn_reads.entry(txn.name.clone()).or_default().extend(txn_read_vars);
                self.txn_writes.entry(txn.name.clone()).or_default().extend(txn_write_vars);
            }
        }
    }

    fn collect_identifiers(&mut self, expr: &Expr, reader_for: &str) {
        match expr {
            Expr::Identifier(name) | Expr::OwnedRef(name) => {
                self.deps
                    .entry(reader_for.to_string())
                    .or_default()
                    .insert(name.clone());
                self.rev_deps
                    .entry(name.clone())
                    .or_default()
                    .insert(reader_for.to_string());
            }
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Mod(a, b)
            | Expr::Eq(a, b)
            | Expr::Ne(a, b)
            | Expr::Lt(a, b)
            | Expr::Le(a, b)
            | Expr::Gt(a, b)
            | Expr::Ge(a, b)
            | Expr::And(a, b)
            | Expr::Or(a, b)
            | Expr::BitAnd(a, b)
            | Expr::BitOr(a, b)
            | Expr::BitXor(a, b)
            | Expr::Shl(a, b)
            | Expr::Shr(a, b)
            | Expr::Concat(a, b) => {
                self.collect_identifiers(a, reader_for);
                self.collect_identifiers(b, reader_for);
            }
            Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _)
            | Expr::ListLen(a) => {
                self.collect_identifiers(a, reader_for);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.collect_identifiers(arg, reader_for);
                }
            }
            Expr::ListLiteral(elems) => {
                for e in elems {
                    self.collect_identifiers(e, reader_for);
                }
            }
            Expr::Tuple(elems) => {
                for e in elems {
                    self.collect_identifiers(e, reader_for);
                }
            }
            Expr::ListIndex(list, idx) => {
                self.collect_identifiers(list, reader_for);
                self.collect_identifiers(idx, reader_for);
            }
            Expr::FieldAccess(obj, _) => {
                self.collect_identifiers(obj, reader_for);
            }
            Expr::Block(_, last) | Expr::TupleDestructure(_, last) => {
                self.collect_identifiers(last, reader_for);
            }
            _ => {}
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
}

fn expr_to_var_set(expr: &Expr) -> HashSet<String> {
    let mut vars = HashSet::new();
    collect_var_ids(expr, &mut vars);
    vars
}

fn collect_var_ids(expr: &Expr, vars: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(n) | Expr::OwnedRef(n) => { vars.insert(n.clone()); }
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
        | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
        | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::And(a, b)
        | Expr::Or(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
        | Expr::Shl(a, b) | Expr::Shr(a, b) | Expr::Concat(a, b) => {
            collect_var_ids(a, vars);
            collect_var_ids(b, vars);
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _)
        | Expr::ListLen(a) => collect_var_ids(a, vars),
        Expr::Call(_, args) => { for a in args { collect_var_ids(a, vars); } }
        Expr::ListLiteral(elems) => { for e in elems { collect_var_ids(e, vars); } }
        Expr::Tuple(elems) => { for e in elems { collect_var_ids(e, vars); } }
        Expr::ListIndex(l, i) => { collect_var_ids(l, vars); collect_var_ids(i, vars); }
        Expr::FieldAccess(o, _) => collect_var_ids(o, vars),
        Expr::Block(_, last) | Expr::TupleDestructure(_, last) => collect_var_ids(last, vars),
        _ => {}
    }
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
            is_wake: false,
            span: None,
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
}
