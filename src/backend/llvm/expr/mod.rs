// ── Expression Codegen Submodules ──────────────────────────────────
//
// 2026-06-29: Modularized from emit_expr.rs (6,145 lines → 43 lines).
// Each submodule handles one category of expression. Adding a new
// expression variant: create a new file here, add one match arm to
// expr/rest.rs.
//
// Migration status:
//   literal.rs      — DONE (integers, floats, bools, strings, char, term)
//   math.rs         — DONE (add, sub, mul, div, mod, neg, bitwise ops)
//   compare.rs      — DONE (Eq, Ne, Lt, Le, Gt, Ge, And, Or, Not)
//   collections.rs  — DONE (ListLiteral, Tuple)
//   intrinsics.rs   — DONE (200+ intrinsic variants)
//   identifier.rs   — DONE (Identifier, OwnedRef, PriorState)
//   rest.rs         — All remaining (Call, FieldAccess, Arrow, Match, etc.)

pub mod collections;
pub mod compare;
pub mod identifier;
pub mod intrinsics;
pub mod literal;
pub mod math;
pub mod rest;
