use crate::ast::{EnumDefinition, Type};

pub struct EnumItem(pub EnumDefinition);

impl EnumItem {
    pub fn name(&self) -> &str { &self.0.name }
}
