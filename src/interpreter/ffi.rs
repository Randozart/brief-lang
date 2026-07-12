// ── FFI Dispatch ───────────────────────────────────────────────────────
// 2026-07-12: Phase 3.4 — Foreign function call dispatch.
// No Intrinsic references — all ffis are dispatched by name string.

use crate::errors::RuntimeError;
use crate::interpreter::Value;

/// Dispatch a foreign function call.
/// This is a simplified stub. Full implementation loads native libraries
/// via libloading and marshals parameters between Value::Bits and C types.
pub fn dispatch_ffi(name: &str, _args: &[Value]) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UndefinedForeignFunction {
        name: name.to_string(),
        source: "FFI".to_string(),
    })
}
