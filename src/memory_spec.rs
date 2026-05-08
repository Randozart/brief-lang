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

//! Memory Spec Output
//!
//! Collects all variable/register/address allocations during compilation
//! and outputs a JSON/TOML spec for foreign language consumption.

use crate::ast::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct MemorySpec {
    pub target: String,
    pub compiler_version: String,
    pub allocations: BTreeMap<String, Allocation>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub metropolitan_ffi: BTreeMap<String, FfiRegion>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub triggers: BTreeMap<String, TriggerInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Allocation {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    pub size_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub is_trigger: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FfiRegion {
    pub address: String,
    pub size_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerInfo {
    #[serde(rename = "type")]
    pub trigger_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl MemorySpec {
    pub fn new(target: &str) -> Self {
        MemorySpec {
            target: target.to_string(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            allocations: BTreeMap::new(),
            metropolitan_ffi: BTreeMap::new(),
            triggers: BTreeMap::new(),
        }
    }

    /// Collect allocations from a parsed Brief program
    pub fn collect_from_program(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.add_state_decl(decl);
                }
                TopLevel::Trigger(trg) => {
                    self.add_trigger_decl(trg);
                }
                TopLevel::Transaction(txn) => {
                    self.add_transaction(txn);
                }
                _ => {}
            }
        }
    }

    fn add_state_decl(&mut self, decl: &StateDecl) {
        let type_name = format_type(&decl.ty);
        let address = decl.address.map(|a| format!("0x{:X}", a));
        let bit_range = decl.bit_range.as_ref().map(|br| format_bit_range(br));

        let size = estimate_type_size(&decl.ty);

        self.allocations.insert(
            decl.name.clone(),
            Allocation {
                type_name,
                address,
                size_bytes: size,
                bit_range,
                stage: None,
                is_trigger: false,
            },
        );
    }

    fn add_trigger_decl(&mut self, trg: &TriggerDeclaration) {
        let type_name = format!("trg {}", format_type(&trg.ty));
        let address = match &trg.address {
            LinkRef::Explicit(addr) => Some(format!("0x{:X}", addr)),
            LinkRef::Linked(name) => Some(format!("link:{}", name)),
        };
        let bit_range = trg.bit_range.as_ref().map(|br| format_bit_range(br));
        let size = estimate_type_size(&trg.ty);

        self.allocations.insert(
            trg.name.clone(),
            Allocation {
                type_name,
                address: address.clone(),
                size_bytes: size,
                bit_range,
                stage: if !trg.stages.is_empty() {
                    Some(trg.stages.join(","))
                } else {
                    None
                },
                is_trigger: true,
            },
        );

        // Also add to triggers map with binding info
        self.triggers.insert(
            trg.name.clone(),
            TriggerInfo {
                trigger_type: "hardware".to_string(),
                binding: address.clone(),
                mode: trg.condition.as_ref().map(|_| "conditional".to_string()),
            },
        );
    }

    fn add_transaction(&mut self, txn: &Transaction) {
        // Collect local triggers inside transactions
        for stmt in &txn.body {
            if let Statement::LocalTrigger { name, ty, expr, .. } = stmt {
                let type_name = format!("trg! {}", format_type(ty));
                let size = estimate_type_size(ty);

                self.allocations.insert(
                    format!("{}.{}", txn.name, name),
                    Allocation {
                        type_name,
                        address: None, // Local triggers don't have fixed addresses
                        size_bytes: size,
                        bit_range: None,
                        stage: None,
                        is_trigger: true,
                    },
                );
            }
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Serialize to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Float => "Float".to_string(),
        Type::Bool => "Bool".to_string(),
        Type::String => "String".to_string(),
        Type::Void => "void".to_string(),
        Type::Data => "Data".to_string(),
        Type::Char => "Char".to_string(),
        Type::Custom(name) => name.clone(),
        Type::Union(types) => {
            let inner: Vec<_> = types.iter().map(format_type).collect();
            inner.join(" | ")
        }
        Type::Tuple(types) => {
            let inner: Vec<_> = types.iter().map(format_type).collect();
            format!("({})", inner.join(", "))
        }
        Type::ContractBound(inner, _) => format_type(inner),
        Type::TypeVar(name) => name.clone(),
        Type::Generic(name, params) => {
            let inner: Vec<_> = params.iter().map(format_type).collect();
            format!("{}<{}>", name, inner.join(", "))
        }
        Type::Applied(name, args) => {
            let inner: Vec<_> = args.iter().map(format_type).collect();
            format!("{}<{}>", name, inner.join(", "))
        }
        Type::Sig(name) => format!("sig:{}", name),
        Type::Vector(elem, dims) => {
            let total_size: usize = dims.iter().map(|d| match d {
                crate::ast::Dimension::Anonymous(s) => *s,
                crate::ast::Dimension::Named(_, s) => *s,
            }).product();
            format!("Vec<{}; {}>", format_type(elem), total_size)
        }
        Type::Enum(name) => format!("enum:{}", name),
        Type::Constrained(inner, bit_range) => {
            format!("{}@/{}", format_type(inner), format_bit_range(bit_range))
        }
    }
}

fn format_bit_range(br: &BitRange) -> String {
    match br {
        BitRange::Single(n) => format!("{}", n),
        BitRange::Any(n) => format!("x{}", n),
        BitRange::Range(start, end) => format!("{}..{}", start, end),
    }
}

fn estimate_type_size(ty: &Type) -> usize {
    match ty {
        Type::Int | Type::UInt => 8,
        Type::Float => 8,
        Type::Bool => 1,
        Type::String => 24,
        Type::Void => 0,
        Type::Data => 24,
        Type::Char => 4,
        Type::Custom(_) => 8,
        Type::Union(types) => types.iter().map(estimate_type_size).max().unwrap_or(8),
        Type::Tuple(types) => types.iter().map(estimate_type_size).sum(),
        Type::ContractBound(inner, _) => estimate_type_size(inner),
        Type::TypeVar(_) => 8,
        Type::Generic(_, _) => 8,
        Type::Applied(_, _) => 8,
        Type::Sig(_) => 8,
        Type::Vector(elem, dims) => {
            let total_size: usize = dims.iter().map(|d| match d {
                crate::ast::Dimension::Anonymous(s) => *s,
                crate::ast::Dimension::Named(_, s) => *s,
            }).product();
            estimate_type_size(elem) * total_size
        }
        Type::Enum(_) => 8,
        Type::Constrained(_, BitRange::Single(_)) => 1,
        Type::Constrained(_, BitRange::Any(n)) => (*n + 7) / 8,
        Type::Constrained(_, BitRange::Range(start, end)) => (end - start + 1 + 7) / 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_spec_empty() {
        let spec = MemorySpec::new("test");
        assert_eq!(spec.target, "test");
        assert!(spec.allocations.is_empty());
    }

    #[test]
    fn test_memory_spec_json_output() {
        let mut spec = MemorySpec::new("aarch64");
        spec.allocations.insert(
            "counter".to_string(),
            Allocation {
                type_name: "Int".to_string(),
                address: Some("0x1000".to_string()),
                size_bytes: 8,
                bit_range: None,
                stage: None,
                is_trigger: false,
            },
        );
        let json = spec.to_json().unwrap();
        assert!(json.contains("counter"));
        assert!(json.contains("0x1000"));
    }

    #[test]
    fn test_format_bit_range() {
        assert_eq!(format_bit_range(&BitRange::Any(16)), "x16");
        assert_eq!(format_bit_range(&BitRange::Range(3, 7)), "3..7");
    }

    #[test]
    fn test_estimate_type_size() {
        assert_eq!(estimate_type_size(&Type::Bool), 1);
        assert_eq!(estimate_type_size(&Type::Int), 8);
        assert_eq!(
            estimate_type_size(&Type::Constrained(
                Box::new(Type::UInt),
                BitRange::Any(8)
            )),
            1
        );
        assert_eq!(
            estimate_type_size(&Type::Constrained(
                Box::new(Type::UInt),
                BitRange::Any(32)
            )),
            4
        );
    }
}
