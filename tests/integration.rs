mod common;

use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("xlsx-dump").unwrap()
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().unwrap()
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

// --- help -------------------------------------------------------------------------------------

#[test]
fn help_documents_exit_codes_everywhere() {
    for args in [
        vec!["--help"],
        vec!["sheets", "--help"],
        vec!["csv", "--help"],
        vec!["cells", "--help"],
        vec!["check", "--help"],
    ] {
        let out = run(&args);
        assert_eq!(code(&out), 0);
        assert!(stdout(&out).contains("Exit codes:"), "{args:?}");
    }
}

// --- sheets -----------------------------------------------------------------------------------

#[test]
fn sheets_reports_visibility_kind_range_and_counts() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["sheets", s(&wb), "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["format"], "xlsx");
    let sheets = v["sheets"].as_array().unwrap();
    let names: Vec<_> = sheets.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Budget", "Dates", "Secret", "Vault", "Empty"]);
    let vis: Vec<_> = sheets
        .iter()
        .map(|s| s["visibility"].as_str().unwrap())
        .collect();
    assert_eq!(
        vis,
        ["visible", "visible", "hidden", "very-hidden", "visible"]
    );
    let budget = &sheets[0];
    assert_eq!(budget["range"], "C3:F7");
    assert_eq!(budget["rows"], 7);
    assert_eq!(budget["cols"], 6);
    assert_eq!(budget["formulas"], 2);
    assert_eq!(budget["uncached_formulas"], 0);
    assert_eq!(budget["merged_regions"], 1);
    let empty = &sheets[4];
    assert_eq!(empty["range"], Value::Null);
    assert_eq!(empty["readable"], true);
}

#[test]
fn sheets_text_lists_every_sheet() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["sheets", s(&wb)]);
    assert_eq!(code(&out), 0);
    let text = stdout(&out);
    assert!(text.contains("very-hidden"));
    assert_eq!(text.lines().count(), 6);
}

#[test]
fn chart_sheet_is_reported_never_as_blank() {
    let dir = TempDir::new().unwrap();
    let wb = common::with_chartsheet(dir.path());
    let v = json(&run(&["sheets", s(&wb), "--json"]));
    let plot = &v["sheets"][1];
    assert_eq!(plot["kind"], "chart-sheet");
    assert_eq!(plot["readable"], false);

    // csv by default writes the worksheet and reports the chart sheet as having no cells.
    let out = run(&["csv", s(&wb), "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let outputs = &json(&out)["files"][0]["outputs"];
    assert_eq!(outputs[0]["status"], "written");
    assert_eq!(outputs[1]["status"], "no-cells");
    assert!(!dir.path().join("Chart-fromXlsx-Plot.csv").exists());

    // Asked for by name, it is an error.
    let out = run(&["csv", s(&wb), "--sheet", "Plot", "--overwrite"]);
    assert_eq!(code(&out), 1);
    assert!(stderr(&out).contains("chart-sheet"));
}

// --- csv: content -----------------------------------------------------------------------------

#[test]
fn csv_writes_every_sheet_with_bom_anchored_at_a1() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["csv", s(&wb)]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));

    let budget = common::csv_text(&dir.path().join("Multi-fromXlsx-Budget.csv"));
    let rows: Vec<&str> = budget.lines().collect();
    assert_eq!(rows.len(), 7);
    assert_eq!(rows[0], ",,,,,");
    assert_eq!(rows[2], ",,Item,Amount,,");
    assert_eq!(rows[3], ",,\"Robes, Hawai‘i\",1234.56,TRUE,");
    assert_eq!(rows[4], ",,Total,1234.56,,#DIV/0!");
    assert_eq!(rows[6], ",,Merged note,,,");

    let dates = common::csv_text(&dir.path().join("Multi-fromXlsx-Dates.csv"));
    assert_eq!(
        dates.lines().collect::<Vec<_>>(),
        [
            "2026-06-13",
            "2026-06-13T09:30:00",
            "18:00:00",
            "46186",
            "PT36H30M"
        ]
    );

    // Hidden sheets are written like any other, and said so on stderr.
    assert_eq!(
        common::csv_text(&dir.path().join("Multi-fromXlsx-Secret.csv")),
        "salary\n"
    );
    assert!(dir.path().join("Multi-fromXlsx-Vault.csv").exists());
    let err = stderr(&out);
    assert!(err.contains("\"Secret\" is hidden"), "{err}");
    assert!(err.contains("\"Vault\" is very-hidden"), "{err}");

    // An empty sheet is an empty CSV: the BOM alone.
    assert_eq!(
        std::fs::read(dir.path().join("Multi-fromXlsx-Empty.csv")).unwrap(),
        b"\xEF\xBB\xBF"
    );
}

#[test]
fn single_sheet_workbook_gets_the_plain_vault_name() {
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "KCS-Budget.xlsx");
    let out = run(&["csv", s(&wb)]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(
        common::csv_text(&dir.path().join("KCS-Budget-fromXlsx.csv")),
        "only,1\n"
    );
}

#[test]
fn selecting_one_sheet_keeps_the_multi_sheet_name() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["csv", s(&wb), "--sheet-index", "2"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(dir.path().join("Multi-fromXlsx-Dates.csv").exists());
    assert!(!dir.path().join("Multi-fromXlsx.csv").exists());
    assert!(!dir.path().join("Multi-fromXlsx-Budget.csv").exists());
}

#[test]
fn no_bom_and_name_template() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&[
        "csv",
        s(&wb),
        "--sheet",
        "Secret",
        "--no-bom",
        "--name",
        "{stem}.{index}.{sheet}.csv",
    ]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(
        std::fs::read(dir.path().join("Multi.3.Secret.csv")).unwrap(),
        b"salary\n"
    );
}

#[test]
fn visible_only_skips_hidden_sheets() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["csv", s(&wb), "--visible-only"]);
    assert_eq!(code(&out), 0);
    assert!(!dir.path().join("Multi-fromXlsx-Secret.csv").exists());
    assert!(!dir.path().join("Multi-fromXlsx-Vault.csv").exists());
    assert!(dir.path().join("Multi-fromXlsx-Budget.csv").exists());
}

#[test]
fn uncached_formula_is_an_empty_cell_and_a_warning() {
    let dir = TempDir::new().unwrap();
    let wb = common::uncached(dir.path());
    let out = run(&["csv", s(&wb)]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(
        common::csv_text(&dir.path().join("Uncached-fromXlsx.csv")),
        // A one-field empty row is `""`, not a blank line, which many readers would skip and so
        // shift every later row.
        "21\n\"\"\n"
    );
    assert!(stderr(&out).contains("1 formula(s) with no cached value"));
}

#[test]
fn date_1904_system_is_honoured() {
    let dir = TempDir::new().unwrap();
    let wb = common::date1904(dir.path());
    assert_eq!(code(&run(&["csv", s(&wb)])), 0);
    assert_eq!(
        common::csv_text(&dir.path().join("Date1904-fromXlsx.csv")),
        "2026-06-13\n"
    );
}

#[test]
fn ods_is_read_with_hidden_sheets_and_normalized_formulas() {
    let dir = TempDir::new().unwrap();
    let wb = common::ods(dir.path());
    let v = json(&run(&["sheets", s(&wb), "--json"]));
    assert_eq!(v["format"], "ods");
    assert_eq!(v["sheets"][1]["visibility"], "hidden");
    assert_eq!(v["sheets"][0]["merged_regions"], Value::Null); // unknown for .ods
    assert_eq!(v["sheets"][0]["uncached_formulas"], 1);

    let out = run(&["csv", s(&wb)]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(
        common::csv_text(&dir.path().join("Sheet-fromOds-Vis.csv")),
        ",2,4,\n2026-10-06,PT13H45M00S,Mālama ‘āina,\n"
    );

    let out = run(&["cells", s(&wb), "--formulas-only"]);
    let records: Vec<Value> = stdout(&out)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["formula"], "=[.B1]*2");
    assert_eq!(records[1]["cached"], false);
}

// --- csv: overwriting and paths ---------------------------------------------------------------

#[test]
fn existing_output_is_refused_and_left_untouched_and_siblings_withheld() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let existing = dir.path().join("Multi-fromXlsx-Dates.csv");
    std::fs::write(&existing, b"the author's own copy").unwrap();

    let out = run(&["csv", s(&wb), "--json"]);
    assert_eq!(code(&out), 1);
    assert_eq!(std::fs::read(&existing).unwrap(), b"the author's own copy");
    // No other file of the workbook was written: never half-converted.
    assert!(!dir.path().join("Multi-fromXlsx-Budget.csv").exists());
    let v = json(&out);
    assert_eq!(v["files"][0]["status"], "failed");
    let statuses: Vec<_> = v["files"][0]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        ["withheld", "refused", "withheld", "withheld", "withheld"]
    );
}

#[test]
fn skip_existing_leaves_it_and_writes_the_rest() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let existing = dir.path().join("Multi-fromXlsx-Dates.csv");
    std::fs::write(&existing, b"keep").unwrap();
    let out = run(&["csv", s(&wb), "--skip-existing"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
    assert!(dir.path().join("Multi-fromXlsx-Budget.csv").exists());
    assert!(stdout(&out).contains("skipped"));
}

#[test]
fn overwrite_replaces() {
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "One.xlsx");
    let target = dir.path().join("One-fromXlsx.csv");
    std::fs::write(&target, b"stale").unwrap();
    assert_eq!(code(&run(&["csv", s(&wb), "--overwrite"])), 0);
    assert_eq!(common::csv_text(&target), "only,1\n");
}

#[test]
fn dry_run_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["csv", s(&wb), "--dry-run", "--json"]);
    assert_eq!(code(&out), 0);
    assert_eq!(
        json(&out)["files"][0]["outputs"][0]["status"],
        "would-write"
    );
    let csvs = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "csv")
        })
        .count();
    assert_eq!(csvs, 0);
}

#[test]
fn colliding_sheet_names_are_refused() {
    let dir = TempDir::new().unwrap();
    let wb = common::colliding(dir.path());
    let out = run(&["csv", s(&wb)]);
    assert_eq!(code(&out), 1);
    assert!(stdout(&out).contains("same file name"), "{}", stdout(&out));
    assert!(!dir.path().join("Collide-fromXlsx-Data.csv").exists());
}

#[test]
fn missing_output_dir_is_exit_1_and_not_created() {
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "One.xlsx");
    let missing = dir.path().join("nope");
    let out = run(&["csv", s(&wb), "--output-dir", s(&missing)]);
    assert_eq!(code(&out), 1);
    assert!(!missing.exists());

    let out = run(&[
        "csv",
        s(&wb),
        "--output-dir",
        s(&missing),
        "--create-destination",
        "final",
    ]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(missing.join("One-fromXlsx.csv").exists());
}

// --- loud Unknown -----------------------------------------------------------------------------

#[test]
fn unreadable_inputs_are_exit_1_naming_the_file() {
    let dir = TempDir::new().unwrap();
    let text = dir.path().join("Notes.xlsx");
    std::fs::write(&text, "not a workbook\n").unwrap();
    let numbers = dir.path().join("Budget.numbers");
    std::fs::write(&numbers, "PK\x03\x04whatever").unwrap();
    let cases = [
        (common::encrypted(dir.path()), "encrypted"),
        (common::truncated(dir.path()), "not a readable .xlsx"),
        (text, "not a workbook"),
        (numbers, "Apple Numbers"),
        (dir.path().join("Missing.xlsx"), "does not exist"),
    ];
    for (path, expect) in cases {
        for sub in ["sheets", "csv", "cells", "check"] {
            let out = run(&[sub, s(&path)]);
            assert_eq!(code(&out), 1, "{sub} {}", path.display());
            let err = stderr(&out);
            assert!(err.contains(expect), "{sub} {}: {err}", path.display());
            assert!(
                err.contains(path.file_name().unwrap().to_str().unwrap()),
                "{err}"
            );
        }
    }
    let csvs = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "csv")
        })
        .count();
    assert_eq!(csvs, 0, "an unreadable workbook must never produce a CSV");
}

#[test]
fn batch_converts_the_good_and_reports_each_failure() {
    let dir = TempDir::new().unwrap();
    let good = common::single(dir.path(), "Good.xlsx");
    let locked = common::encrypted(dir.path());
    let broken = common::truncated(dir.path());
    let out = run(&["csv", s(&locked), s(&good), s(&broken), "--json"]);
    assert_eq!(code(&out), 1);
    assert!(dir.path().join("Good-fromXlsx.csv").exists());
    let v = json(&out);
    let statuses: Vec<_> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["failed", "ok", "failed"]);
    assert!(
        v["files"][0]["error"]
            .as_str()
            .unwrap()
            .contains("encrypted")
    );
}

// --- cells ------------------------------------------------------------------------------------

#[test]
fn cells_records_carry_sheet_visibility_formulas_and_merges() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["cells", s(&wb)]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let records: Vec<Value> = stdout(&out)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let find = |sheet: &str, r: &str| {
        records
            .iter()
            .find(|v| v["sheet"] == sheet && v["ref"] == r)
            .unwrap_or_else(|| panic!("{sheet}!{r} missing"))
    };
    let total = find("Budget", "D5");
    assert_eq!(total["type"], "number");
    assert_eq!(total["value"], 1234.56);
    assert_eq!(total["formula"], "=SUM(D4:D4)");
    assert_eq!(total["cached"], true);
    assert_eq!(total["row"], 5);
    assert_eq!(total["col"], 4);
    assert_eq!(find("Budget", "C7")["merged"], "C7:E7");
    assert_eq!(find("Budget", "F5")["type"], "error");
    let date = find("Dates", "A1");
    assert_eq!(date["type"], "date");
    assert_eq!(date["value"], "2026-06-13");
    assert_eq!(date["serial"], 46186.0);
    assert_eq!(find("Dates", "A4")["type"], "number");
    assert_eq!(find("Dates", "A5")["type"], "duration");
    let secret = find("Secret", "A1");
    assert_eq!(secret["visibility"], "hidden");
    assert_eq!(secret["sheet_index"], 3);
    assert_eq!(find("Vault", "A1")["visibility"], "very-hidden");
}

#[test]
fn cells_marks_an_uncached_formula() {
    let dir = TempDir::new().unwrap();
    let wb = common::uncached(dir.path());
    let out = run(&["cells", s(&wb), "--formulas-only"]);
    assert_eq!(code(&out), 0);
    let v: Value = serde_json::from_str(stdout(&out).lines().next().unwrap()).unwrap();
    assert_eq!(v["type"], "none");
    assert_eq!(v["value"], Value::Null);
    assert_eq!(v["cached"], false);
    assert_eq!(v["formula"], "=A1*2");
}

// --- check ------------------------------------------------------------------------------------

#[test]
fn check_exit_codes() {
    let dir = TempDir::new().unwrap();
    let clean = common::single(dir.path(), "Clean.xlsx");
    assert_eq!(code(&run(&["check", s(&clean)])), 0);

    let multi = common::multi(dir.path());
    let out = run(&["check", s(&multi), "--json"]);
    assert_eq!(code(&out), 3);
    let kinds: Vec<_> = json(&out)["files"][0]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["hidden-sheet", "very-hidden-sheet"]);

    let uncached = common::uncached(dir.path());
    let out = run(&["check", s(&uncached), "--json"]);
    assert_eq!(code(&out), 3);
    assert_eq!(
        json(&out)["files"][0]["findings"][0]["kind"],
        "uncached-formulas"
    );

    // A read failure outranks findings, and every file is still checked.
    let locked = common::encrypted(dir.path());
    let out = run(&["check", s(&multi), s(&locked), "--json"]);
    assert_eq!(code(&out), 1);
    assert_eq!(json(&out)["files"][0]["status"], "findings");
    assert_eq!(json(&out)["files"][1]["status"], "failed");
}

// --- usage errors -----------------------------------------------------------------------------

#[test]
fn usage_errors_are_exit_2() {
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "One.xlsx");
    for args in [
        vec!["csv", s(&wb), "--overwrite", "--skip-existing"],
        vec!["csv", s(&wb), "--sheet-index", "0"],
        vec!["csv", s(&wb), "--name", "{stem}-{nope}.csv"],
        vec!["csv", s(&wb), "--name", "{stem}.xlsx"],
        vec!["csv", s(&wb), "--name", "sub/{stem}.csv"],
        vec!["csv", s(&wb), "--create-destination", "all"],
        vec!["check"],
    ] {
        assert_eq!(code(&run(&args)), 2, "{args:?}");
    }
}

#[test]
fn a_sheet_the_workbook_lacks_is_exit_1() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    let out = run(&["cells", s(&wb), "--sheet", "Nope"]);
    assert_eq!(code(&out), 1);
    assert!(stderr(&out).contains("\"Budget\""));
    assert_eq!(code(&run(&["csv", s(&wb), "--sheet-index", "9"])), 1);
}

// --- review fixes: each test fails if its fix is reverted ---------------------------------------

#[test]
fn overwrite_never_replaces_an_input_workbook() {
    let dir = TempDir::new().unwrap();
    // A workbook named .csv: the format is sniffed, so it opens; its own output name is itself.
    let wb = common::single(dir.path(), "Report.xlsx");
    let disguised = dir.path().join("Report.csv");
    std::fs::rename(&wb, &disguised).unwrap();
    let before = std::fs::read(&disguised).unwrap();
    let out = run(&["csv", s(&disguised), "--name", "{stem}.csv", "--overwrite"]);
    assert_eq!(code(&out), 1);
    assert_eq!(
        std::fs::read(&disguised).unwrap(),
        before,
        "the input was replaced"
    );
    assert!(stdout(&out).contains("input workbooks"), "{}", stdout(&out));
}

#[test]
fn aliased_paths_to_one_directory_collide() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("d")).unwrap();
    let a = common::single(&dir.path().join("d"), "B.xlsx");
    std::fs::copy(&a, dir.path().join("d").join("B.xlsm")).unwrap();
    let first = bin()
        .current_dir(dir.path())
        .args([
            "csv",
            "d/B.xlsx",
            "./d/B.xlsm",
            "--overwrite",
            "--name",
            "{stem}-{index}.csv",
        ])
        .output()
        .unwrap();
    assert_eq!(code(&first), 1, "{}", stdout(&first));
    assert!(
        stdout(&first).contains("same file name"),
        "{}",
        stdout(&first)
    );
}

#[test]
fn unicode_normalization_twins_never_silently_merge() {
    let dir = TempDir::new().unwrap();
    let wb = common::nfc_nfd(dir.path());
    let out = run(&["csv", s(&wb), "--overwrite", "--json"]);
    // On a normalization-insensitive volume the second write is refused; on one that keeps both
    // names, both files exist.  Never: two `written` reports landing in one file.
    let v = json(&out);
    let written = v["files"][0]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["status"] == "written")
        .count();
    let files = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "csv")
        })
        .count();
    assert_eq!(written, files, "{v}");
    if files == 1 {
        assert_eq!(code(&out), 1);
    }
}

#[test]
fn check_on_an_unreadable_sheet_is_exit_1_not_findings() {
    let dir = TempDir::new().unwrap();
    let wb = common::missing_part(dir.path());
    let out = run(&["check", s(&wb), "--json"]);
    assert_eq!(code(&out), 1);
    let v = json(&out);
    assert_eq!(v["files"][0]["status"], "failed");
    assert_eq!(v["files"][0]["findings"][0]["kind"], "unreadable-sheet");

    // csv lists every selected sheet, the unreadable one included, and writes nothing.
    let out = run(&["csv", s(&wb), "--json"]);
    assert_eq!(code(&out), 1);
    let statuses: Vec<_> = json(&out)["files"][0]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["status"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(statuses, ["withheld", "unreadable"]);
    assert!(!dir.path().join("MissingPart-fromXlsx-Here.csv").exists());
}

#[test]
fn shared_formula_before_its_anchor_is_kept() {
    let dir = TempDir::new().unwrap();
    let wb = common::late_shared_anchor(dir.path());
    let out = run(&["cells", s(&wb), "--formulas-only"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let records: Vec<Value> = stdout(&out)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 2, "{records:?}");
    assert_eq!(records[0]["ref"], "A1");
    assert_eq!(records[0]["formula"], "=B1*2");
    assert_eq!(records[0]["cached"], false);
    assert_eq!(code(&run(&["check", s(&wb)])), 3);
}

#[test]
fn visible_only_never_silently_drops_a_named_sheet() {
    let dir = TempDir::new().unwrap();
    let wb = common::multi(dir.path());
    for sub in ["csv", "cells"] {
        let out = run(&[sub, s(&wb), "--sheet", "Secret", "--visible-only"]);
        assert_eq!(code(&out), 1, "{sub}");
        assert!(stderr(&out).contains("--visible-only excludes it"), "{sub}");
    }
}

#[test]
fn dry_run_agrees_with_create_destination() {
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "One.xlsx");
    let target = dir.path().join("new");
    let out = run(&[
        "csv",
        s(&wb),
        "--output-dir",
        s(&target),
        "--create-destination",
        "final",
        "--dry-run",
    ]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("would write"));
    assert!(!target.exists());
}

#[test]
fn relative_single_component_output_dir_with_final() {
    let dir = TempDir::new().unwrap();
    common::single(dir.path(), "One.xlsx");
    let out = bin()
        .current_dir(dir.path())
        .args([
            "csv",
            "One.xlsx",
            "--output-dir",
            "out",
            "--create-destination",
            "final",
        ])
        .output()
        .unwrap();
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(dir.path().join("out/One-fromXlsx.csv").exists());
}

#[cfg(unix)]
#[test]
fn a_symlink_at_an_output_path_is_refused_even_dangling() {
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "One.xlsx");
    let link = dir.path().join("One-fromXlsx.csv");
    std::os::unix::fs::symlink(dir.path().join("nowhere"), &link).unwrap();
    for extra in [None, Some("--overwrite")] {
        let mut args = vec!["csv", s(&wb)];
        args.extend(extra);
        let out = run(&args);
        assert_eq!(code(&out), 1, "{extra:?}");
        assert!(stdout(&out).contains("symbolic link"), "{}", stdout(&out));
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!dir.path().join("nowhere").exists());
    }
}

#[cfg(unix)]
#[test]
fn outputs_get_the_umask_mode_not_0600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let wb = common::single(dir.path(), "One.xlsx");
    assert_eq!(code(&run(&["csv", s(&wb)])), 0);
    let reference = dir.path().join("reference");
    std::fs::write(&reference, b"").unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir.path().join("One-fromXlsx.csv")), mode(&reference));
}

#[test]
fn check_flags_a_grid_too_large_for_csv() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Stray.xlsx");
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_number(0, 0, 1.0).unwrap();
    ws.write_number(1_048_575, 16_383, 2.0).unwrap();
    wb.save(&path).unwrap();
    let out = run(&["check", s(&path), "--json"]);
    assert_eq!(code(&out), 3);
    assert_eq!(
        json(&out)["files"][0]["findings"][0]["kind"],
        "oversized-grid"
    );
}
