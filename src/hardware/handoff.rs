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
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

//! Hardware Handoff — Extract peripheral addresses from Vivado .xsa or
//! xparameters.h and generate DBVS schema + DBV target binding files.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

/// A hardware peripheral extracted from a Vivado handoff file.
#[derive(Debug, Clone)]
pub struct HardwarePeripheral {
    pub name: String,
    pub base_address: u64,
    pub high_address: u64,
    pub interface_type: String,
}

impl HardwarePeripheral {
    pub fn size(&self) -> u64 {
        self.high_address.saturating_sub(self.base_address) + 1
    }

    fn from_parts(instance: &str, base: u64, high: u64, iface: &str) -> Self {
        HardwarePeripheral {
            name: HardwarePeripheral::clean_name(instance),
            base_address: base,
            high_address: high,
            interface_type: iface.to_string(),
        }
    }

    fn clean_name(raw: &str) -> String {
        raw.to_lowercase()
            .replace("xpar_", "")
            .replace("_baseaddr", "")
            .replace('-', "_")
            .replace('/', "_")
    }
}

// ── xparameters.h Extraction ──────────────────────────────────────

/// Extract base addresses from a Vivado-generated `xparameters.h` header.
///
/// Scans for `#define XPAR_*_BASEADDR 0x...` lines and returns a map
/// of cleaned peripheral name → `HardwarePeripheral`.
///
/// High address is conservatively set to `base + 0xFFF` (4K page) since
/// the header only provides base addresses.
pub fn extract_from_xparameters(content: &str) -> HashMap<String, HardwarePeripheral> {
    let mut peripherals = HashMap::new();

    let mut base_addrs: HashMap<String, u64> = HashMap::new();
    let mut high_addrs: HashMap<String, u64> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("#define XPAR_") && trimmed.contains("_BASEADDR") {
            let prefix = trimmed
                .strip_prefix("#define ")
                .unwrap_or("");
            let parts: Vec<&str> = prefix.split_whitespace().collect();
            if parts.len() >= 2 {
                let macro_name = parts[0];
                let addr_str = parts[1];
                let base_name = macro_name.replace("XPAR_", "").replace("_BASEADDR", "");
                if let Ok(addr) = parse_hex_or_dec(addr_str) {
                    base_addrs.insert(base_name.clone(), addr);
                    let clean = HardwarePeripheral::clean_name(&base_name);
                    if !peripherals.contains_key(&clean) {
                        peripherals.insert(
                            clean.clone(),
                            HardwarePeripheral {
                                name: clean,
                                base_address: addr,
                                high_address: addr + 0xFFF,
                                interface_type: "AXI4-Lite".to_string(),
                            },
                        );
                    }
                }
            }
        } else if trimmed.starts_with("#define XPAR_") && trimmed.contains("_HIGHADDR") {
            let prefix = trimmed
                .strip_prefix("#define ")
                .unwrap_or("");
            let parts: Vec<&str> = prefix.split_whitespace().collect();
            if parts.len() >= 2 {
                let macro_name = parts[0];
                let addr_str = parts[1];
                let base_name = macro_name.replace("XPAR_", "").replace("_HIGHADDR", "");
                if let Ok(addr) = parse_hex_or_dec(addr_str) {
                    high_addrs.insert(base_name.clone(), addr);
                    let clean = HardwarePeripheral::clean_name(&base_name);
                    if let Some(peripheral) = peripherals.get_mut(&clean) {
                        peripheral.high_address = addr;
                    }
                }
            }
        }
    }

    // Apply high addresses for peripherals we found BASEADDR for
    for (base_name, high) in &high_addrs {
        let clean = HardwarePeripheral::clean_name(base_name);
        if let Some(peripheral) = peripherals.get_mut(&clean) {
            peripheral.high_address = *high;
        }
    }

    peripherals
}

fn parse_hex_or_dec(s: &str) -> Result<u64, std::num::ParseIntError> {
    let s = s.trim_end_matches(';').trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        u64::from_str_radix(&s[2..], 16)
    } else {
        s.parse::<u64>()
    }
}

// ── .xsa Extraction ───────────────────────────────────────────────

/// Extract peripherals from a Vivado `.xsa` hardware handoff archive.
///
/// Opens the `.xsa` as a zip archive, reads `system.hwh`, and parses
/// `<MEMRANGE BASEVALUE="0x..." HIGHVALUE="0x..."/>` inside each
/// `<MODULE INSTANCE="...">` block.
pub fn extract_from_xsa(xsa_path: &Path) -> Result<HashMap<String, HardwarePeripheral>, String> {
    let file = fs::File::open(xsa_path)
        .map_err(|e| format!("Cannot open {}: {}", xsa_path.display(), e))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to open .xsa zip: {}. Is zip support available?", e))?;

    let mut hwh_content = String::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {}: {}", i, e))?;
        let name = file.name().to_string();
        if name.ends_with("system.hwh") || name == "system.hwh" {
            file.read_to_string(&mut hwh_content)
                .map_err(|e| format!("Failed to read system.hwh: {}", e))?;
            break;
        }
    }

    if hwh_content.is_empty() {
        return Err("No system.hwh found in .xsa archive".to_string());
    }

    extract_from_hwh_xml(&hwh_content)
}

/// Parse `system.hwh` XML for peripheral memory ranges.
///
/// Scans `<MODULE INSTANCE="name">` blocks for nested `<MEMRANGE>`
/// elements with BASEVALUE and HIGHVALUE attributes.
fn extract_from_hwh_xml(xml: &str) -> Result<HashMap<String, HardwarePeripheral>, String> {
    let mut peripherals = HashMap::new();
    let mut current_instance: Option<String> = None;
    let mut current_iface: Option<String> = None;

    for line in xml.lines() {
        let trimmed = line.trim();

        if let Some(instance) = extract_xml_attr(trimmed, "MODULE", "INSTANCE") {
            current_instance = Some(instance.to_string());
            current_iface = None;
        }

        if let Some(iface) = extract_xml_attr(trimmed, "BUSINTERFACE", "BUSNAME") {
            current_iface = Some(iface.to_string());
        }

        if let (Some(instance), _) = (&current_instance, &current_iface) {
            if let Some(base) = extract_xml_attr(trimmed, "MEMRANGE", "BASEVALUE") {
                if let Ok(base_addr) = parse_hex_or_dec(base) {
                    let default_high = format!("0x{:X}", base_addr + 0xFFF);
                    let high_str = extract_xml_attr(trimmed, "MEMRANGE", "HIGHVALUE")
                        .unwrap_or(&default_high);
                    let high_addr = parse_hex_or_dec(high_str).unwrap_or(base_addr + 0xFFF);
                    let p = HardwarePeripheral::from_parts(
                        instance,
                        base_addr,
                        high_addr,
                        current_iface.as_deref().unwrap_or("AXI4"),
                    );
                    peripherals.insert(p.name.clone(), p);
                }
            }
        }

        if trimmed.starts_with("</MODULE>") {
            current_instance = None;
            current_iface = None;
        }
    }

    if peripherals.is_empty() {
        return Err("No MODULE INSTANCE entries with MEMRANGE found in system.hwh".to_string());
    }

    Ok(peripherals)
}

fn extract_xml_attr<'a>(line: &'a str, element: &str, attr: &str) -> Option<&'a str> {
    let open = format!("<{} ", element);
    let rest = line.strip_prefix(&open)?;

    // Find the attribute: ATTR="value" or ATTR='value'
    let search = format!("{}=", attr);
    let pos = rest.find(&search)?;
    let after = &rest[pos + search.len()..];

    let quote = after.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let value_start = &after[1..];
    let end = value_start.find(quote)?;
    Some(&value_start[..end])
}

// ── DBVS Schema Generator ─────────────────────────────────────────

/// Generate a `.dbvs` schema from extracted hardware peripherals.
///
/// Each peripheral becomes a named register with an alias.
/// The user writes `import "chip.dbvs"` in their Brief program
/// and references peripherals by their cleaned name.
pub fn generate_dbvs(peripherals: &HashMap<String, HardwarePeripheral>) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated from Vivado hardware handoff\n");
    out.push_str("// Schema: peripheral registers and their types\n");
    out.push_str("// Compound with --target <board>.dbv to resolve addresses\n\n");

    let mut names: Vec<&String> = peripherals.keys().collect();
    names.sort();

    for name in names {
        let p = &peripherals[name];
        out.push_str(&format!(
            "register @{:#010X} as \"{}\" {{\n    type: UInt;\n    description: \"{} memory-mapped peripheral\";\n}};\n\n",
            p.base_address, p.name, p.interface_type
        ));
        out.push_str(&format!(
            "alias {}: UInt @{:#010X};\n\n",
            p.name, p.base_address
        ));
    }

    out
}

// ── DBV Target Generator ──────────────────────────────────────────

/// Generate a `.dbv` target binding from extracted hardware peripherals.
///
/// Maps each schema alias to its physical address for this specific target.
pub fn generate_dbv(
    peripherals: &HashMap<String, HardwarePeripheral>,
    target_name: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Auto-generated target binding for {}\n",
        target_name
    ));
    out.push_str("// Maps DBVS schema aliases to physical addresses\n");
    out.push_str(&format!(
        "// Generated from Vivado hardware handoff — {} peripherals\n\n",
        peripherals.len()
    ));

    let mut names: Vec<&String> = peripherals.keys().collect();
    names.sort();

    for name in names {
        let p = &peripherals[name];
        out.push_str(&format!(
            "binding {}: UInt @{:#010X};  // {} [{}..{}], size={}\n",
            p.name,
            p.base_address,
            p.interface_type,
            p.base_address,
            p.high_address,
            p.size()
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XPARAMETERS: &str = r#"
#define XPAR_AXI_GPIO_0_BASEADDR 0x8000A000
#define XPAR_AXI_GPIO_0_HIGHADDR 0x8000AFFF
#define XPAR_AXI_UART_1_BASEADDR 0x8000B000
#define XPAR_AXI_UART_1_HIGHADDR 0x8000BFFF
#define XPAR_AXI_DMA_0_BASEADDR 0x80004000
#define XPAR_UNRELATED_VALUE 42
"#;

    const SAMPLE_HWH: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<HWH VERSION="1.0">
  <MODULE INSTANCE="axi_gpio_0" MODTYPE="axi_gpio" VERSION="2.0">
    <BUSINTERFACES>
      <BUSINTERFACE BUSNAME="S_AXI" BUSSTANDARD="AXI4LITE" />
    </BUSINTERFACES>
    <MEMRANGES>
      <MEMRANGE BASEVALUE="0x8000A000" HIGHVALUE="0x8000AFFF" />
    </MEMRANGES>
  </MODULE>
  <MODULE INSTANCE="axi_uart_1" MODTYPE="axi_uartlite" VERSION="2.0">
    <BUSINTERFACES>
      <BUSINTERFACE BUSNAME="S_AXI" BUSSTANDARD="AXI4LITE" />
    </BUSINTERFACES>
    <MEMRANGES>
      <MEMRANGE BASEVALUE="0x8000B000" HIGHVALUE="0x8000BFFF" />
    </MEMRANGES>
  </MODULE>
</HWH>"#;

    #[test]
    fn test_extract_xparameters() {
        let peripherals = extract_from_xparameters(SAMPLE_XPARAMETERS);
        assert_eq!(peripherals.len(), 3);

        let gpio = peripherals.get("axi_gpio_0").unwrap();
        assert_eq!(gpio.base_address, 0x8000A000);
        assert_eq!(gpio.high_address, 0x8000AFFF);

        let dma = peripherals.get("axi_dma_0").unwrap();
        assert_eq!(dma.base_address, 0x80004000);
        assert_eq!(dma.high_address, 0x80004FFF); // default 4K page
    }

    #[test]
    fn test_extract_hwh_xml() {
        let peripherals = extract_from_hwh_xml(SAMPLE_HWH).unwrap();
        assert_eq!(peripherals.len(), 2);

        let gpio = peripherals.get("axi_gpio_0").unwrap();
        assert_eq!(gpio.base_address, 0x8000A000);
        assert_eq!(gpio.high_address, 0x8000AFFF);
        assert!(gpio.interface_type.contains("AXI"));

        let uart = peripherals.get("axi_uart_1").unwrap();
        assert_eq!(uart.base_address, 0x8000B000);
    }

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex_or_dec("0x8000A000").unwrap(), 0x8000A000);
        assert_eq!(parse_hex_or_dec("42").unwrap(), 42);
    }

    #[test]
    fn test_xml_attr_extraction() {
        let line = r#"<MEMRANGE BASEVALUE="0x8000A000" HIGHVALUE="0x8000AFFF"/>"#;
        assert_eq!(
            extract_xml_attr(line, "MEMRANGE", "BASEVALUE").unwrap(),
            "0x8000A000"
        );
        assert_eq!(
            extract_xml_attr(line, "MEMRANGE", "HIGHVALUE").unwrap(),
            "0x8000AFFF"
        );
    }

    #[test]
    fn test_generate_dbvs() {
        let peripherals = extract_from_hwh_xml(SAMPLE_HWH).unwrap();
        let dbvs = generate_dbvs(&peripherals);
        assert!(dbvs.contains("axi_gpio_0"));
        assert!(dbvs.contains("axi_uart_1"));
        assert!(dbvs.contains("register @"));
        assert!(dbvs.contains("alias "));
    }

    #[test]
    fn test_generate_dbv() {
        let peripherals = extract_from_hwh_xml(SAMPLE_HWH).unwrap();
        let dbv = generate_dbv(&peripherals, "zcu4ev");
        assert!(dbv.contains("zcu4ev"));
        assert!(dbv.contains("axi_gpio_0"));
        assert!(dbv.contains("axi_uart_1"));
        assert!(dbv.contains("binding "));
        assert!(dbv.contains("@0x8000A000"));
    }

    #[test]
    fn test_clean_name() {
        assert_eq!(HardwarePeripheral::clean_name("XPAR_AXI_GPIO_0"), "axi_gpio_0");
        assert_eq!(HardwarePeripheral::clean_name("axi_gpio_0"), "axi_gpio_0");
        assert_eq!(HardwarePeripheral::clean_name("my-ip/S_AXI"), "my_ip_s_axi");
    }
}
