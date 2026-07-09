# Defining Custom Types

Brief lets you define types that behave like built-in `Int`, `Float`, and
`String`. The same machinery (TypeUniverse) that handles primitives also
handles your custom types — no special compiler support needed.

## 1. `type MyType <: Base { ... }`

Use the `type` keyword to define a new type:

```brief
type MyInt <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    llvm <~ "i64";
    storage <~ "Boxed";
    op Add(MyInt) -> MyInt = "add nsw";
    op Sub(MyInt) -> MyInt = "sub nsw";
    op Mul(MyInt) -> MyInt = "mul nsw";
};
```

- `<: Bits` — the type is a lens over raw bits
- `bytes <~ N` — size in bytes (required)
- `alignment <~ N` — alignment in bytes (required)
- `llvm <~ "type"` — LLVM type for storage/memory operations
- `storage <~ "Boxed"` or `"Native"` — boxed (i64) or native (float/double)
- `op ...` — operator declarations mapping Brief ops to LLVM opcodes

## 2. Built-in Type Examples

The bootstrap file (`lib/std/types/bootstrap.bv`) defines all primitives:

### Integer Types

```brief
type Int <: Bits {
    bytes <~ 8;          // 64 bits
    alignment <~ 8;
    llvm <~ "i64";
    storage <~ "Boxed";  // boxed to i64 in state
    default_width <~ 64;
    commuting <~ true;
    op Add(Int) -> Int = "add nsw";
    op Sub(Int) -> Int = "sub nsw";
    op Mul(Int) -> Int = "mul nsw";
    op Div(Int) -> Int = "sdiv";
    op Eq(Int) -> Bool = "icmp eq";
    op Ne(Int) -> Bool = "icmp ne";
    op Neg() -> Int = "neg";
};
```

### Float Types

```brief
type Float <: Bits {
    bytes <~ 4;          // 32 bits
    alignment <~ 4;
    llvm <~ "float";
    storage <~ "Native";  // native LLVM float ops
    default_width <~ 32;
    commuting <~ true;
    op Add(Float) -> Float = "fadd fast";
    op Sub(Float) -> Float = "fsub fast";
    op Mul(Float) -> Float = "fmul fast";
    op Neg() -> Float = "fneg";
    op Eq(Float) -> Bool = "fcmp oeq";
};
```

### Bool

```brief
type Bool <: Bits {
    bytes <~ 1;
    alignment <~ 1;
    llvm <~ "i8";
    storage <~ "Boxed";
    box <~ "zext.i1.to.i64#";
    unbox <~ "trunc.i64.to.i1#";
    default_width <~ 1;
    op Eq(Bool) -> Bool = "icmp eq";
    op And(Bool) -> Bool = "and";
    op Or(Bool) -> Bool = "or";
    op Not() -> Bool = "not";
};
```

## 3. Operator Declarations

Each `op` declaration maps a Brief operation to an LLVM instruction string:

```brief
op Add(ParamType) -> ReturnType = "llvm opcode";
```

| Brief Op | Param | Return | Typical LLVM |
|----------|-------|--------|-------------|
| `a + b` | `Add(T)` | T | `"add nsw"`, `"fadd fast"` |
| `a - b` | `Sub(T)` | T | `"sub nsw"`, `"fsub fast"` |
| `a * b` | `Mul(T)` | T | `"mul nsw"`, `"fmul fast"` |
| `a / b` | `Div(T)` | T | `"sdiv"`, `"fdiv fast"` |
| `a % b` | `Mod(T)` | T | `"srem"`, `"frem fast"` |
| `-a` | `Neg()` | T | `"neg"`, `"fneg"` |
| `a == b` | `Eq(T)` | Bool | `"icmp eq"`, `"fcmp oeq"` |
| `a < b` | `Lt(T)` | Bool | `"icmp slt"`, `"fcmp olt"` |
| `!a` | `Not()` | Bool | `"not"` |
| `a && b` | `And(T)` | Bool | `"and"` |

The operator name (Add, Sub, Eq, etc.) is PascalCase in the declaration and
maps to the corresponding `OpRune` in the compiler.

## 4. Annotations

### `default_width <~ N`

Sets the default parameter width. When a type is used without explicit
parameters (`let x: Int = 0`), the `NormalizeTypes` pass resolves it to
`Int<64>`.

```brief
type Int <: Bits {
    default_width <~ 64;
};
```

### `commuting <~ true|false`

Tells the optimizer that operations on this type commute (`a + b = b + a`).
Defaults to `true`. Set to `false` for non-commutative types (e.g., matrices).

### `constant_time <~ true|false`

Tells the optimizer to preserve side-channel resistance by avoiding
data-dependent branching or memory access. Defaults to `false`.

### `box <~ "intrinsic#"` / `unbox <~ "intrinsic#"`

Box/unbox intrinsics for state marshalling. Boxed types are stored as `i64`
in state. The box intrinsic widens the native value to `i64`; unbox narrows
it back:

```brief
type Int8 <: Bits {
    box <~ "sext.i8.to.i64#";
    unbox <~ "trunc.i64.to.i8#";
};
type Bool <: Bits {
    box <~ "zext.i1.to.i64#";
    unbox <~ "trunc.i64.to.i1#";
};
```

## 5. Creating a Custom Numeric Type

Here's a complete example of a 24-bit unsigned integer:

```brief
type UInt24 <: Bits {
    bytes <~ 3;
    alignment <~ 4;
    llvm <~ "i32";
    storage <~ "Boxed";
    box <~ "zext.i24.to.i64#";
    unbox <~ "trunc.i64.to.i24#";
    default_width <~ 24;
    commuting <~ true;
    op Add(UInt24) -> UInt24 = "add";
    op Sub(UInt24) -> UInt24 = "sub";
    op Mul(UInt24) -> UInt24 = "mul";
    op Eq(UInt24) -> Bool = "icmp eq";
    op Ne(UInt24) -> Bool = "icmp ne";
};
```

Now `UInt24` can be used anywhere a built-in type can:

```brief
let x: UInt24 = 42;
let y: UInt24 = x + 1;      // Uses op Add → "add"
```

## 6. Custom Types with Struct Layout

Types can also have a struct layout. The compiler uses this for field
projection codegen. Built-in `String` is an example:

```brief
// String is a struct: { ptr: Ptr<Bits<8>>, len: Bits(64), codec: Bits(8) }
// Stored as 24 bytes with struct_layout metadata in the TypeUniverse.
```

User-defined struct types are defined with the `type` keyword:

```brief
type Vec3 <: Bits {
    bytes <~ 12;
    alignment <~ 4;
    llvm <~ "{ float, float, float }";
    storage <~ "Native";
    op Add(Vec3) -> Vec3 = "fadd fast";
};
```

## 7. Summary

| Feature | Syntax | Purpose |
|---------|--------|---------|
| Type declaration | `type Name <: Bits { ... }` | Define a new type |
| Size | `bytes <~ N` | Byte size (required) |
| Alignment | `alignment <~ N` | Alignment (required) |
| LLVM type | `llvm <~ "t"` | LLVM storage type |
| Storage | `storage <~ "Boxed"` / `"Native"` | How values are stored in state |
| Operator | `op Add(T) -> T = "add nsw"` | Map Brief op to LLVM opcode |
| Default param | `default_width <~ N` | Default type parameter |
| Commuting | `commuting <~ true/false` | Optimization hint |
| Constant-time | `constant_time <~ true/false` | Side-channel hint |

The `type` syntax allows users to extend Brief with custom numeric types
that compile to the same LLVM IR as built-in primitives.
