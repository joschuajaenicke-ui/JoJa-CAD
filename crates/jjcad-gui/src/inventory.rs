//! Inventar: Lager (Raster), Block-Manager, Funktionsliste.

use crate::app::App;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::geom::Affine;
use jjcad_core::model::BlockDef;
use jjcad_core::render::{self, DisplayList, Shape as PShape};

pub fn show(app: &mut App, ctx: &egui::Context) {
    if !app.ui.inventory_open {
        return;
    }
    let mut open = true;
    egui::Window::new("inventar")
        .title_bar(false)
        .resizable(true)
        .default_size(Vec2::new(880.0, 600.0))
        .default_pos(Pos2::new(
            (ctx.screen_rect().width() - 880.0) * 0.5,
            (ctx.screen_rect().height() - 620.0) * 0.5,
        ))
        .open(&mut open)
        .frame(theme::dialog_frame())
        .show(ctx, |ui| {
            ui.set_min_size(Vec2::new(820.0, 520.0));
            // Kopf
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                crate::icons::paint(ui.painter(), r, "inventory", Palette::ACCENT);
                ui.label(theme::bold("Inventar", 16.0));
                ui.add_space(10.0);
                for (i, name) in ["Lager", "Blöcke", "Funktionen"].iter().enumerate() {
                    let active = app.ui.inventory_tab == i;
                    let galley = ui.painter().layout_no_wrap(
                        name.to_string(),
                        FontId::proportional(12.5),
                        if active { Palette::TEXT } else { Palette::TEXT_DIM },
                    );
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(galley.size().x + 24.0, 30.0),
                        Sense::click(),
                    );
                    if active {
                        ui.painter().rect_filled(rect, Rounding::same(7.0), Palette::PANEL_ALT);
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                Pos2::new(rect.left() + 8.0, rect.bottom() - 2.0),
                                Pos2::new(rect.right() - 8.0, rect.bottom()),
                            ),
                            Rounding::same(1.0),
                            Palette::ACCENT,
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, Rounding::same(7.0), Palette::PANEL_ALT.gamma_multiply(0.5));
                    }
                    ui.painter().galley(
                        Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0),
                        galley,
                        if active { Palette::TEXT } else { Palette::TEXT_DIM },
                    );
                    if resp.clicked() {
                        app.ui.inventory_tab = i;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, "close", 26.0, false, "Schließen (E)").clicked() {
                        app.close_inventory();
                    }
                });
            });
            ui.add_space(6.0);
            widgets::hline(ui);
            ui.add_space(8.0);

            match app.ui.inventory_tab {
                0 => storage_tab(app, ui),
                1 => blocks_tab(app, ui),
                _ => functions_tab(app, ui),
            }
        });
    if !open {
        app.close_inventory();
    }
}

// ---------------------------------------------------------------------------
// Lager
// ---------------------------------------------------------------------------

fn storage_tab(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(theme::dim(
            "Freies Raster – Elemente und Funktionen zwischenlagern. Auf einen Hotbar-Slot ziehen zum Belegen.",
        ));
    });
    ui.add_space(6.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let cols = 9;
        let size = 58.0;
        let rows = (app.prefs.inventory.len() + cols - 1) / cols;
        for row in 0..rows {
            ui.horizontal(|ui| {
                for col in 0..cols {
                    let idx = row * cols + col;
                    if idx >= app.prefs.inventory.len() {
                        break;
                    }
                    let item = app.prefs.inventory[idx].clone();
                    let active = item
                        .as_ref()
                        .map(|a| app.tool_id() == Some(a.as_str()))
                        .unwrap_or(false);
                    let resp = widgets::slot_button(ui, Vec2::splat(size), item.as_deref(), None, active, false);
                    if resp.clicked() {
                        if let Some(a) = &item {
                            app.activate(ui.ctx(), a);
                        } else if let Some(t) = app.tool.clone() {
                            app.prefs.inventory[idx] = Some(t.action.clone());
                            let _ = app.prefs.save();
                            app.toast("Ins Lager gelegt");
                        } else {
                            app.toast("Werkzeug wählen, dann leeren Lagerplatz anklicken");
                        }
                    }
                    if resp.drag_started() {
                        if let Some(a) = &item {
                            app.ui.slot_drag = Some((usize::MAX, a.clone()));
                        }
                    }
                    resp.context_menu(|ui| {
                        if item.is_some() {
                            if ui.button("Leeren").clicked() {
                                app.prefs.inventory[idx] = None;
                                let _ = app.prefs.save();
                                ui.close_menu();
                            }
                        }
                        if ui.button("Verknüpfen …").clicked() {
                            app.ui.bind_slot = Some(usize::MAX);
                            app.ui.inventory_tab = 2;
                            ui.close_menu();
                        }
                    });
                    if let Some(a) = &item {
                        resp.on_hover_text(
                            jjcad_core::actions::action(a)
                                .map(|d| d.label.to_string())
                                .unwrap_or_else(|| a.clone()),
                        );
                    }
                }
            });
        }
        ui.add_space(8.0);
        if ui.button("+ Weitere Reihe").clicked() {
            for _ in 0..9 {
                app.prefs.inventory.push(None);
            }
            let _ = app.prefs.save();
        }
    });
}

// ---------------------------------------------------------------------------
// Block-Manager
// ---------------------------------------------------------------------------

fn blocks_tab(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Blockbibliothek")
                .size(12.0)
                .color(Palette::TEXT_DIM),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Block aus Auswahl").clicked() {
                app.run_action(ui.ctx(), "block.create");
            }
        });
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let blocks = app.doc().blocks.clone();
        let cats: Vec<String> = {
            let mut v: Vec<String> = blocks.iter().map(|b| b.category.clone()).collect();
            v.sort();
            v.dedup();
            v
        };
        for cat in cats {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(cat.to_uppercase())
                        .size(9.5)
                        .color(Palette::TEXT_FAINT)
                        .strong(),
                );
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, Palette::BORDER_SOFT);
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for b in blocks.iter().filter(|b| b.category == cat) {
                    block_tile(app, ui, b);
                }
            });
        }
    });
}

fn block_tile(app: &mut App, ui: &mut Ui, b: &BlockDef) {
    let w = 104.0;
    let h = 118.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    let hovered = resp.hovered();
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let bg = if hovered { Palette::PANEL_ALT } else { Palette::PANEL };
        p.rect_filled(rect, Rounding::same(8.0), bg);
        p.rect_stroke(
            rect,
            Rounding::same(8.0),
            Stroke::new(1.0, if hovered { Palette::ACCENT.gamma_multiply(0.6) } else { Palette::BORDER_SOFT }),
        );
        let thumb = Rect::from_min_size(rect.min + Vec2::new(10.0, 8.0), Vec2::new(w - 20.0, 62.0));
        draw_block_preview(app, p, thumb, b);
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 28.0),
            egui::Align2::CENTER_CENTER,
            truncate(&b.name, 16),
            FontId::proportional(11.0),
            Palette::TEXT,
        );
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 13.0),
            egui::Align2::CENTER_CENTER,
            if b.source == jjcad_core::model::BlockSource::System { "System" } else { "Projekt" },
            FontId::proportional(9.0),
            Palette::TEXT_FAINT,
        );
    }
    if resp.clicked() {
        app.start_ghost_block(b.id.clone());
    }
    if resp.drag_started() {
        app.ui.slot_drag = Some((usize::MAX, format!("block:{}", b.id)));
    }
    resp.context_menu(|ui| {
        if ui.button("Einfügen").clicked() {
            app.start_ghost_block(b.id.clone());
            ui.close_menu();
        }
        if ui.button("In Lager legen").clicked() {
            if let Some(free) = app.prefs.inventory.iter().position(|x| x.is_none()) {
                app.prefs.inventory[free] = Some(format!("block:{}", b.id));
                let _ = app.prefs.save();
            }
            ui.close_menu();
        }
        if b.source != jjcad_core::model::BlockSource::System && ui.button("Löschen").clicked() {
            let _ = app.session_mut().execute("block.delete", &serde_json::json!({"block": b.id}));
            app.mark_changed();
            ui.close_menu();
        }
    });
    resp.on_hover_text(&b.description);
}

pub fn draw_block_preview(app: &App, p: &egui::Painter, rect: Rect, b: &BlockDef) {
    let bbox = b.bbox();
    if !bbox.is_valid() {
        return;
    }
    let bw = bbox.width().max(1.0) as f32;
    let bh = bbox.height().max(1.0) as f32;
    let k: f32 = ((rect.width() - 4.0) / bw).min((rect.height() - 4.0) / bh);
    let cx = bbox.center().x as f32 * k;
    let cy = bbox.center().y as f32 * k;
    let off = rect.center() - Vec2::new(cx, -cy);
    let w2s = |q: jjcad_core::geom::Vec2| {
        Pos2::new(off.x + q.x as f32 * k, off.y - q.y as f32 * k)
    };
    let mut dl = DisplayList::default();
    let ctx = render::RenderCtx::new(&app.doc());
    for e in &b.entities {
        render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    let col = Palette::TEXT_DIM;
    let stroke = Stroke::new(1.0, col);
    for prim in &dl.prims {
        match &prim.shape {
            PShape::Line(a, bb) => {
                p.line_segment([w2s(*a), w2s(*bb)], stroke);
            }
            PShape::Poly(pts, closed) => {
                let mut v: Vec<Pos2> = pts.iter().map(|q| w2s(*q)).collect();
                if *closed && !v.is_empty() {
                    v.push(v[0]);
                }
                p.add(egui::Shape::line(v, stroke));
            }
            PShape::Circle { c, r } => {
                p.add(crate::canvas::kreis_zug(w2s(*c), *r as f32 * k, stroke));
            }
            PShape::Arc { c, r, a0, a1 } => {
                let arc = jjcad_core::geom::Arc::new(*c, *r, *a0, *a1);
                let pts: Vec<Pos2> = jjcad_core::tessellate::CurveQuality::SCREEN
                    .flatten(&arc, k as f64)
                    .iter()
                    .map(|q| w2s(*q))
                    .collect();
                p.add(egui::Shape::line(pts, stroke));
            }
            PShape::Ellipse { c, rx, ry, rot } => {
                let pts: Vec<Pos2> = (0..=40)
                    .map(|i| {
                        let t = i as f64 / 40.0 * std::f64::consts::TAU;
                        w2s(*c + jjcad_core::geom::Vec2::new(rx * t.cos(), ry * t.sin()).rotated(*rot))
                    })
                    .collect();
                p.add(egui::Shape::line(pts, stroke));
            }
            PShape::Fill { outer, holes } => {
                let v: Vec<Pos2> = outer.iter().map(|q| w2s(*q)).collect();
                p.add(egui::Shape::convex_polygon(
                    v,
                    Palette::TEXT_FAINT.gamma_multiply(0.35),
                    Stroke::NONE,
                ));
                // In der Kachel (wenige Pixel) ist die Insel nicht zu sehen;
                // die Fläche bleibt deshalb der Außenring.
                let _ = holes;
            }
            PShape::Dot { p: q, .. } => {
                p.circle_filled(w2s(*q), 1.5, col);
            }
            PShape::Text(t) => {
                p.text(
                    w2s(t.pos),
                    egui::Align2::CENTER_CENTER,
                    &t.text,
                    FontId::proportional(8.0),
                    Palette::TEXT_FAINT,
                );
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

// ---------------------------------------------------------------------------
// Funktionsliste
// ---------------------------------------------------------------------------

/// Gewählte Funktion aus der Liste (T0072): Steht eine Taste bereit, belegt sie
/// diese — sonst wie bisher den Hotbar-Slot bzw. das Lager. Der eine Weg für
/// Klick und Tests.
///
/// T0074: Ist das Ziel erreicht (Taste oder Hotbar-Slot vergeben), schließt sich
/// die Funktionsliste — sie stand nur für diese eine Zuteilung offen. Ohne
/// Zuteilung (fest verdrahtete Funktion) und beim Lager bleibt sie offen: das
/// Lager füllt der Nutzer im Raster und will die Liste weiter sehen.
pub(crate) fn funktion_gewaehlt(app: &mut App, id: &str) {
    if let Some(key) = app.ui.bind_key.clone() {
        if crate::dialogs::funktion_auf_taste(app, id, &key) {
            app.close_inventory();
        } else {
            app.toast("Funktion · fest belegt");
        }
    } else if let Some(slot) = app.ui.bind_slot.take() {
        if slot == usize::MAX {
            if let Some(free) = app.prefs.inventory.iter().position(|x| x.is_none()) {
                app.prefs.inventory[free] = Some(id.to_string());
                let _ = app.prefs.save();
                app.toast("Ins Lager gelegt");
            }
        } else {
            app.bind_slot_action(slot, Some(id.to_string()));
            app.close_inventory();
        }
    } else {
        app.start_tool(id);
        app.ui.inventory_open = false;
    }
}

fn functions_tab(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(theme::dim("Alle Befehle – anklicken zum Starten, auf Hotbar ziehen zum Belegen."));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_sized(
                [180.0, 22.0],
                egui::TextEdit::singleline(&mut app.ui.search).hint_text("Suchen …"),
            );
        });
    });
    // Ziel des nächsten Klicks: Taste (T0072), Hotbar-Slot oder Lager.
    if let Some(key) = app.ui.bind_key.clone() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Funktion für Taste {} wählen", key.to_uppercase()))
                    .color(Palette::ACCENT)
                    .size(11.5),
            );
            if ui.button("Abbrechen").clicked() {
                app.ui.bind_key = None;
            }
        });
    } else if let Some(slot) = app.ui.bind_slot {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if slot == usize::MAX {
                    "Element für das Lager wählen".to_string()
                } else {
                    format!("Element für Slot {} wählen", slot + 1)
                })
                .color(Palette::ACCENT)
                .size(11.5),
            );
            if ui.button("Abbrechen").clicked() {
                app.ui.bind_slot = None;
            }
        });
    }
    ui.add_space(6.0);
    let search = app.ui.search.to_lowercase();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for group in [
            actions::ActionGroup::Zeichnen,
            actions::ActionGroup::Architektur,
            actions::ActionGroup::Aendern,
            actions::ActionGroup::Bemassung,
            actions::ActionGroup::Bloecke,
            actions::ActionGroup::Blatt,
            actions::ActionGroup::Ansicht,
            actions::ActionGroup::Datei,
        ] {
            let items: Vec<&actions::ActionDef> = actions::actions_in_group(group)
                .into_iter()
                .filter(|a| {
                    search.is_empty()
                        || a.label.to_lowercase().contains(&search)
                        || a.id.to_lowercase().contains(&search)
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(group.label().to_uppercase())
                    .size(9.5)
                    .color(Palette::TEXT_FAINT)
                    .strong(),
            );
            ui.add_space(3.0);
            for a in items {
                // Fest verdrahtete Funktionen (Strg+S, F1 …) lassen sich keiner
                // Taste zuweisen — sie stehen sichtbar, aber ohne Klickziel.
                let fest = app.ui.bind_key.is_some() && !app.prefs.belegung_aenderbar(a.id);
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 28.0),
                    Sense::click_and_drag(),
                );
                if ui.is_rect_visible(rect) {
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, Rounding::same(5.0), Palette::PANEL_ALT);
                    }
                    let (ir, _) = (
                        Rect::from_min_size(rect.min + Vec2::new(6.0, 5.0), Vec2::splat(18.0)),
                        (),
                    );
                    crate::icons::paint(ui.painter(), ir, a.icon, Palette::TEXT_DIM);
                    ui.painter().text(
                        Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        a.label,
                        FontId::proportional(12.0),
                        Palette::TEXT,
                    );
                    if fest {
                        ui.painter().text(
                            Pos2::new(rect.right() - 8.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            "· fest",
                            FontId::proportional(9.5),
                            Palette::TEXT_FAINT,
                        );
                    } else if let Some(k) = app.prefs.key_for(a.id) {
                        ui.painter().text(
                            Pos2::new(rect.right() - 8.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            k.to_uppercase(),
                            FontId::proportional(10.0),
                            Palette::TEXT_FAINT,
                        );
                    }
                    ui.painter().text(
                        Pos2::new(rect.right() - 54.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        &a.hint,
                        FontId::proportional(9.5),
                        Palette::TEXT_FAINT,
                    );
                }
                if resp.clicked() && !fest {
                    funktion_gewaehlt(app, a.id);
                }
                if resp.drag_started() {
                    app.ui.slot_drag = Some((usize::MAX, a.id.to_string()));
                }
                resp.on_hover_text(a.hint);
            }
        }
        ui.add_space(12.0);
        ui.label(theme::dim(format!(
            "{} Blöcke · {} Funktionen · {} Objekte",
            app.doc().blocks.len(),
            actions::ACTIONS.len(),
            app.doc().entities.len()
        )));
    });
}
