# Briv Compiler: Universal Transpilation Adapter

**Date:** 2026-05-01  
**Status:** Phase 2 Complete (CBackend refactored)

## Executive Summary

This document defines the **Target Spec TOML** system for the Briv compiler. The goal is to transform Briv from a multi-flag compiler into a universal transpilation adapter where framework-specific conventions are defined in declarative TOML files rather than hardcoded Rust logic.

**Key insight:** The Metropolitan FFI profiles (`lib/ffi/profiles/`) already define 90% of the architecture. We extend them with a `[codegen]` block to create unified Target Spec files that drive both FFI call generation and code generation.

**Separation of concerns:**
- **Language Backend** (`c. rs`, `typescript. rs`) → Only generates syntax (translates Briv AST → target language)
- **Target Spec TOML** → Defines framework wrapping (headers, entry points, memory allocation, type mappings, templates)
- **Metropolitan FFI** → Defines memory layouts and type representations (used by both FFI and codegen)

---

## 1. Architecture

### 1.1 Current State

The compiler currently uses CLI flags to select targets:
```bash
briv compile --target linux_ kernel file. bv   # C backend + kernel headers
briv compile --target bare_ metal file. ebv  # C backend + ARM start
```

In `src/backend/c. rs`: hardcoded boolean flags drive 200+ lines of if/else branching.

### 1.2 Target State

With Target Spec TOML:
```bash
briv compile --target lib/ffi/profiles/ linux_ kernel. toml file. bv
```

CBackend becomes:
```rust
pub struct CBackend {
    spec: TargetSpec,
}
```

### 1.3 Three-Layer Design

```
CLI Layer
  briv compile --target <spec. toml>
         | loads
         v
Target Spec TOML
  [ffi]      - Type mappings & memory
  [codegen]  - Entry points & templates
  [validation] - Type restrictions
         | feeds into
         v
FFI Layer              Backend Layer
  orchestrator     c. rs, typescript. rs
  mapper         types -> syntax
```

---

## 2. Target Spec TOML Format

### 2.1 Unified Spec (Recommended for KV260, Linux kernel)

For targets needing both FFI and codegen:

```toml
# lib/ targets/ kv260_ baremetal. toml
[ffi]
name = "kv260_ baremetal"
description = "Xilinx Zynq UltraScale+ bare- metal ARM"

[ffi. language]
name = "C"
endianness = "little"
pointer_ size = 8

[ffi. types]
Int = { representation = "two's complement", size = 4, signed = true }
UInt = { representation = "unsigned", size = 4, signed = false }
Bool = { size = 4, representation = "int" }
Float = { representation = "error", message = "KV260 has no FPU - use fixed- point" }

[ffi. mapping]
Int = "int32_ t"
UInt = "uint32_ t"
Bool = "int"
Void = "void"

[ffi. conventions]
alignment = 8
call_ conv = "cdecl"
error. null_ pointer = 0
error. error_ pointer = -1

[ffi. overrides]
Int32Array = "int32_ t*"
Buffer = "uint8_ t*"

[codegen]
backend = "c"
extension = "c"
state_ allocation = "static"

[codegen. templates]
header = """
/* KV260 Bare- metal entry point - Zynq UltraScale+ EL2->EL1 */
#include <stdint. h>
#include <stdbool. h>
#define STACK_ TOP 0x0F800000
"""

footer = """
int main(void) {
    init_ wrapper();
    return 0;
}
"""

[codegen. entry_ point]
style = "arm_ el1_ start"
init_ txn = "init"
exit_ txn = "exit"

[codegen. validation]
Float = "error: KV260 has no FPU - use Type:: Custom(\"fixed\")"
```

### 2.2 FFI Profile Only (no codegen)

For language interop without code generation:

```toml
# lib/ffi/ profiles/ js_ ffi. toml
[ffi]
name = "javascript"
description = "JavaScript via WebAssembly"

[ffi. language]
name = "JavaScript"
endianness = "little"
pointer_ size = 8

[ffi. types]
Int = { representation = "two's complement", size = 4, signed = true }
Float = { representation = "IEEE754", size = 8 }

[ffi. mapping]
Int = "i32"
Float = "f64"
String = "string"
Bool = "boolean"
Data = "Uint8Array"

[ffi. conventions]
alignment = 8
call_ conv = "wasm"
```

### 2.3 Codegen Profile Only (for Reach blockchain)

For backends that don't need FFI:

```toml
# lib/ codegen/ reach. toml
[codegen]
backend = "typescript"
extension = "mjs"
state_ allocation = "reach_ participant"

[codegen. templates]
header = """
import * as stdlib from '@reach- sh/stdlib';
import * as backend from './build/ index. main. mjs';
export const startApp = async (interact) => {
"""

footer = """};
"""

[codegen. entry_ point]
style = "reach_ participant"
init_ txn = "init"
exit_ txn = "exit"

[codegen. validation]
Float = "error: Reach does not support floating- point types"
```

---

## 3. Float Type Safety

### 3.1 The Problem

In `src/backend/c. rs`, Float maps to `double`. For kernel modules:
- Kernel does not support FPU operations natively
- GCC generates FPU instructions (SSE/AVX on x86, NEON/VFP on ARM)
- Without kernel FPU save/restore, causes kernel panic or build failure

### 3.2 Solution: Compile-Time Validation

The `[codegen. validation]` section enforces type restrictions at compile time:

```toml
[codegen. validation]
Float = "error: Linux kernel does not support FPU - use fixed- point"
```

During type checking:
1. Parse Target Spec TOML
2. For each type in `[codegen. validation]`, if value starts with "error:", mark type as BLOCKED
3. In typechecker, for each `Float` type in program: emit `E3001` error if blocked

### 3.3 Valid Targets for Float

| Target | Float Support |
|--------|----------------|
| `linux_ kernel. toml` | BLOCKED (error) |
| `kv260_ baremetal. toml` | BLOCKED (error) |
| `wasm. toml` | f64 |
| `cobol. toml` | BINARY- DOUBLE |
| `hosted_ c. toml` | double |

---

### 3.5 Happy Path Inference

The "Happiest Path" is the compiler's ability to infer the most efficient target-specific
mapping based on the loaded `target.toml`, without requiring the programmer to decorate
every declaration.

**Priority Order:**
1. **Attribute Override** (`#[cuda.shared]`): Highest priority, forces specific behavior
2. **Profile Default** (`[inference]` section in TOML): Target-specific defaults
3. **Inferred Happy Path**: Compiler's best guess based on type/context

Example for CUDA:
- `let x: Vector<Int, 262144>` → Happy Path: `__device__ T* x` (global memory, inferred from size)
- `#[cuda.shared] let x: Vector<Int, 64>` → Override: `__shared__ T x[64]` (fast on-chip cache)

This design means **a well-defined TOML can often bypass the need for most, if not all,
attribute declarations**. Attributes are "pointers" — they help the compiler navigate
edge cases where the happy path needs adjustment, but they are not the primary tool.

```toml
# Example: cuda.toml inference section
[inference]
scalar = "register"         # Inferred happiest path for let x: Int
large_vector = "global"     # Inferred for vectors > 1KB
transaction = "kernel"      # Inferred for node
trigger = "interrupt"       # Inferred for trg
```


## 4. Backend Integration

### 4.1 C Backend Refactor

**Current**: 200+ lines of if/else branching based on boolean flags.

**Target Spec version**:
```rust
pub struct CBackend {
    spec: TargetSpec,
}

impl CBackend {
    pub fn generate(&mut self, program: &Program, stem: &str) -> String {
        let output = String::new();

        // 1. Inject header from TOML
        if let Some(header) = &self.spec. codegen. templates. header {
            output. push_ str(header);
            output. push_ str("\n\n");
        }

        // 2. Generate State struct using FFI mapping
        self.generate_ state_ struct( program, &mut output);

        // 3. Generate transactions (AST -> C syntax)
        self.generate_ transactions( program, &mut output);

        // 4. Inject footer from TOML
        if let Some(footer) = &self.spec. codegen. templates. footer {
            output. push_ str(footer);
        }

        output
    }

    fn generate_ state_ struct(&self, program: &Program, output: &mut String) {
        output. push_ str("typedef struct {\n");
        for decl in program. state_ decls {
            let c_ type = self. spec. ffi. mapping. get(&decl. ty)
                . unwrap_ or_ else(|| "int32_ t");
            output. push_ str(&format!("    {} {};\n", c_ type, decl. name));
        }
        output. push_ str("} State;\n\n");
    }
}
```

### 4.2 TypeScript Backend

```rust
pub struct TypeScriptBackend {
    spec: TargetSpec,
}

impl TypeScriptBackend {
    pub fn generate(&mut self, program: &Program, stem: &str) -> String {
        let output = String::new();

        if let Some(header) = &self.spec.codegen.templates.header {
            output.push_str(header);
        }

        output.push_str(&format!("export class {} {{\n", stem));
        for txn in program.transactions {
            self.generate_transaction(txn, &mut output);
        }
        output.push_str("}\n");

        if let Some(footer) = &self.spec.codegen.templates.footer {
            output.push_str(footer);
        }

        output
    }
}
```

### 4.5 New Territory Backends

Each new backend follows the same pattern: accept `TargetSpec`, use `[codegen.templates]` 
for framework wrapping, use `[inference]` for happy path, check `[codegen.validation]`.

#### 4.5.1 CUDA Backend (src/backend/cuda.rs)
- Basis: C++ (`nvcc`)
- Maps `node` → `__global__ void kernel()`
- Maps `Vector<T,N>` → `__device__ T*` (happy path) or `__shared__ T[N]` (override)
- Maps `parameters` → Kernel arguments / constant memory

```toml
# lib/targets/cuda.toml
[codegen]
backend = "cuda"
extension = "cu"

[inference]
scalar = "register"
large_vector = "global"
transaction = "kernel"

[codegen.templates]
header = """
#include <cuda_runtime.h>
#include <device_launch_parameters.h>
"""

[codegen.validation]
Float = "allowed"  # GPU has native float support
```

Happy Path: `let x: Vector<Int, 262144>` → `__device__ int* x` (global memory, inferred from size)
Override: `#[cuda.shared] let x: Vector<Int, 64>` → `__shared__ int x[64]`

#### 4.5.2 WebGPU/WGSL Backend (src/backend/wgsl.rs)
- Basis: WGSL compute shaders
- Maps `node` → `@compute @workgroup_size(64) fn main()`
- Maps `Vector<T,N>` → `var<storage, read_write> x: array<T, N>`

```toml
# lib/targets/webgpu.toml
[codegen]
backend = "wgsl"
extension = "wgsl"

[inference]
scalar = "uniform"
large_vector = "storage"
transaction = "compute"

[codegen.templates]
header = """
@group(0) @binding(0) var<storage, read_write> state: array<u32>;
"""

[codegen.validation]
Float = "allowed"  # WGSL has native f32 support
```

Happy Path: `let world_map: Vector<UInt, 4096>` → `var<storage, read_write> world_map: array<u32, 4096>`
Override: `#[wgsl.uniform] let uniforms: Vector<Float, 16>` → `var<uniform> uniforms: Uniforms`

#### 4.5.3 TypeScript + React Backend (src/backend/react.rs)
- Basis: TypeScript (.tsx)
- Maps `let x: T` → `const [x, setX] = useState<T>(init)`
- Maps `node` → `const handleTxn = useCallback(...)`
- Auto-generates `useEffect` dependencies from precondition analysis

```toml
# lib/codegen/react.toml
[codegen]
backend = "react"
extension = "tsx"

[inference]
scalar = "useState"
transaction = "useCallback"
reactive = "useEffect"

[codegen.templates]
header = """
import React, { useState, useCallback, useEffect } from 'react';
export const BrivApp: React.FC = () => {
"""

footer = """};
"""
```

Happy Path: `let count: Int = 0` → `const [count, setCount] = useState<number>(0);`
Override: `#[react.ref] let input: String` → `const inputRef = useRef<string>("");`

#### 4.5.4 eBPF Backend (src/backend/ebpf.rs)
- Basis: Restricted C (LLVM BPF backend)
- Maps `Vector<T,N>` → BPF map definitions (`struct { ... } x SEC(".maps")`)
- Maps `node` → `SEC("kprobe/...") int handle_event(...)`
- Enforces verifier constraints (unrolled loops, bounded memory)

```toml
# lib/targets/ebpf.toml
[codegen]
backend = "ebpf"
extension = "c"

[inference]
scalar = "stack"
large_vector = "bpf_map"
transaction = "kprobe"

[codegen.templates]
header = """
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
"""

[codegen.validation]
Float = "error: Reach does not support floating-point types"
```

### 8.3 CUDA Target (Parallel High-Performance)

```toml
# lib/targets/cuda.toml
[ffi]
name = "cuda"
description = "NVIDIA CUDA C++ kernel target"

[ffi.language]
name = "C++"
endianness = "little"
pointer_size = 8

[ffi.types]
Int = { representation = "two's complement", size = 4, signed = true }
UInt = { representation = "unsigned", size = 4, signed = false }
Float = { representation = "IEEE754", size = 4 }

[ffi.mapping]
Int = "__device__ int"
UInt = "__device__ uint32_t"
Float = "__device__ float"
Void = "void"

[ffi.conventions]
alignment = 8
call_conv = "cuda_kernel"

[inference]
scalar = "register"
large_vector = "global"
transaction = "kernel"
trigger = "interrupt"

[codegen]
backend = "cuda"
extension = "cu"

[codegen.templates]
header = """
#include <cuda_runtime.h>
#include <device_launch_parameters.h>
"""

[codegen.validation]
Float = "allowed"

[codegen.entry_point]
style = "cuda_kernel"
init_txn = "init"
exit_txn = "exit"
```

Happy Path: `let x: Vector<Int, 262144>` → `__device__ int* x` (global memory, inferred from size)
Override: `#[cuda.shared] let x: Vector<Int, 64>` → `__shared__ int x[64]`
Override: `#[cuda.threads(16, 16)] node compute [...] {...}` → Kernel launch config

### 8.4 WebGPU/WGSL Target (Browser Parallelism)

```toml
# lib/targets/webgpu.toml
[codegen]
backend = "wgsl"
extension = "wgsl"

[inference]
scalar = "uniform"
large_vector = "storage"
transaction = "compute"
trigger = "dispatch"

[codegen.templates]
header = """
@group(0) @binding(0) var<storage, read_write> state: array<u32>;
"""

[codegen.validation]
Float = "allowed"  # WGSL has native f32

[codegen.entry_point]
style = "compute_shader"
workgroup_size = "64"
```

Happy Path: `let world_map: Vector<UInt, 4096>` → `var<storage, read_write> world_map: array<u32, 4096>`
Override: `#[wgsl.uniform] let uniforms: Vector<Float, 16>` → `var<uniform> uniforms: Uniforms`
Override: `#[wgsl.workgroup_size(8, 8, 1)] node cast_rays [...] {...}` → Compute shader dispatch

### 8.5 TypeScript + React Target (Living UI)

```toml
# lib/codegen/react.toml
[codegen]
backend = "react"
extension = "tsx"

[inference]
scalar = "useState"
large_vector = "useState"
transaction = "useCallback"
reactive = "useEffect"

[codegen.templates]
header = """
import React, { useState, useCallback, useEffect } from 'react';

export const BrivApp: React.FC = () => {
"""

footer = """};
"""

[codegen.validation]
Float = "allowed"

[codegen.entry_point]
style = "react_hooks"
init_txn = "init"
exit_txn = "exit"
```

Happy Path: `let count: Int = 0` → `const [count, setCount] = useState<number>(0);`
Happy Path: `node update [count > 0][...]` → `const handleUpdate = useCallback(() => { if (count > 0) {...} }, [count]);`
Override: `#[react.ref] let input: String` → `const inputRef = useRef<string>("");`
Override: `#[react.memo] txn expensive [...] {...}` → Wrapped in `React.memo`

### 8.6 eBPF Target (In-Kernel Orchestration)

```toml
# lib/targets/ebpf.toml
[ffi]
name = "ebpf"
description = "Linux eBPF restricted C"

[ffi.language]
name = "C"
endianness = "little"
pointer_size = 8

[ffi.types]
Int = { representation = "two's complement", size = 4, signed = true }
UInt = { representation = "unsigned", size = 4, signed = false }

[ffi.mapping]
Int = "__u32"
UInt = "__u32"
Bool = "__u8"

[ffi.conventions]
alignment = 8
call_conv = "ebpf_section"

[inference]
scalar = "stack"
large_vector = "bpf_map"
transaction = "kprobe"
trigger = "socket_filter"

[codegen]
backend = "ebpf"
extension = "c"

[codegen.templates]
header = """
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
"""

[codegen.validation]
Float = "error: eBPF verifier does not support floating point"

[codegen.entry_point]
style = "ebpf_section"
init_txn = "init"
exit_txn = "exit"
```

Happy Path: `let packet_counts: Vector<UInt, 1024>` → BPF map definition with `BPF_MAP_TYPE_ARRAY`
Override: `#[ebpf.map("hash")] let flow_table: Vector<UInt, 256>` → `BPF_MAP_TYPE_HASH`
Override: `#[ebpf.section("kprobe/do_sys_open")] node monitor [...] {...}` → eBPF hook point

### 8.7 Python Target (Universal Orchestrator)

```toml
# lib/targets/python.toml
[codegen]
backend = "python"
extension = "py"

[inference]
scalar = "instance_var"
large_vector = "numpy"
transaction = "method"
reactive = "async_method"

[codegen.templates]
header = """
import numpy as np
from dataclasses import dataclass
from typing import List, Optional

@dataclass
class BrivState:
"""

[codegen.validation]
Float = "allowed"

[codegen.entry_point]
style = "class_method"
init_txn = "init"
exit_txn = "exit"
```

Happy Path: `let weights: Vector<Int, 262144>` → `self.weights: np.ndarray = np.zeros(262144, dtype=np.int32)`
Happy Path: `node sort [true][...]` → `def sort(self): ...` with contract validation
Override: `#[python.jit] node heavy_math [...] {...}` → `@numba.jit def heavy_math(self):`
Override: `#[python.fastapi] txn api_endpoint [...] {...}` → `@app.post("/txn") def api_endpoint():`

### 8.8 Swift/Kotlin Target (Mobile Native)

```toml
# lib/targets/swift.toml
[codegen]
backend = "swift"
extension = "swift"

[inference]
scalar = "published"
large_vector = "state_flow"
transaction = "async_func"
reactive = "on_change"

[codegen.templates]
header = """
import SwiftUI
import Combine

class BrivViewModel: ObservableObject {
    @Published var state: BrivState
"""

[codegen.validation]
Float = "allowed"

[codegen.entry_point]
style = "swiftui_viewmodel"
init_txn = "init"
exit_txn = "exit"
```

Happy Path: `let gps_location: LatLong` → `@Published var gpsLocation: LatLong` (SwiftUI)
Happy Path: `node update_ui [state_changed][...]` → `func updateUI() async {...}` in ViewModel
Override: `#[mobile.local_storage] let settings: Config` → Persisted to `UserDefaults`
Override: `#[mobile.main_thread] txn ui_update [...] {...}` → Dispatched to main thread

---

## 8.5 Attribute (`#[]`) Logic: The Diplomatic Layer

Attributes provide target-specific overrides without polluting the core Briv language.
**A well-defined TOML can often bypass the need for most, if not all, attribute declarations.**
Attributes are "pointers" — they help the compiler navigate edge cases where the happy path needs adjustment.

### 8.5.1 For React:
```briv
#[react.hook(useMemo)]
let derived_view = ...

#[react.component(style: "tailwind")]
trg user_clicked: Bool;
```

### 8.5.2 For eBPF:
```briv
#[ebpf.map(type: "hash", max_entries: 1024)]
let packet_counts: Vector<UInt, 1024>;
```

### 8.5.3 For CUDA:
```briv
#[cuda.shared]
let cache: Vector<Int, 64>;

#[cuda.threads(16, 16)]
node compute [true][...] { ... }
```

### 8.5.4 For WebGPU:
```briv
#[wgsl.uniform]
let uniforms: Vector<Float, 16>;

#[wgsl.workgroup_size(8, 8, 1)]
node cast_rays [player_moving][...] { ... }
```

### 8.5.5 For Python:
```briv
#[python.jit]
node heavy_math [true][...] { ... }

#[python.fastapi]
txn api_endpoint [true][...] { ... }
```

### 8.5.6 For Mobile (Swift/Kotlin):
```briv
#[mobile.observable]
let gps_location: LatLong;

#[mobile.main_thread]
node update_ui [state_changed][...] { ... }
```

---

## 9. Implementation Phases

### Phase 1: TargetSpec Struct and Loader
- Create `src/target_ spec/mod. rs` with TargetSpec, FfiSection, CodegenSection
- Create `src/target_ spec/loader. rs` with TargetSpecLoader
- Add `toml` crate to `Cargo. toml`

### Phase 2: C Backend Refactor
- Refactor CBackend to accept TargetSpec instead of boolean flags
- Update generate() to use spec templates
- Add Float validation from [codegen. validation]

### Phase 3: Validation Integration
- Integrate validation into `src/typechecker. rs`
- Emit E3001 errors for blocked types

### Phase 4: CLI Updates
- Update `src/main. rs` to load Target Spec
- Add --target flag support

### Phase 5: TypeScript Backend (for Reach)
- Create `src/backend/typescript. rs`
- Generate TypeScript from Briv AST using spec

### Phase 6: Additional Profiles
- `linux_kernel.toml`
- `kv260_baremetal.toml`
- `reach.toml`

### Phase 7: CUDA Backend
- Create `src/backend/cuda.rs`
- Parse `[inference]` for memory hierarchy (register → shared → global)
- Generate `__global__` kernels from `node`

### Phase 8: WebGPU/WGSL Backend
- Create `src/backend/wgsl.rs`
- Map Briv state to WGSL address spaces (`var<storage>`, `var<uniform>`)
- Generate compute shaders with `@workgroup_size`

### Phase 9: TypeScript + React Backend
- Create `src/backend/react.rs`
- Implement hook inference (`useState`, `useCallback`, `useEffect`)
- Generate `.tsx` with automatic React bindings

### Phase 10: eBPF Backend
- Create `src/backend/ebpf.rs`
- Generate BPF map definitions and section declarations
- Enforce verifier constraints (unroll loops, bound checks)

### Phase 11: Python Backend
- Create `src/backend/python.rs`
- Infer NumPy arrays for large vectors
- Support `@numba.jit` and FastAPI decorators via attributes

### Phase 12: Swift/Kotlin Backend
- Create `src/backend/mobile.rs`
- Map to `@Published` (Swift) or `StateFlow` (Kotlin)
- Generate native mobile library code

---

## 10. Success Criteria

| Criterion | Verification |
|-----------|--------------|
| C backend uses TargetSpec | All boolean flags removed, spec-driven generation |
| Float emits E3001 for kernel | cargo test --lib passes; kernel spec blocks Float |
| Reach codegen works | reach.toml + TypeScript backend generates valid Reach code |
| CLI loads specs | briv compile --target linux_kernel.toml file.bv succeeds |
| Spec loader searches paths | Both lib/ and hardware_lib/ paths resolve |
| Multiple targets | Linux kernel, KV260, WASM, COBOL all work |
| CUDA backend works | `cuda.toml` + generates valid `.cu` kernels |
| WebGPU renders DOOM | `webgpu.toml` + WGSL compute shaders at 60fps |
| React hooks inferred | `.rbv` → `.tsx` with automatic `useState`/`useEffect` |
| eBPF verifier passes | `ebpf.toml` + code passes kernel BPF verifier |
| Python NumPy inferred | `python.toml` + large vectors → `np.ndarray` |
| Swift/Kotlin native | `swift.toml` → `@Published` SwiftUI code |
| Happy path inference | Well-defined TOML bypasses need for most `#[]` attributes |
| Attribute overrides work | `#[cuda.shared]`, `#[react.ref]` etc. override happy path |

---

## 11. Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `src/target_spec/mod.rs` | TargetSpec and section structs |
| `src/target_spec/loader.rs` | TargetSpecLoader |
| `lib/targets/linux_kernel.toml` | Kernel module target |
| `lib/targets/kv260_baremetal.toml` | ARM bare-metal target |
| `lib/codegen/reach.toml` | Reach blockchain target |
| `src/backend/typescript.rs` | TypeScript backend |
| `src/backend/cuda.rs` | CUDA C++ kernel backend |
| `src/backend/wgsl.rs` | WebGPU/WGSL compute shader backend |
| `src/backend/react.rs` | TypeScript + React .tsx backend |
| `src/backend/ebpf.rs` | eBPF restricted C backend |
| `src/backend/python.rs` | Python 3.10+ backend |
| `src/backend/mobile.rs` | Swift/Kotlin mobile backend |
| `lib/targets/cuda.toml` | CUDA target spec |
| `lib/targets/webgpu.toml` | WebGPU target spec |
| `lib/targets/ebpf.toml` | eBPF target spec |
| `lib/targets/python.toml` | Python target spec |
| `lib/targets/swift.toml` | Swift iOS target spec |
| `lib/targets/kotlin.toml` | Kotlin Android target spec |
| `lib/codegen/react.toml` | React TypeScript target spec |

### Modified Files

| File | Change |
|------|--------|
| `src/backend/c.rs` | Accept TargetSpec, use templates |
| `src/backend/mod.rs` | Export TypeScriptBackend, CudaBackend, WgslBackend, ReactBackend, EbpfBackend, PythonBackend, MobileBackend |
| `src/typechecker.rs` | Validate against spec, emit E3001 for blocked types |
| `src/main.rs` | Load Target Spec, wire to all backends |
| `Cargo.toml` | Add toml dependency, numba (Python), wgpu (Rust) |

---

## 12. Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-05-01 | Created specification | OpenCode |
| 2026-05-01 | Added Sections 3.5, 4.5, 8.3-8.8, 8.5 | OpenCode |
| 2026-05-01 | Updated Phases 7-12, Success Criteria, Files | OpenCode |
| 2026-05-01 | Implemented Phase 1: target_spec module | OpenCode |
| 2026-05-01 | Added Sections 3.5, 4.5, 8.3-8.8, 8.5 | OpenCode |
| 2026-05-01 | Updated Phases 7-12, Success Criteria, Files | OpenCode |
| 2026-05-01 | **COMPLETED Phase 2**: CBackend refactor to use TargetSpec | OpenCode |
