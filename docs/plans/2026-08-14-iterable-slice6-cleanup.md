# Plan: Iterable-Protocol Slice 6 — the remaining leak cleanups

**Date:** 2026-08-14 (handoff). **Head commit:** `3595488c`.
**Supersedes/continues:** `docs/plans/2026-08-12-iterable-protocol.md` §10
(the deletion table), `docs/plans/2026-08-14-string-unification-and-boundary.md`
§6a/§6b (the UOL + `.^Size` deletion).

A new agent can execute this with this document alone plus the referenced
files. Read `docs/architecture/iterable-protocol.md` first for the iterable
contract, and the two OPEN BUGS.md entries this plan resolves.

---

## 1. Background: what already landed

Slice 1-5 and most of slice 6 are DONE:
- **`ringbuf_inline` removed** (`3373bc82`) — verified no benchmark regression.
- **Runtime `.^Size` deleted** (`a223c37d`) — element count is the `Count#`
  intrinsic; `.^^Size` (compile-time) keeps the vector shape.
- **`.^Len` → `.^Length`** rename (prior slice).
- **`is_string_operand` iteration special-casing** subsumed by
  `IterKind::String` (the char-decode lane, `emit_stmt.rs:258`).
- **The hardcoded `List` foreach arm is now PRODUCTION-DEAD** — the prelude
  plugin imports `std/collections.bv` (`a53c7f90`), so `tier2_op_collection`
  fires for `List` in real builds and `try_emit_tier_iteration`
  (`emit_stmt.rs:201`) catches it BEFORE `foreach_collection_kind`
  (`emit_stmt.rs:239`). The `IterKind::List` arm (`emit_stmt.rs:250-253`) is
  reached only by stdlib-free unit tests (`test_foreach_list_program`).
- **`op Count`/`op At` are the structural count/element surface** (`List`,
  `lib/std/collections.bv:83-84`).

## 2. The two remaining deletions (the blockers)

### 2.1 `emit_heap_seq` — live for `Expr::Tuple` and expression-position list literals

`emit_heap_seq` (`emit_expr.rs:1725`) is still called from:
- `Expr::Tuple` (`emit_expr.rs:746`) — a tuple literal allocates a heap
  sequence.
- `Expr::List` (`emit_expr.rs:767`) — an UNANNOTATED list literal used as an
  EXPRESSION (not a typed `let` binding) falls through to `emit_heap_seq`.
  The typed-local path (`construct_local_collection`, `emit_stmt.rs`) only
  catches `let x: List<Int> = [...]`. A list literal in a call
  (`f([1,2])`), a `term [1,2]`, or an arrow target still reaches
  `emit_heap_seq`.

The iterable-protocol plan §10 wants `emit_heap_seq`/`emit_svo_list`/
`emit_svo_index` DELETED (replaced by type-directed literals via
`op Init`+`op InsertAt`). The blocker: `Expr::Tuple` has no op-based
replacement — a tuple is NOT an iterable collection; it's a heterogeneous
product. Deleting `emit_heap_seq` breaks tuples.

### 2.2 The production-dead `IterKind::List` foreach arm

`foreach_collection_kind`'s `List` arm (`emit_stmt.rs:250-253`) reads the
hardcoded `[len]` layout of a `List` struct (`inttoptr` + `load i64`). It is
the fallback when `try_emit_tier_iteration` returns None — which happens for
stdlib-free unit tests (no `obj_members["List"]`). Deleting it requires the
unit test to load stdlib or inject List's ops.

## 3. Design decision

### 3.1 Tuple: a dedicated `emit_tuple` (keep the heap alloc, drop the "seq" coupling)

**Decision:** rename/isolate the tuple path. `Expr::Tuple` is a genuine
heterogeneous product that must heap-allocate when it has no struct-literal
type. Split `emit_heap_seq` into:
- **`emit_tuple`** — the tuple-only allocation (kept; tuples are real).
- **`emit_heap_seq`** — the LIST-literal path (deleted once expression-position
  list literals are routed through ops or rejected).

The plan's `Expr::Tuple` at `emit_expr.rs:746` calls `emit_tuple`; the
`Expr::List` at `:767` is the one being deleted.

### 3.2 Expression-position list literals: require the type-directed form

The unconstrained-literal diagnostic (slice 5, `mod.rs:1872`) already rejects
`let x = [1,2,3]` (no annotation). The remaining hole is a list literal in
NON-`let` positions (`f([1,2])`, `term [1,2]`). **Decision:** a list literal
in any position without a collection type context becomes a compile error
directing to the type-directed form (`let xs: List<Int> = [...]`), OR the
literal's element type is inferred and it constructs via `op Init`+`op
InsertAt` structurally. **Which one**: verify whether `f([1,2])` can get a
collection type from the callee's param (expected-type propagation). If the
callee param is `List<Int>`, construct via ops; otherwise error. This matches
SPEC §16.3.

### 3.3 The dead `IterKind::List` arm

**Decision:** delete the arm after updating `test_foreach_list_program` to use
the tier path (inject `obj_members["List"]` or load stdlib in the unit test).
Then a non-List, non-string, non-data foreach falls to the existing `panic!`
("must be a range, List, Data, or vector field") — which should be updated to
direct to the iterable contract (`op Count`/`op At`).

## 4. Work items (implementation order)

### 4.1 Tuple vs list split (`emit_expr.rs`)

1. Extract the tuple allocation into `emit_tuple(out, v, exprs, indent)` —
   copy the current `Expr::Tuple` path (`:746`) body.
2. Route `Expr::Tuple` → `emit_tuple`. Route `Expr::List` → the type-directed
   literal path (§4.2), removing the `emit_heap_seq` list fallback.
3. Verify `detect_struct_list`/`emit_struct_array` (`emit_expr.rs:754-758`)
   stays — struct-array lists are a SEPARATE path (C-compatible pointer), not
   the heap-seq path.
4. Delete `emit_svo_list`/`emit_svo_index` (`feature_svo` + `svo_max_elements`,
   `emit_expr.rs:765-768`) — SVO was gated off by default; verify no test
   depends on it.

### 4.2 Type-directed expression-position literals

1. In the codegen `Expr::List` path, determine the collection type from
   context: if the literal is a call argument whose param is `List<Int>`,
   construct via `op Init`+`op InsertAt` (reuse `construct_local_collection`
   logic, `emit_stmt.rs`). If no context, ERROR ("collection literal requires a
   type annotation or a typed call parameter").
2. Verify `f([1,2])` where `f(x: List<Int>)` compiles and runs; `term [1,2]`
   with no return context errors.
3. Keep the slice-5 `let` diagnostic (`mod.rs:1872`) unchanged.

### 4.3 Delete the dead `List` foreach arm

1. Update `test_foreach_list_program` (and `test_foreach_list_emits_index_loop`)
   to load stdlib (prelude/plugin path) so `tier2_op_collection` fires — or
   inject `backend.ctx.obj_members["List"]` with `op Count`/`op At`. The test
   then asserts the TIER emission (Count/At member bodies), not the hardcoded
   `[len]` layout.
2. Delete the `IterKind::List` arm in `foreach_collection_kind`
   (`emit_stmt.rs:250-253`).
3. Remove the `IterKind::List` variant from the enum and all match arms
   (`emit_stmt.rs:1374`, `:1479`).
4. Update the `panic!` message to direct to the iterable contract.

### 4.4 Tests

- Tuple literal still compiles + runs (`let t = (1, "x")`; `f((1,2))`).
- Expression-position list literal in a typed call param (`f([1,2])` where
  `f(x: List<Int>)`) compiles and runs.
- Uncontextualized `term [1,2]` errors with the diagnostic.
- `foreach x in list` over a `List<Int>` state field emits the tier path
  (Count/At member bodies), with stdlib loaded.
- The panic message on a non-iterable foreach directs to the contract.

### 4.5 Docs

- Update the iterable-protocol plan §10 deletion table (mark
  `emit_heap_seq`/`emit_svo_*`/`IterKind::List` resolved).
- Update SPEC §16.3 (expression-position literal rule).
- Update `docs/architecture/iterable-protocol.md` surface table.
- BUGS.md: resolve the slice-6 OPEN entry.

## 5. Acceptance criteria

1. `emit_heap_seq` no longer exists (replaced by `emit_tuple` for tuples and
   the type-directed path for lists); `emit_svo_list`/`emit_svo_index` deleted.
2. Tuples work unchanged (no regression).
3. Expression-position list literals construct via ops (typed context) or
   error (no context).
4. `IterKind::List` deleted; `foreach x in list` uses the tier path.
5. Full suite green; `queue_drain`/`stack_push_pop` benchmarks MATCH.

## 6. Known file map

- `src/backend/llvm/emit_expr.rs` — `emit_heap_seq` :1725, `Expr::Tuple` :746,
  `Expr::List` :767, `detect_struct_list`/`emit_struct_array` :754-758,
  `emit_svo_list`/`emit_svo_index` :765-768.
- `src/backend/llvm/emit_stmt.rs` — `try_emit_tier_iteration` :201,
  `foreach_collection_kind` :239, `IterKind::List` :250-253 and `:1374`/`:1479`.
- `src/backend/llvm/tests.rs` — `test_foreach_list_program`,
  `test_foreach_list_emits_index_loop`.
- `src/typechecker/mod.rs:1872` — the slice-5 `let` literal diagnostic.
- `lib/std/collections.bv:77-84` — `List` + `op Count`/`op At`.
