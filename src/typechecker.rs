use crate::ast::*;
use crate::errors::{Diagnostic, Severity, Span};
use crate::ffi;
use std::collections::HashMap;
use std::path::PathBuf;

pub use crate::errors::TypeError;

pub struct TypeChecker {
    scopes: Vec<HashMap<String, Type>>,
    errors: Vec<crate::errors::TypeError>,
    diagnostics: Vec<Diagnostic>,
    source: String,
    signatures: HashMap<String, Signature>,
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            scopes: vec![HashMap::new()],
            errors: Vec::new(),
            diagnostics: Vec::new(),
            source: String::new(),
            signatures: HashMap::new(),
        }
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = source;
        self
    }

    pub fn check_program(&mut self, program: &Program) -> Vec<TypeError> {
        // First pass: collect signatures
        for item in &program.items {
            if let TopLevel::Signature(sig) = item {
                let key = sig.alias.clone().unwrap_or_else(|| sig.name.clone());
                self.signatures.insert(key, sig.clone());
            }
        }

        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.declare_variable(&decl.name, decl.ty.clone());
                    if decl.expr.is_none() {
                        self.diagnostics.push(
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
                TopLevel::Constant(const_decl) => {
                    self.declare_variable(&const_decl.name, const_decl.ty.clone());
                    let expr_ty = self.infer_expression(&const_decl.expr);
                    if !self.types_compatible(&expr_ty, &const_decl.ty) {
                        let diag = Diagnostic::new("B001", Severity::Error, "type mismatch")
                            .with_explanation(&format!(
                                "expected {} for constant '{}', but found {}",
                                self.type_to_string(&const_decl.ty),
                                const_decl.name,
                                self.type_to_string(&expr_ty)
                            ))
                            .with_hint("ensure the expression type matches the declared type");

                        self.diagnostics.push(diag);
                        self.errors.push(TypeError::TypeMismatch {
                            expected: self.type_to_string(&const_decl.ty),
                            found: self.type_to_string(&expr_ty),
                            context: format!("const {}", const_decl.name),
                        });
                    }
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
                TopLevel::Import(_) => {}
                TopLevel::ForeignSig(_) => {}
                TopLevel::ForeignBinding {
                    name,
                    toml_path,
                    signature,
                    ..
                } => {
                    self.check_frgn_binding(name, toml_path, signature);
                }
                TopLevel::Struct(_) => {}
                TopLevel::RStruct(_) => {}
                TopLevel::RenderBlock(_) => {}
            }
        }
        self.errors.clone()
    }

    pub fn get_diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn format_diagnostics(&self) -> String {
        let mut output = String::new();
        for diag in &self.diagnostics {
            output.push_str(&diag.format(&self.source, "main.bv"));
            output.push('\n');
        }
        output
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
                    self.errors.push(TypeError::TypeMismatch {
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
                                self.errors.push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(expected_ty),
                                    found: self.type_to_string(&actual_ty),
                                    context: format!("term output {}", i),
                                });
                            }
                        }
                    }
                }
            }
            _ => self.check_statement(stmt, is_async),
        }
    }

    fn check_transaction(&mut self, txn: &Transaction) {
        self.push_scope();

        for stmt in &txn.body {
            self.check_statement(stmt, Some(&txn.is_async));
        }

        self.pop_scope();
    }

    fn check_frgn_binding(&mut self, name: &str, toml_path: &str, signature: &ForeignSignature) {
        // Resolve the TOML path using the FFI resolver
        let resolved_path = match ffi::resolver::resolve_binding_path(toml_path, &None) {
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
                self.diagnostics.push(diag);
                self.errors.push(TypeError::FFIError {
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
                self.diagnostics.push(diag);
                self.errors.push(TypeError::FFIError {
                    message: format!("Failed to load binding file for '{}': {}", name, err),
                });
                return;
            }
        };

        // Find the matching binding for this frgn
        let matching_binding = bindings.iter().find(|b| b.name == name);

        let binding = match matching_binding {
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
                self.diagnostics.push(diag);
                self.errors.push(TypeError::FFIError {
                    message: format!("Binding '{}' not found in '{}'", name, toml_path),
                });
                return;
            }
        };

        // Create a mutable copy of the signature to populate error_fields from the binding
        let mut sig = signature.clone();
        sig.error_fields = binding.error_fields.clone();

        // Validate the frgn signature against the TOML binding
        if let Err(err) = ffi::validator::validate_frgn_against_binding(&sig, binding) {
            let diag = Diagnostic::new("F004", Severity::Error, "FFI binding validation failed")
                .with_explanation(&format!(
                    "The frgn declaration for '{}' does not match its TOML binding: {}",
                    name, err
                ))
                .with_hint("Ensure the frgn signature matches the binding definition");
            self.diagnostics.push(diag);
            self.errors.push(TypeError::FFIError {
                message: format!("Binding validation failed for '{}': {}", name, err),
            });
            return;
        }

        // Validate that all FFI types are supported
        for (_, ty) in &sig.inputs {
            if !ffi::validator::is_valid_ffi_type(ty) {
                let diag = Diagnostic::new("F005", Severity::Error, "Invalid FFI type")
                    .with_explanation(&format!(
                        "Input parameter in '{}' uses unsupported type: {:?}",
                        name, ty
                    ))
                    .with_hint(
                        "FFI supports: String, Int, Float, Bool, Void, Data, and custom structs",
                    );
                self.diagnostics.push(diag);
                self.errors.push(TypeError::FFIError {
                    message: format!("Invalid FFI type in input for '{}'", name),
                });
                return;
            }
        }

        for (_, ty) in &sig.success_output {
            if !ffi::validator::is_valid_ffi_type(ty) {
                let diag = Diagnostic::new("F005", Severity::Error, "Invalid FFI type")
                    .with_explanation(&format!(
                        "Output parameter in '{}' uses unsupported type: {:?}",
                        name, ty
                    ))
                    .with_hint(
                        "FFI supports: String, Int, Float, Bool, Void, Data, and custom structs",
                    );
                self.diagnostics.push(diag);
                self.errors.push(TypeError::FFIError {
                    message: format!("Invalid FFI type in output for '{}'", name),
                });
                return;
            }
        }

        for (_, ty) in &sig.error_fields {
            if !ffi::validator::is_valid_ffi_type(ty) {
                let diag = Diagnostic::new("F005", Severity::Error, "Invalid FFI type")
                    .with_explanation(&format!(
                        "Error field in '{}' uses unsupported type: {:?}",
                        name, ty
                    ))
                    .with_hint(
                        "FFI supports: String, Int, Float, Bool, Void, Data, and custom structs",
                    );
                self.diagnostics.push(diag);
                self.errors.push(TypeError::FFIError {
                    message: format!("Invalid FFI type in error for '{}'", name),
                });
                return;
            }
        }
    }

    fn check_statement(&mut self, stmt: &Statement, is_async: Option<&bool>) {
        match stmt {
            Statement::Assignment {
                is_owned,
                name,
                expr,
            } => {
                let expr_ty = self.infer_expression(expr);
                if let Some(var_ty) = self.lookup_variable(name) {
                    if !self.types_compatible(&expr_ty, &var_ty) {
                        self.errors.push(TypeError::TypeMismatch {
                            expected: self.type_to_string(&var_ty),
                            found: self.type_to_string(&expr_ty),
                            context: format!("assignment to {}", name),
                        });
                    }

                    if *is_owned {
                        let has_lower_scope = self
                            .scopes
                            .iter()
                            .take(self.scopes.len() - 1)
                            .any(|s| s.contains_key(name));
                        if !has_lower_scope {
                            self.errors.push(TypeError::OwnershipViolation {
                                var: name.clone(),
                                reason: "owned reference requires variable to exist in outer scope"
                                    .to_string(),
                            });
                        }
                    }
                } else {
                    self.declare_variable(name, expr_ty);
                }
                
                if self.is_ffi_call(expr) {
                    self.diagnostics.push(
                        Diagnostic::new("F101", Severity::Warning, "FFI call result not handled")
                            .with_explanation(&format!(
                                "FFI function result assigned to '{}' without checking for errors. \
                                 Use .is_ok() or .is_err() to handle potential errors.",
                                name
                            ))
                            .with_hint("Wrap the FFI call with is_ok()/is_err() guards")
                    );
                }
            }
            Statement::Let { name, ty, expr } => {
                let inferred_ty = expr.as_ref().map(|e| self.infer_expression(e));
                let final_ty = ty.clone().or(inferred_ty);

                if let Some(ty) = final_ty {
                    if let Some(e) = expr {
                        let expr_ty = self.infer_expression(e);
                        if !self.types_compatible(&expr_ty, &ty) {
                            self.errors.push(TypeError::TypeMismatch {
                                expected: self.type_to_string(&ty),
                                found: self.type_to_string(&expr_ty),
                                context: format!("let {}", name),
                            });
                        }
                    }
                    self.declare_variable(name, ty);
                }
            }
            Statement::Expression(expr) => {
                self.infer_expression(expr);
            }
            Statement::Term(outputs) => {
                for expr_opt in outputs {
                    if let Some(expr) = expr_opt {
                        self.infer_expression(expr);
                    }
                }
            }
            Statement::Escape(expr_opt) => {
                if let Some(expr) = expr_opt {
                    self.infer_expression(expr);
                }
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let cond_ty = self.infer_expression(condition);
                if !self.types_compatible(&cond_ty, &Type::Bool) {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: self.type_to_string(&cond_ty),
                        context: "guard condition".to_string(),
                    });
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
                self.infer_expression(expr);
                self.declare_variable(name, Type::Custom(pattern.clone()));
            }
        }
    }

    fn infer_expression(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Integer(_) => Type::Int,
            Expr::Float(_) => Type::Float,
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
            Expr::Eq(_, _)
            | Expr::Ne(_, _)
            | Expr::Lt(_, _)
            | Expr::Le(_, _)
            | Expr::Gt(_, _)
            | Expr::Ge(_, _)
            | Expr::Or(_, _)
            | Expr::And(_, _) => Type::Bool,
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => self.infer_expression(e),
            Expr::Call(name, _args) => Type::Custom(name.clone()),
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
                if let Type::Applied(_, type_args) = list_type {
                    if !type_args.is_empty() {
                        type_args[0].clone()
                    } else {
                        Type::TypeVar("T".to_string())
                    }
                } else {
                    Type::TypeVar("T".to_string())
                }
            }
            Expr::ListLen(_) => Type::Int,
            Expr::FieldAccess(_, _) => Type::Custom("unknown".to_string()),
        }
    }

    fn binary_op_type(&self, l: &Expr, r: &Expr, int_type: Type, float_type: Type) -> Type {
        let l_ty = self.infer_expression(l);
        let r_ty = self.infer_expression(r);
        match (&l_ty, &r_ty) {
            (Type::Int, Type::Int) => int_type,
            (Type::Float, Type::Float) => float_type,
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => float_type,
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
            (Type::Float, Type::Float) => true,
            (Type::String, Type::String) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Data, Type::Data) => true,
            (Type::Void, Type::Void) => true,
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
            (Type::Applied(a, _), Type::Applied(b, _))
            | (Type::Applied(b, _), Type::Applied(a, _))
                if a == b =>
            {
                true
            }
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
}
