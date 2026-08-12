# BEAST: Briev Virtual IR — Plugin Mid-End Architecture

## Design Summary

```
                       FAST PATH (default, zero overhead)
Source → Parse → Resolve → Analyze → Backend → .ll
                            ↑
                    Pure Rust AST, no serialization


                       PLUGIN PATH (--plugin flags present)
Source → Parse → Resolve → to_beast() → [PLUGIN CHAIN] → from_beast() → Analyze → Backend → .ll
                            │              │
                       program.beast    pipe stdin→stdout
                            │              │
                            └── diff ──────┘
```

The fast path is what runs during normal compilation. No serialization. Zero cost.

The plugin path activates only when `--plugin` is passed. The frontend serializes to `.beast` text S-expressions. Each plugin reads from stdin and writes to stdout — the compiler chains them with pipes. The backend deserializes the final output and proceeds as normal.

## BEAST Format (S-Expressions)

Every node is `(tag child1 child2 ...)`. Atoms are strings `"..."`, integers `42`, or booleans `true`/`false`.

### Type Universe

```
(universe "Int"
  (bytes 8)
  (alignment 8)
  (properties (primitive "Int")))

(universe "String"
  (bytes 8)
  (alignment 8)
  (properties (primitive "String") (encoding "utf-8")))
```

### Type Definitions

```
(typedef "MyStruct"
  (base "Bits")
  (slots
    (slot "x" "Int")
    (slot "y" "Int"))
  (metadata (jira_ticket "PAY-123")))
```

### Definitions (Functions)

```
(defn "list_users" () "JSON"
  (metadata (rest_route "/api/users"))
  (metadata (rest_method "GET"))
  (body
    (assign "result" (call "query_db" ((string "SELECT * FROM users"))))
    (term (ident "result"))))
```

### Transactions

```
(txn "counter" :reactive true
  (contract
    (pre (lt (ident "count") (ident "N")))
    (post true))
  (body
    (assign "count" (add (ident "count") (int 1)))
    (term)))
```

### State Declarations

```
(state "led_0" "Int"
  (mmio 0x40000000))
```

### Triggers

```
(trigger "btn_pressed"
  (port "PB0"))
```

## Plugin Contract

Every plugin is an executable that follows this protocol:

```
stdin:   receives .beast text (the IR before this plugin)
stdout:  writes .beast text (the IR after this plugin)
exit 0:  success — compilation continues with plugin's output
exit !0: abort — stderr is the error message
```

The compiler chains them:

```
frontend → plugin_a | plugin_b | plugin_c → backend
```

A plugin in any language works:

```python
#!/usr/bin/env python3
import sys
ir = sys.stdin.read()
# Walk IR, find defns with rest_route metadata, modify them
sys.stdout.write(modified_ir)
```

## Metadata — Unified Across All Contexts

`<~` always means metadata assignment. Every context uses `Statement::MetadataAssignment(String, PropertyValue)`.

```
type body:      bytes <~ 8; primitive <~ Int;
func body:      rest_route <~ "/api/users";
inline:         (metadata (rest_route "/api/users"))  ; in .beast
```

Plugins read metadata uniformly by walking the tree. The frontend never matches on metadata keys — that's the plugin's job.

## Pipeline

```
compile_source(file_path, source, opts):
  1. Lex → tokens
  2. Parse → items: Vec<TopLevel>
  3. Resolve → universe: TypeUniverse
  4. If plugins are loaded:
     a. Serialize items + universe → beast_text: String
     b. For each plugin:
        - Spawn plugin process
        - Feed beast_text to stdin
        - Read modified beast_text from stdout
        - If exit code != 0: abort with stderr message
     c. Deserialize beast_text → items, universe
  5. Analyze → analysis: AnalysisResults
  6. Codegen → llvm_ir: String
  7. Write .ll file
  8. Optionally compile to binary via clang
```

## Files To Build

| File | Purpose | Lines |
|------|---------|-------|
| `src/beast/mod.rs` | Module root, re-exports, `to_beast()` / `from_beast()` | ~40 |
| `src/beast/sexpr.rs` | S-expression tokenizer + parser | ~200 |
| `src/beast/serialize.rs` | Walk AST + TypeUniverse, emit S-expressions | ~350 |
| `src/beast/deserialize.rs` | Read S-expressions, produce AST + TypeUniverse | ~450 |
| `src/plugin/runner.rs` | Spawn plugin processes, chain stdin→stdout | ~80 |
| `src/lib.rs` | Add `pub mod beast;` | 1 |
| `src/compile.rs` | Wire plugin path above | ~40 |
| `src/main.rs` | Add `--emit-beast` and `--plugin` flags | ~30 |

## Coding Standards (Every Function)

- Max 2 nesting levels deep. Extract helpers.
- Guard clauses, early returns. No `else-if` chains deeper than 1.
- `///` doc comments on every function.
- `// 2026-07-14: <why this exists>` on every change.

## Implementation Order

1. `src/beast/sexpr.rs` — S-expression parser (foundation)
2. `src/beast/serialize.rs` — walk AST → text
3. `src/beast/deserialize.rs` — text → AST
4. `src/beast/mod.rs` — module root
5. `src/lib.rs` — register module
6. `src/plugin/runner.rs` — plugin process chain
7. `src/compile.rs` — wire fast path vs plugin path
8. `src/main.rs` — CLI flags
9. Tests: round-trip serialize→deserialize, plugin modifies IR
