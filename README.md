# xlsx-dump

Dump spreadsheet workbooks (`.xlsx`, `.xls`, `.ods`) to CSV and JSON — values and formulas — for agents and spreadsheet apps.

Licensed under either of MIT (`LICENSE-MIT`) or Apache-2.0 (`LICENSE-APACHE`), at your option.

## Status

Scaffold only: the command-line skeleton, exit-code table, and test harness exist; no subcommand is implemented yet.  The intended surface is in `DESIGN.md` § 4.

## Install

```
cargo install --path .
```

## Quick Start (planned)

```
xlsx-dump sheets Budget.xlsx            # list sheets, including hidden ones
xlsx-dump csv Budget.xlsx               # Budget-fromXlsx.csv beside the workbook
xlsx-dump cells Budget.xlsx --json      # one JSON record per cell, with formulas
```

## Subcommands (planned)

| Subcommand | Does |
|---|---|
| `sheets` | List sheets with visibility, used range, formulas, merged regions |
| `csv` | Write one CSV per sheet (UTF-8 with BOM by default) |
| `cells` | JSONL of every non-empty cell: typed value and formula |
| `check` | Exit 3 on findings: hidden sheets, formulas without cached values |

## Architecture

See `CLAUDE.md` for the module map and `DESIGN.md` for the contracts.
