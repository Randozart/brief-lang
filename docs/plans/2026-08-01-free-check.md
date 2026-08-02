# The Free-Check — `free`/`keep` Hints, Refcounts, and `memcheck`

**Date:** 2026-08-01
**Status:** Active (Phase 5 of the master plan,
`2026-08-01-consumptive-operators-lifetime-and-c-surface.md`)
**Plan map:** sibling of `2026-08-01-global-lifetime-design.md` (garbage
scheduling), which it extends.

## Problem

The garbage scheduler (`src/analysis/global_lifetime.rs`) proves, at compile
time, the reactor-ordered LAST transaction that touches each heap-backed state
field and schedules a free after it. The contract is *sound but not complete*:
when the proof cannot establish the last use (an unordered reader, an escaping
pointer, an FFI alias), the field falls back to "lives for the program" — a
leak. The developer needs a way to (a) VERIFY a specific deallocation point,
(b) SUPPRESS a scheduler decision the developer knows is wrong, and (c) see
what the scheduler decided.

## The annotations

| Hint | Meaning | Contract | Codegen | Scheduler |
|---|---|---|---|---|
| `free x;` | the backing of `x` is freed HERE | a later read of `x` is a compile error | emits the strategy-aware free (`__brief_free`/`@free` for Malloc-backed, no-op otherwise) | excludes `x` from its auto-free (no double-free) |
| `keep x;` | the scheduler must NOT auto-free `x` (it escapes / is freed elsewhere) | `x` must exist | no runtime emission (an analysis directive) | excludes `x` from its auto-free; a `keep` on a field it would not free anyway is a **redundant-keep warning** |

Both are whole-statement hints (`free x;` / `keep x;`), parsed as
`Statement::FreeHint(String)` / `Statement::KeepHint(String)`. They are
body annotations — a `keep` lives in the transaction whose auto-free it
suppresses; a `free` marks the exact deallocation point.

## The `free` contract in the move pass

`free x;` joins `~x` (consume) in the typechecker's dead-local tracking:
`x` must be a mutable location (a constant cannot be freed), must not already
be dead, and a later read is a compile error. A reassignment (`x = v`) revives
it — matching the consume semantics, so the two mechanisms share one rule:
**a value is dead after its backing is destroyed, until reassigned.**

## memcheck

`briefc memcheck <file.bv>` is the diagnostics subcommand: it runs the
garbage-scheduler analysis and reports, per heap-backed state field:
- whether the scheduler proved a last use and scheduled a free (and after
  which transaction),
- whether it fell back to "lives for the program" (an unprovable field — a
  potential leak),
- the effect of every `free`/`keep` hint (applied, or redundant).

## Refcount free-check (edge-of-use)

For an unprovable heap field, the developer may opt into a runtime refcount:
the scheduler inserts a counter at the edge-of-use checkpoint (the last
transaction that *might* use the field, when the static proof is ambiguous);
each use decrements; a zero count triggers `__brief_free`. The counter is
backend-emitted alongside the field; the correctness contract is *no premature
free* (a zero count means no further use is possible on the analyzed path) and
*no double-free* (the `free`/`keep` exclusions still apply).

## Correctness contract

1. **Sound (never premature):** a scheduled free happens only after the last
   proven use. `free x;` is a developer VERIFIED contract — the typechecker
   enforces no later read.
2. **No double-free:** the scheduler excludes every field that has a manual
   `Free#`, a `free x;`, or a `keep x;`.
3. **Observable:** memcheck reports the decisions; redundant `keep` hints warn.

## Files

- `src/analysis/global_lifetime.rs` — scheduler; `redundant_keeps` detection.
- `src/parser/statements.rs`, `src/ast/top.rs` — the hint statements.
- `src/typechecker/mod.rs` — the free contract (dead-local tracking).
- `src/backend/llvm/emit_stmt.rs` — the `free` emission.
- `src/main.rs` — the `memcheck` subcommand.
