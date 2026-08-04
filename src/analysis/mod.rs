pub mod address_space;
pub mod call_graph;
pub mod concurrency_gate;
pub mod cross_reference;
pub mod dataflow;
pub mod dependency_graph;
pub mod dfa;
pub mod entry_point;
pub mod equality_saturation;
pub mod pgo;
pub mod narrow_slice;
pub mod provenance;
pub mod protocol;
pub mod global_lifetime;
pub mod range;
pub mod roofline;
pub mod struct_generator;
pub mod gpu_cost;
pub mod region;
pub mod schema_validator;
pub mod transition_graph;
pub mod watchdog;
pub mod allocation;
pub mod meld_validation;
pub mod frgn_dispatch;
pub mod frgn_guard;
pub mod export_abi;
pub mod needs_state_projection;
pub mod string_concat;
pub mod boundary_marshalling;
pub mod slp_isomorphism;
pub mod soa_reorder;
pub mod soa_projection;
pub mod licm;
pub mod match_normalize;
pub mod node_decompose;
pub mod loop_carried;
pub mod layout_optimizer;
pub mod protocol_graph;
pub mod swan_song;
pub mod loop_shape;
pub mod density;
pub mod modulo_partition;
pub mod inline_cost;
pub mod batch_shape;
pub mod termination;

/// Determines how a state field behaves in the %State struct layout.
/// Used by the Adaptive Layout Engine (Phase 1) to eliminate unused fields
/// and allocate cache slots for meld-backed deferred projections.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldMode {
    /// Field is used and accessed directly — always present in %State.
    Always,
    /// Field has deferred projections (e.g. `strlen#` for CString .#Size)
    /// that are cached. Only used when both lenses are active in a hot loop.
    LazyCached {
        /// Index into the cache slot area appended after regular fields.
        cache_index: usize,
    },
    /// Field is never accessed through any lens — eliminated from %State.
    Never,
}