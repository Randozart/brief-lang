# Data Brief — Data Files, Schema, and Line Data

**Date:** 2026-06-24
**Status:** Fully implemented

## Overview

Data Brief (`.dbv`, `.dbvs`, `.dbvl`) is Brief's universal data format. It serves the same role as JSON, XML, or TOML in other ecosystems but is parsed by the Brief compiler itself.

## File Types

| Extension | Name | Purpose |
|-----------|------|---------|
| `.dbv` | Data Brief | Structured data (think JSON/YAML) |
| `.dbvs` | Data Brief Schema | Validation schema for `.dbv` and `.dbvl` files |
| `.dbvl` | Data Brief Lines | Line-oriented data (one record per line, think JSONL/NDJSON) |

## DBV — Structured Data

```brief
// config.dbv — Application configuration
{
    "app": "brief-compiler",
    "version": "0.11.0",
    "debug": false,
    "optimization": {
        "budget": 256,
        "simplify": true
    }
}
```

Supports: strings, integers, floats, booleans, null, arrays, objects (key-value maps), comments with `//`.

## DBVS — Schema Validation

```brief
// schema.dbvs — Validates config.dbv
schema Config {
    app: String required,
    version: String required,
    debug: Bool default(false),
    optimization: {
        budget: Int : [0..1000000],
        simplify: Bool default(true)
    }
}
```

A `.dbvs` schema file declares expected structure, types, optionality, defaults, and constraints. The compiler validates `.dbv` and `.dbvl` files against their schema at compile time.

## DBVL — Line Data

```brief
// adapters.dbvl — One adapter per line
rust glue/adapters/rust.bv .rs src/
python glue/adapters/python.bv .py lib/
node  glue/adapters/node.bv .js lib/
```

Each line is a record. Fields are separated by whitespace. Quoted strings allow spaces in values. Comments with `//` are supported per-line.

## DBVL Keyed Access

Dbvl tables support O(1) key lookup when accessed with a `FILTER(_field_0 == "key")` projection:

```brief
// In .bv:
let table = import "adapters.dbvl";
let entry : table { FILTER(_field_0 == "rust"); };
```

## Import

Data Brief files are imported into Brief programs:

```brief
// Import entire data file
let config = import "config.dbv";

// Import DBVL table
let adapters = import "adapters.dbvl";

// Access with subtype projections
let entry : adapters { FILTER(_field_0 == "python"); };
```

## Common Uses

- **Configuration**: Application config stored as `.dbv`, validated by `.dbvs`
- **Hardware maps**: MMIO register maps (`hardware.dbv`)
- **Adapter registries**: GLUE language adapter indexing (`adapters.dbvl`)
- **Data exchange**: Brief-to-Brief data serialization
