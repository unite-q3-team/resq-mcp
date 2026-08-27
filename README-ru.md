# resq-mcp

Плагин [RESQ-kit](https://github.com/unite-q3-team/RESQ-kit): MCP-сервер
(Model Context Protocol), дающий ИИ-агентам полный аналитический верстак
по QVM — открыть модуль Quake 3 VM, искать и листать функции, декомпилировать
в identity C, дизассемблировать, смотреть строки, трассировать xrefs и
классифицировать адреса памяти.

Построен на
[resq-plugin-sdk](https://github.com/unite-q3-team/resq-plugin-sdk)
(JSON-RPC 2.0 построчно поверх stdio — стандартный MCP-транспорт).

## Сборка

```bash
git clone https://github.com/unite-q3-team/resq-mcp
cd resq-mcp
cargo build --release
# бинарник: target/release/resq-mcp
```

До публикации `qvm`/`resq-plugin-sdk` крейт ждёт соседние чекауты — см.
«Путевые зависимости» ниже.

## Подключение к MCP-клиенту

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

Любой другой MCP-клиент: те же `command`/`args`. Без `args` сервер
стартует без сессии; агент загрузит QVM инструментом `open_qvm`.

## Инструменты

| Инструмент            | Что делает                                                            |
|-----------------------|------------------------------------------------------------------------|
| `open_qvm`            | Загрузить `.qvm` (опционально `map_path` с именами из q3asm)           |
| `session_info`        | Поля заголовка, модуль, счётчики функций/строк                          |
| `list_functions`      | Таблица функций с фильтром и пагинацией (имя/трапы/размер)              |
| `get_function`        | Детально одна функция: трапы, строки, callees, callers                  |
| `decompile_function`  | Identity C (стек -> SSA -> C)                                           |
| `disassemble_function`| Листинг инструкций с аннотациями строк/сисколлов/вызовов                |
| `get_strings`         | Строки литерального сегмента, фильтр + пагинация                        |
| `xrefs_to`            | Кто ссылается на адрес данных через CONST, со счётчиками                |
| `mem_hint`            | Классификация адреса: значение данных / строка / float / BSS / NULL     |

Ссылка на функцию (аргумент `fn`) — целочисленный индекс или строка:
имя из .map либо плейсхолдер `fn_<idx>` (`"fn_12"`, `"vmMain"`,
`"trap_Print"`).

Полный справочник со схемами и рекомендованным сценарием работы агента:
[docs-ru/tools.md](docs-ru/tools.md), [docs-ru/agent-usage.md](docs-ru/agent-usage.md).
English version: [docs/](docs/), [README.md](README.md).

## Пример сессии

```text
> open_qvm {path: "qagame.qvm"}
  -> 1310 функций, 3056 строк, модуль Game
> list_functions {filter: "SendClientCommand"}
  -> idx 66 "trap_SendClientCommand" (танк), idx 402 его вызывает
> decompile_function {fn: 402}
  -> void fn_402(int a0) { trap_SendClientCommand(va("%s", a0)); ... }
> xrefs_to {address: 98304}  // строка какого-никакого cvar-а
  -> fn 120 (3 вхождения), fn 811 (1 вхождение)
```

## Путевые зависимости

Пока `qvm`/`resq-plugin-sdk` не опубликованы, крейт ждёт соседние чекауты:

```text
GitHub/
  RESQ-kit/            # даёт toolchain/qvm
  resq-plugin-sdk/
  resq-mcp/
```

## Лицензия

MIT — см. [LICENSE](LICENSE).
