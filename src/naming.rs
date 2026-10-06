//! Output file naming: the `--name` template, the `-from<Ext>` marker, and sheet-name sanitizing.
//!
//! The default follows Obsidian-Brain's derived-copy convention (`DESIGN.md` § 5): a workbook's
//! CSV is `<stem>-fromXlsx.csv`, and a workbook with more than one sheet gets one CSV per sheet,
//! `<stem>-fromXlsx-<Sheet>.csv`.  Which form applies depends on how many sheets the *workbook*
//! has, never on how many were selected, so a sheet's derived copy has the same name however it
//! was produced.

use std::path::Path;

/// The default template for a workbook with exactly one sheet.
pub const DEFAULT_SINGLE: &str = "{stem}-{FromExt}.csv";
/// The default template for a workbook with more than one sheet.
pub const DEFAULT_MULTI: &str = "{stem}-{FromExt}-{sheet}.csv";

const TOKENS: [&str; 4] = ["stem", "FromExt", "sheet", "index"];

/// Characters replaced by `_` in a sheet name used in a filename.
const UNSAFE: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The values a template is rendered with.
pub struct NameParts<'a> {
    /// The workbook's file name without its extension.
    pub stem: &'a str,
    /// The marker, e.g. `fromXlsx`.
    pub from_ext: &'a str,
    /// The sheet's name, raw; it is sanitized when rendered.
    pub sheet: &'a str,
    /// The sheet's 1-based position in the workbook.
    pub index: usize,
}

/// The `-from<Ext>` marker for a workbook: `from` plus its extension, first letter capitalized and
/// the rest lowercase (`Budget.XLSX` → `fromXlsx`).  `None` when the path has no UTF-8 extension.
pub fn from_ext(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    let mut chars = ext.chars();
    let first = chars.next()?;
    Some(format!(
        "from{}{}",
        first.to_uppercase(),
        chars.as_str().to_lowercase()
    ))
}

/// Make a sheet name safe as part of a file name.  The rule, documented in `csv --help`:
/// `/ \ : * ? " < > |` and control characters become `_`; leading dots and spaces and trailing
/// dots and spaces are trimmed; a name left empty becomes `Sheet<index>`.
pub fn sanitize_sheet(name: &str, index: usize) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if UNSAFE.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = replaced
        .trim_start_matches(['.', ' '])
        .trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        format!("Sheet{index}")
    } else {
        trimmed.to_string()
    }
}

/// Check a `--name` template on its face: every `{…}` is a known token, braces balance, and the
/// template names a file, not a path.  Used as a clap value parser, so a bad template is a usage
/// error (exit 2).
pub fn validate_template(template: &str) -> Result<String, String> {
    if template.is_empty() {
        return Err("the name template is empty".into());
    }
    // Also what keeps --overwrite from ever replacing a workbook with its own CSV.
    if !template.to_ascii_lowercase().ends_with(".csv") {
        return Err("the name template must end in .csv".into());
    }
    if template.chars().any(std::path::is_separator) {
        return Err(
            "the name template must be a file name, without a path separator \
                    (use --output-dir for the directory)"
                .into(),
        );
    }
    let mut rest = template;
    while let Some(open) = rest.find(['{', '}']) {
        if rest[open..].starts_with('}') {
            return Err(format!("unmatched '}}' in name template {template:?}"));
        }
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            return Err(format!("unmatched '{{' in name template {template:?}"));
        };
        let token = &after[..close];
        if !TOKENS.contains(&token) {
            return Err(format!(
                "unknown token {{{token}}} in name template; known tokens: {{stem}}, {{FromExt}}, \
                 {{sheet}}, {{index}}"
            ));
        }
        rest = &after[close + 1..];
    }
    Ok(template.to_string())
}

/// Render a template already accepted by [`validate_template`].
pub fn render(template: &str, parts: &NameParts<'_>) -> String {
    template
        .replace("{stem}", parts.stem)
        .replace("{FromExt}", parts.from_ext)
        .replace("{sheet}", &sanitize_sheet(parts.sheet, parts.index))
        .replace("{index}", &parts.index.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts<'a>(sheet: &'a str, index: usize) -> NameParts<'a> {
        NameParts {
            stem: "KCS-Budget",
            from_ext: "fromXlsx",
            sheet,
            index,
        }
    }

    #[test]
    fn from_ext_capitalizes_the_extension() {
        assert_eq!(from_ext(Path::new("a/B.xlsx")).unwrap(), "fromXlsx");
        assert_eq!(from_ext(Path::new("B.XLS")).unwrap(), "fromXls");
        assert_eq!(from_ext(Path::new("B.ods")).unwrap(), "fromOds");
        assert_eq!(from_ext(Path::new("B")), None);
    }

    #[test]
    fn sanitize_replaces_unsafe_and_trims() {
        assert_eq!(sanitize_sheet("Q1/Q2: Budget?", 1), "Q1_Q2_ Budget_");
        assert_eq!(sanitize_sheet("..hidden ", 2), "hidden");
        assert_eq!(sanitize_sheet("tab\tname", 1), "tab_name");
        assert_eq!(sanitize_sheet(" . ", 3), "Sheet3");
        assert_eq!(sanitize_sheet("Mālama ‘Āina", 1), "Mālama ‘Āina");
    }

    #[test]
    fn default_templates_render_the_vault_convention() {
        assert_eq!(
            render(DEFAULT_SINGLE, &parts("Sheet1", 1)),
            "KCS-Budget-fromXlsx.csv"
        );
        assert_eq!(
            render(DEFAULT_MULTI, &parts("By Class", 2)),
            "KCS-Budget-fromXlsx-By Class.csv"
        );
        assert_eq!(
            render("{stem}.{index}.csv", &parts("x", 3)),
            "KCS-Budget.3.csv"
        );
    }

    #[test]
    fn validate_rejects_bad_templates() {
        assert!(validate_template(DEFAULT_MULTI).is_ok());
        assert!(validate_template("").is_err());
        assert!(validate_template("out/{stem}.csv").is_err());
        assert!(validate_template("{stem}-{nope}.csv").is_err());
        assert!(validate_template("{stem.csv").is_err());
        assert!(validate_template("stem}.csv").is_err());
        assert!(validate_template("{stem}.xlsx").is_err());
    }
}
