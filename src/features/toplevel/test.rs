use crate::ast::{Expr, Type};

pub struct TestItem {
    pub item: Box<crate::ast::TopLevel>,
    pub groups: Vec<String>,
}
