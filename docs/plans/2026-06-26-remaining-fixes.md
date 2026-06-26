# Remaining Fixes: Proof Engine Defn Analysis, Contract Note, let mut

**Date:** 2026-06-26
**Status:** Active

## Fix 1: Proof engine — defn path analysis with guards

`append_bool` in string_builder.bv has no explicit contracts (removed),
but the proof engine's `verify_definition` still enters path-splitting
on `[b] { term ... }; term ...` and reports P008 on the fallthrough
path. The engine needs to accept that every path through a defn
produces a satisfactory output.

The fix: in `collect_entity_paths` or the defn verification entry point,
treat guarded statements inside a defn as terminating paths when the
guard body contains a `term`. The `is_proven_terminable` check should
not apply — the engine should verify `append_bool` passes by checking
that both paths (guard-true → term "true", guard-false → term "false")
are valid and cover all inputs.

## Fix 2: Remove contract-position note

The note `"contracts read more clearly before the return type"` fires
on every `-> Type [pre][post]` syntax. Remove the note emission.

## Fix 3: Remove `let mut` from volatile-io.bv and target-import.bv

Replace 8 occurrences of `let mut` with `let` across both files.
Variables in Brief are mutable by default — `mut` is not a keyword.
