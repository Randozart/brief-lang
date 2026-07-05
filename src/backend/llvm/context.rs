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

/// 2026-07-02: Indices of the 4 inline RingBuffer fields in %State.
/// Used by emit_arrow_push/emit_arrow_discard to access RingBuf fields
/// directly via GEP instead of inttoptr on an opaque handle. This lets
/// LLVM's SROA promote the fields to SSA registers.
#[derive(Debug, Clone)]
pub struct RingbufInlineFields {
    pub data_idx: usize,
    pub head_idx: usize,
    pub tail_idx: usize,
    pub mask_idx: usize,
}
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
        pub ringbuf_inline: HashMap<String, RingbufInlineFields>,
    /// 2026-07-02: Tracks RingBuffer variables whose fields are stored inline
    /// in %State (data_ptr, head, tail, mask) instead of via an opaque handle.
    /// Maps base name → indices of the 4 inline fields.
    pub field_modes: HashMap<String, FieldMode>,
    pub cache_slots: HashMap<String, HashMap<String, (usize, usize)>>,
    pub range_bounds: HashMap<String, (i64, i64)>,
    pub field_to_meta_idx: HashMap<String, usize>,
    // 2026-07-04: Metadata ID for the !StateAliasScope used by !noalias
    // on Ptr<T> volatile accesses. Set during IR emission, then read-only
    // by intrinsics.rs volatile_load#/volatile_store# emission.
    pub state_alias_scope_md: usize,
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
            ringbuf_inline: HashMap::new(),
            field_modes: HashMap::new(),
            cache_slots: HashMap::new(),
            range_bounds: HashMap::new(),
            field_to_meta_idx: HashMap::new(),
            state_alias_scope_md: 0,
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
    // 2026-07-03: Full Statement blocks hoisted from body, not just field+intrinsic
    // pairs. Allows hoisting guards whose swan song references let-bindings (e.g.
    // `energy` in nbody) by re-emitting the entire guard body post-loop.
    pub pending_post_hoist: Vec<Vec<Statement>>,
    pub pending_cleanup: Vec<Statement>,
    // 2026-07-03: Native-typed backedge values for per-field phi loops.
    // Populated by emit_memory_field_store when it computes the typed value
    // (after ensure_typed_value).  Used by emit_countable_latch to avoid
    // reloading from %State — the typed register name is substituted directly
    // as the phi backedge operand, eliminating the store→GEP→load roundtrip.
    pub pending_phi_native_backedge: HashMap<String, String>,

    // 2026-07-04: Whether the A005c hot loop body must emit stores to %State
    // for the done: block to read. When false (the common case — no post-loop
    // hoisted guards), the field stores are suppressed. The phi registers and
    // pending_phi_native_backedge carry all values forward, and the latch uses
    // the native backedge registers directly. LLVM's optimizer sees a clean
    // phi loop with zero memory traffic — no dead stores for DSE to eliminate,
    // no barriers for the vectorizer.
    //
    // Dual-path architecture:
    //   Path A (false): Zero stores in the hot loop body. The loop body
    //     is a pure register pipeline (phi → compute → latch backedge).
    //     Used when done: does not read %State (no pending_post_hoist).
    //     Enables full vectorization and ILP scheduling.
    //   Path B (true): Stores emitted as before. Required when done:
    //     reads %State via GEP+load (post-loop hoisted guards from
    //     term! -> swan_song). The stores ensure done:'s loads see
    //     the final iteration's field values.
    //
    // Both paths must be preserved when refactoring. Removing Path A
    // regresses all A005c benchmarks by N dead stores per iteration
    // (N = field count). Removing Path B breaks term! swan song
    // correctness for benchmarks that print at convergence.
    pub needs_state_stores_in_body: bool,

    // 2026-07-04: Whether the current loop body is parallel-safe.
    // When true, emit_memory_field_store does NOT update ssa_old_*_regs
    // after & assignments — all reads continue to use the phi register
    // (old value).  This makes every computation independent of every
    // other, enabling LLVM's vectorizer to SIMD the entire body.
    //
    // Enabled for ALL bodies.  This restores the A005a struct-SSA behavior:
    // extractvalue from the state phi always gives old values, so all
    // computations are naturally independent.  The per-field phi loop
    // (A005c) broke this by updating ssa_old caches after each &
    // (correct per Brief semantics but creates artificial dependency
    // chains).  Parallel-safe mode restores the A005a independence.
    //
    // Exception: the counter field (tracked by counter_field_name) always
    // updates ssa_old_*_regs even in parallel-safe mode.  Guard conditions
    // like [count % 5000000 == 0] read the counter — they must see the
    // new value, not the old phi register.
    pub parallel_safe_body: bool,

    // 2026-07-04: Name of the loop counter field (the induction variable).
    // Set by emit_countable_main when entering the loop body.  This field
    // is exempt from parallel-safe mode — it always updates ssa_old_*_regs
    // so guard conditions like [count % N == 0] see the correct new value.
    pub counter_field_name: Option<String>,

    // 2026-07-04: State fields that guard conditions read.
    // Populated by scanning the body for Guarded statements and collecting
    // Expr::Identifier references.  These fields are exempt from parallel-
    // safe mode — they always update ssa_old_*_regs so guards see the
    // correct new values.  The counter_field_name is also implicitly exempt
    // (tracked separately as it's always the induction variable).
    // Guards containing TermBang (terminating guards) are excluded from
    // this scan — their bodies are hoisted and re-emitted post-loop, so
    // they don't need sequential updates within the loop body.
    pub parallel_safe_exempt_fields: HashSet<String>,

    // 2026-07-04: State fields that the done: block reads via
    // emit_hoisted_post_loop_prints. Populated by scanning hoisted
    // statements for Expr::Identifier references. When non-empty,
    // only these fields get stores emitted in Path B
    // (needs_state_stores_in_body=true). Empty set means "all fields
    // needed" (fallback — emit all stores).
    pub done_needs_fields: HashSet<String>,

    // 2026-07-04: Last-value temporaries (allocas) for phi commit block.
    // When the done: block reads state fields (hoisted prints), these
    // allocas store the phi's final value ONCE at loop exit (in the commit
    // block).  emit_hoisted_post_loop_prints loads from these instead of
    // from %State, eliminating per-iteration stores.  Maps field_name →
    // alloca register name.
    pub last_val_temps: HashMap<String, String>,

    // 2026-07-05: Fields participating in a body rotation pattern.
    // Forces body stores for these fields so the latch can reload them
    // via GEP, breaking circular phi chains for SCEV analysis.
    pub rotation_fields: HashSet<String>,

    // 2026-07-05: Vector phi groups for register pressure reduction.
    // When multiple scalar fields form a contiguous group (e.g., vx0..vx3),
    // they are promoted to a single <4 x float> phi node.  This reduces
    // register pressure in the hot loop from 32 scalar phis to ~8 vector
    // phis, eliminating register spills (nbody_sqrt: 16 spills → 0).
    // Maps vector phi register name → Vec of field names it covers.
    pub vector_phi_groups: HashMap<String, Vec<String>>,

    // 2026-07-05: Tracks the current accumulated vector value during body
    // emission for insertelement chaining.  Maps vector phi register name
    // → the most recent insertelement result register.
    pub vector_phi_current: HashMap<String, String>,

    // Chimera tracking

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
            pending_phi_native_backedge: HashMap::new(),
            needs_state_stores_in_body: true,
            parallel_safe_body: true,
            counter_field_name: None,
            parallel_safe_exempt_fields: HashSet::new(),
            done_needs_fields: HashSet::new(),
            last_val_temps: HashMap::new(),
            rotation_fields: HashSet::new(),
            vector_phi_groups: HashMap::new(),
            vector_phi_current: HashMap::new(),
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
