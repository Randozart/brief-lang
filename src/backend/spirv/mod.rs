/// SPIR-V backend — compiles Briev GPU kernels to SPIR-V binary modules.
///
/// 2026-07-15: v1 baseline. 2026-08-23 (plan §2.1–2.2): real statement/
/// expression lowering + frontend accel-driven kernel selection. 2026-08-26
/// (plan §2.3): Load#/Store# take ADDRESS EXPRESSIONS rooted in program
/// state — `Load#(field)` / `Load#(field[i])` / `Store#(field[i], v)` —
/// lowered to AccessChain over the single StorageBuffer binding; numeric
/// addresses do not exist in a Vulkan kernel and error naming the fix.
/// Supported builtins: GetGlobalId#, GetLocalId#, WorkgroupSize#.
///
/// # Entry point
/// `compile_spirv(program, options) -> Result<Vec<u8>>`
///
/// The returned `Vec<u8>` is a valid SPIR-V binary suitable for Vulkan
/// or OpenCL consumption.

pub mod builder;
pub mod kernel;
pub mod lower;
pub mod normalizer;

use crate::ast::TopLevel;
use crate::backend::spirv::builder::SpirvBuilder;
use crate::backend::spirv::kernel::emit_kernel;

/// 2026-08-23 (Plan 0.2): SPIR-V's declared surface — v1 lowers the kernel
/// loop skeleton plus integer scalar compute inside bodies. Floats, strings,
/// collections and control flow beyond the induction loop are compile
/// errors until their emission lands (plan 2026-08-23-spirv-kernel-emission).
pub const CAPABILITIES: crate::backend::capabilities::BackendCapabilities =
    crate::backend::capabilities::BackendCapabilities {
        name: "SPIR-V (.spv GPU kernels)",
        nature: "a Vulkan/OpenCL compute kernel is bounded structured control \
                 flow over typed buffers",
        int_literals: true,
        bool_char_literals: true,
        int_ops: true,
        unary_ops: true,
        intrinsics: true,
        if_expr: true,
        let_stmt: true,
        assign_stmt: true,
        term_endprogram: true,
        ..crate::backend::capabilities::BackendCapabilities::NONE
    };

/// 2026-07-15: Compile a Briev program to SPIR-V binary.
///
/// # Parameters
/// * `program` — The typed AST (must be type-checked already)
/// * `entry_name` — The kernel entry point name (e.g., "main")
///
/// # Returns
/// A valid SPIR-V binary, or an error describing what went wrong.
pub fn compile_spirv(
    program: &[TopLevel],
    entry_name: &str,
    analysis: &crate::backend::AnalysisResults,
) -> Result<Vec<u8>, String> {
    compile_spirv_builder(program, entry_name, analysis)?.build()
}

/// 2026-08-23 (§2.2): frontend-driven selection AND module construction
/// without assembling — tests inspect the dr::Module directly. Kernels are
/// the ELIGIBLE entries of the accel analysis (the shape's proven statements
/// form the body); `entry_name` = "main" accepts any, a specific name must
/// exist among them.
pub fn compile_spirv_builder(
    program: &[TopLevel],
    entry_name: &str,
    analysis: &crate::backend::AnalysisResults,
) -> Result<SpirvBuilder, String> {
    let mut builder = SpirvBuilder::new();
    let mut emitted: Vec<String> = Vec::new();

    for item in program {
        if let TopLevel::Transaction(txn) = item {
            if let Some(entry) = analysis.accel.get(&txn.name) {
                if !entry.shape.eligible {
                    continue;
                }
                emit_kernel(&mut builder, &txn.name.clone(), &entry.shape, program)?;
                emitted.push(txn.name.clone());
            }
        }
    }

    if emitted.is_empty() {
        return Err(
            "no GPU kernels: no transaction passed the accel eligibility proof \
             (mark '!> accel:' on the module and bound the node with '[i < N]' \
             over a real counter)"
                .into(),
        );
    }
    if entry_name != "main" && !emitted.iter().any(|n| n == entry_name) {
        return Err(format!(
            "entry '{}' is not an eligible GPU kernel (eligible: {:?})",
            entry_name, emitted
        ));
    }

    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use rspirv::spirv;

    fn state_decl(name: &str, n: i64) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.into(),
            ty: Type::Vector(Box::new(Type::int()), vec![Dimension::Anonymous(n as usize)]),
            span: None,
        })
    }

    /// §2.3 tests: a DIRECT shape for lowering-focused fixtures — the accel
    /// eligibility model (§2.2) does not classify Load#/Store# bodies yet.
    fn raw_shape(
        index_var: &str,
        kernel_stmts: Vec<Statement>,
        reads: &[&str],
        writes: &[&str],
    ) -> crate::analysis::accel::KernelShape {
        crate::analysis::accel::KernelShape {
            index_var: index_var.into(),
            count_expr: Some(Expr::Decimal(64)),
            kernel_stmts,
            host_stmts: vec![],
            read_buffers: reads.iter().map(|s| s.to_string()).collect(),
            write_buffers: writes.iter().map(|s| s.to_string()).collect(),
            scalar_ins: vec![],
            eligible: true,
            reasons: vec![],
        }
    }

    /// Canonical accel-shape fixture (Design A): `i` is a real state counter
    /// incremented in the body; `out[i] = i * 2` is work-item affine. This is
    /// exactly the shape src/analysis/accel.rs proves eligible.
    fn scale_kernel_program() -> Vec<TopLevel> {
        // 2026-08-23: `!> accel: try_all` gates the analysis — without it
        // accel.analyze produces no entries at all (policy: absent means
        // keyword-bodies-only).
        let mut meta = std::collections::HashMap::new();
        meta.insert(
            "accel".into(),
            crate::ast::PropertyValue::String("try_all".into()),
        );
        vec![
            TopLevel::ModuleMetadata(meta),
            TopLevel::StateDecl(StateDecl {
                name: "i".into(),
                ty: Type::int(),
                span: None,
            }),
            state_decl("out", 1024),
            TopLevel::Transaction(Transaction {
                name: "scale".into(),
                is_reactive: true,
                is_async: false,
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![],
                contract: Contract {
                    pre_condition: Expr::BinaryOp(
                        BinaryOpKind::Lt,
                        Box::new(Expr::Identifier("i".into())),
                        Box::new(Expr::Decimal(64)),
                    ),
                    post_condition: Expr::Bool(true),
                    watchdog: None,
                    explicit: false,
                    span: None,
                },
                body: vec![
                    Statement::Assign(
                        Expr::Index(
                            Box::new(Expr::Identifier("out".into())),
                            Box::new(Expr::Identifier("i".into())),
                        ),
                        Expr::BinaryOp(
                            BinaryOpKind::Mul,
                            Box::new(Expr::Identifier("i".into())),
                            Box::new(Expr::Decimal(2)),
                        ),
                    ),
                    Statement::Assign(
                        Expr::Identifier("i".into()),
                        Expr::BinaryOp(
                            BinaryOpKind::Add,
                            Box::new(Expr::Identifier("i".into())),
                            Box::new(Expr::Decimal(1)),
                        ),
                    ),
                ],
                metadata: std::collections::HashMap::new(),
                derivation: None,
                modifiers: vec![],
                span: None,
                doc: None,
            }),
        ]
    }

    fn analyze(program: &[TopLevel]) -> crate::backend::AnalysisResults {
        let universe = crate::type_universe::TypeUniverse::new();
        crate::backend::analyze_program(program, false, 1, Some(&universe))
    }

    fn eligible_shape<'a>(
        analysis: &'a crate::backend::AnalysisResults,
    ) -> &'a crate::analysis::accel::KernelShape {
        let entry = analysis
            .accel
            .get("scale")
            .expect("fixture txn must be accel-analyzed");
        assert!(entry.shape.eligible, "fixture must be eligible: {:?}", entry.shape.reasons);
        &entry.shape
    }

    /// §2.1/§2.2: the lowered kernel contains real work-item compute.
    #[test]
    fn test_scale_kernel_lowers_real_body() {
        let program = scale_kernel_program();
        let analysis = analyze(&program);
        let shape = eligible_shape(&analysis).clone();
        let mut builder = SpirvBuilder::new();
        emit_kernel(&mut builder, "scale", &shape, &program)
            .expect("kernel with real body must compile");
        let m = builder.module_ref();

        let mut ops: Vec<rspirv::spirv::Op> = Vec::new();
        for g in &m.types_global_values { ops.push(g.class.opcode); }
        for f in &m.functions {
            for b in &f.blocks {
                for i in &b.instructions { ops.push(i.class.opcode); }
            }
        }
        assert!(ops.contains(&rspirv::spirv::Op::IMul),
            "body must contain IMul; ops={:?}", ops);
        assert!(ops.contains(&rspirv::spirv::Op::AccessChain),
            "state access must go through AccessChain");
        assert!(ops.contains(&rspirv::spirv::Op::Store),
            "state write must Store");

        let has_global_id_builtin = m.types_global_values.iter().any(|i| {
            i.class.opcode == rspirv::spirv::Op::Decorate
                && matches!(i.operands.get(1),
                    Some(rspirv::dr::Operand::Decoration(rspirv::spirv::Decoration::BuiltIn)))
                && matches!(i.operands.get(2),
                    Some(rspirv::dr::Operand::BuiltIn(rspirv::spirv::BuiltIn::GlobalInvocationId)))
        });
        assert!(has_global_id_builtin,
            "the index counter must bind to BuiltIn GlobalInvocationId");

        let ssbo = m.types_global_values.iter().any(|i| {
            i.class.opcode == rspirv::spirv::Op::Variable
                && matches!(i.operands.first(),
                    Some(rspirv::dr::Operand::StorageClass(rspirv::spirv::StorageClass::StorageBuffer)))
        });
        assert!(ssbo, "indexed state must lower to a StorageBuffer variable");
    }

    /// §2.5: spirv-val validation — typed-emission refactor closed the
    /// assembly bug (BUGS.md 2026-08-23 CLOSED).
    #[test]
    fn test_scale_kernel_passes_spirv_val() {
        if !std::process::Command::new("spirv-val").arg("--version").output().is_ok() {
            eprintln!("spirv-val not found — skipping");
            return;
        }
        let program = scale_kernel_program();
        let analysis = analyze(&program);
        let shape = eligible_shape(&analysis).clone();
        let mut builder = SpirvBuilder::new();
        emit_kernel(&mut builder, "scale", &shape, &program).unwrap();
        let binary = builder.build().unwrap();

        let dir = std::env::temp_dir().join(format!("briev_spv_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scale.spv");
        std::fs::write(&path, &binary).unwrap();
        let out = std::process::Command::new("spirv-val")
            .arg(&path)
            .output()
            .expect("spirv-val");
        assert!(
            out.status.success(),
            "spirv-val rejected:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// §2.1 read-path lock: a kernel READING two buffers (scale-and-add)
    /// must emit TWO AccessChains + loads feeding the compute — locks the
    /// SSBO read side, not just writes.
    #[test]
    fn test_mad_kernel_reads_two_buffers() {
        // out[i] = fa * (a[i] + b[i])
        // Policy gate — without '!> accel:' the analysis produces no entries.
        let mut meta = std::collections::HashMap::new();
        meta.insert("accel".into(), crate::ast::PropertyValue::String("try_all".into()));
        let program = vec![
            TopLevel::ModuleMetadata(meta),
            TopLevel::StateDecl(StateDecl {
                name: "i".into(),
                ty: Type::int(),
                span: None,
            }),
            state_decl("a", 256),
            state_decl("b", 256),
            state_decl("out", 256),
            TopLevel::Transaction(Transaction {
                name: "mad".into(),
                is_reactive: true,
                is_async: false,
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![],
                contract: Contract {
                    pre_condition: Expr::BinaryOp(
                        BinaryOpKind::Lt,
                        Box::new(Expr::Identifier("i".into())),
                        Box::new(Expr::Decimal(64)),
                    ),
                    post_condition: Expr::Bool(true),
                    watchdog: None,
                    explicit: false,
                    span: None,
                },
                body: vec![
                    Statement::Assign(
                        Expr::Index(
                            Box::new(Expr::Identifier("out".into())),
                            Box::new(Expr::Identifier("i".into())),
                        ),
                        Expr::BinaryOp(
                            BinaryOpKind::Add,
                            Box::new(Expr::Index(
                                Box::new(Expr::Identifier("a".into())),
                                Box::new(Expr::Identifier("i".into())),
                            )),
                            Box::new(Expr::Index(
                                Box::new(Expr::Identifier("b".into())),
                                Box::new(Expr::Identifier("i".into())),
                            )),
                        ),
                    ),
                    Statement::Assign(
                        Expr::Identifier("i".into()),
                        Expr::BinaryOp(
                            BinaryOpKind::Add,
                            Box::new(Expr::Identifier("i".into())),
                            Box::new(Expr::Decimal(1)),
                        ),
                    ),
                ],
                metadata: std::collections::HashMap::new(),
                derivation: None,
                modifiers: vec![],
                span: None,
                doc: None,
            }),
        ];
        let analysis = analyze(&program);
        let entry = analysis.accel.get("mad")
            .unwrap_or_else(|| panic!("mad must be analyzed; accel keys: {:?}",
                analysis.accel.keys().collect::<Vec<_>>()));
        assert!(entry.shape.eligible, "{:?}", entry.shape.reasons);
        assert!(entry.shape.write_buffers.contains(&"out".to_string()),
            "write buffers: {:?}", entry.shape.write_buffers);
        // Reads may be empty if the analysis classifies a[i]/b[i] as
        // work-item-affine loads folded into the write — the KERNEL-side
        // assertion below (AccessChains) is what locks the read path.
        let reads = entry.shape.read_buffers.clone();
        eprintln!("read_buffers={:?} scalars={:?}", reads, entry.shape.scalar_ins);

        let mut builder = SpirvBuilder::new();
        emit_kernel(&mut builder, "mad", &entry.shape, &program).unwrap();
        let m = builder.module_ref();
        let access_chains = m.functions.iter()
            .flat_map(|f| f.blocks.iter())
            .flat_map(|b| b.instructions.iter())
            .filter(|i| i.class.opcode == rspirv::spirv::Op::AccessChain)
            .count();
        // 3 chains: a[i] load, b[i] load, out[i] store (the index local
        // needs none). Locks BOTH read paths + the write path.
        assert!(access_chains >= 3,
            "two reads + one write need >=3 access chains; got {}",
            access_chains);
    }

    /// §2.3: Load#/Store# address forms — Load#(field[i]), Store#(field[i], v),
    /// Load#(scalar), Store#(scalar, v) all lower to SSBO AccessChains and the
    /// binary passes spirv-val.
    ///
    /// Shape is CONSTRUCTED directly: this test locks the §2.3 LOWERING.
    /// Frontend eligibility is §2.2's surface (its own tests) — the accel
    /// purity model does not yet classify Load#/Store# bodies, which is
    /// tracked in planned-features-tracker.md under SPIR-V follow-ups.
    #[test]
    fn test_load_store_address_forms_pass_spirv_val() {
        if !std::process::Command::new("spirv-val").arg("--version").output().is_ok() {
            eprintln!("spirv-val not found — skipping");
            return;
        }
        let load_call = |arg| Expr::Call("Load#".into(), vec![arg], None);
        let store_call = |addr, val| {
            Expr::Call("Store#".into(), vec![addr, val], None)
        };
        let program = vec![
            TopLevel::StateDecl(StateDecl { name: "i".into(), ty: Type::int(), span: None }),
            // Scalar state — Load#/Store#-only surface (no plain Assign to it).
            TopLevel::StateDecl(StateDecl { name: "total".into(), ty: Type::int(), span: None }),
            state_decl("a", 256),
            state_decl("out", 256),
            TopLevel::Transaction(Transaction {
                name: "ls".into(),
                is_reactive: true,
                is_async: false,
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![],
                contract: Contract {
                    pre_condition: Expr::BinaryOp(
                        BinaryOpKind::Lt,
                        Box::new(Expr::Identifier("i".into())),
                        Box::new(Expr::Decimal(64)),
                    ),
                    post_condition: Expr::Bool(true),
                    watchdog: None,
                    explicit: false,
                    span: None,
                },
                body: vec![
                    Statement::Assign(
                        Expr::Index(
                            Box::new(Expr::Identifier("out".into())),
                            Box::new(Expr::Identifier("i".into())),
                        ),
                        // out[i] = Load#(a[i]) + 1
                        Expr::BinaryOp(
                            BinaryOpKind::Add,
                            Box::new(load_call(Expr::Index(
                                Box::new(Expr::Identifier("a".into())),
                                Box::new(Expr::Identifier("i".into())),
                            ))),
                            Box::new(Expr::Decimal(1)),
                        ),
                    ),
                    // total = Store#(total, Load#(total) + i)  (expression stmt)
                    Statement::Expression(store_call(
                        Expr::Identifier("total".into()),
                        Expr::BinaryOp(
                            BinaryOpKind::Add,
                            Box::new(load_call(Expr::Identifier("total".into()))),
                            Box::new(Expr::Identifier("i".into())),
                        ),
                    )),
                    Statement::Assign(
                        Expr::Identifier("i".into()),
                        Expr::BinaryOp(
                            BinaryOpKind::Add,
                            Box::new(Expr::Identifier("i".into())),
                            Box::new(Expr::Decimal(1)),
                        ),
                    ),
                ],
                metadata: std::collections::HashMap::new(),
                derivation: None,
                modifiers: vec![],
                span: None,
                doc: None,
            }),
        ];
        // Direct shape: §2.3 locks the LOWERING; eligibility is §2.2's
        // separate surface (see this file's selection tests + tracker note).
        let txn_stmts = match &program.last().unwrap() {
            TopLevel::Transaction(t) => t.body.clone(),
            other => panic!("expected transaction, got {other:?}"),
        };
        let shape = crate::analysis::accel::KernelShape {
            index_var: "i".into(),
            count_expr: Some(Expr::Decimal(64)),
            kernel_stmts: txn_stmts,
            host_stmts: vec![],
            read_buffers: vec!["a".into()],
            write_buffers: vec!["out".into(), "total".into()],
            scalar_ins: vec![],
            eligible: true,
            reasons: vec![],
        };

        let mut builder = SpirvBuilder::new();
        emit_kernel(&mut builder, "ls", &shape, &program).unwrap();
        // Count inside a scope: module_ref borrows; build() consumes.
        let chain_count = {
            let m = builder.module_ref();
            m.functions.iter()
                .flat_map(|f| f.blocks.iter())
                .flat_map(|b| b.instructions.iter())
                .filter(|inst| inst.class.opcode == spirv::Op::AccessChain)
                .count()
        };
        let binary = builder.build().unwrap();
        // a[i] load-chain, out[i] store-chain, total member-load chain,
        // total member-store chain (2 chains each side of the scalar RMW).
        assert!(chain_count >= 4,
            "two element forms + scalar load/store need >=4 access chains; got {}",
            chain_count);

        let dir = std::env::temp_dir().join(format!("briev_spv_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("load_store.spv");
        std::fs::write(&path, &binary).unwrap();
        let out = std::process::Command::new("spirv-val")
            .arg(&path)
            .output()
            .expect("spirv-val");
        assert!(
            out.status.success(),
            "spirv-val rejected:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// §2.3 honesty: a non-address first argument is a CAPABILITY ERROR
    /// naming the valid forms — no numeric-address fallback, no silent drop.
    #[test]
    fn test_load_rejects_non_address_expressions() {
        let program = vec![
            TopLevel::StateDecl(StateDecl { name: "i".into(), ty: Type::int(), span: None }),
            TopLevel::StateDecl(StateDecl { name: "total".into(), ty: Type::int(), span: None }),
            TopLevel::Transaction(Transaction {
                name: "bad".into(),
                is_reactive: true,
                is_async: false,
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![],
                contract: Contract {
                    pre_condition: Expr::Bool(true),
                    post_condition: Expr::Bool(true),
                    watchdog: None,
                    explicit: false,
                    span: None,
                },
                body: vec![
                    // total = Store#(total, Load#(5)) — 5 is not an address.
                    Statement::Assign(
                        Expr::Identifier("total".into()),
                        Expr::Call(
                            "Load#".into(),
                            vec![Expr::Decimal(5)],
                            None,
                        ),
                    ),
                ],
                metadata: std::collections::HashMap::new(),
                derivation: None,
                modifiers: vec![],
                span: None,
                doc: None,
            }),
        ];
        let stmts = match &program.last().unwrap() {
            TopLevel::Transaction(t) => t.body.clone(),
            other => panic!("expected transaction, got {other:?}"),
        };
        let mut builder = SpirvBuilder::new();
        let err = emit_kernel(&mut builder, "bad", &raw_shape("i", stmts, &[], &["total"]), &program)
            .err()
            .expect("Load#(5) must be rejected");
        assert!(err.contains("not an address expression"), "{err}");
        assert!(err.contains("field"), "{err}"); // names the fix form
    }

    /// §2.3 width honesty: an explicit byte-count that disagrees with the
    /// declared field type errors naming both numbers.
    #[test]
    fn test_load_width_mismatch_errors() {
        let program = vec![
            TopLevel::StateDecl(StateDecl { name: "i".into(), ty: Type::int(), span: None }),
            state_decl("a", 16),
            TopLevel::Transaction(Transaction {
                name: "wbad".into(),
                is_reactive: true,
                is_async: false,
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![],
                contract: Contract {
                    pre_condition: Expr::Bool(true),
                    post_condition: Expr::Bool(true),
                    watchdog: None,
                    explicit: false,
                    span: None,
                },
                body: vec![Statement::Expression(Expr::Call(
                    "Load#".into(),
                    vec![
                        Expr::Index(
                            Box::new(Expr::Identifier("a".into())),
                            Box::new(Expr::Identifier("i".into())),
                        ),
                        // Int elements are 8 bytes; 4 disagrees.
                        Expr::Decimal(4),
                    ],
                    None,
                ))],
                metadata: std::collections::HashMap::new(),
                derivation: None,
                modifiers: vec![],
                span: None,
                doc: None,
            }),
        ];
        let stmts = match &program.last().unwrap() {
            TopLevel::Transaction(t) => t.body.clone(),
            other => panic!("expected transaction, got {other:?}"),
        };
        let mut builder = SpirvBuilder::new();
        let err = emit_kernel(&mut builder, "wbad", &raw_shape("i", stmts, &["a"], &[]), &program)
            .err()
            .expect("width mismatch must error");
        assert!(err.contains("byte-width"), "{err}");
    }

    /// Capability honesty + selection: ineligible bodies never become
    /// kernels, and a named entry that doesn't exist errors helpfully.
    #[test]
    fn test_selection_rejects_ineligible_and_honors_entry_name() {
        let program = scale_kernel_program();
        let analysis = analyze(&program);
        // Eligible + `!> accel:` metadata → "main" accepts any kernel.
        compile_spirv(&program, "main", &analysis)
            .expect("eligible fixture must build under wildcard entry");

        // A specific entry name must EXIST among eligible kernels.
        let err = compile_spirv_builder(&program, "nope", &analysis)
            .err()
            .expect("missing named entry must error");
        assert!(err.contains("'nope'"), "{err}");
        compile_spirv_builder(&program, "scale", &analysis)
            .expect("named existing entry compiles");

        // Ineligible body (counter never incremented) → not a kernel.
        let mut bad = scale_kernel_program();
        if let TopLevel::Transaction(t) = &mut bad[3] {
            t.body.pop(); // drop the i = i + 1 increment
        }
        let analysis_bad = analyze(&bad);
        assert!(!analysis_bad.accel.get("scale").map_or(false, |e| e.shape.eligible));
        let err = compile_spirv(&bad, "main", &analysis_bad)
            .err()
            .expect("ineligible body must not become a kernel");
        assert!(err.contains("no GPU kernels"), "{err}");
    }
}
