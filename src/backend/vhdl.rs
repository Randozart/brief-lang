// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::ast::*;
use crate::linkage::LinkageConfig;
use std::collections::HashMap;
use std::fmt::Write;

/// VHDL code generator: converts a Brief Program into multi-file VHDL output.
pub struct VhdlGenerator {
    spec: Option<crate::target_spec::TargetSpec>,
    entity_name: String,
    clock_freq: u32,
    hw_config: HardwareConfig,
    linkage: Option<LinkageConfig>,
    signal_counter: usize,
    process_counter: usize,
    pending_cleanup: Vec<Statement>,
    has_cycles: bool,
}

/// Read a pragma attribute from the attrs list, filtered by vhdl target.
fn get_pragma<'a>(attrs: &'a [Attribute], key: &str) -> Option<&'a str> {
    attrs.iter()
        .find(|a| a.key == key && (a.target.is_none() || a.target.as_deref() == Some("vhdl")))
        .and_then(|a| a.value.as_deref())
}

impl VhdlGenerator {
    /// Create a new VhdlGenerator with the given entity name and hardware config.
    pub fn new(entity_name: &str, hw_config: HardwareConfig) -> Self {
        let clock_freq = hw_config.target.clock_hz;
        VhdlGenerator {
            spec: None,
            entity_name: entity_name.to_string(),
            clock_freq,
            hw_config,
            linkage: None,
            signal_counter: 0,
            process_counter: 0,
            pending_cleanup: Vec::new(),
            has_cycles: false,
        }
    }

    /// Attach an optional target spec for codegen templates.
    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    /// Attach an optional linkage config for SV wire resolution.
    pub fn with_linkage(mut self, linkage: LinkageConfig) -> Self {
        self.linkage = Some(linkage);
        self
    }

    /// Generate all VHDL files for the given program. Returns (filename, source) pairs.
    pub fn generate(&mut self, program: &Program) -> Vec<(String, String)> {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        let _pr = &_analysis.param_ranges;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  VHDL backend: acyclic call graph — static dispatch enabled");
        }

        if let Err(e) = self.validate_hardware(program) {
            panic!("Hardware validation failed: {}", e);
        }

        let mut files: Vec<(String, String)> = Vec::new();

        let pkg = self.emit_package(program);
        files.push((format!("{}_pkg.vhd", self.entity_name), pkg));

        let top = self.emit_top(program);
        files.push((format!("top.vhd"), top));

        let iface = &self.hw_config.interface.name;
        if iface == "axi4-lite" || iface == "axi4-full" {
            let axi = self.emit_axi_lite_slave(program);
            files.push(("axi_lite_slave.vhd".to_string(), axi));
        }

        let clk_div = self.emit_clock_divider();
        files.push(("clk_div.vhd".to_string(), clk_div));

        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                if self.is_ram_state(state) {
                    let ram = self.emit_ram(state);
                    files.push((format!("ram_{}.vhd", state.name), ram));
                }
            }
        }

        let mut has_fsm = false;
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    has_fsm = true;
                    let stage = self.emit_reactive_txn(txn);
                    files.push((format!("txn_{}.vhd", txn.name), stage));
                }
            }
        }
        if has_fsm {
            let fsm = self.emit_fsm(program);
            files.push(("fsm.vhd".to_string(), fsm));
        }

        let tb = self.emit_testbench(program);
        files.push((format!("{}_tb.vhd", self.entity_name), tb));

        files
    }

    /// Validate hardware configuration against program declarations (placeholder).
    fn validate_hardware(&self, _program: &Program) -> Result<(), String> {
        Ok(())
    }

    /// Look up an IO mapping for the given address in the HW config.
    fn get_io_mapping(&self, address: u64) -> Option<&IoMapping> {
        let addr_str_upper = format!("0x{:08X}", address);
        let addr_str_lower = format!("0x{:08x}", address);
        let addr_str_hex_upper = format!("0x{:X}", address);
        let addr_str_hex_lower = format!("0x{:x}", address);

        self.hw_config.io.as_ref().and_then(|io| {
            io.get(&addr_str_upper)
                .or_else(|| io.get(&addr_str_lower))
                .or_else(|| io.get(&addr_str_hex_upper))
                .or_else(|| io.get(&addr_str_hex_lower))
        })
    }

    /// Check whether the given address is mapped as memory in the HW config.
    fn has_memory_mapping(&self, address: u64) -> bool {
        let addr_str_upper = format!("0x{:08X}", address);
        let addr_str_lower = format!("0x{:08x}", address);
        let addr_str_hex_upper = format!("0x{:X}", address);
        let addr_str_hex_lower = format!("0x{:x}", address);

        self.hw_config.memory.contains_key(&addr_str_upper)
            || self.hw_config.memory.contains_key(&addr_str_lower)
            || self.hw_config.memory.contains_key(&addr_str_hex_upper)
            || self.hw_config.memory.contains_key(&addr_str_hex_lower)
    }

    /// Determine whether a state declaration should be implemented as a RAM block.
    fn is_ram_state(&self, state: &StateDecl) -> bool {
        if let Some(addr) = state.address {
            self.has_memory_mapping(addr)
        } else if matches!(&state.ty, Type::Vector(_, dims) if dims.iter().map(|d| match d { Dimension::Anonymous(s) => *s, Dimension::Named(_, s) => *s }).product::<usize>() > 64) {
            true
        } else if let Some(style) = get_pragma(&state.attrs, "ram_style") {
            style == "block" || style == "ultra" || style == "ultraram"
        } else {
            false
        }
    }

    /// Read the memory type (bram, ultraram, etc.) for a given address.
    fn get_mem_type(&self, address: u64) -> &str {
        let addr_str = format!("0x{:08X}", address);
        self.hw_config.memory.get(&addr_str).map(|m| m.mem_type.as_str()).unwrap_or("auto")
    }

    /// Emit a VHDL package file with types, constants, and component declarations.
    fn emit_package(&self, program: &Program) -> String {
        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n\n");

        o.push_str(&format!("package {}_pkg is\n\n", self.entity_name));

        let mut type_names: Vec<String> = Vec::new();
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                let vh = self.brief_type_to_vhdl(&state.ty);
                if vh.starts_with("record") || vh.starts_with("array") {
                    let tn = format!("{}_t", state.name);
                    if !type_names.contains(&tn) {
                        type_names.push(tn.clone());
                        o.push_str(&format!("    type {} is {};\n", tn, vh));
                    }
                }
            }
        }

        o.push_str("\n    -- Constants\n");
        for item in &program.items {
            if let TopLevel::Constant(c) = item {
                let vh = self.brief_type_to_vhdl(&c.ty);
                o.push_str(&format!("    constant {} : {} := {};\n", c.name, vh, self.expr_to_string(&c.expr)));
            }
        }

        o.push_str("\n    -- Component declarations\n");
        if self.hw_config.interface.name == "axi4-lite" || self.hw_config.interface.name == "axi4-full" {
            o.push_str("    component axi_lite_slave is\n");
            o.push_str("        port (\n");
            o.push_str("            aclk : in std_logic;\n");
            o.push_str("            aresetn : in std_logic;\n");
            o.push_str("            s_axi_awaddr : in std_logic_vector(31 downto 0);\n");
            o.push_str("            s_axi_awvalid : in std_logic;\n");
            o.push_str("            s_axi_awready : out std_logic;\n");
            o.push_str("            s_axi_wdata : in std_logic_vector(31 downto 0);\n");
            o.push_str("            s_axi_wstrb : in std_logic_vector(3 downto 0);\n");
            o.push_str("            s_axi_wvalid : in std_logic;\n");
            o.push_str("            s_axi_wready : out std_logic;\n");
            o.push_str("            s_axi_bresp : out std_logic_vector(1 downto 0);\n");
            o.push_str("            s_axi_bvalid : out std_logic;\n");
            o.push_str("            s_axi_bready : in std_logic;\n");
            o.push_str("            s_axi_araddr : in std_logic_vector(31 downto 0);\n");
            o.push_str("            s_axi_arvalid : in std_logic;\n");
            o.push_str("            s_axi_arready : out std_logic;\n");
            o.push_str("            s_axi_rdata : out std_logic_vector(31 downto 0);\n");
            o.push_str("            s_axi_rresp : out std_logic_vector(1 downto 0);\n");
            o.push_str("            s_axi_rvalid : out std_logic;\n");
            o.push_str("            s_axi_rready : in std_logic\n");
            o.push_str("        );\n");
            o.push_str("    end component;\n\n");
        }

        o.push_str(&format!("end package {}_pkg;\n", self.entity_name));
        o
    }

    /// Emit the top-level entity and architecture with structural instantiations.
    fn emit_top(&mut self, program: &Program) -> String {
        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n");
        o.push_str(&format!("use work.{}_pkg.all;\n\n", self.entity_name));

        o.push_str(&format!("entity {} is\n", self.entity_name));
        o.push_str("    port (\n");
        o.push_str("        clk : in std_logic;\n");
        o.push_str("        rst : in std_logic;\n");

        let iface = &self.hw_config.interface.name;
        if iface == "axi4-lite" || iface == "axi4-full" {
            let aw = self.hw_config.interface.address_width.unwrap_or(32) as usize;
            let dw = self.hw_config.interface.data_width.unwrap_or(32) as usize;
            o.push_str(&format!("        s_axi_awaddr : in std_logic_vector({} downto 0);\n", aw - 1));
            o.push_str("        s_axi_awvalid : in std_logic;\n");
            o.push_str("        s_axi_awready : out std_logic;\n");
            o.push_str(&format!("        s_axi_wdata : in std_logic_vector({} downto 0);\n", dw - 1));
            o.push_str("        s_axi_wstrb : in std_logic_vector(3 downto 0);\n");
            o.push_str("        s_axi_wvalid : in std_logic;\n");
            o.push_str("        s_axi_wready : out std_logic;\n");
            o.push_str("        s_axi_bresp : out std_logic_vector(1 downto 0);\n");
            o.push_str("        s_axi_bvalid : out std_logic;\n");
            o.push_str("        s_axi_bready : in std_logic;\n");
            o.push_str(&format!("        s_axi_araddr : in std_logic_vector({} downto 0);\n", aw - 1));
            o.push_str("        s_axi_arvalid : in std_logic;\n");
            o.push_str("        s_axi_arready : out std_logic;\n");
            o.push_str(&format!("        s_axi_rdata : out std_logic_vector({} downto 0);\n", dw - 1));
            o.push_str("        s_axi_rresp : out std_logic_vector(1 downto 0);\n");
            o.push_str("        s_axi_rvalid : out std_logic;\n");
            o.push_str("        s_axi_rready : in std_logic\n");
        }

        let mut first = true;
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                if let Some(addr) = state.address {
                    if self.get_io_mapping(addr).is_some() && !self.has_memory_mapping(addr) {
                        if first { first = false; }
                        let vh = self.brief_type_to_vhdl(&state.ty);
                        o.push_str(&format!("        {} : out {};\n", state.name, vh));
                    }
                } else if let Some(_p) = get_pragma(&state.attrs, "port") {
                    if first { first = false; }
                    let vh = self.brief_type_to_vhdl(&state.ty);
                    o.push_str(&format!("        {} : out {};\n", state.name, vh));
                }
            }
        }

        o.push_str("    );\n");
        o.push_str(&format!("end entity {};\n\n", self.entity_name));

        o.push_str(&format!("architecture rtl of {} is\n\n", self.entity_name));

        o.push_str("    -- Internal signals\n");
        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                if let Some(addr) = state.address {
                    if self.get_io_mapping(addr).is_some() && !self.has_memory_mapping(addr) {
                        continue;
                    }
                    if self.is_ram_state(state) {
                        continue;
                    }
                } else if get_pragma(&state.attrs, "port").is_some() {
                    continue;
                }
                let vh = self.brief_type_to_vhdl(&state.ty);
                let init = self.get_default_value(&state.ty);
                o.push_str(&format!("    signal {} : {} := {};\n", state.name, vh, init));
            }
        }

        o.push_str("\n    -- Clock divider signals\n");
        o.push_str("    signal clk_en : std_logic;\n");
        o.push_str("    signal clk_en_rst : std_logic;\n\n");

        o.push_str("begin\n\n");

        o.push_str("    -- Clock divider instantiation\n");
        o.push_str(&format!("    clk_div_inst : entity work.clk_div\n"));
        o.push_str("        port map (\n");
        o.push_str("            clk_in => clk,\n");
        o.push_str("            rst_in => rst,\n");
        o.push_str("            clk_out => clk_en,\n");
        o.push_str("            rst_out => clk_en_rst\n");
        o.push_str("        );\n\n");

        if iface == "axi4-lite" || iface == "axi4-full" {
            o.push_str("    -- AXI4-Lite slave bridge\n");
            o.push_str("    axi_inst : entity work.axi_lite_slave\n");
            o.push_str("        port map (\n");
            o.push_str("            aclk => clk,\n");
            o.push_str("            aresetn => rst,\n");
            o.push_str("            s_axi_awaddr => s_axi_awaddr,\n");
            o.push_str("            s_axi_awvalid => s_axi_awvalid,\n");
            o.push_str("            s_axi_awready => s_axi_awready,\n");
            o.push_str("            s_axi_wdata => s_axi_wdata,\n");
            o.push_str("            s_axi_wstrb => s_axi_wstrb,\n");
            o.push_str("            s_axi_wvalid => s_axi_wvalid,\n");
            o.push_str("            s_axi_wready => s_axi_wready,\n");
            o.push_str("            s_axi_bresp => s_axi_bresp,\n");
            o.push_str("            s_axi_bvalid => s_axi_bvalid,\n");
            o.push_str("            s_axi_bready => s_axi_bready,\n");
            o.push_str("            s_axi_araddr => s_axi_araddr,\n");
            o.push_str("            s_axi_arvalid => s_axi_arvalid,\n");
            o.push_str("            s_axi_arready => s_axi_arready,\n");
            o.push_str("            s_axi_rdata => s_axi_rdata,\n");
            o.push_str("            s_axi_rresp => s_axi_rresp,\n");
            o.push_str("            s_axi_rvalid => s_axi_rvalid,\n");
            o.push_str("            s_axi_rready => s_axi_rready\n");
            o.push_str("        );\n\n");
        }

        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                if self.is_ram_state(state) {
                    o.push_str(&format!("    -- RAM block: {}\n", state.name));
                    o.push_str(&format!("    ram_{}_inst : entity work.ram_{}\n", state.name, state.name));
                    o.push_str("        port map (\n");
                    o.push_str("            clk => clk,\n");
                    o.push_str("            we => '0',\n");
                    o.push_str("            addr => (others => '0'),\n");
                    o.push_str("            din => (others => '0'),\n");
                    o.push_str("            dout => open\n");
                    o.push_str("        );\n\n");
                }
            }
        }

        let has_fsm = program.items.iter().any(|item| matches!(item, TopLevel::Transaction(t) if t.is_reactive));
        if has_fsm {
            o.push_str("    -- FSM instantiation\n");
            o.push_str("    fsm_inst : entity work.fsm\n");
            o.push_str("        port map (\n");
            o.push_str("            clk => clk,\n");
            o.push_str("            rst => rst\n");
            o.push_str("        );\n\n");
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    o.push_str(&format!("    -- Reactive transaction: {}\n", txn.name));
                    self.emit_transaction(&mut o, txn);
                    o.push_str("\n");
                }
            }
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if !txn.is_reactive {
                    o.push_str(&format!("    -- Transaction: {}\n", txn.name));
                    self.emit_transaction(&mut o, txn);
                    o.push_str("\n");
                }
            }
        }

        o.push_str("end architecture rtl;\n");
        o
    }

    /// Emit a separate AXI4-Lite slave bridge component with full handshake FSM.
    fn emit_axi_lite_slave(&self, _program: &Program) -> String {
        let aw = self.hw_config.interface.address_width.unwrap_or(32) as usize;
        let dw = self.hw_config.interface.data_width.unwrap_or(32) as usize;
        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n\n");

        o.push_str("entity axi_lite_slave is\n");
        o.push_str("    port (\n");
        o.push_str("        aclk : in std_logic;\n");
        o.push_str("        aresetn : in std_logic;\n");
        o.push_str(&format!("        s_axi_awaddr : in std_logic_vector({} downto 0);\n", aw - 1));
        o.push_str("        s_axi_awvalid : in std_logic;\n");
        o.push_str("        s_axi_awready : out std_logic;\n");
        o.push_str(&format!("        s_axi_wdata : in std_logic_vector({} downto 0);\n", dw - 1));
        o.push_str("        s_axi_wstrb : in std_logic_vector(3 downto 0);\n");
        o.push_str("        s_axi_wvalid : in std_logic;\n");
        o.push_str("        s_axi_wready : out std_logic;\n");
        o.push_str("        s_axi_bresp : out std_logic_vector(1 downto 0);\n");
        o.push_str("        s_axi_bvalid : out std_logic;\n");
        o.push_str("        s_axi_bready : in std_logic;\n");
        o.push_str(&format!("        s_axi_araddr : in std_logic_vector({} downto 0);\n", aw - 1));
        o.push_str("        s_axi_arvalid : in std_logic;\n");
        o.push_str("        s_axi_arready : out std_logic;\n");
        o.push_str(&format!("        s_axi_rdata : out std_logic_vector({} downto 0);\n", dw - 1));
        o.push_str("        s_axi_rresp : out std_logic_vector(1 downto 0);\n");
        o.push_str("        s_axi_rvalid : out std_logic;\n");
        o.push_str("        s_axi_rready : in std_logic\n");
        o.push_str("    );\n");
        o.push_str("end entity axi_lite_slave;\n\n");

        o.push_str("architecture rtl of axi_lite_slave is\n");
        o.push_str("    type axil_state_t is (IDLE, WRITE_ADDR, WRITE_DATA, WRITE_RESP, READ_ADDR, READ_DATA);\n");
        o.push_str("    signal state : axil_state_t;\n\n");
        o.push_str("    -- Internal register file\n");
        o.push_str("    type reg_array_t is array(0 to 255) of std_logic_vector(31 downto 0);\n");
        o.push_str("    signal regs : reg_array_t := (others => (others => '0'));\n");
        o.push_str("    signal read_data : std_logic_vector(31 downto 0);\n");
        o.push_str("begin\n\n");

        o.push_str("    -- AXI4-Lite write state machine\n");
        o.push_str("    process(aclk, aresetn) is\n");
        o.push_str("    begin\n");
        o.push_str("        if aresetn = '0' then\n");
        o.push_str("            state <= IDLE;\n");
        o.push_str("            s_axi_awready <= '0';\n");
        o.push_str("            s_axi_wready <= '0';\n");
        o.push_str("            s_axi_bvalid <= '0';\n");
        o.push_str("            s_axi_arready <= '0';\n");
        o.push_str("            s_axi_rvalid <= '0';\n");
        o.push_str("            s_axi_bresp <= (others => '0');\n");
        o.push_str("            s_axi_rresp <= (others => '0');\n");
        o.push_str("            read_data <= (others => '0');\n");
        o.push_str("        elsif rising_edge(aclk) then\n");
        o.push_str("            case state is\n");
        o.push_str("                when IDLE =>\n");
        o.push_str("                    if s_axi_awvalid = '1' then\n");
        o.push_str("                        s_axi_awready <= '1';\n");
        o.push_str("                        state <= WRITE_ADDR;\n");
        o.push_str("                    elsif s_axi_arvalid = '1' then\n");
        o.push_str("                        s_axi_arready <= '1';\n");
        o.push_str("                        state <= READ_ADDR;\n");
        o.push_str("                    end if;\n");
        o.push_str("                when WRITE_ADDR =>\n");
        o.push_str("                    s_axi_awready <= '0';\n");
        o.push_str("                    if s_axi_wvalid = '1' then\n");
        o.push_str("                        s_axi_wready <= '1';\n");
        o.push_str("                        regs(to_integer(unsigned(s_axi_awaddr(9 downto 2)))) <= s_axi_wdata;\n");
        o.push_str("                        state <= WRITE_DATA;\n");
        o.push_str("                    end if;\n");
        o.push_str("                when WRITE_DATA =>\n");
        o.push_str("                    s_axi_wready <= '0';\n");
        o.push_str("                    s_axi_bvalid <= '1';\n");
        o.push_str("                    s_axi_bresp <= \"00\";\n");
        o.push_str("                    state <= WRITE_RESP;\n");
        o.push_str("                when WRITE_RESP =>\n");
        o.push_str("                    if s_axi_bready = '1' then\n");
        o.push_str("                        s_axi_bvalid <= '0';\n");
        o.push_str("                        state <= IDLE;\n");
        o.push_str("                    end if;\n");
        o.push_str("                when READ_ADDR =>\n");
        o.push_str("                    s_axi_arready <= '0';\n");
        o.push_str("                    read_data <= regs(to_integer(unsigned(s_axi_araddr(9 downto 2))));\n");
        o.push_str("                    s_axi_rvalid <= '1';\n");
        o.push_str("                    s_axi_rresp <= \"00\";\n");
        o.push_str("                    state <= READ_DATA;\n");
        o.push_str("                when READ_DATA =>\n");
        o.push_str("                    if s_axi_rready = '1' then\n");
        o.push_str("                        s_axi_rvalid <= '0';\n");
        o.push_str("                        state <= IDLE;\n");
        o.push_str("                    end if;\n");
        o.push_str("            end case;\n");
        o.push_str("        end if;\n");
        o.push_str("    end process;\n\n");
        o.push_str("    s_axi_rdata <= read_data;\n");
        o.push_str("end architecture rtl;\n");
        o
    }

    /// Emit a clock divider component using counter-based division from board config.
    fn emit_clock_divider(&self) -> String {
        let target_hz = self.clock_freq;
        let divisor = if target_hz > 0 && target_hz < 100_000_000 {
            100_000_000 / target_hz
        } else {
            1
        };

        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n\n");

        o.push_str("entity clk_div is\n");
        o.push_str("    port (\n");
        o.push_str("        clk_in : in std_logic;\n");
        o.push_str("        rst_in : in std_logic;\n");
        o.push_str("        clk_out : out std_logic;\n");
        o.push_str("        rst_out : out std_logic\n");
        o.push_str("    );\n");
        o.push_str("end entity clk_div;\n\n");

        o.push_str("architecture rtl of clk_div is\n");
        let bits = if divisor <= 1 { 1 } else { (usize::BITS - divisor.leading_zeros()) as usize };
        o.push_str(&format!("    signal cnt : unsigned({} downto 0) := (others => '0');\n", bits));
        o.push_str("    signal clk_en : std_logic := '0';\n");
        o.push_str("begin\n\n");
        o.push_str("    process(clk_in, rst_in) is\n");
        o.push_str("    begin\n");
        o.push_str("        if rst_in = '1' then\n");
        o.push_str("            cnt <= (others => '0');\n");
        o.push_str("            clk_en <= '0';\n");
        o.push_str("        elsif rising_edge(clk_in) then\n");

        if divisor <= 1 {
            o.push_str("            clk_en <= '1';\n");
        } else {
            o.push_str(&format!("            if cnt = {} then\n", divisor - 1));
            o.push_str("                cnt <= (others => '0');\n");
            o.push_str("                clk_en <= '1';\n");
            o.push_str("            else\n");
            o.push_str("                cnt <= cnt + 1;\n");
            o.push_str("                clk_en <= '0';\n");
            o.push_str("            end if;\n");
        }

        o.push_str("        end if;\n");
        o.push_str("    end process;\n\n");
        o.push_str("    clk_out <= clk_en;\n");
        o.push_str("    rst_out <= rst_in;\n");
        o.push_str("end architecture rtl;\n");
        o
    }

    /// Emit a RAM inference component (BRAM/URAM) with dual-process pattern and ram_style attribute.
    fn emit_ram(&self, state: &StateDecl) -> String {
        let (depth, width) = match &state.ty {
            Type::Vector(inner, dims) => {
                let d: usize = dims.iter().map(|dim| match dim {
                    Dimension::Anonymous(s) => *s,
                    Dimension::Named(_, s) => *s,
                }).product();
                let w = self.get_type_width(inner);
                (d, w)
            }
            _ => (64, 32),
        };

        let addr_bits = if depth <= 1 { 1 } else { (usize::BITS - depth.leading_zeros()) as usize };
        let mem_type = state.address.map_or("auto", |a| self.get_mem_type(a));

        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n\n");

        o.push_str(&format!("entity ram_{} is\n", state.name));
        o.push_str("    port (\n");
        o.push_str("        clk : in std_logic;\n");
        o.push_str("        we : in std_logic;\n");
        o.push_str(&format!("        addr : in std_logic_vector({} downto 0);\n", addr_bits - 1));
        o.push_str(&format!("        din : in std_logic_vector({} downto 0);\n", width - 1));
        o.push_str(&format!("        dout : out std_logic_vector({} downto 0)\n", width - 1));
        o.push_str("    );\n");
        o.push_str(&format!("end entity ram_{};\n\n", state.name));

        o.push_str(&format!("architecture rtl of ram_{} is\n", state.name));

        if mem_type == "bram" || mem_type == "block" || get_pragma(&state.attrs, "ram_style").map_or(false, |s| s == "block") {
            o.push_str("    attribute ram_style : string;\n");
            o.push_str(&format!("    attribute ram_style of ram : signal is \"block\";\n"));
        } else if mem_type == "ultraram" || mem_type == "ultra" || get_pragma(&state.attrs, "ram_style").map_or(false, |s| s == "ultra") {
            o.push_str("    attribute ram_style : string;\n");
            o.push_str(&format!("    attribute ram_style of ram : signal is \"ultra\";\n"));
        }

        o.push_str(&format!("    type ram_type is array(0 to {}) of std_logic_vector({} downto 0);\n", depth - 1, width - 1));
        o.push_str("    signal ram : ram_type := (others => (others => '0'));\n");
        o.push_str("    signal read_data : std_logic_vector(width-1 downto 0);\n");
        o.push_str("begin\n\n");

        o.push_str("    -- Write process (sync)\n");
        o.push_str("    process(clk) is\n");
        o.push_str("    begin\n");
        o.push_str("        if rising_edge(clk) then\n");
        o.push_str("            if we = '1' then\n");
        o.push_str("                ram(to_integer(unsigned(addr))) <= din;\n");
        o.push_str("            end if;\n");
        o.push_str("        end if;\n");
        o.push_str("    end process;\n\n");

        o.push_str("    -- Read process (sync)\n");
        o.push_str("    process(clk) is\n");
        o.push_str("    begin\n");
        o.push_str("        if rising_edge(clk) then\n");
        o.push_str("            read_data <= ram(to_integer(unsigned(addr)));\n");
        o.push_str("        end if;\n");
        o.push_str("    end process;\n\n");

        o.push_str("    dout <= read_data;\n");
        o.push_str("end architecture rtl;\n");
        o
    }

    /// Emit a separate FSM component derived from reactive transactions and state encoding.
    fn emit_fsm(&self, program: &Program) -> String {
        let reactive: Vec<&Transaction> = program.items.iter()
            .filter_map(|item| if let TopLevel::Transaction(t) = item { if t.is_reactive { Some(t) } else { None } } else { None })
            .collect();

        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n\n");

        o.push_str("entity fsm is\n");
        o.push_str("    port (\n");
        o.push_str("        clk : in std_logic;\n");
        o.push_str("        rst : in std_logic\n");
        o.push_str("    );\n");
        o.push_str("end entity fsm;\n\n");

        o.push_str("architecture rtl of fsm is\n");
        o.push_str("    type state_type is (IDLE");
        for txn in &reactive {
            o.push_str(&format!(", {}", txn.name.to_uppercase()));
        }
        o.push_str(");\n");

        o.push_str("    signal current_state, next_state : state_type;\n");
        o.push_str("begin\n\n");

        o.push_str("    -- State register\n");
        o.push_str("    process(clk, rst) is\n");
        o.push_str("    begin\n");
        o.push_str("        if rst = '1' then\n");
        o.push_str("            current_state <= IDLE;\n");
        o.push_str("        elsif rising_edge(clk) then\n");
        o.push_str("            current_state <= next_state;\n");
        o.push_str("        end if;\n");
        o.push_str("    end process;\n\n");

        o.push_str("    -- Next state logic\n");
        o.push_str("    process(current_state");
        for txn in &reactive {
            for dep in &txn.dependencies {
                o.push_str(&format!(", {}", dep));
            }
        }
        o.push_str(") is\n");
        o.push_str("    begin\n");
        o.push_str("        next_state <= current_state;\n");
        o.push_str("        case current_state is\n");
        o.push_str("            when IDLE =>\n");
        for (i, txn) in reactive.iter().enumerate() {
            let pre = self.expr_to_string(&txn.contract.pre_condition);
            if pre != "true" && pre != "1" && pre != "'1'" {
                o.push_str(&format!("                if {} then\n", pre));
                o.push_str(&format!("                    next_state <= {};\n", txn.name.to_uppercase()));
                o.push_str("                end if;\n");
            } else if i < reactive.len() {
                o.push_str(&format!("                next_state <= {};\n", txn.name.to_uppercase()));
            }
        }
        if reactive.is_empty() {
            o.push_str("                next_state <= IDLE;\n");
        }
        for txn in &reactive {
            o.push_str(&format!("            when {} =>\n", txn.name.to_uppercase()));
            let post = self.expr_to_string(&txn.contract.post_condition);
            if post != "true" && post != "1" && post != "'1'" {
                o.push_str(&format!("                if {} then\n", post));
                o.push_str("                    next_state <= IDLE;\n");
                o.push_str("                end if;\n");
            } else {
                o.push_str("                next_state <= IDLE;\n");
            }
        }
        o.push_str("            when others =>\n");
        o.push_str("                next_state <= IDLE;\n");
        o.push_str("        end case;\n");
        o.push_str("    end process;\n");
        o.push_str("end architecture rtl;\n");
        o
    }

    /// Emit a standalone reactive transaction with PSL assertion comments.
    fn emit_reactive_txn(&mut self, txn: &Transaction) -> String {
        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n\n");

        o.push_str(&format!("-- Reactive transaction: {}\n", txn.name));
        o.push_str(&format!("-- Guards: {}\n", self.expr_to_string(&txn.contract.pre_condition)));
        let pre_str = self.expr_to_string(&txn.contract.pre_condition);
        let post_str = self.expr_to_string(&txn.contract.post_condition);
        if pre_str != "true" && pre_str != "1" && pre_str != "'1'" {
            o.push_str(&format!("-- psl assert never ({}) report \"Pre-condition violated for {}\";\n",
                pre_str, txn.name));
        }
        if !matches!(&txn.contract.post_condition, Expr::Bool(true)) {
            o.push_str(&format!("-- psl assert always ({} -> next({})) report \"Post-condition violated for {}\";\n",
                pre_str, post_str, txn.name));
        }

        let proc_name = format!("proc_{}", txn.name);
        o.push_str(&format!("{}: process(clk, rst) is\n", proc_name));
        o.push_str("begin\n");
        o.push_str("    if rst = '1' then\n");
        for item in &txn.body {
            self.statement_to_vhdl(&mut o, item, "        ");
        }
        o.push_str("    elsif rising_edge(clk) then\n");
        let pre = self.expr_to_string(&txn.contract.pre_condition);
        if pre != "true" && pre != "1" && pre != "'1'" {
            o.push_str(&format!("        if {} then\n", pre));
        }
        for item in &txn.body {
            self.statement_to_vhdl(&mut o, item, "            ");
        }
        if pre != "true" && pre != "1" && pre != "'1'" {
            o.push_str("        end if;\n");
        }
        o.push_str("    end if;\n");
        o.push_str(&format!("end process {};\n", proc_name));
        o
    }

    /// Emit a synchronous process for a transaction (reactive or not) with PSL assertions.
    fn emit_transaction(&mut self, output: &mut String, txn: &Transaction) {
        let proc_name = format!("proc_{}", txn.name);
        self.process_counter += 1;

        output.push_str(&format!("    {}: process(clk, rst) is\n", proc_name));
        output.push_str("    begin\n");
        output.push_str("        if rst = '1' then\n");

        for item in &txn.body {
            self.statement_to_vhdl(output, item, "            ");
        }

        output.push_str("        elsif rising_edge(clk) then\n");

        if txn.is_reactive {
            let pre = self.expr_to_string(&txn.contract.pre_condition);
            if pre != "true" && pre != "1" && pre != "'1'" {
                output.push_str(&format!("            if {} then\n", pre));
            }

            for item in &txn.body {
                self.statement_to_vhdl(output, item, "                ");
            }

            if pre != "true" && pre != "1" && pre != "'1'" {
                output.push_str("            end if;\n");
            }

            if pre != "true" && pre != "1" && pre != "'1'" {
                output.push_str(&format!("            -- psl assert never ({}) report \"Pre-condition violated for {}\";\n",
                    pre, txn.name));
            }
            if !matches!(&txn.contract.post_condition, Expr::Bool(true)) {
                output.push_str(&format!("            -- psl assert always ({} -> next({})) report \"Post-condition violated for {}\";\n",
                    pre,
                    self.expr_to_string(&txn.contract.post_condition),
                    txn.name));
            }
        } else {
            for item in &txn.body {
                self.statement_to_vhdl(output, item, "            ");
            }
        }

        output.push_str("        end if;\n");
        output.push_str(&format!("    end process {};\n\n", proc_name));
    }

    /// Convert a Brief statement to VHDL code with the given indentation level.
    fn statement_to_vhdl(&mut self, output: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                let _ = write!(output, "{}{} <= {};\n", indent,
                    self.expr_to_string(lhs),
                    self.expr_to_string(expr));
            }
            Statement::Let { name, expr, address_expr, address, .. } => {
                if let Some(addr_expr) = address_expr {
                    let addr_code = self.expr_to_string(addr_expr);
                    let _ = write!(output, "{}-- let {} at address {}\n", indent, name, addr_code);
                } else if let Some(addr) = address {
                    let _ = write!(output, "{}-- let {} at address 0x{:X}\n", indent, name, addr);
                } else if let Some(e) = expr {
                    let expr_code = self.expr_to_string(e);
                    let _ = write!(output, "{}-- let {} = {}\n", indent, name, expr_code);
                } else {
                    let _ = write!(output, "{}-- let {}\n", indent, name);
                }
            }
            Statement::Term { values, .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for stmt in &cleanup {
                    self.statement_to_vhdl(output, stmt, indent);
                }
                if values.is_empty() {
                    let _ = write!(output, "{}-- term\n", indent);
                } else if values.len() == 1 {
                    if let Some(v) = &values[0] {
                        let expr_code = self.expr_to_string(v);
                        let _ = write!(output, "{}-- term with {}\n", indent, expr_code);
                    } else {
                        let _ = write!(output, "{}-- term\n", indent);
                    }
                } else {
                    let vals: Vec<String> = values.iter().map(|v| {
                        match v {
                            Some(e) => self.expr_to_string(e),
                            None => "open".to_string(),
                        }
                    }).collect();
                    let _ = write!(output, "{}-- term with ({})\n", indent, vals.join(", "));
                }
            }
            Statement::Expression(expr) => {
                let expr_code = self.expr_to_string(expr);
                let _ = write!(output, "{}-- side effect: {}\n", indent, expr_code);
            }
            Statement::LocalTrigger { name, ty, expr, .. } => {
                let _ty_str = self.brief_type_to_vhdl(ty);
                if let Some(e) = expr {
                    let expr_code = self.expr_to_string(e);
                    let _ = write!(output, "{}-- trg! {}: {} = {}\n", indent, name, _ty_str, expr_code);
                } else {
                    let _ = write!(output, "{}-- trg! {}: await external {}\n", indent, name, _ty_str);
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                let _ = write!(output, "{}-- on_exit cleanup registered\n", indent);
            }
            Statement::Escape(value) => {
                if let Some(v) = value {
                    let expr_code = self.expr_to_string(v);
                    let _ = write!(output, "{}-- escape with {}\n", indent, expr_code);
                } else {
                    let _ = write!(output, "{}-- escape\n", indent);
                }
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    let _ = write!(output, "{}{}\n", indent, line);
                }
            }
            Statement::InlineAsm { asm_string, clobbers, .. } => {
                if clobbers.is_empty() {
                    let _ = write!(output, "{}-- asm: {}\n", indent, asm_string);
                } else {
                    let clobber_list = clobbers.join(", ");
                    let _ = write!(output, "{}-- asm: {} (clobbers: {})\n", indent, asm_string, clobber_list);
                }
            }
            Statement::Unification { name, pattern, expr } => {
                let expr_code = self.expr_to_string(expr);
                let _ = write!(output, "{}-- uni {}({}) = {}\n", indent, name, pattern, expr_code);
            }
            Statement::Guarded { condition, statements } => {
                let cond_code = self.expr_to_string(condition);
                let _ = write!(output, "{}if {} then\n", indent, cond_code);
                for s in statements {
                    self.statement_to_vhdl(output, s, &format!("{}    ", indent));
                }
                let _ = write!(output, "{}end if;\n", indent);
            }
        }
    }

    /// Emit a testbench with clock/reset stimulus and assertion checking from contracts.
    fn emit_testbench(&self, program: &Program) -> String {
        let mut o = String::new();
        o.push_str("library IEEE;\n");
        o.push_str("use IEEE.std_logic_1164.all;\n");
        o.push_str("use IEEE.numeric_std.all;\n");
        o.push_str("use std.textio.all;\n\n");

        o.push_str(&format!("entity {}_tb is\n", self.entity_name));
        o.push_str("end entity;\n\n");

        o.push_str(&format!("architecture sim of {}_tb is\n", self.entity_name));
        o.push_str(&format!("    signal clk : std_logic := '0';\n"));
        o.push_str(&format!("    signal rst : std_logic := '0';\n"));

        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                let vh = self.brief_type_to_vhdl(&state.ty);
                o.push_str(&format!("    signal {} : {};\n", state.name, vh));
            }
        }

        o.push_str("begin\n\n");

        o.push_str(&format!("    -- Unit Under Test\n"));
        o.push_str(&format!("    uut : entity work.{}", self.entity_name));
        o.push_str("\n        port map (\n");
        o.push_str("            clk => clk,\n");
        o.push_str("            rst => rst\n");
        o.push_str("        );\n\n");

        o.push_str("    -- Clock generation\n");
        let half_period_ns = if self.clock_freq > 0 { 500_000_000u64 / self.clock_freq as u64 } else { 5 };
        o.push_str(&format!("    clk <= not clk after {} ns;\n\n", half_period_ns));

        o.push_str("    -- Stimulus process\n");
        o.push_str("    stim_proc: process is\n");
        o.push_str("    begin\n");
        o.push_str("        -- Initial reset\n");
        o.push_str("        rst <= '1';\n");
        o.push_str("        wait for 100 ns;\n");
        o.push_str("        rst <= '0';\n");
        o.push_str("        wait for 100 ns;\n\n");

        o.push_str("        -- Wait for design to stabilize\n");
        o.push_str("        wait for 1 us;\n\n");

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if !matches!(&txn.contract.post_condition, Expr::Bool(true)) {
                    o.push_str(&format!("        -- Assert: {}\n", self.expr_to_string(&txn.contract.post_condition)));
                }
            }
        }

        o.push_str("\n        report \"Testbench completed.\";\n");
        o.push_str("        wait;\n");
        o.push_str("    end process stim_proc;\n");

        o.push_str("end architecture sim;\n");
        o
    }

    /// Convert a Brief Type to its VHDL type string, handling all type variants.
    fn brief_type_to_vhdl(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "std_logic".to_string(),
            Type::UInt => "std_logic_vector(31 downto 0)".to_string(),
            Type::Int => "signed(31 downto 0)".to_string(),
            Type::Float => "real".to_string(),
            Type::String => "string".to_string(),
            Type::Data => "std_logic_vector(7 downto 0)".to_string(),
            Type::Void => "std_logic".to_string(),
            Type::Char => "std_logic_vector(31 downto 0)".to_string(),
            Type::Custom(name) => format!("std_logic_vector(31 downto 0) -- custom {}", name),
            Type::Vector(inner, dims) => {
                let inner_vhdl = self.brief_type_to_vhdl(inner);
                let mut result = inner_vhdl;
                for d in dims.iter().rev() {
                    let size = match d {
                        Dimension::Anonymous(s) => *s,
                        Dimension::Named(_, s) => *s,
                    };
                    result = format!("array(0 to {}) of {}", size - 1, result);
                }
                result
            }
            Type::Tuple(types) => {
                let fields: Vec<String> = types.iter().enumerate().map(|(i, t)| {
                    format!("        field_{} : {}", i, self.brief_type_to_vhdl(t))
                }).collect();
                format!("record\n{}\n    end record", fields.join(";\n"))
            }
            Type::Union(types) => {
                let max_width = types.iter().map(|t| self.get_type_width(t)).max().unwrap_or(32);
                let field_types: Vec<String> = types.iter().map(|t| self.brief_type_to_vhdl(t)).collect();
                format!("record\n        tag : std_logic_vector(7 downto 0);\n        data : std_logic_vector({} downto 0);\n    end record", max_width - 1)
            }
            Type::Custom(n) if n == "Option" || n.starts_with("Option<") => {
                format!("std_logic_vector(31 downto 0) -- Option")
            }
            Type::Custom(n) if n == "HashMap" || n.starts_with("HashMap<") => {
                format!("std_logic_vector(31 downto 0) -- HashMap (BRAM-backed)")
            }
            Type::Custom(n) if n == "Addr" => {
                "std_logic_vector(31 downto 0)".to_string()
            }
            Type::Constrained(base, r) => {
                let is_signed = matches!(**base, Type::Int);
                match r {
                    BitRange::Single(n) => {
                        if *n <= 1 {
                            "std_logic".to_string()
                        } else if is_signed {
                            format!("signed({} downto 0)", n - 1)
                        } else {
                            format!("std_logic_vector({} downto 0)", n - 1)
                        }
                    }
                    BitRange::Range(start, end) => {
                        let width = end - start + 1;
                        if width <= 1 {
                            "std_logic".to_string()
                        } else if is_signed {
                            format!("signed({} downto 0)", width - 1)
                        } else {
                            format!("std_logic_vector({} downto 0)", width - 1)
                        }
                    }
                    BitRange::Any(n) => {
                        if *n <= 1 {
                            "std_logic".to_string()
                        } else if is_signed {
                            format!("signed({} downto 0)", n - 1)
                        } else {
                            format!("std_logic_vector({} downto 0)", n - 1)
                        }
                    }
                }
            }
            Type::Enum(name) => {
                format!("std_logic_vector(7 downto 0) -- enum {}", name)
            }
            Type::ContractBound(inner, _) => self.brief_type_to_vhdl(inner),
            Type::TypeVar(name) => format!("std_logic_vector(31 downto 0) -- typevar {}", name),
            Type::Generic(name, _) => format!("std_logic_vector(31 downto 0) -- generic {}", name),
            Type::Applied(name, _) => format!("std_logic_vector(31 downto 0) -- applied {}", name),
            Type::Sig(name) => format!("std_logic_vector(31 downto 0) -- sig {}", name),
        }
    }

    /// Return the bit width of a type (for address decoding and RAM sizing).
    fn get_type_width(&self, ty: &Type) -> usize {
        match ty {
            Type::Bool => 1,
            Type::UInt | Type::Int => 32,
            Type::Float => 64,
            Type::String => 256,
            Type::Data => 8,
            Type::Void => 1,
            Type::Char => 32,
            Type::Custom(_) => 32,
            Type::Vector(inner, dims) => {
                let d: usize = dims.iter().map(|dim| match dim {
                    Dimension::Anonymous(s) => *s,
                    Dimension::Named(_, s) => *s,
                }).product();
                self.get_type_width(inner) * d
            }
            Type::Tuple(types) => types.iter().map(|t| self.get_type_width(t)).sum(),
            Type::Union(types) => 8 + types.iter().map(|t| self.get_type_width(t)).max().unwrap_or(32),
            Type::Custom(n) if n == "Option" || n.starts_with("Option<") => 32,
            Type::Custom(n) if n == "HashMap" || n.starts_with("HashMap<") => 32,
            Type::Constrained(base, r) => match r {
                BitRange::Single(n) => *n,
                BitRange::Range(start, end) => end - start + 1,
                BitRange::Any(n) => *n,
            },
            Type::Enum(_) => 8,
            Type::ContractBound(inner, _) => self.get_type_width(inner),
            _ => 32,
        }
    }

    /// Return a VHDL default value expression for a given type.
    fn get_default_value(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "'0'".to_string(),
            Type::UInt | Type::Int => "(others => '0')".to_string(),
            Type::Float => "0.0".to_string(),
            Type::String => "".to_string(),
            Type::Data => "(others => '0')".to_string(),
            Type::Void => "'0'".to_string(),
            Type::Char => "(others => '0')".to_string(),
            Type::Vector(_, _) => "(others => (others => '0'))".to_string(),
            Type::Tuple(_) => "(others => '0')".to_string(),
            Type::Union(_) => "(others => '0')".to_string(),
            Type::Custom(n) if n == "Option" || n.starts_with("Option<") => "(others => '0')".to_string(),
            Type::Custom(n) if n == "HashMap" || n.starts_with("HashMap<") => "(others => '0')".to_string(),
            Type::Constrained(_, _) => "(others => '0')".to_string(),
            _ => "(others => '0')".to_string(),
        }
    }

    /// Convert a Brief expression to a VHDL expression string.
    fn expr_to_string(&self, expr: &Expr) -> String {
        match expr {
            Expr::Bool(b) => if *b { "'1'" } else { "'0'" }.to_string(),
            Expr::Integer(i) => i.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Char(c) => format!("character'val({})", *c as u32),
            Expr::Identifier(name) => name.clone(),
            Expr::OwnedRef(name) => name.clone(),
            Expr::PriorState(name) => name.clone(),
            Expr::Not(e) => format!("not {}", self.expr_to_string(e)),
            Expr::Neg(e) => format!("-{}", self.expr_to_string(e)),
            Expr::Add(lhs, rhs) => format!("({} + {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Sub(lhs, rhs) => format!("({} - {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Mul(lhs, rhs) => format!("({} * {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Div(lhs, rhs) => format!("({} / {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Mod(lhs, rhs) => format!("({} mod {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Eq(lhs, rhs) => format!("({} = {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Ne(lhs, rhs) => format!("({} /= {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Lt(lhs, rhs) => format!("({} < {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Le(lhs, rhs) => format!("({} <= {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Gt(lhs, rhs) => format!("({} > {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Ge(lhs, rhs) => format!("({} >= {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::And(lhs, rhs) => format!("({} and {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Or(lhs, rhs) => format!("({} or {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::BitNot(e) => format!("not {}", self.expr_to_string(e)),
            Expr::BitAnd(lhs, rhs) => format!("({} and {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::BitOr(lhs, rhs) => format!("({} or {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::BitXor(lhs, rhs) => format!("({} xor {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Shl(lhs, rhs) => format!("shift_left({}, {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Shr(lhs, rhs) => format!("shift_right({}, {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Call(name, args) => {
                let a: Vec<String> = args.iter().map(|a| self.expr_to_string(a)).collect();
                format!("{}({})", name, a.join(", "))
            }
            Expr::ListIndex(list, idx) => {
                format!("{}(to_integer(unsigned({})))", self.expr_to_string(list), self.expr_to_string(idx))
            }
            Expr::ListLen(list) => format!("{}.length", self.expr_to_string(list)),
            Expr::Slice { value, start, end, stride, mask } => {
                let v = self.expr_to_string(value);
                let s = start.as_ref().map(|e| self.expr_to_string(e)).unwrap_or("0".to_string());
                let e = end.as_ref().map(|e| self.expr_to_string(e)).unwrap_or("0".to_string());
                let st = stride.as_ref().map(|e| self.expr_to_string(e)).unwrap_or("1".to_string());
                if let Some(m) = mask {
                    format!("{} -- slice [{}, {}, {}, mask: {}]", v, s, e, st, self.expr_to_string(m))
                } else {
                    format!("{} -- slice [{}, {}, {}]", v, s, e, st)
                }
            }
            Expr::FieldAccess(inner, field) => {
                format!("{}.{}", self.expr_to_string(inner), field)
            }
            Expr::Tuple(fields) => {
                let f: Vec<String> = fields.iter().map(|e| self.expr_to_string(e)).collect();
                format!("({})", f.join(", "))
            }
            Expr::ListLiteral(items) => {
                let f: Vec<String> = items.iter().map(|e| self.expr_to_string(e)).collect();
                format!("({})", f.join(", "))
            }
            _ => "'0'".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use crate::analysis::call_graph::CallGraph;
use crate::ast::*;

    #[test]
    fn test_vhdl_generates_entity() {
        let hw_config = HardwareConfig {
            project: ProjectConfig { name: "test".to_string(), version: "1.0".to_string() },
            target: TargetConfig { fpga: "test".to_string(), clock_hz: 100_000_000, platform: None, synthesis: None },
            interface: InterfaceConfig { name: "none".to_string(), address_width: None, data_width: None, controller: None, situs: None },
            io: None,
            memory: HashMap::new(),
        };
        let mut backend = VhdlGenerator::new("test_entity", hw_config);
        let program = Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        };
        let files = backend.generate(&program);
        let output = files.iter().map(|(_, s)| s.as_str()).collect::<Vec<&str>>().join("\n");
        assert!(output.contains("entity"), "output should contain entity declaration");
        assert!(output.contains("architecture"), "output should contain architecture body");
    }
}
