//! JoJa-CAD — Desktop-Anwendung.
//!
//! 2D-CAD für Architektur und Bauantragszeichnungen.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod dialogs;
mod ghost;
mod hotbar;
mod handles;
mod icons;
mod inventory;
mod panels;
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
            .with_icon(load_icon()),
        vsync: true,
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

/// Kleines Fenstericon (32×32, gezeichnet).
fn load_icon() -> egui::IconData {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let border = x < 2 || y < 2 || x >= size - 2 || y >= size - 2;
            let bar = (6..26).contains(&x) && ((9..12).contains(&y) || (15..18).contains(&y) || (21..24).contains(&y));
            if (border || bar) && x > 3 && x < 29 && y > 3 && y < 29 {
                rgba[i] = 0xE3;
                rgba[i + 1] = 0xA2;
                rgba[i + 2] = 0x3D;
                rgba[i + 3] = 0xFF;
            }
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}
