use crate::ast::{Expr, Program, Statement, TopLevel};
use std::collections::{HashMap, HashSet, VecDeque};

/// A variable-level dependency graph built from the program's top-level
/// declarations and transactions. Used by the `trg` reactive dirty-flag
/// system to topologically order variables and assign bit indices.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Variables in topological order (trg inputs first, then dependents).
    pub topo_order: Vec<String>,
    /// Bit index assigned to each variable (0..63 for u64, extendable).
    pub bit_index: HashMap<String, usize>,
    /// Direct dependencies: variable -> variables it reads.
    pub dependencies: HashMap<String, Vec<String>>,
    /// Reverse map: variable -> variables that read it.
    pub dependents: HashMap<String, Vec<String>>,
    /// Variables declared as `trg` (externally driven inputs).
    pub is_trg: HashSet<String>,
    /// All known variable names from StateDecl and Trigger declarations.
    pub all_vars: HashSet<String>,
}

#[derive(Debug)]
pub struct DependencyError {
    pub cycle: Vec<String>,
}

impl DependencyGraph {
    /// Build the dependency graph from a program.
    pub fn build(program: &Program) -> Result<Self, DependencyError> {
        let mut graph = DependencyGraph {
            topo_order: Vec::new(),
            bit_index: HashMap::new(),
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            is_trg: HashSet::new(),
            all_vars: HashSet::new(),
        };

        // Pass 1: collect all variable names
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    graph.all_vars.insert(decl.name.clone());
                }
                TopLevel::Trigger(trg) => {
                    graph.all_vars.insert(trg.name.clone());
                    graph.is_trg.insert(trg.name.clone());
                }
                TopLevel::TriggerBinding { name, .. } => {
                    graph.all_vars.insert(name.clone());
                    graph.is_trg.insert(name.clone());
                }
                TopLevel::Transaction(txn) => {
                    // Transaction names are callable, not state variables
                    // But local variables inside bodies may reference state vars
                }
                TopLevel::Definition(_defn) => {
                    // Definition params are local, not state vars — skip
                }
                _ => {}
            }
        }

        // Pass 2: extract variable reads from state decl expressions
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                if let Some(expr) = &decl.expr {
                    let reads = collect_expr_identifiers(expr);
                    let filtered: Vec<String> = reads
                        .into_iter()
                        .filter(|r| graph.all_vars.contains(r))
                        .collect();
                    if !filtered.is_empty() {
                        graph.dependencies.insert(decl.name.clone(), filtered.clone());
                        for dep in &filtered {
                            graph.dependents.entry(dep.clone()).or_default().push(decl.name.clone());
                        }
                    }
                }
            }
        }

        // Pass 3: extract variable reads from transaction bodies
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                for stmt in &txn.body {
                    graph.collect_from_statement(stmt);
                }
                // Also collect from pre/post conditions
                let pre_ids = collect_expr_identifiers(&txn.contract.pre_condition);
                for id in pre_ids {
                    if graph.all_vars.contains(&id) && !graph.is_trg.contains(&id) {
                        // Tracked as dependency reference
                    }
                }
            }
        }

        // Pass 4: topological sort (Kahn's algorithm)
        let topo = graph.topological_sort()?;
        graph.topo_order = topo;

        // Pass 5: assign bit indices
        for (i, var) in graph.topo_order.iter().enumerate() {
            graph.bit_index.insert(var.clone(), i);
        }

        Ok(graph)
    }

    fn collect_from_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                let lhs_name = match lhs {
                    Expr::Identifier(n) => Some(n.clone()),
                    _ => None,
                };
                let rhs_ids: Vec<String> = collect_expr_identifiers(expr)
                    .into_iter()
                    .filter(|r| self.all_vars.contains(r))
                    .collect();
                if let Some(lhs) = lhs_name {
                    if self.all_vars.contains(&lhs) && !rhs_ids.is_empty() {
                        self.dependencies.entry(lhs.clone()).or_default().extend(rhs_ids.clone());
                        for dep in &rhs_ids {
                            self.dependents.entry(dep.clone()).or_default().push(lhs.clone());
                        }
                    }
                }
            }
            Statement::Let { name, expr, .. } => {
                if let Some(e) = expr {
                    let rhs_ids: Vec<String> = collect_expr_identifiers(e)
                        .into_iter()
                        .filter(|r| self.all_vars.contains(r))
                        .collect();
                    if self.all_vars.contains(name) && !rhs_ids.is_empty() {
                        self.dependencies.entry(name.clone()).or_default().extend(rhs_ids.clone());
                        for dep in &rhs_ids {
                            self.dependents.entry(dep.clone()).or_default().push(name.clone());
                        }
                    }
                }
            }
            Statement::Guarded { statements, .. } => {
                for s in statements {
                    self.collect_from_statement(s);
                }
            }
            Statement::Expression(expr) => {
                // Expression statements may reference state vars (e.g., FFI calls)
                let ids: Vec<String> = collect_expr_identifiers(expr)
                    .into_iter()
                    .filter(|r| self.all_vars.contains(r))
                    .collect();
                // These are reads, tracked passively
                let _ = ids;
            }
            _ => {}
        }
    }

    fn topological_sort(&self) -> Result<Vec<String>, DependencyError> {
        // In-degree count per variable
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for var in &self.all_vars {
            in_degree.entry(var).or_insert(0);
        }
        for (_, deps) in &self.dependencies {
            for dep in deps {
                *in_degree.entry(dep).or_insert(0) += 0; // ensure entry exists
            }
        }
        // Actually, properly compute in_degree: for each var, count deps from others
        let mut in_degree2: HashMap<&str, usize> = HashMap::new();
        for var in &self.all_vars {
            in_degree2.entry(var.as_str()).or_insert(0);
        }
        for (var, deps) in &self.dependencies {
            for _dep in deps {
                // var depends on dep — dep has an outgoing edge to var
                // var's in-degree = number of vars it depends on
                *in_degree2.entry(var.as_str()).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm: start with vars that have in-degree 0 (trg inputs, independent vars)
        let mut queue: VecDeque<&str> = VecDeque::new();
        for var in &self.all_vars {
            if *in_degree2.get(var.as_str()).unwrap_or(&0) == 0 {
                queue.push_back(var.as_str());
            }
        }

        let mut sorted: Vec<String> = Vec::new();
        let mut in_deg = in_degree2.clone();

        while let Some(var) = queue.pop_front() {
            sorted.push(var.to_string());

            // Decrease in-degree for all dependents
            if let Some(deps) = self.dependents.get(var) {
                for dep in deps {
                    if let Some(deg) = in_deg.get_mut(dep.as_str()) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep.as_str());
                        }
                    }
                }
            }
        }

        if sorted.len() != self.all_vars.len() {
            // Find the cycle members
            let unsorted: Vec<String> = self
                .all_vars
                .difference(&sorted.iter().cloned().collect())
                .cloned()
                .collect();
            return Err(DependencyError { cycle: unsorted });
        }

        Ok(sorted)
    }
}

/// Collect all identifier references from an expression (recursive walk).
pub fn collect_expr_identifiers(expr: &Expr) -> Vec<String> {
    let mut ids = Vec::new();
    collect_expr_ids_inner(expr, &mut ids);
    ids
}

fn collect_expr_ids_inner(expr: &Expr, ids: &mut Vec<String>) {
    match expr {
        Expr::Identifier(n) | Expr::PriorState(n) => {
            ids.push(n.clone());
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
            collect_expr_ids_inner(a, ids);
            collect_expr_ids_inner(b, ids);
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Cast(a, _) => {
            collect_expr_ids_inner(a, ids);
        }
        Expr::Call(_, args) | Expr::CellCall(_, args) => {
            for arg in args {
                collect_expr_ids_inner(arg, ids);
            }
        }
        Expr::ListLiteral(items) => {
            for item in items {
                collect_expr_ids_inner(item, ids);
            }
        }
        Expr::ListIndex(list, index) => {
            collect_expr_ids_inner(list, ids);
            collect_expr_ids_inner(index, ids);
        }
        Expr::FieldAccess(obj, _) => {
            collect_expr_ids_inner(obj, ids);
        }
        Expr::Projection { source, .. } => {
            collect_expr_ids_inner(source, ids);
        }
        Expr::StructInstance(_, fields) => {
            for (_, expr) in fields {
                collect_expr_ids_inner(expr, ids);
            }
        }
        Expr::Match { value, arms } => {
            collect_expr_ids_inner(value, ids);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_ids_inner(guard, ids);
                }
                collect_expr_ids_inner(&arm.body, ids);
            }
        }
        Expr::Slice { value, start, end, stride, mask } => {
            collect_expr_ids_inner(value, ids);
            if let Some(s) = start { collect_expr_ids_inner(s, ids); }
            if let Some(e) = end { collect_expr_ids_inner(e, ids); }
            if let Some(s) = stride { collect_expr_ids_inner(s, ids); }
            if let Some(m) = mask { collect_expr_ids_inner(m, ids); }
        }
        Expr::ArrowMut { target, index, value, .. } => {
            collect_expr_ids_inner(target, ids);
            collect_expr_ids_inner(index, ids);
            if let Some(v) = value { collect_expr_ids_inner(v, ids); }
        }
        Expr::ArrowDiscard { target, index } => {
            collect_expr_ids_inner(target, ids);
            collect_expr_ids_inner(index, ids);
        }
        Expr::ArrowTransfer { dest, source, filter, consume: _ } => {
            collect_expr_ids_inner(dest, ids);
            collect_expr_ids_inner(source, ids);
            if let Some(f) = filter { collect_expr_ids_inner(f, ids); }
        }
        Expr::Tuple(items) => {
            for item in items {
                collect_expr_ids_inner(item, ids);
            }
        }
        Expr::TupleDestructure(_, expr) => {
            collect_expr_ids_inner(expr, ids);
        }
        Expr::Block(stmts, last_expr) => {
            for stmt in stmts {
                statement_ids(stmt, ids);
            }
            collect_expr_ids_inner(last_expr, ids);
        }
        Expr::IsType(expr, _) | Expr::Like(expr, _) | Expr::FromCheck(expr, _) => {
            collect_expr_ids_inner(expr, ids);
        }
        Expr::MapLiteral(entries) => {
            for (k, v) in entries {
                collect_expr_ids_inner(k, ids);
                collect_expr_ids_inner(v, ids);
            }
        }
        Expr::SetLiteral(items) => {
            for item in items {
                collect_expr_ids_inner(item, ids);
            }
        }
        Expr::IntrinsicCall { args, .. } => {
            for arg in args {
                collect_expr_ids_inner(arg, ids);
            }
        }
        Expr::PatternMatch { value, .. } => {
            collect_expr_ids_inner(value, ids);
        }
        // Literal leaves — no identifiers
        Expr::Integer(_) | Expr::IntegerSuffixed(_, _) | Expr::Float(_) | Expr::Float64(_) | Expr::String(_) | Expr::Char(_)
        | Expr::Bool(_) | Expr::Term | Expr::Ellipsis | Expr::TypeRef(_)
        | Expr::RegexLiteral(_) | Expr::SharedMem(_) => {}
        // Pattern B wrappers — literal leaves, no identifiers
        Expr::Literal(_) => {}
        Expr::BinaryOp(op) => {
            collect_expr_ids_inner(&op.left, ids);
            collect_expr_ids_inner(&op.right, ids);
        }
        Expr::UnaryOp(op) => {
            collect_expr_ids_inner(&op.operand, ids);
        }
        Expr::ArrowMutExpr(op) => {
            collect_expr_ids_inner(&op.target, ids);
            collect_expr_ids_inner(&op.index, ids);
            if let Some(v) = &op.value { collect_expr_ids_inner(v, ids); }
        }
        Expr::ArrowDiscardExpr(op) => {
            collect_expr_ids_inner(&op.target, ids);
            collect_expr_ids_inner(&op.index, ids);
        }
        Expr::ArrowTransferExpr(op) => {
            collect_expr_ids_inner(&op.dest, ids);
            collect_expr_ids_inner(&op.source, ids);
            if let Some(f) = &op.filter { collect_expr_ids_inner(f, ids); }
        }
        Expr::EllipsisExpr(_) => {}
        // Pattern B — remaining packed feature structs
        Expr::ProjectionExpr(proj) => {
            collect_expr_ids_inner(&proj.source, ids);
        }
        Expr::CallExpr(call) => {
            for arg in &call.args {
                collect_expr_ids_inner(arg, ids);
            }
        }
        Expr::ListLiteralExpr(lit) => {
            for elem in &lit.elements {
                collect_expr_ids_inner(elem, ids);
            }
        }
        Expr::MapLiteralExpr(lit) => {
            for (k, v) in &lit.entries {
                collect_expr_ids_inner(k, ids);
                collect_expr_ids_inner(v, ids);
            }
        }
        Expr::SetLiteralExpr(lit) => {
            for elem in &lit.entries {
                collect_expr_ids_inner(elem, ids);
            }
        }
        Expr::SliceExpr(slice) => {
            collect_expr_ids_inner(&slice.value, ids);
            if let Some(s) = &slice.start { collect_expr_ids_inner(s, ids); }
            if let Some(e) = &slice.end { collect_expr_ids_inner(e, ids); }
            if let Some(s) = &slice.stride { collect_expr_ids_inner(s, ids); }
            if let Some(m) = &slice.mask { collect_expr_ids_inner(m, ids); }
        }
        Expr::MultiSlice { value, .. } => {
            collect_expr_ids_inner(value, ids);
        }
        Expr::MultiSliceExpr(ms) => {
            collect_expr_ids_inner(&ms.value, ids);
        }
        Expr::FieldAccessExpr(fa) => {
            collect_expr_ids_inner(&fa.obj, ids);
        }
        Expr::StructInstanceExpr(si) => {
            for (_, expr) in &si.fields {
                collect_expr_ids_inner(expr, ids);
            }
        }
        Expr::ObjectLiteral(fields) => {
            for (_, expr) in fields {
                collect_expr_ids_inner(expr, ids);
            }
        }
        Expr::ObjectLiteralExpr(ol) => {
            for (_, expr) in &ol.fields {
                collect_expr_ids_inner(expr, ids);
            }
        }
        Expr::PatternMatchExpr(pm) => {
            collect_expr_ids_inner(&pm.value, ids);
        }
        Expr::MatchExpr(me) => {
            collect_expr_ids_inner(&me.value, ids);
            for arm in &me.arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_ids_inner(guard, ids);
                }
                collect_expr_ids_inner(&arm.body, ids);
            }
        }
        Expr::BlockExpr(block) => {
            for stmt in &block.stmts {
                statement_ids(stmt, ids);
            }
            collect_expr_ids_inner(&block.last, ids);
        }
        Expr::TupleExpr(t) => {
            for elem in &t.exprs {
                collect_expr_ids_inner(elem, ids);
            }
        }
        Expr::TupleDestructureExpr(td) => {
            collect_expr_ids_inner(&td.expr, ids);
        }
        Expr::SigCall { expr, .. } => {
            collect_expr_ids_inner(expr, ids);
        }
        Expr::SigCallExpr(sce) => {
            collect_expr_ids_inner(&sce.expr, ids);
        }
        Expr::Within { body, fallback, .. } => {
            collect_expr_ids_inner(body, ids);
            collect_expr_ids_inner(fallback, ids);
        }
        Expr::SubtypeProjection { source, ops } => {
            collect_expr_ids_inner(source, ids);
            for op in ops {
                match op {
                    crate::ast::SubtypeOp::Filter(e)
                    | crate::ast::SubtypeOp::Map(e)
                    | crate::ast::SubtypeOp::Sort(e)
                    | crate::ast::SubtypeOp::Match(e)
                    | crate::ast::SubtypeOp::Group(e) => collect_expr_ids_inner(e, ids),
                    crate::ast::SubtypeOp::Join(e1, e2) => {
                        collect_expr_ids_inner(e1, ids);
                        collect_expr_ids_inner(e2, ids);
                    }
                    crate::ast::SubtypeOp::Sum(e)
                    | crate::ast::SubtypeOp::Avg(e)
                    | crate::ast::SubtypeOp::Min(e)
                    | crate::ast::SubtypeOp::Max(e) => collect_expr_ids_inner(e, ids),
                    crate::ast::SubtypeOp::Limit(_)
                    | crate::ast::SubtypeOp::Skip(_)
                    | crate::ast::SubtypeOp::Unique
                    | crate::ast::SubtypeOp::Count => {}
                }
            }
        }
        Expr::SubtypeProjectionExpr(spe) => {
            collect_expr_ids_inner(&spe.source, ids);
            for op in &spe.ops {
                match op {
                    crate::ast::SubtypeOp::Filter(e)
                    | crate::ast::SubtypeOp::Map(e)
                    | crate::ast::SubtypeOp::Sort(e)
                    | crate::ast::SubtypeOp::Match(e)
                    | crate::ast::SubtypeOp::Group(e) => collect_expr_ids_inner(e, ids),
                    crate::ast::SubtypeOp::Join(e1, e2) => {
                        collect_expr_ids_inner(e1, ids);
                        collect_expr_ids_inner(e2, ids);
                    }
                    crate::ast::SubtypeOp::Sum(e)
                    | crate::ast::SubtypeOp::Avg(e)
                    | crate::ast::SubtypeOp::Min(e)
                    | crate::ast::SubtypeOp::Max(e) => collect_expr_ids_inner(e, ids),
                    crate::ast::SubtypeOp::Limit(_)
                    | crate::ast::SubtypeOp::Skip(_)
                    | crate::ast::SubtypeOp::Unique
                    | crate::ast::SubtypeOp::Count => {}
                }
            }
        }
        Expr::DbvlTable { .. } | Expr::DbvlTableExpr(_) => {}
        // Macro/template nodes — should be expanded before reaching analysis
        Expr::TemplateCall { .. } | Expr::MacroCall { .. } | Expr::Interpolate(..) | Expr::InterpolateExpr(..) | Expr::QuoteBlock { .. } => {
            unreachable!("macro/template should have been expanded")
        }
        // Pipe chains — desugared before this pass
        Expr::PipeChain(_) => unreachable!("PipeChain should have been desugared"),
            Expr::AddrOf(inner) => collect_expr_ids_inner(inner, ids),
            Expr::Deref(inner) => collect_expr_ids_inner(inner, ids),
    }
}

fn statement_ids(stmt: &Statement, ids: &mut Vec<String>) {
    match stmt {
        Statement::Assignment { lhs, expr, .. } => {
            // 2026-07-09: Handle AddrOf LHS for pointer writes.
            if let Some(n) = lhs.as_var_name() {
                ids.push(n.to_string());
            }
            collect_expr_ids_inner(expr, ids);
        }
        Statement::Let { name, expr, .. } => {
            ids.push(name.clone());
            if let Some(e) = expr {
                collect_expr_ids_inner(e, ids);
            }
        }
        Statement::Guarded { statements, .. } => {
            for s in statements {
                statement_ids(s, ids);
            }
        }
        Statement::Expression(expr) => {
            collect_expr_ids_inner(expr, ids);
        }
        Statement::TrgBinding { name, .. } => {
            ids.push(name.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_program(items: Vec<TopLevel>) -> Program {
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        }
    }

    fn make_state_decl(name: &str, ty: Type, expr: Option<Expr>) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty,
            expr,
            address: None,
            bit_range: None,
            constraint: None,
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
            address: LinkRef::Explicit(0),
            bit_range: None,
            stages: vec![],
            condition: None,
            is_wake: true,
            is_const: false,
            span: None,
            annotations: vec![],
            modifiers: vec![],
        })
    }

    #[test]
    fn test_empty_program() {
        let graph = DependencyGraph::build(&make_program(vec![])).unwrap();
        assert!(graph.topo_order.is_empty());
        assert!(graph.all_vars.is_empty());
    }

    #[test]
    fn test_trg_variable_only() {
        let graph = DependencyGraph::build(&make_program(vec![
            make_trigger("sensor", Type::int()),
        ])).unwrap();
        assert_eq!(graph.topo_order.len(), 1);
        assert!(graph.is_trg.contains("sensor"));
        assert_eq!(graph.bit_index["sensor"], 0);
    }

    #[test]
    fn test_simple_dependency() {
        let graph = DependencyGraph::build(&make_program(vec![
            make_trigger("sensor", Type::int()),
            make_state_decl(
                "derived",
                Type::int(),
                Some(Expr::Add(
                    Box::new(Expr::Identifier("sensor".to_string())),
                    Box::new(Expr::Integer(5)),
                )),
            ),
        ])).unwrap();
        assert_eq!(graph.topo_order.len(), 2);
        let sensor_pos = graph.topo_order.iter().position(|v| v == "sensor").unwrap();
        let derived_pos = graph.topo_order.iter().position(|v| v == "derived").unwrap();
        assert!(sensor_pos < derived_pos, "trg must come before dependent");
        assert_eq!(graph.bit_index["sensor"], 0);
        assert_eq!(graph.bit_index["derived"], 1);
        assert!(graph.dependencies.contains_key("derived"));
        assert_eq!(graph.dependencies["derived"], vec!["sensor"]);
        assert_eq!(graph.dependents["sensor"], vec!["derived"]);
    }

    #[test]
    fn test_chain_dependency() {
        let graph = DependencyGraph::build(&make_program(vec![
            make_trigger("a", Type::int()),
            make_state_decl("b", Type::int(), Some(Expr::Identifier("a".to_string()))),
            make_state_decl("c", Type::int(), Some(Expr::Identifier("b".to_string()))),
        ])).unwrap();
        let pos_a = graph.topo_order.iter().position(|v| v == "a").unwrap();
        let pos_b = graph.topo_order.iter().position(|v| v == "b").unwrap();
        let pos_c = graph.topo_order.iter().position(|v| v == "c").unwrap();
        assert!(pos_a < pos_b && pos_b < pos_c, "topological order broken");
    }

    #[test]
    fn test_cycle_detection() {
        let result = DependencyGraph::build(&make_program(vec![
            make_state_decl("a", Type::int(), Some(Expr::Identifier("b".to_string()))),
            make_state_decl("b", Type::int(), Some(Expr::Identifier("a".to_string()))),
        ]));
        assert!(result.is_err(), "cycle should be detected");
        if let Err(e) = result {
            assert!(!e.cycle.is_empty(), "cycle should have members");
        }
    }

    #[test]
    fn test_multiple_trgs() {
        let graph = DependencyGraph::build(&make_program(vec![
            make_trigger("a", Type::int()),
            make_trigger("b", Type::int()),
            make_state_decl(
                "sum",
                Type::int(),
                Some(Expr::Add(
                    Box::new(Expr::Identifier("a".to_string())),
                    Box::new(Expr::Identifier("b".to_string())),
                )),
            ),
        ])).unwrap();
        assert_eq!(graph.topo_order.len(), 3);
        assert!(graph.bit_index["a"] < graph.bit_index["sum"]);
        assert!(graph.bit_index["b"] < graph.bit_index["sum"]);
    }

    #[test]
    fn test_independent_vars() {
        let graph = DependencyGraph::build(&make_program(vec![
            make_trigger("x", Type::int()),
            make_state_decl("y", Type::int(), None),
        ])).unwrap();
        assert_eq!(graph.topo_order.len(), 2);
    }
}
