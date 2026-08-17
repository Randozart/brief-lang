# Plan: HashMap storage + tuple correctness (final, unambiguous)

**Date:** 2026-08-17. **Supersedes:** the storage/API decisions in
`2026-08-16-hashmap-redesign.md`. **Head commit:** `26bb0955`.
**Build/test commands:** `cargo build` / `cargo test --lib` /
`bash benchmarks/build_and_bench.sh --runtime` / `./target/release/brievc check <file>`.

## Context and corrections

Commit `26bb0955` shipped a HashMap redesign with three wrong decisions. The
user's corrections (all verified against source):

1. **Unpacked (SoA) columns are the intentional design.** The instance-pool
   machinery (`build_field_index` mod.rs:5119+, `emit_instance_init`
   emit_toplevel.rs:1529+, `self_prefix` member resolution emit_expr.rs:222-233,
   and the "pool instance must NEVER reach the boxed path" guard
   emit_expr.rs:2484-2489) is the compiler's efficiency decision. My
   `is_heap_coll` op-surface force (mod.rs:5515) contradicted it.
2. **Tuples are codegen gaps, not fundamentals.** Three concrete bugs:
   (a) destructure codegen drops `names` (emit_stmt.rs:292 `..`), while the
   parser (statements.rs:145-152) and typechecker (typechecker:2383) handle it;
   (b) the parser rejects numeric field access `.0`/`.1` (expressions.rs:278
   `expect_identifier`), while `resolve_field_type` (typechecker:3656) supports
   it; (c) `llvm_type(Type::Tuple)` resolves to `{ i64, i64 }` but call sites
   pass a boxed i64 handle.
3. **"Overflows clang" was misdiagnosed.** A minimal hot-loop HashMap compiles
   fine; the crash requires the benchmark's `when i % 5000000 == 0` guard. This
   is the documented Guarded/`cur_block` countdown-latch-phi fragility
   (emit_stmt.rs:1254: "clang's LoopDeletionPass then crashes").

**Design decisions (locked):**
- **Q1 storage:** state-field HashMap UNPACKS to SoA columns; local HashMap
  BOXES (per-firing). The existing machinery is the compiler's choice. No new
  heuristic or strategy keyword.
- **Q2 API:** tuple pairs — `m <- (k,v)`, `[(1,10),(2,20)]`,
  `let (k,v) = p`, `foreach p in m` yields `(K,V)`, `get(k)`/`contains(k)`/
  `remove(k)` by key. `Entry<K,V>` is dropped once all tuple paths work.
- **Q3 scope:** fix the `when`-guard crash NOW, do not defer.

**Terminology:** "op surface" = the operator members a type declares
(`op Count`/`op At`/`op InsertAt`/`op Init`/`op Iter`/`op Step`/`op IsEnd`/
`op Current`) that the collection dispatch keys on. Op-driven, never
`coll`-keyword or type-name based.

---

## Phase 1 — Restore SoA storage

**1.1** Revert `is_heap_coll` in `src/backend/llvm/mod.rs` (~5515) to the
pre-`26bb0955` body: `matches!(self.ctx.coll_storage.get(base), Some(CollStorage::HeapGrowable))`.
Remove the operator_defs op-surface check and its comment. HashMap is NOT in
`coll_storage` (it's a hand-written obj), so it unpacks again.

**1.2** First execution check: build release, compile
`let m: HashMap<Int,Int> = 0; m.insert(...); m.get(...)` (state-field form).
Expected: `m` registers as 5 unpacked columns (`HashMap.keys`/`.vals`/
`.occupied`/`.count`/`.cap`), `op Init` runs against row 0 via
`emit_instance_init`, member bodies resolve bare names via `self_prefix`.
If `init` does not write the columns (columns stay 0 → NULL deref), fix the
unpack registration or `emit_instance_init` — that is the real bug, NOT
unpacking.

**1.3** Keep unchanged (all op-driven and correct):
- `Count#` dispatch → `emit_method_call("Count")` (intrinsics.rs:165).
- `<-` insert → `find_insert_strategy` → `op InsertAt` binding.
- literal construction gates: `construct_local_collection` op-surface gate
  (emit_toplevel.rs:1277) and typechecker `list_literal_accepted_by` /
  `declares_collection_ops` (typechecker ~1213-1290).
- `construct_local_collection_seed` (emit_toplevel.rs ~1243) — locals boxed.

**1.4 Acceptance:** state + local HashMap, `Count#`, `get`, `contains`,
`remove`, literal `[...]` all run with correct values. Full `cargo test --lib`
green. No new Praetor diagnostics.

## Phase 2 — Fix tuples

**2.1 Destructure codegen** (`src/backend/llvm/emit_stmt.rs:292` Let arm).
When `names.len() > 1`, after emitting the RHS value (a boxed tuple handle,
`emit_tuple` layout `[len, e0, e1, …]` i64 block): for each `names[i]`, emit
`inttoptr handle`, `GEP i64 slot i+1`, `load i64`, bind `names[i]` to that
register (and to `let_binding_types` with the element type). The parser
already puts `name = names[0]`; bind it the same way (element 0). Reuse the
tuple-element offset logic. Typechecker already bound element types
(`check_let_destructure`, typechecker:2383).

**2.2 Numeric field access.**
- Parser (`src/parser/expressions.rs:276-278`): after `.`, if the next token is
  an Integer literal, accept it as a numeric field name (else `expect_identifier`).
- Backend `Expr::Field` arm (emit_expr.rs ~2186, `emit_field_access`): when the
  receiver's type is `Type::Tuple`, a numeric field name indexes the boxed
  tuple block: `inttoptr handle`, `GEP i64 slot (n+1)`, `load i64`, typed with
  the tuple's element type n. Mirrors 2.1.
- `resolve_field_type` (typechecker:3656) already handles numeric — no change.

**2.3 Tuple value ABI** (`src/backend/llvm/emit_toplevel.rs`, `llvm_type`
~530-620 and the defn-param builder ~2356-2366). Add an early arm:
`Type::Tuple(_) => format!("i{}", int_bits)` (boxed handle, same as obj
values at 580-582), so defn params, returns, and struct fields carry the
boxed i64 ABI consistently. Fixes `%arg0 {i64,i64}` vs `inttoptr i64`.
Verify the defn body's param binding adapts the boxed handle (already does
for obj values).

**2.4 Acceptance (interpreter reference, rule 4):**
`let (a,b) = (1,2)`; `t.0`/`t.1` on a tuple binding; a `defn` taking and
returning `(Int,Int)`; `List<(Int,Int)>` foreach + destructure. All agree with
the interpreter's Product semantics.

## Phase 3 — HashMap on tuple pairs

**3.1** `lib/std/collections.bv`, `obj HashMap<K,V>`:
- `txn insert(e: (K, V)) [count < cap][count <= cap]` — `let (k, v) = e;`
  linear-probe (done-flag pattern, as shipped).
- `op InsertAt: insert(#Lh, #Rh)` (unchanged binding, now a tuple param).
- `op Current(i: Int) -> (K, V)` — returns the pair `(keys[i], vals[i])`;
  `foreach p in m` yields `(K,V)` pairs; `let (k,v) = p` in the body.
- `get(key: K) -> V`, `contains(key: K) -> Bool`, `remove(key: K) -> V` —
  unchanged (key-arg).
- Restore `keys()`, `values()`, `entries()` scans: `List<K>`/`List<V>`/
  `List<(K,V)>` accumulators with `acc <- ...` pushes. Investigate the
  cross-import free-T arrow (why `brievc check lib/std/collections.bv` passes
  but a consumer import fails) and fix it as part of the tuple work — likely
  `push_element_type`/`substitute_type_params` for a `List<(K,V)>` element
  inside a generic member.
- Delete `struct Entry<K,V>` if no file references it after 3.3.

**3.2** `lib/std/hashmap.bv`: rewrite `insert`/`get`/`contains_key`/`remove`/
`len`/`is_empty`/`keys`/`values`/`entries`/`clear`/`merge` on the tuple
surface. `brievc check` clean.

**3.3** Grep the repo for `Entry` — update/remove `hash_ops_idio.bv`, tests,
and docs that referenced the struct. `Entry` is dropped unless a tuple path
proves unfixable (it should not).

**3.4 Acceptance:** `m <- (k,v)`, `let m: HashMap<Int,Int> = [(1,10),(2,20)]`,
`m.get(k)`, `m.contains(k)`, `m.remove(k)`, `m.Count#()`, `foreach p in m`
with destructure, `m.keys()`, `m.values()`, `m.entries()` — all run with
correct values (state + local forms). `brievc check` clean on all stdlib.

## Phase 4 — `when`-guard clang crash (fix, do not defer)

**4.1** Reproduce minimal: countdown node (`node work [i < N][i == N]`) whose
body contains a nested probe `foreach` AND a `when cond { println!(sum) }`
guard. `env BOUND=... brievc build` → clang segfault.

**4.2** Trace: the countdown-loop latch phis key their body predecessor on
`fun.cur_block` (emit_stmt.rs:1249-1256 comment). The Guarded arm sets
`cur_block = guard.endN` (line 1256). A nested probe `foreach` inside the
body, when its own latch phi is computed, may capture a stale/wrong
predecessor when a `when`-guard is the body's final construct. Fix the
phi/predecessor bookkeeping so every nested loop's latch sees the guard's
merge label. Use `llc`/`clang -S` to confirm valid IR (no invalid-phi).

**4.3** Restore `hash_ops_idio` to `benchmarks/build_and_bench.sh` (~168).
Confirm it builds with `BOUND=50000000`, runs, and its output MATCHes the C
reference. Keep the `.bv` on the tuple API (3.3).

**4.4 Acceptance:** the minimal case and hash_ops_idio both build and run;
benchmark suite all MATCH (hash_ops_idio included).

## Phase 5 — Tests + docs

**5.1** Tests:
- typechecker: tuple destructure typechecks (already does — add a regression),
  numeric field access, HashMap tuple API, literal construction.
- backend: tuple destructure IR (element GEPs), tuple field access, HashMap
  state-field SoA columns (no `@m.` unpack assertions), local HashMap boxed,
  foreach-over-map.
- interpreter: tuple destructure parity, HashMap (as Product) parity.
- runtime consumers: state + local HashMap end-to-end.

**5.2** Docs:
- `docs/plans/2026-08-16-hashmap-redesign.md`: append SHIPPED revision
  (SoA restored, tuples fixed, when-guard fixed; Entry dropped).
- This plan's SHIPPED section.
- BUGS.md: close hashmap.bv (already), log the three tuple-bug fixes (closed),
  log the `when`-guard fix (closed).
- SPEC §17.1: hand-written collection obj via op surface; tuple pairs.

**5.3** Acceptance: `cargo test --lib` green; Praetor no NEW diagnostics
(baseline: emit_stmt 6, emit_toplevel 15, mod.rs 19, typechecker 17,
expressions.rs/statements.rs as-measured); benchmarks all MATCH including
hash_ops_idio; `git grep` for `Entry` zero in `src/` and `lib/std/`.

## Execution order

1 → 2.1 → 2.2 → 2.3 → 2.4 → 3.1 → 3.2 → 3.3 → 3.4 → 4.1 → 4.2 → 4.3 →
4.4 → 5.1 → 5.2 → 5.3. Commit after each phase that leaves the suite green.

---

## SHIPPED (2026-08-17, Phases 1-3)

### P1 — SoA storage restored
`is_heap_coll` (mod.rs) reverted to the `coll_storage`-only check; a hand-written
`obj HashMap` UNPACKS to SoA columns for state fields. Verified: `%State` has
the 5 member columns (`keys`/`vals`/`occupied`/`count`/`cap`), `op Init` writes
them, member bodies resolve via `self_prefix`. Added `is_op_surface_coll`
(storage-independent) for the LOCAL construction decision — locals box via
`construct_local_collection_seed`.

### P2 — Tuple correctness (three fixes)
1. **Destructure codegen** (emit_stmt.rs Let arm): `let (a,b) = t` splits the
   boxed tuple handle into element registers (`GEP i64 slot i+1`), binds each
   name with its element type. Verified: `let (a,b) = (1,2)`, triples, mixed
   Float/Int.
2. **Numeric field access**: parser accepts `.0`/`.1` after a tuple receiver;
   backend `Expr::Field` Tuple-receiver arm GEPs the block slot. Verified:
   `t.0 + t.1`.
3. **Tuple value ABI**: `llvm_type(Type::Tuple)` → boxed `i64` (consistent with
   call sites and `emit_tuple`). Verified: tuple params/returns through defns,
   `List<(Int,Int)>` At + destructure (previously `@v` undefined).

### P3 — HashMap on tuple pairs (core)
`obj HashMap<K,V>` element is the `(K,V)` pair: `insert((k,v))` with
`let (k,v) = e`, `op Count`, `get`/`contains`/`remove` by key, linear probe
with `p = (h+q)%cap` derived from the foreach counter (no loop-carried position
register). `Entry<K,V>` struct RETIRED. State SoA + local boxed both verified
via direct member calls.

### Blocked (pre-existing compiler bugs, BUGS.md)
- **Member-inline + foreach register collision**: multiple inlined probe
  members (3+ inserts, or 2+ gets) in one SSA node body produce forward-
  referenced registers (clang "instruction forward referenced"). A plain
  node-body foreach and a SINGLE inlined member work.
- **Defn-param mutation loss**: a map passed as a defn param then mutated via
  a member (`insert(map, ...)` wrapper) loses the mutation (prints 0) — the
  member-on-param ABI bug.
- **Iteration scans** (`keys()`/`values()`/`entries()`): `acc <- keys[i]` into
  a loop-carried local List double-constructs the List (the return reads a
  stale original). The mirror-list field approach hit the same push-in-member
  register issue.
- **`foreach p in m`** (Tier-1 cursor ops): removed from the map; the cursor
  ops' internal `foreach` overflows the SSA bookkeeping. Iteration is via
  `entries()` (blocked by the scan bug) — documented follow-up.

### Acceptance
1889 lib tests green; all stdlib `brievc check` clean; `cargo build` no new
warnings; Praetor no new diagnostics (verified vs baseline). P4 (the
`when`-guard clang crash) is NOT yet fixed — separate follow-up.

## P4 — `when`-guard clang crash (FIXED, 2026-08-17)

**Bug (exact):** a countdown node whose body contains a nested `foreach` (an
inlined collection member's probe loop) AND a `when`-guard produced IR where
the countdown latch phi's predecessor set included the nested loop's internal
blocks — `llc`: "Instruction does not dominate all uses! `%cdm337 = sub i64
%cdr136, 1`". clang's LoopDeletionPass then segfaulted.

**Fix (emit_stmt.rs Foreach arm):** after emitting `foreach.endN`, set
`fun.cur_block = Some(end_lbl)` — mirroring the Guarded/If arms (1289/1323).
A countdown body that ends in a `foreach` now reports the foreach's END block
so the countdown places its decrement + latch phi there. Verified: the pure
`when`-guard countdown prints correct sums (45/190/435 — the trailing 0 from a
non-converging `node fin` is pre-existing baseline behavior); hash_ops_idio
now COMPILES and RUNS without crashing clang.

**Remaining (separate pre-existing bug, BUGS.md):** the multi-member register
collision — `m.get(i)` returns 0 when 3+ inlined probe members share a node
body (hash_ops_idio compiles but its hot-loop get reads wrong values). Until
fixed, hash_ops_idio stays out of the suite (it would fail the correctness
check). The P4 fix (compiler crash) is independent and landed.
