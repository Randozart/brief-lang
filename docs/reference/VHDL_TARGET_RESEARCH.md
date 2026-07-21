# Research: VHDL Target for Embedded Brief

## Concept: SystemVerilog + VHDL Dual Output from .ebv

This document explores adding VHDL as a second transpile target for Embedded Brief, alongside SystemVerilog. VHDL offers advantages in formal verification and certain hardware design workflows.

---

## 1. Why VHDL?

| Aspect | SystemVerilog | VHDL |
|--------|---------------|------|
| **Industry Use** | Mostly US/Asia | Europe, aerospace |
| **Formal Verification** | UVM-based | Property-based (PSL) |
| **Tool Support** | Commercial + open | Strong formal tools |
| **Safety Critical** | Good | Excellent (DO-254) |
| **Learning Curve** | Lower | Higher |

### VHDL Advantages for Brief

1. **PSL (Property Specification Language)** - Native temporal logic assertions
2. **Formal verification** - Tools like Cadence JasperGold support VHDL better
3. **DO-254 compliance** - Preferred for aviation/aerospace
4. **European tooling** - Different toolchain than SV projects

---

## 2. Target Architecture

### 2.1 Brief → VHDL Flow

```brief
// config.ebv - Brief embedded code
led_on: Bool = false

node init [true][led_on] {
    led_on = true
}

node toggle [led_on][!led_on] {
    led_on = !led_on
}
```

**Target: VHDL**
```vhdl
-- config.vhdl
library IEEE;
use IEEE.std_logic_1164.all;

entity config is
    port (
        clk : in std_logic;
        rst : in std_logic;
        led_out : out std_logic
    );
end entity config;

architecture rtl of config is
    signal led_on : std_logic := '0';
begin
    process(clk, rst) is
    begin
        if rst = '1' then
            led_on <= '0';
        elsif rising_edge(clk) then
            if led_on = '0' then
                led_on <= '1';
            else
                led_on <= not led_on;
            end if;
        end if;
    end process;
    
    led_out <= led_on;
end architecture rtl;
```

---

## 3. Translation Mapping

### 3.1 Brief → VHDL Types

| Brief Type | VHDL Type | Notes |
|------------|-----------|-------|
| `Bool` | `std_logic` | Single bit |
| `UInt[N]` | `std_logic_vector(N-1 downto 0)` | Unsigned |
| `Int[N]` | `signed(N-1 downto 0)` | Signed |
| `Vector[T, N]` | `array(0 to N-1) of T` | Array |
| `String` | `string` | VHDL string |

### 3.2 Transaction → Process

```brief
// Brief
node set_led(value) [true][led == value] {
    led = value
}
```

```vhdl
-- VHDL
set_led: process(value) is
begin
    led <= value;
end process;
```

### 3.3 Guard → Clocked Process

```brief
// Brief - reactive
node toggle [led_on][!led_on] {
    led_on = !led_on
}
```

```vhdl
-- VHDL
toggle_proc: process(clk) is
begin
    if rising_edge(clk) then
        if led_on = '1' then
            led_on <= '0';
        end if;
    end if;
end process;
```

---

## 4. Contracts → PSL Assertions

### 4.1 Brief Contract → VHDL/PSL

```brief
// Brief with contract
node increment [counter < 1000][counter == @counter + 1] {
    counter = @counter + 1
}
```

```vhdl
-- VHDL with PSL
-- psl property counter_wrap is always
--     (counter < to_unsigned(1000, 10) ->
--      next(counter = counter + 1));

architecture rtl of counter is
    signal counter : unsigned(9 downto 0) := (others => '0');
begin
    process(clk, rst) is
    begin
        if rst = '1' then
            counter <= (others => '0');
        elsif rising_edge(clk) then
            if counter < 1000 then
                counter <= counter + 1;
            else
                counter <= (others => '0');
            end if;
        end if;
    end process;
    
    -- PSL assertion
    -- psl assert always (counter <= 1000);
end architecture rtl;
```

---

## 5. Example: KV260 to VHDL

### 5.1 .ebv Source

```brief
// kv260.ebv
ALIAS led: Bool
ALIAS button: Bool

node led_on [button && !led][led] {
    led = true
}

node led_off [led][!led] {
    led = false
}
```

### 5.2 VHDL Output

```vhdl
-- kv260.vhdl
library IEEE;
use IEEE.std_logic_1164.all;

entity kv260 is
    port (
        clk : in std_logic;
        rst : in std_logic;
        button : in std_logic;
        led : out std_logic
    );
end entity kv260;

architecture rtl of kv260 is
    signal led_reg : std_logic := '0';
begin
    -- Main process
    proc_led: process(clk, rst) is
    begin
        if rst = '1' then
            led_reg <= '0';
        elsif rising_edge(clk) then
            if button = '1' and led_reg = '0' then
                led_reg <= '1';
            elsif led_reg = '1' then
                led_reg <= '0';
            end if;
        end if;
    end process;
    
    led <= led_reg;
    
    -- PSL properties
    -- psl default clock is clk;
    -- psl assert led_on_stable: always (led = '1' -> next(led = '0'));
end architecture rtl;
```

---

## 6. Dual Output Support

### 6.1 Target Selection

```brief
// In .ebv or compile command
TARGET sv "./output.sv"
TARGET vhdl "./output.vhdl"

TARGET both  // Generate both
```

### 6.2 Shared Validation

Both outputs share the same contract checking:
- Type verification
- Range checks  
- Temporal logic (via PSL for VHDL, SVA for SV)

---

## 7. Tooling

### 7.1 VHDL Toolchain

| Tool | Purpose |
|------|---------|
| **GHDL** | Open-source simulator |
| **ModelSim** | Commercial simulator |
| **Vivado** | Xilinx synthesis |
| **JasperGold** | Formal verification |
| **Sigifify** | Formal property checking |

### 7.2 Formal Verification Flow

```
.ebv → VHDL → PSL → JasperGold → Proof
```

---

## 8. Summary

| Feature | SV Target | VHDL Target |
|---------|----------|------------|
| **Output** | `.sv` | `.vhd` |
| **Assertions** | SVA | PSL |
| **Best For** | US projects | EU, aerospace |
| **Formal** | Good | Excellent |
| **Verification** | Simulation-based | Property-based |

VHDL provides an additional path for teams requiring:
- DO-254 compliance
- European toolchains
- Advanced formal verification
- ESL/various design methodologies

*Both targets share the same Brief contract verification, ensuring correctness before transpilation.*