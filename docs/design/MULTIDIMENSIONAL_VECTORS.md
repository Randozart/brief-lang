# Multidimensional Vectors and Named Dimensions

**Status**: Implemented (Core)  
**Created**: 2026-05-08  
**Updated**: 2026-05-08  
**Purpose**: Add multidimensional vector syntax, named dimensions, and struct property filtering to Briev

---

## Implementation Status

### ✅ Completed
- Multidimensional vector declaration: `Vector<T, dim1, dim2, ...>`
- Named dimensions: `Vector<T, width:50, height:50>`
- AST: `SliceCoordinate` enum (Index, Range, Named)
- AST: `MultiSlice` expression variant
- Parser: multidimensional slice parsing with commas
- Parser: named dimension slicing (`time:5`)
- Parser: range slicing (`0..10`, `5..`, `..10`)
- Parser: striding (`::2`)
- Parser: vectorized filtering (`; condition`)
- Typechecker: Vector SIMD operations
- Typechecker: List SIMD type inference
- All backends updated (Rust, C, Cobol, VHDL, Verilog, WASM)
- Spec updated with new grammar
- Learn-briev documentation updated

### 🔄 In Progress
- List SIMD code generation backends (strip-mining for AArch64, AXI-Stream for FPGA)
- Proof engine length assertions for List operations

---

## Implementation Status

### ✅ Completed
- Multidimensional vector declaration: `Vector<T, dim1, dim2, ...>`
- Named dimensions: `Vector<T, width:50, height:50>`
- AST: `SliceCoordinate` enum (Index, Range, Named)
- AST: `MultiSlice` expression variant
- Parser: multidimensional slice parsing with commas
- Parser: named dimension slicing (`time:5`)
- Parser: range slicing (`0..10`, `5..`, `..10`)
- Parser: striding (`::2`)
- Parser: vectorized filtering (`; condition`)
- Typechecker: Vector SIMD operations
- Typechecker: List SIMD type inference
- All backends updated (Rust, C, Cobol, VHDL, Verilog, WASM)
- Spec updated with new grammar
- Learn-briev documentation updated

### 🔄 In Progress
- List SIMD code generation backends (strip-mining for AArch64, AXI-Stream for FPGA)
- Proof engine length assertions for List operations

---

## Motivation

Briev currently supports 1D vectors with `Type[N]` syntax (e.g., `Int[10]`). This is insufficient for:

1. **Hardware design** - FPGAs and SIMD units work with multi-dimensional data (matrices, tensors, image buffers)
2. **Spatial thinking** - Vectors should encourage thinking about data layout, not sequential access
3. **Auditability** - Named dimensions make code self-documenting
4. **Struct property filtering** - Filtering vector elements by struct field values enables database-like operations

---

## 1. Multidimensional Vector Declaration

### Syntax

```briev
Vector<Int, 50, 10, 20, 50>              // 4D: anonymous dimensions
Vector<Person, width:50, height:50, depth:40, time:10>  // 4D: named dimensions
```

### Rules

- First position is always the **element type**
- Remaining positions are **dimensions** (positive integers)
- Dimensions can be **anonymous** (just numbers) or **named** (`name:size`)
- Named and anonymous dimensions can be mixed (not recommended)
- Total elements = product of all dimensions

### Examples

```briev
// 1D (same as current)
let vec: Vector<Int, 100>;

// 2D matrix
let mat: Vector<Int, 10, 20>;           // 10 rows, 20 columns

// 3D tensor
let tensor: Vector<Float, 3, 32, 32>;   // 3 channels, 32x32

// 4D with named dimensions
let persons: Vector<Person, width:50, height:50, depth:40, time:10>;

// 4D anonymous
let data: Vector<Int, 50, 10, 20, 50>;
```

### Memory Layout

All vectors are stored as **flat buffers** with stride calculations. The compiler computes:

```
offset = d1 * stride1 + d2 * stride2 + d3 * stride3 + ...
stride_n = product(dimensions after n)
```

This enables:
- Zero-cost slicing (just adjust base pointer + bounds)
- Hardware synthesis knows exact size at compile time
- SIMD operations can target any dimension

---

## 2. Named Dimensions

### Declaration

```briev
Vector<Person, width:50, height:50, depth:40, time:10>
```

### Benefits

- **Self-documenting**: `persons[time:5]` is clearer than `persons[5, :, :, :]`
- **Error prevention**: Compiler catches `persons[width:100]` when width is 50
- **Backend optimization**: Hardware generators can use names for signal naming

### No Memory Impact

Named dimensions are purely syntactic sugar. The memory layout is identical to anonymous dimensions. Names are stripped during compilation.

---

## 3. Coordinate Slicing

### Named Dimension Slicing

```briev
persons[time:5]                    // slice at time=5 (returns 3D vector)
persons[time:5, width:10]          // slice at time=5, width=10 (returns 2D)
persons[time:5, width:10, height:20]  // returns 1D vector
```

### Anonymous Coordinate Slicing

```briev
data[5, 10, 20, 30]                // single point (returns element)
data[5, :, :, :]                   // slice at first dimension
data[0..10, :, :, :]               // range slice
```

### Range Syntax

```briev
data[0..10]                        // indices 0 to 10 (inclusive)
data[10..]                         // from 10 to end
data[..20]                         // from 0 to 20 (inclusive)
data[0..100:5]                     // range with stride (every 5th)
```

---

## 4. Striding

### Syntax

```briev
data[::2]                          // every 2nd element
data[time:0..10:2]                 // every 2nd from 0 to 10 in time dimension
data[width:0..50:5]                // every 5th in width dimension
```

### Combined with Slicing

```briev
persons[time:0..10:2, width:5]     // every 2nd time step, at width=5
```

---

## 5. Vectorized Filtering (Struct Property Filtering)

### Syntax

The semicolon `;` separates **coordinates** from **filter conditions**:

```briev
persons[: age > 18]                // filter: all persons where age > 18
persons[time:5; age > 18]          // slice + filter: at time=5, where age > 18
persons[0..10, :, :; city == "NYC"] // anonymous range + filter
```

### What This Enables

```briev
// Set adult flag for all persons over 18
persons[: age > 18].adult = true;

// Set region for all NYC residents
persons[: city == "NYC"].region = "East";

// Complex filter with slice
persons[time:0..10:2; age > 18 && city == "NYC"].processed = true;
```

### Semantics

- Filter operates on **values**, not positions
- Returns a **masked view** into the original vector
- Assignment only affects matching elements
- Non-matching elements remain unchanged

### Compiler Verification

The compiler must prove:
1. Filter condition references valid struct fields
2. Filter condition evaluates to `Bool`
3. No overlapping writes (deterministic)
4. Filter terminates (no infinite loops in condition)

---

## 6. Combined Operations

### Full Syntax

```briev
vector[coordinates; condition]
```

Where `coordinates` can be:
- Named: `time:5, width:10`
- Anonymous: `5, 10, 20`
- Ranges: `0..10, :, 5..`
- Strided: `0..100:5, ::2`

And `condition` is:
- Simple: `age > 18`
- Compound: `age > 18 && city == "NYC"`
- Field access: `person.age > 18` (when iterating over structs)

### Examples

```briev
// Every 2nd time step from 0 to 10, where age > 18, set adult to true
persons[time:0..10:2; age > 18].adult = true;

// All rows, every 2nd column, where value > 5
mat[:, ::2; > 5] = 0;

// Rows 5 onwards, all columns where value > 3
mat[5.., ; > 3] = 0;
```

---

## 7. AST Changes

### Type Representation

Current:
```rust
Vector(Box<Type>, usize),  // (inner type, size)
```

New:
```rust
Vector {
    inner: Box<Type>,
    dimensions: Vec<Dimension>,
}

enum Dimension {
    Anonymous(usize),
    Named(String, usize),
}
```

### Slice Expression

Current:
```rust
Slice {
    value: Box<Expr>,
    start: Option<Box<Expr>>,
    end: Option<Box<Expr>>,
}
```

New:
```rust
Slice {
    value: Box<Expr>,
    coordinates: Vec<Coordinate>,
    filter: Option<Box<Expr>>,
}

enum Coordinate {
    Index(Expr),
    Range { start: Option<Expr>, end: Option<Expr> },
    NamedSlice { name: String, coord: Coordinate },
}
```

---

## 8. Parser Changes

### Vector Type Parsing

```
vector_type → "Vector" "<" type "," dimension ("," dimension)* ">"
dimension   → identifier ":" integer  // named
            | integer                 // anonymous
```

### Slice Parsing

```
slice       → "[" coordinates? (";" condition)? "]"
coordinates → coordinate ("," coordinate)*
coordinate  → identifier ":" coord_value  // named
            | coord_value                 // anonymous
coord_value → integer                     // index
            | integer ".." integer?       // range
            | ".." integer                // range from start
            | "::" integer                // stride
            | ":"                         // all
```

---

## 9. Typechecker Changes

1. **Dimension validation**: All dimension sizes must be positive integers
2. **Named dimension resolution**: Names must match declared dimensions
3. **Coordinate type checking**: Each coordinate must match its dimension's type
4. **Filter type checking**: Filter expression must evaluate to `Bool`
5. **Return type inference**: Slicing reduces dimensionality appropriately
6. **Geometry preservation**: Filtered assignment maintains vector size

---

## 10. Backend Changes

### Rust Backend

```rust
// Vector<Int, 10, 20> → [[Int; 20]; 10]
// Named dimensions stripped, only sizes matter
```

### C Backend

```c
// Vector<Int, 10, 20> → int data[10][20];
```

### Verilog Backend

```verilog
// Vector<Int, 10, 20> → logic [31:0] data [0:9][0:19];
// Named dimensions become comments for readability
```

### VHDL Backend

```vhdl
-- Vector<Int, 10, 20> → type data_type is array (0 to 9, 0 to 19) of integer;
```

---

## 11. Examples

### Example 1: Image Processing

```briev
struct Pixel {
    r: UInt,
    g: UInt,
    b: UInt
}

let frame: Vector<Pixel, width:1920, height:1080>;

// Brighten all red pixels > 200
frame[: r > 200].r = 255;

// Process every 4th pixel in both dimensions
frame[width::4, height::4].r = frame[width::4, height::4].r / 2;
```

### Example 2: Time Series Database

```briev
struct SensorReading {
    temperature: Float,
    humidity: Float,
    alert: Bool
}

let readings: Vector<SensorReading, sensor:100, time:1000>;

// Mark all readings where temperature > 100
readings[: temperature > 100.0].alert = true;

// Get all readings from sensor 5 at time 0..500
let subset: Vector<SensorReading, time:500> = readings[sensor:5, time:0..500];
```

### Example 3: Neural Network Weights

```briev
let weights: Vector<Float, layer:4, input:128, output:64>;

// Initialize all weights to small random values
weights[: ] = random_float(0.0, 0.01);

// Zero out all weights in layer 2
weights[layer:2, :, :] = 0.0;
```

---

## 12. Open Questions

1. **Should `Vector<T>` (no dimensions) be allowed?**
   - **Answer**: No, dimensions are required. Use `List<T>` for dynamic sizing.

2. **Can dimensions be runtime values?**
   - **Answer**: No, dimensions must be compile-time constants for hardware synthesis.

3. **Should named dimensions support expressions?**
   - **Answer**: No, names are identifiers only. Values must be integer literals.

4. **What happens with out-of-bounds access?**
   - **Answer**: Compile-time error if bounds are known, runtime panic otherwise.

5. **Can filters reference external variables?**
   - **Answer**: Yes, `persons[: age > min_age]` where `min_age` is a variable.

---

## 13. Relationship to Existing SIMD Design

This design builds on the existing **Masked SIMD Assignment** design (`docs/design/MASKED_SIMD_ASSIGNMENT.md`):

- The `;condition` mask syntax is identical
- Stride syntax (`::N`) is identical
- Range syntax (`start..end`) is identical
- This design adds: multidimensional declaration, named dimensions, struct property filtering

---

## Implementation Plan

1. **AST changes** - Add `Vector` struct with dimensions, update `Slice` expression
2. **Lexer changes** - Add tokens for `;` in slice context (if needed)
3. **Parser changes** - Parse multidimensional types, named dimensions, filter syntax
4. **Typechecker changes** - Validate dimensions, named resolution, filter types
5. **Backend changes** - Update all backends for multidimensional code generation
6. **Proof engine** - Verify filter termination, no overlapping writes
7. **Tests** - Comprehensive test suite for all new features

---

**Next Steps**: Begin with AST changes, then work through the compiler pipeline.
