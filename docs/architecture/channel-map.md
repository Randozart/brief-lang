<!-- 2026-06-09 -->

# Channel Map — Data Flow Between Compiler Passes

## Pipeline

```
Source text
  │
  ▼
Lexer ──────────► Vec<Token>
  │
  ▼
Parser ─────────► Program { items: Vec<TopLevel>, comments, attrs, ... }
  │
  ▼
Type-Universe ────► TypeUniverse (frozen map of resolved type metadata)
  │                 TypeDef declarations validated, chain-resolved
  ▼
Import Resolver ──► Program (resolved paths, validated imports)
  │
  ▼
Desugarer ────────► Program (sugar constructs lowered to core AST)
  │
  ▼
Typechecker ──────► Program (annotated with types), TypecheckContext
  │                 Routes Expr variants through ExprTypecheck trait
  │                 Routes Statement variants inline (infer_expression is private)
  ▼
Proof Engine ──────► Vec<ProofError> (contracts verified symbolically)
  │                 check_convergence: syntactic convergence detection
  │                   - AND/OR precondition extraction
  │                   - Popcount decay (reg & (reg - 1))
  │                   - Algebraic cancellation (count + (R + 1 - R))
  │                   - Compound increment ((count + N) - M)
  │                 enumerate_paths_recursive: path exploration
  │                   - Guard-taken paths now continue to term
  ▼
Annotator ─────────► Program (file attributes processed)
  │
  ▼
Analysis ──────────► AnalysisResults {
    call_graph, parameter_ranges, fusable_pairs,
    dataflow_errors, transition_graph, region_analyzer
  }
  │
  ▼
Codegen ───────────► String (target code)
  │                 Routes Expr variants through ExprCodegen* traits
  │                 Routes Statement variants inline (dispatch not yet migrated)
```

## Data Contracts Between Passes

*Parse → Type-Universe*: Program — TypeDef items collected, chain-resolved, frozen
*Type-Universe → Import*: Program + TypeUniverse (read-only reference for all subsequent passes)
*Parse → Import*: Program with unresolved imports marked
*Import → Desugar*: Fully resolved module graph
*Desugar → Typecheck*: Core-level AST (no sugar)
*Typecheck → Proof*: Typed AST, inferred types for all expressions
*Proof → Annotator*: Program with verified contracts
*Annotator → Analysis*: Fully attributed program
*Analysis → Codegen*: AnalysisResults + fully analyzed Program

## Pattern B Integration

### Expr Dispatch

Each pass file has a dispatch function (e.g., `eval_expr`, `infer_expression`,
`emit_expr`) that matches on `Expr` variants. New Pattern B variants route
through the `Expr*` traits; old variants are handled inline. In Phase 4,
BinaryOp and UnaryOp feature files received real (non-stub) implementations
for evaluation. All three backends route Pattern B variants through
`ExprCodegenLLVM`/`VHDL`/`Webstack` traits.

### Statement Dispatch

Statement features are defined in `src/features/stmt/` with 5 stub trait
implementations each (StmtTypecheck, StmtEval, StmtCodegenLLVM/VHDL/Webstack).
The dispatch migration is deferred to Phase 4 — pass files still match on
old Statement::Assignment { ... } variants directly.

### TopLevel Dispatch

TopLevel features are defined in `src/features/toplevel/` with stub struct
definitions referencing the existing AST types. The pass files iterate
over `program.items` matching on `TopLevel::Transaction`, `TopLevel::Struct`,
etc. directly. No trait dispatch exists for TopLevel items yet.

### Proof Engine

The proof engine uses `check_convergence` for syntactic convergence detection
and `verify_contract_implication` for symbolic post-condition checking. The
convergence analysis handles AND/OR preconditions, popcount decay patterns,
algebraic cancellation, and compound increment patterns. Guard-taken paths
are fully explored through to `term`. All benchmarks pass type-checking.
