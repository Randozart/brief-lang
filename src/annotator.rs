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
use std::collections::{HashMap, HashSet};

pub struct Annotator {
    pub call_paths: HashMap<String, Vec<String>>,
}

impl Annotator {
    pub fn new() -> Self {
        Annotator {
            call_paths: HashMap::new(),
        }
    }

    pub fn analyze(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                let mut calls = Vec::new();
                self.collect_calls_from_body(&defn.body, &mut calls);
                self.call_paths.insert(defn.name.clone(), calls);
            }
        }
    }

    fn collect_calls_from_body(&self, body: &[Statement], calls: &mut Vec<String>) {
        for stmt in body {
            match stmt {
                Statement::Expression(expr) => self.collect_calls_from_expr(expr, calls),
                Statement::Assignment { expr, lhs, .. } => {
                    self.collect_calls_from_expr(expr, calls);
                    self.collect_calls_from_expr(lhs, calls);
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    self.collect_calls_from_expr(condition, calls);
                    self.collect_calls_from_body(statements, calls);
                }
                Statement::Term(outputs) => {
                    for out in outputs {
                        if let Some(expr) = out {
                            self.collect_calls_from_expr(expr, calls);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_calls_from_expr(&self, expr: &Expr, calls: &mut Vec<String>) {
        match expr {
            Expr::Call(name, args) => {
                calls.push(name.clone());
                for arg in args {
                    self.collect_calls_from_expr(arg, calls);
                }
            }
            Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Or(l, r)
            | Expr::And(l, r)
            | Expr::BitAnd(l, r)
            | Expr::BitOr(l, r)
            | Expr::BitXor(l, r) => {
                self.collect_calls_from_expr(l, calls);
                self.collect_calls_from_expr(r, calls);
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => self.collect_calls_from_expr(e, calls),
            Expr::ListLiteral(elems) => {
                for e in elems {
                    self.collect_calls_from_expr(e, calls);
                }
            }
            Expr::ListIndex(list, index) => {
                self.collect_calls_from_expr(list, calls);
                self.collect_calls_from_expr(index, calls);
            }
            Expr::ListLen(list) => self.collect_calls_from_expr(list, calls),
            Expr::FieldAccess(obj, _) => self.collect_calls_from_expr(obj, calls),
            Expr::StructInstance(_, fields) => {
                for (_, v) in fields {
                    self.collect_calls_from_expr(v, calls);
                }
            }
            Expr::ObjectLiteral(fields) => {
                for (_, v) in fields {
                    self.collect_calls_from_expr(v, calls);
                }
            }
            _ => {}
        }
    }

    pub fn annotate_program(&self, program: &Program) -> String {
        let mut output = String::new();
        for item in &program.items {
            match item {
                TopLevel::Definition(defn) => output.push_str(&self.format_definition(defn)),
                TopLevel::Transaction(txn) => output.push_str(&self.format_transaction(txn)),
                TopLevel::Signature(sig) => output.push_str(&self.format_signature(sig)),
                TopLevel::StateDecl(decl) => output.push_str(&self.format_state_decl(decl)),
                _ => {}
            }
        }
        output
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
            Type::ContractBound(inner, _) => self.type_to_string(inner),
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
            Type::Option(inner) => format!("Option<{}>", self.type_to_string(inner)),
            Type::Enum(name) => name.clone(),
            Type::UInt => "UInt".to_string(),
            Type::Vector(inner, size) => {
                format!("Vector<{}>[{}]", self.type_to_string(inner), size)
            }
            Type::Constrained(inner, _) => self.type_to_string(inner),
        }
    }

    fn format_definition(&self, defn: &Definition) -> String {
        let params: Vec<String> = defn
            .parameters
            .iter()
            .map(|(n, t)| format!("{}: {}", n, self.type_to_string(t)))
            .collect();

        let params_str = if params.is_empty() {
            "()".to_string()
        } else {
            format!("({})", params.join(", "))
        };
        let outputs_str = if defn.outputs.is_empty() {
            String::new()
        } else {
            let outputs: Vec<String> = defn
                .outputs
                .iter()
                .map(|t| self.type_to_string(t))
                .collect();
            format!(": {}", outputs.join(", "))
        };

        let pre = self.format_expr(&defn.contract.pre_condition);
        let post = self.format_expr(&defn.contract.post_condition);

        let body = self.format_body(&defn.body);

        format!(
            "defn {}{}{} [{}][{}] {{\n{}}};\n",
            defn.name, params_str, outputs_str, pre, post, body
        )
    }

    fn format_transaction(&self, txn: &Transaction) -> String {
        let modifier = if txn.is_async { "async " } else { "" };
        let rct = if txn.is_reactive { "rct " } else { "" };

        let pre = self.format_expr(&txn.contract.pre_condition);
        let post = self.format_expr(&txn.contract.post_condition);

        let body = self.format_body(&txn.body);

        format!(
            "{}txn {}{} [{}][{}] {{\n{}}};\n",
            rct, modifier, txn.name, pre, post, body
        )
    }

    fn format_signature(&self, sig: &Signature) -> String {
        let inputs: Vec<String> = sig
            .input_types
            .iter()
            .map(|t| self.type_to_string(t))
            .collect();
        let results: Vec<String> = match &sig.result_type {
            ResultType::Projection(types) => types.iter().map(|t| self.type_to_string(t)).collect(),
            ResultType::TrueAssertion => vec!["true".to_string()],
            ResultType::VoidType => vec!["void".to_string()],
        };

        format!(
            "sig {}: ({}) -> ({});\n",
            sig.name,
            inputs.join(", "),
            results.join(", ")
        )
    }

    fn format_state_decl(&self, decl: &StateDecl) -> String {
        let init = if let Some(e) = &decl.expr {
            format!(" = {}", self.format_expr(e))
        } else {
            String::new()
        };
        let addr = if let Some(a) = decl.address {
            format!(" @ 0x{:x}", a)
        } else {
            String::new()
        };
        format!(
            "let {}: {}{}{};\n",
            decl.name,
            self.type_to_string(&decl.ty),
            addr,
            init
        )
    }

    fn format_body(&self, body: &[Statement]) -> String {
        let mut output = String::new();
        for stmt in body {
            output.push_str(&self.format_statement(stmt, 2));
        }
        output
    }

    fn format_statement(&self, stmt: &Statement, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        match stmt {
            Statement::Expression(expr) => format!("{}{};\n", spaces, self.format_expr(expr)),
            Statement::Assignment { lhs, expr, timeout } => {
                let timeout_str = if let Some((expr, unit)) = timeout {
                    let unit_str = match unit {
                        TimeUnit::Cycles => "cycles",
                        TimeUnit::Ms => "ms",
                        TimeUnit::Seconds => "s",
                        TimeUnit::Minutes => "min",
                    };
                    format!(" within {} {}", self.format_expr(expr), unit_str)
                } else {
                    String::new()
                };
                format!(
                    "{}{} = {}{};\n",
                    spaces,
                    self.format_expr(lhs),
                    self.format_expr(expr),
                    timeout_str
                )
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let mut output = format!("{}[{}] {{\n", spaces, self.format_expr(condition));
                for s in statements {
                    output.push_str(&self.format_statement(s, indent + 2));
                }
                output.push_str(&format!("{}}}\n", spaces));
                output
            }
            Statement::Term(outputs) => {
                let outputs_str: Vec<String> = outputs
                    .iter()
                    .map(|o| o.as_ref().map(|e| self.format_expr(e)).unwrap_or_default())
                    .collect();
                format!("{}term {};\n", spaces, outputs_str.join(", "))
            }
            Statement::Escape(expr) => {
                let val = expr
                    .as_ref()
                    .map(|e| format!(" {}", self.format_expr(e)))
                    .unwrap_or_default();
                format!("{}escape{};\n", spaces, val)
            }
            _ => String::new(),
        }
    }

    fn format_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Bool(true) => "true".to_string(),
            Expr::Bool(false) => "false".to_string(),
            Expr::Identifier(n) => n.clone(),
            Expr::OwnedRef(n) => format!("&{}", n),
            Expr::PriorState(n) => format!("@{}", n),
            Expr::Call(name, args) => {
                let args_str = args
                    .iter()
                    .map(|a| self.format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
            Expr::Add(l, r) => format!("({} + {})", self.format_expr(l), self.format_expr(r)),
            Expr::Sub(l, r) => format!("({} - {})", self.format_expr(l), self.format_expr(r)),
            Expr::Mul(l, r) => format!("({} * {})", self.format_expr(l), self.format_expr(r)),
            Expr::Div(l, r) => format!("({} / {})", self.format_expr(l), self.format_expr(r)),
            Expr::Eq(l, r) => format!("({} == {})", self.format_expr(l), self.format_expr(r)),
            Expr::Ne(l, r) => format!("({} != {})", self.format_expr(l), self.format_expr(r)),
            Expr::Lt(l, r) => format!("({} < {})", self.format_expr(l), self.format_expr(r)),
            Expr::Le(l, r) => format!("({} <= {})", self.format_expr(l), self.format_expr(r)),
            Expr::Gt(l, r) => format!("({} > {})", self.format_expr(l), self.format_expr(r)),
            Expr::Ge(l, r) => format!("({} >= {})", self.format_expr(l), self.format_expr(r)),
            Expr::Or(l, r) => format!("({} || {})", self.format_expr(l), self.format_expr(r)),
            Expr::And(l, r) => format!("({} && {})", self.format_expr(l), self.format_expr(r)),
            Expr::BitAnd(l, r) => format!("({} & {})", self.format_expr(l), self.format_expr(r)),
            Expr::BitOr(l, r) => format!("({} | {})", self.format_expr(l), self.format_expr(r)),
            Expr::BitXor(l, r) => format!("({} ^ {})", self.format_expr(l), self.format_expr(r)),
            Expr::Shl(l, r) => format!("({} << {})", self.format_expr(l), self.format_expr(r)),
            Expr::Shr(l, r) => format!("({} >> {})", self.format_expr(l), self.format_expr(r)),
            Expr::Not(e) => format!("!{}", self.format_expr(e)),
            Expr::Neg(e) => format!("-{}", self.format_expr(e)),
            Expr::BitNot(e) => format!("~{}", self.format_expr(e)),
            Expr::ListLiteral(elements) => {
                let elements_str = elements
                    .iter()
                    .map(|e| self.format_expr(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", elements_str)
            }
            Expr::ListIndex(list, index) => {
                format!("{}[{}]", self.format_expr(list), self.format_expr(index))
            }
            Expr::ListLen(list) => {
                format!("{}.len()", self.format_expr(list))
            }
            Expr::FieldAccess(obj, field) => {
                format!("{}.{}", self.format_expr(obj), field)
            }
            Expr::StructInstance(typename, fields) => {
                let fields_str = fields
                    .iter()
                    .map(|(f, v)| format!("{}: {}", f, self.format_expr(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {{{}}}", typename, fields_str)
            }
            Expr::ObjectLiteral(fields) => {
                let fields_str = fields
                    .iter()
                    .map(|(n, v)| format!("{}: {}", n, self.format_expr(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", fields_str)
            }
            Expr::PatternMatch {
                value,
                variant,
                fields,
            } => {
                format!(
                    "{} {}({})",
                    self.format_expr(value),
                    variant,
                    fields.join(", ")
                )
            }
            Expr::Slice {
                value,
                start,
                end,
                stride: _,
            } => {
                format!(
                    "{}[{}:{}]",
                    self.format_expr(value),
                    start
                        .as_ref()
                        .map(|e| self.format_expr(e))
                        .unwrap_or_default(),
                    end.as_ref()
                        .map(|e| self.format_expr(e))
                        .unwrap_or_default()
                )
            }
            Expr::ForAll { var, expr } => format!("forall {} in {}", var, self.format_expr(expr)),
            Expr::Exists { var, expr } => format!("exists {} in {}", var, self.format_expr(expr)),
        }
    }
}
