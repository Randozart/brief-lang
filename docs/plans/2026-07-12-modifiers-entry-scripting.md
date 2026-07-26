# Extension Modifiers, Entry Points, and Scripting

**Date:** 2026-07-12
**Status:** Plan — pre-implementation
**Depends on:** Completion of Extensible Types (Phases 0–7), Derivation &
Synthesis Phases 8.0–8.2 (lexer/parser/AST), Pure Bits Refactor (Phases
7.5, 8A–8G)
**See also:** `docs/plans/2026-07-11-derivation-synthesis-comprehensive.md`,
`docs/plans/2026-07-11-extensible-types-comprehensive.md`,
`docs/architecture/features/metadata-dispatch.md`

---

## Overview

This plan adds four orthogonal frontend features that share no dependencies
on each other and no dependencies on backend code:

| Feature | Flag/Syntax | Phase |
|---------|-------------|-------|
| **Extension modifiers** | `.sf.bv`, `.c.bv`, `.sf.ebv` etc. | 16A |
| **`[#]` entry precondition** | `defn build() -> Int [#] [result == 0]` | 16B |
| **`.f` layout pre-processor** | `.f.bv` — indentation instead of braces | 16C |
| **`.c` cell wrapper + `input`/`output`** | `.c.bv` — file becomes `cell <stem> { ... }` | 16D |
| **Top-level scripting** | Implicit `[#]` + implicit `txn` wrapper | 16E |
| **Stdlib `cli.c.bv`** | Extensible CLI framework in pure Brief | 16F |

All are frontend-only. No backend, no `.dbvl` archive format, no SMT solver
changes. Each can be implemented independently.

---

## Phase 16A — Extension Modifier System

### Goal

Recognize aggregated single-segment modifiers in filenames:
`main.sf.bv`, `server.c.bv`, `sensor.fs.ebv`. Parse the modifier segment
character-by-character and set boolean flags on `CompilationJob`.

### Filename Convention

```
[name].[modifiers].[variant]
   │        │           └── bv, ebv, cbv, abv, rbv, ibv
   │        └── zero or more single-char flags: s, f, c
   └── logical name
```

**Modifier flags:**

| Flag | Meaning | Effect |
|------|---------|--------|
| `s` | Strict | Enable extra verification passes (Phase 16B constraints) |
| `f` | Formatted | Use layout pre-processor (INDENT/DEDENT injection) |
| `c` | Cell | Wrap file contents in `cell <stem> { ... }` |

**Examples:**

| Filename | Variant | Modifiers | Meaning |
|----------|---------|-----------|---------|
| `main.bv` | `bv` | — | Standard Brief |
| `main.f.bv` | `bv` | `f` | Formatted (indentation) |
| `main.s.bv` | `bv` | `s` | Strict checks |
| `main.sf.bv` | `bv` | `s`, `f` | Strict + Formatted |
| `server.c.bv` | `bv` | `c` | Cell-wrapped |
| `server.sfc.bv` | `bv` | `s`, `f`, `c` | Strict + Formatted + Cell |
| `sensor.c.ebv` | `ebv` | `c` | Cell-wrapped Embedded |
| `main.fs.bv` | `bv` | `f`, `s` | Same as `.sf.bv` (order-independent) |

### Step 16A.0 — Parse modifier segment in compiler driver

**File:** `src/main.rs`

**What:** Add a `CompilationJob` struct and `analyze_file_pipeline()` that
splits the filename, parses the modifier segment, and dispatches to the
correct parser pipeline.

```rust
/// Parsed compilation request from a filename.
/// 2026-07-12: Phase 16A.
pub struct CompilationJob {
    pub source_path: PathBuf,
    pub variant: BriefVariant,
    pub layout_parser: bool,      // .f
    pub strict_mode: bool,        // .s
    pub cell_wrapper: bool,       // .c
}
```

```rust
/// Split `main.sf.bv` → name="main", flags="sf", variant="bv"
/// 2026-07-12: Phase 16A.0
fn analyze_file_pipeline(path: &Path) -> Result<CompilationJob, CompilerError> {
    let filename = path.file_name()
        .and_then(|s| s.to_str())
        .ok_or(CompilerError::InvalidPath)?;

    let parts: Vec<&str> = filename.split('.').collect();
    match parts.len() {
        2 => {
            let variant = BriefVariant::from_str(parts[1])?;
            Ok(CompilationJob {
                source_path: path.to_path_buf(),
                variant,
                layout_parser: false,
                strict_mode: false,
                cell_wrapper: false,
            })
        }
        3 => {
            let flags = parts[1];
            let variant = BriefVariant::from_str(parts[2])?;
            let layout_parser = flags.contains('f');
            let strict_mode = flags.contains('s');
            let cell_wrapper = flags.contains('c');
            Ok(CompilationJob {
                source_path: path.to_path_buf(),
                variant,
                layout_parser,
                strict_mode,
                cell_wrapper,
            })
        }
        _ => Err(CompilerError::MalformedFilename(
            "expected [name].[bv] or [name].[flags].[bv]".to_string()
        )),
    }
}
```

**Nesting check:** Single match with three arms, each arm is a guard clause
sequence — depth 1.

**Tests:**
- `test_filename_standard`: `main.bv` → no modifiers
- `test_filename_sf`: `main.sf.bv` → strict=true, layout=true
- `test_filename_fs`: `main.fs.bv` → same as sf (order-independent)
- `test_filename_c`: `server.c.bv` → cell=true
- `test_filename_sfc_ebv`: `sensor.sfc.ebv` → all three + ebv variant
- `test_filename_too_many_parts`: `a.b.c.bv` → error
- `test_filename_no_extension`: `main` → error

### Step 16A.1 — Wire CompilationJob into the existing dispatch

**File:** `src/main.rs`

**What:** Replace the current single-path `fn compile(path)` with dispatch
through `CompilationJob`. When `cell_wrapper` is true, parse the file then
wrap the resulting AST in a `TopLevel::Cell(CellDef { ... })`. When
`layout_parser` is true, run the layout pre-processor before the main lexer.

Existing dispatch point (pseudocode):

```rust
fn compile_source(path: &Path) -> Result<(), CompilerError> {
    let job = analyze_file_pipeline(path)?;
    let source = fs::read_to_string(&job.source_path)?;

    // Phase 16C: .f — run layout pre-processor before lexer
    let preprocessed = if job.layout_parser {
        LayoutPreprocessor::process(&source)?
    } else {
        source.clone()
    };

    // Standard parse pipeline
    let mut parser = Parser::new(&preprocessed);
    let mut program = parser.parse()?;

    // Phase 16D: .c — wrap in cell
    if job.cell_wrapper {
        let stem = path.file_stem().unwrap().to_str().unwrap();
        // Strip the modifier prefix if present: "main.sf" → "main"
        let cell_name = stem.split('.').next().unwrap();
        program = wrap_in_cell(program, cell_name)?;
    }

    // Phase 16B: .s — enable strict verification
    if job.strict_mode {
        program.flags.strict_mode = true;
    }

    // Continue with existing compilation ...
}
```

**Tests:**
- `test_compile_sf_bv`: `.sf.bv` file compiles with layout + strict
- `test_compile_c_bv`: `.c.bv` file produces cell-wrapped AST
- `test_compile_c_ebv`: `.c.ebv` file produces cell-wrapped AST with ebv variant

---

## Phase 16B — `[#]` Entry Precondition

### Goal

Add `[#]` as a contract precondition that marks a function as a CLI-addressable
entry point. The compiler enforces that no internal code calls it, and the
backend generates a lightweight argument parser from the function signature.

### Semantics

```brief
defn build(project: String, clean: Bool) -> Int
    [#]
    [project != ""]
    [result == 0]
{ ... }
```

1. **CLI dispatch:** Running `myapp build --project ./src --clean` calls
   `build("./src", true)`. The compiler generates a zero-dependency
   `argc`/`argv` parser from the function's parameter names, types, and
   preconditions.

2. **Call graph isolation:** No internal Brief code can call `build()`.
   The SMT verifier enforces this statically.

3. **Precondition validation:** `[project != ""]` is validated at runtime
   by the generated CLI wrapper before the function body executes.

4. **Composability:** A binary can have multiple `[#]` functions — they
   become subcommands (`myapp build`, `myapp test`, etc.).

### Step 16B.0 — Add `HashPrecondition` contract variant

**File:** `src/ast.rs` — Contract struct

**What:** The `Contract` struct currently has `pre_condition: Expr` and
`post_condition: Expr`. Add `is_entry: bool` to mark `[#]`:

```rust
/// 2026-07-12: Phase 16B.0
pub struct Contract {
    pub pre_condition: Expr,
    pub post_condition: Expr,
    pub is_entry: bool,      // true if [#] was used
    pub watchdog: Option<Watchdog>,
    pub span: Option<Span>,
}
```

When `is_entry` is true, the `Expression` for the precondition field is
ignored (the environment provides it). The compiler treats it as
`pre_condition = true` for internal SMT purposes, but the `is_entry` flag
signals the codegen path.

**Tests:**
- `test_contract_entry_true`: Parse `defn f() -> Int [#] [result == 0]` →
  `is_entry == true`, post_condition is `result == 0`
- `test_contract_entry_false`: Parse `defn f() -> Int [true] [true]` →
  `is_entry == false`
- `test_contract_entry_no_post`: `defn f() -> Int [#]` → `is_entry == true`,
  post_condition defaults to `true`

### Step 16B.1 — Parse `[#]` in contract position

**File:** `src/parser.rs` — `parse_contract()` method

**What:** Recognize `[#]` as a valid contract. The parser treats it as a
contract with `is_entry = true`. Both `[#]` alone and `[#][post]` are valid.

```rust
fn parse_contract(&mut self) -> Result<Contract, SyntaxError> {
    // Parse optional first bracket: [#] or [pre]
    if let Some(Ok(Token::LBracket)) = self.current_token() {
        self.advance();
        // Check for the # entry marker: [#]
        if let Some(Ok(Token::Hash)) = self.current_token() {
            self.advance();
            self.expect(Token::RBracket)?;
            // Now look for optional postcondition: [#] [post]
            let post = if let Some(Ok(Token::LBracket)) = self.current_token() {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(Token::RBracket)?;
                expr
            } else {
                Expr::Bool(true)
            };
            return Ok(Contract {
                is_entry: true,
                pre_condition: Expr::Bool(true),
                post_condition: post,
                watchdog: None,
                span: None,
            });
        }
        // ... existing pre/post parsing ...
    }
    // ... no-contract case ...
}
```

**Tests:**
- `test_parse_entry_only`: `defn main() -> Int [#] { term 0; };` → entry=true
- `test_parse_entry_with_post`: `defn f() -> Int [#] [result == 0] { ... };`
- `test_parse_entry_rejects_pre`: `[#][x > 0]` → error (no precondition with `[#]`)
- `test_parse_entry_on_txn`: `txn run() [#] [bal' == bal - amt] { ... };` → valid

### Step 16B.2 — Enforce call graph isolation

**File:** `src/typechecker.rs` — call graph analysis pass

**What:** During type-checking, build the call graph. If any function calls
a `[#]`-marked function, emit a compile error.

```rust
/// Check that [#] functions are never called internally.
/// 2026-07-12: Phase 16B.2
fn check_entry_call_graph(program: &Program) -> Result<(), Vec<CompileError>> {
    let entry_fns: HashSet<&str> = program.definitions.iter()
        .filter(|d| d.contract.is_entry)
        .map(|d| d.name.as_str())
        .collect();

    let mut errors = Vec::new();
    for defn in &program.definitions {
        if defn.contract.is_entry {
            continue; // don't check entry functions for calling themselves
        }
        let calls = extract_function_calls(&defn.body);
        for call_name in &calls {
            if entry_fns.contains(call_name.as_str()) {
                errors.push(CompileError::new(
                    "cannot call entry-marked function from internal code",
                    format!("'{}' is marked [#] and can only be called from the CLI", call_name),
                ));
            }
        }
    }
    // ... same for transactions ...
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

**Tests:**
- `test_entry_call_graph_violation`: `defn helper() { main(); };` → error
- `test_entry_call_graph_isolated`: No internal calls to `[#]` functions → ok
- `test_entry_called_from_entry`: One `[#]` calling another → ok (still CLI)

### Step 16B.3 — Emit CLI dispatch wrapper in LLVM backend

**File:** `src/backend/llvm/emit_entry.rs` (new)

**What:** When a `[#]` function is present, emit a `main` function that
parses `argc`/`argv`, matches the first argument to the function name,
parses `--key value` pairs from the parameter list, validates preconditions,
calls the function, and returns its result.

**For a binary with no `[#]` functions:** emit the existing `_start` wrapper.

**For a binary with one explicit `[#]` function:** emit a `main` that
parses `--key value` arguments for that function's parameters directly
(no name subcommand — the function name is irrelevant to the CLI).
This case arises from explicit `defn f() -> Int [#] { ... };` or from
the implicit scripting wrapper (Phase 16E), which are mutually exclusive.

**For a binary with multiple `[#]` functions:** emit a `main` that matches
`argv[1]` to the function name, then parses the remaining args per that
function's signature. This only applies to explicit `[#]` functions.

**Example LLVM IR for single `[#]` function (`build(project: String, clean: Bool)`):**

```llvm
define i32 @main(i32 %argc, ptr %argv) {
entry:
  ; Parse --project and --clean from argv
  ; Validate project != ""
  ; Call @build(%project, %clean)
  ; Return result
}
```

**Precondition validation:** For each precondition on the `[#]` function,
emit a runtime check in the wrapper:

```llvm
; From [project != ""]
%empty = icmp eq i64 %project_len, 0
br i1 %empty, label %error, label %ok

error:
  call i32 @puts(ptr @err_msg)
  ret i32 1
```

**Tests:**
- `test_emit_entry_single`: One `[#]` function → wrapper emitted
- `test_emit_entry_multiple`: Two `[#]` functions → subcommand dispatch
- `test_emit_entry_precondition_check`: `[port > 1024]` → runtime check in IR
- `test_emit_entry_no_entry_functions`: No `[#]` → no wrapper (existing behavior)

### Step 16B.4 — Export entry signatures in `.dbvl` archive

**File:** `src/archive/writer.rs`

**What:** Include `[#]` entry points with their full signature (name, params,
types, preconditions) in the `.dbvl` archive. This allows external tools
(doc generators, shell completion scripts, the stdlib `Cli` cell) to discover
entry points without parsing source code.

```dbvl
// In the .dbvl archive:
entry,build,project:String|clean:Bool,Int,{condition:"project != \"\""}
```

The `entry` tag is distinct from `defn` — it marks functions that are
CLI-addressable. Backends that generate shell completions or man pages
consume this tag.

**Tests:**
- `test_archive_emit_entry`: `[#]` function produces `entry` line in archive
- `test_archive_entry_precondition`: Precondition string serialized in archive

---

## Phase 16C — `.f` Layout Pre-Processor

### Goal

Support indentation-based syntax (no braces, no semicolons) via a lexer
pre-processor that injects virtual `{`, `}`, and `;` tokens from
indentation changes. Activated by the `.f` modifier flag.

### Step 16C.0 — Build LayoutPreprocessor

**File:** `src/layout.rs` (new)

**What:** A lightweight pre-processor that reads the source text, tracks
indentation levels, and injects INDENT/DEDENT/NEWLINE virtual tokens that
the standard lexer consumes as `{` / `}` / `;`.

**Algorithm (standard off-side rule):**

1. Split the source into lines
2. For each line, count leading whitespace (tabs or spaces, consistent)
3. Track current indentation stack (start with level 0)
4. When indentation increases → emit `{` virtual token
5. When indentation decreases → emit `}` for each level dropped
6. At end of each line → emit `;` virtual token (unless line is a block
   opener like `{` or a keyword like `if`, `else`, `txn`, `defn`)

```rust
/// Pre-process indented source into brace-delimited source.
/// 2026-07-12: Phase 16C.0
pub struct LayoutPreprocessor;

impl LayoutPreprocessor {
    pub fn process(source: &str) -> Result<String, LayoutError> {
        let mut output = String::new();
        let mut indent_stack: Vec<usize> = vec![0];

        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                output.push_str(line);
                output.push('\n');
                continue;
            }

            let indent = line.len() - trimmed.len();

            // Dedent: close blocks
            while indent < *indent_stack.last().unwrap() {
                indent_stack.pop();
                output.push_str("}\n");
            }

            // Indent: open block
            if indent > *indent_stack.last().unwrap() {
                // Only indent if the previous line ended with a brace opener
                // or the line ends with : (like defn f() -> Int :)
                indent_stack.push(indent);
                output.push_str("{\n");
            }

            // Emit the line with virtual semicolon
            output.push_str(trimmed);
            if !trimmed.ends_with('{') && !trimmed.ends_with('}') {
                output.push_str(";\n");
            } else {
                output.push('\n');
            }
        }

        // Close all remaining open blocks
        while indent_stack.len() > 1 {
            indent_stack.pop();
            output.push_str("}\n");
        }

        Ok(output)
    }
}
```

**Mixed indentation detection:** If the file mixes tabs and spaces, emit a
clear error: `error: mixed indentation (tabs and spaces) in formatted Brief
(.f.bv)`. Only one indentation style per file.

**Tests:**
- `test_layout_simple`: Basic function with indented body → `{` / `}` / `;`
  injected correctly
- `test_layout_nested`: Nested blocks (if inside function) → correct
  INDENT/DEDENT
- `test_layout_empty_lines`: Blank lines and comments preserved
- `test_layout_mixed_indentation`: Tab + space mix → error
- `test_layout_no_trailing_semicolon_on_brace`: Line ending with `{` doesn't
  get virtual `;`

### Step 16C.1 — Wire into parse pipeline

**File:** `src/main.rs` (Phase 16A.1 integration point)

When `job.layout_parser` is true, run `LayoutPreprocessor::process(&source)`
before passing the result to `Parser::new()`. The parser sees standard
braces and semicolons.

**Note:** The layout pre-processor is a source-to-source transformation.
It runs before the lexer, so the lexer and parser need zero changes.

**Tests:**
- Integration: `.f.bv` file round-trips through pre-processor → parser
  produces same AST as equivalent `.bv` file
- `test_layout_f_bv_vs_bv`: Write same program in `.f.bv` and `.bv`, parse
  both, compare ASTs

### Step 16C.2 — `f` flag exclusion with embedded targets

**Documentation only:** The `.f.` layout pre-processor is valid with any
variant (`.f.bv`, `.f.ebv`, `.f.cbv`). No code change needed — the layout
pre-processor is independent of the variant.

---

## Phase 16D — `.c` Cell Wrapper + `input`/`output` Keywords

### Goal

When the `.c` modifier is active, the file contents are wrapped in a
`cell <stem> { ... }` declaration. Two new top-level keywords `input` and
`output` declare the cell's parameters and return type.

### Step 16D.0 — Add `input`/`output` keyword tokens

**File:** `src/lexer.rs`

**What:** Add `Input` and `Output` token variants. These are reserved
keywords that are only parsed as top-level declarations when `cell_wrapper`
is active.

```rust
// 2026-07-12: Phase 16D.0
#[token("input")]
Input,
#[token("output")]
Output,
```

**Tests:**
- `test_lexer_input_keyword`: `input` → `Token::Input`
- `test_lexer_output_keyword`: `output` → `Token::Output`

### Step 16D.1 — Parse `input`/`output` at top level

**File:** `src/parser.rs` — `parse_top_level()` or a new cell-file parser

**What:** When parsing a `.c.bv` file, the parser recognizes `input` and
`output` as top-level declarations that define the cell's parameters and
return type.

```brief
// server.c.bv
input port: UInt16;
input verbose: Bool;
output status: Int;

state running: Bool = false;
txn start { running = true; };
```

Parsing yields:
- `input` → cell parameter `(port: UInt16, verbose: Bool)`
- `output` → cell return type `-> status: Int`
- Everything else → cell body (state, transactions, definitions)

```rust
/// Parse input/output declarations in .c.bv files.
/// 2026-07-12: Phase 16D.1
fn parse_cell_io_declarations(&mut self) -> Result<(Vec<(String, Type)>, Option<OutputType>), SyntaxError> {
    let mut params = Vec::new();
    let mut output = None;

    loop {
        match self.current_token() {
            Some(Ok(Token::Input)) => {
                self.advance();
                let name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                params.push((name, ty));
            }
            Some(Ok(Token::Output)) => {
                if output.is_some() {
                    return self.spanned_err("only one output declaration allowed".to_string());
                }
                self.advance();
                let name = self.expect_identifier()?;
                self.expect(Token::Colon)?;
                let ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                output = Some(OutputType::Named(name, Box::new(OutputType::Single(ty))));
            }
            _ => break,
        }
    }

    Ok((params, output))
}
```

**Tests:**
- `test_parse_cell_input`: `input port: UInt16;` → parameter added
- `test_parse_cell_output`: `output status: Int;` → output type set
- `test_parse_cell_multiple_inputs`: Multiple `input` lines → all parameters
- `test_parse_cell_duplicate_output`: Two `output` lines → error
- `test_parse_cell_no_output`: No `output` → `output_type` is `None`

### Step 16D.2 — Implement `wrap_in_cell()`

**File:** `src/main.rs` (Phase 16A.1 integration)

**What:** After parsing a `.c.bv` file into a `Program`, wrap it in a
`TopLevel::Cell(CellDef { ... })` using the parsed `input`/`output`
declarations and the filename stem as the cell name.

```rust
/// Wrap a parsed program into a cell declaration.
/// 2026-07-12: Phase 16D.2
fn wrap_in_cell(program: Program, cell_name: &str) -> Result<TopLevel, CompilerError> {
    let (params, output_type) = extract_cell_io(&program);

    // Validate: state fields, transactions, definitions, triggers
    // become the cell body. Everything else is an error.

    Ok(TopLevel::Cell(Box::new(CellDef {
        is_persistent: false,
        name: cell_name.to_string(),
        type_params: vec![],
        parameters: params,
        output_type,
        fields: extract_state_decls(&program),
        transactions: extract_transactions(&program),
        definitions: extract_definitions(&program),
        internal_triggers: extract_triggers(&program),
        span: None,
        modifiers: vec![],
    })))
}
```

**Tests:**
- `test_wrap_cell_basic`: Simple `.c.bv` → cell with correct name, params,
  output, and body
- `test_wrap_cell_name_from_stem`: `server.c.bv` → cell name is `server`
- `test_wrap_cell_no_output`: Cell with no output → `output_type` is `None`
- `test_wrap_cell_rejects_imports`: `import` inside `.c.bv` → error (cells
  can't have top-level imports)

---

## Phase 16E — Top-Level Scripting with Implicit `[#]`

### Goal

Allow writing linear Brief code at the top level without explicitly wrapping
it in a `txn { ... }` block. The compiler automatically:
1. Wraps statements in an implicit `txn` with the file stem as the name
2. Adds `[#]` as the implicit precondition
3. This means every `.bv` file is a valid, runnable entry point by default

### Codegen rule: scripting vs. explicit `[#]` are mutually exclusive

| Case | Source shape | Generated `main` behavior | CLI usage |
|------|-------------|---------------------------|-----------|
| **Explicit `[#]`** (16B) | One or more `defn`/`txn` with `[#]` | Subcommand dispatch via `argv[1]` | `myapp build --x 5` |
| **Implicit `[#]` (scripting)** (16E) | Top-level statements, no named `[#]` | Direct dispatch — no name matching | `./myapp` |

**Rule:** A file can have scripting statements OR explicit `[#]` functions,
never both. The implicit wrapper has no name to register in the subcommand
table because the statements are not attached to any specific function name.
Attempting to combine them is a compile error (enforced in Step 16E.0).

### Step 16E.0 — Detect script mode

**File:** `src/parser.rs`

**What:** After parsing all named declarations (defn, txn, cell, etc.), if
there are top-level statements that aren't wrapped in any declaration, wrap
them in an implicit transaction with `[#]`.

```rust
/// If the program has top-level statements that aren't inside a
/// declaration, wrap them in an implicit entry transaction.
/// 2026-07-12: Phase 16E.0
fn wrap_implicit_entry(program: &mut Program) {
    let mut body = Vec::new();
    let mut has_implicit = false;

    // Collect trailing statements that aren't in any named block
    // (Implementation detail: depends on parser structure)

    if has_implicit {
        let stem = program.source_stem.clone().unwrap_or("main".to_string());
        program.items.push(TopLevel::Transaction(Transaction {
            is_async: false,
            is_reactive: false,
            name: stem,
            parameters: vec![],
            contract: Contract {
                is_entry: true,
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                watchdog: None,
                span: None,
            },
            body,
            // ... remaining fields default ...
        }));
    }
}
```

**Key constraint:** If the file already has at least one `defn` or `txn`
or `[#]` entry point, top-level statements are an error (ambiguity). The
implicit wrapping only applies when there are NO explicit declarations.

**Tests:**
- `test_implicit_entry_simple`: `let x = 42; frgn print_int(x);` → implicit
  `txn` generated with `[#]`
- `test_implicit_entry_name`: File named `compute.bv` → implicit txn named
  `compute`
- `test_implicit_entry_with_explicit`: File has a `defn` and top-level
  statements → error
- `test_implicit_entry_empty`: Empty file → no implicit txn

---

## Phase 16F — Stdlib `cli.c.bv`

### Goal

Provide a **pure convenience** cell in the standard library that wraps
`argc`/`argv` parsing into a reactive interface via `trg` bindings.
Lives at `lib/std/cli.c.bv`.

No interaction with `[#]` entry functions. No compiler changes. No archive
dependencies. This is purely a stdlib utility — it reads `argv` internally
via `frgn __get_argv()`, parses flags into structured values, and pushes
them out through output ports for the rest of the program to consume.

A programmer building a CLI tool never writes argv parsing, never matches
subcommands manually. They import the cell, wire it with a `trg` binding,
and react to the outputs:

```brief
// myapp.bv
import cli from "std/cli";

trg cli @ self.cli;

txn handle_build {
    [self.cli.command == "build"]
    let msg: String = "Building project: %s\n";
    frgn printf(msg, self.cli.project);
};

txn handle_test {
    [self.cli.command == "test"]
    let msg: String = "Running test suite: %s\n";
    frgn printf(msg, self.cli.suite);
};
```

### Design sketch

```brief
// lib/std/cli.c.bv
// Reactive CLI parser cell.
// Reads environment entry data via metadata, emits structured results
// via output ports. No frgn calls, no C runtime dependency.

// The backend (LLVM) populates these from main(argc, argv) on startup.
state argc: Int {
    llvm_entry_arg <~ "argc";
};
state argv: Ptr<Ptr<Byte>> {
    llvm_entry_arg <~ "argv";
};

output command: String;       // First positional arg: "build", "test"
output arg_count: Int;        // Number of flags parsed

// Flag values (populated by name)
output project: String;
output suite: String;
output verbose: Bool;
output port: Int;

// The cell reads argv on tick, parses --key value pairs,
// pushes results to output ports. Program reacts via trg.
```

### How the backend fulfills the metadata

The `llvm_entry_arg` key tells the LLVM backend that a state field should
be initialized from the `main(argc, argv)` function parameters. The backend
generates:

```llvm
define i32 @main(i32 %argc, ptr %argv) {
entry:
  %state = alloca %State, align 8
  ; Populate cell state from entry args
  %argc_field = getelementptr %State, %State* %state, i32 0, i32 <argc_offset>
  store i32 %argc, ptr %argc_field
  %argv_field = getelementptr %State, %State* %state, i32 0, i32 <argv_offset>
  store ptr %argv, ptr %argv_field
  ; ... continue with normal state initialization ...
}
```

The interpreter provides mock values during compile-time evaluation:

```rust
// In the interpreter, when a state field has llvm_entry_arg:
fn initialize_entry_state(state: &mut State, universe: &TypeUniverse) {
    for (field_name, metadata) in &state.metadata {
        if let Some(entry_arg) = metadata.get("llvm_entry_arg") {
            match entry_arg {
                "argc" => state.set(field_name, Value::Bits(0u64.to_le_bytes().to_vec())),
                "argv" => state.set(field_name, Value::Bits(vec![])), // empty mock
                _ => {}
            }
        }
    }
}
```

### Key constraints

1. **No `frgn` calls** — the cell uses `llvm_entry_arg` metadata to receive
   `argc`/`argv` from the backend. No C runtime dependency, no `libc` linking.
2. **No compiler changes** — `llvm_entry_arg` follows the existing metadata
   dispatch pattern (`llvm_instr`, `llvm_asm`, `interpreter_impl`). The
   frontend carries it opaquely; the LLVM backend interprets it.
3. **No `[#]` interaction** — this is a separate utility from the compiler's
   entry dispatch. They share no code, no state, no design.
4. **Interpreter mock** — compile-time evaluation provides dummy values
   (`argc=0`, empty `argv`). Full evaluation happens at runtime.
5. **Extensible by inheritance** — projects define
   `cell MyCli : cli { ... }` to override flag parsing for custom types.

### When to use which

| Case | Use |
|------|-----|
| Simple flags, one or two subcommands | `[#]` — zero-boilerplate compiler feature |
| Complex CLI, custom parsing, reactive dispatch | `cli.c.bv` — fully featured stdlib cell |
| Both together in one project | `[#]` for simple commands, `cli.c.bv` for complex sub-parsing |

### Tests

- `test_cli_cell_roundtrip`: Import and instantiate the Cli cell
- `test_cli_flag_parsing`: `--port 8080` parsed through cell output port
- `test_cli_subcommand_dispatch`: `myapp build --project ./src` → output
  ports match expected values
- `test_cli_extends_cell`: Custom cell inherits Cli, overrides flag parser

---

## Integration with Existing Plan Documents

### Dependency chain

```
Extensible Types (0-7)
  └─ Derivation (8.0-8.2)
  └─ Pure Bits (7.5, 8A-8G)
      ├─ Modifiers + Entry + Scripting (16A-16F)    ← THIS PLAN
      ├─ Alloc Metadata (A.0-A.6)                   ← parallel plan
      └─ Derivation remaining (8.4-8.5, 9-14)
          └─ Phase 15: Library mode
              └─ Zero-copy meld
```

### What changes in the existing derivation plan

The derivation plan's Phase 9 (synthesis engine) does not need changes —
the synthesizer generates standard `Expr::Call` and `Definition` nodes,
unaffected by entry points or layout parsing.

The derivation plan's Phase 12 (.dbvl archive) gains one new entry tag
(`entry`) for `[#]` functions (Phase 16B.4).

### What changes in the extensible types plan

The roadmap section after Phase 7 should reference this plan as the
next step alongside the derivation plan:

> "Phases 16A–16F (Extension Modifiers, Entry Points, Scripting) —
> adds `.s`/`.f`/`.c` filename modifiers, `[#]` entry precondition,
> layout pre-processor, cell wrapper, and top-level scripting."

---

## Testing Strategy Summary

| Phase | Focus | Test count delta |
|-------|-------|-----------------|
| 16A | Extension modifier parsing | ~+10 (filename parsing, dispatch) |
| 16B | `[#]` entry precondition | ~+20 (parser, call graph, codegen, archive) |
| 16C | Layout pre-processor | ~+15 (INDENT/DEDENT, mixed indentation, round-trip) |
| 16D | `.c` cell wrapper + input/output | ~+15 (keywords, parser, wrap_in_cell) |
| 16E | Top-level scripting | ~+10 (implicit txn, implicit [#]) |
| 16F | Stdlib `cli.c.bv` | ~+5 (integration tests) |

---

## Documentation Updates

| Doc | Phase | What |
|-----|-------|------|
| `docs/architecture/features/extension-modifiers.md` | 16A | Filename conventions, flag meanings, examples |
| `docs/architecture/features/entry-points.md` | 16B | `[#]` syntax, call graph isolation, CLI dispatch |
| `docs/architecture/features/layout-parser.md` | 16C | Indentation rules, INDENT/DEDENT, mixed indent error |
| `docs/architecture/features/cell-files.md` | 16D | `.c.bv` convention, input/output keywords, cell wrapping |
| `docs/architecture/features/scripting.md` | 16E | Top-level scripting, implicit entry rules |

---

## Risk Register

| Risk | Phase | Mitigation |
|------|-------|------------|
| `.s` strict mode semantics unclear | 16A | Start with a placeholder flag; define semantics when concrete passes are added |
| Layout pre-processor mis-handles edge cases (tab-aligned comments, inline braces in strings) | 16C | Comprehensive edge-case test suite; round-trip comparison with equivalent `.bv` |
| `[#]` CLI dispatch wrapper conflicts with existing `_start` codegen | 16B | The entry wrapper replaces `_start` when `[#]` is present; no `[#]` → existing behavior unchanged |
| `.c` cell wrapper does not handle all AST node types | 16D | Reject unsupported top-level nodes (imports, melds) with clear error |
| Implicit scripting causes surprises for users writing libraries | 16E | Only trigger when file has zero explicit declarations. Files with explicit `defn`/`txn` → no implicit behavior. Scripting and explicit `[#]` are mutually exclusive by construction (compile error if both present). |
