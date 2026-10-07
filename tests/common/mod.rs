#![allow(dead_code)]
//! Synthetic fixtures, generated per test.  The repo is public: no real workbook, nor anything
//! derived from one, is ever committed (`CLAUDE.md` § Gotchas).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Chart, ChartType, ExcelDateTime, Format, Formula, Workbook as XlsxWriter};

/// The rich multi-sheet workbook most tests use:
///
/// 1. `Budget` (visible): data from C3, a formula with a cached value, a merged region C7:E7,
///    a boolean, an error, and an ‘okina.
/// 2. `Dates` (visible): a date, a date-time, a time, a plain number, a duration.
/// 3. `Secret` (hidden), 4. `Vault` (very hidden), 5. `Empty`.
pub fn multi(dir: &Path) -> PathBuf {
    let path = dir.join("Multi.xlsx");
    let mut wb = XlsxWriter::new();

    let ws = wb.add_worksheet().set_name("Budget").unwrap();
    ws.write_string(2, 2, "Item").unwrap();
    ws.write_string(2, 3, "Amount").unwrap();
    ws.write_string(3, 2, "Robes, Hawai‘i").unwrap();
    ws.write_number(3, 3, 1234.56).unwrap();
    ws.write_boolean(3, 4, true).unwrap();
    ws.write_string(4, 2, "Total").unwrap();
    ws.write_formula(4, 3, Formula::new("=SUM(D4:D4)").set_result("1234.56"))
        .unwrap();
    ws.write_formula(4, 5, Formula::new("=1/0").set_result("#DIV/0!"))
        .unwrap();
    ws.merge_range(6, 2, 6, 4, "Merged note", &Format::new())
        .unwrap();

    let ws = wb.add_worksheet().set_name("Dates").unwrap();
    let date = Format::new().set_num_format("yyyy-mm-dd");
    let datetime = Format::new().set_num_format("yyyy-mm-dd hh:mm");
    let time = Format::new().set_num_format("hh:mm");
    let duration = Format::new().set_num_format("[h]:mm");
    ws.write_datetime_with_format(0, 0, ExcelDateTime::from_ymd(2026, 6, 13).unwrap(), &date)
        .unwrap();
    ws.write_datetime_with_format(
        1,
        0,
        ExcelDateTime::from_ymd(2026, 6, 13)
            .unwrap()
            .and_hms(9, 30, 0)
            .unwrap(),
        &datetime,
    )
    .unwrap();
    ws.write_datetime_with_format(2, 0, ExcelDateTime::from_hms(18, 0, 0).unwrap(), &time)
        .unwrap();
    ws.write_number(3, 0, 46186.0).unwrap();
    ws.write_number_with_format(4, 0, 1.5 + 0.5 / 24.0, &duration)
        .unwrap();

    let ws = wb.add_worksheet().set_name("Secret").unwrap();
    ws.write_string(0, 0, "salary").unwrap();
    ws.set_hidden(true);

    let ws = wb.add_worksheet().set_name("Vault").unwrap();
    ws.write_string(0, 0, "very hidden").unwrap();
    ws.set_very_hidden(true);

    wb.add_worksheet().set_name("Empty").unwrap();

    wb.save(&path).unwrap();
    path
}

/// A one-sheet workbook with a single value, under the given file name.
pub fn single(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let mut wb = XlsxWriter::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "only").unwrap();
    ws.write_number(0, 1, 1.0).unwrap();
    wb.save(&path).unwrap();
    path
}

/// A one-sheet workbook whose formula in A2 has no cached value — the file a generator that never
/// computes would write.  rust_xlsxwriter always caches a value, so the `<v>` is cut out.
pub fn uncached(dir: &Path) -> PathBuf {
    let src = dir.join("uncached-src.xlsx");
    let mut wb = XlsxWriter::new();
    let ws = wb.add_worksheet();
    ws.write_number(0, 0, 21.0).unwrap();
    ws.write_formula(1, 0, "=A1*2").unwrap();
    wb.save(&src).unwrap();
    let path = dir.join("Uncached.xlsx");
    rewrite(&src, &path, |name, bytes| {
        if name != "xl/worksheets/sheet1.xml" {
            return bytes;
        }
        let xml = String::from_utf8(bytes).unwrap();
        assert!(
            xml.contains("</f><v>0</v>"),
            "fixture assumption broke: {xml}"
        );
        xml.replace("</f><v>0</v>", "</f>").into_bytes()
    });
    path
}

/// A one-sheet workbook in the 1904 date system holding 2026-06-13 (serial 44724 there).
pub fn date1904(dir: &Path) -> PathBuf {
    let src = dir.join("d1904-src.xlsx");
    let mut wb = XlsxWriter::new();
    let ws = wb.add_worksheet();
    ws.write_number_with_format(0, 0, 44724.0, &Format::new().set_num_format("yyyy-mm-dd"))
        .unwrap();
    wb.save(&src).unwrap();
    let path = dir.join("Date1904.xlsx");
    rewrite(&src, &path, |name, bytes| {
        if name != "xl/workbook.xml" {
            return bytes;
        }
        let xml = String::from_utf8(bytes).unwrap();
        assert!(
            xml.contains("<workbookPr "),
            "fixture assumption broke: {xml}"
        );
        xml.replace("<workbookPr ", "<workbookPr date1904=\"1\" ")
            .into_bytes()
    });
    path
}

/// A workbook with a worksheet and a chart sheet.
pub fn with_chartsheet(dir: &Path) -> PathBuf {
    let path = dir.join("Chart.xlsx");
    let mut wb = XlsxWriter::new();
    let ws = wb.add_worksheet().set_name("Data").unwrap();
    for row in 0..3 {
        ws.write_number(row, 0, f64::from(row) + 1.0).unwrap();
    }
    let mut chart = Chart::new(ChartType::Column);
    chart.add_series().set_values("Data!$A$1:$A$3");
    let cs = wb.add_chartsheet().set_name("Plot").unwrap();
    cs.insert_chart(0, 0, &chart).unwrap();
    wb.save(&path).unwrap();
    path
}

/// Two sheets whose file names collide once sanitized and case-folded (`Data` and `DATA `; a `/`
/// is illegal in an Excel sheet name, so case and a trailing space carry the collision).
pub fn colliding(dir: &Path) -> PathBuf {
    let path = dir.join("Collide.xlsx");
    let mut wb = XlsxWriter::new();
    wb.add_worksheet()
        .set_name("Data")
        .unwrap()
        .write_number(0, 0, 1.0)
        .unwrap();
    wb.add_worksheet()
        .set_name("DATA ")
        .unwrap()
        .write_number(0, 0, 2.0)
        .unwrap();
    wb.save(&path).unwrap();
    path
}

/// An encrypted OOXML package: an OLE container with the two streams that mark one.  No real
/// encryption is needed for the reader to classify it.
pub fn encrypted(dir: &Path) -> PathBuf {
    let path = dir.join("Locked.xlsx");
    let mut c = cfb::CompoundFile::create(std::fs::File::create(&path).unwrap()).unwrap();
    c.create_stream("/EncryptionInfo")
        .unwrap()
        .write_all(&[4, 0, 4, 0, 0x40, 0, 0, 0])
        .unwrap();
    c.create_stream("/EncryptedPackage")
        .unwrap()
        .write_all(&[0u8; 4096])
        .unwrap();
    c.flush().unwrap();
    path
}

/// The first half of a real package: a zip with no central directory.
pub fn truncated(dir: &Path) -> PathBuf {
    let good = single(dir, "whole.xlsx");
    let bytes = std::fs::read(&good).unwrap();
    let path = dir.join("Truncated.xlsx");
    std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    path
}

/// A hand-built ODS: sheet `Vis` with a value, a formula with a cached value, a formula without
/// one, a date, a time, and a merged cell; sheet `Hid`, hidden.
pub fn ods(dir: &Path) -> PathBuf {
    let path = dir.join("Sheet.ods");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    w.start_file("mimetype", stored).unwrap();
    w.write_all(b"application/vnd.oasis.opendocument.spreadsheet")
        .unwrap();
    w.start_file("META-INF/manifest.xml", stored).unwrap();
    w.write_all(br#"<?xml version="1.0"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/></manifest:manifest>"#).unwrap();
    w.start_file("content.xml", stored).unwrap();
    w.write_all(r#"<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:automatic-styles><style:style style:name="ta1" style:family="table"><style:table-properties table:display="true"/></style:style><style:style style:name="ta2" style:family="table"><style:table-properties table:display="false"/></style:style></office:automatic-styles><office:body><office:spreadsheet><table:table table:name="Vis" table:style-name="ta1"><table:table-row><table:table-cell/><table:table-cell office:value-type="float" office:value="2"><text:p>2</text:p></table:table-cell><table:table-cell table:formula="of:=[.B1]*2" office:value-type="float" office:value="4"><text:p>4</text:p></table:table-cell><table:table-cell table:formula="of:=[.B1]*3"/></table:table-row><table:table-row><table:table-cell office:value-type="date" office:date-value="2026-10-06"/><table:table-cell office:value-type="time" office:time-value="PT13H45M00S"/><table:table-cell office:value-type="string"><text:p>Mālama ‘āina</text:p></table:table-cell></table:table-row></table:table><table:table table:name="Hid" table:style-name="ta2"><table:table-row><table:table-cell office:value-type="string"><text:p>secret</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#.as_bytes()).unwrap();
    w.finish().unwrap();
    path
}

/// Copy a zip package entry by entry, passing each entry's bytes through `edit`.
fn rewrite(src: &Path, dst: &Path, edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(src).unwrap()).unwrap();
    let mut w = zip::ZipWriter::new(std::fs::File::create(dst).unwrap());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        w.start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(&edit(&name, bytes)).unwrap();
    }
    w.finish().unwrap();
}

/// A CSV file as text, with its BOM checked and stripped.
pub fn csv_text(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    assert!(
        bytes.starts_with(b"\xEF\xBB\xBF"),
        "{} lacks a BOM",
        path.display()
    );
    String::from_utf8(bytes[3..].to_vec()).unwrap()
}

/// A worksheet whose shared-formula group has a derived cell (A1) placed *before* its anchor (A2)
/// — legal, and what a non-Excel generator may write — and no cached values at all.
pub fn late_shared_anchor(dir: &Path) -> PathBuf {
    let src = dir.join("late-src.xlsx");
    let mut wb = XlsxWriter::new();
    wb.add_worksheet().write_number(0, 1, 5.0).unwrap();
    wb.save(&src).unwrap();
    let path = dir.join("LateAnchor.xlsx");
    rewrite(&src, &path, |name, bytes| {
        if name != "xl/worksheets/sheet1.xml" {
            return bytes;
        }
        let xml = String::from_utf8(bytes).unwrap();
        let start = xml.find("<sheetData").unwrap();
        let end = xml.find("</sheetData>").unwrap() + "</sheetData>".len();
        let data = r#"<sheetData><row r="1"><c r="A1"><f t="shared" si="0"/></c><c r="B1"><v>5</v></c></row><row r="2"><c r="A2"><f t="shared" ref="A1:A2" si="0">B2*2</f></c><c r="B2"><v>6</v></c></row></sheetData>"#;
        format!("{}{data}{}", &xml[..start], &xml[end..]).into_bytes()
    });
    path
}

/// A workbook listing a second sheet whose part is missing from the package: a readable workbook
/// with one unreadable sheet.
pub fn missing_part(dir: &Path) -> PathBuf {
    let src = dir.join("missing-src.xlsx");
    let mut wb = XlsxWriter::new();
    wb.add_worksheet()
        .set_name("Here")
        .unwrap()
        .write_number(0, 0, 1.0)
        .unwrap();
    wb.add_worksheet()
        .set_name("Gone")
        .unwrap()
        .write_number(0, 0, 2.0)
        .unwrap();
    wb.save(&src).unwrap();
    let path = dir.join("MissingPart.xlsx");
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&src).unwrap()).unwrap();
    let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        if name == "xl/worksheets/sheet2.xml" {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        w.start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(&bytes).unwrap();
    }
    w.finish().unwrap();
    path
}

/// Two sheets whose names differ only in Unicode normalization (and perhaps case): `Café`
/// precomposed (NFC) and decomposed (NFD), say.  The default macOS volume treats the two file
/// names as one.
pub fn twins(dir: &Path, first: &str, second: &str) -> PathBuf {
    let path = dir.join("Accents.xlsx");
    let mut wb = XlsxWriter::new();
    wb.add_worksheet()
        .set_name(first)
        .unwrap()
        .write_string(0, 0, "first")
        .unwrap();
    wb.add_worksheet()
        .set_name(second)
        .unwrap()
        .write_string(0, 0, "second")
        .unwrap();
    wb.save(&path).unwrap();
    path
}
