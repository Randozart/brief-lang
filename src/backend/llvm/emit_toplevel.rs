use crate::ast::{Expr, Statement, TopLevel, Type};
use crate::backend::llvm::{float_to_llvm_hex, LlvmBackend, TypedRegister};
use std::fmt::Write;

impl LlvmBackend {
    pub(super) fn emit_header(&self, out: &mut String) {
        writeln!(out, "; ModuleID = 'program.ll'").ok();
        writeln!(out, "source_filename = \"program.bv\"").ok();
        writeln!(out, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(out, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
    }

    pub(super) fn emit_declares(&self, out: &mut String) {
        writeln!(out).ok();
        writeln!(out, "declare void @llvm.assume(i1) #1").ok();
        // Intrinsic declares used by name#() instrinsic calls in emit_expr.
        // Previously these came from std/llvm.bv via as intrinsic, but the
        // name#() mechanism emits them directly without frgn_map entries.
        writeln!(out, "declare float @llvm.sqrt.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.fabs.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.ceil.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.floor.f32(float) #1").ok();
        writeln!(out, "declare i64 @llvm.ctpop.i64(i64) #1").ok();
        writeln!(out, "declare i64 @llvm.ctlz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.cttz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.abs.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.bitreverse.i64(i64) #1").ok();
        // Runtime support functions
        writeln!(out, "declare void @brief_barrier_release()").ok();
        writeln!(out, "declare void @brief_barrier_wait()").ok();
        writeln!(out, "declare void @brief_thread_pool_init(i32, i8**)").ok();
        writeln!(out, "declare i64 @time(i64*) nounwind").ok();
        writeln!(out, "declare noalias i8* @malloc(i64) nounwind").ok();
        writeln!(out, "declare i64 @brief_read_file(i64)").ok();
    }

    pub(super) fn llvm_type(&self, ty: &Type) -> &str {
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

    pub(super) fn native_float_or_box(&mut self, out: &mut String, indent: &str, val_reg: &str) -> String {
        if let Some(cached) = self.reg_float_cache.get(val_reg) {
            return cached.clone();
        }
        let tr = format!("%nftr{}", self.txn_counter); self.txn_counter += 1;
        let fl = format!("%nffl{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val_reg).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        fl
    }

    pub(super) fn ensure_float_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        // Check cache first — even float-typed registers may have their
        // native float counterpart cached (e.g. FFI return value boxed as i64).
        if let Some(cached) = self.reg_float_cache.get(&reg.name) {
            return cached.clone();
        }
        if reg.ty == Type::Float {
            return reg.name.clone();
        }
        self.native_float_or_box(out, indent, &reg.name)
    }

    pub(super) fn emit_trg_load(&mut self, out: &mut String, indent: &str, dst: &str, addr_src: &str, addr_is_ptr: bool, trg_ty: &Type) {
        let store_ty = super::trg_llvm_storage_ty(trg_ty);
        let tr_counter = self.txn_counter;
        self.txn_counter += 1;
        let raw = format!("%tr{}", tr_counter);
        if addr_is_ptr {
            writeln!(out, "{}{} = load volatile {}, {}* {}", indent, raw, store_ty, store_ty, addr_src).ok();
        } else {
            writeln!(out, "{}{} = load volatile {}, {}* inttoptr (i64 {} to {}*), align 1", indent, raw, store_ty, store_ty, addr_src, store_ty).ok();
        }
        match trg_ty {
            Type::Bool => {
                let zc = self.txn_counter; self.txn_counter += 1;
                let z = format!("%tz{}", zc);
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, z).ok();
            }
            Type::Int | Type::UInt => {
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, raw).ok();
            }
            Type::Char => {
                let zc = self.txn_counter; self.txn_counter += 1;
                let z = format!("%tz{}", zc);
                writeln!(out, "{}{} = zext i32 {} to i64", indent, z, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, z).ok();
            }
            Type::String | Type::Data => {
                let pc = self.txn_counter; self.txn_counter += 1;
                let p = format!("%tp{}", pc);
                writeln!(out, "{}{} = ptrtoint {} {} to i64", indent, p, store_ty, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, p).ok();
            }
            _ => {
                let zc = self.txn_counter; self.txn_counter += 1;
                let z = format!("%tz{}", zc);
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, z).ok();
            }
        }
    }

    pub(super) fn align_of(&self, ty: &str) -> u32 {
        match ty {
            "i64" => 8,
            "float" => 4,
            "i8" => 1,
            "i32" => 4,
            _ => 8,
        }
    }

    pub(super) fn declare_state_type(&mut self, out: &mut String) {
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

    pub(super) fn emit_init_state(&mut self, out: &mut String) {
        writeln!(out, "define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        let mut reg = 0u32;
        let mut fields: Vec<(String, usize, String)> = self.field_index_map.iter()
            .map(|(name, &idx)| (name.clone(), idx, self.field_types[idx].clone()))
            .collect();
        fields.sort_by_key(|&(_, idx, _)| idx);
        for (name, idx, ty) in fields {
            let p = format!("%ip{}", reg); reg += 1;
            writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", p, idx).ok();
            let init_clone = self.field_initializers.get(&name).and_then(|e| e.clone());
            match init_clone {
                Some(Expr::Integer(n)) => {
                    writeln!(out, "  store i64 {}, i64* {}, align {}", n, p, self.align_of("i64")).ok();
                }
                Some(Expr::Float(f)) => {
                    let h = float_to_llvm_hex(f);
                    let bits_reg = format!("%ip{}b", reg - 1);
                    writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                    writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                }
                Some(Expr::Neg(ref inner)) => {
                    match inner.as_ref() {
                        Expr::Float(f) => {
                            let h = float_to_llvm_hex(-*f);
                            let bits_reg = format!("%ip{}b", reg - 1);
                            writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                            writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                        }
                        Expr::Literal(lit) => {
                            if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                                let h = float_to_llvm_hex(-*f);
                                let bits_reg = format!("%ip{}b", reg - 1);
                                writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                                writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                            } else {
                                writeln!(out, "  store i64 0, i64* {}, align {}", p, self.align_of("i64")).ok();
                            }
                        }
                        Expr::Integer(n) => {
                            writeln!(out, "  store i64 -{}, i64* {}, align {}", n, p, self.align_of("i64")).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 0, i64* {}, align {}", p, self.align_of("i64")).ok();
                        }
                    }
                }
                Some(Expr::Bool(b)) => {
                    let v = if b { "1" } else { "0" };
                    writeln!(out, "  store i8 {}, i8* {}, align {}", v, p, self.align_of("i8")).ok();
                }
                Some(Expr::String(_)) => {
                    writeln!(out, "  store i8* null, i8** {}, align {}", p, self.align_of("i8*")).ok();
                }
                Some(Expr::Char(c)) => {
                    let v = c as i32;
                    writeln!(out, "  store i32 {}, i32* {}, align {}", v, p, self.align_of("i32")).ok();
                }
                Some(expr) => {
                    let val_reg = self.emit_expr(out, &expr, "  ");
                    match ty.as_str() {
                        "i8" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = trunc i64 {} to i8", t, val_reg).ok();
                            writeln!(out, "  store i8 {}, i8* {}, align {}", t, p, self.align_of("i8")).ok();
                        }
                        "i32" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = trunc i64 {} to i32", t, val_reg).ok();
                            writeln!(out, "  store i32 {}, i32* {}, align {}", t, p, self.align_of("i32")).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, "  ", &val_reg.to_string());
                            writeln!(out, "  store float {}, float* {}, align {}", fl, p, self.align_of("float")).ok();
                        }
                        "i8*" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = inttoptr i64 {} to i8*", t, val_reg).ok();
                            writeln!(out, "  store i8* {}, i8** {}, align {}", t, p, self.align_of("i8*")).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 {}, {}* {}, align {}", val_reg, ty, p, self.align_of(&ty)).ok();
                        }
                    }
                }
                None => {
                    let default = if ty == "i8*" { "null".to_string() } else { "0".to_string() };
                    writeln!(out, "  store {} {}, {}* {}, align {}", ty, default, ty, p, self.align_of(&ty)).ok();
                }
            }
        }
        let mmio_inits: Vec<(u64, Expr)> = {
            let mut v = Vec::new();
            for (name, &addr) in &self.mmio_fields {
                if let Some(Some(expr)) = self.mmio_initializers.get(name).cloned() {
                    v.push((addr, expr.clone()));
                }
            }
            v
        };
        for (addr, expr) in mmio_inits {
            let p = format!("%mio{}", reg); reg += 1;
            writeln!(out, "  {} = inttoptr i64 {} to i64*", p, addr).ok();
            let val_reg = self.emit_expr(out, &expr, "  ");
            writeln!(out, "  store volatile i64 {}, i64* {}, align 1", val_reg, p).ok();
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition) {
        self.pending_cleanup.clear();
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        write!(out, "define i64 @{}(", d.name).ok();
        write!(out, "%State* noalias nocapture %state").ok();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv = format!("%ac{}", i);
            let reg: String;
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
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
                reg = conv;
            } else {
                reg = raw;
            }
            self.let_bindings.insert(n.clone(), reg);
        }
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = true;
        for s in &d.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret i64 0").ok(); }
        writeln!(out, "}}").ok();
    }
    // 2026-06-13: Added %State* %state param — definitions can access global state.
    // Was missing the state pointer, causing invalid LLVM IR (SSA value out of scope).

    pub(super) fn emit_transaction(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str, range_meta: &mut Vec<String>) {
        let has_output = txn.output_type.is_some() || !txn.outputs.is_empty();
        if !txn.is_reactive && (!txn.parameters.is_empty() || has_output) {
            self.emit_callable_txn(out, txn, name);
            return;
        }
        self.pending_cleanup.clear();
        self.range_bounds = Self::extract_ranges(&txn.contract.pre_condition);
        self.field_to_meta_idx.clear();
        for (f, &(lo, hi)) in &self.range_bounds {
            if hi < i64::MAX {
                let mi = range_meta.len();
                let dlo = if lo > i64::MIN { lo } else { i64::MIN };
                range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, dlo, hi));
                self.field_to_meta_idx.insert(f.clone(), mi);
            }
        }
        let alwaysinline = if !self.has_cycles { " alwaysinline" } else { "" };
        let txn_attr = self.slp_attr(name, "#0");

        let assume_action: Option<&str> = txn.modifiers.iter()
            .find(|m| m.name == "assume_shape")
            .and_then(|m| m.value.as_ref())
            .and_then(|v| {
                let parts: Vec<&str> = v.splitn(2, ", ").collect();
                if parts.len() == 2 {
                    let action = parts[1].trim();
                    if action == "run" || action == "exit" { Some(action) } else { Some("escape") }
                } else {
                    Some("escape")
                }
            });

        if let Some(action) = assume_action {
            writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            writeln!(out, "  br i1 true, label %body, label %rollback").ok();
            writeln!(out, "  body:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            for s in &txn.body { self.emit_stmt(out, s, "  "); }
            if !self.terminated { writeln!(out, "  ret void").ok(); }
            writeln!(out, "  rollback:").ok();
            match action {
                "exit" => {
                    writeln!(out, "    call void @__exit(i64 1)").ok();
                    writeln!(out, "    unreachable").ok();
                }
                "run" => {
                    writeln!(out, "    br label %body").ok();
                }
                _ => {
                    writeln!(out, "    ret void").ok();
                }
            }
            writeln!(out, "}}").ok();
        } else {
            writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            for s in &txn.body { self.emit_stmt(out, s, "  "); }
            if !self.terminated { writeln!(out, "  ret void").ok(); }
            writeln!(out, "}}").ok();
        }
    }

    pub(super) fn emit_callable_txn(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        self.pending_cleanup.clear();
        self.let_bindings.clear();
        self.let_binding_types.clear();
        self.reg_float_cache.clear();
        self.reg_type_cache.clear();
        self.param_slots.clear();

        let has_return = if let Some(ref ot) = txn.output_type {
            match ot {
                crate::ast::OutputType::Single(ty) => !matches!(ty, Type::Void),
                crate::ast::OutputType::Tuple(ts) => !ts.is_empty(),
                _ => false,
            }
        } else {
            !txn.outputs.is_empty() && !matches!(txn.outputs.first(), Some(Type::Void))
        };
        let ret_llvm = if has_return { "i64" } else { "void" };

        write!(out, "define {} @{}(", ret_llvm, name).ok();
        write!(out, "%State* noalias nocapture %state").ok();
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();

        writeln!(out, "  %result = alloca i64, align 8").ok();
        writeln!(out, "  store i64 0, i64* %result, align 8").ok();

        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv: String;
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
                let ac = format!("%ac{}", i);
                match t {
                    Type::Bool => { writeln!(out, "  {} = zext i8 {} to i64", ac, raw).ok(); }
                    Type::Char => { writeln!(out, "  {} = zext i32 {} to i64", ac, raw).ok(); }
                    Type::String | Type::Data => { writeln!(out, "  {} = ptrtoint i8* {} to i64", ac, raw).ok(); }
                    Type::Float => {
                        let m = format!("%ai{}", i);
                        writeln!(out, "  {} = bitcast float {} to i32", m, raw).ok();
                        writeln!(out, "  {} = zext i32 {} to i64", ac, m).ok();
                    }
                    _ => {}
                }
                conv = ac;
            } else {
                conv = raw;
            }
            let slot = format!("%p{}_s", i);
            writeln!(out, "  {} = alloca i64, align 8", slot).ok();
            writeln!(out, "  store i64 {}, i64* {}, align 8", conv, slot).ok();
            self.param_slots.insert(n.clone(), slot);
        }

        writeln!(out, "  br label %loop").ok();
        writeln!(out, "loop:").ok();

        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let slot = format!("%p{}_s", i);
            let loaded = format!("%p{}_l{}", i, self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "  {} = load i64, i64* {}, align 8", loaded, slot).ok();
            self.let_bindings.insert(n.clone(), loaded);
            self.let_binding_types.insert(n.clone(), t.clone());
        }

        self.callable_txn_result = Some("%result".to_string());
        self.callable_txn_post_label = Some("post".to_string());
        self.in_callable_txn = true;
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = has_return;

        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
            let i1 = format!("%pc{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            writeln!(out, "  br i1 {}, label %body, label %done", i1).ok();
        } else {
            writeln!(out, "  br label %body").ok();
        }

        writeln!(out, "body:").ok();

        for s in &txn.body {
            self.emit_stmt(out, s, "  ");
            if self.terminated {
                self.terminated = false;
            }
        }

        writeln!(out, "post:").ok();
        writeln!(out, "  br label %loop").ok();

        writeln!(out, "done:").ok();
        if has_return {
            let ret = format!("%ret{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = load i64, i64* %result, align 8", ret).ok();
            writeln!(out, "  ret i64 {}", ret).ok();
        } else {
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();

        self.callable_txn_result = None;
        self.callable_txn_post_label = None;
        self.in_callable_txn = false;
        self.param_slots.clear();
    }
    // 2026-06-13: Added %State* %state param — callable txns can access global state.
    // Was missing the state pointer, causing invalid LLVM IR (SSA value out of scope).

    pub(super) fn emit_precondition_check(&mut self, out: &mut String, pre: &Expr, indent: &str) {
        let cond = self.emit_expr(out, pre, indent);
        let i1 = format!("%pi{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();
        let panic_l = format!("pp{}", self.txn_counter); self.txn_counter += 1;
        let safe_l = format!("ps{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, safe_l, panic_l).ok();
        writeln!(out, "{}{}:", indent, panic_l).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, safe_l).ok();
        writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
    }

    pub(super) fn emit_pre_function(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        if matches!(txn.contract.pre_condition, Expr::Bool(true)) { return; }
        writeln!(out, "define internal i1 @pre_{}(%State* noalias nocapture %state) #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
        writeln!(out, "  ret i1 {}", i1).ok();
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_async_body(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        let async_name = format!("async_body_{}", name);
        let async_attr = self.slp_attr(&async_name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", async_name, async_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
        let txn_fire_l = format!("txn_fire_{}", self.txn_counter + 1);
        writeln!(out, "  br i1 {}, label %{}, label %{}_done", i1, txn_fire_l, async_name).ok();
        writeln!(out, "{}:", txn_fire_l).ok();
        self.terminated = false;
        self.returns_i64 = false;
        for s in &txn.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "{}_done:", async_name).ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_fused(&mut self, out: &mut String, a: &crate::ast::Transaction, b: &crate::ast::Transaction, name: &str) {
        let body_a: Vec<Statement> = a.body.iter()
            .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. } | Statement::Escape(_)))
            .cloned().collect();
        let combined: Vec<Statement> = body_a.into_iter().chain(b.body.iter().cloned()).collect();
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in &combined { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_shape_guarded_body(&mut self, out: &mut String, body: &[Statement], name: &str, action: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  br i1 true, label %body, label %rollback").ok();
        writeln!(out, "  body:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "  rollback:").ok();
        match action {
            "exit" => {
                writeln!(out, "    call void @__exit(i64 1)").ok();
                writeln!(out, "    unreachable").ok();
            }
            "run" => {
                writeln!(out, "    br label %body").ok();
            }
            _ => {
                writeln!(out, "    ret void").ok();
            }
        }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_fused_composed(&mut self, out: &mut String, body: &[Statement], name: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }
}
