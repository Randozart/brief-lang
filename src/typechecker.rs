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
    }

    pub fn check_program(&mut self, program: &mut Program) -> Vec<TypeError> {
        // Add stdlib signatures (to_json, from_json, etc.)
        self.register_stdlib_signatures();

        // First pass: collect signatures, definitions, and foreign sigs
        for item in &program.items {
            match item {
                TopLevel::Signature(sig) => {
                    let key = sig.alias.clone().unwrap_or_else(|| sig.name.clone());
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
                _ => {}
            }
        }

        for item in &mut program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.declare_variable(&decl.name, decl.ty.clone());
                    if decl.expr.is_none() {
                        self.diagnostics.borrow_mut().push(
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
                                ),
                        );
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
                TopLevel::ForeignBinding {
                    name,
                    toml_path,
                    signature,
                    ..
                } => {
                    self.check_frgn_binding(name, toml_path, signature);
                    // Update the stored signature with populated wasm_impl from TOML
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
                // Check if name matches a signature
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
            | Expr::BitXor(left, right) => {
                self.check_expr_for_function_calls(left);
                self.check_expr_for_function_calls(right);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.check_expr_for_function_calls(arg);
                }
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

        // Handle any postcondition that references 'result'
        let has_result = self.expr_has_result(postcond);
        if !has_result {
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
                Diagnostic::new(
                    "V102",
                    Severity::Warning,
                    "Function call postcondition may not be satisfied",
                )
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
            | Expr::Ge(l, r) => self.expr_has_result(l) || self.expr_has_result(r),
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.expr_has_result(l) || self.expr_has_result(r)
            }
            Expr::And(l, r) | Expr::Or(l, r) => self.expr_has_result(l) || self.expr_has_result(r),
            Expr::Not(inner) => self.expr_has_result(inner),
            Expr::Call(_, args) => args.iter().any(|a| self.expr_has_result(a)),
            _ => false,
        }
    }

    fn build_argument_substitution(
        &self,
        params: &[(String, Type)],
        args: &[Expr],
    ) -> HashMap<String, Expr> {
        let mut subst = HashMap::new();
        for (i, (param_name, _)) in params.iter().enumerate() {
            if i < args.len() {
                subst.insert(param_name.clone(), args[i].clone());
            }
        }
        subst
    }

    fn simplify_substituted_postcondition(
        &self,
        expr: &Expr,
        _params: &[(String, Type)],
        _args: &[Expr],
    ) -> Expr {
        expr.clone()
    }

    fn check_transaction(&mut self, txn: &Transaction) {
        self.push_scope();

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
        // Resolve the TOML path using the FFI resolver
        let resolved_path = match ffi::resolver::resolve_binding_path(
            toml_path,
            &None,
            &Some(self.current_file.clone()),
            self.no_stdlib,
            &self.custom_stdlib_path,
        ) {
            Ok(path) => path,
            Err(err) => {
                let diag = Diagnostic::new(
                    "F001",
                    Severity::Error,
                    "FFI binding path resolution failed",
                )
                .with_explanation(&format!(
                    "Failed to resolve binding path '{}': {}",
                    toml_path, err
                ))
                .with_hint("Ensure the path is correct and the file exists");
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Path resolution failed for '{}': {}", name, err),
                });
                return;
            }
        };

        // Load the TOML binding file
        let bindings = match ffi::loader::load_binding(&resolved_path) {
            Ok(bindings) => bindings,
            Err(err) => {
                let diag = Diagnostic::new("F002", Severity::Error, "FFI binding file load failed")
                    .with_explanation(&format!(
                        "Failed to load binding file '{}': {}",
                        toml_path, err
                    ))
                    .with_hint("Ensure the TOML file is valid");
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Failed to load binding file for '{}': {}", name, err),
                });
                return;
            }
        };

        // Find the matching binding for this frgn
        let primary_binding = bindings.iter().find(|b| b.name == name);

        let binding = match primary_binding {
            Some(b) => b,
            None => {
                let diag = Diagnostic::new("F003", Severity::Error, "FFI binding not found")
                    .with_explanation(&format!(
                        "No binding found for '{}' in '{}'",
                        name, toml_path
                    ))
                    .with_hint(&format!(
                        "Available bindings in '{}': {}",
                        toml_path,
                        bindings
                            .iter()
                            .map(|b| format!("'{}'", b.name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Binding '{}' not found in '{}'", name, toml_path),
                });
                return;
            }
        };

        // Populate signature fields from the binding
        signature.error_fields = binding.error_fields.clone();
        signature.location = binding.location.clone();

        // Also look for a WASM-specific implementation if available
        let wasm_binding = bindings
            .iter()
            .find(|b| b.name == name && b.target == ForeignTarget::Wasm);
        if let Some(wb) = wasm_binding {
            signature.wasm_impl = wb.wasm_impl.clone();
            signature.wasm_setup = wb.wasm_setup.clone();
        } else {
            // Fallback to primary binding's wasm_impl if it exists
            signature.wasm_impl = binding.wasm_impl.clone();
            signature.wasm_setup = binding.wasm_setup.clone();
        }

        // Validate the frgn signature against the TOML binding
        if let Err(err) = ffi::validator::validate_frgn_against_binding(signature, binding) {
            let diag = Diagnostic::new("F004", Severity::Error, "FFI binding validation failed")
                .with_explanation(&format!(
                    "The frgn declaration for '{}' does not match its TOML binding: {}",
                    name, err
                ))
                .with_hint("Ensure the frgn signature matches the binding definition");
            self.diagnostics.borrow_mut().push(diag);
            self.errors.borrow_mut().push(TypeError::FFIError {
                message: format!("Binding validation failed for '{}': {}", name, err),
            });
            return;
        }

        // Validate that all FFI types are supported
        for (_, ty) in &signature.inputs {
            if !ffi::validator::is_valid_ffi_type(ty) {
                let diag = Diagnostic::new("F005", Severity::Error, "Invalid FFI type")
                    .with_explanation(&format!(
                        "Input parameter in '{}' uses unsupported type: {:?}",
                        name, ty
                    ))
                    .with_hint(
                        "FFI supports: String, Int, Float, Bool, Void, Data, and custom structs",
                    );
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Invalid FFI type in input for '{}'", name),
                });
                return;
            }
        }

        for (_, ty) in &signature.success_output {
            if !ffi::validator::is_valid_ffi_type(ty) {
                let diag = Diagnostic::new("F005", Severity::Error, "Invalid FFI type")
                    .with_explanation(&format!(
                        "Output parameter in '{}' uses unsupported type: {:?}",
                        name, ty
                    ))
                    .with_hint(
                        "FFI supports: String, Int, Float, Bool, Void, Data, and custom structs",
                    );
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Invalid FFI type in output for '{}'", name),
                });
                return;
            }
        }

        for (_, ty) in &signature.error_fields {
            if !ffi::validator::is_valid_ffi_type(ty) {
                let diag = Diagnostic::new("F005", Severity::Error, "Invalid FFI type")
                    .with_explanation(&format!(
                        "Error field in '{}' uses unsupported type: {:?}",
                        name, ty
                    ))
                    .with_hint(
                        "FFI supports: String, Int, Float, Bool, Void, Data, and custom structs",
                    );
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Invalid FFI type in error for '{}'", name),
                });
                return;
            }
        }
    }

    fn check_statement(&mut self, stmt: &Statement, is_async: Option<&bool>) {
        match stmt {
            Statement::Assignment { lhs, expr, timeout } => {
                self.check_expr_for_ffi_errors(lhs);
                self.check_expr_for_ffi_errors(expr);
                let lhs_ty = self.infer_expression(lhs);
                let expr_ty = self.infer_expression(expr);

                let var_name = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => Some(n.clone()),
                    Expr::ListIndex(list_expr, _) => {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = &**list_expr {
                            Some(n.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                // If timeout is used, the type must be a Union containing Error
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

                if let Expr::OwnedRef(name) = lhs {
                    let has_lower_scope = self
                        .scopes
                        .iter()
                        .take(self.scopes.len() - 1)
                        .any(|s| s.contains_key(name));
                    if !has_lower_scope {
                        self.errors
                            .borrow_mut()
                            .push(TypeError::OwnershipViolation {
                                var: name.clone(),
                                reason: "owned reference requires variable to exist in outer scope"
                                    .to_string(),
                            });
                    }
                }

                if let Some(name) = var_name {
                    if self.is_ffi_call(expr) {
                        self.ffi_results
                            .borrow_mut()
                            .insert(name.clone(), ResultCheckStatus::Unchecked);
                        self.diagnostics.borrow_mut().push(
                            Diagnostic::new("F101", Severity::Warning, "FFI call result not handled")
                                .with_explanation(&format!(
                                    "FFI function result assigned to '{}' without checking for errors. \
                                     Use .is_ok() or .is_err() to handle potential errors.",
                                    name
                                ))
                                .with_hint("Wrap the FFI call with is_ok()/is_err() guards"),
                        );
                    }
                }
            }
            Statement::Let {
                name,
                ty,
                expr,
                address: _,
                bit_range: _,
                is_override: _,
            } => {
                let mut inferred_expr_ty: Option<Type> = None;

                if let Some(e) = expr {
                    self.check_expr_for_ffi_errors(e);
                    inferred_expr_ty = Some(self.infer_expression(e));
                }

                let final_ty = ty.clone().or(inferred_expr_ty.clone());

                if let Some(final_type) = final_ty {
                    if let Some(e) = expr {
                        if let Some(expr_ty) = &inferred_expr_ty {
                            if !self.types_compatible(expr_ty, &final_type) {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(&final_type),
                                    found: self.type_to_string(expr_ty),
                                    context: format!("let {}", name),
                                });
                            }
                        }
                        if self.is_ffi_call(e) {
                            self.ffi_results
                                .borrow_mut()
                                .insert(name.clone(), ResultCheckStatus::Unchecked);
                        }
                    }
                    self.declare_variable(name, final_type);
                }
            }
            Statement::Expression(expr) => {
                self.check_expr_for_ffi_errors(expr);
                self.infer_expression(expr);
            }
            Statement::Term(outputs) => {
                for expr_opt in outputs {
                    if let Some(expr) = expr_opt {
                        self.check_expr_for_ffi_errors(expr);
                        self.infer_expression(expr);
                    }
                }
            }
            Statement::Escape(expr_opt) => {
                if let Some(expr) = expr_opt {
                    self.check_expr_for_ffi_errors(expr);
                    self.infer_expression(expr);
                }
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                self.check_expr_for_ffi_errors(condition);
                let cond_ty = self.infer_expression(condition);
                if !self.types_compatible(&cond_ty, &Type::Bool) {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: self.type_to_string(&cond_ty),
                        context: "guard condition".to_string(),
                    });
                }

                if let Expr::FieldAccess(obj, field) = condition {
                    if field == "is_ok" || field == "is_err" {
                        if let Expr::Identifier(var_name) = obj.as_ref() {
                            let status = if field == "is_ok" {
                                ResultCheckStatus::CheckedOk
                            } else {
                                ResultCheckStatus::CheckedErr
                            };
                            self.ffi_results
                                .borrow_mut()
                                .insert(var_name.clone(), status);
                        }
                    }
                }

                for stmt in statements {
                    self.check_statement(stmt, is_async);
                }
            }
            Statement::Unification {
                name,
                pattern,
                expr,
            } => {
                self.check_expr_for_ffi_errors(expr);
                self.infer_expression(expr);
                self.declare_variable(name, Type::Custom(pattern.clone()));
            }
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
            Expr::Identifier(name) => self
                .lookup_variable(name)
                .unwrap_or(Type::Custom(name.clone())),
            Expr::OwnedRef(name) => self
                .lookup_variable(name)
                .unwrap_or(Type::Custom(name.clone())),
            Expr::PriorState(name) => self
                .lookup_variable(name)
                .unwrap_or(Type::Custom(name.clone())),
            Expr::Add(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Sub(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Mul(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Div(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r) => {
                self.binary_op_type(l, r, Type::Int, Type::Int)
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
            Expr::Call(name, _args) => {
                // Check if it's a foreign binding
                if let Some(frgn_binding) = self.foreign_bindings.get(name) {
                    // Return the first output type from success_output
                    frgn_binding
                        .success_output
                        .first()
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or(Type::Custom(name.clone()))
                } else if let Some(sig) = self.signatures.get(name) {
                    // Check signature return type
                    match &sig.result_type {
                        crate::ast::ResultType::Projection(types) => {
                            types.first().cloned().unwrap_or(Type::Custom(name.clone()))
                        }
                        crate::ast::ResultType::TrueAssertion => Type::Bool,
                    }
                } else if let Some(defn) = self.definitions.get(name) {
                    // Return defn's return type
                    defn.outputs
                        .first()
                        .cloned()
                        .unwrap_or(Type::Custom(name.clone()))
                } else {
                    // Unknown function, return custom type
                    Type::Custom(name.clone())
                }
            }
            Expr::ListLiteral(elements) => {
                if elements.is_empty() {
                    Type::Applied("List".to_string(), vec![Type::TypeVar("T".to_string())])
                } else {
                    let elem_type = self.infer_expression(&elements[0]);
                    Type::Applied("List".to_string(), vec![elem_type])
                }
            }
            Expr::ListIndex(list_expr, _) => {
                let list_type = self.infer_expression(list_expr);
                match list_type {
                    Type::Applied(_, type_args) => {
                        if !type_args.is_empty() {
                            type_args[0].clone()
                        } else {
                            Type::TypeVar("T".to_string())
                        }
                    }
                    Type::Vector(inner, _) => *inner,
                    _ => Type::TypeVar("T".to_string()),
                }
            }
            Expr::Slice {
                value, start, end, ..
            } => {
                let base_ty = self.infer_expression(value);
                if let Type::Vector(inner, _) = base_ty {
                    let size = match (start, end) {
                        (Some(s), Some(e)) => {
                            if let (Expr::Integer(sv), Expr::Integer(ev)) = (&**s, &**e) {
                                (*ev - *sv) as usize
                            } else {
                                0
                            }
                        }
                        _ => 0, // Unknown size or dynamic slice
                    };
                    Type::Vector(inner, size)
                } else {
                    base_ty
                }
            }
            Expr::ListLen(_) => Type::Int,
            Expr::FieldAccess(_, _) => Type::Custom("unknown".to_string()),
            Expr::StructInstance(typename, _fields) => Type::Custom(typename.clone()),
            Expr::ObjectLiteral(_) => Type::Custom("ObjectLiteral".to_string()),
            Expr::PatternMatch { .. } => Type::Bool,
            Expr::ForAll { .. } | Expr::Exists { .. } => Type::Bool,
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
            (Type::Vector(inner, _), scalar) => self.types_compatible(inner, scalar),
            (scalar, Type::Vector(inner, _)) => self.types_compatible(scalar, inner),
            (a, b) => self.types_compatible(a, b),
        }
    }

    fn binary_op_type(&self, l: &Expr, r: &Expr, int_type: Type, float_type: Type) -> Type {
        let l_ty = self.infer_expression(l);
        let r_ty = self.infer_expression(r);

        // Handle vector lifting
        match (&l_ty, &r_ty) {
            (Type::Vector(inner_l, size_l), Type::Vector(inner_r, size_r)) => {
                if size_l != size_r {
                    // Geometry mismatch - return unknown or error
                    return Type::Custom("GeometryMismatch".to_string());
                }
                let res_inner = self.binary_op_type_scalar(inner_l, inner_r, int_type, float_type);
                return Type::Vector(Box::new(res_inner), *size_l);
            }
            (Type::Vector(inner, size), scalar) | (scalar, Type::Vector(inner, size)) => {
                let res_inner = self.binary_op_type_scalar(inner, scalar, int_type, float_type);
                return Type::Vector(Box::new(res_inner), *size);
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
            (Type::Float, Type::Float) => float_type,
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => float_type,
            (Type::UInt, Type::Float) | (Type::Float, Type::UInt) => float_type,
            (Type::String, _) | (_, Type::String) => Type::String,
            (Type::Applied(a, _), Type::Applied(b, _))
            | (Type::Generic(a, _), Type::Generic(b, _))
                if a == "List" && b == "List" =>
            {
                l_ty.clone()
            }
            (Type::Sig(_), Type::Sig(_)) => l_ty.clone(),
            _ => Type::Custom("unknown".to_string()),
        }
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Int, Type::Int) => true,
            (Type::UInt, Type::UInt) => true,
            (Type::Int, Type::UInt) | (Type::UInt, Type::Int) => true,
            (Type::Float, Type::Float) => true,
            (Type::String, Type::String) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Data, Type::Data) => true,
            (Type::Void, Type::Void) => true,
            (Type::Vector(inner_a, size_a), Type::Vector(inner_b, size_b)) => {
                size_a == size_b && self.types_compatible(inner_a, inner_b)
            }
            (Type::Custom(a), Type::Custom(b)) => a == b,
            (Type::Sig(a), Type::Sig(b)) => a == b, // Sig types match by name
            (Type::Sig(_), _) | (_, Type::Sig(_)) => false, // Sig doesn't match other types
            (Type::Union(types), t) | (t, Type::Union(types)) => {
                types.iter().any(|u| self.types_compatible(u, t))
            }
            (Type::TypeVar(a), Type::TypeVar(b)) => a == b,
            (Type::TypeVar(_), _) | (_, Type::TypeVar(_)) => true,
            (Type::Generic(a, args_a), Type::Generic(b, args_b)) => {
                a == b
                    && args_a.len() == args_b.len()
                    && args_a
                        .iter()
                        .zip(args_b.iter())
                        .all(|(x, y)| self.types_compatible(x, y))
            }
            (Type::Applied(a, args_a), Type::Applied(b, args_b)) => {
                a == b
                    && args_a.len() == args_b.len()
                    && args_a
                        .iter()
                        .zip(args_b.iter())
                        .all(|(x, y)| self.types_compatible(x, y))
            }
            // Constrained: compare inner types
            (Type::Constrained(inner_a, _), Type::Constrained(inner_b, _)) => {
                self.types_compatible(inner_a, inner_b)
            }
            (Type::Constrained(inner, _), other) | (other, Type::Constrained(inner, _)) => {
                self.types_compatible(inner, other)
            }

            // ContractBound: compare inner types, ignore the contract
            (Type::ContractBound(inner_a, _), Type::ContractBound(inner_b, _)) => {
                self.types_compatible(inner_a, inner_b)
            }
            (Type::ContractBound(inner, _), t) | (t, Type::ContractBound(inner, _)) => {
                self.types_compatible(inner, t)
            }
            (Type::Enum(a), Type::Enum(b)) => a == b,
            (Type::Enum(_), _) | (_, Type::Enum(_)) => false,
            _ => false,
        }
    }

    fn validate_type(&self, ty: &Type) {
        match ty {
            Type::Union(types) => {
                for t in types {
                    self.validate_type(t);
                }
            }
            Type::ContractBound(inner, _) => {
                self.validate_type(inner);
            }
            Type::Generic(_, type_args) | Type::Applied(_, type_args) => {
                for t in type_args {
                    self.validate_type(t);
                }
            }
            Type::Sig(name) => {
                if !self.signatures.contains_key(name) {
                    // Will be caught as error in check_definition if used incorrectly
                }
            }
            _ => {}
        }
    }

    fn type_to_string(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "Int".to_string(),
            Type::Float => "Float".to_string(),
            Type::String => "String".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::Data => "Data".to_string(),
            Type::Void => "Void".to_string(),
            Type::Custom(name) => name.clone(),
            Type::Sig(name) => format!("sig {}", name),
            Type::TypeVar(name) => name.clone(),
            Type::Union(types) => types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(" | "),
            Type::ContractBound(inner, guard) => {
                format!("{}[{:?}]", self.type_to_string(inner), guard)
            }
            Type::Generic(name, type_args) => {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Type::Applied(name, type_args) => {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Type::Option(inner) => {
                format!("Option<{}>", self.type_to_string(inner))
            }
            Type::Enum(name) => name.clone(),
            Type::UInt => "UInt".to_string(),
            Type::Vector(inner, size) => {
                format!("Vector<{}>[{}]", self.type_to_string(inner), size)
            }
            Type::Constrained(inner, _) => self.type_to_string(inner),
        }
    }

    fn is_ffi_call(&self, expr: &Expr) -> bool {
        if let Expr::Call(name, _) = expr {
            return self.signatures.iter().any(|(sig_name, sig)| {
                sig_name == name && sig.result_type == ResultType::Projection(vec![])
            });
        }
        false
    }

    fn check_expr_for_ffi_errors(&mut self, expr: &Expr) {
        match expr {
            Expr::FieldAccess(obj, field) => {
                if field == "value" || field == "error" {
                    if let Expr::Identifier(var_name) = obj.as_ref() {
                        if let Some(status) = self.ffi_results.borrow().get(var_name) {
                            if *status == ResultCheckStatus::Unchecked {
                                self.errors.borrow_mut().push(TypeError::FFIError {
                                    message: format!(
                                        "FFI result '{}' accessed with .{} before checking .is_ok() or .is_err()",
                                        var_name,
                                        field
                                    ),
                                });
                            }
                        }
                    }
                }
                self.check_expr_for_ffi_errors(obj);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.check_expr_for_ffi_errors(arg);
                }
            }
            Expr::Add(left, right)
            | Expr::Sub(left, right)
            | Expr::Mul(left, right)
            | Expr::Div(left, right)
            | Expr::Eq(left, right)
            | Expr::Ne(left, right)
            | Expr::BitAnd(left, right)
            | Expr::BitOr(left, right)
            | Expr::BitXor(left, right)
            | Expr::Lt(left, right)
            | Expr::Le(left, right)
            | Expr::Gt(left, right)
            | Expr::Ge(left, right)
            | Expr::Or(left, right)
            | Expr::And(left, right) => {
                self.check_expr_for_ffi_errors(left);
                self.check_expr_for_ffi_errors(right);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.check_expr_for_ffi_errors(inner);
            }
            Expr::ListLiteral(elems) => {
                for elem in elems {
                    self.check_expr_for_ffi_errors(elem);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_term_function_call_simple() {
        let code = r#"
            defn get_value() -> Int [true][result == 42] {
                term 42;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        // Must desugar to fix contract order
        let mut desugarer = crate::desugarer::Desugarer::new();
        let program = desugarer.desugar(&program);

        let mut tc = TypeChecker::new();
        tc.check_program(&mut program.clone());

        let diagnostics = tc.diagnostics.clone();
        println!("Diagnostics: {:?}", diagnostics);
    }

    #[test]
    fn test_verify_term_function_call_with_param() {
        // Test with an actual function call in the term
        let code = r#"
            defn five() -> Int [true][result == 5] {
                term 5;
            };
            
            defn double(x: Int) -> Int [true][result == x * 2] {
                term five() * 2;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        // Must desugar to fix contract order
        let mut desugarer = crate::desugarer::Desugarer::new();
        let program = desugarer.desugar(&program);

        let mut tc = TypeChecker::new();
        tc.check_program(&mut program.clone());

        let diagnostics = tc.diagnostics.clone();
        println!("Diagnostics: {:?}", diagnostics);

        // We should see the verification attempt
        let has_verification = diagnostics
            .iter()
            .any(|d| d.code == "V101" || d.code == "V102");

        assert!(
            has_verification,
            "Expected verification diagnostic, got: {:?}",
            diagnostics
        );
    }
}
