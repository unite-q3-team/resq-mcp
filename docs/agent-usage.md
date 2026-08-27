# Agent usage guide

How an AI agent should drive resq-mcp to actually understand an unknown
QVM, not just enumerate it.

## Setup

Point your MCP client at the binary:

```json
{ "mcpServers": { "resq": { "command": ".../resq-mcp.exe", "args": ["C:/mods/x/vm/qagame.qvm"] } } }
```

Preloading in `args` saves one tool round-trip; `open_qvm` works too and
allows switching images mid-session.

## Recommended workflow (unknown module)

1. **Orient.** `session_info` — module type and sizes tell you what you
   are in (qagame/cgame/ui). `named_functions: 0`? You are on a clean
   image; everything is `fn_<idx>` until a `.map` shows up.

2. **Strings first.** `get_strings` is the cheapest orientation surface:
   cvar names (`"g_gametype"`), configstrings (`"cs_%i"`), entity field
   error strings (`"G_Spawn: %s doesn't have a spawn function"`),
   UI labels. Search for game-flow keywords:
   `filter: "gamelogic"`, `"InitGame"`, `"ClientConnect"`, `"score"`.

3. **Anchor via xrefs.** Pick a telling string, then `xrefs_to` its
   address. The referencing functions are your candidates (e.g. the
   function citing `"InitGame"` IS (or calls) `G_InitGame`).

4. **Decompile the anchor, not the whole module.** `decompile_function`
   on the candidate; read the C. Callees from `get_function` expand the
   frontier — thunk functions whose single trap names the whole story
   (`trap_SendServerCommand`) are free wins.

5. **Syscall wrappers give free vocabulary.** `list_functions` with
   `filter: "trap_"` lists every trap user; small functions with exactly
   one trap are engine wrappers and effectively part of the SDK surface.

6. **Memory model.** `mem_hint` on an address before you narrate about it:
   `[0x247d0] data = 5` is a constant, `BSS` addresses are runtime
   globals — anything you "see" there in the QVM image is zero, the real
   value exists only at game runtime.

7. **Keep a symbol sheet.** Maintain your own name→idx map in the working
   notes and address functions as `"fn_402"` in every call. When a real
   `.map` is found, restart with `open_qvm {path, map_path}` — every tool
   output becomes named.

## Etiquette rules for agents

- **Paginate.** `list_functions`/`get_strings` default to 200/200 rows —
  that is already a lot of tokens; do not raise `limit` beyond need,
  prefer filters.
- **Prefer `decompile_function` over `disassemble_function`.** Read C,
  fall back to the listing only when the C looks wrong (odd jumps,
  switch tables).
- **Treat identity C as ground truth, not as idiomatic C.** There are no
  types, no loops reconstructed beyond SSA — pointer arithmetic like
  `*(<int>*)((loc_20) + (704))` means "field at offset 704 of whatever
  struct lives at loc_20".
- **One session, one QVM.** `open_qvm` resets state; re-run your
  orientation steps after switching images.
- **isError results are task feedback** ("no QVM loaded", "no function
  named X") — fix the call, do not retry blindly.

## What this server is not

- Not a game emulator: no code execution, no runtime state (that is
  RESQ-kit's `emu`, not exposed here yet).
- Not a persistence layer: renames/types done in the RESQ-kit GUI
  sidecars (`.map`, `types.json`) are read via `open_qvm` `map_path`, but
  there is no write API yet.
