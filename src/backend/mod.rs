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
}

/// Intent: Run shared program analysis for backend code generation.
/// Returns an AnalysisResults with CallGraph, ParameterRanges, fusable pairs,
/// and dataflow errors. When optimize is true, runs extra analysis passes.
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

    AnalysisResults {
        call_graph: cg,
        param_ranges: pr,
        fusable_pairs,
        dataflow_errors,
        optimize_mode: optimize,
    }
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
        // Check scoped tags: only validate if scope matches the current backend
        if let Some(ref scope) = tag.scoped {
            if scope != backend {
                continue; // not our concern
            }
        }

        let is_supported = supported.iter().any(|s| *s == tag.name);

        if is_supported {
            results.push(HashtagValidation::Supported);
        } else if tag.mandatory {
            // Check fallback chain
            let fallback_supported = tag.fallback.iter().any(|f| supported.contains(&f.as_str()));
            if fallback_supported {
                results.push(HashtagValidation::Supported);
            } else {
                results.push(HashtagValidation::UnsupportedMandatory(tag.name.clone()));
            }
        } else {
            results.push(HashtagValidation::UnsupportedAdvisory(tag.name.clone()));
        }
    }

    results
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
fn collect_expr_identifiers(expr: &Expr, ids: &mut std::collections::HashSet<String>) {
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
fn collect_assigned_identifiers(body: &[Statement]) -> Vec<String> {
    let mut ids = Vec::new();
    for stmt in body {
        if let Statement::Assignment { lhs, .. } = stmt {
            if let Expr::Identifier(name) = lhs {
                ids.push(name.clone());
            }
        }
    }
    ids
}

/// Intent: Collect all identifiers read by an expression/statement.
fn collect_read_identifiers(body: &[Statement]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for stmt in body {
        match stmt {
            Statement::Assignment { expr, .. } => {
                collect_expr_identifiers(expr, &mut ids);
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

    let mut pairs = Vec::new();
    for a in &txns {
        let a_writes: Vec<String> = collect_assigned_identifiers(&a.body);
        let a_post_ids = {
            let mut ids = std::collections::HashSet::new();
            collect_expr_identifiers(&a.contract.post_condition, &mut ids);
            ids
        };

        for b in &txns {
            if a.name == b.name {
                continue;
            }
            let b_reads: std::collections::HashSet<String> = collect_read_identifiers(&b.body);
            let b_pre_ids = {
                let mut ids = std::collections::HashSet::new();
                collect_expr_identifiers(&b.contract.pre_condition, &mut ids);
                ids
            };

            // Check if post(A) writes overlap with pre(B) reads
            let fusable = a_writes.iter().any(|w| b_pre_ids.contains(w))
                || a_post_ids.iter().any(|id| b_reads.contains(id));

            if fusable {
                pairs.push((a.name.clone(), b.name.clone()));
            }
        }
    }
    pairs
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