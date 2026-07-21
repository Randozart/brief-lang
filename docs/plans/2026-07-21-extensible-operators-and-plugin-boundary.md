# Extensible Operators and the Plugin/Language Boundary

**2026-07-21:** Captures the conversation about making `|>` a plugin-registered
operator, the gap it exposed in the metaprogramming system (plugins cannot add
lexer tokens), and the proposed solution: a Pratt parser driven by a dynamic
operator table, with a catch-all `CustomOp` token for user-registered operators.

**This is a design capture, NOT an implementation plan.** The ideas here are
frozen for reference and may be implemented in a future phase.

---

## 1. The Pipeline Operator (`|>`) Conversation

### 1.1 What the user wants to write

```brief
// Pipeline syntax: pipe one expression's result into a function
x |> f            // → f(x)
x |> f()          // → f(x)
x |> f(y)         // → f(x, y)
x |> f |> g       // → g(f(x))  — chained
x |> f |> g .1|> h  // → h(f(x)) — skip 1 in the pipeline stack
x |> f .2|> g        // → g(x)   — skip 2
```

The `.N|>` variants (`|>` = depth 0, `.|>` = depth 1, `..|>` = depth 2, etc.)
allow selecting a result from further back in the pipeline stack rather than
the most recent one.

### 1.2 The initial attempt: `.` chain syntax in the parser

The `.`-based chain syntax (e.g., `Tag$("import").First$().Before$()`) was
implemented via a special case in `src/parser/expressions.rs:250-260`:

```rust
if name.ends_with('$') && self.check(&Token::LParen) {
    // Navigation chain call: a.first$(args)
    // Convert to Call("first$", [a, ...args])
    self.expect(Token::LParen)?;
    let mut args = vec![expr]; // receiver as first arg
    // ... parse user args ...
    expr = Expr::Call(name, args, None);
}
```

**Problem:** This is a parser special-case for `$`-suffixed identifiers only.
It doesn't generalize to `|>`, and it hardcodes knowledge of what the `$`
suffix means into the parser.

### 1.3 The realization: plugins can't add tokens

The `|>` operator cannot be implemented as a plugin for a fundamental reason:
the lexer/parser runs BEFORE any plugin stage. A `$(Parsed)` plugin sees the
AST but `|>` never reaches the AST — the parser rejects it because `|>` is
not a recognized token/operator.

Trace for `a |> f`:

```
1. a        → identifier
2. |        → Pipe token (bitwise OR)
3. parse_bitor calls parse_bitxor for the RHS
4. parse_bitxor → ... → parse_term → parse_factor → parse_unary → parse_postfix → parse_primary
5. >        → Gt token
6. parse_primary fails — > is not a primary expression
```

Without a single `|>` token, the parser cannot construct a valid tree.
The only stage that could help is `$(PreLex)` with a source-text regex
transformation, but that's fragile with nested expressions and parentheses.

**Key insight:** The boundary between what the core provides (lexer tokens,
parser grammar) and what plugins provide (AST rewrites) has a gap at the
lexer/parser level. Any syntax requiring a lexer token change needs a Rust
change — not a plugin.

### 1.4 What `|>` would look like with a token

Ironically, `|>` ALREADY EXISTS as a `PipeGreater` token in the lexer
(`src/lexer.rs:261`), but it's a dead token — lexed but never consumed by
the parser. Wiring it up would be ~20 lines of parser code.

The `.N|>` variants (`.1|>`, `.2|>`, `.3|>`) would need new lexer entries.
With the `logos` crate, a regex pattern works:

```rust
#[regex(r"\.[0-9]+\|>", |lex| {
    let s = lex.slice();
    s[1..s.len()-2].parse().unwrap_or(1)
})]
PipeDot(u32),
```

The multi-dot aliases (`.|>`, `..|>`, `...|>`) are harder because `.` alone
is already a `Dot` token. The `logos` longest-match would see `.` then `|>`
as two separate tokens. These would need dedicated token entries:

```rust
#[token(".|>")]
PipeDot1,
#[token("..|>")]
PipeDot2,
#[token("...|>")]
PipeDot3,
```

For the parser, `|>` and `.N|>` would slot into the binary operator precedence
chain — sitting between assignment and OR:

```
parse_assignment → parse_pipe → parse_or → parse_and → ...
```

Where `parse_pipe` eats `|>` / `.N|>` tokens and produces `BinaryOp(Pipe)` nodes.
A `$(Parsed)` plugin then rewrites `BinaryOp(Pipe, ...)` trees into `Call(...)`
trees that the macro engine already handles.

But this still requires a Rust change to add the token and parser support.
The question became: can we eliminate even this small core footprint?

---

## 2. Full Operator Extensibility

### 2.1 The goal

Allow any `$(PreLex)` plugin to register custom infix operators:

```brief
$(PreLex) {
    Token$.Register$("|>", 30, Left);
    Token$.Register$(".1|>", 30, Left);
    Token$.Register$("~>", 30, Left);
    Token$.Register$(">>=", 25, Right);
};
```

After registration, the parser handles these operators naturally — producing
`BinaryOp(Custom("|>"), lhs, rhs)` nodes — and a `$(Parsed)` plugin rewrites
them into whatever AST form is needed.

### 2.2 Required changes

#### 2.2a Lexer: catch-all for operator characters

Add a `CustomOp(String)` token that catches sequences of operator characters
not matching any existing token:

```rust
#[regex(r"[|>&$@^~!+=/*%-]+", |lex| lex.slice().to_string())]
CustomOp(String),
```

This uses `logos` regex support. The existing fixed tokens (`->`, `:>`, `<-`,
`<~`, etc.) are tried first via longest-match, so they stay unambiguous.
The dead `PipeGreater` token is removed — `|>` falls through to `CustomOp`.

**Concern:** Token ordering in `logos` matters. The regex must be placed
after all fixed tokens so that fixed tokens match first. The `logos` derive
macro processes `#[token(...)]` entries before `#[regex(...)]` entries, so
this ordering should happen naturally. But it needs verification.

#### 2.2b Parser: Pratt loop instead of cascade

Current state (10 recursive functions for binary operators):

```
parse_assignment → parse_or → parse_and → parse_equality → parse_comparison
→ parse_bitor → parse_bitxor → parse_bitand → parse_shift → parse_term
→ parse_factor → parse_unary → parse_as → parse_postfix → parse_primary
```

Proposed replacement: a single `parse_pratt(min_prec)` with a runtime operator
table, plus the existing prefix/postfix/primary parsers for the left-hand side.

```rust
struct OpTable {
    entries: HashMap<String, OpEntry>,
}

struct OpEntry {
    precedence: u8,     // 0-255, higher = tighter
    assoc: Assoc,       // Left | Right
}

impl Parser {
    fn parse_expression(&mut self) -> Result<Expr, SyntaxError> {
        self.parse_pratt(0)
    }

    fn parse_pratt(&mut self, min_prec: u8) -> Result<Expr, SyntaxError> {
        // LHS = any prefix/postfix/primary expression
        let mut expr = self.parse_prefix_unary_or_postfix()?;

        // Consume operators at sufficient precedence
        while let Some((name, entry)) = self.current_op(min_prec) {
            self.advance();
            let next_min = match entry.assoc {
                Assoc::Left => entry.precedence + 1,
                Assoc::Right => entry.precedence,
            };
            let rhs = self.parse_pratt(next_min)?;
            let kind = Self::resolve_kind(&name);
            expr = Expr::BinaryOp(kind, Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn current_op(&self, min_prec: u8) -> Option<(String, OpEntry)> {
        match self.current_token() {
            // Built-in token-based operators
            Some(Token::Plus) => Some(("+".into(), PLUS_ENTRY)),
            Some(Token::Minus) => Some(("-".into()), MINUS_ENTRY)),
            // ...
            // Custom operators from the runtime table
            Some(Token::CustomOp(s)) => {
                self.op_table.get(s).map(|e| (s.clone(), *e))
                    .filter(|(_, e)| e.precedence >= min_prec)
            }
            _ => None,
        }
    }

    fn resolve_kind(name: &str) -> BinaryOpKind {
        match name {
            "+" => BinaryOpKind::Add,
            "-" => BinaryOpKind::Sub,
            // ... all built-in operators ...
            _ => BinaryOpKind::Custom(name.to_string()),
        }
    }
}
```

The `OpTable` is pre-populated at parser creation with all built-in operators
and their precedences (matching the current cascade). `Token$.Register$` adds
entries at `$(PreLex)` time.

**Advantage:** The cascade functions (`parse_or`, `parse_and`, `parse_equality`,
etc.) are replaced by ~80 lines of Pratt loop. This is less code, more
flexible, and eliminates the special-case nature of the current parser.

**Disadvantage:** It's a parser refactor. The existing cascade is well-tested
and handles edge cases (like `a < b > c` and `a + b * c`) correctly. The Pratt
parser needs equivalent testing.

#### 2.2c AST: `Custom(String)` variant in `BinaryOpKind`

```rust
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    // ... all existing ...
    /// 2026-07-21: User-registered binary operator from Token$.Register$.
    /// Rewritten by a $(Parsed) plugin before codegen — handled identically
    /// to other BinaryOpKind variants in the AST, but with no language-level
    /// meaning until a plugin assigns one.
    Custom(String),
}
```

All code sites that match `BinaryOpKind` get a fallthrough arm:

```rust
BinaryOpKind::Custom(ref name) => {
    // Custom operators should be rewritten by a $(Parsed) plugin.
    // If any survive to codegen, it's a compile error.
    return Err(format!("unresolved custom operator '{}' at codegen — \
        a $(Parsed) plugin must rewrite it before this stage", name));
}
```

The interpreter, SMT solver, fuzzer, and derive engine all get this fallthrough.
Since custom operators are always rewritten by `$(Parsed)` plugins, they should
never reach these systems in practice.

#### 2.2d `Token$.Register$` intrinsic

A `$`-suffixed intrinsic callable at `$(PreLex)`:

```brief
$(PreLex) @ 0 {
    Token$.Register$("|>", 30, Left);
    Token$.Register$("~>", 30, Left);
    Token$.Register$(regex: "\\.[0-9]+\\|>", 30, Left);
};
```

Implemented as an intrinsic that adds entries to `OpTable`:

```rust
fn intrinsic_token_register(args: &[Expr], op_table: &mut OpTable) -> Result<(), String> {
    let name = expect_str_arg(args, 0, "Token$.Register$")?;
    let precedence = expect_int_arg(args, 1, "Token$.Register$")? as u8;
    let assoc_str = expect_str_arg(args, 2, "Token$.Register$")?;
    let assoc = match assoc_str.as_str() {
        "Left" | "left" => Assoc::Left,
        "Right" | "right" => Assoc::Right,
        _ => return Err("Token$.Register$: assoc must be 'Left' or 'Right'".into()),
    };
    op_table.insert(name, OpEntry { precedence, assoc });
    Ok(())
}
```

The `$(PreLex)` stage runs before the parser, so the operator table is fully
populated when parsing begins.

### 2.3 What's not covered

| Feature | Why excluded |
|---------|-------------|
| New keywords (`if`, `match`, `foreach`) | Keywords require grammar changes, not just operator precedence. The parser has dedicated functions for `parse_if`, `parse_foreach`, etc. that produce specific AST nodes. |
| New prefix/unary operators | Prefix operators (`!`, `-`, `~`) are parsed in `parse_unary` which has fixed patterns. Extending this would require a `PrefixOpTable`. |
| New postfix operators | Postfix operators (`.`, `()`, `[]`, `!`) are parsed in `parse_postfix`. The dot-call chain syntax (`.f()`) is a special case. |
| New AST node types | Plugins rewrite existing nodes; they cannot create new `Expr`/`Statement` variants. All custom operators produce `BinaryOp(Custom(...), ...)` or are rewritten to `Call(...)`. |
| Multi-token syntax forms | `a as Type` uses two tokens (`as` keyword + type). `a: b` uses colon. These are not single-token operators and aren't covered. |

### 2.4 Comparison: fixed cascade vs. Pratt parser

| Aspect | Fixed cascade | Pratt parser |
|--------|--------------|--------------|
| Code size | ~175 lines (10 functions) | ~80 lines (1 function + table) |
| New operator cost | Add variant + parser fn + codegen match | `Token$.Register$` call |
| Custom operators | Not supported | Full support |
| Tested behavior | Yes, well-tested for 16 operators | Needs equivalent tests |
| Readability | Explicit precedence per function | Precedence table |
| Error messages | Good (parser knows context) | Needs work (opaque token) |

### 2.5 Key design question: how does `CustomOp` interact with `logos`?

The `logos` derive macro processes tokens by priority:
1. All `#[token(...)]` entries — exact string matches
2. All `#[regex(...)]` entries — pattern matches
3. Within each group, longer matches take priority

The `CustomOp` regex `[|>&$@^~!+=/*%-]+` would match strings like `|>`, `~>`,
`<$>` — but it would also match `->` (which is a fixed token) unless `logos`
handles longest-match correctly. `logos` does handle longest-match: if `->`
is a `#[token("->")]` entry, it will match `->` rather than the regex's `-`
followed by `>`.

**But there's a trap:** the regex `[|>&$@^~!+=/*%-]+` would also match `<<=`
or `>>=` if those aren't fixed tokens. This is the desired behavior — they
fall through to `CustomOp` and can be registered via `Token$.Register$`.

**Need to verify:** that `logos` correctly handles the overlap between the
catch-all regex and fixed multi-character tokens like `->`, `:>`, `<-`, `<~`.
If `logos` tries the regex before the fixed tokens, the catch-all would
swallow everything. The typical fix is to list the regex FIRST (lowest priority)
by using `logos`'s `priority` attribute or ordering the match arms carefully.

**Alternative:** Skip the `logos` regex entirely and handle `CustomOp` in a
hand-written fallback in the lexer's main loop — if no `logos` token matches,
try the catch-all operator regex manually. This gives precise control over
ordering.

---

## 3. The `.` Chain Syntax (Future Extraction)

### 3.1 Current state

The parser has a special case at `src/parser/expressions.rs:250-260` that
converts `expr.f$(args)` into `Call("f$", [expr, ...args])`. This enables
navigation chains like `Tag$("import").First$().Before$().Insert$(...)`.

### 3.2 Why it's a candidate for extraction

The `.` chain syntax is **syntactic sugar** for function application:
`a.f(x)` means `f(a, x)`. This is a pure AST rewrite — no semantic analysis
needed. It belongs in a `$(Parsed)` plugin, not the parser.

If we add `Expr::MethodCall { receiver, name, args }` to the AST, the parser
can produce `MethodCall` nodes for ANY `a.f(...)` syntax, not just `$`-suffixed
names. Then a plugin rewrites `MethodCall` → `Call` at `$(Parsed)`.

### 3.3 What stays vs. what moves

| Layer | Stays in parser | Moves to plugin |
|-------|----------------|-----------------|
| `a.f` field access | Yes — `Expr::Field` | — |
| `a.f(x)` method call | **New:** `Expr::MethodCall` | Rewrite `MethodCall` → `Call` |
| `a, f$()` special handling | Removed | Replaced by general `MethodCall` |

### 3.4 Benefits

- The parser no longer knows about `$`-suffixed intrinsics
- Any method call (not just `$` ones) can be rewritten by plugins
- The whole `$` chain mechanism becomes a plugin concern
- The parser provides `Expr::MethodCall` as a general building block

### 3.5 Changes needed

1. Add `Expr::MethodCall { receiver: Box<Expr>, name: String, args: Vec<Expr> }`
2. In `parse_postfix`, when `.` is followed by an identifier and `(`, produce
   `MethodCall` instead of `Field` + error
3. Remove the `$`-suffixed special case (lines 250-260)
4. Write a `$(Parsed)` plugin that rewrites `MethodCall` → `Call`

**Status:** Not implemented. Reserved for future work.

---

## 4. The Operator Table Initialization

Built-in operators with their precedences (matching current cascade behavior):

| Operator | Precedence | Assoc | BinaryOpKind |
|----------|-----------|-------|-------------|
| `=` (assignment) | 10 | Right | Eq (reused) |
| `\|\|` | 20 | Left | Or |
| `&&` | 30 | Left | And |
| `==`, `!=` | 40 | Left | Eq, Neq |
| `<`, `>`, `<=`, `>=` | 50 | Left | Lt, Gt, Le, Ge |
| `\|` | 60 | Left | BitOr |
| `^` | 70 | Left | BitXor |
| `&` | 80 | Left | BitAnd |
| `<<`, `>>` | 90 | Left | Shl, Shr |
| `+`, `-` | 100 | Left | Add, Sub |
| `++` | 110 | Left | Concat |
| `*`, `/`, `%` | 120 | Left | Mul, Div, Mod |
| `as` | 130 | Left | (cast, not BinaryOp) |
| `!`, `-`, `~`, `*`, `&` (unary) | 140 | — | UnaryOp/Deref/AddrOf |
| `.`, `()`, `[]`, `!` (postfix) | 150 | — | Field/Call/Index/PluginIntercept |

Custom operators registered via `Token$.Register$` can use any precedence
0-255. The range 10-130 is the well-defined binary operator space. Values
above 140 are tighter than unary operators (custom prefix/postfix may be
added in the future).

---

## 5. Open Questions

### 5.1 `logos` catch-all interaction

Does the `CustomOp` regex `[|>&$@^~!+=/*%-]+` work correctly with `logos`
longest-match? Specifically:

- `->` is a fixed token; does `logos` match `->` or fall through to
  `[|>&$@^~!+=/*%-]+` which would match `->` as `-` + `>` (two tokens)?

- `|>` is NOT a fixed token (after removing `PipeGreater`); does `logos`
  match `|>` via the regex, or does it match `|` (Pipe) first?

- `$` alone is a fixed token; does `$>` via regex, or `$` + `>` as two tokens?

The answer depends on `logos` behavior. If `logos` always prefers longer
matches, the regex would match `$>` when `$` is only present as a single-char
token. But `$>` should be two tokens: `$` (sigil) followed by `>` (comparison).
This is a real ambiguity.

**Solution:** The catch-all regex should be for characters that are ONLY
operator characters, not characters that also have single-char meaning.
Specifically, exclude `$`, `&`, `*`, `!`, `?`, `.` from the catch-all since
they have standalone meanings. The revised regex:

```
[|>@^~+=/%<>-]+  (excluding: $ & * ! ? .)
```

Even this has overlap: `<` alone is Lt, `>` alone is Gt, `=` alone is Eq,
`-` alone is Minus. But `|=`, `|>>`, `<>`, `<=>` are not fixed tokens and
should be caught.

**Need to verify experimentally** which characters can safely be included.

### 5.2 Pipe depth semantics

The `.N|>` variants need precise specification:

```
x |> f |> g .N|> h
```

The pipeline maintains a stack of intermediate results:
- After `x |> f`: stack = `[x, f(x)]`
- After `f(x) |> g`: stack = `[x, f(x), g(f(x))]`
- `.N|>`: take result at depth N from the top of the stack
  - `.0|>` = `g(f(x))` (most recent, direct LHS)
  - `.1|>` = `f(x)`
  - `.2|>` = `x`

But the AST is a tree, not a stack. The plugin rewriting `BinaryOp(Pipe{N}, ...)`
must walk the nested `BinaryOp(Pipe, ...)` tree to find the result at depth N.

For `x |> f |> g .1|> h`:

```
BinaryOp(Pipe{ depth: 1 },
  BinaryOp(Pipe{ depth: 0 },     // The LHS of the depth-1 pipe
    BinaryOp(Pipe{ depth: 0 }, x, f),   // Inner pipe
    g
  ),
  h
)
```

The plugin walks Bottom-up:
1. Inner `Pipe{0}`: produces `Call("f", [x])` → stack at this node: `[x, f(x)]`
2. Middle `Pipe{0}`: produces `Call("g", [Call("f", [x])])` → stack: `[x, f(x), g(f(x))]`
3. Outer `Pipe{1}`: depth 1 → take `f(x)` from the stack → `Call("h", [Call("f", [x])])`

For the plugin to implement this, it needs to track the pipeline stack
recursively. This is achievable in Brief compile-time code but requires
the Level C interpreter to support recursion and `match` on AST nodes.

### 5.3 Should `|>` just be wired up?

Given that `PipeGreater` already exists as a lexer token (dead, never consumed),
the pragmatic path is to wire it up as a `BinaryOpKind::Pipe` variant in ~20
lines of parser code, without building the full extensible operator system.
This gives us `|>` immediately and the `.N|>` variants can be added later.

The full extensible operator system (Pratt parser + `CustomOp` + `Token$.Register$`)
is a larger refactor that should be planned separately. This document captures
the design for future reference.

---

## 6. `[[post]` / `[pre]]` Contract Sugar as Plugin Syntax

### 6.1 Current state

The `[[post]` and `[pre]]` shortcuts are parsed directly by the contract parser
in `src/parser/definitions.rs`. The parser knows that `[[` means
"postcondition-only" and `]]` means "precondition-only." This is hardcoded
knowledge in the parser.

```brief
// Current syntax (hardcoded in parser):
defn main() -> Int [[result > 0]
// Equivalent to: defn main() -> Int [true][result > 0]

defn main() -> Int [x > 0]]
// Equivalent to: defn main() -> Int [x > 0][true]
```

### 6.2 The extraction

Instead of the parser interpreting `[[` / `]]` as contract syntax, the parser
produces generic bracket blocks:

```rust
TopLevel::BracketBlock {
    kind: BracketKind::DoubleOpen,   // [[
    inner: Vec<Expr>,
    span: Option<Span>,
}
```

A `$(Parsed)` plugin then rewrites these into contracts:

```brief
$(Parsed) @ 200 {
    // [[post] → Contract([true], post)
    foreach(block in Tag$("bracket_block").Named$("double_open")) {
        block.ReplaceWith$(
            Contract$(Expr$(true), block.Children$().First$())
        );
    };

    // [pre]] → Contract(pre, [true])
    foreach(block in Tag$("bracket_block").Named$("double_close")) {
        block.ReplaceWith$(
            Contract$(block.Children$().First$(), Expr$(true))
        );
    };
};
```

### 6.3 What stays vs. what moves

| Construct | Stays in parser | Moves to plugin |
|-----------|----------------|-----------------|
| `[pre][post]` (both sides explicit) | Yes — `Contract` parsing | — |
| `[[post]` (double-open shortcut) | `BracketBlock(DoubleOpen, ...)` | Rewrite into `[true][post]` |
| `[pre]]` (double-close shortcut) | `BracketBlock(DoubleClose, ...)` | Rewrite into `[pre][true]` |

### 6.4 Why this matters

The contract syntax is one of the most distinctive features of Brief. Making
its sugar plugin-extensible proves that even core language ergonomics can be
moved into the plugin layer, keeping the parser minimalist and the stdlib
responsible for language feel.

---

## 7. Highlighter Plugin System (Option B)

### 7.1 The problem

When a plugin registers a new operator via `Token$.Register$`, the syntax
highlighter doesn't know about it. The user writes `|>` but it appears in
the default identifier color — no visual feedback that it's a recognized
operator.

### 7.2 The solution: grammar snippet files

Each plugin can ship a `.highlight.json` file alongside its `.bv` file.
These files contain TextMate grammar snippets that are merged into the Brief
grammar at editor startup.

**Plugin directory convention:**

```
plugins/
  parsed/
    prelude.bv
    prelude.highlight.json          (optional)
    pipeline.bv
    pipeline.highlight.json         (optional)
    contract-sugar.bv
    contract-sugar.highlight.json   (optional)
```

**File format:**

```json
{
  "name": "pipeline",
  "version": "0.1.0",
  "patterns": [
    {
      "match": "\\|>",
      "name": "keyword.operator.pipe.brief"
    },
    {
      "match": "\\.[0-9]+\\|>",
      "name": "keyword.operator.pipe.depth.brief"
    }
  ]
}
```

**contract-sugar.highlight.json:**

```json
{
  "name": "contract-sugar",
  "patterns": [
    {
      "name": "meta.contract.double-open.brief",
      "begin": "\\[\\[",
      "end": "\\]",
      "patterns": [{ "include": "#expressions" }]
    },
    {
      "name": "meta.contract.double-close.brief",
      "begin": "\\[",
      "end": "\\]\\]",
      "patterns": [{ "include": "#expressions" }]
    }
  ]
}
```

### 7.3 How it's loaded

The VSCode extension reads all `.highlight.json` files from plugin directories
at activation time and merges their patterns into the Brief grammar's
repository. This happens in `extension.ts`:

```typescript
import * as fs from 'fs';
import * as path from 'path';

function loadPluginHighlighters() {
    const pluginDirs = [
        'plugins/prelex', 'plugins/parsed', 'plugins/resolved',
        'plugins/typed', 'plugins/normalized', 'plugins/verified',
        'plugins/allocated', 'plugins/provenanced', 'plugins/generated',
        'plugins/optimized', 'plugins/linked',
    ];

    for (const dir of pluginDirs) {
        const fullPath = path.join(__dirname, '..', dir);
        if (!fs.existsSync(fullPath)) continue;

        const files = fs.readdirSync(fullPath)
            .filter(f => f.endsWith('.highlight.json'));

        for (const file of files) {
            const content = fs.readFileSync(
                path.join(fullPath, file), 'utf-8'
            );
            const highlight = JSON.parse(content);
            registerPluginPatterns(highlight.patterns);
        }
    }
}
```

Each pattern is added to the Brief grammar's `repository` so it participates
in the full highlighting pipeline (including inside contract brackets, string
interpolation, etc.).

### 7.4 Scope naming convention

Plugin highlight scopes follow a fixed naming convention for consistency:

| Pattern | Scope |
|---------|-------|
| Plugin-registered operator | `keyword.operator.plugin.<name>.brief` |
| Plugin-registered bracket | `meta.bracket.plugin.<name>.brief` |
| Plugin-registered keyword | `keyword.plugin.<name>.brief` |
| Plugin-specific constant | `constant.plugin.<name>.brief` |

Where `<name>` is the plugin's identifier (e.g., `pipe`, `contract-sugar`).

### 7.5 What ships with the compiler

The compiler ships default `.highlight.json` files alongside its built-in
plugins:

| File | Registered by | What it highlights |
|------|---------------|-------------------|
| `plugins/parsed/prelude.highlight.json` | Prelude (always active) | (none — no unique tokens) |
| `plugins/parsed/pipeline.highlight.json` | Pipeline plugin | `|>`, `.N|>` operators |
| `plugins/parsed/contract-sugar.highlight.json` | Contract sugar plugin | `[[`, `]]` bracket patterns |

These ship as empty stubs initially and are populated when the corresponding
plugins are implemented.

---

## 8. Files Referenced in This Conversation

| File | Relevance |
|------|-----------|
| `src/lexer.rs:261` | Dead `PipeGreater` token — already lexed, never consumed |
| `src/parser/expressions.rs:250-260` | `$`-suffixed field call special case in `parse_postfix` |
| `src/parser/expressions.rs:13-212` | Binary operator cascade (`parse_assignment` → `parse_factor`) |
| `src/ast/expr.rs:70-91` | `BinaryOpKind` enum — fixed set, no `Custom` variant |
| `src/macros/eval.rs` | Navigation chain evaluator — already handles `Call(name, [receiver, ...args])` |
| `src/plugin/intrinsics.rs` | `$` intrinsic dispatch — where `Token$.Register$` would go |
| `src/plugin/loader.rs` | Where `StageBlockPlugin` stores `pm_ptr` for `Stage$` operations |

---

## 9. Timeline (If Implemented)

| Phase | What | Effort |
|-------|------|--------|
| **A** | Wire up `PipeGreater` token as `BinaryOpKind::Pipe` | ~20 lines, 1 day |
| **B** | Add `.N|>` token variants and parser support | ~30 lines, 1 day |
| **C** | Write `plugins/parsed/pipeline.bv` (pipe rewrite plugin) | ~50 lines, 1 day |
| **D** | Build extensible operator system: Pratt parser + `OpTable` + `CustomOp` | ~200 lines, 3 days |
| **E** | Add `Token$.Register$` intrinsic | ~30 lines, 1 day |
| **F** | Extract `.` chain syntax: `Expr::MethodCall` + plugin | ~60 lines, 2 days |
| **G** | Remove `$`-suffixed special case from `parse_postfix` | ~10 lines removed, 0.5 days |
| **H** | Extract `[[post]` / `[pre]]` sugar: `BracketBlock` AST + plugin | ~80 lines, 2 days |
| **I** | Highlighter plugin system: `.highlight.json` loading in extension | ~60 lines, 1 day |
| **J** | Ship default `.highlight.json` stubs for built-in plugins | ~15 lines, 0.5 days |

Phases A-C are the minimum viable `|>` support. Phases D-J are the full
extensible operator and plugin-highlighting system.

**This document is a design capture. No implementation has begun.**
