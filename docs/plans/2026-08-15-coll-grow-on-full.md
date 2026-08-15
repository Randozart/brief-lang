# Plan: coll grow-on-full auto-trigger

**Date:** 2026-08-15
**Head commit:** `4dfffb88` (decision documentation; baseline captured at this
commit)
**Design source:** `docs/plans/2026-08-15-coll-length-semantics.md` §3.6 (the
mutation model) — normative per the 2026-08-15 decision, recorded in
`spec/SPEC.md` §8.10/§15.2 and `learn-briev/05-data-types.md`.
**Bug:** `docs/plans/2026-08-15-coll-length-semantics.md` §12 (final) — the
grow-on-full auto-trigger was deferred as a "future slice," leaving the
shipped scaffold with a memory-safety hole.

## 1. The bug

The scaffolded `coll obj` InsertAt (`push`) is
`data[len] = val; len = len + 1;` with **no capacity guard**
(`src/backend/llvm/coll_scaffold.rs:164-201`), and `init`/`init_empty`
allocate exactly `16 × 8` bytes (`coll_scaffold.rs:229-259`). Pushing past
16 elements writes past the heap block — silent memory corruption. The
reference interpreter's collection value is a `Value::Product` (a `Vec`),
which grows freely — so the interpreter appends correctly and the backend
diverges from the reference (rule 4: fix codegen, never the interpreter).

The deferral reason is documented at `coll_scaffold.rs:166-169`: *"A grow
guard needs a phi merge of the old/new data pointer across the guard branch,
which the member-body self-slot write path doesn't provide yet."*

## 2. The decision (normative, 2026-08-15)

Grow-on-full is the default behavior, not a future slice (SPEC §8.10/§15.2):

- When an insertion would exceed capacity (`len == cap`), the compiler's
  default `op Grow` **doubles** the capacity before the element is stored.
  An insert past capacity is never an out-of-bounds write and never requires
  the author to call `Resize#`/`EnsureCap#` first.
- The default is the efficient path (rule 2): the guard is one compare +
  branch on the never-taken path; a `coll`-beaten hand-written collection is
  a compiler bug.
- A type overrides the policy with its own `op Grow`/`op Shrink` handle-only
  binding (`op Grow: grow(#Lh)`); **binding wins**.
- A `coll struct` (fixed `T[N]`) never grows: its `InsertAt` carries a
  `len < N` obligation — an insert past N is a precondition error.
- Interpreter parity (§3.6 ambiguity #6): `Capacity#(product)` = field count
  (exact-fit), `Resize#`/`EnsureCap#`/`TrimCap#` on a product are no-ops —
  a `Vec` grows freely and capacity is not observable.

## 3. Baseline (rule 11)

`bash benchmarks/build_and_bench.sh --runtime` at commit `4dfffb88`
(clean `cargo build --release`; 5 iterations/benchmark, BOUND=50_000_000).

| Benchmark | Briev | C | Ratio | Winner | Correct |
|-----------|:----:|:--:|:-----:|:------:|:-------:|
| ring_buffer | .0546s | .0465s | 1.17x | C | MATCH |
| float_math | .0449s | .0712s | .63x | Briev | MATCH |
| float_math_nonzero | .1587s | .1649s | .96x | Briev | MATCH |
| sparse_dispatch | .0487s | .0610s | .79x | Briev | MATCH |
| print_loop | .0357s | .0582s | .61x | Briev | MATCH |
| nbody_newton | 7.0379s | 8.4391s | .83x | Briev | MATCH |
| nbody_newton_accel | .8838s | .1321s | 6.69x | C | MATCH |
| nbody_sqrt | 2.2161s | 2.8773s | .77x | Briev | MATCH |
| nbody_sqrt_idio | 2.8023s | 3.7041s | .75x | Briev | MATCH |
| fasta | .2107s | .2113s | .99x | Briev | MATCH |
| fannkuch_redux | .0610s | .0661s | .92x | Briev | MATCH |
| mandelbrot | .6907s | .6743s | 1.02x | C | MATCH |
| kalman_filter_runtime | .1552s | .1795s | .86x | Briev | MATCH |
| knucleotide | .1908s | .1890s | 1.00x | ~tie | MATCH |
| cancel_math | .0528s | .0609s | .86x | Briev | MATCH |
| bit_clear | .0004s | .0002s | 2.00x | C | MATCH |
| queue_drain | .0348s | .0628s | .55x | Briev | MATCH |
| queue_drain_sym | .0354s | .0606s | .58x | Briev | MATCH |
| queue_drain_idio | .0351s | .0597s | .58x | Briev | MATCH |
| stack_push_pop | .0343s | .0598s | .57x | Briev | MATCH |
| interval_step | .0624s | .0612s | 1.01x | C | MATCH |
| telemetry_stream | .1945s | .1990s | .97x | Briev | MATCH |
| pid_control | .3431s | .3498s | .98x | Briev | MATCH |
| matrix_pipeline | .4619s | .6869s | .67x | Briev | MATCH |
| accumulator_flush | .1060s | .1430s | .74x | Briev | MATCH |
| sweep_sparse | .2216s | .1555s | 1.42x | C | MATCH |
| sweep_mid | .2590s | .2354s | 1.10x | C | MATCH |
| sweep_dense | .3976s | .2676s | 1.48x | C | MATCH |
| sweep_arr | .4093s | .3477s | 1.17x | C | MATCH |
| series_converge | .0001s | .0003s | .33x | Briev | MATCH |
| global_lifetime | .0304s | .0692s | .43x | Briev | MATCH |
| deep_recursion | .0003s | .0002s | 1.50x | C | MATCH |
| arena_churn | .0871s | .0954s | .91x | Briev | MATCH |
| linked_list | 1.2316s | 1.7713s | .69x | Briev | MATCH |
| hash_ops | .8938s | 1.0219s | .87x | Briev | MATCH |
| hash_ops_idio | .0307s | .0587s | .52x | Briev | MATCH |
| enemy_swarm | .0969s | .1244s | .77x | Briev | MATCH |

**Watch list:** no runtime benchmark hot-loops a `coll` push (queue_drain /
stack_push_pop use `RingBuffer`; linked_list / hash_ops use hand-written
types). The guard adds one `icmp` + never-taken branch to every coll push;
risk of regression is concentrated in stdlib `iter_map` List appends (tests,
not runtime benchmarks). Expect MATCH across the board.

## 4. Work items

### 4.1 The guard branch in a synthesized member body (the blocker)

The statement emitter already handles `Statement::If` (`emit_stmt.rs:151,
1199`). The blocker is the **self-slot register cache** in
`emit_member_body` (`emit_expr.rs:2336`, cache at `:2402-2407`):
`last_val_temps`/`last_val_types` memoize last-assigned member names to
registers. A guard that stores a new `data` pointer inside the `if` branch
then reads `data` after the join would resolve the read to the branch-path
register — valid on the taken path, stale on the not-taken path (the phi
problem).

**Fix (reload-after-branch, no phi needed):** after emitting a guard branch
that assigns self-slots, invalidate the cached registers for every member
name written inside the branch, so the post-branch read reloads from the
self-slot store. Implement a small `invalidate_self_slot_cache(branch_writes)`
called from the member-body `Statement::If` path (or invalidate the whole
self-slot cache on the join — correct and simpler; the guard is cold).

**Safety:** the grow path reallocates the buffer. Verify the emitted
realloc-or-copy orders the copy **before** the old-buffer release (the arena
grow path precedent is `emit_toplevel.rs:482`). Add a Kani harness for the
guard logic (old-cap / new-cap / pointer-copy / free ordering are
safety-critical).

### 4.2 Default `op Grow`/`op Shrink` strategy bodies

- Default `Grow(#Lh)`: `if len == cap { Resize#(h, cap * 2); }` — doubling.
- Default `Shrink(#Lh)`: `if len < cap / 2 { Resize#(h, cap / 2); }` —
  halving when sparse (thresholds per coll plan §3.6).
- Confirm these are emitted as `operator_defs` strategy entries
  (`coll_scaffold.rs:365-395`) and that the scaffolded `InsertAt`/`ExtractFrom`
  dispatch through the strategy binding (`emit_strategy_fn_call`,
  `emit_stmt.rs:1793`): **binding wins** (an override replaces the default,
  else the default fires).

### 4.3 InsertAt grow-on-full

Synthesized `push` body becomes (coll obj, `HeapGrowable` only):

```briev
if len == cap { Grow(#Lh); }     // default doubling; override if bound
data[len] = val;
len = len + 1;
```

- `coll struct` (`InlineFixed`) is untouched: its `InsertAt` carries the
  `len < N` obligation — no grow, an insert past N is a precondition error.

### 4.4 ExtractFrom shrink-when-sparse

Synthesized `pop` body becomes (coll obj only):

```briev
v = data[len - 1];
len = len - 1;
if len < cap / 2 { Shrink(#Lh); }   // after the read
term v;
```

### 4.5 Interpreter parity (rule 4 — interpreter is the reference)

- `Capacity#(product)` = field count (already per §3.6).
- `Resize#`/`EnsureCap#`/`TrimCap#` on a product are no-ops (already per
  §3.6) — verify the arms exist (`7005f206` shipped the intrinsics).
- `InsertAt`/`ExtractFrom` interpreter arms append/pop freely; no capacity
  concept to model. Confirm a >16 push runs identically in interpreter and
  codegen (test).

### 4.6 Tests

1. **grow-on-full:** `coll obj` push 17 elements — all 17 present, `cap`
   doubled (32), no OOB. ASAN build of this program proves the pre-fix
   overflow is gone.
2. **shrink-when-sparse:** extract below `cap / 2` — capacity halves.
3. **override wins:** a custom `op Grow` (triple, `geometric_grow`) is called
   on the grow boundary, not the default doubling (extends `6a8a0653`).
4. **coll struct bound:** `InsertAt` past `N` is a precondition error — no
   grow, no OOB.
5. **interpreter parity:** the same >16 push program through the interpreter
   and the backend produces identical output.
6. Full suite green (`cargo test --lib`).

### 4.7 Benchmarks (rule 11/11b)

Re-run `bash benchmarks/build_and_bench.sh --runtime` after the change and
compare against §3. Baseline worktree `../briv-compiler-baseline` for any
non-MATCH ratio: `bash benchmarks/compare_baseline.sh <name>`.

## 5. Docs

- SPEC §8.10/§15.2, `learn-briev/05-data-types.md` already state the
  normative behavior (commit `4dfffb88`).
- `docs/plans/2026-08-15-coll-length-semantics.md` §12 "future slice"
  statements are historical records — left untouched, referenced, not edited.
- `src/backend/llvm/coll_scaffold.rs:165-169` rationale comment is rewritten
  (never deleted) to state the implemented trigger.

## 6. Verification checklist

- `cargo test --lib` green before commit.
- `praetor validate --warn --target src/backend/llvm` — no new diagnostics.
- Kani harness for the grow guard (pointer copy/free ordering).
- Benchmarks MATCH vs §3 baseline; any regression resolved via baseline A/B.
- ASAN run on the >16 push program proves the OOB is closed.

## 7. Roadmap (subsequent legs, in order)

0. **Frontend grow-guard pass (the queue_drain_idio regression fix)** — see
   §8. Hoist the inlined grow call to the batch-loop's cold guard block or
   eliminate it when the frontend proves the coll's length stays below
   capacity across the loop.
1. **Coll-struct construction** — list-literal→`Int[N]` coercion so
   `coll struct` constructs from literals (prerequisite for const generics;
   unblocks the SPEC §8.10 example end-to-end).
2. **Const generics** — `coll struct Fixed<T,N> { data: T[N] }` per SPEC §8.10
   (stays normative; spec-outlined is work-to-do, not a spec edit).
3. OPEN BUGS.md: stdlib `iterator.bv`/`hashmap.bv` never compile; iterable
   slice-6 deletions.
4. Fundamentals-as-types with `Data` as reflective floor (decision recorded).

## 8. Implementation results (2026-08-15)

**Implemented (verified end-to-end):**
- Grow-on-full: a 21-push coll obj produces count 21, capacity 32 (doubled),
  sum 210 (0..=20, no element loss, no OOB) — the pre-fix code wrote past the
  16-slot buffer.
- The grow guard is `if len == cap { Resize#(#Self, cap*2) }` BEFORE the
  store; the runtime `__briev_coll_resize` (malloc + copy `min(len, cap)` +
  free + in-place slot update) mutates the block in place, so the post-guard
  `data` read reloads the possibly-new pointer from the slot — **no register
  merge across the guard branch**, sidestepping the coll plan §3.6 phi-merge
  blocker.
- `Resize#` now routes through the runtime `__briev_coll_resize` — the old
  inline emission malloc'd fresh WITHOUT copying or freeing (latent data-loss
  bug for explicit `Resize#` calls). `EnsureCap#`/`TrimCap#` already did.
- `#Self` resolves in member bodies to the boxed-self handle (§15.1).
- **Phi bug fix:** the generic `Statement::If`/`Guarded` emitters now record
  their merge label in `fun.cur_block`. The countdown loop's latch phis key
  their body predecessor on `cur_block`; a guard `if` inside an inlined member
  body split the loop body, leaving the phi's predecessor list mismatched with
  the CFG (invalid IR that crashed clang's LoopDeletionPass). Pre-existing
  latent bug — the grow guard was the first `if` in a countdown body.
  `opt -verify` now passes.

**Auto-shrink removed (drain fix).** Auto-shrink-when-sparse resizes on EVERY
pop in a drain pattern (`<- q; q <- x` keeps len ≤ 1 < cap/2, so the shrink
fires each iteration — an allocate/copy/free per iteration). It regressed
queue_drain_idio catastrophically and its opaque in-loop call triggered the
clang crash. Shrink stays explicit: `TrimCap#` + a declared `op Shrink` binding
(handle-only validation kept; nothing auto-invokes it). SPEC §8.10 mandates
grow-on-full only.

**Known regression: queue_drain_idio 0.58x → 4.00x.** The opaque resize call
in a coll-push hot loop blocks LLVM's loop optimization (the batch-loop inner
loop can no longer be if-converted; the call is a `memory(readwrite)` barrier).
Tested structural alternatives — `cold` declare (0.53s), helper function
(0.52s), `memory(argmem)` ptr declare (0.52s), sibling cold-block with
duplicated store (0.75s), `llvm.expect` cold marking (crash) — none restore
the baseline 0.0351s; the branch alone (no call) is free (0.03s), so ANY call
in the loop CFG is the cost. queue_drain_idio still MATCHES (correctness); all
C-comparable benchmarks (queue_drain, queue_drain_sym, stack_push_pop — fixed
RingBuffer/Stack, not coll) are unchanged at 0.55-0.61x. The principled fix is
roadmap item 0.
