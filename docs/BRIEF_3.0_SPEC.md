# Brief 3.0 Specification: Software-Defined Silicon (EBV-SV 1.0)

**Version:** 3.0  
**Date:** 2026-04-14  
**Status:** Final Design Specification  

---

## 1. Introduction: The Silicon Pivot

Brief 3.0 introduces **Spatial Isomorphism**, a paradigm where the source code is not a set of instructions for a processor, but a formal description of the hardware itself. When targeting SystemVerilog (`.sv`), the Brief compiler "prints" the logic directly into silicon gates and wires.

### 1.1 Core Philosophy: Software-Defined Silicon
In Brief 3.0, the "World Model" is the **RTL Netlist**. Every construct in the language has a deterministic physical representation:
- **State (`let`)**: Registers (Flip-Flops) or Wires.
- **Transactions (`txn`)**: Combinational Logic.
- **Reactive Transactions (`rct txn`)**: Synchronous Sequential Logic.
- **Vectors (`Type[N]`)**: Parallel Hardware (SIMD).
- **Contracts (`[pre][post]`)**: Formal Verification Targets (SVA).

---

## 2. Hardware Topology & Types

### 2.1 Bit-Level Precision
Brief 3.0 enforces strict bit-width mapping to SystemVerilog **Packed Arrays** for scalars and **Unpacked Arrays** for vectors.

| Brief Type | SV Representation | Physical Implementation |
|:---|:---|:---|
| `Bool` | `logic` | 1-bit wire/register |
| `UInt @/0..7` | `logic [7:0]` | 8-bit unsigned bus |
| `Int @/0..7` | `logic signed [7:0]` | 8-bit signed bus |
| `Type[N]` | `logic [W:0] name [0:N-1]` | Unpacked Array (BRAM/Registers) |

### 2.2 Memory-Mapped I/O & Pins
Declarations with `@ address` or `trg` define the **Module Boundary**.
- **`trg`**: Synthesized as `input logic`.
- **`let @ address`**: Synthesized as `output logic`.
- **`hardware.toml`**: Mandatory external configuration file mapping `@ address` to physical FPGA/ASIC pins or AXI register offsets. Addresses can be specified in `0x4000` or `0x00004000` formats.

---

## 3. The Reactive Pipeline (Synchronous Logic)

### 3.1 The Single-Master Heartbeat
All `rct txn` blocks are synthesized into a global `always_ff @(posedge clk)` block.
- **Reset**: A synchronous active-low `rst_n` signal is mandatory.
- **Initial State**: Variable initializers (`let x = 0`) are synthesized into the `if (!rst_n)` reset block.
- **Clock Enables (Strobes)**: Multiple reactor speeds are handled via internal clock dividers and `ce_<speed>hz` strobe signals.

### 3.2 State Mutation & Non-Blocking Assignments
In the SystemVerilog target, the `&` operator (mutable access) maps directly to **Non-Blocking Assignments (`<=`)**.
- This ensures that all reactive state changes across the entire chip occur simultaneously on the clock edge, eliminating race conditions by design.

### 3.3 Automatic Pipelining
A cascade of reactive transactions (`rct txn`) creates a synchronous pipeline. The compiler automatically inserts registers at transaction boundaries, allowing the logic to be distributed across clock cycles to maximize $F_{max}$.

---

## 4. Geometric SIMD (Parallel Unrolling)

### 4.1 Spatial Unrolling
Vector operations (e.g., `pixels = pixels + 1`) are synthesized using SystemVerilog `generate` blocks.
- **Rule**: $N$ elements = $N$ physical ALUs.
- **Lifting**: Scalar-to-vector operations are automatically "lifted" to all lanes.
- **Slicing**: Part-select syntax `vec[0:7]` maps to zero-cost physical wiring rerouting.

### 4.2 Shadow Buffering (AXI Integration)
When a `Vector[N]` is mapped to an AXI-Lite address, the compiler synthesizes a **Shadow Buffer (BRAM)**. Sequential AXI bursts fill the buffer, and the SIMD logic fires in **1 cycle** once the transaction precondition is met.

---

## 5. Temporal Logic & Watchdogs

### 5.1 Cycle-Accurate Determination
The `within N cycles` syntax is synthesized as a physical **Down-Counter**.
- **Execution**: The counter starts when the assignment is triggered.
- **Timeout**: If the result is not ready before the counter reaches zero, the `Error` variant of the result Union is asserted (`_tag <= 1`), and the `_err` signal is driven HIGH.

### 5.2 Mandatory Error Handling
Assignments with `within` timeouts **must** target a Union type containing `Error`. The compiler enforces exhaustive pattern matching on the result.

---

## 6. Formal Verification (SVA Mapping)

Brief 3.0 maps contracts directly to **SystemVerilog Assertions (SVA)**.
- **Preconditions**: Synthesized as `assume property`.
- **Postconditions**: Synthesized as `assert property`.
- **Prior State (`@var`)**: Mapped to the `$past(var)` function.

---

## 7. The Metropolitan Hub 2.0 (I/O & Bus)

### 7.1 AXI4-Lite Slave Synthesis
The compiler can wrap the core logic in an AXI4-Lite slave module using addresses as decoder offsets. Burst support is implemented for `Vector` types.

### 7.2 Port Mappings
- **Inputs**: `trg name: Type @ address` $\rightarrow$ `input logic [W:0] name`.
- **Outputs**: `let name @ address: Type` $\rightarrow$ `output logic [W:0] name`.

---

## 8. Compiler Constraints & Safety

1. **No Combinational Loops**: Proof Engine errors if wire feedback exists without a register.
2. **No Multi-Driver Violations**: Two transactions cannot drive the same `&` register in the same cycle.
3. **Floating Point Prohibited**: `Float` types result in a compile error for Verilog targets. Use fixed-point representation.
4. **Void Type**: Empty parentheses `()` are parsed as `Type::Void`.

---

*End of Brief 3.0 Specification*
