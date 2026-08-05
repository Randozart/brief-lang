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

use crate::ast::HardwareConfig;
use std::collections::HashMap;

pub struct TclGenerator {
    project_name: String,
    part_number: String,
    board_part: Option<String>,
    top_module: String,
    sv_files: Vec<String>,
    interface_name: String,
    interface_controller: Option<String>,
    interface_situs: Option<String>,
    synthesis_mode: String,
    max_jobs: u32,
}

impl TclGenerator {
    pub fn new(config: &HardwareConfig, sv_files: Vec<String>) -> Self {
        let part_number = Self::resolve_part_number(&config.target.fpga);
        let board_part = Self::resolve_board_part(&config.target.platform);
        let (synthesis_mode, max_jobs) = Self::resolve_synthesis_config(&config.target);

        TclGenerator {
            project_name: config.project.name.clone(),
            part_number,
            board_part,
            top_module: config.project.name.clone(),
            sv_files,
            interface_name: config.interface.name.clone(),
            interface_controller: config.interface.controller.clone(),
            interface_situs: config.interface.situs.clone(),
            synthesis_mode,
            max_jobs,
        }
    }

    fn resolve_part_number(fpga: &str) -> String {
        let parts: HashMap<&str, &str> = HashMap::from([
            ("xczu4ev", "xczu4ev-sfvc784-2-e"),
            ("xczu6eg", "xczu6eg-sfvc784-2-e"),
            ("xczu9eg", "xczu9eg-sfvc784-2-e"),
            ("xc7a35t", "xc7a35tfgg484-2"),
            ("xc7a100t", "xc7a100tfgg484-2"),
            ("xc7k325t", "xc7k325t-fbg900-2"),
            ("xc7z010", "xc7z010clg400-2"),
            ("xc7z020", "xc7z020clg400-2"),
        ]);
        parts.get(fpga).map(|s| s.to_string()).unwrap_or_else(|| fpga.to_string())
    }

    fn resolve_board_part(platform: &Option<String>) -> Option<String> {
        let board_parts: HashMap<&str, &str> = HashMap::from([
            ("kv260", "xilinx.com:kv260_som:1.1"),
            ("kria", "xilinx.com:kv260_som:1.1"),
            ("zcu102", "xilinx.com:zcu102:1.1"),
            ("zcu104", "xilinx.com:zcu104:1.0"),
            ("zcu106", "xilinx.com:zcu106:1.1"),
            ("zedboard", "em.avnet.com:zedboard:1.0"),
            ("pynqz2", "xilinx.com:pynq-z2:1.0"),
        ]);
        platform.as_ref().and_then(|p| board_parts.get(p.as_str()).map(|s| s.to_string()))
    }

    fn resolve_synthesis_config(target: &crate::ast::TargetConfig) -> (String, u32) {
        let synthesis = target.synthesis.as_ref();
        
        let mode = synthesis
            .and_then(|s| {
                if s.mode.is_empty() { None } else { Some(s.mode.clone()) }
            })
            .unwrap_or_else(|| "global".to_string());
        
        let max_jobs = synthesis
            .and_then(|s| {
                if s.max_jobs == 0 { None } else { Some(s.max_jobs) }
            })
            .unwrap_or_else(|| Self::auto_detect_job_count());

        (mode, max_jobs)
    }

    fn auto_detect_job_count() -> u32 {
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("sh")
                .arg("-c")
                .arg("cat /proc/meminfo | grep MemTotal | awk '{print $2}'")
                .output()
                .ok()
                .and_then(|output| {
                    let kb: u64 = String::from_UTF8_lossy(&output.stdout)
                        .trim().parse().ok()?;
                    let gb = kb / 1024 / 1024;
                    if gb < 16 { Some(1) } else if gb < 32 { Some(4) } else { Some(8) }
                })
                .unwrap_or(1)
        }
        #[cfg(not(target_os = "linux"))]
        { 1 }
    }

    fn get_axi_controller_port(&self) -> (&'static str, &'static str) {
        match self.interface_controller.as_deref().unwrap_or("LPD_MASTER") {
            "FPD_MASTER" => ("M_AXI_FPM", "FPD"),
            _ => ("M_AXI_HPM0_LPD", "LPD"),
        }
    }

    pub fn generate(&self) -> String {
        let files_list = self.sv_files.join(" ");
        let block_design = self.generate_block_design();
        let board_part_line = self.board_part.as_ref()
            .map(|bp| format!("set_property board_part {} [current_project]\n", bp))
            .unwrap_or_default();
        let synth_checkpoint = if self.synthesis_mode == "global" {
            "set_property synth_checkpoint_mode None [get_files system.bd]\n".to_string()
        } else {
            String::new()
        };
        let board_part_display = self.board_part.as_deref().unwrap_or("none");
        let tm = &self.top_module;
        let pn = &self.project_name;

        let mut tcl = String::new();

        tcl.push_str("# ============================================================\n");
        tcl.push_str("# AUTOMATED CHANCERY DECREE (VIVADO BUILD SCRIPT)\n");
        tcl.push_str("# Generated by briv-compiler v0.2.0\n");
        tcl.push_str(&format!("# Project: {}\n", pn));
        tcl.push_str(&format!("# Part: {}\n", self.part_number));
        tcl.push_str(&format!("# Board: {}\n", board_part_display));
        tcl.push_str(&format!("# Interface: {}\n", self.interface_name));
        tcl.push_str(&format!("# Synthesis Mode: {}\n", self.synthesis_mode));
        tcl.push_str(&format!("# Max Jobs: {}\n", self.max_jobs));
        tcl.push_str("# ============================================================\n\n");

        tcl.push_str(&format!("set project_name \"{}\"\n", pn));
        tcl.push_str(&format!("set part_number \"{}\"\n", self.part_number));
        tcl.push_str(&format!("set top_module \"{}\"\n\n", tm));

        tcl.push_str("puts \"=== Briv Compiler Automated Build ===\"\n");
        tcl.push_str("puts \"Part: $part_number\"\n");
        tcl.push_str(&format!("puts \"Board: {}\"\n", board_part_display));
        tcl.push_str(&format!("puts \"Synthesis: {} (jobs: {})\n", self.synthesis_mode, self.max_jobs));
        tcl.push_str("\n");

        tcl.push_str("file delete -force $project_name\n");
        tcl.push_str("file delete -force ./ip_repo\n");
        tcl.push_str("file delete -force ./ip_packager_proj\n\n");

        tcl.push_str("# STEP 1: PACKAGE SYSTEMVERILOG AS IP\n");
        tcl.push_str("puts \"Packaging SystemVerilog as IP...\"\n");
        tcl.push_str("create_project -force ip_packager_proj ./ip_packager_proj -part $part_number\n");
        tcl.push_str(&format!("add_files [list {}]\n", files_list));
        tcl.push_str("set_property top $top_module [current_fileset]\n");
        tcl.push_str("update_compile_order -fileset sources_1\n\n");
        tcl.push_str("ipx::package_project -root_dir ./ip_repo -vendor user.org -library user -taxonomy /UserIP -import_files\n");
        tcl.push_str("set core [ipx::current_core]\n");
        tcl.push_str("ipx::save_core $core\n");
        tcl.push_str("close_project\n\n");

        tcl.push_str("# STEP 2: CREATE MAIN PROJECT\n");
        tcl.push_str("puts \"Creating main project...\"\n");
        tcl.push_str("create_project -force $project_name . -part $part_number\n");
        tcl.push_str(&board_part_line);
        tcl.push_str("set_property ip_repo_paths \"[pwd]/ip_repo\" [current_project]\n");
        tcl.push_str("update_ip_catalog\n\n");

        tcl.push_str("# STEP 3: BLOCK DESIGN SETUP\n");
        tcl.push_str("puts \"Creating block design...\"\n");
        tcl.push_str("create_bd_design \"system\"\n\n");
        tcl.push_str(&block_design);
        tcl.push_str(&synth_checkpoint);
        tcl.push_str("puts \"Block design complete.\"\n\n");

        tcl.push_str("# STEP 4: BUILD BITSTREAM\n");
        tcl.push_str("puts \"Preparing Block Design targets...\"\n");
        tcl.push_str("generate_target all [get_files system.bd]\n");
        tcl.push_str("export_ip_user_files -of_objects [get_files system.bd] -no_script -force\n\n");
        tcl.push_str("puts \"Running synthesis and implementation...\"\n");
        tcl.push_str(&format!("puts \"Mode: {}, Jobs: {}\"\n", self.synthesis_mode, self.max_jobs));
        tcl.push_str(&format!("launch_runs impl_1 -to_step write_bitstream -jobs {}\n", self.max_jobs));
        tcl.push_str("wait_on_run impl_1\n\n");

        tcl.push_str("# STEP 5: VERIFY AND REPORT\n");
        tcl.push_str("set progress [get_property PROGRESS [get_runs impl_1]]\n");
        tcl.push_str("if { $progress != \"100%\" } {\n");
        tcl.push_str("    puts \"ERROR: Build failed. Progress: $progress\"\n");
        tcl.push_str("    puts \"Check logs in the .runs directory.\"\n");
        tcl.push_str("    exit 1\n");
        tcl.push_str("}\n\n");
        tcl.push_str("puts \"=== SUCCESS ===\"\n");
        tcl.push_str("puts \"Bitstream: [pwd]/$project_name.runs/impl_1/system_wrapper.bit\"\n");
        tcl.push_str("exit 0\n");

        tcl
    }

    fn generate_block_design(&self) -> String {
        if self.interface_name == "axi4-lite" || self.interface_name == "axi4-full" {
            self.generate_axi_block_design()
        } else {
            self.generate_parallel_block_design()
        }
    }

    fn generate_axi_block_design(&self) -> String {
        let (axi_port, axi_domain) = self.get_axi_controller_port();
        let situs_addr = self.interface_situs.as_deref().unwrap_or("0x80000000");
        let tm = &self.top_module;
        let board_preset = if self.board_part.is_some() { "1" } else { "0" };

        let mut tcl = String::new();
        tcl.push_str("# Add processor\n");
        tcl.push_str("create_bd_cell -type ip -vlnv xilinx.com:ip:zynq_ultra_ps_e zynq_ps\n");
        tcl.push_str(&format!("apply_bd_automation -rule xilinx.com:bd_rule:zynq_ultra_ps_e -config {{ {{ apply_board_preset \"{}\" }} }} [get_bd_cells zynq_ps]\n", board_preset));
        
        tcl.push_str("# Add Generated IP\n");
        tcl.push_str(&format!("create_bd_cell -type ip -vlnv user.org:user:{}:1.0 {}_0\n", tm, tm));
        
        tcl.push_str("# Connect AXI interface\n");
        tcl.push_str(&format!("connect_bd_net [get_bd_pins zynq_ps/pl_clk0] [get_bd_pins {}_0/s_axi_aclk]\n", tm));
        tcl.push_str(&format!("connect_bd_net [get_bd_pins zynq_ps/pl_resetn0] [get_bd_pins {}_0/s_axi_aresetn]\n", tm));
        
        tcl.push_str("# Automate AXI Connection\n");
        tcl.push_str(&format!("apply_bd_automation -rule xilinx.com:bd_rule:axi4 -config {{ {{ Master /zynq_ps/{port} Clk_master {{ Auto }} Clk_slave {{ Auto }} Clk_xbar {{ Auto }} Slave /{tm}_0/s_axi }} }} [get_bd_intf_pins {tm}_0/s_axi]\n", port = axi_port, tm = tm));
        
        tcl.push_str("# Map address space\n");
        tcl.push_str(&format!("set target_seg [get_bd_addr_segs -of_objects [get_bd_cells zynq_ps] -filter {{ NAME =~ \"*{tm}_0*\" }}]\n"));
        tcl.push_str(&format!("set_property offset {} ", situs_addr));
        tcl.push_str("$target_seg\n");
        tcl.push_str("set_property range 64K $target_seg\n");
        
        tcl.push_str("# DECREE OF EXCLUSION: Disable unused ports\n");
        tcl.push_str("puts \"Brick up unused ports...\"\n");
        tcl.push_str("set_property config { { EXCLUDE { FPD_S_AXI_INTF } } } [get_bd_cells zynq_ps]\n");
        tcl.push_str("set_property config { { EXCLUDE { LPD_S_AXI_INTF } } } [get_bd_cells zynq_ps]\n");
        tcl.push_str("puts \"Unused ports bricked up.\"\n");
        
        tcl.push_str("# Create Wrapper\n");
        tcl.push_str("set wrapper_path [make_wrapper -files [get_files system.bd] -top]\n");
        tcl.push_str("add_files -norecurse $wrapper_path\n");
        tcl.push_str("set_property top system_wrapper [current_fileset]\n");
        tcl.push_str("update_compile_order -fileset sources_1\n");

        tcl
    }

    fn generate_parallel_block_design(&self) -> String {
        let tm = &self.top_module;
        let has_board = self.board_part.is_some();

        let mut tcl = String::new();
        
        if has_board {
            tcl.push_str("# Add clock wizard with board preset\n");
            tcl.push_str("create_bd_cell -type ip -vlnv xilinx.com:ip:clk_wiz clk_wiz_0\n");
            tcl.push_str("apply_bd_automation -rule xilinx.com:bd_rule:clk_wiz -config { { CLK_IN1 \"sys_clk\" } { CLK_BOARD_UI { { sys_clk_clk_wiz_0 } } } } } [get_bd_cells clk_wiz_0]\n");
        } else {
            tcl.push_str("# Add clock wizard\n");
            tcl.push_str("create_bd_cell -type ip -vlnv xilinx.com:ip:clk_wiz clk_wiz_0\n");
            tcl.push_str("apply_bd_automation -rule xilinx.com:bd_rule:clk_wiz -config { { CLK_IN1 \"sys_clk\" } } [get_bd_cells clk_wiz_0]\n");
        }

        tcl.push_str("# Add Generated IP\n");
        tcl.push_str(&format!("create_bd_cell -type ip -vlnv user.org:user:{}:1.0 {}_0\n", tm, tm));
        
        tcl.push_str("# Connect Clock and Reset\n");
        tcl.push_str(&format!("connect_bd_net [get_bd_pins clk_wiz_0/clk_out1] [get_bd_pins {}_0/clk]\n", tm));
        tcl.push_str(&format!("connect_bd_net [get_bd_pins clk_wiz_0/locked] [get_bd_pins {}_0/rst_n]\n", tm));
        
        tcl.push_str("# Create Wrapper\n");
        tcl.push_str("set wrapper_path [make_wrapper -files [get_files system.bd] -top]\n");
        tcl.push_str("add_files -norecurse $wrapper_path\n");
        tcl.push_str("set_property top system_wrapper [current_fileset]\n");
        tcl.push_str("update_compile_order -fileset sources_1\n");

        tcl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{HardwareConfig, InterfaceConfig, ProjectConfig, TargetConfig, SynthesisConfig};
    use std::collections::HashMap;

    fn test_config_with_board() -> HardwareConfig {
        HardwareConfig {
            project: ProjectConfig { name: "test_proj".to_string(), version: "1.0.0".to_string() },
            target: TargetConfig {
                fpga: "xczu4ev".to_string(),
                clock_hz: 100_000_000,
                platform: Some("kv260".to_string()),
                synthesis: Some(SynthesisConfig { mode: "global".to_string(), max_jobs: 0 }),
            },
            interface: InterfaceConfig {
                name: "axi4-lite".to_string(),
                address_width: Some(16),
                data_width: Some(32),
                controller: Some("LPD_MASTER".to_string()),
                situs: Some("0x80000000".to_string()),
            },
            memory: HashMap::new(),
            io: None,
        }
    }

    fn test_config_minimal() -> HardwareConfig {
        HardwareConfig {
            project: ProjectConfig { name: "minimal".to_string(), version: "1.0.0".to_string() },
            target: TargetConfig {
                fpga: "xc7a35t".to_string(),
                clock_hz: 50_000_000,
                platform: None,
                synthesis: None,
            },
            interface: InterfaceConfig {
                name: "parallel".to_string(),
                address_width: None,
                data_width: None,
                controller: None,
                situs: None,
            },
            memory: HashMap::new(),
            io: None,
        }
    }

    #[test]
    fn test_tcl_generation_with_board() {
        let config = test_config_with_board();
        let tcl_gen = TclGenerator::new(&config, vec!["test_proj.sv".to_string()]);
        let tcl = tcl_gen.generate();
        
        assert!(tcl.contains("create_project"));
        assert!(tcl.contains("set part_number \"xczu4ev-sfvc784-2-e\""));
        assert!(tcl.contains("board_part xilinx.com:kv260_som:1.1"));
        assert!(tcl.contains("launch_runs impl_1 -to_step write_bitstream -jobs"));
        assert!(tcl.contains("DECREE OF EXCLUSION"));
        assert!(tcl.contains("0x80000000"));
    }

    #[test]
    fn test_tcl_generation_minimal() {
        let config = test_config_minimal();
        let tcl_gen = TclGenerator::new(&config, vec!["minimal.sv".to_string()]);
        let tcl = tcl_gen.generate();
        
        assert!(tcl.contains("create_project"));
        assert!(tcl.contains("set part_number \"xc7a35tfgg484-2\""));
        assert!(!tcl.contains("board_part"));
        assert!(tcl.contains("create_bd_design"));
    }
}