# Embedded Brief 2.0 Specification

**Version:** 2.0  
**Date:** 2026-04-12  
**Status:** Design Specification - High-Velocity Hardware  

---

## 1. Introduction

### 1.1 Purpose

Embedded Brief 2.0 extends Embedded Brief 1.0 with capabilities for high-velocity hardware (GPUs, NPUs, vector processors, FPGAs). It maintains the same contract-first philosophy while adding vector operations, cycle floor semantics, and hardware-aware compilation.

### 1.2 Relationship to 1.0

Embedded Brief 2.0 is an **extension layer**, not a replacement. It shares:
- The same TOML memory model
- The same trigger (`trg`) and address (`@`) syntax
- The same transaction and contract model
- The same `no_std` compilation target

### 1.3 New in 2.0

| Feature | Description |
|---------|------------|
| Vector Types | `Int[32]`, `UInt[64]`, `Bool[8]` - wide registers |
| Floor Cycles | `[within N cycles]` - guaranteed N or fewer |
| Memory Banking | Explicit bank declarations |
| Pipeline Stages | Conceptual pipeline definition |
| Arbitrary Widths | Extended bit syntax (`/xN`) |

---

## 2. Type System

### 2.1 Scalar Types (from 1.0)

| Type | Aliases | Description |
|------|---------|-------------|
| `Int` | `Signed`, `Sgn` | Signed integer |
| `UInt` | `Unsigned`, `USgn` | Unsigned integer |
| `Bool` | - | Boolean (single bit) |

### 2.2 Vector Types (NEW)

Vector types represent wide registers processed simultaneously:

```brief
let pixels: Int[100];        // 100-element signed vector
let weights: UInt[64];      // 64-element unsigned vector  
let masks: Bool[8];        // 8-element boolean vector
```

**Syntax:**
```
vector_type ::= base_type "[" size "]"
base_type  ::= "Int" | "UInt" | "Bool"
size      ::= integer
```

**Characteristics:**
- Processed as SIMD (Single Instruction, Multiple Data)
- All elements processed in one clock cycle
- Contract applies to **all elements** simultaneously

```brief
// Contract applies to EVERY element in the vector
txn normalize [true][output[i] >= 0] {
    // Must ensure ALL elements >= 0
    &output = input;
    term;
};
```

### 2.3 Bit Range Syntax (REFINED)

The original 1.0 syntax has been refined for clarity:

| Syntax | Meaning | Example |
|--------|---------|---------|
| `/N` | Single bit at position N | `/8` = bit 8 only |
| `/M..N` | Bit range from M to N | `/0..8` = bits 0 through 8 |
| `/xN` | Any N-bit range (compiler finds slot) | `/x8` = any 8-bit slot |

**Important:** All allocations are bit-aligned. "Values" don't make sense because you cannot share half a bit between variables.

```brief
let flag: Bool / 7;         // Single bit at position 7
let flags: UInt / 0..3;     // Bits 0, 1, 2, 3 (4 bits)
let buffer: UInt / x8;      // Compiler finds any 8-bit slot
```

---

## 3. Memory Model

### 3.1 TOML Configuration (from 1.0)

```toml
[chip]
name = "STM32F405"
architecture = "ARM Cortex-M4"

[memory.ram]
start = "0x20000000"
size = "128KB"

[memory.flash]
start = "0x08000000"
size = "1MB"

[[memory.reserved]]
start = "0x40000000"
size = "128KB"
name = "Peripherals"
```

### 3.2 Memory Banking (NEW)

For high-velocity hardware, memory banks prevent conflicts:

```toml
[memory.bank0]
start = "0x20000000"
size = "32KB"
lanes = 1

[memory.bank1]
start = "0x20008000"
size = "32KB"
lanes = 1
```

**Bank Declaration:**
```brief
let pixel_buffer: UInt[100] @ 0x20000000 bank0;
let weight_buffer: UInt[64] @ 0x20008000 bank1;
```

**Compiler Rules:**
- Variables in different banks can be accessed simultaneously
- Variables in the same bank cause conflict unless sequentialized
- Compiler can rearrange allocation to minimize conflicts

### 3.3 Memory Hierarchy

Different memory types have different latency:

| Type | Latency | Use Case |
|------|--------|----------|
| Register | 0 cycles | Immediate values |
| Scratchpad | 1 cycle | Working data |
| VRAM | 100+ cycles | Bulk storage |

```brief
let register_value: Int @ reg0;           // 0-cycle register
let scratch_value: Int @ /scratch;       // 1-cycle scratchpad
let vram_value: Int @ 0x8000000;       // External VRAM
```

---

## 4. Execution Model

### 4.1 Reactor (from 1.0)

Static interrupt-driven execution with WFI idle:

```
1. Initialize (static memory layout)
2. Enable hardware interrupts  
3. Loop: check triggers → evaluate preconditions → execute transactions → WFI
```

### 4.2 Vector Processing (NEW)

Vector transactions process all elements simultaneously:

```brief
// Process 100 pixels - single instruction, 100 ALUs in parallel
txn apply_filter [input.len > 0][output.len > 0 within 4 cycles] {
    // This applies to ALL 100 elements in ONE cycle
    &output = input * 2;
    term;
};
```

**When compiled:**
- Generates SIMD instruction or parallel gates
- All vector elements processed simultaneously
- Single contract evaluation for entire vector

### 4.3 Floor Cycle Semantics (NEW)

The `[within N cycles]` clause is a **floor guarantee**:

**Meaning:** "This transaction is guaranteed to complete in N or fewer clock cycles."

```brief
txn ddr_read [true][data_valid == true within 20 cycles] {
    &data_valid = true;
    term;
};
```

**Compiler Analysis:**
- Reads TOML for hardware constraints
- Checks if N is >= minimum achievable cycles
- Errors if floor is impossible for target hardware

**What it CAN verify:**
- DDR minimum latency (from TOML)
- Register-to-register operations (1 cycle floor)
- Pipeline depth limits

**What it CANNOT verify:**
- Exact cycles (depends on voltage, temperature, silicon variation)
- Runtime behavior (cache hits, branch misprediction)

**Floor vs. Exact:**
- `[within 4 cycles]` = "4 or fewer" (guaranteed floor)
- NOT "exactly 4 cycles"

---

## 5. Transactions and Contracts

### 5.1 Transaction Types (from 1.0)

```brief
// Passive transaction
txn init [true][initialized == true] {
    term;
};

// Reactive transaction
rct txn on_trigger [trigger == true][trigger == false] {
    term;
};
```

### 5.2 Vector Contracts (NEW)

Vector contracts apply to all elements:

```brief
// ALL elements must be positive
txn normalize [true][forall i: output[i] >= 0] {
    term;
};

// EXISTS element meets condition  
txn check_overflow [true][exists i: output[i] > 255] {
    &overflow = true;
    term;
};
```

**Quantifiers:**
- `forall` - all elements must satisfy
- `exists` - at least one element satisfies

### 5.3 Temporal Contracts

```brief
// Floor guarantee: completes in N or fewer cycles
txn process_frame [input_ready][frame_ready within 100 cycles] {
    &frame_ready = true;
    term;
};
```

---

## 6. Pipeline Concepts

### 6.1 Pipeline Stages (NEW)

Brief can define conceptual pipeline stages:

```brief
// Stage definitions
stage fetch [true][data_fetched == true];
stage compute [data_fetched][result_ready == true];
stage write [result_ready][write_complete == true];
```

**Usage:**
```brief
txn pipeline_fetch [true][data_fetched == true] {
    &data_fetched = true;
    term;
};

rct txn pipeline_compute [data_fetched][result_ready == true] {
    &result_ready = true;
    term;
};
```

**Compiler analysis:**
- Detects dependencies between stages
- Ensures proper ordering
- Can report if pipeline stalls (stage waiting on upstream)

### 6.2 Throughput Semantics

```brief
// Throughput: X items per second
txn render_frame [render_buffer_full == false][frames_rendered == frames_rendered + 1] 
    throughput(60);
```

This is documentation - compiler generates comments but cannot enforce.

---

## 7. Hardware Integration

### 7.1 Triggers (from 1.0)

```brief
trg button: Bool @ 0x40020010 / 0;
trg sensor: Int @ 0x40020014 / 0..13;
```

**trg vs let @:**
- `trg` = hardware input (read-only)
- `let @ address` = hardware output (writable)

### 7.2 Vector Triggers (NEW)

```brief
// 8 simultaneous sensor readings
trg sensors: Int[8] @ 0x40020000 / 0..31;
```

### 7.3 DMA Ownership (from 1.0)

```brief
let rx_buffer: UInt[256];

on dma_complete {
    term;
};
```

---

## 8. Reference

### 8.1 Syntax Summary

| Construct | Syntax | Description |
|-----------|--------|-------------|
| Variable (scalar) | `let x: Int @ address / range;` | Scalar variable |
| Variable (vector) | `let x: Int[100] @ address;` | Vector variable |
| Trigger | `trg x: Bool @ address / bit;` | Hardware input |
| Vector trigger | `trg x: Int[8] @ address / range;` | Hardware vector input |
| Override | `!@ address` | Bypass reserved check |
| Bit (single) | `/N` | Bit N |
| Bit range | `/M..N` | Bits M through N |
| Bit (any) | `/xN` | Any N-bit slot |
| Reactive | `rct txn name [pre][post] { ... }` | Interrupt-driven |
| Floor cycles | `[post within N cycles]` | N or fewer cycles |
| Vector all | `forall i:` | For all elements |
| Vector exists | `exists i:` | Exists element |

### 8.2 Complete Example: Vector Processor

```brief
// memory.toml required:
// [chip] name = "VectorGPU"
// [memory.bank0] start = "0x00000000", size = "32KB"
// [memory.bank1] start = "0x00008000", size = "32KB"

// INPUT: 8 ADCs reading simultaneously
trg adc_inputs: Int[8] @ 0x40000000 / 0..31;

// OUTPUT: 8 DACs
let dac_outputs: Int[8] @ 0x40010000 bank1;

// Software state
let filter_active: Bool = false;
let sample_count: Int = 0;

// Initialize
txn init [true][filter_active == false] {
    &filter_active = false;
    term;
};

// Reactive: sample all 8 ADCs when trigger fires
// ALL 8 samples captured in ONE cycle
rct txn sample_adc [adc_ready == true][sample_count == sample_count + 8 within 2 cycles] {
    &sample_count = sample_count + 8;
    term;
};

// Vector operation: apply filter to all 8 channels
// SINGLE instruction, 8 parallel ALUs
txn apply_filter [filter_active][forall i: output[i] >= 0 within 4 cycles] {
    &output = input * 2;
    term;
};

// Reactive: write to DACs when ready
rct txn write_dac [result_ready][write_complete == true within 3 cycles] {
    term;
};
```

### 8.3 Vector Types Full Example

```brief
// 100-element signed vector
let pixels: Int[100] @ 0x20000000 bank0;

// 64-element unsigned vector  
let weights: UInt[64] @ 0x20000100 bank0;

// 8-element boolean mask
let mask: Bool[8] @ 0x20000200 bank1;

// Apply weights to pixels (single SIMD operation)
txn apply_weights [true][result[i] >= 0 within 4 cycles] {
    // Multiplies each of 100 pixels by corresponding weight
    &result = pixels * weights;
    term;
};
```

---

## 9. Grammar (BNF)

```bnf
program ::= (variable_decl | trg_decl | txn_def | struct_def | enum_def | constant | import)*

variable_decl ::= "let" identifier ":" type ( ( "@" address ( "/" range )? ) | "@/" range? ) ";"
trg_decl       ::= "trg" identifier ":" type "@" address "/" range ";"
address       ::= hex_literal
range         ::= bit_range | any_bit
bit_range    ::= number ".." number | number
any_bit      ::= "x" number

type          ::= "Bool" | "Int" | "UInt" | "Signed" | "Unsigned" | vector_type
vector_type  ::= base_type "[" size "]"
base_type    ::= "Bool" | "Int" | "UInt"

txn_def       ::= ("rct")? "txn" identifier parameters? contract ("{" body "}" ";" | ";")
contract     ::= "[" expression "]" "[" expression ( "within" number "cycles" )? "]"

parameters   ::= "(" (param ("," param)*)? ")"
param       ::= identifier ":" type

constant    ::= "const" identifier ":" type "=" expression ";"
struct_def  ::= "struct" identifier "{" struct_member* "}"
enum_def    ::= "enum" identifier "{" identifier ("," identifier)* "}"
```

---

## 10. Reserved Keywords

```
let, const, trg, txn, rct, term, escape, stage, forall, exists
Bool, Int, UInt, Signed, Unsigned, true, false
struct, enum, import, from, within, cycles
@, !@, /
```

---

## 11. Differences from 1.0

| Feature | 1.0 | 2.0 |
|--------|-----|-----|
| Types | Int, UInt, Bool | Added: Int[n], UInt[n], Bool[n] |
| Cycles | Not supported | Floor semantics: `[within N cycles]` |
| Banking | Not supported | Explicit bank declarations |
| Pipeline | Not supported | Stage definitions |
| Quantifiers | Not supported | forall, exists |

---

## 12. Out of Scope

- Exact cycle verification (impossible to guarantee)
- Dynamic memory allocation (heap)
- Floating-point operations (via FFI)
- Multi-threading beyond pipeline stages
- Network/USB stacks (via FFI)

---

*End of Specification*