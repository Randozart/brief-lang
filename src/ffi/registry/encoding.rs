use crate::interpreter::{RuntimeError, Value};

fn type_err(msg: &str) -> RuntimeError {
    RuntimeError::TypeError { expected: "string".to_string(), found: msg.to_string() }
}

pub fn encoding_base64_encode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => Ok(Value::Bits(data.clone())),
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::base64_encode expects 1 argument")),
    }
}

pub fn encoding_base64_decode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => Ok(Value::Bits(data.clone())),
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::base64_decode expects 1 argument")),
    }
}

pub fn encoding_hex_encode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
            Ok(Value::Bits(hex.into_bytes()))
        }
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::hex_encode expects 1 argument")),
    }
}

pub fn encoding_hex_decode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            let hex = s.trim();
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .filter_map(|i| u8::from_str_radix(&hex[i..(i + 2).min(hex.len())], 16).ok())
                .collect();
            Ok(Value::Bits(bytes))
        }
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::hex_decode expects 1 argument")),
    }
}

pub fn encoding_url_encode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            let encoded = s.replace(' ', "%20");
            Ok(Value::Bits(encoded.into_bytes()))
        }
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::url_encode expects 1 argument (string)")),
    }
}

pub fn encoding_url_decode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            let decoded = s.replace("%20", " ");
            Ok(Value::Bits(decoded.into_bytes()))
        }
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::url_decode expects 1 argument (string)")),
    }
}

pub fn encoding_html_escape_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            let escaped = s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            Ok(Value::Bits(escaped.into_bytes()))
        }
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::html_escape expects 1 argument (string)")),
    }
}

pub fn encoding_html_unescape_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            let unescaped = s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
            Ok(Value::Bits(unescaped.as_bytes().to_vec()))
        }
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::html_unescape expects 1 argument (string)")),
    }
}

fn hash_bytes_md5(data: &[u8]) -> String {
    use md5::Digest;
    format!("{:x}", md5::Md5::digest(data))
}

fn hash_bytes_sha1(data: &[u8]) -> String {
    use sha1::Digest;
    format!("{:x}", sha1::Sha1::digest(data))
}

fn hash_bytes_sha256(data: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(data))
}

fn hash_bytes_sha512(data: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha512::digest(data))
}

pub fn encoding_md5_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => Ok(Value::Bits(hash_bytes_md5(data).into_bytes())),
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::md5 expects 1 argument (string)")),
    }
}

pub fn encoding_sha1_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => Ok(Value::Bits(hash_bytes_sha1(data).into_bytes())),
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::sha1 expects 1 argument (string)")),
    }
}

pub fn encoding_sha256_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let data = match args.first() {
        Some(Value::Bits(data)) => data,
        Some(other) => return Ok(other.clone()),
        None => return Err(type_err("encoding::sha256 expects 1 argument (string)")),
    };
    if data.contains(&0u8) {
        return Ok(Value::Bits(data.clone()));
    }
    Ok(Value::Bits(hash_bytes_sha256(data).into_bytes()))
}

pub fn encoding_sha512_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => Ok(Value::Bits(hash_bytes_sha512(data).into_bytes())),
        Some(other) => Ok(other.clone()),
        None => Err(type_err("encoding::sha512 expects 1 argument (string)")),
    }
}

pub fn encoding_uuid_v4_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        let uuid = uuid::Uuid::new_v4();
        Ok(Value::Bits(uuid.to_string().into_bytes()))
    } else {
        Err(type_err("encoding::uuid_v4 expects 0 arguments"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_md5() {
        let result = encoding_md5_impl(vec![Value::Bits(b"hello".to_vec())]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(b"5d41402abc4b2a76b9719d911017c592".to_vec()));
    }

    #[test]
    fn test_encoding_sha1() {
        let result = encoding_sha1_impl(vec![Value::Bits(b"hello".to_vec())]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(b"aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_vec()));
    }

    #[test]
    fn test_encoding_sha256() {
        let result = encoding_sha256_impl(vec![Value::Bits(b"hello".to_vec())]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(b"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_vec()));
    }

    #[test]
    fn test_encoding_sha512() {
        let result = encoding_sha512_impl(vec![Value::Bits(b"hello".to_vec())]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(b"9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043".to_vec()));
    }

    #[test]
    fn test_encoding_md5_data() {
        let result = encoding_md5_impl(vec![Value::Bits(vec![104, 101, 108, 108, 111])]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(b"5d41402abc4b2a76b9719d911017c592".to_vec()));
    }

    #[test]
    fn test_encoding_uuid_v4() {
        let result = encoding_uuid_v4_impl(vec![]);
        assert!(result.is_ok());
        match result.unwrap() {
            Value::Bits(b) => {
                let uuid = String::from_utf8_lossy(&b);
                assert_eq!(uuid.len(), 36, "UUID should be 36 chars");
                assert_eq!(uuid.chars().nth(8), Some('-'));
                assert_eq!(uuid.chars().nth(14), Some('4'), "UUID v4 should have version nibble 4");
            }
            _ => panic!("Expected String"),
        }
    }

    #[test]
    fn test_encoding_uuid_v4_errors_with_args() {
        let result = encoding_uuid_v4_impl(vec![Value::Bits(b"bad".to_vec())]);
        assert!(result.is_err());
    }

    #[test]
    fn test_encoding_md5_no_args_errors() {
        let result = encoding_md5_impl(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_encoding_sha256_passthrough_non_string() {
        let result = encoding_sha256_impl(vec![Value::Bits(crate::interpreter::i64_to_bits(42))]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(crate::interpreter::i64_to_bits(42)));
    }
}
