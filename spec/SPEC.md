# Briv Language Specification

**Version:** Draft 2026-08-05
**Status:** Normative target specification; implementation conformance is staged
**Authority:** This document is the language contract. Architecture documents explain implementation. Tutorials teach only this syntax.

## 1. Scope and conformance

Briv is a contract-first language for layout-independent values, reactive state machines, target-specialized execution, and adaptable foreign boundaries.

A conforming implementation must:

1. Preserve the semantic behavior defined here across every supported target.
2. Implement a normative construct identically to the reference interpreter or reject it with a compile-time target-capability error.
3. Never substitute placeholder values, silently weaken contracts, or select behavior from user-visible type names.
4. Resolve semantic operations in the frontend and leave only equivalent physical realization to backends.
5. Treat every normative example as a conformance fixture.

A section marked **Staged** is normative design that may not yet be implemented. Until implemented, the compiler must reject it explicitly.

## 2. Core model

### 2.1 Types have no canonical layout

A Briv type denotes semantic behavior, logical fields, invariants, metadata, and operation bindings. It does not own one physical representation.

Physical representation is selected from:

- operations performed on values;
- target constraints;
- declared metadata;
- protocol membership and cast paths;
- ownership and effect requirements;
- observed access, slicing, and traversal patterns;
- explicit boundary descriptors.

The frontend decides meaning. LLVM, SPIR-V, CIRCT, Webstack, and future backends may choose different equivalent layouts and instruction strategies.

Every runtime value selected for a concrete target must be materializable by that target. Failure to resolve a representation is a compile-time error; there is no generic integer fallback.

### 2.2 Semantic values and materialization

The reference interpreter operates on generic semantic values:

- semantic type identity;
- optimized primitive atoms;
- bits;
- products;
- sums;
- references;
- closures;
- void.

Stdlib concepts such as lists, maps, options, and results are not hardcoded interpreter value variants. Materialization lowers semantic values to target representations.

### 2.3 Frontend/backend boundary

The frontend resolves each syntactic operation to semantic behavior, contracts, effects, and access shape. Backends may choose any equivalent physical realization but must not redispatch by source type name or redefine operation meaning.

Examples:

- LLVM may scalarize, vectorize, reorder, or choose strided storage.
- CIRCT may realize the same operation as registers, memories, or parallel paths.
- SPIR-V may select target-appropriate storage classes and vector operations.

## 3. Source files and target profiles

### 3.1 Base variants

| Extension | Role | Canonical target path |
|---|---|---|
| `.bv` | General Briv | LLVM/native; optional configured offload |
| `.ebv` | Embedded Briv | LLVM embedded target profile |
| `.abv` | Accelerator Briv | Direct SPIR-V through `rspirv` |
| `.cbv` | Circuit Briv | CIRCT |
| `.rbv` | Rendered Briv | Webstack |
| `.dbv` | Structured Data Briv | Data parser |
| `.dbvl` | Line-oriented Data Briv | Streaming data parser |

`.dbvs`, `.sbv`, `.srbv`, `.sebv`, `.c.bv`, and other compact or legacy variants are not part of the language.

### 3.2 Dotted profiles

Profiles precede the base extension as separate dotted segments.

```text
main.s.bv
ui.s.rbv
kernel.f.ebv
```

#### `.s` — strict verification

`.s` changes acceptance criteria, not runtime semantics or grammar. Strict verification rejects unresolved proof obligations, representation fallbacks, unclassified concurrency, and unresolved lifetime warnings.

Explicitly trusted FFI or protocol axioms are permitted only when:

- trust is visible;
- boundary contracts are complete;
- ownership and effects are declared;
- all mechanically checkable obligations are proven;
- the trust boundary appears in the verification report.

#### `.f` — formatted source

`.f` is a strict indentation dialect of the same language.

- Indentation delimits declaration and statement blocks.
- Statement-block braces are forbidden.
- Semicolon terminators are forbidden.
- Literal delimiters retain their normal meanings.
- The layout frontend is token-aware and preserves original source spans.
- `.f` produces the same AST as canonical brace syntax.

### 3.3 Target capability profiles

Target restrictions are declared in configuration and validated once in the frontend. A backend does not independently invent a source-language subset.

Target-specific sibling modules may coexist. Extensionless imports select the variant configured for the active target. Every sibling variant must satisfy the same exported interface and trait contracts.

## 4. Lexical conventions

### 4.1 Keywords and identifiers

Keywords are lowercase and case-sensitive. Wrong spelling or casing of compiler-known vocabulary is an error with a suggested correction.

Compiler-known hashwords, intrinsics, and syntactic operation identities also require exact spelling.

User-declared casing is advisory:

- `PascalCase`: types, traits, structs, enums, objs, cells, protocol variants, and operation identities;
- `snake_case`: functions, fields, nodes, variables, and macros;
- `PascalCase#`: intrinsics.

Violations for user-defined names produce informational diagnostics or warnings, not errors.

The names `sed`, `pvt`, and `reg` remain reserved for future language contracts.

### 4.2 Comments

```briv
// line comment
/* block comment */
/// declaration documentation
//! module documentation
```

### 4.3 Sigils

| Form | Meaning |
|---|---|
| `#Category` | compiler-known semantic hashword |
| `Intrinsic#` | compiler intrinsic |
| `$name` | compile-time-only declaration/binding |
| `name!(...)` | explicit compile-time expansion |
| `$(Stage)` | staged compile-time execution |
| `value.^Field` | runtime field reflection |
| `value.^^Field` | compile-time descriptor reflection |

`#` may occur only in recognized prefix hashwords or as the terminal intrinsic suffix. Embedded forms such as `foo#bar` are invalid.

Prefix `!value` is Boolean negation. Postfix callable `!` is compile-time expansion. Runtime keyword-bang forms do not exist.

### 4.4 Removed lexical forms

The following are not Briv syntax:

- `rstruct`, `uni`, `like`, `is`, `prop`, `sig`, `state`, `meld`, `syscall`;
- `term!`, `trg!`, `cell!`, `sync!`, `frgn!`, `frgn?`, `frgn?!`, `syscall!`;
- `Ptr!`, `<:`, `:>`, `|>`, `++`;
- legacy pragma/attribute forms `#[`, `#![`, `#pragma`, `#!pragma`, `#?`, `#!`;
- `Ok`, `Err`, `Some`, and `None` as reserved tokens;
- adjacent tagged literals such as `sql"..."`;
- `@`-quoted raw literals;
- import aliases using `as`;
- explicit `[true][true]` contracts.

## 5. Delimiters and arrows

### 5.1 Delimiter load

- `<>`: compile-time specialization, protocol variants, target specialization, and synchronization groups.
- `()`: application, binding, parameters, and construction.
- `[]`: containment, bounds, indexing, contracts, guards, and slicing.
- `{}`: grouping and definitions.

Generic application uses `<...>` only. Square-bracket generic aliases are invalid.

### 5.2 Arrow load

- `->`: result/output flow and directional declarations.
- `<-`: transfer/insertion/extraction behavior.
- `~<-`: destructive transfer/extraction.
- `=>`: match-arm and associative-literal mapping.

`<:` and `:>` do not exist.

## 6. Grammar overview

The grammar below is normative at the structural level. Expression precedence is defined in §15.

```ebnf
program          ::= item*

item             ::= import_decl
                   | export_import_decl
                   | export_decl
                   | const_decl
                   | let_decl
                   | type_decl
                   | trait_decl
                   | proto_decl
                   | struct_decl
                   | enum_decl
                   | impl_decl
                   | obj_decl
                   | cell_decl
                   | defn_decl
                   | txn_decl
                   | node_decl
                   | frgn_decl
                   | asm_decl
                   | trigger_decl
                   | render_decl
                   | compile_time_item

identifier       ::= user_identifier
path             ::= string_literal | "<" path_component ("/" path_component)* ">"
```

## 7. Modules and imports

### 7.1 Path model

```briv
import "./local/path.bv";
import <std/collections>;
```

- Quoted paths are exact or project-relative.
- Angle-bracket paths resolve through ordered roots declared by compiler/target configuration.
- Package registries are configured roots, not a separate source-language import category.
- Resolution is deterministic and records the resolved path.

### 7.2 Aliases and selective imports

`as` is reserved for semantic conversion. Import aliases use local-to-source `:` binding.

```briv
import collections: <std/collections>;
import { LocalName: ExportedName, OtherName } from "./module.bv";
```

Conflicting unqualified imports are errors and must be resolved with a module alias or selective rename. Import order never changes meaning.

Glob imports are invalid.

### 7.3 Visibility and re-export

Ordinary imports are private to the importing module.

```briv
export import { PublicName } from "./internal.bv";
```

`export import` is the only re-export form.

### 7.4 Import graph

Diamond dependencies are valid. Genuine import cycles are compile-time errors. Shared declarations must move to an acyclic interface module.

## 8. Declarations

### 8.1 `let` and `const`

```briv
let counter: Int = 0;
const MaxRetries: Int = 3;
```

Top-level `let` declares reactive program state. Local `let` declares a local binding. There is no `state` keyword.

`const` declares an immutable top-level value. `$const` is a separate compile-time-only binding erased before runtime.

Protocol-supplied defaults may initialize omitted logical fields. Unknown fields, duplicate fields, unresolved defaults, and mismatched constructor arity are errors. There is no generic zero-fill fallback.

### 8.2 `struct`

A `struct` declares a named data relationship. It does not own methods, operations, identity, or lifecycle.

```briv
struct Point<T> {
    x: T;
    y: T;
};
```

A plain struct is layout-adaptive: a backend may reorder, split, scalarize, or eliminate fields when semantics permit.

```briv
type Length32: #Int {
    !> bits: 32;
};

seq struct Header {
    tag: Byte;
    length: Length32;
};
```

`seq struct` preserves field order and containment. Target protocol/ABI configuration still determines widths, alignment, and padding unless more explicit boundary constraints apply.

Behavior for a struct is attached through an inherent `impl` in the defining module.

### 8.3 `enum`

An enum is a closed nominal sum. Its physical tag and payload layout are derived.

```briv
enum Result<T, E> {
    Ok(T),
    Err(E),
};
```

Enum behavior is attached through `impl`. Variant names are ordinary identifiers, not compiler keywords.

### 8.4 Structural sums

```briv
Int | String
```

`A | B` is an anonymous structural sum type. It is matched with typed bindings:

```briv
match value {
    number: Int => use_int(number),
    text: String => use_string(text),
};
```

### 8.5 `type`

A type declares semantic identity, logical fields, invariants, metadata/layout hints, functions, and operation bindings.

```briv
type UserId: Int, Comparable<UserId>, #Int {
    value: Int;
    [value >= 0]

    defn display(self: UserId) -> String {
        term self.value as String;
    };
};
```

The relationship list after `:` may contain:

- at most one parent type;
- explicit trait assertions;
- explicit protocol membership.

#### Refinement parent

`type Child: Parent` is refinement-only inheritance.

- There is one parent maximum.
- The child strengthens semantic invariants.
- No layout, object state, lifecycle, or node graph is inherited.
- Child-to-parent conversion is safe erasure.
- Parent-to-child conversion requires proof or checked conversion.
- Overrides may not strengthen preconditions or weaken postconditions.

### 8.6 `trait`

A trait declares reusable behavioral requirements and defaults. It has no target-specific storage meaning.

```briv
trait Sized {
    Size: Int;
};

trait Comparable<T> {
    defn compare(left: Self, right: T) -> Int;

    op Equal(T): equal(#L, #R);
};
```

Traits may require:

- logical fields;
- function signatures;
- operation signatures;
- contracts;
- effects.

Traits may provide default functions and default operation bindings. Defaults do not establish conformance by themselves.

Conformance is inferred structurally when concrete behavior proves refinement of the trait's signatures, preconditions, postconditions, invariants, and effects. Explicit assertion documents intent and requests direct diagnostics.

Conflicting defaults require an explicit local resolution.

Runtime trait dispatch is explicit:

```briv
let value: dyn Printable = source;
```

Static trait-constrained generics are monomorphized by default.

Trait node templates never activate through inferred conformance. A type or object must import them explicitly.

### 8.7 `proto`

A protocol is a compiler-visible semantic category and cast-coherence domain. Protocols do not prescribe one layout.

```briv
proto #String<UTF8> {
    !> encoding: UTF8;
};
```

Protocol variants exist only when they change semantic interpretation or validity. Width, alignment, storage class, and target layout belong to frozen descriptors rather than protocol variants.

Every route within one protocol coherence domain must be proven functionally equivalent. The compiler may choose among equivalent routes by target cost.

Each written `as` may traverse:

1. any proven-equivalent path within the source protocol;
2. at most one explicitly declared cross-protocol edge;
3. any proven-equivalent path within the destination protocol.

Crossing multiple semantic protocol categories requires multiple written casts.

```briv
let bits = text as #Bit;
let number = text as #String as #Int;
```

Missing proof evidence is an error unless the edge is visibly declared as a trusted foreign/intrinsic axiom.

### 8.8 `impl`

`impl` attaches inherent behavior to data-only nominal declarations and imported foreign shapes.

```briv
impl Point<Float> {
    op Add(Point<Float>): add_points(#L, #R);
};
```

Inherent implementations may appear only in the target declaration's module. Explicit trait implementations obey ownership coherence: either the trait or target must be locally owned.

`type`, `obj`, and `cell` keep their cohesive behavior in their own declarations; `impl` does not arbitrarily split those definitions.

### 8.9 Metadata

`!>` is the canonical metadata-binding operator.

```briv
type Int32: #Int {
    !> bits: 32;
};
```

`!> observable: true` is not valid; use the `out` modifier.

**Staged.** At module top level, `!>` binds metadata to the module as a whole.
Top-level `!>` is a shortcut for attaching metadata to the script; it never
attaches to the following declaration.

```briv
!> accel: try_all;
```

Metadata keys and values are lowercase. Multiple top-level `!>` bindings merge
into one module metadata map (last binding wins per key). Values use the same
grammar as declaration metadata (identifier, integer, boolean, string, or
list). Module metadata is available to any backend or plugin that consults
the metadata vocabulary.

## 9. Functions, transactions, nodes, objects, and cells

### 9.1 Functions

```briv
defn add(left: Int, right: Int) -> Int [true][#R == left + right] {
    term left + right;
};
```

`term expression;` completes the current callable or convergence step. Briv has no `return` keyword.

A body-less internal signature uses `defn` with its contracts/effects and is staged until an implementation is supplied. There is no `sig` declaration.

### 9.2 Callable types and closures

Callable types use signature shape directly:

```briv
let transform: (Int) -> Int = value => value + 1;
```

Named functions and transactions may be used as values when their effect and ownership requirements fit the expected callable type.

Closures capture lexical bindings. Captures participate in ownership, lifetime, and effect checking.

### 9.3 Transactions

```briv
txn increment()[counter < Max][counter >= 0] {
    counter = counter + 1;
    term;
};
```

Prior-state `@value` references (for postconditions such as
`counter == @counter + 1`) are **staged**: the `@` raw-literal prefix is
removed, and prior-state expression syntax is not yet implemented. Until then,
transactions must express their postconditions without prior-state reads.

A transaction is atomic with respect to its declared state transition. `rollback;` or `rollback reason;` aborts and reverts the current transaction/reactive firing.

```briv
when invalid_input {
    rollback InvalidInput;
};
```

`rollback` is invalid outside rollback-capable transaction/reactive contexts.

### 9.4 Nodes

A node is a reactive transition that may fire when its precondition is satisfied.

```briv
node update [pending][!pending] {
    pending = false;
    term;
};
```

The keyword is `node`; `rct` is not source syntax.

### 9.5 Objects

An object owns identity, lifecycle, logical state, ports, and reactive behavior in its parent reactor.

```briv
obj Enemy(damage: Event<Damage>) -> died: Event<EnemyId> {
    health: Int;

    node apply_damage()[damage.Ready][health >= 0] {
        health = health - damage.amount;
        term;
    };
};
```

Objects use traits and composition rather than parent inheritance.

### 9.6 Cells

A cell is a sealed state machine with an independent convergence membrane.

```briv
cell Timer(period: Duration) -> tick: Event {
    // owned state and internal nodes
};
```

- Communication occurs only through declared ports.
- Internal state is not externally visible.
- Cells and objects share input `(...)` and named output `->` syntax.
- Multiple outputs form a complete named product on every target.

### 9.7 Acceleration (`accel`)

**Staged.** `accel` is a keyword that may prefix a `node` or `txn` declaration.
It marks a native counted loop as a *parallel map over work-items*: the work
is expressed as an ordinary loop over a real counter, and the compiler may
coalesce the loop into one GPU dispatch of N work-items.

**Design A — the counter is a real state field, never virtual.** The program
declares and initializes the work-item counter explicitly (`let i: Int = 0;`),
bounds it with a counted-loop contract, and advances it in the body. No
compiler-synthesized variables: everything the loop does is visible in source.

```briv
let i: Int = 0;                       // work-item counter, explicit init
accel node force [i < nbodies][i == nbodies] {
    dv[i] = force_on(i);              // per-work-item compute
    i = i + 1;                        // native counted-loop advance
    term;
};
```

- The precondition `[i < N]` is the loop bound and the firing gate; the
  postcondition `[i == N]` is the goal ("loop until true"). Both reference the
  real counter — valid runtime checks, never an undefined variable.
- The compiler **proves** the counted loop is a parallel map: `i` is the
  counter (incremented in the body), every write targets a slot affine in `i`
  (disjoint across work-items), shared reads are permitted, and value types are
  flat. An unproven body is silently kept on the CPU path with a remark.

**Dispatch.** On the GPU path, one dispatch of N work-items replaces the
N-firing loop: the runtime launches the kernel (the work-item id is the
counter value) and fast-forwards the counter to N so the loop's bound is met
after a single firing. On the CPU path, the loop runs natively — each firing
is one work-item. Cross-work-item data exchange is permitted only through
host-sequenced separate `accel` declarations, never within a single firing.

**Verification requirement.** In try modes, GPU deferral happens only when
the compiler verifies a speedup. The compiler must prove eligibility (bound,
write disjointness, flat value types, purity) and then either

- prove statically, for a compile-time-known N, that N exceeds the device
  crossover, or
- emit a runtime auto-tuning probe that measures both the CPU and the GPU path
  at program start, checks output equality within tolerance, and commits to
  the faster path.

If eligibility cannot be proven or the speedup is not verified, execution
silently uses the CPU path. An ineligible or unverified `accel` body is never
an error by itself.

**Force mode.** Under `!> accel: force;` or `!> accel: try_all_force;`, an
`accel`-keyword-marked body must offload: eligibility must be provable or the
compiler rejects the program, the speedup gate is skipped (the developer
asserts GPU wins), and a missing GPU at runtime is a runtime error — never a
silent CPU fallback.

**Module shortcut.** Top-level module metadata `!> accel:` (§8.9) takes
lowercase policy values:

- `try_all` — every eligible body in the module is a candidate, verified;
- `force` — `accel`-keyword bodies must offload (see Force mode);
- `try_all_force` — every body is tried and keyword bodies are forced.

Absent means only bodies carrying the `accel` keyword are candidates, in try
mode. There is no `off` value; absence is the default. `!> accel_report:
verbose;` is a separate observability key that emits an optimization remark
for every analyzed body. The keyword and the module shortcut feed the same
verification pipeline.

## 10. Contracts, invariants, and watchdogs

### 10.1 Contracts

Callable and transition contracts use precondition/postcondition brackets:

```briv
defn divide(a: Int, b: Int) -> Int [b != 0][#R * b == a] {
    term a / b;
};
```

Omitted clauses retain implicit provenance. The compiler must distinguish omission from an explicitly written tautology.

Explicit `[true][true]` is invalid everywhere: it asserts nothing (`true ⇒ true` is trivial), so it is indistinguishable from an omitted contract and records no obligation.

Contracts are **mandatory** (present and non-trivial) on `node`, `txn`, and `asm` declarations: the reactor uses the pre/post pair to prove and classify the transition. `defn` contracts are optional; `cell` declarations do not require a contract.

Type invariants must be proven across construction and every mutating transformation.

### 10.2 Inline guards and gates

```briv
[ready] process();
[converged];
```

- `[condition] statement;` guards one statement.
- `[condition];` is a convergence gate.
- `[condition] { ... }` is invalid; use `when` for blocks.

### 10.3 Watchdogs

Watchdogs occupy a dedicated grammar slot after a contract.

```briv
txn poll()[ready][done] ?[progress] within 10ms -> on_timeout() {
    // body
};

txn critical()[ready][done] ![progress] within 100cyc {
    // body
};
```

- `?[condition]`: optional watchdog enforcement.
- `![condition]`: required watchdog enforcement.
- Canonical duration units are `cyc`, `ns`, `ms`, `s`, and `min`.

The watchdog `?`/`!` forms are contextual and do not conflict with expression propagation or compile-time expansion.

## 11. Control flow

### 11.1 No `if`/`else`

Briv has no `if` or `else`. Conditional branching uses exhaustive `match`. One-sided guarded execution uses `when` or inline guards.

### 11.2 `when`

```briv
when ready {
    process();
};
```

A `when` block has no implicit complementary branch.

### 11.3 `match`

```briv
match value {
    Result::Ok(result) => use(result),
    Result::Err(error) when error.retryable => retry(error),
};
```

- Arms use `=>`.
- Guards use `when`, never `if`.
- Closed sums and enums require exhaustive coverage.
- Open or unknown alternatives require `_ =>`.
- Unreachable arms are errors.
- All expression arms must have compatible result types.

### 11.4 Iteration

`foreach` is the sole iteration keyword.

```briv
foreach(item in items) {
    consume(item);
};
```

There are no `for`, `while`, or `loop` keywords. Counted iteration uses iterable ranges or reactive/transactional structure.

### 11.5 Local and process completion

- `term expression;`: complete the current callable/transition.
- `endprogram;`: complete the process boundary normally.
- `endprogram code;`: complete the process with an exit code.
- `defer { ... };`: register cleanup for the enclosing scope.

`endprogram` (formerly `exit program`, and before that the removed `term!`)
runs applicable `defer` cleanup and terminates the process. Unlike `term`,
which only ends the current transaction, `endprogram` exits the process even
when a node's precondition remains satisfiable. Abrupt termination is not
currently a source-language feature.

There is no `main` declaration in Briv. The program entry is either an
explicit `beginprogram` node (§11.5.1) or, absent one, whichever reactive
node fires first: the reactor evaluates node preconditions and the first
satisfiable one fires. A program converges (and exits) when no node can fire.

#### 11.5.1 Entry loops (`beginprogram`)

**Staged.** `beginprogram` is a keyword usable as a conjunct in a node's
precondition: `[beginprogram && <state>][<goal>]`. It is a pure marker — true
exactly once at program start — and takes no conditions itself. The node's
other precondition terms are ordinary state expressions over top-level
bindings seeded from the environment or compile time at startup:

```briv
let startingnumber: Int = get_env_int!("env_var");

node entry1 [beginprogram && startingnumber == 1][done] {
    done = 1;
    term;
};
```

A `beginprogram` node is an **entry loop**:

- It is entered exactly once at program start when its state conditions hold.
- The precondition is evaluated once at entry and **never re-checked** during
  the loop.
- The node itself is a loop: the body runs repeatedly until the postcondition
  (goal) is met.
- The goal must be **provably reachable**: a counter comparison whose body
  advances the counter toward the bound, or `[true]` (a single pass). A goal
  that cannot be proven reachable is a compile error.
- At most one `beginprogram` node may be eligible at program start: the
  compiler proves the entry conditions are mutually exclusive. Unprovable
  overlap is a compile error.

`beginprogram` is scoped to `node` declarations.

### 11.6 Critical sections and barriers

```briv
mutex {
    update_shared_state();
};

barrier<workers>;
```

- `mutex { ... }` is a critical section.
- `barrier<group>` is an explicit runtime synchronization point.
- `sync<group>` is reserved for node classification.

## 12. Concurrency and task lifecycle

### 12.1 No implicit concurrency

If two reactive nodes may fire simultaneously and have no XOR read/write dependency, both must be classified:

- `async` on both, acknowledging simultaneous firing; or
- `sync<group>` on both, establishing a group barrier.

An unclassified eligible pair is a compile-time error.

### 12.2 `spawn` and `await`

```briv
let task = spawn compute(input);
let result = await task;
```

`async` is not a statement-level call modifier. Legacy `async call` and `async await` forms do not exist.

`spawn` creates a persistent task or component instance and returns a linear owned handle.

- `await task` consumes a task handle and returns the callable's declared result.
- `free task` requests cancellation/stop and runs `defer` cleanup.
- `keep task` transfers the handle to the enclosing owner/boundary.
- Silently dropping or discarding a live handle is an error.

`free task` is valid only when effect analysis proves cooperative cancellation points and cancellation-safe active FFI. Otherwise the handle must be awaited or kept.

### 12.3 Reference scheduling

The reference interpreter uses a deterministic semantic scheduler for normal execution. Verification mode explores all legal interleavings. Host-thread nondeterminism does not define language meaning.

## 13. Triggers and external events

The reactive input keyword is `trg`.

```briv
trg input_ready @ device;
```

`@` binds a trigger to its source. Trigger source forms are target/profile validated. Typed event ports on `obj`/`cell` declarations are the staged replacement for a typed trigger surface.

`trg!` does not exist. Local asynchronous suspension uses ports, nodes, spawned tasks, and `await`.

Event fairness assumptions belong to explicit event-port contracts. There is no global `#assume_event` pragma.

## 14. Ownership, lifetimes, and effects

### 14.1 Universal ownership algebra

Boundary and callable ownership uses:

- `borrow`: caller retains ownership; callee cannot retain beyond the call;
- `consume`: ownership transfers to the callee;
- `owned`: caller receives ownership;
- `borrowed<source>`: returned lifetime is bounded by a named input;
- `shared`: ownership uses a declared retain/release policy.

```briv
frgn parse(
    borrow input: Ptr<Byte>,
    consume arena: Arena
) -> owned Node from #System;

frgn view(borrow source: Buffer) -> borrowed<source> Slice from #System;
```

Allocation and destruction policy is configured rather than hardcoded into the ownership keyword. Read/write permission belongs to effects.

### 14.2 Pointer safety

- Pointer types are `Ptr` and `Ptr<T>`.
- `&value` requests addressability and returns a pointer.
- `*pointer` dereferences.
- Dangling pointers are hard errors in every profile.
- Mutable access requires proven exclusive provenance.
- Intentional shared mutation requires atomic/synchronization behavior or a cell boundary.

There is no `Ptr!` alias and no `.^Address` acquisition form.

### 14.3 `free` and `keep`

```briv
free value;
keep value;
```

- Proven last use may be scheduled automatically.
- `free` requests a release point and requires proof of no later or aliased use.
- `keep` transfers the value to boundary/owner lifetime.
- Unresolved lifetime is a warning in normal profiles and a hard error in `.s`.
- A boundary collector may service warned unresolved lifetimes in normal profiles.

### 14.4 Unified effects

The frontend infers one effect set covering at least:

- reads and writes;
- allocation and release;
- spawn and await;
- FFI and I/O;
- blocking;
- purity;
- cancellation behavior.

Traits and contracts may constrain effects. Compile-time reflection exposes `.^^Effects`.

## 15. Expressions and operations

### 15.1 Operation dispatch

Syntax maps to operation identities, which types bind to functions.

```briv
type Number: #Int {
    op Add(Number): add(#L, #R);
};
```

Compiler-known operand hashwords include:

- `#L`: left operand;
- `#R`: right operand/result position according to operation signature;
- `#T`: type parameter;
- `#Self`: semantic self-reference.

The compiler knows operation identities but not stdlib collection type names.

### 15.2 Operator classes

Implementations may bind the following syntax families:

- arithmetic: `+`, `-`, `*`, `/`, `%`;
- comparison: `==`, `!=`, `<`, `<=`, `>`, `>=`;
- Boolean: `&&`, `||`, prefix `!`;
- bitwise: `&`, `|`, `^`, `~`, `<<`, `>>`;
- indexing/slicing: `[]`;
- transfer: `<-`, `~<-`;
- assignment/update forms such as `+=` where supported.

Concatenation has no dedicated `++`; it resolves through an ordinary operation binding.

### 15.3 Transfer arrows

```briv
list <- value;
value <- list[index];
value ~<- list[index];
map[key] <- value;
```

The complete semantic shape of insertion/extraction is carried to the resolved operation binding. The compiler does not hardcode `List`, `Map`, `Entry`, stack, or queue behavior.

### 15.4 Precedence

From highest to lowest:

1. application, indexing, field access, reflection;
2. prefix operators and postfix propagation/expansion;
3. multiplicative operators;
4. additive operators;
5. shifts;
6. comparisons;
7. equality;
8. bitwise operators;
9. Boolean `&&`, then `||`;
10. ranges;
11. transfers and assignment.

## 16. Literals, ranges, and slicing

### 16.1 Numeric literals

Normative numeric forms are:

- decimal integers and floats;
- hexadecimal `0x`;
- binary `0b`;
- octal `0o`;
- canonical duration suffixes.

Physical width is expressed through type annotation or cast, not `i32`, `u8`, or `f64` lexer tokens.

Custom parse-prefix/suffix bindings are not currently exposed. Unknown prefixes and suffixes are errors.

### 16.2 Strings and bytes

```briv
"escaped string"
#r"raw \ text"
#b"\x89PNG\r\n"
```

- `#r`: raw string; escapes are not interpreted.
- `#b`: byte literal; byte escapes are interpreted.
- Formatting/interpolation uses explicit compile-time expansion such as `format!(...)`.
- Lexer-level interpolated strings and adjacent tagged literals do not exist.

### 16.3 Ordered and associative literals

```briv
[1, 2, 3]
["one" => 1, "two" => 2]
```

Both are type-directed. Associative literals lower through expected-type construction/insertion behavior; they do not imply a compiler-known hash map.

There is no universal `null` or `nil`. Absence uses an ordinary sum variant such as `Option::None`.

### 16.4 Ranges

- `start..end`: half-open range.
- `start..=end`: inclusive range.
- `...`: multidimensional slice ellipsis only.

### 16.5 Python-style slicing

```briv
tensor[start:stop:step, ..., time => 5, width => 0:10]
array[mask]
array[start:stop][mask]
```

- Slice coordinates use `start:stop:step`.
- Named dimensions use `name => selector`.
- Boolean masks use ordinary mask indexing.
- The legacy `range; condition` slice form is invalid.

### 16.6 Fixed containment and const dimensions

```briv
Int[8]
Matrix<T, Rows, Cols>
```

`T[N]` expresses fixed containment. Const generics and dependent bounds permit dimensions to be compile-time parameters. Bounds are proven during specialization.

## 17. Reflection

### 17.1 Runtime reflection

```briv
value.^Field
```

Runtime reflection reads a declared/materialized logical field. Missing fields are compile-time errors for the selected program/target.

### 17.2 Compile-time descriptor reflection

```briv
value.^^Type
value.^^Ops
value.^^Bytes
value.^^Alignment
value.^^Effects
```

Compile-time reflection occurs after semantic/layout freezing. Reflection-driven specialization may inspect the frozen descriptor but may not introduce new layout requirements that invalidate the freeze.

Descriptor fields include, where applicable:

- `Type`, `Ops`, `Effects`;
- `Bytes`, `Alignment`, `Endian`, `StorageClass`, `AddressSpace`, `Addressable`;
- declaration metadata `Name`, `Params`, `Returns`, `Arity`, `Loc`, `FnSpan`, `Doc`, `Hash`, `Contracts`, `Module`, `IsPure`.

Declaration/source metadata is compile-time-only unless explicitly materialized.

`Alignment` and `Endian` describe a selected materialization, not a universal property of an abstract type.

### 17.3 Transformations are not projections

`Absolute`, `BitReverse`, `Popcount`, `LeadingZeros`, and `TrailingZeros` are explicit intrinsics rather than universal projection names.

`Values` and `Elements` are ordinary logical fields when declared. `AsStack` and `AsQueue` are type-defined conversions.

## 18. Compile-time execution and macros

### 18.1 Compile-time-only bindings

```briv
$const Limit = 32;
$let current = 0;
$defn build(...) { ... };
```

`$` declarations exist only during compilation and are erased before runtime.

### 18.2 Expansion

```briv
format!("value: {}", value)
regex!(#r"[a-z]+")
```

`name!(...)` performs explicit compile-time expansion. Its arguments may follow macro-specific, noncanonical syntax because the compile-time expansion defines their parse contract.

Privileged macros declare capabilities at definition. Calls still use `name!(...)`; `$!name` does not exist.

### 18.3 Stages

```briv
$(Parsed) { ... }
$(Allocated) { ... }
```

Stage blocks state when compile-time work executes. Stage vocabulary is compiler-known and exact.

### 18.4 Quotation

Quotation and interpolation operate on AST values during compile time. They must preserve hygiene unless an explicit compiler capability requests generated names.

### 18.5 Derivation

`:=` introduces compile-time derivation/synthesis examples or a reference implementation.

```briv
defn parity(x: Int) -> Bool
    := { 0 => false; 1 => true; }
    := parity_reference;
```

Generated behavior must satisfy the declared contracts and reference obligations. Derivation never weakens a contract.

## 19. Foreign functions, export, and GLUE

### 19.1 Foreign declaration

```briv
frgn local_name(
    borrow input: Ptr<Byte>,
    consume arena: Arena
) -> owned Node: external_symbol from #System;
```

The declaration name is the local Briv name. `:` binds a different external symbol. `as` is not an alias operator.

A `frgn` signature declares the actual Briv-visible return type. Foreign calls are never implicitly wrapped in `Result`.

GLUE configuration explicitly maps errno, status codes, exceptions, or delivery failures into `Result` when required.

### 19.2 Provenance

Exactly four provenance forms exist:

```briv
from "path"
from <configured/path>
from #Link<name>
from #System
```

- Quoted paths are exact/project-relative.
- Angle paths use ordered configured roots.
- `#Link<name>` declares a linker dependency.
- `#System` uses the selected system/runtime profile.

Platform families such as POSIX are target configuration, not source hashwords.

### 19.3 Optional symbols

```briv
optional frgn feature(...) -> T from #System;

when feature.^^Available {
    use(feature(...));
};
```

There is no `frgn?` or declaration-level `fallback` clause. Fallback behavior uses ordinary typed control flow.

### 19.4 Variadics

```briv
frgn log(format: String, variadic args: ForeignArgs) -> Void from #System;
```

A variadic foreign signature has an explicit final named variadic parameter. GLUE supplies ABI behavior. `...` is reserved for slicing.

### 19.5 Raw system transitions

Named system APIs use `frgn ... from #System`. Raw target-specific kernel transitions use an explicit intrinsic such as `SysCall#(...)`. There is no `syscall` keyword.

### 19.6 Foreign layouts

Exact foreign field order, width, alignment, calling convention, and release policy live in GLUE/Data Briv configuration.

There is no `meld` declaration. Foreign shapes adapt through configured descriptors, declared protocol cast edges, ownership contracts, and effects.

### 19.7 MMIO

`frgn name @ address` is invalid. Memory-mapped I/O uses configured device/cell ports or explicit pointer/address intrinsics.

### 19.8 Export

```briv
export defn add(left: Int, right: Int) -> Int {
    term left + right;
};
```

`export` is the sole export syntax. `#export` does not exist.

## 20. Assembly declarations

```briv
asm<x86_64> add_words(left: Int, right: Int) -> Int
    [true][#R == left + right]
    !> effects: [read, pure]
{
    "add ...";
};
```

`asm<target>` is an ordinary top-level declaration analogous to `defn` with a target-specialized body.

The target capability profile validates instruction syntax. Every assembly declaration supplies contracts and an effect profile including read/write sets, clobbers, blocking, FFI, and purity facts as applicable.

## 21. Rendered Briv

### 21.1 Document structure

An `.rbv` document contains Briv source plus `<view>` and optional `<style>` blocks. Legacy `<script>` wrappers are invalid.

### 21.2 View attachment

```briv
render Counter {
    <button b-trigger:click="increment">
        <span b-text="count"></span>
    </button>
};
```

`render Name { ... }` is the sole attachment form. The compiler resolves whether `Name` is a struct, type, obj, or cell and applies the relevant visibility/lifecycle rules.

### 21.3 Components

Custom component tags create first-class reactive instances. The rendered parent owns each mounted component handle. Mounting creates the handle; unmounting releases state and subscriptions.

### 21.4 Directives

Canonical directives include:

- `b-text`;
- `b-show`;
- `b-when`;
- `b-style`;
- `b-class`;
- `b-each:name`;
- `b-key`;
- `b-bind:value`;
- `b-trigger:event`.

`b-if` is invalid.

`b-when` structurally mounts/unmounts a subtree. `b-show` changes presentation only and preserves identity/state.

Dynamic repetition requires a stable `b-key` whenever children may be inserted, removed, or reordered.

Reactive component nodes and view-event handlers obey the same no-implicit-concurrency rule as every other node: eligible simultaneous pairs require `async` or `sync<group>` classification.

`b-bind:value` accepts only an assignable logical field with a proven write contract. Computed expressions use separate value and trigger handlers.

### 21.5 View expressions

Every directive expression is canonical Briv, not a JavaScript-like mini-language. Ternaries and brace object literals are invalid.

View expressions are pure/read-only. Mutation, FFI, allocation, and spawning occur only in explicit event handlers or compiler-managed component lifecycle.

### 21.6 Web representations

View-bound values are not restricted to compiler-known primitive names. Web GLUE configuration supplies protocol casts and layout descriptors. Unsupported values are rejected by the target capability validator.

## 22. Data Briv

### 22.1 Shared principles

`.dbv` and `.dbvl` share one value/schema core but have distinct document grammars.

Values remain raw until interpreted by an asserted schema. Without a schema, arbitrary raw/scraped data is valid.

Quoted values are always supported.

### 22.2 `.dbv`

`.dbv` is structured/category data. `>` introduces entries under the current category.

```dbv
schema Person from "person.dbv";

> people
name: "Ada";
age: 37;
```

Schema imports use:

```dbv
schema Name from "file.dbv";
```

### 22.3 `.dbvl`

`.dbvl` is line-oriented. Each non-instruction physical line is exactly one record. Records may not span lines or merge across lines.

`>` introduces non-data instructions.

```dbvl
>schema Person from "person.dbv";
Ada,37
Grace,85
```

A schema key field derives the lookup key. Missing or duplicate keys are errors.

`.dbvl` supports lazy/streaming reads and append-only canonical writes.

### 22.4 Schema types

Canonical collection schema forms are:

```text
T[N]
List<T>
Map<K, V>
Option<T>
field?: T
```

`Vec[T]`, `T[]`, `Array<T>`, bare `Map`, and semicolon-separated generic arguments are invalid.

### 22.5 Validation

When a schema is asserted, validation covers:

- required and unknown fields;
- raw-token conversion;
- field types;
- constraints;
- named schemas;
- optional values;
- key presence and uniqueness.

### 22.6 Canonical serialization

Data Briv defines deterministic field/key ordering, quoting, numeric spelling, and instruction placement for reproducible builds and hashing.

`briv check file.dbv` and `briv check file.dbvl` select the correct parser mode and perform schema validation when asserted.

### 22.7 GLUE files

Human-authored per-language GLUE configuration is structured multiline `.dbv`:

```text
lib/glue/<language>/glue.dbv
```

Generated `bridge-exports.dbvl` remains line-oriented machine metadata.

## 23. Diagnostics, tooling, and documentation

### 23.1 Shared language manifest

Lexer vocabulary, LSP vocabulary, highlighter grammar, extensions, profiles, and reserved words derive from one machine-readable manifest.

### 23.2 LSP

The LSP uses the compiler's real Briv/Data Briv parsers and semantic analyses. It does not maintain an independent language grammar.

### 23.3 Formatter

The canonical formatter must satisfy parse-format-parse AST equivalence. SPEC examples and repository rewrites use that formatter.

### 23.4 Repository conformance

CI parses and typechecks every active shipped `.bv`, `.ebv`, `.abv`, `.cbv`, `.rbv`, `.dbv`, and `.dbvl` file under its declared target/profile.

Excluded legacy material belongs under `archive/`.

### 23.5 Documentation hierarchy

1. `spec/SPEC.md` is normative.
2. `docs/architecture/` explains implementation and rationale.
3. `learn-briv/` teaches normative syntax.
4. Timestamped plans are historical records and are not retroactively rewritten.

## 24. Standard-library boundary

The compiler knows bootstrap primitives, semantic operation identities, hashwords, intrinsics, and grammar. Collection organizations, regex APIs, formatting APIs, options/results, platform handles, and host-language types belong to stdlib, plugins, or configuration.

Examples:

- Regex is implemented through `regex!(#r"...")` and plugins/stdlib, not a `/.../` lexer literal.
- Associative literals are type-directed and do not imply `HashMap`.
- Stack/queue conversions are type-defined.
- DOM handles and host ABI categories are GLUE configuration, not Rust type-name matches.

## 25. Implementation staging

This specification supersedes active contradictory syntax documentation. Implementation must proceed through explicit migration phases.

Until a normative feature is implemented, the compiler must:

- reject it with a precise staged-feature diagnostic; or
- continue accepting only already-conforming subsets.

It must not retain removed aliases in the normal parser merely for compatibility. Briv is pre-adoption; active repository source is rewritten directly to canonical syntax.

No compatibility parser or `briv migrate` tool is part of this migration. The canonical parser accepts only this specification.

The implementation plan following this specification defines parser, AST, analysis, interpreter, backend, stdlib, tooling, documentation, and verification order.
