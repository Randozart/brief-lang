use crate::ast::{Transaction, Type};

pub struct TransactionItem(pub Transaction);

impl TransactionItem {
    pub fn name(&self) -> &str { &self.0.name }
}
