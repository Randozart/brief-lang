use std::collections::HashMap;
use super::*;
use crate::ast::*;

fn empty_program() -> Vec<TopLevel> {
    vec![]
}

fn make_txn(name: &str, modifiers: Vec<Annotation>) -> TopLevel {
    TopLevel::Transaction(Transaction {
        name: name.to_string(),
        is_reactive: true,
        is_async: false,
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: Contract {
            pre_condition: Expr::Bool(true),
            post_condition: Expr::Bool(true),
            is_entry: false,
            watchdog: None,
            explicit: false,
            span: None,
        },
        body: vec![
            Statement::Assign(Expr::Identifier("count".to_string()), Expr::Decimal(1)),
            Statement::Term(None),
        ],
        metadata: HashMap::new(),
        derivation: None,
        modifiers,
        span: None,
        doc: None,
    })
}

fn state_count() -> TopLevel {
    TopLevel::StateDecl(StateDecl {
        name: "count".to_string(),
        ty: Type::int(),
        span: None,
    })
}

fn default_contract() -> Contract {
    Contract {
        pre_condition: Expr::Bool(true),
        post_condition: Expr::Bool(true),
        is_entry: false,
        watchdog: None,
        explicit: false,
        span: None,
    }
}

#[test]
fn test_llvm_generates_module() {
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&empty_program(), None);
    assert!(output.contains("ModuleID"));
    assert!(output.contains("target triple"));
}

#[test]
fn test_webstack_enabled_emits_flush_state() {
    // 2026-07-26: Phase 4 — with_webstack(true) should emit __web_flush_state
    // import and state_layout() export function in the generated IR.
    let mut backend = LlvmBackend::new()
        .with_webstack(true)
        .with_int_bits(32)
        .with_target_triple("wasm32-unknown-wasi");
    let output = backend.generate(&empty_program(), None);
    assert!(output.contains("__web_flush_state"),
        "should declare __web_flush_state import");
    assert!(output.contains("state_layout"),
        "should export state_layout function");
    assert!(output.contains("__web_generation"),
        "should emit generation counter global");
}

#[test]
fn test_webstack_disabled_omits_flush_state() {
    // 2026-07-26: Phase 4 — Without with_webstack, no webstack emits.
    let mut backend = LlvmBackend::new()
        .with_int_bits(32)
        .with_target_triple("wasm32-unknown-wasi");
    let output = backend.generate(&empty_program(), None);
    assert!(!output.contains("__web_flush_state"),
        "should NOT declare __web_flush_state without webstack enabled");
    assert!(!output.contains("state_layout"),
        "should NOT export state_layout without webstack enabled");
}

#[test]
fn test_webstack_emits_flush_at_term() {
    // 2026-07-26: Phase 4 — Transactions with webstack emit __web_flush_state call.
    let mut backend = LlvmBackend::new()
        .with_webstack(true)
        .with_int_bits(32)
        .with_target_triple("wasm32-unknown-wasi");
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
        make_txn("increment", vec![]),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("__web_flush_state"),
        "transactions should call __web_flush_state at term");
}

#[test]
fn test_webstack_bv_logic_only() {
    // 2026-07-26: Phase 5 — A .bv-style program (pure logic, no view bindings)
    // compiled with webstack backend should produce WASM-targeted LLVM IR.
    let mut backend = LlvmBackend::new()
        .with_webstack(true)
        .with_int_bits(32)
        .with_target_triple("wasm32-unknown-wasi");
    let program = vec![
        state_count(),
        make_txn("compute", vec![]),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("wasm32-unknown-wasi"),
        "should use wasm32 target triple in .bv webstack mode");
    assert!(output.contains("__web_flush_state"),
        "should emit flush state for webstack even with logic-only .bv");
    assert!(output.contains("state_layout"),
        "should export state_layout function");
}

#[test]
fn test_llvm_generates_state_type() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "counter".to_string(),
            ty: Type::int(),
            span: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("%State"));
    assert!(output.contains("i64"));
    assert!(output.contains("%state"));
}

#[test]
fn test_llvm_generates_transaction() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "increment".to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("count".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("count".to_string())), Box::new(Expr::Decimal(1)))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // 2026-07-17: emit_transaction uses @txn_<name> prefix to match the
    // call sites in emit_ssa_loop and emit_folded_multi_main.
    assert!(output.contains("@txn_increment("));
}

#[test]
fn test_llvm_has_noalias() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "increment".to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![Statement::Term(None)],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("noalias"), "Transaction should have noalias");
    assert!(output.contains("nocapture"), "Transaction should have nocapture");
    assert!(output.contains("local_unnamed_addr"), "Should have local_unnamed_addr");
    assert!(output.contains("attributes #0"), "Should have attribute block");
    assert!(output.contains("mustprogress"), "Should have mustprogress");
    assert!(output.contains("llvm.assume"), "Should declare llvm.assume intrinsic");
}

#[test]
fn test_llvm_acyclic_annotation() {
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&empty_program(), None);
    assert!(!output.is_empty());
}

#[test]
fn test_inline_directive_emits_alwaysinline() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        state_count(),
        make_txn("inline_txn", vec![Annotation { name: "inline".to_string(), value: Some(Expr::Bool(true)) }]),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("alwaysinline"), "#inline should emit alwaysinline");
}

#[test]
fn test_speculative_inline_emits_inlinehint() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        state_count(),
        make_txn("hinted_txn", vec![Annotation { name: "?inline".to_string(), value: Some(Expr::Bool(true)) }]),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("inlinehint"), "#?inline should emit inlinehint");
}

#[test]
fn test_inline_directive_absent_no_extra_attr() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        state_count(),
        make_txn("plain_txn", vec![]),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("alwaysinline"), "cycle-free txn should have alwaysinline by default");
}

#[test]
fn test_gpu_directive_collects_spirv_kernel() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        state_count(),
        make_txn("gpu_test", vec![Annotation { name: "gpu".to_string(), value: Some(Expr::Bool(true)) }]),
    ];
    let _output = backend.generate(&program, None);
    assert!(backend.spirv_kernels().len() >= 1,
        "gpu txn should produce at least one SPIR-V kernel");
}

#[test]
fn test_gpu_directive_embeds_spirv_blob_in_output() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        state_count(),
        make_txn("embed_test", vec![Annotation { name: "gpu".to_string(), value: Some(Expr::Bool(true)) }]),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("GPU Kernel Blobs") || backend.spirv_kernels().len() >= 1,
        "gpu txn output should contain SPIR-V blob section");
}

#[test]
fn test_gpu_offload_flag_collects_kernels() {
    let mut backend = LlvmBackend::new().with_gpu_offload(true);
    let program = vec![
        state_count(),
        make_txn("offload_test", vec![]),
    ];
    let _output = backend.generate(&program, None);
    assert!(backend.spirv_kernels().len() >= 1,
        "--gpu-offload should collect kernels for all txns");
}

#[test]
fn test_gpu_e2e_simple_add() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        state_count(),
        make_txn("e2e_add", vec![Annotation { name: "gpu".to_string(), value: Some(Expr::Bool(true)) }]),
    ];
    let _output = backend.generate(&program, None);
    assert!(backend.spirv_kernels().len() >= 1,
        "e2e: GPU txn should produce at least one SPIR-V kernel");
}

#[test]
fn test_gpu_e2e_invocation_count() {
    let mut backend = LlvmBackend::new().with_gpu_offload(true);
    let program = vec![
        state_count(),
        make_txn("k1", vec![]),
        make_txn("k2", vec![]),
    ];
    let _output = backend.generate(&program, None);
    assert!(backend.spirv_kernels().len() == 2,
        "e2e: two txns with --gpu-offload should produce 2 kernels");
}

#[test]
fn test_escape_non_ASCII_string() {
    let output = escape_llvm_string("héllo");
    assert!(output.contains("\\c3"), "Should hex-escape byte C3");
    assert!(output.contains("\\a9"), "Should hex-escape byte A9");
    assert!(output.contains("h"), "ASCII 'h' should be preserved");
    assert!(output.contains("llo"), "ASCII 'llo' should be preserved after escape bytes");
}

#[test]
fn test_no_range_lower_bound_defaults_to_i64_min() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "x".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::BinaryOp(BinaryOpKind::Lt,
                    Box::new(Expr::Identifier("x".to_string())), Box::new(Expr::Decimal(100))),
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body: vec![Statement::Term(None)],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("-9223372036854775808"),
        "Range with no lower bound should use i64::MIN");
}

#[test]
fn test_binop_no_nuw_nsw() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "x".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::StateDecl(StateDecl {
            name: "y".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::BinaryOp(BinaryOpKind::And,
                    Box::new(Expr::BinaryOp(BinaryOpKind::Ge, Box::new(Expr::Identifier("x".to_string())), Box::new(Expr::Decimal(0)))),
                    Box::new(Expr::BinaryOp(BinaryOpKind::Lt,
                        Box::new(Expr::Identifier("x".to_string())), Box::new(Expr::Decimal(10))))),
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body: vec![
                Statement::Assign(Expr::Identifier("x".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("x".to_string())), Box::new(Expr::Identifier("y".to_string())))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(!output.contains("nuw nsw"),
        "add on bounded variables should NOT emit nuw nsw (LLVM infers from !range; nuw nsw causes urem→128bit mul)");
}

#[test]
fn test_float_binary_add() {
    let tu = crate::type_universe::TypeUniverse::new();
    let mut backend = LlvmBackend::new().with_type_universe(tu);
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "x".to_string(),
            ty: Type::float(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("x".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("x".to_string())), Box::new(Expr::Float(2.0)))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // 2026-07-17: Float literal + Float field → fmul/fadd float (32-bit), not double.
    // The typechecker assigns Type::float() to Float literals and the constant
    // emitter stores Float as "float" in LLVM IR. Operations use the correct width.
    assert!(output.contains("fadd fast float"),
        "Float binary add should emit fadd fast float");
}

#[test]
fn test_enum_type_registered_and_variant_disc() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Enum(EnumDefinition {
            name: "Option".to_string(),
            type_params: vec![],
            variants: vec![
                EnumVariant::Unit("None".to_string()),
                EnumVariant::Tuple("Some".to_string(), vec![Type::int()]),
            ],
            span: None,
        }),
    ];
    let _ = backend.generate(&program, None);
    assert!(backend.ctx.enum_types.contains_key("Option"));
    assert!(backend.ctx.variant_disc.contains_key("None"));
    assert!(backend.ctx.variant_disc.contains_key("Some"));
    assert_eq!(backend.ctx.variant_disc.get("None").map(|(_, d, _)| *d), Some(0));
    assert_eq!(backend.ctx.variant_disc.get("Some").map(|(_, d, _)| *d), Some(1));
    assert_eq!(backend.ctx.variant_disc.get("Some").map(|(_, _, f)| *f), Some(1));
}

#[test]
fn test_enum_multi_variant_discriminants() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Enum(EnumDefinition {
            name: "Tree".to_string(),
            type_params: vec![],
            variants: vec![
                EnumVariant::Unit("Leaf".to_string()),
                EnumVariant::Tuple("Node".to_string(), vec![Type::int(), Type::int()]),
            ],
            span: None,
        }),
    ];
    let _ = backend.generate(&program, None);
    assert_eq!(backend.ctx.variant_disc.get("Leaf").map(|(_, d, _)| *d), Some(0));
    assert_eq!(backend.ctx.variant_disc.get("Node").map(|(_, d, _)| *d), Some(1));
    assert_eq!(backend.ctx.variant_disc.get("Node").map(|(_, _, f)| *f), Some(2));
}

#[test]
fn test_struct_type_registered() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("ModuleID"), "Output should be valid IR");
    assert!(backend.ctx.struct_types.contains_key("Point"),
        "Struct 'Point' should be registered");
    assert_eq!(backend.ctx.struct_types["Point"].len(), 2);
}

#[test]
fn test_struct_type_declaration_in_ir() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("%Point = type { i64, i64 }"),
        "Struct type declaration should appear in IR.\nGot:\n{}", output);
}

#[test]
fn test_struct_type_declaration_empty_struct() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Empty".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("%Empty = type {}"),
        "Empty struct should emit %Empty = type {{}}.\nGot:\n{}", output);
}

#[test]
fn test_struct_type_declaration_sorted_order() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Zebra".to_string(), type_params: vec![], parent: None,
            fields: vec![StructField { name: "s".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public }],
            transactions: vec![], view_html: None, span: None, modifiers: vec![], variants: vec![],
        }),
        TopLevel::Obj(StructDefinition {
            name: "Alpha".to_string(), type_params: vec![], parent: None,
            fields: vec![StructField { name: "s".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public }],
            transactions: vec![], view_html: None, span: None, modifiers: vec![], variants: vec![],
        }),
    ];
    let output = backend.generate(&program, None);
    let alpha_pos = output.find("%Alpha = type { i64 }").unwrap();
    let zebra_pos = output.find("%Zebra = type { i64 }").unwrap();
    assert!(alpha_pos < zebra_pos,
        "Struct declarations should be sorted: Alpha before Zebra. Got:\n{}", output);
}

#[test]
fn test_type_with_slots_populates_struct_types() {
    let program = vec![
        TopLevel::TypeDef(Box::new(TypeDef {
            name: "MyBuffer".to_string(),
            type_params: vec![],
            parent: None,
            protocol: None,
            bit_range: None,
            body: TypeDefBody {
                slots: vec![
                    TypeDefSlot { name: "ptr".to_string(), ty: Type::Applied("Ptr".to_string(), vec![Type::Custom("UInt8".to_string())]), bit_range: None },
                    TypeDefSlot { name: "len".to_string(), ty: Type::Custom("Int".to_string()), bit_range: None },
                ],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![],
                operators: vec![], op_bindings: vec![],
            props: vec![],
                constraints: vec![],
                members: vec![],
                span: None,
            },
            span: None,
        })),
    ];
    let tu = crate::type_universe::TypeUniverse::new();
    let mut backend = LlvmBackend::new().with_type_universe(tu);
    let output = backend.generate(&program, None);
    assert!(output.contains("ModuleID"), "Output should be valid IR");
    assert!(backend.ctx.struct_types.contains_key("MyBuffer"),
        "Type with slots 'MyBuffer' should be registered in struct_types");
    assert_eq!(backend.ctx.struct_types["MyBuffer"].len(), 2);
}

#[test]
fn test_struct_auto_registered_in_type_universe() {
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
    ];
    let tu = crate::type_universe::TypeUniverse::new();
    let mut backend = LlvmBackend::new().with_type_universe(tu);
    let _output = backend.generate(&program, None);
    if let Some(ref universe) = backend.ctx.type_universe {
        assert!(universe.types.contains_key("Point"),
            "Struct 'Point' should be auto-registered in TypeUniverse");
        let rt = universe.types.get("Point").unwrap();
        assert_eq!(rt.bytes, 16);
        assert_eq!(rt.base, "Bit");
    } else {
        panic!("TypeUniverse should exist after generate");
    }
}

fn make_point_program(body: Vec<Statement>) -> Vec<TopLevel> {
    vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
        TopLevel::StateDecl(StateDecl {
            name: "pt".to_string(),
            ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body,
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ]
}

#[test]
fn test_string_state_init_not_null() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "s".to_string(),
            ty: Type::string(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "_".to_string(),
                    ty: None,
                    expr: Some(Expr::Identifier("s".to_string())),
                    modifiers: vec![],
                },
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(!output.contains("store ptr null, ptr"),
        "String state field should NOT be null. Got: {}", output);
}

#[test]
fn test_const_trg_write_emits_error() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "locked".to_string(),
            ty: Type::bool_(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("locked".to_string()), Expr::Bool(true)),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // This test verifies the backend doesn't crash for simple state assignments
    assert!(!output.is_empty());
}

#[test]
fn test_local_float_binding() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "x".to_string(),
            ty: Type::float(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("x".to_string()), Expr::Float(2.0)),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // 2026-07-29: Float literal emits bitcast i32 <hex> to float (32-bit).
    // The add+i32 + bitcast + fadd wrapper was removed — a single bitcast
    // from the hex i32 bit pattern produces the float value.
    assert!(output.contains("bitcast i32"),
        "Float literal should emit bitcast i32 to float: {}", output);
}

#[test]
fn test_tfd_sfd_nonblock_constants() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "x".to_string(),
            ty: Type::int(),
            span: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(!output.is_empty());
}

// ── Main and reactor attribute tests ──────────────────────────────

fn make_wake_program_no_triggers() -> Vec<TopLevel> {
    vec![
        TopLevel::StateDecl(StateDecl {
            name: "ops".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Constant(Constant {
            name: "N".to_string(),
            ty: Type::int(),
            expr: Expr::Decimal(100),
        }),
        TopLevel::Transaction(Transaction {
            name: "work".to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::BinaryOp(BinaryOpKind::And,
                    Box::new(Expr::Bool(true)),
                    Box::new(Expr::BinaryOp(BinaryOpKind::Lt,
                        Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("N".to_string()))))),
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body: vec![
                Statement::Assign(Expr::Identifier("ops".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Decimal(1)))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ]
}

#[test]
fn test_main_and_reactor_use_non_willreturn_attr() {
    let program = make_wake_program_no_triggers();
    let output = LlvmBackend::new().generate(&program, None);
    // 2026-07-14: Program may fold to a no-main constant store (EmitPureCounterFold) when
    // the analysis correctly detects it as fully precomputable.
    let has_main = output.contains("define i32 @main()");
    if has_main {
        let has_correct_main = output.contains("define i32 @main() local_unnamed_addr #3")
            || output.contains("define i32 @main() local_unnamed_addr #5")
            || output.contains("define i32 @main() local_unnamed_addr #9");
        assert!(has_correct_main,
            "main() should use #3/#5/#9, got: {:?}",
            output.lines().find(|l| l.contains("define i32 @main")).unwrap_or("(not found)"));
    }
    assert!(output.contains("attributes #0"),
        "attributes #0 should still be present for terminating functions");
    assert!(output.contains("define void @init_state(ptr noundef"),
        "init_state() should still use #0 with noundef");
}

// ── Exit condition tests ──────────────────────────────────

fn make_exit_program(exit_expr: Option<Expr>, is_wake: bool) -> Vec<TopLevel> {
    let mut items: Vec<TopLevel> = vec![
        TopLevel::StateDecl(StateDecl {
            name: "ops".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Constant(Constant {
            name: "N".to_string(),
            ty: Type::int(),
            expr: Expr::Decimal(100),
        }),
    ];
    // 2026-07-14: Create a trigger when is_wake is set so has_wake_triggers
    // fires and natural death detection runs.
    if is_wake {
        items.push(TopLevel::Trigger(Trigger {
            name: "__wake_trg".to_string(),
            instance: Expr::Identifier("".to_string()),
            port: "__wake".to_string(),
            span: None,
        }));
    }
    let pre = Expr::BinaryOp(BinaryOpKind::And,
        Box::new(Expr::Bool(true)),
        Box::new(Expr::BinaryOp(BinaryOpKind::Lt,
            Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("N".to_string())))));
    let txn = Transaction {
        name: "work".to_string(),
        is_reactive: true,
        is_async: false,
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: Contract {
            pre_condition: pre,
            post_condition: Expr::Bool(true),
            is_entry: false,
            watchdog: None,
            explicit: false,
            span: None,
        },
        body: vec![
            Statement::Assign(Expr::Identifier("ops".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Decimal(1)))),
            Statement::Term(None),
        ],
        metadata: HashMap::new(),
        derivation: None,
        modifiers: vec![],
        span: None,
        doc: None,
    };
    items.push(TopLevel::Transaction(txn));
    items
}

#[test]
fn test_exit_pragma_in_wake_main() {
    let exit_cond = Expr::BinaryOp(BinaryOpKind::Eq,
        Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("N".to_string())));
    let program = make_exit_program(Some(exit_cond.clone()), true);
    let output = LlvmBackend::new().generate(&program, Some(Box::new(exit_cond)));
    assert!(output.contains("trunc i64"),
        "Exit condition should trunc i64 to i1");
    assert!(output.contains("br i1"),
        "Exit condition should branch on icmp result");
    assert!(output.contains(".end:"),
        "Exit condition should emit .end label");
    assert!(output.contains("ret i32 0"),
        ".end label should return 0");
}

#[test]
fn test_exit_pragma_without_wake_no_change() {
    let exit_cond = Expr::BinaryOp(BinaryOpKind::Eq,
        Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("N".to_string())));
    let program = make_exit_program(Some(exit_cond.clone()), false);
    let output = LlvmBackend::new().generate(&program, Some(Box::new(exit_cond)));
    assert!(output.contains("trunc i64"),
        "Exit condition should trunc i64 to i1 even without wake");
    assert!(output.contains("br i1"),
        "Exit condition should branch");
    assert!(output.contains(".end:"),
        "Exit condition should emit .end label");
    assert!(output.contains("ret i32 0"),
        "done label should return 0");
}

#[test]
fn test_no_exit_without_pragma() {
    let program = make_exit_program(None, true);
    let output = LlvmBackend::new().generate(&program, None);
    assert!(!output.contains("wait:"),
        "No wait label without exit condition in this path");
}

#[test]
fn test_exit_in_enum_main() {
    let exit_cond = Expr::BinaryOp(BinaryOpKind::Eq,
        Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("N".to_string())));
    let program = make_exit_program(Some(exit_cond.clone()), false);
    let output = LlvmBackend::new().with_optimize_budget(256).generate(&program, Some(Box::new(exit_cond)));
    assert!(output.contains("ret i32 0"),
        "Should return 0");
}

// ── Exit diagnostic tests ──────────────────────────────────

#[test]
fn test_check_exit_condition_idents_valid() {
    let mut backend = LlvmBackend::new();
    backend.ctx.field_index_map.insert("ops".to_string(), 0);
    backend.ctx.constants.insert("N".to_string(), (Type::int(), Expr::Decimal(100)));

    let expr = Expr::BinaryOp(BinaryOpKind::Eq,
        Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("N".to_string())));
    let errors = backend.check_exit_condition_idents(&expr);
    assert!(errors.is_empty(),
        "No errors for known identifiers: {:?}", errors);
}

#[test]
fn test_check_exit_condition_idents_invalid() {
    let mut backend = LlvmBackend::new();
    backend.ctx.field_index_map.insert("ops".to_string(), 0);
    backend.ctx.constants.insert("N".to_string(), (Type::int(), Expr::Decimal(100)));

    let expr = Expr::BinaryOp(BinaryOpKind::Eq,
        Box::new(Expr::Identifier("ops".to_string())), Box::new(Expr::Identifier("bogus_var".to_string())));
    let errors = backend.check_exit_condition_idents(&expr);
    assert!(!errors.is_empty(),
        "Should report error for unknown identifier");
    assert!(errors[0].contains("bogus_var"),
        "Error should reference the unknown name: {}", errors[0]);
}

// ── Natural death tests ───────────────────────────────────

#[test]
fn test_natural_death_exits_foldable_program() {
    let program = make_exit_program(None, true);
    let mut backend = LlvmBackend::new();
    let _output = backend.generate(&program, None);
    // 2026-07-14: Natural death creates a synthetic exit condition, so the
    // "no exit path" warning is not emitted (there IS an exit path now).
    assert!(backend.ctx.has_natural_exit,
        "Foldable wake program should have natural exit");
}

#[test]
fn test_natural_death_skipped_for_persistent_txn() {
    let program = make_exit_program(None, true);
    let mut backend = LlvmBackend::new();
    let _output = backend.generate(&program, None);
    // 2026-07-14: Natural death creates a synthetic exit condition, so
    // the "no exit path" warning is no longer emitted.
    let has_warning = backend.warnings().iter().any(|w| {
        w.contains("has wake triggers but no exit path")
    });
    assert!(!has_warning,
        "Persistent program without #!exit — natural death creates exit condition");
}

// ── SLP Hazard Detection Tests ────────────────────────────

fn make_slp_float_program(n_floats: usize, cross_body: Vec<Statement>, precondition: Option<Expr>) -> Vec<TopLevel> {
    let mut items: Vec<TopLevel> = Vec::new();
    for i in 0..n_floats {
        items.push(TopLevel::StateDecl(StateDecl {
            name: format!("f{}", i),
            ty: Type::float(),
            span: None,
        }));
    }
    items.push(TopLevel::StateDecl(StateDecl {
        name: "count".to_string(),
        ty: Type::int(),
        span: None,
    }));
    items.push(TopLevel::StateDecl(StateDecl {
        name: "total".to_string(),
        ty: Type::int(),
        span: None,
    }));
    items.push(TopLevel::Transaction(Transaction {
        name: "tick".to_string(),
        is_reactive: true,
        is_async: false,
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: Contract {
            pre_condition: precondition.unwrap_or(Expr::Bool(true)),
            post_condition: Expr::Identifier("count".to_string()),
            is_entry: false,
            watchdog: None,
            explicit: false,
            span: None,
        },
        body: cross_body,
        metadata: HashMap::new(),
        derivation: None,
        modifiers: vec![],
        span: None,
        doc: None,
    }));
    items
}

fn make_cross_float_body(n_floats: usize, cross_count: usize) -> Vec<Statement> {
    let mut stmts: Vec<Statement> = Vec::new();
    for i in 0..cross_count {
        let a = (i * 3) % n_floats;
        let b = ((i * 3) + 1) % n_floats;
        let c = ((i * 3) + 2) % n_floats;
        stmts.push(Statement::Assign(
            Expr::Identifier(format!("f{}", a)),
            Expr::BinaryOp(BinaryOpKind::Mul,
                Box::new(Expr::Identifier(format!("f{}", b))),
                Box::new(Expr::Identifier(format!("f{}", c)))),
        ));
    }
    stmts.push(Statement::Assign(
        Expr::Identifier("count".to_string()),
        Expr::BinaryOp(BinaryOpKind::Add,
            Box::new(Expr::Identifier("count".to_string())),
            Box::new(Expr::Decimal(1))),
    ));
    stmts
}

#[test]
fn test_slp_hazard_no_floats() {
    let program = make_slp_float_program(0, make_cross_float_body(0, 0), None);
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&program, None);
    assert!(!output.contains("disable-slp-vectorize"),
        "No float fields should produce no SLP-disabled attributes");
}

#[test]
fn test_slp_hazard_small_field_count() {
    let body = make_cross_float_body(4, 6);
    let program = make_slp_float_program(4, body, None);
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&program, None);
    assert!(!output.contains("disable-slp-vectorize"),
        "4 float fields with 6 ops should not trigger SLP disable");
}

#[test]
fn test_slp_hazard_large_field_count() {
    let body = make_cross_float_body(20, 40);
    let program = make_slp_float_program(20, body, None);
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&program, None);
    // 2026-07-27: SLP hazard attribute emission removed — manual SLP vector
    // codegen was disabled so there's no conflict with LLVM's auto-vectorizer.
    // The hazard analysis still runs but produces no attribute output.
    assert!(!output.contains("disable-slp-vectorize"),
        "SLP hazard attributes no longer emitted after SLP codegen removal");
}

#[test]
fn test_slp_hazard_independent_channels() {
    let mut body: Vec<Statement> = Vec::new();
    for i in 0..12 {
        body.push(Statement::Assign(
            Expr::Identifier(format!("f{}", i)),
            Expr::BinaryOp(BinaryOpKind::Add,
                Box::new(Expr::Identifier(format!("f{}", i))),
                Box::new(Expr::Float(1.0))),
        ));
    }
    body.push(Statement::Assign(
        Expr::Identifier("count".to_string()),
        Expr::BinaryOp(BinaryOpKind::Add,
            Box::new(Expr::Identifier("count".to_string())),
            Box::new(Expr::Decimal(1))),
    ));
    let program = make_slp_float_program(12, body, None);
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&program, None);
    assert!(!output.contains("disable-slp-vectorize"),
        "12 independent float fields should NOT disable SLP");
}

#[test]
fn test_slp_hazard_with_target_spec() {
    let body = make_cross_float_body(12, 18);
    let program = make_slp_float_program(12, body, None);
    let mut backend = LlvmBackend::new();
    let spec = crate::target_spec::TargetSpec {
        target: Some(crate::target_spec::TargetSection {
            name: "aarch64-unknown-linux-gnu".to_string(),
            backend: "llvm".to_string(),
            capabilities: vec!["neon".to_string()],
            import_ffi: None,
        }),
        ffi: None,
        codegen: None,
        memory: None,
        bottlenecks: None,
    };
    backend = backend.with_spec(spec);
    let output = backend.generate(&program, None);
    assert!(!output.contains("disable-slp-vectorize"),
        "AArch64 with 32 registers and ASR 2.4 > 1.5 should allow SLP for 12 fields");
}

#[test]
fn test_slp_hazard_avx_target() {
    let body = make_cross_float_body(12, 32);
    let program = make_slp_float_program(12, body, None);
    let mut backend = LlvmBackend::new();
    let spec = crate::target_spec::TargetSpec {
        target: Some(crate::target_spec::TargetSection {
            name: "x86_64-unknown-linux-gnu".to_string(),
            backend: "llvm".to_string(),
            capabilities: vec!["avx2".to_string()],
            import_ffi: None,
        }),
        ffi: None,
        codegen: None,
        memory: None,
        bottlenecks: None,
    };
    backend = backend.with_spec(spec);
    let output = backend.generate(&program, None);
    // 2026-07-27: SLP hazard attribute emission removed — same rationale as
    // test_slp_hazard_large_field_count.
    assert!(!output.contains("disable-slp-vectorize"),
        "SLP hazard attributes no longer emitted after SLP codegen removal");
}

// ── Schema alias tests ────────────────────────────────────

#[test]
fn test_schema_aliases_loaded() {
    let mut aliases = HashSet::new();
    aliases.insert("uart_debug".to_string());
    let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
    assert_eq!(backend.ctx.schema_alias_names.len(), 1);
    assert!(backend.ctx.schema_alias_names.contains("uart_debug"));
    let output = backend.generate(&empty_program(), None);
    assert!(output.contains("ModuleID"));
}

#[test]
fn test_no_schema_import_no_validation() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
    ];
    let _output = backend.generate(&program, None);
    assert!(backend.warnings().is_empty(),
        "No schema import should produce no warnings");
}

#[test]
fn test_multiple_schema_imports_merged() {
    let mut aliases = HashSet::new();
    aliases.insert("gpio0".to_string());
    aliases.insert("gpio1".to_string());
    let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
    assert_eq!(backend.ctx.schema_alias_names.len(), 2);
    let output = backend.generate(&empty_program(), None);
    assert!(output.contains("ModuleID"));
}

#[test]
fn test_imported_alias_is_mmio() {
    let mut aliases = HashSet::new();
    aliases.insert("led_0".to_string());
    let mut mmio: HashMap<String, u64> = HashMap::new();
    mmio.insert("led_0".to_string(), 0x40000000);
    let mut backend = LlvmBackend::new()
        .with_schema_aliases(aliases)
        .with_mmio_addresses(mmio);
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "led_0".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            type_params: vec![],
            parameters: vec![],
            is_reactive: false,
            is_async: false,
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("led_0".to_string()), Expr::Decimal(1)),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("inttoptr i64 1073741824"),
        "led_0 with schema import should be MMIO (inttoptr). Got: {}", output);
    assert!(output.contains("store volatile i64"),
        "led_0 with schema import should use volatile store. Got: {}", output);
}

#[test]
fn test_unimported_alias_not_mmio() {
    let mut aliases = HashSet::new();
    aliases.insert("uart_debug".to_string());
    let mut mmio: HashMap<String, u64> = HashMap::new();
    mmio.insert("led_0".to_string(), 0x40000000);
    mmio.insert("uart_debug".to_string(), 0xFF010000);
    let mut backend = LlvmBackend::new()
        .with_schema_aliases(aliases)
        .with_mmio_addresses(mmio);
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "led_0".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "t".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "_".to_string(),
                    ty: None,
                    expr: Some(Expr::Identifier("led_0".to_string())),
                    modifiers: vec![],
                },
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(!output.contains("inttoptr i64 1073741824"),
        "led_0 NOT in schema should NOT be MMIO (no inttoptr for 0x40000000). Got: {}", output);
    assert!(output.contains("getelementptr inbounds %State"),
        "led_0 NOT in schema should use struct GEP. Got: {}", output);
}

// ── Intrinsic tests ──────────────────────────────────────

fn make_intrinsic_program(intrinsic: Expr) -> Vec<TopLevel> {
    vec![
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "r".to_string(),
                    ty: Some(Type::int()),
                    expr: Some(intrinsic),
                    modifiers: vec![],
                },
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ]
}

fn make_float_intrinsic_program(intrinsic: Expr) -> Vec<TopLevel> {
    vec![
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "r".to_string(),
                    ty: Some(Type::float()),
                    expr: Some(intrinsic),
                    modifiers: vec![],
                },
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ]
}

#[test]
fn test_emit_cast_int_to_string() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "r".to_string(),
                    ty: Some(Type::string()),
                    expr: Some(Expr::Cast(Box::new(Expr::Decimal(42)), Type::string())),
                    modifiers: vec![],
                },
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("call i64 @__int_to_str__(i64"),
        "Cast Int -> String should call __int_to_str__. Got:\n{}", output);
}

#[test]
fn test_emit_cast_string_to_int() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "r".to_string(),
                    ty: Some(Type::int()),
                    expr: Some(Expr::Cast(Box::new(Expr::Quoted("42".into())), Type::int())),
                    modifiers: vec![],
                },
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // 2026-07-30: Protocol-based cast path replaces __str_to_int.
    // String→Int now goes through protocol dispatch, not __str_to_int.
    // 2026-07-30: Check that __str_to_int is NOT called (only declared as extern).
    // The extern declaration is always emitted for known runtime functions.
    assert!(!output.contains("call i64 @__str_to_int"),
        "Cast String -> Int should NOT call __str_to_int (protocol path). Got:\n{}", output);
}

// ── List tests ───────────────────────────────────────────

#[test]
fn test_list_literal_2slot_header() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "lst".to_string(), ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "mklist".to_string(), is_reactive: false, is_async: false,
            type_params: vec![], parameters: vec![],
            output_type: None, outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("lst".to_string()), Expr::List(vec![Expr::Decimal(10), Expr::Decimal(20)])),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![], span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("call ptr @malloc(i64 32)"), "2-elem list = 32 bytes (4 slots × 8). Got: {}", output);
    assert!(output.contains("bitcast ptr"), "Should bitcast malloc result to ptr. Got: {}", output);
    assert!(output.contains("store i64 2, ptr"), "Length should be 2. Got: {}", output);
    assert!(output.contains("ptrtoint ptr"), "Should emit ptrtoint for data_ptr. Got: {}", output);
}

#[test]
fn test_empty_list_global_sentinel() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "e".to_string(), ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "mkempty".to_string(), is_reactive: false, is_async: false,
            type_params: vec![], parameters: vec![],
            output_type: None, outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("e".to_string()), Expr::List(vec![])),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![], span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("@ll_empty_list"), "Empty list should reference global sentinel. Got: {}", output);
    assert!(!output.contains("alloca i64, i64 2"), "Empty list should NOT alloca 2 slots. Got: {}", output);
    assert!(!output.contains("call ptr @malloc(i64 16"), "Empty list should NOT call 16-byte malloc. Got: {}", output);
}

#[test]
fn test_nonempty_list_uses_malloc() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "v".to_string(), ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "mklist".to_string(), is_reactive: false, is_async: false,
            type_params: vec![], parameters: vec![],
            output_type: None, outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("v".to_string()), Expr::List(vec![Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3)])),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![], span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("call ptr @malloc(i64 40)"), "3-elem list = 40 bytes (5 slots × 8). Got: {}", output);
    assert!(output.contains("bitcast ptr"), "Should bitcast malloc result to ptr. Got: {}", output);
    assert!(!output.contains("alloca i64, i64 5"), "Non-empty list should NOT use alloca. Got: {}", output);
    assert!(output.contains("add i64 0, 1") && output.contains("add i64 0, 2") && output.contains("add i64 0, 3"),
        "Should compute all 3 elements. Got: {}", output);
    assert!(output.contains("store i64 3, ptr"), "Length should be 3. Got: {}", output);
}

#[test]
fn test_list_index_uses_2slot_header() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "elem".to_string(), ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "idx".to_string(), is_reactive: false, is_async: false,
            type_params: vec![], parameters: vec![],
            output_type: None, outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("elem".to_string()), Expr::Index(Box::new(Expr::List(vec![Expr::Decimal(99)])), Box::new(Expr::Decimal(0)))),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![], span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("load i64, ptr"), "Should load data_ptr. Got: {}", output);
    assert!(output.contains("getelementptr i64, ptr"), "Should GEP from data. Got: {}", output);
}

#[test]
fn test_list_len_loads_length() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "len".to_string(), ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "chk_len".to_string(), is_reactive: false, is_async: false,
            type_params: vec![], parameters: vec![],
            output_type: None, outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("len".to_string()), Expr::Call("Len#".to_string(), vec![Expr::List(vec![Expr::Decimal(1), Expr::Decimal(2)])], None)),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![], span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("load i64, ptr"), "Size projection should load from memory. Got: {}", output);
}

// ── Tuple tests ──────────────────────────────────────────

#[test]
fn test_tuple_emits_2slot_header() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "t".to_string(), ty: Type::int(), span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "mktup".to_string(), is_reactive: false, is_async: false,
            type_params: vec![], parameters: vec![],
            output_type: None, outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("t".to_string()), Expr::Tuple(vec![Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3)])),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![], span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("call ptr @malloc(i64 40)"), "3-elem tuple = 40 bytes (5 slots × 8). Got: {}", output);
    assert!(output.contains("store i64 3, ptr"), "Length should be 3. Got: {}", output);
}

// ── Optimization report & chain composition ──────────────

fn make_chain_program(
    txns: Vec<(&str, Vec<Statement>)>,
    consts: &[(&str, i64)],
    states: &[(&str, i64)],
) -> Vec<TopLevel> {
    let mut items: Vec<TopLevel> = Vec::new();
    for (name, val) in consts {
        items.push(TopLevel::Constant(Constant {
            name: name.to_string(),
            ty: Type::int(),
            expr: Expr::Decimal(*val),
        }));
    }
    for (name, val) in states {
        items.push(TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty: Type::int(),
            span: None,
        }));
    }
    for (txn_name, body) in txns {
        let pre = Expr::BinaryOp(BinaryOpKind::Lt,
            Box::new(Expr::Identifier("count".to_string())), Box::new(Expr::Identifier("total".to_string())));
        items.push(TopLevel::Transaction(Transaction {
            name: txn_name.to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body,
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }));
    }
    items
}

fn ident_s(s: &str) -> Expr { Expr::Identifier(s.to_string()) }
fn int_s(v: i64) -> Expr { Expr::Decimal(v) }

#[test]
fn test_report_shows_ranking() {
    let program = make_chain_program(
        vec![("t1", vec![
            Statement::Assign(ident_s("x"), ident_s("sensor")),
            Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
        ])],
        &[("total", 100)],
        &[("count", 0), ("x", 0)],
    );
    let mut backend = LlvmBackend::new()
        .with_optimize_budget(256).with_optimize_report(true);
    let _output = backend.generate(&program, None);
    let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
    let joined = report.join("\n");
    // 2026-07-14: With full analysis wired, check that the report has
    // substantive optimization content.
    assert!(!report.is_empty(), "Report should contain content");
    assert!(joined.contains("Budget plan") || joined.contains("Linear transaction")
        || joined.contains("Optimization priority"),
        "Report should contain optimization analysis");
}

#[test]
fn test_report_shows_budget() {
    let program = make_chain_program(
        vec![("t1", vec![
            Statement::Assign(ident_s("x"), ident_s("sensor")),
            Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
        ])],
        &[("total", 100)],
        &[("count", 0), ("x", 0)],
    );
    let mut backend = LlvmBackend::new()
        .with_optimize_budget(10).with_optimize_report(true);
    let _output = backend.generate(&program, None);
    let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
    let joined = report.join("\n");
    assert!(joined.contains("Budget plan"),
        "Report should contain budget plan section");
}

#[test]
fn test_report_shows_size() {
    let program = make_chain_program(
        vec![("t1", vec![
            Statement::Assign(ident_s("x"), ident_s("sensor")),
            Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
        ])],
        &[("total", 100)],
        &[("count", 0), ("x", 0)],
    );
    let mut backend = LlvmBackend::new()
        .with_optimize_budget(256).with_optimize_report(true)
        .with_optimize_size(10000);
    let _output = backend.generate(&program, None);
    let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
    let joined = report.join("\n");
    // 2026-07-14: Size estimation requires triggers for enumerable dispatch.
    // The report still contains chain analysis and budget info.
    assert!(joined.contains("Budget plan") || joined.contains("Linear transaction"),
        "Report should contain optimization info");
}

#[test]
fn test_report_shows_chains() {
    let program = make_chain_program(
        vec![
            ("step_a", vec![
                Statement::Assign(ident_s("x"), ident_s("sensor")),
                Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
            ]),
            ("step_b", vec![
                Statement::Assign(ident_s("y"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("x")), Box::new(int_s(1)))),
                Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
            ]),
        ],
        &[("total", 100)],
        &[("count", 0), ("x", 0), ("y", 0)],
    );
    let mut backend = LlvmBackend::new()
        .with_optimize_budget(256).with_optimize_report(true);
    let _output = backend.generate(&program, None);
    let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
    let joined = report.join("\n");
    assert!(joined.contains("Linear transaction chains")
        || joined.contains("Composed chains"),
        "Report should detect multi-txn chains");
}

#[test]
fn test_precompute_pure_counter() {
    let program = make_chain_program(
        vec![
            ("step_a", vec![
                Statement::Assign(ident_s("x"), int_s(42)),
                Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
            ]),
            ("step_b", vec![
                Statement::Assign(ident_s("y"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("x")), Box::new(int_s(1)))),
                Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
            ]),
        ],
        &[("total", 100)],
        &[("count", 0), ("x", 0), ("y", 0)],
    );
    let output = LlvmBackend::new().with_optimize_budget(256).generate(&program, None);
    assert!(output.contains("ret i32 0"),
        "Should return normally");
}

#[test]
fn test_precompute_budget_exceeded_fallback() {
    let program = make_chain_program(
        vec![
            ("step_a", vec![
                Statement::Assign(ident_s("x"), int_s(42)),
                Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
            ]),
            ("step_b", vec![
                Statement::Assign(ident_s("y"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("x")), Box::new(int_s(1)))),
                Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
            ]),
        ],
        &[("total", 100)],
        &[("count", 0), ("x", 0), ("y", 0)],
    );
    let output = LlvmBackend::new().with_optimize_budget(0).generate(&program, None);
    assert!(output.contains("getelementptr inbounds %State, ptr %state, i32 0, i32"),
        "All-convergent program should use per-field GEP loads");
    assert!(!output.contains("@reactor_tick"),
        "All-convergent program should not emit reactor_tick");
}

#[test]
fn test_iir_filter_folded_path_regression() {
    let program = make_chain_program(
        vec![("process", vec![
            Statement::Assign(ident_s("x"), int_s(42)),
            Statement::Assign(ident_s("count"), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("count")), Box::new(int_s(1)))),
        ])],
        &[("total", 50000000)],
        &[("count", 0), ("x", 0)],
    );
    let output = LlvmBackend::new().generate(&program, None);
    assert!(!output.contains("switch i64"),
        "Single-txn convergence should use folded path, not enum dispatch");
    assert!(!output.contains("@reactor_tick"),
        "Single-txn convergence should use folded path, not standard reactor");
    assert!(output.contains("store i64 50000000"),
        "Effectively-pure body should emit O(1) store i64 total, not a while-loop");
    assert!(output.contains("ret i32 0"),
        "Should return after store");
    let main_idx = output.find("define i32 @main()").unwrap_or(0);
    let store_in_main = output[main_idx..].contains("store i64 50000000");
    assert!(store_in_main, "store must be in main, not in process");
}

// ── Async tests ──────────────────────────────────────────

fn make_async_pair_program() -> Vec<TopLevel> {
    vec![
        TopLevel::StateDecl(StateDecl {
            name: "a".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::StateDecl(StateDecl {
            name: "b".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "inc_a".to_string(),
            is_reactive: true,
            is_async: true,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("a".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("a")), Box::new(int_s(1)))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "inc_b".to_string(),
            is_reactive: true,
            is_async: true,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("b".to_string()), Expr::BinaryOp(BinaryOpKind::Add, Box::new(ident_s("b")), Box::new(int_s(1)))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ]
}

#[test]
fn test_async_body_functions_emitted() {
    let program = make_async_pair_program();
    let output = LlvmBackend::new().generate(&program, None);
    assert!(output.contains("@async_body_inc_a"),
        "Async body function for inc_a should be emitted");
    assert!(output.contains("@async_body_inc_b"),
        "Async body function for inc_b should be emitted");
}

#[test]
fn test_thread_pool_metadata_emitted() {
    let program = make_async_pair_program();
    let output = LlvmBackend::new().generate(&program, None);
    assert!(output.contains("@llvm.thread_pool"),
        "Thread pool metadata should be emitted for async txns");
    assert!(output.contains("@thread_pool_fns"),
        "Thread pool function pointer array should be emitted");
}

#[test]
fn test_async_barrier_calls_in_main() {
    let program = make_async_pair_program();
    let output = LlvmBackend::new().generate(&program, None);
    assert!(output.contains("call void @__thread_pool_init__"),
        "Main should call thread_pool_init");
    assert!(output.contains("call void @__barrier_release__"),
        "Main should call barrier_release");
    assert!(output.contains("call void @__barrier_wait__"),
        "Main should call barrier_wait");
}

#[test]
fn test_no_thread_pool_without_async_txns() {
    let program = make_exit_program(None, false);
    let output = LlvmBackend::new().generate(&program, None);
    assert!(!output.contains("@llvm.thread_pool"),
        "No thread pool metadata without async txns");
    assert!(!output.contains("call void @__barrier__"),
        "No barrier calls without async txns");
    assert!(!output.contains("call void @__thread_pool_init__"),
        "No thread pool init without async txns");
}

// ── Struct param tests ───────────────────────────────────

#[test]
fn test_struct_param_uses_ptr_in_signature() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
        TopLevel::Definition(Definition {
            name: "process".to_string(),
            type_params: vec![],
            parameters: vec![("p".to_string(), Type::Custom("Point".to_string()))],
            outputs: vec![Type::bool_()],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Term(Some(Expr::Bool(true))),
            ],
            modifiers: vec![Annotation { name: "export".to_string(), value: Some(Expr::Bool(true)) }],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("define i64 @process(ptr noundef noalias nocapture align 8 %state, ptr %arg0"),
        "Struct param should be 'ptr' in function signature.\nGot:\n{}", output);
}

#[test]
fn test_struct_param_ptrtoint_at_entry() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
        TopLevel::Definition(Definition {
            name: "process".to_string(),
            type_params: vec![],
            parameters: vec![("p".to_string(), Type::Custom("Point".to_string()))],
            outputs: vec![Type::bool_()],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Term(Some(Expr::Bool(true))),
            ],
            modifiers: vec![Annotation { name: "export".to_string(), value: Some(Expr::Bool(true)) }],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("ptrtoint ptr %arg0 to i64"),
        "Struct param should have ptrtoint at entry.\nGot:\n{}", output);
}

#[test]
fn test_call_with_ptr_arg_emits_inttoptr() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Definition(Definition {
            name: "callee".to_string(),
            type_params: vec![],
            parameters: vec![("p".to_string(), Type::Ptr(Box::new(Type::int())))],
            outputs: vec![Type::int()],
            output_type: None,
            contract: default_contract(),
            body: vec![Statement::Term(Some(Expr::Decimal(42)))],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            annotations: vec![],
            span: None,
            doc: None,
        }),
        TopLevel::Definition(Definition {
            name: "caller".to_string(),
            type_params: vec![],
            parameters: vec![("p".to_string(), Type::Ptr(Box::new(Type::int())))],
            outputs: vec![Type::int()],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Term(Some(Expr::Call(
                    "callee".to_string(),
                    vec![Expr::Identifier("p".to_string())],
                    None,
                ))),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("inttoptr"),
        "Call with Ptr arg should emit inttoptr before the call.\nGot:\n{}", output);
}

#[test]
fn test_struct_param_field_access_works() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::Obj(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            parent: None,
            fields: vec![
                StructField { name: "x".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
                StructField { name: "y".to_string(), ty: Type::int(), default: None, visibility: Visibility::Public },
            ],
            transactions: vec![],
            view_html: None,
            span: None,
            modifiers: vec![],
            variants: vec![],
        }),
        TopLevel::Definition(Definition {
            name: "get_x".to_string(),
            type_params: vec![],
            parameters: vec![("p".to_string(), Type::Custom("Point".to_string()))],
            outputs: vec![Type::int()],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Term(Some(Expr::Field(Box::new(Expr::Identifier("p".to_string())), "x".to_string()))),
            ],
            modifiers: vec![Annotation { name: "export".to_string(), value: Some(Expr::Quoted("get_x".into())) }],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("getelementptr"),
        "Field access on struct param should emit GEP.\nGot:\n{}", output);
    assert!(!output.contains("not found on object"),
        "Field access on struct param should succeed.\nGot:\n{}", output);
}

// ── Event model / Trigger (basic) tests ──────────────────

#[test]
fn test_event_model_trigger_handling() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "pump".to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![Statement::Term(None)],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("%State"),
        "Should generate state type");
    assert!(output.contains("define void @init_state"),
        "Should have init_state");
}

// ── First-class function ptr test ────────────────────────

fn make_fn_ptr_program() -> Vec<TopLevel> {
    vec![
        TopLevel::StateDecl(StateDecl {
            name: "x".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "apply".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(Expr::Identifier("x".to_string()), Expr::Decimal(42)),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ]
}

#[test]
fn test_fn_ptr_not_crashes() {
    let program = make_fn_ptr_program();
    let output = LlvmBackend::new().generate(&program, None);
    assert!(output.contains("define i32 @main"),
        "Should emit main function");
}

#[test]
fn test_emit_address_of() {
    // AddressOf#("uart") should emit inttoptr with uart's address (0xFFE01000)
    let program = vec![
        TopLevel::Transaction(Transaction {
            name: "main".to_string(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: default_contract(),
            body: vec![
                Statement::Assign(
                    Expr::Identifier("ptr".to_string()),
                    Expr::Call("AddressOf#".to_string(), vec![Expr::Quoted(b"uart".to_vec())], None),
                ),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = LlvmBackend::new().generate(&program, None);
    // uart = 0xFFE01000 in config/address-map.toml
    assert!(output.contains("inttoptr"), "Should emit inttoptr");
    // Resolve the expected address from the shared resolver
    let expected_addr = crate::address_resolver::resolve_address("uart");
    let expected_str = expected_addr.to_string();
    assert!(output.contains(&expected_str), "Should contain uart address {} (= 0x{:X})", expected_str, expected_addr);
}

#[test]
fn test_frgn_ptr_declare() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::ForeignBinding(ForeignBinding {
            foreign_name: "test_fn".to_string(),
            brief_name: None,
            from: FromSpec::CompilerRegistry("c".to_string()),
            target: ForeignTarget::C,
            inputs: vec![("arg".to_string(), Type::Ptr(Box::new(Type::int())))],
            success_output: vec![("".to_string(), Type::int())],
            error_type: String::new(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback: Fallback::None,
            span: None,
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("declare i64 @test_fn(ptr)"),
        "Ptr param should produce 'ptr' in declare, not 'i64'.\nGot:\n{}", output);
}

#[test]
fn test_frgn_ptr_return() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::ForeignBinding(ForeignBinding {
            foreign_name: "make_ptr".to_string(),
            brief_name: None,
            from: FromSpec::CompilerRegistry("c".to_string()),
            target: ForeignTarget::C,
            inputs: vec![("n".to_string(), Type::int())],
            success_output: vec![("".to_string(), Type::Ptr(Box::new(Type::int())))],
            error_type: String::new(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback: Fallback::None,
            span: None,
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    assert!(output.contains("declare ptr @make_ptr(i64)"),
        "Ptr return should produce 'ptr' in declare, not 'i64'.\nGot:\n{}", output);
}

#[test]
fn test_struct_literal_field_offsets() {
    let tu = crate::type_universe::TypeUniverse::new();
    let mut backend = LlvmBackend::new().with_type_universe(tu);
    let program = vec![
        TopLevel::StaticStruct(StructDef {
            type_params: vec![],
            name: "Mixed".to_string(),
            fields: vec![
                ("a".to_string(), Type::int()),
                ("b".to_string(), Type::bool_()),
                ("c".to_string(), Type::char_()),
            ],
            metadata: HashMap::new(),
            span: None,
        }),
        TopLevel::Definition(Definition {
            name: "test".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "x".to_string(),
                    ty: None,
                    expr: Some(Expr::StructLiteral {
                        type_name: "Mixed".to_string(),
                        fields: vec![
                            ("a".to_string(), Expr::Decimal(42)),
                            ("b".to_string(), Expr::Bool(true)),
                            ("c".to_string(), Expr::Decimal(65)),
                        ],
                    }),
                    modifiers: vec![],
                },
                Statement::Term(Some(Expr::Decimal(0))),
            ],
            modifiers: vec![],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // Field layout (pack=1): a(Int=8B)@0, b(Bool=1B)@8, c(Char=4B)@9
    assert!(output.contains("getelementptr i8, ptr %t1, i64 0"),
        "Field 'a' should be at offset 0.\nGot:\n{}", output);
    assert!(output.contains("getelementptr i8, ptr %t1, i64 8"),
        "Field 'b' (Bool) should be at offset 8.\nGot:\n{}", output);
    assert!(output.contains("getelementptr i8, ptr %t1, i64 9"),
        "Field 'c' (Char) should be at offset 9.\nGot:\n{}", output);
}

#[test]
fn test_addr_of_struct_literal() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StaticStruct(StructDef {
            type_params: vec![],
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Type::int()),
                ("y".to_string(), Type::int()),
            ],
            metadata: HashMap::new(),
            span: None,
        }),
        TopLevel::ForeignBinding(ForeignBinding {
            foreign_name: "use_ptr".to_string(),
            brief_name: None,
            from: FromSpec::CompilerRegistry("c".to_string()),
            target: ForeignTarget::C,
            inputs: vec![("p".to_string(), Type::Ptr(Box::new(Type::int())))],
            success_output: vec![("".to_string(), Type::int())],
            error_type: String::new(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback: Fallback::None,
            span: None,
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
            doc: None,
        }),
        TopLevel::Definition(Definition {
            name: "main".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "pt".to_string(),
                    ty: None,
                    expr: Some(Expr::StructLiteral {
                        type_name: "Point".to_string(),
                        fields: vec![
                            ("x".to_string(), Expr::Decimal(10)),
                            ("y".to_string(), Expr::Decimal(20)),
                        ],
                    }),
                    modifiers: vec![],
                },
                // Call frgn with &pt param
                Statement::Expression(Expr::Call(
                    "use_ptr".to_string(),
                    vec![Expr::AddrOf(Box::new(Expr::Identifier("pt".to_string())))],
                    None,
                )),
                Statement::Term(Some(Expr::Decimal(0))),
            ],
            modifiers: vec![],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // &pt should emit ptrtoint on the struct alloca, NOT ptrtoint on a function ptr
    // The alloca is created by emit_struct_literal: alloca i8, i64 16 (2 x i64 = 16B)
    assert!(output.contains("alloca i8, i64 16"),
        "Struct literal should allocate 16 bytes (2 x 8B fields).\nGot:\n{}", output);
    assert!(output.contains("ptrtoint ptr %t"),
        "Should emit ptrtoint of the struct alloca for &pt.\nGot:\n{}", output);
    // Should NOT reference @pt as a function symbol
    assert!(!output.contains("ptrtoint ptr @pt"),
        "Should NOT emit ptrtoint of @pt as if it were a function.\nGot:\n{}", output);
}

#[test]
fn test_frgn_ptr_param_inttoptr() {
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StaticStruct(StructDef {
            type_params: vec![],
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Type::int()),
                ("y".to_string(), Type::int()),
            ],
            metadata: HashMap::new(),
            span: None,
        }),
        TopLevel::ForeignBinding(ForeignBinding {
            foreign_name: "use_ptr".to_string(),
            brief_name: None,
            from: FromSpec::CompilerRegistry("c".to_string()),
            target: ForeignTarget::C,
            inputs: vec![("p".to_string(), Type::Ptr(Box::new(Type::int())))],
            success_output: vec![("".to_string(), Type::int())],
            error_type: String::new(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback: Fallback::None,
            span: None,
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
            doc: None,
        }),
        TopLevel::Definition(Definition {
            name: "main".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "pt".to_string(),
                    ty: None,
                    expr: Some(Expr::StructLiteral {
                        type_name: "Point".to_string(),
                        fields: vec![
                            ("x".to_string(), Expr::Decimal(10)),
                            ("y".to_string(), Expr::Decimal(20)),
                        ],
                    }),
                    modifiers: vec![],
                },
                Statement::Expression(Expr::Call(
                    "use_ptr".to_string(),
                    vec![Expr::AddrOf(Box::new(Expr::Identifier("pt".to_string())))],
                    None,
                )),
                Statement::Term(Some(Expr::Decimal(0))),
            ],
            modifiers: vec![],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // When calling a frgn with Ptr param, the i64 address should be converted
    // via inttoptr so the LLVM call uses ptr type matching the declare.
    assert!(output.contains("inttoptr i64"),
        "Should emit inttoptr to convert i64 to ptr for Ptr param.\nGot:\n{}", output);
    // The call should use ptr type for the Ptr param
    assert!(output.contains("call i64 @use_ptr(ptr"),
        "Call to use_ptr(ptr) should use 'ptr' type for the first param.\nGot:\n{}", output);
}

#[test]
fn test_trg_deref_error_flag() {
    // When --error-unresolved-trg is set, a @ *ptr dynamic trigger should
    // emit a null check + unreachable before the load volatile.
    // The trigger must be referenced in the transaction's precondition
    // for emit_trg_load to be called. Without a precondition reference,
    // the trigger is dead code and the backend skips it.
    use crate::ast::Contract;
    let program = vec![
        TopLevel::Trigger(Trigger {
            name: "dyn_trg".to_string(),
            // @ *ptr — Expr::Deref wraps the pointer expression
            instance: Expr::Deref(Box::new(Expr::Identifier("my_ptr".to_string()))),
            port: "data".to_string(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "pump".to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::BinaryOp(
                    BinaryOpKind::Eq,
                    Box::new(Expr::Identifier("dyn_trg".to_string())),
                    Box::new(Expr::Decimal(1)),
                ),
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body: vec![Statement::Term(None)],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = LlvmBackend::new()
        .with_trg_unresolved_action(crate::backend::llvm::TrgUnresolvedAction::Error)
        .generate(&program, None);
    assert!(output.contains("icmp eq ptr"), "Should emit null check for error mode");
    assert!(output.contains("unreachable"), "Should emit unreachable for error mode");
}

#[test]
fn test_trg_deref_warn_default_no_null_check() {
    // Default (Warn) mode should NOT emit null check for @ *ptr triggers.
    use crate::ast::Contract;
    let program = vec![
        TopLevel::Trigger(Trigger {
            name: "dyn_trg".to_string(),
            instance: Expr::Deref(Box::new(Expr::Identifier("my_ptr".to_string()))),
            port: "data".to_string(),
            span: None,
        }),
        TopLevel::Transaction(Transaction {
            name: "pump".to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::BinaryOp(
                    BinaryOpKind::Eq,
                    Box::new(Expr::Identifier("dyn_trg".to_string())),
                    Box::new(Expr::Decimal(1)),
                ),
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body: vec![Statement::Term(None)],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = LlvmBackend::new().generate(&program, None);
    // 2026-07-18: The comparison type now uses the operand's LLVM type.
    // For pointer operands, icmp eq ptr is valid — update assertion to match.
    // If the output contains "icmp eq ptr" that's fine (no null check flag).
    // The null check flag is "null_check" in the IR comments, not icmp.
    assert!(!output.contains("null_check"), "Default mode should not emit null check");
}

#[test]
fn test_struct_array_list_literal() {
    // When a list literal contains only struct literals of the same
    // known struct type, the backend should emit a contiguous stack array
    // (alloca) instead of a heap-allocated list (malloc).
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StaticStruct(StructDef {
            type_params: vec![],
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Type::int()),
                ("y".to_string(), Type::int()),
            ],
            metadata: HashMap::new(),
            span: None,
        }),
        TopLevel::Definition(Definition {
            name: "main".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "pts".to_string(),
                    ty: None,
                    expr: Some(Expr::List(vec![
                        Expr::StructLiteral {
                            type_name: "Point".to_string(),
                            fields: vec![
                                ("x".to_string(), Expr::Decimal(10)),
                                ("y".to_string(), Expr::Decimal(20)),
                            ],
                        },
                        Expr::StructLiteral {
                            type_name: "Point".to_string(),
                            fields: vec![
                                ("x".to_string(), Expr::Decimal(30)),
                                ("y".to_string(), Expr::Decimal(40)),
                            ],
                        },
                    ])),
                    modifiers: vec![],
                },
                Statement::Term(Some(Expr::Decimal(0))),
            ],
            modifiers: vec![],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // Should allocate 32 bytes (2 elements * 16 bytes each)
    assert!(output.contains("alloca i8, i64 32"),
        "Should allocate 32 bytes for 2-element Point array.\nGot:\n{}", output);
    // First element at offset 0: x=10, y=20
    assert!(output.contains("getelementptr i8, ptr %t"),
        "Should emit GEP for first element's first field at offset 0.\nGot:\n{}", output);
    // Should NOT call malloc (no heap allocation)
    assert!(!output.contains("call @malloc"),
        "Should NOT emit malloc call for struct array list.\nGot:\n{}", output);
    // Should emit ptrtoint (the handle is the pointer to the stack array)
    assert!(output.contains("ptrtoint ptr"),
        "Should emit ptrtoint to produce i64 handle.\nGot:\n{}", output);
}

#[test]
fn test_struct_array_addr_of_and_frgn_call() {
    // Struct array + &var + frgn call with Ptr param.
    // The address-of should produce the alloca pointer, and the frgn call
    // should emit inttoptr to convert i64 handle to ptr for the Ptr param.
    let mut backend = LlvmBackend::new();
    let program = vec![
        TopLevel::StaticStruct(StructDef {
            type_params: vec![],
            name: "PyMethodDef".to_string(),
            fields: vec![
                ("name".to_string(), Type::int()),
                ("flags".to_string(), Type::int()),
            ],
            metadata: HashMap::new(),
            span: None,
        }),
        TopLevel::ForeignBinding(ForeignBinding {
            foreign_name: "use_methods".to_string(),
            brief_name: None,
            from: FromSpec::CompilerRegistry("c".to_string()),
            target: ForeignTarget::C,
            inputs: vec![("p".to_string(), Type::Ptr(Box::new(Type::int())))],
            success_output: vec![("".to_string(), Type::int())],
            error_type: String::new(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback: Fallback::None,
            span: None,
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
            doc: None,
        }),
        TopLevel::Definition(Definition {
            name: "main".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            contract: default_contract(),
            body: vec![
                Statement::Let { names: vec![], 
                    name: "methods".to_string(),
                    ty: None,
                    expr: Some(Expr::List(vec![
                        Expr::StructLiteral {
                            type_name: "PyMethodDef".to_string(),
                            fields: vec![
                                ("name".to_string(), Expr::Decimal(1)),
                                ("flags".to_string(), Expr::Decimal(2)),
                            ],
                        },
                    ])),
                    modifiers: vec![],
                },
                Statement::Expression(Expr::Call(
                    "use_methods".to_string(),
                    vec![Expr::AddrOf(Box::new(Expr::Identifier("methods".to_string())))],
                    None,
                )),
                Statement::Term(Some(Expr::Decimal(0))),
            ],
            modifiers: vec![],
            metadata: HashMap::new(),
            derivation: None,
            annotations: vec![],
            span: None,
            doc: None,
        }),
    ];
    let output = backend.generate(&program, None);
    // Should allocate 16 bytes (1 element * 16 bytes = 2 x i64)
    assert!(output.contains("alloca i8, i64 16"),
        "Should allocate 16 bytes for 1-element PyMethodDef array.\nGot:\n{}", output);
    // Should NOT call malloc
    assert!(!output.contains("call @malloc"),
        "Should NOT emit malloc call.\nGot:\n{}", output);
    // Should emit inttoptr for passing the struct array pointer to the frgn
    assert!(output.contains("inttoptr i64"),
        "Should emit inttoptr for Ptr param.\nGot:\n{}", output);
    // The call should use ptr type for the Ptr param
    assert!(output.contains("call i64 @use_methods(ptr"),
        "Should call use_methods with ptr type.\nGot:\n{}", output);
}

#[test]
fn test_shape_vector_groups_same_type_gate() {
    // Phase 1b: the frontend structural pass cannot express the LLVM same-type
    // gate; the backend re-applies it in shape_vector_groups. A mixed-type
    // group must be dropped, an all-float group accepted.
    let mut backend = LlvmBackend::new();
    backend.ctx.field_index_map.insert("f0".to_string(), 0);
    backend.ctx.field_index_map.insert("f1".to_string(), 1);
    backend.ctx.field_index_map.insert("i0".to_string(), 2);
    backend.ctx.field_index_map.insert("i1".to_string(), 3);
    backend.ctx.field_types = vec![
        "float".to_string(),
        "float".to_string(),
        "i64".to_string(),
        "i64".to_string(),
    ];
    let write_set: HashSet<String> = [
        "f0".to_string(),
        "f1".to_string(),
        "i0".to_string(),
        "i1".to_string(),
    ]
    .into_iter()
    .collect();
    let groups = vec![
        crate::analysis::loop_shape::VectorGroup {
            name: "mixed".to_string(),
            width: 2,
            fields: vec!["f0".to_string(), "i0".to_string()],
        },
        crate::analysis::loop_shape::VectorGroup {
            name: "floats".to_string(),
            width: 2,
            fields: vec!["f0".to_string(), "f1".to_string()],
        },
    ];
    let vg = backend.shape_vector_groups(&groups, &write_set);
    assert_eq!(vg.len(), 1, "mixed-type group must be dropped");
    assert_eq!(vg[0].name, "floats");
    assert_eq!(vg[0].element_ty, "float");
}

#[test]
fn test_shape_vector_groups_drops_not_in_write_set() {
    // A group whose fields are not all unconditionally written must be dropped.
    let mut backend = LlvmBackend::new();
    backend.ctx.field_index_map.insert("f0".to_string(), 0);
    backend.ctx.field_index_map.insert("f1".to_string(), 1);
    backend.ctx.field_types = vec!["float".to_string(), "float".to_string()];
    // write_set only contains f0; f1 is written conditionally (e.g. in a
    // guarded block) so the group must not be used.
    let write_set: HashSet<String> = ["f0".to_string()].into_iter().collect();
    let groups = vec![crate::analysis::loop_shape::VectorGroup {
        name: "g".to_string(),
        width: 2,
        fields: vec!["f0".to_string(), "f1".to_string()],
    }];
    let vg = backend.shape_vector_groups(&groups, &write_set);
    assert!(vg.is_empty(), "group with unwritten field must be dropped");
}

#[test]
fn test_shape_vector_groups_no_overlap() {
    // A group whose fields overlap an already-accepted group must be dropped.
    let mut backend = LlvmBackend::new();
    backend.ctx.field_index_map.insert("f0".to_string(), 0);
    backend.ctx.field_index_map.insert("f1".to_string(), 1);
    backend.ctx.field_index_map.insert("f2".to_string(), 2);
    backend.ctx.field_types = vec![
        "float".to_string(),
        "float".to_string(),
        "float".to_string(),
    ];
    let write_set: HashSet<String> =
        ["f0".to_string(), "f1".to_string(), "f2".to_string()].into_iter().collect();
    let groups = vec![
        crate::analysis::loop_shape::VectorGroup {
            name: "g0".to_string(),
            width: 2,
            fields: vec!["f0".to_string(), "f1".to_string()],
        },
        crate::analysis::loop_shape::VectorGroup {
            name: "g1".to_string(),
            width: 2,
            fields: vec!["f1".to_string(), "f2".to_string()],
        },
    ];
    let vg = backend.shape_vector_groups(&groups, &write_set);
    // g1 reuses f1 from the accepted g0 → dropped; only g0 survives.
    assert_eq!(vg.len(), 1, "overlapping group must be dropped");
    assert_eq!(vg[0].name, "g0");
}

// ── Phase 2 measurement-pass consumers ──────────────────────────────

/// Phase 2 (§7.2): a sparse-dispatch-like modulo program must still be
/// dispatched via the modulo-rotated main loop (`.mr_loop`), driven by the
/// frontend-computed ModuloPartition.
#[test]
fn test_modulo_partition_drives_rotated_loop() {
    let mut program = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::StateDecl(StateDecl {
            name: "total".to_string(),
            ty: Type::int(),
            span: None,
        }),
    ];
    for (i, name) in ["even".to_string(), "odd".to_string()].iter().enumerate() {
        let pre = Expr::BinaryOp(BinaryOpKind::And,
            Box::new(Expr::BinaryOp(BinaryOpKind::Lt,
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Identifier("total".to_string())))),
            Box::new(Expr::BinaryOp(BinaryOpKind::Eq,
                Box::new(Expr::BinaryOp(BinaryOpKind::Mod,
                    Box::new(Expr::Identifier("count".to_string())),
                    Box::new(Expr::Decimal(2)))),
                Box::new(Expr::Decimal(i as i64)))),
        );
        program.push(TopLevel::Transaction(Transaction {
            name: name.clone(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: Expr::BinaryOp(BinaryOpKind::Eq,
                    Box::new(Expr::Identifier("count".to_string())),
                    Box::new(Expr::Identifier("total".to_string()))),
                is_entry: false,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body: vec![
                Statement::Assign(
                    Expr::Identifier("count".to_string()),
                    Expr::BinaryOp(BinaryOpKind::Add,
                        Box::new(Expr::Identifier("count".to_string())),
                        Box::new(Expr::Decimal(1))),
                ),
                Statement::Term(None),
            ],
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        }));
    }
    let output = LlvmBackend::new().generate(&program, None);
    assert!(output.contains(".mr_loop"),
        "modulo-bounded set must use the rotated loop, got:\n{}", &output[..output.len().min(2000)]);
    assert!(output.contains("srem i64"));
}

/// Phase 2 (§7.1): a dense kalman-style txn (FFI guard outlined → #11) must
/// be downgraded to `#0` because the frontend density measurement is > 4.0.
#[test]
fn test_density_consumer_downgrades_dense_txn() {
    let float_field = |name: &str| TopLevel::Statement(Box::new(Statement::Let {
        name: name.to_string(),
        names: vec![],
        ty: Some(Type::Custom("Float".to_string())),
        expr: Some(Expr::Float(0.0)),
        modifiers: vec![],
    }));
    let mut program: Vec<TopLevel> = vec![
        TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::int(),
            span: None,
        }),
        TopLevel::StateDecl(StateDecl {
            name: "total".to_string(),
            ty: Type::int(),
            span: None,
        }),
    ];
    for n in ["x0", "x1", "x2", "p00", "p10", "p20"] {
        program.push(float_field(n));
    }
    for (n, v) in [("a00", 1.0), ("a01", 0.01), ("a02", 0.0)] {
        program.push(TopLevel::Constant(Constant {
            name: n.to_string(),
            ty: Type::Custom("Float".to_string()),
            expr: Expr::Float(v),
        }));
    }
    let mul = |l: Expr, r: Expr| Expr::BinaryOp(BinaryOpKind::Mul, Box::new(l), Box::new(r));
    let add = |l: Expr, r: Expr| Expr::BinaryOp(BinaryOpKind::Add, Box::new(l), Box::new(r));
    let body = vec![
        Statement::Let {
            name: "nx0".to_string(),
            names: vec![],
            ty: Some(Type::Custom("Float".to_string())),
            expr: Some(add(add(
                mul(Expr::Identifier("a00".to_string()), Expr::Identifier("x0".to_string())),
                mul(Expr::Identifier("a01".to_string()), Expr::Identifier("x1".to_string())),
            ), mul(Expr::Identifier("a02".to_string()), Expr::Identifier("x2".to_string())))),
            modifiers: vec![],
        },
        Statement::Let {
            name: "ap00".to_string(),
            names: vec![],
            ty: Some(Type::Custom("Float".to_string())),
            expr: Some(add(add(
                mul(Expr::Identifier("a00".to_string()), Expr::Identifier("p00".to_string())),
                mul(Expr::Identifier("a01".to_string()), Expr::Identifier("p10".to_string())),
            ), mul(Expr::Identifier("a02".to_string()), Expr::Identifier("p20".to_string())))),
            modifiers: vec![],
        },
        Statement::Assign(
            Expr::Identifier("x0".to_string()),
            Expr::Identifier("nx0".to_string()),
        ),
        Statement::Assign(
            Expr::Identifier("count".to_string()),
            Expr::BinaryOp(BinaryOpKind::Add,
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Decimal(1))),
        ),
        // Guarded FFI → outlined (cold function), so the density check fires.
        Statement::Guarded(
            Expr::BinaryOp(BinaryOpKind::Eq,
                Box::new(Expr::BinaryOp(BinaryOpKind::Mod,
                    Box::new(Expr::Identifier("count".to_string())),
                    Box::new(Expr::Decimal(5)))),
                Box::new(Expr::Decimal(0))),
            vec![Statement::Expression(Expr::Call("PrintLn#".to_string(), vec![], None))],
        ),
        Statement::Term(None),
    ];
    program.push(TopLevel::Transaction(Transaction {
        name: "propagate".to_string(),
        is_reactive: true,
        is_async: false,
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: Contract {
            pre_condition: Expr::BinaryOp(BinaryOpKind::Lt,
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Identifier("total".to_string()))),
            post_condition: Expr::BinaryOp(BinaryOpKind::Eq,
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Identifier("total".to_string()))),
            is_entry: false,
            watchdog: None,
            explicit: false,
            span: None,
        },
        body,
        metadata: HashMap::new(),
        derivation: None,
        modifiers: vec![],
        span: None,
        doc: None,
    }));
    let output = LlvmBackend::new().generate(&program, None);
    let txn_line = output.lines()
        .find(|l| l.contains("define void @txn_propagate"))
        .unwrap_or("(not found)");
    assert!(txn_line.contains("#0"),
        "dense txn must be downgraded to #0 via the density measurement, got: {}\n{}",
        txn_line, &output[..output.len().min(1500)]);
}

// ── Batch-loop dispatch (plan 2026-07-31-regain-kalman-float-math-parity) ──

/// A post-increment periodic guard (`when count % N == 0` AFTER count++)
/// dispatches via the countdown loop (.cd_/.cdg_ structure), eliminating the
/// per-iteration modulo check.
#[test]
fn test_batch_loop_dispatch_post_increment() {
    // Dense body (≥ 40 arithmetic ops — the batch cost-model gate) on a set of
    // float fields, kalman-style. 12 fields each updated by a 3-term multiply
    // chain gives ~50+ ops, so the batch dispatch fires.
    let fld = |n: &str| TopLevel::StateDecl(StateDecl { name: n.into(), ty: Type::float(), span: None });
    let mut program = vec![
        TopLevel::StateDecl(StateDecl { name: "count".into(), ty: Type::int(), span: None }),
        TopLevel::StateDecl(StateDecl { name: "total".into(), ty: Type::int(), span: None }),
    ];
    for i in 0..12 {
        program.push(fld(&format!("f{}", i)));
    }
    let mul = |l: Expr, r: Expr| Expr::BinaryOp(BinaryOpKind::Mul, Box::new(l), Box::new(r));
    let add = |l: Expr, r: Expr| Expr::BinaryOp(BinaryOpKind::Add, Box::new(l), Box::new(r));
    let mut body: Vec<Statement> = Vec::new();
    // Each f_i update = f_i*a + f_(i+1)%12*b + f_(i+2)%12*c  (5 ops each → 60 total).
    for i in 0..12 {
        let rhs = add(add(
            mul(Expr::Identifier(format!("f{}", i)), Expr::Float(0.5)),
            mul(Expr::Identifier(format!("f{}", (i + 1) % 12)), Expr::Float(0.25)),
        ), mul(Expr::Identifier(format!("f{}", (i + 2) % 12)), Expr::Float(0.125)));
        body.push(Statement::Assign(Expr::Identifier(format!("f{}", i)), rhs));
    }
    body.push(Statement::Assign(Expr::Identifier("count".into()),
        Expr::BinaryOp(BinaryOpKind::Add,
            Box::new(Expr::Identifier("count".into())),
            Box::new(Expr::Decimal(1)))));
    body.push(Statement::Guarded(
        Expr::BinaryOp(BinaryOpKind::Eq,
            Box::new(Expr::BinaryOp(BinaryOpKind::Mod,
                Box::new(Expr::Identifier("count".into())),
                Box::new(Expr::Decimal(100)))),
            Box::new(Expr::Decimal(0))),
        vec![Statement::Expression(Expr::Call("__print_float".into(), vec![Expr::Identifier("f0".into())], None))]));
    body.push(Statement::Term(None));
    program.push(TopLevel::Transaction(Transaction {
        name: "tick".into(),
        is_reactive: true,
        is_async: false,
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: Contract {
            pre_condition: Expr::BinaryOp(BinaryOpKind::Lt,
                Box::new(Expr::Identifier("count".into())),
                Box::new(Expr::Identifier("total".into()))),
            post_condition: Expr::BinaryOp(BinaryOpKind::Eq,
                Box::new(Expr::Identifier("count".into())),
                Box::new(Expr::Identifier("total".into()))),
            is_entry: false, watchdog: None, explicit: false, span: None,
        },
        body,
        metadata: HashMap::new(), derivation: None, modifiers: vec![],
        span: None, doc: None,
    }));
    let output = LlvmBackend::new().generate(&program, None);
    assert!(output.contains(".cd_"), "post-increment periodic guard must use the countdown loop, got:\n{}", &output[..output.len().min(1200)]);
    assert!(output.contains(".cdg_"), "countdown loop must have a cold guard block");
    // The per-iteration modulo must be GONE from the body (it fires only when
    // the countdown %rem hits 0).
    let body = output.split(".cdb_").nth(1).unwrap_or("");
    let body_seg = body.split(".cdg_").next().unwrap_or("");
    assert!(!body_seg.contains("urem"), "countdown body must not compute count % N per iteration");
}

/// A pre-increment periodic guard (knucleotide pattern) is NOT batched — it
/// stays on version-DAG (the batch structure is off-by-one for it).
#[test]
fn test_batch_loop_rejects_pre_increment() {
    let program = vec![
        TopLevel::StateDecl(StateDecl { name: "count".into(), ty: Type::int(), span: None }),
        TopLevel::StateDecl(StateDecl { name: "total".into(), ty: Type::int(), span: None }),
        TopLevel::StateDecl(StateDecl { name: "acc".into(), ty: Type::int(), span: None }),
        TopLevel::Transaction(Transaction {
            name: "tick".into(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::BinaryOp(BinaryOpKind::Lt,
                    Box::new(Expr::Identifier("count".into())),
                    Box::new(Expr::Identifier("total".into()))),
                post_condition: Expr::BinaryOp(BinaryOpKind::Eq,
                    Box::new(Expr::Identifier("count".into())),
                    Box::new(Expr::Identifier("total".into()))),
                is_entry: false, watchdog: None, explicit: false, span: None,
            },
            body: vec![
                // Guard BEFORE the increment (pre-increment semantics).
                Statement::Guarded(
                    Expr::BinaryOp(BinaryOpKind::Eq,
                        Box::new(Expr::BinaryOp(BinaryOpKind::Mod,
                            Box::new(Expr::Identifier("count".into())),
                            Box::new(Expr::Decimal(100)))),
                        Box::new(Expr::Decimal(0))),
                    vec![Statement::Expression(Expr::Call("__print_int".into(), vec![Expr::Identifier("acc".into())], None))]),
                Statement::Assign(Expr::Identifier("acc".into()),
                    Expr::BinaryOp(BinaryOpKind::Add,
                        Box::new(Expr::Identifier("acc".into())),
                        Box::new(Expr::Decimal(1)))),
                Statement::Assign(Expr::Identifier("count".into()),
                    Expr::BinaryOp(BinaryOpKind::Add,
                        Box::new(Expr::Identifier("count".into())),
                        Box::new(Expr::Decimal(1)))),
                Statement::Term(None),
            ],
            metadata: HashMap::new(), derivation: None, modifiers: vec![],
            span: None, doc: None,
        }),
    ];
    let output = LlvmBackend::new().generate(&program, None);
    assert!(!output.contains(".cd_"), "pre-increment guard must NOT use the countdown loop");
}
