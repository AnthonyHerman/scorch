use std::path::PathBuf;

/// File type categories for color coding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Directory,
    Video,
    Image,
    Audio,
    Archive,
    Document,
    Code,
    Other,
}

impl FileType {
    /// Determine file type from extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // Video
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpeg" | "mpg" => {
                FileType::Video
            }
            // Image
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "raw" => {
                FileType::Image
            }
            // Audio
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" | "m4a" | "opus" => FileType::Audio,
            // Archive
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz4" => FileType::Archive,
            // Document
            "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx" => {
                FileType::Document
            }
            // Code
            "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "java" | "go" | "rb" | "php" | "sh"
            | "bash" | "zsh" | "json" | "yaml" | "yml" | "toml" | "xml" | "html" | "css"
            | "scss" | "md" | "sql" => FileType::Code,
            _ => FileType::Other,
        }
    }

    /// Get RGBA color for this file type (fire/heat themed)
    pub fn color(&self) -> (f64, f64, f64, f64) {
        match self {
            FileType::Directory => (0.6, 0.25, 0.1, 1.0),   // Deep ember
            FileType::Video => (1.0, 0.2, 0.1, 1.0),        // Hot red (big files!)
            FileType::Image => (1.0, 0.5, 0.0, 1.0),        // Flame orange
            FileType::Audio => (0.9, 0.3, 0.5, 1.0),        // Magenta fire
            FileType::Archive => (1.0, 0.8, 0.0, 1.0),      // Golden flame
            FileType::Document => (1.0, 0.6, 0.2, 1.0),     // Warm orange
            FileType::Code => (0.8, 0.4, 0.1, 1.0),         // Copper ember
            FileType::Other => (0.5, 0.2, 0.15, 1.0),       // Cool ember
        }
    }
}

/// A directory or file entry with size information
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub file_type: FileType,
    pub children: Vec<DirEntry>,
    pub is_file: bool,
}

impl DirEntry {
    /// Create a new directory entry
    pub fn new_dir(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        Self {
            path,
            name,
            size: 0,
            file_type: FileType::Directory,
            children: Vec::new(),
            is_file: false,
        }
    }

    /// Create a new file entry
    pub fn new_file(path: PathBuf, size: u64) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let file_type = path
            .extension()
            .map(|ext| FileType::from_extension(&ext.to_string_lossy()))
            .unwrap_or(FileType::Other);
        Self {
            path,
            name,
            size,
            file_type,
            children: Vec::new(),
            is_file: true,
        }
    }

    /// Calculate total size including all children
    pub fn total_size(&self) -> u64 {
        if self.is_file {
            self.size
        } else {
            self.children.iter().map(|c| c.total_size()).sum()
        }
    }

    /// Get the number of items (files + directories) including self
    pub fn item_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.item_count()).sum::<usize>()
    }

    /// Sort children by size (largest first)
    pub fn sort_by_size(&mut self) {
        self.children.sort_by(|a, b| b.total_size().cmp(&a.total_size()));
        for child in &mut self.children {
            child.sort_by_size();
        }
    }

    /// Find entry by path
    pub fn find_by_path(&self, target: &PathBuf) -> Option<&DirEntry> {
        if &self.path == target {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_path(target) {
                return Some(found);
            }
        }
        None
    }

    /// Get parent path
    pub fn parent_path(&self) -> Option<PathBuf> {
        self.path.parent().map(|p| p.to_path_buf())
    }
}

/// Format bytes into human-readable string
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Protected system paths that cannot be deleted
pub const PROTECTED_PATHS: &[&str] = &[
    "/",
    "/usr",
    "/etc",
    "/bin",
    "/sbin",
    "/lib",
    "/lib64",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/run",
    "/var",
    "/root",
];

/// Check if a path is protected from deletion
pub fn is_protected_path(path: &PathBuf) -> bool {
    let path_str = path.to_string_lossy();
    PROTECTED_PATHS.iter().any(|p| path_str == *p)
}
