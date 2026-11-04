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
        app.ui.last_lager = None;
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
                crate::icons::paint(ui.painter(), r, "inventory", Palette::accent());
                ui.label(theme::bold(jjcad_core::i18n::tr("gui.inventory.inventar"), 16.0));
                ui.add_space(10.0);
                for (i, name) in [
                    jjcad_core::i18n::tr("gui.inventory.lager"),
                    jjcad_core::i18n::tr("gui.inventory.bloecke"),
                    jjcad_core::i18n::tr("gui.inventory.funktionen"),
                ]
                .iter()
                .enumerate()
                {
                    let active = app.ui.inventory_tab == i;
                    let galley = ui.painter().layout_no_wrap(
                        name.to_string(),
                        FontId::proportional(12.5),
                        if active { Palette::text() } else { Palette::text_dim() },
                    );
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(galley.size().x + 24.0, 30.0),
                        Sense::click(),
                    );
                    if active {
                        ui.painter().rect_filled(rect, Rounding::same(7.0), Palette::panel_alt());
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                Pos2::new(rect.left() + 8.0, rect.bottom() - 2.0),
                                Pos2::new(rect.right() - 8.0, rect.bottom()),
                            ),
                            Rounding::same(1.0),
                            Palette::accent(),
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, Rounding::same(7.0), Palette::panel_alt().gamma_multiply(0.5));
                    }
                    ui.painter().galley(
                        Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0),
                        galley,
                        if active { Palette::text() } else { Palette::text_dim() },
                    );
                    if resp.clicked() {
                        app.ui.inventory_tab = i;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, "close", 26.0, false, jjcad_core::i18n::tr("gui.inventory.schliessen_e")).clicked() {
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
            jjcad_core::i18n::tr("gui.inventory.freies_raster_elemente_und_funktionen_zwischenlagern_auf"),
        ));
    });
    ui.add_space(6.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let cols = 9;
        let size = 58.0;
        let rows = (app.prefs.inventory.len() + cols - 1) / cols;
        let mut plaetze: Vec<(usize, Rect)> = Vec::new();
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
                            app.toast(jjcad_core::i18n::tr("gui.inventory.ins_lager_gelegt"));
                        } else {
                            app.toast(jjcad_core::i18n::tr("gui.inventory.werkzeug_waehlen_dann_leeren_lagerplatz_anklicken"));
                        }
                    }
                    // Mittlere Maustaste: Platz leeren — dieselbe Geste wie in
                    // der Hotbar (T0088). Die linke Taste startet weiterhin das
                    // Element; `clicked()` gilt in egui nur der linken.
                    if resp.clicked_by(egui::PointerButton::Middle) && item.is_some() {
                        app.prefs.inventory[idx] = None;
                        let _ = app.prefs.save();
                        app.toast(jjcad_core::i18n::tr("gui.inventory.lager_geleert"));
                    }
                    if resp.drag_started() {
                        if let Some(a) = &item {
                            app.ui.slot_drag = Some((usize::MAX, a.clone()));
                        }
                    }
                    plaetze.push((idx, resp.rect));
                    resp.context_menu(|ui| {
                        if item.is_some() {
                            if ui.button(jjcad_core::i18n::tr("gui.inventory.leeren")).clicked() {
                                app.prefs.inventory[idx] = None;
                                let _ = app.prefs.save();
                                ui.close_menu();
                            }
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.inventory.verknuepfen")).clicked() {
                            app.ui.bind_slot = Some(usize::MAX);
                            app.ui.inventory_tab = 2;
                            ui.close_menu();
                        }
                    });
                    if let Some(a) = &item {
                        resp.on_hover_text(
                            jjcad_core::actions::action(a)
                                .map(|d| d.label().to_string())
                                .unwrap_or_else(|| a.clone()),
                        );
                    }
                }
            });
        }
        app.ui.last_lager = Some(plaetze);
        ui.add_space(8.0);
        if ui.button(jjcad_core::i18n::tr("gui.inventory.weitere_reihe")).clicked() {
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
            RichText::new(jjcad_core::i18n::tr("gui.inventory.blockbibliothek"))
                .size(12.0)
                .color(Palette::text_dim()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(jjcad_core::i18n::tr("gui.inventory.block_aus_auswahl")).clicked() {
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
                        .color(Palette::text_faint())
                        .strong(),
                );
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, Palette::border_soft());
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
        let bg = if hovered { Palette::panel_alt() } else { Palette::panel() };
        p.rect_filled(rect, Rounding::same(8.0), bg);
        p.rect_stroke(
            rect,
            Rounding::same(8.0),
            Stroke::new(1.0, if hovered { Palette::accent().gamma_multiply(0.6) } else { Palette::border_soft() }),
        );
        let thumb = Rect::from_min_size(rect.min + Vec2::new(10.0, 8.0), Vec2::new(w - 20.0, 62.0));
        draw_block_preview(app, p, thumb, b);
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 28.0),
            egui::Align2::CENTER_CENTER,
            truncate(&b.name, 16),
            FontId::proportional(11.0),
            Palette::text(),
        );
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 13.0),
            egui::Align2::CENTER_CENTER,
            if b.source == jjcad_core::model::BlockSource::System { jjcad_core::i18n::tr("gui.inventory.system") } else { jjcad_core::i18n::tr("gui.inventory.projekt") },
            FontId::proportional(9.0),
            Palette::text_faint(),
        );
    }
    if resp.clicked() {
        app.start_ghost_block(b.id.clone());
    }
    if resp.drag_started() {
        app.ui.slot_drag = Some((usize::MAX, format!("block:{}", b.id)));
    }
    resp.context_menu(|ui| {
        if ui.button(jjcad_core::i18n::tr("gui.inventory.einfuegen")).clicked() {
            app.start_ghost_block(b.id.clone());
            ui.close_menu();
        }
        if ui.button(jjcad_core::i18n::tr("gui.inventory.in_lager_legen")).clicked() {
            if let Some(free) = app.prefs.inventory.iter().position(|x| x.is_none()) {
                app.prefs.inventory[free] = Some(format!("block:{}", b.id));
                let _ = app.prefs.save();
            }
            ui.close_menu();
        }
        if b.source != jjcad_core::model::BlockSource::System && ui.button(jjcad_core::i18n::tr("gui.inventory.loeschen")).clicked() {
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
    let col = Palette::text_dim();
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
            PShape::EllipseArc { c, rx, ry, rot, a0, a1 } => {
                let el = jjcad_core::geom::Ellipse::new(*c, *rx, *ry, *rot);
                let pts: Vec<Pos2> = el
                    .arc_points(*a0, *a1, 0.3 / (k as f64).max(1e-9))
                    .iter()
                    .map(|q| w2s(*q))
                    .collect();
                p.add(egui::Shape::line(pts, stroke));
            }
            PShape::Fill { outer, holes } => {
                let v: Vec<Pos2> = outer.iter().map(|q| w2s(*q)).collect();
                p.add(egui::Shape::convex_polygon(
                    v,
                    Palette::text_faint().gamma_multiply(0.35),
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
                    Palette::text_faint(),
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
            app.toast(jjcad_core::i18n::tr("gui.inventory.funktion_fest_belegt"));
        }
    } else if let Some(slot) = app.ui.bind_slot.take() {
        if slot == usize::MAX {
            if let Some(free) = app.prefs.inventory.iter().position(|x| x.is_none()) {
                app.prefs.inventory[free] = Some(id.to_string());
                let _ = app.prefs.save();
                app.toast(jjcad_core::i18n::tr("gui.inventory.ins_lager_gelegt"));
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
        ui.label(theme::dim(jjcad_core::i18n::tr("gui.inventory.alle_befehle_anklicken_zum_starten_auf_hotbar_ziehen_zum")));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_sized(
                [180.0, 22.0],
                egui::TextEdit::singleline(&mut app.ui.search).hint_text(jjcad_core::i18n::tr("gui.inventory.suchen")),
            );
        });
    });
    // Ziel des nächsten Klicks: Taste (T0072), Hotbar-Slot oder Lager.
    if let Some(key) = app.ui.bind_key.clone() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(jjcad_core::i18n::t_args_owned("gui.inventory.funktion_fuer_taste_x_waehlen", &[("wert1", format!("{}", key.to_uppercase()))]))
                    .color(Palette::accent())
                    .size(11.5),
            );
            if ui.button(jjcad_core::i18n::tr("gui.inventory.abbrechen")).clicked() {
                app.ui.bind_key = None;
            }
        });
    } else if let Some(slot) = app.ui.bind_slot {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if slot == usize::MAX {
                    jjcad_core::i18n::tr("gui.inventory.element_fuer_das_lager_waehlen").to_string()
                } else {
                    jjcad_core::i18n::t_args_owned("gui.inventory.element_fuer_slot_x_waehlen", &[("wert1", format!("{}", slot + 1))])
                })
                .color(Palette::accent())
                .size(11.5),
            );
            if ui.button(jjcad_core::i18n::tr("gui.inventory.abbrechen")).clicked() {
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
                        || a.label().to_lowercase().contains(&search)
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
                    .color(Palette::text_faint())
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
                        ui.painter().rect_filled(rect, Rounding::same(5.0), Palette::panel_alt());
                    }
                    let (ir, _) = (
                        Rect::from_min_size(rect.min + Vec2::new(6.0, 5.0), Vec2::splat(18.0)),
                        (),
                    );
                    crate::icons::paint(ui.painter(), ir, a.icon, Palette::text_dim());
                    ui.painter().text(
                        Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        a.label(),
                        FontId::proportional(12.0),
                        Palette::text(),
                    );
                    if fest {
                        ui.painter().text(
                            Pos2::new(rect.right() - 8.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            jjcad_core::i18n::tr("gui.inventory.fest"),
                            FontId::proportional(9.5),
                            Palette::text_faint(),
                        );
                    } else if let Some(k) = app.prefs.key_for(a.id) {
                        ui.painter().text(
                            Pos2::new(rect.right() - 8.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            k.to_uppercase(),
                            FontId::proportional(10.0),
                            Palette::text_faint(),
                        );
                    }
                    ui.painter().text(
                        Pos2::new(rect.right() - 54.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        &a.hint(),
                        FontId::proportional(9.5),
                        Palette::text_faint(),
                    );
                }
                if resp.clicked() && !fest {
                    funktion_gewaehlt(app, a.id);
                }
                if resp.drag_started() {
                    app.ui.slot_drag = Some((usize::MAX, a.id.to_string()));
                }
                resp.on_hover_text(a.hint());
            }
        }
        ui.add_space(12.0);
        ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.inventory.x_bloecke_x_funktionen_x_objekte", &[("wert1", format!("{}", app.doc().blocks.len())), ("wert2", format!("{}", actions::ACTIONS.len())), ("wert3", format!("{}", app.doc().entities.len()))])));
    });
}

#[cfg(test)]
mod tests_lager {
    //! T0088: Die mittlere Maustaste leert einen Platz im Inventar-Lager —
    //! dieselbe Geste, die die Hotbar schon seit T0064 kennt. Geprüft wird über
    //! echte Zeigerereignisse am gezeichneten Raster, nicht über einen Aufruf
    //! der Löschfunktion: sonst bliebe ein fehlender Mauszweig unbemerkt.

    use super::*;
    use crate::dialogs::tests_tastatur::{WegwerfKonfig, KONFIG};

    thread_local! {
        /// egui-Zeit je Testfaden — ein prozessweiter Zähler lief im
        /// Parallelauf davon und ließ Klicks verfallen (siehe T0076).
        static ZEIT: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    }

    fn naechste_zeit() -> f64 {
        ZEIT.with(|z| {
            let t = z.get() + 0.25;
            z.set(t);
            t
        })
    }

    fn rahmen(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(naechste_zeit()),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    fn klick(ctx: &egui::Context, app: &mut App, pos: Pos2, button: egui::PointerButton) {
        let knopf = |pressed: bool| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        rahmen(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        rahmen(ctx, app, vec![knopf(true)]);
        rahmen(ctx, app, vec![knopf(false)]);
        rahmen(ctx, app, vec![]);
    }

    /// Lager mit zwei belegten Plätzen (0 und 4), offen im Reiter „Lager“.
    fn app_mit_lager() -> (App, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.inventory = vec![None; 40];
        app.prefs.inventory[0] = Some("draw.wall".into());
        app.prefs.inventory[4] = Some("draw.line".into());
        app.ui.inventory_open = true;
        app.ui.inventory_tab = 0;
        // Zwei Bilder: ein frisches Fenster kennt seine Größe erst danach.
        rahmen(&ctx, &mut app, vec![]);
        rahmen(&ctx, &mut app, vec![]);
        (app, ctx)
    }

    fn platz(app: &App, idx: usize) -> Rect {
        let plaetze = app.ui.last_lager.clone().expect("Lager gezeichnet");
        plaetze
            .iter()
            .find(|(i, _)| *i == idx)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("Lagerplatz {idx} wurde nicht gezeichnet"))
    }

    #[test]
    fn mittlere_maustaste_leert_den_lagerplatz() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("lager-mitte");
        let (mut app, ctx) = app_mit_lager();

        let ziel = platz(&app, 0).center();
        klick(&ctx, &mut app, ziel, egui::PointerButton::Middle);

        assert_eq!(
            app.prefs.inventory[0], None,
            "die mittlere Maustaste hat den Lagerplatz nicht geleert"
        );
        assert_eq!(
            app.prefs.inventory[4].as_deref(),
            Some("draw.line"),
            "ein fremder Lagerplatz wurde mitgeleert"
        );
        assert_eq!(
            app.tool_id(),
            None,
            "das Leeren darf das Element nicht zusätzlich starten"
        );
        assert_eq!(app.prefs.inventory.len(), 40, "das Raster bleibt vollständig");
    }

    #[test]
    fn linke_taste_startet_weiterhin_und_leere_plaetze_bleiben_leer() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("lager-links");
        let (mut app, ctx) = app_mit_lager();

        // Platz 1 ist leer: die mittlere Taste hat dort nichts zu leeren.
        let leer = platz(&app, 1).center();
        klick(&ctx, &mut app, leer, egui::PointerButton::Middle);
        assert_eq!(
            app.prefs.inventory[0].as_deref(),
            Some("draw.wall"),
            "ein Leerklick hat einen anderen Platz geleert"
        );
        assert_eq!(app.prefs.inventory[1], None, "Platz 1 war und bleibt leer");

        // Die linke Taste startet das Element wie bisher.
        let belegt = platz(&app, 4).center();
        klick(&ctx, &mut app, belegt, egui::PointerButton::Primary);
        assert_eq!(
            app.tool_id(),
            Some("draw.line"),
            "die linke Taste startet das Element nicht mehr"
        );
        assert_eq!(
            app.prefs.inventory[4].as_deref(),
            Some("draw.line"),
            "die linke Taste hat den Platz geleert"
        );
    }
}
