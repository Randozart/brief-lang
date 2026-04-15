use crate::ast::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
pub struct HardwareConfig {
    pub target: TargetConfig,
    pub io: HashMap<String, IoPinConfig>,
    pub bus: Option<BusConfig>,
}

#[derive(Debug, Deserialize)]
pub struct TargetConfig {
    pub name: String,
    pub clock_hz: u32,
}

#[derive(Debug, Deserialize)]
pub struct IoPinConfig {
    pub pin: String,
    pub mode: Option<String>,
    pub standard: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BusConfig {
    #[serde(rename = "type")]
    pub bus_type: String,
    pub burst_support: bool,
}

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
                        let addr_str_long = format!("0x{:08x}", addr);
                        let addr_str_short = format!("0x{:x}", addr);

                        let io_cfg = self
                            .hw_config
                            .io
                            .get(&addr_str_long)
                            .or_else(|| self.hw_config.io.get(&addr_str_short));

                        if let Some(io_cfg) = io_cfg {
                            let width = self.get_bit_width(&decl.ty, decl.bit_range.as_ref());
                            let direction = "output";
                            self.output.push_str(&format!(
                                ",\n    {} logic {} {} // pin: {}",
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
                TopLevel::Trigger(trg) => {
                    let addr_str_long = format!("0x{:08x}", trg.address);
                    let addr_str_short = format!("0x{:x}", trg.address);

                    let io_cfg = self
                        .hw_config
                        .io
                        .get(&addr_str_long)
                        .or_else(|| self.hw_config.io.get(&addr_str_short));

                    if let Some(io_cfg) = io_cfg {
                        let width = self.get_bit_width(&trg.ty, trg.bit_range.as_ref());
                        let direction = "input";
                        self.output.push_str(&format!(
                            ",\n    {} logic {} {} // pin: {}",
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
                _ => {}
            }
        }

        self.output.push_str("\n);\n\n");
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
                // Skip if it's a pin (already in header)
                if let Some(addr) = decl.address {
                    let addr_str_long = format!("0x{:08x}", addr);
                    let addr_str_short = format!("0x{:x}", addr);
                    if self.hw_config.io.contains_key(&addr_str_long)
                        || self.hw_config.io.contains_key(&addr_str_short)
                    {
                        continue;
                    }
                }

                self.emit_type_signals(&decl.name, &decl.ty, decl.bit_range.as_ref());
            }
        }
        self.output.push_str("\n");
    }

    fn emit_type_signals(&mut self, name: &str, ty: &Type, range: Option<&BitRange>) {
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
                    self.emit_type_signals(&format!("{}{}", name, suffix), t, range);
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
                self.output.push_str(&format!(
                    "    logic {}{} {} [0:{}];\n",
                    signed,
                    width_str,
                    name,
                    size - 1
                ));
            }
            Type::Constrained(inner, r) => {
                self.emit_type_signals(name, inner, Some(r));
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

        if is_vector {
            self.output.push_str("    genvar i;\n");
            self.output.push_str(&format!(
                "    generate\n        for (i = 0; i < {}; i = i + 1) begin : {}_logic\n",
                vector_size, name
            ));
            self.output
                .push_str("            always_ff @(posedge clk) begin\n");
            self.output.push_str("                if (!rst_n) begin\n");

            if let Some(expr) = init_expr {
                self.output.push_str(&format!(
                    "                    {}[i] <= {};\n",
                    name,
                    self.expr_to_verilog(expr)
                ));
            } else {
                self.output
                    .push_str(&format!("                    {}[i] <= 0;\n", name));
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

    fn emit_var_assignment_from_txn(
        &mut self,
        var_name: &str,
        body: &[Statement],
        program: &Program,
    ) {
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, timeout } => {
                    if self.extract_root_var(lhs).as_deref() == Some(var_name) {
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
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    if self.extract_root_var(lhs).as_deref() == Some(var_name) {
                        let expr_str = self.expr_to_verilog(expr);
                        let lifted_expr = expr_str.replace(var_name, &format!("{}[i]", var_name));

                        match lhs {
                            Expr::Identifier(_) | Expr::OwnedRef(_) => {
                                self.output.push_str(&format!(
                                    "                        {}[i] <= {};\n",
                                    var_name, lifted_expr
                                ));
                            }
                            Expr::ListIndex(_, idx_expr) => {
                                let idx_str = self.expr_to_verilog(idx_expr);
                                self.output.push_str(&format!(
                                    "                        if (i == {}) begin\n",
                                    idx_str
                                ));
                                self.output.push_str(&format!(
                                    "                            {}[i] <= {};\n",
                                    var_name, lifted_expr
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
            Expr::Or(l, r) => format!(
                "({} || {})",
                self.expr_to_verilog(l),
                self.expr_to_verilog(r)
            ),
            Expr::Not(e) => format!("!{}", self.expr_to_verilog(e)),
            Expr::Neg(e) => format!("-{}", self.expr_to_verilog(e)),
            Expr::BitNot(e) => format!("~{}", self.expr_to_verilog(e)),
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

    pub fn generate_testbench(&self, program: &Program) -> String {
        let mut tb = String::new();
        tb.push_str("`timescale 1ns/1ps\n\n");
        tb.push_str(&format!("module {}_tb;\n\n", self.module_name));

        tb.push_str("    logic clk;\n");
        tb.push_str("    logic rst_n;\n\n");

        // Declare signals for ports
        let mut ports = Vec::new();
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if let Some(addr) = decl.address {
                        let addr_str_long = format!("0x{:08x}", addr);
                        let addr_str_short = format!("0x{:x}", addr);
                        let io_cfg = self
                            .hw_config
                            .io
                            .get(&addr_str_long)
                            .or_else(|| self.hw_config.io.get(&addr_str_short));

                        if io_cfg.is_some() {
                            let width = self.get_bit_width(&decl.ty, decl.bit_range.as_ref());
                            let width_str = if width > 1 {
                                format!("[{}:0] ", width - 1)
                            } else {
                                "".to_string()
                            };
                            tb.push_str(&format!("    logic {}{};\n", width_str, decl.name));
                            ports.push(decl.name.clone());
                        }
                    }
                }
                TopLevel::Trigger(trg) => {
                    let addr_str_long = format!("0x{:08x}", trg.address);
                    let addr_str_short = format!("0x{:x}", trg.address);
                    let io_cfg = self
                        .hw_config
                        .io
                        .get(&addr_str_long)
                        .or_else(|| self.hw_config.io.get(&addr_str_short));

                    if io_cfg.is_some() {
                        let width = self.get_bit_width(&trg.ty, trg.bit_range.as_ref());
                        let width_str = if width > 1 {
                            format!("[{}:0] ", width - 1)
                        } else {
                            "".to_string()
                        };
                        tb.push_str(&format!("    logic {}{};\n", width_str, trg.name));
                        ports.push(trg.name.clone());
                    }
                }
                _ => {}
            }
        }

        tb.push_str("\n    // Instantiate Unit Under Test\n");
        tb.push_str(&format!("    {} uut (\n", self.module_name));
        tb.push_str("        .clk(clk),\n");
        tb.push_str("        .rst_n(rst_n)");
        for port in ports {
            tb.push_str(&format!(",\n        .{}({})", port, port));
        }
        tb.push_str("\n    );\n\n");

        tb.push_str("    // Clock generation (100MHz)\n");
        tb.push_str("    initial begin\n");
        tb.push_str("        clk = 0;\n");
        tb.push_str("        forever #5 clk = ~clk;\n");
        tb.push_str("    end\n\n");

        tb.push_str("    // Test Stimulus\n");
        tb.push_str("    initial begin\n");
        tb.push_str("        $dumpfile(\"waveform.vcd\");\n");
        tb.push_str("        $dumpvars(0, uut);\n\n");
        tb.push_str("        rst_n = 0;\n");
        tb.push_str("        #20 rst_n = 1;\n\n");
        tb.push_str("        // Let it run for 1000ns\n");
        tb.push_str("        #1000;\n");
        tb.push_str("        $display(\"Simulation finished.\");\n");
        tb.push_str("        $finish;\n");
        tb.push_str("    end\n\n");

        tb.push_str("endmodule\n");
        tb
    }
}
