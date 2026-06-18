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
    fn test_unknown_directive_is_ignored() {
        let effects = resolve_directives(&[tag("volatile")], DirectiveCtx::Transaction);
        assert_eq!(effects.len(), 0, "unknown directives should be ignored");
    }
}
