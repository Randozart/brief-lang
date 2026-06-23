// GLUE — General Language Unification Engine
//
// FFI broker built on Brief's meld system. Any two languages that consume
// LLVM-compatible object code can be linked through GLUE. Neither language
// knows Brief exists. Both see their own native interface.
//
// See docs/plans/2026-06-22-glue-architecture.md for full design.

pub mod dbvl_reader;
pub mod dbvs_validator;
