# Plan: `Error#` intrinsic, Fix 1 + Fix 2, PiggyBank, HashMap completion

**Date:** 2026-08-17. **Head commit:** `b9fbc3e3`.
**Build/test:** `cargo build --release` / `cargo test --lib` /
`bash benchmarks/build_and_bench.sh --runtime` / `./target/release/brievc check <file>`.
**SHIPPED 2026-08-18** (Phases A–F; commits `cebd4b8d` → `7f20a2cc` +
Phase E `db0bc869`, Phase F follow-ups).

## Context

The HashMap tuple core (P1-P3) and the `when`-guard clang crash (P4) shipped.
Three pre-existing compiler bugs block the rest, and the user wants a new
collection (`PiggyBank`) both as a language-universality test and as a debug
vehicle. Two new language capabilities land: a compile-time `Error#` intrinsic
and its use to make a PiggyBank's sealed ops fail at compile time with
helpful messages.

## Phase A — `Error#(msg)` compile-time intrinsic

**Semantics (user-specified):**
- `Error#("msg")` is a COMPILE-TIME failure. A REACHABLE error means the
  program does NOT compile (the message is the diagnostic).
- If the compiler can PROVE the call is never triggered (a member never
  invoked from live code; a provably-dead branch) → it is unreachable, dead
  code, eliminated; the program compiles.
- If it can PROVE it's triggered (a live call site reaches it) → hard error.
- Unprovable (runtime-conditioned path) → conservative: fail (a runtime
  `Error#` would be unexpected; the intrinsic has no runtime meaning).

**Implementation:**
1. `src/intrinsic_signatures.rs`: add `ReturnKind::Never` (diverging) and
   register `"Error#"` with it. A member `defn f(x) -> K { Error#("..."); }`
   typechecks without a trailing `term` (Never is assignable to any return).
2. `src/typechecker/mod.rs`:
   - In `infer_call`, handle `Error#`: validate the arg is a `String` literal
     (or const); return `Type::never()`/void.
   - Record a `pending_compile_errors: HashMap<String /*member/fn*/, String /*msg*/>`
     when a body statically reaches an `Error#` (skip provably-dead branches:
     constant-false conditions, statements after a `term`).
   - Promote the pending error to a hard `TypeError` when a call/method/op
     dispatch resolves that member (resolve_method_call /
     infer_generative_op_call / the call path) — i.e., the error is
     usage-gated. A PiggyBank declares its error-ops but never invokes them →
     compiles; invoking CopyFrom/At/Count/Iter on it → compile error.
3. No backend emission — the compile fails before codegen; a dead `Error#` is
   DCE'd like any unreachable statement.

**SHIPPED 2026-08-17 (commits `cebd4b8d` → `68ce2ecc`).** The pending-error
store lives on `TypeUniverse` (`pending_member_errors: Mutex` — RefCell broke
the interpreter's static OnceLock; a manual `Clone` was added since Mutex
isn't Clone). Two implementation refinements surfaced during verification: (1)
the member-body mctx and the call-site ctx must share a single store — the
TypeUniverse both borrow; (2) `check_program` must check obj MEMBER bodies
BEFORE `check_top_level` so a member's `Error#` is recorded before any call
site promotes it (the original order promoted against an empty store and a
invoked sealed-op wrongly compiled). Verified: top-level `Error#` → fails with
the message; a member with `Error#` NOT invoked compiles; the SAME member
invoked fails with the message. SPEC §18.6 documents the semantics.

**Verify:** `Error#("x")` in a called fn → `brievc check` errors with "x".
`Error#` in a never-called member → compiles. `Error#` after `term` / in a
constant-false `when` → compiles.

## Phase B — Fix 1: inlined member `foreach` register/cache collision

**Bug:** 3+ inlined probe members (each a `foreach` with loop-carried locals)
in one node body produce "instruction forward referenced" (registers used
before their def in `@main`) or silently wrong values (`get` returns 0 in a
hot loop). Root: the countdown per-field register cache (`last_val_temps`,
`pending_phi_backedge`) leaks across inlined member `foreach` loops.

**Symptom status 2026-08-17 (P4 changed the failure mode — verify against the
LIVE symptom, not this text):** after the P4 fix (Foreach arm sets
`fun.cur_block`), the compile-time forward-ref is GONE for every repro shape
(all `.ll` emit, `llc` clean). The bug now surfaces at RUNTIME when the member
result is consumed INLINE. Empirically verified at HEAD (commit `ef2875e6`):

- `p5_3ins2get` (3 inserts + 2 gets, `println!(m.get(1))` inline) → **SEGFAULT**.
- `p5_core` (3 ins + `Count#` + 3 gets + 2 contains) → **SEGFAULT**.
- `p5_3ins1get` (3 ins + 1 get inline) → silent no-output (wrong value).
- `p5_getonly` (1 ins + 1 get) → correct `10`.
- `hash_ops_idio` hot loop (`sum = sum + m.get(i)`, BOUND=10M) → prints `240`
  twice; the C reference prints `24999995000000` / `99999990000000`.
- **TRAP: `pb_obs` (let-bound `let g1 = m.get(1)` + `endprogram __print_int`)
  already prints the correct `30`.** The bug hides behind a let-binding — the
  register-collision cache is read back through `last_val_temps` for an
  identifier. A verify that only uses the let-bound shape will falsely green.
  The Phase B acceptance MUST use an inline-consumption shape (printed operand,
  `sum = sum + m.get(i)`, a get result consumed directly in an FFI call).

**Fix:** scope the field cache correctly across member inlines within a
countdown body pass — check the save/restore at emit_expr.rs:2541/2655 and the
clear points at counter.rs:569/832/867; ensure a member's field writes do not
leave stale `last_val_temps` entries that a later statement reads as a register
whose def was re-emitted. **Notable asymmetry:** the A5d save/restore at
emit_expr.rs:2541/2655 covers `last_val_temps`/`last_val_types` only — no
member-inline boundary restores `pending_phi_backedge`. Verify that hypothesis
first (save/restore `pending_phi_backedge` at the same boundary, or key it by
scope); the hash_ops_idio `240` (vs C's trillions) and the inline-consume
segfaults are the litmus tests.

**Verify (inline-consumption shapes — the let-bound `pb_obs` shape ALREADY
passes and proves nothing):** `println!(m.get(1)); println!(m.get(2));` after 3
inserts in one node → `10`, `20`, no segfault. `sum = sum + m.get(i)` hot loop
(`p4_verify.bv` shape, BOUND short enough to cross the `when i % 5000000 == 0`
print) → sum MATCHes the C reference. hash_ops_idio hot loop (BOUND=10M) →
`24999995000000` / `99999990000000` → re-add to the suite.

**STATUS 2026-08-17 (implemented):** SSA-reactor path FIXED; countdown path OPEN.

The `pending_phi_backedge` save/restore hypothesis was TESTED and REFUTED — the
raw `.ll` is SSA-clean and label-clean (per-function register-dup scan = 0;
`llc` accepts every shape). The collapse is clang -O3 (LTO): pass traces show
every pass through ADCE retains the body, TailCallElim creates 13 tail calls,
the loop fully unrolls (16x, per-iteration phi chains), and a later pass
collapses `@main` to print-first + ret (`m2_clang_passes.log`). `opt -O3`
alone KEEPS the body. Empirical rule: `FAIL ⟺ (≥2 in-body member-probe
consumptions) OR (1 get + ≥2 fresh in-body constant tuple mallocs)`. A
`.ll`-level A/B through the exact harness link proved that relocating the FULL
tuple materialization (malloc + count header + element stores + ptrtoint) to
the loop preheader fixes it; relocating only the malloc (or the ptrtoint) does
NOT — the stores must leave the loop.

**Fix shipped (SSA path):** `emit_tuple` defers a CONSTANT-element tuple
literal to `pending_struct_allocas` when `defer_struct_allocas` is set
(emit_expr.rs, with a literal-only `is_compile_time_const` guard so body-defined
registers never dominate a preheader); `emit_ssa_main` buffers the reactive
loop and flushes before `.ss_main_loop` (ssa.rs), mirroring the countdown
path's loop_buf + flush (2026-08-13 struct-literal fix, counter.rs:590/591).

Verified at HEAD+fix (all via inline-consumption shapes):
- `p5_3ins2get` → `10 20` ✓ (was SEGFAULT)
- `p5_core` → `3 10 20 30 true false` ✓ (was SEGFAULT)
- m-grid: m1, m2, m2_letget, m1_2get, vB, m3i_marker all print full marker
  sequences ✓ (m2 was `1000`-only)
- `cargo test --lib` 1893/1893 green.

**OPEN (countdown path):** `hash_ops_idio` still collapses — the hot tuple
`(i, i * 2)` is DYNAMIC, so the deferral cannot move it (domination). The
countdown path needs a per-iteration preallocated slot or a tuple-slot ABI that
drops the in-body `inttoptr + GEP` round-trip; A/B against the harness link
before building. `vA_2get` (let-bound inserts + gets-only observable) segfaults
even pre-fix: the inserts emit into the unused alwaysinline `@txn_go` copy, not
`@main` — pre-existing, out of scope here.

**UPDATE 2026-08-17 (diagnosis — the countdown failure is NOT a collapse):**
a full LTO pass trace of `hash_ops_idio` (8178 pass dumps) shows the
`__print_int` NEVER leaves the IR; the final `@main` is semantically complete
(fully unrolled 16-slot probe). The `240` output is a runtime EMISSION bug: the
HashMap `init`'s nested `let init_items: List<(K, V)> = []` wrote the List's
hidden `cap = 16` through the ENCLOSING `HashMap`-prefixed context into the
MAP's `cap` column — clobbering `cap = 256` to 16 (10M inserts into a 16-slot
table → read-back sum 240). Same double-store in both `@init_state` and `@main`.
Small-key repros (p5, m-grid, keys < 16) were insensitive to cap 16 vs 256, so
the grid never exposed it.

**Fix shipped (cap column clobber):** `emit_member_body`'s boxed-self branch
clears the leaking `self_prefix` for the body duration — boxed coll members
resolve bare names against their OWN receiver, never the enclosing instance's
columns (emit_expr.rs). Verified: hash_ops_idio 240 → 65280 (= 2·Σ0..255, the
exact read-back for a correctly-256-cap map that fills at count==cap).

**Remaining gap (this session):** even with cap correctly 256, the map fills at
256 entries (`insert` refuses at `count < cap`) while the C reference is a
direct O(1) write (`keys[i % CAP] = i; sum += vals[i % CAP]`, output ≡ Σ2i).
And the map's linear-probe `foreach q in 0..cap` scans the FULL cap per
get/insert (no early exit — the language has no `break`). Resolution: (a) add a
bare `break` early-exit to `foreach` (search-until-found is a condition exit,
per SPEC "counted iteration uses … reactive/transactional structure" — `break`
is an exit FORM of `foreach`, not a for/while/loop keyword); (b) stdlib
`HashMap.init` honors a capacity arg (0 → 256) and `insert`/`get`/`contains`/
`remove` `break` on the matched slot; (c) `hash_ops_idio` uses `2 * N` capacity
so it never fills, giving O(1) probes and output == C (Σ2i).

## Phase C — Fix 2: arrow-push double-construction + member-field push

**Bugs:** (a) `acc <- keys[i]` into a loop-carried local List in a member
body emits two `[]` blocks; the push writes a copy, `term acc` reads the
stale original (scans return 1 of N). (b) `items <- e` on a MEMBER-FIELD
List in a member body hits `%t432` undefined (register collision).

**Fix (as-built 2026-08-18):** the ACTUAL root of (a) was the CALLER side —
`let ks: List<K> = b.keys()` routed the returned (already-a-List) collection
through the seed constructor and wrapped it as `[<list>]` (len forced to 1).
`construct_local_collection_seed` now takes an emitted `TypedRegister` and
binds a collection-valued RHS DIRECTLY; only a genuine non-collection becomes
a seed. Bug (b) was the strategy lookup + plain-copy fallback recognizing only
BARE names while a pooled slot is keyed `{prefix}.{name}` with the COLUMN type
`Vector(inner, [Anonymous(1)])` — centralized `collection_base_type_name`
(emit_toplevel.rs) resolves locals / state fields / self-prefixed member names,
peeling the Vector wrapper; arrow stores into pooled member targets write the
instance column via a GEP-into-element store (`emit_state_store_self_slot`).

**Verify (as-built):** repro prints `3`/`3` at -O0 and -O3;
`test_arrow_push_binds_returned_list_and_pooled_member_field` (asserts no
constant-1 len seed, exactly 8 push increments, printed values are loads).
1898 lib tests green.

**Perf follow-up (as-built):** fixing (b) exposed that the HashMap's persistent
`items: List<(K, V)>` mirror (a 2026-08-16 workaround for bug (a)) was being
silently DROPPED at Phase B — hash_ops_idio's 1.09x was measured with no mirror
writes. With the push fixed the mirror cost 2 mallocs/insert (3.28x). Since
bug (a) is now fixed, the mirror is DEAD (no reader) and was DELETED:
keys()/values()/entries()/foreach will scan the columns directly in Phase E
(the pre-workaround design). hash_ops_idio re-measured 1.06x MATCH with no
dropped work.

## Phase D — `PiggyBank<K>` (one-shot, opaque, ExtractFrom, self-free)

In `lib/std/collections.bv`:

```
obj PiggyBank<K> {
    items: List<K>;
    op InsertAt: put(#Lh, #Rh);           // the ONLY way in: `piggy <- x`
    op ExtractFrom: smash(#Lh);           // the ONLY way out: `~<- piggy`
    op Init: init(#Lh, #Rh);
    op CopyFrom: read_error(#Lh, #Rh);    // `x <- piggy` → compile error
    op At(i: Int): at_error(#Lh, i);      // `piggy[i]` → compile error
    op Count(): count_error(#Lh);         // `piggy.Count#()` → compile error
    op Iter(): iter_error(#Lh);           // `foreach` → compile error

    txn init(v: K) { let e: List<K> = []; items = e; }
    defn put(e: K) { items <- e; }        // member-field push (Fix C)
    defn smash() -> List<K> {             // returns ALL, self-frees
        let all: List<K> = items;
        let e: List<K> = [];
        items = e;
        free items;                       // one-shot: backing freed
        term all;
    }
    defn read_error(e: K) -> K {
        Error#("a PiggyBank is opaque — individual elements cannot be read out. Smash it to extract everything at once: `let all: List<K> = ~<- piggy;`");
    }
    defn at_error(i: Int) -> K {
        Error#("a PiggyBank is opaque — there is no indexing into a sealed jar. Smash it to extract everything at once: `let all: List<K> = ~<- piggy;`");
    }
    defn count_error() -> Int {
        Error#("a PiggyBank is opaque — you cannot count what is inside without smashing it. Extract everything at once: `let all: List<K> = ~<- piggy;`");
    }
    defn iter_error() -> Int {
        Error#("a PiggyBank is opaque — a sealed jar cannot be iterated. Smash it to extract everything at once: `let all: List<K> = ~<- piggy;`");
    }
}
```

- **One-shot:** `smash()` drains, resets the slot, `free items` marks the
  field consumed — a second `put` is a use-after-free compile error. No
  drain-and-reuse.
- **Decoupling proof:** `Count#()` dispatches through the declared `op Count`
  (→ `count_error`), never implicitly reading the jar's internal length.
- **Verification:** `piggy <- 1; piggy <- 2; piggy <- 3;` then
  `let all: List<K> = ~<- piggy;` → `[1,2,3]`, jar self-freed. Each sealed-op
  usage (`x <- piggy`, `piggy[0]`, `piggy.Count#()`, `foreach`) → compile
  error with the message. A second `put` after smash → use-after-free error.

## Phase D — `PiggyBank<K>` (one-shot, opaque, ExtractFrom, self-free)

**As-built 2026-08-18:** shipped in `lib/std/collections.bv`. The sealed ops
are declared inline as op-as-member error bodies (`op At(i: Int) -> K { Error#
(…); }`) plus a CopyFrom binding (`op CopyFrom: read_error(#Lh, #Rh)`) —
the param'd op BINDING form (`op At(i): fn(…)`) doesn't parse, and a CopyFrom
op-as-member would shadow the ExtractFrom binding in the arrow dispatch.

Five pre-existing compiler gaps fixed (BUGS.md 2026-08-18): consume-aware
arrow op selection (CopyFrom for `<-`, ExtractFrom for `~<-`), zero-param
extract-op rule, pooled-instance strategy resolution, `{type}.{member}`
pending-error keys, and sealed At/Iter promotion through Index/Foreach syntax.

**Verify (as-built):** `tests/tier1/test_piggybank.bv` → `3`/`6`/`0` at -O0 and
-O3; each sealed op (`x <- piggy`, `piggy[0]`, `piggy.Count#()`,
`foreach x in piggy`) → compile error with the message. Regression test
`test_arrow_consume_selects_copyfrom_vs_extractfrom` pins the arity rule (fails
with the `@i` undefined bug otherwise). `queue_drain_idio` re-expressed as
`~<- queue` (a drain is destructive).

## Phase E — HashMap completion

1. Restore `keys()`/`values()`/`entries()` scans (Fix C unblocks them).
2. `foreach p in es` (bound `entries()` List) → normal List iteration.
3. Migrate `insert`/`remove` to `count += 1` / `count -= 1` (demos
   compound-assign in members).
4. Re-add `hash_ops_idio` to `benchmarks/build_and_bench.sh` (Fix B makes its
   hot loop correct); confirm MATCH.

**Verify:** full HashMap surface with 3+ member calls in one node (insert/get/
contains/remove/Count#/keys/values/entries/foreach) all correct; hash_ops_idio
MATCH.

**As-built 2026-08-18:** keys()/values()/entries() are column scans
(`foreach i in 0..cap { when occupied[i] == 1 { acc <- keys[i] } }`); the
HashMap `items` mirror list is gone (deleted in Phase C). insert/remove use
`count += 1` / `count -= 1`. `hash_ops_idio` re-verified 1.06x MATCH.
`tests/tier1/test_hashmap_surface.bv` exercises the full surface in one node
(prints 20/true/false/5/10/100/10/10/4/9).

**Two pre-existing compiler bugs fixed on the way (both BUGS.md 2026-08-18):**
- FFI guard outlining: a guard body with member calls/state writes was outlined
  into a `txn_*_cold_*` function that has NO `%state` param — the inlined
  member bodies (pooled-column reads) referenced the undefined `%state`
  (compile error). Outline is now restricted to pure scalar-read guard bodies
  (`println!(sum)`-style); everything else stays inline.
- Foreach item / member-destructure register poisoning (new BUGS.md entry):
  the loop-variable binding leaked out of the foreach into `last_val_temps`,
  and `clear_locals` didn't clear that map, so the SSA-main replay's
  `let (k, v) = e` resolved `k` to a stale/forward register — wrong values and
  a clang -O3 -flto SIGSEGV. Fixed by scoping the item to the body and clearing
  `last_val_temps` at pass boundaries.

## Phase F — Tests + docs

- Tests: `Error#` reachable/dead; PiggyBank (drain, one-shot use-after-free,
  each sealed-op compile error); HashMap full surface; tuple regressions.
- Docs: plan SHIPPED; BUGS.md close Fix 1 + Fix 2 entries (add the Error#
  intrinsic note); SPEC (§17.1: `Error#`; PiggyBank as a one-shot opaque
  op-surface collection; `Count#`/field decoupling).
- `cargo test --lib` green; stdlib `brievc check` clean; Praetor no new
  diagnostics; benchmarks all MATCH (hash_ops_idio included).

## Execution order

A → B → C → D → E → F. Commit after each phase that leaves the suite green.
