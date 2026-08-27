# Справочник инструментов

Все инструменты вызываются через MCP `tools/call` и возвращают
`{"content":[{"type":"text","text":"<json>"}], "structuredContent":{...}}`.
Сбой задачи приходит как `isError: true` с сообщением в тексте; протокольная
ошибка `-32602` зарезервирована за кривыми вызовами.

Если не оговорено, инструменту нужна сессия (сначала `open_qvm`).

## open_qvm

`{"path": string, "map_path"?: string}`

Загружает QVM и заменяет сессию (сервер односессионный). Без `map_path`
автоматически подхватывается соседний `<имя>.map`, если он есть. Имена из
карты появляются во всех ответах.

Возвращает payload `session_info` (ниже).

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

`module` выбирает таблицу имён сисколлов (`Game`/`CGame`/`Ui`).
`named_functions == 0` — чистый образ без `.map`: адресация по индексу и
плейсхолдеру (`fn_5`).

## list_functions

`{"filter"?: string, "offset"?: int, "limit"?: int}` (лимит по умолчанию
200, максимум 2000)

Регистронезависимый подстрочный фильтр по: имени, плейсхолдеру `fn_<idx>`,
индексу, адресу входа, именам трапов, строковым литералам. Возвращает
`{"total": N, "functions": [...]}`: в строке `idx`, `entry`, `end`,
`insns`, `name` (nullable), `placeholder`, `traps` (разрешённые имена).

## get_function

`{"fn": idx | name}`

К строке листинга добавляет: полные `traps` (`{"syscall": n, "name": t}`),
до 40 строк `strings`, `callees`/`callers` как ссылки `{"idx","name",
"placeholder"}` (только прямые рёбра CONST+CALL).

## decompile_function

`{"fn": idx | name}` → `{"language": "c", "code": string}`

Identity C одной функции: локалы как `loc_<офсет кадра>`, аргументы как
`arg_N`, нетипизированная память как `*(<int>*)(addr)` — тот же вывод,
что в гуишке. Типы/структуры здесь не применяются (типизированная
декомпиляция — в RESQ-kit GUI).

## disassemble_function

`{"fn": idx | name}` → `{"listing": string}`

По инструкции на строку (`idx addr OP operand`), на CONST-строках перед
CALL — аннотации `; "string"`, `; syscall N trap_X`, `; call имя`.

## get_strings

`{"filter"?: string, "offset"?: int, "limit"?: int}` →
`{"total": N, "strings": [{"addr": int, "value": string}, ...]}`

Строки литерального сегмента по порядку адресов. `addr` — адрес памяти VM,
пригодный для `xrefs_to`/`mem_hint`.

## xrefs_to

`{"address": int}` → `{"address": int, "refs": [fn_ref + {"refs": count}]}`

Функции, содержащие `CONST address` (любое назначение: указатель на
данные, строку, BSS-глобал). Ограничения на цели вызовов нет; для графа
вызовов — callees/callers из `get_function`.

## mem_hint

`{"address": int}` → `{"address": int, "hint": string | null}`

Примеры классификации: `[0x247d0] data = 5 (0x5)`,
`[0x1000] data = "g_gametype"`, `[0xa40] lit = float 0.001`,
`[0x104bdc] BSS = runtime global (zero at load)`, `"NULL"`,
`null` (вне памяти — скорее всего адрес кода).

## Арифметика адресов

- Адреса памяти VM — `i32` из [0, data+lit+bss); отрицательные операнды
  CONST — номера сисколлов; адреса кода — `>= data+lit+bss`.
- Сегмент данных начинается с адреса 0; за ним литералы; за ними BSS
  (нуль-инициализация).
