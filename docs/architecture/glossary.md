<!-- 2026-06-09 -->

# Brief Compiler Glossary

| Term | Definition |
|------|------------|
| **Pattern B** | Struct-Variant Delegation architecture. Each AST construct is a struct in its own file with co-located parse/typecheck/eval/codegen. |
| **Feature file** | A file in `src/features/` containing one coherent language construct group, its struct definition, and all pass implementations. |
| **Router** | A thin dispatch function in a main pass file (e.g., `infer_expr` in `typechecker.rs`) that matches on an enum variant and delegates to the corresponding feature struct method. |
| **Praetor** | A strict LSP that enforces complexity limits: cyclomatic ≤ 15, cognitive ≤ 15, lines ≤ 100, params ≤ 6, nesting ≤ 6. |
| **ExprDispatch** | A handle passed to feature struct methods so they can recursively dispatch sub-expressions back through the router. |
| **Type-Universe** | Pass 1: collects all `Type Name <: Base` declarations, resolves derivation chains, inherits/overrides metadata, and freezes the type map for Pass 2. |
| **TypeDef** | A `TopLevel::TypeDef` variant representing a `Type Name <: Base { ... }` declaration. |
| **TypeProperty** | An enum with 13 variants (Bytes, Alignment, Endian, Volatile, Atomic, ElementType, FixedSize, InsertAt, ExtractFrom, AllowIndex, AllowSlice, AllowArrow, Codec) — the compiler's primitive kernel. |
| **Primitive Kernel** | The ~13 type properties the Rust compiler hardcodes. Everything else (`String`, `Stack`, `Queue`, `HashMap`, etc.) is defined in user-space Brief. |
