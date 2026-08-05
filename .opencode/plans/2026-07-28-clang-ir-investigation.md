# Compiler Theory Investigation: How Clang Optimizes the Benchmarks

**Date:** 2026-07-28
**Type:** Pure research — no code changes until we understand
**Scope:** LLVM source code analysis, clang IR comparison, assembly diffing

---

## Why This Investigation

Our hand-rolled SLP vectorization accidentally helps nbody_newton and accidentally
hurts nbody_sqrt_idio. We tuned heuristics (chain_pass_ok, is_reduction_pattern,
depth*width threshold) without understanding WHY the vectorization is beneficial
for one pattern and harmful for another.

**The correct approach:** Understand how clang (the production compiler) handles
these same computation patterns. Clang emits LLVM IR from C source that goes
through the SAME backend (llc, -O3, -ffast-math). If clang's IR is different
from ours, the difference is in IR QUALITY, not in available optimization passes.

Then instead of guessing which heuristics to tune, we can adopt clang's approach
directly: emit the IR that clang would emit for the same pattern.

## Research Questions

### Q1: What IR does clang produce for nbody_newton? (High priority)

```bash
clang -O3 -S -emit-llvm -march=native -ffast-math \
    benchmarks/nbody_newton_c.c -o /tmp/clang_nbody.ll
```

- Does clang emit `willreturn` on any function?
- Does clang emit `memory(argmem: readwrite)` or `memory(readwrite)`?
- Does clang use hand-rolled SLP (<3 x float> ops) or rely on the auto-vectorizer?
- What attributes does clang put on the outer loop function?

**Source:** `benchmarks/nbody_newton_c.c`

### Q2: What does clang's assembly look like? (High priority)

```bash
clang -O3 -S -march=native -ffast-math \
    benchmarks/nbody_newton_c.c -o /tmp/clang_nbody.s
objdump -d /tmp/nbody_newton_c > /tmp/clang_nbody_disasm.txt
```

Compare against our compiled binary's assembly.

### Q3: What attributes does clang put on its hot loop functions? (Medium)

- Search for `willreturn` in clang's C FE: `llvm-project/clang/lib/CodeGen/`
- When does clang emit `willreturn`? Only for known-pure functions, or for all
  functions by default?
- Does clang add `noundef` to function parameters? When?

```bash
# Search clang's source for willreturn emission
grep -r "willreturn" /usr/lib/llvm-*/include/ /usr/lib/llvm-*/lib/ | head -20
```

### Q4: How does LLVM's SLP vectorizer decide profitability? (Medium)

- Location: `llvm/lib/Transforms/Vectorize/SLPVectorizer.cpp`
- The function `isGather` / `isVectorized` / `BoUpSLP::vectorizeTree`
- LLVM's cost model considers: target instruction costs, register pressure,
  lane dependency, gather/scatter overhead
- How does LLVM's cost model differ from ours?

### Q5: When does SROA succeed or fail? (Medium)

- Location: `llvm/lib/Transforms/Scalar/SROA.cpp`
- The function `shouldPromote` / `tryToSimplify` / `deleteDeadInstructions`
- What conditions cause SROA to abort (not promote an alloca)?
- Does `memory(argmem: readwrite)` vs `memory(readwrite)` matter?

### Q6: What makes nbody_newton (Newton) so different from nbody_sqrt? (High)

- Both compute the SAME N-Body physics
- Both have 33 state fields
- Both have `when count % 5000000 == 0 { term! -> PrintLn!(energy); }`
- Newton uses while-loop for convergence; sqrt uses sqrt approximation
- The ONLY difference is the core computation: Newton iteration vs sqrt/distance

The question: does the Newton iteration (which has a convergence loop inside the
main loop) prevent LLVM from vectorizing, requiring our SLP to fill the gap?

## Procedure

### Phase 1: Reverse-Engineer Clang's IR

```bash
# 1. Generate IR from each C benchmark
for c_file in benchmarks/*_c.c; do
    clang -O3 -S -emit-llvm -march=native -ffast-math \
        "$c_file" -o "/tmp/clang_$(basename ${c_file%.c}).ll"
done

# 2. Count vector ops, attributes, function count
grep -c "<[0-9] x float>" /tmp/clang_*.ll
grep -c "willreturn\|noundef\|dereferenceable\|argmem" /tmp/clang_*.ll
grep "^define" /tmp/clang_*.ll

# 3. Compare against our Briv-generated .ll
diff <(grep -v '^;' /tmp/clang_nbody_newton.ll) \
     <(grep -v '^;' benchmarks/nbody_newton.ll) | head -200
```

### Phase 2: Diff Assembly Output

```bash
# Briv binary assembly
objdump -d /tmp/nbody_newton > /tmp/briv_nbody.s
# C reference assembly
objdump -d /tmp/nbody_newton_c > /tmp/clang_nbody.s

# Compare hot loop regions
diff /tmp/briv_nbody.s /tmp/clang_nbody.s | head -300
```

### Phase 3: Trace LLVM's Decision Pass

```bash
# Run opt with remarks to see what LLVM decided
opt -O3 -S \
    -pass-remarks-missed=sroa,slp-vectorizer,loop-vectorize \
    briv_nbody.ll -o /dev/null 2>&1 | head -50
opt -O3 -S \
    -pass-remarks-missed=sroa,slp-vectorizer,loop-vectorize \
    clang_nbody.ll -o /dev/null 2>&1 | head -50
```

Compare the remarks. LLVM will tell us EXACTLY why it vectorized (or didn't)
each loop, and why SROA promoted (or didn't) each alloca.

### Phase 4: LLVM Source Dive

```bash
# Find LLVM include/lib directories
ls /usr/lib/llvm-*/include/llvm/
ls /usr/lib/llvm-*/lib/Transforms/Vectorize/SLPVectorizer.cpp

# Search for key functions
grep -n "getEntryCost\|isGather\|vectorizeTree\|buildTree" \
    /usr/lib/llvm-*/lib/Transforms/Vectorize/SLPVectorizer.cpp | head -20
```

## Expected Insights

| Finding | Implication | Action |
|---------|------------|--------|
| clang uses `willreturn` on hot loops | We need `willreturn` on `#11` | Re-evaluate revert |
| clang's IR has fewer GEP+load chains | Our per-field phi path creates too many GEPs | Improve phi emission |
| clang's SLP cost model blocks <3xfloat> groups | Our SLP is too aggressive | Widen is_reduction_pattern |
| clang uses `memory(argmem: readwrite)` everywhere | Our `#9` on @main is too conservative | Design separate @main dispatch |
| clang's assembly has fewer mov instructions | Our IR produces more register spills | Attribute tuning |

## Deliverables

1. A table: "Clang vs Briv IR comparison" for all 19 benchmark C sources
2. A file: clang-generated `.ll` files for each C benchmark (saved to `docs/reference-ll/clang/`)
3. A file: LLVM pass-remarks for both Briv and clang IR
4. A written analysis: "The 3-5 things clang does differently from Briv"

## Timeline

- Phase 1 (clang IR generation): ~5 minutes (19 files × 15s each)
- Phase 2 (assembly comparison): ~5 minutes
- Phase 3 (pass remarks): ~5 minutes
- Phase 4 (LLVM source dive): ~20 minutes
- Analysis writeup: ~15 minutes

Total: ~50 minutes of research, zero code changes until we understand.

## Files to Create

```
docs/research/clang-ir-comparison.md        — Final analysis
docs/reference-ll/clang/                     — Clang IR for all C benchmarks
docs/reference-ll/clang/*.opt.ll             — Opt + pass-remarks output
```
