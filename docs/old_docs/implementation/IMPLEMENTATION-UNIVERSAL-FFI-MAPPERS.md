# Universal FFI with External Mappers - Implementation Plan

## Overview

This plan implements a universal FFI system where:
- **TOML** defines explicit contracts between foreign libraries and Brief
- **Mappers** are external, user-extensible translation layers (NOT built into compiler)
- **Any language** can be supported by writing a mapper, no compiler changes needed

---

## Discovery Mechanism

| TOML Config | Behavior |
|------------|----------|
| `mapper = "rust"` | System searches default paths |
| `mapper = "rust" path = "./mappers/my_mapper"` | Explicit path overrides defaults |

**Default search order:**
1. `lib/mappers/<mapper_name>/`
2. `lib/ffi/mappers/<mapper_name>/`

---

## Directory Structure

```
lib/
├── ffi/
│   ├── __init__.bv
│   ├── bindings/
│   │   ├── io.toml
│   │   ├── math.toml
│   │   ├── string.toml
│   │   ├── time.toml
│   │   ├── encoding.toml
│   │   └── json.toml
│   ├── mappers/                     # DEFAULT mappers (user-editable)
│   │   ├── mod.rs                   # Mapper registry
│   │   ├── rust_mapper.bv           # 1:1 Rust mapping
│   │   ├── c_mapper.bv              # C string handling
│   │   └── wasm_mapper.bv           # WASM memory handling
│   └── native/
│
├── mappers/                         # USER-DEFINED mappers (empty, ready)
│
└── std/
```

---

## Implementation Phases

### Phase 1: Add Mapper + Path Fields

**Changes to `src/ast.rs`:**
```rust
pub struct ForeignBinding {
    pub name: String,
    pub description: Option<String>,
    pub location: String,
    pub target: ForeignTarget,
    pub mapper: Option<String>,      // NEW: which mapper to use
    pub path: Option<String>,       // NEW: explicit path to mapper
    pub inputs: Vec<(String, Type)>,
    pub success_output: Vec<(String, Type)>,
    pub error_type: String,
    pub error_fields: Vec<(String, Type)>,
}
```

**Changes to `src/ffi/loader.rs`:**
- Parse both `mapper` and `path` fields from TOML
- Make them optional

**Update `lib/ffi/bindings/*.toml`:**
```toml
[[functions]]
name = "__sin"
mapper = "rust"
target = "native"
location = "brief_ffi_native::__sin"
```

---

### Phase 2: Mapper Registry + Discovery

**New file: `lib/ffi/mappers/mod.rs`**

```rust
pub trait FfiMapper: Send + Sync {
    fn map_input(&self, name: &str, value: ForeignValue) -> BriefValue;
    fn map_output(&self, name: &str, value: BriefValue) -> ForeignValue;
    fn map_error(&self, error: ForeignError) -> BriefError;
}

pub struct MapperRegistry {
    mappers: HashMap<String, Box<dyn FfiMapper>>,
}

impl MapperRegistry {
    // If path provided: use exact path
    // Else: search default paths in order:
    // 1. lib/mappers/<mapper_name>/
    // 2. lib/ffi/mappers/<mapper_name>/
    pub fn find_mapper(&self, name: &str, custom_path: Option<&str>) -> Option<MapperInfo> { ... }
}
```

---

### Phase 3: Default Mappers

**Rust Mapper** (`lib/ffi/mappers/rust_mapper.bv`):
```brief
// 1:1 mapping - no transformation
defn map_input(value: Value) -> Value [true][true] { term value; };
defn map_output(value: Value) -> Value [true][true] { term value; };
defn map_error(err: Error) -> Error [true][true] { term err; };
```

**C Mapper** (`lib/ffi/mappers/c_mapper.bv`):
```brief
// Handles C string null-termination, UTF-8
defn c_string_to_brief(c_str: CString) -> String [c_str.is_valid()][true] {
  term c_str.to_str();
};

defn brief_string_to_c(s: String) -> CString [true][true] {
  term CString::new(s);
};
```

**WASM Mapper** (`lib/ffi/mappers/wasm_mapper.bv`):
```brief
// Handles WASM linear memory, JS value conversion
defn wasm_ptr_to_string(ptr: Int, mem: Memory) -> String [ptr > 0][true] {
  term mem.read_string(ptr);
};
```

---

### Phase 4: Update FFI Loader

```rust
pub fn load_and_call(binding: &ForeignBinding, args: Vec<Value>) -> Result<Value> {
    // 1. Read mapper and path from binding
    let mapper_name = binding.mapper.as_deref().unwrap_or("rust");
    let mapper_path = binding.path.as_deref();
    
    // 2. Find and load mapper
    let mapper = registry.find_mapper(mapper_name, mapper_path)?;
    
    // 3. Map input args
    let mapped_args: Vec<Value> = args.into_iter()
        .map(|arg| mapper.map_input(&binding.name, arg))
        .collect();
    
    // 4. Call via location
    let result = call_foreign(&binding.location, mapped_args)?;
    
    // 5. Map output
    match result {
        Ok(v) => Ok(mapper.map_output(&binding.name, v)),
        Err(e) => Err(mapper.map_error(e)),
    }
}
```

---

### Phase 5: User Documentation

Document how to create custom mappers:
- Place in `lib/mappers/<mapper_name>/`
- TOML references `mapper = "<mapper_name>"` or `mapper = "<mapper_name>" path = "./path"`

---

## Key Design Principles

| Principle | How It's Achieved |
|-----------|-------------------|
| **Not built into compiler** | All mapper code is in `lib/` - user-editable |
| **No PR required** | User adds `lib/mappers/<name>/` - discovered automatically |
| **Brief-can-define** | Mappers can be written in Brief (`*.bv` files) |
| **Universal** | Add new language = write new mapper, no compiler changes |
| **Explicit** | TOML is the contract boundary |

---

## Deliverables

| File | Description |
|------|-------------|
| `src/ast.rs` | Added mapper + path fields to ForeignBinding |
| `src/ffi/loader.rs` | Updated to parse and use mappers |
| `lib/ffi/bindings/*.toml` | All have explicit mapper field |
| `lib/ffi/mappers/mod.rs` | New registry with discovery |
| `lib/ffi/mappers/rust_mapper.bv` | Default Rust mapper |
| `lib/ffi/mappers/c_mapper.bv` | Default C mapper |
| `lib/ffi/mappers/wasm_mapper.bv` | Default WASM mapper |
| `lib/mappers/` | User mapper directory (empty, ready for users) |

---

## Implementation Order

1. Phase 1: Add mapper + path fields to AST and TOML
2. Phase 2: Create mapper registry with discovery logic
3. Phase 3: Implement default mappers in Brief
4. Phase 4: Update FFI loader to use mappers
5. Phase 5: Document user mapper creation