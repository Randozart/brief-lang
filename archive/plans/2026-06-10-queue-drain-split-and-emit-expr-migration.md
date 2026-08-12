# Plan: queue_drain Symmetric Split + emit_expr Feature Migration

## Part 1 — queue_drain Symmetric/Idiomatic Split

### Background
The current `queue_drain.bv` uses a reactive transaction with collection ops
(`<- &queue` pop, `&queue <- count` push), while `queue_drain_c.c` is just a
counter loop (no collections). They compute the same output (count at periodic
checkpoints) but through fundamentally different algorithms. Per the symmetric
guideline, these should be split into two variants.

### Changes

#### Create `queue_drain_sym.bv` (symmetric — mirrors C)
Straight-line counter loop matching C's algorithm:
```briev
frgn __print_int(n: Int) -> Bool ;
let N: Int = __get_env_int("BOUND");
let count: Int = 0;
node work [count < N][count == N] {
    &count = count + 1;
    [count % 5000000 == 0] {
        __print_int(count);
    };
    term;
};
```

#### Keep `queue_drain_c.c` → rename to `queue_drain_sym_c.c`
Same as current `queue_drain_c.c` — counter-only loop with periodic print.

#### Create `queue_drain_idio.bv` (idiomatic — Briev-native)
Rename current `queue_drain.bv` → `queue_drain_idio.bv`:
Reactive txn with `<- &queue` + `&queue <- count` — exercises collection
drain analysis in the compiler.

#### No `queue_drain_idio_c.c`
The idiomatic variant tests if Briev's optimizer can find a better path.
No C reference needed (compared against `_sym`'s C for correctness, or
compared internally in the harness).

#### Update `build_and_bench.sh`
Add both variants to the benchmark list with `--runtime` tag (both have
FFI in the guard path — `__print_int` is not in the hot loop body but
is reachable).

#### Cleanup
Existing `queue_drain.bv` / `queue_drain_c.c` — keep as-is for now
(backward compatibility), or remove if no other scripts reference them.

### Effort: ~1 hour

---

## Part 2 — emit_expr → Feature File Migration

### Current State
`emit_expr.rs`: 897 lines, ~46 match arms in `emit_expr()`.
Feature file `ExprCodegenLLVM` status:

| Feature File | Expr variant | Status | Lines to migrate |
|---|---|---|---|
| `literal.rs` | Integer, Bool, Float, String, Char, Term | ✅ Real impl | Term arm needs adding (~1 line) |
| `binary_op.rs` | BinaryOp (Add, Sub, etc.) | ✅ Real impl | Stub arms dead if AST migrated |
| `unary_op.rs` | UnaryOp (Not, Neg) | ✅ Real impl | Neg needs float handling (~15 lines) |
| `call.rs` | Call | ✅ Real impl | FFI dispatch complete |
| `collection.rs` | ListLiteral, ListIndex, Slice, MultiSlice, MapLiteral, SetLiteral | ❌ Stubs | ~136 lines total |
| `field.rs` | FieldAccess, StructInstance, ObjectLiteral | ❌ Stubs | ~48 lines total |
| `pattern.rs` | PatternMatch, Match | ❌ Stubs | ~65 lines total |
| `tuple.rs` | Tuple, TupleDestructure | ❌ Stubs | ~22 lines total |
| `projection.rs` | Projection (Size, Bytes, etc.) | ❌ Stub | ~14 lines (Size only) |
| `arrow.rs` | ArrowMut, ArrowDiscard, ArrowTransfer | ❌ Stubs | — (handled by statement emit, not expr emit) |

### Strategy

**Phase A**: Real implementations in feature files (adds missing codegen).
One feature file per cycle:
1. `collection.rs` — ListLiteral, ListIndex (~34 lines)
2. `collection.rs` — Slice, MultiSlice (~100 lines)
3. `field.rs` — FieldAccess, StructInstance, ObjectLiteral (~48 lines)
4. `pattern.rs` — PatternMatch, Match (~65 lines)
5. `tuple.rs` — Tuple, TupleDestructure (~22 lines)
6. `projection.rs` — Projection Size (~14 lines)
7. `unary_op.rs` — Neg float handling (~15 lines)
8. `literal.rs` — Term arm (~1 line)

Each phase:
1. Copy the real code from emit_expr.rs into the feature file's `ExprCodegenLLVM` impl
2. Translate `Box<Expr>` fields to feature struct fields
3. Replace the old-style match arm in emit_expr.rs with delegation to feature struct
4. `cargo test --lib` — verify

**Phase B**: Cleanup dead old-style match arms in emit_expr.rs.
After all old-style Expr variants have feature struct equivalents:
- Remove `Expr::Add`, `Expr::Sub`, ... arms (now dead if AST no longer produces them)
- Remove `Expr::Integer`, `Expr::Bool`, `Expr::Float` ... arms (delegated via Expr::Literal)
- Keep: `Expr::Identifier`, `Expr::OwnedRef`, `Expr::PriorState` (state access — stays)

**Keep in emit_expr.rs**: 
- `Expr::Identifier` + OwnedRef/PriorState (complex state access)
- `emit_precomputed_main`, metadata emission, `emit_binop`, `emit_fcmp`,
  `emit_cast_convert`, `i64_to_float_reg`, `resolve_fusable_pairs`

### Target End State
`emit_expr.rs` shrinks from 897 to ~400 lines (Identifier logic + helpers).
Feature files gain ~300 lines of real codegen across 5 files.

### Effort: ~8 cycles × 0.5-1 hour each = ~4-6 hours total

---

## Schedule

1. queue_drain split (1 hour)
2. Phase A cycles 1-3 (2 hours) — collections + field access (highest impact)
3. Phase A cycles 4-6 (2 hours) — pattern, tuple, projection
4. Phase A cycles 7-8 (0.5 hours) — unary, literal
5. Phase B cleanup (0.5 hours) — remove dead arms, verify tests
