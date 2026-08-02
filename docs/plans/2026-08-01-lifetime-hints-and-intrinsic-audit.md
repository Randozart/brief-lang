# Lifetime Hints, Intrinsic Audit, and C-Surface Reduction

**Date:** 2026-08-01
**Status:** Superseded by the master plan —
[2026-08-01-consumptive-operators-lifetime-and-c-surface.md](2026-08-01-consumptive-operators-lifetime-and-c-surface.md).
**Worktree:** `brief-compiler-cwm` (`feat/collections-watchdogs-memory`)

**Relationship (plan map):** the original Phases 1–4. Phase 1 (docs/highlighter)
is DONE. Phases 2–6 (intrinsic audit, consumptive operators, stream symbols,
free-check, C-surface) are folded into the master plan, which records the design
research (C independence, the intrinsic criteria, the garbage-scheduler
free-check, `#StdIn`/`#StdOut`/`#StdErr` stream symbols, the trigger-port
removal) with the full rationale. Also related:
[2026-08-01-global-lifetime-design.md](2026-08-01-global-lifetime-design.md)
(garbage scheduling) and
[2026-07-31-collections-watchdogs-memory.md](2026-07-31-collections-watchdogs-memory.md)
(the `<-`-op syntax that Phase 3's arrow rewrite changes).

## Goals

1. **Docs/highlighter** — the syntax highlighter stops special-casing `word!`
   and treats `?`/`!` as standalone markers (same scope as `@`/`&`); the SPEC
   watchdog grammar and the collections plan's stale note are corrected.
2. **Intrinsic audit** — one generic `Print#` (type-dispatched emission) replaces
   the four print intrinsics; the dead `AllocArena#`/`Realloc#` vestiges are
   removed; `Malloc#`/`Alloc#` stay the universal allocation primitives; native
   LLVM mapping is extended where the C runtime is underused. Two real bugs are
   fixed: the silent Implicit Int×Float bitcast, and the frgn String declare
   SSO mismatch.
3. **Free-check** — the garbage scheduler's unprovable tail is reclaimed by a
   sound runtime refcount at the *edge-of-use* checkpoint; the `free`/`keep`
   body-annotation keywords let the programmer fill proof gaps with verified
   contracts; `briefc memcheck` reports every unprovable point and the hint that
   closes it.
4. **C-surface reduction** — the `.bv`/`.ebv` split (Rust's core/alloc/std
   layering, in Brief's file-extension terms): logic moves to `.ebv` pure Brief,
   `.bv` keeps only the genuine OS-syscall shim, the target picks the variant.

## Design decisions (settled with the user)

- **`free`/`keep` are body annotations** (statements marking a point in a
  txn/node body), not sigils or intrinsic spam.
  - `free x;` — "beyond here x is dead". The scheduler verifies the read-set
    after the annotation is empty; a LATER read is a **compile error (refuse the
    hint)**. On verification the free is emitted at that point.
  - `keep x;` — "beyond here x must stay alive". Suppresses a scheduled free; a
    provably-dead-after `keep` is a **warning (redundant keep)**, not an error.
  - Both are FRONTEND (scheduler) directives — a backend that doesn't free (a
    GC target) simply doesn't emit; no parse rejection, no backend-agnosticism
    conflict. They never make code faster (a resource/lifetime contract that
    fills a proof gap; if the scheduler already proves the free, the keyword
    adds nothing).
- **One generic `Print#`** — the compiler (plugin/typechecker) dispatches
  int/float/char/str emission by the argument type; other types cast first.
- **`.bv` vs `.ebv`** — the same stdlib API has two target variants: `.bv`
  (OS/libc-backed — maps to OS functions) and `.ebv` (embedded/no-OS — pure
  Brief logic). The import resolver prefers `.bv` on OS targets and `.ebv` on
  freestanding targets.

## Audit criterion (intrinsics vs stdlib)

- Works under `--no-stdlib` → **intrinsic**.
- Expressible in stdlib syntax (bits model, reflection, op bindings) → **`.bv`/`.ebv`**.
- N special-cased intrinsics with one dispatch decision → **one generic intrinsic**.

---

## Phase 1 — Docs/highlighter

### 1.1 `syntax-highlighter/syntaxes/brief.tmLanguage.json`
- REMOVE the `"macro-words"` repository (`\b[a-zA-Z_][a-zA-Z0-9_]*!` →
  `entity.name.tag.macro-call.brief`) and its `"include": "#macro-words"`.
- REMOVE the `"macros"` repository (`\b(entry|args|print|println|get_env|get_env_int)!` →
  `keyword.control.macro.brief`) and its `"include": "#macros"`.
- ADD standalone `?` and `!` operator rules after the existing `@`/`&` rules,
  same scope name (`storage.modifier.ownership.brief`). Rule order keeps
  `!=` and `~?` winning over the standalone chars.
- ADD the missing keywords `seq`/`vol`/`sync` and the intrinsics
  `Now#`/`Malloc#`/`Alloc#`/`Free#`/`Load#`/`Store#`.

### 1.2 `syntax-highlighter/syntaxes/dbrief.tmLanguage.json`
- ADD the standalone `?`/`!` rules after `@`/`&` (mirroring the dbrief
  convention). No word-bang to remove (verified absent).

### 1.3 `spec/SPEC.md`
- Update the watchdog BNF (§2.5):
  `watchdog ::= ("?" | "!") "[" expression "]" ("within" integer unit)? ("->" identifier "(" identifier? ")")?`
  and the prose (liveliness, `-> handler(val)`, `within N ms/cyc/seconds/minute`).

### 1.4 `docs/plans/2026-07-31-collections-watchdogs-memory.md`
- Correct the stale "Deferred: fuel/time" note — the `within N <unit>` deadline
  + `Now#` are implemented.

**Verify:** `cargo test --lib` green; the highlighter JSON parses; the examples
snippets type-check.

---

## Phase 2 — Intrinsic audit + the two bug fixes

### 2.1 Generic `Print#`
- Merge `PrintInt#`/`PrintFloat#`/`PrintChar#`/`PrintStr#` into one `Print#`
  intrinsic. The print plugin's `PrintKind` dispatch (already type-based)
  drives the emission (int→`__print_int`, float→`__print_float` + the
  float-cache unbox, str→`__print_str`, char→`__print_char`). Update
  `intrinsic_signatures.rs`, the `intrinsics.rs` emission, `print_plugin.rs`,
  and every test/benchmark reference.
- Other types print by casting first (the user's rule).

### 2.2 Dead vestige removal
- Remove the `AllocArena#`/`Realloc#` name-match arm from
  `src/analysis/allocation.rs` (the arena need is strategy-driven via
  `default_strategy()`).
- Remove `AllocArena#` from `src/analysis/global_lifetime.rs` heap-alloc
  detection (keep `Malloc#`/`Alloc#`).

### 2.3 Fix: implicit Int×Float coercion silently bitcasts (BUGS.md:2730)
- The typechecker must reject implicit Int↔Float arithmetic in mixed binary
  ops (or coerce via `sitofp`). Contract-first: `(count % 101) * 0.5` becomes a
  compile error demanding an explicit `as Float`. Add a test.

### 2.4 Fix: frgn String declare SSO mismatch (BUGS.md:2779)
- Make `protocol_llvm_type` honor the `feature_sso_strings` gate so the frgn
  `declare` agrees with the call site in both configurations. Additive; verify
  `float_math` still builds and its declare/call match.

### 2.5 Native-LLVM mapping audit
- The already-mapped `Copy#`→`llvm.memcpy`, `Fill#`→`llvm.memset`, math→`llvm.*`
  stay. Audit the remaining C calls in `intrinsics.rs`; `Length#`/`Concat#`
  move to stdlib/`.ebv` in Phase 4 (noted here, not done).

**Verify:** full suite + full harness zero MISMATCH; `float_math` declares/calls
agree; a mixed Int×Float expression errors with a clear message.

---

## Phase 3 — Free-check: `free`/`keep` + refcount + `memcheck`

### 3.1 Keywords + parser
- Lexer: `Free`/`Keep` tokens. Parser: body-annotation statements
  `free <identifier>;` / `keep <identifier>;` (like `term`).
- AST: `Statement::FreeHint(String)` / `Statement::KeepHint(String)`.

### 3.2 Scheduler semantics (`analysis/global_lifetime.rs`)
- `free x;` in a txn body → the field is a free candidate AT that point: the
  scheduler verifies no later read (across all ordered txns) → emits the free
  there; a later read → compile error.
- `keep x;` → the field is excluded from scheduling (a scheduled free is
  suppressed); if the field is provably dead after the keep → warning.
- Manually-freed fields stay excluded (no double-free).

### 3.3 Refcount free-check (the unprovable tail)
- For heap fields the static proof cannot close (an unordered reader exists),
  the scheduler inserts a refcount at the edge-of-use checkpoint: the compiler
  emits inc/dec on address copies (reusing provenance), and at the checkpoint a
  zero count → `__brief_free`. Sound (no premature free — only freed when no
  live reference remains).
- The checkpoint is the LAST POSSIBLE consumer boundary, not an arbitrary point.

### 3.4 `briefc memcheck`
- A diagnostics subcommand that reports every unprovable heap field, its
  possible readers, and the exact `free`/`keep` hint that would close the gap.
- The hints the scheduler verifies (a `free` with a later read is an error).

### 3.5 Design doc
- `docs/plans/2026-08-01-free-check.md`: the refcount mechanism, the
  `free`/`keep` annotation semantics (error vs warning), the memcheck UX, the
  correctness contract (no premature free), and the backend-agnosticism note.

**Verify:** a `free` hint with a later read errors; a `keep` on a provably-dead
field warns; a program with an unprovable field + a correct `free` hint matches
C; `briefc memcheck` lists the unprovable points.

---

## Phase 4 — `.bv`/`.ebv` C-surface reduction

### 4.1 Runtime inventory (committed as its own step)
- Classify every `brief_rt.c` function as **syscall shim** (stays C) vs
  **logic** (moves to `.ebv`): the digit/float formatting
  (`__print_int`/`__print_float` itoa/ftoa), string ops
  (`brief_str_eq`/`band`/`bor`/`bxor`/`bnot`/`concat`/`len`), the allocator.
- Produce the classification table in the architecture doc.

### 4.2 `.ebv` variants
- Introduce `lib/std/*.ebv` implementations of the formatting/string/allocator
  logic in pure Brief (byte loops, digit conversion, a free-list allocator on a
  buffer). The `.bv` variants keep the OS/libc calls.
- The import resolver prefers `.bv` on OS targets; a freestanding target
  prefers `.ebv`.

### 4.3 No-libc target sketch
- A freestanding target design (raw syscalls `write`/`exit`/`mmap`/
  `clock_gettime` + a `_start` stub) documented in the architecture docs; not
  necessarily a full implementation in this phase.

**Verify:** the stdlib `.bv` programs still build (the `.bv` default unchanged);
at least one logic function (e.g. integer printing) has a working `.ebv`
implementation that builds and prints identically.

---

## Verification (every phase)

- `cargo test --lib` green before each commit.
- `cargo build --release` — no new warnings.
- Full harness `bash benchmarks/build_and_bench.sh --runtime` — zero MISMATCH,
  zero precomputed flags.
- Praetor on changed directories.
- Architecture docs updated in the same commit as the structural change.

## Out of scope (parked)

- Chained-bucket HashMap (the flat form works; the chained end-state is a
  standalone follow-up).
- The Rust `compiler_builtins` equivalent (wide-int/memcpy libcalls) — Brief's
  integer ops are native-width i64, so the surface is minimal; deferred to the
  no-libc target work.
