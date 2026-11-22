//! Seitenpanels: Layer, Eigenschaften und Blattmanager (T0218).

use crate::app::{self, App, TabKind};
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::model::*;
use jjcad_core::style::{ArrowKind, Color, LineType, PaperSize};
use jjcad_core::units::{format_number, format_scale, Unit};
use serde_json::json;

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------

pub fn layers(app: &mut App, ui: &mut Ui) {
    let tr = jjcad_core::i18n::tr;
    app.ui.last_layer_kopf.clear();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr("gui.panels.layer"))
                .size(10.0)
                .color(Palette::text_faint())
                .strong(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let plus =
                widgets::icon_button(ui, "plus", 22.0, false, tr("gui.panels.neuer_layer"));
            if plus.clicked() {
                // Neuer Layer im Plan des aktuellen Layers — jeder Plan hat
                // seine eigenen Layer (T0202).
                let plan = app.doc().aktueller_plan();
                let n = plan
                    .and_then(|p| app.doc().layers_of_plan(p).len().checked_add(1))
                    .unwrap_or(1);
                app.ui.new_layer_plan = plan;
                app.ui.new_layer_dialog = Some(format!("Layer {n}"));
            }
            // T0203: Der Pfeil zeigt zur Seite, auf der das Panel liegt —
            // eingeklappt wird dorthin, wo die Leiste steht.
            let pfeil = if app.panel_seite(crate::app::SeitenPanel::Layer)
                == jjcad_core::persist::PanelSeite::Links
            {
                "chevron_left"
            } else {
                "chevron_right"
            };
            // Das Einklappen ist die einzige Schließmöglichkeit im Kopf
            // (T0212: der frühere Knopf „Layer verwalten“ schloss das Panel
            // nur und ist entfallen).
            let einklappen =
                widgets::icon_button(ui, pfeil, 20.0, false, tr("gui.panels.einklappen"));
            if einklappen.clicked() {
                app.ui.layers_collapsed = true;
            }
            // Lage der Knöpfe im Bild, von links nach rechts — Prüfwert der
            // Tests (der Kopf trägt genau diese zwei).
            let mut kopf = vec![plus.rect, einklappen.rect];
            kopf.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
            app.ui.last_layer_kopf = kopf;
        });
    });
    ui.add_space(4.0);

    // ---- Dialoge: neuer Plan, neuer Layer, Umbenennen ----
    // Ein neuer Plan (T0202).
    if let Some(name) = app.ui.new_plan_dialog.clone() {
        let mut n = name;
        let mut fertig = false;
        let resp = ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut n).hint_text(tr("gui.panels.plan_name")),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(tr("gui.panels.abbrechen")).clicked() {
                    fertig = true;
                }
                if ui.button(tr("gui.panels.ok")).clicked() || enter {
                    match app
                        .session_mut()
                        .execute("plan.create", &json!({"name": n}))
                    {
                        Ok(res) => {
                            // Im neuen Plan weiterzeichnen: seine „Zeichnung“ wird
                            // die aktuelle Ebene (der Nutzer will dort arbeiten).
                            if let Some(pid) = res.get("id").and_then(|v| v.as_u64()) {
                                if let Some(l) = app.doc().layers_of_plan(pid).first().map(|l| l.id) {
                                    let _ = app
                                        .session_mut()
                                        .execute("layer.set_current", &json!({"layer": l}));
                                }
                            }
                            app.mark_changed();
                            app.toast(jjcad_core::i18n::t_args_owned(
                                "gui.panels.plan_x_angelegt",
                                &[("n", format!("{n}"))],
                            ));
                            fertig = true;
                        }
                        Err(e) => app.toast(e),
                    }
                }
            });
        });
        app.ui.new_plan_dialog = if fertig { None } else { Some(n) };
        ui.add_space(4.0);
    }
    // Ein neuer Layer (im gewählten Plan).
    if let Some(name) = app.ui.new_layer_dialog.clone() {
        let mut n = name;
        let mut fertig = false;
        let resp = ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut n).hint_text(tr("gui.panels.layer_name")),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(tr("gui.panels.abbrechen")).clicked() {
                    fertig = true;
                }
                if ui.button(tr("gui.panels.ok")).clicked() || enter {
                    let mut params = json!({"name": n});
                    if let Some(p) = app.ui.new_layer_plan {
                        params["plan"] = json!(p);
                    }
                    match app.session_mut().execute("layer.create", &params) {
                        Ok(_) => {
                            app.mark_changed();
                            app.toast(jjcad_core::i18n::t_args_owned(
                                "gui.panels.layer_x_angelegt",
                                &[("n", format!("{n}"))],
                            ));
                            fertig = true;
                        }
                        Err(e) => app.toast(e),
                    }
                }
            });
        });
        if fertig {
            app.ui.new_layer_dialog = None;
            app.ui.new_layer_plan = None;
        } else {
            app.ui.new_layer_dialog = Some(n);
        }
        ui.add_space(4.0);
    }
    // Umbenennen eines Layers läuft **in der Layerzeile** (T0206) — kein
    // zweites Eingabefeld über der Liste, das die Zeile verschiebt und das
    // Panel verbreitert.
    if let Some((id, name)) = app.ui.rename_plan.clone() {
        let mut n = name;
        let mut fertig = false;
        // Feld über die volle Panelbreite, Knöpfe darunter (T0206): eine
        // Zeile aus Feld und zwei Knöpfen war breiter als das Panel und trieb
        // es auf — links blieb ein dunkler Balken stehen.
        let resp = ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut n).hint_text(tr("gui.panels.plan_name")),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(tr("gui.panels.abbrechen")).clicked() {
                    fertig = true;
                }
                if ui.button(tr("gui.panels.ok")).clicked() || enter {
                    let _ = app
                        .session_mut()
                        .execute("plan.update", &json!({"plan": id, "name": n}));
                    app.mark_changed();
                    fertig = true;
                }
            });
        });
        app.ui.rename_plan = if fertig { None } else { Some((id, n)) };
        ui.add_space(4.0);
    }

    let current = app.doc().current_layer;
    let plans = app.doc().plans.clone();
    let mut befehle: Vec<(String, serde_json::Value)> = Vec::new();
    let mut set_current: Option<Id> = None;
    let mut new_layer_fuer: Option<Id> = None;

    egui::ScrollArea::vertical()
        // T0222: eigener Scrollzustand je Panel. Ohne eigenes `id_salt` bilden
        // alle Scrollflächen im selben Elternteil dieselbe Kennung
        // (`Id::new("scroll_area")` auf der stabilen Ui-Kennung) — dann bewegt
        // ein Mausrad über einem Panel auch das andere daneben.
        .id_salt("panel_layer")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for plan in &plans {
                // ---------------- Plan-Kopfzeile ----------------
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 28.0),
                    Sense::click(),
                );
                if ui.is_rect_visible(rect) {
                    let p = ui.painter();
                    // Eine ausgeblendete oder gesperrte Planzeile bleibt
                    // sichtbar, aber gedämpft — sie ist der Schalter dafür.
                    let farbe = if !plan.visible {
                        Palette::text_faint()
                    } else if plan.locked {
                        Palette::text_dim()
                    } else {
                        Palette::text()
                    };
                    // Einklapp-Dreieck
                    let dreieck = Rect::from_center_size(
                        Pos2::new(rect.left() + 9.0, rect.center().y),
                        Vec2::splat(14.0),
                    );
                    crate::icons::paint(
                        p,
                        dreieck,
                        if plan.collapsed {
                            "chevron_right"
                        } else {
                            "chevron_down"
                        },
                        Palette::text_dim(),
                    );
                    p.text(
                        Pos2::new(rect.left() + 20.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &plan.name,
                        FontId::proportional(12.0),
                        farbe,
                    );
                    // Maßstab des Plans (T0232): er gehört zu jedem Plan und
                    // steuert alle Papiergrößen seiner Objekte.
                    p.text(
                        Pos2::new(rect.right() - 58.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        format_scale(plan.scale),
                        FontId::monospace(10.0),
                        Palette::text_dim(),
                    );
                    // Auge und Schloss des ganzen Plans
                    let eye = Rect::from_center_size(
                        Pos2::new(rect.right() - 40.0, rect.center().y),
                        Vec2::splat(20.0),
                    );
                    crate::icons::paint(
                        p,
                        eye.shrink(3.0),
                        if plan.visible { "eye" } else { "eye_off" },
                        if plan.visible {
                            Palette::text_dim()
                        } else {
                            Palette::text_faint()
                        },
                    );
                    let lock = Rect::from_center_size(
                        Pos2::new(rect.right() - 19.0, rect.center().y),
                        Vec2::splat(20.0),
                    );
                    crate::icons::paint(
                        p,
                        lock.shrink(4.0),
                        if plan.locked { "lock" } else { "unlock" },
                        if plan.locked {
                            Palette::accent()
                        } else {
                            Palette::text_faint()
                        },
                    );
                }
                // Ein Doppelklick auf den **Namen** benennt um; auf Dreieck,
                // Auge oder Schloss zählt jeder Klick einzeln (sonst öffnete
                // ein zweiter Klick auf das Dreieck das Umbenennen-Feld).
                let pos = resp.interact_pointer_pos().unwrap_or(rect.center());
                let schalter = pos.x < rect.left() + 18.0 || pos.x > rect.right() - 52.0;
                if resp.double_clicked() && !schalter {
                    app.ui.rename_plan = Some((plan.id, plan.name.clone()));
                } else if resp.clicked() {
                    if pos.x > rect.right() - 30.0 {
                        befehle.push((
                            "plan.update".into(),
                            json!({"plan": plan.id, "locked": !plan.locked}),
                        ));
                    } else if pos.x > rect.right() - 52.0 {
                        befehle.push((
                            "plan.update".into(),
                            json!({"plan": plan.id, "visible": !plan.visible}),
                        ));
                    } else if pos.x < rect.left() + 18.0 {
                        // Einklappen — reine Anzeige.
                        befehle.push((
                            "plan.update".into(),
                            json!({"plan": plan.id, "collapsed": !plan.collapsed}),
                        ));
                    } else if let Some(l) = app.doc().layers_of_plan(plan.id).first() {
                        // Klick auf den Namen: in diesem Plan zeichnen.
                        set_current = Some(l.id);
                    }
                }
                resp.clone().on_hover_text(jjcad_core::i18n::t_args_owned(
                    "gui.panels.plan_tip",
                    &[("n", format!("{}", app.doc().layers_of_plan(plan.id).len()))],
                ));
                resp.context_menu(|ui| {
                    ui.label(theme::dim(&plan.name));
                    ui.separator();
                    if ui.button(tr("gui.panels.umbenennen")).clicked() {
                        app.ui.rename_plan = Some((plan.id, plan.name.clone()));
                        ui.close_menu();
                    }
                    if ui
                        .button(if plan.visible {
                            tr("gui.panels.plan_ausblenden")
                        } else {
                            tr("gui.panels.plan_einblenden")
                        })
                        .clicked()
                    {
                        befehle.push((
                            "plan.update".into(),
                            json!({"plan": plan.id, "visible": !plan.visible}),
                        ));
                        ui.close_menu();
                    }
                    if ui
                        .button(if plan.locked {
                            tr("gui.panels.plan_entsperren")
                        } else {
                            tr("gui.panels.plan_sperren")
                        })
                        .clicked()
                    {
                        befehle.push((
                            "plan.update".into(),
                            json!({"plan": plan.id, "locked": !plan.locked}),
                        ));
                        ui.close_menu();
                    }
                    ui.separator();
                    // Maßstab des Plans (T0232) — 1:N. Er bestimmt die
                    // Papiergrößen aller Objekte dieses Plans (Strichstärke im
                    // Modus „Papier (maßstabsgetreu)“, Text- und
                    // Bemaßungshöhen, Punktsymbole).
                    let mut skala = plan.scale;
                    ui.horizontal(|ui| {
                        ui.label(tr("gui.panels.massstab"));
                        if ui
                            .add(crate::widgets::zahl(&mut skala, 1.0, 1.0..=1000.0).prefix("1:"))
                            .changed()
                        {
                            befehle.push((
                                "plan.update".into(),
                                json!({"plan": plan.id, "scale": skala}),
                            ));
                        }
                    });
                });

                if plan.collapsed {
                    continue;
                }

                // ---------------- Layer des Plans ----------------
                // Kopie der Layer: darunter wird `app.ui` verändert (Dialoge,
                // Farbwähler) — ein laufender Leih auf das Dokument ginge nicht.
                let plan_layer: Vec<Layer> = app
                    .doc()
                    .layers_of_plan(plan.id)
                    .into_iter()
                    .cloned()
                    .collect();
                for l in plan_layer {
                    let is_current = l.id == current;
                    // Wird dieser Layer gerade umbenannt? Dann steht das
                    // Eingabefeld **in** der Zeile (T0206).
                    let umbenennen =
                        app.ui.rename_layer.as_ref().map(|(id, _)| *id) == Some(l.id);
                    // Eingerückt unter dem Plan; die Sichtbarkeit folgt auch
                    // dem Plan (T0202).
                    let sichtbar = l.visible && plan.visible;
                    let gesperrt = l.locked || plan.locked;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 27.0),
                        Sense::click(),
                    );
                    if ui.is_rect_visible(rect) {
                        let p = ui.painter();
                        if is_current {
                            p.rect_filled(rect, Rounding::same(6.0), Palette::panel_alt());
                            p.rect_filled(
                                Rect::from_min_size(
                                    rect.left_top(),
                                    Vec2::new(2.0, rect.height()),
                                ),
                                Rounding::same(1.0),
                                Palette::accent(),
                            );
                        } else if resp.hovered() {
                            p.rect_filled(
                                rect,
                                Rounding::same(6.0),
                                Palette::panel_alt().gamma_multiply(0.6),
                            );
                        }
                        // Farbpunkt in der Ebenenfarbe — dasselbe Bild wie vor
                        // der Überarbeitung, nur um die Einrückung unter dem
                        // Plan verschoben. Kein heller Grund: der wirkte als
                        // breiter weißer Rand (T0202, Turn 5).
                        let dot = Pos2::new(rect.left() + 27.0, rect.center().y);
                        p.circle_filled(dot, 5.0, app::color_to_egui(l.color));
                        p.circle_stroke(dot, 5.0, Stroke::new(1.0, farbpunkt_ring(l.color)));
                        // Name — beim Umbenennen steht hier das Eingabefeld
                        // (unten, nach dem Maler), nicht der gemalte Text.
                        if !umbenennen {
                            let max_w = rect.width() - 106.0;
                            let mut name = l.name.clone();
                            while p
                                .layout_no_wrap(
                                    name.clone(),
                                    FontId::proportional(12.0),
                                    Palette::text(),
                                )
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
                                Pos2::new(rect.left() + 39.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                name,
                                FontId::proportional(12.0),
                                if is_current {
                                    Palette::text()
                                } else if sichtbar {
                                    Palette::text_dim()
                                } else {
                                    Palette::text_faint()
                                },
                            );
                        }
                        let eye = Rect::from_center_size(
                            Pos2::new(rect.right() - 40.0, rect.center().y),
                            Vec2::splat(20.0),
                        );
                        crate::icons::paint(
                            p,
                            eye.shrink(3.0),
                            if sichtbar { "eye" } else { "eye_off" },
                            if sichtbar {
                                Palette::text_dim()
                            } else {
                                Palette::text_faint()
                            },
                        );
                        let lock = Rect::from_center_size(
                            Pos2::new(rect.right() - 19.0, rect.center().y),
                            Vec2::splat(20.0),
                        );
                        crate::icons::paint(
                            p,
                            lock.shrink(4.0),
                            if gesperrt { "lock" } else { "unlock" },
                            if gesperrt {
                                Palette::accent()
                            } else {
                                Palette::text_faint()
                            },
                        );
                    }
                    if umbenennen {
                        // Das Feld liegt in der Zeile, an der Stelle des Namens.
                        let mut n = app
                            .ui
                            .rename_layer
                            .as_ref()
                            .map(|(_, n)| n.clone())
                            .unwrap_or_default();
                        // Das Feld füllt die Zeile **ganz** (T0206, Turn 3):
                        // `ui.put` schiebt den Layoutzeiger auf das Rechteck
                        // des Feldes weiter. Ein 20-px-Feld in einer 27-px-
                        // Zeile zog deshalb alle folgenden Zeilen um 3 px nach
                        // oben — die Liste sprang beim Umbenennen.
                        let feld = Rect::from_min_max(
                            Pos2::new(rect.left() + 39.0, rect.top()),
                            Pos2::new(rect.right() - 52.0, rect.bottom()),
                        );
                        let feld_resp = ui.put(
                            feld,
                            egui::TextEdit::singleline(&mut n)
                                .font(FontId::proportional(12.0))
                                .margin(egui::Margin {
                                    left: 4.0,
                                    right: 4.0,
                                    // Oben mehr Luft: der Text sitzt sonst
                                    // 2 px höher als der Name der Nachbarzeilen.
                                    top: 6.0,
                                    bottom: 2.0,
                                }),
                        );
                        if !app.ui.rename_layer_fokus {
                            feld_resp.request_focus();
                            app.ui.rename_layer_fokus = true;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            // Esc verwirft den neuen Namen.
                            app.ui.rename_layer = None;
                        } else if feld_resp.lost_focus() {
                            // Enter und ein Klick daneben übernehmen ihn.
                            let _ = app.session_mut().execute(
                                "layer.update",
                                &json!({"layer": l.id, "name": n}),
                            );
                            app.mark_changed();
                            app.ui.rename_layer = None;
                        } else {
                            app.ui.rename_layer = Some((l.id, n));
                        }
                    } else {
                        let pos = resp.interact_pointer_pos().unwrap_or(rect.center());
                        let schalter =
                            pos.x < rect.left() + 36.0 || pos.x > rect.right() - 52.0;
                        if resp.double_clicked() && !schalter {
                            app.ui.rename_layer = Some((l.id, l.name.clone()));
                            app.ui.rename_layer_fokus = false;
                        } else if resp.clicked() {
                            if pos.x > rect.right() - 30.0 {
                                befehle.push((
                                    "layer.update".into(),
                                    json!({"layer": l.id, "locked": !l.locked}),
                                ));
                            } else if pos.x > rect.right() - 52.0 {
                                befehle.push((
                                    "layer.update".into(),
                                    json!({"layer": l.id, "visible": !l.visible}),
                                ));
                            } else if pos.x < rect.left() + 36.0 {
                                // Linksklick auf den Farbpunkt: Farbe direkt
                                // ändern (T0206).
                                app.ui.color_popup = Some(l.id);
                            } else {
                                set_current = Some(l.id);
                            }
                        }
                        let hover_text = jjcad_core::i18n::t_args_owned(
                            "gui.panels.x_x_mm_x",
                            &[
                                ("wert1", format!("{}", l.name)),
                                ("wert2", format!("{:.2}", l.line_weight)),
                                ("wert3", format!("{}", l.line_type.text())),
                            ],
                        );
                        resp.clone().on_hover_text(hover_text);
                        // **Ein** Menü beim Rechtsklick (T0206): vorher öffnete
                        // sich zusätzlich der Farbwähler, der das Menü zudeckte.
                        resp.context_menu(|ui| {
                            ui.label(theme::dim(&l.name));
                            ui.separator();
                            if ui.button(tr("gui.panels.farbe_aendern")).clicked() {
                                app.ui.color_popup = Some(l.id);
                                ui.close_menu();
                            }
                            if ui.button(tr("gui.panels.aktueller_layer")).clicked() {
                                set_current = Some(l.id);
                                ui.close_menu();
                            }
                            if ui.button(tr("gui.panels.umbenennen")).clicked() {
                                app.ui.rename_layer = Some((l.id, l.name.clone()));
                                app.ui.rename_layer_fokus = false;
                                ui.close_menu();
                            }
                            if ui.button(tr("gui.panels.loeschen")).clicked() {
                                ui.close_menu();
                                // T0213: Hat der Layer noch Objekte, fragt ein
                                // Popup nach dem Ziel; erst danach wird er
                                // gelöscht. Ein leerer Layer geht sofort.
                                match loeschen_ziel(app.doc(), l.id) {
                                    Some(z) => app.ui.layer_delete = Some((l.id, z)),
                                    None => befehle
                                        .push(("layer.delete".into(), json!({"layer": l.id}))),
                                }
                            }
                        });
                    }
                }

                // ---------------- Layer hinzufügen (im Plan) ----------------
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 24.0),
                    Sense::click(),
                );
                if ui.is_rect_visible(rect) {
                    let p = ui.painter();
                    let plus = Rect::from_center_size(
                        Pos2::new(rect.left() + 27.0, rect.center().y),
                        Vec2::splat(14.0),
                    );
                    let f = if resp.hovered() {
                        Palette::text()
                    } else {
                        Palette::text_faint()
                    };
                    crate::icons::paint(p, plus, "plus", f);
                    p.text(
                        Pos2::new(rect.left() + 39.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        tr("gui.panels.layer_hinzufuegen"),
                        FontId::proportional(11.0),
                        f,
                    );
                }
                if resp.clicked() {
                    new_layer_fuer = Some(plan.id);
                }
                ui.add_space(4.0);
            }

            // ---------------- Plan hinzufügen (am Ende der Liste) ----------
            let (rect, resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), 26.0),
                Sense::click(),
            );
            if ui.is_rect_visible(rect) {
                let p = ui.painter();
                let f = if resp.hovered() {
                    Palette::text()
                } else {
                    Palette::text_faint()
                };
                let plus = Rect::from_center_size(
                    Pos2::new(rect.left() + 9.0, rect.center().y),
                    Vec2::splat(14.0),
                );
                crate::icons::paint(p, plus, "plus", f);
                p.text(
                    Pos2::new(rect.left() + 20.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    tr("gui.panels.plan_hinzufuegen"),
                    FontId::proportional(11.0),
                    f,
                );
            }
            if resp.clicked() {
                let n = app.doc().plans.len() + 1;
                app.ui.new_plan_dialog = Some(jjcad_core::i18n::t_args_owned(
                    "gui.panels.plan_n",
                    &[("n", format!("{n}"))],
                ));
            }
            ui.add_space(6.0);
            let aktuell = app
                .doc()
                .layer(current)
                .map(|l| l.name.clone())
                .unwrap_or_default();
            ui.label(theme::dim(jjcad_core::i18n::t_args_owned(
                "gui.panels.x_plaene_x_layer_aktuell_x",
                &[
                    ("plaene", format!("{}", plans.len())),
                    ("layer", format!("{}", app.doc().layers.len())),
                    ("aktuell", aktuell),
                ],
            )));
        });

    if let Some(plan) = new_layer_fuer {
        app.ui.new_layer_plan = Some(plan);
        let n = app.doc().layers_of_plan(plan).len() + 1;
        app.ui.new_layer_dialog = Some(format!("Layer {n}"));
    }
    if let Some(id) = set_current {
        let _ = app
            .session_mut()
            .execute("layer.set_current", &json!({"layer": id}));
    }
    for (aktion, params) in befehle {
        match app.session_mut().execute(&aktion, &params) {
            Ok(_) => app.mark_changed(),
            Err(e) => app.toast(e),
        }
    }
}

/// Ziel einer Lösch-Rückfrage (T0213): Hat der Layer Objekte und gibt es im
/// selben Plan einen anderen Layer, wird vor dem Löschen gefragt — `Some(ziel)`.
/// Ein leerer Layer (oder der letzte seines Plans) geht direkt an die Action,
/// die den Fehler meldet.
pub(crate) fn loeschen_ziel(doc: &Document, lid: Id) -> Option<Id> {
    if doc.layer_objekte(lid) == 0 {
        return None;
    }
    let plan = doc.layer(lid)?.plan;
    doc.layers_of_plan(plan)
        .into_iter()
        .find(|l| l.id != lid)
        .map(|l| l.id)
}

/// Ring um den Farbpunkt einer Layerzeile.
///
/// Wie vor der Überarbeitung ein dünner dunkler Ring — nur bei sehr dunklen
/// Ebenenfarben („Zeichnung“, „Architektur“) hell, sonst verschwände der Punkt
/// auf dem dunklen Panel. Der Ring ist eine Haarlinie; ein heller **Grund**
/// unter dem Punkt wirkte als breiter weißer Rand.
fn farbpunkt_ring(color: Color) -> Color32 {
    if color.luminance() < 0.25 {
        Palette::text_faint()
    } else {
        Color32::from_black_alpha(80)
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
                // Dieselbe Farbliste wie im Eigenschaften-Panel (T0183) — es
                // gibt nur noch eine. Das doppelte Schwarz steht hier einmal:
                // zwei gleiche Kreise wären nicht zu unterscheiden.
                let palette = farbpalette();
                for row in palette.chunks(4) {
                    ui.horizontal(|ui| {
                        for (name, c) in row {
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                            ui.painter().circle_filled(r.center(), 10.0, app::color_to_egui(*c));
                            if layer.color == *c {
                                ui.painter().circle_stroke(
                                    r.center(),
                                    12.5,
                                    Stroke::new(1.5, Palette::accent()),
                                );
                            }
                            if resp.clicked() {
                                chosen = Some(*c);
                            }
                            resp.on_hover_text(name.as_str());
                        }
                    });
                }
                ui.add_space(6.0);
                let mut rgb = [layer.color.r, layer.color.g, layer.color.b];
                ui.horizontal(|ui| {
                    ui.label(theme::dim(jjcad_core::i18n::tr("gui.panels.eigene_farbe")));
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        chosen = Some(Color::rgb(rgb[0], rgb[1], rgb[2]));
                    }
                });
                ui.add_space(4.0);
                if ui.button(jjcad_core::i18n::tr("gui.panels.schliessen")).clicked() {
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
// Blattmanager (T0218)
// ---------------------------------------------------------------------------

/// Liste **aller** Blätter der aktiven Datei — auch derer, deren Reiter
/// geschlossen ist. Ein Klick öffnet das Blatt und holt den Reiter zurück;
/// das Häkchen rechts nimmt es in den Plansatz auf, der Knopf darunter
/// schreibt den Plansatz als eine mehrseitige PDF.
pub fn blaetter(app: &mut App, ui: &mut Ui) {
    let tr = jjcad_core::i18n::tr;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr("gui.panels.blattmanager"))
                .size(10.0)
                .color(Palette::text_faint())
                .strong(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // T0218: Der Pfeil zeigt zur Seite, auf der das Panel liegt —
            // eingeklappt wird dorthin, wo die Leiste steht (wie beim
            // Layer-Panel). Das Symbol in der Leiste holt den Manager zurück.
            let pfeil = if app.panel_seite(crate::app::SeitenPanel::Blatt)
                == jjcad_core::persist::PanelSeite::Links
            {
                "chevron_left"
            } else {
                "chevron_right"
            };
            if widgets::icon_button(ui, pfeil, 20.0, false, tr("gui.panels.einklappen"))
                .clicked()
            {
                app.ui.blatt_collapsed = true;
            }
        });
    });
    ui.add_space(4.0);

    let aktiv = app.tab().tab.clone();
    let zeilen: Vec<(Id, String, bool, bool, bool)> = app
        .doc()
        .sheets
        .iter()
        .map(|s| {
            (
                s.id,
                s.name.clone(),
                s.plansatz,
                matches!(aktiv, TabKind::Sheet(a) if a == s.id),
                app.blatt_tab_offen(s.id),
            )
        })
        .collect();
    let mut oeffnen: Option<Id> = None;
    let mut plansatz: Option<(Id, bool)> = None;
    app.ui.last_blattzeilen.clear();

    let list_hoehe = (ui.available_height() - 36.0).max(60.0);
    egui::ScrollArea::vertical()
        // T0222: eigener Scrollzustand (siehe `layers`).
        .id_salt("panel_blatt")
        .max_height(list_hoehe)
        // Senkrecht schrumpfen: der Knopf gehört **ans Ende der Liste**, nicht
        // an den unteren Fensterrand (bei drei Blättern stand er 500 px tiefer).
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (id, name, im_plansatz, ist_aktiv, tab_offen) in &zeilen {
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 24.0),
                    Sense::hover(),
                );
                // Der Klickbereich endet vor dem Häkchen — sonst öffnete ein
                // Klick daneben immer das Blatt.
                let text_rect = Rect::from_min_max(
                    rect.min,
                    Pos2::new((rect.right() - 30.0).max(rect.left() + 1.0), rect.max.y),
                );
                let resp = ui.interact(
                    text_rect,
                    egui::Id::new(("blattmanager", *id)),
                    Sense::click(),
                );
                if *ist_aktiv || resp.hovered() {
                    ui.painter()
                        .rect_filled(rect, Rounding::same(5.0), Palette::panel_alt());
                }
                let symbol = if *ist_aktiv {
                    Palette::accent()
                } else if *tab_offen {
                    Palette::text_faint()
                } else {
                    Palette::text_dim()
                };
                crate::icons::paint(
                    ui.painter(),
                    Rect::from_min_size(
                        Pos2::new(rect.left() + 4.0, rect.center().y - 6.0),
                        Vec2::splat(12.0),
                    ),
                    "sheet",
                    symbol,
                );
                let farbe = if *ist_aktiv {
                    Palette::text()
                } else if *tab_offen {
                    Palette::text_dim()
                } else {
                    // Geschlossener Reiter: das Blatt ist da, aber nicht offen.
                    Palette::text_faint()
                };
                let galley = ui.painter().layout_no_wrap(
                    name.clone(),
                    FontId::proportional(11.5),
                    farbe,
                );
                ui.painter().galley(
                    Pos2::new(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0),
                    galley,
                    farbe,
                );
                let mut an = *im_plansatz;
                let cb_rect = Rect::from_center_size(
                    Pos2::new(rect.right() - 12.0, rect.center().y),
                    Vec2::splat(18.0),
                );
                let cb = ui.put(cb_rect, egui::Checkbox::without_text(&mut an));
                cb.clone().on_hover_text(tr("gui.panels.plansatz"));
                app.ui.last_blattzeilen.push((*id, text_rect, cb_rect));
                if cb.changed() {
                    plansatz = Some((*id, an));
                }
                if resp.clicked() {
                    oeffnen = Some(*id);
                }
            }
        });

    if let Some((id, an)) = plansatz {
        let _ = app.session_mut().execute(
            "sheet.update",
            &json!({"sheet": id, "plansatz": an}),
        );
        app.mark_changed();
    }
    if let Some(id) = oeffnen {
        app.blatt_oeffnen(id);
    }

    ui.add_space(6.0);
    let anzahl = zeilen.iter().filter(|z| z.2).count();
    let beschriftung = jjcad_core::i18n::t_args_owned(
        "gui.panels.plansatz_pdf_n",
        &[("n", format!("{anzahl}"))],
    );
    let farbe = if anzahl > 0 { Palette::text() } else { Palette::text_faint() };
    let knopf = egui::Button::new(RichText::new(beschriftung).size(11.5).color(farbe))
        .min_size(Vec2::new(ui.available_width(), 24.0))
        .fill(Palette::panel_alt())
        .stroke(Stroke::new(1.0_f32, Palette::border()));
    if ui.add_enabled(anzahl > 0, knopf).clicked() {
        app.plansatz_exportieren();
    }
}

// ---------------------------------------------------------------------------
// Eigenschaften
// ---------------------------------------------------------------------------

pub fn properties(app: &mut App, ui: &mut Ui) {
    // Zeilenprotokoll für den Wächter-Test (props::text_hat_…): welche Zeilen
    // dieses Bild gezeichnet wurden. Kostet ein paar Pushs je Bild.
    crate::props::protokoll_start();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(jjcad_core::i18n::tr("gui.panels.eigenschaften"))
                .size(10.0)
                .color(Palette::text_faint())
                .strong(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // T0203: Der Pfeil zeigt zur Seite, auf der das Panel liegt.
            let pfeil = if app.panel_seite(crate::app::SeitenPanel::Eigenschaften)
                == jjcad_core::persist::PanelSeite::Links
            {
                "chevron_left"
            } else {
                "chevron_right"
            };
            if widgets::icon_button(ui, pfeil, 20.0, false, jjcad_core::i18n::tr("gui.panels.einklappen")).clicked() {
                app.ui.props_collapsed = true;
            }
        });
    });
    ui.add_space(2.0);
    widgets::reset_rows();
    // Das Ziel der Mehrfachauswahl gilt nur für dieses Bild — `props::mehrfach`
    // setzt es beim Zeichnen der Gruppe. Bleibt nichts stehen, schreibt jede
    // Zeile wieder nur an ihr eigenes Objekt.
    app.mehrfach_ziel.clear();
    let sel = app.selection().to_vec();
    let tool_active = app.tool.is_some();
    // Während ein Text in der Zeichenfläche getippt wird, zeigt das Panel
    // **denselben** Text wie beim Bearbeiten (dieselben Zeilen aus `props`).
    // Vorher hatte dieser Zustand ein eigenes, kürzeres Panel (T0028) — genau
    // das ließ das Fenster beim Editieren anders aussehen.
    let eingabe = app.ui.texteingabe.as_ref().map(|t| t.id);
    egui::ScrollArea::vertical()
        // T0222: eigener Scrollzustand (siehe `layers`).
        .id_salt("panel_props")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(id) = eingabe {
                match id {
                    Some(id) => crate::props::text(app, ui, crate::props::Ziel::Auswahl(id)),
                    None => crate::props::text(app, ui, crate::props::Ziel::Neu),
                }
                // Panelwerte in die laufende Eingabe spiegeln: Größe, Schrift,
                // Drehung und Ausrichtung wirken sofort auf die Vorschau.
                crate::props::eingabe_spiegeln(app);
                return;
            }
            // Ein Erstellwerkzeug: das Panel gehört dem neuen Element. Die
            // Auswahl bleibt in der Zeichnung markiert, ihre Eigenschaften
            // erscheinen wieder, sobald das Werkzeug endet — sonst stünden zwei
            // Eigenschafts-Blöcke übereinander.
            let erzeugt_neues = app
                .tool
                .as_ref()
                .map(|t| t.action.starts_with("draw.") || t.action.starts_with("dim."))
                .unwrap_or(false);
            if tool_active {
                // **Alle** Erstellwerkzeuge zeichnen über `props` — eine Quelle
                // für Erzeugen und Bearbeiten. Ein zweiter Maler existiert nicht
                // mehr; neue Objektarten gehören in `props.rs`.
                let tool_id = app
                    .tool
                    .as_ref()
                    .map(|t| t.action.clone())
                    .unwrap_or_default();
                let ziel = crate::props::Ziel::Neu;
                match tool_id.as_str() {
                    "draw.text" => crate::props::text(app, ui, ziel),
                    "draw.wall" => crate::props::wand(app, ui, ziel),
                    "draw.door" => crate::props::tuer(app, ui, ziel),
                    "draw.window" => crate::props::fenster(app, ui, ziel),
                    "draw.stair" => crate::props::treppe(app, ui, ziel),
                    "dim.room" => crate::props::raumstempel(app, ui, ziel),
                    // Ein Element für Einzelmaß und Maßkette (T0154).
                    "dim.linear" | "dim.chain" | "dim.chain.h" | "dim.chain.v"
                    | "dim.chain.d" => crate::props::bemassung(app, ui, ziel),
                    "sheet.viewport_tool" => crate::props::viewport(app, ui, ziel),
                    "draw.volume" | "draw.volume.points" | "draw.volume.paint" => {
                        crate::props::volumen(app, ui, ziel)
                    }
                    "draw.fill" | "draw.hatch" | "draw.fill.boundary"
                    | "draw.fill.points" | "draw.fill.paint" => {
                        crate::props::fuellflaeche(app, ui, ziel)
                    }
                    other => match crate::props::Einfach::aus_werkzeug(other) {
                        Some(art) => crate::props::einfach(app, ui, ziel, art),
                        // Werkzeug ohne eigene Eigenschaften (Verschieben,
                        // Drehen, Ansicht …): das Panel gehört der Auswahl.
                        None => {}
                    },
                }
                if erzeugt_neues {
                    return;
                }
                if !sel.is_empty() {
                    ui.add_space(6.0);
                    widgets::hline(ui);
                    ui.add_space(4.0);
                }
            }
            if sel.len() == 1 {
                let id = sel[0];
                let art = app.doc().entity(id).map(|e| e.kind.clone());
                let ziel = crate::props::Ziel::Auswahl(id);
                match art {
                    Some(jjcad_core::model::EntityKind::Text { .. }) => {
                        crate::props::text(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Wall { .. }) => {
                        crate::props::wand(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Door { .. }) => {
                        crate::props::tuer(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Window { .. }) => {
                        crate::props::fenster(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Fill { .. }) => {
                        crate::props::fuellflaeche(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Dim { .. }) => {
                        crate::props::bemassung(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Volume { .. }) => {
                        crate::props::volumen(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Stair { .. }) => {
                        crate::props::treppe(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Room { .. }) => {
                        crate::props::raumstempel(app, ui, ziel)
                    }
                    Some(jjcad_core::model::EntityKind::Viewport(_)) => {
                        crate::props::viewport(app, ui, ziel)
                    }
                    Some(kind) => match crate::props::Einfach::aus_objekt(&kind) {
                        Some(art) => crate::props::einfach(app, ui, ziel, art),
                        None => {}
                    },
                    None => {}
                }
            } else if sel.len() > 1 {
                crate::props::mehrfach(app, ui, &sel);
            } else if !tool_active {
                let reiter = app.tab().tab.clone();
                match &reiter {
                    TabKind::Sheet(id) => sheet_props(app, ui, *id),
                    TabKind::Block(id) => block_props(app, ui, id),
                    TabKind::Model => doc_props(app, ui),
                }
            }
        });
}

/// Umschaltreihe für die Auszeichnung — F fett, K kursiv, U unterstrichen,
/// x² hochgestellt. Die Knöpfe zeigen ihre eigene Auszeichnung, damit man
/// nicht raten muss, welcher was tut.
pub(crate) fn auszeichnen_knoepfe(
    ui: &mut Ui,
    aktuel: jjcad_core::text::RunStyle,
) -> Option<(&'static str, bool)> {
    let mut klick = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (schluessel, beschriftung, an, hinweis) in [
            ("bold", "F", aktuel.bold, jjcad_core::i18n::tr("gui.panels.fett")),
            ("italic", "K", aktuel.italic, jjcad_core::i18n::tr("gui.panels.kursiv")),
            ("underline", "U", aktuel.underline, jjcad_core::i18n::tr("gui.panels.unterstrichen")),
            ("superscript", "x²", aktuel.superscript, jjcad_core::i18n::tr("gui.panels.hochgestellt")),
        ] {
            let beschriftung = egui::RichText::new(beschriftung).size(13.0);
            let beschriftung = match schluessel {
                "bold" => beschriftung.strong(),
                "italic" => beschriftung.italics(),
                "underline" => beschriftung.underline(),
                _ => beschriftung,
            };
            if ui
                .add(egui::SelectableLabel::new(an, beschriftung))
                .on_hover_text(hinweis)
                .clicked()
            {
                klick = Some((schluessel, !an));
            }
        }
    });
    klick
}

/// Zeichenbereich, für den ein Umschalter gilt: die Markierung im Textfeld,
/// sonst der ganze Text. Ohne Markierung wäre der Umschalter sonst wirkungslos.
fn zeichenbereich(markierung: Option<(usize, usize)>, laenge: usize) -> (usize, usize) {
    match markierung {
        Some((a, b)) if b > a => (a.min(laenge), b.min(laenge)),
        _ => (0, laenge),
    }
}

/// Auszeichnung über die Befehlsschicht setzen und das Dokument als geändert
/// melden. Der eine Weg für alle Umschalter (Panel, Erzeugen, Eingabe).
pub(crate) fn auszeichnen(app: &mut App, id: Id, von: usize, bis: usize, schluessel: &str, wert: bool) {
    let _ = app.session_mut().execute(
        "text.format",
        &json!({"id": id, "from": von, "to": bis, schluessel: wert}),
    );
    app.mark_changed();
}

// Hinweis: Während ein Text in der Zeichenfläche getippt wird, zeichnet
// `props::text` dasselbe Panel wie beim Bearbeiten — ein eigenes Panel für
// die Eingabe gibt es nicht mehr (es sah immer anders aus).

// ---------------------------------------------------------------------------
// Vorgabewerte für Farbe und Linienstärke
// ---------------------------------------------------------------------------

/// Linienbreiten nach DIN EN ISO 128-2:2023-06, Abschnitt 5.1 — die
/// vollständige Reihe mit dem Verhältnis 1 : √2. Andere Werte sind nach der
/// Norm unzulässig; Abweichungen nur über „Freier Wert“ (Abschnitt 5.2).
/// Beschriftet wird jede Breite nur mit Zahl und Einheit — „schmal, breit,
/// extrabreit“ ergibt sich erst aus der Liniengruppe der Zeichnung (Tabelle B.2).
pub const WEIGHT_PRESETS: [f64; 9] = [0.13, 0.18, 0.25, 0.35, 0.50, 0.70, 1.00, 1.40, 2.00];

/// Beschriftung einer Linienbreite: Zahl mit Einheit, deutsches Komma.
fn weight_label(w: f64) -> String {
    format!("{:.2} mm", w).replace('.', ",")
}

/// Eine Gruppe von Farbvorgaben: Überschriftenschlüssel und Einträge
/// (Name, Bedeutung, RGB).
pub struct Farbgruppe {
    pub titel: &'static str,
    pub farben: &'static [(&'static str, &'static str, u32)],
}

/// Die eine Farbliste des Programms (T0183), in drei Gruppen: Bauteilfarben
/// tragen eine Bedeutung (Bestand/Neubau/Abbruch), Gestaltungsfarben sind der
/// geschlossene Farbkreis für die freie Zeichnung, Neutraltöne die Grauwerte.
///
/// `#000000` steht bewusst in **zwei** Gruppen: das neutrale Schwarz bleibt
/// wählbar, wenn die Bauteilfarben ausgeblendet sind. Der Namensrückgriff
/// nimmt den ersten Treffer — im Feld steht dann „Schwarz · Bestand“.
pub const FARBGRUPPEN: [Farbgruppe; 3] = [
    Farbgruppe {
        titel: "gui.panels.farbgruppe_bauteile",
        farben: &[
            ("gui.panels.farbe_schwarz", "gui.panels.bedeutung_bestand", 0x000000),
            ("gui.panels.farbe_rot", "gui.panels.bedeutung_neubau", 0xFF0000),
            ("gui.panels.farbe_gelb", "gui.panels.bedeutung_abbruch", 0xFFFF00),
        ],
    },
    Farbgruppe {
        titel: "gui.panels.farbgruppe_gestaltung",
        farben: &[
            ("gui.panels.farbe_rot", "", 0xBE3226),
            ("gui.panels.farbe_orange", "", 0xD9822B),
            ("gui.panels.farbe_gelb", "", 0xEBCE2E),
            ("gui.panels.farbe_braun", "", 0x7A5637),
            ("gui.panels.farbe_gruen", "", 0x2E7D4F),
            ("gui.panels.farbe_tuerkis", "", 0x2A9D8F),
            ("gui.panels.farbe_cyan", "", 0x3FA6C2),
            ("gui.panels.farbe_blau", "", 0x2E5FA3),
            ("gui.panels.farbe_violett", "", 0x6A4C93),
            ("gui.panels.farbe_magenta", "", 0xB3508A),
        ],
    },
    Farbgruppe {
        titel: "gui.panels.farbgruppe_neutral",
        farben: &[
            ("gui.panels.farbe_schwarz", "", 0x000000),
            ("gui.panels.farbe_grau_25", "", 0x404040),
            ("gui.panels.farbe_grau_50", "", 0x808080),
            ("gui.panels.farbe_grau_75", "", 0xBFBFBF),
            ("gui.panels.farbe_hellgrau", "", 0xD9D9D9),
            ("gui.panels.farbe_weiss", "", 0xFFFFFF),
        ],
    },
];

/// Anzeigename einer Vorgabe: „Name“ oder „Name · Bedeutung“.
fn preset_label(name: &'static str, meaning: &'static str) -> String {
    let n = jjcad_core::i18n::tr(name);
    if meaning.is_empty() {
        n.to_string()
    } else {
        format!("{} · {}", n, jjcad_core::i18n::tr(meaning))
    }
}

/// Anzeigename zu einem Hexwert — erster Treffer in der Reihenfolge der
/// Gruppen (bei `#000000` also „Schwarz · Bestand“).
fn preset_name(hex: &str) -> Option<String> {
    FARBGRUPPEN
        .iter()
        .flat_map(|g| g.farben.iter())
        .find(|(_, _, rgb)| preset_color(*rgb).to_hex() == hex)
        .map(|(n, m, _)| preset_label(n, m))
}

/// Alle Vorgaben für den Layer-Farbwähler, je Hexwert einmal (das doppelte
/// Schwarz erscheint dort nur einmal).
fn farbpalette() -> Vec<(String, Color)> {
    let mut out: Vec<(String, Color)> = Vec::new();
    for g in FARBGRUPPEN.iter() {
        for (name, meaning, rgb) in g.farben.iter() {
            let c = preset_color(*rgb);
            if out.iter().any(|(_, v)| v.to_hex() == c.to_hex()) {
                continue;
            }
            out.push((preset_label(name, meaning), c));
        }
    }
    out
}

fn preset_color(rgb: u32) -> Color {
    Color::rgb(((rgb >> 16) & 0xFF) as u8, ((rgb >> 8) & 0xFF) as u8, (rgb & 0xFF) as u8)
}

fn swatch(ui: &mut Ui, c: Color) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(11.0), Sense::hover());
    ui.painter()
        .rect_filled(r, Rounding::same(2.0), egui::Color32::from_rgb(c.r, c.g, c.b));
    ui.painter()
        .rect_stroke(r, Rounding::same(2.0), Stroke::new(0.7, Palette::text_faint()));
    ui.add_space(4.0);
}

fn weight_text(w: f64) -> String {
    weight_label(w)
}

/// Eigener Dropdown: bleibt offen, während man im Popup arbeitet
/// (Zahlenfeld, Farbwähler) und zeigt im geschlossenen Zustand ein Farbfeld.
#[derive(Clone, Copy)]
pub(crate) enum BtnIcon {
    Color(Color),
    Line(LineType),
}

/// Höhe eines Auswahlfelds in den Panels (dichte Zeilen).
///
/// T0178: dieselbe Höhe wie ein Eingabefeld (`widgets::FELD_H`) plus einen
/// Bildpunkt — ein Zahlenfeld zeichnet seinen Rahmen eine halbe Strichbreite
/// über die Fläche hinaus, der Auswahlknopf innerhalb seiner Fläche. Mit 20 px
/// wirkte jedes Auswahlfeld neben einem Feld deutlich kleiner (gemessen 21
/// gegen 26 px im Bild).
pub(crate) const DROPDOWN_H: f32 = crate::widgets::FELD_H + 1.0;

/// Höchsthöhe des aufgeklappten Auswahlmenüs (rund 15 Zeilen). Darüber wird
/// gerollt — bis dahin wächst das Menü mit der Zahl der Einträge (T0076).
pub(crate) const DROPDOWN_MAX_H: f32 = 420.0;

pub(crate) fn dropdown<R>(
    ui: &mut Ui,
    salt: &str,
    caption: &str,
    icon: Option<BtnIcon>,
    add_contents: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    dropdown_mit_hoehe(ui, salt, caption, icon, DROPDOWN_H, add_contents)
}

/// Derselbe Auswahlknopf in einer vorgegebenen Höhe — für Zeilen mit normalen
/// Knöpfen daneben, die alle dieselbe Höhe haben sollen (T0076).
pub(crate) fn dropdown_mit_hoehe<R>(
    ui: &mut Ui,
    salt: &str,
    caption: &str,
    icon: Option<BtnIcon>,
    hoehe: f32,
    add_contents: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    dropdown_mit_hoehe_rect(ui, salt, caption, icon, hoehe, &mut None, add_contents)
}

/// Wie [`dropdown_mit_hoehe`], meldet zusätzlich die Fläche des geschlossenen
/// Felds (Prüfung: gleiche Höhe wie die Knöpfe daneben, T0076).
pub(crate) fn dropdown_mit_hoehe_rect<R>(
    ui: &mut Ui,
    salt: &str,
    caption: &str,
    icon: Option<BtnIcon>,
    hoehe: f32,
    flaeche: &mut Option<Rect>,
    add_contents: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    dropdown_mit_menue(ui, salt, caption, icon, hoehe, None, flaeche, add_contents)
}

/// Wie [`dropdown_mit_hoehe_rect`], zusätzlich mit der gewünschten Höhe des
/// aufgeklappten Menüs (T0076: das Layoutmenü wächst mit der Zahl der Layouts).
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_mit_menue<R>(
    ui: &mut Ui,
    salt: &str,
    caption: &str,
    icon: Option<BtnIcon>,
    hoehe: f32,
    menue_hoehe: Option<f32>,
    flaeche: &mut Option<Rect>,
    add_contents: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    let id = ui.make_persistent_id(("dd", salt));
    let w = ui.available_width().min(crate::widgets::VAL_W);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, hoehe), Sense::click());
    *flaeche = Some(rect);
    let open = ui.memory(|m| m.is_popup_open(id));
    let hovered = resp.hovered();
    let pnt = ui.painter();
    pnt.rect_filled(
        rect,
        egui::Rounding::same(4.0),
        if open || hovered { Palette::panel_alt() } else { Palette::bg() },
    );
    pnt.rect_stroke(
        rect,
        egui::Rounding::same(4.0),
        Stroke::new(
            1.0,
            if open {
                Palette::accent()
            } else {
                Palette::panel_alt().gamma_multiply(1.6)
            },
        ),
    );
    let mut x = rect.left() + 7.0;
    match icon {
        Some(BtnIcon::Color(c)) => {
            let r = Rect::from_min_size(egui::pos2(x, rect.center().y - 5.0), Vec2::splat(10.0));
            pnt.rect_filled(r, egui::Rounding::same(2.0), egui::Color32::from_rgb(c.r, c.g, c.b));
            pnt.rect_stroke(r, egui::Rounding::same(2.0), Stroke::new(0.8, Palette::text_faint()));
            x += 16.0;
        }
        Some(BtnIcon::Line(lt)) => {
            let y = rect.center().y;
            let a = egui::pos2(x, y);
            let b = egui::pos2(x + 22.0, y);
            match crate::canvas::dash_pattern(lt, 3.2, false) {
                Some(pat) => {
                    for seg in crate::canvas::dashed_segments(a, b, &pat) {
                        pnt.line_segment(seg, Stroke::new(1.3, Palette::text_dim()));
                    }
                }
                None => {
                    pnt.line_segment([a, b], Stroke::new(1.3, Palette::text_dim()));
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
        Palette::text(),
    );
    pnt.text(
        egui::pos2(rect.right() - 7.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        jjcad_core::i18n::tr("gui.panels.v"),
        FontId::proportional(9.0),
        Palette::text_dim(),
    );

    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }

    let mut out = None;
    if ui.memory(|m| m.is_popup_open(id)) {
        // Das Menü wächst mit seinem Inhalt bis `DROPDOWN_MAX_H`. Reicht der
        // Platz unter dem Feld nicht, öffnet es nach oben — sonst stünde die
        // halbe Liste außerhalb des Fensters (T0076).
        let menue_h = menue_hoehe.unwrap_or(DROPDOWN_MAX_H).min(DROPDOWN_MAX_H);
        let bildschirm = ui.ctx().screen_rect();
        let platz_unten = bildschirm.bottom() - resp.rect.bottom() - 12.0;
        let platz_oben = resp.rect.top() - bildschirm.top() - 12.0;
        let lage = if platz_unten >= menue_h || platz_unten >= platz_oben {
            egui::AboveOrBelow::Below
        } else {
            egui::AboveOrBelow::Above
        };
        egui::popup_above_or_below_widget(
            ui,
            id,
            &resp,
            lage,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(330.0);
                egui::ScrollArea::vertical()
                    .max_height(menue_h)
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
pub(crate) fn linetype_combo(ui: &mut Ui, salt: &str, cur: LineType, mixed: bool) -> Option<LineType> {
    let cap = if mixed {
        "*variiert*".to_string()
    } else {
        cur.text().to_string()
    };
    let icon = if mixed { None } else { Some(BtnIcon::Line(cur)) };
    dropdown(ui, salt, &cap, icon, |ui| {
        let mut out = None;
        for cand in LineType::ALL {
            let sel = !mixed && cand == cur;
            ui.horizontal(|ui| {
                lt_preview(ui, cand, if sel { Palette::accent() } else { Palette::text_dim() });
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
pub(crate) fn weight_combo(ui: &mut Ui, salt: &str, cur: f64, mixed: bool) -> Option<f64> {
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
            if ui.small_button(jjcad_core::i18n::tr("gui.panels.liste")).on_hover_text(jjcad_core::i18n::tr("gui.panels.vorgaben_anzeigen")).clicked() {
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
        for val in WEIGHT_PRESETS {
            let txt = weight_label(val);
            if ui.selectable_label((cur - val).abs() < 1e-6, txt).clicked() {
                out = Some(val);
            }
        }
        ui.separator();
        if ui.selectable_label(false, jjcad_core::i18n::tr("gui.panels.freier_wert")).clicked() {
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
pub(crate) fn color_combo(
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
            if ui.small_button(jjcad_core::i18n::tr("gui.panels.liste")).on_hover_text(jjcad_core::i18n::tr("gui.panels.vorgaben_anzeigen")).clicked() {
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
        let name = preset_name(&hex).unwrap_or_else(|| format!("eigene {}", hex));
        (name, Some(cur))
    };
    dropdown(ui, salt, &cap, sw.map(BtnIcon::Color), |ui| {
        let mut out: Option<Option<Color>> = None;
        if ui.selectable_label(unset, unset_label).clicked() {
            out = Some(None);
        }
        ui.separator();
        for g in FARBGRUPPEN.iter() {
            ui.add_space(2.0);
            ui.label(
                theme::bold(jjcad_core::i18n::tr(g.titel), 10.5).color(Palette::text_faint()),
            );
            ui.columns(2, |cols| {
                for (i, (name, meaning, rgb)) in g.farben.iter().enumerate() {
                    let cell = &mut cols[i % 2];
                    let c = preset_color(*rgb);
                    let sel = !unset && cur.to_hex() == c.to_hex();
                    cell.horizontal(|ui| {
                        swatch(ui, c);
                        if ui.selectable_label(sel, preset_label(name, meaning)).clicked() {
                            out = Some(Some(c));
                        }
                    });
                }
            });
        }
        ui.separator();
        if ui.selectable_label(false, jjcad_core::i18n::tr("gui.panels.freie_farbe")).clicked() {
            ui.data_mut(|d| d.insert_temp(eid, true));
            out = Some(Some(cur));
        }
        out
    })
}

pub(crate) fn section_title(ui: &mut Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
        crate::icons::paint(ui.painter(), r, icon, Palette::accent());
        ui.label(theme::bold(title, 12.5));
    });
    ui.add_space(4.0);
}

/// Anzeigezeilen des Abschnitts „Geometrie“ — für alle Typen dieselbe Quelle.
///
/// Der Wert ist reiner Text (Einheit schon enthalten); jede Zeile hat genau
/// einen Namen und genau einen Wert. Deshalb taucht z. B. „Länge“ bei einer
/// Polylinie nicht doppelt auf und die Flächeneinheit steht genau einmal im
/// Text (vorher kam „m²“ aus `format_area` und noch einmal aus dem Aufrufer).
pub(crate) fn geo_rows(e: &jjcad_core::model::Entity, unit: Unit) -> Vec<(&'static str, String)> {
    use jjcad_core::model::EntityKind as K;
    let mut rows: Vec<(&'static str, String)> = Vec::new();
    let len = entity_length(e);
    if len > 0.0 {
        rows.push((jjcad_core::i18n::tr("gui.panels.laenge"), unit.format(len, unit.decimals())));
    }
    if let Some((dx, dy)) = delta_xy(e, unit) {
        rows.push(("Delta X", dx));
        rows.push(("Delta Y", dy));
    }
    let area = entity_area(e);
    if area > 0.0 {
        rows.push((jjcad_core::i18n::tr("gui.panels.flaeche"), area_text(unit, area)));
    }
    if let K::Path { segments, closed } = &e.kind {
        rows.push((
            jjcad_core::i18n::tr("gui.panels.stuecke"),
            segments.len().to_string(),
        ));
        rows.push((
            jjcad_core::i18n::tr("gui.panels.geschlossen"),
            if *closed {
                jjcad_core::i18n::tr("gui.panels.ja")
            } else {
                jjcad_core::i18n::tr("gui.panels.nein")
            }
            .to_string(),
        ));
    }
    if let K::Polyline { verts, closed } | K::Spline { verts, closed } = &e.kind {
        rows.push((jjcad_core::i18n::tr("gui.panels.stuetzpunkte"), verts.len().to_string()));
        rows.push((
            jjcad_core::i18n::tr("gui.panels.geschlossen"),
            if *closed {
                jjcad_core::i18n::tr("gui.panels.ja")
            } else {
                jjcad_core::i18n::tr("gui.panels.nein")
            }
            .to_string(),
        ));
    }
    rows
}

/// Relativer Abstand zwischen den Basispunkten einer Linie oder Wand
/// (`b - a`, vorzeichenbehaftet — so bleibt die Richtung ablesbar).
///
/// Eine Quelle für die Geometrie-Zeilen (`geo_rows`) und die Kennzahlen der
/// Wand im Eigenschaften-Panel: sonst rechneten zwei Stellen dasselbe und die
/// Anzeige liefe auseinander.
pub(crate) fn delta_xy(
    e: &jjcad_core::model::Entity,
    unit: Unit,
) -> Option<(String, String)> {
    use jjcad_core::model::EntityKind as K;
    match &e.kind {
        K::Line { a, b } | K::Wall { a, b, .. } => {
            let d = *b - *a;
            Some((
                unit.format(d.x, unit.decimals()),
                unit.format(d.y, unit.decimals()),
            ))
        }
        _ => None,
    }
}

/// Flächentext für die Anzeige: die Einheit kommt aus `format_area` und steht
/// damit genau einmal im Text.
pub(crate) fn area_text(unit: Unit, mm2: f64) -> String {
    unit.format_area(mm2, 2)
}

/// Länge eines Elements in mm (0, wenn nicht sinnvoll).
pub(crate) fn entity_length(e: &jjcad_core::model::Entity) -> f64 {
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
        // Ellipsenbogen: Länge des Abschnitts als Summe der Sehnen (ein
        // Ellipsenbogen hat kein geschlossenes Längenmaß).
        K::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = jjcad_core::geom::Ellipse::new(*c, *rx, *ry, *rot);
            let pts = el.arc_points(*a0, *a1, 0.05);
            pts.windows(2).map(|w| w[0].dist(w[1])).sum()
        }
        K::Spline { verts, closed } => {
            let sp = jjcad_core::geom::Spline {
                verts: verts.clone(),
                closed: *closed,
            };
            sp.len()
        }
        // Pfad: Summe aller Stücke (Bogen und Ellipsenbogen über ihre Zerlegung).
        K::Path { segments, .. } => jjcad_core::pfad::pfad_len(segments),
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
///
/// Die Rechnung steht in `jjcad_core::area::entity_area` — dieselbe Zahl
/// liefert die MCP-Abfrage (`area_m2`, `select.area`). Hier nur der
/// Zahlenwert für die Anzeige; 0 heißt „keine Fläche“.
pub(crate) fn entity_area(e: &jjcad_core::model::Entity) -> f64 {
    jjcad_core::area::entity_area(e).unwrap_or(0.0)
}

fn sheet_props(app: &mut App, ui: &mut Ui, sid: Id) {
    let Some(sheet) = app.doc().sheet(sid).cloned() else { return };
    section_title(ui, "sheet", &sheet.name);
    let (pw, ph) = sheet.size();
    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.panels.x_x_x_mm_x", &[("wert1", format!("{}", sheet.paper.label())), ("wert2", format!("{:.0}", pw)), ("wert3", format!("{:.0}", ph)), ("wert4", format!("{}", format_scale(sheet.scale)))])));
    ui.add_space(8.0);

    let mut name = sheet.name.clone();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.name"), |ui| {
        if ui.add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut name)).changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "name": name}));
        }
    });
    let mut paper = sheet.paper;
    let mut landscape = sheet.landscape;
    let mut scale = sheet.scale;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.format"), |ui| {
        egui::ComboBox::from_id_salt("shpaper")
            .width(70.0)
            .selected_text(paper.label())
            .show_ui(ui, |ui| {
                for p in PaperSize::ALL {
                    ui.selectable_value(&mut paper, p, p.label());
                }
            });
        ui.checkbox(&mut landscape, jjcad_core::i18n::tr("gui.panels.quer"));
    });
    if paper != sheet.paper || landscape != sheet.landscape {
        let _ = app.session_mut().execute(
            "sheet.update",
            &json!({"sheet": sid, "paper": paper.label(), "landscape": landscape}),
        );
        app.mark_changed();
    }
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.massstab"), |ui| {
        if ui.add_sized([70.0, 20.0], crate::widgets::zahl(&mut scale, 1.0, 1.0..=1000.0)).changed() {
            let _ = app
                .session_mut()
                .execute("sheet.update", &json!({"sheet": sid, "scale": scale}));
            app.mark_changed();
        }
    });
    // T0228: Eingabeeinheit des **Papierraums**, unabhängig vom Modell. Das
    // Papier ist in Millimetern gezeichnet (Blattecken (0,0)…(297,210), Text
    // 2,5 mm) — in Metern stünde jedes Feld auf 0,00. Vorgabe Millimeter.
    let mut paper_unit = app.doc().settings.paper_input_unit;
    let einheiten: Vec<(Unit, String)> = Unit::ALL
        .iter()
        .map(|u| (*u, u.label().to_string()))
        .collect();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.einheit"), |ui| {
        if let Some(neu) = pick_combo(ui, "papiereinheit", paper_unit, &einheiten) {
            let _ = app.session_mut().execute(
                "doc.settings",
                &json!({ "paper_input_unit": neu.short() }),
            );
            app.mark_changed();
        }
    });
    // Rahmen und Schriftfeld sind normale Blattobjekte (T0004): sie werden im
    // Papierraum angeklickt und geändert wie jede Linie und jeder Text. Ein
    // Schalter dafür wäre irreführend — hier steht deshalb nur der Hinweis.
    ui.label(theme::dim(
        jjcad_core::i18n::tr("gui.panels.rahmen_und_schriftfeld_sind_normale_objekte_im_papierrau"),
    ));

    ui.add_space(10.0);
    widgets::hline(ui);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(theme::bold(jjcad_core::i18n::tr("gui.panels.ansichtsfenster"), 12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, "plus", 20.0, false, jjcad_core::i18n::tr("gui.panels.ansichtsfenster_hinzufuegen")).clicked() {
                app.run_action(ui.ctx(), "sheet.add_viewport");
            }
        });
    });
    ui.add_space(4.0);

    // Ein Ansichtsfenster ist seit T0109 ein normales Blattobjekt: es wird im
    // Papierraum angeklickt und im Eigenschaften-Panel geändert wie jede Linie.
    ui.label(theme::dim(jjcad_core::i18n::tr(
        "gui.panels.ansichtsfenster_sind_normale_objekte_im_papierraum",
    )));
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
            zeilen.push((jjcad_core::i18n::tr("gui.panels.datei"), name, Some(pfad.to_string())));
            zeilen.push((
                jjcad_core::i18n::tr("gui.panels.stand"),
                if doc.modified {
                    jjcad_core::i18n::tr("gui.panels.geaendert_nicht_gespeichert")
                } else {
                    jjcad_core::i18n::tr("gui.panels.gespeichert")
                }
                .to_string(),
                None,
            ));
            let bytes = std::fs::metadata(pfad).map(|m| m.len()).unwrap_or(0);
            zeilen.push((jjcad_core::i18n::tr("gui.panels.groesse"), datei_groesse(bytes), None));
        }
        None => {
            zeilen.push((
                jjcad_core::i18n::tr("gui.panels.datei"),
                jjcad_core::i18n::tr("gui.panels.nicht_gespeichert").to_string(),
                None,
            ));
            zeilen.push((jjcad_core::i18n::tr("gui.panels.stand"), "–".to_string(), None));
            zeilen.push((jjcad_core::i18n::tr("gui.panels.groesse"), "–".to_string(), None));
        }
    }
    zeilen.push((jjcad_core::i18n::tr("gui.panels.ausdehnung"), ausdehnung_text(doc, unit), None));
    zeilen.push((jjcad_core::i18n::tr("gui.panels.blaetter"), blatt_text(doc), None));
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
        return jjcad_core::i18n::tr("gui.panels.leer").to_string();
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
            if s.landscape {
                jjcad_core::i18n::tr("gui.panels.quer")
            } else {
                jjcad_core::i18n::tr("gui.panels.hoch")
            }
        )
    };
    match doc.sheets.len() {
        0 => jjcad_core::i18n::tr("gui.panels.keine").to_string(),
        1 => format!("1 × {}", beschreibung(&doc.sheets[0])),
        n => {
            let liste: Vec<String> = doc.sheets.iter().map(beschreibung).collect();
            jjcad_core::i18n::t_args_owned("gui.panels.n_blaetter_x", &[("anzahl", format!("{}", n)), ("liste", liste.join(", "))])
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
            EntityKind::Dim { .. }
            | EntityKind::ElevationPlan { .. }
            | EntityKind::ElevationSection { .. } => masse += 1,
            EntityKind::Text { .. } => texte += 1,
            EntityKind::Fill { .. } => flaechen += 1,
            EntityKind::BlockRef { .. } => bloecke += 1,
            _ => {}
        }
    }
    // Einzahl/Mehrzahl getrennt: „1 Tür", aber „3 Türen".
    let teile: Vec<String> = [
        (waende, "gui.panels.ein_wand", "gui.panels.viele_waende"),
        (tueren, "gui.panels.ein_tuer", "gui.panels.viele_tueren"),
        (fenster, "gui.panels.ein_fenster", "gui.panels.viele_fenster"),
        (treppen, "gui.panels.ein_treppe", "gui.panels.viele_treppen"),
        (masse, "gui.panels.ein_mass", "gui.panels.viele_masse"),
        (texte, "gui.panels.ein_text", "gui.panels.viele_texte"),
        (flaechen, "gui.panels.ein_flaeche", "gui.panels.viele_flaechen"),
        (bloecke, "gui.panels.ein_block", "gui.panels.viele_bloecke"),
    ]
    .iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, ein, viel)| {
        let wort = jjcad_core::i18n::tr(if *n == 1 { ein } else { viel });
        jjcad_core::i18n::t_args_owned(
            "gui.panels.n_objekt",
            &[("anzahl", format!("{}", n)), ("objekt", wort.to_string())],
        )
    })
    .collect();
    if teile.is_empty() {
        "keine Objekte".to_string()
    } else {
        teile.join(" · ")
    }
}

/// Eigenschaften des **Blocks**, dessen Editor offen ist (T0137).
///
/// Erscheint nur, solange nichts ausgewählt ist — ein ausgewähltes Objekt
/// gehört mit seinen Eigenschaften dem Panel (dieselbe Regel wie im Modell).
/// Name, Gruppe und Beschreibung sind hier bearbeitbar; „Speichern“ übernimmt
/// (bei Benutzerblöcken samt Datei) und schließt den Reiter.
fn block_props(app: &mut App, ui: &mut Ui, id: &str) {
    let Some(block) = app.doc().block(id).cloned() else {
        return;
    };
    section_title(ui, "block", &block.name);
    ui.label(theme::dim(
        jjcad_core::i18n::tr("gui.panels.blockeditor_hinweis"),
    ));
    ui.add_space(8.0);

    let mut name = block.name.clone();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.name"), |ui| {
        if ui
            .add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut name))
            .changed()
        {
            if let Some(b) = app.doc_mut().block_mut(id) {
                b.name = name.clone();
            }
            app.mark_changed();
        }
    });
    let mut gruppe = block.category.clone();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.gruppe"), |ui| {
        if ui
            .add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut gruppe))
            .changed()
        {
            if let Some(b) = app.doc_mut().block_mut(id) {
                b.category = gruppe.clone();
            }
            app.mark_changed();
        }
    });
    let mut beschreibung = block.description.clone();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.beschreibung"), |ui| {
        if ui
            .add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut beschreibung))
            .changed()
        {
            if let Some(b) = app.doc_mut().block_mut(id) {
                b.description = beschreibung.clone();
            }
            app.mark_changed();
        }
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.herkunft"), |ui| {
        ui.label(block.source.text());
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.bauteile"), |ui| {
        ui.label(format!("{}", block.entities.len()));
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.kennung"), |ui| {
        ui.label(theme::dim(&block.id));
    });

    ui.add_space(12.0);
    widgets::hline(ui);
    ui.add_space(8.0);
    let knopf = egui::Button::new(theme::bold(
        jjcad_core::i18n::tr("gui.panels.block_speichern"),
        12.0,
    ))
    .min_size(Vec2::new(150.0, 26.0));
    if ui.add(knopf).clicked() {
        app.block_editor_speichern();
    }
}

fn doc_props(app: &mut App, ui: &mut Ui) {
    section_title(ui, "settings", jjcad_core::i18n::tr("gui.panels.zeichnung"));
    let unit = app.doc().settings.input_unit;
    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.panels.x_x_objekte_layer_x", &[("wert1", format!("{}", app.doc().name)), ("wert2", format!("{}", app.doc().entities.len())), ("wert3", format!("{}", app.doc().layers.len()))])));
    ui.add_space(8.0);

    // Informationen über die Zeichnungsdatei selbst — reine Anzeige.
    let info = zeichnungs_info(app.doc(), unit);
    for (name, wert, tip) in &info {
        widgets::info_row(ui, name, wert, tip.as_deref());
    }
    // Der Bestand ist zu lang für die 150-px-Wertespalte — als umbrechende
    // Zeile darunter, damit nichts abgeschnitten wird.
    ui.add_space(4.0);
    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.panels.bestand_x", &[("wert1", format!("{}", bestand_text(app.doc())))])));

    ui.add_space(10.0);
    widgets::hline(ui);
    ui.add_space(6.0);

    // Einheit, Dezimalen und Rasterweite gelten für die ganze Zeichnung —
    // Texthöhe und Wandstärke setzt man im Einstellungen-Dialog (Architektur),
    // nicht hier (T0011, Turn 2).
    let mut input = app.doc().settings.input_unit;
    let mut decimals = app.doc().settings.decimals as i32;
    let mut grid = app.doc().settings.snap.grid_step;

    // T0178: eigener Auswahlknopf statt `ComboBox` — der ist niedriger als ein
    // Eingabefeld und im Panel deshalb als „zu klein“ aufgefallen.
    let einheiten: Vec<(Unit, String)> = Unit::ALL
        .iter()
        .map(|u| (*u, u.label().to_string()))
        .collect();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.einheit"), |ui| {
        if let Some(neu) = pick_combo(ui, "inpunit", input, &einheiten) {
            input = neu;
        }
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.dezimalen"), |ui| {
        ui.add_sized([ui.available_width(), 20.0], crate::widgets::zahl(&mut decimals, 1.0, 0..=4));
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.raster"), |ui| {
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
    // Den Maßstab der Zeichnung gibt es nicht mehr (T0232): er gehört zu jedem
    // Plan und steht im Layer-Manager an der Planzeile (Rechtsklick → Maßstab).
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
    section_title(ui, "info", jjcad_core::i18n::tr("gui.panels.projektdaten"));
    let mut project = app.doc().project.clone();
    let mut changed = false;
    for (label, field) in [
        (jjcad_core::i18n::tr("gui.panels.projekt"), &mut project.project),
        (jjcad_core::i18n::tr("gui.panels.bauherr"), &mut project.client),
        (jjcad_core::i18n::tr("gui.panels.planung"), &mut project.planner),
        (jjcad_core::i18n::tr("gui.panels.adresse"), &mut project.address),
        ("Plan-Nr.", &mut project.plan_no),
        (jjcad_core::i18n::tr("gui.panels.planinhalt"), &mut project.plan_title),
        (jjcad_core::i18n::tr("gui.panels.revision"), &mut project.revision),
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
    ui.label(theme::dim(jjcad_core::i18n::tr("gui.panels.platzhalter_in_texten_x_x_x_x_x_x")));
}

/// Maßketten: Maßeinheit, Textgröße, Maßstab und Bogensymbole.
///
/// Diese vier Werte standen früher im Ribbon und danach im Eigenschaften-Panel
/// des Modells. Letzteres zeigte sie an, obwohl dort „Informationen über die
/// Zeichnung" stehen — deshalb sitzen sie seit T0011 im Einstellungen-Dialog
/// (Abschnitt „Bemaßung") und an keiner anderen Stelle. Sie sind
/// Dokumenteinstellungen: `doc.settings.dim_unit`, der Maßstab des aktuellen
/// Plans (T0232) und `doc.dim_style.text_height` / `.arrow`. Jede Änderung geht als Action
/// (`doc.settings` bzw. `dim.style`) über die Befehlsschicht und landet damit
/// auch in Undo/Redo und in der gespeicherten Datei.
pub(crate) fn dim_fields(app: &mut App, ui: &mut Ui) {

    // Maßeinheit der Maßzahl (mm / cm / m)
    let cur_unit = app.doc().settings.dim_unit;
    let unit_items: Vec<(Unit, String)> = Unit::ALL
        .iter()
        .map(|u| (*u, u.text().to_string()))
        .collect();
    let mut picked_unit = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.masseinheit"), |ui| {
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
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.textgroesse"), |ui| {
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

    // Bezugsmaßstab (1:N) des Plans, in dem gezeichnet wird (T0232) — alle
    // Papiergrößen der Bemaßung hängen daran. Jeder Plan trägt seinen eigenen;
    // geschrieben wird der des aktuellen Plans.
    let mut s = app.doc().current_scale();
    let skala_plan = app.doc().aktueller_plan();
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.massstab"), |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 20.0],
                crate::widgets::zahl(&mut s, 1.0, 1.0..=1000.0).prefix("1:"),
            )
            .changed()
        {
            if let Some(pid) = skala_plan {
                let _ = app.session_mut().execute(
                    "plan.update",
                    &json!({"plan": pid, "scale": s}),
                );
                app.mark_changed();
            }
        }
    });

    // Bogensymbole an den Maßlinienenden. Im geschlossenen Feld steht die kurze
    // Form (die DIN-Angabe passt nicht in die Wertespalte), im Menü der volle Name.
    let cur_arrow = app.doc().dim_style.arrow;
    let mut picked_arrow = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.panels.bogensymbole"), |ui| {
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
        ArrowKind::Slash => jjcad_core::i18n::tr("gui.panels.schraegstrich"),
        ArrowKind::Arrow => jjcad_core::i18n::tr("gui.panels.pfeil"),
        ArrowKind::Dot => jjcad_core::i18n::tr("gui.panels.punkt"),
        ArrowKind::None => jjcad_core::i18n::tr("gui.panels.ohne"),
    }
}

#[cfg(test)]
mod tests_massstab {
    //! T0232: Der Maßstab (1:N) gehört zum **Plan**, nicht mehr zur Datei. Er
    //! steht in der Planzeile des Layer-Managers (als Text „1:N“) und wird im
    //! Rechtsklickmenü der Zeile geändert.

    use super::*;
    use crate::app::App;

    /// Zwei Pläne: „Plan 1“ 1:100, „Erdgeschoss“ 1:200.
    fn zwei_plaene() -> (App, Id) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.ui.layers_open = true;
        let _ = app
            .session_mut()
            .execute(
                "plan.create",
                &json!({"name": "Erdgeschoss", "scale": 200.0}),
            )
            .expect("Plan anlegen");
        let zweiter = app.doc().plans[1].id;
        (app, zweiter)
    }

    fn texte(app: &mut App, ctx: &egui::Context) -> Vec<String> {
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn planzeile_zeigt_den_massstab_seines_plans() {
        let (mut app, _) = zwei_plaene();
        let ctx = app.ctx.clone();
        let alle = texte(&mut app, &ctx);
        assert!(
            alle.iter().any(|t| t == "1:100"),
            "Maßstab 1:100 fehlt in der Planzeile (Texte: {alle:?})"
        );
        assert!(
            alle.iter().any(|t| t == "1:200"),
            "Maßstab 1:200 fehlt in der Planzeile des zweiten Plans (Texte: {alle:?})"
        );
    }

    #[test]
    fn plan_update_setzt_den_massstab() {
        let (mut app, zweiter) = zwei_plaene();
        let _ = app
            .session_mut()
            .execute("plan.update", &json!({"plan": zweiter, "scale": 50.0}))
            .expect("plan.update");
        assert_eq!(app.doc().plan_scale(zweiter), 50.0);
        // Die Layer dieses Plans rechnen mit seinem Maßstab — der andere Plan
        // bleibt bei 1:100.
        let layer2 = app.doc().layers_of_plan(zweiter)[0].id;
        assert_eq!(app.doc().plan_scale_of_layer(layer2), 50.0);
        let layer1 = app.doc().plans[0].id;
        let erste = app.doc().layers_of_plan(layer1)[0].id;
        assert_eq!(app.doc().plan_scale_of_layer(erste), 100.0);
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

    fn linie(a: (f64, f64), b: (f64, f64)) -> jjcad_core::model::Entity {
        jjcad_core::model::Entity::new(
            1,
            1,
            EntityKind::Line {
                a: jjcad_core::geom::Vec2::new(a.0, a.1),
                b: jjcad_core::geom::Vec2::new(b.0, b.1),
            },
        )
    }

    fn wand(a: (f64, f64), b: (f64, f64)) -> jjcad_core::model::Entity {
        jjcad_core::model::Entity::new(
            1,
            1,
            EntityKind::Wall {
                a: jjcad_core::geom::Vec2::new(a.0, a.1),
                b: jjcad_core::geom::Vec2::new(b.0, b.1),
                thickness: 240.0,
                height: 2600.0,
                material: String::new(),
                hatch: None,
                align: jjcad_core::style::WallAlign::Center,
            },
        )
    }

    #[test]
    fn linie_zeigt_delta_x_und_delta_y_zwischen_den_basispunkten() {
        // (1,00; 2,00) → (4,00; 0,50): Delta X = +3,00 m, Delta Y = −1,50 m.
        let z = zeilen(&linie((1000.0, 2000.0), (4000.0, 500.0)), Unit::M);
        let namen: Vec<&str> = z.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(namen, vec!["Länge", "Delta X", "Delta Y"]);
        assert_eq!(z[1].1, "3,00 m");
        assert_eq!(z[2].1, "-1,50 m");
    }

    #[test]
    fn wand_zeigt_delta_x_und_delta_y_zwischen_den_basispunkten() {
        // (0,00; 0,00) → (0,00; 1,50): Delta X = 0, Delta Y = +1,50 m.
        let z = zeilen(&wand((0.0, 0.0), (0.0, 1500.0)), Unit::M);
        let wert = |name: &str| {
            z.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, w)| w.clone())
                .unwrap_or_else(|| panic!("Zeile {name} fehlt"))
        };
        assert_eq!(wert("Delta X"), "0,00 m");
        assert_eq!(wert("Delta Y"), "1,50 m");
        // Länge der Basislinie bleibt die Strecke — Delta ist kein Ersatz.
        assert_eq!(wert("Länge"), "1,50 m");
    }

    #[test]
    fn delta_zeilen_nur_bei_linie_und_wand() {
        // Eine Polylinie hat keinen einzelnen Basispunkt-Paar-Abstand.
        let e = poly(&[(0.0, 0.0), (1000.0, 0.0), (1000.0, 500.0)], false);
        let z = zeilen(&e, Unit::M);
        assert_eq!(z.iter().filter(|(n, _)| n.starts_with("Delta")).count(), 0);
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
            enthaelt(&texte, "Textgröße"),
            "während der Eingabe fehlt die Textgröße: {texte:?}"
        );
        // Dasselbe Panel wie beim Erzeugen: Kopfzeile und alle Zeilen.
        assert!(
            enthaelt(&texte, "gilt für das nächste Element"),
            "die Kopfzeile des Erzeugen-Panels fehlt: {texte:?}"
        );
        assert!(
            enthaelt(&texte, "Layer"),
            "während der Eingabe fehlt der Layer: {texte:?}"
        );
        // Schriftart steht während der Eingabe zur Wahl (T0034).
        assert!(
            enthaelt(&texte, "Schriftart"),
            "während der Eingabe fehlt die Schriftart: {texte:?}"
        );
    }

    /// Die Schriftart des neuen Textes kommt in das Dokument (T0034): Auswahl
    /// im Panel, Wert im erzeugten Text.
    #[test]
    fn schriftart_kommt_am_text_an() {
        let mut app = app_mm();
        app.options.text_font = jjcad_core::model::TextFont::Mono;
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        let _ = panel_texte(&mut app, 2.0);
        // Text tippen und mit Esc übernehmen.
        {
            let t = app.ui.texteingabe.as_mut().expect("Eingabe läuft");
            t.text = "Prüfung".into();
        }
        crate::textedit::uebernehmen_fuer_test(&mut app);
        let font = app
            .doc()
            .ents()
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Text { font, .. } => Some(*font),
                _ => None,
            })
            .expect("Text angelegt");
        assert_eq!(font, jjcad_core::model::TextFont::Mono);
    }

    /// Auswahl eines Textes: keine Zeilen für Stärke, Linientyp oder einen
    /// leeren Abschnitt „Geometrie" — dafür Text, Textgröße und die
    /// Umschalter der Auszeichnung (T0032).
    #[test]
    fn ausgewaehlter_text_hat_keine_linien_zeilen_und_keine_leere_geometrie() {
        let mut app = app_mm();
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1000, 500], "text": "Wohnen", "height": 250}),
            )
            .unwrap();
        let id = app.doc().ents()[0].id;
        app.session_mut().select(vec![id]);
        let texte = panel_texte(&mut app, 2.0);
        for unerwuenscht in ["Stärke", "Linientyp", "Geometrie", "Fett", "Kursiv"] {
            assert!(
                !enthaelt(&texte, unerwuenscht),
                "{unerwuenscht} gehört nicht ins Text-Panel: {texte:?}"
            );
        }
        for erwuenscht in ["Text", "Textgröße", "Drehung", "Ausrichtung", "F", "K"] {
            assert!(
                enthaelt(&texte, erwuenscht),
                "{erwuenscht} fehlt im Text-Panel: {texte:?}"
            );
        }
    }

    /// Umschalter im Panel: nur die markierten Zeichen werden ausgezeichnet,
    /// ohne Markierung der ganze Text (T0032).
    #[test]
    fn umschalter_zeichnet_nur_die_markierung_aus() {
        let mut app = app_mm();
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [0, 0], "text": "Raum Wohnen", "height": 250}),
            )
            .unwrap();
        let id = app.doc().ents()[0].id;

        // Markierung „Wohnen" (Zeichen 5–11) — wie sie egui im Textfeld liefert.
        let (von, bis) = zeichenbereich(Some((5, 11)), "Raum Wohnen".chars().count());
        assert_eq!((von, bis), (5, 11));
        auszeichnen(&mut app, id, von, bis, "bold", true);

        let EntityKind::Text { runs, .. } = &app.doc().entity(id).unwrap().kind else {
            panic!("Text erwartet");
        };
        assert_eq!(runs.len(), 2, "zwei Läufe erwartet: {runs:?}");
        assert_eq!(runs[0].text, "Raum ");
        assert!(!runs[0].bold, "Raum darf nicht fett werden");
        assert_eq!(runs[1].text, "Wohnen");
        assert!(runs[1].bold, "Wohnen muss fett sein");

        // Ohne Markierung gilt der ganze Text.
        let (von, bis) = zeichenbereich(None, "Raum Wohnen".chars().count());
        assert_eq!((von, bis), (0, 11));
    }

    /// Werkzeug „Text": Stärke und Linientyp entfallen schon beim Erzeugen.
    #[test]
    fn textwerkzeug_zeigt_keine_linien_zeilen() {
        let mut app = app_mm();
        app.start_tool("draw.text");
        let _ = panel_texte(&mut app, 3.0);
        let texte = panel_texte(&mut app, 3.1);
        assert!(!enthaelt(&texte, "Stärke"), "Stärke beim Text: {texte:?}");
        assert!(!enthaelt(&texte, "Linientyp"), "Linientyp beim Text: {texte:?}");
        assert!(enthaelt(&texte, "Textgröße"), "Textgröße fehlt: {texte:?}");
        assert!(enthaelt(&texte, "Farbe"), "Farbe fehlt: {texte:?}");
    }

    /// Änderungen im Panel landen in der Eingabe — und damit im Ergebnis.
    ///
    /// Das Panel schreibt in die Vorgaben, `props::eingabe_spiegeln` reicht sie
    /// an den laufenden Editor weiter (früher schrieb die Matrix direkt in den
    /// Editor — deshalb gab es ein zweites Panel).
    #[test]
    fn aenderung_im_panel_gilt_fuer_die_eingabe() {
        let mut app = app_mm();
        app.options.text_height = 250.0;
        app.options.text_align = TextAlignH::Right;
        app.options.text_valign = TextAlignV::Top;
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
        }
        // Panel zeichnen: es spiegelt die Vorgaben in die Eingabe.
        let _ = panel_texte(&mut app, 2.0);
        {
            let t = app.ui.texteingabe.as_ref().expect("Eingabe läuft");
            assert_eq!(
                (t.align, t.valign),
                (TextAlignH::Right, TextAlignV::Top),
                "Panelwerte kommen nicht in der Eingabe an"
            );
        }
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

    /// Die Wand zeigt den relativen Abstand ihrer Basispunkte im Panel —
    /// ohne Scrollen, direkt unter der Kennzahlenzeile (T0053).
    #[test]
    fn wandpanel_zeigt_delta_x_und_delta_y() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "draw.wall",
                &json!({ "a": [1.0, 2.0], "b": [4.0, 0.5], "thickness": 0.24 }),
            )
            .expect("Wand");
        let id = app.doc().entities.last().expect("Wand fehlt").id;
        app.session_mut().select(vec![id]);
        let texte = panel_texte(&mut app, 1.0);
        assert!(enthaelt(&texte, "Delta X"), "Delta X fehlt: {texte:?}");
        assert!(enthaelt(&texte, "Delta Y"), "Delta Y fehlt: {texte:?}");
        assert!(enthaelt(&texte, "3,00 m"), "Delta X falsch: {texte:?}");
        assert!(enthaelt(&texte, "-1,50 m"), "Delta Y falsch: {texte:?}");
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

/// Linienstärken-Liste nach DIN EN ISO 128-2:2023-06, Abschnitt 5.1 (T0124).
///
/// Die Liste ist die vollständige Normreihe 1 : √2 — neun Werte, aufsteigend,
/// ohne Dubletten. Beschriftet wird nur die Zahl mit Einheit; Klassennamen wie
/// „dünn/mittel/dick“ gibt es nicht mehr, weil sie eine Liniengruppe meinen
/// (Tabelle B.2) und nicht eine einzelne Breite.
mod tests_linienstaerken {
    use super::*;
    use jjcad_core::session::Session;

    #[test]
    fn liste_ist_die_normreihe_aufsteigend_ohne_dubletten() {
        assert_eq!(
            WEIGHT_PRESETS,
            [0.13, 0.18, 0.25, 0.35, 0.50, 0.70, 1.00, 1.40, 2.00],
            "die neun Breiten aus DIN EN ISO 128-2, Abschnitt 5.1"
        );
        for paar in WEIGHT_PRESETS.windows(2) {
            assert!(
                paar[1] > paar[0],
                "Liste muss aufsteigend sein: {:?}",
                WEIGHT_PRESETS
            );
        }
    }

    #[test]
    fn beschriftung_ist_zahl_und_einheit_ohne_klassennamen() {
        assert_eq!(weight_text(0.18), "0,18 mm");
        assert_eq!(weight_text(0.13), "0,13 mm");
        assert_eq!(weight_text(2.0), "2,00 mm");
        // Kein Listenwert: der Ist-Wert wird weiter gezeigt, nicht verschluckt.
        assert_eq!(weight_text(0.2), "0,20 mm");
        assert_eq!(weight_text(0.37), "0,37 mm");
    }

    /// Ein Objekt mit der schmalsten Normbreite wird angelegt, gespeichert und
    /// wieder geladen — die Anzeige zeigt danach weiter `0,13 mm` (T0124).
    #[test]
    fn objekt_mit_013_mm_ueberlebt_speichern_und_laden() {
        let mut session = Session::new();
        session
            .execute("doc.settings", &json!({"input_unit": "mm"}))
            .expect("Einheit");
        let id = session
            .execute(
                "draw.line",
                &json!({
                    "a": [0.0, 0.0], "b": [500.0, 0.0],
                    "lineweight": 0.13, "layer": "Wände"
                }),
            )
            .expect("Linie")["created"][0]
            .as_u64()
            .expect("Id");
        let e = session.doc.entity(id).expect("Linie fehlt");
        assert!((session.doc.effective_line_weight(e) - 0.13).abs() < 1e-9);

        let text = jjcad_core::persist::to_json(&session.doc).expect("JSON");
        let wieder = jjcad_core::persist::from_json(&text).expect("zurückgelesen");
        let e = wieder.entity(id).expect("Linie nach dem Laden");
        let w = wieder.effective_line_weight(e);
        assert!((w - 0.13).abs() < 1e-9, "gespeicherte Breite: {w}");
        assert_eq!(weight_text(w), "0,13 mm");
    }
}

#[cfg(test)]
mod tests_farbgruppen {
    use super::*;

    /// Die 19 Werte samt Gruppenzugehörigkeit (T0183).
    #[test]
    fn farbgruppen_haben_die_vereinbarten_werte() {
        let erwartet: [(&str, &[u32]); 3] = [
            (
                "gui.panels.farbgruppe_bauteile",
                &[0x000000, 0xFF0000, 0xFFFF00],
            ),
            (
                "gui.panels.farbgruppe_gestaltung",
                &[
                    0xBE3226, 0xD9822B, 0xEBCE2E, 0x7A5637, 0x2E7D4F, 0x2A9D8F, 0x3FA6C2,
                    0x2E5FA3, 0x6A4C93, 0xB3508A,
                ],
            ),
            (
                "gui.panels.farbgruppe_neutral",
                &[0x000000, 0x404040, 0x808080, 0xBFBFBF, 0xD9D9D9, 0xFFFFFF],
            ),
        ];
        assert_eq!(FARBGRUPPEN.len(), 3, "drei Gruppen");
        for (g, (titel, werte)) in FARBGRUPPEN.iter().zip(erwartet) {
            assert_eq!(g.titel, titel);
            let ist: Vec<u32> = g.farben.iter().map(|(_, _, rgb)| *rgb).collect();
            assert_eq!(ist, werte, "Gruppe {titel}");
        }
        let anzahl: usize = FARBGRUPPEN.iter().map(|g| g.farben.len()).sum();
        assert_eq!(anzahl, 19, "3 + 10 + 6 Einträge");
    }

    fn gestaltung(name: &str) -> u32 {
        FARBGRUPPEN[1]
            .farben
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, _, rgb)| *rgb)
            .expect("Eintrag in der Gruppe Gestaltung")
    }

    /// Der Farbkreis ist geschlossen: Gelb heller, Cyan und Magenta dazu.
    #[test]
    fn gestaltungsfarben_schliessen_den_farbkreis() {
        assert_eq!(gestaltung("gui.panels.farbe_gelb"), 0xEBCE2E, "Gelb heller");
        assert_eq!(gestaltung("gui.panels.farbe_cyan"), 0x3FA6C2, "Cyan");
        assert_eq!(gestaltung("gui.panels.farbe_magenta"), 0xB3508A, "Magenta");
        assert_eq!(gestaltung("gui.panels.farbe_braun"), 0x7A5637, "Braun");
    }

    /// `#000000` steht bewusst in zwei Gruppen; der Namensrückgriff nimmt den
    /// ersten Treffer und zeigt „Schwarz · Bestand“.
    #[test]
    fn schwarz_steht_in_zwei_gruppen() {
        let treffer: Vec<&str> = FARBGRUPPEN
            .iter()
            .filter(|g| g.farben.iter().any(|(_, _, rgb)| *rgb == 0x000000))
            .map(|g| g.titel)
            .collect();
        assert_eq!(
            treffer,
            vec![
                "gui.panels.farbgruppe_bauteile",
                "gui.panels.farbgruppe_neutral"
            ],
            "Schwarz in Bauteilen und Neutraltönen"
        );
        assert_eq!(
            preset_name("#000000").as_deref(),
            Some("Schwarz · Bestand")
        );
    }

    /// Der Layer-Farbwähler nimmt dieselbe Liste, das doppelte Schwarz aber
    /// nur einmal — zwei gleiche Kreise wären nicht zu unterscheiden.
    #[test]
    fn layer_palette_hat_keine_dubletten() {
        let palette = farbpalette();
        assert_eq!(palette.len(), 18, "19 Einträge minus doppeltes Schwarz");
        let mut hex: Vec<String> = palette.iter().map(|(_, c)| c.to_hex()).collect();
        hex.sort();
        hex.dedup();
        assert_eq!(hex.len(), 18, "kein Hexwert doppelt");
    }

    /// Jede Vorgabe und jede Überschrift steht im Katalog (sonst zeigt das
    /// Panel den Schlüssel statt des Namens).
    #[test]
    fn jede_vorgabe_hat_einen_katalogeintrag() {
        for g in FARBGRUPPEN.iter() {
            assert!(jjcad_core::i18n::deutsch_hat(g.titel), "{}", g.titel);
            for (name, meaning, _) in g.farben.iter() {
                assert!(jjcad_core::i18n::deutsch_hat(name), "{name}");
                if !meaning.is_empty() {
                    assert!(jjcad_core::i18n::deutsch_hat(meaning), "{meaning}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Layer-Manager mit Plänen (T0202)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests_plaene {
    use super::*;

    /// Ein eigenes Bild des Layer-Managers im übergebenen Kontext.
    fn rahmen(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> Vec<(String, Pos2)> {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(232.0, 900.0))),
            events,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::SidePanel::left("probe")
                .exact_width(232.0)
                .show(ctx, |ui| layers(app, ui));
        });
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.pos)),
                _ => None,
            })
            .collect()
    }

    /// Lage der Zeile mit diesem Text (Mitte der Zeilenbox).
    fn mitte(texte: &[(String, Pos2)], text: &str) -> Pos2 {
        let (_, pos) = texte
            .iter()
            .find(|(t, _)| t.trim() == text)
            .unwrap_or_else(|| panic!("Text „{text}“ fehlt im Bild: {texte:?}"));
        Pos2::new(pos.x, pos.y + 8.0)
    }

    /// Klickt an eine Stelle: erst ein Bild mit Zeigerlage (sonst kennt egui
    /// kein Widget darunter), dann Druck, dann Loslassen.
    fn klick(ctx: &egui::Context, app: &mut App, pos: Pos2) {
        rahmen(
            ctx,
            app,
            vec![egui::Event::PointerMoved(pos)],
        );
        for gedrueckt in [true, false] {
            rahmen(
                ctx,
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: gedrueckt,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }

    /// T0202 (Turn 5): Der Farbpunkt hat keinen hellen Grund mehr — nur einen
    /// Haarring. Bei dunklen Ebenenfarben ist er hell, sonst dunkel.
    #[test]
    fn farbpunkt_ohne_hellen_grund() {
        let dunkel = farbpunkt_ring(Color::rgb(35, 38, 44));
        assert_eq!(dunkel, Palette::text_faint(), "dunkle Ebene: heller Ring");
        let hell = farbpunkt_ring(Color::rgb(190, 70, 60));
        assert_eq!(hell, Color32::from_black_alpha(80), "helle Ebene: dunkler Ring");
    }

    #[test]
    fn neue_zeichnung_zeigt_einen_plan_mit_vier_layern() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let alle: Vec<&str> = texte.iter().map(|(t, _)| t.trim()).collect();
        for erwartet in [
            "Plan 1",
            "Zeichnung",
            "Architektur",
            "Beschriftung",
            "Blöcke",
            "Plan hinzufügen",
            "Layer hinzufügen",
        ] {
            assert!(
                alle.contains(&erwartet),
                "„{erwartet}“ fehlt in der Anzeige: {alle:?}"
            );
        }
        // Ein Plan → eine Kopfzeile und eine Zeile „Layer hinzufügen“.
        assert_eq!(alle.iter().filter(|t| **t == "Plan 1").count(), 1);
        assert_eq!(alle.iter().filter(|t| **t == "Layer hinzufügen").count(), 1);
    }

    /// T0212: Der Kopf des Layer-Panels trägt nur noch „Neuer Layer“ und
    /// „Einklappen“ — der frühere Knopf „Layer verwalten“ schloss das Panel
    /// nur und ist entfallen. Kein Kopfknopf darf das Panel schließen.
    #[test]
    fn kopf_hat_keinen_schliessen_knopf() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        rahmen(&ctx, &mut app, Vec::new());
        assert_eq!(
            app.ui.last_layer_kopf.len(),
            2,
            "der Kopf trägt genau zwei Knöpfe"
        );
        let knoepfe = app.ui.last_layer_kopf.clone();

        // Beide drücken: keiner darf `layers_open` umschalten (früher saß dort
        // der Knopf „Layer verwalten“, der das Panel nur schloss).
        for r in knoepfe {
            klick(&ctx, &mut app, r.center());
            assert!(
                app.ui.layers_open,
                "kein Kopfknopf darf das Panel schließen"
            );
        }
        // Die beiden Funktionen bleiben: neuer Layer und einklappen.
        assert!(
            app.ui.new_layer_dialog.is_some(),
            "der Plus-Knopf legt weiter einen Layer an"
        );
        assert!(
            app.ui.layers_collapsed,
            "der Pfeil klappt das Panel weiter ein"
        );
    }

    #[test]
    fn einklappen_versteckt_die_layer_des_plans() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let kopf = mitte(&texte, "Plan 1");
        assert!(!app.doc().plans[0].collapsed);
        // Das Einklapp-Dreieck sitzt links in der Kopfzeile.
        klick(&ctx, &mut app, Pos2::new(kopf.x - 11.0, kopf.y));
        assert!(
            app.doc().plans[0].collapsed,
            "der Klick auf das Dreieck muss den Plan einklappen"
        );
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let alle: Vec<&str> = texte.iter().map(|(t, _)| t.trim()).collect();
        assert!(!alle.contains(&"Zeichnung"), "Layer bleiben sichtbar");
        assert!(alle.contains(&"Plan 1"), "die Kopfzeile bleibt");

        // Und wieder auf: derselbe Klick.
        let kopf = mitte(&texte, "Plan 1");
        klick(&ctx, &mut app, Pos2::new(kopf.x - 11.0, kopf.y));
        assert!(!app.doc().plans[0].collapsed);
    }

    #[test]
    fn plan_hinzufuegen_legt_einen_zweiten_plan_an() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let zeile = mitte(&texte, "Plan hinzufügen");
        klick(&ctx, &mut app, Pos2::new(zeile.x + 40.0, zeile.y));
        assert!(
            app.ui.new_plan_dialog.is_some(),
            "der Klick muss das Eingabefeld für den neuen Plan öffnen"
        );

        // Das Feld schlägt „Plan 2“ vor; „OK“ bestätigt.
        let texte = rahmen(&ctx, &mut app, Vec::new());
        assert!(
            texte.iter().any(|(t, _)| t.trim() == "Plan 2"),
            "Vorschlag fehlt: {texte:?}"
        );
        let ok = mitte(&texte, "OK");
        klick(&ctx, &mut app, Pos2::new(ok.x + 4.0, ok.y));
        assert_eq!(app.doc().plans.len(), 2, "zweiter Plan fehlt");
        assert_eq!(app.doc().layers.len(), 8, "vier Layer je Plan");
        assert_eq!(app.doc().plans[1].name, "Plan 2");
    }

    /// Rechter Rand aller gemalten Texte im Panel — ein Inhalt, der breiter
    /// als das Panel ist, schiebt es auf (T0206: „schwarzer Balken“).
    fn rechter_rand(ctx: &egui::Context, app: &mut App) -> f32 {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(232.0, 900.0))),
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::SidePanel::left("probe")
                .exact_width(232.0)
                .show(ctx, |ui| layers(app, ui));
        });
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.pos.x + t.galley.size().x),
                _ => None,
            })
            .fold(0.0_f32, f32::max)
    }

    /// T0206: Das Umbenennen steht **in** der Layerzeile — keine zusätzliche
    /// Zeile über der Liste, kein breiteres Panel.
    #[test]
    fn umbenennen_steht_in_der_layerzeile() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let vorher = rahmen(&ctx, &mut app, Vec::new());
        let plan_y = mitte(&vorher, "Plan 1").y;
        let id = app.doc().layers[0].id;
        app.ui.rename_layer = Some((id, "Wände".into()));
        app.ui.rename_layer_fokus = false;

        let texte = rahmen(&ctx, &mut app, Vec::new());
        assert!(
            app.ui.rename_layer.is_some(),
            "das Feld muss offen bleiben, bis der Nutzer bestätigt"
        );
        assert_eq!(
            mitte(&texte, "Plan 1").y,
            plan_y,
            "das Umbenennen darf keine neue Zeile über der Liste öffnen"
        );
        assert_eq!(
            texte.iter().filter(|(t, _)| t.trim() == "Wände").count(),
            1,
            "der Name steht genau einmal im Bild (im Feld): {texte:?}"
        );
        assert!(
            ctx.memory(|m| m.focused()).is_some(),
            "das Feld muss den Tastaturfokus haben"
        );
        assert!(
            rechter_rand(&ctx, &mut app) <= 232.0,
            "der Inhalt darf das Panel nicht verbreitern"
        );
    }

    /// T0206 (Turn 3): Das Feld füllt die Zeile ganz — die Liste behält
    /// während des Umbenennens ihre Länge, keine Zeile wandert.
    #[test]
    fn umbenennen_verschiebt_die_liste_nicht() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let vorher = rahmen(&ctx, &mut app, Vec::new());
        let naechste_y = mitte(&vorher, "Architektur").y;
        let id = app.doc().layers[0].id;
        app.ui.rename_layer = Some((id, "Zeichnung".into()));
        app.ui.rename_layer_fokus = false;
        let texte = rahmen(&ctx, &mut app, Vec::new());
        assert_eq!(
            mitte(&texte, "Architektur").y,
            naechste_y,
            "die Zeilen unter dem Feld dürfen nicht wandern"
        );
    }

    /// T0206: Ein Rechtsklick öffnet **ein** Menü — nicht zusätzlich den
    /// Farbwähler, der das Menü zudeckte.
    #[test]
    fn rechtsklick_oeffnet_nur_das_menue() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let zeile = mitte(&texte, "Zeichnung");
        rahmen(&ctx, &mut app, vec![egui::Event::PointerMoved(zeile)]);
        for gedrueckt in [true, false] {
            rahmen(
                &ctx,
                &mut app,
                vec![egui::Event::PointerButton {
                    pos: zeile,
                    button: egui::PointerButton::Secondary,
                    pressed: gedrueckt,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert!(
            app.ui.color_popup.is_none(),
            "der Rechtsklick darf den Farbwähler nicht sofort öffnen"
        );
        // Das Menü wird im nächsten Bild gezeichnet.
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let alle: Vec<&str> = texte.iter().map(|(t, _)| t.trim()).collect();
        assert!(
            alle.contains(&"Farbe ändern …"),
            "Menüeintrag „Farbe ändern …“ fehlt: {alle:?}"
        );
    }

    /// T0206: Der Linksklick auf den Farbpunkt ändert die Farbe weiterhin direkt.
    #[test]
    fn linksklick_auf_das_farbicon_oeffnet_den_farbwaehler() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let zeile = mitte(&texte, "Zeichnung");
        let id = app.doc().layers[0].id;
        // Der Farbpunkt sitzt 12 px links vom Namen (left + 27 gegen left + 39).
        klick(&ctx, &mut app, Pos2::new(zeile.x - 12.0, zeile.y));
        assert_eq!(
            app.ui.color_popup,
            Some(id),
            "der Klick auf den Farbpunkt muss den Farbwähler öffnen"
        );
    }

    #[test]
    fn plan_auge_blendet_den_ganzen_plan_aus() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let texte = rahmen(&ctx, &mut app, Vec::new());
        let kopf = mitte(&texte, "Plan 1");
        // Rechte Seite der Kopfzeile: Auge bei right − 40, Schloss right − 19.
        // Die Panelbreite ist 232 px, das Auge liegt also bei x ≈ 192.
        klick(&ctx, &mut app, Pos2::new(kopf.x + 160.0, kopf.y));
        assert!(
            !app.doc().plans[0].visible,
            "das Auge der Kopfzeile muss den Plan ausblenden"
        );
        let layer = app.doc().layers[0].id;
        assert!(!app.doc().layer_sichtbar(layer));
        // Die Layerzeile bleibt stehen (ausgegraut) — der Layer selbst auch an.
        assert!(app.doc().layer(layer).unwrap().visible);
    }
}

#[cfg(test)]
mod tests_layer_loeschen {
    //! T0213: Ein Layer mit Objekten fragt vor dem Löschen nach; ein leerer
    //! Layer geht direkt an die Action.
    use super::*;

    fn app() -> App {
        let ctx = egui::Context::default();
        App::with_ctx(ctx)
    }

    #[test]
    fn leerer_layer_wird_ohne_rueckfrage_geloescht() {
        let app = app();
        let id = app.doc().layer_id_by_name("Zeichnung").expect("Zeichnung");
        assert!(loeschen_ziel(app.doc(), id).is_none());
    }

    #[test]
    fn layer_mit_objekten_bekommt_ein_ziel_im_selben_plan() {
        let mut app = app();
        let id = app.doc().layer_id_by_name("Zeichnung").expect("Zeichnung");
        app.session_mut()
            .execute(
                "draw.line",
                &json!({"a": [0.0, 0.0], "b": [1000.0, 0.0], "layer": id}),
            )
            .expect("Linie");
        let ziel = loeschen_ziel(app.doc(), id).expect("Rückfrage mit Ziel");
        let ziel_layer = app.doc().layer(ziel).expect("Ziellayer");
        let plan = app.doc().layer(id).expect("Quelle").plan;
        assert_eq!(ziel_layer.plan, plan, "das Ziel liegt im selben Plan");
        assert_ne!(ziel, id);
    }

    #[test]
    fn letzter_layer_eines_plans_bekommt_keine_rueckfrage() {
        let mut app = app();
        let id = app.doc().layer_id_by_name("Zeichnung").expect("Zeichnung");
        app.session_mut()
            .execute(
                "draw.line",
                &json!({"a": [0.0, 0.0], "b": [1000.0, 0.0], "layer": id}),
            )
            .expect("Linie");
        // Alle anderen Layer des Plans entfernen, das Objekt vorher umhängen.
        let plan = app.doc().layer(id).expect("Quelle").plan;
        let andere: Vec<Id> = app
            .doc()
            .layers_of_plan(plan)
            .into_iter()
            .map(|l| l.id)
            .filter(|l| *l != id)
            .collect();
        for andere in andere {
            app.session_mut()
                .execute("layer.delete", &json!({"layer": andere}))
                .expect("leerer Layer geht");
        }
        assert_eq!(app.doc().layers_of_plan(plan).len(), 1);
        assert!(
            loeschen_ziel(app.doc(), id).is_none(),
            "ohne anderes Ziel im Plan keine Rückfrage"
        );
    }
}

