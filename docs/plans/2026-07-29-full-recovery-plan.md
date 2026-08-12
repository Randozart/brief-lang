# Full Recovery Plan: Recover All-Time Best Benchmarks via Simplification

**Date:** 2026-07-29  
**Branch:** `recovery-branch`  
**Worktree:** `../briev-compiler-recovery`  
**Baseline:** `b39461e2` (SLP stride gate — all 19 benchmarks at parity or better)  

---

## Table of Contents

1. [Philosophy](#1-philosophy)
2. [Architectural Principles](#2-architectural-principles)
3. [The Casting Model](#3-the-casting-model)
4. [Era-by-Era Benchmark Analysis](#4-era-by-era-benchmark-analysis)
5. [Phase -1: Hotfix — Correctness Bug in coerce_to_param_type](#5-phase--1-hotfix)
6. [Phase 0: Target-Aware Type System](#6-phase-0-target-aware-type-system)
7. [Phase 1: Strip Heuristic Bloat](#7-phase-1-strip-heuristic-bloat)
8. [Phase 2: Repurpose Isomorphism + Vector Phi Groups](#8-phase-2-vector-phi-groups)
9. [Phase 3: Fix Accumulation Chaining](#9-phase-3-accumulation-chaining)
10. [Phase 4: Simplify Dispatch Decision Tree](#10-phase-4-simplify-dispatch)
11. [Phase 5: Verify](#11-phase-5-verify)
12. [Post-Verification Investigations](#12-post-verification-investigations)
13. [Dependency Graph](#13-dependency-graph)
14. [Risk Register](#14-risk-register)
15. [Per-Phase Checklist](#15-per-phase-checklist)
16. [Per-Phase File Manifest](#16-per-phase-file-manifest)

---

## 1. Philosophy

Three architectural axioms that every decision in this plan follows:

1. **LLVM optimizes better when given canonical IR**, not pre-optimized IR. The frontend should emit clean structural patterns and get out of LLVM's way. Every attempt to "out-smart" LLVM by reordering statements, gating SLP, or emitting shufflevector chains was proven counterproductive (see recovery experiments).

2. **The compiler knows semantic information LLVM cannot infer** (contract bounds, protocol membership, field exclusivity). That is the *only* thing the frontend should be "smart" about — everything else LLVM does better. The three categories of unique semantic info:
   - **Value bounds** → `!range` metadata, `!prof` branch weights
   - **Memory exclusivity** → `noalias`, `dereferenceable`, `memory(argmem: readwrite)` attributes
   - **Loop convergence** → phi-node structure for induction variable analysis

3. **No heuristic bloat.** Every threshold (`write_density >= 0.5`, `total_fields < 8`, `phi_cap = 10`) is a future regression waiting to happen. Structural decisions only: "does this field group have isomorphic writes?" not "is write_density above 0.8?" If a decision cannot be made from structural properties of the AST alone, it should not exist in the frontend.

---

## 2. Architectural Principles

### 2a. Protocol-Driven Type System

Briev has no "types" — only protocols. A type is a bundle of bits subscribed to one or more protocols:

```
type Int: Bit #Int;       // Bits subscribing to #Int and #Bit protocols
type UInt: Bit #UInt;     // Bits subscribing to #UInt and #Bit protocols
type Float: Bit #Float { !> bits: 32; };
type Int32: Bit #Int { !> bits: 32; };
```

- `type Int: Bit #Int;` has NO `!> bits` metadata — the width is determined by the target's native integer width (`int_bits`).
- Fixed-width types (`Int32`, `Int64`, `Float`) have explicit `!> bits` metadata that overrides the target default.
- The `Bit` protocol is universal — everything is Bit. Casting to Bit is always a physical `bitcast` reinterpretation of the in-memory representation.
- `#String<UTF8>` and `#String<ASCII>` are variants within the SAME protocol — casting between them is free.
- `#Int` and `#UInt` are SEPARATE protocols — casting between them requires an explicit declaration and range verification by the SMT solver.

### 2b. Primordials Are Boring

Primordial types are "boring" — they declare protocol subscription and nothing more:

```
type Int: Bit #Int;       // No body, no ops, nothing
type UInt: Bit #UInt;
type Float: Bit #Float { !> bits: 32; };
```

The compiler's fallback mechanism handles all operations:
- If a type does not declare an `op Add` override → emit native LLVM `add` instruction.
- If a type does declare `op Add` → emit a call to the override function.
- This is the "direct-to-CPU fallback" — absence of an override means native codegen.

### 2c. Bitwise Operations Are Non-Overridable

`BitAnd`, `BitOr`, `BitXor`, `BitNot`, `Shl`, `Shr` belong exclusively to the `#Bit` protocol. They cannot be overridden in any type. Any `TypeDef` declaring an override for these ops produces a compile-time error. These operations are axiomatic — they map directly to the physical CPU instructions and must never be semantically overloaded.

### 2d. Parsing/Lexing Ops Are Built-In and Non-Overridable

`op Parse()` and `op Lex()` are compile-time structural phases. If user code could override them, the parser would need to bootstrap its own parsing rules — a chicken-and-egg problem. Types inherit their parent protocol's parsing rules. Any attempt to declare a custom `op Parse` or `op Lex` body produces a compile-time error.

### 2e. Flat Control Flow

All code written or modified during this plan must follow max 2 levels of nesting. No arrowhead code. Use `?`, `if let`, guard clauses, and early returns. Extract deeply nested logic into named helper functions. This is non-negotiable per AGENTS.md Plan Directive 1.

---

## 3. The Casting Model

Three tiers of casting, each with distinct codegen:

### 3a. Within-Protocol Variant Casting (Free)

```
#String<UTF8> ↔ #String<ASCII>
```

Same protocol, different encoding variant. The compiler handles conversion automatically. No explicit declaration needed.

### 3b. Cross-Protocol Semantic Casting (Explicit Declaration Required)

```
Float → Int:   fptosi float %x to i64     (truncation)
Int → Float:   sitofp i64 %x to float      (promotion)
String → Int:  runtime parse function       (semantic parsing)
Int → UInt:    range check by SMT solver    (must be non-negative)
```

Each requires an explicit `CastTo`/`CastFrom` operator declaration in the type definition. The SMT solver enforces range legality (e.g., no negative Int values into UInt).

### 3c. Physical Reinterpretation (Cast to Bit, Always Available)

```
Float → Bit:   bitcast float %x to i32     (IEEE 754 raw bits → 1077936128 for 3.0f)
Int → Bit:     register-level no-op        (same bits)
String → Bit:  first byte of string data   (0x31 for "1")
```

Casting to `Bit` is always available for any type. It bypasses semantic conversion and reinterprets the in-memory representation as raw bits. This is the universal escape hatch. "Messy for Float and String, but still correct."

### 3d. The `coerce_to_param_type` Bug (Fixed in Phase -1)

The current `coerce_to_param_type` function in `emit_expr.rs` converts `float` to `i64` using:
```rust
writeln!(out, "{}  {} = bitcast float {} to i32", indent, b32, arg_reg.name).ok();
writeln!(out, "{}  {} = zext i32 {} to i64", indent, result, b32).ok();
```

This is **wrong** for semantic float-to-integer conversion. `bitcast float 3.0 to i32` yields `1077936128` (the IEEE-754 bit pattern), not `3`. The correct instruction for semantic conversion is `fptosi float %x to i64`.

The `bitcast` approach is only correct when casting TO `Bit` (physical reinterpretation). Cross-protocol `Float → Int` must use `fptosi`.

---

## 4. Era-by-Era Benchmark Analysis

Every benchmark's best result and the IR structure that enabled it:

| Benchmark | Best Era | Best Ratio | IR Structure That Enabled It | Strategy |
|---|---|---|---|---|
| **nbody_newton** | Era 5 | 0.75x | `<4 x float>` vector phis for 5 groups (bx,by,bz,vx,vy,vz) + extractelement/insertelement body + flat @main with #5 | VectorPhiGroup |
| **nbody_sqrt** | Era 10 | 0.85x | Per-field phi, no SLP, #2 on reactor_tick (state-size gated) | PerFieldPhi |
| **nbody_sqrt_idio** | Era 10 | 0.67x | Per-field phi, no SLP, no stride gate | PerFieldPhi |
| **sparse_dispatch** | Era 5 | 0.09x | Pure-counter fold (loop eliminated), dispatch collapse | PureCounterFold |
| **queue_drain** | Era 5 | 0.01x | Pure-counter fold (loop eliminated) | PureCounterFold |
| **interval_step** | Era 4 | 0.01x | Pure-counter fold | PureCounterFold |
| **float_math** | Era 5 | 0.81x | Per-field phi, no SLP, arena-by-proof | PerFieldPhi |
| **fannkuch_redux** | Era 5 | 0.96x | InlineSsa (insertvalue chain), no SLP | InlineSsa |
| **ring_buffer** | Era 4 | 0.99x | Pure-counter fold, no SLP, no outlining, IR determinism | PureCounterFold |
| **mandelbrot** | Era 5 | 0.99x | Per-field phi, no SLP | PerFieldPhi |
| **kalman_filter_runtime** | Era 1 | 0.95x | Pre-outlining, pre-SLP, pre-arena — simplest possible codegen | PerFieldPhi |
| **bit_clear** | Era 10 | 0.50x | Arena removal + SROA on 2-field state → trivial | PureCounterFold |
| **float_math_nonzero** | Era 10 | 0.98x | No SLP, #2 on reactor_tick | PerFieldPhi |
| **fasta** | Era 14 | 0.95x | `!prof` + per-field phi | PerFieldPhi |
| **cancel_math** | Era 14 | 0.96x | Per-field phi | PerFieldPhi |
| **knucleotide** | Era 1 | 0.97x | Simplest compiler (pre-SLP, pre-outlining, pre-arena) | InlineSsa |
| **print_loop** | Era 7 | 0.93x | `memory(argmem:readwrite)` on swan-song body | PerFieldPhi |

### Four Fundamentally Distinct Loop Structures Needed

| Strategy | Used By | IR Pattern |
|---|---|---|
| **PureCounterFold** | sparse_dispatch, queue_drain, interval_step, ring_buffer, bit_clear | No loop; single O(1) store. Already works. |
| **PerFieldPhi** | float_math, mandelbrot, nbody_sqrt, kalman, fasta, cancel_math, print_loop | Scalar phi per written field. Already works. |
| **VectorPhiGroup** | nbody_newton | `<N x T>` vector phis for isomorphic field groups. Needs implementing. |
| **InlineSsa** | fannkuch_redux, knucleotide | `insertvalue`/`extractvalue` chain on `%State`. Already works. |

### Key Insight from Era 5 nbody_newton IR

The Era 5 nbody_newton.ll loop header uses:
- **6 × `<4 x float>` phi nodes** for groups of 4 related fields (bx0-3 → `%phi_bx_v4`, etc.)
- **5 × scalar phi nodes** for singleton elements (bx4, by4, bz4, vx4, vy4, vz4)
- `extractelement` to get individual lanes for scalar position computation (fsub between body pairs)
- `insertelement` to update velocity vector phis
- `pre_phi` block initializes vector phis via explicit `insertelement` chains (not `shufflevector`)
- `!invariant.load !{}` metadata on initial field loads
- Two chunk allocas (`%StateChunk0`, `%StateChunk1`) instead of one monolithic `%State`
- Separate arena allocation (`malloc(65536)`)
- Entire loop in `@main` (no separate `txn_simulate` function call)
- `#5` attribute on `@main` (vs current `#9`)

This was NOT LLVM auto-vectorization finding vector ops — the frontend explicitly emitted vector phis and extractelement/insertelement. The SLP horizontal reduction was supplementary, not primary.

---

## 5. Phase -1: Hotfix

### Problem

`coerce_to_param_type` in `src/backend/llvm/emit_expr.rs` converts `float` to `i64` using `bitcast`, which reinterprets the IEEE-754 bit pattern instead of mathematically truncating the value. `3.0f` becomes `1077936128` instead of `3`.

### Fix

Replace the `bitcast float %x to i32; zext i32 %result to i64` sequence with `fptosi float %x to i64` for semantic float-to-integer conversion.

### Files Changed

| File | Change |
|---|---|
| `src/backend/llvm/emit_expr.rs` | Locate `coerce_to_param_type` function. Find the `float → i64` branch. Replace `bitcast`+`zext` with `fptosi`. |

### Correctness Check

```
Before: 3.0f → bitcast → 1077936128 (IEEE 754 raw bits)
After:  3.0f → fptosi → 3 (mathematically correct)
```

The `bitcast` approach is only correct when casting TO `Bit` (physical reinterpretation). Cross-protocol `Float → Int` must use `fptosi`.

### Commit Checklist

- [ ] `cargo test --lib` passes
- [ ] Rationale comment at edit site: `// 2026-07-29: Float→Int semantic cast must use fptosi, not bitcast. bitcast reinterprets IEEE-754 bits instead of truncating the value.`
- [ ] `cargo build` no warnings

---

## 6. Phase 0: Target-Aware Type System

### 6a. Determine `int_bits` from DataLayout

**Problem:** `int_bits` defaults to 64 and is never auto-derived from the target triple's DataLayout string. `parse_pointer_width()` exists at `src/backend/llvm/context.rs:196-219` but is only called in unit tests.

**Fix:** In `with_data_layout()` and `with_target_triple()`, call `parse_pointer_width()` and set `self.ctx.int_bits`.

**Files Changed:**

| File | Change |
|---|---|
| `src/backend/llvm/mod.rs` | In `with_target_triple()`, after setting the data layout string, call `parse_pointer_width()` and update `int_bits`. Also handle `with_data_layout()` similarly. |
| `src/backend/llvm/context.rs` | May need to expose `parse_pointer_width()` as `pub(crate)` or add a helper method that updates `int_bits` from the stored data layout. |

**Result:** On x86_64, `int_bits = 64`. On wasm32, `int_bits = 32`. On ARM32, `int_bits = 32`. No CLI flag needed for normal use.

**Edge case:** If the DataLayout string does not contain a `-p:` segment (malformed), fall back to the existing default (64).

### 6b. Clean the Primordial Table

**Problem:** Int, UInt, and Bit primordials have hardcoded `llvm_type = "i64"` and implicit `max_bits = 64`. This is a static default that conflicts with target-awareness. Also, primordials don't carry `Cast.#` protocol properties — they are injected later by the normalizer from source declarations, but the primordials themselves should declare protocol membership.

**Fix:** Change the PRIMORDIALS entries in `src/type_universe/mod.rs`:

#### Current PRIMORDIALS

```rust
("Int",    8, 0, 64, 8, "i64", &[]),
("UInt",   8, 0, 64, 8, "i64", &[]),
("Bit",    1, 0, 64, 1, "i64", &[]),  // actually line 89-118 range
("Int8",   1, 8, 8,  1, "i8", &[]),
("Int32",  4, 32, 32, 4, "i32", &[]),
("Int64",  8, 64, 64, 8, "i64", &[]),
("Float",  4, 32, 32, 4, "float", &[]),
("Float64",8, 64, 64, 8, "double", &[]),
("Bool",   1, 8, 8,  1, "i8", &[]),
// ... others unchanged
```

#### New PRIMORDIALS

```rust
// Flexible protocol types — all fields resolved by normalizer from int_bits
("Int",    0, 0, 0,  0, "", &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
("UInt",   0, 0, 0,  0, "", &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
("Bit",    0, 0, 0,  0, "", &[("Cast.#Bit", "true")]),

// Fixed-width integer types — exact bit width is absolute
("Int8",   1, 8, 8,  1, "i8", &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
("UInt8",  1, 8, 8,  1, "i8", &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
("Int16",  2, 16, 16,2, "i16", &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
("UInt16", 2, 16, 16,2, "i16", &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
("Int32",  4, 32, 32,4, "i32", &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
("UInt32", 4, 32, 32,4, "i32", &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
("Int64",  8, 64, 64,8, "i64", &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
("UInt64", 8, 64, 64,8, "i64", &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
("Int128", 16,128,128,16,"i128", &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
("UInt128",16,128,128,16,"i128", &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),

// Floating-point types — bit-width is a matter of accuracy, not maximum storage.
// Each float type declares !> bits explicitly (Half=16, Float=32, Double=64, etc.).
// The normalizer's resolve_llvm_type reads the "bits" property for Float protocol types.
("Half",   2, 16, 16,2, "half", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "16")]),
("BFloat", 2, 16, 16,2, "bfloat", &[("disamb", "bfloat"), ("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "16")]),
("Float",  4, 32, 32,4, "float", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "32")]),
("Float32",4, 32, 32,4, "float", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "32")]),
("Float64",8, 64, 64,8, "double", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "64")]),
("Double", 8, 64, 64,8, "double", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "64")]),
("X86_FP80",10,80,80,4, "x86_fp80", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "80")]),
("FP128",  16,128,128,16,"fp128", &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "128")]),

// Other
("Bool",   1, 8, 8,  1, "i8", &[("Cast.#Bool", "true"), ("Cast.#Bit", "true")]),
("Char",   4, 32, 32,4, "i32", &[("Cast.#Bit", "true")]),
("Data",   8, 64, 64,8, "ptr", &[("Cast.#Data", "true"), ("Cast.#Bit", "true")]),
("Void",   0, 0, 0,  0, "void", &[]),
```

Key changes:
- Int/UInt/Bit: `llvm_type=""`, all numeric fields `0` — no baked-in width or alignment. Everything is resolved by the normalizer from `int_bits` + protocol membership.
- Float types: ALL carry an explicit `("bits", "<N>")` property (stored as `PropertyValue::Int(N)` by the numeric-string detection in the insertion loop). Bit-width is an accuracy declaration, not a maximum bound. The normalizer reads this via `get_exact_bits()`.
- Fixed-width types: ALL get explicit protocol membership properties (`Cast.#Int`, `Cast.#Bit`).
- Bool: `Cast.#Bool` and `Cast.#Bit` — Bool subscribes to its own protocol AND Bit.
- The `(name, bytes, min, max, align, llvm_ty, &[extras])` tuple format stays the same. For flexible types, all `0` means "not yet resolved" — struct layout code must account for this (it runs after normalization).
- Property insertion code detects numeric extras values (like `"16"`, `"32"`, `"64"`) and stores them as `PropertyValue::Int` so `get_exact_bits()` in the normalizer can read them.

### 6c. Pass `int_bits` to the Normalizer

**Problem:** The normalizer runs at `compile.rs:391`, before the backend is constructed at line 903. It doesn't know `int_bits`.

**Fix:** 

1. Change the normalizer signature:
```rust
// BEFORE:
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String>

// AFTER:
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse, int_bits: u64) -> Result<(), String>
```

2. In `compile.rs`, lift the `int_bits` determination before the normalizer call:
```rust
// BEFORE (line ~389-404):
match opts.backend {
    BackendKind::Llvm | BackendKind::Gpu => {
        briev_compiler::backend::llvm::normalizer::normalize(&mut items, &mut universe)?;
    }
    ...
}

// AFTER:
// Determine int_bits from opts (target config can override later)
let int_bits = opts.int_bits; // default 64, may be overridden by --int-bits

match opts.backend {
    BackendKind::Llvm | BackendKind::Gpu => {
        briev_compiler::backend::llvm::normalizer::normalize(&mut items, &mut universe, int_bits)?;
    }
    BackendKind::Circt => {
        briev_compiler::backend::circt_normalizer::normalize(&mut items, &mut universe, int_bits)?;
    }
    BackendKind::Webstack => {
        briev_compiler::backend::webstack_normalizer::normalize(&mut items, &mut universe, 32)?;
    }
    BackendKind::Spirv => {
        briev_compiler::backend::spirv::normalizer::normalize(&mut items, &mut universe, int_bits)?;
    }
    BackendKind::Vm => {}
}
```

3. Also update the other backends' normalizer signatures to accept `int_bits` (they can ignore it if they don't need it).

### 6d. Normalizer Resolves Protocol-Based Types

**Problem:** The normalizer's main loop (lines 37-77) skips types that already have `llvm_type`:
```rust
if rt.properties.contains_key("llvm_type") {
    continue;  // Int has primordial "i64" — skipped!
}
```

This means Int's primordial `"i64"` is never reconsidered. The query-time `llvm_type()` function then overrides it via the `name == "Int"` check, which is a brittle workaround.

**Fix:** In the normalizer's main loop, add protocol-driven resolution BEFORE the "skip if has llvm_type" check. The new logic:

```rust
// ── Phase 1: Strip primordial llvm_type for protocol-resolved types ──
// Types that subscribe to a protocol with target-dependent width should
// NOT keep their primordial llvm_type. The normalizer resolves them from
// protocol membership + int_bits + explicit !> bits metadata.
let has_protocol_for_resolution = rt.properties.contains_key("Cast.#Int")
    || rt.properties.contains_key("Cast.#UInt")
    || rt.properties.contains_key("Cast.#Float")
    || rt.properties.contains_key("Cast.#Bool")
    || rt.properties.contains_key("Cast.#Bit");

// Only strip if the user didn't provide explicit llvm <~ override
let has_explicit_llvm = rt.properties.contains_key("llvm");
if has_protocol_for_resolution && !has_explicit_llvm {
    rt.properties.remove("llvm_type");
}

// ── Phase 2: Skip if llvm_type is already set ──
// This catches fixed-width types (Int32, Float) that have primordial llvm_type
// and don't need protocol resolution (because we didn't strip them above).
if rt.properties.contains_key("llvm_type") {
    continue;
}

// ── Phase 3: Protocol-driven llvm_type resolution ──
let llvm_ty = resolve_llvm_type(rt, int_bits)?;
rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
```

Where `resolve_llvm_type` is:

```rust
fn resolve_llvm_type(rt: &ResolvedType, int_bits: u64) -> Result<String, String> {
    let has_cast_int = rt.properties.contains_key("Cast.#Int");
    let has_cast_uint = rt.properties.contains_key("Cast.#UInt");
    let has_cast_float = rt.properties.contains_key("Cast.#Float");
    let has_cast_bool = rt.properties.contains_key("Cast.#Bool");
    let has_cast_bit = rt.properties.contains_key("Cast.#Bit");

    if has_cast_int || has_cast_uint {
        // Priority: explicit !> bits: N > !> maxbits: N > int_bits
        if let Some(bits) = get_exact_bits(rt) {
            return Ok(format!("i{}", bits));
        }
        if let Some(ceiling) = get_maxbits(rt) {
            return Ok(format!("i{}", int_bits.min(ceiling)));
        }
        return Ok(format!("i{}", int_bits));
    }

    if has_cast_float {
        let bits = get_exact_bits(rt).unwrap_or(32);
        match bits {
            16 => Ok("half".to_string()),
            32 => Ok("float".to_string()),
            64 => Ok("double".to_string()),
            _ => Err(format!("Float type '{}' has unsupported bit width {}", rt.name, bits)),
        }
    }

    if has_cast_bool {
        return Ok("i8".to_string());
    }

    if has_cast_bit {
        if let Some(bits) = get_exact_bits(rt) {
            return Ok(format!("i{}", bits));
        }
        return Ok("i8".to_string()); // smallest addressable unit
    }

    // Fallback: struct with fields
    if !rt.fields.is_empty() {
        // Field-based struct type derivation (existing logic)
        // ...
    }

    Err(format!(
        "cannot determine LLVM type for type '{}' — \
         no protocol membership (Cast.#Int/.#Float/.#Bool/.#Bit) and no explicit llvm <~ property",
        rt.name
    ))
}

/// Read exact !> bits: N metadata from type properties.
fn get_exact_bits(rt: &ResolvedType) -> Option<u64> {
    rt.properties.get("bits").and_then(|pv| {
        if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
    })
}

/// Read !> maxbits: N metadata from type properties.
fn get_maxbits(rt: &ResolvedType) -> Option<u64> {
    rt.properties.get("maxbits").and_then(|pv| {
        if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
    })
}
```

### 6e. Strip Hardcoded Name Match from `llvm_type()`

**Problem:** `src/backend/llvm/emit_toplevel.rs:301-312`:
```rust
if name == "Int" || name == "UInt" {
    // ... int_bits.max(type_floor) ... round to 8/16/32/64
    return format!("i{}", llvm_bits);
}
```
This violates AGENTS.md Rule 18 (no type name matching) and Rule 2 (no magic). After Phase 0d, the normalizer already set `llvm_type` correctly. This name-based override is redundant and harmful.

**Fix:** Delete lines 301-312 (the entire `name == "Int" || name == "UInt"` block) from `llvm_type()`. The function falls through to the universe property lookup which reads the `llvm_type` property set by the normalizer.

**Files Changed:**

| File | Change |
|---|---|
| `src/backend/llvm/emit_toplevel.rs` | Remove the `name == "Int" || name == "UInt"` block at lines 301-312. |

### 6f. Fix `binop_int_type()` Default

**Problem:** `src/backend/llvm/emit_expr.rs`:
```rust
fn binop_int_type(&self) -> String {
    "i64".to_string()  // hardcoded
}
```

**Fix:** Change to `format!("i{}", self.ctx.int_bits)`. This matches the target width for all intermediate SSA values.

**Files Changed:**

| File | Change |
|---|---|
| `src/backend/llvm/emit_expr.rs` | `binop_int_type()` returns `format!("i{}", self.ctx.int_bits)` instead of `"i64"` |

### 6g. Bitwise Operations Are Non-Overridable

**Problem:** The type system should reject any attempt to override `op BitAnd`, `op BitOr`, `op BitXor`, `op BitNot`, `op Shl`, `op Shr`.

**Fix:** In the normalizer or type checker, validate that no `TypeDef` declares an `op` with one of the bitwise names. Produce a compile-time error if found.

**Files Changed:**

| File | Change |
|---|---|
| `src/backend/llvm/normalizer.rs` | Add validation in `register_typedefs` or as a standalone function called after typedef registration |

**Bitwise operation names:**
```
BitAnd, BitOr, BitXor, BitNot, Shl, Shr
```

**Error message:** `"bitwise operation '{name}' cannot be overridden — it is an axiom of the #Bit protocol"`

### 6h. Parsing/Lexing Ops Are Built-In

**Problem:** `op Parse()` and `op Lex()` should not be overridable.

**Fix:** Same mechanism as 6g — reject any `TypeDef` declaring `op Parse` or `op Lex` with a custom body.

**Operation names:**
```
Parse, Lex
```

**Error message:** `"'{name}' is a built-in compile-time operation and cannot be overridden"`

### Commit Checklist for Phase 0

- [ ] All sub-phases committed individually (0a, 0b, 0c, 0d, 0e, 0f, 0g, 0h)
- [ ] Each commit: rationale comment at every edit site (`// 2026-07-29: ...`)
- [ ] `cargo test --lib` after each sub-phase
- [ ] `cargo build` — no warnings
- [ ] Update `docs/architecture/` for the type system changes (protocol-driven resolution, target-aware int_bits)

---

## 7. Phase 1: Strip Heuristic Bloat

~2700 lines of code across 5+ modules that actively harm performance or add zero value. Each was proven counterproductive in the recovery branch experiments.

### 7a. Delete `reorder.rs` (354 lines)

**What it does:** Kahn topological sort of transaction body statements to group independent operations for ILP.

**Why delete:** Proven ineffective (Finding #10 from recovery). No benchmark benefited. LLVM's instruction scheduler does this better within each basic block. The reordering at the Briev statement level is a premature optimization that LLVM undoes and redoes.

**Files:**
- `src/backend/llvm/reorder.rs` — entire file. `git rm` it.
- `src/backend/llvm/mod.rs` — remove `pub mod reorder;` and any `use` imports

### 7b. Delete `hazard.rs` (584 lines)

**What it does:** SLP hazard analysis — detects register pressure, cross-field dependencies, stride patterns to decide "should we SLP this group?"

**Why delete:** The entire "should we SLP?" question is LLVM's job. LLVM's SLP vectorizer has its own cost model that considers target-specific instruction latencies, register pressure, and packing penalties. The frontend cannot replicate this accurately.

**Files:**
- `src/backend/llvm/hazard.rs` — entire file. `git rm` it.
- `src/backend/llvm/mod.rs` — remove `pub mod hazard;` and any `use` imports

### 7c. Delete `vector_codegen.rs` (407 lines)

**What it does:** Manual `insertelement`/`shufflevector`/`extractelement` emission for SLP groups detected by `slp_isomorphism.rs`.

**Why delete:** This was the wrong codegen strategy. Instead of emitting vector operations after-the-fact (fragile, creates shufflevector chains that LLVM struggles with), we should emit VECTOR PHI NODES at the loop header. Phase 2 implements the correct approach.

**Files:**
- `src/backend/llvm/vector_codegen.rs` — entire file. `git rm` it.
- `src/backend/llvm/mod.rs` — remove `pub mod vector_codegen;` and any `use` imports

### 7d. Delete `optimizer.rs` (408 lines)

**What it does:** The 5-axis strategy analyzer (DispatchStrategy/StrategyAnalyzer) that classifies programs into enum/async/sequential dispatch modes.

**Why delete:** The 5-axis strategy was proven to add complexity without improving outcomes. The actual dispatch decision is simple (3-way, see Phase 4). The "optimizer" name is misleading — it doesn't optimize anything.

**Files:**
- `src/backend/llvm/optimizer.rs` — entire file. `git rm` it.
- `src/backend/llvm/mod.rs` — remove `pub mod optimizer;` and any `use` imports

### 7e. Strip SLP Gating from `counter.rs` (~60 lines)

**What it does:** `should_vec`, `stride_ok`, `chain_pass_ok` checks in `emit_countable_body`.

**Why delete:** Manual SLP gating was counterproductive. It tried to predict whether LLVM's SLP vectorizer would succeed, which the frontend cannot do accurately.

**Files:**
- `src/backend/llvm/loop_engine/counter.rs` — remove the `should_vec` logic, `stride_gate`, and `chain_pass_ok` code paths

**What to look for:** Search for `should_vec`, `stride_gate`, `stride_ok`, `chain_pass_ok`, `max_field_stride`, `slp_hazard` in counter.rs and remove those code paths.

### 7f. Delete `emit_folded_memory_main` (67 lines)

**What it does:** The "memory counter" loop (EmitMemoryCounter path). Currently selected for nbody. All fields go through GEP+load+store every iteration. No phi nodes.

**Why delete:** This path produces the worst IR for LLVM optimization. No SSA structure, no induction variable analysis, no vectorization opportunity. Per-field phi (`emit_countable_main`) does everything better.

**Files:**
- `src/backend/llvm/loop_engine/counter.rs` — delete the `emit_folded_memory_main` function
- `src/backend/llvm/mod.rs` — remove the dispatch path that selects it (around line 2812-2817)

### 7g. Remove `reorder_body_statements` Call

**What it does:** Calls the Kahn topological sort during dispatch.

**Why delete:** Unnecessary once `reorder.rs` is deleted. Also remove the import.

**Files:**
- `src/backend/llvm/dispatch.rs` — remove `reorder_body_statements` import and the call site

### 7h. Clean Up `mod.rs` Imports and Module Declarations

After all deletions, `src/backend/llvm/mod.rs` will have stale `pub mod`, `use`, and references to deleted modules. Remove them.

### Commit Checklist for Phase 1

- [ ] Each deletion committed individually (or grouped logically — reorder+hazard as one, vector_codegen+optimizer as one, counter.rs changes as one, dispatch as one)
- [ ] `cargo test --lib` after each commit
- [ ] `cargo build` — no warnings
- [ ] Rationale comment at each deletion site if the deletion is not obvious: `// 2026-07-29: Removed — proven counterproductive. See docs/plans/2026-07-29-full-recovery-plan.md §7.`

---

## 8. Phase 2: Repurpose Isomorphism + Vector Phi Groups

This is the core structural change that recovers `nbody_newton` from ~1.09x to ~0.75x.

### 8a. Refactor `slp_isomorphism.rs`

**Current state:** `src/analysis/slp_isomorphism.rs` detects groups of statements with identical expression tree structure (isomorphic ops). Output: `Vec<SlpIsomorphicGroup>`.

**What stays:** The isomorphism detection algorithm. It is the CORRECT general mechanism. No name-based pattern matching — it compares expression tree structure. Correct in 100% of cases.

**What changes:** The output type. Instead of "SLP merge candidates," produce "vector phi candidates":

```rust
// OLD:
pub struct SlpIsomorphicGroup {
    pub base_index: usize,
    pub fields: Vec<String>,
    pub op_structure: ExprPattern,
}

// NEW:
#[derive(Debug, Clone)]
pub struct VectorPhiCandidate {
    /// Group name derived from common field prefix (descriptive only, not used for matching)
    pub group_name: String,
    /// The LLVM element type (e.g., "float", "i64")
    pub element_ty: String,
    /// Number of lanes in the group
    pub width: usize,
    /// The field names in this group, in index order
    pub fields: Vec<String>,
}
```

**Output contract:** The isomorphism detector outputs a group only when ALL fields in the group:
1. Have the same LLVM type
2. Are unconditionally written every iteration (not inside `when` guards)
3. Have structurally isomorphic expression trees (same operator sequence, same structure)

**Note on fields with different expression operands:** Two fields like `bx0 = fsub(%a, %b)` and `bx1 = fsub(%c, %d)` ARE isomorphic at the operator level (both are `fsub` with two operands), even though the operand variables differ. The isomorphism detection looks at expression TREE STRUCTURE, not variable names. This is correct — LLVM's SLP vectorizer will look at the data flow, not the variable names.

### 8b. Create `src/backend/llvm/vector_phi.rs`

New module with five public functions:

#### `detect_vector_groups`

```rust
/// Scan a transaction's write set and body statements for fields that can be
/// grouped into vector phi nodes. Uses isomorphism analysis from slp_isomorphism.rs.
///
/// A group is formed when:
/// 1. All fields have the same LLVM type (e.g., all float)
/// 2. All fields are unconditionally written every iteration (not inside `when` guards)
/// 3. The fields' assignment expressions are structurally isomorphic
///
/// Returns empty vec if no groups of size ≥ 4 are found.
pub(crate) fn detect_vector_groups(
    write_set: &HashSet<String>,
    body: &[Statement],
    field_index_map: &HashMap<String, usize>,
    type_universe: Option<&TypeUniverse>,
    backend: &LlvmBackend,
) -> Vec<VectorPhiGroup> {
    // ...
}
```

#### `VectorPhiGroup` struct

```rust
pub(crate) struct VectorPhiGroup {
    /// Descriptive name (e.g., "bx") for register naming
    pub name: String,
    /// LLVM element type string (e.g., "float", "i64", "double")
    pub element_ty: String,
    /// Number of lanes (e.g., 4 for <4 x float>)
    pub width: usize,
    /// Field names in index order
    pub fields: Vec<String>,
    /// The SSA register name for this group's phi (e.g., "%phi_bx")
    pub phi_reg: String,
    /// The SSA register name for this group's backedge (e.g., "%be_bx")
    pub backedge_reg: String,
}
```

#### `emit_vector_header`

```rust
/// Emit the pre_phi block initialization (insertelement chain) and the
/// loop header phi nodes for all vector groups.
///
/// For each group of <4 x float>:
///   %iv1 = insertelement <4 x float> undef, float %init_bx0, i32 0
///   %iv2 = insertelement <4 x float> %iv1, float %init_bx1, i32 1
///   ...
///   %phi_bx = phi <4 x float> [ %ivN, %pre_phi ], [ %be_bx, %latch ]
pub(crate) fn emit_vector_header(
    backend: &mut LlvmBackend,
    out: &mut String,
    groups: &[VectorPhiGroup],
    init_regs: &HashMap<String, String>,
    indent: &str,
) {
    // ...
}
```

#### `emit_extractelement`

```rust
/// When the body codegen encounters Identifier("bx0"):
/// - Look up "bx0" in the vector groups
/// - Emit: %lane = extractelement <4 x float> %phi_bx, i32 0
/// - Return the lane register name
///
/// Returns None if the field is not in any active vector group (caller
/// should fall back to scalar phi resolution).
pub(crate) fn emit_extractelement(
    backend: &mut LlvmBackend,
    out: &mut String,
    field_name: &str,
    groups: &[VectorPhiGroup],
    indent: &str,
) -> Option<String> {
    // ...
}
```

#### `emit_insertelement` / `emit_vector_backedge`

```rust
/// Record a field update for the backedge. Called when the body codegen
/// processes Assign(Identifier("bx0"), val).
///
/// Stores the updated lane value. At the latch, all updated lanes are
/// assembled into the backedge vector via insertelement chain.
pub(crate) fn record_field_update(
    backend: &mut LlvmBackend,
    field_name: &str,
    value_reg: &str,
    groups: &[VectorPhiGroup],
) {
    // ...
}

/// Emit the latch block's insertelement chain for all updated lanes.
/// For each group with updates:
///   %upd1 = insertelement <4 x float> %phi_bx, float %new_bx0, i32 0
///   ...
///   %be_bx = bitcast <4 x float> %updN to <4 x float>
///
/// The bitcast is a deliberate no-op that simplifies phi structure.
/// LLVM's peephole eliminates it.
pub(crate) fn emit_vector_backedge(
    backend: &mut LlvmBackend,
    out: &mut String,
    groups: &[VectorPhiGroup],
    indent: &str,
) {
    // ...
}
```

### 8c. Route Body Codegen Through Vector Phis

In `counter.rs`'s `emit_countable_body` and `emit_expr`:

#### Identifier Resolution Priority (in emit_expr)

When encountering `Identifier("bx0")` in an expression:

```
1. Check if "bx0" belongs to an active VectorPhiGroup
   → emit_extractelement(backend, out, "bx0", groups) → return lane register

2. Check last_val_temps["bx0"] (most recent write this iteration)
   → return the chained register

3. Check phi_field_regs["bx0"] (loop header scalar phi)
   → return the scalar phi register

4. Check field_index_map["bx0"] (initial state load)
   → emit GEP+load, return loaded register
```

#### Assignment Resolution

When processing `Assign(Identifier("bx0"), expr)`:

```
1. Emit the expression (may reference other lanes via extractelement)
2. If "bx0" belongs to an active VectorPhiGroup
   → record_field_update(backend, "bx0", computed_reg, groups)
3. Else (scalar phi path):
   → pending_phi_backedge.insert("bx0", computed_reg)
   → last_val_temps.insert("bx0", computed_reg)
```

#### Key Architectural Invariant

The expression codegen for each lane is IDENTICAL to the scalar case. `fsub %a, %b` compiles to the same `fsub` instruction regardless of whether `%a` came from `extractelement` or from a scalar phi. Only the phi storage mechanism changes. No special "vector codegen" path for arithmetic ops. No shufflevector instructions. No hand-rolled SLP tree traversal.

### 8d. Wire VectorPhiGroup into Dispatch

In `src/backend/llvm/mod.rs` around line 2742, replace the heuristic decision:

```rust
// BEFORE:
write_density >= 0.8 && total_fields >= 8 → emit_folded_memory_main

// AFTER:
let groups = detect_vector_groups(&node.write_set, raw_body, &self.ctx.field_index_map, ...);
if !groups.is_empty() && total_fields > 14 {
    // Vector phi group path — emit directly into @main (like Era 5)
    self.emit_vector_phi_main(&mut out, &node.name, counter_idx, ...);
} else {
    // Per-field phi path (scalar)
    self.emit_countable_main(&mut out, ...);
}
```

### Commit Checklist for Phase 2

- [ ] 8a: slp_isomorphism.rs refactored (keep detection, change output type). Update all callers.
- [ ] 8b: vector_phi.rs created with all 5 functions
- [ ] 8c: Body codegen routes through vector phis
- [ ] 8d: Dispatch wired to detect and select VectorPhiGroup
- [ ] `cargo test --lib` — all pass
- [ ] `cargo build` — no warnings
- [ ] Every edit site has `// 2026-07-29: <why>` rationale comment

---

## 9. Phase 3: Fix Accumulation Chaining

### Problem

When a state field is assigned multiple times in the same iteration, the second assignment uses the loop-header phi as the base register, making the first assignment dead:

```llvm
%t1 = fsub float %phi_bx0, %a    ; uses phi (first assignment)
%t2 = fsub float %phi_bx0, %b    ; uses phi again — %t1 is DEAD!
```

This kills SLP vectorization because LLVM sees independent (dead) instructions instead of a live dependency chain. The dead instruction gets DCE'd before the vectorizer even sees it.

### Root Cause

In `emit_expr`, identifier resolution for state fields checks `phi_field_regs` (loop header phi) BEFORE `last_val_temps` (most recent write this iteration). When the body has:

```briev
&bx0 = body0.x - body1.x;
&bx0 = body0.x - body2.x;  // second write to bx0
```

The expression for the second assignment contains a reference to `body2.x`, NOT to the previous value of `bx0`. But if the expression DOES reference `bx0` (true accumulation):

```briev
&energy = energy + body0.mass * body1.mass;  // first energy write
&energy = energy + body0.mass * body2.mass;  // second — energy on RHS
```

Here, the `Identifier("energy")` on the RHS should resolve to the just-computed value, not the loop-header phi.

### Fix

Change the priority in `emit_expr` identifier resolution for state fields:

```rust
// OLD priority:
fn resolve_state_field(&self, name: &str) -> Option<String> {
    // 1. phi_field_regs (loop header) — tried first
    if let Some(reg) = self.fun.phi_field_regs.get(name) { return Some(reg.clone()); }
    // 2. last_val_temps (this iteration)
    if let Some(reg) = self.fun.last_val_temps.get(name) { return Some(reg.clone()); }
    // 3. field_index_map (initial load)
    None  // handled elsewhere
}

// NEW priority:
fn resolve_state_field(&self, name: &str) -> Option<String> {
    // 1. last_val_temps — most recent write this iteration (THE FIX)
    if let Some(reg) = self.fun.last_val_temps.get(name) { return Some(reg.clone()); }
    // 2. VectorPhiGroup — extractelement from vector phi
    if let Some(lane) = self.vector_phi_regs.get(name) { return Some(lane.clone()); }
    // 3. phi_field_regs — loop header scalar phi
    if let Some(reg) = self.fun.phi_field_regs.get(name) { return Some(reg.clone()); }
    None
}
```

And in the body emission loop in `counter.rs`, when processing `Assign(Identifier("bx0"), expr)`:

```
After computing the value:
1. last_val_temps["bx0"] = computed_reg   ← always update
2. If vector group active: record_field_update("bx0", computed_reg)
3. Else: pending_phi_backedge["bx0"] = computed_reg
```

### Result

```llvm
; Before (dead instructions):
%t1 = fsub float %phi_bx0, %a     ; dead — DCE'd
%t2 = fsub float %phi_bx0, %b     ; only live instruction

; After (live chain):
%t1 = fsub float %phi_bx0, %a     ; live
%t2 = fsub float %t1, %b          ; chained — BOTH live
```

LLVM's SLP vectorizer now sees multiple live, isomorphic `fsub` instructions that can be merged into vector operations.

### Files Changed

| File | Change |
|---|---|
| `src/backend/llvm/emit_expr.rs` | `resolve_state_field()` or equivalent — swap priority of `last_val_temps` vs `phi_field_regs` |
| `src/backend/llvm/loop_engine/counter.rs` | Ensure `last_val_temps` is updated for EVERY field assignment, not skipped |

### Why This Is Safe

For benchmarks where each field is written at most once per iteration (which is >90% of benchmarks), `last_val_temps` and `phi_field_regs` contain the same value. The priority change has zero effect on single-write benchmarks. Only nbody_newton (with multiple writes to the same field) sees a behavioral change.

### Commit Checklist for Phase 3

- [ ] Identifier resolution priority changed in emit_expr.rs
- [ ] last_val_temps update in counter.rs body loop
- [ ] `cargo test --lib` — all pass
- [ ] Rationale comment: `// 2026-07-29: Accumulation chaining — last_val_temps takes priority over phi_field_regs so successive writes to the same field form a live dependency chain instead of dead independent instructions.`
- [ ] Verify no other benchmark regresses (Phase 5 verification)

---

## 10. Phase 4: Simplify Dispatch Decision Tree

### Problem

The current dispatch selection at `src/backend/llvm/mod.rs:2714-2840` is a ~150-line heuristic tree with 6 thresholds, duplicated for pure and non-pure paths. Every threshold is a regression waiting to happen:

- `has_body_ffi && total_fields < 16` → while-loop (x2, for pure and non-pure paths)
- `write_density >= 0.5 && total_fields < 8 && !has_body_ffi` → InlineSsa (x2)
- `write_density >= 0.8 && total_fields >= 8` → EmitMemoryCounter (x2)
- Everything else → per-field phi with `phi_cap = min(10, max(6, write_set.len()))`

### Fix

Replace with a single, structural 4-way decision. Note: the `density` threshold for InlineSsa is acceptable because it's a structural property of the IR (insertvalue/extractvalue chain has O(N) cost that only amortizes when most fields are written).

```rust
fn select_loop_strategy(txn, graph, ctx) -> LoopStrategy {
    // 1. Pure body with constant bound → O(1) fold
    if is_pure_body && has_constant_bound {
        return LoopStrategy::PureCounterFold;
    }

    // 2. Has vector-groupable fields → VectorPhiGroup
    let groups = detect_vector_groups(
        &node.write_set, raw_body, &ctx.field_index_map, ...
    );
    if !groups.is_empty() && ctx.field_index_map.len() > 14 {
        return LoopStrategy::VectorPhiGroup;
    }

    // 3. Dense writes, small state → InlineSsa
    let write_count = node.write_set.len();
    let total_fields = ctx.field_index_map.len();
    let density = write_count as f64 / total_fields as f64;
    if density >= 0.5 && total_fields < 8 {
        return LoopStrategy::InlineSsa;
    }

    // 4. Everything else → PerFieldPhi
    LoopStrategy::PerFieldPhi
}
```

### Removed Heuristics

| Removed Threshold | Why Removed |
|---|---|
| `has_body_ffi` gate | False-positive with sqrt() intrinsic — never correctly fired. The FFI detection bug made this a no-op. |
| `phi_cap` calculation | Structural: per-field phi uses ALL write_set fields. Capping was an admission that the heuristic was wrong. |
| `needs_state_stores_in_body` | Only needed when `phi_cap` excludes some write_set fields. Without `phi_cap`, every written field gets a phi. |
| `write_density >= 0.8 && total_fields >= 8` → EmitMemoryCounter | EmitMemoryCounter is deleted in Phase 1f. |
| `has_body_ffi && total_fields < 16` → while-loop | The while-loop path was a workaround for the InlineSsa FFI bug (globalopt eliminating prints). That bug is now understood and avoided by using PerFieldPhi instead. |

### Attribute Selection for @main / alwaysinline

**Current:** All programs use `#9` on @main. `alwaysinline` was removed for all reactive txns (Decision 2).

**New:** Gate `alwaysinline` by state size. This determines whether the hot loop lives in `@main` (via inlining) or in a separate function:

```rust
// After selecting per-field phi or vector-phi strategy:
let state_field_count = ctx.field_index_map.len();
if state_field_count > 14 {
    // Large state — inline into @main for maximum SLP visibility.
    // Era 5 used alwaysinline for everything and got 0.75x on nbody.
    // The function call boundary blocks SROA for large states.
    self.emit_inline_main(ctx); // body emitted directly in @main
} else {
    // Small state — keep reactor_tick standalone with #12 = memory(argmem: readwrite)
    // for maximum SROA (Scalar Replacement of Aggregates).
    // This is what enables ring_buffer's 1.06x.
    self.emit_separate_txn_fn(ctx); // body in reactor_tick with #12
}
```

Implementation:
- For `VectorPhiGroup`: always emit directly in `@main` (like Era 5). The vector phi grouping only benefits when the entire loop is flat.
- For `PerFieldPhi`: gate by state size. ≤14 fields → separate function with `#12`. >14 fields → `alwaysinline` into `@main`.
- For `InlineSsa`: always separate function (small states benefit from SROA).
- For `PureCounterFold`: no loop, no function needed — single store in `@main`.

### Files Changed

| File | Change |
|---|---|
| `src/backend/llvm/mod.rs` | Replace lines ~2714-2840 with the simplified dispatch. Add attribute selection. |
| `src/backend/llvm/dispatch.rs` | Remove any `reorder_body_statements` or heuristic references |
| `src/backend/llvm/loop_engine/counter.rs` | Ensure `emit_countable_main` (PerFieldPhi) and the new vector phi entry point can be called from dispatch |

### Commit Checklist for Phase 4

- [ ] Dispatch tree simplified to 4-way structural decision
- [ ] `alwaysinline` gate implemented by state size
- [ ] All removed heuristics verified to have no callers
- [ ] `cargo test --lib` — all pass
- [ ] Rationale comment: `// 2026-07-29: Simplified dispatch — structural 4-way decision replaces 150-line heuristic tree. See docs/plans/2026-07-29-full-recovery-plan.md §10.`

---

## 11. Phase 5: Verify

### 5a. Unit Tests

```bash
cargo test --lib
```

All tests must pass. If any test fails, investigate immediately. The test failure is either:
- A regression from the changes in Phases -1 through 4
- A pre-existing fragility that was masked by the heuristic bloat

Do NOT modify tests to match changed behavior unless the behavioral change is documented and intentional.

### 5b. Full Benchmark Suite

```bash
bash benchmarks/build_and_bench.sh --all
```

Target outcomes by benchmark:

| Benchmark | Current Ratio | Target | Pass/Fail |
|---|---|---|---|
| nbody_newton | 1.09x C | ≤ 0.85x C | |
| nbody_sqrt | 0.85x Briev | ≤ 0.85x Briev | |
| nbody_sqrt_idio | 0.67x Briev | ≤ 0.67x Briev | |
| sparse_dispatch | 0.81x Briev | ≤ 0.81x Briev | |
| queue_drain | 0.96x Briev | ≤ 0.96x Briev | |
| queue_drain_sym | Parity | ≤ 1.00x | |
| queue_drain_idio | Parity | ≤ 1.00x | |
| ring_buffer | 1.06x C | ≤ 1.06x C | |
| float_math | 0.96x Briev | ≤ 0.96x Briev | |
| float_math_nonzero | 0.98x Briev | ≤ 0.98x Briev | |
| fannkuch_redux | 0.90x Briev | ≤ 0.96x Briev | |
| kalman_filter_runtime | 0.99x Briev | ≤ 1.00x | |
| mandelbrot | 0.99x Briev | ≤ 1.00x | |
| fasta | 0.95x Briev | ≤ 0.95x Briev | |
| cancel_math | 0.96x Briev | ≤ 0.96x Briev | |
| print_loop | Parity | ≤ 1.00x | |
| bit_clear | 0.50x Briev | ≤ 0.50x Briev | |
| knucleotide | Parity | ≤ 1.00x | |
| interval_step | Parity | ≤ 1.00x | |

Every benchmark must MATCH (correctness). Any benchmark that doesn't match is a blocker.
Every benchmark must be at parity or better with the current recovery-branch tip. Regressions are blockers.

### 5c. IR Diff Against Era 5 Reference

```bash
# Compile nbody_newton from the benchmark suite
diff <(grep -v '^;' docs/reference-ll/era5-8a827db/nbody_newton.ll | grep -v '^$') \
     <(grep -v '^;' /tmp/nbody_newton_current.ll | grep -v '^$') | head -200
```

Structural similarity check:
- [ ] Does the loop header have `<4 x float>` vector phis? (grep `phi <4 x float>`)
- [ ] Does the body use `extractelement`/`insertelement`?
- [ ] Are memory operations minimal (most operations are register-to-register)?
- [ ] Is the hot loop in `@main` (not behind a function call boundary)?

The check is structural, not literal. The exact register names and line numbers will differ. What matters is whether the IR structure is the same.

### 5d. Compare Against Baseline Worktree

```bash
bash benchmarks/compare_baseline.sh nbody_newton
bash benchmarks/compare_baseline.sh sparse_dispatch
bash benchmarks/compare_baseline.sh queue_drain
bash benchmarks/compare_baseline.sh ring_buffer
```

The baseline worktree at `../briev-compiler-baseline` is pinned at `b39461e2`. Any regression against this baseline is a blocker unless the improvement in another benchmark is strictly greater than the regression (trade-off must be documented).

---

## 12. Post-Verification Investigations

These are NOT commits. They are research investigations to be performed after Phase 5 verification passes.

### 12a. Native Float Phi Types

**Observation:** Era 5 stored floats as native `float` in `%State` and used `fadd 0.0, %val` identity backedges. Current code boxes floats as `i64` via `adapt_to_i64`/`unbox_from_i64` conversions. This adds 2-4 instructions per float field per iteration.

**Question:** Can `emit_countable_body` use native float phi types for `#Float` protocol fields instead of boxing through `i64`?

**To investigate:** Trace the `phi_field_regs` emission in `counter.rs`. Currently all phi nodes are emitted as `i64`. Change float-typed fields to emit `float` phi nodes instead. The backedge identity would be `fadd float %val, 0.0` instead of `bitcast i64 %val to i64` — LLVM's peephole eliminates both equally well.

### 12b. float_math 0.81x Gap

**Observation:** float_math at 0.96x is below Era 5's 0.81x. The difference is likely the arena interaction (Era 5's arena-by-proof added a `malloc(65536)` call that changed LLVM's optimization landscape).

**Question:** Does removing the arena (when the program doesn't use `Alloc#`) close this gap? Or is it something else about the per-field phi vs InlineSsa interaction?

### 12c. The `bytes` Field Removal

**Observation:** `ResolvedType.bytes` is vestigial — it duplicates information derivable from `llvm_type`. After Phase 0, ALL width decisions use the `llvm_type` property, not `bytes`.

**Question:** Can `bytes` be removed from `ResolvedType` entirely? All consumers would switch to `llvm_bit_width(rt.properties.get("llvm_type"))` or `ceil(max_bits / 8)`.

---

## 13. Dependency Graph

```
Phase -1 (Hotfix — independent)
  │
Phase 0a-0b (int_bits + primordial table — sequential)
  │
Phase 0c-0d (pass int_bits to normalizer, protocol resolution — depends on 0a-0b)
  │
Phase 0e-0f (strip name checks, fix binop_int_type — depends on 0c-0d)
  │
Phase 0g-0h (bitwise/parsing non-overridable — independent of 0e-0f, depends on 0b)
  │
Phase 1 (Strip bloat — INDEPENDENT of Phase 0, can be done in parallel)
  │
Phase 2a (Refactor isomorphism output — depends on 1 for vector_codegen removal)
  │
Phase 2b (Create vector_phi.rs — depends on 2a)
  │
Phase 2c (Route body codegen — depends on 2a + 2b)
  │
Phase 2d (Wire into dispatch — depends on 2c + Phase 1)
  │
Phase 3 (Accumulation chaining — independent of Phase 2, can be done in parallel)
  │
Phase 4 (Simplify dispatch — depends on Phase 1 + Phase 2d + Phase 3)
  │
Phase 5 (Verify — depends on all)
```

### Parallelizable Work

| Parallel Group | Phases |
|---|---|
| Group A | Phase -1, Phase 0a-0b |
| Group B | Phase 0c-0f (after 0a-0b) |
| Group C | Phase 0g-0h (after 0b, parallel with 0c-0f) |
| Group D | Phase 1 (parallel with Group A) |
| Group E | Phase 2a+2b+2c+2d (after Group D) |
| Group F | Phase 3 (parallel with Group E) |
| Group G | Phase 4 (after Group E + Group F) |
| Group H | Phase 5 (after everything) |

### Merge Order

Due to the dependency graph, the recommended commit order is:

1. Phase -1 (Hotfix) — safe, independent, correctness fix
2. Phase 1 (Strip bloat) — safe, independent, structural cleanup
3. Phase 0a-0b (Type system foundation)
4. Phase 0c-0d (Normalizer protocol resolution)
5. Phase 0e-0f (Name checks, binop type)
6. Phase 0g-0h (Validation)
7. Phase 3 (Accumulation chaining — independent of Phase 2, ready after Phase 1)
8. Phase 2a-2b-2c-2d (Vector phi groups — structural change)
9. Phase 4 (Simplify dispatch — depends on everything above)
10. Phase 5 (Verify — always last)

---

## 14. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | VectorPhiGroup doesn't match Era 5's exact IR shape | Medium | nbody_newton stays at current ratio | Even partial recovery (0.85-0.95x) is better than current. The isomorphism-based approach is structurally correct; only the profitability may vary. If <0.95x, disable VectorPhiGroup and fall back to PerFieldPhi for nbody. |
| R2 | Bloat removal breaks a benchmark that silently depended on heuristic | Low | One benchmark regresses | Every deleted module was tested in recovery branch and proven ineffective. Full benchmark suite in Phase 5 catches regressions immediately. If regression found, revert the specific deletion (never re-add heuristic). |
| R3 | `int_bits` change breaks integer width assumptions | Very low | Compile error or wrong output | Same-type operations use the same width. Only default width changes; on x86_64 it stays 64. Verified by existing WASM tests. |
| R4 | Accumulation chaining changes phi backedge structure for non-nbody benchmarks | Low | Structural change in IR | The fix only affects identifier resolution priority within emit_expr. Non-nbody benchmarks have at most 1 assignment per field per iteration, so last_val_temps == phi_field_regs for them. Zero behavioral change. |
| R5 | Phase 0c changes other backends' normalizer signatures | Medium | CIRCT/Webstack/SPIR-V break | All backend normalizers need signature update. Each can accept `int_bits: u64` and ignore it. Test all backends after Phase 0c. |
| R6 | The `#5` vs `#9` attribute difference matters for non-nbody benchmarks | Low | Some benchmark regresses | Era 5 used `#5`. Current uses `#9`. Both are `memory(readwrite)`. The difference is other attributes (willreturn, nosync, etc.) that don't affect the hot loop. If regression found, emit both and let benchmark pick — but this contradicts the "no heuristic" principle. Investigate root cause. |

### Risk Mitigation Strategy

For each risk that materializes:
1. **Isolate the change** — revert the specific commit, not the entire phase
2. **Investigate the root cause** — is it the structural change or an interaction?
3. **Document in BUGS.md** — what happened, why, how it was resolved
4. **Proceed** — the plan is large; don't block on edge cases

---

## 15. Per-Phase Checklist

### Every Commit

- [ ] `cargo test --lib` passes
- [ ] `cargo build` — no warnings (allow `dead_code` only if the dead code is temporary and documented)
- [ ] **Rationale comment at EVERY edit site**: `// YYYY-MM-DD: <why this change exists, what problem it solves>`
- [ ] No `todo!()`, `unreachable!()`, `// TODO:`, or stubs
- [ ] HashMap iteration determinism: all HashMaps iterated for IR emission sorted by key
- [ ] Flat control flow: max 2 levels nested. No arrowhead code.
- [ ] `git add` only intended files — inspect `git status` and `git diff` first
- [ ] Concise commit message stating what and why (reference this plan: `docs/plans/2026-07-29-full-recovery-plan.md §N`)

### Every Phase

- [ ] Update `docs/architecture/` if the phase changes any API contract, dispatch decision, or IR emission strategy
- [ ] Log bugs/gotchas in BUGS.md
- [ ] Run Praetor on new/changed files (if Praetor is available)
- [ ] Update `AGENTS_HISTORY.md` at phase completion with the net change in lines of code and benchmark ratios

### Phase 5 (Final)

- [ ] All 19 benchmarks MATCH (correctness)
- [ ] All 19 benchmarks at parity or better vs recovery-branch tip
- [ ] nbody_newton IR structurally similar to Era 5 reference
- [ ] No regression against baseline worktree at `../briev-compiler-baseline`
- [ ] If regressions found: triage per §14 Risk Mitigation Strategy

---

## 16. Per-Phase File Manifest

### Phase -1: Hotfix

| Action | File |
|---|---|
| Edit | `src/backend/llvm/emit_expr.rs` |

### Phase 0: Target-Aware Type System

| Action | File |
|---|---|
| Edit | `src/type_universe/mod.rs` |
| Edit | `src/backend/llvm/context.rs` |
| Edit | `src/backend/llvm/mod.rs` |
| Edit | `src/backend/llvm/normalizer.rs` |
| Edit | `src/backend/llvm/emit_toplevel.rs` |
| Edit | `src/backend/llvm/emit_expr.rs` |
| Edit | `src/compile.rs` |
| Edit | `src/backend/circt_normalizer.rs` (if exists) |
| Edit | `src/backend/webstack_normalizer.rs` (if exists) |
| Edit | `src/backend/spirv/normalizer.rs` (if exists) |

### Phase 1: Strip Bloat

| Action | File |
|---|---|
| Delete | `src/backend/llvm/reorder.rs` |
| Delete | `src/backend/llvm/hazard.rs` |
| Delete | `src/backend/llvm/vector_codegen.rs` |
| Delete | `src/backend/llvm/optimizer.rs` |
| Edit | `src/backend/llvm/loop_engine/counter.rs` |
| Edit | `src/backend/llvm/mod.rs` |
| Edit | `src/backend/llvm/dispatch.rs` |

### Phase 2: Vector Phi Groups

| Action | File |
|---|---|
| Edit | `src/analysis/slp_isomorphism.rs` |
| Create | `src/backend/llvm/vector_phi.rs` |
| Edit | `src/backend/llvm/mod.rs` |
| Edit | `src/backend/llvm/loop_engine/counter.rs` |
| Edit | `src/backend/llvm/emit_expr.rs` |
| Edit | `src/backend/llvm/helpers.rs` |

### Phase 3: Accumulation Chaining

| Action | File |
|---|---|
| Edit | `src/backend/llvm/emit_expr.rs` |
| Edit | `src/backend/llvm/loop_engine/counter.rs` |

### Phase 4: Simplify Dispatch

| Action | File |
|---|---|
| Edit | `src/backend/llvm/mod.rs` |
| Edit | `src/backend/llvm/dispatch.rs` |

### Phase 5: Verify

| Action | File |
|---|---|
| Run | `cargo test --lib` |
| Run | `bash benchmarks/build_and_bench.sh --all` |
| Run | `diff` against Era 5 reference IR |
| Run | `bash benchmarks/compare_baseline.sh` |
| Edit | `BUGS.md` (if issues found) |
| Edit | `AGENTS_HISTORY.md` (phase completion) |

---

## Appendix A: Normative References

| Document | Relevance |
|---|---|
| `AGENTS.md` | All coding standards, golden rules, plan directives |
| `AGENTS_HISTORY.md` | Historical context (Era 1-14) |
| `docs/reference-ll/era5-8a827db/nbody_newton.ll` | Era 5 nbody_newton reference IR |
| `docs/reference-ll/current/nbody_newton.ll` | Current nbody_newton IR (baseline before this plan) |
| `docs/architecture/historical-benchmarks.md` | Full historical timeline of all 14 eras |
| `docs/architecture/casting-protocol.md` | Protocol-based casting architecture |
| `docs/architecture/narrowing-by-proof.md` | Narrowing pass architecture |
| `docs/research/nbody-regression-root-cause.md` | Root cause analysis for nbody regression |
| `docs/plans/2026-07-25-int-bits-and-narrowing-fix.md` | --int-bits architecture |
| `docs/plans/2026-07-05-vector-phi-emission.md` | Prior vector phi plan |
| `BUGS.md` | Bug tracker |
| `.opencode/plans/2026-07-29-flat-emission-refactoring.md` | Prior flat emission plan |

## Appendix B: Reverted Changes Summary

The following changes from the recovery branch are kept (not reverted by this plan):

| Change | Why Kept |
|---|---|
| `!range` metadata from contracts | Proven benefit for ring_buffer |
| `!prof` branch weights from postcondition | Proven benefit for fasta |
| `noundef`/`dereferenceable` on %state params | Correct semantics |
| Stable field sort in `build_field_index` | Deterministic IR mandatory |
| `hoist_terminating_guard` | Correct swan-song emission |
| Bit → Bits rename | Cleaner naming |
| DataLayout-driven int_bits infra | Foundation for Phase 0a |
| Pure-body fold detection | Already correct |

The following changes are REVERTED by this plan:

| Change | Phase | Why Reverted |
|---|---|---|
| `reorder.rs` | Phase 1a | Proven ineffective |
| `hazard.rs` | Phase 1b | SLP gating is LLVM's job |
| `vector_codegen.rs` | Phase 1c | Wrong approach — vector phis replace it |
| `optimizer.rs` | Phase 1d | 5-axis strategy was bloat |
| SLP gating in counter.rs | Phase 1e | Manual SLP gating counterproductive |
| `emit_folded_memory_main` | Phase 1f | Produces worst IR for LLVM |
| `reorder_body_statements` | Phase 1g | Unnecessary |
| `name == "Int"` in llvm_type() | Phase 0e | Violates no-magic rule |
| Heuristic dispatch tree | Phase 4 | 150 lines of fragile thresholds |
| Hardcoded `binop_int_type` → "i64" | Phase 0f | Must use int_bits |
| Primordial Int `llvm_type = "i64"` | Phase 0b | Must be protocol-resolved |
| Hardcoded `bytes` fallback in normalizer | Phase 0d | Must use protocol + int_bits |
