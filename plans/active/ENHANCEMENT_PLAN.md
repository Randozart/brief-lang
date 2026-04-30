# Brief Compiler Enhancement Plan: Catch Hardware Violations at Compile Time

**Date:** 2026-04-22
**Status:** Planning

## Problem Statement

During IMP development, multiple critical hardware-level bugs were found by manual code review that the Brief compiler *should* have caught:

1. **Address Space Violation** - ARM code accessed FPGA internal BRAM directly (would cause Data Abort)
2. **MMIO Struct Mismatch** - Rust `State` struct didn't match `hardware.toml` memory map
3. **Transaction Protocol Violation** - ARM didn't set `control` before triggering transactions
4. **Missing Dataflow** - Weights not loaded before computation (all outputs zero)
5. **Unary Minus Bug** - Compiler crashed on `-1` in expressions, outputting `/* Unsupported Expr: Neg(Integer(1)) */`
6. **Address Inconsistency** - `hardware.toml` and `neuralcore.ebv` had mismatched `read_data` addresses (`0x40000050` vs `0x4000A050`)

These bugs survived past multiple reviews and would have caused hardware crashes or silent failures at runtime.

---

## Required Compiler Enhancements

### 1. Address Space Access Verification

**Problem:** ARM code wrote directly to `0x40A80000` (internal FPGA BRAM), which is not accessible from the CPU.

**Current behavior:** Compiler defines `weight_buffer` at `0x40A80000` in `.ebv` and generates hardware, but has no way to know Rust code is trying to access this address directly.

**Required analysis:**
- Track all address literals (`0x40A80000`, etc.) used in code
- Know which addresses are FPGA-internal vs MMIO-external
- Emit error if code accesses FPGA-internal addresses from ARM/CPU code

**Implementation approach:**
```
1. Parse hardware.toml to build address space map
2. Classify addresses:
   - DDR4 (0x00000000-0xFFFFFFFF): CPU accessible
   - MMIO (0x4000A000-0x4000AFFF): CPU accessible via AXI4-Lite
   - FPGA Internal (0x40A80000+): NOT directly accessible from CPU
3. In FFI generator, warn/error on accesses to FPGA Internal ranges
```

### 2. Rust Struct Generator from hardware.toml

**Problem:** `State` struct in `kernel.rs` had wrong field layout:
```rust
// kernel.rs had (WRONG):
pub struct State {
    pub control: u32,
    pub status: u32,
    pub opcode: u32,
    pub token_count: u32,
    pub input_embedding: u32,  // 0x40A80000 - NOT in MMIO range!
    ...
}

// hardware.toml defines:
"0x4000A040" = { write_data }
"0x4000A044" = { write_addr }
"0x4000A048" = { write_en }
```

**Required analysis:**
- Parse `hardware.toml` memory map
- Generate matching `#[repr(C)]` struct with correct field offsets
- Include padding fields to maintain alignment

**Implementation approach:**
```
1. Parse [memory] section from hardware.toml
2. Sort entries by address
3. Generate Rust struct:
   #[repr(C)]
   pub struct State {
       pub control: u32,        // 0x4000A000
       pub status: u32,          // 0x4000A004
       pub opcode: u32,          // 0x4000A008
       pub token_count: u32,    // 0x4000A00C
       _pad0: [u32; 12],        // Padding to 0x40
       pub write_data: i32,     // 0x4000A040
       pub write_addr: u32,     // 0x4000A044
       pub write_en: u32,       // 0x4000A048
       ...
   }
4. Generate From hardware.toml conversions
```

### 3. Transaction Protocol Verification

**Problem:** Transaction in `.ebv`:
```brief
rct txn load_weights [cpu_write_en && control == 1][...] {
```
has precondition `control == 1`, but code set `write_en` without setting `control` first.

**Required analysis:**
- Parse transaction preconditions from `.ebv`
- Generate helper methods that enforce protocol
- Warn/error if protocol not followed

**Implementation approach:**
```
1. Parse transaction preconditions from .ebv
2. Generate Rust methods with built-in protocol enforcement:
   pub fn send_weights(&mut self, data: &[i16]) {
       self.state.control = 1;  // <-- Compiler enforces this
       self.state.write_addr = i as u32;
       self.state.write_data = value as i32;
       self.state.write_en = 1;
       self.state.write_en = 0;
   }
3. Mark registers as "write-protected" until protocol followed
```

### 4. Dataflow Analysis: Weight Loading Requirement

**Problem:** Inference loop sent input tokens but never loaded weights from DDR4 to FPGA. Result: all computations multiplied by zero.

**Required analysis:**
- Track that `weight_buffer` must be populated before computation
- Verify dataflow: DDR4 → FPGA BRAM → Computation

**Implementation approach:**
```
1. Build dependency graph:
   - weight_buffer: defined empty, must be loaded before use
   - scratch: defined empty, must be loaded before use
   - compute_ternary: requires weight_buffer AND scratch
2. Emit error if computation attempted without prior load
3. Generate weight loading stubs with TODO markers
```

---

## Implementation Phases

### Phase 1: Address Space Analyzer
- Parse `hardware.toml` address classifications
- Track address literals in generated code
- Emit warnings for FPGA-internal accesses from CPU code

### Phase 2: Struct Generator
- Generate `#[repr(C)]` structs from `hardware.toml`
- Generate `hardware.rs` with type-safe field access
- Include doc comments linking to TOML source

### Phase 3: Protocol Enforcer
- Parse transaction preconditions from `.ebv`
- Generate protocol-aware wrapper methods
- Emit errors on missing protocol steps

### Phase 4: Dataflow Tracker
- Build memory dependency graph
- Emit errors on use-before-set
- Generate weight streaming stubs

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/backend/rust.rs` | New: Generate `#[repr(C)]` structs from hardware.toml |
| `src/analysis/address_space.rs` | New: Analyze address accessibility |
| `src/analysis/dataflow.rs` | New: Track memory dependencies |
| `src/verifier/protocol.rs` | New: Verify transaction protocols |

---

## Success Criteria

After implementation, the compiler must catch:

| Bug | Detection Method |
|-----|------------------|
| Direct access to `0x40A80000` | Address space analyzer |
| Wrong MMIO field offsets | Struct generator output mismatch |
| Missing `control = 1` before `write_en` | Protocol enforcer |
| Computation without weight load | Dataflow tracker |

---

### 5. Unary Negation Support (CRITICAL - Compiler Bug)

**Problem:** The Brief compiler's AST cannot handle unary negation. Expression `current_weight == -1` produced:
```verilog
if ((current_weight == /* Unsupported Expr: Neg(Integer(1)) */)) begin
```

**Root cause:** Incomplete AST node for unary `-` operator. The `Neg` variant exists in the AST but code generation fails to traverse it properly.

**Required fix:**
```
1. Find where Expr::Neg is handled in code generation (likely verilog.rs or similar)
2. Ensure operand is recursively generated
3. Emit unary minus operator in target language
```

**Workaround used:** `[current_weight + 1 == 0]` instead of `[current_weight == -1]`

**Files to examine:**
- `src/ast.rs` - Verify Neg variant exists
- `src/backend/verilog.rs` - Check expr generation for unary ops

### 6. Cross-Reference Address Validation

**Problem:** `hardware.toml` had `read_data` at `0x40000050` but `neuralcore.ebv` also said `0x40000050`. These should both be `0x4000A050`. No validation caught this.

**Required analysis:**
- Parse all addresses from `.ebv` and `hardware.toml`
- Verify they match for corresponding definitions
- Error on mismatch between source files

**Implementation approach:**
```
1. Collect all address references from .ebv
2. Collect all address definitions from hardware.toml
3. Cross-reference: any address used in .ebv must exist in hardware.toml
4. Cross-reference: addresses should be in same AXI peripheral block (0x4000Axxx)
```

---

## References

- Issue discovered during IMP KV260 development
- `hardware.toml` - Memory map definition
- `neuralcore.ebv` - Transaction and precondition definitions
- `kernel.rs` - ARM software with violations
