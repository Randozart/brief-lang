// ── TopLevel::Test — #test("group") Pragma ─────────────────────────
//
// Phase 5: Wraps a TopLevel item in test group metadata. Skipped in
// production; included in test mode (--dev, --test, --group flags).

use crate::ast::{TopLevel, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct TestItem {
    pub item: Box<TopLevel>,
    pub groups: Vec<String>,
}

impl TestItem {
    pub fn inner(&self) -> &TopLevel {
        &self.item
    }
}
