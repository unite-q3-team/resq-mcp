//! resq-mcp — RESQ-kit plugin: an MCP server exposing QVM analysis tools
//! to AI agents.
//!
//! Transport: newline-delimited JSON-RPC 2.0 over stdio (per the Model
//! Context Protocol), built on the resq-plugin-sdk framing/transport.
//!
//! Usage (standalone or via an MCP client config):
//!
//! ```text
//! resq-mcp                    # serve on stdio; open a QVM via open_qvm
//! resq-mcp path/to/game.qvm   # preload the session before serving
//! ```

mod session;
mod tools;

use resq_plugin_sdk::{serve_stdio, Handler, PluginInfo, RpcError};
use serde_json::Value;
use session::Session;

/// MCP protocol date-versions this server speaks. On `initialize` we echo
/// the client's choice when supported, else fall back to our newest.
const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2024-11-05"];

struct McpHandler {
    session: Option<Session>,
}

impl Handler for McpHandler {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "resq-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    fn call(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => {
                let asked = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = if SUPPORTED_VERSIONS.contains(&asked) {
                    asked
                } else {
                    SUPPORTED_VERSIONS[0]
                };
                Ok(serde_json::json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "resq-mcp",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }))
            }
            "ping" => Ok(serde_json::json!({})),
            "tools/list" => Ok(tools::definitions()),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("tools/call: missing `name`"))?;
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                // Tool failures are results with isError:true (MCP style);
                // only protocol-level misuse becomes a JSON-RPC error.
                match tools::call(&mut self.session, name, &args) {
                    Ok(Ok(value)) => Ok(serde_json::json!({
                        "content": [ { "type": "text", "text": value.to_string() } ],
                        "structuredContent": value,
                    })),
                    Ok(Err(msg)) => Ok(serde_json::json!({
                        "content": [ { "type": "text", "text": msg } ],
                        "isError": true,
                    })),
                    Err(msg) => Err(RpcError::invalid_params(msg)),
                }
            }
            other => Err(RpcError::method_not_found(other)),
        }
    }

    fn notify(&mut self, method: &str, _params: &Value) {
        if method == "notifications/initialized" {
            eprintln!("[resq-mcp] client initialized");
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut handler = McpHandler { session: None };

    // Optional preload: resq-mcp <qvm-path> [map-path]
    match args.first() {
        Some(path) => match Session::open(path, args.get(1).map(String::as_str)) {
            Ok(s) => {
                eprintln!(
                    "[resq-mcp] preloaded {} ({} functions)",
                    s.qvm.path,
                    s.fns.len()
                );
                handler.session = Some(s);
            }
            Err(e) => {
                eprintln!("[resq-mcp] preload failed: {e}");
                std::process::exit(2);
            }
        },
        None => eprintln!("[resq-mcp] no preload; call open_qvm to load a QVM"),
    }

    if let Err(e) = serve_stdio(&mut handler) {
        eprintln!("[resq-mcp] exited: {e}");
        std::process::exit(1);
    }
}
