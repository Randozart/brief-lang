use crate::ast::{Expr, Hashtag, Statement, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use std::fmt::Write;

pub struct ForeachStmt {
    pub item: String,
    pub list: Box<Expr>,
    pub body: Vec<Statement>,
    pub modifiers: Vec<Hashtag>,
}

impl StmtTypecheck for ForeachStmt {
    fn typecheck(&self, _ctx: &mut crate::typechecker::TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> {
        Ok(())
    }
}

impl StmtEval for ForeachStmt {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        let list_val = ctx.eval_expr(&self.list)?;
        match list_val {
            Value::List(items) => {
                for elem in items {
                    ctx.state.insert(self.item.clone(), elem);
                    for stmt in &self.body {
                        ctx.exec_stmt(stmt)?;
                    }
                }
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch(
                "foreach requires a List<T> value".to_string(),
            )),
        }
    }
}

impl StmtCodegenLLVM for ForeachStmt {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &StmtDispatch, indent: &str,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) {
        let list_val = ctx.emit_expr(out, &self.list, indent);
        let tc = ctx.fun.txn_counter; ctx.fun.txn_counter += 20;
        let hp = format!("%fe_hp_{}", tc);
        writeln!(out, "{0}{1} = inttoptr i64 {2} to i64*", indent, hp, list_val.name).ok();
        let dp_gep = format!("%fe_dp_gep_{}", tc);
        writeln!(out, "{0}{1} = getelementptr i64, i64* {2}, i64 0", indent, dp_gep, hp).ok();
        let dp_val = format!("%fe_dp_{}", tc);
        writeln!(out, "{0}{1} = load i64, i64* {2}, align 8", indent, dp_val, dp_gep).ok();
        let ep = format!("%fe_ep_{}", tc);
        writeln!(out, "{0}{1} = inttoptr i64 {2} to i64*", indent, ep, dp_val).ok();
        let len_gep = format!("%fe_len_gep_{}", tc);
        writeln!(out, "{0}{1} = getelementptr i64, i64* {2}, i64 1", indent, len_gep, hp).ok();
        let len_val = format!("%fe_len_{}", tc);
        writeln!(out, "{0}{1} = load i64, i64* {2}, align 8", indent, len_val, len_gep).ok();
        let idx_slot = format!("%fe_idx_slot_{}", tc);
        writeln!(out, "{0}{1} = alloca i64, align 8", indent, idx_slot).ok();
        writeln!(out, "{}store i64 0, i64* {}", indent, idx_slot).ok();
        let hdr_l = format!("fe_hdr_{}", tc);
        let body_l = format!("fe_body_{}", tc);
        let done_l = format!("fe_done_{}", tc);
        writeln!(out, "{}br label %{}", indent, hdr_l).ok();
        writeln!(out, "{}{}:", indent, hdr_l).ok();
        let cur_idx = format!("%fe_cur_{}", tc);
        writeln!(out, "{0}{1} = load i64, i64* {2}, align 8", indent, cur_idx, idx_slot).ok();
        let idx_cmp = format!("%fe_cmp_{}", tc);
        writeln!(out, "{0}{1} = icmp slt i64 {2}, {3}", indent, idx_cmp, cur_idx, len_val).ok();
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, idx_cmp, body_l, done_l).ok();
        writeln!(out, "{}{}:", indent, body_l).ok();
        let elem_gep = format!("%fe_elem_gep_{}", tc);
        writeln!(out, "{0}{1} = getelementptr i64, i64* {2}, i64 {3}", indent, elem_gep, ep, cur_idx).ok();
        let elem_val = format!("%fe_elem_{}", tc);
        writeln!(out, "{0}{1} = load i64, i64* {2}, align 8", indent, elem_val, elem_gep).ok();
        let prev_item = ctx.fun.let_bindings.insert(self.item.clone(), elem_val.clone());
        let prev_item_ty = ctx.fun.let_binding_types.insert(self.item.clone(), Type::Int);
        for s in &self.body {
            ctx.emit_stmt(out, s, &format!("{}  ", indent));
        }
        if let Some(prev) = prev_item {
            ctx.fun.let_bindings.insert(self.item.clone(), prev);
            ctx.fun.let_binding_types.insert(self.item.clone(), prev_item_ty.unwrap_or(Type::Int));
        } else {
            ctx.fun.let_bindings.remove(self.item.as_str());
            ctx.fun.let_binding_types.remove(self.item.as_str());
        }
        let next_idx = format!("%fe_next_{}", tc);
        writeln!(out, "{0}{1} = add i64 {2}, 1", indent, next_idx, cur_idx).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, next_idx, idx_slot).ok();
        // Resolve loop directives (#unroll, #vectorize, etc.) from modifiers.
        let dir_effects = crate::backend::llvm::directive::resolve_directives(
            &self.modifiers,
            crate::backend::llvm::directive::DirectiveCtx::Loop,
        );
        // Emit optimization remarks for speculative loop directives.
        for tag in &self.modifiers {
            if !tag.speculative { continue; }
            match tag.name.as_str() {
                "unroll" => {
                    let is_full = dir_effects.iter().any(|e| {
                        matches!(e, crate::backend::llvm::directive::DirectiveEffect::LoopMetadata(k, _) if k == "llvm.loop.unroll.full")
                    });
                    let is_enable = dir_effects.iter().any(|e| {
                        matches!(e, crate::backend::llvm::directive::DirectiveEffect::LoopMetadata(k, _) if k == "llvm.loop.unroll.enable")
                    });
                    if is_full {
                        ctx.push_remark(crate::backend::llvm::directive::OptimizationRemark::applied(
                            "unroll", "full unroll applied via foreach".to_string()));
                    } else if is_enable {
                        ctx.push_remark(crate::backend::llvm::directive::OptimizationRemark::applied(
                            "unroll", "heuristic unroll enabled via foreach".to_string()));
                    }
                }
                "vectorize" => {
                    let is_enabled = dir_effects.iter().any(|e| {
                        matches!(e, crate::backend::llvm::directive::DirectiveEffect::LoopMetadata(k, _) if k == "llvm.loop.vectorize.enable")
                    });
                    if is_enabled {
                        ctx.push_remark(crate::backend::llvm::directive::OptimizationRemark::applied(
                            "vectorize", "vectorization enabled via foreach".to_string()));
                    }
                }
                _ => {}
            }
        }
        // Build loop metadata nodes. Always emit the canonical self-referencing
        // loop metadata node. Additional nodes are added for each directive.
        let mut md_count = 1; // count of metadata operands beyond self-ref
        let mut md_entries = Vec::new();
        for effect in &dir_effects {
            if let crate::backend::llvm::directive::DirectiveEffect::LoopMetadata(key, val) = effect {
                let entry_md = ctx.fun.metadata_counter + md_count;
                if val.is_empty() {
                    writeln!(out, "!{0} = !{{!\"{1}\"}}", entry_md, key).ok();
                } else {
                    writeln!(out, "!{0} = !{{!\"{1}\", {2}}}", entry_md, key, val).ok();
                }
                md_entries.push(format!("!{}", entry_md));
                md_count += 1;
            }
        }
        // Default: emit !llvm.loop.vectorize.enable = true if no vectorize directive present.
        let has_vectorize = dir_effects.iter().any(|e| {
            matches!(e, crate::backend::llvm::directive::DirectiveEffect::LoopMetadata(k, _) if k == "llvm.loop.vectorize.enable")
        });
        if !has_vectorize {
            let vd_md = ctx.fun.metadata_counter + md_count;
            writeln!(out, "!{} = !{{!\"llvm.loop.vectorize.enable\", i1 true}}", vd_md).ok();
            md_entries.push(format!("!{}", vd_md));
            md_count += 1;
        }
        let md_idx = ctx.fun.metadata_counter;
        let entries = md_entries.join(", ");
        writeln!(out, "!{0} = !{{!{0}, {1}}}", md_idx, entries).ok();
        writeln!(out, "{}br label %{} !llvm.loop !{}", indent, hdr_l, md_idx).ok();
        ctx.fun.metadata_counter += md_count;
        writeln!(out, "{}{}:", indent, done_l).ok();
    }
}

impl StmtCodegenWebstack for ForeachStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
