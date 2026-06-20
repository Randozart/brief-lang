use crate::ast::Hashtag;

/// Context where a directive is applied — determines which LLVM annotations
/// the directive maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectiveCtx {
    /// Reactive transaction function definition.
    Transaction,
    /// Callable transaction or definition function.
    CallableTxn,
    /// Counted loop body (foreach, folded loop).
    Loop,
    /// General guarded or straight-line body.
    Body,
}

/// Effect produced by resolving one or more directive hashtags.
/// Multiple effects can be returned from a single set of tags.
#[derive(Debug, Clone)]
pub enum DirectiveEffect {
    /// Apply a function-level LLVM attribute string (e.g. `alwaysinline`).
    FunctionAttribute(String),
    /// Emit a `!llvm.loop.*` metadata key with the given value.
    /// The caller formats the key-value into the appropriate metadata node.
    LoopMetadata(String, String),
    /// Request GPU offloading for the current loop/txn body.
    /// The optional string is a user-specified threshold or config.
    GpuOffload(Option<String>),
    /// Export this function with a globally-visible symbol for cross-language
    /// FFI. The string is the exported symbol name.
    Export(String),
    /// No effect in this context — directive is not applicable.
    None,
}

/// Resolve all applicable directives from a list of hashtags.
/// Returns effects for the given context. Callers examine the effects
/// and apply them to the emitted LLVM IR.
pub fn resolve_directives(tags: &[Hashtag], context: DirectiveCtx) -> Vec<DirectiveEffect> {
    let mut effects = Vec::new();

    for tag in tags {
        let effect = match tag.name.as_str() {
            "inline" => resolve_inline(tag, context),
            "unroll" => resolve_unroll(tag, context),
            "vectorize" => resolve_vectorize(tag, context),
            "gpu" => resolve_gpu(tag, context),
            "export" => resolve_export(tag, context),
            _ => None,
        };
        if let Some(e) = effect {
            effects.push(e);
        }
    }

    effects
}

/// Resolve #inline / #?inline / #!inline for the given context.
fn resolve_inline(tag: &Hashtag, context: DirectiveCtx) -> Option<DirectiveEffect> {
    match context {
        DirectiveCtx::Transaction | DirectiveCtx::CallableTxn => {
            if tag.speculative {
                Some(DirectiveEffect::FunctionAttribute("inlinehint".to_string()))
            } else {
                Some(DirectiveEffect::FunctionAttribute("alwaysinline".to_string()))
            }
        }
        _ => None,
    }
}

/// Resolve #unroll / #?unroll / #!unroll for the given context.
fn resolve_unroll(tag: &Hashtag, context: DirectiveCtx) -> Option<DirectiveEffect> {
    match context {
        DirectiveCtx::Loop => {
            if tag.speculative {
                Some(DirectiveEffect::LoopMetadata(
                    "llvm.loop.unroll.enable".to_string(),
                    String::new(),
                ))
            } else {
                Some(DirectiveEffect::LoopMetadata(
                    "llvm.loop.unroll.full".to_string(),
                    String::new(),
                ))
            }
        }
        _ => None,
    }
}

/// Resolve #vectorize / #?vectorize / #!vectorize for the given context.
fn resolve_vectorize(tag: &Hashtag, context: DirectiveCtx) -> Option<DirectiveEffect> {
    match context {
        DirectiveCtx::Loop => {
            if tag.speculative {
                Some(DirectiveEffect::LoopMetadata(
                    "llvm.loop.vectorize.enable".to_string(),
                    "true".to_string(),
                ))
            } else {
                Some(DirectiveEffect::LoopMetadata(
                    "llvm.loop.vectorize.enable".to_string(),
                    "true".to_string(),
                ))
            }
        }
        _ => None,
    }
}

/// Resolve #gpu / #?gpu / #!gpu for the given context.
fn resolve_gpu(tag: &Hashtag, context: DirectiveCtx) -> Option<DirectiveEffect> {
    match context {
        // GPU offloading is applicable to both loops and full transaction bodies.
        DirectiveCtx::Loop | DirectiveCtx::Transaction | DirectiveCtx::CallableTxn => {
            Some(DirectiveEffect::GpuOffload(tag.value.clone()))
        }
        _ => None,
    }
}

/// Resolve #export / #export("name") for the given context.
/// Causes the function to be emitted as a dso_local global symbol
/// with C calling convention, making it callable from other languages.
fn resolve_export(tag: &Hashtag, context: DirectiveCtx) -> Option<DirectiveEffect> {
    match context {
        DirectiveCtx::Transaction | DirectiveCtx::CallableTxn => {
            let export_name = tag.value.clone().unwrap_or_else(|| tag.name.clone());
            Some(DirectiveEffect::Export(export_name))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Optimization Remarks
// ---------------------------------------------------------------------------

/// The compiler's decision about a speculative directive.
#[derive(Debug, Clone)]
pub enum RemarkDecision {
    /// The optimization was applied successfully.
    Applied { detail: String },
    /// The optimization was skipped (benign reason).
    Skipped { reason: String },
    /// The optimization failed (structural impossibility).
    Failed { error: String },
}

/// A structured diagnostic message explaining the compiler's decision
/// for a `#?` speculative directive.
#[derive(Debug, Clone)]
pub struct OptimizationRemark {
    /// The directive name (e.g. "vectorize", "inline", "unroll", "gpu").
    pub directive: String,
    /// The decision the compiler made.
    pub decision: RemarkDecision,
    /// Bullet-point analysis explaining the math or reasoning.
    pub analysis: Vec<String>,
    /// Actionable hints for the developer.
    pub hints: Vec<String>,
}

impl OptimizationRemark {
    pub fn applied(directive: &str, detail: String) -> Self {
        OptimizationRemark {
            directive: directive.to_string(),
            decision: RemarkDecision::Applied { detail },
            analysis: Vec::new(),
            hints: Vec::new(),
        }
    }

    pub fn skipped(directive: &str, reason: String) -> Self {
        OptimizationRemark {
            directive: directive.to_string(),
            decision: RemarkDecision::Skipped { reason },
            analysis: Vec::new(),
            hints: Vec::new(),
        }
    }

    pub fn failed(directive: &str, error: String) -> Self {
        OptimizationRemark {
            directive: directive.to_string(),
            decision: RemarkDecision::Failed { error },
            analysis: Vec::new(),
            hints: Vec::new(),
        }
    }

    pub fn with_analysis(mut self, lines: Vec<String>) -> Self {
        self.analysis = lines;
        self
    }

    pub fn with_hints(mut self, lines: Vec<String>) -> Self {
        self.hints = lines;
        self
    }

    /// Format this remark as a human-readable string.
    pub fn format(&self) -> String {
        let decision_str = match &self.decision {
            RemarkDecision::Applied { detail } => format!("applied: {}", detail),
            RemarkDecision::Skipped { reason } => format!("skipped: {}", reason),
            RemarkDecision::Failed { error } => format!("failed: {}", error),
        };
        let mut out = format!("remark: #?{} {}", self.directive, decision_str);
        for line in &self.analysis {
            out.push_str(&format!("\n  analysis:\n    - {}", line));
        }
        for line in &self.hints {
            out.push_str(&format!("\n  help:\n    - {}", line));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str) -> Hashtag {
        Hashtag { name: name.into(), value: None, mandatory: false, speculative: false, fallback: vec![], scoped: None }
    }

    fn spec_tag(name: &str) -> Hashtag {
        Hashtag { name: name.into(), value: None, mandatory: false, speculative: true, fallback: vec![], scoped: None }
    }

    #[test]
    fn test_inline_on_txn() {
        let effects = resolve_directives(&[tag("inline")], DirectiveCtx::Transaction);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::FunctionAttribute(attr) => {
                assert_eq!(attr, "alwaysinline");
            }
            _ => panic!("Expected FunctionAttribute"),
        }
    }

    #[test]
    fn test_speculative_inline_on_txn() {
        let effects = resolve_directives(&[spec_tag("inline")], DirectiveCtx::Transaction);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::FunctionAttribute(attr) => {
                assert_eq!(attr, "inlinehint");
            }
            _ => panic!("Expected FunctionAttribute"),
        }
    }

    #[test]
    fn test_inline_on_loop_is_none() {
        let effects = resolve_directives(&[tag("inline")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 0, "inline should have no effect on Loop context");
    }

    #[test]
    fn test_unroll_on_loop() {
        let effects = resolve_directives(&[tag("unroll")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::LoopMetadata(key, _) => {
                assert_eq!(key, "llvm.loop.unroll.full");
            }
            _ => panic!("Expected LoopMetadata"),
        }
    }

    #[test]
    fn test_speculative_unroll_on_loop() {
        let effects = resolve_directives(&[spec_tag("unroll")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::LoopMetadata(key, _) => {
                assert_eq!(key, "llvm.loop.unroll.enable");
            }
            _ => panic!("Expected LoopMetadata"),
        }
    }

    #[test]
    fn test_unroll_on_txn_is_none() {
        let effects = resolve_directives(&[tag("unroll")], DirectiveCtx::Transaction);
        assert_eq!(effects.len(), 0, "unroll should have no effect on Transaction context");
    }

    #[test]
    fn test_vectorize_on_loop() {
        let effects = resolve_directives(&[tag("vectorize")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::LoopMetadata(key, val) => {
                assert_eq!(key, "llvm.loop.vectorize.enable");
                assert_eq!(val, "true");
            }
            _ => panic!("Expected LoopMetadata"),
        }
    }

    #[test]
    fn test_speculative_vectorize_on_loop() {
        let effects = resolve_directives(&[spec_tag("vectorize")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::LoopMetadata(_, _) => {} // both modes produce the same metadata for now
            _ => panic!("Expected LoopMetadata"),
        }
    }

    #[test]
    fn test_multiple_directives() {
        let effects = resolve_directives(
            &[tag("inline"), tag("unroll")],
            DirectiveCtx::Loop,
        );
        // inline has no effect on Loop; unroll does
        assert_eq!(effects.len(), 1);
    }

    #[test]
    fn test_gpu_directive_on_loop() {
        let effects = resolve_directives(&[tag("gpu")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::GpuOffload(val) => {
                assert_eq!(*val, None);
            }
            _ => panic!("Expected GpuOffload"),
        }
    }

    #[test]
    fn test_gpu_directive_with_value() {
        let t = Hashtag { name: "gpu".into(), value: Some("threshold=1000".into()), mandatory: false, speculative: false, fallback: vec![], scoped: None };
        let effects = resolve_directives(&[t], DirectiveCtx::Transaction);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::GpuOffload(val) => {
                assert_eq!(val.as_deref(), Some("threshold=1000"));
            }
            _ => panic!("Expected GpuOffload"),
        }
    }

    #[test]
    fn test_speculative_gpu_directive() {
        let effects = resolve_directives(&[spec_tag("gpu")], DirectiveCtx::Loop);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            DirectiveEffect::GpuOffload(_) => {} // OK
            _ => panic!("Expected GpuOffload"),
        }
    }

    #[test]
    fn test_gpu_directive_on_body_is_none() {
        let effects = resolve_directives(&[tag("gpu")], DirectiveCtx::Body);
        assert_eq!(effects.len(), 0, "gpu should have no effect on Body context");
    }

    #[test]
    fn test_unknown_directive_is_ignored() {
        let effects = resolve_directives(&[tag("volatile")], DirectiveCtx::Transaction);
        assert_eq!(effects.len(), 0, "unknown directives should be ignored");
    }

    // ── Remark tests ──────────────────────────────────────

    #[test]
    fn test_remark_applied_format() {
        let r = OptimizationRemark::applied("inline", "inlined successfully".to_string());
        let s = r.format();
        assert!(s.contains("remark: #?inline applied: inlined successfully"));
    }

    #[test]
    fn test_remark_skipped_format() {
        let r = OptimizationRemark::skipped("vectorize", "loop-carried dependency".to_string());
        let s = r.format();
        assert!(s.contains("remark: #?vectorize skipped: loop-carried dependency"));
    }

    #[test]
    fn test_remark_failed_format() {
        let r = OptimizationRemark::failed("gpu", "unsafe side effects".to_string());
        let s = r.format();
        assert!(s.contains("remark: #?gpu failed: unsafe side effects"));
    }

    #[test]
    fn test_remark_with_analysis_and_hints() {
        let r = OptimizationRemark::skipped("unroll", "trip count too small".to_string())
            .with_analysis(vec!["trip count = 3".to_string(), "minimum = 8".to_string()])
            .with_hints(vec!["use #unroll to force unrolling".to_string()]);
        let s = r.format();
        assert!(s.contains("trip count = 3"));
        assert!(s.contains("use #unroll to force unrolling"));
    }
}
