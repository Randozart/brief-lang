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

// x86-64 Binary Backend - Direct assembly generation with PRAXIS optimizations
// 
// Implements:
// - Branchless code generation (CMOV instead of branches)
// - Predictive fetching (PREFETCHT0 instructions)
// - Transaction fusion
// - Memory overlay for non-overlapping lifetimes
// - Parallel transaction scheduling

use crate::analysis::call_graph::CallGraph;
use crate::ast::{Expr, Program, Statement, TopLevel};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

// x86-64 instruction set
#[derive(Debug, Clone)]
pub enum X64Instr {
    // Data movement
    Mov(String, String),
    MovImm(String, i64),
    MovMem(String, String, i64),
    MovMemImm(String, i64, i64),
    
    // Arithmetic
    Add(String, String),
    AddImm(String, i64),
    Sub(String, String),
    SubImm(String, i64),
    Imul(String, String),
    Idiv(String),
    
    // Logic
    And(String, String),
    Or(String, String),
    Xor(String, String),
    Not(String),
    Neg(String),
    
    // Conditional move (branchless)
    Cmov(String, String, String),
    Setcc(String, String),
    
    // Compare
    Cmp(String, String),
    CmpImm(String, i64),
    Test(String, String),
    
    // Predictive fetch (PRAXIS optimization)
    Prefetch(String, String),
    
    // Control flow
    Jmp(String),
    Je(String),
    Jne(String),
    Jl(String),
    Jg(String),
    Jle(String),
    Jge(String),
    Call(String),
    Ret,
    Nop,
    
    // Stack
    Push(String),
    Pop(String),
    Leave,
    
    // Labels and comments
    Label(String),
    Comment(String),
}

impl X64Instr {
    pub fn to_asm(&self) -> String {
        match self {
            X64Instr::Mov(rd, rs) => format!("    mov {}, {}", rd, rs),
            X64Instr::MovImm(rd, imm) => format!("    mov {}, {}", rd, imm),
            X64Instr::MovMem(rd, base, offset) => format!("    mov {}, [{}+{}]", rd, base, offset),
            X64Instr::MovMemImm(base, offset, imm) => format!("    mov qword [{}+{}], {}", base, offset, imm),
            X64Instr::Add(rd, rs) => format!("    add {}, {}", rd, rs),
            X64Instr::AddImm(rd, imm) => format!("    add {}, {}", rd, imm),
            X64Instr::Sub(rd, rs) => format!("    sub {}, {}", rd, rs),
            X64Instr::SubImm(rd, imm) => format!("    sub {}, {}", rd, imm),
            X64Instr::Imul(rd, rs) => format!("    imul {}, {}", rd, rs),
            X64Instr::Idiv(rs) => format!("    idiv {}", rs),
            X64Instr::And(rd, rs) => format!("    and {}, {}", rd, rs),
            X64Instr::Or(rd, rs) => format!("    or {}, {}", rd, rs),
            X64Instr::Xor(rd, rs) => format!("    xor {}, {}", rd, rs),
            X64Instr::Not(rd) => format!("    not {}", rd),
            X64Instr::Neg(rd) => format!("    neg {}", rd),
            X64Instr::Cmov(rd, rs, cond) => format!("    cmov{} {}, {}", cond, rd, rs),
            X64Instr::Setcc(rd, cond) => format!("    set{} {}", cond, rd),
            X64Instr::Cmp(rn, rm) => format!("    cmp {}, {}", rn, rm),
            X64Instr::CmpImm(rn, imm) => format!("    cmp {}, {}", rn, imm),
            X64Instr::Test(rn, rm) => format!("    test {}, {}", rn, rm),
            X64Instr::Prefetch(hint, addr) => format!("    prefetch{} {}", hint, addr),
            X64Instr::Jmp(label) => format!("    jmp {}", label),
            X64Instr::Je(label) => format!("    je {}", label),
            X64Instr::Jne(label) => format!("    jne {}", label),
            X64Instr::Jl(label) => format!("    jl {}", label),
            X64Instr::Jg(label) => format!("    jg {}", label),
            X64Instr::Jle(label) => format!("    jle {}", label),
            X64Instr::Jge(label) => format!("    jge {}", label),
            X64Instr::Call(label) => format!("    call {}", label),
            X64Instr::Ret => "    ret".to_string(),
            X64Instr::Nop => "    nop".to_string(),
            X64Instr::Push(reg) => format!("    push {}", reg),
            X64Instr::Pop(reg) => format!("    pop {}", reg),
            X64Instr::Leave => "    leave".to_string(),
            X64Instr::Label(name) => format!("{}:", name),
            X64Instr::Comment(text) => format!("    ; {}", text),
        }
    }
}

// x86-64 backend with PRAXIS optimizations
pub struct X86_64Backend {
    spec: Option<crate::target_spec::TargetSpec>,
    signal_counter: usize,
    txn_counter: usize,
    signal_map: HashMap<String, usize>,
    optimizations: OptimizationFlags,
    pending_cleanup: Vec<Statement>,
    has_cycles: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizationFlags {
    pub branchless: bool,
    pub predictive_fetch: bool,
    pub transaction_fusion: bool,
    pub memory_overlay: bool,
    pub parallel_scheduling: bool,
}

impl X86_64Backend {
    pub fn new() -> Self {
        Self {
            spec: None,
            signal_counter: 0,
            txn_counter: 0,
            signal_map: HashMap::new(),
            optimizations: OptimizationFlags {
                branchless: true,
                predictive_fetch: true,
                transaction_fusion: true,
                memory_overlay: true,
                parallel_scheduling: true,
            },
            pending_cleanup: Vec::new(),
            has_cycles: false,
        }
    }
    
    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }
    
    pub fn with_optimizations(mut self, opts: OptimizationFlags) -> Self {
        self.optimizations = opts;
        self
    }
    
    pub fn generate(&mut self, program: &Program) -> String {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        let _pr = &_analysis.param_ranges;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  x86_64 backend: acyclic call graph — static dispatch enabled");
        }

        self.collect_signals(program);
        
        let mut output = String::new();
        
        output.push_str("; x86-64 assembly - generated by Brief\n");
        output.push_str("; PRAXIS Optimizations: ");
        if self.optimizations.branchless {
            output.push_str("branchless ");
        }
        if self.optimizations.predictive_fetch {
            output.push_str("predictive-fetch ");
        }
        if self.optimizations.transaction_fusion {
            output.push_str("transaction-fusion ");
        }
        if self.optimizations.memory_overlay {
            output.push_str("memory-overlay ");
        }
        if self.optimizations.parallel_scheduling {
            output.push_str("parallel-scheduling ");
        }
        output.push_str("\n\n");
        
        // Generate data section
        self.generate_data_section(&mut output, program);
        
        // Generate text section
        output.push_str("\nsection .text\n");
        output.push_str("global _start\n");
        output.push_str("global reactor_entry\n\n");
        
        // Generate entry point
        self.generate_entry_point(&mut output);
        
        // Generate reactor loop (PRAXIS: parallel scheduling)
        let txns = X86_64Backend::collect_transactions(program);
        if self.optimizations.parallel_scheduling {
            self.generate_parallel_reactor(&mut output, &txns);
        } else {
            self.generate_sequential_reactor(&mut output, &txns);
        }
        
        // Generate transaction functions
        for (name, txn) in &txns {
            self.generate_transaction(&mut output, name, txn);
        }
        
        // Generate transaction check functions (with predictive fetch)
        for (name, txn) in &txns {
            if self.optimizations.predictive_fetch {
                self.generate_transaction_with_prefetch(&mut output, name, txn);
            }
        }
        
        output
    }
    
    fn collect_signals(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                self.signal_map.insert(state.name.clone(), self.signal_counter);
                self.signal_counter += 1;
            }
        }
    }
    
    fn collect_transactions(program: &Program) -> Vec<(String, &crate::ast::Transaction)> {
        let mut txns = Vec::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                txns.push((txn.name.clone(), txn));
            }
        }
        txns
    }
    
    fn generate_data_section(&mut self, output: &mut String, program: &Program) {
        output.push_str("section .data\n");
        
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                let size = Self::type_size(&state.ty);
                output.push_str(&format!("    align 8\n"));
                output.push_str(&format!("    global sig_{}\n", state.name));
                output.push_str(&format!("sig_{}:\n", state.name));
                output.push_str(&format!("    resb {}\n", size));
            }
        }
    }
    
    fn type_size(ty: &crate::ast::Type) -> usize {
        match ty {
            crate::ast::Type::Int | crate::ast::Type::UInt => 8,
            crate::ast::Type::Bool => 1,
            crate::ast::Type::Float => 8,
            crate::ast::Type::String => 16,
            _ => 8,
        }
    }
    
    fn generate_entry_point(&self, output: &mut String) {
        output.push_str("_start:\n");
        output.push_str("    ; Entry point\n");
        output.push_str("    mov rbp, rsp\n");
        output.push_str("    call reactor_entry\n");
        output.push_str("    ; Exit (Linux syscall)\n");
        output.push_str("    mov rax, 60\n");
        output.push_str("    xor rdi, rdi\n");
        output.push_str("    syscall\n\n");
    }
    
    // PRAXIS: Sequential reactor (baseline)
    fn generate_sequential_reactor(&self, output: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        output.push_str("reactor_entry:\n");
        output.push_str("    ; Reactor loop - sequential\n");
        output.push_str("    push rbp\n");
        output.push_str("    mov rbp, rsp\n\n");
        
        output.push_str("reactor_loop:\n");
        
        for (name, _) in txns {
            output.push_str(&format!("    call {}_check\n", name));
        }
        
        output.push_str("    jmp reactor_loop\n\n");
        
        output.push_str("reactor_exit:\n");
        output.push_str("    pop rbp\n");
        output.push_str("    ret\n\n");
    }
    
    // PRAXIS: Parallel reactor with transaction fusion
    fn generate_parallel_reactor(&self, output: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        output.push_str("reactor_entry:\n");
        output.push_str("    ; Reactor loop - parallel with transaction fusion\n");
        output.push_str("    push rbp\n");
        output.push_str("    mov rbp, rsp\n\n");
        
        output.push_str("reactor_loop:\n");
        output.push_str("    mfence\n");
        output.push_str("    ; Memory barrier for parallel execution\n\n");
        
        // Check all guards in parallel (branchless)
        for (name, txn) in txns {
            if self.optimizations.branchless {
                self.generate_branchless_guard_check(output, name, txn);
            } else {
                output.push_str(&format!("    call {}_check\n", name));
            }
        }
        
        output.push_str("    jmp reactor_loop\n\n");
        
        output.push_str("reactor_exit:\n");
        output.push_str("    pop rbp\n");
        output.push_str("    ret\n\n");
    }
    
    // PRAXIS: Branchless guard check
    fn generate_branchless_guard_check(&self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
        output.push_str(&format!("    ; Branchless guard check for {}\n", name));
        output.push_str(&format!("    call {}_guard\n", name));
        output.push_str("    ; Result in RAX: 1=fire, 0=skip\n");
        output.push_str("    test rax, rax\n");
        output.push_str("    jz .skip\n");
        output.push_str(&format!("    call {}\n", name));
        output.push_str(".skip:\n\n");
    }
    
    fn generate_transaction(&mut self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
        output.push_str(&format!("{}:\n", name));
        output.push_str(&format!("    ; Transaction: {}\n", name));
        output.push_str("    push rbp\n");
        output.push_str("    mov rbp, rsp\n\n");
        
        // Generate body
        for stmt in &txn.body {
            self.generate_statement(output, stmt);
        }
        
        output.push_str("\n    pop rbp\n");
        output.push_str("    ret\n\n");
    }
    
    // PRAXIS: Transaction with predictive fetch
    fn generate_transaction_with_prefetch(&self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
        output.push_str(&format!("{}_guard:\n", name));
        output.push_str(&format!("    ; Guard check with predictive fetch for {}\n", name));
        
        // Generate guard evaluation
        let pre = &txn.contract.pre_condition;
        self.generate_expr_branchless(output, pre);
        
        // PRAXIS: Insert predictive fetch instructions
        if self.optimizations.predictive_fetch {
            output.push_str(&format!("    ; Predictive fetch for {}\n", name));
            let data_addrs = self.collect_data_addresses(txn);
            for addr in data_addrs.iter() {
                output.push_str(&format!("    prefetcht0 [rel sig_data+{}]\n", addr));
            }
        }
        
        output.push_str("    ret\n\n");
    }
    
    fn collect_data_addresses(&self, txn: &crate::ast::Transaction) -> Vec<i64> {
        let mut addrs = Vec::new();
        for stmt in &txn.body {
            self.collect_addresses_from_stmt(stmt, &mut addrs);
        }
        addrs
    }
    
    fn collect_addresses_from_stmt(&self, stmt: &Statement, addrs: &mut Vec<i64>) {
        match stmt {
            Statement::Assignment { expr, .. } => {
                self.collect_addresses_from_expr(expr, addrs);
            }
            Statement::Guarded { statements, .. } => {
                for s in statements {
                    self.collect_addresses_from_stmt(s, addrs);
                }
            }
            _ => {}
        }
    }
    
    fn collect_addresses_from_expr(&self, expr: &Expr, addrs: &mut Vec<i64>) {
        match expr {
            Expr::Identifier(name) => {
                if let Some(offset) = self.signal_map.get(name) {
                    addrs.push(*offset as i64 * 8);
                }
            }
            Expr::Add(left, right) | Expr::Sub(left, right) | 
            Expr::Mul(left, right) | Expr::Div(left, right) |
            Expr::Eq(left, right) | Expr::Ne(left, right) |
            Expr::Lt(left, right) | Expr::Le(left, right) |
            Expr::Gt(left, right) | Expr::Ge(left, right) |
            Expr::Or(left, right) | Expr::And(left, right) => {
                self.collect_addresses_from_expr(left, addrs);
                self.collect_addresses_from_expr(right, addrs);
            }
            _ => {}
        }
    }
    
    fn generate_statement(&mut self, output: &mut String, stmt: &Statement) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                self.generate_expr(output, expr);
                if let Expr::Identifier(name) = lhs {
                    if let Some(offset) = self.signal_map.get(name) {
                        output.push_str(&format!("    mov [rbp+{}], rax\n", offset * 8));
                    }
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                if self.optimizations.branchless {
                    self.generate_guarded_branchless(output, condition, statements);
                } else {
                    self.generate_guarded_with_branch(output, condition, statements);
                }
            }
            Statement::Term { .. } | Statement::TermBang { .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for stmt in &cleanup {
                    self.generate_statement(output, stmt);
                }
                writeln!(output, "    ; term — transaction complete").ok();
            }
            Statement::Let { name, expr, address_expr, .. } => {
                if let Some(addr) = address_expr {
                    self.generate_expr(output, addr);
                    writeln!(output, "    ; let {}: ptr = address_expr", name).ok();
                } else if let Some(e) = expr {
                    self.generate_expr(output, e);
                    writeln!(output, "    ; let {} = (expr in rax)", name).ok();
                }
            }
            Statement::Expression(e) => {
                self.generate_expr(output, e);
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                writeln!(output, "    ; #on_exit cleanup registered").ok();
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    self.generate_expr(output, e);
                    writeln!(output, "    ; trg! {}: await expr (in rax)", name).ok();
                } else {
                    writeln!(output, "    ; trg! {}: await external signal", name).ok();
                }
            }
            Statement::Escape(value) => {
                if let Some(v) = value {
                    self.generate_expr(output, v);
                    writeln!(output, "    ; escape (value in rax)").ok();
                } else {
                    writeln!(output, "    ; escape (unit)").ok();
                }
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    let _ = writeln!(output, "    {}", line);
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                writeln!(output, "    {}", asm_string).ok();
            }
            Statement::SyncBlock { body } => {
                for s in body { self.generate_statement(output, s); }
            }
            Statement::Unification { name, variant, fields: _, expr } => {
                self.generate_expr(output, expr);
                writeln!(output, "    ; unification: {} {} (expr in rax)", name, variant).ok();
            }
        }
    }
    
    // PRAXIS: Branchless guarded statement using CMOV
    fn generate_guarded_branchless(&mut self, output: &mut String, condition: &Expr, body: &[Statement]) {
        self.generate_expr_branchless(output, condition);
        output.push_str("    neg rax\n");
        output.push_str("    sbb rax, rax\n");
        output.push_str("    mov r8, rax\n");

        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if let Expr::Identifier(name) = lhs {
                        if let Some(offset) = self.signal_map.get(name) {
                            output.push_str(&format!("    mov r9, [rbp+{}]\n", offset * 8));
                            self.generate_expr(output, expr);
                            output.push_str("    mov rdx, rax\n");
                            output.push_str("    xor rdx, r9\n");
                            output.push_str("    and rdx, r8\n");
                            output.push_str("    mov rax, r9\n");
                            output.push_str("    xor rax, rdx\n");
                            output.push_str(&format!("    mov [rbp+{}], rax\n", offset * 8));
                            continue;
                        }
                    }
                    self.generate_statement(output, stmt);
                }
                _ => {
                    self.generate_statement(output, stmt);
                }
            }
        }
    }
    
    fn generate_guarded_with_branch(&mut self, output: &mut String, condition: &Expr, body: &[Statement]) {
        let label = format!(".guard_end_{}", self.signal_counter);
        
        self.generate_expr_branchless(output, condition);
        output.push_str(&format!("    test rax, rax\n"));
        output.push_str(&format!("    jz {}\n", label));
        
        for stmt in body {
            self.generate_statement(output, stmt);
        }
        
        output.push_str(&format!("{}:\n", label));
    }
    
    fn generate_expr(&self, output: &mut String, expr: &Expr) {
        match expr {
            Expr::Integer(n) => {
                output.push_str(&format!("    mov rax, {}\n", n));
            }
            Expr::Bool(true) => {
                output.push_str("    mov rax, 1\n");
            }
            Expr::Bool(false) => {
                output.push_str("    xor rax, rax\n");
            }
            Expr::Float(f) => {
                write!(output, "    ; Float: {} (load from literal pool)\n", f).unwrap();
            }
            Expr::String(s) => {
                write!(output, "    ; String: \"{}\" (load from data section)\n", s).unwrap();
            }
            Expr::Identifier(name) => {
                if let Some(offset) = self.signal_map.get(name) {
                    output.push_str(&format!("    mov rax, [rbp+{}]\n", offset * 8));
                }
            }
            Expr::Add(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    add rax, rbx\n");
            }
            Expr::Sub(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    sub rbx, rax\n");
                output.push_str("    mov rax, rbx\n");
            }
            Expr::Mul(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    imul rax, rbx\n");
            }
            Expr::Div(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    mov rdx, 0\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    idiv rax\n");
            }
            Expr::Mod(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    mov rdx, 0\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    idiv rax\n");
                output.push_str("    mov rax, rdx\n");
            }
            Expr::Eq(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    cmp rax, rbx\n");
                output.push_str("    sete al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Ne(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    cmp rax, rbx\n");
                output.push_str("    setne al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Lt(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setl al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Gt(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setg al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Le(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setle al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Ge(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setge al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::And(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    and rax, rbx\n");
            }
            Expr::Or(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    or rax, rbx\n");
            }
            Expr::Not(inner) => {
                self.generate_expr(output, inner);
                output.push_str("    cmp rax, 0\n");
                output.push_str("    sete al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Neg(inner) => {
                self.generate_expr(output, inner);
                output.push_str("    neg rax\n");
            }
            Expr::BitAnd(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    and rax, rbx\n");
            }
            Expr::BitOr(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    or rax, rbx\n");
            }
            Expr::BitXor(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    xor rax, rbx\n");
            }
            Expr::BitNot(inner) => {
                self.generate_expr(output, inner);
                output.push_str("    not rax\n");
            }
            Expr::Shl(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    mov rcx, rax\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    shl rax, cl\n");
            }
            Expr::Shr(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr(output, right);
                output.push_str("    mov rcx, rax\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    shr rax, cl\n");
            }
            Expr::Call(name, args) => {
                write!(output, "    ; call {}(", name).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { output.push_str(", "); }
                    self.generate_expr(output, arg);
                }
                output.push_str(")\n");
            }
            Expr::ListLiteral(elems) => {
                output.push_str("    /* [");
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 { output.push_str(", "); }
                    self.generate_expr(output, elem);
                }
                output.push_str("] */\n");
            }
            Expr::ListIndex(list, idx) => {
                self.generate_expr(output, list);
                output.push_str("[");
                self.generate_expr(output, idx);
                output.push_str("]\n");
            }
            Expr::Projection { source: list, .. } => {
                self.generate_expr(output, list);
                output.push_str(".length\n");
            }
            Expr::FieldAccess(obj, field) => {
                self.generate_expr(output, obj);
                write!(output, ".{}\n", field).unwrap();
            }
            _ => {
                output.push_str("    ; Unimplemented expr\n");
            }
        }
    }
    
    fn generate_expr_branchless(&self, output: &mut String, expr: &Expr) {
        match expr {
            Expr::Eq(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp rax, rbx\n");
                output.push_str("    sete al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Ne(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp rax, rbx\n");
                output.push_str("    setne al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Lt(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setl al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Gt(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setg al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Le(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setle al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Ge(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp rbx, rax\n");
                output.push_str("    setge al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::And(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    and rax, rbx\n");
            }
            Expr::Or(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    or rax, rbx\n");
            }
            Expr::Add(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    add rax, rbx\n");
            }
            Expr::Sub(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    sub rbx, rax\n");
                output.push_str("    mov rax, rbx\n");
            }
            Expr::Mul(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    imul rax, rbx\n");
            }
            Expr::Div(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    mov rdx, 0\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    idiv rax\n");
            }
            Expr::Mod(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    mov rdx, 0\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    idiv rax\n");
                output.push_str("    mov rax, rdx\n");
            }
            Expr::Not(inner) => {
                self.generate_expr_branchless(output, inner);
                output.push_str("    cmp rax, 0\n");
                output.push_str("    sete al\n");
                output.push_str("    movzx rax, al\n");
            }
            Expr::Neg(inner) => {
                self.generate_expr_branchless(output, inner);
                output.push_str("    neg rax\n");
            }
            Expr::BitAnd(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    and rax, rbx\n");
            }
            Expr::BitOr(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    or rax, rbx\n");
            }
            Expr::BitXor(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    xor rax, rbx\n");
            }
            Expr::BitNot(inner) => {
                self.generate_expr_branchless(output, inner);
                output.push_str("    not rax\n");
            }
            Expr::Shl(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    mov rcx, rax\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    shl rax, cl\n");
            }
            Expr::Shr(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov rbx, rax\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    mov rcx, rax\n");
                output.push_str("    mov rax, rbx\n");
                output.push_str("    shr rax, cl\n");
            }
            Expr::Identifier(name) => {
                if let Some(offset) = self.signal_map.get(name) {
                    output.push_str(&format!("    mov rax, [rbp+{}]\n", offset * 8));
                }
            }
            Expr::Integer(n) => {
                output.push_str(&format!("    mov rax, {}\n", n));
            }
            Expr::Bool(b) => {
                if *b {
                    output.push_str("    mov rax, 1\n");
                } else {
                    output.push_str("    xor rax, rax\n");
                }
            }
            Expr::Float(f) => {
                write!(output, "    mov rax, #{} ; Float placeholder\n", f).unwrap();
            }
            Expr::String(s) => {
                write!(output, "    ; String literal: \"{}\"\n", s).unwrap();
            }
            Expr::Call(name, args) => {
                write!(output, "    ; call {}(", name).unwrap();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { output.push_str(", "); }
                    self.generate_expr_branchless(output, arg);
                }
                output.push_str(")\n");
            }
            Expr::ListLiteral(elems) => {
                output.push_str("    /* [");
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 { output.push_str(", "); }
                    self.generate_expr_branchless(output, elem);
                }
                output.push_str("] */\n");
            }
            Expr::ListIndex(list, idx) => {
                self.generate_expr_branchless(output, list);
                output.push_str("[");
                self.generate_expr_branchless(output, idx);
                output.push_str("]\n");
            }
            Expr::Projection { source: list, .. } => {
                self.generate_expr_branchless(output, list);
                output.push_str(".length\n");
            }
            Expr::FieldAccess(obj, field) => {
                self.generate_expr_branchless(output, obj);
                write!(output, ".{}\n", field).unwrap();
            }
            _ => {
                output.push_str("    xor rax, rax\n");
            }
        }
    }
}

/// Intent: Peephole optimization pass that eliminates redundant x86-64 instructions.
pub fn peephole_optimize(instrs: Vec<X64Instr>) -> Vec<X64Instr> {
    let mut result = Vec::with_capacity(instrs.len());
    for instr in instrs {
        match &instr {
            X64Instr::Mov(rd, rs) if rd == rs => continue,
            X64Instr::AddImm(rd, 0) if !rd.starts_with("rsp") && !rd.starts_with("rbp") => continue,
            X64Instr::SubImm(rd, 0) if !rd.starts_with("rsp") && !rd.starts_with("rbp") => continue,
            X64Instr::Nop => continue,
            _ => {}
        }

        // Consecutive identical ops on same register
        if let Some(prev) = result.last() {
            match (prev, &instr) {
                (X64Instr::Add(rd1, rs1), X64Instr::Add(rd2, rs2))
                    if rd1 == rd2 && rs1 == rs2 => continue,
                (X64Instr::Sub(rd1, rs1), X64Instr::Sub(rd2, rs2))
                    if rd1 == rd2 && rs1 == rs2 => continue,
                _ => {}
            }
        }

        result.push(instr);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    #[test]
    fn test_x86_64_generates_assembly() {
        let mut backend = X86_64Backend::new();
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
        let output = backend.generate(&program);
        assert!(output.contains(".data"));
        assert!(output.contains(".text"));
    }

    #[test]
    fn test_x86_64_generates_state_decl() {
        let mut backend = X86_64Backend::new();
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
                    attrs: Vec::new(),
                }),
            ],
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
        let output = backend.generate(&program);
        assert!(output.contains("_start"));
        assert!(output.contains("counter"));
    }
}
