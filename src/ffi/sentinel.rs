//! FFI Sentinel
//!
//! Validates pre-conditions and post-conditions for FFI calls.

use super::types::FfiValue;
use crate::ast::ForeignBinding;

pub struct Sentinel;

impl Sentinel {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_precondition(
        &self,
        binding: &ForeignBinding,
        args: &[FfiValue],
    ) -> Result<(), String> {
        if let Some(pre) = &binding.precondition {
            // TODO: Real expression evaluation for contracts
            // For now, we just check if it's "true"
            if pre != "true" && !pre.is_empty() {
                // eprintln!("[DEBUG] Precondition check: {}", pre);
            }
        }
        Ok(())
    }

    pub fn validate_postcondition(
        &self,
        binding: &ForeignBinding,
        result: &FfiValue,
    ) -> Result<(), String> {
        if let Some(post) = &binding.postcondition {
            // TODO: Real expression evaluation for contracts
            if post != "true" && !post.is_empty() {
                // eprintln!("[DEBUG] Postcondition check: {}", post);
            }
        }
        Ok(())
    }
}
