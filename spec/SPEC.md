# Brief Language Specification

**Version:** v0.18.0  
**Date:** 2026-07-12  
**Status:** Development (Phases 0-7 complete: extensible types, property system, codec declarations, custom literal parsers, WASM target, plugin system. Phases 8.0-8.2 complete: derivation block lexer/parser/AST. Phases 8A-8F in progress: Pure Bits refactor.)  
**Language Variants:** Core (.bv), Rendered (.rbv), Embedded (.ebv), Accelerated (.abv), Circuit (.cbv), Data (.dbv, .dbvs, .dbvl), plus extension modifiers (.s, .f, .c)

## 1. Introduction and Philosophy

Brief is a declarative, contract-enforced logic language designed for building verifiable state machines. It treats program execution as a series of verified state transitions rather than sequential instructions.

Brief is designed for **Formal Verification without the Boilerplate**. It eliminates imperative control flow (`if`, `else`, `while`) in favor of contracts, guards, and atomic transactions.

### 1.1 Core Design Principles

1. **Contracts First**: Every transaction declares what must be true before and after it runs. The compiler verifies these contracts.
2. **Atomic State Transitions**: Transactions are atomic - they either complete fully or roll back completely.
3. **Reactive Execution**: Brief programs use a reactor model where transactions fire automatically when their preconditions are met.
4. **Zero-Nesting Logic**: Branching is handled via guards, not nested blocks. This improves clarity and LLM comprehension.
5. **FFI for External Capabilities**: Brief cannot do everything (file I/O, networking, hardware math). Foreign Function Interface handles these cases with explicit contracts.

### 1.2 Language Variants and Extension Modifiers

Brief uses a **base variant** extension (`.bv`, `.ebv`, etc.) with optional
**modifier flags** in the filename as a middle segment. The format is
`[name].[modifiers].[variant]` where modifiers are single-character flags
in any order.

| Flags | Meaning | Effect |
|-------|---------|--------|
| `s` | Strict | Extra compiler verification passes |
| `f` | Formatted | Indentation-based layout (no braces/semicolons) |
| `c` | Cell | File becomes `cell <stem> { ... }` with `input`/`output` keywords |

**Examples:**

| Filename | Variant | Modifiers | Meaning |
|----------|---------|-----------|---------|
| `main.bv` | `bv` | — | Standard Brief |
| `main.s.bv` | `bv` | `s` | Strict |
| `main.f.bv` | `bv` | `f` | Formatted (indentation) |
| `main.sf.bv` | `bv` | `s`, `f` | Strict + Formatted |
| `server.c.bv` | `bv` | `c` | Cell-wrapped |
| `server.sfc.bv` | `bv` | `s`, `f`, `c` | Strict + Formatted + Cell |
| `sensor.c.ebv` | `ebv` | `c` | Cell-wrapped Embedded |

**Deprecation:** The old single-segment `.sbv`, `.srbv`, `.sebv` extensions
are superseded by `.s.bv`, `.s.rbv`, `.s.ebv`. The old forms continue to
compile during a deprecation window but emit a warning.

**Base Variants:**
* **Core Brief** (`.bv`): Transactional state machines with FFI support. Compiles to native binary via LLVM backend.
* **Rendered Brief** (`.rbv`): Adds `render struct`/`render obj`, view components (HTML/CSS/SVG), and UI binding directives (b-text, b-show, b-trigger). Compiles to WASM + JS shim via Webstack backend (LlvmBackend wasm32 + GlueWebGenerator). `rstruct` deprecated.
* **Embedded Brief** (`.ebv`): Adds native `Float` types, vector types, bit-range addressing, and hardware triggers (`trg`). Compiles to microcontroller binary via LLVM backend.
* **Accelerated Brief** (`.abv`): GPU compute kernels. Compiles to SPIR-V via LLVM backend (GPU intrinsics, no FFI).
* **Circuit Brief** (`.cbv`): Pure hardware logic. Compiles to Verilog/VHDL via CIRCT backend (no FFI, no external deps).
* **Data Brief Schema** (`.dbvs`): Schema definitions for Data Brief, including aliases and validation rules.
* **Data Brief Lines** (`.dbvl`): Line-based mutable database for large datasets with verification.
* **Rendered Brief** (`.rbv`): Combines Strict Brief enforcement with verified view-state isomorphism. Same targets as `.rbv`.
* **Strict Embedded Brief** (`.s.ebv`): Strict Brief for hardware targets. Same targets as `.ebv` with additional strictness.

### 1.3 Versioning

* **Semantic**: `v0.18.0` (development, Phases 4-7 complete)
* **Date-based**: `2026-07-11`

### 1.4 Compiler Architecture

```
Source (.bv/.rbv/.ebv/.abv/.cbv/.dbv)
  ↓
Lexer (src/lexer.rs)
  ↓ Token stream
Parser (src/parser/)  — defn / txn / node, contracts in either position,
  ↓                     .^ / .^^ reflection, obj generics + methods
AST
  ↓
Import Resolver (src/import_resolver.rs)
  ↓ Resolved AST
Type Universe Build (src/type_universe.rs)  — hashword protocols, casting graph
  ↓ Frozen universe
NormalizeTypes Pass (src/backend/llvm/normalizer.rs)  — registers types; the
  ↓                                              casting graph resolves LLVM types
Type Checker (src/typechecker/)  — declaration-level validation (let/term/args),
  ↓                         field/method/reflection resolution, no implicit coercion
Proof Engine (src/proof_engine/)  — tautology detection, convergence
  ↓
Shared Analysis (src/analysis/)  — frontend-driven: LoopShape, swan songs,
  ↓                         density, modulo partitions, inline decisions
Plugin Hooks (Front/Mid/Back)  — prelude injection, env intercepts
  ↓
Backends — consume frontend decisions; never re-derive them
  ├── LLVM (src/backend/llvm/) → .bv → native binary, .ebv → MCU, .abv → SPIR-V
  │   └── wasm32-wasi target via llc + wasm-ld
  ├── CIRCT (src/backend/circt.rs) → .cbv → Verilog/VHDL
  ├── Webstack (src/backend/webstack.rs) → .rbv → TS + WASM
  └── (archived: aarch64/x86_64/rust/c/wasm/verilog/vhdl assembler backends)
```
  ├── Webstack (webstack.rs + LlvmBackend wasm32) → .rbv → WASM + JS shim
  └── CIRCT (circt.rs) → .cbv → MLIR → Verilog/VHDL
```

**Backends (active):**

| Backend | Input | Output | Status |
|---------|-------|--------|--------|
| LLVM | `.bv`, `.ebv`, `.abv` | Native binary, MCU binary, SPIR-V, WASM | Active |
| Webstack | `.rbv` | WASM + JS shim + view bindings | Active (migrating to WASM-first) |
| CIRCT | `.cbv` | MLIR → Verilog/VHDL | Active |

9 retired backends (Rust, C, WASM text, COBOL, SystemVerilog, VHDL, AArch64, x86_64, TCL) are archived in `src/archive/backend/` for reference.

---

## 1.5. Symbolic Design Philosophy

Brief's symbols are not arbitrary ASCII choices. Each symbol's **visual shape** maps to a **cognitive metaphor**, which maps to a **systems meaning**. All uses of a given symbol share that core metaphor.

| Symbol | Visual Shape | Cognitive Metaphor | Systems Meaning | Group |
|:---:|---|---|---|---|
| **`;`** | A dot with a tail falling away | A hard stop, a reset | Universal statement termination | — |
| **`.`** | A single pinpoint | Puncturing, reaching into | Struct field access / method call | — |
| **`->`** | An arrow pointing right | Forward motion | Dataflow / State transition | — |
| **`<-`** | An arrow pointing left | Backward motion | Mutation / Discard | **Transfer** |
| **`:`** | Two stacked dots | Identity, equivalence | Static type definition | — |
| **`.^` / `.^^`** | Pinpoint + caret(s) | Reflecting on a value/type | **Reflection** — runtime (`.^`) and compile-time (`.^^`) metadata | **Reflection** |
| **`[]`** | Brackets that enclose | Containment, boundary | Constraints, guards, partitions — segments a layout into addressable sub-ranges | **Partition** |
| **`{}`** | Curly braces that hug | Grouping, bundling | Code block / organization | — |
| **`()`** | Parentheses that cup | Holding, containing | Argument enclosure | — |
| **`@`** | The at-sign (loop + 'a') | Position, location, anchor | Spatial / Temporal / Dimensional / Chronological anchor | **Anchor** |
| **`&`** | Ampersand (et-ligature) | Connection, conjunction | Mutation marker (required) | — |
| **`!`** | Vertical line + dot | Exclamation, warning | Control flow anomaly / fire-and-forget | — |
| **`~`** | A wavy line | Oscillation, flipping | Boolean toggle / atomic lock | — |
| **`?`** | A hook | A question, a check | Watchdog / timeout | — |
| **`_`** | A small horizontal line | A gap, a placeholder | Ignored / unused value | — |

**The principle:** If an operation has distinct physical, temporal, or compiler-level behavior, its visual representation must explicitly reflect that boundary. No hidden transformations.

### Operator Taxonomy

Brief's operators fall into conceptual groups that govern how types relate, how data is partitioned, how data moves, and how metadata is read:

| Group | Operators | Purpose |
|-------|-----------|---------|
| **Reflection** | `.^` (runtime), `.^^` (compile-time) | Read compiler-known metadata about a value/type: `s.^Len`, `x.^^Bytes`. The targets are PascalCase compiler-known identifiers (explicitly marked). |
| **Partition Operators** | `[]`, `@/` | Constrain focus to a spatial sub-range of a layout. `list[3]` selects the 4th element; `bits @/0..3` selects bits 0-3. |
| **Transfer Operator** | `<-` | Directional data movement across layout boundaries. `&list <- x` pushes x into list; `val <- &list` pops from list. |

The **Anchor** (`@`) is not itself an operator but a universal modifier — it anchors a value to a position in space or time (`@` for prior state, `@/` for bit position, `@"..."` for compile-time strings).

### Delimiter Semantic Load

Each delimiter carries exactly one meaning — a delimiter used for the wrong
load is a design error, not a stylistic choice:

| Delimiter | Load | Examples |
|-----------|------|----------|
| `<>` | **compile-time type-level specialization** — a named kind of the thing | `Stack<T>`, `#String<UTF8>`, `asm<x86_64>`, `sync<group>` |
| `()` | **application & binding** — call it, construct it, bind an implementation | `f(a)`, `defn f(x: Int)`, `Person(...)`, `op Add: func(#L,#R)`, `op Add(Float)` |
| `[]` | **containment / bound** | `Int[8]`, `[pre]` guards |
| `{}` | **grouping / definition** | blocks, struct literals |

`sync<group>` uses `<>` because the group is a compile-time identity — the same
shape as `asm<chip>` (which target) and `#String<UTF8>` (which variant).
`op Add(Float)` stays `()`: `op` is a nested declaration, declarations take
params, and it avoids angle-bracket nesting.

### Modifiers (2026-07-31)

User-facing directives (`seq`, `vol`, `async`, `sync<group>`) are ordinary
keywords, **not** compiler-knowns (no `#`). They **must never make code
faster** — the default is always the efficient path; a modifier-beaten default
is a compiler bug. All modifiers are **prefix** (`async node`; `node async` is
rejected).

| Modifier | Meaning |
|----------|---------|
| `seq struct Name` | declared field layout preserved — no reordering/compaction/dead-field elimination |
| `seq txn foo` / `seq node foo` | sequential dispatch — never the parallel reactor |
| `seq Int[N]` / `seq foreach` | sequential access — `!llvm.loop.vectorize.enable = false` |
| `vol let x` | every access is `load volatile` / `store volatile` — never folded, never promoted |
| `async node foo` | explicit acknowledgement of simultaneous firing (a semantic declaration, not a hint) |
| `sync<group> node foo` | group barrier — members that fire hold off finishing until every fired group member has |

`seq` and `vol` are orthogonal and combinable: `vol seq let Int[x]` is a
volatile *and* sequential array.

**The concurrency gate (NO IMPLICIT CONCURRENCY):** for reactive nodes A and B,
if the proof engine proves `pre_A ∧ pre_B` satisfiable AND there is no XOR
read-write overlap, the pair is *eligible to fire together* and the compiler
DEMANDS a classification — `async` on both (explicit acknowledgement of
simultaneous firing) or `sync<group>` on both (group barrier). An unclassified
eligible pair is a **hard error**: "nodes A and B can fire together; declare
`async` on both or `sync<group>` on both." The escapes are provable mutual
exclusion (`pre_A ∧ pre_B` UNSAT) or an XOR read-write dependency forcing
sequential order.

---

## 2. Grammar Specification

### 2.1 Program Structure

```bnf
program ::= (top_level)*

top_level ::= definition
            | transaction
            | state_decl
            | constant
            | import
            | struct_def
            | render_struct_def
            | render_obj_def
            | enum_def
            | type_def       (* NEW 2026-06-09: Type Name : Base { ... } *)
            | signature
            | resource_decl
            | render_block
            | exit_condition
            | export_def
            | cell_input       (* Valid only in .c.bv files *)
            | cell_output      (* Valid only in .c.bv files *)

type_def ::= "type" identifier type_params? ":" type_expr "{" (slot_decl | type_property | op_decl | prop_decl | constraint)* "}" ";"
slot_decl ::= identifier ":" type_expr ";"
type_property ::= "!>" identifier ":" property_value ";"        (* metadata: !> bytes: 8; *)
property_value ::= int | string | identifier
constraint ::= "[" expression "]"
op_decl ::= "op" rune_name ":" fn_call                                (* binding: op Add: func(#L, #R); *)
          | "op" rune_name "(" rhs_type? ")" ":" fn_call             (* RHS-only overload, declared on the LHS type: op Add(Float): func(#L, #R); *)
(* The LHS operand type is the declaring type/protocol — it is never listed.
   op Add(#Int, #Int) (two operands) is REMOVED: protocols know their own
   self-arithmetic; only the RHS overload goes between the parens. *)
prop_decl ::= "prop" identifier "=" expression ";"

exit_condition ::= "#!exit" expression

definition ::= ("defn" | "def" | "definition") identifier type_params? parameters? "->" output_types contract body derivation?

transaction ::= ("async")? "node"? "txn" identifier type_params? parameters? contract body derivation?

body ::= "{" statement* "}" ";" | ";"

derivation ::= ":=" "{" derivation_example ((";" | ",") derivation_example)* "}" ";"

derivation_example ::= expression ("," expression)* "->" expression
                     (* Inputs -> output mapping, e.g. 2, 2 -> 4 *)

export_def ::= "export" definition
             (* Export a function for C/foreign linking *)

cell_input ::= "input" identifier ":" type ";"
             (* Only valid in .c.bv (cell-wrapped) files — declares a cell parameter *)

cell_output ::= "output" identifier ":" type ";"
              (* Only valid in .c.bv (cell-wrapped) files — declares the cell return type *)

signature ::= "sig" ("#inline")? identifier "(" parameters? ")" "->" output_type ("from" path | "=" identifier)? ";"
(* Note: `#out` modifier is removed from the language. Use `!> observable: true` metadata instead. *)

output_type ::= union_type
union_type ::= product_type ("|" product_type)*
product_type ::= array_type ("," array_type)*
array_type ::= [identifier ":"] type ("[" "]")?

binding ::= "=" identifier "(" arguments? ")" | "from" path

resource_decl ::= ("rsrc" | "resource") identifier ":" resource_type "(" arguments? ")" ";"

constant ::= ("const" | "constant") identifier ":" type "=" expression ";"

state_decl ::= "state" identifier ":" type ("=" expression)? ";"

struct_def ::= "struct" identifier "{" struct_member* "}"

struct_member ::= field_decl | transaction

field_decl ::= identifier ":" type ("=" expression)? ";"

rstruct_def ::= "rstruct" identifier "{" struct_member* view_body "}"  (* DEPRECATED 2026-07-26 — use render struct *)
render_struct_def ::= "render" "struct" identifier "{" view_body "}"
render_obj_def ::= "render" "obj" identifier "{" view_body "}"

enum_def ::= "enum" identifier type_params? "{" enum_variant ("," enum_variant)* ","? "}"

enum_variant ::= identifier ("(" type ("," type)* ")")?

import_stmt ::= "import" (import_items | string_literal ("as" identifier)?) ("from" path)? ";"

import_items ::= "{" import_item ("," import_item)* "}"

import_item ::= identifier ("as" identifier)?

render_block ::= "render" ("struct" | "obj") identifier "{" view_body "}"

view_body ::= (view_component | html_element)*

view_component ::= "<" component_name attributes? ">" children? "</" component_name ">"
                 | "<" component_name attributes? "/>"

html_element ::= "<" tag_name attributes? ">" children? "</" tag_name ">"
               | "<" tag_name attributes? "/>"

attributes ::= attribute+

attribute ::= identifier "=" string_literal
            | "b-text:" identifier "=" expression
            | "b-show:" identifier "=" expression
            | "b-trigger:" identifier "=" identifier
            | "b-model:" identifier

children ::= (html_element | text_content)*

text_content ::= [^<]+
```

### 2.2 Parameters and Types

```bnf
parameters ::= "(" (param ("," param)*)? ")"

param ::= identifier ":" type

type_params ::= "<" identifier ("," identifier)* ">"

type ::= "Int" | "UInt" | "Float" | "String" | "Bool" | "Void" | "Data" | "Char"
       | identifier
       | "Vector" "<" type "," dimension ("," dimension)* ">"  // Multidimensional vector
       | "Option" "[" type "]"  // Optional type
       | "Result" "[" type "," type "]"  // Result type (for FFI)
       | "List" "[" type "]"  // Dynamic list
        | "Ptr" "[" type "]"  // Verified typed pointer
        | "Ptr"               // Bare: Ptr<Bits @/0..63> (safe void*)
        | "Ptr8" | "Ptr16" | "Ptr32" | "Ptr64" | "Ptr128" | "Ptr256"  // Fixed-width
        | "Sig" "[" identifier "]"  // Signature type
       | type "Union" "[" type ("," type)* "]"  // Union type
       | "(" type ("," type)* ")"  // Tuple type
       | "const" type  // Const-qualified type

dimension ::= identifier ":" integer  // Named dimension, e.g., width:50
            | integer                  // Anonymous dimension, e.g., 50

output_types ::= type ("," type)*  // Multi-output: (A, B, C)
```

**Flexible vs fixed bit width.** A type is **one machine word** — `int_bits`
wide, derived from the target's data-layout pointer width (`--int-bits`, one of
8/16/32/64) — unless a bit width is explicitly given as metadata. This applies
uniformly to integer, float, and String types:

- **Flexible-width types** (`Int`, `UInt`, `String`, and any type declared
  without `!> bits`, `!> maxbits:`, or `!> minbits:`): one machine word on every
  target. `Int` carries no bits metadata and has a *derived* width; `String` is
  a pointer to `[len][bytes]`, so it too follows the machine word (64 bits on
  x86-64, 32 on wasm32).
- **Fixed-width types** are absolute and never follow the machine word:
  `Int32` is always 32 bits, `Int64` always 64, `Float` always 32, `Float64`
  always 64.
- **Explicit metadata always wins**: `!> bits: N` fixes the exact width;
  `!> maxbits: N` sets a ceiling; `!> minbits: N` sets a floor. Absent all three,
  the type is flexible and derives to the machine word.

### 2.3 Statements

```bnf
statement ::= assignment
            | unification
            | guarded
            | when_guard
            | term
            | termbang
            | escape
            | expression_stmt
            | let_binding
            | inline_asm

assignment ::= "&"? lhs "=" expression ("," expression)* ";"

lhs ::= identifier | field_access | index_access

unification ::= identifier "(" pattern ")" "=" expression ";"

guarded ::= "[" condition "]" ("{" statement* "}" | statement)
          (* Same-line enforcement: if braces omitted, condition and
             statement must be on the same line *)

when_guard ::= "when" condition ("->" statement | "{" statement* "}")
             (* -> arrow form: condition and statement must be same line.
                Block form: no ->, braces required, no same-line restriction.
                when is forbidden on signatures and type definitions. *)

term ::= "term" (expression ("," expression)*)? ("->" statement)? ";"

termbang ::= "term!" (expression ("," expression)*)? ("->" statement)? ";"

escape ::= "escape" expression? ";"

expression_stmt ::= expression ";"

let_binding ::= "let" identifier (":" type)? ("=" expression)? ";"

inline_asm ::= "asm" string_literal ("{" string_literal ("," string_literal)* "}")? ";"
```

**Guard same-line rule:** Both `[condition]` and `when condition -> statement;`
require that the condition and its effect reside on the same physical line when
braces are omitted. If the effect spills to a new line, braces `{ }` are
required. This prevents dangling statements and maintains flat vertical
structure.

| Form | Same-line required? | Braces? | Arrow? |
|------|-------------------|---------|--------|
| `when x > 0 -> term 0;` | ✅ | ❌ | ✅ Required |
| `when x > 0 { term 0; };` | ❌ | ✅ | ❌ |
| `[x > 0] term 0;` | ✅ | ❌ | ❌ |
| `[x > 0] { term 0; };` | ❌ | ✅ | ❌ |

**AST equivalence:** Both `when condition` and `[condition]` forms parse to
the identical `Statement::Guarded { condition, statements, metadata }` node.
The SMT verifier, interpreter, and backends treat them identically.

**Guards have no else chain.** A `when` guard is an independent conditional
block. The body of a reactive transaction is therefore a *sequence of segments*
— contiguous runs of statements separated by `when` guards. This is the basis
of the compiler's **recursive version-DAG decomposition** (see
`docs/plans/2026-07-30-flat-node-decomposition.md` §11):

1. **Three-segment split.** The compiler splits the transaction body at each
   top-level `when` guard into `[pre]`, `[guard]`, `[post]` segments.
2. **Predicate analysis at the split point.** The guard condition is evaluated
   with the state at the exact point where the guard sits in the body. This
   captures whether the guard observes the counter pre- or post-increment
   *naturally* — no position scanning, no counter-name matching.
3. **Two-version reconstruction** (neutral framing — neither version is
   structurally "hot" or "cold"). Each guard produces a *guard-absent version*
   (`[pre] + [post]`, side effects removed) and a *guard-present version*
   (`[pre] + [guard] + [post]`, side effects present). Which version dominates
   at runtime is a predicate-frequency property, not structural.
4. **Static predicate simplification.** Classify each guard predicate before
   versioning: **provably always-true** → inline the guard body (or keep it
   apart if that is more efficient for LLVM); **provably always-false** → drop
   it (unless observable — keep the call for liveness); **runtime-dependent** →
   two versions.
5. **Recursion.** Nested `when` guards inside a guard body decompose into
   sub-versions, producing a **DAG of self-terminating while loops** that the
   backend emits as clean canonical loops for LLVM.
6. **Match normalization.** Statement-level `match` is normalized to a `when`
   sequence so the decomposition pass handles only `when`. The fallback arm
   becomes `when !(c1 ∨ ... ∨ cn)` — the negation of ALL other arm predicates.
   It is **never** `when true`, which would be indistinguishable from an
   unconditional block to the predicate analysis.

The write-conflict analysis (the XOR rule in §3.1 below) makes the
guard-present→absent dependency sequential: a guard-present version that reads
state written by the guard-absent version fires only after the guard-absent
version commits, preserving Brief's concurrent-firing semantics.

**Minimal-state / loop purity.** A variable is hot-loop state (a phi register)
iff it is loop-carried (written in iteration N, read in iteration N+k) or read
by a convergence contract / observable side effect at a different point than
its write. Loop-invariant fields are hoisted; boundary-only fields are
materialized to %State once at the loop boundary. The hot loop body must have
zero %State load/store so LLVM can prove no cross-iteration dependencies and
vectorize. See `docs/architecture/minimal-state-and-purity.md`.

### 2.4 Expressions

```bnf
expression ::= literal
             | identifier
             | binary_op
             | unary_op
             | call
             | field_access
             | index_access
             | slice
             | tuple
             | list
             | range
             | cast
             | prior_state
             | addr_of
             | projection
             | arrow_mut
             | arrow_discard
             | block

literal ::= Int | Float | Bool | String | Char | "true" | "false"

binary_op ::= expression operator expression

operator ::= "+" | "-" | "*" | "/" | "%"
           | "==" | "!=" | "<" | ">" | "<=" | ">="
           | "&&" | "||"
           | "&" | "|" | "^" | "<<" | ">>"

unary_op ::= "-" expression | "!" expression | "~" expression 
           | "&" expression          // Address-of
           | "*" expression           // Dereference

call ::= expression "(" (expression ("," expression)*)? ")"

field_access ::= expression "." identifier

index_access ::= expression "[" expression "]"

slice ::= expression "[" coordinate ("," coordinate)* (";" condition)? "]"

coordinate ::= expression                    // Single index: 5
             | expression? ".." expression?  // Range: 0..10, ..10, 5..
             | "::" expression               // Stride: ::2
             | identifier ":" coordinate     // Named dimension: time:5
             | "..."                         // Ellipsis: fill unspecified dimensions
             | "@" integer ":" coordinate    // Dimension specifier: @3:0..10

addr_of ::= "&" expression           // Address-of: generalized beyond identifiers

reflect_runtime      ::= expression ".^" reflect_target   // x.^Len, x.^Ptr
reflect_compile_time ::= expression ".^^" reflect_target  // x.^^Size, x.^^Bytes
reflect_target ::= "Len" | "Ptr" | "Size" | "Bytes" | "Alignment" | "Type"
                   // PascalCase compiler-known targets; any other name is an error.
                    | "AsStack" | "AsQueue"

arrow_mut ::= expression "<-" expression           // dest <- value: copy into lhs (or InsertAt on the collection)
            | expression "~<-" expression          // dest ~<- src: destructive extract (copy, then destroy rhs)
            | expression "[" expression "]" "<-" expression  // Indexed write
arrow_discard ::= "<-" expression                  // <- value: read discard
               | "~<-" expression                  // ~<- value: destructive discard
               | "<-" expression "[" expression "]"  // Indexed remove: <- list[i]

consumptive_op ::= "~=" | "~+" | "~-" | "~*" | "~/"   // op with the RHS consumed after
                                                   // (~ unary bitwise NOT is unchanged)

contract_invert ::= "[!/" expression "]"            // [!/X] → pre !X, post X
                  | "[!/!" expression "]"           // [!/!X] → pre X, post !X

tuple ::= "(" (expression ("," expression)*)? ")"

list ::= "[" (expression ("," expression)*)? "]"

range ::= expression ".." expression?

cast ::= expression "as" type

prior_state ::= "@" identifier

block ::= "{" statement* "}"

match_expr ::= "match" expression "{" match_arm+ "}"
match_arm ::= match_pattern ("if" expression)? "=>" (expression | block) ("," | ";")?
match_pattern ::= "_" | identifier ("(" identifier ("," identifier)* ")")?

\[Added 2026-05-29\]
```

### 2.5 Contracts

```bnf
contract ::= "[" expression? "]" "[" expression? "]" watchdog?

watchdog ::= ("?" | "!") "[" expression "]"
             ("within" integer unit)?
             ("->" identifier "(" identifier? ")")?
unit      ::= "ms" | "seconds" | "minute" | "cyc"
```

* **Precondition**: First bracket `[pre]` - what must hold before the
  function/transaction runs.
* **Postcondition**: Second bracket `[post]` - what the function guarantees
  will be true after execution.
* **Watchdog**: A **liveliness** contract. `?[...]` (optional) or `![...]`
  (required). The loop continues while the condition holds and **fires** the
  moment it stops holding. The `-> handler(val)` on-fire callback receives the
  last computed value. The `within N <unit>` clause adds a deadline — the fire
  happens even if the condition never stops holding, after `N` milliseconds/
  seconds/minutes (via the `Now#` monotonic clock) or `N` loop cycles (`cyc`).
* **Entry points**: The `[#]` entry marker is **removed** (2026-08-01, Phase
  2). CLI-addressable entry points are expressed with the `entry!` / `args!`
  macros instead (see the entry-point plugin). Writing `[#]` is a syntax
  error.

### 2.6 FFI Grammar

```bnf
foreign_sig ::= "frgn" identifier "(" parameters? ")" ("->" result_type)?
                ("as" identifier)? "from" source_spec
                ("fallback" fallback_expr)? ";"

source_spec ::= string_literal          (* literal path, e.g. "link/brief_rt.c" *)
              | "<" identifier ">"       (* compiler registry, e.g. <xxhash.c> *)

fallback_expr ::= ";"                   (* implicit — skip call, return zero-value *)
                | expression            (* static value, e.g. 0, "" *)
                | identifier "(" arguments? ")"  (* function call *)

result_type ::= "Result" "<" type "," type ">"
              | type
              | /* omitted = void */

(* Note: `from` is REQUIRED — every frgn must declare provenance. *)
(* `inop` and `#out` modifiers are removed from the language. *)
```


```bnf
foreign_sig ::= ("frgn" | "syscall") "sig" identifier "(" parameters? ")" "->" output_types ";"

frgn_binding ::= identifier "(" parameters? ")" "->" Result "[" type_params "]" "from" path") 

contract ::= "[" expression "]" "[" expression "]"
```

The compiler enforces that all FFI calls handle `Result` types. The `frgn` variant returns `Result<T, Error>` and must be handled; the `frgn!` variant returns `void` and is fire-and-forget.

---

## 3. Core Language Features

### 3.1 Transactions and Reactivity

Brief uses a reactor model where transactions declare when they can run and what they guarantee:

```brief
// Passive transaction (must be explicitly called)
txn increment(amount: Int) [amount > 0][counter == @counter + amount] {
    counter = counter + amount;
    term;
};

// Reactive transaction (fires automatically when precondition met)
node auto_save [dirty && !saving][!dirty] {
    saving = true;
    save_to_disk();
    dirty = false;
    saving = false;
    term;
};

// Async reactive transaction (can run concurrently with verified safety)
async node fetch_data [needs_update][data != @data] {
    let result = http_get(url);
    [result.is_ok()] {
        data = result.value;
    };
    term;
};
```

**Transaction modifiers:**
- `node` - Reactive: fires automatically when precondition becomes true
- `async` - Can run concurrently; compiler verifies mutual exclusion
- Both can be combined: `async node`

**Contract semantics:**
- `[pre]` - Precondition: when the transaction is allowed to fire
- `[post]` - Postcondition: what must be true after completion

### 3.2 Guard-Based Control Flow

Brief eliminates imperative branching (`if`/`else`) in favor of guards:

```brief
txn process(value: Int) [true][result != 0] {
    let result: Int = 0;
    
    // Guard: only executes if condition is true
    [value > 0] {
        result = value * 2;
    };
    
    [value < 0] {
        result = value * -1;
    };
    
    [value == 0] {
        escape;  // Rollback transaction
    };
    
    term;
};
```

**Guard behavior:**
- Multiple guards can execute (unlike `if`/`else if`)
- Guards are evaluated in order
- Empty guard body is valid: `[x > 0] &positive = true;`
- `escape` inside a guard rolls back the entire transaction

### 3.3 Definitions (Functions)

Functions (`defn`) are pure computations with contracts:

```brief
// Simple function
defn abs(n: Int) [true][result >= 0] -> Int {
    [n < 0] {
        term -n;
    };
    term n;
};

// Generic function
defn max<T>(a: T, b: T) [a >= b || b >= a][result == a.max(b)] -> T {
    [a >= b] {
        term a;
    };
    term b;
};

// Multi-output function
defn div_mod(a: Int, b: Int) [b != 0][quotient * b + remainder == a] -> (Int, Int) {
    term (a / b, a % b);
};

// Function with named outputs
defn get_coords() -> (x: Int, y: Int) {
    term (10, 20);
}
```

**Definition syntax:**
- `defn name(params) -> output_type [pre][post] { body }`
- Can return multiple values: `-> (Type1, Type2)`
- Named outputs: `-> (name: Type, ...)`
- Contracts are verified at compile time

### 3.4 Signatures (FFI)

Signatures declare external function bindings. The `frgn` keyword declares an external symbol; `frgn!` is fire-and-forget with no return.

```brief
// Standard FFI returning Result — caller must handle both Ok and Err
frgn sqrt(x: Float) -> Result<Float, MathError>;

// Fire-and-forget — no return type parsed, result discarded
frgn! log_message(msg: String);

// frgn without from searches import "link/..." targets
import "link/brief_rt.c";
frgn __print_int(n: Int) -> Result<Bool, Error>;

// sig #out — observable output, LLVM memory(write) prevents elimination
import { OUT__print_int } from "std/out.bv";
```

**FFI keywords:**
- `frgn` — Foreign function returning `Result<T, E>` — caller must handle both paths
- `frgn!` — Fire-and-forget — no return captured, errors cause runtime panic
- `sig #out` — Observable output modifier — prevents dead-code elimination
- `sig #inline` — Pure modifier — safe to fold/eliminate

**`from` clause:**
  - `from "c"` — C calling convention (symbol name is the Brief name)
  - `from "rust"` — Rust calling convention (name-mangled symbol)
  - `from "js"` — JavaScript (interpreter only, no LLVM backend)
  - `from "python"` — Python (interpreter only, no LLVM backend)
  - If omitted, the compiler searches `import "link/..."` targets for the symbol name

**Zero-cost inlining (LTO pipeline):** Languages that compile to LLVM IR (C, Rust, Zig, Swift, Julia, D) are linked via `llvm-link` and inlined across language boundaries by `opt -O2`. No FFI boundary overhead in the compiled binary. The pipeline is:

1. **`compile_to_bitcode()`** — compiles the foreign source file to LLVM bitcode using the appropriate compiler (`clang` for C, `rustc --emit=llvm-bc` for Rust, `zig build-obj -femit-llvm-bc` for Zig). The `LinkLanguage` enum determines which compiler to invoke:
   - `CLanguage` — invokes `clang -O2 -c -emit-llvm -o <bc> <file>`
   - `RustLanguage` — invokes `rustc --emit=llvm-bc -C opt-level=2 -o <bc> <file>`
   - `ZigLanguage` — invokes `zig build-obj -femit-llvm-bc -O ReleaseFast -o <bc> <file>`
2. **`link_and_optimize()`** — runs `llvm-link` to merge program bitcode with foreign bitcode, then `opt -O2 -S -vectorize-slp=false` to inline and optimize. Returns `Some(bc_path)` on success or `None` with a warning.

**`import "link/..."` search order:**
The `resolve_link_source()` function searches in this order, returning the first match:
1. Project-relative (same directory as source file)
2. `lib/runtime/` — built-in runtime modules (`brief_rt.c`)
3. `lib/std/c/` — vendored C libraries (`xxhash/`, `yyjson/`, etc.)
4. `BRIEF_STDLIB_PATH` environment variable
5. Absolute path resolution

The `import "link/..."` directive's `"..."` path is the file name (e.g., `"brief_rt.c"`, `"xxhash/xxhash.c"`). The resolver appends this to each search directory. This means `import "link/xxhash/xxhash.c"` resolves to `lib/std/c/xxhash/xxhash.c` via the `lib/std/c/` prefix.

```brief
import "link/brief_rt.c";       // resolves to lib/runtime/brief_rt.c
import "link/xxhash/xxhash.c";  // resolves to lib/std/c/xxhash/xxhash.c
```

### 3.5 State Management

State is declared globally and mutated with `&`:

```brief
// Simple state
let counter: Int = 0;
let name: String = "default";

// State without initial value (defaults to 0, "", false)
let balance: Int;
let active: Bool;

// Constant (immutable)
const MAX_SIZE: Int = 100;
const VERSION: String = "1.0.0";

// Mutable state in transaction
txn increment() [true][counter == @counter + 1] {
    counter = counter + 1;  // & required for mutation
    term;
};
```

**State rules:**
- `let` - Mutable state
- `const` - Immutable constant
- `&var = expr` - Mutation (required in transactions)
- `@var` - Prior state value in contracts

**Transaction modifiers:**
- `node` - Reactive: fires automatically when precondition becomes true
- `async` - Can run concurrently; compiler verifies mutual exclusion
- Both can be combined: `async node`

**Contract semantics:**
- `[pre]` - Precondition: when the transaction is allowed to fire
- `[post]` - Postcondition: what must be true after completion
- `@var` - Prior state: value of `var` at transaction start
- `term` - Completes transaction; verifies postcondition

### 3.6 Structs and Rstructs

Structs define composite types with fields and transactions:

```brief
// Basic struct
struct Point {
    x: Int;
    y: Int;
};

// Struct with methods (transactions)
struct Counter {
    value: Int = 0;
    
    txn increment(amount: Int) [amount > 0][value == @value + amount] {
        value = value + amount;
        term;
    };
    
    txn reset() [true][value == 0] {
        value = 0;
        term;
    };
};

// Usage
let p: Point = Point { x: 10, y: 20 };
let x_val = p.x;

let c: Counter = Counter {};
c.increment(5);
```

**Render Structs** attach UI views to state (`.rbv` files):

```brief
// counter.rbv — state declarations + transactions + render struct
let count: Int = 0;

txn increment [count < 100][@count + 1 == count] {
    count = count + 1;
    term;
};

render struct App {
    <div class="counter">
        <span b-text="count"></span>
        <button b-trigger:click="increment">+</button>
    </div>
};
```

The old `rstruct` keyword is deprecated — use `render struct`/`render obj`.

### 3.7 Enums

Enums define sum types with variants:

```brief
// Simple enum
enum Color {
    Red,
    Green,
    Blue
};

// Enum with data
enum Result<T, E> {
    Ok(T),
    Err(E)
};

// Usage with pattern matching
defn handle_result(r: Result<Int, String>) -> Int {
    unification r(Ok(value)) = value;
    unification r(Err(_)) = -1;
    term 0;  // Default
};

// Enum methods
enum Option<T> {
    Some(T),
    None;
    
    defn is_some(self) -> Bool {
        unification self(Some(_)) = true;
        term false;
    };
    
    defn unwrap(self) -> T {
        unification self(Some(value)) = value;
        term panic("unwrapped None");
    };
};
```

### 3.8 Imports and Modules

Imports bring external code into scope:

```brief
// Import entire module
import "std/math";
let x = math.sqrt(4.0);

// Import with alias
import "std/collections" as coll;
let list = coll.new_list();

// Import specific items
import {HashMap, HashSet} from "std/collections";
let map = HashMap::new();

// Import from file
import "./my_module.bv";

// Import with rename
import {foo as bar} from "./utils.bv";

// Import resource (CSS, SVG, etc.)
import "./styles.css";
import "./logo.svg" as Logo;
```

**Import resolution:**
- Relative paths: `./module.bv`, `../parent.bv`
- Standard library: `std.math`, `std.string`, etc.
- Resources: `.css`, `.svg`, `.png` (for Rendered Brief)

### 3.9 Resources

Resources declare external objects (files, kernel objects, etc.):

```brief
// File resource
rsrc config: File("config.toml", "read");

// Framebuffer (graphics)
rsrc fb: FrameBuffer(1920, 1080);

// Shared memory
rsrc shared: SharedMemory("my_app", 4096);

// Network socket
rsrc sock: Socket(AF_INET, SOCK_STREAM);

// Mutex
rsrc lock: Mutex();
```

**Built-in resource types:**
- `File(path, flags)` - File handle
- `FrameBuffer(width, height)` - GPU framebuffer
- `SharedMemory(name, size)` - IPC shared memory
- `Socket(domain, type)` - Network socket
- `EventFD()` - Event notification
- `Semaphore(initial)` - Counting semaphore
- `Mutex` - Mutual exclusion lock

### 3.10 Inline Assembly

Inline assembly for low-level operations:

```brief
// ARM assembly
txn wait_for_interrupt() [true][true] {
    asm "wfi";
    term;
};

// ARM with clobber list
txn set_register(value: Int) [true][true] {
    asm "mov x0, %0" { "x0" };
    term;
};

// With multiple clobbers
txn complex_op() [true][true] {
    asm "add x0, x1, x2; mul x3, x0, x4" { "x0", "x3" };
    term;
};
```

**ASM syntax:**
- `asm "instructions";` - Simple form
- `asm "instructions" { "clobber1", "clobber2" };` - With clobber list
- Clobbers tell compiler which registers are modified

### 3.11 Bit-Packed Structures

Struct fields can have bit widths for compact storage:

```brief
// Packed struct (fits in 16 bits)
struct Pixel {
    r: 4bits,
    g: 4bits,
    b: 4bits,
    a: 4bits
};

// Bit ranges
struct Control {
    enable: 0..1,
    mode: 1..3,
    flags: 3..8
};

// Usage
let p: Pixel = Pixel { r: 15, g: 0, b: 0, a: 255 };
let packed: Int = p;  // Automatically packed
```

**Bit packing:**
- `nbits` - Exactly n bits
- `start..end` - Bit range (end-exclusive)
- Compiler auto-packs fields into minimal storage

### 3.12 Vector and Slice Types

Fixed-size arrays (`Vector<T, N>`) and runtime slice views (`Slice<T>`) for
contiguous and strided element access across all Brief source variants.

#### 3.12a Vector Declaration (`Type[N]`)

`Type[N]` declares a fixed-size array of N elements. The compiler embeds it
as `[N x T]` in LLVM IR, enabling SROA decomposition and auto-vectorization.

```brief
let data: Float[64];   // 64 floats
let ints: Int[1024];   // 1024 integers
let frames: Frame[256]; // 256 frames (struct type)
```

`Int[1024]` → `[1024 x i64]` in LLVM. `Frame[256]` → `[256 x %Frame]`.

Embedded variants (`.ebv`) additionally support memory-mapped vectors:

```brief
let sensor: Float[8] @ 0x40000000;  // 8 floats at address 0x40000000
```

#### 3.12b Element Access (`v[i]`)

Index access with contract-proven bounds:

```brief
v[i]        // Single element access
v[i] = val; // Single element assignment
```

Contracts prove bounds at compile time:

```brief
[i >= 0 && i < arr :#Size] {  // Guarded access
    let x = arr[i];
};
```

#### 3.12c Slice View (`arr[start:end:stride]`)

A slice is a zero-copy view into an existing array. It produces a `Vector<T, M>`
when all bounds are compile-time constants, or a `Slice<T>` runtime descriptor
when any bound is a variable.

```brief
arr[:]         // Full view — same as arr but typed as slice
arr[4:]        // Index 4 to end
arr[:8]        // Start to index 8
arr[2:8]       // Range [2, 8), stride 1
arr[2:8:2]     // Strided range — every other element
arr[i:j]       // Dynamic bounds — runtime start and end
arr[i:j:k]     // Dynamic stride — runtime step
```

All components are optional:
- `start = None` → `0`
- `end = None` → `arr .^Len` (array length)
- `stride = None` → `1`

**Type rules:**

| Bounds | Result type | Codegen |
|--------|-------------|---------|
| All constants: `arr[0:16:2]` | `Vector<T, 8>` | Direct GEP, LLVM auto-vectorizes |
| Any variable: `arr[i:j:k]` | `Slice<T>` (runtime descriptor) | Runtime loop, contract-proven bounds |
| Mixed: `arr[i:j:2]` | `Slice<T>` (stride folded to constant) | Partially unrolled loop |

A `Slice<T>` at runtime is a descriptor `{ base_ptr: Ptr<T>, start: Int,
length: Int, stride: Int }`, or inlined as direct GEP when used immediately.

#### 3.12d SIMD Operators on Vector/Slice

Element-wise arithmetic operators on `Vector<T, N>` and `Slice<T>` types:

```brief
let a: Int[4] = ...;
let b: Int[4] = ...;
let sum = a + b;       // <4 x i64> vector add
let prod = a * b;      // <4 x i64> vector mul
let doubled = a * 2;   // Scalar broadcast + vector mul
```

**Supported operators:** `+`, `-`, `*`, `/` (element-wise on matching types).

**Scalar broadcast:** `arr * 2` broadcasts the scalar across all elements
via `insertelement` + `shufflevector`.

**Mixed Vector/Slice:** `Vector<T, N>` + `Slice<T>` where both lengths are
proven equal by contract → same vectorized codegen.

#### 3.12e Slice as Lvalue

Slices can appear on the left of assignment:

```brief
arr[2:8] = src[4:10];    // Contiguous memcpy (stride 1:1)
arr[0:N:2] = src[0:N];   // Strided dest, contiguous src — gather/scatter loop
```

- Contiguous same-stride (1:1): emits `@llvm.memcpy`
- Different strides: per-element load/store loop, LLVM loop vectorizer handles it
- Contract system proves source and destination don't overlap (or semantics of
  overlapping slices are defined)

#### 3.12f View Casts (Type-Punned and Strided)

The `as` operator produces zero-copy views between compatible array types:

**Type-punned view** — reinterpret the same bytes with a different element type:

```brief
let raw: Int[1024];
let bytes = raw as Byte[8192];   // 1024 * 8 = 8192 bytes
let frames = raw as Frame[256];  // if sizeof(Frame) == 32
let bad = raw as Byte[1000];     // ❌ byte size mismatch
```

Compile-time validation: `N * sizeof(T) == M * sizeof(U)`. Emits `bitcast`.

**Strided view** — recast a slice onto a sized array:

```brief
let evens = raw[0:1024:2] as Int[512];  // stride 2 → 512 elements
let subset = raw[2:10] as Int[8];        // contiguous, 8 elements
```

The type checker computes: `ceil((end - start) / stride) == N`.

Both are zero-copy — no allocation, no memcpy. The compiler emits `bitcast`
for matching byte-size vectors.

#### 3.12g Stdlib Iterators (Not Magic)

`map`, `filter`, `fold`, `any`, `all`, `sum`, `product` are regular txn functions
in `lib/std/array.bv`, not compiler intrinsics:

```brief
txn array_map<T, U>(arr: Vector<T, N>, f: T -> U, i: Int)
    -> Vector<U, N>
    [i < N][i == N]
{
    result[i] = f(arr[i]);
    &i = i + 1;
    term result;
};
```

The LLVM auto-vectorizer recognizes the `[i < N]` convergence contract and
vectorizes the load-apply-store loop automatically. No compiler magic needed.

---

\[Updated 2026-07-26: Generalized from Embedded-only to core. Added slice
syntax `arr[start:end:stride]`, SIMD operators, slice as lvalue, view casts,
stdlib iterator pattern.\]

### 3.13 Match Expression

The `match` expression performs multi-arm pattern matching on enum values. It replaces verbose chains of `uni` statements with a single exhaustive expression.

**Syntax:**
```
match value {
    Variant(field1, field2) => body,
    _ => default,
}
```

**Semantics:**
1. The scrutinee expression is evaluated first.
2. Arms are tried in declaration order. The first arm whose pattern matches the scrutinee's enum variant is selected.
3. If the arm has a guard (`if condition`), the guard must also evaluate to `true`.
4. The match is **exhaustive** — a wildcard arm `_ =>` is required if not all variants are covered. Without it, the compiler raises a compile-time error.
5. The match is an **expression** — it can appear on the right side of `let`, inside a `term`, etc. The result is the body of the matched arm.
6. Pattern variables are bound to the corresponding fields of the matched variant and are scoped to the arm's body.

**Examples:**

```brief
// Basic enum matching
enum Option<T> { Some(T), None };
let val: Option<Int> = Some(42);
let result: Int = match val {
    Some(x) => x,
    None => -1,
};

// With wildcard fallthrough
match result {
    Ok(value) => println("Success: " ++ value.to_string()),
    _ => println("Failed"),
};

// Match as a term value
term match status {
    Active => "running",
    Inactive => "stopped",
    _ => "unknown",
};
```

**Relation to `uni`:** The `match` expression is a higher-level construct that desugars to a sequence of `uni` statements with a `term` fallthrough. Single-arm pattern matching should still use `uni` for simplicity.

**Compiler support:** Supported in Rust parser + interpreter; self-hosted (Brief-in-Brief) support pending (requires `KeywordMatch` token in `token.bv` and `parse_match_expr` in `parser.bv`).

### 3.14 Collection Mutation

Collection mutation is expressed through the `<-` arrow syntax, with the `&`
sigil marking the target collection. Three operations are supported:

**Push (append):**
```brief
&list <- item;        // Append item to list
```

**Pop (remove and return):**
```brief
let item = <- &list;  // Pop item from list (removes last element)
```

**Indexed write:**
```brief
&list[i] <- value;    // Write value at index i
```

**Indexed remove:**
```brief
<- &list[i];          // Remove element at index i
```

**Prepend (insert at front):**
```brief
item <- &list;        // Insert item at front of list
```

**Semantics:**
1. `target <- value` — the arrow always points toward the collection. The
   collection must be prefixed with `&`.
2. Push/Pop are `O(1)` amortized operations on the underlying 2-slot header
   `[pointer, length]`.
3. The `<-` operator is the ONLY mutation mechanism. There are no method
   calls, no `.append()`, no `.pop()` — collection mutation is a first-class
   syntactic operation.
4. The `<-` syntax also serves as a halting signal for the dead-field
   elimination pass: a collection targeted by `<-` is never dead, because
   the mutation is an observable side effect on state.

**Discard form:**
```brief
<- &list;             // Pop and discard last element
<- &list[i];          // Remove and discard element at i
```

The discard form (no receiver for the popped value) is equivalent to a pop
followed immediately by dropping the value. It is useful when only the
length effect is needed (e.g., drain-until-empty halting patterns).

**Expression discard form:**
```brief
<- syscall! @ 3 (fd);          // Call syscall, discard result
<- compute_side_effect();       // Call function, discard return
```

The expression discard `<- expr` evaluates any expression and discards its result. This is required for syscall results that are intentionally ignored, ensuring no system-level side-effect can ever be silently ignored.

**Arrow direction rules:**
| Form | Meaning |
|------|---------|
| `&list <- value` | Push: add value to list (append) |

---

### 3.15 Reflection (`.^` runtime, `.^^` compile-time)

Brief has two reflection operators, both compile-time-resolved and explicitly
marked (a lower-case name after the operator is a parse error):

| Operator | Kind | Result |
|----------|------|--------|
| `expr.^Meta` | **Runtime** reflection — value-derived | a runtime value (`Len`, `Ptr`) |
| `expr.^^Meta` | **Compile-time** reflection — type-derived | a foldable constant (`Size`, `Bytes`, `Alignment`, `Type`) |

**Target table (all PascalCase compiler-known):**

| Target | Kind | Result | Notes |
|--------|:----:|--------|-------|
| `Len` | `.^` | `Int` | runtime length of a String/List value |
| `Ptr` | `.^` | `Ptr<T>` | address-of; `&x` is the primary spelling |
| `Size` | `.^^` | `Int` | fixed-size element count (`Int[8].^^Size` → 8) |
| `Bytes` | `.^^` | `Int` | storage size of the type — compile-time constant |
| `Alignment` | `.^^` | `Int` | alignment of the type — compile-time constant |
| `Type` | `.^^` | type token | type identity, usable in cast position |

**Static/runtime boundary.** `.^` is strictly runtime, `.^^` is strictly
compile-time: a compile-time-only target after `.^` (and vice versa) is an
error. Compile-time results fold — usable in `const` initializers and contract
expressions. Runtime introspection beyond these targets uses method calls
(`s.trim()`, `list.len()`) — never a reflection operator.

**Examples:**

```brief
let n: Int   = s.^Len;         // runtime length
let p: Ptr<T> = &x;            // & is primary; x.^Ptr is the reflection form
let sz: Int  = x.^^Bytes;      // compile-time constant
let al: Int  = x.^^Alignment;  // compile-time constant
let size: Int = arr.^^Size;    // fixed-size count (Int[8].^^Size → 8)
```

> **Note on the `^` glyph:** `^` alone remains bitwise XOR (`a ^ b`). The dot
> disambiguates: `expr.^Meta` / `expr.^^Meta` are reflection, `a ^ b` is XOR.

> **Bit intrinsics** (`ctpop`, `ctlz`, `cttz`, `abs`, `bitreverse`) are LLVM
> declarations in the backend; their operator-form projections were removed
> with the `:>` system. They are reachable as stdlib/`#` intrinsics.

### 3.16 Ptr\<T\> Type and Safe Pointer Operations

The `Ptr<T>` type represents a verified pointer to a value of type `T`.
Creation is via **address-of** (`&x`), and dereference via `*p` / `p[i]`. The
compiler tracks provenance (`Provenance::Deref`) so it can verify bounds.

**Creating a pointer:**

| Expression | Result type | Provenance |
|------------|-------------|------------|
| `&x` | `Ptr<Int>` | Bound = sizeof(x), non-null guaranteed |
| `&list[0]` | `Ptr<T>` | Bound = list byte length, non-null guaranteed |
| `x.^Ptr` | `Ptr<T>` | Reflection form — the address-of spelling `&x` is primary |

**Dereferencing:**

When a `Ptr<T>` is indexed with `ptr[i]`, the compiler emits a direct
`getelementptr + load` (or `store`) instruction — identical to raw C pointer
access — but only after verifying the access is within bounds.

```brief
let p: Ptr<Int> = &x;
let val = p[0];                   // Read — compiler verifies 0 < sizeof(x)
p[0] = 42;                        // Write — compiler verifies bounds
```

**Safety verification:**

The `PointerVerifier` pass checks every `ptr[i]` access at compile time:
1. `i >= 0` — must be proven or specified as a precondition
2. `(i + 1) * sizeof(T) <= ptr.^^Bytes` — must be proven
3. Unprovable → `ProofError(P200)` "out of bounds access"

**Standard library:** `std/ptr.bv` provides `read_i64`, `write_i64`,
`address`, `read_byte`, and `copy` with contract-proven safety. See §6.9.

#### 3.16.1 Layout-Constrained Pointers (2026-07-03)

`Ptr<T>` with a `Bits @/` inner type is a **layout-constrained pointer** that
carries spatial information (bytes + alignment) without nominal type semantics.
This is the safe `void*` equivalent.

| Sugar | Desugars to | Byte width |
|-------|-------------|------------|
| `Ptr` | `Ptr<Bits @/0..63>` | 8 |
| `Ptr8` | `Ptr<Bits @/0..7>` | 1 |
| `Ptr16` | `Ptr<Bits @/0..15>` | 2 |
| `Ptr32` | `Ptr<Bits @/0..31>` | 4 |
| `Ptr64` | `Ptr<Bits @/0..63>` | 8 |
| `Ptr128` | `Ptr<Bits @/0..127>` | 16 |
| `Ptr256` | `Ptr<Bits @/0..255>` | 32 |

Operations on layout-constrained pointers are spatial-only:
memcpy, memcmp, memset, hash, address arithmetic, volatile load/store.
Semantic operations (arithmetic on pointee, field access) are rejected.

```brief
let p: Ptr64 = 0x4000 as Ptr64;
let raw: Ptr = p as Ptr;          // cast between layout-constrained ptrs
let i: Ptr<Int32> = p as Ptr<Int32>;  // if bytes match (4 == 4)
```

#### 3.16.2 Layout-Compatible `as` Casts (2026-07-03)

`Ptr<A> as Ptr<B>` is valid when `bytes(A) == bytes(B)` and `alignment(A) >=
alignment(B)`. No explicit `meld` required for simple layout compatibility.

```brief
// Float (4 bytes, align 4) and Int32 (4 bytes, align 4)
let f: Ptr<Float> = 0x40010000 as Ptr<Float>;
let i: Ptr<Int32> = f as Ptr<Int32>;         // ✅ same layout

// Int (8 bytes) vs Int32 (4 bytes) — different sizes
let i64: Ptr<Int> = 0x4000 as Ptr<Int>;
let i32: Ptr<Int32> = i64 as Ptr<Int32>;    // ❌ compile error
```

Layout-compatible casts are zero-cost — the backend merely changes the
`TypedRegister.ty` metadata; no LLVM instructions are emitted.

#### 3.16.3 Spatial Intrinsics (2026-07-03)

Raw memory operations on layout-constrained pointers:

| Intrinsic | Syntax | Description |
|-----------|--------|-------------|
| `__memcpy#` | `__memcpy#(dst, src, n) -> Bool` | Copy n non-overlapping bytes |
| `__memcmp#` | `__memcmp#(a, b, n) -> Int` | Compare n bytes (0 = equal) |
| `__memset#` | `__memset#(ptr, val, n) -> Bool` | Fill n bytes with val |
| `__hash#` | `__hash#(ptr, n) -> Int` | FNV-1a hash of n bytes |

These emit direct `@llvm.memcpy` / `@memcmp` / `@llvm.memset` calls with
no intermediate wrappers. Available via `lib/std/spatial.bv`:
`block_copy`, `block_compare`, `block_fill`, `block_hash`.

#### 3.16.4 Function Pointers via `.^Ptr` (2026-07-03)

A function reference can be converted to a function pointer:

```brief
defn my_cmp(a: Int, b: Int) -> Bool { term a == b; };
let cmp_fn = &my_cmp;   // fn pointer type: Fn(Int, Int) -> Bool
let eq = cmp_fn(3, 5);        // indirect call through fn pointer
```

The type `Fn(Params...) -> Return` is represented as
`Applied("Fn", vec![Type::Tuple(params), return_type])` in the AST.
The LLVM backend marshals arguments per the internal calling convention
(passes `%state`, handles Bool/Float/String types) and emits indirect
`call %fn_ptr()`. Float returns are bitcast through i32 → zext to i64.

#### 3.16.5 Extract-Operate-Repack (EOR) Optimization (2026-07-03)

When `meld T -> Int`, the pattern `(x as Int) op (y as Int) as T` is
detected and compiled as a single native operation without redundant casts:

```brief
meld Meters -> Int;
defn scale(val: Meters, factor: Int) -> Meters {
    term (val as Int) * factor as Meters;  // single mul i64
};
```

The backend detects `Cast(BinaryOp(Cast(a, T), Cast(b, T)), U)` where
`U -> T` and emits the inner operation directly. Both integer and float
arithmetic are supported.

---

### 3.17 Subtyping and Meld

The historical `<:` (derivation) and `:>` (projection) operators were removed
with the hashword-protocol architecture (2026-07-20). Type relationships are
now expressed with protocol hashwords (`type Int: #Int`) and reflection
(§3.15). Cross-layout reuse is expressed with `meld`:

```brief
meld CBuffer -> RSBuffer {
    layout { bytes; alignment; };
};
```

The collection-query pipeline (`FILTER`/`GROUP`/`COUNT` fusion) and the
compile-time regex→DFA capture (§3.17 historical) are **planned** features —
not yet implemented. When they land, they will use method-call syntax
(`list.filter(...)`, `str.match(...)`), not operator projections.

---

### 3.18 Throughput-Matched Optimization (Roofline Model)

Brief's compiler uses physical hardware constraints to guide precompute/fold
decisions. A precomputed LUT that spills out of cache runs *slower* than the
arithmetic loop — the roofline model prevents this.

**Configuration:** Loaded from a `bottlenecks.dbvs` schema file referenced by
the target spec:

```dbvs
// bottlenecks.dbvs
schema BottleneckConfig {
    pcie_bandwidth_gbs: Float = 15.75;
    system_ram_bandwidth_gbs: Float = 40.0;
    l1_cache_size_kb: Int = 32;
    l2_cache_size_kb: Int = 256;
    l3_cache_size_kb: Int = 8192;
    memory_port_width: Int = 1;
    fpga_clock_mhz: Float = 0.0;
};
```

**Roofline decisions:**

| LUT size | Fits cache | Decision |
|----------|------------|----------|
| ≤ L1 size | L1 | Precompute unconditionally (zero-cost LUT) |
| ≤ L2 size | L2 | Precompute (fast lookup) |
| ≤ L3 size | L3 | Precompute only if 10× reuse factor |
| > L3 size | None | Emit runtime loop (spills to RAM) |

The `RooflineAnalyzer` (§6.11) also evaluates arithmetic intensity
(FLOP/byte) against peak compute and memory bandwidth to determine whether
an optimization is compute-bound or memory-bound.

### 3.19 Universal Bracket Syntax (SIMD Protocol) \[2026-06-11\]

Bracket syntax (`[]`) works universally on **every type**, not just collections
and strings. Every value decomposes into its visual fragments. Bracket
operations select, filter, stride, or transform these fragments. The result
reconstructs to the original type.

This makes SIMD vectorization a natural consequence of bracket operations:
uniform element-wise transforms (`&x[;pred] = val`) are trivially vectorizable
across any contiguous sequence type.

#### Fragment Decomposition

| Type | Fragment type | Source | Assignable? |
|------|---------------|--------|-------------|
| `String` | `Char` | Characters | Yes |
| `List<T>` | `T` | Elements | Yes |
| `HashMap<K,V>` | `(K,V)` | Entries | Yes |
| `HashSet<T>` | `T` | Elements | Yes |
| `Stack<T>` / `Queue<T>` | `T` | Elements | Yes |
| `Tuple` | element types | Elements | No |
| `Struct` | `(String, Value)` | Fields | No |
| `Int` | `Char` | Visual digits | No |
| `Float` | `Char` | Visual repr | No |
| `Bool` | `Char` | Visual repr | No |
| `Char` | `Char` | Itself | No |

For atomic types (Int, Float, Bool, Char), bracket decomposition always yields
`Char` fragments. `15561` → `[Char('1'), '5', '5', '6', '1']`.

#### Bracket Dispatch Rules

| Form | Collection types | Atomic types |
|------|-----------------|--------------|
| `val[coord]` | Element index | Char index |
| `val[::N]` | Every Nth element | Every Nth Char |
| `val[;pred]` | Element filter | Char filter |
| `val[string_or_const]` | Coord (existing) | **Desugars to regex filter** |

**Regex desugar rule**: If brackets contain exactly one argument, that argument
is a string literal or `const` string variable, and the value type is **not** a
collection → desugar to `val[;@"string_expr"]` (regex filter on stringified
value).

**Collection exception**: `map["key"]` on `HashMap<String,V>` remains a key
lookup. Collection bracket is never implicitly regex.

#### Regex Literal: `@"pattern"`

A new expression form producing a regex value:

```brief
// Regex literal
let vowel: Regex = @"[aeiou]";

// In bracket filter (explicit)
<- &str[;@"\s+"];              // Remove whitespace

// In bracket filter (implicit via desugar)
let clean = str["[a-z]+"];     // Desugars to str[;@"[a-z]+"]

// As a constraint
let email: String : [@"\A[^@]+@[^@]+\z"];
```

`@"..."` literals known at compile time are compiled to a DFA via the existing
`analysis::dfa` module at parse time (Thompson NFA → powerset construction).
Runtime regex becomes O(n) table walk with zero allocation.

The `@` token is already lexed. Parser disambiguation:
- `@` + identifier → `Expr::PriorState` (existing)
- `@` + string literal → `Expr::RegexLiteral` (new)

#### Arrow + Bracket (SIMD Assignment)

| Construct | Semantics | Example |
|-----------|-----------|---------|
| `&x[;pred] = val` | Replace all matching fragments | `&n[;=='5'] = '7'` on `15561` → `17761` |
| `<- &x[;pred]` | Remove all matching fragments | `<- &n[;=='5']` on `15561` → `161` |
| `&x[;@"re"] = val` | Regex-level replace on stringified | `&s[;@"\d+"] = "N"` |
| `<- &x[;@"re"]` | Remove regex matches | `<- &s[;@"\s+"]` |

The compiler recognizes uniform filter+assign as SIMD candidates when:
- Target is a contiguous sequence (String, List\<Int\>, digit chars of Int)
- Predicate is element-wise (scalar comparison, not multi-fragment regex)
- Assignment value is uniform for all matching elements

#### Result Reconstruction

After filtering or transformation, the remaining fragments are reconstructed
to the original type:

| Original type | Reconstruction |
|---------------|---------------|
| `Int` | Parse remaining `Char` sequence as integer via div/mod |
| `Float` | Parse remaining `Char` sequence as float |
| `String` | Join remaining `Char` values |
| `List<T>` | Already elements — no reconstruction needed |
| `Bool` | `"true"` or `"false"` parse back |

Reconstruction is defined per type and may fail at compile time if the
resulting fragment sequence is not valid for the target type (e.g., removing
the `'.'` from a float leaves a valid integer parse, so it succeeds; removing
a digit from `"true"` may produce a non-Bool string).

#### Relationship to Existing Syntax

- `str["pattern"]` (existing `SubtypeOp::Match`) is preserved but desugars to
  `str[;@"pattern"]` — unified through bracket dispatch.
- `map["key"]` (existing HashMap access) remains Coord — never desugars.
- Collection mutation via `<- &list[;pred]` extends existing ArrowDiscard
  semantics from "pop one" to "remove all matches" when a filter is present.
- `BracketOp::Mask` already evaluates arbitrary expressions — adding
  `Value::Regex` handling is a new match arm, no structural change.

---

### 3.20 Codec Declarations \[2026-07-11: Phase 4\]

A codec declaration defines a named codec that controls how values of a type are serialized, validated, and (in Phase 5) parsed from literal text.

**Syntax:**

```brief
codec HexColor {
    [value >= 0];               // validation constraint
    [value <= 0xFFFFFF];        // validation constraint
    !> parse: parse_hex_color;   // Phase 5: custom literal parser
    !> format: format_hex_color;  // Phase 5: custom formatter
};
```

**Grammar:**

```bnf
codec_decl ::= "codec" ident "{" codec_body "}" ";"
codec_body ::= (constraint | binding)*
constraint ::= "[" expr "]"
binding    ::= "!>" ("parse" | "format") ":" ident ";"
```

**Semantics:**

1. Constraints are expression guards that values of types referencing this codec must satisfy. They are merged into the type's guards during type resolution.
2. `!> parse: fn_name;` registers a function that converts a literal string to a value of the codec's associated type. Used by the custom literal parser system (Phase 5).
3. `!> format: fn_name;` registers a function that converts a value to its string representation.
4. A type references a codec via the `!> codec:` property binding in its body:

```brief
type MyInt : Int {
    !> codec: PositiveInt;
};
```

**Implementation:** `CodecDeclaration` in `src/ast.rs`, parsed in `src/parser.rs`, collected into `TypeUniverse.codecs` during `build()` Phase 1, constraints merged via Phase 4 linking in `resolve_type_def()`.

### 3.21 Custom Literal Parsers \[2026-07-11: Phase 5\]

When a variable is declared with a type that has a codec containing a `!> parse:` handler, the compiler detects bare identifiers in the initializer position and rewrites them as deferred literals:

```brief
codec HexColor {
    [value >= 0];
    [value <= 0xFFFFFF];
    !> parse: parse_hex_color;
};

type Color : Int {
    !> codec: HexColor;
};

let c: Color = FF00FF;   // FF00FF is a DeferredLiteral
```

**Detection pipeline:**

1. Parser: `FF00FF` is parsed as `Expr::Identifier("FF00FF")`.
2. NormalizeTypes pass: detects that the bound type `Color` has a codec with a parse handler. Rewrites to `Expr::DeferredLiteral { text: "FF00FF", expected_type: Custom("Color") }`.
3. Type checker: `DeferredLiteral` typechecks as its `expected_type`.
4. Codegen: emits a zero-initialized value with a warning. Full parse-handler invocation via the interpreter is planned.

**AST representation:** `Expr::DeferredLiteral { text: String, expected_type: Box<Type> }` in `src/ast.rs`.

**Current limitations:**
- The parse handler is not yet invoked via the interpreter. The literal produces a zero placeholder.
- The format handler is declared but not yet consumed.
- Only let-binding initializers are detected; other expression positions are deferred.

### 3.22 Plugin System \[2026-07-11: Phase 7\]

Compiler plugins are WASM (or native `.so`) modules loaded at compile time that can observe and optionally abort the compilation pipeline at defined hook points.

**CLI:**

```bash
brief build file.bv --plugin ./my_plugin.wasm        # WASM plugin
brief build file.bv --plugin ./my_plugin.so           # Native plugin
brief build file.bv --plugin ./p1.wasm --plugin ./p2.wasm  # Multiple plugins
```

**Hook points:**

| Hook | Pipeline Position | Purpose |
|------|-------------------|---------|
| `AfterParse` | After import resolution | Validate raw parse tree |
| `AfterTypeCheck` | After type checking | Verify type-level invariants |
| `BeforeCodegen` | Before code generation | Last transformation opportunity |
| `AfterCodegen` | After LLVM IR generation | Post-process the IR |

**Plugin interface:** See `src/plugin/mod.rs` for the `Plugin` trait and `PluginManager`.

**Architecture:**

```rust
pub trait Plugin: Debug {
    fn name(&self) -> &str;
    fn on_hook(&self, hook: PluginHook, program: &mut Program,
               universe: &TypeUniverse) -> PluginAction;
}
```

**Native plugin ABI:** A `.so`/`.dylib`/`.dll` must export a `brief_plugin_create` function returning a `*mut dyn Plugin`. Loading uses `libloading`.

**WASM plugin loading:** Requires the `plugins` Cargo feature (wasmtime runtime). The WIT interface is defined in `wit/` and compiled to WASM via Brief's own Phase 6 WASM target.

**Why WASM for plugins?** See `docs/architecture/features/plugins.md`.

---

### 3.23 Derivation Blocks (`:=`) \[2026-07-11: Phase 8\]

A derivation block attaches input-output examples to a definition or
transaction. When a body is present, the examples act as compile-time
assertions. When the body is omitted (drafting state), the compiler
synthesizes the minimal formula that satisfies all examples.

```brief
// Resolved state: body + derivation (compile-time assertions)
defn add(a: Int, b: Int) -> Int {
    term a + b;
} := {
    2, 2 -> 4;
    3, 5 -> 8;
};

// Drafting state: derivation only (compiler synthesizes body)
defn swap(x: UInt16) -> UInt16 := {
    0x1234 -> 0x3412;
    0x00FF -> 0xFF00;
};

// Derivation with contracts (hybrid: examples + formal constraints)
defn clamp(val: Int) -> Int
    [result >= 0]
    [result <= 100]
:= {
    -5 -> 0;
    200 -> 100;
};
```

**Behavior:**
- When body is present: compiler interprets the body with each example's
  inputs and asserts the output matches. Compile error on mismatch.
- When body is absent: `brief derive` runs SMT synthesis to infer the body.
- The derivation block is never consumed — it stays in source as the
  permanent specification.
- `#no_derive` pragma blocks synthesis during drafting.

### 3.24 Entry Points \[2026-08-01: Phase 3\]

The `[#]` entry-point contract marker is **removed**. Writing `[#]` is a
syntax error. CLI-addressable entry points and `argc`/`argv` parsing are
expressed with the `entry!` and `args!` macros (the entry-point plugin),
which expand to explicit preconditions and guard injection.

**`entry!("<cmd>")`** in a node's contract makes it a one-shot CLI subcommand:

```brief
// `myapp build` fires this node exactly once; `myapp run` fires the other.
node build [entry!("build")][result == 0] { ... };
node run   [entry!("run")][result == 0]   { ... };
```

Expansion: the plugin injects `let __entry_build_done: Bool = false;`,
rewrites the guard to `entry_cmd() == "build" && !__entry_build_done`, and
appends `__entry_build_done = true;` to the body (one-shot). `[true]` is never
emitted. A non-reactive `defn` entry point gets a synthesized reactive wrapper
(the helper-node path) that calls it once.

**`args!("--flag")`** / **`args!("--flag", T)`** bind a snapshot state field
from `__argv_has` / `__argv_value`:

```brief
let clean: Bool = args!("--clean");   // __argv_has("--clean")
let out: String = args!("--out", String); // __argv_value("--out")
```

**Command semantics:** `entry_cmd()` returns the first non-flag `argv[1..]`
token (`<prog> --verbose build` → `"build"`), honoring `$BRIEF_ENTRY_CMD` as
the sole environment fallback.

**Rules:**
- Two entry nodes with mutually exclusive commands are UNSAT
  (`cmd == "build"` ∧ `cmd == "run"`) → legal subcommand dispatch.
- Generated entry/script nodes are never `async` and never `sync<group>`;
  the concurrency gate (rule #21) demands explicit classification for any
  eligible pair.
- Helper names (`__entry_<cmd>_done`, `arg_<flag>`) are compiler-reserved;
  a collision is a compile error.

### 3.25 `export` Keyword \[2026-07-12: Phase 15\]

The `export` keyword marks a definition for C/foreign linking. The
compiler generates a C-ABI compatible wrapper with state handle.

```brief
export defn add(a: Int, b: Int) -> Int { term a + b; };
```

Compiled with `brief build --library`, this produces:
- `.ll` LLVM IR with a `dso_local` wrapper function
- `.o` object file
- `.a` static library
- `brief_types.h` C header with `__brief_init_state` and `__glue_release`

**Replaces:** The old `#export` pragma (now removed — use `export defn` instead).
deprecation window.

### 3.26 `alloc` Metadata \[2026-07-12: Planned\]

The `alloc` annotation on variable bindings controls where and how memory
is allocated. It follows the `!>` metadata pattern.

```brief
// Stack allocation (verified no-escape at compile time)
let buffer: List<Int>;
!> buffer: alloc("Stack");

// Physical memory-mapped I/O (MMIO)
let uart_status: UInt32 !> buffer: alloc(0x4000_2000);;

// Arena allocation (opaque — backend handles it)
let node: TreeNode;
!> node: alloc("Arena", scratchpad);

// Placement new (bind to existing pointer)
let header: PacketHeader;
!> header: alloc(raw_ptr);
```

**Frontend validation:**
- `alloc("Stack")` — verifies the variable does not escape its scope.
  Expands to `alloca: true` metadata.
- `alloc(0x...)` — verifies the address is a compile-time constant.
  Implicitly expands to `volatile: true`, `observable: true`,
  `fixed_addr: <addr>`.
- `alloc("Arena", ptr)` and `alloc(ptr)` — passed through to backend
  as opaque metadata.

**Backend validation:**
- Known key (`alloc`) + unparseable value → error.
- Unknown key → silently ignored (forward compatibility).
- Physical addresses validated against the target memory map.

### 3.27 `observable` and `volatile` Metadata \[2026-07-12: Phase 8G\]

`observable` marks a function or variable access as having side effects
visible outside the Brief program. DCE must preserve calls to observable
functions. `volatile` prevents LLVM from reordering or redundantly loading.

```brief
defn print_int(n: Int) -> Bool {
    !> observable: true;
    !> llvm_asm: "call @printf";
    !> interpreter_impl: "rust_print_int";
};
```

Both are implicitly set by `alloc(0x...)` — physical MMIO accesses are
always observable and volatile. Default for all other bindings is `false`.

### 3.28 Top-Level Scripting \[2026-08-01: Phase 4\]

When a `.bv` file contains only bare top-level `let` bindings / `const`
declarations — or a single `defn main()` with no explicit `entry!` — the
flat-scripting plugin synthesizes a **one-shot opening node**:

```brief
// No defn, no txn, no node — this is a script (bare let bindings).
let x: Int = 42;
let y: Int = x + 1;
```

becomes:

```brief
let __script_done: Bool = false;
node __script_main [__script_done == false][__script_done] {
    let x: Int = 42;
    let y: Int = x + 1;
    __script_done = true;
};
```

The guard `[__script_done == false]` is true exactly once; the final flip
makes it false afterward. `[true]` is never emitted.

A `defn main() -> Int { ... }` (no `entry!`) is also wired to run exactly once
via the same synthesized node (calling the renamed `brief_main`), fixing the
dead-code gap where a plain `defn main` was defined but never invoked.

**Rules:**
- Scripting mode activates only when the file has zero reactive `node`/`txn`,
  zero `sync<group>`, zero non-`main` `defn`, and zero explicit `entry!`.
- `__script_main` / `__script_done` are compiler-reserved; a user binding
  with either name is a compile error (no silent shadowing).
- Scripting and explicit `entry!` are mutually exclusive in the same file.

### 3.29 `.f` Layout Parsing (Formatted Brief) \[2026-07-12: Phase 16C\]

Files with the `.f` modifier (e.g., `main.f.bv`, `server.f.c.bv`) use
indentation instead of braces and semicolons. The layout pre-processor
injects virtual `{`, `}`, and `;` tokens based on indentation changes.

```brief
// main.f.bv — same semantics as standard Brief
defn add(a: Int, b: Int) -> Int
    a + b         // indented body — virtual { } inserted
                  // virtual ; at end of line
```

**Rules:**
- Tabs and spaces cannot be mixed in the same file (clear error if mixed).
- The pre-processor runs before the lexer — the rest of the pipeline
  sees standard braces and semicolons.
- Valid with any variant: `.f.bv`, `.f.ebv`, `.s.f.bv`, etc.

### 3.30 `.c` Cell Files and `input`/`output` Keywords \[2026-07-12: Phase 16D\]

Files with the `.c` modifier (e.g., `server.c.bv`, `sensor.c.ebv`) are
automatically wrapped in `cell <stem> { ... }`. The `input` and `output`
keywords declare the cell's parameters and return type.

```brief
// server.c.bv — becomes: cell server(port: UInt16, verbose: Bool) -> status: Int { ... }
input port: UInt16;
input verbose: Bool;
output status: Int;

state running: Bool = false;
txn start { running = true; };
```

**Rules:**
- `input` and `output` are only valid in `.c.bv` files.
- Multiple `input` declarations are allowed (one per parameter).
- Only one `output` declaration. If omitted, the cell has no return type.
- `input` parameters are ephemeral — passed per invocation, not persisted.

### 3.31 Metadata Dispatch and Distributed Validation

Metadata follows a key-value pattern with a prefix convention:

| Prefix | Consumed by |
|--------|-------------|
| `alloc` | Frontend + all backends |
| `llvm_*` | `brief-llvm` backend |
| `circt_*` | `brief-circt` hardware backend |
| `hls_*` | `brief-circt` HLS pass |
| `wasm_*` | `brief-webstack` backend |
| `gpu_*` | GPU backends |
| `interpreter_*` | Compile-time interpreter |
| No prefix | All backends (standard Brief) |

**Validation rules:**
1. **Unknown key** → silently ignored. Forward compatibility.
2. **Known key + supported value** → emit code.
3. **Known key + unparseable value** → **error**. The backend recognizes
   the key but cannot fulfill the value.

See `docs/architecture/features/metadata-dispatch.md` for the full
architecture.

---

## 4. Type System

### 4.1 Primitive Types

| Type | Size | Description | Aliases |
|------|------|-------------|---------|
| `Int` | 64-bit | Signed integer | `Signed`, `Sgn`, `I64` |
| `UInt` | 64-bit | Unsigned integer | `Unsigned`, `U64` |
| `Float` | 32-bit | IEEE 754 float | `F32` |
| `Float64` | 64-bit | IEEE 754 double | `Double`, `F64` |
| `Bool` | 1-bit | Boolean | - |
| `Char` | 32-bit | Unicode codepoint | - |
| `String` | variable | UTF-8 string | - |
| `Data` | variable | Opaque binary | `Bytes`, `[u8]` |
| `Void` | 0-bit | Unit type | `()` |

**Type literals:**
```brief
let i: Int = 42;
let u: UInt = 42u;
let f: Float = 3.14;
let f64: Float64 = 3.14f64;
let b: Bool = true;
let c: Char = 'a';
let s: String = "hello";
let d: Data = Data::from_bytes([1, 2, 3]);
```

### 4.2 Compound Types

**Lists (dynamic arrays):**
```brief
let list: List<Int> = [1, 2, 3];
let empty: List<String> = [];

// Operations
list .^Len;           // Length
list[i];              // Index access
list[i..j];           // Slice
list + [4];           // Concatenation
list.contains(2);     // Membership
```

**Vectors (fixed-size arrays):**
```brief
let vec: Int[5] = [1, 2, 3, 4, 5];
let matrix: Float[3][3];  // 3x3 matrix

// Operations
vec[i];             // Index access (bounds-checked)
vec .^Len;          // Size (compile-time constant)
```

**Options (nullable types):**
```brief
let opt: Option<Int> = Some(42);
let none: Option<Int> = None;

// Methods
opt.is_some();      // true if Some
opt.is_none();      // true if None
opt.unwrap();       // Extract value (panics if None)
opt.unwrap_or(0);   // Extract or default
opt.map(|x| x * 2); // Transform if Some
```

**Results (error handling):**
```brief
let result: Result<Int, String> = Ok(42);
let err: Result<Int, String> = Err("error");

// Methods
result.is_ok();     // true if Ok
result.is_err();    // true if Err
result.unwrap();    // Extract Ok value
result.unwrap_err(); // Extract Err value
result.map(|x| x * 2);  // Transform Ok
result.map_err(|e| e .^Len); // Transform Err
result.and_then(|x| Ok(x * 2)); // Chain operations
```

**Tuples:**
```brief
let pair: (Int, String) = (42, "answer");
let triple: (Int, Bool, Float) = (1, true, 3.14);

// Access
let (x, y) = pair;  // Destructuring
let first = pair.0; // Field access
let second = pair.1;
```

**Unions:**
```brief
let value: Int Union String Union Bool = 42;

// Pattern matching
unification value(Int(n)) = n;
unification value(String(s)) = s .^Len;
unification value(Bool(b)) = if b { 1 } else { 0 };
```

### 4.3 Custom Types

**Structs:**
```brief
struct Point {
    x: Int,
    y: Int
};

let p: Point = Point { x: 10, y: 20 };
let x = p.x;
```

**Enums:**
```brief
enum Color {
    Red,
    Green(Int),  // With data
    Blue(Int, Int, Int)
};

let c: Color = Color::Green(255);
```

**Type aliases:**
```brief
type UserId = Int;
type Name = String;
type Point2D = (Int, Int);

let id: UserId = 42;
```

### 4.4 Type Conversions

**Implicit conversions:**
```brief
let i: Int = 42;
let f: Float = i;      // Int → Float (widening)

let u: UInt = 100;
let i2: Int = u;       // UInt → Int (if fits)
```

**Explicit casts:**
```brief
let f: Float = 3.14;
let i: Int = f as Int;  // Float → Int (truncates)

let s: String = "42";
let i2: Int = s as Int; // String → Int (parses)
```

**Type constructors:**
```brief
let s: String = String(42);      // Int → String
let i: Int = Int("42");          // String → Int
let f: Float = Float(42);        // Int → Float
let c: Char = Char(65);          // Int → Char ('A')
```

### 4.5 Generics

Functions and types can be generic:

```brief
// Generic function
defn identity<T>(x: T) -> T {
    term x;
};

// Generic struct
struct Box<T> {
    value: T
};

// Generic enum
enum Result<T, E> {
    Ok(T),
    Err(E)
};

// Usage
let x = identity<Int>(42);
let b: Box<String> = Box { value: "hello" };
let r: Result<Int, String> = Ok(42);
```

**Generic constraints (future):**
```brief
// Trait bounds (planned)
defn max<T: Ord>(a: T, b: T) -> T {
    [a >= b] { term a; };
    term b;
};

// Where clauses (planned)
defn process<T, U>(t: T, u: U) -> String
    where T: Debug, U: Debug
{
    term t.debug() + u.debug();
};
```

### 4.6 Type Inference

Brief can infer types in many contexts:

```brief
// Variable type inference
let x = 42;           // Inferred: Int
let s = "hello";      // Inferred: String
let list = [1, 2, 3]; // Inferred: List<Int>

// Function return type inference
defn add(a: Int, b: Int) {
    term a + b;  // Inferred: Int
};

// Generic type inference
defn make_pair<T>(a: T, b: T) -> (T, T) {
    term (a, b);
};

let p = make_pair(1, 2);  // Inferred: (Int, Int)
```

---

### 4.7 Type Derivation (`type` keyword)

> **Added 2026-06-09 (Phase 1.5)**

Brief types are defined via `Type Name : [Parent] [Protocol] { ... }` declarations. The old `<:`
operator (read as "derives from") connects a new type to its base type. Properties
and constraints within the `{ }` body define how the new type differs from the base.

#### 4.7.1 Primitive Kernel (compiler-native properties)

| Property | Type | Default | Meaning |
|----------|------|---------|---------|
| `Bytes` | `Int` | _required_ | Physical width in bytes — LLVM `alloca`, VHDL width |
| `Alignment` | `Int` | `= Bytes` | Alignment boundary — LLVM `align` |
| `Endian` | `Enum` | `Little` | Byte order — LLVM `bswap`/load-store order |
| `Volatile` | `Bool` | `false` | LLVM `load volatile`/`store volatile` |
| `Atomic` | `Bool` | `false` | LLVM atomic operations |
| `ElementType` | `Type` | _(none)_ | Unlocks `[]` and slicing — compiler synthesizes GEP |
| `FixedSize` | `Bool` | _(none)_ | `false` unlocks `<-`/`->` — heap/circular buffer |
| `InsertAt` | `Expr` | _(none)_ | Index expression for insertion position |
| `ExtractFrom` | `Expr` | _(none)_ | Index for extraction position |
| `AllowIndex` | `Bool` | `true` | Override to `false` to block `[]` |
| `AllowSlice` | `Bool` | `true` | Override to `false` to block slicing |
| `AllowArrow` | `Bool` | `true` | Override to `false` to block `<-`/`->` |
| `Codec` | `Struct` | _(none)_ | Struct with `encode`/`decode` — literal translation |

#### 4.7.2 InsertAt / ExtractFrom

| Expression | Strategy | Example |
|---|---|---|
| `0` | Constant front, head-pointer advance | Queue pop |
| `.^Len` | Append position, pointer increments | List/Queue push |
| `.^Len - N` | Offset from end, pointer decrements | Stack pop |
| `: { MIN(.key) }` | Maintain heap by key | Priority queue |
| `: { MAX(.key) }` | Maintain heap by key | Priority queue |

Unrecognized expression forms produce a compile-time error in Pass 1.

#### 4.7.3 Examples

```brief
// Scalar derivation
Type U8  : Bits { !> Bytes: 1; !> Alignment: 1; };
Type U32 : Bits { !> Bytes: 4; !> Alignment: 4; };
Type Int : U64;
Type MmioReg : U32 { !> Volatile: true; };

// Collection derivation
Type List<T> : Bits {
    !> ElementType: T;
    !> FixedSize: false;
    !> InsertAt: .^Len;
    !> ExtractFrom: .^Len - 1;
};

Type Stack<T> : List<T> { AllowIndex = false; };
Type Queue<T> : List<T> { ExtractFrom = 0; AllowIndex = false; };

// Codec-bearing type
import { UTF8 } from "std/UTF8.bv";
Type String : List<U8> { Codec = UTF8; };

// Refinement constraint
Type PositiveInt : Int {
    [ > 0 && < 100 ]
};
```

#### 4.7.4 Two-Pass Pipeline

```
PASS 1: Type-Universe Pass
  - Collect all Type declarations
  - Resolve derivation chain to Bits
  - Inherit + override properties
  - Validate Bytes on all Bits-derived types
  - Validate InsertAt/ExtractFrom forms
  - Validate Codec has encode/decode
  - FREEZE: type universe immutable

PASS 2: Executable Pass
  - Parse defn/txn/node
  - Resolve let x: Type against frozen universe
  - Validate reflection targets against metadata
  - Synthesize bracket/arrow from AllowIndex/AllowArrow
  - Encode literals via Codec
  - Emit backend code with frozen metadata
```

---

## 5. Foreign Function Interface (FFI)

### 5.1 FFI Declaration

**Foreign signatures:**
```brief
// Standard FFI (must handle Result)
frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// Fire-and-forget (no return)
frgn! log(msg: String);

// Kernel syscall with return
syscall read(fd: Int, buf: Data, count: Int) -> Result<Int, IOError> from "kernel.toml";

// Kernel syscall without return
syscall! exit(code: Int);
```

**FFI keywords:**
- `frgn` - Foreign function, returns `Result<T, E>`, must handle
- `frgn!` - Foreign function, returns `void`, fire-and-forget
- `syscall` - Kernel call, returns `Result<Int, E>`, must handle
- `syscall!` - Kernel call, returns `void`, no handling needed

**Syscall number resolution:** Syscall numbers are defined in the target specification (`.toml` or `.dbvs` files), not hardcoded in the compiler. The compiler resolves `SYS_OPEN` → `2` (x86_64 Linux) or `56` (AArch64 Linux) at compile time via the active target spec's `[syscalls]` section. This keeps the compiler target-agnostic and allows adding new OS targets without modifying Rust source code.

### 5.2 FFI Type Mapping

| Brief Type | C Type | Rust Type | Python Type |
|------------|--------|-----------|-------------|
| `Int` | `int64_t` | `i64` | `int` |
| `UInt` | `uint64_t` | `u64` | `int` |
| `Float` | `float` | `f32` | `float` |
| `Float64` | `double` | `f64` | `float` |
| `Bool` | `bool` | `bool` | `bool` |
| `Char` | `char32_t` | `char` | `str` (len=1) |
| `String` | `const char*` | `&str` | `str` |
| `Data` | `uint8_t*` | `&[u8]` | `bytes` |

### 5.3 Error Handling

**Result handling patterns:**
```brief
// Pattern 1: Guard-based
let result = sqrt(4.0);
[result.is_ok()] {
    let value = result.value;
    log("Success: " + String(value));
};
[result.is_err()] {
    log("Error: " + result.error.message);
};

// Pattern 2: Unification
unification result(Ok(value)) = {
    log("Success: " + String(value));
};
unification result(Err(e)) = {
    log("Error: " + e.message);
};

// Pattern 3: Combinators
let value = result.unwrap_or(0.0);
let doubled = result.map(|x| x * 2.0);
```

**Error types:**
```brief
enum MathError {
    DomainError(String),
    Overflow,
    Underflow
};

enum IOError {
    NotFound(String),
    PermissionDenied,
    TimedOut,
    Other(String)
};
```

### 5.4 Compiler Directives

**Brief's pragma philosophy (2026-06-06):** In other languages, pragmas exist so the programmer can feed the compiler hints to optimize better — they require deep systems-level insight. In Brief, the compiler runs at full speed by default — inlining, folding, precomputing, dead-field-eliminating — with maximum zealotry. **Pragmas are the programmer's way to request the compiler calm down on a specific point.** Not "help me optimize" but "I understand you can prove this is dead, but I need it alive anyway."

Every pragma follows this pattern:
- `#out` — "Calm down, this FFI call has external effects you can't see"
- `#!out(x)` — "Calm down, this field reaches hardware/I/O you can't model"
- `#assume_event(x)` — "Calm down proof engine, trust that `x` fires"
- `#assume_shape(g, a)` — "Calm down, the guard+action contract is valid"

The programmer holds the authority — the compiler defers. This is teachable in one sentence: **"Brief runs at full speed by default. A pragma is a request to the compiler to hold back its zealotry on a specific point."**

Three syntax forms are supported:

#### 5.4.1 `#pragma` Syntax (Recommended)

Item-level (single target):
```brief
#pragma.c           // Target: C backend (replaces #[c])
#pragma.rust        // Target: Rust backend
#pragma.c optimize(3)  // Target + value
```

File-level (multiple directives):
```brief
#!pragma ffi.c, bind("./bindings.toml"), import("./lib.a"), map("uint", "uint32_t")
```

#### 5.4.2 `#[...]` Syntax (Deprecated)

The bracket-based syntax still works but emits a deprecation warning:

```brief
#![ffi.c, bind("./bindings.toml"), import("./lib.a"), map("uint", "uint32_t")]

frgn custom_func(x: Int) -> Result<Int, Error> from "custom.toml";
```

**Directives:**
- `ffi.c` / `#pragma.ffi.c` - C FFI
- `ffi.rust` / `#pragma.ffi.rust` - Rust FFI
- `ffi.python` / `#pragma.ffi.python` - Python FFI
- `ffi.wasm` / `#pragma.ffi.wasm` - WASM FFI
- `bind("path.toml")` - Binding configuration
- `import("lib.a")` - Link library
- `map("brief_type", "foreign_type")` - Type mapping

#### 5.4.3 `#!exit` — Program Termination Condition

The `#!exit` directive declares a condition under which the program should
terminate. It appears at the top level of a program:

```brief
#!exit count >= 1000000;
```

When the exit condition is proven to converge (e.g., a monotonic counter
reaching a bound), the compiler may fold the entire tick loop to a single
store (`O(1)`). If convergence cannot be proven, the program falls back to
`O(N)` reactor ticks with the exit check on each iteration.

The exit condition is also used by **dead-field elimination**: fields that
are not referenced by the exit condition or any transaction precondition are
considered dead and their stores are eliminated.

If a program has wake triggers but no `#!exit` condition, a warning is emitted.

#### 5.4.4 `#assume_event(trigger_name)` — Liveness Fairness Assumption

Declares that the named trigger **will** fire eventually. This enables the proof
engine to prove termination for reactive transactions with wake triggers:

```brief
#assume_event(stdin_ready)
node [count < total][count == total] {
    count = count + 1;
    term;
}
```

Without `#assume_event`, the compiler cannot prove that an external-trigger loop
will terminate, because it has no knowledge of trigger scheduling. With it, the
compiler assumes the trigger fires and can prove convergence through the
bounded precondition + increments.

**Effect on optimization:** Enables pure-counter fold elimination for reactive
transactions that would otherwise be skipped due to wake triggers. No LLVM IR
is emitted for the pragma — it is purely a proof-engine constraint.

#### 5.4.5 `#assume_shape(guard_expr, action)` — Shape Guard with Fast-Path

Declares that `guard_expr` is expected to be true at runtime. The compiler
generates a runtime guard check and splits execution into fast/slow paths:

```brief
#assume_shape(packet, escape)
node [*][*] {
    &processed = processed + 1;
    term;
}
```

**Action** (rollback on guard failure):
- `escape` — silently skip the transaction (default)
- `run` — execute the full body with all safety checks
- `exit` — call `__exit(1)` and unreachable

**Future work:** The fast path will eventually strip runtime type checks
when the guard is proven to hold. Currently the guard is a constant `true`
and only the rollback action infrastructure is emitted.

### 5.5 Annotations (`#`, `#!`, `#?`)

Brief provides a lightweight annotation system for attaching compiler directives
to items (definitions, transactions, types). Annotations are distinct from
metadata (`!>`) — they tell the compiler **what to do**, not **what something is**.

| Form | Mode | Meaning |
|------|------|---------|
| `#gpu` | Advisory | Hint: prefer GPU offloading |
| `#!out` | Mandatory | Requirement: has observable external effects |
| `#?gpu` | Advisory + diagnostic | Hint + explain the compiler's decision |
| `#?!gpu` | Mandatory + diagnostic | Requirement + explain the compiler's decision |
| `#!?gpu` | Mandatory + diagnostic | Same as `#?!gpu` (alternative ordering) |
| bare `#?` | Advisory + diagnostic | Enable diagnostics for ALL passes on this item |

**Diagnostic output**: When `#?` is present, the compiler emits pass-level
explanations at compile time:
```
[my_func] gpu: NOT offloaded (body contains non-GPU-safe intrinsic)
[my_func] vectorize: vectorized by factor 4 (trip count >= 4)
```

Annotations appear on the signature line, before the item keyword:
```brief
#?gpu defn my_compute() -> Int { term 42; };
#!out txn write_port() [*][*] { &port = value; term; };
```

### 5.6 Inline Metadata (`!>`)

The `!> key: value;` declaration attaches compile-time metadata to items.
Unlike `#` annotations (which are compiler directives), `!>` declarations are
declarative data — they describe properties of the annotated item.
(2026-08-01: the old `<~` (Annotation Arrow) syntax was removed; `!>` is the
sole metadata-declaration form. Writing `<~` is a parse error.)

**Inside type bodies**, `!>` declares type properties:
```brief
type UInt32 : Bits {
    !> bytes: 4;
    !> alignment: 4;
    !> storage: Native;
};
```

**Inside definition/transaction bodies**, `!>` at the body top declares
item-level metadata:
```brief
defn process() -> Int {
    !> jira: "FIN-8422";
    !> priority: 2;
    term 42;
};
```

**Inside guard branches**, `!>` declares branch-scoped metadata:
```brief
txn compute [count < N][count == N] {
    [count % 2 == 0] {
        !> priority: 1;
        &even = even + 1;
    };
};
```

**Variable metadata** is reserved for future use (`!> x: (key: val);` after a
`let` binding is recognized syntax but produces a compile-time error).

### 5.7 Type Property System

Every type carries a `properties` map at runtime (`HashMap<String, PropertyValue>`)
populated from `!>` declarations in the type body. Well-known property names
like `bytes`, `alignment`, `llvm`, `storage`, and `tbaa` are dual-written to
both the map AND the corresponding hardcoded `ResolvedType` field during the
Phase 1B–2 migration window.

| Property | Type | Example | Purpose |
|----------|------|---------|---------|
| `bytes` | `Int` | `!> bytes: 8;` | Physical width in bytes |
| `alignment` | `Int` | `!> alignment: 4;` | Memory alignment |
| `llvm` | `String` | `!> llvm: "i64";` | LLVM IR type string |
| `storage` | `Identifier` | `!> storage: Native;` | "Boxed" (i64) or "Native" (float regs) |
| `tbaa` | `String` | `!> tbaa: "Int";` | TBAA type tree node name |
| `box` | `String` | `!> box: "ptrtoint#";` | Boxing intrinsic (Native → Boxed) |
| `unbox` | `String` | `!> unbox: "inttoptr#";` | Unboxing intrinsic (Boxed → Native) |

Codegen queries the property system via `TypeUniverse` convenience methods:
`llvm_type_for()`, `byte_size_for()`, `is_native()`, `tbaa_for()`,
`alignment_for()` — each with hardcoded fallback during migration.

### 5.8 Resource Lifecycle

Resources are declared and managed:

```brief
// Declare resource
rsrc file: File("data.txt", "read");

// Use in transaction
txn read_data() [file.exists()][data .^Len > 0] {
    let result = file.read();
    [result.is_ok()] {
        data = result.value;
    };
    term;
};

// Resource is automatically closed when out of scope
```

\[Added 2026-05-29\]

### 5.2 Dynamic Linking

Inline `frgn` declarations with `from "lib.so"` replace TOML-based FFI bindings. The compiler resolves foreign functions at runtime via dynamic linking (`dlsym`).

**Syntax:**
```brief
// Primitive types — direct C ABI via dlsym (Tier 1)
frgn strlen(s: String) -> Int from "libc.so.6";

// Complex types — shared memory protocol (Tier 2, via metropolitan)
frgn process_json(input: JsonValue) -> JsonValue from "libprocessor" via metropolitan;
```

**Tier 1: Direct Dynamic Linking (default)**
- Functions with primitive parameter/return types (`Int`, `Float`, `Bool`, `Char`, `String`, `Void`) are called via `dlsym` through `libloading`.
- Brief values are auto-converted to C ABI (strings null-terminated, booleans become `i32`, etc.).
- Overhead: ~1-2ns per call (same as native C function pointer).

**Tier 2: Metropolitan Protocol (via metropolitan)**
- Functions with compound types (`List`, `Enum`, `Struct`, `Tuple`) use shared memory + atomic handshake.
- Byte layout is computed at compile time (`compute_layout()`).
- Communication via `/dev/shm/metro_<name>` with CAS handshake: `IDLE → REQ → ACK → RES → IDLE`.
- Requires the `via metropolitan` clause explicitly.

**Bootstrap Set:**
The interpreter provides exactly 4 built-in functions — the minimum needed to load source files before any `.so` can be resolved:

| Function | Signature | Purpose |
|----------|-----------|---------|
| `__read_file` | `(path: String) -> Option<String>` | Load source code |
| `__write_file` | `(path: String, data: String) -> Bool` | Write output |
| `__print` | `(msg: String) -> Bool` | Debug output (frgn) |
| `__exit` | `(code: Int)` | Termination (frgn!, fire-and-forget) |

All other foreign functionality (`strlen`, `is_digit`, file I/O, math functions, etc.) is declared as `frgn` in `lib/std/` and resolved at runtime. The compiler contains no hardcoded FFI beyond the bootstrap set.

**TOML Migration:**
All existing `from "*.toml"` declarations remain valid but are deprecated. The recommended pattern is:
```brief
// Old (deprecated):
frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// New:
frgn sqrt(x: Float) -> Float from "libm.so.6";
```

### 5.6 FFI Registry (No-Magic Architecture)

The interpreter maintains an **FFI registry** that maps location keys (e.g., `"std::HashMap::insert"`, `"std::string::len"`) to Rust-side handler functions. This replaces the previous pattern of hardcoded string matches on function names.

**Architecture:**
1. All built-in operations are registered in `ffi_name_to_location` with a unique location key
2. The `foreign_functions` map associates each location key with a Rust closure/handler
3. Stdlib modules in `lib/std/__builtin/` declare `frgn` signatures that import these registered operations as if they were true FFI calls
4. The `__builtin.dbvs` schema file documents all registered location keys and their type signatures

**Key benefit:** No hardcoded `fn_name == "insert"` string matching anywhere in the interpreter. All dispatch goes through the registry path — the same path used for C, Rust, and Zig FFI. This means:
- Adding a new built-in operation is a matter of registering it in the FFI registry and writing a `frgn` declaration in the corresponding `.bv` file
- The interpreter, LLVM backend, and any future backends all use the same resolution path
- The registry is transparent and inspectable via `--explain ffi`

**Migration status:** All collection operations (HashMap, HashSet, Stack, Queue, StringBuilder) and string operations are registered. Older direct-method-dispatch paths (`dispatch_method_by_type`) are deleted. The interpreter no longer contains any hardcoded Rust string matches that serve as "built-in" functions.

---

## 6. Standard Library

### 6.1 Core Modules

| Module | Description | Key Functions |
|--------|-------------|---------------|
| `std/math` | Mathematical operations | `abs`, `sqrt`, `sin`, `cos`, `pow`, `min`, `max` |
| `std/string` | String manipulation | `len`, `concat`, `find`, `split`, `replace`, `trim` |
| `std/collections` | Data structures | `List`, `HashMap`, `HashSet`, `Stack`, `Queue` |
| `std/option` | Option type methods | `is_some`, `is_none`, `unwrap`, `map`, `and_then` |
| `std/result` | Result type methods | `is_ok`, `is_err`, `unwrap`, `map`, `map_err` |
| `std/bits` | Bit manipulation | `popcount`, `leading_zeros`, `trailing_zeros`, `abs`, `bit_reverse`, `ffs`, `is_power_of_two`, `rotate_left`, `rotate_right` |
| `std/ptr` | Safe pointer operations | `read_i64`, `write_i64`, `address`, `read_byte`, `copy` |
| `std/os/io` | File I/O (read/write/seek) | `open`, `read`, `write`, `close`, `lseek` |
| `std/os/user` | User/group identity | `getuid`, `geteuid`, `getgid`, `getegid` |
| `std/os/time` | System time | `clock_gettime`, `nanosleep`, `gettimeofday` |
| `std/os/env` | Environment variables | `getenv`, `setenv`, `unsetenv` |
| `std/os/signal` | Signal handling | `signal`, `kill`, `sigaction` |
| `std/os/socket` | Networking | `socket`, `bind`, `listen`, `accept`, `connect` |
| `std/os/mman` | Memory mapping | `mmap`, `munmap`, `mprotect` |
| `std/os/sched` | Scheduling | `sched_yield`, `sched_getattr` |
| `std/os/sysinfo` | System information | `uname`, `sysconf` |
| `std/os/resource` | Resource limits | `getrlimit`, `setrlimit`, `getrusage` |
| `std/os/thread` | POSIX threads | `pthread_create`, `pthread_join`, `pthread_mutex_lock` |
| `std/os/process` | Process management | `fork`, `execvp`, `waitpid`, `exit` |
| `std/os/tty` | Terminal I/O | `tcgetattr`, `tcsetattr`, `isatty` |
| `std/os/dir` | Directory operations | `opendir`, `readdir`, `mkdir`, `rmdir` |
| `std/os/temp` | Temporary files | `mkstemp`, `mkdtemp` |
| `std/os/dynlink` | Dynamic linking | `dlopen`, `dlsym`, `dlclose` |
| `std/os/debug` | Debugging | `ptrace`, `get_backtrace` |
| `std/os/ipc` | Inter-process communication | `pipe`, `shm_open`, `sem_open`, `mq_open` |
| `std/os/ring` | Ring buffer | `ring_create`, `ring_push`, `ring_pop`, `ring_free` |
| `std/os/core` | Core I/O | `read`, `write`, `open`, `close` — micro-optimized |
| `std/os/atomic` | Atomic operations | `atomic_load`, `atomic_store`, `atomic_fetch_add`, `cmpxchg`, `fence` |
| `std/rt` | Runtime | `__rt_init`, `__rt_alloc`, `__rt_free` via `frgn` from `brief_rt.c` |

All `std/os/` modules are prelude-imported automatically. Each module contains `inop` declarations that call `brief_rt.c` wrapper functions (or direct LLVM IR for atomics).

### 6.2 Math Module

```brief
import "std/math";

// Basic operations
let abs_val = math.abs(-42);           // 42
let min_val = math.min(10, 20);        // 10
let max_val = math.max(10, 20);        // 20
let sum = math.add(5, 3);              // 8

// Float operations
let sqrt_val = math.sqrt(16.0);        // 4.0
let sin_val = math.sin(3.14 / 2.0);    // ~1.0
let pow_val = math.pow(2.0, 3.0);      // 8.0

// Integer operations
let gcd_val = math.gcd(48, 18);        // 6
let lcm_val = math.lcm(4, 6);          // 12
let fact = math.factorial(5);          // 120
let fib = math.fibonacci(10);          // 55
```

### 6.3 String Module

```brief
import "std/string";

let s = "Hello, World!";

// Basic operations
let len = string.len(s);               // 13
let lower = string.to_lower(s);        // "hello, world!"
let upper = string.to_upper(s);        // "HELLO, WORLD!"

// Search
let contains = string.contains(s, "World");  // true
let idx = string.find(s, "World");     // 7
let starts = string.starts_with(s, "Hello"); // true

// Manipulation
let trimmed = string.trim("  hello  "); // "hello"
let replaced = string.replace(s, "World", "Brief");  // "Hello, Brief!"
let parts = string.split(s, ", ");     // ["Hello", "World!"]

// Substring
let sub = string.substr(s, 7, 12);     // "World"
```

### 6.4 Collections Module

```brief
import "std/collections";

// Lists
let list = [1, 2, 3];
let len = list .^Len;                  // 3
let appended = list + [4];             // [1, 2, 3, 4]
let contains = list.contains(2);       // true
let idx = list.find(2);                // 1
let sliced = list[1..3];               // [2, 3]

// HashMaps (requires Hash + Eq)
let map = HashMap::new();
map = map.insert("key", 42);
let val = map.get("key");              // Some(42)
let has = map.contains_key("key");     // true

// HashSets
let set = HashSet::new();
set = set.insert(1);
set = set.insert(2);
let has = set.contains(1);             // true

// Stacks
let stack = Stack::new();
stack = stack.push(1);
stack = stack.push(2);
let (val, stack) = stack.pop();        // (Some(2), stack with [1])

// Queues
let queue = Queue::new();
queue = queue.enqueue(1);
queue = queue.enqueue(2);
let (val, queue) = queue.dequeue();    // (Some(1), queue with [2])
```

### 6.5 IO Module

```brief
import "std/io";

// Console I/O
io.print("Hello");
io.println("World");
let input = io.input();  // Read line from stdin

// File I/O (FFI-backed)
let content = io.read_file("data.txt");
io.write_file("output.txt", content);
let exists = io.file_exists("data.txt");

// Formatting
io.format("Value: {}", 42);
io.formatln("Name: {}, Age: {}", "Alice", 30);
```

### 6.6 JSON Module

```brief
import "std/json";

// Serialization
let obj = json.object([("name", "Alice"), ("age", 30)]);
let json_str = json.to_string(obj);    // '{"name":"Alice","age":30}'

// Deserialization
let parsed = json.from_string(json_str);
let name = parsed.get("name");         // "Alice"
let age = parsed.get("age");           // 30

// Convenience
let json_str2 = json.to_json(value);
let value2 = json.from_json(json_str2);
```

### 6.7 Time Module

```brief
import "std/time";

// Current time
let now = time.now();                  // Current timestamp (seconds)
let now_ms = time.now_millis();        // Current timestamp (milliseconds)

// Durations
let duration = time.duration_seconds(60);
let sleep_time = time.duration_millis(500);

// Operations
let later = time.add_seconds(now, 60);
let diff = time.diff_seconds(later, now);  // 60

// Sleeping
time.sleep(time.duration_millis(100));
```

### 6.10 String Pattern Match Module

Compile-time regex compilation (planned; see §3.17). The DFA is
compiled during parsing using Thompson construction → subset construction.
The transition table is embedded as a constant; the scan loop is O(n) linear.

```brief
let found : "hello@example.com"["^[a-z]+@[a-z]+\\.[a-z]+$"];
```

### 6.11 RooflineAnalyzer

The `RooflineAnalyzer` (§3.18) uses hardware bottleneck constraints to guide
precompute/fold decisions. Configured via `bottlenecks.dbvs` schema files.

**Public API:**
- `compute_roofline(flops, bytes_moved)` → compute-bound vs memory-bound
- `lut_fits_cache(lut_size_bytes)` → `Some(CacheTier)` or `None`
- `should_precompute_as_lut(iterations, size, reuse)` → fold decision

---

## 7. Address System (Embedded)

The `@` operator provides memory-mapped access:

```brief
// Raw physical address (embedded only)
let reg: Int @ 0x40020000;

// Virtual address (OS-managed)
let buffer: Data @virtual:0x1000;

// Stack-relative
let local: Int @stack:0;

// Heap-relative
let heap_var: Int @heap:0;

// Bit-range addressing
let field: Int @0x40020000.0..4;  // Bits 0-4 at address
```

**Address modes:**
- `@0xADDR` - Raw physical address (`.ebv` only)
- `@virtual:ADDR` - Virtual address (OS-managed)
- `@stack:OFFSET` - Stack-relative offset
- `@heap:OFFSET` - Heap-relative offset
- `@ADDR.START..END` - Bit range within address

**Target behavior:**
| Target | `@` Semantics |
|--------|---------------|
| `.bv` (Native) | Virtual address, OS-managed |
| `.rbv` (WASM) | Linear memory offset |
| `.ebv` (Embedded) | Raw physical address |
| `.ebv` (FPGA) | Register/memory-mapped I/O |

---

## 8. Data Brief (Configuration)

Data Brief provides schema-enforced configuration:

### 8.1 Data Brief Schema (`.dbvs`)

```brief
// hardware.dbvs
schema Hardware {
    name: String,
    version: String,
    fpga: FPGAConfig,
    peripherals: [Peripheral],
    memory: MemoryMap
};

schema FPGAConfig {
    family: String,
    part: String,
    package: String,
    speed_grade: Int
};

schema Peripheral {
    name: String,
    type: String,
    address: Int,
    interrupt: Option<Int>
};

schema MemoryMap {
    regions: [MemoryRegion]
};

schema MemoryRegion {
    name: String,
    start: Int,
    size: Int,
    access: String  // "rw", "ro", "wo"
};

// Aliases for reuse
alias CommonPeriph = {
    name: String,
    address: Int
};
```

### 8.2 Data Brief (`.dbv`)

```brief
// hardware.dbv
import "hardware.dbvs";

Hardware {
    name: "MyBoard",
    version: "1.0.0",
    fpga: FPGAConfig {
        family: "Xilinx",
        part: "xc7a35t",
        package: "cpg236",
        speed_grade: -1
    },
    peripherals: [
        Peripheral {
            name: "UART",
            type: "serial",
            address: 0x40000000,
            interrupt: Some(3)
        },
        Peripheral {
            name: "GPIO",
            type: "gpio",
            address: 0x40001000,
            interrupt: None
        }
    ],
    memory: MemoryMap {
        regions: [
            MemoryRegion {
                name: "SRAM",
                start: 0x00000000,
                size: 65536,
                access: "rw"
            },
            MemoryRegion {
                name: "ROM",
                start: 0x00010000,
                size: 32768,
                access: "ro"
            }
        ]
    }
};
```

### 8.3 Data Brief Lines (`.dbvl`)

Line-based storage for large datasets:

```brief
// sensors.dbvl
schema SensorReading {
    timestamp: Int,
    sensor_id: Int,
    value: Float
};

// Data lines (one per reading)
1234567890, 1, 23.5
1234567891, 2, 45.2
1234567892, 1, 23.7
```

### 8.4 Validation

Data Brief validates against schema:

```brief
// Compile-time validation
brief check hardware.dbv  // Error if schema mismatch

// Access in Brief code
import "hardware.dbv";

let addr = hardware.peripherals[0].address;
let size = hardware.memory.regions[0].size;
```

---

## 9. Compiler Architecture

### 9.1 Compilation Pipeline

```
Source (.bv/.rbv/.ebv/.sbv/.s.rbv/.s.ebv)
    ↓
Lexer (tokenization)
    ↓
Parser (AST construction + strict mode enforcement)
    ↓
Type Checker (type inference, trait resolution)
    ↓
Proof Engine (symbolic execution, contract verification + strict escalation)
    ↓
[.srbv only] View-State Isomorphism Verification
    ↓
Backend (code generation)
    ↓
Target (Rust/C/WASM/Verilog/VHDL/COBOL)
```

### 9.2 Strict Brief Verification

**Strict Brief** (`.s.bv`, `.s.ebv`, `.s.rbv`) extends the standard Brief compiler pipeline with:

4. **Capability Requirements** (`.s.ebv`/`.s.rbv`): Strict embedded files require `hardware_triggers` capability; strict rendered files require `reactive_ui` capability.

Use `--strict` flag to apply strict mode to any file: `brief check --strict file.bv`

### 9.3 CLI Commands

```bash
# Check (type-check only, no codegen)
brief check file.bv

# Build (compile to default target)
brief build file.bv      # .bv → native Rust executable
brief build file.rbv     # .rbv → web app (WASM + JS + UI)
brief build file.ebv     # .ebv → error (needs --target)

# Compile with explicit target
brief compile file.bv --target rust.toml
brief compile file.ebv --target vhdl_fpga.toml
brief compile file.rbv --target nextjs.toml

# Backend-specific
brief wasm file.bv       # .bv → standalone WASM
brief wasm file.rbv      # .rbv → full web app
brief rust file.bv       # .bv → Rust source
brief c file.bv          # .bv/.ebv → C source
brief arm file.ebv       # .ebv → bare-metal Rust
brief verilog file.ebv   # .ebv → SystemVerilog
brief vhdl file.ebv      # .ebv → VHDL
brief cobol file.bv      # .bv → COBOL
brief llvm file.bv       # .bv → LLVM IR \[2026-05-29\]
brief aarch64 file.bv    # .bv → AArch64 binary \[2026-05-29\]
brief x86_64 file.bv     # .bv → x86_64 binary \[2026-05-29\]

# Run (build and execute)
brief run file.rbv       # Build and open in browser
brief run file.bv        # Build and execute native

# Project management
brief init my-app        # Create new project
brief import package     # Add dependency
brief lsp                # Start language server
```

### 9.3 Target Specifications

Targets are configured via TOML:

```toml
# rust.toml
[target]
name = "rust"
edition = "2021"
crate_type = "bin"

[features]
async = true
reactive = true

[output]
directory = "target"
```

```toml
# vhdl_fpga.toml
[target]
name = "vhdl"
family = "Xilinx"
series = "Artix-7"

[synthesis]
optimization = "speed"
fanout_limit = 10000

[pins]
clock = "CLK50"
reset = "RESETn"
```

---

## 10. Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| **Core Language** | | |
| Transactions (`txn`, `node`) | ✅ Complete | |
| Async transactions | ✅ Complete | With mutual exclusion checking |
| Definitions (`defn`) | ✅ Complete | With contracts |
| Guards | ✅ Complete | Guard-based control flow |
| Pattern matching | ✅ Complete | Via unification and match expression \[2026-05-29\] |
| Inline assembly | ✅ Complete | C backend support |
| **Type System** | | |
| Primitive types | ✅ Complete | Int, UInt, Float, Bool, String, Char, Data, Void |
| Lists | ✅ Complete | Dynamic arrays |
| Vectors | ✅ Complete | Fixed-size arrays |
| Options | ✅ Complete | Nullable types |
| Results | ✅ Complete | Error handling |
| Tuples | ✅ Complete | Multi-value |
| Structs | ✅ Complete | With methods |
| Enums | ✅ Complete | With data |
| Render structs | ⚠️ Migration | `rstruct` deprecated — use `render struct`/`render obj` |
| Generics | ⚠️ Partial | Syntax works, trait bounds pending |
| Traits | ❌ Planned | For generic constraints |
| `Ptr<T>` types | ✅ Complete | Verified pointer with compile-time bounds tracking \[2026-06-05\] |
| Reflection `.^`/`.^^` | ✅ Implemented | Len, Ptr (runtime); Size, Bytes, Alignment, Type (compile-time) \[2026-07-31\] |
| LLVM bit intrinsics | ⚠️ Declared, no operator form | ctpop, ctlz, cttz, abs, bitreverse \[2026-07-31: operator form removed with `:>`\] |
| Pointer dereference (`ptr[i]`) | ✅ Complete | Direct GEP for Ptr\<T\>; checked by PointerVerifier \[2026-06-05\] |
| Collection-query / regex-DFA | ❌ Planned | FILTER/GROUP/COUNT fusion + regex captures (not yet implemented) \[2026-07-31\] |
| RooflineAnalyzer | ✅ Complete | Cache-aware LUT sizing, roofline model via bottlenecks.dbvs \[2026-06-05\] |
| Bottleneck config | ✅ Complete | bottlenecks.dbvs schema for PCIe, cache, bandwidth, FPGA \[2026-06-05\] |
| **FFI** | | |
| Foreign signatures | ✅ Complete | `frgn`, `frgn!`, `syscall`, `syscall!` |
| `import "link/..."` LTO pipeline | ✅ Complete | C/Rust/Zig → bitcode → `llvm-link` → `opt -O2` \[2026-06-06\] |
| `compile_to_bitcode()` | ✅ Complete | Compiles C/Rust/Zig to LLVM bitcode via `clang`/`rustc`/`zig` \[2026-06-06\] |
| `link_and_optimize()` | ✅ Complete | `llvm-link` + `opt -O2` with `-vectorize-slp=false` \[2026-06-06\] |
| FFI Registry | ✅ Complete | No hardcoded string matching; all dispatch through `ffi_name_to_location` → `foreign_functions` \[2026-06-06\] |
| `__builtin.dbvs` | ✅ Complete | Schema documenting all registered FFI location keys \[2026-06-06\] |
| `brief_rt.c` as import | ✅ Complete | Runtime functions via `import "link/brief_rt.c"` not hardcoded \[2026-06-06\] |
| Resource declarations | ✅ Complete | `rsrc` keyword |
| Bit-packing | ✅ Complete | AST-level |
| Vector types | ✅ Complete | Embedded only |
| Address system | ✅ Complete | `@`, `@raw:`, `@stack:`, `@heap:` |
| **Data Brief** | | |
| Schema (`.dbvs`) | ✅ Complete | |
| Data (`.dbv`) | ✅ Complete | |
| Lines (`.dbvl`) | ✅ Complete | |
| Validation | ✅ Complete | Compile-time |
| **Standard Library** | | |
| `std/bits.bv` | ✅ Complete | Bit manipulation: popcount, leading_zeros, trailing_zeros, abs, bit_reverse, ffs, rotate \[2026-06-05\] |
| `std/ptr.bv` | ✅ Complete | Safe pointer ops: read_i64, write_i64, address, read_byte, copy \[2026-06-05\] |
| `lib/std/c/xxhash/` | ✅ Complete | Vendored xxHash v0.8.2 (xxhash.h + xxhash.c); LTO-coupled via `import "link/xxhash/xxhash.c"` \[2026-06-07\] |
| `std/xxhash.bv` | ✅ Complete | xxHash FFI declarations + convenience wrappers: `XXH64`, `XXH32`, `XXH3_64`, `XXH3_128` \[2026-06-07\] |
| **Backends** | | |
| Rust | ✅ Complete | Native executables |
| C | ✅ Complete | Hosted and bare-metal |
| WASM | ✅ Complete | Standalone and with JS |
| SystemVerilog | ✅ Complete | With TCL scripts |
| VHDL | ✅ Complete | With PSL assertions |
| COBOL | ✅ Complete | IBM Enterprise COBOL |
| React Native | ✅ Complete | Via target spec |
| Next.js | ✅ Complete | Via target spec |
| Vite | ✅ Complete | Via target spec |
| LLVM IR | ⚠️ Partial | Text IR emitter with acyclic/`noalias`/`!range` optimization \[2026-05-29\] |
| AArch64 | ⚠️ Partial | Binary via acyclic inlining \[2026-05-29\] |
| x86_64 | ⚠️ Partial | Binary via acyclic inlining \[2026-05-29\] |
| Webstack | ⚠️ Migration | WASM-first v2 in progress — LlvmBackend(wasm32) + GlueWebGenerator |
| **Tooling** | | |
| Language Server (LSP) | ✅ Complete | Type-checking, go-to-def |
| Syntax highlighting | ✅ Complete | VS Code extension |
| Formatter | ❌ Planned | |
| Debugger | ❌ Planned | |
| Profiler | ❌ Planned | |
| `brief derive` CLI | ❌ Planned | Synthesizes bodies from `:=` blocks (Phase 9) |
| **Phase 8+ Features** | | |
| `:=` derivation blocks | ✅ Complete | Lexer, parser, AST committed (8.0-8.2) |
| `when` keyword guards | ❌ Planned | Parser addition, same `Statement::Guarded` as brackets (Phase 8.6) |
| Guard same-line enforcement | ❌ Planned | Brace-less guards on single line only (Phase 8.6) |
| `[#]` entry precondition | ❌ Planned | CLI dispatch, call graph isolation (Phase 16B) |
| `export` keyword | ❌ Planned | C/foreign linking (Phase 15) |
| `.f` layout parsing | ❌ Planned | Indentation-based syntax (Phase 16C) |
| `.c` cell wrapper + input/output | ❌ Planned | Cell-wrapped files (Phase 16D) |
| Top-level scripting | ❌ Planned | Implicit `[#]` entry (Phase 16E) |
| `alloc` metadata | ❌ Planned | Stack/MMIO/arena/placement (Planned) |
| `observable`/`volatile` metadata | ❌ Planned | Liveness/dispatch (Phase 8G) |
| Metadata dispatch architecture | ❌ Planned | Backend dispatch on `llvm_*` keys (Phase 8G) |
| Extension modifiers (`.s.bv`, etc.) | ❌ Planned | Aggregated flags in filename (Phase 16A) |
| Pure Bits refactor (`Value::Bits`) | 🔄 In progress | Phase 8A-8F |

**Legend:**
- ✅ Complete - Fully implemented and tested
- ⚠️ Partial - Implemented but incomplete
- ❌ Planned - Not yet implemented

---

## 11. Error Messages

The compiler produces detailed error messages:

### 11.1 Type Errors

```
error[E001]: type mismatch
  --> src/main.bv:10:5
   |
10 |     let x: Int = "hello";
   |         ^      ^^^^^^^^ expected Int, found String
   |         |
   |         expected due to this
   |
   = note: expected type `Int`
              found type `String`
```

### 11.2 Contract Violations

```
error[P001]: postcondition not satisfied
  --> src/main.bv:15:1
   |
15 | txn increment(amount: Int) [amount > 0][counter == @counter + amount] {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   | |
   | this path does not satisfy the postcondition
   |
   = note: postcondition `counter == @counter + amount` may be false
   = help: ensure all paths update `counter`
   = help: counterexample: amount=5, @counter=10, counter=10
```

### 11.3 Mutual Exclusion Violations

```
error[P002]: ownership conflict in reactive cascade
  --> src/main.bv:20:1
   |
20 | async node reader() [!writing][reading == true] { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   | conflicts with:
25 | async node writer() [!reading][writing == true] { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: both transactions can fire simultaneously
   = note: `reader` reads `reading`, `writer` writes `reading`
   = help: add guard to prevent simultaneous execution
```

### 11.4 FFI Errors

```
error[F001]: unhandled FFI result
  --> src/main.bv:30:5
   |
30 |     let result = sqrt(-1.0);
   |         ^^^^^^ Result<Float, MathError> must be handled
   |
   = note: FFI calls return Result and must be handled
   = help: use guard: [result.is_ok()] { ... }
   = help: or use: result.unwrap_or(0.0)
```

### 11.5 Parse Errors

```
error[S001]: unexpected token
  --> src/main.bv:5:15
   |
5  | txn foo(x: Int {
   |               ^ expected `)` or `,`, found `{`
   |
   = note: while parsing transaction parameters
   = help: parameters must be enclosed in parentheses
```

---

## 12. Migration Guide

### From v0.9 to v0.11

**State declaration changes:**
```brief
// Old (v0.9)
state counter: Int = 0;

// New (v0.11)
let counter: Int = 0;
```

**FFI syntax changes:**
```brief
// Old (v0.9)
frgn sqrt(x: Float) -> Float from "math.toml";

// New (v0.11) - must handle errors
frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// Or fire-and-forget
frgn! log(msg: String);
```

**Contract watchdog syntax:**
```brief
// Old (v0.9)
txn foo [true][true] watchdog [100ms] { ... }

// New (v0.11)
txn foo [true][true] ?[100ms] { ... }  // Optional timeout
txn foo [true][true] ![100ms] { ... }  // Required timeout
```

### From v1 (Legacy)

Legacy code continues to work with auto-upgrades:

```brief
// v1 style - auto-upgraded by compiler
frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// v0.11 explicit forms (recommended)
frgn  sqrt(x: Float) -> Result<Float, MathError> from "math.toml";
frgn! write_to_hw(address, value);
```

**Auto-generated defaults:**
- `pre [true]` if no precondition
- `post [true]` if no postcondition
- Layout auto-calculation for structs

---

## 13. Examples

### 13.1 Counter Application

```brief
// counter.rbv — using render struct syntax
let count: Int = 0;

txn increment [count < 100][@count + 1 == count] {
    count = count + 1;
    term;
};

txn decrement [count > 0][@count - 1 == count] {
    count = count - 1;
    term;
};

render struct Counter {
    <div class="counter">
        <h1 b-text="count"></h1>
        <button b-trigger:click="increment">+</button>
        <button b-trigger:click="decrement">-</button>
    </div>
};
```

### 13.2 Bank Transfer

```brief
let alice_balance: Int = 1000;
let bob_balance: Int = 500;
let transfer_in_progress: Bool = false;

txn transfer_alice_to_bob(amount: Int)
    [transfer_in_progress == false && alice_balance >= amount && amount > 0]
    [alice_balance == @alice_balance - amount && bob_balance == @bob_balance + amount]
{
    &transfer_in_progress = true;
    &alice_balance = alice_balance - amount;
    &bob_balance = bob_balance + amount;
    &transfer_in_progress = false;
    term;
};
```

### 13.3 FFI Usage

```brief
import "std/math";
import "std/io";

frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

defn calculate_hypotenuse(a: Float, b: Float) -> Float {
    let a_sq = a * a;
    let b_sq = b * b;
    let sum = a_sq + b_sq;
    
    let result = math.sqrt(sum);
    [result.is_ok()] {
        term result.value;
    };
    [result.is_err()] {
        term 0.0;
    };
    term 0.0;
};

txn main() [true][true] {
    let hypot = calculate_hypotenuse(3.0, 4.0);
    io.println("Hypotenuse: " + String(hypot));
    term;
};
```

---

*Last updated: Brief v0.18.0 (2026-07-11)*
