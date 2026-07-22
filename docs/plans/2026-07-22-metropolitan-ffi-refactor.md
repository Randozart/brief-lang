# Metropolitan FFI Refactoring — GLUE + Metropipe

**Date:** 2026-07-22
**Status:** Implementation

---

## Architecture

The old "Metropolitan FFI" umbrella encompassed two mechanisms.
GLUE now replaces the bridge generation half. The shared memory IPC
half survives as Metropipe.

```
Metropolitan FFI (umbrella)
├── GLUE (compile-time bridge generation)
│   ├── lib/glue.toml, src/glue/
│   └── src/analysis/frgn_dispatch.rs
│
└── Metropipe (runtime shared memory IPC)
    ├── src/ffi/metropipe.rs (was metropolitan.rs)
    ├── src/ffi/metropipe_cli.rs (was metro_cli.rs)
    └── lib/std/metro_bridge.bv
```

## Files to Archive (superceded by GLUE)

Move to `src/ffi/archive/` — preserved for reference, not compiled:

| File | Lines | Replaced by |
|------|-------|-------------|
| `orchestrator.rs` | 187 | `resolve_single_frgn` in GLUE |
| `mapper.rs` | 138 | Protocol transforms in `emit_protocol_chain` |
| `mappers.rs` | 328 | TOML `protocols` + `c_type_map` |
| `native_mapper.rs` | 178 | Protocol CastTo/CastFrom |
| `loader.rs` | 451 | `lib/glue.toml` loading via `src/glue/config.rs` |
| `resolver.rs` | 74 | TOML path resolution |
| `script.rs` | 269 | `emit_expr::Call` in backend |
| `sentinel.rs` | 155 | Contract checker + fallback |
| `types.rs` | 128 | `ast::Type` + protocol entries |
| `dynamic.rs` | 260 | `libloading` in integration tests |
| `registry/` | 541 | TOML protocol mapping + `src/glue/config.rs` |
| `validator.rs` | 86 | Type checker + protocol coherence |

Total: ~2795 lines of dead code removed from active compilation.

## Files to Keep (Metropipe runtime)

| Old Name | New Name | Lines |
|----------|----------|-------|
| `metropolitan.rs` | `metropipe.rs` | 876 |
| `metro_cli.rs` | `metropipe_cli.rs` | 661 |

Both kept. `error.rs` kept for backward compat (used by dead backend c.rs).

## Execution

1. Create `src/ffi/archive/` directory
2. Move 12 files + registry/ directory to archive
3. Rename `metropolitan.rs` → `metropipe.rs`
4. Rename `metro_cli.rs` → `metropipe_cli.rs`
5. Update `src/ffi/mod.rs` module declarations
6. Update `lib/std/metro_bridge.bv` to use new frgn style
7. Update `src/wrapper/generator.rs` reference to renamed module
8. Update `docs/architecture/frgn-export-glue-architecture.md`
9. Update `docs/architecture/glue-as-abi-generator.md`
10. Update README FFI section
11. Verify `cargo test --lib` + `cargo test --test pp_roundtrip_tests`
