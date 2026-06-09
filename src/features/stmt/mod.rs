// ── Statement Feature Modules ──────────────────────────────────────
//
// Phase 2: Each file contains one Statement variant's struct definition,
// typechecking, evaluation, and per-backend codegen — all co-located.
//
// The old Statement enum variants remain active during the dual-path
// transition. These feature files provide Pattern B struct + trait impls
// that will replace the old dispatch in Phase 4.

pub mod assignment;
pub mod let_binding;
pub mod guarded;
pub mod term;
pub mod escape;
pub mod expression;
pub mod unification;
pub mod inline_asm;
pub mod local_trigger;
pub mod alka;
pub mod on_exit;
pub mod sync_block;
