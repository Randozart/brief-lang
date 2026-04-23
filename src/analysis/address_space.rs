use crate::ast::{HardwareConfig, MemoryMapping, TopLevel, StateDecl};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum AddressSpace {
    Ddr4,           // 0x00000000 - 0xFFFFFFFF: CPU accessible main memory
    Mmio(u64),      // MMIO range, CPU accessible via bus
    FpgaInternal,   // 0x40A80000+: FPGA internal BRAM/URAM, NOT CPU accessible
    Unknown,
}

pub struct AddressSpaceAnalyzer {
    config: Option<&'static HardwareConfig>,
    address_spaces: HashMap<String, AddressSpace>,
}

impl AddressSpaceAnalyzer {
    pub fn new(config: Option<&'static HardwareConfig>) -> Self {
        let mut analyzer = AddressSpaceAnalyzer {
            config,
            address_spaces: HashMap::new(),
        };
        analyzer.build_address_map();
        analyzer
    }

    fn build_address_map(&mut self) {
        let Some(config) = self.config else { return };

        for (addr_str, mem_map) in &config.memory {
            let addr = Self::parse_address(addr_str);
            let space = Self::classify_address(addr, mem_map);
            self.address_spaces.insert(addr_str.clone(), space);
        }
    }

    fn parse_address(addr_str: &str) -> u64 {
        let clean = addr_str.trim_start_matches("0x").trim_start_matches("0X");
        u64::from_str_radix(clean, 16).unwrap_or(0)
    }

    fn classify_address(addr: u64, mem_map: &MemoryMapping) -> AddressSpace {
        // FPGA Internal BRAM: typically 0x40A80000+
        if addr >= 0x40A80000 && addr < 0x50000000 {
            return AddressSpace::FpgaInternal;
        }

        // UltraRAM: 0x40B00000+
        if addr >= 0x40B00000 && addr < 0x5000000 {
            return AddressSpace::FpgaInternal;
        }

        // MMIO ranges (common for AXI4-Lite)
        // 0x4000A000 - 0x4000AFFF (first AXI slave)
        // 0x8000A000 - 0x8000AFFF (second AXI slave - KV260)
        if (0x4000A000..=0x4000AFFF).contains(&addr) || 
           (0x8000A000..=0x8000AFFF).contains(&addr) {
            return AddressSpace::Mmio(addr);
        }

        // Other MMIO ranges
        if (0x40000000..=0x4FFFFFFF).contains(&addr) ||
           (0x80000000..=0x8FFFFFFF).contains(&addr) {
            return AddressSpace::Mmio(addr);
        }

        // Default: treat as DDR4 (CPU accessible)
        AddressSpace::Ddr4
    }

    pub fn classify(&self, addr_str: &str) -> AddressSpace {
        self.address_spaces.get(addr_str)
            .cloned()
            .unwrap_or(AddressSpace::Unknown)
    }

    pub fn is_cpu_accessible(&self, addr_str: &str) -> bool {
        match self.classify(addr_str) {
            AddressSpace::Ddr4 => true,
            AddressSpace::Mmio(_) => true,
            AddressSpace::FpgaInternal => false,
            AddressSpace::Unknown => true, // Assume accessible if unknown
        }
    }

    pub fn is_fpga_internal(&self, addr_str: &str) -> bool {
        matches!(self.classify(addr_str), AddressSpace::FpgaInternal)
    }

    pub fn validate_program(&self, program: &crate::ast::Program) -> Vec<AddressValidationError> {
        let mut errors = Vec::new();

        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                if let Some(addr) = decl.address {
                    let addr_str = format!("0x{:08X}", addr);
                    
                    // Check if this is an FPGA-internal address
                    if self.is_fpga_internal(&addr_str) {
                        // Check if any transaction writes to this address
                        for item2 in &program.items {
                            if let TopLevel::Transaction(txn) = item2 {
                                for stmt in &txn.body {
                                    if let crate::ast::Statement::Assignment { lhs, .. } = stmt {
                                        if let crate::ast::Expr::Identifier(name) = lhs {
                                            if name == &decl.name {
                                                errors.push(AddressValidationError {
                                                    variable: decl.name.clone(),
                                                    address: addr_str.clone(),
                                                    message: format!(
                                                        "Variable '{}' at address {} is in FPGA-internal memory. \
                                                        CPU cannot directly access FPGA BRAM/URAM at 0x40A80000+. \
                                                        Use AXI mailbox transactions to transfer data.",
                                                        decl.name, addr_str
                                                    ),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        errors
    }
}

#[derive(Debug, Clone)]
pub struct AddressValidationError {
    pub variable: String,
    pub address: String,
    pub message: String,
}

impl std::fmt::Display for AddressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Address violation for '{}' at {}: {}", 
               self.variable, self.address, self.message)
    }
}

impl std::error::Error for AddressValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_classification() {
        // FPGA internal
        assert!(matches!(
            AddressSpaceAnalyzer::classify_address(0x40A80000, &MemoryMapping { size: 1, mem_type: "bram".to_string(), element_bits: 16 }),
            AddressSpace::FpgaInternal
        ));

        // MMIO
        assert!(matches!(
            AddressSpaceAnalyzer::classify_address(0x8000A000, &MemoryMapping { size: 1, mem_type: "flipflop".to_string(), element_bits: 8 }),
            AddressSpace::Mmio(0x8000A000)
        ));

        // DDR4 (default)
        assert!(matches!(
            AddressSpaceAnalyzer::classify_address(0x10000000, &MemoryMapping { size: 1, mem_type: "ddr4".to_string(), element_bits: 32 }),
            AddressSpace::Ddr4
        ));
    }
}