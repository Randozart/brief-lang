// ── CallExpr — Function Calls ──────────────────────────────────────
//
// Phase 1.3: Struct definition + 5 stub trait impls.
// Phase 9.4: ExprEval implementation — dispatches to definitions,
//   callable transactions, FFI registry, enum constructors, etc.

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
    fn typecheck(&self, ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        // Check argument types
        for arg in &self.args {
            ctx.infer_expression(arg);
        }
        Ok(Type::Int)
    }
}

impl ExprEval for CallExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let fn_name = &self.name;

        // Evaluate arguments
        let mut arg_values = Vec::new();
        for arg in self.args.iter() {
            arg_values.push(ctx.eval_expr(arg)?);
        }

        // Check user definitions BEFORE FFI/builtins, so user code
        // can override (e.g. custom parser for Option/Result types).
        if ctx.definitions.contains_key(fn_name) {
            return ctx.call_defn(fn_name, &self.args);
        }

        // Check callable transactions (non-reactive txns with convergence loops)
        if ctx.callable_txns.contains_key(fn_name) {
            return ctx.call_txn(fn_name, &self.args);
        }

        // Check dynamically linked FFI
        if ctx.frgn_registry.declarations.contains_key(fn_name) {
            // Pipe-syntax frgn with dynamic linking: unwrap the registry's Ok wrapping,
            // validate the raw value, and wrap in Ok/Err with the fallback.
            if let Some(sig) = ctx.ffi_bindings.get(fn_name) {
                if sig.is_pipe {
                    let reg_result = ctx.frgn_registry.call(fn_name, &arg_values)?;
                    let raw = match &reg_result {
                        Value::Enum(_, v, fields) if v == "Ok" => {
                            fields.get("value").cloned().unwrap_or(Value::Void)
                        }
                        _ => return Ok(reg_result),
                    };
                    return ctx.call_pipe_frgn(fn_name, raw);
                }
            }
            return ctx.frgn_registry.call(fn_name, &arg_values);
        }

        // Check for Value::Defn from state (registered defn aliases)
        let defn_call = ctx.state.get(fn_name).and_then(|v| {
            if let Value::Defn(n) = v {
                Some(n.clone())
            } else {
                None
            }
        });
        if let Some(defn_name) = defn_call {
            return ctx.call_defn(&defn_name, &self.args);
        }

        // Check if fn_name is an enum variant constructor
        let enum_construction = ctx.enum_variants.get(fn_name).cloned();
        if let Some(variant_info) = enum_construction {
            let mut fields = std::collections::HashMap::new();
            for (i, arg) in self.args.iter().enumerate() {
                let val = ctx.eval_expr(arg)?;
                if i < variant_info.field_names.len() {
                    fields.insert(variant_info.field_names[i].clone(), val);
                }
            }
            return Ok(Value::Enum(
                variant_info.enum_name,
                variant_info.variant_name,
                fields,
            ));
        }

        // Delegate to FFI registry: check if this name has a registered location.
        if let Some(location) = ctx.ffi_name_to_location.get(fn_name) {
            if let Some(frgn_fn) = ctx.foreign_functions.get(location) {
                if let Some(sig) = ctx.ffi_bindings.get(fn_name) {
                    // Pipe-syntax frgn: sentinel-based validation with fallback value
                    if sig.is_pipe {
                        let raw = frgn_fn(arg_values)?;
                        return ctx.call_pipe_frgn(fn_name, raw);
                    }
                    if sig.input_layout.is_some() || sig.output_layout.is_some() {
                        let binding = crate::ast::ForeignBinding::from_signature(sig);
                        return ctx.orchestrator.call(&binding, arg_values, *frgn_fn);
                    }
                }
                let result = frgn_fn(arg_values)?;
                return ctx.handle_ffi_result(fn_name, result);
            }
        }

        Err(RuntimeError::UndefinedForeignFunction(fn_name.clone()))
    }
}

impl ExprCodegenLLVM for CallExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::Call(self.name.clone(), self.args.clone()), "")
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
        let call = CallExpr::new("foo".into(), vec![Expr::Integer(42)]);
        assert_eq!(call.name, "foo");
    }
}
