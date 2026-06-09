use crate::ast::{ForeignSignature, ForeignTarget, Type};
use crate::errors::Span;

pub struct ForeignItem {
    pub name: String,
    pub toml_path: String,
    pub signature: ForeignSignature,
    pub target: ForeignTarget,
    pub span: Option<Span>,
}
