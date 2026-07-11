# Hardware Specification Guide (.toml)

This document provides a comprehensive specification for the `hardware.toml` file used by the Brief compiler to target hardware architectures (FPGAs/ASICs).

## Overview

The `hardware.toml` file maps logical variables and transactions defined in Brief (`.ebv`) to physical hardware resources, interfaces, and timing constraints.

## Root Sections

### 1. [project]
Defines metadata about the Brief project.
```toml
[project]
name = "my_gpu"
version = "1.0.0"
```

### 2. [target]
Specifies the physical device or architecture family and base clock frequency.
```toml
[target]
fpga = "generic"        # Matches target in hardware_lib/targets/
clock_hz = 100_000_000  # Global clock frequency (e.g., 100MHz)
```

### 3. [interface]
Defines the primary communication protocol for the module. Supported interfaces are located in `hardware_lib/interfaces/`.
```toml
[interface]
name = "axi4-lite"      # Interface type (axi4-lite, axi4-stream)
address_width = 16      # Parameter override for the interface
data_width = 32         # Parameter override for the interface
```

### 4. [memory]
Maps memory-annotated variables to physical memory blocks (BRAM, ROM) or registers (FF).
- **size**: Number of elements.
- **type**: Implementation style (`bram` for block ram, `flipflop` for distributed registers).
- **element_bits**: Data width of a single element.

```toml
[memory]
"0x40000000" = { size = 1, type = "flipflop", element_bits = 8 }
"0x40000010" = { size = 32, type = "bram", element_bits = 16 }
```

### 5. [io]
Maps specific addresses to top-level module pins and defines signal flow.
**CRITICAL**: Every signal that must be preserved after synthesis must be connected to an IO pin or explicitly marked as an `output`.
- **pin**: Physical pin identifier or net name.
- **direction**: `input` or `output`. Vectors default to `output`.

```toml
[io]
"0x40000000" = { pin = "A1", direction = "input" }
"0x40000004" = { pin = "A2", direction = "output" }
"0x40000030" = { pin = "A6", direction = "output" } # Map result vector to pins
```

---

## Hardware Library (`hardware_lib/`)

The compiler uses a reusable library of hardware components. Users can extend this library by adding new TOML files to the corresponding directories.

### Adding a New Target
Create a new file in `hardware_lib/targets/<name>.toml`:
```toml
[target]
name = "my_custom_fpga"
[constraints]
max_bram_kb = 2048
max_lut = 50000
```

### Adding a New Interface
Create a new file in `hardware_lib/interfaces/<name>.toml`:
```toml
[interface]
name = "parallel-bus"
[signals]
valid = { bits = 1, direction = "output" }
data = { bits = "data_width", direction = "output" }
[parameters]
data_width = { default = 32 }
```

## Usage

To compile a Brief file to SystemVerilog with hardware mapping, use the `--hw` flag:

```bash
./brief-compiler verilog my_gpu.ebv --hw hardware.toml --out out_dir
```

## Key Concept: Memory = Internal, IO = External

The distinction between `[memory]` and `[io]` defines the hardware boundary and prevents "pin explosion" errors:

- **[memory]**: Internal hardware resources (BRAM, Registers). These signals **stay inside the chip** and do NOT consume physical pins.
- **[io]**: Physical pins on the device package. These **cross the boundary** to the outside world.

**The Rule**: If an address is defined in the `[memory]` section, it is treated as internal logic, even if it also has an entry in `[io]`.

Example of a correct GPU configuration:
```toml
[memory]
"0x40000010" = { size = 32, type = "bram" }  # Internal BRAM (Stays inside)

[io]
"0x40000000" = { pin = "A1" }  # Control pin (Crosses boundary)
```
In this example, the large vector buffer stays internal, saving hundreds of physical pins.

