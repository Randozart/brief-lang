# Plan: `Error#` intrinsic, Fix 1 + Fix 2, PiggyBank, HashMap completion

**Date:** 2026-08-17. **Head commit:** `b9fbc3e3`.
**Build/test:** `cargo build --release` / `cargo test --lib` /
`bash benchmarks/build_and_bench.sh --runtime` / `./target/release/brievc check <file>`.

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

**Fix:** scope the field cache correctly across member inlines within a
countdown body pass — check the save/restore at emit_expr.rs:2541/2655 and
the clear points at counter.rs:569/832/867; ensure a member's field writes do
not leave stale `last_val_temps` entries that a later statement reads as a
register whose def was re-emitted.

**Verify:** 3 inserts + 2 gets in one node → correct values. hash_ops_idio
hot loop → correct sum → re-add to the suite.

## Phase C — Fix 2: arrow-push double-construction + member-field push

**Bugs:** (a) `acc <- keys[i]` into a loop-carried local List in a member
body emits two `[]` blocks; the push writes a copy, `term acc` reads the
stale original (scans return 1 of N). (b) `items <- e` on a MEMBER-FIELD
List in a member body hits `%t432` undefined (register collision).

**Fix:** the foreach pre-declaration (emit_stmt.rs:1396-1409) must seed the
loop-carried local from the ACTUAL binding handle (no re-`[]`); the `<-`
push (emit_stmt.rs:1033) must write its result back through the binding. Fix
both the loop-local and member-field paths.

**Verify:** `keys()`/`values()`/`entries()` counts correct. A member-field
`items <- e` (PiggyBank `put`) works.

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
