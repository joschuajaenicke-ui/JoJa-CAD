//! Fenster „Wiederherstellen“ (T0223).
//!
//! Zeigt die Zeichnungen aus dem Sicherungsordner (Pool, siehe
//! `jjcad_core::sicherung`) und je Zeichnung deren Stände als Punkte über den
//! Zeitverlauf. Ein gewählter Stand erscheint als Vorschau und lässt sich als
//! Reiter öffnen.
//!
//! Der Inhalt kommt aus **einer** Quelle: `Sicherung::zeichnungen()` für die
//! Liste, `Sicherung::staende_chronologisch()` für das Diagramm und
//! `Sicherung::laden()` für die Vorschau. Die Vorschau malt
//! `canvas::paint_zeichnung` — derselbe Maler wie die Zeichenfläche.

use crate::app::{kurz_zeit, App};
use crate::canvas;
use crate::dialogs::modal_window;
use crate::theme::Palette;
use egui::{Align, Align2, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use jjcad_core::i18n::{t_args_owned, tr};
use jjcad_core::sicherung::Stand;
use jjcad_core::tessellate::CurveQuality;

/// Breite des Fensters (feste Größe wie bei den übrigen Dialogen).
pub const FENSTER_BREITE: f32 = 940.0;
/// Höhe des Fensters.
pub const FENSTER_HOEHE: f32 = 700.0;
const INNEN: f32 = FENSTER_BREITE - 24.0;
const LISTE_BREITE: f32 = 286.0;
/// Höhe des Diagramms über dem Zeitverlauf.
pub const DIAGRAMM_HOEHE: f32 = 180.0;
/// Höhe der Vorschau.
pub const VORSCHAU_HOEHE: f32 = 290.0;
/// Trefferradius eines Punktes im Diagramm.
const PUNKT_RADIUS: f32 = 5.0;
/// Abstand, in dem ein Klick den nächsten Punkt trifft.
const PUNKT_FANG: f32 = 14.0;

/// Zeichnet das Fenster, solange es offen ist.
pub fn show(app: &mut App, ctx: &egui::Context) {
    if app.ui.wiederherstellen.is_none() {
        return;
    }
    crate::dialogs::dim_background(ctx);

    // Daten einmal je Bild aus dem Pool ziehen. Die Liste ist klein (ein
    // Eintrag je Zeichnung), das Diagramm höchstens N Stände lang.
    let dateien = app.wiederherstellen_dateien();
    let gewaehlt: Option<String> = app
        .ui
        .wiederherstellen
        .as_ref()
        .and_then(|w| w.datei.clone());
    let staende: Vec<Stand> = match &gewaehlt {
        Some(id) => app
            .sicherung_pool()
            .staende_chronologisch(id)
            .into_iter()
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    let (gewaehlter_zyklus, fehler) = app
        .ui
        .wiederherstellen
        .as_ref()
        .map(|w| (w.zyklus, w.fehler.clone()))
        .unwrap_or((None, None));
    let vorschau_vorhanden = app
        .ui
        .wiederherstellen
        .as_ref()
        .map(|w| w.vorschau.as_ref().map(|(z, _)| *z))
        .unwrap_or(None);

    let mut offen = true;
    let mut schliessen = false;
    let mut neu_laden = false;
    let mut oeffnen_jetzt = false;
    let mut datei_neu: Option<Option<String>> = None;
    let mut zyklus_neu: Option<Option<u64>> = None;

    modal_window(ctx, tr("wiederherstellen.titel"), FENSTER_BREITE, &mut offen)
        // Über Einstellungen und Inventar: das Fenster wird von dort geöffnet.
        .order(egui::Order::Foreground)
        .default_size([FENSTER_BREITE, FENSTER_HOEHE])
        .show(ctx, |ui| {
            ui.set_width(INNEN);
            ui.label(egui::RichText::new(tr("wiederherstellen.hinweis")).color(Palette::text_dim()));
            ui.add_space(6.0);

            let inhalt_h = FENSTER_HOEHE - 140.0;
            ui.allocate_ui_with_layout(
                Vec2::new(INNEN, inhalt_h),
                Layout::left_to_right(Align::Min),
                |ui| {
                    // ------------------------------ Liste --------------
                    ui.allocate_ui_with_layout(
                        Vec2::new(LISTE_BREITE, inhalt_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.label(RichText::new(tr("wiederherstellen.dateien")).strong());
                            ui.add_space(2.0);
                            if dateien.is_empty() {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(tr(
                                        "wiederherstellen.leer",
                                    ))
                                    .color(Palette::text_dim()))
                                    .wrap(),
                                );
                                return;
                            }
                            let mut rechtecke: Vec<(String, Rect)> = Vec::new();
                            egui::ScrollArea::vertical()
                                .id_salt("wh_dateien")
                                .max_height(inhalt_h - 26.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for d in &dateien {
                                        let aktiv = gewaehlt.as_deref() == Some(d.datei.as_str());
                                        let anzahl = if d.staende == 1 {
                                            tr("wiederherstellen.ein_stand").to_string()
                                        } else {
                                            t_args_owned(
                                                "wiederherstellen.x_staende",
                                                &[("anzahl", d.staende.to_string())],
                                            )
                                        };
                                        let text = format!(
                                            "{}\n{} · {}",
                                            d.name,
                                            anzahl,
                                            kurz_zeit(&d.neueste_zeit)
                                        );
                                        let antwort = ui.add_sized(
                                            [LISTE_BREITE - 4.0, 40.0],
                                            egui::SelectableLabel::new(aktiv, text),
                                        );
                                        rechtecke.push((d.datei.clone(), antwort.rect));
                                        if antwort.clicked() && !aktiv {
                                            datei_neu = Some(Some(d.datei.clone()));
                                        }
                                    }
                                });
                            if let Some(w) = app.ui.wiederherstellen.as_mut() {
                                w.last_liste = rechtecke;
                            }
                        },
                    );
                    ui.add_space(12.0);

                    // --------------------- Diagramm und Vorschau --------
                    let rechts = INNEN - LISTE_BREITE - 12.0;
                    ui.allocate_ui_with_layout(
                        Vec2::new(rechts, inhalt_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            let Some(id) = gewaehlt.clone() else {
                                ui.add_space(inhalt_h * 0.35);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        egui::RichText::new(tr("wiederherstellen.keine_auswahl"))
                                            .color(Palette::text_dim()),
                                    );
                                });
                                return;
                            };
                            let eintrag = dateien.iter().find(|d| d.datei == id).cloned();
                            let name = eintrag
                                .as_ref()
                                .map(|d| d.name.clone())
                                .unwrap_or_else(|| id.clone());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(name).strong());
                                if let Some(p) = eintrag.as_ref().filter(|d| !d.pfad.is_empty()) {
                                    ui.label(
                                        egui::RichText::new(&p.pfad)
                                            .color(Palette::text_faint())
                                            .small(),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new(tr("wiederherstellen.ohne_pfad"))
                                            .color(Palette::text_faint())
                                            .small(),
                                    );
                                }
                            });
                            ui.add_space(4.0);

                            // Diagramm: Größe über dem Zeitverlauf.
                            ui.label(
                                egui::RichText::new(tr("wiederherstellen.verlauf"))
                                    .color(Palette::text_dim()),
                            );
                            let (chart_rect, antwort) = ui.allocate_exact_size(
                                Vec2::new(rechts, DIAGRAMM_HOEHE),
                                Sense::click(),
                            );
                            let punkte = diagramm(
                                ui,
                                chart_rect,
                                &staende,
                                gewaehlter_zyklus,
                                antwort.hover_pos(),
                            );
                            if antwort.clicked() {
                                if let Some(pos) = antwort.interact_pointer_pos() {
                                    if let Some((z, _)) = punkte
                                        .iter()
                                        .map(|(z, r)| (*z, r.center()))
                                        .min_by(|a, b| {
                                            a.1.distance(pos).total_cmp(&b.1.distance(pos))
                                        })
                                        .filter(|(_, c)| c.distance(pos) <= PUNKT_FANG)
                                    {
                                        zyklus_neu = Some(Some(z));
                                    }
                                }
                            }
                            if let Some(w) = app.ui.wiederherstellen.as_mut() {
                                w.last_punkte = punkte.clone();
                            }
                            // Legende der Punktfarben — ein Punkt allein verrät
                            // nicht, ob er Arbeit, Dateistand oder Verworfenes zeigt.
                            ui.horizontal(|ui| {
                                for (farbe, schluessel) in [
                                    (Palette::accent(), "wiederherstellen.status_arbeit"),
                                    (Palette::text_faint(), "wiederherstellen.status_gesichert"),
                                    (Palette::danger(), "wiederherstellen.status_verworfen"),
                                ] {
                                    let (punkt, _) = ui
                                        .allocate_exact_size(Vec2::new(9.0, 9.0), Sense::hover());
                                    ui.painter().circle_filled(punkt.center(), 4.0, farbe);
                                    ui.label(
                                        egui::RichText::new(tr(schluessel))
                                            .small()
                                            .color(Palette::text_faint()),
                                    );
                                    ui.add_space(6.0);
                                }
                            });
                            ui.add_space(4.0);

                            // Stand-Angaben des gewählten Punktes.
                            let stand = staende
                                .iter()
                                .find(|s| Some(s.zyklus) == gewaehlter_zyklus);
                            let text = match stand {
                                Some(s) => t_args_owned(
                                    "wiederherstellen.stand_details",
                                    &[
                                        ("zeit", kurz_zeit(&s.zeit)),
                                        ("groesse", groesse_kb(s.groesse)),
                                        ("status", stand_status(s)),
                                    ],
                                ),
                                None => tr("wiederherstellen.kein_stand").to_string(),
                            };
                            ui.label(egui::RichText::new(text).color(Palette::text_dim()));
                            if let Some(f) = &fehler {
                                ui.label(egui::RichText::new(f).color(Palette::danger()));
                            }
                            ui.add_space(6.0);

                            // Vorschau des Standes.
                            let (p_rect, _) = ui.allocate_exact_size(
                                Vec2::new(rechts, VORSCHAU_HOEHE),
                                Sense::hover(),
                            );
                            vorschau_malen(ui, p_rect, app, vorschau_vorhanden, gewaehlter_zyklus);
                            if let Some(w) = app.ui.wiederherstellen.as_mut() {
                                w.last_vorschau = Some(p_rect);
                            }
                        },
                    );
                },
            );

            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                Vec2::new(INNEN, 30.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    if ui.button(tr("wiederherstellen.schliessen")).clicked() {
                        schliessen = true;
                    }
                    let bedienbar = gewaehlter_zyklus.is_some() && fehler.is_none();
                    if ui
                        .add_enabled(
                            bedienbar,
                            egui::Button::new(tr("wiederherstellen.oeffnen")),
                        )
                        .on_hover_text(tr("wiederherstellen.oeffnen_tip"))
                        .clicked()
                    {
                        oeffnen_jetzt = true;
                    }
                    if ui
                        .button(tr("wiederherstellen.neu_laden"))
                        .on_hover_text(tr("wiederherstellen.neu_laden_tip"))
                        .clicked()
                    {
                        neu_laden = true;
                    }
                },
            );
        });

    // ------------------------------ Auswertung ------------------------------
    if let Some(wahl) = datei_neu {
        if let Some(w) = app.ui.wiederherstellen.as_mut() {
            w.datei = wahl;
            // Ein neuer Verlauf beginnt ohne Auswahl: der alte Punkt gehört
            // zu einer anderen Zeichnung.
            w.zyklus = None;
            w.vorschau = None;
            w.fehler = None;
        }
    }
    if let Some(wahl) = zyklus_neu {
        if let Some(w) = app.ui.wiederherstellen.as_mut() {
            w.zyklus = wahl;
        }
    }
    if neu_laden {
        app.wiederherstellen_neu_laden();
    }
    if oeffnen_jetzt {
        if let Some(z) = gewaehlter_zyklus {
            match app.wiederherstellen_stand_oeffnen(z) {
                Ok(zeit) => {
                    app.ui.wiederherstellen = None;
                    schliessen = true;
                    app.toast(t_args_owned(
                        "wiederherstellen.geoeffnet",
                        &[("zeit", kurz_zeit(&zeit))],
                    ));
                }
                Err(e) => {
                    if let Some(w) = app.ui.wiederherstellen.as_mut() {
                        w.fehler = Some(e);
                    }
                }
            }
        }
    }
    // Esc schließt das Fenster wie das Kreuz oben rechts.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        schliessen = true;
    }
    if schliessen || !offen {
        app.ui.wiederherstellen = None;
    } else {
        app.wiederherstellen_vorschau_nachziehen();
    }
}

/// Zeichnet das Diagramm und liefert die Punktflächen (Zyklus, Rechteck) —
/// der Prüfweg klickt darauf.
fn diagramm(
    ui: &egui::Ui,
    rect: Rect,
    staende: &[Stand],
    gewaehlt: Option<u64>,
    zeiger: Option<Pos2>,
) -> Vec<(u64, Rect)> {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Palette::panel_alt());
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, Palette::border_soft()),
    );
    if staende.is_empty() {
        return Vec::new();
    }
    // Platz für Achsen und Beschriftung.
    let flaeche = Rect::from_min_max(
        Pos2::new(rect.left() + 52.0, rect.top() + 14.0),
        Pos2::new(rect.right() - 12.0, rect.bottom() - 24.0),
    );
    let min_z = staende.first().map(|s| s.zyklus).unwrap_or(0) as f64;
    let max_z = staende.last().map(|s| s.zyklus).unwrap_or(0) as f64;
    let min_g = staende
        .iter()
        .map(|s| s.groesse as f64)
        .fold(f64::INFINITY, f64::min);
    let max_g = staende
        .iter()
        .map(|s| s.groesse as f64)
        .fold(f64::NEG_INFINITY, f64::max);
    let dx = (max_z - min_z).max(1.0);
    let dg = (max_g - min_g).max(1.0);
    let x_von = |z: u64| -> f32 {
        if staende.len() == 1 {
            flaeche.center().x
        } else {
            flaeche.left() + ((z as f64 - min_z) / dx) as f32 * flaeche.width()
        }
    };
    let y_von = |g: u64| -> f32 {
        if (max_g - min_g).abs() < 0.5 {
            flaeche.center().y
        } else {
            flaeche.bottom() - ((g as f64 - min_g) / dg) as f32 * flaeche.height()
        }
    };

    // Waagerechte Hilfslinien mit Größenangabe in kB.
    let schrift = FontId::proportional(9.5);
    for i in 0..=2 {
        let t = i as f32 / 2.0;
        let y = flaeche.top() + t * flaeche.height();
        p.line_segment(
            [Pos2::new(flaeche.left(), y), Pos2::new(flaeche.right(), y)],
            Stroke::new(1.0, Palette::grid_minor()),
        );
        let wert = max_g - (max_g - min_g) * t as f64;
        p.text(
            Pos2::new(flaeche.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            t_args_owned(
                "wiederherstellen.achse_kb",
                &[("wert", groesse_kb(wert.max(0.0) as u64))],
            ),
            schrift.clone(),
            Palette::text_faint(),
        );
    }
    // Grundlinie und Zeitangaben.
    p.line_segment(
        [
            Pos2::new(flaeche.left(), flaeche.bottom()),
            Pos2::new(flaeche.right(), flaeche.bottom()),
        ],
        Stroke::new(1.0, Palette::border()),
    );
    if let (Some(erst), Some(letzt)) = (staende.first(), staende.last()) {
        p.text(
            Pos2::new(flaeche.left(), flaeche.bottom() + 6.0),
            Align2::LEFT_TOP,
            kurz_zeit(&erst.zeit),
            schrift.clone(),
            Palette::text_faint(),
        );
        p.text(
            Pos2::new(flaeche.right(), flaeche.bottom() + 6.0),
            Align2::RIGHT_TOP,
            kurz_zeit(&letzt.zeit),
            schrift.clone(),
            Palette::text_faint(),
        );
    }

    // Verlauf als dünne Linie, darauf die Punkte.
    if staende.len() > 1 {
        let zug: Vec<Pos2> = staende
            .iter()
            .map(|s| Pos2::new(x_von(s.zyklus), y_von(s.groesse)))
            .collect();
        p.add(egui::Shape::line(
            zug,
            Stroke::new(1.0, Palette::accent_soft()),
        ));
    }
    let mut flaechen = Vec::with_capacity(staende.len());
    for s in staende {
        let c = Pos2::new(x_von(s.zyklus), y_von(s.groesse));
        let gewaehlt_hier = gewaehlt == Some(s.zyklus);
        let farbe = if s.verworfen {
            Palette::danger()
        } else if s.gesichert {
            Palette::text_faint()
        } else {
            Palette::accent()
        };
        let r = if gewaehlt_hier {
            PUNKT_RADIUS + 2.0
        } else {
            PUNKT_RADIUS
        };
        p.circle_filled(c, r, farbe);
        p.circle_stroke(c, r, Stroke::new(1.0, Palette::bg()));
        if gewaehlt_hier {
            p.circle_stroke(c, r + 3.0, Stroke::new(1.5, Palette::accent_text()));
        }
        flaechen.push((
            s.zyklus,
            Rect::from_center_size(c, Vec2::splat(2.0 * PUNKT_RADIUS + 4.0)),
        ));
    }

    // Zeiger über einem Punkt: Zeit und Größe daneben schreiben.
    if let Some(pos) = zeiger {
        if let Some(s) = staende
            .iter()
            .filter(|s| {
                let c = Pos2::new(x_von(s.zyklus), y_von(s.groesse));
                c.distance(pos) <= PUNKT_FANG
            })
            .min_by(|a, b| {
                let ca = Pos2::new(x_von(a.zyklus), y_von(a.groesse));
                let cb = Pos2::new(x_von(b.zyklus), y_von(b.groesse));
                ca.distance(pos).total_cmp(&cb.distance(pos))
            })
        {
            let c = Pos2::new(x_von(s.zyklus), y_von(s.groesse));
            p.text(
                Pos2::new(c.x + 8.0, c.y - 8.0),
                Align2::LEFT_BOTTOM,
                t_args_owned(
                    "wiederherstellen.punkt_kb",
                    &[
                        ("zeit", kurz_zeit(&s.zeit)),
                        ("groesse", groesse_kb(s.groesse)),
                    ],
                ),
                schrift,
                Palette::text(),
            );
        }
    }
    flaechen
}

/// Papierfläche mit der Vorschau des gewählten Standes.
fn vorschau_malen(
    ui: &egui::Ui,
    rect: Rect,
    app: &App,
    geladen: Option<u64>,
    gewaehlt: Option<u64>,
) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Palette::papier());
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Palette::border()));
    let doc = app
        .ui
        .wiederherstellen
        .as_ref()
        .and_then(|w| w.vorschau.as_ref());
    match doc {
        Some((z, d)) if Some(*z) == gewaehlt && Some(*z) == geladen => {
            canvas::paint_zeichnung(
                &p,
                rect.shrink(8.0),
                d,
                CurveQuality::mit_pfeilhoehe(app.prefs.kurven_toleranz_px),
            );
            p.text(
                Pos2::new(rect.left() + 8.0, rect.top() + 6.0),
                Align2::LEFT_TOP,
                tr("wiederherstellen.vorschau"),
                FontId::proportional(9.5),
                Palette::text_faint(),
            );
        }
        _ => {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                tr("wiederherstellen.kein_stand"),
                FontId::proportional(11.0),
                Palette::text_faint(),
            );
        }
    }
}

/// Zustand eines Standes als Text (entspricht der Datei, Arbeit, verworfen).
fn stand_status(s: &Stand) -> String {
    let schluessel = if s.verworfen {
        "wiederherstellen.status_verworfen"
    } else if s.gesichert {
        "wiederherstellen.status_gesichert"
    } else {
        "wiederherstellen.status_arbeit"
    };
    tr(schluessel).to_string()
}

/// Größe in Kilobyte, deutsch mit Komma und einer Stelle.
fn groesse_kb(bytes: u64) -> String {
    let kb = bytes as f64 / 1000.0;
    format!("{:.1}", kb).replace('.', ",")
}

#[cfg(test)]
mod tests {
    //! T0223: Das Fenster „Wiederherstellen“ — Liste, Zeitverlauf, Vorschau
    //! und der Weg zurück in die Zeichnung.

    use super::*;
    use crate::app::{App, TabKind};

    fn test_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-wiederherstellen-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Eine App, deren Sicherungsordner der Wegwerfordner ist.
    fn app_mit_pool(dir: &std::path::Path) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app.prefs.sicherungen_pfad = dir.to_string_lossy().to_string();
        app.sicherung = None;
        app.ui.layers_open = false;
        app.ui.props_open = false;
        app.ui.blatt_open = false;
        app
    }

    /// Zwei Zeichnungen im Pool: „Wohnhaus“ mit drei Ständen, „Garage“ mit
    /// zweien. Die Wände wachsen je Stand, damit die Größen sich unterscheiden.
    fn pool_fuellen(dir: &std::path::Path) -> (String, String) {
        let einst = jjcad_core::sicherung::Einstellungen::default();
        let mut pool = jjcad_core::sicherung::Sicherung::oeffnen(dir);
        let mut haus = jjcad_core::model::Document::new();
        haus.name = "Wohnhaus".into();
        haus.id = "aaaaaaaa-0000-4000-8000-0000000000aa".into();
        // Der Stand trägt den Speicherort der Zieldatei (wie nach dem Speichern).
        haus.path = Some(
            dir.join("Wohnhaus.jjcad")
                .to_string_lossy()
                .to_string(),
        );
        for i in 0..3 {
            let _ = jjcad_core::actions::execute(
                &mut haus,
                "draw.wall",
                &serde_json::json!({
                    "a": [0.0, 0.0],
                    "b": [4000.0 + 500.0 * i as f64, 2500.0],
                    "thickness": 0.24
                }),
            );
            pool.sichern(&mut haus, &einst).unwrap();
        }
        let mut garage = jjcad_core::model::Document::new();
        garage.name = "Garage".into();
        garage.id = "bbbbbbbb-0000-4000-8000-0000000000bb".into();
        for i in 0..2 {
            let _ = jjcad_core::actions::execute(
                &mut garage,
                "draw.wall",
                &serde_json::json!({
                    "a": [0.0, 0.0],
                    "b": [3000.0 + 400.0 * i as f64, 2000.0],
                    "thickness": 0.24
                }),
            );
            pool.sichern(&mut garage, &einst).unwrap();
        }
        (haus.id, garage.id)
    }

    fn bild(app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    fn knopf(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn klick(app: &mut App, pos: Pos2) {
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![knopf(pos, true)],
            vec![knopf(pos, false)],
            vec![],
        ] {
            let _ = bild(app, events);
        }
    }

    /// Die Liste nennt jede Zeichnung aus dem Ordner genau einmal, mit der
    /// Zahl ihrer Stände.
    #[test]
    fn liste_zeigt_alle_zeichnungen_des_ordners() {
        let dir = test_dir("liste");
        let (haus, garage) = pool_fuellen(&dir);
        let mut app = app_mit_pool(&dir);
        app.oeffne_wiederherstellen();
        let dateien = app.wiederherstellen_dateien();
        assert_eq!(dateien.len(), 2, "eine Zeile je Zeichnung: {dateien:?}");
        let garage_eintrag = dateien.iter().find(|d| d.datei == garage).unwrap();
        let haus_eintrag = dateien.iter().find(|d| d.datei == haus).unwrap();
        assert_eq!(haus_eintrag.staende, 3);
        assert_eq!(garage_eintrag.staende, 2);
        assert_eq!(garage_eintrag.name, "Garage");
        // Alphabetisch: Garage steht vor Wohnhaus.
        assert_eq!(dateien[0].name, "Garage");
    }

    /// Klick in der Liste wählt die Zeichnung, das Diagramm zeigt ihre Stände
    /// als Punkte; ein Klick auf einen Punkt lädt den Stand in die Vorschau.
    #[test]
    fn klick_waehlt_zeichnung_und_stand() {
        let dir = test_dir("klick");
        let (_haus, garage) = pool_fuellen(&dir);
        let mut app = app_mit_pool(&dir);
        app.oeffne_wiederherstellen();
        // Zwei Bilder: das Fenster legt sich im ersten auf, im zweiten steht
        // die Liste (wie bei den übrigen Dialog-Prüfungen).
        let _ = bild(&mut app, Vec::new());
        let _ = bild(&mut app, Vec::new());

        let mut liste: Vec<(String, Rect)> = Vec::new();
        for _ in 0..3 {
            let _ = bild(&mut app, Vec::new());
            liste = app
                .ui
                .wiederherstellen
                .as_ref()
                .map(|w| w.last_liste.clone())
                .unwrap_or_default();
        }
        assert!(!liste.is_empty(), "die Zeichnungsliste fehlt");
        assert_eq!(liste.len(), 2, "die Zeichnungsliste fehlt: {liste:?}");
        let (_, zeile) = liste
            .iter()
            .find(|(id, _)| id == &garage)
            .expect("Garage in der Liste");
        klick(&mut app, zeile.center());

        let stand = app.ui.wiederherstellen.as_ref().expect("Fenster offen");
        assert_eq!(stand.datei.as_deref(), Some(garage.as_str()));
        assert!(stand.zyklus.is_none(), "neuer Verlauf ohne Auswahl");

        // Das Fenster richtet sich nach der Auswahl neu aus — erst danach
        // stehen die Punkte an ihrem endgültigen Platz.
        let _ = bild(&mut app, Vec::new());
        let _ = bild(&mut app, Vec::new());
        let punkte = app
            .ui
            .wiederherstellen
            .as_ref()
            .map(|w| w.last_punkte.clone())
            .unwrap_or_default();
        assert_eq!(punkte.len(), 2, "zwei Stände der Garage: {punkte:?}");
        let (zyklus, flaeche) = punkte[0].clone();
        klick(&mut app, flaeche.center());

        let stand = app.ui.wiederherstellen.as_ref().expect("Fenster offen");
        assert_eq!(stand.zyklus, Some(zyklus), "der Punkt wählt seinen Stand");
        let (geladen, doc) = stand
            .vorschau
            .as_ref()
            .expect("die Vorschau ist geladen");
        assert_eq!(*geladen, zyklus);
        assert!(
            !doc.entities.is_empty(),
            "die Vorschau muss den Inhalt zeigen"
        );
        assert!(
            stand.last_vorschau.is_some(),
            "die Vorschaufläche wird gezeichnet"
        );
    }

    /// „Zeichnung öffnen“ legt den Stand als Reiter ab — mit dem Speicherort
    /// der Zieldatei und als geändert, damit „Speichern“ bewusst überschreibt.
    #[test]
    fn stand_oeffnet_als_reiter() {
        let dir = test_dir("oeffnen");
        let (haus, _garage) = pool_fuellen(&dir);
        let mut app = app_mit_pool(&dir);
        let vorher = app.tabs.len();
        let zyklus = {
            let pool = app.sicherung_pool();
            pool.staende_chronologisch(&haus)[1].zyklus
        };
        let zeit = app.wiederherstellen_stand_oeffnen(zyklus).expect("öffnen");
        assert!(!zeit.is_empty());
        assert_eq!(app.tabs.len(), vorher + 1, "ein Reiter mehr");
        let doc = app.doc();
        assert!(doc.modified, "der Stand ist nicht der Dateiinhalt");
        assert!(!doc.entities.is_empty(), "der Reiter trägt den Inhalt");
        assert_eq!(app.tab().tab, TabKind::Model);
        assert!(
            doc.path
                .as_deref()
                .map(|p| p.ends_with("Wohnhaus.jjcad"))
                .unwrap_or(false),
            "der Reiter trägt den Speicherort der Zieldatei: {:?}",
            doc.path
        );

        // Derselbe Speicherort liegt schon offen: ein zweiter Stand ersetzt
        // den Inhalt, statt einen zweiten Reiter auf denselben Pfad zu legen.
        let anderer = {
            let pool = app.sicherung_pool();
            pool.staende_chronologisch(&haus)[0].zyklus
        };
        app.wiederherstellen_stand_oeffnen(anderer).expect("öffnen");
        assert_eq!(
            app.tabs.len(),
            vorher + 1,
            "kein zweiter Reiter auf denselben Pfad"
        );
        assert!(app.doc().modified);
    }
}
