# Type Mapping: Briev → LLVM

## Primitive Types

| Briev Type | LLVM Type | Alignment | Notes |
|------------|-----------|-----------|-------|
| `Int` | `i64` | 8 | Signed |
| `UInt` | `i64` | 8 | Unsigned |
| `Float` | `float` | 4 | IEEE 754 |
| `Bool` | `i1` (stored as `i8` in memory) | 1 | Zero/non-zero |
| `Char` | `i32` | 4 | Unicode scalar |
| `String` | `{ i8*, i64 }` | 8 | Pointer + length |
| `Data` | `{ i8*, i64 }` | 8 | Pointer + length |
| `Void` | `void` | - | No value |

## Struct / Rstruct

```briev
rstruct Counter {
    count: Int;
    active: Bool;
}
```

```llvm
%struct.Counter = type { i64, i8 }
```

Alignment: first field at offset 0, subsequent fields aligned to their natural alignment. Tail padding added to match the struct's max alignment.

## Enum (Tagged Union)

```briev
enum Option<Int> { Some(Int), None }
```

```llvm
%struct.Option_Int = type { i64, %variant_data }
; discriminant at offset 0, variant payload at offset 8
```

**Layout:**
- `i64` discriminant (0 = `None`, 1 = `Some`)
- Variant payload as a union of all variant structs, sized to the largest variant

## Tuple

```briev
(Int, Bool)
```

```llvm
%struct.Tuple_2 = type { i64, i8 }
```

Tuples are anonymous structs with numbered fields.

## List (Dynamic Array)

```briev
[Int]
```

```llvm
%struct.List_I64 = type { i64*, i64, i64 }
; pointer, length, capacity
```

Lists are heap-allocated. The LLVM backend emits calls to `malloc`/`free` or uses the reactor's bump allocator.

## SIMD Vector

```briev
let data: Float[64];
```

```llvm
; For vectorized operations — not a single LLVM type, but operations use
; <4 x float> or <8 x float> vector types with explicit alignment
%data = alloca [64 x float], align 32
```

**SIMD alignment rules:**
- N ≤ 4: `align 16` (SSE)
- 4 < N ≤ 8: `align 32` (AVX2)  
- 8 < N ≤ 16: `align 64` (AVX-512)