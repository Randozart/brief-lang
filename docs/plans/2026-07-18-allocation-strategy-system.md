# Allocation Strategy System

**Date:** 2026-07-18
**Status:** Plan — pre-implementation
**Depends on:** None (fixes existing bugs + additive stdlib)
**See also:**
  - `docs/plans/2026-06-23-arena-allocation.md` (arena Phase 1-3, implemented)
  - `docs/plans/2026-07-12-alloc-metadata.md` (superseded — allocation strategies are pure-Brief types)
  - `docs/plans/2026-07-18-ptr-level3-borrow-checking.md` (followup — provenance tracking refines Alloc# escape analysis)
  - `docs/architecture/llvm-memory-management.md` (current memory architecture)
  - `docs/architecture/arrow-syntax-and-arena.md` (arrow + arena spec)

---

## Executive Summary

Make allocation strategies expressible at two levels of abstraction:

1. **Explicit strategies** as pure-Brief types (`Arena`, `CrosswordArena`) — user controls which allocator by picking the type. Written in `.bv` files using existing `Malloc#`/`Free#`/Ptr arithmetic. No new intrinsics needed for these.

2. **`Alloc#(size)` intrinsic** — compiler-delegated allocation. The compiler analyzes the execution graph (contract bounds, lifetimes, txn scopes) and picks the optimal strategy automatically: arena bump inside txns, `@malloc` outside, `alloca` for bounded locals.

This is enabled by fixing three bugs in the `InsertAt`/`ExtractFrom` property pipeline that currently break `<-` on ALL collection types (List, RingBuffer, Queue, etc.), and by wiring the existing arena preallocation infrastructure into active codegen paths.

### Key Principles

- **No new intrinsics for explicit allocators.** `Arena` and `CrosswordArena` are pure Brief using existing primitives, exactly like `ring_push`/`ring_pop`.
- **`Alloc#` is a single delegation point** — one intrinsic that says "compiler decides." Strategy hints and per-allocation metadata are not needed.
- **`Malloc#` stays** for explicit heap semantics (FFI, `--no-stdlib`).
- **Free# dispatch via static analysis** — at compile time, the compiler annotates each `Free#` call with the strategy (arena = no-op, malloc = `@free`). No runtime tag bits.
- **Escape → promote to heap + info message.** When `Alloc#` result escapes the txn scope, promote to `@malloc` and emit info on stderr and as LLVM IR comment.
- **Stdlib arena types use `Alloc#` internally** — enabling nested arena optimization.

### Dependency Graph

```
Phase 1: Fix InsertAt/ExtractFrom ──────────────────────┐
    (normalizer key keep, dispatch key casing,           │
     collect_push_targets, wire prealloc)                │
                                                         ├──→ Phase 4: All testable together
Phase 2: Alloc# intrinsic + Free# static analysis ───────┤
    (intrinsic_signatures, backend emit, interpreter,     │
     AllocStrategy enum, register annotation)             │
                                                         │
Phase 3: Stdlib arena types (pure Brief) ────────────────┘
    (arena.bv, crossword.bv, property bindings)          │
                                                         ├──→ Phase 5: Verify no regressions
Phase 4: Execution-graph → strategy selection ───────────┘
    (analysis pass, codegen annotation, info messages)   │
                                                         ↓
Phase 5: Benchmark baseline + verification ──────────────┘
```

---

## Phase 1: Fix InsertAt/ExtractFrom Property Pipeline

Three targeted fixes with no architectural changes. The `InsertAt`/`ExtractFrom` property system is the mechanism that lets any type hook into `<-` push/pop via property bindings (`InsertAt <~ my_function`). It's designed to be fully generic, but three bugs make it silently broken for ALL types.

### 1a. Stop stripping InsertAt/ExtractFrom in normalizer

**File:** `src/backend/llvm/normalizer.rs:99-105`

The normalizer strips metadata properties that the LLVM backend "doesn't use." The keep list currently has 5 keys:

```rust
let keep: HashSet<String> = ["ctd", "alu", "llvm_type", "encoding", "layout"]
    .iter().map(|s| s.to_string()).collect();
```

Add `"InsertAt"` and `"ExtractFrom"` to the keep set. These ARE used by the LLVM backend — `check_insert_strategy` and `check_extract_strategy` in `emit_toplevel.rs` read them to dispatch `<-` operations.

```rust
// 2026-07-18: Keep InsertAt/ExtractFrom — used by arrow dispatch in emit_stmt
let keep: HashSet<String> = ["ctd", "alu", "llvm_type", "encoding", "layout",
    "InsertAt", "ExtractFrom"]
    .iter().map(|s| s.to_string()).collect();
```

**Test:** `test_insert_at_survives_normalizer`
- Register a typedef with `InsertAt <~ ring_push`
- Run normalizer
- Assert `rt.properties.get("InsertAt") == Some(Identifier("ring_push"))`

### 1b. Fix property key casing in dispatch

**File:** `src/backend/llvm/emit_toplevel.rs:107-132`

The parser stores property keys in the exact casing from source (`"InsertAt"`, PascalCase). The dispatch lookups use `"insert_at"` (snake_case). Two functions need fixing:

```rust
// Line 115: change
.and_then(|rt| rt.properties.get("insert_at"))
// to:
.and_then(|rt| rt.properties.get("InsertAt"))

// Line 130: change
.and_then(|rt| rt.properties.get("extract_from"))
// to:
.and_then(|rt| rt.properties.get("ExtractFrom"))
```

**Test:** `test_insert_strategy_dispatch`
- Create a state field with type `RingBuffer<Int>` (has `InsertAt <~ ring_push`)
- Call `check_insert_strategy("field_name")`
- Assert returns `Some(Identifier("ring_push"))`

**Test:** `test_extract_strategy_dispatch`
- Same pattern for `ExtractFrom <~ ring_pop`
- Assert returns `Some(Identifier("ring_pop"))`

### 1c. Fix collect_push_targets to extract actual field names

**File:** `src/backend/llvm/mod.rs:746-758`

The current implementation only recurses into `Guarded`/`Block`/`SyncBlock` but never matches any statement that contains a push. In the new AST, `<- push` on a list field `queue <- val;` parses as `Statement::Assign(Expr::Identifier("queue"), val_expr)`.

Fix `collect_push_targets` to match `Assign` with an `Identifier` LHS:

```rust
pub(crate) fn collect_push_targets(body: &[Statement], out: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Statement::Guarded(_, statements) => {
                collect_push_targets(statements, out);
            }
            Statement::Block(body) | Statement::SyncBlock(body) => {
                collect_push_targets(body, out);
            }
            // 2026-07-18: Push is Assign(Ident(field), rhs) in new AST.
            // The LHS identifier is the push target field name.
            // Over-approximation is safe — preallocation for a non-push
            // field just allocates an unused buffer (freed at tick end).
            Statement::Assign(Expr::Identifier(name), _) => {
                out.push(name.clone());
            }
            _ => {}
        }
    }
}
```

**Note:** This intentionally over-approximates (matches ALL `Assign(Ident, _)`, not just push). The arena preallocation is harmless for non-push fields — it allocates a buffer that won't be used, freed at tick end. Over-approximation is safe.

**Test:** `test_collect_push_targets_assign`
- Feed `[Statement::Assign(Expr::Identifier("qty".into()), rhs)]`
- Assert `out == vec!["qty"]`

**Test:** `test_collect_push_targets_nested`
- Feed guarded block containing assign
- Assert field name is found

### 1d. Wire emit_prealloc_for_body into active codegen paths

**Files:**
- `src/backend/llvm/loop_engine/counter.rs` (around line 65)
- `src/backend/llvm/loop_engine/ssa.rs` (around line 403)
- `src/backend/llvm/dispatch.rs:78` (sequential reactor)
- `src/backend/llvm/dispatch.rs:326` (parallel reactor)
- `src/backend/llvm/emit_toplevel.rs:1318` (reactive txn)

`emit_prealloc_for_body` and `emit_prealloc_for_targets` (defined at `mod.rs:1087-1117`) are never called. The `bound_reg` is computed in the loop engine paths. Wire them:

**In `counter.rs` (around line 97, after loop body emission):**

The `bound_reg` is computed at line 66-67. After the body is emitted (line 104), call:

```rust
if let Some(stmts) = body {
    let mut push_targets: Vec<String> = Vec::new();
    crate::backend::llvm::collect_push_targets(stmts, &mut push_targets);
    push_targets.sort();
    push_targets.dedup();
    if !push_targets.is_empty() {
        self.emit_prealloc_for_targets(out, "  ", &push_targets, &bound_reg);
    }
}
```

This call goes BEFORE the body emission loop so preallocated buffers exist when body statements reference them. Place it right before `emit_countable_body` or the body emission block.

**In `ssa.rs:emit_folded_multi_main` (around line 403):**

`bound_reg` is computed at line 403-420. After the loop setup (`.fm_loop:` header at line 421-431) and before the txn dispatch loop, collect push targets from all txn bodies and preallocate:

```rust
let mut all_targets: Vec<String> = Vec::new();
for (_name, txn) in txns {
    crate::backend::llvm::collect_push_targets(&txn.body, &mut all_targets);
}
all_targets.sort();
all_targets.dedup();
if !all_targets.is_empty() {
    self.emit_prealloc_for_targets(out, "  ", &all_targets, &bound_reg);
}
```

**In `dispatch.rs` (sequential reactor, line 78):**

After `emit_arena_init`, check if we have a counter field and bound from analysis. If yes, collect push targets from all txns and preallocate:

```rust
// 2026-07-18: Preallocate push targets if bounded
if let Some(bound) = self.get_reactor_bound() {
    let mut targets = Vec::new();
    for (_, txn) in &txns {
        crate::backend::llvm::collect_push_targets(&txn.body, &mut targets);
    }
    targets.sort();
    targets.dedup();
    if !targets.is_empty() {
        self.emit_prealloc_for_targets(out, "  ", &targets, &bound);
    }
}
```

The helper `get_reactor_bound()` checks if the reactor's contract provides a bound (extracting from `total_idx` or `total_const_name` analysis available in dispatch context).

**In `dispatch.rs` (parallel reactor, line 326):** Same pattern as sequential reactor.

**In `emit_toplevel.rs:1318`:** If the reactive txn has a contract-derived bound, preallocate. This is optional for the initial implementation — reactive txns with dynamic bounds will use arena bump for per-element allocation.

---

## Phase 2: Alloc# Intrinsic + Free# Static Analysis

### 2a. Register Alloc# in intrinsic signatures

**File:** `src/intrinsic_signatures.rs:81-86`

Add `Alloc#` alongside `Malloc#`:

```rust
// ── Memory (observable) ─────────────────────────────────────
"Malloc#"  => Some(Signature { name: "Malloc#",  parameters: vec![("size", Type::int())], return_kind: ReturnKind::Exact(Type::ptr(Type::bits(1))), observable: true }),
"Free#"    => Some(Signature { name: "Free#",    parameters: vec![("ptr", Type::ptr(Type::bits(1)))], return_kind: ReturnKind::Exact(Type::void()), observable: true }),
"Alloc#"   => Some(Signature { name: "Alloc#",   parameters: vec![("size", Type::int())], return_kind: ReturnKind::Exact(Type::ptr(Type::bits(1))), observable: true }),
```

Same signature as `Malloc#`. `Alloc#` differs only in codegen strategy selection.

### 2b. Add AllocStrategy enum on TypedRegister

**File:** `src/backend/llvm/mod.rs` — add to `FunctionContext` or as a standalone enum

```rust
/// 2026-07-18: Allocation strategy for a pointer-typed register.
/// Used by emit_alloc (to decide how to allocate) and by emit_free
/// (to decide whether to emit @free or no-op).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AllocStrategy {
    /// Bump-allocated from the per-txn arena. Free# is a no-op.
    Arena,
    /// Heap-allocated via @malloc. Free# must call @free.
    Malloc,
    /// Stack-allocated via alloca. Free# is a no-op.
    Alloca,
}
```

Add a field to `TypedRegister` (or to a side-channel map):

```rust
// In TypedRegister (src/backend/llvm/mod.rs or its own file):
pub struct TypedRegister {
    pub reg: String,
    pub ty: Type,
    pub strategy: Option<AllocStrategy>,  // 2026-07-18
}
```

If modifying `TypedRegister` is too invasive, use a `HashMap<String, AllocStrategy>` keyed by register name on `FunctionContext`.

### 2c. Backend emit_alloc function

**File:** `src/backend/llvm/intrinsics.rs` — add `emit_alloc` called from `emit_intrinsic_call`

Triple dispatch:

```rust
/// 2026-07-18: Emit Alloc#(size) — compiler chooses optimal strategy.
/// Inside an arena-scoped txn → arena bump (inline GEP + overflow check).
/// Inside a bounded scope with no escape → alloca (stack).
/// Otherwise → @malloc (heap).
fn emit_alloc(&mut self, out: &mut String, indent: &str, args: &[TypedRegister]) -> Result<TypedRegister, String> {
    let size = self.get_arg_i64_reg(out, indent, args, 0, "Alloc#")?;

    // Strategy 1: Arena scope active → bump allocate
    if self.fun.arena_slots.is_some() {
        let result = self.emit_arena_alloc(out, indent, &size);
        return Ok(TypedRegister {
            reg: result,
            ty: Type::ptr(Type::bits(1)),
            strategy: Some(AllocStrategy::Arena),
        });
    }

    // Strategy 2: Bounded + no escape → alloca
    // (escape check is simplified — true escape analysis is in Phase 4)
    if self.is_in_bounded_scope() && !self.will_escape_current_allocation() {
        let r = self.fun.next_reg_with_prefix("aa");
        writeln!(out, "{}{} = alloca i8, i64 {}", indent, r, size).ok();
        return Ok(TypedRegister {
            reg: r,
            ty: Type::ptr(Type::bits(1)),
            strategy: Some(AllocStrategy::Alloca),
        });
    }

    // Strategy 3: Default → @malloc
    let r = self.fun.next_reg_with_prefix("aa");
    writeln!(out, "{}{} = call noalias ptr @malloc(i64 {})", indent, r, size).ok();
    Ok(TypedRegister {
        reg: r,
        ty: Type::ptr(Type::bits(1)),
        strategy: Some(AllocStrategy::Malloc),
    })
}
```

**Helper stubs (Phase 4 fills them in):**

```rust
/// 2026-07-18: Check if we're in a scope with known bound (contract-proven).
fn is_in_bounded_scope(&self) -> bool {
    self.fun.contract_bound.is_some()
}

/// 2026-07-18: Check if the allocation result escapes the current scope.
/// Simplified: checks if the result register is stored to %State or returned.
/// Full provenance-based check comes in Ptr Level 3 followup.
fn will_escape_current_allocation(&self) -> bool {
    false // conservative: assume no escape until Phase 4
}
```

Wire into `emit_intrinsic_call`:

```rust
// In emit_intrinsic_call, add before the "Malloc#" fallthrough:
"Alloc#" => {
    return self.emit_alloc(out, indent, args);
},
```

### 2d. Free# static analysis dispatch

**File:** `src/backend/llvm/intrinsics.rs` — add `emit_free` or modify the existing `Free#` handler

When `Free#(ptr)` is emitted:

1. Look up `ptr` register's `AllocStrategy` annotation (from `TypedRegister.strategy` or the side-channel map)
2. Strategy-driven dispatch:

```rust
/// 2026-07-18: Emit Free#(ptr) — strategy-aware.
/// If the pointer was arena-allocated, Free# is a no-op
/// (arena reset at tick end handles it).
/// If the pointer was stack-allocated, Free# is a no-op
/// (stack memory dies at scope end).
/// If the pointer was heap-allocated, emit @free.
fn emit_free(&mut self, out: &mut String, indent: &str, args: &[TypedRegister]) -> Result<TypedRegister, String> {
    let ptr_reg = &args[0];

    // Check allocation strategy
    let strategy = ptr_reg.strategy.as_ref()
        .or_else(|| self.fun.alloc_strategies.get(&ptr_reg.reg));

    match strategy {
        Some(AllocStrategy::Arena) | Some(AllocStrategy::Alloca) => {
            // 2026-07-18: Arena/stack allocations don't need per-element free.
            // Arena reset at scope end reclaims all arena memory.
            // Stack memory dies at scope end.
            Ok(TypedRegister::void())
        }
        _ => {
            // Heap-allocated or unknown → emit @free
            let r = self.fun.next_reg_with_prefix("fr");
            writeln!(out, "{}call void @free(ptr {})", indent, ptr_reg.reg).ok();
            Ok(TypedRegister::void())
        }
    }
}
```

Wire into `emit_intrinsic_call` for the `"Free#"` arm:

```rust
"Free#" => {
    return self.emit_free(out, indent, args);
},
```

### 2e. Interpreter handler

**File:** `src/interpreter/intrinsics.rs:114` — add `Alloc#` handler

```rust
"Alloc#" => {
    let size = arg_as_i64(args, 0)?;
    let addr = heap.allocate(size as usize);
    Ok(i64_to_bits(addr as i64))
}
```

Same implementation as `Malloc#` (the interpreter's VirtualHeap handles both). The strategy distinction is only relevant in codegen.

**Test:** `test_alloc_interpreter`
- Interpret `Alloc#(32)`, check that the returned address is in the heap's allocation range

### 2f. Propagate strategy through let-bindings and copies

**File:** `src/backend/llvm/mod.rs` — in the statement emission path for `Statement::Assign`

When a register is copied to a new let-binding, propagate the `AllocStrategy`:

```rust
// In emit_statement, when handling Assign(lhs, rhs):
if let Some(strategy) = rhs_reg.strategy {
    self.fun.alloc_strategies.insert(new_reg.clone(), strategy.clone());
}
```

This ensures that `Free#(let_bound_ptr)` correctly traces back to the original allocation strategy, even through copies.

---

## Phase 3: Stdlib Arena Types (Pure Brief)

### 3a. Basic Bump Arena

**File:** `lib/std/memory/arena.bv`

```brief
// 2026-07-18: Arena — single-direction bump allocator in pure Brief.
// Backed by Alloc# for the backing buffer. The arena is itself
// arena-allocated when arena_init is called inside a txn context
// (nested arena — the parent arena manages the child arena's backing
// buffer). Arena lifecycle:
//   1. arena_init(cap) — allocate backing buffer via Alloc#
//   2. arena_alloc(a, size) — bump pointer forward
//   3. arena_reset(a) — rewind offset to 0 (reuse memory)
//   4. arena_free(a) — free backing buffer via Free#
// The [pre][post] contract on arena_alloc prevents overflow.
type Arena {
    base: Ptr<Byte>,
    offset: Int,
    capacity: Int,
};

// Allocate backing buffer via Alloc#. If called inside a txn with
// an active arena, the user's arena is nested within the txn's arena:
// the outer arena gives memory for the inner arena's backing buffer.
defn arena_init(cap: Int) -> Arena {
    let base: Ptr<Byte> = Alloc#(cap) as Ptr<Byte>;
    term Arena { base, offset: 0, capacity: cap };
};

// Bump-allocate `size` bytes from the arena. Contract ensures
// offset never exceeds capacity. If contract violated at compile
// time, the compiler catches it. At runtime, unpredictable.
defn arena_alloc(a: Arena, size: Int) -> Ptr<Byte> {
    let ptr: Ptr<Byte> = a.base + a.offset;
    a.offset = a.offset + size;
    [a.offset <= a.capacity]; // contract: no overflow
    term ptr;
};

// Rewind offset to 0. The backing buffer stays live — memory is
// reused on the next cycle. No system call.
defn arena_reset(a: Arena) {
    a.offset = 0;
};

// Free the backing buffer. If the arena was nested within a txn's
// arena, Free# is a static-analysis no-op (arena memory managed by
// outer arena). If standalone, calls @free.
defn arena_free(a: Arena) {
    Free#(a.base as Int);
};
```

### 3b. Crossword Arena (Dual Direction)

**File:** `lib/std/memory/crossword.bv`

```brief
// 2026-07-18: CrosswordArena — dual-direction arena allocator.
// Fixed-size slots grow from base upward (slot_offset).
// Variable-length data (strings, byte buffers) grows from
// (base + capacity) downward (string_offset).
// The two regions meet in the middle — a collision means
// the arena is full and a contract violation is triggered.
//
// This pattern mirrors the "crossword puzzle" layout from
// high-performance game engines and network parsers:
//   [ Slot 1 | Slot 2 | ... | String B | String A | Free ]
//   ^                                  ^                  ^
//   base                            string_offset    base+capacity
//   slot_offset ->
//                                    <- string_offset
//
// CrosswordArena<T> : List<T> — supports the same <- push/pop
// interface as List and RingBuffer, via InsertAt/ExtractFrom
// property bindings.
type CrosswordArena<T> : List<T> {
    InsertAt <~ crossword_push;
    ExtractFrom <~ crossword_pop;
};

// Initialize with total capacity. slot_offset starts at 0
// (bottom of arena). string_offset starts at capacity (top
// of arena). Both grow toward each other.
defn crossword_init(cap: Int) -> CrosswordArena<Byte> {
    let base: Ptr<Byte> = Alloc#(cap) as Ptr<Byte>;
    term CrosswordArena {
        base,
        capacity: cap,
        slot_offset: 0,
        string_offset: cap,
    };
};

// Allocate a fixed-size slot from the bottom of the arena.
// Bumps slot_offset forward. Contract ensures no collision
// with the string region.
defn crossword_alloc_slot(ca: CrosswordArena, size: Int) -> Ptr<Byte> {
    let ptr: Ptr<Byte> = ca.base + ca.slot_offset;
    ca.slot_offset = ca.slot_offset + size;
    [ca.slot_offset <= ca.string_offset]; // collision guard
    term ptr;
};

// Allocate variable-length data from the top of the arena.
// Decrements string_offset backward. Contract ensures no
// collision with the slot region.
defn crossword_alloc_string(ca: CrosswordArena, len: Int) -> Ptr<Byte> {
    ca.string_offset = ca.string_offset - len;
    let ptr: Ptr<Byte> = ca.base + ca.string_offset;
    [ca.slot_offset <= ca.string_offset]; // collision guard
    term ptr;
};

// List interface: push = alloc slot, store value
// handle is ptrtoint of the CrosswordArena struct
defn crossword_push(handle: Int, val: Int) -> Int {
    let ca: Ptr<CrosswordArena> = handle as Ptr<CrosswordArena>;
    let slot: Ptr<Int> = crossword_alloc_slot(*ca, 8) as Ptr<Int>;
    *slot = val;
    ca.slot_offset = ca.slot_offset; // commit the bump
    term 0;
};

// List interface: pop = read last slot, rewind slot_offset
defn crossword_pop(handle: Int) -> Int {
    let ca: Ptr<CrosswordArena> = handle as Ptr<CrosswordArena>;
    ca.slot_offset = ca.slot_offset - 8;
    let val: Int = *((ca.base + ca.slot_offset) as Ptr<Int>);
    term val;
};

// Rewind both offsets. Backing buffer stays live.
defn crossword_reset(ca: CrosswordArena) {
    ca.slot_offset = 0;
    ca.string_offset = ca.capacity;
};

// Free backing buffer. Same nested-arena optimization as
// Arena.free: Free# on Alloc#-allocated memory is a
// static-analysis no-op when nested.
defn crossword_free(ca: CrosswordArena) {
    Free#(ca.base as Int);
};
```

### 3c. Export bindings

**File:** `lib/std/types.bv` — add re-export

```brief
// 2026-07-18: Arena allocator types
import arena from "std/memory/arena.bv";
import crossword from "std/memory/crossword.bv";
```

Or if types.bv doesn't use imports this way, add a `lib/std/memory/mod.bv`:

```brief
// 2026-07-18: Memory allocator module
import Arena from "std/memory/arena.bv";
import CrosswordArena from "std/memory/crossword.bv";
```

(Check existing import patterns in `lib/std/` for the convention.)

---

## Phase 4: Execution-Graph → Strategy Selection

### 4a. Add an AllocationAnalysis pass

**File:** `src/analysis/allocation.rs` (new)

```rust
/// 2026-07-18: Analyze Alloc# call sites in the execution graph and
/// determine the optimal allocation strategy. This runs before codegen
/// and annotates each call site with the chosen strategy.
///
/// The analysis considers:
/// - Is the Alloc# inside an arena-scoped txn? → Arena
/// - Is the result local (not stored to %State or returned)? → Alloca
/// - Otherwise → Malloc (conservative)
///
/// Future: provenance-based escape analysis (see Ptr Level 3 followup)
/// replaces the simple "stored to state" check with full pointer tracing.
```

The analysis walks the program's expressions and identifies `Expr::Call("Alloc#", ...)` sites. For each, it checks:

1. Is this call inside a transaction body (arena scope)?
2. Does the result escape the scope? (Simple: is the result register stored to a state field or returned from the txn?)
3. If bounded scope + no escape → Alloca
4. If arena scope + no escape → Arena
5. Otherwise → Malloc

**Output:** A `HashMap<String, AllocStrategy>` keyed by register name (or expression hash) that codegen reads.

**File:** `src/analysis/mod.rs` — register the module

```rust
pub mod allocation;
```

**File:** `src/compile.rs` — run the pass

```rust
// After type checking, before codegen:
let alloc_strategies = analysis::allocation::analyze(&program)?;
```

### 4b. Resolve Alloc# in codegen from analysis output

**File:** `src/backend/llvm/intrinsics.rs` — `emit_alloc` reads the pre-computed strategy

Instead of the triple-dispatch guessing in Phase 2c, read from the analysis:

```rust
fn emit_alloc(&mut self, out: &mut String, indent: &str, args: &[TypedRegister]) -> Result<TypedRegister, String> {
    let size = self.get_arg_i64_reg(out, indent, args, 0, "Alloc#")?;

    // Read strategy from analysis pass output
    let strategy = self.fun.alloc_strategies.get(&self.current_expr_id())
        .cloned()
        .unwrap_or(AllocStrategy::Malloc); // conservative default

    match strategy {
        AllocStrategy::Arena => {
            let result = self.emit_arena_alloc(out, indent, &size);
            Ok(TypedRegister {
                reg: result, ty: Type::ptr(Type::bits(1)),
                strategy: Some(AllocStrategy::Arena),
            })
        }
        AllocStrategy::Alloca => {
            let r = self.fun.next_reg_with_prefix("aa");
            writeln!(out, "{}{} = alloca i8, i64 {}", indent, r, size).ok();
            Ok(TypedRegister {
                reg: r, ty: Type::ptr(Type::bits(1)),
                strategy: Some(AllocStrategy::Alloca),
            })
        }
        AllocStrategy::Malloc => {
            let r = self.fun.next_reg_with_prefix("aa");
            writeln!(out, "{}{} = call noalias ptr @malloc(i64 {})", indent, r, size).ok();
            Ok(TypedRegister {
                reg: r, ty: Type::ptr(Type::bits(1)),
                strategy: Some(AllocStrategy::Malloc),
            })
        }
    }
}
```

### 4c. Info messages on promotion

When `Alloc#` inside an arena/stack scope gets promoted to heap (because the result escapes), emit:

**Stderr:**

```
info: Alloc#(32) at foo.bv:12 promoted to heap — result escapes txn scope
```

**LLVM IR comment:**

```llvm
; info: Alloc#(32) at foo.bv:12 promoted to heap — result escapes txn scope
%aa1 = call noalias ptr @malloc(i64 32)
```

**File:** `src/backend/llvm/intrinsics.rs` — in `emit_alloc`

```rust
// When strategy differs from the context-default:
let context_default = if self.fun.arena_slots.is_some() {
    AllocStrategy::Arena
} else if self.is_in_bounded_scope() {
    AllocStrategy::Alloca
} else {
    AllocStrategy::Malloc
};

if strategy != context_default && context_default != AllocStrategy::Malloc {
    // Promotion occurred — emit info
    let msg = format!("Alloc#({}) at {}:{} promoted to heap — result escapes txn scope",
        size, self.current_source_file(), self.current_source_line());
    eprintln!("info: {}", msg);
    writeln!(out, "; info: {}", msg).ok();
}
```

### 4d. When information is unavailable

If the analysis pass hasn't run (e.g., `--no-analysis` flag or legacy path), fall back to the simple triple-dispatch from Phase 2c:

```rust
// Fallback: no analysis output available
if strategy == AllocStrategy::Malloc && self.fun.arena_slots.is_some() {
    // Conservative: use arena only if we're definitely in one
    let result = self.emit_arena_alloc(out, indent, &size);
    return Ok(TypedRegister { ... strategy: Some(AllocStrategy::Arena) });
}
```

---

## Phase 5: Benchmark Baseline & Verification

### 5a. Pre-implementation baseline

Per Directive §11, run BEFORE any code changes:

```bash
cargo build --release && bash benchmarks/build_and_bench.sh --runtime
```

Capture ALL output in a table:

| Benchmark | Brief (s) | C (s) | Ratio (Brief/C) | Correctness |
|-----------|-----------|-------|-----------------|-------------|
| ... | ... | ... | ... | PASS/FAIL |

### 5b. Expected improvements

| Benchmark | Why it improves |
|-----------|-----------------|
| **Any with `<- push` in bounded loop** | Preallocation now works (was dead code) |
| **RingBuffer-based benchmarks** | InsertAt/ExtractFrom dispatch fixed (was silently broken) |
| **Arena-heavy reactive programs** | Alloc# uses arena bump instead of malloc per op |

### 5c. Post-implementation verification

Run the same command after all phases:

```bash
cargo build --release && bash benchmarks/build_and_bench.sh --runtime
```

Compare against baseline. No benchmark should regress. RingBuffer benchmarks should show improvement (fixing the broken dispatch). This plan must be updated with the new results.

---

## File Manifest

### New Files

| File | Purpose |
|------|---------|
| `lib/std/memory/arena.bv` | Pure-Brief bump arena type |
| `lib/std/memory/crossword.bv` | Pure-Brief crossword arena type |
| `lib/std/memory/mod.bv` | Memory module re-export |
| `src/analysis/allocation.rs` | Execution-graph → strategy analysis pass |
| `docs/plans/2026-07-18-allocation-strategy-system.md` | This plan |
| `docs/plans/2026-07-18-ptr-level3-borrow-checking.md` | Followup: borrow checker |

### Modified Files

| File | Change |
|------|--------|
| `src/intrinsic_signatures.rs:81` | Add `Alloc#` signature |
| `src/backend/llvm/normalizer.rs:99-105` | Keep `InsertAt`/`ExtractFrom` properties |
| `src/backend/llvm/emit_toplevel.rs:115,130` | Fix property key casing |
| `src/backend/llvm/mod.rs:746-758` | Fix `collect_push_targets` |
| `src/backend/llvm/mod.rs` | Add `AllocStrategy` enum + strategy propagation |
| `src/backend/llvm/intrinsics.rs` | Add `emit_alloc`, `emit_free` strategy dispatch |
| `src/backend/llvm/emit_stmt.rs` | Wire Free# static analysis |
| `src/backend/llvm/loop_engine/counter.rs` | Call `emit_prealloc_for_targets` |
| `src/backend/llvm/loop_engine/ssa.rs` | Call `emit_prealloc_for_targets` |
| `src/backend/llvm/dispatch.rs` | Call `emit_prealloc_for_body` if bounded |
| `src/backend/llvm/emit_toplevel.rs` | Optional: prealloc in reactive txn init |
| `src/interpreter/intrinsics.rs` | Add `Alloc#` handler |
| `src/analysis/mod.rs` | Register `allocation` module |
| `src/compile.rs` | Run allocation analysis pass |
| `lib/std/types.bv` | Memory module re-export |

---

## Testing Strategy

All tests are behavioral (per Directive §5) — they assert outcomes, not IR snapshots.

### Phase 1 Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_insert_at_survives_normalizer` | `InsertAt` property present after normalization | Register typedef, run normalizer, check properties |
| `test_insert_strategy_dispatch` | `check_insert_strategy` returns correct fn name | State field with `RingBuffer<Int>`, call check, assert `Some(Identifier("ring_push"))` |
| `test_extract_strategy_dispatch` | Same for extract | Same pattern for `ExtractFrom <~ ring_pop` |
| `test_collect_push_targets_assign` | Finds `Assign(Ident, _)` field names | Feed statement with push, assert field name |
| `test_collect_push_targets_nested` | Recurses into Guarded/Block | Feed nested push, assert field name |
| `test_arrow_push_on_ringbuffer` | End-to-end: `<- push` on RingBuffer field | Compile txn with ring buffer push, run, verify value |
| `test_arrow_pop_on_ringbuffer` | End-to-end: `<- pop` returns correct value | Push then pop, verify round-trip |

### Phase 2 Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_alloc_signature` | Alloc# registers with correct sig | Call `get_intrinsic_signature("Alloc#")`, check params/return |
| `test_alloc_inside_txn_arena` | Alloc# inside txn emits arena bump | Compile txn with `Alloc#(32)`, check IR for `getelementptr` (bump) not `@malloc` |
| `test_alloc_outside_txn_malloc` | Alloc# outside txn emits `@malloc` | Compile `defn` with `Alloc#(32)`, check IR for `call @malloc` |
| `test_alloc_escapes_to_malloc` | Alloc# result stored to state → heap | Compile txn where result escapes, check IR for `@malloc` |
| `test_free_of_arena_alloc` | Free# on arena pointer → no `@free` | Compile, check IR has no `call @free` for that pointer |
| `test_free_of_malloc_alloc` | Free# on heap pointer → `@free` | Compile, check IR emits `call @free` |
| `test_alloc_strategy_propagates` | Strategy survives let-binding | `let p = Alloc#(32); Free#(p)` — no `@free` if arena-allocated |
| `test_alloc_interpreter` | Alloc# works in interpreter | `Alloc#(32)` returns valid virtual address |

### Phase 3 Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_arena_init_pure_brief` | Arena type allocates correctly | Compile `arena_init(1024)`, run, check non-null base |
| `test_arena_alloc_bump` | Arena returns increasing addresses | `arena_alloc(a, 16)` twice, assert addresses differ by 16 |
| `test_arena_reset` | Reset reuses memory | Alloc, reset, alloc again — same address |
| `test_crossword_init_pure_brief` | Crossword arena initializes | `crossword_init(1024)`, check slot_offset=0, string_offset=1024 |
| `test_crossword_slot_vs_string` | Slots grow up, strings grow down | Alloc slot → `slot_offset` increases. Alloc string → `string_offset` decreases |
| `test_crossword_collision` | Overflow triggers contract violation | Allocate beyond capacity → contract error |
| `test_arrow_push_on_crossword` | `<- push` on crossword arena field works | Full integration: txn with `&ca <- val` |
| `test_arrow_pop_on_crossword` | `<- pop` returns correct value | Push then pop, assert round-trip |

### Phase 4 Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_alloc_auto_strategy_txn` | Alloc# inside txn → analysis picks Arena | Run analysis pass, check strategy annotation |
| `test_alloc_auto_strategy_defn` | Alloc# in defn → analysis picks Malloc | Run analysis pass, check strategy annotation |
| `test_alloc_info_message` | Promotion emits stderr + IR comment | Capture stderr during compile, check for "promoted to heap" |
| `test_alloc_no_info_no_promotion` | No promotion → no message | Alloc# inside txn, result not stored to state → no message |

### Regression Tests

```
cargo test --lib  — all existing tests must pass
```

---

## Documentation

### Inline doc comments to add

| File | What | Rationale |
|------|------|-----------|
| `normalizer.rs:100` | Update keep set comment | `// 2026-07-18: +InsertAt, +ExtractFrom — arrow dispatch` |
| `emit_toplevel.rs:115` | Fix key string + comment | `// 2026-07-18: "InsertAt" matches parser output` |
| `mod.rs:746` | Replace `collect_push_targets` body | `// 2026-07-18: Match Assign(Ident, _) for push targets` |
| `mod.rs:1048` | Update `emit_prealloc_one_field` doc | Document it's called from loop entry |
| `mod.rs` | `AllocStrategy` enum | Full provenance: when added, why, how to extend |
| `intrinsics.rs:emit_alloc` | Triple dispatch | When each strategy is chosen |
| `intrinsics.rs:emit_free` | Static analysis | How strategy annotation prevents redundant `@free` |
| `analysis/allocation.rs` | Module doc | Full description of the analysis, limitations, future improvements |
| `lib/std/memory/arena.bv:1` | Module-level doc | Full provenance for arena type |
| `lib/std/memory/crossword.bv:1` | Module-level doc | Full provenance for crossword type |

### Architecture docs to update

| Document | What changes |
|----------|-------------|
| `docs/architecture/arrow-syntax-and-arena.md` | Add section: "Pure-Brief Arena Types" — document that Arena and CrosswordArena are stdlib types expressible in pure Brief, not compiler intrinsics. Cross-reference with ring_push pattern. |
| `docs/architecture/llvm-memory-management.md` | Add §21: "Alloc# Intrinsic" — triple dispatch (arena/malloc/alloca), Free# static analysis, info-on-promote. Add §22: "Pure-Brief Allocator Types" — how Alloc# enables nested arena. |
| `docs/architecture/intrinsics-vs-stdlib.md` | Add Alloc# as compiler-delegation intrinsic (passes `--no-stdlib` test). Arena types as stdlib. |
| `docs/plans/2026-07-12-alloc-metadata.md` | Update status: "Superseded — allocation strategies are pure-Brief types; alloc metadata is unneeded." |

### Rationale comments at every modified site

Every edit gets a `// 2026-07-18: <why>` comment explaining the fix, pattern targeted, and how to undo if obsolete. Arena type files get provenance comments at the type and each function definition.

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Property key casing** — future parser changes could change `slot_name` casing | Low | Medium — dispatch silently fails | Add a `normalize_property_key()` helper that both parser and dispatch use, ensuring consistent casing. Document the convention. |
| **collect_push_targets over-approximation** — matches ALL `Assign(Ident, _)`, including non-push | Medium | Low — harmless buffer allocation | Harmless: preallocation allocates a buffer that won't be used. The arena is freed at tick end. Tracked as BUGS.md note. |
| **Alloc# escape analysis too conservative** — always assumes escape, always promotes to malloc | Medium | Medium — Alloc# degrades to Malloc# | This is safe (correct behavior, just not optimal). Phase 4 refines with provenance tracking. The Ptr Level 3 followup fixes this. |
| **Crossword arena preallocation** — crossword allocates from both directions, `emit_prealloc_one_field` assumes single-direction | Low | Medium — preallocated capacity wasted on one direction | Preallocate full arena capacity once (not per-direction). Crossword manages internal split. Preallocation just ensures backing buffer is large enough. |
| **Existing benchmarks regress** — fixing InsertAt changes behavior for RingBuffer | Low | High — silent semantic change | All changes are additive or bugfix-only for existing paths. Run benchmark baseline before and after. Any regression is a signal of a deeper bug, not expected. |
| **Alloc# in nested scope** — user writes `Alloc#` in a `defn` called from a txn | Medium | Low — conservative fallback | Static analysis traces through call graph. If the defn is inlined, Alloc# sees the txn's arena. If not inlined, `@malloc`. Conservative and safe. |
| **Free# on phi node** — pointer from either Alloc# or Malloc# depending on control flow | Low | Low — conservative fallback | Static analysis: if all predecessors have the same strategy, use it; otherwise use `@free` (conservative). |

---

## Followup: Ptr Level 3 Borrow Checker

See `docs/plans/2026-07-18-ptr-level3-borrow-checking.md` for the next phase of work.

The borrow checker's provenance tracking will replace the simple escape analysis in Phase 4 with full pointer-origin tracing, making `Alloc#` strategy selection more precise. Key connections:

| Allocation system need | Borrow checker provides |
|------------------------|------------------------|
| `Alloc#` escape → promote | `Provenance::FieldAccess` detection — pointer stored to state |
| `Free#` static dispatch | `infer_provenance()` — walk expression tree to find allocation origin |
| Arena lifetime tracking | `Provenance::Known("arena_field")` — pointer tied to arena scope |
| Cross-txn pointer safety | `check_convergence_safety()` — prevent dangling pointers across tick |

The two systems are independent: the allocation strategy system works correctly without the borrow checker, but benefits from its refinements.
