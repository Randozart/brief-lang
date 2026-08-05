# Phase: LLVM Backend Re-Optimization — Contract-Driven SSA Codegen

**Date:** 2026-05-29
**Status:** Design Document
**Version:** 1.0

## 1. Philosophy: Safety as Optimization Fuel

Briv's three first-class safety features are not a runtime tax — they are
compile-time optimization intrinsics:

| Feature | Syntax | LLVM Optimization |
|---------|--------|-------------------|
| Precondition | `[count < 100]` | `!range` on load + `@llvm.assume` |
| Postcondition | `[@count + 1 == count]` | `nuw nsw` on arithmetic + dead store elimination |
| State mutation | `&field = expr` | `noalias nocapture` on `%State*` → `mem2reg` |
| Trigger sampling | `trg name: Type @ link sym` | Single `load volatile` per tick → deterministic SSA |

The compiler proves contracts at build time, then feeds the proven bounds into
LLVM's optimizer. The result: verified-correct code that LLVM can optimize
*more* aggressively than unverified C/Rust, because it has stronger invariants.

### The Inversion of the Safety Tax

In mainstream languages, safety features are an afterthought — runtime checks
that get stripped in release mode:

| Language | Safety Mechanism | Release Behavior |
|----------|-----------------|------------------|
| C/C++ | `assert()` | Stripped by `NDEBUG` — no protection |
| Rust | `bounds_check` | Compiler removes provably-safe checks, keeps others |
| Ada | `pragma Pre(...)` | Runtime assertion unless proven |
| **Briv** | `[pre][post]` | Proven at compile-time → converted to `!range` + `@llvm.assume` → **code gets faster** |

Because contracts are first-class syntactic citizens parsed directly into the
AST, the compiler's proof engine can statically verify them. Once proven:

1. Runtime checks are completely eliminated
2. Verified bounds are injected as `!range` metadata feeding LLVM's
   `ScalarEvolution`, LICM, and branch folding passes
3. Arithmetic gets `nuw nsw` flags (no overflow guard needed)
4. The resulting binary is *faster* than unverified code, because LLVM has
   *more* invariants to optimize against

This is the core architectural insight of Briv's LLVM backend: **safety
features pay a performance dividend.**

## 2. Performance Architecture (Three Tiers)

### Tier 1: Structural Guarantees — `noalias` / `nocapture`

**Cost:** Zero (guaranteed by language semantics — no `&` operator, no pointer
arithmetic, no reference escaping)
**Win:** Largest single optimization available to LLVM

Every `%State*` parameter carries `noalias nocapture`. This tells LLVM:

- **`noalias`**: Nothing written through any other pointer can affect memory
  reachable from `%State*`. Enables load-to-store forwarding, dead store
  elimination, and instruction reordering.
- **`nocapture`**: The pointer is not stored or returned. Enables allocas to be
  promoted to SSA registers.

Combined with `local_unnamed_addr` + `norecurse` + `willreturn`, LLVM's
`mem2reg` pass promotes ALL struct field loads to SSA registers for the
duration of the transaction. The result:

```llvm
; Without noalias: every field access is load-from-memory
%a = load i64, i64* %field_a_ptr
store i64 %new_a, i64* %field_a_ptr
%b = load i64, i64* %field_b_ptr  ; must re-load — a might alias b!

; With noalias: fields are SSA registers
%a = load i64, i64* %field_a_ptr  ; loaded once at entry
%new_a = add i64 %a, 1
%b = load i64, i64* %field_b_ptr  ; independent load (can be hoisted)
store i64 %new_a, i64* %field_a_ptr  ; single store at commit
```

Intermediate stores inside the transaction body become dead — only the final
commit store per field survives.

### Tier 2: Contract Bounds — `!range` / `@llvm.assume`

**Cost:** Compile-time proof (Z3 or symbolic engine)
**Win:** LLVM deletes all runtime safety checks, emits `nuw nsw`

**`!range` metadata on loads:** Preconditions that bound a variable to a
numeric range become `!range` metadata:

| Precondition | LLVM !range | Optimization Enabled |
|-------------|-------------|---------------------|
| `[x < 100]` | `!{ i64 0, i64 100 }` | Dead branch elimination on `x >= 100` |
| `[x >= 0]` | `!{ i64 0, i64 9223372036854775808 }` | Sign-extend removal |
| `[x in 5..10]` | `!{ i64 5, i64 10 }` | Array bounds check elimination |
| `[len > 0]` | `!{ i64 1, i64 9223372036854775808 }` | Loop trip count ≥ 1 |

**`@llvm.assume` for complex invariants:** When a precondition involves
multiple variables or relationships that can't be expressed as simple range
metadata:

```llvm
; [from > 0 && to < 100 && from + amount == to]
%c1 = icmp sgt i64 %from, 0
%c2 = icmp slt i64 %to, 100
%c3 = icmp eq i64 %from, %to
%cond = and i1 %c1, %c2
%cond2 = and i1 %cond, %c3
call void @llvm.assume(i1 %cond2)
```

What `@llvm.assume` enables: loop vectorization, dead code elimination,
algebraic simplification (proven bounds → `nuw nsw` on arithmetic), and
speculative execution safety.

**Guard-select conversion:** Single-assignment guards become `select i1`
instead of `br` + `phi`. No branch mispredict penalty, no basic block split.

### Tier 3: Transition Fusing — Atomic Multi-Transaction

**Cost:** Call graph analysis + symbolic post/pre condition intersection
**Win:** Eliminates store/load round-trip between fused ticks

When `post(TxA) ⇒ pre(TxB)` is proven by the proof engine, the backend fuses
their bodies into a single atomic transition. This eliminates an entire reactor
tick for the intermediate state:

```llvm
; Without fusion (2 ticks, 2 loads + 2 stores of gpuBar0):
; Tick 1 — StateX:     store i64 %reservedMem, i64* %gpuBar0_ptr
; Tick 2 — StateY:     %val = load i64, i64* %gpuBar0_ptr; store i64 %signalY, ...

; With fusion (1 tick, 0 intermediate memory round-trips):
%new_bar0 = add i64 0, %reservedMem
%new_bar1 = add i64 0, %signalY
store i64 %new_bar0, i64* %gpuBar0_ptr
store i64 %new_bar1, i64* %gpuBar1_ptr
```

Both stores happen in the same basic block — LLVM can schedule them optimally,
issue them to the store buffer together, or hoist them past a single memory
barrier.

**Inhibition rules** (fusion refused if any apply):
- `Txn_B`'s precondition references a volatile `trg` (trigger can mutate
  between ticks)
- `Txn_A` writes to a field that `Txn_B` reads AND writes (WAW hazard)
- `Txn_A` or `Txn_B` is async (external preemption breaks atomicity)
- Fused state would exceed LLVM's per-function complexity budget (default 1000
  insts)

### Optimization Hierarchy Summary

```
Briv Source
    ↓ (parsing + type checking)
AST with Contracts
    ↓ (proof engine)
Verified IR with Proven Bounds
    ↓ (LLVM backend)
┌──────────────────────────────────────────────┐
│  Tier 1: noalias nocapture → mem2reg → SSA   │  ← language semantics, zero effort
│  Tier 2: !range + @llvm.assume → nuw nsw     │  ← contract parsing, compile-time proof
│  Tier 3: Transition fusing → atomic multi-txn │  ← call graph + symbolic analysis
│  + Guard-select → no branch penalty           │  ← peephole optimization
│  + Acyclic inlining → zero call instructions  │  ← call graph analysis
└──────────────────────────────────────────────┘
    ↓ (llc -O3)
Machine Code (x86_64, AArch64, WASM, GPU)
```

## 3. Expression System Design

### Choice: Hybrid i64-Centric with Logical Type Tracking

#### Analysis of the Trade-off

**Pure i64-Centric (current approach):**

| Pro | Con |
|-----|-----|
| Uniform codegen — one type for all expressions, simple emit logic | Every boundary needs conversions — trunc to i32 for floats/bools before stores, bitcast to float, zext back on load. Miss one and IR is invalid. |
| No type-tracking state — no HashMap<String, Type> for registers | Type errors are silent — `xor i64 -1` produces garbage for booleans |
| Easier match/phi — all phi nodes are `phi i64` | Hard to add `nuw nsw` — can't emit `add nuw nsw` without knowing operand types |
| Smaller backend code | Hides bugs until `llc` — type mismatches are runtime assembly failures, not compile errors |

**Pure Type-Aware (alternative):**

| Pro | Con |
|-----|-----|
| Correct by construction — each register knows its type, emit correct opcode | Stateful tracking — need a stack of HashMap<String, &str> scoping register types |
| nuw nsw possible — when both operands are i64 with contract bounds, emit flags | Complex phi nodes — match arms may produce different types, need bitcast/zext at merge |
| Natural float ops — `%v = fadd float %a, %b` instead of integer-on-bit-pattern | Larger backend — more match arms, type-dispatch logic |
| FFI marshaling is trivial — just pass the register with its native type | More complex let_bindings — must track (register, type) pairs |

**Decision: Hybrid approach** — i64 as the universal representation, with a
lightweight `HashMap<String, Type>` tracking each register's *logical type* at
its definition site.

Rationale:
- i64 keeps phi/switch/match simple — all merge points are `phi i64`
- Type tracking is needed only at *boundary points*: stores (to determine
  trunc/bitcast), binary ops (to select `fadd` vs `add`), and FFI marshaling
- A single `HashMap` scoped per transaction/definition is sufficient — no need
  for per-register type annotations throughout

#### Implementation

```rust
pub struct LlvmBackend {
    // ... existing fields ...
    
    /// Tracks the logical Briv type of each SSA register, for use at
    /// boundary points (stores, binary ops, FFI marshaling).
    register_types: HashMap<String, Type>,
}
```

**Register type tracking rules:**

| Expression | Emitted IR | Register Type |
|-----------|-----------|---------------|
| `Expr::Integer(n)` | `add i64 0, n` | `Type::Int` |
| `Expr::Bool(b)` | `add i64 0, b` | `Type::Bool` |
| `Expr::Float(f)` | `fadd float 0.0, f` → `bitcast float %f to i32` → `zext i32 %f to i64` | `Type::Float` |
| `Expr::Char(c)` | `add i32 0, c` → `zext i32 %c to i64` | `Type::Char` |
| `Expr::String(s)` | global const + `ptrtoint` | `Type::String` |
| `Expr::Identifier(n)` | `add i64 0, %reg` | lookup from `let_bindings` |

**Binary op dispatch:**

```rust
fn binary_op_ty(&self, a: &str, b: &str) -> Type {
    let ta = self.register_types.get(a).unwrap_or(&Type::Int);
    let tb = self.register_types.get(b).unwrap_or(&Type::Int);
    if matches!(ta, Type::Float) || matches!(tb, Type::Float) {
        Type::Float  // promote to float
    } else {
        Type::Int    // default integer
    }
}
```

Then emit the correct opcode based on the resolved type.

**Store dispatch:**

```rust
let field_ty = &self.field_types[idx]; // "float", "i8", "i64", "i8*"
match field_ty {
    "float" => {
        // trunc i64 %val to i32, bitcast i32 %tmp to float, store float
    }
    "i8" => {
        // trunc i64 %val to i8, store i8 (already done)
    }
    _ => {
        // store i64 %val (already done)
    }
}
```

#### Scoped `let_bindings` + Register Types

```rust
struct Binding {
    register: String,
    ty: Option<Type>,  // None = i64 default (Type::Int)
}

// Stack of scopes — push on guarded block entry, pop on exit
let_scopes: Vec<HashMap<String, Binding>>,
```

Resolution: search scopes top-to-bottom (innermost first).
Clear on definition/transaction entry.

### `self.terminated` Redesign

Replace flat `bool` with a scope stack:

```rust
termination_depth: usize,          // incremented on scope entry
scope_terminated: Vec<bool>,       // per nesting level
```

On entry to a guarded block or match arm: push `false`.
On exit: pop, and if the outer scope wasn't already terminated, restore.
Simpler alternative for now: save/restore `self.terminated` around guarded
blocks and match arms.

## 4. Correctness Fixes: 15 Confirmed Bugs

All line references target `src/backend/llvm.rs` at commit `ef992b8`.

### Group A: SSA Correctness (IR won't verify without these)

| # | Lines | Bug | Fix |
|---|-------|-----|-----|
| 1A | 396,405 | `self.terminated` leaks across branches — a `term` inside a guarded block suppresses the implicit `ret` at the function's end, leaving the trailing block without a terminator | Save/restore `self.terminated` around `Statement::Guarded`, match arms, and unification. If the inner block terminated, the outer block still needs its own `ret`. |
| 1B | 277-312 | `let_bindings` not cleared on definition entry — bindings from prior functions leak into subsequent definitions, causing register names from different functions to be referenced | `self.let_bindings.clear()` at start of `emit_definition`. Then convert parameters. |
| 1C | 715-726 | Match arm labels `ma{}` use a local `vi` counter — two `match` expressions in one function produce duplicate `ma0`, `ma1` labels | Incorporate `self.txn_counter`: `format!("ma{}_{}", self.txn_counter, vi)` |
| 5A | 484-494 | Unification dominance violation — `%upX` defined only in the matching arm but inserted into `self.let_bindings` globally; the `def_l` (default) path never defines it, yet `merge_l` references it | Change `def_l` to `unreachable` (Briv guarantees the pattern must match or the program panics) |
| S6 | 447-460 | Guard-select optimization hardcodes `i64`/`i64*` for `load`/`store`, but the GEP pointer may be `i8*` or `float*` | Use `self.field_types[idx]` to determine the load/store type. Apply trunc/bitcast/zext accordingly. |

### Group B: Type Safety (produces invalid IR)

| # | Lines | Bug | Fix |
|---|-------|-----|-----|
| S4 | 431 | Float store type mismatch — `store float %val, float* %p` where `%val` is an `i64` register | `trunc i64 %val to i32` → `bitcast i32 %tmp to float` → `store float %f, float* %p, align 4` |
| S3 | 605-606 | FFI marshaling uses `zext` where `trunc` is needed — `zext i64 to i32` violates LLVM's rule that `zext` requires dest > source | `trunc i64 %raw to i32` for Bool and Char FFI arguments |
| S5 | 584 | Logical not emits bitwise inversion — `xor i64 %v, -1` turns `1` (true) into `-2` (still truthy) | `xor i64 %v, 1` — flips only the LSB |
| 3A | 510-517, 569-585 | Float arithmetic uses integer instructions — `add i64` on IEEE 754 bit patterns produces garbage | Check operand type: if `Type::Float`, emit `fadd`/`fsub`/`fmul`/`fdiv`/`fcmp` instead |

### Group C: Memory Correctness (reads garbage)

| # | Lines | Bug | Fix |
|---|-------|-----|-----|
| S1 | 518-522 | String literals allocate stack space but never write the characters — the pointer returned points to uninitialized memory | Declare a private unnamed global constant (`@str.0 = private unnamed_addr constant [N x i8] c"...\00"`), GEP to it, pass pointer directly (or `memcpy` to stack if mutability is needed). |
| S2 | 665-669 | List literal `alloca i64, i64 0` ignores all elements — elements are never evaluated or stored | Determine `N = elems.len()`, emit `alloca i64, i64 N`, emit `getelementptr` + `store` for each element |
| 3B | 655-658 | Enum constructor (uppercase call like `Some(42)`) allocates storage but emits zero `store` instructions — discriminant and payload are garbage | After `alloca`, emit `getelementptr` to discriminant slot → `store i64 <disc>`, then for each arg, GEP to payload slot → `store i64 <arg>` |

### Group D: FFI & Trigger Completeness

| # | Lines | Bug | Fix |
|---|-------|-----|-----|
| 4A | 106 | FFI return type inferred from param count — `if sig.inputs.is_empty() { "void" } else { "i64" }` ignores the actual return type | Inspect `sig.output` to determine return type. Map `Type::Void` → `"void"`, everything else → `"i64"`. |
| 4B | 535-538 | Linked triggers (`LinkRef::Linked(sym)`) reference `@sym` in `load volatile` but no `@sym = external global i8` is emitted at module level | Scan `self.triggers` map, collect all `LinkRef::Linked` symbols, emit `@sym = external global i8, align 1` in the module header |

### Group E: Transition Fusing Correctness

| # | Lines | Bug | Fix |
|---|-------|-----|-----|
| 2A | 820, 839 | Fused transactions (e.g., `TxA_TxB_fused`) are not present in the `txns` slice of original transactions. `txns.iter().find()` returns `None`, defaulting `has_pre` to `false` via `unwrap_or(false)`. This generates `br i1 true, label %b0, label %ck0`, bypassing all preconditions of the participating transactions. | When `first` is a fused name, look up the precondition of the *initial* transaction (split on `_fused`, take the first component). Use that precondition for the dispatch check. Similarly for `has_next_pre`. |

## 5. Implementation Order

### Phase A: SSA Correctness
IR won't verify — must fix first.

1. Fix `self.terminated` scoping (1A)
2. Fix `let_bindings` scope + clear on def entry (1B)
3. Fix match label collision (1C)
4. Fix unification dominance (5A)
5. Fix guard-select type mismatch (S6)

### Phase B: Type Safety
Produces invalid IR — must fix before any test passes `llc`.

6. Fix float store trunc+bitcast (S4)
7. Fix FFI trunc vs zext (S3)
8. Fix logical not (S5)
9. Fix float arithmetic opcodes (3A)

### Phase C: Memory Correctness
Reads garbage at runtime — fix before any real program runs.

10. Fix string literal initialization (S1)
11. Fix list literal element emission (S2)
12. Fix enum constructor stores (3B)

### Phase D: FFI & Trigger Completeness
Missing declarations cause linker errors.

13. Fix FFI return type mapping (4A)
14. Emit external global declarations for linked triggers (4B)

### Phase E: Transition Fusing Correctness
Safety bypass — critical for correctness of fused dispatch.

15. Fix fused txn precondition lookup (2A)

### Phase F: Optimization Enhancement
Not correctness-critical, but realizes Briv's optimization vision.

16. Emit `nuw nsw` on `add`/`sub`/`mul` when contracts prove bounds
17. Wire `@llvm.assume` for complex preconditions (currently declared but unused)
18. Add `alwaysinline` for acyclic transactions (per spec at `03-TRANSACTIONS.md`)
19. Register type tracking for binary op dispatch (float/int resolution)

## 6. FFI Architecture

### Foreign Function Declarations

The current approach (scan `frgn_map` → emit `declare`) is structurally
correct but buggy in detail. Corrected algorithm:

```
For each (name, sig) in frgn_map:
    ret_ty = match sig.output {
        Type::Void => "void",
        _ => "i64",  // all other types marshaled to i64
    }
    param_tys = sig.inputs.map(|(_, t)| match t {
        Type::Int | Type::UInt => "i64",
        Type::Bool => "i32",
        Type::Char => "i32",
        Type::String | Type::Data => "i8*",
        _ => "i64",
    })
    emit: declare {ret_ty} @{name}({param_tys joined by ", "}) #1
```

### Argument Marshaling at Call Sites

| Briv Type | C ABI Type | LLVM Conversion |
|------------|------------|-----------------|
| `Int` | `int64_t` | `i64 %raw` (direct) |
| `Float` | `float` | `trunc i64 %raw to i32` → `bitcast i32 to float` |
| `Bool` | `int32_t` | `trunc i64 %raw to i32` |
| `Char` | `uint32_t` | `trunc i64 %raw to i32` |
| `String` | `const char*` | `inttoptr i64 %raw to i8*` |

### Bootstrap Intrinsics

| Name | Signature | Notes |
|------|-----------|-------|
| `__print` | `(String) -> Void` | First arg marshaled to `i8*`, call `strlen` + `write` |
| `__exit` | `(Void) -> Void` | `call void @exit(i32 0)` |

### String Literal Emission

```llvm
; Module-level constant (emitted once per unique string)
@str.0 = private unnamed_addr constant [6 x i8] c"hello\00", align 1

; At the Expr::String("hello") site:
%sp0 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
%v   = ptrtoint i8* %sp0 to i64  ; i64 representation
```

If strings need to be mutable (unlikely in Briv's semantics), fall back to
`alloca` + `memcpy` from the global constant.

### Trigger Global Declarations

```llvm
; Emitted in module header for each LinkRef::Linked("button"):
@button = external global i8, align 1

; At the trigger sampling site:
%tr0 = load volatile i8, i8* @button, align 1
%tz0 = zext i8 %tr0 to i64
```

## 7. Type System Cross-Reference

### LLVM Type Mapping

| Briv Type | LLVM Type | Storage in %State | i64 Representation |
|-----------|-----------|-------------------|-------------------|
| `Int` | `i64` | `i64` | Direct |
| `UInt` | `i64` | `i64` | Direct |
| `Float` | `float` | `float` | `bitcast float to i32` → `zext i32 to i64` |
| `Bool` | `i1` (stored as `i8`) | `i8` | `zext i8 to i64` (load), `trunc i64 to i8` (store) |
| `Char` | `i32` | `i32` | `zext i32 to i64` (load), `trunc i64 to i32` (store) |
| `String` | `{ i8*, i64 }` | `i8*` (pointer only) | `ptrtoint i8* to i64` (load), `inttoptr i64 to i8*` (store) |
| `Data` | `{ i8*, i64 }` | `i8*` | Same as String |
| `Void` | `void` | N/A | N/A |

### Register Type Tracking at Key Points

| Operation | Type Check | Action |
|-----------|-----------|--------|
| `Expr::Add(l, r)` | `register_types[l]` or `register_types[r]` is `Float` | Emit `fadd float %a, %b` |
| `Expr::Add(l, r)` | both are `Int`/`UInt` | Emit `add i64 %a, %b` (with `nuw nsw` if bounds proven) |
| `&field = expr` | `field_types[idx]` is `"float"` | `trunc i64 %val to i32` → `bitcast i32 to float` → `store float` |
| `&field = expr` | `field_types[idx]` is `"i8"` | `trunc i64 %val to i8` → `store i8` |
| FFI call arg | param type is `Bool` | `trunc i64 %raw to i32` |
| FFI call arg | param type is `Float` | `trunc i64 %raw to i32` → `bitcast i32 to float` |

## 8. Verification Checklist

After implementing all phases, every `.ll` file must pass:

```bash
# 1. Structural validity
llc input.ll -o /dev/null          # Must succeed (no assembly errors)
opt -O3 -S input.ll -o /dev/null   # Must succeed (no optimization crashes)
opt -verify -S input.ll -o /dev/null  # Must pass IR verification

# 2. Contract preservation
grep -c "noalias" input.ll         # Must be > 0
grep -c "!range" input.ll          # Must be > 0 (if preconditions exist)

# 3. Optimization confirmation
opt -O3 -S input.ll -o /dev/null   # Verify nuw/nsw emitted where applicable
```

### Regression Test Fixtures

| Fixture | Tests |
|---------|-------|
| `tests/fixtures/counter.bv` | Basic Int field, `[count < 10]` contract, `add`, `store` |
| `tests/fixtures/multifield.bv` | Int + Bool, two txns with disjoint field access |
| `tests/fixtures/minimal.bv` | No transactions — just state and main loop |
| `tests/fixtures/float_arith.bv` | Float fields + arithmetic operations |
| `tests/fixtures/ffi_call.bv` | `frgn` declarations + call sites |
| `tests/fixtures/match_enum.bv` | Match expressions on enum variants |
| `tests/fixtures/string_literal.bv` | String constants passed to FFI |

### Runtime Linking

After code generation, the compiled `.ll` file must be linked against
`runtime/briv_rt.c`:

```bash
llc input.ll -o input.s
as input.s -o input.o
cc -c runtime/briv_rt.c -o briv_rt.o
ld input.o briv_rt.o -o program
```

On bare-metal targets, `briv_rt.c` provides `wfi`/`hlt` implementations.
On OS targets, it provides epoll/kqueue implementations.
One file, C preprocessor handles platform detection. See `runtime/briv_rt.c`.

## 9. Event Model Integration

The event model (`@ link` as universal doorbell, `node` as event handler) is
documented in two companion documents:

- **`specs/EVENT-MODEL.md`** — Core language event architecture
- **`llvm-spec/14-EVENT-LLVM-LOWERING.md`** — LLVM IR lowering for events
- **`runtime/briv_rt.c`** — Single-file C runtime providing `@ link` global definitions
  and `__wait_for_event()` per platform

### Impact on This Document

The 17 bugs cataloged in Section 4 are all pre-existing in `src/backend/llvm.rs`
and are orthogonal to the event model. The event model adds four behaviors
to the backend:

| # | Behavior | Priority |
|---|----------|----------|
| E1 | Emit `@sym = external global <ty>` for each `@ link` trigger | Fixes bug 4B (already in Phase D) |
| E2 | Pre-sample triggers at reactor_tick entry into named registers | Phase F (completed) |
| E3 | Remove hardcoded `__wait_for_event()` from equilibrium path | Phase F (completed) |
| E4 | Fall-through dispatch chain (not first-true-return) | Phase F (completed) — each body branches to next precondition check, `ret void` only after all evaluated |

Previously, each transaction body emitted `ret void`, causing the dispatch
chain to exit after the first true precondition. This was incorrect — the
interpreter model evaluates ALL dirty transactions sequentially, with each
transaction's side effects visible to the next. The fix changes body blocks
to `br label %ck{N+1}` (fall-through), so `__io_pump` can set `io_ready` and
a downstream consumer reads it in the same tick. Only the final `ret void`
at the end of the chain actually returns.

The `trg!` statement (`Statement::LocalTrigger`) is emitted as a no-op comment.
It is excluded from the LLVM backend's event model. New code should use
top-level `trg` + `node`.

### Bug Inventory Update

The event model analysis reveals one additional backend issue:

| # | Lines | Bug | Fix | Phase |
|---|-------|-----|-----|-------|
| E2 | 532-545 | Trigger identifiers emit fresh `load volatile` per reference instead of using pre-sampled register | Move trigger sampling to reactor_tick prologue; Expr::Identifier references pre-sampled `%sz_<name>` registers | F |

## 10. Summary: What Makes Briv's LLVM Backend Unique

1. **Contracts are optimization fuel.** No other language feeds precondition
   bounds into LLVM's `!range` and `@llvm.assume` as a first-class codegen
   path. The safety feature pays a performance dividend.

2. **`noalias` as a language guarantee.** Briv's prohibition on arbitrary
   pointers means every `%State*` can carry `noalias nocapture` without the
   programmer writing `restrict`. LLVM gets alias analysis for free.

3. **Acyclicity as a compiler property.** The call graph analysis proves
   `norecurse` + `willreturn`, enabling the entire tick loop to inline to
   zero `call` instructions. LLVM sees one SSA graph.

4. **Transition fusing as a semantic optimization.** Because the proof engine
   can compute `post(TxA) ⇒ pre(TxB)`, the backend can fuse transactions
   across tick boundaries — an optimization no general-purpose compiler can
   attempt.

5. **i64-centric with lightweight type tracking.** The hybrid approach keeps
   codegen simple (uniform register type for phi/switch/match) while
   correcting the 15 identified bugs through targeted type checks at boundary
   points.