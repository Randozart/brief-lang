// ── Arrow Operation Codegen ─────────────────────────────────────
//
// 2026-06-30: Extracted from rest.rs (lines 934-1427).
// Handles ArrowMut (Push/Pop), ArrowDiscard, and ArrowTransfer
// expression emission for the LLVM backend.
//
// ArrowMut Push:   `&list <- x` or `&list << x`
// ArrowMut Pop:    `x <- &list` or `x << &list`
// ArrowDiscard:    `<- &list` or `<- &list[i]`
// ArrowTransfer:   `&dest <- &source`
//
// Pointer-tagging assumption: i64 is used as the universal handle type.
// String pointers have the lower 2 bits masked off, verified against
// target platform alignment at compile time.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

// ── emit_arrow_push ─────────────────────────────────────────────
//
// Handle list append: box both list and element, unbox header, read
// length, try fast path (preallocated capacity), fall back to
// malloc+memcpy for new buffer, update state field.

pub fn emit_arrow_push(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    target: &Box<Expr>,
    _index: &Box<Expr>,
    val: &Box<Expr>,
    indent: &str,
) -> TypedRegister {
    let list_val = backend.emit_expr(out, target, indent);
    let elem_val = backend.emit_expr(out, val, indent);
    let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
    let elem_boxed = backend.adapt_to_i64(out, indent, &elem_val);
    // Check InsertAt strategy: Custom functions get an early call emission,
    // built-in strategies determine prepend vs append behavior.
    let push_strategy = backend.check_insert_strategy(target);
    if let Some(crate::type_universe::InsertStrategy::Custom(fn_name)) = &push_strategy {
        // 2026-07-01: Check if the custom strategy is an intrinsic.
        // If so, emit the intrinsic inline via Expr::IntrinsicCall, avoiding a
        // function call wrapper. This is used by ring_push for RingBuffer<T>.
        if let Some(intrinsic) = crate::ast::Intrinsic::from_name(fn_name) {
            let call_expr = crate::ast::Expr::IntrinsicCall {
                intrinsic,
                args: vec![
                    target.as_ref().clone(),
                    val.as_ref().clone(),
                ],
            };
            let result = backend.emit_expr(out, &call_expr, indent);
            // Store result back to state field (intrinsic may return updated handle)
            if let Expr::OwnedRef(field_name) = target.as_ref() {
                if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                    let ap = format!("%aap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                    let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, result.name, ap, tn).ok();
                } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, result.name, slot).ok();
                }
            }
            return TypedRegister { name: result.name.clone(), ty: Type::Int };
        }
        // Custom push: emit call @fn_name(i64, i64) -> i64
        writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})", indent, v, fn_name, list_boxed, elem_boxed).ok();
        // Store new list handle back to state field if target is OwnedRef
        if let Expr::OwnedRef(field_name) = target.as_ref() {
            if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                let ap = format!("%aap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, v, ap, tn).ok();
            } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, v, slot).ok();
            }
        }
        return TypedRegister { name: v.to_string(), ty: Type::Int };
    }
    let prepend = matches!(push_strategy, Some(crate::type_universe::InsertStrategy::Prepend));
    // Unbox list header: inttoptr i64 to i64*
    let hp = format!("%ahp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
    // Read current length from header slot 1
    let lp = format!("%alp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
    let old_len = format!("%aol{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, old_len, lp).ok();
    // Phase 2 fast path: if preallocated capacity exists and
    // length < capacity, write directly without alloc/memcpy.
    // Only works for append — prepend requires element shifting
    // which the fast path doesn't support. The slow_l label is
    // emitted here (always) so the branch target exists even
    // when the fast path returns early.
    let slow_l = format!("push_slow_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    // Track whether we emitted a branch to slow_l. If not (e.g.
    // prepend mode or no prealloc info), we need a br label %slow_l
    // to terminate the preceding basic block before the label.
    let mut emitted_slow_branch = false;
    if !prepend {
    if let Expr::OwnedRef(field_name) = target.as_ref() {
        if let Some((cap_reg, buf_i64)) = backend.fun.field_prealloc_info.get(field_name.as_str()).cloned() {
            let cap_check = format!("%acap{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = icmp ult i64 {}, {}", indent, cap_check, old_len, cap_reg).ok();
            let fast_l = format!("push_fast_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cap_check, fast_l, slow_l).ok();
            emitted_slow_branch = true;
            // Fast path: write element at header[2 + old_len], increment length.
            // Uses buf_i64 (preallocated i64* buffer from prealloc_info)
            // rather than hp (which alias the same memory but may be stale
            // after the first iteration resets state via the normal store path).
            writeln!(out, "{}{}:", indent, fast_l).ok();
            let el_off = format!("%apfo{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 {}, 2", indent, el_off, old_len).ok();
            let el_gep = format!("%apfg{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, el_gep, buf_i64, el_off).ok();
            let new_len_fast = format!("%apfn{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 {}, 1", indent, new_len_fast, old_len).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem_boxed, el_gep).ok();
            // Update length in header slot 1 of the preallocated buffer
            let len_gep = format!("%apfl{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, len_gep, buf_i64).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len_fast, len_gep).ok();
            // Store back to state (buffer pointer unchanged — we modified
            // the preallocated buffer in-place, no new allocation).
            let store_idx = backend.ctx.field_index_map[field_name];
            let ap = format!("%aapf{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, store_idx).ok();
            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[store_idx]);
            let base_fast = format!("%apfb{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, base_fast, buf_i64).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base_fast, ap, tn).ok();
            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, buf_i64).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Int };
        }
    }
    }
    // 2026-06-26: If the fast-path branch was not emitted (e.g.
    // prepend mode or no prealloc info), terminate the preceding
    // block with br %slow_l so LLVM does not see an unterminated
    // basic block before the label. The push_slow label was always
    // emitted unconditionally, but its preceding br i1 only fired
    // when prealloc info existed — leaving the block unterminated
    // ("expected instruction opcode") for all other cases.
    if !emitted_slow_branch {
        writeln!(out, "{}br label %{}", indent, slow_l).ok();
    }
    writeln!(out, "{}{}:", indent, slow_l).ok();
    // Allocate: when inside an arena scope (loop/tick), use bump
    // alloc (no free — arena resets at scope exit). Outside a
    // scope, fall back to per-operation malloc via emit_arena_alloc.
    let new_cnt = format!("%anc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 3", indent, new_cnt, old_len).ok();
    let alloc_bytes = format!("%aab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
    let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
    // Free old buffer: when arena is active, the arena owns all
    // memory — no per-operation free needed. When arena is inactive
    // (standalone call), free the old buffer normally.
    if backend.fun.arena_slots.is_none() {
        let old_ptr = format!("%aop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
        writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
    }
    // Set header: data_ptr at slot 0, new length at slot 1
    let new_hp = format!("%anh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
    let base = format!("%aba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
    let dp = format!("%adp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, new_hp).ok();
    let nlp = format!("%anp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
    let new_len = format!("%anl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 1", indent, new_len, old_len).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
    // Copy old elements: for prepend, shift right by 1; for append, same position
    let old_dp = format!("%aod{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, old_dp, hp).ok();
    let copy_dst = if prepend {
        // Prepend: copy to position 1 (one slot after base)
        let cd = format!("%acd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 3", indent, cd, new_hp).ok();
        cd
    } else {
        // Append: copy to position old_len (same position as before)
        let cd = format!("%acd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, cd, new_hp).ok();
        cd
    };
    let copy_bytes = format!("%acb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, copy_bytes, old_len).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, copy_dst, old_dp, copy_bytes).ok();
    // Store new element at position 0 for prepend, or old_len+2 for append
    // (list header is 2 slots: capacity at 0, length at 1; elements start at 2).
    let ne_ptr = format!("%aep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    let new_elem_pos = if prepend {
        "2".to_string()
    } else {
        // 2026-06-27: Compute element position = old_len + 2 account for
        // the 2 header slots. Previously used old_len directly, which
        // overwrote header slot 0 or the first element (queue_drain crash).
        let ep = format!("%aep2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, 2", indent, ep, old_len).ok();
        ep
    };
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ne_ptr, new_hp, new_elem_pos).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem_boxed, ne_ptr).ok();
    // Store new list handle back to state field if target is OwnedRef
    if let Expr::OwnedRef(field_name) = target.as_ref() {
        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
            let ap = format!("%aap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
            // 2026-06-27: Record field as modified for per-field phi
            // back-edge reload. Without this, the phi back-edge for
            // this field remains a pass-through (old value), causing
            // the next tick to see the pre-push handle (queue_drain).
            backend.fun.pending_phi_backedge.insert(field_name.clone(), base.clone());
        } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
        }
    }
    // Return new list handle (ptrtoint of new buffer)
    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, new_buf).ok();
    return TypedRegister { name: v.to_string(), ty: Type::Int };
}

// ── emit_arrow_pop ──────────────────────────────────────────────
//
// Handle list pop/shift: compute target index (0 for shift, len-1
// for pop, or expression), free old buffer, malloc+memcpy for new
// buffer of len-1, return popped element, update state field.

pub fn emit_arrow_pop(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    target: &Box<Expr>,
    index: &Box<Expr>,
    indent: &str,
) -> TypedRegister {
    let pop_strategy = backend.check_extract_strategy(target);
    if let Some(crate::type_universe::ExtractStrategy::Custom(fn_name)) = &pop_strategy {
        // 2026-07-01: Check if the custom strategy is an intrinsic (name ends with #).
        // If so, emit the intrinsic inline. RingPop returns the popped value directly
        // (i64), and the handle is NOT updated (ring buffer mutates in place).
        if let Some(intrinsic) = crate::ast::Intrinsic::from_name(fn_name) {
            let call_expr = crate::ast::Expr::IntrinsicCall {
                intrinsic,
                args: vec![target.as_ref().clone()],
            };
            let result = backend.emit_expr(out, &call_expr, indent);
            // Ring buffer handle is immutable (mutates in-place in heap).
            // The handle in %State does NOT change — no store or backedge
            // tracking needed. The phi will pass through the same handle.
            return TypedRegister { name: result.name.clone(), ty: Type::Int };
        }
        // Custom extract: call @fn_name(i64) -> { i64, i64 }
        let list_val = backend.emit_expr(out, target, indent);
        let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
        let call_reg = format!("%pc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = call {{ i64, i64 }} @{}(i64 {})", indent, call_reg, fn_name, list_boxed).ok();
        // Extract popped value (index 0) and new collection handle (index 1)
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, v, call_reg).ok();
        let new_list = format!("%pnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 1", indent, new_list, call_reg).ok();
        // Store new list handle back to state
        if let Expr::OwnedRef(field_name) = target.as_ref() {
            if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                let ap = format!("%pap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, new_list, ap, tn).ok();
                // 2026-06-27: Record field as modified for per-field
                // phi back-edge reload (same rationale as push).
                backend.fun.pending_phi_backedge.insert(field_name.clone(), new_list.clone());
            } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_list, slot).ok();
            }
        }
        let popped = v.to_string();
        return TypedRegister { name: popped, ty: Type::Int };
    }
    // Determine pop index: Shift strategy removes from front (0), otherwise end (len-1)
    let should_shift = matches!(pop_strategy, Some(crate::type_universe::ExtractStrategy::Shift));
    let list_val = backend.emit_expr(out, target, indent);
    let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
    // Unbox list header
    let hp = format!("%php{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
    // Read length
    let lp = format!("%plp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
    let len = format!("%pln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
    // Compute target index: 0 for shift, len - 1 for pop, or expression value
    let pop_idx = format!("%ppi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    match index.as_ref() {
        Expr::Term if should_shift => {
            writeln!(out, "{}{} = add i64 0, 0", indent, pop_idx).ok();
        }
        Expr::Term => {
            writeln!(out, "{}{} = add i64 {}, -1", indent, pop_idx, len).ok();
        }
        other => {
            let idx_val = backend.emit_expr(out, other, indent);
            let idx_boxed = backend.adapt_to_i64(out, indent, &idx_val);
            writeln!(out, "{}{} = add i64 {}, 0", indent, pop_idx, idx_boxed).ok();
        }
    }
    // Load popped element from data_ptr[pop_idx]
    let dp = format!("%pdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
    let ep = format!("%pep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, dp, pop_idx).ok();
    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, ep).ok();
    let popped = v.to_string();
    // Free old buffer: arena-active skips per-op free; standalone frees
    if backend.fun.arena_slots.is_none() {
        let old_ptr = format!("%pop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
        writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
    }
    // Allocate new buffer: (len + 1) * 8 (2 header + len - 1 elements)
    let new_cnt = format!("%pnc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 1", indent, new_cnt, len).ok();
    let alloc_bytes = format!("%pab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
    let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
    // Set header
    let new_hp = format!("%pnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
    let base = format!("%pba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
    let new_dp_val = format!("%pnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 16", indent, new_dp_val, base).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_dp_val, new_hp).ok();
    let nlp = format!("%pnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
    let new_len = format!("%pnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, -1", indent, new_len, len).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
    // Copy elements before pop_idx
    let ndp = format!("%pndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
    let bef_bytes = format!("%pbb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, bef_bytes, pop_idx).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, ndp, dp, bef_bytes).ok();
    // Copy elements after pop_idx
    let after_off = format!("%pao{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 1", indent, after_off, pop_idx).ok();
    let aft_src = format!("%pas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_src, dp, after_off).ok();
    let aft_dst = format!("%pad{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_dst, ndp, pop_idx).ok();
    let aft_cnt = format!("%pac{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = sub i64 {}, {}", indent, aft_cnt, new_len, pop_idx).ok();
    let aft_bytes = format!("%pab2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, aft_bytes, aft_cnt).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, aft_dst, aft_src, aft_bytes).ok();
    // Store updated list back
    if let Expr::OwnedRef(field_name) = target.as_ref() {
        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
            let ap = format!("%pap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
            // 2026-06-27: Record field as modified for per-field
            // phi back-edge reload (same rationale as push).
            backend.fun.pending_phi_backedge.insert(field_name.clone(), base.clone());
        } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
        }
    }
    return TypedRegister { name: popped.to_string(), ty: Type::Int };
}

// ── emit_arrow_discard ──────────────────────────────────────────
//
// Handle discard (element removal without return): free old buffer,
// allocate new buffer of len-1, memcpy before/after discard index,
// update state field.

pub fn emit_arrow_discard(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    target: &Box<Expr>,
    index: &Box<Expr>,
    indent: &str,
) -> TypedRegister {
    // 2026-07-01: Check for custom discard strategy (e.g., ring_pop for RingBuffer).
    // If the target has a custom ExtractFrom strategy that maps to an intrinsic,
    // emit it inline — no arena alloc, no memcpy, just head/tail pointer arithmetic.
    let pop_strategy = backend.check_extract_strategy(target);
    if let Some(crate::type_universe::ExtractStrategy::Custom(fn_name)) = &pop_strategy {
        if let Some(intrinsic) = crate::ast::Intrinsic::from_name(fn_name) {
            let call_expr = crate::ast::Expr::IntrinsicCall {
                intrinsic,
                args: vec![target.as_ref().clone()],
            };
            let _result = backend.emit_expr(out, &call_expr, indent);
            // Discard: result is not needed, ring buffer handle is unchanged.
            return TypedRegister { name: v.to_string(), ty: Type::Int };
        }
    }
    let list_val = backend.emit_expr(out, target, indent);
    let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
    // Unbox list header, read length
    let hp = format!("%dhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
    let lp = format!("%dlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
    let len = format!("%dln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
    // Compute discard index
    let discard_idx = format!("%ddi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    if matches!(index.as_ref(), Expr::Term) {
        writeln!(out, "{}{} = add i64 {}, -1", indent, discard_idx, len).ok();
    } else {
        let iv = backend.emit_expr(out, index, indent);
        let ib = backend.adapt_to_i64(out, indent, &iv);
        writeln!(out, "{}{} = add i64 {}, 0", indent, discard_idx, ib).ok();
    }
    // Free old buffer: arena-active skips per-op free; standalone frees
    if backend.fun.arena_slots.is_none() {
        let old_ptr = format!("%dop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
        writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
    }
    // Allocate new buffer: (len + 1) slots (2 header + len - 1 elements)
    let new_cnt = format!("%dnc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 1", indent, new_cnt, len).ok();
    let alloc_bytes = format!("%dab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
    let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
    // Set header
    let new_hp = format!("%dnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
    let base = format!("%dba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
    let ndv = format!("%dnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 16", indent, ndv, base).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ndv, new_hp).ok();
    let nlp = format!("%dnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
    let new_len = format!("%dnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, -1", indent, new_len, len).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
    // Copy before discard_idx
    let dp = format!("%ddp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
    let ndp = format!("%dndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
    let bef_bytes = format!("%dbb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, bef_bytes, discard_idx).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, ndp, dp, bef_bytes).ok();
    // Copy after discard_idx
    let after_off = format!("%dao{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 1", indent, after_off, discard_idx).ok();
    let aft_src = format!("%das{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_src, dp, after_off).ok();
    let aft_dst = format!("%dad{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_dst, ndp, discard_idx).ok();
    let aft_cnt = format!("%dac{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = sub i64 {}, {}", indent, aft_cnt, new_len, discard_idx).ok();
    // 2026-07-01: Use %dabcp (dab-copy) prefix — NOT %dab2 — to prevent
    // register name collision with %dab{N}. Prefix %dab2 + counter 63
    // produces "dab263" which is identical to %dab + counter 263.
    let aft_bytes = format!("%dabcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, aft_bytes, aft_cnt).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, aft_dst, aft_src, aft_bytes).ok();
    // Store updated list back
    if let Expr::OwnedRef(field_name) = target.as_ref() {
        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
            let ap = format!("%dap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
        } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
        }
    }
    writeln!(out, "{}{} = add i64 0, {} ; discard", indent, v, base).ok();
    return TypedRegister { name: v.to_string(), ty: Type::Int };
}

// ── emit_arrow_transfer ─────────────────────────────────────────
//
// Handle transfer (move all elements from source to dest): free
// both old buffers, allocate new combined buffer, memcpy dest then
// source, set source to empty list, update state fields.

pub fn emit_arrow_transfer(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    dest: &Box<Expr>,
    source: &Box<Expr>,
    indent: &str,
) -> TypedRegister {
    // Unfiltered: move all elements from source to dest
    let dest_val = backend.emit_expr(out, dest, indent);
    let src_val = backend.emit_expr(out, source, indent);
    let dest_boxed = backend.adapt_to_i64(out, indent, &dest_val);
    let src_boxed = backend.adapt_to_i64(out, indent, &src_val);
    let dhp = format!("%tdh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, dhp, dest_boxed).ok();
    let shp = format!("%tsh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, shp, src_boxed).ok();
    // Read lengths
    let dlp = format!("%tdl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, dlp, dhp).ok();
    let dlen = format!("%tdn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, dlen, dlp).ok();
    let slp = format!("%tsl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, shp).ok();
    let slen = format!("%tsn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, slen, slp).ok();
    // Total = dest_len + src_len
    let total = format!("%ttl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, {}", indent, total, dlen, slen).ok();
    // Free old buffers: arena skips per-op free; standalone frees
    if backend.fun.arena_slots.is_none() {
        let dold = format!("%tdop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, dold, dest_boxed).ok();
        writeln!(out, "{}call void @free(i8* {})", indent, dold).ok();
        let sold = format!("%tsop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, sold, src_boxed).ok();
        writeln!(out, "{}call void @free(i8* {})", indent, sold).ok();
    }
    // Allocate new dest buffer: (total + 2) * 8
    let alloc_slots = format!("%tas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 2", indent, alloc_slots, total).ok();
    let alloc_bytes = format!("%tab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, alloc_slots).ok();
    let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
    // Set dest header
    let new_hp = format!("%tnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
    let dbase = format!("%tdb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, dbase, new_buf).ok();
    let ndv = format!("%tnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 16", indent, ndv, dbase).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ndv, new_hp).ok();
    let tnlp = format!("%tnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, tnlp, new_hp).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, total, tnlp).ok();
    // Copy dest elements
    let ddp = format!("%tddp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ddp, dhp).ok();
    let ndp = format!("%tndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
    let dbytes = format!("%tdb2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, dbytes, dlen).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, ndp, ddp, dbytes).ok();
    // Copy source elements after dest
    let sdp = format!("%tsdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, sdp, shp).ok();
    let src_off = format!("%tso{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, src_off, ndp, dlen).ok();
    let sbytes = format!("%tsb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, sbytes, slen).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
        indent, src_off, sdp, sbytes).ok();
    // Store dest back
    if let Expr::OwnedRef(field_name) = dest.as_ref() {
        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
            let ap = format!("%tap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, dbase, ap, tn).ok();
        }
    }
    // Store source (empty) back
    if let Expr::OwnedRef(field_name) = source.as_ref() {
        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
            let ap = format!("%sap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
            // Allocate new empty list for source
            let ebuf = backend.emit_arena_alloc(out, indent, "16");
            let ehp = format!("%seh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, ehp, ebuf).ok();
            let ebase = format!("%seb2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, ebase, ebuf).ok();
            let edv = format!("%sed{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 {}, 16", indent, edv, ebase).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, edv, ehp).ok();
            let elp = format!("%sel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, elp, ehp).ok();
            writeln!(out, "{}store i64 0, i64* {}, align 8, !tbaa !1", indent, elp).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, ebase, ap, tn).ok();
        }
    }
    writeln!(out, "{}{} = add i64 0, {} ; transfer", indent, v, dbase).ok();
    return TypedRegister { name: v.to_string(), ty: Type::Int };
}
