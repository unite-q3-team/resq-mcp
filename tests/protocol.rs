//! End-to-end: spawn the resq-mcp binary and drive the MCP protocol over
//! stdio against a tiny generated QVM.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

fn write_min_qvm(path: &std::path::Path) {
    let code = {
        let mut c = vec![3u8]; // ENTER 16
        c.extend_from_slice(&16i32.to_le_bytes());
        c.push(4); // LEAVE 16
        c.extend_from_slice(&16i32.to_le_bytes());
        c
    };
    let header: [i32; 8] = [
        qvm::loader::VM_MAGIC as i32,
        2,
        32,
        code.len() as i32,
        32 + code.len() as i32,
        0,
        0,
        64,
    ];
    let mut bytes = Vec::new();
    for v in header {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&code);
    std::fs::write(path, &bytes).expect("write qvm");
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl Proc {
    fn spawn() -> Proc {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resq-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn resq-mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        Proc {
            child,
            stdin,
            reader: BufReader::new(stdout),
        }
    }

    fn rpc(&mut self, line: &str) -> serde_json::Value {
        writeln!(self.stdin, "{line}").expect("write rpc");
        self.stdin.flush().expect("flush");
        let mut buf = String::new();
        let n = self.reader.read_line(&mut buf).expect("read rpc");
        assert!(n > 0, "resq-mcp closed stdout before replying to: {line}");
        serde_json::from_str(&buf).expect("valid json reply")
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_session_over_stdio() {
    let dir = std::env::temp_dir().join(format!("resq_mcp_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let qvm_path = dir.join("smoke.qvm");
    write_min_qvm(&qvm_path);

    let mut p = Proc::spawn();

    // initialize -> our server info and an accepted version.
    let init = p.rpc(r#"{"id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#);
    assert_eq!(init["result"]["serverInfo"]["name"], "resq-mcp");
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");

    // tools/list -> our nine tools with schemas.
    let list = p.rpc(r#"{"id":2,"method":"tools/list"}"#);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 9);
    assert_eq!(tools[0]["name"], "open_qvm");

    // open_qvm on the fixture.
    let open = p.rpc(&format!(
        r#"{{"id":3,"method":"tools/call","params":{{"name":"open_qvm","arguments":{{"path":{}}}}}}}"#,
        serde_json::to_string(&qvm_path).unwrap()
    ));
    let info: serde_json::Value =
        serde_json::from_str(open["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(info["functions"], 1);
    assert_eq!(info["instruction_count"], 2);

    // decompile_function by placeholder name.
    let dec = p.rpc(
        r#"{"id":4,"method":"tools/call","params":{"name":"decompile_function","arguments":{"fn":"fn_0"}}}"#,
    );
    assert!(
        !dec["result"]["isError"].is_boolean(),
        "decompile failed: {dec}"
    );
    let payload: serde_json::Value =
        serde_json::from_str(dec["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(
        payload["code"].as_str().unwrap().contains("void"),
        "{payload}"
    );

    // Tool failure (no such function) -> isError result, not a protocol error.
    let bad = p.rpc(
        r#"{"id":5,"method":"tools/call","params":{"name":"decompile_function","arguments":{"fn":"nope"}}}"#,
    );
    assert_eq!(bad["result"]["isError"], true);

    // Unknown method -> JSON-RPC error.
    let unk = p.rpc(r#"{"id":6,"method":"resources/list"}"#);
    assert_eq!(unk["error"]["code"], -32601);

    drop(p);
    std::fs::remove_file(&qvm_path).ok();
    std::fs::remove_dir(&dir).ok();
}
