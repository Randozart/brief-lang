use crate::ast::*;
use crate::errors::{Diagnostic, Severity, Span};
use std::collections::HashMap;

pub use crate::errors::TypeError;

pub struct TypeChecker {
    scopes: Vec<HashMap<String, Type>>,
    errors: Vec<crate::errors::TypeError>,
    diagnostics: Vec<Diagnostic>,
    source: String,
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            scopes: vec![HashMap::new()],
            errors: Vec::new(),
            diagnostics: Vec::new(),
            source: String::new(),
        }
    }
    
    pub fn with_source(mut self, source: String) -> Self {
        self.source = source;
        self
    }
    
    pub fn check_program(&mut self, program: &Program) -> Vec<TypeError> {
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
                                .with_note("uninitialized signals may contain garbage values at runtime")
                        );
                    }
                }
                TopLevel::Constant(const_decl) => {
                    self.declare_variable(&const_decl.name, const_decl.ty.clone());
                    let expr_ty = self.infer_expression(&const_decl.expr);
                    if !self.types_compatible(&expr_ty, &const_decl.ty) {
                        let diag = Diagnostic::new(
                            "B001", 
                            Severity::Error, 
                            "type mismatch"
                        )
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
            self.declare_variable(param_name, param_ty.clone());
        }

        for stmt in &defn.body {
            self.check_statement(stmt, None);
        }

        self.pop_scope();
    }

    fn check_transaction(&mut self, txn: &Transaction) {
        self.push_scope();

        for stmt in &txn.body {
            self.check_statement(stmt, Some(&txn.is_async));
        }

        self.pop_scope();
    }

    fn check_statement(&mut self, stmt: &Statement, is_async: Option<&bool>) {
        match stmt {
            Statement::Assignment { is_owned, name, expr } => {
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
                        let has_lower_scope = self.scopes.iter().take(self.scopes.len() - 1).any(|s| s.contains_key(name));
                        if !has_lower_scope {
                            self.errors.push(TypeError::OwnershipViolation {
                                var: name.clone(),
                                reason: "owned reference requires variable to exist in outer scope".to_string(),
                            });
                        }
                    }
                } else {
                    self.declare_variable(name, expr_ty);
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
            Statement::Guarded { condition, statement } => {
                let cond_ty = self.infer_expression(condition);
                if !self.types_compatible(&cond_ty, &Type::Bool) {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: self.type_to_string(&cond_ty),
                        context: "guard condition".to_string(),
                    });
                }
                self.check_statement(statement, is_async);
            }
            Statement::Unification { name, pattern, expr } => {
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
            Expr::Identifier(name) => {
                self.lookup_variable(name).unwrap_or(Type::Custom(name.clone()))
            }
            Expr::OwnedRef(name) => {
                self.lookup_variable(name).unwrap_or(Type::Custom(name.clone()))
            }
            Expr::PriorState(name) => {
                self.lookup_variable(name).unwrap_or(Type::Custom(name.clone()))
            }
            Expr::Add(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Sub(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Mul(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Div(l, r) => self.binary_op_type(l, r, Type::Int, Type::Float),
            Expr::Eq(_, _) | Expr::Ne(_, _) | Expr::Lt(_, _) | Expr::Le(_, _)
            | Expr::Gt(_, _) | Expr::Ge(_, _) | Expr::Or(_, _) | Expr::And(_, _) => Type::Bool,
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => self.infer_expression(e),
            Expr::Call(name, _args) => Type::Custom(name.clone()),
        }
    }

    fn binary_op_type(&self, l: &Expr, r: &Expr, int_type: Type, float_type: Type) -> Type {
        let l_ty = self.infer_expression(l);
        let r_ty = self.infer_expression(r);
        match (&l_ty, &r_ty) {
            (Type::Int, Type::Int) => int_type,
            (Type::Float, Type::Float) => float_type,
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => float_type,
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
            (Type::Union(types), t) | (t, Type::Union(types)) => {
                types.iter().any(|u| self.types_compatible(u, t))
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
            Type::Union(types) => {
                types.iter().map(|t| self.type_to_string(t)).collect::<Vec<_>>().join(" | ")
            }
            Type::ContractBound(inner, guard) => {
                format!("{}[{:?}]", self.type_to_string(inner), guard)
            }
        }
    }
}
