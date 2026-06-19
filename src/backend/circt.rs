// CIRCT Backend — emits MLIR text in HW + Comb + Seq dialects.
// Invoked via: brief build file.cbv → program.mlir → circt-opt → circt-translate → verilog

use crate::analysis::dependency_graph::DependencyGraph;
use crate::ast::{BitRange, Contract, Expr, LinkRef, Program, Statement, TopLevel, Type};
use std::collections::HashMap;
use std::fmt::Write;

/// Metadata for a trigger variable mapped to a module port.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerPort {
    /// Signal name used as the port name.
    pub port_name: String,
    /// Original trigger name in Brief source.
    pub trg_name: String,
    /// Whether this trigger has the `#wake` modifier (generates wake_ output).
    pub is_wake: bool,
}

/// CIRCT backend state for MLIR code generation.
#[derive(Debug, Clone)]
pub struct CirctBackend {
    pub trg_ports: Vec<TriggerPort>,
    pub var_types: HashMap<String, Type>,
    pub var_exprs: HashMap<String, Option<Expr>>,
    /// State variables with @ addresses that should become external ports (MMIO).
    pub mmio_vars: Vec<String>,
}

/// Per-generation counters for unique MLIR value names.
#[derive(Debug, Default)]
struct NameGen {
    reg_counter: usize,
    wire_counter: usize,
    const_counter: usize,
}

impl NameGen {
    fn fresh_reg(&mut self, prefix: &str) -> String {
        let n = self.reg_counter;
        self.reg_counter += 1;
        format!("%{}_{}", prefix, n)
    }
    fn fresh_wire(&mut self, prefix: &str) -> String {
        let n = self.wire_counter;
        self.wire_counter += 1;
        format!("%{}_{}", prefix, n)
    }
    fn fresh_const(&mut self, prefix: &str) -> String {
        let n = self.const_counter;
        self.const_counter += 1;
        format!("%c{}_{}", prefix, n)
    }
}

impl CirctBackend {
    pub fn new() -> Self {
        CirctBackend {
            trg_ports: Vec::new(),
            var_types: HashMap::new(),
            var_exprs: HashMap::new(),
            mmio_vars: Vec::new(),
        }
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let dep_graph = DependencyGraph::build(program)
            .unwrap_or_else(|_| DependencyGraph {
                topo_order: Vec::new(),
                bit_index: HashMap::new(),
                dependencies: HashMap::new(),
                dependents: HashMap::new(),
                is_trg: std::collections::HashSet::new(),
                all_vars: std::collections::HashSet::new(),
            });

        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.var_types.insert(decl.name.clone(), decl.ty.clone());
                    self.var_exprs.insert(decl.name.clone(), decl.expr.clone());
                    if decl.address.is_some() {
                        self.mmio_vars.push(decl.name.clone());
                    }
                }
                TopLevel::Trigger(trg) => {
                    let port_name = self.trigger_port_name(&trg.address, &trg.name);
                    self.trg_ports.push(TriggerPort {
                        port_name: port_name.clone(),
                        trg_name: trg.name.clone(),
                        is_wake: trg.is_wake,
                    });
                    self.var_types.insert(port_name, trg.ty.clone());
                    self.var_exprs.insert(trg.name.clone(), None);
                }
                TopLevel::Trigger(trg) => {
                    let port_name = self.trigger_port_name(&trg.address, &trg.name);
                    self.trg_ports.push(TriggerPort {
                        port_name: port_name.clone(),
                        trg_name: trg.name.clone(),
                        is_wake: trg.is_wake,
                    });
                    self.var_types.insert(port_name, trg.ty.clone());
                    self.var_exprs.insert(trg.name.clone(), None);
                }
                TopLevel::Transaction(txn) => {
                    for stmt in &txn.body {
                        if let Statement::Assignment { lhs: Expr::OwnedRef(name), expr, .. } = stmt {
                            if !self.var_types.contains_key(name) {
                                self.var_types.insert(name.clone(), Type::Int);
                                self.var_exprs.insert(name.clone(), Some(expr.clone()));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let mut out = String::new();
        self.emit_header(&mut out);
        self.emit_module(&mut out, &dep_graph, program);
        out
    }

    /// Determine the port name for a trigger based on its LinkRef address.
    fn trigger_port_name(&self, address: &LinkRef, default_name: &str) -> String {
        match address {
            LinkRef::Explicit(_) => default_name.to_string(),
            LinkRef::Linked(name) => name.clone(),
            LinkRef::Timer(freq_hz) => format!("timer_{}hz", freq_hz),
            LinkRef::Signal(name) => name.clone(),
            LinkRef::Stdin => format!("{}_stdin", default_name),
        }
    }

    fn emit_header(&self, out: &mut String) {
        writeln!(out, "// Generated by brief-compiler CIRCT backend").ok();
        writeln!(out, "// Run: circt-opt program.mlir | circt-translate --export-verilog").ok();
        writeln!(out).ok();
    }

    fn mlir_type(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "i1".into(),
            Type::Int | Type::UInt => "i64".into(),
            Type::Char => "i32".into(),
            Type::Float => "f64".into(),
            Type::Constrained(inner, bit_range) => {
                let width = match bit_range {
                    BitRange::Single(w) => *w,
                    BitRange::Range(_, hi) => *hi,
                    BitRange::Any(w) => *w,
                };
                if matches!(inner.as_ref(), Type::Bool) && width <= 1 {
                    return "i1".into();
                }
                format!("i{}", width)
            }
            _ => "i64".into(),
        }
    }

    fn emit_module(&mut self, out: &mut String, dep_graph: &DependencyGraph, program: &Program) {
        let mut ng = NameGen::default();

        // Collect input and output ports separately
        let mut input_ports: Vec<String> = Vec::new();
        let mut output_ports: Vec<(String, String)> = Vec::new(); // (name, mlir_type)
        input_ports.push("in %clock: i1".to_string());
        input_ports.push("in %reset: i1".to_string());

        for trg in &self.trg_ports {
            if let Some(ty) = self.var_types.get(&trg.port_name) {
                let mlir_ty = self.mlir_type(ty);
                input_ports.push(format!("in %{}: {}", trg.port_name, mlir_ty));
            }
            if trg.is_wake {
                let mlir_ty = self.mlir_type(&Type::Bool);
                output_ports.push((format!("wake_{}", trg.port_name), mlir_ty));
            }
        }

        let sorted_vars = &dep_graph.topo_order;
        let trg_names: std::collections::HashSet<String> = self.trg_ports.iter().map(|t| t.trg_name.clone()).collect();
        for var_name in sorted_vars {
            if trg_names.contains(var_name) {
                continue;
            }
            if self.mmio_vars.contains(var_name) {
                // MMIO vars become external input ports instead of registers
                if let Some(ty) = self.var_types.get(var_name) {
                    let mlir_ty = self.mlir_type(ty);
                    input_ports.push(format!("in %{}: {}", var_name, mlir_ty));
                }
                continue;
            }
            if let Some(ty) = self.var_types.get(var_name) {
                let mlir_ty = self.mlir_type(ty);
                output_ports.push((var_name.clone(), mlir_ty));
            }
        }

        // Emit hw.module with input ports and output return signature
        write!(out, "hw.module @top(").ok();
        for (i, port) in input_ports.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}", port).ok();
        }
        write!(out, ") -> (").ok();
        for (i, (name, mlir_ty)) in output_ports.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}: {}", name, mlir_ty).ok();
        }
        writeln!(out, ") {{").ok();

        // Emit sequential registers for state variables (skip MMIO vars — they're external ports)
        let mut reg_names: HashMap<String, String> = HashMap::new();
        for (var_name, mlir_ty) in &output_ports {
            if self.mmio_vars.contains(var_name) {
                continue;
            }
            let init_val = self.initial_value(var_name);
            let reg = ng.fresh_reg(var_name);
            writeln!(out, "  {} = seq.firreg initial_value {{ init_value = {} : {} }} : {}", reg, init_val, mlir_ty, mlir_ty).ok();
            reg_names.insert(var_name.clone(), reg);
        }

        // Emit combinational expressions for each output variable
        for (var_name, mlir_ty) in &output_ports {
            if let Some(expr) = self.var_exprs.get(var_name).and_then(|e| e.as_ref()) {
                let result = self.emit_expr(&mut ng, out, expr, &reg_names, mlir_ty);
                if let Some(r) = result {
                    if let Some(reg) = reg_names.get(var_name) {
                        writeln!(out, "  seq.always(posedge %clock) {{").ok();
                        writeln!(out, "    {} <= comb.mux %reset, {}_init, {}", reg, r, r).ok();
                        writeln!(out, "  }}").ok();
                    }
                }
            } else {
                let c = ng.fresh_const(var_name);
                writeln!(out, "  {} = hw.constant 0 : {}", c, mlir_ty).ok();
            }
        }

        // Emit transaction body logic if any
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.emit_txn_body(&mut ng, out, &txn.name, &txn.body, &txn.contract, &reg_names);
            }
        }

        // Final hw.output maps register values to output ports
        write!(out, "  hw.output").ok();
        for (var_name, mlir_ty) in &output_ports {
            if let Some(reg) = reg_names.get(var_name) {
                write!(out, " {} : {},", reg, mlir_ty).ok();
            }
        }
        writeln!(out).ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    fn initial_value(&self, var_name: &str) -> String {
        if let Some(Some(expr)) = self.var_exprs.get(var_name) {
            match expr {
                Expr::Integer(n) => format!("{}", n),
                Expr::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
                Expr::Float(f) => format!("{}", f),
                _ => "0".to_string(),
            }
        } else {
            "0".to_string()
        }
    }

    fn emit_expr(&self, ng: &mut NameGen, out: &mut String, expr: &Expr, reg_names: &HashMap<String, String>, result_ty: &str) -> Option<String> {
        match expr {
            Expr::Integer(n) => {
                let c = ng.fresh_const("int");
                writeln!(out, "  {} = hw.constant {} : {}", c, n, result_ty).ok();
                Some(c)
            }
            Expr::Bool(b) => {
                let c = ng.fresh_const("bool");
                let v = if *b { "1" } else { "0" };
                writeln!(out, "  {} = hw.constant {} : i1", c, v).ok();
                Some(c)
            }
            Expr::Float(f) => {
                let c = ng.fresh_const("float");
                writeln!(out, "  {} = hw.constant {} : f64", c, f).ok();
                Some(c)
            }
            Expr::Identifier(name) => {
                if let Some(reg) = reg_names.get(name) {
                    Some(reg.clone())
                } else if self.trg_ports.iter().any(|t| t.trg_name == *name || t.port_name == *name) {
                    Some(format!("%{}", name))
                } else {
                    None
                }
            }
            Expr::Add(l, r) => self.emit_binary_comb(ng, out, "comb.add", l, r, reg_names, result_ty),
            Expr::Sub(l, r) => self.emit_binary_comb(ng, out, "comb.sub", l, r, reg_names, result_ty),
            Expr::Mul(l, r) => self.emit_binary_comb(ng, out, "comb.mul", l, r, reg_names, result_ty),
            Expr::Div(l, r) => self.emit_binary_comb(ng, out, "comb.divu", l, r, reg_names, result_ty),
            Expr::Mod(l, r) => self.emit_binary_comb(ng, out, "comb.mod", l, r, reg_names, result_ty),
            Expr::Eq(l, r) => self.emit_binary_comb(ng, out, "comb.icmp eq", l, r, reg_names, "i1"),
            Expr::Ne(l, r) => self.emit_binary_comb(ng, out, "comb.icmp ne", l, r, reg_names, "i1"),
            Expr::Lt(l, r) => self.emit_binary_comb(ng, out, "comb.icmp ult", l, r, reg_names, "i1"),
            Expr::Le(l, r) => self.emit_binary_comb(ng, out, "comb.icmp ule", l, r, reg_names, "i1"),
            Expr::Gt(l, r) => self.emit_binary_comb(ng, out, "comb.icmp ugt", l, r, reg_names, "i1"),
            Expr::Ge(l, r) => self.emit_binary_comb(ng, out, "comb.icmp uge", l, r, reg_names, "i1"),
            Expr::And(l, r) => self.emit_binary_comb(ng, out, "comb.and", l, r, reg_names, "i1"),
            Expr::Or(l, r) => self.emit_binary_comb(ng, out, "comb.or", l, r, reg_names, "i1"),
            Expr::Not(inner) => self.emit_unary_comb(ng, out, "comb.xor", inner, reg_names, "i1"),
            Expr::Neg(inner) => self.emit_unary_comb(ng, out, "comb.neg", inner, reg_names, result_ty),
            Expr::BitAnd(l, r) => self.emit_binary_comb(ng, out, "comb.and", l, r, reg_names, result_ty),
            Expr::BitOr(l, r) => self.emit_binary_comb(ng, out, "comb.or", l, r, reg_names, result_ty),
            Expr::BitXor(l, r) => self.emit_binary_comb(ng, out, "comb.xor", l, r, reg_names, result_ty),
            Expr::BitNot(inner) => self.emit_unary_comb(ng, out, "comb.xor", inner, reg_names, result_ty),
            Expr::Cast(inner, target_ty) => {
                let inner_mlir_ty = self.mlir_type(&crate::ast::Type::Int);
                let target_mlir_ty = self.mlir_type(target_ty);
                let val = self.emit_expr(ng, out, inner, reg_names, &inner_mlir_ty)?;
                let w = ng.fresh_wire("cast");
                if inner_mlir_ty == target_mlir_ty {
                    Some(val)
                } else {
                    writeln!(out, "  {} = comb.extract {} from 0 : ({}) -> {}", w, val, inner_mlir_ty, target_mlir_ty).ok();
                    Some(w)
                }
            }
            Expr::Call(name, _args) => {
                // Function calls become submodule instantiations (stub for now)
                None
            }
            _ => None,
        }
    }

    fn emit_binary_comb(&self, ng: &mut NameGen, out: &mut String, op: &str, l: &Expr, r: &Expr, reg_names: &HashMap<String, String>, result_ty: &str) -> Option<String> {
        let left = self.emit_expr(ng, out, l, reg_names, result_ty)?;
        let right = self.emit_expr(ng, out, r, reg_names, result_ty)?;
        let w = ng.fresh_wire("bin");
        writeln!(out, "  {} = {} {}, {} : {}", w, op, left, right, result_ty).ok();
        Some(w)
    }

    fn emit_unary_comb(&self, ng: &mut NameGen, out: &mut String, op: &str, inner: &Expr, reg_names: &HashMap<String, String>, result_ty: &str) -> Option<String> {
        let val = self.emit_expr(ng, out, inner, reg_names, result_ty)?;
        let w = ng.fresh_wire("un");
        writeln!(out, "  {} = {} {}, {}_one : {}", w, op, val, result_ty, result_ty).ok();
        Some(w)
    }

    fn emit_txn_body(&self, ng: &mut NameGen, out: &mut String, _name: &str, body: &[Statement], contract: &Contract, reg_names: &HashMap<String, String>) {
        // Allocate FSM state register: 2 bits supports 3 states (idle/running/done)
        let state_reg = ng.fresh_reg("txn_state");
        writeln!(out, "  {} = seq.firreg initial_value {{ init_value = 0 : i2 }} : i2", state_reg).ok();

        // Precondition: evaluate guard condition
        let pre_cond = ng.fresh_wire("pre");
        self.emit_contract_condition(out, ng, &contract.pre_condition, &pre_cond, reg_names);

        // Emit body logic with state-based enable
        let mut has_await = false;
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if let Expr::OwnedRef(var_name) = lhs {
                        let mlir_ty = self.mlir_type(self.var_types.get(var_name).unwrap_or(&Type::Int));
                        if let Some(reg) = reg_names.get(var_name) {
                            let val = self.emit_expr(ng, out, expr, reg_names, &mlir_ty);
                            if let Some(v) = val {
                                writeln!(out, "  seq.always(posedge %clock) {{").ok();
                                writeln!(out, "    {} <= {}", reg, v).ok();
                                writeln!(out, "  }}").ok();
                            }
                        }
                    }
                }
                Statement::Expression(expr) => {
                    self.emit_expr(ng, out, expr, reg_names, "i64");
                }
                Statement::Await { expr, .. } => {
                    has_await = true;
                    // Extract sub-module name from call expression
                    let sub_name = match expr {
                        Expr::Call(name, _args) => name.clone(),
                        _ => "sub".to_string(),
                    };
                    // Emit sub-module instance with start/done handshake
                    let start_wire = ng.fresh_wire(&format!("{}_start", sub_name));
                    let done_wire = ng.fresh_wire(&format!("{}_done", sub_name));
                    let sub_result = ng.fresh_wire(&format!("{}_result", sub_name));
                    writeln!(out, "  {} = hw.wire : i1", start_wire).ok();
                    writeln!(out, "  {} = hw.wire : i1", done_wire).ok();
                    writeln!(out, "  {} = hw.wire : i64", sub_result).ok();
                    // FSM: assert start on entering, stall until done
                    let stall_wire = ng.fresh_wire("stall");
                    let not_done = ng.fresh_wire("not_done");
                    let c1 = ng.fresh_const("one");
                    writeln!(out, "  {} = hw.constant 1 : i1", c1).ok();
                    writeln!(out, "  {} = comb.xor {}, {} : i1", not_done, done_wire, c1).ok();
                    writeln!(out, "  {} = comb.mux {}, {}, {} : i2", stall_wire, not_done, 2, 1).ok();
                }
                Statement::Async { body, .. } | Statement::AsyncAwait { body, .. } => {
                    self.emit_stmt_body(ng, out, body, reg_names);
                }
                _ => {}
            }
        }

        // Postcondition: evaluate guarantee condition
        let post_cond = ng.fresh_wire("post");
        self.emit_contract_condition(out, ng, &contract.post_condition, &post_cond, reg_names);

        // State transition: if postcondition met, go to done (2);
        // if await stall, stay in stall state; else if precondition false go to idle (0)
        let state_next = ng.fresh_wire("txn_state_next");
        if has_await {
            writeln!(out, "  {} = comb.mux {}, {}, {} : i2", state_next, post_cond, 2, 2).ok();
        } else {
            writeln!(out, "  {} = comb.mux {}, {}, {} : i2", state_next, post_cond, 2, 1).ok();
        }
        let state_after_body = ng.fresh_wire("txn_state_after");
        writeln!(out, "  {} = comb.mux {}, {}, {} : i2", state_after_body, pre_cond, state_next, 0).ok();
        writeln!(out, "  seq.always(posedge %clock) {{").ok();
        writeln!(out, "    {} <= {}", state_reg, state_after_body).ok();
        writeln!(out, "  }}").ok();
    }

    /// Emit statements that are purely combinational (no FSM involvement).
    fn emit_stmt_body(&self, ng: &mut NameGen, out: &mut String, stmt: &Statement, reg_names: &HashMap<String, String>) {
        match stmt {
            Statement::Expression(expr) => {
                self.emit_expr(ng, out, expr, reg_names, "i64");
            }
            Statement::Assignment { lhs, expr, .. } => {
                if let Expr::OwnedRef(var_name) = lhs {
                    let mlir_ty = self.mlir_type(self.var_types.get(var_name).unwrap_or(&Type::Int));
                    if let Some(reg) = reg_names.get(var_name) {
                        let val = self.emit_expr(ng, out, expr, reg_names, &mlir_ty);
                        if let Some(v) = val {
                            writeln!(out, "  seq.always(posedge %clock) {{").ok();
                            writeln!(out, "    {} <= {}", reg, v).ok();
                            writeln!(out, "  }}").ok();
                        }
                    }
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                for s in statements {
                    self.emit_stmt_body(ng, out, s, reg_names);
                }
            }
            Statement::SyncBlock { body } => {
                // Sync block: emit all statements combinatorially (parallel)
                for s in body {
                    self.emit_stmt_body(ng, out, s, reg_names);
                }
            }
            _ => {}
        }
    }

    /// Emit combinational logic for a contract condition (precondition or postcondition).
    /// Supports simple comparisons like `[x < N]`, `[x == N]`, and logical combinations.
    fn emit_contract_condition(&self, out: &mut String, ng: &mut NameGen, cond: &Expr, result_wire: &str, reg_names: &HashMap<String, String>) {
        match cond {
            Expr::Bool(true) => {
                writeln!(out, "  {} = hw.constant 1 : i1", result_wire).ok();
            }
            Expr::Bool(false) => {
                writeln!(out, "  {} = hw.constant 0 : i1", result_wire).ok();
            }
            Expr::Lt(l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp ult {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::Le(l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp ule {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::Gt(l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp ugt {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::Ge(l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp uge {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::Eq(l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp eq {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::And(l, r) => {
                let left_wire = ng.fresh_wire("cond_l");
                self.emit_contract_condition(out, ng, l, &left_wire, reg_names);
                let right_wire = ng.fresh_wire("cond_r");
                self.emit_contract_condition(out, ng, r, &right_wire, reg_names);
                writeln!(out, "  {} = comb.and {}, {} : i1", result_wire, left_wire, right_wire).ok();
            }
            Expr::Or(l, r) => {
                let left_wire = ng.fresh_wire("cond_l");
                self.emit_contract_condition(out, ng, l, &left_wire, reg_names);
                let right_wire = ng.fresh_wire("cond_r");
                self.emit_contract_condition(out, ng, r, &right_wire, reg_names);
                writeln!(out, "  {} = comb.or {}, {} : i1", result_wire, left_wire, right_wire).ok();
            }
            Expr::Not(inner) => {
                let inner_wire = ng.fresh_wire("cond_not");
                self.emit_contract_condition(out, ng, inner, &inner_wire, reg_names);
                let c = ng.fresh_const("true");
                writeln!(out, "  {} = hw.constant 1 : i1", c).ok();
                writeln!(out, "  {} = comb.xor {}, {} : i1", result_wire, inner_wire, c).ok();
            }
            _ => {
                writeln!(out, "  {} = hw.constant 1 : i1", result_wire).ok();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_program(items: Vec<TopLevel>) -> Program {
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        }
    }

    fn make_state_decl(name: &str, ty: Type, expr: Option<Expr>) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty,
            expr,
            address: None,
            bit_range: None,
            range_constraint: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
        })
    }

    fn make_trigger(name: &str, ty: Type) -> TopLevel {
        TopLevel::Trigger(TriggerDeclaration {
            name: name.to_string(),
            ty,
            address: LinkRef::Explicit(0),
            bit_range: None,
            stages: vec![],
            condition: None,
            is_wake: false,
            span: None,
        })
    }

    fn make_txn(name: &str, body: Vec<Statement>, pre: Expr, post: Expr) -> TopLevel {
        TopLevel::Transaction(Transaction {
            is_async: false, is_reactive: true,
            name: name.to_string(),
            parameters: vec![],
            contract: Contract { pre_condition: pre, post_condition: post, watchdog: None, span: None },
            body,
            reactor_speed: None, span: None, is_lambda: false,
            dependencies: vec![], attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        })
    }

    #[test]
    fn test_circt_empty_program() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![]));
        assert!(output.contains("hw.module @top"));
        assert!(output.contains("clock: i1"));
        assert!(output.contains("hw.output"));
    }

    #[test]
    fn test_circt_trg_port() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_trigger("sensor", Type::Int),
        ]));
        assert!(output.contains("sensor: i64"));
    }

    #[test]
    fn test_circt_state_var_has_seq_register() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("counter", Type::Int, Some(Expr::Integer(0))),
        ]));
        assert!(output.contains("seq.firreg"), "State vars should use seq.firreg. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_add() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::Int, Some(Expr::Add(
                Box::new(Expr::Integer(1)),
                Box::new(Expr::Integer(2)),
            ))),
        ]));
        assert!(output.contains("comb.add"), "Add expr should emit comb.add. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_mul() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("y", Type::Int, Some(Expr::Mul(
                Box::new(Expr::Integer(3)),
                Box::new(Expr::Integer(4)),
            ))),
        ]));
        assert!(output.contains("comb.mul"), "Mul expr should emit comb.mul. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_lt() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("cond", Type::Bool, Some(Expr::Lt(
                Box::new(Expr::Integer(5)),
                Box::new(Expr::Integer(10)),
            ))),
        ]));
        assert!(output.contains("comb.icmp ult"), "Lt expr should emit comb.icmp ult. Got:\n{}", output);
    }

    #[test]
    fn test_circt_modern_output_ports() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("counter", Type::Int, Some(Expr::Integer(0))),
        ]));
        // Modern form: hw.module @top(in %clock: i1, in %reset: i1) -> (counter: i64)
        assert!(output.contains("in %clock: i1"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
        assert!(output.contains("in %reset: i1"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
        assert!(output.contains("-> (counter: i64)"), "Outputs should be in return signature. Got:\n{}", output);
        assert!(!output.contains("hw.output_assign"), "Should not use deprecated hw.output_assign. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sized_int() {
        let mut backend = CirctBackend::new();
        // UInt constrained to 8 bits
        let ty = Type::Constrained(Box::new(Type::UInt), BitRange::Single(8));
        let output = backend.generate(&make_program(vec![
            make_state_decl("byte", ty, Some(Expr::Integer(0))),
        ]));
        assert!(output.contains(": i8)"), "Sized UInt[8] should map to i8. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sized_int_32() {
        let mut backend = CirctBackend::new();
        let ty = Type::Constrained(Box::new(Type::UInt), BitRange::Single(32));
        let output = backend.generate(&make_program(vec![
            make_state_decl("word", ty, Some(Expr::Integer(0))),
        ]));
        assert!(output.contains(": i32)"), "Sized UInt[32] should map to i32. Got:\n{}", output);
    }

    #[test]
    fn test_circt_linked_trg() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            TopLevel::Trigger(TriggerDeclaration {
                name: "btn".to_string(), ty: Type::Bool,
                address: LinkRef::Linked("button0".to_string()),
                bit_range: None, stages: vec![], condition: None,
                is_wake: false, span: None,
            }),
        ]));
        // Linked triggers use the linked name as port name
        assert!(output.contains("button0"), "Linked trg should use linked name. Got:\n{}", output);
    }

    #[test]
    fn test_circt_wake_trg() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            TopLevel::Trigger(TriggerDeclaration {
                name: "btn".to_string(), ty: Type::Bool,
                address: LinkRef::Explicit(0),
                bit_range: None, stages: vec![], condition: None,
                is_wake: true, span: None,
            }),
        ]));
        // Wake triggers produce a wake_ output port
        assert!(output.contains("wake_btn"), "Wake trg should emit wake_ port. Got:\n{}", output);
    }

    #[test]
    fn test_circt_mmio_input_port() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            TopLevel::StateDecl(StateDecl {
                name: "status".to_string(), ty: Type::Int,
                expr: None,
                address: Some(0x40000000),
                bit_range: None, range_constraint: None,
                is_override: false, os_mode: false,
                span: None, attrs: vec![],
            }),
        ]));
        // MMIO vars with @ address should become input ports, not registers
        assert!(output.contains("in %status: i64"), "MMIO var should be input port. Got:\n{}", output);
        assert!(output.contains("-> ()"), "MMIO-only module has no output ports. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_state_reg() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_trigger("tick", Type::Bool),
            make_state_decl("counter", Type::Int, Some(Expr::Integer(0))),
            make_txn("count", vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("counter".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("counter".to_string())),
                        Box::new(Expr::Integer(1)),
                    ),
                    timeout: None, modifiers: vec![],
                },
            ], Expr::Bool(true), Expr::Bool(true)),
        ]));
        assert!(output.contains("seq.firreg"), "FSM should have state reg. Got:\n{}", output);
        assert!(output.contains("comb.mux"), "FSM should have state transition mux. Got:\n{}", output);
        assert!(output.contains("seq.always(posedge %clock)"), "FSM should have seq.always. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_precondition_check() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("done", Type::Int, Some(Expr::Integer(0))),
            make_txn("loop", vec![], Expr::Lt(
                Box::new(Expr::Identifier("done".to_string())),
                Box::new(Expr::Integer(10)),
            ), Expr::Bool(true)),
        ]));
        assert!(output.contains("comb.icmp ult"), "Precondition should emit comb.icmp. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_postcondition_check() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("done", Type::Int, Some(Expr::Integer(0))),
            make_txn("loop", vec![], Expr::Bool(true), Expr::Eq(
                Box::new(Expr::Identifier("done".to_string())),
                Box::new(Expr::Integer(10)),
            )),
        ]));
        assert!(output.contains("comb.icmp eq"), "Postcondition should emit comb.icmp eq. Got:\n{}", output);
    }

    #[test]
    fn test_circt_await_handshake() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::Int, Some(Expr::Integer(0))),
            make_txn("test", vec![
                Statement::Await {
                    expr: Expr::Call("compute".to_string(), vec![Expr::Integer(42)]),
                    modifiers: vec![],
                },
            ], Expr::Bool(true), Expr::Bool(true)),
        ]));
        assert!(output.contains("hw.wire"), "Await should emit handshake wires. Got:\n{}", output);
        assert!(output.contains("comb.mux"), "Await should emit stall mux. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sync_block() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("a", Type::Int, Some(Expr::Integer(0))),
            make_state_decl("b", Type::Int, Some(Expr::Integer(0))),
            make_txn("test", vec![
                Statement::SyncBlock {
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("a".to_string()),
                            expr: Expr::Integer(10),
                            timeout: None, modifiers: vec![],
                        },
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("b".to_string()),
                            expr: Expr::Integer(20),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                },
            ], Expr::Bool(true), Expr::Bool(true)),
        ]));
        assert!(output.contains("seq.always(posedge %clock)"), "Sync block should emit seq updates. Got:\n{}", output);
    }
}
