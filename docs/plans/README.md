# Briev Compiler — Plan Documents Index

**Last updated:** 2026-07-23

This index links every active and completed plan document. If this session
disappears, a new agent can pick up from here.

---

## Active Work

| Priority | Area | Document | Status |
|----------|------|----------|--------|
| 1 | Macro system: security, VFS, capabilities, `SysQuery$`, multi-target | `2026-07-23-macro-system-safety.md` | Plan |
| 2 | Macro system: implement remaining `$` intrinsics | `2026-07-22-macro-system-extensions.md` | 7 done, in testing |
| 3 | Macro system: rewrite GLUE bridge as `.bv` plugin | `2026-07-23-macro-system-safety.md` Step 15 | Planned |

## Completed

| Area | Document | Last Commit |
|------|----------|-------------|
| Phase 8: AST pretty-printer port | `2026-07-22-phase8-pp-port.md` | Completed |
| GLUE pipeline stress test | `2026-07-22-stress-test-glue-pipeline.md` | Completed |
| GLUE pipeline fixes (export, state, protocols) | `2026-07-22-complete-glue-fix-plan.md` | Completed |
| Protocol-driven type mapping | `2026-07-22-protocol-driven-glue.md` | Completed |
| Dynamic GLUE config (remove hardcoded languages) | `2026-07-22-fully-dynamic-glue-config.md` | Completed |
| Post-Phase 8 automation (arena, layout) | `2026-07-22-post-phase8-automation.md` | Completed |
| Metropolitan FFI refactoring | `2026-07-22-metropolitan-ffi-refactor.md` | Completed |
| Stdlib frgn cleanup | `2026-07-22-stdlib-frgn-cleanup.md` | Completed |
| Ship of Theseus (InopDeclaration removal) | `2026-07-22-ship-of-theseus-inop-removal.md` | Completed |

## Architecture Docs (reference)

| Document | Covers |
|----------|--------|
| `docs/architecture/macro-system.md` | Full `$` intrinsic catalog, generic design principles |
| `docs/architecture/protocol-types.md` | Protocol type system (hashwords, CastTo/CastFrom, BFS) |
| `docs/architecture/glue-as-abi-generator.md` | GLUE as ABI generator, layout-agnostic types |
| `docs/architecture/frgn-export-glue-architecture.md` | Full frgn/export/GLUE/Metropipe architecture |

---

## Key Implementation Details

### Plugin System

- `$(Stage @ priority)` blocks in `.bv` files are extracted and registered at
  compile time by `extract_inline_stage_blocks` in `src/plugin/loader.rs`
- Plugins run in priority order within each stage
- Inline plugins use the actual priority from `StageBlock.priority` (fixed in
  commit `f0729d53` — was hardcoded to 200)
- The `Parsed` stage runs BEFORE import resolution and type checking
- The `Typed` stage runs AFTER type checking — type queries available here

### Current Bugs Fixed This Session

| Bug | Fix | File |
|-----|-----|------|
| `prelude.bv` called `Before$()` on empty selection when no `txn` existed | Added `Count$() > 0` check in `when anchor.Count$() == 0` branch | `plugins/parsed/prelude.bv` |
| `prelude-hw.bv` called `Before$()` on empty selection when no imports existed | Added `Count$() > 0` check | `plugins/parsed/prelude-hw.bv` |
| `extract_inline_stage_blocks` hardcoded priority 200 instead of reading `block.priority` | Changed to `block.priority` | `src/plugin/loader.rs` |

### Current `$` Intrinsics Implemented

These 12 intrinsics are implemented in `src/macros/eval.rs`:
- AST: `Tag$`, `Named$`, `WithKey$`, `WithAttr$`, `All$`
- Traversal: `First$`, `Last$`, `Nth$`, `Children$`, `Descendants$`, `Parent$`
- Introspection: `Count$`, `Names$`, `IsEmpty$`
- Positions: `Before$`, `After$`, `Replace$`, `Inside$`, `AppendTo$`
- Actions: `Insert$`, `Delete$`, `ReplaceWith$`, `Set$`, `Rename$`
- Constructors: `Import$`, `Defn$`, `Call$`, `Block$`
- Stage: `Stage$.Insert$`, `Stage$.List$`, `Stage$.Remove$`
- Diagnostics: `EmitInfo$`, `EmitWarning$`, `EmitError$`
- **String (new):** `StrLen$`, `StrReplace$`, `StrJoin$`, `StrSplit$`, `StrSubstr$`
- **File (new):** `FileRead$`, `FileWrite$`
- **Config (new):** `ConfigGet$`
- **Universe (new):** `DocRead$`, `CastPath$`
- **Type (new):** `TypeInfo$`
- **Shell (new):** `ShellCmd$`

### Current Test Suite

```bash
cargo test --lib          # 898 lib tests pass
cargo test --test pp_roundtrip_tests -- --test-threads=1  # 8/8 pass
```

### Build

```bash
cargo build                # 0 errors, 0 warnings
./target/debug/briev-compiler build <file.bv> --llvm --out <dir>
```
