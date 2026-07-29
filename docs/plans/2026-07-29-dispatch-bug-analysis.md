# Dispatch Bug Analysis — 2026-07-29

## Findings from IR Investigation

### Verified Facts

#### `#UInt → i8` hypothesis: FALSE
Neither `fasta.bv` nor `knucleotide.bv` uses `UInt`. Both use only `Int` declarations. The
emitted `.ll` files confirm every operation is `i64`:

- `%State = type { i64, i64, i64, i64 }` (fasta)
- `%State = type { i64, i64, i64, i64, i64, i64 }` (knucleotide)
- All `mul nsw i64`, `srem i64`, `shl i64` — no `i8` anywhere

The normalizer (`normalizer.rs:524`) handles `Cast.#UInt` identically to `Cast.#Int`,
both resolving to `i{int_bits}` = `i64`. The primordial table (`type_universe/mod.rs:93`)
gives `UInt` no baked-in `llvm_type`, same as `Int`.

#### Dispatch IS selecting InlineSsa for both benchmarks
The `.fmain.` label prefix in both `.ll` files confirms `emit_folded_main`
(not `emit_countable_main` which uses `.cm_` labels).

#### `emit_folded_loop` passes empty `write_set` — ROOT CAUSE
In `counter.rs:110-111`:

```rust
let write_set: HashSet<String> = HashSet::new();  // ← EMPTY!
let mut hoisted = Vec::new();
self.emit_countable_body(out, stmts, &write_set, &mut hoisted);
```

When `emit_folded_main` calls `emit_folded_loop` with `use_phi=false` and a body
(which is the InlineSsa path at `mod.rs:2724`), it creates an **empty `write_set`**
and passes it to `emit_countable_body`.

This means at `counter.rs:669`:
```rust
if write_set.contains(n) {  // ← NEVER true for non-counter fields
```

ALL non-counter state writes (seed, hash, chksum) are silently dropped:
- `pending_phi_backedge` is NEVER updated
- %State stores are NEVER emitted (unless `needs_state_stores_in_body` is set)
- The computed values vanish after each iteration

**Effect on fasta**: `seed` is always `42` (initial value) → `(42*3877+29573)%139968 = 52439`
→ `52439%26+97 = 120 = 'x'` → prints "xxxxx..." forever.

**Effect on knucleotide**: `seed=42` constant, `hash=0` constant → wrong LCG values →
17x slowdown from pathological hash table behavior + wrong output.

#### `PrintChar#` vs `__print_char`
- `PrintChar#` IS a real intrinsic (`intrinsics.rs:81`, `observable: true`)
- `__print_char` is a separate FFI declaration (in the `.ll` as `declare i64 @__print_char`)
- The print plugin (`print_plugin.rs:240`) transforms `PrintLn!` to
  `PrintInt#(x)` + `PrintChar#(10)` (both intrinsics)
- `fasta.bv` calls `__print_char` directly (not through `PrintLn!`)
- `is_pure_body` correctly detects BOTH as impure (`Expr::Call(_,_,_) => true`
  at `transition_graph.rs:772`)

**Therefore**: purity is NOT why these benchmarks enter InlineSsa. They enter
because of the Phase 4 dispatch condition `write_density >= 0.5 && total_fields < 8`.

#### Dispatch selection criteria (Post-Phase 4)
At `mod.rs:2718`:
```rust
} else if write_density >= 0.5 && total_fields < 8 {
    // InlineSsa: insertvalue chain for small, dense-write states.
```

No check for "does the body write non-counter state fields." fasta (2/4, seed+count)
and knucleotide (4/6, seed+hash+chksum+count) both qualify purely on density + count.

#### `cancel_math` and `queue_drain` at 0x MATCH
These write only the counter variable. The empty `write_set` doesn't hurt them
because there are no non-counter writes to drop. Their runtime behavior is correct.

#### Dead code (Post-Phase 4)
- `emit_while_main` (`counter.rs:236`) — had `needs_state_stores_in_body = true`
- `emit_folded_memory_main` (`counter.rs:159`) — had `needs_state_stores_in_body = true`
Both are unreferenced from the dispatch. The old `has_body_ffi → emit_while_main`
path was the correct handler for FFI bodies with non-counter writes.

### Still to Investigate

1. **Other benchmarks hitting InlineSsa silently** — any with non-counter state writes
   matching `total_fields < 8` and `write_density >= 0.5`. `mandelbrot` (5 fields,
   all written) is one candidate.

2. **`nbody_newton` at 1.23x C** — 30+ fields, routed to VectorPhi (not InlineSsa).
   Separate investigation needed. Vector group detection might not fire correctly
   for its field structure.

## Implementation Plan

### Fix 1: Dispatch guardrail — prevent InlineSsa for bodies with non-counter writes

In `mod.rs` before the InlineSsa path (around line 2717):

```rust
let writes_non_counter = node.write_set.iter().any(|f| {
    counter_var.map_or(true, |cv| f != cv)
});
```

**Option A (recommended)**: Skip InlineSsa when `writes_non_counter` → route to
PerFieldPhi. PerFieldPhi is battle-tested and handles FFI, complex writes, and
all edge cases correctly.

**Option B**: Set `needs_state_stores_in_body = true` inside `emit_folded_loop`
when body has non-counter writes. More complex — `emit_folded_loop` doesn't
currently create per-field phis or a `%State` SSA phi, so stores would need to
go to memory.

### Fix 2 (cleanup): Remove dead `emit_while_main` / `emit_folded_memory_main`

After confirming Fix 1 stabilizes all benchmarks, these functions are dead
code. Remove them to prevent future confusion.

### Fix 3 (separate): Investigate `nbody_newton` vector phi regression

Approach: IR diff against Era 5 baseline (`b39461e2`), check if vector group
detection is firing correctly for the 30+ field state. Plan in separate
document if needed.

## Verification Steps

1. Apply Fix 1 (Option A)
2. `cargo test --lib`
3. `bash benchmarks/build_and_bench.sh --correctness`
4. Verify fasta → MATCH, knucleotide → MATCH
5. Verify cancel_math, queue_drain still at 0x MATCH
6. Check all other benchmarks for regression
7. `bash benchmarks/build_and_bench.sh --runtime`
8. Compare nbody_newton ratio against pre-fix
