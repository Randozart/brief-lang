use crate::ast::{Definition, Type};

pub struct DefinitionItem(pub Definition);

impl DefinitionItem {
    pub fn name(&self) -> &str { &self.0.name }
}
