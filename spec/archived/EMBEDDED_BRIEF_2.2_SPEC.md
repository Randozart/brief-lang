# Embedded Brief 2.2 Specification

**Version:** 2.2  
**Date:** 2026-04-12  
**Status:** Definitive Design Specification  

---

## 1. Introduction
Embedded Brief 2.2 unifies the logic across **Core Brief (.bv)**, **Rendered Brief (.rbv)**, and **Embedded Brief (.ebv)**. It provides high-velocity hardware primitives for "Silicon Architecting," introducing concepts like vector geometry, cycle-accurate timing, and cross-variant logic sharing.

---

## 2. Type System

### 2.1 Scalar Primitives
| Type | Aliases | Description |
|------|---------|-------------|
| `Int` | `Signed`, `Sgn` | Signed integer |
| `UInt` | `Unsigned`, `USgn` | Unsigned integer |
| `Bool` | - | Boolean (single bit) |
| `Float` | - | Floating-point (default 32-bit) |

### 2.2 Bit Range Allocation (`@/N`)
In `.ebv` specifically, types can be constrained to specific bit-widths or addresses:
| Syntax | Meaning | Example |
|--------|---------|---------|
| `@/N` | Bit at position N | `Bool @/7` (Bit 7) |
| `@/M..N`| Bit range M to N | `UInt @/0..7` (8 bits, 0-255) |
| `@/xN` | Any N-bit slot | `Float @/x16` (16-bit half-precision float) |

### 2.3 Vector Types (Multi-Dimensional)
Vectors represent physical wide wires or contiguous memory buffers, enabling SIMD operations.
**Syntax:** `Type[Dim1][Dim2]...[DimN]`

```brief
let bus: UInt[100];                  // 100-lane unsigned bus
let frame: UInt[3][1920][1080];      // 1080p RGB frame buffer
```

### 2.4 Union Types (`|`) and Results
Unions represent a logical super-position.
**Syntax:** `TypeA | TypeB`

```brief
let response: String | Error; 
```

---

## 3. Geometric Operations & Slicing

### 3.1 Implicit Lifting
Operations on vectors are implicitly parallel. The compiler "lifts" the scalar operation to the entire vector geometry.
```brief
&pixels = pixels * 2; // 100 parallel multiplications (SIMD)
```

### 3.2 Slicing, Striding, and Selection
To modify specific "lanes," use slicing:

| Syntax | Description | Result Size |
|--------|-------------|-------------|
| `vec[n]` | Selection | Scalar |
| `vec[start..end]` | Continuous Slice | `end - start` |
| `vec[::stride]` | Strided Slice | `vec.len / stride` |
| `vec[start..end:stride]` | Full slice | `(end - start) / stride` |
| `vec[..]` | Entire dimension | `vec.len` |

```brief
// Modify only the RED channel (first dimension)
&frame[0] = frame[0] + 5; 

// Modify every 2nd pixel in the middle of the screen
&frame[..][500..600:2][..] = 255; 
```

### 3.3 Geometry Matching Rule
An operation `A op B` is valid if `dim(A) == dim(B)`. The compiler enforces geometric alignment.
```brief
let base: Int[100];
let modifier: Int[50];
&base[25..75] = base[25..75] * modifier; // Valid: 50 lanes vs 50 lanes
```

---

## 4. Temporal Logic & Timeouts

### 4.1 Universal `within` Syntax
The `within` keyword handles asynchronous timeouts for assignments.
**Syntax:** `let var: Type | Error = expression within <value><unit>;`

**Units:** 
*   **All Variants:** `ms`, `s`, `sec`, `seconds`, `min`, `minute`.
*   **Embedded (.ebv) Only:** `cycles`, `cyc`.

### 4.2 Mandatory Error Handling
If `within` is used, the receiver **must** be a `Union` type containing `Error` (e.g., `Result` or `String | Error`). The compiler enforces exhaustive checking of all variants. You cannot `term` until the error condition is guarded.

```brief
let response: String | Error = httpGet(url) within 5s;

[response Ok(data)] { 
    &stored_data = data; 
    term; 
};

[response Err(err)] { 
    &status = "Failed"; 
    term; 
};
// Compiler error if the Err path is omitted.
```

---

## 5. Embedded Hardware (.ebv) Specifics

### 5.1 Triggers (`trg`) vs Outputs (`let @ address`)
*   **`trg` (Read-Only):** Physical hardware inputs (sensors, buttons). Must have an `@ address`.
*   **`let` with `@ address` (Writable):** Physical hardware outputs (LEDs, motors).

```brief
trg button: Bool @ 0x40020010 / 0;  // Input
let led: Bool @ 0x40020000 / 0;     // Output
```

### 5.2 Override Safety (`!@`)
Required to map variables to reserved hardware memory regions defined in `memory.toml`.
```brief
let debug_reg: Int !@ 0xE000E100;
```

---

## 6. Cross-Variant Integration

*   **Shared Core (.bv):** Shared logic can be imported by `.ebv` or `.rbv` via `import "logic.bv"`.
*   **Context Locking:** If a shared logic file uses `cycles`, it becomes context-locked to `.ebv` targets. `ms` and `s` are universal.

---

*End of Specification*