# Data Brief — Configuration & Structured Data Format

**Date:** 2026-07-26
**Status:** Specification
**Supersedes:** `docs/DATABRIEF.md`, `docs/DATABRIEF_GUIDE.md`

---

## 1. Philosophy

Data Brief is a family of two formats (`.dbv`, `.dbvl`) designed for the
systems-programming sweet spot: **deterministic parsing, zero ambiguity, and
human ergonomics** — without the bloat of XML, the magic coercions of YAML,
the quote pollution of JSON, or the bracket noise of TOML.

Three principles govern the design:

1. **`;` is the universal terminator.** No commas, no trailing-quote state
   machines. The parser is a single-pass byte scanner that only needs to see
   `;` and `}` to know a field is done.
2. **Quotes are an escape hatch, not a default.** Bare tokens carry 90% of
   data. The `" "` syntax exists for literal data containing `;`, `:`, or
   `}` — and is gated behind a parser flag so the default path has zero
   quote-tracking overhead.
3. **Schema is local, optional, and always explicit.** A `.dbv` file may
   declare its schema inline, import it, or omit it entirely. Schema inference
   is never automatic — the compiler never guesses.

---

## 2. File Types

| Extension | Name | Purpose |
|-----------|------|---------|
| `.dbv` | Data Brief Volume | Structured data with optional inline schema. Supports named entries, positional entries, nested blocks, and key-value maps. |
| `.dbvl` | Data Brief Lines | Line-oriented format. One entry per line. Positional fields separated by `;`. Schema imported via `#` directives. |

`.dbvs` is removed. Schema definitions live in `.dbv` files (inline or as
standalone schema-only `.dbv` files).

---

## 3. Lexical Rules (Both Formats)

### 3.1 Tokens

| Token | Role |
|-------|------|
| `;` | Field terminator and entry terminator |
| `:` | Key-value binder (entry level and map level) |
| `{ }` | Groups a nested sub-record or map |
| `>` | **Line-start**: directive prefix (`.dbvl`) / **Block-scoped**: marks a positional entry (`.dbv`) |
| `" "` | Quoted string (opt-in via parser flag) |

### 3.2 Whitespace

Whitespace (spaces, tabs) between tokens is ignored. A field reads from the
end of any preceding whitespace until `;` or `}`:

```
name:   Alice Smith;    age:    30;
```

Both parse as `name = "Alice Smith"`, `age = "30"`. The parser does not
distinguish between one space and ten.

### 3.3 Bare Tokens

By default, all values are unquoted bare tokens. A bare token is any sequence
of bytes that does not contain `;`, `}`, or `>` (at line start in `.dbvl`).

```
alice: Alice Smith; 30; Main St;
```

Produces four fields: `"Alice Smith"`, `"30"`, `"Main St"`.

The parser does not interpret type — `30` is the string `"30"` unless the
schema declares the field as `Int`. Type interpretation is the schema's job,
not the parser's.

### 3.4 Quoted Strings (Parser Flag)

When the `--quoted` parser flag is enabled, `"..."` can contain any character
including `;`, `:`, `}`, and newlines. Inside quotes, `\"` is the escape for
a literal quote.

```
alice: "Alice Smith; age 30"; next_field;
```

Without `--quoted`, the `"` is treated as a literal character in a bare token
(which is almost never what the user wants — the flag is recommended when data
may contain syntax characters).

### 3.5 Comments

Comments begin with `//` and extend to end of line. They are stripped during
lexing and never reach the parser.

```
// This is a comment
alice: Alice Smith; 30;  // inline comment
```

---

## 4. Schema Definition

### 4.1 Basic Schema

A schema defines field names, types, and order. It appears at the top of a
`.dbv` file and applies to the `as` block that follows.

```
schema Person {
    name: String;
    age: Int;
    street: String;
};
```

Types are advisory for tooling and code generation. The parser itself treats
all values as raw byte strings — type validation is the consumer's
responsibility.

### 4.2 Key Field

A parenthesized name after the schema name declares the key field — the field
that uniquely identifies each entry. This is purely a parsing directive, not
a constraint system.

```
schema Person (name) {
    name: String;
    age: Int;
};
```

The parser uses the key field to bind entries in keyed context. In `.dbvl`,
the key field can be used to auto-assign keys to positional entries.

### 4.3 Nested Schemas

A field's type may reference another schema:

```
schema Address {
    street: String;
    city: String;
    postal: String;
};

schema Person (name) {
    name: String;
    age: Int;
    address: Address;
};
```

Nested data uses `{ }` at the value site (see §6.4).

### 4.4 Schema Import

A `.dbv` file may import a schema from another `.dbv` file instead of defining
it inline:

```
schema Person from "types/person.dbv";

as Person {
    alice: Alice Smith; 30;
    bob: Bob; 25;
};
```

The import path is resolved relative to the current file's directory.

---

## 5. Primitive Types

Data Brief defines exactly six primitive types. These are the only types the
parser and schema validator understand natively — no Brief `.bv` type universe
required. A `.dbv` file can be parsed by any language (C, Python, Rust, Brief)
without importing any Brief source code.

### 5.1 The Six Primitives

| Type | Meaning | Example values |
|------|---------|----------------|
| `String` | UTF-8 text, any length | `Alice Smith`, `hello world` |
| `Int` | Signed integer | `42`, `-7`, `0` |
| `Float` | IEEE 754 double-precision | `3.14`, `-0.5`, `1e10` |
| `Bool` | Boolean | `true`, `false` |
| `Map` | Unschematicated key-value pairs | `{ key: val; k2: v2; }` |
| `Array<T>` | Ordered list of type `T` | `String[]`, `Int[]`, `Person[]` |

### 5.2 Parser Treatment

The parser treats all values as raw byte strings. Type interpretation is the
schema consumer's responsibility. However, the schema validator MAY perform
lightweight structural checks:

- `Int` fields that fail `strtol` produce a validation warning
- `Bool` fields not matching `true`/`false` produce a validation warning
- `Array<T>` fields not delimited by `[]` in the schema produce a parse error

### 5.3 Schema Composition

Fields may reference another schema by name. The referenced schema must be
defined in the same file or imported:

```
schema Address {
    street: String;
    city: String;
};

schema Person (name) {
    name: String;
    age: Int;
    address: Address;     // references Address schema above
    tags: String[];       // array of String
};
```

### 5.4 Array Syntax

Arrays are declared with `[]` appended to any type:

| Declaration | Meaning |
|-------------|---------|
| `String[]` | Array of strings |
| `Int[]` | Array of integers |
| `Person[]` | Array of Person entries |

At the value site, arrays use `{ }` with positional or keyed entries:

```
schema Team {
    name: String;
    members: Person[];
};

as Team {
    alpha: Alpha Team; { @ Alice Smith; 30; @ Bob; 25; };
};
```

### 5.5 Why Only Six Primitives

The goal is universal parseability. A C program reading a `.dbv` file should
not need to link against Brief's type system. Six primitives cover every
configuration and metadata use case the format targets. If richer typing is
needed, the bridge layer or a downstream tool can cast raw `String` values
to Brief types — the data format does not enforce the mapping.

---

## 6. `.dbv` — Structured Data Format

### 6.1 Entry Syntax

Inside an `as` block, each entry has either a **keyed** or **positional**
form:

#### Keyed Entry

```
key: field; field; { nested; }; field;
```

The key is separated from the fields by `:`. The key is a bare token — it
cannot contain `:`, `;`, or `}`.

```
as Person {
    alice: Alice Smith; 30; { Main St; Springfield; };
    bob: Bob; 25; { Oak Ave; Portland; };
    charlie: Charlie; 40; { Elm St; Denver; };
};
```

#### Positional Entry

When the key is omitted, the entry starts with `>`:

```
> field; field; { nested; }; field;
```

The `>` signals "positional entry" — the parser does not expect a key. The
entry is indexed by its line position within the block.

```
as Person {
    > Alice Smith; 30; { Main St; Springfield; };
    > Bob; 25; { Oak Ave; Portland; };
};
```

If the schema declares a key field (`schema Person (name) { ... }`), the
parser extracts the key from that field's position in the positional entry.

### 6.2 The `as` Block

An `as` block binds entries to a schema. The schema must be declared or
imported before the `as` block.

```
schema Person (name) {
    name: String;
    age: Int;
};

as Person {
    alice: Alice Smith; 30;
    bob: Bob; 25;
};
```

A file may contain multiple `as` blocks:

```
as Person {
    alice: Alice Smith; 30;
};

as Address {
    alice_home: { Main St; Springfield; };
};
```

### 6.3 Standalone Entries (No `as` Block)

At the top level of a `.dbv` file, entries can appear without an `as` block.
Each entry declares its own schema inline:

```
alice: Person { name: Alice Smith; age: 30; };
bob: Person { name: Bob; age: 25; };
```

The form is `key: SchemaName { fields; };`. This is syntactic sugar for an
anonymous single-entry `as` block.

### 6.4 Nested Blocks

A field whose schema defines sub-fields uses `{ }` to group them:

```
schema Address {
    street: String;
    city: String;
};

as Person {
    alice: Alice Smith; 30; { Main St; Springfield; };
};
```

Nested blocks are positional or keyed depending on the nested schema. If
`Address` has no key field, the block is positional — fields map by schema
order.

If `Address` declared `(street)` as its key, the block could be keyed:

```
alice: Alice Smith; 30; { street: Main St; city: Springfield; };
```

### 6.5 Key-Value Maps

Inside `{ }`, `:` can also create key-value pairs (not bound to a schema):

```
schema RegistryEntry {
    language: String;
    path: String;
    extension: String;
    triple: String;
    c_type_map: Map;
};

as RegistryEntry {
    rust: rust; glue/rust/types.bv; rs; x86_64-unknown-linux-gnu;
        { Int: int64_t; Float: double; Bool: bool; };
};
```

Inside an unschematized map block, `key: value;` pairs are free-form — the
parser collects them into an associative structure for the consumer.

### 6.6 Trailing `;`

The final field in any block may omit its trailing `;`:

```
alice: Alice Smith; 30     // valid — no ; after 30
```

This applies at every nesting depth.

---

## 7. `.dbvl` — Line-Oriented Format

### 7.1 Structure

A `.dbvl` file is one entry per line. Fields are positional, separated by `;`.
Schema is linked via `>` directives at the top of the file.

```
>schema Person from "person.dbv"
Alice Smith; 30; Main St; Springfield;
Bob; 25; Oak Ave; Portland;
Charlie; 40; Elm St; Denver;
```

### 7.2 Directives (`#`)

Lines beginning with `#` are directives, not data. They are processed in order
and affect all subsequent data lines.

| Directive | Purpose |
|-----------|---------|
| `#schema <Name> from <path>` | Imports a schema for data validation |
| `#import <path>` | Imports another `.dbvl` file inline |
| `#encoding <name>` | Sets text encoding for subsequent lines |
| `#version <n>` | Version marker for tooling |

A `.dbvl` file may have any number of directives interleaved with data. Once
a `#schema` directive is processed, all subsequent data lines are validated
against that schema until the next `#schema` directive replaces it.

```
#schema Person from "person.dbv"
Alice Smith; 30;
Bob; 25;

#schema Address from "address.dbv"
Main St; Springfield;
Oak Ave; Portland;
```

### 7.3 Key Assignment

If the imported schema declares a key field, the parser extracts the key from
that field's position automatically. No `@` or `key:` syntax is needed — every
line is implicitly a positional entry.

```
// schema Person (name) { name: String; age: Int; }
#schema Person from "person.dbv"
Alice Smith; 30;     // key = "Alice Smith"
Bob; 25;             // key = "Bob"
```

If a line should use an explicit key that differs from the key field's value,
prefix the line with `key: `:

```
#schema Person from "person.dbv"
admin: Alice Smith; 30;    // key = "admin", name = "Alice Smith"
```

The explicit key overrides the schema-derived key.

### 7.4 Why No `" "` in `.dbvl`

Quoted strings are not supported in `.dbvl` — not even as a parser flag. The
format is designed for single-pass streaming parse. If data contains `;` or
`#` at line start, use `.dbv` instead. This keeps the `.dbvl` parser at
~40 lines of code in any language.

---

## 8. Canonical Form

Two `.dbv` or `.dbvl` files with the same data must produce the same canonical
binary representation. This is critical for incremental builds and caching.

### 8.1 Canonicalization Rules

1. **Keys are sorted alphabetically** within each `as` block.
2. **Fields follow schema order**, not the order they appear in the source.
3. **Whitespace is stripped** from all field values.
4. **Comments are removed**.
5. **Trailing `;` is removed** from all fields.
6. **Nested blocks are canonicalized recursively** with the same rules.
7. **Map key-value pairs** are sorted by key alphabetically.

### 8.2 Example

Source (non-canonical):

```
schema Person (name) { name: String; age: Int; };
as Person {
    bob:  Bob; 25;
    alice:  Alice Smith;  30;
};
```

Canonical form:

```
schema Person (name) { name: String; age: Int; };
as Person {
    alice: Alice Smith; 30;
    bob: Bob; 25;
};
```

---

## 9. BeastDB Binary Format (`.beastdb`)

*Concept — implementation deferred to a dedicated plan.*

BeastDB is a compiled binary representation of `.dbv`/`.dbvl` data designed
for memory-mapped, zero-deserialization reads.

### 9.1 Header

```
┌──────────────────────────────────────┐
│ Magic: 4 bytes "BDB\0"              │
│ Version: u32                        │
│ Key Dictionary: (count: u32,        │
│   entries: [offset: u32, len: u32]*) │
│ Schema checksum: [u8; 32]           │
│ Record count: u32                   │
│ Page size: u32                      │
└──────────────────────────────────────┘
```

### 9.2 Key Dictionary

The key dictionary maps string keys to `u16` integer IDs. It is embedded in
the header so the binary is self-contained.

```
Key 0 → "alice"   (ID = 0)
Key 1 → "bob"     (ID = 1)
...
```

### 9.3 Bit-Mask Presence Layout

Each record starts with a `u32` bit-mask. Bit N is 1 if field ID N is present
in this record, 0 otherwise. Fields are packed sequentially in the order of
their bits — no padding for absent fields.

```
Record:
  u32      mask      0b00000000_00000000_00000000_00000101
  String   field[0]  (present — bit 0 = 1)
  String   field[2]  (present — bit 2 = 1)
                       (field[1] absent — bit 1 = 0, no storage)
```

### 9.4 Sparse Lookup

Finding field N in a record:

```
u32 mask = *record_ptr;
u32 shifted = mask & ((1 << N) - 1);
u32 offset = popcount(shifted);  // single-cycle hardware instruction
```

The field's data is at `record_ptr + 4 + offset` (after the mask).

### 9.5 Schema Evolution

Adding a field assigns the next available bit. Old records have bit = 0 for
the new field, which the popcount query handles automatically — the offset
computation skips absent fields without needing to rewrite old records.

### 9.6 Max Fields

128 fields per schema (u32 bit-mask, bits 0-127 reserved).

---

## 10. Parser Architecture (Reference)

### 10.1 `.dbvl` Parser

```
for each line in file:
    if line.is_empty() or line.starts_with("//"):
        continue
    if line.starts_with("#"):
        process_directive(line)
        continue
    fields = split_on_semicolons(line)
    for i, field in enumerate(fields):
        entry.set_field(i, field.trim())
    emit(entry)
```

No state machine beyond directive tracking. ~40 lines in C/Rust/Brief.

### 10.2 `.dbv` Parser

```
parse_schema_declaration()  // optional
parse_as_block() {
    expect("as"), expect(ident), expect("{")
    while not "}":
        skip comments
        if peek is "}": break
        if peek is "@":
            entry = parse_positional_entry()
        else:
            entry = parse_keyed_entry()
        add(entry)
    expect("}")
}
parse_keyed_entry() {
    key = eat_until(":")
    eat(":")
    entry = parse_field_list()
    entry.key = key
    return entry
}
parse_field_list() {
    while not ";" and not "}" and not peek(next_key):
        if peek is "{":
            field = parse_nested_block()
        else:
            field = eat_until(";")
        add(field)
    return entry
}
```

### 10.3 Error Handling

All parse errors must:
1. Report the file path and line number
2. Show the offending byte range
3. State what was expected vs. what was found

---

## 11. Examples

### 11.1 FFI Bindings (`.dbv`)

```
schema FnBinding (name) {
    name: String;
    impl: String;
};

as FnBinding {
    __json_parse:     __json_parse;     json::parse;
    __json_stringify: __json_stringify;  json::stringify;
    __json_is_object: __json_is_object;  json::is_object;
    __json_is_array:  __json_is_array;   json::is_array;
    __json_is_string: __json_is_string;  json::is_string;
    __json_is_number: __json_is_number;  json::is_number;
    __json_is_bool:   __json_is_bool;    json::is_bool;
    __json_is_null:   __json_is_null;    json::is_null;
    __json_get:       __json_get;        json::get;
    __json_set:       __json_set;        json::set;
    __json_keys:      __json_keys;       json::keys;
    __json_length:    __json_length;     json::length;
};
```

### 11.2 GLUE Adapter Registry (`.dbvl`)

```
#schema RegistryEntry from "glue/registry.dbv"
rust;  glue/rust/types.bv;   rs;  x86_64-unknown-linux-gnu;  { Int: int64_t; Float: double; Bool: bool; };
python; glue/python/types.bv;  py;  any;                       { Int: int64_t; Float: double; Bool: bool; };
node;  glue/node/types.bv;    js;  any;                       { Int: int64_t; Float: double; Bool: bool; };
```

### 11.3 Hardware Register Map (`.dbv`)

```
schema Register {
    offset: Int;
    access: String;
};

schema Device {
    base: Int;
    width: Int;
    registers: Register[];
};

as Device {
    gpio: 0x4000; 32; { @ 0; rw; @ 4; ro; @ 8; rw; };
    uart: 0x8000; 8;  { @ 0; rw; @ 1; ro; @ 2; rw; };
};
```

### 11.4 Standalone Entries

```
config: AppConfig { debug: true; budget: 256; threads: 4; };
target: Target { arch: x86_64; os: linux; };
```

---

## 12. Design Decisions

### 12.1 Why `;` Instead of `,`

Commas appear frequently in data (lists, function arguments). Semicolons are
rare in identifiers, paths, and type names. Using `;` as the universal
separator means bare tokens are the default and escapes are rarely needed.

### 12.2 Why No `/ /` Line Continuation

Every entry is self-contained. Line continuation would require the parser to
track state across lines, which conflicts with the single-pass streaming
design of `.dbvl` and the brace-delimited clarity of `.dbv`.

### 12.3 Why Schema is Never Inferred

Inference is unsound without heuristics. A field containing `42` could be
`Int`, `String`, `Float`, or `UInt`. The compiler must never guess — schema
must always be explicitly declared or imported. This eliminates an entire
class of silent miscompilation bugs.

### 12.4 Why `.dbvl` Has No Nested Blocks

Line-oriented format means one entry per line. Nested blocks belong in `.dbv`
where braces and multiple lines are expected. A `.dbvl` line with `{ }` would
require the parser to track brace depth across line boundaries, defeating the
purpose of the line-per-record design.

---

## 13. Migration from Legacy Syntax

The old Data Brief syntax (`docs/DATABRIEF.md`, `docs/DATABRIEF_GUIDE.md`) used
commas, quotes, and `schema { }` blocks with different token rules. Migration:

| Old | New |
|-----|-----|
| `field: "value",` | `field: value;` |
| `field: value,` | `field: value;` |
| `schema S { ... },` | `schema S { ... };` |
| `import "file.dbvs"` | `schema Name from "file.dbv"` |
| `@` not used | `@` for positional entries |
| `.dbvs` separate | schema inline in `.dbv` |

The canonicalization rules (§8) mean the compiler can mechanically convert old
files — the resulting binary is identical regardless of input format.

---

## 14. Future Directions

### 14.1 Schema Registry

A `.brief/schemas/` directory (similar to `.brief/registry/`) for storing
reusable schemas. `schema Person from "std/person.dbv"` resolves against the
registry when the relative path fails.

### 14.2 Byte Prefixes for Typed Bare Tokens

Optional type hint prefixes for bare tokens:
- `i42` — literal Int
- `f3.14` — literal Float
- `btrue` / `bfalse` — literal Bool

These are consumed by the schema-aware consumer, not the parser. The parser
sees them as bare tokens.

### 14.3 Streaming `.dbvl` with `#checkpoint`

A `#checkpoint N` directive that tells the parser "flush all prior entries,
this is a safe resumption point" — enabling streaming processing of
terabyte-scale `.dbvl` files.
