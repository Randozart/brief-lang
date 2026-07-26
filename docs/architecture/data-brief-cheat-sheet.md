# Data Brief — Syntax Cheat Sheet

**One-page reference for `.dbv` and `.dbvl` formats.**

---

## Tokens

| Symbol | Role |
|--------|------|
| `;` | Terminates every field value |
| `:` | Binds key to entry (`key: value...;`) or map pair (`k: v;`) |
| `{ }` | Groups nested sub-record or array |
| `>` | **Line-start:** directive (`.dbvl`) / **Block-scoped:** positional entry (`.dbv`) |
| `//` | Comment to end of line |
| `" "` | Quoted string (requires `--quoted` parser flag) |

Bare tokens are the default — no quotes needed for plain strings, numbers, paths.

---

## Schema Definition

```
schema Name (key_field) {
    field_name: Type;
    field_name?: Type;     // optional
};
```

| Type | Meaning |
|------|---------|
| `String` | Text of any length |
| `Int` | Signed integer |
| `Float` | IEEE 754 double |
| `Bool` | `true` or `false` |
| `T[]` | Array of type T (e.g. `String[]`, `Int[]`) |
| `Map` | Key-value pairs (`{ k: v; k: v; }`) |
| `Option[T]` | Optional value of type T |

Import a schema: `schema Name from "path.dbv";`

---

## Entries (`.dbv`)

### Keyed entry
```
key_name: field; field; { nested; }; field;
```

### Positional entry (inside `as {}`)
```
> field; field; { nested; }; field;
```

### Standalone inline entry
```
key: SchemaName { field; field; };
```

### Nested block as array value
```
entry: name; age; { elem1; elem2; elem3; };
```

### Key-value map inside block
```
{ key1: val1; key2: val2; key3: val3; };
```

---

## Data Groups (`.dbv`)

```
schema SchemaName {
    field1: String;
    field2: Int;
};

as SchemaName {
    key_a: value_a; 42;
    key_b: value_b; 99;
};
```

---

## Directives (`.dbvl`)

```
>schema Name from "schemas/types.dbv"
>import "other.dbvl"
>encoding utf-8
```

---

## Lines (`.dbvl`)

Each line is one positional entry. Fields separated by `;`:

```
>schema Person from "schemas/person.dbv"
Alice Smith; 30;
Bob; 25;
Charlie; 40;
```

Lines without `>` are data. Lines starting with `>` (before data) are directives.

---

## File-type overview

| Extension | Structure | Best for |
|-----------|-----------|----------|
| `.dbv` | Schema + `as {}` blocks with keyed or positional entries | Hierarchical configs, structured data, schemas |
| `.dbvl` | Lines of `;`-delimited fields, optional schema directive | Registries, logs, streaming data, flat tables |

---

## Common patterns

**Array of strings:** `tags: String[];` → data: `{ a; b; c; }`

**Map:** `meta: Map;` → data: `{ key1: val1; key2: val2; }`

**Optional field:** `desc?: String;` → data: omit or leave empty

**Nested record:** `address: Address;` → data: `{ street; city; zip; }`
