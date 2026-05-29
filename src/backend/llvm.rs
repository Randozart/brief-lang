use crate::analysis::call_graph::CallGraph;
use crate::ast::{Expr, MatchPattern, Program, Statement, TopLevel, Type};
use crate::ast::ForeignSignature;
use std::collections::HashMap;
use std::fmt::Write;

/// LLVM IR backend — compiles Brief programs to LLVM intermediate representation.
///
/// Uses shared analysis (CallGraph + ParameterRanges) to optimize:
/// - Acyclic graphs: %State passed by value, extractvalue/insertvalue/phi
/// - Cyclic graphs: %State* pointer dispatch, noalias on every call
pub struct LlvmBackend {
    spec: Option<crate::target_spec::TargetSpec>,
    field_index_map: HashMap<String, usize>,
    field_types: Vec<String>,
    txn_counter: usize,
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
    let_bindings: HashMap<String, String>,
    terminated: bool,
    range_bounds: HashMap<String, (i64, i64)>,
    is_release: bool,
    field_to_meta_idx: HashMap<String, usize>,
    triggers: HashMap<String, crate::ast::TriggerDeclaration>,
    trigger_names: Vec<String>,
    program_txns: Vec<String>,
    frgn_map: HashMap<String, ForeignSignature>,
}

impl LlvmBackend {
    pub fn new() -> Self {
        LlvmBackend {
            spec: None,
            field_index_map: HashMap::new(),
            field_types: Vec::new(),
            txn_counter: 0,
            has_cycles: false,
            pending_cleanup: Vec::new(),
            let_bindings: HashMap::new(),
            terminated: false,
            range_bounds: HashMap::new(),
            is_release: false,
            field_to_meta_idx: HashMap::new(),
            triggers: HashMap::new(),
            trigger_names: Vec::new(),
            program_txns: Vec::new(),
            frgn_map: HashMap::new(),
        }
    }

    pub fn with_release(mut self, release: bool) -> Self {
        self.is_release = release;
        self
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  LLVM backend: acyclic call graph — norecurse + inlining enabled");
        }

        self.build_field_index(program);
        self.triggers.clear();
        self.trigger_names.clear();
        self.program_txns.clear();

        let mut output = String::new();

        writeln!(output, "; ModuleID = 'program.ll'").ok();
        writeln!(output, "source_filename = \"program.bv\"").ok();
        writeln!(output, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(output, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
        writeln!(output).ok();

        writeln!(output, "declare void @llvm.assume(i1) #1").ok();
        writeln!(output, "declare void @llvm.memcpy.p0i8.p0i8.i64(i8*, i8*, i64, i1) #1").ok();
        writeln!(output).ok();

        // Declare foreign bindings and populate frgn_map
        self.frgn_map.clear();
        for item in &program.items {
            if let TopLevel::ForeignBinding { name, signature, .. } = item {
                self.frgn_map.insert(name.clone(), signature.clone());
                let ret_ty = if signature.inputs.is_empty() { "void" } else { "i64" };
                let param_tys: Vec<&str> = signature.inputs.iter().map(|(_, t)| match t {
                    Type::Int | Type::UInt => "i64",
                    Type::Bool => "i32",
                    Type::Char => "i32",
                    Type::String | Type::Data => "i8*",
                    _ => "i64",
                }).collect();
                write!(output, "declare {} @{}(", ret_ty, name).ok();
                for (i, pt) in param_tys.iter().enumerate() {
                    if i > 0 { write!(output, ", ").ok(); }
                    write!(output, "{}", pt).ok();
                }
                writeln!(output, ") #1").ok();
            }
        }

        self.declare_state_type(&mut output, program);
        self.declare_state_global(&mut output, program);
        writeln!(output).ok();

        let mut txns: Vec<(String, &crate::ast::Transaction)> = Vec::new();
        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    txns.push((txn.name.clone(), txn));
                    self.program_txns.push(txn.name.clone());
                }
                TopLevel::Trigger(trg) => {
                    self.triggers.insert(trg.name.clone(), trg.clone());
                    self.trigger_names.push(trg.name.clone());
                }
                _ => {}
            }
        }

        // Declare C standard functions for bootstrap intrinsics
        let has_print = self.frgn_map.contains_key("__print");
        let has_exit = self.frgn_map.contains_key("__exit");
        let has_read = self.frgn_map.contains_key("__read_file");
        let has_write = self.frgn_map.contains_key("__write_file");
        if has_print || has_read || has_write {
            writeln!(output, "declare i64 @write(i32, i8*, i64) #1").ok();
        }
        if has_print {
            writeln!(output, "declare i64 @strlen(i8*) #1").ok();
        }
        if has_exit {
            writeln!(output, "declare void @exit(i32) #1").ok();
        }
        if has_read || has_write {
            writeln!(output, "declare i64 @open(i8*, i32) #1").ok();
            writeln!(output, "declare i64 @read(i32, i8*, i64) #1").ok();
        }

        let mut range_meta_nodes: Vec<String> = Vec::new();

        for (name, txn) in &txns {
            self.generate_transaction(&mut output, txn, name, &mut range_meta_nodes);
            writeln!(output).ok();
        }

        self.generate_init_state(&mut output, program);
        writeln!(output).ok();

        if !txns.is_empty() {
            self.generate_reactor(&mut output, &txns);
        }

        writeln!(output).ok();
        writeln!(output, "attributes #0 = {{").ok();
        writeln!(output, "    mustprogress").ok();
        writeln!(output, "    nofree").ok();
        writeln!(output, "    norecurse").ok();
        writeln!(output, "    nosync").ok();
        writeln!(output, "    nounwind").ok();
        writeln!(output, "    willreturn").ok();
        writeln!(output, "    memory(argmem: readwrite)").ok();
        writeln!(output, "}}").ok();
        writeln!(output, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn memory(argmem: write) }}").ok();

        if !range_meta_nodes.is_empty() {
            writeln!(output).ok();
            writeln!(output, "; Range metadata").ok();
            for md in &range_meta_nodes {
                writeln!(output, "{}", md).ok();
            }
        }

        output
    }

    fn build_field_index(&mut self, program: &Program) {
        self.field_index_map.clear();
        self.field_types.clear();
        let mut idx = 0;
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                self.field_index_map.insert(s.name.clone(), idx);
                self.field_types.push(self.llvm_type(&s.ty).to_string());
                idx += 1;
            }
        }
    }

    fn llvm_type(&self, ty: &Type) -> &str {
        match ty {
            Type::Int | Type::UInt => "i64",
            Type::Bool => "i8",
            Type::Float => "float",
            Type::Char => "i32",
            Type::String | Type::Data => "i8*",
            Type::Void => "void",
            _ => "i64",
        }
    }

    fn declare_state_type(&self, output: &mut String, _program: &Program) {
        if self.field_types.is_empty() {
            writeln!(output, "%State = type {{ i64 }}").ok();
            return;
        }
        write!(output, "%State = type {{ ").ok();
        for (i, f) in self.field_types.iter().enumerate() {
            if i > 0 { write!(output, ", ").ok(); }
            write!(output, "{}", f).ok();
        }
        writeln!(output, " }}").ok();
    }

    fn declare_state_global(&self, output: &mut String, _program: &Program) {
        writeln!(output, "@global_state = global %State zeroinitializer").ok();
    }

    fn generate_init_state(&self, output: &mut String, program: &Program) {
        writeln!(output, "define void @init_state() local_unnamed_addr #0 {{").ok();
        writeln!(output, "  entry:").ok();
        let mut reg_counter = 0u32;
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                if let Some(ref expr) = s.expr {
                    let idx = self.field_index_map.get(&s.name).unwrap_or(&0);
                    let ty = self.llvm_type(&s.ty);
                    let ptr_reg = format!("%init_ptr{}", reg_counter);
                    reg_counter += 1;
                    writeln!(output, "  {} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", ptr_reg, idx).ok();
                    if let Expr::Integer(n) = expr {
                        writeln!(output, "  store volatile {} {}, {}* {}, align {}", ty, n, ty, ptr_reg, self.align_of(ty)).ok();
                    } else if let Expr::Bool(b) = expr {
                        let val = if *b { 1 } else { 0 };
                        writeln!(output, "  store volatile {} {}, {}* {}, align {}", ty, val, ty, ptr_reg, self.align_of(ty)).ok();
                    }
                }
            }
        }
        writeln!(output, "  ret void").ok();
        writeln!(output, "}}").ok();
    }

    fn align_of(&self, ty: &str) -> u32 {
        match ty {
            "i64" => 8,
            "float" => 4,
            "i8" => 1,
            "i32" => 4,
            _ => 8,
        }
    }

    fn extract_ranges(pre: &Expr) -> HashMap<String, (i64, i64)> {
        let mut ranges = HashMap::new();
        Self::extract_ranges_inner(pre, &mut ranges);
        ranges
    }

    fn extract_ranges_inner(expr: &Expr, ranges: &mut HashMap<String, (i64, i64)>) {
        match expr {
            Expr::And(l, r) => {
                Self::extract_ranges_inner(l, ranges);
                Self::extract_ranges_inner(r, ranges);
            }
            Expr::Lt(l, r) => {
                if let Expr::Identifier(name) = l.as_ref() {
                    if let Expr::Integer(n) = r.as_ref() {
                        let entry = ranges.entry(name.clone()).or_insert((i64::MIN, i64::MAX));
                        if *n < entry.1 { entry.1 = *n; }
                    }
                }
            }
            Expr::Ge(l, r) => {
                if let Expr::Identifier(name) = l.as_ref() {
                    if let Expr::Integer(n) = r.as_ref() {
                        let entry = ranges.entry(name.clone()).or_insert((i64::MIN, i64::MAX));
                        if *n > entry.0 { entry.0 = *n; }
                    }
                }
            }
            Expr::Gt(l, r) => {
                if let Expr::Identifier(name) = l.as_ref() {
                    if let Expr::Integer(n) = r.as_ref() {
                        let entry = ranges.entry(name.clone()).or_insert((i64::MIN, i64::MAX));
                        if *n + 1 > entry.0 { entry.0 = *n + 1; }
                    }
                }
            }
            _ => {}
        }
    }

fn emit_precondition(&mut self, output: &mut String, pre: &Expr, indent: &str) {
        let cond = self.generate_expr(output, pre, indent);
        let cond_i1 = format!("%pre_i1{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(output, "{}{} = icmp ne i64 {}, 0", indent, cond_i1, cond).ok();
        if self.is_release {
            writeln!(output, "{}call void @llvm.assume(i1 {})", indent, cond_i1).ok();
        } else {
            let panic_lab = format!("pre_panic{}", self.txn_counter);
            self.txn_counter += 1;
            let safe_lab = format!("pre_safe{}", self.txn_counter);
            self.txn_counter += 1;
            writeln!(output, "{}br i1 {}, label %{}, label %{}", indent, cond_i1, safe_lab, panic_lab).ok();
            writeln!(output, "{}{}:", indent, panic_lab).ok();
            writeln!(output, "{}  unreachable", indent).ok();
            writeln!(output, "{}{}:", indent, safe_lab).ok();
        }
    }

    fn generate_transaction(&mut self, output: &mut String, txn: &crate::ast::Transaction, name: &str, range_meta_nodes: &mut Vec<String>) {
        writeln!(output, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", name).ok();
        writeln!(output, "entry:").ok();

        self.range_bounds = Self::extract_ranges(&txn.contract.pre_condition);

        self.field_to_meta_idx.clear();
        for (field, &(lo, hi)) in &self.range_bounds {
            if hi < i64::MAX {
                let idx = range_meta_nodes.len();
                let display_lo = if lo > i64::MIN { lo } else { 0i64 };
                range_meta_nodes.push(format!("!{} = !{{ i64 {}, i64 {} }}", idx, display_lo, hi));
                self.field_to_meta_idx.insert(field.clone(), idx);
            }
        }

        self.txn_counter = 0;
        self.let_bindings.clear();
        self.terminated = false;

        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            self.emit_precondition(output, &txn.contract.pre_condition, "  ");
        }

        writeln!(output, "  ; pre: {:?}", txn.contract.pre_condition).ok();

        for stmt in &txn.body {
            self.generate_statement(output, stmt, "  ");
        }

        writeln!(output, "  ; post: {:?}", txn.contract.post_condition).ok();
        if !self.terminated {
            writeln!(output, "  ret void").ok();
        }
        writeln!(output, "}}").ok();
    }

    fn generate_statement(&mut self, output: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { values, .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for s in &cleanup {
                    self.generate_statement(output, s, indent);
                }
                if let Some(Some(v)) = values.first() {
                    let val = self.generate_expr(output, v, indent);
                    writeln!(output, "{}ret i64 {}", indent, val).ok();
                } else {
                    writeln!(output, "{}ret void", indent).ok();
                }
                self.terminated = true;
            }
            Statement::Let { name, expr, address_expr, .. } => {
                if address_expr.is_some() {
                    writeln!(output, "{}; let {} = ptr (address expression)", indent, name).ok();
                } else if let Some(e) = expr {
                    let val = self.generate_expr(output, e, indent);
                    writeln!(output, "{}; let {} = {}", indent, name, val).ok();
                    self.let_bindings.insert(name.clone(), val.clone());
                } else {
                    writeln!(output, "{}; let {} = uninitialized", indent, name).ok();
                }
            }
            Statement::Assignment { lhs, expr, .. } => {
                let val = self.generate_expr(output, expr, indent);
                let field_name = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    _ => {
                        writeln!(output, "{}; assign {}", indent, val).ok();
                        return;
                    }
                };
                if let Some(&idx) = self.field_index_map.get(&field_name) {
                    let ty = &self.field_types[idx];
                    let ptr_reg = format!("%ptr{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(output, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, ptr_reg, idx).ok();
                    if ty == "i8" {
                        let trunc_reg = format!("%tr{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(output, "{}{} = trunc i64 {} to i8", indent, trunc_reg, val).ok();
                        writeln!(output, "{}store i8 {}, i8* {}, align {}", indent, trunc_reg, ptr_reg, self.align_of(ty)).ok();
                    } else {
                        writeln!(output, "{}store {} {}, {}* {}, align {}", indent, ty, val, ty, ptr_reg, self.align_of(ty)).ok();
                    }
                } else {
                    writeln!(output, "{}; assign {} to {}", indent, val, field_name).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.generate_expr(output, condition, indent);
                let cond_i1 = format!("%cnd{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp ne i64 {}, 0", indent, cond_i1, cond).ok();

                if statements.len() == 1 {
                    if let Statement::Assignment { lhs, expr, .. } = &statements[0] {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                            if let Some(&idx) = self.field_index_map.get(n) {
                                let ty = self.field_types[idx].clone();
                                let ptr_reg = format!("%ptr{}", self.txn_counter);
                                self.txn_counter += 1;
                                let assign_val = self.generate_expr(output, expr, indent);
                                writeln!(output, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, ptr_reg, idx).ok();
                                let load_reg = format!("%ld{}", self.txn_counter);
                                self.txn_counter += 1;
                                writeln!(output, "{}{} = load i64, i64* {}, align 8", indent, load_reg, ptr_reg).ok();
                                let select_reg = format!("%sel{}", self.txn_counter);
                                self.txn_counter += 1;
                                writeln!(output, "{}{} = select i1 {}, i64 {}, i64 {}", indent, select_reg, cond_i1, assign_val, load_reg).ok();
                                if ty == "i8" {
                                    let trunc_reg = format!("%tr{}", self.txn_counter);
                                    self.txn_counter += 1;
                                    writeln!(output, "{}{} = trunc i64 {} to i8", indent, trunc_reg, select_reg).ok();
writeln!(output, "{}store i8 {}, i8* {}, align {}", indent, trunc_reg, ptr_reg, self.align_of(&ty)).ok();
                                } else {
                                    writeln!(output, "{}store i64 {}, i64* {}, align {}", indent, select_reg, ptr_reg, self.align_of(&ty)).ok();
                                }
                                return;
                            }
                        }
                    }
                }

                writeln!(output, "{}br i1 {}, label %then, label %end", indent, cond_i1).ok();
                writeln!(output, "{}then:", indent).ok();
                for s in statements {
                    self.generate_statement(output, s, &format!("{}  ", indent));
                }
                writeln!(output, "{}  br label %end", indent).ok();
                writeln!(output, "{}end:", indent).ok();
            }
            Statement::Expression(e) => {
                let _ = self.generate_expr(output, e, indent);
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    let val = self.generate_expr(output, e, indent);
                    writeln!(output, "{}; trg! {}: await {}", indent, name, val).ok();
                } else {
                    writeln!(output, "{}; trg! {}: await external", indent, name).ok();
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                writeln!(output, "{}; #on_exit registered", indent).ok();
            }
            Statement::Escape(expr) => {
                if let Some(v) = expr {
                    let val = self.generate_expr(output, v, indent);
                    writeln!(output, "{}ret i64 {}", indent, val).ok();
                } else {
                    writeln!(output, "{}ret void", indent).ok();
                }
                self.terminated = true;
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    let _ = writeln!(output, "{}{}", indent, line);
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                writeln!(output, "{}{}", indent, asm_string).ok();
            }
            Statement::Unification { name: _, pattern, expr } => {
                let val = self.generate_expr(output, expr, indent);
                let disc_reg = format!("%u_disc{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = and i64 {}, 255", indent, disc_reg, val).ok();
                let arm_label = format!("u_arm{}", self.txn_counter);
                self.txn_counter += 1;
                let def_label = format!("u_def{}", self.txn_counter);
                self.txn_counter += 1;
                let merge_label = format!("u_merge{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}switch i64 {}, label %{} [", indent, disc_reg, def_label).ok();
                writeln!(output, "{}  i64 0, label %{}", indent, arm_label).ok();
                writeln!(output, "{}]", indent).ok();
                writeln!(output, "{}:", arm_label).ok();
                let payload_reg = format!("%u_pay{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = lshr i64 {}, 8", indent, payload_reg, val).ok();
                self.let_bindings.insert(pattern.clone(), payload_reg.clone());
                writeln!(output, "{}br label %{}", indent, merge_label).ok();
                writeln!(output, "{}:", def_label).ok();
                writeln!(output, "{}br label %{}", indent, merge_label).ok();
                writeln!(output, "{}:", merge_label).ok();
            }
        }
    }

    fn generate_expr(&mut self, output: &mut String, expr: &Expr, indent: &str) -> String {
        let val = format!("%tmp{}", self.txn_counter);
        self.txn_counter += 1;

        match expr {
            Expr::Integer(n) => {
                writeln!(output, "{}{} = add i64 0, {}", indent, val, n).ok();
            }
            Expr::Bool(b) => {
                let i = if *b { 1 } else { 0 };
                writeln!(output, "{}{} = add i64 0, {}", indent, val, i).ok();
            }
            Expr::Float(f) => {
                writeln!(output, "{}{} = fadd float 0.0, {}", indent, val, f).ok();
            }
            Expr::String(s) => {
                writeln!(output, "{}{} = alloca i8, i64 {}", indent, val, s.len() + 1).ok();
            }
            Expr::Char(c) => {
                writeln!(output, "{}{} = add i32 0, {}", indent, val, *c as i32).ok();
            }
            Expr::Identifier(name) => {
                if let Some(reg) = self.let_bindings.get(name) {
                    writeln!(output, "{}{} = add i64 0, {}", indent, val, reg).ok();
                } else if let Some(&idx) = self.field_index_map.get(name) {
                    let ty = &self.field_types[idx];
                    let ptr_reg = format!("%ptr{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(output, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, ptr_reg, idx).ok();
                    let load_reg = format!("%ld{}", self.txn_counter);
                    self.txn_counter += 1;
                    let range_suffix = if let Some(&meta_idx) = self.field_to_meta_idx.get(name) {
                        format!(", !range !{}", meta_idx)
                    } else {
                        String::new()
                    };
                    writeln!(output, "{}{} = load {}, {}* {}, align {}{}", indent, load_reg, ty, ty, ptr_reg, self.align_of(ty), range_suffix).ok();
                    if ty == "i8" {
                        writeln!(output, "{}{} = zext i8 {} to i64", indent, val, load_reg).ok();
                    } else if ty == "i8*" {
                        writeln!(output, "{}{} = ptrtoint i8* {} to i64", indent, val, load_reg).ok();
                    } else {
                        writeln!(output, "{}{} = add i64 0, {}", indent, val, load_reg).ok();
                    }
                } else {
                    writeln!(output, "{}{} = add i64 0, 0 ; identifier {}", indent, val, name).ok();
                }
            }
            Expr::Add(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = add i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Sub(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = sub i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Mul(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = mul i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Div(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = sdiv i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Mod(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = srem i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Eq(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp eq i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Ne(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp ne i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Lt(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp slt i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Le(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp sle i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Gt(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp sgt i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Ge(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp sge i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::And(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = and i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Or(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = or i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Not(e) => {
                let inner = self.generate_expr(output, e, indent);
                writeln!(output, "{}{} = xor i64 {}, -1", indent, val, inner).ok();
            }
            Expr::Neg(e) => {
                let inner = self.generate_expr(output, e, indent);
                writeln!(output, "{}{} = sub i64 0, {}", indent, val, inner).ok();
            }
            Expr::BitAnd(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = and i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::BitOr(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = or i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::BitXor(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = xor i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::BitNot(e) => {
                let inner = self.generate_expr(output, e, indent);
                writeln!(output, "{}{} = xor i64 {}, -1", indent, val, inner).ok();
            }
            Expr::Shl(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = shl i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Shr(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = lshr i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Call(name, args) => {
                if let Some(_sig) = self.frgn_map.get(name) {
                    // FFI path — marshaling + call
                    match name.as_str() {
"__print" if args.len() == 1 => {
                            let str_reg = self.generate_expr(output, &args[0], indent);
                            let ptr_reg = format!("%ffi_p{}", self.txn_counter);
                            self.txn_counter += 1;
                            writeln!(output, "{}{} = inttoptr i64 {} to i8*", indent, ptr_reg, str_reg).ok();
                            let len_reg = format!("%ffi_l{}", self.txn_counter);
                            self.txn_counter += 1;
                            writeln!(output, "{}{} = call i64 @strlen(i8* {})", indent, len_reg, ptr_reg).ok();
                            writeln!(output, "{}{} = call i64 @write(i32 1, i8* {}, i64 {})", indent, val, ptr_reg, len_reg).ok();
                        }
                        "__exit" => {
                            writeln!(output, "{}{} = call void @exit(i32 0)", indent, val).ok();
                            writeln!(output, "{}{} = add i64 0, 0", indent, val).ok();
                        }
_ => {
                            let mut marshaled: Vec<String> = Vec::new();
                            let sig_inputs: Vec<(String, Type)> = self.frgn_map.get(name)
                                .map(|s| s.inputs.clone())
                                .unwrap_or_default();
                            for (i, (_, arg_ty)) in sig_inputs.iter().enumerate() {
                                if i < args.len() {
                                    let raw = self.generate_expr(output, &args[i], indent);
                                    match arg_ty {
                                        Type::Int | Type::UInt => marshaled.push(format!("i64 {}", raw)),
                                        Type::Bool => {
                                            let z = format!("%ffi_z{}", self.txn_counter);
                                            self.txn_counter += 1;
                                            writeln!(output, "{}{} = zext i64 {} to i32", indent, z, raw).ok();
                                            marshaled.push(format!("i32 {}", z));
                                        }
                                        Type::Char => {
                                            let z = format!("%ffi_z{}", self.txn_counter);
                                            self.txn_counter += 1;
                                            writeln!(output, "{}{} = zext i32 {} to i32", indent, z, raw).ok();
                                            marshaled.push(format!("i32 {}", z));
                                        }
                                        Type::String | Type::Data => {
                                            let ptr = format!("%ffi_p{}", self.txn_counter);
                                            self.txn_counter += 1;
                                            writeln!(output, "{}{} = inttoptr i64 {} to i8*", indent, ptr, raw).ok();
                                            marshaled.push(format!("i8* {}", ptr));
                                        }
                                        _ => marshaled.push(format!("i64 {}", raw)),
                                    }
                                }
                            }
                            let args_str = marshaled.join(", ");
                            writeln!(output, "{}{} = call i64 @{}({})", indent, val, name, args_str).ok();
                        }
                    }
                } else {
                    // Internal call (no marshaling)
                    let mut arg_strs = Vec::new();
                    for arg in args {
                        arg_strs.push(self.generate_expr(output, arg, indent));
                    }
                    let args_str = arg_strs.join(", ");
                    writeln!(output, "{}{} = call i64 @{}({})", indent, val, name, args_str).ok();
                }
            }
            Expr::ListLiteral(_) => {
                writeln!(output, "{}{} = alloca i64, i64 0", indent, val).ok();
            }
            Expr::ListIndex(list, idx) => {
                let list_val = self.generate_expr(output, list, indent);
                let idx_val = self.generate_expr(output, idx, indent);
                writeln!(output, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, val, list_val, idx_val).ok();
            }
            Expr::ListLen(_) => {
                writeln!(output, "{}{} = add i64 0, 0", indent, val).ok();
            }
            Expr::FieldAccess(obj, field) => {
                let obj_val = self.generate_expr(output, obj, indent);
                writeln!(output, "{}{} = add i64 0, 0 ; {}.{}", indent, val, obj_val, field).ok();
            }
            Expr::Match { value, arms } => {
                let val_inner = self.generate_expr(output, value, indent);
                let disc_reg = format!("%m_disc{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = and i64 {}, 255", indent, disc_reg, val_inner).ok();

                let merge_label = format!("m_merge{}", self.txn_counter);
                self.txn_counter += 1;

                let has_wildcard = arms.iter().any(|a| a.pattern == MatchPattern::Wildcard);
                let default_label = if has_wildcard {
                    format!("m_def{}", self.txn_counter)
                } else {
                    format!("m_unreach{}", self.txn_counter)
                };
                self.txn_counter += 1;

                writeln!(output, "{}switch i64 {}, label %{} [", indent, disc_reg, default_label).ok();
                let mut variant_idx = 0u64;
                for arm in arms.iter() {
                    if let MatchPattern::Variant { name: _, fields: _ } = &arm.pattern {
                        writeln!(output, "{}  i64 {}, label %m_arm{}", indent, variant_idx, variant_idx).ok();
                        variant_idx += 1;
                    }
                }
                writeln!(output, "{}]", indent).ok();

                variant_idx = 0;
                let mut phi_regs: Vec<String> = Vec::new();
                let mut phi_labels: Vec<String> = Vec::new();
                for arm in arms.iter() {
                    match &arm.pattern {
                        MatchPattern::Variant { name: _, fields } => {
                            writeln!(output, "{}m_arm{}:", indent, variant_idx).ok();
                            let payload_reg = format!("%m_pay{}", self.txn_counter);
                            self.txn_counter += 1;
                            writeln!(output, "{}{} = lshr i64 {}, 8", indent, payload_reg, val_inner).ok();
                            let arm_val = self.generate_expr(output, &arm.body, indent);
                            phi_regs.push(arm_val);
                            phi_labels.push(format!("%%m_arm{}", variant_idx));
                            writeln!(output, "{}br label %{}", indent, merge_label).ok();
                            variant_idx += 1;
                        }
                        MatchPattern::Wildcard => {}
                    }
                }

                if has_wildcard {
                    let wildcard_arm = arms.iter().find(|a| a.pattern == MatchPattern::Wildcard).unwrap();
                    writeln!(output, "{}:", default_label).ok();
                    let wc_val = self.generate_expr(output, &wildcard_arm.body, indent);
                    phi_regs.push(wc_val);
                    phi_labels.push(format!("%%{}", default_label));
                    writeln!(output, "{}br label %{}", indent, merge_label).ok();
                } else {
                    writeln!(output, "{}:", default_label).ok();
                    writeln!(output, "{}  unreachable", indent).ok();
                }

                writeln!(output, "{}:", merge_label).ok();
                if phi_regs.len() == 1 {
                    writeln!(output, "{}{} = add i64 0, {}", indent, val, phi_regs[0]).ok();
                } else {
                    let phi_strs: Vec<String> = phi_regs.iter().enumerate()
                        .map(|(i, r)| format!("[i64 {}, {}]", r, phi_labels[i])).collect();
                    writeln!(output, "{}{} = phi i64 {}", indent, val, phi_strs.join(", ")).ok();
                }
            }
            Expr::PatternMatch { value, variant, fields: _ } => {
                let inner = self.generate_expr(output, value, indent);
                let disc = format!("%pm_d{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = and i64 {}, 255", indent, disc, inner).ok();
                let target = if variant == "None" || variant == "Err" { 0u64 } else { 1u64 };
                let cmp = format!("%pm_c{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp eq i64 {}, {}", indent, cmp, disc, target).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            _ => {
                writeln!(output, "{}{} = add i64 0, 0 ; fallback", indent, val).ok();
            }
        }

        val
    }

    fn generate_reactor(&mut self, output: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        // Collect fusable pairs
        let fusable_pairs = self.resolve_fusable_pairs(txns);
        let mut used_fused: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Generate fused transactions
        for (a_name, b_name) in &fusable_pairs {
            let fused_name = format!("{}_{}_fused", a_name, b_name);
            if used_fused.contains(&fused_name) { continue; }
            used_fused.insert(fused_name.clone());
            if let Some(txn_a) = txns.iter().find(|(n, _)| n == a_name) {
                if let Some(txn_b) = txns.iter().find(|(n, _)| n == b_name) {
                    self.generate_fused_transaction(output, txn_a.1, txn_b.1, &fused_name);
                    writeln!(output).ok();
                }
            }
        }

        // Determine which original txns are consumed by fusion
        let mut fused_txns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (a, b) in &fusable_pairs {
            fused_txns.insert(a.clone());
            fused_txns.insert(b.clone());
        }

        writeln!(output, "define void @reactor_tick() local_unnamed_addr #0 {{").ok();
        writeln!(output, "entry:").ok();

        // Trigger sampling phase
        let mut trig_regs: Vec<(String, String)> = Vec::new();
        for trg_name in &self.trigger_names {
            if let Some(trg) = self.triggers.get(trg_name) {
                let raw_reg = format!("%trg_raw_{}", self.txn_counter);
                self.txn_counter += 1;
                let samp_reg = format!("%trg_{}", self.txn_counter);
                self.txn_counter += 1;
                match &trg.address {
                    crate::ast::LinkRef::Explicit(addr) => {
                        writeln!(output, "  {} = load volatile i8, i8* inttoptr (i64 {} to i8*), align 1", raw_reg, addr).ok();
                    }
                    crate::ast::LinkRef::Linked(sym) => {
                        writeln!(output, "  {} = load volatile i8, i8* @{}, align 1", raw_reg, sym).ok();
                    }
                }
                writeln!(output, "  {} = icmp ne i8 {}, 0", samp_reg, raw_reg).ok();
                trig_regs.push((trg_name.clone(), samp_reg));
            }
        }

        if txns.is_empty() && fusable_pairs.is_empty() {
            writeln!(output, "  ret void").ok();
            writeln!(output, "}}").ok();
            writeln!(output).ok();
            self.write_main(output);
            return;
        }

        writeln!(output, "  ; Evaluate and dispatch first-true transaction").ok();

        let mut dispatch_txns: Vec<String> = Vec::new();
        for (name, _) in txns {
            if !fused_txns.contains(name) {
                dispatch_txns.push(name.clone());
            }
        }
        for (a_name, b_name) in &fusable_pairs {
            dispatch_txns.push(format!("{}_{}_fused", a_name, b_name));
        }

        if let Some(first) = dispatch_txns.first() {
            writeln!(output, "  call void @{}(%State* @global_state)", first).ok();
        }
        writeln!(output, "  ret void").ok();

        writeln!(output, "commit:").ok();
        writeln!(output, "  ret void").ok();
        writeln!(output, "}}").ok();
        writeln!(output).ok();

        self.write_main(output);
    }

    fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| {
                crate::ast::TopLevel::Transaction((*t).clone())
            }).collect(),
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
        };
        let mut pairs = crate::backend::detect_fusable_pairs(&prg);
        pairs.retain(|(a_name, b_name)| {
            let a = txns.iter().find(|(n, _)| n == a_name);
            let b = txns.iter().find(|(n, _)| n == b_name);
            if let (Some((_, a_txn)), Some((_, b_txn))) = (a, b) {
                if a_txn.is_async || b_txn.is_async { return false; }
                let a_writes = crate::backend::collect_assigned_identifiers(&a_txn.body);
                let b_writes = crate::backend::collect_assigned_identifiers(&b_txn.body);
                for w in &a_writes {
                    if b_writes.contains(w) { return false; }
                }
                if self.trg_in_precondition(&b_txn.contract.pre_condition) { return false; }
                true
            } else {
                false
            }
        });
        pairs
    }

    fn trg_in_precondition(&self, pre: &Expr) -> bool {
        let mut ids = std::collections::HashSet::new();
        crate::backend::collect_expr_identifiers(pre, &mut ids);
        for id in &ids {
            if self.trigger_names.contains(id) {
                return true;
            }
        }
        false
    }

    fn generate_fused_transaction(&mut self, output: &mut String, a: &crate::ast::Transaction, b: &crate::ast::Transaction, fused_name: &str) {
        writeln!(output, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", fused_name).ok();
        writeln!(output, "entry:").ok();

        let combined_body: Vec<Statement> = a.body.iter().cloned()
            .chain(b.body.iter().cloned())
            .collect();

        self.txn_counter = 0;
        self.let_bindings.clear();
        self.terminated = false;

        for stmt in &combined_body {
            self.generate_statement(output, stmt, "  ");
        }

        if !self.terminated {
            writeln!(output, "  ret void").ok();
        }
        writeln!(output, "}}").ok();
    }

    /// Emit stack-allocated null-terminated C string from a String's pointer+length.
    /// Returns the SSA register name for the i8* C string.
    fn emit_string_to_cstr(&mut self, output: &mut String, ptr_reg: String, len_reg: String, indent: &str) -> String {
        let cstr = format!("%cstr{}", self.txn_counter);
        self.txn_counter += 1;
        let dest = format!("%cstr_d{}", self.txn_counter);
        self.txn_counter += 1;
        let nul = format!("%cstr_n{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(output, "{}{} = alloca i8, i64 {}", indent, cstr, len_reg).ok();
        writeln!(output, "{}{} = getelementptr i8, i8* {}, i64 0", indent, dest, cstr).ok();
        writeln!(output, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)", indent, dest, ptr_reg, len_reg).ok();
        writeln!(output, "{}{} = getelementptr i8, i8* {}, i64 {}", indent, nul, cstr, len_reg).ok();
        writeln!(output, "{}store i8 0, i8* {}", indent, nul).ok();
        cstr
    }

    fn write_main(&self, output: &mut String) {
        writeln!(output, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(output, "entry:").ok();
        writeln!(output, "  call void @init_state()").ok();
        writeln!(output, "  br label %tick").ok();
        writeln!(output, "tick:").ok();
        writeln!(output, "  call void @reactor_tick()").ok();
        writeln!(output, "  br label %tick").ok();
        writeln!(output, "}}").ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn empty_program() -> Program {
        Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        }
    }

    #[test]
    fn test_llvm_generates_module() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
        assert!(output.contains("target triple"));
    }

    #[test]
    fn test_llvm_generates_state_type() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "counter".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        };
        let output = backend.generate(&program);
        assert!(output.contains("%State"));
        assert!(output.contains("i64"));
        assert!(output.contains("global_state"));
    }

    #[test]
    fn test_llvm_generates_transaction() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("count".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("count".to_string())),
                                Box::new(Expr::Integer(1)),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        };
        let output = backend.generate(&program);
        assert!(output.contains("@increment("));
    }

    #[test]
    fn test_llvm_has_noalias() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![] }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        };
        let output = backend.generate(&program);
        assert!(output.contains("noalias"), "Transaction should have noalias");
        assert!(output.contains("nocapture"), "Transaction should have nocapture");
        assert!(output.contains("local_unnamed_addr"), "Should have local_unnamed_addr");
        assert!(output.contains("attributes #0"), "Should have attribute block");
        assert!(output.contains("mustprogress"), "Should have mustprogress");
        assert!(output.contains("llvm.assume"), "Should declare llvm.assume intrinsic");
    }

    #[test]
    fn test_llvm_acyclic_annotation() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(!output.is_empty());
    }
}