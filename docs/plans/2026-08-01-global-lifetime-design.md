# Garbage Scheduling — Global-Lifetime Design Plan

**Date:** 2026-08-01
**Status:** Design → implementation (Phase D2 of
`2026-07-31-collections-watchdogs-memory.md`). The `global_lifetime` stress
benchmark pins this end-state.
**Worktree:** `brief-compiler-cwm` (`feat/collections-watchdogs-memory`)

## 0. The framing: a garbage SCHEDULER, not a garbage collector

This design is a **garbage scheduler** (proof-directed deallocation), not a
garbage collector:

| | Garbage collector | Garbage scheduler (this design) |
|---|---|---|
| Liveness established | at runtime (trace / refcount) | at **compile time** (a proof of each field's last reader) |
| Free fires | at an arbitrary point (GC pause) | at a **deterministic program point** — right after the proven last consumer |
| Cost | mark/sweep/copy + pauses | an O(1) `free` at a scheduled point |
| Correctness | runtime discovery | a **soundness proof** — a field freed but read later is a compile error |

The closest academic analogs are **region inference** (Tofte–Talpin) and
**static reference counting**: compile-time lifetime assignment, not runtime
collection. What makes it possible here is the **deterministic reactor firing
order** — the transition graph says *which* transaction fires when, so "the
last transaction that reads field `f`" is a well-defined, provable notion that
C-style arbitrary control flow cannot soundly answer.

The scheduler is **partial and conservative**: when the proof cannot establish
the last use (an unordered reader, an escaping pointer, an FFI alias), the
field falls back to "lives for the program" — it leaks by design rather than
risk a premature free. The contract is *sound (never premature) but not
complete (may not reclaim)*, matching the never-faster principle: a scheduled
free must never be beaten by a manual one, and a wrong free is a compiler bug.

## 1. Goal

Emit a `Free#` for a heap-backed state field when the compiler can PROVE the
field is never read again. Today **all state lives for the program's duration**:
`analysis/allocation.rs` builds a producer→consumer DAG per transaction and
chooses Arena (in-txn) / Alloca (bounded) / Malloc (top-level), and the arena
bumps + realloc-grows — but a top-level Malloc-backed state field is never freed.
Garbage scheduling closes that gap: a heap-backed field provably dead after its
last consumer is freed exactly once, at the scheduled point after that consumer.

Correctness is the contract: **no premature free, no leak**. A field is freed
only when the last read is proven; anything else (a read after the free) is a
compile error, never a silent dangling deref.

## 2. Current reality (verified 2026-08-01)

| Mechanism | What it does | What it does NOT do |
|---|---|---|
| `analysis/allocation.rs` | per-`Alloc#` strategy: Arena (in-txn) / Alloca (bounded) / Malloc (escaped or top-level); producer→consumer DAG traces escapes | no whole-program lifetime: a Malloc-backed state field is never freed |
| `Provenance` (`analysis/provenance.rs`) | tracks pointer origins (`Known`, `FieldAccess`, `Index`, `Deref`), flags `&local` escapes (Phase D1) | no liveness: provenance says WHERE a pointer came from, not WHEN it dies |
| Arena (`emit_arena_fini`) | bumps a 64KB buffer, realloc-grows | in-txn only; state arena persists for the program |

State fields that are heap-backed (a `Ptr<T>` slot, an obj instance, or a
`Malloc#` result stored in state) live for the program. `global_lifetime` is
the benchmark that makes the leak observable.

## 3. The design (locked)

### 3.1 The core proof

For each heap-backed state field `f`:

1. **Collect every transaction/defn that READS `f`** (the read set). Reads are
   any `Expr` referencing `f` that is not a store of a fresh value to `f`.
2. **Order the transactions** by the reactor's execution model: a txn `T1`
   provably precedes `T2` when they are ordered (the transition graph / reactor
   dispatch determines firing order). Two unordered txns that BOTH read `f`
   block the proof (an unordered later reader may exist).
3. **`f`'s last consumer** = the latest ordered transaction in the read set
   that has no later reader.
4. **Emit `Free#(f)` after the last consumer's body**, exactly once, in the
   state where `f` is no longer read by any reachable path.

### 3.2 What blocks the proof (conservative)

- An unordered txn in the read set (could fire after the free) → **no free**.
- A `defn` that reads `f` and is called from an unanalyzed path → **no free**.
- `f` escaped (its address stored in another heap object / passed to FFI)
  → **no free** (the alias is out of scope).
- A `when` guard reading `f` in a txn not provably before the last consumer
  → **no free**.

**Freeing is a privilege, not a default.** The proof must be airtight; the
default is "lives for the program".

### 3.3 Where the free is emitted

The free is emitted in the FRONTEND-driven dispatch (per the architecture
pillar): `AnalysisResults` gains a `free_after: HashMap<String, String>`
(field → last-consumer txn name), computed once by a new
`analysis/global_lifetime.rs` pass. The backend's reactor emission, when it
finishes the last-consumer txn's body, emits `Free#(field_handle)` before the
txn's term. Tunables in `config/ir-lowering.toml`.

### 3.4 Soundness rule

Every emitted free is paired with a **compile-time assertion** in the pass: the
free field must not appear in any `collect_read_identifiers` set of a txn that
can fire after the last consumer. If the assertion fails, the pass itself is
buggy — the compiler refuses to emit (a compile error, not a miscompile).

## 4. The `global_lifetime` benchmark

The benchmark makes the leak/free observable and calibrates correctness:

- A state `Ptr<Int>` heap buffer, written by an init txn, read by a consumer
  txn, then provably dead.
- The consumer prints a value derived from the buffer; a `Free#` counter in the
  runtime (`__brief_free_count`) is printed at the end.
- **Correctness contract**: (a) the freed buffer is never dereferenced after the
  free (a read-after-free is a compile error, not a runtime crash); (b) the free
  count matches the allocation count exactly (no leak, no double-free).

The C reference mirrors the same allocate/read/free lifecycle and prints the
same values; the Brief version additionally prints the free count (the
`_sym`/`_idio` split: `_sym` mirrors C step-for-step, `_idio` uses the
Brief-native free-after-last-consumer form).

## 5. Edge cases

| Case | Behavior |
|---|---|
| Field read only in the LAST txn of the program | freed after that txn's body |
| Field never read after init | freed after the init txn |
| Two unordered readers | no free (conservative) |
| Field read in a `when` guard only | the guard is a read; the free goes after the guard's txn |
| Field is a `Ptr` to a heap object with an internal pointer | the internal pointer is an escape; no free |
| `--no-stdlib` | the pass needs no stdlib (Free# is an intrinsic); behavior identical |
| Free emitted in a loop body | never — a free is emitted only after a txn's body, never inside a loop that may iterate again |

## 6. Relationship to the existing passes

- **`analysis/allocation.rs`** decides WHERE a field's backing comes from
  (Arena/Alloca/Malloc). Global-lifetime decides WHEN a Malloc-backed field can
  be freed. The passes compose: allocation first (strategy), lifetime second
  (free timing). A field with Arena/Alloca strategy is never heap-freed.
- **`Provenance`** supplies the escape test (3.2): a field whose address
  escaped has `Provenance::Unknown`/Deref origins in another object — the pass
  asks the provenance layer before freeing.
- **The reactor** (`transition_graph.rs`) supplies the ordering — the firing
  order that makes "last consumer" well-defined.

## 7. Verification plan

1. `global_lifetime` builds, runs, and the free count == allocation count.
2. A read-after-free program is a **compile error** (the pass asserts the free
   field is not in any later read set).
3. A premature-free program (an unordered second reader) is a **compile error**
   or falls back to "no free" (conservative), never a miscompile.
4. Full `cargo test --lib`; `cargo build` no new warnings; Praetor clean.
5. The free is emitted in the frontend-computed dispatch (no backend heuristics).
