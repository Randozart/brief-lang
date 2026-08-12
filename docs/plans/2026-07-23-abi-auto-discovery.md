# Foreign ABI Auto-Discovery — Protocol Graph Population

**Date:** 2026-07-23
**Status:** Plan (1. implement, 2. research)

---

## Goal

Automatically discover foreign struct layouts and insert them into Briev's
protocol graph, so cross-language bridges can find `Identity` paths — zero
transform overhead — without manual configuration.

Two approaches:

---

## 1. ABI Probing (ShellCmd$ — implement first)

Compile and run a small C probe program at compile time. The probe uses
`offsetof` and `sizeof` to extract struct layout, returns it as text.
A `$defn` helper parses the output and calls `InjectTypeLayout$` to push
it into the type universe.

### New intrinsic: `InjectTypeLayout$`

```rust
InjectTypeLayout$(type_name, size, fields_list)
// type_name:   String (e.g., "Point")
// size:        Int   (e.g., 16)
// fields_list: List of [name: String, type: String, offset: Int]
```

Pushes layout info into the type universe so `find_cast_path` BFS can
find protocol paths that include this type. ~30 lines of Rust.

### ABI probe `$defn`

```briev
$defn probe_struct(name: String, fields: List, lib_path: String) {
    // 1. Generate C probe source
    let src = "#include <stddef.h>\nint main() {\n";
    foreach(f in fields) {
        src = src + "  printf(\"" + f + "=%zu\\n\", offsetof(" + name + ", " + f + "));\n";
    };
    src = src + "  printf(\"sizeof=%zu\\n\", sizeof(" + name + "));\n";
    src = src + "  return 0;\n}";
    // 2. Write, compile, run
    FileWrite$("probe.c", src, true);
    let out = ShellCmd$("gcc", "probe.c", "-o", "probe", "-I" + lib_path);
    out = ShellCmd$("./probe");
    // 3. Parse output → inject layout
    //    Each line: "fieldname=offset" or "sizeof=bytes"
    let lines = StrSplit$(out, "\n");
    let fields_list = [];
    foreach(line in lines) {
        // parse "name=value" into (name, value)
        // accumulate into fields_list
    };
    InjectTypeLayout$(name, size, fields_list);
};
```

### Limitations
- Requires header files at compile time
- Only works for types visible to the probe compiler
- Can't probe opaque types (void*)

---

## 2. DWARF Parsing (research — implement after probing)

Read `.debug_info` sections from the compiled `.so` to discover struct
layouts without needing header files.

### New intrinsic: `DwarfReadLayout$`

```
DwarfReadLayout$(lib_path, type_name) → List of [field, type, offset]
```

Walks the DWARF unit DIEs to find the named type, then enumerates its
members with byte offsets and type references. Returns the same format
as `InjectTypeLayout$` consumes, so the same universe population works.

### Structure
- Minimal DWARF parser in Rust (~200 lines — enough for DWARF 4/5 structs)
- `ShellCmd$("readelf --debug-dump=info lib.so")` → pipe output to a `$txn`
  BFS parser (pure Briev, zero Rust code)

### Advantages
- No headers needed — works with any `.so` that has debug info
- Discovers complete type graph (nested structs, typedefs)
- Can probe opaque types if `.dwo` / `.dwp` split DWARF is available

### Disadvantages
- Debug info often stripped from release builds
- DWARF spec is large — partial parser only covers structs/enums

---

## Files Touched

| File | Change |
|------|--------|
| `src/macros/eval.rs` | Add `InjectTypeLayout$` handler (~30 lines) |
| `src/macros/eval.rs` | Add `DwarfReadLayout$` handler (future, ~200 lines) |
| `lib/glue/generator.bv` | Add `probe_struct` `$defn` (pure Briev) |
| `src/analysis/layout_optimizer.rs` | Maybe — wire `InjectTypeLayout$` into protocol graph |

## Success Criteria

A `.bv` file that:

```briev
$(Parsed) {
    probe_struct("Point", ["x", "y"], "/usr/include");
    // After probe: compiler knows struct Point { int x; float y; }
    // Bridge between Briev Point<->C Point is Identity → zero cost
};
```
