# Memory by Contract — Fixed-Size Array Types
## 2026-07-25

## Overview

Brief gains fixed-size array syntax `Int[1024]` as a first-class type construct.
Arrays are embedded in `struct` types, proven safe by contract bounds, and
auto-vectorized by LLVM. No `Alloc#` or manual memory management needed.

The tamer's VM buffer structs become:

```brief
struct VMStack { data: Int[1024]; len: Int; };
struct VMLocals { data: Int[4096]; len: Int; };
struct VMFrame { locals_base: Int; local_count: Int; return_pc: Int; return_frame: Int; };
struct VMFrames { data: Frame[256]; count: Int; };
```

The compiler emits LLVM `[1024 x i64]` array fields directly in `%State`, and
index expressions become GEP+load/store — proven safe by `[stack.len < 1024]`.

## Changes

### 1. Parser: `Type[N]` syntax

**File: `src/parser/types.rs`**

In `parse_type()`, after the `.ext` suffix check, check for `[N]`:

```rust
// Array syntax: Int[1024], MyType[16]
if self.eat(&Token::LBracket) {
    let size = match self.peek() {
        Some(&Token::Integer(n)) => { self.pos += 1; Some(*n as usize) }
        _ => None,
    };
    self.expect(Token::RBracket)?;
    if let Some(s) = size {
        return Ok(Type::Vector(Box::new(base.1), vec![Dimension::Anonymous(s)]));
    }
}
```

In `parse_named_type_body()`, add the same check after the `.ext` suffix check
for custom types like `MyArray[10]`.

### 2. Type system: `Type::Vector`

Already exists in `src/ast/types.rs:38`:

```rust
Vector(Box<Type>, Vec<Dimension>),
Dimension::Anonymous(usize),
```

The type checker already handles this variant. No changes needed.

### 3. Normalizer: Field emission

**File: `src/backend/llvm/mod.rs`, `push_field_type()`**

Add an arm for `Type::Vector(inner, dims)`:

```rust
// Array type: Int[1024] → [1024 x i64]
if let Type::Vector(inner, dims) = ty {
    if dims.len() == 1 {
        if let Dimension::Anonymous(n) = dims[0] {
            let inner_llvm = self.llvm_type(inner);
            self.ctx.field_types.push(format!("[{} x {}]", n, inner_llvm));
            self.ctx.field_brief_types.push(ty.clone());
            return;
        }
    }
}
```

### 4. Index expression: `a[i]` for arrays

**File: `src/backend/llvm/emit_expr.rs`**

Currently `a[i]` for Ptr<T> emits `load`/`store` with ptrtoint/inttoptr.
For `Type::Vector` arrays, emit a GEP into the `[N x T]` field:

```llvm
%gep = getelementptr [1024 x i64], ptr %state, i32 0, i32 field_idx, i64 %index
%val = load i64, ptr %gep
```

### 5. Tamer: VM in pure Brief

**New files: `lib/tamer/*.bv`**

The VM interpreter, .bounty parser, and LLVM IR generator, all in pure Brief
using `struct` with `Int[N]` arrays. No `Alloc#`, no custom C, no manual memory.

## Implementation Order

1. Parser: add `Int[N]` syntax
2. Normalizer: add `Type::Vector` → `[N x T]` emission
3. LLVM codegen: add GEP path for array index
4. `>>` lexer: split `>>` into two `>` in type context
5. Discard binding: `let _ = expr`
6. Tuple destructuring: `let (a, b) = expr`
7. Type checker: tuple destructuring binding
8. Write tamer structs and VM
9. Build `briefc build --backend llvm lib/tamer/main.bv -o tamer`
10. Test: produce .bounty and process with tamer

## Future Work: `struct` Migration

`type { field: Type }` patterns (Arena, etc.) should migrate to `struct { field: Type }`.
The `type` keyword becomes purely for protocol/operator definitions:

| Construct | Keyword | Example |
|-----------|---------|---------|
| Protocol | `type` | `type Int: #Int { op Add(#Int); };` |
| Data | `struct` | `struct Arena { base: Ptr<Byte>; offset: Int; };` |
| Object | `obj` | `obj Channel<T> { ... };` |

## Future Work: Bracket Array Syntax with SIMD

`Int[N]` arrays gain bracket operations:

```brief
struct Matrix { data: Float[16]; };
defn add(m: Matrix, n: Matrix) -> Matrix {
    term m + n;  // auto-vectorized: 4× vec4 fadd
};
```

Planned operations:
- **Slice**: `arr[0:1024:2]` → strided view at compile time
- **Map**: `arr.map(f)` → element-wise transform, auto-vectorized
- **Reduce**: `arr.reduce(+)` → horizontal reduction
- **Contract-bounds**: `[idx < arr.len]` proves safety of every access
- **LLVM lowering**: `[N x T]` → SLP vectorizer + loop vectorizer natively
