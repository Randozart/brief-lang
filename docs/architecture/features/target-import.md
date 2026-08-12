# `import "target"` — Board-Level Device Constants

**File:** `src/import_resolver.rs`
**Created:** 2026-06-25

## Overview

`import "target"` is a special import that loads board-level device descriptions
from `lib/boards/<name>.toml`. It emits typed `Int` constants for each
peripheral and register, enabling contract-proven MMIO access.

## Usage

```briev
import "target";  // uses --board or defaults to "stm32f407"

// Peripheral base address:
const UART1: Int = 0x40011000;  // from board TOML

// Register addresses (base + offset):
const uart_dr: Int  = 0x40011004;
const uart_sr: Int  = 0x40011008;

// MMIO via volatile_load#/volatile_store#:
let status: Byte = volatile_load#(uart_sr as Ptr<Byte>);
volatile_store#(uart_dr as Ptr<Byte>, 0x41);  // 'A'
```

## Board File Format (TOML)

```
# lib/boards/<name>.toml
[board]
name = "stm32f407"
cpu = "armv7em-none-eabi"

[[peripherals]]
name = "UART1"
base = 0x40011000
size = 0x18

[[peripherals.registers]]
name = "DR"
offset = 0x00
size = 8
access = "rw"
```

## CLI Flag

```bash
briev-compiler --board stm32f407 source.bv
```

Defaults to `stm32f407` if `--board` is not specified.

## Implementation

The `ImportResolver` detects `import "target"` by checking if
`import.path == ["target"]`. It then:

1. Resolves `--board` or falls back to `"stm32f407"`
2. Searches `lib/boards/<name>.toml` in the search paths
3. Parses the TOML file
4. Emits `TopLevel::Constant` entries for each peripheral and register

All constants are typed as `Int` with compile-time known values.
