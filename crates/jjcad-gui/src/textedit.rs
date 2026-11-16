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
use jjcad_core::model::{EntityKind, Id as EId, TextAlignH, TextAlignV, TextRun};
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

/// Markierter Zeichenbereich im Editor (Zeichenindizes), falls das Feld den
/// Fokus hat. Wird im Eigenschaften-Panel für die Umschalter gebraucht.
pub fn markierung(ctx: &egui::Context) -> Option<(usize, usize)> {
    egui::text_edit::TextEditState::load(ctx, feld_id())
        .and_then(|st| st.cursor.char_range())
        .map(|r| {
            let (a, b) = (r.primary.index, r.secondary.index);
            (a.min(b), a.max(b))
        })
        .filter(|(a, b)| b > a)
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
    /// Schriftart — beim Ändern aus dem Text, beim Neuen aus dem Panel.
    pub font: jjcad_core::model::TextFont,
    /// Zeilenabstand als Vielfaches der Zeilenhöhe der Schrift.
    pub line_spacing: f64,
    /// Umrandung dauerhaft anzeigen (T0126) — beim Ändern aus dem Text,
    /// beim Neuen aus dem Panel.
    pub frame: bool,
    /// Runde Umrandung (Radius = halbe Schrifthöhe).
    pub frame_round: bool,
    /// Hintergrundfarbe (None = keine Fläche).
    pub background: Option<jjcad_core::style::Color>,
    /// Auszeichnung je Lauf, aus dem bearbeiteten Text übernommen — beim
    /// Übernehmen auf den neuen Text übertragen, damit Formatierung bleibt.
    pub runs: Vec<TextRun>,
    /// Textstand des letzten Frames: daraus wird erkannt, was eingefügt wurde
    /// (damit die vorgemerkte Auszeichnung dort ankommt).
    letzter_text: String,
    /// Auszeichnung für das nächste getippte Zeichen (Umschalter im Panel).
    pub pending: Option<jjcad_core::text::RunStyle>,
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
        font: jjcad_core::model::TextFont,
        line_spacing: f64,
        frame: bool,
        frame_round: bool,
        background: Option<jjcad_core::style::Color>,
    ) -> Self {
        Self {
            pos,
            text,
            height,
            font,
            angle,
            align,
            valign,
            id,
            farbe,
            runs: Vec::new(),
            line_spacing,
            frame,
            frame_round,
            background,
            letzter_text: String::new(),
            pending: None,
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
    // Eine im Panel gewählte Farbe gilt auch für den neuen Text, sonst die
    // Layerfarbe (wie `draw.text` ohne eigene Farbe).
    let farbe = app.options.text_color.unwrap_or_else(|| {
        app.doc()
            .layer(app.doc().current_layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    app.ui.texteingabe = Some(Texteingabe::neu(
        pos,
        app.options.text_height,
        String::new(),
        app.options.text_angle.to_radians(),
        app.options.text_align,
        app.options.text_valign,
        None,
        farbe,
        app.options.text_font,
        app.options.text_line_spacing,
        app.options.text_frame,
        app.options.text_frame_round,
        app.options.text_background,
    ));
    app.mitteilung(jjcad_core::i18n::tr("gui.textedit.text_text_eingeben_esc_uebernimmt"));
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
    let EntityKind::Text {
        runs,
        font,
        line_spacing,
        frame,
        frame_round,
        background,
        ..
    } = &e.kind
    else {
        return false;
    };
    let font = *font;
    let line_spacing = *line_spacing;
    let (frame, frame_round, background) = (*frame, *frame_round, *background);
    let (p, text, height, angle) = (*p, text.clone(), *height, *angle);
    let (align, valign) = (*align, *valign);
    let runs = runs.clone();
    let farbe = app.doc().effective_color(e);
    app.tool = None;
    // Ohne Auswahl: der blaue Auswahlrahmen des Textes würde sonst um den
    // bearbeiteten Text stehen — beim Schreiben soll nur der Text dastehen.
    app.session_mut().clear_selection();
    let mut e = Texteingabe::neu(
        p,
        height,
        text,
        angle,
        align,
        valign,
        Some(id),
        farbe,
        font,
        line_spacing,
        frame,
        frame_round,
        background,
    );
    e.runs = runs;
    app.ui.texteingabe = Some(e);
    app.mitteilung(jjcad_core::i18n::tr("gui.textedit.text_inhalt_aendern_esc_uebernimmt"));
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

/// Klick in die Zeichenfläche, aber außerhalb des Eingabefelds?
///
/// Der Klick muss **in** der Zeichenfläche liegen: ein Klick ins
/// Eigenschaften-Panel (Höhe, Ausrichtung) oder in den Ribbon beendet die
/// Eingabe nicht, sonst ließe sich während des Tippens nichts einstellen.
pub(crate) fn klick_neben_dem_text(
    ctx: &egui::Context,
    flaeche: Rect,
    feld: Option<Rect>,
) -> bool {
    let (gedrueckt, pos) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.interact_pos()));
    if !gedrueckt {
        return false;
    }
    let Some(p) = pos else { return false };
    if !flaeche.contains(p) {
        return false;
    }
    !feld.map(|r| r.contains(p)).unwrap_or(true)
}

/// Rechtsklick in der Zeichenfläche?
fn rechtsklick_in_der_flaeche(ctx: &egui::Context, flaeche: Rect) -> bool {
    let (gedrueckt, pos) = ctx.input(|i| {
        (
            i.pointer.secondary_pressed(),
            i.pointer.interact_pos(),
        )
    });
    gedrueckt && pos.map(|p| flaeche.contains(p)).unwrap_or(false)
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
        app.mitteilung(jjcad_core::i18n::tr("gui.textedit.text_abgebrochen"));
        return;
    }
    let unit = app.unit();
    // Auszeichnung (Fett/Kursiv/…) bleibt beim Ändern an den Zeichen hängen.
    let runs = jjcad_core::text::retarget_runs(
        &t.text,
        &t.runs,
        &text,
        jjcad_core::text::RunStyle::default(),
    );
    let runs_json = serde_json::to_value(&runs).unwrap_or(json!([]));
    let farbe = t.farbe.to_hex();
    let (action, params, meldung) = match t.id {
        Some(id) => (
            "modify.set",
            json!({"ids": [id], "text": text, "height": unit.from_mm(t.height),
                   "angle": t.angle.to_degrees(),
                   "align": t.align.key(), "valign": t.valign.key(),
                   "font": t.font.key(),
                   "line_spacing": t.line_spacing,
                   "frame": t.frame,
                   "frame_round": t.frame_round,
                   // "none" nimmt eine vorhandene Fläche wieder weg.
                   "background": match t.background {
                       Some(c) => json!(c.to_hex()),
                       None => json!("none"),
                   },
                   "runs": runs_json}),
            jjcad_core::i18n::tr("gui.textedit.text_geaendert"),
        ),
        None => {
            let mut params = json!({"p": [unit.from_mm(t.pos.x), unit.from_mm(t.pos.y)],
                       "text": text,
                       "height": unit.from_mm(t.height),
                       "angle": t.angle.to_degrees(),
                       "align": t.align.key(),
                       "valign": t.valign.key(),
                       "bold": app.options.text_bold,
                       "italic": app.options.text_italic,
                       "underline": app.options.text_underline,
                       "superscript": app.options.text_superscript,
                       "font": t.font.key(),
                       "line_spacing": t.line_spacing,
                       "frame": t.frame,
                       "frame_round": t.frame_round,
                       "runs": runs_json.clone()});
            // Ebene wie bei jedem Werkzeug (T0202): neuer Text gehört auf
            // „Beschriftung“ (seine Standardebene), außer eine eigene Ebene ist
            // im Layer-Manager gewählt.
            let ebene = app
                .ui
                .pending
                .layer
                .filter(|l| app.doc().layer(*l).is_some())
                .or_else(|| app.werkzeug_ebene("draw.text"));
            if let Some(ebene) = ebene {
                params["layer"] = json!(ebene);
            }
            // Nur wenn eine Farbe gewählt wurde — sonst gilt die Layerfarbe.
            if let Some(c) = app.options.text_color {
                params["color"] = json!(c.to_hex());
            }
            // Hintergrundfläche der Umrandung (T0126).
            if let Some(c) = t.background {
                params["background"] = json!(c.to_hex());
            }
            ("draw.text", params, "Text · gesetzt")
        }
    };
    match app.session_mut().execute(action, &params) {
        Ok(_) => {
            app.mark_changed();
            app.mitteilung(meldung);
            // Zuletzt gesetzte Werte bleiben für das nächste Element stehen —
            // aber **nicht** der Inhalt: ein neues Textfeld beginnt leer, und
            // die Vorschau zeigt immer das Beispielwort (T0066).
            app.options.text_height = t.height;
            app.options.text_line_spacing = t.line_spacing;
        }
        Err(e) => app.toast(e),
    }
}

/// Laufende Eingabe abschließen, bevor der Reiter wechselt (T0073).
///
/// Getippter Text gehört in die Datei, in der er begonnen wurde — der Wechsel
/// darf ihn nicht in den neuen Reiter schreiben und auch nicht still verwerfen.
pub(crate) fn beenden(app: &mut App) {
    if app.ui.texteingabe.is_some() {
        let ctx = app.ctx.clone();
        uebernehmen(app, &ctx);
    }
}

/// Übernimmt die laufende Eingabe — derselbe Weg wie Esc (nur für Tests).
#[cfg(test)]
pub(crate) fn uebernehmen_fuer_test(app: &mut App) {
    let ctx = app.ctx.clone();
    uebernehmen(app, &ctx);
}

/// Abbruch: der vorhandene Text bleibt unverändert, ein neuer entsteht nicht.
fn abbrechen(app: &mut App, ctx: &egui::Context) {
    if app.ui.texteingabe.take().is_some() {
        ctx.memory_mut(|m| m.stop_text_input());
        app.mitteilung(jjcad_core::i18n::tr("gui.textedit.text_abgebrochen"));
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
    // Rechtsklick bricht ab — aber nur in der Zeichenfläche: im
    // Eigenschaften-Panel (Höhe, Ausrichtung) hat er nichts mit dem Text zu tun.
    if rechtsklick_in_der_flaeche(&ctx, rect) {
        abbrechen(app, &ctx);
        return;
    }

    let mut t = match app.ui.texteingabe.take() {
        Some(t) => t,
        None => return,
    };

    // Genau die Schrift der Zeichnung (`canvas::paint_text`): Höhe × Zoom × 1,32,
    // gerastert höchstens in der Deckelgröße und auf die Größenleiter gelegt
    // (T0114). Den Rest holt die Skalierung des ganzen Feldes — sonst rastet
    // egui beim Tippen im hohen Zoom jede Größe in den gemeinsamen
    // Schriftatlas und der Text friert ein.
    let gewuenscht = (t.height * view.zoom * render::TEXT_GROESSE_FAKTOR).max(3.0) as f32;
    let (size, faktor) = crate::textgroesse::raster_und_skalierung(
        gewuenscht,
        app.prefs.textaufloesung.px(),
    );
    // Die Schrift des Textes auch für das Feld: egui nimmt sie für die
    // Zeilenhöhe der leeren Zeile (Cursorstand). Mit der Ersatzschrift stand
    // der Cursor eines leeren Textes neben dem Anker (T0110).
    let font = FontId::new(size, crate::canvas::schrift_familie(&ctx, t.font, false));
    // Zeilenhöhe wie Rahmen und Zeichnung: die der Schriftfamilie mal
    // Zeilenabstand. Vorher kam sie von `row_height` der Ersatzschrift — das
    // traf nur, solange die Schrift zufällig dieselbe Zeilenhöhe hatte (T0110).
    let lh_em = jjcad_core::fonts::line_height_em(t.font, jjcad_core::fonts::FontStyle::Regular) as f32;
    // Höhe der leeren Zeile im Feld: egui nimmt für das Feld die Zeilenhöhe der
    // Schrift, mindestens unseren Zeilenabstand — sonst säße der Cursor eines
    // leeren Textes neben dem Anker.
    let zeilenhoehe = ctx
        .fonts(|f| f.row_height(&font))
        .max(size * lh_em * t.line_spacing.clamp(0.1, 10.0) as f32);
    let anker = world_to_screen(rect, view, t.pos);
    let farbe = color32(t.farbe);

    // Stand vor diesem Frame: daraus wird abgeleitet, welche Zeichen neu sind
    // (sie bekommen die vorgemerkte Auszeichnung).
    let text_vorher = t.text.clone();
    let runs_vorher = t.runs.clone();
    let pending = t.pending;
    let text_fuer_setzer = std::sync::Arc::new(text_vorher.clone());
    let runs_fuer_setzer = std::sync::Arc::new(runs_vorher.clone());
    // Breite aus der echten Schriftbreite: der Text darf beim Schreiben **nicht**
    // umbrechen — das tut der eingefügte Text auch nicht, sonst springt er beim
    // Übernehmen in eine andere Form.
    let (textbreite, texthoehe) = {
        let runs = jjcad_core::text::edit_runs(
            &text_vorher,
            &runs_vorher,
            &t.text,
            jjcad_core::text::RunStyle::default(),
            pending,
        );
        let runs = jjcad_core::text::runs_for(&t.text, &runs, jjcad_core::text::RunStyle::default());
        let mut job = crate::canvas::auszeichnungs_job(&ctx, &runs, size, farbe, t.font);
        job.wrap.max_width = f32::INFINITY;
        crate::canvas::zeilenhoehe_setzen(&mut job, size, lh_em, t.line_spacing);
        ctx.fonts(|f| {
            let g = f.layout_job(job);
            (g.size().x, g.size().y)
        })
    };
    let breite = (textbreite + size * 1.5).max(if t.text.is_empty() { 20.0 } else { 8.0 });
    // Eigener Setzer: ohne Umbruch, nur echte Zeilenumbrüche (Enter).
    //
    // **Im Feld wird immer linksbündig gesetzt**, auch wenn der Text später
    // zentriert oder rechtsbündig steht: `LayoutJob::halign` legt die Zeilen um
    // den Ursprung (zentriert ab −Breite/2), das Feld beginnt aber am Ursprung —
    // die Zeilen ragten damit zur Hälfte (bzw. ganz) links aus dem Feld und
    // wurden abgeschnitten („abcdefg" zeigte nur „defg", T0028 Turn 2). Die
    // Lage übernimmt stattdessen das Feld selbst (`feld_lage`).
    // Der Setzer bekommt den **gerade** zu setzenden Text: die Läufe werden
    // für genau diesen Text abgeleitet, sonst läge die Galley einen Frame hinter
    // dem Feld und der Cursor spränge an den Anfang.
    let mut setzer = move |ui: &egui::Ui, text_jetzt: &str, _wrap: f32| {
        let runs = jjcad_core::text::edit_runs(
            &text_fuer_setzer,
            &runs_fuer_setzer,
            text_jetzt,
            jjcad_core::text::RunStyle::default(),
            pending,
        );
        let runs = jjcad_core::text::runs_for(
            text_jetzt,
            &runs,
            jjcad_core::text::RunStyle::default(),
        );
        let mut job = crate::canvas::auszeichnungs_job(ui.ctx(), &runs, size, farbe, t.font);
        job.wrap.max_width = f32::INFINITY;
        crate::canvas::zeilenhoehe_setzen(&mut job, size, lh_em, t.line_spacing);
        ui.fonts(|f| f.layout_job(job))
    };
    // Höhe des gezeichneten Textblocks — **nicht** die Zeilenhöhe der Schrift
    // (`row_height`, rund 1,36 × Schriftgröße): die Anzeige legt den Text über
    // die Höhe der Galley an den Anker (`canvas::text_ort`), und die ist bei
    // einer Zeile kleiner (rund 1,15 ×). Mit der Zeilenhöhe stand eine
    // einzeilige Bearbeitung deshalb sichtbar über dem Basispunkt, ein
    // mehrzeiliger Text traf (T0044).
    // Nur der **leere** Text hat keine Galley-Höhe (0): dort braucht der
    // Cursor eine ganze Zeile, sonst stünde er unter dem Anker.
    let block_h = if t.text.is_empty() {
        zeilenhoehe
    } else {
        texthoehe
    };
    let lage = feld_lage(anker, t.align, t.valign, textbreite, block_h);

    // Das Feld wird um den Anker gestreckt (T0114): gerastert ist es in der
    // Leiterstufe, angezeigt in voller Bildschirmgröße. `TSTransform` bildet
    // `p` auf `p · f + t` ab; mit `t = Anker · (1 − f)` bleibt der Anker stehen
    // und die Lage aus `feld_lage` gilt unverändert (nur eben gestreckt).
    let transform = if (faktor - 1.0).abs() < 1e-3 {
        egui::emath::TSTransform::IDENTITY
    } else {
        egui::emath::TSTransform::new(anker.to_vec2() * (1.0 - faktor), faktor)
    };

    egui::Area::new(Id::new("texteingabe"))
        .order(egui::Order::Foreground)
        // Die Ausrichtung gilt schon beim Schreiben: „Zentriert" wächst nach
        // beiden Seiten, „Unten" steht über dem Anker.
        .fixed_pos(lage)
        .show(&ctx, |ui| {
            // Die Eingabe gehört in die Zeichenfläche, nicht über die
            // Oberfläche: das Feld liegt im Vordergrund über dem ganzen
            // Fenster, ohne Beschnitt hing ein großer Text über Ribbon,
            // Ebenen-Panel und Eigenschaften (T0047). Die Zeichnung selbst
            // endet an derselben Kante — nach dem Übernehmen sieht der Text
            // also genauso aus.
            // Der Beschnitt wird von der Ebenen-Transformation mitgestreckt;
            // deshalb die Zeichenfläche vorher in die unskalierte Ebene
            // zurückrechnen — sonst hinge der Text über Ribbon und Panels.
            let beschnitt = if transform == egui::emath::TSTransform::IDENTITY {
                rect
            } else {
                transform.inverse() * rect
            };
            ui.set_clip_rect(beschnitt);
            // Keine Fläche, kein Rahmen: der Text steht frei in der Zeichnung
            // und sieht während der Eingabe genauso aus wie danach.
            ui.visuals_mut().text_cursor.stroke = Stroke::new(1.5_f32, Palette::accent());
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
            // Die Fläche für „Klick daneben" in Bildschirmkoordinaten: der
            // Zeiger kommt unskaliert, das Feld ist gestreckt (T0114).
            t.flaeche = Some(transform * resp.rect.expand(2.0));
            // Hinweis nur solange nichts getippt ist: beim Schreiben soll nur
            // der Text dastehen. Er steht unter dem Text, nicht dahinter.
            if t.text.is_empty() {
                ui.label(
                    egui::RichText::new(jjcad_core::i18n::tr("gui.textedit.esc_uebernimmt_hinweis"))
                        .size(10.0)
                        .color(Palette::text_faint()),
                );
            }
        });

    // Die Ebene des Feldes strecken (T0114): gerastert in der Leiterstufe,
    // angezeigt in voller Größe. egui rechnet Zeigerpositionen über die
    // Transformation zurück — Klicks und Cursor sitzen damit richtig.
    ctx.set_transform_layer(
        egui::LayerId::new(egui::Order::Foreground, Id::new("texteingabe")),
        transform,
    );

    // Eingefügte Zeichen tragen jetzt die vorgemerkte Auszeichnung.
    if t.text != text_vorher {
        t.runs = jjcad_core::text::edit_runs(
            &text_vorher,
            &runs_vorher,
            &t.text,
            jjcad_core::text::RunStyle::default(),
            pending,
        );
        t.letzter_text = t.text.clone();
    }

    // Klick in der Zeichenfläche neben den Text übernimmt (wie in
    // Office-Programmen). Ein Klick **außerhalb** der Zeichenfläche — etwa in
    // das Eigenschaften-Panel, um Höhe oder Ausrichtung zu ändern — lässt die
    // Eingabe offen (T0028).
    let aussen_geklickt = klick_neben_dem_text(&ctx, rect, t.flaeche);

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

    /// Farbe und Auszeichnung aus dem Panel landen im erzeugten Text (T0032).
    #[test]
    fn neue_texte_uebernehmen_farbe_und_auszeichnung() {
        let mut app = app_mm();
        app.options.text_color = Some(Color::RED);
        app.options.text_bold = true;
        app.options.text_underline = true;
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
        }
        uebernehmen_fuer_test(&mut app);
        let e = app.doc().ents().first().expect("Text erzeugt");
        assert_eq!(e.style.color, Some(Color::RED), "Farbe fehlt");
        let EntityKind::Text {
            bold, underline, ..
        } = &e.kind
        else {
            panic!("Text erwartet");
        };
        assert!(*bold && *underline, "Auszeichnung fehlt");
    }

    /// Name der Ebene, auf der der erste Text liegt.
    fn text_ebene(app: &App) -> String {
        let e = app.doc().ents().first().expect("Text fehlt");
        app.doc()
            .layer(e.layer)
            .map(|l| l.name.clone())
            .expect("Ebene fehlt")
    }

    /// T0202: Ein neuer Text gehört auf „Beschriftung“ — die zuletzt
    /// angeklickte Ebene („Zeichnung“) darf ihn nicht einsammeln.
    #[test]
    fn neuer_text_landet_auf_beschriftung() {
        let mut app = app_mm();
        assert_eq!(
            app.doc().layer(app.doc().current_layer).map(|l| l.name.as_str()),
            Some("Zeichnung"),
            "Ausgangslage: aktuelle Ebene ist „Zeichnung“"
        );
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
        }
        uebernehmen_fuer_test(&mut app);
        assert_eq!(text_ebene(&app), "Beschriftung");
    }

    /// Eine **eigene** Ebene sticht die Standardebene weiterhin (T0039).
    #[test]
    fn eigene_ebene_sticht_beschriftung() {
        let mut app = app_mm();
        let ziel = app
            .doc_mut()
            .ensure_layer("Bauantrag", jjcad_core::style::Color::BLACK);
        app.doc_mut().current_layer = ziel;
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
        }
        uebernehmen_fuer_test(&mut app);
        assert_eq!(text_ebene(&app), "Bauantrag");
    }

    /// Die Ebene aus dem Eigenschaften-Panel sticht auch beim Text (T0202).
    #[test]
    fn panel_wahl_sticht_die_beschriftung() {
        let mut app = app_mm();
        let ziel = app.doc().layer_id_by_name("Architektur").expect("Ebene");
        app.ui.pending.layer = Some(ziel);
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
        }
        uebernehmen_fuer_test(&mut app);
        assert_eq!(text_ebene(&app), "Architektur");
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

    /// Vorgemerkte Auszeichnung gilt für das nächste Getippte — nicht für den
/// vorhandenen Text (T0033).
#[test]
fn vorgemerkte_auszeichnung_gilt_fuer_neue_zeichen() {
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
    let mut schritt = |app: &mut App, events: Vec<egui::Event>| {
        let raw = egui::RawInput {
            screen_rect: Some(flaeche),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(app, ui, flaeche, view);
            });
        });
    };
    schritt(&mut app, Vec::new());
    schritt(&mut app, vec![egui::Event::Text("Raum ".into())]);
    // Das Panel merkt Fett für das nächste Zeichen vor.
    if let Some(t) = app.ui.texteingabe.as_mut() {
        t.pending = Some(jjcad_core::text::RunStyle {
            bold: true,
            ..Default::default()
        });
    }
    schritt(&mut app, vec![egui::Event::Text("Wohnen".into())]);
    schritt(
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    let e = app.doc().ents().first().expect("Text erzeugt");
    let EntityKind::Text { text, runs, .. } = &e.kind else {
        panic!("Text erwartet");
    };
    assert_eq!(text, "Raum Wohnen");
    let muss: Vec<(String, bool)> = runs.iter().map(|r| (r.text.clone(), r.bold)).collect();
    assert_eq!(
        muss,
        vec![("Raum ".to_string(), false), ("Wohnen".to_string(), true)],
        "nur das Nachgetippte wird fett: {runs:?}"
    );
}

/// Die im Eigenschaften-Panel eingestellte Drehung gilt für den neuen Text —
/// das Eingabefeld bleibt beim Tippen bildschirmparallel (wie in einem
/// Textprogramm), das Ergebnis ist gedreht.
#[test]
fn drehung_aus_dem_panel_gilt_fuer_den_neuen_text() {
    let ctx = egui::Context::default();
    let mut app = App::with_ctx(ctx.clone());
    app.session_mut()
        .execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Einheit setzen");
    app.options.text_height = 250.0;
    app.options.text_angle = 30.0;
    start(&mut app, V2::new(1000.0, 500.0));

    let flaeche = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let view = View {
        center: V2::new(1000.0, 500.0),
        zoom: 0.2,
        rot: 0.0,
    };
    let mut schritt = |app: &mut App, events: Vec<egui::Event>| {
        let raw = egui::RawInput {
            screen_rect: Some(flaeche),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(app, ui, flaeche, view);
            });
        });
    };
    schritt(&mut app, Vec::new());
    schritt(&mut app, vec![egui::Event::Text("Gedreht".into())]);
    schritt(
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    let e = app.doc().ents().first().expect("Text erzeugt");
    let EntityKind::Text { angle, text, .. } = &e.kind else {
        panic!("Text erwartet");
    };
    assert_eq!(text, "Gedreht");
    assert!(
        (angle.to_degrees() - 30.0).abs() < 1e-6,
        "Drehung nicht übernommen: {}°",
        angle.to_degrees()
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

#[cfg(test)]
mod tests_klick_neben_dem_text {
    //! Während der Eingabe bleibt das Eigenschaften-Panel bedienbar (T0028):
    //! ein Klick dort beendet den Text nicht, ein Klick in die Zeichenfläche
    //! daneben schon.

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
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 950.0))
    }

    fn view() -> View {
        View {
            center: V2::new(1000.0, 500.0),
            zoom: 0.2,
            rot: 0.0,
        }
    }

    /// Ein Frame mit den angegebenen Ereignissen.
    fn frame(app: &mut App, ctx: &egui::Context, ereignisse: Vec<egui::Event>, zeit: f64) {
        let raw = egui::RawInput {
            screen_rect: Some(flaeche()),
            time: Some(zeit),
            predicted_dt: 0.016,
            events: ereignisse,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(app, ui, flaeche(), view());
            });
        });
    }

    fn klick(x: f32, y: f32) -> Vec<egui::Event> {
        let pos = Pos2::new(x, y);
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    #[test]
    fn klick_ins_panel_beendet_die_eingabe_nicht() {
        let ctx = egui::Context::default();
        let mut app = app_mm();
        start(&mut app, V2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "Wohnen".to_string();
        }
        frame(&mut app, &ctx, Vec::new(), 1.0);

        // Rechts neben der Zeichenfläche liegt das Eigenschaften-Panel.
        frame(&mut app, &ctx, klick(1150.0, 300.0), 1.1);
        assert!(
            app.ui.texteingabe.is_some(),
            "ein Klick ins Eigenschaften-Panel darf die Eingabe nicht beenden"
        );
        assert!(app.doc().ents().is_empty(), "noch kein Text angelegt");

        // Klick in die Zeichenfläche, weit weg vom Text: übernimmt.
        frame(&mut app, &ctx, klick(900.0, 850.0), 1.2);
        assert!(
            app.ui.texteingabe.is_none(),
            "ein Klick daneben in der Zeichenfläche übernimmt"
        );
        assert_eq!(app.doc().ents().len(), 1, "der Text steht in der Zeichnung");
    }
}

#[cfg(test)]
mod tests_lage_beim_bearbeiten {
    //! Der bearbeitete Text steht **pixelgleich** dort, wo die Zeichnung ihn
    //! ablegt (T0044).
    //!
    //! Gemeldet: „so lange ein Textfeld editiert wird, bewegt es sich leicht
    //! nach oben", nur bei **einzeiligen** Texten. Ursache war die Höhe des
    //! Textblocks: das Feld hing an der Zeilenhöhe der Schrift
    //! (`row_height`, hier 1,36 × Schriftgröße), die Anzeige aber an der Höhe
    //! der gesetzten Galley (1,15 ×). Bei einer Zeile traf beides nicht
    //! zusammen (13,9 px bei 66 px Schrift), bei zwei Zeilen überwog die
    //! Galley und der Fehler blieb unsichtbar.
    //!
    //! Geprüft wird gegen die **gemalte** Schrift der Zeichenfläche (beide
    //! entstehen aus `Shape::Text`, gemessen wird der Kasten der Galley), nicht
    //! gegen eine im Test nachgebaute Lage.

    use super::*;
    use crate::app::App;

    fn flaeche() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))
    }

    /// Kästen aller in einem Frame gemalten Texte (galley.rect + Position).
    fn gemalt(app: &mut App, ctx: &egui::Context, zeit: f64) -> Vec<(String, Rect)> {
        let raw = egui::RawInput {
            screen_rect: Some(flaeche()),
            time: Some(zeit),
            predicted_dt: 0.016,
            ..Default::default()
        };
        let out = ctx.run(raw, |c| app.ui(c));
        let mut gefunden = Vec::new();
        for cs in &out.shapes {
            if let egui::Shape::Text(t) = &cs.shape {
                gefunden.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                ));
            }
        }
        gefunden
    }

    fn kasten(von: &[(String, Rect)], text: &str) -> Rect {
        von.iter()
            .find(|(t, _)| t == text)
            .unwrap_or_else(|| panic!("{text:?} wurde nicht gezeichnet"))
            .1
    }

    /// Ein Frame Zeichnung, dann ein Frame Bearbeitung: der Kasten des
    /// bearbeiteten Textes muss auf dem der gezeichneten Schrift liegen.
    fn pruefe(text: &str, align: TextAlignH, valign: TextAlignV) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        let mitte = app.current_view().center;
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [mitte.x, mitte.y], "text": text, "height": 250.0,
                        "align": align.key(), "valign": valign.key()}),
            )
            .expect("Text anlegen");
        let id = app.doc().ents()[0].id;

        // Zwei Frames: der erste setzt Ansicht und Zwischenspeicher.
        gemalt(&mut app, &ctx, 1.0);
        let gezeichnet = kasten(&gemalt(&mut app, &ctx, 1.1), text);

        assert!(start_entitaet(&mut app, id), "Bearbeiten startet");
        gemalt(&mut app, &ctx, 1.2);
        let bearbeitet = kasten(&gemalt(&mut app, &ctx, 1.3), text);

        let name = format!("{text:?} {}/{}", align.label(), valign.label());
        // Toleranz 1,5 px statt 0,6: das Feld setzt **linksbündig** (T0028,
        // Turn 2), die Zeichnung nach `halign`. egui rundet jede Zeichenposition
        // auf ganze Bildpunkte; bei verschiedenem Zeilenursprung weichen die
        // Breiten deshalb um bis zu einen Bildpunkt ab, und die Streckung der
        // Rastergröße (T0114) vervielfacht diesen einen Punkt. Ein echter
        // Versatz (der gemeldete Fehler) lag bei einer halben Zeilenhöhe.
        assert!(
            (bearbeitet.left() - gezeichnet.left()).abs() < 1.5,
            "{name}: waagerecht {:.2} px daneben (bearbeitet {:?}, gezeichnet {:?})",
            bearbeitet.left() - gezeichnet.left(),
            bearbeitet,
            gezeichnet
        );
        assert!(
            (bearbeitet.top() - gezeichnet.top()).abs() < 1.5,
            "{name}: senkrecht {:.2} px daneben (bearbeitet {:?}, gezeichnet {:?})",
            bearbeitet.top() - gezeichnet.top(),
            bearbeitet,
            gezeichnet
        );
    }

    /// Einzeilig — der gemeldete Fall: der Text stand beim Bearbeiten oben.
    #[test]
    fn einzeilig_steht_wie_gezeichnet() {
        for align in TextAlignH::ALL {
            for valign in TextAlignV::ALL {
                pruefe("Pruefwort", align, valign);
            }
        }
    }

    /// Mehrzeilig — traf schon vor dem Fix, darf nicht zurückfallen.
    #[test]
    fn mehrzeilig_steht_wie_gezeichnet() {
        for align in TextAlignH::ALL {
            for valign in TextAlignV::ALL {
                pruefe("Pruefwort\nZeile2", align, valign);
            }
        }
    }
}

#[cfg(test)]
mod tests_neuer_text_leer {
    //! Ein **leerer** neuer Text hat keine Galley-Höhe; der Cursor braucht
    //! trotzdem eine ganze Zeile. Sonst stünde er beim ersten Tippen unter dem
    //! Basispunkt (Gegenprobe zu T0044: dort ist nicht die Galley maßgeblich,
    //! sondern eine Zeile).

    use super::*;
    use crate::app::App;

    #[test]
    fn cursor_steht_auf_dem_basispunkt() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.options.text_height = 250.0;
        app.options.text_valign = TextAlignV::Bottom;
        let mitte = app.current_view().center;
        let punkt = jjcad_core::geom::Vec2::new(mitte.x, mitte.y);
        start(&mut app, punkt);

        let flaeche = Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0));
        let mut frame = |app: &mut App, zeit: f64| {
            let raw = egui::RawInput {
                screen_rect: Some(flaeche),
                time: Some(zeit),
                predicted_dt: 0.016,
                ..Default::default()
            };
            let _ = ctx.run(raw, |c| app.ui(c));
        };
        frame(&mut app, 1.0);
        frame(&mut app, 1.1);

        let anker = world_to_screen(app.ui.last_canvas, app.current_view(), punkt);
        let feld = app
            .ui
            .texteingabe
            .as_ref()
            .and_then(|t| t.flaeche)
            .expect("Feld liegt im Bild");
        // `flaeche` ist um 2 px erweitert (Trefferfläche für „daneben klicken").
        assert!(
            ((feld.bottom() - 2.0) - anker.y).abs() < 1.5,
            "Feldunterkante {:.1}, Basispunkt {:.1}",
            feld.bottom() - 2.0,
            anker.y
        );
    }
}

#[cfg(test)]
mod tests_beschnitt {
    //! Die Eingabe bleibt in der Zeichenfläche (T0047).
    //!
    //! Vorher hing ein großer Text während der Bearbeitung über Ribbon,
    //! Ebenen-Panel und Eigenschaften: das `egui::Area` der Eingabe liegt über
    //! der ganzen Oberfläche und war nicht beschnitten.

    use super::*;
    use crate::app::App;

    #[test]
    fn eingabe_ragt_nicht_ueber_die_oberflaeche() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        // 2,50 m Schrifthöhe: der Text ist breiter als die Zeichenfläche.
        app.options.text_height = 2500.0;
        let fenster = Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0));
        // Zeichenfläche wie im Bild: links das Ebenen-Panel, rechts die
        // Eigenschaften, oben das Ribbon.
        let flaeche = Rect::from_min_max(Pos2::new(143.0, 100.0), Pos2::new(1100.0, 860.0));
        let view = View {
            center: V2::new(1000.0, 500.0),
            zoom: 0.3,
            rot: 0.0,
        };
        start(&mut app, V2::new(1000.0, 500.0));
        if let Some(t) = app.ui.texteingabe.as_mut() {
            t.text = "TEXTFELD".to_string();
        }

        let mut klipps = Vec::new();
        for zeit in [1.0_f64, 1.1] {
            let raw = egui::RawInput {
                screen_rect: Some(fenster),
                time: Some(zeit),
                predicted_dt: 0.016,
                ..Default::default()
            };
            let out = ctx.run(raw, |c| {
                egui::CentralPanel::default().show(c, |ui| show(&mut app, ui, flaeche, view));
            });
            klipps.clear();
            for cs in &out.shapes {
                if matches!(&cs.shape, egui::Shape::Text(_)) {
                    klipps.push((cs.clip_rect, cs.shape.visual_bounding_rect()));
                }
            }
        }

        assert!(!klipps.is_empty(), "Eingabetext nicht gezeichnet");
        for (clip, kasten) in klipps {
            assert!(
                flaeche.contains_rect(clip),
                "Beschnitt {:?} ragt aus der Zeichenfläche {:?}",
                clip,
                flaeche
            );
            assert!(
                kasten.intersects(flaeche),
                "Text liegt außerhalb der Zeichenfläche: {:?}",
                kasten
            );
        }
    }
}
