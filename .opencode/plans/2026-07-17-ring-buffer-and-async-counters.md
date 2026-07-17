# Ring Buffer & Async Counters — Architecture & Benchmark Design

## Current State Assessment

### Pointer/Ring Buffer Infrastructure

| Component | Status | What's Broken |
|-----------|--------|---------------|
| `Type::Ptr(T)` | ✅ Defined in types.rs | — |
| `Malloc#` / `Free#` | ✅ Intrinsics exist | Returns `i64` (should be `Ptr`) |
| `data[idx]` read | ⚠️ Partially works | +1 offset for list-header (raw buffer reads off-by-one) |
| `data[idx] = val` write | ❌ **Silent no-op** | `emit_stmt.rs` only handles `Identifier` LHS in Assign |
| `&` address-of operator | ❌ Never parsed | `Token::Ampersand` exists but no parser rule |
| Ptr state store/load | ✅ Works by accident | `adapt_to_i64` fallthrough passes raw ptrtoint bits |

### Async Dispatch Infrastructure

| Component | Status | What's Broken |
|-----------|--------|---------------|
| `async_txn_names` population | ✅ Works (conflict analysis) | Ignores user `is_async` flag |
| `DispatchMode::Parallel` | ✅ Selected correctly | Reactor returns no-op when async |
| `emit_async_body` | ✅ Emits functions | Functions are emitted but never called |
| `__thread_pool_init__` | ❌ **No-op** | C runtime body is empty |
| `__barrier_release__/__wait__` | ❌ **No-op** | Stub only |
| Parser `rct async txn` | ❌ Not consumed | Keyword token exists, parser skips it |
| `is_async` → optimizer | ❌ Disconnected | Flag ignored in analysis |
| True parallel benchmarks | ❌ Non-functional | Dead metadata only |

---

## Benchmark Designs

### 1. Real Ring Buffer (`benchmarks/ring_buffer.bv`)

```brief
let data: Ptr<Int> = Malloc#(1024 * 8);
let head: Int = 0;
let tail: Int = 0;

txn enqueue(value: Int) [tail - head < 1024][tail - head == 0] {
    data[tail % 1024] = value;
    tail = tail + 1;
    term;
};

txn dequeue() [tail - head > 0][tail - head == 0] -> Int {
    let val: Int = data[head % 1024];
    head = head + 1;
    term val;
};
```

### 2. Real Async Counters (`benchmarks/async_counters.bv`)

```brief
const N: Int = 50000000;
let a: Int = 0;
let b: Int = 0;
let report: Int = 0;

rct txn inc_a [a < N][a == N] {
    a = a + 1;
    [a % 10000000 == 0] { PrintInt#(a); };
    term;
};

rct txn inc_b [b < N][b == N] {
    b = b + 1;
    [b % 10000000 == 0] { PrintInt#(b); };
    term;
};
```

---

## Implementation Plan

### Phase 1: Fix Pointer Write Path (`emit_stmt.rs`)
Fix `Statement::Assign(Expr::Index(obj, idx), rhs)` — currently silently drops the write.

### Phase 2: Fix Ptr Index Read Offset (`emit_expr.rs`)
Remove the +1 list-header offset from `Expr::Index` Ptr read path.

### Phase 3: Fix Malloc# Return Type (`intrinsics.rs`)
Change return type from `Type::int()` to `Type::ptr(Type::int())`.

### Phase 4: Fix Parser for `rct async txn` (`definitions.rs`)
Add `eat(&Token::Async)` before consuming `txn` token.

### Phase 5: Wire `is_async` to Optimizer (`optimizer.rs`)
Skip conflict-free requirement when `is_async` is true.

### Phase 6: Implement Real Thread Pool (C Runtime)
Implement `__thread_pool_init__`, `__barrier_release__`, `__barrier_wait__`.

### Phase 7: Write the Actual Benchmarks
Replace placeholder benchmarks with real ring buffer + async counters.

### Phase 8: Fix Remaining Correctness Mismatches
Fix `print_loop` (148M lines, SSA path) and verify all pass.

---

## Regression Guard Checklist

- [ ] `cargo test --lib` — all 913+ tests pass
- [ ] `precompute_sum` still produces 249500
- [ ] `iir_filter` compiles and runs
- [ ] `float_math` compiles and runs
- [ ] `const_heavy BOUND=5` matches C
- [ ] New ring buffer compiles, runs, matches C reference
- [ ] New async counters compiles, produces matching output
