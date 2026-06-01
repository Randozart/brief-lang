pub mod aarch64;
pub mod c;
pub mod llvm;
pub mod rust;
pub mod verilog;
pub mod vhdl;
pub mod wasm;
pub mod webstack;
pub mod x86_64;
pub mod tcl_generator;
pub mod cobol;

use crate::analysis::call_graph::CallGraph;
use crate::analysis::range::ParameterRanges;
use crate::analysis::dataflow::DataflowError;
use crate::analysis::region::RegionAnalyzer;
use crate::analysis::transition_graph::ReactorTransitionGraph;
use crate::ast::{Expr, Hashtag, Program, Statement, TopLevel, Transaction, Definition, StructDefinition};

/// Intent: Container for all shared analysis results that backends can consume.
/// Backends check `optimize_mode` to decide whether to use optimized paths
/// (pre-scheduled DAG emission) or fall back to full idiomatic codegen.
pub struct AnalysisResults {
    pub call_graph: CallGraph,
    pub param_ranges: ParameterRanges,
    pub fusable_pairs: Vec<(String, String)>,
    pub dataflow_errors: Vec<DataflowError>,
    pub optimize_mode: bool,
    pub transition_graph: ReactorTransitionGraph,
    pub region_analyzer: RegionAnalyzer,
}

/// Intent: Run shared program analysis for backend code generation.
/// Returns an AnalysisResults with CallGraph, ParameterRanges, fusable pairs,
/// and dataflow errors. When optimize is true, runs extra analysis passes
/// and applies peephole optimization.
pub fn analyze_program(program: &Program, optimize: bool) -> AnalysisResults {
    let mut cg = CallGraph::new();
    cg.build_from_program(program);

    let mut pr = ParameterRanges::new();
    pr.analyze(program);

    let fusable_pairs = if optimize {
        detect_fusable_pairs(program)
    } else {
        Vec::new()
    };

    let dataflow_errors = if optimize {
        let analyzer = crate::analysis::dataflow::DataflowAnalyzer::new(program);
        analyzer.analyze()
    } else {
        Vec::new()
    };

    let transition_graph = ReactorTransitionGraph::build(program);
    let region_analyzer = RegionAnalyzer::analyze(program);

    AnalysisResults {
        call_graph: cg,
        param_ranges: pr,
        fusable_pairs,
        dataflow_errors,
        optimize_mode: optimize,
        transition_graph,
        region_analyzer,
    }
}

/// Intent: Apply peephole optimization after analysis. Returns a new Program
/// with redundant assignments, dead expressions, and foldable constants removed.
/// Only called when optimize mode is active.
pub fn run_peephole(program: &Program, analysis: &AnalysisResults) -> Program {
    if !analysis.optimize_mode {
        return program.clone();
    }
    peephole_optimize_program(program)
}

/// Intent: Return the list of hashtags supported by a given backend name.
/// Backend names match the subcommand (e.g. "c", "rust", "wasm", "verilog", "vhdl", "x86_64", "aarch64", "cobol").
pub fn supported_hashtags(backend: &str) -> Vec<&'static str> {
    match backend {
        "c" | "x86_64" | "aarch64" | "llvm" => {
            vec!["volatile", "sfence", "lfence", "mfence", "aligned", "packed"]
        }
        "rust" => {
            vec!["volatile", "sync", "aligned", "repr", "packed"]
        }
        "wasm" | "webstack" => {
            vec!["volatile", "aligned"]
        }
        "verilog" | "vhdl" => {
            vec!["clock", "register", "gate", "posedge", "negedge"]
        }
        "cobol" => {
            vec!["volatile", "packed", "aligned"]
        }
        _ => {
            vec![] // unknown backend — no known support
        }
    }
}

/// Intent: Result of validating a single hashtag against a backend.
#[derive(Debug, Clone, PartialEq)]
pub enum HashtagValidation {
    Supported,
    UnsupportedAdvisory(String),
    UnsupportedMandatory(String),
}

/// Intent: Validate a list of hashtags against a given backend.
/// Returns a list of validation results — callers should emit
/// warnings for `UnsupportedAdvisory` and errors for `UnsupportedMandatory`.
pub fn validate_hashtags(hashtags: &[Hashtag], backend: &str) -> Vec<HashtagValidation> {
    let supported = supported_hashtags(backend);
    let mut results = Vec::new();

    for tag in hashtags {
        if is_scoped_elsewhere(tag, backend) {
            continue;
        }
        results.push(validate_single_hashtag(tag, &supported));
    }

    results
}

fn is_scoped_elsewhere(tag: &Hashtag, backend: &str) -> bool {
    if let Some(ref scope) = tag.scoped {
        return scope != backend;
    }
    false
}

fn validate_single_hashtag(tag: &Hashtag, supported: &[&'static str]) -> HashtagValidation {
    if supported.contains(&tag.name.as_str()) {
        return HashtagValidation::Supported;
    }
    if tag.mandatory && has_supported_fallback(tag, supported) {
        return HashtagValidation::Supported;
    }
    if tag.mandatory {
        return HashtagValidation::UnsupportedMandatory(tag.name.clone());
    }
    HashtagValidation::UnsupportedAdvisory(tag.name.clone())
}

fn has_supported_fallback(tag: &Hashtag, supported: &[&'static str]) -> bool {
    tag.fallback.iter().any(|f| supported.contains(&f.as_str()))
}

/// Intent: Collect all hashtags from a list of statements recursively.
fn collect_hashtags_from_body(body: &[Statement]) -> Vec<crate::ast::Hashtag> {
    let mut tags = Vec::new();
    for stmt in body {
        match stmt {
            Statement::Assignment { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Let { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Term { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Guarded { statements, .. } => tags.extend(collect_hashtags_from_body(statements)),
            Statement::OnExit { body, .. } => tags.extend(collect_hashtags_from_body(body)),
            _ => {}
        }
    }
    tags
}

/// Intent: Validate all hashtags in a program against the target backend.
/// Returns true if there are NO unsupported mandatory tag errors.
/// Prints warnings/eprintfs for unsupported tags.
pub fn validate_hashtags_in_program(program: &Program, backend: &str, strict: bool) -> bool {
    let mut all_tags: Vec<crate::ast::Hashtag> = Vec::new();

    for item in &program.items {
        match item {
            TopLevel::Transaction(txn) => {
                all_tags.extend(txn.modifiers.clone());
                all_tags.extend(collect_hashtags_from_body(&txn.body));
                for (_, variant_body) in &txn.variant_bodies {
                    all_tags.extend(collect_hashtags_from_body(variant_body));
                }
            }
            TopLevel::Definition(defn) => {
                all_tags.extend(defn.modifiers.clone());
                all_tags.extend(collect_hashtags_from_body(&defn.body));
                for (_, variant_body) in &defn.variant_bodies {
                    all_tags.extend(collect_hashtags_from_body(variant_body));
                }
            }
            TopLevel::Struct(sdef) => {
                all_tags.extend(sdef.modifiers.clone());
            }
            TopLevel::StateDecl(..) => {} // top-level let, no hashtags
            _ => {}
        }
    }

    let results = validate_hashtags(&all_tags, backend);
    let mut has_errors = false;

    for result in &results {
        match result {
            HashtagValidation::Supported => {}
            HashtagValidation::UnsupportedAdvisory(name) => {
                eprintln!("warning: Hashtag #{} is not supported by {} backend (advisory, ignored)", name, backend);
            }
            HashtagValidation::UnsupportedMandatory(name) => {
                eprintln!("error: Mandatory hashtag #!{} is not supported by {} backend", name, backend);
                if strict {
                    eprintln!("  Hint: Use a different backend, remove the tag, or add fallbacks with #!A|B|C");
                }
                has_errors = true;
            }
        }
    }

    !has_errors
}

/// Intent: Collect all identifiers referenced by an expression.
pub fn collect_expr_identifiers(expr: &Expr, ids: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Identifier(n) | Expr::OwnedRef(n) | Expr::PriorState(n) => {
            ids.insert(n.clone());
        }
        Expr::Integer(_) | Expr::Bool(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_) => {}
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Mod(a, b)
        | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b)
        | Expr::Ge(a, b) | Expr::And(a, b) | Expr::Or(a, b) | Expr::BitAnd(a, b)
        | Expr::BitOr(a, b) | Expr::BitXor(a, b) | Expr::Shl(a, b) | Expr::Shr(a, b) => {
            collect_expr_identifiers(a, ids);
            collect_expr_identifiers(b, ids);
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) => collect_expr_identifiers(a, ids),
        Expr::Call(_, args) => {
            for arg in args {
                collect_expr_identifiers(arg, ids);
            }
        }
        Expr::FieldAccess(obj, _) => collect_expr_identifiers(obj, ids),
        Expr::ListLiteral(elems) => {
            for elem in elems {
                collect_expr_identifiers(elem, ids);
            }
        }
        Expr::ListIndex(list, idx) => {
            collect_expr_identifiers(list, ids);
            collect_expr_identifiers(idx, ids);
        }
        Expr::ListLen(inner) => collect_expr_identifiers(inner, ids),
        Expr::Tuple(elems) => {
            for elem in elems {
                collect_expr_identifiers(elem, ids);
            }
        }
        _ => {}
    }
}

/// Intent: Collect all identifiers assigned in a guarded statement body.
pub fn collect_assigned_identifiers(body: &[Statement]) -> Vec<String> {
    let mut ids = Vec::new();
    for stmt in body {
        if let Statement::Assignment { lhs, .. } = stmt {
            if let Expr::Identifier(name) | Expr::OwnedRef(name) = lhs {
                ids.push(name.clone());
            }
        }
    }
    ids
}

/// Intent: Collect all identifiers read by an expression/statement.
pub fn collect_read_identifiers(body: &[Statement]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for stmt in body {
        match stmt {
            Statement::Assignment { expr, .. } => {
                collect_expr_identifiers(expr, &mut ids);
            }
            Statement::Let { expr: Some(e), .. } => {
                collect_expr_identifiers(e, &mut ids);
            }
            Statement::Guarded { condition, statements, .. } => {
                collect_expr_identifiers(condition, &mut ids);
                ids.extend(collect_read_identifiers(statements));
            }
            Statement::Expression(e) => {
                collect_expr_identifiers(e, &mut ids);
            }
            _ => {}
        }
    }
    ids
}

/// Intent: Detect pairs of transactions where post(A) implies pre(B),
/// meaning they could be fused into a single atomic transaction.
/// Pre-computes analysis per transaction to avoid recomputing in the O(n²) loop.
pub fn detect_fusable_pairs(program: &Program) -> Vec<(String, String)> {
    let txns: Vec<&crate::ast::Transaction> = program
        .items
        .iter()
        .filter_map(|item| {
            if let TopLevel::Transaction(txn) = item {
                Some(txn)
            } else {
                None
            }
        })
        .collect();

    // Pre-compute for each transaction: write list, read set, post set, pre set
    let mut all_writes: Vec<Vec<String>> = Vec::new();
    let mut all_reads: Vec<std::collections::HashSet<String>> = Vec::new();
    let mut all_post_ids: Vec<std::collections::HashSet<String>> = Vec::new();
    let mut all_pre_ids: Vec<std::collections::HashSet<String>> = Vec::new();

    for txn in &txns {
        all_writes.push(collect_assigned_identifiers(&txn.body));
        all_reads.push(collect_read_identifiers(&txn.body));
        let mut post_ids = std::collections::HashSet::new();
        collect_expr_identifiers(&txn.contract.post_condition, &mut post_ids);
        all_post_ids.push(post_ids);
        let mut pre_ids = std::collections::HashSet::new();
        collect_expr_identifiers(&txn.contract.pre_condition, &mut pre_ids);
        all_pre_ids.push(pre_ids);
    }

    let mut pairs = Vec::new();
    for i in 0..txns.len() {
        for j in 0..txns.len() {
            if i == j { continue; }
            let fusable = all_writes[i].iter().any(|w| all_pre_ids[j].contains(w))
                || all_post_ids[i].iter().any(|id| all_reads[j].contains(id));
            if fusable {
                pairs.push((txns[i].name.clone(), txns[j].name.clone()));
            }
        }
    }
    pairs
}

/// Intent: Shared peephole optimizer that works at the AST level.
/// Handles redundant assignments, constant folding, dead statement
/// elimination, and guard simplification. Called in optimized mode.
pub fn peephole_optimize_program(program: &Program) -> Program {
    let mut items = program.items.clone();
    for item in &mut items {
        match item {
            TopLevel::Transaction(txn) => {
                txn.body = peephole_optimize_body(&txn.body);
            }
            TopLevel::Definition(defn) => {
                defn.body = peephole_optimize_body(&defn.body);
            }
            _ => {}
        }
    }
    Program {
        items,
        ..program.clone()
    }
}

fn peephole_optimize_body(body: &[Statement]) -> Vec<Statement> {
    let mut result = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        match &body[i] {
            Statement::Let { name, expr, .. } => {
                if let Some(Expr::Identifier(n)) = expr {
                    if n == name {
                        i += 1;
                        continue;
                    }
                }
                if let Some(stmt) = peephole_optimize_stmt(&body[i]) {
                    result.push(stmt);
                }
            }
            Statement::Assignment { lhs, expr, .. } => {
                if let Expr::Identifier(name) = lhs {
                    if let Expr::Identifier(n) = expr {
                        if n == name {
                            i += 1;
                            continue;
                        }
                    }
                }
                if let Some(stmt) = peephole_optimize_stmt(&body[i]) {
                    result.push(stmt);
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                if let Some(stmts) = peephole_simplify_guard(condition, statements) {
                    result.extend(stmts);
                }
            }
            _ => {
                if let Some(stmt) = peephole_optimize_stmt(&body[i]) {
                    result.push(stmt);
                }
            }
        }
        i += 1;
    }
    result
}

fn peephole_optimize_stmt(stmt: &Statement) -> Option<Statement> {
    match stmt {
        Statement::Guarded { condition, statements } => {
            let opt_body: Vec<Statement> = statements.iter().filter_map(peephole_optimize_stmt).collect();
            Some(Statement::Guarded {
                condition: peephole_optimize_expr(condition),
                statements: opt_body,
            })
        }
        Statement::Assignment { lhs, expr, timeout, modifiers } => {
            Some(Statement::Assignment {
                lhs: lhs.clone(),
                expr: peephole_optimize_expr(expr),
                timeout: timeout.clone(),
                modifiers: modifiers.clone(),
            })
        }
        Statement::Let { name, ty, expr, address, address_expr, bit_range, is_override, modifiers } => {
            Some(Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| peephole_optimize_expr(e)),
                address: *address,
                address_expr: address_expr.as_ref().map(|e| Box::new(peephole_optimize_expr(e))),
                bit_range: bit_range.clone(),
                is_override: *is_override,
                modifiers: modifiers.clone(),
            })
        }
        Statement::Expression(expr) => {
            match peephole_optimize_expr(expr) {
                Expr::Integer(_) | Expr::Bool(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_) => None,
                opt => Some(Statement::Expression(opt)),
            }
        }
        other => Some(other.clone()),
    }
}

fn peephole_optimize_expr(expr: &Expr) -> Expr {
    match expr {
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Mod(a, b) => {
            let a = peephole_optimize_expr(a);
            let b = peephole_optimize_expr(b);
            peephole_fold_binop(expr, &a, &b)
        }
        Expr::And(a, b) | Expr::Or(a, b) => {
            let a = peephole_optimize_expr(a);
            let b = peephole_optimize_expr(b);
            peephole_fold_boolop(expr, &a, &b)
        }
        Expr::Not(a) => {
            let a = peephole_optimize_expr(a);
            match &a {
                Expr::Bool(v) => Expr::Bool(!v),
                _ => Expr::Not(Box::new(a)),
            }
        }
        Expr::Neg(a) => {
            let a = peephole_optimize_expr(a);
            match &a {
                Expr::Integer(n) => Expr::Integer(-n),
                Expr::Float(f) => Expr::Float(-f),
                _ => Expr::Neg(Box::new(a)),
            }
        }
        Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) => {
            let a = peephole_optimize_expr(a);
            let b = peephole_optimize_expr(b);
            peephole_fold_cmp(expr, &a, &b)
        }
        _ => expr.clone(),
    }
}

fn peephole_fold_binop(expr: &Expr, a: &Expr, b: &Expr) -> Expr {
    match (a, b) {
        (Expr::Integer(la), Expr::Integer(rb)) => {
            match expr {
                Expr::Add(_, _) => Expr::Integer(la + rb),
                Expr::Sub(_, _) => Expr::Integer(la - rb),
                Expr::Mul(_, _) => Expr::Integer(la * rb),
                Expr::Div(_, _) => if *rb != 0 { Expr::Integer(la / rb) } else { expr.clone() },
                Expr::Mod(_, _) => if *rb != 0 { Expr::Integer(la % rb) } else { expr.clone() },
                _ => expr.clone(),
            }
        }
        (Expr::Float(la), Expr::Float(rb)) => {
            match expr {
                Expr::Add(_, _) => Expr::Float(la + rb),
                Expr::Sub(_, _) => Expr::Float(la - rb),
                Expr::Mul(_, _) => Expr::Float(la * rb),
                Expr::Div(_, _) => if *rb != 0.0 { Expr::Float(la / rb) } else { expr.clone() },
                _ => expr.clone(),
            }
        }
        (_, Expr::Integer(1)) if matches!(expr, Expr::Mul(_, _)) => a.clone(),
        (_, Expr::Integer(0)) if matches!(expr, Expr::Add(_, _) | Expr::Sub(_, _)) => a.clone(),
        (Expr::Integer(0), _) if matches!(expr, Expr::Add(_, _)) => b.clone(),
        (Expr::Integer(1), _) if matches!(expr, Expr::Mul(_, _)) => b.clone(),
        _ => {
            match expr {
                Expr::Add(_, _) => Expr::Add(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Sub(_, _) => Expr::Sub(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Mul(_, _) => Expr::Mul(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Div(_, _) => Expr::Div(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Mod(_, _) => Expr::Mod(Box::new(a.clone()), Box::new(b.clone())),
                _ => expr.clone(),
            }
        }
    }
}

fn peephole_fold_boolop(expr: &Expr, a: &Expr, b: &Expr) -> Expr {
    match (a, b) {
        (Expr::Bool(true), _) if matches!(expr, Expr::Or(_, _)) => Expr::Bool(true),
        (_, Expr::Bool(true)) if matches!(expr, Expr::Or(_, _)) => Expr::Bool(true),
        (Expr::Bool(false), _) if matches!(expr, Expr::And(_, _)) => Expr::Bool(false),
        (_, Expr::Bool(false)) if matches!(expr, Expr::And(_, _)) => Expr::Bool(false),
        (Expr::Bool(false), _) if matches!(expr, Expr::Or(_, _)) => b.clone(),
        (_, Expr::Bool(false)) if matches!(expr, Expr::Or(_, _)) => a.clone(),
        (Expr::Bool(true), _) if matches!(expr, Expr::And(_, _)) => b.clone(),
        (_, Expr::Bool(true)) if matches!(expr, Expr::And(_, _)) => a.clone(),
        _ => {
            match expr {
                Expr::And(_, _) => Expr::And(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Or(_, _) => Expr::Or(Box::new(a.clone()), Box::new(b.clone())),
                _ => expr.clone(),
            }
        }
    }
}

fn peephole_fold_cmp(expr: &Expr, a: &Expr, b: &Expr) -> Expr {
    match (a, b) {
        (Expr::Integer(la), Expr::Integer(rb)) => {
            match expr {
                Expr::Eq(_, _) => Expr::Bool(la == rb),
                Expr::Ne(_, _) => Expr::Bool(la != rb),
                Expr::Lt(_, _) => Expr::Bool(la < rb),
                Expr::Le(_, _) => Expr::Bool(la <= rb),
                Expr::Gt(_, _) => Expr::Bool(la > rb),
                Expr::Ge(_, _) => Expr::Bool(la >= rb),
                _ => expr.clone(),
            }
        }
        _ => {
            match expr {
                Expr::Eq(_, _) => Expr::Eq(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Ne(_, _) => Expr::Ne(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Lt(_, _) => Expr::Lt(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Le(_, _) => Expr::Le(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Gt(_, _) => Expr::Gt(Box::new(a.clone()), Box::new(b.clone())),
                Expr::Ge(_, _) => Expr::Ge(Box::new(a.clone()), Box::new(b.clone())),
                _ => expr.clone(),
            }
        }
    }
}

fn peephole_simplify_guard(condition: &Expr, body: &[Statement]) -> Option<Vec<Statement>> {
    match condition {
        Expr::Bool(true) => Some(body.to_vec()),
        Expr::Bool(false) => Some(Vec::new()),
        _ => {
            let opt_body: Vec<Statement> = body.iter().filter_map(peephole_optimize_stmt).collect();
            Some(vec![Statement::Guarded {
                condition: peephole_optimize_expr(condition),
                statements: opt_body,
            }])
        }
    }
}

/// Intent: Memory overlay analysis — identifies mutually exclusive variables
/// that can share the same memory location to reduce stack usage.
/// Used by C and Rust backends.
#[derive(Debug, Clone)]
pub struct MemoryOverlay {
    pub groups: Vec<Vec<String>>,
}

impl MemoryOverlay {
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    pub fn analyze(program: &Program) -> Self {
        let txns: Vec<&Transaction> = program
            .items
            .iter()
            .filter_map(|item| {
                if let TopLevel::Transaction(txn) = item {
                    Some(txn)
                } else {
                    None
                }
            })
            .collect();

        let mut all_assigned: Vec<String> = Vec::new();
        for txn in &txns {
            collect_assignments(&txn.body, &mut all_assigned);
        }
        all_assigned.sort();
        all_assigned.dedup();

        let mut overlay_groups: Vec<Vec<String>> = Vec::new();
        let mut used = std::collections::HashSet::new();

        for v in &all_assigned {
            if used.contains(v) {
                continue;
            }
            let mut group = vec![v.clone()];
            used.insert(v.clone());
            for w in &all_assigned {
                if v == w || used.contains(w) {
                    continue;
                }
                let simultaneous = txns.iter().any(|txn| {
                    let reads_v = reads_variable_general(&txn.body, v);
                    let reads_w = reads_variable_general(&txn.body, w);
                    let writes_v = writes_variable_general(&txn.body, v);
                    let writes_w = writes_variable_general(&txn.body, w);
                    (reads_v || writes_v) && (reads_w || writes_w)
                });
                if !simultaneous {
                    group.push(w.clone());
                    used.insert(w.clone());
                }
            }
            if group.len() > 1 {
                overlay_groups.push(group);
            }
        }

        Self {
            groups: overlay_groups,
        }
    }

    pub fn has_overlays(&self) -> bool {
        !self.groups.is_empty()
    }
}

fn collect_assignments(body: &[Statement], out: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, .. } => {
                if let Expr::Identifier(name) = lhs {
                    out.push(name.clone());
                }
            }
            Statement::Guarded { statements, .. } => collect_assignments(statements, out),
            _ => {}
        }
    }
}

fn reads_variable_general(body: &[Statement], var: &str) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assignment { expr, .. } => {
                let mut ids = std::collections::HashSet::new();
                collect_expr_identifiers(expr, &mut ids);
                if ids.contains(var) {
                    return true;
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let mut ids = std::collections::HashSet::new();
                collect_expr_identifiers(condition, &mut ids);
                if ids.contains(var) || reads_variable_general(statements, var) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn writes_variable_general(body: &[Statement], var: &str) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, .. } => {
                if let Expr::Identifier(name) = lhs {
                    if name == var {
                        return true;
                    }
                }
            }
            Statement::Guarded { statements, .. } => {
                if writes_variable_general(statements, var) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Intent: Tracks guard dependencies for pre-computation caching.
/// Allows backends to pre-compute guard conditions that depend on state variables.
#[derive(Debug, Clone)]
pub struct GuardTracker {
    pub var_to_guards: std::collections::HashMap<String, std::collections::HashSet<String>>,
    pub guard_to_vars: std::collections::HashMap<String, Vec<String>>,
    pub state_vars: Vec<String>,
}

impl GuardTracker {
    pub fn new() -> Self {
        Self {
            var_to_guards: std::collections::HashMap::new(),
            guard_to_vars: std::collections::HashMap::new(),
            state_vars: Vec::new(),
        }
    }

    pub fn register_guard(&mut self, guard_name: &str, dependencies: Vec<String>) {
        for dep in &dependencies {
            self.var_to_guards
                .entry(dep.clone())
                .or_default()
                .insert(guard_name.to_string());
        }
        self.guard_to_vars
            .insert(guard_name.to_string(), dependencies);
    }

    pub fn guard_dependencies(&self, guard_name: &str) -> Option<&Vec<String>> {
        self.guard_to_vars.get(guard_name)
    }

    pub fn all_state_vars(&self) -> &[String] {
        &self.state_vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Hashtag;

    /// Intent: Verify the C backend supports the volatile hashtag.
    #[test]
    fn test_c_backend_supports_volatile() {
        let tag = Hashtag { name: "volatile".into(), value: None, mandatory: false, fallback: vec![], scoped: None };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::Supported);
    }

    /// Intent: Verify the C backend rejects an unknown advisory hashtag.
    #[test]
    fn test_c_backend_rejects_unknown_advisory() {
        let tag = Hashtag { name: "thermal_sense".into(), value: None, mandatory: false, fallback: vec![], scoped: None };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::UnsupportedAdvisory("thermal_sense".to_string()));
    }

    /// Intent: Verify the C backend rejects an unknown mandatory hashtag.
    #[test]
    fn test_c_backend_rejects_unknown_mandatory() {
        let tag = Hashtag { name: "thermal_sense".into(), value: None, mandatory: true, fallback: vec![], scoped: None };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::UnsupportedMandatory("thermal_sense".to_string()));
    }

    /// Intent: Verify fallback chain tries alternative hashtags.
    #[test]
    fn test_fallback_chain_tries_alternatives() {
        let tag = Hashtag {
            name: "unknown_op".into(),
            value: None,
            mandatory: true,
            fallback: vec!["lfence".to_string(), "mfence".to_string()],
            scoped: None,
        };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::Supported);
    }

    /// Intent: Verify fallback chain returns error when all alternatives unknown.
    #[test]
    fn test_fallback_chain_all_unknown() {
        let tag = Hashtag {
            name: "unknown_op".into(),
            value: None,
            mandatory: true,
            fallback: vec!["nope1".to_string(), "nope2".to_string()],
            scoped: None,
        };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results[0], HashtagValidation::UnsupportedMandatory("unknown_op".to_string()));
    }

    /// Intent: Verify scoped tag is skipped when backend does not match.
    #[test]
    fn test_scoped_tag_skipped_for_wrong_backend() {
        let tag = Hashtag {
            name: "volatile".into(),
            value: None,
            mandatory: false,
            fallback: vec![],
            scoped: Some("verilog".to_string()),
        };
        let results = validate_hashtags(&[tag], "c");
        assert_eq!(results.len(), 0, "Scoped tag should be skipped for wrong backend");
    }

    /// Intent: Verify scoped tag is validated when backend matches.
    #[test]
    fn test_scoped_tag_validated_for_correct_backend() {
        let tag = Hashtag {
            name: "clock".into(),
            value: None,
            mandatory: false,
            fallback: vec![],
            scoped: Some("verilog".to_string()),
        };
        let results = validate_hashtags(&[tag], "verilog");
        assert_eq!(results[0], HashtagValidation::Supported);
    }
}