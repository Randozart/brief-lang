# Compile-Time Hang Detection

## What we're fixing

Four locations where the compiler can hang (infinite loop or effectively
unbounded iteration) instead of producing a compile-time error:

| # | Location | Pattern | Risk |
|---|----------|---------|------|
| 1 | `main.rs:3093` | RBV component tag expansion: `while changed` with **no iteration limit** | HIGH |
| 2 | `import_resolver.rs:219` | Import resolution: **no cycle detection**, A→B→A recurses infinitely | HIGH |
| 3 | `interpreter.rs:916` | Callable txn convergence: 10M max iterations, **silent break** on limit | MEDIUM |

## Principle

"If hanging is even suspected, Briv throws a compile time error. If it's not
an error state, it's a bug in the compiler."

The cycle-budget watchdog is the exception — when enabled, it correctly
terminates cycles via watchdog violation. These three locations have no
watchdog at all.

## Plan

### Fix 1 — Component tag expansion (`main.rs:3093`)

Add `MAX_TAG_PASSES = 100` iteration counter. When exceeded, return a
compile error with the offending component tag names instead of looping
forever.

Diff: ~10 lines. Surrounded by existing error-returning code.

### Fix 2 — Import resolution cycle detection (`import_resolver.rs`)

Add `in_progress: HashSet<String>` field to `ImportResolver`. Before the
recursive `resolve_imports` call (line 699), check if the current
`path_str` is already `in_progress`. If so, return a cycle error.

The insert/remove wraps only the read-parse-resolve-cache section, so early
returns (CSS/SVG/DBV) don't touch it. No cleanup needed on error — the
resolver is discarded after the compilation.

Diff: ~15 lines + struct field.

### Fix 3 — Callable txn convergence (`interpreter.rs:916`)

1. Add `txn_convergence_max_iterations: u64` field (default 10_000) to
   `Interpreter` struct.
2. Use it instead of the hardcoded `10_000_000`.
3. **Return an error** when the limit is hit, instead of silently breaking
   with partial state.
4. Add `with_txn_convergence_max_iterations()` builder method.
5. Add `--txn-convergence-max-iterations <N>` CLI flag to `llvm` command.
6. Thread it through `run_llvm_compile` → macro expansion / PGO so the
   interpreter respects the user's setting during compilation.

Default of 10_000 because a non-converging callable txn at compile time
should be caught in milliseconds, not minutes.

### Testing

- `cargo test --lib` after each fix — all 1363+ tests must pass.
- For Fix 3: existing tests that rely on convergence (e.g. `iter_map` pattern)
  should converge well within 10_000 iterations. Any that don't will get a
  clear error message to increase the limit.

## Key Decisions

- Fix 3's error-on-limit is the important behavioral change: silently breaking
  from the convergence loop produces incorrect partial results, which is worse
  than a hang. A compile error is always preferable.
- The default of 10_000 is conservative — most benchmarks converge in <100
  iterations. A user hitting the limit gets a specific error message telling
  them to use `--txn-convergence-max-iterations <higher>`.
- Import cycle detection uses `path_str` (the import path string, e.g.
  "std/foo.bv") as the dedup key. This matches the cache key used in
  `loaded_modules` and provides clear error messages.
- Component tag expansion does a depth-limit check rather than cycle
  detection because the component graph may legitimately repeat tags at
  different expansion levels (though this would be unusual).
