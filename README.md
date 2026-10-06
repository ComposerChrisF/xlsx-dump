# xlsx-dump

Dump spreadsheet workbooks (`.xlsx`, `.xls`, `.ods`) to CSV and JSON — values and formulas — for agents and spreadsheet apps.

Licensed under either of MIT (`LICENSE-MIT`) or Apache-2.0 (`LICENSE-APACHE`), at your option.

## Status

All four subcommands are built — `sheets`, `csv`, `cells`, `check` — over `.xlsx`, `.xlsm`, `.xlsb`, `.xls`, and `.ods`.  Encrypted, corrupt, and unsupported files (`.numbers` included) are exit 1 by name, never an empty dump; hidden sheets are dumped and flagged; formulas with no cached value are counted and reported.  Charts and images on an ordinary worksheet are not yet detected (`plans/plan-0001`).  `xlsx-dump --help` and each subcommand’s `--help` are the reference, including the reader’s known limits.

## Install

```
cargo install --path .
```

Building currently needs the author’s `cli-contract` crate checked out beside this one (`../cli-contract`); until it is released, a fresh clone does not build on its own.

## Quick Start

```
xlsx-dump sheets Budget.xlsx            # list sheets, including hidden ones
xlsx-dump csv Budget.xlsx               # Budget-fromXlsx.csv beside the workbook
xlsx-dump cells Budget.xlsx             # one JSON record per cell, with formulas
xlsx-dump check *.xlsx                  # exit 3 on hidden sheets, uncached formulas, …
xlsx-dump csv inbox/*.xlsx --skip-existing --json   # the idempotent hook form
```

## Subcommands

| Subcommand | Does |
|---|---|
| `sheets` | List sheets with visibility, used range, formulas, merged regions |
| `csv` | Write CSV derived copies, one per sheet (UTF-8 with BOM by default), atomically and never over an existing file unless `--overwrite` |
| `cells` | JSONL of every non-empty cell: typed value and formula |
| `check` | Exit 3 on findings: hidden sheets, formulas without cached values, chart sheets, unreadable sheets |

## Architecture

See `CLAUDE.md` for the module map and `DESIGN.md` for the contracts.
