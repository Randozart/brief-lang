# Brief Compiler Target Abstraction Layer (TAL)

**Date:** 2026-04-23
**Status:** Proposed Architecture

---

## Problem Statement

The Brief compiler currently embeds vendor-specific "magic" words directly in its Rust source code:

1. **Hardcoded Synthesis Pragmas** in `verilog.rs`:
   ```rust
   let ram_pragma = "/* synthesis syn_ramstyle = \"block_ram\" */";
   ```

2. **Hardcoded AXI Port Mapping** in the backend:
   - `cpu_write_addr` → `s_axi_awaddr`
   - `cpu_write_data` → `s_axi_wdata`

3. **Tool-Specific Build Commands** scattered across the codebase

This creates three problems:

| Issue | Impact |
|-------|--------|
| **Compiler Bloat** | Adding support for new FPGAs requires recompiling |
| **Vendor Lock-in** | Hardcoded Xilinx/Intel strings pollute the backend |
| **Magic Leakage** | Users see synthesis pragmas in generated SV |

---

## Solution: Data-Driven Target Abstraction

Move all vendor-specific configuration to TOML files. The compiler becomes a template engine that reads target profiles at runtime.

---

## Architecture

```
brief-compiler/
├── hardware_lib/
│   ├── targets/                    # Vendor definitions
│   │   ├── xilinx_ultrascale_plus.toml
│   │   ├── intel_stratix.toml
│   │   └── lattice_ecp5.toml
│   ├── interfaces/               # Bus protocol definitions
│   │   ├── axi4_lite.toml
│   │   ├── wishbone.toml
│   │   └── axistream.toml
│   └── src/
│       ├── lib.rs                # Public API
│       ├── target.rs             # Target loading/parsing
│       └── interface.rs          # Interface loading/parsing
```

---

## TOML Schema Definitions

### Target Profile: `hardware_lib/targets/xilinx_ultrascale_plus.toml`

```toml
# Target: Xilinx Zynq UltraScale+ (KV260, etc.)
# Used for: IMP KV260 project

[vendor]
name = "xilinx"
family = "zynq_ultrascale_plus"
part_prefix = "xczu"

[memory.pragmas]
bram = "/* synthesis syn_ramstyle = \"block_ram\" */"
ultraram = "/* synthesis syn_ramstyle = \"ultra_ram\" */"
distributed = "/* synthesis syn_ramstyle = \"distributed\" */"
flipflop = ""

[logic.pragmas]
keep = "/* synthesis keep */"
buffer_type = "/* synthesis buffer_type = \"bufg\" */"
noshare = "/* synthesis nelshare = \"none\" */"

[toolchain]
synth_tool = "vivado"
synth_mode = "batch"
script_ext = ".tcl"
default_part = "xczu4ev-sfvc784-1"

[constraints]
max_frequency_hz = 100_000_000
timing_units = "ns"
```

### Target Profile: `hardware_lib/targets/intel_stratix.toml`

```toml
# Target: Intel Stratix (for future support)

[vendor]
name = "intel"
family = "stratix"
part_prefix = "10M"

[memory.pragmas]
bram = "/* synthesis ramstyle = \"M20K\" */"
mlab = "/* synthesis ramstyle = \"MLAB\" */"
flipflop = ""

[logic.pragmas]
keep = "/* synthesis preserve */"
noprune = "/* synthesis noprune */"

[toolchain]
synth_tool = "quartus"
script_ext = ".tcl"

[constraints]
max_frequency_hz = 100_000_000
```

### Interface Profile: `hardware_lib/interfaces/axi4_lite.toml`

```toml
# Interface: AXI4-Lite
# Used for: ARM-to-FPGA communication on Zynq UltraScale+

[interface]
name = "axi4_lite"
type = "memory_mapped_slave"
address_width = 18
data_width = 32

# Map clean Brief names to AXI physical port names
[port_map]
clock = "s_axi_aclk"
reset_n = "s_axi_aresetn"

write_addr = "s_axi_awaddr"
write_valid = "s_axi_awvalid"
write_ready = "s_axi_awready"

write_data = "s_axi_wdata"
write_strb = "s_axi_wstrb"

read_addr = "s_axi_araddr"
read_valid = "s_axi_arvalid"
read_ready = "s_axi_arready"

read_data = "s_axi_rdata"
read_response = "s_axi_rresp"

# Auto-generated wrapper template
[wrapper.template]
filename = "{module}_axi_wrapper.sv"
body = '''
module {module_name}_axi_wrapper (
    // AXI Clock and Reset
    input wire {clock},
    input wire {reset_n},
    
    // Write Address Channel
    input wire [{addr_width}-1:0] {write_addr},
    input wire {write_valid},
    output wire {write_ready},
    
    // Write Data Channel  
    input wire [{data_width}-1:0] {write_data},
    input wire [{data_width/8}-1:0] {write_strb},
    
    // Write Response Channel
    output wire [1:0] {write_response},
    output wire {write_response_valid},
    
    // Read Address Channel
    input wire [{addr_width}-1:0] {read_addr},
    input wire {read_valid},
    output wire {read_ready},
    
    // Read Data Channel
    output wire [{data_width}-1:0] {read_data},
    output wire [1:0] {read_response},
    output wire {read_valid},
    
    // User Logic Outputs (connected to clean Brief module)
    output wire [{addr_width}-1:0] user_addr,
    output wire [{data_width}-1:0] user_wdata,
    output wire user_valid,
    input wire user_ready
);

    // Instantiate the clean Brief module
    {module_name} core_inst (
        .clk({clock}),
        .rst_n({reset_n}),
        .cpu_write_addr(user_addr),
        .cpu_write_data(user_wdata),
        .cpu_write_en(user_valid),
        .cpu_read_addr(user_addr),
        .cpu_read_en(user_valid)
    );

    // AXI to User Logic Protocol Adapter
    // (Simplified - full implementation would include handshaking)
    assign user_valid = {write_valid};
    assign user_addr = {write_addr};
    assign user_wdata = {write_data};
    assign {write_response} = 2'b00;
    assign {write_response_valid} = {write_valid};
    assign {read_valid} = {read_valid};
    assign {read_data} = user_ready ? 32'hDEADBEEF : 32'h0;

endmodule
'''
```

### Interface Profile: `hardware_lib/interfaces/wishbone.toml`

```toml
# Interface: Wishbone (for Lattice/_open-source FPGAs)
# Alternative to AXI4-Lite for simpler platforms

[interface]
name = "wishbone"
type = "memory_mapped_slave"
address_width = 16
data_width = 32

[port_map]
clock = "wb_clk"
reset = "wb_rst"

address = "wb_addr"
data_in = "wb_dat_i"
data_out = "wb_dat_o"
we = "wb_we"
oe = "wb_oe"
stb = "wb_stb"
ack = "wb_ack"

[wrapper.template]
filename = "{module}_wb_wrapper.sv"
body = '''
module {module_name}_wb_wrapper (
    input wire {clock},
    input wire {reset},
    
    input wire [{addr_width}-1:0] {address},
    input wire [{data_width}-1:0] {data_out},
    input wire {we},
    input wire {stb},
    output wire [{data_width}-1:0] {data_in},
    output wire {ack}
);

    {module_name} core_inst (
        .clk({clock}),
        .rst({reset}),
        .addr({address}),
        .wdata({data_out}),
        .we({we}),
        .rdata({data_in}),
        .ack({ack})
    );

endmodule
'''
```

---

## Compiler Integration

### Module: `src/hardware/mod.rs`

```rust
use std::collections::HashMap;
use std::path::Path;

/// Hardware library containing vendor and interface definitions
pub struct HardwareLib {
    targets: HashMap<String, TargetProfile>,
    interfaces: HashMap<String, InterfaceProfile>,
}

pub struct TargetProfile {
    pub vendor: String,
    pub family: String,
    pub memory_pragmas: HashMap<String, String>,
    pub logic_pragmas: HashMap<String, String>,
    pub toolchain: ToolchainConfig,
}

pub struct InterfaceProfile {
    pub name: String,
    pub port_map: HashMap<String, String>,
    pub wrapper_template: String,
}

impl HardwareLib {
    /// Load all profiles from hardware_lib directory
    pub fn load<P: AsRef<Path>>(base_path: P) -> Result<Self, LoadError> {
        let mut targets = HashMap::new();
        let mut interfaces = HashMap::new();
        
        // Load all .toml files from targets/
        for entry in fs::read_dir(base_path.as_ref().join("targets")) {
            let target = load_target(entry.path())?;
            targets.insert(target.name.clone(), target);
        }
        
        // Load all .toml files from interfaces/
        for entry in fs::read_dir(base_path.as_ref().join("interfaces")) {
            let interface = load_interface(entry.path())?;
            interfaces.insert(interface.name.clone(), interface);
        }
        
        Ok(HardwareLib { targets, interfaces })
    }
    
    /// Get memory pragma for a given type and target
    pub fn get_memory_pragma(&self, target: &str, ram_type: &str) -> String {
        self.targets
            .get(target)
            .and_then(|t| t.memory_pragmas.get(ram_type))
            .cloned()
            .unwrap_or_default()
    }
}
```

### Modified Backend: `src/backend/verilog.rs`

```rust
// BEFORE (hardcoded - BAD):
let ram_pragma = "/* synthesis syn_ramstyle = \"block_ram\" */";

// AFTER (data-driven - GOOD):
fn get_ram_pragma(&self, ram_type: &str) -> String {
    let target = &self.config.target_vendor;
    self.hardware_lib
        .get_memory_pragma(target, ram_type)
        .unwrap_or("".to_string())
}

// Usage in array generation:
let pragma = self.get_ram_pragma("bram");
format!("logic signed [15:0] buffer [0:262143] {};", pragma)
```

### Interface Wrapper Generation

```rust
fn generate_wrapper(&self, module_name: &str) -> String {
    let interface = &self.config.interface;
    let profile = self.hardware_lib.get_interface(interface);
    
    // Apply port mapping
    let code = profile.wrapper_template.clone();
    code.replace("{module_name}", module_name);
    code.replace("{clock}", &profile.port_map["clock"]);
    // ... apply all port mappings
    
    code
}
```

---

## Usage Flow

### Current (compiler-centric):

```
user writes:  neuralcore.ebv
compiler generates: neuralcore.sv (with hardcoded Xilinx pragmas)
```

### Future (data-driven):

```
user specifies:  hardware.toml:
  [target]
  fpga = "xczu4ev"
  interface = "axi4-lite"

compiler loads:  hardware_lib/targets/xilinx_ultrascale_plus.toml
                hardware_lib/interfaces/axi4_lite.toml

compiler generates:
  - neuralcore.sv (pure, no pragmas visible to user)
  - neuralcore_axi_wrapper.sv (auto-generated with AXI ports)
  - build.tcl (auto-generated Vivado script)
```

---

## Advantages

| Advantage | Description |
|-----------|-------------|
| **Extensible** | Add new FPGA vendors without compiler changes |
| **Clean Compiler** | Backend stays generic, profiles do the work |
| **No Magic Leakage** | Users write pure Brief, magic stays in TOML |
| **Multi-Vendor Support** | Same compiler targets Xilinx, Intel, Lattice |
| **Testable** | Profiles are just data files, easy to validate |

---

## Implementation Phases

| Phase | Description |
|-------|-------------|
| 1 | Create `hardware_lib/` directory structure |
| 2 | Define `xilinx_ultrascale_plus.toml` |
| 3 | Define `axi4_lite.toml` wrapper template |
| 4 | Create `src/hardware/mod.rs` loader |
| 5 | Modify `src/backend/verilog.rs` to use loader |
| 6 | Test with IMP KV260 project |
| 7 | Add `intel_stratix.toml` and `wishbone.toml` |

---

## Files to Create

| File | Purpose |
|------|---------|
| `hardware_lib/targets/xilinx_ultrascale_plus.toml` | KV260 target |
| `hardware_lib/interfaces/axi4_lite.toml` | AXI4-Lite wrapper |
| `src/hardware/mod.rs` | TOML loader |
| `src/hardware/target.rs` | Target profile struct |
| `src/hardware/interface.rs` | Interface profile struct |

---

## References

- Original architecture inspiration from LLVM/ GCC backend分离
- Xilinx Synthesis Pragma documentation
- AXI4-Lite protocol specification (AMBA)
- Wishbone specification (OpenCores)