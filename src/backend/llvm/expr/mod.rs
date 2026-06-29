// ── Expression Codegen Submodules ──────────────────────────────────
//
// 2026-06-29: Modularized from emit_expr.rs (6,145 lines) into focused
// submodules. Each submodule handles one category of expression.
//
// Migration status:
//   literal.rs   — DONE (integers, floats, bools, strings, char, term)
//   math.rs      — DONE (add, sub, mul, div, mod, neg, bitwise ops)
//   compare.rs   — TODO
//   collections.rs — TODO
//   field.rs     — TODO
//   control.rs   — TODO
//   call.rs      — TODO
//   projection.rs — TODO
//   intrinsics/  — TODO
//   misc.rs      — TODO

pub mod collections;
pub mod compare;
pub mod literal;
pub mod math;
