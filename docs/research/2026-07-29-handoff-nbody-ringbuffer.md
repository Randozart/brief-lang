# Handoff: nbody_newton & ring_buffer Investigation

## Baseline Context

**Current baseline**: `32e5a24a` (this commit) — worktree at `../briev-compiler-baseline`
**All 19/19 benchmarks MATCH**.
**Previous baseline**: `b39461e2` — removed, replaced by the above.

---

## Investigation 1: nbody_newton (1.23x C — Briev loses)

### Status
- **Ratio**: 1.23x C (10.2063s vs 8.2850s)
- **Correctness**: MATCH (output: "-0.169203")
- **Dispatch**: PerFieldPhi (31 per-field scalar phi nodes)
- **Pre-existing**: MISMATCH predates all recent changes (dispatch guardrail, RHS mapping fix, etc.)

### Best Ever
- **0.75x** at Era 5 (commit `8a827db`, Jul 11) — Briev BEAT C by 25%
- Era 5 worktree available at `../briev-compiler-era5`

### What's Been Tried

1. **Dispatch guardrail** (`88818123`): Fixed fasta/knucleotide. No effect on nbody.
2. **RHS mapping fix** (`066b86a7`): Fixed `statements_isomorphic` to build mapping from both LHS+RHS of Assignment. nbody's velocity/position assignments now correctly detected as isomorphic. **Keep this** — it's correct and tested.
3. **Vector phi emission enablement** (rolled back): Produced NaN output (wrong field grouping) and clang segfault (mandelbrot). See `docs/research/2026-07-29-vector-phi-investigation.md` for full details.
4. **SLP disable experiment**: Eliminated LLVM's `<4 x float>` vector phis, but independent recompilation produced wrong output. Trend: SLP seems to add ~6% overhead for nbody.

### Root Cause (Phase 4 regression)

Phase 4 removed two things that the 0.75x Era-5 version used:

1. **`-slp-vectorize-hor=false`** compiled flag — passed via `llvm_extra_flags()` in baseline `b39461e2` when `slp_hazard_fns` was non-empty.
2. **SLP hazard analysis** (`src/backend/llvm/hazard.rs`) — computed peak live float values to decide SLP-disabling attributes. Removed entirely.

Without these, LLVM's SLP vectorizer creates 6 `<4 x float>` vector phis from nbody's 31 scalar phis, adding extractelement/insertelement overhead that outweighs any vector compute benefit.

### What To Investigate Next

1. **Trace Era-5 IR structure**: Compare Era-5's `nbody_newton.ll` (worktree at `../briev-compiler-era5`) with current to see exact IR differences. Era-5 used our own vector phi groups + hazard-gated SLP-disable. What IR produced 0.75x?

2. **Restore `-slp-vectorize-hor=false`**: Check if simply adding back `--mllvm -slp-vectorize-hor=false` closes the gap. The baseline had this. Does it work alone, or only in combination with Era-5's own vector phis?

3. **Revisit vector phi groups**: The current `detect_vector_groups` infrastructure (with all safety fixes) is correct but groups semantically unrelated fields. Manual group specification via config might be needed — nbody's 5 body-pairs need bx/by/bz/vx/vy/vz groups, not the isomorphism analysis's random groupings.

4. **Register-pressure dispatch**: Counter.rs's `emit_countable_main` always uses PerFieldPhi (31 scalar phis). A store-back mode (like the old `emit_while_main`) might be better for >16 fields where register pressure causes spills. The dead code for this was just removed in `94be0897`.

### Key Files

| File | Relevance |
|------|-----------|
| `benchmarks/nbody_newton.bv` | Benchmark source (359 lines, 31 state fields) |
| `src/backend/llvm/loop_engine/counter.rs` | `emit_countable_main` — PerFieldPhi dispatch |
| `src/backend/llvm/mod.rs` | Dispatch tree (~line 2692) — VectorPhi vs InlineSsa vs PerFieldPhi |
| `src/backend/llvm/vector_phi.rs` | Vector phi infrastructure (safety checks done, not enabled) |
| `src/analysis/slp_isomorphism.rs` | `analyze_body` + `statements_isomorphic` — RHS mapping fix applied |
| `docs/research/2026-07-29-vector-phi-investigation.md` | Full investigation record |
| `docs/plans/2026-07-29-vector-phi-assign-isomorphism.md` | Plan doc with benchmark table |

---

## Investigation 2: ring_buffer (1.11x C — Briev loses)

### Status
- **Ratio**: 1.11x C (0.0564s vs 0.0505s)
- **Correctness**: MATCH
- **Not investigated yet** — this is fresh.

### Benchmark Structure (44 lines)

```briev
const CAP: Int = 1024;
const TOTAL: Int = 50000000;

let data: Ptr<Int> = Malloc#(CAP * 8);
let head: Int = 0;
let tail: Int = 0;
let ops: Int = 0;

node enqueue [ops < TOTAL][ops == TOTAL] {
    data[tail % CAP] = ops;
    tail = tail + 1;
    ops = ops + 1;
    when ops % 5000000 == 0 {
        let filled: Int = tail - head;
        let buf_val: Int = data[tail % CAP];
        PrintLn!(filled + buf_val);
    };
    when ops == TOTAL {
        let chk: Int = data[0] + data[512];
        PrintLn!(chk);
    };
    term;
};
```

Key observations:
- Single node (`enqueue`), NOT reactive (no separate dequeue — the comment says "Two reactive transactions" but the `.bv` only has one node)
- 4 state fields: `data` (Ptr<Int>), `head` (Int), `tail` (Int), `ops` (Int)
- `data` is a Malloc#-allocated pointer stored as Ptr<Int> → i64 in state
- `data[idx]` involves: `ptrtoint` → `getelementptr` → load/store (the `Expr::Index` lowering)
- Periodic print at 5M iterations + final checksum print

### Hypothesized Bottlenecks (speculative — unverified)

1. **Pointer boxing**: `Malloc#` returns `Ptr<Int>`, which is stored as i64 in state. Each access requires `inttoptr` + GEP. C uses native pointers directly.
2. **Modular arithmetic**: `tail % CAP` every iteration. Could be strength-reduced, but Briev's IR emits `srem` which LLVM should optimize.
3. **State field phi overhead**: 4 fields (data, head, tail, ops) is small (< 8) and should get InlineSsa dispatch, which has the write_set bug fix. Likely fine.
4. **Ptr dereference cost**: `data[tail % CAP]` goes through `Expr::Index` → `inttoptr` + GEP + load/store. This is the main loop body.

### Suggested First Steps

1. **Check dispatch path**: Compile with `--report` or check the `.ll` file for `.fmain.` vs `%ppf` labels.
2. **Profile the binary**: Use `perf stat -e cycles,instructions,cache-misses` to find the bottleneck. Compare against C binary.
3. **IR comparison**: Compile with baseline and current, diff the `.ll` files for the main loop. Look for `inttoptr`/`ptrtoint` pairs — these are the pointer boxing overhead.
4. **Check if Malloc# gets optimized out**: With `when ops % 5000000 == 0 { PrintLn!(...) }`, the periodic print should keep the buffer writes alive. Verify the swan song pattern works correctly.
5. **Check for `needs_state_stores_in_body`**: The periodic print + final print may trigger post-loop hoisting, causing state stores. Check if these stores add overhead.

### Key Files

| File | Relevance |
|------|-----------|
| `benchmarks/ring_buffer.bv` | Benchmark source (44 lines) |
| `benchmarks/ring_buffer_c.c` | C reference implementation |
| `src/backend/llvm/emit_expr.rs` | `Expr::Index` lowering (`inttoptr` + GEP) |
| `src/backend/llvm/helpers.rs` | `adapt_to_i64` — pointer boxing/unboxing |
| `src/backend/llvm/loop_engine/counter.rs` | Dispatch + phi generation |
| `src/backend/llvm/mod.rs` | Dispatch tree decision |

---

## Shared Tools

### Baseline comparison
```bash
bash benchmarks/compare_baseline.sh <benchmark_name>
```
Compiles and times on both `../briev-compiler-baseline` and current worktree.

### LLVM diagnostic commands
```bash
# Check SROA failures
opt -O3 -pass-remarks-missed=sroa unopt.ll -disable-output 2>&1
# Check vectorization failures
opt -O3 -pass-remarks-missed=loop-vectorize unopt.ll -disable-output 2>&1
# Check SLP vectorization
opt -O3 -pass-remarks-missed=slp-vectorizer unopt.ll -disable-output 2>&1
# All optimization remarks
opt -O3 -pass-remarks-missed=sroa,gvn,licm,loop-vectorize unopt.ll -disable-output 2>&1
```

### Running benchmarks
```bash
bash benchmarks/build_and_bench.sh --correctness   # Correctness only (uses existing binaries)
bash benchmarks/build_and_bench.sh --runtime       # Build + time all benchmarks
bash benchmarks/build_and_bench.sh --optimizer     # Precompute-only benchmarks
```

### Build & Test
```bash
cargo test --lib              # Unit tests
cargo build --release         # Release binary (what benchmarks use)
```

### Key Permissions
- **Never `git checkout -- <file>` or `git restore`**: Destroys uncommitted work.
- **Never modify existing match arms in optimization paths**: Additive changes only.
- **Always run `cargo test --lib` before committing**.
- **Update `/tmp` temp directory is safe** for scratch files.
