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
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use crate::ast::*;
use crate::errors::{Diagnostic, Severity, Span};
use crate::ffi;
use crate::symbolic;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

pub use crate::errors::TypeError;

#[derive(Debug, Clone, PartialEq)]
pub enum ResultCheckStatus {
    Unchecked,
    CheckedOk,
    CheckedErr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompilationTarget {
    Interpreter,
    Wasm,
    Verilog,
}

pub struct TypeChecker {
    scopes: Vec<HashMap<String, Type>>,
    errors: RefCell<Vec<crate::errors::TypeError>>,
    diagnostics: RefCell<Vec<Diagnostic>>,
    source: String,
    current_file: PathBuf,
    no_stdlib: bool,
    custom_stdlib_path: Option<PathBuf>,
    signatures: HashMap<String, Signature>,
    definitions: HashMap<String, Definition>,
    ffi_results: RefCell<HashMap<String, ResultCheckStatus>>,
    foreign_bindings: HashMap<String, ForeignSignature>,
    pub target: CompilationTarget,
    enum_variants: HashMap<String, String>,  // variant_name -> enum_name
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            scopes: vec![HashMap::new()],
            errors: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
            source: String::new(),
            current_file: PathBuf::from("main.bv"),
            no_stdlib: false,
            custom_stdlib_path: None,
            signatures: HashMap::new(),
            definitions: HashMap::new(),
            ffi_results: RefCell::new(HashMap::new()),
            foreign_bindings: HashMap::new(),
            target: CompilationTarget::Interpreter,
            enum_variants: HashMap::new(),
        }
    }

    pub fn with_target(mut self, target: CompilationTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = source;
        self
    }

    pub fn with_file(mut self, file: PathBuf) -> Self {
        self.current_file = file;
        self
    }

    pub fn with_stdlib_config(mut self, no_stdlib: bool, custom_path: Option<PathBuf>) -> Self {
        self.no_stdlib = no_stdlib;
        self.custom_stdlib_path = custom_path;
        self
    }

    fn register_stdlib_signatures(&mut self) {
        // Add stdlib function signatures for type checking
        // to_json(value: Object) -> String
        self.signatures.insert(
            "to_json".to_string(),
            Signature {
                name: "to_json".to_string(),
                input_types: vec![Type::Custom("Object".to_string())],
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // from_json(json_str: String) -> Result<Object, String>
        self.signatures.insert(
            "from_json".to_string(),
            Signature {
                name: "from_json".to_string(),
                input_types: vec![Type::String],
                result_type: ResultType::Projection(vec![Type::Applied(
                    "Result".to_string(),
                    vec![Type::Custom("Object".to_string()), Type::String],
                )]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // StringBuilder functions
        self.signatures.insert(
            "new_builder".to_string(),
            Signature {
                name: "new_builder".to_string(),
                input_types: vec![],
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append_str".to_string(),
            Signature {
                name: "append_str".to_string(),
                input_types: vec![Type::Custom("StringBuilder".to_string()), Type::String],
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append_char".to_string(),
            Signature {
                name: "append_char".to_string(),
                input_types: vec![Type::Custom("StringBuilder".to_string()), Type::Char],
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append_int".to_string(),
            Signature {
                name: "append_int".to_string(),
                input_types: vec![Type::Custom("StringBuilder".to_string()), Type::Int],
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "to_string".to_string(),
            Signature {
                name: "to_string".to_string(),
                input_types: vec![Type::Custom("StringBuilder".to_string())],
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "String".to_string(),
            Signature {
                name: "String".to_string(),
                input_types: vec![Type::Int],
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "char_to_string".to_string(),
            Signature {
                name: "char_to_string".to_string(),
                input_types: vec![Type::Char],
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // Register stdlib enum variants for type checking
        // Option<T>
        self.enum_variants.insert("Some".to_string(), "Option".to_string());
        self.enum_variants.insert("None".to_string(), "Option".to_string());
        // Result<T, E>
        self.enum_variants.insert("Ok".to_string(), "Result".to_string());
        self.enum_variants.insert("Err".to_string(), "Result".to_string());

        // Option<T> methods
        self.signatures.insert(
            "is_some".to_string(),
            Signature {
                name: "is_some".to_string(),
                input_types: vec![Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])],
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_none".to_string(),
            Signature {
                name: "is_none".to_string(),
                input_types: vec![Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])],
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "unwrap".to_string(),
            Signature {
                name: "unwrap".to_string(),
                input_types: vec![Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])],
                result_type: ResultType::Projection(vec![Type::TypeVar("T".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );
    }

    pub fn check_program(&mut self, program: &mut Program) -> Vec<TypeError> {
        self.source = String::new();
        self.scopes = vec![HashMap::new()];
        self.errors = RefCell::new(Vec::new());

        self.register_stdlib_signatures();

        // Pass 1: Collect all signatures and definitions for global visibility
        for item in &program.items {
            match item {
                TopLevel::Signature(sig) => {
                    let key = sig.name.clone();
                    self.signatures.insert(key, sig.clone());
                }
                TopLevel::Definition(defn) => {
                    self.definitions.insert(defn.name.clone(), defn.clone());
                }
                TopLevel::ForeignBinding {
                    name, signature, ..
                } => {
                    // Collect foreign binding signature for type inference
                    self.foreign_bindings
                        .insert(name.clone(), signature.clone());
                }
                TopLevel::Enum(enum_def) => {
                    for variant in &enum_def.variants {
                        let variant_name = match variant {
                            crate::ast::EnumVariant::Unit(name) => name.clone(),
                            crate::ast::EnumVariant::Tuple(name, _) => name.clone(),
                            crate::ast::EnumVariant::Struct(name, _) => name.clone(),
                        };
                        self.enum_variants.insert(variant_name, enum_def.name.clone());
                    }
                }
                _ => {}
            }
        }

        for item in &mut program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.declare_variable(&decl.name, decl.ty.clone());
                    if let Some(expr) = &decl.expr {
                        let expr_ty = self.infer_expression(expr);
                        if !self.types_compatible(&decl.ty, &expr_ty) {
                            let mut diag = Diagnostic::new("B002", Severity::Error, "type mismatch")
                                .with_explanation(&format!(
                                    "expected {} for initial value of state variable '{}', but found {}",
                                    self.type_to_string(&decl.ty),
                                    decl.name,
                                    self.type_to_string(&expr_ty)
                                ));
                            if let Some(span) = decl.span {
                                diag = diag.with_span(span);
                            }
                            self.diagnostics.borrow_mut().push(diag);

                            self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                expected: self.type_to_string(&decl.ty),
                                found: self.type_to_string(&expr_ty),
                                context: format!("initial value of state variable '{}'", decl.name),
                            });
                        }
                    } else {
                        let mut diag =
                            Diagnostic::new("B002", Severity::Warning, "uninitialized signal")
                                .with_explanation(&format!(
                                    "signal '{}' has no initial value specified",
                                    decl.name
                                ))
                                .with_hint(&format!(
                                    "add an initial value: let {}: {} = 0;",
                                    decl.name,
                                    self.type_to_string(&decl.ty)
                                ))
                                .with_note(
                                    "uninitialized signals may contain garbage values at runtime",
                                );
                        if let Some(span) = decl.span {
                            diag = diag.with_span(span);
                        }
                        self.diagnostics.borrow_mut().push(diag);
                    }
                }
                TopLevel::Constant(cons) => {
                    self.declare_variable(&cons.name, cons.ty.clone());
                }
                TopLevel::Signature(sig) => {
                    self.check_signature(sig);
                }
                TopLevel::Definition(defn) => {
                    self.check_definition(defn);
                }
                TopLevel::Transaction(txn) => {
                    self.check_transaction(txn);
                }
                TopLevel::Trigger(trg) => {
                    self.declare_variable(&trg.name, trg.ty.clone());
                }
                TopLevel::ForeignBinding {
                    name,
                    toml_path,
                    signature,
                    ..
                } => {
                    self.check_frgn_binding(name, toml_path, signature);
                    if let Some(stored_sig) = self.foreign_bindings.get_mut(name) {
                        stored_sig.wasm_impl = signature.wasm_impl.clone();
                        stored_sig.wasm_setup = signature.wasm_setup.clone();
                    }
                }
                _ => {}
            }
        }

        self.errors.borrow().clone()
    }

    pub fn get_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.borrow().clone()
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_variable(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_variable(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn resolve_type(&self, ty: Type) -> Type {
        match ty {
            Type::Custom(name) => {
                if self.signatures.contains_key(&name) {
                    Type::Sig(name)
                } else {
                    Type::Custom(name)
                }
            }
            other => other,
        }
    }

    fn check_signature(&mut self, sig: &Signature) {
        for input_ty in &sig.input_types {
            self.validate_type(input_ty);
        }
        match &sig.result_type {
            ResultType::Projection(types) => {
                for ty in types {
                    self.validate_type(ty);
                }
            }
            ResultType::TrueAssertion => {}
            ResultType::VoidType => {}
        }
    }

    fn check_definition(&mut self, defn: &Definition) {
        self.push_scope();
        for (param_name, param_ty) in &defn.parameters {
            let resolved_ty = self.resolve_type(param_ty.clone());
            self.declare_variable(param_name, resolved_ty);
        }

        let expected_output_types = self.get_expected_output_types(defn);
        for stmt in &defn.body {
            self.check_statement_with_outputs(stmt, None, &expected_output_types);
        }

        self.pop_scope();
    }

    fn get_expected_output_types(&self, defn: &Definition) -> Vec<Type> {
        if let Some(ref output_type) = defn.output_type {
            output_type.all_types()
        } else if !defn.outputs.is_empty() {
            defn.outputs.clone()
        } else {
            vec![]
        }
    }

    fn check_statement_with_outputs(
        &mut self,
        stmt: &Statement,
        is_async: Option<&bool>,
        expected_outputs: &[Type],
    ) {
        match stmt {
            Statement::Term(outputs) => {
                let actual_count = outputs.len();
                let expected_count = expected_outputs.len();

                if expected_count > 0 && actual_count != expected_count {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: format!("{} outputs", expected_count),
                        found: format!("{} outputs", actual_count),
                        context: "term statement output count".to_string(),
                    });
                }

                for (i, expr_opt) in outputs.iter().enumerate() {
                    if let Some(expr) = expr_opt {
                        let actual_ty = self.infer_expression(expr);
                        if i < expected_outputs.len() {
                            let expected_ty = &expected_outputs[i];
                            if !self.types_compatible(&actual_ty, expected_ty) {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(expected_ty),
                                    found: self.type_to_string(&actual_ty),
                                    context: format!("term output {}", i),
                                });
                            }
                        }
                        self.check_expr_for_function_calls(expr);
                    }
                }
            }
            _ => self.check_statement(stmt, is_async),
        }
    }

    fn check_expr_for_function_calls(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(func_name, args) => {
                self.verify_term_function_call(func_name, args);
                for arg in args {
                    self.check_expr_for_function_calls(arg);
                }
            }
            Expr::Add(left, right)
            | Expr::Sub(left, right)
            | Expr::Mul(left, right)
            | Expr::Div(left, right)
            | Expr::Mod(left, right)
            | Expr::Eq(left, right)
            | Expr::Ne(left, right)
            | Expr::Lt(left, right)
            | Expr::Le(left, right)
            | Expr::Gt(left, right)
            | Expr::Ge(left, right)
            | Expr::Or(left, right)
            | Expr::And(left, right)
            | Expr::BitAnd(left, right)
            | Expr::BitOr(left, right)
            | Expr::BitXor(left, right)
            | Expr::Shl(left, right)
            | Expr::Shr(left, right) => {
                self.check_expr_for_function_calls(left);
                self.check_expr_for_function_calls(right);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.check_expr_for_function_calls(inner);
            }
            Expr::FieldAccess(obj, _) => {
                self.check_expr_for_function_calls(obj);
            }
            Expr::ListLiteral(elems) => {
                for elem in elems {
                    self.check_expr_for_function_calls(elem);
                }
            }
            _ => {}
        }
    }

    fn verify_term_function_call(&mut self, func_name: &str, args: &[Expr]) {
        let defn = match self.definitions.get(func_name) {
            Some(d) => d,
            None => return,
        };

        let postcond = &defn.contract.post_condition;
        if !self.expr_has_result(postcond) {
            return;
        }

        let precond = &defn.contract.pre_condition;
        let mut state = symbolic::SymbolicState::new(precond);

        for (i, (param_name, _)) in defn.parameters.iter().enumerate() {
            if i < args.len() {
                state.assign(param_name, &args[i]);
            }
        }

        let verified = symbolic::satisfies_postcondition(postcond, &state);
        let postcond_str = format!("{:?}", postcond);
        if verified {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new(
                    "V101",
                    Severity::Info,
                    "Function call postcondition verified",
                )
                .with_explanation(&format!(
                    "term {} uses function '{}' which guarantees {} (symbolically verified)",
                    func_name, func_name, postcond_str
                )),
            );
        } else {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new("V102", Severity::Warning, "Function call postcondition may not be satisfied")
                    .with_explanation(&format!(
                        "term {} uses function '{}' with postcondition {} - could not verify symbolically",
                        func_name, func_name, postcond_str
                    )),
            );
        }
    }

    fn expr_has_result(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Identifier(name) => name == "result",
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Mod(l, r)
            | Expr::And(l, r)
            | Expr::Or(l, r) => self.expr_has_result(l) || self.expr_has_result(r),
            Expr::Not(inner) => self.expr_has_result(inner),
            Expr::Call(_, args) => args.iter().any(|a| self.expr_has_result(a)),
            _ => false,
        }
    }

    fn check_transaction(&mut self, txn: &Transaction) {
        self.push_scope();
        
        for (param_name, param_ty) in &txn.parameters {
            let resolved_ty = self.resolve_type(param_ty.clone());
            self.declare_variable(param_name, resolved_ty);
        }

        for stmt in &txn.body {
            self.check_statement(stmt, Some(&txn.is_async));
        }
        self.pop_scope();
    }

    fn check_frgn_binding(
        &mut self,
        name: &str,
        toml_path: &str,
        signature: &mut ForeignSignature,
    ) {
        // If toml_path is empty, we're using the new FFI syntax with profile-based resolution
        // Skip binding loading and use profile defaults
        if toml_path.is_empty() {
            // Use profile-based FFI - address and type mappings come from the FFI state
            // For now, just set a placeholder location
            signature.location = format!("<profile:{}>", name);
            return;
        }

        let resolved_path = match ffi::resolver::resolve_binding_path(
            toml_path,
            &None,
            &Some(self.current_file.clone()),
            self.no_stdlib,
            &self.custom_stdlib_path,
        ) {
            Ok(path) => path,
            Err(err) => {
                self.diagnostics.borrow_mut().push(
                    Diagnostic::new(
                        "F001",
                        Severity::Error,
                        "FFI binding path resolution failed",
                    )
                    .with_explanation(&format!(
                        "Failed to resolve binding path '{}': {}",
                        toml_path, err
                    )),
                );
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Path resolution failed for '{}': {}", name, err),
                });
                return;
            }
        };

        let bindings = match ffi::loader::load_binding(&resolved_path) {
            Ok(b) => b,
            Err(err) => {
                self.diagnostics.borrow_mut().push(
                    Diagnostic::new("F002", Severity::Error, "FFI binding file load failed")
                        .with_explanation(&format!(
                            "Failed to load binding file '{}': {}",
                            toml_path, err
                        )),
                );
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Failed to load binding file for '{}': {}", name, err),
                });
                return;
            }
        };

        let primary_binding = bindings.iter().find(|b| b.name == name);
        let binding = match primary_binding {
            Some(b) => b,
            None => {
                self.diagnostics.borrow_mut().push(
                    Diagnostic::new("F003", Severity::Error, "FFI binding not found")
                        .with_explanation(&format!(
                            "No binding found for '{}' in '{}'",
                            name, toml_path
                        )),
                );
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Binding '{}' not found in '{}'", name, toml_path),
                });
                return;
            }
        };

        signature.error_fields = binding.error_fields.clone();
        signature.location = binding.location.clone();
        signature.input_layout = binding.input_layout.clone();
        signature.output_layout = binding.output_layout.clone();
        signature.precondition = binding.precondition.clone();
        signature.postcondition = binding.postcondition.clone();
        signature.buffer_mode = binding.buffer_mode.clone();

        if let Err(err) = ffi::validator::validate_frgn_against_binding(signature, binding) {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new("F004", Severity::Error, "FFI binding validation failed")
                    .with_explanation(&format!(
                        "The frgn declaration for '{}' does not match its TOML binding: {}",
                        name, err
                    )),
            );
            self.errors.borrow_mut().push(TypeError::FFIError {
                message: format!("Binding validation failed for '{}': {}", name, err),
            });
        }
    }

    fn check_statement(&mut self, stmt: &Statement, is_async: Option<&bool>) {
        match stmt {
            Statement::Assignment { lhs, expr, timeout } => {
                self.check_expr_for_ffi_errors(lhs);
                self.check_expr_for_ffi_errors(expr);
                let lhs_ty = self.infer_expression(lhs);
                let expr_ty = self.infer_expression(expr);

                if let Some((_t_expr, _unit)) = timeout {
                    if !self.is_error_union(&lhs_ty) {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: "Union type containing Error".to_string(),
                            found: self.type_to_string(&lhs_ty),
                            context: "assignment with timeout".to_string(),
                        });
                    }
                }

                if !self.check_geometry(&lhs_ty, &expr_ty) {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: self.type_to_string(&lhs_ty),
                        found: self.type_to_string(&expr_ty),
                        context: "assignment".to_string(),
                    });
                }
            }
            Statement::Let { name, ty, expr, .. } => {
                let inferred_expr_ty = expr.as_ref().map(|e| {
                    self.check_expr_for_ffi_errors(e);
                    self.infer_expression(e)
                });
                let final_ty = ty.clone().or(inferred_expr_ty.clone());
                if let Some(final_type) = final_ty {
                    if let (Some(_), Some(expr_ty)) = (expr, &inferred_expr_ty) {
                        if !self.types_compatible(expr_ty, &final_type) {
                            self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                expected: self.type_to_string(&final_type),
                                found: self.type_to_string(expr_ty),
                                context: format!("let {}", name),
                            });
                        }
                    }
                    self.declare_variable(name, final_type);
                }
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let cond_ty = self.infer_expression(condition);
                if !self.types_compatible(&cond_ty, &Type::Bool) {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: self.type_to_string(&cond_ty),
                        context: "guard condition".to_string(),
                    });
                }
                for s in statements {
                    self.check_statement(s, is_async);
                }
            }
            Statement::LocalTrigger { name, ty, expr, .. } => {
                // Local trigger: trg! name: Type = expr;
                // Type-check the expression if present
                if let Some(e) = expr {
                    self.check_expr_for_ffi_errors(e);
                    let expr_ty = self.infer_expression(e);
                    if !self.types_compatible(&expr_ty, ty) {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: self.type_to_string(ty),
                            found: self.type_to_string(&expr_ty),
                            context: format!("trg! {} (local trigger)", name),
                        });
                    }
                }
                // Declare the trigger variable in the local transaction scope
                self.declare_variable(name, ty.clone());
            }
            _ => {}
        }
    }

    fn infer_expression(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Integer(_) => Type::Int,
            Expr::Float(_) => {
                if self.target == CompilationTarget::Verilog {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: "Fixed-point or Integer".to_string(),
                        found: "Float".to_string(),
                        context: "Verilog synthesis".to_string(),
                    });
                }
                Type::Float
            }
            Expr::String(_) => Type::String,
            Expr::Bool(_) => Type::Bool,
            Expr::Identifier(name) | Expr::OwnedRef(name) | Expr::PriorState(name) => self
                .lookup_variable(name)
                .unwrap_or(Type::Custom(name.clone())),
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
                self.binary_op_type(l, r, Type::Int, Type::Float)
            }
            Expr::Eq(_, _)
            | Expr::Ne(_, _)
            | Expr::Lt(_, _)
            | Expr::Le(_, _)
            | Expr::Gt(_, _)
            | Expr::Ge(_, _)
            | Expr::Or(_, _)
            | Expr::And(_, _) => Type::Bool,
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => self.infer_expression(e),
            Expr::Call(name, _) => {
                if let Some(fb) = self.foreign_bindings.get(name) {
                    fb.success_output
                        .first()
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or(Type::Void)
                } else if let Some(sig) = self.signatures.get(name) {
                    match &sig.result_type {
                        ResultType::Projection(types) => {
                            types.first().cloned().unwrap_or(Type::Void)
                        }
                        ResultType::TrueAssertion => Type::Bool,
                        ResultType::VoidType => Type::Void,
                    }
                } else if let Some(defn) = self.definitions.get(name) {
                    if let Some(ref output_type) = defn.output_type {
                        output_type.all_types().first().cloned().unwrap_or(Type::Void)
                    } else if !defn.outputs.is_empty() {
                        defn.outputs.first().cloned().unwrap_or(Type::Void)
                    } else {
                        Type::Void
                    }
                } else if let Some(enum_name) = self.enum_variants.get(name) {
                    Type::Custom(enum_name.clone())
                } else {
                    Type::Custom(name.clone())
                }
            }
            Expr::ListLiteral(elements) => {
                let elem_type = elements
                    .first()
                    .map(|e| self.infer_expression(e))
                    .unwrap_or(Type::TypeVar("T".to_string()));
                Type::Applied("List".to_string(), vec![elem_type])
            }
            Expr::ListIndex(list_expr, _) => match self.infer_expression(list_expr) {
                Type::Applied(_, args) if !args.is_empty() => args[0].clone(),
                Type::Vector(inner, _) => *inner,
                _ => Type::TypeVar("T".to_string()),
            },
            Expr::Slice { value, mask, .. } => {
                if let Some(mask_expr) = mask {
                    let mask_type = self.infer_expression(mask_expr);
                    if mask_type != Type::Bool {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: "Bool".to_string(),
                            found: format!("{:?}", mask_type),
                            context: "Slice mask expression".to_string(),
                        });
                    }
                }
                self.infer_expression(value)
            },
            _ => Type::Custom("unknown".to_string()),
        }
    }

    fn is_error_union(&self, ty: &Type) -> bool {
        match ty {
            Type::Union(types) => types.iter().any(|t| self.is_error_type(t)),
            Type::Applied(name, _) | Type::Generic(name, _) => name == "Result",
            _ => false,
        }
    }

    fn is_error_type(&self, ty: &Type) -> bool {
        if let Type::Custom(name) = ty {
            name == "Error"
        } else {
            false
        }
    }

    fn check_geometry(&self, lhs: &Type, rhs: &Type) -> bool {
        match (lhs, rhs) {
            (Type::Vector(inner_lhs, size_lhs), Type::Vector(inner_rhs, size_rhs)) => {
                (*size_lhs == 0 || *size_rhs == 0 || size_lhs == size_rhs)
                    && self.check_geometry(inner_lhs, inner_rhs)
            }
            (Type::Vector(inner, _), scalar) | (scalar, Type::Vector(inner, _)) => {
                self.types_compatible(inner, scalar)
            }
            (a, b) => self.types_compatible(a, b),
        }
    }

    fn binary_op_type(&self, l: &Expr, r: &Expr, int_type: Type, float_type: Type) -> Type {
        let l_ty = self.infer_expression(l);
        let r_ty = self.infer_expression(r);
        match (&l_ty, &r_ty) {
            (Type::Vector(inner_l, size_l), Type::Vector(inner_r, size_r)) if size_l == size_r => {
                Type::Vector(
                    Box::new(self.binary_op_type_scalar(inner_l, inner_r, int_type, float_type)),
                    *size_l,
                )
            }
            (Type::Vector(inner, size), scalar) | (scalar, Type::Vector(inner, size)) => {
                Type::Vector(
                    Box::new(self.binary_op_type_scalar(inner, scalar, int_type, float_type)),
                    *size,
                )
            }
            _ => self.binary_op_type_scalar(&l_ty, &r_ty, int_type, float_type),
        }
    }

    fn binary_op_type_scalar(
        &self,
        l_ty: &Type,
        r_ty: &Type,
        int_type: Type,
        float_type: Type,
    ) -> Type {
        match (l_ty, r_ty) {
            (Type::UInt, Type::UInt) | (Type::Int, Type::UInt) | (Type::UInt, Type::Int) => {
                Type::UInt
            }
            (Type::Int, Type::Int) => int_type,
            (Type::Float, _) | (_, Type::Float) => float_type,
            _ => Type::Custom("unknown".to_string()),
        }
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Int, Type::Int)
            | (Type::UInt, Type::UInt)
            | (Type::Float, Type::Float)
            | (Type::String, Type::String)
            | (Type::Bool, Type::Bool)
            | (Type::Void, Type::Void)
            | (Type::Char, Type::Char)
            | (Type::Data, Type::Data) => true,
            (Type::Int, Type::UInt) | (Type::UInt, Type::Int) => true,
            (Type::Vector(ia, sa), Type::Vector(ib, sb)) => {
                sa == sb && self.types_compatible(ia, ib)
            }
            (Type::Applied(an, aa), Type::Applied(bn, ba)) => {
                an == bn && aa.len() == ba.len() && aa.iter().zip(ba.iter()).all(|(a, b)| self.types_compatible(a, b))
            }
            (Type::Sig(an), Type::Sig(bn)) => an == bn,
            (Type::Union(types), t) | (t, Type::Union(types)) => {
                types.iter().any(|u| self.types_compatible(u, t))
            }
            // Enum variant subtyping: if 'a' is a variant of enum 'b', they're compatible
            (Type::Custom(variant_name), Type::Applied(enum_name, _)) => {
                self.enum_variants.get(variant_name.as_str())
                    .map(|parent| parent == enum_name)
                    .unwrap_or(false)
            }
            // Custom types: exact match or variant-of-enum relationship
            (Type::Custom(a_name), Type::Custom(b_name)) => {
                if a_name == b_name {
                    true
                } else if let Some(parent) = self.enum_variants.get(a_name.as_str()) {
                    parent == b_name
                } else {
                    false
                }
            }
            // TypeVar is compatible with any type (generic placeholder)
            (Type::TypeVar(_), _) | (_, Type::TypeVar(_)) => true,
            // Generic types with same name and compatible args
            (Type::Generic(an, aa), Type::Generic(bn, ba)) => {
                an == bn && aa.len() == ba.len() && aa.iter().zip(ba.iter()).all(|(a, b)| self.types_compatible(a, b))
            }
            _ => false,
        }
    }

    fn validate_type(&self, ty: &Type) {
        match ty {
            Type::Union(types) | Type::Tuple(types) => {
                for t in types {
                    self.validate_type(t);
                }
            }
            Type::Applied(_, args) | Type::Generic(_, args) => {
                for t in args {
                    self.validate_type(t);
                }
            }
            _ => {}
        }
    }

    fn type_to_string(&self, ty: &Type) -> String {
        format!("{:?}", ty)
    }

    fn check_expr_for_ffi_errors(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(name, args) => {
                if self.foreign_bindings.contains_key(name) {
                    let binding = self.foreign_bindings.get(name).unwrap();
                    if !binding.success_output.is_empty() || binding.error_type_name != "Error" {
                        let mut diag = Diagnostic::new(
                            "T001",
                            Severity::Info,
                            "FFI call returns Result type",
                        )
                        .with_explanation(&format!(
                            "FFI function '{}' returns Result. Ensure both Success and Error branches are handled.",
                            name
                        ))
                        .with_hint("Use unification to handle both branches: Success(val) = func()");
                        self.diagnostics.borrow_mut().push(diag);
                    }
                    if !args.is_empty() && args.len() != binding.inputs.len() {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: format!("{} parameters", binding.inputs.len()),
                            found: format!("{} arguments", args.len()),
                            context: format!("FFI call '{}'", name),
                        });
                    }
                }
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.check_expr_for_ffi_errors(l);
                self.check_expr_for_ffi_errors(r);
            }
            Expr::FieldAccess(obj, _) => {
                self.check_expr_for_ffi_errors(obj);
            }
            _ => {}
        }
    }
}
