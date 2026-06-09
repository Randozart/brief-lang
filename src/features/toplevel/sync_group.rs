use crate::ast::{TopLevel, Type};

pub struct SyncGroupItem {
    pub domains: Vec<String>,
    pub item: Box<TopLevel>,
}
