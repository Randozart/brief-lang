# Embedded Brief 2.1 Specification

**Version:** 2.1  
**Date:** 2026-04-12  
**Status:** Design Specification - Float Extensions  

---

## 1. Introduction

### 1.1 Purpose

Embedded Brief 2.1 is an incremental update to 2.0, adding native floating-point support while maintaining the contract-first philosophy. This specification is complete and includes all features from 1.0 and 2.0.

### 1.2 Relationship to Previous Versions

| Version | Focus | Status |
|---------|-------|--------|
| 1.0 | Microcontroller/bare-metal | Complete |
| 2.0 | High-velocity (GPU/NPU/FPGA) | Complete |
| 2.1 | Float extensions + FFI presets | This spec |

### 1.3 What's New in 2.1

| Feature | Description |
|---------|------------|
| `Float` | Native floating-point type |
| Float bit ranges | `@/x16`, `@/x32`, `@/x64` |
| Network FFI presets | TOML configurations |
| USB FFI presets | TOML configurations |

### 1.4 What's NOT in 2.1

These were considered and rejected:

| Feature | Reason for Exclusion |
|---------|---------------------|
| UFloat | Doesn't exist - IEEE 754 floats are inherently signed |
| Pool allocation | Unnecessary - `@/N` already provides this |
| Vram keyword | Unnecessary - explicit addresses already work |
| Parallel keyword | Vector types are already inherently parallel |
| Exact cycle verification | Impossible to guarantee at compile time |

---

## 2. Type System

### 2.1 Primitive Types

| Type | Aliases | Description |
|------|---------|-------------|
| `Int` | `Signed`, `Sgn` | Signed integer |
| `UInt` | `Unsigned`, `USgn` | Unsigned integer |
| `Bool` | - | Boolean (single bit) |
| `Float` | - | Floating-point (default 32-bit) |

### 2.2 Vector Types

| Type | Description |
|------|-------------|
| `Int[n]` | n-element signed vector |
| `UInt[n]` | n-element unsigned vector |
| `Bool[n]` | n-element boolean vector |
| `Float[n]` | n-element floating-point vector (NEW) |

```brief
let pixels: Int[100];        // 100-element signed vector
let weights: Float[64];     // 64-element float vector
```

### 2.3 Bit Range Syntax

| Syntax | Meaning | Example |
|--------|---------|---------|
| `/N` | Single bit at position N | `/8` = bit 8 |
| `/M..N` | Bit range from M to N | `/0..8` = bits 0-8 |
| `/xN` | Any N-bit slot (compiler finds) | `/x8` = any 8-bit slot |

### 2.4 Float Type Specification (NEW)

Float types support explicit bit-width specification:

```brief
let temperature: Float;        // Default 32-bit IEEE 754
let sensor: Float @/x16;     // 16-bit half precision
let precise: Float @/x64;    // 64-bit double precision

// Vector floats
let inputs: Float[8] @/x64;  // 8 × 64-bit = 512-bit vector
```

**Float bit ranges:**
| Syntax | IEEE 754 Equivalent | Bits | Decimal Precision |
|--------|-------------------|------|------------------|
| `@/x16` | Half precision | 16 | ~3.3 decimal digits |
| `@/x32` | Single precision | 32 | ~7 decimal digits |
| `@/x64` | Double precision | 64 | ~15 decimal digits |

**Bit size calculation:**
- `Float @/x16` = 16 bits (2 bytes)
- `Float @/x32` = 32 bits (4 bytes)
- `Float @/x64` = 64 bits (8 bytes)

---

## 3. Memory Model

### 3.1 TOML Configuration

```toml
[chip]
name = "VectorGPU"
architecture = "Custom"

[memory.ram]
start = "0x00000000"
size = "256KB"

[memory.vram]
start = "0x10000000"
size = "1GB"

[[memory.bank]]
start = "0x00000000"
size = "64KB"
name = "Bank0"

[[memory.reserved]]
start = "0x40000000"
size = "128KB"
name = "Peripherals"
```

### 3.2 Memory Banking

```brief
let pixel_buffer: Int[100] @ 0x20000000 bank0;
let weight_buffer: UInt[64] @ 0x20010000 bank1;
```

### 3.3 VRAM Explicit

```brief
let gpu_texture: Float[1024] @ vram0;
```

---

## 4. Execution Model

### 4.1 Reactor

```
1. Initialize (static memory layout)
2. Enable hardware interrupts
3. Loop: check triggers → evaluate preconditions → execute transactions → WFI
```

### 4.2 Vector Processing

Vector transactions process all elements simultaneously:

```brief
txn apply_filter [true][forall i: output[i] >= 0 within 4 cycles] {
    &output = input * 2;
    term;
};
```

### 4.3 Floor Cycle Semantics

`[within N cycles]` = "guaranteed to complete in N or fewer cycles"

```brief
txn ddr_read [true][data_valid == true within 20 cycles] {
    &data_valid = true;
    term;
};
```

---

## 5. Transactions and Contracts

### 5.1 Transaction Types

```brief
// Passive
txn init [true][initialized == true] {
    term;
};

// Reactive (interrupt-driven)
rct txn on_trigger [trigger == true][trigger == false] {
    term;
};
```

### 5.2 Contracts

```brief
// Precondition + Postcondition
txn process [pre_condition][post_condition] {
    body;
};

// With floor cycles
txn process [pre][post within 10 cycles] {
    body;
};

// Float precision in contracts
txn calibrate [true][error < 0.01] {
    term;
};
```

### 5.3 Quantifiers for Vectors

```brief
// ALL elements must satisfy
txn normalize [true][forall i: output[i] >= 0] {
    term;
};

// AT LEAST ONE element satisfies
txn check_overflow [true][exists i: output[i] > 255] {
    term;
};
```

---

## 6. Triggers

### 6.1 Scalar Triggers

```brief
trg button: Bool @ 0x40020010 / 0;
trg sensor: Int @ 0x40020014 / 0..13;
```

### 6.2 Vector Triggers

```brief
trg sensors: Int[8] @ 0x40020000 / 0..31;
trg adcInputs: Float[4] @ 0x40021000 / 0..127;
```

---

## 7. FFI and Presets

### 7.1 Float Operations (via FFI)

```brief
defn fmul(a: Float, b: Float) -> Float 
    [true] 
    [result ~= a * b] 
from "embedded::soft_fpu" {};

defn fsqrt(value: Float) -> Float 
    [value > 0] 
    [result * result ~= value] 
from "embedded::soft_fpu" {};
```

### 7.2 Network Presets (NEW)

```toml
# chip_maps/lwip.toml
[pasport.tcp_socket]
target = "embedded"
import = "lwip"

[pasport.tcp_send]
pre = "[socket_open == true]"
post = "[sent == data.len]"
```

### 7.3 USB Presets (NEW)

```toml
# chip_maps/tinyusb.toml
[pasport.usb_init]
target = "embedded"
import = "tinyusb"

[pasport.usb_send]
pre = "[device_ready == true]"
post = "[data_sent == true]"
```

---

## 8. Reference

### 8.1 Syntax Summary

| Construct | Syntax | Description |
|-----------|--------|-------------|
| Variable (scalar) | `let x: Int @ address / range;` | Scalar variable |
| Variable (vector) | `let x: Int[100];` | Vector variable |
| Float (default) | `let x: Float;` | 32-bit float |
| Float (specific) | `let x: Float @/x16;` | 16-bit float |
| Float (vector) | `let x: Float[8];` | 8-element float vector |
| Trigger | `trg x: Bool @ address / bit;` | Hardware input |
| Override | `!@ address` | Bypass reserved check |
| Bit (single) | `/N` | Bit N |
| Bit range | `/M..N` | Bits M through N |
| Bit (any) | `/xN` | Any N-bit slot |
| Reactive | `rct txn name [pre][post] { ... }` | Interrupt-driven |
| Floor cycles | `[post within N cycles]` | N or fewer cycles |
| Vector all | `forall i:` | For all elements |
| Vector exists | `exists i:` | Exists element |

### 8.2 Complete Examples

#### Float Sensor Example

```brief
trg temp_sensor: Float @ 0x40000000 / 0..31;

let temperature: Float = 0.0;
let celsius: Float @/x16 = 0.0;
let fahrenheit: Float @/x16 = 0.0;

// Convert Celsius to Fahrenheit: F = C × 9/32 + 32
txn convert_temp [celsius >= -40 && celsius <= 85] 
    [fahrenheit == celsius * 9.0 / 5.0 + 32.0 within 10 cycles] 
{
    &fahrenheit = celsius * 9.0 / 5.0 + 32.0;
    term;
};

// Reactive: update when sensor changes
rct txn on_temp_change [temperature != @temperature] 
    [celsius == temperature within 5 cycles] 
{
    &celsius = temperature;
    term;
};
```

#### Vector Float Example

```brief
trg adc_inputs: Float[8] @ 0x40020000 / 0..255;

let smoothed: Float[8] @/x256;

// Moving average filter on 8 channels
txn filter [forall i: input[i] >= 0] [forall i: output[i] >= 0 within 8 cycles] {
    // Apply filter to ALL 8 channels simultaneously
    &output = input;
    term;
};
```

#### Network Example

```brief
defn tcp_send(data: Data, len: Int) -> Int 
    [len > 0] 
    [sent == len || error != 0] 
from "lwip" {};

let socket_open: Bool = false;
let connected: Bool = false;

// Initialize network
txn init_network [true][connected == true] {
    &connected = true;
    term;
};

// Send data when connected
txn send_data [connected][sent == true within 100 cycles] {
    term;
};
```

### 8.3 Full Grammar

```bnf
program ::= (variable_decl | trg_decl | txn_def | struct_def | enum_def | constant | import)*

variable_decl ::= "let" identifier ":" type ( ( "@" address ( "/" range )? ) | "@/" range? ) ";"
trg_decl       ::= "trg" identifier ":" type "@" address "/" range ";"
address       ::= hex_literal
range         ::= bit_range | any_bit
bit_range    ::= number ".." number | number
any_bit      ::= "x" number

type          ::= "Bool" | "Int" | "UInt" | "Float" | "Signed" | "Unsigned" | vector_type
vector_type  ::= base_type "[" size "]"
base_type    ::= "Bool" | "Int" | "UInt" | "Float"

txn_def       ::= ("rct")? "txn" identifier parameters? contract ("{" body "}" ";" | ";")
contract     ::= "[" expression "]" "[" expression ( "within" number "cycles" )? "]"

parameters   ::= "(" (param ("," param)*)? ")"
param       ::= identifier ":" type

constant    ::= "const" identifier ":" type "=" expression ";"
struct_def  ::= "struct" identifier "{" struct_member* "}"
enum_def    ::= "enum" identifier "{" identifier ("," identifier)* "}"
```

---

## 9. Reserved Keywords

```
let, const, trg, txn, rct, term, escape, stage, forall, exists
Bool, Int, UInt, Float, Signed, Unsigned, true, false
struct, enum, import, from, within, cycles
@, !@, /
```

---

## 10. Differences Summary

### From 1.0 to 2.1

| Feature | 1.0 | 2.1 |
|--------|-----|-----|
| Integer types | Int, UInt, Bool | ✓ |
| Address binding | @ address | ✓ |
| Bit ranges | /N, /M..N | ✓ |
| Reactive txn | rct | ✓ |
| Temporal contracts | — | ✓ |
| Cycle floors | — | ✓ |
| Vector types | — | ✓ |
| Memory banking | — | ✓ |
| Float | — | ✓ |
| Pipeline stages | — | Added |

---

## 11. Out of Scope

- Exact cycle verification
- Dynamic heap allocation
- Multi-threading beyond vectors
- UFloat (doesn't exist)
- Pool/Vram keywords (unnecessary)

---

## 12. FFI Preset Files to Ship

```bash
chip_maps/
├── stm32f405.toml      # Basic microcontroller
├── esp32.toml         # WiFi-enabled
├── vector_gpu.toml     # GPU/NPU target
├── lwip.toml          # TCP/IP stack
└── tinyusb.toml       # USB stack
```

---

*End of Specification*