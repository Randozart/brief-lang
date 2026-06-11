# Plan: Error Messages & Feature Gaps

**Created:** 2026-06-11T21:30Z  
**Status:** Draft  
**Source:** officina-cli development session — ~600 lines of Brief written blind

---

## Phase 1 — Error Message Fixes (P1)

These three are the highest impact because they actively mislead the user. Each produces an error that
points at the wrong problem or communicates in compiler-internal terms.

### 1.1 `Some(Ok(...))` Rust-internal token leaks

**Problem**: Many parser error paths use `format!("{:?}", token)` which renders the Rust Debug
representation of a token. A `Token::LParen` becomes `Some(Ok(LParen))`. A Brief user has no idea
what `Some(Ok(...))` means — this is the compiler's internal type leaking into user-facing output.

**File**: `src/parser.rs`

**Fix**:
1. Add `fn token_display(&self, t: &Token) -> String` that maps each `Token` variant to a
   user-friendly string:
   - `Token::LParen` → `"("`
   - `Token::Underscore` → `"_"`
   - `Token::String(s)` → `'"{s}"'`
   - `Token::Integer(n)` → `"{n}"`
   - `Token::Identifier(s)` → `"{s}"`
   - All keyword tokens → their keyword strings (`Token::Let` → `"let"`, etc.)
2. Search all error formatting strings in `parser.rs` for `{:?}` on token values
3. Replace each with `{}` using `token_display()`
4. In particular, fix `expect_identifier()` — currently renders `"expected identifier, found
   Some(Ok(Underscore))"` when `_` is encountered — should render `"expected identifier, found '_'"`

**Impact**: Every parse error becomes readable. Estimated 10–20 error paths affected.

### 1.2 Cascade hiding in import resolution

**Problem**: When `import_resolver.rs` finds a file on disk but `parse_file()` returns a parse error
(not a "not found" error), the resolver discards the parse error and falls through to the
"not found" error path. The user sees "Cannot find module 'x'" at the import site when the real
error is a syntax error inside the dependency.

**File**: `src/import_resolver.rs`, lines ~420–450

**Fix**:
1. In the file search loop, when `parse_file()` succeeds for the path, check the result:
   - If `Err(parse_error)`, propagate it as `Err(format!("Import error in '{path}': {parse_error}"))`
   - Only emit "Cannot find module" when the file genuinely doesn't exist on disk
2. Ensure the propagated error includes the source file path so the user knows where to look

**Impact**: Saves hours of debugging — the error points at the actual problem, not the symptom.

### 1.3 `[true][true]` silent contract degradation

**Problem**: `parse_contract()` initializes both `pre_condition` and `post_condition` to
`Expr::Bool(true)` (defaults). It only overwrites them if the `while` loop successfully parses `[expr]`
brackets. If bracket parsing fails silently for any reason — including the contract-after-arrow
ordering mistake — both conditions remain `Bool(true)`. The user sees "your contract is [true][true]"
which they never wrote, eroding trust.

**File**: `src/parser.rs`, `parse_contract()` at line 3682, `parse_definition()` at line 3410

**Fix**:
1. In `parse_definition()`, when parsing Path B (contract after `-> Type`), detect that
   `parse_contract()` returned both defaults and emit a hard error:
   *"Contract conditions must appear before `-> Type`. Use `[pre][post] -> Type` syntax, not
   `-> Type [pre][post]`."*
2. In `parse_contract()`, if the `while` loop consumed any `[` brackets but `parse_expression()`
   returned an error that was silently ignored, propagate that error.
3. Rename the result to make silent defaults harder to miss in code review.

**Impact**: The most misleading error in the current compiler becomes a clear, actionable message.

---

## Phase 2 — Feature Gaps (P1–P2)

These are language constructs that clearly *should* work but don't. Each has a specific structural
benefit that's lost without it.

### 2.1 `_` wildcard in tuple destructuring

**Problem**: Both `let (a, _, b) = expr` and `&(a, _, b) = expr` fail because `expect_identifier()`
rejects `Token::Underscore`. The user must assign every position to a named variable even when
they don't need the value.

**Files**: `src/parser.rs` lines ~3991 and ~5238; `src/interpreter.rs` tuple destructure handler

**Fix**:
1. **Parser** at both identifier-parsing loops: before calling `expect_identifier()`, check for
   `Token::Underscore` and push `"_"` (string sentinel) into the names list.
2. **Interpreter** in both `let` and `&` assignment destructure handlers: when iterating names,
   skip any name equal to `"_"` — do not insert into state.

**Impact**: The user can ignore positions they don't need. ~10 lines of code changes.

### 2.2 Match on string literals (and bare literals)

**Problem**: `match x { "foo" => 1, _ => 0 }` fails because:
1. `MatchPattern` has no `Literal` variant — only `Wildcard` and `Variant { name, fields }`
2. The parser calls `expect_identifier()` as the first step for every non-wildcard arm
3. The interpreter's `MatchExpr::evaluate()` only handles `Value::Enum` targets

Note: `Pattern::LitString` already exists and `Interpreter::pattern_match()` already handles it
correctly — but neither is reachable for top-level match arm patterns.

**Files**:
- `src/ast.rs` — `MatchPattern` enum at line 797
- `src/parser.rs` — `parse_match_expr()` at line 6019
- `src/features/pattern.rs` — `MatchExpr::evaluate()` at line 47
- Various backend/analysis files with `MatchPattern` match arms

**Fix**:
1. **AST** (`ast.rs`): Add `MatchPattern::Literal(Pattern)` variant
2. **Parser** (`parser.rs`): Before the `expect_identifier()` fallthrough, add branches for
   `Token::String(s)` → `MatchPattern::Literal(Pattern::LitString(s.clone()))`,
   `Token::Integer(n)` → `Pattern::LitInt(*n)`,
   `Token::Float(f)` → `Pattern::LitFloat(*f)`,
   `Token::BoolTrue/False` → `Pattern::LitBool(true/false)`,
   `Token::Char(c)` → `Pattern::LitChar(*c)`
3. **Interpreter** (`pattern.rs`): Add `MatchPattern::Literal(pat)` arm that calls
   `Interpreter::pattern_match(pat, &target, &mut ctx.state)` directly
4. **Backends**: Add `MatchPattern::Literal(_)` arms to all `MatchPattern` match blocks —
   LLVM codegen, hazard analysis, symbolic execution, dataflow, transition graph

**Impact**: Users can pattern-match on plain values without wrapping them in enum variants.
The exhaustiveness benefit is absent for literal patterns (can't prove coverage), but the
consistency is valuable.

---

## Phase 3 — `foreach(item in list) { body }` Statement (P2)

### Motivation

Current iteration pattern requires ~4 lines of mechanical scaffolding:
```brief
txn filter_fluff(tokens, result, i) [i < tokens:>Size][i == tokens:>Size] -> List<String> {
    [not_fluff(tokens[i])] { &result <- tokens[i]; };
    &i = i + 1;
    term result;
};
```

The contract proves termination, but the index management, accumulator plumbing, and `term` are
bureaucracy — they prove nothing the compiler couldn't prove once and reuse. A `foreach` statement
eliminates the boilerplate while preserving the structural guarantee (the list is finite → termination
is structural, not proven by contract).

### Design

```
foreach (item in list) { body };
```

- **Statement**, not expression — only valid inside `defn`/`txn`/`rct txn` bodies
- Iterates a `List<T>`, binds `item: T` in each iteration (lexical scope)
- **No contracts** — termination is structural (list is finite), no `[i < N][i == N]` needed
- Body can use `[guard] { action }` for conditional logic
- No manual `term` — the loop ends when the list is exhausted
- Result is discarded (use `&acc <-` push inside body for accumulation)

### Implementation

**Parser** (`src/parser.rs`):
- Add `foreach` as a keyword token or reuse `Token::Identifier("foreach")`
- Parse: `foreach ( ident in expr ) { stmts }` followed by `;`
- Produce `Statement::Foreach { item: String, list: Box<Expr>, body: Vec<Statement> }`

**AST** (`src/ast.rs`):
- Add `Statement::Foreach { item: String, list: Box<Expr>, body: Vec<Statement> }`

**Interpreter** (`src/interpreter.rs`):
```rust
Statement::Foreach { item, list, body } => {
    let list_val = self.eval_expr(list)?;
    if let Value::List(items) = list_val {
        for elem in items {
            self.state.insert(item.clone(), elem);
            for stmt in body {
                self.exec_stmt(stmt)?;
            }
        }
    }
}
```

**Typechecker** (`src/typechecker.rs`):
- `list` must evaluate to `List<T>` — infer `T` from element type
- Inside the body, `item: T` is in scope

**Backends** (LLVM, Webstack, VHDL, Rust):
- LLVM: Emit a bounded loop — load list pointer, iterate over elements, execute body for each
- Other backends: Same pattern, or desugar to a synthetic convergent txn

**Validation**:
- Only valid inside `defn`/`txn`/`rct txn` bodies (not at top level)
- Error if `list` is not a `List<T>` type

### Future: `async foreach`

Parallel variant using the existing conflict-free dispatch infrastructure:
```
async foreach (item in list) { body };
```
Same desugaring but iterations dispatched via thread pool when conflict-free.

---

## Phase 4 — Import Resolution Cleanup (P3)

Lower priority because existing imports work correctly for all stdlib paths. The fixes are
architectural hygiene.

### 4.1 Eliminate `/` → `.` → `/` double transform

**File**: `src/import_resolver.rs`, line 123  
**Change**: `import.path.join(".")` → `import.path.join("/")`  
**Impact**: Eliminates a fragile round-trip through string replacement. All module paths
are joined with `/` directly.

### 4.2 Cache resolved programs (not parsed-only)

**File**: `src/import_resolver.rs`, lines 445-446  
**Change**: Move cache insert after `self.resolve_imports()` call, so cached entries contain
fully resolved programs (sub-imports already processed).  
**Bug**: Currently caches the parsed-but-unresolved program on cache miss, then resolves
sub-imports on a separate `resolved` clone. A subsequent cache hit returns the unresolved
version.

### 4.3 Include `source_dir` in "not found" errors

**File**: `src/import_resolver.rs`, lines 428-434  
**Change**: Show the actual filesystem paths searched (prefixed with `source_dir`) instead
of the ambiguous `lib/{path}.bv, imports/{path}.bv, ./{path}.bv` message.

### 4.4 Support relative imports (`"./core"`)

**Problem**: `import "./core"` should resolve relative to the importing file's directory, but
the parser's `trim_start_matches("./")` strips the prefix and the resolver treats it as an
absolute (project-root-relative) path. The user has no way to express "this file's sibling."

**File**: `src/import_resolver.rs`

**Fix**:
1. Preserve a flag when the import path starts with `"./"`: treat it as a relative path hint.
2. When resolving a relative import, use the importing file's directory as the base search
   path instead of (or in addition to) the standard search paths.
3. This also gives users a way to disambiguate local imports from library imports, which
   would have avoided confusion during the officina session.

---

## Cross-cutting: File/Directory Naming Conflicts

**Problem**: A file named `officina.bv` in the same directory as `officina/` (a subdirectory)
creates an ambiguity. When resolving `import "officina.core"`, the module resolver searches:
- `officina.bv` matched as a module namespace → then looks for `core` inside the parsed file
- `officina/core.bv` matched as a subdirectory → the intended target

The resolver could pick the wrong one, or error on ambiguity. In practice, the `.bv` extension
triggers a different code path than the directory search, but the overlap is a footgun.

**Status**: Documented, not yet fixed. A candidate fix would be to prefer directory matches
over file matches when both exist (the directory is more likely to contain submodules).

---

## Already Completed (These Session Fixes)

| Fix | Commit | Summary |
|-----|--------|---------|
| `&(a, b) = expr` destructuring | `67a93ae` | Parser + interpreter + typechecker + LLVM backend |
| Silent postcondition failure | `62afa10` | `return Ok(result)` → `return Err(ContractViolation(...))` |
| Convergence proof for callable txns | `62afa10` | Removed `txn.is_reactive` gate from `check_convergence` |
| Architecture docs | `c52aee5` | Updated `statement.md` and `proof-engine-convergence.md` |
| BUGS.md documentation | `62afa10` | All findings documented with root cause analysis |

---

## Implementation Order

1. Phase 1.1 — `Some(Ok(...))` leaks (~20 lines, all in parser.rs)
2. Phase 1.3 — `[true][true]` contract degradation (~15 lines, parser.rs)
3. Phase 1.2 — Cascade hiding (~10 lines, import_resolver.rs)
4. Phase 2.1 — `_` in tuple destructuring (~10 lines, parser.rs + interpreter.rs)
5. Phase 2.2 — Match on string literals (~40 lines, ast.rs + parser.rs + pattern.rs + backends)
6. Phase 3 — `foreach` statement (~150 lines, parser.rs + ast.rs + interpreter.rs + typechecker.rs + backends)
7. Phase 4 — Import resolution cleanup (~15 lines, import_resolver.rs)
