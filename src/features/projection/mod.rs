pub mod collection;
pub mod scalar;

use crate::ast::{Expr, ProjectionTarget, Type};
use crate::features::projection::collection as coll;
use crate::features::projection::scalar as scal;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionExpr {
    pub source: Box<Expr>,
    pub target: ProjectionTarget,
}

impl ProjectionExpr {
    pub fn new(source: Expr, target: ProjectionTarget) -> Self {
        ProjectionExpr { source: Box::new(source), target }
    }
}

impl ExprTypecheck for ProjectionExpr {
    fn typecheck(&self, _ctx: &mut crate::typechecker::TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        Ok(Type::int())
    }
}

impl ExprEval for ProjectionExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let source_val = match ctx.eval_expr(&self.source)? {
            Value::Ref(inner) => *inner,
            v => v,
        };
        match &self.target {
            ProjectionTarget::Size => scal::eval_size_projection(&source_val),
            ProjectionTarget::Ptr => scal::eval_ptr_projection(&source_val),
            ProjectionTarget::IsEmpty => scal::eval_isempty_projection(&source_val),
            ProjectionTarget::Contains(key_expr) => coll::eval_contains_projection(ctx, &source_val, key_expr),
            ProjectionTarget::Get(key_expr) => coll::eval_get_projection(ctx, &source_val, key_expr),
            ProjectionTarget::Top => coll::eval_top_projection(&source_val),
            ProjectionTarget::Front => coll::eval_front_projection(&source_val),
            ProjectionTarget::BitRange(br) => scal::eval_bitrange_projection(&source_val, br),
            ProjectionTarget::Width => scal::eval_width_projection(&source_val),
            ProjectionTarget::Bytes => scal::eval_bytes_projection(&source_val),
            ProjectionTarget::Keys => coll::eval_keys_projection(&source_val),
            ProjectionTarget::Values => coll::eval_values_projection(&source_val),
            ProjectionTarget::Alignment => scal::eval_alignment_projection(&source_val),
            ProjectionTarget::Range => scal::eval_popcount_projection(&source_val),
            ProjectionTarget::Popcount => scal::eval_popcount_projection(&source_val),
            ProjectionTarget::LeadingZeros => scal::eval_leading_zeros_projection(&source_val),
            ProjectionTarget::TrailingZeros => scal::eval_trailing_zeros_projection(&source_val),
            ProjectionTarget::Absolute => scal::eval_absolute_projection(&source_val),
            ProjectionTarget::BitReverse => scal::eval_bitreverse_projection(&source_val),
            ProjectionTarget::Type => scal::eval_type_projection(&source_val),
            ProjectionTarget::PtrBang => scal::eval_ptrbang_projection(&source_val),
            ProjectionTarget::Endian => scal::eval_endian_projection(&source_val),
            ProjectionTarget::Codec => scal::eval_codec_projection(&source_val),
            ProjectionTarget::Ops => scal::eval_ops_projection(&source_val),
            ProjectionTarget::Elements => scal::eval_elements_projection(&source_val),
            ProjectionTarget::AsStack => coll::eval_asstack_projection(&source_val),
            ProjectionTarget::AsQueue => coll::eval_asqueue_projection(&source_val),
            ProjectionTarget::Address => scal::eval_address_projection(&source_val),
            ProjectionTarget::Name => scal::eval_name_projection(&source_val),
            ProjectionTarget::Params => scal::eval_params_projection(&source_val),
            ProjectionTarget::Returns => scal::eval_returns_projection(&source_val),
            ProjectionTarget::Arity => scal::eval_arity_projection(&source_val),
            ProjectionTarget::Loc => scal::eval_loc_projection(&source_val),
            ProjectionTarget::Doc => scal::eval_doc_projection(&source_val),
            ProjectionTarget::Hash => scal::eval_hash_projection(&source_val),
            ProjectionTarget::Contracts => scal::eval_contracts_projection(&source_val),
            ProjectionTarget::Module => scal::eval_module_projection(&source_val),
            ProjectionTarget::IsPure => scal::eval_ispure_projection(&source_val),
            ProjectionTarget::FnSpan => scal::eval_fnspan_projection(&source_val),
            ProjectionTarget::UserDefined(name) => scal::eval_user_defined_projection(&source_val, name),
            ProjectionTarget::UserDefinedWithArg(name, arg_expr) => scal::eval_user_projection_fast_path(ctx, &source_val, name, arg_expr),
        }
    }
}

impl crate::features::traits::ExprCodegenLLVM for ProjectionExpr {
    fn emit_llvm(&self,
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::Projection { source: self.source.clone(), target: self.target.clone() }, "")
    }
}

impl crate::features::traits::ExprCodegenWebstack for ProjectionExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::undefined".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::interpreter::Value;

    fn extract_bits(n: i64, lo: usize, hi: usize) -> i64 {
        if hi > 63 { return n; }
        let width = hi - lo + 1;
        let shifted = (n as u64) >> lo;
        if width >= 64 { shifted as i64 }
        else { (shifted & ((1u64 << width) - 1)) as i64 }
    }

    #[test]
    fn test_bit_range_single() {
        assert_eq!(extract_bits(0b1101, 2, 2), 1);
        assert_eq!(extract_bits(0b1101, 0, 0), 1);
        assert_eq!(extract_bits(0b1101, 3, 3), 1);
    }

    #[test]
    fn test_bit_range_range() {
        assert_eq!(extract_bits(0b1101, 0, 1), 0b01);
        assert_eq!(extract_bits(0b1101, 1, 2), 0b10);
        assert_eq!(extract_bits(0b1101, 0, 3), 0b1101);
    }

    #[test]
    fn test_bit_range_wide() {
        assert_eq!(extract_bits(255, 0, 7), 255);
        assert_eq!(extract_bits(0xFF00, 8, 11), 0xF);
        assert_eq!(extract_bits(0xFF00, 8, 15), 0xFF);
    }

    #[test]
    fn test_bit_range_exceeds_64() {
        assert_eq!(extract_bits(42, 0, 100), 42);
    }
}
