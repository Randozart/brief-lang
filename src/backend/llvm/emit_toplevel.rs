use crate::ast::{BinaryOpKind, Expr, OutputType, Statement, TopLevel, Type};
use crate::backend::llvm::emit_stmt::emit_statement;
use crate::backend::llvm::{float_to_llvm_hex, float64_to_llvm_hex, LlvmBackend, TypedRegister};
use crate::type_universe::{ResolvedType, TypeUniverse};
use std::collections::HashSet;
use std::fmt::Write;
use std::sync::LazyLock;

impl LlvmBackend {
    /// Check if any modifier has the given name and extract its export name.
    /// Returns Some(export_name) if #export or #export("name") was found.
    /// 2026-07-14: string_value() removed from Annotation. Extract string from tag.value instead.
    pub fn get_export_name(modifiers: &[crate::ast::Annotation]) -> Option<String> {
        for tag in modifiers {
            if tag.name == "export" {
                let export_name = tag.value.as_ref().and_then(|v| {
                    if let Expr::Quoted(bytes) = v {
                        Some(String::from_utf8_lossy(bytes).to_string())
                    } else {
                        None
                    }
                }).unwrap_or_else(|| tag.name.clone());
                if export_name == "export" {
                    return None;
                }
                return Some(export_name);
            }
        }
        None
    }

    /// Emit cleanup calls for all local variables whose type has an
    /// OnExit foreign destructor. Called at scope exit points.
    fn emit_on_exit_cleanup(&mut self, out: &mut String, indent: &str) {
        let Some(ref universe) = self.ctx.type_universe else { return };
        for (name, ty) in &self.fun.let_binding_types {
            let type_name = match ty {
                crate::ast::Type::Custom(n) => n,
                crate::ast::Type::Applied(n, _) => n,
                _ => continue,
            };
            let Some(resolved) = universe.types.get(type_name) else { continue };
            // 2026-07-14: on_exit removed from ResolvedType, check properties["on_exit"] instead.
            let on_exit_fn = resolved.properties.get("on_exit").and_then(|pv| {
                if let crate::ast::PropertyValue::Identifier(s) = pv { Some(s.clone()) } else { None }
            });
            let Some(ref on_exit_fn) = on_exit_fn else { continue };
            let Some(reg) = self.fun.let_bindings.get(name) else { continue };
            // Emit: call void @on_exit_fn(i64 %reg)
            writeln!(out, "{}{} = call i64 @{}(i64 {})",
                indent,
                format!("%pcl{}", self.fun.txn_counter),
                on_exit_fn,
                reg
            ).ok();
            self.fun.txn_counter += 1;
        }
    }

    /// Check the target expression for an InsertAt strategy by looking up
    /// the variable's type in the TypeUniverse.
    fn lookup_strategy_type_name(&self, var_name: &str) -> Option<String> {
        // 2026-07-01: First check let_original_types (populated for function params).
        // If not found, fall back to ctx.field_brief_types (populated for state vars).
        // State variables like `queue: RingBuffer<Int>` are NOT in let_original_types
        // (only function params go there). Without this fallback, strategy dispatch
        // returns None and custom types like RingBuffer fall through to the default
        // List arena path, causing realloc on non-heap memory.
        if let Some(ty) = self.fun.let_original_types.get(var_name) {
            return match ty {
                crate::ast::Type::Custom(n) => Some(n.clone()),
                crate::ast::Type::Applied(n, _) => Some(n.clone()),
                _ => None,
            };
        }
        if let Some(&idx) = self.ctx.field_index_map.get(var_name) {
            let ty = self.ctx.field_brief_types.get(idx)?;
            return match ty {
                crate::ast::Type::Custom(n) => Some(n.clone()),
                crate::ast::Type::Applied(n, _) => Some(n.clone()),
                _ => None,
            };
        }
        None
    }

    /// 2026-07-20: Find an OperatorDef for InsertAt by looking up the
    /// variable's type in the operator_defs map (populated from AST).
    /// Returns None when the type has no InsertAt operator definition.
    pub(super) fn find_insert_strategy(&self, target: &crate::ast::Expr) -> Option<&crate::ast::top::OperatorDef> {
        let var_name = match target {
            crate::ast::Expr::Identifier(n) => n,
            _ => target.as_var_name()?,
        };
        let type_name = self.lookup_strategy_type_name(var_name)?;
        self.ctx.operator_defs.get(&type_name)?
            .iter().find(|d| d.op == "InsertAt")
    }

    /// 2026-07-20: Find an OperatorDef for ExtractFrom by looking up the
    /// variable's type in the operator_defs map (populated from AST).
    /// Returns None when the type has no ExtractFrom operator definition.
    pub(super) fn find_extract_strategy(&self, target: &crate::ast::Expr) -> Option<&crate::ast::top::OperatorDef> {
        let var_name = match target {
            crate::ast::Expr::Identifier(n) => n,
            _ => target.as_var_name()?,
        };
        let type_name = self.lookup_strategy_type_name(var_name)?;
        self.ctx.operator_defs.get(&type_name)?
            .iter().find(|d| d.op == "ExtractFrom")
    }

    pub(super) fn emit_header(&self, out: &mut String) {
        writeln!(out, "; ModuleID = 'program.ll'").ok();
        writeln!(out, "source_filename = \"program.bv\"").ok();
        // 2026-07-11: Phase 6 — target triple and data layout are now
        // configurable via CompilerContext.target_triple / .data_layout.
        if let Some(ref dl) = self.ctx.data_layout {
            writeln!(out, "target datalayout = \"{}\"", dl).ok();
        }
        writeln!(out, "target triple = \"{}\"", self.ctx.target_triple).ok();
    }

    /// Emit LLVM struct type declarations for user-defined struct types.
    /// Each struct becomes a named LLVM type so foreign callers (Rust via LTO,
    /// Python via ctypes) can match the memory layout. All fields are boxed
    /// as i64 (Brief's universal scalar storage type).
    ///
    /// Called after `emit_header()` and before `emit_declares()` so that
    /// struct types are available for use in function signatures emitted
    /// later. Structs with no fields are emitted as `{}` (empty type).
    ///
    /// 2026-07-10: Phase 1 — zero-copy GLUE bridge struct type declarations.
    pub(super) fn declare_struct_types(&self, out: &mut String) {
        writeln!(out).ok();
        // 2026-07-30: Emit known struct types from the universe (String, SmallString64,
        // UTF8View, Slice, etc.) BEFORE user-defined struct types. These affect LLVM's
        // struct layout and alignment computation — without them, LICM/sinkRegion in
        // clang 18.1.3 crashes on loops that reference these types through FFI calls.
        // Also hardcode common stdlib types that may not have universe entries if the
        // importing benchmark doesn't reference them directly.
        //
        // Hardcoded stdlib struct types (always emitted for ABI consistency):
        // 2026-08-01: `%String` removed — under the bits model a String value is a
        // `ptr` to `[len][bytes]`, so a named `{ i64, i64 }` decl would be a false
        // ABI claim (B0 acceptance: no `{ i64, i64 }` for String in emitted IR).
        // Nothing references it anymore. StaticString/UTF8View/SmallString64 remain
        // until their legacy retirement (Phase B4).
        for (name, field_tys) in &[
            ("SmallString64", "i64, i64, i64, i64, i64, i64, i64, i64, i64"),
            ("StaticString", "i64, i64"),
            ("UTF8View", "i64, i64"),
        ] {
            writeln!(out, "%{} = type {{ {} }}", name, field_tys).ok();
        }
        // Universe-registered struct types (from type declarations with slots):
        // 2026-07-30: Skip types already hardcoded above — prevents duplicate
        // %String = type { i64, i64 } declarations that clang rejects.
        let mut emitted: std::collections::HashSet<String> = [
            "SmallString64", "StaticString", "UTF8View",
        ].iter().map(|s| s.to_string()).collect();
        if let Some(u) = &self.ctx.type_universe {
            let mut universe_fields: Vec<(String, Vec<String>)> = Vec::new();
            for rt in u.types.values() {
                if rt.fields.is_empty() { continue; }
                if emitted.contains(&rt.name) { continue; }
                let field_tys: Vec<String> = rt.fields.iter()
                    .map(|(_, fty)| self.llvm_type(fty))
                    .collect();
                universe_fields.push((rt.name.clone(), field_tys));
            }
            universe_fields.sort_by_key(|(k, _)| k.clone());
            for (name, field_tys) in &universe_fields {
                writeln!(out, "%{} = type {{ {} }}", name, field_tys.join(", ")).ok();
                emitted.insert(name.clone());
            }
        }
        if self.ctx.struct_types.is_empty() {
            return;
        }
        // Iteration order MUST be sorted by key for deterministic IR emission.
        let mut sorted: Vec<(String, Vec<(String, Type)>)> = self.ctx.struct_types.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        sorted.sort_by_key(|(k, _)| k.clone());
        for (name, fields) in &sorted {
            // 2026-07-31: skip if the universe already declared this type.
            if emitted.contains(name.as_str()) { continue; }
            if fields.is_empty() {
                writeln!(out, "%{} = type {{}}", name).ok();
            } else {
                let field_tys: Vec<&str> = fields.iter().map(|_| "i64").collect();
                writeln!(out, "%{} = type {{ {} }}", name, field_tys.join(", ")).ok();
            }
        }
    }

    pub(super) fn emit_declares(&self, out: &mut String) {
        writeln!(out).ok();
        writeln!(out, "declare void @llvm.assume(i1) #1").ok();
        writeln!(out, "declare void @llvm.trap() noreturn").ok();
        // Intrinsic declares used by name#() instrinsic calls in emit_expr.
        // Previously these came from std/llvm.bv via as intrinsic, but the
        // name#() mechanism emits them directly without frgn_map entries.
        writeln!(out, "declare float @llvm.sqrt.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.fabs.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.ceil.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.floor.f32(float) #1").ok();
        // 2026-06-29: Double-precision (Float64) intrinsic variants
        writeln!(out, "declare double @llvm.sqrt.f64(double) #1").ok();
        writeln!(out, "declare double @llvm.fabs.f64(double) #1").ok();
        writeln!(out, "declare double @llvm.ceil.f64(double) #1").ok();
        writeln!(out, "declare double @llvm.floor.f64(double) #1").ok();
        writeln!(out, "declare i64 @llvm.ctpop.i64(i64) #1").ok();
        writeln!(out, "declare i64 @llvm.ctlz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.cttz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.abs.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.bitreverse.i64(i64) #1").ok();
        // Runtime support functions
        writeln!(out, "declare void @__barrier_release__()").ok();
        writeln!(out, "declare void @__barrier_wait__()").ok();
        writeln!(out, "declare void @__thread_pool_init__(i32, ptr)").ok();
        // 2026-07-01: Stores the current state snapshot pointer for worker threads.
        // Called by main before __barrier_release__ so async body functions receive
        // the correct state argument instead of a garbage pointer.
        writeln!(out, "declare void @__set_async_state__(ptr)").ok();
        writeln!(out, "declare i64 @time(ptr) nounwind").ok();
        // 2026-07-28: atol and getenv used by GetEnvInt# intrinsic.
        writeln!(out, "declare i64 @atol(ptr) nounwind").ok();
        writeln!(out, "declare ptr @getenv(ptr) nounwind").ok();
        writeln!(out, "declare noalias ptr @malloc(i64) nounwind").ok();
        writeln!(out, "declare void @free(ptr) nounwind").ok();
        // 2026-06-26: realloc used by the arena allocator grow path when
        // the bump-allocated buffer is exhausted (emit_arena_alloc in mod.rs).
        writeln!(out, "declare ptr @realloc(ptr, i64) nounwind").ok();
        writeln!(out, "declare i64 @ShellCmd(i64)").ok();
        // 2026-07-26: ~50 dead declares removed — no Rust code path generated
        // calls to them. Only ShellCmd is kept (called via ShellCmd# intrinsic).
        // 2026-07-15: Raw OS syscall (SysCall# intrinsic)
        writeln!(out, "declare i64 @brief_syscall(i64, i64, i64, i64, i64, i64, i64)").ok();
        // 2026-07-15: Runtime system configuration (SysConf# intrinsic)
        writeln!(out, "declare i64 @brief_sysconf(i64)").ok();
        // 2026-07-15: Dynamic linker (DlOpen#/DlSym#/DlClose# intrinsics)
        writeln!(out, "declare ptr @dlopen(ptr, i32) nounwind").ok();
        writeln!(out, "declare ptr @dlsym(ptr, ptr) nounwind").ok();
        writeln!(out, "declare i32 @dlclose(ptr) nounwind").ok();
        // 2026-07-15: Stack backtrace (Backtrace# intrinsic)
        writeln!(out, "declare i64 @brief_backtrace()").ok();
        // 2026-07-15: POSIX socket/ioctl declarations removed — they conflict
        // with the defn wrappers in std/os/ (which now use SysCall# internally).
    }

    /// 2026-07-08: Fallback for types not in the universe (test code,
    /// custom types before registration).
    /// 2026-07-19: Fixed Float→"float" vs Float64→"double" mapping.
    /// The normalizer handles all properly registered types via category
    /// inference + config lookup; this is a last-resort fallback.
    fn fallback_llvm_type(ty: &Type) -> &'static str {
        match ty {
            Type::Ptr(_) => "ptr",
            Type::Void => "void",
            Type::Bits(bytes) => match bytes * 8 {
                1 => "i1",
                8 => "i8",
                16 => "i16",
                32 => "i32",
                64 => "i64",
                _ => "i64",
            },
            _ => "i64",
        }
    }

    pub(super) fn llvm_type(&self, ty: &Type) -> String {
        // 2026-07-15: Ptr<T> always maps to LLVM opaque pointer type.
        // Must be checked BEFORE the universe lookup because Ptr<T> has no
        // primitive property set (it's a compiler construct, not from bootstrap.bv).
        if matches!(ty, Type::Ptr(_)) {
            return "ptr".to_string();
        }
        // 2026-07-18: UTF8View/Slice always use {ptr, i64} or {i64, i64} (fat pointer),
        // regardless of SSO. Must be checked BEFORE the general struct_types check
        // because they are also registered as struct types but should be passed by value.
        // 2026-07-30: Slice<T> replaces UTF8View. UTF8View kept for backward compat.
        if let Type::Custom(name) = ty {
            if name == "UTF8View" {
                return "{ i64, i64 }".to_string();
            }
            if name == "Slice" {
                return "{ ptr, i64 }".to_string();
            }
        }
        // 2026-07-10: Phase 1 — check for user-defined struct types first.
        // Struct types are passed by pointer at the FFI boundary, so return
        // "ptr" (LLVM opaque pointer). The named struct type is declared in
        // `declare_struct_types()` for the foreign caller's reference.
        if let Type::Custom(name) = ty {
            if self.ctx.struct_types.contains_key(name) {
                return "ptr".to_string();
            }
        }
        // 2026-07-18: SVO List — return multi-slot struct type for vector-like types.
        if self.feature_svo {
            if self.ctx.type_universe.as_ref().map_or(false, |u| u.is_vector_like(ty)) {
                let cap = self.ctx.type_universe.as_ref()
                    .map(|u| u.svo_capacity(ty)).unwrap_or(0);
                if cap > 0 {
                    let slots = cap + 1; // N data slots + 1 len+cap slot
                    return format!("{{ {} }}", std::iter::repeat("i64").take(slots)
                        .collect::<Vec<_>>().join(", "));
                }
            }
        }
        if self.feature_sso_strings {
            // 2026-07-31: Phase 3 (§8.4-D7) — String detection via protocol
            // membership (#String) instead of the type name.
            if self.is_protocol_member(ty, "#String") {
                return "{ i64, i64 }".to_string();
            }
            if self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty)) {
                return "{ i64, i64 }".to_string();
            }
        }
        // 2026-07-22: Non-SSO strings use ptr (opaque pointer), even if the
        // universe declares them as {i64, i64}. The SSO code path converts
        // between {i64, i64} and ptr internally; without SSO, the parameter
        // is treated as a raw pointer.
        if !self.feature_sso_strings {
            // 2026-07-31: Phase 3 (§8.4-D7) — #String/#Data membership instead
            // of the type-name match.
            if self.is_protocol_member(ty, "#String") || self.is_protocol_member(ty, "#Data") {
                return "ptr".to_string();
            }
        }
        // 2026-07-30: Struct-like types derive LLVM type from field shapes.
        // This handles Slice<T> (fields: { Ptr<T>, Int } → { ptr, i64 }),
        // List<T>, and any future struct type without requiring llvm_type
        // metadata or primordial entries. Must come AFTER the SSO/SVO/String
        // special cases but BEFORE the casting graph resolution.
        if let Some(rt) = self.ctx.type_universe.as_ref()
            .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
        {
            if !rt.fields.is_empty() {
                let field_tys: Vec<String> = rt.fields.iter()
                    .map(|(_, fty)| self.llvm_type(fty))
                    .collect();
                return format!("{{ {} }}", field_tys.join(", "));
            }
        }
        // 2026-07-30: Casting graph resolves LLVM type from (protocol, metadata).
        // This replaces the old universe llvm_type property lookup.
        // Types without protocol membership fall through to fallback_llvm_type.
        if let Some(graph) = self.ctx.casting_graph.as_ref() {
            if let Some(universe) = self.ctx.type_universe.as_ref() {
                return graph.resolve_llvm_type(universe, ty, self.ctx.int_bits);
            }
        }
        Self::fallback_llvm_type(ty).to_string()
    }

    /// Is this a boxed primordial type — the set marshaled to i64 for %State
    /// storage? Bool/Char/String/Data are always boxed; the 32-bit Float is
    /// boxed via bitcast(float→i32→i64).
    ///
    /// 2026-07-31: Phase 3 (§8.4-D1) — protocol membership (is_protocol_member)
    /// replaces the box_op hardcoded type-name fallback. Float64 (#Float with
    /// bytes 8) is deliberately EXCLUDED: the legacy box_op only boxed the
    /// 32-bit Float, and Float64 params pass through as native double.
    pub(super) fn is_boxed_type(&self, ty: &Type) -> bool {
        if self.is_protocol_member(ty, "#Bool")
            || self.is_protocol_member(ty, "#Char")
            || self.is_protocol_member(ty, "#String")
            || self.is_protocol_member(ty, "#Data")
        {
            return true;
        }
        self.is_protocol_member(ty, "#Float")
            && self.ctx.type_universe.as_ref()
                .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
                .map_or(false, |rt| rt.bytes <= 4)
    }

    /// Is this a boxed NON-float type (Bool/Char/String/Data)? These become
    /// `Type::int()` in let_binding_types because their %State slot is i64.
    /// Float is NOT included — float stays `Type::float()` and is handled by
    /// ensure_float_reg via the reg_float_cache.
    ///
    /// 2026-07-31: Phase 3 (§8.4-D1) — protocol membership instead of the
    /// hardcoded name set.
    pub(super) fn is_boxed_int_type(&self, ty: &Type) -> bool {
        self.is_protocol_member(ty, "#Bool")
            || self.is_protocol_member(ty, "#Char")
            || self.is_protocol_member(ty, "#String")
            || self.is_protocol_member(ty, "#Data")
    }

    /// Box a value of a boxed type to its i64 representation.
    ///
    /// 2026-07-31: Phase 3 (§8.4-D1) — the per-protocol conversion replaces
    /// the box_op name match: Bool → zext i8, Char → zext i32, String/Data →
    /// ptrtoint, Float → bitcast(float→i32) + zext. `float_tmp` is a fresh
    /// register name for the Float bitcast.
    pub(super) fn emit_box_value_to_i64(
        &mut self,
        out: &mut String,
        indent: &str,
        ty: &Type,
        raw: &str,
        conv: &str,
        float_tmp: &str,
    ) {
        if self.is_protocol_member(ty, "#Bool") {
            writeln!(out, "{}{} = zext i8 {} to i64", indent, conv, raw).ok();
        } else if self.is_protocol_member(ty, "#Char") {
            writeln!(out, "{}{} = zext i32 {} to i64", indent, conv, raw).ok();
        } else if self.is_protocol_member(ty, "#String") || self.is_protocol_member(ty, "#Data") {
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, conv, raw).ok();
        } else if self.is_protocol_member(ty, "#Float") {
            writeln!(out, "{}{} = bitcast float {} to i32", indent, float_tmp, raw).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, conv, float_tmp).ok();
        }
    }


    pub(super) fn ensure_float_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        // Check cache first — even float-typed registers may have their
        // native float counterpart cached (e.g. parameter marshaling boxes
        // float to i64 at function entry; the cache maps boxed→native).
        if let Some(cached) = self.fun.reg_float_cache.get(&reg.name) {
            return cached.clone();
        }
        // 2026-07-17: If the register is Float (32-bit) but the caller expects
        // double (e.g. Print# passes to printf which variadic-promotes float),
        // emit an fpext to double. All brief floats are represented as float
        // (32-bit), but C variadic functions receive double (64-bit).
        if reg.ty == Type::float() {
            let dbl = self.fun.gen_reg();
            writeln!(out, "{}{} = fpext float {} to double", indent, dbl, reg.name).ok();
            self.fun.reg_float_cache.insert(reg.name.clone(), dbl.clone());
            return dbl;
        }
        // 2026-07-17: For float64 (double) and non-float types, return as-is.
        // The old code used a hardcoded is_native = true path that always
        // returned reg.name directly, bypassing native_float_or_box entirely.
        reg.name.clone()
    }

    /// Emit epoll-based initialization for built-in trigger sources.
    /// Creates an epoll fd, registers each built-in trigger's source fd,
    /// and stores the epfd in a synthetic state field.
    pub(super) fn emit_trg_init(&mut self, out: &mut String) {
        // Need at least one built-in trigger to emit setup
        let has_builtin = self.ctx.triggers.iter().any(|(_, trg)| matches!(
            trg.address,
            crate::ast::LinkRef::Stdin | crate::ast::LinkRef::Timer(_) | crate::ast::LinkRef::Signal(_)
        ));
        if !has_builtin { return; }

        // Constants
        let epoin = 0x01u32; // EPOLLIN
        let epolet = 0x80000000u32; // EPOLLET
        let epoloneshot = 0x40000000u32; // EPOLLONESHOT
        let f_setfl = 4; // F_SETFL
        let o_nonblock = 0x800u32; // O_NONBLOCK
        let clo_monotonic = 1; // CLOCK_MONOTONIC
        let tfd_nonblock = 0x800; // TFD_NONBLOCK = O_NONBLOCK = 2048 on x86_64
        let sfd_nonblock = 0x800; // SFD_NONBLOCK = O_NONBLOCK = 2048 on x86_64
        let sig_block = 0; // SIG_BLOCK

        // epoll_create1(0)
        let epfd = format!("%epfd");
        writeln!(out, "  {} = call i32 @epoll_create1(i32 0)", epfd).ok();

        // Store epfd in epfd_field slot
        let sge = format!("%sge{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        if let Some(epfd_idx) = self.ctx.field_index_map.get("__trg_epfd") {
            // 2026-07-20: Intentionally hand-rolled — stores i32 (not i64); centralized
            // emit_state_store_i64_by_idx only handles i64 stores.
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", sge, epfd_idx).ok();
            writeln!(out, "  store i32 {}, ptr {}, align 4", epfd, sge).ok();
        }

        // Per-trigger setup
        for (name, trg) in &self.ctx.triggers {
            let bit = self.ctx.dep_graph.bit_index.get(name).copied().unwrap_or(0);
            match &trg.address {
                crate::ast::LinkRef::Stdin => {
                    // fcntl(0, F_SETFL, O_NONBLOCK)
                    writeln!(out, "  %fcntl_{} = call i32 @fcntl(i32 0, i32 {}, i32 {})", name, f_setfl, o_nonblock).ok();
                    // epoll_event struct on stack: { events: EPOLLIN, data: { u64: bit } }
                    let ev_slot = format!("%ev_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 16, align 8", ev_slot).ok();
                    let ev_events = format!("%eve_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 0", ev_events, ev_slot).ok();
                    let ev_events_i32 = format!("%evei_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_events_i32, ev_events).ok();
                    writeln!(out, "  store i32 {}, ptr {}, align 4", epoin, ev_events_i32).ok();
                    let ev_data = format!("%evd_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 8", ev_data, ev_slot).ok();
                    let ev_data_u64 = format!("%evdu_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_data_u64, ev_data).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", bit, ev_data_u64).ok();
                    // epoll_ctl(epfd, EPOLL_CTL_ADD, 0, &ev)
                    let ctl = format!("%ectl_{}", name);
                    writeln!(out, "  {} = call i32 @epoll_ctl(i32 {}, i32 1, i32 0, ptr {})", ctl, epfd, ev_slot).ok();
                }
                crate::ast::LinkRef::Timer(hz) => {
                    // timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK)
                    let tfd = format!("%tfd_{}", name);
                    writeln!(out, "  {} = call i32 @timerfd_create(i32 {}, i32 {})", tfd, clo_monotonic, tfd_nonblock).ok();
                    // timerfd_settime(tfd, 0, &its, null) — fires at Hz
                    let its_slot = format!("%its_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 32, align 8", its_slot).ok();
                    // itimerspec.it_interval.tv_sec = 0
                    // itimerspec.it_interval.tv_nsec = 1_000_000_000 / hz
                    let interval_nsec = if *hz > 0 { 1_000_000_000u64 / *hz as u64 } else { 0 };
                    // itimerspec.it_value.tv_sec = 0
                    // itimerspec.it_value.tv_nsec = interval_nsec (fire immediately, then at interval)
                    let its_val_sec = format!("%its_vs_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 0", its_val_sec, its_slot).ok();
                    let its_val_sec_i64 = format!("%itsvsi_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", its_val_sec_i64, its_val_sec).ok();
                    writeln!(out, "  store i64 0, ptr {}, align 8", its_val_sec_i64).ok();
                    let its_val_nsec = format!("%its_vn_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 8", its_val_nsec, its_slot).ok();
                    let its_val_nsec_i64 = format!("%itsvni_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", its_val_nsec_i64, its_val_nsec).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", interval_nsec, its_val_nsec_i64).ok();
                    let its_int_sec = format!("%its_is_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 16", its_int_sec, its_slot).ok();
                    let its_int_sec_i64 = format!("%itsisi_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", its_int_sec_i64, its_int_sec).ok();
                    writeln!(out, "  store i64 0, ptr {}, align 8", its_int_sec_i64).ok();
                    let its_int_nsec = format!("%its_in_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 24", its_int_nsec, its_slot).ok();
                    let its_int_nsec_i64 = format!("%itsini_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", its_int_nsec_i64, its_int_nsec).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", interval_nsec, its_int_nsec_i64).ok();
                    writeln!(out, "  %tfd_settime_{} = call i32 @timerfd_settime(i32 {}, i32 0, ptr {}, ptr null)", name, tfd, its_slot).ok();
                    // epoll_ctl(epfd, EPOLL_CTL_ADD, tfd, &ev)
                    let ev_slot = format!("%ev_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 16, align 8", ev_slot).ok();
                    let ev_events = format!("%eve_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 0", ev_events, ev_slot).ok();
                    let ev_events_i32 = format!("%evei_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_events_i32, ev_events).ok();
                    writeln!(out, "  store i32 {}, ptr {}, align 4", epoin, ev_events_i32).ok();
                    let ev_data = format!("%evd_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 8", ev_data, ev_slot).ok();
                    let ev_data_u64 = format!("%evdu_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_data_u64, ev_data).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", bit, ev_data_u64).ok();
                    writeln!(out, "  %ectl_{} = call i32 @epoll_ctl(i32 {}, i32 1, i32 {}, ptr {})", name, epfd, tfd, ev_slot).ok();
                }
                crate::ast::LinkRef::Signal(sig) => {
                    // sigemptyset(&mask)
                    let mask_slot = format!("%mask_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 128, align 8", mask_slot).ok();
                    writeln!(out, "  %sigemptyset_{} = call i32 @sigemptyset(ptr {})", name, mask_slot).ok();
                    // sigaddset(&mask, SIG)
                    let sig_num = sig_number(sig);
                    writeln!(out, "  %sigadd_{} = call i32 @sigaddset(ptr {}, i32 {})", name, mask_slot, sig_num).ok();
                    // sigprocmask(SIG_BLOCK, &mask, null)
                    writeln!(out, "  %sigprocmask_{} = call i32 @sigprocmask(i32 {}, ptr {}, ptr null)", name, sig_block, mask_slot).ok();
                    // signalfd(-1, &mask, SFD_NONBLOCK)
                    let sfd = format!("%sfd_{}", name);
                    writeln!(out, "  {} = call i32 @signalfd(i32 -1, ptr {}, i32 {})", sfd, mask_slot, sfd_nonblock).ok();
                    // epoll_ctl(epfd, EPOLL_CTL_ADD, sfd, &ev)
                    let ev_slot = format!("%ev_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 16, align 8", ev_slot).ok();
                    let ev_events = format!("%eve_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 0", ev_events, ev_slot).ok();
                    let ev_events_i32 = format!("%evei_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_events_i32, ev_events).ok();
                    writeln!(out, "  store i32 {}, ptr {}, align 4", epoin, ev_events_i32).ok();
                    let ev_data = format!("%evd_{}", name);
                    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 8", ev_data, ev_slot).ok();
                    let ev_data_u64 = format!("%evdu_{}", name);
                    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_data_u64, ev_data).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", bit, ev_data_u64).ok();
                    writeln!(out, "  %ectl_{} = call i32 @epoll_ctl(i32 {}, i32 1, i32 {}, ptr {})", name, epfd, sfd, ev_slot).ok();
                }
                _ => {} // Explicit(addr), Linked(sym) — handled by emit_trg_load in step()
            }
        }
    }

    /// Load an external trigger value (MMIO address or C global).
    /// Built-in triggers (@stdin#, @timer#, @signal#) are stored to state
    /// by the event loop — load from the state field.
    pub(super) fn emit_trg_load(&mut self, out: &mut String, indent: &str, dst: &str, address: &crate::ast::LinkRef, trg_ty: &Type) {
        match address {
            crate::ast::LinkRef::Explicit(addr) => {
                let store_ty = super::trg_llvm_storage_ty(trg_ty, self.ctx.type_universe.as_ref());
                let tr_counter = self.fun.txn_counter;
                self.fun.txn_counter += 1;
                let raw = format!("%tr{}", tr_counter);
                writeln!(out, "{}{} = load volatile {}, {}* inttoptr (i64 {} to {}*), align 1", indent, raw, store_ty, store_ty, addr, store_ty).ok();
                self.emit_trg_load_finish(out, indent, dst, raw, trg_ty);
            }
            crate::ast::LinkRef::Linked(sym) => {
                let store_ty = super::trg_llvm_storage_ty(trg_ty, self.ctx.type_universe.as_ref());
                let tr_counter = self.fun.txn_counter;
                self.fun.txn_counter += 1;
                let raw = format!("%tr{}", tr_counter);
                writeln!(out, "{}{} = load volatile {}, {}* @{}", indent, raw, store_ty, store_ty, sym).ok();
                self.emit_trg_load_finish(out, indent, dst, raw, trg_ty);
            }
            // 2026-07-15: @ *ptr dynamic trigger — emit the pointer expression,
            // then load volatile from the resulting pointer value.
            // When --error-unresolved-trg is set, emit a null check before the
            // load that branches to unreachable if the pointer is null.
            crate::ast::LinkRef::Deref(ptr_expr) => {
                let store_ty = super::trg_llvm_storage_ty(trg_ty, self.ctx.type_universe.as_ref());
                let tr_counter = self.fun.txn_counter;
                self.fun.txn_counter += 1;
                let raw = format!("%tr{}", tr_counter);
                let ptr_reg = self.emit_expr(out, ptr_expr, indent);
                if self.trg_unresolved_action == crate::backend::llvm::TrgUnresolvedAction::Error {
                    let err_bb = format!("trg_err_{}", tr_counter);
                    let ok_bb = format!("trg_ok_{}", tr_counter);
                    let null_check = format!("%nullchk_{}", tr_counter);
                    writeln!(out, "{}{} = icmp eq ptr {}, null", indent, null_check, ptr_reg.name).ok();
                    writeln!(out, "  br i1 {}, label %{}, label %{}", null_check, err_bb, ok_bb).ok();
                    writeln!(out, "{}:", err_bb).ok();
                    writeln!(out, "  call void @llvm.trap()").ok();
                    writeln!(out, "  unreachable").ok();
                    writeln!(out, "{}:", ok_bb).ok();
                }
                writeln!(out, "{}{} = load volatile {}, ptr {}, align 1", indent, raw, store_ty, ptr_reg.name).ok();
                self.emit_trg_load_finish(out, indent, dst, raw, trg_ty);
            }
            _ => {
                // Stdin, Timer, Signal — event loop stores values to state.
                // Emit add i64 0, 0 as zero default; callers with field access
                // patterns will have already loaded from state via Identifier -> field_index_map path.
                writeln!(out, "{}{} = add i64 0, 0 ; built-in trigger (loaded by event loop)", indent, dst).ok();
            }
        }
    }

    /// Finish loading a trigger value after the raw load: convert to native LLVM type.
    ///
    /// 2026-07-31: Phase 3 (§8.4-D7) — protocol-membership dispatch replaces
    /// the hardcoded type-name match.
    pub(super) fn emit_trg_load_finish(&self, out: &mut String, indent: &str, dst: &str, raw: String, trg_ty: &Type) {
        if self.is_protocol_member(trg_ty, "#Bool") {
            writeln!(out, "{}{} = trunc i8 {} to i1", indent, dst, raw).ok();
        } else if self.is_protocol_member(trg_ty, "#Int") || self.is_protocol_member(trg_ty, "#UInt") {
            writeln!(out, "{}{} = add i64 0, {}", indent, dst, raw).ok();
        } else if self.is_protocol_member(trg_ty, "#Float") {
            writeln!(out, "{}{} = add float 0.0, {}", indent, dst, raw).ok();
        } else if self.is_protocol_member(trg_ty, "#Char") {
            writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, raw).ok();
        } else if self.is_protocol_member(trg_ty, "#String") || self.is_protocol_member(trg_ty, "#Data") {
            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, dst, raw).ok();
        } else {
            writeln!(out, "{}{} = add i64 0, {}", indent, dst, raw).ok();
        }
    }

    pub(super) fn align_of(&self, ty: &str) -> u32 {
        // 2026-07-30: For standard LLVM types, use direct alignment (below).
        // Only query the universe for non-standard types (struct types, typed enums),
        // matching by BOTH LLVM type string AND type name to avoid finding wrong
        // entries (e.g., a struct with alignment 2 whose LLVM type resolves to "float").
        if !matches!(ty, "i8" | "i16" | "i32" | "i64" | "i128" | "float" | "double" | "half" | "bfloat" | "ptr") {
            if let Some(u) = &self.ctx.type_universe {
                for rt in u.types.values() {
                    let rt_ll = self.llvm_type(&Type::Custom(rt.name.clone()));
                    if rt_ll == ty && rt.alignment > 0 {
                        return rt.alignment as u32;
                    }
                }
            }
        }
        // 2026-07-29: Derive alignment from LLVM bit width for integer types.
        // Covers i8/i16/i32/i64/i128 and any future integer width — bits / 8.
        if let Some(bits) = ty.strip_prefix('i').and_then(|s| s.parse::<u32>().ok()) {
            let a = bits / 8;
            if a > 0 { return a; }
        }
        // Named floating-point, pointer, and other types.
        match ty {
            "double" | "fp128" => 8,
            "float" => 4,
            "half" | "bfloat" => 2,
            "ptr" => 8,
            _ => 8,
        }
    }

    pub(super) fn declare_state_type(&mut self, out: &mut String) {
        // Emit %CellState.<name> types for persistent cells (used by thread functions)
        // 2026-07-19: Sorted for deterministic IR.
        let mut sorted_cell_types: Vec<&String> = self.ctx.cell_state_types.keys().collect();
        sorted_cell_types.sort();
        for cell_name in &sorted_cell_types {
            let (cs_imap, cs_tys) = &self.ctx.cell_state_types[*cell_name];
            write!(out, "%CellState.{} = type {{ ", cell_name).ok();
            for (i, f) in cs_tys.iter().enumerate() {
                if i > 0 { write!(out, ", ").ok(); }
                write!(out, "{}", f).ok();
            }
            writeln!(out, " }}").ok();
        }

        if self.ctx.field_types.is_empty() {
            writeln!(out, "%State = type {{ i64 }}").ok();
            return;
        }
        // 2026-07-04: Emit chunk struct definitions so SROA can decompose
        // each chunk into scalar registers.  Each chunk has ≤CHUNK fields
        // (15).  The monolithic %State is kept for backward compat with
        // non-routed paths (old EmitInlineSsa/b, @init_state).
        // 2026-07-31: Phase 3 (§8.2) — chunk cap from config/ir-lowering.toml.
        let chunk_size = crate::config_tuning::ir_lowering().max_fields_per_alloca;
        let total = self.ctx.field_types.len();
        let num_chunks = (total + chunk_size - 1) / chunk_size;
        for chunk in 0..num_chunks {
            let start = chunk * chunk_size;
            let end = std::cmp::min(start + chunk_size, total);
            write!(out, "%StateChunk{} = type {{ ", chunk).ok();
            for i in start..end {
                if i > start { write!(out, ", ").ok(); }
                write!(out, "{}", self.ctx.field_types[i]).ok();
            }
            writeln!(out, " }}").ok();
        }
        write!(out, "%State = type {{ ").ok();
        for (i, f) in self.ctx.field_types.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}", f).ok();
        }
        writeln!(out, " }}").ok();
    }

    //
    // WHY emit_init_state as a separate function AND emit_inline_init_stores:
    //   Two callers need init logic. The main reactor loop uses the inline path
    //   (emit_inline_init_stores) so SROA can scalarize %State. But library-mode
    //   and external-C callers (via __brief_init_state) need a callable function
    //   that returns an initialized ptr — those callers don't have an alloca
    //   to inline into, so they need @init_state as a named function. Both share
    //   the same store logic; the tradeoff is SROA opportunity (inline) vs callable
    //   interface (function).
    /// 2026-07-03: Shared field initializer value emitter. Handles the
    /// match-on-init_expr dispatch for field initialization, shared by
    /// emit_init_state and emit_inline_init_stores. Arguments:
    ///   - tag_strings: when true, OR 1 onto string pointers to mark as
    ///     static (not heap-allocated). Used by inline init but not by
    ///     @init_state (which runs at first access, before heap is live).
    ///   - reg_suffix: suffix for intermediate register names (e.g. "s", "b").
    ///     Use format!("%ip_{}{}", idx, suffix) to generate stable names.
    fn emit_field_init_value(&mut self, out: &mut String, indent: &str,
        init_clone: Option<Expr>, ty: &str, gep: &str, idx: usize, tag_strings: bool)
    {
        let mut field_reg = |suffix: &str| -> String { format!("%ip_{}{}", idx, suffix) };
                // 2026-06-20: Handle LiteralExpr::Float directly, matching Expr::Float arm above.
                // Without this, the catch-all boxes float to i64 and immediately unboxes it
                // back, producing dead IR. LLVM DCE would clean them, but they may cause
                // verifier errors if they cross adapt_to_i64 before DCE runs.
        match init_clone {
            Some(Expr::Decimal(n)) => {
                writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, n, gep, self.align_of("i64")).ok();
            }
            Some(Expr::Float(f)) => {
                // 2026-07-17: State fields are always i64. Box float via
                // bitcast to i32 + zext to i64 before storing.
                let h = float_to_llvm_hex(f);
                if ty == "float" {
                    let bits_reg = field_reg("b");
                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                    writeln!(out, "{}store float {}, ptr {}, align 4", indent, bits_reg, gep).ok();
                } else {
                    let bits_reg = field_reg("b");
                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                    let boxed = field_reg("z");
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, boxed, bits_reg).ok();
                    let zext = field_reg("x");
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, zext, boxed).ok();
                    writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, zext, gep, self.align_of("i64")).ok();
                }
            }
            // 2026-07-14: Float and Float32 unified to Expr::Float(f64).
            // Negative values handled via Expr::UnaryOp(UnaryOpKind::Neg, Expr::Float(f)).
            Some(Expr::UnaryOp(crate::ast::UnaryOpKind::Neg, ref inner)) => {
                match inner.as_ref() {
                    Expr::Float(f) => {
                        // 2026-07-19: Native float fields store directly.
                        let h = float_to_llvm_hex(-*f);
                        if ty == "float" {
                            let h = float_to_llvm_hex(-*f);
                            let bits_reg = field_reg("b");
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                            writeln!(out, "{}store float {}, ptr {}, align 4", indent, bits_reg, gep).ok();
                        } else {
                            let bits_reg = field_reg("b");
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                            let boxed = field_reg("z");
                            writeln!(out, "{}{} = bitcast float {} to i32", indent, boxed, bits_reg).ok();
                            let zext = field_reg("x");
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, zext, boxed).ok();
                            writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, zext, gep, self.align_of("i64")).ok();
                        }
                    }
                    Expr::Decimal(n) => {
                        writeln!(out, "{}store i64 -{}, ptr {}, align {}", indent, n, gep, self.align_of("i64")).ok();
                    }
                    _ => {
                        writeln!(out, "{}store i64 0, ptr {}, align {}", indent, gep, self.align_of("i64")).ok();
                    }
                }
            }
            Some(Expr::Bool(b)) => {
                let v = if b { "1" } else { "0" };
                writeln!(out, "{}store i8 {}, ptr {}, align {}", indent, v, gep, self.align_of("i8")).ok();
            }
            Some(Expr::Quoted(s)) => {
                // 2026-07-14: Store string constant pointer (Quoted replaces LiteralExpr::String).
                let s_str = String::from_utf8_lossy(&s);
                let si = self.ctx.string_constants.iter().position(|x| x.as_str() == s_str).unwrap_or(0);
                let g = format!("@str.{}", si);
                let str_p = field_reg("s");
                writeln!(out, "{}{} = bitcast <{{ i64, [{} x i8] }}>* {} to ptr", indent, str_p, s.len() + 1, g).ok();
                if tag_strings && self.feature_sso_strings {
                    // Tag with bit 0 = 1 to mark as static (not heap-allocated).
                    // 2026-08-01 (B1): ONLY under SSO. Under the bits model a
                    // String value is an UNTAGGED ptr to a length-prefixed
                    // [len][bytes] buffer — OR-ing 1 onto the address makes
                    // brief_str_eq read a misaligned length header and
                    // compare garbage (observed: equal-content strings at
                    // heap vs literal addresses compared unequal). The
                    // untagged path below is the bits-model store.
                    let tag_p = field_reg("t");
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, tag_p, str_p).ok();
                    let tag_o = field_reg("o");
                    writeln!(out, "{}{} = or i64 {}, 1", indent, tag_o, tag_p).ok();
                    let tag_b = field_reg("b");
                    self.emit_inttoptr(out, indent, &tag_b, &tag_o);
                    writeln!(out, "{}store i8* {}, ptr {}, align {}", indent, tag_b, gep, self.align_of("i8*")).ok();
                } else {
                    // 2026-07-14: No tagging for @init_state path — the init_state
                    // function runs at first field access, before heap is live.
                    // String constants in init_state are stored as raw untagged i8*.
                    // 2026-08-01 (B1): the untagged store is also the bits-model
                    // store for inline init (feature_sso_strings off) — the tag
                    // bit belongs to the SSO encoding only.
                    writeln!(out, "{}store i8* {}, ptr {}, align {}", indent, str_p, gep, self.align_of("i8*")).ok();
                }
            }
            Some(expr) => {
                let val_reg = self.emit_expr(out, &expr, indent);
                // 2026-07-19: Store with native type when possible. Skip
                // adapt_to_i64 boxing when the value's LLVM type matches the
                // field's LLVM type (float→"float", double→"double").
                let val_ty = self.llvm_type(&val_reg.ty);
                if val_ty == *ty {
                    writeln!(out, "{}store {} {}, ptr {}, align {}", indent, ty, val_reg.name, gep, self.align_of(ty)).ok();
                } else {
                    let boxed = self.adapt_to_i64(out, indent, &val_reg);
                    writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, boxed, gep, self.align_of("i64")).ok();
                }
            }
            None => {
                if ty == "i8*" {
                    // 2026-06-29: Initialize uninitialized String fields to @str.0
                    // (empty string sentinel, untagged) instead of null.
                    // Must NOT add tag bit (OR 1) because all sentinel comparisons
                    // in trim/submit_input/handle_action check against the untagged
                    // @str.0 address. A tagged pointer would shift all struct
                    // field accesses by 1 byte, causing garbage reads and crashes.
                    let str_p = field_reg("s");
                    writeln!(out, "{}{} = bitcast <{{ i64, i64, [1 x i8] }}>* @str.0 to ptr", indent, str_p).ok();
                    writeln!(out, "{}store i8* {}, ptr {}, align {}", indent, str_p, gep, self.align_of("i8*")).ok();
                } else if ty.starts_with('[') {
                    // 2026-07-31 (A4): array state fields initialize to
                    // zeroinitializer (`store [16 x float] 0` is invalid IR).
                    writeln!(
                        out,
                        "{}store {} zeroinitializer, ptr {}, align {}",
                        indent, ty, gep, self.align_of(ty)
                    )
                    .ok();
                } else {
                    writeln!(out, "{}store {} 0, ptr {}, align {}", indent, ty, gep, self.align_of(&ty)).ok();
                }
            }
        }
    }

    pub(super) fn emit_init_state(&mut self, out: &mut String) {
        writeln!(out, "define void @init_state({}) local_unnamed_addr #0 {{", self.ctx.state_ptr_param).ok();
        writeln!(out, "  entry:").ok();
        let mut fields: Vec<(String, usize, String)> = self.ctx.field_index_map.iter()
            .map(|(name, &idx)| (name.clone(), idx, self.ctx.field_types[idx].clone()))
            .collect();
        fields.sort_by_key(|&(_, idx, _)| idx);
        for (name, idx, ty) in fields {
            // 2026-07-20: Intentionally hand-rolled — register name %ip_{idx} is referenced by
            // emit_field_init_value below which passes %ip_{idx} to ringbuf init codegen.
            let p = format!("%ip_{}", idx);
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", p, idx).ok();
            let init_clone = self.ctx.field_initializers.get(&name).and_then(|e| e.clone());
            // 2026-07-31: Phase 3 (§8.5-E4) — the ringbuf-init detection branch
            // (always-false stub after insert_at removal) is deleted. RingBuffer
            // state fields are expanded inline via ringbuf_inline; a bracket-list
            // initializer falls through to emit_field_init_value as before.
            self.emit_field_init_value(out, "  ", init_clone, &ty, &p, idx, false);
        }
        let mmio_inits: Vec<(u64, Expr)> = {
            let mut v = Vec::new();
            for (name, &addr) in &self.ctx.mmio_fields {
                if let Some(Some(expr)) = self.ctx.mmio_initializers.get(name).cloned() {
                    v.push((addr, expr.clone()));
                }
            }
            v
        };
        for (addr, expr) in mmio_inits {
            let p = format!("%mio{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            self.emit_inttoptr(out, "  ", &p, &addr.to_string());
            let val_reg = self.emit_expr(out, &expr, "  ");
            writeln!(out, "  store volatile i64 {}, ptr {}, align 1", val_reg, p).ok();
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    /// Emit field initialization stores inline in the current function (no function call).
    /// Same logic as emit_init_state but stores are emitted directly, not wrapped in a
    /// separate @init_state function. This prevents the %state alloca from escaping to
    /// init_state, enabling LLVM's SROA to decompose the %State struct.
    ///
    /// WHY inline instead of a separate @init_state call:
    ///   If %state alloca is passed to an external @init_state function, LLVM must
    ///   conservatively assume the call writes to ALL of %State, making SROA analysis
    ///   impossible — the struct stays as one opaque alloca. By inlining the stores,
    ///   every field store is a direct GEP+store that SROA can scalarize and GVN can
    ///   eliminate. This is critical for embedded targets where %State must decompose
    ///   into scalar registers to eliminate the stack alloca entirely.
    ///
    /// WHY every field gets explicit initial values (including zero-initialized ones):
    ///   LLVM's SROA will not promote an alloca to SSA registers if any field is
    ///   undef or has a non-GEP use. Dead fields still need explicit initializers so
    ///   SROA can see every field's lifetime. Zero-initialized fields are store i64 0
    ///   / i8* null explicitly — without these, the field is undef and LLVM may
    ///   introduce poison which inhibits downstream optimizations like load elimination.
    pub(super) fn emit_inline_init_stores(&mut self, out: &mut String, state_ptr: &str) {
        let indent = if state_ptr == "%state" { "  " } else { "" };
        let mut fields: Vec<(String, usize, String)> = self.ctx.field_index_map.iter()
            .map(|(name, &idx)| (name.clone(), idx, self.ctx.field_types[idx].clone()))
            .collect();
        fields.sort_by_key(|&(_, idx, _)| idx);
        for (name, idx, _ty) in &fields {
            let gep_reg = self.emit_state_gep(out, indent, "ip", "%state", *idx);
            let init_clone = self.ctx.field_initializers.get(name).and_then(|e| e.clone());
            let ty = self.ctx.field_types[*idx].clone();
            // 2026-07-31: Phase 3 (§8.5-E4) — always-false ringbuf-init branch
            // deleted; bracket-list initializers go through emit_field_init_value.
            self.emit_field_init_value(out, indent, init_clone, &ty, &gep_reg, *idx, true);
        }
        // Initialize cache slots for LazyCached fields: cache_value = 0, valid_flag = 0
        // 2026-07-19: Sorted for deterministic IR.
        let mut sorted_cache: Vec<&String> = self.ctx.cache_slots.keys().collect();
        sorted_cache.sort();
        for _field_name in &sorted_cache {
            let targets = &self.ctx.cache_slots[*_field_name];
            let mut sorted_targets: Vec<&String> = targets.keys().collect();
            sorted_targets.sort();
            for _target_name in &sorted_targets {
                let &(cache_idx, valid_idx) = &targets[*_target_name];
                // 2026-07-20: Intentionally hand-rolled — uses parameterized state_ptr (not %state).
                let cp = format!("%icp_{}", cache_idx);
                writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, cp, state_ptr, cache_idx).ok();
                writeln!(out, "{}store i64 0, ptr {}, align {}", indent, cp, self.align_of("i64")).ok();
                let vp = format!("%ivp_{}", valid_idx);
                writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, vp, state_ptr, valid_idx).ok();
                writeln!(out, "{}store i8 0, ptr {}, align {}", indent, vp, self.align_of("i8")).ok();
            }
        }
    }

    // ── Parameter Boxing ──────────────────────────────────────────
    //
    // 2026-07-01: Box a function parameter from its native LLVM type to i64
    // using the universe-declared box_op intrinsic. This is called at function
    // entry for parameters whose declared LLVM type differs from i64.
    //
    // Why this exists: Function parameters arrive in their native LLVM type
    // (e.g., i32 for Char, i8 for Bool). We widen them to i64 for uniform SSA
    // register storage. The boxing intrinsic (box_op) from the TypeUniverse
    // tells us exactly which LLVM operation to emit — no more hardcoded
    // Type::char_()/Bool/String/Float match arms.
    //
    // This mirrors adapt_via_box_op in emit_stmt.rs but operates on raw
    // parameter registers rather than TypedRegister SSA values. The key
    // difference: adapt_via_box_op handles already-boxed SSA values (no-op
    // for Char since Char SSA values are always i64), while this method
    // handles native-type parameters that need the initial boxing.
    //
    fn emit_box_param(&mut self, out: &mut String, indent: &str, result: &str, raw: &str, box_op: &str) {
        match box_op {
            // Bool: zext i8 → i64 (Bool parameters arrive as i8)
            "zext.i1.to.i64#" => {
                writeln!(out, "{}{} = zext i8 {} to i64", indent, result, raw).ok();
            }
            // Char / UInt32: zext i32 → i64 (native i32 widened to boxed i64)
            "zext.i32.to.i64#" => {
                writeln!(out, "{}{} = zext i32 {} to i64", indent, result, raw).ok();
            }
            // String/Data: ptrtoint ptr → i64 (native pointer to boxed integer)
            "ptrtoint#" => {
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, result, raw).ok();
            }
            // Float: bitcast f32→i32 then zext i32→i64.
            // Also cache the native float register so ensure_float_reg can
            // recover the native float from the boxed i64 later.
            "bitcast.f32.to.i64#" => {
                let m = format!("%ai{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, m, raw).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, result, m).ok();
                self.fun.reg_float_cache.insert(result.to_string(), raw.to_string());
            }
            // Float64: bitcast double→i64 (same width, direct bitcast).
            // Cache the native double register for downstream unboxing.
            "bitcast.f64.to.i64#" => {
                writeln!(out, "{}{} = bitcast double {} to i64", indent, result, raw).ok();
                self.fun.reg_float_cache.insert(result.to_string(), raw.to_string());
            }
            // Signed fixed-width: sext i8/i16/i32 to i64
            op if op.starts_with("sext.") => {
                let llvm_ty = match op {
                    "sext.i8.to.i64#" => "i8",
                    "sext.i16.to.i64#" => "i16",
                    _ => "i32",
                };
                writeln!(out, "{}{} = sext {} {} to i64", indent, result, llvm_ty, raw).ok();
            }
            // Unsigned fixed-width: zext i8/i16 to i64
            op if op.starts_with("zext.") && op != "zext.i1.to.i64#" && op != "zext.i32.to.i64#" => {
                let llvm_ty = match op {
                    "zext.i8.to.i64#" => "i8",
                    "zext.i16.to.i64#" => "i16",
                    _ => "i32",
                };
                writeln!(out, "{}{} = zext {} {} to i64", indent, result, llvm_ty, raw).ok();
            }
            // Unknown box_op — the parameter stays in its native LLVM type
            // (e.g., Native storage types like raw structs). No conversion.
            _ => {}
        }
    }

    /// Check if a definition's body needs the state pointer.
    /// Pure arithmetic functions (no function calls, no observable side effects)
    /// can be exported without the state parameter, matching C ABI exactly.
    /// 2026-07-23: Used by TopLevel::Export dispatch to skip unnecessary state param.
    pub(super) fn definition_needs_state(&self, d: &crate::ast::Definition) -> bool {
        for stmt in &d.body {
            if Self::stmt_needs_state(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_needs_state(stmt: &Statement) -> bool {
        match stmt {
            Statement::Term(opt) | Statement::TermBang(opt) | Statement::Return(opt) | Statement::Escape(opt) => {
                opt.as_ref().is_some_and(|e| Self::expr_needs_state(e))
            }
            Statement::Expression(expr) => Self::expr_needs_state(expr),
            Statement::Let { expr, .. } => {
                expr.as_ref().is_some_and(|e| Self::expr_needs_state(e))
            }
            Statement::Assign(_, expr) => Self::expr_needs_state(expr),
            Statement::Guarded(_, body) => body.iter().any(Self::stmt_needs_state),
            Statement::If(_, then, els) => {
                then.iter().any(Self::stmt_needs_state) || els.iter().any(Self::stmt_needs_state)
            }
            Statement::Foreach { body, .. } => body.iter().any(Self::stmt_needs_state),
            Statement::Block(body) => body.iter().any(Self::stmt_needs_state),
            // MetadataAssignment is compile-time only, no state needed
            Statement::MetadataAssignment(..) => false,
            // Conservative: non-exhaustive match assumes needs state
            _ => true,
        }
    }

    fn expr_needs_state(expr: &Expr) -> bool {
        match expr {
            // Field access always needs state (reads struct metadata)
            Expr::Field(_, _) => true,
            // Calls: only observable/stateful intrinsics need state.
            // Regular function calls pass state through but don't use it.
            Expr::Call(name, _, _) => {
                matches!(name.as_str(),
                    "Malloc#" | "Memcpy#" | "Memmove#" | "Memset#"
                    | "PrintInt#" | "PrintChar#" | "PrintFloat#" | "Print#"
                    | "FileRead#" | "FileWrite#" | "ShellCmd#"
                    | "SysQuery#" | "EnvGet#" | "HttpFetch#"
                    | "AllocArray#" | "AllocInitArray#" | "StringNew#"
                    | "StringFromPtr#" | "StringConcat#"
                )
            }
            Expr::BinaryOp(_, lhs, rhs) => {
                Self::expr_needs_state(lhs) || Self::expr_needs_state(rhs)
            }
            Expr::UnaryOp(_, inner) => Self::expr_needs_state(inner),
            Expr::List(items) => items.iter().any(Self::expr_needs_state),
            // literals, identifiers are pure
            _ => false,
        }
    }

    pub(super) fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition, needs_state: bool) {
        self.fun.pending_cleanup.clear();
        self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        self.fun.is_static_bound = false;
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();
        // 2026-07-01: Use "i64" for all non-float returns instead of llvm_type().
        // The body always produces i64 values (via adapt_to_i64) and call.rs expects
        // i64 at the call site. Using llvm_type() gave "i8*" for String/Bool returns,
        // creating a type mismatch (ret i64 in a define i8* function) that broke opt/llc.
        let has_ret = d.output_type.is_some() || !d.outputs.is_empty();
        // 2026-07-18: Use the correct LLVM type for the return type instead of
        // always using "i64". Bool returns need "i8" to match term's ret i8.
        let ll_ret_ty = if !has_ret {
            "float".to_string()
        } else {
            d.output_type.as_ref()
                .and_then(|ot| match ot {
                    crate::ast::OutputType::Single(ty) => Some(self.llvm_type(ty)),
                    _ => None,
                })
                .unwrap_or_else(|| "i64".to_string())
        };
        let is_float_fn = ll_ret_ty == "float" || ll_ret_ty == "double";
        self.fun.fn_ret_ty = ll_ret_ty.clone();
        self.fun.returns_i64 = has_ret;
        // Rename user `main` to `brief_main` to avoid collision with
        // the runtime entry point `define i32 @main()` in loop_engine.rs.
        // 2026-07-19: In --shared mode, internal functions keep original names.
        // Export wrappers use a unique suffix to avoid name collision.
        let ll_name: String = if d.name == "main" {
            "brief_main".to_string()
        } else {
            d.name.clone()
        };
        if self.ctx.is_shared_lib {
            write!(out, "define dso_local {} @{}(", ll_ret_ty, ll_name).ok();
        } else {
            write!(out, "define {} @{}(", ll_ret_ty, ll_name).ok();
        }
        if needs_state {
            write!(out, "{}", self.ctx.state_ptr_param).ok();
        }
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            if needs_state || i > 0 {
                let _ = write!(out, ", ");
            }
            // 2026-07-04: dereferenceable(N) for Ptr<T> parameters.
            // LLVM parameter attributes on i64 scalars are rejected by
            // LLVM 18+ (only function-level attributes apply). Ptr<T>
            // parameters are also i64 (opaque pointer addresses) so
            // dereferenceable is ommitted — it only works on pointer
            // types, and Ptr<T> is an i64 at the LLVM level.
            let _ = write!(out, "{} %arg{}", self.llvm_type(t), i);
        }
        // 2026-07-04: Use #8 (argmemonly) for definitions.
        // Definitions never access @link trigger globals — they only
        // read/write through %state. argmemonly tells LLVM the function
        // only accesses memory through its pointer arguments.
        writeln!(out, ") local_unnamed_addr #8 {{").ok();
        writeln!(out, "  entry:").ok();
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let reg: String;
            // 2026-07-01: Use universe-declared box_op for parameter boxing.
            // If llvm_type is already i64, no boxing is needed (Int, UInt).
            // If a box_op exists, use the universe-driven emit_box_param to
            // widen from native type to i64. Otherwise, leave the parameter
            // in its native LLVM type (fixed-width integers like Int32, Native
            // storage types like Float64).
            //
            // This replaces the old pattern of matching on Type::char_()/Bool/etc.
            // The WHETHER-to-box decision is still type-category based (the
            // matches! check below), but the HOW-to-box decision now comes from
            // the universe's box_op field rather than hardcoded LLVM IR.
            let param_llvm_ty = self.llvm_type(t);
            // 2026-07-10: Phase 1 — struct params are passed by pointer (ptr).
            // Convert to i64 via ptrtoint so downstream field access code
            // (which does inttoptr + GEP) works unchanged. The round-trip
            // ptrtoint→inttoptr is eliminated by LLVM's optimizer.
            if param_llvm_ty == "ptr" {
                let conv = format!("%ac{}", i);
                writeln!(out, "  {} = ptrtoint ptr {} to i64", conv, raw).ok();
                reg = conv;
            } else if param_llvm_ty == "i64" {
                reg = raw;
            } else if self.feature_sso_strings
                && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(t))
            {
                // 2026-07-18: SSO String params are {i64, i64} — no boxing needed.
                reg = raw;
            } else if self.is_boxed_type(t) {
                // 2026-07-12: box_op removed from ResolvedType — use hardcoded fallback.
                // 2026-07-31: Phase 3 (§8.4-D1) — boxed-type detection via
                // protocol membership (is_boxed_type), conversion via
                // emit_box_value_to_i64, replacing the name-match arm.
                let conv = format!("%ac{}", i);
                self.emit_box_value_to_i64(out, "  ", t, &raw, &conv, &format!("%ai{}", i));
                if self.is_protocol_member(t, "#Float") {
                    self.fun.reg_float_cache.insert(conv.clone(), raw.to_string());
                }
                reg = conv;
            } else {
                reg = raw;
            }
            self.fun.let_bindings.insert(n.clone(), reg.clone());
            // Boxed params (Bool/Char/String/Data) are stored as i64 in
            // let_bindings after boxing. Register as Type::int() so downstream
            // doesn't treat them as native i1/i32/i8*. Float stays Type::float()
            // (handled specially by ensure_float_reg via the cache).
            // 2026-07-01: This decision is type-category based (Bool/Char/
            // String/Data are semantically boxed types) and cannot be derived
            // from universe data alone — Int32 also has storage="Boxed" but
            // stays native in SSA.
            // 2026-07-01: Always store original type for ALL variable types,
            // not just boxed types. Custom types like RingBuffer<Int> need
            // their original type in let_original_types so that arrow strategy
            // dispatch (check_insert_strategy / check_extract_strategy) can
            // look up the type's InsertAt/ExtractFrom in the TypeUniverse.
            // Without this, <- and discard on RingBuffer would fall through
            // to the default List arena path, causing realloc on non-heap memory.
            self.fun.let_original_types.insert(n.clone(), t.clone());
            if self.is_boxed_int_type(t) {
                self.fun.let_binding_types.insert(n.clone(), Type::int());
            } else {
                self.fun.let_binding_types.insert(n.clone(), t.clone());
            }
        }
        self.fun.txn_counter = 0;
        self.fun.terminated = false;
        // 2026-06-26: in_callable_txn must be true so Statement::Term in the
        // defn body emits a ret instruction (emit_stmt.rs line 78). Without
        // this, Statement::Term becomes a no-op, terminated stays false, and
        // the function falls through to "ret i64 0" — every defn silently
        // returns zero regardless of its actual computation.
        self.fun.in_callable_txn = true;
        for s in &d.body {
            if self.fun.terminated { break; }
            emit_statement(self, out, s, "  ");
        }
        // Foreign destructor cleanup: emit OnExit calls before returning
        self.emit_on_exit_cleanup(out, "  ");
        if !self.fun.terminated {
            if is_float_fn {
                writeln!(out, "  ret float 0.0").ok();
            } else if d.outputs.is_empty() && d.output_type.is_none() {
                // 2026-07-23: Check both legacy outputs and modern output_type.
                // Without this, definitions using -> Int (which sets output_type
                // but leaves outputs empty) get ret void while the function
                // signature says i64.
                writeln!(out, "  ret void").ok();
            } else {
                // 2026-07-23: Emit the correct zero value for the return type.
                // Previously always used "ret i64 0", which broke functions
                // returning ptr, float, double, etc.
                let zero_val = match ll_ret_ty.as_str() {
                    "ptr" => "null",
                    "float" | "double" => "0.0",
                    _ => "0",
                };
                writeln!(out, "  ret {} {}", ll_ret_ty, zero_val).ok();
            }
        }
        writeln!(out, "}}").ok();
        // Phase 4.5: Emit dso_local export wrapper if #export modifier present
        if let Some(export_name) = Self::get_export_name(&d.modifiers) {
            writeln!(out, "define dso_local {} @{}(" , ll_ret_ty, export_name).ok();
            write!(out, "{}", self.ctx.state_ptr_param).ok();
            for (i, (n, t)) in d.parameters.iter().enumerate() {
                write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
            }
            writeln!(out, ") local_unnamed_addr #0 {{").ok();
            write!(out, "  %res = call {} @{}(", ll_ret_ty, ll_name).ok();
            write!(out, "ptr %state").ok();
            for (i, (n, t)) in d.parameters.iter().enumerate() {
                write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
            }
            writeln!(out, ") local_unnamed_addr #0").ok();
            writeln!(out, "  ret {} %res", ll_ret_ty).ok();
        writeln!(out, "}}").ok();
    }

}
    // 2026-06-13: Added ptr %state param — definitions can access global state.
    // Was missing the state pointer, causing invalid LLVM IR (SSA value out of scope).

    // 2026-07-04: Return the known !range bounds for a Brief type based on
    // its byte size in the type universe.  Narrow integer types have
    // representation-level ranges that LLVM can exploit for bounds-check
    // elimination — no contract precondition required.
    // Two-tier range system:
    // 1. Type-driven: from universe byte size (always available, no contract)
    // 2. Contract-driven: from [pre] constraints (may tighten type-driven range)
    // Returns None for types without a known range (Int is unbounded, Float
    // is not range-constrained).
    fn type_driven_range(universe: &TypeUniverse, ty: &Type) -> Option<(i64, i64)> {
        // 2026-07-14: byte_size removed from TypeUniverse, use ResolvedType.bytes instead.
        let key = ty.universe_key()?;
        let resolved = universe.get(key)?;
        // 2026-07-31: Phase 3 (§8.3) — byte→range is a representation fact
        // (N bytes = unsigned range [0, 2^(8N))), DERIVED from resolved.bytes
        // instead of a hardcoded 1/2-byte table. Behavior unchanged for the
        // 1/2-byte cases the previous table covered.
        match resolved.bytes {
            1 | 2 => Some((0, 1i64 << (resolved.bytes * 8))),
            _ => None,
        }
    }

    /// 2026-07-27: Rewrite identifier references in a statement to use cold-function
    /// parameter names. Each Expr::Identifier matching a field_name is replaced with
    /// the corresponding parameter name (__cp_{field}).
    fn rewrite_stmt_idents(stmt: &Statement, field_names: &[String], param_names: &[String]) -> Statement {
        use crate::ast::*;
        match stmt {
            Statement::Let { name, names, ty, expr, modifiers } => {
                Statement::Let {
                    name: name.clone(),
                    names: names.clone(),
                    ty: ty.clone(),
                    expr: expr.as_ref().map(|e| Self::rewrite_expr_idents(e, field_names, param_names)),
                    modifiers: modifiers.clone(),
                }
            }
            Statement::Expression(e) => Statement::Expression(Self::rewrite_expr_idents(e, field_names, param_names)),
            Statement::Assign(lhs, rhs) => {
                Statement::Assign(
                    Self::rewrite_expr_idents(lhs, field_names, param_names),
                    Self::rewrite_expr_idents(rhs, field_names, param_names),
                )
            }
            Statement::Term(Some(e)) => Statement::Term(Some(Self::rewrite_expr_idents(e, field_names, param_names))),
            Statement::TermBang(Some(e)) => Statement::TermBang(Some(Self::rewrite_expr_idents(e, field_names, param_names))),
            Statement::Guarded(cond, body) => {
                Statement::Guarded(
                    Self::rewrite_expr_idents(cond, field_names, param_names),
                    body.iter().map(|s| Self::rewrite_stmt_idents(s, field_names, param_names)).collect(),
                )
            }
            _ => stmt.clone(),
        }
    }

    fn rewrite_expr_idents(expr: &Expr, field_names: &[String], param_names: &[String]) -> Expr {
        use crate::ast::*;
        match expr {
            Expr::Identifier(name) => {
                if let Some(pos) = field_names.iter().position(|f| f == name) {
                    Expr::Identifier(param_names[pos].clone())
                } else {
                    Expr::Identifier(name.clone())
                }
            }
            Expr::Call(name, args, id) => {
                Expr::Call(name.clone(),
                    args.iter().map(|a| Self::rewrite_expr_idents(a, field_names, param_names)).collect(),
                    *id)
            }
            Expr::BinaryOp(op, lhs, rhs) => {
                Expr::BinaryOp(*op,
                    Box::new(Self::rewrite_expr_idents(lhs, field_names, param_names)),
                    Box::new(Self::rewrite_expr_idents(rhs, field_names, param_names)))
            }
            Expr::UnaryOp(op, e) => Expr::UnaryOp(*op, Box::new(Self::rewrite_expr_idents(e, field_names, param_names))),
            Expr::Cast(inner, target) => Expr::Cast(Box::new(Self::rewrite_expr_idents(inner, field_names, param_names)), target.clone()),
            Expr::Field(obj, f) => Expr::Field(Box::new(Self::rewrite_expr_idents(obj, field_names, param_names)), f.clone()),
            Expr::Index(obj, idx) => Expr::Index(Box::new(Self::rewrite_expr_idents(obj, field_names, param_names)), Box::new(Self::rewrite_expr_idents(idx, field_names, param_names))),
            Expr::Block(stmts) => Expr::Block(stmts.iter().map(|s| Self::rewrite_stmt_idents(s, field_names, param_names)).collect()),
            Expr::List(elems) | Expr::Tuple(elems) => {
                let n: Vec<Expr> = elems.iter().map(|e| Self::rewrite_expr_idents(e, field_names, param_names)).collect();
                if let Expr::List(_) = expr { Expr::List(n) } else { Expr::Tuple(n) }
            }
            Expr::Deref(inner) => Expr::Deref(Box::new(Self::rewrite_expr_idents(inner, field_names, param_names))),
            Expr::AddrOf(inner) => Expr::AddrOf(Box::new(Self::rewrite_expr_idents(inner, field_names, param_names))),
            _ => expr.clone(),
        }
    }

    pub(super) fn emit_transaction(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str, range_meta: &mut Vec<String>) {
        let has_output = txn.output_type.is_some() || !txn.outputs.is_empty();
        if !txn.is_reactive && (!txn.parameters.is_empty() || has_output) {
            self.emit_callable_txn(out, txn, name);
            return;
        }
        // 2026-07-27: Set txn_name for per-function arena gating.
        self.fun.txn_name = name.to_string();
        self.fun.pending_cleanup.clear();
        self.ctx.range_bounds = Self::extract_ranges_with_constants(
            &txn.contract.pre_condition, &self.ctx.constants);
        self.ctx.field_to_meta_idx.clear();
        self.ctx.idx_to_field_name.clear();
        for (name, &idx) in &self.ctx.field_index_map {
            self.ctx.idx_to_field_name.insert(idx, name.clone());
        }
        // 2026-08-01: A contract-derived !range is only sound for fields that
        // no node body writes. The range is extracted from the *precondition*
        // (e.g. [tick < 1] ⇒ tick ∈ [MIN, 1)), but emit_state_load attaches it
        // to EVERY load of the field — including the dispatch-loop guard that
        // re-reads state each tick. If the node body writes tick = 1, the value
        // leaves the range on the next iteration, which is UB in LLVM semantics;
        // clang then assumes the guard always fires and the reactor never
        // converges (observed: format-demo infinite loop). The write_set (computed
        // once in the transition graph) is the exact guard: if no node writes the
        // field, the precondition range is loop-invariant and sound. Type-driven
        // ranges below are still emitted for written fields — a UInt8 field holds
        // [0, 256) by type construction regardless of writes.
        let written_fields = self.ctx.transition_graph.as_ref()
            .map(|g| g.nodes.iter().flat_map(|n| n.write_set.iter().cloned()).collect::<HashSet<String>>())
            .unwrap_or_default();
        for (f, &(lo, hi)) in &self.ctx.range_bounds {
            if hi < i64::MAX && !written_fields.contains(f) {
                // 2026-07-18: Offset by 50 to avoid collision with TBAA metadata nodes
                // (!0 through ~!20). LLVM metadata IDs must be unique across the module.
                let mi = range_meta.len() + 50;
                range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, lo, hi));
                self.ctx.field_to_meta_idx.insert(f.clone(), mi);
            }
        }
        // 2026-07-04: Type-driven !range for narrow integer types.
        // The type universe provides byte size information for each type.
        // Types with known ranges (UInt8, UInt16, UInt32, Int8, Char) get
        // automatic !range metadata on their field loads — no contract
        // precondition required. This is information the type system
        // provides for free that LLVM uses to eliminate bounds checks.
        if let Some(ref universe) = self.ctx.type_universe {
            for (field_name, &idx) in &self.ctx.field_index_map {
                let brief_ty = self.ctx.field_brief_types.get(idx);
                let Some(brief_ty) = brief_ty else { continue; };
                let Some(range_bounds) = Self::type_driven_range(universe, brief_ty) else { continue; };
                // Only add if no contract-driven range already exists
                // (contract ranges are tighter and take priority).
                if !self.ctx.field_to_meta_idx.contains_key(field_name) {
                    // 2026-07-18: Offset by 50 to avoid TBAA node collision.
                    let mi = range_meta.len() + 50;
                    range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, range_bounds.0, range_bounds.1));
                    self.ctx.field_to_meta_idx.insert(field_name.clone(), mi);
                }
            }
        }
        // Resolve #inline / #?inline directives from transaction modifiers.
        // #inline forces alwaysinline regardless of cycles.
        // #?inline emits inlinehint (safe with cycles).
        // If neither, fall back to cycle-based alwaysinline.
        let alwaysinline = {
            let dirs = super::directive::resolve_directives(
                &txn.modifiers,
                super::directive::DirectiveCtx::Transaction,
            );
            let inline_attr = dirs.iter().find_map(|e| {
                if let super::directive::DirectiveEffect::FunctionAttribute(a) = e {
                    Some(a.as_str())
                } else {
                    None
                }
            });
            // Emit remarks for speculative #?inline directives.
            if txn.modifiers.iter().any(|m| m.name == "inline" && m.name.starts_with('?')) {
                let remark = match inline_attr {
                    Some("inlinehint") => {
                        super::directive::OptimizationRemark::applied("inline",
                            format!("inlinehint applied to txn '{}'", name))
                    }
                    _ => {
                        super::directive::OptimizationRemark::skipped("inline",
                            "inlinehint not applicable for this context".to_string())
                    }
                };
                self.push_remark(remark);
            }
            match inline_attr {
                Some("alwaysinline") => " alwaysinline",
                Some("inlinehint") => " inlinehint",
                _ => {
                    if !self.ctx.has_cycles { " alwaysinline" } else { "" }
                }
            }
        };
        // 2026-07-28: Phase H.0 — Emit function-level attributes from !> metadata.
        // Iterates over txn.metadata (sorted for deterministic IR), converts each
        // PropertyValue to a string, and looks up the LLVM IR attribute via the
        // MetadataRegistry. The registry's wildcard pattern "*" matches any value.
        let meta_attrs: String = {
            let registry = &self.ctx.metadata_registry;
            let mut attrs = String::new();
            let mut keys: Vec<&String> = txn.metadata.keys().collect();
            keys.sort();
            for key in keys {
                let v = &txn.metadata[key];
                let value_str = match v {
                    crate::ast::PropertyValue::Bool(b) => { if *b { "true".to_string() } else { "false".to_string() } }
                    crate::ast::PropertyValue::Int(n) => n.to_string(),
                    crate::ast::PropertyValue::Float(f) => f.to_string(),
                    crate::ast::PropertyValue::String(s) => s.clone(),
                    crate::ast::PropertyValue::Identifier(s) => s.clone(),
                    _ => continue,
                };
                if let Some(llvm_attr) = registry.llvm_attr(key, &value_str) {
                    attrs.push(' ');
                    attrs.push_str(llvm_attr);
                }
            }
            attrs
        };
        // 2026-07-27: txn_attr for assume_action path (no outlining — rare path).
        // The non-assume_action path below computes its own attr from reordered body.
        let txn_attr = "#0";

        // ── Helper: collect identifiers from a statement ────────────────
        /// 2026-07-28: Check if an expression references the named induction variable.
        fn get_induction_var_name(e: &Expr, var: &str) -> Option<String> {
            match e {
                Expr::Identifier(name) if name == var => Some(name.clone()),
                Expr::BinaryOp(_, lhs, rhs) => {
                    get_induction_var_name(lhs, var).or_else(|| get_induction_var_name(rhs, var))
                }
                Expr::UnaryOp(_, inner) => get_induction_var_name(inner, var),
                _ => None,
            }
        }
        fn collect_let_names(stmt: &Statement, names: &mut Vec<String>) {
            match stmt {
                Statement::Let { name, names: extra, .. } => {
                    names.push(name.clone());
                    for n in extra { names.push(n.clone()); }
                }
                Statement::Guarded(_, body) => { for s in body { collect_let_names(s, names); } }
                Statement::Block(body) => { for s in body { collect_let_names(s, names); } }
                _ => {}
            }
        }
        fn has_ffi_call(stmt: &Statement) -> bool {
            match stmt {
                Statement::Let { expr: Some(e), .. } => is_ffi_call(e),
                Statement::Expression(e) => is_ffi_call(e),
                Statement::Term(Some(e)) | Statement::TermBang(Some(e)) | Statement::Return(Some(e)) => is_ffi_call(e),
                Statement::Assign(_, e) => is_ffi_call(e),
                Statement::Guarded(_, body) => body.iter().any(|s| has_ffi_call(s)),
                Statement::Block(body) => body.iter().any(|s| has_ffi_call(s)),
                _ => false,
            }
        }
        fn is_ffi_call(expr: &Expr) -> bool {
            match expr {
                Expr::Call(name, _, _) => {
                    !name.ends_with('#')
                    // 2026-07-28: Observable intrinsics (PrintInt#, Malloc#, etc.)
                    // also need outlining — they create memory barriers in guard
                    // bodies even though they're not FFI calls. Check the intrinsic
                    // signature's observable flag to distinguish from inert intrinsics
                    // like Add#, Sub#, etc.
                    || crate::intrinsic_signatures::get_intrinsic_signature(name)
                        .map_or(false, |sig| sig.observable)
                }
                Expr::Block(stmts) => stmts.iter().any(|s| has_ffi_call(s)),
                _ => false,
            }
        }
        fn collect_idents(stmt: &Statement, names: &mut Vec<String>) {
            match stmt {
                Statement::Let { expr: Some(e), .. } => collect_expr_idents(e, names),
                Statement::Expression(e) => collect_expr_idents(e, names),
                Statement::Term(Some(e)) | Statement::TermBang(Some(e)) | Statement::Return(Some(e)) => collect_expr_idents(e, names),
                Statement::Assign(lhs, rhs) => { collect_expr_idents(lhs, names); collect_expr_idents(rhs, names); }
                Statement::Guarded(_, body) => { for s in body { collect_idents(s, names); } }
                Statement::Block(body) => { for s in body { collect_idents(s, names); } }
                _ => {}
            }
        }
        fn collect_expr_idents(expr: &Expr, names: &mut Vec<String>) {
            match expr {
                Expr::Identifier(name) => { names.push(name.clone()); }
                Expr::Call(_, args, _) => { for a in args { collect_expr_idents(a, names); } }
                Expr::BinaryOp(_, lhs, rhs) => { collect_expr_idents(lhs, names); collect_expr_idents(rhs, names); }
                Expr::UnaryOp(_, e) => collect_expr_idents(e, names),
                Expr::Cast(inner, _) => collect_expr_idents(inner, names),
                Expr::Field(obj, _) => collect_expr_idents(obj, names),
                Expr::Index(obj, idx) => { collect_expr_idents(obj, names); collect_expr_idents(idx, names); }
                Expr::Block(stmts) => { for s in stmts { collect_idents(s, names); } }
                Expr::List(elems) | Expr::Tuple(elems) => { for e in elems { collect_expr_idents(e, names); } }
                Expr::Deref(inner) | Expr::AddrOf(inner) | Expr::IsType(inner, _) => collect_expr_idents(inner, names),
                _ => {}
            }
        }

        let assume_action: Option<String> = txn.modifiers.iter()
            .find(|m| m.name == "assume_shape")
            .and_then(|m| m.value.as_ref().and_then(|v| {
                if let Expr::Quoted(bytes) = v {
                    Some(String::from_utf8_lossy(bytes).to_string())
                } else {
                    None
                }
            }))
            .and_then(|v| {
                let parts: Vec<&str> = v.splitn(2, ", ").collect();
                if parts.len() == 2 {
                    let action = parts[1].trim();
                    if action == "run" || action == "exit" { Some(action.to_string()) } else { Some("escape".to_string()) }
                } else {
                    Some("escape".to_string())
                }
            });

        // 2026-07-31: Phase 3 (§8.5-E5) — the unconditional `br i1 true, label
        // %body, label %rollback` made the rollback block (and its
        // exit/run/escape action handling) unreachable dead code; both removed.
        // The assume_shape modifier still selects this emission path.
        if assume_action.is_some() {
            // 2026-07-17: Use @txn_<name> prefix so call sites in emit_ssa_loop
            // and emit_folded_multi_main can reference the function (they call
            // @txn_{name}). Without this, the definition is @<name> but the
            // call is @txn_<name> — undefined reference error at link time.
            writeln!(out, "define void @txn_{}({}) local_unnamed_addr {}{}{} {{", name, self.ctx.state_ptr_param, txn_attr, alwaysinline, meta_attrs).ok();
            writeln!(out, "  entry:").ok();
            // Arena for body emission — same rationale as the standard path:
            // the reactor dispatch calls @txn_name as a separate function,
            // so arena allocas must live here, not in main().
            self.emit_arena_init(out, "  ");
            self.fun.ssa_old_int_regs.clear();
            self.fun.ssa_old_float_regs.clear();
            // 2026-06-28: Do NOT reset txn_counter here — this emits into the
            // existing @main() function. Resetting would produce duplicate
            // %t{N} registers across inlined transactions, violating SSA.
            // The counter keeps incrementing across all inlined transactions.
            self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
            self.fun.terminated = false;
            // 2026-06-26: Reset in_callable_txn — emit_definition may have left
            // it true from a prior TopLevel::Definition. Reactive transactions
            // use the non-callable code path (emit_stmt.rs:162). Without this,
            // term!/TermBang inside guards takes the callable path, emits no
            // ret/br, and leaves basic blocks unterminated.
            self.fun.in_callable_txn = false;
            self.fun.returns_i64 = false;
            self.fun.fn_ret_ty = "void".to_string();
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            // 2026-07-29: Statement reordering removed — proven counterproductive.
            // LLVM's scheduler does this better within each basic block.
            for s in &txn.body {
                if self.fun.terminated { break; }
                emit_statement(self, out, s, "  ");
            }
            if !self.fun.terminated {
                self.emit_arena_fini(out, "  ");
                writeln!(out, "  ret void").ok();
            }
            writeln!(out, "}}").ok();
        } else {
            // 2026-07-29: Statement reordering removed — proven counterproductive.
            // Use body directly (reordering removed 2026-07-29).
            // The `reordered` alias preserves existing code patterns below.
            let body = &txn.body;
            let reordered = body;
            // 2026-07-27: Three-category outlining — identifiers can be state
            // fields (GEP+load), let bindings (lookup at emission time), or
            // compile-time constants (ctx.constants). Only `Unknown` blocks
            // outlining. This lets ring_buffer (references CAP) and nbody
            // (references energy) get #11 = memory(argmem: readwrite).
            #[derive(Debug, Clone)]
            enum ParamSrc {
                StateField(usize),
                LetBinding(String), // LLVM type string ("float" or "i64")
                Constant(Expr, Type), // value expression + Brief type
            }
            // Scan body for FFI guards that can be outlined.
            let outlined_info: Vec<(usize, String, Vec<(String, String, ParamSrc)>)> = {
                let mut ffi_guard_indices: Vec<usize> = Vec::new();
                let mut guard_bodies: Vec<&[Statement]> = Vec::new();
                for (ri, s) in reordered.iter().enumerate() {
                    if let Statement::Guarded(_, body) = s {
                        if body.iter().any(|s| has_ffi_call(s)) {
                            ffi_guard_indices.push(ri);
                            guard_bodies.push(body);
                        }
                    }
                }
                let mut can_outline_all = true;
                let mut param_sets: Vec<Vec<(String, String, ParamSrc)>> = Vec::new();
                // Collect let-names defined in the txn body before each guard
                // (pre-scan: which identifiers are let bindings?)
                let mut txn_let_names: Vec<String> = Vec::new();
                let mut txn_let_types: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                for s in reordered.iter() {
                    collect_let_names(s, &mut txn_let_names);
                    if let Statement::Let { name, ty: Some(t), .. } = s {
                        // 2026-07-31: Phase 3 (§8.4) — float let-param LLVM type
                        // derived from #Float protocol membership + byte width
                        // (4 → float, 8 → double) instead of the type-name match.
                        // The casting graph's Fixed("float") for the #Float
                        // category doesn't distinguish Float64, so width is read
                        // from the universe bytes. Other widths (e.g. BFloat)
                        // stay i64, matching the prior name-based behavior.
                        let llvm_ty = if self.is_protocol_member(t, "#Float") {
                            let bytes = self.ctx.type_universe.as_ref()
                                .and_then(|u| t.universe_key().and_then(|k| u.get(k)))
                                .map(|rt| rt.bytes).unwrap_or(4);
                            if bytes == 8 { "double" } else if bytes == 4 { "float" } else { "i64" }
                        } else {
                            "i64"
                        };
                        txn_let_types.insert(name.clone(), llvm_ty.to_string());
                    }
                }
                txn_let_names.sort();
                txn_let_names.dedup();
                for body in &guard_bodies {
                    let mut idents: Vec<String> = Vec::new();
                    let mut local_lets: Vec<String> = Vec::new();
                    for stmt in *body {
                        collect_idents(stmt, &mut idents);
                        collect_let_names(stmt, &mut local_lets);
                    }
                    idents.sort(); idents.dedup();
                    local_lets.sort(); local_lets.dedup();
                    idents.retain(|i| !local_lets.contains(i));
                    let mut params: Vec<(String, String, ParamSrc)> = Vec::new();
                    for ident in &idents {
                        if let Some(&idx) = self.ctx.field_index_map.get(ident) {
                            // Ptr<T> fields are stored as i64 in %State (opaque handles).
                            // Float fields use the native float type. All others use i64.
                            let brief_ty = self.ctx.field_brief_types.get(idx).cloned().unwrap_or(Type::int());
                            // 2026-07-31 (A4): aggregate (array) fields cannot be
                            // outlined as scalar cold-function params — the guard
                            // must stay inline so it reads them via the %State GEP.
                            if matches!(brief_ty, Type::Vector(_, _)) {
                                can_outline_all = false;
                                break;
                            }
                            let llvm_ty = if matches!(brief_ty, Type::Ptr(_)) {
                                "i64".to_string()
                            } else {
                                self.llvm_type(&brief_ty)
                            };
                            params.push((ident.clone(), llvm_ty, ParamSrc::StateField(idx)));
                        } else if txn_let_names.contains(ident) {
                            let llvm_ty = txn_let_types.get(ident).cloned().unwrap_or_else(|| "i64".to_string());
                            params.push((ident.clone(), llvm_ty.clone(), ParamSrc::LetBinding(llvm_ty)));
                        } else if let Some((const_ty, val_expr)) = self.ctx.constants.get(ident.as_str()) {
                            // 2026-07-31: Phase 3 (§8.4) — the old `== "Float" →
                            // "float"` special case is redundant: llvm_type(Float)
                            // already resolves to "float" via the casting graph,
                            // so the const param LLVM type is derived uniformly.
                            let llvm_ty = self.llvm_type(const_ty);
                            params.push((ident.clone(), llvm_ty, ParamSrc::Constant(val_expr.clone(), const_ty.clone())));
                        } else {
                            can_outline_all = false;
                            break;
                        }
                    }
                    if !can_outline_all { break; }
                    if !params.is_empty() {
                        param_sets.push(params);
                    }
                }
                if !can_outline_all { Vec::new() }
                else {
                    ffi_guard_indices.into_iter().zip(param_sets.into_iter())
                        .enumerate()
                        .map(|(ci, (ri, params))| {
                            (ri, format!("txn_{}_cold_{}", name, ci), params)
                        })
                        .collect()
                }
            };
            let local_outlined = !outlined_info.is_empty();
            let mut local_txn_attr = if local_outlined { "#11".to_string() } else { txn_attr.to_string() };
            // 2026-07-28: Dense matrix detection — if #11 would be selected but
            // the txn has dense cross-field float computation (cross-per-field > 8),
            // force #0 = memory(readwrite) instead. LLVM's auto-vectorizer creates
            // expensive wide vectors (<12 x float>) for dense matrices that cause
            // register spilling — the kalman 3.5x regression.
            // 2026-07-31: Phase 2 — the measurement is computed once in the
            // frontend (src/analysis/density.rs, plan §7.1) instead of re-counting
            // cross ops here. The frontend version FIXES the old metric's gap:
            // count_cross_float_ops_in_expr ignored its _all_idents set, so int-only
            // counter arithmetic inflated the count. Only txns with dense cross-field
            // FLOAT computation downgrade.
            // 2026-07-31: Phase 3 (§8.1) — the threshold comes from
            // config/targets.toml `dense_compute_density` (default 4.0).
            if local_txn_attr == "#11" {
                if let Some(d) = self.ctx.density.get(name) {
                    let threshold = crate::config_tuning::target_settings_for(&self.ctx.target_triple)
                        .dense_compute_density;
                    if d.float_idents > 4 && d.per_field > threshold {
                        local_txn_attr = "#0".to_string();
                    }
                }
            }

            // Emit the txn function
            writeln!(out, "define void @txn_{}({}) local_unnamed_addr {}{}{} {{", name, self.ctx.state_ptr_param, local_txn_attr, alwaysinline, meta_attrs).ok();
            writeln!(out, "  entry:").ok();
            self.fun.ssa_old_int_regs.clear();
            self.fun.ssa_old_float_regs.clear();
            self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
            self.fun.terminated = false;
            self.fun.in_callable_txn = false;
            self.fun.returns_i64 = false;
            self.fun.fn_ret_ty = "void".to_string();
            self.emit_arena_init(out, "  ");
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            // Emission loop with guard substitution
            for (ri, s) in reordered.iter().enumerate() {
                if self.fun.terminated { break; }
                if let Some((_, cold_name, params)) = outlined_info.iter().find(|(idx, _, _)| *idx == ri) {
                    if let Statement::Guarded(cond, _) = s {
                        let cond_reg = self.emit_expr(out, cond, "  ");
                        let label_n = self.fun.txn_counter;
                        self.fun.txn_counter += 1;
                        let then_lbl = format!("guard.then{}", label_n);
                        let end_lbl = format!("guard.end{}", label_n);
                        let cond_i1 = if cond_reg.ty == Type::bool_() {
                            let b = self.fun.gen_reg();
                            writeln!(out, "  {} = trunc i8 {} to i1", b, cond_reg.name).ok();
                            b
                        } else {
                            cond_reg.name.clone()
                        };
                        // 2026-07-27: Compute !prof branch weights from guard
                        // condition and postcondition bound. For a modulo condition
                        // like `count % N == C` with postcondition `[count == total]`,
                        // the guard fires `ceil(total / N)` times out of `total`.
                        let prof_meta = {
                            // 2026-07-28: Extended !prof computation — uses transition graph
                            // data (bounded_pre, increments) and iter_bounds for more precise
                            // weights. Falls back to postcondition + modulo analysis.
                            let txn_name = name;
                            let bounded_pre = self.ctx.transition_graph.as_ref()
                                .and_then(|tg| tg.nodes.iter().find(|n| n.name == txn_name))
                                .and_then(|n| n.bounded_pre.as_ref());
                            let increment = self.ctx.transition_graph.as_ref()
                                .and_then(|tg| tg.nodes.iter().find(|n| n.name == txn_name))
                                .and_then(|n| n.increments.as_ref());
                            let iter_bound = self.ctx.iter_bounds.get(txn_name).copied();

                            // Helper: compute and scale !prof weights
                            // 2026-07-31: Phase 3 (§8.3) — the cap is i32-range
                            // normalization, not a tunable: LLVM branch_weights
                            // are i32, and the cap is a power of two near
                            // i32::MAX / 2 (2^30). Scaling keeps the SUM ≤ cap so
                            // every weight fits i32 with no overflow and minimal
                            // ratio rounding — more precise than the old 1000 cap.
                            let scale_weights = |taken: u64, not_taken: u64| -> Option<(u32, u32)> {
                                let max_w = 1u64 << 30;
                                let total = taken + not_taken;
                                if taken == 0 || not_taken == 0 { return None; }
                                let (wt, wn) = if total <= max_w {
                                    (taken as u32, not_taken as u32)
                                } else {
                                    let ratio = total as f64 / max_w as f64;
                                    ((taken as f64 / ratio).ceil() as u32,
                                     (not_taken as f64 / ratio).ceil() as u32)
                                };
                                if wt > 0 && wn > 0 { Some((wt, wn)) } else { None }
                            };
                            let format_weights = |wt: u32, wn: u32| -> String {
                                format!(", !prof !{{!\"branch_weights\", i32 {}, i32 {}}}", wt, wn)
                            };

                            // Strategy 1: Use transition graph bounded_pre + increments
                            if let (Some(bp), Some(inc), Some(ib)) = (bounded_pre, increment, iter_bound) {
                                let step = inc.delta.unsigned_abs() as u64;
                                // Match guard condition referencing the induction variable
                                let guard_var = match cond {
                                    Expr::BinaryOp(BinaryOpKind::Eq, lhs, _) => {
                                        get_induction_var_name(lhs, &bp.var)
                                    }
                                    _ => None,
                                };
                                if guard_var.is_some() && step > 0 && ib > 0 {
                                    // Extract divisor from modulo: var % N == C
                                    let mod_n = match cond {
                                        Expr::BinaryOp(BinaryOpKind::Eq, lhs, _) => match lhs.as_ref() {
                                            Expr::BinaryOp(BinaryOpKind::Mod, _, d) => match d.as_ref() {
                                                Expr::Decimal(n) if *n > 0 => Some(*n as u64),
                                                _ => None,
                                            },
                                            _ => None,
                                        },
                                        _ => None,
                                    };
                                    if let Some(mn) = mod_n {
                                        // Modulo pattern: count % N == C
                                        let taken = ib / (step * mn);
                                        let not_taken = ib.saturating_sub(taken);
                                        if let Some((wt, wn)) = scale_weights(taken, not_taken) {
                                            format_weights(wt, wn)
                                        } else { String::new() }
                                    } else {
                                        // General induction variable constraint:
                                        // Use iter_bound / step as total, guard fires 1 per step
                                        let taken = ib / step;
                                        let not_taken = ib.saturating_sub(taken);
                                        if let Some((wt, wn)) = scale_weights(taken, not_taken) {
                                            format_weights(wt, wn)
                                        } else { String::new() }
                                    }
                                } else { String::new() }
                            } else {
                                // Strategy 2: Postcondition + modulo (original Phase 2)
                                // Extract postcondition bound: [x == N]
                                let post_bound: Option<i64> = (|| {
                                    let (lhs, rhs) = match &txn.contract.post_condition {
                                        Expr::BinaryOp(BinaryOpKind::Eq, l, r) => (l.as_ref(), r.as_ref()),
                                        _ => return None,
                                    };
                                    let n = match rhs {
                                        Expr::Decimal(n) => Some(*n),
                                        Expr::Identifier(id) => self.ctx.constants.get(id.as_str())
                                            .and_then(|(_, e)| if let Expr::Decimal(v) = e { Some(*v) } else { None }),
                                        _ => None,
                                    };
                                    n.filter(|&n| n > 0)
                                })();
                                // Extract modulo divisor from guard condition: x % N == C
                                let mod_div: Option<i64> = (|| {
                                    let lhs = match cond {
                                        Expr::BinaryOp(BinaryOpKind::Eq, l, _) => l.as_ref(),
                                        _ => return None,
                                    };
                                    match lhs {
                                        Expr::BinaryOp(BinaryOpKind::Mod, _, d) => match d.as_ref() {
                                            Expr::Decimal(n) if *n > 0 => Some(*n),
                                            _ => None,
                                        },
                                        _ => None,
                                    }
                                })();
                                match (post_bound, mod_div) {
                                    (Some(bound), Some(mod_n)) => {
                                    let taken = (bound as f64 / mod_n as f64).ceil() as u64;
                                    let not_taken = (bound as u64).saturating_sub(taken);
                                    // 2026-07-31: Phase 3 (§8.3) — i32-range
                                    // normalization cap, power of two near
                                    // i32::MAX / 2 (see scale_weights above).
                                    let max_w = 1u64 << 30;
                                    let total = taken + not_taken;
                                    let (wt, wn) = if total <= max_w {
                                        (taken as u32, not_taken as u32)
                                    } else {
                                        let ratio = total as f64 / max_w as f64;
                                        ((taken as f64 / ratio).ceil() as u32,
                                         (not_taken as f64 / ratio).ceil() as u32)
                                    };
                                    // Only emit if weights are meaningful
                                    if wt > 0 && wn > 0 {
                                        // First weight = true branch (guard fires, cold),
                                        // second weight = false branch (guard doesn't fire, hot)
                                        format!(", !prof !{{!\"branch_weights\", i32 {}, i32 {}}}", wt, wn)
                                    } else { String::new() }
                                }
                                _ => String::new(),
                            }
                        }
                        };
                        writeln!(out, "  br i1 {}, label %{}, label %{}{}",
                            cond_i1, then_lbl, end_lbl, prof_meta).ok();
                        writeln!(out, "  {}:", then_lbl).ok();
                        // Emit load for each param — handles three cases:
                        // StateField: GEP+load from %state
                        // LetBinding: lookup register from let_bindings table
                        // Constant: emit literal value (Expr::Decimal or Expr::Float)
                        let mut param_regs: Vec<String> = Vec::new();
                        for (p_name, llvm_ty, src) in params {
                            match src {
                                ParamSrc::StateField(idx) => {
                                    let gep_reg = self.fun.gen_reg();
                                    let load_reg = self.fun.gen_reg();
                                    writeln!(out, "    {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gep_reg, idx).ok();
                                    writeln!(out, "    {} = load {}, ptr {}", load_reg, llvm_ty, gep_reg).ok();
                                    param_regs.push(load_reg);
                                }
                                ParamSrc::LetBinding(_) => {
                                    if let Some(reg) = self.fun.let_bindings.get(p_name) {
                                        param_regs.push(reg.clone());
                                    } else {
                                        // Fallback: load from state (may fail)
                                        let fallback = self.fun.gen_reg();
                                        writeln!(out, "    {} = add i64 0, 0", fallback).ok();
                                        param_regs.push(fallback);
                                    }
                                }
                                ParamSrc::Constant(val_expr, _) => {
                                    let const_reg = self.fun.gen_reg();
                                    match val_expr {
                                        Expr::Decimal(v) => {
                                            writeln!(out, "    {} = add i64 0, {}", const_reg, v).ok();
                                        }
                                        Expr::Float(v) => {
                                            let hex = crate::backend::llvm::float_to_llvm_hex(*v);
                                            writeln!(out, "    {} = bitcast i32 {} to float", const_reg, hex).ok();
                                        }
                                        _ => {
                                            writeln!(out, "    {} = add i64 0, 0", const_reg).ok();
                                        }
                                    }
                                    param_regs.push(const_reg);
                                }
                            }
                        }
                        let mut typed_args: Vec<String> = Vec::new();
                        for (fi, (_, llvm_ty, _)) in params.iter().enumerate() {
                            typed_args.push(format!("{} {}", llvm_ty, param_regs[fi]));
                        }
                        writeln!(out, "    call void @{}({})", cold_name, typed_args.join(", ")).ok();
                        writeln!(out, "    br label %{}", end_lbl).ok();
                        writeln!(out, "  {}:", end_lbl).ok();
                        self.fun.terminated = false;
                    }
                } else {
                    emit_statement(self, out, s, "  ");
                }
            }
            if !self.fun.terminated {
                self.emit_arena_fini(out, "  ");
            }
            writeln!(out, "  ret void").ok();
            writeln!(out, "}}").ok();

            // Emit cold functions after the txn function
            for (ri, cold_name, fields) in &outlined_info {
                let body = match &reordered[*ri] {
                    Statement::Guarded(_, body) => body.clone(),
                    _ => continue,
                };
                let saved_let_bindings = self.fun.let_bindings.clone();
                let saved_let_binding_types = self.fun.let_binding_types.clone();
                let saved_let_original_types = self.fun.let_original_types.clone();
                let saved_reg_float_cache = self.fun.reg_float_cache.clone();
                let saved_reg_type_cache = self.fun.reg_type_cache.clone();
                self.fun.let_bindings.clear();
                self.fun.let_binding_types.clear();
                self.fun.let_original_types.clear();
                self.fun.reg_float_cache.clear();
                self.fun.reg_type_cache.clear();

                // Build param list — register with correct Brief type so that
                // emit_statement uses pointer semantics (inttoptr+GEP+load) rather
                // than vector semantics (extractelement) for Ptr<Int> fields.
                let field_names: Vec<String> = fields.iter().map(|(f, _, _)| f.clone()).collect();
                let cp_names: Vec<String> = fields.iter().map(|(f_name, _, _)| format!("__cp_{}", f_name)).collect();
                let mut param_sig: Vec<String> = Vec::new();
                for (fi, (f_name, llvm_ty, src)) in fields.iter().enumerate() {
                    param_sig.push(format!("{} %{}", llvm_ty, cp_names[fi]));
                    let brief_ty = match src {
                        ParamSrc::StateField(idx) => {
                            self.ctx.field_brief_types.get(*idx).cloned().unwrap_or(Type::int())
                        }
                        ParamSrc::LetBinding(llvm_ty) => {
                            if llvm_ty == "float" { Type::float() } else { Type::int() }
                        }
                        ParamSrc::Constant(_, brief_ty) => brief_ty.clone(),
                    };
                    self.fun.let_bindings.insert(cp_names[fi].clone(), format!("%{}", cp_names[fi]));
                    self.fun.let_binding_types.insert(cp_names[fi].clone(), brief_ty.clone());
                    self.fun.let_original_types.insert(cp_names[fi].clone(), brief_ty);
                }
                writeln!(out, "define void @{}({}) local_unnamed_addr #0 {{", cold_name, param_sig.join(", ")).ok();

                // Rewrite guard body: replace ident references with param names
                for stmt in &body {
                    let rewritten = Self::rewrite_stmt_idents(stmt, &field_names, &cp_names);
                    emit_statement(self, out, &rewritten, "  ");
                }
                writeln!(out, "  ret void").ok();
                writeln!(out, "}}").ok();
                writeln!(out).ok();

                self.fun.let_bindings = saved_let_bindings;
                self.fun.let_binding_types = saved_let_binding_types;
                self.fun.let_original_types = saved_let_original_types;
                self.fun.reg_float_cache = saved_reg_float_cache;
                self.fun.reg_type_cache = saved_reg_type_cache;
            }
        }

        // Collect GPU kernel for this transaction if it has #gpu / #!gpu / #?gpu.
        if self.ctx.gpu_offload || txn.modifiers.iter().any(|m| m.name == "gpu") {
            let is_speculative = txn.modifiers.iter()
                .any(|m| m.name == "gpu" && m.name.starts_with('?'));
            self.collect_gpu_kernel(name, &txn.body, is_speculative);
        }
    }

    pub(super) fn emit_callable_txn(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        // 2026-07-27: Set txn_name for per-function arena gating.
        self.fun.txn_name = name.to_string();
        self.fun.pending_cleanup.clear();
        self.fun.let_bindings.clear();
        self.fun.let_binding_types.clear();
        self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear();
        self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        // 2026-07-18: Detect bounded pre-condition (e.g. x < N) — enables
        // Alloca strategy for stack-allocated temporaries within the loop.
        self.fun.is_static_bound = matches!(&txn.contract.pre_condition,
            Expr::BinaryOp(kind, left, _) if matches!(kind,
                crate::ast::BinaryOpKind::Lt | crate::ast::BinaryOpKind::Le
                    | crate::ast::BinaryOpKind::Lt | crate::ast::BinaryOpKind::Ge)
                && matches!(left.as_ref(), Expr::Identifier(_)));
        self.fun.param_slots.clear();
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();

        let has_return = if let Some(ref ot) = txn.output_type {
            match ot {
                crate::ast::OutputType::Single(ty) => !matches!(ty, Type::Void),
                crate::ast::OutputType::Tuple(ts) => !ts.is_empty(),
                _ => false,
            }
        } else {
            !txn.outputs.is_empty() && !matches!(txn.outputs.first(), Some(Type::Void))
        };
        // 2026-07-18: Use correct LLVM type for return instead of always "i64".
        let ret_llvm = if has_return {
            txn.output_type.as_ref()
                .and_then(|ot| match ot {
                    crate::ast::OutputType::Single(ty) => Some(self.llvm_type(ty)),
                    _ => None,
                })
                .unwrap_or_else(|| "i64".to_string())
        } else {
            "void".to_string()
        };

        // Resolve #inline / #?inline directives for callable txns.
        let inline_attr = {
            let dirs = super::directive::resolve_directives(
                &txn.modifiers,
                super::directive::DirectiveCtx::CallableTxn,
            );
            dirs.iter().find_map(|e| {
                if let super::directive::DirectiveEffect::FunctionAttribute(a) = e {
                    Some(format!(" {}", a))
                } else {
                    None
                }
            })
        };
        // 2026-07-19: Auto-inline small callable txns — few params, small body,
        // no frgn calls. Lets LLVM optimize cross-function (e.g. memcmp_loop).
        // 2026-07-31: Phase 2 — the decision is computed once in the frontend
        // (src/analysis/inline_cost.rs, plan §7.3) with a weighted body cost
        // (call=10, binop=1) instead of the `params < 8 && body < 20` statement
        // count. The `term`-statement gate is preserved (has_ffi_or_trigger_stmt
        // treats Term as ffi-like), so callable txns that return via `term` are
        // still never auto-inlined — matching prior behavior.
        let auto_inline = if inline_attr.is_none()
            && self.ctx.inline_decisions.get(name).map_or(false, |d| {
                matches!(*d, crate::analysis::inline_cost::InlineDecision::AlwaysInline)
            })
        {
            " alwaysinline"
        } else {
            ""
        };
        let inline_str = inline_attr.as_deref().unwrap_or(auto_inline);

        write!(out, "define {} @{}(", ret_llvm, name).ok();
        write!(out, "{}", self.ctx.state_ptr_param).ok();
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            // 2026-07-04: Parameter-level attributes are omitted because
            // Ptr<T> is i64 at the LLVM level (not a pointer type). LLVM
            // function-level attributes (#8) provide the important guarantees
            // (argmemonly, nofree, nosync, nounwind).
            let _ = write!(out, ", {} %arg{}", self.llvm_type(t), i);
        }
        // 2026-07-04: Use #8 (argmemonly) for callable transactions.
        // Callable txns never access @link trigger globals — they only
        // read/write through %state. argmemonly tells LLVM the function
        // only accesses memory through its pointer arguments.
        writeln!(out, ") local_unnamed_addr #8{} {{", inline_str).ok();
        writeln!(out, "  entry:").ok();

        // 2026-07-18: Use the function's return type for %result, not always i64.
        // Bool-returning functions need i8 result type to match the define i8 signature.
        writeln!(out, "  %result = alloca {}, align 8", ret_llvm).ok();
        writeln!(out, "  store {} 0, ptr %result, align 8", ret_llvm).ok();

        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv: String;
            // 2026-07-01: Use universe-declared box_op for parameter boxing.
            // Same approach as emit_definition — keeps signature and boxing
            // consistent through universe data.
            let param_llvm_ty = self.llvm_type(t);
            // 2026-07-10: Phase 1 — struct params are passed by pointer (ptr).
            // Convert to i64 via ptrtoint for storage in param_slots.
            if param_llvm_ty == "ptr" {
                let ac = format!("%ac{}", i);
                writeln!(out, "  {} = ptrtoint ptr {} to i64", ac, raw).ok();
                conv = ac;
            } else if param_llvm_ty == "i64" {
                conv = raw;
            } else if self.is_boxed_type(t) {
                // 2026-07-12: box_op removed from ResolvedType — use hardcoded fallback.
                // 2026-07-31: Phase 3 (§8.4-D1) — protocol-membership boxed-type
                // detection + emit_box_value_to_i64 replacing the name match.
                let ac = format!("%ac{}", i);
                self.emit_box_value_to_i64(out, "  ", t, &raw, &ac, &format!("%ai{}", i));
                conv = ac;
            } else {
                conv = raw;
            }
            let slot = format!("%p{}_s", i);
            writeln!(out, "  {} = alloca i64, align 8", slot).ok();
            writeln!(out, "  store i64 {}, ptr {}, align 8", conv, slot).ok();
            self.fun.param_slots.insert(n.clone(), slot);
        }

        writeln!(out, "  br label %loop").ok();
        writeln!(out, "loop:").ok();
        // 2026-07-26: Set convergence target so [expr]; gates inside the txn
        // body branch back to loop: when their condition is false.
        self.fun.convergence_target = Some("loop".to_string());

        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let slot = format!("%p{}_s", i);
            let loaded = format!("%p{}_l{}", i, self.fun.txn_counter);
            self.fun.txn_counter += 1;
            writeln!(out, "  {} = load i64, ptr {}, align 8", loaded, slot).ok();
            // 2026-07-18: Store the SLOT (alloca) in let_bindings, not the loaded
            // register. This makes assigns to the parameter (`i = i + 1`) store
            // through the alloca correctly. Reads still load from the slot, but
            // LLVM's optimizer merges redundant loads via SROA.
            self.fun.let_bindings.insert(n.clone(), slot.clone());
            // loaded is i64 (boxed value from param slot). Store Type::int()
            // for boxed types so downstream doesn't treat them as native.
            // 2026-07-31: Phase 3 (§8.4-D1) — is_boxed_int_type (protocol
            // membership) replaces the hardcoded name set.
            if self.is_boxed_int_type(t) {
                self.fun.let_binding_types.insert(n.clone(), Type::int());
            } else {
                self.fun.let_binding_types.insert(n.clone(), t.clone());
            }
        }

        // 2026-07-20: Set fn_ret_ty so term codegen stores with the correct type
        // (Bool → i8, Int → i64, etc.) instead of whatever the previous function left.
        self.fun.fn_ret_ty = ret_llvm.clone();
        self.fun.callable_txn_result = Some("%result".to_string());
        self.fun.callable_txn_post_label = Some("post".to_string());
        self.fun.in_callable_txn = true;
        self.fun.txn_counter = 0;
        self.fun.terminated = false;
        self.fun.returns_i64 = has_return;

        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
            let i1 = format!("%pc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            if cond.ty == Type::bool_() {
                writeln!(out, "  {} = trunc i8 {} to i1", i1, cond).ok();
            } else {
                writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            }
            writeln!(out, "  br i1 {}, label %body, label %done", i1).ok();
        } else {
            writeln!(out, "  br label %body").ok();
        }

        writeln!(out, "body:").ok();

        for s in &txn.body {
            if self.fun.terminated { break; }
            emit_statement(self, out, s, "  ");
        }

        // Foreign destructor cleanup: emit OnExit calls before loop exit
        self.emit_on_exit_cleanup(out, "  ");

        if !self.fun.terminated {
            writeln!(out, "  br label %post").ok();
        }
        writeln!(out, "post:").ok();
        writeln!(out, "  br label %loop").ok();

        writeln!(out, "done:").ok();
        if has_return {
            let ret = format!("%ret{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = load {}, ptr %result, align 8", ret, ret_llvm).ok();
            writeln!(out, "  ret {} {}", ret_llvm, ret).ok();
        } else {
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();

        self.fun.callable_txn_result = None;
        self.fun.callable_txn_post_label = None;
        self.fun.in_callable_txn = false;
        self.fun.param_slots.clear();
    }

    //
    // WHY both br...unreachable AND @llvm.assume / !range are needed:
    //   Two distinct correctness requirements: (1) If the precondition is false,
    //   the program must stop — unreachable tells LLVM the false path is dead
    //   and enables fold elimination. (2) If the precondition is true, LLVM must
    //   know the bound so it can optimize subsequent loads/stores. The
    //   br...unreachable establishes the contract violation as UB (LLVM can DCE
    //   the entire tick). The assumption/range gives the optimizer a known bound.
    //
    // WHY !range metadata instead of @llvm.assume:
    //   @llvm.assume is a control-flow barrier — it prevents LLVM from moving
    //   instructions across it, which blocks GVN and LICM. !range metadata on a
    //   load has no such barrier: GVN eliminates the redundant load, LLVM's
    //   ValueTracking propagates the range, and passes like LICM are not blocked.
    //   But !range only works for the pattern "x = load before check; check x < N;
    //   use x" — you need a load to attach the metadata to, hence the re-load.
    //
    // WHY only Expr::Lt(x, Integer(N)) gets the fast path:
    //   !range metadata natively expresses "value in [lo, hi)" — a single
    //   lt-against-constant maps directly. Any other pattern (le, gt, ge,
    //   compound and/or) cannot be represented as a single !range entry.
    //   Complex patterns fall back to @llvm.assume, which is correct (the
    //   optimizer still gets the info) but slightly slower (barrier cost).
    pub(super) fn emit_precondition_check(&mut self, out: &mut String, pre: &Expr, indent: &str) {
        let cond = self.emit_expr(out, pre, indent);
        let i1 = format!("%pi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        if cond.ty == Type::bool_() {
            // 2026-07-14: bool is i8 — trunc to i1 for br/assume
            writeln!(out, "{}{} = trunc i8 {} to i1", indent, i1, cond).ok();
        } else {
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();
        }
        let panic_l = format!("pp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        let safe_l = format!("ps{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, safe_l, panic_l).ok();
        writeln!(out, "{}{}:", indent, panic_l).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, safe_l).ok();
        // Replace @llvm.assume with !range metadata for simple patterns.
        // Pattern: [x < N] on a state field known to ssa_old_int_regs.
        // We emit a re-load of x with !range { 0, N } — the extra load is
        // GVN-eliminated if the bound is already provable by LLVM.
        match pre {
            Expr::BinaryOp(BinaryOpKind::Lt, lhs, rhs) if matches!(rhs.as_ref(), Expr::Decimal(_)) => {
                // 2026-07-04: Unwrap Cast(Identifier, Int) for Ptr<T> fields.
                // Ptr<T> fields use "ptr_field as Int" in precondition contracts
                // (e.g., [ptr as Int >= BASE && ptr as Int < END]). The Cast
                // wrapper must be unwrapped to find the state field name.
                let field_name = match lhs.as_ref() {
                    // 2026-07-31: Phase 3 (§8.4) — cast-to-Int via the canonical
                    // Type::int() primitive instead of the type-name string.
                    Expr::Cast(inner, t) if *t == Type::int() => match inner.as_ref() {
                        Expr::Identifier(n) => Some(n.clone()),
                        _ => None,
                    },
                    Expr::Identifier(name) => Some(name.clone()),
                    _ => None,
                };
                if let Some(ref name) = field_name {
                    if let Some(&idx) = self.ctx.field_index_map.get(name.as_str()) {
                        let bound = if let Expr::Decimal(b) = rhs.as_ref() { *b } else { 0 };
                        let gep = format!("%prg{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        let ty = self.ctx.field_types[idx].clone();
                        let idx_val = idx;
                        let gep = self.emit_state_gep(out, indent, "prg", "%state", idx_val);
                        let rl = format!("%prl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        let tn = crate::backend::llvm::tbaa_node(&ty, self.ctx.type_universe.as_ref());
                        writeln!(out, "{}{} = load {}, ptr {}, align {}, !tbaa !{}, !range !{{i64 {}, i64 {}}}",
                            indent, rl, ty, gep, self.align_of(&ty), tn, 0i64, bound).ok();
                    } else {
                        writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
                    }
                } else {
                    writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
                }
            }
            _ => {
                writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
            }
        }
    }

    //
    // WHY preconditions are extracted into separate @pre_* functions:
    //   The fast-path registry (try_projection_fast_path) and the main dispatch
    //   loop both need to check the same precondition before deciding whether to
    //   fire a txn. Extracting it into @pre_*(ptr) avoids duplicating the
    //   check IR across 7+ dispatch paths (folded, SSA, reactor, parallel, etc.).
    //   LLVM will inline @pre_* into its single caller (alwaysinline), so there
    //   is zero runtime cost — the extraction is purely an IR-size optimization
    //   during codegen, not a runtime abstraction.
    //
    // WHY ptr noalias nocapture on ptr:
    //   noalias tells LLVM that no other pointer aliases %state during @pre_*'s
    //   execution — enables load/store reordering and redundant load elimination
    //   across the call boundary. nocapture means @pre_* does not store %state
    //   in a global or return it, which lets LLVM's -mem2reg promote stack
    //   allocas that would otherwise escape to the @pre_* call.
    pub(super) fn emit_pre_function(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        if matches!(txn.contract.pre_condition, Expr::Bool(true)) { return; }
        // 2026-07-04: Use #7 (memory(readonly)) for @pre_* functions.
        // Precondition expressions never write to %State — they only read
        // state fields via GEP+load. readonly tells LLVM the function has
        // no memory writes, enabling CSE of redundant pre_ calls and load
        // hoisting past precondition checks.
        // Other paths: #0 for definitions and callable txns (they may read
        // and write through %state), #2 for reactor_tick (always writes
        // the state copy), #3 for @main (writes through reactor tick loop).
        // 2026-07-04: Use #10 (argmemonly + readonly) for @pre_*.
        // Precondition functions never write to %State and never access
        // @link trigger globals. argmemonly + readonly is the tightest
        // constraint — tells LLVM the function only reads memory through
        // its pointer arguments.
        writeln!(out, "define internal i8 @pre_{}({}) #10 {{", name, self.ctx.state_ptr_param).ok();
        writeln!(out, "  entry:").ok();
        self.fun.txn_counter = 0;
        self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
        // 2026-06-27: Clear ssa_old int/float regs so identifier lookups fall
        // through to GEP+load from %state. Without this, stale entries from a
        // prior emit (main function) produce forward references to registers
        // not defined in this function (precompute_sum: %t28 undefined).
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        if cond.ty == Type::bool_() {
            writeln!(out, "  ret i8 {}", cond).ok();
        } else {
            let i1 = format!("%ri{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            let i8_reg = format!("%r8{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            writeln!(out, "  {} = zext i1 {} to i8", i8_reg, i1).ok();
            writeln!(out, "  ret i8 {}", i8_reg).ok();
        }
        writeln!(out, "}}").ok();

        // Collect GPU kernel for callable txns with #gpu directives.
        if self.ctx.gpu_offload || txn.modifiers.iter().any(|m| m.name == "gpu") {
            let is_speculative = txn.modifiers.iter()
                .any(|m| m.name == "gpu" && m.name.starts_with('?'));
            self.collect_gpu_kernel(name, &txn.body, is_speculative);
        }
    }

    //
    // WHY async bodies have their own function with noalias nocapture ptr:
    //   Async dispatch spawns concurrent evaluation of multiple txns. Each async
    //   task operates on the same ptr but with thread-level interleaving
    //   guarantees (barriers between reads and writes). Giving each async body
    //   its own LLVM function with noalias nocapture per-task allows ThreadSanitizer
    //   and LLVM's alias analysis to reason about independent regions within %State.
    //   Without the separate function, the inlined barrier call sites would appear
    //   as unstructured control flow that LLVM cannot analyze for race conditions.
    //
    // WHY the pre-check + body structure mirrors the sequential path:
    //   Async txns have the same contract semantics as sequential ones — the
    //   precondition must be checked (contracts are not a "correctness tax"),
    //   and the body must execute atomically with respect to the pre-check.
    //   Mirroring the sequential emit ensures identical semantics under both
    //   dispatch strategies. The only difference is the function boundary, which
    //   enables the async runtime to call each body independently.
    pub(super) fn emit_async_body(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        let async_name = format!("async_body_{}", name);
        let async_attr = "#0".to_string();
        writeln!(out, "define void @{}({}) local_unnamed_addr {} {{", async_name, self.ctx.state_ptr_param, async_attr).ok();
        writeln!(out, "  entry:").ok();
        self.fun.txn_counter = 0;
        self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
        // 2026-06-27: Clear ssa_old int/float regs so identifier lookups fall
        // through to GEP+load from %state (same rationale as emit_pre_function).
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = if cond.ty == Type::bool_() {
            let b = format!("%ric{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = trunc i8 {} to i1", b, cond).ok();
            b
        } else {
            let i1 = format!("%ri{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            i1
        };
        let txn_fire_l = format!("txn_fire_{}", self.fun.txn_counter + 1);
        writeln!(out, "  br i1 {}, label %{}, label %{}_done", i1, txn_fire_l, async_name).ok();
        writeln!(out, "{}:", txn_fire_l).ok();
        self.fun.terminated = false;
        self.fun.returns_i64 = false;
            self.fun.fn_ret_ty = "void".to_string();
        for s in &txn.body {
            if self.fun.terminated { break; }
            emit_statement(self, out, s, "  ");
        }
        // 2026-07-19: term; sets terminated=true but emits no terminator —
        // always branch to the done label so the block is not left dangling.
        // If the body already emitted a ret (not typical for async void fn),
        // this br is dead code after a terminator (harmless).
        writeln!(out, "  br label %{}_done", async_name).ok();
        writeln!(out, "{}_done:", async_name).ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    //
    // WHY bodies are concatenated (not analyzed for dependencies):
    //   Fused txns come from the fusion pass, which has already proven that txn A
    //   and txn B have non-conflicting read/write sets (their union is conflict-
    //   free). Concatenation is correct because the fusion analysis already did the
    //   dependency work — re-analyzing at emit time would duplicate that analysis
    //   and risk desynchronizing with the fusion pass. The fusion pass guarantees
    //   no false dependencies between A's state mutations and B's, so straight-line
    //   concatenation is safe.
    //
    // WHY terminators (term/term!/escape) are filtered out:
    //   In a fused pair, A's terminator would prevent B from executing. The fusion
    //   analysis verified that A cannot terminate before B runs (both preconditions
    //   must be satisfied simultaneously), so A's terminator is dead code in the
    //   fused context. Filtering it avoids emitting unreachable IR that would
    //   confuse LLVM's control-flow analysis (specifically the structurizercfg pass).
    //
    // WHY a single ptr is shared:
    //   A and B operate on the same %State struct. Creating separate %State allocas
    //   would require merging them after both bodies execute, which would need
    //   explicit memcpy — defeating the purpose of fusion by doubling memory
    //   traffic. A single pointer is correct because the fusion pass guaranteed
    //   that A and B do not conflict on any state field.
    pub(super) fn emit_fused(&mut self, out: &mut String, a: &crate::ast::Transaction, b: &crate::ast::Transaction, name: &str) {
        let body_a: Vec<Statement> = a.body.iter()
            .filter(|s| !matches!(s, Statement::Term(..) | Statement::TermBang(..) | Statement::Escape(_)))
            .cloned().collect();
        let combined: Vec<Statement> = body_a.into_iter().chain(b.body.iter().cloned()).collect();
        let fused_attr = "#0";
        writeln!(out, "define void @{}(ptr noalias nocapture align 8 %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.fun.txn_counter = 0; self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear(); self.fun.terminated = false; self.fun.returns_i64 = false;
            self.fun.fn_ret_ty = "void".to_string();
        for s in &combined {
            if self.fun.terminated { break; }
            emit_statement(self, out, s, "  ");
        }
        if !self.fun.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_fused_composed(&mut self, out: &mut String, body: &[Statement], name: &str) {
        let fused_attr = "#0";
        writeln!(out, "define void @{}({}) local_unnamed_addr {} {{", name, self.ctx.state_ptr_param, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.fun.txn_counter = 0; self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.let_original_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear(); self.fun.terminated = false; self.fun.returns_i64 = false;
            self.fun.fn_ret_ty = "void".to_string();
        for s in body {
            if self.fun.terminated { break; }
            emit_statement(self, out, s, "  ");
        }
        if !self.fun.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }
}

/// Map a Brief signal name (e.g. "SIGWINCH", "SIGINT") to its POSIX number.
fn sig_number(name: &str) -> i32 {
    match name {
        "SIGHUP" => 1,
        "SIGINT" => 2,
        "SIGQUIT" => 3,
        "SIGILL" => 4,
        "SIGTRAP" => 5,
        "SIGABRT" => 6,
        "SIGBUS" => 7,
        "SIGFPE" => 8,
        "SIGKILL" => 9,
        "SIGUSR1" => 10,
        "SIGSEGV" => 11,
        "SIGUSR2" => 12,
        "SIGPIPE" => 13,
        "SIGALRM" => 14,
        "SIGTERM" => 15,
        "SIGSTKFLT" => 16,
        "SIGCHLD" => 17,
        "SIGCONT" => 18,
        "SIGSTOP" => 19,
        "SIGTSTP" => 20,
        "SIGTTIN" => 21,
        "SIGTTOU" => 22,
        "SIGURG" => 23,
        "SIGXCPU" => 24,
        "SIGXFSZ" => 25,
        "SIGVTALRM" => 26,
        "SIGPROF" => 27,
        "SIGWINCH" => 28,
        "SIGIO" => 29,
        "SIGPWR" => 30,
        "SIGSYS" => 31,
        _ => 0,
    }
}

impl LlvmBackend {
    /// Emit a library shim — no main function, only `__brief_init_state`
    /// and dso_local wrappers for #export functions.
    /// 2026-07-19: Emit wrappers for exported functions in shared library.
    /// In --shared mode, exported functions keep their original names (e.g., @add)
    /// with dso_local visibility. The C caller passes a %State pointer as the first
    /// argument. This is the simplest ABI; future work may add a %State-less wrapper.
    pub(super) fn emit_shared_lib_exports(&mut self, out: &mut String, items: &[TopLevel]) {
        for item in items {
            if let TopLevel::Export(e) = item {
                if let TopLevel::Definition(d) = e.inner.as_ref() {
                    // The function is already emitted by emit_definition with its
                    // original name. In --shared mode, it has dso_local visibility
                    // (set by emit_definition). No additional wrapper needed.
                    // Future: emit a wrapper that allocates %State on the caller's
                    // behalf, avoiding the need for the host to pass a state pointer.
                }
            }
        }
        writeln!(out, "define dso_local void @__brief_init_state({}) #0 {{", self.ctx.state_ptr_param).ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "define void @__brief_init() #0 {{").ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "define void @__brief_fini() #0 {{").ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "@llvm.global_ctors = appending global [1 x {{ i32, ptr, ptr }}] [{{ i32, ptr, ptr }} {{ i32 65535, ptr @__brief_init, ptr null }}]").ok();
    }

    /// Called when `self.ctx.library_mode` is true.
    pub(super) fn emit_library_shim(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        // The #export wrappers are already emitted by emit_definition (called
        // earlier in generate()). We only need to add __brief_init_state.
        // __brief_init_state — allocates %State, calls init_state, returns ptr
        writeln!(out, "define dso_local i64 @__brief_init_state() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        writeln!(out, "  %ptr = ptrtoint ptr %state to i64").ok();
        writeln!(out, "  ret i64 %ptr").ok();
        writeln!(out, "}}").ok();
        // Also emit a __glue_release placeholder (no-op for arena-free bridge)
        writeln!(out, "define dso_local void @__glue_release(i64 %frame_tag) local_unnamed_addr #0 {{").ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    /// Emit a standalone `@cell_persistent_ticks(ptr %state)` function that runs
    /// one convergence pass on each registered persistent cell. Called from the
    /// main reactor loop after `@reactor_tick`.
    pub(super) fn emit_persistent_cell_ticks(&mut self, out: &mut String) {
        // 2026-06-28: Clear let_bindings at function entry. Stale bindings from
        // the convergence loop (loop_engine.rs) leak across function boundaries
        // because let_bindings is a shared HashMap. This causes emit_expr to
        // return registers that were defined in a different basic block, producing
        // SSA dominance violations ("Instruction does not dominate all uses").
        self.fun.let_bindings.clear();
        self.fun.let_binding_types.clear();
        let names: Vec<String> = self.ctx.cell_defs.iter()
            .filter(|(_, c)| c.is_persistent)
            .map(|(name, _)| name.clone())
            .collect();
        if names.is_empty() { return; }

        // Emit a standalone @cell_persistent_ticks(ptr %state) that runs one
        // convergence pass on each registered persistent cell. This is called
        // from @reactor_tick (see dispatch.rs) so it fires every main loop
        // iteration regardless of which dispatch path the program takes.
        //
        // Why a separate function instead of inlining in each dispatch path?
        // The LLVM backend has 7+ dispatch strategies (folded, SSA, reactor,
        // parallel, etc.). A single tick function avoids duplicating the cell
        // evaluation logic across all of them. The function takes ptr so
        // it reads/writes the same %State struct as the rest of the program,
        // using the cell$name$field prefixed slots registered in
        // build_field_index.
        writeln!(out, "define void @cell_persistent_ticks({}) local_unnamed_addr #2 {{", self.ctx.state_ptr_param).ok();
        writeln!(out, "  entry:").ok();

        let prev_state = self.fun.state_reg_name.clone();
        self.fun.state_reg_name = "%state".to_string();

        for name in &names {
            let cell = self.ctx.cell_defs.get(name).unwrap().clone();

            // Evaluate internal triggers before running transactions
            for trg in &cell.internal_triggers {
                let trg_key = format!("cell${}${}", name, trg.name);
                let trg_idx_opt = self.ctx.field_index_map.get(&trg_key).copied();
                if let Some(trg_idx) = trg_idx_opt {
                    let trg_ll_ty = self.ctx.field_types[trg_idx].clone();
                    // 2026-07-14: Trigger LinkRef removed; use TtyReadKey# for all trigger reads
                    let read_expr = crate::ast::Expr::Call("TtyReadKey#".to_string(), vec![], None);
                    let result = self.emit_expr(out, &read_expr, "  ");
                    let conv = format!("%cit_{}_{}", self.fun.txn_counter, trg.name);
                    self.fun.txn_counter += 1;
                    // tty_read_key returns i64; trunc to match the state slot's type
                    let ll_storage_ty = &trg_ll_ty;
                    if ll_storage_ty == "i8" {
                        writeln!(out, "  {} = trunc i64 {} to i8", conv, result.name).ok();
                    } else {
                        // i32 for Char, i64 for Int, etc.
                        writeln!(out, "  {} = trunc i64 {} to {}", conv, result.name, ll_storage_ty).ok();
                    }
                    let gep = format!("%cit_gep_{}_{}", self.fun.txn_counter, trg.name);
                    self.fun.txn_counter += 1;
                    writeln!(out, "  {} = getelementptr %State, ptr %state, i32 0, i32 {}",
                        gep, trg_idx).ok();
                    writeln!(out, "  store {} {}, ptr {}, align 1", trg_ll_ty, conv, gep).ok();
                }
            }

            for txn in &cell.transactions {
                let pre = Self::rewrite_cell_identifiers(
                    &txn.contract.pre_condition, name);
                let cond = self.emit_expr(out, &pre, "  ");

                let fire_l = format!(".cpt_{}_{}", name, txn.name);
                let skip_l = format!(".cpt_{}_{}_s", name, txn.name);

                let cond_i1 = {
                    let r = format!("%cpct_{}_{}", name, txn.name);
                    self.fun.txn_counter += 1;
                    if cond.ty == Type::bool_() {
                        let t = format!("%cpct_{}_{}_t", name, txn.name);
                        writeln!(out, "  {} = trunc i8 {} to i1", t, cond.name).ok();
                        writeln!(out, "  {} = and i1 {}, true", r, t).ok();
                    } else {
                        writeln!(out, "  {} = icmp ne i64 {}, 0", r, cond.name).ok();
                    }
                    r
                };
                writeln!(out, "  br i1 {}, label %{}, label %{}",
                    cond_i1, fire_l, skip_l).ok();
                writeln!(out, "{}:", fire_l).ok();

                for stmt in &txn.body {
                    let rewritten = Self::rewrite_cell_stmt_identifiers(stmt, name);
                    emit_statement(self, out, &rewritten, "  ");
                }

                writeln!(out, "  br label %{}", skip_l).ok();
                writeln!(out, "{}:", skip_l).ok();
            }
            // Phase 4: propagate cell-to-cell wires after convergence
            // Only propagate when the source cell is NOT threaded (threaded cells
            // store outputs to channel globals; propagation from those requires
            // a separate channel-read path in the main loop).
            let is_threaded = self.cell_thread_names.contains(name);
            if !is_threaded {
                for (from_cell, from_port, to_cell, to_param) in &self.ctx.cell_wires.clone() {
                    if from_cell != name { continue; }
                    let src_prefixed = format!("cell${}${}", from_cell, from_port);
                    let dst_prefixed = format!("cell${}${}", to_cell, to_param);
                    if let Some(&src_idx) = self.ctx.field_index_map.get(&src_prefixed) {
                        if let Some(&dst_idx) = self.ctx.field_index_map.get(&dst_prefixed) {
                            let src_ll_ty = &self.ctx.field_types[src_idx];
                            let dst_ll_ty = &self.ctx.field_types[dst_idx];
                            let src_gep = format!("%cpw_src_{}_{}", self.fun.txn_counter, from_cell);
                            let dst_gep = format!("%cpw_dst_{}_{}", self.fun.txn_counter, from_cell);
                            let src_val = format!("%cpw_val_{}_{}", self.fun.txn_counter, from_cell);
                            self.fun.txn_counter += 1;
                            writeln!(out, "  {} = getelementptr %State, ptr %state, i32 0, i32 {}",
                                src_gep, src_idx).ok();
                            writeln!(out, "  {} = getelementptr %State, ptr %state, i32 0, i32 {}",
                                dst_gep, dst_idx).ok();
                            writeln!(out, "  {} = load {}, ptr {}, align 8", src_val, src_ll_ty, src_gep).ok();
                            writeln!(out, "  store {} {}, ptr {}, align 8", dst_ll_ty, src_val, dst_gep).ok();
                        }
                }
            }
            // Sync cell output ports to parent trigger bindings
            for (trg_name, cell_name, port_name) in &self.ctx.cell_trigger_bindings.clone() {
                if cell_name != name { continue; }
                let src_key = format!("cell${}${}", cell_name, port_name);
                let dst_key = trg_name.clone();
                if let Some(&src_idx) = self.ctx.field_index_map.get(&src_key) {
                    if let Some(&dst_idx) = self.ctx.field_index_map.get(&dst_key) {
                        let src_ll_ty = &self.ctx.field_types[src_idx];
                        let dst_ll_ty = &self.ctx.field_types[dst_idx];
                        let src_gep = format!("%cos_src_{}_{}", self.fun.txn_counter, cell_name);
                        let dst_gep = format!("%cos_dst_{}_{}", self.fun.txn_counter, cell_name);
                        let src_val = format!("%cos_val_{}_{}", self.fun.txn_counter, cell_name);
                        self.fun.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr %State, ptr %state, i32 0, i32 {}",
                            src_gep, src_idx).ok();
                        writeln!(out, "  {} = getelementptr %State, ptr %state, i32 0, i32 {}",
                            dst_gep, dst_idx).ok();
                        writeln!(out, "  {} = load {}, ptr {}, align 8", src_val, src_ll_ty, src_gep).ok();
                        writeln!(out, "  store {} {}, ptr {}, align 8", dst_ll_ty, src_val, dst_gep).ok();
                    }
                }
            }

            // Phase 4 (threaded): propagate wires from THREADED source cells.
            // Threaded cells store outputs to atomic channel globals. We read
            // those globals here and store the value into the target cell's
            // state slot. This runs after all cell convergence passes.
            for (from_cell, from_port, to_cell, to_param) in &self.ctx.cell_wires.clone() {
                if !self.cell_thread_names.contains(from_cell) { continue; }
                let dst_prefixed = format!("cell${}${}", to_cell, to_param);
                if let Some(&dst_idx) = self.ctx.field_index_map.get(&dst_prefixed) {
                    let dst_ll_ty = &self.ctx.field_types[dst_idx];
                    let ch_val = format!("%ctw_val_{}_{}", self.fun.txn_counter, from_cell);
                    let ch_gep = format!("%ctw_ch_{}_{}", self.fun.txn_counter, from_cell);
                    let dst_gep = format!("%ctw_dst_{}_{}", self.fun.txn_counter, from_cell);
                    self.fun.txn_counter += 1;
                    // Volatile load from channel global
                    writeln!(out, "  {} = load volatile {}, ptr @chan_val_{}_{}, align 8",
                        ch_val, dst_ll_ty, from_cell, from_port).ok();
                    writeln!(out, "  {} = getelementptr %State, ptr %state, i32 0, i32 {}",
                        dst_gep, dst_idx).ok();
                    writeln!(out, "  store {} {}, ptr {}, align 8", dst_ll_ty, ch_val, dst_gep).ok();
                }
            }
        }

        }

        self.fun.state_reg_name = prev_state;
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a thread function for a persistent cell that loops with nanosleep.
    /// The function takes a ptr to a %CellState struct, runs convergence,
    /// and stores outputs to atomic channel globals.
    pub(super) fn emit_cell_thread(&mut self, out: &mut String, cell: &crate::ast::CellDef) {
        let cell_name = &cell.name;
        // Thread function receives a private %CellState.<name>* allocated by the caller
        // in emit_main. We temporarily replace field_index_map and field_types with
        // the cell-local versions so all GEP loads/stores resolve against the
        // %CellState.<name> type instead of %State.
        let saved_imap = self.ctx.field_index_map.clone();
        let saved_types = self.ctx.field_types.clone();
        let saved_state_reg = self.fun.state_reg_name.clone();

        if let Some((cs_imap, cs_tys)) = self.ctx.cell_state_types.get(cell_name) {
            self.ctx.field_index_map = cs_imap.clone();
            self.ctx.field_types = cs_tys.clone();
        }
        writeln!(out, "define ptr @cell_thread_{}(ptr %state) local_unnamed_addr #0 {{", cell_name).ok();
        writeln!(out, "  entry:").ok();
        self.fun.state_reg_name = "%state".to_string();

        let tick_ns = 1_000_000; // 1kHz default
        writeln!(out, "  %ts_sec = alloca i64, align 8").ok();
        writeln!(out, "  %ts_nsec = alloca i64, align 8").ok();
        writeln!(out, "  store i64 0, ptr %ts_sec").ok();
        writeln!(out, "  store i64 {}, ptr %ts_nsec", tick_ns).ok();

        writeln!(out, "  br label %loop").ok();
        writeln!(out, "loop:").ok();

        // nanosleep for tick interval
        writeln!(out, "  call i32 @nanosleep(ptr %ts_sec, ptr null)").ok();

        // Run convergence on cell state
        for txn in &cell.transactions {
            let pre = Self::rewrite_cell_identifiers(&txn.contract.pre_condition, cell_name);
            let cond = self.emit_expr(out, &pre, "  ");
            let fire_l = format!(".ct_{}_{}", cell_name, txn.name);
            let skip_l = format!(".ct_{}_{}_s", cell_name, txn.name);
            let cond_i1 = {
                let r = format!("%cti_{}_{}", cell_name, txn.name);
                self.fun.txn_counter += 1;
                if cond.ty == Type::bool_() {
                    writeln!(out, "  {} = and i1 {}, true", r, cond.name).ok();
                } else {
                    writeln!(out, "  {} = icmp ne i64 {}, 0", r, cond.name).ok();
                }
                r
            };
            writeln!(out, "  br i1 {}, label %{}, label %{}", cond_i1, fire_l, skip_l).ok();
            writeln!(out, "{}:", fire_l).ok();
            for stmt in &txn.body {
                let rewritten = Self::rewrite_cell_stmt_identifiers(stmt, cell_name);
                emit_statement(self, out, &rewritten, "  ");
            }
            writeln!(out, "  br label %{}", skip_l).ok();
            writeln!(out, "{}:", skip_l).ok();
        }

        // Atomic store outputs to channel globals
        // Use %CellState.<name> type for GEPs since cell fields are in the cell state
        let cell_state_type = format!("%CellState.{}", cell_name);
        let output_names = Self::extract_output_names_llvm(&cell.output_type);
        for port_name in &output_names {
            let prefixed = format!("cell${}${}", cell_name, port_name);
            if let Some(&idx) = self.ctx.field_index_map.get(&prefixed) {
                let ll_ty = &self.ctx.field_types[idx];
                let gep = format!("%ctg_{}_{}", cell_name, port_name);
                writeln!(out, "  {} = getelementptr {}, ptr {}, i32 0, i32 {}", gep, cell_state_type, self.fun.state_reg_name, idx).ok();
                let val = format!("%ctv_{}_{}", cell_name, port_name);
                writeln!(out, "  {} = load {}, ptr {}, align 8", val, ll_ty, gep).ok();
                writeln!(out, "  store atomic {} {}, ptr @chan_val_{}_{} seq_cst, align 8", ll_ty, val, cell_name, port_name).ok();
            }
        }
        // Set dirty flag
        writeln!(out, "  store atomic i8 1, ptr @chan_dirty_{} seq_cst, align 1", cell_name).ok();

        self.fun.state_reg_name = saved_state_reg;
        self.ctx.field_index_map = saved_imap;
        self.ctx.field_types = saved_types;
        writeln!(out, "  br label %loop").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit channel globals for a persistent cell's output ports.
    pub(super) fn emit_cell_channel_globals(&mut self, out: &mut String, cell: &crate::ast::CellDef) {
        let cell_name = &cell.name;
        let output_names = Self::extract_output_names_llvm(&cell.output_type);
        // For persistent cells, look up field types in cell_state_types
        if let Some((cs_imap, cs_tys)) = self.ctx.cell_state_types.get(cell_name) {
            for port_name in &output_names {
                let prefixed = format!("cell${}${}", cell_name, port_name);
                if let Some(&idx) = cs_imap.get(&prefixed) {
                    let ll_ty = &cs_tys[idx];
                    let init = if ll_ty.contains('*') { "null" } else { "0" };
                    writeln!(out, "@chan_val_{}_{} = global {} {}, align 8", cell_name, port_name, ll_ty, init).ok();
                }
            }
        } else {
            // Fall back to field_index_map for non-persistent cells (shouldn't happen)
            for port_name in &output_names {
                let prefixed = format!("cell${}${}", cell_name, port_name);
                if let Some(&idx) = self.ctx.field_index_map.get(&prefixed) {
                    let ll_ty = &self.ctx.field_types[idx];
                    let init = if ll_ty.contains('*') { "null" } else { "0" };
                    writeln!(out, "@chan_val_{}_{} = global {} {}, align 8", cell_name, port_name, ll_ty, init).ok();
                }
            }
        }
        writeln!(out, "@chan_dirty_{} = global i8 0, align 1", cell_name).ok();
    }

    // ── AsmFn Emission ──────────────────────────────────────────────

    /// 2026-07-29: Emit an LLVM function from an AsmFn declaration.
    /// Each instruction in the body becomes a `call asm sideeffect` call.
    /// The function signature matches the declared params and return type.
    /// Registers are allocated by LLVM's register allocator (use "=r"/"r" constraints).
    pub(super) fn emit_asm_fn(&mut self, out: &mut String, af: &crate::ast::AsmFn) {
        // Determine LLVM return type
        let ll_ret = self.llvm_type(&af.ret_type);
        let ll_name = &af.name;

        // Emit the function header
        writeln!(out, "define {} @{}(", ll_ret, ll_name).ok();
        for (i, (_, t)) in af.params.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #8 {{").ok();
        writeln!(out, "  entry:").ok();

        if af.body.is_empty() {
            // No instructions — return 0
            writeln!(out, "  ret {} 0", ll_ret).ok();
            writeln!(out, "}}").ok();
            return;
        }

        // Build constraint string and operand list for each instruction.
        // Each instruction uses "=r" for result and "r" for each param.
        let num_operands = af.params.len();
        let result_operand = if af.ret_type != crate::ast::Type::Void { 1 } else { 0 };

        for (ins_idx, instruction) in af.body.iter().enumerate() {
            // Substitute {param} → $N and {result} → $0
            let mut asm_text = instruction.clone();
            // Replace {result} with $0 (output operand)
            if result_operand > 0 {
                asm_text = asm_text.replace("{result}", "$0");
            }
            // Replace {param_name} with $N+1 (input operand)
            let mut offset = result_operand;
            for (p_idx, (p_name, _)) in af.params.iter().enumerate() {
                let placeholder = format!("{{{}}}", p_name);
                asm_text = asm_text.replace(&placeholder, &format!("${}", offset));
                offset += 1;
            }

            let is_last = ins_idx == af.body.len() - 1;

            if result_operand > 0 {
                // Build constraint: "=r" for output, "r" for each input, then clobbers
                let mut constraint = "=r".to_string();
                for _ in &af.params {
                    constraint.push_str(",r");
                }
                if !af.params.is_empty() {
                    constraint.push(',');
                }
                constraint.push_str("~{dirflag},~{fpsr},~{flags}");

                let mut args = Vec::new();
                for (_, t) in &af.params {
                    args.push(format!("{} %arg{}", self.llvm_type(t), args.len()));
                }

                writeln!(out, "  %r{} = call {} asm \"{}\", \"{}\"({})",
                    ins_idx, ll_ret, asm_text, constraint, args.join(", ")).ok();

                if is_last {
                    writeln!(out, "  ret {} %r{}", ll_ret, ins_idx).ok();
                }
            } else {
                // No return value (void function or intermediate)
                let mut constr = String::new();
                for (i, (_, _)) in af.params.iter().enumerate() {
                    if i > 0 { constr.push(','); }
                    constr.push('r');
                }
                if !af.params.is_empty() {
                    constr.push(',');
                }
                constr.push_str("~{dirflag},~{fpsr},~{flags}");
                let args: Vec<String> = af.params.iter().enumerate()
                    .map(|(i, (_, t))| format!("{} %arg{}", self.llvm_type(t), i))
                    .collect();
                writeln!(out, "  call void asm sideeffect \"{}\", \"{}\"({})",
                    asm_text, constr, args.join(", ")).ok();
            }
        }

        // Close function if no ret was emitted
        if result_operand == 0 || af.body.len() > 1 {
            writeln!(out, "  ret {} 0", ll_ret).ok();
        }
        writeln!(out, "}}").ok();
    }
}
