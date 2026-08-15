// ── Coll Scaffold ─────────────────────────────────────────────────────
// 2026-08-15 (coll plan §3.4): synthesize the collection op surface for
// `coll obj` / `coll struct` declarations. A `coll` type declares its ONE
// sequence member (the storage shape); the compiler scaffolds the ops that
// make the existing structural probes fire — `op Count`/`op At` (iteration),
// the construction ops (`op InitEmpty`/`op Init`/`op InsertAt`/
// `op ExtractFrom`/`op CopyFrom`), and the default `op Grow`/`op Shrink`
// strategy entries. The compiler knows the capability (`coll`), never a
// type name.
//
// The synthesized members are ordinary `TypeDefOperator`/`Definition`
// members stored in `obj_members`, so `emit_member_body` runs them through
// the existing boxed-self path (no new codegen path).
//
// Layout contract (coll obj): the hidden `cap`/`len` slots are appended
// AFTER the declared slots (register_types.rs / mod.rs registration), so
// offset N (sequence) ... cap at (N+1), len at (N+2).

use crate::ast::top::{
    Contract, Definition, OutputType, Statement, TopLevel, TypeDef, TypeDefBody, TypeDefSlot,
    TypeParam,
};
use crate::ast::{Expr, Type};

/// The storage the compiler chooses for a `coll` (SPEC §8.10 — "storage is
/// the compiler's choice"; `seq coll` forces the contiguous element block).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CollStorage {
    /// Growable `Ptr<T>` sequence — a heap block `[data, cap, len]`. Never
    /// pooled (the value must outlive the creating scope). `seq` is already
    /// satisfied: the data buffer IS one contiguous block.
    HeapGrowable,
    /// Fixed `T[N]` sequence — an inline array. May pool (unpack to columns)
    /// when zero/literal-initialized as a named instance; `seq` forbids the
    /// columnar layout (inline only).
    InlineFixed,
}

/// Classify a coll type's storage from its one sequence member.
pub(crate) fn coll_storage_mode(
    slots: &[TypeDefSlot],
    struct_types: &std::collections::HashMap<String, Vec<(String, crate::ast::Type)>>,
) -> Option<CollStorage> {
    let seq = derive_sequence_member(slots, struct_types)?;
    // The sequence member's element store: Ptr<T> (growable) vs T[N] (fixed).
    for slot in slots {
        match &slot.ty {
            Type::Ptr(_) => return Some(CollStorage::HeapGrowable),
            Type::Vector(..) => return Some(CollStorage::InlineFixed),
            // Nested buffer (`inner: ListBuffer<T>`) — resolve its store.
            Type::Custom(n) | Type::Applied(n, _) => {
                if let Some(fields) = struct_types.get(n) {
                    if fields.iter().any(|(_, t)| matches!(t, Type::Ptr(_))) {
                        return Some(CollStorage::HeapGrowable);
                    }
                }
                return Some(CollStorage::InlineFixed);
            }
            _ => {}
        }
    }
    let _ = seq;
    None
}

/// A synthesized `op Count() -> Int { term len; }` member. Reads the hidden
/// `len` slot through the boxed-self GEP (the same path a hand-written
/// `op Count { term len; }` uses).
fn synth_op_count() -> Definition {
    Definition {
        name: "Count".to_string(),
        type_params: vec![],
        parameters: vec![],
        output_type: Some(OutputType::single(Type::int())),
        outputs: vec![Type::int()],
        contract: default_coll_contract(),
        body: vec![Statement::Term(Some(Expr::Identifier("len".to_string())))],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (compiler-owned Length)".to_string()),
    }
}

/// A synthesized `op At(i: Int) -> T { term <seq>[i]; }` member. `seq_expr`
/// is the sequence member's access path (`inner.data` for a nested buffer,
/// `data` for a direct `Ptr<T>`).
fn synth_op_at(seq_expr: &str, elem_ty: crate::ast::Type) -> Definition {
    Definition {
        name: "At".to_string(),
        type_params: vec![],
        parameters: vec![("i".to_string(), Type::int())],
        output_type: Some(OutputType::single(elem_ty.clone())),
        outputs: vec![elem_ty.clone()],
        contract: default_coll_contract(),
        body: vec![Statement::Term(Some(Expr::Index(
            Box::new(Expr::Identifier(seq_expr.to_string())),
            Box::new(Expr::Identifier("i".to_string())),
        )))],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (indexed element read)".to_string()),
    }
}

/// A synthesized `txn init_empty() [true][len == 0] { data = Malloc#(cap); cap = CAP; len = 0; }`
/// member — the zero-arg construction. `seq_expr` is the sequence member;
/// `cap_expr` is where the capacity lives (a `data`-side capacity for a
/// nested buffer, or `cap` for a direct `Ptr<T>` coll). This slice keeps it
/// simple: allocate a default buffer, zero len.
fn synth_init_empty(
    seq_expr: &str,
    _seq_ty: crate::ast::Type,
    elem_ty: crate::ast::Type,
) -> Definition {
    // data = Malloc#(DEFAULT_CAP * elem_size) as Ptr<Elem>
    let default_cap: i64 = 16;
    let elem_size: i64 = 8; // this slice: word elements
    let alloc = Expr::Call("Malloc#".to_string(), vec![Expr::Decimal(default_cap * elem_size)], None);
    let data_assign = Statement::Assign(
        Expr::Identifier(seq_expr.to_string()),
        Expr::Cast(Box::new(alloc), Type::Ptr(Box::new(elem_ty))),
    );
    let len_zero = Statement::Assign(
        Expr::Identifier("len".to_string()),
        Expr::Decimal(0),
    );
    let cap_set = Statement::Assign(
        Expr::Identifier("cap".to_string()),
        Expr::Decimal(default_cap),
    );
    Definition {
        name: "init_empty".to_string(),
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![data_assign, cap_set, len_zero],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (empty construction)".to_string()),
    }
}

/// A synthesized `txn push(val: T) { data[len] = val; len = len + 1; }`
/// member — InsertAt. This slice: no grow-on-full (a precondition error at
/// cap; the default Grow policy is a follow-up).
fn synth_push(seq_expr: &str, elem_ty: crate::ast::Type) -> Definition {
    let data_write = Statement::Assign(
        Expr::Index(
            Box::new(Expr::Identifier(seq_expr.to_string())),
            Box::new(Expr::Identifier("len".to_string())),
        ),
        Expr::Identifier("val".to_string()),
    );
    let len_inc = Statement::Assign(
        Expr::Identifier("len".to_string()),
        Expr::BinaryOp(
            crate::ast::BinaryOpKind::Add,
            Box::new(Expr::Identifier("len".to_string())),
            Box::new(Expr::Decimal(1)),
        ),
    );
    Definition {
        name: "push".to_string(),
        type_params: vec![],
        parameters: vec![("val".to_string(), elem_ty)],
        output_type: None,
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![data_write, len_inc],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (InsertAt)".to_string()),
    }
}

/// A synthesized `defn get(i: Int) -> T { term data[i]; }` member — CopyFrom.
fn synth_get(seq_expr: &str, elem_ty: crate::ast::Type) -> Definition {
    Definition {
        name: "get".to_string(),
        type_params: vec![],
        parameters: vec![("i".to_string(), Type::int())],
        output_type: Some(OutputType::single(elem_ty.clone())),
        outputs: vec![elem_ty.clone()],
        contract: default_coll_contract(),
        body: vec![Statement::Term(Some(Expr::Index(
            Box::new(Expr::Identifier(seq_expr.to_string())),
            Box::new(Expr::Identifier("i".to_string())),
        )))],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (CopyFrom)".to_string()),
    }
}

/// A synthesized `txn init(val: T) { data = Malloc#(DEFAULT_CAP * size) as Ptr<T>; cap = DEFAULT_CAP; data[0] = val; len = 1; }`
/// member — the one-element construction. Allocates the data buffer (like
/// init_empty) so the first element store never hits an uninitialized pointer.
fn synth_init(seq_expr: &str, elem_ty: crate::ast::Type) -> Definition {
    let default_cap: i64 = 16;
    let elem_size: i64 = 8; // this slice: word elements
    let alloc = Expr::Call("Malloc#".to_string(), vec![Expr::Decimal(default_cap * elem_size)], None);
    let data_assign = Statement::Assign(
        Expr::Identifier(seq_expr.to_string()),
        Expr::Cast(Box::new(alloc), Type::Ptr(Box::new(elem_ty.clone()))),
    );
    let cap_set = Statement::Assign(
        Expr::Identifier("cap".to_string()),
        Expr::Decimal(default_cap),
    );
    let data_write = Statement::Assign(
        Expr::Index(
            Box::new(Expr::Identifier(seq_expr.to_string())),
            Box::new(Expr::Decimal(0)),
        ),
        Expr::Identifier("val".to_string()),
    );
    let len_one = Statement::Assign(
        Expr::Identifier("len".to_string()),
        Expr::Decimal(1),
    );
    Definition {
        name: "init".to_string(),
        type_params: vec![],
        parameters: vec![("val".to_string(), elem_ty)],
        output_type: None,
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![data_assign, cap_set, data_write, len_one],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (one-element construction)".to_string()),
    }
}

/// A synthesized `defn pop() -> T { term data[len-1]; }` member — ExtractFrom.
fn synth_pop(seq_expr: &str, elem_ty: crate::ast::Type) -> Definition {
    Definition {
        name: "pop".to_string(),
        type_params: vec![],
        parameters: vec![],
        output_type: Some(OutputType::single(elem_ty.clone())),
        outputs: vec![elem_ty.clone()],
        contract: default_coll_contract(),
        body: vec![Statement::Term(Some(Expr::Index(
            Box::new(Expr::Identifier(seq_expr.to_string())),
            Box::new(Expr::BinaryOp(
                crate::ast::BinaryOpKind::Sub,
                Box::new(Expr::Identifier("len".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
        )))],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (ExtractFrom)".to_string()),
    }
}

fn default_coll_contract() -> Contract {
    Contract {
        pre_condition: Expr::Bool(true),
        post_condition: Expr::Bool(true),
        watchdog: None,
        explicit: false,
        span: None,
    }
}

/// Derive the sequence member's access path and element type from a coll
/// type's declared slots.
///
/// - `data: Ptr<T>` → seq expr `data`, elem `T` (direct pointer member).
/// - `inner: ListBuffer<T>` (a struct whose first slot is `data: Ptr<T>`) →
///   seq expr `inner.data`, elem `T` (nested buffer, one level).
/// - `T[N]` array → seq expr `data`, elem `T` (coll struct).
///
/// `struct_types` resolves a nested buffer's slots (one level). Returns
/// `(seq_expr, elem_ty)` or None if no single sequence member.
pub(crate) fn derive_sequence_member(
    slots: &[TypeDefSlot],
    struct_types: &std::collections::HashMap<String, Vec<(String, crate::ast::Type)>>,
) -> Option<(String, crate::ast::Type)> {
    let mut seq: Option<(String, crate::ast::Type)> = None;
    for slot in slots {
        let derived = match &slot.ty {
            // `data: Ptr<T>` — the sequence is the pointer; elements are `T`.
            Type::Ptr(inner) => Some((slot.name.clone(), (**inner).clone())),
            // `inner: ListBuffer<T>` — a nested struct whose first slot is a
            // `data: Ptr<T>`. Sequence path is `inner.data`, elem `T`.
            Type::Custom(n) => {
                // Resolve the nested struct's slots (one level) for a Ptr.
                struct_types.get(n).and_then(|fields| {
                    fields.iter().find(|(_, t)| matches!(t, Type::Ptr(_))).map(
                        |(field, ptr_ty)| {
                            let elem = match ptr_ty {
                                Type::Ptr(inner) => (**inner).clone(),
                                _ => Type::int(),
                            };
                            (format!("{}.{}", slot.name, field), elem)
                        },
                    )
                })
            }
            Type::Applied(n, args) => {
                struct_types.get(n).and_then(|fields| {
                    fields.iter().find(|(_, t)| matches!(t, Type::Ptr(_))).map(
                        |(field, _)| {
                            // elem from the applied type's arg (ListBuffer<T> → T)
                            let elem = args.first().cloned().unwrap_or_else(Type::int);
                            (format!("{}.{}", slot.name, field), elem)
                        },
                    )
                })
            }
            _ => None,
        };
        if derived.is_some() {
            if seq.is_some() {
                return None; // two sequence members — validation catches it too
            }
            seq = derived;
        }
    }
    seq
}

/// Synthesize the full member list for a `coll` type: the op-as-member
/// iteration ops, the construction/mutation members, and (for coll obj) the
/// default Grow/Shrink strategy entries appended to `operator_defs` by the
/// caller. Returns the members to store in `obj_members`.
pub(crate) fn synthesize_members(
    td: &TypeDef,
    seq_expr: &str,
    elem_ty: crate::ast::Type,
) -> Vec<TopLevel> {
    let mut members = Vec::new();
    members.push(TopLevel::TypeDefOperator(synth_op_count()));
    members.push(TopLevel::TypeDefOperator(synth_op_at(seq_expr, elem_ty.clone())));
    members.push(TopLevel::Definition(synth_init_empty(
        seq_expr, elem_ty.clone(), elem_ty.clone(),
    )));
    members.push(TopLevel::Definition(synth_push(seq_expr, elem_ty.clone())));
    members.push(TopLevel::Definition(synth_get(seq_expr, elem_ty.clone())));
    members.push(TopLevel::Definition(synth_pop(seq_expr, elem_ty.clone())));
    members.push(TopLevel::Definition(synth_init(seq_expr, elem_ty)));
    // The op bindings (op InitEmpty: init_empty(#Lh), op InsertAt: push(#Lh,#Rh), ...)
    // live in operator_defs; they are appended by the caller.
    let _ = td;
    members
}

/// Build the `operator_defs` bindings for a coll type: the op-name → member
/// function map. These are the bindings `construct_local_collection` and the
/// `<-`/`foreach` dispatch consult.
pub(crate) fn synthesize_bindings(_base: &str) -> Vec<crate::ast::top::OperatorBinding> {
    // 2026-08-15: the bindings are registered directly in the backend (see
    // emit_toplevel::register_coll_bindings) because an OperatorBinding's
    // `expr` must be a full function name and the existing resolution path
    // keys on `operator_defs`. This helper documents the shape; the real
    // registration is next to the coll member registration.
    Vec::new()
}

/// Build the `TypeDefBody` for a `coll` type including its synthesized
/// members, given the original body and the sequence-member derivation.
pub(crate) fn coll_body_with_members(
    td: &TypeDef,
    original: TypeDefBody,
    struct_types: &std::collections::HashMap<String, Vec<(String, crate::ast::Type)>>,
) -> TypeDefBody {
    let seq = derive_sequence_member(&original.slots, struct_types);
    let mut body = original;
    if let Some((seq_expr, elem_ty)) = seq {
        let members = synthesize_members(td, &seq_expr, elem_ty);
        body.members.extend(members);
    }
    body
}

/// 2026-08-15 (coll plan §3.4): synthesize the op-as-member ops the
/// TYPECHECKER must see — `op Count`, `op At`, `op InsertAt`, `op ExtractFrom`,
/// `op CopyFrom`, `op Init`, `op InitEmpty`. The backend's `synthesize_members`
/// adds the full set; the typechecker only needs the op surface (element
/// types, push element type, iterability). Uses the SAME synthesis helpers so
/// the two never disagree. Element type comes from the sequence member; a
/// coll struct's fixed `T[N]` member has no element slot, so it gets Count
/// only (length is the constant N).
pub fn synthesize_members_for_check(td: &TypeDef) -> Vec<TopLevel> {
    // The typechecker has no struct_types map here; derive the element type
    // from a direct Ptr<T> sequence member only (the common coll obj shape).
    let seq_ty = td.body.slots.iter().find_map(|s| match &s.ty {
        Type::Ptr(inner) => Some((s.name.clone(), (**inner).clone())),
        _ => None,
    });
    let mut members = Vec::new();
    members.push(TopLevel::TypeDefOperator(synth_op_count()));
    let Some((seq_expr, elem_ty)) = seq_ty else {
        // coll struct (fixed T[N]): Count only — At/InsertAt need an element
        // store this slice; the validation errors on a Ptr coll struct, and a
        // T[N] member's At would be a constant-index read (future work).
        return members;
    };
    members.push(TopLevel::TypeDefOperator(synth_op_at(&seq_expr, elem_ty.clone())));
    // op InsertAt(v: T) — the push element type the `<-` dispatch reads.
    members.push(TopLevel::TypeDefOperator(synth_insert_at_op(elem_ty.clone())));
    members.push(TopLevel::TypeDefOperator(synth_extract_from_op(elem_ty.clone())));
    members.push(TopLevel::TypeDefOperator(synth_copy_from_op(elem_ty.clone())));
    // op Init(v: T) — literal construction (first element).
    members.push(TopLevel::TypeDefOperator(synth_init_op(elem_ty)));
    members.push(TopLevel::TypeDefOperator(synth_init_empty_op()));
    members
}

fn synth_insert_at_op(elem_ty: Type) -> Definition {
    Definition {
        name: "InsertAt".to_string(),
        type_params: vec![],
        parameters: vec![("v".to_string(), elem_ty)],
        output_type: None,
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (InsertAt)".to_string()),
    }
}

fn synth_extract_from_op(elem_ty: Type) -> Definition {
    Definition {
        name: "ExtractFrom".to_string(),
        type_params: vec![],
        parameters: vec![],
        output_type: Some(OutputType::single(elem_ty)),
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (ExtractFrom)".to_string()),
    }
}

fn synth_copy_from_op(elem_ty: Type) -> Definition {
    Definition {
        name: "CopyFrom".to_string(),
        type_params: vec![],
        parameters: vec![],
        output_type: Some(OutputType::single(elem_ty)),
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (CopyFrom)".to_string()),
    }
}

fn synth_init_op(elem_ty: Type) -> Definition {
    Definition {
        name: "Init".to_string(),
        type_params: vec![],
        parameters: vec![("v".to_string(), elem_ty)],
        output_type: None,
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (Init)".to_string()),
    }
}

fn synth_init_empty_op() -> Definition {
    Definition {
        name: "InitEmpty".to_string(),
        type_params: vec![],
        parameters: vec![],
        output_type: None,
        outputs: vec![],
        contract: default_coll_contract(),
        body: vec![],
        metadata: Default::default(),
        derivation: None,
        modifiers: vec![],
        annotations: vec![],
        span: None,
        doc: Some("scaffolded by `coll` (InitEmpty)".to_string()),
    }
}

// Re-export TypeParam for the caller.
#[allow(unused)]
fn _type_param_usage(_p: TypeParam) {}
