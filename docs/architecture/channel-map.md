<!-- 2026-06-12 -->

# Channel Map — Data Flow Between Compiler Passes

## Pipeline

```
Source text
  │
  ▼
Lexer ──────────► Vec<Token>
  │
  ▼
Parser ─────────► Program { items: Vec<TopLevel>, comments, attrs, exit_condition }
  │               TopLevel::Statement(Box<Statement>) — top-level executable stmts
  ▼
Type-Universe ────► TypeUniverse (frozen map of resolved type metadata)
  │
  ▼
Import Resolver ──► Program (resolved paths, validated imports, synthesized imports)
  │                 Accepts `sed_item_names: Vec<String>` from Parser
  │                 Filters sed items from exported symbols via filter_items()
  │                 Cache: HashMap<String, (Program, Vec<String>)>
  │
  ├──► Program::synthesize_init_txn() — wraps TopLevel::Statement in __init txn
  │     (called in run_llvm_compile and run_check after import resolution)
  ▼
Desugarer ────────► Program (sugar constructs lowered to core AST)
  │                 @"..." → Expr::RegexLiteral
  │                 name#() → Expr::IntrinsicCall
  │                 Struct derivation: flatten parent fields into child structs
  │                   before state generation. Handles chain inheritance,
  │                   detects field name collisions, preserves parent link
  │                   for type system queries.
  ▼
Typechecker ──────► Program (annotated with types), TypecheckContext
  │                 Routes Expr variants through ExprTypecheck trait
  │                 Visibility enforcement: enforce_field_visibility()
  │                   — Sedentary cross-file access → TypeError
  │                   — Public allowed everywhere (Private stubbed)
  │                 Struct derivation: is_derived_from() walks parent chain
  │                   — B <: A → types_compatible(B, A) == true
  ▼
EqSaturation ─────► Program (simplified via 5-pass fixpoint rewrite)
  │                 9 rules: add-zero, mul-one, sub-self, double-neg, etc.
  ▼
Proof Engine ──────► Vec<ProofError> (contracts verified symbolically)
  │
  ▼
Annotator ────────► Program (file-level attributes processed)
  │
  ▼
Analysis ─────────► AnalysisResult
  │                 ├── CallGraph ──► backend dispatch selection
  │                 ├── Dataflow ───► prior-state field analysis
  │                 ├── TransitionGraph ──► dispatch collapse
  │                 ├── SLP Hazard ──► compute_peak_live_floats → SLP enable/disable
  │                 ├── RegionAnalyzer ──► precomputation budget
  │                 └── PGO ──► branch weight annotations for LLVM
  │
  ▼
Codegen ──────────► Output (LLVM IR / VHDL / Webstack / C)
  │                 ├── Direct SSA loop (A006) — no async, no MMIO
  │                 ├── Enum dispatch — folded multi-txn
  │                 └── Reactor tick — async triggers or MMIO
  │
  ▼
LLVM Pipeline ────► Binary
  ├── llvm-link (LTO merge with brief_rt.c)
  ├── opt -O3 -ffast-math
  └── llc -O3 --mcpu=native
```

## Top::Statement Synthesis Flow

```
Parser produces TopLevel::Statement(s)
  │
  ▼
Program::synthesize_init_txn() called after import resolution:
  1. Collect all TopLevel::Statement indices in order
  2. Remove from program.items
  3. Find unique __booted_N name (check N=0..63)
  4. Create StateDecl { name: "__booted_N", ty: Int, expr: Integer(0) }
  5. Create synthesized body: [stmts..., &__booted_N = 1, term;]
  6. Create rct txn __init [!__booted_N][__booted_N] { body }
  7. Prepend state decl, append __init txn to program.items
```

## IntrinsicCall Routing

```
Parser produces Expr::IntrinsicCall { intrinsic, args }
  │
  ▼
Typechecker: infers return type per intrinsic table (29 entries)
  │
  ▼
Interpreter: dispatches on Intrinsic enum — native Rust implementation
  │             ├── println# → println!("{}", v)
  │             ├── read_file# → std::fs::read_to_string
  │             └── sort# → passthrough (no-op in interpreter)
  │
  ▼
LLVM Backend: dispatches on Intrinsic enum
                ├── Sqrt → call float @llvm.sqrt.f32
                ├── Println → printf with per-type format
                └── Socket → add i64 0, -1 (stub)
```

## Universal Bracket Routing

```
Bracket expression parsed as:
  val[start..end]      → Expr::Slice { value, start, end }
  val[coord...]         → Expr::MultiSlice { value, ops }
  val; mask             → BracketOp::Mask(predicate)
  @"pattern"            → Expr::RegexLiteral(String)

  │
  ▼
DFA Compiler (analysis/dfa.rs):
  @"pattern" literal → compile_to_dfa() → RegexPattern (DFA table)
                     → Value::Regex(RegexPattern)

  │
  ▼
Interpreter:
  SliceExpr::evaluate() → decompose_atomic_to_chars() for atomics
  MultiSliceExpr::evaluate() → char decomposition + ops + reconstruction
  eval_mask_condition() → Value::Bool | Value::Regex | Value::String

  Type-directed desugar:
    atomic_val["string"] → coord desugars to regex filter on chars

  │
  ▼
LLVM Backend:
  Expr::Slice / MultiSlice on atomics → passthrough (coord) or stub (stride/mask)
  Expr::RegexLiteral → string constant pointer
```

## Self-Loop Elimination (2026-06-11)

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

```
Program with rct async txn @NHz or trg declarations
  │
  ▼
Reactor::build_from_program()
  ├── ReactiveTransaction constructed per rct txn:
  │     name, contract, body, is_async, reactor_speed, dependencies
  │     is_async and reactor_speed copied directly from parsed Transaction
  ├── TopLevel::Trigger stored in triggers map:
  │     HashMap<String, TriggerDeclaration>
  └── last_fired timestamps initialized: vec![Instant::now(); N_transactions]

  │
  ├── run_reactor (convergent, existing):
  │     Loop on dirty_preconditions → convergence → break
  │     Used for programs without async/triggers
  │
  └── run_reactor_continuous (event-driven, new):
        Loop:
          (1) reactor.run(interp)         — responsive convergence
          (2) reactor.fire_due_async_txns  — polled @Hz transactions
          (3) reactor.run(interp)         — catch cascades from (2)
          (4) sleep 1ms                   — yield to OS

fire_due_async_txns logic:
  for each transaction with is_async && reactor_speed.is_some():
    interval_ms = 1000 / hz
    if last_fired[idx].elapsed() >= interval_ms:
      mark dirty, fire transaction, update last_fired[idx]
```
