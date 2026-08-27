# Tool reference

All tools are invoked via MCP `tools/call` and return
`{"content":[{"type":"text","text":"<json>"}], "structuredContent":{...}}`.
Task failures return `isError: true` with the message as text — the
protocol-level error `-32602` is reserved for malformed calls.

Unless stated otherwise, tools require a session (`open_qvm` first).

## open_qvm

`{"path": string, "map_path"?: string}`

Loads a QVM and replaces the session (single-session server). Without
`map_path`, a sibling `<name>.map` next to the QVM is auto-loaded when
present. Names from the map appear in every tool's output.

Returns `session_info` payload (see below).

## session_info

`{}`

```json
{
  "path": "C:/cpmatest/vm/qagame.qvm",
  "module": "Game",
  "vm_magic": 1279843586,
  "instruction_count": 277423,
  "code_length": 831152,
  "data_length": 22916,
  "lit_length": 64388,
  "bss_length": 4344784,
  "functions": 1310,
  "strings": 3056,
  "named_functions": 0
}
```

`module` drives the syscall name table (`Game`/`CGame`/`Ui`).
`named_functions == 0` means a clean image without a `.map` — address it
by index/placeholder (`fn_5`).

## list_functions

`{"filter"?: string, "offset"?: int, "limit"?: int}` (limit default 200,
max 2000)

Case-insensitive substring filter over: display name, `fn_<idx>`
placeholder, function index, entry address, trap names, referenced string
literals. Returns `{"total": N, "functions": [...]}` where each row has
`idx`, `entry`, `end`, `insns`, `name` (nullable), `placeholder`,
`traps` (resolved names).

## get_function

`{"fn": idx | name}`

Adds to the listing row: full `traps` (`{"syscall": n, "name": t}`),
up to 40 referenced `strings`, `callees`/`callers` as `{"idx","name",
"placeholder"}` refs (direct CONST+CALL edges only).

## decompile_function

`{"fn": idx | name}` → `{"language": "c", "code": string}`

Identity C of one function: locals as `loc_<frameoff>`, args as `arg_N`,
untyped memory as `*(<int>*)(addr)` — the same output the GUI shows.
Types/structs are not applied here (see RESQ-kit GUI for typed
decompilation).

## disassemble_function

`{"fn": idx | name}` → `{"listing": string}`

One instruction per line (`idx addr OP operand`), with `; "string"`,
`; syscall N trap_X` and `; call name` annotations on CONST lines
followed by CALL.

## get_strings

`{"filter"?: string, "offset"?: int, "limit"?: int}` →
`{"total": N, "strings": [{"addr": int, "value": string}, ...]}`

Strings from the literal segment in address order. `addr` is a VM memory
address suitable for `xrefs_to`/`mem_hint`.

## xrefs_to

`{"address": int}` → `{"address": int, "refs": [fn_ref + {"refs": count}]}`

Functions containing `CONST address` (any purpose: data pointer, string
pointer, BSS global). Not limited to call targets; for call graphs use
`get_function` callees/callers.

## mem_hint

`{"address": int}` → `{"address": int, "hint": string | null}`

Classification examples: `[0x247d0] data = 5 (0x5)`,
`[0x1000] data = "g_gametype"`, `[0xa40] lit = float 0.001`,
`[0x104bdc] BSS = runtime global (zero at load)`, `"NULL"`,
`null` (outside memory — likely a code address).

## Address bookkeeping

- VM memory addresses are `i32` in [0, data+lit+bss); negative CONST
  operands are syscall numbers, code addresses are `>= data+lit+bss`.
- Data segment starts at address 0; literals follow data; BSS (zero-init)
  follows literals.
