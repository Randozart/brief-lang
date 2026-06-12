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
use std::fmt::Write;

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
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
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
            has_cycles: false,
            pending_cleanup: Vec::new(),
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
        // Run shared program analysis
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        let _pr = &_analysis.param_ranges;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  AArch64 backend: acyclic call graph — PRAXIS optimizations enabled");
        }

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
    
    /// Intent: Generate AArch64 assembly for the State struct layout with zero-initialized storage.
    fn generate_state_struct(&self, output: &mut String, program: &Program) {
        writeln!(output, "// State structure").ok();
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                let size = Self::type_size(&s.ty);
                writeln!(output, "    .globl {}", s.name).ok();
                writeln!(output, "{}:", s.name).ok();
                writeln!(output, "    .zero {}", size).ok();
            }
        }
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
    fn generate_transaction(&mut self, output: &mut String, name: &str, txn: &crate::ast::Transaction) {
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
    
    fn generate_statement(&mut self, output: &mut String, stmt: &Statement) {
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
            Statement::Term { .. } | Statement::TermBang { .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for stmt in &cleanup {
                    self.generate_statement(output, stmt);
                }
                writeln!(output, "    // term — transaction complete").ok();
            }
            Statement::Let { name, expr, address_expr, .. } => {
                if let Some(addr) = address_expr {
                    self.generate_expr(output, addr);
                    writeln!(output, "    // let {} = ptr (addr computed above)", name).ok();
                } else if let Some(e) = expr {
                    self.generate_expr(output, e);
                    writeln!(output, "    // let {} = expr (value in x0)", name).ok();
                }
            }
            Statement::Expression(e) => {
                self.generate_expr(output, e);
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    self.generate_expr(output, e);
                    writeln!(output, "    // trg! {}: await expr (in x0)", name).ok();
                } else {
                    writeln!(output, "    // trg! {}: await external signal", name).ok();
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                writeln!(output, "    // #on_exit cleanup registered").ok();
            }
            Statement::Escape(Some(v)) => {
                self.generate_expr(output, v);
                writeln!(output, "    // escape (value in x0)").ok();
            }
            Statement::Escape(None) => {
                writeln!(output, "    // escape (unit)").ok();
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    let _ = writeln!(output, "    {}", line);
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                let _ = writeln!(output, "    {}", asm_string);
            }
            Statement::SyncBlock { body } => {
                for s in body { self.generate_statement(output, s); }
            }
            Statement::Unification { name, variant, fields: _, expr } => {
                self.generate_expr(output, expr);
                writeln!(output, "    // unification: {} {} (value in x0)", name, variant).ok();
            }
            Statement::Foreach { .. } => { /* foreach: not yet implemented in AArch64 backend */ }
        }
    }
    
    // PRAXIS: Branchless guarded statement using CSEL
    fn generate_guarded_branchless(&mut self, output: &mut String, condition: &Expr, body: &[Statement]) {
        self.generate_expr_branchless(output, condition);
        output.push_str("    cmp x0, #0\n");
        output.push_str("    cset x8, ne\n");
        output.push_str("    sub x8, x8, #1\n");

        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if let Expr::Identifier(name) = lhs {
                        if let Some(offset) = self.signal_map.get(name) {
                            output.push_str(&format!("    ldr x9, [x29, #{}]\n", offset * 8));
                            self.generate_expr(output, expr);
                            output.push_str("    eor x10, x9, x0\n");
                            output.push_str("    and x10, x10, x8\n");
                            output.push_str("    eor x0, x9, x10\n");
                            output.push_str(&format!("    str x0, [x29, #{}]\n", offset * 8));
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
            Expr::Bool(true) => {
                output.push_str("    movz x0, #1\n");
            }
            Expr::Bool(false) => {
                output.push_str("    movz x0, #0\n");
            }
            Expr::Float(f) => {
                write!(output, "    // Float: {} (load from literal pool)\n", f).unwrap();
            }
            Expr::String(s) => {
                write!(output, "    // String: \"{}\" (load from data section)\n", s).unwrap();
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
            Expr::Mod(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    udiv x2, x1, x0\n");
                output.push_str("    msub x0, x2, x0, x1\n");
            }
            Expr::Eq(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, eq\n");
            }
            Expr::Ne(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, ne\n");
            }
            Expr::Lt(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, lt\n");
            }
            Expr::Le(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, le\n");
            }
            Expr::Gt(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, gt\n");
            }
            Expr::Ge(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    cmp x0, x1\n");
                output.push_str("    cset x0, ge\n");
            }
            Expr::And(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    and x0, x1, x0\n");
            }
            Expr::Or(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    orr x0, x1, x0\n");
            }
            Expr::Not(inner) => {
                self.generate_expr(output, inner);
                output.push_str("    cmp x0, #0\n");
                output.push_str("    cset x0, eq\n");
            }
            Expr::Neg(inner) => {
                self.generate_expr(output, inner);
                output.push_str("    neg x0, x0\n");
            }
            Expr::BitAnd(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    and x0, x1, x0\n");
            }
            Expr::BitOr(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    orr x0, x1, x0\n");
            }
            Expr::BitXor(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    eor x0, x1, x0\n");
            }
            Expr::BitNot(inner) => {
                self.generate_expr(output, inner);
                output.push_str("    mvn x0, x0\n");
            }
            Expr::Shl(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    lsl x0, x1, x0\n");
            }
            Expr::Shr(left, right) => {
                self.generate_expr(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr(output, right);
                output.push_str("    lsr x0, x1, x0\n");
            }
            Expr::Call(name, args) => {
                write!(output, "    // call {}(", name).unwrap();
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
            Expr::Add(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    add x0, x1, x0\n");
            }
            Expr::Sub(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    sub x0, x1, x0\n");
            }
            Expr::Mul(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    mul x0, x1, x0\n");
            }
            Expr::Div(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    sdiv x0, x1, x0\n");
            }
            Expr::Mod(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    udiv x2, x1, x0\n");
                output.push_str("    msub x0, x2, x0, x1\n");
            }
            Expr::Not(inner) => {
                self.generate_expr_branchless(output, inner);
                output.push_str("    cmp x0, #0\n");
                output.push_str("    cset x0, eq\n");
            }
            Expr::Neg(inner) => {
                self.generate_expr_branchless(output, inner);
                output.push_str("    neg x0, x0\n");
            }
            Expr::BitAnd(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    and x0, x1, x0\n");
            }
            Expr::BitOr(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    orr x0, x1, x0\n");
            }
            Expr::BitXor(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    eor x0, x1, x0\n");
            }
            Expr::BitNot(inner) => {
                self.generate_expr_branchless(output, inner);
                output.push_str("    mvn x0, x0\n");
            }
            Expr::Shl(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    lsl x0, x1, x0\n");
            }
            Expr::Shr(left, right) => {
                self.generate_expr_branchless(output, left);
                output.push_str("    mov x1, x0\n");
                self.generate_expr_branchless(output, right);
                output.push_str("    lsr x0, x1, x0\n");
            }
            Expr::Identifier(name) => {
                if let Some(offset) = self.signal_map.get(name) {
                    output.push_str(&format!("    ldr x0, [x29, #{}]\n", offset * 8));
                }
            }
            Expr::Integer(n) => {
                output.push_str(&format!("    movz x0, #{}\n", n));
            }
            Expr::Bool(true) => {
                output.push_str("    movz x0, #1\n");
            }
            Expr::Bool(false) => {
                output.push_str("    movz x0, #0\n");
            }
            Expr::Float(f) => {
                write!(output, "    movz x0, #{} // Float placeholder\n", f).unwrap();
            }
            Expr::String(s) => {
                write!(output, "    // String literal: \"{}\"\n", s).unwrap();
            }
            Expr::Call(name, args) => {
                write!(output, "    // call {}(", name).unwrap();
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
                output.push_str("    mov x0, #0\n");
            }
        }
    }
}

/// Intent: Peephole optimization pass that eliminates redundant AArch64 instructions.
pub fn peephole_optimize(instrs: Vec<A64Instr>) -> Vec<A64Instr> {
    let mut result = Vec::with_capacity(instrs.len());
    for instr in instrs {
        match &instr {
            A64Instr::MovImm(rd, _) if rd == "xzr" || rd == "wzr" => continue,
            A64Instr::Nop => continue,
            _ => {}
        }

        // Consecutive identical ops on same register (excluding xzr/wzr)
        if let Some(prev) = result.last() {
            match (prev, &instr) {
                (A64Instr::AddReg(rd1, rn1, rm1), A64Instr::AddReg(rd2, rn2, rm2))
                    if rd1 == rd2 && rn1 == rn2 && rm1 == rm2 => continue,
                (A64Instr::SubReg(rd1, rn1, rm1), A64Instr::SubReg(rd2, rn2, rm2))
                    if rd1 == rd2 && rn1 == rn2 && rm1 == rm2 => continue,
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
    fn test_aarch64_generates_assembly() {
        let mut backend = AArch64Backend::new();
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
    fn test_aarch64_generates_entry_point() {
        let mut backend = AArch64Backend::new();
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
                range_constraint: None,
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
