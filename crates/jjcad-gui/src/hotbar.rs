//! Hotbar mit 10 Item-Slots (Tasten 1–9 und 0), Vorlagen und Drag & Drop.

use crate::app::App;
use crate::theme::Palette;
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};

pub const SLOT: f32 = 50.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        // Vorlagen-Auswahl als Dropdown (ersetzt die alten Punkte)
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(3.0);
            ui.label(
                RichText::new("HOTBAR")
                    .size(9.0)
                    .color(Palette::TEXT_FAINT)
                    .strong(),
            );
            let cur = app.prefs.active_preset.min(app.prefs.hotbar_presets.len().saturating_sub(1));
            let name = app
                .prefs
                .hotbar_presets
                .get(cur)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Vorlage".into());
            let mut select: Option<usize> = None;
            let mut new_dialog = false;
            let mut rename = false;
            let mut delete = false;
            egui::ComboBox::from_id_salt("hotbar_preset")
                .width(132.0)
                .selected_text(RichText::new(name.clone()).size(11.5).color(Palette::TEXT_DIM))
                .show_ui(ui, |ui| {
                    ui.set_min_width(160.0);
                    for (i, p) in app.prefs.hotbar_presets.iter().enumerate() {
                        let active = i == cur;
                        if ui.selectable_label(active, &p.name).clicked() {
                            select = Some(i);
                        }
                    }
                    ui.separator();
                    if ui.button("+ Neue Vorlage …").clicked() {
                        new_dialog = true;
                    }
                    if ui.button("Umbenennen …").clicked() {
                        rename = true;
                    }
                    if app.prefs.hotbar_presets.len() > 1 && ui.button("Löschen").clicked() {
                        delete = true;
                    }
                });
            if let Some(i) = select {
                app.prefs.active_preset = i;
                let _ = app.prefs.save();
            }
            if new_dialog {
                app.ui.preset_dialog = Some((None, format!("Vorlage {}", app.prefs.hotbar_presets.len() + 1)));
            }
            if rename {
                app.ui.preset_dialog = Some((Some(cur), name.clone()));
            }
            if delete {
                app.prefs.hotbar_presets.remove(cur);
                app.prefs.active_preset = cur.saturating_sub(1);
                let _ = app.prefs.save();
                app.toast("Vorlage gelöscht");
            }
        });

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() * 0.5 - 300.0).max(0.0));
                let slots = app.prefs.slots();
                for i in 0..10 {
                    let action = slots.get(i).cloned().flatten();
                    let active = action
                        .as_ref()
                        .map(|a| app.tool_id() == Some(a.as_str()))
                        .unwrap_or(false);
                    let highlight = app.ui.bind_slot == Some(i);
                    let key = ((i + 1) % 10).to_string();
                    let resp = widgets::slot_button(
                        ui,
                        Vec2::splat(SLOT),
                        action.as_deref(),
                        Some(&key),
                        active,
                        highlight,
                    );

                    // Klick: starten oder belegen
                    if resp.clicked() {
                        if let Some(a) = &action {
                            app.activate(ui.ctx(), a);
                        } else if let Some(t) = app.tool.clone() {
                            app.bind_slot_action(i, Some(t.action.clone()));
                            app.tool = None;
                            app.toast(format!("{} auf Slot {} gelegt", label_of(&t.action), i + 1));
                        } else {
                            app.ui.bind_slot = Some(i);
                            app.ui.inventory_open = true;
                            app.ui.inventory_tab = 2;
                            app.toast("Funktion für den Slot wählen");
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
                                app.toast(format!("Slot {} belegt", i + 1));
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
                            if ui.button("Leeren").clicked() {
                                app.bind_slot_action(i, None);
                                ui.close_menu();
                            }
                            if ui.button("Bearbeiten").clicked() {
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
                        if ui.button("Verknüpfen …").clicked() {
                            app.ui.bind_slot = Some(i);
                            app.ui.inventory_open = true;
                            app.ui.inventory_tab = 2;
                            ui.close_menu();
                        }
                    });
                }
            });
            ui.add_space(2.0);

            // Ziehen-Vorschau
            if let Some((_, a)) = app.ui.slot_drag.clone() {
                if let Some(pos) = ui.ctx().pointer_hover_pos() {
                    let r = Rect::from_center_size(pos, Vec2::splat(SLOT));
                    ui.painter().rect_filled(
                        r,
                        Rounding::same(9.0),
                        Color32::from_rgba_unmultiplied(30, 34, 40, 230),
                    );
                    ui.painter()
                        .rect_stroke(r, Rounding::same(9.0), Stroke::new(1.0, Palette::ACCENT));
                    if let Some(def) = jjcad_core::actions::action(&a) {
                        crate::icons::paint(
                            ui.painter(),
                            r.shrink(12.0),
                            def.icon,
                            Palette::ACCENT,
                        );
                    }
                }
            }
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(10.0);
            let (r, resp) = ui.allocate_exact_size(Vec2::new(94.0, 34.0), Sense::click());
            let hovered = resp.hovered();
            ui.painter().rect_filled(
                r,
                Rounding::same(8.0),
                if hovered { Palette::PANEL_ALT } else { Palette::PANEL },
            );
            ui.painter()
                .rect_stroke(r, Rounding::same(8.0), Stroke::new(1.0, Palette::BORDER_SOFT));
            crate::icons::paint(
                ui.painter(),
                Rect::from_min_size(r.min + Vec2::new(8.0, 9.0), Vec2::splat(16.0)),
                "inventory",
                Palette::TEXT_DIM,
            );
            ui.painter().text(
                Pos2::new(r.left() + 30.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                "Inventar",
                FontId::proportional(11.0),
                Palette::TEXT_DIM,
            );
            ui.painter().text(
                Pos2::new(r.right() - 8.0, r.center().y),
                egui::Align2::RIGHT_CENTER,
                "E",
                FontId::proportional(9.5),
                Palette::TEXT_FAINT,
            );
            if resp.clicked() {
                app.toggle_inventory();
            }
            resp.on_hover_text("Inventar ein-/ausblenden (E)");
        });
    });
}

fn label_of(action: &str) -> &'static str {
    jjcad_core::actions::action(action)
        .map(|d| d.label)
        .unwrap_or("Werkzeug")
}
