// 2026-07-25: VM backend — expression bytecode emission.
// Each expression pushes its result onto the operand stack.

use crate::ast::*;
use crate::ast::Expr;
use super::assembler::Assembler;
use super::VmBackend;

impl VmBackend {
    /// Emit VM bytecode for an expression. Result is left on the stack.
    pub(crate) fn emit_expr(&mut self, expr: &Expr) {
        let expr = expr.clone();
        self.emit_expr_inner(&expr);
    }

    fn emit_expr_inner(&mut self, expr: &Expr) {
        match expr {
            // ── Literals ───────────────────────────────────────────────
            Expr::Decimal(n) => {
                let v = *n;
                if v >= i16::MIN as i64 && v <= i16::MAX as i64 {
                    self.asm.emit_push_i16(v as i16);
                } else if v >= i32::MIN as i64 && v <= i32::MAX as i64 {
                    self.asm.emit_push_i32(v as i32);
                } else {
                    self.asm.emit_push_i64(v);
                }
            }

            Expr::Bool(b) => {
                self.asm.emit_push_i8(if *b { 1 } else { 0 });
            }

            Expr::Float(_) => {
                // Float literals: delegate to host FFI for now.
                // For MVP, push a placeholder and emit a trap.
                // In the full tamer, the host provides float operations.
                self.asm.emit_push_i64(0);
                self.asm.emit_trap();
            }

            // ── Identifiers (variable access) ──────────────────────────
            Expr::Identifier(name) => {
                match self.local_slots.get(name.as_str()) {
                    Some(slot) => {
                        self.asm.emit_load_local(*slot);
                    }
                    None => {
                        // Not a local — could be a global or function name.
                        // For MVP, treat as zero and emit trap.
                        self.asm.emit_push_i64(0);
                        self.asm.emit_trap();
                    }
                }
            }

            // ── Binary operations ──────────────────────────────────────
            Expr::BinaryOp(kind, lhs, rhs) => {
                // 2026-07-25: Ptr<Int> + Int — scale Int by 8 before adding.
                // In Brief, Ptr arithmetic is scaled by element size.
                // The VM's add is unscaled, so we multiply the offset by 8.
                if matches!(kind, BinaryOpKind::Add) {
                    if let Expr::Identifier(name) = lhs.as_ref() {
                        if let Some(&slot) = self.local_slots.get(name.as_str()) {
                            if self.ptr_slots.contains(&slot) {
                                // Ptr<Int> + Int: emit ptr, emit offset, mul 8, add
                                self.emit_expr(lhs);
                                self.emit_expr(rhs);
                                self.asm.emit_push_i8(3); // shift left 3 = multiply by 8
                                self.asm.emit_shl();
                                self.asm.emit_add();
                                return;
                            }
                        }
                    }
                }
                self.emit_expr(lhs);
                self.emit_expr(rhs);
                match kind {
                    BinaryOpKind::Add    => self.asm.emit_add(),
                    BinaryOpKind::Sub    => self.asm.emit_sub(),
                    BinaryOpKind::Mul    => self.asm.emit_mul(),
                    BinaryOpKind::Div    => self.asm.emit_div_s(),
                    BinaryOpKind::Mod    => self.asm.emit_rem_s(),
                    BinaryOpKind::Eq     => self.asm.emit_eq(),
                    BinaryOpKind::Neq    => { self.asm.emit_eq(); self.asm.emit_push_i8(1); self.asm.emit_xor(); },
                    BinaryOpKind::Lt     => self.asm.emit_lt_s(),
                    BinaryOpKind::Gt     => self.asm.emit_gt_s(),
                    BinaryOpKind::Le     => self.asm.emit_le_s(),
                    BinaryOpKind::Ge     => self.asm.emit_ge_s(),
                    BinaryOpKind::And | BinaryOpKind::BitAnd => self.asm.emit_and(),
                    BinaryOpKind::Or  | BinaryOpKind::BitOr  => self.asm.emit_or(),
                    BinaryOpKind::BitXor => self.asm.emit_xor(),
                    BinaryOpKind::Shl    => self.asm.emit_shl(),
                    BinaryOpKind::Shr    => self.asm.emit_shr_s(),
                    BinaryOpKind::Concat => self.asm.emit_trap(),
                }
            }

            Expr::UnaryOp(kind, inner) => {
                self.emit_expr(inner);
                match kind {
                    UnaryOpKind::Neg => {
                        self.asm.emit_push_i64(0);
                        self.asm.emit_swap();
                        self.asm.emit_sub();
                    }
                    UnaryOpKind::Not => {
                        // 2026-07-26: Logical NOT — emit NOT which is now
                        // a logical NOT (0→1, else→0) in both C and Brief VM.
                        self.asm.emit_not();
                    }
                    UnaryOpKind::BitNot => {
                        // 2026-07-26: Bitwise NOT — emit BNOT for ~a.
                        self.asm.emit_bnot();
                    }
                }
            }

            // ── Control flow expressions ───────────────────────────────
            Expr::If(cond, then_branch, else_branch) => {
                self.emit_expr(cond);
                let else_label = self.fresh_label("if_else");
                let end_label = self.fresh_label("if_end");

                self.asm.emit_jz(&else_label);
                self.emit_expr(then_branch);
                self.asm.emit_jmp(&end_label);

                self.asm.define_label(&else_label);
                if let Some(els) = else_branch {
                    self.emit_expr(els);
                } else {
                    self.asm.emit_push_i64(0);
                }

                self.asm.define_label(&end_label);
            }

            Expr::Match(scrutinee, arms) => {
                self.emit_expr(scrutinee);
                let end_label = self.fresh_label("match_end");
                for arm in arms {
                    let next_label = self.fresh_label("match_next");
                    // Emit pattern matching
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            // Always matches — emit body
                        }
                        Pattern::Literal(lit_expr) => {
                            // Extract the literal value from the Expr wrapper
                            if let Expr::Decimal(val) = lit_expr {
                                self.asm.emit_dup();
                                if *val >= i16::MIN as i64 && *val <= i16::MAX as i64 {
                                    self.asm.emit_push_i16(*val as i16);
                                } else if *val >= i32::MIN as i64 && *val <= i32::MAX as i64 {
                                    self.asm.emit_push_i32(*val as i32);
                                } else {
                                    self.asm.emit_push_i64(*val);
                                }
                                self.asm.emit_eq();
                                self.asm.emit_jz(&next_label);
                            } else {
                                // Non-integer literal pattern — fall through for MVP
                            }
                        }
                        _ => {
                            // Complex patterns: fall through to body for MVP
                        }
                    }
                    self.emit_expr(&arm.body);
                    self.asm.emit_jmp(&end_label);
                    self.asm.define_label(&next_label);
                }
                // Drop scrutinee (no arm matched — should not happen with wildcard)
                self.asm.emit_drop();
                self.asm.emit_push_i64(0);
                self.asm.define_label(&end_label);
            }

            // ── Function calls ─────────────────────────────────────────
            Expr::Call(name, args, _analysis_id) => {
                // Check if this is a host function (frgn call)
                // 2026-07-25: Check intrinsic (#) BEFORE host_fn_ids, so that
                // Alloc#/SysCall# etc. always take the PascalCase-skip path
                // even after being registered in host_fn_ids by a prior call.
                if name.ends_with('#') {
                    // 2026-07-25: Intrinsic calls (Alloc#, SysCall#, ShellCmd#, etc.)
                    // are handled as host calls.
                    if !self.host_fn_ids.contains_key(name.as_str()) {
                        let id = self.host_fn_ids.len() as u32;
                        self.asm.register_host_fn(name, id);
                        self.host_fn_ids.insert(name.clone(), id);
                    }
                    let host_id = self.host_fn_ids[name.as_str()];
                    // Emit only Int arguments (skip PascalCase strategy identifiers
                    // like Malloc that the VM backend can't resolve).
                    for arg in args.iter().rev() {
                        if matches!(arg, Expr::Identifier(_)) {
                            // 2026-07-25: Skip PascalCase identifiers (strategy tags)
                            continue;
                        }
                        self.emit_expr(arg);
                    }
                    self.asm.emit_hcall(host_id);
                } else if let Some(&host_id) = self.host_fn_ids.get(name.as_str()) {
                    // Emit arguments right-to-left
                    for arg in args.iter().rev() {
                        self.emit_expr(arg);
                    }
                    self.asm.emit_hcall(host_id);
                } else {
                    // Regular function call — emit args, then call
                    for arg in args.iter().rev() {
                        self.emit_expr(arg);
                    }
                    // Look up function index by name
                    match self.fn_indices.get(name.as_str()) {
                        Some(idx) => self.asm.emit_call(*idx),
                        None => {
                            // Unknown function — trap
                            self.asm.emit_trap();
                        }
                    }
                }
            }

            // ── Block expression ───────────────────────────────────────
            Expr::Block(stmts) => {
                for stmt in stmts {
                    self.emit_stmt(stmt);
                }
            }

            // ── Array index ───────────────────────────────────────────
            Expr::Index(obj, idx) => {
                // data[sl] → push base, push idx, mul 8, add, LOAD
                self.emit_expr(obj);
                self.emit_expr(idx);
                self.asm.emit_push_i8(3);  // scale by 8 (Int size)
                self.asm.emit_shl();
                self.asm.emit_add();
                self.asm.emit_load();
            }

            // ── Field access ───────────────────────────────────────────
            Expr::Field(obj, field_name) => {
                // 2026-07-30: Determine the struct type name to compute
                // the correct field byte offset. If the object is an
                // identifier, look it up in local_types.
                let struct_name = match obj.as_ref() {
                    Expr::Identifier(name) => self.local_types.get(name).map(|s| s.as_str()),
                    _ => None,
                };
                let offset = self.field_offset(struct_name, field_name) as i64;
                self.emit_expr(obj);
                self.asm.emit_push_i64(offset);
                self.asm.emit_add();
            }

            // ── Cast ───────────────────────────────────────────────────
            Expr::Cast(inner, _target_type) => {
                // For MVP: no-op (cast is a type system annotation).
                // The VM is untyped, so casts are identity.
                self.emit_expr(inner);
            }

            // ── Pointer dereference ──────────────────────────────────
            Expr::Deref(inner) => {
                // Emit the pointer expression, then LOAD from that address.
                // *(bc + index) → emit ptr, load
                self.emit_expr(inner);
                self.asm.emit_load();
            }

            // ── Address-of ────────────────────────────────────────────
            Expr::AddrOf(inner) => {
                // For MVP: no-op. AddrOf in the VM is just the value itself
                // (the VM is untyped, all values are Int handles).
                self.emit_expr(inner);
            }

            // ── Remaining expressions ───────────────────────────────────
            other => {
                // For MVP: unsupported expressions push 0 and trap.
                self.asm.emit_push_i64(0);
                self.asm.emit_trap();
            }
        }
    }

    pub(crate) fn fresh_label(&mut self, prefix: &str) -> String {
        let id = self.label_counter;
        self.label_counter += 1;
        format!("{}_{}", prefix, id)
    }
}
