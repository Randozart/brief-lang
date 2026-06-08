// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{FfiValue, MemoryLayout, Endian};

    struct DummyMapper;
    impl Mapper for DummyMapper {
        fn drop(&self, _buf: &mut [u8], _layout: &MemoryLayout, _data: &[FfiValue]) -> Result<usize, String> {
            Ok(42)
        }
        fn fetch(&self, _buf: &[u8], _layout: &MemoryLayout) -> Result<FfiValue, String> {
            Ok(FfiValue::Void)
        }
        fn validate(&self, _buf: &[u8], _contract: &str) -> bool {
            true
        }
    }

    #[test]
    fn test_mapper_trait_object_safety() {
        let mapper: Box<dyn Mapper> = Box::new(DummyMapper);
        let layout = MemoryLayout { endian: Endian::Native, fields: vec![], alignment: 1, size_bytes: 0 };
        let result: Result<usize, String> = Mapper::drop(&*mapper, &mut [], &layout, &[]);
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_value_debug_clone() {
        let v = Value::Int(42);
        let _debug = format!("{:?}", v);
        let _cloned = v.clone();
        assert!(matches!(v, Value::Int(42)));
    }
}
