use crate::models::{ArtifactRecord, ArtifactStatus};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const QUARANTINE_DIR: &str = ".quarantine";

#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub max_file_bytes: u64,
    pub allowed_extensions: BTreeSet<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 25 * 1024 * 1024,
            allowed_extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("invalid relative path: {0}")]
    InvalidPath(String),
    #[error("path is outside the artifact root: {0}")]
    OutsideRoot(PathBuf),
    #[error("artifact root is not a directory: {0}")]
    RootNotDirectory(PathBuf),
    #[error("job directory does not exist: {0}")]
    JobNotFound(PathBuf),
    #[error("artifact record is not quarantined")]
    NotQuarantined,
    #[error("restore destination already exists: {0}")]
    RestoreConflict(PathBuf),
    #[error("I/O error for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

impl ArtifactError {
    fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

/// Scans files below `root`. Symlinks are never followed and are marked for
/// review. Indexing is read-only: cleanup only happens after an explicit call
/// to `quarantine_record`.
#[derive(Clone, Debug)]
pub struct ArtifactScanner {
    root: PathBuf,
    canonical_root: PathBuf,
    config: ScanConfig,
}

impl ArtifactScanner {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, ArtifactError> {
        Self::with_config(root, ScanConfig::default())
    }

    pub fn with_config(
        root: impl Into<PathBuf>,
        config: ScanConfig,
    ) -> Result<Self, ArtifactError> {
        let root = root.into();
        if root.exists() {
            let metadata = fs::symlink_metadata(&root).map_err(|e| ArtifactError::io(&root, e))?;
            if !metadata.is_dir() {
                return Err(ArtifactError::RootNotDirectory(root));
            }
        } else {
            fs::create_dir_all(&root).map_err(|e| ArtifactError::io(&root, e))?;
        }
        let canonical_root = fs::canonicalize(&root).map_err(|e| ArtifactError::io(&root, e))?;
        Ok(Self {
            root,
            canonical_root,
            config,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn scan_job(
        &self,
        job_id: &str,
        now_ms: u64,
    ) -> Result<Vec<ArtifactRecord>, ArtifactError> {
        validate_component(job_id)?;
        if job_id == QUARANTINE_DIR {
            return Err(ArtifactError::InvalidPath(job_id.into()));
        }
        let job_dir = self.root.join(job_id);
        let metadata = fs::symlink_metadata(&job_dir).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ArtifactError::JobNotFound(job_dir.clone())
            } else {
                ArtifactError::io(&job_dir, error)
            }
        })?;
        if !metadata.is_dir() {
            return Err(ArtifactError::JobNotFound(job_dir));
        }
        self.ensure_inside(&job_dir)?;
        let mut records = Vec::new();
        self.scan_dir(&job_dir, job_id, now_ms, &mut records)?;
        Ok(records)
    }

    fn scan_dir(
        &self,
        dir: &Path,
        job_id: &str,
        now_ms: u64,
        records: &mut Vec<ArtifactRecord>,
    ) -> Result<(), ArtifactError> {
        for entry in fs::read_dir(dir).map_err(|e| ArtifactError::io(dir, e))? {
            let entry = entry.map_err(|e| ArtifactError::io(dir, e))?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&self.root)
                .map_err(|_| ArtifactError::OutsideRoot(path.clone()))?;
            if relative.starts_with(QUARANTINE_DIR) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|e| ArtifactError::io(&path, e))?;
            let rel_string = relative.to_string_lossy().to_string();
            if metadata.file_type().is_symlink() {
                records.push(ArtifactRecord {
                    id: id_for(job_id, &rel_string, 0, "symlink"),
                    job_id: job_id.into(),
                    source_path: rel_string,
                    quarantined_path: None,
                    status: ArtifactStatus::Review,
                    mime_type: None,
                    size_bytes: 0,
                    sha256: None,
                    reason: Some("symlink is never followed automatically".into()),
                    created_at_ms: now_ms,
                });
                continue;
            }
            if metadata.is_dir() {
                // Verify the directory target before descending.  This also
                // closes the common symlink replacement race between
                // `symlink_metadata` and `read_dir`.
                self.ensure_inside(&path)?;
                self.scan_dir(&path, job_id, now_ms, records)?;
                continue;
            }
            if !metadata.is_file() {
                records.push(ArtifactRecord {
                    id: id_for(job_id, &rel_string, metadata.len(), "unsupported"),
                    job_id: job_id.into(),
                    source_path: rel_string,
                    quarantined_path: None,
                    status: ArtifactStatus::Review,
                    mime_type: None,
                    size_bytes: metadata.len(),
                    sha256: None,
                    reason: Some("unsupported file type".into()),
                    created_at_ms: now_ms,
                });
                continue;
            }
            let size = metadata.len();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase());
            if size > self.config.max_file_bytes {
                records.push(ArtifactRecord {
                    id: id_for(job_id, &rel_string, size, "oversize"),
                    job_id: job_id.into(),
                    source_path: rel_string,
                    quarantined_path: None,
                    status: ArtifactStatus::Review,
                    mime_type: extension
                        .as_deref()
                        .and_then(mime_for_extension)
                        .map(str::to_owned),
                    size_bytes: size,
                    sha256: None,
                    reason: Some("file exceeds configured size limit".into()),
                    created_at_ms: now_ms,
                });
                continue;
            }
            if extension
                .as_deref()
                .map(|value| !self.config.allowed_extensions.contains(value))
                .unwrap_or(true)
            {
                records.push(ArtifactRecord {
                    id: id_for(job_id, &rel_string, size, "extension"),
                    job_id: job_id.into(),
                    source_path: rel_string,
                    quarantined_path: None,
                    status: ArtifactStatus::Review,
                    mime_type: extension
                        .as_deref()
                        .and_then(mime_for_extension)
                        .map(str::to_owned),
                    size_bytes: size,
                    sha256: None,
                    reason: Some("file extension is not in the configured preview set".into()),
                    created_at_ms: now_ms,
                });
                continue;
            }
            // Re-check canonical location immediately before opening to avoid
            // following a symlink introduced between metadata and read.
            self.ensure_inside(&path)?;
            let mut file = File::open(&path).map_err(|e| ArtifactError::io(&path, e))?;
            let digest = digest_reader(&mut file, &path)?;
            let id = id_for(job_id, &rel_string, size, &digest);
            records.push(ArtifactRecord {
                id,
                job_id: job_id.into(),
                source_path: rel_string,
                quarantined_path: None,
                status: ArtifactStatus::Accepted,
                mime_type: extension
                    .as_deref()
                    .and_then(mime_for_extension)
                    .map(str::to_owned),
                size_bytes: size,
                sha256: Some(digest),
                reason: None,
                created_at_ms: now_ms,
            });
        }
        Ok(())
    }

    /// Move one explicitly selected artifact into the in-root quarantine.
    pub fn quarantine_record(
        &self,
        record: &ArtifactRecord,
    ) -> Result<ArtifactRecord, ArtifactError> {
        if !matches!(
            record.status,
            ArtifactStatus::Accepted | ArtifactStatus::Review
        ) {
            return Err(ArtifactError::NotQuarantined);
        }
        validate_relative(&record.source_path)?;
        let path = self.root.join(&record.source_path);
        let metadata = fs::symlink_metadata(&path).map_err(|e| ArtifactError::io(&path, e))?;
        self.quarantine(
            &path,
            &record.job_id,
            record.source_path.clone(),
            metadata.len(),
            record.created_at_ms,
            record.reason.as_deref().unwrap_or("selected by user"),
        )
    }

    fn quarantine(
        &self,
        path: &Path,
        job_id: &str,
        source_path: String,
        size: u64,
        now_ms: u64,
        reason: &str,
    ) -> Result<ArtifactRecord, ArtifactError> {
        // Use a lexical check here.  `path` may itself be a symlink and must
        // be moved as a directory entry without canonicalizing/following it.
        self.ensure_lexical_inside(path)?;
        validate_relative(&source_path)?;
        let id = id_for(job_id, &source_path, size, reason);
        let quarantine_dir = self.root.join(QUARANTINE_DIR).join(job_id);
        fs::create_dir_all(&quarantine_dir).map_err(|e| ArtifactError::io(&quarantine_dir, e))?;
        // A pre-existing symlink at `.quarantine/<job>` must not redirect a
        // move outside the root.
        self.ensure_inside(&quarantine_dir)?;
        let file_name = format!(
            "{}-{}",
            id,
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("artifact")
        );
        let quarantine_path = quarantine_dir.join(file_name);
        move_entry(path, &quarantine_path)?;
        let quarantine_relative = quarantine_path
            .strip_prefix(&self.root)
            .map_err(|_| ArtifactError::OutsideRoot(quarantine_path.clone()))?
            .to_string_lossy()
            .to_string();
        Ok(ArtifactRecord {
            id,
            job_id: job_id.into(),
            source_path,
            quarantined_path: Some(quarantine_relative),
            status: ArtifactStatus::Quarantined,
            mime_type: None,
            size_bytes: size,
            sha256: None,
            reason: Some(reason.into()),
            created_at_ms: now_ms,
        })
    }

    pub fn restore(&self, record: &ArtifactRecord) -> Result<ArtifactRecord, ArtifactError> {
        if record.status != ArtifactStatus::Quarantined {
            return Err(ArtifactError::NotQuarantined);
        }
        let quarantine_relative = record
            .quarantined_path
            .as_deref()
            .ok_or(ArtifactError::NotQuarantined)?;
        validate_relative(&record.source_path)?;
        validate_relative(quarantine_relative)?;
        let source = self.root.join(&record.source_path);
        let quarantined = self.root.join(quarantine_relative);
        // The source normally does not exist while restoring, so canonicalize
        // only the quarantine entry and use a lexical root check for the
        // destination.
        self.ensure_lexical_inside(&source)?;
        self.ensure_lexical_inside(&quarantined)?;
        if let Some(parent) = quarantined.parent() {
            self.ensure_inside(parent)?;
        }
        if let Some(parent) = source.parent() {
            self.ensure_inside(parent)?;
        }
        if fs::symlink_metadata(&source).is_ok() {
            return Err(ArtifactError::RestoreConflict(source));
        }
        if fs::symlink_metadata(&quarantined).is_err() {
            return Err(ArtifactError::io(
                &quarantined,
                io::Error::new(io::ErrorKind::NotFound, "quarantined artifact is missing"),
            ));
        }
        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent).map_err(|e| ArtifactError::io(parent, e))?;
        }
        move_entry(&quarantined, &source)?;
        let mut restored = record.clone();
        restored.status = ArtifactStatus::Restored;
        restored.quarantined_path = None;
        Ok(restored)
    }

    fn ensure_inside(&self, path: &Path) -> Result<(), ArtifactError> {
        let candidate = fs::canonicalize(path).map_err(|e| ArtifactError::io(path, e))?;
        if !candidate.starts_with(&self.canonical_root) {
            return Err(ArtifactError::OutsideRoot(path.to_path_buf()));
        }
        Ok(())
    }

    fn ensure_lexical_inside(&self, path: &Path) -> Result<(), ArtifactError> {
        if !path.starts_with(&self.root) {
            return Err(ArtifactError::OutsideRoot(path.to_path_buf()));
        }
        Ok(())
    }
}

fn validate_component(value: &str) -> Result<(), ArtifactError> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(ArtifactError::InvalidPath(value.into()));
    }
    Ok(())
}

fn validate_relative(value: &str) -> Result<(), ArtifactError> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ArtifactError::InvalidPath(value.into()));
    }
    Ok(())
}

fn move_entry(source: &Path, destination: &Path) -> Result<(), ArtifactError> {
    if fs::symlink_metadata(destination).is_ok() {
        return Err(ArtifactError::RestoreConflict(destination.to_path_buf()));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| ArtifactError::io(parent, e))?;
    }
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // This fallback is only for ordinary files.  A symlink is never
            // read or copied, so a failed symlink rename leaves it untouched.
            let metadata =
                fs::symlink_metadata(source).map_err(|e| ArtifactError::io(source, e))?;
            if !metadata.is_file() {
                return Err(ArtifactError::io(source, rename_error));
            }
            fs::copy(source, destination).map_err(|e| ArtifactError::io(destination, e))?;
            fs::remove_file(source).map_err(|e| ArtifactError::io(source, e))
        }
    }
}

fn digest_reader(reader: &mut impl Read, path: &Path) -> Result<String, ArtifactError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| ArtifactError::io(path, e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_string(&hasher.finalize()))
}

fn id_for(job_id: &str, source: &str, size: u64, extra: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(job_id.as_bytes());
    hasher.update([0]);
    hasher.update(source.as_bytes());
    hasher.update([0]);
    hasher.update(size.to_le_bytes());
    hasher.update([0]);
    hasher.update(extra.as_bytes());
    hex_string(&hasher.finalize())[..24].to_owned()
}

fn hex_string(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn mime_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scans_allowed_file_and_hashes_it() {
        let root = tempdir().unwrap();
        let job = root.path().join("job-1");
        fs::create_dir_all(&job).unwrap();
        fs::write(job.join("result.png"), b"pixels").unwrap();
        let scanner = ArtifactScanner::new(root.path()).unwrap();
        let records = scanner.scan_job("job-1", 42).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, ArtifactStatus::Accepted);
        assert_eq!(records[0].source_path, "job-1/result.png");
        assert_eq!(records[0].mime_type.as_deref(), Some("image/png"));
        assert_eq!(
            records[0].sha256.as_deref(),
            Some("6ec9c2b0eb14010746c8bce8939303b382344b296206612eb8a907a37b2b2f37")
        );
    }

    #[test]
    fn indexing_does_not_move_unsupported_files_and_explicit_cleanup_restores_them() {
        let root = tempdir().unwrap();
        let job = root.path().join("job-1");
        fs::create_dir_all(&job).unwrap();
        fs::write(job.join("result.txt"), b"secret").unwrap();
        let scanner = ArtifactScanner::new(root.path()).unwrap();
        let records = scanner.scan_job("job-1", 42).unwrap();
        assert_eq!(records[0].status, ArtifactStatus::Review);
        assert!(job.join("result.txt").exists());
        let quarantined = scanner.quarantine_record(&records[0]).unwrap();
        assert!(!job.join("result.txt").exists());
        let restored = scanner.restore(&quarantined).unwrap();
        assert_eq!(restored.status, ArtifactStatus::Restored);
        assert_eq!(fs::read(job.join("result.txt")).unwrap(), b"secret");
    }

    #[test]
    fn rejects_job_path_traversal() {
        let root = tempdir().unwrap();
        let scanner = ArtifactScanner::new(root.path()).unwrap();
        assert!(matches!(
            scanner.scan_job("../outside", 0),
            Err(ArtifactError::InvalidPath(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_marked_for_review_without_following_target() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let target = tempdir().unwrap();
        fs::write(target.path().join("secret.png"), b"secret").unwrap();
        let job = root.path().join("job-1");
        fs::create_dir_all(&job).unwrap();
        symlink(target.path().join("secret.png"), job.join("linked.png")).unwrap();
        let scanner = ArtifactScanner::new(root.path()).unwrap();
        let records = scanner.scan_job("job-1", 42).unwrap();
        assert_eq!(records[0].status, ArtifactStatus::Review);
        assert!(target.path().join("secret.png").exists());
    }
}
