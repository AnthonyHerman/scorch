use crate::model::DirEntry;
use crate::sunburst::Segment;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Application state
#[derive(Debug, Clone)]
pub struct AppState {
    /// Root of the scanned tree
    pub scan_root: Option<DirEntry>,
    /// Current view root (for zooming)
    pub view_root: PathBuf,
    /// Currently hovered segment
    pub hover_path: Option<PathBuf>,
    /// Cached segments for current view
    pub segments: Vec<Segment>,
    /// Is scanning in progress
    pub scanning: bool,
    /// Scan progress message
    pub progress_msg: String,
    /// Items scanned count
    pub items_scanned: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            scan_root: None,
            view_root: PathBuf::from("/"),
            hover_path: None,
            segments: Vec::new(),
            scanning: false,
            progress_msg: String::new(),
            items_scanned: 0,
        }
    }
}

impl AppState {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    /// Get current view entry from scan root
    pub fn get_view_entry(&self) -> Option<&DirEntry> {
        self.scan_root
            .as_ref()
            .and_then(|root| root.find_by_path(&self.view_root))
    }

    /// Navigate to a subdirectory
    pub fn navigate_to(&mut self, path: PathBuf) {
        if self.scan_root.as_ref().and_then(|r| r.find_by_path(&path)).is_some() {
            self.view_root = path;
            self.rebuild_segments();
        }
    }

    /// Navigate to parent directory
    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.view_root.parent() {
            let parent = parent.to_path_buf();
            // Only navigate up if we can find the parent in our tree
            if self.scan_root.as_ref().and_then(|r| r.find_by_path(&parent)).is_some() {
                self.view_root = parent;
                self.rebuild_segments();
            }
        }
    }

    /// Check if we can navigate up
    pub fn can_navigate_up(&self) -> bool {
        if let Some(root) = &self.scan_root {
            self.view_root != root.path && self.view_root.parent().is_some()
        } else {
            false
        }
    }

    /// Rebuild segments from current view
    pub fn rebuild_segments(&mut self) {
        if let Some(entry) = self.get_view_entry() {
            self.segments = crate::sunburst::build_segments(entry, crate::sunburst::MAX_DEPTH);
        }
    }

    /// Get breadcrumb path components
    pub fn get_breadcrumbs(&self) -> Vec<(PathBuf, String)> {
        let mut crumbs = Vec::new();
        let mut current = self.view_root.clone();

        // Build path from current up to root
        loop {
            let name = current
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| current.to_string_lossy().to_string());
            crumbs.push((current.clone(), name));

            if let Some(root) = &self.scan_root {
                if current == root.path {
                    break;
                }
            }

            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        crumbs.reverse();
        crumbs
    }
}
