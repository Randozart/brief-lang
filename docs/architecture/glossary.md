<!-- 2026-06-09 -->

# Brief Compiler Glossary

| Term | Definition |
|------|------------|
| **Pattern B** | Struct-Variant Delegation architecture. Each AST construct is a struct in its own file with co-located parse/typecheck/eval/codegen. |
| **Feature file** | A file in `src/features/` containing one coherent language construct group, its struct definition, and all pass implementations. |
| **Router** | A thin dispatch function in a main pass file (e.g., `infer_expr` in `typechecker.rs`) that matches on an enum variant and delegates to the corresponding feature struct method. |
| **ExprDispatch** | A handle passed to Expr feature struct methods so they can recursively dispatch sub-expressions back through the router. |
| **StmtDispatch** | A handle passed to Statement feature struct methods for recursive sub-statement dispatch. Analogous to `ExprDispatch`. |
| **StmtTypecheck** | Trait for typechecking a statement variant. Methods take `&mut TypeChecker` + `&StmtDispatch`. |
| **StmtEval** | Trait for evaluating a statement variant in the interpreter. |
| **StmtCodegenLLVM / VHDL / Webstack** | Traits for per-backend statement code generation. |
| **Type-Universe** | Pass 1: collects all `Type Name <: Base` declarations, resolves derivation chains, inherits/overrides metadata, and freezes the type map for Pass 2. |
| **TypeDef** | A `TopLevel::TypeDef` variant representing a `Type Name <: Base { ... }` declaration. |
| **TypeProperty** | An enum with 13 variants (Bytes, Alignment, Endian, Volatile, Atomic, ElementType, FixedSize, InsertAt, ExtractFrom, AllowIndex, AllowSlice, AllowArrow, Codec) — the compiler's primitive kernel. |
| **Primitive Kernel** | The ~13 type properties the Rust compiler hardcodes. Everything else (`String`, `Stack`, `Queue`, `HashMap`, etc.) is defined in user-space Brief. |
| **check_convergence** | A function in `proof_engine.rs` that syntactically detects convergence patterns (increments, popcount decay) without symbolic execution. |
| **eval_const_expr** | A helper that evaluates pure-integer constant expressions, resolving identifiers through the `initial_values` map. |
| **extract_var_relation** | A helper that pulls the counter-variable-involving sub-expression out of AND/OR preconditions. |
| **is_self_minus_one** | A helper that detects `reg & (reg - 1)` popcount decay patterns. |
| **enumerate_paths_recursive** | The path exploration core. Iterates through a sequence of statements, forking on guarded branches, collecting paths that end in `term`. |
| **Praetor** | A strict LSP that enforces complexity limits: cyclomatic ≤ 15, cognitive ≤ 15, lines ≤ 100, params ≤ 6, nesting ≤ 6. |
| **Feature struct** | A Rust struct in a feature file representing one AST construct. Has its own fields (not enum variants). Implements the relevant traits. |
