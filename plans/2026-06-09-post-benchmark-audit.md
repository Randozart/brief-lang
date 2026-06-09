<!-- 2026-06-09 -->

# Post-Benchmark-Audit Bug Analysis

There are no "quick fixes." Every bug below represents either a compiler
semantic error (emitting wrong code), a benchmark design error (using the
wrong construct for the intent), or a missing safety check (silently
producing a hang instead of an error). Each root cause is traced to the
specific decision that made it possible.

---

## 1. `constant float 0` — LLVM Backend Emits Invalid IR

**Symptom**: `@b2 = constant float 0` — clang rejects with "integer constant
must have integer type". The correct LLVM IR is `constant float 0.0`.

**File**: `src/backend/llvm/mod.rs`

### Root Cause 1a: dedup catch-all conflates distinct float constants

Line 641: `_ => format!("{}:0", llvm_ty)`

When `try_eval_cfloat()` returns `None` for a float expression (it does for
any expression it cannot fold — `_ => None` on line 52), the expression
stays in its original form (e.g. `Expr::Cast`, `Expr::Identifier`, etc.).
The dedup key computation's catch-all produces `"float:0"` for ALL such
expressions. Every non-folded float constant gets the same key, causing
them all to alias to the first one (which happens to be `@a1 = constant float 0`).

**Design error**: The dedup key is computed from the EXPRESSION VALUE, not
from the expression STRUCTURE. Two completely different float expressions
that cannot be folded will produce the SAME key `"float:0"` and erroneously
alias. This is a compiler correctness bug — it silently emits wrong values
for computed float constants.

### Root Cause 1b: value catch-all emits integer `0` for float type

Line 675: `_ => "0".to_string()`

When the non-folded expression reaches the value emission path, the same
catch-all produces `"0"` (an integer literal). For `float` constants, LLVM
IR requires a floating-point literal (`0.0`) or a bitcast expression.

**Design error**: The catch-all value `"0"` is type-oblivious. It produces
valid integer IR but invalid float IR. The dedup and value emission paths
never verified that their fallback produces valid LLVM IR for the declared
type.

### Fix Must Address

1. The dedup key must either (a) be a structural hash of the entire
   expression tree (so two distinct expressions never collide), or (b) every
   expression must produce a unique key with no catch-all.
2. The value emission must produce a syntactically valid float literal
   (`"0.0"`) when `ty == Type::Float`, regardless of the expression form.
3. `try_eval_cfloat` should be strengthened to fold ALL float-typed
   constant expressions, or the compiler should reject un-foldable float
   constants at compile time rather than silently emitting wrong values.

---

## 2. `@str.0` undefined — LLVM Backend References Undeclared Global

**Symptom**: `getelementptr inbounds [6 x i8], [6 x i8]* @str.0` but `@str.0`
has no definition in the module. Clang rejects.

### Root Cause 2a: collectors don't traverse all Expr/Statement variants

`collect_strings_expr` (mod.rs:114-158) has a `_ => {}` wildcard at line 156
that silently skips any `Expr` variant not explicitly handled. Missing:
`Expr::Literal`, `MapLiteral`, `SetLiteral`, `ArrowMut`, `ArrowDiscard`,
`ArrowTransfer`, and multiple Pattern-B packed variants.

`collect_strings_stmt` (mod.rs:100-112) similarly skips `Escape`,
`LocalTrigger`, `Alka`, `OnExit`.

**Design error**: The collectors are manually maintained match blocks with
no exhaustiveness check. Every time a new `Expr` or `Statement` variant is
added, the collectors silently go out of date. There is no compile-time
safety net (`_ => {}` suppresses the "missing match arms" warning).

### Root Cause 2b: reference site silently resolves to index 0

`emit_expr.rs:30` and `literal.rs:104`:
```rust
let si = self.string_constants.iter().position(|x| x == s).unwrap_or(0);
```

When a string was never collected, `position()` returns `None`, and
`.unwrap_or(0)` maps it to index 0 — which exists but is the FIRST string
in the collected set, not the requested string. If no strings were collected
at all, `@str.0` is referenced but never defined.

**Design error**: The reference site silently degrades to a wrong value
instead of panicking or producing a diagnostic at compile time. Silent
degradation means the first time you notice is when clang fails to link.
`.unwrap_or(0)` masks the bug during compiler development.

### Fix Must Address

1. Replace `_ => {}` with explicit arms for ALL Expr/Statement variants,
   or generate the collectors procedurally from the variant list.
2. Add a debug assertion or compile-time check that the string is actually
   in `self.string_constants` after collection.
3. Eliminate `unwrap_or(0)` — if a string is referenced but not collected,
   collect it on the spot or assert.

---

## 3. fasta LCG Broken — rct txn Atomic Write Semantics

**Symptom**: All output characters are `q` (ASCII 113). The LCG seed stays
at 42 for all iterations.

**File**: `benchmarks/fasta.bv`

### Root Cause

`rct txn` atomically batches all state writes until `term;`. Inside a
single tick, the three `&seed = ...` statements each read the PRE-TICK
value of seed (42), compute their result, and defer the write. At `term;`,
the writes commit in sequence — the last write wins:

```
&seed = seed * IA;     // reads 42, writes 162834 (deferred)
&seed = seed + IC;     // reads 42, writes 192407 (deferred)
&seed = seed % IM;     // reads 42, writes 42    (deferred, wins)
```

The seed is 42 forever. All iterations produce `42 % 26 + 97 = 113 = 'q'`.

### Design Error

The benchmark author expected sequential in-tick execution of
`&field = expr(changes to field)`. But `rct txn` semantics are:
- All reads see pre-tick state (consistent snapshot)
- Writes commit at transaction boundary
- Within a transaction, later `&field = ...` destinations overwrite earlier
  deferred writes for the same field

This is documented in the language spec (reactive transactions = atomic
state transitions). The benchmark was written against a mental model that
doesn't exist.

### Fix Must Address

1. Restructure the LCG as a single expression within the rct txn so
   writes are not chained: `&seed = (seed * IA + IC) % IM;`
2. Add a convergence output: `[count == N] { term! -> __putchar(seed % 26 + 97); };`
   so the program produces exactly one output byte per run — symmetric with
   C's `putchar` on the final value.
3. Verify the output matches C reference for BOUND=5.
4. Document the rct txn atomic-write constraint in the benchmark header:
   `&field = expr1; &field = expr2;` is NOT sequential — all read pre-tick
   state, last deferred write wins.

---

## 4. precompute_sum Infinite Tick Loop — Compiler Emits Non-Terminating Code

**Symptom**: Binary never exits. LLVM IR shows the classic reactor pattern:
```
br label %tick
tick:
  call void @reactor_tick(%State* %state)
  br label %tick
```

No exit check, no `#!exit` condition, no observable output. The binary spins
forever.

### Root Cause — Three layers of missing safety

**Layer 1: Budget-exceeded fallback is silent**. `const total: Int = 500`
with `--optimize-budget 256` means the compiler cannot fully precompute
(500 > 256). The fallback path is the reactor loop — an infinite tick loop
that relies on `reactor_tick()` to converge the state. But the compiler
never warns: "budget exceeded, falling back to unbounded reactor loop."

**Layer 2: Reactor has no exit provision**. The emitted loop is:
```
tick:
  call void @reactor_tick(%State* %state)
  br label %tick
```
There is no exit branch. Even if both transactions' preconditions are false,
the loop spins forever. The `#!exit` pragma exists to provide an exit
condition, but `precompute_sum.bv` doesn't have one, AND the reactor
codegen path doesn't check for `#!exit` (it emits the pragma only in folded
loop paths, a pre-existing codegen gap documented in BUGS.md:2026-06-01).

**Layer 3: No observability warning**. The loop body has zero observable
side effects — no FFI calls, no IO, no volatile stores. LLVM can eliminate
this loop at `-O3`, but the `.o` linking path uses `cc -O2` which may not
run SROA. Even if LLVM eliminated it, the compiler should not rely on LLVM
to fix its own non-termination problems. The compiler should warn when it
emits a loop with no exit condition and no observable side effects.

### Why `term! -> print(...)` is the right fix

Rather than relying on `#!exit` (which has its own codegen path gaps), the
correct fix is to restructure `precompute_sum.bv` as a single `rct txn`
with the convergence output embedded in the body:

```brief
rct txn compute [count < total][count == total] {
    &acc_a = acc_a + count;
    &acc_b = acc_b + count;
    &count = count + 1;
    [count == total] {
        term! -> __print_int(acc_a + acc_b);
    };
    term;
};
```

This makes the program's observable output coincident with its convergence
point. The compiler cannot precompute (the bound exceeds budget) but cannot
eliminate the loop either (the FFI call is structurally live as a swan song
gated on the convergence condition). Symmetric with C's `printf("%d\n", a+b)`
at the end of main.

The `#!exit` pragma is the wrong tool for benchmarks — it's a safety net
for programs that genuinely need to detect a global termination condition
across multiple independent transactions. For single-txn benchmarks, the
`term! -> swan_song` pattern inside the convergence guard is both cleaner
and more robust.

### Fix Must Address

1. **Restructure `precompute_sum.bv`** as a single txn with
   `[count == total] { term! -> __print_int(acc_a + acc_b); };`
2. **Audit all optimizer benchmarks** — if they lack an FFI call in the
   convergence path, add one. An optimizer benchmark without observable
   output is not a benchmark, it's a compiler stress test.
3. **Add a compiler safety pass** that warns when:
   - The budget is exceeded AND the emitted loop has zero observable
     side effects (no FFI calls, no volatile stores)
   - No convergence output path exists through any codegen path
4. This safety pass should be an error in `--strict` mode, a warning in
   default mode.

---

## 5. C Reference Binaries Exit Code 6 — C Code Has Runtime Errors

**Symptom**: `float_math_c`, `float_math_nonzero_c`, `const_heavy_c` all
exit with code 6 and no output when run with `BOUND=5`.

### Root Cause

Unknown without investigation. The exit code 6 and zero output suggest a
crash during startup (before any `putchar`/`printf` call). Likely causes:
1. Missing `-lm` link flag for `sqrtf` or other math functions
2. Assertion failure in runtime init when `BOUND` is small
3. The `fprintf(stderr, ...)` in `__print_float` writes to an unopened
   file descriptor (unlikely — stderr is always open)

### Fix

Investigate individually with `gdb` and minimal test cases. Each C reference
that fails must be fixed before the benchmark is considered valid for
correctness comparison.

---

## 6. Correctness Harness — Detects But Doesn't Diagnose

The `--correctness` mode correctly detects mismatches but doesn't
distinguish failure modes:
- **Binary crashed**: exit code != 0 → link or runtime bug (Bug #5)
- **Binary hung**: timeout → infinite loop (Bug #4)
- **Output differed**: ran but wrong → semantic bug (Bug #1, #3)

The harness should infer the failure mode from timing and retcode, and
print a diagnosis hint directing to the relevant bug.

---

## Fix Priority

| Priority | Bug | Impact | What it requires |
|----------|-----|--------|------------------|
| **P0** | #1a — Float dedup conflates constants | Wrong values emitted for computed floats | Replace catch-all with structural-expression key |
| **P0** | #1b — `constant float 0` instead of `0.0` | Invalid IR, clang rejects | Type-aware catch-all value |
| **P0** | #2a — Collectors miss Expr variants | Missing `@str.N` definitions | Exhaustive match or derive |
| **P0** | #2b — `unwrap_or(0)` masks collection gaps | Links `@str.0` undefined | Assert or on-the-fly collect |
| **P1** | #4 — Missing observability safety pass | No diagnostic when emitted loop has no observable output | New compiler pass: warn/error on dead-loop emission |
| **P1** | #3 — fasta LCG chain broken | Wrong benchmark output, seed never changes | Single-expression `&seed = (seed * IA + IC) % IM;` + `[count == N] { term! -> print(...); }` |
| **P1** | #4 — precompute_sum no observable output | Benchmark hangs — no FFI in convergence path | Restructure as single txn with `term! -> __print_int(...)` at convergence |
| **P2** | #5 — C reference binaries exit 6 | Wrong C reference comparisons | GDB per-binary investigation |
| **P2** | #6 — Harness failure diagnosis | No diagnosis hints, manual debug | Add exit-code/timeout analysis |

P0 = compiler emits semantically wrong or non-compilable code.
P1 = compiler emits code that doesn't terminate correctly, or benchmark
     uses wrong patterns (no observable output at convergence).
P2 = auxiliary code needs fixing (not compiler bugs).
