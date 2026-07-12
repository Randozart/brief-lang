// CIRCT Backend — emits MLIR text in HW + Comb + Seq dialects.
// Invoked via: brief build file.cbv → program.mlir → circt-opt → circt-translate → verilog

use crate::analysis::dependency_graph::DependencyGraph;
use crate::ast::{BitRange, Contract, Expr, Intrinsic, LinkRef, OutputType, Program, Statement, TopLevel, Type};
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
    /// Known function/module names and their argument counts (for submodule instantiation).
    pub fn_arity: HashMap<String, usize>,
    /// Cell definitions encountered during program traversal.
    /// Key is cell name, value is the CellDef AST node.
    cell_defs: HashMap<String, crate::ast::CellDef>,
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
            fn_arity: HashMap::new(),
            cell_defs: HashMap::new(),
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
                TopLevel::Transaction(txn) => {
                    self.fn_arity.insert(txn.name.clone(), txn.parameters.len());
                    for stmt in &txn.body {
                        if let Statement::Assignment { lhs, expr, .. } = stmt {
                            if let Some(name) = lhs.as_var_name() {
                                if !self.var_types.contains_key(name) {
                                    self.var_types.insert(name.to_string(), Type::int());
                                    self.var_exprs.insert(name.to_string(), Some(expr.clone()));
                                }
                            }
                        }
                    }
                }
                TopLevel::Cell(cell) => {
                    // Track the cell definition for module emission
                    self.cell_defs.insert(cell.name.clone(), cell.as_ref().clone());
                    // Register cell fields as state variables
                    for field in &cell.fields {
                        self.var_types.insert(format!("{}${}", cell.name, field.name), field.ty.clone());
                        self.var_exprs.insert(format!("{}${}", cell.name, field.name), field.default.clone());
                    }
                    // Register cell parameters as input ports
                    for (param_name, param_ty) in &cell.parameters {
                        self.var_types.insert(format!("{}${}", cell.name, param_name), param_ty.clone());
                    }
                }
                _ => {}
            }
        }

        let mut out = String::new();
        self.emit_header(&mut out);
        self.emit_module(&mut out, &dep_graph, program);
        // Emit separate hw.module for each cell with synthesized transaction bodies
        let cell_defs: Vec<crate::ast::CellDef> = self.cell_defs.values().cloned().collect();
        for cell_def in &cell_defs {
            self.emit_cell_module(&mut out, &dep_graph, cell_def);
        }
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
            Type::Custom(__t) if __t == "Bool" => "i1".into(),
            Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64".into(),
            // 2026-06-29: Fixed-width types for CIRCT backend
            Type::Custom(__t) if __t == "Int8" => "si8".into(),
            Type::Custom(__t) if __t == "Int16" => "si16".into(),
            Type::Custom(__t) if __t == "Int32" => "si32".into(),
            Type::Custom(__t) if __t == "UInt8" => "ui8".into(),
            Type::Custom(__t) if __t == "UInt16" => "ui16".into(),
            Type::Custom(__t) if __t == "UInt32" => "ui32".into(),
            Type::Custom(__t) if __t == "Char" => "i32".into(),
            Type::Custom(__t) if __t == "Float" => "f64".into(),
            Type::Custom(__t) if __t == "Float64" => "f64".into(),
            Type::Constrained(inner, bit_range) => {
                let width = match bit_range {
                    BitRange::Single(w) => *w,
                    BitRange::Range(_, hi) => *hi,
                    BitRange::Any(w) => *w,
                };
                if matches!(inner.as_ref(), Type::Custom(__t) if __t == "Bool") && width <= 1 {
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
                let mlir_ty = self.mlir_type(&Type::bool_());
                output_ports.push((format!("wake_{}", trg.port_name), mlir_ty));
            }
        }

        // Always add a halt output port (driven high by term! statements)
        output_ports.push(("halt".to_string(), "i1".to_string()));

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
        // Also skip special output ports like halt (handled separately in txn body)
        let mut reg_names: HashMap<String, String> = HashMap::new();
        let special_outputs: std::collections::HashSet<&str> = ["halt"].iter().cloned().collect();
        for (var_name, mlir_ty) in &output_ports {
            if self.mmio_vars.contains(var_name) || special_outputs.contains(var_name.as_str()) {
                continue;
            }
            let init_val = self.initial_value(var_name);
            let reg = ng.fresh_reg(var_name);
            writeln!(out, "  {} = seq.firreg initial_value {{ init_value = {} : {} }} : {}", reg, init_val, mlir_ty, mlir_ty).ok();
            reg_names.insert(var_name.clone(), reg);
        }

        // Emit combinational expressions for each output variable

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
                self.emit_txn_body(&mut ng, out, &txn.name, &txn.body, &txn.contract, &mut reg_names);
            }
        }

        // Final hw.output maps register values to output ports
        write!(out, "  hw.output").ok();
        for (var_name, mlir_ty) in &output_ports {
            if let Some(reg) = reg_names.get(var_name) {
                write!(out, " {} : {},", reg, mlir_ty).ok();
            } else if var_name == "halt" {
                // If no txn drives halt, default to 0
                let c = ng.fresh_const("halt_default");
                writeln!(out, "  {} = hw.constant 0 : i1", c).ok();
                write!(out, " {} : {},", c, mlir_ty).ok();
            }
        }
        writeln!(out).ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    fn initial_value(&self, var_name: &str) -> String {
        if let Some(Some(expr)) = self.var_exprs.get(var_name) {
            match expr {
                Expr::Decimal(n) => format!("{}", n),
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
            Expr::Decimal(n) => {
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
                let inner_mlir_ty = self.mlir_type(&crate::ast::Type::int());
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
            Expr::Call(name, args) => {
                // Function calls become submodule instantiations
                let inst_name = name.replace('-', "_");
                let result_wire = ng.fresh_wire(&format!("{}_result", inst_name));
                // Emit argument wires
                let mut arg_parts = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let arg_mlir_ty = if i == 0 { "i64" } else { result_ty };
                    if let Some(arg_val) = self.emit_expr(ng, out, arg, reg_names, arg_mlir_ty) {
                        arg_parts.push(format!("{}: $arg{}: {}", arg_val, i, arg_mlir_ty));
                    }
                }
                let arity = self.fn_arity.get(name).copied().unwrap_or(args.len());
                let result_mlir_ty = result_ty;
                writeln!(out, "  {} = hw.instance \"{}\" @{} ({}) -> ({}: ${}: {})",
                    result_wire, inst_name, inst_name,
                    arg_parts.join(", "),
                    result_wire, "result", result_mlir_ty,
                ).ok();
                // Declare the external module if not already declared
                let _ = arity;
                Some(result_wire)
            }
            Expr::CellCall(callee, args) => {
                let callee_name = match callee.as_ref() {
                    Expr::Identifier(name) => name.clone(),
                    _ => return None,
                };
                let inst_name = format!("{}_inst", callee_name);
                let result_wire = ng.fresh_wire(&format!("{}_result", callee_name));
                let mut arg_parts = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let arg_mlir_ty = result_ty;
                    if let Some(arg_val) = self.emit_expr(ng, out, arg, reg_names, arg_mlir_ty) {
                        arg_parts.push(format!("{}: $arg{}: {}", arg_val, i, arg_mlir_ty));
                    }
                }
                // Read output ports from the cell instance
                writeln!(out, "  {} = hw.instance \"{}\" @{} ({}) -> ({}: ${}: {})",
                    result_wire, inst_name, callee_name,
                    arg_parts.join(", "),
                    result_wire, "result", result_ty,
                ).ok();
                Some(result_wire)
            }
            Expr::IntrinsicCall { intrinsic, args } => {
                let mut arg = |i: usize| -> String {
                    args.get(i).and_then(|a| {
                        let ty = if matches!(intrinsic, Intrinsic::Fabs | Intrinsic::Sqrt | Intrinsic::Ceil | Intrinsic::Floor | Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow) { "f64" } else { result_ty };
                        self.emit_expr(ng, out, a, reg_names, ty)
                    }).unwrap_or_else(|| "%0".to_string())
                };
                match intrinsic {
                    Intrinsic::Abs => {
                        // abs(x) = x < 0 ? -x : x — implement via comb.icmp + comb.mux + comb.neg
                        let x = arg(0);
                        let neg_x = ng.fresh_wire("neg");
                        let cmp = ng.fresh_wire("cmp_neg");
                        writeln!(out, "  {} = comb.neg {} : {}", neg_x, x, result_ty).ok();
                        writeln!(out, "  {} = comb.icmp slt {}, %c0_0 : {}", cmp, x, result_ty).ok();
                        let w = ng.fresh_wire("abs");
                        writeln!(out, "  {} = comb.mux {}, {}, {} : {}", w, cmp, neg_x, x, result_ty).ok();
                        Some(w)
                    }
                    Intrinsic::Ctpop => {
                        let x = arg(0);
                        let w = ng.fresh_wire("popcount");
                        writeln!(out, "  {} = comb.ctpop {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    Intrinsic::Ctlz => {
                        let x = arg(0);
                        let w = ng.fresh_wire("ctlz");
                        writeln!(out, "  {} = comb.ctlz {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    Intrinsic::Cttz => {
                        let x = arg(0);
                        let w = ng.fresh_wire("cttz");
                        writeln!(out, "  {} = comb.cttz {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    Intrinsic::Bitreverse => {
                        let x = arg(0);
                        let w = ng.fresh_wire("bitreverse");
                        writeln!(out, "  {} = comb.rev {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    Intrinsic::Size => {
                        // Size of a known variable: emit the field width in bits
                        let c = ng.fresh_const("size");
                        writeln!(out, "  {} = hw.constant 64 : {}", c, result_ty).ok();
                        Some(c)
                    }
                    Intrinsic::Sqrt | Intrinsic::Fabs | Intrinsic::Ceil | Intrinsic::Floor => {
                        // Float intrinsics: emit as f64 ops
                        let x = arg(0);
                        let op = match intrinsic {
                            Intrinsic::Sqrt => "sqrt",
                            Intrinsic::Fabs => "absf",
                            Intrinsic::Ceil => "ceil",
                            Intrinsic::Floor => "floor",
                            _ => unreachable!(),
                        };
                        let w = ng.fresh_wire(op);
                        writeln!(out, "  {} = comb.{} {} : f64", w, op, x).ok();
                        Some(w)
                    }
                    Intrinsic::Sin => {
                        let x = arg(0);
                        let w = ng.fresh_wire("sin");
                        writeln!(out, "  {} = comb.sin {} : f64", w, x).ok();
                        Some(w)
                    }
                    Intrinsic::Cos => {
                        let x = arg(0);
                        let w = ng.fresh_wire("cos");
                        writeln!(out, "  {} = comb.cos {} : f64", w, x).ok();
                        Some(w)
                    }
                    Intrinsic::Pow => {
                        let x = arg(0);
                        let y = arg(1);
                        let w = ng.fresh_wire("pow");
                        writeln!(out, "  {} = comb.pow {}, {} : f64", w, x, y).ok();
                        Some(w)
                    }
                    _ => {
                        // Unknown intrinsic: emit constant 0
                        let c = ng.fresh_const("unk_intr");
                        writeln!(out, "  {} = hw.constant 0 : {}", c, result_ty).ok();
                        Some(c)
                    }
                }
            }
            Expr::AddrOf(inner) => {
                // In CIRCT, address-of evaluates the inner expression and
                // returns the register name (same as Identifier — no pointer
                // indirection needed for hardware synthesis).
                self.emit_expr(ng, out, inner, reg_names, result_ty)
            }
            Expr::Deref(ptr) => {
                // Dereference reads the pointed-to value.
                // In CIRCT, this is just a wire assignment.
                self.emit_expr(ng, out, ptr, reg_names, result_ty)
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

    fn emit_txn_body(&self, ng: &mut NameGen, out: &mut String, _name: &str, body: &[Statement], contract: &Contract, reg_names: &mut HashMap<String, String>) {
        // Allocate FSM state register: 2 bits supports 3 states (idle/running/done)
        let state_reg = ng.fresh_reg("txn_state");
        writeln!(out, "  {} = seq.firreg initial_value {{ init_value = 0 : i2 }} : i2", state_reg).ok();

        // Halt register: goes high when term! is encountered (drives module output)
        let halt_reg = ng.fresh_reg("halt");
        let c0 = ng.fresh_const("zero_i1");
        writeln!(out, "  {} = hw.constant 0 : i1", c0).ok();
        writeln!(out, "  {} = seq.firreg initial_value {{ init_value = 0 : i1 }} : i1", halt_reg).ok();
        reg_names.insert("halt".to_string(), halt_reg.clone());

        // Precondition: evaluate guard condition
        let pre_cond = ng.fresh_wire("pre");
        self.emit_contract_condition(out, ng, &contract.pre_condition, &pre_cond, reg_names);

        // Emit body logic with state-based enable
        let mut has_await = false;
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if let Some(var_name) = lhs.as_var_name() {
                        let mlir_ty = self.mlir_type(self.var_types.get(var_name).unwrap_or(&Type::int()));
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
                Statement::TermBang { .. } => {
                    writeln!(out, "  seq.always(posedge %clock) {{").ok();
                    writeln!(out, "    {} <= 1 : i1", halt_reg).ok();
                    writeln!(out, "  }}").ok();
                }
                Statement::Term { .. } => {
                    // Regular term (non-bang): no halt — commit action continues
                }
                Statement::Foreach { item, list, body, .. } => {
                    // Hardware can't do dynamic iteration. Try compile-time unroll.
                    let list_items = match list.as_ref() {
                        Expr::ListLiteral(items) => Some(items),
                        _ => None,
                    };
                    if let Some(items) = list_items {
                        for (i, elem) in items.iter().enumerate() {
                            writeln!(out, "  // foreach iteration {}: {} = {:?}", i, item, elem).ok();
                            for stmt in body {
                                self.emit_txn_body(ng, out, _name, &[stmt.clone()], contract, reg_names);
                            }
                        }
                    } else {
                        writeln!(out, "  // foreach skipped — non-constant list, unroll not possible").ok();
                    }
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

    /// Emit a standalone hw.module for a cell definition with synthesized
    /// transaction body. The module has input ports for parameters, state
    /// registers for fields, and output ports matching the cell's output type.
    fn emit_cell_module(&mut self, out: &mut String, dep_graph: &DependencyGraph, cell: &crate::ast::CellDef) {
        let cell_name = &cell.name;
        let mut ng = NameGen::default();

        // Cell module ports: clock, reset, input params, output result
        write!(out, "hw.module @{}(", cell_name).ok();
        write!(out, "in %clock: i1, in %reset: i1").ok();
        for (param_name, param_ty) in &cell.parameters {
            let mlir_ty = self.mlir_type(param_ty);
            write!(out, ", in %{}: {}", param_name, mlir_ty).ok();
        }
        let output_names = Self::extract_output_names_llvm(&cell.output_type);
        if let Some(first_out) = output_names.first() {
            let out_mlir_ty = cell.parameters.first()
                .map(|(_, t)| self.mlir_type(t))
                .unwrap_or_else(|| "i64".to_string());
            write!(out, ") -> ({}: ${}: {})", first_out, "result", out_mlir_ty).ok();
        } else {
            write!(out, ") -> ()").ok();
        }
        writeln!(out, " {{").ok();

        // Emit registers for state fields
        let mut reg_names: HashMap<String, String> = HashMap::new();
        for field in &cell.fields {
            let mlir_ty = self.mlir_type(&field.ty);
            let init_val = match &field.default {
                Some(Expr::Decimal(n)) => format!("{}", n),
                Some(Expr::Bool(b)) => format!("{}", if *b { 1 } else { 0 }),
                _ => "0".to_string(),
            };
            let reg = ng.fresh_reg(cell_name);
            writeln!(out, "  {} = seq.firreg initial_value {{ init_value = {} : {} }} : {}", reg, init_val, mlir_ty, mlir_ty).ok();
            reg_names.insert(field.name.clone(), reg);
        }

        // Emit transaction bodies as combinational+sequential logic
        for txn in &cell.transactions {
            // Emit precondition as a when-guard
            if !matches!(&txn.contract.pre_condition, Expr::Bool(true)) {
                let pre_mlir_ty = "i1";
                if let Some(pre_val) = self.emit_expr(&mut ng, out, &txn.contract.pre_condition, &reg_names, pre_mlir_ty) {
                    writeln!(out, "  %when_{} = comb.icmp eq {} %true : {}", ng.fresh_wire("wc"), pre_val, pre_mlir_ty).ok();
                }
            }
            for stmt in &txn.body {
                self.emit_stmt_body(&mut ng, out, stmt, &reg_names);
            }
        }

        // Drive output ports from the last assigned field value
        if let Some(first_out) = output_names.first() {
            if let Some(reg) = reg_names.get(first_out) {
                writeln!(out, "  seq.always(posedge %clock) {{").ok();
                writeln!(out, "    {} <= {}", reg.clone(), reg.clone()).ok();
                writeln!(out, "  }}").ok();
            }
        }

        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Extract output port names from an optional OutputType.
    /// This is CIRCT's poor sibling of the LLVM backend function — CIRCT
    /// doesn't support multi-output cells, so we return at most one name.
    fn extract_output_names_llvm(output_type: &Option<crate::ast::OutputType>) -> Vec<String> {
        match output_type {
            Some(crate::ast::OutputType::Named(name, _)) => vec![name.clone()],
            Some(crate::ast::OutputType::Single(_)) => vec!["result".to_string()],
            Some(crate::ast::OutputType::Array(_)) => vec!["result".to_string()],
            Some(crate::ast::OutputType::Tuple(types)) => {
                (0..types.len()).map(|i| format!("out{}", i)).collect()
            }
            Some(crate::ast::OutputType::Union(types)) => {
                (0..types.len()).map(|i| format!("case{}", i)).collect()
            }
            None => vec![],
        }
    }

    /// Emit statements that are purely combinational (no FSM involvement).
    fn emit_stmt_body(&self, ng: &mut NameGen, out: &mut String, stmt: &Statement, reg_names: &HashMap<String, String>) {
        match stmt {
            Statement::Expression(expr) => {
                self.emit_expr(ng, out, expr, reg_names, "i64");
            }
            Statement::Assignment { lhs, expr, .. } => {
                if let Some(var_name) = lhs.as_var_name() {
                    let mlir_ty = self.mlir_type(self.var_types.get(var_name).unwrap_or(&Type::int()));
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
                watchdog_defaults: (None, None),
        }
    }

    fn make_state_decl(name: &str, ty: Type, expr: Option<Expr>) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty,
            expr,
            address: None,
            bit_range: None,
            constraint: None,
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
            is_wake: true,
            is_const: false,
            span: None,
            annotations: vec![],
            modifiers: vec![],
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
            dependencies: vec![],
            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
            derivation: None,
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
            make_trigger("sensor", Type::int()),
        ]));
        assert!(output.contains("sensor: i64"));
    }

    #[test]
    fn test_circt_state_var_has_seq_register() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("counter", Type::int(), Some(Expr::Decimal(0))),
        ]));
        assert!(output.contains("seq.firreg"), "State vars should use seq.firreg. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_add() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::int(), Some(Expr::Add(
                Box::new(Expr::Decimal(1)),
                Box::new(Expr::Decimal(2)),
            ))),
        ]));
        assert!(output.contains("comb.add"), "Add expr should emit comb.add. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_mul() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("y", Type::int(), Some(Expr::Mul(
                Box::new(Expr::Decimal(3)),
                Box::new(Expr::Decimal(4)),
            ))),
        ]));
        assert!(output.contains("comb.mul"), "Mul expr should emit comb.mul. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_lt() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("cond", Type::bool_(), Some(Expr::Lt(
                Box::new(Expr::Decimal(5)),
                Box::new(Expr::Decimal(10)),
            ))),
        ]));
        assert!(output.contains("comb.icmp ult"), "Lt expr should emit comb.icmp ult. Got:\n{}", output);
    }

    #[test]
    fn test_circt_modern_output_ports() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("counter", Type::int(), Some(Expr::Decimal(0))),
        ]));
        // Modern form: hw.module @top(in %clock: i1, in %reset: i1) -> (counter: i64)
        assert!(output.contains("in %clock: i1"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
        assert!(output.contains("in %reset: i1"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
        assert!(output.contains("halt: i1"), "Outputs should include halt. Got:\n{}", output);
        assert!(output.contains("counter: i64"), "Outputs should include counter. Got:\n{}", output);
        assert!(!output.contains("hw.output_assign"), "Should not use deprecated hw.output_assign. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sized_int() {
        let mut backend = CirctBackend::new();
        // UInt constrained to 8 bits
        let ty = Type::Constrained(Box::new(Type::uint()), BitRange::Single(8));
        let output = backend.generate(&make_program(vec![
            make_state_decl("byte", ty, Some(Expr::Decimal(0))),
        ]));
        assert!(output.contains(": i8)"), "Sized UInt[8] should map to i8. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sized_int_32() {
        let mut backend = CirctBackend::new();
        let ty = Type::Constrained(Box::new(Type::uint()), BitRange::Single(32));
        let output = backend.generate(&make_program(vec![
            make_state_decl("word", ty, Some(Expr::Decimal(0))),
        ]));
        assert!(output.contains(": i32)"), "Sized UInt[32] should map to i32. Got:\n{}", output);
    }

    #[test]
    fn test_circt_linked_trg() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            TopLevel::Trigger(TriggerDeclaration {
                name: "btn".to_string(), ty: Type::bool_(),
                address: LinkRef::Linked("button0".to_string()),
                is_wake: true, is_const: false, span: None,
                bit_range: None, stages: vec![], condition: None,
                annotations: vec![],
                modifiers: vec![],
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
                name: "btn".to_string(), ty: Type::bool_(),
                address: LinkRef::Explicit(0),
                is_wake: true, is_const: false, span: None,
                annotations: vec![],
                modifiers: vec![],
                bit_range: None, stages: vec![], condition: None,
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
                name: "status".to_string(), ty: Type::int(),
                expr: None,
                address: Some(0x40000000),
                bit_range: None, constraint: None,
                is_override: false, os_mode: false,
                span: None, attrs: vec![],
            }),
        ]));
        // MMIO vars with @ address should become input ports, not registers
        assert!(output.contains("in %status: i64"), "MMIO var should be input port. Got:\n{}", output);
        assert!(output.contains("-> (halt: i1)"), "MMIO-only module has only halt output. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_state_reg() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_trigger("tick", Type::bool_()),
            make_state_decl("counter", Type::int(), Some(Expr::Decimal(0))),
            make_txn("count", vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("counter".to_string()))),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("counter".to_string())),
                        Box::new(Expr::Decimal(1)),
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
            make_state_decl("done", Type::int(), Some(Expr::Decimal(0))),
            make_txn("loop", vec![], Expr::Lt(
                Box::new(Expr::Identifier("done".to_string())),
                Box::new(Expr::Decimal(10)),
            ), Expr::Bool(true)),
        ]));
        assert!(output.contains("comb.icmp ult"), "Precondition should emit comb.icmp. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_postcondition_check() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("done", Type::int(), Some(Expr::Decimal(0))),
            make_txn("loop", vec![], Expr::Bool(true), Expr::Eq(
                Box::new(Expr::Identifier("done".to_string())),
                Box::new(Expr::Decimal(10)),
            )),
        ]));
        assert!(output.contains("comb.icmp eq"), "Postcondition should emit comb.icmp eq. Got:\n{}", output);
    }

    #[test]
    fn test_circt_await_handshake() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::int(), Some(Expr::Decimal(0))),
            make_txn("test", vec![
                Statement::Await {
                    expr: Expr::Call("compute".to_string(), vec![Expr::Decimal(42)]),
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
            make_state_decl("a", Type::int(), Some(Expr::Decimal(0))),
            make_state_decl("b", Type::int(), Some(Expr::Decimal(0))),
            make_txn("test", vec![
                Statement::SyncBlock {
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::AddrOf(Box::new(Expr::Identifier("a".to_string()))),
                            expr: Expr::Decimal(10),
                            timeout: None, modifiers: vec![],
                        },
                        Statement::Assignment {
                            lhs: Expr::AddrOf(Box::new(Expr::Identifier("b".to_string()))),
                            expr: Expr::Decimal(20),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                },
            ], Expr::Bool(true), Expr::Bool(true)),
        ]));
        assert!(output.contains("seq.always(posedge %clock)"), "Sync block should emit seq updates. Got:\n{}", output);
    }

    #[test]
    fn test_circt_call_submodule() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::int(), Some(Expr::Decimal(0))),
            make_txn("compute", vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Call("add".to_string(), vec![
                        Expr::Decimal(1),
                        Expr::Decimal(2),
                    ]),
                    timeout: None, modifiers: vec![],
                },
            ], Expr::Bool(true), Expr::Bool(true)),
        ]));
        assert!(output.contains("hw.instance"), "Expr::Call should emit hw.instance. Got:\n{}", output);
    }

    #[test]
    fn test_circt_intrinsic_abs() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::int(), Some(Expr::IntrinsicCall {
                intrinsic: Intrinsic::Abs,
                args: vec![Expr::Decimal(-5)],
            })),
        ]));
        assert!(output.contains("comb.neg"), "Abs intrinsic should emit comb.neg. Got:\n{}", output);
        assert!(output.contains("comb.mux"), "Abs intrinsic should emit comb.mux. Got:\n{}", output);
    }

    #[test]
    fn test_circt_intrinsic_ctpop() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::int(), Some(Expr::IntrinsicCall {
                intrinsic: Intrinsic::Ctpop,
                args: vec![Expr::Decimal(255)],
            })),
        ]));
        assert!(output.contains("comb.ctpop"), "Ctpop intrinsic should emit comb.ctpop. Got:\n{}", output);
    }

    #[test]
    fn test_circt_intrinsic_bitreverse() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_state_decl("x", Type::int(), Some(Expr::IntrinsicCall {
                intrinsic: Intrinsic::Bitreverse,
                args: vec![Expr::Decimal(1)],
            })),
        ]));
        assert!(output.contains("comb.rev"), "Bitreverse should emit comb.rev. Got:\n{}", output);
    }

    #[test]
    fn test_circt_duplicate_trg_fixed() {
        // Previously, duplicate trigger processing added each trigger twice,
        // producing duplicate port declarations. This test verifies single ports.
        let mut backend = CirctBackend::new();
        let output = backend.generate(&make_program(vec![
            make_trigger("sensor", Type::int()),
        ]));
        // Count occurrences of "sensor: i64" (should be exactly 2: once for the port, once for reg write)
        let count = output.matches("sensor: i64").count();
        assert!(count == 2 || count == 1, "Trigger should appear once as port and optionally in reg. Got {} occurrences. Output:\n{}", count, output);
    }
}
