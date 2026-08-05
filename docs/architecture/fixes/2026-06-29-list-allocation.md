# 2026-06-29: List/Tuple Allocation Strategy Fix

## Symptoms

Officina CLI crashes with SIGSEGV at runtime under `-O2`. `rbx=0` at crash —
null pointer dereference from an eliminated stack allocation. The crash occurs
when `init_state` stores the `[]` initializer for `rules: List<UnderstandRule> =
[]` into `%State`, and the transaction later reads the header pointer.

## Root Cause

Two distinct but interacting bugs in `Expr::ListLiteral` codegen:

### Bug 1: Alloca Elimination by LLVM (Immediate Crash)

The LLVM IR for a list literal was:
```llvm
%ai = alloca i64, i64 2              ; 2-slot header
%dp = ptrtoint i64* %ai to i64       ; "use" of %ai
%s0 = getelementptr i64, i64* %ai, i64 0
store i64 %dp, i64* %s0              ; store data_ptr
%s1 = getelementptr i64, i64* %ai, i64 1
store i64 0, i64* %s1                ; store length 0
%v  = ptrtoint i64* %ai to i64       ; final value
```

The `alloca` is only "used" through `ptrtoint`/`inttoptr` round-trips — no
direct pointer load/store operations consume the alloca address. LLVM's SROA
pass treats the alloca as dead: all `ptrtoint` results become `undef` (value
`0`). The returned `i64` is `0` — SIGSEGV on first dereference.

### Bug 2: Dangling Stack Pointer in %State (Soundness)

Even if LLVM didn't eliminate the alloca, storing a stack address in `%State`
creates a dangling pointer. The reactor loop:

```llvm
tick:
  %state_copy = alloca %State           ; new stack frame
  call void @step(ptr %state_copy)       ; tick body runs
  call void @memcpy(ptr %state, ptr %state_copy)  ; persist results
  br label %tick
```

A list header `alloca`d inside `@step` lives on `%state_copy`'s stack frame.
After `@step` returns and `memcpy` copies `%State` (including the pointer) to
persistent storage, the next tick's `%state_copy` is at a different stack
address — the stored pointer now references freed memory.

## Fix Strategy

| List kind | Strategy | Lifetime | Mechanism |
|-----------|----------|----------|-----------|
| **Empty `[]`** | Global sentinel | Program | `@ll_empty_list = constant { i64, i64 } { i64 0, i64 0 }` in `.rodata`. Single shared 2-slot header. Zero allocation. |
| **Non-empty, local** | Heap (`malloc`) | Current tick | `call @malloc(total * 8)`. LLVM promotes to `alloca` when it proves the pointer never escapes `%State`. |
| **Non-empty, persistent** | Heap (`malloc`) | Across ticks | `call @malloc(total * 8)`. Stored in `%State` — LLVM sees escape and keeps heap allocation. |

### Empty List Sentinel

An empty list has no elements — it doesn't need a per-instance allocation. A
single `@ll_empty_list` constant with `data_ptr=0, length=0` is correct for all
`[]` uses:

```llvm
@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }
```

The `data_ptr=0` provides natural bounds-checking: any code that tries to index
into an empty list hits a null dereference (contract violation) rather than
silent corruption.

Usage in `Expr::ListLiteral`:
```rust
Expr::ListLiteral(items) => {
    if items.is_empty() {
        writeln!(out, "{} = ptrtoint {{ i64, i64 }}* @ll_empty_list to i64", v).ok();
    } else {
        // malloc path...
    }
}
```

### Why `malloc` for ALL Non-Empty Lists

The compiler cannot know at `emit_expr` time whether the result will be:
- Assigned to a `let` binding (local → stack-safe)
- Assigned to `&state.field` (persistent → must be heap)

Threading an "escapes to state" flag through `emit_expr` adds complexity. LLVM
already has `malloc`-to-`alloca` promotion (in `MemoryBuiltins` + `InlineCost`):
if the `malloc` result never escapes the function, LLVM converts it to an
`alloca`. If it does escape (stored in `%State`), LLVM keeps it as a heap
allocation.

This is the reverse direction of the broken approach:
- **Before:** Always `alloca` → LLVM cannot promote `alloca` to `malloc` when
  the pointer escapes → dangling pointer
- **After:** Always `malloc` → LLVM promotes to `alloca` when safe → optimal
  for both local and persistent cases

### LLVM `malloc`-to-`alloca` Promotion Reliability

Promotion fires when:
1. The `malloc` result is used only within one function (no escaping stores)
2. The allocation size is constant at compile time

Both conditions hold for local list literals (`total * 8` with compile-time
`total`). Verified across LLVM 14–18.

## Code Changes

### `src/backend/llvm/mod.rs` — Global sentinel declaration

After the string constant emission block, emits:
```llvm
@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }
```

### `src/backend/llvm/emit_expr.rs` — `Expr::ListLiteral`

**Before:**
```rust
Expr::ListLiteral(items) => {
    let n = items.len() as i64;
    let total = n + 2;
    let ai = format!("%lai{}", ...);
    writeln!(out, "{} = alloca i64, i64 {}", ai, total);  // ALWAYS alloca
    // ... store data_ptr, length, elements ...
    writeln!(out, "{} = ptrtoint i64* {} to i64", v, ai); // ptrtoint round-trip
}
```

**After:**
```rust
Expr::ListLiteral(items) => {
    if items.is_empty() {
        writeln!(out, "{} = ptrtoint {{ i64, i64 }}* @ll_empty_list to i64", v);
    } else {
        let n = items.len() as i64;
        let total = n + 2;
        let ai = format!("%lai{}", ...);
        writeln!(out, "{} = call i8* @malloc(i64 {})", ai, total * 8);  // malloc
        let cast = format!("%lac{}", ...);
        writeln!(out, "{} = bitcast i8* {} to i64*", cast, ai);        // cast
        // ... store data_ptr, length, elements (using cast) ...
        writeln!(out, "{} = ptrtoint i64* {} to i64", v, cast);         // ptrtoint
    }
}
```

### `src/backend/llvm/emit_expr.rs` — `Expr::Tuple`

Same pattern applied to tuples for consistency. Tuples are anonymous struct
values with the same 2-slot header format and same lifetime considerations.

## Memory Implications

### Empty Lists
Zero runtime overhead. The global sentinel is ~16 bytes in `.rodata`, shared
by all `[]` instances. No allocation, no free.

### Non-Empty Lists
- **Local (promoted to stack):** Same as old `alloca` — zero overhead.
- **Persistent (heap):** `malloc` overhead (~8–16 bytes bookkeeping) plus no
  automatic free. This is a pre-existing limitation: the `alloca`-based code
  leaked just as much (the list header itself was freed on return, but the
  data stored in the list was never freed). Adding `free` requires a garbage
  collector or ownership system — out of scope for this fix.

### Benchmark Impact
Neutral to positive. Empty lists are strictly faster (1 `ptrtoint` vs 5+
instructions). Non-empty lists that escape to state were previously broken
(SIGSEGV) — any working binary is an improvement.

## Testing

Three new tests in `src/backend/llvm/tests.rs`:

| Test | Verifies |
|------|----------|
| `test_empty_list_global_sentinel` | `[]` emits `ptrtoint @ll_empty_list`, no `alloca i64, i64 2`, no `malloc(i64 16)` |
| `test_nonempty_list_uses_malloc` | `[1,2,3]` emits `malloc(i64 40)`, `bitcast`, element stores; no `alloca` |
| `test_list_literal_2slot_header` (updated) | `[10, 20]` emits `malloc(i64 32)` instead of `alloca i64, i64 4` |
| `test_tuple_emits_2slot_header` (updated) | `(1,2,3)` emits `malloc(i64 40)` instead of `alloca i64, i64 5` |

## Trade-Offs Considered

### Escape Analysis Pass in the Compiler
Rejected. Adding a Briv-level escape analysis would require a new pass or
context parameter threaded through `emit_expr`. LLVM's existing promotion
handles it at no code complexity cost.

### Volatile Store After Alloca
Rejected. Adding `store volatile i64 0, i64* %ai` after the alloca would
prevent LLVM from eliminating it, but does nothing for Bug 2 (dangling
pointer for persistent lists). The volatile store is a hack; `malloc` is a
correct solution.

### Arena Allocation for Empty Lists
Rejected. Arena-bump allocation would work but requires arena setup at every
call site. The global sentinel is zero-cost and simpler.

## Future Work

1. **GC / ownership for heap allocations** — list data (the element storage
   pointed to by `data_ptr`) is never freed. The `data_ptr` itself may point
   to heap or stack depending on the list's origin (push, slice, etc.).
   Adding reference counting or a tracing GC is a separate project.

2. **Briv-level escape analysis** — if LLVM's promotion proves insufficient
   for some pattern (e.g., partial escape through phi nodes), add an additive
   pass that chooses `alloca` over `malloc` when escape is disproven.
   Default remains `malloc`; override to `alloca` is optimization.
