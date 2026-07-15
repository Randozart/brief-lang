# Plugin Architecture & Prelude Design

**2026-07-15**: Phase 2 decisions. Supersedes `macro.md`.

## How $ intrinsics work

`$` intrinsics (e.g. `InsertRegistryImport$("std/prelude.bv")`) are Rust
functions that operate directly on `&mut Vec<TopLevel>` (AST) and
`&mut TypeUniverse` — no text-level expansion. A `$(Front)` block's body
is a sequence of `$` calls; when the stage runs, each call dispatches to
the corresponding `execute_$intrinsic()` Rust function inline.

## Prelude per extension

Not all backends need the same types. The prelude is a system plugin
(`plugins/front/prelude.bv`) that injects stdlib imports into user code.
Different extensions get different preludes:

| Extension | Backend | Plugin | Stdlib entry point |
|-----------|---------|--------|--------------------|
| `.bv` | LLVM | `prelude` | `std/prelude.bv` — Int, Bool, Float, String, collections, Option, Result |
| `.ebv` | LLVM embedded | `prelude` | same |
| `.rbv` | Webstack | `prelude` | same |
| `.abv` | GPU | `prelude` | same |
| `.cbv` | CIRCT | `prelude-hw` | `std/hardware.bv` — Cell, Wire, Register, Bit, etc. |

The prelude plugin file is just a `$(Front @ highest)` block that calls
`InsertRegistryImport$()`. System plugins import what they need manually.

## Plugin naming

System plugins discovered from `plugins/{stage}/<name>.bv` are registered
with the name `<name>` (the file stem, no prefix/suffix). This lets
`config/targets.toml` reference them by simple name in the `plugins` list.

## Metaprogramming with Collect$ and MatchIR$

**2026-07-15**: Phase 6. Two `$` intrinsics provide BVIR-level pattern
matching for compile-time metaprogramming:

- `Collect$(pattern)` — serialize the current program AST to BVIR, match
  the pattern against all sub-trees, log the match count and first few
  matches. Useful for inspecting what the compiler sees.
- `MatchIR$(pattern, replacement)` — serialize, apply pattern-match
  replacement, deserialize back into the AST. The program is modified
  in place. Returns an error if the pattern matches nothing.

### Pattern syntax

The BVIR pattern language uses S-expressions with `?` variables:

| Pattern | Meaning |
|---------|---------|
| `?x` | Match any single sub-tree, bind to `x` |
| `?*` | Wildcard: match any single sub-tree, no binding |
| `??*` | Rest wildcard: match zero or more trailing children |
| `*` | Wildcard tag: match any list tag (first position) |
| `(?tag ?x ?y)` | Match list with tag `?tag`, children `?x`, `?y` in order |
| `(ident ?name)` | Match list with tag `ident`, bind first child to `name` |
| `(call ?fn ??*)` | Match list with tag `call`, bind `fn`, rest wildcard |

### Collect$ example

```brief
$(Mid) {
    // --emit-bvir mid to see what BVIR looks like at this stage
    Collect$("(call PrintInt# ?*)");
};
```

### MatchIR$ example

```brief
$(Mid) {
    // Replace all (add (int 0) ?x) with just ?x
    MatchIR$("(add (int 0) ?x)", "(?x)");
};
```

## Visualing BVIR with --emit-bvir

The `--emit-bvir` flag writes BVIR snapshots at pipeline stages so
metaprogrammers can inspect the AST format when writing `Collect$` and
`MatchIR$` patterns:

```bash
brief-compiler build program.bv --emit-bvir ast    # after parse + front plugins
brief-compiler build program.bv --emit-bvir mid    # after typecheck + mid plugins
brief-compiler build program.bv --emit-bvir post   # after normalizer, before codegen
brief-compiler build program.bv --emit-bvir        # all three stages
```

Each stage writes `<file>.bvir.{ast,mid,post}`. Use these files to
understand what patterns match your AST.

## --no-std is now --disable-plugin prelude

The old `--no-std` flag still works but is equivalent to
`--disable-plugin prelude`. The prelude is a system plugin
(`plugins/front/prelude.bv`) that injects stdlib imports. To disable
it, use either form.

## Webstack output

The webstack backend emits HTML + CSS + SVG + TypeScript, not WASM.
`.rbv` uses it.

## Dynamic Trigger Targets via `@ *ptr`

**2026-07-15**: Phase 5 design. Static triggers (`trg x @ fixed_instance.#port`)
are resolved at compile time. Dynamic triggers use a typed pointer to change
the target at init time or across reconfigurations.

### Motivation

Embedded and reactive systems often need to bind a handler to an entity that
isn't known until runtime — a USB device on a hot-swappable bus, a virtual
device registered by another component, or a memory-mapped peripheral whose
base address is configurable. Brief's contract system must extend to these
cases without sacrificing safety.

### The pattern

`AddressOf#<T>(id)` returns `Ptr<T>` — a typed pointer that carries the
entity's declared shape in its type parameter. Applying `.#field` scopes the
type further:

```brief
let uart_rx: Ptr<UartRxPort> = AddressOf#<UartRxPort>("sys:uart/rx").#rx;
trg x @ *uart_rx;
```

Here `*uart_rx` dereferences the pointer to the target entity. The backend
resolves the target at _init time_ instead of compile time. Since the type
`UartRxPort` was already checked when `AddressOf#<T>` was called, the trigger
binding is statically type-safe.

### Without explicit field projection

If `AddressOf#` is called with a type that already describes the port:

```brief
trg x @ *AddressOf#<UartRxPort>("sys:uart/rx");
```

The type `UartRxPort` carries the port shape. No `.#rx` is needed because
the pointer type already describes exactly what trigger to set up.

### Safety model

| Check | When | What happens on failure |
|-------|------|------------------------|
| `T` matches expected port type | Compile (type resolution) | Type error — rejected |
| Target entity exists at `*ptr` | Init time (runtime) | Warning with `--warn-unresolved-trg` |
| Entity shape matches `T` | Init time (runtime) | Error with `--error-unresolved-trg` (or warning by default) |

The compile-time contract (`T`) guarantees the _shape_ is correct. The runtime
check guarantees the _entity exists and matches_. This is two-phase safety:
static type safety + runtime assertion, mirroring Brief's contract philosophy.

### Example: hot-swappable input device

```brief
type GamepadInput <: InputPort {
    // fields: button_a, button_b, dpad_x, dpad_y
};

txn handle_input [has_device][has_device] {
    let device: Ptr<GamepadInput> = AddressOf#<GamepadInput>("usb:gamepad");
    [*device != null] {
        trg x @ *device;
        term;
    };
    [*device == null] {
        term; // no device — skip
    };
};
```

### Implementation approach

1. Parser: accept `Expr::Deref(Box<Expr>)` on the instance side of `@`
   (existing `@` parsing, new deref expr variant)
2. Type checker: verify `*ptr` resolves to a type compatible with the
   port's expected trigger shape
3. Codegen (LLVM): emit an init-time table lookup + null check + listener
   registration. The table maps entity names to addresses and is populated
   by the runtime linker.
4. Codegen (CIRCT/Webstack): backend-specific init-time resolution
5. Post-stage plugin: inject a validation guard that runs at init and
   warns on unresolved targets

Phase 5 implemented `AddressOf#`, the `*` deref expression
(`Expr::Deref`), and the two-phase safety model. Phase 6 added the BVIR
pattern compiler for `Collect$`/`MatchIR$`.

## Mid-Stage Plugins (Auto-Entry + Entry Check)

Three Mid-stage plugins are enabled by default. They run in priority order:

| Plugin | Stage | Priority | Purpose |
|--------|-------|----------|---------|
| `auto-main` | Mid | 0 | Adds `[#]` entry marker to `defn main` or `txn main` |
| `entry-check` | Mid | 1 | Rejects programs with no entry mechanism |
| `check-reactive` | Mid | 2 | Verifies reactive transactions have live field bindings |

### auto-main (`plugins/mid/auto-main.bv`)

Uses `MatchIR$` to rewrite `defn main` or `txn main`, inserting an `(entry)`
marker in the contract. This is equivalent to writing `[#]` in the source:

```brief
$(Mid) {
    MatchIR$(
        "(defn main ?contract ?params ?ret ?body)",
        "(defn main (contract (entry) (pre true) (post true)) ?params ?ret ?body)"
    );
};
```

After this plugin runs, the program has an explicit entry point. Disable
with `--disable-plugin auto-main`.

### entry-check (`plugins/mid/entry-check.bv`)

Uses `Collect$` to check for `(entry)` markers, reactive transactions, and
top-level triggers. If none exist, emits `EmitError$`:

```brief
$(Mid) {
    let has_entry = Collect$("(contract (entry) ??*)");
    let has_trg = Collect$("(trigger ?name ?type @ ?binding)");
    [has_entry == 0 && has_trg == 0] {
        CheckReactive$();
    };
};
```

This rejects programs that have no way to start executing.

### check-reactive (`CheckReactive$` intrinsic)

A `$` intrinsic (invoked by `entry-check`) that walks the program AST to
verify at least one reactive transaction has either an `[#]` entry marker or
reads a top-level `let` binding with an initial value. This ensures reactive
transactions have something to react to on startup.

Disable with `--disable-plugin entry-check` (which also suppresses
`CheckReactive$` since it's called from `entry-check`).
