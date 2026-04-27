# Brief Compiler Changelog - C Backend Fixes

**Date:** 2026-04-27
**Project:** brief-compiler
**Purpose:** Document C backend changes for bare-metal ARM support

---

## Changes Made on 2026-04-27

### File: `src/backend/c.rs`

#### 1. Added Linkage Support (Lines 15-21)

**Before:**
```rust
use crate::ast::{Expr, Program, Statement, TopLevel, Type};

pub struct CBackend;
```

**After:**
```rust
use crate::ast::{Expr, LinkRef, Program, Statement, TopLevel, Type};
use crate::linkage::LinkageConfig;

pub struct CBackend {
    linkage: Option<LinkageConfig>,
    hw_register_names: Vec<String>,
}
```

**Purpose:** Added linkage configuration support and tracking of hardware register names.

---

#### 2. Added `with_linkage()` Method (Lines 27-33)

```rust
pub fn with_linkage(mut self, linkage: LinkageConfig) -> Self {
    self.linkage = Some(linkage);
    self
}
```

**Purpose:** Allows passing linkage config to the C backend.

---

#### 3. Updated `generate()` to be `&mut self` (Line 36)

**Before:** `pub fn generate(&self, program: &Program) -> String`
**After:** `pub fn generate(&mut self, program: &Program) -> String`

**Purpose:** Required to collect hardware register names before generating output.

---

#### 4. Added `collect_hw_registers()` Method (Lines 152-158)

```rust
fn collect_hw_registers(&mut self, program: &Program) {
    self.hw_register_names.clear();
    for item in &program.items {
        if let TopLevel::Trigger(trg) = item {
            if let LinkRef::Linked(name) = &trg.address {
                self.hw_register_names.push(name.clone());
            }
        }
    }
}
```

**Purpose:** Collects all `@ link` hardware register names for later use in expression translation.

---

#### 5. Added `is_hw_register()` Method (Lines 160-163)

```rust
fn is_hw_register(&self, name: &str) -> bool {
    self.hw_register_names.iter().any(|n| n == name)
}
```

**Purpose:** Checks if an identifier is a hardware register (should use MMIO macro, not state->).

---

#### 6. Added `generate_linkage_defines()` Method (Lines 165-177)

```rust
fn generate_linkage_defines(&self, output: &mut String) {
    let Some(linkage) = &self.linkage else { return; };

    for name in &self.hw_register_names {
        if let Some(c_addr) = linkage.resolve_c(name) {
            let upper_name = name.to_uppercase();
            output.push_str(&format!("/* @ link: {} -> {} */\n", name, c_addr));
            output.push_str(&format!("#define {}_ADDR {}\n", upper_name, c_addr));
            output.push_str(&format!("#define {} (*(volatile uint32_t *){}_ADDR)\n", upper_name, upper_name));
            output.push_str("\n");
        }
    }
}
```

**Purpose:** Generates `#define` macros for MMIO hardware registers from linkage.toml.

---

#### 7. Updated Includes (Lines 45-47)

**Before:**
```c
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
```

**After:**
```c
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
```

**Purpose:** Removed `stdio.h` and `stdlib.h` (not available in bare-metal). Added `stddef.h` for NULL.

---

#### 8. Changed State Allocation to Static (Lines 64-66)

**Before:**
```c
static State *state = NULL;
```

**After:**
```c
static State state_instance;
static State *state = &state_instance;
```

**Purpose:** Uses static allocation instead of malloc for bare-metal targets.

---

#### 9. Removed malloc from `brief_init()` (Lines 100-103)

**Before:**
```c
void brief_init(void) {
    state = (State *)malloc(sizeof(State));
    state_init();
}
```

**After:**
```c
void brief_init(void) {
    // Using static allocation for bare-metal
    state_init();
}
```

**Purpose:** No dynamic allocation in bare-metal.

---

#### 10. Fixed ASM Clobber Syntax (Lines 192-202)

**Before (incorrect):**
```rust
output.push_str(&format!(
    "        : : \"r\" ({})\n",
    clobber_list
));
```

**After (correct):**
```rust
let clobber_list = clobbers.iter()
    .map(|c| format!("\"{}\"", c.as_str()))
    .collect::<Vec<_>>()
    .join(", ");
output.push_str("    __asm__ __volatile__(\n");
output.push_str(&format!("        \"{} \\n\"\n", asm_string));
output.push_str(&format!("        : : : {}\n", clobber_list));
output.push_str("    );\n");
```

**Purpose:** ASM clobbers must go in the third section of GCC asm statement (output, input, clobber).

---

#### 11. Updated `expr_to_c()` for Hardware Registers (Lines 228-240)

**Before:**
```rust
Expr::Identifier(n) => format!("state->{}", Self::sanitize_name(n)),
Expr::OwnedRef(n) => format!("state->{}", Self::sanitize_name(n)),
```

**After:**
```rust
Expr::Identifier(n) => {
    if self.is_hw_register(n) {
        n.to_uppercase()
    } else {
        format!("state->{}", Self::sanitize_name(n))
    }
}
Expr::OwnedRef(n) => {
    if self.is_hw_register(n) {
        n.to_uppercase()
    } else {
        format!("state->{}", Self::sanitize_name(n))
    }
}
```

**Purpose:** Hardware register identifiers use the generated macro name, not struct member access.

---

### File: `src/main.rs`

#### Updated `run_c()` to Load Linkage Config (Lines 1194-1203)

**Added:**
```rust
// Load linkage config (optional - look alongside source file)
let linkage_path = file_path
    .parent()
    .map(|p| p.join("linkage.toml"));
let linkage_config = if let Some(ref lp) = linkage_path {
    if lp.exists() {
        Some(linkage::LinkageConfig::load(lp).map_err(|e| {
            format!("Failed to load linkage.toml: {}", e)
        })?)
    } else {
        None
    }
} else {
    None
};

let mut c_backend = backend::c::CBackend::new();
if let Some(linkage) = linkage_config {
    c_backend = c_backend.with_linkage(linkage);
}
let output = c_backend.generate(&program);
```

**Purpose:** Loads `linkage.toml` from the same directory as the source file if present.

---

## Syntax Tested

### Brief kernel.ebv (working):
```brief
trg hw_control: UInt @ link hw_control;
trg hw_status: UInt @ link hw_status;

rct txn flush_cache [true] [true] {
    asm "dsb sy" {};
    term;
};
```

### Generated C:
```c
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/* @ link: hw_control -> 0x8000A000 */
#define HW_CONTROL_ADDR 0x8000A000
#define HW_CONTROL (*(volatile uint32_t *)HW_CONTROL_ADDR)

static State state_instance;
static State *state = &state_instance;

bool flush_cache(void) {
    __asm__ __volatile__(
        "dsb sy \n"
    );
    return true;
}
```

---

## Compilation Test

```bash
# Generate
./brief-compiler c kernel.ebv --out /tmp/test

# Compile for ARM bare-metal
aarch64-linux-gnu-gcc -nostdlib -static -march=armv8-a -ffreestanding -O2 -c /tmp/test/kernel.c -o kernel.o

# Result: Compiles without errors
```

---

## Previous Issues Fixed

| Issue | Error | Fix |
|-------|-------|-----|
| `malloc` in bare-metal | implicit declaration | Static allocation |
| ASM operand syntax | wrong operand format | Clobbers in third section |
| Hardware registers in expr | `state->hw_control` | `HW_CONTROL` macro |
| NULL undeclared | missing header | Added stddef.h |

---

## Related Files

- `src/linkage.rs` - LinkageConfig loader (pre-existing)
- `src/ast.rs` - LinkRef enum (pre-existing)
- `kernel.ebv` - Test Brief file (imp folder)
- `linkage.toml` - Test linkage config (imp folder)

---

## Allocation Mode Distinction (2026-04-27 23:20)

### File: `src/backend/c.rs`

Added `bare_metal` flag to distinguish targets:

```rust
pub struct CBackend {
    linkage: Option<LinkageConfig>,
    hw_register_names: Vec<String>,
    bare_metal: bool,  // NEW
}

pub fn bare_metal(mut self, bare_metal: bool) -> Self {
    self.bare_metal = bare_metal;
    self
}
```

### File: `src/main.rs`

Sets `bare_metal=true` for `.ebv` files:

```rust
let is_ebv = file_path.extension().map(|e| e == "ebv").unwrap_or(false);
// ...
if is_ebv {
    c_backend = c_backend.bare_metal(true);
}
```

### Target-Specific Output

| File Type | Allocation | Includes | Use Case |
|-----------|------------|----------|----------|
| `.bv` | `malloc` | `stdlib.h` | Desktop/Embedded Linux |
| `.ebv` | Static | None | Bare-metal ARM |

### Example .bv (hosted):
```c
/* Target: Hosted (Desktop/Embedded Linux) */
#include <stdlib.h>
static State *state = NULL;
void brief_init(void) {
    state = (State *)malloc(sizeof(State));
}
```

### Example .ebv (bare-metal):
```c
/* Target: Bare-metal ARM (Cortex-A) */
static State state_instance;
static State *state = &state_instance;
void brief_init(void) {
    state_init();  // No malloc
}
```
