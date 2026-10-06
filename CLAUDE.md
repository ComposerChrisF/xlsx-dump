# xlsx-dump — Project Instructions

A read-only CLI that dumps spreadsheet workbooks (`.xlsx`, `.xls`, `.ods`, via the `calamine` crate) to CSV and JSON, values and formulas, so agents and spreadsheet apps can read what arrives as an email attachment.  A sibling of `pdf-dump`.  It exists because three sessions in two vaults hit unreadable KCS workbooks and worked around them by hand; the full story, the consumers, and every contract are in **`DESIGN.md`** — read it before writing code.

**Consumers:** Obsidian-Brain (owns the `-fromXlsx.csv` naming rule; its web app scans hidden sheets), the KCS-Board vault (the first bulk run), the KCS-Assistant vault (the `email-indexer` hook), and eml-tool (a possible home for that hook).  What each needs is `DESIGN.md` § 2; change nothing that breaks one of them without asking it.

## Where the Current State Lives

| The question | The source |
|---|---|
| Version | `Cargo.toml`, or `xlsx-dump --version` |
| Flags, subcommands, exit codes | `xlsx-dump --help` and each subcommand’s `--help` |
| What is built | `README.md` § Status |
| What comes next | the top of `TODO.md` |
| What is open | `bugs/` and `plans/` (both start empty; see the global `bug-reports` and `plan-files` rules) |
| Why, for whom, and the contracts | `DESIGN.md` |

**Open source** (MIT OR Apache-2.0), **public** GitHub repo `ComposerChrisF/xlsx-dump`.  Hence the hard fixture rule below.

## Build / Test / Lint

```
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Commit through `/commit` (it picks `commit-rust-cli`), never a hand-rolled `git commit`.

## Module Map

| File | Responsibility |
|---|---|
| `src/main.rs` | Entry point; silent exit 0 on BrokenPipe, exit 1 on error |
| `src/lib.rs` | `run()` and subcommand dispatch; returns `ExitCode` so findings can exit 3 |
| `src/cli.rs` | clap definitions; the exit-code table, naming rule, CSV contract, and reader limits in `after_long_help` |
| `src/workbook.rs` | Opening (format sniffing, encrypted/corrupt/sheetless detection, panic containment) and reading sheets into a sparse, format-independent cell list |
| `src/render.rs` | Values as CSV text and typed JSON (15-digit numbers); the A1-anchored CSV grid |
| `src/dates.rs` | Excel serials to ISO 8601 dates, times, and durations (1900 and 1904 systems) |
| `src/naming.rs` | The `--name` template, the `-from<Ext>` marker, sheet-name sanitizing |
| `src/output.rs` | The three-state clobber probe (via `cli-contract`) and atomic no-clobber writes |
| `src/commands/` | One module per subcommand; `mod.rs` holds sheet selection |
| `src/types.rs` | Every serde type for `--json` / JSONL output — field names are a consumer contract |
| `tests/common/mod.rs` | Synthetic fixture builders (rust_xlsxwriter, zip rewrites, cfb) |

## Design Tenets

- **A reader, never a writer or evaluator.**  It reports formula text and the application’s cached value; it never recomputes, executes macros, or follows external links.
- **Loud `Unknown`.**  An encrypted, corrupt, or unsupported workbook is exit 1 naming the file — never an empty CSV.  A hidden sheet is dumped and flagged, never skipped by default.  A formula with no cached value is counted and reported, never a silent blank.  `DESIGN.md` § 6 is the table, and it is the contract that matters most.
- **Read failures are never exit 3.**  Findings (3) mean the tool read the data and found something; a failed read is 1.
- **CSV carries a UTF-8 BOM by default** (`--no-bom` to omit), is anchored at A1, and holds raw values with ISO 8601 dates.
- **No clobbering by default.**  An author-supplied derived copy must never be rebuilt, and the tool cannot tell it from the vault’s own — so overwriting is the caller’s explicit `--overwrite`.
- **Path contracts from day one:** a missing input is exit 1; a missing output directory is exit 1 (`--create-destination`, default `none`).

## Gotchas

- **`calamine` ranges start at the first used cell, not A1.**  Pad, or column positions shift.  The value and formula ranges start independently; index by absolute position.
- **Never `open_workbook_auto`.**  It discards calamine’s “password protected” error for an unexpected extension; `workbook.rs` sniffs the container instead.  `DESIGN.md` § 10.
- **rust_xlsxwriter always caches a formula value** and cannot write the 1904 system; the fixtures for those cases rewrite the package with `zip` (`tests/common/mod.rs`).
- **Excel dates are numbers with a format.**  A date `calamine` cannot classify stays a number; the JSON must say which it is, never guess.
- **Sheet names are not filenames.**  Sanitize by a documented rule, and refuse a collision rather than overwrite one sheet’s output with another’s.
- **Fixtures are synthetic, always.**  The repo is public.  Never commit a real KCS, personal, or financial workbook, nor a CSV made from one, not even with figures altered.  The real treasurer workbook in the KCS-Board vault is a _local_ acceptance oracle only (`DESIGN.md` § 8).

## Roadmap

The work queue is `TODO.md`; the consumer questions are `DESIGN.md` § 9, and their rulings § 10.
