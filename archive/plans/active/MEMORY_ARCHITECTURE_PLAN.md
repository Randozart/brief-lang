# Brief Compiler - Memory Architecture Extension Plan

**Date:** 2026-04-27
**Revised:** 2026-04-27 (minimal scope)
**Purpose:** Native Brief syntax for IMP v1.4 split-DDR memory architecture
**Target:** Both Rust and C transpilation

---

## Executive Summary

After reviewing existing codebase and IMP requirements, the plan scope is **dramatically reduced**. Most features already exist or can be handled via `hardware.toml` configuration. Only **two features** require new syntax:

1. **`link`** - Cross-target IO linkage between SystemVerilog and Rust/C
2. **`asm`** - Architecture-gated inline assembly for `.bv` (optional, Von Neumann only)

All other requirements from `FEATURE_REQUIREMENTS.md` are already addressable with existing syntax or `hardware.toml` configuration.

**Core Philosophy:** "Compiler figures it out" by default. Memory management is automatic; explicit syntax only needed for cross-target linkage and low-level control.

---

## File Type Clarification

| File Type | Purpose | Transpilation Target | Memory Model |
|-----------|---------|----------------------|--------------|
| `.bv` | Brief specification | Rust, C | Virtual memory - compiler uses OS allocator optimally |
| `.rbv` | Rendered Brief + View | WASM + JS (frontend) | N/A (view layer only) |
| `.ebv` | Embedded Brief + Hardware | Rust/C + SystemVerilog | Physical addresses via `hardware.toml` |

**Transpilation Rules:**
- `.bv` → **Rust/C only** (never SystemVerilog)
- `.ebv` → **SV + Rust/C** (both by default, or Rust/C standalone)

---

## Part I: IMP Requirements Coverage

This section maps each requirement from `FEATURE_REQUIREMENTS.md` to existing Brief syntax, showing that **most gaps are already addressed**.

### Requirement 1: Memory Region Definitions

**What IMP Needs:**
```
DDR Low Bank:    0x00000000 - 0x7FFFFFFF
DDR High Bank:   0x800000000 - 0x87FFFFFFF
GAP:             0x80000000 - 0x7FFFFFFF
```

**Existing Solution:** `hardware.toml` already defines memory regions:

```toml
[memory]
"0x00000000" = { size = 2147483648, type = "ddr_low", element_bits = 64 }
"0x800000000" = { size = 2147483648, type = "ddr_high", element_bits = 64 }
```

**Status:** ✅ **Already Supported** - Extend `hardware.toml` schema to support DDR bank types instead of new syntax.

### Requirement 2: Gap-Jumping Address Translation

**What IMP Needs:**
```c
uint64_t get_weight_addr(uint64_t virtual_index) {
    if (virtual_idx < MODEL_PART_A_SIZE) {
        return 0x0 + virtual_index;  // Low bank direct
    } else {
        return 0x800000000 + (virtual_index - MODEL_PART_A_SIZE);  // Jump gap
    }
}
```

**Existing Solution:** Standard ternary expressions already work:
```brief
let phys_addr = (virtual_idx < MODEL_PART_A_SIZE)
    ? 0x0 + virtual_idx
    : 0x800000000 + (virtual_idx - MODEL_PART_A_SIZE);
```

**Status:** ✅ **Already Supported** - Ternary `? :` expressions support conditional address selection. No new syntax needed.

### Requirement 3: Cache Allocation

**What IMP Needs:**
```c
cache_t cache;
cache.low_base = MODEL_A_END;
cache.low_size = GAP_START - MODEL_A_END;
```

**Existing Solution:** Vector types + hardware.toml sizing:
```brief
let kv_cache: Int[?remaining] @ hardware.toml;  // Compiler fills from available
```

**Status:** ⚠️ **Partial** - Vector sizing with `?` exists, but `remaining` keyword not implemented. May extend `hardware.toml` with `auto_allocate: true` instead of new syntax.

### Requirement 4: DMA Descriptor Setup

**What IMP Needs:**
```c
DMA->SRC_ADDR = get_weight_addr(current_weight_idx);
DMA->DST_ADDR = FPGA_BRAM_BASE;
DMA->LEN = TRANSFER_SIZE;
DMA->CTRL |= START;
```

**Existing Solution:** Transactions with triggers already handle register writes:
```brief
trg dma_src: UInt @ 0x80040000 /0..31;
trg dma_dst: UInt @ 0x80040004 /0..31;
trg dma_len: UInt @ 0x80040008 /0..15;

node setup_dma [true] {
    &dma_src = get_weight_addr(current_idx);
    &dma_dst = FPGA_BRAM_BASE;
    &dma_len = 524288;
    term;
};
```

**Status:** ✅ **Already Supported** - Individual `trg` declarations work; grouping is cosmetic.

### Requirement 5: Multi-Bank Memory Access Pattern

**What IMP Needs:**
```
Boot: Load Part A → 0x00000000, Load Part B → 0x800000000
Inference: Read weights from either bank
```

**Existing Solution:** Transaction preconditions + address arithmetic:
```brief
node read_weight [idx < PART_A_SIZE] {
    &phys_addr = 0x0 + idx;
    term;
};

node read_weight_b [idx >= PART_A_SIZE] {
    &phys_addr = 0x800000000 + (idx - PART_A_SIZE);
    term;
};
```

**Status:** ✅ **Already Supported** - Separate transactions handle different banks, proven by `kernel.ebv` patterns.

### Requirement 6: Weight Streaming from DDR to FPGA

**What IMP Needs:**
```c
for (offset = 0; offset < model_size; offset += CHUNK_SIZE) {
    FPGA->WEIGHT_ADDR = offset;
    FPGA->DDR_ADDR = get_weight_addr(offset);
    FPGA->TRANSFER_LEN = CHUNK_SIZE;
    FPGA->START_STREAM = 1;
    wait_for(FPGA->DONE);
}
```

**Existing Solution:** Transactions ARE the iteration mechanism (proof-friendly loops):
```brief
node stream_next [pending > 0] [stream_state == streaming] {
    &offset = (stream_state == idle) ? 0 : offset + CHUNK_SIZE;
    &fpga_addr = offset;
    &ddr_addr = get_weight_addr(offset);
    &pending = pending - 1;
    term;
};
```

**Status:** ✅ **Already Supported** - Transactions serve as implicit loops via watchdogs. No `for` syntax needed.

### Requirement 7: Cache Coherency Operations

**What IMP Needs:**
```asm
DC CIVAC X0, X1    // ARMv8 cache flush
DSB SY             // Data synchronization barrier
```

**Existing Solution:** None currently - **this is the primary new feature**.

**Status:** ❌ **Requires New Syntax** - See Feature 2 below.

### Requirement 8: Peripheral Register Definitions

**What IMP Has:**
```brief
trg hw_control: UInt @ 0x40000000 /0..7;
trg hw_status: UInt @ 0x40000004 /0..7;
```

**What IMP Needs:**
```brief
trg dma_src: UInt @ 0x80040000 /0..31;
trg fpga_weight_addr: UInt @ 0x8000A040 /0..17;
```

**Status:** ✅ **Already Supported** - `trg` with `@ address` works for any address.

**Linkage Note:** The `link` keyword would allow sharing these across SV+Rust/C without hardcoding addresses.

### Requirement 9: Interrupt Handling

**What IMP Needs:**
```c
void layer_complete_isr(void) {
    pending_layers--;
    if (pending_layers == 0) signal_tokens_ready();
}
```

**Existing Solution:** `trg` for interrupt variables, backend generates ISR:
```brief
trg layer_complete_irq: Bool @ 0x40000010;

node handle_irq [layer_complete_irq] [layer_complete_irq == false] {
    &pending_layers = pending_layers - 1;
    term;
};
```

**Status:** ✅ **Already Supported** - Backend must generate interrupt vector table and ISR registration. This is a **backend implementation task**, not new syntax.

### Requirement 10: Multi-Process Memory Isolation

**What IMP Needs:**
```
Process A (9B Model): 0x0 - 0x7FFFFFFF
Process B (Context):  0x800000000 - 0x87FFFFFFF
Kernel:              0x00100000 - 0x00400000
```

**Existing Solution:** `hardware.toml` defines regions, OS manages isolation:
```toml
[memory.isolation]
kernel_region = "0x00100000..0x00400000"
model_region = "0x00000000..0x7FFFFFFF"
context_region = "0x800000000..0x87FFFFFFF"
```

**Status:** ⚠️ **Partial** - TOML defines regions; actual MMU configuration is target-specific (bare-metal vs OS).

---

## Part II: Genuinely New Features

Only two features require new syntax that doesn't exist in the current codebase.

---

## Feature 1: IO Linkage (`link`)

**Applies to:** `.ebv` exclusively
**Purpose:** Share IO pins/addresses between SV and Rust/C without hardcoding concrete addresses in Brief source

### Rationale
When `.ebv` transpiles to both SV and Rust/C, concrete addresses/wires must be agreed upon by both sides. Anonymous `link` references enable this without polluting Brief source with target-specific details.

### Proposed Syntax
```brief
// linkage.toml (shared config)
[fpga_io]
weight_valid = { sv: "fpga_weight_valid_wire", rust: "0x8000A040", c: "0x8000A040" }
result_data = { sv: "fpga_result_data_wire", rust: "0x8000A050", c: "0x8000A050" }

// .ebv source - anonymous linkage
trg weight_valid @ link;
trg result_data @ link;

// Instead of hardcoded:
trg weight_valid: Bool @ 0x8000A040;
trg result_data: UInt @ 0x8000A050;
```

### Compiler Behavior
1. Parse `.ebv`, emit triggers with `LinkRef::Linked(name)` addresses
2. Read `linkage.toml` to resolve concrete values per target
3. Generate SV: `wire fpga_weight_valid_wire; assign weight_valid = fpga_weight_valid_wire;`
4. Generate Rust: `let weight_valid = 0x8000A040;`
5. Generate C: `#define WEIGHT_VALID 0x8000A040`
6. If `linkage.toml` missing, error with helpful message

### Implementation
- **Lexer:** Add `Link` token
- **AST:** `LinkRef::Explicit(u64) | Linked(String)`
- **Parser:** Recognize `@ link` syntax
- **linkage.rs:** New file - read and resolve linkage config
- **Codegen:** Resolve `LinkRef::Linked` to concrete values per target

### Files Modified
- `src/lexer.rs` - Add `Link` token
- `src/ast.rs` - Add `LinkRef` enum
- `src/parser.rs` - Handle `@ link` syntax
- `src/linkage.rs` - **NEW** - linkage config reader
- `src/wasm_gen.rs` - Resolve linked addresses
- `src/codegen.rs` - Resolve linked addresses
- `src/sv_gen.rs` - Resolve linked wire names

---

## Feature 2: Inline Assembly (`asm`)

**Applies to:** `.bv` only (architecture-gated)
**Purpose:** Optional low-level control for Von Neumann architectures; useless for Moore-style hardware (stored-program not required)

### Rationale
Cache coherency operations (DC CIVAC, DSB SY), DSP intrinsics, or other architecture-specific instructions. Only makes sense for targets with Von Neumann architecture.

### Architecture Gating
```brief
// .bv targeting x86/ARM/RISC-V can use asm
asm "DC CIVAC X0, X1" { "x0", "x1" };

// .ebv targeting Moore hardware generates error or warning
// (Moore: no stored program, no assembly有意义)
```

### Proposed Syntax
```brief
txn flush_cache_for_dma [true] {
    asm "DC CIVAC X0, X1" { "x0", "x1" };
    asm "DSB SY" {};
    term;
};
```

### Implementation
- **Lexer:** `Asm` token
- **AST:** `InlineAsm { asm_string, clobbers: Vec<String> }`
- **Parser:** `parse_asm_block()`
- **Typechecker:** Verify target architecture supports inline assembly
- **Codegen:**
  - Rust: `core::arch::asm!()`
  - C: `__asm__ __volatile__()`
  - SV: Error or warning (Moore architecture)

### Files Modified
- `src/lexer.rs` - Add `Asm` token
- `src/ast.rs` - Add `InlineAsm` type
- `src/parser.rs` - Add `parse_asm()`
- `src/typechecker.rs` - Architecture validation
- `src/wasm_gen.rs` - Handle inline asm
- `src/codegen.rs` - Handle inline asm

---

## Part III: Requirements to Implementation Mapping

### IMP Requirements vs Brief Features

| IMP Requirement | Brief Solution | Status |
|----------------|----------------|--------|
| 1. Memory Regions | `hardware.toml` `[memory]` section | ✅ Already supported |
| 2. Gap-Jumping Translation | Ternary `? :` expressions | ✅ Already supported |
| 3. Cache Allocation | Vector sizing + `hardware.toml` | ⚠️ May need `auto_allocate` in TOML |
| 4. DMA Descriptor Setup | `trg` + transactions | ✅ Already supported |
| 5. Multi-Bank Access | Transactions with preconditions | ✅ Already supported |
| 6. Weight Streaming | Transactions as implicit loops | ✅ Already supported |
| 7. Cache Coherency | **`asm` keyword** | ❌ New feature |
| 8. Peripheral Registers | `trg` with `@ address` | ✅ Already supported |
| 9. Interrupt Handling | `trg` variables (backend ISR gen) | ⚠️ Backend implementation |
| 10. Memory Isolation | `hardware.toml` regions | ⚠️ TOML schema extension |

### New Syntax Summary

| Feature | Keywords | Files Modified |
|---------|----------|----------------|
| IO Linkage | `link` | lexer, ast, parser, linkage.rs, codegen, sv_gen |
| Inline Assembly | `asm` | lexer, ast, parser, typechecker, codegen |

---

## Part IV: Deferred Features

The following were in the original plan but are **deferred** based on review:

| Feature | Reason Deferred |
|---------|----------------|
| `region` keyword | Redundant with `hardware.toml` `[memory]` |
| `block`/`reg` keywords | Individual `trg`/`let @` already work |
| `for`/`range()` loops | Transactions serve as proof-friendly loops |
| `handler`/`interrupt` | `trg` already for interrupts; ISR gen is backend task |
| `domain` protection | Needs more design; OS handles isolation for `.bv` |

---

## Implementation Phases

### Phase 1: IO Linkage
- Add `link` keyword + `LinkRef` AST
- Add `linkage.toml` reader
- Update SV, Rust, C codegen to resolve linked references
- Test with existing `.ebv` files

### Phase 2: Inline Assembly
- Add `asm` keyword + `InlineAsm` AST
- Add architecture validation in typechecker
- Update Rust/C codegen for asm blocks
- SV target: warn/error (Moore architecture)

### Phase 3: IMP Test Integration
- Compile `kernel.ebv` and `neuralcore.ebv` with new features
- Verify generated SV + Rust/C outputs
- Verify `link` resolves correctly via `linkage.toml`

---

## Backward Compatibility

**All additions are pure extensions:**
- `link` only activates when `@ link` syntax appears
- `asm` only activates when `asm "..."` syntax appears
- No existing `.bv`, `.rbv`, or `.ebv` files need changes

---

## Verification Plan

```bash
# 1. Build and run existing tests
cargo test --lib

# 2. Build the compiler
cargo build

# 3. Test new link syntax parsing
./target/release/brief-compiler ebv examples/linkage_test.ebv

# 4. Verify SV output with linkage
./target/release/brief-compiler ebv examples/linkage_test.ebv --output sv

# 5. Verify Rust output with linkage
./target/release/brief-compiler ebv examples/linkage_test.ebv --output rust

# 6. Test asm syntax (if target supports it)
./target/release/brief-compiler bv examples/asm_test.bv --output rust
```

---

## Success Criteria

1. **Linkage:** `.ebv` with `@ link` transpiles to SV + Rust/C with correctly resolved addresses
2. **Assembly:** `.bv` with `asm` transpiles to Rust/C; `.ebv` with `asm` warns/errors appropriately
3. **Backward Compat:** All existing files continue to work unchanged
4. **IMP Integration:** `kernel.ebv` and `neuralcore.ebv` compile successfully

---

## Appendix A: IMP Project File Mapping

Reference files from `FEATURE_REQUIREMENTS.md`:

| File | Path | Purpose |
|------|------|---------|
| `kernel.ebv` | `/imp/kernel.ebv` | ARM software layer state machine |
| `neuralcore.ebv` | `/imp/neuralcore.ebv` | FPGA neural inference engine |
| `hardware.toml` | `/imp/hardware.toml` | KV260 memory map |
| `kernel.toml` | `/imp/kernel.toml` | ARM peripheral interface |

### Current IMP Syntax Usage

**Peripheral registers (already working):**
```brief
trg hw_control: UInt @ 0x40000000 /0..7;
trg hw_status: UInt @ 0x40000004 /0..7;
trg hw_opcode: UInt @ 0x40000008 /0..3;
```

**Vector buffers (already working):**
```brief
let weight_buffer: Int[262144] @ 0x40A80000 / x16;
let scratch: Int[262144] @ 0x40B00000 / x16;
```

**Address arithmetic (already working):**
```brief
let phys_addr = (virtual_idx < MODEL_PART_A_SIZE)
    ? 0x0 + virtual_idx
    : 0x800000000 + (virtual_idx - MODEL_PART_A_SIZE);
```

### IMP Requirements Not Yet Met

1. **`link`**: `neuralcore.ebv` and `kernel.ebv` share signals but no linkage mechanism
2. **`asm`**: Cache flush before DMA not expressible in current Brief

---

## Appendix B: Quick Reference - Existing Syntax

### Hardware Registers
```brief
trg name: Type @ address [/bit-range];
let name: Type @ address [/bit-range] = init;
```

### Address Modes
```brief
@address         // Target-dependent address
@raw:0xADDRESS   // Raw physical (embedded)
@stack:OFFSET     // Stack-relative
@heap:OFFSET      // Heap-relative
```

### Bit Ranges
```brief
@/N              // Bit at position N
@/M..N           // Bit range M to N
@/xN             // Any N-bit slot
```

### Transactions (Implicit Loops)
```brief
node name [pre][post][?watchdog] {
    // body - executes when precondition is true
}
```

---

*End of Plan*
