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

## BVIR path not needed for in-process plugins

The BVIR serialize→external→deserialize path is only for the legacy
`--plugin` CLI flag (external executables). In-process plugins via
`PluginManager` work directly on AST/IR in memory.

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

Phase 5 will implement the full `AddressOf#` intrinsic, the `*` deref
expression, and the two-phase safety model.
