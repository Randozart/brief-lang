use crate::ast::{Expr, Signature, Type};

pub struct SignatureItem(pub Signature);

impl SignatureItem {
    pub fn name(&self) -> &str { &self.0.name }
}
