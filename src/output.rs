//! Deciding whether an output may be written, and writing it atomically.
//!
//! Symlink policy (documented in `csv --help`): an input workbook reached through a symlink is
//! read through it; a symlink sitting at an output path is never followed and never replaced —
//! it is refused, or skipped under `--skip-existing`.  A derived copy must land exactly where its
//! name says, not wherever a link points.

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use cli_contract::{Clobber, CreateMode, PathError, prepare_output_file};

/// A file's identity on disk, independent of how its path is spelled (case, Unicode
/// normalization, `./`, a symlinked directory): what the filesystem itself says is one file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId(
    #[cfg(unix)] (u64, u64),
    #[cfg(not(unix))] std::path::PathBuf,
);

impl FileId {
    /// The identity of the file at `path`, following symlinks: `Ok(None)` only for a proven
    /// `NotFound`, an error for anything that could not be inspected.
    pub fn of(path: &Path) -> io::Result<Option<FileId>> {
        match std::fs::metadata(path) {
            Ok(meta) => Ok(Some(Self::from_meta(path, &meta)?)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    #[cfg(unix)]
    fn from_meta(_path: &Path, meta: &std::fs::Metadata) -> io::Result<FileId> {
        use std::os::unix::fs::MetadataExt;
        Ok(FileId((meta.dev(), meta.ino())))
    }

    #[cfg(not(unix))]
    fn from_meta(path: &Path, _meta: &std::fs::Metadata) -> io::Result<FileId> {
        Ok(FileId(path.canonicalize()?))
    }
}

/// What may happen at an output path, decided before anything is written.
#[derive(Debug)]
pub enum Plan {
    /// Absent (proven `NotFound`), or present and `--overwrite` was given.
    Write,
    /// Present and `--skip-existing` was given.
    Skip,
    /// Must not be written; the reason names the path.
    Refuse(String),
}

/// The caller's choices that bear on one output.
pub struct Policy<'a> {
    pub overwrite: bool,
    pub skip_existing: bool,
    /// False in a dry run whose `--output-dir` does not exist yet but would be created: nothing
    /// can be in it, so every output there is absent.
    pub dir_exists: bool,
    /// The identities of every input workbook in the run.
    pub inputs: &'a HashSet<FileId>,
    /// An input that could not be inspected, with why.  While there is one, `inputs` is
    /// incomplete: an existing file might be that input, so `--overwrite` may replace nothing.
    pub uninspected: Option<&'a str>,
}

/// Probe an output path three ways.  Only a proven absence, or the caller's explicit
/// `--overwrite` over a regular file that is provably not an input, licenses a write; a path that
/// could not be inspected is refused, never treated as absent (`positive-evidence-of-absence.md`).
pub fn plan(path: &Path, policy: &Policy<'_>) -> Plan {
    if !policy.dir_exists {
        return Plan::Write;
    }
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Plan::Refuse(format!("cannot inspect {}: {e}", path.display())),
        Ok(meta) if meta.file_type().is_symlink() => {
            return if policy.skip_existing {
                Plan::Skip
            } else {
                Plan::Refuse(format!(
                    "{} is a symbolic link; an output is never written through or over one",
                    path.display()
                ))
            };
        }
        Ok(_) => match FileId::of(path) {
            Ok(Some(id)) if policy.inputs.contains(&id) => {
                return Plan::Refuse(format!(
                    "{} is one of this run's input workbooks; refusing to replace it with a CSV",
                    path.display()
                ));
            }
            Ok(_) => {
                if let (true, Some(input)) = (policy.overwrite, policy.uninspected) {
                    // The protected set is incomplete, so this file may be the input that could
                    // not be inspected: Unknown never licenses the replacement.
                    return Plan::Refuse(format!(
                        "{} exists, and an input could not be inspected ({input}), so it cannot \
                         be ruled out as that input; refusing to replace it",
                        path.display()
                    ));
                }
            }
            Err(e) => return Plan::Refuse(format!("cannot inspect {}: {e}", path.display())),
        },
    }
    let clobber = if policy.overwrite {
        Clobber::Allow
    } else {
        Clobber::Deny
    };
    // The directory was already checked against --create-destination; here it must exist.
    match prepare_output_file(path, CreateMode::None, clobber) {
        Ok(()) => Plan::Write,
        Err(PathError::Clobber(_)) if policy.skip_existing => Plan::Skip,
        Err(PathError::Clobber(p)) => Plan::Refuse(format!(
            "{} already exists (use --overwrite to replace it, or --skip-existing to leave it)",
            p.display()
        )),
        Err(e) => Plan::Refuse(e.to_string()),
    }
}

/// Write `bytes` to `path` through a temporary file in the same directory, so an interrupted run
/// never leaves a truncated CSV that reads as a short sheet.  Without `overwrite`, a file that
/// appeared since [`plan`] probed is still not replaced.  With it, a file this run already wrote
/// is never replaced either: two outputs whose names the filesystem treats as one (Unicode
/// normalization, case) must not silently become one file.  Each written file's identity is added
/// to `written`.
pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
    overwrite: bool,
    written: &mut HashSet<FileId>,
) -> Result<()> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let mut builder = tempfile::Builder::new();
    builder.prefix(".xlsx-dump-").suffix(".tmp");
    #[cfg(unix)]
    {
        // Like `cp` and `>`: 0666 less the umask, not the temporary file's private 0600.
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut tmp = builder
        .tempfile_in(dir)
        .with_context(|| format!("cannot create a temporary file in {}", dir.display()))?;
    tmp.write_all(bytes)
        .and_then(|()| tmp.as_file().sync_all())
        .with_context(|| format!("cannot write a temporary file in {}", dir.display()))?;
    if overwrite {
        match FileId::of(path) {
            Ok(Some(id)) if written.contains(&id) => bail!(
                "{} was already written by this run under another name the filesystem treats as \
                 the same; refusing to replace it",
                path.display()
            ),
            Ok(_) => {}
            Err(e) => bail!("cannot inspect {}: {e}", path.display()),
        }
        tmp.persist(path)
            .map_err(|e| e.error)
            .with_context(|| format!("cannot write {}", path.display()))?;
    } else {
        tmp.persist_noclobber(path)
            .map_err(|e| e.error)
            .with_context(|| format!("cannot write {} (refusing to replace it)", path.display()))?;
    }
    match FileId::of(path) {
        Ok(Some(id)) => {
            written.insert(id);
            Ok(())
        }
        // Written, but its identity is unknown, so the backstop above could not protect it.
        _ => bail!(
            "{} was written but cannot be inspected afterwards; check it by hand",
            path.display()
        ),
    }
}
