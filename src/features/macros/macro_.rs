use crate::features::macros::context::MacroContext;

/// Expand a single MacroCall node.
/// Macros have full access to I/O, AST introspection, and $gensym().
pub fn expand_macro_call(
    _ctx: &mut MacroContext,
    _name: &str,
    _args: &[crate::ast::Expr],
) -> Result<crate::interpreter::Value, String> {
    // TODO: implement macro expansion (M3)
    Err("macro expansion not yet implemented".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_expansion_stub() {
        let mut ctx = MacroContext::new();
        let result = expand_macro_call(&mut ctx, "test", &[]);
        assert!(result.is_err(), "Macro expansion should fail (not yet implemented)");
    }
}
