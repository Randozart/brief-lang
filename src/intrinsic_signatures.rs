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
    /// If true, this intrinsic accepts additional arguments beyond declared
    /// parameters (e.g., SysCall# takes num + up to 6 more). Default false.
    /// 2026-07-26: Added for variadic intrinsics to avoid false arity errors.
    pub variadic: bool,
}

/// Look up the signature of a # intrinsic by name.
pub fn get_intrinsic_signature(name: &str) -> Option<Signature> {
    match name {
        // ── Arithmetic (return matches input type) ────────────────────
        "Add#" => Some(Signature { name: "Add#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Sub#" => Some(Signature { name: "Sub#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Mul#" => Some(Signature { name: "Mul#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Div#" => Some(Signature { name: "Div#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Rem#" => Some(Signature { name: "Rem#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Neg#" => Some(Signature { name: "Neg#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Abs#" => Some(Signature { name: "Abs#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),

        // ── Comparison (returns Bool, but type-inferred) ──────────────
        "Eq#"  => Some(Signature { name: "Eq#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Neq#" => Some(Signature { name: "Neq#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Lt#"  => Some(Signature { name: "Lt#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Gt#"  => Some(Signature { name: "Gt#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Le#"  => Some(Signature { name: "Le#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Ge#"  => Some(Signature { name: "Ge#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),

        // ── Bitwise (return matches input type) ─────────────────────────
        "BitAnd#" => Some(Signature { name: "BitAnd#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "BitOr#" => Some(Signature { name: "BitOr#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "BitXor#" => Some(Signature { name: "BitXor#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Shl#" => Some(Signature { name: "Shl#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Shr#" => Some(Signature { name: "Shr#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "BitNot#" => Some(Signature { name: "BitNot#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        // ── Logical (unary, no short-circuit) ────────────────────────────
        "Not#" => Some(Signature { name: "Not#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        // ── Pointer operations ─────────────────────────────────────────
        "Deref#" => Some(Signature { name: "Deref#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Index#" => Some(Signature { name: "Index#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Ptr#" => Some(Signature { name: "Ptr#", parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),

        // ── Float math (returns native Float) ─────────────────────────
        "Sqrt#"  => Some(Signature { name: "Sqrt#",  parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "Sin#"   => Some(Signature { name: "Sin#",   parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "Cos#"   => Some(Signature { name: "Cos#",   parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "Fabs#"  => Some(Signature { name: "Fabs#",  parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "Ceil#"  => Some(Signature { name: "Ceil#",  parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "Floor#" => Some(Signature { name: "Floor#", parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "Pow#"   => Some(Signature { name: "Pow#",   parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),

        // ── Runtime (observable) ────────────────────────────────────
        // 2026-07-19: GetEnv#/GetEnvInt# moved to stdlib env.bv via ! plugin.
        // 2026-07-28: one generic `Print#` (2026-08-01 audit) — the backend
        // dispatches the emission by the argument's protocol category
        // (#String → __print_str, #Float → __print_float, else __print_int).
        // Empty parameters = type-inferred (any arg); observable prevents DCE.
        // PrintChar# remains the INTERNAL newline/char primitive (there is no
        // distinct Char type — a char is an Int code point, so it cannot be
        // type-dispatched; the print plugin's newline uses it).
        "Print#"     => Some(Signature { name: "Print#",     parameters: vec![], return_kind: ReturnKind::Exact(Type::int()), observable: true, variadic: false }),

        // ── Memory (observable) ─────────────────────────────────────
        "Malloc#"  => Some(Signature { name: "Malloc#",  parameters: vec![("size", Type::int())], return_kind: ReturnKind::Exact(Type::ptr(Type::bits(1))), observable: true, variadic: false }),
        // 2026-07-18: Variadic — first arg is size (Int), optional second is
        // strategy (Identifier or Quoted). Codegen handles both cases.
        "Alloc#"   => Some(Signature { name: "Alloc#",   parameters: vec![], return_kind: ReturnKind::Exact(Type::ptr(Type::bits(1))), observable: true, variadic: false }),
        // 2026-08-01 (D2): `Now#` — monotonic clock in nanoseconds, for the
        // watchdog `within N ms` deadline compare.
        "Now#"     => Some(Signature { name: "Now#",   parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: true, variadic: false }),
        "Free#"    => Some(Signature { name: "Free#",    parameters: vec![("ptr", Type::ptr(Type::bits(1)))], return_kind: ReturnKind::Exact(Type::void()), observable: true, variadic: false }),
        "Load#"    => Some(Signature { name: "Load#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: true, variadic: false }),
        "Store#"   => Some(Signature { name: "Store#",   parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true, variadic: false }),
        "Copy#"    => Some(Signature { name: "Copy#",    parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true, variadic: false }),
        "Fill#"    => Some(Signature { name: "Fill#",    parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true, variadic: false }),

        // ── String ───────────────────────────────────────────────────
        "Concat#"    => Some(Signature { name: "Concat#",    parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Length#"    => Some(Signature { name: "Length#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "ToInt#"     => Some(Signature { name: "ToInt#",     parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "ToFloat#"   => Some(Signature { name: "ToFloat#",   parameters: vec![], return_kind: ReturnKind::Native("Float"), observable: false, variadic: false }),
        "ToString#"  => Some(Signature { name: "ToString#",  parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),

        // ── Collection ───────────────────────────────────────────────
        "Get#"    => Some(Signature { name: "Get#",    parameters: vec![], return_kind: ReturnKind::Inferred, observable: false, variadic: false }),
        "Insert#" => Some(Signature { name: "Insert#", parameters: vec![], return_kind: ReturnKind::Exact(Type::void()), observable: true, variadic: false }),

        // ── GPU ───────────────────────────────────────────────────────
        "GetGlobalId#"   => Some(Signature { name: "GetGlobalId#",   parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "GetGlobalSize#" => Some(Signature { name: "GetGlobalSize#", parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "GetLocalId#"    => Some(Signature { name: "GetLocalId#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "WorkgroupSize#" => Some(Signature { name: "WorkgroupSize#", parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        // 2026-07-15: Additional GPU intrinsics
        "GetGroupId#"    => Some(Signature { name: "GetGroupId#",    parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "GetNumGroups#"  => Some(Signature { name: "GetNumGroups#",  parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),
        "Dims#"          => Some(Signature { name: "Dims#",          parameters: vec![], return_kind: ReturnKind::Native("Int"), observable: false, variadic: false }),

        // ── Pointers (compile-time address resolution) ──────────────
        "AddressOf#" => Some(Signature {
            name: "AddressOf#",
            parameters: vec![("id", Type::string())],
            return_kind: ReturnKind::Exact(Type::ptr(Type::bits(8))),
            observable: false,
            variadic: false,
        }),

        // ── Callbacks ────────────────────────────────────────────────
        // 2026-08-03: call a function-pointer value: CallPtr#(cb, args...).
        // `cb` is a fn(...) value (opaque pointer across the FFI boundary).
        // Fully variadic — the typechecker infers the return from cb's fn
        // type (see ReturnKind::Inferred special-case).
        "CallPtr#" => Some(Signature {
            name: "CallPtr#",
            parameters: vec![],
            return_kind: ReturnKind::Inferred,
            observable: false,
            variadic: true,
        }),

        // ── OS SysCall (observable, variadic) ──────────────────────────
        // 2026-07-15: Returns native Int (result or errno).
        // 2026-07-26: variadic — first arg is syscall number, up to 6 more args.
        "SysCall#" => Some(Signature {
            name: "SysCall#",
            parameters: vec![("num", Type::int())],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: true,
        }),

        // ── POSIX SysConf (observable) ─────────────────────────────────
        // 2026-07-15: Returns native Int (page size, CPU count, etc.).
        "SysConf#" => Some(Signature {
            name: "SysConf#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
        }),

        // ── Atomic operations (LLVM atomicrmw / load atomic / fence) ──
        // 2026-07-15: All return native Int except Fence# (void).
        "AtomicLoad#" => Some(Signature {
            name: "AtomicLoad#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: false,
            variadic: false,
        }),
        "AtomicStore#" => Some(Signature {
            name: "AtomicStore#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
        }),
        "AtomicCas#" => Some(Signature {
            name: "AtomicCas#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
        }),
        "AtomicXchg#" => Some(Signature {
            name: "AtomicXchg#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
        }),
        "AtomicAdd#" => Some(Signature {
            name: "AtomicAdd#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
        }),
        "Fence#" => Some(Signature {
            name: "Fence#",
            parameters: vec![],
            return_kind: ReturnKind::Exact(Type::void()),
            observable: true,
            variadic: false,
        }),

        // ── Dynamic linker (platform library functions) ──────────────
        // 2026-07-15: Return pointers (DlOpen#, DlSym#) or Int (DlClose#).
        "DlOpen#" => Some(Signature {
            name: "DlOpen#",
            parameters: vec![],
            return_kind: ReturnKind::Exact(Type::ptr(Type::bits(8))),
            observable: true,
            variadic: false,
        }),
        "DlSym#" => Some(Signature {
            name: "DlSym#",
            parameters: vec![],
            return_kind: ReturnKind::Exact(Type::ptr(Type::bits(8))),
            observable: false,
            variadic: false,
        }),
        "DlClose#" => Some(Signature {
            name: "DlClose#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
        }),

        // ── String operations (runtime + compile-time) ──────────────
        // 2026-07-25: Migrated from $ intrinsics — usable at runtime.
        "StrSplit#" => Some(Signature {
            name: "StrSplit#",
            parameters: vec![("s", Type::string()), ("pat", Type::string())],
            return_kind: ReturnKind::Inferred,
            observable: false,
            variadic: false,
        }),
        "EnvGet#" => Some(Signature {
            name: "EnvGet#",
            parameters: vec![("name", Type::string())],
            return_kind: ReturnKind::Inferred,
            observable: false,
            variadic: false,
        }),

        // ── System queries (observable — reads host state) ──────────
        "SysQuery#" => Some(Signature {
            name: "SysQuery#",
            parameters: vec![("query", Type::string())],
            return_kind: ReturnKind::Inferred,
            observable: false,
            variadic: false,
        }),
        "TimeNow#" => Some(Signature {
            name: "TimeNow#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: false,
            variadic: false,
        }),

        // ── External I/O (observable — side effects) ────────────────
        "HttpFetch#" => Some(Signature {
            name: "HttpFetch#",
            parameters: vec![("url", Type::string())],
            return_kind: ReturnKind::Inferred,
            observable: true,
            variadic: false,
        }),
        "ShellCmd#" => Some(Signature {
            name: "ShellCmd#",
            parameters: vec![("cmd", Type::string())],
            return_kind: ReturnKind::Inferred,
            observable: true,
            variadic: false,
        }),

        // ── Debugging (stack trace) ──────────────────────────────────
        // 2026-07-15: Returns frame count (native Int).
        "Backtrace#" => Some(Signature {
            name: "Backtrace#",
            parameters: vec![],
            return_kind: ReturnKind::Native("Int"),
            observable: true,
            variadic: false,
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
            "Malloc#", "Alloc#", "Free#", "Load#", "Store#", "Copy#", "Fill#",
            "Concat#", "Length#", "ToInt#", "ToFloat#", "ToString#",
            "Get#", "Insert#",
            "GetGlobalId#", "GetGlobalSize#", "GetLocalId#", "WorkgroupSize#",
            "GetGroupId#", "GetNumGroups#", "Dims#",
            "AddressOf#", "SysCall#", "SysConf#",
            "AtomicLoad#", "AtomicStore#", "AtomicCas#", "AtomicXchg#", "AtomicAdd#", "Fence#",
            "DlOpen#", "DlSym#", "DlClose#",
            "Backtrace#",
            "StrSplit#", "EnvGet#", "SysQuery#", "TimeNow#", "HttpFetch#", "ShellCmd#",
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
        assert!(get_intrinsic_signature("Malloc#").unwrap().observable);
        assert!(get_intrinsic_signature("Free#").unwrap().observable);
        assert!(get_intrinsic_signature("Load#").unwrap().observable);
        assert!(get_intrinsic_signature("Copy#").unwrap().observable);
        assert!(get_intrinsic_signature("Insert#").unwrap().observable);
        assert!(!get_intrinsic_signature("Add#").unwrap().observable);
        assert!(!get_intrinsic_signature("Eq#").unwrap().observable);
        assert!(get_intrinsic_signature("AtomicStore#").unwrap().observable);
        assert!(get_intrinsic_signature("DlOpen#").unwrap().observable);
        assert!(get_intrinsic_signature("Backtrace#").unwrap().observable);
    }

    #[test]
    fn test_address_of_signature() {
        let sig = get_intrinsic_signature("AddressOf#").unwrap();
        assert_eq!(sig.name, "AddressOf#");
        assert_eq!(sig.parameters.len(), 1);
        assert_eq!(sig.parameters[0].0, "id");
        assert!(!sig.observable);
        assert!(matches!(sig.return_kind, ReturnKind::Exact(_)));
    }
}
