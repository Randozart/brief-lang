# Hardware Configuration Guide

This document explains how `hardware.toml` affects the generated SystemVerilog output.

## Overview

The `hardware.toml` file provides target-specific configuration that changes how Brief code compiles to hardware. This allows the same `.ebv` source to target different FPGAs or memory architectures.

## Memory Type Abstraction

### The Problem

Brief's `let buf: Int[1024]` declares a vector of 1024 integers. When compiling to SystemVerilog, this could become:

1. **Flip-flop array**: `logic [15:0] buf [0:1023]` with per-element always_ff logic
2. **Block RAM (BRAM)**: Single `always_ff` with address muxing
3. **UltraRAM (URAM)**: Single `always_ff` with URAM-specific attributes

Without hardware configuration, the compiler can't know which to generate.

### How hardware.toml Resolves This

```toml
[memory]
"0x40010000" = { size = 1024, type = "bram", element_bits = 16 }
"0x40020000" = { size = 65536, type = "ultraram", element_bits = 16 }
```

The compiler matches vector address ranges to `hardware.toml` entries to determine memory type.

## Generated Code Differences

### flipflop (default, small vectors)
```systemverilog
genvar buf_i;
generate
    for (buf_i = 0; buf_i < 1024; buf_i = buf_i + 1) begin : buf_logic
        always_ff @(posedge clk) begin
            if (!rst_n)
                buf[buf_i] <= 0;
            else if (condition)
                buf[buf_i] <= data;
        end
    end
endgenerate
```
- Uses generate-for loop creating individual flip-flops
- Each element has its own always_ff
- Reset logic per-element

### bram / ultraram (large vectors with memory type hint)
```systemverilog
// RAM template for buf (type: Some("bram"), size: 1024)
always_ff @(posedge clk) begin
    if (condition) begin
        if (write_addr == idx) begin
            buf[idx] <= write_data;
        end
    end
end
```
- Single always_ff with address-based muxing
- **No reset initialization** - BRAM/UltraRAM have power-on initialization
- Synthesizes to actual Block RAM primitives

## Why No Reset for RAM?

Block RAM and UltraRAM on FPGA:
- **Auto-initialize** to zero on power-up
- Don't have reset pins like flip-flops
- You write data to them, you don't reset them

The generated code reflects this reality. If you need non-zero initial values, use:
- `$readmemh()` in an `initial` block (simulation only)
- FPGA bitstream initialization (synthesis)

## Address Width Considerations

The `address_width` in `hardware.toml` determines the address signal width:

```toml
[interface]
address_width = 18  # 18-bit addressing
```

This generates address signals like:
```systemverilog
logic [17:0] cpu_write_addr;
```

The actual array size may be smaller (e.g., 1024 = 10 bits). Verilator will warn about width truncation:
```
%Warning-WIDTHTRUNC: Bit extraction of array[1023:0] requires 10 bit index, not 18 bits.
```

This is **expected and correct** - the condition `write_addr == idx` properly masks the address.

## Verification with Verilator

```bash
verilator -Wall --lint-only your_file.sv
```

### Expected Warnings (Not Errors)

| Warning | Cause | Action |
|---------|-------|--------|
| WIDTHEXPAND | Signal width mismatch in assignment | Review or widen signals |
| UNDRIVEN | Input signals not driven in testbench | Connect to testbench |
| UNUSEDSIGNAL | Signal not used in logic | Verify or remove |
| WIDTHTRUNC | 18-bit address for smaller array | **No action needed** - correct behavior |

### What Failures Look Like

```systemverilog
// BROKEN: Array initialization syntax
logic [15:0] arr [0:1023];
always_ff @(posedge clk) begin
    if (!rst_n)
        arr <= 0;  // ERROR: Can't assign scalar to unpacked array
end
```

The compiler now avoids this by skipping reset initialization for RAM types.

## Summary

| hardware.toml setting | Effect on generated SV |
|----------------------|----------------------|
| `type = "bram"` | Single always_ff, no reset init |
| `type = "ultraram"` | Single always_ff, URAM attributes |
| `type = "flipflop"` | Generate-for loop with per-element reset |
| `address_width = 18` | 18-bit address signals generated |

The abstraction allows:
- Portable Brief code across different FPGA targets
- Memory architecture decisions external to business logic
- Correct synthesis to actual FPGA resources (BRAM vs FFs)