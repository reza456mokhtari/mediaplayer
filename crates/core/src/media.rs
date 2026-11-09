//! Local media paths, validated before anything reaches a backend.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Why a local path cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    EmptyPath,
    NotFound(PathBuf),
    /// The path exists but is not a regular file (folder, device, pipe, ...).
    NotAFile(PathBuf),
    /// The path exists but its metadata cannot be read (permissions, ...).
    Inaccessible {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => f.write_str("No file was given."),
            Self::NotFound(path) => write!(f, "File not found: {}", path.display()),
            Self::NotAFile(path) => write!(f, "Not a playable file: {}", path.display()),
            Self::Inaccessible { path, reason } => {
                write!(f, "Can't read {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for OpenError {}

/// Resolves `path` to the absolute path of an existing regular file.
///
/// Engines only receive paths that passed this check. Absolute paths can't be
/// mistaken for backend URLs or options (libmpv treats `scheme://...` as a
/// protocol), and refusing non-regular files keeps devices and FIFOs from
/// blocking a backend. Symlinks are followed. The file may still vanish
/// afterwards, so engines report their own failures too.
pub fn validate_local_file(path: &Path) -> Result<PathBuf, OpenError> {
    if path.as_os_str().is_empty() {
        return Err(OpenError::EmptyPath);
    }
    let absolute = std::path::absolute(path).map_err(|err| OpenError::Inaccessible {
        path: path.to_path_buf(),
        reason: err.to_string(),
    })?;
    match std::fs::metadata(&absolute) {
        Ok(metadata) if metadata.is_file() => Ok(absolute),
        Ok(_) => Err(OpenError::NotAFile(absolute)),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Err(OpenError::NotFound(absolute))
        }
        Err(err) => Err(OpenError::Inaccessible {
            path: absolute,
            reason: err.to_string(),
        }),
    }
}

/// A media file the player knows about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaItem {
    path: PathBuf,
    title: String,
}

impl MediaItem {
    pub fn new(path: PathBuf) -> Self {
        let title = path.file_stem().or_else(|| path.file_name()).map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        Self { path, title }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Display title: the file name without its extension, until tags are read.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The containing folder, for secondary text.
    pub fn folder(&self) -> Option<String> {
        self.path
            .parent()
            .map(|parent| parent.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
    }

    #[test]
    fn accepts_an_existing_file() {
        assert_eq!(validate_local_file(&manifest()), Ok(manifest()));
    }

    #[test]
    fn resolves_relative_paths_against_the_working_directory() {
        // `cargo test` runs unit tests from the package root.
        assert_eq!(validate_local_file(Path::new("Cargo.toml")), Ok(manifest()));
    }

    #[test]
    fn reports_missing_files() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-file.mp4");
        assert_eq!(
            validate_local_file(&missing),
            Err(OpenError::NotFound(missing))
        );
    }

    #[test]
    fn a_path_below_a_file_is_not_found() {
        let below = manifest().join("child.mp3");
        assert_eq!(validate_local_file(&below), Err(OpenError::NotFound(below)));
    }

    #[test]
    fn rejects_directories() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(validate_local_file(&dir), Err(OpenError::NotAFile(dir)));
    }

    #[test]
    fn rejects_an_empty_path() {
        assert_eq!(
            validate_local_file(Path::new("")),
            Err(OpenError::EmptyPath)
        );
    }

    #[test]
    fn title_is_the_file_stem() {
        let item = MediaItem::new(PathBuf::from("/music/Artist - Song.flac"));
        assert_eq!(item.title(), "Artist - Song");
        assert_eq!(item.folder().as_deref(), Some("/music"));
    }
}
