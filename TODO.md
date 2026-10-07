# xlsx-dump — TODO

Both consumer audits of 0.2.0 passed with no defects (6-Oct-2026): Obsidian-Brain converted the Board vault (23 workbooks, 27 CSVs; Board vault `11b4505`), and KCS-Assistant wired its `email-indexer` hook (its `f97d39c`).  No real workbook so far holds a date cell, so the date path is covered by the synthetic fixtures only.

## Next

- [ ] When `cli-contract` is released, replace its path dependency with a version or git dependency so a clone builds (Chris, ask 0002: path dependency until then; `DESIGN.md` § 10).
- [ ] `plans/plan-0002`: `--numbers format`, rounding a number to its cell's fixed decimals — KCS-Assistant's stated need from its 0.2.0 audit (QuickBooks drift such as `12.3399999999999` in 2-decimal cells).
- [ ] Run `/audit-cli` on the first release.
- [ ] `plans/plan-0001`: detect charts and images on ordinary `.xlsx` worksheets.
- [ ] A readable `.xls` fixture: rust_xlsxwriter cannot write BIFF, so `.xls` reading is covered only by the local corpus run (`DESIGN.md` § 10) and the encrypted-OLE test; a hand-built minimal BIFF8 stream would pin it.
- [ ] Read `.xlsb` cell by cell too (calamine has an `.xlsb` cells reader), so a far stray value there cannot exhaust memory as it can today; `.xls` and `.ods` have no streaming reader, so pre-check their dimensions if calamine exposes them (`DESIGN.md` § 10).
- [ ] An `.xlsb` fixture, for the same reason as the `.xls` one.
- [ ] Fix `bugs/bug-0002` (a Unicode-normalization sheet-name clash half-converts a workbook).
- [ ] Small `/audit-cli` findings (7-Oct-2026): give `skipped` a `reason` in `--json` (`exists` / `symlink` / `dangling symlink`), since a dangling symlink now reads as an existing derived copy; emit a JSON error document when the destination check fails under `--json`; have a dry run say `would create <dir>` under `final`/`all`; derive the dry run's `dir_exists` from `check_destination`'s three-state result instead of `is_dir()`; refuse, or document, `--overwrite` over a non-regular file such as a FIFO; give the read-only subcommands an exit table without the `csv`-only lines.
- [ ] The mount-root floor is only as good as cli-contract's: its `bugs/bug-0009` (the floor matches the spelling `Volumes`, so `/volumes/X` passes).
