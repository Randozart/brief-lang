# Brief v0.11 Overhaul Plan

**Date:** 2026-04-23
**Status:** Planning
**Scope:** All planned overhauls unified into single release

---

## Test Case References

Reference files (do not modify):
- `/home/randozart/Desktop/Projects/imp/kernel.ebv` - ARM kernel state machine
- `/home/randozart/Desktop/Projects/imp/neuralcore.ebv` - FPGA neural core
- `/home/randozart/Desktop/Projects/imp/hardware.toml` - Memory map
- `/home/randozart/Desktop/Projects/imp/arm/kernel.rs` - Current manual Rust kernel

---

## Part 1: Language Fixes (Crash Bug)

### 1.1 Unary Negation Support

**Problem:** `current_weight == -1` produces `/* Unsupported Expr: Neg(Integer(1)) */`

**Location:** Likely `src/backend/verilog.rs` or expression code generation

**Fix:** Traverse `Expr::Neg` operand and emit unary minus

**Test:** `neuralcore.ebv:91` - change `[current_weight + 1 == 0]` back to `[current_weight == -1]`

---

## Part 2: Compiler Enhancements (ENHANCEMENT_PLAN)

### 2.1 Address Space Access Verification

**Goal:** Detect when CPU code accesses FPGA-internal addresses

**Analysis:**
- Parse `hardware.toml` memory map
- Classify addresses:
  - DDR4 (0x00000000-0xFFFFFFFF): CPU accessible
  - MMIO (0x4000A000-0x4000AFFF, 0x8000A000-0x8000AFFF): CPU accessible
  - FPGA Internal (0x40A80000+): NOT CPU accessible
- In Rust FFI generator, emit ERROR if code writes to FPGA-internal addresses

**Example violation:** `kernel.rs` directly writes to `0x40A80000` (BRAM) - should error

### 2.2 Rust Struct Generator

**Goal:** Generate `State` struct from `hardware.toml`

**Flow:**
1. Parse `[memory]` section from hardware.toml
2. Sort by address, calculate field offsets
3. Generate `#[repr(C)]` struct:
   ```rust
   pub struct State {
       pub control: u32,        // 0x8000A000
       pub status: u32,          // 0x8000A004
       ...
   }
   ```

**Output modes:**
- **Validate:** Check existing kernel.rs matches hardware.toml, emit errors on mismatch
- **Generate:** Write/update kernel.rs in project (with diff shown)

### 2.3 Transaction Protocol Verification

**Goal:** Ensure preconditions are met before transactions fire

**Analysis:**
- Parse transaction preconditions from `.ebv`
- Track "protected" registers (e.g., `control` must be set before `write_en` can fire)
- Generate helper methods that enforce protocol:
  ```rust
  pub fn send_weights(&mut self, data: &[i16]) {
      self.control = 1;  // Compiler ensures this
      self.write_en = 1;
  }
  ```

### 2.4 Dataflow Analysis

**Goal:** Detect use-before-set errors

**Analysis:**
- Build dependency graph: `weight_buffer` must be loaded before computation
- Emit ERROR if computation fires without prior weight load
- Generate weight loading stubs with TODO markers

### 2.5 Entry Point Detection

**Goal:** Detect which transaction fires first from initial state

**Rules:**
1. Initial state: all `let` variables have their declared/zero values
2. Find all transactions where precondition is TRUE in initial state
3. If multiple non-async txns can fire → ERROR (ambiguous entry)
4. If multiple async txns can fire → OK (run concurrently), check for conflicts
5. First firing txn is the entry point

**Example:** `kernel.ebv:72-75` has `rct txn idle [kernel_state >= 0]` - this is TRUE at init (kernel_state=0), so it's the entry if no other txn also has TRUE precondition

### 2.6 Cross-Reference Address Validation

**Goal:** Verify addresses in `.ebv` match `hardware.toml`

**Analysis:**
- Collect all addresses from `.ebv` (`@ 0x...` declarations)
- Collect all addresses from `hardware.toml`
- Emit ERROR if `.ebv` uses address not in hardware.toml

---

## Part 3: Bare-Metal Language Extensions (MISSING_FEATURES)

### Philosophy

Brief-native syntax. Each word must be clear, predictable, inferrable. No magic.

### 3.1 File Type Inference

**Rule:** `.ebv` = embedded brief (no_std implied, MMIO volatility inferred)
**Rule:** `.bv` = standard Brief (full stdlib)

**Implementation:** Different compilation pipelines for each extension

### 3.2 MMIO Volatility Inference

**Rule:** Any access to address in MMIO range (`0x4000A000-0x4000AFFF`, `0x8000A000-0x8000AFFF`) is automatically volatile

**What can be inferred:**
- From hardware.toml MMIO range → volatile read/write
- No explicit `volatile` keyword needed

**Test case:** `neuralcore.ebv` accesses `0x8000A000` → generates `core::ptr::read_volatile`

### 3.3 Raw Pointer Inference

**Goal:** Don't require explicit pointer syntax

**Inference flow:**
1. Brief declares `let state: State @ 0x8000A000`
2. Compiler knows this is MMIO (from hardware.toml)
3. Compiler generates:
   ```rust
   const STATE_BASE: *mut State = 0x8000A000 as *mut State;
   pub fn get() -> &'static mut State { unsafe { &mut *STATE_BASE } }
   ```

**No explicit `*mut` syntax needed in Brief source**

### 3.4 Entry Point Generation

**Rule:** First non-async transaction to fire is entry point
**Rule:** Entry becomes `main()` or `_start()` depending on target

**Inference:**
- Analyze initial state
- Find entry txn
- Generate entry function

### 3.5 No-Standard Library (.ebv only)

**Rule:** `.ebv` files compile with `#![no_std]`
**Rule:** Reject `import std/*` in `.ebv`

### 3.6 Panic Handler Generation

**Default:** Generate minimal panic handler (UART puts if available)
**Override:** Allow custom panic handler via attribute if needed

### 3.7 Static Buffers (No Heap)

**Rule:** Brief vectors compile to fixed-size arrays in Rust (no Vec)
**Inference:** Length known at compile time → stack/static allocation

### 3.8 Embedded Data

**Status:** May not need syntax - weights loaded from DDR4 via DMA
**Future:** If needed, consider `#embed` attribute

### 3.9 Atomic Operations

**Status:** Defer - may be expressible via transactions
**Future:** If needed, explicit atomic types

### 3.10 Inline Assembly

**Status:** Defer - likely expressible in Brief
**Future:** If needed, `asm!` block

### 3.11 Watchdog (Commit Guard)

**Purpose:** Ensure transactions make progress or rollback

**Syntax:**
```
[pre][post]?[var]           // Optional watchdog - only checked if proof fails
[pre][post]![var]           // Required watchdog - always checked
[pre][post]?[var > threshold]  // Optional with condition
[pre][post]![var != initial_var]  // Required with comparison
```

**Rules:**
- `?[...]`: Only enforced when proof engine CANNOT prove halting
- `![...]`: Always enforced, even if proof succeeds
- Rollback: Reverts ALL assignments in transaction body
- Forbidden: `?[true]` - no point, always passes
- Watchdog expressions cannot be `true`

**Implementation:**
- Parser: Accept `?[expr]` and `![expr]` after postcondition
- AST: Add `watchdog: Option<WatchdogSpec>` to Transaction
- Proof Engine: Mark txn as "unprovable" when complex, emit flag
- Runtime: If watchdog fails at `term`, rollback all assignments

**Example:**
```
rct txn game_tick [frame_ready == true][?]![frame_processed] {
    &frame_processed = true;
    term;
}
```
If `frame_processed` doesn't change from initial, transaction rolls back.

### 3.12 Bit-Range and Vector ARM Rust Codegen

**Current Gap:**
- All signals generate as flat `u32`
- Vectors generate as single `u32` (not arrays)
- Bit ranges (`@/x16`, `@/0..6`) are ignored

**Required Fixes:**

#### 3.12.1 Type Mapping from Bit Range

| Bit Range | Brief Type | Rust Type |
|-----------|------------|-----------|
| `@/x1` - `@/x8` | `UInt[N]` | `u8` |
| `@/x1` - `@/x8` | `Int[N]` | `i8` |
| `@/x9` - `@/x16` | `UInt[N]` | `u16` |
| `@/x9` - `@/x16` | `Int[N]` | `i16` |
| `@/x17` - `@/x32` | `UInt[N]` | `u32` |
| `@/x17` - `@/x32` | `Int[N]` | `i32` |
| `@/x33` - `@/x64` | any | `u64` |
| `@/x65`+ | any | Packed multi-u32 |

#### 3.12.2 Vector Generation

```brief
let buf: Int[1024] @ 0x8000A000 /x16;
```
Should generate:
```rust
pub struct State {
    pub buf: [i16; 1024],  // 1024 x 16-bit elements
}
```

#### 3.12.3 Bit Packing for Large Bit Ranges

For `@/x33` and above, implement manual bit packing:

```brief
let wide: UInt[1] @ 0x8000A000 /x64;
```
Should generate:
```rust
pub struct State {
    pub wide_lo: u32,  // bits 0-31
    pub wide_hi: u32,  // bits 32-63
}
```

With getter/setter methods that mask.

#### 3.12.4 Vector Operations (Lifting)

```brief
let vec: Int[1024];
vec + 1;  // Add 1 to all elements
```

Should generate iterator-based or manual loop:
```rust
for elem in self.vec.iter_mut() {
    *elem = elem.wrapping_add(1);
}
```

### 3.13 Automatic Bit Packing and Inference

#### 3.13.1 Proof-Driven Bit Width Inference

**Purpose:** Automatically determine smallest possible bit width based on runtime value analysis.

**Flow:**
1. Parse explicit bit ranges: `let x: Int @/x16` (user-specified)
2. Run proof engine to analyze all possible values
3. If no explicit range specified and proof can determine bounds:
   - `0..255` → infer `@/x8`
   - `-128..127` → infer `@/x8`
   - `0..1000000` → infer `@/x20`
4. Verify explicit user specifications don't overflow:
   - User says `@/x8` but proof shows max=1000 → ERROR
5. Generate Rust type based on inferred/explicit width

**Proof Engine Requirements:**
- Track min/max of all variables
- Handle FFI with contract-specified ranges
- Mark unverifiable variables (external input) as needing explicit annotation

#### 3.13.2 Intelligent Bit Packing

**When Packing Occurs:**
- Multiple variables share same `@ address` without explicit `/bit`
- Variables with no `@ address` can share implicit address space

**Algorithm:**
```
1. Sort unpacked variables by size (smallest first for efficiency)
2. For each variable:
   a. If has explicit /bit → place at that exact bit
   b. If shares address with others → pack sequentially into free bits
   c. If no address → find free space in global "scratch" area
3. Verify no overlapping ranges
4. Generate bitmask getters/setters for packed access
```

**Example - Shared Address:**
```brief
let flags: UInt @ 0x8000A000;
let counter: UInt @ 0x8000A000;
let flag: Bool @ 0x8000A000;
```
**Packed result:**
```rust
pub struct State {
    pub flags: u32,       // @ 0x8000A000, bits 0-31
    pub counter: u32,     // @ 0x8000A000, bits 32-63
}

impl State {
    pub fn get_flag(&self) -> bool {
        (self.flags >> 32) & 1 != 0
    }
    pub fn set_flag(&mut self, val: bool) {
        if val {
            self.flags |= 1 << 32;
        } else {
            self.flags &= !(1 << 32);
        }
    }
}
```

#### 3.13.3 Overflow Verification

**For explicit bit ranges:**
```brief
let danger: UInt @/x8;  // User claims max 255
```
**Proof check:**
```
if proof shows danger can be > 255 → ERROR:
  "Overflow: variable 'danger' annotated with /x8 (max 255) 
   but proof shows possible value up to 1000"
```

**For FFI without contracts:**
```
WARNING: Cannot verify bit range for 'external_input' 
         (uncontracted FFI). Add explicit /bit annotation.
```

#### 3.13.4 Syntax Summary

| Syntax | Meaning |
|--------|---------|
| `let x: UInt` | No address, no bit range → pack anywhere |
| `let x: UInt @ addr` | Fixed address, auto bit range |
| `let x: UInt @/x16` | Auto address, fixed 16-bit range |
| `let x: UInt @ addr /x16` | Fixed address, fixed 16-bit range |
| `let x: Bool @ addr /6` | Fixed address, bit 6 |

---

## Part 4: Target Abstraction (TARGET_ABSTRACTION)

### 4.1 Hardware Library Structure

```
brief-compiler/
├── hardware_lib/
│   ├── targets/
│   │   ├── xilinx_ultrascale_plus.toml
│   │   └── (future: intel_stratix.toml)
│   └── interfaces/
│       ├── axi4_lite.toml
│       └── (future: wishbone.toml)
```

### 4.2 Implementation

1. Create loader in `src/hardware/mod.rs`
2. Target profile provides:
   - Memory pragmas (BRAM, UltraRAM, flipflop)
   - Toolchain commands
3. Interface profile provides:
   - Port mapping (clean name → AXI physical)
   - Wrapper template

### 4.3 Integration

- `hardware.toml` references target/interface by name
- Backend loads profile at compile time
- Generates pure SystemVerilog (no pragmas in source)

---

## Implementation Phases

### Phase 1: Fix Crash Bug
- Unary negation support

### Phase 2: Core Analysis Infrastructure
- Address space analyzer
- Struct generator (validate mode)
- Cross-reference validation
- Entry point detection

### Phase 3: Bare-Metal Code Generation
- MMIO volatility inference
- Pointer inference
- Entry point generation
- No-std for .ebv
- Vector + bit_range ARM codegen
- Automatic bit packing

### Phase 4: Full Generator
- Struct generator (generate mode)
- Protocol verification
- Dataflow analysis
- Helper method generation
- Watchdog implementation

### Phase 5: Target Abstraction
- Hardware library loader
- TOML profiles
- Backend integration

---

## Files to Modify/Create

| File | Changes |
|------|---------|
| `src/ast.rs` | May need Neg variant fix |
| `src/backend/verilog.rs` | Unary negation, volatility |
| `src/backend/rust.rs` | No-std, struct generator |
| `src/analysis/address_space.rs` | New: address classification |
| `src/analysis/dataflow.rs` | New: dependency tracking |
| `src/verifier/entry_point.rs` | New: entry detection |
| `src/verifier/protocol.rs` | New: protocol verification |
| `src/analysis/bit_packing.rs` | New: automatic bit packing |
| `src/analysis/bit_inference.rs` | New: proof-driven bit width inference |
| `src/hardware/mod.rs` | New: TOML loader |
| `hardware_lib/targets/*.toml` | New: target profiles |
| `hardware_lib/interfaces/*.toml` | New: interface profiles |
| `BRIEF_v011_PLAN.md` | This document |

---

## Test Cases

### Unit Tests
- Unary negation: `-1`, `-x`, `-(a + b)`
- Address classification: MMIO vs FPGA internal
- Entry point: single entry, multiple async, ambiguous non-async
- Watchdog: optional vs required, rollback behavior

### Integration Tests (create `test_cases/v011/`)
- `mmio_inference.ebv` - MMIO at 0x8000A000
- `entry_point_single.ebv` - one txn at init
- `entry_point_ambiguous.ebv` - multiple non-async (should error)
- `address_violation.ebv` - writes to 0x40A80000 (should error)
- `struct_match.ebv` + `hardware.toml` - struct generator test
- `watchdog_optional.ebv` - optional watchdog test
- `watchdog_required.ebv` - required watchdog test
- `watchdog_rollback.ebv` - rollback on failure test

---

## Success Criteria

After v0.11, the compiler must:

| Feature | Criteria |
|---------|----------|
| Unary negation | `current_weight == -1` compiles correctly |
| Entry detection | Identifies `idle` as entry in kernel.ebv |
| MMIO volatility | Generates volatile Rust for 0x8000A000 access |
| Address validation | Errors on write to 0x40A80000 from CPU code |
| Struct generation | Produces matching `#[repr(C)]` struct |
| No-std | `.ebv` compiles without std imports |
| Target abstraction | Loads Xilinx profile from TOML |
| Watchdog optional | `?[var]` only enforced when proof fails |
| Watchdog required | `![var]` always enforced |
| Watchdog rollback | Failed watchdog reverts all assignments |
| Vector codegen | `Int[1024] @/x16` generates `[i16; 1024]` |
| Bit range mapping | `@/x8` → `u8`, `@/x18` → `u32` |
| Bit overflow check | Explicit `/x8` with value>255 → ERROR |
| Bit packing | Multiple vars at same address pack efficiently |