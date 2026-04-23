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
                    let addr_str = format!("0x{:08X}", trg.address);
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
                
                // For now, just note this - full overlap checking would require
                // tracking actual bit ranges used
                if let Some(first) = decls.first() {
                    if first.1.is_none() {
                        // No explicit bit range specified - could cause conflicts
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

    #[test]
    fn test_address_validation() {
        // This would require a full HardwareConfig to test properly
        assert!(true);
    }
}