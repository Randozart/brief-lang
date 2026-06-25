# `Ptr<T>` — Typed Memory Addresses

**Date added:** 2026-06-25
**Status:** Planned (see `docs/plans/2026-06-25-native-brief-io.md`)

## Purpose

`Ptr<T>` is a first-class type representing a typed memory address. It carries
the address value at runtime and the pointee type `T` at the type level, enabling
contract-proven safe MMIO and memory access without a borrow checker or GC.

A `Ptr<T>` **cannot be dereferenced directly** — reading or writing through a
pointer requires the `volatile_load#`/`volatile_store#` intrinsics (Phase 2).
This ensures all memory access is explicit and contract-verified.

## Type representation

```brief
// Ptr<T> is represented as Type::Applied("Ptr", vec![T]) in the AST.
// No dedicated Type::Ptr variant — consistent with List<T>, Option<T>, etc.
let p: Ptr<Byte> = 0x40011000 as Ptr<Byte>;
```

## Operations

All arithmetic operations preserve `T` — `ptr + 4` is still `Ptr<Byte>`.
Use explicit `as Ptr<Int>` cast for type reinterpretation.

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
| `ptr :> Ptr!` | `Int` | Projection — raw address escape hatch |
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
defn read_device(reg: Ptr<Byte>) -> Byte
    [reg :> Ptr! >= UART0_BASE]
    [reg :> Ptr! <  UART0_END]
{
    term volatile_load#(reg);
};
```

The proof engine already handles `>=` and `<`. The projection `:> Ptr!`
extracts the raw u64 address for use in contract expressions. No new
verification machinery needed.

## Explicit casts only

There is no implicit coercion between `Ptr<T>` and `Int`. All conversions
require explicit `as`:

```brief
let addr: Int = ptr as Int;           // Ptr<T> → Int
let ptr: Ptr<Byte> = addr as Ptr<Byte>;  // Int → Ptr<T>
```

## Runtime representation

In both the interpreter and LLVM backend, a `Ptr<T>` value is stored as
`u64`/`i64` — the same size as `Int`. The pointee type `T` exists only
at the type level (LLVM IR type annotation via `TypedRegister.ty`).

## Relationship to other features

| Feature | Relationship |
|---|---|
| `volatile_load#`/`volatile_store#` | Phase 2 — the only way to dereference a Ptr<T> |
| `import "target"` (Phase 5) | Board DBL files populate typed Ptr<T> constants |
| `as` casts | Ptr ↔ Int uses the existing `Expr::Cast` mechanism |
| Contracts | `:> Ptr!` projection enables contract bounds checking |
| BILD `asm target { }` | Syscall stubs use Ptr<T> for buffer addresses |
