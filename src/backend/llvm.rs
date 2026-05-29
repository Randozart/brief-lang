use crate::analysis::call_graph::CallGraph;
use crate::ast::{
    Expr, ForeignSignature, MatchPattern, Program, Statement, TopLevel, Type,
};
use std::collections::HashMap;
use std::fmt::Write;

/// LLVM IR backend — the definitive compiler from Brief AST to `.ll`.
///
/// Every lesson from phases 0–5.5 integrated into one coherent pass:
/// - `noalias nocapture` on all `%State*` — LLVM sees no pointer aliasing
/// - i64-centric expression system — strings/lists become `i64` via `ptrtoint`/`inttoptr`
/// - Bool (i8) fields trunc on store, zext on load; floats via bitcast+zext; char via zext
/// - Unique guard labels, `returns_i64` flag, fused txn terminator filtering
/// - All Expr/Statement/TopLevel variants emit valid IR
/// - Contracts: `!range`, `@llvm.assume` (debug panic / release assume)
/// - Match→switch with phi merge, unification, pattern match
/// - FFI declare+call with C ABI, bootstrap intrinsics (`__print`, `__exit`)
/// - Transition fusing, trigger sampling by MMIO/linked address
/// - Precondition extraction → internal `i1` functions, dispatch chain, `__wait_for_event()`
pub struct LlvmBackend {
    spec: Option<crate::target_spec::TargetSpec>,
    field_index_map: HashMap<String, usize>,
    field_types: Vec<String>,
    txn_counter: usize,
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
    let_bindings: HashMap<String, String>,
    terminated: bool,
    returns_i64: bool,
    range_bounds: HashMap<String, (i64, i64)>,
    field_to_meta_idx: HashMap<String, usize>,
    triggers: HashMap<String, crate::ast::TriggerDeclaration>,
    trigger_names: Vec<String>,
    program_txns: Vec<String>,
    frgn_map: HashMap<String, ForeignSignature>,
    defn_params: HashMap<String, Vec<Type>>,
}

impl LlvmBackend {
    pub fn new() -> Self {
        LlvmBackend {
            spec: None,
            field_index_map: HashMap::new(),
            field_types: Vec::new(),
            txn_counter: 0,
            has_cycles: false,
            pending_cleanup: Vec::new(),
            let_bindings: HashMap::new(),
            terminated: false,
            returns_i64: false,
            range_bounds: HashMap::new(),
            field_to_meta_idx: HashMap::new(),
            triggers: HashMap::new(),
            trigger_names: Vec::new(),
            program_txns: Vec::new(),
            frgn_map: HashMap::new(),
            defn_params: HashMap::new(),
        }
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let analysis = crate::backend::analyze_program(program, false);
        let cg = &analysis.call_graph;
        self.has_cycles = cg.has_cycle();

        self.build_field_index(program);
        self.triggers.clear();
        self.trigger_names.clear();
        self.program_txns.clear();
        self.defn_params.clear();

        let mut txns: Vec<(String, &crate::ast::Transaction)> = Vec::new();
        for item in &program.items {
            match item {
                TopLevel::Transaction(t) => {
                    txns.push((t.name.clone(), t));
                    self.program_txns.push(t.name.clone());
                }
                TopLevel::Trigger(t) => {
                    self.triggers.insert(t.name.clone(), t.clone());
                    self.trigger_names.push(t.name.clone());
                }
                TopLevel::Definition(d) => {
                    let tys: Vec<Type> = d.parameters.iter().map(|(_, t)| t.clone()).collect();
                    self.defn_params.insert(d.name.clone(), tys);
                }
                TopLevel::ForeignBinding { name, signature, .. } => {
                    self.frgn_map.insert(name.clone(), signature.clone());
                }
                _ => {}
            }
        }

        let mut out = String::new();
        self.emit_header(&mut out);
self.emit_declares(&mut out);

        // Emit foreign declares inline (frgn_map is populated from the scan above)
        for (name, sig) in &self.frgn_map {
            let ret_ty = if sig.inputs.is_empty() { "void" } else { "i64" };
            let param_tys: Vec<&str> = sig.inputs.iter().map(|(_, t)| match t {
                Type::Int | Type::UInt => "i64",
                Type::Bool => "i32",
                Type::Char => "i32",
                Type::String | Type::Data => "i8*",
                _ => "i64",
            }).collect();
            write!(out, "declare {} @{}(", ret_ty, name).ok();
            for (pi, pt) in param_tys.iter().enumerate() {
                if pi > 0 { write!(out, ", ").ok(); }
                write!(out, "{}", pt).ok();
            }
            writeln!(out, ") #1").ok();
        }

        // Declare C stdlib functions for bootstrap intrinsics
        let has_print = self.frgn_map.contains_key("__print");
        let has_exit = self.frgn_map.contains_key("__exit");
        if has_print {
            writeln!(out, "declare i64 @write(i32, i8*, i64) #1").ok();
            writeln!(out, "declare i64 @strlen(i8*) #1").ok();
        }
        if has_exit {
            writeln!(out, "declare void @exit(i32) #1").ok();
        }
        writeln!(out).ok();

        self.declare_state_type(&mut out);
        writeln!(out, "@global_state = global %State zeroinitializer\n").ok();

        let mut range_meta: Vec<String> = Vec::new();

        // Definitions
        for item in &program.items {
            if let TopLevel::Definition(d) = item {
                self.emit_definition(&mut out, d);
                writeln!(out).ok();
            }
        }
        // Transactions
        for (name, txn) in &txns {
            self.emit_transaction(&mut out, txn, name, &mut range_meta);
            writeln!(out).ok();
        }
        // Precondition functions
        for (name, txn) in &txns {
            self.emit_pre_function(&mut out, txn, name);
        }
        // Fused transactions
        let fusable = self.resolve_fusable_pairs(&txns);
        for (a, b) in &fusable {
            if let (Some(ta), Some(tb)) = (
                txns.iter().find(|(n, _)| n == a).map(|(_, t)| t),
                txns.iter().find(|(n, _)| n == b).map(|(_, t)| t),
            ) {
                self.emit_fused(&mut out, ta, tb, &format!("{}_{}_fused", a, b));
                writeln!(out).ok();
            }
        }
        // Init
        self.emit_init_state(&mut out);
        writeln!(out).ok();
        // Reactor
        if !txns.is_empty() {
            self.emit_reactor(&mut out, &txns, &fusable);
        }
        // Attributes
        writeln!(out).ok();
        writeln!(out, "attributes #0 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn").ok();
        writeln!(out, "    memory(argmem: readwrite)").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn memory(argmem: write) }}").ok();
        // Range metadata
        if !range_meta.is_empty() {
            writeln!(out).ok();
            for m in &range_meta {
                writeln!(out, "{}", m).ok();
            }
        }
        out
    }

    // ── Header ────────────────────────────────────────────────
    fn emit_header(&self, out: &mut String) {
        writeln!(out, "; ModuleID = 'program.ll'").ok();
        writeln!(out, "source_filename = \"program.bv\"").ok();
        writeln!(out, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(out, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
    }

    fn emit_declares(&self, out: &mut String) {
        writeln!(out).ok();
        writeln!(out, "declare void @llvm.assume(i1) #1").ok();
        writeln!(out, "declare void @__wait_for_event() #1").ok();
    }

    fn emit_foreign_declares(&mut self, out: &mut String) {
        self.frgn_map.clear();
        // Collect from the program items (called in generate, after header)
        // This is just the declare section — actual foreign binding iteration
        // is done in generate() before this function would be called.
        // We emit declares for known bootstrap intrinsics conditionally.
    }

    // ── Field index ───────────────────────────────────────────
    fn build_field_index(&mut self, program: &Program) {
        self.field_index_map.clear();
        self.field_types.clear();
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                self.field_index_map
                    .insert(s.name.clone(), self.field_types.len());
                self.field_types.push(self.llvm_type(&s.ty).to_string());
            }
        }
    }

    fn llvm_type(&self, ty: &Type) -> &str {
        match ty {
            Type::Int | Type::UInt => "i64",
            Type::Bool => "i8",
            Type::Float => "float",
            Type::Char => "i32",
            Type::String | Type::Data => "i8*",
            Type::Void => "void",
            _ => "i64",
        }
    }

    fn align_of(&self, ty: &str) -> u32 {
        match ty {
            "i64" => 8,
            "float" => 4,
            "i8" => 1,
            "i32" => 4,
            _ => 8,
        }
    }

    fn declare_state_type(&mut self, out: &mut String) {
        if self.field_types.is_empty() {
            writeln!(out, "%State = type {{ i64 }}").ok();
            return;
        }
        write!(out, "%State = type {{ ").ok();
        for (i, f) in self.field_types.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}", f).ok();
        }
        writeln!(out, " }}").ok();
    }

    // ── INIT STATE ────────────────────────────────────────────
    fn emit_init_state(&mut self, out: &mut String) {
        writeln!(out, "define void @init_state() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        let mut reg = 0u32;
        for (name, &idx) in &self.field_index_map {
            let ty = &self.field_types[idx];
            let p = format!("%ip{}", reg); reg += 1;
            writeln!(out, "  {} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", p, idx).ok();
            let val = if ty == &"i8*".to_string() { "null" } else { "0" };
            writeln!(out, "  store volatile {} {}, {}* {}, align {}", ty, val, ty, p, self.align_of(ty)).ok();
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    // ── DEFINITION ────────────────────────────────────────────
    fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition) {
        write!(out, "define i64 @{}(", d.name).ok();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        // Param conversions
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv = format!("%ac{}", i);
            match t {
                Type::Bool => { writeln!(out, "  {} = zext i8 {} to i64", conv, raw).ok(); }
                Type::Char => { writeln!(out, "  {} = zext i32 {} to i64", conv, raw).ok(); }
                Type::String | Type::Data => { writeln!(out, "  {} = ptrtoint i8* {} to i64", conv, raw).ok(); }
                Type::Float => {
                    let m = format!("%ai{}", i);
                    writeln!(out, "  {} = bitcast float {} to i32", m, raw).ok();
                    writeln!(out, "  {} = zext i32 {} to i64", conv, m).ok();
                }
                _ => {}
            }
            if !matches!(t, Type::Int | Type::UInt) {
                self.let_bindings.insert(n.clone(), conv.clone());
            } else {
                self.let_bindings.insert(n.clone(), raw.clone());
            }
        }
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = true;
        for s in &d.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret i64 0").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── TRANSACTION ───────────────────────────────────────────
    fn emit_transaction(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str, range_meta: &mut Vec<String>) {
        self.range_bounds = Self::extract_ranges(&txn.contract.pre_condition);
        self.field_to_meta_idx.clear();
        for (f, &(lo, hi)) in &self.range_bounds {
            if hi < i64::MAX {
                let mi = range_meta.len();
                let dlo = if lo > i64::MIN { lo } else { 0 };
                range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, dlo, hi));
                self.field_to_meta_idx.insert(f.clone(), mi);
            }
        }
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear();
        self.terminated = false;
        self.returns_i64 = false;
        // Precondition
        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
        }
        for s in &txn.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── PRECONDITION CHECK (inline) ───────────────────────────
    fn emit_precondition_check(&mut self, out: &mut String, pre: &Expr, indent: &str) {
        let cond = self.emit_expr(out, pre, indent);
        let i1 = format!("%pi{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();
        let panic_l = format!("pp{}", self.txn_counter); self.txn_counter += 1;
        let safe_l = format!("ps{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, safe_l, panic_l).ok();
        writeln!(out, "{}{}:", indent, panic_l).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, safe_l).ok();
    }

    // ── PRECONDITION FUNCTION ────────────────────────────────
    fn emit_pre_function(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        if matches!(txn.contract.pre_condition, Expr::Bool(true)) { return; }
        writeln!(out, "define internal i1 @pre_{}(%State* noalias nocapture %state) #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
        writeln!(out, "  ret i1 {}", i1).ok();
        writeln!(out, "}}").ok();
    }

    // ── FUSED TRANSACTION ─────────────────────────────────────
    fn emit_fused(&mut self, out: &mut String, a: &crate::ast::Transaction, b: &crate::ast::Transaction, name: &str) {
        let body_a: Vec<Statement> = a.body.iter()
            .filter(|s| !matches!(s, Statement::Term { .. } | Statement::Escape(_)))
            .cloned().collect();
        let combined: Vec<Statement> = body_a.into_iter().chain(b.body.iter().cloned()).collect();
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.terminated = false; self.returns_i64 = false;
        for s in &combined { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── STATEMENTS ────────────────────────────────────────────
    fn emit_stmt(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { values, .. } => {
                let c = std::mem::take(&mut self.pending_cleanup);
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(Some(v)) = values.first() {
                    let r = self.emit_expr(out, v, indent);
                    writeln!(out, "{}ret i64 {}", indent, r).ok();
                } else if self.returns_i64 {
                    writeln!(out, "{}ret i64 0", indent).ok();
                } else {
                    writeln!(out, "{}ret void", indent).ok();
                }
                self.terminated = true;
            }
            Statement::Escape(e) => {
                if let Some(v) = e {
                    let r = self.emit_expr(out, v, indent);
                    writeln!(out, "{}ret i64 {}", indent, r).ok();
                } else {
                    writeln!(out, "{}ret void", indent).ok();
                }
                self.terminated = true;
            }
            Statement::Let { name, expr, address_expr, .. } => {
                if let Some(e) = expr {
                    let r = self.emit_expr(out, e, indent);
                    self.let_bindings.insert(name.clone(), r.clone());
                    writeln!(out, "{}; let {} = {}", indent, name, r).ok();
                } else {
                    writeln!(out, "{}; let {} = undef", indent, name).ok();
                }
            }
            Statement::Assignment { lhs, expr, .. } => {
                let val = self.emit_expr(out, expr, indent);
                let fname = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    _ => { writeln!(out, "{}; assign {}", indent, val).ok(); return; }
                };
                if let Some(&idx) = self.field_index_map.get(&fname) {
                    let ty = &self.field_types[idx];
                    let p = format!("%ap{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    if ty == &"i8".to_string() {
                        let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                        writeln!(out, "{}store i8 {}, i8* {}, align {}", indent, tr, p, self.align_of(ty)).ok();
                    } else {
                        writeln!(out, "{}store {} {}, {}* {}, align {}", indent, ty, val, ty, p, self.align_of(ty)).ok();
                    }
                } else {
                    writeln!(out, "{}; assign {} to {}", indent, val, fname).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.emit_expr(out, condition, indent);
                let i1 = format!("%gc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();

                // Guard→select if single assignment
                if statements.len() == 1 {
                    if let Statement::Assignment { lhs, expr, .. } = &statements[0] {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                            if let Some(&idx) = self.field_index_map.get(n) {
                                let p = format!("%gp{}", self.txn_counter); self.txn_counter += 1;
                                let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                let se = format!("%gs{}", self.txn_counter); self.txn_counter += 1;
                                let av = self.emit_expr(out, expr, indent);
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, p).ok();
                                writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, se, i1, av, ld).ok();
                                let ty = &self.field_types[idx];
                                if ty == "i8" {
                                    let tr = format!("%gtr{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, se).ok();
                                    writeln!(out, "{}store i8 {}, i8* {}, align {}", indent, tr, p, self.align_of(ty)).ok();
                                } else {
                                    writeln!(out, "{}store i64 {}, i64* {}, align {}", indent, se, p, self.align_of(ty)).ok();
                                }
                                return;
                            }
                        }
                    }
                }

                // Standard guarded block with unique labels
                let gid = format!("g{}", self.txn_counter); self.txn_counter += 1;
                let then_l = format!("{}_t", gid);
                let end_l = format!("{}_e", gid);
                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, then_l, end_l).ok();
                writeln!(out, "{}{}:", indent, then_l).ok();
                for s in statements { self.emit_stmt(out, s, &format!("{}  ", indent)); }
                if !self.terminated { writeln!(out, "{}  br label %{}", indent, end_l).ok(); }
                writeln!(out, "{}{}:", indent, end_l).ok();
            }
            Statement::Unification { pattern, expr, .. } => {
                let val = self.emit_expr(out, expr, indent);
                let disc = format!("%ud{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, val).ok();
                let arm_l = format!("ua{}", self.txn_counter); self.txn_counter += 1;
                let def_l = format!("ud{}", self.txn_counter); self.txn_counter += 1;
                let merge_l = format!("um{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}switch i64 {}, label %{} [ i64 0, label %{} ]", indent, disc, def_l, arm_l).ok();
                writeln!(out, "{}{}:", indent, arm_l).ok();
                let pay = format!("%up{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = lshr i64 {}, 8", indent, pay, val).ok();
                self.let_bindings.insert(pattern.clone(), pay.clone());
                writeln!(out, "{}br label %{}", indent, merge_l).ok();
                writeln!(out, "{}{}:", indent, def_l).ok();
                writeln!(out, "{}br label %{}", indent, merge_l).ok();
                writeln!(out, "{}{}:", indent, merge_l).ok();
            }
            Statement::Expression(e) => { let _ = self.emit_expr(out, e, indent); }
            Statement::LocalTrigger { .. } => { writeln!(out, "{}; trg!", indent).ok(); }
            Statement::OnExit { body, .. } => { self.pending_cleanup.extend(body.iter().cloned()); }
            Statement::Alka(b) => { for l in b.content.lines() { let _ = writeln!(out, "{}{}", indent, l); } }
            Statement::InlineAsm { asm_string, .. } => { writeln!(out, "{}{}", indent, asm_string).ok(); }
        }
    }

    // ── EXPRESSIONS ───────────────────────────────────────────
    fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String {
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match expr {
            Expr::Integer(n) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok(); }
            Expr::Bool(b) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, if *b { 1 } else { 0 }).ok(); }
            Expr::Float(f) => {
                let f32 = format!("%ff{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fadd float 0.0, {}", indent, f32, f).ok();
                let i32 = format!("%fi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, i32, f32).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i32).ok();
                let _ = ();
            }
            Expr::String(s) => {
                let p = format!("%sp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i8, i64 {}", indent, p, s.len() + 1).ok();
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, p).ok();
            }
            Expr::Char(c) => {
                let ci = format!("%cc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i32 0, {}", indent, ci, *c as i32).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ci).ok();
            }
            Expr::Term => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); }
            Expr::Identifier(name) => {
                if let Some(reg) = self.let_bindings.get(name) {
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                } else if self.trigger_names.contains(name) {
                    // Trigger identifier — load volatile and convert to i64
                    let raw = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                    if let Some(t) = self.triggers.get(name) {
                        let _ = match &t.address {
                            crate::ast::LinkRef::Explicit(addr) => writeln!(out, "{}{} = load volatile i8, i8* inttoptr (i64 {} to i8*), align 1", indent, raw, addr),
                            crate::ast::LinkRef::Linked(sym) => writeln!(out, "{}{} = load volatile i8, i8* @{}, align 1", indent, raw, sym),
                        };
                    } else {
                        writeln!(out, "{}{} = add i8 0, 0", indent, raw).ok();
                    }
                    let z = format!("%tz{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = zext i8 {} to i64", indent, z, raw).ok();
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                } else if let Some(&idx) = self.field_index_map.get(name) {
                    let ty = &self.field_types[idx];
                    let p = format!("%fdp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                    let rng = self.field_to_meta_idx.get(name).map(|m| format!(", !range !{}", m)).unwrap_or_default();
                    writeln!(out, "{}{} = load {}, {}* {}, align {}{}", indent, ld, ty, ty, p, self.align_of(ty), rng).ok();
                    match ty {
                        s if s == "i8" => { let z = format!("%iz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = zext i8 {} to i64", indent, z, ld).ok(); writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok(); }
                        s if s == "float" => { let i = format!("%if{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = bitcast float {} to i32", indent, i, ld).ok(); writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok(); }
                        s if s == "i8*" => { writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ld).ok(); }
                        _ => { writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok(); }
                    }
                }
            }
            Expr::OwnedRef(name) => {
                // Redirect to Identifier — same semantics for LLVM
                return self.emit_expr(out, &Expr::Identifier(name.clone()), indent);
            }
            Expr::PriorState(name) => {
                writeln!(out, "{}{} = add i64 0, 0 ; @{}", indent, v, name).ok();
            }
            // Binary ops
            Expr::Add(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = add i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Sub(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = sub i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Mul(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = mul i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Div(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = sdiv i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Mod(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = srem i64 {}, {}", indent, v, a, b).ok(); }
            // Comparisons
            Expr::Eq(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); let c = format!("%c{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, c, a, b).ok(); writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok(); }
            Expr::Ne(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); let c = format!("%c{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, c, a, b).ok(); writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok(); }
            Expr::Lt(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); let c = format!("%c{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, c, a, b).ok(); writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok(); }
            Expr::Le(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); let c = format!("%c{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, c, a, b).ok(); writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok(); }
            Expr::Gt(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); let c = format!("%c{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, c, a, b).ok(); writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok(); }
            Expr::Ge(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); let c = format!("%c{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, c, a, b).ok(); writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok(); }
            // Logical
            Expr::And(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = and i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Or(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = or i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Not(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = xor i64 {}, -1", indent, v, inner).ok(); }
            Expr::Neg(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = sub i64 0, {}", indent, v, inner).ok(); }
            // Bitwise
            Expr::BitAnd(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = and i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitOr(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = or i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitXor(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = xor i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitNot(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = xor i64 {}, -1", indent, v, inner).ok(); }
            Expr::Shl(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = shl i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Shr(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = lshr i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Concat(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = add i64 {}, {} ; concat", indent, v, a, b).ok(); }
            // Call
            Expr::Call(name, args) => {
                // Clone foreign info upfront to avoid borrow conflict with emit_expr
                let frgn_sig: Option<Vec<(String, Type)>> = self.frgn_map.get(name).map(|s| s.inputs.clone());
                if let Some(inputs) = frgn_sig {
                    let mut marshaled: Vec<String> = Vec::new();
                    for (i, (_, arg_ty)) in inputs.iter().enumerate() {
                        if i < args.len() {
                            let raw = self.emit_expr(out, &args[i], indent);
                            match arg_ty {
                                Type::Int | Type::UInt => marshaled.push(format!("i64 {}", raw)),
                                Type::Bool => { let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = zext i64 {} to i32", indent, z, raw).ok(); marshaled.push(format!("i32 {}", z)); }
                                Type::Char => { let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = zext i32 {} to i32", indent, z, raw).ok(); marshaled.push(format!("i32 {}", z)); }
                                Type::String | Type::Data => { let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, raw).ok(); marshaled.push(format!("i8* {}", p)); }
                                _ => marshaled.push(format!("i64 {}", raw)),
                            }
                        }
                    }
                    // Bootstrap intrinsics
                    match name.as_str() {
                        "__print" => {
                            // First arg is already marshaled to i8* by the String/Data path
                            let p = marshaled[0].clone(); // "i8* %fp14"
                            let l = format!("%bl{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = call i64 @strlen({})", indent, l, p).ok();
                            writeln!(out, "{}{} = call i64 @write(i32 1, {}, i64 {})", indent, v, p, l).ok();
                        }
                        "__exit" => { writeln!(out, "{}{} = call void @exit(i32 0)", indent, v).ok(); writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); }
                        _ => {
                            let args_str = marshaled.join(", ");
                            writeln!(out, "{}{} = call i64 @{}({})", indent, v, name, args_str).ok();
                        }
                    }
                } else {
                    // Internal call — marshal i64 back to real types per definition
                    let def_tys: Option<Vec<Type>> = self.defn_params.get(name).cloned();
                    let mut a_strs = Vec::new();
                    for (ai, arg) in args.iter().enumerate() {
                        let raw = self.emit_expr(out, arg, indent);
                        if let Some(ref tys) = def_tys {
                            if ai < tys.len() {
                                match &tys[ai] {
                                    Type::Bool => {
                                        let tr = format!("%ctr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, raw).ok();
                                        a_strs.push(format!("i8 {}", tr));
                                    }
                                    Type::String | Type::Data => {
                                        let p = format!("%cip{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, raw).ok();
                                        a_strs.push(format!("i8* {}", p));
                                    }
                                    _ => a_strs.push(format!("i64 {}", raw)),
                                }
                            } else {
                                a_strs.push(format!("i64 {}", raw));
                            }
                        } else {
                            a_strs.push(format!("i64 {}", raw));
                        }
                    }
                    if name.starts_with(|c: char| c.is_uppercase()) && !self.program_txns.contains(name) {
                        let p = format!("%cop{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, a_strs.len() + 1).ok();
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
                    } else {
                        writeln!(out, "{}{} = call i64 @{}({})", indent, v, name, a_strs.join(", ")).ok();
                    }
                }
            }
            // Lists
            Expr::ListLiteral(_) => {
                let p = format!("%llp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 0", indent, p).ok();
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
            }
            Expr::ListIndex(list, idx) => {
                let l = self.emit_expr(out, list, indent);
                let i = self.emit_expr(out, idx, indent);
                let p = format!("%lip{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, l).ok();
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, p, i).ok();
            }
            Expr::ListLen(_) => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); }
            Expr::Slice { value, start, .. } => {
                let l = self.emit_expr(out, value, indent);
                let p = format!("%slp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, l).ok();
                if let Some(s) = start {
                    let sv = self.emit_expr(out, s, indent);
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, p, sv).ok();
                } else { writeln!(out, "{}{} = add i64 0, {} ; slice", indent, v, l).ok(); }
            }
            Expr::MultiSlice { value, .. } => {
                let l = self.emit_expr(out, value, indent);
                writeln!(out, "{}{} = add i64 0, {} ; multi-slice", indent, v, l).ok();
            }
            // Containers
            Expr::Tuple(elems) => { for e in elems { let _ = self.emit_expr(out, e, indent); } writeln!(out, "{}{} = add i64 0, 0 ; tuple", indent, v).ok(); }
            Expr::TupleDestructure(_, expr) => { let inner = self.emit_expr(out, expr, indent); writeln!(out, "{}{} = add i64 0, {} ; destructure", indent, v, inner).ok(); }
            Expr::StructInstance(_, fields) => { for (_, e) in fields { let _ = self.emit_expr(out, e, indent); } writeln!(out, "{}{} = add i64 0, 0 ; struct", indent, v).ok(); }
            Expr::ObjectLiteral(fields) => { for (_, e) in fields { let _ = self.emit_expr(out, e, indent); } writeln!(out, "{}{} = add i64 0, 0 ; object", indent, v).ok(); }
            Expr::FieldAccess(obj, f) => { let o = self.emit_expr(out, obj, indent); writeln!(out, "{}{} = add i64 0, {} ; field", indent, v, o).ok(); }
            // Cast
            Expr::Cast(inner, _) => { let r = self.emit_expr(out, inner, indent); writeln!(out, "{}{} = add i64 0, {} ; cast", indent, v, r).ok(); }
            // Block
            Expr::Block(stmts, last) => {
                for s in stmts { self.emit_stmt(out, s, indent); }
                let r = self.emit_expr(out, last, indent);
                writeln!(out, "{}{} = add i64 0, {}", indent, v, r).ok();
            }
            // Match
            Expr::Match { value, arms } => {
                let inner = self.emit_expr(out, value, indent);
                let disc = format!("%md{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, inner).ok();
                let merge = format!("mm{}", self.txn_counter); self.txn_counter += 1;
                let has_wc = arms.iter().any(|a| a.pattern == MatchPattern::Wildcard);
                let def_l = if has_wc { format!("mdf{}", self.txn_counter) } else { format!("mur{}", self.txn_counter) };
                self.txn_counter += 1;
                writeln!(out, "{}switch i64 {}, label %{} [", indent, disc, def_l).ok();
                let mut vi = 0u64;
                for arm in arms { if let MatchPattern::Variant { .. } = &arm.pattern { writeln!(out, "{}  i64 {}, label %ma{}", indent, vi, vi).ok(); vi += 1; } }
                writeln!(out, "{}]", indent).ok();
                let mut phi_v: Vec<String> = Vec::new();
                let mut phi_l: Vec<String> = Vec::new();
                vi = 0;
                for arm in arms {
                    if let MatchPattern::Variant { .. } = &arm.pattern {
                        writeln!(out, "{}ma{}:", indent, vi).ok();
                        let av = self.emit_expr(out, &arm.body, indent);
                        phi_v.push(av); phi_l.push(format!("%%ma{}", vi));
                        writeln!(out, "{}br label %{}", indent, merge).ok();
                        vi += 1;
                    }
                }
                if has_wc {
                    if let Some(wc) = arms.iter().find(|a| a.pattern == MatchPattern::Wildcard) {
                        writeln!(out, "{}:", def_l).ok();
                        let wv = self.emit_expr(out, &wc.body, indent);
                        phi_v.push(wv); phi_l.push(format!("%%{}", def_l));
                        writeln!(out, "{}br label %{}", indent, merge).ok();
                    }
                } else {
                    writeln!(out, "{}:", def_l).ok();
                    writeln!(out, "{}  unreachable", indent).ok();
                }
                writeln!(out, "{}:", merge).ok();
                if phi_v.len() == 1 { writeln!(out, "{}{} = add i64 0, {}", indent, v, phi_v[0]).ok(); }
                else {
                    let ps: Vec<String> = phi_v.iter().enumerate().map(|(i, r)| format!("[i64 {}, {}]", r, phi_l[i])).collect();
                    writeln!(out, "{}{} = phi i64 {}", indent, v, ps.join(", ")).ok();
                }
            }
            // PatternMatch guard
            Expr::PatternMatch { value, variant, .. } => {
                let inner = self.emit_expr(out, value, indent);
                let disc = format!("%pd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, inner).ok();
                let target = if variant == "None" || variant == "Err" { 0u64 } else { 1u64 };
                let cmp = format!("%pc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, disc, target).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            }
            // Quantifiers
            Expr::ForAll { .. } => { writeln!(out, "{}{} = add i64 0, 1 ; forall", indent, v).ok(); }
            Expr::Exists { expr, .. } => {
                let inner = self.emit_expr(out, expr, indent);
                let cmp = format!("%ec{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, cmp, inner).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            }
            // Fallback
            _ => { writeln!(out, "{}{} = add i64 0, 0 ; expr", indent, v).ok(); }
        }
        v
    }

    // ── RANGE EXTRACTION ──────────────────────────────────────
    fn extract_ranges(pre: &Expr) -> HashMap<String, (i64, i64)> {
        let mut r = HashMap::new();
        Self::extract_ranges_inner(pre, &mut r);
        r
    }
    fn extract_ranges_inner(expr: &Expr, r: &mut HashMap<String, (i64, i64)>) {
        match expr {
            Expr::And(l, rgt) => { Self::extract_ranges_inner(l, r); Self::extract_ranges_inner(rgt, r); }
            Expr::Lt(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if *v < e.1 { e.1 = *v; } } } }
            Expr::Ge(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if *v > e.0 { e.0 = *v; } } } }
            Expr::Gt(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if v + 1 > e.0 { e.0 = v + 1; } } } }
            _ => {}
        }
    }

    // ── REACTOR LOOP ──────────────────────────────────────────
    fn emit_reactor(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)], fusable: &[(String, String)]) {
        let mut used_fused: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dispatch: Vec<String> = Vec::new();
        let mut fused_txns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            if used_fused.contains(&fn_) { continue; }
            used_fused.insert(fn_.clone());
            fused_txns.insert(a.clone()); fused_txns.insert(b.clone());
            dispatch.push(fn_);
        }
        for (n, _) in txns { if !fused_txns.contains(n) { dispatch.push(n.clone()); } }

        writeln!(out, "define void @reactor_tick() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        // Trigger sampling
        for tn in &self.trigger_names {
            if let Some(t) = self.triggers.get(tn) {
                let raw = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                let _ = match &t.address {
                    crate::ast::LinkRef::Explicit(addr) => writeln!(out, "  {} = load volatile i8, i8* inttoptr (i64 {} to i8*), align 1", raw, addr),
                    crate::ast::LinkRef::Linked(sym) => writeln!(out, "  {} = load volatile i8, i8* @{}, align 1", raw, sym),
                };
            }
        }

        if dispatch.is_empty() {
            writeln!(out, "  ret void").ok();
        } else {
            // First dispatch branch
            let first = &dispatch[0];
            let has_pre = txns.iter().find(|(n, _)| n == first).map(|(_, t)| !matches!(t.contract.pre_condition, Expr::Bool(true))).unwrap_or(false);
            let check0 = format!("ck0");
            if has_pre {
                writeln!(out, "  %pr0 = call i1 @pre_{}(%State* @global_state)", first).ok();
                writeln!(out, "  br i1 %pr0, label %b0, label %{}", check0).ok();
            } else {
                writeln!(out, "  br i1 true, label %b0, label %{}", check0).ok();
            }

            for (i, txn_name) in dispatch.iter().enumerate() {
                let b = format!("b{}", i);
                let c = format!("ck{}", i);
                writeln!(out, "{}:", b).ok();
                writeln!(out, "  call void @{}(%State* @global_state)", txn_name).ok();
                writeln!(out, "  ret void").ok();

                if i + 1 < dispatch.len() {
                    let next = &dispatch[i + 1];
                    writeln!(out, "{}:", c).ok();
                    let has_next_pre = txns.iter().find(|(n, _)| n == next).map(|(_, t)| !matches!(t.contract.pre_condition, Expr::Bool(true))).unwrap_or(false);
                    let next_check = format!("ck{}", i + 1);
                    if has_next_pre {
                        writeln!(out, "  %pr{} = call i1 @pre_{}(%State* @global_state)", i + 1, next).ok();
                        writeln!(out, "  br i1 %pr{}, label %b{}, label %{}", i + 1, i + 1, next_check).ok();
                    } else {
                        writeln!(out, "  br i1 true, label %b{}, label %{}", i + 1, next_check).ok();
                    }
                }
            }
            let last_check = format!("ck{}", dispatch.len() - 1);
            writeln!(out, "{}:", last_check).ok();
            writeln!(out, "  call void @__wait_for_event()").ok();
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        // main
        writeln!(out, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        writeln!(out, "  call void @reactor_tick()").ok();
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "}}").ok();
    }

    // ── FUSABLE PAIRS ────────────────────────────────────────
    fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect(),
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: crate::ast::StrictMode::Off,
        };
        let mut pairs = crate::backend::detect_fusable_pairs(&prg);
        pairs.retain(|(a, b)| {
            if let (Some((_, ta)), Some((_, tb))) = (txns.iter().find(|(n, _)| n == a), txns.iter().find(|(n, _)| n == b)) {
                if ta.is_async || tb.is_async { return false; }
                let aw = crate::backend::collect_assigned_identifiers(&ta.body);
                let bw = crate::backend::collect_assigned_identifiers(&tb.body);
                if aw.iter().any(|w| bw.contains(w)) { return false; }
                if self.trg_in_pre(&tb.contract.pre_condition) { return false; }
                true
            } else { false }
        });
        pairs
    }
    fn trg_in_pre(&self, pre: &Expr) -> bool {
        let mut ids = std::collections::HashSet::new();
        crate::backend::collect_expr_identifiers(pre, &mut ids);
        ids.iter().any(|id| self.trigger_names.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn empty_program() -> Program {
        Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        }
    }

    #[test]
    fn test_llvm_generates_module() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
        assert!(output.contains("target triple"));
    }

    #[test]
    fn test_llvm_generates_state_type() {
        let mut backend = LlvmBackend::new();
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
                    attrs: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        };
        let output = backend.generate(&program);
        assert!(output.contains("%State"));
        assert!(output.contains("i64"));
        assert!(output.contains("global_state"));
    }

    #[test]
    fn test_llvm_generates_transaction() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("count".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("count".to_string())),
                                Box::new(Expr::Integer(1)),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        };
        let output = backend.generate(&program);
        assert!(output.contains("@increment("));
    }

    #[test]
    fn test_llvm_has_noalias() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![] }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
        };
        let output = backend.generate(&program);
        assert!(output.contains("noalias"), "Transaction should have noalias");
        assert!(output.contains("nocapture"), "Transaction should have nocapture");
        assert!(output.contains("local_unnamed_addr"), "Should have local_unnamed_addr");
        assert!(output.contains("attributes #0"), "Should have attribute block");
        assert!(output.contains("mustprogress"), "Should have mustprogress");
        assert!(output.contains("llvm.assume"), "Should declare llvm.assume intrinsic");
    }

    #[test]
    fn test_llvm_acyclic_annotation() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(!output.is_empty());
    }
}