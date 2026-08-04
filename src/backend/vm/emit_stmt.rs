// 2026-07-25: VM backend — statement bytecode emission.

use crate::ast::*;
use crate::ast::top::*;
use crate::ast::Statement;
use super::VmBackend;

impl VmBackend {
    pub(crate) fn emit_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Let { name, names, ty, expr, modifiers: _, .. } => {
                // 2026-07-30: Handle tuple destructuring: let (a, b, c) = expr;
                if names.is_empty() {
                    // Single variable
                    // 2026-07-30: Allocate slots for struct types (let stack: VMStack;
                    // allocates enough slots for all struct fields + len).
                    let field_count = if let Some(Type::Custom(sname)) = ty {
                        self.struct_fields.get(sname.as_str())
                            .map(|f| f.len() as u8)
                            .unwrap_or(1)
                    } else { 1 };
                    let slot = self.next_local_slot;
                    self.next_local_slot += field_count;
                    self.local_slots.insert(name.clone(), slot);
                    // 2026-07-30: Track struct type name for field offset resolution.
                    if let Some(Type::Custom(sname)) = ty {
                        self.local_types.insert(name.clone(), sname.clone());
                    }
                    if let Some(e) = expr {
                        self.emit_expr(e);
                        self.asm.emit_store_local(slot);
                    }
                } else {
                    // Multi-value destructuring: values already on stack (pushed
                    // by preceding function call). Stack order: ..., v2, v1, v0
                    // where v0 is the first return value (top of stack).
                    // names are in order [v0, v1, v2].
                    let mut dest_names = names.clone();
                    if let Some(e) = expr {
                        // If there's an init expr, it pushes all values
                        self.emit_expr(e);
                    }
                    // Allocate slots for all names first
                    for n in &dest_names {
                        let slot = self.next_local_slot;
                        self.next_local_slot += 1;
                        self.local_slots.insert(n.clone(), slot);
                    }
                    // Pop values from stack into locals (reverse: v0 on top goes to names[0])
                    for n in dest_names.iter() {
                        if let Some(slot) = self.local_slots.get(n.as_str()) {
                            self.asm.emit_store_local(*slot);
                        }
                    }
                }
            }

            Statement::Assign(target, value) => {
                self.emit_expr(value);
                // 2026-07-30: Support Field and Index targets for struct field
                // and array element assignment: stack.len = val; stack.data[sl] = val;
                match target {
                    Expr::Identifier(name) => {
                        if let Some(slot) = self.local_slots.get(name.as_str()) {
                            self.asm.emit_store_local(*slot);
                        } else {
                            self.asm.emit_trap();
                        }
                    }
                    Expr::Field(obj, field_name) => {
                        // stack.len = val → emit val, emit base_ptr, add offset, STORE
                        let sname = match obj.as_ref() {
                            Expr::Identifier(n) => self.local_types.get(n).map(|s| s.as_str()),
                            _ => None,
                        };
                        let offset = self.field_offset(sname, field_name) as i64;
                        self.emit_expr(obj);  // push base ptr
                        self.asm.emit_push_i64(offset);
                        self.asm.emit_add();
                        self.asm.emit_swap(); // val is on top, addr under
                        self.asm.emit_store();
                    }
                    Expr::Index(obj, idx) => {
                        // data[sl] = val → emit val, emit base, emit idx, mul 8, add, STORE
                        self.emit_expr(obj);  // push base ptr
                        self.emit_expr(idx);  // push index
                        self.asm.emit_push_i8(3);  // scale by 8
                        self.asm.emit_shl();
                        self.asm.emit_add();
                        self.asm.emit_swap(); // val is on top, addr under
                        self.asm.emit_store();
                    }
                    _ => {
                        self.asm.emit_trap();
                    }
                }
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
                        // 2026-07-30: Multi-pattern: duplicate scrutinee for each
                        StmtMatchPattern::Multi(patterns) => {
                            for pat in patterns {
                                match pat {
                                    StmtMatchPattern::Literal(pat_val) => {
                                        self.asm.emit_dup();
                                        let v = *pat_val;
                                        if v >= i16::MIN as i128 && v <= i16::MAX as i128 {
                                            self.asm.emit_push_i16(v as i16);
                                        } else {
                                            self.asm.emit_push_i64(v as i64);
                                        }
                                        self.asm.emit_eq();
                                        let body_label = self.fresh_label("multi_match_body");
                                        self.asm.emit_jnz(&body_label);
                                        self.asm.emit_jmp(&next_label);
                                        self.asm.define_label(&body_label);
                                    }
                                    StmtMatchPattern::Wildcard => {}
                                    _ => {}
                                }
                            }
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
