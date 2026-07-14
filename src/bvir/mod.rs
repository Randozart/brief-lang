// ── BVIR: Brief Virtual IR ──────────────────────────────────────────────
// 2026-07-14: S-expression format for the plugin mid-end.
// Serialize/deserialize between Rust AST and .bvir text.
// Modules:
//   sexpr.rs — S-expression tokenizer, parser, pretty-printer
//   serialize.rs — walk AST + TypeUniverse → .bvir text
//   deserialize.rs — .bvir text → AST + TypeUniverse

pub mod sexpr;
