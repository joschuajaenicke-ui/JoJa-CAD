//! Seitenpanels: Layer (links) und Eigenschaften (rechts).

use crate::app::{self, App, TabKind};
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::geom::Polyline;
use jjcad_core::model::*;
use jjcad_core::style::{Color, LineType, PaperSize, WallAlign};
use jjcad_core::units::{format_number, format_scale, Unit};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------

pub fn layers(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("LAYER")
                .size(10.0)
                .color(Palette::TEXT_FAINT)
                .strong(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, "plus", 22.0, false, "Neuer Layer").clicked() {
                let n = app.doc().layers.len() + 1;
                app.ui.new_layer_dialog = Some(format!("Layer {n}"));
            }
            if widgets::icon_button(ui, "layers", 22.0, false, "Layer verwalten").clicked() {
                app.ui.layers_open = !app.ui.layers_open;
            }
            if widgets::icon_button(ui, "chevron_left", 20.0, false, "Einklappen").clicked() {
                app.ui.layers_collapsed = true;
            }
        });
    });
    ui.add_space(4.0);

    if let Some(name) = app.ui.new_layer_dialog.clone() {
        ui.horizontal(|ui| {
            let mut n = name;
            let resp = ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut n).hint_text("Layer-Name"));
            let ok = ui.button("OK").clicked() || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if ok {
                match app.session_mut().execute("layer.create", &json!({"name": n})) {
                    Ok(_) => {
                        app.mark_changed();
                        app.toast(format!("Layer '{n}' angelegt"));
                    }
                    Err(e) => app.toast(e),
                }
                app.ui.new_layer_dialog = None;
            } else {
                app.ui.new_layer_dialog = Some(n);
            }
            if ui.button("Abbrechen").clicked() {
                app.ui.new_layer_dialog = None;
            }
        });
        ui.add_space(4.0);
    }

    let current = app.doc().current_layer;
    let layers = app.doc().layers.clone();
    let mut set_current: Option<Id> = None;
    let mut toggle_vis: Option<Id> = None;
    let mut toggle_lock: Option<Id> = None;
    let mut color_change: Option<(Id, Color)> = None;
    let mut rename: Option<(Id, String)> = None;
    let mut delete: Option<Id> = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for l in &layers {
                let is_current = l.id == current;
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 27.0),
                    Sense::click(),
                );
                if ui.is_rect_visible(rect) {
                    let p = ui.painter();
                    if is_current {
                        p.rect_filled(rect, Rounding::same(6.0), Palette::PANEL_ALT);
                        p.rect_filled(
                            Rect::from_min_size(rect.left_top(), Vec2::new(2.0, rect.height())),
                            Rounding::same(1.0),
                            Palette::ACCENT,
                        );
                    } else if resp.hovered() {
                        p.rect_filled(rect, Rounding::same(6.0), Palette::PANEL_ALT.gamma_multiply(0.6));
                    }
                    // Farbpunkt
                    let dot = Rect::from_center_size(
                        Pos2::new(rect.left() + 13.0, rect.center().y),
                        Vec2::splat(14.0),
                    );
                    p.circle_filled(dot.center(), 5.0, app::color_to_egui(l.color));
                    p.circle_stroke(dot.center(), 5.0, Stroke::new(1.0, Color32::from_black_alpha(80)));
                    // Name (gekürzt, damit er nicht unter die Symbole läuft)
                    let max_w = rect.width() - 92.0;
                    let mut name = l.name.clone();
                    while p
                        .layout_no_wrap(name.clone(), FontId::proportional(12.0), Palette::TEXT)
                        .size()
                        .x
                        > max_w
                        && name.chars().count() > 3
                    {
                        name.pop();
                    }
                    if name != l.name {
                        name.pop();
                        name.push('…');
                    }
                    p.text(
                        Pos2::new(rect.left() + 25.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(12.0),
                        if is_current { Palette::TEXT } else { Palette::TEXT_DIM },
                    );
                    // Sichtbarkeit / Sperre
                    let eye = Rect::from_center_size(
                        Pos2::new(rect.right() - 40.0, rect.center().y),
                        Vec2::splat(20.0),
                    );
                    crate::icons::paint(
                        p,
                        eye.shrink(3.0),
                        if l.visible { "eye" } else { "eye_off" },
                        if l.visible { Palette::TEXT_DIM } else { Palette::TEXT_FAINT },
                    );
                    let lock = Rect::from_center_size(
                        Pos2::new(rect.right() - 19.0, rect.center().y),
                        Vec2::splat(20.0),
                    );
                    crate::icons::paint(
                        p,
                        lock.shrink(4.0),
                        if l.locked { "lock" } else { "unlock" },
                        if l.locked { Palette::ACCENT } else { Palette::TEXT_FAINT },
                    );
                }
                // Interaktion
                if resp.double_clicked() {
                    rename = Some((l.id, l.name.clone()));
                } else if resp.clicked() {
                    let pos = resp.interact_pointer_pos().unwrap_or(rect.center());
                    if pos.x > rect.right() - 30.0 {
                        toggle_lock = Some(l.id);
                    } else if pos.x > rect.right() - 52.0 {
                        toggle_vis = Some(l.id);
                    } else if pos.x < rect.left() + 22.0 {
                        app.ui.color_popup = Some(l.id);
                    } else {
                        set_current = Some(l.id);
                    }
                }
                if resp.secondary_clicked() {
                    app.ui.color_popup = Some(l.id);
                }
                let hover_text = format!("{} · {:.2} mm · {}", l.name, l.line_weight, l.line_type.label());
                resp.clone().on_hover_text(hover_text);
                resp.context_menu(|ui| {
                    ui.label(theme::dim(&l.name));
                    ui.separator();
                    if ui.button("Aktueller Layer").clicked() {
                        set_current = Some(l.id);
                        ui.close_menu();
                    }
                    if ui.button("Umbenennen").clicked() {
                        rename = Some((l.id, l.name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Löschen").clicked() {
                        delete = Some(l.id);
                        ui.close_menu();
                    }
                });
                // Kontextmenü: Linienstärke/Sichtbarkeit
                if resp.secondary_clicked() {
                    app.ui.color_popup = Some(l.id);
                }
            }
            ui.add_space(6.0);
            ui.label(theme::dim(format!(
                "{} Layer · aktuell: {}",
                layers.len(),
                layers
                    .iter()
                    .find(|l| l.id == current)
                    .map(|l| l.name.clone())
                    .unwrap_or_default()
            )));
        });

    if let Some(id) = set_current {
        let _ = app.session_mut().execute("layer.set_current", &json!({"layer": id}));
    }
    if let Some(id) = toggle_vis {
        let vis = app.doc().layer(id).map(|l| !l.visible).unwrap_or(true);
        let _ = app
            .session_mut()
            .execute("layer.update", &json!({"layer": id, "visible": vis}));
        app.mark_changed();
    }
    if let Some(id) = toggle_lock {
        let lock = app.doc().layer(id).map(|l| !l.locked).unwrap_or(true);
        let _ = app
            .session_mut()
            .execute("layer.update", &json!({"layer": id, "locked": lock}));
        app.mark_changed();
    }
    if let Some((id, new)) = color_change {
        let _ = app
            .session_mut()
            .execute("layer.update", &json!({"layer": id, "color": new.to_hex()}));
        app.mark_changed();
    }
    if let Some((id, name)) = rename {
        app.ui.rename_layer = Some((id, name));
    }
    if let Some(id) = delete {
        match app.session_mut().execute("layer.delete", &json!({"layer": id})) {
            Ok(_) => {
                app.mark_changed();
                app.toast("Layer gelöscht");
            }
            Err(e) => app.toast(e),
        }
    }
}

/// Farbwähler-Popup für einen Layer.
pub fn layer_color_popup(app: &mut App, ctx: &egui::Context) {
    let Some(lid) = app.ui.color_popup else { return };
    let Some(layer) = app.doc().layer(lid).cloned() else {
        app.ui.color_popup = None;
        return;
    };
    let mut open = true;
    let mut chosen: Option<Color> = None;
    egui::Area::new(egui::Id::new("layercolor"))
        .anchor(egui::Align2::LEFT_TOP, Vec2::new(70.0, 220.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(&ctx.style()).show(ui, |ui| {
                ui.set_width(196.0);
                ui.label(theme::bold(&layer.name, 12.0));
                ui.add_space(4.0);
                let palette = [
                    ("Schwarz", Color::BLACK),
                    ("Grau", Color::GREY),
                    ("Hellgrau", Color::LIGHT_GREY),
                    ("Rot", Color::RED),
                    ("Orange", Color::ORANGE),
                    ("Grün", Color::GREEN),
                    ("Blau", Color::BLUE),
                    ("Cyan", Color::CYAN),
                    ("Violett", Color::PURPLE),
                    ("Braun", Color::rgb(120, 85, 55)),
                    ("Ocker", Color::rgb(190, 160, 90)),
                    ("Tinte", Color::rgb(40, 45, 52)),
                ];
                for row in palette.chunks(4) {
                    ui.horizontal(|ui| {
                        for (name, c) in row {
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                            ui.painter().circle_filled(r.center(), 10.0, app::color_to_egui(*c));
                            if layer.color == *c {
                                ui.painter().circle_stroke(
                                    r.center(),
                                    12.5,
                                    Stroke::new(1.5, Palette::ACCENT),
                                );
                            }
                            if resp.clicked() {
                                chosen = Some(*c);
                            }
                            resp.on_hover_text(*name);
                        }
                    });
                }
                ui.add_space(6.0);
                let mut rgb = [layer.color.r, layer.color.g, layer.color.b];
                ui.horizontal(|ui| {
                    ui.label(theme::dim("Eigene Farbe"));
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        chosen = Some(Color::rgb(rgb[0], rgb[1], rgb[2]));
                    }
                });
                ui.add_space(4.0);
                if ui.button("Schließen").clicked() {
                    open = false;
                }
            });
        });
    if let Some(c) = chosen {
        let _ = app
            .session_mut()
            .execute("layer.update", &json!({"layer": lid, "color": c.to_hex()}));
        app.mark_changed();
    }
    if !open {
        app.ui.color_popup = None;
    }
    // Schließen bei Klick außerhalb
    if ctx.input(|i| i.pointer.any_click()) {
        if let Some(pos) = ctx.pointer_latest_pos() {
            if pos.x > 300.0 && pos.y > 300.0 {
                app.ui.color_popup = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Eigenschaften
// ---------------------------------------------------------------------------

pub fn properties(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("EIGENSCHAFTEN")
                .size(10.0)
                .color(Palette::TEXT_FAINT)
                .strong(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, "chevron_right", 20.0, false, "Einklappen").clicked() {
                app.ui.props_collapsed = true;
            }
        });
    });
    ui.add_space(2.0);
    widgets::reset_rows();
    let sel = app.selection().to_vec();
    let tool_active = app.tool.is_some();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if tool_active {
                pending_props(app, ui);
                if !sel.is_empty() {
                    ui.add_space(6.0);
                    widgets::hline(ui);
                    ui.add_space(4.0);
                }
            }
            if sel.len() == 1 {
                entity_props(app, ui, sel[0]);
            } else if sel.len() > 1 {
                multi_props(app, ui, &sel);
            } else if !tool_active {
                match app.tab().tab {
                    TabKind::Sheet(id) => sheet_props(app, ui, id),
                    TabKind::Model => doc_props(app, ui),
                }
            }
        });
}

/// Eigenschaften für das nächste Element, während ein Werkzeug läuft.
fn pending_props(app: &mut App, ui: &mut Ui) {
    let tool_id = app.tool.as_ref().map(|t| t.action.clone()).unwrap_or_default();
    let is_wall = tool_id == "draw.wall";
    // Beide Füllwerkzeuge tragen dieselben Typparameter (Muster, Winkel,
    // Abstand, Randlinie) — Werte bleiben für das nächste Element stehen.
    let is_fill = matches!(
        tool_id.as_str(),
        "draw.fill" | "draw.hatch" | "draw.fill.boundary" | "draw.fill.points"
    );
    let label = app
        .tool
        .as_ref()
        .and_then(|t| jjcad_core::actions::action(&t.action))
        .map(|d| d.label.to_string())
        .unwrap_or_else(|| "Werkzeug".to_string());
    // Gleiche Kopfzeile wie bei einem ausgewaehlten Objekt
    if is_wall {
        section_title(ui, "wall", "Wand");
    } else if is_fill {
        section_title(ui, "fill", "Füllfläche");
    } else {
        section_title(ui, "tool", &label);
    }
    ui.label(theme::dim("gilt für das nächste Element"));
    ui.add_space(4.0);

    // Layer
    let cur = app.doc().current_layer;
    let cur_name = app
        .doc()
        .layer(cur)
        .map(|l| l.name.clone())
        .unwrap_or_default();
    let mut new_layer = cur;
    let layer_items: Vec<(Id, String)> = app
        .doc()
        .layers
        .iter()
        .map(|l| (l.id, l.name.clone()))
        .collect();
    widgets::field_row(ui, "Layer", |ui| {
        if let Some(l) = pick_combo(ui, "pend_layer", new_layer, &layer_items) {
            new_layer = l;
        }
    });
    if new_layer != cur {
        app.doc_mut().current_layer = new_layer;
    }

    // Farbe der Linie (bei der Wand: Umrandung des Körpers)
    let by_layer = app.ui.pending.color.is_none();
    let col = app.ui.pending.color.unwrap_or(Color::BLACK);
    let mut picked_col: Option<Option<Color>> = None;
    let color_label = if is_wall || is_fill { "Linienfarbe" } else { "Farbe" };
    widgets::field_row(ui, color_label, |ui| {
        picked_col = color_combo(ui, "pend_col", col, by_layer, false, "vom Layer");
    });
    if let Some(v) = picked_col {
        app.ui.pending.color = v;
    }

    // Linienstärke
    let lw = app.ui.pending.lineweight.unwrap_or(0.35);
    let mut picked_w: Option<f64> = None;
    widgets::field_row(ui, "Stärke", |ui| {
        picked_w = weight_combo(ui, "pend_w", lw, false);
    });
    if let Some(w) = picked_w {
        app.ui.pending.lineweight = Some(w);
    }

    // Linientyp
    let mut lt = app.ui.pending.line_type.unwrap_or(LineType::Solid);
    let lt_before = lt;
    widgets::field_row(ui, "Linientyp", |ui| {
        if let Some(c) = linetype_combo(ui, "pending_lt", lt, false) {
            lt = c;
        }
    });
    if lt != lt_before {
        app.ui.pending.line_type = Some(lt);
    }

    // ---- Typparameter Wand (gleiche Reihenfolge wie im Objekt-Panel) ----
    if is_wall {
        let mut t = app.options.wall_thickness;
        widgets::field_row(ui, "Dicke", |ui| {
            if widgets::length_input(ui, &mut t, app.unit(), ui.available_width()) {
                app.options.wall_thickness = t;
            }
        });
        let mut h = app.options.wall_height;
        widgets::field_row(ui, "Höhe", |ui| {
            if widgets::length_input(ui, &mut h, app.unit(), ui.available_width()) {
                app.options.wall_height = h;
            }
        });
        let align_items: Vec<(jjcad_core::style::WallAlign, String)> =
            jjcad_core::style::WallAlign::ALL
                .iter()
                .map(|c| (*c, c.label().to_string()))
                .collect();
        let cur_align = app.options.wall_align;
        let mut picked_align: Option<jjcad_core::style::WallAlign> = None;
        widgets::field_row(ui, "Ausrichtung", |ui| {
            picked_align = pick_combo(ui, "pend_wall_align", cur_align, &align_items);
        });
        if let Some(c) = picked_align {
            app.options.wall_align = c;
        }

        let cur_hatch = app
            .options
            .wall_hatch
            .unwrap_or(jjcad_core::style::FillPattern::Solid);
        let hatch_items: Vec<(jjcad_core::style::FillPattern, String)> =
            jjcad_core::style::FillPattern::ALL
                .iter()
                .map(|c| (*c, c.label().to_string()))
                .collect();
        let mut picked_hatch: Option<jjcad_core::style::FillPattern> = None;
        widgets::field_row(ui, "Schraffur", |ui| {
            picked_hatch = pick_combo(ui, "pend_wall_hatch", cur_hatch, &hatch_items);
        });
        if let Some(hp) = picked_hatch {
            app.options.wall_hatch = if hp == jjcad_core::style::FillPattern::Solid {
                None
            } else {
                Some(hp)
            };
        }

        // Füllfläche der Wand — eigene Farbe, unabhängig von der Linie
        let fill_unset = app.ui.pending.fill.is_none();
        let fill_cur = app
            .ui
            .pending
            .fill
            .unwrap_or_else(|| col.mix(Color::WHITE, 0.74));
        let mut picked_fill: Option<Option<Color>> = None;
        widgets::field_row(ui, "Füllfarbe", |ui| {
            picked_fill = color_combo(ui, "pend_fill", fill_cur, fill_unset, false, "Standard");
        });
        if let Some(v) = picked_fill {
            app.ui.pending.fill = v;
        }

        ui.add_space(2.0);
        ui.label(theme::faint("Tab wechselt die Ausrichtung"));
    }

    // ---- Typparameter Füllfläche (gleiche Zeilen wie im Objekt-Panel) ----
    if is_fill {
        let pat_items: Vec<(jjcad_core::style::FillPattern, String)> =
            jjcad_core::style::FillPattern::ALL
                .iter()
                .map(|c| (*c, c.label().to_string()))
                .collect();
        widgets::field_row(ui, "Muster", |ui| {
            if let Some(p) = pick_combo(ui, "pend_fill_pattern", app.options.fill_pattern, &pat_items) {
                app.options.fill_pattern = p;
            }
        });
        let mut winkel = app.options.fill_angle.to_degrees();
        widgets::field_row(ui, "Winkel", |ui| {
            if ui
                .add(crate::widgets::dv(&mut winkel).speed(1.0).suffix("°"))
                .changed()
            {
                app.options.fill_angle = winkel.to_radians();
            }
        });
        let mut abstand = app.options.fill_scale;
        widgets::field_row(ui, "Abstand", |ui| {
            if widgets::length_input(ui, &mut abstand, app.unit(), ui.available_width()) {
                app.options.fill_scale = abstand;
            }
        });
        let mut rand = app.options.fill_border;
        widgets::field_row(ui, "Randlinie", |ui| {
            if ui.checkbox(&mut rand, "").changed() {
                app.options.fill_border = rand;
            }
        });

        // Füllfarbe der Fläche — unabhängig von der Linienfarbe des Umrisses
        let fill_unset = app.ui.pending.fill.is_none();
        let fill_cur = app
            .ui
            .pending
            .fill
            .unwrap_or_else(|| col.mix(Color::WHITE, 0.72));
        let mut picked_fill: Option<Option<Color>> = None;
        widgets::field_row(ui, "Füllfarbe", |ui| {
            picked_fill = color_combo(ui, "pend_fill_area", fill_cur, fill_unset, false, "Standard");
        });
        if let Some(v) = picked_fill {
            app.ui.pending.fill = v;
        }
    }
}


// ---------------------------------------------------------------------------
// Vorgabewerte für Farbe und Linienstärke
// ---------------------------------------------------------------------------

/// Strichstärken nach DIN 1356 / Bauzeichnung
pub const WEIGHT_PRESETS: [(f64, &str); 3] = [
    (0.50, "Dick"),
    (0.35, "Mittel"),
    (0.25, "Dünn"),
];

/// Farbvorgaben: (Name, Bedeutung, RGB)
pub const COLOR_PRESETS: [(&str, &str, u32); 13] = [
    ("Schwarz", "Bestand", 0x000000),
    ("Rot", "Neubau", 0xFF0000),
    ("Gelb", "Abbruch", 0xFFFF00),
    ("Blau", "", 0x2E5FA3),
    ("Grün", "", 0x2E7D4F),
    ("Orange", "", 0xD9822B),
    ("Violett", "", 0x6A4C93),
    ("Türkis", "", 0x2A9D8F),
    ("Grau 25 %", "", 0x404040),
    ("Grau 50 %", "", 0x808080),
    ("Grau 75 %", "", 0xBFBFBF),
    ("Hellgrau", "", 0xD9D9D9),
    ("Weiß", "", 0xFFFFFF),
];

fn preset_color(rgb: u32) -> Color {
    Color::rgb(((rgb >> 16) & 0xFF) as u8, ((rgb >> 8) & 0xFF) as u8, (rgb & 0xFF) as u8)
}

fn swatch(ui: &mut Ui, c: Color) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(11.0), Sense::hover());
    ui.painter()
        .rect_filled(r, Rounding::same(2.0), egui::Color32::from_rgb(c.r, c.g, c.b));
    ui.painter()
        .rect_stroke(r, Rounding::same(2.0), Stroke::new(0.7, Palette::TEXT_FAINT));
    ui.add_space(4.0);
}

fn weight_text(w: f64) -> String {
    for (val, name) in WEIGHT_PRESETS {
        if (w - val).abs() < 1e-6 {
            return format!("{:.2} mm · {}", val, name).replace('.', ",");
        }
    }
    format!("{:.2} mm", w).replace('.', ",")
}

/// Eigener Dropdown: bleibt offen, während man im Popup arbeitet
/// (Zahlenfeld, Farbwähler) und zeigt im geschlossenen Zustand ein Farbfeld.
#[derive(Clone, Copy)]
enum BtnIcon {
    Color(Color),
    Line(LineType),
}

fn dropdown<R>(
    ui: &mut Ui,
    salt: &str,
    caption: &str,
    icon: Option<BtnIcon>,
    add_contents: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    let id = ui.make_persistent_id(("dd", salt));
    let w = ui.available_width().min(crate::widgets::VAL_W);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
    let open = ui.memory(|m| m.is_popup_open(id));
    let hovered = resp.hovered();
    let pnt = ui.painter();
    pnt.rect_filled(
        rect,
        egui::Rounding::same(4.0),
        if open || hovered { Palette::PANEL_ALT } else { Palette::BG },
    );
    pnt.rect_stroke(
        rect,
        egui::Rounding::same(4.0),
        Stroke::new(
            1.0,
            if open {
                Palette::ACCENT
            } else {
                Palette::PANEL_ALT.gamma_multiply(1.6)
            },
        ),
    );
    let mut x = rect.left() + 7.0;
    match icon {
        Some(BtnIcon::Color(c)) => {
            let r = Rect::from_min_size(egui::pos2(x, rect.center().y - 5.0), Vec2::splat(10.0));
            pnt.rect_filled(r, egui::Rounding::same(2.0), egui::Color32::from_rgb(c.r, c.g, c.b));
            pnt.rect_stroke(r, egui::Rounding::same(2.0), Stroke::new(0.8, Palette::TEXT_FAINT));
            x += 16.0;
        }
        Some(BtnIcon::Line(lt)) => {
            let y = rect.center().y;
            let a = egui::pos2(x, y);
            let b = egui::pos2(x + 22.0, y);
            match crate::canvas::dash_pattern(lt, 3.2, false) {
                Some(pat) => {
                    for seg in crate::canvas::dashed_segments(a, b, &pat) {
                        pnt.line_segment(seg, Stroke::new(1.3, Palette::TEXT_DIM));
                    }
                }
                None => {
                    pnt.line_segment([a, b], Stroke::new(1.3, Palette::TEXT_DIM));
                }
            }
            x += 27.0;
        }
        None => {}
    }
    pnt.text(
        egui::pos2(x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        caption,
        FontId::proportional(11.5),
        Palette::TEXT,
    );
    pnt.text(
        egui::pos2(rect.right() - 7.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        "v",
        FontId::proportional(9.0),
        Palette::TEXT_DIM,
    );

    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }

    let mut out = None;
    if ui.memory(|m| m.is_popup_open(id)) {
        egui::popup_below_widget(
            ui,
            id,
            &resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(330.0);
                egui::ScrollArea::vertical()
                    .max_height(560.0)
                    .show(ui, |ui| {
                        ui.set_min_width(316.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        if let Some(v) = add_contents(ui) {
                            out = Some(v);
                        }
                    });
            },
        );
    }
    if out.is_some() {
        ui.memory_mut(|m| m.close_popup());
    }
    out
}


/// Zeichnet ein kleines Muster der Linienart (Breite 34 px).
fn lt_preview(ui: &mut Ui, lt: LineType, col: egui::Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(34.0, 12.0), Sense::hover());
    let y = r.center().y;
    let a = egui::pos2(r.left(), y);
    let b = egui::pos2(r.right(), y);
    let p = ui.painter();
    match crate::canvas::dash_pattern(lt, 3.2, false) {
        Some(pat) => {
            for seg in crate::canvas::dashed_segments(a, b, &pat) {
                p.line_segment(seg, Stroke::new(1.4, col));
            }
        }
        None => {
            p.line_segment([a, b], Stroke::new(1.4, col));
        }
    }
    ui.add_space(4.0);
}

/// Auswahl der Linienart mit Mustervorschau.
fn linetype_combo(ui: &mut Ui, salt: &str, cur: LineType, mixed: bool) -> Option<LineType> {
    let cap = if mixed {
        "*variiert*".to_string()
    } else {
        cur.label().to_string()
    };
    let icon = if mixed { None } else { Some(BtnIcon::Line(cur)) };
    dropdown(ui, salt, &cap, icon, |ui| {
        let mut out = None;
        for cand in LineType::ALL {
            let sel = !mixed && cand == cur;
            ui.horizontal(|ui| {
                lt_preview(ui, cand, if sel { Palette::ACCENT } else { Palette::TEXT_DIM });
                if ui.selectable_label(sel, cand.label()).clicked() {
                    out = Some(cand);
                }
            });
        }
        out
    })
}


/// Einheitlicher Auswahldropdown (gleiche Optik wie Farbe/Stärke/Linientyp).
pub(crate) fn pick_combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    salt: &str,
    cur: T,
    items: &[(T, String)],
) -> Option<T> {
    let cap = items
        .iter()
        .find(|(v, _)| *v == cur)
        .map(|(_, l)| l.clone())
        .unwrap_or_default();
    dropdown(ui, salt, &cap, None, |ui| {
        let mut out = None;
        for (v, l) in items {
            if ui.selectable_label(*v == cur, l).clicked() {
                out = Some(*v);
            }
        }
        out
    })
}

/// Auswahl einer Strichstärke: Vorgaben aus der Liste, freier Wert inline.
fn weight_combo(ui: &mut Ui, salt: &str, cur: f64, mixed: bool) -> Option<f64> {
    let eid = ui.make_persistent_id(("we", salt));
    let editing = ui.data(|d| d.get_temp::<bool>(eid)).unwrap_or(false);
    if editing {
        let mut out = None;
        let mut v = cur;
        ui.horizontal(|ui| {
            if ui
                .add(
                    crate::widgets::dv(&mut v)
                        .speed(0.01)
                        .range(0.05..=3.0)
                        .suffix(" mm"),
                )
                .changed()
            {
                out = Some(v);
            }
            if ui.small_button("Liste").on_hover_text("Vorgaben anzeigen").clicked() {
                ui.data_mut(|d| d.insert_temp(eid, false));
            }
        });
        return out;
    }
    let cap = if mixed {
        "*variiert*".to_string()
    } else {
        weight_text(cur)
    };
    dropdown(ui, salt, &cap, None, |ui| {
        let mut out = None;
        for (val, name) in WEIGHT_PRESETS {
            let txt = format!("{:.2} mm · {}", val, name).replace('.', ",");
            if ui.selectable_label((cur - val).abs() < 1e-6, txt).clicked() {
                out = Some(val);
            }
        }
        ui.separator();
        if ui.selectable_label(false, "Freier Wert …").clicked() {
            ui.data_mut(|d| d.insert_temp(eid, true));
            out = Some(cur);
        }
        out
    })
}

/// Auswahl einer Farbe: Vorgaben aus der Liste, freie Farbe inline.
///
/// `unset_label` benennt den Zustand "kein eigener Wert". Für Linien ist das
/// "vom Layer", für Wandflächen gibt es keine Layerfarbe — dort heißt der
/// Zustand "Standard" (Fläche hellt aus der Linienfarbe auf).
fn color_combo(
    ui: &mut Ui,
    salt: &str,
    cur: Color,
    unset: bool,
    mixed: bool,
    unset_label: &str,
) -> Option<Option<Color>> {
    let eid = ui.make_persistent_id(("ce", salt));
    let editing = ui.data(|d| d.get_temp::<bool>(eid)).unwrap_or(false);
    if editing {
        let mut out = None;
        let mut arr = [cur.r, cur.g, cur.b];
        ui.horizontal(|ui| {
            if ui.color_edit_button_srgb(&mut arr).changed() {
                out = Some(Some(Color::rgb(arr[0], arr[1], arr[2])));
            }
            let hex = format!("#{:02X}{:02X}{:02X}", arr[0], arr[1], arr[2]);
            ui.label(theme::mono(hex));
            if ui.small_button("Liste").on_hover_text("Vorgaben anzeigen").clicked() {
                ui.data_mut(|d| d.insert_temp(eid, false));
            }
        });
        return out;
    }
    let (cap, sw) = if mixed {
        ("*variiert*".to_string(), None)
    } else if unset {
        (unset_label.to_string(), None)
    } else {
        let hex = cur.to_hex();
        let name = COLOR_PRESETS
            .iter()
            .find(|(_, _, rgb)| preset_color(*rgb).to_hex() == hex)
            .map(|(n, m, _)| {
                if m.is_empty() {
                    n.to_string()
                } else {
                    format!("{} · {}", n, m)
                }
            })
            .unwrap_or_else(|| format!("eigene {}", hex));
        (name, Some(cur))
    };
    dropdown(ui, salt, &cap, sw.map(BtnIcon::Color), |ui| {
        let mut out: Option<Option<Color>> = None;
        if ui.selectable_label(unset, unset_label).clicked() {
            out = Some(None);
        }
        ui.separator();
        ui.columns(2, |cols| {
            for (i, (name, meaning, rgb)) in COLOR_PRESETS.iter().enumerate() {
                let cell = &mut cols[i % 2];
                let c = preset_color(*rgb);
                let sel = !unset && cur.to_hex() == c.to_hex();
                cell.horizontal(|ui| {
                    swatch(ui, c);
                    let txt = if meaning.is_empty() {
                        name.to_string()
                    } else {
                        format!("{} · {}", name, meaning)
                    };
                    if ui.selectable_label(sel, txt).clicked() {
                        out = Some(Some(c));
                    }
                });
            }
        });
        ui.separator();
        if ui.selectable_label(false, "Freie Farbe …").clicked() {
            ui.data_mut(|d| d.insert_temp(eid, true));
            out = Some(Some(cur));
        }
        out
    })
}

fn section_title(ui: &mut Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
        crate::icons::paint(ui.painter(), r, icon, Palette::ACCENT);
        ui.label(theme::bold(title, 12.5));
    });
    ui.add_space(4.0);
}

fn entity_props(app: &mut App, ui: &mut Ui, id: Id) {
    let Some(e) = app.doc().entity(id).cloned() else {
        return;
    };
    let layer_name = app
        .doc()
        .layer(e.layer)
        .map(|l| l.name.clone())
        .unwrap_or_default();
    section_title(ui, app::entity_kind_icon(&e.kind), e.kind.type_name());
    ui.label(theme::dim(format!("#{} · Layer {}", e.id, layer_name)));
    ui.add_space(6.0);

    // Layer
    let mut new_layer = e.layer;
    let layer_items: Vec<(Id, String)> = app
        .doc()
        .layers
        .iter()
        .map(|l| (l.id, l.name.clone()))
        .collect();
    widgets::field_row(ui, "Layer", |ui| {
        if let Some(l) = pick_combo(ui, "ent_layer", new_layer, &layer_items) {
            new_layer = l;
        }
    });
    if new_layer != e.layer {
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": [id], "layer": new_layer}));
        app.mark_changed();
    }

    // Farbe der Linie (bei Wand und Füllfläche: Umrandung bzw. Musterlinie)
    let ist_wand = matches!(
        e.kind,
        EntityKind::Wall { .. } | EntityKind::Fill { .. }
    );
    let color = e.style.color.unwrap_or(app.doc().effective_color(&e));
    let by_layer = e.style.color.is_none();
    let mut picked_col: Option<Option<Color>> = None;
    let color_label = if ist_wand { "Linienfarbe" } else { "Farbe" };
    widgets::field_row(ui, color_label, |ui| {
        picked_col = color_combo(ui, "ent_col", color, by_layer, false, "vom Layer");
    });
    if let Some(v) = picked_col {
        let cval = match v {
            None => "bylayer".to_string(),
            Some(c) => c.to_hex(),
        };
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": [id], "color": cval}));
        app.mark_changed();
    }

    // Linienstärke
    let lw = e
        .style
        .line_weight
        .unwrap_or_else(|| app.doc().effective_line_weight(&e));
    let mut picked_w: Option<f64> = None;
    widgets::field_row(ui, "Stärke", |ui| {
        picked_w = weight_combo(ui, "ent_w", lw, false);
    });
    if let Some(w) = picked_w {
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": [id], "lineweight": w}));
        app.mark_changed();
    }

    // Linientyp
    let lt = e.style.line_type.unwrap_or_else(|| app.doc().effective_line_type(&e));
    let mut picked_lt: Option<LineType> = None;
    widgets::field_row(ui, "Linientyp", |ui| {
        picked_lt = linetype_combo(ui, "ent_lt", lt, false);
    });
    if let Some(new_lt) = picked_lt {
        let name = match new_lt {
            LineType::Solid => "solid",
            LineType::Dashed => "dashed",
            LineType::Dotted => "dotted",
            LineType::DashDot => "dashdot",
            LineType::Hidden => "hidden",
        };
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": [id], "linetype": name}));
        app.mark_changed();
    }

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);

    // Geometrie (nur Anzeige)
    section_title(ui, "info", "Geometrie");
    let unit = app.unit();
    let len = entity_length(&e);
    if len > 0.0 {
        widgets::field_row(ui, "Länge", |ui| {
            ui.label(theme::mono(unit.format(len, unit.decimals())));
        });
    }
    if let jjcad_core::model::EntityKind::Circle { r, .. }
    | jjcad_core::model::EntityKind::Arc { r, .. } = &e.kind
    {
        widgets::field_row(ui, "Radius", |ui| {
            ui.label(theme::mono(unit.format(*r, unit.decimals())));
        });
    }
    let area = entity_area(&e);
    if area > 0.0 {
        widgets::field_row(ui, "Fläche", |ui| {
            ui.label(theme::mono(format!("{} m²", unit.format_area(area, 2))));
        });
    }
    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);

    // Typspezifische Parameter
    let unit = app.unit();
    let mut changed_params = serde_json::Map::new();
    match &e.kind {
        EntityKind::Wall {
            a,
            b,
            thickness,
            height,
            hatch,
            align,
            ..
        } => {
            let mut t = *thickness;
            let mut h = *height;
            widgets::field_row(ui, "Dicke", |ui| {
                if widgets::length_input(ui, &mut t, unit, 100.0) {
                    changed_params.insert("thickness".into(), json!(unit.from_mm(t)));
                }
            });
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_input(ui, &mut h, unit, 100.0) {
                    changed_params.insert("height".into(), json!(unit.from_mm(h)));
                }
            });
            // Ausrichtung des Wandkörpers zur Achse
            let align_items: Vec<(jjcad_core::style::WallAlign, String)> =
                jjcad_core::style::WallAlign::ALL
                    .iter()
                    .map(|c| (*c, c.label().to_string()))
                    .collect();
            let cur_align = *align;
            let mut picked_align: Option<jjcad_core::style::WallAlign> = None;
            widgets::field_row(ui, "Ausrichtung", |ui| {
                picked_align = pick_combo(ui, "ent_wall_align", cur_align, &align_items);
            });
            if let Some(wa) = picked_align {
                let name = match wa {
                    jjcad_core::style::WallAlign::Left => "left",
                    jjcad_core::style::WallAlign::Center => "center",
                    jjcad_core::style::WallAlign::Right => "right",
                };
                let _ = app
                    .session_mut()
                    .execute("modify.set", &json!({"ids": [id], "align": name}));
                app.mark_changed();
            }

            // Schraffur der Wandfläche
            let cur_hatch = hatch.unwrap_or(jjcad_core::style::FillPattern::Solid);
            let mut picked_hatch: Option<jjcad_core::style::FillPattern> = None;
            let hatch_items: Vec<(jjcad_core::style::FillPattern, String)> =
                jjcad_core::style::FillPattern::ALL
                    .iter()
                    .map(|c| (*c, c.label().to_string()))
                    .collect();
            widgets::field_row(ui, "Schraffur", |ui| {
                picked_hatch = pick_combo(ui, "ent_wall_hatch", cur_hatch, &hatch_items);
            });
            if let Some(hp) = picked_hatch {
                let name = match hp {
                    jjcad_core::style::FillPattern::Solid => "none",
                    jjcad_core::style::FillPattern::Lines => "lines",
                    jjcad_core::style::FillPattern::Cross => "cross",
                    jjcad_core::style::FillPattern::Dots => "dots",
                    jjcad_core::style::FillPattern::Insulation => "insulation",
                };
                let _ = app
                    .session_mut()
                    .execute("modify.set", &json!({"ids": [id], "hatch": name}));
                app.mark_changed();
            }

            // Füllfläche getrennt von der Umrandung
            let fill_set = e.style.fill.is_some();
            // Ohne eigene Füllfarbe zeigt der Farbwähler den tatsächlich
            // gezeichneten Standard: Linienfarbe, 74 % Richtung Weiß.
            let fill_cur = e.style.fill.unwrap_or_else(|| color.mix(Color::WHITE, 0.74));
            let mut picked_fill: Option<Option<Color>> = None;
            widgets::field_row(ui, "Füllfarbe", |ui| {
                picked_fill = color_combo(ui, "wall_fill", fill_cur, !fill_set, false, "Standard");
            });
            if let Some(v) = picked_fill {
                match v {
                    Some(c) => {
                        let _ = app.session_mut().execute(
                            "modify.set",
                            &json!({"ids": [id], "fill": c.to_hex()}),
                        );
                        app.mark_changed();
                    }
                    None => {
                        // "Standard" = Fläche wieder aus der Linienfarbe aufhellen
                        let _ = app.session_mut().execute(
                            "modify.set",
                            &json!({"ids": [id], "fill": "none"}),
                        );
                        app.mark_changed();
                    }
                }
            }
            ui.add_space(4.0);
            ui.label(theme::dim(format!(
                "Länge {} · Achse ({:.2}, {:.2}) → ({:.2}, {:.2})",
                unit.format(a.dist(*b), unit.decimals()),
                unit.from_mm(a.x),
                unit.from_mm(a.y),
                unit.from_mm(b.x),
                unit.from_mm(b.y)
            )));
        }
        // Füllfläche: dieselben Felder wie beim Platzieren (Muster, Winkel,
        // Abstand, Randlinie) plus die beiden Farben.
        EntityKind::Fill {
            boundary,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
        } => {
            let mut winkel = pattern_angle.to_degrees();
            let mut abstand = *pattern_scale;
            let mut rand = *border;
            let pat_items: Vec<(jjcad_core::style::FillPattern, String)> =
                jjcad_core::style::FillPattern::ALL
                    .iter()
                    .map(|c| (*c, c.label().to_string()))
                    .collect();
            widgets::field_row(ui, "Muster", |ui| {
                if let Some(v) = pick_combo(ui, "fill_pattern", *pattern, &pat_items) {
                    changed_params.insert("pattern".into(), json!(v.key()));
                }
            });
            widgets::field_row(ui, "Winkel", |ui| {
                if ui
                    .add(crate::widgets::dv(&mut winkel).speed(1.0).suffix("°"))
                    .changed()
                {
                    changed_params.insert("pattern_angle".into(), json!(winkel));
                }
            });
            widgets::field_row(ui, "Abstand", |ui| {
                if widgets::length_input(ui, &mut abstand, unit, 100.0) {
                    changed_params.insert("pattern_scale".into(), json!(unit.from_mm(abstand)));
                }
            });
            widgets::field_row(ui, "Randlinie", |ui| {
                if ui.checkbox(&mut rand, "").changed() {
                    changed_params.insert("border".into(), json!(rand));
                }
            });

            // Fläche getrennt von der Umrandung — wie bei der Wand.
            let fill_set = e.style.fill.is_some();
            let fill_cur = e.style.fill.unwrap_or_else(|| color.mix(Color::WHITE, 0.72));
            let mut picked_fill: Option<Option<Color>> = None;
            widgets::field_row(ui, "Füllfarbe", |ui| {
                picked_fill = color_combo(ui, "fill_area", fill_cur, !fill_set, false, "Standard");
            });
            if let Some(v) = picked_fill {
                let wert = match v {
                    Some(c) => json!(c.to_hex()),
                    None => json!("none"),
                };
                let _ = app
                    .session_mut()
                    .execute("modify.set", &json!({"ids": [id], "fill": wert}));
                app.mark_changed();
            }
            ui.add_space(4.0);
            ui.label(theme::dim(format!(
                "{} Punkte · Umriss {}",
                boundary.len(),
                unit.format(
                    boundary
                        .iter()
                        .zip(boundary.iter().cycle().skip(1))
                        .take(boundary.len())
                        .map(|(a, b)| a.dist(*b))
                        .sum::<f64>(),
                    unit.decimals()
                )
            )));
        }
        EntityKind::Door {
            width,
            kind,
            hinge,
            swing_in,
            center,
            angle,
            ..
        } => {
            let mut w = *width;
            let mut k = *kind;
            let mut hg = *hinge;
            let mut sw = *swing_in;
            widgets::field_row(ui, "Breite", |ui| {
                if widgets::length_input(ui, &mut w, unit, 100.0) {
                    changed_params.insert("width".into(), json!(unit.from_mm(w)));
                }
            });
            widgets::field_row(ui, "Typ", |ui| {
                let items: Vec<(DoorKind, String)> = DoorKind::ALL
                    .iter()
                    .map(|c| (*c, c.label().to_string()))
                    .collect();
                if let Some(v) = pick_combo(ui, "door_kind", k, &items) {
                    let name = match v {
                        DoorKind::Hinge => "hinge",
                        DoorKind::Sliding => "sliding",
                        DoorKind::Double => "double",
                        DoorKind::Opening => "opening",
                    };
                    changed_params.insert("kind".into(), json!(name));
                }
            });
            widgets::field_row(ui, "Anschlag", |ui| {
                if ui.selectable_value(&mut hg, 1.0, "rechts").clicked()
                    || ui.selectable_value(&mut hg, -1.0, "links").clicked()
                {
                    changed_params.insert("hinge".into(), json!(hg));
                }
            });
            widgets::field_row(ui, "Blatt", |ui| {
                if ui
                    .selectable_label(sw, "Seite A")
                    .on_hover_text("Blatt schlägt auf Seite A der Wand auf")
                    .clicked()
                {
                    sw = true;
                    changed_params.insert("swing_in".into(), json!(sw));
                }
                if ui
                    .selectable_label(!sw, "Seite B")
                    .on_hover_text("Blatt schlägt auf Seite B der Wand auf")
                    .clicked()
                {
                    sw = false;
                    changed_params.insert("swing_in".into(), json!(sw));
                }
            });
            ui.add_space(4.0);
            ui.label(theme::dim(format!(
                "Position ({:.2}, {:.2}) · Richtung {:.0}°",
                unit.from_mm(center.x),
                unit.from_mm(center.y),
                angle.to_degrees()
            )));
        }
        EntityKind::Window {
            width,
            sill,
            height,
            align,
            center,
            angle,
            ..
        } => {
            let mut w = *width;
            let mut s = *sill;
            let mut h = *height;
            let align_cur = *align;
            let align_items: Vec<(WallAlign, String)> = WallAlign::ALL
                .iter()
                .map(|a| (*a, a.label().to_string()))
                .collect();
            widgets::field_row(ui, "Breite", |ui| {
                if widgets::length_input(ui, &mut w, unit, 100.0) {
                    changed_params.insert("width".into(), json!(unit.from_mm(w)));
                }
            });
            widgets::field_row(ui, "Brüstung", |ui| {
                if widgets::length_input(ui, &mut s, unit, 100.0) {
                    changed_params.insert("sill".into(), json!(unit.from_mm(s)));
                }
            });
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_input(ui, &mut h, unit, 100.0) {
                    changed_params.insert("height".into(), json!(unit.from_mm(h)));
                }
            });
            // Lage des Fensters innerhalb der Wanddicke
            widgets::field_row(ui, "Ausrichtung", |ui| {
                if let Some(v) = pick_combo(ui, "win_align", align_cur, &align_items) {
                    changed_params.insert(
                        "align".into(),
                        json!(match v {
                            WallAlign::Left => "left",
                            WallAlign::Center => "center",
                            WallAlign::Right => "right",
                        }),
                    );
                }
            });
            ui.add_space(4.0);
            ui.label(theme::dim(format!(
                "Position ({:.2}, {:.2}) · Richtung {:.0}°",
                unit.from_mm(center.x),
                unit.from_mm(center.y),
                angle.to_degrees()
            )));
        }
        EntityKind::Stair {
            steps,
            width,
            direction,
            ..
        } => {
            let mut s = *steps;
            let mut w = *width;
            let mut d = *direction;
            widgets::field_row(ui, "Stufen", |ui| {
                if ui.add(crate::widgets::dv(&mut s).range(2..=80)).changed() {
                    changed_params.insert("steps".into(), json!(s));
                }
            });
            widgets::field_row(ui, "Breite", |ui| {
                if widgets::length_input(ui, &mut w, unit, 100.0) {
                    changed_params.insert("width".into(), json!(unit.from_mm(w)));
                }
            });
            widgets::field_row(ui, "Richtung", |ui| {
                let items: Vec<(StairDir, String)> = StairDir::ALL
                    .iter()
                    .map(|c| (*c, c.label().to_string()))
                    .collect();
                if let Some(v) = pick_combo(ui, "stair_dir", d, &items) {
                    changed_params.insert(
                        "direction".into(),
                        json!(match v {
                            StairDir::Up => "up",
                            StairDir::Down => "down",
                            StairDir::Both => "both",
                        }),
                    );
                }
            });
        }
        EntityKind::Text {
            text,
            height,
            angle,
            bold,
            ..
        } => {
            let mut t = text.clone();
            let mut h = *height;
            let mut a = angle.to_degrees();
            let mut b = *bold;
            ui.label(theme::dim("Text"));
            if ui
                .add_sized([ui.available_width(), 54.0], egui::TextEdit::multiline(&mut t).desired_rows(2))
                .changed()
            {
                changed_params.insert("text".into(), json!(t));
            }
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_input(ui, &mut h, unit, 100.0) {
                    changed_params.insert("height".into(), json!(unit.from_mm(h)));
                }
            });
            widgets::field_row(ui, "Drehung", |ui| {
                if ui.add(crate::widgets::dv(&mut a).speed(1.0).suffix("°")).changed() {
                    changed_params.insert("angle".into(), json!(a));
                }
            });
            widgets::field_row(ui, "Fett", |ui| {
                if ui.checkbox(&mut b, "").changed() {
                    changed_params.insert("bold".into(), json!(b));
                }
            });
            ui.add_space(2.0);
            ui.label(theme::dim("Platzhalter: {DATUM} {PROJEKT} {MASSTAB} {PLAN}"));
        }
        EntityKind::Circle { r, .. } => {
            let mut rr = *r;
            widgets::field_row(ui, "Radius", |ui| {
                if widgets::length_input(ui, &mut rr, unit, 110.0) {
                    changed_params.insert("r".into(), json!(unit.from_mm(rr)));
                }
            });
        }
        EntityKind::Dim {
            axis,
            override_text,
            a,
            b,
            ..
        } => {
            let mut ax = *axis;
            let mut txt = override_text.clone().unwrap_or_default();
            widgets::field_row(ui, "Richtung", |ui| {
                egui::ComboBox::from_id_salt(("da", id))
                    .width(ui.available_width())
                    .selected_text(ax.label())
                    .show_ui(ui, |ui| {
                        for cand in DimAxis::ALL {
                            if ui.selectable_value(&mut ax, cand, cand.label()).changed() {
                                changed_params.insert(
                                    "axis".into(),
                                    json!(match cand {
                                        DimAxis::X => "x",
                                        DimAxis::Y => "y",
                                        DimAxis::Aligned => "aligned",
                                    }),
                                );
                            }
                        }
                    });
            });
            widgets::field_row(ui, "Maßtext", |ui| {
                if ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut txt).hint_text("automatisch")).changed() {
                    changed_params.insert("text".into(), json!(txt));
                }
            });
            ui.add_space(4.0);
            let m = match axis {
                DimAxis::X => (b.x - a.x).abs(),
                DimAxis::Y => (b.y - a.y).abs(),
                DimAxis::Aligned => a.dist(*b),
            };
            ui.label(theme::dim(format!(
                "Maß: {} {}",
                format_number(app.doc().settings.dim_unit.from_mm(m), app.doc().dim_style.decimals),
                app.doc().settings.dim_unit.short()
            )));
        }
        EntityKind::Elevation { z, .. } => {
            let mut v = *z;
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_input(ui, &mut v, unit, 100.0) {
                    changed_params.insert("z".into(), json!(unit.from_mm(v)));
                }
            });
        }
        EntityKind::BlockRef {
            block,
            rot,
            scale,
            ..
        } => {
            let name = app
                .doc()
                .block(block)
                .map(|b| b.name.clone())
                .unwrap_or_else(|| block.clone());
            ui.label(theme::dim(format!("Block: {name}")));
            let mut r = rot.to_degrees();
            let mut s = scale.x;
            widgets::field_row(ui, "Drehung", |ui| {
                if ui.add(crate::widgets::dv(&mut r).speed(1.0).suffix("°")).changed() {
                    let _ = app
                        .session_mut()
                        .execute("modify.set", &json!({"ids": [id], "angle": r}));
                }
            });
            widgets::field_row(ui, "Faktor", |ui| {
                if ui.add(crate::widgets::dv(&mut s).speed(0.01)).changed() {
                    let _ = app.session_mut().execute(
                        "modify.set",
                        &json!({"ids": [id], "scale": s}),
                    );
                }
            });
            if ui.button("Blockdefinition bearbeiten").clicked() {
                app.ui.inventory_open = true;
                app.ui.inventory_tab = 1;
            }
        }
        EntityKind::Polyline { verts, closed } => {
            ui.label(theme::dim(format!(
                "{} Stützpunkte · {}",
                verts.len(),
                if *closed { "geschlossen" } else { "offen" }
            )));
            ui.label(theme::dim(format!("Länge {}", unit.format(
                Polyline { verts: verts.clone(), closed: *closed }.len(), unit.decimals()
            ))));
        }
        EntityKind::Line { a, b } => {
            ui.label(theme::dim(format!(
                "Länge {} · Winkel {:.1}°",
                unit.format(a.dist(*b), unit.decimals()),
                (*b - *a).angle().to_degrees()
            )));
        }
        EntityKind::Arc { r, a0, a1, .. } => {
            ui.label(theme::dim(format!(
                "Radius {} · Winkel {:.0}°",
                unit.format(*r, unit.decimals()),
                (a1 - a0).to_degrees().abs()
            )));
        }
        _ => {}
    }

    if !changed_params.is_empty() {
        let mut params = json!({"ids": [id]});
        if let Value::Object(map) = &mut params {
            for (k, v) in changed_params {
                map.insert(k, v);
            }
        }
        let _ = app.session_mut().execute("modify.set", &params);
        app.mark_changed();
    }

    ui.add_space(10.0);
    widgets::hline(ui);
    ui.add_space(8.0);
    let dup_unit = app.unit();
    ui.horizontal(|ui| {
        if ui.button("Duplizieren").clicked() {
            let dx = dup_unit.from_mm(300.0);
            let dy = dup_unit.from_mm(-300.0);
            let _ = app.session_mut().execute(
                "modify.copy",
                &json!({"ids": [id], "from": [0, 0], "to": [dx, dy]}),
            );
            app.mark_changed();
        }
        if ui
            .button(egui::RichText::new("Löschen").color(Palette::DANGER))
            .clicked()
        {
            let _ = app.session_mut().execute("modify.erase", &json!({"ids": [id]}));
            app.session_mut().clear_selection();
            app.mark_changed();
        }
    });
}

fn multi_props(app: &mut App, ui: &mut Ui, ids: &[Id]) {
    let ents: Vec<jjcad_core::model::Entity> = ids
        .iter()
        .filter_map(|i| app.doc().entity(*i).cloned())
        .collect();
    if ents.is_empty() {
        return;
    }
    section_title(ui, "select", "Mehrfachauswahl");
    ui.label(theme::dim(format!("{} Objekte", ents.len())));
    // Wände und Füllflächen tragen eine eigene Fläche — nur dort heißen die
    // beiden Farbeinstellungen "Linienfarbe" und "Füllfarbe".
    let alle_waende = ents.iter().all(|e| {
        matches!(
            e.kind,
            jjcad_core::model::EntityKind::Wall { .. } | jjcad_core::model::EntityKind::Fill { .. }
        )
    });
    ui.add_space(2.0);

    // ---- gemeinsame Felder ----
    // Layer
    let same_layer = ents.iter().all(|e| e.layer == ents[0].layer);
    let mut picked: Option<Id> = None;
    if same_layer {
        // gleicher Layer in allen Objekten: normaler Dropdown
        let layer_items: Vec<(Id, String)> = app
            .doc()
            .layers
            .iter()
            .map(|l| (l.id, l.name.clone()))
            .collect();
        let cur = ents[0].layer;
        widgets::field_row(ui, "Layer", |ui| {
            picked = pick_combo(ui, "multi_layer", cur, &layer_items);
        });
    } else {
        // verschiedene Layer: Feld zeigt *variiert*, Auswahl setzt alle
        let layer_items: Vec<(Id, String)> = app
            .doc()
            .layers
            .iter()
            .map(|l| (l.id, l.name.clone()))
            .collect();
        widgets::field_row(ui, "Layer", |ui| {
            picked = dropdown(ui, "multi_layer", "*variiert*", None, |ui| {
                let mut out = None;
                for (id, name) in &layer_items {
                    if ui.selectable_label(false, name).clicked() {
                        out = Some(*id);
                    }
                }
                out
            });
        });
    }
    if let Some(l) = picked {
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": ids, "layer": l}));
        app.mark_changed();
    }

    // Farbe
    let cols: Vec<Color> = ents.iter().map(|e| app.doc().effective_color(e)).collect();
    let same_col = cols.iter().all(|c| *c == cols[0]);
    let mut picked_col: Option<Option<Color>> = None;
    let multi_color_label = if alle_waende { "Linienfarbe" } else { "Farbe" };
    widgets::field_row(ui, multi_color_label, |ui| {
        picked_col = color_combo(ui, "multi_col", cols[0], false, !same_col, "vom Layer");
    });
    if let Some(Some(c)) = picked_col {
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": ids, "color": c.to_hex()}));
        app.mark_changed();
    }

    // Füllfarbe — nur wenn ausschließlich Wände gewählt sind; sonst wäre das
    // Feld für Linien und Punkte ohne Bedeutung.
    if alle_waende {
        let fills: Vec<Option<Color>> = ents.iter().map(|e| e.style.fill).collect();
        let same_fill = fills.iter().all(|f| *f == fills[0]);
        let fill_unset = same_fill && fills[0].is_none();
        let fill_cur = fills[0].unwrap_or_else(|| cols[0].mix(Color::WHITE, 0.74));
        let mut picked_fill: Option<Option<Color>> = None;
        widgets::field_row(ui, "Füllfarbe", |ui| {
            picked_fill = color_combo(ui, "multi_fill", fill_cur, fill_unset, !same_fill, "Standard");
        });
        if let Some(v) = picked_fill {
            let wert = match v {
                Some(c) => json!(c.to_hex()),
                None => json!("none"),
            };
            let _ = app
                .session_mut()
                .execute("modify.set", &json!({"ids": ids, "fill": wert}));
            app.mark_changed();
        }
    }

    // Linienstärke
    let wts: Vec<f64> = ents
        .iter()
        .map(|e| app.doc().effective_line_weight(e))
        .collect();
    let same_w = wts.iter().all(|w| (w - wts[0]).abs() < 1e-6);
    let mut new_w: Option<f64> = None;
    let lw = if same_w { wts[0] } else { 0.35 };
    widgets::field_row(ui, "Stärke", |ui| {
        new_w = weight_combo(ui, "multi_w", lw, !same_w);
    });
    if let Some(w) = new_w {
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": ids, "lineweight": w}));
        app.mark_changed();
    }

    // Linientyp
    let lts: Vec<LineType> = ents
        .iter()
        .map(|e| app.doc().effective_line_type(e))
        .collect();
    let same_lt = lts.iter().all(|t| *t == lts[0]);
    let mut new_lt: Option<LineType> = None;
    let lt = if same_lt { lts[0] } else { LineType::Solid };
    widgets::field_row(ui, "Linientyp", |ui| {
        new_lt = linetype_combo(ui, "multi_lt", lt, !same_lt);
    });
    if let Some(t) = new_lt {
        let name = match t {
            LineType::Solid => "solid",
            LineType::Dashed => "dashed",
            LineType::Dotted => "dotted",
            LineType::DashDot => "dashdot",
            LineType::Hidden => "hidden",
        };
        let _ = app
            .session_mut()
            .execute("modify.set", &json!({"ids": ids, "linetype": name}));
        app.mark_changed();
    }

    // ---- Auswertung ----
    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    let unit = app.unit();
    let (mut total_len, mut total_area) = (0.0f64, 0.0f64);
    for e in &ents {
        total_len += entity_length(e);
        total_area += entity_area(e);
    }
    if total_len > 0.0 {
        widgets::field_row(ui, "Gesamtlänge", |ui| {
            ui.label(theme::mono(unit.format(total_len, unit.decimals())));
        });
    }
    if total_area > 0.0 {
        widgets::field_row(ui, "Gesamtfläche", |ui| {
            ui.label(theme::mono(format!(
                "{} m²",
                unit.format_area(total_area, 2)
            )));
        });
    }

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Verschieben").clicked() {
            app.start_tool("modify.move");
        }
        if ui.button("Kopieren").clicked() {
            app.start_tool("modify.copy");
        }
        if ui.button("Drehen").clicked() {
            app.start_tool("modify.rotate");
        }
        if ui.button("Spiegeln").clicked() {
            app.start_tool("modify.mirror");
        }
        if ui
            .button(egui::RichText::new("Löschen").color(Palette::DANGER))
            .clicked()
        {
            let _ = app
                .session_mut()
                .execute("modify.erase", &json!({"ids": ids}));
            app.session_mut().clear_selection();
            app.mark_changed();
        }
    });
}

/// Länge eines Elements in mm (0, wenn nicht sinnvoll).
fn entity_length(e: &jjcad_core::model::Entity) -> f64 {
    use jjcad_core::model::EntityKind as K;
    match &e.kind {
        K::Line { a, b } | K::Wall { a, b, .. } => a.dist(*b),
        K::Circle { r, .. } => std::f64::consts::TAU * r,
        K::Arc { r, a0, a1, .. } => {
            let mut d = a1 - a0;
            while d < 0.0 {
                d += std::f64::consts::TAU;
            }
            d * r
        }
        K::Polyline { verts, closed } => {
            let mut len = 0.0;
            for i in 0..verts.len().saturating_sub(1) {
                len += verts[i].p.dist(verts[i + 1].p);
            }
            if *closed && verts.len() > 2 {
                len += verts[verts.len() - 1].p.dist(verts[0].p);
            }
            len
        }
        _ => 0.0,
    }
}

/// Fläche eines Elements in mm² (0, wenn nicht sinnvoll).
fn entity_area(e: &jjcad_core::model::Entity) -> f64 {
    use jjcad_core::model::EntityKind as K;
    match &e.kind {
        K::Circle { r, .. } => std::f64::consts::PI * r * r,
        K::Polyline { verts, closed } if *closed && verts.len() > 2 => {
            let mut s = 0.0;
            for i in 0..verts.len() {
                let a = verts[i].p;
                let b = verts[(i + 1) % verts.len()].p;
                s += a.x * b.y - b.x * a.y;
            }
            (s * 0.5).abs()
        }
        _ => 0.0,
    }
}

fn sheet_props(app: &mut App, ui: &mut Ui, sid: Id) {
    let Some(sheet) = app.doc().sheet(sid).cloned() else { return };
    section_title(ui, "sheet", &sheet.name);
    let (pw, ph) = sheet.size();
    ui.label(theme::dim(format!(
        "{} · {:.0} × {:.0} mm · {}",
        sheet.paper.label(),
        pw,
        ph,
        format_scale(sheet.scale)
    )));
    ui.add_space(8.0);

    let mut name = sheet.name.clone();
    widgets::field_row(ui, "Name", |ui| {
        if ui.add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut name)).changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "name": name}));
        }
    });
    let mut paper = sheet.paper;
    let mut landscape = sheet.landscape;
    let mut scale = sheet.scale;
    widgets::field_row(ui, "Format", |ui| {
        egui::ComboBox::from_id_salt("shpaper")
            .width(70.0)
            .selected_text(paper.label())
            .show_ui(ui, |ui| {
                for p in PaperSize::ALL {
                    ui.selectable_value(&mut paper, p, p.label());
                }
            });
        ui.checkbox(&mut landscape, "quer");
    });
    if paper != sheet.paper || landscape != sheet.landscape {
        let _ = app.session_mut().execute(
            "sheet.update",
            &json!({"sheet": sid, "paper": paper.label(), "landscape": landscape}),
        );
        app.mark_changed();
    }
    widgets::field_row(ui, "Maßstab", |ui| {
        if ui.add_sized([70.0, 20.0], crate::widgets::dv(&mut scale).speed(1.0).range(1.0..=1000.0)).changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "scale": scale}));
            app.mark_changed();
        }
    });
    let mut frame = sheet.frame;
    let mut tb = sheet.show_title_block;
    widgets::field_row(ui, "Rahmen", |ui| {
        if ui.checkbox(&mut frame, "Blattrahmen").changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "frame": frame}));
            app.mark_changed();
        }
    });
    widgets::field_row(ui, "Schriftfeld", |ui| {
        if ui.checkbox(&mut tb, "anzeigen").changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "show_title_block": tb}));
            app.mark_changed();
        }
    });

    ui.add_space(10.0);
    widgets::hline(ui);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(theme::bold("Ansichtsfenster", 12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, "plus", 20.0, false, "Ansichtsfenster hinzufügen").clicked() {
                app.run_action(ui.ctx(), "sheet.add_viewport");
            }
        });
    });
    ui.add_space(4.0);

    let selected_vp = app.tab().selected_viewport;
    for vp in &sheet.viewports {
        let sel = selected_vp == Some(vp.id);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
        if ui.is_rect_visible(rect) {
            if sel {
                ui.painter().rect_filled(rect, Rounding::same(4.0), Palette::ACCENT_SOFT);
            }
            ui.painter().text(
                Pos2::new(rect.left() + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!("{} · {}", vp.name, format_scale(vp.scale)),
                FontId::proportional(11.0),
                if sel { Palette::ACCENT } else { Palette::TEXT_DIM },
            );
        }
        if resp.clicked() {
            app.tab_mut().selected_viewport = Some(vp.id);
        }
    }
    if let Some(vid) = selected_vp {
        if let Some(vp) = sheet.viewports.iter().find(|v| v.id == vid) {
            ui.add_space(6.0);
            let mut vs = vp.scale;
            widgets::field_row(ui, "Ansicht", |ui| {
                if ui.add_sized([70.0, 20.0], crate::widgets::dv(&mut vs).speed(1.0).range(1.0..=1000.0)).changed() {
                    let _ = app.session_mut().execute(
                        "sheet.update_viewport",
                        &json!({"sheet": sid, "viewport": vid, "scale": vs}),
                    );
                    app.mark_changed();
                }
            });
            // Exakte Papierlage und Größe
            let mut vx = vp.x;
            let mut vy = vp.y;
            let mut vw = vp.w;
            let mut vh = vp.h;
            let mut geom_changed = false;
            widgets::field_row(ui, "X / Y (mm)", |ui| {
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::dv(&mut vx).speed(0.5).suffix("")).changed();
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::dv(&mut vy).speed(0.5).suffix("")).changed();
            });
            widgets::field_row(ui, "B / H (mm)", |ui| {
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::dv(&mut vw).speed(0.5).range(10.0..=2000.0)).changed();
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::dv(&mut vh).speed(0.5).range(10.0..=2000.0)).changed();
            });
            if geom_changed {
                let _ = app.session_mut().execute(
                    "sheet.update_viewport",
                    &json!({"sheet": sid, "viewport": vid, "x": vx, "y": vy, "w": vw, "h": vh}),
                );
                app.mark_changed();
            }
            ui.horizontal(|ui| {
                if ui.button("Auf Zeichnung zentrieren").clicked() {
                    let c = app.doc().bbox();
                    let u = app.unit();
                    if c.is_valid() {
                        let cx = u.from_mm(c.center().x);
                        let cy = u.from_mm(c.center().y);
                        let _ = app.session_mut().execute(
                            "sheet.update_viewport",
                            &json!({"sheet": sid, "viewport": vid, "center": [cx, cy]}),
                        );
                        app.mark_changed();
                    }
                }
                if ui.button("Entfernen").clicked() {
                    let _ = app.session_mut().execute(
                        "sheet.remove_viewport",
                        &json!({"sheet": sid, "viewport": vid}),
                    );
                    app.tab_mut().selected_viewport = None;
                    app.mark_changed();
                }
            });
            let mut show_frame = vp.show_frame;
            if ui.checkbox(&mut show_frame, "Fensterrahmen zeigen").changed() {
                let _ = app.session_mut().execute(
                    "sheet.update_viewport",
                    &json!({"sheet": sid, "viewport": vid, "show_frame": show_frame}),
                );
                app.mark_changed();
            }
        }
    }
}

fn doc_props(app: &mut App, ui: &mut Ui) {
    section_title(ui, "settings", "Zeichnung");
    let unit = app.doc().settings.input_unit;
    ui.label(theme::dim(format!(
        "{} · {} Objekte · Layer {}",
        app.doc().name,
        app.doc().entities.len(),
        app.doc().layers.len()
    )));
    ui.add_space(8.0);

    let mut input = app.doc().settings.input_unit;
    let mut dimu = app.doc().settings.dim_unit;
    let mut decimals = app.doc().settings.decimals as i32;
    let mut annot = app.doc().settings.annotation_scale;
    let mut grid = app.doc().settings.snap.grid_step;
    let mut text_h = app.doc().settings.text_height;
    let mut wall_t = app.doc().settings.wall_thickness;

    widgets::field_row(ui, "Einheit", |ui| {
        egui::ComboBox::from_id_salt("inpunit")
            .width(ui.available_width())
            .selected_text(input.label())
            .show_ui(ui, |ui| {
                for cand in Unit::ALL {
                    ui.selectable_value(&mut input, cand, cand.label());
                }
            });
    });
    widgets::field_row(ui, "Maß-Einheit", |ui| {
        egui::ComboBox::from_id_salt("dimpunit")
            .width(ui.available_width())
            .selected_text(dimu.label())
            .show_ui(ui, |ui| {
                for cand in Unit::ALL {
                    ui.selectable_value(&mut dimu, cand, cand.label());
                }
            });
    });
    widgets::field_row(ui, "Dezimalen", |ui| {
        ui.add_sized([ui.available_width(), 20.0], crate::widgets::dv(&mut decimals).range(0..=4));
    });
    widgets::field_row(ui, "Raster", |ui| {
        if widgets::length_input(ui, &mut grid, unit, ui.available_width()) {
            let _ = app
                .session_mut()
                .execute("doc.settings", &json!({"grid_step": unit.from_mm(grid)}));
            app.mark_changed();
        }
    });
    widgets::field_row(ui, "Maßstab", |ui| {
        ui.add_sized([ui.available_width(), 20.0], crate::widgets::dv(&mut annot).speed(1.0).range(1.0..=1000.0));
        ui.label(theme::dim(format_scale(annot)));
    });
    widgets::field_row(ui, "Texthöhe", |ui| {
        widgets::length_input(ui, &mut text_h, unit, ui.available_width());
    });
    widgets::field_row(ui, "Wandstärke", |ui| {
        widgets::length_input(ui, &mut wall_t, unit, ui.available_width());
    });
    if input != app.doc().settings.input_unit
        || dimu != app.doc().settings.dim_unit
        || decimals as usize != app.doc().settings.decimals
        || (annot - app.doc().settings.annotation_scale).abs() > 1e-9
        || (text_h - app.doc().settings.text_height).abs() > 1e-9
        || (wall_t - app.doc().settings.wall_thickness).abs() > 1e-9
    {
        let _ = app.session_mut().execute(
            "doc.settings",
            &json!({
                "input_unit": input.short(),
                "dim_unit": dimu.short(),
                "decimals": decimals,
                "annotation_scale": annot,
                "text_height": unit.from_mm(text_h),
                "wall_thickness": unit.from_mm(wall_t),
            }),
        );
        app.mark_changed();
    }

    ui.add_space(10.0);
    widgets::hline(ui);
    ui.add_space(6.0);
    section_title(ui, "info", "Projektdaten");
    let mut project = app.doc().project.clone();
    let mut changed = false;
    for (label, field) in [
        ("Projekt", &mut project.project),
        ("Bauherr", &mut project.client),
        ("Planung", &mut project.planner),
        ("Adresse", &mut project.address),
        ("Plan-Nr.", &mut project.plan_no),
        ("Planinhalt", &mut project.plan_title),
        ("Revision", &mut project.revision),
    ] {
        widgets::field_row(ui, label, |ui| {
            if ui
                .add_sized([ui.available_width(), 20.0], egui::TextEdit::singleline(field))
                .changed()
            {
                changed = true;
            }
        });
    }
    if changed {
        let _ = app.session_mut().execute(
            "doc.project",
            &json!({
                "project": project.project, "client": project.client,
                "planner": project.planner, "address": project.address,
                "plan_no": project.plan_no, "plan_title": project.plan_title,
                "revision": project.revision,
            }),
        );
        app.mark_changed();
    }

    ui.add_space(10.0);
    ui.label(theme::dim("Platzhalter in Texten: {DATUM} {PROJEKT} {BAUHERR} {PLAN} {MASSTAB} {ADRESSE}"));
}
