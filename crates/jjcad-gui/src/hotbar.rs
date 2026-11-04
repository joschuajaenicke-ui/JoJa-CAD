//! Untere Leiste: links Inventar/Tastaturbelegung/Hotbar-Auswahl, mittig die
//! Hotbar-Slots (Tasten 1–9 und 0), rechts Stärke/Fang/Raster.
//!
//! Die Leiste ist **ein** Element (früher Hotbar und Statuszeile getrennt).
//! Koordinaten, Objektzahl, Build-Kennung und Einheitenwahl stehen nicht mehr
//! hier — sie sind doppelt (Eigenschaften-Panel, Fenstertitel, Einstellungen).
//!
//! Die drei Gruppen sitzen in festen Bereichen: die Slots stehen exakt in der
//! Mitte der Leiste, unabhängig von der Breite der Randgruppen
//! (`tests_untere_leiste::hotbar_slots_stehen_mittig`).

use crate::app::App;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{
    Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, UiBuilder,
    Vec2,
};
use jjcad_core::units::format_scale;

/// Slot-Kante in Pixeln — die ursprüngliche Größe der Hotbar-Slots. Sie
/// bestimmt die Höhe der Leiste (App::HOTBAR_H), nicht umgekehrt.
pub const SLOT: f32 = 50.0;

/// Runde Schaltfläche der linken und rechten Gruppe — gleiche Größe, gleicher
/// Stil, damit Inventar, Tastatur, Hotbar-Auswahl, Stärke, Fang und Raster
/// wie eine Familie aussehen.
const KOPF: f32 = 30.0;
const ABSTAND: f32 = 4.0;
/// Abstand der Randgruppen zum Fensterrand. 8 px war zu knapp — die äußeren
/// Schaltflächen lagen praktisch am Bildschirmrand und wirkten abgeschnitten.
const RAND: f32 = 16.0;

/// Taste, die einen Hotbar-Slot startet — die **eine** Quelle für die
/// Beschriftung im Slot und die Auswertung des Tastendrucks (T0065).
/// Der zehnte Slot trägt `0`, nicht `10`: die Ziffern `1` … `9` gehören den
/// ersten neun Slots.
pub fn slot_taste(slot: usize) -> String {
    ((slot + 1) % 10).to_string()
}

/// Slot zu einer Zifferntaste — die Umkehrung von [`slot_taste`].
/// `0` ist damit der **letzte** Slot, nicht der erste. Früher rechnete die
/// Taste `0` über `saturating_sub(1)` und startete das erste Element (T0065).
pub fn taste_slot(label: &str) -> Option<usize> {
    let n: usize = label.trim().parse().ok()?;
    if n > 9 {
        return None;
    }
    Some((n + 9) % 10)
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let bar = ui.max_rect();
    let slots = app.prefs.slots();
    let n = slots.len().max(1);
    let gesamt = n as f32 * SLOT + (n - 1) as f32 * ABSTAND;
    let slots_rect = Rect::from_center_size(
        Pos2::new(bar.center().x, bar.center().y),
        Vec2::new(gesamt, SLOT),
    );
    let links_rect = Rect::from_min_max(
        bar.min + Vec2::new(RAND, 0.0),
        Pos2::new((slots_rect.left() - 8.0).max(bar.left()), bar.max.y),
    );
    let rechts_rect = Rect::from_min_max(
        Pos2::new((slots_rect.right() + 8.0).min(bar.right()), bar.top()),
        bar.max - Vec2::new(RAND, 0.0),
    );

    // ---- Links: Inventar · Tastaturbelegung · Hotbar-Vorlagen ----
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(links_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            // Clip auf die ganze Leiste: ein Clip genau an der Gruppenkante
            // schneidet die Antialiasing-Kante der äußeren Schaltfläche ab.
            ui.set_clip_rect(bar);
            // Abstände macht die Leiste selbst — egui soll nicht zusätzlich
            // 8 px zwischen jedem Knopf setzen (sonst wandern die Slots aus
            // der Mitte).
            ui.spacing_mut().item_spacing.x = 0.0;
            let mut kopf: Option<Rect> = None;
            let mut merke = |r: Rect| {
                kopf = Some(match kopf {
                    Some(b) => b.union(r),
                    None => r,
                });
            };
            let resp =
                widgets::icon_button(ui, "inventory", KOPF, app.ui.inventory_open, jjcad_core::i18n::tr("gui.hotbar.inventar_e"));
            merke(resp.rect);
            if resp.clicked() {
                app.toggle_inventory();
            }
            ui.add_space(ABSTAND);
            let resp =
                widgets::icon_button(ui, "keyboard", KOPF, app.ui.keymap_open, jjcad_core::i18n::tr("gui.hotbar.tastaturbelegung"));
            merke(resp.rect);
            if resp.clicked() {
                app.ui.keymap_open = !app.ui.keymap_open;
            }
            ui.add_space(ABSTAND);
            let vorlage = app
                .prefs
                .hotbar_presets
                .get(app.prefs.active_preset.min(app.prefs.hotbar_presets.len().saturating_sub(1)))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| jjcad_core::i18n::tr("gui.hotbar.vorlage").to_string());
            let tip = jjcad_core::i18n::t_args_owned("gui.hotbar.hotbar_x_vorlage_waehlen", &[("vorlage", format!("{}", vorlage))]);
            let resp =
                widgets::icon_button(ui, "hotbar", KOPF, app.ui.hotbar_picker_open, &tip);
            merke(resp.rect);
            if resp.clicked() {
                app.ui.hotbar_picker_open = !app.ui.hotbar_picker_open;
            }
            app.ui.last_kopf_links = kopf;
        },
    );

    // ---- Mitte: die Slots ----
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(slots_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.set_clip_rect(bar);
            ui.spacing_mut().item_spacing.x = 0.0;
            let mut slots_ist: Option<Rect> = None;
            for i in 0..n {
                let action = slots.get(i).cloned().flatten();
                let active = action
                    .as_ref()
                    .map(|a| app.tool_id() == Some(a.as_str()))
                    .unwrap_or(false);
                let highlight = app.ui.bind_slot == Some(i);
                let key = slot_taste(i);
                let resp = widgets::slot_button(
                    ui,
                    Vec2::splat(SLOT),
                    action.as_deref(),
                    Some(&key),
                    active,
                    highlight,
                );

                // T0101: Ein gedrückt gehaltener Schwenk-Slot bewegt die Ansicht
                // fortlaufend — `App::schwenk_fortschritt` liest die Richtung im
                // selben Bild und löscht sie wieder.
                if resp.is_pointer_button_down_on() {
                    if let Some(r) = action.as_deref().and_then(crate::app::schwenk_richtung) {
                        app.ui.schwenk_zeiger = Some(r);
                    }
                }

                // Klick: starten oder belegen
                if resp.clicked() {
                    if let Some(a) = &action {
                        app.activate(ui.ctx(), a);
                    } else if let Some(t) = app.tool.clone() {
                        app.bind_slot_action(i, Some(t.action.clone()));
                        app.tool = None;
                        app.toast(jjcad_core::i18n::t_args_owned("gui.hotbar.x_auf_slot_x_gelegt", &[("wert1", format!("{}", label_of(&t.action))), ("wert2", format!("{}", i + 1))]));
                    } else {
                        app.ui.bind_slot = Some(i);
                        app.ui.inventory_open = true;
                        app.ui.inventory_tab = 2;
                        app.toast(jjcad_core::i18n::tr("gui.hotbar.funktion_fuer_den_slot_waehlen"));
                    }
                }

                // Mittlere Maustaste: leeren
                if resp.clicked_by(egui::PointerButton::Middle) {
                    app.bind_slot_action(i, None);
                }

                // Ziehen
                if resp.drag_started() {
                    if let Some(a) = &action {
                        app.ui.slot_drag = Some((i, a.clone()));
                    }
                }
                if let Some((from, a)) = app.ui.slot_drag.clone() {
                    if resp.hovered() && ui.ctx().input(|inp| inp.pointer.any_released()) {
                        if from == usize::MAX {
                            app.prefs.set_slot(i, Some(a.clone()));
                            let _ = app.prefs.save();
                            app.toast(jjcad_core::i18n::t_args_owned("gui.hotbar.hotbar_slot_x_belegt", &[("wert1", format!("{}", i + 1))]));
                        } else if from != i {
                            app.prefs.set_slot(i, Some(a.clone()));
                            app.prefs.set_slot(from, action.clone());
                            let _ = app.prefs.save();
                        }
                        app.ui.slot_drag = None;
                    }
                }

                // Kontextmenü
                resp.context_menu(|ui| {
                    if action.is_some() {
                        if ui.button(jjcad_core::i18n::tr("gui.hotbar.leeren")).clicked() {
                            app.bind_slot_action(i, None);
                            ui.close_menu();
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.hotbar.bearbeiten")).clicked() {
                            if let Some(a) = &action {
                                if a == "block.insert" {
                                    app.ui.inventory_open = true;
                                    app.ui.inventory_tab = 1;
                                } else {
                                    app.start_tool(a);
                                }
                            }
                            ui.close_menu();
                        }
                    }
                    if ui.button(jjcad_core::i18n::tr("gui.hotbar.verknuepfen")).clicked() {
                        app.ui.bind_slot = Some(i);
                        app.ui.inventory_open = true;
                        app.ui.inventory_tab = 2;
                        ui.close_menu();
                    }
                });

                slots_ist = Some(match slots_ist {
                    Some(r) => r.union(resp.rect),
                    None => resp.rect,
                });

                if i + 1 < n {
                    ui.add_space(ABSTAND);
                }
            }
            app.ui.last_slots = slots_ist;
        },
    );

    // ---- Rechts: Stärke · Fang · Raster, dazu Maßstab und Zoom ----
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rechts_rect)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.set_clip_rect(bar);
            ui.spacing_mut().item_spacing.x = 0.0;
            let mut kopf: Option<Rect> = None;
            // Raster: Linksklick an/aus, Rechtsklick Menü
            let resp = widgets::icon_button(
                ui,
                "grid",
                KOPF,
                app.ui.show_grid,
                jjcad_core::i18n::tr("gui.hotbar.raster_klick_ein_aus_rechtsklick_optionen"),
            );
            if resp.clicked() {
                app.ui.show_grid = !app.ui.show_grid;
                let v = app.ui.show_grid;
                let _ = app.session_mut().execute(
                    "doc.settings",
                    &serde_json::json!({"show_grid": v, "snap": {"grid": v}}),
                );
            }
            if resp.secondary_clicked() {
                let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                app.open_menu(crate::app::MenuKind::Chip(crate::app::StatusMenu::Raster), p);
            }
            kopf = Some(resp.rect);
            ui.add_space(ABSTAND);

            // Fang (Klick: ein/aus · Rechtsklick: Fangmodi und Ausrichtung)
            let snap_now = app.doc().settings.snap.clone();
            let tip = if snap_now.endpoint {
                jjcad_core::i18n::tr("gui.hotbar.fang_an_endpunkt_rechtsklick_fangmodi_und_ausrichtung")
            } else {
                jjcad_core::i18n::tr("gui.hotbar.fang_aus_rechtsklick_fangmodi_und_ausrichtung")
            };
            let resp = widgets::icon_button(ui, "snap", KOPF, snap_now.endpoint, tip);
            if resp.clicked() {
                app.set_snap("endpoint", !snap_now.endpoint);
            }
            if resp.secondary_clicked() {
                let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                app.open_menu(crate::app::MenuKind::Chip(crate::app::StatusMenu::Fang), p);
            }
            kopf = kopf.map(|b| b.union(resp.rect));
            ui.add_space(ABSTAND);

            // Linienstärken: px (bildschirmkonstant) oder DPI (wie im PDF).
            // Das Zeichen rechts unten nennt den Modus, der Tooltip den Rest.
            let scaled = app.ui.lineweights_scaled;
            let tip = if scaled {
                jjcad_core::i18n::tr("gui.hotbar.linienstaerke_dpi_wie_im_pdf_klick_px_rechtsklick_auswah")
            } else {
                jjcad_core::i18n::tr("gui.hotbar.linienstaerke_px_bildschirmkonstant_klick_dpi_rechtsklic")
            };
            let resp = widgets::icon_button(ui, "staerke", KOPF, scaled, tip);
            ui.painter().text(
                Pos2::new(resp.rect.right() - 2.0, resp.rect.bottom() - 1.0),
                egui::Align2::RIGHT_BOTTOM,
                if scaled { jjcad_core::i18n::tr("gui.hotbar.d") } else { jjcad_core::i18n::tr("gui.hotbar.p") },
                FontId::proportional(8.0),
                if scaled {
                    Palette::accent()
                } else {
                    Palette::text_faint()
                },
            );
            if resp.clicked() {
                app.ui.lineweights = true;
                app.ui.lineweights_scaled = !scaled;
                app.prefs.lineweights_scaled = app.ui.lineweights_scaled;
                let _ = app.prefs.save();
            }
            if resp.secondary_clicked() {
                let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                app.open_menu(crate::app::MenuKind::Chip(crate::app::StatusMenu::Staerke), p);
            }
            kopf = kopf.map(|b| b.union(resp.rect));
            app.ui.last_kopf_rechts = kopf;

            // Maßstab und Zoom: klein, links von den Schaltern
            ui.add_space(8.0);
            ui.label(theme::mono(format_scale(app.doc().settings.annotation_scale)));
            ui.add_space(6.0);
            ui.label(theme::mono(format!(
                "{:.0} %",
                (app.current_view().zoom * 100.0).max(1.0)
            )));
            ui.add_space(8.0);
            widgets::vline(ui, 18.0);
            ui.add_space(6.0);
            if let Some(s) = app.snap_label.clone() {
                ui.label(RichText::new(s).size(10.5).color(Palette::accent()));
            }
        },
    );

    // Ziehen-Vorschau (über der Leiste, am Zeiger)
    if let Some((_, a)) = app.ui.slot_drag.clone() {
        if let Some(pos) = ui.ctx().pointer_hover_pos() {
            let r = Rect::from_center_size(pos, Vec2::splat(SLOT));
            ui.painter().rect_filled(
                r,
                Rounding::same(9.0),
                Palette::schwebend(),
            );
            ui.painter()
                .rect_stroke(r, Rounding::same(9.0), Stroke::new(1.0, Palette::accent()));
            if let Some(def) = jjcad_core::actions::action(&a) {
                crate::icons::paint(ui.painter(), r.shrink(8.0), def.icon, Palette::accent());
            }
        }
    }
}

/// Fenster „Hotbar-Vorlagen" — ersetzt das Aufklappfeld in der Leiste.
/// Auswählen wechselt die Vorlage, Neu/Umbenennen öffnen den Namensdialog.
pub fn auswahl_fenster(app: &mut App, ctx: &egui::Context) {
    if !app.ui.hotbar_picker_open {
        return;
    }
    let mut offen = true;
    // Esc schließt das Fenster (wie die übrigen Dialoge)
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        offen = false;
    }
    let mut waehle: Option<usize> = None;
    let mut neu = false;
    let mut umbenennen: Option<usize> = None;
    let mut loeschen: Option<usize> = None;

    egui::Window::new("hotbar_wahl")
        .title_bar(false)
        .resizable(false)
        .default_width(250.0)
        .anchor(
            egui::Align2::LEFT_BOTTOM,
            Vec2::new(10.0, -(crate::app::HOTBAR_H + 8.0)),
        )
        .frame(theme::dialog_frame())
        .show(ctx, |ui| {
            ui.set_min_width(250.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                crate::icons::paint(ui.painter(), r, "hotbar", Palette::accent());
                ui.label(theme::bold(jjcad_core::i18n::tr("gui.hotbar.hotbar_vorlagen"), 13.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, "close", 22.0, false, jjcad_core::i18n::tr("gui.hotbar.schliessen")).clicked() {
                        offen = false;
                    }
                });
            });
            ui.add_space(4.0);
            widgets::hline(ui);
            ui.add_space(4.0);

            let cur = app
                .prefs
                .active_preset
                .min(app.prefs.hotbar_presets.len().saturating_sub(1));
            for (i, p) in app.prefs.hotbar_presets.iter().enumerate() {
                let aktiv = i == cur;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                let p2 = ui.painter();
                if aktiv {
                    p2.rect_filled(rect, Rounding::same(7.0), Palette::panel_alt());
                } else if resp.hovered() {
                    p2.rect_filled(
                        rect,
                        Rounding::same(7.0),
                        Palette::panel_alt().gamma_multiply(0.6),
                    );
                }
                if aktiv {
                    p2.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(rect.left() + 3.0, rect.top() + 6.0),
                            Pos2::new(rect.left() + 5.5, rect.bottom() - 6.0),
                        ),
                        Rounding::same(1.0),
                        Palette::accent(),
                    );
                }
                p2.text(
                    Pos2::new(rect.left() + 14.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &p.name,
                    FontId::proportional(12.0),
                    if aktiv {
                        Palette::text()
                    } else {
                        Palette::text_dim()
                    },
                );
                p2.text(
                    Pos2::new(rect.right() - 8.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    jjcad_core::i18n::t_args_owned("gui.hotbar.x_slots", &[("wert1", format!("{}", p.slots.len()))]),
                    FontId::proportional(10.0),
                    Palette::text_faint(),
                );
                if resp.clicked() {
                    waehle = Some(i);
                }
            }

            ui.add_space(4.0);
            widgets::hline(ui);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(jjcad_core::i18n::tr("gui.hotbar.neu")).clicked() {
                    neu = true;
                }
                if ui.button(jjcad_core::i18n::tr("gui.hotbar.umbenennen")).clicked() {
                    umbenennen = Some(cur);
                }
                let mehrere = app.prefs.hotbar_presets.len() > 1;
                if ui.add_enabled(mehrere, egui::Button::new(jjcad_core::i18n::tr("gui.hotbar.loeschen"))).clicked() {
                    loeschen = Some(cur);
                }
            });
        });

    if let Some(i) = waehle {
        app.prefs.active_preset = i;
        let _ = app.prefs.save();
        let name = app.prefs.hotbar_presets[i].name.clone();
        app.toast(jjcad_core::i18n::t_args_owned("gui.hotbar.hotbar_x", &[("name", format!("{}", name))]));
        app.ui.hotbar_picker_open = false;
    }
    if neu {
        app.ui.preset_dialog =
            Some((None, format!("Vorlage {}", app.prefs.hotbar_presets.len() + 1)));
    }
    if let Some(i) = umbenennen {
        let name = app
            .prefs
            .hotbar_presets
            .get(i)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        app.ui.preset_dialog = Some((Some(i), name));
    }
    if let Some(i) = loeschen {
        app.prefs.hotbar_presets.remove(i);
        app.prefs.active_preset = i.saturating_sub(1);
        let _ = app.prefs.save();
        app.toast(jjcad_core::i18n::tr("gui.hotbar.vorlage_geloescht"));
    }
    if !offen {
        app.ui.hotbar_picker_open = false;
    }
}

fn label_of(action: &str) -> &'static str {
    jjcad_core::actions::action(action)
        .map(|d| d.label())
        .unwrap_or(jjcad_core::i18n::tr("gui.hotbar.werkzeug"))
}

#[cfg(test)]
mod tests_slot_tasten {
    //! T0065: Die Zifferntaste gehört zu dem Slot, der sie anzeigt — `0` ist der
    //! zehnte (letzte) Slot. Die Auswertung rechnete früher `0 - 1` und startete
    //! damit das erste Element.

    use super::*;

    #[test]
    fn taste_0_gehoert_dem_letzten_slot() {
        assert_eq!(
            taste_slot("0"),
            Some(9),
            "„0“ muss den letzten Slot starten, nicht den ersten"
        );
        assert_eq!(taste_slot("1"), Some(0), "„1“ gehört dem ersten Slot");
        assert_eq!(taste_slot("9"), Some(8), "„9“ gehört dem neunten Slot");
    }

    #[test]
    fn beschriftung_und_taste_passen_zusammen() {
        for slot in 0..10 {
            let taste = slot_taste(slot);
            assert_eq!(
                taste_slot(&taste),
                Some(slot),
                "Slot {slot} trägt die Taste „{taste}“, die aber woanders hin führt"
            );
        }
        assert_eq!(slot_taste(0), "1", "erster Slot trägt die 1");
        assert_eq!(slot_taste(9), "0", "letzter Slot trägt die 0");
    }
}
