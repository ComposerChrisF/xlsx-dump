# Plan: Report Charts and Images on Ordinary Worksheets

## Problem

`DESIGN.md` § 6 asks that a sheet whose content is only charts or images be “never indistinguishable from a sheet that is truly blank”.  v0.1 meets that for a **chart sheet** (calamine reports its type), but not for an ordinary **worksheet** that holds a chart or picture and no cells: calamine exposes no per-sheet drawing information in any format, so such a sheet reads as empty.  Today `sheets` says so honestly (“charts or images on a worksheet are not inspected”), which is `Unknown` stated aloud, not a false `Absent`.  But a reader is still left not knowing whether a board-packet sheet holding only a pasted chart has content.

## Proposed Change

For `.xlsx`/`.xlsm` (nearly the whole real corpus: 226 of 227 workbooks at v0.1), report per worksheet whether it carries drawings:

- `sheets --json`: a new field `drawings`: `true` / `false` for `.xlsx`, `null` (unknown) for other formats.  The text table shows it as a column.
- `check`: a new finding kind, `drawings-only` — a worksheet with no cells but with drawings, whose content is therefore not in any dump.
- The empty-sheet note changes from “not inspected” to the fact, where the fact is known.

## Implementation Notes

- Read the package directly with `zip` (already compiled as calamine’s dependency, so moving it from `[dev-dependencies]` to `[dependencies]` adds nothing to the build).
- Map sheet name → part: `xl/workbook.xml` `<sheet name r:id>` → `xl/_rels/workbook.xml.rels` target → `xl/worksheets/_rels/sheetN.xml.rels`; a relationship whose `Type` ends in `/drawing` means drawings (charts and pictures both live there).  `quick-xml` is also already compiled via calamine.
- Every step is three-state: a part that is missing where it should exist, or unparsable, is `null` (unknown), never `false`.
- Fixture: rust_xlsxwriter can insert a chart or image into a worksheet with no cells.

## Why Not a Workaround

The consumers (Obsidian-Brain’s detectors, the vault registers) read `sheets --json` and `check` mechanically; a hand inspection of the zip in each session is exactly the repeated mechanical operation the portfolio turns into a CLI feature.
