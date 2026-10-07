# Bug: `--overwrite` Can Replace an Input Workbook That Could Not Be Inspected

**Severity:** high — data loss (an input workbook is replaced by a CSV), under `--overwrite` only.
**Found:** 7-Oct-2026, `/audit-cli` on 0.2.0; reproduced independently the same day.

## Contract Violated

`csv --help`: “Even with `--overwrite`, an output is refused if it is one of the run’s input workbooks.”  And `positive-evidence-of-absence.md`: a probe that gates a destructive write must answer Present / Absent / **Unknown**, and Unknown must never license the write.

## Cause

`src/commands/csv.rs` builds the protected set of input identities with

```rust
.filter_map(|f| FileId::of(f).ok().flatten())
```

so an input whose `stat` fails for any reason other than `NotFound` — `EACCES`, `ENAMETOOLONG`, `ELOOP`, `EIO` — is silently dropped from the set.  “Could not look” is read as “not an input”.  That input then fails to open (exit 1, named), but the run carries on, and another workbook in the same batch may write an output onto the very file the dropped spelling named.

## Reproduction (synthetic; no chmod needed)

```
mkdir src && cp One.xlsx R.csv && cp One.xlsx src/R.xlsx      # R.csv is a real workbook
xlsx-dump csv --overwrite --output-dir . --name '{stem}.csv' \
    "$(printf './%.0s' $(seq 520))R.csv" src/R.xlsx
```

The first spelling of `R.csv` exceeds `PATH_MAX`, so its `stat` is `ENAMETOOLONG` and it leaves the protected set.  `src/R.xlsx` then renders to `./R.csv`, the planner sees a regular file that is not a known input, `--overwrite` permits the clobber, and the workbook `R.csv` is replaced by a CSV (verified: 270,501 bytes of workbook became a BOM-prefixed CSV).  The run exits 1 overall — but only after the data is gone.  A `chmod 000` directory in the first spelling (`locked/../R.csv`) does the same with `EACCES`.  `--dry-run` reports `would write` for it.

## Fix Direction

Collect each input’s identity three ways.  If any input is `Unknown`, the protected set is incomplete, so no output of the run may overwrite an existing file: refuse every such overwrite (or abort a `--overwrite` run before any write), naming the input that could not be inspected.  Pin it with a test on the side effect — the input’s bytes unchanged — using the `ENAMETOOLONG` spelling, so the test needs no chmod and works as root.  Revert the fix and confirm the test fails.
