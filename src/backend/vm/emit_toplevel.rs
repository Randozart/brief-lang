// 2026-07-25: VM backend — top-level item bytecode emission.
// Converts TopLevel items (Defn, Txn, TypeDef, etc.) into .lair functions.

use crate::ast::*;
use crate::ast::top::*;
use super::VmBackend;

impl VmBackend {
    pub(crate) fn emit_toplevel(&mut self, item: &TopLevel) {
        match item {
            TopLevel::Definition(d) => self.emit_definition(d),
            TopLevel::Transaction(t) => self.emit_transaction(t),
            // Type declarations become no-ops (they define types, not code)
            TopLevel::TypeDef(_) => {}
            TopLevel::Constant(c) => self.emit_constant(c),
            // Other items are skipped for MVP
            _ => {}
        }
    }

    fn emit_definition(&mut self, d: &Definition) {
        // Map parameters to local slots
        self.local_slots.clear();
        self.next_local_slot = 0;
        for (name, _ty) in &d.parameters {
            self.local_slots.insert(name.clone(), self.next_local_slot);
            self.next_local_slot += 1;
        }

        // Also register the function name in fn_indices for call resolution
        self.fn_index_counter += 1;
        // The assembler tracks function indices in order of define_function calls

        self.asm.define_function(
            &d.name,
            self.next_local_slot as u16,  // local_count
            d.parameters.len() as u16,  // arg_count
        );

        // Emit body statements
        for stmt in &d.body {
            self.emit_stmt(stmt);
        }

        // If the function doesn't end with ret, add one
        // (Functions with no explicit term/return still need to return)
        self.asm.emit_ret();
    }

    fn emit_transaction(&mut self, t: &Transaction) {
        // Transactions are like definitions with contracts
        self.local_slots.clear();
        self.next_local_slot = 0;
        for (name, _ty) in &t.parameters {
            self.local_slots.insert(name.clone(), self.next_local_slot);
            self.next_local_slot += 1;
        }

        self.asm.define_function(
            &t.name,
            self.next_local_slot as u16,
            t.parameters.len() as u16,
        );

        for stmt in &t.body {
            self.emit_stmt(stmt);
        }

        self.asm.emit_ret();
    }

    fn emit_constant(&mut self, c: &Constant) {
        // Constants are compile-time values. For the VM, emit a function
        // that returns the constant's value.
        self.local_slots.clear();
        self.next_local_slot = 0;

        self.asm.define_function(&c.name, 0, 0);
        self.emit_expr(&c.expr);
        self.asm.emit_ret();
    }
}
