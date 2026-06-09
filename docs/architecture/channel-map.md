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
  │
  ▼
Proof Engine ──────► Vec<ProofError> (contracts verified symbolically)
  │
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

As features are migrated to `src/features/`, each feature module
implements its own `parse()`, `typecheck()`, `evaluate()`, and
per-backend codegen methods. The pass files become thin routers
that delegate to feature modules. The data contracts above remain
unchanged — only the internal dispatch mechanism changes.
