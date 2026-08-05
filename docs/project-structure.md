# Briv Project Structure

> Extracted from the README (2026-07-31).
> Note: the tree is a summary; the canonical layout is the repository itself.

```
briv-compiler/
├── src/                          # Rust bootstrap compiler
│   ├── main.rs                   # CLI: check, build, compile, bind, metrod, lsp
│   ├── lib.rs                    # Crate root
│   ├── ast.rs                    # AST definitions
│   ├── parser.rs                 # Rust parser
│   ├── import_resolver.rs        # Module import resolution
│   ├── desugarer.rs              # AST desugaring
│   ├── typechecker.rs            # Type checker
│   ├── proof_engine.rs           # Proof engine + CallGraph integration
│   ├── lsp.rs                    # LSP server (hover, definition, completions, symbols)
│   ├── reactor.rs                # Reactive runtime
│   ├── signal_graph.rs           # Signal dependency tracking
│   │
│   ├── analysis/                 # Shared program analysis
│   │   ├── call_graph.rs         # Transaction call graph + cycle detection
│   │   ├── range.rs              # Parameter bounds inference
│   │   ├── dataflow.rs           # Read/write dependency analysis
│   │   ├── protocol.rs           # Control register prerequisites
│   │   ├── address_space.rs      # Memory address classification
│   │   ├── cross_reference.rs    # Address validation
│   │   ├── entry_point.rs        # Triggerable transaction discovery
│   │   └── struct_generator.rs   # State struct generation
│   │
│   ├── backend/                  # Three canonical code generation backends
│   │   ├── llvm/                 # LLVM IR — native, embedded, SPIR-V (active)
│   │   ├── webstack.rs           # TypeScript + WASM — web target (active)
│   │   ├── circt.rs              # CIRCT MLIR — hardware target (active)
│   │   └── mod.rs                # Backend registry + dispatch
│   │
│   └── archive/backend/          # Retired backends (preserved for reference)
│       ├── aarch64.rs            # ARM64 assembly (archived)
│       ├── x86_64.rs             # AMD64 assembly (archived)
│       ├── rust.rs               # Rust source (archived)
│       ├── c.rs                  # C source (archived)
│       ├── wasm.rs               # WASM text format (archived)
│       ├── cobol.rs              # COBOL source (archived)
│       ├── vhdl.rs               # VHDL (archived)
│       ├── verilog.rs            # SystemVerilog (archived)
│       ├── tcl_generator.rs      # Vivado TCL (archived)
│       └── webstack_rust_codegen.rs  # Old Rust/wasm-bindgen webstack (archived)
│   │
│   ├── ffi/                      # Foreign Function Interface
│   │   ├── metropolitan.rs       # Shared memory IPC (876 lines)
│   │   ├── orchestrator.rs       # Native + Metropolitan dispatch
│   │   ├── registry.rs           # 60+ Rust impl functions (892 lines)
│   │   ├── sentinel.rs           # Pre/post-condition validation
│   │   ├── native_mapper.rs      # Byte serialization
│   │   ├── loader.rs             # DBVS binding file loader
│   │   ├── resolver.rs           # Binding path resolution
│   │   ├── metro_cli.rs          # `briv metrod connect` CLI (661 lines)
│   │   ├── types.rs              # FfiValue, MemoryLayout, FfiType
│   │   ├── error.rs              # Error conventions
│   │   ├── protocol.rs           # Mapper trait
│   │   ├── mapper.rs             # Mapper registry
│   │   ├── mappers.rs            # Built-in mappers
│   │   ├── script.rs             # Script function resolution
│   │   ├── validator.rs          # Binding validation
│   │   └── mod.rs                # FFI crate root
│   │
│   ├── dbriv/                   # Data Briv (DBVS) subsystem
│   │   ├── ast.rs                # DBVS AST + Fn/Trigger/Result types
│   │   ├── parser.rs             # DBVS parser
│   │   └── ...                   # DBVS compiler
│   │
│   ├── wrapper/                  # Library wrapper/bindings generator
│   │   ├── generator.rs          # DBVS + bridge.bv + foreign stub gen
│   │   ├── c_analyzer.rs         # C header parser
