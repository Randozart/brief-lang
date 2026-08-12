pub mod encoding;
pub mod io;
pub mod json;
pub mod strings;

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
pub struct FunctionRegistry {
    functions: HashMap<String, ForeignFn>,
    syscall_numbers: HashMap<String, HashMap<String, i64>>,
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
        let binding_dir = std::env::var("BRIEV_STDLIB_PATH")
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

    pub fn load_from_bindings_dir(&mut self) {
        let bindings_dir = Self::bindings_dir();
        let mut loaded_count = 0;
        if let Ok(entries) = std::fs::read_dir(&bindings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str());
                if ext == Some("dbv") {
                    if let Err(e) = self.load_from_dbv(&path) {
                        eprintln!("[WARN] Failed to load DBV binding {}: {}", path.display(), e);
                    } else {
                        loaded_count += 1;
                    }
                } else if ext == Some("toml") {
                    eprintln!("[WARN] TOML bindings are deprecated, use .dbv: {}", path.display());
                    if let Err(e) = self.load_from_toml(&path) {
                        eprintln!("[WARN] Failed to load legacy TOML binding {}: {}", path.display(), e);
                    }
                }
            }
        }
        eprintln!("[INFO] FFI Registry loaded {} functions from {} binding files",
            self.functions.len(), loaded_count);
    }

    fn load_from_dbv(&mut self, path: &std::path::Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let doc = crate::dbriev::v2::parse_document(&content)
            .map_err(|e| format!("Failed to parse DBV: {}", e))?;
        for group in &doc.data_groups {
            if group.schema_name.as_deref() != Some("FnBinding") {
                continue;
            }
            for entry in &group.entries {
                let name = match entry.fields.first() {
                    Some(crate::dbriev::v2::DataField::Positional(crate::dbriev::v2::DataValue::String(s))) => s.clone(),
                    _ => continue,
                };
                let impl_location = match entry.fields.get(1) {
                    Some(crate::dbriev::v2::DataField::Positional(crate::dbriev::v2::DataValue::String(s))) => s.clone(),
                    _ => continue,
                };
                self.fn_locations_by_name.insert(name.clone(), impl_location.clone());
                if let Some(func) = resolve_location_to_impl(&impl_location) {
                    self.register(impl_location.clone(), func);
                } else {
                    eprintln!("[WARN] No implementation for location '{}' in {}", impl_location, path.display());
                }
            }
        }
        Ok(())
    }



    pub fn get_location_by_name(&self, name: &str) -> Option<&str> {
        self.fn_locations_by_name.get(name).map(|s| s.as_str())
    }

    fn load_from_toml(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bindings = loader::load_binding(path).map_err(|e| format!("Failed to parse TOML: {}", e))?;
        for binding in bindings {
            let loc_str = binding.from.as_str();
            if let Some(func) = resolve_location_to_impl(&loc_str) {
                self.register(loc_str.clone(), func);
            } else {
                eprintln!("[WARN] No implementation for location '{}' in {}", loc_str, path.display());
            }
        }
        Ok(())
    }

    fn bindings_dir() -> PathBuf {
        let exe_path = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));
        let relative_path = exe_dir.join("std/bindings");
        if relative_path.exists() { relative_path }
        else { PathBuf::from("std/bindings") }
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

// Re-export for test access
pub use io::dbvl_append_impl;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_loading() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();
        assert!(registry.contains("std::io::println"), "println not loaded");
        assert!(registry.contains("std::f64::sqrt"), "sqrt not loaded");
    }

    #[test]
    fn test_metro_bridge_name_resolution() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();
        assert!(registry.get_location_by_name("__shm_open").is_some(), "__shm_open not registered");
        assert!(registry.get_location_by_name("__mmap_anonymous").is_some(), "__mmap_anonymous not registered");
        assert!(registry.get_location_by_name("__atomic_cas_u32").is_some(), "__atomic_cas_u32 not registered");
        assert!(registry.get_location_by_name("__metro_create_channel").is_some(), "__metro_create_channel not registered");
        assert_eq!(registry.get_location_by_name("__shm_open"), Some("metro::shm::open"));
        assert_eq!(registry.get_location_by_name("__atomic_fence"), Some("metro::atomic::fence"));
    }

    #[test]
    fn test_registry_basics() {
        let registry = FunctionRegistry::new();
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_location_resolution() {
        assert!(resolve_location_to_impl("std::f64::sqrt").is_some());
        assert!(resolve_location_to_impl("std::io::println").is_some());
        assert!(resolve_location_to_impl("unknown::function").is_none());
    }

    #[test]
    fn test_metro_bridge_impls_exist() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();
        let metro_locations = [
            "metro::shm::open", "metro::shm::unlink", "metro::shm::ftruncate",
            "metro::shm::list", "metro::shm::exists", "metro::shm::size",
            "metro::mmap::anonymous", "metro::mmap::unmap", "metro::mmap::sync",
            "metro::mmap::write", "metro::mmap::read", "metro::mmap::read_u32",
            "metro::mmap::write_u32", "metro::atomic::load_u32", "metro::atomic::store_u32",
            "metro::atomic::cas_u32", "metro::atomic::fence", "metro::atomic::xchg_u32",
            "metro::atomic::add_u32", "metro::channel::create", "metro::channel::destroy",
            "metro::channel::get_layout", "metro::channel::gen_c_header",
        ];
        for loc in &metro_locations {
            assert!(registry.contains(loc), "Metro impl missing: {}", loc);
        }
    }

    #[test]
    fn test_new_binding_impls_exist() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();
        let new_locations = [
            "encoding::base64_encode", "encoding::base64_decode",
            "encoding::hex_encode", "encoding::hex_decode",
            "encoding::url_encode", "encoding::url_decode",
            "encoding::html_escape", "encoding::html_unescape",
            "encoding::md5", "encoding::sha1", "encoding::sha256",
            "encoding::sha512", "encoding::uuid_v4",
            "json::parse", "json::stringify",
            "json::is_object", "json::is_array", "json::is_string",
            "json::is_number", "json::is_bool", "json::is_null",
            "json::get", "json::set", "json::keys", "json::length",
            "http::get", "http::post",
        ];
        for loc in &new_locations {
            assert!(registry.contains(loc), "New binding impl missing: {}", loc);
        }
    }
}

/// Resolve a binding location string to an actual function implementation.
fn resolve_location_to_impl(location: &str) -> Option<ForeignFn> {
    let func: fn(Vec<Value>) -> Result<Value, RuntimeError> = match location {
        "std::io::print" => io::print_impl,
        "std::io::println" => io::println_impl,
        "std::io::input" => io::input_impl,
        "std::dbvl::append" => io::dbvl_append_impl,

        "std::f64::sqrt" => io::sqrt_impl,
        "std::f64::powf" => io::pow_impl,
        "std::f64::sin" => io::sin_impl,
        "std::f64::cos" => io::cos_impl,
        "std::f64::abs" => io::abs_impl,
        "std::f64::floor" => io::floor_impl,
        "std::f64::ceil" => io::ceil_impl,
        "std::f64::round" => io::round_impl,

        "std::string::String::len" => strings::len_impl,
        "std::string::String::push_str" => strings::concat_impl,
        "std::string::String::trim" => strings::trim_impl,
        "std::string::String::contains" => strings::contains_impl,
        "std::string::String::to_lowercase" => strings::to_lower_impl,
        "std::string::String::to_uppercase" => strings::to_upper_impl,
        "std::string::String::replace" => strings::replace_impl,
        "std::string::String::chars" => strings::chars_impl,
        "std::string::String::starts_with" => strings::starts_with_impl,
        "std::string::String::ends_with" => strings::ends_with_impl,
        "std::str::FromStr::from_str" => strings::from_str_impl,
        "std::string::ToString::to_string" => strings::to_string_impl,

        "std::time::SystemTime::now" => io::now_impl,

        "std::fs::read_to_string" => io::read_file_impl,
        "std::fs::write" => io::write_file_impl,
        "std::fs::remove_file" => io::delete_file_impl,
        "std::fs::create_dir" => io::create_dir_impl,
        "std::fs::remove_dir" => io::delete_dir_impl,

        "metro::shm::open" => io::metro_shm_open_impl,
        "metro::shm::unlink" => io::metro_shm_unlink_impl,
        "metro::shm::ftruncate" => io::metro_ftruncate_impl,
        "metro::shm::list" => io::metro_shm_list_impl,
        "metro::shm::exists" => io::metro_shm_exists_impl,
        "metro::shm::size" => io::metro_shm_size_impl,

        "metro::mmap::anonymous" => io::metro_mmap_anonymous_impl,
        "metro::mmap::unmap" => io::metro_munmap_impl,
        "metro::mmap::sync" => io::metro_msync_impl,
        "metro::mmap::write" => io::metro_mmap_write_impl,
        "metro::mmap::read" => io::metro_mmap_read_impl,
        "metro::mmap::read_u32" => io::metro_mmap_read_u32_impl,
        "metro::mmap::write_u32" => io::metro_mmap_write_u32_impl,

        "metro::atomic::load_u32" => io::metro_atomic_load_u32_impl,
        "metro::atomic::store_u32" => io::metro_atomic_store_u32_impl,
        "metro::atomic::cas_u32" => io::metro_atomic_cas_u32_impl,
        "metro::atomic::fence" => io::metro_atomic_fence_impl,
        "metro::atomic::xchg_u32" => io::metro_atomic_xchg_u32_impl,
        "metro::atomic::add_u32" => io::metro_atomic_add_u32_impl,

        "metro::channel::create" => io::metro_channel_create_impl,
        "metro::channel::destroy" => io::metro_channel_destroy_impl,
        "metro::channel::get_layout" => io::metro_channel_get_layout_impl,
        "metro::channel::gen_c_header" => io::metro_channel_gen_c_header_impl,

        "encoding::base64_encode" => encoding::encoding_base64_encode_impl,
        "encoding::base64_decode" => encoding::encoding_base64_decode_impl,
        "encoding::hex_encode" => encoding::encoding_hex_encode_impl,
        "encoding::hex_decode" => encoding::encoding_hex_decode_impl,
        "encoding::url_encode" => encoding::encoding_url_encode_impl,
        "encoding::url_decode" => encoding::encoding_url_decode_impl,
        "encoding::html_escape" => encoding::encoding_html_escape_impl,
        "encoding::html_unescape" => encoding::encoding_html_unescape_impl,
        "encoding::md5" => encoding::encoding_md5_impl,
        "encoding::sha1" => encoding::encoding_sha1_impl,
        "encoding::sha256" => encoding::encoding_sha256_impl,
        "encoding::sha512" => encoding::encoding_sha512_impl,
        "encoding::uuid_v4" => encoding::encoding_uuid_v4_impl,

        "std::tty::raw_mode" => io::tty_raw_mode_impl,
        "std::tty::size" => io::tty_size_impl,
        "std::tty::read_key" => io::tty_read_key_impl,
        "std::process::exec" => io::exec_cmd_impl,

        "std::string::trim" => strings::string_trim_impl,
        "std::string::to_lower" => strings::string_to_lower_impl,
        "std::string::contains" => strings::string_contains_impl,
        "std::string::starts_with" => strings::string_starts_with_impl,
        "std::string::split" => strings::string_split_impl,
        "std::string::substring" => strings::substring_impl,
        "std::convert::int_to_string" => strings::int_to_string_impl,

        "std::json::parse" => json::json_parse_impl,
        "std::json::is_array" => json::json_is_array_impl,
        "std::json::length" => json::json_length_impl,
        "std::json::get" => json::json_get_impl,
        "std::json::get_by_index" => json::json_get_by_index_impl,

        "json::parse" => json::json_parse_impl,
        "json::stringify" => json::json_stringify_impl,
        "json::is_object" => json::json_is_object_impl,
        "json::is_array" => json::json_is_array_impl,
        "json::is_string" => json::json_is_string_impl,
        "json::is_number" => json::json_is_number_impl,
        "json::is_bool" => json::json_is_bool_impl,
        "json::is_null" => json::json_is_null_impl,
        "json::get" => json::json_get_impl,
        "json::set" => json::json_set_impl,
        "json::keys" => json::json_keys_impl,
        "json::length" => json::json_length_impl,

        "http::get" => io::http_get_impl,
        "http::post" => io::http_post_impl,

        _ => {
            eprintln!("[DEBUG] Unresolved location: {}", location);
            return None;
        }
    };
    Some(func)
}
