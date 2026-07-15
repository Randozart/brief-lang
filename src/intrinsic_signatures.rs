// ── Intrinsic Signature Registry ────────────────────────────────────────
// 2026-07-14: Generic operations. Types inferred from arguments.
// Adding a new intrinsic: add one arm here + one arm in execute_intrinsic().
// Every generic op uses vec![] for param_types — type is inferred.
//
// 2026-07-15: ReturnKind replaces return_type: Option<Type> for
// backend-agnostic type dispatch. Native("Int") means "backend's native
// integer type" — LLVM maps to i64 { primitive <~ Int, bytes <~ 8 }.
// Inferred means polymorphic (return matches argument type).

use crate::ast::Type;

/// How an intrinsic's return type is determined.
/// 2026-07-15: Backend-agnostic type dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum ReturnKind {
    /// Backend-native type — exact representation depends on target.
    /// LLVM: #Int → i64 { primitive <~ Int, bytes <~ 8 }
    ///       #Float → double { primitive <~ Float, bytes <~ 8 }
    Native(&'static str),
    /// Inferred from argument types (e.g. Add# returns same as input).
    Inferred,
    /// Fixed concrete type (pointer, void, etc.).
    Exact(Type),
}

/// The signature of a compiler-known # intrinsic.
pub struct Signature {
    pub name: &'static str,
    pub parameters: Vec<(&'static str, Type)>,
    pub return_kind: ReturnKind,
    /// If true, this intrinsic has observable side effects (DCE guard).
    pub observable: bool,
}

/// Look up the signature of a # intrinsic by name.
pub fn get_intrinsic_signature(name: &str) -> Option<Signature> {
    match name {
        // ── Arithmetic (return matches input type) ────────────────────
        "Add#" => Some(Signature { name: "Add#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Sub#" => Some(Signature { name: "Sub#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Mul#" => Some(Signature { name: "Mul#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Div#" => Some(Signature { name: "Div#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Rem#" => Some(Signature { name: "Rem#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Neg#" => Some(Signature { name: "Neg#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Abs#" => Some(Signature { name: "Abs#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),

        // ── Comparison (returns Bool, but type-inferred) ──────────────
        "Eq#"  => Some(Signature { name: "Eq#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Neq#" => Some(Signature { name: "Neq#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Lt#"  => Some(Signature { name: "Lt#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Gt#"  => Some(Signature { name: "Gt#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Le#"  => Some(Signature { name: "Le#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Ge#"  => Some(Signature { name: "Ge#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),

        // ── Float math (returns native Float) ─────────────────────────
        "Sqrt#"  => Some(Signature { name: "Sqrt#",  parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "Sin#"   => Some(Signature { name: "Sin#",   parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "Cos#"   => Some(Signature { name: "Cos#",   parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "Fabs#"  => Some(Signature { name: "Fabs#",  parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "Ceil#"  => Some(Signature { name: "Ceil#",  parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "Floor#" => Some(Signature { name: "Floor#", parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "Pow#"   => Some(Signature { name: "Pow#",   parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),

        // ── Memory (observable) ─────────────────────────────────────
        "Malloc#"  => Some(Signature { name: "Malloc#",  parameters: vec![("size", Type::int())], return_kind: ReturnKind::Exact(Type::ptr(Type::bits(1))), observable: true }),
        "Free#"    => Some(Signature { name: "Free#",    parameters: vec![("ptr", Type::ptr(Type::bits(1)))], return_kind: ReturnKind::Exact(Type::void()), observable: true }),
        "Load#"    => Some(Signature { name: "Load#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: true }),
        "Store#"   => Some(Signature { name: "Store#",   parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true }),
        "Copy#"    => Some(Signature { name: "Copy#",    parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true }),
        "Fill#"    => Some(Signature { name: "Fill#",    parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true }),

        // ── I/O (observable, returns native Int) ──────────────────────
        "Print#"  => Some(Signature { name: "Print#",  parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true }),
        "GetEnv#" => Some(Signature { name: "GetEnv#", parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: true }),

        // ── String ───────────────────────────────────────────────────
        "Concat#"    => Some(Signature { name: "Concat#",    parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Length#"    => Some(Signature { name: "Length#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        "ToInt#"     => Some(Signature { name: "ToInt#",     parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        "ToFloat#"   => Some(Signature { name: "ToFloat#",   parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false }),
        "ToString#"  => Some(Signature { name: "ToString#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),

        // ── Collection ───────────────────────────────────────────────
        "Get#"    => Some(Signature { name: "Get#",    parameters: vec![], return_kind: ReturnKind::Inferred, observable: false }),
        "Insert#" => Some(Signature { name: "Insert#", parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true }),

        // ── GPU ───────────────────────────────────────────────────────
        "GetGlobalId#"   => Some(Signature { name: "GetGlobalId#",   parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        "GetGlobalSize#" => Some(Signature { name: "GetGlobalSize#", parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        "GetLocalId#"    => Some(Signature { name: "GetLocalId#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        // 2026-07-15: Additional GPU intrinsics
        "GetGroupId#"    => Some(Signature { name: "GetGroupId#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        "GetNumGroups#"  => Some(Signature { name: "GetNumGroups#",  parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),
        "Dims#"          => Some(Signature { name: "Dims#",          parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false }),

        // ── Pointers (compile-time address resolution) ──────────────
        "AddressOf#" => Some(Signature {
            name: "AddressOf#",
            parameters: vec![("id", Type::string())],
            return_kind: ReturnKind::Exact(Type::ptr(Type::bits(8))),
            observable: false,
        }),

        // ── OS SysCall (observable) ────────────────────────────────────
        // 2026-07-15: Returns native Int (result or errno).
        "SysCall#" => Some(Signature {
            name: "SysCall#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),

        // ── POSIX SysConf (observable) ─────────────────────────────────
        // 2026-07-15: Returns native Int (page size, CPU count, etc.).
        "SysConf#" => Some(Signature {
            name: "SysConf#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),

        // ── Atomic operations (LLVM atomicrmw / load atomic / fence) ──
        // 2026-07-15: All return native Int except Fence# (void).
        "AtomicLoad#" => Some(Signature {
            name: "AtomicLoad#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: false,
        }),
        "AtomicStore#" => Some(Signature {
            name: "AtomicStore#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),
        "AtomicCas#" => Some(Signature {
            name: "AtomicCas#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),
        "AtomicXchg#" => Some(Signature {
            name: "AtomicXchg#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),
        "AtomicAdd#" => Some(Signature {
            name: "AtomicAdd#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),
        "Fence#" => Some(Signature {
            name: "Fence#",
            parameters: vec![],
            return_kind: ReturnKind::Exact(Type::void()),
            observable: true,
        }),

        // ── Dynamic linker (platform library functions) ──────────────
        // 2026-07-15: Return pointers (DlOpen#, DlSym#) or Int (DlClose#).
        "DlOpen#" => Some(Signature {
            name: "DlOpen#",
            parameters: vec![],
            return_kind: ReturnKind::Exact(Type::ptr(Type::bits(8))),
            observable: true,
        }),
        "DlSym#" => Some(Signature {
            name: "DlSym#",
            parameters: vec![],
            return_kind: ReturnKind::Exact(Type::ptr(Type::bits(8))),
            observable: false,
        }),
        "DlClose#" => Some(Signature {
            name: "DlClose#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),

        // ── Debugging (stack trace) ──────────────────────────────────
        // 2026-07-15: Returns frame count (native Int).
        "Backtrace#" => Some(Signature {
            name: "Backtrace#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
        }),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_intrinsics_have_signatures() {
        let intrinsics = [
            "Add#", "Sub#", "Mul#", "Div#", "Rem#", "Neg#", "Abs#",
            "Eq#", "Neq#", "Lt#", "Gt#", "Le#", "Ge#",
            "Sqrt#", "Sin#", "Cos#", "Fabs#", "Ceil#", "Floor#", "Pow#",
            "Malloc#", "Free#", "Load#", "Store#", "Copy#", "Fill#",
            "Print#", "GetEnv#",
            "Concat#", "Length#", "ToInt#", "ToFloat#", "ToString#",
            "Get#", "Insert#",
            "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
            "GetGroupId#", "GetNumGroups#", "Dims#",
            "AddressOf#", "SysCall#", "SysConf#",
            "AtomicLoad#", "AtomicStore#", "AtomicCas#", "AtomicXchg#", "AtomicAdd#", "Fence#",
            "DlOpen#", "DlSym#", "DlClose#",
            "Backtrace#",
        ];
        for name in &intrinsics {
            let sig = get_intrinsic_signature(name);
            assert!(sig.is_some(), "missing signature for {}", name);
        }
    }

    #[test]
    fn test_unknown_intrinsic_returns_none() {
        assert!(get_intrinsic_signature("NonExistent#").is_none());
        assert!(get_intrinsic_signature("AddI64#").is_none());
        assert!(get_intrinsic_signature("").is_none());
        assert!(get_intrinsic_signature("user_function").is_none());
    }

    #[test]
    fn test_observable_intrinsics() {
        assert!(get_intrinsic_signature("Print#").unwrap().observable);
        assert!(get_intrinsic_signature("Malloc#").unwrap().observable);
        assert!(get_intrinsic_signature("Free#").unwrap().observable);
        assert!(get_intrinsic_signature("Load#").unwrap().observable);
        assert!(get_intrinsic_signature("Copy#").unwrap().observable);
        assert!(get_intrinsic_signature("GetEnv#").unwrap().observable);
        assert!(get_intrinsic_signature("Insert#").unwrap().observable);
        assert!(!get_intrinsic_signature("Add#").unwrap().observable);
        assert!(!get_intrinsic_signature("Eq#").unwrap().observable);
        assert!(get_intrinsic_signature("AtomicStore#").unwrap().observable);
        assert!(get_intrinsic_signature("DlOpen#").unwrap().observable);
        assert!(get_intrinsic_signature("Backtrace#").unwrap().observable);
    }
}
