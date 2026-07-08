// 2026-07-08: Phase 2C — type normalization pass
// Resolves Custom/Applied types to their concrete Bits widths using
// universe defaults. Runs after parsing, before typechecking and codegen.
//
// NOTE: The full normalize_types pass is gated (typechecker doesn't
// understand Applied types yet). Only desugar_string_literals() is active.
//
// Phase 2G: desugar_string_literals converts "hello" to a struct instance
// String { ptr: &"hello", len: 5, codec: 0 } when in a String-typed context.

use crate::ast::*;
use crate::type_universe::TypeUniverse;

/// Convert string literals to struct instances in String-typed assignments.
/// "hello" → String { ptr: &"hello", len: 5, codec: 0 }
pub fn desugar_string_literals(program: &mut Program, universe: &TypeUniverse) {
    for item in &mut program.items {
        desugar_toplevel_strings(item, universe);
    }
}

fn desugar_toplevel_strings(item: &mut TopLevel, universe: &TypeUniverse) {
    match item {
        TopLevel::StateDecl(decl) => {
            if decl.ty == Type::Custom("String".to_string()) {
                if let Some(expr) = &mut decl.expr {
                    if let Expr::String(s) = expr {
                        *expr = make_string_struct(s.clone());
                    }
                }
            }
        }
        TopLevel::Constant(constant) => {
            if constant.ty == Type::Custom("String".to_string()) {
                if let Expr::String(s) = &constant.expr {
                    constant.expr = make_string_struct(s.clone());
                }
            }
        }
        _ => {}
    }
}

fn make_string_struct(s: String) -> Expr {
    let len = s.len() as i64;
    Expr::StructInstance("String".to_string(), vec![
        ("ptr".to_string(), Expr::String(s)),
        ("len".to_string(), Expr::Integer(len)),
        ("codec".to_string(), Expr::Integer(0)),
    ])
}
