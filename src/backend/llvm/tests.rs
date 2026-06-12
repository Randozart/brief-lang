use super::*;
use crate::ast::*;

fn empty_program() -> Program {
        Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    #[test]
    fn test_llvm_generates_module() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
        assert!(output.contains("target triple"));
    }

    #[test]
    fn test_llvm_generates_state_type() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "counter".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("%State"));
        assert!(output.contains("i64"));
        assert!(output.contains("%state"));
    }

    #[test]
    fn test_llvm_generates_transaction() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("count".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("count".to_string())),
                                Box::new(Expr::Integer(1)),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("@increment("));
    }

    #[test]
    fn test_llvm_has_noalias() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
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
        let output = backend.generate(&empty_program());
        assert!(!output.is_empty());
    }

    #[test]
    fn test_llvm_event_model_lowering() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Trigger(TriggerDeclaration {
                    name: "io_pending".to_string(),
                    ty: Type::Bool,
                    address: LinkRef::Linked("__io_pending".to_string()),
                    bit_range: None,
                    stages: vec![],
                    condition: None,
                    is_wake: false,
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "event_count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "pump".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
                TopLevel::Transaction(Transaction {
                    name: "sleep".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);

        // @ link trigger emits external global
        assert!(output.contains("external global"), "Should declare external globals for @ link");
        assert!(output.contains("__io_pending"), "Should contain trigger global name");

        // Fall-through dispatch: body blocks don't end with ret void
        assert!(output.contains("reactor_tick"), "Should have reactor_tick");
        assert!(output.contains("%state"), "Should reference state pointer");
        assert!(output.contains("__io_pending"), "Should reference trigger");

        // Trigger sampling emits load volatile
        assert!(output.contains("load volatile"), "Should have volatile trigger loads");

        // Must not have __wait_for_event as hardcoded intrinsic
        assert!(!output.contains("declare void @__wait_for_event()"),
            "Should NOT have hardcoded __wait_for_event declaration");
    }

    // ── Phase 4: Backend correctness tests ──────────────────────────

    #[test]
    fn test_escape_non_ascii_string() {
        let output = escape_llvm_string("héllo");
        // 'é' is U+00E9 → bytes C3 A9
        assert!(output.contains("\\c3"), "Should hex-escape byte C3");
        assert!(output.contains("\\a9"), "Should hex-escape byte A9");
        // ASCII 'h' 'e' 'l' 'l' 'o' should be preserved as-is
        assert!(output.contains("h"), "ASCII 'h' should be preserved");
        assert!(output.contains("llo"), "ASCII 'llo' should be preserved after escape bytes");
    }

    #[test]
    fn test_unification_payload_discriminant() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Option".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("None".to_string()),
                        EnumVariant::Tuple("Some".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "s".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Unification {
                            name: "Some".to_string(),
                            variant: "v".to_string(),
                            fields: vec![],
                            expr: Expr::Integer(1),
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false,
                    is_reactive: false,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        // Payload variant Some → discriminant 1
        assert!(output.contains("i64 1, label"),
            "Unification of 'Some' should target discriminant 1");
    }

    #[test]
    fn test_no_range_lower_bound_defaults_to_i64_min() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Lt(
                            Box::new(Expr::Identifier("x".to_string())),
                            Box::new(Expr::Integer(100)),
                        ),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false,
                    is_reactive: false,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        // Lower bound should be i64::MIN = -9223372036854775808
        assert!(output.contains("-9223372036854775808"),
            "Range with no lower bound should use i64::MIN");
    }

    #[test]
    fn test_binop_no_nuw_nsw() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "y".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::And(
                            Box::new(Expr::And(
                                Box::new(                Expr::Ge(
                                    Box::new(Expr::Identifier("x".to_string())),
                                    Box::new(Expr::Integer(0)),
                                )),
                                Box::new(Expr::Lt(
                                    Box::new(Expr::Identifier("x".to_string())),
                                    Box::new(Expr::Integer(10)),
                                )),
                            )),
                            Box::new(Expr::Lt(
                                Box::new(Expr::Identifier("y".to_string())),
                                Box::new(Expr::Integer(10)),
                            )),
                        ),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("x".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("x".to_string())),
                                Box::new(Expr::Identifier("y".to_string())),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false,
                    is_reactive: false,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        // Must NOT emit nuw nsw — we removed manual emission
        assert!(!output.contains("nuw nsw"),
            "add on bounded variables should NOT emit nuw nsw (LLVM infers from !range)");
    }

    // ── Phase 5: Wake trigger and blocking wait tests ────────────────

    fn make_wake_trg_program(trg_name: &str, sym: &str, ty: Type, is_wake: bool) -> Program {
        Program {
            items: vec![
                TopLevel::Trigger(TriggerDeclaration {
                    name: trg_name.to_string(),
                    ty,
                    address: LinkRef::Linked(sym.to_string()),
                    bit_range: None,
                    stages: vec![],
                    condition: None,
                    is_wake,
                    span: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Identifier(trg_name.to_string()),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    #[test]
    fn test_no_wake_triggers_no_metadata() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("@llvm.wake_triggers"),
            "No wake triggers → no @llvm.wake_triggers metadata");
        assert!(!output.contains("call void @__rt_wait()"),
            "No wake triggers → no __rt_wait call");
    }

    #[test]
    fn test_single_wake_trigger_metadata() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("@llvm.wake_triggers = constant [1 x i8*] [i8* @__sigint_flag]"),
            "Single wake trigger → constant global with one symbol");
        assert!(output.contains("!llvm.wake_triggers = !{!0}"),
            "Named metadata node present");
        assert!(output.contains("!0 = !{!\"__sigint_flag\"}"),
            "Metadata references __sigint_flag");
    }

    #[test]
    fn test_multiple_wake_triggers_metadata() {
        let mut p1 = make_wake_trg_program("sigint", "__sigint_flag", Type::Bool, true);
        p1.items.insert(1, TopLevel::Trigger(TriggerDeclaration {
            name: "stdin".to_string(),
            ty: Type::Bool,
            address: LinkRef::Linked("__stdin_ready".to_string()),
            bit_range: None,
            stages: vec![],
            condition: None,
            is_wake: true,
            span: None,
        }));
        let output = LlvmBackend::new().generate(&p1);
        assert!(output.contains("[2 x i8*]"),
            "Multiple wake triggers → array size 2");
        assert!(output.contains("__sigint_flag"),
            "First symbol present");
        assert!(output.contains("__stdin_ready"),
            "Second symbol present");
    }

    #[test]
    fn test_main_calls_rt_wait_with_wake_triggers() {
        // Use Int trigger (non-enumerable) to force standard reactor path
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @__rt_wait()"),
            "main() calls __rt_wait() after reactor_tick");
    }

    #[test]
    fn test_enum_with_wake_triggers_hybrid() {
        // Bool trigger with is_wake → enters enum dispatch in hybrid wake mode.
        // Previously this bypassed enum entirely (Phase A gate). Now enum dispatch
        // is active, with __rt_wait() wrapping the switch arms.
        // With uniform-body detection: identical case arms skip the switch dispatch.
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @__rt_wait()"),
            "Wake triggers get __rt_wait between ticks");
        assert!(!output.contains("switch i64"),
            "Uniform enum bodies skip the switch dispatch");
        assert!(output.contains("load volatile"),
            "Triggers are volatile-loaded for sampling");
        assert!(output.contains("define i32 @main() local_unnamed_addr #3"),
            "Wake hybrid uses #3 attribute (no willreturn, no mustprogress) for infinite tick loop");
    }

    #[test]
    fn test_main_no_wait_without_wake_triggers() {
        // Use Int trigger (non-enumerable) to force standard reactor path
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("call void @__rt_wait()"),
            "main() does not call __rt_wait() without wake triggers");
    }

    #[test]
    fn test_rt_declares_present() {
        // Use Int trigger (non-enumerable) to force standard reactor path
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("declare void @__rt_wait()"),
            "__rt_wait not declared without wake triggers");
    }

    #[test]
    fn test_rt_declares_present_with_wake() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("declare void @__rt_wait()"),
            "__rt_wait declared with wake triggers");
    }

    #[test]
    fn test_wake_non_link_trigger_no_metadata() {
        // MMIO triggers with #wake should not appear in metadata (parse-time error, but belt-and-suspenders)
        let program = Program {
            items: vec![
                TopLevel::Trigger(TriggerDeclaration {
                    name: "mmio".to_string(),
                    ty: Type::Bool,
                    address: LinkRef::Explicit(0x4000),
                    bit_range: None,
                    stages: vec![],
                    condition: None,
                    is_wake: true,
                    span: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), span: None, watchdog: None },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
                    is_async: false, is_reactive: true, reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![], attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = LlvmBackend::new().generate(&program);
        // MMIO triggers with is_wake → metadata only includes LinkRef::Linked symbols, not Explicit
        assert!(!output.contains("@llvm.wake_triggers"),
            "MMIO wake trigger should not produce metadata (not a linked symbol)");
    }

    // ── Plan C: Local float binding tests ─────────────────────────

    #[test]
    fn test_local_float_binding() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Float,
                    expr: Some(Expr::Float(1.5)),
                    address: None, bit_range: None,
                    is_override: false, os_mode: false,
                    span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None, watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("x".to_string()),
                            expr: Expr::Float(2.0),
                            timeout: None, modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false, is_reactive: false,
                    reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![],
                    attrs: vec![], modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        // Float literal emits bitcast i32 <hex> to float directly (no i64 boxing)
        assert!(output.contains("bitcast i32"),
            "Float literal should emit bitcast i32 to float: {}", output);
        // The float value should NOT be boxed through i32->i64
        assert!(!output.contains("zext i32"),
            "Float should not be boxed to i64: {}", output);
    }

    #[test]
    fn test_float_binary_add() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Float,
                    expr: Some(Expr::Float(1.0)),
                    address: None, bit_range: None,
                    is_override: false, os_mode: false,
                    span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None, watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("x".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("x".to_string())),
                                Box::new(Expr::Float(2.0)),
                            ),
                            timeout: None, modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false, is_reactive: false,
                    reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![],
                    attrs: vec![], modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("fadd fast float"),
            "Float binary add should emit fadd fast float");
    }

    #[test]
    fn test_main_and_reactor_use_non_willreturn_attr() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        // With A006, wake-trigger programs go through direct SSA loop (emit_ssa_main)
        // or enumerable dispatch (emit_folded_multi_main) — both use #3 for wake.
        // The pre_t and t functions still use #0.
        let has_correct_main = output.contains("define i32 @main() local_unnamed_addr #3")
            || output.contains("define i32 @main() local_unnamed_addr #5");
        assert!(has_correct_main,
            "main() should use #3 (or #5 if SLP-disabled), got: {:?}",
            output.lines().find(|l| l.contains("define i32 @main")).unwrap_or("(not found)"));
        // No reactor_tick with A006 path — triggers sampled inline
        assert!(!output.contains("define void @reactor_tick("),
            "reactor_tick should not be emitted (A006 direct SSA loop)");
        assert!(output.contains("attributes #0"),
            "attributes #0 should still be present for terminating functions");
        assert!(output.contains("define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0"),
            "init_state() should still use #0 with willreturn");
    }

    // ── Integration: optimization report & chain composition ──

    fn make_chain_program(
        txns: Vec<(&str, Vec<Statement>)>,
        trigger: Option<(&str, Type)>,
        consts: &[(&str, i64)],
        states: &[(&str, i64)],
    ) -> Program {
        let mut items: Vec<TopLevel> = Vec::new();
        for (name, val) in consts {
            items.push(TopLevel::Constant(Constant {
                name: name.to_string(),
                ty: Type::Int,
                expr: Expr::Integer(*val),
            }));
        }
        for (name, val) in states {
            items.push(TopLevel::StateDecl(StateDecl {
                name: name.to_string(),
                ty: Type::Int,
                expr: Some(Expr::Integer(*val)),
                address: None, bit_range: None, is_override: false,
                os_mode: false, span: None, attrs: vec![],
                range_constraint: None,
            }));
        }
        if let Some((trg_name, trg_ty)) = trigger {
            items.push(TopLevel::Trigger(TriggerDeclaration {
                name: trg_name.to_string(), ty: trg_ty,
                address: LinkRef::Explicit(0), bit_range: None,
                stages: vec![], condition: None, is_wake: false, span: None,
            }));
        }
        for (txn_name, body) in txns {
            let pre = Expr::Lt(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Identifier("total".to_string())),
            );
            items.push(TopLevel::Transaction(Transaction {
                name: txn_name.to_string(), parameters: vec![],
                contract: Contract {
                    pre_condition: pre,
                    post_condition: Expr::Bool(true),
                    span: None, watchdog: None,
                },
                body, is_async: false, is_reactive: true, reactor_speed: None,
                span: None, is_lambda: false, dependencies: vec![],
                attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                         outputs: Vec::new(),
             output_type: None,
         }));
        }
        Program {
            items, comments: vec![], reactor_speed: None, attrs: Vec::new(),
            ffi: None, strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    fn ident_s(s: &str) -> Expr { Expr::Identifier(s.to_string()) }
    fn int_s(v: i64) -> Expr { Expr::Integer(v) }

    #[test]
    fn test_report_shows_ranking() {
        let program = make_chain_program(
            vec![("t1", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            Some(("sensor", Type::Bool)),
            &[("total", 100)], &[("count", 0), ("x", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(256).with_optimize_report(true);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Optimization priority ranking"),
            "Report should contain priority ranking section");
    }

    #[test]
    fn test_report_shows_budget() {
        let program = make_chain_program(
            vec![("t1", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            Some(("sensor", Type::Bool)),
            &[("total", 100)], &[("count", 0), ("x", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(10).with_optimize_report(true);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Budget plan"),
            "Report should contain budget plan section");
    }

    #[test]
    fn test_report_shows_size() {
        let program = make_chain_program(
            vec![("t1", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            Some(("sensor", Type::Bool)),
            &[("total", 100)], &[("count", 0), ("x", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(256).with_optimize_report(true)
            .with_optimize_size(10000);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Size estimation") || joined.contains("Base binary"),
            "Report should contain size estimation section");
    }

    #[test]
    fn test_report_shows_chains() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            Some(("sensor", Type::Bool)),
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(256).with_optimize_report(true);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Linear transaction chains")
            || joined.contains("Composed chains"),
            "Report should detect multi-txn chains");
    }

    #[test]
    fn test_enum_with_composed_chain() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            Some(("sensor", Type::Bool)),
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        // All-internal chains skip fused fn emission; pure counter store
        // is emitted directly in the per-case switch arm.
        assert!(output.contains("switch i64"),
            "Should emit switch dispatch for enumerable trigger");
        assert!(output.contains("@main"),
            "Should emit main function with enum dispatch");
    }

    #[test]
    fn test_all_internal_pure_counter_emitted() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("_trig"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("internal"), expr: int_s(42), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("result"), expr: Expr::Add(Box::new(ident_s("internal")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            Some(("sensor", Type::Bool)),
            &[("total", 100)],
            &[("count", 0), ("internal", 0), ("result", 0), ("_trig", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(output.contains("@main"),
            "Should emit main function");
    }

    #[test]
    fn test_precompute_pure_counter() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: int_s(42), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            None,
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(output.contains("call void @init_state(%State* noalias nocapture %state)"),
            "Should call init_state");
        assert!(!output.contains("switch i64"),
            "No enum dispatch for precomputed path");
        assert!(!output.contains("@reactor_tick"),
            "No reactor_tick for precomputed path");
        assert!(output.contains("ret i32 0"),
            "Should return normally");
    }

    #[test]
    fn test_precompute_budget_exceeded_fallback() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: int_s(42), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            None,
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(0).generate(&program);
        assert!(!output.contains("switch i64"),
            "No enum dispatch without triggers");
        assert!(output.contains("getelementptr inbounds %State, %State* %state, i32 0, i32"),
            "All-convergent program should use per-field GEP loads");
        assert!(!output.contains("@reactor_tick"),
            "All-convergent program should not emit reactor_tick");
    }

    #[test]
    fn test_iir_filter_folded_path_regression() {
        let program = make_chain_program(
            vec![("process", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: int_s(42), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            None,
            &[("total", 50000000)],
            &[("count", 0), ("x", 0)],
        );
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("switch i64"),
            "Single-txn convergence should use folded path, not enum dispatch");
        assert!(!output.contains("@reactor_tick"),
            "Single-txn convergence should use folded path, not standard reactor");
        // With dead-field elimination, the float state x is never observed
        // (no exit condition references it, no other txn reads it).
        // The txn becomes effectively pure — only count = count + 1 survives.
        assert!(output.contains("store i64 50000000"),
            "Effectively-pure body should emit O(1) store i64 total, not a while-loop");
        assert!(output.contains("ret i32 0"),
            "Should return after store");
        // The while-loop body (process) is still emitted but main is O(1).
        // Verify main is the pure counter form by checking main is between
        // the store and the return.
        let main_idx = output.find("define i32 @main()").unwrap_or(0);
        let store_in_main = output[main_idx..].contains("store i64 50000000");
        assert!(store_in_main, "store must be in main, not in process");
    }

    fn make_async_pair_program() -> Program {
        Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "a".to_string(),
                    ty: Type::Int,
                    expr: Some(int_s(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "b".to_string(),
                    ty: Type::Int,
                    expr: Some(int_s(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "inc_a".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("a".to_string()),
                            expr: Expr::Add(Box::new(ident_s("a")), Box::new(int_s(1))),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: true,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
                TopLevel::Transaction(Transaction {
                    name: "inc_b".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("b".to_string()),
                            expr: Expr::Add(Box::new(ident_s("b")), Box::new(int_s(1))),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: true,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    #[test]
    fn test_async_body_functions_emitted() {
        let program = make_async_pair_program();
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("@async_body_inc_a"),
            "Async body function for inc_a should be emitted");
        assert!(output.contains("@async_body_inc_b"),
            "Async body function for inc_b should be emitted");
    }

    #[test]
    fn test_thread_pool_metadata_emitted() {
        let program = make_async_pair_program();
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("@llvm.thread_pool"),
            "Thread pool metadata should be emitted for async txns");
        assert!(output.contains("@thread_pool_fns"),
            "Thread pool function pointer array should be emitted");
    }

    #[test]
    fn test_async_barrier_calls_in_main() {
        let program = make_async_pair_program();
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @brief_thread_pool_init"),
            "Main should call thread_pool_init");
        assert!(output.contains("call void @brief_barrier_release"),
            "Main should call barrier_release");
        assert!(output.contains("call void @brief_barrier_wait"),
            "Main should call barrier_wait");
    }

    #[test]
    fn test_no_thread_pool_without_async_txns() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("@llvm.thread_pool"),
            "No thread pool metadata without async txns");
        assert!(!output.contains("call void @brief_barrier"),
            "No barrier calls without async txns");
        assert!(!output.contains("call void @brief_thread_pool_init"),
            "No thread pool init without async txns");
    }

    // ── Exit condition tests ──────────────────────────────────

    fn make_exit_program(exit_expr: Option<Expr>, trg_ty: Type, is_wake: bool) -> Program {
        let trg_name = "io_pending";
        let mut items = vec![
            TopLevel::StateDecl(StateDecl {
                name: "ops".to_string(),
                ty: Type::Int,
                expr: Some(int_s(0)),
                address: None, bit_range: None, is_override: false,
                os_mode: false, span: None, attrs: vec![],
                range_constraint: None,
            }),
        ];
        items.push(TopLevel::Constant(Constant {
            name: "N".to_string(),
            ty: Type::Int,
            expr: int_s(100),
        }));
        items.push(TopLevel::Trigger(TriggerDeclaration {
            name: trg_name.to_string(),
            ty: trg_ty,
            address: LinkRef::Linked("__io_pending".to_string()),
            bit_range: None, stages: vec![], condition: None,
            is_wake, span: None,
        }));
        let pre = Expr::And(
            Box::new(Expr::Identifier(trg_name.to_string())),
            Box::new(Expr::Lt(
                Box::new(Expr::Identifier("ops".to_string())),
                Box::new(Expr::Identifier("N".to_string())),
            )),
        );
        items.push(TopLevel::Transaction(Transaction {
            name: "work".to_string(),
            parameters: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: Expr::Bool(true),
                span: None, watchdog: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("ops".to_string()),
                    expr: Expr::Add(Box::new(ident_s("ops")), Box::new(int_s(1))),
                    timeout: None, modifiers: vec![],
                },
                Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
            ],
            is_async: false, is_reactive: true, reactor_speed: None,
            span: None, is_lambda: false, dependencies: vec![],
            attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     }));
        Program {
            items,
            comments: vec![], reactor_speed: None, attrs: vec![],
            ffi: None, strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: exit_expr.map(Box::new),
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    #[test]
    fn test_exit_pragma_in_wake_main() {
        // #!exit ops == N; with Int trigger (standard reactor path)
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        // Exit check should appear before __rt_wait
        assert!(output.contains("trunc i64"),
            "Exit condition should trunc i64 to i1");
        assert!(output.contains("br i1"),
            "Exit condition should branch on icmp result");
        assert!(output.contains("done:"),
            "Exit condition should emit done label");
        assert!(output.contains("wait:"),
            "Wake main should emit wait label after exit check");
        assert!(output.contains("ret i32 0"),
            "done label should return 0");
    }

    #[test]
    fn test_exit_pragma_without_wake_no_change() {
        // #!exit ops == N; with Int trigger but is_wake=false → no __rt_wait
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, false);
        let output = LlvmBackend::new().generate(&program);
        // Exit check still emitted, but no wait label
        assert!(output.contains("trunc i64"),
            "Exit condition should trunc i64 to i1 even without wake");
        assert!(output.contains("br i1"),
            "Exit condition should branch");
        assert!(output.contains("done:"),
            "Exit condition should emit done label");
        assert!(!output.contains("wait:"),
            "No wait label without wake triggers");
        assert!(output.contains("ret i32 0"),
            "done label should return 0");
    }

    #[test]
    fn test_no_exit_without_pragma() {
        // Non-foldable wake program without #!exit: no exit check, no natural death
        let program = make_wake_trg_program("io", "__io_pending", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        // Exit check pattern: `trunc` then `br i1 ..., label %done, ...`
        assert!(!output.contains("label %done"),
            "No branch-to-done without exit condition or natural death");
    }

    #[test]
    fn test_exit_in_enum_main() {
        // Bool trigger → enum dispatch path, no wake → one-shot.
        // Uniform-body detection skips the switch when all case arms are identical.
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, false);
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        // One-shot enum dispatch: no tick loop, no exit check needed
        assert!(!output.contains("switch i64"),
            "Uniform enum bodies skip the switch dispatch");
        assert!(output.contains("ret i32 0"),
            "One-shot path returns 0 at each case arm");
        assert!(!output.contains("exit_check:"),
            "No exit check label in one-shot path (no tick loop)");
    }

    #[test]
    fn test_exit_in_enum_hybrid_wake() {
        // Bool trigger with is_wake → hybrid path (enum + wake).
        // Uniform-body detection skips the switch when all case arms are identical.
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, true);
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(!output.contains("switch i64"),
            "Uniform enum bodies skip the switch dispatch");
        assert!(output.contains("exit_check:"),
            "Hybrid mode should emit exit_check label");
        assert!(output.contains("do_wait:"),
            "Hybrid mode should still have do_wait for wake path");
        assert!(output.contains("call void @__rt_wait()"),
            "Hybrid mode should have __rt_wait");
        assert!(output.contains("ret i32 0"),
            "Should return 0 on exit");
    }

    // ── Exit diagnostic tests ──────────────────────────────────

    #[test]
    fn test_check_exit_condition_idents_valid() {
        // Known identifiers (state field + constant) should produce no errors
        let mut backend = LlvmBackend::new();
        backend.field_index_map.insert("ops".to_string(), 0);
        backend.constants.insert("N".to_string(), (Type::Int, Expr::Integer(100)));

        let expr = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let errors = backend.check_exit_condition_idents(&expr);
        assert!(errors.is_empty(),
            "No errors for known identifiers: {:?}", errors);
    }

    #[test]
    fn test_check_exit_condition_idents_invalid() {
        // Unknown identifier should produce an error
        let mut backend = LlvmBackend::new();
        backend.field_index_map.insert("ops".to_string(), 0);
        backend.constants.insert("N".to_string(), (Type::Int, Expr::Integer(100)));

        let expr = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("bogus_var".to_string())),
        );
        let errors = backend.check_exit_condition_idents(&expr);
        assert!(!errors.is_empty(),
            "Should report error for unknown identifier");
        assert!(errors[0].contains("bogus_var"),
            "Error should reference the unknown name: {}", errors[0]);
    }

    #[test]
    fn test_one_shot_exit_warning_enum() {
        // Bool trigger without wake → enum dispatch → one-shot → warning
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, false);
        let mut backend = LlvmBackend::new().with_optimize_budget(256);
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("#!exit declared but program has no tick loop")
        });
        assert!(has_warning,
            "Expected one-shot warning for enum dispatch with #!exit");
    }

    #[test]
    fn test_no_one_shot_warning_in_wake_main() {
        // Int trigger with wake → standard reactor → checks exit → no warning
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("#!exit declared but program has no tick loop")
        });
        assert!(!has_warning,
            "No one-shot warning for standard reactor wake main with #!exit");
    }

    #[test]
    fn test_no_exit_path_warning_for_wake_program() {
        // Wake program without #!exit and without foldable txns should warn.
        // Non-foldable reactive txns cannot converge, so natural death won't help.
        let program = make_wake_trg_program("io", "__io_pending", Type::Bool, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(has_warning,
            "Expected no-exit-path warning for wake program without #!exit");
    }

    #[test]
    fn test_no_no_exit_path_warning_when_exit_present() {
        // Wake program WITH #!exit should NOT warn about missing exit path
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(!has_warning,
            "No no-exit-path warning when #!exit is present");
    }

    #[test]
    fn test_no_exit_path_warning_for_non_wake_program() {
        // Non-wake program without #!exit should NOT warn (one-shot is fine)
        let program = make_exit_program(None, Type::Int, false);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(!has_warning,
            "No no-exit-path warning for non-wake program");
    }

    // ── Natural death tests ───────────────────────────────────

    #[test]
    fn test_natural_death_exits_foldable_program() {
        // Wake program with foldable txn but no #!exit → natural death emits exit check
        let program = make_exit_program(None, Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        // Natural death should have set has_natural_exit
        assert!(backend.has_natural_exit,
            "Foldable wake program should have natural exit");
        // Exit check should be emitted (trunc + branch to done)
        assert!(_output.contains("label %done"),
            "Natural death should emit exit check (branch to done)");
        // No warning about missing exit path — natural death handles it
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(!has_warning,
            "No no-exit-path warning when natural death handles it");
    }

    #[test]
    fn test_natural_death_skipped_for_persistent_txn() {
        // Wake program with non-foldable txn → natural death should NOT apply
        let program = make_wake_trg_program("io", "__io_pending", Type::Bool, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        assert!(!backend.has_natural_exit,
            "Program with persistent txn should NOT have natural exit");
        // Warning about missing exit path should fire
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(has_warning,
            "Persistent wake program without #!exit should warn");
        // No exit check emitted
        assert!(!_output.contains("label %done"),
            "No exit check for persistent program");
    }

    #[test]
    fn test_natural_death_skipped_for_non_wake() {
        // Non-wake program with foldable txn → natural death not needed (one-shot)
        let program = make_exit_program(None, Type::Int, false);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        assert!(!backend.has_natural_exit,
            "Non-wake program should NOT use natural death");
    }

    // ── SLP Hazard Detection Tests ────────────────────────────

    fn make_slp_float_program(n_floats: usize, cross_body: Vec<Statement>, precondition: Option<Expr>) -> Program {
        let mut items: Vec<TopLevel> = Vec::new();
        // Add n float fields: f0..f{n-1} = 0.0
        for i in 0..n_floats {
            items.push(TopLevel::StateDecl(StateDecl {
                name: format!("f{}", i),
                ty: Type::Float,
                expr: Some(Expr::Float(0.0)),
                address: None,
                bit_range: None,
                is_override: false,
                os_mode: false,
                span: None,
                attrs: vec![],
                range_constraint: None,
            }));
        }
        // Add counter field so bounded_pre can work
        items.push(TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::Int,
            expr: Some(Expr::Integer(0)),
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
            range_constraint: None,
        }));
        items.push(TopLevel::StateDecl(StateDecl {
            name: "total".to_string(),
            ty: Type::Int,
            expr: Some(Expr::Integer(100)),
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
            range_constraint: None,
        }));
        items.push(TopLevel::Transaction(Transaction {
            name: "tick".to_string(),
            is_async: false,
            is_reactive: true,
            parameters: vec![],
            contract: Contract {
                pre_condition: precondition.unwrap_or(Expr::Bool(true)),
                post_condition: Expr::Identifier("count".to_string()),
                watchdog: None,
                span: None,
            },
            body: cross_body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     }));
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    fn make_cross_float_body(n_floats: usize, cross_count: usize) -> Vec<Statement> {
        let mut stmts: Vec<Statement> = Vec::new();
        // Assignment: f0 = f1 * f2; f1 = f2 * f3; etc.
        for i in 0..cross_count {
            let a = (i * 3) % n_floats;
            let b = ((i * 3) + 1) % n_floats;
            let c = ((i * 3) + 2) % n_floats;
            stmts.push(Statement::Assignment {
                lhs: Expr::Identifier(format!("f{}", a)),
                expr: Expr::Mul(
                    Box::new(Expr::Identifier(format!("f{}", b))),
                    Box::new(Expr::Identifier(format!("f{}", c))),
                ),
                timeout: None,
                modifiers: vec![],
            });
        }
        // Increment counter so bounded_pre can fire
        stmts.push(Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        });
        stmts
    }

    #[test]
    fn test_slp_hazard_no_floats() {
        // No float fields → no SLP hazard
        let program = make_slp_float_program(0, make_cross_float_body(0, 0), None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        assert!(!output.contains("disable-slp-vectorize"),
            "No float fields should produce no SLP-disabled attributes");
    }

    #[test]
    fn test_slp_hazard_small_field_count() {
        // 4 float fields, 6 float ops → 6/4=1.5 ops/field ≥ threshold, SLP is safe
        let body = make_cross_float_body(4, 6);
        let program = make_slp_float_program(4, body, None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        assert!(!output.contains("disable-slp-vectorize"),
            "4 float fields with 6 ops should not trigger SLP disable");
    }

    #[test]
    fn test_slp_hazard_large_field_count() {
        // 20 float fields + many cross-ops → SLP hazard on SSE (peak ≥ 16)
        // Formula: ceil(20/4)=5 packed, min(10,20)=10 shuffles, 0 temps, 0 consts, +2 = 17
        let body = make_cross_float_body(20, 40);
        let program = make_slp_float_program(20, body, None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        assert!(output.contains("disable-slp-vectorize"),
            "20 float fields with cross-ops should disable SLP on SSE");
    }

    #[test]
    fn test_slp_hazard_independent_channels() {
        // 12 float fields with ZERO cross-ops → no shuffles needed, SLP is safe
        let mut body: Vec<Statement> = Vec::new();
        for i in 0..12 {
            body.push(Statement::Assignment {
                lhs: Expr::Identifier(format!("f{}", i)),
                expr: Expr::Add(
                    Box::new(Expr::Identifier(format!("f{}", i))),
                    Box::new(Expr::Float(1.0)),
                ),
                timeout: None,
                modifiers: vec![],
            });
        }
        body.push(Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        });
        let program = make_slp_float_program(12, body, None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        // Independent channels: packed_phis=3, shuffle_regs=0, temps=0, margin=2 → peak=5 < 16
        assert!(!output.contains("disable-slp-vectorize"),
            "12 independent float fields should NOT disable SLP");
    }

    #[test]
    fn test_slp_hazard_with_target_spec() {
        // AArch64 (R=32, W=4), 12 fields, 18 float ops → 18/12=1.5 ops/field, SLP safe
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
        let output = backend.generate(&program);
        assert!(!output.contains("disable-slp-vectorize"),
            "AArch64 with 32 registers and ASR 2.4 > 1.5 should allow SLP for 12 fields");
    }

    #[test]
    fn test_slp_hazard_avx_target() {
        // With AVX2 (R=16, W=8) → 12 fields, 32 cross-ops
        // shuffle_pressure=min(32,24)=24, peak=2+24+0+0+2=28 >= 16 → SLP disabled
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
        let output = backend.generate(&program);
        // 32 cross-ops on 12 fields → peak 28 ≥ 16 → spills on AVX2 → disable
        assert!(output.contains("disable-slp-vectorize"),
            "AVX2: 12 fields with 32 cross-ops should disable SLP (peak=28 ≥ 16)");
    }

    #[test]
    fn test_dbvs_import_aliases_loaded() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("uart_debug".to_string(), crate::dbrief::DbriefType::Data);
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        assert_eq!(backend.schema_aliases.len(), 1);
        assert!(backend.schema_aliases.contains_key("uart_debug"));
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
    }

    #[test]
    fn test_schema_type_unsigned_warning() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("count".to_string(), crate::dbrief::DbriefType::UInt(64));
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let _output = backend.generate(&program);
        let warnings = backend.warnings();
        let has_unsigned_warning = warnings.iter().any(|w| w.contains("unsigned") && w.contains("count"));
        assert!(has_unsigned_warning,
            "UInt(64) schema type with Int Brief type should produce unsigned warning, got: {:?}", warnings);
    }

    #[test]
    fn test_schema_vector_rejected() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("buf".to_string(), crate::dbrief::DbriefType::Vector(
            Box::new(crate::dbrief::DbriefType::UInt(8)), Some(256)));
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "buf".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let _output = backend.generate(&program);
        let warnings = backend.warnings();
        let has_vector_warning = warnings.iter().any(|w| w.contains("Vector") && w.contains("buf"));
        assert!(has_vector_warning,
            "Vector schema type should produce incompatibility warning, got: {:?}", warnings);
    }

    #[test]
    fn test_no_schema_import_no_validation() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let _output = backend.generate(&program);
        assert!(backend.warnings().is_empty(),
            "No schema import should produce no warnings");
    }

    #[test]
    fn test_multiple_schema_imports_merged() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("gpio0".to_string(), crate::dbrief::DbriefType::UInt(32));
        aliases.insert("gpio1".to_string(), crate::dbrief::DbriefType::UInt(32));
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        assert_eq!(backend.schema_aliases.len(), 2);
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
    }

    #[test]
    fn test_imported_alias_is_mmio() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("led_0".to_string(), crate::dbrief::DbriefType::UInt(32));
        let mut mmio: HashMap<String, u64> = HashMap::new();
        mmio.insert("led_0".to_string(), 0x40000000);
        let mut backend = LlvmBackend::new()
            .with_schema_aliases(aliases)
            .with_mmio_addresses(mmio);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "led_0".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("inttoptr i64 1073741824"),
            "led_0 with schema import should be MMIO (inttoptr). Got: {}", output);
        assert!(output.contains("store volatile i64"),
            "led_0 with schema import should use volatile store. Got: {}", output);
    }

    #[test]
    fn test_unimported_alias_not_mmio() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("uart_debug".to_string(), crate::dbrief::DbriefType::Data);
        let mut mmio: HashMap<String, u64> = HashMap::new();
        mmio.insert("led_0".to_string(), 0x40000000);
        mmio.insert("uart_debug".to_string(), 0xFF010000);
        let mut backend = LlvmBackend::new()
            .with_schema_aliases(aliases)
            .with_mmio_addresses(mmio);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "led_0".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(!output.contains("inttoptr i64 1073741824"),
            "led_0 NOT in schema should NOT be MMIO (no inttoptr for 0x40000000). Got: {}", output);
        assert!(output.contains("getelementptr inbounds %State"),
            "led_0 NOT in schema should use struct GEP. Got: {}", output);
    }

    // ── Struct codegen tests ───────────────────────────────────

    #[test]
    fn test_struct_type_registered() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Struct(StructDefinition {
                    name: "Point".to_string(),
                    type_params: vec![],
                    fields: vec![
                        StructField { name: "x".to_string(), ty: Type::Int, default: None, visibility: Visibility::Public },
                        StructField { name: "y".to_string(), ty: Type::Int, default: None, visibility: Visibility::Public },
                    ],
                    transactions: vec![],
                    view_html: None,
                    span: None,
                    modifiers: vec![],
                    variants: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("ModuleID"), "Output should be valid IR");
        assert!(backend.struct_types.contains_key("Point"),
            "Struct 'Point' should be registered");
        assert_eq!(backend.struct_types["Point"].len(), 2);
    }

    fn make_point_program(body: Vec<Statement>) -> Program {
        Program {
            items: vec![
                TopLevel::Struct(StructDefinition {
                    name: "Point".to_string(),
                    type_params: vec![],
                    fields: vec![
                        StructField { name: "x".to_string(), ty: Type::Int, default: None, visibility: Visibility::Public },
                        StructField { name: "y".to_string(), ty: Type::Int, default: None, visibility: Visibility::Public },
                    ],
                    transactions: vec![],
                    view_html: None,
                    span: None,
                    modifiers: vec![],
                    variants: vec![],
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "pt".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "main".to_string(),
                    is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body,
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        }
    }

    #[test]
    fn test_struct_instance_emits_alloca_store_ptrtoint() {
        let mut backend = LlvmBackend::new();
        let body = vec![
            Statement::Let {
                name: "p".to_string(),
                ty: Some(Type::Custom("Point".to_string())),
                expr: Some(Expr::StructInstance("Point".to_string(), vec![
                    ("x".to_string(), Expr::Integer(10)),
                    ("y".to_string(), Expr::Integer(20)),
                ])),
                address: None, address_expr: None, bit_range: None,
                is_override: false, modifiers: vec![],
                range_constraint: None,
            },
        ];
        let output = backend.generate(&make_point_program(body));
        assert!(output.contains("alloca i64, i64 2"),
            "StructInstance should alloca for 2 fields. Got: {}", output);
        assert!(output.contains("add i64 0, 10"),
            "StructInstance should load field value 10. Got: {}", output);
        assert!(output.contains("add i64 0, 20"),
            "StructInstance should load field value 20. Got: {}", output);
        assert!(output.contains("ptrtoint i64*"),
            "StructInstance should return ptrtoint. Got: {}", output);
    }

    #[test]
    fn test_field_access_resolves_correct_offset() {
        let mut backend = LlvmBackend::new();
        let body = vec![
            Statement::Let {
                name: "p".to_string(),
                ty: Some(Type::Custom("Point".to_string())),
                expr: Some(Expr::StructInstance("Point".to_string(), vec![
                    ("x".to_string(), Expr::Integer(10)),
                    ("y".to_string(), Expr::Integer(20)),
                ])),
                address: None, address_expr: None, bit_range: None,
                is_override: false, modifiers: vec![],
                range_constraint: None,
            },
            Statement::Assignment {
                lhs: Expr::Identifier("pt".to_string()),
                expr: Expr::FieldAccess(
                    Box::new(Expr::Identifier("p".to_string())),
                    "y".to_string(),
                ),
                timeout: None, modifiers: vec![],
            },
        ];
        let output = backend.generate(&make_point_program(body));
        assert!(output.contains("getelementptr i64, i64*"),
            "FieldAccess should emit GEP. Got: {}", output);
    }

    #[test]
    fn test_field_access_unknown_struct_falls_back() {
        let mut backend = LlvmBackend::new();
        fn empty_contract() -> Contract {
            Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None }
        }
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "raw".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "bad".to_string(),
                    is_reactive: false, parameters: vec![],
                    contract: empty_contract(),
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("raw".to_string()),
                            expr: Expr::FieldAccess(
                                Box::new(Expr::Identifier("raw".to_string())),
                                "nonexistent".to_string(),
                            ),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("add i64 0, 0 ; field"),
            "Unknown struct FieldAccess should emit fallback. Got: {}", output);
    }

    #[test]
    fn test_object_literal_emits_alloca_store_ptrtoint() {
        let mut backend = LlvmBackend::new();
        fn empty_contract() -> Contract {
            Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None }
        }
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "obj".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "make_obj".to_string(),
                    is_reactive: false, parameters: vec![],
                    contract: empty_contract(),
                    body: vec![
                        Statement::Let {
                            name: "o".to_string(),
                            ty: None,
                            expr: Some(Expr::ObjectLiteral(vec![
                                ("name".to_string(), Expr::String("test".to_string())),
                                ("value".to_string(), Expr::Integer(42)),
                            ])),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("alloca i64, i64 2"),
            "ObjectLiteral should alloca for fields. Got: {}", output);
        assert!(output.contains("ptrtoint i64*"),
            "ObjectLiteral should return ptrtoint. Got: {}", output);
    }

    // ── Enum codegen tests ────────────────────────────────────

    #[test]
    fn test_enum_type_registered_and_variant_disc() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Option".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("None".to_string()),
                        EnumVariant::Tuple("Some".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
            ],
            ..empty_program()
        };
        let _ = backend.generate(&program);
        assert!(backend.enum_types.contains_key("Option"));
        assert!(backend.variant_disc.contains_key("None"));
        assert!(backend.variant_disc.contains_key("Some"));
        assert_eq!(backend.variant_disc.get("None").map(|(_, d, _)| *d), Some(0));
        assert_eq!(backend.variant_disc.get("Some").map(|(_, d, _)| *d), Some(1));
        assert_eq!(backend.variant_disc.get("Some").map(|(_, _, f)| *f), Some(1));
    }

    #[test]
    fn test_enum_constructor_uses_registered_discriminant() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Result".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("Err".to_string()),
                        EnumVariant::Tuple("Ok".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "r".to_string(), ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "wrap".to_string(), is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body: vec![
                        Statement::Let {
                            name: "x".to_string(), ty: None,
                            expr: Some(Expr::Call("Ok".to_string(), vec![Expr::Integer(42)])),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("store i64 1"), "Ok should have disc 1. Got: {}", output);
        assert!(output.contains("store i64 %t"), "Ok should store payload register. Got: {}", output);
    }

    #[test]
    fn test_pattern_match_uses_registered_discriminant() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Status".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("Off".to_string()),
                        EnumVariant::Unit("On".to_string()),
                        EnumVariant::Unit("Error".to_string()),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "check".to_string(), ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "test".to_string(), is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body: vec![
                        Statement::Let {
                            name: "s".to_string(), ty: None,
                            expr: Some(Expr::Call("Error".to_string(), vec![])),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                        Statement::Let {
                            name: "matched".to_string(), ty: None,
                            expr: Some(Expr::PatternMatch {
                                value: Box::new(Expr::Identifier("s".to_string())),
                                variant: "Error".to_string(),
                                fields: vec![],
                            }),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("icmp eq i64"), "PatternMatch should compare discriminant. Got: {}", output);
    }

    #[test]
    fn test_match_arm_field_binding() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Option".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("None".to_string()),
                        EnumVariant::Tuple("Some".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "inner".to_string(), ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "unwrap".to_string(), is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("inner".to_string()),
                            expr: Expr::Match {
                                value: Box::new(Expr::Call("Some".to_string(), vec![Expr::Integer(7)])),
                                arms: vec![
                                        MatchArm {
                                            pattern: MatchPattern::Variant {
                                                name: "Some".to_string(),
                                                fields: vec![Pattern::Var("val".to_string())],
                                            },
                                        guard: None,
                                        body: Box::new(Expr::Identifier("val".to_string())),
                                    },
                                    MatchArm {
                                        pattern: MatchPattern::Wildcard,
                                        guard: None,
                                        body: Box::new(Expr::Integer(-1)),
                                    },
                                ],
                            },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("switch i64"), "Match should emit switch. Got: {}", output);
        assert!(output.contains("getelementptr i64, i64*"), "Field binding should GEP. Got: {}", output);
    }

    #[test]
    fn test_enum_multi_variant_discriminants() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Tree".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("Leaf".to_string()),
                        EnumVariant::Tuple("Node".to_string(), vec![Type::Int, Type::Int]),
                    ],
                    span: None,
                }),
            ],
            ..empty_program()
        };
        let _ = backend.generate(&program);
        assert_eq!(backend.variant_disc.get("Leaf").map(|(_, d, _)| *d), Some(0));
        assert_eq!(backend.variant_disc.get("Node").map(|(_, d, _)| *d), Some(1));
        assert_eq!(backend.variant_disc.get("Node").map(|(_, _, f)| *f), Some(2));
    }

    // ── Collection (list) tests ────────────────────────────────────

    #[test]
    fn test_list_literal_2slot_header() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "lst".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "mklist".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("lst".to_string()),
                            expr: Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20)]),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // 2-slot header means 4 slots: [data_ptr, len, elem0, elem1]
        assert!(output.contains("alloca i64, i64 4"), "2-elem list = 4 slots. Got: {}", output);
        assert!(output.contains("store i64 2, i64*"), "Length should be 2. Got: {}", output);
        assert!(output.contains("ptrtoint i64*"), "Should emit ptrtoint for data_ptr. Got: {}", output);
    }

    #[test]
    fn test_list_index_uses_2slot_header() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "elem".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "idx".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("elem".to_string()),
                            expr: Expr::ListIndex(
                                Box::new(Expr::ListLiteral(vec![Expr::Integer(99)])),
                                Box::new(Expr::Integer(0)),
                            ),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // ListIndex must load data_ptr from slot 0 before GEP
        assert!(output.contains("load i64, i64*"), "Should load data_ptr. Got: {}", output);
        assert!(output.contains("getelementptr i64, i64*"), "Should GEP from data. Got: {}", output);
    }

    #[test]
    fn test_list_len_loads_length() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "len".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "chk_len".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("len".to_string()),
                            expr: Expr::Projection { source: Box::new(Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2)])), target: ProjectionTarget::Size },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // Size projection must load length from slot 1, NOT return constant 0
        assert!(output.contains("load i64, i64*"), "Size projection should load from memory. Got: {}", output);
    }

    #[test]
    fn test_slice_emits_copy_loop() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "sliced".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "slice_op".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("sliced".to_string()),
                            expr: Expr::Slice {
                                value: Box::new(Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20), Expr::Integer(30)])),
                                start: Some(Box::new(Expr::Integer(1))),
                                end: Some(Box::new(Expr::Integer(3))),
                                stride: None,
                                mask: None,
                            },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // Slice should emit a counted loop (phi + icmp + br)
        assert!(output.contains("phi i64"), "Slice should emit a phi. Got: {}", output);
        assert!(output.contains("icmp slt"), "Slice should have loop condition. Got: {}", output);
    }

    #[test]
    fn test_multislice_index_delegates() {
        let mut backend = LlvmBackend::new();
        let mkv: Vec<Expr> = (0..5).map(|i| Expr::Integer(i)).collect();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "v".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "m".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("v".to_string()),
                            expr: Expr::MultiSlice {
                                value: Box::new(Expr::ListLiteral(mkv)),
                                ops: vec![BracketOp::Coord(SliceCoordinate::Index(Box::new(Expr::Integer(2))))],
                            },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // MultiSlice with single Index should load data_ptr and GEP
        assert!(output.contains("getelementptr i64, i64*"), "Should GEP. Got: {}", output);
    }

    // ── Tuple tests ────────────────────────────────────────────

    #[test]
    fn test_tuple_emits_2slot_header() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "t".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "mktup".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("t".to_string()),
                            expr: Expr::Tuple(vec![Expr::Integer(1), Expr::Integer(2), Expr::Integer(3)]),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("alloca i64, i64 5"), "3-elem tuple = 5 slots. Got: {}", output);
        assert!(output.contains("store i64 3, i64*"), "Length should be 3. Got: {}", output);
    }

    #[test]
    fn test_tuple_destructure_binds_variables() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "val".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "destr".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Let {
                            name: "$a_b".to_string(), ty: None,
                            expr: Some(Expr::TupleDestructure(
                                vec!["a".to_string(), "b".to_string()],
                                Box::new(Expr::Tuple(vec![Expr::Integer(5), Expr::Integer(6)])),
                            )),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                        Statement::Assignment {
                            lhs: Expr::Identifier("val".to_string()),
                            expr: Expr::Identifier("b".to_string()),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("add i64 0, %tdr"), "Should bind destructured vars. Got: {}", output);
    }

    #[test]
    fn test_list_index_assign_non_ssa() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "xs".to_string(), ty: Type::Int,
                    expr: Some(Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20), Expr::Integer(30)])),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "update".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::ListIndex(Box::new(Expr::Identifier("xs".to_string())), Box::new(Expr::Integer(1))),
                            expr: Expr::Integer(99),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("inttoptr i64"), "Should inttoptr list ptr. Output:\n{}", output);
        // store into list element: store i64 %t..., i64* %lep...
        assert!(output.contains("%lep") && output.contains("store i64"), "Should store at list element ptr. Output:\n{}", output);
    }

    #[test]
    fn test_slice_full_range_emitted() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "xs".into(), ty: Type::Int,
                    expr: Some(Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2), Expr::Integer(3), Expr::Integer(4), Expr::Integer(5)])),
                    address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "slice".into(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("xs".into()),
                            expr: Expr::Slice { value: Box::new(Expr::Identifier("xs".into())), start: Some(Box::new(Expr::Integer(1))), end: Some(Box::new(Expr::Integer(3))), stride: None, mask: None },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false, dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![], outputs: Vec::new(), output_type: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // Slice with start/end should emit a copy loop
        assert!(output.contains("phi") || output.contains("icmp"), "Slice should produce loop. Output:\n{}", output);
    }

    #[test]
    fn test_map_literal_emitted() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "m".into(), ty: Type::Custom("Map".into()),
                    expr: Some(Expr::MapLiteral(vec![(Expr::String("a".into()), Expr::Integer(1))])),
                    address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // MapLiteral falls through to stub — should not crash
        assert!(!output.is_empty());
    }

    #[test]
    fn test_set_literal_emitted() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "s".into(), ty: Type::Custom("Set".into()),
                    expr: Some(Expr::SetLiteral(vec![Expr::Integer(1), Expr::Integer(2)])),
                    address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_projection_keys_stub() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "k".into(), ty: Type::Int,
                    expr: Some(Expr::Projection { source: Box::new(Expr::Identifier("m".into())), target: ProjectionTarget::Keys }),
                    address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_projection_contains_stub() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "c".into(), ty: Type::Bool,
                    expr: Some(Expr::Projection { source: Box::new(Expr::Identifier("m".into())), target: ProjectionTarget::Contains(Box::new(Expr::String("k".into()))) }),
                    address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
                    range_constraint: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(!output.is_empty());
    }

    // (intrinsic_name field removed — intrinsics use name#() syntax instead)

    // ── IntrinsicCall codegen tests ─────────────────────────────

    fn make_intrinsic_program(intrinsic: Expr) -> Program {
        Program {
            items: vec![
                TopLevel::Transaction(Transaction {
                    name: "main".into(),
                    is_async: false,
                    is_reactive: false,
                    parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Let {
                            name: "r".into(),
                            ty: Some(Type::Int),
                            expr: Some(intrinsic),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                        Statement::Term { values: vec![None], modifiers: vec![], swan_song: None },
                    ],
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                    outputs: vec![],
                    output_type: None,
                }),
            ],
            ..empty_program()
        }
    }

    #[test]
    fn test_intrinsic_sqrt_emits_llvm_sqrt() {
        let mut backend = LlvmBackend::new();
        let program = make_intrinsic_program(
            Expr::IntrinsicCall {
                intrinsic: Intrinsic::Sqrt,
                args: vec![Expr::Float(9.0)],
            }
        );
        let output = backend.generate(&program);
        assert!(output.contains("call float @llvm.sqrt.f32"),
            "sqrt# should emit call to llvm.sqrt.f32. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_abs_emits_llvm_abs() {
        let mut backend = LlvmBackend::new();
        let program = make_intrinsic_program(
            Expr::IntrinsicCall {
                intrinsic: Intrinsic::Abs,
                args: vec![Expr::Integer(-42)],
            }
        );
        let output = backend.generate(&program);
        assert!(output.contains("call i64 @llvm.abs.i64"),
            "abs# should emit call to llvm.abs.i64. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_ctpop_emits_llvm_ctpop() {
        let mut backend = LlvmBackend::new();
        let program = make_intrinsic_program(
            Expr::IntrinsicCall {
                intrinsic: Intrinsic::Ctpop,
                args: vec![Expr::Integer(255)],
            }
        );
        let output = backend.generate(&program);
        assert!(output.contains("call i64 @llvm.ctpop.i64"),
            "ctpop# should emit call to llvm.ctpop.i64. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_ctlz_emits_llvm_ctlz() {
        let mut backend = LlvmBackend::new();
        let program = make_intrinsic_program(
            Expr::IntrinsicCall {
                intrinsic: Intrinsic::Ctlz,
                args: vec![Expr::Integer(1)],
            }
        );
        let output = backend.generate(&program);
        assert!(output.contains("call i64 @llvm.ctlz.i64"),
            "ctlz# should emit call to llvm.ctlz.i64. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_cttz_emits_llvm_cttz() {
        let mut backend = LlvmBackend::new();
        let program = make_intrinsic_program(
            Expr::IntrinsicCall {
                intrinsic: Intrinsic::Cttz,
                args: vec![Expr::Integer(8)],
            }
        );
        let output = backend.generate(&program);
        assert!(output.contains("call i64 @llvm.cttz.i64"),
            "cttz# should emit call to llvm.cttz.i64. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_bitreverse_emits_llvm_bitreverse() {
        let mut backend = LlvmBackend::new();
        let program = make_intrinsic_program(
            Expr::IntrinsicCall {
                intrinsic: Intrinsic::Bitreverse,
                args: vec![Expr::Integer(1)],
            }
        );
        let output = backend.generate(&program);
        assert!(output.contains("call i64 @llvm.bitreverse.i64"),
            "bitreverse# should emit call to llvm.bitreverse.i64. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_fabs_emits_llvm_fabs() {
        let mut backend = LlvmBackend::new();
        // fabs returns float, so we use a float result slot
        let program = Program {
            items: vec![
                TopLevel::Transaction(Transaction {
                    name: "main".into(),
                    is_async: false,
                    is_reactive: false,
                    parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Let {
                            name: "r".into(),
                            ty: Some(Type::Float),
                            expr: Some(Expr::IntrinsicCall {
                                intrinsic: Intrinsic::Fabs,
                                args: vec![Expr::Float(-3.5)],
                            }),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                        Statement::Term { values: vec![None], modifiers: vec![], swan_song: None },
                    ],
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                    outputs: vec![],
                    output_type: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("call float @llvm.fabs.f32"),
            "fabs# should emit call to llvm.fabs.f32. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_floor_emits_llvm_floor() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Transaction(Transaction {
                    name: "main".into(),
                    is_async: false,
                    is_reactive: false,
                    parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Let {
                            name: "r".into(),
                            ty: Some(Type::Float),
                            expr: Some(Expr::IntrinsicCall {
                                intrinsic: Intrinsic::Floor,
                                args: vec![Expr::Float(3.8)],
                            }),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                        Statement::Term { values: vec![None], modifiers: vec![], swan_song: None },
                    ],
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                    outputs: vec![],
                    output_type: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("call float @llvm.floor.f32"),
            "floor# should emit call to llvm.floor.f32. Got:\n{}", output);
    }

    #[test]
    fn test_intrinsic_ceil_emits_llvm_ceil() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Transaction(Transaction {
                    name: "main".into(),
                    is_async: false,
                    is_reactive: false,
                    parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Let {
                            name: "r".into(),
                            ty: Some(Type::Float),
                            expr: Some(Expr::IntrinsicCall {
                                intrinsic: Intrinsic::Ceil,
                                args: vec![Expr::Float(3.2)],
                            }),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                            range_constraint: None,
                        },
                        Statement::Term { values: vec![None], modifiers: vec![], swan_song: None },
                    ],
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                    outputs: vec![],
                    output_type: None,
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("call float @llvm.ceil.f32"),
            "ceil# should emit call to llvm.ceil.f32. Got:\n{}", output);
    }
