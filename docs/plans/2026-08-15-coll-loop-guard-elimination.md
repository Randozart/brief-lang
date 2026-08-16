# Plan: frontend bounded-length grow-guard elimination (queue_drain_idio fix)

**Date:** 2026-08-15
**Head commit:** `a7e3af74` (grow-on-full shipped; queue_drain_idio 0.58x → 4.00x)
**Bug:** the grow-on-full guard inlines an opaque `memory(readwrite)` resize call
into the counted-loop body, blocking LLVM's if-conversion (the call rejoins the
body continuation before the store; the batch-loop's own `when` guard is fast
because it rejoins the latch). See the plan §8 and the 2026-08-15
investigation: `!prof`, `cold`, `argmem`, helper-function, and sibling-block
variants all fail; the branch alone is free (0.03s) — ANY call in the loop's
data-flow path is the cost.

**Post-store-check rejected (2026-08-15).** Moving the check after the store
(`data[len] = val; len++; if len == cap { grow }`) is UNSOUND: `Resize#(h, len)`
/ `TrimCap#(h)` set `cap == len` exactly (the runtime copies `min(len, cap)` and
stores `block[1] = cap`), and the next push stores at index `len` — out of
bounds. The pre-check grows first and is safe there. The reserved-slot variant
shifts `Capacity#` semantics (physically `cap+1`) — rejected without a contract
amendment.

## 1. The fix

The compiler knows the loop, so it should not pay for a guard it can prove dead.
A frontend analysis tracks each coll's length across a txn body and, when it
provably stays below the coll's initial capacity, marks that txn's pushes as
**guard-free**. The backend inlines the push member body WITHOUT the grow guard
at proven-safe arrow sites. Soundness: never strip without a complete proof; the
grow guard stays everywhere else. This is frontend-driven dispatch — the
architecture's core pillar — and does not depend on clang honoring hints.

## 2. Analysis (new pass: `src/analysis/coll_length.rs`)

For each txn (node) body, track the length of every coll referenced by arrows:

- **Arrow semantics**: `q <- v` (InsertAt push) ⇒ `len += 1`; `<- q`
  (ExtractFrom pop) ⇒ `len -= 1`; `q.push(v)` / `q.pop()` member calls — the
  same operations. Other `q` reads (`q.Count#()`, `Capacity#`, `q[0]`,
  `foreach x in q`) do not change length.
- **Initial length**: from the coll's state-field initializer (`[0]` ⇒ 1,
  `[]` ⇒ 0, `[a, b, c]` ⇒ 3); a literal list counts its fields. Unknown ⇒
  conservative (do not prove).
- **Initial capacity**: the scaffold default (16) from `InitEmpty`/`Init`,
  UNLESS the body (or an earlier txn in firing order) calls
  `Resize#`/`EnsureCap#`/`TrimCap#`/`Capacity#` on the coll — capacity-intrinsic
  writes make cap unknown ⇒ conservative (do not prove).
- **Conditional arrows** (inside `when`/`if`/`foreach`): bound both paths.
  `foreach` over a range with a statically-known count adds its bound
  (`foreach x in 0..21 { q <- x }` ⇒ +21); otherwise conservatively assume the
  max possible growth (do not prove).
- **Decision**: compute the sequential max of `len` across the body. If
  `max_len < initial_cap` provably, the txn's pushes for that coll can never
  overflow ⇒ add `(txn_name, coll_type)` to the **safe set**.

**Scope:** the coll must be a `coll obj` (HeapGrowable). A coll only ever
drained (`<- q` without pushes, or balanced pop+push) proves `len ≤ initial`.
A monotone push loop (`foreach x in 0..N { q <- x }`) with a known `N < cap`
also proves. Unknown bounds do not prove — the guard stays (contract never
weakens).

## 3. Communication

`AnalysisResults` gains `coll_safe_txns: HashSet<(String, String)>` (txn name,
coll type name). The backend copies it into `self.ctx` so the member-inline
path can consult it.

## 4. Backend strip

`emit_strategy_member_call` / the ArrowAssign inline path in
`emit_countable_body` knows the current txn name and the coll type. When the
pair is in `coll_safe_txns`, the push member body is emitted WITHOUT its
leading grow guard.

**Guard identification:** the guard is the synthesized push body's FIRST
statement — `Statement::If(BinaryOp(Eq, Identifier("len"), Identifier("cap")),
[Statement::Expression(<grow-action>)], [])`. The inline path skips exactly that
statement when the site is proven safe (checked structurally; any other shape
is kept). This is contained to `emit_member_body`'s caller for coll pushes.

## 5. Verification

- queue_drain_idio back to ≤ ~1.0x vs C (baseline 0.0351s / 0.58x).
- Full suite green (`cargo test --lib`); the grow tests (grow-guard IR,
  override-binding-wins, interpreter parity) still pass — the guard must still
  be emitted in the unproven case.
- New test: a txn where the length is provably bounded emits a guard-FREE push
  (no `__briev_coll_resize` call in the loop), and an unproven txn still emits
  the guard.
- Benchmarks: 37/37 MATCH; only queue_drain_idio's ratio moves.
- Grow semantics re-verified (21/32/210 + ASAN) — the guard still fires when
  the coll actually grows.

## 6. Docs

- This plan. `coll_scaffold.rs` guard comment notes the strip path.
- The status tracker §8 row and the grow-on-full plan §8 get the resolution
  note (regression closed by the length analysis).
- No SPEC change: grow-on-full remains normative; the analysis only removes a
  provably-dead guard (semantics identical).

## 7. Roadmap (unchanged after this)

Coll-struct construction (list-literal→`Int[N]`) → const generics for
`Fixed<T,N>` → stdlib/slice-6 → fundamentals-as-types.
