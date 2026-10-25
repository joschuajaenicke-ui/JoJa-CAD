//! Kleine, wiederverwendbare UI-Bausteine im JoJa-CAD-Look.

use crate::icons;
use crate::theme::Palette;
use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2,
};

pub fn icon(ui: &mut Ui, rect: Rect, name: &str, color: Color32) {
    icons::paint(ui.painter(), rect, name, color);
}

/// Runder Icon-Knopf (Werkzeugleiste, Panels, Fenster).
pub fn icon_button(
    ui: &mut Ui,
    icon_name: &str,
    size: f32,
    active: bool,
    tooltip: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hovered = resp.hovered();
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if active {
            p.rect_filled(rect, Rounding::same(7.0), Palette::ACCENT_SOFT);
            p.rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0, Palette::ACCENT));
        } else if hovered {
            p.rect_filled(rect, Rounding::same(7.0), Palette::PANEL_ALT);
        }
        let color = if active {
            Palette::ACCENT
        } else if hovered {
            Palette::TEXT
        } else {
            Palette::TEXT_DIM
        };
        icons::paint(p, rect.shrink(size * 0.24), icon_name, color);
    }
    resp.on_hover_text(tooltip)
}

/// Großer Ribbon-Knopf mit Icon und Beschriftung.
pub fn ribbon_button(ui: &mut Ui, icon_name: &str, label: &str, active: bool) -> Response {
    let w = 70.0;
    let h = 54.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hovered = resp.hovered();
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if active {
            p.rect_filled(rect, Rounding::same(8.0), Palette::ACCENT_SOFT);
            p.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0, Palette::ACCENT));
        } else if hovered {
            p.rect_filled(rect, Rounding::same(8.0), Palette::PANEL_ALT);
        }
        let color = if active { Palette::ACCENT } else { Palette::TEXT };
        let ir = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 19.0),
            Vec2::splat(21.0),
        );
        icons::paint(p, ir, icon_name, color);
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 10.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.5),
            if active { Palette::ACCENT } else { Palette::TEXT_DIM },
        );
    }
    resp
}

/// Item-Slot (Hotbar/Inventar/Tastatur).
pub fn slot_button(
    ui: &mut Ui,
    size: Vec2,
    action: Option<&str>,
    key_label: Option<&str>,
    active: bool,
    highlight: bool,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let bg = if highlight {
            Palette::ACCENT_SOFT
        } else if resp.hovered() {
            Palette::PANEL_ALT
        } else {
            Palette::PANEL
        };
        p.rect_filled(rect, Rounding::same(9.0), bg);
        let border = if active {
            Stroke::new(2.0, Palette::ACCENT)
        } else if highlight {
            Stroke::new(1.0, Palette::ACCENT.gamma_multiply(0.7))
        } else if resp.hovered() {
            Stroke::new(1.0, Palette::BORDER)
        } else {
            Stroke::new(1.0, Palette::BORDER_SOFT)
        };
        p.rect_stroke(rect, Rounding::same(9.0), border);
        if let Some(a) = action {
            let icon_name = jjcad_core::actions::action(a)
                .map(|d| d.icon)
                .unwrap_or("block");
            let ir = Rect::from_center_size(
                rect.center() - Vec2::new(0.0, 2.0),
                Vec2::splat(size.x * 0.48),
            );
            icons::paint(p, ir, icon_name, if active { Palette::ACCENT } else { Palette::TEXT });
            if let Some(k) = key_label {
                p.text(
                    Pos2::new(rect.right() - 5.0, rect.bottom() - 4.0),
                    Align2::RIGHT_BOTTOM,
                    k,
                    FontId::proportional(9.5),
                    Palette::TEXT_FAINT,
                );
            }
        } else {
            // leerer Slot: dezentes Plus
            icons::paint(
                p,
                Rect::from_center_size(rect.center(), Vec2::splat(size.x * 0.24)),
                "plus",
                Palette::TEXT_FAINT.gamma_multiply(0.6),
            );
        }
    }
    resp
}

/// Farbpunkt mit Rahmen.
pub fn color_dot(ui: &mut Ui, color: Color32, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.circle_filled(rect.center(), size * 0.42, color);
        p.circle_stroke(
            rect.center(),
            size * 0.42,
            Stroke::new(1.0, Color32::from_black_alpha(90)),
        );
    }
    resp
}

/// Beschriftete Zeile in Panels.
thread_local! {
    /// Zeilenzähler für die Zebra-Streifen der Eigenschaftentabelle
    static ROW_IDX: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Setzt den Zeilenzähler zurück (vor jedem Panel neu).
pub fn reset_rows() {
    ROW_IDX.with(|c| c.set(0));
}

/// Tabellenzeile: feste Namensspalte, Wert rechts daneben, jede zweite Zeile leicht hinterlegt.
pub const LABEL_W: f32 = 96.0;
/// Breite der Wertspalte — alle Eingabefelder liegen damit exakt untereinander.
pub const VAL_W: f32 = 150.0;
/// Höhe einer Zeile
pub const ROW_H: f32 = 22.0;

pub fn field_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let idx = ROW_IDX.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    });
    let w = ui.available_width();
    let (row, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
    if idx % 2 == 1 {
        ui.painter().rect_filled(
            Rect::from_min_size(egui::pos2(row.left(), row.top()), Vec2::new(w, ROW_H)),
            Rounding::same(3.0),
            Palette::PANEL_ALT.gamma_multiply(0.55),
        );
    }
    // Name: exakt links, einzeilig, notfalls gekürzt
    let mut text = label.to_string();
    if text.chars().count() > 16 {
        text = format!("{}…", text.chars().take(15).collect::<String>());
    }
    ui.painter().text(
        egui::pos2(row.left() + 4.0, row.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(11.5),
        Palette::TEXT_DIM,
    );
    // Wert: eigener Bereich mit fester linker Kante → alle Felder liegen exakt untereinander
    let cell = Rect::from_min_size(
        egui::pos2(row.left() + LABEL_W, row.center().y - ROW_H * 0.5),
        Vec2::new(VAL_W.min(w - LABEL_W - 2.0), ROW_H),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(cell)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_max_width(cell.width());
            add(ui);
        },
    );
    let _ = ui;
}

/// Textfeld mit Einheitensuffix, für Längen in Dokumenteinheit.
pub fn length_input(ui: &mut Ui, value_mm: &mut f64, unit: jjcad_core::units::Unit, width: f32) -> bool {
    let mut v = unit.from_mm(*value_mm);
    let decimals = unit.decimals();
    // Anzeige fest an der Einheit ausrichten statt an egui's automatischer
    // Stellenzahl (die hängt an aim_radius/speed) und statt an min_decimals(0):
    // sonst stand für 0,24 m nur „0 m“ im Feld. format_length_auto zeigt
    // 0,24 m / 2,75 m / 240 mm und keine Tausenderpunkte (eindeutig lesbar).
    let resp = ui.add_sized(
        [width, 20.0],
        egui::DragValue::new(&mut v)
            .speed(unit.mm() * 0.5)
            .min_decimals(decimals)
            .max_decimals(decimals + 2)
            .custom_parser(flexible_number)
            .custom_formatter(move |n, _| unit.format_length_auto(unit.to_mm(n)))
            .suffix(format!(" {}", unit.short())),
    );
    if resp.changed() {
        *value_mm = unit.to_mm(v);
        true
    } else {
        false
    }
}

/// Trennlinie in der Horizontalen.
pub fn hline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::ZERO, Palette::BORDER_SOFT);
}

/// Vertikaler Trenner für Ribbon-Gruppen.
pub fn vline(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, height), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_center_size(rect.center(), Vec2::new(1.0, height * 0.7)),
        Rounding::ZERO,
        Palette::BORDER_SOFT,
    );
}

pub fn window_frame() -> egui::Frame {
    crate::theme::dialog_frame()
}

// ---------------------------------------------------------------------------
// Zahlenfelder: Komma und Punkt als Dezimaltrennzeichen, Anzeige mit Komma
// ---------------------------------------------------------------------------

/// Akzeptiert "0,35" und "0.35".
pub fn flexible_number(s: &str) -> Option<f64> {
    let t = s.trim().replace(' ', "").replace(',', ".");
    t.parse::<f64>().ok()
}

/// DragValue mit deutschem Zahlenformat (komma) und flexibler Eingabe.
pub fn dv<'a, T: egui::emath::Numeric>(v: &'a mut T) -> egui::DragValue<'a> {
    egui::DragValue::new(v)
        .custom_parser(flexible_number)
        .custom_formatter(|n, decimals| {
            let d = *decimals.start();
            format!("{:.*}", d, n).replace('.', ",")
        })
}

#[cfg(test)]
mod zahltests {
    use super::*;

    #[test]
    fn zahlenfeld_akzeptiert_komma_und_punkt() {
        assert_eq!(flexible_number("0,35"), Some(0.35));
        assert_eq!(flexible_number("0.35"), Some(0.35));
        assert_eq!(flexible_number(" 1,5 "), Some(1.5));
        assert_eq!(flexible_number("2"), Some(2.0));
        assert_eq!(flexible_number("abc"), None);
    }
}
