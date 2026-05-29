# Phase 4: Selfhost CLI Command

**Timestamp**: 2026-05-28T14:48:35Z  
**Prerequisites**: Route B complete (keywords accepted as identifiers, all 18 `.bv` files parse)  
**Rust compiler**: ✓ `cargo build`, ✓ `cargo test --lib` — 269/269 pass  

---

## Objective

Wire the Rust interpreter to `lib/compiler/main.bv` so that `main.bv` can compile Brief programs — producing its own backend output (initially stubs, eventually real C/Rust).

This is the bootstrap bridge: Rust runs `.bv` until `.bv` can compile itself.

---

## Plan

### Step 1: Fix `frgn` location mapping in `lib/std/io.bv`

**Problem**: `__read_file` and `__write_file` are declared as `frgn` without `from "..."` clauses. The Rust FFI registry maps them by location string (`"std::fs::read_to_string"`), but the name-to-location resolution depends on DBVS binding files which don't exist for these functions.

**Fix**: Add `from "..."` clauses:
```brief
frgn __read_file(path: String) -> Result<String, String> from "std::fs::read_to_string";
frgn __write_file(path: String, content: String) -> Result<Void, String> from "std::fs::write";
```

Same for `__file_exists`, `__delete_file`, `__list_directory`, `__create_dir` if they ever get used.

### Step 2: Add `selfhost` CLI subcommand in `src/main.rs`

A new subcommand:
```
brief-compiler selfhost <file.bv> [--backend c|rust] [--verbose]
```

CLI dispatches to `run_selfhost()` which:

1. Reads `lib/compiler/main.bv` and parses it
2. Resolves all imports (token, ast, lexer, parser, std.*, etc.)
3. Loads the resolved program into an Interpreter
4. Looks up the `compile_file` definition
5. Calls it with the user's file path as an argument
6. Prints the returned string

### Step 3: Wire Interpreter + ImportResolver

The interpreter already has:
- `Interpreter::new()` — constructor
- `Interpreter::load_program(&program)` — loads definitions, transactions
- `Interpreter::run()` — executes the reactive loop
- `Interpreter::call_function(name, args)` — directly calls a named function

The import resolver already has:
- `ImportResolver::new()` — constructor
- `ImportResolver::resolve_imports(&mut self, program, file_path)` — splices imported modules

The missing piece: a function that chains them together.

### Step 4: Handle `compile_file` entry point

`main.bv:45`: `defn compile_file(path: String, backend: String, verbose: Bool) -> Result<String, String>`

The interpreter will evaluate this when called. It:
- Calls `__read_file(path)` (FFI → reads file from disk)
- Tokenizes → Lexer
- Parses → Parser
- Analyzes call graph → CallGraph
- Dispatches to backend

The backends are stubs for now, but the pipeline will execute and produce output.

---

## Implementation Details

### `src/main.rs` changes

Add a match arm in the CLI dispatch:
```rust
"selfhost" => {
    let file = args.positional_arg()?;
    let backend = args.get::<String>("--backend").unwrap_or("c".to_string());
    let verbose = args.has("--verbose");
    run_selfhost(file, backend, verbose)?;
}
```

### `run_selfhost()` function

```rust
fn run_selfhost(file: &str, backend: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    // 1. Read and parse main.bv
    let main_source = std::fs::read_to_string("lib/compiler/main.bv")?;
    let main_tokens = tokenize(&main_source, "lib/compiler/main.bv");
    let main_program = parse_program(&main_tokens, "lib/compiler/main.bv")?;
    
    // 2. Resolve imports
    let mut resolver = ImportResolver::new();
    let resolved = resolver.resolve_imports(main_program, "lib/compiler/main.bv")?;
    
    // 3. Load into interpreter
    let mut interpreter = Interpreter::new();
    interpreter.load_program(&resolved)?;
    
    // 4. Call compile_file
    let args = vec![
        Value::String(file.to_string()),
        Value::String(backend.to_string()),
        Value::Bool(verbose),
    ];
    let result = interpreter.call_function("compile_file", args)?;
    
    // 5. Print result
    match result {
        Value::Result(Ok(inner)) => {
            if let Value::String(output) = *inner {
                println!("{}", output);
            }
        }
        Value::Result(Err(inner)) => {
            if let Value::String(err) = *inner {
                eprintln!("Error: {}", err);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Unexpected result type");
            std::process::exit(1);
        }
    }
    
    Ok(())
}
```

### ImportResolver integration

The `ImportResolver` needs to be initialized with the working directory as a search path so it can find both `lib/std/io.bv` (stdlib) and `lib/compiler/*.bv` (compiler lib).

The stdlib files (`lib/std/*.bv`) use `import from "..."` syntax with actual filesystem paths — these need to work too.

### Risk: `main.bv` imports `std.option`, `std.result`

These are `.bv` stdlib files that need to exist and parse. Check they're present and clean.

---

## Files Changed

| File | Change |
|---|---|
| `lib/std/io.bv` | Add `from "..."` to all `frgn` declarations |
| `src/main.rs` | Add `selfhost` subcommand + `run_selfhost()` function |
| `lib/compiler/main.bv` | **No change needed** — already has `compile_file` entry point |

---

## Verification

1. `cargo build` — compiles with new subcommand
2. `cargo test --lib` — all 269 tests pass
3. `brief-compiler selfhost examples/counter.rbv` — produces backend output (stub or real)
4. Backends not imported by `main.bv` (wasm, x86_64, aarch64, webstack) don't affect the pipeline

---

## Future Work

After Phase 4:
- **Phase 5**: Implement `backends/c.bv` codegen (translate all AST nodes to C)
- **Phase 6**: Compile `main.bv` with itself → bootstrap C output
- **Phase 7**: `gcc` the C output → standalone Brief compiler