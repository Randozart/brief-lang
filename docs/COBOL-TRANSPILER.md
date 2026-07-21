# Brief-to-COBOL Transpiler

A production-ready COBOL code generator that transpiles Brief's declarative, state-based smart contracts into IBM Enterprise COBOL for z/OS.

## Overview

Brief is a contract-first programming language designed for financial systems. This transpiler generates **bank-ready COBOL** with:

- **Contract verification** - Pre/post conditions become runtime guards
- **Zero-cost abstraction** - No runtime library required
- **Mainframe-native** - Generates IBM Enterprise COBOL with `RECURSIVE` support
- **Formal verification hooks** - State transitions are formally verifiable

## Quick Start

```bash
# Compile a Brief file to COBOL
cargo run --bin brief-compiler -- cobol mycontract.bv --out output/

# Or after installing
brief cobol contract.brv --out ./cobol_output
```

## Example Transpilation

### Brief Source (`transfer.br`)

```brief
let balance: Int = 1000;
let transfer_in_progress: Bool = false;

node transfer [transfer_in_progress == false && balance >= 100]
  [balance == @balance - 100]
{
  &transfer_in_progress = true;
  &balance = balance - 100;
  &transfer_in_progress = false;
  term;
};
```

### Generated COBOL (`transfer.cbl`)

```cobol
>>SOURCE FORMAT IS FREE
IDENTIFICATION DIVISION.
PROGRAM-ID. TRANSFER RECURSIVE.

DATA DIVISION.
WORKING-STORAGE SECTION.
01  WS-OLD-BALANCE PIC S9(18) COMP-5 VALUE 0.
01  WS-BALANCE PIC S9(18) COMP-5 VALUE 1000.
01  WS-TRANSFER_IN_PROGRESS PIC X VALUE 'N'.
    88  WS-TRANSFER_IN_PROGRESS-TRUE   VALUE 'Y'.
    88  WS-TRANSFER_IN_PROGRESS-FALSE  VALUE 'N'.
01  WS-RECURSION-DEPTH PIC 9(4) COMP-5 VALUE 0.
01  WS-RECURSION-MAX   PIC 9(4) COMP-5 VALUE 1000.

PROCEDURE DIVISION.
MAIN-LOGIC SECTION.
    * PRE-CONDITION: ((transfer_in_progress == false) and (balance >= 100))
    IF NOT (((WS-TRANSFER_IN_PROGRESS = 'N') AND (WS-BALANCE >= 100)))
        DISPLAY "BRIEF CONTRACT FAILED: PRECONDITION: ..."
        MOVE 4000 TO RETURN-CODE
        GOBACK
    END-IF.

    MOVE WS-BALANCE TO WS-OLD-BALANCE.

    COMPUTE WS-TRANSFER_IN_PROGRESS = 'Y'.
    SUBTRACT 100 FROM WS-BALANCE.
    COMPUTE WS-TRANSFER_IN_PROGRESS = 'N'.

    * POST-CONDITION: (balance == (... - 100))
    IF NOT ((WS-BALANCE = (WS-OLD-BALANCE - 100)))
        DISPLAY "BRIEF CONTRACT FAILED: POSTCONDITION: ..."
        MOVE 4000 TO RETURN-CODE
        GOBACK
    END-IF.

    EXIT PARAGRAPH.
    GOBACK.

END PROGRAM TRANSFER.
```

## How It Works

### Contract Enforcement

Every Brief transaction has optional pre and post conditions:

```brief
txn withdraw [balance >= amount]
  [balance == @balance - amount]
{ ... }
```

These compile to **hard runtime guards** in COBOL:
- Pre-conditions: Checked before the transaction body
- Post-conditions: Checked after the transaction body (using `old()` to capture prior state)
- On failure: `RETURN-CODE` is set to 4000 and control returns to caller

### Type Mapping

| Brief Type | COBOL PIC Clause | Notes |
|------------|-----------------|-------|
| `Int` | `PIC S9(18) COMP-5` | Native binary |
| `Bool` | `PIC X` + 88-level | Condition names |
| `String` | `PIC X(N)` | Fixed length |
| `Vector(T, N)` | `OCCURS N TIMES` | Arrays |
| Custom `dec` | `PIC S9(13)V99 COMP-3` | Packed decimal |

### State Management

Brief state maps directly to COBOL's `WORKING-STORAGE SECTION`:

```brief
let counter: Int = 0;
let enabled: Bool = false;
```

```cobol
WORKING-STORAGE SECTION.
01  WS-COUNTER PIC S9(18) COMP-5 VALUE 0.
01  WS-ENABLED PIC X VALUE 'N'.
    88  WS-ENABLED-TRUE   VALUE 'Y'.
    88  WS-ENABLED-FALSE  VALUE 'N'.
```

## Attribute Syntax

Fine-tune COBOL generation with attributes:

```brief
# Custom PIC clause
state balance: Int #[cobol, type("PIC S9(15) COMP-3")]

# Use native binary (COMP-5)
state flags: Int #[cobol, native]

# Use packed decimal (COMP-3)
state amount: Int #[cobol, packed]

# Custom initialization
state timeout: Int #[cobol, init("300")]

# Abort on contract failure (use CEE3ABD)
txn critical_op() #[cobol, abend]
  pre: system_ready == true
{ ... }
```

## File Extensions

- **`.br`** - Pure Brief (specification only)
- **`.bv`** - Brief with view/rendering
- **`.ebv`** - Embedded Brief (hardware)
- **`.rbv`** - Rendered Brief (compiled to JS)

## Error Codes

| Code | Meaning |
|------|---------|
| 4000 | Pre or post-condition contract violated |
| 4001 | Recursion watchdog triggered (depth exceeded) |
| 4002 | Invalid state transition |

## CLI Usage

```bash
# Basic compilation
brief-compiler cobol <file> [--out <directory>]

# With custom output directory
brief-compiler cobol contracts/transfer.br --out ./cobol_gen

# With verbose output
brief-compiler cobol -v mycontract.bv
```

## Architecture

```
Brief Source (.bv)
       │
       ▼
┌──────────────────┐
│     Parser       │
└──────────────────┘
       │
       ▼
┌──────────────────┐
│   Desugarer      │
└──────────────────┘
       │
       ▼
┌──────────────────┐
│  Type Checker    │
└──────────────────┘
       │
       ▼
┌──────────────────┐
│ COBOL Backend    │ ◄─── Generates COBOL with contract guards
└──────────────────┘
       │
       ▼
  COBOL (.cbl)
```

## Supported Features

| Feature | Status | Notes |
|---------|--------|-------|
| Pre-conditions | ✅ | IF guards at block entry |
| Post-conditions | ✅ | IF guards at block exit |
| `old()` state capture | ✅ | Auto-generates shadow variables |
| Boolean types | ✅ | Level 88 condition names |
| Recursion (RECURSIVE) | ✅ | Always enabled |
| Watchdog counters | ✅ | On transactions with `[?]watchdog` |
| Attribute overrides | ✅ | `#[cobol, type("...")]` |
| Integer arithmetic | ✅ | ADD/SUBTRACT/COMPUTE |
| FFI Linkage | 🚧 | Via LINKAGE SECTION |

## Integration with Existing COBOL

For calling Brief-generated COBOL from existing mainframe programs:

```cobol
       CALL 'TRANSFER' USING LS-BALANCE, LS-AMOUNT
       IF RETURN-CODE NOT = 0
           DISPLAY 'Transfer failed with: ' RETURN-CODE
       END-IF
```

## Requirements

- Brief compiler (this repository)
- IBM Enterprise COBOL 5.1+ or GnuCOBOL
- z/OS, VSE, or Linux (GnuCOBOL)

## License

Copyright 2026 Randy Smits-Schreuder Goedheijt

Licensed under the Apache License, Version 2.0