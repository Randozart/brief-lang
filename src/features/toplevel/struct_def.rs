use crate::ast::{StructDefinition, Type};

pub struct StructItem(pub StructDefinition);

impl StructItem {
    pub fn name(&self) -> &str { &self.0.name }
}
