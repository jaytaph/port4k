use crate::error::{AppResult, DomainError, InfraError};
use crate::hardening::{ALLOW_SYMLINKS, MAX_FILE_BYTES, MAX_FILES_PER_IMPORT, MAX_TOTAL_BYTES};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub mod args;
pub mod helpers;
pub mod telnet;
pub mod serde;

pub fn resolve_content_subdir(base: &Path, subdir: &str) -> AppResult<PathBuf> {
    let p = Path::new(subdir);

    let mut comps = p.components();
    match comps.next() {
        Some(Component::Normal(_)) if comps.next().is_none() => {} // ok
        _ => {
            return Err(DomainError::Validation {
                field: "subdir",
                message: "must be a single path segment".into(),
            });
        }
    }

    // Resolve base and join
    let base_can = base.canonicalize().map_err(InfraError::from)?;
    let joined = base_can.join(p);

    if !ALLOW_SYMLINKS {
        let md = fs::symlink_metadata(&joined).map_err(InfraError::from)?;
        if md.file_type().is_symlink() {
            return Err(DomainError::Validation {
                field: "subdir",
                message: format!("symlink not allowed: {}", joined.display()),
            });
        }
    }

    // Canonicalize the target and enforce containment
    let target_can = joined.canonicalize().map_err(InfraError::from)?;
    if !target_can.starts_with(&base_can) {
        return Err(DomainError::Validation {
            field: "subdir",
            message: "path escapes content base".into(),
        });
    }
    if !target_can.is_dir() {
        return Err(DomainError::Validation {
            field: "subdir",
            message: format!("not a directory: {}", target_can.display()),
        });
    }

    Ok(target_can)
}

pub fn list_yaml_files_guarded(dir: &Path) -> AppResult<Vec<PathBuf>> {
    use std::fs;

    let mut files = Vec::new();
    let mut total: u64 = 0;

    for entry in fs::read_dir(dir).map_err(InfraError::from)? {
        let entry = entry.map_err(InfraError::from)?;
        let path = entry.path();

        // Only plain files
        if !entry.file_type().map_err(InfraError::from)?.is_file() {
            continue;
        }

        // Only .yml / .yaml
        match path.extension().and_then(|s| s.to_str()) {
            Some("yml") | Some("yaml") => {}
            _ => continue,
        }

        if !ALLOW_SYMLINKS
            && fs::symlink_metadata(&path)
                .map_err(InfraError::from)?
                .file_type()
                .is_symlink()
        {
            continue;
        }

        // Enforce per-file size and cumulative limits
        let len = fs::metadata(&path).map_err(InfraError::from)?.len(); // u64
        if len > MAX_FILE_BYTES as u64 {
            return Err(DomainError::Validation {
                field: "import",
                message: format!("file too large: {} ({} bytes)", path.display(), len),
            });
        }

        total = total.saturating_add(len);
        if total > MAX_TOTAL_BYTES as u64 {
            return Err(DomainError::Validation {
                field: "import",
                message: "import exceeds total size limit".into(),
            });
        }

        files.push(path);

        if files.len() > MAX_FILES_PER_IMPORT {
            return Err(DomainError::Validation {
                field: "import",
                message: format!("too many files (> {})", MAX_FILES_PER_IMPORT),
            });
        }
    }

    files.sort();
    Ok(files)
}
