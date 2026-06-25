# `Ptr<T>` — Typed Memory Addresses

**Date added:** 2026-06-25
**Status:** Active (Phase 1+2 complete, see `docs/plans/2026-06-25-native-brief-io.md`)

## Purpose

`Ptr<T>` is a first-class type representing a typed memory address. It carries
the address value at runtime and the pointee type `T` at the type level, enabling
contract-proven safe MMIO and memory access without a borrow checker or GC.

A `Ptr<T>` **cannot be dereferenced directly** — reading or writing through a
pointer requires the `volatile_load#`/`volatile_store#` intrinsics (Phase 2).
This ensures all memory access is explicit and contract-verified.

## Type representation

```brief
// Ptr<T> is Type::Applied("Ptr", vec![T]) in the AST — no dedicated variant.
let p: Ptr<Int> = 0x40011000 as Ptr<Int>;
```

## Operations

All arithmetic operations preserve `T` — `ptr + 4` is still `Ptr<Int>`.
Use explicit `as Ptr<U>` cast for type reinterpretation (type punning).

| Expression | Result | Notes |
|---|---|---|
| `ptr + n` | `Ptr<T>` | Wrapping byte offset |
| `ptr - n` | `Ptr<T>` | Wrapping negative offset |
| `ptr ^ n` | `Ptr<T>` | Bitwise XOR address with mask |
| `ptr & n` | `Ptr<T>` | Bitwise AND address with mask |
| `ptr \| n` | `Ptr<T>` | Bitwise OR address with mask |
| `ptr << n` | `Ptr<T>` | Left shift address |
| `ptr >> n` | `Ptr<T>` | Logical right shift address |
| `ptr as Int` | `Int` | Extract raw address (explicit cast) |
| `addr as Ptr<T>` | `Ptr<T>` | Reinterpret Int as typed pointer |
| `ptr as Ptr<U>` | `Ptr<U>` | Type punning — same address, new pointee type |
| `ptr_a == ptr_b` | `Bool` | Address equality |
| `ptr_a != ptr_b` | `Bool` | Address inequality |
| `ptr_a < ptr_b` | `Bool` | Address less-than |
| `ptr_a <= ptr_b` | `Bool` | Address less-or-equal |
| `ptr_a > ptr_b` | `Bool` | Address greater-than |
| `ptr_a >= ptr_b` | `Bool` | Address greater-or-equal |

## Safety via contracts

Safety is proven by the existing contract system, not by a borrow checker.
A pointer is just an integer with a *provenance* — the contract proves it
points to valid memory:

```brief
defn read_device(reg: Ptr<Int>) -> Int
    [reg as Int >= UART0_BASE]
    [reg as Int <  UART0_END]
{
    term volatile_load#(reg);
};
```

The expression `reg as Int` converts the pointer to its raw address for use
in contract comparisons. The proof engine handles `>=` and `<` natively.

## Explicit casts only — no implicit coercions

There is no implicit coercion between `Ptr<T>` and `Int`. All conversions
require explicit `as`:

```brief
let addr: Int = ptr as Int;              // Ptr<T> → Int
let ptr: Ptr<Int> = addr as Ptr<Int>;   // Int → Ptr<T>
let punned: Ptr<Char> = ptr as Ptr<Char>; // Ptr<Int> → Ptr<Char> (type punning)
```

## Low-level trickery

Ptr<T> supports all the operations you'd expect from a C pointer, without
the implicit coercion foot-guns:

### MMIO register block navigation

```brief
const UART_DR:   Ptr<Int> = 0x40011000 as Ptr<Int>;
const UART_FR:   Ptr<Int> = 0x40011004 as Ptr<Int>;   // DR + 4
const UART_LCRH: Ptr<Int> = 0x40011010 as Ptr<Int>;   // DR + 16

// Computed navigation:
let fr: Ptr<Int> = UART_DR + 4;   // arithmetic, preserves Ptr<Int>
```

### Type punning (reinterpretation)

Cast a `Ptr<Int>` to `Ptr<Char>` to access the same memory with different
semantics. The address stays the same — only the type-level pointee changes:

```brief
let reg: Ptr<Int> = 0x40011000 as Ptr<Int>;
let as_chars: Ptr<Char> = reg as Ptr<Char>;  // same address, Char semantics
```

This is a zero-cost reinterpretation — no runtime conversion. The LLVM
backend emits the same `i64` address register; only the `TypedRegister.ty`
changes, which affects downstream `volatile_load#` codegen.

### Address alignment and bit manipulation

```brief
let misaligned: Ptr<Int> = 0x40011007 as Ptr<Int>;
let aligned: Ptr<Int> = misaligned & !7;   // align down to 8-byte boundary
let masked: Ptr<Int> = misaligned ^ 0xFFF; // toggle low 12 bits
```

### Runtime representation

A `Ptr<T>` value is `u64`/`i64` in both the interpreter and LLVM backend —
identical in size to `Int`. The pointee type `T` exists only at the type
level (`TypedRegister.ty` in the backend).

## Type definitions with Bits

For fine-grained control over pointer semantics, use the type universe
to define custom integer types from `Bits`:

```brief
type Byte <: Bits {
    Bytes = 1;
    Alignment = 1;
};
```

Then use `Ptr<Byte>` as a pointer to byte-addressed memory. Note: the
LLVM backend currently maps all custom types to `i64` for load/store width;
`Bytes = 1` is tracked for contract and alignment reasoning but does not
(yet) change the LLVM IR type for `volatile_load#`/`volatile_store#`.

## Intrinsics

| Intrinsic | Signature | Description |
|---|---|---|
| `volatile_load#` | `(Ptr<T>) -> T` | Volatile read from MMIO register |
| `volatile_store#` | `(Ptr<T>, T) -> Bool` | Volatile write to MMIO register |

Both are `inop!` (side-effecting, not foldable). Contracts prove pointer
validity at compile time. Without a proven contract, the compiler emits a
compile error — Brief is not a "blame the programmer" language for MMIO.

## BILD asm target with Ptr<T>

Inline assembly in BILD bodies can take `Ptr<T>` values as arguments.
The address is passed as an `i64` to the asm block:

```bild
inop! dma_transfer(src: Ptr<Int>, dst: Ptr<Int>, len: Int) -> Int {
    %res = asm target {
        [arch("x86_64")]:
            "mov %2, %%rcx; rep movsb"
            : "={rax},{rsi},{rdi},{rcx}"
            : (i64 %src, i64 %dst, i64 %len);
        default:
            "ud2"
            : "={rax}"
            : (i64 %src);
    };
    term %res;
} fallback -1;
```

## Relationship to other features

| Feature | Relationship |
|---|---|
| `volatile_load#`/`volatile_store#` | Phase 2 — the only way to dereference a Ptr<T> |
| `import "target"` (Phase 5) | Board DBL files populate typed Ptr<T> constants |
| `as` casts | Ptr ↔ Int, Ptr<T> ↔ Ptr<U> use `Expr::Cast` |
| Contracts | `ptr as Int` enables contract bounds checking |
| BILD `asm target { }` | Syscall stubs use Ptr<T> for buffer addresses |
| Type universe (`Bits`) | Define custom types with `Bytes = N` for Ptr<T> semantics |
