# VHDL Target Implementation Plan

**Date:** 2026-05-05
**Status:** Planning
**Related:** `docs/reference/VHDL_TARGET_RESEARCH.md`

---

## 1. Scope

Add VHDL as a second transpile target for Embedded Brief (.ebv), alongside existing SystemVerilog target.

---

## 2. Why VHDL

| Market | Use Case |
|--------|----------|
| **Europe** | Preferred over SystemVerilog |
| **Aerospace** | DO-254 compliance |
| **Formal** | Better PSL tool support |

---

## 3. Required Components

### 3.1 Type Mapping

| Brief Type | VHDL Type | Notes |
|------------|-----------|-------|
| `Bool` | `std_logic` | Single bit |
| `UInt[N]` | `std_logic_vector(N-1 downto 0)` | Unsigned |
| `Int[N]` | `signed(N-1 downto 0)` | Signed |
| `Float` | `real` | Floating point |
| `Vector[T, N]` | `array(0 to N-1) of T` | Array |
| `String` | `string` | VHDL string |
| `Addr` | `std_logic_vector(31 downto 0)` | Address |

### 3.2 Translation

| Brief Construct | VHDL Output |
|----------------|-------------|
| `state` | Signal declaration |
| `rct txn` | Clocked process |
| `txn` (non-reactive) | Combinatorial process |
| `[guard]` | If/elsif in process |
| `check` | PSL assertion |

### 3.3 PSL Assertions

| Brief Contract | PSL Property |
|----------------|--------------|
| `pre[condition]` | `assert never condition` |
| `post[condition]` | `assert always condition -> next` |

---

## 4. Implementation Phases

### Phase 1: Basic Translation

1. Add `--target vhdl` to CLI
2. Implement type mapping
3. Generate entity/architecture skeleton
4. Map state to signals

### Phase 2: Process Translation

1. Reactive txn → clocked process
2. Guards → if/elsif statements
3. Assignments → signal assignments
4. Handle @address mapping

### Phase 3: Contract Translation

1. Parse CHECK conditions
2. Generate PSL properties
3. Add default clock
4. Verify temporal logic

### Phase 4: Dual Output

1. Support `--target both`
2. Share validation between targets
3. Generate both .sv and .vhd

---

## 5. Files to Modify

| File | Changes |
|------|---------|
| `src/backend/mod.rs` | Add VHDL backend module |
| `src/backend/vhdl.rs` | New: VHDL code generation |
| `src/cli.rs` | Add `--target vhdl` option |
| `src/target.rs` | Add VHDL target enum |

---

## 6. Example Translation

### Input (.ebv)
```brief
led_on: Bool = false
ALIAS led: Bool @0xFF5E0000

rct txn init [true][led_on] {
    led_on = true
}

rct txn toggle [led_on][!led_on] {
    led_on = !led_on
}
```

### Output (.vhd)
```vhdl
library IEEE;
use IEEE.std_logic_1164.all;

entity top is
    port (
        clk : in std_logic;
        rst : in std_logic;
        led : out std_logic
    );
end entity top;

architecture rtl of top is
    signal led_on : std_logic := '0';
begin
    proc_init: process(clk, rst) is
    begin
        if rst = '1' then
            led_on <= '0';
        elsif rising_edge(clk) then
            led_on <= '1';
        end if;
    end process proc_init;

    proc_toggle: process(clk, rst) is
    begin
        if rst = '1' then
            null;
        elsif rising_edge(clk) then
            if led_on = '1' then
                led_on <= '0';
            end if;
        end if;
    end process proc_toggle;

    led <= led_on;
end architecture rtl;
```

---

## 7. Tooling

| Tool | Purpose |
|------|---------|
| **GHDL** | Simulation (testing) |
| **Vivado** | Xilinx synthesis |
| **ModelSim** | Waveform debugging |
| **JasperGold** | Formal verification |

---

## 8. Success Criteria

| Criteria | Verification |
|-----------|--------------|
| Valid .vhd output | Compiles in GHDL |
| Same contracts as SV | PSL matches SVA |
| Dual target works | Generates both .sv and .vhd |

---

## 9. Open Questions

1. **Naming convention** - `.vhd` or `.vhdl` for output files?
2. **PSL version** - PSL93, PSL05, or PSL06?
3. **Standard library** - Include VHDL packages for common types?