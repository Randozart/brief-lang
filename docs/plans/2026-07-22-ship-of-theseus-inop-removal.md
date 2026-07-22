# Ship of Theseus: Remove Dead `InopDeclaration` Code

**Date:** 2026-07-22
**Status:** Committed
**Applies to:** `src/ast/`, `src/backend/llvm/`, `src/analysis/`, `src/fuzz_checker/`, `src/import_resolver.rs`, `src/lsp.rs`, `src/macros/selection.rs`

---

## Background

The parser stopped producing `TopLevel::Inop(InopDeclaration)` when `inop` was replaced by `defn + SysCall#` (2026-07-15). The stdlib cleanup (commit `5096b523`) removed all `inop` usage from `lib/std/`. The `InopDeclaration` struct, `TopLevel::Inop` variant, and all code paths that handle them are architecturally dead — the parser never emits them, so every match arm is unreachable.

## Scope

~850 lines removed:

| Category | Lines | Files |
|----------|-------|-------|
| Dead match arms | ~60 | 7 files |
| `emit_inop` function | ~98 | `emit_toplevel.rs` |
| `InopDeclaration` struct + variant | ~20 | `top.rs` |
| `bild_sim.rs` (inop-only fuzz simulator) | 561 | `fuzz_checker/` |
| `bild_symexec.rs` (inop-only symexec) | ~120 | `analysis/` |
| `bild_verifier.rs` (inop-only BILD verifier) | ~90 | `analysis/` |

## Execution

### Phase 1 — Remove all match arms

**`src/backend/llvm/mod.rs`**
- Remove `TopLevel::Inop(inop) => { self.ctx.inop_decls.insert(inop.name.clone(), inop.clone()); }` (line 1780-1782)
- Remove `if let TopLevel::Inop(inop) = item { self.emit_inop(&mut out, inop); writeln!(out).ok(); }` (lines 2281-2284)

**`src/backend/llvm/context.rs`**
- Remove `InopDeclaration` from import (line 25)
- Remove `pub inop_decls: HashMap<String, InopDeclaration>` field (line 93)
- Remove `inop_decls: HashMap::new()` initialization (line 203)

**`src/import_resolver.rs`**
- Remove `if matches!(item, TopLevel::Inop(_)) { return true; }` (lines 624-626)
- Remove `TopLevel::Inop(i) => Some(i.name.as_str()),` arm (line 643)
- Remove `TopLevel::Inop(i) => Some(("inop", &i.name)),` arm (line 771)

**`src/lsp.rs`**
- Remove `TopLevel::Inop(_) => 12,` arm (line 693)
- Remove `TopLevel::Inop(i) => i.span,` arm (line 781)
- Remove `TopLevel::Inop(i) => i.name.clone(),` arm (line 798)
- Remove `TopLevel::Inop(_) => "user-defined intrinsic".to_string(),` arm (line 818)

**`src/macros/selection.rs`**
- Remove `TopLevel::Inop(_) => Some("inop"),` arm (line 361)
- Remove `TopLevel::Inop(i) => Some(&i.name),` arm (line 394)

**`src/fuzz_checker/mod.rs`**
- Remove `InopDeclaration` from import (line 16)
- Remove `TopLevel::Inop(inop) => { verify_inop_fuzz(inop, fuzz_case, case_idx, interpreter, span) }` arm (lines 69-71)
- Remove `fn verify_inop_fuzz(...)` (lines 212-301)

**`src/analysis/transition_graph.rs`**
- Remove lines 51-59 (inop_decls collection from items)
- Remove `inop_decls` parameter from `is_pure_body`, `compute_effectively_pure`, `statement_contains_ffi_with_decls`, `references_triggers_or_ffi_with_decls`
- Collapse `*_with_decls` calls into their non-`_with_decls` counterparts (the parameter is never read)
- Remove `matches!(item, TopLevel::Inop(inop) if inop.has_state_access)` at line 984-986

### Phase 2 — Remove emit infrastructure

**`src/backend/llvm/emit_toplevel.rs`**
- Remove entire `emit_inop` function (lines 1994-2091, ~98 lines of BILD IR emission)

### Phase 3 — Remove AST struct + dead modules

**`src/ast/top.rs`**
- Remove `TopLevel::Inop(InopDeclaration)` variant (line 30)
- Remove `pub struct InopDeclaration { ... }` (lines 662-671)

**`src/fuzz_checker/bild_sim.rs`**
- Delete entire file (561 lines, only supports inop fuzzing)

**`src/analysis/bild_symexec.rs`**
- Delete entire file

**`src/analysis/bild_verifier.rs`**
- Delete entire file

**`src/analysis/mod.rs`**
- Remove `pub mod bild_symexec;` and `pub mod bild_verifier;`

### Phase 4 — Update comments

- `src/ast/top.rs:3` — update to reflect removal
- `src/ast/mod.rs:6` — update to reflect removal
- `src/normalize_types.rs:3` — update to reflect removal
- `src/analysis/equality_saturation.rs:3` — update to reflect removal
- `src/desugarer.rs:3` — update to reflect removal

### Phase 5 — Verify

```bash
cargo test --lib    # all tests pass
cargo build         # 0 errors
grep -rn 'InopDeclaration' src/  # zero hits
```

## Risk Assessment

| Factor | Rating | Rationale |
|--------|--------|-----------|
| Lines removed | ~850 | 300 production + 561 bild_sim.rs |
| Compile risk | Low | Each phase compiles after its changes |
| Behavioral change | None | Every removed code path was unreachable |
| Test impact | ~12 tests removed | All tested dead code (`bild_symexec`, `bild_verifier`) |

## The `transition_graph.rs` Nuance

The `*_with_decls` family (`references_triggers_or_ffi_with_decls`, `statement_contains_ffi_with_decls`) passes `inop_decls` as a parameter that is **never read** in the function body. The fix is:

1. Remove the `inop_decls` parameter from all signatures
2. Inline `references_triggers_or_ffi_with_decls` into `references_triggers_or_ffi` (they become identical)
3. Same for `statement_contains_ffi_with_decls` → `statement_contains_ffi`
4. Remove the `inop_decls` construction (lines 51-59) — was always empty at runtime
5. Remove `TopLevel::Inop` match at lines 984-986 — was always `false`
