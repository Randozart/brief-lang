// ── Backend Context Architecture ───────────────────────────────────────
//
// 2026-06-29: Three-tier context separation to eliminate the fragile
// "save/restore" anti-pattern that previously required manual cloning of
// 7+ fields at every inline txn boundary (see dispatch.rs emit_inline_txn_body).
//
// Why three contexts instead of one:
//   CompilerContext — global, read-only during codegen. Holds AST definitions,
//     target spec, FFI signatures, type info. Never modified once codegen starts.
//   FunctionContext — per-function, mutable. Holds SSA counter, local bindings,
//     phi state, arena slots. Cloned at inline boundaries; restored after.
//   BlockContext — per-basic-block, lightweight. Tracks current label and
//     transient allocations. Rarely needed beyond label tracking.
//
// Why not just make all per-function fields Clone-and-restore:
//   Cloning HashMaps at every inline boundary has measurable cost, but the
//   functional correctness benefit (eliminating state contamination) far
//   outweighs the overhead. If profiling shows this is a bottleneck, switch
//   to an arena-backed HashMap or an indexed slot approach.

use crate::analysis::dependency_graph::DependencyGraph;
use crate::analysis::pgo::PgoProfile;
use crate::ast::{
    CellDef, EnumDefinition, Expr, ForeignSignature, InopDeclaration,
    Statement, TriggerDeclaration, Type,
};
use crate::backend::llvm::directive::OptimizationRemark;
use crate::backend::llvm::ChimeraInfo;
use crate::dbrief::DbriefType;
use crate::target_spec::TargetSpec;
use crate::type_universe::TypeUniverse;
use crate::analysis::FieldMode;
use std::collections::HashMap;
use std::collections::HashSet;

// ── CompilerContext ───────────────────────────────────────────────────
//
// Global compilation context — immutable once code generation begins.
// Holds everything that does not change between functions/transactions.
// Previously part of the LlvmBackend god object; extracted to prevent
// accidental mutation during per-function codegen.
#[derive(Debug, Clone)]
pub struct CompilerContext {
    // Target & Spec
    pub spec: Option<TargetSpec>,
    pub explain: bool,
    pub dump_layout: bool,
    pub library_mode: bool,

    // State layout (built during generate(), then read-only)
    pub field_index_map: HashMap<String, usize>,
    pub field_types: Vec<String>,
    pub field_brief_types: Vec<Type>,
    pub field_initializers: HashMap<String, Option<Expr>>,
    pub field_modes: HashMap<String, FieldMode>,
    pub cache_slots: HashMap<String, HashMap<String, (usize, usize)>>,
    pub range_bounds: HashMap<String, (i64, i64)>,
    pub field_to_meta_idx: HashMap<String, usize>,
    pub exit_condition: Option<Box<Expr>>,
    pub has_natural_exit: bool,

    // MMIO & Schema
    pub mmio_fields: HashMap<String, u64>,
    pub mmio_initializers: HashMap<String, Option<Expr>>,
    pub mmio_prepopulated: bool,
    pub schema_aliases: HashMap<String, DbriefType>,

    // FFI & Declarations
    pub triggers: HashMap<String, TriggerDeclaration>,
    pub trigger_names: Vec<String>,
    pub frgn_map: HashMap<String, ForeignSignature>,
    pub inop_decls: HashMap<String, InopDeclaration>,
    pub defn_params: HashMap<String, Vec<Type>>,
    pub defn_return_types: HashMap<String, Vec<Type>>,

    // Constants & Strings
    pub string_constants: Vec<String>,
    pub constants: HashMap<String, (Type, Expr)>,

    // Type info
    pub struct_types: HashMap<String, Vec<(String, Type)>>,
    pub enum_types: HashMap<String, EnumDefinition>,
    pub cell_defs: HashMap<String, CellDef>,
    pub cell_state_types: HashMap<String, (HashMap<String, usize>, Vec<String>)>,
    pub cell_wires: Vec<(String, String, String, String)>,
    pub cell_trigger_bindings: Vec<(String, String, String)>,
    pub variant_disc: HashMap<String, (String, u64, usize)>,

    // Optimization
    pub optimize_budget: u64,
    pub optimize_report: bool,
    pub optimize_size: Option<u64>,
    pub pgo_profile: Option<PgoProfile>,
    pub dead_info_disabled: bool,
    pub emit_remarks: bool,
    pub has_cycles: bool,
    pub slp_hazard_fns: HashSet<String>,

    // GPU config
    pub gpu_offload: bool,
    pub gpu_backend: String,

    // Embedded mode
    pub is_embedded: bool,
    pub type_universe: Option<TypeUniverse>,

    // Dependency graph (built during generate(), then read-only)
    pub dep_graph: DependencyGraph,
}

impl CompilerContext {
    pub fn new() -> Self {
        CompilerContext {
            spec: None,
            explain: false,
            dump_layout: false,
            library_mode: false,
            field_index_map: HashMap::new(),
            field_types: Vec::new(),
            field_brief_types: Vec::new(),
            field_initializers: HashMap::new(),
            field_modes: HashMap::new(),
            cache_slots: HashMap::new(),
            range_bounds: HashMap::new(),
            field_to_meta_idx: HashMap::new(),
            exit_condition: None,
            has_natural_exit: false,
            mmio_fields: HashMap::new(),
            mmio_initializers: HashMap::new(),
            mmio_prepopulated: false,
            schema_aliases: HashMap::new(),
            triggers: HashMap::new(),
            trigger_names: Vec::new(),
            frgn_map: HashMap::new(),
            inop_decls: HashMap::new(),
            defn_params: HashMap::new(),
            defn_return_types: HashMap::new(),
            string_constants: Vec::new(),
            constants: HashMap::new(),
            struct_types: HashMap::new(),
            enum_types: HashMap::new(),
            cell_defs: HashMap::new(),
            cell_state_types: HashMap::new(),
            cell_wires: Vec::new(),
            cell_trigger_bindings: Vec::new(),
            variant_disc: HashMap::new(),
            optimize_budget: 256,
            optimize_report: false,
            optimize_size: None,
            pgo_profile: None,
            dead_info_disabled: false,
            emit_remarks: false,
            has_cycles: false,
            slp_hazard_fns: HashSet::new(),
            gpu_offload: false,
            gpu_backend: "vulkan".to_string(),
            is_embedded: false,
            type_universe: None,
            dep_graph: DependencyGraph {
                topo_order: Vec::new(),
                bit_index: HashMap::new(),
                dependencies: HashMap::new(),
                dependents: HashMap::new(),
                is_trg: HashSet::new(),
                all_vars: HashSet::new(),
            },
        }
    }
}

// ── FunctionContext ────────────────────────────────────────────────────
//
// Per-function/transaction mutable state. Instantiated at the start of
// each function emission and discarded at the end. Previously these fields
// lived on LlvmBackend and required manual save/restore when inlining
// transaction bodies — a constant source of state contamination bugs.
//
// When inlining, clone this struct before entering the inline body and
// restore it after. FunctionContext implements Clone for this purpose.
#[derive(Debug, Clone)]
pub struct FunctionContext {
    // SSA register counters — NEVER rewound (prevents %t{N} collisions)
    pub txn_counter: usize,
    pub within_counter: usize,
    pub metadata_counter: usize,
    /// Arena bump counter for unique per-allocation register names
    pub arena_counter: usize,

    // Local variable bindings (let x = ...)
    pub let_bindings: HashMap<String, String>,
    pub let_binding_types: HashMap<String, Type>,
    pub let_original_types: HashMap<String, Type>,

    // Register type caches
    pub reg_float_cache: HashMap<String, String>,
    pub reg_type_cache: HashMap<String, Type>,

    // Phi/loop state
    pub ssa_old_int_regs: HashMap<String, String>,
    pub ssa_old_float_regs: HashMap<String, String>,
    pub pending_phi_backedge: HashMap<String, String>,
    pub phi_field_regs: HashMap<String, String>,
    pub backedge_field_regs: HashMap<String, String>,
    pub used_phi_loop: bool,
    pub phi_induction_reg: Option<(String, String, String)>,
    pub loop_exit_label: Option<String>,

    // Function-level state flags
    pub terminated: bool,
    pub returns_i64: bool,
    pub fn_ret_ty: String,
    pub main_body: bool,
    pub in_callable_txn: bool,
    pub callable_txn_result: Option<String>,
    pub callable_txn_post_label: Option<String>,

    // SSA state
    pub ssa_state_reg: Option<String>,
    pub param_slots: HashMap<String, String>,
    pub state_reg_name: String,

    // Arena allocator state (per-function)
    pub arena_slots: Option<(String, String, String)>,
    pub field_prealloc_info: HashMap<String, (String, String)>,

    // Whether the canonical loop bound is a compile-time constant
    // 2026-07-01: Enables post-inc comparison (counting-down loop) for
    // static bounds — LLVM can emit `add + jne` instead of `cmp + add + jl`.
    // For dynamic (runtime-determined) bounds, pre-inc comparison is used.
    pub is_static_bound: bool,

    // Accumulators flushed per-function
    pub pending_metadata: String,
    pub pending_post_hoist: Vec<(String, String)>,
    pub pending_cleanup: Vec<Statement>,

    // Chimera tracking
    pub chimera_map: HashMap<String, ChimeraInfo>,

    // Expression hash-consing dedup cache.
    // 2026-07-01: Maps (op_string, lhs_reg, rhs_reg) → result_reg.
    // Prevents emitting the same instruction twice within a body emission scope.
    // Persists across let-bindings (not cleared between body statements) so that
    // sub-expressions like `dxe23*dxe23` that appear in multiple statements
    // (e.g., energy computation) reuse the emitted register.
    // Only caches "expensive" ops (fp ops, division) — not cheap integer add/sub.
    // Cleared at function entry alongside other caches.
    pub expr_dedup_cache: HashMap<(String, String, String), String>,
}

impl FunctionContext {
    pub fn new() -> Self {
        FunctionContext {
            txn_counter: 0,
            within_counter: 0,
            metadata_counter: 100,
            arena_counter: 0,
            let_bindings: HashMap::new(),
            let_binding_types: HashMap::new(),
            let_original_types: HashMap::new(),
            reg_float_cache: HashMap::new(),
            reg_type_cache: HashMap::new(),
            ssa_old_int_regs: HashMap::new(),
            ssa_old_float_regs: HashMap::new(),
            pending_phi_backedge: HashMap::new(),
            phi_field_regs: HashMap::new(),
            backedge_field_regs: HashMap::new(),
            used_phi_loop: false,
            phi_induction_reg: None,
            loop_exit_label: None,
            terminated: false,
            returns_i64: false,
            fn_ret_ty: "void".to_string(),
            main_body: false,
            in_callable_txn: false,
            callable_txn_result: None,
            callable_txn_post_label: None,
            ssa_state_reg: None,
            param_slots: HashMap::new(),
            state_reg_name: "%state".to_string(),
            arena_slots: None,
            field_prealloc_info: HashMap::new(),
            is_static_bound: false,
            pending_metadata: String::new(),
            pending_post_hoist: Vec::new(),
            pending_cleanup: Vec::new(),
            chimera_map: HashMap::new(),
            expr_dedup_cache: HashMap::new(),
        }
    }

    /// Generate a unique SSA register name within this function.
    /// This is the SOLE source of register names — never use format!("%t{}", counter)
    /// outside this method. Guarantees no duplicate `%t{N}` definitions.
    pub fn next_reg(&mut self) -> String {
        let r = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        r
    }

    /// Generate a unique label name within this function.
    pub fn next_label(&mut self, prefix: &str) -> String {
        let l = format!("{}_{}", prefix, self.txn_counter);
        self.txn_counter += 1;
        l
    }

    /// Generate a unique register with a custom prefix (for non-%t{N} names).
    /// Used by type-conversion helpers and specialized intrinsic emitters.
    pub fn next_reg_with_prefix(&mut self, prefix: &str) -> String {
        let r = format!("%{}{}", prefix, self.txn_counter);
        self.txn_counter += 1;
        r
    }

    /// Clear all local variable bindings (used at function entry).
    pub fn clear_locals(&mut self) {
        self.let_bindings.clear();
        self.let_binding_types.clear();
        self.let_original_types.clear();
        self.reg_float_cache.clear();
        self.reg_type_cache.clear();
    }

    /// Reset function state for a new function (keeps txn_counter if needed).
    pub fn reset(&mut self) {
        self.txn_counter = 0;
        self.within_counter = 0;
        self.clear_locals();
        self.terminated = false;
        self.returns_i64 = false;
        self.fn_ret_ty = "void".to_string();
        self.main_body = false;
        self.in_callable_txn = false;
        self.callable_txn_result = None;
        self.callable_txn_post_label = None;
        self.loop_exit_label = None;
        self.phi_induction_reg = None;
        self.arena_slots = None;
        self.field_prealloc_info.clear();
        self.pending_metadata.clear();
        self.pending_cleanup.clear();
        self.chimera_map.clear();
    }
}

// ── BlockContext ──────────────────────────────────────────────────────
//
// Per-basic-block lightweight context. Primarily tracks the current block
// label. In the future, this may track transient allocations that should
// be freed at block exit.
#[derive(Debug, Clone)]
pub struct BlockContext {
    pub label: String,
}

impl BlockContext {
    pub fn new(label: &str) -> Self {
        BlockContext {
            label: label.to_string(),
        }
    }

    pub fn entry() -> Self {
        BlockContext {
            label: "entry".to_string(),
        }
    }
}

// ── Function Guard ────────────────────────────────────────────────────
//
// Saves a FunctionContext snapshot for later restoration.
// Unlike the RAII pattern (which creates borrow-checker conflicts because
// it holds &mut FunctionContext while the caller also needs &mut self.fun),
// this guard is explicitly restored via restore().
//
// This is still safer than the original 7-field save/restore pattern because
// it snapshots ALL fields — adding new FunctionContext fields automatically
// protects them without editing the save/restore code.
//
// Usage:
//   let guard = FunctionGuard::new(&self.fun);
//   self.fun.terminated = false;
//   // ... modify self.fun extensively ...
//   guard.restore(&mut self.fun);
pub struct FunctionGuard {
    saved: FunctionContext,
}

impl FunctionGuard {
    pub fn new(fun: &FunctionContext) -> Self {
        FunctionGuard {
            saved: fun.clone(),
        }
    }

    /// Restore the FunctionContext to the state captured at construction time.
    /// Call this after the inline body has been emitted.
    pub fn restore(self, fun: &mut FunctionContext) {
        *fun = self.saved;
    }

    // 2026-07-01: Restore all state EXCEPT SSA register counters.
    //
    // When inlining multiple txn bodies into the same function (e.g., in
    // emit_reactor's emit_inline_txn_body), restore() rewinds txn_counter
    // and arena_counter to the pre-body snapshot value. The second body then
    // emits identical register names (%dab263, %t7, etc.), causing "multiple
    // definition of local value" errors from opt.
    //
    // This variant preserves the monotonic counter invariants documented on
    // txn_counter ("NEVER rewound — prevents %t{N} collisions") and
    // arena_counter, while still restoring all other state (local bindings,
    // caches, phi state, flags).
    //
    // Trade-off: Register numbers grow monotonically across the full function
    // (~0.1% longer names at scale). No functional impact — LLVM normalizes
    // register names in its own passes.
    pub fn restore_preserve_counters(self, fun: &mut FunctionContext) {
        let txn_ct = fun.txn_counter;
        let arena_ct = fun.arena_counter;
        let within_ct = fun.within_counter;
        let md_ct = fun.metadata_counter;
        *fun = self.saved;
        fun.txn_counter = txn_ct;
        fun.arena_counter = arena_ct;
        fun.within_counter = within_ct;
        fun.metadata_counter = md_ct;
    }
}
