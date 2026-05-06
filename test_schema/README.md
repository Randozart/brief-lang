# Schema Import Validation Showcase

This directory contains examples for the `.dbvs` schema import validation feature.

## Files
- `hardware.dbvs`: A DBrief schema file defining hardware aliases.
- `valid.ebv`: A Brief file that correctly imports and uses aliases from the schema.
- `invalid_undefined.ebv`: A Brief file that imports the schema but uses a variable not defined in it.

## How to Test

### 1. Valid Case
Run the compiler on `valid.ebv`. It should succeed and generate Verilog.
```bash
./target/release/brief-compiler compile test_schema/valid.ebv
```

### 2. Invalid Case (Undefined Alias)
Run the compiler on `invalid_undefined.ebv`. It should fail with `error[HW007]`.
```bash
./target/release/brief-compiler compile test_schema/invalid_undefined.ebv
```

### 3. Missing Schema
Run the compiler on a file that imports a non-existent schema.
```bash
./target/release/brief-compiler compile test_schema/missing_schema.ebv
```
(You can create this by changing the import in `valid.ebv` to a fake filename).
