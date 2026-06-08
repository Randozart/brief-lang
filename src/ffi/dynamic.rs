use std::collections::HashMap;
use std::ffi::{CStr, CString};
use libloading::{Library, Symbol};
use crate::interpreter::{Value, RuntimeError};

/// Type of a parameter or return value in a frgn declaration
#[derive(Debug, Clone, PartialEq)]
pub enum FrgnType {
    Int,
    Float,
    Bool,
    Char,
    String,
    Void,
}

/// Parsed frgn declaration: frgn name { params } -> Ret [from "lib.so"]
#[derive(Debug, Clone)]
pub struct FrgnDecl {
    pub name: String,
    pub params: Vec<(String, FrgnType)>,
    pub ret: FrgnType,
    pub lib: String,
}

impl FrgnType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Int" => Some(FrgnType::Int),
            "Float" => Some(FrgnType::Float),
            "Bool" => Some(FrgnType::Bool),
            "Char" => Some(FrgnType::Char),
            "String" => Some(FrgnType::String),
            "Void" => Some(FrgnType::Void),
            _ => None,
        }
    }
}

/// Wrap a raw value into a Result::Ok for compatibility with frgn's Result<T,E> return.
fn wrap_ok(ret: &FrgnType, value: Value) -> Value {
    let key = match ret {
        FrgnType::String => "result".to_string(),
        _ => "result".to_string(),
    };
    let mut fields = std::collections::HashMap::new();
    fields.insert("value".to_string(), value);
    Value::Enum("Result".to_string(), "Ok".to_string(), fields)
}

fn wrap_err(msg: String) -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("error".to_string(), Value::String(msg));
    Value::Enum("Result".to_string(), "Err".to_string(), fields)
}

/// Resolve and call a foreign function by name using the given library.
/// Converts Brief Values to/from C ABI for each supported signature pattern.
pub fn call_foreign_by_name(
    lib: &Library,
    name: &str,
    params: &[(String, FrgnType)],
    ret: &FrgnType,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    // Convert args to C values
    let mut c_strings: Vec<CString> = Vec::new();
    let mut c_ints: Vec<i64> = Vec::new();
    let mut c_doubles: Vec<f64> = Vec::new();

    for (i, (_, param_type)) in params.iter().enumerate() {
        let val = if i < args.len() { &args[i] } else { &Value::Void };
        match param_type {
            FrgnType::Int => {
                c_ints.push(match val { Value::Int(n) => *n, _ => 0 });
            }
            FrgnType::Float => {
                c_doubles.push(match val { Value::Float(f) => *f, _ => 0.0 });
            }
            FrgnType::Bool => {
                c_ints.push(match val { Value::Bool(b) => *b, _ => false } as i64);
            }
            FrgnType::Char => {
                c_ints.push(match val { Value::Char(c) => *c, _ => '\0' } as i64);
            }
            FrgnType::String => {
                let s = match val { Value::String(s) => s.clone(), _ => String::new() };
                c_strings.push(CString::new(s).unwrap_or_else(|_| CString::new("").unwrap()));
            }
            FrgnType::Void => {}
        }
    }

    let name_bytes = name.as_bytes();

    // Dispatch by parameter count and types
    match (params.len(), ret) {
        // 0 args, void return
        (0, FrgnType::Void) => {
            let f: Symbol<unsafe extern "C" fn()> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            unsafe { f() };
            Ok(Value::Void)
        }

        // 0 args, Int/Bool/Char return
        (0, FrgnType::Int | FrgnType::Bool | FrgnType::Char) => {
            let f: Symbol<unsafe extern "C" fn() -> i64> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            let raw = unsafe { f() };
            Ok(match ret {
                FrgnType::Int => Value::Int(raw),
                FrgnType::Bool => Value::Bool(raw != 0),
                FrgnType::Char => Value::Char(char::from_u32(raw as u32).unwrap_or('\0')),
                _ => unreachable!(),
            })
        }

        // 0 args, Float return
        (0, FrgnType::Float) => {
            let f: Symbol<unsafe extern "C" fn() -> f64> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            Ok(Value::Float(unsafe { f() }))
        }

        // 0 args, String return
        (0, FrgnType::String) => {
            let f: Symbol<unsafe extern "C" fn() -> *const libc::c_char> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            let ptr = unsafe { f() };
            if ptr.is_null() {
                Ok(Value::String(String::new()))
            } else {
                Ok(Value::String(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()))
            }
        }

        // 1 arg Int → Int/Bool/Char
        (1, FrgnType::Int | FrgnType::Bool | FrgnType::Char) if c_ints.len() == 1 => {
            let f: Symbol<unsafe extern "C" fn(i64) -> i64> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            let raw = unsafe { f(c_ints[0]) };
            Ok(match ret {
                FrgnType::Int => Value::Int(raw),
                FrgnType::Bool => Value::Bool(raw != 0),
                FrgnType::Char => Value::Char(char::from_u32(raw as u32).unwrap_or('\0')),
                _ => unreachable!(),
            })
        }

        // 1 arg Float → Float
        (1, FrgnType::Float) if c_doubles.len() == 1 => {
            let f: Symbol<unsafe extern "C" fn(f64) -> f64> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            Ok(Value::Float(unsafe { f(c_doubles[0]) }))
        }

        // 1 arg String → Int (strlen etc)
        (1, FrgnType::Int) if c_strings.len() == 1 => {
            let f: Symbol<unsafe extern "C" fn(*const libc::c_char) -> i64> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            let ptr = c_strings[0].as_ptr();
            Ok(Value::Int(unsafe { f(ptr) }))
        }

        // 1 arg Void
        (1, FrgnType::Void) if c_ints.len() == 1 => {
            let f: Symbol<unsafe extern "C" fn(i64)> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            unsafe { f(c_ints[0]) };
            Ok(Value::Void)
        }

        // 2 args (Int, Int) → Int/Bool
        (2, FrgnType::Int | FrgnType::Bool) if c_ints.len() >= 2 => {
            let f: Symbol<unsafe extern "C" fn(i64, i64) -> i64> = unsafe { lib.get(name_bytes) }
                .map_err(|e| RuntimeError::TypeMismatch(format!("'{}' not found: {}", name, e)))?;
            let raw = unsafe { f(c_ints[0], c_ints[1]) };
            Ok(match ret {
                FrgnType::Int => Value::Int(raw),
                FrgnType::Bool => Value::Bool(raw != 0),
                _ => unreachable!(),
            })
        }

        _ => Err(RuntimeError::TypeMismatch(
            format!("Unsupported FFI signature for '{}' ({} args, {:?} ret)",
                name, params.len(), ret)
        )),
    }
}

/// Registry of parsed frgn declarations, populated at load_program time.
#[derive(Default)]
pub struct FrgnRegistry {
    pub declarations: HashMap<String, FrgnDecl>,
    pub libraries: HashMap<String, Library>,
}

impl FrgnRegistry {
    pub fn new() -> Self {
        Self {
            declarations: HashMap::new(),
            libraries: HashMap::new(),
        }
    }

    pub fn register(&mut self, decl: FrgnDecl) {
        self.declarations.insert(decl.name.clone(), decl);
    }

    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        let decl = self.declarations.get(name)
            .ok_or_else(|| RuntimeError::TypeMismatch(
                format!("Unknown foreign function: '{}'. Is it declared with frgn?", name)
            ))?;

        if !self.libraries.contains_key(&decl.lib) {
            let lib = unsafe { Library::new(&decl.lib) }
                .map_err(|e| RuntimeError::TypeMismatch(
                    format!("Failed to load '{}' for '{}': {}", decl.lib, name, e)
                ))?;
            self.libraries.insert(decl.lib.clone(), lib);
        }

        let lib = self.libraries.get(&decl.lib).unwrap();
        let raw = call_foreign_by_name(lib, &decl.name, &decl.params, &decl.ret, args)?;

        // Wrap in Result::Ok to match the frgn's Result<T,E> return type
        let mut fields = std::collections::HashMap::new();
        fields.insert("value".to_string(), raw);
        Ok(Value::Enum("Result".to_string(), "Ok".to_string(), fields))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frgn_type_from_name_valid() {
        assert_eq!(FrgnType::from_name("Int"), Some(FrgnType::Int));
        assert_eq!(FrgnType::from_name("Float"), Some(FrgnType::Float));
        assert_eq!(FrgnType::from_name("Bool"), Some(FrgnType::Bool));
        assert_eq!(FrgnType::from_name("Char"), Some(FrgnType::Char));
        assert_eq!(FrgnType::from_name("String"), Some(FrgnType::String));
        assert_eq!(FrgnType::from_name("Void"), Some(FrgnType::Void));
    }

    #[test]
    fn test_frgn_type_from_name_invalid() {
        assert_eq!(FrgnType::from_name("Invalid"), None);
        assert_eq!(FrgnType::from_name(""), None);
    }

    #[test]
    fn test_wrap_ok_creates_result_enum() {
        let result = wrap_ok(&FrgnType::Int, Value::Int(42));
        match result {
            Value::Enum(_, variant, fields) => {
                assert_eq!(variant, "Ok");
                assert_eq!(fields.get("value"), Some(&Value::Int(42)));
            }
            _ => panic!("Expected Enum(Result, Ok, ...)"),
        }
    }

    #[test]
    fn test_wrap_err_creates_result_enum() {
        let result = wrap_err("something failed".to_string());
        match result {
            Value::Enum(_, variant, fields) => {
                assert_eq!(variant, "Err");
                assert_eq!(fields.get("error"), Some(&Value::String("something failed".into())));
            }
            _ => panic!("Expected Enum(Result, Err, ...)"),
        }
    }

    #[test]
    fn test_frgn_registry_register_and_unknown() {
        let mut registry = FrgnRegistry::new();
        registry.register(FrgnDecl {
            name: "foo".into(),
            params: vec![],
            ret: FrgnType::Void,
            lib: "invalid.so".into(),
        });
        let result = registry.call("nonexistent", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_frgn_registry_unknown_function() {
        let mut registry = FrgnRegistry::new();
        let result = registry.call("unknown_fn", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_frgn_decl_display() {
        let decl = FrgnDecl {
            name: "my_fn".into(),
            params: vec![("x".into(), FrgnType::Int)],
            ret: FrgnType::Bool,
            lib: "lib.so".into(),
        };
        let debug = format!("{:?}", decl);
        assert!(debug.contains("my_fn"));
    }

    #[test]
    fn test_frgn_type_equality_and_clone() {
        let t = FrgnType::Float;
        assert_eq!(t.clone(), t);
        assert_ne!(FrgnType::Int, FrgnType::Float);
    }
}