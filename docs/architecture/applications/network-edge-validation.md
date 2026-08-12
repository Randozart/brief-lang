# Network Edge Validation — Streaming `.dbvl` as a Wire Format

**Date:** 2026-07-26
**Status:** Aspirational / Concept

---

## 1. Motivation

Parsing untrusted data at a network boundary is the single largest source of
security vulnerabilities in distributed systems: buffer overflows, OOM crashes
from nested bomb payloads, and deserialization exploits. JSON, XML, CBOR, and
Protocol Buffers each have well-known attack surfaces at the parsing layer.

`.dbvl` (Data Briev Lines) has structural properties that make it a natural
candidate for **contract-driven edge validation**: a streaming, single-pass,
zero-allocation byte scanner that can reject invalid or incomplete payloads
before any application code touches the data.

---

## 2. The Standard Parsing Problem

Every network request goes through three phases:

```
[bytes arrive] → [parse into memory] → [validate structure] → [application]
```

In JSON, an attacker can send:

```json
{"user_id": 1, "price": 29.99}
```

The parser must:
1. Allocate a `HashMap` or `Value` tree for the entire object
2. Walk string keys to discover which fields exist
3. Allocate strings for each key
4. *Then* let application code check if `price` is present

If the attacker sends `[{},{},{},{}...]` for 100MB, the parser allocates
100MB of object trees before any validation happens. This is a classic OOM
vector — the parser cannot reject the payload until it has fully parsed it.

---

## 3. How `.dbvl` Changes the Equation

A `.dbvl` payload has a fixed schema known at the boundary:

```
// Expected schema: temperature: Float; humidity: Float; pressure: Float;
// Wire format (3 semicolons = 3 fields):
28.5; 63.2; 1013.25
```

### 3.1 Single-Pass Byte Scanner

The edge validator is a finite state machine with exactly two states:

```
OUTSIDE_FIELD:  scan until ';' or line end
INSIDE_MAP:    track brace depth for map values
```

Pseudo-code:

```
fn validate_line(line: &[u8], expected_fields: usize) -> Result<(), Error> {
    let mut count = 0;
    let mut depth = 0;
    for byte in line {
        match byte {
            b';' if depth == 0 => count += 1,
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => continue,
        }
        if depth < 0 { return Err("unmatched brace"); }
    }
    if count != expected_fields {
        return Err(format!("expected {} fields, got {}", expected_fields, count));
    }
    Ok(())
}
```

This is ~20 lines. No allocations. No key comparisons. No object tree. The
validator can run on a microcontroller, in a kernel module, or as the first
stage of a TCP server's accept loop.

### 3.2 Streaming Validation

Because `.dbvl` is line-oriented (`\n`-delimited), the validator can operate
on a streaming byte buffer:

```
[bytes arrive] → [scan for \n] → [validate line] → [route to application]
```

Each line is validated independently. If a line has the wrong number of
semicolons, it is rejected instantly — the remaining bytes in the buffer are
discarded without further processing. This prevents slow-read attacks and
line-bomb attacks.

### 3.3 No OOM Vector

The `.dbvl` line parser never allocates. It scans byte-by-byte, counting
semicolons and tracking brace depth. An attacker sending a 1GB single line
would be rejected at the first `\n` or timeout — the validator never needs to
store the payload in memory beyond a small staging buffer.

---

## 4. Contract-Driven Edge Validation

The schema defines the exact structure the endpoint expects:

```briev
schema SensorReading {
    temperature: Float;
    humidity: Float;
    pressure: Float;
};
```

The edge gateway reads the schema at startup and configures the validator
with `expected_fields = 3`. This is the **contract** — the sender and receiver
agree on structure before any data flows.

### 4.1 Per-Message Schema Switching

A single stream can carry multiple message types by switching schemas via the
`>schema` directive:

```
>schema SensorReading from "types/sensor.dbv"
28.5; 63.2; 1013.25;
29.1; 62.8; 1012.90;

>schema AlarmEvent from "types/alarm.dbv"
critical; temp_exceeded; boiler_room_a; 95.0;
```

The validator tracks the active schema and validates each line against it.
Schema switches are part of the data stream — no out-of-band negotiation.

### 4.2 Rejection Response

When validation fails, the endpoint returns a precise, machine-readable error:

```
Error: Line 3: expected 3 fields, got 2
```

The error identifies the exact byte offset (line number) and the contract
violation. The sender can correct and retry without ambiguity.

---

## 5. Security Properties

| Attack | JSON/CBOR | `.dbvl` |
|--------|-----------|---------|
| Nested bomb `[[[...]]]` | OOM crash | Rejected at first `}` (tracking depth) |
| Long string DoS | Allocates until OOM | Rejected at line-length limit |
| Key-name bomb | Allocates strings for every key | No keys in wire format |
| Schema drift | Silently parses wrong fields | Rejected at field-count mismatch |
| Slow loris | Holds connection with partial payload | Rejected at `\n` timeout |

---

## 6. Integration with Briev's Type System

The edge validator uses `.dbvl` for syntax validation only. Type validation
(Is `28.5` a valid Float? Is `1013.25` within range?) happens at the
application layer, where the schema's FieldType annotations provide the
type constraints.

This two-layer design keeps the hot-path byte scanner simple (counting
semicolons) while deferring semantic validation to the layer that has
access to the full type universe:

```
Edge (byte scanner):          counts fields, checks brace balance
Application (Briev node):     parses values, validates ranges, processes
```

---

## 7. Comparison to Existing Wire Formats

| Property | JSON | CBOR | Protobuf | `.dbvl` |
|----------|------|------|----------|---------|
| Human-readable | Yes | No | No | Yes |
| Streaming parse | No | Partial | Yes | Yes |
| Schema required | No | No | Yes | Optional |
| Zero-alloc validation | No | No | No | Yes |
| Per-field rejection | After parse | After parse | After parse | At semicolon boundary |
| Line-delimited | JSONL | No | No | Native |

---

## 8. Open Questions

1. **Line length limit**: Should the edge validator enforce a maximum line
   length to prevent memory exhaustion on the staging buffer?

2. **Schema negotiation**: Should the `>schema` directive be authenticated
   (signed schema hash) to prevent schema-injection attacks?

3. **Partial line handling**: What happens when a line is split across TCP
   segments? The validator must buffer incomplete lines until `\n`.

4. **Backpressure signaling**: Should validation errors trigger TCP backpressure
   (e.g., closing the connection) or allow the sender to retry with corrected
   data?

5. **Compatibility with TLS**: `.dbvl` is plain text. For encrypted transport,
   the validator runs after TLS termination. This is the same model as HTTP/2
   — terminate TLS, then parse.
