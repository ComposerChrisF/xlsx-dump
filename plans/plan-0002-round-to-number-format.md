# Plan: Round Numbers to Their Cell’s Number Format, on Request

## Problem

xlsx-dump writes numbers at Excel’s precision, 15 significant digits (`DESIGN.md` § 10).  That cleans a single operation’s float noise (`=0.1+0.2` → `0.3`), but not drift that QuickBooks and similar exporters accumulate over many operations before the value is stored.  KCS-Assistant’s audit of 0.2.0 (6-Oct-2026) found 10 such values in two real QuickBooks P&L-by-class workbooks — values of the shape `12.3399999999999` or `-45.6700000000001`, each in a cell formatted to two decimals, where every reader means `12.34`.  (The examples here are synthetic; this repo is public, and the real figures stay in the workbooks.)  Obsidian-Brain’s Board-vault run the same day saw the same drift in formula results and recorded it as a note, with no stated need of its own.  The values are faithful to the stored doubles, so no precision rule can tell them from a genuine 15-digit value; only the cell’s number format says what the author meant.

This is the “stated need” `DESIGN.md` § 9 q. 3 asked for before adding any mode beyond raw values.  KCS-Assistant’s `email-indexer` skill currently tells the reader to treat such values as the rounded amount — a per-session workaround.

## Proposed Change

An opt-in flag on `csv` and `cells`: **`--numbers raw|format`**, default `raw` (today’s behavior, unchanged).

- `format`: a number in a cell whose number format fixes its decimal places is rounded to those places — `0.00`, `#,##0.00`, currency and accounting formats → 2; `0.00%` → 4 (a percentage shows the value × 100); `0` → 0.  The output is still a raw number: no separators, currency symbols, or `%`.
- A cell in `General`, or any format whose decimals are not fixed (scientific, fractions, `#,##0.##`), is left at 15 significant digits.
- Dates and durations are unaffected (they already go by their format).
- `cells` keeps the stored value in a new field `stored` whenever rounding changed it, so a detector can still see the exact double.  Field additions only, per the JSONL contract.
- The flag name is a proposal; `--precision format` (KCS-Assistant’s wording) is the alternative.

## Implementation Notes

- **calamine does not expose a cell’s number format.**  It reads `styles.xml` only to classify dates (`formats.rs`), and its cell reader does not surface the style index (`s=`).  Two routes, to be weighed with the `crate-architect` agent:
  1. **Read it ourselves for `.xlsx`:** parse `xl/styles.xml` (`numFmts` and `cellXfs`) with `quick-xml` (already compiled through calamine) to map a style index to a format code, and take each cell’s `s=` from a second, lightweight pass over the sheet XML through `zip` (also already compiled).  It is cheap in dependencies but a second parser of the sheet.
  2. **`calamine-styles`**, a fork of calamine that parses number formats, fonts, fills, and more for `.xlsx`.  It would make one pass do everything, but swaps a 14M-download crate for a fork; vet its maintenance and how far it trails upstream.
- Decimal places from a format code: built-in IDs (2, 4, 7, 8, 10, 39, 40, 43, 44, …) by table; a custom code from its first section, counting `0`/`#`/`?` after the decimal point, skipping quoted text, `\`-escapes, and `[…]` blocks.  A code that cannot be parsed leaves the number raw — `Unknown` keeps the value, never guesses.
- Scope v1 to `.xlsx`/`.xlsm` (all of the KCS corpus but one file).  For other formats, `--numbers format` leaves values raw, and `--help` says so.
- Acceptance: KCS-Assistant’s two workbooks (in its `inbox/attachments/`, read in place, never copied) — all 10 drifting values come out at two decimals, and no other value changes.  Synthetic fixtures for every built-in ID in the table, a custom code, a percentage, and a `General` cell.

## Why Not a Workaround

The workaround in use is a sentence in a skill prompt asking each reading session to round mentally — exactly the repeated judgment a tool should take over, and one that an agent comparing figures across two CSVs will eventually forget.  A post-processing script would need the cell formats, which only a workbook reader can see.
