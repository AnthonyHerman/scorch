use crate::actions::{delete_entry, DeleteResult};
use crate::app::AppState;
use crate::model::{format_size, is_protected_path};
use crate::scanner::{scan_directory, ScanProgress};
use crate::sunburst::{draw_sunburst, find_segment_at_point, get_ring_width};

use gtk4::gdk::Display;
use gtk4::glib::{timeout_add_local, ControlFlow};
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, DrawingArea,
    FileChooserAction, FileChooserDialog, GestureClick, Label, MessageDialog, MessageType,
    ButtonsType, Orientation, ProgressBar, ResponseType,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

pub fn build_ui(app: &Application) {
    let state = AppState::new();

    // Main window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("SCORCH - Burn Through Your Disk")
        .default_width(900)
        .default_height(700)
        .build();

    // Apply dark theme CSS
    let provider = CssProvider::new();
    provider.load_from_data(
        r#"
        window, window.background {
            background-color: #1a1215;
        }
        button {
            background-image: none;
            background-color: #3d2020;
            color: #ffddcc;
            text-shadow: none;
            box-shadow: none;
            border: 1px solid #ff6633;
            padding: 8px 16px;
            border-radius: 6px;
        }
        button:hover {
            background-color: #5a3030;
            color: #ffffff;
            border-color: #ff8844;
        }
        button:disabled {
            background-color: #2a1818;
            color: #666666;
            border-color: #442222;
        }
        button label {
            color: #ffddcc;
        }
        label {
            color: #ffeeee;
        }
        .path-label {
            font-family: monospace;
            font-size: 13px;
            color: #ffaa88;
        }
        .status-label {
            font-size: 12px;
            color: #ffccaa;
        }
        .hover-label {
            font-size: 12px;
            color: #ffff88;
            font-weight: bold;
        }
        .breadcrumb {
            background-color: #3d2020;
            padding: 4px 8px;
            border-radius: 4px;
            margin: 2px;
            border: 1px solid #663322;
        }
        .breadcrumb:hover {
            background-color: #5a3030;
            border-color: #ff6633;
        }
        progressbar {
            min-height: 24px;
        }
        progressbar trough {
            background-color: #2a1818;
            border-radius: 4px;
            min-height: 24px;
        }
        progressbar progress {
            background-image: linear-gradient(to right, #ff4400, #ff8800, #ffaa00);
            background-color: #ff6600;
            border-radius: 4px;
            min-height: 24px;
        }
        progressbar text {
            color: #ffffff;
            font-weight: bold;
        }
        "#,
    );
    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Main container
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // Header bar with controls
    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.set_margin_start(12);
    header.set_margin_end(12);
    header.set_margin_top(8);
    header.set_margin_bottom(8);

    // Directory chooser button
    let choose_btn = Button::with_label("Target");
    let path_label = Label::new(Some("/"));
    path_label.add_css_class("path-label");
    path_label.set_hexpand(true);
    path_label.set_halign(Align::Start);

    // Scan button
    let scan_btn = Button::with_label("IGNITE");

    // Up button
    let up_btn = Button::with_label("↑ Escape");
    up_btn.set_sensitive(false);

    header.append(&choose_btn);
    header.append(&path_label);
    header.append(&up_btn);
    header.append(&scan_btn);

    // Breadcrumb bar
    let breadcrumb_box = GtkBox::new(Orientation::Horizontal, 4);
    breadcrumb_box.set_margin_start(12);
    breadcrumb_box.set_margin_end(12);
    breadcrumb_box.set_margin_bottom(4);

    // Drawing area for sunburst
    let drawing_area = DrawingArea::new();
    drawing_area.set_hexpand(true);
    drawing_area.set_vexpand(true);

    // Progress bar (hidden initially)
    let progress_bar = ProgressBar::new();
    progress_bar.add_css_class("scan-progress");
    progress_bar.set_margin_start(12);
    progress_bar.set_margin_end(12);
    progress_bar.set_margin_top(4);
    progress_bar.set_visible(false);

    // Status bar
    let status_bar = GtkBox::new(Orientation::Horizontal, 12);
    status_bar.set_margin_start(12);
    status_bar.set_margin_end(12);
    status_bar.set_margin_top(4);
    status_bar.set_margin_bottom(8);

    let status_label = Label::new(Some("Select a target and hit IGNITE to burn through your disk"));
    status_label.add_css_class("status-label");
    status_label.set_halign(Align::Start);
    status_label.set_hexpand(true);

    let hover_label = Label::new(Some(""));
    hover_label.add_css_class("hover-label");
    hover_label.set_halign(Align::End);

    status_bar.append(&status_label);
    status_bar.append(&hover_label);

    // Assemble UI
    main_box.append(&header);
    main_box.append(&breadcrumb_box);
    main_box.append(&drawing_area);
    main_box.append(&progress_bar);
    main_box.append(&status_bar);
    window.set_child(Some(&main_box));

    // Drawing handler
    let state_draw = state.clone();
    drawing_area.set_draw_func(move |_, cr, width, height| {
        let state = state_draw.borrow();
        let hover = state.hover_path.as_ref();
        draw_sunburst(cr, &state.segments, width as f64, height as f64, hover);
    });

    // Mouse motion for hover
    let motion_ctrl = gtk4::EventControllerMotion::new();
    let state_motion = state.clone();
    let drawing_area_motion = drawing_area.clone();
    let hover_label_motion = hover_label.clone();
    motion_ctrl.connect_motion(move |_, x, y| {
        let width = drawing_area_motion.width() as f64;
        let height = drawing_area_motion.height() as f64;
        let ring_width = get_ring_width(width, height);

        // Find segment first with immutable borrow
        let found = {
            let state = state_motion.borrow();
            find_segment_at_point(
                &state.segments,
                x,
                y,
                width / 2.0,
                height / 2.0,
                ring_width,
            ).map(|seg| (seg.path.clone(), seg.size))
        };

        // Then mutate with mutable borrow
        let mut state = state_motion.borrow_mut();
        if let Some((path, size)) = found {
            state.hover_path = Some(path.clone());
            hover_label_motion.set_text(&format!(
                "{} ({})",
                path.display(),
                format_size(size)
            ));
        } else {
            state.hover_path = None;
            hover_label_motion.set_text("");
        }
        drop(state);
        drawing_area_motion.queue_draw();
    });
    drawing_area.add_controller(motion_ctrl);

    // Left click for navigation
    let click_ctrl = GestureClick::new();
    click_ctrl.set_button(1); // Left click
    let state_click = state.clone();
    let drawing_area_click = drawing_area.clone();
    let up_btn_click = up_btn.clone();
    let breadcrumb_box_click = breadcrumb_box.clone();
    let state_bc = state.clone();
    let drawing_area_bc = drawing_area.clone();
    let up_btn_bc = up_btn.clone();
    click_ctrl.connect_released(move |_, _, x, y| {
        let width = drawing_area_click.width() as f64;
        let height = drawing_area_click.height() as f64;
        let ring_width = get_ring_width(width, height);

        // Find segment first with immutable borrow
        let found = {
            let state = state_click.borrow();
            find_segment_at_point(
                &state.segments,
                x,
                y,
                width / 2.0,
                height / 2.0,
                ring_width,
            ).map(|seg| (seg.depth, seg.is_file, seg.path.clone()))
        };

        if let Some((depth, is_file, path)) = found {
            let mut state = state_click.borrow_mut();
            if depth == 0 {
                // Click on center = go up
                state.navigate_up();
            } else if !is_file {
                // Click on directory = zoom in
                state.navigate_to(path);
            }
            up_btn_click.set_sensitive(state.can_navigate_up());

            // Update breadcrumbs
            update_breadcrumbs(
                &breadcrumb_box_click,
                &state.get_breadcrumbs(),
                state_bc.clone(),
                drawing_area_bc.clone(),
                up_btn_bc.clone(),
            );
        }
        drawing_area_click.queue_draw();
    });
    drawing_area.add_controller(click_ctrl);

    // Right click for context menu (delete)
    let right_click_ctrl = GestureClick::new();
    right_click_ctrl.set_button(3); // Right click
    let state_rclick = state.clone();
    let drawing_area_rclick = drawing_area.clone();
    let window_rclick = window.clone();
    right_click_ctrl.connect_released(move |_, _, x, y| {
        let width = drawing_area_rclick.width() as f64;
        let height = drawing_area_rclick.height() as f64;
        let ring_width = get_ring_width(width, height);

        // Find segment with immutable borrow, then release it
        let found = {
            let state = state_rclick.borrow();
            find_segment_at_point(
                &state.segments,
                x,
                y,
                width / 2.0,
                height / 2.0,
                ring_width,
            ).map(|seg| (seg.depth, seg.path.clone(), seg.name.clone(), seg.size, seg.is_file))
        };

        if let Some((depth, path, name, size, is_file)) = found {
            // Don't allow deleting the center (view root) or protected paths
            if depth == 0 || is_protected_path(&path) {
                return;
            }

            show_delete_dialog(
                &window_rclick,
                path,
                name,
                size,
                is_file,
                state_rclick.clone(),
                drawing_area_rclick.clone(),
            );
        }
    });
    drawing_area.add_controller(right_click_ctrl);

    // Directory chooser
    let state_choose = state.clone();
    let path_label_choose = path_label.clone();
    let window_choose = window.clone();
    choose_btn.connect_clicked(move |_| {
        let dialog = FileChooserDialog::new(
            Some("Choose Directory to Scan"),
            Some(&window_choose),
            FileChooserAction::SelectFolder,
            &[("Cancel", ResponseType::Cancel), ("Select", ResponseType::Accept)],
        );

        let state = state_choose.clone();
        let path_label = path_label_choose.clone();
        dialog.connect_response(move |dialog, response| {
            if response == ResponseType::Accept {
                if let Some(file) = dialog.file() {
                    if let Some(path) = file.path() {
                        state.borrow_mut().view_root = path.clone();
                        path_label.set_text(&path.to_string_lossy());
                    }
                }
            }
            dialog.close();
        });
        dialog.show();
    });

    // Up button
    let state_up = state.clone();
    let drawing_area_up = drawing_area.clone();
    let breadcrumb_box_up = breadcrumb_box.clone();
    let state_bc_up = state.clone();
    let drawing_area_bc_up = drawing_area.clone();
    let up_btn_bc_up = up_btn.clone();
    up_btn.connect_clicked(move |btn| {
        let mut state = state_up.borrow_mut();
        state.navigate_up();
        btn.set_sensitive(state.can_navigate_up());

        update_breadcrumbs(
            &breadcrumb_box_up,
            &state.get_breadcrumbs(),
            state_bc_up.clone(),
            drawing_area_bc_up.clone(),
            up_btn_bc_up.clone(),
        );

        drop(state);
        drawing_area_up.queue_draw();
    });

    // Scan button
    let state_scan = state.clone();
    let status_label_scan = status_label.clone();
    let drawing_area_scan = drawing_area.clone();
    let up_btn_scan = up_btn.clone();
    let scan_btn_scan = scan_btn.clone();
    let breadcrumb_box_scan = breadcrumb_box.clone();
    let state_bc_scan = state.clone();
    let drawing_area_bc_scan = drawing_area.clone();
    let up_btn_bc_scan = up_btn.clone();
    let progress_bar_scan = progress_bar.clone();
    scan_btn.connect_clicked(move |_| {
        let mut state = state_scan.borrow_mut();
        if state.scanning {
            return;
        }

        state.scanning = true;
        state.items_scanned = 0;
        let path = state.view_root.clone();
        drop(state);

        scan_btn_scan.set_sensitive(false);
        status_label_scan.set_text(&format!("Burning through {}...", path.display()));
        progress_bar_scan.set_visible(true);
        progress_bar_scan.set_fraction(0.0);
        progress_bar_scan.set_text(Some("Igniting..."));
        progress_bar_scan.set_show_text(true);

        let rx = scan_directory(path.clone());

        let state = state_scan.clone();
        let status_label = status_label_scan.clone();
        let drawing_area = drawing_area_scan.clone();
        let up_btn = up_btn_scan.clone();
        let scan_btn = scan_btn_scan.clone();
        let breadcrumb_box = breadcrumb_box_scan.clone();
        let state_bc = state_bc_scan.clone();
        let drawing_area_bc = drawing_area_bc_scan.clone();
        let up_btn_bc = up_btn_bc_scan.clone();
        let progress_bar = progress_bar_scan.clone();

        timeout_add_local(Duration::from_millis(50), move || {
            // Pulse progress bar to show activity
            progress_bar.pulse();

            while let Ok(progress) = rx.try_recv() {
                match progress {
                    ScanProgress::Scanning(_) => {
                        // Update less frequently to reduce UI overhead
                    }
                    ScanProgress::ItemCount(count) => {
                        state.borrow_mut().items_scanned = count;
                        status_label.set_text(&format!("Scorching... {} items consumed", count));
                        progress_bar.set_text(Some(&format!("{} items burned", count)));
                    }
                    ScanProgress::Complete(entry) => {
                        let mut state = state.borrow_mut();
                        let total_size = entry.total_size();
                        let item_count = entry.item_count();
                        state.view_root = entry.path.clone();
                        state.scan_root = Some(entry);
                        state.rebuild_segments();
                        state.scanning = false;

                        status_label.set_text(&format!(
                            "Scorched {} items - {} ablaze",
                            item_count,
                            format_size(total_size)
                        ));
                        progress_bar.set_visible(false);
                        scan_btn.set_sensitive(true);
                        up_btn.set_sensitive(state.can_navigate_up());

                        update_breadcrumbs(
                            &breadcrumb_box,
                            &state.get_breadcrumbs(),
                            state_bc.clone(),
                            drawing_area_bc.clone(),
                            up_btn_bc.clone(),
                        );

                        drop(state);
                        drawing_area.queue_draw();
                        return ControlFlow::Break;
                    }
                    ScanProgress::Error(e) => {
                        state.borrow_mut().scanning = false;
                        status_label.set_text(&format!("Error: {}", e));
                        progress_bar.set_visible(false);
                        scan_btn.set_sensitive(true);
                        return ControlFlow::Break;
                    }
                }
            }
            ControlFlow::Continue
        });
    });

    window.present();
}

fn update_breadcrumbs(
    container: &GtkBox,
    crumbs: &[(PathBuf, String)],
    state: Rc<RefCell<AppState>>,
    drawing_area: DrawingArea,
    up_btn: Button,
) {
    // Clear existing
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    for (i, (path, name)) in crumbs.iter().enumerate() {
        if i > 0 {
            let sep = Label::new(Some(" › "));
            sep.add_css_class("status-label");
            container.append(&sep);
        }

        let btn = Button::with_label(name);
        btn.add_css_class("breadcrumb");

        let path = path.clone();
        let state = state.clone();
        let drawing_area = drawing_area.clone();
        let up_btn = up_btn.clone();
        let container_clone = container.clone();
        btn.connect_clicked(move |_| {
            let mut s = state.borrow_mut();
            s.navigate_to(path.clone());
            up_btn.set_sensitive(s.can_navigate_up());

            let crumbs = s.get_breadcrumbs();
            drop(s);

            // Rebuild breadcrumbs (recursive but limited depth)
            update_breadcrumbs(&container_clone, &crumbs, state.clone(), drawing_area.clone(), up_btn.clone());
            drawing_area.queue_draw();
        });

        container.append(&btn);
    }
}

fn show_delete_dialog(
    window: &ApplicationWindow,
    path: PathBuf,
    name: String,
    size: u64,
    is_file: bool,
    state: Rc<RefCell<AppState>>,
    drawing_area: DrawingArea,
) {
    let message = format!(
        "INCINERATE {}?\n\nTarget: {}\nSize: {}\nType: {}\n\nThis will be reduced to ashes!",
        name,
        path.display(),
        format_size(size),
        if is_file { "File" } else { "Directory" }
    );

    let dialog = MessageDialog::new(
        Some(window),
        gtk4::DialogFlags::MODAL | gtk4::DialogFlags::DESTROY_WITH_PARENT,
        MessageType::Warning,
        ButtonsType::None,
        &message,
    );
    dialog.add_buttons(&[("Spare", ResponseType::Cancel), ("BURN IT", ResponseType::Accept)]);

    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            // Delete confirmed
            match delete_entry(&path) {
                DeleteResult::Success => {
                    // Update tree
                    let mut s = state.borrow_mut();
                    if let Some(root) = &mut s.scan_root {
                        remove_entry_from_tree(root, &path);
                    }
                    s.rebuild_segments();
                    drop(s);
                    drawing_area.queue_draw();
                }
                DeleteResult::ProtectedPath => {
                    eprintln!("Cannot delete protected path");
                }
                DeleteResult::PermissionDenied(e) => {
                    eprintln!("Permission denied: {}", e);
                }
                DeleteResult::Error(e) => {
                    eprintln!("Delete error: {}", e);
                }
                DeleteResult::NotFound => {
                    eprintln!("Path not found");
                }
            }
        }
        dialog.close();
    });

    dialog.show();
}

fn remove_entry_from_tree(entry: &mut crate::model::DirEntry, target: &PathBuf) -> bool {
    entry.children.retain(|child| &child.path != target);

    for child in &mut entry.children {
        if remove_entry_from_tree(child, target) {
            return true;
        }
    }

    // Recalculate size
    entry.size = entry.children.iter().map(|c| c.total_size()).sum();

    false
}
