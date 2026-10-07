use serde::Serialize;
use std::env;
use std::fs::{self, File};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;

const SESSION_EXTENSION: &str = "jsonl";
const QUARANTINE_DIR: &str = ".quarantine";

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("environment variable HOME or CODEX_HOME is not configured")]
    MissingHome,
    #[error("discovery root does not exist: {0}")]
    RootNotFound(PathBuf),
    #[error("discovery root is not a directory: {0}")]
    RootNotDirectory(PathBuf),
    #[error("path is outside discovery root: {0}")]
    OutsideRoot(PathBuf),
    #[error("path is not an allowed Codex workspace file: {0}")]
    PathNotAllowed(PathBuf),
    #[error("symbolic links are not allowed: {0}")]
    SymlinkRejected(PathBuf),
    #[error("invalid path: {0}")]
    InvalidPath(PathBuf),
    #[error("I/O error for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

impl DiscoveryError {
    fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

/// A JSONL Codex session file discovered below the configured Codex home.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CodexSessionRecord {
    pub id: String,
    pub path: String,
    pub bytes: u64,
    #[serde(rename = "modifiedAtMs")]
    pub modified_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// A regular file discovered below a user-selected workspace root.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct WorkspaceFileRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub size: u64,
    pub kind: String,
    pub session: String,
    pub selected: bool,
    #[serde(rename = "modifiedAtMs")]
    pub modified_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct QuarantineResult {
    pub status: String,
    pub path: String,
    #[serde(rename = "quarantinedPath")]
    pub quarantined_path: String,
    pub bytes: u64,
}

/// Resolve the Codex home without taking a path from the renderer. `CODEX_HOME`
/// follows the Codex convention and points to the directory containing
/// `sessions`; a nested `.codex/sessions` is accepted for compatibility with
/// callers that set the variable to their home directory.
pub fn codex_sessions_root() -> Result<PathBuf, DiscoveryError> {
    let configured = env::var_os("CODEX_HOME").map(PathBuf::from);
    let base = match configured {
        Some(path) => path,
        None => env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(DiscoveryError::MissingHome)?
            .join(".codex"),
    };
    if base.join("sessions").is_dir() || !base.join(".codex").is_dir() {
        Ok(base.join("sessions"))
    } else {
        Ok(base.join(".codex").join("sessions"))
    }
}

pub fn list_codex_sessions() -> Result<Vec<CodexSessionRecord>, DiscoveryError> {
    let root = codex_sessions_root()?;
    let canonical_root = canonical_directory(&root)?;
    let mut records = Vec::new();
    collect_sessions(&canonical_root, &canonical_root, &mut records)?;
    records.sort_by(|left, right| {
        right
            .modified_at_ms
            .cmp(&left.modified_at_ms)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(records)
}

pub fn list_workspace_files(
    root: impl AsRef<Path>,
) -> Result<Vec<WorkspaceFileRecord>, DiscoveryError> {
    let canonical_root = canonical_directory(root.as_ref())?;
    let mut records = Vec::new();
    collect_workspace_files(&canonical_root, &canonical_root, &mut records)?;
    records.sort_by(|left, right| {
        right
            .modified_at_ms
            .cmp(&left.modified_at_ms)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    Ok(records)
}

/// Return the Codex-owned directories that are safe to inspect by default.
/// The whole `~/.codex` directory is intentionally excluded because it also
/// contains credentials, SQLite databases, and runtime caches.
pub fn list_default_workspace_files() -> Result<Vec<WorkspaceFileRecord>, DiscoveryError> {
    let codex_root = codex_home_root()?;
    let candidates = [
        codex_root.join("generated_images"),
        codex_root.join("sessions"),
        codex_root.join("archived_sessions"),
    ];
    let mut records = Vec::new();
    for candidate in candidates {
        if candidate.is_dir() {
            let canonical_root = canonical_directory(&candidate)?;
            let start = records.len();
            collect_workspace_files(&canonical_root, &canonical_root, &mut records)?;
            if canonical_root.file_name().and_then(|name| name.to_str()) == Some("generated_images")
            {
                for record in &mut records[start..] {
                    record.session = record
                        .relative_path
                        .split(std::path::MAIN_SEPARATOR)
                        .next()
                        .unwrap_or_default()
                        .to_owned();
                }
            }
        }
    }
    records.sort_by(|left, right| {
        right
            .modified_at_ms
            .cmp(&left.modified_at_ms)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(records)
}

/// Move a user-selected file into a recoverable quarantine directory. Only
/// Codex-owned roots are accepted; the command never follows a symlink.
pub fn quarantine_path(path: impl AsRef<Path>) -> Result<QuarantineResult, DiscoveryError> {
    let codex_root = codex_home_root()?;
    quarantine_path_in_root(path.as_ref(), &codex_root)
}

fn codex_home_root() -> Result<PathBuf, DiscoveryError> {
    let sessions_root = codex_sessions_root()?;
    sessions_root
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| DiscoveryError::RootNotFound(sessions_root))
}

fn quarantine_path_in_root(
    path: &Path,
    codex_root: &Path,
) -> Result<QuarantineResult, DiscoveryError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| component == Component::ParentDir)
    {
        return Err(DiscoveryError::InvalidPath(path.to_path_buf()));
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DiscoveryError::RootNotFound(path.to_path_buf())
        } else {
            DiscoveryError::io(path, error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(DiscoveryError::SymlinkRejected(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(DiscoveryError::PathNotAllowed(path.to_path_buf()));
    }

    let canonical_codex_root =
        fs::canonicalize(codex_root).map_err(|error| DiscoveryError::io(codex_root, error))?;
    let allowed_roots = ["generated_images", "sessions", "archived_sessions"]
        .into_iter()
        .map(|name| canonical_codex_root.join(name))
        .filter(|candidate| candidate.is_dir())
        .map(|candidate| canonical_directory(&candidate))
        .collect::<Result<Vec<_>, _>>()?;
    let canonical_path = fs::canonicalize(path).map_err(|error| DiscoveryError::io(path, error))?;
    let source_root = allowed_roots
        .iter()
        .find(|root| canonical_path.starts_with(root))
        .ok_or_else(|| DiscoveryError::PathNotAllowed(path.to_path_buf()))?;
    let source_relative = canonical_path
        .strip_prefix(source_root)
        .map_err(|_| DiscoveryError::OutsideRoot(canonical_path.clone()))?;
    if source_relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        == Some(QUARANTINE_DIR)
    {
        return Err(DiscoveryError::PathNotAllowed(path.to_path_buf()));
    }
    let canonical_source = ensure_inside(source_root, &canonical_path)?;
    if fs::symlink_metadata(&canonical_source)
        .map_err(|error| DiscoveryError::io(&canonical_source, error))?
        .file_type()
        .is_symlink()
    {
        return Err(DiscoveryError::SymlinkRejected(path.to_path_buf()));
    }

    let quarantine_root = source_root.join(QUARANTINE_DIR);
    fs::create_dir_all(&quarantine_root)
        .map_err(|error| DiscoveryError::io(&quarantine_root, error))?;
    let quarantine_root = canonical_directory(&quarantine_root)?;
    if !quarantine_root.starts_with(source_root) {
        return Err(DiscoveryError::OutsideRoot(quarantine_root));
    }
    let metadata = fs::metadata(&canonical_source)
        .map_err(|error| DiscoveryError::io(&canonical_source, error))?;
    let relative = canonical_source
        .strip_prefix(source_root)
        .map_err(|_| DiscoveryError::OutsideRoot(canonical_source.clone()))?;
    let destination = quarantine_root.join(relative);
    if destination.exists() {
        return Err(DiscoveryError::Io {
            path: destination,
            source: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "quarantine destination exists",
            ),
        });
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| DiscoveryError::io(parent, error))?;
        let canonical_parent = canonical_directory(parent)?;
        if !canonical_parent.starts_with(&quarantine_root) {
            return Err(DiscoveryError::OutsideRoot(canonical_parent));
        }
    }
    fs::rename(&canonical_source, &destination)
        .map_err(|error| DiscoveryError::io(&canonical_source, error))?;
    Ok(QuarantineResult {
        status: "quarantined".into(),
        path: canonical_source.to_string_lossy().into_owned(),
        quarantined_path: destination.to_string_lossy().into_owned(),
        bytes: metadata.len(),
    })
}

fn canonical_directory(root: &Path) -> Result<PathBuf, DiscoveryError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DiscoveryError::RootNotFound(root.to_path_buf())
        } else {
            DiscoveryError::io(root, error)
        }
    })?;
    if !metadata.is_dir() {
        return Err(DiscoveryError::RootNotDirectory(root.to_path_buf()));
    }
    fs::canonicalize(root).map_err(|error| DiscoveryError::io(root, error))
}

fn collect_sessions(
    root: &Path,
    directory: &Path,
    records: &mut Vec<CodexSessionRecord>,
) -> Result<(), DiscoveryError> {
    for entry in read_entries(directory)? {
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| DiscoveryError::io(&path, error))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            let canonical = ensure_inside(root, &path)?;
            collect_sessions(root, &canonical, records)?;
            continue;
        }
        if !metadata.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some(SESSION_EXTENSION)
        {
            continue;
        }
        let canonical = ensure_inside(root, &path)?;
        let metadata =
            fs::metadata(&canonical).map_err(|error| DiscoveryError::io(&canonical, error))?;
        let (title, workspace) = read_session_metadata(&canonical);
        records.push(CodexSessionRecord {
            id: session_id_from_path(&canonical),
            path: canonical.to_string_lossy().into_owned(),
            bytes: metadata.len(),
            modified_at_ms: modified_at_ms(&metadata),
            title,
            workspace,
        });
    }
    Ok(())
}

fn collect_workspace_files(
    root: &Path,
    directory: &Path,
    records: &mut Vec<WorkspaceFileRecord>,
) -> Result<(), DiscoveryError> {
    for entry in read_entries(directory)? {
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| DiscoveryError::io(&path, error))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some(QUARANTINE_DIR) {
                continue;
            }
            let canonical = ensure_inside(root, &path)?;
            collect_workspace_files(root, &canonical, records)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let canonical = ensure_inside(root, &path)?;
        let metadata =
            fs::metadata(&canonical).map_err(|error| DiscoveryError::io(&canonical, error))?;
        let relative = canonical
            .strip_prefix(root)
            .map_err(|_| DiscoveryError::OutsideRoot(canonical.clone()))?
            .to_string_lossy()
            .into_owned();
        let name = canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned();
        let kind = canonical
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("file")
            .to_owned();
        let session = (kind == SESSION_EXTENSION)
            .then(|| session_id_from_path(&canonical))
            .unwrap_or_default();
        records.push(WorkspaceFileRecord {
            id: format!("workspace:{}", canonical.to_string_lossy()),
            name,
            path: canonical.to_string_lossy().into_owned(),
            relative_path: relative,
            size: metadata.len(),
            kind,
            session,
            selected: false,
            modified_at_ms: modified_at_ms(&metadata),
        });
    }
    Ok(())
}

fn read_entries(directory: &Path) -> Result<Vec<fs::DirEntry>, DiscoveryError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| DiscoveryError::io(directory, error))?
        .map(|entry| entry.map_err(|error| DiscoveryError::io(directory, error)))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    Ok(entries)
}

fn ensure_inside(root: &Path, path: &Path) -> Result<PathBuf, DiscoveryError> {
    let canonical = fs::canonicalize(path).map_err(|error| DiscoveryError::io(path, error))?;
    if !canonical.starts_with(root) {
        return Err(DiscoveryError::OutsideRoot(path.to_path_buf()));
    }
    Ok(canonical)
}

fn modified_at_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn session_id_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    stem.rsplit('_')
        .next()
        .unwrap_or(stem)
        .rsplit('-')
        .next()
        .unwrap_or(stem)
        .to_owned()
}

fn read_session_metadata(path: &Path) -> (Option<String>, Option<String>) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return (None, None),
    };
    let mut line = String::new();
    if BufReader::new(file).read_line(&mut line).is_err() {
        return (None, None);
    }
    let value: serde_json::Value = match serde_json::from_str(&line) {
        Ok(value) => value,
        Err(_) => return (None, None),
    };
    let payload = value.get("payload").unwrap_or(&value);
    let title = payload
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let workspace = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    (title, workspace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn lists_jsonl_sessions_recursively_and_extracts_id() {
        let root = tempdir().unwrap();
        let day = root.path().join("2026").join("10").join("08");
        fs::create_dir_all(&day).unwrap();
        let session = day.join("rollout-2026-10-08T01-02-03-01abc123.jsonl");
        fs::write(
            &session,
            r#"{"payload":{"cwd":"/tmp/workspace","title":"测试会话"}}"#.as_bytes(),
        )
        .unwrap();
        fs::write(day.join("not-a-session.txt"), b"ignore").unwrap();

        let records = {
            let canonical = fs::canonicalize(root.path()).unwrap();
            let mut records = Vec::new();
            collect_sessions(&canonical, &canonical, &mut records).unwrap();
            records
        };
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "01abc123");
        assert_eq!(records[0].workspace.as_deref(), Some("/tmp/workspace"));
        assert_eq!(records[0].title.as_deref(), Some("测试会话"));
    }

    #[test]
    fn workspace_listing_includes_regular_files_and_skips_quarantine_and_symlinks() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join(QUARANTINE_DIR)).unwrap();
        fs::write(root.path().join("src").join("main.rs"), b"fn main() {}").unwrap();
        fs::write(
            root.path().join(QUARANTINE_DIR).join("hidden.txt"),
            b"hidden",
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            root.path().join("src").join("main.rs"),
            root.path().join("linked.rs"),
        )
        .unwrap();

        let records = list_workspace_files(root.path()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].relative_path, "src/main.rs");
        assert_eq!(records[0].size, 12);
    }

    #[test]
    fn missing_or_non_directory_roots_are_rejected() {
        let root = tempdir().unwrap();
        let file = root.path().join("file");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(
            list_workspace_files(root.path().join("missing")),
            Err(DiscoveryError::RootNotFound(_))
        ));
        assert!(matches!(
            list_workspace_files(file),
            Err(DiscoveryError::RootNotDirectory(_))
        ));
    }

    #[test]
    fn quarantine_path_moves_only_inside_codex_owned_roots() {
        let root = tempdir().unwrap();
        let codex_root = root.path().join("codex");
        fs::create_dir_all(codex_root.join("generated_images")).unwrap();
        let owned = codex_root.join("generated_images").join("image.png");
        fs::write(&owned, b"png").unwrap();
        let outside = root.path().join("outside.txt");
        fs::write(&outside, b"outside").unwrap();
        let result = quarantine_path_in_root(&owned, &codex_root).unwrap();
        assert!(result
            .quarantined_path
            .ends_with("generated_images/.quarantine/image.png"));
        assert!(!owned.exists());
        assert!(matches!(
            quarantine_path_in_root(Path::new(&result.quarantined_path), &codex_root),
            Err(DiscoveryError::PathNotAllowed(_))
        ));
        assert!(quarantine_path_in_root(&outside, &codex_root).is_err());
    }

    #[test]
    fn quarantine_rejects_traversal_and_symbolic_links() {
        let root = tempdir().unwrap();
        let codex_root = root.path().join("codex");
        let generated = codex_root.join("generated_images");
        fs::create_dir_all(&generated).unwrap();
        let outside = root.path().join("outside.png");
        fs::write(&outside, b"outside").unwrap();
        let traversal = generated.join("..").join("outside.png");
        assert!(matches!(
            quarantine_path_in_root(&traversal, &codex_root),
            Err(DiscoveryError::InvalidPath(_))
        ));

        #[cfg(unix)]
        {
            let link = generated.join("linked.png");
            std::os::unix::fs::symlink(&outside, &link).unwrap();
            assert!(matches!(
                quarantine_path_in_root(&link, &codex_root),
                Err(DiscoveryError::SymlinkRejected(_))
            ));
        }
    }
}
