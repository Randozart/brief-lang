# Briv Compiler - Hardware Backend Development Guide

## Overview

The Briv compiler's hardware backend generates production-ready Verilog and Vivado build scripts from `.rbv` files and `hardware.toml` configuration.

## Architecture

```
src/backend/
├── mod.rs           # Module exports
├── c.rs             # C code generator
├── rust.rs          # Native Rust code generator
├── verilog.rs       # Verilog/SystemVerilog generator
└── tcl_generator.rs # Vivado Tcl build script generator
```

## Quick Start

### Generate Verilog

```rust
use briv_compiler::backend::{verilog::VerilogGenerator, tcl_generator::TclGenerator};
use briv_compiler::ast::HardwareConfig;

// Parse hardware.toml
let hw_config: HardwareConfig = toml::from_str(&std::fs::read_to_string("hardware.toml")?)?;

// Generate Verilog
let gen = VerilogGenerator::new(&hw_config.project.name, hw_config.clone());
let sv = gen.generate_auto(&program);  // Auto-detects interface type
std::fs::write(format!("{}.sv", hw_config.project.name), sv)?;

// Generate build script
let tcl_gen = TclGenerator::new(&hw_config, vec![format!("{}.sv", hw_config.project.name)]);
let tcl = tcl_gen.generate();
std::fs::write("build.tcl", tcl)?;
```

### Compile with CLI

```bash
cargo build --release
./target/release/briv-compiler rbv examples/shopping_cart.rbv
```

---

## Hardware Configuration Schema

### Complete `hardware.toml`

```toml
[project]
name = "my_design"
version = "1.0.0"

[target]
fpga = "xczu4ev"           # Silicon part number
platform = "kv260"          # Board/carrier card (optional)
clock_hz = 100_000_000

[synthesis]                 # Optional
mode = "global"             # "global" | "ooc" (out-of-context)
max_jobs = 0                # 0 = auto-detect from system RAM

[interface]
name = "axi4-lite"          # "axi4-lite" | "axi4-full" | "parallel"
controller = "LPD_MASTER"   # "LPD_MASTER" | "FPD_MASTER" (optional)
situs = "0x80000000"        # AXI address offset (optional)

[memory]
"0x4000" = { size = 1024, type = "bram", element_bits = 32 }

[io]
"0x4000" = { pin = "P11", direction = "output" }
```

### Field Reference

#### `[target]` Section

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `fpga` | string | Yes | Silicon part number |
| `platform` | string | No | Board/carrier card identifier |
| `clock_hz` | u32 | Yes | Target clock frequency in Hz |

#### `[synthesis]` Section (Optional)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | string | `"global"` | `"global"` for full design, `"ooc"` for out-of-context |
| `max_jobs` | u32 | `0` | Max parallel jobs. `0` = auto-detect |

#### `[interface]` Section

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | `"parallel"` | Interface type |
| `controller` | string | `"LPD_MASTER"` | AXI master controller |
| `situs` | string | `"0x80000000"` | AXI address offset |

#### `[memory]` Section

| Field | Type | Description |
|-------|------|-------------|
| `size` | usize | Number of elements |
| `type` | string | Memory type: `"bram"`, `"ultraram"`, `"flipflop"`, `"distributed"` |
| `element_bits` | usize | Bits per element |

---

## VerilogGenerator API

### Methods

#### `new(name: &str, hw_config: HardwareConfig) -> Self`

Create a new generator.

```rust
let gen = VerilogGenerator::new("my_design", hw_config);
```

#### `generate(&mut self, program: &Program) -> String`

Generate Verilog with raw parallel I/O (no AXI).

```rust
let sv = gen.generate(&program);
```

#### `generate_with_axi(&mut self, program: &Program) -> String`

Generate Verilog with AXI4-Lite interface.

```rust
let sv = gen.generate_with_axi(&program);
```

#### `generate_auto(&mut self, program: &Program) -> String`

Auto-detect interface type from `hardware.toml`.

```rust
let sv = gen.generate_auto(&program);
```

### Generated Verilog Features

#### For `parallel` Interface
- Raw `clk` and `rst_n` ports
- Individual I/O ports from `[io]` section
- Clock dividers for reactor speeds

#### For `axi4-lite` Interface
- 21-pin AXI4-Lite bus (AW, W, B, AR, R channels)
- AXI state machine (IDLE, WRITE, WWAIT, RWAIT)
- CPU interface signals: `cpu_write_addr`, `cpu_write_data`, `cpu_write_en`, `cpu_read_data`, `cpu_read_en`

#### Memory Attributes
```verilog
(* ram_style = "block" *)    // BRAM
(* ram_style = "ultra" *)    // UltraRAM
(* ram_style = "distributed" *) // Distributed LUTRAM
```

---

## TclGenerator API

### Methods

#### `new(config: &HardwareConfig, sv_files: Vec<String>) -> Self`

Create generator with config and output SV files.

```rust
let tcl_gen = TclGenerator::new(&hw_config, vec!["my_design.sv".to_string()]);
```

#### `generate(&self) -> String`

Generate Vivado Tcl build script.

```rust
let tcl = tcl_gen.generate();
```

### Generated Tcl Features

#### Board Preset Support
Automatically applies board-specific configurations when `platform` is specified.

#### Supported Platforms

| Platform | Board Part |
|----------|------------|
| `kv260`, `kria` | `xilinx.com:kv260_som:1.1` |
| `zcu102` | `xilinx.com:zcu102:1.1` |
| `zcu104` | `xilinx.com:zcu104:1.0` |
| `zcu106` | `xilinx.com:zcu106:1.1` |
| `zedboard` | `em.avnet.com:zedboard:1.0` |
| `pynqz2` | `xilinx.com:pynq-z2:1.0` |

#### Supported FPGA Parts

| FPGA | Part Number |
|------|-------------|
| `xczu4ev` | `xczu4ev-sfvc784-2-e` |
| `xczu6eg` | `xczu6eg-sfvc784-2-e` |
| `xczu9eg` | `xczu9eg-sfvc784-2-e` |
| `xc7a35t` | `xc7a35tfgg484-2` |
| `xc7a100t` | `xc7a100tfgg484-2` |
| `xc7k325t` | `xc7k325t-fbg900-2` |
| `xc7z010` | `xc7z010clg400-2` |
| `xc7z020` | `xc7z020clg400-2` |

#### Decree of Exclusion
Disables unused AXI ports to prevent clock leakage issues:
```tcl
set_property config { { EXCLUDE { FPD_S_AXI_INTF } } } [get_bd_cells zynq_ps]
set_property config { { EXCLUDE { LPD_S_AXI_INTF } } } [get_bd_cells zynq_ps]
```

#### RAM-Aware Job Control
Automatically detects system RAM and sets job count:
- `< 16 GB`: 1 job
- `16-32 GB`: 4 jobs
- `> 32 GB`: 8 jobs

---

## The Four Laws of Hardware Compilation

### Law 1: The Law of Two Doors (RAM Multiplexer)

BRAM/UltraRAM have exactly 2 ports. The compiler must consolidate all writes into a single port.

**Location**: `src/backend/verilog.rs:609-666`

**Generated Pattern**:
```verilog
// Address/Data multiplexer
always_comb begin
    s_we = 1'b0;
    s_waddr = '0;
    s_wdata = '0;
    if (txn_cond_0) begin
        s_we = 1'b1;
        s_waddr = addr_0;
        s_wdata = data_0;
    end
    // Priority encoding (last wins)
end

// Single BRAM write
always_ff @(posedge clk) begin
    if (s_we) begin
        ram[s_waddr] <= s_wdata;
    end
end
```

### Law 2: The Separation of Crown and State (AXI Wrapper)

The AXI protocol wrapper is generated separately from the algorithmic core logic.

**Location**: `src/backend/verilog.rs:78-206` (`generate_with_axi`)

### Law 3: The Lexical Authority of Attributes

RAM attributes are prepended using IEEE attribute syntax.

**Location**: `src/backend/verilog.rs:123-145, 330-364`

**Correct**:
```verilog
(* ram_style = "block" *) logic [31:0] ram [0:1023];
```

**Incorrect**:
```verilog
logic [31:0] ram [0:1023] /* synthesis syn_ramstyle = "block_ram" */;
```

### Law 4: The Tcl Generator (Physical Constraints)

Build scripts respect host machine constraints.

**Location**: `src/backend/tcl_generator.rs`

**Key Features**:
- Global synthesis for large designs: `set_property synth_checkpoint_mode None`
- Single threading on low-RAM machines: `launch_runs impl_1 -jobs 1`
- Board preset application

---

## Testing

```bash
# Run all tests
cargo test --lib

# Run specific test
cargo test --lib tcl_generation

# Build release
cargo build --release

# Compile example
./target/release/briv-compiler rbv examples/shopping_cart.rbv
```

---

## File Structure

```
briv-compiler/
├── src/
│   ├── backend/
│   │   ├── mod.rs           # Module exports
│   │   ├── verilog.rs       # VerilogGenerator
│   │   └── tcl_generator.rs # TclGenerator
│   ├── ast.rs               # HardwareConfig, MemoryMapping, etc.
│   └── main.rs              # CLI entry point
├── examples/
│   ├── hardware.toml       # Example configuration
│   ├── shopping_cart.rbv   # Example Briv program
│   └── ...
└── HARDWARE_BUG_FIX_PLAN.md # Bug fix history
```

---

## Changelog

### v0.2.0 (Current)
- Added `hardware.toml` schema extensions (`platform`, `synthesis`, `controller`, `situs`)
- Added `TclGenerator` with board preset support
- Added RAM multiplexer for BRAM/UltraRAM
- Added AXI4-Lite wrapper generation
- Added auto-detect job count based on system RAM
- Added "Decree of Exclusion" to disable unused AXI ports
- Fixed attribute placement (IEEE syntax)

### v0.1.0
- Basic Verilog generation
- Parallel I/O interface only