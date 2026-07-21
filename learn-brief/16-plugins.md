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

```brief
// Bind a selection to a variable
Let$imports = Tag$("import");

// Iterate over matches
ForEach$(Tag$("import")) {
    $.After$().Insert$(Import$("std/debug.bv"));
};

// Conditional
If$(Tag$("import").Count$() == 0) {
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
| `$(PreLex)` | `Source$` (text) | `Find$`, `Prepend$`, `ReplaceWith$` |
| `$(Parsed)` through `$(Provenanced)` | AST (tree) | `Tag$`, `Named$`, `Insert$`, `Delete$`, `Set$` |
| `$(Generated)` through `$(Optimized)` | `Ir$` (text) | `Find$`, `InsertBefore$`, `ReplaceWith$` |
| `$(Linked)` | `Bin$` (binary) | `Run$("command {{path}}")` |

You can always override by prefixing `Source$.`, `Ir$.`, or `Bin$.`:

```brief
$(Typed) {
    // Default: AST operations
    Tag$("defn").Named$("main").Set$("entry", true);
    // Explicit: source text access (read-only)
    let lines = Source$.Find$("#define").Count$();
};
```

## What's Next

- See `docs/architecture/features/plugins.md` for the full intrinsic reference.
- See `examples/stage/` for runnable plugin examples.
- See `docs/plans/2026-07-21-granular-pipeline-and-ast-navigation.md` for the design.
