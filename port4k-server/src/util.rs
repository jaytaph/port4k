use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{bail, Context};
use crate::hardering::{ALLOW_SYMLINKS, MAX_FILES_PER_IMPORT, MAX_FILE_BYTES, MAX_TOTAL_BYTES};

pub mod args;
pub mod telnet;


pub fn resolve_content_subdir(base: &Path, subdir: &str) -> anyhow::Result<PathBuf> {
    if subdir.is_empty() || subdir == "." || subdir == ".." { bail!("invalid subdir"); }
    if subdir.contains('/') || subdir.contains('\\') { bail!("subdir must be a single name"); }

    let base_can = base.canonicalize()
        .with_context(|| format!("canonicalizing base dir {:?}", base))?;
    let joined = base_can.join(subdir);

    // Avoid following symlinks for the subdir itself when ALLOW_SYMLINKS = false
    if !ALLOW_SYMLINKS {
        let md = fs::symlink_metadata(&joined)
            .with_context(|| format!("stat {}", joined.display()))?;
        if md.file_type().is_symlink() {
            bail!("subdir is a symlink, not allowed: {}", joined.display());
        }
    }

    let target_can = joined.canonicalize()
        .with_context(|| format!("canonicalizing {}", joined.display()))?;
    if !target_can.starts_with(&base_can) { bail!("path escapes content base"); }
    if !target_can.is_dir() { bail!("not a directory: {}", target_can.display()); }

    Ok(target_can)
}

pub fn list_yaml_files_guarded(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut total = 0usize;

    for e in fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let e = e?;
        let p = e.path();
        if !p.is_file() { continue; }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !(name.ends_with(".yml") || name.ends_with(".yaml")) { continue; }

        if !ALLOW_SYMLINKS {
            let md = fs::symlink_metadata(&p)?;
            if md.file_type().is_symlink() { // skip symlinked files
                continue;
            }
        }
        let len = fs::metadata(&p)?.len() as usize;
        if len > MAX_FILE_BYTES { bail!("file too large: {} ({} bytes)", p.display(), len); }
        total = total.saturating_add(len);
        if total > MAX_TOTAL_BYTES { bail!("import exceeds total size limit"); }

        files.push(p);
        if files.len() > MAX_FILES_PER_IMPORT { bail!("too many files (> {})", MAX_FILES_PER_IMPORT); }
    }
    files.sort();
    Ok(files)
}