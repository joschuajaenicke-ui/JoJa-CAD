//! Text direkt in der Zeichnungsebene schreiben (T0019).
//!
//! Das Textwerkzeug öffnet keinen Dialog mehr: nach dem Klick wird direkt an
//! der Textposition **in** die Zeichnung geschrieben. Ein Doppelklick auf einen
//! vorhandenen Text öffnet dessen Inhalt zum Ändern.
//!
//! Während der Eingabe sieht der Text **genau so aus wie in der Zeichnung**:
//! keine Fläche, kein Rahmen — dieselbe Schriftgröße (`Höhe · Zoom · 1,32`),
//! dieselbe Platzierung (`canvas::paint_text`, Anker links/mittig) und dieselbe
//! Farbe (Objekt- bzw. Layerfarbe). Der bearbeitete Text wird dafür aus der
//! Anzeige genommen, damit er nicht doppelt dasteht.
//!
//! Esc übernimmt, ein Klick daneben übernimmt ebenfalls, Enter fügt eine neue
//! Zeile ein, Rechtsklick bricht ab.

use crate::app::{world_to_screen, App, View};
use crate::theme::Palette;
use egui::{Color32, FontId, Id, Pos2, Rect, Stroke, Ui, Vec2};
use jjcad_core::geom::Vec2 as V2;
use jjcad_core::model::{EntityKind, Id as EId, TextAlignH, TextAlignV};
use jjcad_core::render;
use jjcad_core::style::Color;
use serde_json::json;

/// Bildschirmfarbe einer Zeichnungsfarbe (wie `canvas::color32`).
fn color32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Egui-ID des Feldes — auch für Tests und die Fokusprüfung.
pub fn feld_id() -> Id {
    Id::new("texteingabe_feld")
}

/// Laufende Texteingabe in der Zeichenfläche.
pub struct Texteingabe {
    /// Einfügepunkt des Textes in Weltkoordinaten — die Stelle, die die
    /// Ausrichtung (`align`/`valign`) benennt, nicht die linke untere Ecke.
    pub pos: V2,
    pub text: String,
    pub height: f64,
    pub angle: f64,
    /// Ausrichtung des Textes (auch während des Schreibens schon wirksam)
    pub align: TextAlignH,
    pub valign: TextAlignV,
    /// `Some` = vorhandener Text wird geändert, `None` = neuer Text
    pub id: Option<EId>,
    /// Farbe, in der der Text in der Zeichnung erscheint (Objekt- oder Layerfarbe)
    pub farbe: Color,
    /// Beim ersten Frame den Fokus (Cursor) holen
    pub fokus: bool,
    /// Bildschirmfläche des Feldes — ein Klick daneben übernimmt
    pub flaeche: Option<Rect>,
}

impl Texteingabe {
    fn neu(
        pos: V2,
        height: f64,
        text: String,
        angle: f64,
        align: TextAlignH,
        valign: TextAlignV,
        id: Option<EId>,
        farbe: Color,
    ) -> Self {
        Self {
            pos,
            text,
            height,
            angle,
            align,
            valign,
            id,
            farbe,
            fokus: true,
            flaeche: None,
        }
    }
}

/// Neuen Text an dieser Stelle schreiben — der Cursor steht sofort darin.
pub fn start(app: &mut App, pos: V2) {
    app.tool = None;
    // Angezeigt wird der Text in der Layerfarbe (wie bei `draw.text` ohne
    // eigene Farbe) — die Eingabe soll nicht anders aussehen als das Ergebnis.
    let farbe = app
        .doc()
        .layer(app.doc().current_layer)
        .map(|l| l.color)
        .unwrap_or(Color::BLACK);
    app.ui.texteingabe = Some(Texteingabe::neu(
        pos,
        app.options.text_height,
        String::new(),
        0.0,
        app.options.text_align,
        app.options.text_valign,
        None,
        farbe,
    ));
    app.mitteilung("Text · Text eingeben (Esc übernimmt)");
}

/// Inhalt eines vorhandenen Textes bearbeiten (Doppelklick).
pub fn start_entitaet(app: &mut App, id: EId) -> bool {
    let Some(e) = app.doc().entity(id) else {
        return false;
    };
    let EntityKind::Text {
        p,
        text,
        height,
        angle,
        align,
        valign,
        ..
    } = &e.kind
    else {
        return false;
    };
    let (p, text, height, angle) = (*p, text.clone(), *height, *angle);
    let (align, valign) = (*align, *valign);
    let farbe = app.doc().effective_color(e);
    app.tool = None;
    // Ohne Auswahl: der blaue Auswahlrahmen des Textes würde sonst um den
    // bearbeiteten Text stehen — beim Schreiben soll nur der Text dastehen.
    app.session_mut().clear_selection();
    app.ui.texteingabe =
        Some(Texteingabe::neu(p, height, text, angle, align, valign, Some(id), farbe));
    app.mitteilung("Text · Inhalt ändern (Esc übernimmt)");
    true
}

/// Text unter dem Punkt (Weltkoordinate) — es zählt der sichtbare Rahmen, also
/// dieselbe Fläche, die bei Auswahl blau markiert wird.
pub fn text_bei(app: &App, w: V2) -> Option<EId> {
    let doc = app.doc();
    let tol = 2.0; // mm, damit der Rand des Rahmens noch trifft
    doc.ents().iter().rev().find_map(|e| {
        if !matches!(e.kind, EntityKind::Text { .. }) {
            return None;
        }
        let b = render::entity_bbox(doc, e);
        if !b.is_valid() {
            return None;
        }
        let drin = w.x >= b.min.x - tol
            && w.x <= b.max.x + tol
            && w.y >= b.min.y - tol
            && w.y <= b.max.y + tol;
        if drin {
            Some(e.id)
        } else {
            None
        }
    })
}

/// Eckpunkt des Eingabefelds auf dem Bildschirm (linke obere Ecke).
///
/// Das Feld setzt seinen Inhalt **linksbündig**; die Ausrichtung entsteht
/// dadurch, dass das Feld entsprechend versetzt wird: bei „Zentriert" um die
/// halbe Textbreite nach links, bei „Rechtsbündig" um die ganze — der Text
/// steht dann genau dort, wo er nach dem Übernehmen erscheint. Senkrecht gilt
/// dieselbe Lage wie in der Anzeige (`canvas::text_senkrecht`).
pub(crate) fn feld_lage(
    anker: Pos2,
    align: TextAlignH,
    valign: TextAlignV,
    textbreite: f32,
    blockhoehe: f32,
) -> Pos2 {
    let dx = match align {
        TextAlignH::Left => 0.0,
        TextAlignH::Center => -textbreite * 0.5,
        TextAlignH::Right => -textbreite,
    };
    let dy = match valign {
        TextAlignV::Top => 0.0,
        TextAlignV::Middle => -blockhoehe * 0.5,
        TextAlignV::Bottom => -blockhoehe,
    };
    Pos2::new(anker.x + dx, anker.y + dy)
}

/// Übernimmt den Text als Objekt (neu) bzw. ändert den vorhandenen.
fn uebernehmen(app: &mut App, ctx: &egui::Context) {
    let Some(t) = app.ui.texteingabe.take() else {
        return;
    };
    ctx.memory_mut(|m| m.stop_text_input());
    // Ein Zeilenumbruch am Ende entsteht leicht beim Tippen und wäre im
    // Textkasten eine leere Zeile.
    let text = t.text.trim_end_matches('\n').to_string();
    if text.trim().is_empty() {
        app.mitteilung("Text · abgebrochen");
        return;
    }
    let unit = app.unit();
    let (action, params, meldung) = match t.id {
        Some(id) => (
            "modify.set",
            json!({"ids": [id], "text": text, "height": unit.from_mm(t.height),
                   "align": t.align.key(), "valign": t.valign.key()}),
            "Text · geändert",
        ),
        None => (
            "draw.text",
            json!({"p": [unit.from_mm(t.pos.x), unit.from_mm(t.pos.y)],
                   "text": text,
                   "height": unit.from_mm(t.height),
                   "angle": t.angle.to_degrees(),
                   "align": t.align.key(),
                   "valign": t.valign.key()}),
            "Text · gesetzt",
        ),
    };
    match app.session_mut().execute(action, &params) {
        Ok(_) => {
            app.mark_changed();
            app.mitteilung(meldung);
            // Zuletzt gesetzte Werte bleiben für das nächste Element stehen.
            app.options.text = text.clone();
            app.options.text_height = t.height;
        }
        Err(e) => app.toast(e),
    }
}

/// Abbruch: der vorhandene Text bleibt unverändert, ein neuer entsteht nicht.
fn abbrechen(app: &mut App, ctx: &egui::Context) {
    if app.ui.texteingabe.take().is_some() {
        ctx.memory_mut(|m| m.stop_text_input());
        app.mitteilung("Text · abgebrochen");
    }
}

/// Schreibt an der Textposition und wertet Tastatur und Klicks aus.
pub fn show(app: &mut App, ui: &mut Ui, rect: Rect, view: View) {
    if app.ui.texteingabe.is_none() {
        return;
    }
    let ctx = ui.ctx().clone();

    // Esc übernimmt, Rechtsklick bricht ab. Vor dem Textfeld geprüft: das
    // TextEdit verbraucht die Tastendrücke selbst, und egui nimmt einem Feld
    // bei Esc den Fokus.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        uebernehmen(app, &ctx);
        return;
    }
    if ctx.input(|i| i.pointer.secondary_pressed()) {
        abbrechen(app, &ctx);
        return;
    }

    let mut t = match app.ui.texteingabe.take() {
        Some(t) => t,
        None => return,
    };

    // Genau die Schrift der Zeichnung (`canvas::paint_text`): Höhe × Zoom × 1,32.
    let size = (t.height * view.zoom * 1.32).max(3.0) as f32;
    let font = FontId::proportional(size);
    let zeilenhoehe = ctx.fonts(|f| f.row_height(&font));
    let anker = world_to_screen(rect, view, t.pos);
    let farbe = color32(t.farbe);
    // Breite aus der echten Schriftbreite: der Text darf beim Schreiben **nicht**
    // umbrechen — das tut der eingefügte Text auch nicht, sonst springt er beim
    // Übernehmen in eine andere Form.
    let (textbreite, texthoehe) = ctx.fonts(|f| {
        let g = f.layout_no_wrap(t.text.clone(), font.clone(), farbe);
        (g.size().x, g.size().y)
    });
    let breite = (textbreite + size * 1.5).max(if t.text.is_empty() { 20.0 } else { 8.0 });
    // Eigener Setzer: ohne Umbruch, nur echte Zeilenumbrüche (Enter).
    //
    // **Im Feld wird immer linksbündig gesetzt**, auch wenn der Text später
    // zentriert oder rechtsbündig steht: `LayoutJob::halign` legt die Zeilen um
    // den Ursprung (zentriert ab −Breite/2), das Feld beginnt aber am Ursprung —
    // die Zeilen ragten damit zur Hälfte (bzw. ganz) links aus dem Feld und
    // wurden abgeschnitten („abcdefg" zeigte nur „defg", T0028 Turn 2). Die
    // Lage übernimmt stattdessen das Feld selbst (`feld_lage`).
    let setzer_font = font.clone();
    let mut setzer = move |ui: &egui::Ui, text: &str, _wrap: f32| {
        ui.fonts(|f| f.layout_no_wrap(text.to_owned(), setzer_font.clone(), farbe))
    };
    let block_h = texthoehe.max(zeilenhoehe);
    let lage = feld_lage(anker, t.align, t.valign, textbreite, block_h);

    egui::Area::new(Id::new("texteingabe"))
        .order(egui::Order::Foreground)
        // Die Ausrichtung gilt schon beim Schreiben: „Zentriert" wächst nach
        // beiden Seiten, „Unten" steht über dem Anker.
        .fixed_pos(lage)
        .show(&ctx, |ui| {
            // Keine Fläche, kein Rahmen: der Text steht frei in der Zeichnung
            // und sieht während der Eingabe genauso aus wie danach.
            ui.visuals_mut().text_cursor.stroke = Stroke::new(1.5_f32, Palette::ACCENT);
            // Cursor ohne Blinken: er ist die einzige Markierung der
            // Schreibstelle.
            ui.visuals_mut().text_cursor.blink = false;
            ui.visuals_mut().selection.bg_fill =
                Color32::from_rgba_unmultiplied(0x2F, 0x8C, 0xF0, 90);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut t.text)
                    .layouter(&mut setzer)
                    .id(feld_id())
                    .frame(false)
                    .margin(Vec2::ZERO)
                    .font(font.clone())
                    .text_color(farbe)
                    .desired_width(breite)
                    .desired_rows(1),
            );
            if t.fokus {
                resp.request_focus();
                t.fokus = false;
            }
            t.flaeche = Some(resp.rect.expand(2.0));
            // Hinweis nur solange nichts getippt ist: beim Schreiben soll nur
            // der Text dastehen. Er steht unter dem Text, nicht dahinter.
            if t.text.is_empty() {
                ui.label(
                    egui::RichText::new("Esc übernimmt · Enter neue Zeile · Rechtsklick bricht ab")
                        .size(10.0)
                        .color(Palette::TEXT_FAINT),
                );
            }
        });

    // Klick neben den Text übernimmt (wie in Office-Programmen).
    let aussen_geklickt = ctx.input(|i| i.pointer.primary_pressed())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .map(|p| !t.flaeche.map(|r| r.contains(p)).unwrap_or(true))
            .unwrap_or(false);

    app.ui.texteingabe = Some(t);
    if aussen_geklickt {
        uebernehmen(app, &ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use jjcad_core::model::EntityKind;

    fn app_mm() -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app
    }

    fn texte(app: &App) -> Vec<(String, f64)> {
        app.doc()
            .ents()
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Text { text, height, .. } => Some((text.clone(), *height)),
                _ => None,
            })
            .collect()
    }

    /// Klick mit dem Textwerkzeug öffnet das Feld — kein Dialog — und der Cursor
    /// steht sofort darin (T0019).
    #[test]
    fn textwerkzeug_oeffnet_feld_mit_fokus() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");

        start(&mut app, V2::new(100.0, 200.0));
        assert!(app.tool.is_none(), "kein Werkzeug mehr aktiv");
        let t = app.ui.texteingabe.as_ref().expect("Eingabe offen");
        assert_eq!(t.id, None, "neuer Text");
        assert_eq!(t.pos, V2::new(100.0, 200.0));

        // Ein Frame: das Feld holt sich den Fokus.
        let flaeche = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let view = View {
            center: V2::new(0.0, 0.0),
            zoom: 0.2,
            rot: 0.0,
        };
        let raw = egui::RawInput {
            screen_rect: Some(flaeche),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(&mut app, ui, flaeche, view);
            });
        });
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(feld_id()),
            "Cursor muss im Textfeld stehen"
        );
    }

    /// Getippter Text wird mit Esc übernommen und steht danach in der Zeichnung.
    #[test]
    fn esc_uebernimmt_den_getippten_text() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.options.text_height = 250.0;
        start(&mut app, V2::new(1000.0, 500.0));

        let flaeche = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let view = View {
            center: V2::new(1000.0, 500.0),
            zoom: 0.2,
            rot: 0.0,
        };
        let mut frames = vec![
            Vec::new(),
            vec![egui::Event::Text("Wohnen".into())],
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        ];
        for events in frames.drain(..) {
            let raw = egui::RawInput {
                screen_rect: Some(flaeche),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(&mut app, ui, flaeche, view);
                });
            });
        }

        assert!(app.ui.texteingabe.is_none(), "Eingabe abgeschlossen");
        assert_eq!(texte(&app), vec![("Wohnen".to_string(), 250.0)]);
    }

    /// Lage des Eingabefelds: der Einfügepunkt der Ausrichtung steht am Anker.
    ///
    /// Das Feld setzt linksbündig; zentriert/rechtsbündig entsteht durch den
    /// Versatz des Feldes um die halbe bzw. ganze Textbreite.
    #[test]
    fn feld_lage_setzt_den_einfuegepunkt_auf_den_anker() {
        let anker = Pos2::new(400.0, 300.0);
        let breite = 120.0;
        let hoehe = 40.0;
        for h in TextAlignH::ALL {
            for v in TextAlignV::ALL {
                let p = feld_lage(anker, h, v, breite, hoehe);
                let x = match h {
                    TextAlignH::Left => p.x,
                    TextAlignH::Center => p.x + breite / 2.0,
                    TextAlignH::Right => p.x + breite,
                };
                let y = match v {
                    TextAlignV::Top => p.y,
                    TextAlignV::Middle => p.y + hoehe / 2.0,
                    TextAlignV::Bottom => p.y + hoehe,
                };
                assert!((x - anker.x).abs() < 1e-4, "{}/{}: x {x}", h.label(), v.label());
                assert!((y - anker.y).abs() < 1e-4, "{}/{}: y {y}", h.label(), v.label());
            }
        }
    }

    /// Doppelklick: der Treffer richtet sich nach dem Textkasten.
    #[test]
    fn text_bei_trifft_den_kasten() {
        let mut app = app_mm();
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1000.0, 500.0], "text": "Wohnen", "height": 250.0}),
            )
            .expect("Text anlegen");
        let id = app.doc().ents()[0].id;

        assert_eq!(text_bei(&app, V2::new(1100.0, 500.0)), Some(id), "im Kasten");
        assert_eq!(text_bei(&app, V2::new(4000.0, 500.0)), None, "daneben");
    }

    /// Bearbeiten eines vorhandenen Textes behält Höhe und Drehung.
    #[test]
    fn bearbeiten_aendert_nur_den_inhalt() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1000.0, 500.0], "text": "Alt", "height": 250.0, "angle": 0.0}),
            )
            .expect("Text anlegen");
        let id = app.doc().ents()[0].id;
        assert!(start_entitaet(&mut app, id), "Bearbeiten startet");
        assert_eq!(app.ui.texteingabe.as_ref().unwrap().text, "Alt");
        assert_eq!(app.ui.texteingabe.as_ref().unwrap().height, 250.0);

        let flaeche = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let view = View {
            center: V2::new(1000.0, 500.0),
            zoom: 0.2,
            rot: 0.0,
        };
        let mut frames = vec![
            Vec::new(),
            vec![egui::Event::Text("Neu".into())],
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        ];
        for events in frames.drain(..) {
            let raw = egui::RawInput {
                screen_rect: Some(flaeche),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(&mut app, ui, flaeche, view);
                });
            });
        }
        assert_eq!(texte(&app), vec![("AltNeu".to_string(), 250.0)], "ein Text");
    }

    /// Doppelklick in der Zeichenfläche öffnet den vorhandenen Text — geprüft
    /// über echte Zeigerereignisse im ganzen Fenster (T0019).
    #[test]
    fn doppelklick_auf_text_oeffnet_die_eingabe() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [2000.0, 1500.0], "text": "Wohnen", "height": 250.0}),
            )
            .expect("Text anlegen");

        let mut zeit = 1.0;
        let mut frame = |app: &mut App, events: Vec<egui::Event>, zeit: &mut f64| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
                time: Some(*zeit),
                predicted_dt: 0.016,
                events,
                ..Default::default()
            };
            *zeit += 0.1;
            let _ = ctx.run(input, |c| app.ui(c));
        };
        frame(&mut app, vec![], &mut zeit);
        let pos = world_to_screen(app.ui.last_canvas, app.current_view(), V2::new(2100.0, 1500.0));

        let klick = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![egui::Event::PointerMoved(pos)], &mut zeit);
        frame(&mut app, vec![klick(pos, true)], &mut zeit);
        frame(&mut app, vec![klick(pos, false)], &mut zeit);
        frame(&mut app, vec![klick(pos, true)], &mut zeit);
        frame(&mut app, vec![klick(pos, false)], &mut zeit);

        let t = app
            .ui
            .texteingabe
            .as_ref()
            .unwrap_or_else(|| panic!("Doppelklick muss die Eingabe öffnen (pos {pos:?})"));
        assert_eq!(t.text, "Wohnen", "Inhalt wird übernommen");
        assert_eq!(t.id, Some(app.doc().ents()[0].id));
    }

    /// Solange das Feld offen ist, gehört die Tastatur dem Text: „l" startet
    /// keine Linie, „w" keine Wand (T0019).
    #[test]
    fn tippen_startet_kein_werkzeug() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        start(&mut app, V2::new(0.0, 0.0));
        let mut zeit = 1.0;
        let mut frame = |app: &mut App, events: Vec<egui::Event>, zeit: &mut f64| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
                time: Some(*zeit),
                predicted_dt: 0.016,
                events,
                ..Default::default()
            };
            *zeit += 0.1;
            let _ = ctx.run(input, |c| app.ui(c));
        };
        frame(&mut app, vec![], &mut zeit);
        frame(&mut app, vec![egui::Event::Text("l".into())], &mut zeit);
        frame(&mut app, vec![egui::Event::Text("w".into())], &mut zeit);
        assert!(app.tool.is_none(), "kein Werkzeug gestartet");
        assert_eq!(
            app.ui.texteingabe.as_ref().map(|t| t.text.as_str()),
            Some("lw")
        );
    }

    /// Enter fügt eine neue Zeile ein und schließt die Eingabe nicht (T0019).
    #[test]
    fn enter_fuegt_eine_neue_zeile_ein() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        start(&mut app, V2::new(0.0, 0.0));
        let mut zeit = 1.0;
        let mut frame = |app: &mut App, events: Vec<egui::Event>, zeit: &mut f64| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
                time: Some(*zeit),
                predicted_dt: 0.016,
                events,
                ..Default::default()
            };
            *zeit += 0.1;
            let _ = ctx.run(input, |c| app.ui(c));
        };
        let enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![], &mut zeit);
        frame(&mut app, vec![egui::Event::Text("EG".into())], &mut zeit);
        frame(&mut app, vec![enter], &mut zeit);
        frame(&mut app, vec![egui::Event::Text("OG".into())], &mut zeit);
        let t = app
            .ui
            .texteingabe
            .as_ref()
            .expect("Eingabe bleibt nach Enter offen");
        assert_eq!(t.text, "EG\nOG", "Enter ist der Zeilenumbruch");
        assert!(app.doc().ents().is_empty(), "noch kein Text angelegt");
    }

    /// Rechtsklick (bzw. `abbrechen`) lässt einen neuen Text verschwinden.
    #[test]
    fn rechtsklick_bricht_ab() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        start(&mut app, V2::new(0.0, 0.0));
        abbrechen(&mut app, &ctx);
        assert!(app.ui.texteingabe.is_none());
        assert!(app.doc().ents().is_empty(), "kein Text angelegt");
    }
}

#[cfg(test)]
mod tests_eingabe_sichtbar {
    //! Der ganze Text steht im Eingabefeld — auch zentriert und rechtsbündig
    //! (T0028, Turn 2).
    //!
    //! Vorher setzte der Setzer den Text mit `halign` wie die Zeichnung; das Feld
    //! beginnt aber am Ursprung der Galley, also lag ein zentrierter Text zur
    //! Hälfte links außerhalb des Feldes (bei „abcdefg" war „abc" weg und „d"
    //! halb). Jetzt setzt das Feld linksbündig und wandert selbst.

    use super::*;
    use crate::app::App;

    fn app_mm() -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.options.text_height = 250.0;
        app
    }

    fn flaeche() -> Rect {
        // Groß genug, dass das Feld nicht an den Fensterrand geschoben wird
        // (ein `Area` bleibt im Bild, sonst wandert die Vorschau mit).
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))
    }

    fn view() -> View {
        View {
            center: V2::new(1000.0, 500.0),
            zoom: 0.2,
            rot: 0.0,
        }
    }

    /// Ein Frame Zeichnung; liefert (Text, Kasten, Clip) jedes gezeichneten
    /// Textes.
    fn bild(ctx: &egui::Context, app: &mut App, zeit: f64) -> Vec<(String, Rect, Rect)> {
        let raw = egui::RawInput {
            screen_rect: Some(flaeche()),
            time: Some(zeit),
            predicted_dt: 0.016,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(app, ui, flaeche(), view());
            });
        });
        let mut gefunden = Vec::new();
        for cs in &out.shapes {
            if let egui::Shape::Text(t) = &cs.shape {
                // `galley.rect` trägt den Versatz der Zeilenausrichtung
                // (halign) — nur mit ihm ist der Kasten der *gemalten* Schrift
                // richtig; `from_min_size(t.pos, size)` verschluckt ihn.
                gefunden.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                    cs.clip_rect,
                ));
            }
        }
        gefunden
    }

    /// Getippter Text bleibt in allen drei waagerechten Ausrichtungen sichtbar
    /// und steht dort, wo er nach dem Übernehmen erscheint.
    #[test]
    fn text_bleibt_im_feld_sichtbar() {
        for align in TextAlignH::ALL {
            let ctx = egui::Context::default();
            let mut app = app_mm();
            app.options.text_align = align;
            app.options.text_valign = TextAlignV::Middle;
            start(&mut app, V2::new(1000.0, 500.0));
            if let Some(t) = app.ui.texteingabe.as_mut() {
                t.text = "abcdefg".to_string();
            }
            // Erster Frame setzt den Inhalt, zweiter zeichnet ihn.
            let _ = bild(&ctx, &mut app, 1.0);
            let bilder = bild(&ctx, &mut app, 1.1);
            let (_, kasten, clip) = bilder
                .iter()
                .find(|(t, _, _)| t == "abcdefg")
                .unwrap_or_else(|| panic!("{}: Text nicht gezeichnet", align.label()));

            assert!(kasten.width() > 1.0, "{}: leerer Kasten", align.label());
            assert!(
                clip.contains_rect(*kasten),
                "{}: der Text ragt aus dem Feld ({:?} außerhalb von {:?})",
                align.label(),
                kasten,
                clip
            );
            let anker = world_to_screen(flaeche(), view(), V2::new(1000.0, 500.0));
            let x = match align {
                TextAlignH::Left => kasten.left(),
                TextAlignH::Center => kasten.center().x,
                TextAlignH::Right => kasten.right(),
            };
            assert!(
                (x - anker.x).abs() < 1.5,
                "{}: Kante bei {:.1}, Anker bei {:.1}",
                align.label(),
                x,
                anker.x
            );
            assert!(
                (kasten.center().y - anker.y).abs() < 1.5,
                "{}: Mitte bei {:.1}, Anker bei {:.1}",
                align.label(),
                kasten.center().y,
                anker.y
            );
        }
    }
}
