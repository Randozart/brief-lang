// 2026-08-23 (Plan 1.1/1.4): regression tests for intrinsic argument
// emission and compile-error recording.

use crate::ast::top::*;
use crate::ast::*;
use crate::backend::vm::VmBackend;
use crate::type_universe::TypeUniverse;
use std::collections::HashMap;

fn defn(name: &str, params: Vec<(String, Type)>, body: Vec<Statement>) -> TopLevel {
    TopLevel::Definition(Definition {
        name: name.into(),
        type_params: vec![],
        parameters: params,
        output_type: None,
        outputs: vec![],
        contract: Contract {
            pre_condition: Expr::Bool(true),
            post_condition: Expr::Bool(true),
            watchdog: None,
            explicit: false,
            span: None,
        },
        body,
        metadata: HashMap::new(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: None,
    })
}

/// Plan 1.1: an intrinsic call with a variable argument must pass the
/// variable's VALUE — the old code dropped every Identifier arg.
#[test]
fn intrinsic_call_passes_variable_arguments() {
    // fn go(x: Int) { Alloc#(x); term; }
    let prog = vec![
        defn(
            "go",
            vec![("x".into(), Type::int())],
            vec![
                Statement::Expression(Expr::Call(
                    "Alloc#".into(),
                    vec![Expr::Identifier("x".into())],
                    None,
                )),
                Statement::Term(None),
            ],
        ),
    ];
    let universe = TypeUniverse::new();
    let mut vm = VmBackend::new();
    let bytes = vm.generate(&prog, &universe);
    assert!(vm.errors.is_empty(), "variable arg must not error: {:?}", vm.errors);

    // Bytecode shape: PUSH_LOCAL 0; HCALL <id>; RET — the load must be there.
    // (The file starts with a LAIR header, so scan the whole buffer.)
    let load_local = super::assembler::OP_LOAD_LOCAL;
    let hcall = super::assembler::OP_HCALL;
    assert!(
        bytes.contains(&(load_local as u8)),
        "expected LOAD_LOCAL for the variable arg in {:?}",
        &bytes[..bytes.len().min(32)]
    );
    assert!(bytes.contains(&(hcall as u8)), "host call must follow");
}

/// Plan 1.1: literal arguments still pass through unchanged.
#[test]
fn intrinsic_call_passes_literal_arguments() {
    let prog = vec![
        defn(
            "go",
            vec![],
            vec![
                Statement::Expression(Expr::Call(
                    "Alloc#".into(),
                    vec![Expr::Decimal(64)],
                    None,
                )),
                Statement::Term(None),
            ],
        ),
    ];
    let universe = TypeUniverse::new();
    let mut vm = VmBackend::new();
    vm.generate(&prog, &universe);
    assert!(
        vm.errors.is_empty(),
        "literal arg must not error: {:?}",
        vm.errors
    );
}

/// Plan 1.4: a float literal records a helpful error naming the construct.
#[test]
fn float_literal_records_compile_error() {
    let prog = vec![defn(
        "f",
        vec![],
        vec![Statement::Term(Some(Expr::Float(2.5)))],
    )];
    let universe = TypeUniverse::new();
    let mut vm = VmBackend::new();
    vm.generate(&prog, &universe);
    assert_eq!(vm.errors.len(), 1, "{:?}", vm.errors);
    assert!(vm.errors[0].contains("float literals"), "{}", vm.errors[0]);
    assert!(vm.errors[0].contains("in 'f'"), "names the function: {}", vm.errors[0]);
}

/// Plan 1.4: unknown function call is recorded, not silent.
#[test]
fn unknown_function_records_error() {
    let prog = vec![defn(
        "f",
        vec![],
        vec![
            Statement::Expression(Expr::Call("nope".into(), vec![], None)),
            Statement::Term(None),
        ],
    )];
    let universe = TypeUniverse::new();
    let mut vm = VmBackend::new();
    vm.generate(&prog, &universe);
    assert!(
        vm.errors.iter().any(|e| e.contains("'nope'")),
        "{:?}",
        vm.errors
    );
}
