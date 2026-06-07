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

use std::fmt::Write;

use crate::analysis::call_graph::CallGraph;
use crate::ast::{Attribute, Expr, OutputType, Program, Statement, TopLevel, Transaction, Type, Contract, WatchdogSpec, StrictMode, AlkaBlock};

pub struct CobolBackend {
    spec: Option<crate::target_spec::TargetSpec>,
    program_id: String,
    use_abend: bool,
    recursion_limit: u32,
    pending_cleanup: Vec<Statement>,
    has_cycles: bool,
}

impl CobolBackend {
    pub fn new() -> Self {
        Self {
            spec: None,
            program_id: String::new(),
            use_abend: false,
            recursion_limit: 1000,
            pending_cleanup: Vec::new(),
            has_cycles: false,
        }
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn with_program_id(mut self, id: String) -> Self {
        self.program_id = id;
        self
    }

    pub fn with_abend(mut self, abend: bool) -> Self {
        self.use_abend = abend;
        self
    }

    pub fn generate(&mut self, program: &Program, stem: &str) -> String {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        let _pr = &_analysis.param_ranges;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  COBOL backend: acyclic call graph — static dispatch enabled");
        }

        let program_id = if self.program_id.is_empty() {
            Self::sanitize_name(stem).to_uppercase()
        } else {
            self.program_id.clone()
        };

        let mut output = String::new();

        output.push_str(">>SOURCE FORMAT IS FREE\n");
        output.push_str("IDENTIFICATION DIVISION.\n");
        output.push_str(&format!("PROGRAM-ID. {} RECURSIVE.\n\n", program_id));

        if let Some(spec) = &self.spec {
            if let Some(cg) = &spec.codegen {
                if let Some(header) = &cg.templates.header {
                    output.push_str(header);
                    output.push_str("\n");
                }
            }
        }

        output.push_str("DATA DIVISION.\n");

        let (working_storage, linkage_vars) = self.collect_state_declarations(program);
        let params = self.extract_parameters(program);

        output.push_str("WORKING-STORAGE SECTION.\n");
        let old_state_vars = self.collect_all_old_state_vars(program);
        if !old_state_vars.is_empty() {
            for var in &old_state_vars {
                output.push_str(&format!("01  WS-OLD-{} PIC S9(18) COMP-5 VALUE 0.\n", var.to_uppercase()));
            }
            output.push_str("\n");
        }
        if self.has_watchdog(program) {
            output.push_str(&format!("01  WS-RECURSION-DEPTH PIC 9(4) COMP-5 VALUE 0.\n"));
            output.push_str(&format!("01  WS-RECURSION-MAX   PIC 9(4) COMP-5 VALUE {}.\n", self.recursion_limit));
            output.push_str("\n");
        }
        if !working_storage.is_empty() {
            output.push_str(&working_storage);
        }
        output.push_str("\n");

        if !linkage_vars.is_empty() || !params.is_empty() {
            output.push_str("LINKAGE SECTION.\n");
            if !linkage_vars.is_empty() {
                output.push_str(&linkage_vars);
            }
            if !params.is_empty() {
                output.push_str(&self.generate_linkage_params(&params));
            }
            output.push_str("\n");
        }

        output.push_str("PROCEDURE DIVISION");

        if !params.is_empty() {
            output.push_str(" USING");
            for (i, (name, _)) in params.iter().enumerate() {
                if i > 0 {
                    output.push_str(" ");
                }
                output.push_str(&format!("LS-{}", Self::sanitize_name(name).to_uppercase()));
            }
        }

        output.push_str(".\n");
        output.push_str("MAIN-LOGIC SECTION.\n");

        self.generate_transactions(program, &mut output);

        output.push_str("    GOBACK.\n\n");
        output.push_str(&format!("END PROGRAM {}.\n", program_id));

        output
    }

    fn collect_state_declarations(&self, program: &Program) -> (String, String) {
        let mut working_storage = String::new();
        let mut linkage = String::new();

        for item in &program.items {
            match item {
                TopLevel::StateDecl(state) => {
                    let name = Self::sanitize_name(&state.name);

                    if state.address.is_some() {
                        linkage.push_str(&format!(
                            "01  LS-{} {} {}.\n",
                            name.to_uppercase(),
                            self.get_cobol_type(&state.ty, &state.attrs),
                            self.get_init_value(&state.ty, &state.attrs, &state.expr)
                        ));
                    } else if state.ty == Type::Bool {
                        working_storage.push_str(&format!(
                            "01  WS-{} PIC X VALUE 'N'.\n",
                            name.to_uppercase()
                        ));
                        working_storage.push_str(&format!(
                            "    88  WS-{}-TRUE   VALUE 'Y'.\n",
                            name.to_uppercase()
                        ));
                        working_storage.push_str(&format!(
                            "    88  WS-{}-FALSE  VALUE 'N'.\n",
                            name.to_uppercase()
                        ));
                    } else {
                        working_storage.push_str(&format!(
                            "01  WS-{} {} {}.\n",
                            name.to_uppercase(),
                            self.get_cobol_type(&state.ty, &state.attrs),
                            self.get_init_value(&state.ty, &state.attrs, &state.expr)
                        ));
                    }
                }
                TopLevel::Constant(c) => {
                    let cobol_type = self.get_cobol_type(&c.ty, &[]);
                    let name = Self::sanitize_name(&c.name);
                    let init = self.get_expr_init(&c.expr);

                    working_storage.push_str(&format!(
                        "01  WS-{} {} VALUE {}.\n",
                        name.to_uppercase(),
                        cobol_type,
                        init
                    ));
                }
                _ => {}
            }
        }

        (working_storage, linkage)
    }

    fn generate_linkage_params(&self, params: &[(String, Type)]) -> String {
        let mut output = String::new();
        for (name, ty) in params {
            let cobol_type = self.get_cobol_type(ty, &[]);
            output.push_str(&format!(
                "01  LS-{} {} VALUE 0.\n",
                Self::sanitize_name(name).to_uppercase(),
                cobol_type
            ));
        }
        output
    }

    fn extract_parameters(&self, program: &Program) -> Vec<(String, Type)> {
        let mut params = Vec::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                for (name, ty) in &txn.parameters {
                    params.push((name.clone(), ty.clone()));
                }
            }
        }

        params
    }

    fn has_watchdog(&self, program: &Program) -> bool {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.contract.watchdog.is_some() {
                    return true;
                }
            }
        }
        false
    }

    fn collect_all_old_state_vars(&self, program: &Program) -> Vec<String> {
        fn extract_old_vars(expr: &Expr, vars: &mut Vec<String>) {
            match expr {
                Expr::Call(name, args) => {
                    if name == "old" {
                        if let Some(Expr::Identifier(v)) = args.first() {
                            if !vars.contains(v) {
                                vars.push(v.clone());
                            }
                        }
                    }
                    for arg in args {
                        extract_old_vars(arg, vars);
                    }
                }
                Expr::PriorState(name) => {
                    if !vars.contains(name) {
                        vars.push(name.clone());
                    }
                }
                Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
                | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b) | Expr::Le(a, b)
                | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::And(a, b) | Expr::Or(a, b) => {
                    extract_old_vars(a, vars);
                    extract_old_vars(b, vars);
                }
                Expr::Not(a) | Expr::Neg(a) => {
                    extract_old_vars(a, vars);
                }
                _ => {}
            }
        }

        let mut all_vars = Vec::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                extract_old_vars(&txn.contract.post_condition, &mut all_vars);
            }
        }
        all_vars
    }

    fn get_cobol_type(&self, ty: &Type, attrs: &[Attribute]) -> String {
        for attr in attrs {
            if attr.target.as_deref() == Some("cobol") || attr.target.is_none() {
                if attr.key == "type" {
                    if let Some(val) = &attr.value {
                        return val.clone();
                    }
                }
                if attr.key == "native" {
                    return "PIC S9(18) COMP-5".to_string();
                }
                if attr.key == "packed" {
                    return "PIC S9(15)V99 COMP-3".to_string();
                }
                if attr.key == "decimal" {
                    if let Some(val) = &attr.value {
                        let parts: Vec<&str> = val.split(',').collect();
                        if parts.len() == 2 {
                            let whole: u32 = parts[0].parse().unwrap_or(15);
                            let frac: u32 = parts[1].parse().unwrap_or(2);
                            return format!("PIC S9({})V9({}) COMP-3", whole, frac);
                        }
                    }
                }
            }
        }

        match ty {
            Type::Int | Type::UInt => "PIC S9(18) COMP-5".to_string(),
            Type::Float => "PIC S9(15)V99 COMP-3".to_string(),
            Type::Custom(s) if s == "dec" || s == "decimal" => "PIC S9(13)V99 COMP-3".to_string(),
            Type::String => "PIC X(256)".to_string(),
            Type::Bool => "PIC X".to_string(),
            Type::Vector(ty, dims) => {
                let inner = self.get_cobol_type(ty, &[]);
                let total: usize = dims.iter().map(|d| match d {
                    crate::ast::Dimension::Anonymous(s) => *s,
                    crate::ast::Dimension::Named(_, s) => *s,
                }).product();
                format!("OCCURS {} TIMES {}", total, inner)
            }
            _ => "PIC S9(18) COMP-5".to_string(),
        }
    }

    fn get_init_value(&self, ty: &Type, attrs: &[Attribute], expr: &Option<Expr>) -> String {
        for attr in attrs {
            if attr.target.as_deref() == Some("cobol") || attr.target.is_none() {
                if attr.key == "init" {
                    if let Some(val) = &attr.value {
                        return format!("VALUE {}", val);
                    }
                }
            }
        }

        if let Some(e) = expr {
            return self.get_expr_init(e);
        }

        match ty {
            Type::Int | Type::UInt | Type::Float | Type::Custom(_) => "VALUE 0".to_string(),
            Type::String => "VALUE SPACES".to_string(),
            Type::Bool => "VALUE 'N'".to_string(),
            Type::Vector(_, _) => "VALUE SPACES".to_string(),
            _ => "VALUE 0".to_string(),
        }
    }

    fn get_expr_init(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => format!("VALUE {}", n),
            Expr::Float(f) => format!("VALUE {}", f),
            Expr::String(s) => format!("VALUE \"{}\"", s),
            Expr::Bool(b) => format!("VALUE '{}'", if *b { 'Y' } else { 'N' }),
            Expr::Identifier(_) => "VALUE 0".to_string(),
            _ => "VALUE 0".to_string(),
        }
    }

    fn generate_transactions(&mut self, program: &Program, output: &mut String) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.generate_transaction(txn, output);
            }
        }
    }

    fn generate_transaction(&mut self, txn: &crate::ast::Transaction, output: &mut String) {
        let name = Self::sanitize_name(&txn.name).to_uppercase();

        let pre_contract = &txn.contract.pre_condition;
        let cond_str = self.translate_condition(pre_contract);

        if cond_str != "1 = 1" {
            output.push_str(&format!(
                "    * PRE-CONDITION: {}\n    IF NOT ({})\n        DISPLAY \"BRIEF CONTRACT FAILED: PRECONDITION: {}\"\n        MOVE 4000 TO RETURN-CODE",
                Self::expr_to_display(pre_contract),
                cond_str,
                Self::expr_to_display(pre_contract)
            ));
            if self.use_abend {
                output.push_str("\n        CALL \"CEE3ABD\" USING BY VALUE 4000 BY VALUE 0");
            }
            output.push_str("\n        GOBACK\n    END-IF.\n\n");
        }

        self.collect_old_state_captures(&txn.contract.post_condition, output);

        self.generate_body(&txn.body, output);

        let post_contract = &txn.contract.post_condition;
        let cond_str = self.translate_condition(post_contract);
        output.push_str(&format!(
            "    * POST-CONDITION: {}\n    IF NOT ({})\n        DISPLAY \"BRIEF CONTRACT FAILED: POSTCONDITION: {}\"\n        MOVE 4000 TO RETURN-CODE",
            Self::expr_to_display(post_contract),
            cond_str,
            Self::expr_to_display(post_contract)
        ));
        if self.use_abend {
            output.push_str("\n        CALL \"CEE3ABD\" USING BY VALUE 4000 BY VALUE 0");
        }
        output.push_str("\n        GOBACK\n    END-IF.\n\n");

        if let Some(watchdog) = &txn.contract.watchdog {
            output.push_str(&format!(
                "    * WATCHDOG: {}\n    ADD 1 TO WS-RECURSION-DEPTH.\n    IF WS-RECURSION-DEPTH > {}\n        DISPLAY \"BRIEF WATCHDOG: Recursion depth exceeded\"\n        MOVE 4001 TO RETURN-CODE\n        GOBACK\n    END-IF.\n\n",
                Self::expr_to_display(&watchdog.condition),
                self.recursion_limit
            ));
        }
    }

    fn generate_body(&mut self, body: &[Statement], output: &mut String) {
        for stmt in body {
            self.generate_statement(stmt, output);
        }
    }

    fn generate_statement(&mut self, stmt: &Statement, output: &mut String) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                let lhs_name = match lhs {
                    Expr::Identifier(s) => s.clone(),
                    Expr::OwnedRef(s) => s.clone(),
                    _ => "_unknown_".to_string(),
                };
                let target_name = Self::sanitize_name(&lhs_name).to_uppercase();
                let value_str = self.translate_expr(expr);

                if let Expr::Add(lhs, rhs) = expr {
                    let rhs_str = self.translate_expr(rhs);
                    output.push_str(&format!(
                        "    ADD {} TO WS-{}.\n",
                        rhs_str, target_name
                    ));
                    return;
                }
                if let Expr::Sub(lhs, rhs) = expr {
                    let rhs_str = self.translate_expr(rhs);
                    output.push_str(&format!(
                        "    SUBTRACT {} FROM WS-{}.\n",
                        rhs_str, target_name
                    ));
                    return;
                }

                output.push_str(&format!(
                    "    COMPUTE WS-{} = {}.\n",
                    target_name, value_str
                ));
            }
            Statement::Term { .. } | Statement::TermBang { .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for stmt in &cleanup {
                    self.generate_statement(stmt, output);
                }
                output.push_str("    STOP RUN.\n");
            }
            Statement::Escape(expr) => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for stmt in &cleanup {
                    self.generate_statement(stmt, output);
                }
                if let Some(e) = expr {
                    let expr_str = self.translate_expr(e);
                    output.push_str(&format!("    MOVE {} TO RETURN-CODE.\n", expr_str));
                }
                output.push_str("    GOBACK.\n");
            }
            Statement::Guarded { condition, statements } => {
                let cond_str = self.translate_expr(condition);
                output.push_str(&format!("    IF {}\n", cond_str));
                for stmt in statements {
                    self.generate_statement(stmt, output);
                }
                output.push_str("    END-IF.\n");
            }
            Statement::Expression(e) => {
                let expr_str = self.translate_expr(e);
                output.push_str(&format!("    {}.\n", expr_str));
            }
            Statement::Let { name, expr, address_expr, .. } => {
                let safe_name = Self::sanitize_name(name).to_uppercase();
                if let Some(addr) = address_expr {
                    let addr_str = self.translate_expr(addr);
                    output.push_str(&format!("    * let {} at address {}\n", safe_name, addr_str));
                } else if let Some(e) = expr {
                    let val_str = self.translate_expr(e);
                    output.push_str(&format!("    MOVE {} TO WS-{}.\n", val_str, safe_name));
                }
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    let val_str = self.translate_expr(e);
                    output.push_str(&format!("    *> trg! {} = {}\n", name, val_str));
                } else {
                    output.push_str(&format!("    *> trg! {}: await external\n", name));
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                output.push_str("    *> #on_exit cleanup registered\n");
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    output.push_str(&format!("    {}\n", line));
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                output.push_str(&format!("    *> asm: {}\n", asm_string));
            }
            Statement::SyncBlock { .. } => {}
            Statement::Unification { name, variant, fields: _, expr } => {
                let expr_str = self.translate_expr(expr);
                output.push_str(&format!("    *> unification: {} {} = {}\n", name, variant, expr_str));
            }
        }
    }

    fn collect_old_state_captures(&self, post_condition: &Expr, output: &mut String) {
        fn extract_old_vars(expr: &Expr, vars: &mut Vec<String>) {
            match expr {
                Expr::Call(name, args) => {
                    if name == "old" {
                        if let Some(Expr::Identifier(v)) = args.first() {
                            if !vars.contains(v) {
                                vars.push(v.clone());
                            }
                        }
                    }
                    for arg in args {
                        extract_old_vars(arg, vars);
                    }
                }
                Expr::PriorState(name) => {
                    if !vars.contains(name) {
                        vars.push(name.clone());
                    }
                }
                Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
                | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b) | Expr::Le(a, b)
                | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::And(a, b) | Expr::Or(a, b) => {
                    extract_old_vars(a, vars);
                    extract_old_vars(b, vars);
                }
                Expr::Not(a) | Expr::Neg(a) => {
                    extract_old_vars(a, vars);
                }
                _ => {}
            }
        }

        let mut old_vars = Vec::new();
        extract_old_vars(post_condition, &mut old_vars);

        for var in &old_vars {
            let var_name = Self::sanitize_name(var).to_uppercase();
            output.push_str(&format!(
                "    MOVE WS-{} TO WS-OLD-{}.\n",
                var_name, var_name
            ));
        }
        if !old_vars.is_empty() {
            output.push_str("\n");
        }
    }

    fn translate_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("'{}'", s),
            Expr::Bool(b) => if *b { "'Y'" } else { "'N'" }.to_string(),
            Expr::Identifier(name) | Expr::OwnedRef(name) => {
                let n = Self::sanitize_name(name);
                format!("WS-{}", n.to_uppercase())
            }
            Expr::PriorState(name) => {
                format!("WS-OLD-{}", Self::sanitize_name(name).to_uppercase())
            }
            Expr::Eq(a, b) => {
                let a_str = self.translate_expr(a);
                let b_str = self.translate_expr(b);
                if let Expr::Bool(true) = a.as_ref() {
                    format!("({} = {} OR {} = 'y')", b_str, b_str, b_str)
                } else if let Expr::Bool(false) = a.as_ref() {
                    format!("(NOT ({} = {}) AND NOT ({} = 'y'))", b_str, b_str, b_str)
                } else if let Expr::Bool(true) = b.as_ref() {
                    format!("({} = {} OR {} = 'y')", a_str, a_str, a_str)
                } else if let Expr::Bool(false) = b.as_ref() {
                    format!("(NOT ({} = {}) AND NOT ({} = 'y'))", a_str, a_str, a_str)
                } else {
                    format!("({} = {})", a_str, b_str)
                }
            }
            Expr::Add(a, b) => format!("({} + {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Sub(a, b) => format!("({} - {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Mul(a, b) => format!("({} * {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Div(a, b) => format!("({} / {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Mod(a, b) => format!("(FUNCTION MOD({}, {}))", self.translate_expr(a), self.translate_expr(b)),
            Expr::Eq(a, b) => format!("({} = {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Ne(a, b) => format!("(NOT ({} = {}))", self.translate_expr(a), self.translate_expr(b)),
            Expr::Lt(a, b) => format!("({} < {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Le(a, b) => format!("({} <= {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Gt(a, b) => format!("({} > {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Ge(a, b) => format!("({} >= {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::And(a, b) => format!("({} AND {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Or(a, b) => format!("({} OR {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Not(a) => format!("NOT {}", self.translate_expr(a)),
            Expr::Neg(a) => format!("-{}", self.translate_expr(a)),
            Expr::BitNot(a) => format!("(FUNCTION BOOLEAN(-1, 0) - {})", self.translate_expr(a)),
            Expr::BitAnd(a, b) => format!("(FUNCTION BOOLEAN({}, {}))", self.translate_expr(a), self.translate_expr(b)),
            Expr::BitOr(a, b) => format!("(FUNCTION INTEGER({}) + FUNCTION INTEGER({}) - FUNCTION BOOLEAN({}, {}))", self.translate_expr(a), self.translate_expr(b), self.translate_expr(a), self.translate_expr(b)),
            Expr::BitXor(a, b) => format!("(FUNCTION INTEGER({}) + FUNCTION INTEGER({}) - 2 * FUNCTION BOOLEAN({}, {}))", self.translate_expr(a), self.translate_expr(b), self.translate_expr(a), self.translate_expr(b)),
            Expr::Shl(a, b) => format!("({} * FUNCTION 2 ** ({}))", self.translate_expr(a), self.translate_expr(b)),
            Expr::Shr(a, b) => format!("({} / FUNCTION 2 ** ({}))", self.translate_expr(a), self.translate_expr(b)),
            Expr::Call(name, args) => {
                if name == "old" {
                    if let Some(arg) = args.first() {
                        if let Expr::Identifier(v) = arg {
                            return format!("WS-OLD-{}", Self::sanitize_name(v).to_uppercase());
                        }
                    }
                }
                format!("{}({})", Self::sanitize_name(name), args.iter().map(|a| self.translate_expr(a)).collect::<Vec<_>>().join(", "))
            }
            Expr::ListLiteral(items) => {
                if items.is_empty() {
                    "0".to_string()
                } else {
                    items.iter().map(|i| self.translate_expr(i)).collect::<Vec<_>>().join(", ")
                }
            }
            Expr::ListIndex(list, index) => {
                let list_str = self.translate_expr(list);
                let index_str = self.translate_expr(index);
                format!("{} ({})", list_str, index_str)
            }
            Expr::Projection { source: list, .. } => {
                format!("FUNCTION LENGTH({})", self.translate_expr(list))
            }
            Expr::FieldAccess(obj, field) => {
                let obj_str = self.translate_expr(obj);
                format!("{}-{}", obj_str, Self::sanitize_name(field).to_uppercase())
            }
            _ => "0".to_string(),
        }
    }

    fn translate_condition(&self, expr: &Expr) -> String {
        match expr {
            Expr::Bool(true) => "1 = 1".to_string(),
            Expr::Bool(false) => "1 = 0".to_string(),
            Expr::Eq(a, b) => {
                let a_str = self.translate_expr(a);
                let b_str = self.translate_expr(b);
                format!("({} = {})", a_str, b_str)
            }
            Expr::Ne(a, b) => {
                let a_str = self.translate_expr(a);
                let b_str = self.translate_expr(b);
                format!("(NOT ({} = {}))", a_str, b_str)
            }
            Expr::Lt(a, b) => format!("({} < {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Le(a, b) => format!("({} <= {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Gt(a, b) => format!("({} > {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::Ge(a, b) => format!("({} >= {})", self.translate_expr(a), self.translate_expr(b)),
            Expr::And(a, b) => format!("({} AND {})", self.translate_condition(a), self.translate_condition(b)),
            Expr::Or(a, b) => format!("({} OR {})", self.translate_condition(a), self.translate_condition(b)),
            Expr::Not(a) => format!("NOT {}", self.translate_condition(a)),
            _ => self.translate_expr(expr),
        }
    }

    fn sanitize_name(name: &str) -> String {
        name.chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    }

    fn expr_to_display(expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => s.clone(),
            Expr::Bool(b) => b.to_string(),
            Expr::Identifier(name) | Expr::OwnedRef(name) => name.clone(),
            Expr::Add(a, b) => format!("({} + {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Sub(a, b) => format!("({} - {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Mul(a, b) => format!("({} * {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Div(a, b) => format!("({} / {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Eq(a, b) => format!("({} == {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Ne(a, b) => format!("({} != {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Lt(a, b) => format!("({} < {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Le(a, b) => format!("({} <= {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Gt(a, b) => format!("({} > {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Ge(a, b) => format!("({} >= {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::And(a, b) => format!("({} and {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Or(a, b) => format!("({} or {})", Self::expr_to_display(a), Self::expr_to_display(b)),
            Expr::Call(name, args) => format!("{}(...)", name),
            _ => "...".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Contract, OutputType, Transaction};

    fn test_program() -> Program {
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "transfer".to_string(),
            parameters: vec![
                ("balance".to_string(), Type::Custom("dec".to_string())),
                ("amount".to_string(), Type::Custom("dec".to_string())),
            ],
            contract: Contract {
                pre_condition: Expr::Gt(
                    Box::new(Expr::Identifier("amount".to_string())),
                    Box::new(Expr::Integer(0)),
                ),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("balance".to_string())),
                    Box::new(Expr::Call("old".to_string(), vec![Expr::Identifier("balance".to_string())])),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::Identifier("balance".to_string()),
                    expr: Expr::Sub(
                        Box::new(Expr::Identifier("balance".to_string())),
                        Box::new(Expr::Identifier("amount".to_string())),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     };

        Program {
            items: vec![TopLevel::Transaction(txn)],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    #[test]
    fn test_basic_generation() {
        let mut backend = CobolBackend::new();
        let program = test_program();
        let output = backend.generate(&program, "transfer");

        assert!(output.contains(">>SOURCE FORMAT IS FREE"));
        assert!(output.contains("PROGRAM-ID. TRANSFER RECURSIVE"));
        assert!(output.contains("WORKING-STORAGE SECTION"));
        assert!(output.contains("LINKAGE SECTION"));
    }

    #[test]
    fn test_precondition_generation() {
        let mut backend = CobolBackend::new();
        let program = test_program();
        let output = backend.generate(&program, "transfer");

        assert!(output.contains("PRE-CONDITION"));
        assert!(output.contains("GOBACK"));
    }

    #[test]
    fn test_postcondition_old_capture() {
        let mut backend = CobolBackend::new();
        let program = test_program();
        let output = backend.generate(&program, "transfer");

        assert!(output.contains("WS-OLD-BALANCE"));
        assert!(output.contains("POST-CONDITION"));
    }

    #[test]
    fn test_cobol_generates_program() {
        let mut backend = CobolBackend::new();
        let program = Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program, "test_program");
        assert!(output.contains("PROGRAM-ID"));
        assert!(output.contains("DATA DIVISION"));
        assert!(output.contains("PROCEDURE DIVISION"));
    }
}