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
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use crate::analysis::call_graph::CallGraph;
use crate::ast::*;
use crate::linkage::LinkageConfig;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct RamWrite {
    condition: String,
    address_expr: String,
    data_expr: String,
}

pub struct VerilogGenerator {
    spec: Option<crate::target_spec::TargetSpec>,
    module_name: String,
    clock_freq: u32,
    hw_config: HardwareConfig,
    linkage: Option<LinkageConfig>,
    _indent_level: usize,
    output: String,
    pending_cleanup: Vec<Statement>,
    has_cycles: bool,
}

impl VerilogGenerator {
    pub fn new(module_name: &str, hw_config: HardwareConfig) -> Self {
        let clock_freq = hw_config.target.clock_hz;
        VerilogGenerator {
            spec: None,
            module_name: module_name.to_string(),
            clock_freq,
            hw_config,
            linkage: None,
            _indent_level: 0,
            output: String::new(),
            pending_cleanup: Vec::new(),
            has_cycles: false,
        }
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn with_linkage(mut self, linkage: LinkageConfig) -> Self {
        self.linkage = Some(linkage);
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        let _pr = &_analysis.param_ranges;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  Verilog backend: acyclic call graph — static dispatch enabled");
        }

        self.output.clear();

        if let Err(e) = self.validate_hardware(program) {
            panic!("Hardware validation failed: {}", e);
        }

        if let Some(spec) = &self.spec {
            if let Some(cg) = &spec.codegen {
                if let Some(header) = &cg.templates.header {
                    self.output.push_str(&format!("// Spec header from {}\n", spec.target.as_ref().map(|t| t.name.as_str()).unwrap_or("unknown")));
                    self.output.push_str(header);
                    self.output.push_str("\n\n");
                }
            }
        }

        self.emit_header(program);

        // Emit clock dividers for reactor speeds
        self.emit_clock_dividers(program);

        // Define internal signals
        self.emit_signals(program);

        // Define functions (definitions)
        self.emit_definitions(program);

        // Define logic
        self.emit_logic(program);

        self.emit_footer();
        self.output.push_str("\n");

        if let Some(spec) = &self.spec {
            if let Some(cg) = &spec.codegen {
                if let Some(footer) = &cg.templates.footer {
                    self.output.push_str(footer);
                    self.output.push_str("\n");
                }
            }
        }

        self.output.clone()
    }

    pub fn generate_auto(&mut self, program: &Program) -> String {
        let iface = &self.hw_config.interface.name;
        if iface == "axi4-lite" || iface == "axi4-full" {
            self.generate_with_axi(program)
        } else {
            self.generate(program)
        }
    }

    pub fn generate_with_axi(&mut self, program: &Program) -> String {
        self.output.clear();

        if let Err(e) = self.validate_hardware(program) {
            panic!("Hardware validation failed: {}", e);
        }

        let address_width = self.hw_config.interface.address_width.unwrap_or(16) as usize;
        let data_width = self.hw_config.interface.data_width.unwrap_or(32) as usize;

        self.output
            .push_str(&format!("module {} (\n", self.module_name));
        self.output.push_str("    input logic clk,\n");
        self.output.push_str("    input logic rst_n,\n");

        self.output.push_str(&format!(
            "    // AXI4-Lite write address channel\n    input  logic [{}:0] s_awaddr,\n",
            address_width - 1
        ));
        self.output.push_str("    input  logic       s_awvalid,\n");
        self.output.push_str("    output logic       s_awready,\n");

        self.output.push_str(&format!(
            "    // AXI4-Lite write data channel\n    input  logic [{}:0] s_wdata,\n",
            data_width - 1
        ));
        self.output.push_str("    input  logic [3:0]  s_wstrb,\n");
        self.output.push_str("    input  logic       s_wvalid,\n");
        self.output.push_str("    output logic       s_wready,\n");

        self.output.push_str("    // AXI4-Lite write response channel\n");
        self.output.push_str("    output logic [1:0] s_bresp,\n");
        self.output.push_str("    output logic       s_bvalid,\n");
        self.output.push_str("    input  logic       s_bready,\n");

        self.output.push_str(&format!(
            "    // AXI4-Lite read address channel\n    input  logic [{}:0] s_araddr,\n",
            address_width - 1
        ));
        self.output.push_str("    input  logic       s_arvalid,\n");
        self.output.push_str("    output logic       s_arready,\n");

        self.output.push_str(&format!(
            "    // AXI4-Lite read data channel\n    output logic [{}:0] s_rdata,\n",
            data_width - 1
        ));
        self.output.push_str("    output logic [1:0] s_rresp,\n");
        self.output.push_str("    output logic       s_rvalid,\n");
        self.output.push_str("    input  logic       s_rready\n");

        self.output.push_str(");\n\n");

        self.emit_axi_state_machine(program, address_width, data_width);
        self.emit_clock_dividers(program);
        self.emit_signals(program);
        self.emit_definitions(program);
        self.emit_logic(program);
        self.emit_footer();

        self.output.clone()
    }

    fn emit_axi_state_machine(&mut self, program: &Program, _addr_width: usize, _data_width: usize) {
        self.output.push_str(
            "    // AXI4-Lite State Machine (Law 2: The Envoy)\n",
        );

        self.output.push_str("    logic [1:0] axil_state;\n");
        self.output.push_str("    localparam AXIL_IDLE = 2'd0;\n");
        self.output.push_str("    localparam AXIL_WRITE = 2'd1;\n");
        self.output.push_str("    localparam AXIL_WWAIT = 2'd2;\n");
        self.output.push_str("    localparam AXIL_RWAIT = 2'd3;\n");

        self.output.push_str("    // CPU interface signals\n");
        self.output.push_str("    logic [31:0] cpu_write_data;\n");
        self.output.push_str("    logic [17:0] cpu_write_addr;\n");
        self.output.push_str("    logic       cpu_write_en;\n");
        self.output.push_str("    logic [31:0] cpu_read_data;\n");
        self.output.push_str("    logic       cpu_read_en;\n");

        self.output.push_str("    always_ff @(posedge clk) begin\n");
        self.output.push_str("        if (!rst_n) begin\n");
        self.output.push_str("            axil_state <= AXIL_IDLE;\n");
        self.output.push_str("            s_awready <= 1'b0;\n");
        self.output.push_str("            s_wready <= 1'b0;\n");
        self.output.push_str("            s_bvalid <= 1'b0;\n");
        self.output.push_str("            s_arready <= 1'b0;\n");
        self.output.push_str("            s_rvalid <= 1'b0;\n");
        self.output.push_str("            cpu_write_en <= 1'b0;\n");
        self.output.push_str("            cpu_read_en <= 1'b0;\n");
        self.output.push_str("        end else begin\n");
        self.output.push_str("            case (axil_state)\n");
        self.output.push_str("                AXIL_IDLE: begin\n");
        self.output.push_str("                    if (s_awvalid) begin\n");
        self.output.push_str("                        s_awready <= 1'b1;\n");
        self.output.push_str("                        cpu_write_addr <= s_awaddr[17:0];\n");
        self.output.push_str("                        cpu_write_data <= s_wdata;\n");
        self.output.push_str("                        axil_state <= AXIL_WRITE;\n");
        self.output.push_str("                    end else if (s_arvalid) begin\n");
        self.output.push_str("                        s_arready <= 1'b1;\n");
        self.output.push_str("                        cpu_read_en <= 1'b1;\n");
        self.output.push_str("                        axil_state <= AXIL_RWAIT;\n");
        self.output.push_str("                    end\n");
        self.output.push_str("                end\n");
        self.output.push_str("                AXIL_WRITE: begin\n");
        self.output.push_str("                    s_awready <= 1'b0;\n");
        self.output.push_str("                    if (s_wvalid) begin\n");
        self.output.push_str("                        s_wready <= 1'b1;\n");
        self.output.push_str("                        cpu_write_en <= 1'b1;\n");
        self.output.push_str("                        axil_state <= AXIL_WWAIT;\n");
        self.output.push_str("                    end\n");
        self.output.push_str("                end\n");
        self.output.push_str("                AXIL_WWAIT: begin\n");
        self.output.push_str("                    s_wready <= 1'b0;\n");
        self.output.push_str("                    cpu_write_en <= 1'b0;\n");
        self.output.push_str("                    s_bvalid <= 1'b1;\n");
        self.output.push_str("                    s_bresp <= 2'b00;\n");
        self.output.push_str("                    if (s_bready) begin\n");
        self.output.push_str("                        s_bvalid <= 1'b0;\n");
        self.output.push_str("                        axil_state <= AXIL_IDLE;\n");
        self.output.push_str("                    end\n");
        self.output.push_str("                end\n");
        self.output.push_str("                AXIL_RWAIT: begin\n");
        self.output.push_str("                    s_arready <= 1'b0;\n");
        self.output.push_str("                    s_rvalid <= 1'b1;\n");
        self.output.push_str("                    s_rresp <= 2'b00;\n");
        self.output.push_str("                    if (s_rready) begin\n");
        self.output.push_str("                        s_rvalid <= 1'b0;\n");
        self.output.push_str("                        cpu_read_en <= 1'b0;\n");
        self.output.push_str("                        axil_state <= AXIL_IDLE;\n");
        self.output.push_str("                    end\n");
        self.output.push_str("                end\n");
        self.output.push_str("            endcase\n");
        self.output.push_str("        end\n");
        self.output.push_str("    end\n\n");

        self.output.push_str("    // Read data multiplexer\n");
        self.output.push_str("    always_ff @(posedge clk) begin\n");
        self.output.push_str("        if (!rst_n) begin\n");
        self.output.push_str("            cpu_read_data <= '0;\n");
        self.output.push_str("        end else begin\n");
        self.output.push_str("            case (cpu_write_addr[7:2])\n");
        self.output.push_str("                // Map address to register\n");
        self.output.push_str("                default: cpu_read_data <= '0;\n");
        self.output.push_str("            endcase\n");
        self.output.push_str("        end\n");
        self.output.push_str("    end\n");
        self.output.push_str("    assign s_rdata = cpu_read_data;\n\n");
    }

    fn emit_header(&mut self, program: &Program) {
        self.output
            .push_str(&format!("module {} (\n", self.module_name));
        self.output.push_str("    input logic clk,\n");
        self.output.push_str("    input logic rst_n");

        // Collect ports from StateDecls with addresses
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if let Some(addr) = decl.address {
                        // Only emit as port if in [io] AND NOT in [memory]
                        if let Some(io_cfg) = self.get_io_mapping(addr) {
                            if !self.has_memory_mapping(addr) {
                                let width = self.get_bit_width(&decl.ty, decl.bit_range.as_ref());
                                let direction = io_cfg.direction.as_deref().unwrap_or("output");

                                match &decl.ty {
                                    Type::Vector(inner, dims) => {
                                        let element_bits =
                                            self.get_bit_width(inner, decl.bit_range.as_ref());
                                        let signed = if matches!(**inner, Type::Int) {
                                            "signed "
                                        } else {
                                            ""
                                        };

                                        let total_size: usize = dims.iter().map(|d| match d {
                                            crate::ast::Dimension::Anonymous(s) => *s,
                                            crate::ast::Dimension::Named(_, s) => *s,
                                        }).product();

                                        let mut attr = "";
                                        let addr_str_upper = format!("0x{:08X}", addr);
                                        let addr_str_lower = format!("0x{:08x}", addr);
                                        let addr_str_hex_upper = format!("0x{:X}", addr);
                                        let addr_str_hex_lower = format!("0x{:x}", addr);

                                        let mem_cfg = self
                                            .hw_config
                                            .memory
                                            .get(&addr_str_upper)
                                            .or_else(|| self.hw_config.memory.get(&addr_str_lower))
                                            .or_else(|| {
                                                self.hw_config.memory.get(&addr_str_hex_upper)
                                            })
                                            .or_else(|| {
                                                self.hw_config.memory.get(&addr_str_hex_lower)
                                            });

                                        if let Some(mem_cfg) = mem_cfg {
                                            attr = match mem_cfg.mem_type.as_str() {
                                                "bram" => "(* ram_style = \"block\" *) ",
                                                "ultraram" => "(* ram_style = \"ultra\" *) ",
                                                "distributed" => "(* ram_style = \"distributed\" *) ",
                                                _ => "",
                                            };
                                        }

                                        self.output.push_str(&format!(
                                            ",\n    {} {}logic {}{} {} [0:{}]{} /* pin: {} */",
                                            attr, direction, signed,
                                            if element_bits > 1 {
                                                format!("[{}:0]", element_bits - 1)
                                            } else {
                                                "".to_string()
                                            },
                                            decl.name,
                                            total_size - 1,
                                            attr,
                                            io_cfg.pin
                                        ));
                                    }
                                    _ => {
                                        self.output.push_str(&format!(
                                            ",\n    {} logic {} {} /* pin: {} */",
                                            direction,
                                            if width > 1 {
                                                format!("[{}:0]", width - 1)
                                            } else {
                                                "".to_string()
                                            },
                                            decl.name,
                                            io_cfg.pin
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                TopLevel::Trigger(trg) => {
                    match &trg.address {
                        LinkRef::Explicit(addr) => {
                            // Traditional: look up address in IO mappings
                            if let Some(io_cfg) = self.get_io_mapping(*addr) {
                                if !self.has_memory_mapping(*addr) {
                                    let width = self.get_bit_width(&trg.ty, trg.bit_range.as_ref());
                                    let direction = "input";
                                    self.output.push_str(&format!(
                                        ",\n    {} logic {} {} /* pin: {} */",
                                        direction,
                                        if width > 1 {
                                            format!("[{}:0]", width - 1)
                                        } else {
                                            "".to_string()
                                        },
                                        trg.name,
                                        io_cfg.pin
                                    ));
                                }
                            }
                        }
                        LinkRef::Linked(name) => {
                            // Linked: get SV wire name from linkage config
                            if let Some(linkage) = &self.linkage {
                                if let Some(sv_wire) = linkage.resolve_sv(name) {
                                    let width = self.get_bit_width(&trg.ty, trg.bit_range.as_ref());
                                    self.output.push_str(&format!(
                                        ",\n    {} logic {} {} /* link: {} */",
                                        "input",
                                        if width > 1 {
                                            format!("[{}:0]", width - 1)
                                        } else {
                                            "".to_string()
                                        },
                                        trg.name,
                                        sv_wire
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.output.push_str("\n);\n\n");
    }

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

    fn get_link_ref_address(&self, link_ref: &LinkRef) -> Option<u64> {
        match link_ref {
            LinkRef::Explicit(addr) => Some(*addr),
            LinkRef::Linked(name) => {
                if let Some(linkage) = &self.linkage {
                    linkage.resolve_sv(name).and_then(|_| None)
                } else {
                    None
                }
            }
        }
    }

    fn get_link_ref_sv_wire(&self, link_ref: &LinkRef) -> Option<&str> {
        match link_ref {
            LinkRef::Explicit(_) => None,
            LinkRef::Linked(name) => {
                self.linkage.as_ref().and_then(|l| l.resolve_sv(name))
            }
        }
    }

    fn emit_clock_dividers(&mut self, program: &Program) {
        let mut speeds = HashSet::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if let Some(speed) = txn.reactor_speed {
                    speeds.insert(speed);
                }
            }
        }

        for speed in speeds {
            let divisor = self.clock_freq / speed;
            self.output
                .push_str(&format!("    // Clock enable for {}Hz\n", speed));
            self.output
                .push_str(&format!("    logic ce_{}hz;\n", speed));
            self.output
                .push_str(&format!("    logic [31:0] div_cnt_{}hz;\n", speed));
            self.output.push_str("    always_ff @(posedge clk) begin\n");
            self.output.push_str("        if (!rst_n) begin\n");
            self.output
                .push_str(&format!("            div_cnt_{}hz <= 0;\n", speed));
            self.output
                .push_str(&format!("            ce_{}hz <= 0;\n", speed));
            self.output.push_str("        end else begin\n");
            self.output.push_str(&format!(
                "            if (div_cnt_{}hz == {}) begin\n",
                speed,
                divisor - 1
            ));
            self.output
                .push_str(&format!("                div_cnt_{}hz <= 0;\n", speed));
            self.output
                .push_str(&format!("                ce_{}hz <= 1;\n", speed));
            self.output.push_str("            end else begin\n");
            self.output.push_str(&format!(
                "                div_cnt_{}hz <= div_cnt_{}hz + 1;\n",
                speed, speed
            ));
            self.output
                .push_str(&format!("                ce_{}hz <= 0;\n", speed));
            self.output.push_str("            end\n");
            self.output.push_str("        end\n");
            self.output.push_str("    end\n\n");
        }
    }

    fn emit_signals(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                // Skip if it was emitted as a port in the header
                // (i.e. has [io] mapping BUT NO [memory] mapping)
                if let Some(addr) = decl.address {
                    if self.get_io_mapping(addr).is_some() && !self.has_memory_mapping(addr) {
                        continue;
                    }
                }

                self.emit_type_signals(&decl.name, &decl.ty, decl.bit_range.as_ref(), decl.address);
            }
            if let TopLevel::Trigger(trg) = item {
                match &trg.address {
                    LinkRef::Explicit(addr) => {
                        if self.get_io_mapping(*addr).is_some()
                            && !self.has_memory_mapping(*addr)
                        {
                            continue;
                        }
                        self.emit_type_signals(
                            &trg.name,
                            &trg.ty,
                            trg.bit_range.as_ref(),
                            Some(*addr),
                        );
                    }
                    LinkRef::Linked(name) => {
                        if let Some(linkage) = &self.linkage {
                            if let Some(sv_wire) = linkage.resolve_sv(name) {
                                let width = self.get_bit_width(&trg.ty, trg.bit_range.as_ref());
                                self.output.push_str(&format!(
                                    "    {}logic {} {} /* link: {} */;\n",
                                    "",
                                    if width > 1 {
                                        format!("[{}:0]", width - 1)
                                    } else {
                                        "".to_string()
                                    },
                                    trg.name,
                                    sv_wire
                                ));
                            }
                        }
                    }
                }
            }
        }
        self.output.push_str("\n");
    }

    fn emit_type_signals(
        &mut self,
        name: &str,
        ty: &Type,
        range: Option<&BitRange>,
        address: Option<u64>,
    ) {
        match ty {
            Type::Union(types) => {
                self.output
                    .push_str(&format!("    // Union type signals for {}\n", name));
                for t in types {
                    let suffix = if self.is_error_type(t) {
                        "_err"
                    } else {
                        "_data"
                    };
                    self.emit_type_signals(&format!("{}{}", name, suffix), t, range, address);
                }
                self.output
                    .push_str(&format!("    logic [7:0] {}_tag;\n", name));
            }
            Type::Tuple(types) => {
                self.output
                    .push_str(&format!("    // Tuple type signals for {}\n", name));
                for (i, t) in types.iter().enumerate() {
                    self.emit_type_signals(&format!("{}_{}", name, i), t, range, address);
                }
            }
            Type::Vector(inner, dims) => {
                let width = self.get_bit_width(inner, range);
                let signed = if matches!(**inner, Type::Int) {
                    "signed "
                } else {
                    ""
                };
                let width_str = if width > 1 {
                    format!("[{}:0]", width - 1)
                } else {
                    "".to_string()
                };

                // Calculate total size (product of all dimensions)
                let total_size: usize = dims.iter().map(|d| match d {
                    crate::ast::Dimension::Anonymous(s) => *s,
                    crate::ast::Dimension::Named(_, s) => *s,
                }).product();

                let mut attr = "";
                let mut suffix = "";
                if let Some(addr) = address {
                    let addr_str_upper = format!("0x{:08X}", addr);
                    let addr_str_lower = format!("0x{:08x}", addr);
                    let addr_str_hex_upper = format!("0x{:X}", addr);
                    let addr_str_hex_lower = format!("0x{:x}", addr);

                    let mem_cfg = self
                        .hw_config
                        .memory
                        .get(&addr_str_upper)
                        .or_else(|| self.hw_config.memory.get(&addr_str_lower))
                        .or_else(|| self.hw_config.memory.get(&addr_str_hex_upper))
                        .or_else(|| self.hw_config.memory.get(&addr_str_hex_lower));

                    if let Some(mem_cfg) = mem_cfg {
                        attr = match mem_cfg.mem_type.as_str() {
                            "bram" => "(* ram_style = \"block\" *) ",
                            "ultraram" => "(* ram_style = \"ultra\" *) ",
                            "distributed" => "(* ram_style = \"distributed\" *) ",
                            _ => "",
                        };
                        suffix = " /* synthesis keep */";
                    } else {
                        suffix = " /* synthesis keep */";
                    }
                }

                self.output.push_str(&format!(
                    "    {}logic {}{} {} [0:{}]{};\n",
                    attr,
                    signed,
                    width_str,
                    name,
                    total_size - 1,
                    suffix
                ));
            }
            Type::Constrained(inner, r) => {
                self.emit_type_signals(name, inner, Some(r), address);
            }
            _ => {
                let width = self.get_bit_width(ty, range);
                let signed = if matches!(ty, Type::Int) {
                    "signed "
                } else {
                    ""
                };
                let width_str = if width > 1 {
                    format!("[{}:0]", width - 1)
                } else {
                    "".to_string()
                };
                self.output
                    .push_str(&format!("    logic {}{} {};\n", signed, width_str, name));
            }
        }
    }

    fn is_error_type(&self, ty: &Type) -> bool {
        if let Type::Custom(name) = ty {
            name == "Error"
        } else {
            false
        }
    }

    fn get_bit_width(&self, ty: &Type, range: Option<&BitRange>) -> usize {
        if let Some(range) = range {
            match range {
                BitRange::Single(_) => 1,
                BitRange::Range(start, end) => end - start + 1,
                BitRange::Any(n) => *n,
            }
        } else {
            match ty {
                Type::Int | Type::UInt => 32,
                Type::Bool => 1,
                Type::Vector(inner, _) => self.get_bit_width(inner, None),
                Type::Constrained(inner, r) => self.get_bit_width(inner, Some(r)),
                _ => 32,
            }
        }
    }

    fn emit_definitions(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                let ret_ty = defn.outputs.first().unwrap_or(&Type::Int);
                let ret_width = self.get_bit_width(ret_ty, None);
                let signed = if matches!(ret_ty, Type::Int) {
                    "signed "
                } else {
                    ""
                };

                self.output.push_str(&format!(
                    "    function automatic logic {}{}[{}:0] {}(\n",
                    signed,
                    "",
                    ret_width - 1,
                    defn.name
                ));

                for (i, (name, ty)) in defn.parameters.iter().enumerate() {
                    let width = self.get_bit_width(ty, None);
                    let p_signed = if matches!(ty, Type::Int) {
                        "signed "
                    } else {
                        ""
                    };
                    self.output.push_str(&format!(
                        "        input logic {}{} {} {}\n",
                        p_signed,
                        if width > 1 {
                            format!("[{}:0]", width - 1)
                        } else {
                            "".to_string()
                        },
                        name,
                        if i == defn.parameters.len() - 1 {
                            ""
                        } else {
                            ","
                        }
                    ));
                }
                self.output.push_str("    );\n");
                self.emit_function_body(&defn.name, &defn.body);
                self.output.push_str("    endfunction\n\n");
            }
        }
    }

    fn emit_function_body(&mut self, fn_name: &str, body: &[Statement]) {
        for stmt in body {
            match stmt {
                Statement::Term { values: outputs, .. } | Statement::TermBang { values: outputs, .. } => {
                    if let Some(Some(expr)) = outputs.first() {
                        self.output
                            .push_str(&format!("        return {};\n", self.expr_to_verilog(expr)));
                    }
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    self.output.push_str(&format!(
                        "        if ({}) begin\n",
                        self.expr_to_verilog(condition)
                    ));
                    self.emit_function_body(fn_name, statements);
                    self.output.push_str("        end\n");
                }
                _ => {}
            }
        }
    }
    fn emit_logic(&mut self, program: &Program) {
        let mut write_map: HashMap<String, Vec<&Transaction>> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    let mut writes = HashSet::new();
                    self.collect_writes(&txn.body, &mut writes);
                    for var in writes {
                        write_map.entry(var).or_default().push(txn);
                    }
                }
            }
        }

        // Emit always_ff for each state variable
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                self.emit_variable_logic(
                    &decl.name,
                    decl.expr.as_ref(),
                    write_map.get(&decl.name).cloned().unwrap_or_default(),
                    program,
                );
            }
        }
    }

    fn collect_writes(&self, body: &[Statement], writes: &mut HashSet<String>) {
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, .. } => {
                    if let Some(name) = self.extract_root_var(lhs) {
                        writes.insert(name);
                    }
                }
                Statement::Guarded { statements, .. } => {
                    self.collect_writes(statements, writes);
                }
                _ => {}
            }
        }
    }

    fn extract_root_var(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name) | Expr::OwnedRef(name) | Expr::PriorState(name) => {
                Some(name.clone())
            }
            Expr::ListIndex(inner, _)
            | Expr::Slice { value: inner, .. }
            | Expr::FieldAccess(inner, _) => self.extract_root_var(inner),
            _ => None,
        }
    }

    fn emit_variable_logic(
        &mut self,
        name: &str,
        init_expr: Option<&Expr>,
        txns: Vec<&Transaction>,
        program: &Program,
    ) {
        let decl = program
            .items
            .iter()
            .find_map(|item| {
                if let TopLevel::StateDecl(d) = item {
                    if d.name == name {
                        Some(d)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();

        let is_union = matches!(decl.ty, Type::Union(_));

        // Check if any txn has a timeout for this variable
        let mut has_any_timeout = false;
        for txn in &txns {
            if self.has_timeout_for_var(name, &txn.body) {
                has_any_timeout = true;
                break;
            }
        }

        if has_any_timeout {
            self.output
                .push_str(&format!("    // Timeout watchdog for {}\n", name));
            self.output
                .push_str(&format!("    logic [31:0] {}_timeout_cnt;\n", name));
            self.output
                .push_str(&format!("    logic {}_waiting;\n", name));
        }

        let (is_vector, vector_size) = match &decl.ty {
            Type::Vector(_, dims) => {
                let total_size: usize = dims.iter().map(|d| match d {
                    crate::ast::Dimension::Anonymous(s) => *s,
                    crate::ast::Dimension::Named(_, s) => *s,
                }).product();
                (true, total_size)
            }
            _ => (false, 1),
        };

        self.output
            .push_str(&format!("    // Logic for variable: {}\n", name));

        // Check memory type for this address
        let mem_type = if let Some(addr) = decl.address {
            let addr_str = format!("0x{:08X}", addr);
            self.hw_config.memory.get(&addr_str).map(|m| m.mem_type.clone())
        } else {
            None
        };

        // Determine generation style based on memory type
        // bram/ultraram -> RAM template (single always_ff with address)
        // flipflop or unknown -> generate for loop (current behavior)
        let use_ram_template = matches!(mem_type.as_deref(), Some("bram") | Some("ultraram"));

        if is_vector && use_ram_template && vector_size > 64 {
            // Law 1: RAM Multiplexer - consolidate all writes into single port
            // BRAM/UltraRAM have exactly 2 ports - we use 1 for writes
            self.output.push_str(&format!(
                "    // RAM template for {} (type: {:?}, size: {})\n",
                name, mem_type, vector_size
            ));

            // Collect all indexed writes from all transactions
            let all_writes = self.collect_ram_writes(name, &txns, program);

            // Generate address/data mux signals
            let addr_bits = (vector_size as f64).log2() as usize + 1;
            self.output.push_str("    // Law 1: Single write port mux\n");
            self.output.push_str(&format!(
                "    logic [{}:0] s_waddr;\n",
                addr_bits.saturating_sub(1)
            ));
            self.output.push_str("    logic s_we;\n");

            // Emit data signal with proper width
            let data_width = self.get_bit_width(&decl.ty, decl.bit_range.as_ref());
            self.output.push_str(&format!(
                "    logic [{}:0] s_wdata;\n",
                data_width.saturating_sub(1)
            ));

            // Generate priority encoder (last txn wins)
            self.output.push_str("    always_comb begin\n");
            self.output.push_str("        s_we = 1'b0;\n");
            self.output.push_str("        s_waddr = '0;\n");
            self.output.push_str("        s_wdata = '0;\n");

            for write in all_writes.iter().rev() {
                self.output.push_str(&format!(
                    "        if ({}) begin\n",
                    write.condition
                ));
                self.output.push_str("            s_we = 1'b1;\n");
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

            // Single BRAM write using muxed signals
            self.output.push_str("    always_ff @(posedge clk) begin\n");
            self.output.push_str("        if (s_we) begin\n");
            self.output.push_str(&format!(
                "            {}[s_waddr] <= s_wdata;\n",
                name
            ));
            self.output.push_str("        end\n");
            self.output.push_str("    end\n\n");
        } else if is_vector {
            // Original generate-for pattern for small vectors or flipflop
            let genvar_name = format!("{}_i", name);
            self.output
                .push_str(&format!("    genvar {};\n", genvar_name));
            self.output.push_str(&format!(
                "    generate\n        for ({} = 0; {} < {}; {} = {} + 1) begin : {}_logic\n",
                genvar_name, genvar_name, vector_size, genvar_name, genvar_name, name
            ));
            self.output
                .push_str("            always_ff @(posedge clk) begin\n");
            self.output.push_str("                if (!rst_n) begin\n");

            if let Some(expr) = init_expr {
                self.output.push_str(&format!(
                    "                    {}[{}] <= {};\n",
                    name,
                    genvar_name,
                    self.expr_to_verilog(expr)
                ));
            } else {
                self.output.push_str(&format!(
                    "                    {}[{}] <= 0;\n",
                    name, genvar_name
                ));
            }

            self.output.push_str("                end else begin\n");

            for (idx, txn) in txns.iter().enumerate() {
                let ce_cond = if let Some(speed) = txn.reactor_speed {
                    format!("ce_{}hz && ", speed)
                } else {
                    "".to_string()
                };

                let cond = format!(
                    "{}{}",
                    ce_cond,
                    self.expr_to_verilog(&txn.contract.pre_condition)
                );

                self.output.push_str(&format!(
                    "                    {}if ({}) begin\n",
                    if idx > 0 { "else " } else { "" },
                    cond
                ));
                for stmt in &txn.body {
                    if let Statement::Assignment { lhs, expr: rhs, .. } = stmt {
                        if let Expr::ListIndex(val, _) = lhs {
                            if let Expr::Identifier(var_name) = &**val {
                                if var_name == name {
                                     self.emit_vector_assignment_from_txn(name, &txn.body, program);
                                }
                            }
                        }
                    }
                }
                self.output.push_str("                    end\n");
            }

            self.output.push_str("                end\n");
            self.output.push_str("            end\n");
            self.output.push_str("        end\n    endgenerate\n\n");
        } else {
            self.output.push_str("    always_ff @(posedge clk) begin\n");
            self.output.push_str("        if (!rst_n) begin\n");

            if is_union {
                self.output
                    .push_str(&format!("            {}_data <= 0;\n", name));
                self.output
                    .push_str(&format!("            {}_err <= 0;\n", name));
                self.output
                    .push_str(&format!("            {}_tag <= 0;\n", name));
            } else {
                if let Some(expr) = init_expr {
                    self.output.push_str(&format!(
                        "            {} <= {};\n",
                        name,
                        self.expr_to_verilog(expr)
                    ));
                } else {
                    self.output
                        .push_str(&format!("            {} <= 0;\n", name));
                }
            }

            if has_any_timeout {
                self.output
                    .push_str(&format!("            {}_waiting <= 0;\n", name));
                self.output
                    .push_str(&format!("            {}_timeout_cnt <= 0;\n", name));
            }

            self.output.push_str("        end else begin\n");

            // Handle timeout countdown
            if has_any_timeout {
                self.output
                    .push_str(&format!("            if ({}_waiting) begin\n", name));
                self.output.push_str(&format!(
                    "                if ({}_timeout_cnt > 0) begin\n",
                    name
                ));
                self.output.push_str(&format!(
                    "                    {}_timeout_cnt <= {}_timeout_cnt - 1;\n",
                    name, name
                ));
                self.output.push_str("                end else begin\n");
                self.output
                    .push_str(&format!("                    {}_waiting <= 0;\n", name));
                if is_union {
                    self.output.push_str(&format!(
                        "                    {}_err <= 1; // Driving Error variant\n",
                        name
                    ));
                    self.output.push_str(&format!(
                        "                    {}_tag <= 1; // Assuming 1 is Err\n",
                        name
                    ));
                }
                self.output.push_str("                end\n");
                self.output.push_str("            end\n");
            }

            for (i, txn) in txns.iter().enumerate() {
                let ce_cond = if let Some(speed) = txn.reactor_speed {
                    format!("ce_{}hz && ", speed)
                } else {
                    "".to_string()
                };

                let cond = format!(
                    "{}{}",
                    ce_cond,
                    self.expr_to_verilog(&txn.contract.pre_condition)
                );

                self.output.push_str(&format!(
                    "            {}if ({}) begin\n",
                    if i > 0 { "else " } else { "" },
                    cond
                ));
                self.emit_var_assignment_from_txn(name, &txn.body, program);
                self.output.push_str("            end\n");
            }

            self.output.push_str("        end\n");
            self.output.push_str("    end\n\n");
        }
    }

    fn has_timeout_for_var(&self, var_name: &str, body: &[Statement]) -> bool {
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, timeout, .. } => {
                    if self.extract_root_var(lhs).as_deref() == Some(var_name) && timeout.is_some()
                    {
                        return true;
                    }
                }
                Statement::Guarded { statements, .. } => {
                    if self.has_timeout_for_var(var_name, statements) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn is_union_variable(&self, name: &str, program: &Program) -> bool {
        program.items.iter().any(|item| {
            if let TopLevel::StateDecl(d) = item {
                if d.name == name {
                    return matches!(d.ty, Type::Union(_));
                }
            }
            false
        })
    }

    fn extract_assignment_target(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::OwnedRef(name) => Some(name.clone()),
            Expr::ListIndex(inner, _) => self.extract_assignment_target(inner),
            _ => None,
        }
    }

    fn emit_var_assignment_from_txn(
        &mut self,
        var_name: &str,
        body: &[Statement],
        program: &Program,
    ) {
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, timeout, .. } => {
                    if self.extract_assignment_target(lhs).as_deref() == Some(var_name) {
                        if let Some((t_expr, _unit)) = timeout {
                            self.output
                                .push_str(&format!("                {}_waiting <= 1;\n", var_name));
                            self.output.push_str(&format!(
                                "                {}_timeout_cnt <= {};\n",
                                var_name,
                                self.expr_to_verilog(t_expr)
                            ));
                        }

                        let is_union = self.is_union_variable(var_name, program);
                        let final_name = if is_union {
                            format!("{}_data", var_name)
                        } else {
                            var_name.to_string()
                        };

                        let lhs_sv = self.lhs_to_verilog(lhs, &final_name);

                        self.output.push_str(&format!(
                            "                {} <= {};\n",
                            lhs_sv,
                            self.expr_to_verilog(expr)
                        ));
                        if is_union {
                            self.output.push_str(&format!(
                                "                {}_tag <= 0; // Assuming 0 is Ok\n",
                                var_name
                            ));
                        }
                    }
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    self.output.push_str(&format!(
                        "                if ({}) begin\n",
                        self.expr_to_verilog(condition)
                    ));
                    self.emit_var_assignment_from_txn(var_name, statements, program);
                    self.output.push_str("                end\n");
                }
                _ => {}
            }
        }
    }

    fn lhs_to_verilog(&self, lhs: &Expr, root_name: &str) -> String {
        match lhs {
            Expr::Identifier(_) | Expr::OwnedRef(_) => root_name.to_string(),
            Expr::ListIndex(inner, idx) => {
                format!(
                    "{}[{}]",
                    self.lhs_to_verilog(inner, root_name),
                    self.expr_to_verilog(idx)
                )
            }
            _ => root_name.to_string(),
        }
    }

    fn emit_vector_assignment_from_txn(
        &mut self,
        var_name: &str,
        body: &[Statement],
        program: &Program,
    ) {
        let genvar_name = format!("{}_i", var_name);

        // Collect all vector names from program for lifting
        let vector_names: Vec<String> = program
            .items
            .iter()
            .filter_map(|item| {
                if let TopLevel::StateDecl(decl) = item {
                    if let Type::Vector(_, dims) = &decl.ty {
                        let total: usize = dims.iter().map(|d| match d {
                            crate::ast::Dimension::Anonymous(s) => *s,
                            crate::ast::Dimension::Named(_, s) => *s,
                        }).product();
                        if total > 1 {
                            return Some(decl.name.clone());
                        }
                    }
                }
                None
            })
            .collect();

        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if self.extract_assignment_target(lhs).as_deref() == Some(var_name) {
                        let expr_str = self.expr_to_verilog(expr);

                        // Lift all vector references in the expression (but not already indexed ones)
                        let mut lifted_expr = expr_str.clone();
                        for vec_name in &vector_names {
                            // Only replace if not already indexed in original expr
                            let pattern = format!("{}[", vec_name);
                            if !expr_str.contains(&pattern) {
                                // Match only standalone word vec_name to avoid partial matches
                                // and replace it with vec_name[genvar_name]
                                let re = regex::Regex::new(&format!(r"\b{}\b", vec_name)).unwrap();
                                lifted_expr = re
                                    .replace_all(
                                        &lifted_expr,
                                        &format!("{}[{}]", vec_name, genvar_name),
                                    )
                                    .to_string();
                            }
                        }

                        match lhs {
                            Expr::Identifier(_) | Expr::OwnedRef(_) => {
                                self.output.push_str(&format!(
                                    "                        {}[{}] <= {};\n",
                                    var_name, genvar_name, lifted_expr
                                ));
                            }
                            Expr::ListIndex(_, idx_expr) => {
                                let idx_str = self.expr_to_verilog(idx_expr);
                                self.output.push_str(&format!(
                                    "                        if ({} == {}) begin\n",
                                    genvar_name, idx_str
                                ));
                                self.output.push_str(&format!(
                                    "                            {}[{}] <= {};\n",
                                    var_name, genvar_name, lifted_expr
                                ));
                                self.output.push_str("                        end\n");
                            }
                            Expr::Slice { start, end, stride, mask, .. } => {
                                let range_str = match (start, end) {
                                    (Some(s), Some(e)) => {
                                        let s_str = self.expr_to_verilog(s);
                                        let e_str = self.expr_to_verilog(e);
                                        format!("{} <= {} && {} <= {}", genvar_name, s_str, genvar_name, e_str)
                                    }
                                    (Some(s), None) => {
                                        let s_str = self.expr_to_verilog(s);
                                        format!("{} >= {}", genvar_name, s_str)
                                    }
                                    (None, Some(e)) => {
                                        let e_str = self.expr_to_verilog(e);
                                        format!("{} <= {}", genvar_name, e_str)
                                    }
                                    (None, None) => "1".to_string(),
                                };
                                
                                let stride_str = stride.as_ref().map(|s| {
                                    let s_str = self.expr_to_verilog(s);
                                    format!("({} % {}) == 0", genvar_name, s_str)
                                });
                                
                                let mask_str = mask.as_ref().map(|m| {
                                    self.expr_to_verilog(m)
                                });
                                
                                let mut condition = range_str.clone();
                                
                                if let Some(s) = stride_str {
                                    condition = format!("{} && {}", condition, s);
                                }
                                
                                if let Some(m) = mask_str {
                                    condition = format!("{} && ({})", condition, m);
                                }
                                
                                self.output.push_str(&format!(
                                    "                        if ({}) begin\n",
                                    condition
                                ));
                                self.output.push_str(&format!(
                                    "                            {}[{}] <= {};\n",
                                    var_name, genvar_name, lifted_expr
                                ));
                                self.output.push_str("                        end\n");
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    self.output.push_str(&format!(
                        "                        if ({}) begin\n",
                        self.expr_to_verilog(condition)
                    ));
                    self.emit_vector_assignment_from_txn(var_name, statements, program);
                    self.output.push_str("                        end\n");
                }
                _ => {}
            }
        }
    }

    fn emit_ram_assignment_from_txn(
        &mut self,
        var_name: &str,
        body: &[Statement],
        program: &Program,
        _base_address: Option<u32>,
    ) {
        // For RAM template: use address from AXI interface instead of genvar
        // The write happens at specific addresses - we generate per-element conditionals
        // This is less efficient than true dual-port RAM but ensures correctness

        let addr_signal = "cpu_write_addr";  // Standard AXI write address signal

        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if self.extract_assignment_target(lhs).as_deref() == Some(var_name) {
                        let expr_str = self.expr_to_verilog(expr);

                        match lhs {
                            Expr::Identifier(_) | Expr::OwnedRef(_) => {
                                // Full vector assignment - create loop over all addresses
                                self.output.push_str(&format!(
                                    "                // Full buffer write via AXI\n",
                                ));
                            }
                            Expr::ListIndex(_, idx_expr) => {
                                let idx_str = self.expr_to_verilog(idx_expr);
                                self.output.push_str(&format!(
                                    "                if ({} == {}) begin\n",
                                    addr_signal, idx_str
                                ));
                                self.output.push_str(&format!(
                                    "                    {}[{}] <= {};\n",
                                    var_name, idx_str, expr_str
                                ));
                                self.output.push_str("                end\n");
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    self.output.push_str(&format!(
                        "                if ({}) begin\n",
                        self.expr_to_verilog(condition)
                    ));
                    self.emit_ram_assignment_from_txn(var_name, statements, program, None);
                    self.output.push_str("                end\n");
                }
                _ => {}
            }
        }
    }

    fn emit_ram_write_statement(
        &mut self,
        var_name: &str,
        body: &[Statement],
        program: &Program,
    ) {
        // For RAM templates: direct write, no per-element conditionals
        // The transaction condition already gates when writes occur
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if self.extract_assignment_target(lhs).as_deref() == Some(var_name) {
                        let expr_str = self.expr_to_verilog(expr);
                        match lhs {
                            Expr::Identifier(_) | Expr::OwnedRef(_) => {
                                self.output.push_str(&format!(
                                    "                    {} <= {};\n",
                                    var_name, expr_str
                                ));
                            }
                            Expr::ListIndex(_, idx_expr) => {
                                let idx_str = self.expr_to_verilog(idx_expr);
                                self.output.push_str(&format!(
                                    "                    {}[{}] <= {};\n",
                                    var_name, idx_str, expr_str
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Guarded { condition, statements } => {
                    self.output.push_str(&format!(
                        "                    if ({}) begin\n",
                        self.expr_to_verilog(condition)
                    ));
                    self.emit_ram_write_statement(var_name, statements, program);
                    self.output.push_str("                    end\n");
                }
                _ => {}
            }
        }
    }

    fn collect_ram_writes(
        &self,
        var_name: &str,
        txns: &[&Transaction],
        program: &Program,
    ) -> Vec<RamWrite> {
        let mut writes = Vec::new();

        for txn in txns {
            let ce_cond = if let Some(speed) = txn.reactor_speed {
                format!("ce_{}hz && ", speed)
            } else {
                "".to_string()
            };

            let cond = format!(
                "{}{}",
                ce_cond,
                self.expr_to_verilog(&txn.contract.pre_condition)
            );

            self.extract_writes_from_body(var_name, &txn.body, &cond, &mut writes);
        }

        writes
    }

    fn extract_writes_from_body(
        &self,
        var_name: &str,
        body: &[Statement],
        txn_condition: &str,
        writes: &mut Vec<RamWrite>,
    ) {
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if self.extract_assignment_target(lhs).as_deref() == Some(var_name) {
                        let data_expr = self.expr_to_verilog(expr);
                        match lhs {
                            Expr::Identifier(_) | Expr::OwnedRef(_) => {
                                writes.push(RamWrite {
                                    condition: txn_condition.to_string(),
                                    address_expr: "*".to_string(),
                                    data_expr,
                                });
                            }
                            Expr::ListIndex(_, idx_expr) => {
                                let addr_expr = self.expr_to_verilog(idx_expr);
                                writes.push(RamWrite {
                                    condition: txn_condition.to_string(),
                                    address_expr: addr_expr,
                                    data_expr,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Guarded { condition, statements } => {
                    let guard_cond = self.expr_to_verilog(condition);
                    let combined = format!("{} && {}", txn_condition, guard_cond);
                    self.extract_writes_from_body(var_name, statements, &combined, writes);
                }
                _ => {}
            }
        }
    }

    fn statement_to_verilog(&mut self, stmt: &Statement) -> String {
        let mut out = String::new();
        match stmt {
            Statement::Term { .. } | Statement::TermBang { .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for s in &cleanup {
                    out.push_str(&self.statement_to_verilog(s));
                }
                out.push_str("        /* transaction complete */\n");
            }
            Statement::Let { name, expr, address, address_expr, .. } => {
                if let Some(addr) = address {
                    if let Some(addr_expr) = address_expr {
                        out.push_str(&format!(
                            "        // let {} @ 0x{:x} = {};\n",
                            name, addr, self.expr_to_verilog(addr_expr)
                        ));
                    } else {
                        out.push_str(&format!("        // let {} @ 0x{:x}\n", name, addr));
                    }
                }
                if let Some(e) = expr {
                    out.push_str(&format!("        // let {} = {};\n", name, self.expr_to_verilog(e)));
                } else {
                    out.push_str(&format!("        // let {};\n", name));
                }
            }
            Statement::Expression(expr) => {
                out.push_str(&format!("        /* {} */\n", self.expr_to_verilog(expr)));
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    out.push_str(&format!("        // trg! {}: await {}\n", name, self.expr_to_verilog(e)));
                } else {
                    out.push_str(&format!("        // trg! {}: await external event\n", name));
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                out.push_str("        /* #on_exit cleanup registered */\n");
            }
            Statement::Escape(opt_expr) => {
                if let Some(e) = opt_expr {
                    out.push_str(&format!("        // escape {}\n", self.expr_to_verilog(e)));
                } else {
                    out.push_str("        // escape\n");
                }
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    out.push_str(&format!("        {}\n", line));
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                out.push_str(&format!("        /* asm: {} */\n", asm_string));
            }
            Statement::Unification { name, pattern, expr } => {
                out.push_str(&format!("        // unification: {} {} <= {}\n", name, pattern, self.expr_to_verilog(expr)));
            }
            Statement::Assignment { lhs, expr, .. } => {
                if let Expr::Identifier(name) = lhs {
                    out.push_str(&format!("        {} <= {};\n", name, self.expr_to_verilog(expr)));
                } else {
                    out.push_str(&format!("        /* assign */ {} <= {};\n", self.expr_to_verilog(lhs), self.expr_to_verilog(expr)));
                }
            }
            Statement::Guarded { condition, statements } => {
                out.push_str(&format!("        if ({}) begin\n", self.expr_to_verilog(condition)));
                for s in statements {
                    out.push_str(&self.statement_to_verilog(s));
                }
                out.push_str("        end\n");
            }
        }
        out
    }

    fn expr_to_verilog(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Bool(true) => "1'b1".to_string(),
            Expr::Bool(false) => "1'b0".to_string(),
            Expr::Identifier(name) => name.clone(),
            Expr::OwnedRef(name) => name.clone(),
            Expr::PriorState(name) => name.clone(),
            Expr::Add(l, r) => format!(
                "({} + {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Sub(l, r) => format!(
                "({} - {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Mul(l, r) => format!(
                "({} * {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Div(l, r) => format!(
                "({} / {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Eq(l, r) => format!(
                "({} == {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Ne(l, r) => format!(
                "({} != {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Lt(l, r) => format!(
                "({} < {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Le(l, r) => format!(
                "({} <= {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Gt(l, r) => format!(
                "({} > {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Ge(l, r) => format!(
                "({} >= {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::And(l, r) => format!(
                "({} && {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::BitAnd(l, r) => format!(
                "({} & {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::BitOr(l, r) => format!(
                "({} | {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::BitXor(l, r) => format!(
                "({} ^ {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Shl(l, r) => format!(
                "({} << {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Shr(l, r) => format!(
                "({} >> {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Neg(inner) => format!(
                "(-{})",
                self.expr_to_verilog(inner)
            ),
            Expr::Not(inner) => format!(
                "(!{})",
                self.expr_to_verilog(inner)
            ),
            Expr::BitNot(inner) => format!(
                "(~{})",
                self.expr_to_verilog(inner)
            ),
            Expr::Call(name, args) => {
                let args_str = args
                    .iter()
                    .map(|a| self.expr_to_verilog(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
            Expr::PatternMatch { value, variant, .. } => {
                let v_str = self.expr_to_verilog(value);
                if variant == "Ok" {
                    format!("({}_tag == 0)", v_str)
                } else if variant == "Err" {
                    format!("({}_tag == 1)", v_str)
                } else {
                    format!("({}_tag == {})", v_str, variant)
                }
            }
            Expr::Slice {
                value, start, end, stride, mask
            } => {
                if let Some(mask_expr) = mask {
                    // This is a masked assignment, handle it differently
                    return format!("/* Masked assignment for {} */", self.expr_to_verilog(value));
                }
                let v_str = self.expr_to_verilog(value);
                let s_str = start
                    .as_ref()
                    .map(|e| self.expr_to_verilog(e))
                    .unwrap_or("0".to_string());
                let e_str = end
                    .as_ref()
                    .map(|e| self.expr_to_verilog(e))
                    .unwrap_or("0".to_string());
                format!("{}[{}:{}]", v_str, s_str, e_str)
            }
            Expr::ListIndex(list, index) => {
                format!(
                    "{}[{}]",
                    self.expr_to_verilog(list),
                    self.expr_to_verilog(index)
                )
            }
            Expr::Float(f) => format!("{}", f),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Mod(l, r) => format!(
                "({} % {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Or(l, r) => format!(
                "({} || {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::ListLiteral(items) => {
                let items_str = items
                    .iter()
                    .map(|i| self.expr_to_verilog(i))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {}}} ", items_str)
            }
            Expr::Projection { source: list, .. } => {
                format!("$size({})", self.expr_to_verilog(list))
            }
            Expr::FieldAccess(obj, field) => {
                format!(
                    "{}_{}",
                    self.expr_to_verilog(obj),
                    field
                )
            }
            _ => format!("/* Unsupported Expr: {:?} */", expr),
        }
    }

    fn emit_footer(&mut self) {
        self.output.push_str("endmodule\n");
    }

    fn validate_hardware(&self, program: &Program) -> Result<(), String> {
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                if let Some(addr) = decl.address {
                    let addr_str_upper = format!("0x{:08X}", addr);
                    let addr_str_lower = format!("0x{:08x}", addr);
                    let addr_str_hex_upper = format!("0x{:X}", addr);
                    let addr_str_hex_lower = format!("0x{:x}", addr);

                    let mem_cfg = self
                        .hw_config
                        .memory
                        .get(&addr_str_upper)
                        .or_else(|| self.hw_config.memory.get(&addr_str_lower))
                        .or_else(|| self.hw_config.memory.get(&addr_str_hex_upper))
                        .or_else(|| self.hw_config.memory.get(&addr_str_hex_lower));

                    if let Some(mem_cfg) = mem_cfg {
                        // Check size
                        if let Type::Vector(_, dims) = &decl.ty {
                            let total_size: usize = dims.iter().map(|d| match d {
                                crate::ast::Dimension::Anonymous(s) => *s,
                                crate::ast::Dimension::Named(_, s) => *s,
                            }).product();
                            if total_size > mem_cfg.size {
                                return Err(format!(
                                    "Vector '{}' size ({}) exceeds hardware memory size ({}) at address 0x{:x}",
                                    decl.name, total_size, mem_cfg.size, addr
                                ));
                            }
                        }

                        // Check element bits
                        let bits = self.get_bit_width(&decl.ty, decl.bit_range.as_ref());
                        if bits > mem_cfg.element_bits {
                            return Err(format!(
                                "Variable '{}' bit width ({}) exceeds hardware element bits ({}) at address 0x{:x}",
                                decl.name, bits, mem_cfg.element_bits, addr
                            ));
                        }
                    } else if self.get_io_mapping(addr).is_none() {
                        return Err(format!(
                            "Address 0x{:x} used by '{}' is not defined in hardware.toml memory or io",
                            addr, decl.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn generate_testbench(&self, program: &Program) -> String {
        let mut tb = String::new();
        tb.push_str("`timescale 1ns/1ps\n\n");
        tb.push_str(&format!("module {}_tb;\n\n", self.module_name));

        tb.push_str("    // Clock and reset\n");
        tb.push_str("    logic clk = 0;\n");
        tb.push_str("    logic rst_n = 0;\n\n");

        tb.push_str("    // Testbench control\n");
        tb.push_str("    logic [7:0] cpu_control = 0;\n");
        tb.push_str("    logic [7:0] cpu_status;\n");
        tb.push_str("    logic [3:0] cpu_opcode = 0;\n");
        tb.push_str("    logic signed [15:0] cpu_write_data = 0;\n");
        tb.push_str("    logic [17:0] cpu_write_addr = 0;\n");
        tb.push_str("    logic cpu_write_en = 0;\n");
        tb.push_str("    logic cpu_read_en = 0;\n\n");

        tb.push_str("    // Instantiate Unit Under Test\n");
        tb.push_str(&format!("    {} uut (\n", self.module_name));
        tb.push_str("        .clk(clk),\n");
        tb.push_str("        .rst_n(rst_n)\n");
        tb.push_str("    );\n\n");

        tb.push_str("    // Clock generation (100MHz = 10ns period)\n");
        tb.push_str("    always #5 clk = ~clk;\n\n");

        tb.push_str("    // Test sequence\n");
        tb.push_str("    initial begin\n");
        tb.push_str("        $dumpfile(\"waveform.vcd\");\n");
        tb.push_str("        $dumpvars(0, uut);\n\n");
        
        tb.push_str("        // Reset sequence\n");
        tb.push_str("        #0 rst_n = 0;\n");
        tb.push_str("        #10 rst_n = 1;\n");
        tb.push_str("        #5;\n\n");
        
        tb.push_str("        // Test 1: Sync control\n");
        tb.push_str("        cpu_control = 1;\n");
        tb.push_str("        #10;\n");
        tb.push_str("        cpu_control = 0;\n");
        tb.push_str("        #10;\n\n");
        
        tb.push_str("        // Test 2: Load input data\n");
        tb.push_str("        cpu_control = 1;\n");
        tb.push_str("        cpu_write_en = 1;\n");
        tb.push_str("        cpu_write_addr = 0;\n");
        tb.push_str("        cpu_write_data = 16'h1234;\n");
        tb.push_str("        #10;\n");
        tb.push_str("        cpu_write_en = 0;\n");
        tb.push_str("        #10;\n\n");
        
        tb.push_str("        // Test 3: Execute forward pass\n");
        tb.push_str("        cpu_control = 20;\n");
        tb.push_str("        #10;\n");
        tb.push_str("        cpu_control = 0;\n");
        tb.push_str("        #10;\n\n");
        
        tb.push_str("        // Wait and finish\n");
        tb.push_str("        #100;\n");
        tb.push_str("        $display(\"Test completed successfully.\");\n");
        tb.push_str("        $finish;\n");
        tb.push_str("    end\n\n");

        tb.push_str("    // Monitor for debugging\n");
        tb.push_str("    always @(posedge clk) begin\n");
        tb.push_str("        if (uut.control != 0) begin\n");
        tb.push_str("            $display(\"t=%0d: control=%d, status=%d\", $time, uut.control, uut.status);\n");
        tb.push_str("        end\n");
        tb.push_str("    end\n\n");

        tb.push_str("endmodule\n");

        tb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verilog_generates_module() {
        let hw_config = HardwareConfig {
            project: ProjectConfig {
                name: "test".to_string(),
                version: "1.0".to_string(),
            },
            target: TargetConfig {
                fpga: "test".to_string(),
                clock_hz: 100_000_000,
                platform: None,
                synthesis: None,
            },
            interface: InterfaceConfig {
                name: "none".to_string(),
                address_width: None,
                data_width: None,
                controller: None,
                situs: None,
            },
            memory: HashMap::new(),
            io: None,
        };
        let mut backend = VerilogGenerator::new("test_module", hw_config);
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
        let output = backend.generate(&program);
        assert!(output.contains("module"));
    }
}
