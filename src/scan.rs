use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct Discovery {
    pub repos: Vec<PathBuf>,
    pub unreadable: usize,
}

pub fn discover(root: &Path) -> Discovery {
    discover_until(root, || false)
}

/// Return the repositories discovered so far if cancellation is requested.
pub fn discover_until(root: &Path, cancelled: impl Fn() -> bool) -> Discovery {
    let mut found = Discovery::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        if cancelled() {
            break;
        }
        // symlink_metadata does not follow a .git symlink either.
        if let Ok(meta) = fs::symlink_metadata(dir.join(".git")) {
            if meta.is_dir() || meta.is_file() {
                found.repos.push(dir);
                continue;
            }
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                found.unreadable += 1;
                continue;
            }
        };
        let mut unreadable_entry = false;
        for entry in entries {
            if cancelled() {
                break;
            }
            match entry.and_then(|entry| entry.file_type().map(|kind| (entry.path(), kind))) {
                Ok((path, kind)) if kind.is_dir() => pending.push(path),
                Ok(_) => {}
                Err(_) => unreadable_entry = true,
            }
        }
        found.unreadable += usize::from(unreadable_entry);
    }
    found.repos.sort();
    found
}
