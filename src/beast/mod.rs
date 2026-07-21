// ── BEAST: Brief Virtual IR ──────────────────────────────────────────────
// 2026-07-14: S-expression format for the plugin mid-end.
// Serialize/deserialize between Rust AST and .beast text.

pub mod sexpr;
pub mod serialize;
pub mod deserialize;
pub mod layout;
pub mod pattern;

pub use serialize::to_beast;
pub use deserialize::from_beast;
