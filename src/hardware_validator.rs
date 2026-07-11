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

use crate::ast::{Expr, HardwareConfig, Program, Statement, TopLevel, Type};
use crate::dbrief::DbvsEngine;
use crate::errors::{Diagnostic, Severity};
use crate::target_spec::TargetSpec;
use crate::typechecker::CompilationTarget;
use std::collections::HashSet;

pub struct HardwareValidator;

impl HardwareValidator {
    pub fn validate(
        program: &Program,
        hw_config: Option<&HardwareConfig>,
        _target: &str,
        comp_target: CompilationTarget,
        target_spec: Option<&TargetSpec>,
        dbvs_engine: Option<&DbvsEngine>,
    ) -> Vec<Diagnostic> {
        let is_embedded = comp_target == CompilationTarget::Embedded
            || comp_target == CompilationTarget::Circuit;
        let write_graph = WriteGraph::build(program);
        let trigger_graph = TriggerGraph::build(program);
        let read_graph = ReadGraph::build(program);

        let mut diagnostics = Vec::new();

        diagnostics.extend(Self::check_orphan_variables(
            program,
            hw_config,
            &write_graph,
            &trigger_graph,
            is_embedded,
        ));
        diagnostics.extend(Self::check_untriggerable_transactions(
            program,
            &write_graph,
            &trigger_graph,
            is_embedded,
        ));
        diagnostics.extend(Self::check_unused_variables(
            program,
            hw_config,
            &read_graph,
        ));

        if let Some(spec) = target_spec {
            diagnostics.extend(Self::check_memory_overlaps(program, hw_config, spec, dbvs_engine));
        }

        // .cbv / .ebv-specific checks (circuit/embedded tier)
        if is_embedded {
            diagnostics.extend(Self::check_hebv_restrictions(program));
        }

        diagnostics
    }

    fn check_hebv_restrictions(program: &Program) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for item in &program.items {
            match item {
                TopLevel::LinkDependency(_) => {
                    diagnostics.push(Diagnostic::new(
                        "B5001",
                        Severity::Error,
                        ".cbv does not allow 'import \"link/...\"' — no external dependencies",
                    ));
                }
                TopLevel::ForeignBinding { .. } => {
                    diagnostics.push(Diagnostic::new(
                        "B5002",
                        Severity::Error,
                        ".cbv does not allow 'frgn' declarations — pure logic graph only",
                    ));
                }
                TopLevel::Import(imp) => {
                    for item in &imp.items {
                        if let Some(path) = imp.path.first() {
                            if path == "link" {
                                diagnostics.push(Diagnostic::new(
                                    "B5003",
                                    Severity::Error,
                                    ".cbv cannot import from 'link/' — no external dependencies",
                                ));
                            }
                        }
                    }
                }
                TopLevel::Transaction(txn) => {
                    // Check total contracts (no [true] defaults)
                    if matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                        diagnostics.push(Diagnostic::new(
                            "B5004",
                            Severity::Error,
                            &format!(".cbv transaction '{}' has [true] precondition — must be total", txn.name),
                        ));
                    }
                    if matches!(txn.contract.post_condition, Expr::Bool(true)) {
                        diagnostics.push(Diagnostic::new(
                            "B5005",
                            Severity::Error,
                            &format!(".cbv transaction '{}' has [true] postcondition — must be total", txn.name),
                        ));
                    }
                    // Check for dynamic heap usage
                    Self::check_synthesizable_types(&txn.parameters, &mut diagnostics, &txn.name);
                }
                TopLevel::StateDecl(decl) => {
                    Self::check_type_synthesizable(&decl.ty, &mut diagnostics, &decl.name);
                }
                _ => {}
            }
        }
        diagnostics
    }

    fn check_synthesizable_types(params: &[(String, Type)], diagnostics: &mut Vec<Diagnostic>, txn_name: &str) {
        for (name, ty) in params {
            Self::check_type_synthesizable(ty, diagnostics, &format!("{}.{}", txn_name, name));
        }
    }

    fn check_type_synthesizable(ty: &Type, diagnostics: &mut Vec<Diagnostic>, context: &str) {
        match ty {
            Type::Custom(__t) if __t == "Int" || __t == "UInt" => {
                diagnostics.push(Diagnostic::new(
                    "B5006",
                    Severity::Error,
                    &format!(".cbv type '{}' uses Int/UInt (unsized) — use UInt[N] or SInt[N] for synthesizable logic", context),
                ));
            }
            Type::Custom(__t) if __t == "Float" => {
                diagnostics.push(Diagnostic::new(
                    "B5007",
                    Severity::Error,
                    &format!(".cbv type '{}' uses Float — not synthesizable", context),
                ));
            }
            Type::Custom(__t) if __t == "String" => {
                diagnostics.push(Diagnostic::new(
                    "B5008",
                    Severity::Error,
                    &format!(".cbv type '{}' uses String — not synthesizable", context),
                ));
            }
            Type::Custom(__t) if __t == "Bool" || __t == "Char" => {} // OK for hardware
            Type::Vector(inner, _) => Self::check_type_synthesizable(inner, diagnostics, context),
            Type::Tuple(types) => {
                for t in types {
                    Self::check_type_synthesizable(t, diagnostics, context);
                }
            }
            Type::Custom(_) | Type::Enum(_) => {
                // Struct/enum — assumed synthesizable if fields are
            }
            Type::Constrained(inner, _) => Self::check_type_synthesizable(inner, diagnostics, context),
            Type::Custom(__t) if __t == "Data" => {} // OK
            Type::Void => {} // OK
            _ => {}
        }
    }

    fn check_memory_overlaps(
        program: &Program,
        _hw_config: Option<&HardwareConfig>,
        spec: &TargetSpec,
        dbvs_engine: Option<&DbvsEngine>,
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

        // Use DbvsEngine to check for overflows and add to occupied regions
        if let Some(engine) = dbvs_engine {
            for item in &program.items {
                if let TopLevel::StateDecl(decl) = item {
                    if let Some(addr) = decl.address {
                        if let Some(alias) = engine.get_alias(&decl.name) {
                            let size = get_dbrief_type_size(&alias.alias_type, engine);
                            if size > 0 {
                                occupied_regions.push((addr, addr + size, format!("StateDecl '{}'", decl.name)));

                                // Check if this region fits within any memory bank
                                let mut found_bank = false;
                                if let Some(memory) = &spec.memory {
                                    for (_bank_name, bank) in &memory.banks {
                                        if addr >= bank.start && (addr + size) <= (bank.start + bank.size) {
                                            found_bank = true;
                                            break;
                                        }
                                    }
                                }
                                if !found_bank && spec.memory.as_ref().map_or(false, |m| !m.banks.is_empty()) {
                                    let mut diag = Diagnostic::new(
                                        "B4006",
                                        Severity::Error,
                                        &format!("Memory region for '{}' (0x{:X} - 0x{:X}) is outside any defined memory bank", decl.name, addr, addr + size),
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

    pub fn validate_schema_imports(
        program: &Program,
        source_file: &std::path::Path,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        
        let mut schema_files: Vec<(String, std::path::PathBuf)> = Vec::new();
        
        for item in &program.items {
            if let TopLevel::Import(import) = item {
                let path_str = import.path.join("/");
                if path_str.ends_with(".dbvs") {
                    if let Some(parent) = source_file.parent() {
                        schema_files.push((path_str.clone(), parent.join(&path_str)));
                    }
                }
            }
        }
        
        if schema_files.is_empty() {
            return diagnostics;
        }
        
        let mut schema_aliases: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        
        for (schema_path, full_path) in &schema_files {
            match std::fs::read_to_string(full_path) {
                Ok(content) => {
                    match crate::dbrief::parse_dbvs(&content) {
                        Ok(dbvs) => {
                            for alias in &dbvs.aliases {
                                schema_aliases.insert(alias.name.clone(), schema_path.clone());
                            }
                        }
                        Err(e) => {
                            diagnostics.push(Diagnostic {
                                code: "HW005".to_string(),
                                title: "Failed to parse schema".to_string(),
                                explanation: vec![format!("Failed to parse {}: {}", schema_path, e)],
                                severity: Severity::Error,
                                span: None,
                                source_snippet: None,
                                proof_chain: Vec::new(),
                                examples: Vec::new(),
                                hints: Vec::new(),
                                notes: Vec::new(),
                            });
                        }
                    }
                }
                Err(e) => {
                    diagnostics.push(Diagnostic {
                        code: "HW006".to_string(),
                        title: "Schema file not found".to_string(),
                        explanation: vec![format!("Cannot read {}: {}", schema_path, e)],
                        severity: Severity::Error,
                        span: None,
                        source_snippet: None,
                        proof_chain: Vec::new(),
                        examples: Vec::new(),
                        hints: Vec::new(),
                        notes: Vec::new(),
                    });
                }
            }
        }
        
        for item in &program.items {
            match item {
                TopLevel::StateDecl(state) => {
                    let is_internal = state.attrs.iter().any(|a| a.key == "internal");
                    if !schema_aliases.contains_key(&state.name) && !state.name.starts_with("_") && !is_internal {
                        // Check if it's a Definition (pure function) - those don't need to be in schema
                        let is_definition = program.items.iter().any(|i| {
                            match i {
                                TopLevel::Definition(d) => d.name == state.name,
                                _ => false,
                            }
                        });
                        
                        // Only report error if schemas were loaded and state is not a definition
                        if !is_definition && !schema_aliases.is_empty() {
                            diagnostics.push(Diagnostic {
                                code: "HW007".to_string(),
                                title: "Undefined alias reference".to_string(),
                                explanation: vec![format!(
                                    "State '{}' is not declared in any imported schema. Import schema or provide via --hw config.dbv",
                                    state.name
                                )],
                                severity: Severity::Error,
                                span: state.span.clone(),
                                source_snippet: None,
                                proof_chain: Vec::new(),
                                examples: Vec::new(),
                                hints: vec!["Add ALIAS declaration to .dbvs schema file".to_string()],
                                notes: Vec::new(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        
        diagnostics
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
            expr @ Expr::AddrOf(_) => Some(expr.as_var_name().unwrap().to_string()),
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
                    ..
                } => {
                    read.extend(condition.extract_dependencies());
                    Self::collect_reads_stmts(statements, read);
                }
                Statement::Term { values: exprs, .. } | Statement::TermBang { values: exprs, .. } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
use std::collections::HashMap;
    use crate::errors::Severity;

    fn make_program(items: Vec<TopLevel>) -> Program {
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        }
    }

    fn txn(name: &str, pre: Expr, post: Expr, body: Vec<Statement>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true, is_async: false,
            parameters: vec![],
            contract: Contract { pre_condition: pre, post_condition: post, watchdog: None, span: None },
            body, reactor_speed: None, span: None,
            is_lambda: false, dependencies: vec![],
            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![], variant_bodies: vec![], outputs: vec![], output_type: None, derivation: None,
        })
    }

    fn state(name: &str, ty: Type) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(), ty, expr: None, address: None,
            bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
        constraint: None,
        })
    }

    #[test]
    fn test_hebv_rejects_link_dependency() {
        let program = make_program(vec![
            TopLevel::LinkDependency(LinkDependency {
                path: "link/foo.c".to_string(),
                source_lang: LinkLanguage::C,
            }),
        ]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].title.contains("link"));
    }

    #[test]
    fn test_hebv_rejects_frgn() {
        let program = make_program(vec![
            TopLevel::ForeignBinding {
                name: "foo".to_string(),
                toml_path: "foo.toml".to_string(),
                target: ForeignTarget::Native,
                signature: ForeignSignature {
                    name: "foo".to_string(),
                    location: "std::foo".to_string(),
                    wasm_impl: None, wasm_setup: None, inputs: vec![],
                    success_output: vec![], result_type: ResultType::VoidType,
                    error_type_name: "Error".to_string(), error_fields: vec![],
                    input_layout: None, output_layout: None,
                    precondition: None, postcondition: None,
                    buffer_mode: None, ffi_kind: None, is_out: false,
                    is_pipe: false, fallback: None, default_watchdog: None, span: None,
                },
                span: None,
            },
        ]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].title.contains("frgn"));
    }

    #[test]
    fn test_hebv_rejects_true_precondition() {
        let program = make_program(vec![
            txn("bad", Expr::Bool(true), Expr::Bool(false), vec![]),
        ]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        let pre_errors: Vec<_> = diags.iter().filter(|d| d.title.contains("precondition")).collect();
        assert_eq!(pre_errors.len(), 1, "Expected rejection of [true] precondition");
    }

    #[test]
    fn test_hebv_rejects_true_postcondition() {
        let program = make_program(vec![
            txn("bad", Expr::Bool(false), Expr::Bool(true), vec![]),
        ]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        let post_errors: Vec<_> = diags.iter().filter(|d| d.title.contains("postcondition")).collect();
        assert_eq!(post_errors.len(), 1, "Expected rejection of [true] postcondition");
    }

    #[test]
    fn test_hebv_rejects_float_type() {
        let program = make_program(vec![state("x", Type::float())]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].title.contains("Float"));
    }

    #[test]
    fn test_hebv_rejects_string_type() {
        let program = make_program(vec![state("s", Type::string())]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].title.contains("String"));
    }

    #[test]
    fn test_hebv_rejects_unsized_int() {
        let program = make_program(vec![state("x", Type::int())]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        let int_errors: Vec<_> = diags.iter().filter(|d| d.title.contains("Int/UInt")).collect();
        assert!(!int_errors.is_empty(), "Expected rejection of unsized Int");
    }

    #[test]
    fn test_hebv_accepts_synthesizable_types() {
        let program = make_program(vec![
            state("a", Type::bool_()),
            txn("good",
                Expr::Identifier("a".to_string()),
                Expr::Not(Box::new(Expr::Identifier("a".to_string()))),
                vec![]),
        ]);
        let diags = HardwareValidator::check_hebv_restrictions(&program);
        let type_errors: Vec<_> = diags.iter().filter(|d| d.code.starts_with("B500")).collect();
        assert_eq!(type_errors.len(), 0, "Bool + bounded txns should be OK");
    }
}

fn get_dbrief_type_size(db_type: &crate::dbrief::ast::DbriefType, engine: &DbvsEngine) -> u64 {
    use crate::dbrief::ast::DbriefType;
    match db_type {
        DbriefType::Bool => 1,
        DbriefType::Int(bits) | DbriefType::UInt(bits) => (bits / 8) as u64,
        DbriefType::Float => 8, // Assume f64
        DbriefType::Vector(inner, Some(size)) => {
            get_dbrief_type_size(inner, engine) * (*size as u64)
        }
        DbriefType::Named(name) => {
            if let Some(s) = engine.get_struct(name) {
                s.fields.iter().map(|(_, f_type)| get_dbrief_type_size(f_type, engine)).sum()
            } else if let Some(e) = engine.get_enum(name) {
                // Enums are usually stored as an integer type
                4 // Assume 32-bit integer for enum
            } else {
                0 // Or handle as an error
            }
        }
        DbriefType::Struct(fields) => {
            fields.iter().map(|(_, f_type)| get_dbrief_type_size(f_type, engine)).sum()
        }
        _ => 0, // Other types don't have a fixed size
    }
}
