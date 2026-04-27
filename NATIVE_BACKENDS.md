# Native Code Generation (Rust & C Backends)

**Date:** 2026-04-27
**Status:** Implemented

---

## Overview

Added two new compiler backends for generating native code from Brief programs:

1. **Rust Backend** (`src/backend/rust.rs`) - Generates native Rust with `asm!` blocks
2. **C Backend** (`src/backend/c.rs`) - Generates C with `__asm__ __volatile__`

---

## CLI Commands

### Rust Backend

```bash
./target/release/brief-compiler rust <file.bv> [--out <dir>]
```

**Output:** `<filename>.rs`

**Features:**
- Generates standalone Rust application
- Uses `std` for desktop/embedded Linux
- Comments indicate where `asm!` blocks would go (requires nightly Rust)
- State struct with transaction methods
- `main()` function that initializes state and runs transactions

**Example:**
```brief
let count: Int = 0;

txn inc [true][true] {
    count = count + 1;
};
```

Generates:
```rust
#[derive(Debug, Clone)]
pub struct State {
    pub count: i32,
}

impl State {
    pub fn inc(&mut self) -> bool {
        self.count = (self.count + 1);
        true
    }
}

fn main() {
    let mut state = State::default();
    state.inc();
}
```

---

### C Backend

```bash
./target/release/brief-compiler c <file.bv> [--out <dir>]
```

**Output:** `<filename>.c`

**Features:**
- Generates portable C code
- Uses `stdint.h`, `stdbool.h`, `stdio.h`, `stdlib.h`
- Global state pointer with `malloc`/`free`
- Transaction functions return `bool` (success/failure)

**Example:**
```brief
let count: Int = 0;

txn inc [true][true] {
    count = count + 1;
};
```

Generates:
```c
typedef struct {
    int32_t count;
} State;

static State *state = NULL;

bool inc(void) {
    state->count = (state->count + 1);
    return true;
}

int main(void) {
    state = (State *)malloc(sizeof(State));
    state_init();
    inc();
    free(state);
    return 0;
}
```

---

## Inline Assembly Syntax

Both backends support inline assembly:

```brief
asm "instruction" { "clobber1", "clobber2" };
```

### Examples

```brief
// ARM cache flush
txn flush_cache {
    effect {
        asm "DC CIVAC X0, X1" { "x0", "x1" };
        asm "DSB SY" {};
    }
}

// Set register to zero
asm "mov x0, #0" {};
```

### Backend Output

**Rust (requires nightly):**
```rust
// asm: DC CIVAC X0, X1
// clobbers: ["x0", "x1"]
unsafe { asm!("DC CIVAC X0, X1", inout("x0") _x0, inout("x1") _x1, options(nostack)); }
```

**C:**
```c
__asm__ __volatile__("DC CIVAC X0, X1" : : : "x0", "x1");
```

---

## CLI Changes

| Change | Description |
|--------|-------------|
| `rust` command | New - compile to native Rust |
| `c` command | New - compile to C (was `check` alias) |
| `ck` command | `check` now uses `ck` alias (freed `c` for C backend) |

---

## Files Changed/Added

### New Files
- `src/backend/rust.rs` - Rust backend generator
- `src/backend/c.rs` - C backend generator
- `src/backend/mod.rs` - Module exports (updated)

### Modified Files
- `src/main.rs` - Added `run_rust()`, `run_c()`, `rust`/`c` command handlers
- `src/lexer.rs` - Added `Asm` token
- `src/ast.rs` - Added `Statement::InlineAsm`
- `src/parser.rs` - Added `parse_asm_block()`
- `src/interpreter.rs` - Added `InlineAsm` handler
- `src/proof_engine.rs` - Added `InlineAsm` handler
- `src/reactor.rs` - Added `InlineAsm` handler
- `src/backend/wasm.rs` - Added `InlineAsm` handler (generates comment)
- `CHANGELOG.md` - Documented changes
- `BRIEF_LANGUAGE_REFERENCE.md` - Documented inline asm syntax

---

## Expression Translation

Both backends translate Brief expressions to native code:

| Brief | Rust | C |
|-------|------|---|
| `count` | `self.count` | `state->count` |
| `count + 1` | `self.count + 1` | `state->count + 1` |
| `old.count` | `self.count` (prior) | `state->count` (prior) |
| `a == b` | `a == b` | `a == b` |
| `!cond` | `!cond` | `!cond` |

---

## Limitations

1. **Rust `asm!`** - Requires nightly Rust compiler; current output is commented template
2. **No runtime verification** - Generated code assumes contracts are satisfied
3. **Limited expression support** - Complex expressions may fall back to `0 /* expr not implemented */`
4. **No foreign bindings** - FFI calls not yet translated in native backends

---

## Testing

```bash
# Test Rust backend
cargo build --release
echo 'let count: Int = 0;
txn inc [true][true] { count = count + 1; };' > test.bv
./target/release/brief-compiler rust test.bv
cat test.rs

# Test C backend
./target/release/brief-compiler c test.bv
cat test.c

# Clean up
rm test.bv test.rs test.c
```
