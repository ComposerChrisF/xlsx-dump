# Bug: Sheet Names Differing Only in Unicode Normalization Half-Convert a Workbook

**Severity:** medium — no data loss; a workbook is left half-converted, loudly (exit 1).
**Found:** 7-Oct-2026, `/audit-cli` on 0.2.0 (also anticipated in `DESIGN.md` § 10, which names the gap but overstates the guarantee).

## Contract Violated

`csv --help`: “Two sheets whose names collide (case-insensitively) are refused, never overwritten” and “Every output of a workbook is planned before any is written: if one would be refused, none of that workbook’s files is written.”

## Cause

The planning-time collision key in `src/commands/csv.rs` is the canonical directory plus the **case-folded** file name, but not the **Unicode-normalized** one.  The default macOS volume (APFS) treats `Café` (NFC, U+00E9) and `Café` (NFD, `e` + U+0301) as one name, so the planner sees two distinct outputs.  The collision is caught only at write time — by `persist_noclobber` (`File exists`), or under `--overwrite` by `write_atomic`’s written-identity backstop — after the first sheet’s CSV is already written.

## Reproduction

A workbook with two sheets, `Caf\u{e9}` and `Cafe\u{301}` (the test fixture `tests/common/mod.rs::nfc_nfd` builds one):

```
xlsx-dump csv Accents.xlsx
```

Result: `wrote …-fromXlsx-Café.csv`, then `refused …: File exists (os error 17)`, exit 1 — one sheet’s CSV is left on disk without its partner.  The existing test `unicode_normalization_twins_never_silently_merge` asserts only that no two outputs merge silently, which holds; nothing asserts the all-or-nothing promise.

## Fix Direction

Normalize the file name (NFC) before case-folding it in the collision key, so the clash is refused at planning time and nothing is written.  That needs a normalization table — the `unicode-normalization` crate, vetted with `/add-crate` first — or, if a new dependency is unwelcome, soften `--help` to say a normalization-only clash is caught at write time.  Strengthen the test to assert that no file of the workbook exists afterwards.
