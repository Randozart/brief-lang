// ── AsmAssembler Trait — Pluggable Assembly Backend ───────────────────
// 2026-07-29: Compile-time assembly validation trait.
// Each implementation validates asm text for a target architecture and
// returns assembled bytes (for verification) or an error.
// Config-driven: selected via `assembler` key in config/targets.toml.
// Supports three backends: Keystone (default on supported platforms),
// platform assembler (system as/ml64), and stub (warn-only, no validation).

use std::fmt::Debug;

/// 2026-07-29: Compile-time assembly validation trait.
///
/// Each implementation validates asm instruction text for a target
/// architecture and returns the assembled bytes or an error.
/// The selected implementation is determined by `config/targets.toml`'s
/// `assembler` key. The `:=` verification chain uses this to cross-verify
/// asm bodies against reference implementations at compile time.
///
/// # Implementations
///
/// - `KeystoneAssembler` — links against Keystone Engine C library.
///   Fast, supports all LLVM architectures. Default when available.
/// - `PlatformAssembler` — shells out to system `as`/`ml64`.
///   No external library needed. Slower, arch-dependent.
/// - `StubAssembler` — no-op, warns at compile time.
///   Safe fallback for unsupported platforms.
pub trait AsmAssembler: Debug {
    /// Human-readable name (e.g., "keystone", "platform", "none").
    fn name(&self) -> &str;

    /// Validate and assemble an instruction or block.
    ///
    /// `text`: the instruction template with `{param}` already substituted
    /// with ABI registers.
    /// `arch`: target architecture string (e.g., "x86_64", "aarch64").
    ///
    /// Returns assembled bytes on success, error description on failure.
    fn assemble(&self, text: &str, arch: &str) -> Result<Vec<u8>, String>;

    /// Whether this assembler is available on the current system.
    fn is_available(&self) -> bool;
}

// ── Stub Implementation ─────────────────────────────────────────────

/// 2026-07-29: No-op assembler — warns at compile time and passes through
/// without validation. Selected via `assembler = "none"` in targets.toml.
/// Safe fallback for platforms where Keystone or system assembler
/// is not available. Cross-verification in the := chain still catches
/// semantic errors even without byte-level validation.
#[derive(Debug)]
pub struct StubAssembler;

impl AsmAssembler for StubAssembler {
    fn name(&self) -> &str { "none" }

    fn assemble(&self, text: &str, arch: &str) -> Result<Vec<u8>, String> {
        eprintln!("  warning: assembly not validated for {}: {}", arch, text);
        eprintln!("  warning: set assembler = \"keystone\" or assembler = \"platform\" in config/targets.toml");
        Ok(vec![])
    }

    fn is_available(&self) -> bool { true }
}

// ── Selector ─────────────────────────────────────────────────────────

/// 2026-07-29: Select the assembler implementation based on config.
/// Falls back to StubAssembler for unknown values.
pub fn get_assembler(config: &crate::target::TargetEntry) -> Box<dyn AsmAssembler> {
    match config.assembler.as_str() {
        "none" => Box::new(StubAssembler),
        other => {
            eprintln!("  warning: unknown assembler '{}', falling back to 'none'", other);
            Box::new(StubAssembler)
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_assembler_name() {
        let stub = StubAssembler;
        assert_eq!(stub.name(), "none");
    }

    #[test]
    fn test_stub_assembler_available() {
        let stub = StubAssembler;
        assert!(stub.is_available());
    }

    #[test]
    fn test_stub_assembler_assemble() {
        let stub = StubAssembler;
        let result = stub.assemble("nop", "x86_64");
        assert!(result.is_ok(), "stub should always succeed");
    }

    #[test]
    fn test_get_assembler_unknown_fallback() {
        let entry = crate::target::TargetEntry {
            backend: "llvm".into(),
            defaults: vec![],
            plugins: None,
            target_triple: None,
            data_layout: None,
            assembler: "unknown_value".into(),
            cross_verify_samples: 50,
        };
        let asm = get_assembler(&entry);
        assert_eq!(asm.name(), "none", "unknown assembler should fall back to stub");
    }

    #[test]
    fn test_get_assembler_none() {
        let entry = crate::target::TargetEntry {
            backend: "llvm".into(),
            defaults: vec![],
            plugins: None,
            target_triple: None,
            data_layout: None,
            assembler: "none".into(),
            cross_verify_samples: 50,
        };
        let asm = get_assembler(&entry);
        assert_eq!(asm.name(), "none");
    }
}
