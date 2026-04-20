# Metropolitan FFI v2.1 + Syscall + Resource Enhancements

## Overview

This document describes the enhancements to the Metropolitan FFI system in Brief v10+, adding fire-and-forget FFI calls, direct syscall support, and an extensible resource system for OS/kernel resource negotiation.

---

## Part 1: FFI Return Type Taxonomy

### The Four Variants

| Keyword | Return Type | Error Handling | Use Case |
|---------|-------------|---------------|----------|
| `frgn` | `Result<T, Error>` | Must handle (transaction can escape) | Foreign functions that return values |
| `frgn!` | `void` | No return, fire-and-forget | Side-effect-only FFI calls |
| `syscall` | `Result<Int, Error>` | Must handle | Kernel calls returning file descriptors/ints |
| `syscall!` | `void` | No return | Kernel calls with no meaningful return |

### Syntax Examples

```brief
// Standard FFI - returns Result, must handle errors
frgn calculate(x: Float) -> Result<Float, MathError> from "math.toml";

// Fire-and-forget - executes and returns void
frgn! println(msg: String);

// Kernel syscall with return (must handle errors)  
syscall open(path: String, flags: Int) -> Result<Int, Error>;

// Kernel syscall void (fire-and-forget)
syscall! munmap(addr: Int, len: Int);
```

### Implementation Note

- `syscall` return type `Result<Int, Error>` wraps the raw syscall return value
- Errors are determined per-target (negative return typically means error on most syscalls)
- The `!` suffix means "ignore any return value" - compiler skips Result wrapping

---

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

**.ebv (Embedded Target)**:
- `@address` is absolute physical address
- No heap - all memory is static/pool-based
- No bounds checking (programmer responsible)

**.rbv (Browser Target)**:
- `@address` is WASM memory offset
- All memory is linear memory - no stack/heap distinction

### Declarative Structs with Bit Packing

```brief
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

The compiler handles the layout - programmer declares intent, compiler figures addresses.

---

## Part 3: Resource System

### The `rsrc` / `resource` Keyword

Resources declare data that comes from OS/kernel negotiation:

```brief
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

### Resource Acquisition Protocol

```
1. Declare resource in Brief
       ↓
2. Compiler generates kernel calls (open → ioctl → mmap)
       ↓
3. Runtime negotiates with kernel
       ↓
4. Address returned and bound to variable
       ↓
5. Brief logic uses @address to access
```

### Resource Struct Form

```brief
// Type with constructor-style args
rsrc framebuffer: FrameBuffer(width: 320, height: 200);

// Usage
txn draw [tick][true] {
    &framebuffer = compute_frame();
    term;
}
```

---

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
windows_x64    │ win32io.h
```

### Syscall Binding Example (TOML)

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

```brief
// Raw syscall returns Int (file descriptor)
syscall open(path: String, flags: Int) -> Result<Int, Error>;

// Or wrapped in type (future enhancement)
syscall open(path: String, flags: Int) -> Result<FileDescriptor, Error>;
```

---

## Part 5: Memory Architecture Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    BRIEF MEMORY MODELS                       │
├────────────────┬──────────────────────────────────────────┤
│   .ebv          │           .bv / .rbv                     │
│  (Embedded)     │        (OS / Browser)                   │
├────────────────┼──────────────────────────────────────────┤
│ @ = raw        │ @ = virtual offset                     │
│ No heap        │ Compiler manages heap/stack           │
│ Static memory  │ Escape analysis → stack if local       │
│ Fixed addrs    │ Static if permanent, mmap if transitory│
│ No OS calls    │ rsrc negotiates with kernel            │
└────────────────┴──────────────────────────────────────────┘
```

---

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

---

## Implementation Checklist

### Parser
- [ ] Add `frgn!` keyword parsing (fire-and-forget)
- [ ] Add `syscall` keyword parsing
- [ ] Add `syscall!` keyword parsing  
- [ ] Add `rsrc` / `resource` keyword parsing
- [ ] Add address scoping: `@raw:`, `@stack:`, `@heap:`

### FFI Backend
- [ ] FFI: Skip Result wrapping for `frgn!` (void return)
- [ ] FFI: Error mapping for void returns

### Syscall System
- [ ] Target-specific syscall number tables
- [ ] Syscall binding loader (new TOML section)
- [ ] Direct syscall code generation per target
- [ ] Cache syscall numbers (invalidate on --no-cache or arch change)

### Address System
- [ ] AddressMode enum: Raw, Virtual, Wasm
- [ ] Target detection (.ebv vs .bv vs .rbv)
- [ ] Compiler-based address resolution per target
- [ ] Escape analysis for stack/heap determination

### Resource System
- [ ] Built-in resource types:
  - [ ] FrameBuffer
  - [ ] File
  - [ ] SharedMemory
  - [ ] Socket
  - [ ] EventFD
- [ ] Resource struct parsing
- [ ] Kernel negotiation code generation
- [ ] Resource lifecycle management

### Memory Layout
- [ ] Bit-packed struct support
- [ ] Automated offset resolution
- [ ] Bounds verification for indexed access

---

## Migration from v1

Existing code continues to work:

```brief
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

## Appendix A: File Type Summary

| Extension | Target | @ Behavior | Resources |
|-----------|--------|-----------|-----------|
| `.ebv` | Embedded | Raw addresses | Static memory pools |
| `.bv` | OS | Virtual offsets | rsrc negotiates with kernel |
| `.rbv` | Browser | WASM offsets | WASM linear memory |

---

## Appendix B: Quick Reference

```brief
// ===== FFI =====
frgn sin(x: Float) -> Result<Float, MathError> from "math.toml";
frgn! log(msg: String);

// ===== SYSCALL =====
syscall open(path: String, flags: Int) -> Result<Int, Error>;
syscall! munmap(addr, len);

// ===== RESOURCES =====
rsrc screen: FrameBuffer(320, 200);
resource audio: SharedMemory("audio_buf", 4096);

// ===== ADDRESSING =====
@raw:0x40021000 = value;        // Embedded raw
@stack:0x10 = local_var;        // Stack offset  
@heap:ptr + 4 = heap_var;      // Heap offset

// ===== PACKED STRUCTS =====
struct Pixel { r: 4bits, g: 4bits, b: 4bits, a: 4bits }
@screen[index] = pixel;  // Compiler handles layout
```

---

*Last updated: Brief v10 development*
*Supersedes: METROPOLITAN_FFI_V2.md*