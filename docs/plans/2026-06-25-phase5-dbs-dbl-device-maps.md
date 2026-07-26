# Phase 5: DBS/DBL Device Address Maps + `import "target"`

**Date:** 2026-06-25
**Status:** Planned
**Dependencies:** Phase 1–4 (complete), existing D-brief schema system

---

## Goal

Enable Brief programs to import board-level device descriptions via
`import "target"`, gaining typed `Ptr<T>` constants for peripheral MMIO
registers with contract-proven address ranges.

A Brief program writes:
```brief
import "target";  // resolves to board spec

// Board provides: uart, gpio, timer, etc. as typed constants
uart .#Ptr;      // Int — base address of UART peripheral
uart_dr .#Ptr;   // Ptr<Byte> — UART data register at UART_BASE+0x00
```

## Design

### Schema files (DBS — Device Byte Specification)

**Location:** `lib/devices/<peripheral>.dbvs`

Each `.dbvs` file defines a peripheral's register layout using the existing
D-brief v2 schema format:

```
// lib/devices/uart.dbvs
schema Uart {
    base_addr: UInt[64];
    register Data @ 0x00 { size: 8; access: rw; };
    register Status @ 0x01 { size: 8; access: r; };
    register Control1 @ 0x0C { size: 16; access: rw; };
    register Control2 @ 0x10 { size: 16; access: rw; };
};
```

The existing `dbrief::v2` parser (`src/dbrief/v2.rs`) already parses `schema`
definitions with fields. The `DBS` loader adds a `register` keyword (or uses
the existing field system to represent register offsets).

Alternatively, reuse the existing ALIAS system from legacy `.dbvs`:

```
// lib/devices/uart.dbvs
ALIAS UART_BASE: Int;
ALIAS UART_DR: Int = @0x00;    // offset from base
ALIAS UART_SR: Int = @0x01;
```

### Board layout files (DBL — Device Board Layout)

**Location:** `lib/boards/<board>.dbvl`

Each `.dbvl` file maps peripherals to addresses on a specific board:

```
// lib/boards/stm32f407.dbvl
schema lib/devices/uart.dbvs;
UART1 { base_addr: 0x40011000; };
UART2 { base_addr: 0x40004400; };

schema lib/devices/gpio.dbvs;
GPIOA { base_addr: 0x40020000; };
GPIOB { base_addr: 0x40020400; };
```

The existing `dbrief::v2` import system and `glue::dbvl_reader` already
handle `.dbvl` file parsing.

### `import "target"` resolver

**File:** `src/import_resolver.rs` (new logic)

When `import "target"` is encountered:

1. **Resolve board name** from CLI flag `--board <name>` or default target spec
2. **Load** `lib/boards/<name>.dbvl` using existing D-brief v2 parser
3. **Resolve schemas**: For each `schema <path>;` line, load the referenced
   `.dbvs` file
4. **Emit typed constants**: For each peripheral instance, generate compile-time
   constants of type `Ptr<T>` with contract-proven base addresses
5. **Auto-derive register addresses**: For each register at offset N from the
   peripheral base, emit `periph_reg: Ptr<Byte> = base + N`.

The existing `ImportResolver` already handles `.dbv`, `.dbvs`, `.dbvl` files
(lines 392–491 of `import_resolver.rs`). The bridge at
`dbrief::bridge::document_to_program_flags()` converts schema documents to
`Vec<TopLevel>`. This path is extended to handle `import "target"`.

### CLI flags

**File:** `src/main.rs`

```
brief-compiler --board stm32f407 --target armv7em-none-eabi source.bv
```

- `--board <name>` selects `lib/boards/<name>.dbvl`
- If `--board` is not set, `import "target"` emits a diagnostic
- Auto-detect board from target triple (future: match board CPU to target)

### Compile-time address constants

For each peripheral register, the resolver emits:

```brief
const uart : Ptr<Uart> = 0x40011000 as Ptr<Uart>;
const uart_dr : Ptr<Byte> = (0x40011000 + 0x00) as Ptr<Byte>;
const uart_sr : Ptr<Byte> = (0x40011000 + 0x01) as Ptr<Byte>;
const uart_cr1 : Ptr<Byte> = (0x40011000 + 0x0C) as Ptr<Byte>;
```

The contracts from `Ptr<T> .#Ptr` ensure address ranges are verified at
compile time.

## Implementation

### Part A: Schema and board files (30 min)

1. Create `lib/devices/` directory with initial schema files:
   - `lib/devices/uart.dbvs`
   - `lib/devices/gpio.dbvs`
   - `lib/devices/timer.dbvs`
   - `lib/devices/spi.dbvs`
   - `lib/devices/i2c.dbvs`

2. Create `lib/boards/` with initial board files:
   - `lib/boards/stm32f407.dbvl` — STM32F407 Discovery
   - `lib/boards/kv260.dbvl` — KV260 FPGA board

### Part B: Parser extensions (1 hr)

3. Add `register` keyword to D-brief v2 parser (optional — can use existing
   `FieldDef` system with `@<offset>` notation)

4. Add `#!board("<name>")` pragma as alternative to `--board` CLI flag

### Part C: Import resolver extension (2 hr)

5. In `src/import_resolver.rs`, add `import "target"` handling:
   - When `import.path == ["target"]`, resolve to board spec
   - Load `lib/boards/<board>.dbvl` via existing D-brief parser
   - Walk schema imports, resolve register offsets
   - Emit `TopLevel::Constant` entries for each PTR

6. Add memory overlap validation between board peripherals (reuse existing
   `hardware_validator.rs::check_memory_overlaps()`)

### Part D: Integration (30 min)

7. Add `--board` CLI flag to `src/main.rs`
8. Tests for each board file loading
9. Example file: `examples/target-import.bv`

---

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- Existing schema validation tests pass (hardware_validator)
- `import "target"` produces expected Ptr constants
- Board DBL files validate against device DBS files
- `_ => return None;` fallthrough unchanged
- Docs: `docs/architecture/features/target-import.md`
- Example: `examples/target-import.bv`

---

## Remaining: Usage reminder

After Phase 5, Brief programs can:
```brief
import "target";  // loads board-level device constants

// Interrupt handler registration
volatile_store#(IVT_BASE + 32, timer_isr :> Address);

// MMIO register access with contracts
let status: Byte = volatile_load#(uart_sr);
volatile_store#(uart_dr, 'H' as Byte);
```
