use crate::model::DirEntry;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Virtual filesystems to skip (they don't represent real disk usage)
const VIRTUAL_FS_PATHS: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/snap",
];

/// Check if a path is a virtual filesystem that should be skipped
fn is_virtual_fs(path: &PathBuf) -> bool {
    let path_str = path.to_string_lossy();
    VIRTUAL_FS_PATHS.iter().any(|vfs| {
        path_str == *vfs || path_str.starts_with(&format!("{}/", vfs))
    })
}

/// Progress update during scanning
#[derive(Debug, Clone)]
pub enum ScanProgress {
    /// Currently scanning this directory
    Scanning(String),
    /// Number of items scanned so far
    ItemCount(usize),
    /// Scan completed with result
    Complete(DirEntry),
    /// Scan failed with error
    Error(String),
}

/// Start scanning a directory in a background thread
pub fn scan_directory(root: PathBuf) -> Receiver<ScanProgress> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        scan_recursive(&root, &tx, &mut 0);
    });

    rx
}

fn scan_recursive(path: &PathBuf, tx: &Sender<ScanProgress>, count: &mut usize) {
    // Send progress update
    *count += 1;
    if *count % 100 == 0 {
        let _ = tx.send(ScanProgress::ItemCount(*count));
    }
    let _ = tx.send(ScanProgress::Scanning(path.to_string_lossy().to_string()));

    match build_entry(path, tx, count) {
        Ok(mut entry) => {
            entry.sort_by_size();
            let _ = tx.send(ScanProgress::Complete(entry));
        }
        Err(e) => {
            let _ = tx.send(ScanProgress::Error(e));
        }
    }
}

fn build_entry(
    path: &PathBuf,
    tx: &Sender<ScanProgress>,
    count: &mut usize,
) -> Result<DirEntry, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

    if metadata.is_file() {
        return Ok(DirEntry::new_file(path.clone(), metadata.len()));
    }

    let mut entry = DirEntry::new_dir(path.clone());

    // Read directory contents
    let read_dir = fs::read_dir(path).map_err(|e| format!("Cannot read directory {}: {}", path.display(), e))?;

    for item in read_dir {
        let item = match item {
            Ok(i) => i,
            Err(_) => continue, // Skip entries we can't read
        };

        let item_path = item.path();
        *count += 1;

        if *count % 100 == 0 {
            let _ = tx.send(ScanProgress::ItemCount(*count));
        }

        // Get metadata (don't follow symlinks)
        let item_metadata = match fs::symlink_metadata(&item_path) {
            Ok(m) => m,
            Err(_) => continue, // Skip unreadable items
        };

        // Skip symlinks to avoid loops
        if item_metadata.is_symlink() {
            continue;
        }

        if item_metadata.is_file() {
            entry.children.push(DirEntry::new_file(item_path, item_metadata.len()));
        } else if item_metadata.is_dir() {
            // Skip virtual filesystems
            if is_virtual_fs(&item_path) {
                continue;
            }
            // Recursively scan subdirectory
            match build_entry_quiet(&item_path, count) {
                Ok(child) => entry.children.push(child),
                Err(_) => continue, // Skip directories we can't read
            }
        }
    }

    // Calculate size from children
    entry.size = entry.children.iter().map(|c| c.total_size()).sum();

    Ok(entry)
}

/// Build entry without sending progress (for recursive calls)
fn build_entry_quiet(path: &PathBuf, count: &mut usize) -> Result<DirEntry, String> {
    // Skip virtual filesystems
    if is_virtual_fs(path) {
        return Ok(DirEntry::new_dir(path.clone()));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

    if metadata.is_file() {
        return Ok(DirEntry::new_file(path.clone(), metadata.len()));
    }

    let mut entry = DirEntry::new_dir(path.clone());

    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return Ok(entry), // Return empty dir if unreadable
    };

    for item in read_dir {
        let item = match item {
            Ok(i) => i,
            Err(_) => continue,
        };

        let item_path = item.path();
        *count += 1;

        let item_metadata = match fs::symlink_metadata(&item_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if item_metadata.is_symlink() {
            continue;
        }

        if item_metadata.is_file() {
            entry.children.push(DirEntry::new_file(item_path, item_metadata.len()));
        } else if item_metadata.is_dir() {
            // Skip virtual filesystems
            if is_virtual_fs(&item_path) {
                continue;
            }
            match build_entry_quiet(&item_path, count) {
                Ok(child) => entry.children.push(child),
                Err(_) => continue,
            }
        }
    }

    entry.size = entry.children.iter().map(|c| c.total_size()).sum();

    Ok(entry)
}
