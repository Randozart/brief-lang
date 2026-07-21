# Compiler Plugins

Brief supports compile-time plugins that run at defined hooks in the
compilation pipeline.  Plugins are written in Brief and use a tree-navigation
DSL to inspect and transform the AST, source text, or generated IR.

## Pipeline Stages

There are 11 stages.  Each runs a specific subset of plugins:

```text
Source ──► PreLex ──► Parsed ──► Resolved ──► Typed ──► Normalized
             │           │            │           │            │
          text ops   tree ops      tree ops    tree ops     tree ops

──► Verified ──► Allocated ──► Provenanced ──► Generated ──► Optimized
        │             │              │               │              │
     tree ops      tree ops       tree ops        text ops       text ops

──► Linked
       │
    binary ops
```

## Writing a Plugin

A plugin is a `.bv` file with one or more `$(StageName)` blocks:

```brief
// my-plugin.bv
$(Parsed) {
    // Runs after parsing, before import resolution
    Tag$("import").First$().Before$()
        .Insert$(Import$("std/custom.bv"));
};
```

### The Navigation Chain

Every operation follows the same pattern:

```
SELECT ──► TRAVERSE ──► POSITION ──► ACT
```

```brief
Tag$("import") .First$() .Before$() .Insert$(Import$("std/x.bv"))
 └─SELECT──┘  └TRAVERSE┘ └POSITION┘ └────────ACT────────────┘
```

- **Selectors** find nodes: `Tag$("defn")`, `Named$("main")`, `WithAttr$("entry", true)`
- **Traversal** narrows: `.First$()`, `.Children$("param")`, `.Descendants$("call")`
- **Positions** pick where: `.Before$()`, `.After$()`, `.Replace$()`, `.Inside$()`
- **Actions** do the work: `.Insert$(Import$("..."))`, `.Delete$()`, `.Set$("key", val)`

### Flow Control

Inside `$(Stage)` blocks, standard Brief syntax (`let`, `if`, `foreach`, `match`)
is evaluated at compile time. Navigation selections are first-class values.

```brief
// Bind a selection to a variable
let imports = Tag$("import");

// Iterate over matches
foreach(imp in imports) {
    imp.After$().Insert$(Import$("std/debug.bv"));
};

// Conditional
if(imports.Count$() == 0) {
    EmitWarning$("no imports found");
};
```

## Enabling/Disabling Plugins

```bash
# Disable the prelude plugin (no auto-imports)
brief build file.bv --disable-plugin prelude

# Enable a specific plugin
brief build file.bv --enable-plugin my-custom

# Disable all plugins (equivalent to --no-stdlib)
brief build file.bv --disable-plugin prelude
```

## Building With Plugins

```bash
# Default: system plugins run automatically
brief build file.bv

# With BEAST snapshots for debugging
brief build file.bv --emit-beast parsed

# Custom plugin file
brief build file.bv --enable-plugin my-plugin
```

## Target Selection

Each stage has a default data target:

| Stage | Default target | What you can do |
|-------|---------------|-----------------|
| `$(PreLex)` | `Source$` (text) | `Find$`, `Prepend$`, `ReplaceWith$`, `Text$()` |
| `$(Parsed)` through `$(Provenanced)` | AST (tree) | `Tag$`, `Named$`, `Insert$`, `Delete$`, `Set$` |
| `$(Generated)` through `$(Optimized)` | `Ir$` (text) | `Find$`, `InsertBefore$`, `ReplaceWith$`, `Text$()` |
| `$(Linked)` | `Bin$` (binary) | `Run$("command {{path}}")`, `Path$()`, `ReadBytes$()`, `Size$()` |
| All stages | `Stage$` (registry) | `Insert$(block)`, `Insert$(file)`, `Remove$(name)`, `List$()` |

You can always override by prefixing `Source$.`, `Ir$.`, `Bin$.`, or `Stage$.`:

```brief
$(Typed) {
    // Default: AST operations
    Tag$("defn").Named$("main").Set$("entry", true);
    // Explicit: source text access (read-only)
    let lines = Source$.Find$("#define").Count$();
};
```

## Full Brief at Compile Time

Inside `$(Stage)` blocks, you can write arbitrary Brief code and it runs at
compile time:

```brief
$(Parsed) {
    defn count_tagged(sel: Selection, tag: String) -> Int {
        let total = 0;
        foreach(item in sel) {
            if(item.Tag$(tag).Count$() > 0) {
                total = total + 1;
            };
        };
        term total;
    };

    let defns = Tag$("defn");
    EmitInfo$("defns with calls: " + count_tagged(defns, "call"));
};
```

Note: `txn`/`node`/`trg`/`frgn`/`Malloc#` are not available at compile time.
Only `let`/`defn`/`if`/`match`/`for` and the navigation DSL.

## Diagnostics

```brief
EmitInfo$("informational message");     // prints to stdout
EmitWarning$("suspicious pattern");     // prints to stderr
EmitError$("fatal problem");            // aborts compilation
```

## Plugins Creating Plugins

A plugin can register new plugins for later stages:

```brief
$(Parsed) {
    if(Tag$("call").Named$("Unsafe#").Count$() > 0) {
        Stage$.Insert$(Typed) {
            foreach(call in Tag$("call").Named$("Unsafe#")) {
                EmitWarning$("unsafe: " + call.Names$().First$());
            };
        };
    };
};
```

Forward-only: a `$(Parsed)` plugin cannot register for `$(Parsed)` or earlier.
Only stages > the current one.

## What's Next

- See `docs/architecture/features/plugins.md` for the full intrinsic reference.
- See `examples/stage/` for runnable plugin examples.
- See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md` for the design.
