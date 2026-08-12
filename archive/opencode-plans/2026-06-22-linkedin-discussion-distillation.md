# LinkedIn Discussion Distillation — New Directions for Briev

**Date:** 2026-06-22  
**Source:** LinkedIn comment chain on "Types are just bits with expectations"  
**Authors:** Randy Smits-Schreuder Goedheijt, Konstantin Sharon, Paul Cook, Pete Howard, Roger Scott, Victor S., Kees Jan Hermans  
**Status:** Design Notes / Plan

---

## Table of Contents

1. [Overview — What This Captures](#1-overview--what-this-captures)
2. [Proposal A: Adaptive Physical Layout Engine](#2-proposal-a-adaptive-physical-layout-engine)
3. [Proposal B: Lazy-to-Eager Promotion](#3-proposal-b-lazy-to-eager-promotion)
4. [Proposal C: Lens-Composition Symbolic Execution](#4-proposal-c-lens-composition-symbolic-execution)
5. [Proposal D: Contract-Proven Integer Semantics](#5-proposal-d-contract-proven-integer-semantics)
6. [Proposal E: Safe Type Introspection / "Cracking Open"](#6-proposal-e-safe-type-introspection--cracking-open)
7. [Proposal F: Postcondition Instrumentation for `defn`](#7-proposal-f-postcondition-instrumentation-for-defn)
8. [Priority and Sequencing](#8-priority-and-sequencing)
9. [Relationship to Existing Bits Thesis Plan](#9-relationship-to-existing-bits-thesis-plan)

---

## 1. Overview — What This Captures

The [Bits Thesis plan](../docs/plans/2026-06-20-bits-thesis.md) already formalizes the core ideas from the LinkedIn post: types are lenses over Bits, operators desugar to projections, lazy CString interop, zero-cost FFI re-lensing. This document captures **the ideas that emerged organically in the comment chain that go beyond what the Bits Thesis plan covers**.

Each proposal traces back to a specific exchange in the thread and is grounded in the current compiler architecture so the work items are concrete.

---

## 2. Proposal A: Adaptive Physical Layout Engine

### Source

**Randy's response to Paul Cook**, where Paul pointed out the caching complexity of lazy C-string length:

> "If our liveness analysis shows the length is never requested, we allocate 0 bytes for caching and emit 0 instructions. But if the length is requested repeatedly, the compiler does something unique: it adapts the physical layout of that specific string in memory to include a 64-bit cache slot, then injects the cache-invalidation logic on writes."

> "The data structure's physical footprint isn't a rigid, hardcoded class or type definition. Rather, the compiler custom-tailors the memory layout based on how your code actually uses it."

### What Already Exists

- **Dead-field elimination** (`transition_graph.rs:compute_live_fields()`) drops stores to fields never observed by FFI calls or exit conditions
- **Pure-counter fold** (A001) eliminates entire loop bodies
- **Liveness is per-field, whole-struct** — a large struct field is either entirely live or entirely dead

### What's New

Extend liveness analysis to track **usage patterns at the sub-field / projection level**, and use that to **vary physical layout** at compile time:

| Usage pattern | Current | Adaptive |
|---|---|---|
| `str :> Size` never called | 16 bytes (ptr+len), len field dead but allocated | 8 bytes (ptr only) — len field eliminated from struct |
| `str :> Size` called once | Must `strlen` each time | No change — one-shot cost acceptable |
| `str :> Size` called in hot loop | `strlen` on every iteration (O(N)!) | Inject 64-bit cache slot + valid flag, invalidate on writes |

### Concrete Work Items

| # | Task | Modules | Effort |
|---|------|---------|--------|
| A.1 | Extend liveness to projection-usage tracking | `analysis/transition_graph.rs` | Medium |
| A.2 | Add `LiveField.mode: FieldMode` — Always/LazyCached/Never | `analysis/mod.rs` | Low |
| A.3 | Emit adaptive struct layouts in LLVM — omit Never fields, add valid-flag for LazyCached | `backend/llvm/emit_toplevel.rs` | High |
| A.4 | Inject cache-invalidation on writes to LazyCached fields | `backend/llvm/loop_engine.rs` | Medium |
| A.5 | Add `--layout` diagnostic flag | `backend/llvm/mod.rs` | Low |

### Risks & Mitigation

- **GEP offset breakage**: Variable struct layout means different compilation units may disagree on field offsets. Mitigation: only apply adaptive layout to types with no FFI exposure (no `frgn` pointer escape).
- **Cache invalidation overhead**: Store-before-store patterns may confuse LLVM. Mitigation: gate on `has_mutations`; read-only fields need no invalidation.

---

## 3. Proposal B: Lazy-to-Eager Promotion

### Source

**Randy's follow-up to Paul Cook**: A three-tier model where evaluation strategy is selected per projection-site based on usage frequency.

### What Already Exists

- Lazy projection pattern (`CString :> Size = strlen#`) — always lazy, never promoted
- No mechanism to change evaluation strategy based on code analysis

### What's New

A **three-tier evaluation strategy**:

```
Tier 1: Eliminated  — Projection never used. 0 instructions, 0 bytes.
Tier 2: Deferred    — Used infrequently. Evaluated on demand, not cached.
Tier 3: Cached      — Used in hot loop. Cached after first eval, invalidated on writes.
```

Paul's question ("how is this different from calling strlen each time?") is the key: Tier 2 IS the same as calling strlen each time. The value is in Tiers 1 and 3 — both represent strict wins.

### Concrete Work Items

| # | Task | Modules | Effort |
|---|------|---------|--------|
| B.1 | Add `ProjectionEvaluationStrategy` enum (Eliminated/Deferred/Cached) | `features/projection.rs` | Low |
| B.2 | Implement call-site frequency analysis — count projection usage in loops vs straight-line | `analysis/mod.rs` | Medium |
| B.3 | Emit cache-slot + valid-flag runtime for Cached projections | `backend/llvm/emit_expr.rs` | Medium |
| B.4 | Emit cache invalidation on writes | `backend/llvm/loop_engine.rs` | Medium |
| B.5 | Document three-tier model | `docs/architecture/features/projection.md` | Low |

---

## 4. Proposal C: Lens-Composition Symbolic Execution

### Source

**Randy's response to Pete Howard**: "We can sometimes even achieve pure symbolic execution at compile time without the need to allocate memory, just because we are working with compatible lenses."

### What Already Exists

- Equality saturation — 9-rule fixpoint rewrite engine (5 passes, O(n))
- Pure-counter fold — const-folds loop bodies when inputs are const
- Projection system follows the "type as lens" model

### What's New

When two values have **compatible type lenses** (same codec, same layout), the compiler performs operations **symbolically on the lens representation** without materializing the underlying bits:

```briev
let a: String = "hello ";
let b: String = "world";
let c = a ++ b;  // Computed symbolically via String lens at compile time

// Lens compatibility enables cross-type symbolic exec:
let cs: CString = getenv("PATH");
let combined = (cs as String) ++ "/bin";  // Can be symbolic IF cs is const
```

The key: `are_lens_compatible(CString, String)` returns true if layouts match, enabling the equality saturation engine to treat operations across these types as symbolic.

### Concrete Work Items

| # | Task | Modules | Effort |
|---|------|---------|--------|
| C.1 | Add `TypeUniverse::are_lens_compatible(a, b)` — checks Bytes, Codec, `@/` ranges | `type_universe.rs` | Low |
| C.2 | Extend equality saturation with lens-aware rewrite rules | `equality_saturation.rs` | Medium |
| C.3 | Add `symbolic_materialize` pass — skip allocation/copy for all-symbolic programs | `codegen/mod.rs` | Medium |
| C.4 | Add diagnostic: "computed symbolically via String lens — zero allocations" | Diagnostics | Low |

---

## 5. Proposal D: Contract-Proven Integer Semantics

### Source

**Konstantin Sharon**: Natural integers prove `a + b > a` and `a + b > b`.  
**Randy**: With wrapping 8-bit integers, `200 + 200` wraps below both operands.  
**Kees Jan Hermans**: "Does this language have integer overflow protection?"

### What Already Exists

- Bare `i64` wrapping arithmetic (interpreter + LLVM)
- No `nsw`/`nuw` flags on LLVM integer ops
- `!range` metadata for `[x < N]` patterns
- Contracts can express overflow constraints

### What's New

**A dual integer type system** gated by contracts:

| Type | Semantics | LLVM flags | Contract requires |
|------|-----------|------------|-------------------|
| `Int` (current) | Wrapping `i64` | None | Nothing |
| `Nat` | Non-negative, overflow-checked | `nuw` when proven | `[x >= 0]` |
| `Checked<N..M>` | Bounded, overflow-preventing | `nsw`+`nuw` when proven | `[x in N..M]` on all ops |

Konst's insight: a **proven integer** (non-negative, bounded) lets LLVM optimize more aggressively — strength reduction, IV elimination, etc. The contract is the optimization enabler.

### Concrete Work Items

| # | Task | Modules | Effort |
|---|------|---------|--------|
| D.1 | Add `Nat` type — non-negative, checked on construction | `ast.rs`, `typechecker.rs`, `interpreter.rs` | Low |
| D.2 | Add `Checked<T,N,M>` type — bounded integer | Same files | Medium |
| D.3 | Emit `nsw`+`nuw` for checked operations in LLVM | `backend/llvm/emit_expr.rs` | Low |
| D.4 | Auto-upgrade Int → Nat for codegen when `[x >= 0]` proven | `analysis/region.rs` | Medium |
| D.5 | Add `checked_add#` intrinsic — `(Result, wrapped)` | `ast.rs` (Intrinsic) | Low |
| D.6 | Document integer semantics | `docs/architecture/features/integer-semantics.md` | Low |

---

## 6. Proposal E: Safe Type Introspection / "Cracking Open"

### Source

**Randy to Victor S.**: "It's important not just to be literate about what the type represents, but also how to crack it open in a way that is safe and ergonomic."

**Roger Scott**: "A good type is not just an 'expectation' — it is a *constraint*. In other words, an expectation with teeth."

### What Already Exists

- `reinterpret<T>(val)` — unsafe lens swap
- `val :> Ptr` — get address
- `val :> Bytes` — get byte width
- `@/` — bit-range access

### What's New

A formalized **`crack<T>` operation** with proof-engine guarantees:

```briev
// Safe — proof-checked at compile time via ?#:
let raw = crack<MyStruct>(val) :> [layout == expected];

// Unsafe — explicit opt-out:
let raw = crack!<MyStruct>(val);
```

`crack` proves (via `?#`) that the source bits are a valid target type representation. No memory copy. The proof can be deferred to runtime when compile-time proof is impossible.

### Concrete Work Items

| # | Task | Modules | Effort |
|---|------|---------|--------|
| E.1 | Add `Expr::Crack` AST node | `ast.rs` | Low |
| E.2 | Implement `crack` in proof engine — compile-time or deferred layout check | `proof_engine.rs` | Medium |
| E.3 | Emit zero-cost re-lens in LLVM (TBAA swap only) | `backend/llvm/emit_expr.rs` | Low |
| E.4 | Add `crack!` unsafe variant | Same files | Low |
| E.5 | Add `layout_eq#(T,U)` intrinsic for runtime proof | `ast.rs` (Intrinsic) | Low |

---

## 7. Proposal F: Postcondition Instrumentation for `defn`

### Source

**Roger Scott**: "Good types are not just expectations — they are *constraints with teeth*."

Implicit in every exchange about contracts as source of truth.

### What Already Exists

- Preconditions checked at runtime for `defn` and `txn`
- Postconditions checked **only for `txn` convergence** (loop repeat)
- Postconditions for `defn` calls are **trusted but never checked** in LLVM backend
- Interpreter does check postconditions after `defn`

### What's New

**Runtime postcondition checking for `defn` calls in LLVM**, gated by a new `--contracts` flag:

```
--contracts check   Verify postconditions at runtime (default in --dev)
--contracts trust   No postcondition codegen (default in --release)
--contracts skip    No precondition or postcondition codegen
```

LLVM emission:

```llvm
call void @defn_body(...)
%post_ok = ...       ; evaluate postcondition
br i1 %post_ok, %pass, %fail
fail:
  call void @llvm.trap()
  unreachable
pass:
  ret ...
```

### Concrete Work Items

| # | Task | Modules | Effort |
|---|------|---------|--------|
| F.1 | Emit postcondition check after `defn` body in LLVM | `backend/llvm/emit_toplevel.rs` | Medium |
| F.2 | Gate on `--dev` vs `--contracts` flag | `main.rs`, `emit_toplevel.rs` | Low |
| F.3 | Add `--contracts check|trust|skip` to CLI | `main.rs` | Low |
| F.4 | Add diagnostic for contract violations with runtime values | Diagnostics | Medium |

---

## 8. Priority and Sequencing

| Priority | Proposal | Effort | Depends on | Rationale |
|----------|----------|--------|------------|-----------|
| **P1** | F — Postcondition Instrumentation | 3-5 days | Nothing | Lowest risk, strongest "constraints with teeth" signal. Builds on existing infrastructure. |
| **P1** | D — Integer Semantics | 5-7 days | Nothing | Clear LLVM optimization win (`nsw`/`nuw`). Addresses real questions from the thread. |
| **P2** | A — Adaptive Layout | 10-15 days | Existing liveness | The most novel idea from the thread. High effort but high impact. |
| **P2** | B — Lazy-to-Eager Promotion | 5-8 days | A (foundation) | Natural extension of adaptive layout. Combined they form "usage-driven optimization." |
| **P3** | E — Safe Type Introspection | 3-5 days | Proof engine (exists) | Important for ergonomics; `reinterpret` + `:>` cover most current needs. |
| **P3** | C — Lens-Composition Symbolic | 8-12 days | Equ. saturation (exists) | Most speculative. Requires clear theory of lens compatibility. |

### Sprint Plan

**Sprint 1** (P1):
- F.1-F.4: Postcondition instrumentation + `--contracts` flag
- D.1-D.6: `Nat`/`Checked` types, `nsw`/`nuw` emission

**Sprint 2** (P2):
- A.1-A.5: Adaptive layout engine
- B.1-B.5: Lazy-to-eager promotion

**Sprint 3** (P3):
- E.1-E.5: Safe type cracking
- C.1-C.4: Lens-composition symbolic execution

---

## 9. Relationship to Existing Bits Thesis Plan

The Bits Thesis plan (`docs/plans/2026-06-20-bits-thesis.md`) and this document are **complementary**:

| Bits Thesis (existing) | This document (new) |
|------------------------|---------------------|
| Types = lenses over Bits | **Adaptive layout**: lens changes physical footprint by usage |
| Lazy CString: `strlen#` deferred | **Lazy-to-eager**: deferred → cached automatically |
| Operator desugaring → projections | **Lens-composition symbolic**: compatible lenses → symbolic ops |
| Fast-path registry (Phase 3.5) | **Integer semantics**: contracts unlock `nsw`/`nuw` flags |
| Zero-cost FFI re-lensing (Section 13) | **Safe cracking**: `crack<T>` with proof-engine validation |
| Contract pre/post exist | **Stronger enforcement**: postconditions checked for `defn` |

### Philosophical Addition

The LinkedIn discussion introduced a refinement beyond the Bits Thesis plan:

**Types are not just lenses — they are expectations with adaptive footprint.** The existing Bits Thesis is a *static* theory: every type has a fixed bit layout. The discussion extends this to a *dynamic* theory: the layout itself adapts to usage, determined by the compiler through static analysis.

This is the single most important new idea: **the Bits Thesis gains a meta-level where the compiler designs the data structure for you, based on how each instance is actually used.**

---

*End of plan.*
