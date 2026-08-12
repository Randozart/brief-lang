# Native Briev I/O: Eliminating the C Runtime Dependency

**Date:** 2026-06-25
**Status:** Planned (awaiting implementation start)
**Priority:** High

## Goal

Make the C runtime (`briev_rt.c`) optional by expressing all I/O through three
Briev-native primitives: `Ptr<T>`, `volatile_load#`/`volatile_store#`, and
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
- **BILD is just LLVM IR with Briev conventions** — The emission is verbatim
  paste. Any LLVM IR instruction works. BILD adds only label-based params,
  automatic `term`→`ret` lowering, and the new `asm target { }` desugaring.
- **Every new syntax gets an example file** — `examples/` receives a working
  `.bv` file demonstrating each new construct before it lands in stdlib.
- **Arch docs and learn-briev updated in the same commit** — structural
  language changes always include documentation in the same diff.

## Language extensions beyond Phases 1-6

Three language extensions make every "not replaced" case expressible in
pure Briev. They are prerequisites that run alongside Phases 1-3.

### Extension A: Universal asm in BILD — `asm target { }`

A new BILD statement that desugars to target-specific LLVM inline asm at
BILD compile time. Syntax:

```bild
inop! syscall(nr: Int, a1: Int, a2: Int, a3: Int) -> Int {
    %res = asm target {
        [arch("x86_64")]:
            "mov %2, %%r10; syscall"
            : "={rax},{rax},{rdi},{rsi},{rdx},{r10}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3);
        [arch("aarch64")]:
            "svc #0"
            : "={x0},{x8},{x0},{x1},{x2}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3);
        [arch("riscv64")]:
            "ecall"
            : "={a0},{a7},{a0},{a1},{a2}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3);
        default:
            "ud2"
            : "={rax},{rax},{rdi},{rsi},{rdx}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3);
    };
    term %res;
} fallback -1;
```

**Desugaring rules:**
1. Compiler evaluates `target_arch` (same mechanism as `#!cfg`)
2. Selects the matching `[arch("...")]` arm (first match wins; `default` always matches)
3. `[arch("x86_64", "amd64")]` — multiple arch names, matches any
4. `[os("linux")]` — predicate on `target_os` instead of `target_arch`
5. Compound: `[arch("x86_64"), os("windows")]` — both must match
6. Desugars the triple `"instr" : "constraints" : (types %reg1, %reg2)` into
   standard LLVM `call <ty> asm "<instr>", "<constraints>"(<types> %reg1, %reg2)`
7. The result register appears first in the constraint string (like `={rax}`)

**Where this applies in BILD:**
The `asm target { }` block can appear anywhere a BILD statement is expected.
It desugars to a single `call asm` statement — one line of LLVM IR, selected
and assembled at BILD compile time.

### Extension B: Function pointer type `fn(T) -> U` and `&f` address-of

```briev
type SignalHandler = fn(Int) -> Void;

defn my_handler(sig: Int) -> Void { ... };

let h: SignalHandler = &my_handler;
h(signum);
```

Also works for `inop!` declarations:

```briev
inop! __trampoline(fn: fn(Int) -> Int, arg: Int) -> Int {
    %res = call i64 %fn(i64 %arg);
    term %res;
} fallback 0;

// Get the address as an integer (for passing to clone/sigaction syscall)
let handler_addr: Int = &__trampoline as Int;
```

**LLVM IR mapping:**
- `&f` on a `defn` or `inop!` → LLVM function pointer (`i64` boxed, or `ptr` in opaque pointer mode)
- Indirect call `h(args)` → `call i64 %h(i64 %arg)` with appropriate `bitcast`
- `fn(Int) -> Int` type → `ptr` in LLVM IR, assigned in the type system as `Type::Fn(Vec<Type>, Box<Type>)`

**Safety:**
- Calling a `fn` pointer has the same contract requirements as calling the
  original function — the compiler can verify pre/post at the indirect call
  site if the function pointer type carries the contract signature.
- A bare `fn(Int) -> Int` without contracts is allowed but the compiler emits
  a note that contract verification was skipped.

### Extension C: `#section("name")` attribute on `inop!` declarations

```briev
#section(".init_array")
inop! __constructor() -> Void {
    call void @__rt_init();
    ret void;
}

#section(".isr_vector")
inop! __vector_table() -> Void {
    // Emit interrupt vector entries as data
    ...
};
```

**LLVM IR:** `define void @__constructor() section ".init_array" { ... }`

**Use cases:**
- `.init_array` — constructors run before `main` (POSIX)
- `.isr_vector` — interrupt vector table (bare-metal ARM/RISC-V)
- `.text.itcm` — tightly-coupled memory (performance-critical)
- `.ramfunc` — functions that must run from RAM

### Extension D: Symexec fallthrough for all LLVM IR opcodes

The BILD symbolic execution engine (`bild_symexec.rs`) currently errors on
unsupported opcodes (`inttoptr`, `ptrtoint`, `bitcast`, `phi`, `br`, `switch`,
etc.). Change these from error to opaque — matching the existing treatment of
`load`/`store`/`call`/`alloca`/`GEP`.

```rust
// Before: SymExecError::UnsupportedOpcode
// After: Ok(SymValue::Opaque)
```

This means:
- BILD bodies using any LLVM IR instruction compile and run correctly
- Contract verification falls through to the `fallback` expression
- No compilation errors for using `inttoptr`/`phi`/`br` in BILD
- The "BILD subset" is now "all of LLVM IR" for compilation purposes

### How these eliminate the "not replaced" items

| Item | Previously "not replaced" | Now expressible via |
|------|--------------------------|---------------------|
| **Thread creation** | Needed C trampoline | `clone` syscall via `asm target { }`, stack via `mmap` syscall, thread entry via `fn` pointer — all in an `inop!` body |
| **Dynamic linking** | Needed `dlopen`/`dlsym` from C | `openat` + `mmap` syscalls + pure Briev ELF parser on `Ptr<Byte>`; `dlsym` is a symbol table walk |
| **Signal handlers** | C function with specific ABI | `sigaction` syscall via `asm target { }`, handler is an `inop!` with `#section` if needed, passed as `fn` pointer |
| **Arch startup** | C `__attribute__((constructor))` | `#section(".init_array") inop! __ctor() { ... }` |
| **TLS setup** | Inline asm in C | `asm target { [arch("x86_64")]: "wrfsbase %0" : ... }` in an `inop!` body |

The "not replaced" list in Phase 6 is removed — everything is expressible.

---

## Phase 1: `Ptr<T>` as a first-class Briev type

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

```briev
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

```briev
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
  (not a runtime check — Briev is not a "blame the programmer" language
  for MMIO)

---

## Phase 3: BILD-inline syscalls for kernel operations

Not a compiler intrinsic — a BILD `inop!` declaration in the standard library.
Uses the `asm target { }` syntax (Extension A) for cross-architecture dispatch
within a single `inop!` body.

### Universal syscall (one `inop!` for all architectures)

```briev
// lib/std/syscall.bv
inop! syscall6(nr: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int, a6: Int) -> Int
    [nr > 0][nr < 512]
{
    %res = asm target {
        [arch("x86_64")]:
            "syscall"
            : "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
        [arch("aarch64")]:
            "svc #0"
            : "={x0},{x8},{x0},{x1},{x2},{x3},{x4},{x5}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
        default:
            "ud2"
            : "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
    };
    term %res;
} fallback -1;
```

### Why BILD is sufficient

- BILD bodies are pasted verbatim into LLVM IR output
- `asm target { }` desugars to a single `call asm` statement at BILD compile time
- LLVM `call asm` supports full inline assembly with register constraints
- The `fallback` expression provides interpreter/non-LLVM semantics
- Zero C required — `llc` produces the binary directly

---

## Phase 4: `#!cfg` conditional compilation

### Syntax (extends existing `#!` pragma system)

```briev
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
briev compile --board stm32f407 --os freestanding my_program.bv
briev compile --target x86_64-unknown-linux-gnu my_program.bv  # auto-detects target_os=linux, target_arch=x86_64
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

### Import in Briev code

```briev
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

### What remains as `frgn` after all phases

With the language extensions (A–D), nothing is fundamentally inexpressible in
pure Briev. The following remain as `frgn` only for pragmatic reasons:

- **GPU intrinsics** (`get_global_id#` etc.) — hardware-specific, no C involved
- **Math intrinsics** (`sqrt#`, `sin#`, etc.) — LLVM native, zero C needed
- **Collection helpers** (`sort#`, `reverse#`, `trim_left#`) — pure Briev now,
  already being migrated; no C dependency

Thread creation, dynamic linking, signal handling, and arch startup are all
expressible via syscalls + BILD + function pointers. See Extensions A–D above.

### Impact on `briev_rt.c`

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
4. The C runtime function becomes dead code — removed from `briev_rt.c`

---

## Documentation and examples commitment

Every new syntax construct MUST ship with:

### Example files (`examples/`)

| New syntax | Example file | Content |
|---|---|---|
| `Ptr<T>` type + ops | `examples/ptr-arithmetic.bv` | Arithmetic, casts, comparisons |
| `volatile_load#`/`volatile_store#` | `examples/volatile-io.bv` | MMIO-style read/write with contracts |
| `asm target { }` in BILD | `examples/bild-asm-target.bv` | Multi-arch asm dispatch |
| `fn(T) -> U` + `&f` | `examples/function-pointers.bv` | Indirect calls, address-of |
| `#section("name")` | `examples/section-attr.bv` | Section placement on inop! |
| `#!cfg(...)` | `examples/cfg-guards.bv` | Conditional compilation |
| `import "target"` | `examples/target-import.bv` | Board DBL import + MMIO access |

### Architecture docs (`docs/architecture/`)

| Document | Content |
|---|---|
| `docs/architecture/features/ptr.md` | `Ptr<T>` type, operations, contract integration |
| `docs/architecture/features/volatile-io.md` | `volatile_load#`/`volatile_store#` semantics |
| `docs/architecture/features/bild.md` | Add `asm target { }`, `#section` to BILD reference |
| `docs/architecture/features/cfg.md` | `#!cfg` condition reference |
| `docs/architecture/features/fn-ptr.md` | Function pointer type, `&f` syntax |
| `docs/architecture/features/target-import.md` | DBL-based board import |

### Learn Briev (`learn-briev/`)

Add or update lessons covering:
- Pointers and MMIO (`learn-briev/07-pointers-and-mmio.md`)
- Platform-aware code with `#!cfg` (`learn-briev/08-platform-code.md`)
- BILD and inline assembly (`learn-briev/09-bild-and-asm.md`)
- Targets and device trees (`learn-briev/10-targets-and-boards.md`)

All documentation changes must land in the **same commit** as the structural
change they describe.

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
- Update architecture docs if API contracts changed
- Update `learn-briev/` for any user-facing syntax change
- Create or update example `.bv` file for every new construct
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
