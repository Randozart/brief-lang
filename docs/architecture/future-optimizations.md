# Future Optimization Opportunities

Date: 2026-07-29

## Performance

| # | Optimization | Impact | Effort | Status |
|---|-------------|--------|--------|--------|
| **5** | **Incremental eval caching** — store per-example results from pruning, reuse during `candidate_matches_all_examples` | ~2× at depth 4 | Medium | ✓ Implemented |
| **E** | **Adaptive beam width** — scale beam by depth | ~3× at depth 4 | Low | ✓ Implemented |
| **F** | **Candidate pre-screening** — check first example before all | ~3× at depth 3 | Low | ✓ Implemented |
| **B** | **Beam during generation** — generate candidates in cost order, stop after beam width | ~10× at depth 4+ | High | Pending |
| **C** | **Pruning by example requirement** — evaluate example contribution per candidate | ~2× | High | Not started |

## Correctness

| # | Optimization | Impact | Effort | Status |
|---|-------------|--------|--------|--------|
| ~~P~~ | **`<-` for assignment** — DROPPED (2026-08-01): `&i = i + 1` was never valid (`&` is genuine address-of); plain `i = i + 1` / `i += 1;` are the correct forms. | — | — | Done |
| **A** | **Expression hashing** — deduplicate equivalent sub-expressions | ~2× | Low | Not needed (symmetry breaking handles commutativity) |

## Depth 4+ Feasibility

The current bottleneck is depth 4+ search. The raw candidate count at depth 4
for a 1-param Int function is ~5M. Even with beam=16000, evaluating the beam
takes >2min.

**Beam during generation** (`B`) is the single highest-impact optimization:
instead of generating ALL candidates then keeping the cheapest N, generate
candidates in cost order and stop after N. This avoids the O(n log n) sort
and the memory allocation for all M candidates.

Implementation sketch:
- Replace `generate_next_level` with a callback-based generator that yields
  candidates in cost order (cheapest first)
- `synthesize_enumerative` counts candidates and stops after beam width
- No sort needed, no full Vec allocation for all M candidates

## Type-Level Synthesis

The synthesis engine currently only supports Int parameters. To synthesize
compiler passes (AST transformations), it needs:

| Feature | Status |
|---------|--------|
| Compound types (Call/Field/Match evaluation) | ✓ Done (Phase 2) |
| Compound type generation in `generate_next_level` | ✓ Done (Phase 5) |
| declare-datatypes in Z3 queries | ✓ Done (Phase 4) |
| Cross-type extraction (Expr → Int via Match) | ✓ Done |
| Cross-type generation (generating Match from Expr params) | ✓ Done |

The constant-folding pass (Expr → Expr) requires the engine to generate
`Call` and `Match` expressions at the same depth, which is still untested.
