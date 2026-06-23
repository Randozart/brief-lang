<!-- 2026-06-18 -->

# Channel Map — Data Flow Between Compiler Passes

## Pipeline

```mermaid
graph TD
    S[Source text] --> Lex[Lexer]
    Lex -->|Vec Token| Par[Parser]
    Par -->|Program| TU[Type-Universe]
    TU -->|TypeUniverse| IR[Import Resolver]
    IR -->|Resolved Program| Des[Desugarer]

    IR --> Init[Program::synthesize_init_txn<br>wraps TopLevel::Statement in __init txn]

    Des --> T1a[Phase 1a: Template Expansion]
    Des --> T1b[Phase 1b: Macro Expansion]
    T1a --> T1b

    Des --> TC[Typechecker]
    TC -->|Typed Program| Eq[EqSaturation]
    Eq -->|Simplified Program| PE[Proof Engine]
    PE -->|ProofResults| An[Annotator]
    An -->|Annotated Program| Anl[Analysis]

    Anl --> CG[CallGraph]
    Anl --> DG[DependencyGraph]
    Anl --> DF[Dataflow]
    Anl --> TG[TransitionGraph]
    Anl --> SLP[SLP Hazard]
    Anl --> RA[RegionAnalyzer]
    Anl --> PGO[PGO]

    Anl --> Codegen[Codegen]
    Codegen -->|LLVM IR| LLVM[LLVM Pipeline]
    Codegen -->|MLIR| CIRCT[CIRCT]
    Codegen -->|TS + WASM| Web[Webstack]

    LLVM --> Link[llvm-link LTO with brief_rt.c]
    Link --> Opt[opt O3 ffast-math]
    Opt --> LLC[llc O3 mcpu=native]
    LLC --> Bin[Binary]

    style Init fill:#55b,color:#fff
    style T1a fill:#55b,color:#fff
    style T1b fill:#55b,color:#fff
```

## Top::Statement Synthesis Flow

```mermaid
graph TD
    P[Parser produces TopLevel::Statement] --> Syn[Program::synthesize_init_txn<br>after import resolution]
    Syn --> C1[1. Collect all TopLevel::Statement indices]
    C1 --> C2[2. Remove from program.items]
    C2 --> C3[3. Find unique __booted_N name]
    C3 --> C4[4. Create StateDecl __booted_N: Int = 0]
    C4 --> C5[5. Synthesize body: stmts + booted = 1 + term]
    C5 --> C6[6. Create rct txn __init]
    C6 --> C7[7. Prepend state decl, append __init]
```

## IntrinsicCall Routing

```mermaid
graph LR
    subgraph Parser[Parser]
        Intrin[IntrinsicCall expr]
    end
    Intrin --> TC[Typechecker]
    TC -->|annotated| Interp[Interpreter]
    TC -->|annotated| LLVM[LLVM Backend]

    Interp -->|evaluate| Result[Intrinsic return value]
    LLVM -->|emit| IR[LLVM IR intrinsic call]
```

## Universal Bracket Routing

```mermaid
graph TD
    BR[BracketOp / RegexLiteral] --> DFA[DFA Compiler]
    DFA -->|state machine| Interp2[Interpreter]
    DFA -->|state machine| LLVM2[LLVM Backend]
```

## Import Target Routing (2026-06-19)

The `(wasm) import`, `(circt) import`, and `(javascript) import` syntaxes
set `Import.target` on the AST node:

| Syntax | `ImportTarget` | Webstack behavior | LLVM behavior |
|--------|----------------|-------------------|---------------|
| `import "path"` | `Native` | Inline as TS | Inline as LLVM IR |
| `(wasm) import` | `Wasm` | Queue for LLVM wasm32, base64-embed in TS | Compile to wasm32 |
| `(circt) import` | `Circt` | Error | Route to CIRCT backend |
| `(javascript) import` | `Javascript` | Inline JS from `wasm_impl` field | Error |

(WASM sidecar compilation is Phase B — pending wasm32 target support.)

```
Precondition failure in direct SSA loop:
  Before: br i1 %ok, label %body, label %skip_l → skip_l loops back to tick
  After:  br i1 %ok, label %body, label %done_{name} → done_{name}: br done

Body completion in direct SSA loop:
  skip_l: (still loops back to tick for normal iteration)
```

## loop_exit_label (2026-06-11)

```
Before body emission:  self.loop_exit_label = Some("done".into())
After body emission:   self.loop_exit_label = None

During body emission, TermBang handler checks loop_exit_label:
  If set: emit br label %done (instead of ret)
  If None: emit ret (original behavior, for non-loop contexts)
```

## Reactor / Async / Trigger Flow (2026-06-11)

```mermaid
graph TD
    PB[Program Build] --> Ana[Analysis]
    Ana -->|determines| Sel{Dispatch type?}
    Sel -->|No async, no MMIO| SSA[Direct SSA loop A006]
    Sel -->|Bounded triggers| Enum[Enum dispatch folded]
    Sel -->|Async or MMIO| React[Reactor tick]

    React --> Conv[Convergence loop:<br>pre body post]
    Conv -->|triggers| Event[Event-driven loop:<br>epoll wait step]
```

## GPU Offloading Flow (2026-06-18)

```mermaid
graph TD
    GPUMark[#gpu annotated txn] --> Check[check_eligibility]
    Check -->|passes| SPIRV[emit_spirv_module]
    SPIRV --> LLCSPV[llc mtriple=spirv64]
    LLCSPV --> Vulkan[Vulkan / OpenCL runtime]
    Check -->|fails| Warn[Warn: GPU-ineligible types]
```
