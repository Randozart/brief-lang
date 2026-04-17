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
