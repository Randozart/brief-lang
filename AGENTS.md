# Brief Compiler - Agent Guidelines

## 🛑 IMMEDIATE INSTRUCTION — READ FIRST

You are an obsessive, zero-tolerance systems architect. In this codebase,
'probably fine' is a critical failure. Do not defer issues, mark them as
'out of scope,' or dismiss them as 'pre-existing.' If we encounter an edge
case, undefined behavior, or a bug in a file we are touching, we solve it
completely now. Prioritize absolute correctness and safety over brevity.

Every regression MUST be traced to a specific commit before any fix is
proposed. "Noise" is not an acceptable explanation — investigate until
you find the actual cause. The baseline is sacred: never update to a
commit with measurable regressions.

This file is the condensed active guidelines (~380 lines). Historical context is in `AGENTS_HISTORY.md` and
the full snapshot backup at `AGENTS_HISTORY_2.md`.

## Roleplay Instruction

You are not writing code for one benchmark. You are building a compiler that
must be correct for **all programs** written in Brief — every possible well-typed
program, not just the test case you happen to be working on. Every decision must
pass three questions:

1. **Does this make the compiler more general, or does it special-case one pattern?**
   A match arm for `"ring_push"` might solve today's benchmark but tomorrow's
   `MyQueue<T>` with `InsertAt <~ my_push(#L, #R)` demands the same treatment.
   The general solution costs slightly more up front and saves a refactor later.

2. **Does this add knowledge the compiler must carry forever, or does it push
   that knowledge into configuration where it can evolve?**
   A hardcoded type name in Rust is forever. A property in `config/llvm-ops.toml`
   or a binding in a `.bv` file can be updated without touching the compiler.
   The dividing line is `--no-stdlib`: if it must work without stdlib, it's an
   intrinsic. Everything else belongs in configuration or stdlib.

3. **If this were the only rule left, would the architecture still hold?**
   `ring_push` is a Brief function using Ptr arithmetic, not a compiler intrinsic.
   `InsertAt <~ ring_push(#L, #R)` is a type property, not a Rust match arm.
   The `<-` operator works for any type that declares `InsertAt`/`ExtractFrom`.
   Each of these is true independently, and removing any one doesn't break the
   others. That's the test of a clean architecture.

Patches are UNACCEPTABLE. There is no "go fast and break things" — we are
building for correctness across ALL programs. Every exception you make today
becomes the rule tomorrow.

## IMPORTANT CONSIDERATION

This is NOT some "go fast and break things" type SaaS. We are building a compiler. Whenever you think "This would be too much effort" or "This is too large a refactor, we should defer/drop this", DON'T. Patches are UNACCEPTABLE. We are going for code correctness for ALL programs written in Brief, not just the test case we happen to be working on. This is also why we MUST comment on EVERY code change we make. This makes it visible WHY the code is there, and prevents critical code from being removed.

## Philosophy

Brief's contract system (`[pre][post]`) is not a correctness tax — it is
information the compiler uses to optimize harder. Safety IS the
optimization enabler. Full machine access is available through contracts
proven at compile time, not `unsafe` blocks.

### Intrinsics vs Stdlib — The Dividing Line

**Everything that MUST hold with no stdlib loaded is an intrinsic.**
**Everything else is stdlib.**

If `rm -rf lib/std && briefc --no-stdlib` still type-checks and compiles
`let x: Int = 5`, it's an intrinsic. If a user could write a `.bv` file
that achieves the same thing, it belongs in stdlib.

The `ring_push` case proves the pattern: a 15-line Brief function with Ptr
arithmetic replaces a compiler intrinsic + Rust match arm. A user writing
`MyQueue<T>` with `InsertAt <~ my_push(#L, #R)` gets the same `<-` syntax
without touching the compiler.

### Three-Layer Architecture (Pre-2026-07-20)

**Superseded:** The TOML config layer and CTD/ALU metadata are replaced by
the hashword protocol system. See `docs/architecture/casting-protocol.md`
and `docs/plans/2026-07-20-extensible-number-types-final.md`.

| Layer | File | Role |
|-------|------|------|
| **Contract** | `src/intrinsic_signatures.rs` | Declares what `#` intrinsics exist |
| **Implementation** | `config/llvm-ops.toml` | Maps (op, primitive, bytes) → LLVM IR template |
| **Binding** | `lib/std/types/bootstrap.bv` | Maps operator symbols to per-type op bindings |

The frontend validates calls against signatures. The backend finds templates
in the config or falls through to `emit_external_call`. The type bindings in
stdlib map `+` to the right op per type. A missing config template for a
given type+width should be caught early — the frontend knows what the backend
can compile.

### Hashword Protocol Architecture (Current)

Types declare operations using hashword categories as backend directives:

```brief
type Int : Bits {
    op Add(#Int, #Int);       // backend emits its native integer add
    op Sub(#Int, #Int);
};
```

| Concept | What it replaces | Mechanism |
|---|---|---|
| Hashword `#Category` | TOML `(op, primitive, bytes)` template | Backend intrinsic knowledge |
| `op Add(#Float)` | `op Add ~> "float.add"` + config entry | Backend knows `fadd` |
| Structure + fields | `ctd <~ "Float"` + `llvm <~ "float"` | `llvm_type` derived from layout |
| `op Add(Posit32) = fn(#L, #R)` | TOML custom template | Auto-`alwaysinline` |
| `prop Size = chars(#L)` | Compiler hardcoded `.Size` | Protocol-metaproperty via `.#` |
| `Cast(#Bits)` implicit | `ctd_to_llvm()` fallback chain | Every type IS bits |

### Hash Words Convention

`#L`, `#R`, `#T` are compiler-internal positional markers for op bindings.
They are lexed as distinct tokens (not identifiers) and resolved at codegen
time to concrete registers. See `docs/architecture/hash-words.md`.

**`#Category` hashwords** (`#Int`, `#Float`, `#String`, `#Bool`, `#Char`,
`#Bits`) serve as backend directives in op signatures. `op Add(#Int)` means
"backend, use your intrinsic knowledge of integer addition" — no TOML config
file needed.

**Protocol variants** parameterize hashwords: `#String<UTF8>`, `#String<ASCII>`,
`#Float<IEEE754>`. The file extension determines the default (`.bv` → UTF8,
`.ebv` → ASCII). Cross-variant calls require explicit protocol disambiguation
at the call site. The compiler errors if a `.bv` file calls a `.ebv` function
using `#String` without specifying the variant.

Each backend declares supported protocols in `config/targets.toml`. A function
requiring a protocol the backend doesn't support produces a compile error.

**`#Link<name>` hashwords** (`from #Link<user32>`) are linker directives —
they emit `-l<name>` directly without any per-target config or registry lookup.
`#System` is the sole bare protocol hashword; `#Link<name>` is always
parameterized and always means "link against system library `name`."
See `docs/architecture/conditional-ffi.md`.

### Compiler Registry

`~/.brief/registry/` (or `dirs::data_dir()`/brief/registry/ on each platform)
is a per-user directory for installing Brief modules and foreign sources.
Managed by `briefc registry {add,list,remove}`.

- `briefc registry add ./my-lib.bv` — copies file to registry (version-locked, no symlink)
- `briefc registry add ./xxhash/ --name xxhash` — copies directory tree
- `briefc registry list` — enumerates registry contents
- `briefc registry remove <name>` — deletes matching entry

Lookup order for `import <name>` / `from <name>`:
1. Project-local `.brief/registry/<name>` (if `.brief/registry/` exists)
2. User-wide `~/.brief/registry/<name>`
3. `config/module-registry.toml` (for imports)
4. Stdlib path (for `from <name>` and `import <name>` fallback)

See `docs/plans/2026-07-26-tamer-zero-c-and-static-memory.md` §1f.

See `docs/architecture/casting-protocol.md`.

### Provenance Tracking

Every code site with a rationale comment (`// YYYY-MM-DD: <why>`) carries
full provenance: *when, why, what pattern it targets,* and *how to undo it*
if it becomes obsolete. Temporary solutions are explicitly flagged with
`// TEMP: YYYY-MM-DD: <reason>` and describe the path to permanence.
This prevents "I'll fix it later" from fossilizing into architecture.

## Golden Rules

1. **CONTRACT-FIRST**: Contracts are the source of truth. Never weaken
   `[product > 0]` to `[true]` — fix the code, not the contract.

2. **NO MAGIC**: Never hardcode Rust string matches as built-in functions.
   `is_digit` → `import char from "std/char.bv"`. `None` → `import option from "std/option.bv"`.
   The compiler must not know about specific types (`String`, `RingBuffer`),
   specific function names (`"ring_push"`), or specific properties. Everything
   type-specific belongs in config/llvm-ops.toml or stdlib .bv files.
   Primitive types (Int, Float, Bool, Ptr, Void) are the sole exceptions —
   the compiler needs these to bootstrap.

3. **INTRINSICS BEFORE FRGN**: Before writing `frgn`, check if an intrinsic
   exists. Print? `PrintInt#`. Input? `GetEnvInt#`. GPU? `GetGlobalId#`.
   The `#` suffix is part of the identifier — `Sqrt#(x)` parses as a regular
   `Expr::Call("Sqrt#", [x])`. All intrinsic names are PascalCase + `#` suffix.
   Add new intrinsics to `execute_intrinsic()` in `src/interpreter/intrinsics.rs`
   and to `get_intrinsic_signature()` — never add `frgn`.

4. **INTERPRETER IS REFERENCE**: If the interpreter runs it correctly, the
   backend must compile it. Fix codegen, never the interpreter.

5. **ADDITIVE ONLY**: Never modify existing optimization paths. New match arms
   only. The `_ => return None;` fallthrough must remain unchanged.

6. **ALWAYS FINISH**: No `todo!()`, `unreachable!()`, `// TODO:`, or stubs in
   committed code. Every feature must be wired parser → AST → analysis → codegen → tests.

7. **NEVER DISCARD UNCOMMITTED WORK**: The working tree may hold critical
   work-in-progress from multiple agents. `git checkout -- <file>`, `git restore`,
   and `git checkout .` DESTROY uncommitted changes permanently. They are never
   acceptable. If you need to change focus:
   - **Commit your own changes** with `git add <your files> && git commit`.
     Targeted add+commit never touches other agents' files.
   - **Never stash** work from other agents — stashing creates a single bundle
     that risks loss when popped over conflicts. Commit is safer.
   - **Never use `git checkout --`** or `git restore` on any file, for any reason.
     If a file needs reverting, discuss with the team first.
   - `git reset HEAD <file>` is safe for unstaging (does not modify file contents).
   The git index holds critical work-in-progress. Treat every uncommitted change
   as irreplaceable.

8. **TESTS OR IT DOESN'T EXIST**: Every new feature, every code path, every
   match arm must have tests. `cargo test --lib` before every commit.

9. **NO PROTOTYPING — BUILD CLEAN**: Every optimization is a first-class pass
   in its proper module. Never inline new analysis into codegen as a shortcut.

10. **EXECUTIVE REQUESTS ARE NOT OPTIONAL**: When told to fix a pattern, do
    the work. All of it. If unsure, ask — do not decide. If prereqs are missing,
    implement them first.

11. **PLAN WITH BENCHMARKS**: Every performance optimization plan MUST include
    a baseline table of ALL benchmark results (ratios, Brief times, C times,
    correctness status) at the current commit BEFORE any changes. After
    implementation, the plan MUST be updated with the new results for
    comparison. This prevents "optimizations" that fix one benchmark while
    silently regressing others. The baseline must be from a clean `cargo build
    --release` + `bash benchmarks/build_and_bench.sh --runtime` run.

11b. **PERSISTENT BASELINE WORKTREE**: A permanent git worktree at
    `../brief-compiler-baseline` holds the current baseline commit (`b39461e2`)
    for regression detection. Always compare against this baseline before
    committing performance-sensitive changes:

    ```bash
    bash benchmarks/compare_baseline.sh <benchmark_name>
    ```

    The worktree is a detached HEAD at the pinned commit. It shares git
    objects with the main worktree (no duplication). Update only when ALL
    current benchmarks equal or exceed the baseline:

    ```bash
    rm -rf ../brief-compiler-baseline
    git worktree prune
    git worktree add ../brief-compiler-baseline <new-tip-commit>
    cd ../brief-compiler-baseline && cargo build --release
    ```

    This provides a controlled A/B experiment (same machine, same hardware)
    that eliminates "system noise" as an excuse — per Golden Rule 11 in
    AGENTS_HISTORY.md. See `docs/plans/2026-07-19-baseline-comparison.md`.

12. **DOCUMENTATION MAINTENANCE IN PLANS**: Every optimization plan MUST
    include a "Documentation" section that specifies:
    - Which `///` doc comments need updating (function signatures, new params)
    - Which rationale comments (`// 2026-07-DD: ...`) need adding at each
      modified code site, explaining WHY the change exists and what pattern
      it targets
    - Which architecture docs (`docs/architecture/`) need updating if the
      optimization changes a dispatch decision or codegen strategy
    - How to preserve existing commentary when refactoring (never delete
      rationale comments — rewrite them to explain the new structure instead)
    Rationale comments are institutional memory. A plan without a documentation
    strategy will produce unmaintainable code.

13. **STDLIB IS THE EXTENSION MECHANISM**: New functionality goes in `.bv`
    files, not new Rust match arms. The compiler teaches; stdlib learns.
    A user writing `MyQueue<T>` with `InsertAt <~ my_push(#L, #R)` must get
    the same `<-` behavior as `RingBuffer<T>` without touching the compiler.

14. **NO KNOWLEDGE OF SPECIFIC TYPES**: The compiler must never check for
    `Type::string()` or match on `"ring_push"` in Rust code. Type-specific
    logic lives in config files (`config/llvm-ops.toml`) and stdlib `.bv`
    files (property bindings). The sole exception: primitive types the
    typechecker needs to bootstrap (`Int`, `Float`, `Bool`, `Void`, `Ptr<T>`).

15. **FULL PROVENANCE TRACKING**: Every code site with a rationale comment
    must carry *when, why, what pattern it targets,* and *how to undo it*
    if it becomes obsolete. Temporary solutions are explicitly flagged with
    `// TEMP: YYYY-MM-DD: <reason>` and describe the path to permanence.
    This prevents "I'll fix it later" from fossilizing into architecture.
    The absence of a provenance comment is itself a decision — it means
    "this change needs no justification," which should be rare.

16. **DON'T REPEAT YOURSELF**: Every code pattern that appears in 3+ places
    must be extracted into a centralized helper. Before writing a new backend
    IR emission sequence, check if an existing function already does it:
    `emit_state_gep`, `ensure_typed_value`, `adapt_to_i64`, etc.
    When changing a function's return type or behavior, grep ALL call sites
    — don't assume you found them all. A pattern repeated in N places means
    every bug fix must be applied N times. The first time you write a pattern,
    it's a one-off. The second time, extract it. The third time is a bug
    waiting to happen. See `docs/plans/2026-07-19-dry-consolidation.md`.

 17. **MIGRATE WHEN TOUCHED**: Remaining pre-DRY sites should not be migrated
     in bulk — that risks regression with low reward. Instead, when you modify
     a file for any other reason, migrate its hand-rolled GEP+load/store
     instances to the centralized `emit_state_load_*` / `emit_state_store_*`
     helpers at the same time. The centralized helpers already exist; this rule
     ensures they are adopted incrementally without dedicated refactoring passes.

 18. **NO TYPE NAME MATCHING**: Never match on Brief type names (`t == "Int"`,
     `t == "Float"`, `s == "String"`) in Rust code. The type's LLVM representation,
     protocol category, boxing behavior, and ABI width are derived from its
     `ResolvedType` in the `TypeUniverse` — specifically the `llvm_type` property,
     `max_bits`/`min_bits` bounds, and `Cast.#<Protocol>` properties. These are
     populated by the PRIMORDIALS table and the normalizer. The only exceptions
     are: (a) `Type::Ptr(_)` and `Type::Vector(_, _)` — compiler constructs not
     stored in the universe, (b) `Type::Bits(N)` — a width construct, and
     (c) the `tbaa_node` function — operates on LLVM IR type strings, not Brief
     type names. Everything else must go through the universe.

     **DO**:  `self.is_protocol_member(&ty, "#Float")`
     **DON'T**: `t == "Float"` or `type_is(&universe, ty, "Float")`

     **DO**:  `self.ctx.type_universe.and_then(|u| u.get(key)).map(|rt| rt.properties.get("llvm_type"))`
     **DON'T**: `match s.as_str() { "Int" => "i64", "Float" => "float", ... }`

     Violations are caught by code review and automated audit. A `git grep`
     for `Type::Custom.*==` in `src/backend/llvm/` and `src/glue/` must return
     zero results.

## Plan Directives

Every plan document and every implementation commit must adhere to these five
directives. They are non-negotiable — refer to "Plan Directives" in reviews.

1. **FLAT CONTROL FLOW**: Max 2 nesting levels. No arrowhead code. Use `?`,
   `if let`, guard clauses, and early returns. Extract deeply nested logic
   into named helper functions.

2. **COMMENT THE CODE**: Every modified or added code site must have a rationale
   comment (`// YYYY-MM-DD: <why>`). Comments explain intent, not mechanics.
   Never delete rationale comments from refactored code — rewrite them to
   explain the new structure.

3. **UPDATE ALL EXAMPLES**: When syntax changes, update every example file
   (`examples/`, `lib/std/`, `benchmarks/`) that used the old syntax. Create
   new example files for any syntax that has no existing example.

4. **DOCUMENTATION IS CODE**: Update `docs/architecture/`, `docs/features/`,
   and inline `///` doc comments in the same commit as the code change.
   Outdated docs are bugs.

5. **BEHAVIORAL TESTS, NOT LITERAL TESTS**: Every new feature must have unit
   tests that assert behavioral outcomes — not literal IR snapshots or
   implementation details. A test must pass after refactoring if the behavior
   is preserved. This is the primary regression guard. Test the contract, not
   the implementation.

The user will refer to these as "the Plan Directives" and expects them to be
followed without being reminded.

## Coding Standards

### 1. Flat Control Flow — Max 2 Levels Deep

Never write arrowhead code. Indentation depth must not exceed 2 levels.

**Instead of:**
```rust
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {
        if let Some(result) = val.as_i64() {
            if result > 0 {
                return Some(result);
            }
        }
    }
    None
}
```

**Write:**
```rust
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 {
        return None;
    }
    Some(result)
}
```

Use `?`, `if let`, `map`, `and_then`, and guard clauses to flatten code:
- `let val = opt else { return ... };`
- `if !eligible { return; }`
- `let Some(inner) = x else { return None; }`

If a function requires deeper nesting, extract the inner logic into a named helper function.

### 2. Doc Comments on Every Definition

Every `fn`, `struct`, `enum`, `trait`, `type`, `const`, and `mod` must have a `///` doc comment explaining intent, invariants, and usage.

- **Functions**: what it does, each parameter, return value, any panics or errors
- **Types**: what data they represent, valid invariants, field meanings
- **Traits**: what capability they abstract, expected implementer contract, required methods
- **Modules**: what the module provides, key types, relationship to other modules

Doc comments are read by every engineer touching the code. Write them as if the reader knows Rust but not the domain. This is non-negotiable — code with missing doc comments must be rejected in review.

### 3. Input Validation and Defensive Checks

Every function must validate its inputs before use:
- Check array/vector bounds before indexing
- Assert struct invariants hold after construction or mutation
- Print diagnostic context (function name, relevant values, expected vs actual) when validation fails
- Check for NaN/Inf in floating-point parameters at FFI boundaries

Use `debug_assert!` on hot paths, `assert!` for safety-critical invariants. Validation failures must produce messages that identify the function, file, and relevant state so bugs can be diagnosed from logs alone.

### 4. Early Returns Over else-if

Beyond a simple `if/else`, use guard clauses and early returns. `else if` chains deeper than one level are forbidden.

```rust
// Forbidden:
if a { A }
else if b { B }
else if c { C }
else { D }

// Write:
if a { return A; }
if b { return B; }
if c { return C; }
D
```

### 5. Continuous Git Commits

- Commit after each logical step, not at end of day
- `git add` only intended files — inspect `git status` and `git diff` first
- Write concise commit messages that state what and why (reference plan file if applicable)
- Never amend commits — create new ones
- The repo must always be in a state you can roll back to a working checkpoint
- **The user explicitly requires auto-committing between checkpoints.** Do not ask
  "shall I commit?" — just commit when a logical step is complete and tests pass.

### 6. Need-to-Know Dependency Injection

Functions should receive only the data they need, not large context structs. When a function needs specific fields from a large state object, pass those fields explicitly.

```rust
// Avoid:
fn emit_binop(ctx: &CompilerContext, state: &State) -> Result<()>;

// Prefer:
fn emit_binop(builder: &mut LlvmBuilder, op: BinOp, lhs: Type, rhs: Type) -> Result<String>;
```

This makes dependencies explicit, improves testability, and documents which data each function actually uses.

### 8. Metropolitan FFI — GLUE Export Is TOML-Driven (Phase 8)

The `brief export` command generates language wrappers entirely from
`lib/glue.toml` templates. No Rust code knows about specific languages.
Adding a language = adding a `[lang]` section with `protocols` and `templates`.
Config discovery uses `#[serde(flatten)]` — no named struct fields for languages.

GLUE and Metropipe are the two mechanisms under the **Metropolitan FFI** umbrella.
GLUE handles compile-time bridge generation; Metropipe handles runtime shared
memory IPC (`src/ffi/metropipe.rs`).

### 9. Export Uses the Full LLVM Backend (Phase 8)

`brief export` calls `LlvmBackend::generate()` — the same code path as
`brief build --llvm`. No `ret i64 0` stubs. The stub codegen in `library.rs`
is deprecated for the export path but kept for `brief library` backward compat.

### 10. String Format Is C-Compatible (Phase 8)

The LLVM backend stores strings as `[i64 length][data\0]` — the same format
as `brief_rt.c`. Global constants use `<{ i64, [N x i8] }>` with the handle
pointing to the struct start. `emit_load_length` reads `handle[0]`.
`brief_str_to_c` strips tag bits via `& ~3` before reading.

### 11. Protocol Paths Are Computed via BFS (Phase 8)

`resolve_single_frgn()` calls `compute_protocol_path()` which uses
`find_cast_path()` BFS from `layout_optimizer.rs`. Falls back to
`Cast(#Bits)` bitcast when no protocol path exists. `emit_protocol_chain()`
in `src/glue/bridge.rs` emits real LLVM IR for Bitcast, MeldShuffle, and
ProtocolTransform kinds.

### 7. HashMap Iteration Determinism

Every HashMap iteration that produces LLVM IR instructions MUST be sorted by
key before the loop. Rust's `HashMap` uses SipHash with a random seed per
process — iteration order differs every compilation.

**Wrong** (non-deterministic IR — up to ~9% performance variation):
```rust
for (name, reg) in &self.fun.phi_field_regs {
    writeln!(out, "  {} = phi {} ...", reg, ty).ok();
}
```

**Right** (deterministic IR — same machine code every compilation):
```rust
let mut sorted: Vec<(String, String)> = self.fun.phi_field_regs.iter()
    .map(|(k, v)| (k.clone(), v.clone())).collect();
sorted.sort_by_key(|(k, _)| k.clone());
for (name, reg) in &sorted {
    writeln!(out, "  {} = phi {} ...", reg, ty).ok();
}
```

This applies to ALL HashMaps whose iteration order determines IR instruction
order: `field_index_map`, `phi_field_regs`, `backedge_field_regs`, `last_val_temps`,
`done_needs_fields`, `pending_phi_backedge`, `pending_phi_native_backedge`,
`vector_phi_groups`, `vector_phi_current`, etc. HashMaps used solely for O(1)
lookups (never iterated for emission) are fine.

Reference: commit `139c345`, `docs/plans/2026-07-06-ir-determinism-and-benchmark-strategy.md`,
and the warning comment at `src/backend/llvm/context.rs:223`.

## Commands

- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/briefc rbv <file.rbv>`
- **Benchmark**: `bash benchmarks/build_and_bench.sh` — always use this harness.
  Ad-hoc timing produces false hangs and imprecise numbers.
- **Compare against baseline**: `bash benchmarks/compare_baseline.sh <name>` —
  compiles and times a benchmark on both the main worktree and the permanent
  baseline worktree at `../brief-compiler-baseline` (post-SLP anchor `be6583bc`).

## Anti-Patterns (NEVER DO)

- Changing `[product > 0]` to `[true]` because code doesn't set product
- Using generic contracts like `[true]` that pass everything
- Adding postconditions that don't guarantee specific outcomes
- Adding Rust string-match built-ins when the standard library or import system should be used
- Pre-populating interpreter state with enum constants (None, Some, Ok, Err)
- Adding `x == x` self-references in preconditions to force liveness
- Adding synthetic exit-condition fields solely to prevent dead-field elimination
- **Hardcoded `from` strings**: `from "libruntime"` — use `from "c"` or `from "link/brief_rt.c"`
- **Missing `from` on frgn**: `from` is required for every frgn declaration
- **`#export` modifier**: Use `export defn` (straight keyword), never `#export`
- **`#out` modifier**: Removed — use `observable <~ true` metadata instead
- **Hardcoded runtime declares**: `__rt_init` etc. must be `frgn` in `std/rt.bv`
- **Name-based interpreter dispatch**: Dispatch on `Value::HashMap`, not `fn_name == "insert"`
- **`"None"`/`"Err"` discriminant magic**: Use enum declaration order, not variant names
- **Runtime type tags for dispatch**: Type is determined statically by `TypedRegister.ty`, never at runtime
- **Implicit coercions**: All type reinterpretations must be explicit via `as` casts
- **Dynamic optimization path switching**: Choose layouts at compile time based on liveness evidence
- **Transitive compatibility inference**: Each compatibility must be explicitly declared
- **Weakening existing optimization paths**: Additional match arms only, never modify existing arms
- **Blaming regressions on "system noise" or "HashMap iteration order" without a controlled A/B experiment**:
  Every suspected regression must be investigated by running a controlled experiment
  (old compiler vs new compiler on the full benchmark suite, same machine, same load).
  Document the results in a plan or fix document before any corrective action.
  "System noise" is not an excuse — if benchmarks are noisy, increase iterations or
  switch to statistical comparison. Always refer to existing documentation
  (`docs/plans/`, `docs/architecture/`, BUGS.md) when a regression is suspected.
- **Old-style Expr match without BinaryOp normalization**: The parser creates `Expr::BinaryOp`/`Expr::UnaryOp`
  (new-style packed variants) for all operations. Any function matching `Expr::Add`, `Expr::Mul`, etc.
  (old-style variants) must first normalize via `expr.normalize_to_old()`. Missing this produces
  silent wrong output — e.g., `try_eval_cfloat` returned `None` for `4.0 * pi * pi`, causing all
  nbody mass constants to be `constant float 0.0` in the IR. **Always normalize before matching.**

## For OpenCode

1. Read CLAUDE.md and this file for full context
2. Follow Contract-First Philosophy — never weaken contracts
3. Test with `cargo test --lib` before committing
4. Document bugs and root causes in BUGS.md
5. Never add Rust built-ins for things the standard library provides
6. **No prototyping**: Every optimization is a first-class pass in its proper module
7. **Never weaken C benchmarks**: Fix Brief to match or beat C, never hobble C with `volatile`
8. **Interpreter IS the reference**: Add to interpreter first, then codegen
9. **Benchmarks on our own terms**: End-to-end results. Features must add language value
10. Write `docs/plans/YYYY-MM-DD-<topic>.md` before starting plan-driven work
11. Update `docs/architecture/` in the same commit as structural changes
12. Add Kani proof harnesses for all new safety-critical code
13. Run Praetor on new/changed files: complexity ≤ 15, lines ≤ 100, params ≤ 6

## Per-Commit Checklist

Before every commit:
1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Run Praetor on new/changed files
4. Update architecture docs if API contracts changed
5. Log bugs/gotchas in BUGS.md or `docs/architecture/praetor-log.md`
6. Add Kani harnesses for all newly written or modified functions

### Regression Guard Checklist (every refactoring)

Before every refactoring change:
7. **Inspect every match arm** in the function being refactored.
   Silent regressions come from removed arms, not added ones.
8. **Verify optimized IR** — not just that tests pass. Run the relevant
   benchmarks and compare against the pre-refactoring numbers.
9. **Update architecture comments** to reflect the new structure. Delete
   no rationale comments; rewrite them to explain the current design.

### LLVM Diagnostic Commands (when optimizer fails)

```bash
# SROA failures (struct not decomposed into scalars)
opt -O3 -pass-remarks-missed=sroa unopt.ll -disable-output 2>&1
# Loop vectorization failures
opt -O3 -pass-remarks-missed=loop-vectorize unopt.ll -disable-output 2>&1
# Alias analysis / GVN failures
opt -O3 -pass-remarks-missed=gvn unopt.ll -disable-output 2>&1
# All optimization remarks at once
opt -O3 -pass-remarks-missed=sroa,gvn,licm,loop-vectorize unopt.ll -disable-output 2>&1
# Inspect IR before/after
opt -S -O3 unopt.ll -o opt.ll
diff <(grep -v '^;' unopt.ll | grep -v '^$') <(grep -v '^;' opt.ll | grep -v '^$')
# Check if %State struct survived SROA
grep '%State' opt.ll
```

## Observability as Liveness

A program that produces no observable effect IS dead code. The compiler is
correct to eliminate it. **A value is live if an FFI call consumes it.**

If the compiler folded your hot loop to `store i64 N`, **the compiler is
right.** Your program produced no observable output. The fix is NOT liveness
hacks (`x == x`, synthetic exit fields). The fix IS `frgn __print_int(result)`.

### `term! -> swan_song` is the correct liveness pattern for terminal programs

```brief
term! -> __print_int(result);   // swan song runs before ret — structurally live
```

**Do NOT:**
- Use `io_pending` or other opaque triggers purely to prevent fold elimination
- Add `#!exit` pragmas when `term!` already terminates the program
- Add synthetic exit-condition fields or `x == x` self-references
- Complain that `main` is just `ret` — the compiler is RIGHT. Fix your program.

**The correct pattern:**
```brief
let N: Int = GetEnvInt#("BOUND");   // runtime-determined

node compute [done < N][done == N] {
    [done == N - 1] {
        term! -> PrintInt#(result);
    };
    &done = done + 1;
    term;
};
```

### Precomputation is Correct, Not a Bug

If the compiler folds your entire hot loop — it had all information at compile
time. Do NOT fight it with hacks. Make the bound runtime-determined:
```
let N: Int = GetEnvInt#("BOUND");  // ✓ not precomputable
const N: Int = 50000000;           // ✗ precomputable
```

The `--optimize-budget` flag (default 256) controls simulation depth. Increase
it or use runtime bounds — never weaken contracts or add hacks.

## Benchmark Philosophy (Condensed)

### Semantic goals, not syntax

Brief benchmarks answer: **"Can Brief compute X with competitive performance
vs C?"** — not "Does Brief have feature Y?" Implement the semantic goal using
Brief's idioms, not a line-by-line port.

### Benchmarks exist to find flaws

A benchmark that fails tells you something is missing. A benchmark that is
"too good to be true" (0.001s for real work) tells you the compiler folded
dead code. Both are diagnostic signals.

### When a benchmark can't be implemented as-is: find the isomorphism

| C pattern | Brief-idiomatic equivalent |
|-----------|---------------------------|
| `malloc` + pointer navigation | Contract-proven struct arrays + index traversal |
| `double u[N]` (runtime-sized) | Contract-proven compile-time bound + `<-` push |
| `HashMap<String, Int>` | Integer-encoded keys + flat field lookup |
| `for (i=0; i<N; i++)` loop | Convergent contract `[count < N][count == N]` |
| `while (true)` + `break` | Reactive transaction with natural death |
| Recursive `enum Tree` | Flat struct pool with index navigation |

### Symmetric by default

Every Brief benchmark must compute the **same output** as its C reference for
the same input. If approaches differ fundamentally, create two benchmarks:

| Variant | Intent |
|---------|--------|
| **Symmetric** (`_sym`) | Mirrors C step-for-step. Answers: "Does Brief's throughput match C?" |
| **Idiomatic** (`_idio`) | Uses Brief-native patterns. Answers: "Can Brief's optimizer find a better path?" |

Both get `-O3 -ffast-math` from the same clang. No `volatile`, no unused
variables. Any asymmetry is a signal of a missing Brief optimization — fix
the compiler, not the C code.

### Two benchmark categories

| Category | Tag | What it measures | Criteria |
|----------|-----|------------------|----------|
| **Runtime** | `--runtime` | Throughput of compiled code | FFI call in hot loop body |
| **Optimizer** | `--optimizer` | Compile-time folding power | All const inputs, no FFI in hot loop |

A benchmark cannot be both. The harness detects precomputed binaries by
`.text` size ratio (< 25% of C → skip timing). Correctness is checked for all.

`bash benchmarks/build_and_bench.sh --runtime` | `--optimizer` | `--correctness`

### Useful utilities become stdlib functions

When a benchmark produces a general-purpose helper (rolling hash, vector math,
frequency counting), extract it into `lib/std/`.

## Iteration Pattern

**Iteration requires `txn` with `[pre][post]` convergence, NOT `defn` + `[guard]`:**

`Statement::Guarded` is a **one-shot conditional** — evaluates the guard once,
executes the body zero or one times. A `defn` body is straight-line with no
implicit transaction wrapping.

The correct pattern is a **callable `txn`** (not `node`). A regular `txn`
takes parameters and returns values like `defn`, but its body executes in a
convergence loop: precondition → body → postcondition → repeat if precondition
still holds.

```brief
txn iter_map<T, U>(list: List<T>, f: T -> U, result: List<U>, i: Int)
    [i < list :> Size][i == list :> Size] -> List<U>
{
    result = result.append(f(list[i]));
    i = i + 1;
    term result;
};

defn iter_map<T, U>(list: List<T>, f: T -> U) -> List<U> {
    term iter_map_loop(list, f, [], 0);
};
```

| Construct | Semantics | When to use |
|-----------|-----------|-------------|
| `defn` | Pure function, straight-line | Stateless computations, wrappers |
| `txn params [pre][post] -> Ret` | Callable convergent loop | Iteration, accumulation, recursion |
| `node [pre][post]` | Reactive, reactor-driven | State machines, event-driven |
| `[guard] { body }` | One-shot conditional | If/else inside a `txn` body |

## Key Backend Rules

### Three canonical backends (only these matter)

| Backend | Target | Status |
|---------|--------|--------|
| **LLVM** (`src/backend/llvm/`) | Native binary (`.ll` + `llc`) | **Active** |
| **Webstack** (`src/backend/webstack.rs`) | WASM + JS glue | **Active** |
| **CIRCT** (`src/backend/circt.rs`) | Hardware (`.mlir` → Verilog) | **Active** |

### Dead backends — zero fixes

`verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`, `x86_64.rs`,
`aarch64.rs`, `wasm.rs`, `tcl_generator.rs`

Do not modify for any reason. If a shared API change mechanically breaks a
dead backend, use `#[allow(unused_variables)]`, `_ => {}`, or `todo!()` with
a comment `// dead backend` — do not implement the feature.

### Never weaken optimizations for new features

Existing optimization paths MUST NOT regress. All additions are additive —
new match arms only, no touching existing fold/precompute/dispatch paths.
The `_ => return None;` fallthrough must remain unchanged.

### Contracts enable optimizations

Preserve contract information in codegen so the optimizer can reason about
it. The more LLVM knows, the more aggressively it can optimize.

### Contract Rules

1. **`defn` needs no contract** — body is linear, translation from inputs
   to outputs is inherently provable. Add contracts only when you need
   the optimization leverage they unlock.

2. **`txn` must have at least one contract side** — either `[pre][post]`,
   `[pre]]`, or `[[post]`. Convergence must be provable.

3. **Intrinsics have no body** — `Sqrt#(x)` is never declared in source.
   The compiler knows it via the hardcoded signature registry
   (`get_intrinsic_signature("Sqrt#")`). `inop` is removed.

4. **`[true][true]` is rejected** — parser enforces at least one
   meaningful constraint. Use `[[post]` or `[pre]]` sugar instead.

5. **`[[post]` = `[true][post]`** — postcondition-only.
   **`[pre]]` = `[pre][true]`** — precondition-only.

6. **`[true][term == true \|\| term == false]` is a useless tautology** —
   the type system already guarantees the return type. Write a contract
   that actually constrains behavior.

7. **Single-bracket `[expr]` is ambiguous** — parser rejects it.
   Must be `[[expr]` or `[expr]]`.

### Intrinsic Conventions

8. **Intrinsics follow PascalCase with `#` suffix** — `Sqrt#`, `Malloc#`,
   `PrintInt#`, `GetEnvInt#`. The `#` is part of the identifier lexically.
   The `_` prefix convention does not exist in Brief.

9. **No `inop` keyword** — `inop` is removed. All compiler-known operations
   are `#` intrinsics with entries in `get_intrinsic_signature()` and
   `execute_intrinsic()`. Use `defn` with `interpreter_impl` metadata for
   backend-specific implementations.

10. **Side-effecting intrinsics must declare `observable <~ true`** —
    `PrintInt#`, `Malloc#`, `Memcpy#`, and any intrinsic with external side
    effects MUST have `observable <~ true` in their metadata. This prevents
    DCE from eliminating the call. The `observable` property is a
    frontend-intrinsic PascalCase identifier.

### Common Syntax Traps
11. **PascalCase vs snake_case convention** — Protocol identifiers, hashwords,
    and intrinsics use PascalCase. User-defined functions in `.bv` files and
    Rust standard library calls use snake_case.
    - **PascalCase**: `#String<UTF8>`, `#Float<IEEE754>`, `Sqrt#`, `PrintInt#`,
      `#Int`, `#Bits`, `Posit32`, `CastTo(#String<UTF8>)`
    - **snake_case**: `ascii_to_utf8()`, `from_utf8_lossy()`, `jsstring_to_utf8()`,
      `array_map()`, `utf8_to_utf16()`
    The dividing line: if the compiler MUST know the name to function (intrinsic
    registry entries, protocol hashwords), it is PascalCase. If a user could
    rename it and the compiler still works (stdlib functions, Rust calls,
    library helpers), it is snake_case.


 12. **`<-` is statement-level** — it breaks the expression parser.
     You cannot write `let x = &list <- val`. Use standalone statements:
     - `&list <- val;` — push val onto list (destructive insert)
     - `x <- &list;` — pop from list into x (destructive extract)
     - `x <- list;` — read from list without removing (non-destructive copy)
     See item 33 for the `&` move/copy semantics on the RHS of `<-`.

13. **`Byte` is defined in `lib/std/types.bv`** — do not assume it
    exists without importing. If the type isn't needed, use `Int`.

14. **`frgn` is an import** — first name after `frgn` is the C/foreign symbol,
    `as` gives the Brief name. `from` is required. Example:
    `frgn XXH64(data_ptr: Int, len: Int, seed: Int) -> Int as frgn__xxh64 from "link/xxhash/xxhash.c" fallback 0;`

15. **`>>` in nested generics** — `Ptr<Ptr<Int>>` triggers the shift-right token.
    Always add a space: `Ptr<Ptr<Int> >`. This is a lexer limitation.

16. **`_` discard binding** — `let _ = expr;` discards the value. The
    `_` identifier binds nothing. Also works in tuple destructuring:
    `let (_, value) = get_pair();` ignores the first element.

17. **Post-body loops `{ body; &i = i + 1; } [condition];` only work in `txn`/`node`** —
    NOT in `defn`. Use `txn` for iteration; `defn` is straight-line.

18. **One precondition, one postcondition** — A txn contract has exactly
    `[pre][post]` with one block each. Combine multiple conditions with `&&`:
    `[a < N && b < M && running != 0][running == 0]`.

19. **Flat import namespace** — Imports bring names directly into scope.
    No `::` module path syntax. `loader::read_u8(x)` is invalid; use `read_u8(x)`.

20. **No tuple destructuring in `let`** — `let (a, b) = expr;` IS supported.
    The parser accepts `let (name1, name2, ...) = expr;` for tuple return values.
    `_` may be used as a placeholder: `let (_, value) = result;`.

21. **`Int` narrowing is protocol-based** — `defn f() -> Int` may emit LLVM
    `i8` for small constants. This is intended — the narrowing pass proves
    value ranges through contracts, then propagates the narrowed width through
    ALL SSA values consistently (`add i8 0, 42` rather than `add i64 0, 42`
    with `trunc`). Fixed-width types (`Int8`/`Int16`/`Int32`/`Int64`) cap the
    floor via `bits <~ N` metadata so `Int64` never narrows below 64 bits.
    Narrowing operates on `#Int`/`#UInt` protocol membership, not type names.

22. **Import resolution uses file-relative paths** — `import "foo.bv"` resolves
    relative to the **file's own directory**, not the parent directory.
    `"<foo>"` (angle brackets) is a registry lookup, not a file path.

23. **Tuples are heap-allocated** — `(1, 2)` calls `@malloc`. LLVM SROA should
    promote small tuples to SSA registers in optimized builds.

### Type System

24. **`type Foo: List { ... }` declares a type inheriting from List** — but `Foo<Int>`
    is NOT automatically assignable to `List<Int>` in the type checker.
    Projections like `.#Size` and index `foo[i]` may fail on `Foo<Int>`
    even though the runtime representation is identical.

25. **No implicit `Copy` on enums with `String`** — `InsertStrategy::Custom(String)`
    requires removing `Copy` and adjusting comparison code.

### Control Flow

26. **`[expr]` guard syntax — three distinct forms** — The bracket prefix
    introduces a conditional or convergence gate, but must NOT be followed by
    `{`:
    - `[expr];` — **convergence gate** (`Statement::Gate`). Compile-time
      assertion: the analysis must prove `expr` holds at this point, or
      compilation is denied. At runtime, if false, execution branches back
      to the convergence target (loop top in callable txns, end-of-tick in
      reactive txns).
    - `[expr] stmt;` — **guarded single statement** (`Statement::Guarded` with
      1-item body). Evaluates `expr`; if true, executes `stmt`.
    - `when expr { body };` — **block guarded body** (`Statement::Guarded` with
      N-item body). The preferred form for multi-statement guards.

    Guard blocks can appear in sequence: `when cond1 { ... }; when cond2 { ... };`.
    Each guard is checked in order; the first matching guard executes its body.
    A trailing `{ body }` without `when` serves as the else clause.

27. **`type` is for protocols, `struct` is for data** — The keywords have distinct
    roles:
    - `type`: Protocol definitions, operator bindings (`#Int`, `#Float`),
      type system extensibility. `type Int: #Int { op Add(#Int); };`
    - `struct`: Pure data, fixed layout, C-compatible, no methods, no contracts.
      `struct VMStack { data: Int[1024]; len: Int; };` The `struct` keyword
      receives fixed-size array types and the bracket syntax for SIMD operations.
    - `obj`: Full-featured types with methods, contracts, type parameters,
      visibility modifiers.
    Migrating `type { field: T }` patterns to `struct { field: T }` is in progress.

28. **Bracket array syntax with SIMD** — `Int[1024]` declares a fixed-size array
    known at compile time. The compiler embeds it as `[1024 x i64]` in LLVM IR.
    `MyType[N]` works for any type. Currently supports:
    - **Slice syntax**: `arr[start:end:stride]` — zero-copy view with compile-time
      bounds. Any component is optional → `arr[:]`, `arr[4:]`, `arr[:8]`,
      `arr[2:8]` (stride 1), `arr[2:8:2]` (stride 2), `arr[i:j:k]` (dynamic bounds).
      All bounds may be variables — the type narrows to `Vector<T, M>` when
      constant, or `Slice<T>` (runtime descriptor) with variable bounds.
    - **SIMD operators**: `arr1 + arr2`, `arr1 * arr2`, `arr * 2` (scalar
      broadcast) on both `Vector<T, N>` and `Slice<T>` types. Emits `<N x T>`
      vector add/mul or auto-vectorized loop.
    - **Slice as lvalue**: `arr[2:8] = src` — contiguous slices use `memcpy`,
      strided slices use element-by-element loop (LLVM vectorizes).
    - **Contract-bound safety**: `[i >= 0 && i < arr :> Size]` proves every
      access in bounds for both `Vector<T, N>` and `Slice<T>`. Slice length
      is proven by contracts, not runtime checks.
    - **View casts**: `raw as Byte[8192]` — type-punned zero-copy view that
      reinterprets the same bytes as a different element type. Compile-time
      validation: `N * sizeof(T) == M * sizeof(U)`. Also works with slices:
      `raw[0:1024:2] as Int[512]` strided view computed from slice bounds.
      Emits `bitcast` in LLVM IR.
    - **Stdlib, not magic**: `map`, `filter`, `fold`, `any`, `all`, `sum`,
      `product` are regular txn functions in `lib/std/array.bv`. The LLVM
      auto-vectorizer recognizes the `[i < N]` convergence contract and
      vectorizes the load-apply-store loop automatically.

29. **`[[post]` = `[true][post]`** — postcondition-only.
    **`[pre]]` = `[pre][true]`** — precondition-only.

30. **`[true][true]` is rejected** — parser enforces at least one
    meaningful constraint. Use `[[post]` or `[pre]]` sugar instead.

31. **`[cond] { body }` is rejected** — The parser produces an error telling you
    to use `when cond { body }` instead. The bracket prefix `[cond]` at statement
    level is only valid as a convergence gate (`[cond];`) or a guarded single
    statement (`[cond] stmt;`). Block bodies always require the `when` keyword.

 32. **No `main()` function** — Brief has no `main()` entry point. Programs start
     via state-space triggers on `node` declarations. The compiler implicitly
     creates an entry point by instantiating a node and wiring a corresponding
     bootup variable. The scripting plugin (`script` pragma) is one way to
     trigger this, but it is not `main()` — it creates a node + bootup variable
     pair. There is no `defn main()` or `fn main()`.

 33. **`&` is pointer reference on LHS, move/copy discriminator on RHS** —
     `&` on the left-hand side of any operator means "pointer reference" and
     is never valid for mutation syntax. Old code used `&i = i + 1;` for
     state variable mutation — this is incorrect. Use plain `i = i + 1;`
     or `i += 1;` instead.

     On the right-hand side of `<-` (arrow operator), `&` discriminates
     move vs copy semantics:
     - `target <- &source;` — **consume** (move): extracts value from source,
       source is left in an empty/undefined state
     - `target <- source;` — **copy**: extracts value from source, source
       retains the value (non-destructive read)
     - `<- &source;` — **discard**: extracts from source into void (fire-
       and-forget pop)

     `&` never appears on the LHS of any assignment or arrow syntax.
     Pointer references on the LHS use the `Ptr<T>` type and `.` dereference
     syntax instead.

 34. **`op Parse` discriminator syntax** — Parse ops can have optional `pre:`,
     `suf:`, and `reg:` discriminator fields:
     - `op Parse(Decimal, pre:"0x"): parse_hex(#L);` — literals starting with `0x`
     - `op Parse(Decimal, suf:"km"): parse_km(#L);` — literals ending with `km`
     - `op Parse(Decimal, reg:"[0-9a-fA-F]+"): parse_hex(#L);` — regex match
     - `op Parse(Quoted): parse_string(#L);` — string literals
     - `op Parse(Bare): parse_ident(#L);` — bare identifiers
     Multiple `op Parse` can be declared on the same type with different
     discriminators. The compiler resolves by checking (1) form match,
     (2) pre/suf match, (3) regex match. Ambiguity = error.
     Prefix-tagged strings (`sql"SELECT"`) produce `Expr::TaggedQuotedLiteral`.
     Suffix-tagged numbers (`42km`, `3.14f`) produce `Expr::TaggedLiteral`.

## Commenting Mandate (Backend Updates)

**Never delete rationale comments when refactoring.** When consolidating
repeated code (match arms, type dispatch, etc.) into a shared helper, every
rationale comment from the original sites must be preserved — they are the
project's institutional memory. Rewrite them at the helper's definition or at
each call site. If a comment no longer applies after refactoring, rewrite it
to explain the new structure. Comments explaining why specific types are
handled, what edge cases exist, and what bugs were fixed are precious —
never delete them silently.

**Every backend code change must include a comment explaining why it was made
and what it fixes or enables.** The comment format is:
```
// YYYY-MM-DD: <short description of why this exists>
// <what problem it solves, what pattern it targets>
```
Comments must be placed at the site of the change, not in a commit message.
If the change has trade-offs (faster path A but slower path B), the comment
must document them and explain why the chosen approach is optimal for the
targeted situation.

## Optimization Philosophy

### 1. Long-Term Best Optimization

Always emit the IR that produces the BEST FINAL CODE after LLVM's full
optimization pipeline (SROA + GVN + DSE + LICM + vectorizer + backend),
not the IR that looks cleanest before optimization. If a more complex
emission pattern unlocks a downstream LLVM pass (e.g., struct-SSA enables
SROA where per-field GEPs do not), use the complex pattern.

This means:
- Think about what `opt -O3` + `llc -O2` will produce, not just what
  the initial `.ll` looks like
- Prefer patterns that LLVM's optimizer is designed to recognize and
  simplify (phi + icmp + add for induction variables, extractvalue/
  insertvalue for struct decomposition)
- Avoid patterns that produce "already clean" IR at the cost of
  blocking later optimization (e.g., dead stores that DSE must labor
  through a call barrier)
- When in doubt, check the optimized IR (`opt -O3 -S unopt.ll`) and
  count the remaining instructions — that is the true cost
- If you see a way to make the generated IR produce better final code
  after all LLVM passes, implement it — even if the initial IR looks
  more complex

### 2. Regression Prevention

Every optimization decision must leave a comment documenting:
- What pattern it targets
- What it gains (specific benchmarks, expected improvement)
- What it costs (IR bloat, compile time, edge cases)
- Why the trade-off is optimal for the targeted pattern
- What happens if this optimization is removed (exact regressions)

When refactoring, inspect ALL match arms and code paths that the
refactoring touches. A refactoring that accidentally removes an
optimization (like the A005c body stores eliminated on 2026-07-04)
causes silent regressions that may not be caught by correctness
tests. The architecture comments are the primary defense — they
tell the next engineer WHY each pattern exists.

Before every commit:
1. Check: "Does this change affect any existing optimization path?"
2. If yes, verify the optimization still fires (check IR, benchmark)
3. Update comments to reflect the new structure
4. Run full test suite AND benchmark suite
5. Document any trade-off decisions in the commit message

The cost of a missed optimization is measured in months — a pattern
broken today may not be rediscovered until a benchmark regresses, and
the regression may be blamed on "noise" rather than root-caused.

## Regression Watch & Trade-Off Analysis

**Every optimization must consider its effect on ALL code paths, not just the
one it targets.** Before committing an optimization:

1. **Identify the pattern** the optimization targets (e.g., "reactive txn with
   3-5 state fields and a cheap body").

2. **Identify when it would hurt** — what workloads pay more under the new
   codegen? (e.g., "adds a `br` that forces a new basic block, which LLVM
   must then merge back — ~0.1% overhead on 3-field txns").

3. **Eliminate trade-offs where possible**: If the code can detect at compile
   time which path is better, emit different IR for each situation. The default
   answer is NOT "pick one" — it is "detect and branch in the compiler."
   Only settle for a single strategy when runtime detection is impossible
   (e.g., property of the input data, not the program structure).

4. **Always consult existing documentation** before attributing a regression.
   Check `docs/plans/`, `docs/architecture/`, `BUGS.md`, and `git log` for
   prior analysis. A regression that looks like "random noise" often has a
   documented root cause from a previous investigation. Never blame "system
   noise" or "HashMap iteration order" without first checking what changed
   between the two compiler versions and running a controlled A/B experiment
   on the full benchmark suite.

5. **Benchmark both paths** before and after. Compare against C baseline. If
   the optimization helps benchmark A by 2× but hurts benchmark B by 0.1×,
   it may still be worth it — but the comment must explicitly state the cost.

6. **Add a regression check**: When a heuristic chooses between two codegen
   strategies, store a `bool` field on `LlvmBackend` that records which
   strategy was chosen per transaction. The field must be documented and the
   choice must be logged in `report_lines` so benchmark output shows which
   path was taken for each transaction. This makes regressions diagnosable.

## Testing Mandate

**Every new feature, every code path, every match arm must have corresponding
tests.** No exceptions.

- **Interpreter changes**: Add direct AST-construction tests in `src/interpreter.rs`
- **Parser changes**: Add source-text parsing tests in `src/parser.rs`
- **Backend changes**: Ensure existing tests pass (`cargo test --lib`)
- **Legacy code**: Changing old code paths in backends does not require new tests
  for each backend — but the compiler must build and all tests must pass

Run `cargo test --lib` before every commit. **If a change has no test, it does not exist.**

## Backend Architecture Rules (Post-Refactoring)

### 1. Decoupled, Context-Driven Architecture

The backend state must remain strictly stratified into three distinct lifetimes
to prevent state leakage and the fragile "save/restore" anti-pattern:

1. **CompilerContext (Global):** Read-only during code generation. Contains
   AST-level definitions, FFI signatures, target specs, and layout properties.
2. **FunctionContext (Per-Function):** Instantiated per-function/transaction.
   Tracks local variables, types, and the SSA register counter. Must never
   outlive the function it compiles.
3. **LLVMBuilder (Instruction Builder):** The sole writer of LLVM IR instructions.
   Direct `writeln!` formatting to raw strings is forbidden for standard instructions.

**Rules:**
- **No Global State Pollution:** Never add transient, function-scoped compilation
  variables (temporary register caches, back-edge trackers) to the global backend
  struct.
- **Single-Source Registry:** All registers must be requested via
  `builder.gen_reg()`. Manual string-based register arithmetic
  (`format!("%t{}", counter)`) outside the builder is prohibited.

### 2. Strict Defensive Code Generation & Validation

Textual code generation must not bypass the compiler's semantic type checks.

**Rules:**
- **No Untyped Casts:** Every type coercion (`trunc`, `zext`, `bitcast`,
  `ptrtoint`) must be explicitly handled by a centralized type-conversion helper.
  Never assume sizes or inject raw cast strings inline.
- **Memory Safety & Thread Safety:**
  - When generating temporary files for compiler-driven external tools (like
    `llc`), always generate unique temporary filenames (e.g., using process/thread
    IDs or UUIDs) to prevent parallel build collisions.
  - Verify that any pointer-tagging assumptions (such as masking off the lower
    2 bits of string pointers) are strictly validated against target platform
    alignments.
- **Explicit FFI Type Declarations:** Every foreign function called by the
  compiler must have an explicit LLVM declaration. Mismatches between C-type
  return sizes (like `bool` or `int32_t`) and the LLVM return declaration must
  be explicitly resolved using truncation/extension to prevent ABI register
  corruption.

### 3. Mandatory Trade-Off Documentation

We do not write code without documenting *why* a specific pattern was chosen
over its alternatives.

**Rules:**
- Every significant optimization, structural file separation, or custom logic
  block must begin with a comment block starting with:
  ```rust
  // ── [Feature Name] ──────────────────────────────────────────────────
  //
  // Why [Architectural Choice] over [Alternative]:
  // [Detailed explanation of trade-offs, register pressure, memory, or CPU benefits]
  //
  ```
- This comment must explicitly outline the trade-off (e.g., compile-time budget
  vs. binary size, loop-unrolling factor vs. stack spilling, etc.).

### 4. Dual-Path / Adaptive Optimizations (Dynamic Dispatch)

We do not choose compiler design patterns dogmatically. If a feature can be
implemented in two ways — where each excels under different workloads — **both
must be supported**, and a static decision tree must select the optimal path at
compile-time.

**Rules:**
When implementing or modifying a backend subsystem, evaluate if a hybrid model
is required:

- **Memory Allocations:**
  - *Path A (Stack/Arena):* Short-lived, temporary collections must use scoped
    bump arena allocation.
  - *Path B (Heap):* Escape-analyzed, persistent collections must use safe,
    tracking-enabled heap allocations.
- **Loop Execution:**
  - *Path A (Folded/O(1)):* Bounded loops with pure bodies and constant limits
    must be collapsed into single-instruction compile-time updates.
  - *Path B (Vectorized/Pipeline):* Bounded loops with side-effects or variable
    limits must be compiled using pipeline-friendly SSA register phi nodes.
- **Control-Flow Dispatch:**
  - *Path A (Enum/Switch):* Triggers with value sets within the
    `--optimize-budget` must be lowered to high-performance, switch-dispatched
    case blocks.
  - *Path B (Sequential Reactor):* Complex or unbounded trigger networks must
    fall back to the sequential state-tick evaluation loop.

For every hybrid subsystem, implement a clear, testable cost-model function
(e.g., `optimal_unroll_factor` or `is_fully_precomputable`) to cleanly divide
the execution paths.

### 5. Frontend Constructs Are Abstract — Backends Give Meaning

A frontend construct (`sync`, `txn`, `let`, `[pre][post]`) has a single
consistent meaning regardless of backend. The backend chooses how to
implement that meaning:

| Construct | Universal meaning | LLVM | SPIR-V | CIRCT |
|-----------|------------------|------|--------|-------|
| `sync(d) {}` | Atomic execution + synchronization | Transaction ordering | `OpControlBarrier` | Handshake stall |
| `txn` | Convergent state loop | Phi-node + br | Work-item loop | Clock cycle |
| `let x` | Named binding | Stack or register | Register | Wire |
| `[pre][post]` | State convergence | Branch invariants | Guard predicates | Setup/teardown |

**The rule:** Before adding a `#` intrinsic, check if an existing frontend
construct already carries the semantic information. `Barrier#()` was wrong
because `sync(domain) {}` already means "synchronize here." The backend
should map `sync` to `OpControlBarrier` for SPIR-V. Adding a `#` intrinsic
duplicates semantics and creates maintenance burden.

A `#` intrinsic is only justified when:
1. No frontend construct exists for the operation, AND
2. The operation genuinely requires compiler knowledge (`Sqrt#`, `Memcpy#`)

If the interpreter runs a `sync` block correctly (as sequential code on
CPU), that's the correctness baseline. Backends that need more (barriers,
pipeline stalls) must add `SyncBlock` handling in their codegen — not a
new intrinsic.

## Documentation Maintenance

These documents must be actively maintained to reflect current syntax and behavior.
Always update them in the same commit as corresponding code changes:

| Category | Files | Update Requirement |
|----------|-------|-------------------|
| **Tutorial** | `learn-brief/` | Must reflect current language syntax |
| **Specification** | `spec/SPEC.md` | Ground truth — update on any syntax/semantic change |
| **Architecture** | `docs/architecture/` | Update when dispatch decisions or codegen strategy changes |
| **Tooling** | `syntax-highlighter/README.md` | New keywords/reserved tokens |
| **Bug Tracker** | `BUGS.md` | Log every bug and its fix |
| **Agent History** | `AGENTS_HISTORY.md` | Major session milestones |

The following are **timestamped historical records** and should NOT be retroactively edited:
`docs/plans/*.md`, `.opencode/plans/*.md`, `docs/milestones/*.md`, `benchmarks/results/*.md`.
Outdated claims in these documents are preserved as historical evidence of what was
known at the time. Reference the plan document (`docs/plans/2026-07-21-rct-txn-to-node-rename-and-benchmark-fixes.md`)
for current benchmark results and architecture decisions.

## Key References

| Resource | Location |
|----------|----------|
| **Historical context (pre-2026-06-25)** | `AGENTS_HISTORY.md`, `AGENTS_HISTORY_2.md` (full snapshot) |
| **Bug diagnoses** | `BUGS.md` |
| **Architecture docs** | `docs/architecture/overview.md` |
| **Feature docs** | `docs/architecture/features/` |
| **Channel map** | `docs/architecture/channel-map.md` |
| **Optimization decision tree** | `docs/design/optimization-decision-tree.md` |
| **Backend type dispatch** | `docs/architecture/backend-type-dispatch.md` — **READ THIS FIRST** before modifying any backend type code. No hardcoded `"Int" → i64` mappings. Types are driven by source metadata + config file. |
| **Backend dispatch** | `docs/architecture/features/backend-dispatch.md` |
| **Benchmark strategy** | `docs/architecture/benchmark-strategy.md` |
| **Intrinsics vs stdlib** | `docs/architecture/intrinsics-vs-stdlib.md` |
| **Hash words** | `docs/architecture/hash-words.md` |
| **Kani harnesses** | `docs/architecture/kani-harnesses.md` |
| **Plan documents** | `docs/plans/` |
| **Bounty architecture** | `docs/plans/2026-07-25-phase6-tamer-in-brief.md` |
| **Memory by Proof** | `docs/plans/2026-07-25-memory-by-proof.md` |
| **Ptr Level 3 plan** | `docs/plans/2026-07-09-ptr-level3-borrow-checking.md` |
| **Granular pipeline + AST navigation DSL** | `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md` |
| **Compile-time meta + plugin architecture** | `docs/plans/2026-07-15-compiletime-meta-and-plugin-architecture.md` |
| **Layout DSL** | `docs/architecture/layout-dsl.md` |
| **Data Brief spec** | `docs/architecture/data-brief.md` — `.dbv` and `.dbvl` format specification |
| **Data Brief cheat sheet** | `docs/architecture/data-brief-cheat-sheet.md` — one-page syntax reference |
| **TOML → DBV conversion** | `docs/architecture/applications/toml-to-dbv-guide.md` — patterns for replacing TOML config with DBV |

## Hypothesised Features

These are architectural ideas that have been discussed but not yet designed or
implemented. Each links to a document capturing the current design questions.
They are NOT commitments — they become active plans only when a
`docs/plans/YYYY-MM-DD-<topic>.md` is written.

| Feature | Document | Core Idea |
|---------|----------|-----------|
| **Target-Aware Protocol Resolution** | `docs/architecture/future/target-aware-protocols.md` | `#String` resolves to `#String<UTF16>` on Windows, `#String<UTF8>` on Linux — default variant depends on target. |
| **Operation Marshalling** | `docs/architecture/future/operation-marshalling.md` | Marshal operations at compile time instead of marshalling data at runtime — adapt to target ABI without source changes. |
| **Memory Management by Proof** | `docs/architecture/future/memory-management-by-proof.md` | Automatically select stack arena vs heap based on compile-time proof of pointer escape. Extends provenance analysis. |
