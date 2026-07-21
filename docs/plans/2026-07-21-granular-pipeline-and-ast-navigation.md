# Granular Pipeline + Direct AST Navigation DSL

**2026-07-21:** Replaces the four-stage plugin system (`Front`/`Mid`/`Post`/`Back`)
with ~11 named pipeline stages.  Replaces `Collect$`/`MatchIR$` serialize-deserialize
round-trips with a direct AST navigation DSL that operates on the live tree in memory.
The `.beast` format is preserved as a programmer-facing visualization (`--emit-beast`)
but removed from the plugin data path.

---

## 1. Motivation

### 1.1 Problems With the Current System

| Problem | Manifestation | Root cause |
|---------|---------------|------------|
| **Coarse stages** | Prelude injection (source) and import insertion (AST) both use `Front` — no way to hook between them | Only 4 stages: Front/Mid/Post/Back |
| **Serialize/deserialize round-trip** | `MatchIR$("(defn main ?c ?p ?r ?b)", "...")` serializes entire AST to `.beast` text, runs pattern match, deserializes back | `Collect$`/`MatchIR$` operate on text S-expressions, not live AST |
| **No positional targeting** | Can only match + replace whole nodes; no `Before`/`After`/`Inside` semantics | No tree navigation primitives |
| **Type-blind matching** | `.beast` patterns match S-expression structure only — cannot filter by type metadata | Patterns work on serialized text, not typed AST |
| **No IR-stage text operations** | `Post`/`Back` get `on_ir(&mut String)` but no ergonomic DSL for text transformations | Only Rust `Plugin` trait methods |
| **No binary-stage hooks** | `Post`/`Back` are text-only; linking has no plugin hook | Pipeline ends at codegen output |

### 1.2 Design Goals

1. **Every compiler pass is a named hook.**  Any stage can host `$(StageName)` blocks.
2. **Direct AST navigation.**  No serialize/deserialize for plugin data flow.
3. **Precise positional targeting.**  `Before$`/`After$`/`Replace$`/`Inside$`/`AppendTo$`.
4. **Uniform operation model.**  Source text, AST, IR text, and binary each expose the same
   operation families (Select → Traverse → Position → Act), with target-appropriate semantics.
5. **Complete macro capability.**  A `PrintLn!()` call should be expandable entirely within
   a `$(Stage)` block using the DSL — no Rust plugin needed.
6. **Clean break.**  Old `Collect$`/`MatchIR$`/`InsertLiteralImport$`/`InsertRegistryImport$`
   are removed.  Old `$(Front)`/`$(Mid)`/`$(Post)`/`$(Back)` are replaced.

---

## 2. Pipeline Stages

### 2.1 Stage Enum

Replace `StageKind` (4 variants) with:

```rust
/// A named point in the compilation pipeline at which plugins can run.
/// 2026-07-21: Granular stages replacing Front/Mid/Post/Back.
/// Each stage corresponds to one compiler pass and has a default data target.
pub enum StageKind {
    /// Raw source text loaded — no lexing or parsing yet.
    /// Default target: Source$
    PreLex,

    /// After lex + parse.  AST exists but imports are NOT resolved.
    /// The prelude plugin inserts stdlib imports here.
    /// Default target: AST (implicit, no prefix)
    Parsed,

    /// All imports resolved and merged into the program AST.
    /// Default target: AST
    Resolved,

    /// Type checking complete.  Full TypeUniverse available.
    /// auto-main and entry-check plugins run here.
    /// Default target: AST
    Typed,

    /// Backend normalization applied (type annotations attached).
    /// Default target: AST
    Normalized,

    /// Protocol round-trip verification done.
    /// Default target: AST
    Verified,

    /// Allocation strategy analysis done.
    /// Default target: AST
    Allocated,

    /// Dangling pointer / provenance analysis done.
    /// Default target: AST
    Provenanced,

    /// Backend code generation complete.  IR text (`.ll`, `.mlir`, `.ts`) exists.
    /// Default target: Ir$
    Generated,

    /// Backend optimizations applied to IR text.
    /// Default target: Ir$
    Optimized,

    /// Final binary linked.
    /// Default target: Bin$
    Linked,
}
```

### 2.2 Stage → Pipeline Mapping

```
Source ──► PreLex ──► Lex ──► Parse ──► Parsed ──► Resolve ──► Resolved
            │                    │         │            │           │
         Source$               AST       AST         AST         AST
         plugin hook                    snapshot    snapshot     snapshot

──► TypeCheck ──► Typed ──► Normalize ──► Normalized ──► Verify ──► Verified
       │              │         │               │           │           │
     AST            AST       AST             AST         AST         AST
                   snapshot  snapshot         snapshot

──► AllocAnalyze ──► Allocated ──► Provenance ──► Provenanced ──► Codegen
       │                │              │                │            │
     AST              AST            AST              AST          IR

──► Generated ──► Optimize ──► Optimized ──► Link ──► Linked
       │              │              │           │        │
     Ir$            Ir$            Ir$         Bin$     Bin$
     snapshot       snapshot       snapshot
```

**BEAST snapshots** (`--emit-beast`) are emitted at every stage that has an AST.
The rule: any stage whose default target is AST (Parsed through Provenanced) gets
a `.beast.{stage}` file.  Text stages (Generated, Optimized) get `.ir.{stage}` instead.

### 2.3 Backward Compatibility

Old stage names are rejected at parse time with a clear diagnostic:

```
error: '$(Front)' is no longer a valid stage name
  ┌─ file.bv:1:1
  │
1 │ $(Front) {
  │ ^^^^^^^ use '$(Parsed)' to run after parsing
help: Use $(Parsed) for AST-stage plugins (was Front).
      Use $(PreLex) for source-text plugins (was Front source mode).
      Use $(Generated) for IR-stage plugins (was Post).
      Use $(Optimized) for final-validation plugins (was Back).
```

No transition period — this is a clean cut per user direction.

### 2.4 Plugin Discovery Paths

Old paths and their replacements:

| Old | New |
|-----|-----|
| `plugins/front/` | `plugins/parsed/` |
| `plugins/mid/` | `plugins/typed/` |
| `plugins/post/` | `plugins/generated/` |
| `plugins/back/` | `plugins/optimized/` |

A plugin can target any stage by declaring `$(StageName)` as its top-level block.
The directory is merely organizational.  A plugin in `plugins/parsed/` can still
contain `$(Typed)` blocks if it also needs to run at the Typed stage.

### 2.5 Target Config Changes

In `config/targets.toml`, per-extension plugin lists now reference new stage
directory names:

```toml
[".bv"]
plugins = ["prelude"]           # discovered from plugins/parsed/prelude.bv

[".cbv"]
plugins = ["prelude-hw"]        # discovered from plugins/parsed/prelude-hw.bv
```

The `plugins` key is treated as a filter over discovered plugins — any plugin
not in the list is skipped for that extension.  A plugin in `plugins/typed/`
is still discovered and available; it just won't run for `.bv` files unless
listed in the extension's `plugins` array.

---

## 3. The Four Targets

Every operation chain targets one of four data surfaces:

| Target | Type | Available at stages | Lifetime |
|--------|------|---------------------|----------|
| `Source$` | `String` (source text) | All stages | Read-only after `$(PreLex)` |
| AST (implicit) | `Vec<TopLevel>` | `$(Parsed)`–`$(Provenanced)` | Mutable through all AST stages |
| `Ir$` | `String` (emitted code) | `$(Generated)`–`$(Optimized)` | Mutable |
| `Bin$` | `PathBuf` (binary path) | `$(Linked)` | Read-only at plugin time |

### 3.1 Default Target Per Stage

| Stage | Implicit target (no prefix) |
|-------|----------------------------|
| `$(PreLex)` | `Source$` |
| `$(Parsed)` | AST |
| `$(Resolved)` | AST |
| `$(Typed)` | AST |
| `$(Normalized)` | AST |
| `$(Verified)` | AST |
| `$(Allocated)` | AST |
| `$(Provenanced)` | AST |
| `$(Generated)` | `Ir$` |
| `$(Optimized)` | `Ir$` |
| `$(Linked)` | `Bin$` |

**Explicit override is always possible.**  Writing `Source$.ReplaceAll$("old", "new")`
at `$(Typed)` works (though Source$ is read-only after PreLex — see error handling).

### 3.2 Source$ Semantics

`Source$` is the original `.bv` file text.  At `$(PreLex)` it is mutable; source
text modifications are fed into the lexer.  After `$(PreLex)` completes, the
source text is frozen.  Write operations on `Source$` at later stages produce
warnings and are ignored.

The purpose of keeping `Source$` accessible at later stages is read-only:
`Source$.Find$("// AUTHOR:")` at `$(Parsed)` lets a plugin inspect the original
source for annotations without needing to reconstruct it from AST spans.

### 3.3 Ir$ Semantics

`Ir$` is the emitted backend IR as a string.  It becomes available immediately
after codegen completes (the `$(Generated)` stage).  All text operations
(`ReplaceAll$`, `InsertBefore$`, `Prepend$`, `Append$`, `Find$`) work on the
IR text.  At `$(Optimized)`, the IR text is the optimizer output.

### 3.4 Bin$ Semantics

`Bin$` is a `PathBuf` pointing to the compiled binary.  It supports only:

| Operation | Description |
|-----------|-------------|
| `Bin$.Run$("command {{path}} [...]")` | Run external tool on the binary. `{{path}}` is replaced with the binary path. |

No text operations make sense on a binary, so `Bin$.ReplaceAll$`, `Bin$.Insert$`,
etc. produce errors with explanation.

---

## 4. Core Navigation DSL

### 4.1 The Three-Link Chain

Every transformation follows the pattern:

```
SELECT ──► TRAVERSE ──► POSITION ──► ACT
```

Each link is a `$`-suffixed intrinsic.  The chain begins with a selector on the
default target (or an explicit `Source$`/`Ir$`/`Bin$` prefix) and ends with an
action.

```
Tag$("import") .First$() .Before$() .Insert$(Import$("std/x.bv"))
 └─SELECT──┘  └TRAVERSE┘ └POSITION┘ └───────────ACT────────────┘
```

A chain without an action is a read query — `Tag$("defn").Count$()` returns
the count.  A chain without a selector is an error (except at text stages where
`Find$` is the implicit selector).

### 4.2 Selectors (the "what")

These produce a `Selection`.  They are valid on AST targets only.
For text targets (`Source$`, `Ir$`), use `Find$` instead (see §5).

| Intrinsic | Returns | Description | Example |
|-----------|---------|-------------|---------|
| `All$()` | Selection | Every top-level AST node | `All$()` |
| `Tag$(name)` | Selection | Nodes with matching S-expression tag | `Tag$("defn")`, `Tag$("call")` |
| `Pattern$(sexpr)` | Selection | Nodes matching `.beast` pattern with `?` vars | `Pattern$("(call PrintInt# ?*)")` |
| `Named$(name)` | Selection | Nodes whose name field equals `name` | `Named$("main")` |
| `WithKey$(key)` | Selection | Nodes having metadata key `key` | `WithKey$("inline")` |
| `WithAttr$(key, val)` | Selection | Nodes with metadata `key` = `val` | `WithAttr$("mutable", true)` |
| `Index$(n)` | Selection | Nth child of the parent context | `Index$(0)` |

**Implementation note:** `Pattern$` compiles the `.beast` pattern string into a
live AST matcher — it walks `Vec<TopLevel>` directly, not via serialization.
The pattern engine lives in a new `src/macros/pattern_live.rs` module.

### 4.3 Selector Combinators

Applied to an existing selection:

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.And$(sel)` | Selection | Intersection with another selector |
| `.Or$(sel)` | Selection | Union with another selector |
| `.Not$(sel)` | Selection | Complement within current selection |

Example:
```brief
// Find all defn nodes NOT named "main"
Tag$("defn").Not$(Named$("main"));
```

### 4.4 Traversal (narrowing, expanding, navigating)

Applied to a selection.  Returns a new selection.

**Positional narrowing:**

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.First$(n?)` | Selection | First N elements (default 1) |
| `.Last$(n?)` | Selection | Last N elements (default 1) |
| `.Nth$(n)` | Selection | Nth element (0-indexed) |

**Tree navigation:**

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Children$(sel?)` | Selection | Direct children of each selected (optionally filtered) |
| `.Descendants$(sel?)` | Selection | All descendants (optionally filtered) |
| `.Parent$()` | Selection | Parent nodes of each selected |
| `.Ancestors$(sel)` | Selection | Ancestors matching selector |
| `.Closest$(sel)` | Selection | Nearest ancestor matching selector |

**Sibling navigation:**

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Next$(sel?)` | Selection | Following siblings of each selected (filtered) |
| `.Prev$(sel?)` | Selection | Preceding siblings of each selected (filtered) |

Examples:
```brief
// First parameter of defn main
Tag$("defn").Named$("main").Children$("param").First$();

// All calls within reactive transactions
Tag$("txn").WithAttr$("reactive", true).Descendants$(Tag$("call"));
```

### 4.5 Selection Introspection

These turn a selection into a value:

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Count$()` | Int | Number of nodes in selection |
| `.IsEmpty$()` | Bool | True if selection has zero nodes |
| `.Names$()` | List[String] | Name fields of selected nodes |

Examples:
```brief
Let$count = Tag$("import").Count$();
If$($count == 0) { EmitWarning$("no imports!"); };
```

### 4.6 Positions (where to act)

A position is an ephemeral cursor into the tree, valid for exactly one action.
Positions are created by traversing a selection.

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Before$()` | Position | Cursor *before* each selected node (insert here) |
| `.After$()` | Position | Cursor *after* each selected node |
| `.Replace$()` | Position | Each selected node *will be replaced* |
| `.Inside$()` | Position | First child of each selected (prepend) |
| `.AppendTo$()` | Position | Last child of each selected (append) |

A position is consumed by the next action call.  Using a consumed position
produces a runtime panic (caught during plugin evaluation, reported as warning).

### 4.7 Actions (what to do)

Actions are the terminal link in the chain.  They modify the AST, text, or
selection state.

**AST actions (valid in `$(Parsed)`–`$(Provenanced)`):**

| Intrinsic | Description |
|-----------|-------------|
| `.Insert$(node...)` | Insert constructed AST node(s) at the position |
| `.Delete$()` | Remove selected nodes from the AST |
| `.ReplaceWith$(node)` | Substitute selected nodes with constructed node |
| `.Set$(key, val)` | Set metadata property on selected nodes |
| `.Wrap$(tag)` | Enclose each selected node in a container of tag |
| `.Rename$(name)` | Rename the name field of selected nodes |

**Text actions (valid on `Source$` and `Ir$`):**

| Intrinsic | Description |
|-----------|-------------|
| `.Insert$(text)` | Insert text at position |
| `.Delete$()` | Delete selected text range |
| `.ReplaceWith$(text)` | Substitute selected text |

**Binary actions (valid on `Bin$`):**

| Intrinsic | Description |
|-----------|-------------|
| `.Run$(cmd)` | Run external command with `{{path}}` substitution |

### 4.8 Complete Chain Examples

```brief
// Find the first import, insert prelude imports before it
Tag$("import").First$().Before$().Insert$(
    Import$("std/types/bootstrap.bv"),
    Import$("std/os/fs.bv"),
);

// Navigate into defn main's contract, set entry flag
Tag$("defn").Named$("main").First$()
    .Descendants$("contract").First$()
    .Set$("entry", true);

// Delete all metadata with key "debug"
Tag$("metadata").WithKey$("debug").Delete$();

// Wrap every call to PrintInt# in a sync block
Tag$("call").Named$("PrintInt#")
    .Wrap$("sync");

// Rename all occurrences of "deprecated_func" to "new_func"
Tag$("call").Named$("deprecated_func")
    .Rename$("new_func");
```

---

## 5. Text Target Operations (Source$, Ir$)

Text targets use a parallel operation family optimized for string manipulation.
Replace `Tag$`/`Named$`/etc. with `Find$`.

### 5.1 Text Selectors

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `Find$(pattern)` | TextSelection | Lines/regions matching regex or literal |

`Find$` returns a `TextSelection` — a set of `(start, end)` byte offsets
into the text buffer.

### 5.2 Text Combinators

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Not$(pattern)` | TextSelection | Complement of matches within parent |
| `.And$(pattern)` | TextSelection | Intersection of two patterns |
| `.Or$(pattern)` | TextSelection | Union of two patterns |

### 5.3 Text Traversal

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.First$(n?)` | TextSelection | First N matches |
| `.Last$(n?)` | TextSelection | Last N matches |
| `.Nth$(n)` | TextSelection | Nth match |

(No tree traversal — text is flat.)

### 5.4 Text Positions

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Before$()` | TextPosition | Offset before each matched region |
| `.After$()` | TextPosition | Offset after each matched region |
| `.Replace$()` | TextPosition | Each matched region will be replaced |

### 5.5 Text Actions

| Intrinsic | Description |
|-----------|-------------|
| `.Insert$(text)` | Insert string at position |
| `.Delete$()` | Delete matched regions |
| `.ReplaceWith$(text)` | Substitute matched regions |
| `.Prepend$(text)` | Prepend to the entire buffer (no selection needed) |
| `.Append$(text)` | Append to the entire buffer |

### 5.6 Text Introspection

| Intrinsic | Returns | Description |
|-----------|---------|-------------|
| `.Count$()` | Int | Number of matches |
| `.IsEmpty$()` | Bool | No matches |
| `.Lines$()` | List[Int] | Line numbers of matches |

### 5.7 Text Examples

```brief
$(PreLex) {
    // Source$ is the default target at PreLex
    Find$("#define DEBUG").ReplaceWith$("// #define DEBUG");
    Prepend$("// Auto-generated from Brief\n");
};

$(Generated) {
    // Ir$ is the default target at Generated
    Find$("target triple = \"x86_64\"").ReplaceWith$("target triple = \"arm64\"");
    
    // Still can access source for annotation cross-referencing
    Source$.Find$("// BENCHMARK").Count$();
};
```

---

## 6. Binary Target Operations (Bin$)

The binary target supports only `Run$`:

```brief
$(Linked) {
    Bin$.Run$("strip --strip-unnecessary {{path}}");
    Bin$.Run$("objcopy --only-keep-debug {{path}} {{path}}.debug");
};
```

`{{path}}` is replaced with the binary path.  Additional `{{variable}}`
substitutions can be defined by the plugin — for example, `{{out_dir}}`
for the output directory, `{{file_name}}` for the source filename stem.

---

## 7. Flow Control

### 7.1 Let$ Binding

Binds a selection, position, or value to a name:

```brief
Let$imports = Tag$("import");
Let$first_import = $imports.First$();
```

`Let$` names are prefixed with `$` when referenced:

```brief
$imports.Count$();
$first_import.Before$().Insert$(Import$("std/x.bv"));
```

Scope is the enclosing `$(Stage)` block.  No shadowing — redefining a `$name`
in the same block is an error.

### 7.2 ForEach$ Iteration

Iterates over a selection, binding the current element to `$`:

```brief
ForEach$(Tag$("import")) {
    $.After$().Insert$(Import$("std/debug.bv"));
};
```

For nested iteration, use explicit naming:

```brief
ForEach$(Tag$("defn") as $defn) {
    ForEach$($defn.Descendants$(Tag$("call")) as $call) {
        EmitWarning$("call inside " + $defn.Names$().First$() + ": " + $call.Names$().First$());
    };
};
```

In the body:
- `$` refers to the current element (only available when no `as $name` clause)
- `$name` refers to the named binding (available when `as $name` is used)
- All navigation intrinsics are available on the binding

### 7.3 If$ Conditional

Evaluates a boolean expression and conditionally executes the body:

```brief
If$(Tag$("defn").Named$("main").Count$() == 0) {
    EmitWarning$("no main entry point — program may not start");
};
```

The condition must evaluate to a `Bool`.  Supported operators:
- `==`, `!=` (equality)
- `>`, `<`, `>=`, `<=` (numeric comparison)
- `&&`, `||`, `!` (boolean)
- Method calls on selections (`.Count$()`, `.IsEmpty$()`, `.Names$()`)

### 7.4 Blocks and Sequencing

A `$(Stage)` block body is a sequence of statements.  Each statement is
one of:
- A navigation chain (select → traverse → position → act)
- A `Let$` binding
- A `ForEach$` loop
- An `If$` conditional
- A standalone intrinsic call (`EmitWarning$`, `EmitError$`)

Statements are executed in order.  A navigation chain without a terminal
action is treated as a read query and its result is discarded (unless bound
with `Let$`).

```brief
$(Parsed) {
    // Statement 1: bind
    Let$target = Tag$("import").First$();
    
    // Statement 2: mutate
    $target.Before$().Insert$(Import$("std/prelude.bv"));
    
    // Statement 3: read + conditional
    If$(Tag$("import").Count$() == 0) {
        EmitWarning$("no imports in file");
    };
};
```

---

## 8. AST Construction Primitives

Inside `$(Stage)` blocks, these `PascalCase$` intrinsics construct AST nodes.
They are the building blocks passed to `.Insert$()` and `.ReplaceWith$()`.

### 8.1 Top-Level Constructors

| Builder | Produces | Example |
|---------|----------|---------|
| `Import$(path, symbols?)` | `TopLevel::Import` | `Import$("std/io.bv")` |
| `Defn$(name, params, ret, body, contract?)` | `TopLevel::Definition` | `Defn$("main", [], Type$("Int"), Block$(...))` |
| `Txn$(name, params, contract, body, flags?)` | `TopLevel::Transaction` | `Txn$("work", [], contract, body, [:reactive])` |

### 8.2 Statement Constructors

| Builder | Produces | Example |
|---------|----------|---------|
| `Block$(stmts...)` | `Vec<Statement>` | `Block$(Let$(...), Assign$(...))` |
| `Let$(name, ty?, expr?, mods?)` | `Statement::Let` | `Let$("x", Type$("Int"), Expr$(42))` |
| `Assign$(target, expr)` | `Statement::Assign` | `Assign$(Ident$("x"), Expr$(5))` |
| `Term$(expr?)` | `Statement::Term` | `Term$(Ident$("result"))` |
| `Guarded$(condition, body)` | `Statement::Guarded` | `Guarded$(Expr$(true), Block$(...))` |

### 8.3 Expression Constructors

| Builder | Produces | Example |
|---------|----------|---------|
| `Call$(fn, args...)` | `Expr::Call` | `Call$("PrintInt#", Ident$("x"))` |
| `Ident$(name)` | `Expr::Identifier` | `Ident$("x")` |
| `Expr$(lit)` | Literal expression | `Expr$(42)`, `Expr$("hello")` |
| `BinOp$(kind, lhs, rhs)` | `Expr::BinaryOp` | `BinOp$("Add", Ident$("a"), Ident$("b"))` |
| `Field$(obj, name)` | `Expr::Field` | `Field$(Ident$("list"), "size")` |
| `BlockExpr$(stmts)` | `Expr::Block` | `BlockExpr$(Block$(...))` |
| `Contract$(pre, post, entry?)` | `Contract` | `Contract$(Expr$(true), Expr$(true), true)` |

### 8.4 Type Constructors

| Builder | Produces | Example |
|---------|----------|---------|
| `Type$(name)` | `Type::Custom` | `Type$("Int")` |
| `Bits$(n)` | `Type::Bits` | `Bits$(64)` |
| `Ptr$(inner)` | `Type::Ptr` | `Ptr$(Type$("Int"))` |
| `Tuple$(types...)` | `Type::Tuple` | `Tuple$(Type$("Int"), Type$("Bool"))` |

### 8.5 Metadata Constructors

| Builder | Produces | Example |
|---------|----------|---------|
| `Metadata$(key, val)` | `(String, PropertyValue)` | `Metadata$("mutable", true)` |

### 8.6 Pattern Constructor

| Builder | Produces | Example |
|---------|----------|---------|
| `Pattern$(sexpr)` | Compiled `Pattern` | `Pattern$("(call PrintInt# ?*)")` |

Used for querying the AST with `.beast`-style variables:

```brief
ForEach$(Pattern$("(call ?fn ?arg)") as $match) {
    EmitWarning$("found call to " + $match.Bound$("fn"));
};
```

When iterating over pattern matches, `$match.Bound$(name)` retrieves the
value bound to `?name` in the pattern.

---

## 9. Complete Plugin Rewrites

### 9.1 prelude.bv (was `plugins/front/prelude.bv`)

```brief
// 2026-07-21: Insert stdlib imports before the first user import.
// Runs at Parsed stage — imports are not yet resolved, so we insert
// before the first import statement found in the source AST.

$(Parsed) @ highest {
    Let$anchor = Tag$("import").First$();
    $anchor.Before$().Insert$(
        Import$("std/types/bootstrap.bv"),
        Import$("std/os/fs.bv"),
        Import$("std/os/net.bv"),
        Import$("std/os/signal.bv"),
        Import$("std/os/ipc.bv"),
        Import$("std/os/thread.bv"),
        Import$("std/os/user.bv"),
        Import$("std/os/mem.bv"),
        Import$("std/os/atomic.bv"),
        Import$("std/core/ptr.bv"),
        Import$("std/core/string_builder.bv"),
        Import$("std/env.bv"),
        Import$("std/io.bv")
    );
};
```

### 9.2 prelude-hw.bv (was `plugins/front/prelude-hw.bv`)

```brief
$(Parsed) @ highest {
    Let$anchor = Tag$("import").First$();
    $anchor.Before$().Insert$(
        Import$("std/types/bootstrap.bv"),
        Import$("std/hardware.bv")
    );
};
```

### 9.3 auto-main.bv (was `plugins/mid/auto-main.bv`)

```brief
// 2026-07-21: Set [#] entry marker on defn main / txn main.
// Uses direct AST navigation instead of MatchIR$ serialize/deserialize.

$(Typed) @ highest {
    Tag$("defn").Named$("main").First$()
        .Descendants$("contract").First$()
        .Set$("entry", true);
    Tag$("txn").Named$("main").First$()
        .Descendants$("contract").First$()
        .Set$("entry", true);
};
```

### 9.4 entry-check.bv (was `plugins/mid/entry-check.bv`)

```brief
// 2026-07-21: Verify program has at least one entry mechanism.
// Was previously using Collect$ + CheckReactive$ at Mid stage.

$(Typed) {
    Let$has_entry = Tag$("contract").WithAttr$("entry", true).Count$();
    Let$has_trg = Tag$("trigger").Count$();
    If$($has_entry == 0 && $has_trg == 0) {
        EmitError$("no entry point: add [#] to defn main or trg declaration");
    };
};
```

### 9.5 validate-trg.bv (was `plugins/post/validate-trg.bv`)

```brief
// 2026-07-21: Validate dynamic trigger targets after protocol verification.

$(Verified) {
    ForEach$(Tag$("trigger")) {
        Let$has_target = $.Descendants$(Tag$("ident")).First$().IsEmpty$();
        If$($has_target) {
            EmitWarning$("trigger with no target instance: " + $.Names$().First$());
        };
    };
};
```

### 9.6 PrintLn! Expansion (new — demonstrates completeness)

```brief
// 2026-07-21: Expand !PrintLn(x) into
//   { call PrintString$("\n"); call PrintInt#(x); }

$(Parsed) {
    ForEach$(Tag$("plugin_intercept").Named$("PrintLn") as $intercept) {
        Let$args = $intercept.Children$();
        $intercept.ReplaceWith$(Block$(
            Call$("PrintString#", Expr$("\n")),
            Call$("PrintInt#", $args.Nth$(0))
        ));
    };
};
```

---

## 10. Error Handling Policy

| Situation | Severity | Message |
|-----------|----------|---------|
| Read op on empty selection | No-op | (silent) |
| Write op on empty selection | Warning | "Selection was empty at `<chain>` — no action taken" |
| Tree op on text target (Source$) | Warning | "`Tag$` is not available on Source$ — use `Find$()` for text operations" |
| Text op on AST target | Warning | "`Find$` is not available on AST — use `Tag$()` for tree operations" |
| Write op on frozen Source$ | Warning | "Source$ is frozen after PreLex stage — modification ignored" |
| Empty position consumed twice | Panic → Warning | "Position already consumed at `<chain>` — this is a bug in your plugin" |
| Undefined `$name` in Let$ | Warning | "Undefined binding `$name`" |
| AST constructor in wrong stage | Warning | "`Import$` is not available at $(Generated) stage — AST constructors require an AST target" |
| Bin$.Run$ non-zero exit | Warning | "`Run$(\"<cmd>\")` exited with code <N>: <stderr>" |

The principle: **warnings never abort compilation** (distinct from `EmitError$`
which aborts).  A plugin with warnings still produces a binary.  This lets
users iterate on plugin logic without killing the build.

---

## 11. BEAST Snapshot Changes (`--emit-beast`)

### 11.1 New Snapshot Stages

| Flag | File | Contents |
|------|------|----------|
| `--emit-beast parse` | `file.beast.parse` | AST after parse + PreLex plugin |
| `--emit-beast resolve` | `file.beast.resolve` | AST after import resolution |
| `--emit-beast type-check` | `file.beast.types` | AST after type checking |
| `--emit-beast normalize` | `file.beast.normal` | AST after normalization |
| `--emit-beast verify` | `file.beast.verify` | AST after protocol verification |
| `--emit-beast alloc` | `file.beast.alloc` | AST after allocation analysis |
| `--emit-beast provenance` | `file.beast.prov` | AST after provenance check |
| `--emit-beast codegen` | `file.ir.generated` | Emitted IR text |
| `--emit-beast optimize` | `file.ir.opt` | Optimized IR text |

### 11.2 CLI Changes

```bash
# Old (removed):
brief build program.bv --emit-beast ast
brief build program.bv --emit-beast mid
brief build program.bv --emit-beast post

# New:
brief build program.bv --emit-beast parse
brief build program.bv --emit-beast type-check
brief build program.bv --emit-beast all           # all stages
```

The `BeastStage` enum maps to the granular stages:

```rust
pub enum BeastStage {
    Parse,
    Resolve,
    TypeCheck,
    Normalize,
    Verify,
    Alloc,
    Provenance,
    Codegen,
    Optimize,
}

impl BeastStage {
    /// Stages that have AST data (get .beast files).
    pub fn has_ast(&self) -> bool {
        matches!(self, Parse | Resolve | TypeCheck | Normalize | Verify | Alloc | Provenance)
    }
    /// Stages that have IR text data (get .ir files).
    pub fn has_ir(&self) -> bool {
        matches!(self, Codegen | Optimize)
    }
}
```

### 11.3 .beast File Format (Unchanged)

The `.beast` S-expression format stays the same.  It remains a lossless
serialization of `Vec<TopLevel>` + `TypeUniverse` for programmer
visualization.  The only change is that plugins no longer read `.beast`
text — they read the live AST.  The `.beast` output is purely for human
consumption (`--emit-beast`) and for debugging plugin behavior.

---

## 12. Highlighter Changes

### 12.1 brief.tmLanguage.json

**Remove** old stage tokens:
- `$(Front)`, `$(Mid)`, `$(Post)`, `$(Back)`
- `InsertLiteralImport$`, `InsertRegistryImport$`, `Collect$`, `MatchIR$`, `CheckReactive$`

**Add** new stage tokens:
```jsonc
// Stage blocks
{ "match": "\\$\\((PreLex|Parsed|Resolved|Typed|Normalized|Verified|Allocated|Provenanced|Generated|Optimized|Linked)\\)",
  "name": "entity.name.function.stage-block.brief" }

// AST navigation intrinsics (PascalCase + $)
{ "match": "\\b(Import|Defn|Txn|Contract|Block|Let|Assign|Term|Guarded|Call|Ident|Expr|BinOp|Field|BlockExpr|Bits|Ptr|Tuple|Metadata|Pattern)\\$",
  "name": "support.function.constructor.brief" }

// Navigation intrinsics (mixedCase + $)
{ "match": "\\b(All|Tag|Named|WithKey|WithAttr|And|Or|Not|First|Last|Nth|Children|Descendants|Parent|Ancestors|Closest|Next|Prev|Before|After|Replace|Inside|AppendTo|Insert|Delete|ReplaceWith|Set|Wrap|Rename|Count|IsEmpty|Names|Find|Prepend|Append|Lines|Run|ForEach|Bound)\\$",
  "name": "support.function.navigation.brief" }

// Flow control
{ "match": "\\b(Let|If)\\$",
  "name": "keyword.control.stage.brief" }

// Source$, Ir$, Bin$ targets
{ "match": "\\b(Source|Ir|Bin)\\$",
  "name": "variable.language.target.brief" }

// EmitWarning$, EmitError$
{ "match": "\\b(EmitWarning|EmitError)\\$",
  "name": "keyword.other.diagnostic.brief" }
```

### 12.2 New .beast Grammar

Create `syntax-highlighter/syntaxes/beast.tmLanguage.json` with `source.beast`
scope.  Full highlighting for:

| Token class | Example | Scope |
|-------------|---------|-------|
| Structural tags | `defn`, `txn`, `universe`, `typedef`, `contract`, `state`, `trigger`, `constant` | `entity.name.type.beast` |
| Expression tags | `call`, `ident`, `binop`, `unop`, `field`, `index`, `cast`, `deref`, `addrof` | `support.function.beast` |
| Statement tags | `assign`, `let`, `term`, `term!`, `return`, `guarded`, `if`, `block`, `foreach` | `keyword.control.beast` |
| Attribute tags | `metadata`, `entry`, `pre`, `post`, `bytes`, `alignment`, `properties`, `params`, `body` | `entity.other.attribute-name.beast` |
| Strings | `"...\"` | `string.quoted.double.beast` |
| Numbers | `42`, `3.14` | `constant.numeric.beast` |
| Booleans | `true`, `false` | `constant.language.beast` |
| Comments | `; comment` | `comment.line.semicolon.beast` |
| Pattern variables | `?x`, `?*`, `??*` | `variable.parameter.beast` |

### 12.3 package.json Additions

```jsonc
"contributes": {
  "languages": [
    {
      "id": "beast",
      "aliases": ["BEAST", "beast"],
      "extensions": [".beast"],
      "configuration": "./language-configuration.json",
      "icon": "images/beast-icon.svg"
    }
  ],
  "grammars": [
    {
      "language": "beast",
      "scopeName": "source.beast",
      "path": "./syntaxes/beast.tmLanguage.json"
    }
  ]
}
```

Copy `assets/beast-icon.svg` to `syntax-highlighter/images/beast-icon.svg`.

---

## 13. Implementation Phases

### Phase A: Pipeline Expansion (1-2 days)

Files: `src/ast/top.rs`, `src/compile.rs`, `src/plugin/mod.rs`, `src/plugin/loader.rs`

1. Expand `StageKind` enum from 4 → 11 variants.
2. Add `run_prelex_source`, `run_parsed_ast`, `run_resolved_ast`, ..., `run_linked_bin`
   methods to `PluginManager`.
3. Wire stage hooks into `compile.rs` at each compiler pass.
4. Update `BeastStage` enum and `emit_beast_snapshot` for granular snapshots.
5. Update `config/targets.toml` plugin directory references.

**DRY rule:** Create a helper `fn run_stage_ast(program, universe, stage, pm)` that
filters plugins by `StageKind` and calls `on_ast`.  Do not copy-paste the filter loop
for each stage.

**Flat control flow:**
```rust
// Instead of nested ifs:
for entry in pm.active_plugins() {
    if !entry.plugin.stages().contains(&stage) { continue; }
    entry.plugin.on_ast(program, universe)?;
}
```

### Phase B: Selection Engine (2-3 days)

New module: `src/macros/selection.rs`

Types:
```rust
/// A set of AST node references identified by (parent_id, child_index) pairs.
/// References are stable across the stage (nodes are identified by their
/// position in Vec<TopLevel> or in the parent's children Vec, not by pointer).
struct Selection {
    nodes: Vec<NodeRef>,
}

enum NodeRef {
    TopLevel(usize),           // index into Vec<TopLevel>
    Statement { parent: Box<NodeRef>, index: usize },
    Expr { parent: Box<NodeRef>, index: usize },
    // etc. for each AST node type that can contain children
}

trait Selector {
    fn apply(&self, items: &[TopLevel]) -> Result<Selection, String>;
}
```

Implement concrete selectors:
- `TagSelector`, `NamedSelector`, `WithKeySelector`, `WithAttrSelector`
- `PatternSelector` — compiles `.beast` pattern string, walks live AST
- `AllSelector`, `IndexSelector`

Implement traversal on `Selection`:
- `first(n)`, `last(n)`, `nth(n)` — subset filtering
- `children(filter)`, `descendants(filter)` — tree descent
- `parent()`, `ancestors(filter)`, `closest(filter)` — tree ascent
- `next(filter)`, `prev(filter)` — sibling navigation

**DRY rule:** Tree walking (children, descendants) uses a shared `fn walk_nodes`
that accepts a closure.  Do not write separate walk implementations for children
vs descendants.

**Max 2 nesting levels:** The tree walker extracts a helper `fn visit_node`
that handles dispatch on `TopLevel`/`Statement`/`Expr` variants.  Dispatch
is a flat match with early returns, not nested `if let` chains.

### Phase C: Pattern Engine for Live AST (2-3 days)

New module: `src/macros/pattern_live.rs`

Port `src/beast/pattern.rs` from S-expression matching to live AST matching.

The pattern syntax (`(call PrintInt# ?*)`) is identical.  The internal
representation changes: instead of matching on `SExpr`, the pattern compiler
produces matchers that walk `Expr`, `Statement`, `TopLevel` directly.

```rust
enum LivePattern {
    /// Match a specific AST tag
    Tag(String),
    /// Match any single node (wildcard)
    Wildcard,
    /// Match rest children (rest wildcard)
    WildcardRest,
    /// Bind matched node to variable name
    Var(String),
    /// Bind rest children to variable name
    VarRest(String),
    /// Match list structure: tag + children
    List {
        tag: Option<String>,        // None = wildcard tag
        children: Vec<LivePattern>,
    },
}
```

Key functions:
- `fn collect_matches_live(pattern, items) -> Vec<HashMap<String, ASTNode>>`
- `fn replace_all_live(pattern, replacement, items) -> (Vec<TopLevel>, u32)`

These operate on `Vec<TopLevel>` directly.  No serialization.

**Fallback:** If the live pattern compiler encounters a pattern it cannot match
(e.g., a deeply nested structural pattern that would require re-serialization),
it falls back to the existing `to_beast` → `pattern::replace_all` → `from_beast`
path with a warning: "Live pattern engine cannot match `<pattern>` — falling
back to serialize/deserialize."

### Phase D: Actions + Positions (2-3 days)

New module: `src/macros/actions.rs`

Types:
```rust
enum Position {
    Before(NodeRef),
    After(NodeRef),
    Replace(NodeRef),
    Inside(NodeRef),
    AppendTo(NodeRef),
}
```

Actions mutate the live AST:
- `insert(position, nodes)`: Splice into `Vec` at computed index
- `delete(selection)`: Remove nodes from parent Vec
- `replace_with(selection, node)`: Substitute
- `set_metadata(selection, key, value)`: Write into `HashMap`
- `wrap(selection, tag)`: Create wrapper node, move children in
- `rename(selection, name)`: Update name field

**DRY rule:** All position-based mutations (insert/delete/replace) share a
`fn modify_children(parent_ref, index_range, action)` helper that resolves
the parent Vec and applies the modification.  Do not repeat the Vec-splicing
logic for each action.

**No nested arrow code:**
```rust
fn insert(target: &mut Vec<TopLevel>, pos: Position, nodes: Vec<TopLevel>) -> Result<(), String> {
    let idx = match pos {
        Position::Before(ref node) => node.index(),
        Position::After(ref node) => node.index() + 1,
        Position::Replace(ref node) => { target.remove(node.index()); node.index() }
        // ...
    };
    for (i, node) in nodes.into_iter().enumerate() {
        target.insert(idx + i, node);
    }
    Ok(())
}
```

Max 2 levels: the match on `pos` assigns `idx`, then the loop is flat.

### Phase E: Text + Binary Operations (1 day)

New module: `src/macros/text_ops.rs`

Implement `TextSelection` and text operations:

```rust
struct TextSelection { ranges: Vec<(usize, usize)> }
impl TextSelection {
    fn find(text: &str, pattern: &str) -> Self;
    fn first(self, n: usize) -> Self;
    fn replace_with(self, text: &mut String, replacement: &str);
    fn insert(self, text: &mut String, content: &str);
    fn delete(self, text: &mut String);
}
```

**DRY rule:** The find/match engine is shared between `Source$` and `Ir$`.
A single `RegexEngine` or `PatternEngine` struct handles both, parametrized
by the text buffer reference.

Binary is minimal: `BinRun { cmd: String }` with `{{path}}` substitution.

### Phase F: Parser Changes (1-2 days)

File: `src/parser/definitions.rs`

1. Add parsing for new stage names in `$(StageName)` blocks.
2. Reject old names with clear diagnostic (see §2.3).
3. Parse `Let$name = expr;` as flow control.
4. Parse `ForEach$(sel) { ... }` and `ForEach$(sel as $name) { ... }`.
5. Parse `If$(cond) { ... }`.
6. Parse navigation chains as expression statements.
7. Parse AST constructor calls (`Import$`, `Defn$`, `Call$`, etc.).

Add a new AST node type for the navigation chain:

```rust
enum StageStatement {
    Chain(Vec<StageCall>),       // Select → Traverse → Position → Act
    Binding(String, StageExpr),
    ForEach { selection: StageExpr, binding: Option<String>, body: Vec<StageStatement> },
    Conditional(StageExpr, Vec<StageStatement>),
}
```

### Phase G: Remove Old Intrinsics (1 day)

File: `src/plugin/intrinsics.rs`

Remove:
- `InsertLiteralImport$`
- `InsertRegistryImport$`
- `Collect$`
- `MatchIR$`
- `CheckReactive$`

Replace with:
- Navigation chain evaluation (`evaluate_chain`)
- Flow control evaluation (`evaluate_foreach`, `evaluate_if`, `evaluate_let`)
- AST constructor evaluation (`evaluate_builder`)

The new `dispatch_intrinsic` match arm:
```rust
match name {
    // Retained diagnostics
    "EmitWarning$" => intrinsic_emit_warning(args),
    "EmitError$" => intrinsic_emit_error(args),
    // Everything else goes through the navigation engine
    _ if is_navigation_intrinsic(name) => evaluate_chain(name, args, program, universe),
    _ if is_builder_intrinsic(name) => evaluate_builder(name, args),
    _ => Err(format!("unknown $ intrinsic '{name}'")),
}
```

### Phase H: Plugin Migration (1 day)

1. Move `plugins/front/prelude.bv` → `plugins/parsed/prelude.bv` (rewrite content)
2. Move `plugins/front/prelude-hw.bv` → `plugins/parsed/prelude-hw.bv` (rewrite)
3. Move `plugins/mid/auto-main.bv` → `plugins/typed/auto-main.bv` (rewrite)
4. Move `plugins/mid/entry-check.bv` → `plugins/typed/entry-check.bv` (rewrite)
5. Move `plugins/post/validate-trg.bv` → `plugins/verified/validate-trg.bv` (rewrite)
6. Remove `plugins/mid/` directory
7. Remove `plugins/post/` directory
8. Remove `plugins/front/` directory
9. Remove `plugins/back/` directory (was empty)
10. Update `config/targets.toml` plugin lists
11. Update README files

### Phase I: Highlighter (1 day)

1. Create `syntaxes/beast.tmLanguage.json`
2. Update `brief.tmLanguage.json` — new tokens, remove old tokens
3. Update `package.json` — language entry, grammar entry, icon
4. Copy `assets/beast-icon.svg` → `syntax-highlighter/images/beast-icon.svg`
5. Update theme files if needed for new scopes

### Phase J: Documentation + Examples (1 day)

1. Rewrite `docs/architecture/features/plugins.md`
2. Update `docs/architecture/overview.md` pipeline diagram
3. Update `learn-brief/16-plugins.md`
4. Create example files in `examples/stage/` for each new stage type
5. Update old example files (`collect-match.bv`, `emit-error.bv`, `back-final.bv`,
   `post-validate.bv`, `mid-check.bv`, `front-import.bv`)

---

## 14. Coding Standards Enforcement

### 14.1 No Arrow Code

Never write:
```rust
// Forbidden: > 2 levels
for x in a {
    if let Some(y) = x {
        for z in y {
            ...
        }
    }
}
```

Write:
```rust
for x in a {
    let Some(y) = x else { continue; };
    for z in y {
        ...
    }
}
```

### 14.2 Max 2 Nesting Levels

Functions with >2 levels of indentation must extract helpers:
- Tree walkers → `visit_node`, `walk_children` helpers
- Actions → `resolve_position`, `apply_mutation` helpers
- Pattern matching → `match_atom`, `match_list`, `match_var` helpers

### 14.3 DRY — No Repeated Patterns

Patterns that appear in 3+ places must be centralized:
- Vec splicing → `modify_children` helper
- Stage filtering → `run_stage_ast` / `run_stage_text` / `run_stage_bin` helpers
- Pattern walking → shared `walk_nodes` with closure
- Error/warning emission → `emit_diagnostic(kind, message)` helper

### 14.4 Every Code Site Gets a Rationale Comment

Every `fn`, `struct`, and non-trivial match arm must have a comment:
```rust
// 2026-07-21: This selector matches by the S-expression tag name used in
// .beast serialization.  The tag maps 1:1 to a TopLevel/Statement/Expr
// variant and is the primary navigation key for users reading .beast output.
```

---

## 15. Migration Guide (for users)

### 15.1 Stage Name Changes

| Old | New | Same semantics? |
|-----|-----|-----------------|
| `$(Front)` for source ops | `$(PreLex)` | Yes — source text manipulation |
| `$(Front)` for AST ops | `$(Parsed)` | Yes — runs after parse, before imports |
| `$(Mid)` | `$(Typed)` | Yes — runs after type checking |
| `$(Post)` | `$(Generated)` | Yes — runs after codegen |
| `$(Back)` | `$(Optimized)` | Yes — runs after optimizations |

### 15.2 Intrinsic Changes

| Old | New |
|-----|-----|
| `InsertLiteralImport$("path")` | `Tag$("import").First$().Before$().Insert$(Import$("path"))` |
| `InsertRegistryImport$("name")` | `Tag$("import").First$().Before$().Insert$(Import$("name"))` |
| `Collect$("(call PrintInt# ?*)")` | `Tag$("call").Named$("PrintInt#").Count$()` |
| `MatchIR$("(add (int 0) ?x)", "(?x)")` | `Select$(Pattern$("(add (int 0) ?x)")).ReplaceWith$(Pattern$("?x"))` |
| `CheckReactive$()` | `If$(Tag$("txn").WithAttr$("reactive", true).Count$() > 0) { ... }` |

### 15.3 `--emit-beast` Changes

| Old flag | New flag |
|----------|----------|
| `--emit-beast ast` | `--emit-beast parse` |
| `--emit-beast mid` | `--emit-beast type-check` |
| `--emit-beast post` | `--emit-beast codegen` |
| `--emit-beast` (all) | `--emit-beast all` |

---

## 16. Testing Plan

### 16.1 Unit Tests

| Module | What to test |
|--------|-------------|
| `src/macros/selection.rs` | Each selector returns correct nodes; combinators (And/Or/Not); traversal correctness |
| `src/macros/pattern_live.rs` | Pattern compilation, match, collect, replace; fallback to serialization |
| `src/macros/actions.rs` | Insert/delete/replace preserve tree integrity; positions are consumed once |
| `src/macros/text_ops.rs` | Find/replace/insert on text buffers; boundary conditions |
| `src/macros/flow.rs` | ForEach iteration count; Let$ scoping; If$ short-circuit |
| `src/parser/definitions.rs` | Stage name parsing; old name rejection; chain parsing |
| `src/plugin/intrinsics.rs` | Navigation chain evaluation; builder construction |

### 16.2 Integration Tests

| Scenario | What to verify |
|----------|---------------|
| Prelude plugin inserts imports | `--emit-beast parse` shows correct imports |
| Auto-main adds entry marker | `--emit-beast type-check` shows `(entry)` in contract |
| Entry-check rejects no-entry program | Compilation error with informative message |
| PrintLn! expansion | Generated IR shows expanded calls |
| ForEach over imports | Correct count and positioning |
| Text operation on Ir$ | IR text modified as expected |
| Empty selection write | Warning emitted, compilation continues |
| Old stage name | Clear error with migration help |

### 16.3 Behavioral Tests (no literal snapshots)

Per Golden Rule 5 and Plan Directive 5: tests assert outcomes, not IR text.

```rust
#[test]
fn test_prelude_inserts_imports() {
    let mut items = parse("let x: Int = 5;");
    let mut universe = TypeUniverse::new();
    run_plugin(&plugin_prelude(), StageKind::Parsed, &mut items, &mut universe).unwrap();
    let import_count = items.iter()
        .filter(|i| matches!(i, TopLevel::Import(_)))
        .count();
    assert!(import_count >= 13, "prelude should inject all stdlib imports");
}
```

---

## 17. File Manifest

### New files

| File | Purpose |
|------|---------|
| `src/macros/mod.rs` | Module root — re-exports selection, pattern_live, actions, text_ops, flow |
| `src/macros/selection.rs` | `Selection`, `Selector` trait, concrete selectors, traversal |
| `src/macros/pattern_live.rs` | Live AST pattern compiler (port of `beast/pattern.rs`) |
| `src/macros/actions.rs` | `Position`, mutation actions (insert/delete/replace/wrap/rename/set) |
| `src/macros/text_ops.rs` | `TextSelection`, text operations (find/replace/insert/delete) |
| `src/macros/flow.rs` | `ForEach$`, `Let$`, `If$` evaluation |
| `syntax-highlighter/syntaxes/beast.tmLanguage.json` | Full .beast grammar |
| `assets/beast-icon.svg` | (already exists — used for .beast file icon) |

### Modified files

| File | Changes |
|------|---------|
| `src/ast/top.rs` | Expand `StageKind` enum |
| `src/compile.rs` | Wire stage hooks, update `BeastStage`, update `emit_beast_snapshot` |
| `src/plugin/mod.rs` | Expand `PluginManager` with new stage runners |
| `src/plugin/loader.rs` | Update discovery paths |
| `src/plugin/intrinsics.rs` | Remove old intrinsics, add navigation/builder dispatch |
| `src/parser/definitions.rs` | New stage names, chain parsing, flow control parsing |
| `src/main.rs` | CLI help text for new `--emit-beast` options |
| `config/targets.toml` | Plugin directory references |
| `plugins/parsed/prelude.bv` | Rewrite with new syntax |
| `plugins/parsed/prelude-hw.bv` | Rewrite with new syntax |
| `plugins/typed/auto-main.bv` | Rewrite with new syntax |
| `plugins/typed/entry-check.bv` | Rewrite with new syntax |
| `plugins/verified/validate-trg.bv` | Rewrite with new syntax |
| `examples/stage/*.bv` | Rewrite all stage examples |
| `docs/architecture/overview.md` | Pipeline diagram, module list |
| `docs/architecture/features/plugins.md` | Full rewrite |
| `learn-brief/16-plugins.md` | Update to new API |
| `syntax-highlighter/syntaxes/brief.tmLanguage.json` | New tokens, remove old |
| `syntax-highlighter/package.json` | .beast language + grammar + icon |

### Removed files

| File | Reason |
|------|--------|
| `plugins/front/` (dir) | Replaced by `plugins/parsed/` |
| `plugins/mid/` (dir) | Replaced by `plugins/typed/` |
| `plugins/post/` (dir) | Replaced by `plugins/generated/` (or `plugins/verified/`) |
| `plugins/back/` (dir) | Replaced by `plugins/optimized/` |
| `examples/stage/front-import.bv` | Uses removed $(Front) |
| `examples/stage/mid-check.bv` | Uses removed $(Mid) |
| `examples/stage/post-validate.bv` | Uses removed $(Post) |
| `examples/stage/back-final.bv` | Uses removed $(Back) |
| `examples/stage/collect-match.bv` | Uses removed Collect$/MatchIR$ |
| `examples/emit-error.bv` | Uses removed $(Front) |
