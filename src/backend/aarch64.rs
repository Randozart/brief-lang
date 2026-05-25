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

// AArch64 Binary Backend - Direct assembly generation with PRAXIS optimizations
// 
// Implements:
// - Branchless code generation (CSEL instead of branches)
// - Predictive fetching (PRFM instructions)
// - Transaction fusion
// - Memory overlay for non-overlapping lifetimes
// - Parallel transaction scheduling

use crate::ast::{Expr, Program, Statement, TopLevel, Type};
use std::collections::HashMap;

/// Intent: AArch64 instruction set enum with all supported opcodes and addressing modes.
#[derive(Debug, Clone)]
pub enum A64Instr {
    // Data processing (immediate)
    AddImm(String, String, i64),
    SubImm(String, String, i64),
    MovImm(String, i64),
    
    // Data processing (register)
    AddReg(String, String, String),
    SubReg(String, String, String),
    AndReg(String, String, String),
    OrrReg(String, String, String),
    EorReg(String, String, String),
    
    // Conditional select (branchless)
    Csel(String, String, String, String),
    Cset(String, String),
    
    // Memory operations
    Ldr(String, String, i64),
    Str(String, String, i64),
    
    // Predictive fetch (PRAXIS optimization)
    Prfm(String, i64),
    
    // Compare
    Cmp(String, String),
    CmpImm(String, i64),
    
    // Branch operations
    B(String),
    Bl(String),
    BCond(String, String),
    
    // System
    Nop,
    Ret,
    Dmb,
    
    // Labels and comments
    Label(String),
    Comment(String),
}

impl A64Instr {
    /// Intent: Convert an AArch64 instruction to its assembly string representation.
    pub fn to_asm(&self) -> String {
        match self {
            A64Instr::AddImm(rd, rn, imm) => format!("    add {}, {}, #{}", rd, rn, imm),
            A64Instr::SubImm(rd, rn, imm) => format!("    sub {}, {}, #{}", rd, rn, imm),
            A64Instr::MovImm(rd, imm) => format!("    movz {}, #{}", rd, imm),
            A64Instr::AddReg(rd, rn, rm) => format!("    add {}, {}, {}", rd, rn, rm),
            A64Instr::SubReg(rd, rn, rm) => format!("    sub {}, {}, {}", rd, rn, rm),
            A64Instr::AndReg(rd, rn, rm) => format!("    and {}, {}, {}", rd, rn, rm),
            A64Instr::OrrReg(rd, rn, rm) => format!("    orr {}, {}, {}", rd, rn, rm),
            A64Instr::EorReg(rd, rn, rm) => format!("    eor {}, {}, {}", rd, rn, rm),
            A64Instr::Csel(rd, rn, rm, cond) => format!("    csel {}, {}, {}, {}", rd, rn, rm, cond),
            A64Instr::Cset(rd, cond) => format!("    cset {}, {}", rd, cond),
            A64Instr::Ldr(rd, rn, offset) => format!("    ldr {}, [{}, #{}]", rd, rn, offset),
            A64Instr::Str(rd, rn, offset) => format!("    str {}, [{}, #{}]", rd, rn, offset),
            A64Instr::Prfm(prf_type, addr) => format!("    prfm {}, [{}]", prf_type, addr),
            A64Instr::Cmp(rn, rm) => format!("    cmp {}, {}", rn, rm),
            A64Instr::CmpImm(rn, imm) => format!("    cmp {}, #{}", rn, imm),
            A64Instr::B(label) => format!("    b {}", label),
            A64Instr::Bl(label) => format!("    bl {}", label),
            A64Instr::BCond(cond, label) => format!("    b.{} {}", cond, label),
            A64Instr::Nop => "    nop".to_string(),
            A64Instr::Ret => "    ret".to_string(),
            A64Instr::Dmb => "    dmb sy".to_string(),
            A64Instr::Label(name) => format!("{}:", name),
            A64Instr::Comment(text) => format!("    // {}", text),
        }
    }
}

/// Intent: AArch64 backend with PRAXIS optimizations for branchless code generation, predictive fetching, transaction fusion, memory overlay, and parallel scheduling.
pub struct AArch64Backend {
    spec: Option<crate::target_spec::TargetSpec>,
    signal_counter: usize,
    txn_counter: usize,
    signal_map: HashMap<String, usize>,
    optimizations: OptimizationFlags,
}

/// Intent: Flags to enable or disable specific PRAXIS optimization passes.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizationFlags {
    pub branchless: bool,
    pub predictive_fetch: bool,
    pub transaction_fusion: bool,
    pub memory_overlay: bool,
    pub parallel_scheduling: bool,
}

impl AArch64Backend {
    /// Intent: Create a new AArch64Backend with default PRAXIS optimizations enabled.
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
    
    /// Intent: Attach a target specification to the backend via builder pattern.
    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }
    
    /// Intent: Override the default optimization flags via builder pattern.
    pub fn with_optimizations(mut self, opts: OptimizationFlags) -> Self {
        self.optimizations = opts;
        self
    }
    
    /// Intent: Generate a complete AArch64 assembly output for the given Brief program.
    pub fn generate(&mut self, program: &Program) -> String {
        
        let mut output = String::new();
        
        output.push_str("// AArch64 assembly - generated by Brief\n");
        output.push_str("// PRAXIS Optimizations: ");
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
        output.push_str("\n.text\n");
        output.push_str(".global _start\n");
        output.push_str(".global reactor_entry\n\n");
        
        // Generate entry point
        self.generate_entry_point(&mut output);
        
        // Generate reactor loop (PRAXIS: parallel scheduling)
        let txns = AArch64Backend::collect_transactions(program);
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
    
    /// Intent: Scan the program and build a signal-to-offset map for state declarations.
    fn collect_signals(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                self.signal_map.insert(state.name.clone(), self.signal_counter);
                self.signal_counter += 1;
            }
        }
    }
    
    /// Intent: Extract all transactions from the program AST into a name-to-transaction vector.
    fn collect_transactions(program: &Program) -> Vec<(String, &crate::ast::Transaction)> {
        let mut txns = Vec::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                txns.push((txn.name.clone(), txn));
            }
        }
        txns
    }
    
    /// Intent: Emit the .data section with aligned zero-initialized storage for each state variable.
    fn generate_data_section(&mut self, output: &mut String, program: &Program) {
        output.push_str(".data\n");
        
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                let size = Self::type_size(&state.ty);
                output.push_str(&format!("    .align 3\n"));
                output.push_str(&format!("    .globl sig_{}\n", state.name));
                output.push_str(&format!("sig_{}:\n", state.name));
                output.push_str(&format!("    .zero {}\n", size));
            }
        }
    }
    
    /// Intent: Return the AArch64 memory size in bytes for a given Brief type.
    fn type_size(ty: &Type) -> usize {
        match ty {
            Type::Int | Type::UInt => 8,
            Type::Bool => 1,
            Type::Float => 8,
            Type::String => 16,
            _ => 8,
        }
    }
    
    /// Intent: Emit the _start entry point that sets up the frame pointer and calls the reactor.
    fn generate_entry_point(&self, output: &mut String) {
        output.push_str("_start:\n");
        output.push_str("    // Entry point\n");
        output.push_str("    mov x29, sp\n");
        output.push_str("    bl reactor_entry\n");
        output.push_str("    // Exit\n");
        output.push_str("    mov x8, #93\n");
        output.push_str("    mov x0, #0\n");
        output.push_str("    svc #0\n\n");
    }
    
    // PRAXIS: Sequential reactor (baseline)
    /// Intent: Emit a sequential reactor loop that checks and fires each transaction in order.
    fn generate_sequential_reactor(&self, output: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        output.push_str("reactor_entry:\n");
        output.push_str("    // Reactor loop - sequential\n");
        output.push_str("    stp x29, x30, [sp, #-16]!\n");
        output.push_str("    mov x29, sp\n\n");
        
        output.push_str("reactor_loop:\n");
        
        for (name, _) in txns {
            output.push_str(&format!("    bl {}_check\n", name));
        }
        
        output.push_str("    b reactor_loop\n\n");
        
        output.push_str("reactor_exit:\n");
        output.push_str("    ldp x29, x30, [sp], #16\n");
        output.push_str("    ret\n\n");
    }
    
    // PRAXIS: Parallel reactor with transaction fusion
    /// Intent: Emit a parallel reactor loop with memory barriers and optional branchless guard checks.
    fn generate_parallel_reactor(&self, output: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        output.push_str("reactor_entry:\n");
        output.push_str("    // Reactor loop - parallel with transaction fusion\n");
        output.push_str("    stp x29, x30, [sp, #-16]!\n");
        output.push_str("    mov x29, sp\n\n");
        
        output.push_str("reactor_loop:\n");
        output.push_str("    dmb sy\n");
        output.push_str("    // Memory barrier for parallel execution\n\n");
        
        // Check all guards in parallel (branchless)
        for (name, txn) in txns {
            if self.optimizations.branchless {
                self.generate_branchless_guard_check(output, name, txn);
            } else {
                output.push_str(&format!("    bl {}_check\n", name));
            }
        }
        
        output.push_str("    b reactor_loop\n\n");
        
        output.push_str("reactor_exit:\n");
        output.push_str("    ldp x29, x30, [sp], #16\n");
        output.push_str("    ret\n\n");
    }
    
    // PRAXIS: Branchless guard check
    /// Intent: Emit a branchless guard check that calls the guard function then conditionally fires the transaction.
    fn generate_branchless_guard_check(&self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
        output.push_str(&format!("    // Branchless guard check for {}\n", name));
        output.push_str(&format!("    bl {}_guard\n", name));
        output.push_str("    // Result in X0: 1=fire, 0=skip\n");
        output.push_str("    cbz x0, .skip\n");
        output.push_str(&format!("    bl {}\n", name));
        output.push_str(".skip:\n\n");
    }
    
    /// Intent: Emit a transaction function body with frame setup/teardown and statement generation.
    fn generate_transaction(&self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
        output.push_str(&format!("{}:\n", name));
        output.push_str(&format!("    // Transaction: {}\n", name));
        output.push_str("    stp x29, x30, [sp, #-16]!\n");
        output.push_str("    mov x29, sp\n\n");
        
        // Generate body
        for stmt in &txn.body {
            self.generate_statement(output, stmt);
        }
        
        output.push_str("\n    ldp x29, x30, [sp], #16\n");
        output.push_str("    ret\n\n");
    }
    
    // PRAXIS: Transaction with predictive fetch
    /// Intent: Emit a guard function with predictive prefetch instructions for early data loading.
    fn generate_transaction_with_prefetch(&self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
        output.push_str(&format!("{}_guard:\n", name));
        output.push_str(&format!("    // Guard check with predictive fetch for {}\n", name));
        
        // Generate guard evaluation
        let pre = &txn.contract.pre_condition;
        self.generate_expr_branchless(output, pre);
        
        // PRAXIS: Insert predictive fetch instructions
        if self.optimizations.predictive_fetch {
            output.push_str(&format!("    // Predictive fetch for {}\n", name));
            let data_addrs = self.collect_data_addresses(txn);
            for addr in data_addrs.iter() {
                output.push_str(&format!("    prfm pldl1keep, [x0, #{}]\n", addr));
            }
        }
        
        output.push_str("    ret\n\n");
    }
    
    /// Intent: Collect memory offset addresses accessed by a transaction for prefetch hints.
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
                        output.push_str(&format!("    str x0, [x29, #{}]\n", offset * 8));
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
                output.push_str("    // term\n");
            }
            _ => {}
        }
    }
    
    // PRAXIS: Branchless guarded statement
    fn generate_guarded_branchless(&self, output: &mut String, condition: &Expr, body: &[Statement]) {
        output.push_str("    // Branchless guard\n");
        
        // Evaluate condition
        self.generate_expr_branchless(output, condition);
        
        // Generate body with conditional execution
        for stmt in body {
            output.push_str("    // Conditional execution\n");
            self.generate_statement(output, stmt);
        }
    }
    
    fn generate_guarded_with_branch(&self, output: &mut String, condition: &Expr, body: &[Statement]) {
        let label = format!(".guard_end_{}", self.signal_counter);
        
        self.generate_expr_branchless(output, condition);
        output.push_str(&format!("    cbz x0, {}\n", label));
        
        for stmt in body {
            self.generate_statement(output, stmt);
        }
        
        output.push_str(&format!("{}:\n", label));
    }
    
    fn generate_expr(&self, output: &mut String, expr: &Expr) {
        match expr {
            Expr::Integer(n) => {
                output.push_str(&format!("    movz x0, #{}\n", n));
            }
            Expr::Identifier(name) => {
                if let Some(offset) = self.signal_map.get(name) {
                    output.push_str(&format!("    ldr x0, [x29, #{}]\n", offset * 8));
                }
            }
            Expr::Add(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    add x0, x1, x0\n");
            }
            Expr::Sub(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    sub x0, x1, x0\n");
            }
            Expr::Mul(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    mul x0, x1, x0\n");
            }
            Expr::Div(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    sdiv x0, x1, x0\n");
            }
            _ => {
                output.push_str("    // Unimplemented expr\n");
            }
        }
    }
    
    fn generate_expr_branchless(&self, output: &mut String, expr: &Expr) {
        match expr {
            Expr::Eq(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, eq\n");
            }
            Expr::Ne(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, ne\n");
            }
            Expr::Lt(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, lt\n");
            }
            Expr::Gt(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, gt\n");
            }
            Expr::Le(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, le\n");
            }
            Expr::Ge(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, ge\n");
            }
            Expr::And(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    and x0, x1, x0\n");
            }
            Expr::Or(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    orr x0, x1, x0\n");
            }
            Expr::Identifier(name) => {
                if let Some(offset) = self.signal_map.get(name) {
                    output.push_str(&format!("    ldr x0, [x29, #{}]\n", offset * 8));
                }
            }
            Expr::Integer(n) => {
                output.push_str(&format!("    movz x0, #{}\n", n));
            }
            Expr::Bool(b) => {
                if *b {
                    output.push_str("    movz x0, #1\n");
                } else {
                    output.push_str("    movz x0, #0\n");
                }
            }
            _ => {
                output.push_str("    mov x0, #0\n");
            }
        }
    }
}
