# Webstack backend — v2 completion and legacy removal

**Date:** 2026-08-23
**Status:** active
**Sequencing:** parallel branch; requires Plan 0
(`2026-08-23-backend-scaffolding-foundation.md`) merged first (capability
matrix; artifact contract). Work confined to `src/backend/webstack/`,
`src/glue/web_generator.rs` touch-ups, own tests, own doc sections.

## Charter

Webstack v2 — `LlvmBackend(wasm32)` + `GlueWebGenerator` JS shim — is the
ONLY webstack. This plan finishes its two known stubs and deletes the dead
v1 emitter so the directory stops carrying two architectures.

## Baseline state (2026-08-23)

| Aspect | State |
|--------|-------|
| Live path | compile.rs:1791 routes `.rbv` through `LlvmBackend` with `with_webstack(true)`, wasm32-wasi triple |
| Legacy | `webstack/mod.rs` ~1292 lines: TS-emitter half (`generate_ts_code`, `expr_to_ts`, `emit_ts_txn_body`, `generate_arm_rust_code` dead ARM path :1263 comment) unused by the live path but still compiled + partially tested |
| Stubs | `set_stdout_buf#(...)` no-op warning string (:658); `AddressOf#` no-op returning 0 (:683); `__web_flush_state` called with ptr=0,count=0 per LLVM emitter comment (active plan below) |
| Normalizer | 171 lines, rejects unsupported intrinsics with good errors — healthy |
| Tests | 18 in mod.rs, several pin legacy-emitter behavior |

Related active plan: `docs/plans/2026-08-10-webstack-flush-buffer.md`
(real flush batches at `term;`) — item 4.2 implements it here.

## Work items

### 4.1 Delete the legacy emitter

Remove from `webstack/mod.rs`: `collect_signals_and_transactions`,
`generate_ts_code` + helpers (`ts_type_for_signal`, `ts_ident`,
`emit_ts_txn_body`, `statement_to_ts`, `expr_to_ts`),
`generate_arm_rust_code`, and any trait impls returning Rust-codegen
strings ("JsValue::..."). Keep: normalizer wiring, capability tables,
anything `compile.rs` / `glue/web_generator.rs` references (grep all call
sites first — rule 18). Rewrite tests that pinned legacy behavior to pin
the v2 contract instead (behavioral tests, not literal tests). If a kept
item exists only to serve deleted code, delete it too.

### 4.2 Flush buffer completion

Per `2026-08-10-webstack-flush-buffer.md`: wire real state-update batches
into `__web_flush_state` at `term;` boundaries (replacing ptr=0,count=0).
The bind-routes plumbing (`resolve_bind_routes`, compile.rs:1849) already
derives writer txns — reuse its source of truth. Add e2e test: `.rbv`
fixture compiles, wasm builds via installed `wasm-ld`, shim receives
non-empty batch at term.

### 4.3 Capability honesty for no-op intrinsics

`set_stdout_buf#` and `AddressOf#` emit console.warn strings inside
generated JS today. Per Plan 0's matrix: either implement (AddressOf over
wasm linear memory is meaningful) or produce a compile-time diagnostic at
the Briev call site (file/line, why WASM can't, what to do). No silent
runtime no-ops.

### 4.4 Doc truth

backend-strategy.md webstack section: delete the preserved-historical TS
emitter dump (superseded 2026-07-26 banner stays until this lands, then the
historical body goes; the banner text is updated to "removed 2026-08-23,
see plan"). Keep the FFI marshaling + i64 boxing conventions sections —
they describe the live LLVM path.

## Documentation maintenance

- Rationale comments on deleted-code sites replaced by a single dated note:
  "v1 TS emitter removed 2026-08-23; plan 2026-08-23-webstack-v2-completion".
- `docs/architecture/features/rendered-briev-wasm.md` updated if it still
  references deleted symbols (grep before editing).

## Verification

1. `cargo test --lib` green after deletion (no orphaned tests).
2. `.rbv` e2e fixture: wasm binary produced (wasm-ld present locally),
   JS shim binds view fields, flush batches non-empty at term.
3. `git grep -n "generate_ts_code\|expr_to_ts\|arm_rust"` → zero.
4. Praetor clean on `src/backend/webstack`.
