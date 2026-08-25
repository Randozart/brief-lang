# SPEC Sync + Language Features — Final Plan

**Date:** 2026-08-23
**Status:** FINAL — incorporates all owner decisions from this session
**Scope:** spec/SPEC.md updates + new language features + implementation corrections

---

## Part 1 — SPEC.md Updates (6 changes)

### Change 1 — §8.3 enum: variant construction

**Location:** after the enum declaration example (line ~528)

**Add after the existing text:**

Variants are constructed by calling the variant name as a function:

```briev
term Ok(a / b);
term Err("division by zero");
```

Zero-payload variants (declared without parens) have Void payload and construct with zero arguments:

```briev
Null()
```

Multi-payload variants accept positional arguments stored as a Tuple payload. User-defined fns shadow variants — if a `defn` shares a name with a variant, the function wins. Variant names must be unique across all enums for bare construction to be unambiguous. The typechecker binds the enum's type parameters positionally from payload arguments (`Ok(5)` under `Result<T,E>` binds T=Int); remaining params unify against the contextual expected type.

### Change 2 — §11.3 match: block-expression arm bodies + term/bare-tail split

**Location:** after the patterns bullet list (line ~1057)

**Add:**

Arm bodies may use block expressions with statements and a tail value:

```briev
match res {
    Ok(val) => {
        let doubled = val * 2;
        doubled
    }
    Err(msg) => { Print#(msg); 0 - 1 }
};
```

A trailing expression without `;` is the block's implicit value. Zero-payload variant patterns use `Variant()` (with parens) to distinguish from variable bindings.

In `node` and `txn` bodies, `term expr;` marks the firing checkpoint — the reactor evaluates the goal after this point. In `defn` bodies and match-arm blocks, a trailing expression WITHOUT `;` is the implicit return value. `term` signals "loop iteration complete"; a bare tail expression signals "this scope produces this value."

### Change 3 — §12.2 spawn/await: lazy execution model

**Location:** after the storage-class examples (line ~1290)

Add:

Spawn **captures** the function and its evaluated arguments but does NOT execute the body. Bodies are split at cancellation points into segments. The first `await` triggers round-robin segment execution of all non-Done tasks until the target reaches Done.

Deterministic interleaving at yield boundaries: tasks execute one segment per scheduling pass, in spawn order. Single-threaded scheduler — no data races, no nondeterministic ordering.

`free task` before any await prevents execution entirely (the body never runs). After await has started execution, free sets a cancellation flag checked at each yield boundary.

### Change 4 — §15.2 Operator classes: List concatenation

Replace "Concatenation has no dedicated ++" sentence:

List concatenation uses `+`: both operands must be List<T> with matching element types; it resolves as an intrinsic binding (`list_concat`). Other collection types resolve through an ordinary operation binding.

### Change 5 — §9.5 + §17: port ^Ready correction

§9.5 add port semantics subsection:

- **`port.^Ready`** → Bool — runtime reflection on the port's internal state flag. True when a pending event is observable.
- **`port.field`** → payload member projection. Falls through to the payload type's declared fields (`damage.amount` where damage is `Event<Damage>`).
- Output ports fire via ArrowAssign: `died <- value;` sets the shared slot's Ready flag and stores the payload. Wired consumers observe the same slot.
- Cells enforce sealing: external references to cell internals fail at compile time; only declared ports are externally visible.

§17 add Ready to runtime reflection targets list alongside Length/Ptr/Size/Bytes/Alignment/Type.

### Change 6 — NEW §10.x: `check` statement

New subsection after §10.2 (inline guards):

> ### Liveness checks
>
> ```briev
> check <expr>;
> ```
>
> A liveness check asserts that `<expr>` holds at this point in execution. It serves three roles:
>
> 1. **Compile-time proof**: if the solver proves `expr` from known facts (contracts, prior checks), the check is eliminated — zero cost.
> 2. **Compile-time rejection**: if the solver DISPROVES `expr`, compilation fails with a diagnostic explaining under which conditions the check would be violated.
> 3. **Runtime assertion**: for unprovable loops, the check evaluates at that point in execution. Failure triggers rollback (same as escape).
>
> After a successful check, the solver records `expr` as a known fact, strengthening downstream proofs and enabling further optimization.
>
> `check` may appear in any function body (`defn`, `txn`, `node`). In looping contexts, it evaluates every pass through that point. In non-looping defns, it documents and verifies the programmer's assumptions about the input domain.

### Change 7 — stdlib function mentions

Brief inline additions:
- `char_at(s: String, i: Int) -> Char` — character-level String access
- Process/environment intrinsics: `Spawn#`, `SpawnWithOutput#`, `SetEnv#`, `GetCwd#`, `ChDir#`
- `Barrier#()` — workgroup barrier

---

## Part 2 — Implementation Items

| # | Item | Files | Effort |
|---|---|---|---|
| I1 | `check` statement parsing | src/parser/statements.rs | Small |
| I2 | `check` typechecker (prove/disprove/defer) | src/typechecker/mod.rs | Medium |
| I3 | `check` interp eval (runtime assert + rollback on fail) | src/interpreter/eval.rs | Small |
| I4 | `check` LLVM lowering (branch to trap/rollback if false) | src/backend/llvm/emit_stmt.rs | Small |
| I5 | `.^Ready` reflect correction | typechecker + interpreter + examples | Small |
| I6 | Migrate result.bv/process.bv/etc to comma-except-last arm style | lib/std/*.bv | Trivial |
| I7 | Kani harnesses for check proof propagation | kani/ | Medium |

## Part 3 — Deferred (tracked, not this session)

- json.bv deep migration (Char indexing, slice parens — language gaps)
- Async Phase B (port event scheduler integration)
- Async Phase C (LLVM lowering for spawn/yield/await)
- Cell scheduling
- 5c LLVM dyn tables
- glue.dbv parser decision
- Tutorial chapters 11+ refresh
