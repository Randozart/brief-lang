# FFI Implementation Plan - Brief v6.2

**Date:** 2026-04-05  
**Status:** Detailed Implementation Roadmap  
**Scope:** Complete FFI system for v6.2

---

## Overview

This plan details the implementation of Brief's robust Foreign Function Interface (FFI) system as specified in SPEC-v6.2.md. The system is built on three pillars:

1. **TOML Binding Declarations** - Explicit contracts between Brief and foreign code
2. **frgn Syntax** - Lightweight foreign function declarations
3. **Safe Wrapper Pattern** - defn handles all error cases and contracts

---

## Phase Breakdown

### Phase 1: Core Data Structures & AST (Days 1-2)

**Objective:** Establish FFI representation in Brief's type system

**Files to Create/Modify:**
- `src/ast.rs` - Add FFI-related AST nodes
- `spec/FFI-BINDING-SCHEMA.md` - TOML schema specification

**Tasks:**

1. **Add to AST enum `TopLevel`:**
   ```rust
   pub enum TopLevel {
       // ... existing ...
       ForeignBinding {
           name: String,
           toml_path: String,
           signature: ForeignSignature,
           target: ForeignTarget,
           span: Option<Span>,
       }
   }
   ```

2. **Add new enums:**
   ```rust
   pub enum ForeignTarget {
       Native,   // Rust FFI (v6.2)
       Wasm,     // WebAssembly (future)
   }
   ```

3. **Add ForeignSignature struct:**
   ```rust
   pub struct ForeignSignature {
       pub name: String,
       pub inputs: Vec<(String, Type)>,           // param_name -> type
       pub success_output: Vec<(String, Type)>,   // named fields
       pub error_type_name: String,               // e.g., "IoError"
       pub error_fields: Vec<(String, Type)>,     // error shape
   }
   ```

4. **Add ForeignBinding struct:**
   ```rust
   pub struct ForeignBinding {
       pub name: String,
       pub description: Option<String>,
       pub location: String,                      // Rust module path
       pub target: ForeignTarget,
       pub inputs: HashMap<String, Type>,
       pub success_output: HashMap<String, Type>,
       pub error_type: String,
       pub error_fields: HashMap<String, Type>,
   }
   ```

**Testing:**
- Unit tests for AST construction
- Type equality checks for ForeignSignature

**Verification:**
- Compiler builds without errors
- All existing tests still pass

---

### Phase 2: TOML Loader Module (Days 2-4)

**Objective:** Parse and validate TOML binding files

**Files to Create:**
- `src/ffi/mod.rs` - Module coordinator
- `src/ffi/loader.rs` - TOML loading and parsing
- `src/ffi/types.rs` - FFI type definitions
- `src/ffi/validator.rs` - Binding validation
- `src/ffi/resolver.rs` - Path resolution

**Tasks:**

1. **`src/ffi/mod.rs` (Coordinator)**
   ```rust
   pub mod loader;
   pub mod validator;
   pub mod resolver;
   pub mod types;
   
   pub use loader::load_binding;
   pub use validator::validate_frgn_against_binding;
   pub use resolver::resolve_binding_path;
   ```

2. **`src/ffi/loader.rs` (TOML Parsing)**
   ```rust
   pub fn load_binding(path: &str) -> Result<ForeignBinding, LoadError>
   
   // Parses TOML file and extracts:
   // - Function name and description
   // - Location (module::function)
   // - Input/output/error types
   // - Target specification
   ```

   **Error Handling:**
   - File not found
   - Invalid TOML syntax
   - Missing required fields
   - Type parsing failures

3. **`src/ffi/resolver.rs` (Path Resolution)**
   ```rust
   pub fn resolve_binding_path(path: &str, project_root: &str) -> Result<PathBuf, ResolveError>
   
   // Resolve paths:
   // "std/bindings/io.toml" -> stdlib location
   // "bindings/custom.toml" -> project relative
   // "/abs/path/binding.toml" -> absolute
   ```

4. **`src/ffi/validator.rs` (Validation)**
   ```rust
   pub fn validate_frgn_against_binding(
       frgn_sig: &ForeignSignature,
       binding: &ForeignBinding
   ) -> Result<(), ValidationError>
   
   // Validates:
   // - Names match
   // - Input types compatible
   // - Output types compatible
   // - Error type properly named
   ```

5. **`src/ffi/types.rs` (Type System)**
   ```rust
   pub enum FfiType {
       String,
       Int,
       Float,
       Bool,
       Void,
       Array(Box<FfiType>),
       Struct(HashMap<String, FfiType>),
   }
   
   pub fn brief_type_to_ffi_type(t: &Type) -> FfiType
   pub fn ffi_type_to_brief_type(t: &FfiType) -> Type
   ```

**Testing:**
- Unit tests for each loader/resolver/validator function
- TOML parsing edge cases
- Path resolution for all scenarios
- Type mapping verification

**Verification:**
- Load `std/bindings/io.toml` successfully
- Validate sample frgn declarations
- Error messages clear and actionable

---

### Phase 3: Parser Integration (Days 4-5)

**Objective:** Parse `frgn` declarations in Brief code

**Files to Modify:**
- `src/parser.rs` - Add frgn parsing
- `src/lexer.rs` - Add "frgn" keyword (if not present)

**Tasks:**

1. **Add "frgn" keyword to lexer** (if needed)
   ```rust
   Token::Frgn // = "frgn"
   ```

2. **Add parser function:**
   ```rust
   fn parse_foreign_binding(&mut self) -> Result<TopLevel, ParseError>
   
   // Parses:
   // frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";
   ```

3. **Updated BNF in parser:**
   ```bnf
   foreign_binding ::= 
       "frgn" 
       identifier 
       "(" parameters? ")" 
       "->" result_type 
       "from" string_literal 
       ";"
   ```

4. **Parser output:**
   - Validates syntax
   - Extracts function name, parameters, return type
   - Extracts TOML path
   - Creates `TopLevel::ForeignBinding` AST node

**Testing:**
- Parse simple frgn declarations
- Parse frgn with generic types
- Parse frgn with complex types
- Error cases (malformed, missing from clause)

**Verification:**
- All existing parser tests still pass
- New frgn tests cover all variants

---

### Phase 4: Type Checker Integration (Days 5-6)

**Objective:** Validate FFI bindings during type checking phase

**Files to Modify:**
- `src/typechecker.rs` - Add FFI validation

**Tasks:**

1. **In `check_program()` method:**
   ```rust
   // New first pass: collect and validate foreign bindings
   for item in &program.items {
       if let TopLevel::ForeignBinding { .. } = item {
           self.validate_foreign_binding(item)?;
       }
   }
   ```

2. **New method: `validate_foreign_binding()`**
   ```rust
   fn validate_foreign_binding(&mut self, binding: &TopLevel) -> Result<(), TypeError>
   
   // Validates:
   // - Load TOML file
   // - Binding name matches frgn name
   // - All input types compatible
   // - Output type is Result<T, E>
   // - Error type matches TOML
   ```

3. **During defn/txn type checking:**
   - When `frgn foo()` is called, type system knows it returns `Result<T, E>`
   - Force pattern matching on result
   - Ensure all branches handled (exhaustiveness)

**Testing:**
- Valid FFI bindings pass
- Missing TOML file caught
- Type mismatches caught
- Missing error type handling caught

**Verification:**
- Type checker rejects invalid FFI
- Type checker accepts valid FFI
- Error messages are clear

---

### Phase 5: Proof Engine Integration (Days 6-7)

**Objective:** Verify FFI contracts during proof verification

**Files to Modify:**
- `src/proof_engine.rs` - Add FFI contract verification

**Tasks:**

1. **New method: `verify_frgn_calls()`**
   ```rust
   fn verify_frgn_calls(&mut self, program: &Program) {
       // For each defn/txn that calls frgn:
       // - Verify success path handling
       // - Verify error path handling
       // - Check exhaustiveness of error cases
   }
   ```

2. **Enhanced contract verification:**
   - `frgn` calls treated as statements with known Result type
   - Must verify both `Success(T)` and `Error(E)` branches
   - Postcondition must hold for all outcomes

3. **Error reporting:**
   - Error code `F001` - frgn call not exhaustively handled
   - Error code `F002` - frgn binding not found
   - Error code `F003` - frgn contract violation

**Testing:**
- Proof engine accepts safe frgn usage
- Proof engine rejects unsafe patterns
- Error messages guide user to fix

**Verification:**
- All existing proof tests pass
- New FFI examples verify correctly

---

### Phase 6: Standard Library Bindings (Days 7-9)

**Objective:** Create stdlib FFI bindings

**Files to Create:**
- `std/bindings/io.toml`
- `std/bindings/time.toml`
- `std/bindings/math.toml`
- `std/bindings/string.toml`

**Each TOML file:**

1. **`std/bindings/io.toml`**
   ```toml
   [[functions]]
   name = "read_file"
   location = "std::fs::read_to_string"
   # ... (per spec)
   
   [[functions]]
   name = "write_file"
   location = "std::fs::write"
   # ... (per spec)
   ```

2. **`std/bindings/time.toml`**
   ```toml
   [[functions]]
   name = "now"
   location = "std::time::SystemTime::now"
   # ... (per spec)
   ```

3. **`std/bindings/math.toml`**
   ```toml
   [[functions]]
   name = "sqrt"
   location = "libm::sqrt"
   # ... (per spec)
   ```

4. **`std/bindings/string.toml`**
   ```toml
   [[functions]]
   name = "concat"
   location = "String::from"  # Pseudo-implementation
   # ... (per spec)
   ```

**Testing:**
- Each binding TOML is valid
- Can be loaded by FFI system
- Type mappings correct

**Verification:**
- All stdlib bindings load without error
- Examples can reference them

---

### Phase 7: Comprehensive Testing (Days 9-11)

**Objective:** Full test coverage for FFI system

**Files to Create:**
- `tests/ffi_loader_tests.rs` - TOML loading
- `tests/ffi_validator_tests.rs` - Validation
- `tests/ffi_integration_tests.rs` - End-to-end
- `tests/ffi_parser_tests.rs` - Parser
- `examples/ffi_file_read_safe.bv` - Example 1
- `examples/ffi_error_handling.bv` - Example 2
- `examples/ffi_nested_fallback.bv` - Example 3

**Test Categories:**

1. **Unit Tests: Loader**
   - Parse valid TOML
   - Reject invalid TOML
   - Extract all fields correctly
   - Handle missing files

2. **Unit Tests: Validator**
   - Accept matching signatures
   - Reject mismatched types
   - Detect missing error types
   - Verify type compatibility

3. **Unit Tests: Parser**
   - Parse simple frgn declarations
   - Parse frgn with generic types
   - Parse frgn with complex types
   - Reject malformed frgn

4. **Integration Tests: End-to-End**
   - Load TOML → Create FFI → Use in defn → Verify
   - Error handling paths
   - Complex type mappings
   - Exhaustiveness checking

5. **Example Programs:**
   - Safe file read with fallback
   - Error inspection patterns
   - Nested FFI calls
   - Generic type parameters

**Coverage Goals:**
- 90%+ code coverage for FFI modules
- All error paths tested
- All success paths tested
- Examples compile and verify

---

### Phase 8: Documentation (Days 11-12)

**Objective:** Complete user-facing documentation

**Files to Create/Modify:**
- `spec/FFI-USER-GUIDE.md` - How to write bindings
- `spec/FFI-STDLIB-REFERENCE.md` - Stdlib bindings reference
- `README.md` - Update with FFI mention

**Content:**

1. **FFI-USER-GUIDE.md**
   - Step-by-step guide for writing custom bindings
   - TOML format explained
   - Rust FFI implementation example
   - Error handling patterns
   - Best practices

2. **FFI-STDLIB-REFERENCE.md**
   - Each stdlib binding documented
   - Function descriptions
   - Error types and codes
   - Example usage for each

3. **README.md updates**
   - FFI as major v6.2 feature
   - Link to user guide
   - Quick start example

**Verification:**
- Documentation is complete
- Examples are accurate
- User can follow guide to create binding

---

## Implementation Timeline

```
Days 1-2:   Phase 1 - AST Structures
Days 2-4:   Phase 2 - TOML Loader
Days 4-5:   Phase 3 - Parser Integration
Days 5-6:   Phase 4 - Type Checker
Days 6-7:   Phase 5 - Proof Engine
Days 7-9:   Phase 6 - Stdlib Bindings
Days 9-11:  Phase 7 - Testing
Days 11-12: Phase 8 - Documentation

Total: ~12 days (2 weeks) for complete implementation
```

---

## File Structure (Final)

```
src/
├── ffi/
│   ├── mod.rs              (300 lines)
│   ├── loader.rs           (400 lines)
│   ├── validator.rs        (300 lines)
│   ├── resolver.rs         (200 lines)
│   └── types.rs            (250 lines)
├── ast.rs                  (+ 150 lines for FFI nodes)
├── parser.rs               (+ 200 lines for frgn parsing)
├── typechecker.rs          (+ 150 lines for FFI validation)
└── proof_engine.rs         (+ 200 lines for FFI verification)

std/bindings/
├── io.toml                 (60 lines)
├── time.toml               (40 lines)
├── math.toml               (50 lines)
└── string.toml             (50 lines)

spec/
├── SPEC-v6.2.md            (Main spec - 500+ lines)
├── FFI-USER-GUIDE.md       (150+ lines)
├── FFI-STDLIB-REFERENCE.md (100+ lines)
└── IMPLEMENTATION-PLAN-FFI-v6.2.md (this file)

tests/
├── ffi_loader_tests.rs     (200 lines)
├── ffi_validator_tests.rs  (150 lines)
├── ffi_parser_tests.rs     (200 lines)
└── ffi_integration_tests.rs (300 lines)

examples/
├── ffi_file_read_safe.bv   (40 lines)
├── ffi_error_handling.bv   (50 lines)
└── ffi_nested_fallback.bv  (60 lines)
```

**Total New Code:** ~3,500 lines (core + tests + docs)

---

## Success Criteria

### Compile & Build
- ✅ All new modules compile without warnings
- ✅ No breaking changes to existing code
- ✅ All 47 existing tests still pass (39 unit + 8 integration)

### Functionality
- ✅ Can load and parse TOML binding files
- ✅ Can validate frgn declarations against TOML
- ✅ Can type check FFI usage in Brief code
- ✅ Can prove FFI contracts in defn bodies
- ✅ Can handle all error cases properly

### Testing
- ✅ 50+ new unit tests covering all FFI functionality
- ✅ 8+ integration tests for end-to-end scenarios
- ✅ 3+ working example programs
- ✅ 90%+ code coverage of FFI modules

### Documentation
- ✅ SPEC-v6.2.md complete and authoritative
- ✅ User guide enables writing custom bindings
- ✅ Stdlib reference documents all provided bindings
- ✅ Examples show common patterns

### User Experience
- ✅ Error messages are clear and actionable
- ✅ TOML format is intuitive
- ✅ frgn syntax is minimal and clear
- ✅ Wrapper defn pattern is well-documented

---

## Risk Mitigation

**Risk: TOML loading complexity**
- Mitigation: Start with basic TOML parsing, use existing libraries
- Fallback: Use serde_toml crate for robust parsing

**Risk: Type mapping edge cases**
- Mitigation: Start with simple types, expand incrementally
- Fallback: Conservative type validation, reject ambiguous cases

**Risk: FFI glue code generation**
- Mitigation: v6.2 is stub-only, real generation in v6.3
- Fallback: Manual Rust implementation for v6.2 stdlib

**Risk: Breaking changes to existing code**
- Mitigation: FFI is opt-in, no changes to core language
- Fallback: Careful AST design, thorough regression testing

---

## Future Enhancements (v6.3+)

1. **FFI Glue Code Generation**
   - Auto-generate Rust FFI stubs from TOML

2. **Non-Rust Language Support**
   - C FFI bindings
   - Python bindings
   - Type mapping system for cross-language types

3. **WASM Target**
   - WASM import declarations
   - Host function binding

4. **Dynamic Binding Loading**
   - Load bindings at runtime
   - Plugin-style foreign functions

5. **Reflection & Introspection**
   - Query available foreign functions
   - Runtime type inspection

---

## Approval Checklist

- [ ] User approves design
- [ ] User approves timeline
- [ ] User approves file structure
- [ ] User approves success criteria
- [ ] Ready to begin implementation

