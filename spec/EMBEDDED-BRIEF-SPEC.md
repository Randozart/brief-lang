# Embedded Brief Specification

**Version:** 1.0  
**Date:** 2026-04-12  
**Status:** Draft - Design Specification  

---

## 1. Introduction

### 1.1 Purpose

Embedded Brief (`.ebv`) is a bare-metal variant of the Brief language designed for microcontroller and embedded systems programming. It extends Brief's contract-first philosophy to direct hardware interaction, enabling verifiable, deterministic embedded software.

### 1.2 Position in Brief Ecosystem

Brief consists of three variants sharing a common core:

| Variant | File Extension | Purpose | Target |
|---------|---------------|---------|--------|
| Core Brief | `.br` | Pure logic specification | Verification/analysis |
| Rendered Brief | `.rbv` | Logic + UI/View bindings | Web (WASM) |
| Embedded Brief | `.ebv` | Logic + hardware bindings | Bare metal (no_std) |

**Core Brief** handles `struct`, `txn`, and contracts. **Rendered Brief** adds `rstruct` with view bodies. **Embedded Brief** removes view constructs and adds hardware binding syntax (`trg`, address ranges, interrupt-driven execution).

### 1.3 Design Philosophy

Embedded Brief inherits Brief's core principles while adapting for physical silicon:

1. **Contracts First**: Every hardware interaction is verified via preconditions/postconditions
2. **Deterministic Execution**: No garbage collection, no runtime heap, predictable timing
3. **Compile-Time Memory Layout**: All addresses validated before flash
4. **Transparent Hardware**: Memory-mapped I/O is explicit, not hidden
5. **Interrupt-Aware Logic**: The compiler models asynchronous hardware events

### 1.4 File Extension

Embedded Brief source files use the `.ebv` extension.

---

## 2. Memory Model

### 2.1 TOML Memory Specification

Every `.ebv` file requires a memory configuration file (typically `memory.toml`). This defines the physical constraints of the target chip.

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

[[memory.reserved]]
start = "0x40020000"
size = "64KB"
name = "GPIO"
```

**Sections:**
- `[chip]` - Chip identification (architecture, name)
- `[memory.ram]` - Available RAM for mutable variables
- `[memory.flash]` - Available flash for constants
- `[memory.reserved]` - Hardware registers or reserved areas (compiler blocks access unless `!@` used)

### 2.2 Address Binding

Variables bind to hardware addresses using the `@` syntax:

```brief
let status: Int @ 0x4000_2000;
let counter: UInt @ 0x4000_2004 / 0..3;
let flag: Bool @ 0x4000_2004 / 7;
```

**Syntax:**
```
variable_decl ::= "let" identifier ":" type ( "@" address ( "/" range )? )? ";"
address       ::= hex_literal
range         ::= bit_range | value_range
bit_range     ::= number ".." number | number
value_range   ::= number ".." number
```

**Rules:**
- `@ address` - Binds variable to specific memory-mapped address
- `/ range` - Specifies bit width or value constraints
- Address must be within RAM, Flash, or explicitly mapped hardware region
- Reserved ranges block access by default

### 2.3 Override Safety

The `!@` operator allows explicit override of reserved range safety:

```brief
let debug_reg: Int !@ 0xE000E100;  // System Control Block - allowed with override
```

This tells the compiler: "I know this is reserved, I accept responsibility."

### 2.4 Compiler-Allocated Addresses

When no address is specified, the compiler allocates from available RAM:

```brief
let temp_buffer: UInt @/8;       // Compiler finds 8-bit slot in RAM
let scratch: Int @/;            // Default 32-bit allocation
let flags: UInt @/0..3;        // Compiler allocates 4-bit field
```

**Rules:**
- `@/` without address means "allocate in available RAM"
- Range still applies - compiler packs variables tightly
- Compiler fails if insufficient contiguous space available

---

## 3. Type System

### 3.1 Primitive Types

Embedded Brief supports these primitives:

| Type | Aliases | Description | Default Range |
|------|---------|-------------|---------------|
| `Int` | `Signed`, `Sgn` | Signed integer | Platform-defined |
| `UInt` | `Unsigned`, `USgn` | Unsigned integer | Platform-defined |
| `Bool` | - | Boolean (single bit) | true/false |

**Type + Range Combination:**
- `Int @/8` = 8-bit signed (values -128 to 127)
- `UInt @/8` = 8-bit unsigned (values 0-255)
- `Int / 0..6` = 7-bit signed (values -64 to 63)
- `UInt / 0..6` = 7-bit unsigned (values 0-127)

### 3.2 Range Modifiers

**Bit Range** (`/0..6`, `/7`):

```brief
let value: UInt / 0..6;   // Bits 0-6 (7 bits, values 0-127)
let bit7: Bool / 7;       // Single bit at position 7
let full_byte: UInt;      // No range = full 8 bits
```

**Value Range** (`/0..15`):

```brief
let brightness: UInt / 0..15;   // Values 0-15 (4-bit precision)
let count: UInt / 0..100;        // Values 0-100 (triggers compile error if >255)
let raw: UInt;                   // Full range 0-255
```

**Range Semantics:**
- `/N` - Single bit at position N (valid for Bool types)
- `/M..N` - Bit range from M to N (inclusive)
- `/0..X` - Value range from 0 to X (bits calculated as ceil(log2(X+1)))
- No range - Full machine word size

### 3.3 Trigger Type (`trg`)

The `trg` type represents hardware-sourced values that can change asynchronously. These are inputs from the physical world (buttons, sensors, ADCs).

```brief
trg button: Bool @ 0x4000_2000 / 0;
trg encoder: Int @ 0x4000_2004 / 0..13;
trg sensor_error: Bool @ 0x4000_2008 / 2;
```

**Characteristics:**
- Must have explicit `@ address` - no compiler allocation
- Changes detected via hardware interrupt
- Compiler models these as "untrusted" in reactive analysis
- **Cannot be mutated by code** - only read from hardware (read-only)

**`trg` vs `let @ address`:**
- `trg` = input/sensor (hardware → software, read-only)
- `let` with `@ address` = output/actuator (software → hardware, writable)

```brief
// INPUT: hardware button (read-only)
trg button: Bool @ 0x40020010 / 0;

// OUTPUT: LED connected to hardware (writable)
let led: Bool @ 0x40020000 / 0;

rct txn light_led [button == true][led == true] {
    &led = true;
    term;
};
```

**Syntax:**
```
trg_decl ::= "trg" identifier ":" type "@" address "/" range ";"
```

### 3.4 `trg` with Bit-Field Access

Multiple `trg` variables can map to the same address:

```brief
// GPIO Port A at 0x40020000 - INPUT triggers (read-only)
trg button1: Bool @ 0x40020000 / 8;   // Bit 8
trg button2: Bool @ 0x40020000 / 9;   // Bit 9

// Combined value range - INPUT
trg adc_value: Int @ 0x40020004 / 0..11;  // 12-bit ADC (bits 0-11)
```

### 3.5 Type Inference

When range is not explicitly specified:

```brief
let x: Int @ 0x4000_2000;    // Default: machine word (32-bit on Cortex-M)
let y: UInt @ 0x4000_2004;   // Default: 8 bits (0-255)
let z: Bool @ 0x4000_2008;   // Default: 1 bit (single bit access)
```

**Rule:** If compiler cannot determine required size and user input could exceed available range, compilation fails with error requiring explicit range specification.

---

## 4. Execution Model

### 4.1 Reactor on Bare Metal

Embedded Brief replaces the dynamic event loop with **static interrupt-driven execution**:

```
┌─────────────────────────────────────────────────────────┐
│                    Embedded Reactor                     │
├─────────────────────────────────────────────────────────┤
│  1. Initialize (static memory layout)                  │
│  2. Enable hardware interrupts                          │
│  3. Loop:                                               │
│     a. Check trg states (hardware registers)            │
│     b. Evaluate rct preconditions                       │
│     c. Execute matching transactions                    │
│     d. If no rct ready: WFI (Wait For Interrupt)       │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Static Reactor Collapse

Because all states, addresses, and triggers are known at compile time, the reactor compiles to **static jumps**:

```rust
// Compiled reactor pseudocode
fn reactor() {
    loop {
        // Check triggers (read volatile hardware registers)
        let btn = ptr::read_volatile(0x4000_2000 as *const bool);
        
        // Static dispatch based on precondition analysis
        if btn && precondition_1() {
            txn_button_pressed();
            continue;
        }
        
        // ... more static branches ...
        
        // Idle: wait for hardware interrupt
        cortex_m::asm::wfi();
    }
}
```

### 4.3 No Standard Library

Embedded Brief compiles to `no_std` Rust with no OS dependencies:

```rust
#![no_std]
#![no_main]

// Minimal runtime - just what Brief needs
use core::panic::PanicHandler;
```

### 4.4 WFI (Wait For Interrupt)

When no reactive transactions are ready, the reactor enters low-power mode:

```brief
// Compiler generates: cortex_m::asm::wfi() when no preconditions met
```

This enables power-efficient embedded operation - the CPU halts until hardware event occurs.

---

## 5. Transactions and Contracts

### 5.1 Transaction Types

Embedded Brief supports both passive and reactive transactions:

```brief
// Passive - called explicitly
txn initialize [true][gpio_initialized == true] {
    &gpio_initialized = true;
    term;
};

// Reactive - fires when trigger changes
rct txn on_button_pressed [button == true][button == false] {
    term;
};
```

### 5.2 Contract Verification

Contracts apply to hardware interactions:

```brief
// Precondition: button must be pressed
// Postcondition: button will be cleared (read as false after term)
rct txn handle_button [button == true][button == false] {
    term;
};
```

### 5.3 Temporal Contracts (Watchdog)

Contracts can include timing constraints:

```brief
rct txn motor_control [command != 0][motor_running == true within 10ms] {
    &motor_running = true;
    term;
};
```

The compiler translates `[within X]` to hardware watchdog timer configuration.

### 5.4 Atomic Mutations

Hardware state mutations must be atomic to prevent interrupt tearing:

```brief
rct txn update_counter [true][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

The compiler ensures:
- Variables accessed by ISR and main reactor use atomic instructions
- If atomicity impossible, critical section wraps the mutation

### 5.5 Shadow State Verification

Brief contracts can verify hardware state matches software "shadow":

```brief
let shadow_motor_speed: Int = 0;
trg motor_speed: Int @ 0x4000_2000 / 0..7;

rct txn verify_shadow [shadow_motor_speed != motor_speed][true] {
    // Contract: hardware must match shadow
    // If cosmic ray flips bit, precondition fails
    term;
};
```

---

## 6. Hardware Integration

### 6.1 DMA Ownership

When hardware (DMA) borrows memory from software:

```brief
// Declare DMA buffer - compiler allocates in RAM
let rx_buffer: UInt @/256;

// Loan to DMA hardware (compiler locks variable)
on dma_complete {
    // rx_buffer locked - reactor cannot read
    term;
};
```

The compiler statically forbids reading loaned variables until ownership returns.

### 6.2 Interrupt Handlers

Hardware interrupts map to reactive transactions via triggers:

```brief
trg timer_irq: Bool @ 0x40020000 / 0;

rct txn timer_tick [timer_irq == true][timer_irq == false] {
    &tick_count = tick_count + 1;
    term;
};
```

The compiler maps the trigger's address to the interrupt vector table entry.

### 6.3 Watchdog Contracts

Temporal contracts generate hardware watchdog configuration:

```brief
// If this txn doesn't complete within 100ms, hardware resets
rct txn safety_critical [trigger][safe_state == true within 100ms] {
    &safe_state = true;
    term;
};
```

Compiler generates:
- WDT configuration (timeout = 100ms)
- Timer reset on `term`
- If `term` never reached → system reset

### 6.4 Fault Handling

Contract failures trigger hardware fault handlers:

```brief
// On contract failure: log to flash, enter safe mode
on fault {
    &error_code = 0xDEAD;
    term;
};
```

---

## 7. FFI and Targets

### 7.1 Target System

Embedded Brief extends the target system:

```bash
# Compile for embedded (no_std)
brief build --target embedded <file.ebv>

# Use embedded TOML config
brief build --target embedded --chip STM32F405 <file.ebv>
```

**Targets:**
- `wasm` - WebAssembly (Rendered Brief)
- `native` - Native execution (Core Brief)
- `embedded` - Bare metal (Embedded Brief)

### 7.2 Passport System

FFI works with embedded-specific functions:

```toml
[pasport.uart]
target = "embedded"
import = "-hal::uart"

[pasport.uart.write]
pre = "[buffer.len > 0]"
post = "[written == buffer.len]"
```

Functions map to `no_std` HAL crates or direct register access.

### 7.3 Embedded FFI Patterns

```brief
// Direct register access via FFI
defn uart_send(byte: UInt) -> () 
    [true] 
    [tx_register == byte] 
from "embedded::hal::uart" {};

// Hardware math (if no native implementation)
defn fast_sqrt(value: Int) -> Int 
    [value > 0] 
    [result * result <= value && (result+1)*(result+1) > value] 
from "embedded::math";
```

---

## 8. Compiler Workflow

### 8.1 Compilation Pipeline

```
.ebv File
    │
    ▼
┌─────────────────────┐
│ 1. Parse            │ ← Tokenize, build AST
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 2. Type Check       │ ← Validate types, ranges
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 3. Load TOML        │ ← Memory layout, reserved ranges
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 4. Address Validate │ ← Check @ addresses against TOML
│   - Overlap detect  │ ← No variable overlaps reserved
│   - Bounds check    │ ← Within RAM/Flash
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 5. Pack Layout      │ ← Allocate @/ variables
│   - Static packing  │ ← Tight fit into available RAM
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 6. Reactor Gen      │ ← Static interrupt dispatch
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 7. no_std Rust      │ ← Output Rust with #![no_std]
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│ 8. Binary           │ ← Link + flash
└─────────────────────┘
```

### 8.2 Compile-Time Validation

**Address Validation:**
- Check all `@ address` are within declared RAM/Flash
- Check no overlap with reserved ranges (unless `!@`)
- Error if address outside chip boundaries

**Range Validation:**
- Check bit ranges don't exceed word size
- Check value ranges don't exceed bit capacity
- Error if value range requires more bits than type allows

**Overlap Detection:**
- Static analysis of all variable addresses and sizes
- Error if two variables map to same address
- For `@/` allocation: ensure sufficient contiguous space

### 8.3 Flash Workflow

```bash
# Compile and flash to chip
brief build --target embedded --chip STM32F405 --flash <file.ebv>

# Steps:
# 1. Compile .ebv → Rust → ELF
# 2. Link with linker script (memory layout from TOML)
# 3. Extract binary
# 4. Flash via debugger (OpenOCD/pyOCD/probe-rs)
```

---

## 9. Reference

### 9.1 Syntax Summary

| Construct | Syntax | Description |
|-----------|--------|-------------|
| Variable | `let x: Type @ address / range;` | Memory-mapped variable |
| Trigger | `trg x: Type @ address / range;` | Hardware-triggered variable |
| Override | `!@ address` | Bypass reserved range check |
| Range (bits) | `/0..6` or `/7` | Bit range or single bit |
| Range (value) | `/0..15` | Value range (implies bit width) |
| No address | `@/8` | Compiler allocates from RAM |
| Reactive | `rct txn name [pre][post] { ... }` | Interrupt-driven transaction |
| Passive | `txn name [pre][post] { ... }` | Called transaction |
| Temporal | `[post within Xms]` | Watchdog timeout |

### 9.2 Example: LED Controller

```brief
// memory.toml required:
// [chip] name = "STM32F405"
// [memory.ram] start = "0x20000000", size = "128KB"

// INPUT: hardware button (read-only)
trg button: Bool @ 0x40020010 / 0;

// OUTPUT: LED connected to hardware (writable)
let led: Bool @ 0x40020000 / 0;

// Software state
let led_state: Bool = false;
let press_count: Int = 0;

// Reactive: toggle LED on button press
rct txn toggle_led [button == true][button == false] {
    &led_state = !led_state;
    &led = led_state;
    term;
};

// Reactive: count presses (if we wanted continuous counting)
rct txn count_press [button == true && press_count < 255][press_count == @press_count + 1] {
    &press_count = press_count + 1;
    term;
};
```

### 9.3 Example: Motor Controller

```brief
// Hardware registers - OUTPUTS (writable via let @ address)
let motor_enable: Bool @ 0x40001000 / 0;
let motor_dir: Bool @ 0x40001000 / 1;
let motor_speed: Int @ 0x40001000 / 2..7;    // 6 bits (0-63)

// Hardware register - INPUT (read-only via trg)
trg motor_fault: Bool @ 0x40001004 / 0;

// Software state
let motor_running: Bool = false;
let target_speed: Int = 0;

// Initialize
txn init_motor [true][motor_fault == false] {
    &motor_enable = false;
    &motor_dir = false;
    &motor_speed = 0;
    term;
};

// Set speed - precondition ensures valid speed
txn set_speed [target_speed <= 63][motor_speed == target_speed] {
    &motor_speed = target_speed;
    term;
};

// Reactive: enable motor when speed set
rct txn start_motor [target_speed > 0 && !motor_running][motor_running == true] {
    &motor_running = true;
    &motor_enable = true;
    term;
};

// Reactive: fault handler
rct txn handle_fault [motor_fault == true][motor_running == false] {
    &motor_running = false;
    &motor_enable = false;
    term;
};
```

### 9.4 Example: Value Range

```brief
// 10-bit ADC result (values 0-1023, requiring 10 bits) - INPUT
trg adc_result: Int @ 0x40002000 / 0..9;

let display_value: Int / 0..99 = 0;    // 7 bits (values 0-99)

// Scale ADC to display (0-1023 → 0-99)
txn scale_adc [adc_result > 0][display_value == (adc_result / 10)] {
    &display_value = adc_result / 10;
    term;
};
```

---

## 10. Future Considerations

### 10.1 Potential Extensions

- **Chip Templates**: Ship TOML configs for common microcontrollers (STM32, ESP32, RP2040)
- **Value Range Validation**: Compile-time check if values exceed range
- **Multiple Chip Targets**: Single .ebv targeting multiple hardware variants
- **Safety Certification**: Formal proof generation for safety-critical systems

### 10.2 Out of Scope

- Dynamic memory allocation (heap)
- Floating-point operations (unless via FFI)
- Multi-threading (single interrupt-driven reactor)
- Network stacks (available via FFI to existing embedded crates)
- USB stacks (available via FFI to existing embedded crates)

---

## Appendix A: Grammar (BNF)

```bnf
program ::= (variable_decl | trg_decl | txn_def | struct_def | enum_def | constant | import)*

variable_decl ::= "let" identifier ":" type ( "@" address ( "/" range )? | "@/" range? ) ";"
trg_decl ::= "trg" identifier ":" type "@" address "/" range ";"
address ::= hex_literal
range ::= bit_range | value_range
bit_range ::= number ".." number | number
value_range ::= number ".." number

txn_def ::= ("rct")? "txn" identifier parameters? contract ("{" body "}" ";" | ";")
contract ::= "[" expression "]" "[" expression ( "within" number "ms" )? "]"

parameters ::= "(" (param ("," param)*)? ")"
param ::= identifier ":" type

type ::= "Bool" | "Int" | "UInt" | "Signed" | "Unsigned" | identifier
constant ::= "const" identifier ":" type "=" expression ";"
struct_def ::= "struct" identifier "{" struct_member* "}"
enum_def ::= "enum" identifier "{" identifier ("," identifier)* "}"
import ::= "import" ( "{" identifier ("," identifier)* "}" )? ( "from" string_literal )? ";"
```

---

## Appendix B: Reserved Keywords

```
let, const, trg, txn, rct, term, escape
Bool, Int, UInt, Signed, Unsigned, true, false
struct, enum, import, from
@, !@, /
```

---

*End of Specification*