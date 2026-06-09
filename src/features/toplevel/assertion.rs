// ── TopLevel::Assertion — #!assert Directive ──────────────────────
//
// Phase 6: Compile-time assertion chain. Syntax:
//   #!assert [pre_condition] fn_a -> fn_b -> fn_c;
// The proof engine verifies that executing the chain of functions
// from the precondition reaches each step's postcondition.

use crate::ast::{Expr, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct AssertionItem {
    pub pre: Expr,
    pub chain: Vec<String>,
}
