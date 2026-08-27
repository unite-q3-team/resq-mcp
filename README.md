# resq-mcp

[RESQ-kit](https://github.com/unite-q3-team/RESQ-kit) plugin: an MCP
(Model Context Protocol) server that gives AI agents a full QVM analysis
workbench — open a Quake 3 VM module, list/search functions, decompile to
identity C, disassemble, inspect strings, trace xrefs and classify memory
addresses.

Built on [resq-plugin-sdk](https://github.com/unite-q3-team/resq-plugin-sdk)
(newline-delimited JSON-RPC 2.0 over stdio, the standard MCP transport).

## Install / build

```bash
git clone https://github.com/unite-q3-team/resq-mcp
cd resq-mcp
cargo build --release
# binary: target/release/resq-mcp
```

Depends on sibling checkouts at build time (`../resq-plugin-sdk`,
`../RESQ-kit/toolchain/qvm`); see *Path dependencies* below.

## Wire into an MCP client

Claude Desktop (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "resq-mcp": {
      "command": "C:/path/to/resq-mcp/target/release/resq-mcp.exe",
      "args": ["C:/path/to/baseq3a/vm/game/qagame.qvm"]
    }
  }
}
```

Any other MCP client: same `command`/`args`. Without `args` the server
starts with no session; the agent loads a QVM with the `open_qvm` tool.

## Tools

| Tool                  | What it does                                                        |
|-----------------------|---------------------------------------------------------------------|
| `open_qvm`            | Load a `.qvm` (optional `map_path` for q3asm symbol names)           |
| `session_info`        | Header fields, module, function/string counts                        |
| `list_functions`      | Filterable, paginated function table (name/traps/size)               |
| `get_function`        | One function in detail: traps, strings, callees, callers             |
| `decompile_function`  | Identity C (stack -> SSA -> C)                                       |
| `disassemble_function`| Instruction listing with string/syscall/call annotations             |
| `get_strings`         | Literal-segment strings, filterable, paginated                       |
| `xrefs_to`            | Which functions CONST-reference a data address, with hit counts      |
| `mem_hint`            | Classify an address: data value / string / float / BSS / NULL        |

Function references (`fn` argument) accept an integer index or a string —
map name or `fn_<idx>` placeholder (`"fn_12"`, `"vmMain"`, `"trap_Print"`).

Full reference with schemas and a recommended agent workflow:
[docs/tools.md](docs/tools.md), [docs/agent-usage.md](docs/agent-usage.md).
Русская версия: [docs-ru/](docs-ru/), [README-ru.md](README-ru.md).

## Example session

```text
> open_qvm {path: "qagame.qvm"}
  -> 1310 functions, 3056 strings, module Game
> list_functions {filter: "SendClientCommand"}
  -> idx 66 "trap_SendClientCommand" (thunk), idx 402 calls it
> decompile_function {fn: 402}
  -> void fn_402(int a0) { trap_SendClientCommand(va("%s", a0)); ... }
> xrefs_to {address: 98304}  // some cvar string
  -> fn 120 (3 refs), fn 811 (1 ref)
```

## Path dependencies

Until `qvm`/`resq-plugin-sdk` are published, this crate expects sibling
checkouts:

```text
GitHub/
  RESQ-kit/            # provides toolchain/qvm
  resq-plugin-sdk/
  resq-mcp/
```

## License

MIT — see [LICENSE](LICENSE).
