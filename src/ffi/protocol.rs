//! FFI Mapper Protocol
//!
//! Defines the interface for pluggable memory mappers.

use super::types::{FfiValue, MemoryLayout};

/// The Mapper trait defines how data is moved into and out of memory pipes.
pub trait Mapper: Send + Sync {
    /// Write input data to the provided buffer according to the memory layout.
    /// Returns the number of bytes written.
    fn drop(
        &self,
        buffer: &mut [u8],
        layout: &MemoryLayout,
        data: &[FfiValue],
    ) -> Result<usize, String>;

    /// Read output data from the buffer according to the memory layout.
    fn fetch(&self, buffer: &[u8], layout: &MemoryLayout) -> Result<FfiValue, String>;

    /// Validate that the data in the buffer satisfies a contract.
    fn validate(&self, buffer: &[u8], contract: &str) -> bool;
}

/// A value that can be passed through a memory pipe.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Data(Vec<u8>),
    Void,
}
