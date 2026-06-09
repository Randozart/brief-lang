use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub name: String,
    pub args: Vec<Expr>,
}

impl CallExpr {
    pub fn new(name: String, args: Vec<Expr>) -> Self {
        CallExpr { name, args }
    }
}

impl ExprTypecheck for CallExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        Ok(Type::Void)
    }
}

impl ExprEval for CallExpr {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch(String::new()))
    }
}

impl ExprCodegenLLVM for CallExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        crate::backend::llvm::TypedRegister { name: "%call".to_string(), ty: Type::Void }
    }
}

impl ExprCodegenVHDL for CallExpr {
    fn emit_vhdl(&self, _ctx: &crate::backend::vhdl::VhdlGenerator, _dispatch: &ExprDispatch) -> String {
        "'0'".to_string()
    }
}

impl ExprCodegenWebstack for CallExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::undefined".to_string()
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_call_expr_construct() {
        let e = CallExpr::new("foo".to_string(), vec![]);
        assert_eq!(e.name, "foo");
    }
}
