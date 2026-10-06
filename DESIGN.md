# xlsx-dump — Design

**Status of this document:** the design brief written at scaffolding (6-Oct-2026), before any implementation.  It records _why_ the tool exists, who consumes it, and the contracts it must keep.  Where the implementation settles an open question, record the ruling here and move the user-facing half into `--help`.

## 1.  Why This Tool Exists

Spreadsheet workbooks keep arriving in Chris’s vaults as email attachments — treasurer workbooks, budgets, ticket reports — and no house tool can read them.  Three separate sessions hit the wall independently:

- **16-Sep-2026, KCS-Board vault:** a treasurer-report session found no converter installed (`xlsx2csv`, `ssconvert`, `libreoffice` all missing), unzipped the `.xlsx` by hand, and read the XML to get the variance columns.
- **16-Sep-2026, KCS-Assistant:** the same workbook (`KCS-Financials-20260613-Treasurer-BudgetVsActualsByClass.xlsx`) was extracted by a one-off script.  Chris then decided to build this tool (KCS-Assistant memory `project_xlsx_dump_cli.md`; Proj/Coding’s `TODO.md`).
- **6-Oct-2026, Obsidian-Brain:** priming the KCS-Board vault for go-live, the session found every workbook’s `-fromXlsx.csv` copy pending because no converter exists, and asked to install Python `xlsx2csv` via Homebrew.

Per the portfolio rule “feature gaps get plans, not workarounds” (`~/.claude/rules/strategy-defaults.md`), the answer is a tool, not `xlsx2csv`/csvkit.  What a house tool adds over `xlsx2csv`: the **formulas** (a budget-versus-actual workbook’s calculations are what a reviewer wants to check), **`--json`** typed cell records for agents, the **UTF-8 BOM**, the portfolio **exit codes and path contracts**, and **loud `Unknown`** where a converter would print an empty file.

Its model is `pdf-dump`: an inspection-and-extraction tool over one opaque format.

## 2.  Consumers

These are the projects that will call the tool or depend on its output.  Each should audit the first release against its own needs (§ 9).

| Consumer | What it needs | Where its need is written down |
|---|---|---|
| **Obsidian-Brain** (`~/Chris/Proj/Obsidian-Brain`) | Owns the vault convention every brain vault inherits: a workbook’s derived copy is `<master>-fromXlsx.csv`, never edited, regenerable.  Its web app’s ingestion detectors must scan attachment text — including a workbook’s **hidden sheets** (the routing rubric’s “spreadsheet’s hidden salary tab” case).  Its design doc currently says “pandoc for DOCX/XLSX”, but pandoc does not read `.xlsx`; this tool fills that gap. | `brain/templates/rules/file-kinds-and-naming.md`, `brain/templates/rules/inbox-workflow.md`; `docs/web-app-vault-audiences-and-email-routing.md` § 7; `docs/web-app-routing-rubric.md`; its `TODO.md` (“the workbooks’ `-fromXlsx.csv` copies, which need an xlsx converter installed”) |
| **KCS-Board vault** (`~/Chris/App/Obsidian/KCS-Board/vault`) | The first bulk run: 23 workbooks at scaffolding time, each owed a `-fromXlsx.csv`, marked pending in `inbox/ingestion-log.md`.  The one hand-made copy is the acceptance oracle (§ 8). | `.claude/rules/file-kinds-and-naming.md` (generated from Obsidian-Brain’s template); `references/finance/KCS Monthly Treasurer Report Walkthrough.md` |
| **KCS-Assistant vault** (`~/Chris/App/Obsidian/KCS-Assistant/KCS-Assistant`) | The automation hook: after its `email-indexer` skill extracts attachments, run `xlsx-dump` over each new workbook so the CSV lands beside it.  36 workbooks at scaffolding time.  **The skill does not handle attachments at all yet** — the hook is a change to that skill, made after this tool ships. | Its memory `project_xlsx_dump_cli.md`; `.claude/skills/email-indexer/SKILL.md` |
| **eml-tool** (`~/Chris/App/Rust/eml-tool`) | Possibly the better home for the hook: `eml-tool attachments --extract` already writes attachments to disk.  If conversion-at-extraction belongs there, it is a plan in eml-tool’s repo, not a change here. | — (to be decided; see § 9) |

Chris’s non-vault trees also hold workbooks (`Proj/KCS/*Business`, `Proj/Harmony`, `Proj/KonaOrchards`), but nothing there consumes the tool programmatically; they are a source of realistic local test material, never of committed fixtures.

## 3.  Scope

**In:** read `.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.ods` (whatever `calamine` reads — 133 `.xlsx` and one `.xls` across the KCS trees at scaffolding); list sheets with their visibility and dimensions; dump sheets to CSV; dump cells to JSON with typed values and formula strings; report merged regions.

**Out:**
- **Writing or editing workbooks.**  This is a reader.
- **Evaluating formulas.**  The tool reports the formula text and the value the authoring application cached; it never recomputes.
- **Formatting** — fonts, fills, widths, conditional formats.  `calamine` barely exposes them, and the derived copy is for reading, not appearance.
- **`.numbers`** (Apple Numbers).  The vault convention names `fromNumbers`, but `calamine` cannot read it.  A Numbers file must be exported to `.xlsx` first; the tool must say so by name (exit 1), never skip it silently.
- **Executing anything** — macros, external links, data connections.  `calamine` never does; keep it that way.

## 4.  Proposed Surface

Subcommands rather than `pdf-dump`’s flag style, because the three outputs differ in shape and each wants its own exit-code notes.  Names are proposals; settle them in the first implementation session.

```
xlsx-dump sheets <FILE> [--json]
    One line per sheet: index, name, visibility (visible / hidden / very-hidden),
    used range, and whether it holds formulas or merged regions.

xlsx-dump csv <FILE>... [--sheet <NAME|INDEX>]... [--output-dir <DIR>]
                        [--name <TEMPLATE>] [--overwrite] [--no-bom] [--dry-run] [--json]
    Write CSV derived copies.  Default: every sheet, beside the workbook.

xlsx-dump cells <FILE> [--sheet <NAME|INDEX>]... [--formulas-only]
    JSONL, one record per non-empty cell:
    {sheet, ref, row, col, type, value, formula?, merged?}

xlsx-dump check <FILE>...
    Exit 3 if any workbook has findings worth a reader’s attention (§ 6), else 0.
```

`--json` on `csv` prints the list of files written, skipped, and refused — the record a hook or the vault register needs.

## 5.  The CSV Contract

- **UTF-8 with a BOM by default** (`~/.claude/rules/csv-utf8-bom.md`: Excel on macOS mis-reads UTF-8 without one; the KCS ‘okina history).  `--no-bom` for a machine consumer that cannot strip one.
- **RFC 4180 quoting**, via the `csv` crate.
- **Anchored at A1.**  `calamine`’s range starts at the first _used_ cell, not at A1, so a sheet whose data begins at C3 would lose two columns and two rows of position if dumped naively.  Pad leading empty rows and columns so column _N_ of the CSV is column _N_ of the sheet.  (Gotcha to pin with a test.)
- **Values, not displays.**  Numbers at full precision, unformatted — no thousands separators, no currency symbols, no rounding to the displayed decimals.  Booleans `TRUE`/`FALSE`.  Error cells as their literal text (`#DIV/0!`).  Whether to offer a “displayed text” mode is an open question (§ 9); `calamine` exposes number formats only partly.
- **Dates as ISO 8601** — `YYYY-MM-DD`, or `YYYY-MM-DDTHH:MM:SS` when a time part exists (`~/.claude/rules/dates-and-times.md`: machine-facing dates are ISO).  An Excel date is a serial number with a date format; one `calamine` cannot classify as a date stays a number, and the JSON says which.
- **Merged regions:** the value sits in the top-left cell; the others are blank.  The JSON lists the regions.

### Naming the output

The only consumer convention in hand is Obsidian-Brain’s: `<master>-fromXlsx.csv`, where removing `-from<Ext>` and restoring the extension recovers the master.  The marker follows the master’s extension: `fromXlsx`, `fromXls`, `fromOds`, …  That convention assumes **one CSV per workbook**, and a multi-sheet workbook breaks it.  So:

- A `--name` template with tokens such as `{stem}`, `{FromExt}` (e.g. `fromXlsx`), `{sheet}`, `{index}`.
- **Proposed default:** `{stem}-{FromExt}.csv` when the workbook has one sheet (or one is selected); `{stem}-{FromExt}-{sheet}.csv` per sheet otherwise.  The multi-sheet form needs Obsidian-Brain to amend its rule (the recovery step becomes “strip from `-from<Ext>` onward”).  **Open question for Obsidian-Brain’s audit** (§ 9).
- Sheet names can hold characters illegal or awkward in a filename (`/`, `:`, leading dots, trailing spaces).  Sanitize by a documented rule, and refuse (exit 1) rather than let two sheets collide on one output name.

## 6.  Loud `Unknown` — The Contract That Matters Most

The failure this tool must never have is the one `xlsx2csv` has by default: **a plausible empty CSV where the truth is “could not read”**.  A detector scanning that CSV reports “nothing sensitive”; a reviewer reads “no figures”.  Per `~/.claude/rules/positive-evidence-of-absence.md`, every case below is `Unknown`, and `Unknown` is loud:

| Case | Behavior |
|---|---|
| Encrypted / password-protected workbook | exit 1, naming the file — never an empty dump |
| Not a workbook, truncated, corrupt zip, `.numbers` | exit 1, naming the file and what was expected |
| A **hidden or very-hidden sheet** | **dumped like any other** and flagged in `sheets`, `--json`, and on stderr.  Never skipped by default: the routing rubric’s hidden-salary-tab case is exactly a hidden sheet.  Skipping is an explicit `--visible-only`. |
| A formula cell with **no cached value** (files written by tools that never compute, e.g. some generators) | value `null` in JSON with the formula kept, an empty CSV cell, and a counted warning on stderr — never silently empty.  `check` reports it (exit 3). |
| A sheet whose content is only charts or images | reported as such (empty range, but known to hold objects if `calamine` can tell); never indistinguishable from a sheet that is truly blank |
| An existing-but-empty sheet | legitimate: a header-less empty CSV, exit 0, and listed as empty |

**Exit codes:** `check` uses 3 for findings (a hidden sheet, a missing cached value).  ~~An unreadable sheet inside a readable workbook~~ was listed here as a finding at scaffolding; that contradicted this section’s own last sentence and `positive-evidence-of-absence.md`, so it is exit 1 (corrected 6-Oct-2026, § 10).  `csv` and `cells` exit 1 when a requested workbook or sheet cannot be read — the tool did not run over the data it was told to, so there are no findings.  A batch run over several files reports every file’s outcome in `--json` and exits non-zero if any failed.  Never 3 for a read failure: an orchestrator reads 3 as ordinary drift and proceeds.

## 7.  Path Contracts and Overwriting

Per `~/.claude/rules/cli-exit-codes.md` § Input and Output Paths:

- **A named input that does not exist → exit 1**, naming it.  Never auto-created.
- **The output directory must exist → exit 1 if not.**  `--create-destination <none|final|all>`, default `none`.  Beside-the-workbook output needs no directory creation at all.
- **No clobbering by default.**  An existing output file is refused (exit 1, naming it) unless `--overwrite` is given.  This is not caution for its own sake: under the vault convention, a `-fromXlsx.csv` **that came from its author is kept as received and never rebuilt** — only the vault’s own conversions may be.  The tool cannot tell which is which; the caller can.  The probe gating an overwrite is three-state (`symlink_metadata`, `NotFound` only means absent).
- **`--skip-existing`** for an idempotent hook (the email-indexer case): an existing output is left alone and reported as skipped, exit 0.
- **`--dry-run`** on `csv`: print what would be written, skipped, and refused.
- **Write atomically** — to a temporary file in the destination directory, then rename — so an interrupted run never leaves a truncated CSV that reads as a short sheet.

## 8.  Testing

- **Fixtures are synthetic, always.**  The repo is public; no real KCS or personal workbook is ever committed, not even with figures changed.  Generate fixtures in tests (e.g. `rust_xlsxwriter` as a dev-dependency, which can also produce formula cells _without_ cached values — the § 6 case) or commit small hand-built synthetic files under `tests/fixtures/`.
- **Required cases:** multi-sheet; hidden and very-hidden sheets; data starting away from A1; merged regions; dates, times, and date-formatted numbers; error cells; formulas with and without cached values; Unicode sheet names and cell text (an ‘okina among them); an encrypted workbook; a corrupt zip; an empty sheet; a `.xls` and an `.ods`.
- **Assert on effects**, not only exit codes: the CSV’s bytes (BOM present, A1 anchoring), that a refused overwrite left the old file byte-identical, that a hidden sheet’s file exists.
- **Local acceptance oracle (never committed):** the KCS-Board vault’s
  `FY26/board-meetings/2026-06-13/KCS-Financials-20260613-Treasurer-BudgetVsActualsByClass.xlsx`
  has a hand-made derived copy beside it (`…-fromXlsx.csv`, 70 rows).  `xlsx-dump csv` on the master should reproduce its figures; differences are either the tool’s bug or the one-off’s, and each should be explained.  KCS-Assistant holds the same extraction as `FY26/board-meetings/2026-06-13/Budget_vs_Actuals_FY26_by_class.csv`.

## 9.  Open Questions — For the Implementation Session and the Consumer Audits

1. **Multi-sheet naming** (§ 5) — Obsidian-Brain owns the convention; propose the default to it and get the rule amended before the bulk KCS-Board run.
2. **Where the email hook lives** — KCS-Assistant’s `email-indexer` skill (prompt-level, first version) or `eml-tool` (a plan in its repo, e.g. a `--convert` step after `attachments --extract`).  Decide with both.
3. **A “displayed text” mode** — whether any consumer needs values as the sheet shows them (currency, percentages, rounding).  Default is raw values; add a mode only on a consumer’s stated need.
4. **Library API** — whether Obsidian-Brain’s Rust `brain` crate or eml-tool would rather link `xlsx_dump` as a library than shell out.  The crate is bin + lib from day one, so this is cheap to allow later.
5. **`calamine` itself** — vetted by the `crate-architect` agent before `cargo add` (version, maintenance, feature flags, what it reports for sheet visibility, encrypted files, and charts-only sheets).  Several § 6 behaviors depend on what it can distinguish; where it cannot, the tool says “unknown”, never “absent”.

## 10.  Rulings at the First Implementation (6-Oct-2026)

The § 9 questions as they stand, and the decisions the implementation made.  The user-facing half of each lives in `--help`; this section keeps the why.

**The open questions:**

1. **Multi-sheet naming** — **ruled A by Chris, 6-Oct-2026** (effort `xlsx-dump-workbook-reader-and-its`, ask 0001, filed by Obsidian-Brain): one sheet → `<stem>-fromXlsx.csv`; several → `<stem>-fromXlsx-<Sheet>.csv`; the master is recovered by stripping from `-from<Ext>` onward.  The rejected options were B, always include the sheet (it lengthens 91% of names for nothing: 53 of 58 vault workbooks have one sheet), and C, `<stem>-<Sheet>-fromXlsx.csv` (it breaks master recovery).  Obsidian-Brain amends its rule template and the Board vault’s copy, with the sanitizing and refuse-on-collision rules of `csv --help`.
2. **The email hook** — prompt-level in KCS-Assistant’s `email-indexer` skill, as `xlsx-dump csv <new workbooks> --skip-existing --json`, output beside each workbook (KCS-Assistant, 6-Oct-2026).  KCS-Assistant would rather eml-tool not depend on xlsx-dump; an `attachments --extract --convert` is a plan in eml-tool’s repo only if it earns its keep.
3. **Displayed text** — no consumer needs it; both asked for raw values, with numbers free of float noise and dates as ISO.  No mode.
4. **Library API** — shell out (Obsidian-Brain: `brain` stays narrow, “specialized CLIs via Bash, not wrappers”).  The crate stays bin + lib, but no stable API is promised.
5. **calamine** — 0.36.1, vetted (crate-architect, 6-Oct-2026), no features enabled.  What it can and cannot tell apart decided several behaviors below; the limits are in `xlsx-dump --help`.

**Decisions:**

- **The format is sniffed, not taken from the extension.**  `open_workbook_auto` drops the “password protected” error for an unexpected extension, so the tool reads the first bytes: an OLE container is tried as an encrypted OOXML package, then as `.xls`; a zip is chosen by extension.  A container with **zero sheets** (calamine opens some damaged packages cleanly) is exit 1, and every calamine call runs under `catch_unwind`, because calamine can panic on malformed binary records.
- **`.xlsx` is streamed** through calamine’s cell reader: one pass yields each cell’s cached value and formula (a formula with an empty value is exactly “no cached value”), and no dense grid is allocated.  Other formats read calamine’s value and formula grids and merge them by absolute position — their starts differ.
- **Numbers are written at 15 significant digits, then in shortest form.**  Shortest round-trip alone leaves a formula’s float noise in place: a cached `SUM` in the oracle is a double whose shortest text has 17 digits, while Excel displays it at two decimals.  Fifteen digits is Excel’s own precision, so rounding there loses nothing the application kept (`=0.1+0.2` caches `0.30000000000000004` and is written `0.3`).  The rest of the oracle’s 17-digit noise was a 17-digit _print_ of clean doubles, which shortest round-trip alone already fixes.
- **`--sheet <NAME>` and `--sheet-index <N>`** replace § 4’s `--sheet <NAME|INDEX>`: a sheet may be named `2`, and an agent must not have to guess which it gets.
- **The default name follows the workbook’s sheet count, never the selection** (§ 5 said “or one is selected”): selecting one sheet of three must not produce `<stem>-fromXlsx.csv`, or the same sheet would have two names depending on how it was produced.
- **A workbook converts all or nothing.**  Every output is planned — read, rendered, named, collision-checked, probed three ways — before any is written; one refusal withholds the rest, so a workbook is never half-converted.  A batch still carries on past a failed workbook and exits 1 at the end (KCS-Assistant’s ask).
- **A `--name` template must end in `.csv`** — which is also what keeps `--overwrite` from ever replacing a workbook with its own CSV.
- **A chart sheet is not written** (its “CSV” would read as a blank sheet); `csv` reports it as `no-cells`, `check` as a finding, and naming it with `--sheet` is exit 1.  Charts and images on an ordinary worksheet are not detected yet: `plans/plan-0001`.
- **A one-field empty row is `""`**, not a blank line (the `csv` crate’s rule), so a reader that skips blank lines cannot shift a one-column sheet’s rows.  Line endings are LF.
- **A CSV grid above 50 million cells is refused** (a stray value at `XFD1048576` would pad to gigabytes of commas); `cells` reads such a sheet sparsely.
- **A pre-commit review (rust-reviewer, 6-Oct-2026) found five hard and eight medium faults, all fixed before the first commit, each pinned by a test that fails when its fix is reverted:**
  - `--overwrite` could replace an input workbook with its own CSV (a workbook named `.csv` opens, because the format is sniffed); the `.csv`-suffix rule on `--name` was not the guard it was claimed to be.  Outputs are now refused when they are, by file identity, one of the run’s inputs.
  - The in-run collision set was keyed on path spelling, so `--overwrite` let two outputs whose names the filesystem treats as one (`d/` vs `./d/`; NFC vs NFD `Café`) silently become one file.  The key now uses the canonical directory, and every write checks it is not replacing a file this run already wrote.
  - `check` exited 3 for a sheet it could not read (§ 6 above, corrected).
  - An `.xlsx` shared formula whose derived cell precedes its anchor was dropped — with no cached value, the cell vanished from every dump.  Derived cells are now resolved after the pass, and an unresolvable one is counted as unrecognised.
  - `--sheet X --visible-only` on a hidden `X` was a silent empty success; it is exit 1.
  - Also: a dry run now agrees with `--create-destination`; a symlink at an output path is refused (DESIGN § 7 promised an lstat probe; cli-contract’s probe stats through, its bug-0007); outputs take the umask mode, not the temporary file’s 0600; a closed stdout no longer turns `csv` into exit 101 or swallows its errors; `check` reports a grid too large for `csv` (`oversized-grid`); a relative single-component `--output-dir` with `final` works (cli-contract bug-0004, worked around by making the path absolute); a caught reader panic poisons the workbook so no further sheet is read through it.
  - **Softened, not fixed:** “all or nothing per workbook” holds at planning time only; a write that fails afterwards leaves the files already written, reported (`csv --help`).  Undoing them would add a delete path to a reader, for a case (a full disk mid-workbook) that is loud anyway.  And only `.xlsx` is read without a dense grid; a far stray value in `.xls`/`.xlsb`/`.ods` can exhaust memory (`--help` says so).
- **The path contract comes from `cli-contract`**, the portfolio’s shared crate, rather than a local copy.  That crate is private (`publish = false`) and this repo public; **Chris ruled, 6-Oct-2026 (ask 0002), for a path dependency until `cli-contract` is released**, accepting that a clone of this repo does not build off his Mac meanwhile.  When `cli-contract` is published, the dependency becomes a version (or git) dependency — one line of `Cargo.toml`.

**Acceptance (local, never committed):** the § 8 oracle reproduces: all 70 non-blank rows of the hand-made `-fromXlsx.csv` match cell for cell (numbers compared at 15 digits), and the output takes the same name.  `check` over all 227 workbooks in the KCS trees and vaults: 225 clean, one with uncached formulas, one genuinely password-protected file reported as encrypted (exit 1) — and a `csv --dry-run` over the same set planned 790 outputs with no read failure.
