# Brief Language Extensions for Bare-Metal Development

**Document Status:** Planning/Draft - Syntax NOT Final
**Created:** 2026-04-22
**Language Target:** .bv (Core Brief)

---

## ⚠️ IMPORTANT: FOR AI WORKING ON THE COMPILER

**This document is a DRAFT/PLAN. The syntax proposed here is NOT final and should NOT be implemented as-is.**

If you are an AI working on the Brief compiler:
- DO NOT implement any syntax from this document without explicit approval
- The purpose of this document is to RECORD ideas, not ASSIGN work
- Wait for explicit instruction before implementing any feature from here
- This document may contain incorrect assumptions or impractical syntax

---

## Purpose

This document describes extensions needed to make Brief (.bv) suitable for bare-metal software development, specifically targeting systems like the ARM Cortex-A53 on KV260 that run without an OS.

Current Brief can describe hardware state machines (.ebv) and has FFI support for OS-level operations (.bv), but lacks features needed for bare-metal/system-level programming.

---

## Current State

### Already Supported
- Transactions (`rct txn`)
- FFI for external function calls (`frgn`, `syscall`)
- Resource declarations (`rsrc Socket`)
- Standard library: `std/io`, `std/http`, `std/json`
- Struct definitions
- Enums

### Missing for Bare-Metal
- No standard library linkage control
- No raw pointer types
- No volatile memory access
- No static mutable data
- No inline assembly
- No custom panic handlers
- No custom entry points
- No memory section attributes
- No atomic operations
- No embedded data (byte arrays)

---

## Proposed Extensions

### Priority Scale
- **P0-Critical**: Without this, bare-metal is impossible
- **P1-Important**: Strongly needed for practical bare-metal code
- **P2-Nice-to-Have**: Useful but can be worked around
- **P3-Future**: Can be added later

---

## P0-Critical Extensions

### 1. No-Standard Library Mode

**Problem:** Bare-metal code cannot link the standard library.

**Current (OS-level .bv):**
```brief
import std/io;
```

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
#![no_std]
```

**Notes:**
- Compiler should reject `std/*` imports when this is set
- Automatic insertion of `#![no_std]` when targeting embedded

---

### 2. Panic Handler

**Problem:** Need custom panic behavior for embedded systems.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
#![panic(my_panic_handler)]

fn my_panic_handler(info: PanicInfo) -> ! {
    uart_puts("PANIC: ");
    uart_puts(info.message());
    loop { }
}
```

**Compiler Behavior:**
- Generates linker flags to use custom handler
- Rejects default std panic behavior

---

### 3. Raw Pointer Types

**Problem:** Need to access memory-mapped hardware registers.

**Current:** Not possible in Brief

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Typed pointer to MMIO register block
let mmio: *mut State = 0x4000A000 as *mut State;

// Dereferencing (unsafe)
let val = *mmio;  // or (*mmio).field
```

**Notes:**
- Pointer arithmetic needed for array access
- Should require `unsafe` blocks
- Type-safe through struct pointers

---

### 4. Volatile Memory Access

**Problem:** MMIO registers must be read/written with volatile semantics to prevent optimization.

**Current:** Not possible, optimizer could reorder or cache accesses.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Option A: Built-in functions
let val = volatile_read(addr);
volatile_write(addr, val);

// Option B: Pointer attribute
let reg: *volatile u32 = 0x4000A000 as *volatile u32;
let val = *reg;  // Compiler knows volatile

// Option C: Method on raw pointer
let val = mmio.volatile_read();
```

**Notes:**
- Compiler must emit appropriate memory barriers
- ARM: `volatile` keyword or `__asm__ __volatile__`

---

### 5. Custom Entry Point

**Problem:** Bare-metal needs custom start address, not `main`.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
#![entry(_start)]

#[no_mangle]
fn _start() -> ! {
    // Called by bootloader, not main
    main();
}
```

**Alternative Syntax:**
```brief
fn #![start] main() -> ! {
    // This is the entry point
}
```

---

## P1-Important Extensions

### 6. Link Section Attributes

**Problem:** Need to place code/data in specific memory regions.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Place code in specific section
#[link_section(".text.boot")]
fn boot_code() { }

// Place data in specific section
#[link_section(".data")]
static BOOT_FLAG: u32 = 0;
```

**Use Cases:**
- Boot code at fixed address
- Device tree in known location
- Shared memory regions

---

### 7. Static Mutables

**Problem:** Need mutable global state without runtime borrow checking.

**Current:** Not possible in safe Brief.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Option A: Static with unsafe access
static mut COUNTER: u32 = 0;

fn increment() {
    unsafe { COUNTER += 1; }
}

// Option B: Atomic wrapper
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn increment() {
    COUNTER.fetch_add(1);
}
```

---

### 8. Embedded Data

**Problem:** Need to include binary data (vocab, weights) directly in binary.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Option A: Hex string
static VOCAB: [u8; 100] = #embed_hex("4142434445");  // "ABCDE"

// Option B: File inclusion
static VOCAB: [u8; 100] = #embed("vocab.bin");

// Option C: Base64
static VOCAB: [u8; 100] = #embed_base64("QUJDREVGRw==");
```

**Notes:**
- `include_bytes!()` equivalent
- May need alignment directives

---

### 9. Inline Assembly

**Problem:** Need to write architecture-specific instructions.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
fn enable_interrupts() {
    asm("cpsie i");
}

fn memory_barrier() {
    asm("dmb sy" ::: "memory");
}
```

**Alternative (more structured):**
```brief
let result = asm!("mrs $0, PRIMASK" : "=r"(result));
```

---

### 10. Atomic Operations

**Problem:** Need atomic operations for multi-core or ISR communication.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Option A: Built-in atomics
let flag = AtomicBool::new(false);
flag.store(true);
flag.load();

// Option B: Atomic FFI
frgn atomic_store(addr: *mut u32, val: u32) -> Void from "atomics.toml";
frgn atomic_load(addr: *mut u32) -> u32 from "atomics.toml";
```

**Specific Operations Needed:**
- `atomic_store`, `atomic_load`
- `atomic_fetch_add`, `atomic_fetch_sub`
- `atomic_compare_exchange`

---

## P2-Nice-to-Have Extensions

### 11. MMIO Struct Packing

**Problem:** Memory-mapped structs should auto-calculate offsets.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Compiler calculates field offsets
mmio struct State {
    control: u32,    // offset 0
    status: u32,     // offset 4
    padding: [u8; 4],  // explicit padding if needed
    opcode: u32,     // offset 8
}
```

**Notes:**
- Similar to C `__attribute__((packed))`
- Compiler generates size and offset constants

---

### 12. No-Mangle Attribute

**Problem:** Need symbols to match what linker expects.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
#[no_mangle]
fn _start() { }  // Symbol will be "_start", not mangled
```

---

### 13. Compiler Intrinsics

**Problem:** Need access to hardware intrinsics (popcount, clz, etc.).

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
let count = __builtin_popcount(val);
let leading = __builtin_clz(val);
let ctz = __builtin_ctz(val);
```

---

## P3-Future Extensions

### 14. Interrupt Handler Attributes

**Problem:** Bare-metal needs to define interrupt handlers.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
#[interrupt(UART0)]
fn uart_handler() {
    // Handle UART interrupt
    uart_clear_irq();
}
```

---

### 15. Custom Sections for Relaxation

**Problem:** Need finer control over linker script input.

**Proposed Syntax (DRAFT - NOT FINAL):**
```brief
// Provide hints to linker
#[section(".bss.NOINIT")]
static LARGE_BUFFER: [u8; 65536] = undefined;
```

---

## Implementation Priority Order

If implementing these extensions, suggested order:

1. **P0-Critical (Unblock bare-metal):**
   - `#![no_std]`
   - Panic handler
   - Raw pointer types
   - Volatile access
   - Custom entry point

2. **P1-Important (Practical bare-metal):**
   - Link section attributes
   - Static mutables
   - Embedded data
   - Inline asm
   - Atomics

3. **P2-Nice-to-Have:**
   - MMIO struct packing
   - No-mangle
   - Intrinsics

4. **P3-Future:**
   - Interrupt handlers
   - Section relaxation hints

---

## Rejected Ideas

### No GC / Heap Allocation

Brief should NOT add heap allocation for bare-metal. Use static buffers or stack.

### No Virtual Memory Tables

For KV260 bare-metal, no MMU/MPU table generation. Programmer manages flat address space.

### No Exception Handling

Brief transactions replace exception-based flow. No `try/catch/throw`.

### No Dynamic Dispatch

No vtables or dynamic dispatch. Statically resolved calls only.

---

## Open Questions

1. **Should bare-metal be a separate variant (.bmv)?** Or continue with `#![no_std]` attribute?

2. **How to handle FFI naming?** Bare-metal may need to specify symbol names without libraries.

3. **Should we generate linker scripts?** Or provide attributes that feed into external linker script?

4. **Testing strategy?** How to test bare-metal code without hardware?

5. **IDE support?** LLDB/Rust analyzer integration for bare-metal debugging.

---

## References

- [Rust Bare Metal Book](https://docs.rust-embedded.org/book/) - Inspiration for no_std patterns
- [Cortex-M Startup](https://docs.rust-embedded.org/cortex-m/)- Similar interrupt/panic patterns
- [LLVM Atomic intrinsics](https://llvm.org/docs/Atomics.html) - Atomic operation semantics

---

## Change Log

| Date | Change |
|------|--------|
| 2026-04-22 | Initial draft created |

---

**End of Document**