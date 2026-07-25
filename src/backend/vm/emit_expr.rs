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
                self.emit_expr(lhs);
                self.emit_expr(rhs);
                match kind {
                    BinaryOpKind::Add    => self.asm.emit_add(),
                    BinaryOpKind::Sub    => self.asm.emit_sub(),
                    BinaryOpKind::Mul    => self.asm.emit_mul(),
                    BinaryOpKind::Div    => self.asm.emit_div_s(),
                    BinaryOpKind::Mod    => self.asm.emit_rem_s(),
                    BinaryOpKind::Eq     => self.asm.emit_eq(),
                    BinaryOpKind::Neq    => self.asm.emit_ne(),
                    BinaryOpKind::Lt     => self.asm.emit_lt_s(),
                    BinaryOpKind::Gt     => self.asm.emit_gt_s(),
                    BinaryOpKind::Le     => self.asm.emit_le_s(),
                    BinaryOpKind::Ge     => self.asm.emit_ge_s(),
                    BinaryOpKind::And    | BinaryOpKind::BitAnd => self.asm.emit_and(),
                    BinaryOpKind::Or     | BinaryOpKind::BitOr  => self.asm.emit_or(),
                    BinaryOpKind::BitXor => self.asm.emit_xor(),
                    BinaryOpKind::Shl    => self.asm.emit_shl(),
                    BinaryOpKind::Shr    => self.asm.emit_shr_s(),
                    BinaryOpKind::Concat => {
                        // String concat: for MVP, trap.
                        // In the full tamer, this calls host_strcat.
                        self.asm.emit_trap();
                    }
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
                    UnaryOpKind::Not | UnaryOpKind::BitNot => {
                        self.asm.emit_not();
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
                if let Some(&host_id) = self.host_fn_ids.get(name.as_str()) {
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

            // ── Field access ───────────────────────────────────────────
            Expr::Field(obj, field_name) => {
                // For MVP: trap. Full struct support is Phase 6+.
                self.emit_expr(obj);
                self.asm.emit_trap();
            }

            // ── Cast ───────────────────────────────────────────────────
            Expr::Cast(inner, _target_type) => {
                // For MVP: no-op (cast is a type system annotation).
                // The VM is untyped, so casts are identity.
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
