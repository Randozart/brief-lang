# Phase 7 Remaining Items — AddressOf# Backend, Registry, CLI, Docs

**Date:** 2026-07-15
**Status:** Active — implementation in progress
**Branch:** `main`

## Table of Contents

1. [Summary](#1-summary)
2. [Implementation Order](#2-implementation-order)
3. [Item 1: Shared Address Resolver](#3-item-1-shared-address-resolver)
4. [Item 2: AddressOf# LLVM Backend](#4-item-2-addressof-llvm-backend)
5. [Item 3: AddressOf# Stubs (CIRCT, Webstack)](#5-item-3-addressof-stubs-circt-webstack)
6. [Item 4: config/module-registry.toml](#6-item-4-configmodule-registrytoml)
7. [Item 5: --warn-unresolved-trg / --error-unresolved-trg](#7-item-5---warn-unresolved-trg----error-unresolved-trg)
8. [Item 6: target_triple / data_layout → TargetConfig](#8-item-6-target_triple--data_layout--targetconfig)
9. [Item 7: plugins/post/validate-trg.bv](#9-item-7-pluginspostvalidate-trgbv)
10. [Item 8: plugins/{mid,post,back}/README.md](#10-item-8-pluginsmidpostbackreadmemd)
11. [Item 9: register subcommand](#11-item-9-register-subcommand)
12. [Item 10: Tests](#12-item-10-tests)
13. [Verification Gates](#13-verification-gates)

---

## 1. Summary

Complete all remaining items from the compile-time metaprogramming plan
(Phases 5-7) that were not covered in the main implementation waves.

**Not included:** Normalizer auto-annotation removal (wrong-headed per
review — normalizer does infrastructure work, not user transformations).

---

## 2. Implementation Order

Items are ordered by criticality (backends first, then plumbing, then docs):

| Order | Item | Files | Why this order |
|-------|------|-------|----------------|
| 1 | Shared address resolver | `src/address_resolver.rs` (new) | Prerequisite for backend |
| 2 | AddressOf# LLVM backend | `src/backend/llvm/intrinsics.rs` | Most critical — primary backend |
| 3 | AddressOf# stubs (CIRCT, Webstack) | `src/backend/circt.rs`, `webstack.rs` | Dead backends — stubs only |
| 4 | config/module-registry.toml | `config/module-registry.toml` (new) | Required for registry imports |
| 5 | --warn/--error unresolved trg | `src/main.rs`, `src/compile.rs` | Plumbs dynamic trigger safety |
| 6 | target_triple/data_layout → TargetConfig | `context.rs`, `target.rs`, callers | Medium-risk refactor |
| 7 | plugins/post/validate-trg.bv | `plugins/post/validate-trg.bv` (new) | Post-stage guard |
| 8 | plugins/{mid,post,back}/README.md | 3 new files | Documentation |
| 9 | register subcommand | `src/main.rs` | CLI feature |
| 10 | Tests | Various | Verification |

---

## 3. Item 1: Shared Address Resolver

**Goal:** Extract `resolve_address_for_interp()` from `interpreter/intrinsics.rs`
into a shared module usable by both interpreter and LLVM backend.

**Design:**
- New file: `src/address_resolver.rs`
- `pub fn resolve_address(id: &str) -> u64`
  - Loads `config/address-map.toml` at runtime using `toml`
  - Falls back to hardcoded table (same as current interpreter)
  - Unknown names → `0xFE000000` (default MMIO base)
- Wire into `src/lib.rs` as `pub mod address_resolver`
- Update interpreter: replace `resolve_address_for_interp()` body with call
  to shared function
- Remove the private function from the interpreter

**Tests:** Existing `test_address_of_known` / `test_address_of_unknown_defaults`
must still pass.

---

## 4. Item 2: AddressOf# LLVM Backend

**Goal:** Emit correct LLVM IR for `AddressOf#(id)` calls.

**Design:**
- New arm in `emit_intrinsic_call()` in `src/backend/llvm/intrinsics.rs`
- `"AddressOf#" => return emit_address_of(backend, out, v, args, indent)`
- `emit_address_of()`:
  1. Extract string literal arg (must be `Expr::Quoted` at compile time)
  2. Call `resolve_address()` on the string
  3. Emit `%v = inttoptr i64 <addr> to ptr`
  4. Return `TypedRegister { name: v, ty: Type::ptr(Type::bits(8)) }`

**Why `inttoptr`:** Matches existing pattern in `emit_trg_load` for
`LinkRef::Explicit(addr)` (see `emit_toplevel.rs:498`). The address is
a compile-time constant resolved from the address map.

**Error handling:** If arg is not a string literal, return a compile error
(not a runtime abort).

**Tests:** `test_emit_address_of` — compile a program with `AddressOf#("uart")`,
verify `inttoptr i64 4294975488` appears in the output (0xFFE01000 = 4294975488).

---

## 5. Item 3: AddressOf# Stubs (CIRCT, Webstack)

**Design:**
- CIRCT (`src/backend/circt.rs`): Where intrinsics are dispatched, add arm
  `"AddressOf#" => { /* stub: not yet implemented */ ... }` that emits a
  zero value of the expected type.
- Webstack (`src/backend/webstack.rs`): Same pattern.
- Comment: `// 2026-07-15: Phase 5 — stub only`
- Per AGENTS.md dead backend rules — no full implementation.

---

## 6. Item 4: config/module-registry.toml

**Goal:** Create the registry config file so `InsertRegistryImport$` has a
lookup table (currently falls back to literal resolution).

**Design:**
```toml
# config/module-registry.toml
# 2026-07-15: Registry of known module paths for <> imports.
# InsertRegistryImport$("std/prelude.bv") looks up here.

[registry]
"std/prelude" = "lib/std/prelude.bv"
"std/types" = "lib/std/types.bv"
"std/option" = "lib/std/option.bv"
"std/char" = "lib/std/char.bv"
"std/math" = "lib/std/math.bv"
"std/rt" = "lib/std/rt.bv"
```

**Note:** Import resolution code path already has `ImportKind::Registry(path)`
falling through to literal. This file documents what entries should eventually
be there. No Rust code change needed for this item alone.

---

## 7. Item 5: --warn-unresolved-trg / --error-unresolved-trg

**Goal:** CLI flags controlling runtime behavior when a `@ *ptr` dynamic
trigger's target is null at init time.

**Design:**
- Add `ResolveStrategy` enum (or two bools) to `BuildOptions`
- `--warn-unresolved-trg` — warn on null target, continue
- `--error-unresolved-trg` — error on null target, abort
- Default: warn (safe)
- Thread strategy into `emit_trg_load` for `LinkRef::Deref`
- Currently `emit_trg_load` emits `load volatile` from the pointer value
  — no null check. With these flags, wrap the load in a null-check + branch:
  - `icmp eq ptr %ptr, null`
  - `br i1 %cmp, label %trg_null_<n>, label %trg_ok_<n>`
  - `trg_null_<n>`: call `__briv_warn("unresolved trigger")` or abort
  - `trg_ok_<n>`: normal `load volatile`

**Simplification for this pass:** Add the flags to `BuildOptions`, thread
them through, but only emit the null-check + branch for the `error` case
(emit `unreachable`). The `warn` case (default) adds complexity for marginal
benefit — we can land the simpler error-only version.

**Tests:** `test_trg_deref_warn_flag`, `test_trg_deref_error_flag` — verify
IR contains null check + branch for error mode.

---

## 8. Item 6: target_triple / data_layout → TargetConfig

**Goal:** Remove `target_triple` and `data_layout` fields from
`LlvmBackendContext` (in `context.rs`) and make them config-driven via
`TargetConfig`.

**Design:**
- `TargetConfig` already exists in `src/target.rs`. Add optional fields:
  ```rust
  pub target_triple: Option<String>,
  pub data_layout: Option<String>,
  ```
- Add `with_target_triple()` / `with_data_layout()` builder methods
- Load from `config/targets.toml` if present (extend the Entry struct)
- In `context.rs`, remove the hardcoded fields and default values
- Callers that currently set `target_triple` / `data_layout` on context
  must instead provide them via TargetConfig

**Affected call sites:**
- `context.rs` — field declarations, defaults (`x86_64-unknown-linux-gnu` / data layout string)
- `mod.rs` — construction of `LlvmBackendContext`, passing of triple/layout
- `compile.rs` — where backend is constructed, pass target from config
- `target.rs` — Entry struct, config loading

**Default behavior preservation:** If no target config entry specifies
triple/layout, fall back to `x86_64-unknown-linux-gnu` and the corresponding
data layout (same as today). This is a no-op refactoring for the default
case.

**Tests:** Existing `test_compiler_context_default_triple` and
`test_compiler_context_wasm_*` tests must be updated to reflect the new
config-driven approach.

---

## 9. Item 7: plugins/post/validate-trg.bv

**Goal:** A `$(Post)` stage plugin that warns on unresolved dynamic triggers.

**Design:**
```briv
$(Post @ 0) {
    // 2026-07-15: Post-stage guard for dynamic triggers.
    // Scans the generated IR for unresolved @ *ptr references
    // and warns about them.
    EmitWarning$("validate-trg: post-stage guard active");
    // In Phase 5+ the actual scan logic goes here
    // using Collect$("(load volatile ...)") to find deref loads
};
```

For now, a minimal plugin that demonstrates the pattern. The actual scan
logic (walking `LinkRef::Deref` instances) requires mid-stage AST access
and is a future enhancement.

---

## 10. Item 8: plugins/{mid,post,back}/README.md

**Goal:** Briv documentation for each plugin stage directory explaining
what kind of plugins belong there.

Content pattern:
```markdown
# Mid-Stage Plugins

Plugins in this directory run at the `$(Mid)` stage, after type-checking
and before code generation. ...

## Current plugins

(none)

## Writing a Mid plugin

See `docs/architecture/features/plugins.md`.
```

Similar for `post/` and `back/`.

---

## 11. Item 9: register subcommand

**Goal:** Add `briv-compiler register <name>` CLI subcommand for
registering project/target schemas.

**Design:**
- New subcommand in `main.rs` dispatch: `"register" => run_register(&args[2..])`
- `run_register()` — currently just prints a message about the feature being
  a stub (the full implementation is a separate effort)
- This unblocks the plan item without over-engineering

---

## 12. Item 10: Tests

| Test | File | What it asserts |
|------|------|-----------------|
| `test_resolve_address_known` | `address_resolver.rs` | Known name returns correct address |
| `test_resolve_address_unknown` | `address_resolver.rs` | Unknown name returns default |
| `test_emit_address_of` | `intrinsics.rs` (LLVM) | `inttoptr` with correct addr in IR |
| `test_trg_deref_error_flag` | `emit_toplevel.rs` | Null check + branch for error mode |
| Existing tests unchanged | — | All 847+ tests pass |

---

## 13. Verification Gates

Before committing:
1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Praetor on new/changed files (if installed)
4. Update `AGENTS.md` anchored summary with new completions
5. Commit message references this plan document
