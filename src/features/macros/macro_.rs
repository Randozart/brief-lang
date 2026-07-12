use crate::features::macros::context::{MacroContext, MacroDef};

/// Expand a single MacroCall node by evaluating it in a sandboxed interpreter.
/// The macro definition must already be registered in `ctx.macros` (via
/// `collect_macro_defs` or direct insertion).
pub fn expand_macro_call(
    ctx: &mut MacroContext,
    name: &str,
    args: &[crate::ast::Expr],
) -> Result<crate::interpreter::Value, String> {
    let def: MacroDef = ctx.macros.get(name)
        .ok_or_else(|| format!("undefined macro '{}'", name))?
        .clone();
    let block: Option<crate::ast::Block> = None; // $!name() cannot carry trailing block in expr position
    let mut interpreter = crate::interpreter::Interpreter::new();
    let value = crate::features::macros::template::expand_macro(
        ctx, &mut interpreter, &def, args, block,
    )?;
    // Hygiene is NOT applied here — the caller is responsible if injecting
    // the result into a surrounding scope (e.g. expand_macro_in_stmt does it).
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, MacroArgType, Statement, TopLevel};
    use crate::interpreter::Interpreter;
    use crate::features::macros::expand::expand_macro_calls_in_items;
    use crate::features::macros::context::MacroDef;

    #[test]
    fn test_expand_macro_call_returns_value() {
        let mut ctx = MacroContext::new();
        ctx.macros.insert("identity".to_string(), MacroDef {
            name: "identity".to_string(),
            params: vec![("x".to_string(), MacroArgType::Expr)],
            return_type: None,
            body: vec![Statement::Term {
                values: vec![Some(Expr::Identifier("x".to_string()))],
                swan_song: None,
                modifiers: vec![],
            }],
        });
        let result = expand_macro_call(&mut ctx, "identity", &[Expr::Integer(42)]);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let value = result.unwrap();
        assert_eq!(value, crate::interpreter::Value::Bits(crate::interpreter::i64_to_bits(42)));
    }

    #[test]
    fn test_expand_undefined_macro_errors() {
        let mut ctx = MacroContext::new();
        let result = expand_macro_call(&mut ctx, "nonexistent", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("undefined macro"));
    }

    #[test]
    fn test_end_to_end_macro_parse_expand_interpret() {
        // Full pipeline: parse source → collect macro defs → expand →
        // interpret the expanded defn → verify result is 42
        let source = r#"macro make_42() -> Block { term quote { term 42; }; };"#;
        let mut parser = crate::parser::Parser::new(source);
        let mut program = parser.parse().expect("Parsing should succeed");
        assert_eq!(program.items.len(), 1);
        // Collect macro def (removes MacroDef from items)
        let mut ctx = MacroContext::new();
        crate::features::macros::expand::collect_macro_defs(&mut program, &mut ctx);
        assert!(program.items.is_empty(), "MacroDef should be removed from items");
        assert!(ctx.macros.contains_key("make_42"));
        // Now call the macro directly (simulating what expansion would produce)
        let result = expand_macro_call(&mut ctx, "make_42", &[]);
        assert!(result.is_ok(), "macro expansion failed: {:?}", result.err());
        let value = result.unwrap();
        // The macro returns a Block containing `term 42;`
        // We need to evaluate this block in the interpreter
        if let crate::interpreter::Value::Block(stmts) = &value {
            assert_eq!(stmts.len(), 1, "Expected 1 statement in block");
            // Execute in interpreter
            let mut interp = Interpreter::new();
            for stmt in stmts {
                interp.exec_stmt(stmt).expect("Statement execution should succeed");
                if let Some(ret) = interp.return_value.take() {
                    assert_eq!(ret, crate::interpreter::Value::Bits(crate::interpreter::i64_to_bits(42)));
                    return;
                }
            }
            panic!("Expected term statement to produce a return value");
        } else {
            panic!("Expected macro to return Block, got {:?}", value);
        }
    }

    #[test]
    fn test_end_to_end_defn_with_macro_call_parse_expand_interpret() {
        // Full pipeline: parse a program with a macro definition + a defn
        // that uses $!, expand, then interpret the defn → verify result is 42
        let source = r#"macro fortytwo() -> Block { term quote { let x: Int = 42; }; }; defn main() -> Int { $!fortytwo(); term x; };"#;
        let mut parser = crate::parser::Parser::new(source);
        let mut program = parser.parse().expect("Parsing should succeed");
        let mut ctx = MacroContext::new();
        // Run Phase 1b: collect macro defs → expand calls
        crate::features::macros::expand::collect_macro_defs(&mut program, &mut ctx);
        expand_macro_calls_in_items(&mut program.items, &mut ctx)
            .expect("Macro expansion should succeed");
        // The program should now have a defn with the let + term
        assert_eq!(program.items.len(), 1, "Expected 1 item after expansion");
        let defn = match &program.items[0] {
            TopLevel::Definition(d) => d.clone(),
            other => panic!("Expected Definition, got {:?}", other),
        };
        assert!(defn.body.len() >= 2, "Expected ≥2 statements in defn body, got {}", defn.body.len());
        // Interpret the defn body
        let mut interp = Interpreter::new();
        for stmt in &defn.body {
            interp.exec_stmt(stmt).expect("Statement execution should succeed");
            if let Some(ret) = interp.return_value.take() {
                assert_eq!(ret, crate::interpreter::Value::Bits(crate::interpreter::i64_to_bits(42)),
                    "Expected 42 from expanded macro program");
                return;
            }
        }
        panic!("Expected term statement to produce a return value");
    }
}
