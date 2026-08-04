// ── Compiler-in-Brief: the needs_state pass loaded through the GLUE C ABI ─
// 2026-08-04 (plan 2026-08-04-compiler-in-brief-dogfood-ffi, P3): the Brief
// pass lib/compiler/needs_state.bv is compiled by briefc (build.rs) into
// target/compiler-in-brief/needs_state.so and loaded HERE via dlopen — the
// same way a host language calls a Brief bridge. `compute_export_needs_state`
// (src/analysis/export_abi.rs) is the Rust reference; this module returns its
// answer when the pass library is present, else falls back to the reference
// (first build / no prebuilt briefc). The transition test
// (tests/c_driver_needs_state.rs) asserts the two agree on the bridge corpus.

use std::ffi::{c_char, c_int, c_void, CString};

use once_cell::sync::OnceCell;

use crate::ast::TopLevel;

/// The dlopen handle + resolved symbols for the Brief needs_state pass.
struct BriefPass {
    _handle: *mut c_void,
    /// `needs_state_compute(state, proj) -> i64` — bit i = export[i] (sorted
    /// by name) needs `ptr %state`.
    compute: unsafe extern "C" fn(state: *const c_void, proj: *const c_char) -> i64,
    /// `__brief_init_state() -> i64` — the pass is stateful by ABI (it calls
    /// regular defns), so the driver supplies the runtime state handle.
    init_state: unsafe extern "C" fn() -> i64,
}

// The pass functions are pure over the projection string; the handle is a
// shared immutable library. The fn pointers are thread-safe to call.
unsafe impl Send for BriefPass {}
unsafe impl Sync for BriefPass {}

const RTLD_NOW: c_int = 2;

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

impl BriefPass {
    /// Load the pass library from `BRIEF_COMPILER_IN_BRIEF_SO` (set by
    /// build.rs via cargo:rustc-env, read here at COMPILE time so the path is
    /// embedded — a runtime env lookup would miss it). None = not built (first
    /// build) or not loadable — the caller falls back to the Rust reference.
    fn load() -> Option<BriefPass> {
        let path = option_env!("BRIEF_COMPILER_IN_BRIEF_SO")?.to_string();
        if path.is_empty() {
            return None;
        }
        let c_path = CString::new(path).ok()?;
        // SAFETY: dlopen with a NUL-terminated path, RTLD_NOW.
        let handle = unsafe { dlopen(c_path.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            return None;
        }
        let compute = unsafe {
            dlsym(handle, b"needs_state_compute\0".as_ptr() as *const c_char)
        };
        let init_state = unsafe {
            dlsym(handle, b"__brief_init_state\0".as_ptr() as *const c_char)
        };
        if compute.is_null() || init_state.is_null() {
            return None;
        }
        // SAFETY: the symbols were resolved to the declared C signatures.
        Some(BriefPass {
            _handle: handle,
            compute: unsafe { std::mem::transmute(compute) },
            init_state: unsafe { std::mem::transmute(init_state) },
        })
    }

    /// Run the pass over a serialized projection and return the bitmask.
    fn compute(&self, proj: &str) -> i64 {
        let c_proj = match CString::new(proj) {
            Ok(c) => c,
            Err(_) => return 0,
        };
        // SAFETY: the pass functions are pure; init_state gives the ABI state
        // handle (the pass never dereferences it), compute reads the C string.
        let state = unsafe { (self.init_state)() };
        unsafe { (self.compute)(state as *const c_void, c_proj.as_ptr()) }
    }
}

static BRIEF_PASS: OnceCell<Option<BriefPass>> = OnceCell::new();

fn brief_pass() -> Option<&'static BriefPass> {
    BRIEF_PASS.get_or_init(BriefPass::load).as_ref()
}

/// Compute the needs_state map for a program, preferring the Brief pass when
/// its library is present. Falls back to the Rust reference (export_abi.rs).
/// The two must agree — asserted by tests/c_driver_needs_state.rs.
pub fn compute_export_needs_state(items: &[TopLevel]) -> std::collections::HashMap<String, bool> {
    if let Some(pass) = brief_pass() {
        let proj = crate::analysis::needs_state_projection::serialize_needs_state_projection(items);
        let mask = pass.compute(&proj);
        // Reconstruct the map in the SAME order the serializer emitted the
        // export section (sorted by defn name): bit i ↔ export[i].
        let mut exports: Vec<String> = Vec::new();
        for item in items {
            if let TopLevel::Export(e) = item {
                if let TopLevel::Definition(d) = e.inner.as_ref() {
                    exports.push(d.name.clone());
                }
            }
        }
        exports.sort();
        let mut map = std::collections::HashMap::new();
        for (i, name) in exports.into_iter().enumerate() {
            map.insert(name, (mask >> i) & 1 == 1);
        }
        return map;
    }
    // Fallback: the Rust reference (also the source of truth on first build).
    crate::analysis::export_abi::compute_export_needs_state(items)
}

/// Whether the compiled Brief pass library is available to load (the build.rs
/// produced it). Exposed for tests/diagnostics.
pub fn pass_available() -> bool {
    brief_pass().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dlopen path must actually run (not silently fall back). Builds the
    /// bridges, runs BOTH the Brief pass and the Rust reference, and requires
    /// equality — and that the Brief path was used when the .so was produced.
    #[test]
    fn brief_pass_matches_reference_when_loaded() {
        let root = env!("CARGO_MANIFEST_DIR");
        let corpus = [
            "examples/glue-host/boundary.bv",
            "examples/glue-host/node_bridge.bv",
            "examples/glue-host/cancel.bv",
            "examples/glue-host/rank.bv",
            "examples/glue-host/bench.bv",
        ];
        let mut used_brief = false;
        for rel in corpus {
            let src = format!("{}/{}", root, rel);
            let source = std::fs::read_to_string(&src).unwrap();
            let (items, _) = crate::library::parse_and_check(&src, &source).unwrap();
            let brief = compute_export_needs_state(&items);
            let reference = crate::analysis::export_abi::compute_export_needs_state(&items);
            assert_eq!(brief, reference, "Brief pass diverged from reference for {}", rel);
            if brief_pass().is_some() {
                used_brief = true;
            }
        }
        if option_env!("BRIEF_COMPILER_IN_BRIEF_SO").map_or(false, |p| !p.is_empty()) {
            assert!(used_brief, "pass library was built but never used");
        }
    }
}
