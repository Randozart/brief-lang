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

use crate::ast::{Expr, HardwareConfig, Program, Statement, TopLevel};
use crate::errors::{Diagnostic, Severity};
use crate::target_spec::TargetSpec;
use std::collections::HashSet;

pub struct HardwareValidator;

impl HardwareValidator {
    pub fn validate(
        program: &Program,
        hw_config: Option<&HardwareConfig>,
        _target: &str,
        is_ebv: bool,
        target_spec: Option<&TargetSpec>,
    ) -> Vec<Diagnostic> {
        let write_graph = WriteGraph::build(program);
        let trigger_graph = TriggerGraph::build(program);
        let read_graph = ReadGraph::build(program);

        let mut diagnostics = Vec::new();

        diagnostics.extend(Self::check_orphan_variables(
            program,
            hw_config,
            &write_graph,
            &trigger_graph,
            is_ebv,
        ));
        diagnostics.extend(Self::check_untriggerable_transactions(
            program,
            &write_graph,
            &trigger_graph,
            is_ebv,
        ));
        diagnostics.extend(Self::check_unused_variables(
            program,
            hw_config,
            &read_graph,
        ));

        if let Some(spec) = target_spec {
            diagnostics.extend(Self::check_memory_overlaps(program, hw_config, spec));
        }

        diagnostics
    }

    fn check_memory_overlaps(
        program: &Program,
        _hw_config: Option<&HardwareConfig>,
        spec: &TargetSpec,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut occupied_regions: Vec<(u64, u64, String)> = Vec::new();

        // 1. Collect sections from target spec
        if let Some(memory) = &spec.memory {
            for (name, section) in &memory.sections {
                if let Some(max_size) = section.max_size {
                    occupied_regions.push((section.at, section.at + max_size, format!("Section '{}'", name)));
                }
            }
            
            // 2. Check memory banks bounds
            for item in &program.items {
                if let TopLevel::StateDecl(decl) = item {
                    if let Some(addr) = decl.address {
                        let mut found_bank = false;
                        for (bank_name, bank) in &memory.banks {
                            if addr >= bank.start && addr < bank.start + bank.size {
                                found_bank = true;
                                break;
                            }
                        }
                        if !found_bank && !memory.banks.is_empty() {
                            let mut diag = Diagnostic::new(
                                "B4006",
                                Severity::Error,
                                &format!("Address 0x{:X} for '{}' is outside any defined memory bank", addr, decl.name),
                            );
                            if let Some(span) = decl.span {
                                diag = diag.with_span(span);
                            }
                            diagnostics.push(diag);
                        }
                    }
                }
            }
        }

        // 3. Check for overlaps between defined sections
        for i in 0..occupied_regions.len() {
            for j in i + 1..occupied_regions.len() {
                let (s1, e1, n1) = &occupied_regions[i];
                let (s2, e2, n2) = &occupied_regions[j];
                
                if s1 < e2 && s2 < e1 {
                    diagnostics.push(Diagnostic::new(
                        "B4007",
                        Severity::Error,
                        &format!("Memory overlap detected between {} and {}", n1, n2),
                    ).with_explanation(&format!(
                        "Region 1: 0x{:X}-0x{:X}, Region 2: 0x{:X}-0x{:X}",
                        s1, e1, s2, e2
                    )));
                }
            }
        }

        diagnostics
    }

    fn check_orphan_variables(
        program: &Program,
        hw_config: Option<&HardwareConfig>,
        write_graph: &WriteGraph,
        trigger_graph: &TriggerGraph,
        is_ebv: bool,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                // Skip if it's an output-only signal in hardware.toml
                if let Some(cfg) = hw_config {
                    if let Some(addr) = decl.address {
                        if let Some(io_cfg) = Self::get_io_mapping(cfg, addr) {
                            if io_cfg.direction.as_deref() == Some("output") {
                                // However, if it's in [memory], it's internal storage
                                // and MUST be written to by something internal.
                                if !Self::has_memory_mapping(cfg, addr) {
                                    continue; // Pure output pin, doesn't need to be written by transactions
                                }
                            }
                        }
                    }
                }

                // If it has an initial value, it's considered "written".
                if decl.expr.is_none()
                    && !write_graph.writes_to(&decl.name)
                    && !trigger_graph.can_set(&decl.name)
                {
                    let severity = if is_ebv {
                        Severity::Error
                    } else {
                        Severity::Warning
                    };
                    let mut diag = Diagnostic::new(
                        "EBV001",
                        severity,
                        &format!("Variable '{}' is never written", decl.name),
                    );
                    if let Some(span) = decl.span {
                        diag = diag.with_span(span);
                    }
                    diag = diag.with_explanation("This variable will be optimized to constant 0 by synthesis tools because it is never updated by any transaction or trigger.");
                    diag = diag.with_hint(&format!(
                        "Add a transaction that writes to '{}', or remove the declaration.",
                        decl.name
                    ));
                    diagnostics.push(diag);
                }
            }
        }
        diagnostics
    }

    fn check_untriggerable_transactions(
        program: &Program,
        write_graph: &WriteGraph,
        trigger_graph: &TriggerGraph,
        is_ebv: bool,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                // Precondition 'true' is always triggerable.
                if let Expr::Bool(true) = txn.contract.pre_condition {
                    continue;
                }

                let deps = txn.contract.pre_condition.extract_dependencies();
                let mut can_be_satisfied = false;

                if deps.is_empty() {
                    can_be_satisfied = true;
                } else {
                    for dep in deps {
                        if write_graph.writes_to(&dep) || trigger_graph.can_set(&dep) {
                            can_be_satisfied = true;
                            break;
                        }
                    }
                }

                if !can_be_satisfied {
                    let severity = if is_ebv {
                        Severity::Error
                    } else {
                        Severity::Warning
                    };
                    let mut diag = Diagnostic::new(
                        "EBV002",
                        severity,
                        &format!("Transaction '{}' can never be triggered", txn.name),
                    );
                    if let Some(span) = txn.span {
                        diag = diag.with_span(span);
                    }
                    diag = diag.with_explanation(&format!(
                        "Transaction '{}' has a precondition that depends on variables that are never updated.",
                        txn.name
                    ));
                    diag = diag.with_hint("Add a trigger (trg) or another transaction that updates the variables used in this precondition.");
                    diagnostics.push(diag);
                }
            }
        }
        diagnostics
    }

    fn check_unused_variables(
        program: &Program,
        hw_config: Option<&HardwareConfig>,
        read_graph: &ReadGraph,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                // Skip if it's an output pin in hardware.toml (reading is done by external world)
                if let Some(cfg) = hw_config {
                    if let Some(addr) = decl.address {
                        if let Some(io_cfg) = Self::get_io_mapping(cfg, addr) {
                            if io_cfg.direction.as_deref() == Some("output") {
                                continue;
                            }
                        }
                    }
                }

                if !read_graph.reads_from(&decl.name) {
                    let mut diag = Diagnostic::new(
                        "EBV003",
                        Severity::Warning,
                        &format!("Variable '{}' is never used", decl.name),
                    );
                    if let Some(span) = decl.span {
                        diag = diag.with_span(span);
                    }
                    diag = diag.with_explanation("This variable is declared but its value is never read in any transaction or computation.");
                    diagnostics.push(diag);
                }
            }
        }
        diagnostics
    }

    fn get_io_mapping(cfg: &HardwareConfig, address: u64) -> Option<&crate::ast::IoMapping> {
        let addr_str_upper = format!("0x{:08X}", address);
        let addr_str_lower = format!("0x{:08x}", address);
        let addr_str_hex_upper = format!("0x{:X}", address);
        let addr_str_hex_lower = format!("0x{:x}", address);

        cfg.io.as_ref().and_then(|io| {
            io.get(&addr_str_upper)
                .or_else(|| io.get(&addr_str_lower))
                .or_else(|| io.get(&addr_str_hex_upper))
                .or_else(|| io.get(&addr_str_hex_lower))
        })
    }

    fn has_memory_mapping(cfg: &HardwareConfig, address: u64) -> bool {
        let addr_str_upper = format!("0x{:08X}", address);
        let addr_str_lower = format!("0x{:08x}", address);
        let addr_str_hex_upper = format!("0x{:X}", address);
        let addr_str_hex_lower = format!("0x{:x}", address);

        cfg.memory.contains_key(&addr_str_upper)
            || cfg.memory.contains_key(&addr_str_lower)
            || cfg.memory.contains_key(&addr_str_hex_upper)
            || cfg.memory.contains_key(&addr_str_hex_lower)
    }
}

struct WriteGraph {
    writers: HashSet<String>,
}

impl WriteGraph {
    fn build(program: &Program) -> Self {
        let mut writers = HashSet::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                Self::collect_writes(&txn.body, &mut writers);
            }
        }
        WriteGraph { writers }
    }

    fn collect_writes(statements: &[Statement], written: &mut HashSet<String>) {
        for stmt in statements {
            match stmt {
                Statement::Assignment { lhs, .. } => {
                    if let Some(name) = Self::extract_variable_name(lhs) {
                        written.insert(name);
                    }
                }
                Statement::Guarded { statements, .. } => {
                    Self::collect_writes(statements, written);
                }
                _ => {}
            }
        }
    }

    fn extract_variable_name(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name) => Some(name.clone()),
            Expr::OwnedRef(name) => Some(name.clone()),
            Expr::ListIndex(list, _) => Self::extract_variable_name(list),
            _ => None,
        }
    }

    fn writes_to(&self, var: &str) -> bool {
        self.writers.contains(var)
    }
}

struct TriggerGraph {
    settable: HashSet<String>,
}

impl TriggerGraph {
    fn build(program: &Program) -> Self {
        let mut settable = HashSet::new();
        for item in &program.items {
            if let TopLevel::Trigger(trg) = item {
                settable.insert(trg.name.clone());
            }
        }
        TriggerGraph { settable }
    }

    fn can_set(&self, var: &str) -> bool {
        self.settable.contains(var)
    }
}

struct ReadGraph {
    reads: HashSet<String>,
}

impl ReadGraph {
    fn build(program: &Program) -> Self {
        let mut reads = HashSet::new();
        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    reads.extend(txn.contract.pre_condition.extract_dependencies());
                    reads.extend(txn.contract.post_condition.extract_dependencies());
                    Self::collect_reads_stmts(&txn.body, &mut reads);
                }
                TopLevel::Definition(defn) => {
                    Self::collect_reads_stmts(&defn.body, &mut reads);
                }
                _ => {}
            }
        }
        ReadGraph { reads }
    }

    fn collect_reads_stmts(statements: &[Statement], read: &mut HashSet<String>) {
        for stmt in statements {
            match stmt {
                Statement::Assignment { expr, .. } => {
                    read.extend(expr.extract_dependencies());
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    read.extend(condition.extract_dependencies());
                    Self::collect_reads_stmts(statements, read);
                }
                Statement::Term(exprs) => {
                    for opt_expr in exprs {
                        if let Some(expr) = opt_expr {
                            read.extend(expr.extract_dependencies());
                        }
                    }
                }
                Statement::Escape(Some(expr)) => {
                    read.extend(expr.extract_dependencies());
                }
                Statement::Expression(expr) => {
                    read.extend(expr.extract_dependencies());
                }
                _ => {}
            }
        }
    }

    fn reads_from(&self, var: &str) -> bool {
        self.reads.contains(var)
    }
}
