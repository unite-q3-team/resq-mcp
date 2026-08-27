//! MCP tool surface: schemas for `tools/list` and dispatch for `tools/call`.

use crate::session::Session;
use serde_json::{json, Value};

/// Cut long strings in hints/lists; full values are in `get_strings`.
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn schema(props: Value, required: &[&str]) -> Value {
    let mut s = json!({ "type": "object", "properties": props });
    if !required.is_empty() {
        s["required"] = json!(required);
    }
    s
}

fn fn_spec() -> Value {
    json!({
        "fn": {
            "description": "Function: integer index or string name (map name or fn_<idx> placeholder)",
            "oneOf": [ { "type": "integer", "minimum": 0 }, { "type": "string" } ]
        }
    })
}

/// Tool descriptions + input schemas, in `tools/list` order.
pub fn definitions() -> Value {
    let tools = vec![
        ("open_qvm",
         "Load a .qvm file as the analysis session. Optional map_path adds q3asm symbol names (by default a sibling .map is picked up automatically). Replaces the currently open QVM.",
         schema(json!({
            "path": { "type": "string", "description": "Path to the .qvm file" },
            "map_path": { "type": "string", "description": "Optional q3asm .map file with function names" }
         }), &["path"])),
        ("session_info",
         "Info about the currently open QVM: header fields, module, function/string counts.",
         schema(json!({}), &[])),
        ("list_functions",
         "List functions (index, entry..end instruction range, name, size, traps). Supports substring filter and pagination.",
         schema(json!({
            "filter": { "type": "string", "description": "Case-insensitive substring over name/placeholder/trap names/string literals" },
            "offset": { "type": "integer", "minimum": 0 },
            "limit": { "type": "integer", "minimum": 1, "maximum": 2000 }
         }), &[])),
        ("get_function",
         "Detail one function: traps, string literals, callees and callers.",
         schema(fn_spec(), &["fn"])),
        ("decompile_function",
         "Identity C decompilation of one function (stack -> SSA -> C).",
         schema(fn_spec(), &["fn"])),
        ("disassemble_function",
         "Instruction listing of one function with string/syscall/call annotations.",
         schema(fn_spec(), &["fn"])),
        ("get_strings",
         "String literals from the literal segment, in address order. Supports substring filter and pagination.",
         schema(json!({
            "filter": { "type": "string" },
            "offset": { "type": "integer", "minimum": 0 },
            "limit": { "type": "integer", "minimum": 1, "maximum": 2000 }
         }), &[])),
        ("xrefs_to",
         "Functions referencing a data address through CONST (with hit counts).",
         schema(json!({
            "address": { "type": "integer", "description": "VM memory address (data/lit/BSS)" }
         }), &["address"])),
        ("mem_hint",
         "Classify one memory address: data value, string, float literal, BSS global, NULL, or outside memory.",
         schema(json!({
            "address": { "type": "integer" }
         }), &["address"])),
    ];
    json!({
        "tools": tools.into_iter().map(|(name, desc, input)| json!({
            "name": name,
            "description": desc,
            "inputSchema": input,
        })).collect::<Vec<_>>()
    })
}

/// Dispatch one `tools/call`. Ok(Err(msg)) models MCP "tool failed" results
/// (isError: true with the message as content).
pub fn call(
    session: &mut Option<Session>,
    name: &str,
    args: &Value,
) -> Result<Result<Value, String>, String> {
    match name {
        "open_qvm" => {
            let path = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or("open_qvm: missing string `path`")?;
            let map = args.get("map_path").and_then(Value::as_str);
            match Session::open(path, map) {
                Ok(s) => {
                    let info = session_info_json(&s);
                    *session = Some(s);
                    Ok(Ok(info))
                }
                Err(e) => Ok(Err(e)),
            }
        }
        _ => {
            let Some(s) = session.as_mut() else {
                return Ok(Err("no QVM loaded - call open_qvm first".into()));
            };
            dispatch(s, name, args)
        }
    }
}

fn dispatch(s: &mut Session, name: &str, args: &Value) -> Result<Result<Value, String>, String> {
    match name {
        "session_info" => Ok(Ok(session_info_json(s))),
        "list_functions" => Ok(Ok(list_functions(s, args))),
        "get_function" => Ok(with_fn(s, args, |s, idx| {
            let f = &s.fns[idx];
            let names =
                |idxs: &[usize]| -> Vec<Value> { idxs.iter().map(|&i| fn_ref(s, i)).collect() };
            Ok(json!({
                "idx": f.idx,
                "entry": f.entry,
                "end": f.end,
                "insns": f.end - f.entry,
                "name": f.name,
                "placeholder": format!("fn_{}", f.idx),
                "traps": f.traps.iter().map(|(n, t)| json!({"syscall": n, "name": t})).collect::<Vec<_>>(),
                "strings": f.strings.iter().take(40).map(|v| clip(v, 120)).collect::<Vec<_>>(),
                "callees": names(&f.callees),
                "callers": names(s.callers.get(&f.idx).map(Vec::as_slice).unwrap_or(&[])),
            }))
        })),
        "decompile_function" => Ok(with_fn(s, args, |s, idx| {
            s.decompile(idx)
                .map(|text| json!({ "language": "c", "code": text }))
        })),
        "disassemble_function" => Ok(with_fn(s, args, |s, idx| {
            Ok(json!({ "listing": s.disasm_text(idx) }))
        })),
        "get_strings" => Ok(Ok({
            let filter = args.get("filter").and_then(Value::as_str).unwrap_or("");
            let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(200)
                .clamp(1, 2000) as usize;
            let all: Vec<&(i32, String)> = s
                .lit_strings
                .iter()
                .filter(|(_a, v)| {
                    filter.is_empty() || v.to_lowercase().contains(&filter.to_lowercase())
                })
                .collect();
            let rows: Vec<Value> = all
                .iter()
                .skip(offset)
                .take(limit)
                .map(|(a, v)| json!({ "addr": a, "value": v }))
                .collect();
            json!({ "total": all.len(), "strings": rows })
        })),
        "xrefs_to" => Ok(Ok({
            let addr = args
                .get("address")
                .and_then(Value::as_i64)
                .ok_or("xrefs_to: missing integer `address`")? as i32;
            let refs: Vec<Value> = s
                .xrefs_to(addr)
                .into_iter()
                .map(|(idx, n)| {
                    let mut v = fn_ref(s, idx);
                    v["refs"] = json!(n);
                    v
                })
                .collect();
            json!({ "address": addr, "refs": refs })
        })),
        "mem_hint" => Ok(Ok({
            let addr = args
                .get("address")
                .and_then(Value::as_i64)
                .ok_or("mem_hint: missing integer `address`")? as i32;
            json!({ "address": addr, "hint": s.mem_hint(addr) })
        })),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn with_fn(
    s: &Session,
    args: &Value,
    f: impl FnOnce(&Session, usize) -> Result<Value, String>,
) -> Result<Value, String> {
    let spec = args.get("fn").ok_or("missing argument `fn`")?;
    let idx = s.resolve_fn(spec)?;
    f(s, idx)
}

fn fn_ref(s: &Session, idx: usize) -> Value {
    match s.fns.get(idx) {
        Some(f) => json!({ "idx": idx, "name": f.name, "placeholder": format!("fn_{}", f.idx) }),
        None => json!({ "idx": idx }),
    }
}

fn session_info_json(s: &Session) -> Value {
    json!({
        "path": s.qvm.path,
        "module": format!("{:?}", s.qvm.module),
        "vm_magic": s.qvm.vm_magic,
        "instruction_count": s.qvm.instruction_count,
        "code_length": s.qvm.code_length,
        "data_length": s.qvm.data_length,
        "lit_length": s.qvm.lit_length,
        "bss_length": s.qvm.bss_length,
        "functions": s.fns.len(),
        "strings": s.lit_strings.len(),
        "named_functions": s.fns.iter().filter(|f| f.name.is_some()).count(),
    })
}

fn list_functions(s: &Session, args: &Value) -> Value {
    let filter = args
        .get("filter")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(1, 2000) as usize;

    let match_fn = |f: &crate::session::FnRow| -> bool {
        if filter.is_empty() {
            return true;
        }
        let name = f.name.clone().unwrap_or_else(|| format!("fn_{}", f.idx));
        let mut hay = format!("{} fn{} {} ", name, f.idx, f.entry);
        for (n, t) in &f.traps {
            hay.push_str(t);
            hay.push(' ');
            hay.push_str(&n.to_string());
            hay.push(' ');
        }
        for v in &f.strings {
            hay.push_str(v);
            hay.push(' ');
        }
        hay.to_lowercase().contains(&filter)
    };

    let matched: Vec<&crate::session::FnRow> = s.fns.iter().filter(|f| match_fn(f)).collect();
    let rows: Vec<Value> = matched
        .iter()
        .skip(offset)
        .take(limit)
        .map(|f| {
            json!({
                "idx": f.idx,
                "entry": f.entry,
                "end": f.end,
                "insns": f.end - f.entry,
                "name": f.name,
                "placeholder": format!("fn_{}", f.idx),
                "traps": f.traps.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "total": matched.len(), "functions": rows })
}
