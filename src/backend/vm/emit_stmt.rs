// 2026-07-25: VM backend — statement bytecode emission.

use crate::ast::*;
use crate::ast::top::*;
use crate::ast::Statement;
use super::VmBackend;

impl VmBackend {
    pub(crate) fn emit_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Let { name, ty: _, expr, modifiers: _, .. } => {
                // Allocate a local slot for the variable
                let slot = self.next_local_slot;
                self.next_local_slot += 1;
                self.local_slots.insert(name.clone(), slot);

                // Emit the initializer expression (if any)
                if let Some(e) = expr {
                    self.emit_expr(e);
                    self.asm.emit_store_local(slot);
                }
            }

            Statement::Assign(target, value) => {
                self.emit_expr(value);
                if let Expr::Identifier(name) = target {
                    if let Some(slot) = self.local_slots.get(name.as_str()) {
                        self.asm.emit_store_local(*slot);
                    } else {
                        self.asm.emit_trap();
                    }
                } else {
                    self.asm.emit_trap();
                }
            }

            Statement::Return(opt_expr) => {
                if let Some(e) = opt_expr {
                    self.emit_expr(e);
                }
                self.asm.emit_ret();
            }

            Statement::Term(opt_expr) => {
                if let Some(e) = opt_expr {
                    self.emit_expr(e);
                }
                self.asm.emit_ret();
            }

            Statement::TermBang(opt_expr) => {
                if let Some(e) = opt_expr {
                    self.emit_expr(e);
                }
                self.asm.emit_ret();
            }

            Statement::Expression(expr) => {
                self.emit_expr(expr);
                // Expression results are left on the stack.
                // If the expression has no side effects, we should drop.
                // For MVP: just let it accumulate (the tamer won't stack-overflow).
            }

            Statement::If(cond, then_stmts, else_stmts) => {
                self.emit_expr(cond);
                let else_label = self.fresh_label("stmt_if_else");
                let end_label = self.fresh_label("stmt_if_end");

                self.asm.emit_jz(&else_label);

                // Then branch
                for s in then_stmts {
                    self.emit_stmt(s);
                }
                self.asm.emit_jmp(&end_label);

                // Else branch
                self.asm.define_label(&else_label);
                for s in else_stmts {
                    self.emit_stmt(s);
                }

                self.asm.define_label(&end_label);
            }

            Statement::Block(stmts) => {
                // Push a new variable scope
                let saved_slots = self.next_local_slot;
                for s in stmts {
                    self.emit_stmt(s);
                }
                // Restore local slot count (pop the block's variables)
                // Note: this doesn't actually deallocate — the frame slots
                // persist for the function's lifetime. This is a simplification
                // for the MVP. The full tamer will use push_frame/pop_frame.
                self.next_local_slot = saved_slots;
            }

            Statement::Match { expr: scrutinee, arms } => {
                self.emit_expr(&**scrutinee);
                let end_label = self.fresh_label("stmt_match_end");
                for arm in arms {
                    let next_label = self.fresh_label("stmt_match_next");
                    match &arm.pattern {
                        StmtMatchPattern::Wildcard => {}
                        StmtMatchPattern::Literal(pat_val) => {
                            self.asm.emit_dup();
                            let v = *pat_val;
                            if v >= i16::MIN as i128 && v <= i16::MAX as i128 {
                                self.asm.emit_push_i16(v as i16);
                            } else {
                                self.asm.emit_push_i64(v as i64);
                            }
                            self.asm.emit_eq();
                            self.asm.emit_jz(&next_label);
                        }
                        _ => {}
                    }
                    for s in &arm.body {
                        self.emit_stmt(s);
                    }
                    self.asm.emit_jmp(&end_label);
                    self.asm.define_label(&next_label);
                }
                self.asm.emit_drop();
                self.asm.define_label(&end_label);
            }

            Statement::Guarded(cond, body) => {
                self.emit_expr(cond);
                let end_label = self.fresh_label("guard_end");
                self.asm.emit_jz(&end_label);
                for s in body {
                    self.emit_stmt(s);
                }
                self.asm.define_label(&end_label);
            }

            _ => {
                // Unsupported statement: trap
                self.asm.emit_trap();
            }
        }
    }
}
