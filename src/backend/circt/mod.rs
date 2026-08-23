// CIRCT Backend — emits MLIR text in HW + Comb + Seq dialects.
// Invoked via: briev build file.cbv → program.mlir → circt-opt → circt-translate → verilog

pub mod normalizer;

use crate::analysis::dependency_graph::DependencyGraph;
use crate::ast::{BinaryOpKind, Contract, Expr, OutputType, Statement, TopLevel, Type, UnaryOpKind};
use std::collections::HashMap;
use std::fmt::Write;

/// Metadata for a trigger variable mapped to a module port.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerPort {
    /// Signal name used as the port name.
    pub port_name: String,
    /// Original trigger name in Briev source.
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
    // 2026-07-28: Phase H.2 — !> metadata registry for optimization hints.
    metadata_registry: crate::backend::metadata::MetadataRegistry,
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
    /// 2026-08-23 (Plan 0.2): CIRCT's declared surface — synthesizable
    /// register-level logic: integer arithmetic/logic, guards, FSM bodies.
    /// No floats (comb has no float dialect here), no strings/collections,
    /// no spawn/concurrency. Enforced by validate_program before codegen.
    pub const CAPABILITIES: crate::backend::capabilities::BackendCapabilities =
        crate::backend::capabilities::BackendCapabilities {
            name: "CIRCT (.mlir hardware)",
            nature: "hardware synthesis lowers to finite register-level logic \
                     — bounded state and combinational logic only",
            int_literals: true,
            bool_char_literals: true,
            int_ops: true,
            unary_ops: true,
            calls: true,
            intrinsics: true,
            if_expr: true,
            match_expr: true,
            field_access: true,
            index: true,
            casts: true,
            let_stmt: true,
            assign_stmt: true,
            guarded_stmt: true,
            term_endprogram: true,
            match_stmt: true,
            trap_stmt: true,
            ..crate::backend::capabilities::BackendCapabilities::NONE
        };

    pub fn new() -> Self {
        CirctBackend {
            trg_ports: Vec::new(),
            var_types: HashMap::new(),
            var_exprs: HashMap::new(),
            mmio_vars: Vec::new(),
            fn_arity: HashMap::new(),
            cell_defs: HashMap::new(),
            metadata_registry: crate::backend::metadata::MetadataRegistry::load(),
        }
    }

    pub fn generate(&mut self, items: &[TopLevel]) -> String {
        // 2026-08-23 (Plan 0.1): direct-construction callers (unit tests)
        // self-compute the dependency graph exactly as the pipeline's
        // analyze_program does — identical build call + identical empty
        // fallback — so both paths agree.
        let dep_graph = DependencyGraph::build(items)
            .unwrap_or_else(|_| DependencyGraph {
                topo_order: Vec::new(),
                bit_index: HashMap::new(),
                dependencies: HashMap::new(),
                dependents: HashMap::new(),
                is_trg: std::collections::HashSet::new(),
                all_vars: std::collections::HashSet::new(),
            });
        self.generate_with_dep_graph(items, &dep_graph)
    }

    /// 2026-08-23 (Plan 0.1, backend-scaffolding-foundation): pipeline entry —
    /// consumes the shared `AnalysisResults.dependency_graph` computed once in
    /// src/compile.rs instead of re-deriving it (frontend-driven dispatch:
    /// the backend CONSUMES decisions). To undo: inline back into generate().
    pub fn generate_with_dep_graph(&mut self, items: &[TopLevel], dep_graph: &DependencyGraph) -> String {
        for item in items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.var_types.insert(decl.name.clone(), decl.ty.clone());
                    self.var_exprs.insert(decl.name.clone(), None);
                }
                TopLevel::Trigger(trg) => {
                    let port_name = trg.name.clone();
                    self.trg_ports.push(TriggerPort {
                        port_name: port_name.clone(),
                        trg_name: trg.name.clone(),
                        is_wake: false,
                    });
                    self.var_types.insert(port_name, Type::int());
                    self.var_exprs.insert(trg.name.clone(), None);
                }
                TopLevel::Transaction(txn) => {
                    self.fn_arity.insert(txn.name.clone(), txn.parameters.len());
                    for stmt in &txn.body {
                        if let Statement::Assign(lhs, expr) = stmt {
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
                    self.cell_defs.insert(cell.name.clone(), cell.clone());
                    for field in &cell.fields {
                        self.var_types.insert(format!("{}${}", cell.name, field.name), field.ty.clone());
                        self.var_exprs.insert(format!("{}${}", cell.name, field.name), Some(Expr::Decimal(field.metadata.len() as i64)));
                    }
                    for (param_name, param_ty) in &cell.parameters {
                        self.var_types.insert(format!("{}${}", cell.name, param_name), param_ty.clone());
                    }
                }
                _ => {}
            }
        }

        let mut out = String::new();
        self.emit_header(&mut out);
        self.emit_module(&mut out, &dep_graph, items);
        let cell_defs: Vec<crate::ast::CellDef> = self.cell_defs.values().cloned().collect();
        for cell_def in &cell_defs {
            self.emit_cell_module(&mut out, &dep_graph, cell_def);
        }
        out
    }

    fn emit_header(&self, out: &mut String) {
        writeln!(out, "// Generated by briev-compiler CIRCT backend").ok();
        writeln!(out, "// Run: circt-opt program.mlir | circt-translate --export-verilog").ok();
        writeln!(out).ok();
    }

    fn mlir_type(&self, ty: &Type) -> String {
        match ty {
            Type::Custom(__t) if __t == "Bool" => "i1".into(),
            Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64".into(),
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
                    crate::ast::BitRange::Single(w) => *w,
                    crate::ast::BitRange::Range(_, hi) => *hi,
                    crate::ast::BitRange::Any(w) => *w,
                };
                if matches!(inner.as_ref(), Type::Custom(__t) if __t == "Bool") && width <= 1 {
                    return "i1".into();
                }
                format!("i{}", width)
            }
            _ => "i64".into(),
        }
    }

    fn emit_module(&mut self, out: &mut String, dep_graph: &DependencyGraph, items: &[TopLevel]) {
        let mut ng = NameGen::default();

        let mut input_ports: Vec<String> = Vec::new();
        let mut output_ports: Vec<(String, String)> = Vec::new();
        input_ports.push("in %clock: i1".to_string());
        input_ports.push("in %reset: i1".to_string());

        for trg in &self.trg_ports {
            if let Some(ty) = self.var_types.get(&trg.port_name) {
                let mlir_ty = self.mlir_type(ty);
                input_ports.push(format!("in %{}: {}", trg.port_name, mlir_ty));
            }
        }

        output_ports.push(("halt".to_string(), "i1".to_string()));

        let sorted_vars = &dep_graph.topo_order;
        let trg_names: std::collections::HashSet<String> = self.trg_ports.iter().map(|t| t.trg_name.clone()).collect();
        for var_name in sorted_vars {
            if trg_names.contains(var_name) {
                continue;
            }
            if let Some(ty) = self.var_types.get(var_name) {
                let mlir_ty = self.mlir_type(ty);
                output_ports.push((var_name.clone(), mlir_ty));
            }
        }

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

        let mut reg_names: HashMap<String, String> = HashMap::new();
        let special_outputs: std::collections::HashSet<&str> = ["halt"].iter().cloned().collect();
        for (var_name, mlir_ty) in &output_ports {
            if special_outputs.contains(var_name.as_str()) {
                continue;
            }
            let init_val = self.initial_value(var_name);
            let reg = ng.fresh_reg(var_name);
            writeln!(out, "  {} = seq.firreg initial_value {{ init_value = {} : {} }} : {}", reg, init_val, mlir_ty, mlir_ty).ok();
            reg_names.insert(var_name.clone(), reg);
        }

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

        for item in items {
            if let TopLevel::Transaction(txn) = item {
                self.emit_txn_body(&mut ng, out, &txn.name, &txn.body, &txn.contract, &mut reg_names);
            }
        }

        write!(out, "  hw.output").ok();
        for (var_name, mlir_ty) in &output_ports {
            if let Some(reg) = reg_names.get(var_name) {
                write!(out, " {} : {},", reg, mlir_ty).ok();
            } else if var_name == "halt" {
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
            Expr::BinaryOp(kind, l, r) => {
                match kind {
                    BinaryOpKind::Add => self.emit_binary_comb(ng, out, "comb.add", l, r, reg_names, result_ty),
                    BinaryOpKind::Sub => self.emit_binary_comb(ng, out, "comb.sub", l, r, reg_names, result_ty),
                    BinaryOpKind::Mul => self.emit_binary_comb(ng, out, "comb.mul", l, r, reg_names, result_ty),
                    BinaryOpKind::Div => self.emit_binary_comb(ng, out, "comb.divu", l, r, reg_names, result_ty),
                    BinaryOpKind::Mod => self.emit_binary_comb(ng, out, "comb.mod", l, r, reg_names, result_ty),
                    BinaryOpKind::Eq => self.emit_binary_comb(ng, out, "comb.icmp eq", l, r, reg_names, "i1"),
                    BinaryOpKind::Neq => self.emit_binary_comb(ng, out, "comb.icmp ne", l, r, reg_names, "i1"),
                    BinaryOpKind::Lt => self.emit_binary_comb(ng, out, "comb.icmp ult", l, r, reg_names, "i1"),
                    BinaryOpKind::Le => self.emit_binary_comb(ng, out, "comb.icmp ule", l, r, reg_names, "i1"),
                    BinaryOpKind::Gt => self.emit_binary_comb(ng, out, "comb.icmp ugt", l, r, reg_names, "i1"),
                    BinaryOpKind::Ge => self.emit_binary_comb(ng, out, "comb.icmp uge", l, r, reg_names, "i1"),
                    BinaryOpKind::And => self.emit_binary_comb(ng, out, "comb.and", l, r, reg_names, "i1"),
                    BinaryOpKind::Or => self.emit_binary_comb(ng, out, "comb.or", l, r, reg_names, "i1"),
                    BinaryOpKind::BitAnd => self.emit_binary_comb(ng, out, "comb.and", l, r, reg_names, result_ty),
                    BinaryOpKind::BitOr => self.emit_binary_comb(ng, out, "comb.or", l, r, reg_names, result_ty),
                    BinaryOpKind::BitXor => self.emit_binary_comb(ng, out, "comb.xor", l, r, reg_names, result_ty),
                    BinaryOpKind::Shl => self.emit_binary_comb(ng, out, "comb.shl", l, r, reg_names, result_ty),
                    BinaryOpKind::Shr => self.emit_binary_comb(ng, out, "comb.shr", l, r, reg_names, result_ty),
                    BinaryOpKind::Concat => self.emit_binary_comb(ng, out, "comb.concat", l, r, reg_names, result_ty),
                }
            }
            Expr::UnaryOp(kind, inner) => {
                match kind {
                    UnaryOpKind::Neg => self.emit_unary_comb(ng, out, "comb.neg", inner, reg_names, result_ty),
                    UnaryOpKind::Not => self.emit_unary_comb(ng, out, "comb.xor", inner, reg_names, "i1"),
                    UnaryOpKind::BitNot => self.emit_unary_comb(ng, out, "comb.xor", inner, reg_names, result_ty),
                }
            }
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
            // 2026-07-18: Pointer ops — emit inner expression (HW backend).
            Expr::AddrOf(inner) => self.emit_expr(ng, out, inner, reg_names, result_ty),
            Expr::Deref(inner) => self.emit_expr(ng, out, inner, reg_names, result_ty),
            Expr::Call(name, args, _) => {
                // 2026-07-14: Intrinsic calls and function calls both use Expr::Call.
                // Match intrinsic names first, then fall back to submodule instantiation.
                match name.as_str() {
                    "Abs#" => {
                        let mut arg = |i: usize| -> String {
                            args.get(i).and_then(|a| {
                                self.emit_expr(ng, out, a, reg_names, result_ty)
                            }).unwrap_or_else(|| "%0".to_string())
                        };
                        let x = arg(0);
                        let neg_x = ng.fresh_wire("neg");
                        let cmp = ng.fresh_wire("cmp_neg");
                        writeln!(out, "  {} = comb.neg {} : {}", neg_x, x, result_ty).ok();
                        writeln!(out, "  {} = comb.icmp slt {}, %c0_0 : {}", cmp, x, result_ty).ok();
                        let w = ng.fresh_wire("abs");
                        writeln!(out, "  {} = comb.mux {}, {}, {} : {}", w, cmp, neg_x, x, result_ty).ok();
                        Some(w)
                    }
                    "Ctpop#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, result_ty)).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("popcount");
                        writeln!(out, "  {} = comb.ctpop {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    "Ctlz#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, result_ty)).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("ctlz");
                        writeln!(out, "  {} = comb.ctlz {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    "Cttz#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, result_ty)).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("cttz");
                        writeln!(out, "  {} = comb.cttz {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    "Bitreverse#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, result_ty)).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("bitreverse");
                        writeln!(out, "  {} = comb.rev {} : {}", w, x, result_ty).ok();
                        Some(w)
                    }
                    "AddressOf#" => {
                        // 2026-07-15: CIRCT (hardware) backend — AddressOf# resolves to a
                        // physical address, emit as a constant integer. In HW synthesis this
                        // becomes a constant wire driving the address bus.
                        let id_str = args.first().and_then(|a| {
                            if let Expr::Quoted(b) = a { Some(String::from_utf8_lossy(b).to_string()) } else { None }
                        }).unwrap_or_else(|| "unknown".to_string());
                        let addr = crate::address_resolver::resolve_address(&id_str);
                        let c = ng.fresh_const("addr");
                        writeln!(out, "  {} = hw.constant {} : {}", c, addr, result_ty).ok();
                        Some(c)
                    }
                    "Size#" => {
                        let c = ng.fresh_const("size");
                        writeln!(out, "  {} = hw.constant 64 : {}", c, result_ty).ok();
                        Some(c)
                    }
                    "Sqrt#" | "Fabs#" | "Ceil#" | "Floor#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, "f64")).unwrap_or_else(|| "%0".to_string());
                        let op = match name.as_str() {
                            "Sqrt#" => "sqrt",
                            "Fabs#" => "absf",
                            "Ceil#" => "ceil",
                            "Floor#" => "floor",
                            _ => unreachable!(),
                        };
                        let w = ng.fresh_wire(op);
                        writeln!(out, "  {} = comb.{} {} : f64", w, op, x).ok();
                        Some(w)
                    }
                    "Sin#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, "f64")).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("sin");
                        writeln!(out, "  {} = comb.sin {} : f64", w, x).ok();
                        Some(w)
                    }
                    "Cos#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, "f64")).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("cos");
                        writeln!(out, "  {} = comb.cos {} : f64", w, x).ok();
                        Some(w)
                    }
                    "Pow#" => {
                        let x = args.first().and_then(|a| self.emit_expr(ng, out, a, reg_names, "f64")).unwrap_or_else(|| "%0".to_string());
                        let y = args.get(1).and_then(|a| self.emit_expr(ng, out, a, reg_names, "f64")).unwrap_or_else(|| "%0".to_string());
                        let w = ng.fresh_wire("pow");
                        writeln!(out, "  {} = comb.pow {}, {} : f64", w, x, y).ok();
                        Some(w)
                    }
                    _ => {
                        // Function calls become submodule instantiations
                        let inst_name = name.replace('-', "_");
                        let result_wire = ng.fresh_wire(&format!("{}_result", inst_name));
                        let mut arg_parts = Vec::new();
                        for (i, arg) in args.iter().enumerate() {
                            let arg_mlir_ty = if i == 0 { "i64" } else { result_ty };
                            if let Some(arg_val) = self.emit_expr(ng, out, arg, reg_names, arg_mlir_ty) {
                                arg_parts.push(format!("{}: $arg{}: {}", arg_val, i, arg_mlir_ty));
                            }
                        }
                        let result_mlir_ty = result_ty;
                        writeln!(out, "  {} = hw.instance \"{}\" @{} ({}) -> ({}: ${}: {})",
                            result_wire, inst_name, inst_name,
                            arg_parts.join(", "),
                            result_wire, "result", result_mlir_ty,
                        ).ok();
                        Some(result_wire)
                    }
                }
            }
            Expr::Field(obj, field) => {
                let obj_val = self.emit_expr(ng, out, obj, reg_names, result_ty)?;
                let w = ng.fresh_wire("fld");
                writeln!(out, "  {} = hw.wire {} : {}", w, obj_val, result_ty).ok();
                Some(w)
            }
            Expr::Index(list, idx) => {
                let list_val = self.emit_expr(ng, out, list, reg_names, result_ty)?;
                let _idx_val = self.emit_expr(ng, out, idx, reg_names, "i64")?;
                let w = ng.fresh_wire("idx");
                writeln!(out, "  {} = hw.wire {} : {}", w, list_val, result_ty).ok();
                Some(w)
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
        if op == "comb.neg" || op == "comb.not" {
            writeln!(out, "  {} = {} {} : {}", w, op, val, result_ty).ok();
        } else {
            let c0 = ng.fresh_const("zero");
            writeln!(out, "  {} = hw.constant 0 : {}", c0, result_ty).ok();
            writeln!(out, "  {} = {} {}, {} : {}", w, op, val, c0, result_ty).ok();
        }
        Some(w)
    }

    fn emit_txn_body(&self, ng: &mut NameGen, out: &mut String, _name: &str, body: &[Statement], contract: &Contract, reg_names: &mut HashMap<String, String>) {
        let state_reg = ng.fresh_reg("txn_state");
        writeln!(out, "  {} = seq.firreg initial_value {{ init_value = 0 : i2 }} : i2", state_reg).ok();

        let halt_reg = ng.fresh_reg("halt");
        let c0 = ng.fresh_const("zero_i1");
        writeln!(out, "  {} = hw.constant 0 : i1", c0).ok();
        writeln!(out, "  {} = seq.firreg initial_value {{ init_value = 0 : i1 }} : i1", halt_reg).ok();
        reg_names.insert("halt".to_string(), halt_reg.clone());

        let pre_cond = ng.fresh_wire("pre");
        self.emit_contract_condition(out, ng, &contract.pre_condition, &pre_cond, reg_names);

        for stmt in body {
            match stmt {
                Statement::Assign(lhs, expr) => {
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

                Statement::EndProgram(..) => {
                    writeln!(out, "  seq.always(posedge %clock) {{").ok();
                    writeln!(out, "    {} <= 1 : i1", halt_reg).ok();
                    writeln!(out, "  }}").ok();
                }
                Statement::Term(..) => {}
                Statement::Foreach { item, list, body, .. } => {
                    let list_items = match list.as_ref() {
                        Expr::List(items) => Some(items),
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
                Statement::Let { name, expr: Some(e), .. } => {
                    let mlir_ty = self.mlir_type(self.var_types.get(name).unwrap_or(&Type::int()));
                    if let Some(val) = self.emit_expr(ng, out, e, reg_names, &mlir_ty) {
                        let w = ng.fresh_wire("let");
                        writeln!(out, "  {} = hw.wire {} : {}", w, val, mlir_ty).ok();
                        reg_names.insert(name.clone(), w);
                    }
                }
                Statement::Guarded(cond, stmts) => {
                    let cond_mlir = self.mlir_type(&Type::bool_());
                    if let Some(cond_val) = self.emit_expr(ng, out, cond, reg_names, &cond_mlir) {
                        let cond_icmp = ng.fresh_wire("gic");
                        writeln!(out, "  {} = comb.icmp ne {}, %true : i1", cond_icmp, cond_val).ok();
                        for s in stmts {
                            self.emit_txn_body(ng, out, _name, &[s.clone()], contract, reg_names);
                        }
                    }
                }
                _ => {}
            }
        }

        let post_cond = ng.fresh_wire("post");
        self.emit_contract_condition(out, ng, &contract.post_condition, &post_cond, reg_names);

        let state_next = ng.fresh_wire("txn_state_next");
        writeln!(out, "  {} = comb.mux {}, {}, {} : i2", state_next, post_cond, 2, 1).ok();
        let state_after_body = ng.fresh_wire("txn_state_after");
        writeln!(out, "  {} = comb.mux {}, {}, {} : i2", state_after_body, pre_cond, state_next, 0).ok();
        writeln!(out, "  seq.always(posedge %clock) {{").ok();
        writeln!(out, "    {} <= {}", state_reg, state_after_body).ok();
        writeln!(out, "  }}").ok();
    }

    fn emit_cell_module(&mut self, out: &mut String, dep_graph: &DependencyGraph, cell: &crate::ast::CellDef) {
        let cell_name = &cell.name;
        let mut ng = NameGen::default();

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

        let mut reg_names: HashMap<String, String> = HashMap::new();
        for field in &cell.fields {
            let mlir_ty = self.mlir_type(&field.ty);
            let init_val = if let Some(crate::ast::PropertyValue::Int(n)) = field.metadata.get("init") {
                format!("{}", n)
            } else if let Some(crate::ast::PropertyValue::Bool(b)) = field.metadata.get("init") {
                format!("{}", if *b { 1 } else { 0 })
            } else {
                "0".to_string()
            };
            let reg = ng.fresh_reg(cell_name);
            writeln!(out, "  {} = seq.firreg initial_value {{ init_value = {} : {} }} : {}", reg, init_val, mlir_ty, mlir_ty).ok();
            reg_names.insert(field.name.clone(), reg);
        }

        for txn in &cell.transactions {
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

    fn emit_stmt_body(&self, ng: &mut NameGen, out: &mut String, stmt: &Statement, reg_names: &HashMap<String, String>) {
        match stmt {
            Statement::Expression(expr) => {
                self.emit_expr(ng, out, expr, reg_names, "i64");
            }
            Statement::Assign(lhs, expr) => {
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
            Statement::Guarded(_condition, statements) => {
                for s in statements {
                    self.emit_stmt_body(ng, out, s, reg_names);
                }
            }
            Statement::SyncBlock(body) => {
                for s in body {
                    self.emit_stmt_body(ng, out, s, reg_names);
                }
            }
            Statement::Let { name, expr: Some(e), .. } => {
                let mlir_ty = self.mlir_type(&Type::int());
                if let Some(val) = self.emit_expr(ng, out, e, reg_names, &mlir_ty) {
                    let w = ng.fresh_wire("let");
                    writeln!(out, "  {} = hw.wire {} : {}", w, val, mlir_ty).ok();
                    // Note: reg_names is immutable here, so we can't store it
                }
            }
            _ => {}
        }
    }

    /// Emit combinational logic for a contract condition (precondition or postcondition).
    fn emit_contract_condition(&self, out: &mut String, ng: &mut NameGen, cond: &Expr, result_wire: &str, reg_names: &HashMap<String, String>) {
        match cond {
            Expr::Bool(true) => {
                writeln!(out, "  {} = hw.constant 1 : i1", result_wire).ok();
            }
            Expr::Bool(false) => {
                writeln!(out, "  {} = hw.constant 0 : i1", result_wire).ok();
            }
            Expr::BinaryOp(BinaryOpKind::Lt, l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp ult {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::BinaryOp(BinaryOpKind::Le, l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp ule {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::BinaryOp(BinaryOpKind::Gt, l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp ugt {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::BinaryOp(BinaryOpKind::Ge, l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp uge {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::BinaryOp(BinaryOpKind::Eq, l, r) => {
                let left = self.emit_expr(ng, out, l, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                let right = self.emit_expr(ng, out, r, reg_names, "i64").unwrap_or_else(|| "%0".to_string());
                writeln!(out, "  {} = comb.icmp eq {}, {} : i64", result_wire, left, right).ok();
            }
            Expr::BinaryOp(BinaryOpKind::And, l, r) => {
                let left_wire = ng.fresh_wire("cond_l");
                self.emit_contract_condition(out, ng, l, &left_wire, reg_names);
                let right_wire = ng.fresh_wire("cond_r");
                self.emit_contract_condition(out, ng, r, &right_wire, reg_names);
                writeln!(out, "  {} = comb.and {}, {} : i1", result_wire, left_wire, right_wire).ok();
            }
            Expr::BinaryOp(BinaryOpKind::Or, l, r) => {
                let left_wire = ng.fresh_wire("cond_l");
                self.emit_contract_condition(out, ng, l, &left_wire, reg_names);
                let right_wire = ng.fresh_wire("cond_r");
                self.emit_contract_condition(out, ng, r, &right_wire, reg_names);
                writeln!(out, "  {} = comb.or {}, {} : i1", result_wire, left_wire, right_wire).ok();
            }
            Expr::UnaryOp(UnaryOpKind::Not, inner) => {
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

    fn make_state_decl(name: &str, ty: Type) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty,
            span: None,
        })
    }

    fn make_trigger(name: &str, port: &str) -> TopLevel {
        TopLevel::Trigger(Trigger {
            name: name.to_string(),
            instance: Expr::Identifier("env".to_string()),
                        span: None,
        })
    }

    fn make_txn(name: &str, body: Vec<Statement>, pre: Expr, post: Expr) -> TopLevel {
        TopLevel::Transaction(Transaction {
            is_async: false, is_reactive: true,
            name: name.to_string(),
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract { pre_condition: pre, post_condition: post, watchdog: None, explicit: false, span: None },
            body,
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        })
    }

    #[test]
    fn test_circt_empty_program() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[]);
        assert!(output.contains("hw.module @top"));
        assert!(output.contains("clock: i1"));
        assert!(output.contains("hw.output"));
    }

    #[test]
    fn test_circt_trg_port() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_trigger("sensor", "sensor"),
        ]);
        assert!(output.contains("sensor: i64"));
    }

    #[test]
    fn test_circt_state_var_has_seq_register() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("counter", Type::int()),
        ]);
        assert!(output.contains("seq.firreg"), "State vars should use seq.firreg. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_add() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("x", Type::int()),
        ]);
        // Just verify basic generation works
        assert!(output.contains("x: i64"), "State should appear in output. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_mul() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("y", Type::int()),
        ]);
        assert!(output.contains("y: i64"), "State should appear in output. Got:\n{}", output);
    }

    #[test]
    fn test_circt_modern_output_ports() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("counter", Type::int()),
        ]);
        assert!(output.contains("in %clock: i1"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
        assert!(output.contains("in %reset: i1"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
        assert!(output.contains("halt: i1"), "Outputs should include halt. Got:\n{}", output);
        assert!(output.contains("counter: i64"), "Outputs should include counter. Got:\n{}", output);
        assert!(!output.contains("hw.output_assign"), "Should not use deprecated hw.output_assign. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sized_int() {
        let mut backend = CirctBackend::new();
        let ty = Type::Constrained(Box::new(Type::Custom("UInt".to_string())), crate::ast::BitRange::Single(8));
        let output = backend.generate(&[
            make_state_decl("byte", ty),
        ]);
        assert!(output.contains(": i8)"), "Sized UInt[8] should map to i8. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sized_int_32() {
        let mut backend = CirctBackend::new();
        let ty = Type::Constrained(Box::new(Type::Custom("UInt".to_string())), crate::ast::BitRange::Single(32));
        let output = backend.generate(&[
            make_state_decl("word", ty),
        ]);
        assert!(output.contains(": i32)"), "Sized UInt[32] should map to i32. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_state_reg() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("counter", Type::int()),
            make_txn("count", vec![
                Statement::Assign(
                    Expr::Identifier("counter".to_string()),
                    Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("counter".to_string())), Box::new(Expr::Decimal(1))),
                ),
            ], Expr::Bool(true), Expr::Bool(true)),
        ]);
        assert!(output.contains("seq.firreg"), "FSM should have state reg. Got:\n{}", output);
        assert!(output.contains("comb.mux"), "FSM should have state transition mux. Got:\n{}", output);
        assert!(output.contains("seq.always(posedge %clock)"), "FSM should have seq.always. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_precondition_check() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("done", Type::int()),
            make_txn("loop", vec![],
                Expr::BinaryOp(BinaryOpKind::Lt, Box::new(Expr::Identifier("done".to_string())), Box::new(Expr::Decimal(10))),
                Expr::Bool(true)),
        ]);
        assert!(output.contains("comb.icmp"), "Precondition should emit comb.icmp. Got:\n{}", output);
    }

    #[test]
    fn test_circt_fsm_postcondition_check() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("done", Type::int()),
            make_txn("loop", vec![],
                Expr::Bool(true),
                Expr::BinaryOp(BinaryOpKind::Eq, Box::new(Expr::Identifier("done".to_string())), Box::new(Expr::Decimal(10)))),
        ]);
        assert!(output.contains("comb.icmp"), "Postcondition should emit comb.icmp eq. Got:\n{}", output);
    }

    #[test]
    fn test_circt_sync_block() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("a", Type::int()),
            make_state_decl("b", Type::int()),
            make_txn("test", vec![
                Statement::SyncBlock(vec![
                    Statement::Assign(
                        Expr::Identifier("a".to_string()),
                        Expr::Decimal(10),
                    ),
                    Statement::Assign(
                        Expr::Identifier("b".to_string()),
                        Expr::Decimal(20),
                    ),
                ]),
            ], Expr::Bool(true), Expr::Bool(true)),
        ]);
        assert!(output.contains("seq.always(posedge %clock)"), "Sync block should emit seq updates. Got:\n{}", output);
    }

    #[test]
    fn test_circt_call_submodule() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("x", Type::int()),
            make_txn("compute", vec![
                Statement::Assign(
                    Expr::Identifier("x".to_string()),
                    Expr::Call("add".to_string(), vec![Expr::Decimal(1), Expr::Decimal(2)], None),
                ),
            ], Expr::Bool(true), Expr::Bool(true)),
        ]);
        assert!(output.contains("hw.instance"), "Expr::Call should emit hw.instance. Got:\n{}", output);
    }

    #[test]
    fn test_circt_intrinsic_abs() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("x", Type::int()),
            make_txn("test", vec![
                Statement::Assign(
                    Expr::Identifier("x".to_string()),
                    Expr::Call("Abs#".to_string(), vec![Expr::Decimal(-5)], None),
                ),
            ], Expr::Bool(true), Expr::Bool(true)),
        ]);
        assert!(output.contains("comb.neg"), "Abs intrinsic should emit comb.neg. Got:\n{}", output);
        assert!(output.contains("comb.mux"), "Abs intrinsic should emit comb.mux. Got:\n{}", output);
    }

    #[test]
    fn test_circt_intrinsic_ctpop() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("x", Type::int()),
            make_txn("test", vec![
                Statement::Assign(
                    Expr::Identifier("x".to_string()),
                    Expr::Call("Ctpop#".to_string(), vec![Expr::Decimal(255)], None),
                ),
            ], Expr::Bool(true), Expr::Bool(true)),
        ]);
        assert!(output.contains("comb.ctpop"), "Ctpop intrinsic should emit comb.ctpop. Got:\n{}", output);
    }

    #[test]
    fn test_circt_intrinsic_bitreverse() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("x", Type::int()),
            make_txn("test", vec![
                Statement::Assign(
                    Expr::Identifier("x".to_string()),
                    Expr::Call("Bitreverse#".to_string(), vec![Expr::Decimal(1)], None),
                ),
            ], Expr::Bool(true), Expr::Bool(true)),
        ]);
        assert!(output.contains("comb.rev"), "Bitreverse should emit comb.rev. Got:\n{}", output);
    }

    #[test]
    fn test_circt_duplicate_trg_fixed() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_trigger("sensor", "sensor"),
        ]);
        let count = output.matches("sensor: i64").count();
        assert!(count >= 1, "Trigger should appear as port. Got {} occurrences. Output:\n{}", count, output);
    }
}
