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

//! FFI Function Registry
//!
//! Manages runtime registration of foreign function implementations.
//! TOML-driven: loads bindings from std/bindings/*.toml and maps locations to implementations.

use crate::ffi::loader;
use crate::interpreter::{ForeignFn, RuntimeError, Value};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;

/// Global FFI function registry
pub static FFI_REGISTRY: Lazy<FunctionRegistry> = Lazy::new(|| {
    let mut registry = FunctionRegistry::new();
    registry.load_from_bindings_dir();
    registry
});

/// Function implementation registry
/// Maps function locations (from TOML location field) to implementations
pub struct FunctionRegistry {
    functions: HashMap<String, ForeignFn>,
    syscall_numbers: HashMap<String, HashMap<String, i64>>,
    /// Reverse mapping from register name (e.g. "__shm_open") to location (e.g. "metro::shm::open")
    /// Used by profile-based frgn declarations that don't use `from "..."` clauses
    fn_locations_by_name: HashMap<String, String>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        FunctionRegistry {
            functions: HashMap::new(),
            syscall_numbers: HashMap::new(),
            fn_locations_by_name: HashMap::new(),
        }
    }

    pub fn register_syscall_numbers(&mut self, target: String, numbers: HashMap<String, i64>) {
        self.syscall_numbers.insert(target, numbers);
    }

    pub fn get_syscall_number(&self, target: &str, name: &str) -> Option<i64> {
        self.syscall_numbers.get(target).and_then(|m| m.get(name).copied())
    }

    pub fn load_syscall_bindings(&mut self) -> Result<(), String> {
        let binding_dir = std::env::var("BRIEF_STDLIB_PATH")
            .map(|p| PathBuf::from(p).join("syscalls"))
            .unwrap_or_else(|_| PathBuf::from("std/bindings/syscalls"));

        if binding_dir.exists() {
            for entry in std::fs::read_dir(binding_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "toml") {
                    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    let toml: toml::Value = toml::from_str(&content).map_err(|e| e.to_string())?;
                    if let Some(syscalls) = toml.get("syscalls").and_then(|v| v.as_array()) {
                        for syscall in syscalls {
                            if let Some(name) = syscall.get("name").and_then(|v| v.as_str()) {
                                let mut numbers = HashMap::new();
                                if let Some(num_map) = syscall.get("syscall_num").and_then(|v| v.as_table()) {
                                    for (target, num) in num_map {
                                        if let Some(n) = num.as_integer() {
                                            numbers.insert(target.clone(), n);
                                        }
                                    }
                                }
                                self.syscall_numbers.insert(name.to_string(), numbers);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn register(&mut self, location: String, func: ForeignFn) {
        self.functions.insert(location, func);
    }

    pub fn get(&self, location: &str) -> Option<ForeignFn> {
        self.functions.get(location).copied()
    }

    pub fn contains(&self, location: &str) -> bool {
        self.functions.contains_key(location)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ForeignFn)> {
        self.functions.iter()
    }

    /// Load all bindings from std/bindings/*.dbvs (Metropolitan FFI)
    pub fn load_from_bindings_dir(&mut self) {
        let bindings_dir = Self::bindings_dir();
        let mut dbvs_count = 0;
        let mut metro_channels = 0;

        if let Ok(entries) = std::fs::read_dir(&bindings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str());
                
                if ext == Some("dbvs") {
                    if let Err(e) = self.load_from_dbvs(&path) {
                        eprintln!("[WARN] Failed to load DBVS binding {}: {}", path.display(), e);
                    } else {
                        dbvs_count += 1;
                    }
                } else if ext == Some("toml") {
                    // TOML files are deprecated - log warning but still load for backward compat
                    eprintln!("[WARN] TOML bindings are deprecated, use .dbvs: {}", path.display());
                    if let Err(e) = self.load_from_toml(&path) {
                        eprintln!("[WARN] Failed to load legacy TOML binding {}: {}", path.display(), e);
                    }
                }
            }
        }

        eprintln!(
            "[INFO] FFI Registry loaded {} functions from {} DBVS schemas (Metropolitan FFI)",
            self.functions.len(),
            dbvs_count
        );
    }

    /// Load bindings from a single DBVS schema file
    fn load_from_dbvs(&mut self, path: &std::path::Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        let program = crate::dbrief::parse_dbvs(&content)
            .map_err(|e| format!("Failed to parse DBVS: {}", e))?;
        
        for register in &program.registers {
            // Skip registers without a name (not FFI bindings)
            let name = register.name.as_ref()
                .ok_or_else(|| "Register missing 'as' name".to_string())?;
            
            // Skip registers without a location (not FFI bindings)
            let location = register.location.as_ref()
                .ok_or_else(|| format!("Register '{}' missing 'location' field", name))?;
            
            // Build reverse name→location map for profile-based frgn resolution
            self.fn_locations_by_name.insert(name.clone(), location.clone());
            
            // Register the function implementation
            if let Some(func) = resolve_location_to_impl(location) {
                self.register(location.clone(), func);
            } else {
                eprintln!(
                    "[WARN] No implementation for location '{}' in {}",
                    location,
                    path.display()
                );
            }
        }
        
        Ok(())
    }

    /// Look up a function's location by its name (e.g. "__shm_open" → "metro::shm::open")
    /// Used for profile-based frgn declarations that don't have `from "..."` clauses
    pub fn get_location_by_name(&self, name: &str) -> Option<&str> {
        self.fn_locations_by_name.get(name).map(|s| s.as_str())
    }

    /// Load bindings from a single TOML file
    fn load_from_toml(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bindings =
            loader::load_binding(path).map_err(|e| format!("Failed to parse TOML: {}", e))?;

        for binding in bindings {
            if let Some(func) = resolve_location_to_impl(&binding.location) {
                self.register(binding.location, func);
            } else {
                eprintln!(
                    "[WARN] No implementation for location '{}' in {}",
                    binding.location,
                    path.display()
                );
            }
        }

        Ok(())
    }

    /// Get the bindings directory path
    fn bindings_dir() -> PathBuf {
        let exe_path = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));

        // Try relative to executable first, then crate root
        let relative_path = exe_dir.join("std/bindings");
        if relative_path.exists() {
            return relative_path;
        }

        // Fallback to crate root (for development)
        std::path::PathBuf::from("std/bindings")
    }

    pub fn register_from_binding(&mut self, location: &str, func: ForeignFn) {
        self.register(location.to_string(), func);
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.load_from_bindings_dir();
        registry
    }
}

/// Resolve a TOML location string to an actual function implementation
fn resolve_location_to_impl(location: &str) -> Option<ForeignFn> {
    let func: fn(Vec<Value>) -> Result<Value, RuntimeError> = match location {
        // IO functions
        "std::io::print" => print_impl,
        "std::io::println" => println_impl,
        "std::io::input" => input_impl,

        // Math functions
        "std::f64::sqrt" => sqrt_impl,
        "std::f64::powf" => pow_impl,
        "std::f64::sin" => sin_impl,
        "std::f64::cos" => cos_impl,
        "std::f64::abs" => abs_impl,
        "std::f64::floor" => floor_impl,
        "std::f64::ceil" => ceil_impl,
        "std::f64::round" => round_impl,

        // String functions
        "std::string::String::len" => len_impl,
        "std::string::String::push_str" => concat_impl,
        "std::string::String::trim" => trim_impl,
        "std::string::String::contains" => contains_impl,
        "std::string::String::to_lowercase" => to_lower_impl,
        "std::string::String::to_uppercase" => to_upper_impl,
        "std::string::String::replace" => replace_impl,
        "std::string::String::chars" => chars_impl,
        "std::string::String::starts_with" => starts_with_impl,
        "std::string::String::ends_with" => ends_with_impl,
        "std::str::FromStr::from_str" => from_str_impl,
        "std::string::ToString::to_string" => to_string_impl,

        // Time functions
        "std::time::SystemTime::now" => now_impl,

        // File system (simplified - these return void on success)
        "std::fs::read_to_string" => read_file_impl,
        "std::fs::write" => write_file_impl,
        "std::fs::remove_file" => delete_file_impl,
        "std::fs::create_dir" => create_dir_impl,
        "std::fs::remove_dir" => delete_dir_impl,

        // Metropolitan FFI - SHM operations
        "metro::shm::open" => metro_shm_open_impl,
        "metro::shm::unlink" => metro_shm_unlink_impl,
        "metro::shm::ftruncate" => metro_ftruncate_impl,
        "metro::shm::list" => metro_shm_list_impl,
        "metro::shm::exists" => metro_shm_exists_impl,
        "metro::shm::size" => metro_shm_size_impl,

        // Metropolitan FFI - MMAP operations
        "metro::mmap::anonymous" => metro_mmap_anonymous_impl,
        "metro::mmap::unmap" => metro_munmap_impl,
        "metro::mmap::sync" => metro_msync_impl,
        "metro::mmap::write" => metro_mmap_write_impl,
        "metro::mmap::read" => metro_mmap_read_impl,
        "metro::mmap::read_u32" => metro_mmap_read_u32_impl,
        "metro::mmap::write_u32" => metro_mmap_write_u32_impl,

        // Metropolitan FFI - Atomic operations
        "metro::atomic::load_u32" => metro_atomic_load_u32_impl,
        "metro::atomic::store_u32" => metro_atomic_store_u32_impl,
        "metro::atomic::cas_u32" => metro_atomic_cas_u32_impl,
        "metro::atomic::fence" => metro_atomic_fence_impl,
        "metro::atomic::xchg_u32" => metro_atomic_xchg_u32_impl,
        "metro::atomic::add_u32" => metro_atomic_add_u32_impl,

        // Metropolitan FFI - Channel operations
        "metro::channel::create" => metro_channel_create_impl,
        "metro::channel::destroy" => metro_channel_destroy_impl,
        "metro::channel::get_layout" => metro_channel_get_layout_impl,
        "metro::channel::gen_c_header" => metro_channel_gen_c_header_impl,

        _ => {
            eprintln!("[DEBUG] Unresolved location: {}", location);
            return None;
        }
    };
    Some(func)
}

// Re-export implementations from interpreter
use crate::interpreter;
use std::sync::atomic::{self, Ordering};

// IO implementations
fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::print_impl(args)
}
fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::println_impl(args)
}
fn input_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::input_impl(args)
}

// Math implementations
fn abs_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::abs_impl(args)
}
fn sqrt_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::sqrt_impl(args)
}
fn pow_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::pow_impl(args)
}
fn sin_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::sin_impl(args)
}
fn cos_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::cos_impl(args)
}
fn floor_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::floor_impl(args)
}
fn ceil_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::ceil_impl(args)
}
fn round_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::round_impl(args)
}

// String implementations
fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::len_impl(args)
}
fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::concat_impl(args)
}
fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::trim_impl(args)
}
fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::contains_impl(args)
}
fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_string_impl(args)
}
fn to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_lower_impl(args)
}
fn to_upper_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_upper_impl(args)
}
fn replace_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::replace_impl(args)
}
fn chars_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::chars_impl(args)
}
fn starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::starts_with_impl(args)
}
fn ends_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::ends_with_impl(args)
}
fn from_str_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::from_str_impl(args)
}

// Time implementation
fn now_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::now_impl(args)
}

// File system implementations
fn read_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::read_file_impl(args)
}
fn write_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::write_file_impl(args)
}
fn delete_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::delete_file_impl(args)
}
fn create_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::create_dir_impl(args)
}
fn delete_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::delete_dir_impl(args)
}

// ===== Metropolitan FFI Implementations =====

/// Concrete address from a Value (Int or ptr-cast)
fn value_to_ptr_offset(args: &[Value], idx: usize) -> Result<*mut u8, RuntimeError> {
    match &args[idx] {
        Value::Int(addr) => Ok(*addr as *mut u8),
        _ => Err(RuntimeError::TypeMismatch(format!("arg {} expected Int (address)", idx))),
    }
}

fn value_to_i32(args: &[Value], idx: usize) -> Result<i32, RuntimeError> {
    match &args[idx] {
        Value::Int(n) => Ok(*n as i32),
        _ => Err(RuntimeError::TypeMismatch(format!("arg {} expected Int", idx))),
    }
}

fn value_to_usize(args: &[Value], idx: usize) -> Result<usize, RuntimeError> {
    match &args[idx] {
        Value::Int(n) => Ok(*n as usize),
        _ => Err(RuntimeError::TypeMismatch(format!("arg {} expected Int", idx))),
    }
}

fn value_to_string(args: &[Value], idx: usize) -> Result<String, RuntimeError> {
    match &args[idx] {
        Value::String(s) => Ok(s.clone()),
        _ => Err(RuntimeError::TypeMismatch(format!("arg {} expected String", idx))),
    }
}

fn metro_shm_open_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    let flags = value_to_i32(&args, 1)?;
    let mode = value_to_i32(&args, 2)?;
    unsafe {
        let fd = libc::shm_open(
            name.as_ptr() as *const i8,
            flags,
            mode as libc::mode_t,
        );
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String(err.to_string()));
                m
            }))
        } else {
            Ok(Value::Int(fd as i64))
        }
    }
}

fn metro_shm_unlink_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    unsafe {
        let ret = libc::shm_unlink(name.as_ptr() as *const i8);
        if ret == 0 {
            Ok(Value::Void)
        } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String(err.to_string()));
                m
            }))
        }
    }
}

fn metro_ftruncate_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let fd = value_to_i32(&args, 0)?;
    let length = value_to_i64(&args, 1)?;
    unsafe {
        let ret = libc::ftruncate(fd, length);
        if ret == 0 {
            Ok(Value::Void)
        } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String(err.to_string()));
                m
            }))
        }
    }
}

fn metro_shm_list_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    // POSIX has no standard shm_list. Check /dev/shm as a heuristic.
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/shm") {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                names.push(Value::String(name));
            }
        }
    }
    Ok(Value::List(names))
}

fn metro_shm_exists_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    let path = format!("/dev/shm/{}", name.trim_start_matches('/'));
    Ok(Value::Bool(std::path::Path::new(&path).exists()))
}

fn metro_shm_size_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let name = value_to_string(&args, 0)?;
    let name_c = std::ffi::CString::new(name.clone()).map_err(|_| {
        RuntimeError::TypeMismatch("Invalid SHM name".to_string())
    })?;
    unsafe {
        let fd = libc::shm_open(name_c.as_ptr(), libc::O_RDONLY, 0);
        if fd < 0 {
            return Ok(Value::Enum("ShmError".to_string(), "ShmNotFound".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String("Not found".to_string()));
                m
            }));
        }
        let mut stat: libc::stat = std::mem::zeroed();
        let ret = libc::fstat(fd, &mut stat);
        libc::close(fd);
        if ret == 0 {
            Ok(Value::Int(stat.st_size as i64))
        } else {
            Ok(Value::Enum("ShmError".to_string(), "ShmOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String("fstat failed".to_string()));
                m
            }))
        }
    }
}

fn metro_mmap_anonymous_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let length = value_to_usize(&args, 0)?;
    let prot = value_to_i32(&args, 1)?;
    let flags = value_to_i32(&args, 2)?;
    unsafe {
        let addr = libc::mmap(
            std::ptr::null_mut(),
            length,
            prot,
            flags,
            -1,
            0,
        );
        if addr == libc::MAP_FAILED {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("MmapError".to_string(), "MmapOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String(err.to_string()));
                m
            }))
        } else {
            Ok(Value::Int(addr as i64))
        }
    }
}

fn metro_munmap_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let length = value_to_usize(&args, 1)?;
    unsafe {
        let ret = libc::munmap(addr as *mut libc::c_void, length);
        if ret == 0 {
            Ok(Value::Void)
        } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("MmapError".to_string(), "MmapOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String(err.to_string()));
                m
            }))
        }
    }
}

fn metro_msync_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let length = value_to_usize(&args, 1)?;
    let flags = value_to_i32(&args, 2)?;
    unsafe {
        let ret = libc::msync(addr as *mut libc::c_void, length, flags);
        if ret == 0 {
            Ok(Value::Void)
        } else {
            let err = std::io::Error::last_os_error();
            Ok(Value::Enum("MmapError".to_string(), "MmapOther".to_string(), {
                let mut m = std::collections::HashMap::new();
                m.insert("message".to_string(), Value::String(err.to_string()));
                m
            }))
        }
    }
}

fn metro_mmap_write_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let data = match &args[2] {
        Value::List(items) => items,
        _ => return Err(RuntimeError::TypeMismatch("arg 2 expected List<Int>".to_string())),
    };
    let _len = value_to_usize(&args, 3)?;
    let target = unsafe { addr.add(offset) };
    for (i, item) in data.iter().enumerate() {
        let byte = match item {
            Value::Int(n) => *n as u8,
            _ => return Err(RuntimeError::TypeMismatch("list items must be Int".to_string())),
        };
        unsafe { *target.add(i) = byte; }
    }
    Ok(Value::Void)
}

fn metro_mmap_read_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let length = value_to_usize(&args, 2)?;
    let source = unsafe { addr.add(offset) };
    let mut result = Vec::with_capacity(length);
    for i in 0..length {
        unsafe { result.push(Value::Int(*source.add(i) as i64)); }
    }
    Ok(Value::List(result))
}

fn metro_mmap_read_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    unsafe {
        let ptr = addr.add(offset) as *const u32;
        let val = std::ptr::read_unaligned(ptr);
        Ok(Value::Int(val as i64))
    }
}

fn metro_mmap_write_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let ptr = addr.add(offset) as *mut u32;
        std::ptr::write_unaligned(ptr, value);
    }
    Ok(Value::Void)
}

use std::sync::atomic::AtomicU32;

fn metro_atomic_load_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let val = atomic_ref.load(Ordering::SeqCst);
        Ok(Value::Int(val as i64))
    }
}

fn metro_atomic_store_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        atomic_ref.store(value, Ordering::SeqCst);
    }
    Ok(Value::Void)
}

fn metro_atomic_cas_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let expected = value_to_u32(&args, 2)?;
    let new_value = value_to_u32(&args, 3)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let prev = atomic_ref.compare_exchange(
            expected,
            new_value,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        Ok(Value::Int(prev.unwrap_or(expected) as i64))
    }
}

fn metro_atomic_fence_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    atomic::fence(Ordering::SeqCst);
    Ok(Value::Void)
}

fn metro_atomic_xchg_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let prev = atomic_ref.swap(value, Ordering::SeqCst);
        Ok(Value::Int(prev as i64))
    }
}

fn metro_atomic_add_u32_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let addr = value_to_ptr_offset(&args, 0)?;
    let offset = value_to_usize(&args, 1)?;
    let value = value_to_u32(&args, 2)?;
    unsafe {
        let atomic_ref = &*(addr.add(offset) as *const AtomicU32);
        let prev = atomic_ref.fetch_add(value, Ordering::SeqCst);
        Ok(Value::Int(prev as i64))
    }
}

use crate::ffi::metropolitan::MetropolitanHub;
use std::sync::Arc;

static GLOBAL_METRO_HUB: once_cell::sync::Lazy<Arc<MetropolitanHub>> =
    once_cell::sync::Lazy::new(|| Arc::new(MetropolitanHub::new()));

fn metro_channel_create_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    let input_size = value_to_usize(&args, 1)?;
    let output_size = value_to_usize(&args, 2)?;
    match GLOBAL_METRO_HUB.create_channel(&channel_id, "native", input_size, output_size) {
        Ok(ch) => {
            let addrs = ch.get_addresses();
            let req_addr = *addrs.get("request").unwrap_or(&0);
            let resp_addr = *addrs.get("response").unwrap_or(&0);
            let sync_addr = *addrs.get("sync").unwrap_or(&0);
            let mut fields = std::collections::HashMap::new();
            fields.insert("request_addr".to_string(), Value::Int(req_addr as i64));
            fields.insert("response_addr".to_string(), Value::Int(resp_addr as i64));
            fields.insert("sync_addr".to_string(), Value::Int(sync_addr as i64));
            fields.insert("handle".to_string(), Value::Int(0));
            Ok(Value::Instance { typename: "MetroChannel".to_string(), fields })
        }
        Err(e) => Err(RuntimeError::UndefinedForeignFunction(e)),
    }
}

fn metro_channel_destroy_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    let _ = GLOBAL_METRO_HUB.close_channel(&channel_id);
    Ok(Value::Void)
}

fn metro_channel_get_layout_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    match GLOBAL_METRO_HUB.get_channel(&channel_id) {
        Some(ch) => {
            let addrs = ch.get_addresses();
            let mut fields = std::collections::HashMap::new();
            for (k, v) in addrs {
                fields.insert(k, Value::Int(v as i64));
            }
            Ok(Value::Instance { typename: "Layout".to_string(), fields })
        }
        None => Err(RuntimeError::UndefinedForeignFunction(
            format!("Channel not found: {}", channel_id)
        )),
    }
}

fn metro_channel_gen_c_header_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let channel_id = value_to_string(&args, 0)?;
    match GLOBAL_METRO_HUB.generate_c_header(&channel_id) {
        Ok(header) => Ok(Value::String(header)),
        Err(e) => Err(RuntimeError::UndefinedForeignFunction(e)),
    }
}

fn value_to_i64(args: &[Value], idx: usize) -> Result<i64, RuntimeError> {
    match &args[idx] {
        Value::Int(n) => Ok(*n),
        _ => Err(RuntimeError::TypeMismatch(format!("arg {} expected Int", idx))),
    }
}

fn value_to_u32(args: &[Value], idx: usize) -> Result<u32, RuntimeError> {
    match &args[idx] {
        Value::Int(n) => Ok(*n as u32),
        _ => Err(RuntimeError::TypeMismatch(format!("arg {} expected Int", idx))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basics() {
        let registry = FunctionRegistry::new();
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_toml_loading() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();

        // Should have loaded functions from TOML
        assert!(registry.contains("std::io::println"), "println not loaded");
        assert!(registry.contains("std::f64::sqrt"), "sqrt not loaded");
    }

    #[test]
    fn test_location_resolution() {
        assert!(resolve_location_to_impl("std::f64::sqrt").is_some());
        assert!(resolve_location_to_impl("std::io::println").is_some());
        assert!(resolve_location_to_impl("unknown::function").is_none());
    }

    #[test]
    fn test_metro_bridge_name_resolution() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();
        
        // Verify that all metro bridge functions resolve by name
        assert!(registry.get_location_by_name("__shm_open").is_some(), "__shm_open not registered");
        assert!(registry.get_location_by_name("__mmap_anonymous").is_some(), "__mmap_anonymous not registered");
        assert!(registry.get_location_by_name("__atomic_cas_u32").is_some(), "__atomic_cas_u32 not registered");
        assert!(registry.get_location_by_name("__metro_create_channel").is_some(), "__metro_create_channel not registered");
        
        // Verify the locations they resolve to
        assert_eq!(registry.get_location_by_name("__shm_open"), Some("metro::shm::open"));
        assert_eq!(registry.get_location_by_name("__atomic_fence"), Some("metro::atomic::fence"));
    }

    #[test]
    fn test_metro_bridge_impls_exist() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();
        
        // All metro locations should have implementations
        let metro_locations = [
            "metro::shm::open",
            "metro::shm::unlink",
            "metro::shm::ftruncate",
            "metro::shm::list",
            "metro::shm::exists",
            "metro::shm::size",
            "metro::mmap::anonymous",
            "metro::mmap::unmap",
            "metro::mmap::sync",
            "metro::mmap::write",
            "metro::mmap::read",
            "metro::mmap::read_u32",
            "metro::mmap::write_u32",
            "metro::atomic::load_u32",
            "metro::atomic::store_u32",
            "metro::atomic::cas_u32",
            "metro::atomic::fence",
            "metro::atomic::xchg_u32",
            "metro::atomic::add_u32",
            "metro::channel::create",
            "metro::channel::destroy",
            "metro::channel::get_layout",
            "metro::channel::gen_c_header",
        ];
        
        for loc in &metro_locations {
            assert!(registry.contains(loc), "Metro impl missing: {}", loc);
        }
    }
}
