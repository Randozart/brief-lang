// ── Function State Management ──────────────────────────────────────────
// 2026-07-12: Phase 4 — Per-function SSA register state, phi field registries,
// back-edge tracking. Split from loop_engine.rs.

use std::collections::HashMap;

/// Per-function SSA register state and phi tracking.
/// Tracked separately from CompilerContext (global/read-only).
#[derive(Debug, Clone)]
pub struct FunctionState {
    /// SSA register counter (for generating unique %tN names).
    pub txn_counter: u64,
    /// Map from state field name -> SSA register name for forward-edge phis.
    pub phi_field_regs: HashMap<String, String>,
    /// Map from state field name -> SSA register name for back-edge phis.
    pub backedge_field_regs: HashMap<String, String>,
    /// Track pending back-edge phi patches.
    pub pending_phi_backedge: Vec<PendingPhi>,
    /// Track pending native back-edge phi patches.
    pub pending_phi_native_backedge: Vec<PendingPhi>,
}

/// A pending phi node that needs to be patched after the back-edge is known.
#[derive(Debug, Clone)]
pub struct PendingPhi {
    pub field: String,
    pub phi_reg: String,
    pub incoming_reg: String,
}

impl FunctionState {
    pub fn new() -> Self {
        FunctionState {
            txn_counter: 0,
            phi_field_regs: HashMap::new(),
            backedge_field_regs: HashMap::new(),
            pending_phi_backedge: Vec::new(),
            pending_phi_native_backedge: Vec::new(),
        }
    }

    /// Generate a unique SSA register name: %tN
    pub fn gen_reg(&mut self) -> String {
        let n = self.txn_counter;
        self.txn_counter += 1;
        format!("%t{}", n)
    }

    /// Register a forward-edge phi for a field.
    pub fn register_phi_field(&mut self, field: &str, reg: &str) {
        self.phi_field_regs.insert(field.to_string(), reg.to_string());
    }

    /// Register a back-edge phi for a field.
    pub fn register_backedge_field(&mut self, field: &str, reg: &str) {
        self.backedge_field_regs.insert(field.to_string(), reg.to_string());
    }

    /// Queue a back-edge phi patch.
    pub fn queue_pending_phi(&mut self, field: &str, phi_reg: &str, incoming: &str) {
        self.pending_phi_backedge.push(PendingPhi {
            field: field.to_string(),
            phi_reg: phi_reg.to_string(),
            incoming_reg: incoming.to_string(),
        });
    }
}
