# Briv to COBOL Examples

This folder contains Briv source files (`*.bv`) and their generated COBOL equivalents.

## Running Examples

```bash
# Compile to COBOL
cargo run --bin briv-compiler -- cobol examples/cobol/withdraw.bv --out examples/cobol/output/

# View generated COBOL
cat examples/cobol/output/withdraw.cbl
```

## Examples

| File | Description |
|------|-------------|
| `simple_contract.bv` | Minimal contract with counter |
| `withdraw.bv` | Bank account withdrawal with guards |
| `transfer.bv` | Multi-party transfer system |
| `bank_system.bv` | Full banking system with audit trail |

## Generated Output

The COBOL files are generated to `examples/cobol/output/` by default.
This folder is gitignored - regenerate with the CLI.