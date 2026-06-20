# The Bits Thesis — Architecture

**Date:** 2026-06-20  
**Status:** Implemented across Phases 1-5  
**See also:** `lib/std/from-bits.bv` (educational type definitions)

## One-Sentence Thesis

Every type in Brief is a lens over `Bits`. Operator sigils (`<:`, `:>`, `@/`, `[]`, `<-`) are spatial layout operations on a bit vector, not nominal abstractions. The compiler recognizes common lens shapes and emits native LLVM IR directly.

## Three Operator Groups

| Group | Operators | Purpose |
|-------|-----------|---------|
| **Lens Operators** | `<:` (Derivation), `:>` (Projection) | Define type boundaries and view meaning through them |
| **Partition Operators** | `[]`, `@/` | Segment a layout into addressable sub-ranges |
| **Transfer Operator** | `<-` | Move data across layout boundaries |

The **Anchor** (`@`) is the universal symbol for spatial/temporal location across all groups.

## What This Means for the Compiler

### 1. Type Layout is Explicit, Not Nominal

A `String` is not magically "a string type." It is `Bits @/0..127` with two fields:

```
@/0..63   → pointer to UTF-8 data
@/64..127 → byte length
```

The compiler does not hardcode "String semantics." It recognizes the shape
`{ptr: Bits @/0..63, len: Bits @/64..127}` and optimizes accordingly.

### 2. Silent Defaults

When a type derives from `Bits @/0..63`, the compiler infers:

| Property | Derivation |
|----------|-----------|
| `Bytes` | `ceil(64 / 8) = 8` |
| `Alignment` | 8 (pointer width) |
| `Endian` | 0 (little-endian by target) |

These are overridable in the TypeDef body.

### 3. Fast-Path Shape Recognition

The compiler does NOT dynamically evaluate `Add(rhs) = _ + rhs` at runtime.
Instead, it recognizes the shape (name="Add", Int source, Int arg) and
emits `add i64` directly — a single LLVM instruction.

The fast-path registry (`src/backend/llvm/emit_expr.rs:3944`) handles
45+ known (type, operator) pairs. User-defined types matching these
shapes get the same optimized codegen.

### 4. TBAA as Field-Index Disambiguation

Because all values are `Bits`, TBAA metadata uses field-index-based
naming (`state_field_N#`) rather than type-name hierarchy. This lets
LLVM's alias analysis disambiguate two `Int` fields at different state
indices even though they have the same nominal type.

### 5. The Anchor is Universal

The `@` symbol anchors a value to a position:

| Context | Meaning | Example |
|---------|---------|---------|
| Prior state | Value at start of tick | `@balance` |
| Bit position | Absolute bit offset | `@/0..63` |
| String literal | Compile-time memory slot | `@"..."` |
| Timer link | Hardware/OS timer | `@ 1kHz` |
| Memory address | Physical address | `@ 0x40020000` |

## FAQ

### "But what about Float? It's not Bits!"

Float IS Bits — IEEE 754 single-precision is a 32-bit layout. The
difference is that the ALU interprets those bits as a float, and
the compiler emits `fadd` instead of `add`. The Bits Thesis is
about **storage**, not **semantics**. A float is `Bits @/0..31`
with float projections (`fadd#`, `fsub#`, etc.) that instruct the
backend to use float registers and instructions.

### "CBV doesn't recognize String or HashMap. Is that a bug?"

No — CBV (Circuit Brief, `.cbv`) targets hardware synthesis. In
hardware, there is no heap, no allocator, no pointer chasing.
String and HashMap are host-tier types that CBV silently drops.
Properties like `Codec` and `ElementType` have no circuit meaning
and are ignored. This is called **tier ignorance** — each backend
recognizes only the subset of metadata that applies to its material.

### "Doesn't this kill performance? All those projections to evaluate?"

No — this is the central insight. The compiler evaluates projections
at **compile time**, not runtime. The fast-path registry catches 45+
well-known patterns and emits the exact same LLVM IR as a hand-written
C program would produce. A user-defined type like `Matrix4x4` that
defines `At(col, row) = _ @/((col*4+row)*32)..((col*4+row)*32+31)`
gets the same `lshr + and` codegen as a native `Int @/` access.

The only cost is in the compiler, not the runtime.

### "What about cross-language FFI? Won't I pay for conversion?"

Zero-cost, if you use lazy lenses. A `CString` lens over a raw `char*`
adds zero instructions at the FFI boundary — it's a metadata-only
change. `strlen` runs only if you explicitly query `Size`. Character
access (`At(i)`) is direct pointer arithmetic. See `lib/std/from-bits.bv`
for the `CString` definition and Section 13 of the Bits Thesis plan
for the full design.

### "Can Brief replace C/Rust for low-level work?"

For layout-defined types — structs, bitfields, collections, arrays —
Brief matches or beats C's codegen because the compiler has more
information (contracts, ranges, provenance) to feed LLVM's optimizer.
For heavily tuned assembly (hand-unrolled SIMD, custom synchronization),
Brief can still call C/Rust via `frgn from "c"` — but the layout of
those calls' data is described in Brief, so there's no marshalling cost.

## Implementation Status

| Component | Phase | Status |
|-----------|-------|--------|
| `@/` in expression context (word @/0..3) | 4 | ✅ shift+mask in interpreter + LLVM |
| `@/` in type context (type Int <: Bits @/0..63) | 4 | ✅ TypeUniverse auto-computes Bytes |
| `TypeBinding` replaces `TypeProperty` | 2 | ✅ 13 variants eliminated |
| `BinaryOp`/`UnaryOp` unification | 3 | ✅ Parser produces canonical structs |
| Backend fast-path registry | 3.5 | ✅ 45+ projection fast paths |
| TypeUniverse pipeline wiring | 3.5 | ✅ main.rs → typechecker → LLVM |
| Educational `from-bits.bv` | 5 | ✅ All fundamental types documented |
| `strlen#` intrinsic | future | 🚧 Planned for CString lazy lens |
| `#export` directive | future | 🚧 Planned for cross-language export |
| Autogenous binding generation | future | 🚧 Planned for auto .h/crate generation |
