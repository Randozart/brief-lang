# Unified Brief 3.0 Architecture: Silicon-Software Convergence

## 1. Vision
Brief 3.0 unifies the **Logic Engine** and **Grammar** across Core Brief (.bv), Rendered Brief (.rbv), and Embedded Brief (.ebv). This allows for **Silicon-Software Co-Design**, where the same logic can be validated in a browser (WASM) and synthesized into physical hardware (SystemVerilog/MCU).

## 2. Separation of Concerns (Containers vs. Language)
While the underlying language engine is unified, the file containers maintain strict boundaries:
- **Rendered Brief (.rbv):** The exclusive container for UI, HTML (`<view>`), and CSS (`<style>`).
- **Embedded Brief (.ebv):** The exclusive container for Silicon Pin Mapping and Memory-Mapped I/O (MMIO) address definitions.
- **Core Brief (.bv):** The shared substrate for platform-agnostic math, state machines, and business logic.

The "Unification" refers to the fact that every Brief script, whether inside an `.rbv` UI or an `.ebv` hardware file, now understands Hardware Geometry and Vector Lifting.

## 3. Core Concepts

### 3.1. Hardware Geometry (Vectors)
The `[]` syntax in Brief 3.0 represents **Physical Geometry**, not just data structures.
- `let pixels: UInt[1024]` defines a block of 1024 discrete physical lanes.
- **WASM Realization:** Implemented as `TypedArrays` (`Uint32Array`), utilizing SIMD when possible.
- **Verilog Realization:** Implemented as **Unpacked Arrays** (`logic pixels [0:1023]`), drawing parallel wires in silicon.

### 2.2. Vector Lifting & SIMD
Operations on vectors are "lifted" to occur in parallel.
- `&pixels = pixels + 1;`
- In software, this uses optimized engine primitives like `.fill()` or SIMD instructions to avoid sequential `for` loops.
- In hardware, this generates `generate` blocks that instantiate parallel ALUs (Spatial Unrolling).

### 2.3. Bit-Precision Shorthand
Types can be decorated with bit-precision constraints using the `@/` operator.
- `Int@/x16`: A 16-bit integer.
- `UInt@/0..7`: An 8-bit unsigned integer.
- This allows for granular control over the physical bus width and storage efficiency across all backends.

### 2.4. Physical Mapping
Hardware addresses are mapped using the `@` operator, which can now appear after the type declaration.
- `let reg: UInt @ 0x40000000;`
- Slashes provide bit-masking or element-width shorthands: `@ 0x40001000 / x16`.

## 3. Implementation Details

### 3.1. Assignment AST
The `Statement::Assignment` has been unified to use a Left-Hand Side (LHS) expression:
```rust
Assignment {
    lhs: Expr, // Supports Identifier, OwnedRef, ListIndex, Slice
    expr: Expr,
    timeout: Option<(Expr, TimeUnit)>,
}
```

### 3.2. Geometry Matching
The Typechecker enforces **Geometry Alignment**. 
- `Vector[N]` can only be assigned to another `Vector[N]`.
- Slices like `vec[0..127]` result in a `Vector[128]`, enabling zero-cost wiring/views into larger buffers.

### 3.3. Target-Specific Timing
- **Embedded:** `within 5ms` maps to hardware watchdog timers and cycle-accurate counters.
- **Software:** `within 5ms` maps to `Promise.race` and event-loop timeouts.
