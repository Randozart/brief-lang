# IR Bugs, Over-Annotation, and Performance Diagnostics

Date: 2026-07-05
Status: Execution
After: 31e6cb5 (A005c revert + A005a adaptive dispatch)
Target: Fix all IR bugs, correct over-annotation, restore precomputation correctness, hit best-known benchmark ratios.

## 1. Executive Summary

After the A005e→A005c revert + A005a adaptive dispatch, 6 benchmarks fail to
produce valid binaries or produce wrong output.  These failures divide into
three categories:

| Category | Count | Benchmarks |
|----------|-------|------------|
| **IR generation bugs** (blocking compilation) | 3 | sparse_dispatch, mandelbrot, queue_drain |
| **Correctness bugs** (wrong output) | 1 | nbody_newton |
| **Over-annotation** (latent miscompile risk) | 1 | #8 argmemonly on callable txns |
| **Precomputation drop** (output "" instead of correct value) | 3 | knucleotide, fasta, cancel_math |

Additionally, 3 benchmarks are still below their best-known ratios:
float_math (0.84x vs 0.67x), ring_buffer (1.00x vs 0.80x), fannkuch_redux
(1.61x vs 0.99x).  These are secondary to fixing the broken benchmarks.

## 2. IR Generation Bugs

### 2.1 sparse_dispatch — `%state_0` Use of Undefined Value

**Error:**
```
%ip_16 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
                                         ^^^^^^^^
error: use of undefined value '%state_0'
```

**Root cause:** `emit_modulo_switch_main` (loop_engine.rs:2090) writes
`%state = alloca %State` manually at line 2109 but never calls
`emit_state_allocas()`.  The `emit_state_gep` routing (emit_stmt.rs:232-234)
checks `main_body == true` and emits chunk-based GEPs like
`getelementptr inbounds %StateChunk0, ptr %state_0` — but `%state_0` was
never defined because `emit_state_allocas` was never called.

**Why only modulo-switch dispatch:** `try_modulo_switch_dispatch` (line 1495)
handles reactive multi-txn programs by constructing a modulo counter +
switch table.  Other paths (`emit_countable_main`, `emit_folded_main`,
`emit_ssa_main`) all call `emit_state_allocas`.  This path was missed.

**Fix:** Replace line 2109's manual alloca with `self.emit_state_allocas(out)`.
`emit_state_allocas` creates both chunk allocas (`%state_0`, `%state_1`)
AND the monolithic `%state` — all GEP paths work.

**Affected dispatch code path:** `try_modulo_switch_dispatch` →
`emit_modulo_switch_main`.

### 2.2 mandelbrot — `%t216` Instruction Does Not Dominate All Uses

**Error:**
```
Instruction does not dominate all uses!
  %t216 = add i64 %t207, %t215
  %t265 = add i64 0, %t216
```

**Root cause:** Guard handler in `emit_stmt.rs:957-965` saves/restores
`ssa_old_int_regs`, `ssa_old_float_regs`, `let_bindings`, and
`let_binding_types` across guard boundaries, but **does not save/restore**
`pending_phi_backedge` or `pending_phi_native_backedge`.

When a store inside a guard body (then_block) populates
`pending_phi_native_backedge` with registers like `%t216`, those registers
are only defined inside `then_block`.  The latch (`emit_countable_latch`)
reads `pending_phi_native_backedge` and emits code like:
```
%be_field = add i64 0, %t216
```
This instruction is placed in the latch block, but `%t216` is defined in
`then_block` which does NOT dominate the latch (the else path reaches the
latch through the merge block without going through `then_block`).

mandelbrot triggers this because `count` is both:
- Written by the counter increment (&count = count + 1)
- Read in a guard condition ([count % 5000000 == 0])
- The store to count sets `pending_phi_native_backedge`
- The guard's then_block defines the register for the new count value
- The latch reads this register for the backedge

**Fix:** Add `pending_phi_backedge` and `pending_phi_native_backedge` to the
save/restore in the guard handler (emit_stmt.rs:957-965).

### 2.3 queue_drain — `%ip_2` Use of Undefined Value

**Error:**
```
store i64 %rb2_h, ptr %ip_2, align 8
                        ^^^^
error: use of undefined value '%ip_2'
```

**Root cause:** `emit_ssa_main` (loop_engine.rs:1710) calls
`emit_state_allocas(out)` at line 1730, but the `main_body` flag is set
at line 1729 **after** the alloca call but **before** the chunk GEP emission
in later phases.  `emit_state_gep` checks `main_body` to decide whether to
emit chunk-based GEPs.  If `main_body` was set to `true` AFTER
`emit_state_allocas` but the chunk GEP emission happens later, the GEPs
reference `%state_0` which was created by `emit_state_allocas`.

But the error is `%ip_2` undefined, not `%state_0`.  This means the chunk
alloca exists (`%state_0` is defined) but the field index `%ip_2` — a GEP
from the chunk — is not.  `%ip_2` is a let binding for the GEP result.

Wait — looking at the actual error: `store i64 %rb2_h, ptr %ip_2, align 8`.
`%ip_2` is the pointer (result of `getelementptr`), and the error says it's
undefined.  This means the `getelementptr` that defined `%ip_2` was NOT
emitted, or was emitted in a different scope.

**Hypothesis:** `queue_drain` goes through the SSA register pipeline
dispatch (non-modulo, non-foldable).  The `emit_ssa_main` function builds
a sequential bounded dispatch.  Field initialization might use a different
path that doesn't emit `%ip_2` GEPs.

Actual root cause from deeper analysis: the `emit_inline_init_stores` /
`emit_state_gep` routing uses `main_body` to choose between chunk-GEP and
monolithic-GEP.  In `emit_ssa_main`, `main_body` is set to `true` correctly,
but the field initialization might happen BEFORE `main_body` is set, or the
GEP emission for `%ip_2` uses a path that doesn't check `main_body`.

**Fix:** Ensure `self.fun.main_body = true` is set BEFORE any
`emit_state_gep` or `emit_inline_init_stores` call.  Reorder the prologue
in `emit_ssa_main` to set `main_body = true` before `emit_state_allocas`.

### 2.4 nbody_newton — MISMATCH (2 lines vs 1)

**Error:** Briev outputs 2 lines, C outputs 1 line.

**Root cause:** The periodic guard `[count % 5000000 == 0]` evaluates using
the phi register `%phi_count` which holds the **pre-increment** counter
value.  On the first iteration, `%phi_count = 0`, so `0 % 5000000 == 0` is
TRUE, and the print fires with the initial energy value.  The C reference
evaluates the guard AFTER incrementing the counter, so the first print
happens at count=1 (1 % 5000000 != 0), producing only the final swan song
line.

The A005c per-field phi dispatch moves the counter increment to the latch
at the bottom of the loop.  Guard conditions in the body evaluate against
the phi register (pre-increment), not the incremented value.  This is
correct for the loop iteration but wrong for guard conditions that should
see the post-increment value.

**The source code ordering:**
```
&count = count + 1;             // increment FIRST
...
[count % 5000000 == 0] {        // guard SECOND — should see post-inc
    print_float#(energy);
};
```

The A005c dispatch restructures this as:
```
body:
    guard [phi_count % 5000000 == 0] { print_float#(energy); }
    ... other body statements ...
latch:
    pn = add i64 pi, 1           // increment here — AFTER guard
```

**Fix:** When emitting a guard condition in A005c mode, if the condition
references the counter field AND the counter increment precedes the guard
in source order, use `phi_count + delta` instead of `phi_count` for the
guard condition evaluation.

**Simpler temporary fix for the benchmark:** Move the guard before the
increment in nbody_newton.bv.  But this is a compiler fix, not a benchmark
patch.

### 2.5 Precomputation Drop — knucleotide, fasta, cancel_math

**Symptom:** Briev runs in 0.0006s (600μs, just process overhead), outputs
"" (empty).  C produces correct output (e.g., knucleotide outputs "3").

**Root cause:** The A005a dispatch path (`emit_folded_main` with
`use_phi=false`) uses a single `%slot_case` alloca + insertvalue chain.
LLVM's SROA + GVN + LICM + SCCP passes can prove the entire bounded loop
produces no observable side effects and fold it completely.  The resulting
binary just returns 0 without executing the loop or the swan song prints.

**Why this is incorrect:** The swan song `term! -> print_int#(result)` is
emitted by `emit_hoisted_post_loop_prints` in the `done:` block.  This code
reads from `%state` (via `pre_load_all_fields`) or from `last_val_temps`
(commit block).  If LLVM folds the loop, it also eliminates the swan song's
loads from `%state` because `%state` was never written (the loop was
replaced by a constant).  The final output is "" because the print never
executes.

**Per AGENTS.md:** "A value is live if an FFI call consumes it."  The
swan song IS an FFI call (print_int → fprintf).  The compiler must NOT
eliminate it.  The precomputation pass must preserve the swan song's
observability.

**Fix:** The precomputation detection (in `generate()`, mod.rs:2207)
checks if `precomputed_final_values` is Some.  When it is, it emits a
`emit_precomputed_main` that stores final values and returns.  But the
swan song's `emit_hoisted_post_loop_prints` is never called in this path.
The fix: in `emit_precomputed_main`, also emit the swan song's print code
using the precomputed final values.

## 3. Over-Annotation Analysis

### 3.1 #8 argmemonly on callable txns — Latent Correctness Bug

**Attribute group:** `#8 = { mustprogress nofree norecurse nosync nounwind
willreturn memory(argmem: readwrite) }`

**Functions using #8:**
- `briev_main` (user `defn` definitions)
- `@txn_name` (callable `txn` definitions)

**The problem:** `memory(argmem: readwrite)` tells LLVM the function only
accesses memory through its pointer arguments (`%state`).  But if a `defn`
or callable `txn` body contains an inline (non-hoisted) FFI call that
accesses global state — like `print_int#` which calls `fprintf` with
`@stdout` — the `argmemonly` annotation is incorrect.  LLVM could
miscompile by reordering stores past the fprintf, CSE'ing loads from
globals incorrectly, or removing the fprintf entirely.

**Why it's currently safe:** All print intrinsics (`term! -> print_int#`,
`term! -> printf#`) are hoisted to the `done:` block of `@main()` by
`hoist_terminating_guard` (mod.rs:129-173).  The `@main()` function uses
`#0` (no `argmemonly`) or `#3` (no `argmemonly`).  So `@briev_main` and
`@txn_name` never contain inline prints.

**The risk:** Any future change that adds inline (non-hoisted) FFI calls
to `defn` or callable `txn` bodies would trigger this bug silently.

**Fix (optional, belt-and-suspenders):** When emitting a `@txn_name` or
`briev_main`, scan the body for FFI calls that access globals.  If found,
use `#0` instead of `#8`.

### 3.2 #1 willreturn on exit/abort — Pre-Existing, Low Impact

**Attribute group:** `#1 = { nocallback nofree nosync nounwind willreturn }`

**Functions using #1:** All libc declarations: `@fprintf`, `@exit`,
`@abort`, `@getenv`, `@atol`, `@epoll_wait`, `@read`, `@write`, `@fflush`.

**The problem:** `exit()` and `abort()` never return, but `willreturn` says
they do.  LLVM could optimize away code after `exit()` assuming it never
executes, which is actually correct!  The issue is more subtle: LLVM might
not insert a `unreachable` after the call, allowing the function to fall
through to garbage code.  But this is pre-existing and not from the July 4
attribute changes.

**epoll_wait and read** with blocking timeouts also violate `willreturn`,
but LLVM doesn't currently exploit this for I/O functions.

### 3.3 Dead attribute groups #7 and #9

`#7` (memory(read)) and `#9` (memory(argmem: readwrite) non-willreturn) are
defined in `mod.rs` but never referenced by any function declaration.

**Impact:** IR bloat (~30 lines in the `.ll` file).  No correctness impact.

**Fix:** Remove dead attribute group emission code and related tests.

### 3.4 `!invariant.load` Scoping — Verified Correct

`!invariant.load` is only applied to fields NOT in the transaction's
`write_set`.  These fields are guaranteed to be unmodified by the loop
body (Briev has no hidden mutation through pointers).  The latch uses
identity backedge (no reload from %State) for these fields.

**No fix needed.**

## 4. LLVM IR Output Convention

The LLVM IR emitted by the compiler is an INTERMEDIATE REPRESENTATION.
It is not user-facing code and does not need to satisfy the Briev compiler's
coding standards (max 2 nesting, flat control flow, etc.).  The generated
IR must:

1. Be valid LLVM IR (no undefined values, no dominance failures, no type
   mismatches) — this is the current set of bugs being fixed.
2. Be maximally optimized — emit patterns that LLVM's optimizer (SROA, GVN,
   DSE, LICM, vectorizer, backend) recognizes and exploits.
3. Preserve program semantics — the output must match the C reference for
   all inputs.

The IR may use:
- Deep extractvalue/insertvalue chains (A005a)
- Many phi nodes (A005c, 31+ per loop)
- Complicated GEP navigation
- `select` for guard→single-assignment
- `!invariant.load`, `!range`, `!tbaa` metadata
- Chunk allocas and struct decomposition

All of these are valid LLVM IR patterns that the optimizer is designed
to process.  Do NOT simplify the IR for readability if it costs performance.

## 5. Implementation Plan

Ordered by dependency:

```
Phase A: Fix IR bugs (unblocks 3 benchmarks)
  A1. emit_modulo_switch_main: replace manual %state alloca with
      emit_state_allocas call.  (sparse_dispatch)
  A2. emit_ssa_main: set main_body = true before emit_state_allocas.
      (queue_drain)
  A3. emit_stmt.rs guard handler: save/restore pending_phi_backedge and
      pending_phi_native_backedge.  (mandelbrot)
  A4. emit_hoisted_post_loop_prints: ensure pre_load_all_fields works
      when last_val_temps is empty (fallback already exists, verify).
      (knucleotide precomputation prefix)

Phase B: Fix precomputation correctness (restores knucleotide/fasta/cancel_math)
  B1. emit_precomputed_main: add swan song emission via
      emit_hoisted_post_loop_prints using precomputed final values.
  B2. Verify that precomputed benchmark binaries produce correct output.

Phase C: Fix nbody_newton MISMATCH (correctness)
  C1. In emit_countable_main or emit_countable_body, when a guard
      condition references the counter field, emit the guard with
      phi_count + delta (the increment delta) instead of phi_count.
      The delta comes from the transaction's increment analysis.
  C2. Only apply when the increment statement precedes the guard in
      source order (not for guards that appear before the increment).

Phase D: Clean over-annotation (low priority, latent safety)
  D1. Remove dead attribute group #7 and its test.
  D2. Remove dead attribute group #9.
  D3. Scan callable txns for inline FFI calls and fall back to #0
      if found.

Phase E: Tune dispatch thresholds for best-known ratios
  E1. After Phases A-D pass, re-run --runtime benchmark.
  E2. If ring_buffer, float_math, fannkuch_redux still below targets:
      check which dispatch path was selected.  Tune thresholds.
```

## 6. Per-Phase Verification

Each phase must pass:
1. `cargo test --lib` — all 1398+ tests pass
2. `bash benchmarks/build_and_bench.sh --correctness` — all MATCH
3. The specific benchmark(s) fixed by that phase produce valid IR and
   correct output

Phase E additionally needs:
4. `bash benchmarks/build_and_bench.sh --runtime` — ratios recorded
5. Compare against `results/2026-07-05-baseline.txt`

## 7. Commit Checklist (per commit)

1. `cargo test --lib` — all tests pass
2. `cargo build --release` — no warnings
3. Praetor on new/changed files
4. Update BUGS.md if a bug diagnosis was confirmed
5. Benchmark the fixed benchmark(s) individually:
   ```bash
   ./target/release/briev-compiler llvm benchmarks/<name>.bv --out /tmp/test
   opt -O2 -S /tmp/test/<name>.ll -o /dev/null 2>&1 | head -5
   ```
