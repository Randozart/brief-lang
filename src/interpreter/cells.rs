// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// ── Cell / Thread / Virtual Memory ───────────────────────────────────
//
// This submodule owns the sandboxed virtual heap, cell convergence logic,
// persistent cell lifecycle, and cell-to-cell wire propagation.
//
// Extracted from the monolithic interpreter/mod.rs during Phase 4.
// Requires flat code (max 2 nesting) with guard clauses throughout.
//
// 2026-07-12: Rewrote to extract deeply-nested blocks into named helpers.
// Each helper is independently testable and fits in ~20 lines.

use super::intrinsics::{f64_to_bits, i64_to_bits, value_as_i64};
use super::{Interpreter, RuntimeError, Value};
use crate::ast::*;
use crate::features::block::BlockExpr;
use crate::features::binary_op::BinaryOpExpr;
use crate::features::call::CallExpr;
use crate::features::collection::{
    ListLiteralExpr, MapLiteralExpr, MultiSliceExpr, SetLiteralExpr, SliceExpr,
};
use crate::features::field::{FieldAccessExpr, ObjectLiteralExpr, StructInstanceExpr};
use crate::features::pattern::{MatchExpr, PatternMatchExpr};
use crate::features::projection::ProjectionExpr;
use crate::features::sigcall::SigCallExpr;
use crate::features::subtype::SubtypeProjectionExpr;
use crate::features::tuple::{TupleDestructureExpr, TupleExpr};
use crate::features::unary_op::UnaryOpExpr;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

// ── VirtualHeap ──────────────────────────────────────────────────────
// Compile-time memory sandbox (same pattern as Miri).
// Maps virtual addresses → byte blocks. No real memory allocation.

/// Sandboxed virtual memory space for compile-time execution.
/// Maps virtual addresses to allocated byte blocks.
/// Used by List, HashMap, Box, and any type that manages heap memory.
/// Same pattern as Miri (Rust's compile-time interpreter).
/// 2026-07-11: Phase 7.5 — Bits thesis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VirtualHeap {
    /// Virtual address → allocated byte block.
    allocations: std::collections::HashMap<u64, Vec<u8>>,
    /// Next free virtual address. Starts at 0x1000 (page-aligned convention).
    next_address: u64,
}

impl VirtualHeap {
    /// Create an empty heap with base address 0x1000.
    pub fn new() -> Self {
        VirtualHeap {
            allocations: std::collections::HashMap::new(),
            next_address: 0x1000,
        }
    }

    /// Allocate a block and return its virtual address.
    /// The address is bumped by the byte length (minimum 1).
    pub fn alloc(&mut self, bytes: &[u8]) -> u64 {
        let addr = self.next_address;
        self.allocations.insert(addr, bytes.to_vec());
        let bump = bytes.len().max(1) as u64;
        self.next_address = addr.wrapping_add(bump);
        addr
    }

    /// Read `size` bytes from a virtual address.
    /// Returns None if the address is not allocated.
    pub fn read(&self, addr: u64, size: u64) -> Option<&[u8]> {
        let block = self.allocations.get(&addr)?;
        let end = block.len().min(size as usize);
        Some(&block[..end])
    }

    /// Write `data` to a virtual address.
    /// Errors with `()` if the address is not allocated.
    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), ()> {
        let block = self.allocations.get_mut(&addr).ok_or(())?;
        let end = data.len().min(block.len());
        block[..end].copy_from_slice(&data[..end]);
        Ok(())
    }

    /// Free a virtual address. No-op if already freed.
    pub fn free(&mut self, addr: u64) {
        self.allocations.remove(&addr);
    }
}

// ── Cell Channel ─────────────────────────────────────────────────────
// Thread-safe communication between cell threads and the reactor loop.
// Lock-free on hot path (dirty flag is atomic).

/// Thread-safe channel for a cell thread to communicate output changes
/// to the parent reactor loop. Lock-free on the hot path (dirty flag is atomic).
#[derive(Debug, Clone)]
pub struct CellChannel {
    /// Most recent output values by port name. Locked only on read.
    pub outputs: Arc<Mutex<HashMap<String, Value>>>,
    /// Set to true when outputs change (atomically observed by reactor).
    pub changed: Arc<AtomicBool>,
    /// Set to true when the reactor wants the thread to exit.
    pub terminate: Arc<AtomicBool>,
}

impl CellChannel {
    /// Create a channel with empty outputs and no pending change.
    pub fn new() -> Self {
        CellChannel {
            outputs: Arc::new(Mutex::new(HashMap::new())),
            changed: Arc::new(AtomicBool::new(false)),
            terminate: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ── Persistent Cell Instance ─────────────────────────────────────────
// Runtime state for one persistent cell.

/// Runtime state for one registered persistent cell.
/// Holds the cell's private state, tick counter, and communication channel.
#[derive(Debug, Clone)]
pub struct PersistentCellInstance {
    /// The cell definition (fields, transactions, output type).
    pub cell_def: CellDef,
    /// The cell's private state map (identifier → Value).
    pub state: HashMap<String, Value>,
    /// State snapshot from the previous convergence pass (for change detection).
    pub prior_state: HashMap<String, Value>,
    /// Cached output values by port name — used to detect changes.
    pub output_cache: HashMap<String, Value>,
    /// Number of reactor-loop iterations this cell has been alive for.
    pub tick_counter: u64,
    /// Minimum main loop iterations between ticks (0 = every iteration).
    pub tick_interval: u64,
    /// Channel for communicating outputs to parent thread.
    pub channel: CellChannel,
}

/// A trigger binding: maps a parent-level trigger name to a cell's output port.
#[derive(Debug, Clone)]
pub struct TrgBindingReg {
    /// Parent-level trigger name (e.g. "connected").
    pub trigger_name: String,
    /// Name of the cell that produces this trigger's value.
    pub cell_name: String,
    /// Port name on the cell.
    pub port_name: String,
    /// Optional type constraint on the port.
    pub ty: Option<Type>,
}

/// A static wire connecting one cell's output port to another cell's input parameter.
/// After each tick of the source cell, the output value is automatically copied to
/// the target cell's parameter state slot. This enables cell-to-cell dataflow without
/// parent-state mediation.
#[derive(Debug, Clone)]
pub struct CellWire {
    pub from_cell: String,
    pub from_port: String,
    pub to_cell: String,
    pub to_param: String,
}

// ── Convergence Helpers ──────────────────────────────────────────────
// The core convergence loop for one transaction's body.
// Extracted from call_cell and tick_persistent_cells to eliminate arrow code.

/// Execute a single transaction's body against the interpreter's current state.
///
/// Side-effects: mutates `interp.state`, `interp.prior_state`, `interp.return_value`.
///
/// Returns `Ok(None)` if the transaction ran without termination.
/// Returns `Ok(Some(ret_val))` if the transaction hit a `term` or `term!`.
/// Returns `Err(e)` if a statement errored.
/// Execute a single statement within a transaction body.
///
/// Rewrites identifiers for cell scoping, executes the statement, and
/// returns `Ok(None)` if the statement completed without terminating,
/// `Ok(Some(ret_val))` if the statement triggered term/term!,
/// or `Err(e)` if the statement errored (including Escape which
/// restores prior state).
fn execute_single_txn_statement(
    interp: &mut Interpreter,
    stmt: &Statement,
    uid: usize,
    cell_name: &str,
) -> Result<Option<Value>, RuntimeError> {
    let rewritten = interp.rewrite_statement_identifiers(stmt, uid, cell_name);
    match interp.exec_stmt(&rewritten) {
        Err(RuntimeError::Escaped) => {
            interp.state = interp.prior_state.clone();
            Ok(None)
        }
        Err(e) => Err(e),
        Ok(()) => {
            // Guard: check if term/term! produced a return value.
            if !interp.return_value.is_some() {
                return Ok(None);
            }
            let ret_val = interp.return_value.take();
            Ok(ret_val)
        }
    }
}

fn execute_txn_body(
    interp: &mut Interpreter,
    txn: &Transaction,
    uid: usize,
    cell_name: &str,
) -> Result<Option<Value>, RuntimeError> {
    // Rewrite precondition to use cell-scoped identifiers.
    let pre = interp.rewrite_identifiers(&txn.contract.pre_condition, uid, cell_name);
    let pre_val = interp.eval_expr(&pre)?;

    // Guard: only fire if precondition is true (byte 1).
    if pre_val != Value::Bits(vec![1u8]) {
        return Ok(None);
    }

    // Snapshot prior state before executing the body.
    interp.prior_state = interp.state.clone();
    interp.return_value = None;

    // Evaluate each statement in the transaction body.
    // Each statement either: errors, returns (terminates), or continues.
    for stmt in &txn.body {
        let outcome = execute_single_txn_statement(interp, stmt, uid, cell_name)?;
        // Guard: termination via term/term! propagates the return value.
        if let Some(ret_val) = outcome {
            return Ok(Some(ret_val));
        }
    }

    // Evaluate postcondition to determine if the cell converged.
    let post = interp.rewrite_identifiers(&txn.contract.post_condition, uid, cell_name);
    let post_val = interp.eval_expr(&post)?;

    // The cell fired if the postcondition is true AND state changed.
    if post_val == Value::Bits(vec![1u8]) && interp.state != interp.prior_state {
        // Postcondition satisfied — convergence achieved.
        // No further action needed; caller checks `executed` flag.
    }

    Ok(None)
}

/// Initialize a cell field's default value from its type.
///
/// Returns the zero-value for known types (Int→0, Bool→0, Float→0.0,
/// Char→'\0', String→empty). Returns Void for unknown types.
fn init_field_default(field_ty: &Type) -> Value {
    // Check known type names for zero-value initialization.
    // This is a temporary heuristic until the type system provides
    // a `default_value` property on every type.
    match field_ty {
        Type::Custom(t) if t == "Int" => Value::Bits(i64_to_bits(0)),
        Type::Custom(t) if t == "Bool" => Value::Bits(vec![0u8]),
        Type::Custom(t) if t == "Float" => Value::Bits(f64_to_bits(0.0)),
        Type::Custom(t) if t == "Char" => Value::Bits((0u32).to_le_bytes().to_vec()),
        Type::Custom(t) if t == "String" => Value::Bits(Vec::new()),
        _ => Value::Void,
    }
}

/// Register a cell output name for trigger propagation.
fn register_output_trigger(
    interp: &mut Interpreter,
    cell_name: &str,
    port_name: &str,
    value: Value,
) {
    // Search trigger bindings that reference this cell+port.
    for trg in &interp.trg_bindings.clone() {
        if trg.cell_name == cell_name && trg.port_name == port_name {
            interp.state.insert(trg.trigger_name.clone(), value.clone());
        }
    }
}

// ── Convergence Pass (Free Function) ─────────────────────────────────
// Runs one convergence pass on a cell's private state maps.
// Used by the persistent-cell tick infrastructure.

/// Run one convergence pass on a cell's private state.
///
/// Iterates over all transactions in the cell definition. For each
/// transaction whose precondition is satisfied, executes the body
/// and checks the postcondition. Returns true if the cell fired
/// (state changed and postcondition satisfied).
///
/// This is the outer loop for persistent cells (which have their own
/// state maps passed explicitly). The `call_cell` method uses a
/// different code path (interpreter-state-based).
pub fn cell_convergence_pass(
    interp: &mut Interpreter,
    cell_def: &CellDef,
    cell_name: &str,
    state: &mut HashMap<String, Value>,
    prior_state: &mut HashMap<String, Value>,
) -> bool {
    let mut fired = false;

    for txn in &cell_def.transactions {
        // Evaluate precondition against the cell's state map.
        let pre = interp.rewrite_identifiers(&txn.contract.pre_condition, 0, cell_name);
        let pre_val = match interp.eval_expr_in_state(&pre, state) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Guard: skip if precondition is not true.
        if pre_val != Value::Bits(vec![1u8]) {
            continue;
        }

        // Save prior state for change detection.
        *prior_state = state.clone();
        let mut return_val = None;
        let mut terminated = false;

        // Execute each statement against the cell's state.
        for stmt in &txn.body {
            let rewritten = interp.rewrite_statement_identifiers(stmt, 0, cell_name);
            match interp.exec_stmt_in_state(&rewritten, state, &mut return_val) {
                Ok(()) if return_val.is_some() => {
                    terminated = true;
                    break;
                }
                Ok(()) => {}
                Err(_) => break,
            }
        }

        // If terminated, stop processing further transactions.
        if terminated {
            break;
        }

        // Check postcondition for convergence.
        let post = interp.rewrite_identifiers(&txn.contract.post_condition, 0, cell_name);
        let post_val = match interp.eval_expr_in_state(&post, state) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Cell fired if postcondition is true AND state changed.
        if post_val == Value::Bits(vec![1u8]) && state != prior_state {
            fired = true;
        }
    }

    fired
}

/// Run a cell tick: convergence pass + output sync.
///
/// Returns a tuple (fired, output_map). The output map contains the
/// cell's latest output values by port name.
pub fn cell_tick(
    interp: &mut Interpreter,
    cell_def: &CellDef,
    cell_name: &str,
    state: &mut HashMap<String, Value>,
    prior_state: &mut HashMap<String, Value>,
) -> (bool, HashMap<String, Value>) {
    let fired = cell_convergence_pass(interp, cell_def, cell_name, state, prior_state);
    let mut outputs = HashMap::new();

    // Extract output values if the cell declares an output type.
    let Some(ref ot) = cell_def.output_type else {
        return (fired, outputs);
    };

    let names = interp.extract_output_names(ot);
    for port_name in names {
        let key = format!("{}${}.{}", cell_name, 0, port_name);
        let Some(val) = state.get(&key) else {
            continue;
        };
        outputs.insert(port_name, val.clone());
    }

    (fired, outputs)
}

/// Result of a single inline convergence pass across all transactions.
/// Used by `run_inline_txn_loop` to communicate termination/fire status.
struct InlineConvergence {
    terminated: bool,
    fired: bool,
}

/// Outcome of a single convergence iteration across all transactions.
///
/// Used by `run_cell_convergence_iteration` to signal the outer loop
/// what action to take next. Avoids the double-for-loop arrow pattern
/// by encoding all possible iteration results in a flat enum.
enum ConvergenceOutcome {
    /// A transaction terminated via term/term! with a return value.
    Terminated(Value),
    /// No transaction fired (preconditions all false). Convergence reached.
    Converged,
    /// At least one transaction fired (postcondition true + state changed).
    Fired,
}

// ── Impl Interpreter — Cell Methods ──────────────────────────────────

impl Interpreter {
    /// Register a persistent cell with optional tick frequency.
    ///
    /// Initializes the cell's state from field defaults + input args,
    /// creates a `PersistentCellInstance`, and optionally spawns a
    /// background thread for autonomous ticking.
    ///
    /// Returns the cell's name.
    pub fn register_persistent_cell(
        &mut self,
        cell_def: &CellDef,
        args: &[Value],
        tick_hz: Option<u64>,
    ) -> Result<String, RuntimeError> {
        let name = cell_def.name.clone();
        let mut state = HashMap::new();

        // Initialize fields from defaults or zero values.
        for field in &cell_def.fields {
            let key = format!("{}${}.{}", cell_def.name, 0, field.name);
            let value = match &field.default {
                Some(expr) => self.eval_expr(expr)?,
                None => init_field_default(&field.ty),
            };
            state.insert(key, value);
        }

        // Bind input arguments with uid=0 prefix.
        for ((param_name, _), arg) in cell_def.parameters.iter().zip(args.iter()) {
            let key = format!("{}${}.{}", cell_def.name, 0, param_name);
            state.insert(key, arg.clone());
        }

        // Compute tick interval from Hz value (0 = every iteration).
        let tick_interval = match tick_hz {
            Some(hz) if hz > 0 => (1_000_000_000 / hz).max(1) as u64,
            _ => 0,
        };

        let instance = PersistentCellInstance {
            cell_def: cell_def.clone(),
            state,
            prior_state: HashMap::new(),
            output_cache: HashMap::new(),
            tick_counter: 0,
            tick_interval,
            channel: CellChannel::new(),
        };

        let chan = instance.channel.clone();
        self.persistent_cells.insert(name.clone(), instance);

        // Spawn background thread if tick_hz > 0.
        if tick_hz.unwrap_or(0) > 0 {
            let tick_ns = (1_000_000_000 / tick_hz.unwrap()).max(1) as u64;
            Self::spawn_cell_thread(self.clone(), cell_def.clone(), name.clone(), chan, tick_ns);
        }

        Ok(name)
    }

    /// Spawn a background thread that ticks a persistent cell at a fixed interval.
    ///
    /// The thread runs a convergence loop, sleeping `tick_ns` nanoseconds between
    /// each tick. When the cell fires, it writes outputs to the channel.
    fn spawn_cell_thread(
        mut interp: Interpreter,
        cell_def: CellDef,
        cell_name: String,
        chan: CellChannel,
        tick_ns: u64,
    ) {
        // 2026-07-11: Thread spawning is a special case — the `thread::spawn`
        // closure captures the cell's state by value. Deep nesting is unavoidable
        // here because the closure body is a separate execution context.
        // Each level inside the closure is a distinct responsibility:
        //   1. thread::spawn (thread boundary)
        //   2. loop (tick loop)
        //   3. cell_tick (convergence + output sync)
        //   4. conditional lock (output update)
        // The closure body uses early returns via `continue` to avoid arrow code.
        thread::spawn(move || {
            let mut state = HashMap::new();
            let mut prior_state = HashMap::new();

            // Converge repeatedly until the reactor signals termination.
            loop {
                // Guard: exit if reactor requested termination.
                if chan.terminate.load(Ordering::Relaxed) {
                    return;
                }

                // Wait for the tick interval before converging.
                thread::sleep(std::time::Duration::from_nanos(tick_ns));

                // Run one convergence pass.
                let (fired, outputs) =
                    cell_tick(&mut interp, &cell_def, &cell_name, &mut state, &mut prior_state);

                // Guard: only write outputs if the cell fired.
                if !fired {
                    continue;
                }

                // Publish outputs to the channel for the reactor loop to consume.
                let mut locked = chan.outputs.lock().unwrap();
                *locked = outputs;
                chan.changed.store(true, Ordering::SeqCst);
            }
        });
    }

    /// Evaluate a single internal trigger declaration and return its current value.
    ///
    /// Handles known `LinkRef` sources:
    /// - `Stdin` → reads one keypress, returns Char
    /// - `Timer` → returns current time (not yet implemented)
    /// Unknown sources return `Value::Void`.
    fn evaluate_internal_trigger(
        &mut self,
        trg: &TriggerDeclaration,
    ) -> Result<Value, RuntimeError> {
        let val = match &trg.address {
            LinkRef::Stdin => {
                // Call the TtyReadKey intrinsic to read one keypress.
                let expr = Expr::IntrinsicCall {
                    intrinsic: Intrinsic::TtyReadKey,
                    args: vec![],
                };
                let raw = self.eval_expr(&expr)?;
                let n = value_as_i64(&raw).unwrap_or(-1);

                // Guard: -1 means no key available.
                if n == -1 {
                    Value::Bits((0u32).to_le_bytes().to_vec())
                } else {
                    Value::Bits((n as u8 as u32).to_le_bytes().to_vec())
                }
            }
            // Fallback: unknown trigger type returns void.
            _ => Value::Void,
        };

        Ok(val)
    }

    /// Call a cell (transient or persistent) and return its output.
    ///
    /// For persistent cells: registers if not yet registered, returns
    /// the first output port's current value.
    ///
    /// For transient cells: sets up a temporary state, runs the convergence
    /// loop, and returns the designated output.
    ///
    /// 2026-07-11: The transient path saves/restores interpreter state around
    /// cell execution. This is intentional — transient cells run in the
    /// interpreter's state space, not their own.
    pub(crate) fn call_cell(
        &mut self,
        cell_def: &CellDef,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        // ── Persistent cell fast path ──────────────────────────────────
        if cell_def.is_persistent {
            return self.call_persistent_cell(cell_def, args);
        }

        // ── Transient cell path ────────────────────────────────────────
        self.call_transient_cell(cell_def, args)
    }

    /// Fast path for persistent cell calls.
    ///
    /// Registers the cell if it hasn't been registered yet, then returns
    /// the first output port's current value from the persistent instance.
    fn call_persistent_cell(
        &mut self,
        cell_def: &CellDef,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        // Register on first call.
        if !self.persistent_cells.contains_key(&cell_def.name) {
            self.register_persistent_cell(cell_def, args, None)?;
        }

        // Look up the persistent instance and return its first output.
        let Some(instance) = self.persistent_cells.get(&cell_def.name) else {
            return Ok(Value::Void);
        };

        let names = match &cell_def.output_type {
            Some(ot) => self.extract_output_names(ot),
            None => return Ok(Value::Void),
        };

        let first_name = match names.first() {
            Some(n) => n,
            None => return Ok(Value::Void),
        };

        let key = format!("{}${}.{}", cell_def.name, 0, first_name);
        let val = instance.state.get(&key).cloned().unwrap_or(Value::Void);
        Ok(val)
    }

    /// Run a single iteration of the convergence loop across all transactions.
    ///
    /// Evaluates every transaction in the cell definition. Each transaction's
    /// body is executed by `execute_txn_body`. Returns a `ConvergenceOutcome`
    /// that tells the outer loop whether to terminate, break, or continue.
    ///
    /// Saved state references are passed for error-recovery restoration.
    /// 2026-07-12: Extracted from `call_transient_cell` to eliminate the
    /// double-for-loop arrow pattern (was for→for→match, now flat).
    fn run_cell_convergence_iteration(
        &mut self,
        cell_def: &CellDef,
        uid: usize,
        saved_state: &HashMap<String, Value>,
        saved_prior: &HashMap<String, Value>,
        saved_return: &Option<Value>,
    ) -> Result<ConvergenceOutcome, RuntimeError> {
        let mut any_fired = false;

        for txn in &cell_def.transactions {
            let outcome = execute_txn_body(self, txn, uid, &cell_def.name)?;

            match outcome {
                Some(ret_val) => {
                    // Transaction terminated via term/term!.
                    // Restore saved state and propagate the return value.
                    self.state = saved_state.clone();
                    self.prior_state = saved_prior.clone();
                    self.return_value = saved_return.clone();
                    return Ok(ConvergenceOutcome::Terminated(ret_val));
                }
                None => {
                    // Transaction ran without terminating. Check postcondition.
                    let any_fired_this_txn =
                        Self::check_transaction_fired(self, txn, uid, &cell_def.name);
                    if any_fired_this_txn {
                        any_fired = true;
                    }
                }
            }
        }

        // No termination occurred. Signal convergence or fired.
        if any_fired {
            Ok(ConvergenceOutcome::Fired)
        } else {
            Ok(ConvergenceOutcome::Converged)
        }
    }

    /// Check whether a non-terminating transaction fired (postcondition
    /// true + state changed). Pure helper extracted to avoid nesting.
    ///
    /// 2026-07-12: Separated from the convergence iteration loop to keep
    /// each function at max 2 nesting levels. The state-change check uses
    /// the interpreter's `prior_state` which was snapshotted before the
    /// transaction body was executed by `execute_txn_body`.
    fn check_transaction_fired(
        interp: &mut Interpreter,
        txn: &Transaction,
        uid: usize,
        cell_name: &str,
    ) -> bool {
        // Evaluate the postcondition to detect state change.
        let post = interp.rewrite_identifiers(&txn.contract.post_condition, uid, cell_name);
        let post_val = match interp.eval_expr(&post) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // Guard: postcondition must be true for fire detection.
        if post_val != Value::Bits(vec![1u8]) {
            return false;
        }

        // The transaction body executed (precondition was true at some point).
        // Check if state actually changed from the prior snapshot.
        // execute_txn_body already updated prior_state before the body.
        // If state differs from prior_state, the body had an effect → fired.
        interp.state != interp.prior_state
    }

    /// Transient cell convergence loop.
    ///
    /// Saves the interpreter's current state, initializes cell-local state,
    /// runs convergence iterations until a transaction terminates, then
    /// restores the original interpreter state and returns the output.
    fn call_transient_cell(
        &mut self,
        cell_def: &CellDef,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let uid = self.next_cell_uid;
        self.next_cell_uid += 1;

        // Save interpreter state for restoration after cell execution.
        let saved_state = self.state.clone();
        let saved_prior = self.prior_state.clone();
        let saved_return = self.return_value.take();

        // Initialize cell fields in the interpreter state.
        self.init_cell_state(cell_def, uid, args)?;

        // Run convergence loop with fuel limit for safety.
        let max_iterations: usize = 100000;
        for _ in 0..max_iterations {
            // Evaluate all transactions. If any terminated, return immediately.
            let iteration_outcome = self.run_cell_convergence_iteration(
                cell_def, uid, &saved_state, &saved_prior, &saved_return,
            )?;

            match iteration_outcome {
                // Termination: return value produced by term/term!.
                ConvergenceOutcome::Terminated(ret_val) => {
                    return Ok(ret_val);
                }
                // No transaction fired: convergence reached, exit loop.
                ConvergenceOutcome::Converged => break,
                // At least one transaction fired: continue iterating.
                ConvergenceOutcome::Fired => {}
            }
        }

        // Extract the designated output and restore state.
        let result = self.get_designated_output(cell_def, uid);
        self.state = saved_state;
        self.prior_state = saved_prior;
        self.return_value = saved_return;
        Ok(result)
    }

    /// Initialize a transient cell's state fields in the interpreter's state map.
    ///
    /// Creates scoped identifiers like `cell$0.field_name` for each field,
    /// then binds input arguments with the same UID prefix.
    fn init_cell_state(
        &mut self,
        cell_def: &CellDef,
        uid: usize,
        args: &[Value],
    ) -> Result<(), RuntimeError> {
        // Initialize fields from defaults or zero values.
        for field in &cell_def.fields {
            let key = format!("{}${}.{}", cell_def.name, uid, field.name);
            let value = match &field.default {
                Some(expr) => self.eval_expr(expr)?,
                None => init_field_default(&field.ty),
            };
            self.state.insert(key, value);
        }

        // Bind input arguments.
        for ((param_name, _), arg) in cell_def.parameters.iter().zip(args.iter()) {
            let key = format!("{}${}.{}", cell_def.name, uid, param_name);
            self.state.insert(key, arg.clone());
        }

        // Evaluate and bind internal triggers.
        for trg in &cell_def.internal_triggers {
            let trg_key = format!("{}${}.{}", cell_def.name, uid, trg.name);
            let trg_val = self.evaluate_internal_trigger(trg)?;
            self.state.insert(trg_key, trg_val);
        }

        Ok(())
    }

    /// Tick all persistent cells. Runs up to 2 convergence passes to handle
    /// cell-to-cell wire propagation: pass 0 ticks all cells, pass 1 re-ticks
    /// cells that received wire updates.
    ///
    /// Returns true if any cell fired (output may have changed).
    /// 2026-07-11: Two-pass design. A single pass would miss updated wire
    /// values that arrive after a cell has already converged. The second pass
    /// re-evaluates only cells whose inputs changed via wires from pass 0.
    pub(crate) fn tick_persistent_cells(&mut self) -> Result<bool, RuntimeError> {
        let mut any_fired = false;

        for pass in 0..2 {
            // Before second pass: reset fired flag for wire target cells.
            if pass == 1 {
                self.reset_wire_target_cells();
            }

            // Collect cell names to avoid borrow conflicts.
            let cell_names: Vec<String> = self.persistent_cells.keys().cloned().collect();

            for name in &cell_names {
                let cell_fired = self.tick_single_persistent_cell(name)?;
                if cell_fired {
                    any_fired = true;
                }
            }
        }

        Ok(any_fired)
    }

    /// Reset the `fired` flag for cells that are targets of cell-to-cell wires.
    /// This ensures they re-evaluate in pass 1 with updated parameter values
    /// that were propagated during pass 0.
    fn reset_wire_target_cells(&mut self) {
        for wire in &self.cell_wires.clone() {
            let Some(instance) = self.persistent_cells.get_mut(&wire.to_cell) else {
                continue;
            };
            let key = format!("{}${}.fired", wire.to_cell, 0);
            instance.state.insert(key, Value::Bits(vec![0u8]));
        }
    }

    /// Tick a single persistent cell by name.
    ///
    /// Handles three cases:
    /// 1. Threaded cell (tick_interval > 0): check channel for outputs.
    /// 2. Non-threaded cell with tick interval: skip if not due for tick.
    /// 3. Non-threaded cell: run convergence inline.
    fn tick_single_persistent_cell(&mut self, name: &str) -> Result<bool, RuntimeError> {
        // Guard: cell must exist (may have been removed by a previous tick).
        if !self.persistent_cells.contains_key(name) {
            return Ok(false);
        }

        // Take ownership of the instance to avoid borrow conflicts.
        let mut instance = self.persistent_cells.remove(name).unwrap();
        instance.tick_counter += 1;

        // ── Case 1: Threaded cell ──────────────────────────────────────
        if instance.tick_interval > 0 {
            let fired = self.tick_threaded_cell(name, &mut instance);

            // Save instance back regardless of outcome.
            self.persistent_cells.insert(name.to_string(), instance);
            return Ok(fired);
        }

        // ── Case 2: Skip if not due (non-threaded interval check) ──────
        // This code path handles cells with tick_interval > 0 that are
        // processed on the main thread (interval-based but not threaded).
        // The interval was already checked above for threaded cells.
        // For non-threaded interval cells, check modulo.
        if instance.tick_interval > 0
            && instance.tick_counter % instance.tick_interval != 0
        {
            self.persistent_cells.insert(name.to_string(), instance);
            return Ok(false);
        }

        // ── Case 3: Run convergence inline ─────────────────────────────
        let fired = self.run_inline_cell_convergence(name, &mut instance)?;

        // Save instance back.
        self.persistent_cells.insert(name.to_string(), instance);
        Ok(fired)
    }

    /// Check a threaded cell's channel for new outputs.
    ///
    /// Threaded cells run their own convergence loop in a background thread
    /// and publish outputs to the channel. The reactor loop just reads the
    /// channel and propagates the outputs to trigger bindings.
    fn tick_threaded_cell(&mut self, name: &str, instance: &PersistentCellInstance) -> bool {
        // Guard: no change since last check.
        if !instance.channel.changed.load(Ordering::SeqCst) {
            return false;
        }

        // Read outputs from the channel.
        let outputs = instance.channel.outputs.lock().unwrap().clone();

        // Reset the changed flag.
        instance.channel.changed.store(false, Ordering::SeqCst);

        // Propagate outputs to trigger bindings.
        for (port_name, val) in &outputs {
            for trg in &self.trg_bindings {
                if trg.cell_name == name && trg.port_name == *port_name {
                    self.state.insert(trg.trigger_name.clone(), val.clone());
                }
            }
        }

        true
    }

    /// Run the transaction loop for an inline persistent cell convergence pass.
    ///
    /// Iterates over all transactions, executing bodies and checking postconditions.
    /// Returns `InlineConvergence` with termination/fire flags. Extracted from
    /// `run_inline_cell_convergence` to eliminate the for→match arrow pattern.
    /// 2026-07-12: The loop body calls `execute_txn_body` which is already flat.
    /// This function returns a struct rather than using out-params.
    fn run_inline_txn_loop(
        &mut self,
        cell_def: &CellDef,
        cell_name: &str,
    ) -> Result<InlineConvergence, RuntimeError> {
        let mut fired = false;
        let mut terminated = false;

        for txn in &cell_def.transactions {
            let result = execute_txn_body(self, txn, 0, cell_name);

            // Guard: propagate error immediately.
            let outcome = match result {
                Err(e) => return Err(e),
                Ok(outcome) => outcome,
            };

            // Guard: check for termination via term/term!.
            if outcome.is_some() {
                terminated = true;
                break;
            }

            // Check postcondition to determine if cell fired.
            let post = self.rewrite_identifiers(&txn.contract.post_condition, 0, cell_name);
            let post_val = self.eval_expr(&post).unwrap_or(Value::Bits(vec![0u8]));
            if post_val == Value::Bits(vec![1u8]) && self.state != self.prior_state {
                fired = true;
            }
        }

        Ok(InlineConvergence { terminated, fired })
    }

    /// Run inline convergence for a non-threaded persistent cell.
    ///
    /// Saves the interpreter's state, installs the cell's state, runs one
    /// convergence pass over the cell's transactions, syncs outputs to
    /// trigger bindings, propagates cell-to-cell wires, and restores the
    /// interpreter's state.
    fn run_inline_cell_convergence(
        &mut self,
        name: &str,
        instance: &mut PersistentCellInstance,
    ) -> Result<bool, RuntimeError> {
        // Save the current parent interpreter state.
        let saved_state = std::mem::replace(&mut self.state, instance.state.clone());
        let saved_prior = std::mem::replace(&mut self.prior_state, instance.prior_state.clone());
        let cell_name = name.to_string();

        // Evaluate internal triggers before running convergence.
        self.eval_internal_triggers(&instance.cell_def, &cell_name)?;

        // Run one convergence pass over the cell's transactions.
        let convergence = self.run_inline_txn_loop(&instance.cell_def, &cell_name)?;

        // Save cell's state back to the instance.
        instance.state = self.state.clone();
        instance.prior_state = self.prior_state.clone();

        // Restore parent interpreter state.
        self.state = saved_state;
        self.prior_state = saved_prior;

        // Handle termination or output sync.
        if convergence.terminated {
            self.remove_cell_bindings(&cell_name);
        } else if convergence.fired {
            self.sync_cell_outputs(&cell_name, instance);
            self.propagate_cell_wires(&cell_name, instance);
        }

        Ok(convergence.terminated || convergence.fired)
    }

    /// Evaluate all internal triggers for a cell and store results in the
    /// interpreter's state map (which is currently the cell's state).
    fn eval_internal_triggers(
        &mut self,
        cell_def: &CellDef,
        cell_name: &str,
    ) -> Result<(), RuntimeError> {
        for trg in &cell_def.internal_triggers {
            let trg_key = format!("{}${}.{}", cell_name, 0, trg.name);
            let trg_val = self.evaluate_internal_trigger(trg)?;
            self.state.insert(trg_key, trg_val);
        }
        Ok(())
    }

    /// Remove all trigger bindings and state entries for a terminated cell.
    fn remove_cell_bindings(&mut self, cell_name: &str) {
        let to_remove: Vec<String> = self
            .trg_bindings
            .iter()
            .filter(|t| t.cell_name == cell_name)
            .map(|t| t.trigger_name.clone())
            .collect();

        for trg_name in to_remove {
            self.state.remove(&trg_name);
        }

        self.trg_bindings.retain(|t| t.cell_name != cell_name);
    }

    /// Sync a cell's output values to trigger bindings in the parent state.
    fn sync_cell_outputs(
        &mut self,
        cell_name: &str,
        instance: &mut PersistentCellInstance,
    ) {
        let names = match &instance.cell_def.output_type {
            Some(ot) => self.extract_output_names(ot),
            None => return,
        };

        for port_name in names {
            let key = format!("{}${}.{}", cell_name, 0, port_name);
            let new_val = instance.state.get(&key).cloned().unwrap_or(Value::Void);
            let old_val = instance.output_cache.get(&port_name);

            // Guard: skip if value hasn't changed.
            if Some(&new_val) == old_val {
                continue;
            }

            // Propagate changed output to trigger bindings.
            register_output_trigger(self, cell_name, &port_name, new_val.clone());
            instance.output_cache.insert(port_name, new_val);
        }
    }

    /// Propagate cell-to-cell wires: copy output values from this cell
    /// to target cell parameter state slots.
    fn propagate_cell_wires(&mut self, cell_name: &str, instance: &PersistentCellInstance) {
        for wire in &self.cell_wires.clone() {
            // Guard: only process wires from this cell.
            if wire.from_cell != cell_name {
                continue;
            }

            // Read the source value from this cell's state.
            let src_key = format!("{}${}.{}", cell_name, 0, wire.from_port);
            let Some(val) = instance.state.get(&src_key).cloned() else {
                continue;
            };

            // Write to the target cell's parameter slot.
            let Some(target) = self.persistent_cells.get_mut(&wire.to_cell) else {
                continue;
            };
            let dst_key = format!("{}${}.{}", wire.to_cell, 0, wire.to_param);
            target.state.insert(dst_key, val);
        }
    }

    /// Rewrite all identifiers in an expression to use cell-scoped names.
    ///
    /// Maps `x` → `cellName$uid.x` for every identifier in the expression tree.
    /// This ensures each cell invocation gets its own isolated namespace.
    /// 2026-07-11: The match arms are all one-liners (Expr → Expr constructor).
    /// Each arm is flat: match → construct. No nesting deeper than 2.
    fn rewrite_identifiers(&self, expr: &Expr, uid: usize, cell_name: &str) -> Expr {
        let prefix = |name: &str| -> String {
            format!("{}${}.{}", cell_name, uid, name)
        };

        match expr {
            // Leaf nodes — no identifiers to rewrite.
            Expr::Decimal(_)
            | Expr::IntegerSuffixed(_, _)
            | Expr::Float(_)
            | Expr::Float64(_)
            | Expr::Quoted(_)
            | Expr::RegexLiteral(_)
            | Expr::Char(_)
            | Expr::Bool(_)
            | Expr::Term
            | Expr::Ellipsis
            | Expr::SharedMem(_) => expr.clone(),

            // Deferred literal carries no identifiers (Phase 5).
            Expr::DeferredLiteral { text, expected_type } => Expr::DeferredLiteral {
                text: text.clone(),
                expected_type: expected_type.clone(),
            },

            // Simple names — rewrite directly.
            Expr::Literal(lit) => Expr::Literal(lit.clone()),
            Expr::Identifier(name) => Expr::Identifier(prefix(name)),
            Expr::AddrOf(inner) => Expr::AddrOf(Box::new(Expr::Identifier(
                prefix(inner.as_var_name().unwrap()),
            ))),
            Expr::PriorState(name) => Expr::PriorState(prefix(name)),
            Expr::EllipsisExpr(e) => Expr::EllipsisExpr(e.clone()),
            Expr::TypeRef(name) => Expr::TypeRef(name.clone()),
            Expr::Interpolate(name) => Expr::Interpolate(prefix(name)),

            // Unary operators. Delegates to a flat helper.
            Expr::Not(_) | Expr::Neg(_) | Expr::BitNot(_) => {
                Self::rewrite_unary_op(self, expr, uid, cell_name)
            }

            // Binary operators (by kind). Delegates to a flat helper.
            Expr::Add(_, _)
            | Expr::Sub(_, _)
            | Expr::Mul(_, _)
            | Expr::Div(_, _)
            | Expr::Mod(_, _)
            | Expr::Eq(_, _)
            | Expr::Ne(_, _)
            | Expr::Lt(_, _)
            | Expr::Le(_, _)
            | Expr::Gt(_, _)
            | Expr::Ge(_, _)
            | Expr::BitAnd(_, _)
            | Expr::BitOr(_, _)
            | Expr::BitXor(_, _)
            | Expr::Shl(_, _)
            | Expr::Shr(_, _)
            | Expr::Or(_, _)
            | Expr::And(_, _)
            | Expr::Concat(_, _) => Self::rewrite_old_binary_op(self, expr, uid, cell_name),

            // Wrapped variants (old-style → delegate to new-style).
            Expr::BinaryOp(e) => Expr::BinaryOp(Box::new(BinaryOpExpr {
                kind: e.kind,
                left: Box::new(self.rewrite_identifiers(&e.left, uid, cell_name)),
                right: Box::new(self.rewrite_identifiers(&e.right, uid, cell_name)),
            })),
            Expr::UnaryOp(e) => Expr::UnaryOp(Box::new(UnaryOpExpr {
                kind: e.kind,
                operand: Box::new(self.rewrite_identifiers(&e.operand, uid, cell_name)),
            })),

            // Type-checking expressions (IsType, FromCheck, Like, Cast).
            Expr::IsType(e, target) => Expr::IsType(
                Box::new(self.rewrite_identifiers(e, uid, cell_name)),
                target.clone(),
            ),
            Expr::FromCheck(e, ty) => Expr::FromCheck(
                Box::new(self.rewrite_identifiers(e, uid, cell_name)),
                ty.clone(),
            ),
            Expr::Like(l, r) => Expr::Like(
                Box::new(self.rewrite_identifiers(l, uid, cell_name)),
                Box::new(self.rewrite_identifiers(r, uid, cell_name)),
            ),
            Expr::Cast(e, ty) => Expr::Cast(
                Box::new(self.rewrite_identifiers(e, uid, cell_name)),
                ty.clone(),
            ),

            // Arrow operations.
            Expr::ArrowMut {
                dir,
                target,
                index,
                value,
                consume,
            } => Expr::ArrowMut {
                consume: *consume,
                dir: dir.clone(),
                target: Box::new(self.rewrite_identifiers(target, uid, cell_name)),
                index: Box::new(self.rewrite_identifiers(index, uid, cell_name)),
                value: value
                    .as_ref()
                    .map(|v| Box::new(self.rewrite_identifiers(v, uid, cell_name))),
            },
            Expr::ArrowDiscard { target, index } => Expr::ArrowDiscard {
                target: Box::new(self.rewrite_identifiers(target, uid, cell_name)),
                index: Box::new(self.rewrite_identifiers(index, uid, cell_name)),
            },
            Expr::ArrowTransfer {
                dest,
                source,
                filter,
                consume,
            } => Expr::ArrowTransfer {
                consume: *consume,
                dest: Box::new(self.rewrite_identifiers(dest, uid, cell_name)),
                source: Box::new(self.rewrite_identifiers(source, uid, cell_name)),
                filter: filter
                    .as_ref()
                    .map(|f| Box::new(self.rewrite_identifiers(f, uid, cell_name))),
            },
            Expr::ArrowMutExpr(e) => Expr::ArrowMutExpr(e.clone()),
            Expr::ArrowDiscardExpr(e) => Expr::ArrowDiscardExpr(e.clone()),
            Expr::ArrowTransferExpr(e) => Expr::ArrowTransferExpr(e.clone()),

            // Projections.
            Expr::Projection { source, target } => Expr::Projection {
                source: Box::new(self.rewrite_identifiers(source, uid, cell_name)),
                target: target.clone(),
            },
            Expr::ProjectionExpr(e) => Expr::ProjectionExpr(ProjectionExpr {
                source: Box::new(self.rewrite_identifiers(&e.source, uid, cell_name)),
                target: e.target.clone(),
            }),

            // Calls.
            Expr::Call(name, args) => Expr::Call(
                name.clone(),
                args.iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            ),
            Expr::CallExpr(e) => Expr::CallExpr(CallExpr {
                name: e.name.clone(),
                args: e
                    .args
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            }),
            Expr::CellCall(callee, args) => Expr::CellCall(
                Box::new(self.rewrite_identifiers(callee, uid, cell_name)),
                args.iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            ),
            Expr::TemplateCall {
                name,
                args,
                block,
                span,
            } => Expr::TemplateCall {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
                block: block.clone(),
                span: *span,
            },
            Expr::MacroCall {
                name,
                args,
                block,
                span,
            } => Expr::MacroCall {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
                block: block.clone(),
                span: *span,
            },
            Expr::IntrinsicCall { intrinsic, args } => Expr::IntrinsicCall {
                intrinsic: intrinsic.clone(),
                args: args
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            },

            // Lists.
            Expr::ListLiteral(items) => Expr::ListLiteral(
                items
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            ),
            Expr::ListLiteralExpr(e) => Expr::ListLiteralExpr(ListLiteralExpr {
                elements: e
                    .elements
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            }),
            Expr::SetLiteral(items) => Expr::SetLiteral(
                items
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            ),
            Expr::SetLiteralExpr(e) => Expr::SetLiteralExpr(SetLiteralExpr {
                entries: e
                    .entries
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            }),

            // Maps.
            Expr::MapLiteral(pairs) => Expr::MapLiteral(
                pairs
                    .iter()
                    .map(|(k, v)| {
                        (
                            self.rewrite_identifiers(k, uid, cell_name),
                            self.rewrite_identifiers(v, uid, cell_name),
                        )
                    })
                    .collect(),
            ),
            Expr::MapLiteralExpr(e) => Expr::MapLiteralExpr(MapLiteralExpr {
                entries: e
                    .entries
                    .iter()
                    .map(|(k, v)| {
                        (
                            self.rewrite_identifiers(k, uid, cell_name),
                            self.rewrite_identifiers(v, uid, cell_name),
                        )
                    })
                    .collect(),
            }),

            // Indexing.
            Expr::ListIndex(list, idx) => Expr::ListIndex(
                Box::new(self.rewrite_identifiers(list, uid, cell_name)),
                Box::new(self.rewrite_identifiers(idx, uid, cell_name)),
            ),
            Expr::Slice {
                value,
                start,
                end,
                stride,
                mask,
            } => Expr::Slice {
                value: Box::new(self.rewrite_identifiers(value, uid, cell_name)),
                start: start
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_identifiers(s, uid, cell_name))),
                end: end
                    .as_ref()
                    .map(|e| Box::new(self.rewrite_identifiers(e, uid, cell_name))),
                stride: stride
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_identifiers(s, uid, cell_name))),
                mask: mask
                    .as_ref()
                    .map(|m| Box::new(self.rewrite_identifiers(m, uid, cell_name))),
            },
            Expr::SliceExpr(e) => Expr::SliceExpr(SliceExpr {
                value: Box::new(self.rewrite_identifiers(&e.value, uid, cell_name)),
                start: e
                    .start
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_identifiers(s, uid, cell_name))),
                end: e
                    .end
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_identifiers(s, uid, cell_name))),
                stride: e
                    .stride
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_identifiers(s, uid, cell_name))),
                mask: e
                    .mask
                    .as_ref()
                    .map(|m| Box::new(self.rewrite_identifiers(m, uid, cell_name))),
            }),
            Expr::MultiSlice { value, ops } => Expr::MultiSlice {
                value: Box::new(self.rewrite_identifiers(value, uid, cell_name)),
                ops: ops.clone(),
            },
            Expr::MultiSliceExpr(e) => Expr::MultiSliceExpr(MultiSliceExpr {
                value: Box::new(self.rewrite_identifiers(&e.value, uid, cell_name)),
                ops: e.ops.clone(),
            }),

            // Field access.
            Expr::FieldAccess(obj, field) => Expr::FieldAccess(
                Box::new(self.rewrite_identifiers(obj, uid, cell_name)),
                field.clone(),
            ),
            Expr::FieldAccessExpr(e) => Expr::FieldAccessExpr(FieldAccessExpr {
                obj: Box::new(self.rewrite_identifiers(&e.obj, uid, cell_name)),
                field: e.field.clone(),
            }),

            // Struct / Object instances.
            Expr::StructInstance(name, fields) => Expr::StructInstance(
                name.clone(),
                fields
                    .iter()
                    .map(|(n, e)| (n.clone(), self.rewrite_identifiers(e, uid, cell_name)))
                    .collect(),
            ),
            Expr::StructInstanceExpr(e) => Expr::StructInstanceExpr(StructInstanceExpr {
                typename: e.typename.clone(),
                fields: e
                    .fields
                    .iter()
                    .map(|(n, e)| (n.clone(), self.rewrite_identifiers(e, uid, cell_name)))
                    .collect(),
            }),
            Expr::ObjectLiteral(fields) => Expr::ObjectLiteral(
                fields
                    .iter()
                    .map(|(n, e)| (n.clone(), self.rewrite_identifiers(e, uid, cell_name)))
                    .collect(),
            ),
            Expr::ObjectLiteralExpr(e) => Expr::ObjectLiteralExpr(ObjectLiteralExpr {
                fields: e
                    .fields
                    .iter()
                    .map(|(n, e)| (n.clone(), self.rewrite_identifiers(e, uid, cell_name)))
                    .collect(),
            }),

            // Pattern matching.
            Expr::PatternMatch {
                value,
                variant,
                fields,
            } => Expr::PatternMatch {
                value: Box::new(self.rewrite_identifiers(value, uid, cell_name)),
                variant: variant.clone(),
                fields: fields.clone(),
            },
            Expr::PatternMatchExpr(e) => Expr::PatternMatchExpr(PatternMatchExpr {
                value: Box::new(self.rewrite_identifiers(&e.value, uid, cell_name)),
                variant: e.variant.clone(),
                fields: e.fields.clone(),
            }),
            Expr::Match { value, arms } => Expr::Match {
                value: Box::new(self.rewrite_identifiers(value, uid, cell_name)),
                arms: arms.clone(),
            },
            Expr::MatchExpr(e) => Expr::MatchExpr(MatchExpr {
                value: Box::new(self.rewrite_identifiers(&e.value, uid, cell_name)),
                arms: e.arms.clone(),
            }),

            // Blocks.
            Expr::Block(stmts, last) => Expr::Block(
                stmts
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                Box::new(self.rewrite_identifiers(last, uid, cell_name)),
            ),
            Expr::BlockExpr(e) => Expr::BlockExpr(BlockExpr {
                stmts: e
                    .stmts
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                last: Box::new(self.rewrite_identifiers(&e.last, uid, cell_name)),
            }),
            Expr::InterpolateExpr(e) => Expr::InterpolateExpr(Box::new(
                self.rewrite_identifiers(e, uid, cell_name),
            )),
            Expr::QuoteBlock {
                statements,
                trailing_expr,
            } => Expr::QuoteBlock {
                statements: statements
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                trailing_expr: trailing_expr
                    .as_ref()
                    .map(|e| Box::new(self.rewrite_identifiers(e, uid, cell_name))),
            },

            // Tuples.
            Expr::TupleDestructure(names, expr) => Expr::TupleDestructure(
                names.clone(),
                Box::new(self.rewrite_identifiers(expr, uid, cell_name)),
            ),
            Expr::TupleDestructureExpr(e) => Expr::TupleDestructureExpr(TupleDestructureExpr {
                names: e.names.clone(),
                expr: Box::new(self.rewrite_identifiers(&e.expr, uid, cell_name)),
            }),
            Expr::Tuple(items) => Expr::Tuple(
                items
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            ),
            Expr::TupleExpr(e) => Expr::TupleExpr(TupleExpr {
                exprs: e
                    .exprs
                    .iter()
                    .map(|a| self.rewrite_identifiers(a, uid, cell_name))
                    .collect(),
            }),

            // Miscellaneous.
            Expr::SigCall { modifier, expr } => Expr::SigCall {
                modifier: modifier.clone(),
                expr: Box::new(self.rewrite_identifiers(expr, uid, cell_name)),
            },
            Expr::SigCallExpr(e) => Expr::SigCallExpr(SigCallExpr {
                modifier: e.modifier.clone(),
                expr: Box::new(self.rewrite_identifiers(&e.expr, uid, cell_name)),
            }),
            Expr::SubtypeProjection { source, ops } => Expr::SubtypeProjection {
                source: Box::new(self.rewrite_identifiers(source, uid, cell_name)),
                ops: ops.clone(),
            },
            Expr::SubtypeProjectionExpr(e) => Expr::SubtypeProjectionExpr(SubtypeProjectionExpr {
                source: Box::new(self.rewrite_identifiers(&e.source, uid, cell_name)),
                ops: e.ops.clone(),
            }),
            Expr::DbvlTable {
                path,
                field_names,
                key_offsets,
                schema_name,
            } => Expr::DbvlTable {
                path: path.clone(),
                field_names: field_names.clone(),
                key_offsets: key_offsets.clone(),
                schema_name: schema_name.clone(),
            },
            Expr::DbvlTableExpr(e) => Expr::DbvlTableExpr(e.clone()),
            Expr::PipeChain(chain) => Expr::PipeChain(crate::ast::PipeChain {
                initial: Box::new(self.rewrite_identifiers(&chain.initial, uid, cell_name)),
                steps: chain
                    .steps
                    .iter()
                    .map(|s| crate::ast::PipeStep {
                        target: Box::new(self.rewrite_identifiers(&s.target, uid, cell_name)),
                        skip: s.skip,
                    })
                    .collect(),
            }),
            Expr::Within {
                body, fallback, ..
            } => Expr::Within {
                body: Box::new(self.rewrite_identifiers(body, uid, cell_name)),
                bound: 0,
                retries: 0,
                unit: crate::ast::TimeUnit::Cycles,
                fallback: Box::new(self.rewrite_identifiers(fallback, uid, cell_name)),
            },
            Expr::Deref(inner) => Expr::Deref(Box::new(
                self.rewrite_identifiers(inner, uid, cell_name),
            )),
        }
    }

    /// Rewrite a unary operator expression's operand.
    ///
    /// Extracted from `rewrite_identifiers` to eliminate the inner-match
    /// arrow pattern. This helper handles all three unary variants (Not,
    /// Neg, BitNot) in one flat function.
    fn rewrite_unary_op(
        &self,
        expr: &Expr,
        uid: usize,
        cell_name: &str,
    ) -> Expr {
        // Extract the operand from whichever variant matched.
        let operand = match expr {
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => e,
            _ => unreachable!(),
        };
        let inner = self.rewrite_identifiers(operand, uid, cell_name);

        // Reconstruct the same variant with the rewritten operand.
        match expr {
            Expr::Not(_) => Expr::Not(Box::new(inner)),
            Expr::Neg(_) => Expr::Neg(Box::new(inner)),
            _ => Expr::BitNot(Box::new(inner)),
        }
    }

    /// Rewrite an old-style binary operator expression (Expr::Add, etc.).
    ///
    /// Extracted from `rewrite_identifiers` to eliminate the triple-match
    /// arrow pattern. This helper extracts operands, rewrites them, and
    /// reconstructs the correct Expr variant — all in one flat function.
    fn rewrite_old_binary_op(
        &self,
        expr: &Expr,
        uid: usize,
        cell_name: &str,
    ) -> Expr {
        // Extract left and right operands from whichever variant matched.
        let (lhs, rhs) = match expr {
            Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Mod(l, r)
            | Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::BitAnd(l, r)
            | Expr::BitOr(l, r)
            | Expr::BitXor(l, r)
            | Expr::Shl(l, r)
            | Expr::Shr(l, r)
            | Expr::Or(l, r)
            | Expr::And(l, r)
            | Expr::Concat(l, r) => (l, r),
            _ => unreachable!(),
        };

        let rewritten_lhs = self.rewrite_identifiers(lhs, uid, cell_name);
        let rewritten_rhs = self.rewrite_identifiers(rhs, uid, cell_name);
        let lhs_box = Box::new(rewritten_lhs);
        let rhs_box = Box::new(rewritten_rhs);

        // Reconstruct the same variant with rewritten operands.
        match expr {
            Expr::Add(_, _) => Expr::Add(lhs_box, rhs_box),
            Expr::Sub(_, _) => Expr::Sub(lhs_box, rhs_box),
            Expr::Mul(_, _) => Expr::Mul(lhs_box, rhs_box),
            Expr::Div(_, _) => Expr::Div(lhs_box, rhs_box),
            Expr::Mod(_, _) => Expr::Mod(lhs_box, rhs_box),
            Expr::Eq(_, _) => Expr::Eq(lhs_box, rhs_box),
            Expr::Ne(_, _) => Expr::Ne(lhs_box, rhs_box),
            Expr::Lt(_, _) => Expr::Lt(lhs_box, rhs_box),
            Expr::Le(_, _) => Expr::Le(lhs_box, rhs_box),
            Expr::Gt(_, _) => Expr::Gt(lhs_box, rhs_box),
            Expr::Ge(_, _) => Expr::Ge(lhs_box, rhs_box),
            Expr::BitAnd(_, _) => Expr::BitAnd(lhs_box, rhs_box),
            Expr::BitOr(_, _) => Expr::BitOr(lhs_box, rhs_box),
            Expr::BitXor(_, _) => Expr::BitXor(lhs_box, rhs_box),
            Expr::Shl(_, _) => Expr::Shl(lhs_box, rhs_box),
            Expr::Shr(_, _) => Expr::Shr(lhs_box, rhs_box),
            Expr::Or(_, _) => Expr::Or(lhs_box, rhs_box),
            Expr::And(_, _) => Expr::And(lhs_box, rhs_box),
            _ => Expr::Concat(lhs_box, rhs_box),
        }
    }

    /// Rewrite all identifiers in a statement (delegates to `rewrite_identifiers`).
    fn rewrite_statement_identifiers(
        &self,
        stmt: &Statement,
        uid: usize,
        cell_name: &str,
    ) -> Statement {
        match stmt {
            Statement::Assignment {
                lhs,
                expr,
                timeout,
                modifiers,
            } => Statement::Assignment {
                lhs: self.rewrite_identifiers(lhs, uid, cell_name),
                expr: self.rewrite_identifiers(expr, uid, cell_name),
                timeout: timeout.clone(),
                modifiers: modifiers.clone(),
            },
            Statement::Unification {
                name,
                variant,
                fields,
                expr,
            } => Statement::Unification {
                name: name.clone(),
                variant: variant.clone(),
                fields: fields.clone(),
                expr: self.rewrite_identifiers(expr, uid, cell_name),
            },
            Statement::Guarded {
                condition,
                statements,
                ..
            } => Statement::Guarded {
                condition: self.rewrite_identifiers(condition, uid, cell_name),
                statements: statements
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                metadata: HashMap::new(),
            },
            Statement::Term {
                values,
                swan_song,
                modifiers,
            } => Statement::Term {
                values: values
                    .iter()
                    .map(|v| {
                        v.as_ref()
                            .map(|e| self.rewrite_identifiers(e, uid, cell_name))
                    })
                    .collect(),
                swan_song: swan_song
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_statement_identifiers(s, uid, cell_name))),
                modifiers: modifiers.clone(),
            },
            Statement::TermBang {
                values,
                swan_song,
                modifiers,
            } => Statement::TermBang {
                values: values
                    .iter()
                    .map(|v| {
                        v.as_ref()
                            .map(|e| self.rewrite_identifiers(e, uid, cell_name))
                    })
                    .collect(),
                swan_song: swan_song
                    .as_ref()
                    .map(|s| Box::new(self.rewrite_statement_identifiers(s, uid, cell_name))),
                modifiers: modifiers.clone(),
            },
            Statement::Escape(expr) => {
                Statement::Escape(expr.as_ref().map(|e| self.rewrite_identifiers(e, uid, cell_name)))
            }
            Statement::Expression(expr) => {
                Statement::Expression(self.rewrite_identifiers(expr, uid, cell_name))
            }
            Statement::Let {
                name,
                ty,
                expr,
                address,
                address_expr,
                bit_range,
                constraint,
                is_override,
                modifiers,
            } => Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr
                    .as_ref()
                    .map(|e| self.rewrite_identifiers(e, uid, cell_name)),
                address: *address,
                address_expr: address_expr
                    .as_ref()
                    .map(|a| Box::new(self.rewrite_identifiers(a, uid, cell_name))),
                bit_range: bit_range.clone(),
                constraint: constraint
                    .as_ref()
                    .map(|c| Box::new(self.rewrite_identifiers(c, uid, cell_name))),
                is_override: *is_override,
                modifiers: modifiers.clone(),
            },
            Statement::InlineAsm {
                asm_string,
                clobbers,
                span,
            } => Statement::InlineAsm {
                asm_string: asm_string.clone(),
                clobbers: clobbers.clone(),
                span: *span,
            },
            Statement::TrgBinding {
                name,
                ty,
                instance,
                port,
                modifiers,
            } => Statement::TrgBinding {
                name: name.clone(),
                ty: ty.clone(),
                instance: self.rewrite_identifiers(instance, uid, cell_name),
                port: port.clone(),
                modifiers: modifiers.clone(),
            },
            Statement::SyncBlock { body } => Statement::SyncBlock {
                body: body
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
            },
            Statement::Foreach {
                item,
                list,
                body,
                modifiers,
            } => Statement::Foreach {
                item: item.clone(),
                list: Box::new(self.rewrite_identifiers(list, uid, cell_name)),
                body: body
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                modifiers: modifiers.clone(),
            },
            Statement::Oracle {
                handler,
                body,
                span,
            } => Statement::Oracle {
                handler: handler
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                body: body
                    .iter()
                    .map(|s| self.rewrite_statement_identifiers(s, uid, cell_name))
                    .collect(),
                span: *span,
            },
            Statement::Await { expr, modifiers } => Statement::Await {
                expr: self.rewrite_identifiers(expr, uid, cell_name),
                modifiers: modifiers.clone(),
            },
            Statement::Async { body, modifiers } => Statement::Async {
                body: Box::new(self.rewrite_statement_identifiers(body, uid, cell_name)),
                modifiers: modifiers.clone(),
            },
            Statement::AsyncAwait {
                body,
                lhs,
                modifiers,
            } => Statement::AsyncAwait {
                body: Box::new(self.rewrite_statement_identifiers(body, uid, cell_name)),
                lhs: lhs.clone(),
                modifiers: modifiers.clone(),
            },
        }
    }

    /// Get the designated output value of a cell.
    ///
    /// If the cell has a single output port, returns its value.
    /// If multiple output ports, returns a Tuple of values.
    /// If no output type declared, returns Void.
    fn get_designated_output(&self, cell_def: &CellDef, uid: usize) -> Value {
        let Some(ref ot) = cell_def.output_type else {
            return Value::Void;
        };

        let names = self.extract_output_names(ot);

        // Single output: return the value directly.
        if names.len() == 1 {
            let key = format!("{}${}.{}", cell_def.name, uid, &names[0]);
            return self.state.get(&key).cloned().unwrap_or(Value::Void);
        }

        // Multiple outputs: return as Tuple.
        if names.len() > 1 {
            let values: Vec<Value> = names
                .iter()
                .map(|n| {
                    let key = format!("{}${}.{}", cell_def.name, uid, n);
                    self.state.get(&key).cloned().unwrap_or(Value::Void)
                })
                .collect();
            return Value::Tuple(values);
        }

        Value::Void
    }

    /// Extract output port names from an OutputType tree.
    ///
    /// Recursively flattens Named, Tuple, and Union output types into
    /// a list of port names. Single and Array types have no named ports.
    pub(crate) fn extract_output_names(&self, ot: &OutputType) -> Vec<String> {
        match ot {
            OutputType::Named(name, inner) => {
                let mut names = vec![name.clone()];
                names.extend(self.extract_output_names(inner));
                names
            }
            OutputType::Tuple(types) => {
                types.iter().flat_map(|t| self.extract_output_names(t)).collect()
            }
            OutputType::Union(types) => {
                types.iter().flat_map(|t| self.extract_output_names(t)).collect()
            }
            OutputType::Single(_) | OutputType::Array(_) => Vec::new(),
        }
    }

    /// Execute a block with a runtime fuel limit.
    ///
    /// Sets the interpreter's oracle_fuel to `fuel`, runs each statement
    /// until fuel is exhausted or an error occurs, then restores the
    /// previous fuel value.
    ///
    /// Returns `Err(RuntimeError::FuelExhausted)` if fuel ran out before
    /// all statements completed.
    pub(crate) fn exec_stmts_with_fuel(
        &mut self,
        stmts: &[Statement],
        fuel: u64,
    ) -> Result<(), RuntimeError> {
        let saved_fuel = self.oracle_fuel;
        self.oracle_fuel = Some(fuel);
        let mut result = Ok(());

        for stmt in stmts {
            // Guard: stop if fuel is exhausted.
            if self.oracle_fuel == Some(0) {
                break;
            }

            let outcome = self.exec_stmt(stmt);
            match outcome {
                Ok(()) => {}
                Err(RuntimeError::FuelExhausted) => break,
                Err(e @ RuntimeError::Timeout(_)) => {
                    result = Err(e);
                    break;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }

        let exhausted = self.oracle_fuel == Some(0);
        self.oracle_fuel = saved_fuel;

        if exhausted {
            Err(RuntimeError::FuelExhausted)
        } else {
            result
        }
    }
}
