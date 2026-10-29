//! Kleine, wiederverwendbare UI-Bausteine im JoJa-CAD-Look.

use crate::icons;
use jjcad_core::model::{TextAlignH, TextAlignV};
use crate::theme::Palette;
use egui::{Shape, 
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
/// Ribbon-Schaltfläche mit kleinem Aufklapp-Pfeil (geteilte Schaltfläche).
/// Gibt (Hauptfläche, Pfeil) zurück — der Aufrufer öffnet das Menü.
/// Geteilte Schaltfläche: Hauptfläche (startet den zuletzt benutzten Modus) und
/// ein getrennter Pfeilstreifen rechts daneben (öffnet die Auswahl). Zwei
/// getrennte Rechtecke, damit ein Klick auf den Pfeil nie das Werkzeug startet.
pub fn ribbon_split_button(
    ui: &mut Ui,
    icon_name: &str,
    label: &str,
    active: bool,
) -> (Response, Response) {
    let h = 54.0;
    let main_w = 56.0;
    let arrow_w = 18.0;
    let (full, _) = ui.allocate_exact_size(Vec2::new(main_w + arrow_w, h), Sense::hover());
    let main_rect = Rect::from_min_size(full.min, Vec2::new(main_w, h));
    let arrow_rect = Rect::from_min_size(
        Pos2::new(full.min.x + main_w, full.min.y),
        Vec2::new(arrow_w, h),
    );
    let main = ui.interact(main_rect, ui.id().with(("split_main", label)), Sense::click());
    let arrow = ui.interact(arrow_rect, ui.id().with(("split_arrow", label)), Sense::click());

    if ui.is_rect_visible(full) {
        let p = ui.painter();
        if active {
            p.rect_filled(main_rect, Rounding::same(8.0), Palette::ACCENT_SOFT);
            p.rect_stroke(main_rect, Rounding::same(8.0), Stroke::new(1.0, Palette::ACCENT));
        } else if main.hovered() {
            p.rect_filled(main_rect, Rounding::same(8.0), Palette::PANEL_ALT);
        }
        if arrow.hovered() {
            p.rect_filled(arrow_rect, Rounding::same(4.0), Palette::PANEL_ALT);
        }
        let color = if active { Palette::ACCENT } else { Palette::TEXT };
        let ir = Rect::from_center_size(
            Pos2::new(main_rect.center().x, main_rect.top() + 19.0),
            Vec2::splat(22.0),
        );
        icons::paint(p, ir, icon_name, color);
        p.text(
            Pos2::new(main_rect.center().x, main_rect.bottom() - 11.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.5),
            if active { Palette::ACCENT } else { Palette::TEXT_DIM },
        );
        let c = arrow_rect.center();
        p.add(Shape::convex_polygon(
            vec![
                Pos2::new(c.x - 4.0, c.y - 2.0),
                Pos2::new(c.x + 4.0, c.y - 2.0),
                Pos2::new(c.x, c.y + 3.0),
            ],
            if arrow.hovered() { Palette::TEXT } else { Palette::TEXT_FAINT },
            Stroke::NONE,
        ));
    }
    (main, arrow)
}

/// Kompaktes Kontrollkästchen für Raster im Ribbon (feste Zellenbreite).
///
/// Alle Zellen sind gleich breit, damit die Spalten untereinander stehen und
/// die Gruppe mit zwei Zeilen genau eine Knopfreihe (54 px) hoch wird —
/// Voraussetzung für die einheitliche Ribbon-Höhe (t_92b03440).
pub fn check_cell(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    ui.add_sized([88.0, 24.0], egui::Checkbox::new(value, label))
}

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

/// Zeile wie [`field_row`], aber mit eigener Höhe für die Wertspalte.
///
/// Für Bausteine, die höher als eine Tabellenzeile sind (3×3-Matrix der
/// Textausrichtung). Der Name steht oben in der Namensspalte, der Block beginnt
/// an derselben linken Kante wie alle Eingabefelder.
pub fn field_block(ui: &mut Ui, label: &str, hoehe: f32, add: impl FnOnce(&mut Ui)) {
    let idx = ROW_IDX.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    });
    let w = ui.available_width();
    let (row, _) = ui.allocate_exact_size(Vec2::new(w, hoehe), Sense::hover());
    if idx % 2 == 1 {
        ui.painter().rect_filled(
            Rect::from_min_size(egui::pos2(row.left(), row.top()), Vec2::new(w, hoehe)),
            Rounding::same(3.0),
            Palette::PANEL_ALT.gamma_multiply(0.55),
        );
    }
    ui.painter().text(
        egui::pos2(row.left() + 4.0, row.top() + ROW_H * 0.5),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        Palette::TEXT_DIM,
    );
    let cell = Rect::from_min_size(
        egui::pos2(row.left() + LABEL_W, row.top()),
        Vec2::new(VAL_W.min(w - LABEL_W - 2.0), hoehe),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(cell)
            .layout(egui::Layout::left_to_right(egui::Align::TOP)),
        |ui| {
            ui.set_max_width(cell.width());
            add(ui);
        },
    );
}

/// Kantenlänge einer Zelle der Ausrichtungsmatrix (px).
pub const ALIGN_ZELLE: f32 = 30.0;
/// Höhe der ganzen Matrix: drei Zellen und zwei Abstände.
pub const ALIGN_HOEHE: f32 = ALIGN_ZELLE * 3.0 + 2.0 * ALIGN_ABSTAND;
const ALIGN_ABSTAND: f32 = 3.0;

/// 3×3-Matrix der Textausrichtung.
///
/// Spalten = waagerecht (Linksbündig / Zentriert / Rechtsbündig), Zeilen =
/// senkrecht (Oben / Mitte / Unten). Jede Zelle zeigt einen Pfeil in ihre
/// Richtung (Ecken und Kanten als Pfeil, die Mitte als Quadrat — wie in der
/// Prinzipskizze des Tickets) und nennt im Tooltip beide Werte. `true`, wenn
/// der Nutzer eine andere Zelle gewählt hat.
pub fn align_matrix(
    ui: &mut Ui,
    salt: &str,
    h: &mut TextAlignH,
    v: &mut TextAlignV,
) -> bool {
    let (bereich, _) = ui.allocate_exact_size(
        Vec2::new(
            ALIGN_ZELLE * 3.0 + 2.0 * ALIGN_ABSTAND,
            ALIGN_HOEHE,
        ),
        Sense::hover(),
    );
    let mut geaendert = false;
    for (zeile, wert_v) in TextAlignV::ALL.iter().enumerate() {
        for (spalte, wert_h) in TextAlignH::ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(
                    bereich.left() + spalte as f32 * (ALIGN_ZELLE + ALIGN_ABSTAND),
                    bereich.top() + zeile as f32 * (ALIGN_ZELLE + ALIGN_ABSTAND),
                ),
                Vec2::splat(ALIGN_ZELLE),
            );
            let resp = ui.interact(r, ui.id().with((salt, zeile, spalte)), Sense::click());
            let aktiv = *h == *wert_h && *v == *wert_v;
            if ui.is_rect_visible(r) {
                let p = ui.painter();
                let (fuellung, rand, farbe) = if aktiv {
                    (Palette::ACCENT_SOFT, Palette::ACCENT, Palette::ACCENT)
                } else if resp.hovered() {
                    (Palette::PANEL_ALT, Palette::ACCENT.gamma_multiply(0.7), Palette::TEXT)
                } else {
                    (Color32::TRANSPARENT, Palette::BORDER, Palette::TEXT_DIM)
                };
                p.rect_filled(r, Rounding::same(5.0), fuellung);
                p.rect_stroke(r, Rounding::same(5.0), Stroke::new(1.0, rand));
                align_pfeil(p, r.shrink(6.0), *wert_h, *wert_v, farbe);
            }
            let tip = format!("{} · {}", wert_h.label(), wert_v.label());
            if resp.on_hover_text(tip).clicked() {
                *h = *wert_h;
                *v = *wert_v;
                geaendert = true;
            }
        }
    }
    geaendert
}

/// Pfeil der Ausrichtungsmatrix: zeigt in die Richtung der gewählten Zelle.
///
/// Ecken: Pfeil zur Ecke plus Winkelklammer an der Ecke (wie in der Skizze),
/// Kanten: Pfeil an die Kante, Mitte: kleines Quadrat.
fn align_pfeil(p: &egui::Painter, r: Rect, h: TextAlignH, v: TextAlignV, col: Color32) {
    let c = r.center();
    let (hx, hy) = (r.width() * 0.5, r.height() * 0.5);
    let dx = match h {
        TextAlignH::Left => -1.0,
        TextAlignH::Center => 0.0,
        TextAlignH::Right => 1.0,
    };
    let dy = match v {
        TextAlignV::Top => -1.0,
        TextAlignV::Middle => 0.0,
        TextAlignV::Bottom => 1.0,
    };
    let st = Stroke::new(1.6, col);
    if dx == 0.0 && dy == 0.0 {
        // Mitte: Quadrat (der Einfügepunkt sitzt in der Textmitte)
        p.rect_stroke(
            Rect::from_center_size(c, Vec2::new(hx * 1.15, hy * 1.15)),
            Rounding::same(1.0),
            st,
        );
        return;
    }
    // Ziel: Kante bzw. Ecke des Symbols
    let ziel = Pos2::new(c.x + dx * hx * 0.92, c.y + dy * hy * 0.92);
    // Winkelklammer an der Ecke — macht sie auch ohne Pfeil lesbar
    if dx != 0.0 && dy != 0.0 {
        let ecke = Pos2::new(c.x + dx * hx, c.y + dy * hy);
        p.line_segment(
            [Pos2::new(c.x + dx * hx * 0.35, ecke.y), ecke],
            st,
        );
        p.line_segment(
            [ecke, Pos2::new(ecke.x, c.y + dy * hy * 0.35)],
            st,
        );
    }
    let start = Pos2::new(c.x - dx * hx * 0.85, c.y - dy * hy * 0.85);
    p.line_segment([start, ziel], st);
    // Spitze: Dreieck in Richtung (dx, dy)
    let l = Vec2::new(dx, dy).normalized();
    let n = Vec2::new(-l.y, l.x);
    let sp = 4.2;
    p.add(Shape::convex_polygon(
        vec![ziel + l * 1.2, ziel - l * sp + n * sp * 0.65, ziel - l * sp - n * sp * 0.65],
        col,
        Stroke::NONE,
    ));
}

/// Zeile mit festem Namen und nicht editierbarem Wert.
///
/// Gleiche Spalten wie `field_row` (96 px Name, Wert ab derselben Kante), damit
/// Anzeige- und Eingabezeilen im Eigenschaften-Panel genau untereinander liegen.
/// Lange Werte werden gekürzt; der volle Text steht im Tooltip (`tip`, sonst der
/// Wert selbst).
pub fn info_row(ui: &mut Ui, label: &str, value: &str, tip: Option<&str>) -> Response {
    let mut resp = None;
    field_row(ui, label, |ui| {
        resp = Some(ui.add(
            egui::Label::new(egui::RichText::new(value).size(11.5).color(Palette::TEXT)).truncate(),
        ));
    });
    let resp = resp.expect("field_row ruft den Zeileninhalt genau einmal auf");
    resp.on_hover_text(tip.unwrap_or(value))
}

/// Textfeld mit Einheitensuffix, für Längen in Dokumenteinheit.
pub fn length_input(ui: &mut Ui, value_mm: &mut f64, unit: jjcad_core::units::Unit, width: f32) -> bool {
    length_input_limited(ui, value_mm, unit, width, None)
}

/// Längenfeld mit Grenzen (in mm), für Werte, bei denen negative oder beliebig
/// große Eingaben sinnlos sind — etwa die Rasterweite: ohne Grenzen zog der
/// Ziehpfeil ins Negative und in Werte, die kein Raster mehr ergeben.
/// Die Grenzen werden in die Dokumenteinheit umgerechnet; der Ziehpfeil kann
/// den Wert nicht mehr aus dem Bereich schieben, auch nicht beim Tippen.
pub fn length_input_between(
    ui: &mut Ui,
    value_mm: &mut f64,
    unit: jjcad_core::units::Unit,
    width: f32,
    min_mm: f64,
    max_mm: f64,
) -> bool {
    length_input_limited(ui, value_mm, unit, width, Some((min_mm, max_mm)))
}

fn length_input_limited(
    ui: &mut Ui,
    value_mm: &mut f64,
    unit: jjcad_core::units::Unit,
    width: f32,
    limits_mm: Option<(f64, f64)>,
) -> bool {
    let mut v = unit.from_mm(*value_mm);
    let decimals = unit.decimals();
    // Anzeige fest an der Einheit ausrichten statt an egui's automatischer
    // Stellenzahl (die hängt an aim_radius/speed) und statt an min_decimals(0):
    // sonst stand für 0,24 m nur „0 m“ im Feld. format_length_auto zeigt
    // 0,24 m / 2,75 m / 240 mm und keine Tausenderpunkte (eindeutig lesbar).
    let mut drag = egui::DragValue::new(&mut v)
        .speed(unit.mm() * 0.5)
        .min_decimals(decimals)
        .max_decimals(decimals + 2)
        .custom_parser(flexible_number)
        .custom_formatter(move |n, _| unit.format_length_auto(unit.to_mm(n)))
        .suffix(format!(" {}", unit.short()));
    if let Some((min_mm, max_mm)) = limits_mm {
        drag = drag.range(unit.from_mm(min_mm)..=unit.from_mm(max_mm));
    }
    let resp = ui.add_sized([width, 20.0], drag);
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

    /// Grenzen beim Ziehen: ohne `.range` zog das Rasterfeld ins Negative und in
    /// beliebig große Werte (T0011). Geprüft am echten Widget im egui-Kontext.
    #[test]
    fn rasterfeld_zieht_nicht_aus_den_grenzen() {
        let feld = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(200.0, 20.0));
        let mitte = feld.center();

        let ziehen = |dx: f32| -> f64 {
            let ctx = egui::Context::default();
            let mut wert = 100.0; // mm (Vorgabe)
            let mut frames: Vec<Vec<egui::Event>> = vec![
                Vec::new(),
                vec![egui::Event::PointerMoved(mitte)],
                vec![egui::Event::PointerButton {
                    pos: mitte,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
                vec![egui::Event::PointerMoved(egui::pos2(mitte.x + dx, mitte.y))],
                vec![egui::Event::PointerButton {
                    pos: egui::pos2(mitte.x + dx, mitte.y),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            ];
            for events in frames.drain(..) {
                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 60.0))),
                    events,
                    ..Default::default()
                };
                let _ = ctx.run(raw, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(feld)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            |ui| {
                                length_input_between(
                                    ui,
                                    &mut wert,
                                    jjcad_core::units::Unit::M,
                                    200.0,
                                    jjcad_core::model::GRID_STEP_MIN_MM,
                                    jjcad_core::model::GRID_STEP_MAX_MM,
                                );
                            },
                        );
                    });
                });
            }
            wert
        };

        // Nach links weit über die Untergrenze hinaus: bleibt bei 1 mm.
        assert_eq!(ziehen(-2000.0), jjcad_core::model::GRID_STEP_MIN_MM);
        // Nach rechts weit über die Obergrenze hinaus: bleibt bei 10 m.
        assert_eq!(ziehen(2000.0), jjcad_core::model::GRID_STEP_MAX_MM);
    }
}
