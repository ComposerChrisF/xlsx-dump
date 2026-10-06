# xlsx-dump — TODO

## Next

- [ ] When `cli-contract` is released, replace its path dependency with a version or git dependency so a clone builds (Chris, ask 0002: path dependency until then; `DESIGN.md` § 10).
- [ ] Ask each consumer to audit the first release (`DESIGN.md` § 2): Obsidian-Brain (naming, the `cells` JSONL for its detectors; it will run the Board-vault bulk conversion), KCS-Assistant (the `email-indexer` hook, `csv --skip-existing --json`).  eml-tool: no change wanted (`DESIGN.md` § 10).
- [ ] Run `/audit-cli` on the first release.
- [ ] `plans/plan-0001`: detect charts and images on ordinary `.xlsx` worksheets.
- [ ] A readable `.xls` fixture: rust_xlsxwriter cannot write BIFF, so `.xls` reading is covered only by the local corpus run (`DESIGN.md` § 10) and the encrypted-OLE test; a hand-built minimal BIFF8 stream would pin it.
- [ ] Read `.xlsb` cell by cell too (calamine has an `.xlsb` cells reader), so a far stray value there cannot exhaust memory as it can today; `.xls` and `.ods` have no streaming reader, so pre-check their dimensions if calamine exposes them (`DESIGN.md` § 10).
- [ ] An `.xlsb` fixture, for the same reason as the `.xls` one.
