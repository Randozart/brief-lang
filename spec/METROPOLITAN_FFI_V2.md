# Metropolitan FFI v2: The Metro Architecture

## "Pipes"

Brief v10 introduces **Metropolitan FFI v2** - the Metro architecture. Just as subway systems move people between stations through pipes, this FFI moves data between languages through memory pipes. No conversion, no translation - just raw bytes traveling through validated tunnels.

## The Core Metaphor

```
┌─────────────────────────────────────────────────────────────┐
│                      THE METRO                           │
│  ┌─────────┐    ╔═══════════╗    ┌─────────┐           │
│  │ Brief  │───▶║  PIPE    ╞───▶│ Foreign│           │
│  │ Station│    ║  (Buffer)║    │ Station│           │
│  └─────────┘    ╚═══════════╝    └─────────┘           │
│        │              ▲              │                  │
│        │              │              │                  │
│   BOUNDS          CONTRACT        WRITE                  │
│  VALIDATION      TUNNEL          SPOT                  │
└─────────────────────────────────────────────────────────────┘
```

- **Brief Station**: Where data enters (validated bounds)
- **The Pipe**: Shared buffer - Brief allocates, foreign uses
- **The Tunnel**: Contract validation between drop and fetch
- **Foreign Station**: Where data exits (writes to provided spot)

---

## 1. The ABI Problem Solved

### The Fundamental Question

> How does Brief know where the foreign language put the data?

### The Metro Answer

**Brief doesn't ask - Brief tells.** 

Brief allocates the buffer and provides the memory address. The foreign language writes to that exact spot. No ambiguity, no magic, no mysterious memory management.

### The Three Handshake Patterns

| Pattern | Description | Use Case |
|---------|-------------|---------|
| **Stack Register** | Data in fixed CPU registers | Embedded, low-latency |
| **Shared Buffer** | Brief allocates, passes address | Most cases |
| **Opaque Handle** | Foreign returns ID, Brief passes it back | Async, streaming |

---

## 2. Core Concepts

### 2.1 The Data Pipe (Zero-Copy Marshalling)

> It's all just bits. A string can be read as an integer if you interpret its bytes that way.

The pipe doesn't care about types - it cares about:
- **Bit-width**: How many bytes?
- **Bounds**: Input valid range
- **Shape**: Output layout (contiguous bytes)

```brief
// This works because both Float and Int are 64-bit (8 bytes)
// The pipe just moves 8 bytes through
frgn calculate(x: Float) -> Result<Float, MathError> from "math.toml";

// This ALSO works - data is bits!
// The contract enforces: input > 0, output = sqrt(input)
frgn sqrt_int(n: Int) -> Result<Int, MathError> from "math.toml";
```

### 2.2 Memory Layouts Over Types

Instead of `String -> String`, think:

```toml
# Input: 8 bytes starting at offset 0
[functions.input_layout]
param = { offset = 0, size = 8 }

# Output: 8 bytes starting at offset 0  
[functions.output_layout]
result = { offset = 0, size = 8 }
```

### 2.3 The Sentinel

Between **Drop** (write to buffer) and **Fetch** (read result), the Sentinel validates:

```
Input → [DROP] → Buffer → [FOREIGN] → Buffer → [SENTINEL] → Output
                     1. Bounds      2. Write      3. Contract
                     Check                            Check
```

If foreign writes invalid data, Sentinel panics before Brief ever sees it.

---

## 3. Memory Orchestrator

The Metro system has a **Memory Orchestrator** - Brief controls memory, not the foreign language.

### 3.1 Orchestration Flow

```
1. LAYOUT    → Calculate contiguous byte layout from fields
2. ALLOCATE  → Brief allocates buffer (stack or heap)
3. DROP      → Brief writes input to buffer at offsets
4. VALIDATE  → Sentinel checks input bounds pre-call
5. PIPE      → Pass buffer address to foreign function
6. WRITE     → Foreign writes result to provided spot
7. SENTINEL  → Validate output contract post-call
8. FETCH     → Brief reads result from buffer
```

### 3.2 Buffer Allocation Modes

```toml
[meta]
# Buffer allocation mode
buffer_mode = "stack"    # Fast, stack-allocated (recommended)
# buffer_mode = "heap"    # For large data, explicit free
# buffer_mode = "static"   # For DMA, fixed addresses
```

---

## 4. Mapper Plugin System

> Brief is the Landlord; Mappers are the Floor Plans.

### 4.1 Mapper Interface

Mappers are Rust `.so` / `.dll` plugins implementing:

```rust
pub trait Mapper: Send + Sync {
    /// How to write to the pipe
    fn drop(&self, buffer: &mut [u8], layout: &MemoryLayout, data: &[u8]) -> usize;

    /// How to read from the pipe  
    fn fetch(&self, buffer: &[u8], layout: &MemoryLayout) -> Vec<u8>;

    /// Validate output matches contract
    fn validate(&self, data: &[u8], contract: &str) -> bool;
}
```

### 4.2 Mapper Directory Structure

```
lib/mappers/
├── rust/           # Rust/Native mapper
├── wasm/          # WebAssembly mapper  
├── c/             # C ABI mapper
├── python/         # Python bridge mapper
├── fpga/          # Memory-mapped FPGA mapper
└── custom/        # User-provided mappers
```

### 4.3 Using Custom Mappers

```toml
[[functions]]
name = "chip_write"
mapper = "/path/to/custom_fpga_mapper.so"
path = "lib/mappers/fpga/chip_mapper.so"

[functions.input]
address = "Int"
value = "Int"
```

---

## 5. TOML v2 Specification

### 5.1 Full Structure

```toml
# Metropolitan FFI v2 Binding
# ========================
[meta]
name = "chip_driver"
version = "2.0.0"
description = "FPGA chip driver via memory-mapped I/O"

# Buffer orchestration
buffer_mode = "static"        # stack | heap | static
buffer_size = 256         # Explicit size for static buffers
alignment = 8             # Byte alignment

# Mappers
default_mapper = "fpga"
# optional_path = "/path/to/mapper.so"

# Validations
pre_validate = "input.address >= 0x4000 && input.address < 0x8000"
post_validate = "result == 0 || result == 0xFF"

# ============================================================================
# FUNCTION: write_register
# ============================================================================
[[functions]]
name = "write_register"
location = "0x4000"           # Memory-mapped address
mapper = "fpga"
description = "Write to FPGA register"

# Input shape (auto-layout if input_layout omitted)
[functions.input]
address = "Int"                 # 4 bytes
value = "Int"                   # 4 bytes

# Explicit input layout (NEW v2)
[functions.input_layout]
address = { offset = 0, size = 4 }
value = { offset = 4, size = 4 }

# Output shape
[functions.output.success]
result = "Int"

# Output layout
[functions.output_layout]
result = { offset = 0, size = 4 }

# Native implementation (Rust)
[functions.native]
location = "chip_driver::write_register"
import = "use chip_driver::write_register;"

# WASM implementation  
[functions.wasm]
target = "wasm"
wasm_impl = """
function write_register(addr, val) {
    // Memory-mapped I/O in WASM
    HEAPU32[(addr >> 2)] = val;
    return 0;
}
"""
```

### 5.2 Field Descriptors

```toml
# Simple form - type name (auto-calculates size)
[functions.input]
x = "Float"
y = "Float"

# Explicit form - full layout spec (NEW v2)
[functions.input_layout]
x = { offset = 0, size = 8 }   # bytes 0-7
y = { offset = 8, size = 8 }  # bytes 8-15

# Array support
[functions.input]
data = "Data"

[functions.input_layout]
data = { offset = 0, size = 64, element_size = 8, count = 8 }
```

### 5.4 Endianness Handling

```toml
# Global endianness (applies to all fields)
[meta]
endian = "little"   # native | little | big

# Per-field override
[functions.input_layout]
value = { offset = 0, size = 8, endian = "big" }
```

| Endianness | Description |
|-----------|-------------|
| `native` | No conversion (default) |
| `little` | Force little-endian byte order |
| `big` | Force big-endian byte order |

**When to specify:**
- Desktop ↔ Desktop (same arch): `native`
- x86 ↔ FPGA/MIPS/PowerPC: `big` often needed
- Network protocols: `big` (network byte order)

**Mapper responsibility:** The mapper performs the byte-swap if:
1. `endian` is set to something other than `native`
2. AND the target platform differs from the host

```rust
// Mapper performs conversion in `drop()` and `fetch()`
fn maybe_swap_endian(bytes: &mut [u8], target_endian: Endian) {
    if target_endian != host_endian() && bytes.len() > 1 {
        bytes.reverse();  // Simple byte-swap for >1 byte values
    }
}
```

### 5.5 Contract Specification

```toml
# Pre-condition (validated BEFORE foreign call)
[functions.contract]
precondition = "x >= 0.0 && y >= 0.0"
precondition_param = "x > 0"    # Can reference params

# Post-condition (validated AFTER foreign call)
[functions.contract]
postcondition = "result >= 0.0"
success_field = "result"             # Which field to validate
error_if_fail = "MathError"        # Which error type

# Bound checking (NEW v2)
[functions.contract]
bound_min = 0.0
bound_max = 1e10
```

---

## 6. Breaking: The New `frgn` Syntax

### v2 Declaration

```brief
# v2 style - with explicit contract
frgn calculate(x: Float, y: Float) 
    -> Result<Float, MathError> 
    pre [x > 0 && y > 0]
    post [result >= 0]
    from "math.toml";
```

### v1 Backward Compatibility

```brief
# v1 style - still works
frgn calculate(x: Float, y: Float) -> Result<Float, MathError> from "math.toml";
```

The compiler auto-generates:
- `pre [true]` if no precondition
- `post [true]` if no postcondition

---

## 7. Memory Layout Engine

### 7.1 Layout Calculation

```rust
pub struct MemoryLayout {
    pub size_bytes: usize,
    pub alignment: usize,
    pub fields: Vec<FieldDescriptor>,
}

pub struct FieldDescriptor {
    pub name: String,
    pub offset: usize,
    pub size_bytes: usize,
    pub element_size: Option<usize>,  // For arrays
    pub count: Option<usize>,         // For arrays
}
```

### 7.2 Auto-Layout Algorithm

```
1. Sort fields by size descending (largest first)
2. For each field:
   a. Calculate alignment padding from current offset
   b. Set field.offset = current_offset + padding
   c. current_offset += field.size
3. total_size = round_up to alignment boundary
```

### 7.3 Explicit Override

```toml
# Explicit overrides auto-calculation
[functions.input_layout]
x = { offset = 16, size = 8 }   # Skip to byte 16
y = { offset = 24, size = 8 }   # Continue  
```

---

## 8. Implementation Architecture

### 8.1 Core Components

```
src/ffi/
├── types.rs           # MemoryLayout, FieldDescriptor (ADD)
├── loader.rs         # Parse v2 TOML, calculate layouts
├── mapper.rs        # Load dynamic mappers
├── mappers.rs      # Mapper registry
├── protocol.rs     # Mapper trait definition (NEW)
├── sentinel.rs     # Contract validation (NEW)
└── orchestrator.rs # Buffer orchestration (NEW)
```

### 8.2 Pipeline Integration

```rust
pub struct FfiPipeline {
    layout_engine: LayoutEngine,
    mapper_registry: MapperRegistry,
    sentinel: Sentinel,
}

impl FfiPipeline {
    pub fn call(&self, binding: &ForeignBinding, args: &[Value]) -> Value {
        // 1. Calculate layouts
        let input_layout = self.layout_engine.input_layout(binding);
        let output_layout = self.layout_engine.output_layout(binding);
        
        // 2. Allocate buffer
        let mut buffer = self.orchestrator.allocate(
            input_layout.size_bytes.max(output_layout.size_bytes)
        );
        
        // 3. Drop input
        self.mapper.drop(&mut buffer, &input_layout, args);
        
        // 4. Validate pre-conditions
        self.sentinel.validate_input(&buffer, binding);
        
        // 5. Call foreign
        let result = self.call_foreign(binding, buffer.as_mut_ptr());
        
        // 6. Validate post-conditions
        self.sentinel.validate_output(&buffer, binding);
        
        // 7. Fetch result
        self.mapper.fetch(&buffer, &output_layout)
    }
}
```

---

## 9. Hardware / FPGA Support

### 9.1 Memory-Mapped I/O

For FPGA, the pipe is physical addresses:

```toml
[functions.input_layout]
data = { offset = 0, size = 64 }   # Write to address 0x4000
enable = { offset = 64, size = 1 } # Write enable bit

[functions.output_layout]
ready = { offset = 0, size = 1 }   # Status bit at 0x4008
```

### 9.2 MMIO Mapper

```rust
// FPGA mapper writes to actual memory addresses
impl Mapper for FpgaMapper {
    fn drop(&self, buffer: &mut [u8], layout: &MemoryLayout, data: &[u8]) -> usize {
        let base_addr = self.base_address;
        for (i, &byte) in data.iter().enumerate() {
            self.write_mmio(base_addr + layout.fields[0].offset + i, byte);
        }
        data.len()
    }
}
```

---

## 10. Changes from v1

| Aspect | v1 | v2 |
|--------|----|----|
| **Focus** | Type conversion | Memory layouts |
| **Data** | Typed | Raw bits |
| **Mapper** | Hardcoded | Pluggable `.so` |
| **Validation** | Type checking | Contract checking |
| **Buffer** | Per-call | Orchestrated |
| **Hardware** | Not supported | MMIO native |

---

## 11. Migration Guide

### v1 TOML → v2 TOML

```toml
# v1
[[functions]]
name = "sin"
location = "std::f64::sin"

[functions.input]
x = "Float"

[functions.output]
result = "Float"

# v2 (equivalent)
[[functions]]
name = "sin"
location = "std::f64::sin"
buffer_mode = "stack"

[functions.input]
x = "Float"

[functions.input_layout]
x = { offset = 0, size = 8 }

[functions.output]
result = "Float"

[functions.output_layout]
result = { offset = 0, size = 8 }
```

### Auto-Upgrade

The compiler will auto-convert v1 to v2:
- Add `buffer_mode = "stack"`
- Calculate `input_layout` from types
- Calculate `output_layout` from types  
- Add `pre [true]`, `post [true]` if missing

---

## 12. Why "Metro"?

> Just as subway stations connect through pipes, languages connect through buffers.

The Metro architecture treats FFI like infrastructure:
- Brief builds the stations (validation, contracts)
- Mappers define the routes (memory layouts)
- Data flows through pipes (buffers)
- The Sentinel guards the tunnels (contracts)

No magic. No confusion. Just bits moving through validated tunnels from one station to another.

---

## 13. Examples

### 13.1 Simple Math

```toml
[[functions]]
name = "sqrt"
location = "std::f64::sqrt"
mapper = "rust"

[functions.input]
x = "Float"

[functions.input_layout]
x = { offset = 0, size = 8 }

[functions.output]
result = "Float"

[functions.output_layout]
result = { offset = 0, size = 8 }

[functions.contract]
precondition = "x >= 0.0"
postcondition = "result >= 0.0 && result * result <= x + 0.0001"
```

```brief
frgn sqrt(x: Float) 
    -> Result<Float, MathError>
    pre [x >= 0]
    post [result >= 0]
    from "std/bindings/math.toml";
```

### 13.2 FPGA Register Write

```toml
# chip_driver.toml
[meta]
name = "chip_driver"
buffer_mode = "static"
default_mapper = "fpga"
static_address = 0x4000_0000

[[functions]]
name = "write_reg"
location = "0x4000"

[functions.input]
address = "Int"
value = "Int"

[functions.input_layout]
address = { offset = 0, size = 4 }
value = { offset = 4, size = 4 }

[functions.output]
result = "Int"

[functions.output_layout]
result = { offset = 0, size = 4 }

[functions.contract]
precondition = "address >= 0x4000 && address < 0x8000"
```

### 13.3 Custom Mapper

```toml
# Using custom quantum computer mapper
[[functions]]
name = "quantum_gate"
mapper = "quantum"
path = "/opt/quantum/mappers/gate_mapper.so"

[functions.input]
qubits = "Int"
gate = "Int"
params = "Data"

[functions.input_layout]
qubits = { offset = 0, size = 4 }
gate = { offset = 4, size = 4 }
params = { offset = 8, size = 64 }

[functions.output]
result = "Int"

[functions.output_layout]
result = { offset = 0, size = 4 }
```

---

## 14. Unified Error Protocol: `Error(type)`

In Metro FFI, all failures are surfaced through a unified error container. While the TOML defines specific fields for each error neighborhood (e.g., `MathError`, `IoError`), Brief treats them as variants of a universal `Error` type.

### 14.1 The Error Container

When an FFI call fails, it returns a `Result::Error` containing an `ErrorInstance`:

```brief
// In Brief logic
let result = sqrt(-1.0); 
// result is Error(MathError { code: 1, message: "Negative input" })
```

### 14.2 TOML Error Definition

```toml
[functions.output.error]
type = "IoError"         # The variant name
code = "Int"            # Standard field
message = "String"      # Standard field
path = "String"         # Neighborhood-specific field
```

### 14.3 Automatic Wrapping

The Orchestrator automatically wraps raw foreign exit codes into the Brief `Error` type:
1. **Foreign Call** returns non-zero or error bit.
2. **Mapper** fetches the error fields from the designated "Error Spot" in the pipe.
3. **Sentinel** constructs the `Error(Variant { ... })` object.

This ensures that regardless of the language or hardware, error handling in Brief remains consistent: `Error(type)`.

---

## Appendix A: Quick Reference

### TOML Fields

| Field | Type | Description |
|-------|------|-------------|
| `meta.buffer_mode` | string | stack/heap/static |
| `meta.endian` | string | native/little/big (global) |
| `meta.default_mapper` | string | Mapper name |
| `functions.input_layout` | table | Explicit input offsets |
| `functions.input_layout.*.endian` | string | Per-field endian override |
| `functions.output_layout` | table | Explicit output offsets |
| `functions.contract.precondition` | string | Pre-call validation |
| `functions.contract.postcondition` | string | Post-call validation |

### Mapper Protocol

```rust
trait Mapper {
    fn drop(&self, buffer: &mut [u8], layout: &MemoryLayout, data: &[u8]) -> usize;
    fn fetch(&self, buffer: &[u8], layout: &MemoryLayout) -> Vec<u8>;
    fn validate(&self, data: &[u8], contract: &str) -> bool;
}
```

### Memory Layout Auto-Size

| Brief Type | Size (bytes) |
|------------|--------------|
| Bool | 1 |
| Int | 8 |
| Float | 8 |
| String | 8 (pointer) |
| Data | 8 (pointer) |

### Endianness Reference

| Setting | Meaning |
|---------|---------|
| `native` | No conversion needed (default) |
| `little` | x86, ARM (little-endian) |
| `big` | Network order, MIPS, PowerPC, FPGA |

---

## Appendix B: Error Codes

| Error | Description | Cause |
|-------|-------------|-------|
| `E_MAPPER_NOT_FOUND` | Mapper not loaded | Invalid path |
| `E_BUFFER_OVERFLOW` | Data exceeds buffer | Layout mismatch |
| `E_CONTRACT_VIOLATION` | Pre/post check failed | Invalid input/output |
| `E_LAYOUT_MISMATCH` | Input/output offset conflict | Bad TOML |
| `E_ALIGNMENT_ERROR` | Unaligned access | Platform requires alignment |
| `E_ENDIAN_MISMATCH` | Endian conversion failed | Invalid byte-swap |

---

## Appendix C: Version History

- **v1 (Brief 8.5-9.x)**: Type-based, naive conversion
- **v2 (Brief 10+)**: Layout-based, memory orchestration, pluggable mappers

---

*Metropolitan FFI v2: The Metro*
*Just pipes. Just bits. Justvalidated tunnels.*