# Briv-to-COBOL Transpiler Specification

## Overview

This document specifies the COBOL transpilation target for the Briv compiler, generating IBM Enterprise COBOL for z/OS from Briv's declarative, state-based logic.

## 1. Lexical & Structural Directives

### 1.1 Source Format
- Inject `>>SOURCE FORMAT IS FREE` at line 1 to bypass 80-column punch card limits

### 1.2 Program Definition
- `IDENTIFICATION DIVISION.` followed by `PROGRAM-ID. [MODULE-NAME] RECURSIVE.`
- The `RECURSIVE` keyword is always appended to support Briv's recursion model

### 1.3 Division Order
Strict ordering: `IDENTIFICATION`, `DATA`, `PROCEDURE`

## 2. Type Mapping (DATA DIVISION)

### 2.1 Default Mappings

| Briv Type | COBOL PIC Clause | Usage | Notes |
|------------|------------------|-------|-------|
| `int` | `PIC S9(18) COMP-5` | COMP-5 | Native binary, 64-bit |
| `dec` | `PIC S9(13)V99 COMP-3` | COMP-3 | Packed decimal, 15 digits |
| `float` | `PIC S9(15)V99 COMP-3` | COMP-3 | Packed decimal (currency-safe) |
| `str(N)` | `PIC X(N)` | — | Fixed length at compile time |
| `bool` | `PIC X` + 88-level | — | Single byte with condition names |

### 2.2 Attribute Override Syntax

Override defaults using attributes:

```briv
# Default: derives from type
state balance: dec

# Explicit override
state balance: dec #[cobol, type("S9(15)V99 COMP-3")]

# Abbreviated forms
state amount: dec #[cobol, decimal(15,2)]
state flags: int #[cobol, native]    # COMP-5
state flags: int #[cobol, packed]   # COMP-3
```

### 2.3 Boolean Mapping

COBOL has no native boolean. Map to single byte with Level 88 condition names:

```cobol
01 WS-IS-VALID PIC X VALUE 'N'.
   88 IS-VALID      VALUE 'Y'.
   88 IS-NOT-VALID  VALUE 'N'.
```

### 2.4 Array/Vector Mapping

```briv
state ids: vec[int, 10]  # OCCURS 10 TIMES
```

```cobol
01 WS-IDS OCCURS 10 TIMES PIC S9(18) COMP-5.
```

## 3. State Scoping & FFI Memory Model

### 3.1 Local State (WORKING-STORAGE)

Briv state declared in module body → `WORKING-STORAGE SECTION`

Name mangling: `{NAME}` → `WS-{MANGLED-NAME}`

```briv
state count: int

fn increment() {
    count = count + 1
}
```

```cobol
WORKING-STORAGE SECTION.
01  WS-COUNT PIC S9(18) COMP-5 VALUE 0.
```

### 3.2 FFI Parameters (LINKAGE SECTION)

Parameters in function signatures → `LINKAGE SECTION` + `PROCEDURE DIVISION USING`

```briv
fn process(balance: dec, amount: dec) -> dec
```

```cobol
LINKAGE SECTION.
01  LS-BALANCE PIC S9(13)V99 COMP-3.
01  LS-AMOUNT  PIC S9(13)V99 COMP-3.

PROCEDURE DIVISION USING LS-BALANCE LS-AMOUNT.
```

### 3.3 Return Values

Return value becomes last parameter in USING clause:

```briv
fn calc(a: int, b: int) -> int
```

```cobol
LINKAGE SECTION.
01  LS-A      PIC S9(18) COMP-5.
01  LS-B      PIC S9(18) COMP-5.
01  LS-RESULT PIC S9(18) COMP-5.

PROCEDURE DIVISION USING LS-A LS-B LS-RESULT.
```

## 4. Contract Transpilation (Pre/Post Conditions)

### 4.1 Pre-condition Guards

Transpile to inverted IF traps at block start:

```briv
fn withdraw(balance: dec, amount: dec) -> dec
    pre: amount > 0
    pre: balance >= amount
```

```cobol
* BRIV PRE-CONDITION GUARDS
IF LS-AMOUNT <= 0
    DISPLAY "BRIV CONTRACT FAILED: PRECONDITION_01: amount > 0"
    MOVE 4000 TO RETURN-CODE
    GOBACK
END-IF.
IF LS-BALANCE < LS-AMOUNT
    DISPLAY "BRIV CONTRACT FAILED: PRECONDITION_02: balance >= amount"
    MOVE 4000 TO RETURN-CODE
    GOBACK
END-IF.
```

### 4.2 Post-condition Guards with `old()` State

**Step 1:** Identify all `old(var)` references in post-condition

**Step 2:** Generate shadow variables in WORKING-STORAGE:
```cobol
01  WS-OLD-BALANCE PIC S9(13)V99 COMP-3.
```

**Step 3:** Capture old state before core logic:
```cobol
MOVE LS-BALANCE TO WS-OLD-BALANCE.
```

**Step 4:** Validate post-condition at end:
```briv
fn withdraw(balance: dec, amount: dec) -> dec
    pre: amount > 0
    post: balance == old(balance) - amount
```

```cobol
* CAPTURE OLD STATE FOR POST-CONDITION
MOVE LS-BALANCE TO WS-OLD-BALANCE.

* CORE LOGIC
SUBTRACT LS-AMOUNT FROM LS-BALANCE.

* POST-CONDITION GUARD
COMPUTE WS-EXPECTED = WS-OLD-BALANCE - LS-AMOUNT.
IF LS-BALANCE NOT = WS-EXPECTED
    DISPLAY "BRIV CONTRACT FAILED: POSTCONDITION_01: balance == old(balance) - amount"
    MOVE 4000 TO RETURN-CODE
    GOBACK
END-IF.
```

### 4.3 Hard ABEND Option

For critical failures requiring program termination:

```briv
#[cobol, abend]
fn critical_op() {
    pre: system_ready == true
}
```

```cobol
IF NOT (WS-SYSTEM-READY = 'Y')
    DISPLAY "BRIV CONTRACT FAILED: PRECONDITION_01"
    MOVE 4000 TO RETURN-CODE
    CALL "CEE3ABD" USING BY VALUE 4000 BY VALUE 0
    GOBACK
END-IF.
```

## 5. Recursion & Watchdog

### 5.1 RECURSIVE Keyword

Always emit `RECURSIVE` in PROGRAM-ID to support Briv's recursion model:
```cobol
PROGRAM-ID. TRANSFER RECURSIVE.
```

### 5.2 Recursion Depth Counter

If watchdog is specified in contract, generate depth limiting:

```briv
fn factorial(n: int) -> int
    pre: n >= 0
    post: result >= 0
    [?]watchdog: n <= 20  # Optional: limit recursion depth
```

```cobol
WORKING-STORAGE SECTION.
01  WS-RECURSION-DEPTH PIC 9(4) COMP-5 VALUE 0.
01  WS-RECURSION-MAX   PIC 9(4) COMP-5 VALUE 20.

* At entry of recursive block
ADD 1 TO WS-RECURSION-DEPTH.
IF WS-RECURSION-DEPTH > WS-RECURSION-MAX
    DISPLAY "BRIV WATCHDOG: Recursion depth exceeded"
    MOVE 4001 TO RETURN-CODE
    GOBACK
END-IF.

* At exit of recursive block
SUBTRACT 1 FROM WS-RECURSION-DEPTH.
```

## 6. Control Flow Mapping

### 6.1 Functions/Blocks → PARAGRAPHs

```briv
fn process() {
    step_one()
    step_two()
}

fn step_one() { ... }
fn step_two() { ... }
```

```cobol
PROCEDURE DIVISION.
MAIN-LOGIC SECTION.
    PERFORM PROCESS.
    GOBACK.

PROCESS SECTION.
    PERFORM STEP-ONE.
    PERFORM STEP-TWO.
    EXIT PARAGRAPH.

STEP-ONE SECTION.
    ...
    EXIT PARAGRAPH.

STEP-TWO SECTION.
    ...
    EXIT PARAGRAPH.
```

### 6.2 If/Else → IF/ELSE/END-IF

```briv
if x > 0 {
    y = 1
} else {
    y = 0
}
```

```cobol
IF LS-X > 0
    MOVE 1 TO LS-Y
ELSE
    MOVE 0 TO LS-Y
END-IF.
```

### 6.3 Match/Switch → EVALUATE

```briv
match status {
    "active" => state = 1
    "pending" => state = 2
    _ => state = 0
}
```

```cobol
EVALUATE WS-STATUS
    WHEN "ACTIVE"
        MOVE 1 TO WS-STATE
    WHEN "PENDING"
        MOVE 2 TO WS-STATE
    WHEN OTHER
        MOVE 0 TO WS-STATE
END-EVALUATE.
```

### 6.4 Loops → PERFORM UNTIL

```briv
while i < 10 {
    sum = sum + i
    i = i + 1
}
```

```cobol
PERFORM UNTIL LS-I >= 10
    COMPUTE WS-SUM = WS-SUM + LS-I
    ADD 1 TO LS-I
END-PERFORM.
```

## 7. File-Level Directives

### 7.1 Default Target (#![...])

Set default transpilation target for subsequent attributes:

```briv
#![cobol, program-id("BANK-TRANSFER")]
#![cobol, keywords("RECURSIVE")]

# Now these attrs default to cobol target:
#[decimal(15,2)]  # no need for #[cobol, ...]
```

### 7.2 Program ID Override

```briv
#![cobol, program-id("MYMODULE")]
```

Output: `PROGRAM-ID. MYMODULE RECURSIVE.`

### 7.3 Global COBOL Options

```briv
#![cobol, abend-on-contract-fail]
```

## 8. Expression Translation

### 8.1 Operators

| Briv | COBOL | Notes |
|-------|-------|-------|
| `+` | `+` | |
| `-` | `-` | |
| `*` | `*` | |
| `/` | `/` | |
| `==` | `=` | |
| `!=` | `NOT =` | |
| `<` | `<` | |
| `>` | `>` | |
| `<=` | `<=` | |
| `>=` | `>=` | |
| `and` | `AND` | |
| `or` | `OR` | |
| `not` | `NOT` | |

### 8.2 Assignment

Briv: `x = y + 1`

COBOL: Use MOVE for simple, COMPUTE for expressions:
```cobol
COMPUTE WS-X = WS-Y + 1.
```

### 8.3 Compound Assignment

Briv: `x += 1`

COBOL:
```cobol
ADD 1 TO WS-X.
```

Briv: `x = x - 1`
```cobol
SUBTRACT 1 FROM WS-X.
```

## 9. Output Template

```cobol
>>SOURCE FORMAT IS FREE
IDENTIFICATION DIVISION.
PROGRAM-ID. {MODULE-NAME} RECURSIVE.

DATA DIVISION.
WORKING-STORAGE SECTION.
{* Shadow variables for old() capture *}
{* Local state declarations with VALUE initialization *}

LINKAGE SECTION.
{* FFI parameter declarations (parameters + return) *}

PROCEDURE DIVISION USING {params}.
MAIN-LOGIC SECTION.
    {* PRE-CONDITION GUARDS *}
    {* OLD STATE CAPTURE *}
    {* CORE LOGIC (transaction body) *}
    {* POST-CONDITION GUARDS *}
    GOBACK.

END PROGRAM {MODULE-NAME}.
```

## 10. CLI Integration

```bash
# Compile to COBOL
briv-compiler cobol <file.brv>

# Output: <file>.cbl
```

## 11. Error Codes

| Code | Meaning |
|------|---------|
| 4000 | Contract pre/post condition failed |
| 4001 | Recursion watchdog triggered |
| 4002 | Invalid state transition |

## 12. Reserved Attribute Keys

| Key | Value Type | Description |
|-----|------------|-------------|
| `type` | string | Override PIC clause |
| `decimal` | string | Abbreviated decimal spec (e.g., "15,2") |
| `native` | flag | Use COMP-5 |
| `packed` | flag | Use COMP-3 |
| `keyword` | string | Emit COBOL keyword |
| `abend` | flag | Use CEE3ABD on failure |
| `init` | string | VALUE initialization |

## 13. Implementation Notes

- Always use `COMPUTE` for complex arithmetic expressions
- Use `MOVE` for simple assignments
- COBOL requires exact column alignment in free format but we're using FREE mode
- Generate unique shadow variable names to avoid collision: `WS-OLD-{FUNC}-{VAR}`
- Transaction parameters go to LINKAGE; module-level state goes to WORKING-STORAGE