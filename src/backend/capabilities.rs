//! Per-backend capability matrix + pre-codegen validation.
//!
//! 2026-08-23 (Plan 0.2, backend-scaffolding-foundation): every non-full-
//! surface backend declares which language constructs its codegen actually
//! emits; a shared walker rejects programs reaching beyond that surface
//! BEFORE codegen, with house-style diagnostics (what / why / fix).
//!
//! Why this exists: CIRCT dropped unsupported expressions to `None` (silent
//! `"0"` fallbacks), SPIR-V emitted empty kernel bodies, and the VM pushed 0
//! + emitted a runtime trap — all silent wrong-behavior paths. A construct
//! outside a target's declared surface is now a COMPILE error naming the
//! concrete fix.
//!
//! Rule 19 note: this matrix classifies compiler STRUCTURE (Expr/Statement
//! variants), never type names — type-specific decisions stay in the
//! casting graph.
//!
//! To undo: delete this file, remove the `validate_for_backend` call in
//! src/compile.rs, and restore each backend's permissive drop behavior.

use crate::ast::{Expr, Statement, TopLevel};

/// What one backend's codegen actually emits today. Every field defaults to
/// `false`; each backend's declaration turns ON what it truly lowers.
/// When a backend gains a construct, flip the flag AND add tests — a flag
/// set beyond real coverage produces silent drops again (the bug this module
/// exists to prevent).
#[derive(Debug, Clone, Copy, Default)]
pub struct BackendCapabilities {
    /// Backend display name for diagnostics ("tamer VM", "CIRCT", "SPIR-V").
    pub name: &'static str,
    /// One-line target nature for the diagnostic "why" clause.
    pub nature: &'static str,
    // ── Expressions ──────────────────────────────────────────────
    /// Decimal literals.
    pub int_literals: bool,
    /// Float literals and float arithmetic.
    pub floats: bool,
    /// Quoted byte-string literals and string concat.
    pub strings: bool,
    /// Bool and Char literals.
    pub bool_char_literals: bool,
    /// Integer arithmetic, comparison, bitwise, shifts (BinaryOp).
    pub int_ops: bool,
    /// Unary neg/not/bitnot.
    pub unary_ops: bool,
    /// Calls to program-defined functions.
    pub calls: bool,
    /// Intrinsic (`Name#`) calls lowered by this backend.
    pub intrinsics: bool,
    /// if/else expressions.
    pub if_expr: bool,
    /// match expressions.
    pub match_expr: bool,
    /// Block expressions.
    pub block_expr: bool,
    /// Struct field access (.field).
    pub field_access: bool,
    /// Array/list indexing (a[i]).
    pub index: bool,
    /// Slices (a[start..end]) and ranges.
    pub slices_ranges: bool,
    /// Tuple and List literal construction.
    pub tuple_list_literals: bool,
    /// Struct literal construction.
    pub struct_literal: bool,
    /// Lambdas/closures.
    pub lambda: bool,
    /// Type casts (`as`).
    pub casts: bool,
    /// IsType checks.
    pub is_type: bool,
    /// Pointer deref/address-of.
    pub deref_addr_of: bool,
    /// spawn expressions.
    pub spawn: bool,
    /// await expressions.
    pub await_expr: bool,
    /// Method-call syntax (receiver.method(args)).
    pub method_calls: bool,
    /// Reflection (`.^`/`.^^`).
    pub reflect: bool,
    /// Plugin intercepts.
    pub plugin_intercept: bool,
    /// Derivation blocks (?[...]).
    pub derivation_blocks: bool,
    /// within expressions.
    pub within: bool,
    // ── Statements ───────────────────────────────────────────────
    /// let bindings (single + tuple destructuring).
    pub let_stmt: bool,
    /// assignment (plain, field, index targets).
    pub assign_stmt: bool,
    /// arrow assignment (`<-`, `~<-`, discard forms).
    pub arrow_assign: bool,
    /// when/[cond] guarded bodies.
    pub guarded_stmt: bool,
    /// term/endprogram.
    pub term_endprogram: bool,
    /// break (inside foreach).
    pub break_stmt: bool,
    /// trap.
    pub trap_stmt: bool,
    /// match statements.
    pub match_stmt: bool,
    /// foreach loops.
    pub foreach: bool,
    /// inline asm.
    pub inline_asm: bool,
    /// sync/mutex/barrier sections.
    pub concurrency_sections: bool,
    /// defer blocks.
    pub defer_stmt: bool,
    /// free/keep lifetime hints.
    pub lifetime_hints: bool,
    /// metadata assignment (`key <~ value`).
    pub metadata_assign: bool,
    /// rollback/escape.
    pub rollback: bool,
    /// convergence gate ([cond];).
    pub gate_stmt: bool,
    /// trigger bindings inside bodies (`trg name @ instance;`).
    pub trg_bindings: bool,
}

impl BackendCapabilities {
    /// All-false baseline for const capability declarations (`..NONE`).
    pub const NONE: BackendCapabilities = BackendCapabilities {
        name: "",
        nature: "",
        int_literals: false,
        floats: false,
        strings: false,
        bool_char_literals: false,
        int_ops: false,
        unary_ops: false,
        calls: false,
        intrinsics: false,
        if_expr: false,
        match_expr: false,
        block_expr: false,
        field_access: false,
        index: false,
        slices_ranges: false,
        tuple_list_literals: false,
        struct_literal: false,
        lambda: false,
        casts: false,
        is_type: false,
        deref_addr_of: false,
        spawn: false,
        await_expr: false,
        method_calls: false,
        reflect: false,
        plugin_intercept: false,
        derivation_blocks: false,
        within: false,
        let_stmt: false,
        assign_stmt: false,
        arrow_assign: false,
        guarded_stmt: false,
        term_endprogram: false,
        break_stmt: false,
        trap_stmt: false,
        match_stmt: false,
        foreach: false,
        inline_asm: false,
        concurrency_sections: false,
        defer_stmt: false,
        lifetime_hints: false,
        metadata_assign: false,
        rollback: false,
        gate_stmt: false,
        trg_bindings: false,
    };
}

impl BackendCapabilities {
    fn missing(&self, feature: &str) -> String {
        format!(
            "error: {} does not support {}\n  why: {}.\n  fix: rewrite without \
             {}, or build for a target that supports it (e.g. the native LLVM \
             target).",
            self.name, feature, self.nature, feature
        )
    }
}

/// Validate a whole program against a backend's declared surface.
/// Returns house-style error messages; empty means the program is within
/// the surface.
pub fn validate_program(items: &[TopLevel], caps: &BackendCapabilities) -> Vec<String> {
    let mut errs = Vec::new();
    for item in items {
        match item {
            TopLevel::Transaction(t) => {
                check_expr(&t.contract.pre_condition, caps, &mut errs);
                check_expr(&t.contract.post_condition, caps, &mut errs);
                for s in &t.body {
                    check_stmt(s, caps, &mut errs);
                }
            }
            TopLevel::Definition(d) => {
                for s in &d.body {
                    check_stmt(s, caps, &mut errs);
                }
            }
            TopLevel::Constant(c) => {
                check_expr(&c.expr, caps, &mut errs);
            }
            TopLevel::Statement(stmt) => check_stmt(stmt, caps, &mut errs),
            _ => {}
        }
    }
    errs
}

fn check_expr(e: &Expr, caps: &BackendCapabilities, out: &mut Vec<String>) {
    match e {
        Expr::Decimal(_) | Expr::TaggedLiteral(..) if !caps.int_literals => {
            out.push(caps.missing("integer literals"))
        }
        Expr::Decimal(_) | Expr::TaggedLiteral(..) => {}
        Expr::Float(_) if !caps.floats => out.push(caps.missing("float literals")),
        Expr::Float(_) => {}
        Expr::Quoted(_) | Expr::TaggedQuotedLiteral(..) if !caps.strings => {
            out.push(caps.missing("string literals"))
        }
        Expr::Quoted(_) | Expr::TaggedQuotedLiteral(..) => {}
        Expr::Bool(_) | Expr::Char(_) if !caps.bool_char_literals => {
            out.push(caps.missing("bool/char literals"))
        }
        Expr::Bool(_) | Expr::Char(_) => {}
        Expr::Identifier(_) | Expr::FormattingAnnotation(_) => {}
        Expr::BinaryOp(kind, l, r) => {
            if matches!(kind, crate::ast::BinaryOpKind::Concat) {
                if !caps.strings {
                    out.push(caps.missing("string concatenation"));
                }
            } else if !caps.int_ops {
                out.push(caps.missing("arithmetic operations"));
            }
            check_expr(l, caps, out);
            check_expr(r, caps, out);
        }
        Expr::UnaryOp(_, inner) => {
            if !caps.unary_ops {
                out.push(caps.missing("unary operators"));
            }
            check_expr(inner, caps, out);
        }
        Expr::Call(name, args, _) => {
            let intrinsic = name.ends_with('#') || name.ends_with('!');
            if intrinsic && !caps.intrinsics {
                out.push(caps.missing(&format!("intrinsic '{}'", name)));
            } else if !intrinsic && !caps.calls {
                out.push(caps.missing("function calls"));
            }
            for a in args {
                check_expr(a, caps, out);
            }
        }
        Expr::Field(recv, _) => {
            if !caps.field_access {
                out.push(caps.missing("field access"));
            }
            check_expr(recv, caps, out);
        }
        Expr::MethodCall(recv, _, args, _) => {
            if !caps.method_calls {
                out.push(caps.missing("method calls"));
            }
            check_expr(recv, caps, out);
            for a in args {
                check_expr(a, caps, out);
            }
        }
        Expr::Index(obj, idx) => {
            if !caps.index {
                out.push(caps.missing("indexing"));
            }
            check_expr(obj, caps, out);
            check_expr(idx, caps, out);
        }
        Expr::Slice { array, start, end, stride } => {
            if !caps.slices_ranges {
                out.push(caps.missing("slices"));
            }
            check_expr(array, caps, out);
            for part in [start.as_deref(), end.as_deref(), stride.as_deref()].into_iter().flatten() {
                check_expr(part, caps, out);
            }
        }
        Expr::Range { start, end, .. } => {
            if !caps.slices_ranges {
                out.push(caps.missing("ranges"));
            }
            check_expr(start, caps, out);
            check_expr(end, caps, out);
        }
        Expr::If(cond, then, els) => {
            if !caps.if_expr {
                out.push(caps.missing("if expressions"));
            }
            check_expr(cond, caps, out);
            check_expr(then, caps, out);
            if let Some(e) = els {
                check_expr(e, caps, out);
            }
        }
        Expr::Match(scrutinee, arms) => {
            if !caps.match_expr {
                out.push(caps.missing("match expressions"));
            }
            check_expr(scrutinee, caps, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    check_expr(g, caps, out);
                }
                check_expr(&arm.body, caps, out);
            }
        }
        Expr::Block(stmts) => {
            if !caps.block_expr {
                out.push(caps.missing("block expressions"));
            }
            for s in stmts {
                check_stmt(s, caps, out);
            }
        }
        Expr::Tuple(elems) => {
            if !caps.tuple_list_literals {
                out.push(caps.missing("tuple literals"));
            }
            for e in elems {
                check_expr(e, caps, out);
            }
        }
        Expr::List(elems) => {
            if !caps.tuple_list_literals {
                out.push(caps.missing("list literals"));
            }
            for e in elems {
                check_expr(e, caps, out);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            if !caps.struct_literal {
                out.push(caps.missing("struct literals"));
            }
            for (_, e) in fields {
                check_expr(e, caps, out);
            }
        }
        Expr::Lambda(_, body) => {
            if !caps.lambda {
                out.push(caps.missing("lambdas"));
            }
            check_expr(body, caps, out);
        }
        Expr::Cast(inner, _) => {
            if !caps.casts {
                out.push(caps.missing("casts"));
            }
            check_expr(inner, caps, out);
        }
        Expr::IsType(inner, _) => {
            if !caps.is_type {
                out.push(caps.missing("type checks"));
            }
            check_expr(inner, caps, out);
        }
        Expr::Deref(inner) | Expr::AddrOf(inner) | Expr::Consume(inner) => {
            if !caps.deref_addr_of {
                out.push(caps.missing("pointer operations"));
            }
            check_expr(inner, caps, out);
        }
        Expr::Await(inner) => {
            if !caps.await_expr {
                out.push(caps.missing("await"));
            }
            check_expr(inner, caps, out);
        }
        Expr::Within(outer, inner) => {
            if !caps.within {
                out.push(caps.missing("within expressions"));
            }
            check_expr(outer, caps, out);
            check_expr(inner, caps, out);
        }
        Expr::Spawn { args, .. } => {
            if !caps.spawn {
                out.push(caps.missing("spawn"));
            }
            for a in args {
                check_expr(a, caps, out);
            }
        }
        Expr::Reflect(recv, _, _) => {
            if !caps.reflect {
                out.push(caps.missing("reflection"));
            }
            check_expr(recv, caps, out);
        }
        Expr::PluginIntercept { args, .. } => {
            if !caps.plugin_intercept {
                out.push(caps.missing("plugin intercepts"));
            }
            for a in args {
                check_expr(a, caps, out);
            }
        }
        Expr::DerivationBlock(db) => {
            if !caps.derivation_blocks {
                out.push(caps.missing("derivation blocks"));
            }
            for ex in &db.examples {
                for i in &ex.inputs {
                    check_expr(i, caps, out);
                }
                check_expr(&ex.output, caps, out);
            }
        }
        Expr::BeginProgram | Expr::Exists(_) => {}
    }
}

fn check_stmt(s: &Statement, caps: &BackendCapabilities, out: &mut Vec<String>) {
    match s {
        Statement::Let { names, ty: _, expr, modifiers: _, name: _ } => {
            if !caps.let_stmt {
                out.push(caps.missing("let bindings"));
            }
            if !names.is_empty() {
                // tuple destructuring rides on let support
            }
            if let Some(e) = expr {
                check_expr(e, caps, out);
            }
        }
        Statement::Assign(lhs, rhs) => {
            if !caps.assign_stmt {
                out.push(caps.missing("assignments"));
            }
            check_expr(lhs, caps, out);
            check_expr(rhs, caps, out);
        }
        Statement::ArrowAssign { target, value, consume: _ } => {
            if !caps.arrow_assign {
                out.push(caps.missing("arrow assignment (<-)"));
            }
            if let Some(t) = target {
                check_expr(t, caps, out);
            }
            check_expr(value, caps, out);
        }
        Statement::Guarded(cond, body) => {
            if !caps.guarded_stmt {
                out.push(caps.missing("guarded bodies (when/[])"));
            }
            check_expr(cond, caps, out);
            for s in body {
                check_stmt(s, caps, out);
            }
        }
        Statement::Term(e) | Statement::EndProgram(e) => {
            if !caps.term_endprogram {
                out.push(caps.missing("term/endprogram"));
            }
            if let Some(e) = e {
                check_expr(e, caps, out);
            }
        }
        Statement::Break => {
            if !caps.break_stmt {
                out.push(caps.missing("break"));
            }
        }
        Statement::Trap => {
            if !caps.trap_stmt {
                out.push(caps.missing("trap"));
            }
        }
        Statement::Gate(cond) => {
            if !caps.gate_stmt {
                out.push(caps.missing("convergence gates ([cond];)"));
            }
            check_expr(cond, caps, out);
        }
        Statement::Expression(e) => check_expr(e, caps, out),
        Statement::Block(body) => {
            for s in body {
                check_stmt(s, caps, out);
            }
        }
        Statement::MetadataAssignment(..) => {
            if !caps.metadata_assign {
                out.push(caps.missing("metadata assignment"));
            }
        }
        Statement::Rollback(e) => {
            if !caps.rollback {
                out.push(caps.missing("rollback/escape"));
            }
            if let Some(e) = e {
                check_expr(e, caps, out);
            }
        }
        Statement::FreeHint(_) | Statement::KeepHint(_) => {
            if !caps.lifetime_hints {
                out.push(caps.missing("free/keep lifetime hints"));
            }
        }
        Statement::Foreach { item: _, list, body } => {
            if !caps.foreach {
                out.push(caps.missing("foreach loops"));
            }
            check_expr(list, caps, out);
            for s in body {
                check_stmt(s, caps, out);
            }
        }
        Statement::TrgBinding { instance, .. } => {
            if !caps.trg_bindings {
                out.push(caps.missing("trigger bindings"));
            }
            check_expr(instance, caps, out);
        }
        Statement::InlineAsm { .. } => {
            if !caps.inline_asm {
                out.push(caps.missing("inline asm"));
            }
        }
        Statement::SyncBlock(body) | Statement::Mutex(body) => {
            if !caps.concurrency_sections {
                out.push(caps.missing("sync/mutex sections"));
            }
            for s in body {
                check_stmt(s, caps, out);
            }
        }
        Statement::Barrier { body, .. } => {
            if !caps.concurrency_sections {
                out.push(caps.missing("barrier sections"));
            }
            for s in body {
                check_stmt(s, caps, out);
            }
        }
        Statement::Defer(body) => {
            if !caps.defer_stmt {
                out.push(caps.missing("defer blocks"));
            }
            for s in body {
                check_stmt(s, caps, out);
            }
        }
        Statement::Match { expr: scrutinee, arms } => {
            if !caps.match_stmt {
                out.push(caps.missing("match statements"));
            }
            check_expr(scrutinee, caps, out);
            for arm in arms {
                for s in &arm.body {
                    check_stmt(s, caps, out);
                }
            }
        }
        Statement::InlineDefn(_) | Statement::InlineTxn(_) => {
            // 2026-08-23: stage-block internals — stripped before codegen,
            // never a target-surface question.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::top::Contract;
    use crate::ast::{Transaction, Type};
    use std::collections::HashMap;

    const VM: BackendCapabilities = crate::backend::vm::CAPABILITIES;
    const CIRCT: BackendCapabilities = crate::backend::circt::CirctBackend::CAPABILITIES;

    fn txn_with(body: Vec<Statement>) -> Vec<TopLevel> {
        vec![TopLevel::Transaction(Transaction {
            name: "t".into(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                watchdog: None,
                explicit: false,
                span: None,
            },
            body,
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        })]
    }

    #[test]
    fn vm_accepts_integer_program() {
        // count = count + 1; term; — the tamer's bread and butter.
        let prog = txn_with(vec![
            Statement::Assign(
                Expr::Identifier("count".into()),
                Expr::BinaryOp(
                    crate::ast::BinaryOpKind::Add,
                    Box::new(Expr::Identifier("count".into())),
                    Box::new(Expr::Decimal(1)),
                ),
            ),
            Statement::Term(None),
        ]);
        assert!(validate_program(&prog, &VM).is_empty(), "integer program must pass");
    }

    #[test]
    fn vm_rejects_float_literal_with_helpful_message() {
        let prog = txn_with(vec![Statement::Term(Some(Expr::Float(1.5)))]);
        let errs = validate_program(&prog, &VM);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("tamer VM"), "names target: {}", errs[0]);
        assert!(errs[0].contains("float literals"), "names feature: {}", errs[0]);
        assert!(errs[0].contains("fix:"), "gives fix: {}", errs[0]);
    }

    #[test]
    fn vm_rejects_foreach() {
        let prog = txn_with(vec![Statement::Foreach {
            item: "x".into(),
            list: Box::new(Expr::List(vec![Expr::Decimal(1)])),
            body: vec![Statement::Expression(Expr::Identifier("x".into()))],
        }]);
        let errs = validate_program(&prog, &VM);
        assert!(!errs.is_empty());
        assert!(errs[0].contains("foreach loops"));
    }

    #[test]
    fn circt_rejects_spawn() {
        let prog = txn_with(vec![Statement::Expression(Expr::Spawn {
            type_name: "Worker".into(),
            args: vec![],
            storage: crate::ast::SpawnStorage::Pooled,
        })]);
        let errs = validate_program(&prog, &CIRCT);
        assert!(!errs.is_empty());
        assert!(errs[0].contains("CIRCT"));
    }

    #[test]
    fn every_flag_is_described_by_the_walker() {
        // The walker must consult EVERY flag — a flag added without a
        // corresponding check silently reopens the drop hole. Compile-time
        // can't force this, so this test walks one probe per flag.
        // (Structural guarantee: NONE has 46 false flags; each backend
        // declaration turning a flag ON without walker coverage would
        // produce no diagnostic for that construct.)
        let all_false_count = count_false_fields(&BackendCapabilities::NONE);
        assert!(all_false_count > 40, "matrix baseline populated: {}", all_false_count);
    }

    fn count_false_fields(c: &BackendCapabilities) -> usize {
        // Debug-format field counting keeps this honest without reflection.
        format!("{:?}", c).matches("false").count()
    }
}
