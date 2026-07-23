# Macro System — Compile-Time `$` Intrinsics

**Date:** 2026-07-22
**Status:** Architecture documentation

---

## Contract

Every `$` intrinsic is **fully generic** — useful to ANY plugin, not just
the GLUE bridge generator. A proposed intrinsic must have at least 3 distinct
non-GLUE use cases before it's accepted.

If a capability is only useful for bridge generation, it belongs in the
bridge generator `.bv` file itself (as a composition of generic intrinsics),
not as a new intrinsic in the Rust engine.

---

## Current Intrinsics

### AST Selection

Select nodes from the program tree by structural properties:

| Intrinsic | Signature | Returns | Example |
|-----------|-----------|---------|---------|
| `Tag$` | `(tag: str)` | Selection | `Tag$("transaction")` — all transactions |
| `Named$` | `(name: str)` | Selection | `Named$("main")` — items named "main" |
| `WithKey$` | `(key: str)` | Selection | `WithKey$("entry")` — items with entry key |
| `WithAttr$` | `(key: str, val: str)` | Selection | `WithAttr$("entry", "true")` — entry items |
| `All$` | `()` | Selection | `All$()` — every top-level item |

### AST Traversal

Navigate the tree relative to a selection:

| Intrinsic | Signature | Returns | Example |
|-----------|-----------|---------|---------|
| `First$` | `([n])` → Selection | Single element | `Tag$("txn").First$()` — first transaction |
| `Last$` | `([n])` → Selection | Single element | `Tag$("txn").Last$()` — last transaction |
| `Nth$` | `(n: int)` → Selection | Single element | `Tag$("txn").Nth$(1)` — second transaction |
| `Children$` | `([filter])` → Selection | Child nodes | `Named$("main").Children$()` |
| `Descendants$` | `([filter])` → Selection | All descendants | `Named$("main").Descendants$("Call")` |
| `Parent$` | `()` → Selection | Parent node | `Tag$("param").Parent$()` |

### AST Introspection

Query properties of selections:

| Intrinsic | Returns | Example |
|-----------|---------|---------|
| `Count$` | `Int` | `Tag$("txn").Count$()` — number of transactions |
| `Names$` | `Vec<String>` | `Tag$("defn").Names$()` — all defn names |
| `IsEmpty$` | `Bool` | `Tag$("error").IsEmpty$()` — true if no errors |

### AST Positions

Compute insertion positions relative to selections:

| Intrinsic | Returns | Example |
|-----------|---------|---------|
| `Before$` | Position | `Tag$("import").First$().Before$()` |
| `After$` | Position | `Tag$("import").Last$().After$()` |
| `Replace$` | Position | `Named$("old_fn").Replace$()` |
| `Inside$` | Position | `Named$("main").Inside$()` |
| `AppendTo$` | Position | `Named$("struct").AppendTo$()` |

### AST Actions

Modify the program tree:

| Intrinsic | Effect | Example |
|-----------|--------|---------|
| `Insert$(pos, nodes...)` | Inserts nodes at position | `Before$().Insert$(Import$("std"))` |
| `Delete$(sel)` | Removes selected nodes | `Delete$(Named$("dead_fn"))` |
| `ReplaceWith$(sel, node)` | Replaces selection | `ReplaceWith$(Named$("old"), Defn$("new"))` |
| `Set$(sel, key, value)` | Sets metadata | `Set$(Named$("f"), "attr", "true")` |
| `Rename$(sel, name)` | Renames item | `Rename$(Named$("x"), "y")` |

### AST Constructors

Build new AST nodes:

| Intrinsic | Returns | Example |
|-----------|---------|---------|
| `Import$(path)` | TopLevel | `Import$("std/env.bv")` |
| `Defn$(name)` | TopLevel | `Defn$("my_fn")` |
| `Call$(name, args...)` | Statement | `Call$("print", Str$("hello"))` |
| `Block$(stmts...)` | Statement | `Block$(Call$("work"))` |

### Compile-Time Data (Added 2026-07-22)

| Intrinsic | Signature | Returns | Category |
|-----------|-----------|---------|----------|
| `StrLen$` | `(s)` | Int | String |
| `StrReplace$` | `(s, from, to)` | String | String |
| `StrJoin$` | `(list, sep)` | String | String |
| `StrSplit$` | `(s, pat)` | List | String |
| `StrSubstr$` | `(s, start, end)` | String | String |
| `FileRead$` | `(path)` | String | I/O |
| `FileWrite$` | `(path, content)` | Void | I/O |
| `ConfigGet$` | `(section, dotted_key)` | String | Config |
| `DocRead$` | `(type_name, property)` | varies | Universe |
| `TypeInfo$` | `(selection, field)` | varies | Type |
| `CastPath$` | `(src_type, tgt_type)` | List | Protocol |
| `ShellCmd$` | `(cmd, args...)` | String | Process |

---

## Generic `$` Design Principles

### Principle 1: No GLUE-specific knowledge in Rust

The Rust engine (`src/macros/eval.rs`) should never reference `glue`, `bridge`,
`export`, or any language-specific concept. Everything in the `$` system is a
generic primitive. The bridge generator is a `.bv` plugin file that composes
these primitives.

### Principle 2: Three-use-case test

Every proposed intrinsic must have at least 3 distinct non-GLUE use cases:

| Intrinsic | Use case 1 | Use case 2 | Use case 3 |
|-----------|-----------|------------|------------|
| `StrReplace$` | Template substitution | Error message formatting | Code generation |
| `FileWrite$` | Scaffold generation | Doc output | Log file writing |
| `ConfigGet$` | Plugin configuration | Build profiles | Feature flags |
| `DocRead$` | Doc generator | Linter rules | LSP plugin |
| `TypeInfo$` | Bridge generator | Doc generator | Linter |
| `CastPath$` | Bridge generator | Type checker extension | Protocol debugger |
| `ShellCmd$` | Bridge build step | External formatter | Test runner |

### Principle 3: `NavValue` variants stay orthogonal

| Variant | Meaning | Used as input by |
|---------|---------|------------------|
| `Selection` | Set of tree nodes | All traversal, position, and action intrinsics |
| `Position` | Insertion point | `Insert$` |
| `Count` | Integer count | `when guard > 0` |
| `Names` | List of string names | `StrJoin$` |
| `Bool` | Boolean | `when guard` |
| `Int` | Integer | Arithmetic, indexing |
| `Str` | String | All string intrinsics |
| `List` | Generic list of NavValues | `StrJoin$` (input), `StrSplit$` (output) |
| `TopLevel` | Single AST node | `Insert$`, `ReplaceWith$` |
| `VecTopLevel` | Multiple AST nodes | `Insert$` |
| `Void` | No value | `FileWrite$`, actions |
| `Map` | Key-value pairs | `ConfigGet$` (output) |

---

## The Bridge Generator as a `.bv` Plugin

The GLUE bridge generator uses exactly 7 generic `$` intrinsics, composed
in a single `.bv` file:

```brief
$(Glue @ highest) {
    // 1. Read templates from config
    let fn_tmpl = ConfigGet$("rust", "templates.fn_template");
    let ffi_tmpl = ConfigGet$("rust", "templates.ffi_template");

    // 2. Find all exported definitions
    let exports = Tag$("export").Children$("Definition");

    // 3. For each export, generate wrapper code
    for export in exports {
        let name = TypeInfo$(export, "name");
        let params = TypeInfo$(export, "params");

        // 4. Compute protocol path for each parameter
        let path = CastPath$(params, "#String");

        // 5. Substitute template variables
        let fn_body = StrReplace$(fn_tmpl, "{{name}}", name);
        let fn_body = StrReplace$(fn_body, "{{params}}", params);

        // 6. Write output files
        FileWrite$("src/lib.rs", fn_body);
    };
};
```

This is the proof that the `$` system is complete: a cross-language bridge
generator that does file I/O, config reading, string templating, protocol
path computation, and AST inspection — all using generic primitives, with
zero Rust changes for language-specific logic.
