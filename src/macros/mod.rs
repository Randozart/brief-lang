// ── AST Navigation Macros ───────────────────────────────────────────────
// 2026-07-21: Selection engine and navigation DSL for compile-time AST
// manipulation inside $(Stage) blocks.

pub mod actions;
pub mod compile_time;
pub mod eval;
pub mod pattern_live;
pub mod selection;
pub mod stage_target;
pub mod text_ops;

pub use pattern_live::*;
pub use selection::*;
