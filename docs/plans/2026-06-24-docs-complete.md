# Documentation and Examples — Complete the Picture

**Date:** 2026-06-24
**Scope:** All missing feature docs + missing standalone examples

## Cross-Reference Summary

| Feature | Doc | Example | Priority |
|---------|-----|---------|----------|
| sigcall (`Expr::SigCall`, `SigModifier`) | ❌ None | ❌ None | **High** |
| subtype projections (`:> filter/map/sort/...`) | ❌ None | ⚠️ Used in stdlib | **High** |
| `link` dependencies | ❌ None | ❌ None | **High** |
| `rstruct` component definition | ❌ None | ✅ `counter.rbv` etc. | **High** |

| `block` expressions | ❌ None | ✅ Used everywhere | **Med** |
| `tuple` / tuple destructure | ❌ None | ✅ `multi_output.bv` | **Med** |
| `pattern` matching subsystem | ❌ None | ✅ `error-handling.bv` | **Med** |
| `ellipsis` in slices (`..`) | ❌ None | ⚠️ In multislice code | **Med** |
| `<-` arrow mutation | ❌ None | ✅ `arrow-mutation.bv` | **Med** |
| Data Briev (`.dbv`/`.dbvl`/`.dbvs`) | ❌ None | ✅ `data-briev/` | **Med** |
| `within N cycles` timeout | ❌ None | ✅ `timeout_test.ebv` | **Med** |
| `wake` trigger modifier | ❌ None | ⚠️ Benchmarks only | **Med** |
| visibility (`pub`/`pvt`/`sed`) | ✅ `visibility.md` | ❌ None | **Med** |
| `rsrc` / resource declarations | ❌ None | ❌ None | **Low** |

## Execution Plan

### Phase A — HIGH: Write 4 feature docs + 4 examples

1. **`sigcall.md`** + `examples/sigcall-demo.bv`
   - Explain `sig` declarations as executable signatures
   - Cover `#safe`, `#out`, `#pure` modifiers
   - Show sig-as-first-class-type, sig dispatch, sig destructuring

2. **`subtype.md`** + `examples/subtype-projections.bv`
   - Cover all `SubtypeOp` variants: FILTER, MAP, SORT, UNIQUE, JOIN, GROUP, COUNT, SUM, AVG, MIN, MAX, SKIP, LIMIT, MATCH, etc.
   - Show chained subtype projections: `list :> filter(pred) :> map(fn) :> sort()`

3. **`link.md`** + `examples/link-demo/`
   - Cover `import "link/foo.o"`, `import "link/foo.c"`, link languages
   - Show how to link C object files, static libs, WASM modules

4. **`rstruct.md`**
   - Document `rstruct` component syntax: state, transactions as methods, embedded HTML view
   - Reference existing `counter.rbv` + `shopping_cart.rbv` as examples

### Phase B — MEDIUM: Write 8 feature docs

5. **`block.md`** — `Expr::Block`, block-as-expression semantics, block value rules
6. **`tuple.md`** — `Expr::Tuple`, tuple type syntax `(Int, String)`, tuple indexing `:> 0`, destructuring `let (a, b) = expr`
7. **`pattern.md`** — `Pattern` enum: literal patterns, variant patterns, wildcard, tuple patterns, field patterns. Separate from match/uni.
8. **`ellipsis.md`** — `..` in multislice coordinates, `Expr::Ellipsis`, `SliceCoordinate::Ellipsis`
9. **`arrow.md`** — `<-` push/pop/discard/transfer for all collection types. Dispatch on value type.
10. **`dbvl.md`** — Data Briev: `.dbv` data, `.dbvs` schema, `.dbvl` lines format, validation
11. **`within.md`** (or add to `statement.md`) — `within N cycles` / `within N ms` timeout syntax on assignments
12. **`visibility.md`** example — Update existing doc or add `examples/visibility-demo.bv` with `pub`, `pvt`, `sed` field examples

### Phase C — LOW: Stretch items

13. **`wake.md`** or add to `trg-dirty-flag.md` — `wake` modifier on triggers
14. **`examples/resource-demo.bv`** — `rsrc` / resource declarations

## File Manifest

### Docs to create:
```
docs/architecture/features/sigcall.md
docs/architecture/features/subtype.md
docs/architecture/features/link.md
docs/architecture/features/rstruct.md
docs/architecture/features/block.md
docs/architecture/features/tuple.md
docs/architecture/features/pattern.md
docs/architecture/features/ellipsis.md
docs/architecture/features/arrow.md
docs/architecture/features/dbvl.md
```

### Docs to update:
```
docs/architecture/features/statement.md       — add `within` timeout section
docs/architecture/features/visibility.md      — add example code
docs/architecture/features/trg-dirty-flag.md  — add `wake` modifier section
```

### Examples to create:
```
examples/sigcall-demo.bv
examples/subtype-projections.bv
examples/link-demo/           (directory with .bv + .c + optional walkthrough)
examples/visibility-demo.bv
examples/resource-demo.bv
```

## Execution Order

1. Phase A docs + examples (sigcall, subtype, link, rstruct)
2. Phase B docs (block, tuple, pattern, ellipsis, arrow, dbvl, within, visibility)
3. Phase C (wake, resource)
4. `cargo build` + `cargo test --lib` after each phase
