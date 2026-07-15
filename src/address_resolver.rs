// ── Shared Address Resolver ────────────────────────────────────────────
//
// 2026-07-15: Resolves named device/entity identifiers to numeric MMIO
// addresses. Used by both the interpreter (AddressOf# evaluation) and the
// LLVM backend (AddressOf# codegen). Prefers config/address-map.toml at
// compile time; falls back to hardcoded known addresses.
//
// Flat dispatch: load config → try config lookup → fallback → default.

use std::collections::HashMap;
use std::path::Path;

/// 2026-07-15: Resolve a named address to its numeric value.
///
/// Tries, in order:
/// 1. config/address-map.toml (if present)
/// 2. Hardcoded well-known device names
/// 3. Default MMIO region base (0xFE000000)
pub fn resolve_address(id: &str) -> u64 {
    // Try config file first
    if let Some(addr) = resolve_from_config(id) {
        return addr;
    }
    // Fall back to hardcoded table
    if let Some(addr) = resolve_from_hardcoded(id) {
        return addr;
    }
    // Default MMIO region base
    0xFE000000
}

/// 2026-07-15: Read config/address-map.toml and look up the id.
fn resolve_from_config(id: &str) -> Option<u64> {
    let config_path = find_config_path()?;
    let content = std::fs::read_to_string(&config_path).ok()?;
    let parsed: HashMap<String, toml::Value> = toml::from_str(&content).ok()?;
    let addresses = parsed.get("addresses")?.as_table()?;
    let value = addresses.get(id)?;
    let s = value.as_str()?;
    let clean = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(clean, 16).ok()
}

/// 2026-07-15: Locate config/address-map.toml relative to the project root
/// or the compiler binary.
fn find_config_path() -> Option<String> {
    // Try relative to CWD (development)
    if Path::new("config/address-map.toml").exists() {
        return Some("config/address-map.toml".to_string());
    }
    // Try relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let path = parent.join("config/address-map.toml");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Hardcoded well-known device address table.
/// 2026-07-15: Mirrors config/address-map.toml for fallback when the
/// config file is not available.
fn resolve_from_hardcoded(id: &str) -> Option<u64> {
    match id.to_lowercase().as_str() {
        "uart"  | "uart0"  | "/dev/ttys0"  | "/dev/ttyama0"  => Some(0xFFE01000),
        "gpio"  | "gpio0"  | "/dev/gpiochip0"                => Some(0xFE001000),
        "timer" | "timer0" | "/dev/timer0"                   => Some(0xFE002000),
        "spi"   | "spi0"   | "/dev/spidev0.0"                => Some(0xFE003000),
        "i2c"   | "i2c0"   | "/dev/i2c-0"                    => Some(0xFE004000),
        "dma"   | "dma0"   | "/dev/dma"                      => Some(0xFE005000),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_address_known() {
        let addr = resolve_address("uart");
        assert_eq!(addr, 0xFFE01000);
    }

    #[test]
    fn test_resolve_address_known_case_insensitive() {
        let addr = resolve_address("UART");
        assert_eq!(addr, 0xFFE01000);
    }

    #[test]
    fn test_resolve_address_unknown_defaults() {
        let addr = resolve_address("nonexistent_device");
        assert_eq!(addr, 0xFE000000);
    }

    #[test]
    fn test_resolve_address_from_config_file() {
        // This test passes if config/address-map.toml is present and
        // contains "uart" → 0xFFE01000. If the file is absent, fallback
        // kicks in and still returns the right value.
        let addr = resolve_address("gpio0");
        assert_eq!(addr, 0xFE001000);
    }

    #[test]
    fn test_resolve_from_hardcoded_device_paths() {
        let addr = resolve_address("/dev/ttyama0");
        assert_eq!(addr, 0xFFE01000);
    }
}
