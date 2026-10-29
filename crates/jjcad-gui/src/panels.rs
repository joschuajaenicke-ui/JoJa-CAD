//! Seitenpanels: Layer (links) und Eigenschaften (rechts).

use crate::app::{self, App, Einfuegemodus, TabKind};
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::model::*;
use jjcad_core::style::{ArrowKind, Color, LineType, PaperSize, WallAlign};
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
    // Während ein Text in der Zeichnung getippt wird, gehört das Panel diesem
    // Text (T0028): Höhe und Ausrichtung wirken sofort auf die Vorschau.
    let text_eingabe = app.ui.texteingabe.is_some();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if text_eingabe {
                text_eingabe_props(app, ui);
                return;
            }
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

/// Eigenschaften des Textes, **während** er in der Zeichnung getippt wird.
///
/// Ohne diesen Abschnitt wäre das Panel beim Editieren leer (kein Werkzeug, keine
/// Auswahl) — Ausrichtung und Höhe ließen sich nicht mehr ändern, obwohl genau
/// sie den Einfügepunkt und die Größe bestimmen. Beide Werte rechnet die Eingabe
/// jeden Frame neu (`textedit::feld_lage`, Schriftgröße), sie wirken also sofort;
/// beim Übernehmen gehen sie in `draw.text` (neuer Text) bzw. `modify.set`.
fn text_eingabe_props(app: &mut App, ui: &mut Ui) {
    let einheit = app.unit();
    let (neu, hoehe, waagerecht, senkrecht) = {
        let Some(t) = app.ui.texteingabe.as_mut() else {
            return;
        };
        section_title(ui, "text", "Text");
        ui.label(theme::dim(if t.id.is_some() {
            "bearbeiteter Text"
        } else {
            "neuer Text"
        }));
        ui.add_space(6.0);
        let mut h = t.height;
        widgets::field_row(ui, "Höhe", |ui| {
            if widgets::length_field(ui, &mut h, einheit, ui.available_width(), widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                t.height = h;
                // Die Eingabe wurde in diesem Frame **davor** gezeichnet —
                // ohne Neuzeichnen bliebe die Vorschau in der alten Größe.
                ui.ctx().request_repaint();
            }
        });
        let mut ah = t.align;
        let mut av = t.valign;
        widgets::field_block(ui, "Ausrichtung", widgets::ALIGN_HOEHE, |ui| {
            if widgets::align_matrix(ui, "eingabe_text_ausrichtung", &mut ah, &mut av) {
                t.align = ah;
                t.valign = av;
                // Tastatur zurück ins Feld: der Klick lag im Panel, der Text
                // wird gleich weitergetippt.
                t.fokus = true;
                // Die Vorschau steht schon im Bild dieses Frames (die Eingabe
                // wird vor dem Panel gezeichnet) — sonst bliebe sie an der
                // alten Stelle stehen, bis der nächste Klick ein Bild auslöst.
                ui.ctx().request_repaint();
            }
        });
        ui.add_space(6.0);
        ui.label(theme::dim("Esc übernimmt · Enter neue Zeile · Rechtsklick bricht ab"));
        (t.id.is_none(), t.height, t.align, t.valign)
    };
    // Bei einem neuen Text bleiben die Werte für das nächste Element stehen.
    if neu {
        app.options.text_height = hoehe;
        app.options.text_align = waagerecht;
        app.options.text_valign = senkrecht;
    }
}

/// Eigenschaften für das nächste Element, während ein Werkzeug läuft.
fn pending_props(app: &mut App, ui: &mut Ui) {
    let tool_id = app.tool.as_ref().map(|t| t.action.clone()).unwrap_or_default();
    let is_wall = tool_id == "draw.wall";
    // Alle Füllwerkzeuge tragen dieselben Typparameter (Muster, Winkel,
    // Abstand, Umrandung) — Werte bleiben für das nächste Element stehen.
    // Die Umrandung merkt sich jeder Erstellmodus getrennt.
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

    // ---- Einfügemodus Rechteck/Ellipse ----
    // Rechteck und Ellipse lassen sich achsenparallel oder über eine Linie
    // beliebiger Lage einfügen (Basislinie + Höhe bzw. Mittelachse + Breite).
    if tool_id == "draw.rect" || tool_id == "draw.ellipse" {
        let cur = app.options.einfuegemodus(tool_id.as_str());
        let items: Vec<(Einfuegemodus, String)> = Einfuegemodus::ALL
            .iter()
            .map(|m| (*m, m.label(tool_id.as_str()).to_string()))
            .collect();
        let mut picked: Option<Einfuegemodus> = None;
        widgets::field_row(ui, "Einfügemodus", |ui| {
            picked = pick_combo(ui, "pend_einfuegemodus", cur, &items);
        });
        if let Some(m) = picked {
            *app.options.einfuegemodus_mut(tool_id.as_str()) = m;
        }
        if cur == Einfuegemodus::Basislinie {
            let hinweis = if tool_id == "draw.ellipse" {
                "Mittelachse klicken, dann die Breite"
            } else {
                "Basislinie klicken, dann die Höhe"
            };
            ui.add_space(2.0);
            ui.label(theme::faint(hinweis));
        }
    }

    // ---- Typparameter Wand (gleiche Reihenfolge wie im Objekt-Panel) ----
    if is_wall {
        let mut t = app.options.wall_thickness;
        widgets::field_row(ui, "Dicke", |ui| {
            if widgets::length_field(ui, &mut t, app.unit(), ui.available_width(), widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                app.options.wall_thickness = t;
            }
        });
        let mut h = app.options.wall_height;
        widgets::field_row(ui, "Höhe", |ui| {
            if widgets::length_field(ui, &mut h, app.unit(), ui.available_width(), widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
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
                .add(crate::widgets::zahl(&mut winkel, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°"))
                .changed()
            {
                app.options.fill_angle = winkel.to_radians();
            }
        });
        let mut abstand = app.options.fill_scale;
        widgets::field_row(ui, "Abstand", |ui| {
            if widgets::length_field(ui, &mut abstand, app.unit(), ui.available_width(), widgets::ABSTAND_MIN_MM, widgets::ABSTAND_MAX_MM) {
                app.options.fill_scale = abstand;
            }
        });
        // Umrandung: ein Wert je Erstellmodus — „Punkte setzen" an,
        // „Kontur anklicken" aus. Der Name folgt dem Glossar.
        let mut rand = app.options.fill_border_for(tool_id.as_str());
        widgets::field_row(ui, "Umrandung", |ui| {
            if ui.checkbox(&mut rand, "").changed() {
                *app.options.fill_border_for_mut(tool_id.as_str()) = rand;
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

    // ---- Tür: gleiche Zeilen wie im Objekt-Panel ----
    if tool_id == "draw.door" {
        let mut w = app.options.door_width;
        widgets::field_row(ui, "Breite", |ui| {
            if widgets::length_field(ui, &mut w, app.unit(), ui.available_width(), widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                app.options.door_width = w;
            }
        });
        let kind_items: Vec<(DoorKind, String)> = DoorKind::ALL
            .iter()
            .map(|c| (*c, c.label().to_string()))
            .collect();
        let mut kind = app.options.door_kind;
        widgets::field_row(ui, "Typ", |ui| {
            if let Some(k) = pick_combo(ui, "pend_door_kind", kind, &kind_items) {
                kind = k;
            }
        });
        app.options.door_kind = kind;
        widgets::field_row(ui, "Anschlag", |ui| {
            if ui.selectable_value(&mut app.options.hinge, 1.0, "rechts").clicked() {}
            if ui.selectable_value(&mut app.options.hinge, -1.0, "links").clicked() {}
        });
        widgets::field_row(ui, "Blatt", |ui| {
            let mut sw = app.options.swing_in;
            if ui.selectable_label(sw, "Seite A").clicked() {
                sw = true;
            }
            if ui.selectable_label(!sw, "Seite B").clicked() {
                sw = false;
            }
            app.options.swing_in = sw;
        });
        ui.add_space(2.0);
        ui.label(theme::faint("R dreht die Öffnung, F spiegelt sie"));
    }

    // ---- Fenster ----
    if tool_id == "draw.window" {
        let mut w = app.options.window_width;
        widgets::field_row(ui, "Breite", |ui| {
            if widgets::length_field(ui, &mut w, app.unit(), ui.available_width(), widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                app.options.window_width = w;
            }
        });
        let mut s = app.options.window_sill;
        widgets::field_row(ui, "Brüstung", |ui| {
            if widgets::length_field(ui, &mut s, app.unit(), ui.available_width(), 0.0, widgets::MASS_MAX_MM) {
                app.options.window_sill = s;
            }
        });
    }

    // ---- Treppe ----
    if tool_id == "draw.stair" {
        // Die Breite kommt aus dem dritten Klick (Abstand zur Lauflinie), hier
        // stehen Höhe, Stufenzahl und Laufrichtung.
        let mut h = app.options.stair_height;
        widgets::field_row(ui, "Höhe", |ui| {
            if widgets::length_field(ui, &mut h, app.unit(), ui.available_width(), 100.0, widgets::MASS_MAX_MM) {
                app.options.stair_height = h;
            }
        });
        let mut auto = app.options.stair_auto;
        widgets::field_row(ui, "Automatisch", |ui| {
            if ui
                .checkbox(&mut auto, "Schrittmaß")
                .on_hover_text("Stufenzahl aus Länge und Höhe nach 2 S + A = 63 cm")
                .changed()
            {
                app.options.stair_auto = auto;
            }
        });
        let mut st = app.options.stair_steps;
        widgets::field_row(ui, "Stufen", |ui| {
            ui.add_enabled_ui(!app.options.stair_auto, |ui| {
                if ui
                    .add(
                        crate::widgets::zahl(&mut st, 1.0, jjcad_core::stair::STUFEN_MIN..=jjcad_core::stair::STUFEN_MAX),
                    )
                    .changed()
                {
                    app.options.stair_steps = st;
                }
            });
        });
        ui.label(theme::faint("Dritter Klick setzt die Breite"));
    }

    // ---- Text ----
    if tool_id == "draw.text" {
        let mut h = app.options.text_height;
        widgets::field_row(ui, "Höhe", |ui| {
            if widgets::length_field(ui, &mut h, app.unit(), ui.available_width(), widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                app.options.text_height = h;
            }
        });
        // Ausrichtung des nächsten Textes: bestimmt zugleich den Einfügepunkt —
        // der Klickpunkt ist die Ecke/Kante/Mitte, die die Matrix zeigt.
        let mut ah = app.options.text_align;
        let mut av = app.options.text_valign;
        widgets::field_block(ui, "Ausrichtung", widgets::ALIGN_HOEHE, |ui| {
            if widgets::align_matrix(ui, "pend_text_ausrichtung", &mut ah, &mut av) {
                app.options.text_align = ah;
                app.options.text_valign = av;
            }
        });
        // Der Text selbst wird in der Zeichnung getippt (T0019), nicht hier:
        // ein Feld an dieser Stelle wäre ein zweiter, wirkungsloser Weg.
    }

    // ---- Bemaßung ----
    if tool_id == "dim.linear" {
        let axis_items: Vec<(Option<DimAxis>, String)> = std::iter::once((None, "Automatisch".to_string()))
            .chain(
                DimAxis::ALL
                    .iter()
                    .map(|a| (Some(*a), a.label().to_string())),
            )
            .collect();
        let mut cur = app.options.dim_axis;
        widgets::field_row(ui, "Richtung", |ui| {
            if let Some(a) = pick_combo(ui, "pend_dim_axis", cur, &axis_items) {
                cur = a;
            }
        });
        app.options.dim_axis = cur;
    }

    // ---- Farbeimer: eigenes Muster (startet mit Vollfläche) ----
    if tool_id == "draw.fill.paint" {
        let pat_items: Vec<(jjcad_core::style::FillPattern, String)> =
            jjcad_core::style::FillPattern::ALL
                .iter()
                .map(|c| (*c, c.label().to_string()))
                .collect();
        let mut p = app.options.paint_pattern;
        widgets::field_row(ui, "Muster", |ui| {
            if let Some(v) = pick_combo(ui, "pend_paint_pattern", p, &pat_items) {
                p = v;
            }
        });
        app.options.paint_pattern = p;
        let mut abstand = app.options.fill_scale;
        widgets::field_row(ui, "Abstand", |ui| {
            if widgets::length_field(ui, &mut abstand, app.unit(), ui.available_width(), widgets::ABSTAND_MIN_MM, widgets::ABSTAND_MAX_MM) {
                app.options.fill_scale = abstand;
            }
        });
        // Vorgabe aus — die Grenze des Eimers ist die Wand. Die Checkbox
        // schaltet die Umrandung für neue Flächen zu.
        let mut rand = app.options.fill_border_for(tool_id.as_str());
        widgets::field_row(ui, "Umrandung", |ui| {
            if ui.checkbox(&mut rand, "").changed() {
                *app.options.fill_border_for_mut(tool_id.as_str()) = rand;
            }
        });
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
                    crate::widgets::zahl(&mut v, 0.01, 0.05..=3.0)
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
    // Blattobjekte (Rahmen, Schriftfeld) liegen ohne Layer im Papierraum —
    // ein leerer Layer-Name sähe im Panel wie ein Fehler aus.
    let layer_name = app.doc().layer(e.layer).map(|l| l.name.clone()).unwrap_or_else(|| {
        if e.layer == 0 {
            "Blatt".to_string()
        } else {
            format!("#{}", e.layer)
        }
    });
    section_title(ui, app::entity_kind_icon(&e.kind), e.kind.type_name());
    ui.label(theme::dim(format!("#{} · Layer {}", e.id, layer_name)));
    ui.add_space(6.0);

    // Zeichenreihenfolge: der Listenplatz ist die Darstellungsposition.
    // Ein Platz kann nur einmal belegt sein — die Zahl lässt sich direkt
    // eintippen, das Element wandert dorthin und alles andere rückt auf.
    let pos = app
        .doc()
        .ents()
        .iter()
        .position(|x| x.id == id)
        .map(|i| i + 1)
        .unwrap_or(0);
    let total = app.doc().ents().len();
    let mut wunsch = pos as i32;
    widgets::field_row(ui, "Reihenfolge", |ui| {
        if ui
            .add(crate::widgets::zahl(&mut wunsch, 1.0, 1..=total.max(1) as i32))
            .changed()
        {
        }
        ui.label(theme::faint(format!("von {total}")));
    });
    if wunsch > 0 && wunsch as usize != pos {
        let _ = app
            .session_mut()
            .execute("modify.order", &json!({"ids": [id], "to": wunsch}));
        app.mark_changed();
    }

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

    // Geometrie (nur Anzeige) — eine Quelle für alle Typen
    section_title(ui, "info", "Geometrie");
    let unit = app.unit();
    for (name, wert) in geo_rows(&e, unit) {
        widgets::field_row(ui, name, |ui| {
            ui.label(theme::mono(wert.clone()));
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
                if widgets::length_field(ui, &mut t, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                    changed_params.insert("thickness".into(), json!(unit.from_mm(t)));
                }
            });
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_field(ui, &mut h, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
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
            // Körper-Kennzahl: die Wand ist ein Volumen (Länge × Dicke × Höhe).
            ui.label(theme::dim(format!(
                "Volumen {:.3} m³",
                a.dist(*b) * *thickness * *height / 1e9
            )));
        }
        // Füllfläche: dieselben Felder wie beim Platzieren (Muster, Winkel,
        // Abstand, Randlinie) plus die beiden Farben.
        EntityKind::Fill {
            boundary,
            holes,
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
                    .add(crate::widgets::zahl(&mut winkel, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°"))
                    .changed()
                {
                    changed_params.insert("pattern_angle".into(), json!(winkel));
                }
            });
            widgets::field_row(ui, "Abstand", |ui| {
                if widgets::length_field(ui, &mut abstand, unit, 100.0, widgets::ABSTAND_MIN_MM, widgets::ABSTAND_MAX_MM) {
                    changed_params.insert("pattern_scale".into(), json!(unit.from_mm(abstand)));
                }
            });
            widgets::field_row(ui, "Umrandung", |ui| {
                if ui.checkbox(&mut rand, "").changed() {
                    // Direkt in das Objekt: die Fläche wird im nächsten Frame
                    // ohne die Randlinie gezeichnet.
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
            // Umriss = alle Ringe; die Inselkanten werden mitgezeichnet.
            let umfang: f64 = std::iter::once(boundary)
                .chain(holes.iter())
                .map(|r| {
                    r.iter()
                        .zip(r.iter().cycle().skip(1))
                        .take(r.len())
                        .map(|(a, b)| a.dist(*b))
                        .sum::<f64>()
                })
                .sum();
            ui.label(theme::dim(format!(
                "{} Punkte · Umriss {}",
                boundary.len(),
                unit.format(umfang, unit.decimals())
            )));
            if !holes.is_empty() {
                ui.label(theme::dim(format!(
                    "{} Inseln · Fläche {}",
                    holes.len(),
                    area_text(unit, jjcad_core::area::area_of(boundary, holes))
                )));
            }
        }
        // Volumen: dieselben Felder wie die Füllfläche, zusätzlich die Höhe und
        // die daraus folgende Kennzahl (m³). Der Körper ist Mauerwerk — die
        // Fläche wird deshalb standardmäßig im Wandton gefüllt.
        EntityKind::Volume {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
            height,
        } => {
            let mut winkel = pattern_angle.to_degrees();
            let mut abstand = *pattern_scale;
            let mut rand = *border;
            let mut hoehe = *height;
            let pat_items: Vec<(jjcad_core::style::FillPattern, String)> =
                jjcad_core::style::FillPattern::ALL
                    .iter()
                    .map(|c| (*c, c.label().to_string()))
                    .collect();
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_field(ui, &mut hoehe, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                    changed_params.insert("height".into(), json!(unit.from_mm(hoehe)));
                }
            });
            widgets::field_row(ui, "Muster", |ui| {
                if let Some(v) = pick_combo(ui, "volume_pattern", *pattern, &pat_items) {
                    changed_params.insert("pattern".into(), json!(v.key()));
                }
            });
            widgets::field_row(ui, "Winkel", |ui| {
                if ui
                    .add(crate::widgets::zahl(&mut winkel, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°"))
                    .changed()
                {
                    changed_params.insert("pattern_angle".into(), json!(winkel));
                }
            });
            widgets::field_row(ui, "Abstand", |ui| {
                if widgets::length_field(ui, &mut abstand, unit, 100.0, widgets::ABSTAND_MIN_MM, widgets::ABSTAND_MAX_MM) {
                    changed_params.insert("pattern_scale".into(), json!(unit.from_mm(abstand)));
                }
            });
            widgets::field_row(ui, "Umrandung", |ui| {
                if ui.checkbox(&mut rand, "").changed() {
                    changed_params.insert("border".into(), json!(rand));
                }
            });

            // Füllfarbe: Standard ist der Wandton (74 % Richtung Weiß), damit
            // der Übergang zur Wand keine Farbkante zeigt.
            let fill_set = e.style.fill.is_some();
            let fill_cur = e
                .style
                .fill
                .unwrap_or_else(|| color.mix(Color::WHITE, jjcad_core::render::WALL_FILL_MIX));
            let mut picked_fill: Option<Option<Color>> = None;
            widgets::field_row(ui, "Füllfarbe", |ui| {
                picked_fill =
                    color_combo(ui, "volume_fill", fill_cur, !fill_set, false, "Wandton");
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
            let flaeche = jjcad_core::area::area_of(boundary, holes);
            ui.label(theme::dim(format!(
                "{} Punkte · Fläche {}",
                boundary.len(),
                area_text(unit, flaeche)
            )));
            if !holes.is_empty() {
                ui.label(theme::dim(format!("{} Inseln", holes.len())));
            }
            ui.label(theme::dim(format!(
                "Volumen {:.3} m³ (Höhe {})",
                flaeche * hoehe / 1e9,
                unit.format(hoehe, unit.decimals())
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
                if widgets::length_field(ui, &mut w, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
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
                if widgets::length_field(ui, &mut w, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                    changed_params.insert("width".into(), json!(unit.from_mm(w)));
                }
            });
            widgets::field_row(ui, "Brüstung", |ui| {
                if widgets::length_field(ui, &mut s, unit, 100.0, 0.0, widgets::MASS_MAX_MM) {
                    changed_params.insert("sill".into(), json!(unit.from_mm(s)));
                }
            });
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_field(ui, &mut h, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
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
            a,
            b,
            width,
            steps,
            height,
            auto_steps,
            cut,
            unten,
            oben,
            ..
        } => {
            let mut h = *height;
            let mut w = *width;
            let mut schnitt = *cut;
            let auto_jetzt = *auto_steps;
            let laenge = a.dist(*b);
            let mass = jjcad_core::stair::treppenmass(laenge, *height, *steps, auto_jetzt);

            // Die Treppe ist ein Rechteck: Länge und Breite sind seine Maße,
            // die Lauflinie liegt in der Mitte.
            let mut l = laenge;
            widgets::field_row(ui, "Länge", |ui| {
                if widgets::length_field(ui, &mut l, unit, 100.0, 100.0, widgets::MASS_MAX_MM) {
                    changed_params.insert("len".into(), json!(unit.from_mm(l)));
                }
            });
            widgets::field_row(ui, "Breite", |ui| {
                if widgets::length_field(ui, &mut w, unit, 100.0, 100.0, widgets::MASS_MAX_MM) {
                    changed_params.insert("width".into(), json!(unit.from_mm(w)));
                }
            });
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_field(ui, &mut h, unit, 100.0, 100.0, widgets::MASS_MAX_MM) {
                    changed_params.insert("height".into(), json!(unit.from_mm(h)));
                }
            });
            // Die berechnete Stufenzahl steht als Wert da; ein eigener Eintrag
            // schaltet die Regel ab und gilt dann fest.
            let mut st = mass.stufen;
            widgets::field_row(ui, "Stufen", |ui| {
                ui.add_enabled_ui(!auto_jetzt, |ui| {
                    if ui
                        .add(
                            crate::widgets::zahl(&mut st, 1.0, jjcad_core::stair::STUFEN_MIN..=jjcad_core::stair::STUFEN_MAX),
                        )
                        .changed()
                    {
                        changed_params.insert("steps".into(), json!(st));
                    }
                });
            });
            widgets::field_row(ui, "Automatisch", |ui| {
                let mut an = auto_jetzt;
                let antwort = ui
                    .checkbox(&mut an, "Schrittmaß")
                    .on_hover_text("Stufenzahl aus Länge und Höhe nach 2 S + A = 63 cm");
                if antwort.changed() {
                    if an {
                        changed_params.insert("auto_steps".into(), json!(true));
                    } else {
                        // Beim Abschalten die berechnete Zahl übernehmen, damit
                        // die Treppe nicht springt.
                        changed_params.insert("auto_steps".into(), json!(false));
                        changed_params.insert("steps".into(), json!(mass.stufen));
                    }
                }
            });
            // Treppenschnitt: die beiden Diagonalen. Er zeigt an, dass zwei
            // Läufe übereinander liegen — ohne ihn ist es eine einzelne Treppe.
            widgets::field_row(ui, "Schnitt", |ui| {
                let antwort = ui
                    .checkbox(&mut schnitt, "zwei Läufe")
                    .on_hover_text("Diagonalen in der Laufmitte; sie trennen oberen und unteren Teil");
                if antwort.changed() {
                    changed_params.insert("cut".into(), json!(schnitt));
                }
            });

            // Schrittmaß als Information — die Zahlen, die auch im Textfeld
            // der Treppe stehen.
            let cm = |mm: f64, stellen: usize| {
                format!("{:.*} cm", stellen, mm / 10.0).replace(".", ",")
            };
            ui.add_space(4.0);
            widgets::info_row(ui, "Auftritt", &cm(mass.auftritt, 1), None);
            widgets::info_row(ui, "Steigung", &cm(mass.steigung, 1), None);
            widgets::info_row(
                ui,
                "2 S + A",
                &cm(mass.regel(), 1),
                Some("Sollwert nach Schrittmaßregel: 63,0 cm"),
            );
            widgets::info_row(
                ui,
                "Auftrittsfläche",
                &unit.format_area(mass.auftrittsflaeche(*width), 2),
                Some("Auftritt × Treppenbreite"),
            );

            ui.add_space(6.0);
            if ui
                .add_sized(
                    [ui.available_width(), 24.0],
                    egui::Button::new("Beschriftung erzeugen"),
                )
                .on_hover_text("Stufenzahl und Schrittmaß als Textfeld neben die Treppe setzen")
                .clicked()
            {
                let _ = app
                    .session_mut()
                    .execute("draw.stair.label", &json!({"id": id}));
                app.mark_changed();
            }

            // Linien der beiden Teile — nur mit Schnitt gibt es oberen und
            // unteren Teil.
            if *cut {
                ui.add_space(6.0);
                widgets::hline(ui);
                ui.add_space(4.0);
                let basis = app
                    .doc()
                    .entity(id)
                    .map(|e| {
                        let c = e.style.color.unwrap_or_else(|| app.doc().effective_color(e));
                        let lw = e
                            .style
                            .line_weight
                            .unwrap_or_else(|| app.doc().effective_line_weight(e));
                        let lt = e
                            .style
                            .line_type
                            .unwrap_or_else(|| app.doc().effective_line_type(e));
                        (e.style.clone(), c, lw, lt)
                    })
                    .unwrap_or_else(|| (EntityStyle::default(), Color::BLACK, 0.25, LineType::Solid));
                let (basis_style, basis_farbe, basis_w, basis_lt) = basis;
                for (name, praefix, teil) in [("unten", "unten", unten), ("oben", "oben", oben)] {
                    let farbe = teil.as_ref().and_then(|t| t.color).or(basis_style.color);
                    let mut neue_farbe: Option<Option<Color>> = None;
                    widgets::field_row(ui, &format!("Farbe {name}"), |ui| {
                        neue_farbe = color_combo(
                            ui,
                            &format!("stair_col_{praefix}"),
                            farbe.unwrap_or(basis_farbe),
                            farbe.is_none(),
                            false,
                            "vom Layer",
                        );
                    });
                    if let Some(v) = neue_farbe {
                        changed_params.insert(
                            format!("color_{praefix}"),
                            json!(match v {
                                None => "bylayer".to_string(),
                                Some(c) => c.to_hex(),
                            }),
                        );
                    }
                    let lw = teil
                        .as_ref()
                        .and_then(|t| t.line_weight)
                        .or(basis_style.line_weight)
                        .unwrap_or(basis_w);
                    let mut picked_w: Option<f64> = None;
                    widgets::field_row(ui, &format!("Stärke {name}"), |ui| {
                        picked_w = weight_combo(ui, &format!("stair_w_{praefix}"), lw, false);
                    });
                    if let Some(v) = picked_w {
                        changed_params.insert(format!("weight_{praefix}"), json!(v));
                    }
                    let lt = teil
                        .as_ref()
                        .and_then(|t| t.line_type)
                        .or(basis_style.line_type)
                        .unwrap_or(basis_lt);
                    let mut picked_lt: Option<LineType> = None;
                    widgets::field_row(ui, &format!("Typ {name}"), |ui| {
                        picked_lt = linetype_combo(ui, &format!("stair_lt_{praefix}"), lt, false);
                    });
                    if let Some(v) = picked_lt {
                        let key = match v {
                            LineType::Solid => "solid",
                            LineType::Dashed => "dashed",
                            LineType::Dotted => "dotted",
                            LineType::DashDot => "dashdot",
                            LineType::Hidden => "hidden",
                        };
                        changed_params.insert(format!("type_{praefix}"), json!(key));
                    }
                    ui.add_space(4.0);
                }
            }

        }
        EntityKind::Text {
            text,
            height,
            angle,
            align,
            valign,
            bold,
            italic,
            underline,
            superscript,
            ..
        } => {
            let mut t = text.clone();
            let mut h = *height;
            let mut a = angle.to_degrees();
            let mut b = *bold;
            let mut it = *italic;
            let mut un = *underline;
            let mut ho = *superscript;
            let mut ah = *align;
            let mut av = *valign;
            ui.label(theme::dim("Text"));
            if ui
                .add_sized([ui.available_width(), 54.0], egui::TextEdit::multiline(&mut t).desired_rows(2))
                .changed()
            {
                changed_params.insert("text".into(), json!(t));
            }
            widgets::field_row(ui, "Höhe", |ui| {
                if widgets::length_field(ui, &mut h, unit, 100.0, widgets::MASS_MIN_MM, widgets::MASS_MAX_MM) {
                    changed_params.insert("height".into(), json!(unit.from_mm(h)));
                }
            });
            widgets::field_row(ui, "Drehung", |ui| {
                if ui.add(crate::widgets::zahl(&mut a, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°")).changed() {
                    changed_params.insert("angle".into(), json!(a));
                }
            });
            widgets::field_block(ui, "Ausrichtung", widgets::ALIGN_HOEHE, |ui| {
                if widgets::align_matrix(ui, "sel_text_ausrichtung", &mut ah, &mut av) {
                    changed_params.insert("align".into(), json!(ah.key()));
                    changed_params.insert("valign".into(), json!(av.key()));
                }
            });
            widgets::field_row(ui, "Fett", |ui| {
                if ui.checkbox(&mut b, "").changed() {
                    changed_params.insert("bold".into(), json!(b));
                }
            });
            widgets::field_row(ui, "Kursiv", |ui| {
                if ui.checkbox(&mut it, "").changed() {
                    changed_params.insert("italic".into(), json!(it));
                }
            });
            widgets::field_row(ui, "Unterstrichen", |ui| {
                if ui.checkbox(&mut un, "").changed() {
                    changed_params.insert("underline".into(), json!(un));
                }
            });
            widgets::field_row(ui, "Hochgestellt", |ui| {
                if ui.checkbox(&mut ho, "").changed() {
                    changed_params.insert("superscript".into(), json!(ho));
                }
            });
            ui.add_space(2.0);
            ui.label(theme::dim("Platzhalter: {DATUM} {PROJEKT} {MASSTAB} {PLAN}"));
        }
        // Kreis: der Radius ist hier einstellbar (T0022) — mit Untergrenze,
        // damit der Ziehpfeil nicht in negative Werte läuft.
        EntityKind::Circle { c, r } => {
            let mut rr = *r;
            widgets::field_row(ui, "Radius", |ui| {
                if widgets::length_field(ui, &mut rr, unit, 110.0, 0.1, widgets::MASS_MAX_MM) {
                    changed_params.insert("r".into(), json!(unit.from_mm(rr)));
                }
            });
            ui.add_space(4.0);
            ui.label(theme::dim(format!(
                "Mittelpunkt ({:.2}, {:.2})",
                unit.from_mm(c.x),
                unit.from_mm(c.y)
            )));
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
                if widgets::length_field(ui, &mut v, unit, 100.0, widgets::KOORD_MIN_MM, widgets::KOORD_MAX_MM) {
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
                if ui.add(crate::widgets::zahl(&mut r, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°")).changed() {
                    let _ = app
                        .session_mut()
                        .execute("modify.set", &json!({"ids": [id], "angle": r}));
                }
            });
            widgets::field_row(ui, "Faktor", |ui| {
                if ui.add(crate::widgets::zahl(&mut s, 0.01, 0.01..=1000.0)).changed() {
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
        // Polylinien tragen keine eigenen Eingabefelder: Länge, Fläche,
        // Stützpunktzahl und Geschlossenheit stehen schon im Abschnitt
        // „Geometrie“ (siehe `geo_rows`).
        EntityKind::Polyline { .. } | EntityKind::Spline { .. } => {}
        EntityKind::Line { a, b } => {
            ui.label(theme::dim(format!(
                "Länge {} · Winkel {:.1}°",
                unit.format(a.dist(*b), unit.decimals()),
                (*b - *a).angle().to_degrees()
            )));
        }
        // Bogen: Radius und beide Endwinkel sind einstellbar (T0022). Die
        // Enden selbst sitzen als runde Griffe auf dem Bogen — dort zieht man
        // sie mit der Maus.
        EntityKind::Arc { c, r, a0, a1 } => {
            let mut rr = *r;
            let mut w0 = a0.to_degrees();
            let mut w1 = a1.to_degrees();
            widgets::field_row(ui, "Radius", |ui| {
                if widgets::length_field(ui, &mut rr, unit, 110.0, 0.1, widgets::MASS_MAX_MM) {
                    changed_params.insert("r".into(), json!(unit.from_mm(rr)));
                }
            });
            widgets::field_row(ui, "Startwinkel", |ui| {
                if ui
                    .add(crate::widgets::zahl(&mut w0, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°"))
                    .changed()
                {
                    changed_params.insert("a0".into(), json!(w0));
                }
            });
            widgets::field_row(ui, "Endwinkel", |ui| {
                if ui
                    .add(crate::widgets::zahl(&mut w1, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX).suffix("°"))
                    .changed()
                {
                    changed_params.insert("a1".into(), json!(w1));
                }
            });
            ui.add_space(4.0);
            let start = *c + jjcad_core::geom::Vec2::polar(*a0, *r);
            let ende = *c + jjcad_core::geom::Vec2::polar(*a1, *r);
            ui.label(theme::dim(format!(
                "Mittelpunkt ({:.2}, {:.2}) · Start ({:.2}, {:.2}) · Ende ({:.2}, {:.2})",
                unit.from_mm(c.x),
                unit.from_mm(c.y),
                unit.from_mm(start.x),
                unit.from_mm(start.y),
                unit.from_mm(ende.x),
                unit.from_mm(ende.y)
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
            ui.label(theme::mono(area_text(unit, total_area)));
        });
    }

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Verschieben").clicked() {
            app.start_tool("modify.move");
        }
        if ui.button("Kopieren mit Basispunkt").clicked() {
            app.start_tool("modify.copy");
        }
        if ui.button("Drehen").clicked() {
            app.start_tool("modify.rotate");
        }
        if ui.button("Spiegeln").clicked() {
            app.start_tool("modify.mirror");
        }
    });
}

/// Anzeigezeilen des Abschnitts „Geometrie“ — für alle Typen dieselbe Quelle.
///
/// Der Wert ist reiner Text (Einheit schon enthalten); jede Zeile hat genau
/// einen Namen und genau einen Wert. Deshalb taucht z. B. „Länge“ bei einer
/// Polylinie nicht doppelt auf und die Flächeneinheit steht genau einmal im
/// Text (vorher kam „m²“ aus `format_area` und noch einmal aus dem Aufrufer).
fn geo_rows(e: &jjcad_core::model::Entity, unit: Unit) -> Vec<(&'static str, String)> {
    use jjcad_core::model::EntityKind as K;
    let mut rows: Vec<(&'static str, String)> = Vec::new();
    let len = entity_length(e);
    if len > 0.0 {
        rows.push(("Länge", unit.format(len, unit.decimals())));
    }
    let area = entity_area(e);
    if area > 0.0 {
        rows.push(("Fläche", area_text(unit, area)));
    }
    if let K::Polyline { verts, closed } | K::Spline { verts, closed } = &e.kind {
        rows.push(("Stützpunkte", verts.len().to_string()));
        rows.push((
            "Geschlossen",
            if *closed { "Ja" } else { "Nein" }.to_string(),
        ));
    }
    rows
}

/// Flächentext für die Anzeige: die Einheit kommt aus `format_area` und steht
/// damit genau einmal im Text.
fn area_text(unit: Unit, mm2: f64) -> String {
    unit.format_area(mm2, 2)
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
        K::Spline { verts, closed } => {
            let sp = jjcad_core::geom::Spline {
                verts: verts.clone(),
                closed: *closed,
            };
            sp.len()
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
        K::Spline { verts, closed } if *closed && verts.len() > 2 => {
            let sp = jjcad_core::geom::Spline {
                verts: verts.clone(),
                closed: *closed,
            };
            sp.area()
        }
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
        if ui.add_sized([70.0, 20.0], crate::widgets::zahl(&mut scale, 1.0, 1.0..=1000.0)).changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "scale": scale}));
            app.mark_changed();
        }
    });
    // Rahmen und Schriftfeld sind normale Blattobjekte (T0004): sie werden im
    // Papierraum angeklickt und geändert wie jede Linie und jeder Text. Ein
    // Schalter dafür wäre irreführend — hier steht deshalb nur der Hinweis.
    ui.label(theme::dim(
        "Rahmen und Schriftfeld sind normale Objekte — im Papierraum anklicken.",
    ));

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
                if ui.add_sized([70.0, 20.0], crate::widgets::zahl(&mut vs, 1.0, 1.0..=1000.0)).changed() {
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
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::zahl(&mut vx, 1.0, widgets::KOORD_MIN_MM..=widgets::KOORD_MAX_MM)).changed();
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::zahl(&mut vy, 1.0, widgets::KOORD_MIN_MM..=widgets::KOORD_MAX_MM)).changed();
            });
            widgets::field_row(ui, "B / H (mm)", |ui| {
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::zahl(&mut vw, 0.5, 10.0..=2000.0)).changed();
                geom_changed |= ui.add_sized([52.0, 20.0], crate::widgets::zahl(&mut vh, 0.5, 10.0..=2000.0)).changed();
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

/// Anzeigezeilen des Abschnitts „Zeichnung": alles, was die Zeichnungsdatei
/// selbst beschreibt — Datei, Stand, Größe, Ausdehnung und Blätter (der
/// Bestand nach Objektart steht als eigene Zeile darunter).
///
/// Reine Anzeige ohne Bedienelemente. Die Dateiangaben kommen aus der Datei
/// (`std::fs::metadata`), nicht aus einem mitgeführten Feld — so stimmen sie
/// auch nach einem Neustart und nach Änderungen von außen. Eine Quelle für
/// Panel und Test.
fn zeichnungs_info(
    doc: &Document,
    unit: Unit,
) -> Vec<(&'static str, String, Option<String>)> {
    let mut zeilen: Vec<(&'static str, String, Option<String>)> = Vec::new();
    match doc.path.as_deref() {
        Some(pfad) => {
            let name = std::path::Path::new(pfad)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| pfad.to_string());
            zeilen.push(("Datei", name, Some(pfad.to_string())));
            zeilen.push((
                "Stand",
                if doc.modified {
                    "geändert (nicht gespeichert)"
                } else {
                    "gespeichert"
                }
                .to_string(),
                None,
            ));
            let bytes = std::fs::metadata(pfad).map(|m| m.len()).unwrap_or(0);
            zeilen.push(("Größe", datei_groesse(bytes), None));
        }
        None => {
            zeilen.push(("Datei", "nicht gespeichert".to_string(), None));
            zeilen.push(("Stand", "–".to_string(), None));
            zeilen.push(("Größe", "–".to_string(), None));
        }
    }
    zeilen.push(("Ausdehnung", ausdehnung_text(doc, unit), None));
    zeilen.push(("Blätter", blatt_text(doc), None));
    zeilen
}

/// Dateigröße mit deutschem Komma („4,2 kB", „1,3 MB").
fn datei_groesse(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{} MB", format_number(bytes as f64 / (1024.0 * 1024.0), 1))
    } else if bytes >= 1024 {
        format!("{} kB", format_number(bytes as f64 / 1024.0, 1))
    } else {
        format!("{bytes} B")
    }
}

/// Ausdehnung des Modells (Breite × Höhe) in der Dokumenteinheit.
fn ausdehnung_text(doc: &Document, unit: Unit) -> String {
    let b = doc.bbox();
    if !b.is_valid() {
        return "leer".to_string();
    }
    let s = b.size();
    let d = unit.decimals();
    format!(
        "{} × {} {}",
        format_number(unit.from_mm(s.x), d),
        format_number(unit.from_mm(s.y), d),
        unit.short()
    )
}

/// Blätter der Zeichnung: „1 × A3 quer" bzw. „2 Blätter: A3 quer, A4 hoch".
fn blatt_text(doc: &Document) -> String {
    let beschreibung = |s: &Sheet| {
        format!(
            "{} {}",
            s.paper.label(),
            if s.landscape { "quer" } else { "hoch" }
        )
    };
    match doc.sheets.len() {
        0 => "keine".to_string(),
        1 => format!("1 × {}", beschreibung(&doc.sheets[0])),
        n => {
            let liste: Vec<String> = doc.sheets.iter().map(beschreibung).collect();
            format!("{n} Blätter: {}", liste.join(", "))
        }
    }
}

/// Bestand nach Objektart — nur Arten, die wirklich vorkommen.
fn bestand_text(doc: &Document) -> String {
    let (mut waende, mut tueren, mut fenster, mut treppen) = (0, 0, 0, 0);
    let (mut masse, mut texte, mut flaechen, mut bloecke) = (0, 0, 0, 0);
    for e in &doc.entities {
        match e.kind {
            EntityKind::Wall { .. } => waende += 1,
            EntityKind::Door { .. } => tueren += 1,
            EntityKind::Window { .. } => fenster += 1,
            EntityKind::Stair { .. } => treppen += 1,
            EntityKind::Dim { .. } | EntityKind::DimChain { .. } | EntityKind::Elevation { .. } => {
                masse += 1
            }
            EntityKind::Text { .. } => texte += 1,
            EntityKind::Fill { .. } => flaechen += 1,
            EntityKind::BlockRef { .. } => bloecke += 1,
            _ => {}
        }
    }
    // Einzahl/Mehrzahl getrennt: „1 Tür", aber „3 Türen".
    let teile: Vec<String> = [
        (waende, "Wand", "Wände"),
        (tueren, "Tür", "Türen"),
        (fenster, "Fenster", "Fenster"),
        (treppen, "Treppe", "Treppen"),
        (masse, "Maß", "Maße"),
        (texte, "Text", "Texte"),
        (flaechen, "Fläche", "Flächen"),
        (bloecke, "Block", "Blöcke"),
    ]
    .iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, ein, viel)| format!("{n} {}", if *n == 1 { ein } else { viel }))
    .collect();
    if teile.is_empty() {
        "keine Objekte".to_string()
    } else {
        teile.join(" · ")
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

    // Informationen über die Zeichnungsdatei selbst — reine Anzeige.
    let info = zeichnungs_info(app.doc(), unit);
    for (name, wert, tip) in &info {
        widgets::info_row(ui, name, wert, tip.as_deref());
    }
    // Der Bestand ist zu lang für die 150-px-Wertespalte — als umbrechende
    // Zeile darunter, damit nichts abgeschnitten wird.
    ui.add_space(4.0);
    ui.label(theme::dim(format!("Bestand: {}", bestand_text(app.doc()))));

    ui.add_space(10.0);
    widgets::hline(ui);
    ui.add_space(6.0);

    // Einheit, Dezimalen und Rasterweite gelten für die ganze Zeichnung —
    // Texthöhe und Wandstärke setzt man im Einstellungen-Dialog (Architektur),
    // nicht hier (T0011, Turn 2).
    let mut input = app.doc().settings.input_unit;
    let mut decimals = app.doc().settings.decimals as i32;
    let mut grid = app.doc().settings.snap.grid_step;

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
    widgets::field_row(ui, "Dezimalen", |ui| {
        ui.add_sized([ui.available_width(), 20.0], crate::widgets::zahl(&mut decimals, 1.0, 0..=4));
    });
    widgets::field_row(ui, "Raster", |ui| {
        // Grenzen: 1 mm … 10 m. Ohne sie zog der Ziehpfeil ins Negative und in
        // Werte, bei denen kein Rasterfang mehr stattfindet.
        if widgets::length_field(
            ui,
            &mut grid,
            unit,
            ui.available_width(),
            GRID_STEP_MIN_MM,
            GRID_STEP_MAX_MM,
        ) {
            let _ = app
                .session_mut()
                .execute("doc.settings", &json!({"grid_step": unit.from_mm(grid)}));
            app.mark_changed();
        }
    });
    if input != app.doc().settings.input_unit || decimals as usize != app.doc().settings.decimals {
        let _ = app.session_mut().execute(
            "doc.settings",
            &json!({
                "input_unit": input.short(),
                "decimals": decimals,
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

/// Maßketten: Maßeinheit, Textgröße, Maßstab und Bogensymbole.
///
/// Diese vier Werte standen früher im Ribbon und danach im Eigenschaften-Panel
/// des Modells. Letzteres zeigte sie an, obwohl dort „Informationen über die
/// Zeichnung" stehen — deshalb sitzen sie seit T0011 im Einstellungen-Dialog
/// (Abschnitt „Bemaßung") und an keiner anderen Stelle. Sie sind
/// Dokumenteinstellungen: `doc.settings.dim_unit`, `doc.settings.annotation_scale`
/// und `doc.dim_style.text_height` / `.arrow`. Jede Änderung geht als Action
/// (`doc.settings` bzw. `dim.style`) über die Befehlsschicht und landet damit
/// auch in Undo/Redo und in der gespeicherten Datei.
pub(crate) fn dim_fields(app: &mut App, ui: &mut Ui) {

    // Maßeinheit der Maßzahl (mm / cm / m)
    let cur_unit = app.doc().settings.dim_unit;
    let unit_items: Vec<(Unit, String)> = Unit::ALL
        .iter()
        .map(|u| (*u, u.label().to_string()))
        .collect();
    let mut picked_unit = None;
    widgets::field_row(ui, "Maßeinheit", |ui| {
        picked_unit = pick_combo(ui, "dim_unit", cur_unit, &unit_items);
    });
    if let Some(u) = picked_unit {
        let _ = app
            .session_mut()
            .execute("doc.settings", &json!({"dim_unit": u.short()}));
        app.mark_changed();
    }

    // Textgröße der Maßzahl in Papier-mm
    let mut h = app.doc().dim_style.text_height;
    widgets::field_row(ui, "Textgröße", |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 20.0],
                crate::widgets::zahl(&mut h, 0.1, 0.5..=20.0).suffix(" mm"),
            )
            .changed()
        {
            let _ = app
                .session_mut()
                .execute("dim.style", &json!({"text_height": h}));
            app.mark_changed();
        }
    });

    // Bezugsmaßstab (1:N) — alle Papiergrößen der Bemaßung hängen daran
    let mut s = app.doc().settings.annotation_scale;
    widgets::field_row(ui, "Maßstab", |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 20.0],
                crate::widgets::zahl(&mut s, 1.0, 1.0..=1000.0).prefix("1:"),
            )
            .changed()
        {
            let _ = app
                .session_mut()
                .execute("doc.settings", &json!({"annotation_scale": s}));
            app.mark_changed();
        }
    });

    // Bogensymbole an den Maßlinienenden. Im geschlossenen Feld steht die kurze
    // Form (die DIN-Angabe passt nicht in die Wertespalte), im Menü der volle Name.
    let cur_arrow = app.doc().dim_style.arrow;
    let mut picked_arrow = None;
    widgets::field_row(ui, "Bogensymbole", |ui| {
        picked_arrow = dropdown(ui, "dim_arrow", arrow_caption(cur_arrow), None, |ui| {
            let mut out = None;
            for cand in ArrowKind::ALL {
                if ui.selectable_label(cand == cur_arrow, cand.label()).clicked() {
                    out = Some(cand);
                }
            }
            out
        });
    });
    if let Some(a) = picked_arrow {
        let key = match a {
            ArrowKind::Arrow => "arrow",
            ArrowKind::Dot => "dot",
            ArrowKind::None => "none",
            ArrowKind::Slash => "slash",
        };
        let _ = app
            .session_mut()
            .execute("dim.style", &json!({"arrow": key}));
        app.mark_changed();
    }
}

/// Kurzform der Bogensymbole für das geschlossene Auswahlfeld.
fn arrow_caption(a: ArrowKind) -> &'static str {
    match a {
        ArrowKind::Slash => "Schrägstrich",
        ArrowKind::Arrow => "Pfeil",
        ArrowKind::Dot => "Punkt",
        ArrowKind::None => "Ohne",
    }
}

#[cfg(test)]
mod tests_geometrie_zeilen {
    //! Eigenschaften-Panel der Polylinien (T0003): „Länge“ steht genau einmal,
    //! die Flächeneinheit genau einmal, und Stützpunkte/Geschlossenheit sind
    //! eigene Zeilen im Schema Name | Wert.
    use super::*;
    use jjcad_core::geom::Vertex;

    fn poly(pts: &[(f64, f64)], closed: bool) -> jjcad_core::model::Entity {
        let verts = pts
            .iter()
            .map(|(x, y)| Vertex::new(jjcad_core::geom::Vec2::new(*x, *y)))
            .collect();
        jjcad_core::model::Entity::new(
            1,
            1,
            EntityKind::Polyline {
                verts,
                closed,
            },
        )
    }

    fn zeilen(e: &jjcad_core::model::Entity, unit: Unit) -> Vec<(String, String)> {
        geo_rows(e, unit)
            .into_iter()
            .map(|(n, w)| (n.to_string(), w))
            .collect()
    }

    #[test]
    fn offene_polylinie_hat_eine_laengenzeile_und_geschlossen_nein() {
        let e = poly(&[(0.0, 0.0), (1000.0, 0.0), (1000.0, 500.0)], false);
        let z = zeilen(&e, Unit::M);
        let namen: Vec<&str> = z.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(namen, vec!["Länge", "Stützpunkte", "Geschlossen"]);
        assert_eq!(z[1].1, "3");
        assert_eq!(z[2].1, "Nein");
        // 1000 mm + 500 mm
        assert_eq!(z[0].1, "1,50 m");
    }

    #[test]
    fn geschlossenes_quadrat_hat_flaeche_mit_einer_einheit() {
        let e = poly(
            &[
                (0.0, 0.0),
                (1000.0, 0.0),
                (1000.0, 1000.0),
                (0.0, 1000.0),
            ],
            true,
        );
        let z = zeilen(&e, Unit::M);
        let namen: Vec<&str> = z.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(namen, vec!["Länge", "Fläche", "Stützpunkte", "Geschlossen"]);
        assert_eq!(z[3].1, "Ja");
        assert_eq!(z[1].1, "1,00 m²");
        // Einheit genau einmal — „m² m²“ war der Fehler
        assert_eq!(z[1].1.matches("m²").count(), 1);
        assert!(!z[1].1.contains("m² m²"));
    }

    #[test]
    fn laenge_kommt_bei_keinem_typ_doppelt_vor() {
        let e = poly(&[(0.0, 0.0), (2000.0, 0.0)], false);
        let z = zeilen(&e, Unit::M);
        assert_eq!(z.iter().filter(|(n, _)| n == "Länge").count(), 1);
    }

    #[test]
    fn flaechentext_traegt_die_einheit_genau_einmal() {
        for u in Unit::ALL {
            let t = area_text(u, 2_500_000.0);
            assert_eq!(t.matches("²").count(), 1, "Einheit doppelt in {t}");
            assert!(t.ends_with('²'), "Einheit fehlt in {t}");
        }
    }
}


#[cfg(test)]
mod tests_zeichnungs_info {
    //! Eigenschaften-Panel ohne Auswahl (T0011): der Abschnitt „Zeichnung" zeigt
    //! Informationen über die Zeichnungsdatei selbst — Datei, Stand, Größe,
    //! Ausdehnung, Blätter, Bestand — und keine Maßketten-Werte mehr.
    use super::*;
    use jjcad_core::session::Session;
    use serde_json::json;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
        s.execute(
            "draw.wall",
            &json!({"a": [0, 0], "b": [4, 0], "thickness": 0.24}),
        )
        .unwrap();
        s.execute(
            "draw.wall",
            &json!({"a": [4, 0], "b": [4, 3], "thickness": 0.24}),
        )
        .unwrap();
        s.execute(
            "draw.text",
            &json!({"p": [1, 1], "text": "Wohnen", "height": 0.25}),
        )
        .unwrap();
        s.execute(
            "dim.chain",
            &json!({"pts": [[0, 0], [4, 0]], "line_pos": [0, -1.2], "axis": "x"}),
        )
        .unwrap();
        s
    }

    fn wert(zeilen: &[(&'static str, String, Option<String>)], name: &str) -> String {
        zeilen
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, w, _)| w.clone())
            .unwrap_or_else(|| panic!("Zeile {name} fehlt"))
    }

    #[test]
    fn ungespeichert_steht_die_dateizeile_auf_nicht_gespeichert() {
        let s = session();
        let z = zeichnungs_info(&s.doc, Unit::M);
        assert_eq!(wert(&z, "Datei"), "nicht gespeichert");
        assert_eq!(wert(&z, "Stand"), "–");
        assert_eq!(wert(&z, "Größe"), "–");
    }

    #[test]
    fn gespeicherte_datei_zeigt_name_groesse_und_stand() {
        let mut s = session();
        let dir = std::env::temp_dir().join("jjcad-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Zeichnungsinfo.jjcad");
        s.save(&path).unwrap();

        let z = zeichnungs_info(&s.doc, Unit::M);
        assert_eq!(wert(&z, "Datei"), "Zeichnungsinfo.jjcad");
        assert_eq!(wert(&z, "Stand"), "gespeichert");
        let groesse = wert(&z, "Größe");
        assert!(
            groesse.ends_with("kB") || groesse.ends_with("B"),
            "Größe ohne Einheit: {groesse}"
        );
        // Tooltip trägt den vollen Pfad (die Zelle ist zu schmal dafür).
        let tip = z
            .iter()
            .find(|(n, _, _)| *n == "Datei")
            .and_then(|(_, _, t)| t.clone())
            .unwrap_or_default();
        assert!(tip.ends_with("Zeichnungsinfo.jjcad"), "Tooltip: {tip}");
    }

    #[test]
    fn bestand_zaehlt_nach_objektart_mit_einzahl_und_mehrzahl() {
        let s = session();
        let t = bestand_text(&s.doc);
        assert!(t.contains("2 Wände"), "Bestand: {t}");
        assert!(t.contains("1 Text"), "Einzahl 1 Text fehlt: {t}");
        assert!(t.contains("1 Maß"), "Einzahl 1 Maß fehlt: {t}");
        // Arten ohne Objekte tauchen nicht auf.
        assert!(!t.contains("Türen") && !t.contains("0 "), "Bestand: {t}");
    }

    #[test]
    fn leere_zeichnung_hat_keine_objekte() {
        let doc = Document::new();
        assert_eq!(bestand_text(&doc), "keine Objekte");
        assert_eq!(ausdehnung_text(&doc, Unit::M), "leer");
    }

    #[test]
    fn ausdehnung_ist_die_bbox_in_dokumenteinheit() {
        let s = session();
        let b = s.doc.bbox();
        let erwartet = format!(
            "{} × {} m",
            jjcad_core::units::format_number(b.size().x / 1000.0, 2),
            jjcad_core::units::format_number(b.size().y / 1000.0, 2)
        );
        assert_eq!(ausdehnung_text(&s.doc, Unit::M), erwartet);
    }

    #[test]
    fn blattzeile_nennt_format_und_lage() {
        let doc = Document::new();
        assert_eq!(blatt_text(&doc), "1 × A3 quer");
    }

    #[test]
    fn dateigroesse_mit_deutschem_komma() {
        assert_eq!(datei_groesse(900), "900 B");
        assert_eq!(datei_groesse(4 * 1024), "4,0 kB");
        assert_eq!(datei_groesse(3 * 1024 * 1024), "3,0 MB");
    }
}

#[cfg(test)]
mod tests_text_eingabe_panel {
    //! Während ein Text in der Zeichnung getippt wird, zeigt das
    //! Eigenschaften-Panel **diesen Text** (T0028): Höhe und Ausrichtung wirken
    //! sofort auf die Vorschau. Vorher stand dort die Zeichen-Info, weil weder
    //! Werkzeug noch Auswahl existierten.

    use super::*;
    use crate::app::App;

    fn app_mm() -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app
    }

    /// Gezeichnete Beschriftungen eines Frames mit dem Eigenschaften-Panel.
    fn panel_texte(app: &mut App, zeit: f64) -> Vec<String> {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 950.0))),
            time: Some(zeit),
            predicted_dt: 0.016,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| properties(app, ui));
        });
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    fn enthaelt(texte: &[String], was: &str) -> bool {
        texte.iter().any(|t| t.trim() == was)
    }

    #[test]
    fn panel_zeigt_hoehe_und_ausrichtung_waehrend_der_eingabe() {
        let mut app = app_mm();
        let vorher = panel_texte(&mut app, 1.0);
        assert!(
            !enthaelt(&vorher, "Ausrichtung"),
            "ohne Eingabe steht keine Ausrichtung im Panel: {vorher:?}"
        );

        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        // Erster Frame richtet das Feld ein, zweiter zeichnet das Panel.
        let _ = panel_texte(&mut app, 1.1);
        let texte = panel_texte(&mut app, 1.2);
        assert!(
            enthaelt(&texte, "Ausrichtung"),
            "während der Eingabe fehlt die Ausrichtung: {texte:?}"
        );
        assert!(
            enthaelt(&texte, "Höhe"),
            "während der Eingabe fehlt die Höhe: {texte:?}"
        );
        assert!(
            enthaelt(&texte, "neuer Text"),
            "das Panel soll den bearbeiteten Text nennen: {texte:?}"
        );
    }

    /// Änderungen im Panel landen in der Eingabe — und damit im Ergebnis.
    #[test]
    fn aenderung_im_panel_gilt_fuer_die_eingabe() {
        let mut app = app_mm();
        app.options.text_height = 250.0;
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
            // So schreibt die Matrix im Panel (dieselben Felder).
            t.align = TextAlignH::Right;
            t.valign = TextAlignV::Top;
        }
        let _ = panel_texte(&mut app, 2.0);
        // Esc übernimmt: derselbe Weg wie beim Klick daneben.
        crate::textedit::uebernehmen_fuer_test(&mut app);
        let e = app.doc().ents().last().cloned().expect("Text fehlt");
        let EntityKind::Text { align, valign, .. } = &e.kind else {
            panic!("kein Text");
        };
        assert_eq!((*align, *valign), (TextAlignH::Right, TextAlignV::Top));
    }
}

/// Kein „Duplizieren“ und kein „Löschen“ im Eigenschaften-Panel (T0030).
///
/// Die beiden Schaltflächen standen bei Einzel- und Mehrfachauswahl unter den
/// Eigenschaften. Dupliziert wird über die Zwischenablage (Kopieren + Einfüge-
/// Vorschau), gelöscht über die Entf-Taste bzw. das Kontextmenü — im Panel
/// gehören die Werte des Objekts hin, keine Befehle.
mod tests_keine_befehle_im_panel {
    use super::*;
    use crate::app::App;

    /// Zeichnet zwei Wände und gibt den App-Zustand samt Ids zurück.
    fn app_mit_zwei_waenden() -> (App, Vec<Id>) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        for x in [0.0, 1000.0] {
            app.session_mut()
                .execute(
                    "draw.wall",
                    &json!({ "a": [x, 0.0], "b": [x, 3000.0], "thickness": 100.0, "height": 2500.0 }),
                )
                .expect("Wand anlegen");
        }
        let ids: Vec<Id> = app.doc().entities.iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), 2, "zwei Wände erwartet");
        (app, ids)
    }

    /// Gezeichnete Beschriftungen eines Frames mit dem Eigenschaften-Panel.
    fn panel_texte(app: &mut App, zeit: f64) -> Vec<String> {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 950.0))),
            time: Some(zeit),
            predicted_dt: 0.016,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| properties(app, ui));
        });
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    fn enthaelt(texte: &[String], was: &str) -> bool {
        texte.iter().any(|t| t.trim() == was)
    }

    #[test]
    fn einzelauswahl_hat_kein_duplizieren_und_kein_loeschen() {
        let (mut app, ids) = app_mit_zwei_waenden();
        app.session_mut().select(vec![ids[0]]);
        let texte = panel_texte(&mut app, 1.0);
        assert!(
            !enthaelt(&texte, "Duplizieren"),
            "„Duplizieren“ gehört nicht ins Panel: {texte:?}"
        );
        assert!(
            !enthaelt(&texte, "Löschen"),
            "„Löschen“ gehört nicht ins Panel: {texte:?}"
        );
        // Das Panel zeigt weiter die Eigenschaften der Wand.
        assert!(enthaelt(&texte, "Dicke"), "Werte fehlen: {texte:?}");
    }

    #[test]
    fn mehrfachauswahl_hat_kein_loeschen() {
        let (mut app, ids) = app_mit_zwei_waenden();
        app.session_mut().select(ids.clone());
        let texte = panel_texte(&mut app, 1.0);
        assert!(
            !enthaelt(&texte, "Löschen"),
            "„Löschen“ gehört nicht in die Mehrfachauswahl: {texte:?}"
        );
        // Die Werkzeugknöpfe für die Auswahl bleiben.
        assert!(enthaelt(&texte, "Verschieben"), "Verschieben fehlt: {texte:?}");
        assert!(enthaelt(&texte, "Drehen"), "Drehen fehlt: {texte:?}");
    }
}
