// ── TopLevel Feature Modules ────────────────────────────────────────
//
// Each module handles one TopLevel variant — typechecking, evaluation,
// and per-backend codegen co-located in a single file.
//
// Phase 1.5: TypeDef is the first TopLevel feature. More follow in Phase 3.

pub mod typedef;
