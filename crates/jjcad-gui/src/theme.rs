//! Farben, Schriften und Grundstile der Benutzeroberfläche.

use egui::{Color32, FontFamily, FontId, Frame, Margin, Rounding, Stroke, TextStyle, Visuals};


pub struct Palette;

impl Palette {
    pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
    pub const PANEL: Color32 = Color32::from_rgb(0x1A, 0x1D, 0x22);
    pub const PANEL_ALT: Color32 = Color32::from_rgb(0x21, 0x25, 0x2B);
    pub const RIBBON: Color32 = Color32::from_rgb(0x17, 0x1A, 0x1F);
    pub const BORDER: Color32 = Color32::from_rgb(0x2C, 0x32, 0x3A);
    pub const BORDER_SOFT: Color32 = Color32::from_rgb(0x24, 0x29, 0x30);
    pub const TEXT: Color32 = Color32::from_rgb(0xE7, 0xE9, 0xEC);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(0x99, 0xA2, 0xAD);
    pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6C, 0x75, 0x81);
    pub const ACCENT: Color32 = Color32::from_rgb(0xE3, 0xA2, 0x3D);
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0x3A, 0x2E, 0x1B);
    pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x1A, 0x15, 0x0C);
    pub const DANGER: Color32 = Color32::from_rgb(0xD8, 0x60, 0x4F);
    pub const OK: Color32 = Color32::from_rgb(0x5A, 0xB5, 0x82);
    pub const PAPER: Color32 = Color32::from_rgb(0xFB, 0xFA, 0xF6);
    pub const PAPER_SHADOW: Color32 = Color32::from_rgb(0x0A, 0x0B, 0x0D);
    pub const SELECT: Color32 = Color32::from_rgb(0x2F, 0x8C, 0xF0);
    pub const HOVER: Color32 = Color32::from_rgb(0x59, 0xA8, 0xFF);
    /// Hilfslinien des erweiterten Punktfangs: eigener Ton, damit sie weder mit
    /// der Auswahl (blau) noch mit dem Fangmarker (Akzent) verwechselt werden.
    pub const TRACK: Color32 = Color32::from_rgb(0x24, 0xB6, 0xA0);
    pub const GRID_MINOR: Color32 = Color32::from_rgb(0xE9, 0xE7, 0xE0);
    pub const GRID_MAJOR: Color32 = Color32::from_rgb(0xD8, 0xD5, 0xCB);
}

pub fn paper_frame() -> Frame {
    Frame::none().fill(Palette::PANEL).inner_margin(Margin::same(0.0))
}

pub fn card() -> Frame {
    Frame::none()
        .fill(Palette::PANEL_ALT)
        .rounding(Rounding::same(8.0))
        .stroke(Stroke::new(1.0, Palette::BORDER_SOFT))
        .inner_margin(Margin::symmetric(10.0, 8.0))
}

pub fn dialog_frame() -> Frame {
    Frame::none()
        .fill(Palette::PANEL)
        .rounding(Rounding::same(12.0))
        .stroke(Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(18.0))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 12.0),
            blur: 32.0,
            spread: 0.0,
            color: Color32::from_black_alpha(140),
        })
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Systemschrift laden (falls vorhanden), sonst egui-Standardschrift
    for path in [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("app".to_owned(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "app".to_owned());
            break;
        }
    }
    for path in [
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
        "C:\\Windows\\Fonts\\arialbd.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("app-bold".to_owned(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Name("bold".into()))
                .or_default()
                .push("app-bold".to_owned());
            break;
        }
    }
    // Schriften für Texte der Zeichnung: je Familie ein eigener Satz, damit
    // „Serifen", „Schmal" und „Rund" auch auf der Zeichenfläche so aussehen.
    // Die Daten kommen aus dem Kern (Systemschriften, nichts eingebettet).
    for font in jjcad_core::model::TextFont::ALL {
        for (stil, anhang) in [
            (jjcad_core::fonts::FontStyle::Regular, ""),
            (jjcad_core::fonts::FontStyle::Bold, "_fett"),
        ] {
            let Some(bytes) = jjcad_core::fonts::ttf_bytes(font, stil) else {
                continue;
            };
            let name = format!("schrift_{}{}", font.key(), anhang);
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes.to_vec()).into());
            fonts
                .families
                .entry(egui::FontFamily::Name(name.clone().into()))
                .or_default()
                .push(name);
        }
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(19.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();

    let mut v = Visuals::dark();
    v.override_text_color = Some(Palette::TEXT);
    v.panel_fill = Palette::PANEL;
    v.window_fill = Palette::PANEL;
    v.extreme_bg_color = Palette::BG;
    v.faint_bg_color = Palette::PANEL_ALT;
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(8.0);
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 30.0,
        spread: 0.0,
        color: Color32::from_black_alpha(150),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_black_alpha(120),
    };
    v.window_stroke = Stroke::new(1.0, Palette::BORDER);
    v.selection.bg_fill = Palette::ACCENT.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, Palette::ACCENT);
    v.widgets.noninteractive.bg_fill = Palette::PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Palette::BORDER_SOFT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Palette::TEXT_DIM);
    v.widgets.inactive.bg_fill = Palette::PANEL_ALT;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::BORDER_SOFT);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::TEXT);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2B, 0x31, 0x39);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Palette::TEXT);
    v.widgets.active.bg_fill = Palette::ACCENT_SOFT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, Palette::ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Palette::ACCENT);
    v.widgets.open.bg_fill = Palette::PANEL_ALT;
    v.widgets.open.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    v.button_frame = true;
    v.striped = false;
    v.slider_trailing_fill = true;
    // Einheitliche Feld-Optik (Dropdowns, Buttons): flach, dünne Kontur, gleiche Rundung
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(4.0);
        w.bg_fill = Palette::BG;
        w.weak_bg_fill = Palette::BG;
    }
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::PANEL_ALT.gamma_multiply(1.6));
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::TEXT);
    v.widgets.hovered.bg_fill = Palette::PANEL_ALT;
    v.widgets.hovered.weak_bg_fill = Palette::PANEL_ALT;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::ACCENT);
    v.widgets.open.bg_fill = Palette::PANEL_ALT;
    v.widgets.open.weak_bg_fill = Palette::PANEL_ALT;
    v.widgets.open.bg_stroke = Stroke::new(1.0, Palette::ACCENT);
    v.widgets.active.bg_fill = Palette::PANEL_ALT;
    v.widgets.active.weak_bg_fill = Palette::PANEL_ALT;

    style.visuals = v;

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(28.0, 24.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.animation_time = 0.10;
    ctx.set_style(style);
}

/// Kleine Überschrift in Panels.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.5)
            .color(Palette::TEXT_FAINT)
            .strong(),
    );
    ui.add_space(2.0);
}

pub fn bold(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(size)
        .family(FontFamily::Name("bold".into()))
}

pub fn dim(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(11.5)
        .color(Palette::TEXT_DIM)
}

pub fn faint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(11.0)
        .color(Palette::TEXT_FAINT)
}

pub fn mono(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .monospace()
        .size(11.5)
        .color(Palette::TEXT_DIM)
}

/// Einheitlicher "Chip"/Badge.
/// Wie [`chip`], aber mit fester Widget-ID — nötig für Kontextmenüs, weil sich
/// automatische IDs verschieben, sobald sich die Anzahl der Widgets davor ändert.
pub fn chip_id(ui: &mut egui::Ui, id: &str, text: &str, active: bool) -> egui::Response {
    ui.push_id(id, |ui| chip(ui, text, active)).inner
}

pub fn chip(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let (bg, fg, stroke) = if active {
        (Palette::ACCENT_SOFT, Palette::ACCENT, Stroke::new(1.0, Palette::ACCENT))
    } else {
        (Palette::PANEL_ALT, Palette::TEXT_DIM, Stroke::new(1.0, Palette::BORDER_SOFT))
    };
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(11.5),
        fg,
    );
    let size = egui::vec2(galley.size().x + 16.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(11.0), bg);
        p.rect_stroke(rect, Rounding::same(11.0), stroke);
        p.galley(
            rect.center() - galley.size() / 2.0,
            galley,
            fg,
        );
    }
    resp
}
