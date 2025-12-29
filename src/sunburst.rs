use crate::model::{format_size, DirEntry, FileType};
use gtk4::cairo::{Context, FontSlant, FontWeight};
use std::f64::consts::PI;
use std::path::PathBuf;

/// Maximum depth of rings to display
pub const MAX_DEPTH: usize = 5;

/// Minimum angle (radians) for a segment to be rendered
const MIN_ANGLE: f64 = 0.01;

/// A segment in the sunburst chart
#[derive(Debug, Clone)]
pub struct Segment {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub file_type: FileType,
    pub depth: usize,
    pub start_angle: f64,
    pub end_angle: f64,
    pub is_file: bool,
}

impl Segment {
    /// Check if a point (in polar coords) is inside this segment
    pub fn contains_point(&self, angle: f64, radius_depth: usize) -> bool {
        if radius_depth != self.depth {
            return false;
        }
        // Normalize angle to [0, 2*PI)
        let mut a = angle;
        while a < 0.0 {
            a += 2.0 * PI;
        }
        while a >= 2.0 * PI {
            a -= 2.0 * PI;
        }
        a >= self.start_angle && a < self.end_angle
    }
}

/// Build segments from a DirEntry tree
pub fn build_segments(root: &DirEntry, max_depth: usize) -> Vec<Segment> {
    let mut segments = Vec::new();
    let total_size = root.total_size();
    if total_size == 0 {
        return segments;
    }

    // Add center segment for root
    segments.push(Segment {
        path: root.path.clone(),
        name: root.name.clone(),
        size: total_size,
        file_type: root.file_type,
        depth: 0,
        start_angle: 0.0,
        end_angle: 2.0 * PI,
        is_file: root.is_file,
    });

    // Build child segments recursively
    build_segments_recursive(root, 1, 0.0, 2.0 * PI, total_size, max_depth, &mut segments);

    segments
}

fn build_segments_recursive(
    entry: &DirEntry,
    depth: usize,
    start_angle: f64,
    end_angle: f64,
    total_size: u64,
    max_depth: usize,
    segments: &mut Vec<Segment>,
) {
    if depth > max_depth {
        return;
    }

    let angle_range = end_angle - start_angle;
    let mut current_angle = start_angle;

    for child in &entry.children {
        let child_size = child.total_size();
        if child_size == 0 {
            continue;
        }

        let child_angle = (child_size as f64 / total_size as f64) * angle_range;
        if child_angle < MIN_ANGLE {
            continue; // Skip tiny segments
        }

        let child_end = current_angle + child_angle;

        segments.push(Segment {
            path: child.path.clone(),
            name: child.name.clone(),
            size: child_size,
            file_type: child.file_type,
            depth,
            start_angle: current_angle,
            end_angle: child_end,
            is_file: child.is_file,
        });

        // Recurse into directories
        if !child.is_file && !child.children.is_empty() {
            build_segments_recursive(
                child,
                depth + 1,
                current_angle,
                child_end,
                child_size,
                max_depth,
                segments,
            );
        }

        current_angle = child_end;
    }
}

/// Find segment at a given point
pub fn find_segment_at_point(
    segments: &[Segment],
    x: f64,
    y: f64,
    center_x: f64,
    center_y: f64,
    ring_width: f64,
) -> Option<&Segment> {
    let dx = x - center_x;
    let dy = y - center_y;
    let distance = (dx * dx + dy * dy).sqrt();

    // Calculate which ring (depth) we're in
    let depth = if distance < ring_width {
        0 // Center
    } else {
        ((distance / ring_width).floor() as usize).min(MAX_DEPTH)
    };

    // Calculate angle
    let mut angle = dy.atan2(dx);
    if angle < 0.0 {
        angle += 2.0 * PI;
    }

    // Find matching segment
    segments
        .iter()
        .find(|s| s.depth == depth && angle >= s.start_angle && angle < s.end_angle)
}

/// Draw the sunburst chart
pub fn draw_sunburst(
    cr: &Context,
    segments: &[Segment],
    width: f64,
    height: f64,
    hover_path: Option<&PathBuf>,
) {
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    let max_radius = (width.min(height) / 2.0) - 20.0;
    let ring_width = max_radius / (MAX_DEPTH as f64 + 1.0);

    // Draw background (dark ember)
    cr.set_source_rgb(0.1, 0.07, 0.08);
    cr.paint().unwrap();

    // Draw segments by depth (inner to outer)
    for depth in 0..=MAX_DEPTH {
        let inner_radius = if depth == 0 { 0.0 } else { ring_width * depth as f64 };
        let outer_radius = ring_width * (depth as f64 + 1.0);

        for segment in segments.iter().filter(|s| s.depth == depth) {
            draw_segment(
                cr,
                segment,
                center_x,
                center_y,
                inner_radius,
                outer_radius,
                hover_path,
            );
        }
    }

    // Draw center text
    if let Some(root) = segments.first() {
        draw_center_text(cr, root, center_x, center_y, ring_width);
    }
}

fn draw_segment(
    cr: &Context,
    segment: &Segment,
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
    outer_radius: f64,
    hover_path: Option<&PathBuf>,
) {
    let is_hovered = hover_path.map_or(false, |p| p == &segment.path);
    let (r, g, b, a) = segment.file_type.color();

    // Adjust color based on depth for visual hierarchy
    let depth_factor = 1.0 - (segment.depth as f64 * 0.1);
    let (r, g, b) = (r * depth_factor, g * depth_factor, b * depth_factor);

    // Brighten on hover
    let (r, g, b) = if is_hovered {
        ((r + 0.2).min(1.0), (g + 0.2).min(1.0), (b + 0.2).min(1.0))
    } else {
        (r, g, b)
    };

    cr.set_source_rgba(r, g, b, a);

    if segment.depth == 0 {
        // Draw center circle
        cr.arc(center_x, center_y, outer_radius, 0.0, 2.0 * PI);
        cr.fill().unwrap();
    } else {
        // Draw arc segment
        cr.new_path();

        // Outer arc
        cr.arc(
            center_x,
            center_y,
            outer_radius,
            segment.start_angle,
            segment.end_angle,
        );

        // Line to inner arc
        cr.line_to(
            center_x + inner_radius * segment.end_angle.cos(),
            center_y + inner_radius * segment.end_angle.sin(),
        );

        // Inner arc (reversed)
        cr.arc_negative(
            center_x,
            center_y,
            inner_radius,
            segment.end_angle,
            segment.start_angle,
        );

        cr.close_path();
        cr.fill().unwrap();

        // Draw border (dark ember)
        cr.set_source_rgba(0.15, 0.08, 0.05, 1.0);
        cr.set_line_width(1.5);

        cr.new_path();
        cr.arc(
            center_x,
            center_y,
            outer_radius,
            segment.start_angle,
            segment.end_angle,
        );
        cr.line_to(
            center_x + inner_radius * segment.end_angle.cos(),
            center_y + inner_radius * segment.end_angle.sin(),
        );
        cr.arc_negative(
            center_x,
            center_y,
            inner_radius,
            segment.end_angle,
            segment.start_angle,
        );
        cr.close_path();
        cr.stroke().unwrap();
    }
}

fn draw_center_text(
    cr: &Context,
    root: &Segment,
    center_x: f64,
    center_y: f64,
    _ring_width: f64,
) {
    // Fiery gold/orange text
    cr.set_source_rgb(1.0, 0.85, 0.4);
    cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Bold);

    // Draw name
    let name = if root.name.len() > 20 {
        format!("{}...", &root.name[..17])
    } else {
        root.name.clone()
    };

    cr.set_font_size(16.0);
    let extents = cr.text_extents(&name).unwrap();
    cr.move_to(center_x - extents.width() / 2.0, center_y - 8.0);
    cr.show_text(&name).unwrap();

    // Draw size in bright flame color
    cr.set_source_rgb(1.0, 0.6, 0.2);
    let size_text = format_size(root.size);
    cr.set_font_size(14.0);
    let extents = cr.text_extents(&size_text).unwrap();
    cr.move_to(center_x - extents.width() / 2.0, center_y + 12.0);
    cr.show_text(&size_text).unwrap();
}

/// Get ring width for hit detection
pub fn get_ring_width(width: f64, height: f64) -> f64 {
    let max_radius = (width.min(height) / 2.0) - 20.0;
    max_radius / (MAX_DEPTH as f64 + 1.0)
}
