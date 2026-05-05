# Masked SIMD Assignment Design

**Status**: Proposed (Not Implemented)  
**Created**: 2026-05-05  
**Purpose**: Extend Brief's SIMD capabilities with conditional masked assignments

---

## Motivation

Brief natively supports vector arrays with implicit SIMD lifting:
```brief
&pixels = pixels * 2;  // 100 parallel multiplications
```

However, there's no way to apply SIMD assignments **selectively** based on a condition. Currently, you must either:
- Assign to entire vectors
- Use slicing (`vec[start..end]`) for contiguous ranges

**Use case**: "Set all vector elements > 5 to 0" without explicit loops.

---

## Proposed Syntax: `:condition`

### Core Syntax

Use `:` followed by a boolean expression inside slice brackets:

```brief
// 1D masked assignment
&vec[:> 5] = 0;              // Elements > 5 become 0, others unchanged
&vec[:== 0] = 1;             // Elements == 0 become 1
&vec[:>= 10 && <= 20] = 5;   // Compound conditions (multiple conditions on same dimension)

// 2D: mask specific dimension
&mat[:, :> 5] = 0;           // For each row, set columns > 5 to 0
&mat[:> 3, :> 5] = 0;       // Filter rows > 3 AND columns > 5 (independent per-dim)

// 3D
&cube[:, :, :> 10] = 255;    // Mask on third dimension
```

### Combined Stride + Mask Syntax

Brief already supports strided slicing with `::stride`. We can combine both:

```brief
// Stride first, then mask
&vec[::2:> 5] = 0;         // Every 2nd element, if > 5 → set to 0
&vec[10..20::2:> 5] = 0;   // Range 10-20, stride 2, mask > 5

// Semantics: stride reduces the set, then mask filters further
```

**Grammar**:
```
slice ::= "[" expr ".." expr ":" expr ":" expr "]"   // range:stride:mask
        | "[" "::" expr ":" expr "]"                // ::stride:mask  
        | "[" "::" expr "]"                         // ::stride (existing)
        | "[" ":" expr "]"                          // :mask (new)
        | "[" expr ".." expr ":" expr "]"           // range:stride (existing)
        | "[" expr "]"                              // index (existing)
```

**Key**: `::stride:mask` uses double-colon then single-colon - distinguishable!

---

## Semantics

### Masked Assignment (`:condition`)

**Behavior**: Only elements matching the condition are updated. Others remain unchanged.

```brief
let vec: Int[5] = [1, 6, 3, 8, 2];
&vec[:> 5] = 0;
// Result: [1, 0, 3, 0, 2]  (only elements > 5 become 0)
```

**Geometry**: Vector size **does not change**. This is a masked write, not a filter.

### Combined Stride + Mask (`::stride:mask`)

**Order of operations**: Stride first, then mask.

```brief
let vec: Int[10] = [1, 6, 3, 8, 2, 9, 4, 7, 0, 2];
&vec[::2:> 5] = 0;
// Stride ::2 → indices [0,2,4,6,8] → values [1,3,2,4,0]
// Mask > 5 → none qualify → no change
// Result: [1, 6, 3, 8, 2, 9, 4, 7, 0, 2] (unchanged)
```

If we had `&vec[::2:> 3] = 0;`:
- Stride `::2` → indices `[0,2,4,6,8]` → values `[1,3,2,4,0]`
- Mask `> 3` → indices `6,8` qualify (values `4,0` don't)
- Result: indices 6,8 set to 0 → `[1,6,3,8,2,9,0,7,0,2]`

---

## Implementation Plan

### 1. AST Changes (`src/ast.rs:312-317`)

Modify `Expr::Slice` to add optional mask field:

```rust
Slice {
    value: Box<Expr>,
    start: Option<Box<Expr>>,
    end: Option<Box<Expr>>,
    stride: Option<Box<Expr>>,   // Existing stride
    mask: Option<Box<Expr>>,     // NEW: filter condition
}
```

### 2. Parser Changes (`src/parser.rs:2726-2770`)

In slice parsing, after detecting `:`, distinguish between:
- `::` → stride (existing behavior)
- `:` followed by expression → mask condition

```rust
// Pseudo-logic
if let Some(Ok(Token::Colon)) = self.current_token() {
    self.advance();
    if let Some(Ok(Token::Colon)) = self.current_token() {
        // ::stride (existing)
        self.advance();
        stride = Some(self.parse_expression()?);
        // Check for :mask after stride
        if let Some(Ok(Token::Colon)) = self.current_token() {
            self.advance();
            mask = Some(self.parse_expression()?);
        }
    } else {
        // :mask (new)
        mask = Some(self.parse_expression()?);
    }
}
```

### 3. Typechecker Changes (`src/typechecker.rs`)

- Mask expression must evaluate to `Bool` type
- Masked assignment keeps same vector geometry (no size change)
- Compound conditions (`> 5 && < 10`) should be supported via `Expr::BinaryOp`

### 4. Code Generation

#### Verilog (`src/backend/verilog.rs`)

Generate loops with conditional checks:

```verilog
// For &vec[:> 5] = 0
integer i;
for (i = 0; i < 100; i = i + 1) begin
    if (vec[i] > 5) begin
        vec[i] = 0;
    end
end
```

#### Rust (`src/backend/rust.rs`)

```rust
// For &vec[:> 5] = 0
for i in 0..100 {
    if vec[i] > 5 {
        vec[i] = 0;
    }
}
```

#### VHDL (`src/backend/vhdl.rs`)

```vhdl
-- For &vec[:> 5] = 0
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

For `&mat[:, :> 5] = 0;` on `mat: Int[10][20]`:

**Semantics**: The mask applies to the specified dimension. In this case, for each row (first dim `:`), apply mask to columns (second dim `:> 5`).

```verilog
// Verilog for &mat[:, :> 5] = 0
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

## Open Questions

1. **Compound conditions**: Should `&vec[:> 5 && < 10] = 0;` work? 
   - **Recommendation**: Yes, just parse as `Expr::BinaryOp(>, <)` with short-circuit

2. **Cross-dimension masks**: Should we support `&mat[:> 5, > 3]` where the mask depends on BOTH indices?
   - **Recommendation**: Not now - too complex. Per-dimension masks are sufficient.

3. **Mask on index**: Should `&vec[:> i]` with runtime `i` be allowed?
   - **Recommendation**: Yes, masks can reference other variables.

4. **Combined stride+mask priority**: Is stride-first correct?
   - **Recommendation**: Yes, stride reduces set first (more predictable).

---

## Examples

### Example 1: Simple 1D Mask
```brief
let sensor_readings: Int[100] = ...;

// Zero out all readings above 100 (error readings)
&sensor_readings[:> 100] = 0;
```

### Example 2: 2D with Per-Dimension Mask
```brief
let frame: UInt[3][1920][1080];  // 1080p RGB

// Set all R and B pixels > 200 to max (over-exposure protection)
&frame[0, :> 200] = 255;
&frame[2, :> 200] = 255;
```

### Example 3: Stride + Mask
```brief
let samples: Float[1000];

// Every 10th sample, if > threshold, reset to 0
&samples[::10:> 50.0] = 0.0;
```

### Example 4: Compound Mask Condition
```brief
let temperatures: Int[24];  // hourly readings

// Flag readings in comfortable range
&temperatures[:>= 18 && <= 24] = 1;
```

---

## References

- **Current slice syntax**: `docs/EMBEDDED_BRIEF_2.2_SPEC.md:62-68`
- **Vector type definition**: `src/ast.rs:119`
- **Slice parsing**: `src/parser.rs:2726-2770`
- **SIMD lifting**: `docs/EMBEDDED_BRIEF_2.2_SPEC.md:54-57`
- **Verilog vector codegen**: `src/backend/verilog.rs:977-1292`

---

**Next Steps**: 
1. Implement AST changes
2. Extend parser for `:condition` syntax
3. Add type checking for mask expressions
4. Update code generation backends
5. Add test cases
