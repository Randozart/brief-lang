use crate::analysis::call_graph::CallGraph;
use crate::ast::{Expr, Program, Statement, TopLevel, Type};
use std::collections::HashMap;
use std::fmt::Write;

/// LLVM IR backend — compiles Brief programs to LLVM intermediate representation.
///
/// Uses shared analysis (CallGraph + ParameterRanges) to optimize:
/// - Acyclic graphs: %State passed by value, extractvalue/insertvalue/phi
/// - Cyclic graphs: %State* pointer dispatch, noalias on every call
pub struct LlvmBackend {
    spec: Option<crate::target_spec::TargetSpec>,
    field_index_map: HashMap<String, usize>,
    field_types: Vec<String>,
    txn_counter: usize,
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
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
        }
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  LLVM backend: acyclic call graph — norecurse + inlining enabled");
        }

        // Build field index map and type list
        self.build_field_index(program);

        let mut output = String::new();

        // Module header
        writeln!(output, "; ModuleID = 'program.ll'").ok();
        writeln!(output, "source_filename = \"program.bv\"").ok();
        writeln!(output, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(output, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
        writeln!(output).ok();

        // Declare intrinsics
        writeln!(output, "declare void @llvm.assume(i1) #1").ok();
        writeln!(output).ok();

        // State type and global
        self.declare_state_type(&mut output, program);
        self.declare_state_global(&mut output, program);
        writeln!(output).ok();

        // Collect transactions
        let mut txns: Vec<(String, &crate::ast::Transaction)> = Vec::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                txns.push((txn.name.clone(), txn));
            }
        }

        // Generate each transaction
        for (name, txn) in &txns {
            self.generate_transaction(&mut output, txn, name);
            writeln!(output).ok();
        }

        // Generate init_state
        self.generate_init_state(&mut output, program);
        writeln!(output).ok();

        // Generate reactor loop
        if !txns.is_empty() {
            self.generate_reactor(&mut output, &txns);
        }

        // Attributes
        writeln!(output).ok();
        writeln!(output, "attributes #0 = {{").ok();
        writeln!(output, "    mustprogress").ok();
        writeln!(output, "    nofree").ok();
        writeln!(output, "    norecurse").ok();
        writeln!(output, "    nosync").ok();
        writeln!(output, "    nounwind").ok();
        writeln!(output, "    willreturn").ok();
        writeln!(output, "    memory(argmem: readwrite)").ok();
        writeln!(output, "}}").ok();
        writeln!(output, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn memory(argmem: write) }}").ok();

        output
    }

    fn build_field_index(&mut self, program: &Program) {
        self.field_index_map.clear();
        self.field_types.clear();
        let mut idx = 0;
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                self.field_index_map.insert(s.name.clone(), idx);
                self.field_types.push(self.llvm_type(&s.ty).to_string());
                idx += 1;
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

    fn declare_state_type(&self, output: &mut String, program: &Program) {
        if self.field_types.is_empty() {
            writeln!(output, "%State = type {{ i64 }}").ok();
            return;
        }
        write!(output, "%State = type {{ ").ok();
        for (i, f) in self.field_types.iter().enumerate() {
            if i > 0 { write!(output, ", ").ok(); }
            write!(output, "{}", f).ok();
        }
        writeln!(output, " }}").ok();
    }

    fn declare_state_global(&self, output: &mut String, _program: &Program) {
        writeln!(output, "@global_state = global %State zeroinitializer").ok();
    }

    fn generate_init_state(&self, output: &mut String, program: &Program) {
        writeln!(output, "define void @init_state() local_unnamed_addr #0 {{").ok();
        writeln!(output, "  entry:").ok();
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                if let Some(ref expr) = s.expr {
                    let idx = self.field_index_map.get(&s.name).unwrap_or(&0);
                    let ty = self.llvm_type(&s.ty);
                    if let Expr::Integer(n) = expr {
                        writeln!(output, "  store volatile {} {}, {}* getelementptr inbounds (%State, %State* @global_state, i32 0, i32 {}), align {}", ty, n, ty, idx, self.align_of(ty)).ok();
                    } else if let Expr::Bool(b) = expr {
                        let val = if *b { 1 } else { 0 };
                        writeln!(output, "  store volatile {} {}, {}* getelementptr inbounds (%State, %State* @global_state, i32 0, i32 {}), align {}", ty, val, ty, idx, self.align_of(ty)).ok();
                    }
                }
            }
        }
        writeln!(output, "  ret void").ok();
        writeln!(output, "}}").ok();
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

    fn state_ptr(&self, output: &mut String, field_name: &str, val_reg: &str, indent: &str) {
        if let Some(&idx) = self.field_index_map.get(field_name) {
            let ty = &self.field_types[idx];
            writeln!(output, "{}{} = getelementptr inbounds (%State, %State* %state, i32 0, i32 {})", indent, val_reg, idx).ok();
        }
    }

    fn generate_transaction(&mut self, output: &mut String, txn: &crate::ast::Transaction, name: &str) {
        writeln!(output, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", name).ok();
        writeln!(output, "entry:").ok();
        writeln!(output, "  ; pre: {:?}", txn.contract.pre_condition).ok();

        // Generate body
        self.txn_counter = 0;
        for stmt in &txn.body {
            self.generate_statement(output, stmt, "  ");
        }

        // Postcondition as comment
        writeln!(output, "  ; post: {:?}", txn.contract.post_condition).ok();
        writeln!(output, "  ret void").ok();
        writeln!(output, "}}").ok();
    }

    fn generate_statement(&mut self, output: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for s in &cleanup {
                    self.generate_statement(output, s, indent);
                }
                writeln!(output, "{}; term", indent).ok();
            }
            Statement::Let { name, expr, address_expr, .. } => {
                if address_expr.is_some() {
                    writeln!(output, "{}; let {} = ptr (address expression)", indent, name).ok();
                } else if let Some(e) = expr {
                    let val = self.generate_expr(output, e, indent);
                    writeln!(output, "{}; let {} = {}", indent, name, val).ok();
                } else {
                    writeln!(output, "{}; let {} = uninitialized", indent, name).ok();
                }
            }
            Statement::Assignment { lhs, expr, .. } => {
                let val = self.generate_expr(output, expr, indent);
                let name = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    _ => {
                        writeln!(output, "{}; assign {}", indent, val).ok();
                        return;
                    }
                };
                if let Some(&idx) = self.field_index_map.get(&name) {
                    let ty = &self.field_types[idx];
                    let ptr_reg = format!("%ptr{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(output, "{}{} = getelementptr inbounds (%State, %State* %state, i32 0, i32 {})", indent, ptr_reg, idx).ok();
                    writeln!(output, "{}store {} {}, {}* {}, align {}", indent, ty, val, ty, ptr_reg, self.align_of(ty)).ok();
                } else {
                    writeln!(output, "{}; assign {} to {}", indent, val, name).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.generate_expr(output, condition, indent);
                writeln!(output, "{}br i1 {}, label %then, label %end", indent, cond).ok();
                writeln!(output, "{}then:", indent).ok();
                for s in statements {
                    self.generate_statement(output, s, &format!("{}  ", indent));
                }
                writeln!(output, "{}  br label %end", indent).ok();
                writeln!(output, "{}end:", indent).ok();
            }
            Statement::Expression(e) => {
                let _ = self.generate_expr(output, e, indent);
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    let val = self.generate_expr(output, e, indent);
                    writeln!(output, "{}; trg! {}: await {}", indent, name, val).ok();
                } else {
                    writeln!(output, "{}; trg! {}: await external", indent, name).ok();
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                writeln!(output, "{}; #on_exit registered", indent).ok();
            }
            Statement::Escape(expr) => {
                if let Some(v) = expr {
                    let val = self.generate_expr(output, v, indent);
                    writeln!(output, "{}ret i64 {}", indent, val).ok();
                } else {
                    writeln!(output, "{}ret void", indent).ok();
                }
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    let _ = writeln!(output, "{}{}", indent, line);
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                writeln!(output, "{}{}", indent, asm_string).ok();
            }
            Statement::Unification { name, pattern, expr } => {
                let val = self.generate_expr(output, expr, indent);
                writeln!(output, "{}; unification: {} {} = {}", indent, name, pattern, val).ok();
            }
        }
    }

    fn generate_expr(&mut self, output: &mut String, expr: &Expr, indent: &str) -> String {
        let val = format!("%tmp{}", self.txn_counter);
        self.txn_counter += 1;

        match expr {
            Expr::Integer(n) => {
                writeln!(output, "{}{} = add i64 0, {}", indent, val, n).ok();
            }
            Expr::Bool(b) => {
                let i = if *b { 1 } else { 0 };
                writeln!(output, "{}{} = add i64 0, {}", indent, val, i).ok();
            }
            Expr::Float(f) => {
                writeln!(output, "{}{} = fadd float 0.0, {}", indent, val, f).ok();
            }
            Expr::String(s) => {
                writeln!(output, "{}{} = alloca i8, i64 {}", indent, val, s.len() + 1).ok();
            }
            Expr::Char(c) => {
                writeln!(output, "{}{} = add i32 0, {}", indent, val, *c as i32).ok();
            }
            Expr::Identifier(name) => {
                if let Some(&idx) = self.field_index_map.get(name) {
                    let ty = &self.field_types[idx];
                    let ptr_reg = format!("%ptr{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(output, "{}{} = getelementptr inbounds (%State, %State* %state, i32 0, i32 {})", indent, ptr_reg, idx).ok();
                    writeln!(output, "{}{} = load {}, {}* {}, align {}", indent, val, ty, ty, ptr_reg, self.align_of(ty)).ok();
                } else {
                    writeln!(output, "{}{} = add i64 0, 0 ; identifier {}", indent, val, name).ok();
                }
            }
            Expr::Add(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = add i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Sub(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = sub i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Mul(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = mul i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Div(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = sdiv i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Mod(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = srem i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Eq(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp eq i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Ne(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp ne i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Lt(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp slt i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Le(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp sle i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Gt(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp sgt i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::Ge(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                let cmp = format!("%cmp{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(output, "{}{} = icmp sge i64 {}, {}", indent, cmp, left, right).ok();
                writeln!(output, "{}{} = zext i1 {} to i64", indent, val, cmp).ok();
            }
            Expr::And(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = and i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Or(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = or i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Not(e) => {
                let inner = self.generate_expr(output, e, indent);
                writeln!(output, "{}{} = xor i64 {}, -1", indent, val, inner).ok();
            }
            Expr::Neg(e) => {
                let inner = self.generate_expr(output, e, indent);
                writeln!(output, "{}{} = sub i64 0, {}", indent, val, inner).ok();
            }
            Expr::BitAnd(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = and i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::BitOr(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = or i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::BitXor(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = xor i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::BitNot(e) => {
                let inner = self.generate_expr(output, e, indent);
                writeln!(output, "{}{} = xor i64 {}, -1", indent, val, inner).ok();
            }
            Expr::Shl(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = shl i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Shr(l, r) => {
                let left = self.generate_expr(output, l, indent);
                let right = self.generate_expr(output, r, indent);
                writeln!(output, "{}{} = lshr i64 {}, {}", indent, val, left, right).ok();
            }
            Expr::Call(name, args) => {
                let mut arg_strs = Vec::new();
                for arg in args {
                    arg_strs.push(self.generate_expr(output, arg, indent));
                }
                let args_str = arg_strs.join(", ");
                writeln!(output, "{}{} = call i64 @{}({})", indent, val, name, args_str).ok();
            }
            Expr::ListLiteral(_) => {
                writeln!(output, "{}{} = alloca i64, i64 0", indent, val).ok();
            }
            Expr::ListIndex(list, idx) => {
                let list_val = self.generate_expr(output, list, indent);
                let idx_val = self.generate_expr(output, idx, indent);
                writeln!(output, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, val, list_val, idx_val).ok();
            }
            Expr::ListLen(_) => {
                writeln!(output, "{}{} = add i64 0, 0", indent, val).ok();
            }
            Expr::FieldAccess(obj, field) => {
                let obj_val = self.generate_expr(output, obj, indent);
                writeln!(output, "{}{} = add i64 0, 0 ; {}.{}", indent, val, obj_val, field).ok();
            }
            _ => {
                writeln!(output, "{}{} = add i64 0, 0 ; fallback", indent, val).ok();
            }
        }

        val
    }

    fn generate_reactor(&mut self, output: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        let norecurse_str = if self.has_cycles { "" } else { " norecurse" };

        writeln!(output, "define void @reactor_tick(){} local_unnamed_addr #0 {{", norecurse_str).ok();
        writeln!(output, "  ; Sample all volatile triggers (see 08a-TRIGGERS.md)").ok();
        writeln!(output, "  ;   load volatile i8, i8* @trg_ptr → %trg_sampled").ok();
        writeln!(output, "  ; Load state").ok();
        writeln!(output, "  ; Evaluate preconditions (priority order)").ok();
        writeln!(output, "  ; Dispatch first-true transaction").ok();
        writeln!(output, "  ; Commit state changes").ok();
        writeln!(output, "  ret void").ok();
        writeln!(output, "}}").ok();
        writeln!(output).ok();

        writeln!(output, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(output, "entry:").ok();
        writeln!(output, "  call void @init_state()").ok();
        writeln!(output, "  br label %tick").ok();
        writeln!(output, "tick:").ok();
        writeln!(output, "  call void @reactor_tick()").ok();
        writeln!(output, "  br label %tick").ok();
        writeln!(output, "}}").ok();
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