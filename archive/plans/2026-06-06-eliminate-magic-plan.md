# Full Plan: Eliminate All Magic — 2026-06-06T16:39:13Z

## Root Philosophy

`frgn` is just a `call` instruction. `frgn!` is fire-and-forget (no return captured). `import "link/..."` provides the symbols and implies the calling convention. `from` is only for disambiguation when multiple link targets export the same symbol name. The contract system handles non-expected results.

## Implementation Order

### Phase A: Semantic Cleanup (Destroy ForAll/Exists, Fix Report)

**A1. Destroy ForAll/Exists** — 23 occurrences across 12 files. Delete AST variants, parser arms, and every match arm. These are stubs that return `Bool(true)` — deleting them changes nothing functionally.

**A2. Fix session report** — Write `plans/2026-06-06-sig-session-report.md` with both Phase 1 and Phase 2 content fully preserved.

### Phase B: `from`/`link` Architecture

**B1. Fix parser `from` discard bug** — `parser.rs:1142`: `location: String::new()` → `location: loc`.

**B2. `import "link/..."` implies calling convention** — `parser.rs`: recognize `.c` → C, `.rs` → Rust, `.bc` → LLVM IR. Register a `link_registry` on `Program`.

**B3. `frgn` without `from` searches link targets** — If `from` absent, scan `link_registry`. Found in one → use that convention. Found in multiple → error requiring explicit `from`. Found in none → linker reference.

**B4. Typechecker validates known `from` values** — Whitelist: `"c"`, `"rust"`, `"js"`, `"python"`. Emit `TypeError::FFIError` for unknown. Special message for `"libruntime"`.

### Phase C: Remove Hardcoded LLVM Runtime Declares

**C1. Delete `emit_declares()`** — `llvm.rs:1844-1869`. Remove entirely.

**C2. Create `lib/std/rt.bv`** — runtime functions become standard imports.

**C3. Update codegen** — Entry point calls resolve through `self.frgn_map`.

### Phase D: Update All `.bv` Files

**D1. Replace `from "libruntime"` everywhere** — benchmarks, trophies, stdlib.

**D2. Fix `lib/std/out.bv`** — was just written with `from "libruntime"`.

### Phase E: Type-Based Interpreter Method Dispatch

**E1. HashMap/Stack/Queue/StringBuilder** — same native code, type-based dispatch.

**E2. Result methods** — `"is_ok"`, `"is_err"`, `"unwrap"`, `"unwrap_err"`.

**E3. String methods** — `"clone"`, `"char_at"`.

### Phase F: Remove `"None"`/`"Err"` Discriminant Magic

**F1. Use `variant_disc`** — `llvm.rs:511,2628,2938,3478`. Already populated from enum declarations.

### Phase G: `sig #out` LLVM Codegen

**G1. `Expr::SigCall` emits `memory(write)` attribute** — prevents call elimination.

### Phase H: Documentation

**H1. `docs/learn/ffi.md`** — FFI architecture, zero-cost inlining, symbol resolution.

**H2. Update `AGENTS.md`** — no magic `from`, type-based dispatch, runtime via import.

**H3. Update `BUGS.md`** — parser `from` discard, emit_declares, `"None"`/`"Err"` hack.
