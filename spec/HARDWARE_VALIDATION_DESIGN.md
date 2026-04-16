# Hardware Validation for Embedded Brief (.ebv)

## Problem Statement

The Brief compiler currently allows `.ebv` files to compile to SystemVerilog even when the generated hardware contains **dead code** - logic that will be optimized away by synthesis tools (Yosys) because:

1. **Orphan Variables**: Variables that are never written to by any transaction
2. **Untriggerable Transactions**: Transactions whose preconditions can never be satisfied

This creates a false sense of correctness - the compiler succeeds, but the hardware doesn't actually do anything (0 LUTs after synthesis).

## Design Goals

1. **Fail Fast for .ebv**: Errors must be emitted during compilation, not after synthesis
2. **Universal Detection**: Check ALL variables and transactions, regardless of whether they have addresses
3. **Actionable Messages**: Error messages must explain what's wrong and how to fix it
4. **Non-Breaking for Other Targets**: Non-.ebv files should receive warnings, not errors

## Validation Rules

### Rule 1: Orphan Variable Detection

**Definition**: A `StateDecl` is orphaned if it is never written to by any transaction or trigger.

**Validation Logic**:
- Collect all write operations: `&V = ...` in all transactions
- Collect all trigger writes: triggers that can set V
- If (write operations is empty AND trigger writes is empty):
  - If .ebv target: ERROR
  - Else: WARNING

**Exception**: Variables with explicit initial values are considered "written" (the initial value counts), but still need to be checked for whether they are used anywhere in logic.

### Rule 2: Untriggerable Transaction Detection

**Definition**: A transaction is untriggerable if its precondition can never be satisfied by any combination of triggers and other transaction postconditions.

**Validation Logic**:
- Extract all variables referenced in precondition
- Check if any trigger can set those variables directly
- Check if any OTHER transaction can set those variables in its postcondition
- If (no path exists to satisfy precondition):
  - If .ebv target: ERROR
  - Else: WARNING

**Exception**: Transactions with precondition `true` are always triggerable.

### Rule 3: Variable Usage Validation

**Definition**: A variable with an initial value must be used in at least one transaction or computation.

**Validation Logic**:
- Collect all read operations: V in expressions
- If (read operations is empty):
  - WARNING (variable exists but is never used)

## Implementation Architecture

### New Module: `src/hardware_validator.rs`

```rust
use crate::ast::{Program, Transaction, StateDecl, TriggerDeclaration, Expr};
use crate::errors::{Diagnostic, Severity, Span};

pub struct HardwareValidator;

impl HardwareValidator {
    /// Main entry point - validates program for hardware targets
    pub fn validate(program: &Program, target: &str, is_ebv: bool) -> Vec<Diagnostic> {
        // Build dependency graphs
        let write_graph = WriteGraph::build(program);
        let precond_graph = PreconditionGraph::build(program);
        let trigger_graph = TriggerGraph::build(program);
        
        let mut diagnostics = Vec::new();
        
        // Rule 1: Check for orphan variables
        diagnostics.extend(Self::check_orphan_variables(program, &write_graph, &trigger_graph, is_ebv));
        
        // Rule 2: Check for untriggerable transactions
        diagnostics.extend(Self::check_untriggerable_transactions(program, &precond_graph, &write_graph, &trigger_graph, is_ebv));
        
        // Rule 3: Check for unused variables with initial values
        diagnostics.extend(Self::check_unused_variables(program, is_ebv));
        
        diagnostics
    }
}
```

## Error Codes

- **EBV001**: Orphan Variable - Variable never written by any transaction
- **EBV002**: Untriggerable Transaction - Transaction precondition can never be satisfied
- **EBV003**: Unused Variable - Variable has initial value but is never used

## Integration

Called from `main.rs` before Verilog generation when target is `verilog`.