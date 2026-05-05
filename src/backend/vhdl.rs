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

pub struct VhdlGenerator {
    spec: Option<crate::target_spec::TargetSpec>,
    entity_name: String,
    clock_freq: u32,
    hw_config: HardwareConfig,
    linkage: Option<LinkageConfig>,
    signal_counter: usize,
    process_counter: usize,
}

impl VhdlGenerator {
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
        let mut output = String::new();

        if let Err(e) = self.validate_hardware(program) {
            panic!("Hardware validation failed: {}", e);
        }

        self.emit_header(&mut output, program);
        self.emit_architecture(&mut output, program);

        output
    }

    fn validate_hardware(&self, _program: &Program) -> Result<(), String> {
        Ok(())
    }

    fn emit_header(&mut self, output: &mut String, program: &Program) {
        output.push_str("library IEEE;\n");
        output.push_str("use IEEE.std_logic_1164.all;\n");
        output.push_str("use IEEE.numeric_std.all;\n\n");

        output.push_str(&format!("entity {} is\n", self.entity_name));
        output.push_str("    port (\n");
        output.push_str("        clk : in std_logic;\n");
        output.push_str("        rst : in std_logic;\n");

        let state_count = program.items.iter().filter(|item| matches!(item, TopLevel::StateDecl(_))).count();
        if state_count > 0 {
            output.push_str("        -- State outputs\n");
            for item in &program.items {
                if let TopLevel::StateDecl(state) = item {
                    let vhdl_type = self.brief_type_to_vhdl(&state.ty);
                    output.push_str(&format!("        {} : out {};\n", state.name, vhdl_type));
                }
            }
        }

        output.push_str("        rst_out : out std_logic\n");
        output.push_str("    );\n");
        output.push_str(&format!("end entity {};\n\n", self.entity_name));
    }

    fn emit_architecture(&mut self, output: &mut String, program: &Program) {
        output.push_str(&format!("architecture rtl of {} is\n", self.entity_name));
        output.push_str("begin\n\n");

        for item in &program.items {
            if let TopLevel::StateDecl(state) = item {
                let vhdl_type = self.brief_type_to_vhdl(&state.ty);
                let init = self.get_default_value(&state.ty);
                output.push_str(&format!("    signal {} : {} := {};\n", state.name, vhdl_type, init));
            }
        }

        output.push_str("\n");

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.emit_transaction(output, txn);
            }
        }

        output.push_str("\n    rst_out <= rst;\n");
        output.push_str("\nend architecture rtl;\n");
    }

    fn emit_transaction(&mut self, output: &mut String, txn: &Transaction) {
        let proc_name = format!("proc_{}", txn.name);
        self.process_counter += 1;

        output.push_str(&format!("    {}: process(clk, rst) is\n", proc_name));
        output.push_str("    begin\n");
        output.push_str("        if rst = '1' then\n");

        for item in &txn.body {
            if let Statement::Assignment { lhs, expr, .. } = item {
                output.push_str(&format!("            {} <= {};\n", self.expr_to_string(lhs), self.expr_to_string(expr)));
            }
        }

        output.push_str("        elsif rising_edge(clk) then\n");

        if txn.is_reactive {
            let pre = self.expr_to_string(&txn.contract.pre_condition);
            if pre != "true" && pre != "1" {
                output.push_str(&format!("            if {} then\n", pre));
            }

            for item in &txn.body {
                if let Statement::Assignment { lhs, expr, .. } = item {
                    output.push_str(&format!("                {} <= {};\n", self.expr_to_string(lhs), self.expr_to_string(expr)));
                }
            }

            if pre != "true" && pre != "1" {
                output.push_str("            end if;\n");
            }
        } else {
            for item in &txn.body {
                if let Statement::Assignment { lhs, expr, .. } = item {
                    output.push_str(&format!("            {} <= {};\n", self.expr_to_string(lhs), self.expr_to_string(expr)));
                }
            }
        }

        output.push_str("        end if;\n");
        output.push_str(&format!("    end process {};\n\n", proc_name));
    }

    fn brief_type_to_vhdl(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "std_logic".to_string(),
            Type::UInt => "std_logic_vector(31 downto 0)".to_string(),
            Type::Int => "signed(31 downto 0)".to_string(),
            Type::Float => "real".to_string(),
            Type::String => "string".to_string(),
            Type::Vector(inner, n) => {
                let inner_vhdl = self.brief_type_to_vhdl(inner);
                format!("array(0 to {}) of {}", n - 1, inner_vhdl)
            }
            Type::Option(inner) => self.brief_type_to_vhdl(inner),
            _ => "std_logic_vector(31 downto 0)".to_string(),
        }
    }

    fn get_default_value(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "'0'".to_string(),
            Type::UInt | Type::Int => "\"(others => '0')\"".to_string(),
            Type::Float => "0.0".to_string(),
            Type::String => "\"\"".to_string(),
            _ => "\"(others => '0')\"".to_string(),
        }
    }

    fn expr_to_string(&self, expr: &Expr) -> String {
        match expr {
            Expr::Bool(b) => if *b { "'1'" } else { "'0'" }.to_string(),
            Expr::Integer(i) => i.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Identifier(name) => name.clone(),
            Expr::Not(e) => format!("not {}", self.expr_to_string(e)),
            Expr::Neg(e) => format!("-{}", self.expr_to_string(e)),
            Expr::Add(lhs, rhs) => format!("({} + {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Sub(lhs, rhs) => format!("({} - {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Mul(lhs, rhs) => format!("({} * {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Div(lhs, rhs) => format!("({} / {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Eq(lhs, rhs) => format!("({} = {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Ne(lhs, rhs) => format!("({} /= {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Lt(lhs, rhs) => format!("({} < {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Le(lhs, rhs) => format!("({} <= {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Gt(lhs, rhs) => format!("({} > {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Ge(lhs, rhs) => format!("({} >= {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::And(lhs, rhs) => format!("({} and {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::Or(lhs, rhs) => format!("({} or {})", self.expr_to_string(lhs), self.expr_to_string(rhs)),
            Expr::PriorState(name) => format!("{}", name),
            _ => "'0'".to_string(),
        }
    }
}