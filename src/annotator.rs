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
use crate::features::literal::LiteralExpr;
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
            Statement::Term { values: outputs, swan_song, .. } => {
                for out in outputs {
                    if let Some(expr) = out {
                        self.collect_calls_from_expr(expr, calls);
                    }
                }
                if let Some(swan) = swan_song {
                    self.collect_calls_from_body(&[swan.as_ref().clone()], calls);
                }
            }
            Statement::TermBang { values: outputs, swan_song, .. } => {
                for out in outputs {
                    if let Some(expr) = out {
                        self.collect_calls_from_expr(expr, calls);
                    }
                }
                if let Some(swan) = swan_song {
                    self.collect_calls_from_body(&[swan.as_ref().clone()], calls);
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
            Expr::Projection { source: list, .. } => self.collect_calls_from_expr(list, calls),
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
            Type::Tuple(types) => format!("({})", types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(", ")),
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
            Type::Enum(name) => name.clone(),
            Type::UInt => "UInt".to_string(),
            Type::Char => "Char".to_string(),
            // Note: HashMap, HashSet, StringBuilder, Stack, Queue, Option
            // are regular structs/enums defined in stdlib, handled via
            // Custom/Applied/Enum variants below.
            Type::Vector(inner, dims) => {
                let dims_str: Vec<String> = dims.iter().map(|d| match d {
                    crate::ast::Dimension::Anonymous(s) => format!("{}", s),
                    crate::ast::Dimension::Named(n, s) => format!("{}:{}", n, s),
                }).collect();
                format!("Vector<{}, {}>", self.type_to_string(inner), dims_str.join(", "))
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
            .input_types()
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
            Statement::Assignment { lhs, expr, timeout, .. } => {
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
            Statement::Term { values: outputs, swan_song, .. } => {
                let outputs_str: Vec<String> = outputs
                    .iter()
                    .map(|o| o.as_ref().map(|e| self.format_expr(e)).unwrap_or_default())
                    .collect();
                let swan_str = swan_song.as_ref().map(|s| format!(" -> {}", self.format_statement(s, 0).trim())).unwrap_or_default();
                format!("{}term {}{};\n", spaces, outputs_str.join(", "), swan_str)
            }
            Statement::TermBang { values: outputs, swan_song, .. } => {
                let outputs_str: Vec<String> = outputs
                    .iter()
                    .map(|o| o.as_ref().map(|e| self.format_expr(e)).unwrap_or_default())
                    .collect();
                let swan_str = swan_song.as_ref().map(|s| format!(" -> {}", self.format_statement(s, 0).trim())).unwrap_or_default();
                format!("{}term! {}{};\n", spaces, outputs_str.join(", "), swan_str)
            }
            Statement::Escape(expr) => {
                let val = expr
                    .as_ref()
                    .map(|e| format!(" {}", self.format_expr(e)))
                    .unwrap_or_default();
                format!("{}escape{};\n", spaces, val)
            }
            Statement::LocalTrigger { name, ty, expr, .. } => {
                let ty_str = self.type_to_string(ty);
                let expr_str = expr
                    .as_ref()
                    .map(|e| format!(" = {}", self.format_expr(e)))
                    .unwrap_or_default();
                format!("{}trg! {}: {}{};\n", spaces, name, ty_str, expr_str)
            }
            _ => String::new(),
        }
    }

    fn format_expr(&self, expr: &Expr) -> String {
        match expr {
            // Pattern B: destructure feature struct
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Integer(n) => n.to_string(),
                LiteralExpr::Float(f) => f.to_string(),
                LiteralExpr::String(s) => format!("\"{}\"", s),
                LiteralExpr::Char(c) => format!("'{}'", c),
                LiteralExpr::Bool(b) => b.to_string(),
                LiteralExpr::Term => "term".to_string(),
            },
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Char(c) => format!("'{}'", c),  // NEW
            Expr::Bool(true) => "true".to_string(),
            Expr::Bool(false) => "false".to_string(),
            Expr::Term => "term".to_string(),
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
            Expr::Mod(l, r) => format!("({} % {})", self.format_expr(l), self.format_expr(r)),
            Expr::Eq(l, r) => format!("({} == {})", self.format_expr(l), self.format_expr(r)),
            Expr::Ne(l, r) => format!("({} != {})", self.format_expr(l), self.format_expr(r)),
            Expr::Lt(l, r) => format!("({} < {})", self.format_expr(l), self.format_expr(r)),
            Expr::Le(l, r) => format!("({} <= {})", self.format_expr(l), self.format_expr(r)),
            Expr::Gt(l, r) => format!("({} > {})", self.format_expr(l), self.format_expr(r)),
            Expr::Ge(l, r) => format!("({} >= {})", self.format_expr(l), self.format_expr(r)),
            Expr::Concat(l, r) => format!("({} ++ {})", self.format_expr(l), self.format_expr(r)),
            Expr::Cast(expr, ty) => format!("({} as {})", self.format_expr(expr), self.type_to_string(ty)),
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
            Expr::Projection { source, target } => {
                format!("{} :> {:?}", self.format_expr(source), target)
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
                    fields.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")
                )
            }
            Expr::Slice {
                value,
                start,
                end,
                stride: _,
                mask: _,
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
            Expr::Block(stmts, expr) => {
                let stmts_str = stmts.iter().map(|s| format!("  {:?}", s)).collect::<Vec<_>>().join(";\n");
                format!("{{\n{}\n  {}\n}}", stmts_str, self.format_expr(expr))
            }
            Expr::TupleDestructure(names, expr) => {
                format!("({}) = {}", names.join(", "), self.format_expr(expr))
            }
            Expr::Tuple(exprs) => {
                let exprs_str = exprs.iter().map(|e| self.format_expr(e)).collect::<Vec<_>>().join(", ");
                format!("({})", exprs_str)
            }
            Expr::MultiSlice { value, ops } => {
                let ops_str: Vec<String> = ops.iter().map(|op| match op {
                    BracketOp::Coord(c) => self.format_slice_coordinate(c),
                    BracketOp::Mask(m) => format!("; {}", self.format_expr(m)),
                    BracketOp::Stride(s) => format!("::{}", self.format_expr(s)),
                }).collect();
                format!("{}[{}]", self.format_expr(value), ops_str.join(", "))
            }
            Expr::Match { value, arms } => {
                let arms_str = arms.iter().map(|arm| {
                    let pat = match &arm.pattern {
                        MatchPattern::Wildcard => "_".to_string(),
                        MatchPattern::Variant { name, fields } => {
                            if fields.is_empty() { name.clone() }
                            else { format!("{}({})", name, fields.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")) }
                        }
                    };
                    let guard_str = arm.guard.as_ref()
                        .map(|g| format!(" if {}", self.format_expr(g)))
                        .unwrap_or_default();
                    format!("{}{}=> {}", pat, guard_str, self.format_expr(&arm.body))
                }).collect::<Vec<_>>().join(", ");
                format!("match {} {{ {} }}", self.format_expr(value), arms_str)
            }
            Expr::ArrowMut { target, index, value, .. } => {
                let t = self.format_expr(target);
                let i = self.format_expr(index);
                let vs = value.as_ref().map(|v| self.format_expr(v)).unwrap_or_default();
                let idx_str = if matches!(index.as_ref(), Expr::Term) { String::new() } else { format!("[{}]", i) };
                format!("{}{} <- {}", t, idx_str, vs)
            }
            Expr::ArrowDiscard { target, index } => {
                let t = self.format_expr(target);
                let i = self.format_expr(index);
                let idx_str = if matches!(index.as_ref(), Expr::Term) { String::new() } else { format!("[{}]", i) };
                format!("<- {}{}", t, idx_str)
            }
            Expr::ArrowTransfer { dest, source, filter } => {
                let d = self.format_expr(dest);
                let s = self.format_expr(source);
                if let Some(f) = filter {
                    format!("{} <- {}[; {}]", d, s, self.format_expr(f))
                } else {
                    format!("{} <- {}", d, s)
                }
            }
            Expr::SigCall { modifier, expr } => {
                let tag = match modifier {
                    crate::ast::SigModifier::Out => "#out",
                    crate::ast::SigModifier::Inline => "#inline",
                };
                format!("sig {} {}", tag, self.format_expr(expr))
            }
            Expr::Ellipsis => "...".to_string(),
            Expr::MapLiteral(entries) => {
                let pairs: Vec<String> = entries.iter()
                    .map(|(k, v)| format!("{}: {}", self.format_expr(k), self.format_expr(v)))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            Expr::SetLiteral(entries) => {
                let elems: Vec<String> = entries.iter().map(|e| self.format_expr(e)).collect();
                format!("{{{}}}", elems.join(", "))
            }
            Expr::DbvlTable { path, key_offsets, .. } => {
                format!("DbvlTable({}, {} entries)", path, key_offsets.len())
            }
            Expr::SubtypeProjection { source, .. } => {
                format!("<: {}", self.format_expr(source))
            }
            _ => String::new(),
        }
    }

    fn format_slice_coordinate(&self, coord: &crate::ast::SliceCoordinate) -> String {
        match coord {
            crate::ast::SliceCoordinate::Index(expr) => self.format_expr(expr),
            crate::ast::SliceCoordinate::Range { start, end } => {
                let start_str = start.as_ref().map(|s| self.format_expr(s)).unwrap_or_default();
                let end_str = end.as_ref().map(|e| self.format_expr(e)).unwrap_or_default();
                format!("{}..{}", start_str, end_str)
            }
            crate::ast::SliceCoordinate::Named { name, coord } => {
                format!("{}:{}", name, self.format_slice_coordinate(coord))
            }
            crate::ast::SliceCoordinate::AtDimension { dimension, coord } => {
                format!("@{}:{}", dimension, self.format_slice_coordinate(coord))
            }
            crate::ast::SliceCoordinate::Ellipsis => "...".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
use crate::ast::*;
    use super::*;

    fn make_defn(name: &str, body: Vec<Statement>) -> TopLevel {
        TopLevel::Definition(Definition {
            name: name.to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body,
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        })
    }

    fn make_call_expr(name: &str) -> Expr {
        Expr::Call(name.to_string(), vec![])
    }

    #[test]
    fn test_analyze_empty_program() {
        let mut ann = Annotator::new();
        let prog = Program { items: vec![], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None };
        ann.analyze(&prog);
        assert!(ann.call_paths.is_empty());
    }

    #[test]
    fn test_analyze_definition_no_calls() {
        let mut ann = Annotator::new();
        let stmts = vec![Statement::Term { values: vec![Some(Expr::Integer(42))], swan_song: None, modifiers: vec![] }];
        let prog = Program { items: vec![make_defn("foo", stmts)], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None };
        ann.analyze(&prog);
        assert_eq!(ann.call_paths.get("foo").map(|v| v.len()).unwrap_or(0), 0);
    }

    #[test]
    fn test_analyze_definition_with_call() {
        let mut ann = Annotator::new();
        let stmts = vec![Statement::Expression(make_call_expr("bar"))];
        let prog = Program { items: vec![make_defn("foo", stmts)], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None };
        ann.analyze(&prog);
        let calls = ann.call_paths.get("foo").unwrap();
        assert_eq!(calls, &vec!["bar".to_string()]);
    }

    #[test]
    fn test_analyze_nested_call() {
        let mut ann = Annotator::new();
        let inner = make_call_expr("inner");
        let outer = Expr::Call("outer".to_string(), vec![inner]);
        let stmts = vec![Statement::Expression(outer)];
        let prog = Program { items: vec![make_defn("foo", stmts)], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None };
        ann.analyze(&prog);
        let calls = ann.call_paths.get("foo").unwrap();
        assert_eq!(calls, &vec!["outer".to_string(), "inner".to_string()]);
    }

    #[test]
    fn test_analyze_guarded_call() {
        let mut ann = Annotator::new();
        let guarded = Statement::Guarded {
            condition: Expr::Bool(true),
            statements: vec![Statement::Expression(make_call_expr("inside_guard"))],
        };
        let prog = Program { items: vec![make_defn("foo", vec![guarded])], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None };
        ann.analyze(&prog);
        let calls = ann.call_paths.get("foo").unwrap();
        assert_eq!(calls, &vec!["inside_guard".to_string()]);
    }

    #[test]
    fn test_analyze_guarded_assignment_call() {
        let mut ann = Annotator::new();
        let guarded = Statement::Guarded {
            condition: Expr::Bool(true),
            statements: vec![Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: make_call_expr("calc"),
                timeout: None,
                modifiers: vec![],
            }],
        };
        let prog = Program { items: vec![make_defn("foo", vec![guarded])], comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None };
        ann.analyze(&prog);
        let calls = ann.call_paths.get("foo").unwrap();
        assert_eq!(calls, &vec!["calc".to_string()]);
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    use crate::features::literal::LiteralExpr;

    #[kani::proof]
    fn verify_annotator_format_expr_literal_integer() {
        let ann = Annotator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = ann.format_expr(&expr);
        assert_eq!(result, "42");
    }

    #[kani::proof]
    fn verify_annotator_format_expr_literal_bool() {
        let ann = Annotator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = ann.format_expr(&expr);
        assert_eq!(result, "true");
    }

    #[kani::proof]
    fn verify_annotator_format_expr_literal_float() {
        let ann = Annotator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        let result = ann.format_expr(&expr);
        assert!(!result.is_empty());
    }

    #[kani::proof]
    fn verify_annotator_format_expr_literal_string() {
        let ann = Annotator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::String("hello".to_string())));
        let result = ann.format_expr(&expr);
        assert_eq!(result, "\"hello\"");
    }

    #[kani::proof]
    fn verify_annotator_format_expr_literal_char() {
        let ann = Annotator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
        let result = ann.format_expr(&expr);
        assert_eq!(result, "'A'");
    }

    #[kani::proof]
    fn verify_annotator_format_expr_literal_term() {
        let ann = Annotator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Term));
        let result = ann.format_expr(&expr);
        assert_eq!(result, "term");
    }
}
