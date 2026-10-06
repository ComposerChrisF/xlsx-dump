# xlsx-dump — TODO

## Next

- [ ] Vet `calamine` with the `crate-architect` agent (via `/add-crate`): version, feature flags, and what it can tell apart — sheet visibility, encrypted files, charts-only sheets, formulas without cached values (`DESIGN.md` § 9 q. 5).
- [ ] Implement `sheets` (with `--json`), replacing the scaffold stub.
- [ ] Implement `csv`: BOM, A1 anchoring, ISO dates, the naming template, no-clobber, `--skip-existing`, `--dry-run`, atomic writes (`DESIGN.md` §§ 5, 7).
- [ ] Implement `cells` (JSONL) and `check` (exit 3 on findings).
- [ ] Synthetic fixture suite per `DESIGN.md` § 8; then the local acceptance run against the KCS-Board treasurer workbook.
- [ ] Ask each consumer to audit the first release (`DESIGN.md` § 2): Obsidian-Brain (multi-sheet naming, hidden sheets), the KCS-Board vault (the pending `-fromXlsx.csv` copies), KCS-Assistant (the `email-indexer` hook), eml-tool (whether the hook lives there).
- [ ] Run `/audit-cli` on the first release.
