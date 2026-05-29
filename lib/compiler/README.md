# Brief Compiler Infrastructure

**Version:** 0.11.0  
**Status:** Complete frontend, partial backends

---

## Structure

```
lib/compiler/
├── README.md                  # This file
├── token.bv                   # Token type definition
├── lexer.bv                   # Lexer implementation
├── parser.bv                  # Parser implementation
├── ast.bv                     # AST definitions
├── typechecker.bv             # Type checker with unification
├── proof_engine.bv            # Symbolic execution & verification
└── backends/
    ├── README.md              # Backend documentation
    ├── aarch64.bv             # AArch64 binary backend ✅
    ├── x86_64.bv              # x86-64 binary backend (planned)
    ├── rust.bv                # Rust backend (planned)
    ├── c.bv                   # C backend (planned)
    ├── wasm.bv                # WASM backend (planned)
    ├── vhdl.bv                # VHDL backend (planned)
    └── verilog.bv             # SystemVerilog backend (planned)
```

---

## Compiler Pipeline

```
Source (.bv)
    ↓
[token.bv] Token Type
    ↓
[lexer.bv] Lexer → List<Token>
    ↓
[parser.bv] Parser → AST (ast.bv)
    ↓
[typechecker.bv] Type Checker → Typed AST
    ↓
[proof_engine.bv] Proof Engine → Verified AST
    ↓
[backends/*.bv] Backend → Target Code
```

---

## Usage

```brief
import "compiler/token";
import "compiler/lexer";
import "compiler/parser";
import "compiler/typechecker";
import "compiler/proof_engine";
import "compiler/backends/aarch64";

defn compile(source: String) -> Result<List<u8>, String> {
    // Phase 1: Lexing
    let tokens = tokenize(source)?;
    
    // Phase 2: Parsing
    let parser = new_parser(tokens);
    let program = parse_program(parser)?;
    
    // Phase 3: Type Checking
    let typed_program = check_program(program)?;
    
    // Phase 4: Verification
    let verified = verify_program(typed_program)?;
    
    // Phase 5: Code Generation
    let binary = generate_aarch64(verified);
    
    term Ok(binary);
}
```

---

## Standard Library vs Compiler

**Standard Library (`lib/std/`):**
- Runtime types (HashMap, HashSet, Stack, Queue)
- String operations
- Math functions
- IO operations
- Collections
- Data structures

**Compiler (`lib/compiler/`):**
- Lexer and parser
- Type checker
- Proof engine
- Code backends
- AST definitions

The compiler uses the standard library, but the standard library doesn't depend on the compiler.

---

## Implementation Status

### Frontend (Complete)
- ✅ Lexer (token.bv, lexer.bv)
- ✅ Parser (parser.bv, ast.bv)
- ✅ Type Checker (typechecker.bv)
- ✅ Proof Engine (proof_engine.bv)

### Backends (Partial)
- ✅ AArch64 binary (backends/aarch64.bv)
- ⏳ x86-64 binary (planned)
- ⏳ Rust (planned)
- ⏳ C (planned)
- ⏳ WASM (planned)
- ⏳ VHDL (planned)
- ⏳ SystemVerilog (planned)

---

## Performance

| Phase | Complexity | Notes |
|-------|------------|-------|
| **Lexing** | O(n) | Single pass |
| **Parsing** | O(n) | Recursive descent |
| **Type Checking** | O(n·α(n)) | Union-Find |
| **Proof Engine** | O(2^b) | b = branches, pruned |
| **Code Gen** | O(n) | Single pass |

---

*Last updated: 2026-05-06*
