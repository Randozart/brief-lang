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

use super::protocol::Mapper;
use super::types::{Endian, FfiValue, MemoryLayout};

pub struct NativeMapper;

impl Mapper for NativeMapper {
    fn drop(
        &self,
        buffer: &mut [u8],
        layout: &MemoryLayout,
        data: &[FfiValue],
    ) -> Result<usize, String> {
        let mut total_written = 0;

        for (i, field) in layout.fields.iter().enumerate() {
            if i >= data.len() {
                break;
            }

            let val = &data[i];
            let offset = field.offset;
            let size = field.size_bytes;

            if offset + size > buffer.len() {
                return Err(format!(
                    "Buffer overflow at field {}: offset {} + size {} > buffer len {}",
                    field.name,
                    offset,
                    size,
                    buffer.len()
                ));
            }

            // Handle embedded mode: 'address' constructor argument sets the buffer location
            if field.name == "address" {
                if let FfiValue::Int(addr) = val {
                    let addr_usize = *addr as usize;
                    // If buffer is at a fixed address in embedded mode, ensure we're writing there
                    let base_ptr = buffer.as_mut_ptr() as usize;
                    let buffer_size = buffer.len();
                    
                    // Check if requested address is within the buffer range
                    if addr_usize < base_ptr || addr_usize + buffer_size > base_ptr + buffer_size {
                        return Err(format!(
                            "Address 0x{:x} outside buffer range [0x{:x}, 0x{:x})",
                            addr_usize, base_ptr, base_ptr + buffer_size
                        ));
                    }
                }
            }

            let field_endian = field.endian.unwrap_or(layout.endian);

            match val {
                FfiValue::Int(v) => {
                    let bytes = match field_endian {
                        Endian::Big => v.to_be_bytes(),
                        Endian::Little => v.to_le_bytes(),
                        Endian::Native => v.to_ne_bytes(),
                    };
                    buffer[offset..offset + size.min(8)].copy_from_slice(&bytes[..size.min(8)]);
                }
                FfiValue::Float(v) => {
                    let bytes = match field_endian {
                        Endian::Big => v.to_be_bytes(),
                        Endian::Little => v.to_le_bytes(),
                        Endian::Native => v.to_ne_bytes(),
                    };
                    buffer[offset..offset + size.min(8)].copy_from_slice(&bytes[..size.min(8)]);
                }
                FfiValue::Bool(v) => {
                    buffer[offset] = if *v { 1 } else { 0 };
                }
                FfiValue::String(s) => {
                    // For native, we might pass a pointer or copy bytes.
                    // For now, let's copy bytes up to size.
                    let s_bytes = s.as_bytes();
                    let copy_len = s_bytes.len().min(size);
                    buffer[offset..offset + copy_len].copy_from_slice(&s_bytes[..copy_len]);
                }
                FfiValue::Data(d) => {
                    let copy_len = d.len().min(size);
                    buffer[offset..offset + copy_len].copy_from_slice(&d[..copy_len]);
                }
                FfiValue::Void => {}
                _ => {
                    return Err(format!(
                        "Unsupported FfiValue for NativeMapper::drop: {:?}",
                        val
                    ))
                }
            }
            total_written += size;
        }

        Ok(total_written)
    }

    fn fetch(&self, buffer: &[u8], layout: &MemoryLayout) -> Result<FfiValue, String> {
        // For fetch, we usually return the first field as the main result if it's a simple type,
        // or a struct if there are multiple fields.
        if layout.fields.is_empty() {
            return Ok(FfiValue::Void);
        }

        if layout.fields.len() == 1 {
            return self.fetch_field(buffer, &layout.fields[0], layout.endian);
        }

        let mut fields = std::collections::HashMap::new();
        for field in &layout.fields {
            fields.insert(
                field.name.clone(),
                self.fetch_field(buffer, field, layout.endian)?,
            );
        }

        Ok(FfiValue::Struct("Result".to_string(), fields))
    }

    fn validate(&self, _buffer: &[u8], _contract: &str) -> bool {
        // TODO: Implement contract validation
        true
    }
}

impl NativeMapper {
    fn fetch_field(
        &self,
        buffer: &[u8],
        field: &super::types::FieldDescriptor,
        default_endian: Endian,
    ) -> Result<FfiValue, String> {
        let offset = field.offset;
        let size = field.size_bytes;
        let endian = field.endian.unwrap_or(default_endian);

        if offset + size > buffer.len() {
            return Err(format!("Buffer underflow reading field {}", field.name));
        }

        let field_bytes = &buffer[offset..offset + size];

        // This is a naive implementation. Real implementation would need to know the expected type.
        // For now, we'll guess based on size.
        match size {
            1 => Ok(FfiValue::Bool(field_bytes[0] != 0)),
            4 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(field_bytes);
                let val = match endian {
                    Endian::Big => i32::from_be_bytes(bytes),
                    Endian::Little => i32::from_le_bytes(bytes),
                    Endian::Native => i32::from_ne_bytes(bytes),
                };
                Ok(FfiValue::Int(val as i64))
            }
            8 => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(field_bytes);
                // We don't know if it's Int or Float. Let's assume Int for now if not specified.
                let val = match endian {
                    Endian::Big => i64::from_be_bytes(bytes),
                    Endian::Little => i64::from_le_bytes(bytes),
                    Endian::Native => i64::from_ne_bytes(bytes),
                };
                Ok(FfiValue::Int(val))
            }
            _ => Ok(FfiValue::Data(field_bytes.to_vec())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{Endian, FfiValue, MemoryLayout, FieldDescriptor};

    fn make_layout(fields: Vec<FieldDescriptor>) -> MemoryLayout {
        let total_bytes = fields.iter().map(|f| f.offset + f.size_bytes).max().unwrap_or(0);
        MemoryLayout { size_bytes: total_bytes, alignment: 8, fields, endian: Endian::Little }
    }

    #[test]
    fn test_drop_int_little_endian() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![FieldDescriptor {
            name: "val".into(), offset: 0, size_bytes: 8, element_size: None, count: None, endian: Some(Endian::Little),
        }]);
        let mut buf = vec![0u8; 8];
        let data = [FfiValue::Int(0x0102030405060708i64)];
        let written = Mapper::drop(&mapper, &mut buf, &layout, &data).unwrap();
        assert_eq!(written, 8);
        assert_eq!(buf, vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_drop_int_big_endian() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![FieldDescriptor {
            name: "val".into(), offset: 0, size_bytes: 8, element_size: None, count: None, endian: Some(Endian::Big),
        }]);
        let mut buf = vec![0u8; 8];
        let data = [FfiValue::Int(0x0102030405060708i64)];
        Mapper::drop(&mapper, &mut buf, &layout, &data).unwrap();
        assert_eq!(buf, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_drop_buffer_overflow_error() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![FieldDescriptor {
            name: "val".into(), offset: 10, size_bytes: 8, element_size: None, count: None, endian: None,
        }]);
        let mut buf = vec![0u8; 8];
        let data = [FfiValue::Int(42)];
        let result = Mapper::drop(&mapper, &mut buf, &layout, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_bool_true_and_false() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![
            FieldDescriptor { name: "a".into(), offset: 0, size_bytes: 1, element_size: None, count: None, endian: None },
            FieldDescriptor { name: "b".into(), offset: 1, size_bytes: 1, element_size: None, count: None, endian: None },
        ]);
        let mut buf = vec![0u8; 2];
        Mapper::drop(&mapper, &mut buf, &layout, &[FfiValue::Bool(true), FfiValue::Bool(false)]).unwrap();
        assert_eq!(buf, vec![1, 0]);
    }

    #[test]
    fn test_drop_string_truncation() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![FieldDescriptor {
            name: "s".into(), offset: 0, size_bytes: 4, element_size: None, count: None, endian: None,
        }]);
        let mut buf = vec![0u8; 4];
        Mapper::drop(&mapper, &mut buf, &layout, &[FfiValue::String("abcdef".into())]).unwrap();
        assert_eq!(&buf, b"abcd");
    }

    #[test]
    fn test_fetch_field_by_size() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![
            FieldDescriptor { name: "b".into(), offset: 0, size_bytes: 1, element_size: None, count: None, endian: None },
            FieldDescriptor { name: "i".into(), offset: 1, size_bytes: 4, element_size: None, count: None, endian: None },
        ]);
        let mut buf = vec![0u8; 5];
        buf[0] = 1;
        buf[1..5].copy_from_slice(&42i32.to_le_bytes());
        match mapper.fetch(&buf, &layout).unwrap() {
            FfiValue::Struct(_, fields) => {
                assert_eq!(fields.get("b"), Some(&FfiValue::Bool(true)));
                assert_eq!(fields.get("i"), Some(&FfiValue::Int(42)));
            }
            _ => panic!("Expected Struct"),
        }
    }

    #[test]
    fn test_fetch_field_underflow_error() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![FieldDescriptor {
            name: "x".into(), offset: 10, size_bytes: 4, element_size: None, count: None, endian: None,
        }]);
        let buf = vec![0u8; 5];
        assert!(mapper.fetch(&buf, &layout).is_err());
    }

    #[test]
    fn test_fetch_empty_layout_returns_void() {
        let mapper = NativeMapper;
        let layout = make_layout(vec![]);
        assert_eq!(mapper.fetch(&[], &layout).unwrap(), FfiValue::Void);
    }
}
