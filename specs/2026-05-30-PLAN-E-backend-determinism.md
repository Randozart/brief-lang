# PLAN E: LLVM Backend Determinism & Cleanup Isolation

**Date**: 2026-05-30
**Author**: External audit + internal verification

## Background

An external audit of the LLVM backend (`src/backend/llvm.rs`) identified two remaining correctness issues after the previous round of fixes. Both are verified as valid.

## Bug 1: OnExit Cross-Transaction Contamination

**Root Cause**: `pending_cleanup: Vec<Statement>` is a persistent field on `LlvmBackend` but is never cleared at transaction or definition boundaries. `emit_transaction` clears `let_bindings`, `field_to_meta_idx`, and `terminated` but omits `pending_cleanup`. `emit_definition` only clears `let_bindings`.

**Impact**: Cleanup statements from Transaction A's `OnExit` blocks leak into Transaction B's emitted IR. On Transaction B's exit points, the cloned cleanup clone emits Transaction A's cleanup code inside Transaction B's assembly.

**Fix**: Add `self.pending_cleanup.clear();` at the start of:
- `emit_transaction` (line 552)
- `emit_definition` (line 513)

## Bug 2: Non-Deterministic `init_state` Emission

**Root Cause**: `emit_init_state` (line 501) iterates `&self.field_index_map`, which is `HashMap<String, usize>`. Rust HashMap iteration order is randomized (SipHash seed per process), so the same source produces different `.ll` output across runs.

**Impact**: Reproducible builds are broken — bit-for-bit identical output from identical source is not guaranteed.

**Fix**: Sort fields by their struct offset index before emitting:

```rust
let mut fields: Vec<_> = self.field_index_map.iter().collect();
fields.sort_by_key(|&(_, &idx)| idx);
for (name, &idx) in fields { ... }
```

## Files Changed

- `src/backend/llvm.rs` — both fixes in one file

## Verification

- `cargo build` — must compile cleanly
- `cargo test --lib` — all 294 tests must pass