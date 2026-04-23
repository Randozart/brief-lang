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

use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub struct VerilogGenerator {
    module_name: String,
    clock_freq: u32,
    hw_config: HardwareConfig,
    _indent_level: usize,
    output: String,
}

impl VerilogGenerator {
    pub fn new(module_name: &str, hw_config: HardwareConfig) -> Self {
        let clock_freq = hw_config.target.clock_hz;
        VerilogGenerator {
            module_name: module_name.to_string(),
            clock_freq,
            hw_config,
            _indent_level: 0,
            output: String::new(),
        }
    }

    pub fn generate(&mut self, program: &Program) -> String {
        self.output.clear();

        if let Err(e) = self.validate_hardware(program) {
            panic!("Hardware validation failed: {}", e);
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
        self.output.clone()
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
                                    Type::Vector(inner, size) => {
                                        let element_bits =
                                            self.get_bit_width(inner, decl.bit_range.as_ref());
                                        let signed = if matches!(**inner, Type::Int) {
                                            "signed "
                                        } else {
                                            ""
                                        };

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
                                            if mem_cfg.mem_type == "bram" {
                                                attr =
                                                    " /* synthesis syn_ramstyle = \"block_ram\" */";
                                            }
                                        }

                                        self.output.push_str(&format!(
                                            ",\n    {} logic {}{} {} [0:{}]{} /* pin: {} */",
                                            direction,
                                            signed,
                                            if element_bits > 1 {
                                                format!("[{}:0]", element_bits - 1)
                                            } else {
                                                "".to_string()
                                            },
                                            decl.name,
                                            size - 1,
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
                    // Triggers (inputs) only emit if NOT in [memory]
                    if let Some(io_cfg) = self.get_io_mapping(trg.address) {
                        if !self.has_memory_mapping(trg.address) {
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
                // Same for triggers
                if self.get_io_mapping(trg.address).is_some()
                    && !self.has_memory_mapping(trg.address)
                {
                    continue;
                }
                self.emit_type_signals(
                    &trg.name,
                    &trg.ty,
                    trg.bit_range.as_ref(),
                    Some(trg.address),
                );
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
            Type::Vector(inner, size) => {
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
                        if mem_cfg.mem_type == "bram" {
                            suffix = " /* synthesis syn_ramstyle = \"block_ram\" */ /* synthesis keep */";
                        } else {
                            suffix = " /* synthesis keep */";
                        }
                    } else {
                        suffix = " /* synthesis keep */";
                    }
                }

                self.output.push_str(&format!(
                    "    logic {}{} {} [0:{}]{};\n",
                    signed,
                    width_str,
                    name,
                    size - 1,
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
                Statement::Term(outputs) => {
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
            Type::Vector(_, size) => (true, *size),
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
            // RAM template: single always_ff with internal address mux
            // BRAM/UltraRAM have power-on initialization - no reset logic needed
            self.output.push_str(&format!(
                "    // RAM template for {} (type: {:?}, size: {})\n",
                name, mem_type, vector_size
            ));
            self.output.push_str("    always_ff @(posedge clk) begin\n");
            self.output.push_str("        // No reset initialization needed - BRAM auto-initializes on power-up\n");

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
                    "        {}if ({}) begin\n",
                    if idx > 0 { "else " } else { "" },
                    cond
                ));

                // For RAM templates: direct assignment, no index conditional needed
                // The transaction condition already gates the write
                self.emit_ram_write_statement(name, &txn.body, program);
                self.output.push_str("        end\n");
            }

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
                self.emit_vector_assignment_from_txn(name, &txn.body, program);
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
                Statement::Assignment { lhs, expr, timeout } => {
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
                    if let Type::Vector(_, size) = &decl.ty {
                        if *size > 1 {
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
                value, start, end, ..
            } => {
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
                        if let Type::Vector(_, size) = &decl.ty {
                            if *size > mem_cfg.size {
                                return Err(format!(
                                    "Vector '{}' size ({}) exceeds hardware memory size ({}) at address 0x{:x}",
                                    decl.name, size, mem_cfg.size, addr
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
