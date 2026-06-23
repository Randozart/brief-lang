// GLUE — General Language Unification Engine
//
// FFI broker built on Brief's meld system. Any two languages that consume
// LLVM-compatible object code can be linked through GLUE. Neither language
// knows Brief exists. Both see their own native interface.
//
// Architecture decisions:
// - GLUE adapters are Brief `$!` macros, not Rust template engines.
//   Adapters live in glue/adapters/<language>.bv and use emit_file#() to
//   write native source files at compile time. Adding a language = writing
//   one .bv file. This avoids throwaway Rust infrastructure that will be
//   rewritten during self-hosting.
//
// - The bridge IS native object code. Brief emits LLVM IR → .o/.a/.wasm.
//   No C compiler, no extern "C", no cc crate. The foreign linker consumes
//   .a directly.
//
// - Error propagation: "Always be prepared for null." Bridge never panics.
//   Returns zero-value on failure, caller must null-check.
//
// See docs/plans/2026-06-22-glue-architecture.md for full design.
// See docs/plans/2026-06-23-glue-implementation-status.md for status.

pub mod dbvl_reader;
pub mod dbvs_validator;
// pub mod export;  // TODO: restore when export.rs is created
// pub mod link;    // TODO: restore when link.rs is created
