# Data Briv — Data Files, Schema, and Line Data

**Date:** 2026-06-24
**Status:** Fully implemented

## Overview

Data Briv (`.dbv`, `.dbvs`, `.dbvl`) is Briv's universal data format. It serves the same role as JSON, XML, or TOML in other ecosystems but is parsed by the Briv compiler itself.

## File Types

| Extension | Name | Purpose |
|-----------|------|---------|
| `.dbv` | Data Briv | Structured data (think JSON/YAML) |
| `.dbvs` | Data Briv Schema | Validation schema for `.dbv` and `.dbvl` files |
| `.dbvl` | Data Briv Lines | Line-oriented data (one record per line, think JSONL/NDJSON) |

## DBV — Structured Data

```briv
// config.dbv — Application configuration
{
    "app": "briv-compiler",
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

```briv
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

```briv
// adapters.dbvl — One adapter per line
rust glue/adapters/rust.bv .rs src/
python glue/adapters/python.bv .py lib/
node  glue/adapters/node.bv .js lib/
```

Each line is a record. Fields are separated by whitespace. Quoted strings allow spaces in values. Comments with `//` are supported per-line.

## DBVL Keyed Access

Dbvl tables support O(1) key lookup when accessed with a `FILTER(_field_0 == "key")` projection:

```briv
// In .bv:
let table = import "adapters.dbvl";
let entry : table { FILTER(_field_0 == "rust"); };
```

## Import

Data Briv files are imported into Briv programs:

```briv
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
- **Data exchange**: Briv-to-Briv data serialization
