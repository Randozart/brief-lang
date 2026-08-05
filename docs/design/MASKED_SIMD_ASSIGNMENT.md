# Masked SIMD Assignment Design

**Status**: Proposed (Not Implemented)  
**Created**: 2026-05-05  
**Updated**: 2026-05-06  
**Purpose**: Extend Briv's SIMD capabilities with conditional masked assignments

---

## Motivation

Briv natively supports vector arrays with implicit SIMD lifting:
```briv
&pixels = pixels * 2;  // 100 parallel multiplications
```

However, there's no way to apply SIMD assignments **selectively** based on a condition. Currently, you must either:
- Assign to entire vectors
- Use slicing (`vec[start..end]`) for contiguous ranges

**Use case**: "Set all vector elements > 5 to 0" without explicit loops.

---

## Syntax Overview

### Core Operators

| Operator | Meaning |
|----------|---------|
| `start..` | "start from" - from index to end |
| `..end` | "end on" - from 0 to index (inclusive) |
| `start..end` | from start to end (inclusive) |
| `::stride` | stride (every Nth element) |
| `;condition` | mask - only elements matching condition |

**Key**: `;` cleanly separates mask from stride. Order of stride and mask matters - they produce different results.

### Grammar

```
slice     → "[" range? (":" stride)? (";" condition)? "]"
range     → expr ".." expr?
          | ".." expr
stride    → "::" expr?
condition → expr
```

---

## Syntax Details

### Simple Mask (All Elements)

```briv
// 1D masked assignment
&vec[;> 5] = 0;              // Elements > 5 become 0, others unchanged
&vec[;== 0] = 1;             // Elements == 0 become 1

// 2D: mask applies to all elements (dimension-agnostic)
&mat[;> 5] = 0;              // All elements > 5 become 0
&cube[;> 10] = 255;          // All elements > 10 become 255
```

### Range Syntax

```briv
&vec[10..;> 5] = 0;          // From index 10 to end, where value > 5
&vec[..20;> 5] = 0;          // From 0 to index 20 (inclusive), where value > 5
&vec[10..20;> 5] = 0;        // From 10 to 20 (inclusive), where value > 5
```

### Stride + Mask Combinations

**Order matters** - stride then mask vs mask then stride produce different results:

```briv
&vec[::2;> 5] = 0;           // Every 2nd element, if > 5 → set to 0
&vec[;> 5:2] = 0;           // Elements > 5, then every 2nd of those

&vec[10..20:2;> 5] = 0;     // Range 10-20, stride 2, then mask > 5
&vec[10..20;> 5:2] = 0;     // Range 10-20, mask > 5, then stride 2
```

**Example showing the difference:**

```briv
let vec: Int[10] = [1, 6, 3, 8, 2, 9, 4, 7, 0, 2];

// Stride first, then mask
&vec[::2;> 3] = 0;
// ::2 → indices [0,2,4,6,8] → values [1,3,2,4,0]
// ;> 3 → indices 4,6 qualify (values 2,4 don't)
// Result: [1, 6, 3, 8, 0, 9, 0, 7, 0, 2]

// Mask first, then stride
&vec[;> 3:2] = 0;
// ;> 3 → indices [1,3,5,7] → values [6,8,9,7]
// :2 → indices [1,5] of the filtered set (every 2nd)
// Result: [1, 6, 3, 0, 2, 9, 4, 7, 0, 2]
```

### Complex Multi-Dimensional (With Comma Syntax)

When you need per-dimension control, use explicit dimension separators:

```briv
let mat: Int[10][20];

// Per-row mask: for each row, mask columns
&mat[:, ::2;> 5] = 0;        // All rows, every 2nd column, if > 5

// Range on specific dimension
&mat[5.., ;> 3] = 0;         // Rows from 5 onwards, all columns > 3
```

For simple value-based masks, comma syntax is unnecessary - `&mat[;> 5]` applies to all elements.

---

## Conditional Assignment Between Vectors

When assigning between equal-sized vectors, conditions can filter which positions are assigned:

```briv
// Assign source to dest, but only where condition matches
let dest: Int[5] = [1, 6, 3, 8, 2];
let source: Int[5] = [10, 20, 30, 40, 50];

&dest[;> 4] = source;
// dest[1]=6>4 → source[1]=20 assigned
// dest[3]=8>4 → source[3]=40 assigned
// Result: [1, 20, 3, 40, 2]

// Filter source before assigning
&dest = source[;> 20];
// source[2]=30>20, source[3]=40>20
// Assigns sequentially to dest[0], dest[1]
// Result: [30, 40, 3, 8, 2]
```

**Rule**: Both vectors must be equal size after slicing/striding. The condition acts as a filter during assignment - matching positions get the value, non-matching positions are skipped.

---

## Semantics

### Masked Assignment

**Behavior**: Only elements matching the condition are updated. Others remain unchanged.

```briv
let vec: Int[5] = [1, 6, 3, 8, 2];
&vec[;> 5] = 0;
// Result: [1, 0, 3, 0, 2]  (only elements > 5 become 0)
```

**Geometry**: Vector size **does not change**. This is a masked write, not a filter.

### Value-Based vs Position-Based

The mask operates on **values**, not positions. For any position `[i]`, the condition checks `vec[i]`:

```briv
let vec: Int[5] = [1, 6, 3, 8, 2];
&vec[;> 5] = 0;
// At index 0: value=1 → 1>5 is false → unchanged
// At index 1: value=6 → 6>5 is true → set to 0
// At index 2: value=3 → 3>5 is false → unchanged
// At index 3: value=8 → 8>5 is true → set to 0
// At index 4: value=2 → 2>5 is false → unchanged
```

---

## Implementation Plan

### 1. AST Changes (`src/ast.rs`)

Modify `Expr::Slice` to add optional mask field:

```rust
Slice {
    value: Box<Expr>,
    start: Option<Box<Expr>>,       // For range: 10.. or 10..20
    end: Option<Box<Expr>>,         // For range: ..20 or 10..20
    stride: Option<Box<Expr>>,      // ::stride
    mask: Option<Box<Expr>>,        // NEW: ;condition
}
```

### 2. Parser Changes (`src/parser.rs`)

Parse slice components in order:
1. Range: `expr ".." expr?` or `".." expr`
2. Stride: `"::" expr?`
3. Mask: `";" expr`

Support flexible ordering of stride and mask:

```rust
// vec[10..20:2;> 5] → range + stride + mask
// vec[10..20;> 5:2] → range + mask + stride

enum SliceComponent {
    Range(Option<Expr>, Option<Expr>),  // (start, end)
    Stride(Expr),
    Mask(Expr),
}
```

### 3. Typechecker Changes (`src/typechecker.rs`)

- Mask expression must evaluate to `Bool` type
- Masked assignment keeps same vector geometry (no size change)
- Compound conditions (`> 5 && < 10`) via `Expr::BinaryOp`
- For conditional vector assignment: verify both sides equal size after slicing/striding

### 4. Code Generation

#### Verilog (`src/backend/verilog.rs`)

```verilog
// For &vec[;> 5] = 0
integer i;
for (i = 0; i < 100; i = i + 1) begin
    if (vec[i] > 5) begin
        vec[i] = 0;
    end
end
```

#### Rust (`src/backend/rust.rs`)

```rust
// For &vec[;> 5] = 0
for i in 0..100 {
    if vec[i] > 5 {
        vec[i] = 0;
    }
}
```

#### VHDL (`src/backend/vhdl.rs`)

```vhdl
-- For &vec[;> 5] = 0
process
begin
    for i in 0 to 99 loop
        if vec(i) > 5 then
            vec(i) <= 0;
        end if;
    end loop;
end process;
```

### 5. Multidimensional Handling

For `&mat[;> 5] = 0` on `mat: Int[10][20]`:

```verilog
// Value-based mask applies to all elements
integer i, j;
for (i = 0; i < 10; i = i + 1) begin
    for (j = 0; j < 20; j = j + 1) begin
        if (mat[i][j] > 5) begin
            mat[i][j] = 0;
        end
    end
end
```

---

## Examples

### Example 1: Simple 1D Mask
```briv
let sensor_readings: Int[100] = ...;

// Zero out all readings above 100 (error readings)
&sensor_readings[;> 100] = 0;
```

### Example 2: 2D Value-Based Mask
```briv
let frame: UInt[3][1920][1080];  // 1080p RGB

// Set all R and B pixels > 200 to max (over-exposure protection)
&frame[0;> 200] = 255;
&frame[2;> 200] = 255;
```

### Example 3: Stride + Mask
```briv
let samples: Float[1000];

// Every 10th sample, if > threshold, reset to 0
&samples[::10;> 50.0] = 0.0;
```

### Example 4: Range + Stride + Mask
```briv
let buffer: Int[100];

// Process middle section: indices 20-60, every 4th, where value > 10
&buffer[20..60:4;> 10] = 0;
```

### Example 5: Conditional Vector Assignment
```briv
let dest: Int[8] = [1, 5, 3, 8, 2, 7, 4, 9];
let source: Int[8] = [10, 20, 30, 40, 50, 60, 70, 80];

// Assign source to dest where dest > 5
&dest[;> 5] = source;
// dest[1]=5>5 false, dest[3]=8>5 true→40, dest[5]=7>5 true→60, dest[7]=9>5 true→80
// Result: [1, 5, 3, 40, 2, 60, 4, 80]
```

---

## Open Questions

1. **Compound conditions**: Should `&vec[;> 5 && < 10] = 0;` work?
   - **Answer**: Yes, parse as `Expr::BinaryOp(>, <)` with short-circuit

2. **Source-side mask with different sizes**:
   ```briv
   let dest: Int[64];
   let source: Int[32];
   &dest[..31] = source[;> 2];
   ```
   - **Answer**: Assign sequentially to dest[0..31] from matching source elements

3. **Mask on index**: Should `&vec[;> i]` with runtime `i` be allowed?
   - **Answer**: Yes, masks can reference other variables

4. **Order of stride+mask**: Is supporting both orders (`::2;> 5` and `;> 5:2`) correct?
   - **Answer**: Yes, they produce different results intentionally

---

## References

- **Current slice syntax**: `docs/EMBEDDED_BRIV_2.2_SPEC.md:62-68`
- **Vector type definition**: `src/ast.rs:119`
- **Slice parsing**: `src/parser.rs:2726-2770`
- **SIMD lifting**: `docs/EMBEDDED_BRIV_2.2_SPEC.md:54-57`
- **Verilog vector codegen**: `src/backend/verilog.rs:977-1292`

---

**Next Steps**: 
1. Implement AST changes
2. Extend parser for new slice syntax (range, stride, mask in any order)
3. Add type checking for mask expressions and conditional assignment
4. Update code generation backends
5. Add test cases