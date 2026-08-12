# Phase 2 Delivery: CBackend → TargetSpec Refactor

**Date:** 2026-05-01  
**Status:** Complete ✅

---

## Executive Summary

Replaced CBackend's boolean CLI flags (`bare_metal`, `kernel_mode`, `kernel_os`) with a declarative TOML-based `TargetSpec` system. This enables framework-specific code generation via configuration files instead of hardcoded Rust logic.

---

## Changes Made

### 1. CBackend Refactor (`src/backend/c.rs`)

| Before | After |
|--------|-------|
| `bare_metal: bool` | `spec: Option<TargetSpec>` |
| `kernel_mode: bool` | `spec.codegen.entry_point.style` |
| `kernel_os: Option<String>` | In spec templates |
| `.bare_metal(true)` | `.with_spec(spec)` |
| `.with_kernel_mode(Some("linux"))` | `.with_spec(load("linux_kernel.toml"))` |

**Key changes:**
- New field: `spec: Option<crate::target_spec::TargetSpec>`
- New builder method: `pub fn with_spec(mut self, spec: TargetSpec) -> Self`
- Removed methods: `.bare_metal()`, `.with_kernel_mode()`
- `generate()` now reads from `spec.codegen`:
  - `entry_point.style` → `"module_init"`, `"bare_metal"`, `"main"`
  - `state_allocation` → `"static"` or `"dynamic"`
  - `templates.header` / `templates.footer`

### 2. TargetSpecLoader Fix (`src/target_spec/loader.rs`)

- Added current directory check before search paths
- Enables relative paths like `--target lib/targets/linux_kernel.toml`

### 3. Main.rs Integration (`src/main.rs`)

```rust
// Load target spec if specified
if let Some(t) = target {
    let spec_path = std::path::Path::new(t);
    let loader = target_spec::loader::TargetSpecLoader::new();
    match loader.load(spec_path) {
        Ok(spec) => c_backend = c_backend.with_spec(spec),
        Err(e) => eprintln!("Warning: failed to load target spec '{}': {}", t, e),
    }
}
```

### 4. Example Target Spec (`lib/targets/linux_kernel.toml`)

```toml
[codegen]
backend = "c"
extension = "c"
state_allocation = "static"

[codegen.entry_point]
style = "module_init"
module_name = "briev_module"

[codegen.templates]
header = """
#include <linux/module.h>
#include <linux/kernel.h>
...
"""
```

---

## Verification

### Tests: 109 passed ✅

```
$ cargo test --lib
test result: ok. 109 passed; 0 failed; 0 ignored
```

### End-to-End Verification

**Default (hosted) target:**
```
$ ./target/release/briev-compiler c test.bv
/* Target: Hosted (Desktop/Embedded Linux) */
#include <stdlib.h>
static State *state = NULL;  // dynamic allocation
int main(void) { ... }
```

**Kernel module target:**
```
$ ./target/release/briev-compiler c test.bv --target lib/targets/linux_kernel.toml
/* Target: module_init */
#include <linux/module.h>
static State state_instance;  // static allocation
module_init(briev_init);
MODULE_LICENSE("GPL");
```

---

## Remaining Work (Phase 3+)

- Add target specs for other backends: CUDA, WebGPU, React, Python, Swift/Kotlin
- Add `#[target]` attribute for inline overrides
- Update CLAUDE.md with new workflow documentation

---

## Files Changed

| File | Change |
|------|--------|
| `src/backend/c.rs` | Refactor to use TargetSpec |
| `src/main.rs` | Wire TargetSpecLoader |
| `src/target_spec/loader.rs` | Fix current directory check |
| `src/target_spec/mod.rs` | Re-export TargetSpecLoader |
| `lib/targets/linux_kernel.toml` | Example target spec |
| `plans/active/UNIVERSAL_ADAPTER_SPEC.md` | Update status |

---

## Commit History

```
a2053ae Add target_spec module with TOML loader structure
6092fb1 Phase 2: Refactor CBackend to use TargetSpec
29a2278 Fix TargetSpecLoader to check current directory first
```

---

## Usage

```bash
# Compile with default (hosted) target
./briev-compiler c input.bv

# Compile with kernel module target
./briev-compiler c input.bv --target lib/targets/linux_kernel.toml

# Compile with bare-metal target (future)
./briev-compiler c input.bv --target lib/targets/arm_el1.toml
```