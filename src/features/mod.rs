// ── Feature Modules (Active Backends) ─────────────────────────────────
// 2026-07-13: Stripped to only modules needed by active backends.
// Only binary_op, unary_op, literal, traits are kept.
// All other feature modules were part of the old Pattern B architecture
// and their types are now in ast or handled by the main passes.

pub mod traits;
pub mod literal;
pub mod binary_op;
pub mod unary_op;
