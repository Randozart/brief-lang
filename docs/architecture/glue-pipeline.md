# GLUE Pipeline

```mermaid
graph TD
    subgraph INPUTS["Inputs"]
        BV[("Bridge .bv")]
        SO[("Foreign .so/.a")]
        DBVL[("glue.dbvl")]
    end

    subgraph LINK["briv link"]
        NM["nm --defined-only -g"]
        XREF["Cross-reference vs Intrinsic enum"]
        GEN_BV["Generate bridge .bv"]
    end

    subgraph EXPORT["briv export"]
        PARSE["Parse bridge AST"]
        EXTRACT["Extract #export / frgn / meld"]
        DBVL_SER["Serialize to DBVL strings"]
        LOOKUP["Find adapter in glue.dbvl"]
        MACRO["Invoke $!macro"]
        EMIT["emit_file#()"]
    end

    subgraph OUTPUTS["Outputs"]
        RUST["Rust crate<br/>Cargo.toml, build.rs, src/"]
        PYTHON["Python package<br/>__init__.py (ctypes)"]
        NODE["Node module<br/>package.json, index.mjs, index.d.ts"]
    end

    BV --> PARSE
    PARSE --> EXTRACT
    EXTRACT --> DBVL_SER
    DBVL_SER --> MACRO
    DBVL --> LOOKUP
    LOOKUP --> MACRO
    MACRO --> EMIT
    EMIT --> RUST
    EMIT --> PYTHON
    EMIT --> NODE

    SO --> NM
    NM --> XREF
    XREF -- "matches intrinsic" --> GEN_BV
    XREF -- "unknown symbol" --> GEN_BV

    LINK -. "input to (bridge .bv)" .-> EXPORT
```

## Flow Description

1. **`briv link <library>`** — Analyzes a foreign `.so`/`.a` via `nm`, extracts T (text) symbols, cross-references each against the compiler's `Intrinsic` enum. Known intrinsics become `intrinsic_call#()` wrappers; unknowns become `frgn` skeletons.

2. **`briv export <bridge.bv> <language>`** — Parses the bridge, extracts `#export`/`frgn`/`meld` declarations, serializes them as D-Briv Lines (bare comma-separated), looks up the language adapter in `glue.dbvl`, invokes the adapter's `$!macro`, which calls `emit_file#()` to write wrapper source files.

3. **Adapters** — Each language has a `.bv` `macro` at `glue/adapters/<lang>.bv`. The macro generates native source files without any Rust template engine. Adding a language = writing one `.bv` file.

4. **No C compiler** — The bridge emits LLVM IR → native `.o`/`.a`. The foreign language's linker (Rust's `rustc`, Python's `ctypes.CDLL`, Node's `WebAssembly.instantiate`) consumes it directly.
