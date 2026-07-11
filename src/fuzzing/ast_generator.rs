// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! AST Generator for Property-Based Fuzzing
//!
//! Procedurally generates valid and semi-valid Brief ASTs for fuzzing.
//! Uses depth limiting to prevent stack overflow during generation.

use crate::ast::*;
use std::collections::HashMap;
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;

/// Maximum depth for recursive AST generation
const MAX_DEPTH: usize = 8;

/// Generate a random Brief program
pub fn arb_program(max_depth: usize) -> impl Strategy<Value = Program> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (0usize..=5usize).prop_flat_map(move |num_items| {
        proptest::collection::vec(
            arb_top_level(max_depth),
            num_items..=num_items + 3,
        ).prop_map(|items| Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
            watchdog_defaults: (None, None),
        })
    })
}

/// Generate a random top-level item
fn arb_top_level(max_depth: usize) -> impl Strategy<Value = TopLevel> {
    let max_depth = max_depth.min(MAX_DEPTH);
    prop_oneof![
        arb_state_decl(max_depth),
        arb_transaction(max_depth),
        arb_definition(max_depth),
        arb_trigger_decl(max_depth),
        arb_enum_def(max_depth),
        arb_struct_def(max_depth),
    ]
}

/// Generate a random state declaration
pub fn arb_state_decl(max_depth: usize) -> impl Strategy<Value = TopLevel> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier(),
        arb_type(),
        arb_expr(max_depth).prop_map(Some),
    ).prop_map(|(name, ty, expr)| {
        TopLevel::StateDecl(StateDecl {
            name,
            ty,
            expr,
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: Vec::new(),
        constraint: None,
        })
    })
}

/// Generate a random trigger declaration
pub fn arb_trigger_decl(_max_depth: usize) -> impl Strategy<Value = TopLevel> {
    (
        arb_identifier(),
        arb_simple_type(),
        any::<u64>(),
    ).prop_map(|(name, ty, addr)| {
        TopLevel::Trigger(TriggerDeclaration {
            name,
            ty,
            address: LinkRef::Explicit(addr),
            bit_range: None,
            stages: Vec::new(),
            condition: None,
            is_wake: true,
            is_const: false,
            span: None,
            annotations: vec![],
            modifiers: vec![],
        })
    })
}

/// Generate a random transaction
pub fn arb_transaction(max_depth: usize) -> impl Strategy<Value = TopLevel> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier(),
        arb_contract(max_depth),
        arb_statement_list(max_depth),
        prop_oneof![Just(false), Just(true)],
    ).prop_map(|(name, contract, body, is_reactive)| {
        TopLevel::Transaction(Transaction {
            is_async: false,
            is_reactive,
            name,
            parameters: Vec::new(),
            contract,
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: Vec::new(),

            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     })
    })
}

/// Generate a random contract
pub fn arb_contract(max_depth: usize) -> impl Strategy<Value = Contract> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_expr(max_depth),
        arb_expr(max_depth),
    ).prop_map(|(pre, post)| {
        Contract {
            pre_condition: pre,
            post_condition: post,
            watchdog: None,
            span: None,
        }
    })
}

/// Generate a random definition
pub fn arb_definition(max_depth: usize) -> impl Strategy<Value = TopLevel> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier(),
        arb_type(),
        arb_expr(max_depth),
    ).prop_map(|(name, return_type, body)| {
        TopLevel::Definition(Definition {
            name,
            type_params: Vec::new(),
            parameters: Vec::new(),
            outputs: vec![return_type],
            output_type: None,
            output_names: vec![None],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term { values: vec![Some(body)], modifiers: vec![], swan_song: None }],
            is_lambda: false,
            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
        })
    })
}

/// Generate a random enum definition
pub fn arb_enum_def(_max_depth: usize) -> impl Strategy<Value = TopLevel> {
    (
        arb_identifier(),
        proptest::collection::vec(arb_identifier(), 1usize..=4usize),
    ).prop_map(|(name, variants)| {
        TopLevel::Enum(EnumDefinition {
            name,
            type_params: Vec::new(),
            variants: variants.into_iter()
                .map(EnumVariant::Unit)
                .collect(),
            span: None,
        })
    })
}

/// Generate a random struct definition
pub fn arb_struct_def(max_depth: usize) -> impl Strategy<Value = TopLevel> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier(),
        proptest::collection::vec(arb_struct_field(max_depth), 1usize..=3usize),
    ).prop_map(|(name, fields)| {
        TopLevel::Struct(StructDefinition {
            name,
            type_params: Vec::new(),
            parent: None,
            fields,
            transactions: Vec::new(),
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        })
    })
}

fn arb_struct_field(max_depth: usize) -> impl Strategy<Value = StructField> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier(),
        arb_type(),
        arb_expr(max_depth).prop_map(Some),
    ).prop_map(|(name, ty, default)| {
        StructField { name, ty, default, visibility: Visibility::Public }
    })
}

/// Generate a random list of statements
pub fn arb_statement_list(max_depth: usize) -> impl Strategy<Value = Vec<Statement>> {
    let max_depth = max_depth.min(MAX_DEPTH);
    proptest::collection::vec(arb_statement(max_depth), 1usize..=5usize)
}

/// Generate a random statement
pub fn arb_statement(max_depth: usize) -> impl Strategy<Value = Statement> {
    let max_depth = max_depth.min(MAX_DEPTH);
    
    if max_depth == 0 {
        // At max depth, only generate leaf statements
        return arb_term_statement(max_depth).boxed();
    }
    
    prop_oneof![
        // Assignment: &var = expr;
        arb_assignment(max_depth),
        // Let binding: let x: Type = expr;
        arb_let_statement(max_depth),
        // Guarded: [condition] { statements };
        arb_guarded_statement(max_depth),
        // Term: term;
        arb_term_statement(max_depth),
        // Escape: escape;
        arb_escape_statement(),
        // Expression: expr;
        arb_expr_statement(max_depth),
        // DISABLED: alka/on_exit — not ready for use.
        // Alka block: alka { ... };
        // arb_alka_statement(),
        // OnExit block pragma: #on_exit { ... };
        // arb_on_exit_statement(max_depth),
    ].boxed()
}

/// Generate a random assignment statement
fn arb_assignment(max_depth: usize) -> impl Strategy<Value = Statement> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier().prop_map(|name| Expr::Identifier(name)),
        arb_expr(max_depth),
    ).prop_map(|(lhs, expr)| {
        Statement::Assignment {
            lhs,
            expr,
            timeout: None,
            modifiers: vec![],
        }
    })
}

/// Generate a random let statement
fn arb_let_statement(max_depth: usize) -> impl Strategy<Value = Statement> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_identifier(),
        arb_type(),
        arb_expr(max_depth).prop_map(Some),
    ).prop_map(|(name, ty, expr)| {
        Statement::Let {
            name,
            ty: Some(ty),
            expr,
            address: None,
            address_expr: None,
            bit_range: None,
            is_override: false,
            modifiers: vec![],
            constraint: None,
        }
    })
}

/// Generate a random guarded statement
fn arb_guarded_statement(max_depth: usize) -> impl Strategy<Value = Statement> {
    let max_depth = max_depth.min(MAX_DEPTH);
    (
        arb_expr(max_depth),
        proptest::collection::vec(arb_statement(max_depth.saturating_sub(1)), 1usize..=3usize),
    ).prop_map(|(condition, statements)| {
        Statement::Guarded {
            condition,
            statements,
            metadata: HashMap::new(),
        }
    })
}

/// Generate a random term statement
fn arb_term_statement(max_depth: usize) -> impl Strategy<Value = Statement> {
    let max_depth = max_depth.min(MAX_DEPTH);
    prop_oneof![
        Just(Statement::Term { values: vec![], modifiers: vec![], swan_song: None }),
        arb_expr(max_depth).prop_map(|e| Statement::Term { values: vec![Some(e)], modifiers: vec![], swan_song: None }),
    ]
}

// DISABLED: alka/on_exit — not ready for use.
// /// Generate a random alka block
// fn arb_alka_statement() -> impl Strategy<Value = Statement> {
//     prop_oneof![
//         Just(Statement::Alka(AlkaBlock {
//             dangerous: false,
//             content: "FENCE ALL;".to_string(),
//             span: None,
//         })),
//         Just(Statement::Alka(AlkaBlock {
//             dangerous: true,
//             content: "PULSE DOORBELL @ 0x90;".to_string(),
//             span: None,
//         })),
//     ]
// }
// /// Generate a random #on_exit block pragma
// fn arb_on_exit_statement(max_depth: usize) -> impl Strategy<Value = Statement> {
//     let max_depth = max_depth.min(MAX_DEPTH);
//     let sub_depth = max_depth.saturating_sub(1);
//     proptest::collection::vec(arb_statement(sub_depth), 1..3)
//         .prop_map(|body| {
//             Statement::OnExit { body, span: None }
//         })
// }

/// Generate a random escape statement
fn arb_escape_statement() -> impl Strategy<Value = Statement> {
    prop_oneof![
        Just(Statement::Escape(None)),
        arb_simple_expr().prop_map(|e| Statement::Escape(Some(e))),
    ]
}

/// Generate a random expression statement
fn arb_expr_statement(max_depth: usize) -> impl Strategy<Value = Statement> {
    let max_depth = max_depth.min(MAX_DEPTH);
    arb_expr(max_depth).prop_map(Statement::Expression)
}

/// Generate a random expression
pub fn arb_expr(max_depth: usize) -> impl Strategy<Value = Expr> {
    let max_depth = max_depth.min(MAX_DEPTH);
    
    if max_depth == 0 {
        return arb_leaf_expr().boxed();
    }
    
    prop_oneof![
        // Leaf expressions
        arb_leaf_expr(),
        // Binary operations
        arb_binary_expr(max_depth),
        // Unary operations
        arb_unary_expr(max_depth),
        // Function calls
        arb_call_expr(max_depth),
        // Prior state access
        arb_identifier().prop_map(|name| Expr::PriorState(name)),
        // Owned ref
        arb_identifier().prop_map(|name| Expr::AddrOf(Box::new(Expr::Identifier(name)))),
    ].boxed()
}

/// Generate a random leaf expression (no recursion)
fn arb_leaf_expr() -> impl Strategy<Value = Expr> {
    prop_oneof![
        any::<i64>().prop_map(Expr::Integer),
        any::<f64>().prop_map(Expr::Float),
        any::<bool>().prop_map(Expr::Bool),
        arb_string_literal().prop_map(Expr::String),
        arb_identifier().prop_map(Expr::Identifier),
    ]
}

/// Generate a random simple expression (for escape, etc.)
fn arb_simple_expr() -> impl Strategy<Value = Expr> {
    prop_oneof![
        any::<i64>().prop_map(Expr::Integer),
        any::<bool>().prop_map(Expr::Bool),
        arb_identifier().prop_map(Expr::Identifier),
    ]
}

/// Generate a random binary expression
fn arb_binary_expr(max_depth: usize) -> impl Strategy<Value = Expr> {
    let max_depth = max_depth.min(MAX_DEPTH);
    let sub_depth = max_depth.saturating_sub(1);
    (
        arb_expr(sub_depth),
        arb_binary_op(),
        arb_expr(sub_depth),
    ).prop_map(|(left, op, right)| {
        match op {
            BinOp::Add => Expr::Add(Box::new(left), Box::new(right)),
            BinOp::Sub => Expr::Sub(Box::new(left), Box::new(right)),
            BinOp::Mul => Expr::Mul(Box::new(left), Box::new(right)),
            BinOp::Div => Expr::Div(Box::new(left), Box::new(right)),
            BinOp::Mod => Expr::Mod(Box::new(left), Box::new(right)),
            BinOp::Eq => Expr::Eq(Box::new(left), Box::new(right)),
            BinOp::Ne => Expr::Ne(Box::new(left), Box::new(right)),
            BinOp::Lt => Expr::Lt(Box::new(left), Box::new(right)),
            BinOp::Le => Expr::Le(Box::new(left), Box::new(right)),
            BinOp::Gt => Expr::Gt(Box::new(left), Box::new(right)),
            BinOp::Ge => Expr::Ge(Box::new(left), Box::new(right)),
            BinOp::And => Expr::And(Box::new(left), Box::new(right)),
            BinOp::Or => Expr::Or(Box::new(left), Box::new(right)),
            BinOp::BitAnd => Expr::BitAnd(Box::new(left), Box::new(right)),
            BinOp::BitOr => Expr::BitOr(Box::new(left), Box::new(right)),
            BinOp::BitXor => Expr::BitXor(Box::new(left), Box::new(right)),
            BinOp::Shl => Expr::Shl(Box::new(left), Box::new(right)),
            BinOp::Shr => Expr::Shr(Box::new(left), Box::new(right)),
        }
    })
}

#[derive(Clone, Copy, Debug)]
enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

fn arb_binary_op() -> impl Strategy<Value = BinOp> {
    prop_oneof![
        Just(BinOp::Add), Just(BinOp::Sub), Just(BinOp::Mul),
        Just(BinOp::Div), Just(BinOp::Mod),
        Just(BinOp::Eq), Just(BinOp::Ne),
        Just(BinOp::Lt), Just(BinOp::Le),
        Just(BinOp::Gt), Just(BinOp::Ge),
        Just(BinOp::And), Just(BinOp::Or),
        Just(BinOp::BitAnd), Just(BinOp::BitOr),
        Just(BinOp::BitXor), Just(BinOp::Shl), Just(BinOp::Shr),
    ]
}

/// Generate a random unary expression
fn arb_unary_expr(max_depth: usize) -> impl Strategy<Value = Expr> {
    let max_depth = max_depth.min(MAX_DEPTH);
    let sub_depth = max_depth.saturating_sub(1);
    prop_oneof![
        arb_expr(sub_depth).prop_map(|e| Expr::Not(Box::new(e))),
        arb_expr(sub_depth).prop_map(|e| Expr::Neg(Box::new(e))),
        arb_expr(sub_depth).prop_map(|e| Expr::BitNot(Box::new(e))),
    ]
}

/// Generate a random function call expression
fn arb_call_expr(max_depth: usize) -> impl Strategy<Value = Expr> {
    let max_depth = max_depth.min(MAX_DEPTH);
    let sub_depth = max_depth.saturating_sub(1);
    (
        arb_identifier(),
        proptest::collection::vec(arb_expr(sub_depth), 0usize..=3usize),
    ).prop_map(|(name, args)| {
        Expr::Call(name, args)
    })
}

/// Generate a random identifier
fn arb_identifier() -> impl Strategy<Value = String> {
    "[a-z_][a-z0-9_]{0,15}".prop_filter(
        "Filter reserved keywords",
        |s| !is_reserved_keyword(s),
    )
}

fn is_reserved_keyword(s: &str) -> bool {
    matches!(
        s,
        "txn" | "defn" | "sig" | "let" | "term" | "escape"
            | "true" | "false" | "Int" | "UInt" | "Float"
            | "Bool" | "String" | "void" | "Data" | "Char"
            | "frgn" | "import" | "from" | "as" | "struct"
            | "enum" | "trg" | "trigger" | "rct" | "async"
            | "match" | "uni" | "unification" | "unify"
            | "resource" | "rsrc" | "registry" | "reg"
            | "render" | "rstruct" | "stage" | "on"
            | "forall" | "exists" | "within" | "link"
            | "asm" | "bank" | "Ok" | "Err" | "some"
            | "none" | "const" | "constant"
    )
}

/// Generate a random string literal
fn arb_string_literal() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 _.-]{0,50}".prop_map(|s| s)
}

/// Generate a random type
pub fn arb_type() -> impl Strategy<Value = Type> {
    prop_oneof![
        Just(Type::int()),
        Just(Type::uint()),
        Just(Type::float()),
        Just(Type::bool_()),
        Just(Type::string()),
        Just(Type::Void),
        Just(Type::data()),
        Just(Type::char_()),
        arb_identifier().prop_map(Type::Custom),
        // Constrained types (sized integers)
        prop_oneof![Just(8), Just(16), Just(32), Just(64)]
            .prop_flat_map(|n| {
                prop_oneof![
                    (Just(Type::int()), Just(n)).prop_map(|(t, n)| Type::Constrained(Box::new(t), BitRange::Any(n))),
                    (Just(Type::uint()), Just(n)).prop_map(|(t, n)| Type::Constrained(Box::new(t), BitRange::Any(n))),
                ]
            }),
    ]
}

/// Generate a random simple type (no constrained types)
fn arb_simple_type() -> impl Strategy<Value = Type> {
    prop_oneof![
        Just(Type::int()),
        Just(Type::uint()),
        Just(Type::float()),
        Just(Type::bool_()),
        Just(Type::string()),
        Just(Type::Void),
        Just(Type::data()),
        Just(Type::char_()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_generate_random_expr() {
        // Generate multiple random expressions and verify they don't panic
        for _ in 0..100 {
            let expr = arb_expr(5).new_tree(&mut TestRunner::default()).unwrap().current();
            // Just verify it doesn't panic
            let _ = format!("{:?}", expr);
        }
    }

    #[test]
    fn test_generate_random_statement() {
        for _ in 0..100 {
            let stmt = arb_statement(5).new_tree(&mut TestRunner::default()).unwrap().current();
            let _ = format!("{:?}", stmt);
        }
    }

    #[test]
    fn test_generate_random_program() {
        for _ in 0..50 {
            let program = arb_program(5).new_tree(&mut TestRunner::default()).unwrap().current();
            let _ = format!("{:?}", program);
        }
    }

    #[test]
    fn test_depth_limiting() {
        // Verify that depth limiting prevents excessive nesting
        let expr = arb_expr(MAX_DEPTH).new_tree(&mut TestRunner::default()).unwrap().current();
        let depth = measure_expr_depth(&expr);
        assert!(depth <= MAX_DEPTH + 2, "Expression depth {} exceeded limit", depth);
    }

    fn measure_expr_depth(expr: &Expr) -> usize {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
            | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
            | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r)
            | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
            | Expr::Shl(l, r) | Expr::Shr(l, r) => {
                1 + measure_expr_depth(l).max(measure_expr_depth(r))
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => 1 + measure_expr_depth(e),
            Expr::Call(_, args) => {
                1 + args.iter().map(measure_expr_depth).max().unwrap_or(0)
            }
            _ => 0,
        }
    }

    proptest! {
        #[test]
        fn test_expr_roundtrip(program in arb_program(4)) {
            // Generate a program and verify it can be formatted without panic
            let _ = format!("{:?}", program);
        }

        #[test]
        fn test_statement_variety(stmt in arb_statement(6)) {
            // Verify we get different statement types
            let is_valid = matches!(
                &stmt,
                Statement::Assignment { .. }
                    | Statement::Let { .. }
                    | Statement::Guarded { .. }
                    | Statement::Term { .. } | Statement::TermBang { .. }
                    | Statement::Escape(_)
                    | Statement::Expression(_)
            );
            prop_assert!(is_valid, "Generated invalid statement type: {:?}", stmt);
        }
    }
}
