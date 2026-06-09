use crate::ast::{Expr, Type};

pub struct AssertionItem {
    pub pre: Expr,
    pub chain: Vec<String>,
}
