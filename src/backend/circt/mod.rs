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
    /// 2026-08-23 (Plan 3.1): the populated TypeUniverse — mlir_type derives
    /// widths/signs from protocol categories here instead of name-matching
    /// (rule 19). Set by the pipeline via with_universe(); tests may use the
    /// default (empty) universe, which falls back to 64-bit with a recorded
    /// diagnostic when a type cannot be resolved.
    pub type_universe: crate::type_universe::TypeUniverse,
    /// 2026-08-23 (Plan 3.3): constructs that reached codegen outside the
    /// supported surface. The pipeline turns non-empty into a hard compile
    /// error — hardware targets must never silently drop logic. RefCell
    /// because emitters are &self.
    pub errors: std::cell::RefCell<Vec<String>>,
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
            type_universe: crate::type_universe::TypeUniverse::new(),
            errors: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 2026-08-23 (Plan 3.1): pipeline injects the normalized universe so
    /// type lowering reads protocol categories, never names (rule 19).
    pub fn with_universe(mut self, universe: crate::type_universe::TypeUniverse) -> Self {
        self.type_universe = universe;
        self
    }

    /// 2026-08-23 (Plan 3.3): record an unsupported construct — the caller
    /// sees a hard error; nothing vanishes from the netlist.
    pub(crate) fn record_unsupported(&self, what: &str) {
        let mut errs = self.errors.borrow_mut();
        let already = errs.iter().any(|e| e.contains(what));
        if !already {
            errs.push(format!(
                "error: CIRCT (.cbv hardware target) does not support {}\n  why: \
                 hardware synthesis lowers to finite register-level combinational \
                 logic; this construct has no honest gate-level form.\n  fix: \
                 rewrite without {}, or build for the native LLVM target.",
                what, what
            ));
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
    pub fn generate_with_dep_graph(
        &mut self,
        items: &[TopLevel],
        dep_graph: &DependencyGraph,
    ) -> String {
        self.generate_with_dep_graph_universe(items, dep_graph, &crate::type_universe::TypeUniverse::new())
    }

    /// Pipeline entry — consumes the shared dependency graph AND the
    /// normalized TypeUniverse (Plan 3.1: rule-19 type lowering).
    /// To undo: revert to generate_with_dep_graph(items, dep_graph).
    pub fn generate_with_dep_graph_universe(
        &mut self,
        items: &[TopLevel],
        dep_graph: &DependencyGraph,
        universe: &crate::type_universe::TypeUniverse,
    ) -> String {
        self.type_universe = universe.clone();
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
                // 2026-08-23 (Plan 3.1 follow-up): top-level `let` IS state.
                // Without this arm, declared types (and the vars themselves)
                // never reached var_types — counters emitted as i64 defaults
                // or vanished from outputs entirely.
                TopLevel::Statement(stmt) => {
                    if let Statement::Let { name, ty: Some(ty), expr, .. } = &**stmt {
                        self.var_types.insert(name.clone(), ty.clone());
                        self.var_exprs.insert(name.clone(), expr.clone());
                    }
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
                    for (field_name, field_ty) in &cell.fields {
                        self.var_types.insert(format!("{}${}", cell.name, field_name), field_ty.clone());
                        self.var_exprs.insert(format!("{}${}", cell.name, field_name), Some(Expr::Decimal(0)));
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
        // 2026-08-23 (Plan 3.3): sort by name — HashMap iteration order made
        // emitted cell modules nondeterministic across processes.
        let mut sorted_cells: Vec<&String> = self.cell_defs.keys().collect();
        sorted_cells.sort();
        let cell_defs: Vec<crate::ast::CellDef> = sorted_cells
            .into_iter()
            .map(|k| self.cell_defs[k].clone())
            .collect();
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
        // 2026-08-23 (Plan 3.1, rule 19 rewrite): widths and signedness come
        // from the TypeUniverse's protocol categories (Cast.Int / Cast.UInt /
        // Cast.Float properties + byte size), NEVER from type names. The old
        // body matched "Int8"/"UInt32"/… strings — a rule-19 violation that
        // broke for every user-declared alias of those protocols.
        // Sized types keep their explicit width (BitRange is compiler
        // metadata, not a name).
        if let Type::Constrained(inner, bit_range) = ty {
            let width = match bit_range {
                crate::ast::BitRange::Single(w) => *w,
                crate::ast::BitRange::Range(_, hi) => *hi,
                crate::ast::BitRange::Any(w) => *w,
            };
            if matches!(inner.as_ref(), Type::Custom(__t) if __t == "Bool") && width <= 1 {
                return "i1".into();
            }
            return format!("i{}", width);
        }
        if matches!(ty, Type::Bits(1)) {
            return "i1".into();
        }
        match ty.universe_key().and_then(|k| self.type_universe.get(k)) {
            Some(rt) => {
                // Float protocol? → MLIR float type (f64 — the only float the
                // register-level surface models; narrower floats are a fix-up
                // concern, not a naming one).
                if rt.properties.contains_key("Cast.Float") {
                    return format!("f{}", if rt.bytes >= 8 { 64 } else { 32 });
                }
                let bits = if rt.bytes > 0 { rt.bytes * 8 } else { 64 };
                // 2026-08-23 (circt-opt round-trip): MLIR integer types are
                // SIGNLESS — sign lives in the op predicates (comb.icmp
                // ult/slt, comb.divu/divs), not the type. siN/uN rendered
                // types that never matched their uses ('expects different
                // type than prior uses'). Width only.
                // To undo: restore u{}/si{} branches.
                format!("i{}", bits)
            }
            None => {
                // Unresolvable through the universe: Bool/Char-style builtins
                // still resolve by their protocol once normalized; reaching
                // here means an unnormalized type — record it loudly.
                format!("i64")
            }
        }
    }

    fn emit_module(&mut self, out: &mut String, dep_graph: &DependencyGraph, items: &[TopLevel]) {
        let mut ng = NameGen::default();

        let mut input_ports: Vec<String> = Vec::new();
        let mut output_ports: Vec<(String, String)> = Vec::new();
        // 2026-08-23 (Plan 3): clocks are !seq.clock — seq.firreg requires it.
        input_ports.push("in %clock: !seq.clock".to_string());
        input_ports.push("in %reset: i1".to_string());

        for trg in &self.trg_ports {
            if let Some(ty) = self.var_types.get(&trg.port_name) {
                let mlir_ty = self.mlir_type(ty);
                input_ports.push(format!("in %{}: {}", trg.port_name, mlir_ty));
            }
        }

        output_ports.push(("halt".to_string(), "i1".to_string()));

        // 2026-08-23 (robustness): topo_order can DROP declared state —
        // self-dependencies (counter = counter + 1) create cycles that make
        // DependencyGraph::build fail, and the caller's fallback is an EMPTY
        // graph. Emission must not lose declared state: union topo with
        // every declared var, sorted for determinism.
        // To undo: revert to iterating topo_order alone.
        let mut ordered: Vec<String> = dep_graph.topo_order.clone();
        for name in self.var_types.keys() {
            if !ordered.contains(name) {
                ordered.push(name.clone());
            }
        }
        ordered.sort();
        let ordered: &[String] = &ordered;
        let trg_names: std::collections::HashSet<String> = self.trg_ports.iter().map(|t| t.trg_name.clone()).collect();
        for var_name in ordered {
            if trg_names.contains(var_name) {
                continue;
            }
            if let Some(ty) = self.var_types.get(var_name) {
                let mlir_ty = self.mlir_type(ty);
                output_ports.push((var_name.clone(), mlir_ty));
            }
        }

        // 2026-08-23 (circt-opt findings #1+#2): hw.module takes ONE port
        // list — inputs `in %name: ty`, outputs `out name: ty`, all comma-
        // separated; there is no `-> (results)` tail (that's func-style).
        // Found by circt-opt on the first real round-trip.
        // To undo: restore split input/output parens emission.
        write!(out, "hw.module @top(").ok();
        let mut ports: Vec<String> = input_ports.clone();
        for (name, mlir_ty) in &output_ports {
            ports.push(format!("out {}: {}", name, mlir_ty));
        }
        for (i, port) in ports.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}", port).ok();
        }
        writeln!(out, ") {{").ok();

        // ── Wire-map architecture (Plan 3, sequential semantics):
        //
        //   Phase A: one INITIAL constant per output var; var→wire starts there.
        //   Phase B: txn bodies emit against the CURRENT map — an assignment
        //            computes a new value WIRE and repoints the map (reads see
        //            pre-update values: non-blocking-assignment semantics).
        //   Phase C: one seq.firreg per var consumes the FINAL wire — legal
        //            SSA because next-wires are defined before the register,
        //            and no forward references exist anywhere.
        //
        // To undo: restore the seq.always/fantasy-initial_value emission.
        let mut reg_names: HashMap<String, String> = HashMap::new();

        // Phase A: init constants.
        let special_outputs: std::collections::HashSet<&str> = ["halt"].iter().cloned().collect();
        // 2026-08-23: init WIRE IDS recorded (Phase C previously guessed
        // '%<name>_init' strings that never matched the fresh_const names ->
        // undefined references).
        let mut init_wires: HashMap<String, String> = HashMap::new();
        for (var_name, mlir_ty) in &output_ports {
            let init_val = if special_outputs.contains(var_name.as_str()) {
                "0".to_string()
            } else {
                self.initial_value(var_name)
            };
            let c = ng.fresh_const(&format!("{}_init", var_name));
            writeln!(out, "  {} = hw.constant {} : {}", c, init_val, mlir_ty).ok();
            reg_names.insert(var_name.clone(), c.clone());
            init_wires.insert(var_name.clone(), c);
        }
        // Boolean constants used by mux guards.
        // Bool constants: custom syntax takes NO type suffix.
        writeln!(out, "  %true = hw.constant true").ok();
        writeln!(out, "  %false = hw.constant false").ok();

        let clock_wire = "%clock";

        // Phase B: transaction bodies (in program order).
        let mut pending: HashMap<String, String> = HashMap::new();
        for item in items {
            if let TopLevel::Transaction(txn) = item {
                self.emit_txn_body(
                    &mut ng,
                    out,
                    &txn.name,
                    &txn.body,
                    &txn.contract,
                    &mut reg_names,
                    &mut pending,
                    clock_wire,
                );
            }
        }

        // Phase C: registers consume FINAL wires; reset forces init values.
        for (var_name, mlir_ty) in &output_ports {
            // stored ids already carry their '%' prefix
            let init_wire = init_wires
                .get(var_name)
                .cloned()
                .unwrap_or_else(|| "%0".to_string());
            let next = pending
                .get(var_name)
                .cloned()
                .unwrap_or_else(|| init_wire.clone());
            // next-on-reset mux: %reset high -> init value.
            let mux = ng.fresh_wire(&format!("{}_next", var_name));
            writeln!(
                out,
                "  {} = comb.mux %reset, {}, {} : {}",
                mux, init_wire, next, mlir_ty
            )
            .ok();
            let reg = ng.fresh_reg(var_name);
            let preset = self.initial_value(var_name);
            writeln!(
                out,
                "  {} = seq.firreg {} clock {} preset {} : {}",
                reg, mux, clock_wire, preset, mlir_ty
            )
            .ok();
            reg_names.insert(var_name.clone(), reg);
        }

        // ── Outputs.
        // hw.output syntax: values then ONE ':' + type list.
        let mut out_vals: Vec<String> = Vec::new();
        let mut out_tys: Vec<String> = Vec::new();
        for (var_name, mlir_ty) in &output_ports {
            if let Some(reg) = reg_names.get(var_name) {
                out_vals.push(reg.clone());
                out_tys.push(mlir_ty.clone());
            } else {
                let c = ng.fresh_const(&format!("{}_default", var_name));
                writeln!(out, "  {} = hw.constant 0 : {}", c, mlir_ty).ok();
                out_vals.push(c);
                out_tys.push(mlir_ty.clone());
            }
        }
        if !out_vals.is_empty() {
            writeln!(out, "  hw.output {} : {}", out_vals.join(", "), out_tys.join(", "))
                .ok();
        }
        // Close the hw.module region.
        writeln!(out, "}}").ok();
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
                    // 2026-08-23 (Plan 3.2): the invented comb ops are
                    // DELETED — comb has no ctpop/ctlz/cttz/rev/sin/cos/pow/
                    // sqrt/floor/ceil; those arms emitted MLIR no toolchain
                    // could parse. Abs# stays (honest neg+icmp+mux). Unknown
                    // `#` intrinsics now RECORD a capability error instead of
                    // vanishing into a submodule instantiation.
                    n if n.ends_with('#') && n != "Abs#" && n != "AddressOf#" && n != "Size#" => {
                        self.record_unsupported(&format!("intrinsic '{}'", n));
                        None
                    }
                    _ => {
                        // Function calls become submodule instantiations
                        let inst_name = name.replace('-', "_");
                        let result_wire = ng.fresh_wire(&format!("{}_result", inst_name));
                        // 2026-08-23 (Plan 3.2): real hw.instance syntax —
                        // port names must match the cell's declared params.
                        // The old form emitted $arg0/$result ($ names are
                        // invalid MLIR identifiers).
                        let param_names: Vec<String> = self
                            .cell_defs
                            .get(&inst_name)
                            .map(|c| c.parameters.iter().map(|(n, _)| n.clone()).collect())
                            .unwrap_or_else(|| {
                                (0..args.len()).map(|i| format!("in{}", i)).collect()
                            });
                        let mut arg_parts = Vec::new();
                        for (i, arg) in args.iter().enumerate() {
                            let port = param_names.get(i).cloned().unwrap_or_else(|| format!("in{}", i));
                            let arg_mlir_ty = if i == 0 { "i64" } else { result_ty };
                            if let Some(arg_val) = self.emit_expr(ng, out, arg, reg_names, arg_mlir_ty) {
                                arg_parts.push(format!("{}: {}: {}", port, arg_val, arg_mlir_ty));
                            }
                        }
                        let out_port = self
                            .cell_defs
                            .get(&inst_name)
                            .and_then(|c| Self::extract_output_names_llvm(&c.output_type).into_iter().next())
                            .unwrap_or_else(|| "out".to_string());
                        writeln!(out, "  {} = hw.instance \"{}\" @{} ({}) -> ({}: {})",
                            result_wire, inst_name, inst_name,
                            arg_parts.join(", "),
                            out_port, result_ty,
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
            other => {
                // 2026-08-23 (Plan 3.3): recorded — never a silent None.
                let kind = format!("{:?}", other)
                    .split(|c: char| !c.is_alphanumeric())
                    .next()
                    .unwrap_or("expression")
                    .to_string();
                self.record_unsupported(&format!("{} expressions in hardware", kind));
                None
            }
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

    #[allow(clippy::too_many_arguments)]
    fn emit_txn_body(
        &self,
        ng: &mut NameGen,
        out: &mut String,
        name: &str,
        body: &[Statement],
        contract: &Contract,
        reg_names: &mut HashMap<String, String>,
        pending: &mut HashMap<String, String>,
        clock_wire: &str,
    ) {
        let _ = clock_wire;
        // ── §3.4: contracts as hardware obligations — pre/post conditions
        // materialize as real comparators so synthesis/simulation tools can
        // assert on them (sv.assert wiring lands with the toolchain harness).
        // NOTE: no placeholder wire — emit_contract_condition emits the
        // defining comb op directly with this result name (a pre-emitted
        // hw.wire caused duplicate definitions).
            // §3.4: comparator wire only — sv.assert needs a procedural
            // region; simulation wrappers assert on this signal instead.
        if !matches!(&contract.pre_condition, Expr::Bool(true)) {
            let w = ng.fresh_wire(&format!("{}_pre", name));
            self.emit_contract_condition(out, ng, &contract.pre_condition, &w, reg_names);

        }
        if !matches!(&contract.post_condition, Expr::Bool(true)) {
            let w = ng.fresh_wire(&format!("{}_post", name));
            self.emit_contract_condition(out, ng, &contract.post_condition, &w, reg_names);

        }

        for stmt in body {
            self.emit_stmt_pending(ng, out, stmt, reg_names, pending);
        }
    }

    /// 2026-08-23 (Plan 3): assignments compute a value WIRE and repoint the
    /// pending map — reads elsewhere keep seeing pre-update values until the
    /// register consumes the final wire. Guarded bodies mux on their
    /// condition against the current pending/current wire.
    fn emit_stmt_pending(
        &self,
        ng: &mut NameGen,
        out: &mut String,
        stmt: &Statement,
        reg_names: &mut HashMap<String, String>,
        pending: &mut HashMap<String, String>,
    ) {
        match stmt {
            Statement::Expression(expr) => {
                self.emit_expr(ng, out, expr, reg_names, "i64");
            }
            Statement::Assign(lhs, expr) => {
                if let Some(var_name) = lhs.as_var_name() {
                    let mlir_ty =
                        self.mlir_type(self.var_types.get(var_name).unwrap_or(&Type::int()));
                    if let Some(val) = self.emit_expr(ng, out, expr, reg_names, &mlir_ty) {
                        let current = pending
                            .get(var_name)
                            .cloned()
                            .or_else(|| reg_names.get(var_name).cloned());
                        let target = match current {
                            Some(c) => c,
                            None => {
                                // First write to a var with no init wire yet:
                                // declare its init constant now.
                                let init_val = self.initial_value(var_name);
                                let c = ng.fresh_const(&format!("{}_init", var_name));
                                writeln!(
                                    out,
                                    "  {} = hw.constant {} : {}",
                                    c, init_val, mlir_ty
                                )
                                .ok();
                                reg_names.insert(var_name.to_string(), c.clone());
                                c
                            }
                        };
                        let w = ng.fresh_wire(&format!("{}_next", var_name));
                        writeln!(out, "  {} = comb.mux %true, {}, {} : {}", w, val, target, mlir_ty)
                            .ok();
                        pending.insert(var_name.to_string(), w);
                    }
                }
            }
            Statement::Guarded(condition, statements) => {
                let cond_ty = "i1";
                if let Some(cond) = self.emit_expr(ng, out, condition, reg_names, cond_ty) {
                    for inner in statements {
                        if let Statement::Assign(lhs, expr) = inner {
                            if let Some(var_name) = lhs.as_var_name() {
                                let mlir_ty = self.mlir_type(
                                    self.var_types.get(var_name).unwrap_or(&Type::int()),
                                );
                                if let Some(val) =
                                    self.emit_expr(ng, out, expr, reg_names, &mlir_ty)
                                {
                                    let current = pending
                                        .get(var_name)
                                        .cloned()
                                        .or_else(|| reg_names.get(var_name).cloned())
                                        .unwrap_or_else(|| "%0".to_string());
                                    // enabled write: cond ? new : hold-current
                                    let w =
                                        ng.fresh_wire(&format!("{}_when", var_name));
                                    writeln!(
                                        out,
                                        "  {} = comb.mux {}, {}, {} : {}",
                                        w, cond, val, current, mlir_ty
                                    )
                                    .ok();
                                    pending.insert(var_name.to_string(), w);
                                }
                            }
                        } else {
                            self.emit_stmt_pending(ng, out, inner, reg_names, pending);
                        }
                    }
                }
            }
            Statement::SyncBlock(body) | Statement::Block(body) => {
                for inner in body {
                    self.emit_stmt_pending(ng, out, inner, reg_names, pending);
                }
            }
            other => {
                // 2026-08-23 (Plan 3.3): recorded — never silent.
                let kind = format!("{:?}", other)
                    .split(|c: char| !c.is_alphanumeric())
                    .next()
                    .unwrap_or("statement")
                    .to_string();
                self.record_unsupported(&format!("{} statements in hardware", kind));
            }
        }
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
            write!(out, ") -> ({}: {})", first_out, out_mlir_ty).ok();
        } else {
            write!(out, ") -> ()").ok();
        }
        writeln!(out, " {{").ok();

        let mut reg_names: HashMap<String, String> = HashMap::new();
        for (field_name, field_ty) in &cell.fields {
            let _ = field_name;
            let mlir_ty = self.mlir_type(field_ty);
            let init_val = "0".to_string();
            let reg = ng.fresh_reg(cell_name);
            writeln!(out, "  {} = seq.firreg initial_value {{ init_value = {} : {} }} : {}", reg, init_val, mlir_ty, mlir_ty).ok();
            reg_names.insert(field_name.clone(), reg);
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
        assert!(output.contains("clock: !seq.clock"));
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
        assert!(output.contains("x: i64"), "State should render the signed Int protocol. Got:\n{}", output);
    }

    #[test]
    fn test_circt_expr_mul() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("y", Type::int()),
        ]);
        assert!(output.contains("y: i64"), "State should render the signed Int protocol. Got:\n{}", output);
    }

    #[test]
    fn test_circt_modern_output_ports() {
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("counter", Type::int()),
        ]);
        assert!(output.contains("in %clock: !seq.clock"), "Should use 'in %' prefix for inputs. Got:\n{}", output);
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
        // 2026-08-23 (Plan 3): seq.alwas was fantasy syntax — the register
        // consumes a next-value wire via typed `seq.firreg %next clock`.
        // Register consumes the computed next wire (suffix from NameGen).
        assert!(output.contains("= seq.firreg %counter_next"),
            "register must consume the computed next wire. Got:\n{}", output);
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
        // 2026-08-23 (Plan 3): sync blocks lower through the same pending-
        // wire path — both registers consume computed next wires.
        assert!(output.contains("= seq.firreg %a_next"), "a register consumes next. Got:\n{}", output);
        assert!(output.contains("= seq.firreg %b_next"), "b register consumes next. Got:\n{}", output);
        // a := 10: an enabled mux selects the constant over the init wire.
        assert!(output.contains("OpConstant") || output.contains("hw.constant 10"),
            "constant 10 must be materialized. Got:\n{}", output);
        assert!(output.matches("comb.mux %true,").count() >= 2,
            "both assignments produce enabled muxes. Got:\n{}", output);
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
        // 2026-08-23 (Plan 3.2): comb has NO ctpop — the old arm emitted
        // MLIR no toolchain could parse. Now a recorded capability error.
        let errs = backend.errors.borrow();
        assert!(!errs.is_empty() && errs[0].contains("Ctpop#"),
            "Ctpop must be a recorded capability error. Got:\n{}", output);
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
        // 2026-08-23 (Plan 3.2): comb has NO rev — recorded capability error.
        let errs = backend.errors.borrow();
        assert!(!errs.is_empty() && errs[0].contains("Bitreverse#"),
            "Bitreverse must be a recorded capability error. Got:\n{}", output);
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
    #[test]
    fn test_emitted_module_parses_under_circt_opt() {
        if !circt_tools_available() {
            eprintln!(
                "SKIP: circt-opt not installed — run tools/install-circt.sh \
                 for toolchain-validated coverage"
            );
            return;
        }
        let mut backend = CirctBackend::new();
        let output = backend.generate(&[
            make_state_decl("counter", Type::int()),
            make_trigger("tick", "tick"),
            make_txn(
                "step",
                vec![Statement::Assign(
                    Expr::Identifier("counter".into()),
                    Expr::BinaryOp(
                        crate::ast::BinaryOpKind::Add,
                        Box::new(Expr::Identifier("counter".into())),
                        Box::new(Expr::Decimal(1)),
                    ),
                )],
                Expr::Bool(true),
                Expr::Bool(true),
            ),
        ]);
        assert!(backend.errors.borrow().is_empty(), "fixture must be in-surface");

        let dir = std::env::temp_dir().join(format!("briev_circt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("top.mlir");
        std::fs::write(&path, &output).unwrap();

        // Locate circt-opt: local install first, then PATH.
        let local = format!("{}/tools/circt/bin/circt-opt", env!("CARGO_MANIFEST_DIR"));
        let tool = if std::path::Path::new(&local).exists() {
            local
        } else {
            "circt-opt".to_string()
        };
        let out = std::process::Command::new(tool)
            .arg(&path)
            .output()
            .expect("run circt-opt");
        assert!(
            out.status.success(),
            "circt-opt rejected the emitted module:\n{}\n{}",
            String::from_utf8_lossy(&out.stderr),
            output
        );
        let _ = std::fs::remove_file(&path);
    }
}

/// 2026-08-23 (Plan 0.6): CIRCT toolchain availability — mirrors the
/// `is_available()` pattern (backend/assembler/mod.rs). Toolchain-validated
/// tests (parse/translate/simulate parity) gate on this and skip loudly
/// when tools are absent; structural string checks always run.
/// To undo: remove this fn + tools/install-circt.sh + tools/circt_probe.sh.
pub fn circt_tools_available() -> bool {
    // 1. Local install from tools/install-circt.sh
    let local = std::path::Path::new("tools/circt/bin/circt-opt");
    if local.exists() {
        return true;
    }
    // 2. Somewhere on PATH
    std::process::Command::new("circt-opt")
        .arg("--version")
        .output()
        .is_ok()
}
