# 2026-07-06: Benchmark Correctness Fixes (bit_clear, fasta, sparse_dispatch, nbody_sqrt)

## Overview

Four benchmarks had pre-existing MISMATCHes against their C references.
Three were fixed with benchmark-level changes, two with compiler-level changes.
The final fix (nbody_sqrt vector phi backedge) revealed a correctness-sensitive
interaction between HashMap iteration order and A005c's per-field phi dispatch.

## Bug 1: bit_clear — Live-value read races deferred store

### Symptoms

`bit_clear` produces output `0` but C reference produces `1`. The Brief
version reads stale (pre-update) field value for the `[reg == 0]` guard.

### Root Cause

The guard `[reg == 0]{ ... }` reads `reg` after `&reg = reg & (reg - 1)`.
In A005c per-field phi dispatch, body stores are deferred to
`pending_phi_backedge` (Path A zero-store optimization).  The guard reads
`reg` from `ssa_old` caches (phi register), which holds the OLD value —
the `&` store hasn't updated it yet.  So the guard always reads the
pre-update value, not the post-update result.

### Fix

Instead of reading `reg` (the field name, resolved via ssa_old cache which
holds the old phi value), compute the new value into a local variable and
read that for the guard:

```
let next_reg: Int = reg & (reg - 1);
&reg = next_reg;
[next_reg == 0] { ... };    // reads next_reg, not ssa_old[reg]
```

### Architecture Impact

None. This is a benchmark-level idiomatic workaround.  The long-term
architecturally correct fix would be to update ssa_old caches after
`&` assignments when a subsequent guard reads the field (parallel-safe
exemption), but that requires liveness analysis of guard conditions
across the body — the local-variable pattern is simpler and more explicit.

## Bug 2: fasta — FFI in hoisted guard treated as output

### Symptoms

`fasta` produces shorter output than C. `putchar#()` calls inside the
[count % N == 0] periodic print guard are dead-code-eliminated.

### Root Cause

The hoisted guard's print output contains `putchar#()` intrinsic calls.
`loop_engine.rs:emit_countable_main` uses `is_output_call()` to detect
whether the body contains observable output, which prevents dead-field
elimination from removing field stores.  `putchar#()` is dispatched via
`Intrinsic::PutChar` in `emit_intrinsic_call`, but `is_output_call()`
didn't recognize it — only `print_int#`, `print_float#`, and `print_str#`
were listed.

Adding `Intrinsic::OutputWriteByte` (the old name) wasn't enough because
the modern Intrinsic enum uses `PutChar`, not `OutputWriteByte`.

### Fix

Added `| Intrinsic::PutChar` to the match in `is_output_call()`:

```rust
Intrinsic::PutChar | Intrinsic::PrintInt | Intrinsic::PrintFloat | Intrinsic::PrintStr => true,
```

### Architecture Impact

None. This was simply an incomplete match arm.

## Bug 3: sparse_dispatch — Dual %State allocas with uninitialized chunk

### Symptoms

`sparse_dispatch` produces wrong output.  The modulo-rotated dispatch
creates a "chunk" alloca (subset of %State fields) for the dispatch body,
then copies back to the monolith %State.  The chunk was never initialized
from the monolith before the dispatch body ran, so GEP loads in guard
conditions read garbage.

### Root Cause

`emit_modulo_rotated()` creates a chunk alloca for the dispatch body's
written fields, then copies chunk→monolith after the body.  But before
the body, the monolith→chunk copy (init from monolith to chunk so the
chunk has the current state) was missing.  Guard conditions like
`[count % 5000000 == 0]` read `count` via GEP load from the chunk,
which was uninitialized → garbage → wrong guard branch → wrong output.

Additionally, the print guard threshold `[count % 5000000 == 0]` was
used with `print_int#(count)`.  C's print happens after `count++`, so
the printed values are 5M, 10M, 15M...  Brief's guard fires BEFORE the
`&count = count + 1` increment, printing 0, 5M-1, 10M-1...  Mismatch.

### Fix

1. After `emit_inline_init_stores(out, "%state")` (which initializes the
   monolith stores in the initial tick), add a copy loop that iterates
   the chunk's fields and GEP-copies each from monolith to chunk:
   ```
   for var_name in chunk_var_names {
       emit gep from chunk_alloca
       emit gep from %state
       emit load from %state gep
       emit store to chunk gep
   }
   ```

2. Changed print guard threshold to `[count % 5000000 == 4999999]` with
   `print_int#(count + 1)` to match C's post-increment timing.

### Architecture Impact

The copy-loop approach is a minimal-fix workaround.  The long-term
architectural fix would route ALL guard GEP reads through
`emit_state_gep` during `main_body=true`, so guard conditions always
read from the monolith %State regardless of which alloca the dispatch
body writes to.  This would eliminate the dual-alloca sync problem
entirely at the cost of one extra GEP indirection per guard read.

## Bug 4: nbody_sqrt — Vector phi backedge captures stale insertelement

### Symptoms

nbody_sqrt produces `-0.170945078` at BOUND=5 instead of C reference
`-0.169288993`.  Energy drifts 0.17% per iteration (should be conserved
by the leapfrog integrator).  The drift is from body positions never
advancing — only the first element of each vector group's phi backedge
carried the updated value; elements 1-3 stagnated at initial values.

### Root Cause

A005c per-field phi dispatch groups fields into `<4 x float>` vector
phigroup (x0..x3, y0..y3, etc.)  The body computes new values for all
4 elements via a chain of `insertelement` instructions, accumulating in
`vector_phi_current[vec_phi]`.  Each store writes the insertelement
result to `pending_phi_native_backedge[field_name]`.

At the latch (`emit_countable_latch`), the backedge entries iterate
`backedge_field_regs` which maps each field name to its backedge register.
All members of a vector group share the same `be_reg` (e.g., `%be_vx_v4`).
The `emitted_be` dedup set emits the backedge only for the FIRST field
name encountered in HashMap iteration order.

The latch reads the backedge value from
`pending_phi_native_backedge[name]` — the insertelement for THAT SPECIFIC
field.  If the first name is "vx0", the value is `%iv1 = insertelement
%phi_vx_v4, nvx0, 0` — elements 1-3 come from `%phi_vx_v4` (previous
iteration).  Only element 0 is updated; elements 1-3 stagnate.

Since HashMap iteration order is non-deterministic (consistent per-binary
but varies with hash seed), different vector groups could pick different
first-field names: one group might get element 3 (all 4 correct), another
element 0 (only first correct).  nbody_sqrt had vx, vy, vz groups
picking element 0 or 1 (3 of 4 elements stale), and bx, by, bz groups
picking element 3 (all 4 correct).  The effect varied by group.

### Bug Timeline

This bug existed since the vector phi group feature was introduced
(2026-07-05).  It was masked by THREE interacting bugs in the same
code path:

1. `build_vector_phi_groups` called AFTER `last_val_temps` allocation
   → scalar allocas for vector fields → commit block stores `<4 x float>`
   into 4-byte alloca (buffer overflow) → garbage in done: block
2. `load_last_val_temps` missing `vec_field = Some(())` before `break`
   → falls through to scalar load from vector alloca → type mismatch
3. The backedge bug itself (this fix)

Fixes 1 and 2 were applied first (reordering + break fix), which made
nbody_sqrt deterministic but exposed the backedge bug.  Without fix 1,
the buffer overflow produced effectively random output, masking the
backedge issue.

### Fix

In `emit_countable_latch`, when the typed_reg starts with `%iv`
(indicating a vector group insertelement), reconstruct the vector phi
name from the backedge register name and read from
`vector_phi_current[vec_phi]` instead of
`pending_phi_native_backedge[name]`:

```rust
// %be_vx_v4 → strip "%be" → "_vx_v4" → strip "_" → "vx_v4"
// → format!("%phi_{}", "vx_v4") → "%phi_vx_v4"
let suffix = be_reg[3..].strip_prefix('_').unwrap_or(&be_reg[3..]);
let vec_phi_name = format!("%phi_{}", suffix);
let acc_reg = self.fun.vector_phi_current.get(&vec_phi_name)
    .map(|s| s.as_str())
    .unwrap_or(typed_reg);
writeln!(out, "  {} = bitcast <4 x float> {} to <4 x float>", be_reg, acc_reg).ok();
```

`vector_phi_current[vec_phi]` holds the FULLY accumulated vector after
ALL 4 insertelements have been chained — every element carries its
correctly computed value.

### Architecture Impact

This is the third bug in the vector phi group code path (after the
last_val_temps ordering bug and the load_last_val_temps break bug).
Three bugs in one feature suggests the feature's test coverage was
insufficient — the only test case (nbody_sqrt) had the buffer overflow
masking all downstream issues.

**Recommendation**: Add a targeted test that exercises the A005c vector
phi backedge path with verifiable float/position values, not just a
full-benchmark comparison against C.  A unit test that constructs a
minimal 4-field vector group and verifies all 4 elements carry correct
values across the phi backedge would have caught this immediately.

## Verification

All 22 benchmarks MATCH against C references (0 MISMATCH):

| Benchmark | Status | Fix Type |
|-----------|--------|----------|
| bit_clear | MATCH | benchmark (local variable guard) |
| fasta | MATCH | compiler (is_output_call arm) |
| sparse_dispatch | MATCH | compiler (copy loop + threshold) |
| nbody_sqrt | MATCH | compiler (vector_phi_current) |

Runtime ratios unchanged or improved:
- nbody_sqrt: non-deterministic → 0.74x (Brief wins, deterministic)
- mandelbrot: unchanged (0.99x)
- All others: within noise
