mod actions;
mod app;
mod model;
mod scanner;
mod sunburst;
mod ui;

use gtk4::prelude::*;
use gtk4::Application;

fn main() {
    let app = Application::builder()
        .application_id("com.scorch.app")
        .build();

    app.connect_activate(|app| {
        ui::build_ui(app);
    });

    app.run();
}
