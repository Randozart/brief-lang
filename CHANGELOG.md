# Changelog

## Unreleased

### Language

- **Enums**: Added `enum` declarations with Unit, Tuple, and Struct variants. Supports type parameters (e.g. `enum Result<T, E> { Ok(T), Err(E) }`).
- **Pattern matching**: New `[value Variant(field1, field2)]` guard syntax for destructuring enum variants and binding fields to variables. Works with identifiers and keyword variants (`Ok`, `Err`).
- **JSON serialization**: Built-in `to_json(value) -> String` and `from_json(json_str) -> Result<Object, String>` functions. `from_json` returns a `Result` enum that must be pattern-matched.
- **`b-style` directive**: Reactive style bindings in views (`b-style="property: signal"`).

### Compiler

- **Lexer**: Added `enum`, `Ok`, `Err`, `match` tokens (`src/lexer.rs`).
- **AST**: Added `EnumDefinition`, `EnumVariant` (Unit/Tuple/Struct), `Type::Enum`, `Expr::PatternMatch`, `TopLevel::Enum`. Changed `SvgComponent(String)` to `SvgComponent { name, content }` (`src/ast.rs`).
- **Parser**: Added `parse_enum()` for enum declarations. Extended guard parsing to detect pattern match expressions (`src/parser.rs`).
- **Typechecker**: Added `Type::Enum` compatibility checks, stdlib signature registration for `to_json`/`from_json`, foreign sig collection, and `Expr::PatternMatch` inference (`src/typechecker.rs`).
- **Interpreter**: Added `Value::Enum` for runtime enum values. Pattern matching evaluates variant and binds fields. `to_json` serializes instances/lists/enums. `from_json` returns `Result::Ok` or `Result::Err` (`src/interpreter.rs`).
- **Import resolver**: SVG imports now extract component name from `as` alias or derive from filename. File-based imports (`.css`, `.svg`) preserve slash paths (`src/import_resolver.rs`).
- **Wasm codegen**: Added JS FFI glue for `__json_decode`, `__json_get_string`, `__json_encode`, `__http_get`, `__http_post`. Added `attr` and `style` directive rendering in patch engine (`src/wasm_gen.rs`).
- **View compiler**: Added `b-style` directive parsing and `Style` binding variant (`src/view_compiler.rs`).
- **Annotator/Proof engine/Symbolic/Reactor**: Updated all passes to handle `Type::Enum`, `Expr::PatternMatch`, and `TopLevel::Enum` (`src/annotator.rs`, `src/proof_engine.rs`, `src/symbolic.rs`, `src/reactor.rs`).

### Stdlib

- **HTTP module**: New `lib/std/http.bv` with `http_get` and `http_post` wrappers over `__http_get`/`__http_post` FFI.

### Documentation

- **Language reference**: Added sections for Enum declarations, Enums with Data, Pattern Matching syntax, and JSON Serialization (`spec/LANGUAGE-REFERENCE.md`).
