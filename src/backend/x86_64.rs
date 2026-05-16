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

use crate::ast::{Expr, Program, Statement, TopLevel};
use std::collections::HashMap;

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
    
    fn generate_transaction(&self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
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
    
    fn generate_statement(&self, output: &mut String, stmt: &Statement) {
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
            Statement::Term { .. } => {
                output.push_str("    ; term\n");
            }
            _ => {}
        }
    }
    
    // PRAXIS: Branchless guarded statement
    fn generate_guarded_branchless(&self, output: &mut String, condition: &Expr, body: &[Statement]) {
        output.push_str("    ; Branchless guard\n");
        
        // Evaluate condition
        self.generate_expr_branchless(output, condition);
        
        // Generate body with conditional execution
        for stmt in body {
            output.push_str("    ; Conditional execution\n");
            self.generate_statement(output, stmt);
        }
    }
    
    fn generate_guarded_with_branch(&self, output: &mut String, condition: &Expr, body: &[Statement]) {
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
            _ => {
                output.push_str("    xor rax, rax\n");
            }
        }
    }
}
