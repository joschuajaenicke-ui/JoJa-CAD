//! Kleine, wiederverwendbare UI-Bausteine im JoJa-CAD-Look.

use crate::icons;
use jjcad_core::model::{BlockDef, Document, TextAlignH, TextAlignV, TextFont};
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
            p.rect_filled(rect, Rounding::same(7.0), Palette::accent_soft());
            p.rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0, Palette::accent()));
        } else if hovered {
            p.rect_filled(rect, Rounding::same(7.0), Palette::panel_alt());
        }
        let color = if active {
            Palette::accent()
        } else if hovered {
            Palette::text()
        } else {
            Palette::text_dim()
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
            p.rect_filled(main_rect, Rounding::same(8.0), Palette::accent_soft());
            p.rect_stroke(main_rect, Rounding::same(8.0), Stroke::new(1.0, Palette::accent()));
        } else if main.hovered() {
            p.rect_filled(main_rect, Rounding::same(8.0), Palette::panel_alt());
        }
        if arrow.hovered() {
            p.rect_filled(arrow_rect, Rounding::same(4.0), Palette::panel_alt());
        }
        let color = if active { Palette::accent() } else { Palette::text() };
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
            if active { Palette::accent() } else { Palette::text_dim() },
        );
        let c = arrow_rect.center();
        p.add(Shape::convex_polygon(
            vec![
                Pos2::new(c.x - 4.0, c.y - 2.0),
                Pos2::new(c.x + 4.0, c.y - 2.0),
                Pos2::new(c.x, c.y + 3.0),
            ],
            if arrow.hovered() { Palette::text() } else { Palette::text_faint() },
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

/// Schriftgröße der Ribbon-Beschriftung im Regelfall.
pub const RIBBON_TEXT: f32 = 10.5;

/// Schriftgröße, mit der eine Beschriftung in `verfuegbar` Punkte passt.
///
/// `gemessen` ist die Breite bei `vorgabe` Punkten, gemessen vom Painter (nicht
/// geschätzt: die Zeichenbreiten hängen an der Schrift). Unter 8 Punkte wird
/// nicht verkleinert — darunter ist die Beschriftung nicht mehr lesbar, dann ist
/// ein längerer Name die falsche Wahl.
pub fn passende_schriftgroesse(gemessen: f32, verfuegbar: f32, vorgabe: f32) -> f32 {
    if gemessen <= verfuegbar || gemessen <= 0.0 {
        return vorgabe;
    }
    (vorgabe * verfuegbar / gemessen).max(8.0)
}

pub fn ribbon_button(ui: &mut Ui, icon_name: &str, label: &str, active: bool) -> Response {
    let w = 70.0;
    let h = 54.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hovered = resp.hovered();
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if active {
            p.rect_filled(rect, Rounding::same(8.0), Palette::accent_soft());
            p.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0, Palette::accent()));
        } else if hovered {
            p.rect_filled(rect, Rounding::same(8.0), Palette::panel_alt());
        }
        let color = if active { Palette::accent() } else { Palette::text() };
        let ir = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 19.0),
            Vec2::splat(21.0),
        );
        icons::paint(p, ir, icon_name, color);
        // Lange Beschriftungen („Schnellauswahl") liefen sonst über die
        // Schaltfläche hinaus in die Nachbargruppe — die Schrift wird deshalb
        // so weit verkleinert, wie es der Platz verlangt (T0060).
        let textfarbe = if active { Palette::accent() } else { Palette::text_dim() };
        let gemessen = p
            .layout_no_wrap(label.to_string(), FontId::proportional(RIBBON_TEXT), color)
            .size()
            .x;
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 10.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(passende_schriftgroesse(gemessen, w - 6.0, RIBBON_TEXT)),
            textfarbe,
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
    block: Option<(&Document, &BlockDef)>,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let bg = if highlight {
            Palette::accent_soft()
        } else if resp.hovered() {
            Palette::panel_alt()
        } else {
            Palette::panel()
        };
        p.rect_filled(rect, Rounding::same(9.0), bg);
        let border = if active {
            Stroke::new(2.0, Palette::accent())
        } else if highlight {
            Stroke::new(1.0, Palette::accent().gamma_multiply(0.7))
        } else if resp.hovered() {
            Stroke::new(1.0, Palette::border())
        } else {
            Stroke::new(1.0, Palette::border_soft())
        };
        p.rect_stroke(rect, Rounding::same(9.0), border);
        if let Some(a) = action {
            let farbe = if active { Palette::accent() } else { Palette::text() };
            let ir = Rect::from_center_size(
                rect.center() - Vec2::new(0.0, 2.0),
                Vec2::splat(size.x * 0.48),
            );
            // T0116: Ein Block zeigt seine Miniaturaansicht — dieselbe wie im
            // Blockmanager — statt eines gemeinsamen Ersatzsymbols.
            if let Some((doc, b)) = block {
                crate::inventory::draw_block_preview(p, ir, doc, b, farbe);
            } else {
                let icon_name = jjcad_core::actions::action(a)
                    .map(|d| d.icon)
                    .unwrap_or("block");
                icons::paint(p, ir, icon_name, farbe);
            }
            if let Some(k) = key_label {
                p.text(
                    Pos2::new(rect.right() - 5.0, rect.bottom() - 4.0),
                    Align2::RIGHT_BOTTOM,
                    k,
                    FontId::proportional(9.5),
                    Palette::text_faint(),
                );
            }
        } else {
            // leerer Slot: dezentes Plus
            icons::paint(
                p,
                Rect::from_center_size(rect.center(), Vec2::splat(size.x * 0.24)),
                "plus",
                Palette::text_faint().gamma_multiply(0.6),
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
pub const LABEL_W: f32 = 160.0;
/// Breite der Wertspalte — alle Eingabefelder liegen damit exakt untereinander.
pub const VAL_W: f32 = 150.0;
/// Höhe einer Zeile
pub const ROW_H: f32 = 22.0;
/// Höhe eines Eingabefelds in einer Zeile (T0178).
///
/// Alle Bedienelemente einer Zeile — Zahlenfeld, Längenfeld und Auswahlfeld
/// (`panels::dropdown`) — sind genau so hoch. Vorher war das Auswahlfeld 20 px
/// hoch, ein einfaches `DragValue` dagegen 24 px (egui-Vorgabe
/// `interact_size.y`); nebeneinander sah das Auswahlfeld dadurch kleiner aus.
/// Die Zeile setzt deshalb in der Wertespalte `interact_size.y = FELD_H`, und
/// jedes Auswahlfeld im Einstellungsfenster bekommt dieselbe Höhe.
pub const FELD_H: f32 = 20.0;
/// Kleinste Schriftgröße, in der ein Zeilenname noch gelesen werden kann —
/// darunter wird er gekürzt (T0178).
const KLEINSTE_LESEGROESSE: f32 = 9.5;

pub fn field_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    field_row_mit_hinweis(ui, label, None, add);
}

/// Wie [`field_row`], aber der Name trägt einen Hinweis (Tooltip) und ein
/// kleines Infozeichen — für Einstellungen, deren Name allein nicht verrät,
/// was sie tun.
pub fn field_row_tip(ui: &mut Ui, label: &str, tip: &str, add: impl FnOnce(&mut Ui)) {
    field_row_mit_hinweis(ui, label, Some(tip), add);
}

fn field_row_mit_hinweis(
    ui: &mut Ui,
    label: &str,
    tip: Option<&str>,
    add: impl FnOnce(&mut Ui),
) {
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
            Palette::panel_alt().gamma_multiply(0.55),
        );
    }
    // Name: exakt links, einzeilig, notfalls gekürzt. Mit Hinweis bleibt rechts
    // Platz für das Infozeichen (T0178: 18 px statt 11-px-Zeichen, damit es
    // nicht am Namen klebt).
    let icon_raum = if tip.is_some() { 18.0 } else { 0.0 };
    let farbe = Palette::text_dim();
    let verfuegbar = LABEL_W - 8.0 - icon_raum;
    // T0146: Ein Name, der die Namensspalte ausfüllt („Geschwindigkeit der
    // Ansicht“), wird so weit verkleinert, bis er **vor** der Wertspalte endet.
    // Die Spaltenbreite bleibt für alle Zeilen gleich; ohne die Verkleinerung
    // liefe der Name ein Pixel in das Eingabefeld. Gekürzt wird erst, wenn der
    // Name auch in der kleinsten Lesegröße nicht passt (T0178: der Platz für
    // das Infozeichen nimmt 18 px weg, „Geschwindigkeit der Ansicht“ wurde
    // deshalb vorher abgeschnitten statt verkleinert).
    let mut text = label.to_string();
    let mut font = FontId::proportional(11.5);
    let messen = |ui: &Ui, text: &str, font: &FontId| {
        ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), farbe))
            .size()
            .x
    };
    let gemessen = messen(ui, &text, &font);
    if gemessen > verfuegbar {
        let kleiner = passende_schriftgroesse(gemessen, verfuegbar, 11.5);
        if kleiner >= KLEINSTE_LESEGROESSE {
            font = FontId::proportional(kleiner);
        } else {
            font = FontId::proportional(KLEINSTE_LESEGROESSE);
            while text.chars().count() > 3 && messen(ui, &format!("{text}…"), &font) > verfuegbar {
                text.pop();
            }
            text = format!("{}…", text.trim_end());
        }
    }
    ui.painter().text(
        egui::pos2(row.left() + 4.0, row.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        farbe,
    );
    if let Some(t) = tip {
        // Einheitliches Infozeichen rechts in der Namensspalte; der Tooltip
        // liegt auf der ganzen Namensfläche (T0178: vorher 11 px, dadurch
        // gequetscht wirkend).
        let icon = Rect::from_center_size(
            egui::pos2(row.left() + LABEL_W - 9.0, row.center().y),
            Vec2::splat(12.0),
        );
        crate::icons::paint(ui.painter(), icon, "info", Palette::text_faint());
        ui.interact(
            Rect::from_min_size(row.min, Vec2::new(LABEL_W, ROW_H)),
            ui.id().with(("feldhinweis", idx)),
            Sense::hover(),
        )
        .on_hover_text(t);
    }
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
            // T0178: gleiche Höhe für alle Bedienelemente der Wertespalte —
            // sonst ist das Auswahlfeld (20 px) kleiner als ein Zahlenfeld
            // (egui-Vorgabe 24 px).
            ui.spacing_mut().interact_size.y = FELD_H;
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
            Palette::panel_alt().gamma_multiply(0.55),
        );
    }
    ui.painter().text(
        egui::pos2(row.left() + 4.0, row.top() + ROW_H * 0.5),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        Palette::text_dim(),
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
                    (Palette::accent_soft(), Palette::accent(), Palette::accent())
                } else if resp.hovered() {
                    (Palette::panel_alt(), Palette::accent().gamma_multiply(0.7), Palette::text())
                } else {
                    // Wie die Eingabefelder: dunkler Feldgrund (`Palette::bg()`,
                    // #121418) statt der durchscheinenden Panel-Zeile
                    // (`Palette::panel()`, #1a1d22). Gleicher Rand und gleiche
                    // Rundung wie die Felder daneben.
                    (Palette::bg(), Palette::panel_alt().gamma_multiply(1.6), Palette::text_dim())
                };
                p.rect_filled(r, Rounding::same(4.0), fuellung);
                p.rect_stroke(r, Rounding::same(4.0), Stroke::new(1.0, rand));
                align_pfeil(p, r.center(), *wert_h, *wert_v, farbe);
            }
            let tip = format!("{} · {}", wert_h.text(), wert_v.text());
            if resp.on_hover_text(tip).clicked() {
                *h = *wert_h;
                *v = *wert_v;
                geaendert = true;
            }
        }
    }
    geaendert
}

/// Strichstärke der Ausrichtungs-Icons (px) — bewusst fett, damit die Pfeile
/// neben den 11-px-Beschriftungen stehen können.
const ALIGN_STRICH: f32 = 2.6;
/// Halbe Länge eines Pfeils (Spitze bei +L, Ende bei −L).
const ALIGN_PFEIL_L: f32 = 6.5;
/// Länge der Pfeilspitze.
const ALIGN_KOPF_L: f32 = 4.2;
/// Halbe Breite der Pfeilspitze.
const ALIGN_KOPF_B: f32 = 3.6;
/// Seitenlänge des Mittelquadrats.
const ALIGN_QUADRAT: f32 = 8.5;

/// Züge eines Ausrichtungspfeils: Schaft und Spitzendreieck.
///
/// Der Pfeil wird **in seiner eigenen Achse** aufgebaut (`l` = Richtung,
/// `n` = Querachse) und erst dann in die Zellrichtung gedreht. Alle acht
/// Richtungen haben dadurch dieselbe Form — der Diagonalpfeil ist eine exakte
/// 45°-Drehung des waagerechten. Vorher wurden Länge und Breite je getrennt
/// über die halbe Zellbreite bemessen (`dx * hx`, `dy * hy`), der diagonale
/// Pfeil war damit √2-mal so lang und wuchtig wie die übrigen (T0041).
fn align_pfeil_zuege(mitte: Pos2, richtung: Vec2) -> ([Pos2; 2], [Pos2; 3]) {
    let l = richtung.normalized();
    let n = Vec2::new(-l.y, l.x);
    let spitze = mitte + l * ALIGN_PFEIL_L;
    let ende = mitte - l * ALIGN_PFEIL_L;
    let kopf = spitze - l * ALIGN_KOPF_L;
    (
        [ende, kopf + l * 0.6],
        [
            spitze,
            kopf + n * ALIGN_KOPF_B,
            kopf - n * ALIGN_KOPF_B,
        ],
    )
}

/// Pfeil der Ausrichtungsmatrix: zeigt in die Richtung der gewählten Zelle.
///
/// Ecken und Kanten: derselbe Pfeil in acht Richtungen, Mitte: kleines
/// Quadrat (der Einfügepunkt sitzt in der Textmitte).
fn align_pfeil(p: &egui::Painter, mitte: Pos2, h: TextAlignH, v: TextAlignV, col: Color32) {
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
    let st = Stroke::new(ALIGN_STRICH, col);
    if dx == 0.0 && dy == 0.0 {
        // Mitte: Quadrat (der Einfügepunkt sitzt in der Textmitte)
        p.rect_stroke(
            Rect::from_center_size(mitte, Vec2::splat(ALIGN_QUADRAT)),
            Rounding::same(1.5),
            st,
        );
        return;
    }
    let (schaft, kopf) = align_pfeil_zuege(mitte, Vec2::new(dx, dy));
    p.line_segment(schaft, st);
    // Spitze: Dreieck in Richtung (dx, dy)
    p.add(Shape::convex_polygon(kopf.to_vec(), col, Stroke::NONE));
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
            egui::Label::new(egui::RichText::new(value).size(11.5).color(Palette::text())).truncate(),
        ));
    });
    let resp = resp.expect("field_row ruft den Zeileninhalt genau einmal auf");
    resp.on_hover_text(tip.unwrap_or(value))
}

// ---------------------------------------------------------------------------
// Zahlenfelder — Grenzen und Zuggeschwindigkeit an genau einer Stelle
// ---------------------------------------------------------------------------
//
// Jedes Zahlenfeld im Programm ist ein `egui::DragValue`: Klick und Ziehen
// ändert den Wert, ein zweiter Klick tippt ihn. egui rechnet beim Ziehen
// `Wert += Punkte × speed` **in der Anzeigeeinheit** und klemmt auf `range`.
//
// Zwei Fehler sind damit möglich und beide schon passiert (T0029):
//   * `speed` aus der Einheit abgeleitet (früher `unit.mm() * 0.5`): in Metern
//     verschob ein Pixel den Wert um 500 m — „springt um 1000 m“.
//   * `range` vergessen: der Ziehpfeil lief ins Negative (negative Wandbreiten)
//     und bis in Milliardenwerte.
//
// Deshalb gibt es nur noch zwei Bausteine, und **beide verlangen Grenzen und
// Geschwindigkeit als Argument** — vergessen kann man sie nicht mehr, das
// Programm übersetzt dann nicht:
//   * `length_field` — Längen in mm, Anzeige in der Dokumenteinheit,
//     Zuggeschwindigkeit fest `ZUG_MM_JE_PUNKT` (1 cm je Bildpunkt).
//   * `zahl` — alles andere (Winkel, Stückzahlen, Faktoren, Papier-mm) mit
//     ausdrücklich genanntem `speed` und `range`.
// Die Grenzen stehen als benannte Vorgaben darunter (`MASS_*`, `ABSTAND_*`,
// `KOORD_*`, `WINKEL_*`); eigener Wert nur, wenn ein Feld wirklich anders ist,
// und dann mit Begründung im Kommentar. `DragValue::new` wird außerhalb dieser
// Datei nirgends aufgerufen — `tests::zahlenfelder_nur_hier` wacht darüber.

/// Zuggeschwindigkeit eines Längenfelds: Millimeter je Bildpunkt, unabhängig
/// von der Anzeigeeinheit (Umschalt gedrückt = ein Zehntel, also 1 mm).
///
/// Bewusst **nicht** von `Unit::mm()` abgeleitet: die Anzeigeeinheit bestimmt
/// nur, wie der Wert aussieht, nicht wie schnell er sich ziehen lässt.
pub const ZUG_MM_JE_PUNKT: f64 = 10.0;

/// Kleinste Abmessung, die ein Feld zulässt: 1 mm.
pub const MASS_MIN_MM: f64 = 1.0;
/// Größte Abmessung: 100 m — alles darüber ist ein Tippfehler, kein Bauwerk.
pub const MASS_MAX_MM: f64 = 100_000.0;
/// Kleinster Abstand bzw. Musterwert: 0,1 mm (feine Schraffur).
pub const ABSTAND_MIN_MM: f64 = 0.1;
/// Größter Abstand bzw. Musterwert: 10 m.
pub const ABSTAND_MAX_MM: f64 = 10_000.0;
/// Koordinaten und Höhenkoten dürfen negativ sein (Lage unter dem Nullpunkt).
pub const KOORD_MIN_MM: f64 = -1_000_000.0;
/// Gegenstück zu [`KOORD_MIN_MM`]: +1 km.
pub const KOORD_MAX_MM: f64 = 1_000_000.0;

/// Zeilenabstand eines Textes als Vielfaches der Zeilenhöhe der Schrift.
///
/// Ein Faktor, keine Länge: 1 = Zeilenhöhe der Schrift selbst, 0,5 staucht
/// die Zeilen, 3,0 streckt sie auf das Dreifache. Der Textrahmen folgt dem
/// Wert (`render::TextPrim::line_height`).
pub const ZEILENABSTAND_MIN: f64 = 0.5;
pub const ZEILENABSTAND_MAX: f64 = 3.0;

/// Winkelbereich für Drehungen: ±360°, 1° je Bildpunkt.
pub const WINKEL_MIN: f64 = -360.0;
/// Gegenstück zu [`WINKEL_MIN`].
pub const WINKEL_MAX: f64 = 360.0;

/// Längenfeld mit Einheitensuffix, Grenzen in **Millimetern** und der
/// Zuggeschwindigkeit aus [`ZUG_MM_JE_PUNKT`].
///
/// Grenzen werden in die Dokumenteinheit umgerechnet und gelten auch beim
/// Tippen (egui klemmt auf `range`). `true`, wenn der Nutzer den Wert geändert
/// hat.
pub fn length_field(
    ui: &mut Ui,
    value_mm: &mut f64,
    unit: jjcad_core::units::Unit,
    width: f32,
    min_mm: f64,
    max_mm: f64,
) -> bool {
    let mut v = unit.from_mm(*value_mm);
    let decimals = unit.decimals();
    // Anzeige fest an der Einheit ausrichten statt an egui's automatischer
    // Stellenzahl (die hängt an aim_radius/speed): sonst stand für 0,24 m nur
    // „0 m“ im Feld. format_length_auto zeigt 0,24 m / 2,75 m / 240 mm und
    // keine Tausenderpunkte (eindeutig lesbar).
    let drag = zahlenkern(&mut v, ZUG_MM_JE_PUNKT / unit.mm(), unit.from_mm(min_mm)..=unit.from_mm(max_mm))
        .min_decimals(decimals)
        .max_decimals(decimals + 2)
        .custom_formatter(move |n, _| unit.format_length_auto(unit.to_mm(n)))
        .suffix(format!(" {}", unit.short()));
    let resp = ui.add_sized([width, FELD_H], drag);
    if resp.changed() {
        *value_mm = unit.to_mm(v);
        true
    } else {
        false
    }
}

/// Zahlenfeld ohne Einheitenumrechnung: Winkel in Grad, Stückzahlen, Faktoren,
/// Papier-Millimeter. `speed` ist der Zuwachs je Bildpunkt, `range` die harten
/// Grenzen — beides Pflicht.
pub fn zahl<'a, T: egui::emath::Numeric>(
    v: &'a mut T,
    speed: f64,
    range: std::ops::RangeInclusive<T>,
) -> egui::DragValue<'a> {
    // Ganzzahlige Felder ohne Nachkommastellen, sonst lässt egui die Stellenzahl
    // aus der Zuggeschwindigkeit ableiten (0,01 Faktor → zwei Stellen).
    let drag = zahlenkern(v, speed, range);
    if T::INTEGRAL {
        drag.max_decimals(0)
    } else {
        drag
    }
}

/// Gemeinsamer Kern: deutsches Zahlenformat, Komma **und** Punkt als
/// Dezimaltrennzeichen, Grenzen und Zuggeschwindigkeit.
fn zahlenkern<'a, T: egui::emath::Numeric>(
    v: &'a mut T,
    speed: f64,
    range: std::ops::RangeInclusive<T>,
) -> egui::DragValue<'a> {
    egui::DragValue::new(v)
        .speed(speed)
        .range(range)
        .custom_parser(flexible_number)
        .custom_formatter(|n, decimals| {
            let d = *decimals.start();
            let s = format!("{:.*}", d, n);
            if d == 0 {
                s
            } else {
                s.replace('.', ",")
            }
        })
}

/// Trennlinie in der Horizontalen.
pub fn hline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::ZERO, Palette::border_soft());
}

/// Vertikaler Trenner für Ribbon-Gruppen.
pub fn vline(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, height), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_center_size(rect.center(), Vec2::new(1.0, height * 0.7)),
        Rounding::ZERO,
        Palette::border_soft(),
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

/// Auswahl der Schriftart für Texte.
///
/// Jede Zeile steht **in ihrer eigenen Schrift** (Muster), damit die
/// technischen Schriften ohne Probieren auseinanderzuhalten sind. Nur Familien,
/// die es auf diesem Rechner gibt, stehen zur Wahl; DIN 1451 und EN ISO 3098
/// liegen mit im Programm und sind deshalb immer da. Reihenfolge und
/// Bezeichnung kommen aus dem Kern (`TextFont`).
pub fn schrift_wahl(ui: &mut Ui, id: &str, font: &mut TextFont) -> bool {
    let gewaehlt = crate::panels::dropdown(ui, id, font.text(), None, |ui| {
        let mut out = None;
        for f in TextFont::ALL {
            let da = jjcad_core::fonts::verfuegbar(f);
            if !da && f != *font {
                continue;
            }
            let text = if da {
                f.text().to_string()
            } else {
                jjcad_core::i18n::t_args_owned("gui.widgets.x_nicht_installiert", &[("schrift", f.text().to_string())])
            };
            let familie = crate::canvas::schrift_familie(ui.ctx(), f, false);
            let zeile = egui::RichText::new(text).font(FontId::new(13.0, familie));
            if ui.selectable_label(*font == f, zeile).clicked() {
                out = Some(f);
            }
        }
        out
    });
    match gewaehlt {
        Some(f) if f != *font => {
            *font = f;
            true
        }
        _ => false,
    }
}

/// Typische Zeilenabstände aus Textprogrammen (Vielfaches der Zeilenhöhe).
///
/// Einfach bis Doppelt sind die Werte, die Word, LibreOffice und InDesign
/// anbieten; ein freier Wert bleibt im selben Menü einstellbar (T0126).
pub const ZEILENABSTAENDE: [(f64, &str); 6] = [
    (1.0, "Einfach"),
    (1.15, "1,15"),
    (1.5, "1,5"),
    (2.0, "Doppelt"),
    (2.5, "2,5"),
    (3.0, "3,0"),
];

/// Auswahl des Zeilenabstands eines Textfelds — dieselben Vorgaben wie in
/// Textprogrammen, dazu ein freies Feld.
pub fn zeilenabstand_wahl(ui: &mut Ui, id: &str, spacing: &mut f64) -> bool {
    let cap = ZEILENABSTAENDE
        .iter()
        .find(|(v, _)| (v - *spacing).abs() < 1e-6)
        .map(|(_, l)| (*l).to_string())
        .unwrap_or_else(|| format!("{:.2} ×", spacing).replace('.', ","));
    let gewaehlt = crate::panels::dropdown(ui, id, &cap, None, |ui| {
        let mut out = None;
        for (v, l) in ZEILENABSTAENDE {
            if ui
                .selectable_label((v - *spacing).abs() < 1e-6, l)
                .on_hover_text(jjcad_core::i18n::tr("gui.props.zeilenabstand_hinweis"))
                .clicked()
            {
                out = Some(v);
            }
        }
        ui.separator();
        // Freier Wert: das Feld selbst gilt weiter (Grenzen wie beim Panel).
        let mut frei = *spacing;
        if ui
            .add(
                zahl(
                    &mut frei,
                    0.01,
                    ZEILENABSTAND_MIN..=ZEILENABSTAND_MAX,
                )
                .suffix(" ×"),
            )
            .changed()
        {
            out = Some(frei);
        }
        out
    });
    match gewaehlt {
        Some(v) if (v - *spacing).abs() > 1e-9 => {
            *spacing = v.clamp(ZEILENABSTAND_MIN, ZEILENABSTAND_MAX);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod zahltests {
    use super::*;

    /// Die Zeilenabstand-Vorgaben sind die üblichen Werte aus
    /// Textprogrammen, aufsteigend und innerhalb der Feldgrenzen (T0126).
    #[test]
    fn zeilenabstand_vorgaben_sind_die_ueblichen_werte() {
        let werte: Vec<f64> = ZEILENABSTAENDE.iter().map(|(v, _)| *v).collect();
        assert!(werte.contains(&1.0), "Einfach fehlt: {werte:?}");
        assert!(werte.contains(&1.5), "1,5 fehlt: {werte:?}");
        assert!(werte.contains(&2.0), "Doppelt fehlt: {werte:?}");
        assert!(
            werte.windows(2).all(|w| w[0] < w[1]),
            "Vorgaben nicht aufsteigend: {werte:?}"
        );
        for v in werte {
            assert!(
                v >= ZEILENABSTAND_MIN && v <= ZEILENABSTAND_MAX,
                "Vorgabe {v} außerhalb der Grenzen"
            );
        }
    }

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
                                length_field(
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

    /// Fährt einen Ziehzug (`dx` Punkte) über dem Widget und gibt den Wert
    /// danach zurück. `feld` zeichnet das zu prüfende Zahlenfeld.
    fn ziehe(
        start_mm: f64,
        einheit: jjcad_core::units::Unit,
        min_mm: f64,
        max_mm: f64,
        dx: f32,
    ) -> f64 {
        let feld = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(200.0, 20.0));
        let mitte = feld.center();
        let ctx = egui::Context::default();
        let mut wert = start_mm;
        let frames: Vec<Vec<egui::Event>> = vec![
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
        for events in frames {
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
                            length_field(ui, &mut wert, einheit, 200.0, min_mm, max_mm);
                        },
                    );
                });
            });
        }
        wert
    }

    /// Wandbreite in Metern (T0029): `speed` kam aus der Einheit, ein Pixel Zug
    /// verschob den Wert um 500 m. Jetzt gilt in jeder Einheit 1 cm je Punkt —
    /// und nach unten ist bei [`MASS_MIN_MM`] Schluss, keine negativen Breiten.
    #[test]
    fn wandbreite_zieht_im_zentimeter_schritt() {
        // 60 Punkte Zug: rund 60 cm. Mit dem alten Faktor wären es 30 000 mm
        // (an der Obergrenze), der Test fiele.
        let nach = ziehe(240.0, jjcad_core::units::Unit::M, MASS_MIN_MM, MASS_MAX_MM, 60.0);
        let zuwachs = nach - 240.0;
        assert!(
            (500.0..=700.0).contains(&zuwachs),
            "60 Punkte Zug müssen ~60 cm ergeben, waren {zuwachs} mm"
        );
        assert!(zuwachs / 60.0 <= 20.0, "höchstens 2 cm je Punkt, war {}", zuwachs / 60.0);
        // Weit nach links: bleibt an der Untergrenze, niemals negativ.
        assert_eq!(
            ziehe(240.0, jjcad_core::units::Unit::M, MASS_MIN_MM, MASS_MAX_MM, -300.0),
            MASS_MIN_MM
        );
        // Dieselbe Zugrate in Zentimetern und Millimetern (Einheit ändert nur die Anzeige).
        for einheit in [jjcad_core::units::Unit::Mm, jjcad_core::units::Unit::Cm] {
            let v = ziehe(240.0, einheit, MASS_MIN_MM, MASS_MAX_MM, 60.0);
            let d = v - 240.0;
            assert!((500.0..=700.0).contains(&d), "60 Punkte in {einheit:?}: {d} mm");
        }
    }

    /// Faktorfeld (Block skalieren) und Stückzahl: Grenzen gelten, auch nach
    /// oben. `zahl` verlangt beides als Argument.
    #[test]
    fn zahlfeld_bleibt_in_den_grenzen() {
        let feld = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(120.0, 20.0));
        let mitte = feld.center();
        let mut wert: f64 = 1.0;
        let ctx = egui::Context::default();
        for events in vec![
            Vec::<egui::Event>::new(),
            vec![egui::Event::PointerMoved(mitte)],
            vec![egui::Event::PointerButton {
                pos: mitte,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            vec![egui::Event::PointerMoved(egui::pos2(mitte.x - 5000.0, mitte.y))],
            vec![egui::Event::PointerButton {
                pos: egui::pos2(mitte.x - 5000.0, mitte.y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        ]
        .into_iter()
        {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 60.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(feld)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| {
                            ui.add_sized([120.0, 20.0], zahl(&mut wert, 0.01, 0.01..=1000.0));
                        },
                    );
                });
            });
        }
        assert_eq!(wert, 0.01, "Faktor bleibt an der Untergrenze");
    }

    /// Wächter: Zahlenfelder entstehen **nur** in dieser Datei und nur über
    /// `zahl`/`length_field`. Ein rohes `DragValue::new` in einem Panel hat
    /// (T0029) reihenweise Felder ohne Grenzen und mit sinnloser
    /// Zuggeschwindigkeit erzeugt.
    #[test]
    fn zahlenfelder_nur_ueber_widgets() {
        for (name, quelle) in [
            ("panels.rs", include_str!("panels.rs")),
            ("dialogs.rs", include_str!("dialogs.rs")),
            ("app.rs", include_str!("app.rs")),
            ("canvas.rs", include_str!("canvas.rs")),
            ("hotbar.rs", include_str!("hotbar.rs")),
            ("inventory.rs", include_str!("inventory.rs")),
            ("textedit.rs", include_str!("textedit.rs")),
        ] {
            assert!(
                !quelle.contains("DragValue"),
                "{name}: Zahlenfelder nur über widgets::zahl bzw. widgets::length_field bauen"
            );
            assert!(
                !quelle.contains(".speed("),
                "{name}: Zuggeschwindigkeit gehört in widgets.rs, nicht an die Aufrufstelle"
            );
            assert!(
                !quelle.contains(".range("),
                "{name}: Grenzen gehören in widgets.rs, nicht an die Aufrufstelle"
            );
        }
    }
}

#[cfg(test)]
mod icontests {
    use super::*;

    /// Sammelt alle gefüllten Rechtecke eines Bildes (auch in Gruppen).
    fn fuellflaechen(shape: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
        match shape {
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| fuellflaechen(s, out)),
            _ => {}
        }
    }

    /// Die acht Pfeile der Ausrichtungsmatrix sind **eine** Form, nur gedreht.
    /// Vorher wurden Länge und Breite je über die halbe Zellbreite bemessen
    /// (`dx * hx`, `dy * hy`) — der Diagonalpfeil war damit √2-mal so lang und
    /// wuchtig wie die übrigen (T0041).
    #[test]
    fn ausrichtungspfeile_sind_dieselbe_gedrehte_form() {
        let mitte = Pos2::new(60.0, 60.0);
        let (schaft, kopf) = align_pfeil_zuege(mitte, Vec2::new(1.0, 0.0));
        let basis: Vec<Vec2> = schaft
            .iter()
            .chain(kopf.iter())
            .map(|p| *p - mitte)
            .collect();
        let radius = basis.iter().map(|p| p.length()).fold(0.0f32, f32::max);

        for (dx, dy) in [
            (1.0f32, 0.0f32),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
        ] {
            let (s, k) = align_pfeil_zuege(mitte, Vec2::new(dx, dy));
            let punkte: Vec<Vec2> = s.iter().chain(k.iter()).map(|p| *p - mitte).collect();
            // Gleiche Ausdehnung in jeder Richtung: die Diagonale ist nicht länger.
            let r = punkte.iter().map(|p| p.length()).fold(0.0f32, f32::max);
            assert!(
                (r - radius).abs() < 1e-3,
                "Richtung ({dx},{dy}): Länge {r} statt {radius}"
            );
            // Und zwar als exakte Drehung der waagerechten Form.
            let winkel = dy.atan2(dx);
            let (si, co) = (winkel.sin(), winkel.cos());
            for b in &basis {
                let gedreht = Vec2::new(b.x * co - b.y * si, b.x * si + b.y * co);
                assert!(
                    punkte.iter().any(|p| (*p - gedreht).length() < 1e-3),
                    "Richtung ({dx},{dy}): gedrehter Punkt {gedreht:?} fehlt"
                );
            }
        }
    }

    /// Die fetteren Pfeile bleiben im Vergleich zur alten Form deutlich
    /// kräftiger: Strichstärke und Spitzenbreite sind größer als vorher
    /// (1,6 px Strich, 2,7 px halbe Spitzenbreite).
    #[test]
    fn ausrichtungspfeile_sind_fetter() {
        assert!(ALIGN_STRICH >= 2.2, "Strichstärke {}", ALIGN_STRICH);
        assert!(ALIGN_KOPF_B >= 3.2, "halbe Spitzenbreite {}", ALIGN_KOPF_B);
        assert!(
            ALIGN_PFEIL_L * 2.0 <= ALIGN_ZELLE - 2.0 * 6.0,
            "Pfeil passt in die Zelle"
        );
    }

    /// Grund der Ausrichtungsfelder ist derselbe wie bei den Eingabefeldern
    /// (`Palette::bg()`, #121418). Vorher schien der Panel-Ton (#1a1d22) durch
    /// die Zelle — die Knöpfe sahen anders aus als die Felder daneben (T0041).
    #[test]
    fn ausrichtungsfelder_haben_feldgrund() {
        let ctx = egui::Context::default();
        let mut h = TextAlignH::Left;
        let mut v = TextAlignV::Top;
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    align_matrix(ui, "pruefung", &mut h, &mut v);
                });
            },
        );
        let mut flaechen = Vec::new();
        for cs in &out.shapes {
            fuellflaechen(&cs.shape, &mut flaechen);
        }
        let zelle = |r: &Rect| {
            (r.width() - ALIGN_ZELLE).abs() < 0.1 && (r.height() - ALIGN_ZELLE).abs() < 0.1
        };
        let felder = flaechen
            .iter()
            .filter(|(r, f)| zelle(r) && *f == Palette::bg())
            .count();
        let aktiv = flaechen
            .iter()
            .filter(|(r, f)| zelle(r) && *f == Palette::accent_soft())
            .count();
        assert_eq!(felder, 8, "acht inaktive Zellen mit Feldgrund, gefunden {felder}");
        assert_eq!(aktiv, 1, "genau eine gewählte Zelle, gefunden {aktiv}");
    }
}

#[cfg(test)]
mod tests_ribbon_beschriftung {
    //! T0060: „Schnellauswahl" ist länger als die 70-px-Schaltfläche bei
    //! 10,5 Punkten — die Beschriftung lief in die Nachbargruppe. Gemessen wird
    //! die Rechnung, die die Schrift verkleinert; die Breite selbst kommt vom
    //! Painter (Zeichenbreiten, keine Schätzung).

    use super::*;

    #[test]
    fn kurze_beschriftung_bleibt_gross() {
        assert_eq!(passende_schriftgroesse(40.0, 64.0, RIBBON_TEXT), RIBBON_TEXT);
    }

    #[test]
    fn lange_beschriftung_wird_kleiner_aber_lesbar() {
        let s = passende_schriftgroesse(90.0, 64.0, RIBBON_TEXT);
        assert!(s < RIBBON_TEXT, "zu breit: {s} Punkte");
        // 90 → 64 verlangt 7,47 Punkte; unter 8 wird nicht verkleinert.
        assert_eq!(s, 8.0, "Untergrenze der Lesbarkeit");
    }

    #[test]
    fn mittlere_beschriftung_passt_genau() {
        let s = passende_schriftgroesse(80.0, 64.0, RIBBON_TEXT);
        assert!((s - 8.4).abs() < 1e-4, "erwartet 8,4 Punkte, war {s}");
    }
}

#[cfg(test)]
mod tests_slot_bild {
    //! T0116: Ein Slot mit Block zeigt die **Miniaturaansicht dieses Blocks** —
    //! nicht das gemeinsame Ersatzsymbol. Geprüft wird die gezeichnete
    //! Geometrie im Slot: ihr Seitenverhältnis muss dem Block entsprechen.

    use super::*;
    use jjcad_core::model::Document;

    fn punkte(shape: &egui::Shape, out: &mut Vec<Pos2>) {
        match shape {
            egui::Shape::LineSegment { points, .. } => out.extend_from_slice(points),
            egui::Shape::Path(p) => out.extend_from_slice(&p.points),
            egui::Shape::Vec(v) => {
                for s in v {
                    punkte(s, out);
                }
            }
            _ => {}
        }
    }

    /// Zeichnet einen Slot und liefert die Streckenpunkte **im** Slot.
    fn slot_punkte(doc: &Document, b: &BlockDef) -> Vec<Pos2> {
        let ctx = egui::Context::default();
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        slot_button(
                            ui,
                            Vec2::splat(50.0),
                            Some(&format!("block:{}", b.id)),
                            None,
                            false,
                            false,
                            Some((doc, b)),
                        );
                    });
                },
            ));
        }
        let out = out.expect("Bild");
        let mut alle = Vec::new();
        for cs in &out.shapes {
            punkte(&cs.shape, &mut alle);
        }
        // Nur die Zeichnung im Slot, nicht der Rahmen der Leiste drumherum.
        let slot = Rect::from_min_size(Pos2::ZERO, Vec2::splat(50.0));
        alle.retain(|p| slot.expand(2.0).contains(*p));
        alle
    }

    fn umriss(punkte: &[Pos2]) -> Rect {
        let mut r = Rect::NOTHING;
        for p in punkte {
            r.extend_with(*p);
        }
        r
    }

    #[test]
    fn block_im_slot_zeigt_seine_eigene_miniatur() {
        let doc = Document::new();
        let mut geprueft = 0;
        let mut ungleich_seitig = 0;
        for b in doc.blocks.iter().take(8) {
            let bbox = b.bbox();
            if !bbox.is_valid() || bbox.width() <= 0.0 || bbox.height() <= 0.0 {
                continue;
            }
            let punkte = slot_punkte(&doc, b);
            assert!(
                punkte.len() >= 6,
                "Block „{}“ zeichnet im Slot nichts ({} Punkte)",
                b.name,
                punkte.len()
            );
            let r = umriss(&punkte);
            let block_seite = (bbox.width() / bbox.height()) as f32;
            let bild_seite = r.width() / r.height().max(0.01);
            if (block_seite - 1.0).abs() > 0.35 {
                ungleich_seitig += 1;
            }
            assert!(
                (bild_seite / block_seite - 1.0).abs() < 0.15,
                "Block „{}“ ist {} breit je Höhe, im Slot steht aber {:.2} — das ist nicht seine Miniatur",
                b.name,
                block_seite,
                bild_seite
            );
            geprueft += 1;
        }
        assert!(geprueft >= 3, "zu wenige Blöcke geprüft: {geprueft}");
        assert!(
            ungleich_seitig >= 1,
            "kein ungleichseitiger Block in der Auswahl — der Test könnte ein Ersatzsymbol nicht erkennen"
        );
    }

    #[test]
    fn werkzeug_im_slot_behaelt_sein_icon() {
        let doc = Document::new();
        let ctx = egui::Context::default();
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        slot_button(ui, Vec2::splat(50.0), Some("draw.wall"), None, false, false, None);
                    });
                },
            ));
        }
        let out = out.expect("Bild");
        let mut alle = Vec::new();
        for cs in &out.shapes {
            punkte(&cs.shape, &mut alle);
        }
        let slot = Rect::from_min_size(Pos2::ZERO, Vec2::splat(50.0));
        alle.retain(|p| slot.expand(2.0).contains(*p));
        assert!(
            alle.len() >= 2,
            "das Werkzeug zeichnet kein Icon mehr ({} Punkte)",
            alle.len()
        );
        let _ = doc;
    }
}
