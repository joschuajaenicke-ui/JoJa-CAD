//! JoJa-CAD — Desktop-Anwendung.
//!
//! 2D-CAD für Architektur und Bauantragszeichnungen.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod chat;
mod diag;
mod dialogs;
mod ghost;
mod hotbar;
mod handles;
mod icon;
mod icons;
mod inventory;
mod panels;
mod perf;
mod props;
mod systemdialog;
mod textedit;
mod theme;
mod widgets;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file = args.get(1).filter(|a| !a.starts_with("--")).cloned();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 950.0])
            .with_min_inner_size([1080.0, 680.0])
            .with_title(app::window_title())
            // Fenstername für Taskleiste und Startmenü (KDE/GNOME ordnen das
            // Fenster über den .desktop-Eintrag „jjcad“ zu)
            .with_app_id("jjcad")
            .with_icon(icon::fenster_icon()),
        vsync: crate::diag::vsync(),
        ..Default::default()
    };

    eframe::run_native(
        "JoJa-CAD",
        options,
        Box::new(move |cc| {
            let mut app = app::App::new(cc);
            if let Some(f) = file.clone() {
                app.load_document(&std::path::PathBuf::from(f));
            }
            // Startzustand für Screenshots/automatisierte Tests (JJCAD_DEMO_UI)
            app.apply_demo_ui();
            Ok(Box::new(app))
        }),
    )
}
