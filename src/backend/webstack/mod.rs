// ── Webstack backend module root ────────────────────────────────────────
// 2026-08-23: reduced to the live v2 surface. The legacy v1 TS emitter +
// JS glue generator (WebstackGenerator, ~1290 lines) had ZERO live callers —
// the shipped path is LlvmBackend(wasm32) for IR plus
// glue/web_generator.rs (GlueWebGenerator) for the JS shim. Deleted with
// plan 2026-08-23-webstack-v2-completion; see that plan for the caller map.
// To undo: git revert the deletion commit.
pub mod normalizer;
