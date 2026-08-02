# Master Plan: Consumptive Operators, Lifetime Hints, Intrinsic Audit, Stream Symbols, and C-Surface Reduction

**Date:** 2026-08-01
**Status:** Planned → executing
**Worktree:** `brief-compiler-cwm` (`feat/collections-watchdogs-memory`)
**Supersedes / consolidates:**
- `docs/plans/2026-08-01-lifetime-hints-and-intrinsic-audit.md` (the original Phases 1–4)
- `docs/plans/2026-08-01-global-lifetime-design.md` (garbage scheduling)
- `docs/plans/2026-07-31-collections-watchdogs-memory.md` (Phase D)

This document is the **authoritative record of the design decisions, the research
behind them, and the implementation plan.** The token shuffling, the arrow
rewrite, the stream symbols, and the trigger-port removal all have rationale
recorded here so nothing is ambiguous.

---

## Part I — Design research (the "why")

### 1. C independence — how far, and why

Brief compiles to LLVM IR; the compiler itself is Rust. The C dependency is the
**runtime** (`lib/runtime/brief_rt.c`), which splits into two layers:

| Layer | What it is | Verdict |
|---|---|---|
| **OS syscall shim** | `write`, `exit`, `mmap`, `clock_gettime`, `getenv`, `sysconf` | Must stay C (or become raw syscalls) — the OS is C's home. The pragmatic floor. |
| **Logic that happens to be in C** | digit→string printing, float formatting, string EQ/band/bor (byte loops), the allocator | Expressible in Brief stdlib with today's syntax. The stand-on-its-own win. |

**Rust's answer (the model):** a three-layer runtime split.
- **`core`** (`#![no_std]`, no libc): arithmetic, control, and `core::fmt`
  (formatting as pure Rust, no OS). `compiler_builtins` supplies the compiler-rt
  helpers (wide-int div/mul, memcpy/memset) in Rust/asm.
- **`alloc`**: the heap via a user-provided `GlobalAlloc` (libc `malloc` or a
  custom allocator on `mmap`).
- **`std`**: OS interaction (files, threads, env) via libc.

The key insight: **formatting is separable from writing, and the heap is
pluggable.** Rust's `println!` formatting is pure Rust; only `write(1, …)` is the
user's.

**Brief's version — the `.bv`/`.ebv` split.** The same stdlib API has two target
variants, in Brief's own file-extension terms:
- **`.bv`** — the OS/libc-backed stdlib (maps to OS functions).
- **`.ebv`** — the embedded/no-OS stdlib (pure Brief logic: digit conversion,
  byte loops, an allocator on a buffer).

The import resolver already tries `.bv` first (and treats `.ebv` as an
interchangeable variant); a freestanding target prefers `.ebv`. The intrinsic
boundary (rule 2): works under `--no-stdlib` → intrinsic; expressible in stdlib
syntax → `.bv`/`.ebv`; N special-cased intrinsics with one dispatch → one.

**Purism rejected:** removing the syscall layer entirely (a kernel needs *some*
interface), or forbidding `Malloc#`→`malloc` mapping (the OS allocator is the
pragmatic base).

### 2. The intrinsic audit

The intrinsic list splits into four real categories:

| Category | Examples | Verdict |
|---|---|---|
| Internal operator dispatch (not user-facing) | `Add#`, `Eq#`, `Shl#` (the `+`/`==`/`<<` resolution) | Keep — this *is* the compiler's type dispatch |
| Genuinely intrinsic | `Sqrt#`, `Sin#`, `Pow#` | Keep — emit native `llvm.*` intrinsics (the hardware claim is real) |
| Consolidatable | `PrintInt#`/`PrintFloat#`/`PrintStr#` | **Merge into one generic `Print#`** — the emission dispatches by the argument's protocol category |
| Stdlib-expressible | `Length#`, `Concat#`, `ToString#` | Move to `.bv`/`.ebv` (Phase 6) — Length# is a string-header read, Concat# is a byte loop, ToString# is digit logic |

**The audit criterion:** `--no-stdlib`-required → intrinsic; stdlib-expressible →
`.bv`/`.ebv`; N-with-one-dispatch → one generic.

**Native-LLVM mapping:** `Copy#`→`llvm.memcpy`, `Fill#`→`llvm.memset`, math→
`llvm.*` already done. The `Length#`/`Concat#` C-calls move in Phase 6.

**`Alloc#` is the universal allocation.** `ArenaAlloc#`/`AllocArena#`/`Realloc#`
are NOT registered intrinsics (no signature, no emission, no usage) — they
survive only as stale name-matches in `allocation.rs` and `global_lifetime.rs`.
The arena is assigned by the frontend's `default_strategy()` on `Alloc#`. The
stale name-matches are removed (Phase 2).

**`PrintChar#` remains** as the internal newline/char primitive — there is no
distinct Char type (a char is an Int code point), so it cannot be type-dispatched.

### 3. The garbage scheduler and the free-check

The garbage scheduler (`analysis/global_lifetime.rs`) proves each heap-backed
field's reactor-ordered last consumer and emits a `Free#` after it. When the
static proof **cannot close** a field (an unordered reader exists), the field
falls back to "lives for the program". The **free-check** reclaims that tail:

- **Refcount at the edge-of-use checkpoint** (the last possible consumer
  boundary, not an arbitrary point): the compiler emits inc/dec on address
  copies (reusing provenance); a zero count at the checkpoint → `__brief_free`.
  Sound (no premature free).
- **`free` / `keep` body annotations** let the programmer fill proof gaps with
  verified contracts:
  - `free x;` — "beyond here x is dead". The scheduler verifies the read-set
    after the annotation is empty; a LATER read is a **compile error (refuse the
    hint)**. Verified → free emitted at that point.
  - `keep x;` — "beyond here x must stay alive". Suppresses a scheduled free; a
    provably-dead-after `keep` is a **warning (redundant keep)**, not an error.
- **`briefc memcheck`** reports every unprovable field, its possible readers,
  and the exact `free`/`keep` hint that closes the gap.
- Both annotations are FRONTEND (scheduler) directives — a backend that doesn't
  free (a GC target) simply doesn't emit; no parse rejection, no
  backend-agnosticism conflict. They never make code faster (a lifetime contract
  that fills a proof gap).

**Composition with `~`:** the consumptive operator is the strongest lifetime
edge ("destroy here, now"); `free` is a verified hint; `keep` suppresses. All
three answer "when is this memory dead" at three confidence levels.

### 4. Stream symbols — `#StdIn` / `#StdOut` / `#StdErr`

Standard streams are **intrinsic-pointer hashword symbols** (`#StdIn` etc.) —
compiler-known values that resolve to the stream handle (backend-agnostic: a
libc `FILE*`/fd on OS targets, a WASM fd, an `.ebv` transport).

**Why hashword symbols (rule 2 disclosure):** the `#` prefix marks
compiler-known backend directives (`#Int`, `#Link<name>`, `#StdOut`), so the
special treatment is disclosed, not hidden.

**Why they compose with the trg system:** the trigger machinery already resolves
a name to a system memory address three ways (`LinkRef::Explicit(addr)` load
volatile from a literal address, `Linked(sym)` from a linked symbol,
`Deref(expr)` from a pointer), and `LinkRef::Stdin` already exists. The stream
symbols resolve through the SAME "get a system pointer address" path — one
mechanism, so `trg read @ #StdIn` (and the write side `#StdOut <- value`) work
naturally.

**The `.bv`/`.ebv` split lands:** `.bv` maps the stream symbols to the OS/fd,
`.ebv` provides a buffered/transport variant.

**Output sugar:** `#StdOut <- value` lowers to the low-level `Print#(value)`.

### 5. The trigger port removal — required-but-unused

Every trigger binding (`trg name @ instance.#port;`) requires a `.#port` suffix
in BOTH parser paths (`parse_top_level_trg`, `parse_trg_binding`), enforced with
a `#`-prefix check. **The port is nearly unused:** the only consumer is the
`is_wake: trg.name.starts_with("__wake") || trg.port == "__wake"` sentinel
(mod.rs:1850). The port never reaches `TriggerDeclaration`, never reaches
`emit_trg_load` (which uses only the `LinkRef` + type), and no trigger uses a
port. The `name.starts_with("__wake")` check already covers wake triggers.

Per rule 2 (strip accidental complexity), a required-but-unused grammar element
must go. **The port is removed:** the trigger form becomes `trg name @ instance;`
(a whole target — an address, a symbol, a stream, a pointer). The stale tutorial
(`trg button: Bool @ 0x1000A000;`, `@ link sym`) is rewritten to the real form.

### 6. Bugs swept (verified fixed)

- **Implicit Int × Float coercion silently bitcasts** (BUGS.md:2730) — already
  fixed: the typechecker rejects implicit Int↔Float arithmetic in binary ops.
  Verified `(count % 101) * 0.5` errors; `((count % 101) as Float) * 0.5`
  type-checks. Marked FIXED.
- **frgn String declare SSO mismatch** (BUGS.md:2779) — already fixed by the B0
  bits model: `protocol_llvm_type` returns `ptr` for every `#String`. Verified
  `float_math.ll` declare/call agree. Marked FIXED.
- **`PutChar#` in `lib/std/ffi/out.bv`** — a vestige referencing a nonexistent
  intrinsic; cleaned in Phase 2/4.

---

## Part II — The phases

### Phase 1 — Docs/highlighter (DONE, commit `0140db38`)

- `brief.tmLanguage.json`: removed the `word!` special-cases (the `"macros"` and
  `"macro-words"` repos); `?`/`!` are now standalone operator rules using the
  same scope as `@`/`&`; added `seq`/`vol`/`sync` keywords.
- `dbrief.tmLanguage.json`: the same `?`/`!` rules + `seq`/`vol`/`sync`.
- `spec/SPEC.md`: the watchdog BNF now includes the liveliness condition, the
  `-> handler(val)` callback, and the `within N <unit>` deadline.
- Collections plan: corrected the stale "Deferred: fuel/time" note.

### Phase 2 — Intrinsic audit + bug fixes (in progress)

1. **Generic `Print#`** — merge `PrintInt#`/`PrintFloat#`/`PrintStr#` into one
   `Print#`; the emission dispatches by the argument's protocol category
   (`#String` → `__print_str`, `#Float` → `__print_float` + the float-cache
   unbox, else `__print_int`). `PrintChar#` stays internal (the newline).
   Update `intrinsic_signatures.rs`, `intrinsics.rs`, `print_plugin.rs`, the
   interpreter, and every test/comment reference.
2. **Dead vestige removal** — the `AllocArena#`/`Realloc#` name-match arm in
   `allocation.rs`; `AllocArena#` in `global_lifetime.rs`; `PutChar#` in
   `lib/std/ffi/out.bv`.
3. **BUGS.md** — mark the two OPEN items FIXED (verified).
4. **Native-LLVM mapping** — already-done `Copy#`→memcpy, `Fill#`→memset,
   math→`llvm.*` stay; `Length#`/`Concat#` move in Phase 6.

### Phase 3 — Consumptive operators (`~op`), move semantics, the arrow rewrite

**The design.** `~` prepends to a binary operator to make it **consumptive** (the
RHS operand is consumed/destroyed after the op):

| Token | Meaning |
|---|---|
| `~=` | move-assign — copy b → a, then destroy b |
| `~<-` | copy into lhs, then **destroy/remove** rhs (the destructive extract) |
| `~+` `~-` `~*` `~/` | arithmetic with the RHS consumed after |

`~` **unary bitwise NOT stays** — the lexer distinguishes the multi-char tokens
(`~=`, `~<-`, `~+`, …) from a bare `~` before an expression. No ambiguity.

**The arrow rewrite** (`&` reserved for pointers):

| Form | Op |
|---|---|
| `src <- value` | insert (`InsertAt`) |
| `dest ~<- src` | **destructive** extract (`ExtractFrom`) |
| `dest <- src` | read (`CopyFrom`) |
| `~<- src;` / `<- src;` | destructive discard / read discard |

`<-` = *copy into lhs*; `~<-` = *copy into lhs, then destroy (or remove/extract
from for a collection) rhs*. The dispatch finds the collection by the **op
binding on each side** (the `&` marker is removed — it was a fake pointer ref
parsed as `AddrOf` then peeled). The AST distinguishes `<-` from `=` via a
dedicated `Statement::ArrowAssign`. `&x` remains genuine address-of.

**The `~/` term-until moves to `!/`** (the token is freed for the consumptive
divide). `[!/X]` is a **two-in-one contract**:

| Form | Precondition | Postcondition |
|---|---|---|
| `[!/X]` | `!X` | `X` |
| `[!/!X]` | `!(!X)` = `X` | `!X` |

A contract bracket beginning with `!/` parses the expression `X` and expands to
the pair `[!X][X]` (the leading-`!` variant collapses the double negation). The
dead `~?` (temporal-fallback) token is removed.

**Soundness rules:**
1. **Use-after-move = compile error** — the move pass tracks consumed operands;
   reading `b` after `a ~= b` is a type error.
2. **`~op` on a const = compile error** — you can only consume a **mutable
   lvalue** (a variable, state field, or collection). `a ~= CONST` and
   `dest ~<- const_collection` reject with "cannot consume a constant".
3. **Interpreter-first** — the reference implements the Consume evaluation +
   consumed-local errors before any codegen.

**Steps:**

| Step | Scope |
|---|---|
| **3.1 Lexer** | New tokens `~=` `~<-` `~+` `~-` `~*` `~/`; repurpose `~/` (drop term-until), drop `~?`; contract tokens `!/`/`!/!`; `~` stays unary bitwise |
| **3.2 Parser** | Consumptive tokens → binary ops with a consumed RHS (`Expr::Consume`); `~<-` → destructive arrow; `<-` → `Statement::ArrowAssign` (no `&`); the `[!/X]`/`[!/!X]` contract expansion |
| **3.3 AST** | `Expr::Consume(Box<Expr>)` (or a consumed-flag on the binary op); `Statement::ArrowAssign`; the contract two-in-one representation |
| **3.4 Typechecker** | The move pass (use-after-move = error; const-consumption = error); arrow dispatch by op binding + `~` marker; the `!/` contract typing |
| **3.5 Codegen** | Consumptive emission (copy; free/zero the source backing for Ptr/String/obj — no double-free); the destructive-extract path; remove the `&` peeling in the strategy dispatch |
| **3.6 Interpreter** | The reference — Consume evaluation, consumed-local read errors, the `!/` contract check |
| **3.7 Docs + migration** | Highlighter (the `~op`/`!/`/`!/!` tokens; drop `~?`/`~/` rules), SPEC grammar, the tutorial (the move/consume concept + the `!/` contract), migrate the 4 arrow files (`queue_drain`, `queue_drain_idio`, `stack_push_pop`, `lib/std/hashmap.bv`) |

**Migration scope (small):** bitwise `~` is unused in source; the arrow `&` is
used in 4 files (`queue_drain.bv`, `queue_drain_idio.bv`, `stack_push_pop.bv`,
`lib/std/hashmap.bv`).

### Phase 4 — Stream symbols (`#StdIn`/`#StdOut`/`#StdErr`) + trigger port removal

1. **Stream hashword symbols** — `#StdIn`/`#StdOut`/`#StdErr` as compiler-known
   intrinsic-pointer symbols. The compiler resolves them to a stream handle
   (backend-agnostic). `#StdOut <- value` lowers to `Print#(value)`. The trg
   system composes through the shared system-pointer-address path
   (`trg read @ #StdIn`).
2. **Trigger port removal** — `parse_top_level_trg` + `parse_trg_binding`: drop
   the `expect(Token::Dot)` + port (the `#`-prefix validation too). mod.rs:
   remove the `port == "__wake"` sentinel (keep the name check). Rewrite the
   trigger tutorial + the SPEC grammar line to the whole-target form.
3. **`briefc memcheck`** (or in Phase 5) — report the unprovable fields + hints.

### Phase 5 — The free-check

1. **`free`/`keep` body annotations** — the parser (`free x;` / `keep x;`), the
   AST (`Statement::FreeHint`/`KeepHint`), the scheduler semantics (free =
   verified contract → error on a later read; keep = suppress → warn on
   redundancy).
2. **Refcount free-check** — for unprovable heap fields, the scheduler inserts a
   refcount at the edge-of-use checkpoint; a zero count → `__brief_free`.
3. **`briefc memcheck`** — the diagnostics subcommand.
4. **Design doc** — `docs/plans/2026-08-01-free-check.md` (the mechanism, the
   annotation semantics, the memcheck UX, the correctness contract).

### Phase 6 — C-surface reduction (`.bv`/`.ebv`)

1. **Runtime inventory** — classify every `brief_rt.c` function as *syscall
   shim* (stays C) vs *logic* (moves to `.ebv`).
2. **`.ebv` variants** — formatting/strings/allocator logic in pure Brief; the
   `.bv` variants keep the OS/libc calls; the import resolver prefers `.bv` on
   OS targets, `.ebv` on freestanding.
3. **No-libc target sketch** — raw syscalls + `_start` documented.

---

## Part III — Verification (every phase)

- `cargo test --lib` green before each commit.
- `cargo build --release` — no new warnings.
- Full harness `bash benchmarks/build_and_bench.sh --runtime` — zero MISMATCH,
  zero precomputed flags.
- Praetor on changed directories.
- Architecture docs updated in the same commit as the structural change.
- The token/arrow migrations leave no dangling lexer tokens; SPEC/tutorial/
  highlighter updated in the same commits.

## Out of scope (parked)

- Chained-bucket HashMap (the flat form works; the chained end-state is a
  standalone follow-up).
- The Rust `compiler_builtins` equivalent (wide-int/memcpy libcalls) — Brief's
  integer ops are native-width i64, so the surface is minimal; deferred to the
  no-libc target work.
