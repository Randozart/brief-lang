// ── External Plugin Runner ──────────────────────────────────────────────
// 2026-07-14: Run external plugins as subprocesses that read/write BVIR
// format via stdin/stdout. Each plugin chain is: plugin_a | plugin_b | ...
//
// Plugin contract:
//   stdin:   receives .bvir text (IR before this plugin)
//   stdout:  writes .bvir text (IR after this plugin)
//   exit 0:  success — compilation continues with plugin's output
//   exit !0: abort — stderr is the error message

use std::path::Path;
use std::process::{Command, Stdio};

/// Run the plugin chain on a BVIR text string.
/// Each plugin is an executable. The output of plugin N becomes the input
/// of plugin N+1. Returns the final BVIR text after all plugins have run.
/// Plugins are executed in order.
pub fn run_plugin_chain(bvir_text: &str, plugin_paths: &[String]) -> Result<String, String> {
    let mut current = bvir_text.to_string();
    for path in plugin_paths {
        let p = Path::new(path);
        if !p.exists() {
            return Err(format!("plugin not found: {}", path));
        }
        let mut child = Command::new(p)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot spawn plugin '{}': {}", path, e))?;

        let stdin = child.stdin.as_mut().unwrap();
        use std::io::Write;
        stdin.write_all(current.as_bytes())
            .map_err(|e| format!("cannot write to plugin '{}': {}", path, e))?;
        drop(stdin);

        let output = child.wait_with_output()
            .map_err(|e| format!("cannot read from plugin '{}': {}", path, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("plugin '{}' aborted: {}", path, stderr.trim()));
        }

        current = String::from_utf8(output.stdout)
            .map_err(|_| format!("plugin '{}' produced invalid UTF-8", path))?;

        if current.trim().is_empty() {
            return Err(format!("plugin '{}' produced empty output", path));
        }
    }
    Ok(current)
}
