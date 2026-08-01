/// SPIR-V backend — compiles Brief GPU kernels to SPIR-V binary modules.
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
pub mod intrinsics;
pub mod kernel;
pub mod normalizer;
pub mod types;

use crate::ast::TopLevel;
use crate::backend::spirv::builder::SpirvBuilder;
use crate::backend::spirv::kernel::{emit_kernel, is_kernel};

/// 2026-07-15: Compile a Brief program to SPIR-V binary.
///
/// # Parameters
/// * `program` — The typed AST (must be type-checked already)
/// * `entry_name` — The kernel entry point name (e.g., "main")
///
/// # Returns
/// A valid SPIR-V binary, or an error describing what went wrong.
pub fn compile_spirv(program: &[TopLevel], entry_name: &str) -> Result<Vec<u8>, String> {
    let mut builder = SpirvBuilder::new();
    let mut kernel_count = 0u32;

    // 2026-07-15: Walk the program for GPU kernels
    for item in program {
        if let TopLevel::Transaction(txn) = item {
            if is_kernel(txn) {
                emit_kernel(&mut builder, txn)?;
                kernel_count += 1;
            }
        }
    }

    if kernel_count == 0 {
        return Err("no GPU kernels found: need txn [idx < N] precondition".into());
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    /// 2026-07-15: Kernel with [idx < 64] compiles to valid SPIR-V.
    #[test]
    fn test_minimal_kernel_compiles() {
        let contract = Contract {
            pre_condition: Expr::BinaryOp(
                BinaryOpKind::Lt,
                Box::new(Expr::Identifier("idx".into())),
                Box::new(Expr::Decimal(64)),
            ),
            post_condition: Expr::Bool(true),
            watchdog: None,
                explicit: false,
                span: None,
        };
        let txn = Transaction {
            name: "vec_add".into(),
            is_reactive: false,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract,
            body: vec![Statement::Term(Some(Expr::Decimal(0)))],
            metadata: std::collections::HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        };
        let program = vec![TopLevel::Transaction(txn)];
        let result = compile_spirv(&program, "vec_add");
        assert!(result.is_ok(), "kernel should compile: {:?}", result.err());
        let binary = result.unwrap();
        assert!(!binary.is_empty());
    }
}
