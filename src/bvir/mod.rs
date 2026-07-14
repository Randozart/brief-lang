// ── BVIR: Brief Virtual IR ──────────────────────────────────────────────
// 2026-07-14: S-expression format for the plugin mid-end.
// Serialize/deserialize between Rust AST and .bvir text.

pub mod sexpr;
pub mod serialize;
pub mod deserialize;
pub mod layout;

pub use serialize::to_bvir;
pub use deserialize::from_bvir;
