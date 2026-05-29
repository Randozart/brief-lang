# Brief Compiler Backends

**Version:** 0.11.0  
**Status:** 4 backends complete, 3 planned

---

## Available Backends

### ✅ Complete

1. **AArch64 Binary** (`aarch64.bv`)
   - Direct ARM64 machine code
   - Linear scan register allocation
   - Reactor loop generation
   - Output: `.bin` file

2. **x86-64 Binary** (`x86_64.bv`)
   - Direct x86-64 machine code
   - AMD64 calling convention
   - Reactor loop generation
   - Output: `.bin` file

3. **Rust** (`rust.bv`)
   - Rust source code generation
   - For bootstrapping
   - Uses unsafe blocks for state
   - Output: `.rs` file

4. **C** (`c.bv`)
   - C99 source code generation
   - For bootstrapping and embedded
   - Uses stdint.h types
   - Output: `.c` file

### ⏳ Planned

5. **WASM** (`wasm.bv`)
   - WebAssembly binary
   - WASM-bindgen glue code
   - Browser runtime

6. **VHDL** (`vhdl.bv`)
   - FPGA synthesis
   - Reactive to clocked logic
   - PSL assertions

7. **SystemVerilog** (`verilog.bv`)
   - FPGA/ASIC synthesis
   - TCL build scripts
   - SVA assertions

---

## Usage

```brief
import "compiler/backends/aarch64";
import "compiler/backends/x86_64";
import "compiler/backends/rust";
import "compiler/backends/c";

defn compile_to_aarch64(program: Program) -> List<u8> {
    term generate_aarch64(program);
}

defn compile_to_x86_64(program: Program) -> List<u8> {
    term generate_x86_64(program);
}

defn compile_to_rust(program: Program) -> String {
    term generate_rust(program);
}

defn compile_to_c(program: Program) -> String {
    term generate_c(program);
}
```

---

## Backend Interface

All backends implement:

```brief
defn generate_<target>(program: Program) -> OutputType
```

Where:
- `OutputType` = `List<u8>` for binary backends
- `OutputType` = `String` for source code backends

---

## Performance Comparison

| Backend | Compile Time | Runtime | Size | Use Case |
|---------|--------------|---------|------|----------|
| **AArch64** | O(n) | Fastest | Smallest | ARM embedded, KV260 |
| **x86-64** | O(n) | Fast | Small | Desktop, servers |
| **Rust** | O(n) + rustc | Fast | Medium | Bootstrapping |
| **C** | O(n) + gcc | Fast | Medium | Embedded, bootstrap |
| **WASM** | O(n) | Medium | Medium | Browser |
| **VHDL** | O(n) + synth | N/A | Large | FPGA |
| **Verilog** | O(n) + synth | N/A | Large | FPGA/ASIC |

---

## Code Generation Optimizations

### AArch64
- Linear scan register allocation: O(n)
- Direct instruction encoding: O(1) per instr
- Single-pass codegen: O(n)

### x86-64
- Similar to AArch64
- AMD64 calling convention
- RIP-relative addressing

### Rust/C
- Direct AST to source
- Minimal runtime
- Unsafe blocks for performance

---

*Last updated: 2026-05-06*
