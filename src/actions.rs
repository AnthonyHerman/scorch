use crate::model::{is_protected_path, DirEntry};
use std::fs;
use std::path::PathBuf;

/// Result of a delete operation
#[derive(Debug)]
pub enum DeleteResult {
    Success,
    ProtectedPath,
    NotFound,
    PermissionDenied(String),
    Error(String),
}

/// Delete a file or directory
pub fn delete_entry(path: &PathBuf) -> DeleteResult {
    // Check if protected
    if is_protected_path(path) {
        return DeleteResult::ProtectedPath;
    }

    // Check if exists
    if !path.exists() {
        return DeleteResult::NotFound;
    }

    // Attempt deletion
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };

    match result {
        Ok(_) => DeleteResult::Success,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                DeleteResult::PermissionDenied(e.to_string())
            } else {
                DeleteResult::Error(e.to_string())
            }
        }
    }
}

/// Get info about what will be deleted
pub fn get_delete_info(entry: &DirEntry) -> (usize, u64) {
    let count = entry.item_count();
    let size = entry.total_size();
    (count, size)
}
