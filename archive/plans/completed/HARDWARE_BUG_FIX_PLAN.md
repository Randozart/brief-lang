# Briev Compiler - Hardware Bug Fix Plan

## Status: COMPLETED

**Completed:**
- [x] Law 3: Proper Attribute Syntax (FIXED)
- [x] Law 1: RAM Multiplexer (FIXED)
- [x] Law 4: Tcl Generator (FIXED - new module)
- [x] Law 2: AXI Wrapper (FIXED)

## Issue 1: The "AXI Lie" - Missing AXI Bus Interface Generation

**Location**: `src/backend/verilog.rs:71-181` (`emit_header`)

**Problem**: The compiler generates raw individual pins but ignores the `interface.name` in `hardware.toml`. When users specify `name = "axi4-lite"` in `[interface]`, they get individual signals instead of an AXI bus.

**Current behavior**:
- Iterates `StateDecl` items and generates `input logic` / `output logic` ports
- Completely ignores `[interface]` section of the config

**Fix**: Add interface mapping at the start of `emit_header`:
- If `self.hw_config.interface.name == "axi4-lite"` → emit AXI4-Lite port bundle
- If `self.hw_config.interface.name == "axi4-full"` → emit full AXI4 with address/data channels
- Otherwise, keep current behavior (raw parallel I/O)

### Implementation

Add new method `emit_interface_ports()` and call it after `clk` and `rst_n` but before iterating state decls:

```rust
fn emit_interface_ports(&mut self) {
    let iface_name = &self.hw_config.interface.name;
    
    match iface_name.as_str() {
        "axi4-lite" => {
            // AXI4-Lite write address channel
            self.output.push_str(",\n    input  logic [15:0] s_awaddr");
            self.output.push_str(",\n    input  logic       s_awvalid");
            self.output.push_str(",\n    output logic       s_awready");
            
            // AXI4-Lite write data channel
            self.output.push_str(",\n    input  logic [31:0] s_wdata");
            self.output.push_str(",\n    input  logic [3:0]  s_wstrb");
            self.output.push_str(",\n    input  logic       s_wvalid");
            self.output.push_str(",\n    output logic       s_wready");
            
            // AXI4-Lite write response channel
            self.output.push_str(",\n    output logic [1:0] s_bresp");
            self.output.push_str(",\n    output logic       s_bvalid");
            self.output.push_str(",\n    input  logic       s_bready");
            
            // AXI4-Lite read address channel
            self.output.push_str(",\n    input  logic [15:0] s_araddr");
            self.output.push_str(",\n    input  logic       s_arvalid");
            self.output.push_str(",\n    output logic       s_arready");
            
            // AXI4-Lite read data channel
            self.output.push_str(",\n    output logic [31:0] s_rdata");
            self.output.push_str(",\n    output logic [1:0] s_rresp");
            self.output.push_str(",\n    output logic       s_rvalid");
            self.output.push_str(",\n    input  logic       s_rready");
        }
        "parallel" | _ => {
            // Current behavior - individual I/O pins (keep as default)
        }
    }
}
```

---

## Issue 2: The OOM Killer - Multi-Port RAM Bug

**Location**: `src/backend/verilog.rs:597-632` (RAM template path)

**Problem**: When multiple transactions write to different RAM addresses in the same cycle (e.g., `scratch[cpu_write_addr]` and `scratch[0]`), the compiler outputs multiple separate assignments inside a single `always_ff`. Vivado sees this as multiple write ports, cannot map to BRAM, and falls back to flip-flops (which blows up to millions of FFs).

**Current partial fix**: There's already a RAM template path (lines 597-632) that attempts to handle vectors > 64 elements, but:
1. It still outputs direct indexed assignments per transaction
2. It doesn't generate an address multiplexer

**Fix**: Modify the RAM template to generate a single-time Multiplexer pattern:

### Implementation

Replace the direct assignment approach in `emit_ram_write_statement` with priority-encoding:

```rust
// In the RAM always_ff block, generate ONE set of signals:
// - s_addr: muxed address from all transactions  
// - s_data: muxed data from all transactions
// - s_we: write enable (OR of all transaction conditions)

logic [ADDR_WIDTH-1:0] s_addr;
logic [DATA_WIDTH-1:0] s_data;
logic s_we;

// Priority encoding - last transaction wins (or first - configurable)
// Each transaction condition gates its address/data into the mux

// Generate address mux logic
always_comb begin
    s_addr = '0;
    s_data = '0;
    s_we  = 1'b0;
    
    // Priority: later transactions have higher priority (last wins)
    // For each transaction:
    if (txn_cond_0) begin
        s_addr = addr_0;
        s_data = data_0;
        s_we  = 1'b1;
    end
    if (txn_cond_1) begin
        s_addr = addr_1;
        s_data = data_1;
        s_we  = 1'b1;
    end
    // ... etc
end

// Single BRAM write
always_ff @(posedge clk) begin
    if (s_we) begin
        scratch[s_addr] <= s_data;
    end
end
```

**Key Changes**:
1. Collect all indexed writes from all transactions
2. Generate address/data mux logic in `always_comb`
3. Single write to BRAM in `always_ff`

---

## Issue 3: The "Ghost Pin" Bug - Mutually Exclusive [io] and [memory]

**Location**: `src/backend/verilog.rs:82-84` and `src/backend/verilog.rs:262-264`

**Problem**: The code treats `[io]` ports and `[memory]` registers as mutually exclusive:
- Line 82-84 (`emit_header`): `if !self.has_memory_mapping(addr)` gates port emission
- Line 262-264 (`emit_signals`): Same logic skips internal signal when has IO mapping

**Consequence**: If an address is defined in BOTH `[memory]` AND `[io]` in `hardware.toml`, the compiler:
1. Skips creating the port (because it HAS memory mapping)
2. Creates an internal signal (but it's never connected externally)
3. The signal becomes a "Ghost" - exists in design but has no physical connection

**This is actually CORRECT behavior for most cases**:
- Memory-mapped registers should be accessed via the bus interface (AXI)
- Not exposed as raw I/O pins

**However**, some registers may need BOTH:
- External pin for direct debug/control
- Memory mapping for CPU access (e.g., control register)

**Fix**: Add explicit flag for "dual-mapped" registers:

### Implementation

Add optional flag to allow dual mapping:

```toml
# In hardware.toml
[memory]
"0x4000" = { size = 1, type = "flipflop", element_bits = 1 }
"0x4004" = { size = 1024, type = "bram", element_bits = 32, expose_pins = true }  # NEW

[io]
"0x4004" = { pin = "P15", direction = "output" }
```

In `emit_header`:
```rust
// Check if explicitly marked as dual-mapped
let mem_cfg = /* ... lookup memory config ... */;
let force_port = mem_cfg.as_ref().map(|m| m.expose_pins).unwrap_or(false);

if (io_cfg.is_some() && !self.has_memory_mapping(addr)) || force_port {
    // Emit port
}
```

In `emit_signals`:
```rust
let force_internal = mem_cfg.as_ref().map(|m| m.expose_pins).unwrap_or(false);

if io_cfg.is_some() && !self.has_memory_mapping(addr) || force_internal {
    continue;  // Skip internal signal, port handles it
}
```

---

## Priority Order

1. **AXI Interface** (Issue 1) - Critical for any FPGA design using AXI
2. **RAM Multiplexer** (Issue 2) - Critical for large memory arrays (>64 elements)
3. **Ghost Pin** (Issue 3) - Lower priority, edge case

---

## Files to Modify

1. `src/backend/verilog.rs` - Main changes
2. `src/ast.rs` - Add `expose_pins` field to `MemoryMapping`
3. `examples/hardware.toml` - Update example to test AXI
4. Add `hardware_lib/interfaces/axi4-lite.toml` if not present

---

## Testing

- Build: `cargo build`
- Test: `cargo test --lib`
- Compile example: `./target/release/briev-compiler rbv examples/shopping_cart.rbv`
- Verify Verilog output contains AXI ports when interface = "axi4-lite"

---

# PART 2: THE FOUR LAWS OF THE STONE

These laws codify the physical constraints of silicon that software compilers must respect.

---

## Law 1: The Law of Two Doors (True RAM Inference)

**The Problem:** Your current compiler walks the syntax tree and emits a write assignment (`scratch[index] <= value;`) every time it sees one in the Briev code. In software, RAM has infinite doors. In hardware, BRAM and UltraRAM have exactly two. If your compiler outputs three different `always_ff` blocks that write to the same array, Vivado will shatter it into 4 million flip-flops.

**The Compiler Fix:**

You need an "AST Consolidation Pass" for memory.

1. The compiler must scan the code and find *every* read and write to a specific array.
2. Instead of emitting direct assignments, it must generate a **Multiplexer** (like the `always_comb` block we wrote).
3. It must generate exactly **one** `s_we` (write enable), **one** `s_waddr` (write address), and **one** `s_wdata`.
4. The RAM block itself is instantiated at the very end in a single `always_ff` block that only listens to those multiplexed signals.

### Implementation

Add new method `generate_ram_mux()` that replaces the current approach:

```rust
fn generate_ram_mux(&mut self, var_name: &str, txns: Vec<&Transaction>, program: &Program) {
    // Step 1: Collect all indexed writes from ALL transactions
    let all_writes: Vec<RamWrite> = Vec::new();
    for txn in &txns {
        let writes = self.extract_indexed_writes(var_name, &txn.body);
        all_writes.extend(writes);
    }
    
    // Step 2: Generate multiplexer logic
    self.output.push_str("    // RAM address/data mux\n");
    self.output.push_str("    always_comb begin\n");
    self.output.push_str("        s_we = 1'b0;\n");
    self.output.push_str("        s_waddr = '0;\n");
    self.output.push_str("        s_wdata = '0;\n");
    
    // Priority encoding (last txn wins)
    for write in all_writes.iter().rev() {
        self.output.push_str(&format!(
            "        if ({}) begin\n",
            write.condition
        ));
        self.output.push_str(&format!(
            "            s_we = 1'b1;\n",
        ));
        self.output.push_str(&format!(
            "            s_waddr = {};\n",
            write.address_expr
        ));
        self.output.push_str(&format!(
            "            s_wdata = {};\n",
            write.data_expr
        ));
        self.output.push_str("        end\n");
    }
    self.output.push_str("    end\n\n");
    
    // Step 3: Single RAM write
    self.output.push_str("    always_ff @(posedge clk) begin\n");
    self.output.push_str("        if (s_we) begin\n");
    self.output.push_str(&format!(
        "            {}[s_waddr] <= s_wdata;\n",
        var_name
    ));
    self.output.push_str("        end\n");
    self.output.push_str("    end\n\n");
}
```

---

## Law 2: The Separation of Crown and State (The AXI Wrapper)

**The Problem:** Your compiler currently tries to put the AXI registers (the `cpu_` pins) inside the same module as the algorithmic math (`calc_phase`, etc). This forces Vivado to guess how to connect it.

**The Compiler Fix:**

Your compiler should output **two** SystemVerilog files (or one file with two modules):

1. **The Core (`neuralcore.sv`):** Pure, clean math. It only takes raw wires as inputs and outputs. It knows nothing about AXI, PCI-e, or any other protocol.

2. **The Envoy (`neuralcore_axi.sv`):** A generated wrapper. The compiler looks at `hardware.toml`. If it sees `[interface] name = "axi4-lite"`, it generates the standard 21-pin AXI state machine we wrote today, and instantiates the Core inside of it.

*This means later, if you want to switch to AXI4-Full (DMA), you just change the `.toml`, and the compiler swaps the Envoy without touching your Core logic.*

### Implementation

Split into two generators:

```rust
// 1. CoreGenerator - pure math, no AXI
pub struct CoreGenerator { /* ... */ }
impl CoreGenerator {
    pub fn generate(&mut self, program: &Program) -> String {
        // Only emit: clk, rst_n, raw input/output wires
        // No AXI, no protocol
    }
}

// 2. AXIEnvoyGenerator - wraps Core with protocol
pub struct AXIEnvoyGenerator {
    core_module: String,
    interface_type: String,
}
impl AXIEnvoyGenerator {
    pub fn generate(&mut self, program: &Program) -> String {
        // Instantiate CoreGenerator
        // Add AXI state machine
        // Connect AXI signals to Core raw wires
    }
}
```

---

## Law 3: The Lexical Authority of Attributes

**The Problem:** Your compiler was outputting `/* synthesis syn_ramstyle = "block_ram" */` at the end of the line. While technically supported in older tools, modern Vivado heavily prefers explicit macro attributes placed *before* the declaration. Furthermore, your compiler didn't know what `ultraram` was.

**The Compiler Fix:**

Update your Rust string formatting to prepend the strict IEEE attribute syntax.

```rust
let attr = match mem_cfg.mem_type.as_str() {
    "bram" => "(* ram_style = \"block\" *) ",
    "ultraram" => "(* ram_style = \"ultra\" *) ",
    "distributed" => "(* ram_style = \"distributed\" *) ",
    _ => ""
};
self.output.push_str(&format!(
    "    {}logic [{}:0] {} [0:{}];\n",
    attr, width, name, depth
));
```

---

## Law 4: The Tcl Generator (The Chancery Scribe)

Your compiler (or your orchestration tool) generated `build_imp.tcl`. You need to update the Tcl generator to respect the physical limits of 16GB developer machines:

*   **Always use Global Synthesis** for large designs: `set_property synth_checkpoint_mode None [get_files system.bd]`
*   **Enforce Single-Threading** on the backend to prevent OOM kills: `launch_runs impl_1 ... -jobs 1`
*   **Ensure strict Tcl spacing:** Your Rust `format!()` macros for Tcl need exact spaces before every opening bracket

### Implementation

New `TclGenerator` module:

```rust
pub struct TclGenerator { ... }
impl TclGenerator {
    pub fn generate_top_tcl(&mut self, config: &BuildConfig) -> String {
        self.output.push_str("create_project -in_tmp_dir -part $PART\n");
        
        // Global synthesis for large designs
        self.output.push_str(&format!(
            "set_property synth_checkpoint_mode None [get_files {}.bd]\n",
            config.top_module
        ));
        
        // Single threading to prevent OOM
        self.output.push_str("launch_runs impl_1 -jobs 1\n");
        
        // Strict Tcl spacing
        self.output.push_str("set_property BITSTYLE.GENERAL true [current_design]\n");
    }
}
```

---

## Summary of Changes

| Law | File | Change | Location |
|-----|------|--------|----------|
| Law 1 | `src/backend/verilog.rs` | RAM Multiplexer | Lines 609-666 |
| Law 2 | `src/backend/verilog.rs` | AXI Wrapper | `generate_with_axi()` |
| Law 3 | `src/backend/verilog.rs` | Attribute Syntax | Lines 123-145, 330-364 |
| Law 4 | `src/backend/tcl_generator.rs` | Tcl Generator | New module |

---

## v0.2 Hardware Configuration Schema

```toml
[project]
name = "my_design"
version = "1.0.0"

[target]
fpga = "xczu4ev"                    # Silicon part
platform = "kv260"                  # Board/carrier card (optional)
clock_hz = 100_000_000

[synthesis]                         # Optional
mode = "global"                     # "global" or "ooc"
max_jobs = 0                        # 0 = auto-detect from system RAM

[interface]
name = "axi4-lite"                  # "axi4-lite", "axi4-full", or "parallel"
controller = "LPD_MASTER"           # "LPD_MASTER" or "FPD_MASTER" (optional)
situs = "0x80000000"                # AXI address offset (optional)

[memory]
"0x4000" = { size = 1024, type = "bram", element_bits = 32 }

[io]
"0x4000" = { pin = "P11", direction = "output" }
```

### Supported Platforms
- `kv260`, `kria` → Xilinx Kria SOM
- `zcu102`, `zcu104`, `zcu106` → ZCU boards
- `zedboard` → Avnet ZedBoard
- `pynqz2` → Pynq-Z2

### Supported FPGAs
- Xilinx UltraScale+: `xczu4ev`, `xczu6eg`, `xczu9eg`
- Xilinx 7-series: `xc7a35t`, `xc7a100t`, `xc7k325t`, `xc7z010`, `xc7z020`

---

## Usage Guide

### Verilog Generation

```rust
use crate::backend::verilog::VerilogGenerator;
use crate::ast::HardwareConfig;

// Load config from hardware.toml
let hw_config = /* ... parse hardware.toml ... */;

// Create generator - uses project name from hardware.toml
let gen = VerilogGenerator::new(&hw_config.project.name, hw_config.clone());

// Generate based on interface type
let sv = match hw_config.interface.name.as_str() {
    "axi4-lite" | "axi4-full" => gen.generate_with_axi(&program),
    _ => gen.generate(&program),
};

// Write to file: {project_name}.sv
std::fs::write(format!("{}.sv", hw_config.project.name), sv)?;
```

### Tcl Generation

```rust
use crate::backend::tcl_generator::TclGenerator;

// Create with config and generated SV files
let tcl_gen = TclGenerator::new(&hw_config, vec![format!("{}.sv", hw_config.project.name)]);

let tcl = tcl_gen.generate();

// Write to file: build.tcl
std::fs::write("build.tcl", tcl)?;
```

### Integration

```rust
// Full pipeline
fn compile_to_vivado(program: &Program, hw_config: &HardwareConfig) -> Result<(), Box<dyn Error>> {
    let project_name = &hw_config.project.name;
    
    // Generate Verilog
    let gen = VerilogGenerator::new(project_name, hw_config.clone());
    let sv = match hw_config.interface.name.as_str() {
        "axi4-lite" | "axi4-full" => gen.generate_with_axi(program),
        _ => gen.generate(program),
    };
    std::fs::write(format!("{}.sv", project_name), sv)?;
    
    // Generate Tcl build script
    let tcl_gen = TclGenerator::new(hw_config, vec![format!("{}.sv", project_name)]);
    let tcl = tcl_gen.generate();
    std::fs::write("build.tcl", tcl)?;
    
    Ok(())
}
```

---

## Generated Outputs

### For `parallel` interface:
- `{project}.sv` - Module with raw clk, rst_n, and I/O ports

### For `axi4-lite` interface:
- `{project}.sv` - Module with 21-pin AXI4-Lite bus
- AXI state machine (AXIL_IDLE, WRITE, WWAIT, RWAIT)
- CPU signals: `cpu_write_addr`, `cpu_write_data`, `cpu_write_en`, `cpu_read_data`, `cpu_read_en`

### For both:
- `build.tcl` - Vivado build script (packages IP, creates BD, generates bitstream)