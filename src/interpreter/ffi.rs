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

// ── FFI Helper Functions ─────────────────────────────────────────────
//
// This submodule owns the foreign-function interface implementations
// that back Brief's stdlib. Each `*_impl` function is a thin
// wrapper over a Rust standard-library operation, converting between
// `Value::Bits` and native types.
//
// Extracted from the monolithic interpreter/mod.rs during Phase 4.
// Every function follows max 2 nesting with guard clauses.
//
// 2026-07-12: All `if let` chains flattened to guard clauses.
// All functions use the pattern: extract bytes → operate → return Bits.

use super::intrinsics::{bits_to_f64, bits_to_i64, f64_to_bits, i64_to_bits, value_as_i64};
use super::{ForeignFn, RuntimeError, Value};
use crate::ffi::FFI_REGISTRY;
use std::collections::HashMap;
use std::io::Read;

// ── FFI Registry Loading ─────────────────────────────────────────────
// These were static associated functions on Interpreter. They don't
// need self, so they're free functions here.

/// Look up a TOML binding by name in a `.toml` file.
/// Returns the bound location string.
pub(super) fn lookup_location_from_toml(name: &str, toml_path: &str) -> Result<String, String> {
    let path = std::path::Path::new(toml_path);
    let bindings = crate::ffi::loader::load_binding(path)
        .map_err(|e| format!("Failed to load TOML: {}", e))?;

    // Search for a binding whose name matches.
    for binding in bindings {
        if binding.name == name {
            return Ok(binding.location);
        }
    }

    Err(format!("Binding '{}' not found in '{}'", name, toml_path))
}

/// Load all registered FFI functions into a name→ForeignFn map.
/// The registry is a global static populated by each backend.
pub(super) fn load_ffi_functions() -> HashMap<String, ForeignFn> {
    let mut functions = HashMap::new();
    let registry = &*FFI_REGISTRY;
    for (location, func) in registry.iter() {
        functions.insert(location.clone(), *func);
    }
    functions
}

// ── TTY Helpers ──────────────────────────────────────────────────────

/// Set terminal raw mode (Unix only). No-op on other platforms.
pub(super) fn set_tty_raw_mode(enable: bool) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::io::RawFd;
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        let ok = unsafe { libc::tcgetattr(0, &mut termios) == 0 };
        if !ok {
            return false;
        }
        if enable {
            let mut raw = termios;
            unsafe {
                libc::cfmakeraw(&mut raw);
            }
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) == 0 }
        } else {
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &termios) == 0 }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = enable;
        false
    }
}

/// Get terminal size (columns, rows). Defaults to 80×24 on failure.
pub(super) fn get_terminal_size() -> (i64, i64) {
    #[cfg(unix)]
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        let ok = libc::ioctl(1, libc::TIOCGWINSZ, &mut ws as *mut _ as *mut libc::c_void) == 0
            && ws.ws_col > 0;
        if ok {
            return (ws.ws_col as i64, ws.ws_row as i64);
        }
    }
    (80, 24)
}

/// Read one keypress in non-blocking mode. Returns None if no key available.
pub(super) fn read_key_nonblocking() -> Option<u8> {
    #[cfg(unix)]
    unsafe {
        let flags = libc::fcntl(0, libc::F_GETFL, 0);
        if flags >= 0 {
            libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    let mut buf = [0u8; 1];
    let result = match std::io::stdin().read(&mut buf) {
        Ok(n) if n > 0 => Some(buf[0]),
        _ => None,
    };
    #[cfg(unix)]
    unsafe {
        let flags = libc::fcntl(0, libc::F_GETFL, 0);
        if flags >= 0 {
            libc::fcntl(0, libc::F_SETFL, flags & !libc::O_NONBLOCK);
        }
    }
    result
}

// ── JSON Helper ──────────────────────────────────────────────────────

/// Convert a serde_json::Value to a Brief Value.
/// Null → Bits("null"), Bool → Bits([0/1]), Number → Bits(i64),
/// String → Bits(bytes), Array → List, Object → List of [key, val] pairs.
fn json_value_to_brief(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Bits(b"null".to_vec()),
        serde_json::Value::Bool(b) => Value::Bits(vec![if b { 1u8 } else { 0u8 }]),
        serde_json::Value::Number(n) => Value::Bits(i64_to_bits(n.as_i64().unwrap_or(0))),
        serde_json::Value::String(s) => Value::Bits(s.into_bytes()),
        serde_json::Value::Array(arr) => {
            Value::List(arr.into_iter().map(json_value_to_brief).collect())
        }
        serde_json::Value::Object(obj) => {
            let pairs: Vec<Value> = obj
                .into_iter()
                .map(|(k, v)| {
                    Value::List(vec![Value::Bits(k.into_bytes()), json_value_to_brief(v)])
                })
                .collect();
            Value::List(pairs)
        }
    }
}

// ── Print / Input ────────────────────────────────────────────────────

/// Print a string to stdout. Returns true (success sentinel).
pub(crate) fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("print expects String".into())),
    };
    print!("{}", String::from_utf8_lossy(s));
    Ok(Value::Bits(vec![1u8]))
}

/// Print a string to stdout with a newline. Returns true.
pub(crate) fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("println expects String".into())),
    };
    println!("{}", String::from_utf8_lossy(s));
    Ok(Value::Bits(vec![1u8]))
}

/// Read a line from stdin. Returns the line as a String value.
pub(crate) fn input_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::io::{self, BufRead};
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    line.pop();
    Ok(Value::Bits(line.into_bytes()))
}

/// Enable or disable terminal raw mode. Placeholder — always succeeds.
pub(crate) fn tty_raw_mode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let enable_bits = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("tty_raw_mode expects Bool".into())),
    };
    let _enable = enable_bits.first().copied().unwrap_or(0) != 0;
    Ok(Value::Bits(vec![1u8]))
}

/// Return the terminal size as (columns * 10000 + rows).
pub(crate) fn tty_size_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    let encoded: i64 = 80 * 10000 + 24;
    Ok(Value::Bits(i64_to_bits(encoded)))
}

/// Read one keypress. Returns the key as a String.
pub(crate) fn tty_read_key_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    let byte = read_key_nonblocking();
    let s = match byte {
        Some(b) => String::from(b as char),
        None => String::new(),
    };
    Ok(Value::Bits(s.into_bytes()))
}

// ── String Operations ────────────────────────────────────────────────

/// Trim whitespace from a string.
pub(crate) fn string_trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("string_trim expects String".into())),
    };
    Ok(Value::Bits(
        String::from_utf8_lossy(s).trim().to_string().into_bytes(),
    ))
}

/// Convert a string to lowercase.
pub(crate) fn string_to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "string_to_lower expects String".into(),
            ))
        }
    };
    Ok(Value::Bits(
        String::from_utf8_lossy(s).to_lowercase().into_bytes(),
    ))
}

/// Check if a string contains a substring.
pub(crate) fn string_contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "string_contains expects String, String".into(),
            ))
        }
    };
    let sub_bytes = match &args[1] {
        Value::Bits(b) => b,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "string_contains expects String, String".into(),
            ))
        }
    };
    let s = String::from_utf8_lossy(s_bytes);
    let sub = String::from_utf8_lossy(sub_bytes);
    Ok(Value::Bits(vec![if s.contains(&*sub) {
        1u8
    } else {
        0u8
    }]))
}

/// Check if a string starts with a prefix.
pub(crate) fn string_starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "string_starts_with expects String, String".into(),
            ))
        }
    };
    let prefix_bytes = match &args[1] {
        Value::Bits(b) => b,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "string_starts_with expects String, String".into(),
            ))
        }
    };
    let s = String::from_utf8_lossy(s_bytes);
    let prefix = String::from_utf8_lossy(prefix_bytes);
    Ok(Value::Bits(vec![if s.starts_with(&*prefix) {
        1u8
    } else {
        0u8
    }]))
}

/// Split a string on whitespace into a List of substrings.
pub(crate) fn string_split_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b).to_string(),
        _ => return Err(RuntimeError::TypeMismatch("string_split expects String".into())),
    };
    let parts: Vec<Value> = s
        .split(char::is_whitespace)
        .filter(|p| !p.is_empty())
        .map(|p| Value::Bits(p.to_string().into()))
        .collect();
    Ok(Value::List(parts))
}

/// Extract the first character of a string as a list of char values.
pub(crate) fn substring_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b).to_string(),
        _ => return Err(RuntimeError::TypeMismatch("substring expects String".into())),
    };
    let chars: Vec<Value> = s
        .chars()
        .map(|c| Value::Bits(i64_to_bits(c as i64)))
        .collect();
    Ok(Value::List(chars))
}

/// Convert an integer to its string representation.
pub(crate) fn int_to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("int_to_string expects Int".into())),
    };
    let n = bits_to_i64(&v).unwrap_or(0);
    Ok(Value::Bits(n.to_string().into_bytes()))
}

/// Measure the byte length of a Bits value.
pub(crate) fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("len expects String".into())),
    };
    Ok(Value::Bits(i64_to_bits(s.len() as i64)))
}

/// Concatenate two strings as `[a][b]`.
pub(crate) fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let a = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b).to_string(),
        _ => return Err(RuntimeError::TypeMismatch("concat expects String".into())),
    };
    let b = match &args[1] {
        Value::Bits(bits) => String::from_utf8_lossy(bits).to_string(),
        _ => return Err(RuntimeError::TypeMismatch("concat expects String".into())),
    };
    Ok(Value::Bits(format!("[{}{}]", a, b).into_bytes()))
}

/// Get the first character of a string.
pub(crate) fn chars_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("chars expects String".into())),
    };
    let s = String::from_utf8_lossy(s_bytes);
    let first_char: String = s.chars().take(1).collect();
    Ok(Value::Bits(first_char.into_bytes()))
}

/// Replace all occurrences of a substring.
pub(crate) fn replace_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("replace expects String".into())),
    };
    let from_bytes = match &args[1] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("replace expects String".into())),
    };
    let to_bytes = match &args[2] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("replace expects String".into())),
    };
    let s = String::from_utf8_lossy(s_bytes).to_string();
    let from = String::from_utf8_lossy(from_bytes).to_string();
    let to = String::from_utf8_lossy(to_bytes).to_string();
    Ok(Value::Bits(s.replace(&from, &to).into_bytes()))
}

// ── JSON ─────────────────────────────────────────────────────────────

/// Parse a JSON string into a Brief Value tree.
pub(crate) fn json_parse_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b),
        _ => return Err(RuntimeError::TypeMismatch("json_parse expects String".into())),
    };
    match serde_json::from_str::<serde_json::Value>(&s) {
        Ok(v) => Ok(json_value_to_brief(v)),
        Err(e) => Ok(Value::Bits(
            format!("[JSON parse error: {}]", e).into_bytes(),
        )),
    }
}

/// Check if a value is a JSON array (List).
pub(crate) fn json_is_array_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let is_array = matches!(&args[0], Value::List(_));
    Ok(Value::Bits(vec![if is_array { 1u8 } else { 0u8 }]))
}

/// Get the length of a list or map.
pub(crate) fn json_length_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let len = match &args[0] {
        Value::List(items) => items.len() as i64,
        Value::Instance { fields, .. } => fields.len() as i64,
        Value::HashMap(map) => map.len() as i64,
        Value::Bits(b) => b.len() as i64,
        _ => 0,
    };
    Ok(Value::Bits(i64_to_bits(len)))
}

/// Get a value by key from a key-value list.
pub(crate) fn json_get_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let obj = match &args[0] {
        Value::List(items) => items,
        _ => return Err(RuntimeError::TypeMismatch("json_get expects Value, String".into())),
    };
    let key_bits = match &args[1] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("json_get expects Value, String".into())),
    };

    // Search for a pair with matching key.
    for pair in obj {
        let kv = match pair {
            Value::List(kv) => kv,
            _ => continue,
        };
        if kv.len() == 2 && kv[0] == Value::Bits(key_bits.clone()) {
            return Ok(kv[1].clone());
        }
    }
    Ok(Value::Bits(Vec::new()))
}

/// Get a value by index from a list.
pub(crate) fn json_get_by_index_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let items = match &args[0] {
        Value::List(items) => items,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "json_get_by_index expects Value, Int".into(),
            ))
        }
    };
    let idx_bits = match &args[1] {
        Value::Bits(b) => b,
        _ => {
            return Err(RuntimeError::TypeMismatch(
                "json_get_by_index expects Value, Int".into(),
            ))
        }
    };
    let idx = bits_to_i64(&Value::Bits(idx_bits.clone())).unwrap_or(0) as usize;
    if idx < items.len() {
        Ok(items[idx].clone())
    } else {
        Ok(Value::Bits(Vec::new()))
    }
}

// ── Math ─────────────────────────────────────────────────────────────

/// Absolute value of an integer.
pub(crate) fn abs_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("abs expects Int".into())),
    };
    let n = bits_to_i64(&v).unwrap_or(0);
    Ok(Value::Bits(i64_to_bits(n.abs())))
}

/// Square root of a float.
pub(crate) fn sqrt_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("sqrt expects Float or Int".into())),
    };
    let n = bits_to_f64(&v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n.sqrt())))
}

/// Power: base raised to exponent.
pub(crate) fn pow_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let base_v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("pow expects Float".into())),
    };
    let exp_v = match &args[1] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("pow expects Float".into())),
    };
    let base = bits_to_f64(&base_v).unwrap_or(0.0);
    let exp = bits_to_f64(&exp_v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(base.powf(exp))))
}

/// Sine of a float.
pub(crate) fn sin_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("sin expects Float".into())),
    };
    let n = bits_to_f64(&v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n.sin())))
}

/// Cosine of a float.
pub(crate) fn cos_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("cos expects Float".into())),
    };
    let n = bits_to_f64(&v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n.cos())))
}

/// Floor of a float.
pub(crate) fn floor_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("floor expects Float".into())),
    };
    let n = bits_to_f64(&v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n.floor())))
}

/// Ceiling of a float.
pub(crate) fn ceil_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("ceil expects Float".into())),
    };
    let n = bits_to_f64(&v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n.ceil())))
}

/// Round a float to nearest integer.
pub(crate) fn round_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = match &args[0] {
        Value::Bits(b) => Value::Bits(b.clone()),
        _ => return Err(RuntimeError::TypeMismatch("round expects Float".into())),
    };
    let n = bits_to_f64(&v).unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n.round())))
}

/// Generate a pseudo-random float in [0, 1) using subsecond nanos.
pub(crate) fn random_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    Ok(Value::Bits(f64_to_bits((nanos as f64) / (u32::MAX as f64))))
}

// ── Type Conversion ──────────────────────────────────────────────────

/// Convert a value to its string representation.
pub(crate) fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let v = &args[0];
    let b = match v {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("to_string expects Int or Float".into())),
    };
    let s = bits_to_i64(v)
        .map(|n| n.to_string())
        .or_else(|_| bits_to_f64(v).map(|f| f.to_string()))
        .unwrap_or_else(|_| String::from_utf8_lossy(b).to_string());
    Ok(Value::Bits(s.into_bytes()))
}

/// Parse a string as a float.
pub(crate) fn to_float_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b),
        _ => return Err(RuntimeError::TypeMismatch("to_float expects String".into())),
    };
    let n = s.parse::<f64>().unwrap_or(0.0);
    Ok(Value::Bits(f64_to_bits(n)))
}

/// Parse a string as an integer.
pub(crate) fn to_int_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b),
        _ => return Err(RuntimeError::TypeMismatch("to_int expects String".into())),
    };
    let n = s.parse::<i64>().unwrap_or(0);
    Ok(Value::Bits(i64_to_bits(n)))
}

/// Trim whitespace (alias for string_trim).
pub(crate) fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("trim expects String".into())),
    };
    Ok(Value::Bits(
        String::from_utf8_lossy(s).trim().to_string().into_bytes(),
    ))
}

/// Check if a string contains a substring (alias for string_contains).
pub(crate) fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let haystack = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b),
        _ => return Err(RuntimeError::TypeMismatch("contains expects String".into())),
    };
    let needle = match &args[1] {
        Value::Bits(b) => String::from_utf8_lossy(b),
        _ => return Err(RuntimeError::TypeMismatch("contains expects String".into())),
    };
    Ok(Value::Bits(vec![if haystack.contains(needle.as_ref()) {
        1u8
    } else {
        0u8
    }]))
}

/// Convert a string to lowercase (alias for string_to_lower).
pub(crate) fn to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("to_lower expects String".into())),
    };
    Ok(Value::Bits(
        String::from_utf8_lossy(s).to_lowercase().into_bytes(),
    ))
}

/// Convert a string to uppercase.
pub(crate) fn to_upper_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("to_upper expects String".into())),
    };
    Ok(Value::Bits(
        String::from_utf8_lossy(s).to_uppercase().into_bytes(),
    ))
}

/// Check if a string starts with a prefix (alias).
pub(crate) fn starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("starts_with expects String".into())),
    };
    let prefix_bytes = match &args[1] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("starts_with expects String".into())),
    };
    let s = String::from_utf8_lossy(s_bytes);
    let prefix = String::from_utf8_lossy(prefix_bytes);
    Ok(Value::Bits(vec![if s.starts_with(&*prefix) {
        1u8
    } else {
        0u8
    }]))
}

/// Check if a string ends with a suffix.
pub(crate) fn ends_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("ends_with expects String".into())),
    };
    let suffix_bytes = match &args[1] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("ends_with expects String".into())),
    };
    let s = String::from_utf8_lossy(s_bytes);
    let suffix = String::from_utf8_lossy(suffix_bytes);
    Ok(Value::Bits(vec![if s.ends_with(&*suffix) {
        1u8
    } else {
        0u8
    }]))
}

/// Parse a string as an integer (alias for to_int).
pub(crate) fn from_str_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::Bits(b) => String::from_utf8_lossy(b),
        _ => return Err(RuntimeError::TypeMismatch("from_str expects String".into())),
    };
    let n = s.parse::<i64>().unwrap_or(0);
    Ok(Value::Bits(i64_to_bits(n)))
}

// ── Time / Exec / File I/O ───────────────────────────────────────────

/// Get the current time in milliseconds since Unix epoch.
pub(crate) fn now_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => Ok(Value::Bits(i64_to_bits(d.as_millis() as i64))),
        Err(_) => Ok(Value::Bits(i64_to_bits(0))),
    }
}

/// Execute a shell command and return stdout.
pub(crate) fn exec_cmd_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let cmd_bytes = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("exec_cmd expects String".into())),
    };
    let cmd_str = String::from_utf8_lossy(cmd_bytes).to_string();
    match std::process::Command::new("sh").arg("-c").arg(&cmd_str).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(Value::Bits(stdout.into_bytes()))
        }
        Err(e) => Err(RuntimeError::TypeMismatch(format!("exec failed: {}", e))),
    }
}

/// Read a file and return its content as a Result enum.
pub(crate) fn read_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let p = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("read_file expects String".into())),
    };
    let path = String::from_utf8_lossy(p).to_string();
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(Value::Enum(
            "Result".into(),
            "Ok".into(),
            HashMap::from([("value".into(), Value::Bits(content.into_bytes()))]),
        )),
        Err(e) => Ok(Value::Enum(
            "Result".into(),
            "Err".into(),
            HashMap::from([("value".into(), Value::Bits(format!("{}", e).into_bytes()))]),
        )),
    }
}

/// Write content to a file.
pub(crate) fn write_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let p = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("write_file expects String".into())),
    };
    let c = match &args[1] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("write_file expects String".into())),
    };
    let path = String::from_utf8_lossy(p).to_string();
    let content = String::from_utf8_lossy(c).to_string();
    match std::fs::write(&path, &content) {
        Ok(_) => Ok(Value::Bits(b"OK".to_vec())),
        Err(e) => Ok(Value::Bits(format!("Error: {}", e).into_bytes())),
    }
}

/// Delete a file.
pub(crate) fn delete_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let p = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("delete_file expects String".into())),
    };
    let path = String::from_utf8_lossy(p).to_string();
    match std::fs::remove_file(&path) {
        Ok(_) => Ok(Value::Bits(b"OK".to_vec())),
        Err(e) => Ok(Value::Bits(format!("[Error: {}]", e).into_bytes())),
    }
}

/// Create a directory.
pub(crate) fn create_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let p = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("create_dir expects String".into())),
    };
    let path = String::from_utf8_lossy(p).to_string();
    match std::fs::create_dir(&path) {
        Ok(_) => Ok(Value::Bits(b"OK".to_vec())),
        Err(e) => Ok(Value::Bits(format!("[Error: {}]", e).into_bytes())),
    }
}

/// Delete a directory.
pub(crate) fn delete_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let p = match &args[0] {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("delete_dir expects String".into())),
    };
    let path = String::from_utf8_lossy(p).to_string();
    match std::fs::remove_dir(&path) {
        Ok(_) => Ok(Value::Bits(b"OK".to_vec())),
        Err(e) => Ok(Value::Bits(format!("[Error: {}]", e).into_bytes())),
    }
}
