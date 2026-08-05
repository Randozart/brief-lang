# Metropolitan FFI v2.2 - Implementation Specification

## Overview

This document defines the authoritative specification for the Metropolitan FFI system implementation in Briv v10+. It encompasses the complete feature set: FFI return type taxonomy, address system, resource negotiation, syscall support, memory architecture, and migration path from v1.

## Part 1: FFI Return Type Taxonomy

### The Four Variants

| Keyword | Return Type | Error Handling | Use Case |
|---------|-------------|---------------|----------|
| `frgn` | `Result<T, Error>` | Must handle (transaction can escape) | Foreign functions that return values |
| `frgn!` | `void` | No return, fire-and-forget | Side-effect-only FFI calls |
| `syscall` | `Result<Int, Error>` | Must handle | Kernel calls returning file descriptors/ints |
| `syscall!` | `void` | No return | Kernel calls with no meaningful return |

### Syntax Examples

```briv
// Standard FFI - returns Result, must handle errors
frgn calculate(x: Float) -> Result<Float, MathError> from "math.toml";

// Fire-and-forget - executes and returns void
frgn! println(msg: String);

// Kernel syscall with return (must handle errors)  
syscall open(path: String, flags: Int) -> Result<Int, Error>;

// Kernel syscall void (fire-and-forget)
syscall! munmap(addr: Int, len: Int);
```

### Implementation Details

- `frgn` and `syscall` return types are wrapped in `Result<T, Error>` by the compiler
- `frgn!` and `syscall!` (the `!` variants) are fire-and-forget - no Result wrapping
- Errors are determined per-target (negative return typically means error on most syscalls)
- The `!` suffix means "ignore any return value" - compiler skips Result wrapping and validation

## Part 2: Address System

### Context-Aware Addressing

The `@` address operator behaves differently based on the target:

| Target | `@address` Meaning |
|--------|-------------------|
| `.ebv` (Embedded) | Raw physical address from memory map |
| `.bv` (OS) | Virtual offset (compiler resolves base) |
| `.rbv` (Browser) | WASM memory offset |

### Address Scoping Syntax

For explicit control:

```
@raw:0x40021000      // Raw physical address (embedded)
@stack:offset        // Offset from current stack pointer  
@heap:pointer+offset // Offset from heap pointer
```

### Compiler Behavior by Target

**.bv (OS Target)**:
- `@address` is treated as an offset from a base pointer
- Compiler auto-determines heap vs stack vs static allocation via escape analysis
- Static resources get fixed addresses; transitory resources get mmap'd addresses
- Bounds checking enabled - compiler validates offset doesn't exceed allocation
- `@stack:` variables are allocated on stack frame
- `@heap:` variables use heap allocation (mmap on demand)

**.ebv (Embedded Target)**:
- `@address` is absolute physical address
- No heap - all memory is static/pool-based
- No bounds checking (programmer responsible)
- Only `@raw:` addressing mode supported

**.rbv (Browser Target)**:
- `@address` is WASM memory offset
- All memory is linear memory - no stack/heap distinction
- `@stack:` and `@heap:` are aliases for linear memory offsets

### Declarative Structs with Bit Packing

```briv
struct Pixel {
    r: 4bits,
    g: 4bits, 
    b: 4bits,
    a: 4bits
}

// Compiler packs into 2 bytes, resolves offsets automatically
let screen: Buffer(320, 200);
@screen[0] = pixel;
```

The compiler handles the layout - programmer declares intent, compiler figures addresses and bit offsets.

## Part 3: Resource System

### The `rsrc` / `resource` Keyword

Resources declare data that comes from OS/kernel negotiation:

```briv
// Aliases: rsrc and resource are interchangeable
rsrc screen: FrameBuffer(320, 200);
resource display: FrameBuffer(640, 480);
```

### Built-in Resource Types

The following resource types auto-negotiate with the kernel:

| Resource Type | Kernel Protocol |
|---------------|----------------|
| `FrameBuffer(w, h)` | DRM dumb buffer + mmap |
| `File(path)` | open() + mmap |
| `SharedMemory(name, size)` | shm_open() + mmap |
| `Socket(domain, type)` | socket() |
| `EventFD()` | eventfd() |
| `Semaphore(initial)` | sem_open() / unnamed semaphore |
| `Mutex` | pthread_mutex / futex |

### Resource Acquisition Protocol

```
1. Declare resource in Briv
       ↓
2. Compiler generates kernel calls (open → ioctl → mmap)
       ↓
3. Runtime negotiates with kernel
       ↓
4. Address returned and bound to variable
       ↓
5. Briv logic uses @address to access
```

### Resource Struct Form

```briv
// Type with constructor-style args
rsrc framebuffer: FrameBuffer(width: 320, height: 200);

// Usage
txn draw [tick][true] {
    &framebuffer = compute_frame();
    term;
}
```

### Resource Lifecycle

- **Static resources**: Known at compile time, fixed kernel handles
- **Dynamic resources**: Negotiated at runtime, may have variable handles
- **Automatic cleanup**: Resources referenced in transaction commits are cleaned up; orphaned resources are tracked and cleaned at process exit
- **Resource ownership**: Resources can be shared between transactions via handles

### Pre-defined Resource Constructors

```briv
// FrameBuffer: width, height in pixels, 32bpp
rsrc fb: FrameBuffer(1920, 1080);

// File: path string, open flags (O_RDONLY, O_WRONLY, O_RDWR)
rsrc f: File("/dev/fb0", O_RDWR);

// SharedMemory: name string, size in bytes
rsvc shm: SharedMemory("my_shm", 4096);

// Socket: domain (AF_INET), type (SOCK_STREAM), optional protocol
rsvc sock: Socket(AF_INET, SOCK_STREAM);

// EventFD: initial count, flags (0 for default)
rsvc ev: EventFD(0, 0);
```

## Part 4: Syscall Support

### Target-Specific Syscall Numbers

The compiler resolves syscall numbers at compile time per target:

```
Target          │ Syscall numbers from
────────────────┼───────────────────
linux_x64      │ arch/x86/usr/include/asm/unistd_64.h
linux_arm64     │ arch/arm64/asm/unistd.h  
macOS_x64       │ sys/syscall.h
macOS_arm64     │ sys/syscall.h
windows_x64    │ winnt.h / win32io.h
```

### Syscall Binding TOML

```toml
[[syscalls]]
name = "open"
syscall_num = { linux_x64 = 257, macOS_arm64 = 5, windows_x64 = "NtOpenFile" }

[syscalls.input]
path = "String"
flags = "Int"

[syscalls.output]
fd = "Int"
```

### Syscall Wrapper Types

For richer type information:

```briv
// Raw syscall returns Int (file descriptor or error code)
syscall open(path: String, flags: Int) -> Result<Int, Error>;

// Or wrapped in type (future enhancement)
syscall open(path: String, flags: Int) -> Result<FileDescriptor, Error>;
```

### Error Handling for Syscalls

- Positive return values are typically file descriptors or success indicators
- Negative return values indicate errors (errno mapping per OS)
- `syscall!` variants ignore the return value (fire-and-forget)
- `syscall` variants require Result handling (transactions can escape with errors)

## Part 5: Memory Architecture Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    BRIEF MEMORY MODELS                       │
├────────────────┬──────────────────────────────────────────┤
│   .ebv          │           .bv / .rbv                     │
│  (Embedded)     │        (OS / Browser)                   │
├────────────────┼──────────────────────────────────────────┤
│ @ = raw        │ @ = virtual offset                     │
│ No heap        │ Compiler manages heap/stack               │
│ Static memory  │ Escape analysis → stack if local       │
│ Fixed addrs    │ Static if permanent, mmap if transitory│
│ No OS calls    │ rsrc negotiates with kernel            │
└────────────────┴──────────────────────────────────────────┘
```

### Memory Addressing Modes

| Mode | Syntax | Description |
|------|--------|-------------|
| Raw | `@raw:0xADDRESS` | Direct physical address (embedded only) |
| Stack | `@stack:OFFSET` | Offset from stack pointer |
| Heap | `@heap:OFFSET` | Offset from heap base |
| Relative | `@label` | Relative to known symbol (future) |

### Escape Analysis

The compiler performs escape analysis to determine memory placement:
- **Stack-allocated**: Local variables, non-escaping references
- **Heap-allocated**: Escaping references, large allocations, dynamic lifetimes
- **Static**: Global constants, known-at-compile-time addresses

## Part 6: Complete Keyword Taxonomy

| Keyword | Category | Meaning |
|---------|----------|---------|
| `frgn` | FFI function | Foreign function → Result<T, Error> |
| `frgn!` | FFI function | Foreign function → void (fire-and-forget) |
| `syscall` | Kernel | Kernel call → Result<Int, Error> |
| `syscall!` | Kernel | Kernel call → void (fire-and-forget) |
| `rsrc` | Resource | External resource from OS (alias: `resource`) |
| `@raw:` | Address | Raw physical address |
| `@stack:` | Address | Stack-relative offset |
| `@heap:` | Address | Heap-relative offset |

## Part 7: Implementation Checklist

### Parser
- [x] Add `frgn!` keyword parsing (fire-and-forget)
- [x] Add `syscall` keyword parsing
- [x] Add `syscall!` keyword parsing  
- [x] Add `rsrc` / `resource` keyword parsing
- [x] Add address scoping: `@raw:`, `@stack:`, `@heap:`

### FFI Backend
- [x] FFI: Skip Result wrapping for `frgn!` (void return)
- [x] FFI: Error mapping for void returns
- [ ] Target-specific FFI kind dispatch (`frgn` vs `frgn!` vs `syscall` vs `syscall!`)

### Syscall System
- [ ] Target-specific syscall number tables
- [ ] Syscall binding loader (new TOML section)
- [ ] Direct syscall code generation per target
- [ ] Cache syscall numbers (invalidate on --no-cache or arch change)

### Address System
- [x] AddressMode enum: Raw, Virtual, Wasm
- [x] Target detection (.ebv vs .bv vs .rbv)
- [x] Compiler-based address resolution per target
- [x] Escape analysis for stack/heap determination
- [x] `@stack:` parsing and handling
- [x] `@heap:` parsing and handling

### Resource System
- [x] Built-in resource types:
  - [x] FrameBuffer
  - [ ] File
  - [ ] SharedMemory
  - [ ] Socket
  - [ ] EventFD
  - [ ] Semaphore
  - [ ] Mutex
- [x] Resource struct parsing
- [ ] Kernel negotiation code generation
- [ ] Resource lifecycle management

### Memory Layout
- [x] Bit-packed struct support
- [x] Automated offset resolution
- [ ] Bounds verification for indexed access
- [ ] Structured bit-field parsing (`struct` with bit widths)

## Migration from v1

Existing code continues to work:

```briv
// v1 style - still valid, auto-upgrades
frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// v2 explicit forms
frgn  sqrt(x: Float) -> Result<Float, MathError> from "math.toml";  // with Result
frgn! write_to_hw(address, value);  // fire and forget
```

Compiler auto-generates:
- `pre [true]` if no precondition
- `post [true]` if no postcondition  
- Layout auto-calculation if not specified

---

*Last updated: Briv v10.2 (latest)*
*Supersedes: METROPOLITAN_FFI_V2.md, METROPOLITAN_FFI_V2_ENHANCEMENTS.md*