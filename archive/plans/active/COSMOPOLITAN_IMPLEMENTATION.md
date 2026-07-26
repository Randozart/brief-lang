# Brief Cosmopolitan Architecture Implementation

**Version:** 1.0  
**Date:** 2026-05-01  
**Status:** Phase 2 Complete - Moving to Phase 3

## Executive Summary

This document details the implementation plan to fully realize the Brief Cosmopolitan Architecture: a single `brief compile` command that adapts to any target via TOML configuration.

---

## Current State (Phase 2 Complete ✅)

### What's Done
- `target_spec` module with TOML loader (`src/target_spec/`)
- CBackend refactored to use `TargetSpec` (`src/backend/c.rs`)
- `linux_kernel.toml` example target (`lib/targets/`)
- Basic `--target` flag support in CLI (`src/main.rs`)

### Verified Working
```bash
# Default: hosted C
./brief-compiler c test.bv → #include <stdlib.h>, main()

# With target spec
./brief-compiler c test.bv --target lib/targets/linux_kernel.toml → Linux kernel module
```

---

## Target State (Phase 5)

### The Unified Command
```bash
brief compile <SOURCE> --target <CONFIG.toml>
```

### Capabilities System
Each target TOML declares supported capabilities:
```toml
[target]
name = "react-web"
backend = "react"
capabilities = ["logic", "reactive_ui"]  # .bv and .rbv allowed
```

---

## Implementation Phases

### Phase 3: Unified CLI Interface

**Goal:** Add `brief compile` command and capability validation.

#### Tasks

1. **Add `compile` subcommand to CLI**
   - File: `src/main.rs`
   - Replace/alias all existing commands to use unified flow
   - `brief compile <file> --target <spec>` as primary interface

2. **Capability Validation**
   - File: `src/target_spec/mod.rs`
   - Add `capabilities: Vec<String>` to `TargetSpec`
   - Add validation in parser/typechecker
   - Error: `B4001: Target lacks capability`

3. **Source Type Detection**
   - Detect `.bv`, `.rbv`, `.ebv` from file extension
   - Map to semantic layers

#### Deliverables
- `brief compile` command working
- Capability validation errors
- Source type detection

---

### Phase 4: Target Spec Library

**Goal:** Create target specs for all supported backends.

#### Tasks

1. **Create `lib/targets/` directory structure**
   ```
   lib/targets/
   ├── README.md
   ├── hosted_c.toml          # Default C (main())
   ├── linux_kernel.toml       # Linux kernel module ✅ (exists)
   ├── arm_el1.toml          # ARM bare-metal
   ├── react_web.toml         # React TypeScript
   ├── webgpu_wgsl.toml     # WebGPU compute
   ├── python_numpy.toml    # Python with NumPy
   ├── rust_std.toml        # Native Rust
   ├── cobol_mainframe.toml  # IBM Enterprise COBOL
   └── verilog_fpga.toml   # SystemVerilog for FPGA
   ```

2. **Each target spec includes:**
   ```toml
   [target]
   name = "..."
   backend = "c" | "react" | "wgsl" | etc.
   capabilities = ["logic", "reactive_ui", "hardware_triggers", "mmio"]
   
   [codegen]
   state_allocation = "static" | "dynamic"
   entry_point = "main" | "module_init" | "_start"
   
   [ffi]
   # Type mappings for this target
   
   [templates]
   header = "..."
   footer = "..."
   ```

#### Deliverables
- 9 target specs in `lib/targets/`
- Each validated to work with test files

---

### Phase 5: Multi-Backend Integration

**Goal:** Wire all backends to use TargetSpec system.

#### Tasks

1. **TypeScript/React Backend**
   - File: `src/backend/typescript.rs` (or `react.rs`)
   - Refactor to accept `TargetSpec`
   - Implement `reactive_ui` capability
   - Generate `.tsx` from view blocks

2. **WebGPU/WGSL Backend**
   - File: `src/backend/wgsl.rs`
   - New backend for compute shaders
   - Implement `compute_kernel` generation

3. **Python Backend**
   - File: `src/backend/python.rs`
   - NumPy integration
   - Generate `.py` with @dataclass state

4. **Rust Backend Enhancement**
   - File: `src/backend/rust.rs`
   - Wire to target spec
   - Support no_std and std targets

5. **SystemVerilog Backend Enhancement**
   - File: `src/backend/verilog.rs`
   - Wire to target spec
   - Support FPGA/ASIC targets

#### Deliverables
- All backends accept TargetSpec
- `brief compile` produces correct output for each target

---

### Phase 6: Inference Engine (Future)

**Goal:** Autonomous optimization based on target context.

#### Tasks

1. **Happy Path Inference**
   - Analyze target context
   - Select optimal FFI bindings
   - Auto-select memory allocation strategy

2. **Attribute Overrides**
   - `#[cuda.shared]`, `#[react.persistent]`, `#[hw.pin]`
   - Override inference when needed

#### Deliverables
- Inference warnings/logs
- Attribute validation

---

## File Changes Summary

| File | Phase | Change |
|------|-------|--------|
| `src/main.rs` | 3 | Add `compile` command, capability check |
| `src/target_spec/mod.rs` | 3 | Add `capabilities` field |
| `src/typechecker.rs` | 3 | Validate capabilities |
| `lib/targets/*.toml` | 4 | Create 9 target specs |
| `src/backend/typescript.rs` | 5 | Wire to TargetSpec |
| `src/backend/wgsl.rs` | 5 | New WebGPU backend |
| `src/backend/python.rs` | 5 | New Python backend |
| `src/backend/rust.rs` | 5 | Wire to TargetSpec |
| `src/backend/verilog.rs` | 5 | Wire to TargetSpec |

---

## Testing Plan

### Phase 3 Tests
```bash
# Capability mismatch
./brief compile app.ebv --target react_web.toml
# Expected: Error B4001

# Valid compile
./brief compile app.rbv --target react_web.toml
# Expected: Success, .tsx output
```

### Phase 4 Tests
```bash
# Test each target spec
for spec in lib/targets/*.toml; do
  ./brief compile test.bv --target $spec
done
```

### Phase 5 Tests
```bash
# Full matrix
for target in react webgpu python cobol; do
  for source in bv rbv ebv; do
    ./brief compile test.$source --target $target.toml
  done
done
```

---

## Success Criteria

1. ✅ **Phase 2:** C backend uses TargetSpec (DONE)
2. ⏳ **Phase 3:** `brief compile` command, capability validation
3. ⏳ **Phase 4:** 9 target specs created
4. ⏳ **Phase 5:** All backends wired to TargetSpec
5. ⏳ **Phase 6:** Inference engine operational

---

## Commit Strategy

```bash
# Phase 3
git commit -m "Phase 3: Add compile command and capability validation"

# Phase 4  
git commit -m "Phase 4: Add target spec library (9 targets)"

# Phase 5
git commit -m "Phase 5: Wire all backends to TargetSpec"

# Phase 6
git commit -m "Phase 6: Add inference engine"
```

---

## Notes

- Legacy commands (`brief c`, `brief rbv`, etc.) will continue to work as convenience aliases
- They will internally resolve to: `brief compile <file> --target <default_spec>.toml`
- Error codes: Use prefix `B4xxx` for Brief Cosmopolitan errors
  - `B4001`: Capability mismatch
  - `B4002`: Target not found
  - `B4003`: Backend error