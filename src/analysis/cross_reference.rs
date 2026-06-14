use crate::ast::{HardwareConfig, Program, TopLevel};
use std::collections::HashSet;

pub struct CrossReferenceValidator {
    hw_config: &'static HardwareConfig,
}

impl CrossReferenceValidator {
    pub fn new(hw_config: &'static HardwareConfig) -> Self {
        CrossReferenceValidator { hw_config }
    }

    pub fn validate(&self, program: &Program) -> Vec<CrossRefError> {
        let mut errors = Vec::new();

        // Collect all addresses used in the .ebv file
        let mut used_addrs: HashSet<String> = HashSet::new();
        let mut defined_addrs: HashSet<String> = HashSet::new();

        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if let Some(addr) = decl.address {
                        let addr_str = format!("0x{:08X}", addr);
                        used_addrs.insert(addr_str.clone());
                        
                        // Check if this address exists in hardware.toml
                        if !self.hw_config.memory.contains_key(&addr_str) {
                            // Also try lowercase
                            let addr_lower = addr_str.to_lowercase();
                            if !self.hw_config.memory.contains_key(&addr_lower) {
                                errors.push(CrossRefError {
                                    variable: decl.name.clone(),
                                    address: addr_str.clone(),
                                    error_type: CrossRefErrorType::AddressNotInHardwareConfig,
                                    message: format!(
                                        "Variable '{}' uses address {} which is not defined in hardware.toml",
                                        decl.name, addr_str
                                    ),
                                });
                            }
                        }
                    }
                }
                TopLevel::Trigger(trg) => {
                    match &trg.address {
                        crate::ast::LinkRef::Explicit(addr) => {
                            let addr_str = format!("0x{:08X}", addr);
                            used_addrs.insert(addr_str.clone());

                            if !self.hw_config.memory.contains_key(&addr_str) {
                                let addr_lower = addr_str.to_lowercase();
                                if !self.hw_config.memory.contains_key(&addr_lower) {
                                    errors.push(CrossRefError {
                                        variable: trg.name.clone(),
                                        address: addr_str,
                                        error_type: CrossRefErrorType::TriggerAddressNotInHardwareConfig,
                                        message: format!(
                                            "Trigger '{}' address not defined in hardware.toml",
                                            trg.name
                                        ),
                                    });
                                }
                            }
                        }
                        crate::ast::LinkRef::Linked(name) => {
                            used_addrs.insert(format!("link:{}", name));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Collect defined addresses from hardware.toml
        for addr in self.hw_config.memory.keys() {
            defined_addrs.insert(addr.clone());
            defined_addrs.insert(addr.to_lowercase());
        }

        // Warn about unused addresses in hardware.toml (optional, not error)
        for addr in &defined_addrs {
            if !used_addrs.contains(addr) {
                // This is informational - some memory regions might be reserved
                // Not adding as error, could be a warning in verbose mode
            }
        }

        errors
    }

    pub fn check_address_consistency(&self, program: &Program) -> Vec<CrossRefError> {
        let mut errors = Vec::new();

        // Group declarations by address
        let mut addr_groups: std::collections::HashMap<u64, Vec<(String, Option<String>)>> = 
            std::collections::HashMap::new();

        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                if let Some(addr) = decl.address {
                    addr_groups.entry(addr).or_default().push((
                        decl.name.clone(),
                        decl.bit_range.as_ref().map(|br| format!("{:?}", br))
                    ));
                }
            }
        }

        // Check that multiple declarations at same address don't conflict
        for (addr, decls) in &addr_groups {
            if decls.len() > 1 {
                // Multiple variables at same address - this is allowed for bit packing
                // but we should verify bit ranges don't overlap
                let addr_str = format!("0x{:08X}", addr);
                
                // Check if ANY declaration at this address lacks an explicit bit range.
                // Only checking the first (as before) would miss cases where a later
                // declaration has no bit range while an earlier one does.
                let has_implicit_overlap = decls.iter().any(|(_, range)| range.is_none());
                if has_implicit_overlap {
                    let addr_for_error = addr_str.clone();
                    errors.push(CrossRefError {
                        variable: decls.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(", "),
                        address: addr_str,
                        error_type: CrossRefErrorType::ImplicitOverlap,
                        message: format!(
                            "Multiple variables at address {} without explicit bit ranges. \
                            Specify /bit ranges to avoid conflicts.",
                            addr_for_error
                        ),
                    });
                }
            }
        }

        errors
    }
}

#[derive(Debug, Clone)]
pub enum CrossRefErrorType {
    AddressNotInHardwareConfig,
    TriggerAddressNotInHardwareConfig,
    ImplicitOverlap,
    AddressRangeMismatch,
}

#[derive(Debug, Clone)]
pub struct CrossRefError {
    pub variable: String,
    pub address: String,
    pub error_type: CrossRefErrorType,
    pub message: String,
}

impl std::fmt::Display for CrossRefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CrossRefError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{HardwareConfig, MemoryMapping, IoMapping, ProjectConfig, TargetConfig, InterfaceConfig};

    fn make_test_hw_config() -> HardwareConfig {
        HardwareConfig {
            project: ProjectConfig {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
            },
            target: TargetConfig {
                fpga: "xczu4ev".to_string(),
                clock_hz: 100_000_000,
                platform: None,
                synthesis: None,
            },
            interface: InterfaceConfig {
                name: "axi4-lite".to_string(),
                address_width: Some(18),
                data_width: Some(32),
                controller: None,
                situs: None,
            },
            memory: std::collections::HashMap::from([
                ("0x8000A000".to_string(), MemoryMapping {
                    size: 1,
                    mem_type: "flipflop".to_string(),
                    element_bits: 8,
                }),
                ("0x8000A004".to_string(), MemoryMapping {
                    size: 1,
                    mem_type: "flipflop".to_string(),
                    element_bits: 8,
                }),
                ("0x40A80000".to_string(), MemoryMapping {
                    size: 262144,
                    mem_type: "bram".to_string(),
                    element_bits: 16,
                }),
            ]),
            io: None,
        }
    }

    fn static_hw_config() -> &'static HardwareConfig {
        Box::leak(Box::new(make_test_hw_config()))
    }

    #[test]
    fn test_address_validation_known_address() {
        let hw_config = static_hw_config();
        let validator = CrossReferenceValidator::new(hw_config);

        let code = r#"
            let control: UInt @ 0x8000A000 = 0;
        "#;
        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let errors = validator.validate(&program);
        assert!(errors.is_empty(), "Expected no errors for known address, got: {:?}", errors);
    }

    #[test]
    fn test_address_validation_unknown_address() {
        let hw_config = static_hw_config();
        let validator = CrossReferenceValidator::new(hw_config);

        let code = r#"
            let unknown: UInt @ 0xDEADBEEF = 0;
        "#;
        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let errors = validator.validate(&program);
        assert!(!errors.is_empty(), "Expected error for unknown address");
        assert!(errors.iter().any(|e| e.variable == "unknown"), "Error should mention 'unknown' variable");
    }

    #[test]
    fn test_link_ref_not_validated_as_address() {
        let hw_config = static_hw_config();
        let validator = CrossReferenceValidator::new(hw_config);

        let code = r#"
            trg signal: Bool @ link my_signal;
        "#;
        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let errors = validator.validate(&program);
        assert!(errors.is_empty(), "Linked references should not trigger address validation");
    }

    #[test]
    fn test_multiple_vars_same_address_no_overlap() {
        let hw_config = static_hw_config();
        let validator = CrossReferenceValidator::new(hw_config);

        let code = r#"
            let low: UInt @ 0x8000A000 /0..3 = 0;
            let high: UInt @ 0x8000A000 /4..7 = 0;
        "#;
        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let errors = validator.check_address_consistency(&program);
        assert!(errors.is_empty(), "Explicit bit ranges should prevent overlap errors");
    }

    #[test]
    fn test_implicit_overlap_detection() {
        let hw_config = static_hw_config();
        let validator = CrossReferenceValidator::new(hw_config);

        let code = r#"
            let a: UInt @ 0x8000A000 = 0;
            let b: UInt @ 0x8000A000 = 0;
        "#;
        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let errors = validator.check_address_consistency(&program);
        assert!(!errors.is_empty(), "Should detect implicit overlap");
    }
}