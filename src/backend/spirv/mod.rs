/// SPIR-V backend — compiles Briev GPU kernels to SPIR-V binary modules.
///
/// 2026-07-15: v1 baseline. Supports `txn [idx < N]` kernels with
/// GetGlobalId#, GetLocalId#, WorkgroupSize#, Load#, and Store# intrinsics.
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
