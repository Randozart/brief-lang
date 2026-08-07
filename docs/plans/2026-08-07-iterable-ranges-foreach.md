# Iterable Ranges + `foreach`

**Date:** 2026-08-07 · **Phase:** 7 (§11.4/§16.4) · **Status:** Shipped (ranges)

## Goal

`foreach(item in iterable)` is the sole iteration keyword (SPEC §11.4 — no
`for`/`while`/`loop`). "Counted iteration uses iterable ranges." This slice
ships:

- **Range expressions** `start..end` (half-open) / `start..=end` (inclusive)
  as iterables (SPEC §16.4).
- **`foreach` over a range** — a counted loop — end-to-end (parser →
  typechecker → interpreter → codegen).
- `foreach` over **collections** (lists, byte data) in the INTERPRETER (the
  reference); the codegen for collection iterables is a documented follow-up
  (hard error, no silent wrongness).

## Semantics

- `foreach(i in start..=end) { body }` binds `i` to each integer in
  `[start, end]`, ascending; `start..end` excludes `end`. An empty range
  (`start > end`) skips the body.
- `foreach(item in list) { body }` binds `item` to each element of a list /
  byte data value in order (interpreter).
- A range used OUTSIDE `foreach` (as a scalar value) is a hard error in the
  codegen (ranges are iterables, not values).

## Implementation

- **Parser**: `Expr::Range { start, end, inclusive }` — postfix arms for
  `..`/`..=` after a primary (the only other `..`/`..=` consumer is the
  range PATTERN parser).
- **AST**: new `Expr::Range` variant; display renders `a..b` / `a..=b`;
  obfuscate + all expression walkers (annotator, dataflow, dependency_graph,
  licm, allocation, symbolic, env_plugin, macros, backend collect walkers)
  gained recurse/leaf arms.
- **Typechecker**: `Range` infers to `Applied("Range", [Int])`.
- **Interpreter**: `Value::Range { start, end, inclusive }`; `foreach` binds
  the loop variable per iteration and runs the body — over a Range, a
  Product (collection), or Bits (byte data). A new `describe_value`/marshal
  arm (no FFI payload).
- **Codegen** (`emit_stmt.rs` Foreach arm): the counted loop — evaluate
  start/end, an alloca slot for the loop var, header compare (`sle`/`slt`),
  body with the loop var bound like a `let`, increment + back-edge. The
  `terminated` flag guards the back-edge against value-form `term`s.
- **Loop-carried locals**: a register-bound `let` assigned in the body (e.g.
  `acc = acc + i`) would read its STALE initial register every iteration
  (the body IR is emitted once). The foreach pre-declares an alloca slot
  seeded with the current value, and the Identifier resolution loads
  slot-backed lets BEFORE the `last_val_temps` fast path. (This fixes the
  same latent bug for txn-local reassigned lets generally.)
- **Liveness**: `scan_for_state_identifiers` + `collect_state_identifiers`
  skipped `Statement::Foreach` and `Expr::Range` entirely, so a range bound
  (`foreach(i in 0..=n)`) or an iterated collection field was pruned as
  `Never` → undefined `@n`. Both now walk the iterable + body.

## Tests

- Parser: `0..=5` → inclusive Range, `0..5` → half-open.
- Interpreter: foreach over inclusive/exclusive/empty ranges accumulates
  correctly; foreach over a Product iterates elements.
- Codegen: the counted loop (`foreach.hdr`, `icmp sle`, `foreach.end`).
- End-to-end: `sum 0..=5` = 15, `sum 0..5` = 10, `sum 1..=n` (state bound)
  = 10, branching body = 9.

## Boundaries / follow-ups

- Codegen for `foreach` over COLLECTIONS (List/Data/vector fields) — the
  interpreter handles them today; a codegen index-loop is the next slice.
- Range as a first-class value (outside foreach) — currently a hard error.
- The Float-vector mask indexing follow-up (unchanged from the mask plan).
