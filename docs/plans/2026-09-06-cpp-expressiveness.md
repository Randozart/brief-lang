# Plan: C++ Expressiveness via Briev Philosophy

**Date:** 2026-09-06
**Status:** Active
**Scope:** Close the C++ low-level expressiveness gap while preserving Briev's compiler-first philosophy
**Conversation:** 2026-09-06 session with user — all design decisions captured below

---

## 1. Origin: The Question

> "Can Briev be as expressive as C++ for the lowest level code?"

This question drove the entire analysis. The answer is: **mostly yes, with specific gaps that matter for niche systems programming.** The gaps aren't about missing *capabilities* — they're about missing *escape hatches with proof obligations*.

Briev's philosophy demands that the compiler wins by default. The programmer wins by proving why the compiler's default is wrong for their case. Every override must carry a contract, not just a keyword.

---

## 2. The Analysis: What Briev Has vs What C++ Has

### What Briev already covers (at parity or ahead):

- Raw pointers (`Ptr<T>`), `Load#`/`Store#`, `Cast#`, `Index#` — full address-level control
- `pack struct` / `seq struct` / `Bit<N>` — bit-level layout control (sub-byte fields, zero-padding, declaration order)
- `union` — untagged overlays
- `VolatileLoad#`/`VolatileStore#` + `vol let` — MMIO / hardware register access
- `atomic` fields + `AtomicLoad#`/`AtomicStore#`/`AtomicCas#`/`Fence#` — lock-free concurrency
- `asm<target>` declarations — inline assembly with compile-time validation
- `Malloc#`/`Alloc#`/`Free#` + `defer` — manual allocation with strategy selection (arena/stack/heap/ring)
- `frgn` + GLUE — C-ABI FFI, dynamic loading (`DlOpen#`/`DlSym#`), variadic calls
- Compile-time computation — `$const`/`$let`/`$defn`, macros, const generics, reflection
- LLVM auto-vectorizer with `seq` disable, vector phi groups, SLP hazard analysis

### The gaps we identified:

| Gap | Severity | Impact |
|-----|----------|--------|
| No pointer arithmetic operators | Medium | Must use `Index#` instead of `ptr + 1` |
| No explicit SIMD intrinsics | Medium | Can't manually unroll/vectorize inner loops |
| No atomic memory ordering parameterization | Medium | Always `seq_cst` (covers correctness, loses perf) |
| No linker section placement | Medium (embedded) | Can't pin code/data to specific memory regions |
| No interrupt handlers | High (embedded) | `.ebv` targets have no ISR support |
| No `restrict` / aliasing hints | Low | LLVM may not auto-alias as aggressively |
| No `alignas` on variables | Low | Only type-level alignment via `spec` |

---

## 3. The Architectural Decisions

### Decision 1: Portable Intrinsics, Not Target-Specific

**The question:** Should we have `SseAdd#`, `AvxAdd#`, `NeonAdd#` etc.?

**The answer: No.** Briev should have portable intrinsics that the backend lowers to the best available hardware. If the target doesn't support the feature, emit a scalar fallback + `#?` remark warning.

**Why:** Target-specific intrinsics (`SseAdd#`, `AvxAdd#`) leak CPU-specific names into user code. They break portability. A program written for x86 AVX shouldn't fail to compile on ARM NEON — it should degrade gracefully with a warning.

**The design:** `SimdAdd#(Vector<T,N>, Vector<T,N>)` is the portable intrinsic. The backend decides:
- Target has AVX2? → `vpaddd ymm`
- Target has SSE4? → `paddd xmm`
- Target has NEON? → `vadd.i32`
- Target has nothing? → scalar loop + `#? "SimdAdd# lowered to scalar: target lacks SIMD"`

This follows Briev's philosophy: the compiler picks the best codegen, and the programmer's intent is preserved across targets.

### Decision 2: Strategy Keywords for Behavioral Control, Not Directives

**The question:** Should alignment use `#!aligned(N)` directives or strategy keywords?

**The answer: Neither.** Alignment should be inferred from usage. If a variable feeds into SIMD operations, align it to the vector width. If it's used for MMIO, align to the hardware requirement. `spec Alignment: N` on type definitions already exists for ABI/hardware constraints.

**Why:** Briev's philosophy is that the compiler is smarter than the programmer. Alignment is a *derived property* of how a value is used, not an independent annotation. The compiler sees every operation on a value and can infer the correct alignment.

**The edge case:** What if you need cache-line alignment (64 bytes) and the compiler can't infer it? The answer: `spec Alignment: 64` on the type definition. If you need a cache-line-aligned buffer, define a type with that alignment and use that type. No variable-level directive needed.

**What we dropped:** `#!aligned(N)` (designed in v0.14 but never implemented), `#likely`/`#unlikely` branch hints (compiler discovers from PGO).

### Decision 3: Context-Sensitive Keywords for Atomic Ordering

**The question:** Should memory ordering be hashwords (`Relaxed`, `Acquire`) or strategy keywords (`relaxed atomic`, `acquire atomic`)?

**The answer: Both, with a clean split.**

- **Declaration-level:** Strategy keywords prefix `atomic` — `acquire atomic flag: Bool;`
- **Operation-level:** Hashwords as intrinsic parameters — `AtomicLoad#(ptr, Relaxed)`

**Why:** Strategy keywords work for declarations (setting a default for a field). Hashwords work for intrinsics (per-call control). You might want different orderings on different calls through the same pointer.

**The snake_case problem:** `acq_rel` and `seq_cst` have underscores. The user pushed for single words.

**The solution:**

| snake_case | Single word | Rationale |
|------------|-------------|-----------|
| `seq_cst` | `seq` | Sequential consistency IS sequentialism. `seq` already exists as a strategy keyword. Reuse it. |
| `acq_rel` | `bartered` | Acquire-release is a *trade*: you release your writes (give visibility) and acquire the other side's (get visibility). A barter between threads. |

**The final vocabulary:**

| Ordering | Word | Meaning | Strength |
|----------|------|---------|----------|
| relaxed | `relaxed` | No ordering constraints | Weakest |
| acquire | `acquire` | Read sees all prior writes | Read barrier |
| release | `release` | Write visible to subsequent reads | Write barrier |
| acq_rel | `bartered` | Both: exchange visibility | RMW exchange |
| seq_cst | `seq` | Total order, no reordering | Total order (default) |

**Context sensitivity:** `relaxed`, `acquire`, `release`, `bartered` are only valid when followed by `atomic`. Outside that context, they're identifiers. No conflicts with existing code.

### Decision 4: No Branch Hints

**The question:** Should we add `#likely`/`#unlikely` directives?

**The answer: No.**

**Why:** If the compiler is smarter than the programmer, it should discover branch probabilities from profiling (PGO) and static analysis, not from programmer annotations. A programmer saying "this branch is likely" is the compiler admitting defeat. Briev's philosophy demands the compiler win by default.

### Decision 5: Linker Sections as Strategy Keywords

**The question:** How should linker section placement work?

**The answer:** Strategy keyword `section` with proof obligations.

```briev
section(".isr_vector") isr_entry: Ptr<Bits<8>>;
section(".init") defn startup(): Void { ... };
section(".data") let globals: Int = 42;
```

**Proof obligations:**
- `section` on `defn`: must not allocate, must not call non-`section` functions (transitive check)
- `section` on `let`: initial value must be compile-time constant
- `section(".isr_vector")`: must be `Ptr<T>` or fixed-size `Byte[N]`

**Why strategy keyword, not directive:** Section placement is a behavioral declaration, not an optimization hint. It tells the linker where to put things. This is exactly what strategy keywords are for — disclosed compiler ownership of a property.

### Decision 6: ISR Handlers with Full Proof Contract

**The question:** How should interrupt handlers work?

**The answer:** `isr handler @ vector: name() { ... }` with ISR restrictions.

```briev
isr handler @ 0x08: timer_irq() {
    counter = counter + 1;
};
```

**ISR restrictions (typechecked):** No `Malloc#`, no `Float`, no `Spawn#`. Body is validated at compile time.

**Why:** ISR handlers are fundamentally user-specified — the programmer knows which interrupts they need. But the compiler can verify the handler is safe for interrupt context. This is Briev's philosophy: the programmer declares intent, the compiler proves correctness.

### Decision 7: Restrict as Strategy Keyword

**The question:** How should aliasing hints work?

**The answer:** Strategy keyword `restrict` on function declarations.

```briev
restrict defn process(a: Ptr<Int>, b: Ptr<Int>) {
    // compiler assumes a and b don't alias
    // emits LLVM noalias attributes
};
```

**Why strategy keyword:** Restrict is a behavioral declaration about the function's aliasing contract. It's disclosed compiler ownership — the programmer says "I promise these don't alias," and the compiler emits `noalias` attributes.

---

## 4. The Complete Feature Set

### 4.1 Pointer Arithmetic

**New intrinsics:**

| Intrinsic | Signature | LLVM Emit |
|-----------|-----------|-----------|
| `PtrAdd#(Ptr<T>, Int) -> Ptr<T>` | `ptr + offset` | `getelementptr inbounds T, ptr %p, i64 %o` |
| `PtrSub#(Ptr<T>, Int) -> Ptr<T>` | `ptr - offset` | `getelementptr inbounds T, ptr %p, i64 -%o` |
| `PtrDiff#(Ptr<T>, Ptr<T>) -> Int` | `ptr1 - ptr2` | GEP subtraction → `ptrdiff_t` |
| `PtrEq#(Ptr<T>, Ptr<T>) -> Bool` | `ptr1 == ptr2` | `icmp eq ptr` |
| `PtrLt#(Ptr<T>, Ptr<T>) -> Bool` | `ptr1 < ptr2` | `icmp ult ptr` |

**Operator sugar (via `op` on `Ptr<T>`):**

```briev
impl Ptr<T> {
    op Add(Int): PtrAdd#(#Lh, #Rh);
    op Sub(Int): PtrSub#(#Lh, #Rh);
    op Sub(Ptr<T>): PtrDiff#(#Lh, #Rh);
};
```

**Proof obligations:**
- `PtrAdd#`/`PtrSub#`: `inbounds` GEP — out-of-bounds is UB (caught by LLVM)
- `PtrDiff#`: both pointers must derive from same allocation (compile-time or runtime check)
- `PtrEq#`/`PtrLt#`: none (comparison is safe)

**Note:** `Ptr<T>` is a bootstrap primitive type, not a user `type`. Adding `op Add`/`op Sub` requires verifying the parser handles `impl Ptr<T>` correctly. If the parser doesn't support `impl` on primitives, the intrinsics alone (`PtrAdd#`, `PtrSub#`) provide the same functionality.

### 4.2 Portable SIMD

> **Implementation note (2026-09-06, Phase 6):** the plan's original
> signatures took `Vector<T,N>` SSA values. Briev's `Type::Vector` lowers to
> LLVM `[N x T]` ARRAYS (aggregates — no arithmetic), so SSA vector registers
> cannot escape into the existing type model. The honest ABI is
> **memory-to-memory**: `SimdAdd#(dst: Ptr<T>, a: Ptr<T>, b: Ptr<T>, count)`.
> Element-wise chunked processing is overlap-safe (per chunk, all loads
> precede the store), so dst may alias a/b — precisely the case where the
> auto-vectorizer must refuse or version with runtime alias checks. The
> intrinsic FORCES vectorization where the default path heuristically
> declines. SimdShuffle#/SimdGather#/SimdScatter# remain planned (they need
> masked-load encoding and index-vector plumbing — separate slice).

**Strategy keywords:**

| Keyword | Context | Effect |
|---------|---------|--------|
| `simd` | Loop, defn | Force vectorization (error if unvectorizable) |
| `nosimd` | Loop, defn | Disable vectorization (scalar only) |
| `simd<N>` | Loop | Force N-wide vectorization (error if target lacks width) |

**Portable intrinsics (target-agnostic, backend lowers to best HW):**

> 2026-09-06: signatures revised to the memory-to-memory ABI (see the
> implementation note above). `SimdLoad#`/`SimdStore#` dropped from the
> first slice — chunked loads/stores are emitted inside the arithmetic
> intrinsics, so standalone load/store adds no expressive power.

| Intrinsic | Purpose | Best HW | Scalar Fallback |
|-----------|---------|---------|-----------------|
| `SimdAdd#(dst, a, b, count)` | Element-wise add | `<4 x float>`/`<4 x i64>` chunks | Scalar tail loop |
| `SimdSub#(dst, a, b, count)` | Element-wise sub | Chunked | Scalar tail |
| `SimdMul#(dst, a, b, count)` | Element-wise mul | Chunked | Scalar tail |
| `SimdFma#(dst, a, b, c, count)` | a*b+c per element | mul+add pairs — ISel contracts to vfmadd on FMA targets | Scalar tail |
| `SimdShuffle#` | Lane permutation | `shufflevector` | Deferred |
| `SimdGather#` | Indexed load | `llvm.masked.gather` | Deferred |
| `SimdScatter#` | Indexed store | `llvm.masked.scatter` | Deferred |

**Fallback contract:**
- If target supports the feature: emit optimal instruction
- If target doesn't: emit scalar fallback + `#?` remark: `"SimdGather# lowered to scalar: target lacks AVX2"`
- Never a compile error — the operation is semantically valid everywhere

**`SimdFma#` target resolution example:**

```
Target has FMA3?    → vfmadd231ps
Target has AVX?     → vmulps + vaddps (2 instructions, not fused)
Target has SSE?     → mulps + addps
Target has NEON?    → fmla
Target has nothing? → a*b+c scalar loop
```

**Why no `__m256` types:** `Vector<Float, 8>` + `SimdFma#` gives the same thing without leaking CPU-specific type names into user code. The compiler resolves `Vector<Float, 8>` to `<8 x float>` on AVX targets and `<4 x float>` twice on SSE targets.

### 4.3 Atomic Memory Ordering

**Strategy keywords (context-sensitive — only valid when followed by `atomic`):**

| Keyword | Ordering | Strength |
|---------|----------|----------|
| `relaxed` | `memory_order_relaxed` | Weakest |
| `acquire` | `memory_order_acquire` | Read barrier |
| `release` | `memory_order_release` | Write barrier |
| `bartered` | `memory_order_acq_rel` | RMW exchange |
| `seq` | `memory_order_seq_cst` | Total order (default) |

**Field declarations:**

```briev
relaxed atomic count: Int;        // default ordering for this field
acquire atomic flag: Bool;        // every read is acquire
release atomic payload: Int[256]; // every write is release
bartered atomic ref_count: Int;   // RMW is acq_rel
seq atomic state: Int;            // total order (default, optional)
atomic ready: Bool;               // same as seq (backward compatible)
```

**Intrinsics parameterized:**

| Intrinsic | Current | New |
|-----------|---------|-----|
| `AtomicLoad#(ptr)` | `seq_cst` | `AtomicLoad#(ptr, Relaxed)` |
| `AtomicStore#(ptr, val)` | `seq_cst` | `AtomicStore#(ptr, val, Release)` |
| `AtomicCas#(ptr, old, new)` | `seq_cst` | `AtomicCas#(ptr, old, new, Bartered, Relaxed)` |
| `AtomicXchg#(ptr, val)` | `seq_cst` | `AtomicXchg#(ptr, val, Bartered)` |
| `AtomicAdd#(ptr, val)` | `seq_cst` | `AtomicAdd#(ptr, val, Bartered)` |
| `Fence#()` | `seq_cst` | `Fence#(Acquire)` |

**Field access inherits from declaration:**
`obj.f = obj.f + 1` on a `bartered atomic` field → `atomicrmw add ... acq_rel`

**New atomics to add:**

| Intrinsic | LLVM Emit |
|-----------|-----------|
| `AtomicSub#(ptr, val, order)` | `atomicrmw sub` |
| `AtomicOr#(ptr, val, order)` | `atomicrmw or` |
| `AtomicAnd#(ptr, val, order)` | `atomicrmw and` |
| `AtomicXor#(ptr, val, order)` | `atomicrmw xor` |
| `AtomicLoadN#(ptr, bytes, order)` | 8/16/32-bit atomics (not just i64) |

**Default:** No keyword = `seq` (backward compatible, zero changes to existing code).

### 4.4 Linker Sections

**Strategy keyword:**

```briev
section(".isr_vector") isr_entry: Ptr<Bits<8>>;
section(".init") defn startup(): Void { ... };
section(".data") let globals: Int = 42;
section(".bss") let buffer: Byte[4096];
```

**Proof obligations:**
- `section` on `defn`: must not allocate, must not call non-`section` functions (transitive check)
- `section` on `let`: initial value must be compile-time constant
- `section(".isr_vector")`: must be `Ptr<T>` or fixed-size `Byte[N]`

### 4.5 ISR Handlers

**Syntax:**

```briev
isr handler @ 0x08: timer_irq() {
    counter = counter + 1;
};
```

- `@ vector` = interrupt vector number
- Body typechecked with ISR restrictions (no `Malloc#`, no `Float`, no `Spawn#`)
- Emits: ISR prologue (save regs) → body → ISR epilogue (restore regs + `iret`)
- Vector table entry emitted automatically

**Open question:** Different architectures have different vector table layouts (ARM Cortex-M vs x86 vs RISC-V). Should `isr` accept a target parameter like `asm<target>`? E.g., `isr<arm_cortex_m> handler @ 0x08: ...`?

### 4.6 Restrict

**Strategy keyword:**

```briev
restrict defn process(a: Ptr<Int>, b: Ptr<Int>) {
    // compiler assumes a and b don't alias
    // emits LLVM noalias attributes on both parameters
};
```

**Open question:** Should `restrict` apply to all pointer params in the function, or to individual parameters? The C model applies it per-parameter (`int *restrict a, int *restrict b`). Briev could do either — function-level is simpler, parameter-level is more precise.

---

## 5. What We Explicitly Dropped

### 5.1 Branch Hints (`#likely`/`#unlikely`)

**Reason:** Briev's philosophy is that the compiler is smarter than the programmer. Branch probabilities should be discovered from profiling (PGO) and static analysis, not from programmer annotations. A programmer saying "this branch is likely" is the compiler admitting defeat.

**What the compiler should do instead:** Use PGO data, static branch prediction heuristics, and loop structure to infer branch weights. If the compiler gets it wrong, that's a compiler bug to fix in the analysis, not a feature to add for the programmer.

### 5.2 Variable-Level Alignment Directives (`#!aligned(N)`)

**Reason:** Alignment is fully inferable from usage. The compiler sees every operation on a value. If the value feeds into `SimdAdd#`, it needs SIMD alignment. If it feeds into `VolatileLoad#`, it needs hardware alignment. If it's passed to an `frgn` function, it needs ABI alignment.

**The escape hatch:** `spec Alignment: N` on type definitions already exists for ABI/hardware constraints. If you need 64-byte alignment for a DMA buffer, define a type with `spec Alignment: 64` and use that type.

**What the v0.14 spec had:** `#!aligned(4096) let buf: Byte[4096];` — designed but never implemented. We're not implementing it because inference is the correct approach.

### 5.3 Target-Specific SIMD Intrinsics

**Reason:** `SseAdd#`, `AvxAdd#`, `NeonAdd#` etc. leak CPU-specific names into user code. They break portability. A program written for x86 AVX shouldn't fail to compile on ARM NEON.

**The alternative:** Portable intrinsics (`SimdAdd#`, `SimdFma#`) that the backend lowers to the best available hardware. If the target doesn't support the feature, emit a scalar fallback + `#?` remark.

---

## 6. Implementation Order

| Phase | Feature | Complexity | Dependencies |
|-------|---------|------------|--------------|
| 1 | Pointer arithmetic intrinsics (`PtrAdd#`, `PtrSub#`, `PtrDiff#`, `PtrEq#`, `PtrLt#`) | Low | None — pure addition, GEP emission |
| 2 | Operator sugar on `Ptr<T>` (`op Add`, `op Sub`) | Low | Phase 1 (intrinsics must exist first) |
| 3 | Atomic ordering strategy keywords (`relaxed`, `acquire`, `release`, `bartered`, `seq`) | Medium | None — lexer + parser changes |
| 4 | Parameterized atomic intrinsics | Medium | Phase 3 (ordering constants must exist) |
| 5 | New atomics (`AtomicSub#`, `AtomicOr#`, `AtomicAnd#`, `AtomicXor#`, `AtomicLoadN#`) | Low | Phase 4 (parameterized intrinsics pattern) |
| 6 | Portable SIMD intrinsics | Medium | None — target detection + fallback emission |
| 7 | SIMD strategy keywords (`simd`, `nosimd`, `simd<N>`) | Low | Phase 6 (intrinsics must exist) |
| 8 | Linker section placement | Medium | None — LLVM section attribute + proof check |
| 9 | ISR handlers | High | Phase 8 (section placement is prerequisite) |
| 10 | Restrict keyword | Low | None — LLVM noalias attribute |

---

## 7. SPEC Updates Required

### `spec/SPEC.md`

| Section | Update |
|---------|--------|
| §2.1 Types have no canonical layout | Add note: alignment is inferred from usage, not annotated at variable level. `spec Alignment` on types covers ABI/hardware constraints. |
| §8.1 Strategy keywords | Add `simd`, `nosimd`, `simd<N>`, `section`, `isr`, `restrict` to strategy keyword list. Add `relaxed`, `acquire`, `release`, `bartered` as context-sensitive atomic qualifiers. Update the "they never make code faster" rule to cover new keywords. |
| §8.2 struct | Add `section` keyword on struct fields/let bindings. |
| §8.9 spec | Clarify: `spec Alignment` is type-level only; variable-level alignment is inferred from usage. |
| §11.4 Iteration | Add `simd`/`nosimd`/`simd<N>` on `foreach` loops. |
| §14.2 Pointer safety | Add `PtrAdd#`, `PtrSub#`, `PtrDiff#`, `PtrEq#`, `PtrLt#` to pointer operations. Document `inbounds` semantics. |
| §15.2 Operator classes | Note: `Ptr<T>` supports `+`, `-` via `op Add`/`op Sub` (if parser supports `impl Ptr<T>`). |
| §15.x Intrinsics (new section or expand existing) | Add all new intrinsics with signatures, proof obligations, and LLVM emission rules. |
| §19.x Atomic operations | Parameterize all atomic intrinsics with ordering. Add ordering constants: `Relaxed`, `Acquire`, `Release`, `Bartered`, `Seq`. Document field-level ordering inheritance. |
| New §: Linker sections | `section(".name")` syntax, proof obligations, LLVM section attribute semantics. |
| New §: ISR handlers | `isr handler @ vector: name() { ... }` syntax, proof obligations, ISR restrictions, vector table emission. |
| New §: Restrict | `restrict defn` syntax, LLVM noalias semantics, proof obligations. |
| New §: Portable SIMD | All `Simd*#` intrinsics, target resolution rules, fallback contract. |

### `docs/architecture/agent-reference.md`

| Section | Update |
|---------|--------|
| §1.1 Naming convention | Add: portable SIMD intrinsics follow PascalCase `#` suffix (`SimdAdd#`, `SimdFma#`). |
| §1.3 Pointer operations | Add `PtrAdd#`, `PtrSub#`, `PtrDiff#`, `PtrEq#`, `PtrLt#` with signatures. |
| §4 Optimization directives | Add `simd`/`nosimd`/`simd<N>` to strategy keyword list. |

### `docs/architecture/hash-words.md`

| Section | Update |
|---------|--------|
| §1 Hashword Categories | Add ordering constants: `Relaxed`, `Acquire`, `Release`, `Bartered`, `Seq` as context-sensitive hashwords (only valid after `atomic`). |
| New section | Add portable SIMD intrinsic hashwords: `SimdLoad#`, `SimdStore#`, `SimdAdd#`, `SimdSub#`, `SimdMul#`, `SimdFma#`, `SimdShuffle#`, `SimdGather#`, `SimdScatter#`. |

### `docs/architecture/backend-contracts.md`

| Section | Update |
|---------|--------|
| Per-backend charters | Add: all backends must support pointer arithmetic intrinsics, portable SIMD with fallback, parameterized atomics. |
| Emission invariants | Add: `PtrAdd#`/`PtrSub#` emit `getelementptr inbounds`; SIMD intrinsics emit target-specific or scalar fallback; atomics emit with user-specified ordering. |

### `src/intrinsic_signatures.rs`

Register all new intrinsics with correct signatures and return kinds.

---

## 8. What Does NOT Change

- `spec Alignment` on types — unchanged, covers ABI/hardware constraints
- Existing atomic intrinsics — backward compatible (default `seq`)
- `seq`/`pack`/`vol`/`async`/`sync`/`atomic`/`union`/`trap` — unchanged
- Directive system (`#`/`#?`/`#!`) — unchanged, new directives not needed
- All existing tests — no behavior changes
- `Ptr<T>` type — unchanged, intrinsics add operations to it
- `Vector<T,N>` type — unchanged, SIMD intrinsics operate on it

---

## 9. Open Questions

1. **`Ptr<T>` as primitive type:** `Ptr<T>` is a bootstrap primitive, not a user `type`. Adding `op Add`/`op Sub` requires care — likely goes through intrinsics, not the standard `op` dispatch. Need to verify parser handles `impl Ptr<T>` correctly. If not, the intrinsics alone provide the same functionality.

2. **SIMD target detection:** How does the backend query target features? Currently `roofline.rs` hardcodes "8-wide SIMD FMA". Need to wire `target_features` from LLVM target machine to the intrinsic lowering. The `config/targets.toml` may need a `[simd]` section listing supported features per target.

3. **ISR vector table format:** Different architectures have different vector table layouts (ARM Cortex-M vs x86 vs RISC-V). Should the `isr` keyword accept a target parameter like `asm<target>`? E.g., `isr<arm_cortex_m> handler @ 0x08: ...`?

4. **`restrict` scope:** Should `restrict` apply to all pointer params in the function, or to individual parameters? The C model applies it per-parameter (`int *restrict a, int *restrict b`). Briev could do either — function-level is simpler, parameter-level is more precise.

5. **`AtomicCas#` ordering parameters:** CAS takes two orderings (success/failure). The current proposal is `AtomicCas#(ptr, old, new, Bartered, Relaxed)`. Should the defaults be different? In C++, the default is `success=seq_cst, failure=seq_cst`.

---

## 10. Summary: The Full Set of Additions

| Category | Additions |
|----------|-----------|
| **Intrinsics** | `PtrAdd#`, `PtrSub#`, `PtrDiff#`, `PtrEq#`, `PtrLt#`, `SimdLoad#`, `SimdStore#`, `SimdAdd#`, `SimdSub#`, `SimdMul#`, `SimdFma#`, `SimdShuffle#`, `SimdGather#`, `SimdScatter#`, `AtomicSub#`, `AtomicOr#`, `AtomicAnd#`, `AtomicXor#`, `AtomicLoadN#` |
| **Strategy keywords** | `simd`, `nosimd`, `simd<N>`, `section`, `isr`, `restrict`, `relaxed`, `acquire`, `release`, `bartered` |
| **Context-sensitive tokens** | `relaxed`, `acquire`, `release`, `bartered` (only valid before `atomic`) |
| **Existing reuse** | `seq` as ordering keyword (already a strategy keyword) |
| **Operator sugar** | `op Add(Int)` and `op Sub(Int)`/`op Sub(Ptr<T>)` on `Ptr<T>` (if parser supports it) |

**No existing code changes behavior. All additions are additive (Rule 6).**
