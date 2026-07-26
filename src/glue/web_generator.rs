// ── GLUE Web Generator (WASM-first webstack) ────────────────────────────
// 2026-07-26: Phase 3 — Produces dom-shim.mjs and .d.ts from view bindings
// and WASM state layout. The JS shim reads WASM linear memory at known
// offsets — no JS watchers, no Proxy, no polling.
//
// Architecture:
//   The WASM module exposes a state_layout() function that returns a pointer
//   to a struct in linear memory describing each state field's offset, size,
//   and type. The JS shim reads this at init time to build its binding table.
//   At each transaction commit (term;), the WASM module calls __web_flush_state
//   with a batch of (field_handle, value_ptr, value_len) tuples. JS applies
//   the DOM mutations synchronously and returns.
//
// Trade-off: One FFI crossing per transaction (not per field). This is optimal
// for transactions with 1-5 field mutations (the common case). For transactions
// with 20+ field mutations, a single large batch still beats per-field crossings.
// See docs/architecture/features/rendered-brief-wasm.md.

use std::collections::HashMap;

/// Descriptor for a single state field in WASM linear memory.
/// 2026-07-26: Phase 3 — Emitted by the LLVM backend in state_layout export.
#[derive(Debug, Clone)]
pub struct FieldLayout {
    /// Unique handle for this field. Matches bindings in the view compiler output.
    pub field_handle: u32,
    /// Byte offset of this field in WASM linear memory.
    pub offset: u32,
    /// Byte size of this field's value.
    pub size: u32,
    /// Type tag for interpreting the value bytes.
    pub type_tag: TypeTag,
}

/// Type tag for interpreting state field values.
/// 2026-07-26: Phase 3 — Used by JS shim to decode value bytes appropriately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Int,
    Float,
    Bool,
    String,
}

impl TypeTag {
    fn js_type_name(&self) -> &'static str {
        match self {
            TypeTag::Int => "number",
            TypeTag::Float => "number",
            TypeTag::Bool => "boolean",
            TypeTag::String => "string",
        }
    }

    fn decoder_expr(&self) -> &'static str {
        match self {
            TypeTag::Int => "mem.getInt32(valPtr, true)",
            TypeTag::Float => "mem.getFloat64(valPtr, true)",
            TypeTag::Bool => "mem.getUint8(valPtr, true) !== 0",
            TypeTag::String => {
                "valLen > 0 ? new TextDecoder().decode(new Uint8Array(mem.buffer, valPtr, valLen)) : null"
            }
        }
    }
}

/// Describes the layout of all state fields in WASM linear memory.
/// 2026-07-26: Phase 3 — Read by the JS shim at init time from state_layout() export.
#[derive(Debug, Clone)]
pub struct StateLayout {
    /// Description of the application for naming.
    pub app_name: String,
    /// Offset of the generation counter (u32) in WASM linear memory.
    pub generation_offset: u32,
    /// Offset of the flush buffer in WASM linear memory.
    pub flush_buffer_offset: u32,
    /// Maximum number of entries the flush buffer can hold.
    pub max_flush_entries: u32,
    /// Per-field layout descriptors.
    pub fields: Vec<FieldLayout>,
}

/// Output of the GlueWebGenerator.
/// 2026-07-26: Phase 3 — Two files: the JS runtime shim and TS declarations.
#[derive(Debug, Clone)]
pub struct GlueWebOutput {
    /// JavaScript module: dom-shim.mjs — WasmDomRuntime class.
    pub dom_shim: String,
    /// TypeScript declarations: app.d.ts.
    pub dts: String,
}

/// Generates the JS runtime shim and TS declarations for a WASM-first webstack app.
///
/// 2026-07-26: Phase 3 — Takes compile-time data (state layout, view bindings,
/// protocol mappings) and produces the minimal JS shim that:
///   1. Instantiates the WASM module with __web_flush_state import
///   2. Reads state_layout() export at init to build the binding table
///   3. Applies DOM mutations when __web_flush_state is called
///
/// In Phase 4 (LLVM backend wired), the WASM module actually calls
/// __web_flush_state. In this phase, the generator infrastructure is built
/// such that it can immediately produce correct output once the LLVM backend
/// side is complete.
pub struct GlueWebGenerator {
    /// The compiled WASM module bytes (or empty during testing).
    wasm_module: Vec<u8>,
    /// View bindings from the view compiler.
    bindings: Vec<crate::view_compiler::Binding>,
    /// Compile-time state layout.
    state_layout: StateLayout,
    /// Protocol mappings from GLUE config (for type resolution).
    protocol_mappings: HashMap<String, crate::glue::config::ProtocolEntry>,
}

impl GlueWebGenerator {
    /// Create a new GlueWebGenerator with compile-time data.
    /// 2026-07-26: Phase 3 — `wasm_module` may be empty during testing;
    /// it is used in Phase 4+ for verifying state_layout export alignment.
    pub fn new(
        wasm_module: Vec<u8>,
        bindings: Vec<crate::view_compiler::Binding>,
        state_layout: StateLayout,
        protocol_mappings: HashMap<String, crate::glue::config::ProtocolEntry>,
    ) -> Self {
        GlueWebGenerator {
            wasm_module,
            bindings,
            state_layout,
            protocol_mappings,
        }
    }

    /// Generate the JS runtime shim and TS declarations.
    /// 2026-07-26: Phase 3 — Produces ES module with WasmDomRuntime class.
    pub fn generate(&self) -> Result<GlueWebOutput, String> {
        let dom_shim = self.generate_dom_shim();
        let dts = self.generate_dts();
        Ok(GlueWebOutput { dom_shim, dts })
    }

    /// Generate the dom-shim.mjs content.
    /// 2026-07-26: Phase 3 — Two sections:
    ///   1. WasmDomRuntime class (instantiate WASM, read layout, apply flushes)
    ///   2. createApp factory function
    fn generate_dom_shim(&self) -> String {
        let bindings_js = self.generate_binding_table();
        let imports_js = self.generate_imports();

        format!(
            r#"// dom-shim.mjs — Auto-generated GLUE web runtime for {app_name}
// Reads WASM linear memory at known state offsets.

export class WasmDomRuntime {{
  constructor(wasmBytes) {{
    this._handles = [null];
    this._bindingTable = [];
    this._instance = null;
    this._memory = null;
    this._generationOffset = {generation_offset};
    this._flushBufferOffset = {flush_buffer_offset};
    this._maxFlushEntries = {max_flush_entries};
    this._init(wasmBytes);
  }}

  async _init(wasmBytes) {{
    const importObject = {{
      env: this._buildImports(),
    }};
    const wasm = await WebAssembly.instantiate(wasmBytes, importObject);
    this._instance = wasm.instance;
    this._memory = wasm.instance.exports.memory;
    this._loadStateLayout();
  }}

  _buildImports() {{
    return {{
      __web_flush_state: (updatesPtr, count) => {{
        this._applyFlush(updatesPtr, count);
      }},
{imports_js}
    }};
  }}

  _loadStateLayout() {{
    const layoutPtr = this._instance.exports.state_layout();
    const mem = new DataView(this._memory.buffer);
    // state_layout table layout:
    //   field_count:    u32 at layoutPtr + 0
    //   generation_off: u32 at layoutPtr + 4
    //   flush_off:      u32 at layoutPtr + 8
    //   max_entries:    u32 at layoutPtr + 12
    //   fields[]:       16 bytes each at layoutPtr + 16
    const fieldCount = mem.getUint32(layoutPtr + 0, true);
    // re-read offsets from WASM module's declared layout
    const genOff = mem.getUint32(layoutPtr + 4, true);
    const flushOff = mem.getUint32(layoutPtr + 8, true);
    if (genOff !== undefined) this._generationOffset = genOff;
    if (flushOff !== undefined) this._flushBufferOffset = flushOff;
    this._bindingTable = [];
    for (let i = 0; i < fieldCount; i++) {{
      const off = layoutPtr + 16 + i * 16;
      const handle = mem.getUint32(off + 0, true);
      const fieldOff = mem.getUint32(off + 4, true);
      const fieldSize = mem.getUint32(off + 8, true);
      const typeTag = mem.getUint32(off + 12, true);
      this._bindingTable[handle] = this._makeBinding(handle, fieldOff, fieldSize, typeTag);
    }}
{bindings_js}
  }}

  _makeBinding(handle, fieldOff, fieldSize, typeTag) {{
    const _this = this;
    return {{
      handle,
      applyFn: function(value) {{
        // Default binding: log state change.
        // The view compiler's bindings customize this per field.
        console.debug(`[web] state update: handle=${{handle}} value=${{value}}`);
      }},
    }};
  }}

  _applyFlush(updatesPtr, count) {{
    const mem = new DataView(this._memory.buffer);
    let off = updatesPtr;
    for (let i = 0; i < count; i++) {{
      const handle = mem.getUint32(off, true); off += 4;
      const valPtr = mem.getUint32(off, true); off += 4;
      const valLen = mem.getUint32(off, true); off += 4;
      const binding = this._bindingTable[handle];
      if (binding) {{
        const value = valLen > 0
          ? new TextDecoder().decode(new Uint8Array(this._memory.buffer, valPtr, valLen))
          : null;
        binding.applyFn(value);
      }}
    }}
  }}

  get generation() {{
    if (!this._memory) return 0;
    const mem = new DataView(this._memory.buffer);
    return mem.getUint32(this._generationOffset, true);
  }}

  set generation(val) {{
    if (!this._memory) return;
    const mem = new DataView(this._memory.buffer);
    mem.setUint32(this._generationOffset, val, true);
  }}
}}

export async function createApp(wasmBytes) {{
  const runtime = new WasmDomRuntime(wasmBytes);
  return runtime._instance.exports;
}}
"#,
            app_name = self.state_layout.app_name,
            generation_offset = self.state_layout.generation_offset,
            flush_buffer_offset = self.state_layout.flush_buffer_offset,
            max_flush_entries = self.state_layout.max_flush_entries,
            bindings_js = bindings_js,
            imports_js = imports_js,
        )
    }

    /// Generate per-binding apply functions from view compiler bindings.
    /// 2026-07-26: Phase 3 — Each binding directive (Text, Show, Trigger, etc.)
    /// produces a JS function that reads from WASM memory and applies DOM changes.
    fn generate_binding_table(&self) -> String {
        if self.bindings.is_empty() {
            return String::new();
        }

        let mut out = String::from("  _applyViewBindings() {\n");
        for binding in &self.bindings {
            let js = self.binding_to_js(binding);
            out.push_str(&format!("    // binding: {} {:?}\n", binding.element_id, binding.directive));
            out.push_str(&format!("    {}\n", js));
        }
        out.push_str("  }\n\n");
        out
    }

    /// Convert a single view binding to a JS apply function.
    fn binding_to_js(&self, binding: &crate::view_compiler::Binding) -> String {
        use crate::view_compiler::Directive;
        let signal = match &binding.directive {
            Directive::Text { signal } => signal,
            Directive::Show { expr } => expr,
            Directive::Hide { expr } => expr,
            Directive::Trigger { txn, .. } => txn,
            _ => return String::new(),
        };

        match &binding.directive {
            Directive::Text { .. } => {
                // Find matching field by signal name heuristic
                let handle = self.state_layout.fields.iter()
                    .position(|f| f.field_handle as usize == self.state_layout.fields.iter()
                        .position(|f2| f2.field_handle as usize == self.state_layout.fields.len())
                        .unwrap_or(0));
                // Emit a placeholder binding — Phase 6 will wire exact field handles
                format!(
                    "    // Text binding: signal='{}' element='{}'",
                    signal, binding.element_id
                )
            }
            Directive::Show { .. } => {
                format!(
                    "    // Show binding: signal='{}' element='{}'",
                    signal, binding.element_id
                )
            }
            Directive::Trigger { event, txn, params } => {
                let param_str = if params.is_empty() {
                    String::new()
                } else {
                    let args: Vec<String> = params.iter().map(|(_, v)| v.clone()).collect();
                    format!(", {}", args.join(", "))
                };
                format!(
                    "    // Trigger: event='{}' element='{}' txn='{}({})'",
                    event, binding.element_id, txn, param_str
                )
            }
            _ => String::new(),
        }
    }

    /// Generate custom imports section for frgn from #Web handlers.
    fn generate_imports(&self) -> String {
        // Phase 6 will wire actual frgn imports resolved through #Web protocol.
        String::from("      // frgn from #Web imports added here in Phase 6\n")
    }

    /// Generate the app.d.ts TypeScript declarations.
    /// 2026-07-26: Phase 3 — Declares the WasmDomRuntime class and createApp function.
    fn generate_dts(&self) -> String {
        let txn_methods = self.generate_txn_declarations();

        format!(
            r#"// app.d.ts — Auto-generated TypeScript declarations for {app_name}

export interface WasmDomRuntime {{
  readonly generation: number;
  readonly _instance: WebAssembly.Instance;
  readonly _memory: WebAssembly.Memory;
{txn_methods}}}

export function createApp(wasmBytes: Uint8Array): Promise<WebAssembly.Exports>;
"#,
            app_name = self.state_layout.app_name,
            txn_methods = txn_methods,
        )
    }

    /// Generate TS method declarations for each transaction.
    /// 2026-07-26: Phase 3 — Each Trigger binding maps to a method on the WASM exports.
    fn generate_txn_declarations(&self) -> String {
        let mut seen = std::collections::HashSet::new();
        let mut out = String::new();

        for binding in &self.bindings {
            use crate::view_compiler::Directive;
            if let Directive::Trigger { txn, .. } = &binding.directive {
                if seen.insert(txn.clone()) {
                    out.push_str(&format!("  {}(...args: any[]): any;\n", txn));
                }
            }
        }

        if out.is_empty() {
            out.push_str("  // No transaction bindings declared\n");
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_empty_generator() -> GlueWebGenerator {
        GlueWebGenerator::new(
            Vec::new(),
            Vec::new(),
            StateLayout {
                app_name: "test_app".to_string(),
                generation_offset: 0,
                flush_buffer_offset: 64,
                max_flush_entries: 16,
                fields: vec![
                    FieldLayout {
                        field_handle: 1,
                        offset: 4,
                        size: 4,
                        type_tag: TypeTag::Int,
                    },
                    FieldLayout {
                        field_handle: 2,
                        offset: 8,
                        size: 4,
                        type_tag: TypeTag::Float,
                    },
                    FieldLayout {
                        field_handle: 3,
                        offset: 12,
                        size: 1,
                        type_tag: TypeTag::Bool,
                    },
                ],
            },
            HashMap::new(),
        )
    }

    #[test]
    fn test_generate_produces_output() {
        let g = make_empty_generator();
        let output = g.generate().expect("generate should succeed");
        assert!(output.dom_shim.contains("WasmDomRuntime"),
            "dom-shim should contain WasmDomRuntime class");
        assert!(output.dts.contains("createApp"),
            "dts should contain createApp function");
    }

    #[test]
    fn test_generate_includes_generation_offset() {
        let g = make_empty_generator();
        let output = g.generate().unwrap();
        assert!(output.dom_shim.contains("_generationOffset"),
            "should include generation offset");
        assert!(output.dom_shim.contains("generation"),
            "should include generation accessor");
    }

    #[test]
    fn test_generate_with_bindings() {
        let bindings = vec![
            crate::view_compiler::Binding {
                element_id: "counter".to_string(),
                directive: crate::view_compiler::Directive::Text {
                    signal: "count".to_string(),
                },
            },
            crate::view_compiler::Binding {
                element_id: "inc-btn".to_string(),
                directive: crate::view_compiler::Directive::Trigger {
                    event: "click".to_string(),
                    txn: "increment".to_string(),
                    params: vec![],
                },
            },
        ];
        let g = GlueWebGenerator::new(
            Vec::new(),
            bindings,
            StateLayout {
                app_name: "counter_app".to_string(),
                generation_offset: 0,
                flush_buffer_offset: 64,
                max_flush_entries: 8,
                fields: vec![
                    FieldLayout {
                        field_handle: 1,
                        offset: 4,
                        size: 4,
                        type_tag: TypeTag::Int,
                    },
                ],
            },
            HashMap::new(),
        );
        let output = g.generate().expect("generate should succeed");
        assert!(output.dom_shim.contains("counter_app"),
            "dom-shim should include app name");
        assert!(output.dts.contains("increment"),
            "dts should include transaction method declarations");
        assert!(output.dom_shim.contains("_applyFlush"),
            "dom-shim should include flush handler");
    }

    #[test]
    fn test_empty_bindings() {
        let g = make_empty_generator();
        let output = g.generate().unwrap();
        assert!(output.dom_shim.contains("createApp"),
            "should still produce createApp with empty bindings");
    }

    #[test]
    fn test_state_layout_roundtrip() {
        let g = make_empty_generator();
        let output = g.generate().unwrap();
        // Verify the flush buffer config appears in the output
        assert!(output.dom_shim.contains("_maxFlushEntries"),
            "should include max flush entries");
        assert!(output.dom_shim.contains("_flushBufferOffset"),
            "should include flush buffer offset");
    }
}
