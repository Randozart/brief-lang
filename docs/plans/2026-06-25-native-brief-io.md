# Native Brief I/O: Eliminating the C Runtime Dependency

**Date:** 2026-06-25
**Status:** Planned (awaiting implementation start)
**Priority:** High

## Goal

Make the C runtime (`brief_rt.c`) optional by expressing all I/O through three
Brief-native primitives: `Ptr<T>`, `volatile_load#`/`volatile_store#`, and
BILD-inline syscalls. Platform descriptions live in DBS/DBL files — data,
not code.

## Key design decisions

- **`Ptr<T>` arithmetic keeps `T`** — `ptr + 4` is still `Ptr<Byte>`. Explicit
  `ptr as Ptr<Int>` for re-interpretation. Most trackable, no alignment ambiguity.
- **Explicit `as` casts only** — No implicit coercion between `Ptr<T>` and `Int`.
  `ptr as Int`, `addr as Ptr<Byte>` required everywhere.
- **Contracts prove safety** — The existing contract system validates pointer
  bounds. No borrow checker, no GC, no runtime tag. Just `[ptr >= BASE][ptr < END]`.
- **C runtime becomes optional** — Thread pool and arch startup remain as a
  compatibility shim; terminal/file/socket/timer I/O all go native.

## Phase 1: `Ptr<T>` as a first-class Brief type

### Type system

- `Type::Ptr(Box<Type>)` — AST node for pointer types
- `Value::Ptr(u64)` — interpreter value carrying a concrete address
- All pointers carry the pointee type at the type level, not at runtime

### Operations on `Ptr<T>`

All arithmetic operations preserve `T`:

| Expression | Result | Notes |
|---|---|---|
| `ptr + n` | `Ptr<T>` | Offset by `n` bytes |
| `ptr - n` | `Ptr<T>` | Negative offset |
| `ptr ^ n` | `Ptr<T>` | Bitwise XOR |
| `ptr & n` | `Ptr<T>` | Bitwise AND |
| `ptr \| n` | `Ptr<T>` | Bitwise OR |
| `ptr << n` | `Ptr<T>` | Left shift |
| `ptr >> n` | `Ptr<T>` | Right shift (logical) |
| `ptr as Int` | `Int` | Extract raw address |
| `addr as Ptr<T>` | `Ptr<T>` | Int → typed pointer |
| `ptr_a == ptr_b` | `Bool` | Address equality |
| `ptr_a < ptr_b` | `Bool` | Address comparison |

### Contract integration

- `ptr :> as_int` extracts the underlying address in contract expressions
- The proof engine reasons about pointer bounds via existing integer reasoning:

```brief
defn read_device(reg: Ptr<Byte>) -> Byte
    [reg :> as_int >= UART0_BASE]
    [reg :> as_int <  UART0_END]
{
    term volatile_load#(reg);
};
```

### Interpreter

- `Value::Ptr(u64)` with full arithmetic, comparison, and cast support
- No dereference outside `volatile_load#` — safety by construction

### Parser

- `Ptr<T>` type annotation syntax in parameter/return/field positions
- `as` cast expressions: `expr as Ptr<Int>`, `expr as Int`
- Arithmetic and bitwise ops on `Ptr<T>` typed sub-expressions

---

## Phase 2: `volatile_load#` / `volatile_store#` intrinsics

### Signatures

```brief
volatile_load#<T>(addr: Ptr<T>) -> T;
volatile_store#<T>(addr: Ptr<T>, val: T);
```

These are `inop!` (side-effecting, not foldable) — `ptr must be valid` in the
contract is a compile-time assertion, not a runtime check.

### LLVM backend

| Intrinsic | LLVM IR |
|---|---|
| `volatile_load#<T>(%ptr)` | `load volatile T, T* %ptr` |
| `volatile_store#<T>(%ptr, %val)` | `store volatile T %val, T* %ptr` |

Size of `T` determines the LLVM type:
- `Byte` → `i8`
- `Char` → `i32`
- `Int` → `i64`
- `Float` → `float`
- `Bool` → `i8`

These replace the existing ad-hoc MMIO codegen in `emit_expr.rs` and
`emit_toplevel.rs` (trigger loading, state variable MMIO). The old `@ addr`
sugar becomes syntactic sugar for declaring a state variable with an
auto-generated `volatile_load#`/`volatile_store#` accessor.

### Interpreter

- `volatile_load#(Value::Ptr(addr))` — returns a sentinel/tracking value
  (interpreter cannot read real hardware registers)
- `volatile_store#(Value::Ptr(addr), val)` — no-op (or logs)
- Both are `Intrinsic::UserDefined` with `has_side_effects: true`

### Safety

- Contracts prove pointer validity at compile time
- Without a proven contract, the compiler emits a **compile error**
  (not a runtime check — Brief is not a "blame the programmer" language
  for MMIO)

---

## Phase 3: BILD-inline syscalls for kernel operations

Not a compiler intrinsic — a BILD `inop!` declaration in the standard library.

### x86_64 Linux syscall (example)

```brief
// lib/std/x86_64/linux/syscall.bv
inop! syscall6(nr: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int, a6: Int) -> Int
    [nr > 0][nr < 512]
{
    %res = call i64 asm "syscall", "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"
        (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
    term %res;
} fallback -1;
```

### aarch64 Linux syscall

```brief
// lib/std/aarch64/linux/syscall.bv
inop! syscall6(nr: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int, a6: Int) -> Int
    [nr > 0][nr < 512]
{
    %res = call i64 asm "svc #0", "={x0},{x8},{x0},{x1},{x2},{x3},{x4},{x5}"
        (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
    term %res;
} fallback -1;
```

### Why BILD is sufficient

- BILD bodies are pasted verbatim into LLVM IR output
- LLVM `call asm` supports full inline assembly with register constraints
- The `fallback` expression provides interpreter/non-LLVM semantics
- Zero C required — `llc` produces the binary directly

### Note on cross-architecture syscall dispatch

The syscall `inop!` is selected by `#!cfg` (Phase 4). A portable stdlib
module imports the arch-appropriate one:

```brief
// lib/std/syscall.bv
#!cfg(target_arch == "x86_64")
    include "x86_64/linux/syscall.bv";

#!cfg(target_arch == "aarch64")
    include "aarch64/linux/syscall.bv";
```

---

## Phase 4: `#!cfg` conditional compilation

### Syntax (extends existing `#!` pragma system)

```brief
// Guard a single definition
#!cfg(target_os == "linux")
defn __read_key() -> Int { syscall#(SYS_read, 0, buf, 1) };

// Guard a block of definitions
#!cfg(target_os == "freestanding")
{
    defn __read_key() -> Int { volatile_load#(uart_dr) };
    defn __write_key(c: Char) { volatile_store#(uart_dr, c) };
};

// Guard an include
#!cfg(target_arch == "x86_64")
    include "x86_64/tty.bv";
```

### Condition expressions

| Key | Example values | Source |
|---|---|---|
| `target_os` | `"linux"`, `"freestanding"`, `"none"` | `--target spec.toml` or auto-detect |
| `target_arch` | `"x86_64"`, `"aarch64"`, `"riscv64"`, `"thumbv7em"` | `--target spec.toml` or auto-detect |
| `board` | `"stm32f407"`, `"kv260"` | `--board` flag or target spec |
| `has_mmio_uart` | `true`, `false` | Board DBL capabilities |
| `has_fpu` | `true`, `false` | Target spec |

Compound: `target_os == "freestanding" && target_arch == "thumbv7em"`

### Evaluation

- Evaluated at parse/import time (before typechecking)
- Skipped definitions produce no AST nodes — dead code elimination is free
- No separate preprocessor pass — integrated into the existing `#!` pragma
  parsing in the lexer/parser pipeline
- Composes with macros: `$!` macros can emit `#!cfg`-gated output

### Compiler CLI

```bash
brief compile --board stm32f407 --os freestanding my_program.bv
brief compile --target x86_64-unknown-linux-gnu my_program.bv  # auto-detects target_os=linux, target_arch=x86_64
```

---

## Phase 5: DBS/DBL as device address maps

### Device schema (.dbvs)

Peripheral register layout defined as an `entry` schema:

```dbvs
// lib/devices/uart.dbvs
entry UartRegs {
    dr:  String;   // "UInt[8]  @ 0x00" — Data register
    sr:  String;   // "UInt[8]  @ 0x01" — Status register
    brr: String;   // "UInt[16] @ 0x08" — Baud rate register
};
```

```dbvs
// lib/devices/gpio.dbvs
entry GpioRegs {
    moder: String;  // "UInt[32] @ 0x00" — Mode register
    odr:   String;  // "UInt[32] @ 0x14" — Output data register
    idr:   String;  // "UInt[32] @ 0x10" — Input data register
};
```

### Board description (.dbvl)

Line-oriented data, one peripheral per line:

```dbvl
// lib/boards/stm32f407.dbvl
schema lib/boards/peripheral.dbvs;
uart1, lib/devices/uart.dbvs::UartRegs, 0x40011000
uart2, lib/devices/uart.dbvs::UartRegs, 0x40004400
uart3, lib/devices/uart.dbvs::UartRegs, 0x40004800
gpioa, lib/devices/gpio.dbvs::GpioRegs, 0x40020000
gpiob, lib/devices/gpio.dbvs::GpioRegs, 0x40020400
```

```dbvs
// lib/boards/peripheral.dbvs — validates the dbvl above
entry PeripheralEntry {
    name: String;       // e.g., "uart1"
    schema: String;     // e.g., "lib/devices/uart.dbvs::UartRegs"
    base_addr: String;  // e.g., "0x40011000" — hex as string for human readability
};
```

### Import in Brief code

```brief
// import "target" is a compiler directive that reads the board DBL
// and populates typed constants for each peripheral
import "target";

// Now use peripherals by name — they're compile-time constants
let c: Byte = volatile_load#(uart1.dr);
volatile_store#(gpioa.odr, 0xFF);

// Contracts are automatically bounded to the peripheral's address range
defn read_char() -> Byte
    [uart1.dr :> as_int >= 0x40011000]
    [uart1.dr :> as_int <  0x40011010]
{
    while (volatile_load#(uart1.sr) & 0x20) == 0 {}
    term volatile_load#(uart1.dr);
};
```

### Integration with existing infrastructure

- The existing `hardware_validator.rs` already validates schema imports and
  checks memory overlaps. Phase 5 reuses this machinery.
- The existing `--target-dbv` flag maps to the new DBL-based board system.
- The existing `lib/targets/stm32f407.dbv` and `kv260.dbv` files remain
  supported via a compat shim that converts `.dbv` → DBL at import time.

### How `import "target"` works

1. Compiler resolves board from `--board` flag or target spec
2. Loads `lib/boards/{board}.dbvl`
3. For each `PeripheralEntry`, loads the referenced `.dbvs` file
4. Parses register name/offset/type from the schema strings
5. Populates a compile-time `HashMap<String, StructInstance>` namespace
6. Each peripheral becomes a typed struct constant with `Ptr<T>` fields
7. Contract bounds are auto-derived from the schema's address ranges

---

## Phase 6: Rewrite stdlib I/O using these primitives

### Mappings

| Intrinsic | New implementation |
|---|---|
| `PrintInt` | `syscall#(SYS_write, 1, buf, len)` or `volatile_store#(UART_TX, ...)` |
| `PutChar` | Same — Char-sized volatile store or syscall |
| `TtyReadKey` | `volatile_load#(UART_RX)` or `syscall#(SYS_read, 0, buf, 1)` |
| `Open`/`Read`/`Write`/`Close` | `syscall#(SYS_open/SYS_read/SYS_write/SYS_close, ...)` |
| `Time`/`ClockGetTime` | `syscall#(SYS_clock_gettime, ...)` |
| `Exit` | `syscall#(SYS_exit, ...)` or `asm { "wfi" }` on freestanding |
| `GetEnvInt` | `syscall#(SYS_getenv, ...)` or `volatile_load#(HW_DIP_SWITCH)` |
| `Sleep`/`NanoSleep` | `syscall#(SYS_nanosleep, ...)` |
| `Mmap`/`MUnmap` | `syscall#(SYS_mmap/SYS_munmap, ...)` |
| `Pipe`/`Socket`/`Bind`/... | `syscall#(SYS_pipe/SYS_socket/SYS_bind/..., ...)` |
| `Futex` | `syscall#(SYS_futex, ...)` |
| `SigAction`/`SigProcMask` | `syscall#(SYS_rt_sigaction/SYS_rt_sigprocmask, ...)` |

### Not replaced (remain as `frgn`)

- **Thread creation** — requires runtime startup (TLS, stack setup)
- **Dynamic linking** (`dlopen`/`dlsym`) — requires loader
- **GPU intrinsics** (`get_global_id#` etc.) — hardware-specific, not I/O
- **Math intrinsics** (`sqrt#`, `sin#`, etc.) — LLVM native, zero C needed
- **Collection helpers** (`sort#`, `reverse#`, `trim_left#`) — pure Brief now,
  already being migrated; no C dependency

### Impact on `brief_rt.c`

- Shrinks from ~1744 lines to ~200
- Remaining: thread pool (`pthread_create`), arch startup (`__rt_init`),
  signal handling (`sigaction` wrapper for `@link` triggers)
- No longer required for:
  - Terminal I/O (UART MMIO or syscall)
  - File I/O (syscall)
  - Socket I/O (syscall)
  - Timer I/O (syscall or MMIO)
  - Process management (syscall)
  - Memory management (syscall)

---

## Migration strategy

| Phase | What changes | Backward compat? |
|---|---|---|
| 1 | `Ptr<T>` type + ops + casts | Yes — additive, no existing code affected |
| 2 | `volatile_load#`/`volatile_store#` intrinsics | Yes — old `@ addr` sugar desugars to these |
| 3 | BILD inline asm for syscalls | Yes — C runtime still linked by default |
| 4 | `#!cfg` pragma | Yes — optional, only evaluated when present |
| 5 | DBS/DBL `import "target"` | Yes — compat shim for `.dbv` targets |
| 6 | Stdlib rewrite | One intrinsic at a time, each with fallback |

Each intrinsic migration:
1. Add `inop!` BILD version in the appropriate `lib/std/<arch>/<os>/` directory
2. Guard with `#!cfg` alongside the old C-calling version
3. When the C-free version is stable, remove the C-calling version
4. The C runtime function becomes dead code — removed from `brief_rt.c`

---

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
- Update architecture docs if API contracts changed
- Kani harnesses for all safety-critical `Ptr<T>` operations
- `_ => return None;` fallthrough unchanged in all optimization passes
- No weakening of existing optimization paths

---

## Open questions

1. Should `volatile_load#`/`volatile_store#` be `Intrinsic` variants (compiler
   built-in) or `Intrinsic::UserDefined` via `inop!` in the stdlib?
   - **Tentative: `inop!` in stdlib** — keeps the compiler smaller, and the
     BILD body is trivial enough that the symbolic verifier can handle it.
   - But: the type parameter `<T>` in BILD is tricky — BILD doesn't currently
     support type-polymorphic bodies. May need a compiler intrinsic after all.

2. How does the DBL-based `import "target"` interact with the existing
   `hardware_validator.rs`? Should the validator consume DBL directly, or
   convert to the internal alias map first?
   - **Tentative:** Keep the validator on the internal alias map. Convert DBL
     to the alias map at import time, reuse all existing validation.
