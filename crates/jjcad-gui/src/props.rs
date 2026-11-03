//! Eigenschaften-Panel — **ein** Satz Zeilen für Erzeugen und Bearbeiten.
//!
//! Vorher gab es je Objektart zwei bis vier unabhängig geschriebene Maler
//! (`pending_props`, `entity_props` je Typ, `multi_props`), die dieselben
//! Eigenschaften in unterschiedlicher Reihenfolge, mit unterschiedlichen
//! Beschriftungen und mit fehlenden Zeilen zeigten. Jedes neue Objekt kam
//! dadurch wieder anders heraus.
//!
//! Hier zeichnet **eine** Funktion je Objektart alle Zeilen; nur die Herkunft
//! der Werte und das Ziel der Änderung unterscheiden sich:
//!
//! * [`Ziel::Neu`] — Werte aus [`crate::app::ToolOptions`], Änderungen gehen
//!   zurück in die Vorgaben und bleiben für das nächste Element stehen.
//! * [`Ziel::Auswahl`] — Werte aus dem Objekt, Änderungen gehen als
//!   `modify.set` an genau diese Auswahl.
//!
//! Damit ist „beim Erzeugen merken, beim Bearbeiten nur die Auswahl ändern"
//! **eine** Verzweigung an der Anwendungsstelle — nicht ein zweites Panel.
//!
//! **Alle** Objektarten laufen hier — `panels` zeichnet kein eigenes Panel
//! mehr. `tests::jede_objektart_hat_ein_panel` zählt jeden `EntityKind`-Zweig
//! auf und `tests::alle_erzeuger_zeigen_dieselben_zeilen_wie_das_objekt`
//! vergleicht Werkzeug und Objekt Zeile für Zeile.
//!
//! # Für Ticketworker: eine neue Objektart eintragen
//!
//! 1. **Zeichnen**: eine Funktion `pub fn <art>(app, ui, ziel)` in dieser Datei
//!    — oder, wenn die Maße aus den Klicks entstehen, ein Zweig in
//!    [`Einfach`] (dazu `aus_werkzeug`, `aus_objekt`, `titel`, `icon` und die
//!    eigenen Zeilen in [`einfach`]).
//! 2. **Werte holen**: `match ziel` — `Ziel::Neu` liest aus
//!    [`crate::app::ToolOptions`], `Ziel::Auswahl(id)` aus dem Objekt.
//!    Niemals zwei getrennte Zeichnungen für die beiden Fälle.
//! 3. **Anwenden**: `Ziel::Neu` schreibt in die Vorgaben (bleibt für das
//!    nächste Element stehen), `Ziel::Auswahl` schickt **ein** `modify.set`.
//! 4. **Zeilen anmelden**: jede Zeile ruft `zeile("schlüssel")`. Zeilen, die
//!    beim Erzeugen sinnlos sind (Reihenfolge, Inhalt, Kennzahlen), werden
//!    **gezeigt, aber gesperrt** — nie weglassen.
//! 5. **Eintragen**: Werkzeug in `panels::properties` (Erzeugen) und die Art in
//!    `Einfach::aus_objekt` bzw. im `match` (Auswahl). Beides fehlt = Test rot.
//! 6. **Test**: die Art in `tests::alle_erzeuger_zeigen_dieselben_zeilen_wie_das_objekt`
//!    aufnehmen. Ohne diesen Eintrag ist die Gleichheit nicht belegt.
//!
//! # Abweichungen, die bleiben dürfen
//!
//! Drei Zeilen sind im Erzeugmodus sinnlos, weil das Objekt noch nicht
//! existiert: `Reihenfolge`, `Inhalt` (Text wird in der Zeichenfläche getippt)
//! und die Kennzahlen (Länge, Fläche, Volumen). Sie werden **gezeigt, aber
//! deaktiviert** — nicht weggelassen. Weglassen ist genau das, was das Panel
//! springen ließ.
//!
//! # Wächter
//!
//! [`protokoll`] hält fest, welche Zeilen im letzten Bild gezeichnet wurden.
//! Die Tests in [`mod tests`] vergleichen das Protokoll von `Neu` und
//! `Auswahl` — eine Zeile, die nur an einer Stelle ergänzt wird, fällt dort
//! sofort auf. `crate::props::tests::jede_objektart_ist_eingetragen` erzwingt
//! außerdem, dass eine neue Objektart nicht ohne Panel-Eintrag dazukommt.

use egui::Ui;
use jjcad_core::model::{
    DimAxis, DoorBase, DoorKind, Entity, EntityKind, Id, TextFont, TextRun,
};
use jjcad_core::style::Color;
use jjcad_core::text::RunStyle;

use crate::app::App;
use crate::panels::{self, section_title};
use crate::theme;
use crate::widgets;

/// Wohin das Panel schreibt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ziel {
    /// Werkzeug läuft: Werte sind Vorgaben, Änderungen bleiben stehen.
    Neu,
    /// Objekt ausgewählt: Werte kommen aus dem Objekt, Änderungen nur dorthin.
    Auswahl(Id),
}

impl Ziel {
    pub fn ist_neu(self) -> bool {
        matches!(self, Ziel::Neu)
    }

    pub fn id(self) -> Option<Id> {
        match self {
            Ziel::Neu => None,
            Ziel::Auswahl(id) => Some(id),
        }
    }

    /// Eigene Widget-Kennung je Fall — sonst teilten sich Vorgaben und Objekt
    /// den Zustand eines Aufklappmenüs.
    fn salt(self, name: &str) -> String {
        match self {
            Ziel::Neu => format!("neu_{name}"),
            Ziel::Auswahl(_) => format!("sel_{name}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Protokoll der gezeichneten Zeilen (für Tests und Fehlersuche)
// ---------------------------------------------------------------------------

std::thread_local! {
    static PROTOKOLL: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Protokoll leeren — `panels::properties` ruft das zu Beginn jedes Bildes.
pub fn protokoll_start() {
    PROTOKOLL.with(|p| p.borrow_mut().clear());
    WERKZEUG_ZEILEN.with(|p| p.borrow_mut().clear());
}

/// Schlüssel der zuletzt gezeichneten Zeilen, in Reihenfolge.
///
/// Nur die Tests lesen das Protokoll; im laufenden Programm kostet das
/// Mitschreiben ein paar Pushs je Bild.
#[cfg_attr(not(test), allow(dead_code))]
pub fn protokoll() -> Vec<&'static str> {
    PROTOKOLL.with(|p| p.borrow().clone())
}

/// Eine Eigenschaftsänderung aus dem Panel ausführen — **der eine Weg** für
/// jedes `modify.set` aus dieser Datei.
///
/// Ist gerade eine Mehrfachauswahl im Panel (`App::mehrfach_ziel`), geht die
/// Änderung an **alle** Objekte dieser Gruppe, nicht nur an das Objekt, dessen
/// Werte angezeigt wurden. Genau dadurch gilt jede Eigenschaft, die ein
/// einzelnes Objekt hat — Dicke, Schraffur, Füllfarbe, Muster, Breite, Höhe,
/// Stufen, Schrift —, ohne eigenen Code auch für die Auswahl: die Zeichnung
/// der Zeilen gibt es nur einmal, nur das Ziel der Änderung ist mehrfach.
pub fn setzen(app: &mut App, params: &serde_json::Value) {
    let mut params = params.clone();
    let ids = app.mehrfach_ziel.clone();
    if ids.len() > 1 {
        if let Some(o) = params.as_object_mut() {
            o.insert("ids".into(), serde_json::json!(ids));
        }
    }
    let leer = params
        .get("ids")
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    if leer {
        return;
    }
    let _ = app.session_mut().execute("modify.set", &params);
    app.mark_changed();
}

/// Unterscheiden sich die Objekte der laufenden Mehrfachauswahl? Verglichen
/// wird das gespeicherte Objekt ohne Kennung (Art, Stil, Layer) — nur dann
/// sind die angezeigten Werte des Referenzobjekts für alle gültig.
pub fn mehrfach_abweichend(app: &App) -> bool {
    let kurz = |e: &Entity| format!("{:?}|{:?}|{}", e.kind, e.style, e.layer);
    let ids = &app.mehrfach_ziel;
    let Some(erste) = ids.first().and_then(|id| app.doc().entity(*id)) else {
        return false;
    };
    let muster = kurz(erste);
    ids.iter()
        .skip(1)
        .any(|id| app.doc().entity(*id).map(kurz).is_some_and(|k| k != muster))
}

/// Eine Zeile anmelden. Jede Zeile im Panel ruft das genau einmal.
fn zeile(key: &'static str) {
    PROTOKOLL.with(|p| p.borrow_mut().push(key));
}

std::thread_local! {
    /// Zeilen, die nur das **Werkzeug** betreffen (z. B. Einfügemodus) und
    /// keine Eigenschaft des Objekts sind. Sie zählen beim Zeilenvergleich
    /// nicht mit — sonst müsste eine Werkzeugeinstellung am fertigen Objekt
    /// stehen.
    static WERKZEUG_ZEILEN: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn zeile_nur_neu(key: &'static str) {
    WERKZEUG_ZEILEN.with(|p| p.borrow_mut().push(key));
}

/// Werkzeug-Zeilen des letzten Bildes (nur für Tests und Fehlersuche).
#[cfg_attr(not(test), allow(dead_code))]
pub fn protokoll_werkzeug() -> Vec<&'static str> {
    WERKZEUG_ZEILEN.with(|p| p.borrow().clone())
}

// ---------------------------------------------------------------------------
// Gemeinsame Zeilen: Kopf, Reihenfolge, Layer, Linie
// ---------------------------------------------------------------------------

/// Kopfzeile mit Symbol und Artname. Beim Erzeugen steht darunter, wofür die
/// Werte gelten; beim Bearbeiten Objektnummer und Layer.
pub fn kopf(app: &App, ui: &mut Ui, ziel: Ziel, titel: &str, icon: &str) {
    section_title(ui, icon, titel);
    match ziel {
        Ziel::Neu => {
            ui.label(theme::dim(jjcad_core::i18n::tr("gui.props.gilt_fuer_das_naechste_element")));
        }
        Ziel::Auswahl(id) => {
            if app.mehrfach_ziel.len() > 1 {
                // Mehrfachauswahl einer Art: das Referenzobjekt liefert die
                // Werte, geändert wird an allen.
                let n = app.mehrfach_ziel.len();
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_objekte_aenderung_gilt_fuer_alle", &[("n", format!("{}", n))])));
                if mehrfach_abweichend(app) {
                    ui.label(theme::faint(
                        jjcad_core::i18n::tr("gui.props.variiert_die_objekte_unterscheiden_sich_eine_aenderung_s"),
                    ));
                }
            } else {
                let layer_name = app
                    .doc()
                    .entity(id)
                    .map(|e| e.layer)
                    .and_then(|l| app.doc().layer(l).map(|l| l.name.clone()))
                    .unwrap_or_else(|| "—".to_string());
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_layer_x", &[("id", format!("{}", id)), ("layer_name", format!("{}", layer_name))])));
            }
        }
    }
    ui.add_space(6.0);
}

/// Zeichenreihenfolge. Beim Erzeugen gibt es noch kein Objekt — die Zeile
/// steht trotzdem da, nur ohne Wirkung.
pub fn reihenfolge(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    zeile("reihenfolge");
    let (pos, total) = match ziel {
        Ziel::Neu => (0usize, 0usize),
        Ziel::Auswahl(id) => {
            let pos = app
                .doc()
                .ents()
                .iter()
                .position(|x| x.id == id)
                .map(|i| i + 1)
                .unwrap_or(0);
            (pos, app.doc().ents().len())
        }
    };
    let mut wunsch = pos as i32;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.reihenfolge"), |ui| {
        match ziel {
            // Noch kein Objekt: kein Platz, also keine Zahl — die Zeile bleibt
            // trotzdem stehen, damit das Panel nicht springt.
            Ziel::Neu => {
                ui.add_enabled(false, egui::Label::new(theme::mono("—")));
                ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.nach_dem_platzieren")));
            }
            // Mehrfachauswahl: eine gemeinsame Position gibt es nicht.
            Ziel::Auswahl(_) if app.mehrfach_ziel.len() > 1 => {
                ui.add_enabled(false, egui::Label::new(theme::mono("—")));
                ui.label(theme::faint(jjcad_core::i18n::t_args_owned("gui.props.je_objekt_x_im_blatt", &[("total", format!("{}", total))])));
            }
            Ziel::Auswahl(_) => {
                ui.add(crate::widgets::zahl(&mut wunsch, 1.0, 1..=total.max(1) as i32));
                ui.label(theme::faint(jjcad_core::i18n::t_args_owned("gui.props.von_x", &[("total", format!("{}", total))])));
            }
        }
    });
    if let Ziel::Auswahl(id) = ziel {
        if wunsch > 0 && wunsch as usize != pos {
            let _ = app
                .session_mut()
                .execute("modify.order", &serde_json::json!({"ids": [id], "to": wunsch}));
            app.mark_changed();
        }
    }
}

/// Zeile für einen Wert, der erst am platzierten Objekt entsteht: kein
/// erfundenes „0,10 m“, sondern ein Strich und der Grund.
fn gesperrt_zeile(ui: &mut Ui, label: &str, hinweis: &str) {
    widgets::field_row(ui, label, |ui| {
        ui.label(theme::mono("—"));
        ui.label(theme::faint(hinweis));
    });
}

/// Angezeigte Linienstärke und Linientyp: am Objekt der tatsächliche Wert
/// (eigener oder aus dem Layer), beim Erzeugen die Vorgabe.
fn linienwerte(app: &App, e: &Option<Entity>) -> (f64, jjcad_core::style::LineType) {
    match e {
        Some(e) => (
            e.style
                .line_weight
                .unwrap_or_else(|| app.doc().effective_line_weight(e)),
            e.style
                .line_type
                .unwrap_or_else(|| app.doc().effective_line_type(e)),
        ),
        None => (
            app.ui.pending.lineweight.unwrap_or(0.35),
            app.ui
                .pending
                .line_type
                .unwrap_or(jjcad_core::style::LineType::Solid),
        ),
    }
}

/// Änderungen der gemeinsamen Zeilen — anwendbar auf Vorgaben oder Objekt.
#[derive(Default)]
pub struct Gemein {
    pub layer: Option<Id>,
    pub farbe: Option<Option<Color>>,
    pub staerke: Option<f64>,
    pub linientyp: Option<jjcad_core::style::LineType>,
}

/// Die drei Zeilen, die jedes Objekt hat: Layer, Farbe, Linie.
///
/// `mit_linie` ist für Text `false` — ein Text hat keine Linie; die Zeilen
/// Stärke und Linientyp erscheinen dann in **beiden** Fällen nicht.
#[allow(clippy::too_many_arguments)]
pub fn gemein(
    app: &App,
    ui: &mut Ui,
    ziel: Ziel,
    layer: Id,
    farbe: Option<Color>,
    farbe_gezeigt: Color,
    staerke: f64,
    linientyp: jjcad_core::style::LineType,
    mit_linie: bool,
    farbe_label: &str,
) -> Gemein {
    let mut out = Gemein::default();
    // Mehrfachauswahl: weichen die Werte ab, zeigen die Felder `*variiert*`
    // statt des Werts des Referenzobjekts.
    let viele: Vec<Id> = app.mehrfach_ziel.clone();
    let mehrfach = viele.len() > 1;
    let gleich_layer = !mehrfach
        || viele
            .iter()
            .all(|i| app.doc().entity(*i).map(|e| e.layer) == Some(layer));
    // Bezug des Vergleichs ist das **erste** Objekt der Gruppe, nicht der
    // angezeigte Vorgabewert: eine Füllfläche zeigt 0,35 mm, ihre wirksame
    // Stärke kommt aber aus der Ebene. Der Vergleich gegen den Vorgabewert
    // meldete „*variiert*“ für zwei gleiche Flächen (T0090).
    let referenz = viele.first().and_then(|i| app.doc().entity(*i));
    let ref_farbe = referenz.map(|e| app.doc().effective_color(e));
    let ref_staerke = referenz
        .map(|e| app.doc().effective_line_weight(e))
        .unwrap_or(staerke);
    let ref_typ = referenz
        .map(|e| app.doc().effective_line_type(e))
        .unwrap_or(linientyp);
    let gleich_farbe = !mehrfach
        || viele.iter().all(|i| {
            app.doc()
                .entity(*i)
                .map(|e| Some(app.doc().effective_color(e)) == ref_farbe)
                .unwrap_or(false)
        });
    let gleich_staerke = !mehrfach
        || viele.iter().all(|i| {
            app.doc()
                .entity(*i)
                .map(|e| (app.doc().effective_line_weight(e) - ref_staerke).abs() < 1e-6)
                .unwrap_or(false)
        });
    let gleich_typ = !mehrfach
        || viele.iter().all(|i| {
            app.doc()
                .entity(*i)
                .map(|e| app.doc().effective_line_type(e) == ref_typ)
                .unwrap_or(false)
        });

    // Layer
    zeile("layer");
    let layer_items: Vec<(Id, String)> = app
        .doc()
        .layers
        .iter()
        .map(|l| (l.id, l.name.clone()))
        .collect();
    let mut new_layer = layer;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.layer"), |ui| {
        let gewaehlt = if gleich_layer {
            panels::pick_combo(ui, &ziel.salt("layer"), new_layer, &layer_items)
        } else {
            // verschiedene Layer: Feld zeigt *variiert*, Auswahl setzt alle
            panels::dropdown(ui, &ziel.salt("layer"), "*variiert*", None, |ui| {
                let mut out = None;
                for (id, name) in &layer_items {
                    if ui.selectable_label(false, name).clicked() {
                        out = Some(*id);
                    }
                }
                out
            })
        };
        if let Some(l) = gewaehlt {
            new_layer = l;
        }
    });
    if new_layer != layer {
        out.layer = Some(new_layer);
    }

    // Farbe
    zeile("farbe");
    let by_layer = farbe.is_none() && gleich_farbe;
    let mut picked: Option<Option<Color>> = None;
    widgets::field_row(ui, farbe_label, |ui| {
        picked = panels::color_combo(
            ui,
            &ziel.salt("farbe"),
            farbe_gezeigt,
            by_layer,
            !gleich_farbe,
            jjcad_core::i18n::tr("gui.props.vom_layer"),
        );
    });
    if let Some(v) = picked {
        out.farbe = Some(v);
    }

    if mit_linie {
        // Linienstärke
        zeile("staerke");
        let mut picked_w: Option<f64> = None;
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.staerke"), |ui| {
            picked_w =
                panels::weight_combo(ui, &ziel.salt("staerke"), staerke, !gleich_staerke);
        });
        if let Some(w) = picked_w {
            out.staerke = Some(w);
        }

        // Linientyp
        zeile("linientyp");
        let mut picked_lt: Option<jjcad_core::style::LineType> = None;
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.linientyp"), |ui| {
            picked_lt =
                panels::linetype_combo(ui, &ziel.salt("linientyp"), linientyp, !gleich_typ);
        });
        if let Some(lt) = picked_lt {
            out.linientyp = Some(lt);
        }
    }

    out
}

/// Gemeinsame Zeilen auf die Vorgaben anwenden (Erzeugen).
pub fn gemein_nach_optionen(app: &mut App, a: &Gemein) {
    if let Some(l) = a.layer {
        app.doc_mut().current_layer = l;
    }
    if let Some(v) = a.farbe {
        app.ui.pending.color = v;
    }
    if let Some(w) = a.staerke {
        app.ui.pending.lineweight = Some(w);
    }
    if let Some(lt) = a.linientyp {
        app.ui.pending.line_type = Some(lt);
    }
}

/// Gemeinsame Zeilen als `modify.set` an die Auswahl schicken.
pub fn gemein_nach_objekt(app: &mut App, id: Id, a: &Gemein) {
    let mut params = serde_json::json!({"ids": [id]});
    if let Some(map) = params.as_object_mut() {
        if let Some(l) = a.layer {
            map.insert("layer".into(), serde_json::json!(l));
        }
        if let Some(v) = a.farbe {
            map.insert(
                "color".into(),
                serde_json::json!(match v {
                    Some(c) => c.to_hex(),
                    None => "bylayer".to_string(),
                }),
            );
        }
        if let Some(w) = a.staerke {
            map.insert("lineweight".into(), serde_json::json!(w));
        }
        if let Some(lt) = a.linientyp {
            map.insert("linetype".into(), serde_json::json!(linetype_key(lt)));
        }
    }
    if params.as_object().map(|m| m.len() > 1).unwrap_or(false) {
        setzen(app, &params);
    }
}

fn linetype_key(lt: jjcad_core::style::LineType) -> &'static str {
    match lt {
        jjcad_core::style::LineType::Solid => "solid",
        jjcad_core::style::LineType::Dashed => "dashed",
        jjcad_core::style::LineType::Dotted => "dotted",
        jjcad_core::style::LineType::DashDot => "dashdot",
        jjcad_core::style::LineType::Hidden => "hidden",
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Alle Zeilen eines Textes — für Erzeugen und Bearbeiten dieselben.
pub fn text(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let icon = "text";

    // ---- Werte besorgen ----
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let (titel, layer, farbe, mut hoehe, mut schrift, mut drehung, mut ah, mut av, mut stil) =
        match &e {
            Some(e) => match &e.kind {
                EntityKind::Text {
                    height,
                    angle,
                    align,
                    valign,
                    bold,
                    italic,
                    underline,
                    superscript,
                    font,
                    ..
                } => (
                    e.kind.type_name(),
                    e.layer,
                    e.style.color,
                    *height,
                    *font,
                    angle.to_degrees(),
                    *align,
                    *valign,
                    RunStyle {
                        bold: *bold,
                        italic: *italic,
                        underline: *underline,
                        superscript: *superscript,
                    },
                ),
                _ => return,
            },
            None => (
                "Text",
                app.doc().current_layer,
                app.options.text_color,
                app.options.text_height,
                app.options.text_font,
                app.options.text_angle,
                app.options.text_align,
                app.options.text_valign,
                RunStyle {
                    bold: app.options.text_bold,
                    italic: app.options.text_italic,
                    underline: app.options.text_underline,
                    superscript: app.options.text_superscript,
                },
            ),
        };

    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    kopf(app, ui, ziel, &titel, icon);
    reihenfolge(app, ui, ziel);

    // Text hat keine Linie: Stärke und Linientyp erscheinen in beiden Fällen nicht.
    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        0.35,
        jjcad_core::style::LineType::Solid,
        false,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);

    section_title(ui, icon, jjcad_core::i18n::tr("gui.props.text"));
    zeile("textkopf");

    // ---- Inhalt ----
    zeile("inhalt");
    let mut feld = Feld::default();
    // Während der Text in der Zeichenfläche getippt wird, gehören die Zeichen
    // diesem Editor — das Feld hier wäre ein zweiter Schreiber.
    let im_editor = ziel.id().is_some()
        && app.ui.texteingabe.as_ref().and_then(|t| t.id) == ziel.id();
    match (&e, ziel) {
        (_, _) if im_editor => {
            // Derselbe Kasten wie in den anderen Fällen — nur ohne Wirkung.
            let mut leer = String::new();
            ui.add_enabled_ui(false, |ui| {
                egui::TextEdit::multiline(&mut leer)
                    .id(egui::Id::new("sel_text_inhalt_gesperrt"))
                    .hint_text(jjcad_core::i18n::tr("gui.props.im_zeichenbereich"))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .show(ui);
            });
        }
        (Some(e), Ziel::Auswahl(id)) => {
            if let EntityKind::Text {
                text,
                runs,
                bold,
                italic,
                underline,
                superscript,
                font,
                ..
            } = &e.kind
            {
                feld = textinhalt(
                    app,
                    ui,
                    id,
                    text,
                    runs,
                    RunStyle {
                        bold: *bold,
                        italic: *italic,
                        underline: *underline,
                        superscript: *superscript,
                    },
                    *font,
                );
            }
        }
        _ => {
            // Noch kein Objekt: derselbe Kasten an derselben Stelle, nur ohne
            // Wirkung. Der Text wird in der Zeichenfläche getippt (T0019) —
            // ein zweites, wirkliches Feld hier wäre eine Lüge.
            let mut leer = String::new();
            ui.add_enabled_ui(false, |ui| {
                let ausgabe = egui::TextEdit::multiline(&mut leer)
                    .id(egui::Id::new("neu_text_inhalt"))
                    .hint_text(jjcad_core::i18n::tr("gui.props.im_zeichenbereich"))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .show(ui);
                let _ = ausgabe;
            });
        }
    }

    // ---- Auszeichnung ----
    zeile("auszeichnung");
    match ziel {
        Ziel::Neu => {
            let klick = panels::auszeichnen_knoepfe(ui, stil);
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.auszeichnung_des_neuen_textes")));
            if let Some((schluessel, wert)) = klick {
                setze_stil(&mut stil, schluessel, wert);
            }
        }
        Ziel::Auswahl(id) => {
            // Ohne Markierung gilt die Auszeichnung für das nächste Getippte
            // (wie in einem Textprogramm), mit Markierung sofort für die
            // markierten Zeichen.
            let pending = app
                .ui
                .text_format_next
                .filter(|(pid, _)| *pid == id)
                .map(|(_, st)| st);
            let hat_markierung = feld.markierung.is_some();
            let angezeigt = if hat_markierung {
                feld.stil_am_cursor
            } else {
                pending.unwrap_or(stil)
            };
            let (von, bis) = zeichenbereich(feld.markierung, feld.laenge);
            let klick = panels::auszeichnen_knoepfe(ui, angezeigt);
            ui.horizontal(|ui| {
                ui.label(theme::faint(if hat_markierung {
                    jjcad_core::i18n::t_args_owned("gui.props.zeichen_xx", &[("von", format!("{}", von)), ("bis", format!("{}", bis))])
                } else {
                    jjcad_core::i18n::tr("gui.props.naechste_zeichen").to_string()
                }))
                .on_hover_text(if hat_markierung {
                    jjcad_core::i18n::tr("gui.props.gilt_fuer_die_markierten_zeichen")
                } else {
                    jjcad_core::i18n::tr("gui.props.ohne_markierung_gilt_die_auszeichnung_fuer_das_was_sie_a")
                });
                if pending.is_some() && !hat_markierung {
                    ui.label(theme::dim(jjcad_core::i18n::tr("gui.props.vorgemerkt")));
                }
            });
            if let Some((schluessel, wert)) = klick {
                let mut neu = angezeigt;
                setze_stil(&mut neu, schluessel, wert);
                if hat_markierung {
                    // Mehrfachauswahl: die Auszeichnung gilt für jedes
                    // ausgewählte Textfeld — nicht nur für das angezeigte.
                    let ziele = if app.mehrfach_ziel.len() > 1 {
                        app.mehrfach_ziel.clone()
                    } else {
                        vec![id]
                    };
                    for ziel_id in ziele {
                        panels::auszeichnen(app, ziel_id, von, bis, schluessel, wert);
                    }
                    // Markierung erhalten, damit die zweite Auszeichnung nicht
                    // neu markiert werden muss.
                    let feld_id = egui::Id::new("sel_text_inhalt");
                    ui.memory_mut(|m| m.request_focus(feld_id));
                    let mut st = egui::text_edit::TextEditState::load(ui.ctx(), feld_id)
                        .unwrap_or_default();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(von),
                        egui::text::CCursor::new(bis),
                    )));
                    st.store(ui.ctx(), feld_id);
                }
                app.ui.text_format_next = Some((id, neu));
                ui.ctx().request_repaint();
            }
        }
    }

    ui.add_space(4.0);

    // ---- Größe, Schrift, Drehung, Ausrichtung ----
    zeile("textgroesse");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.textgroesse"), |ui| {
        widgets::length_field(
            ui,
            &mut hoehe,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });

    zeile("schriftart");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.schriftart"), |ui| {
        widgets::schrift_wahl(ui, &ziel.salt("schrift"), &mut schrift);
    });
    if !schrift.explodierbar() {
        ui.label(theme::faint(schrift.explode_hinweis()));
    }

    zeile("drehung");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.drehung"), |ui| {
        ui.add(
            crate::widgets::zahl(&mut drehung, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX)
                .suffix("°"),
        );
    });

    zeile("ausrichtung");
    widgets::field_block(ui, "Ausrichtung", widgets::ALIGN_HOEHE, |ui| {
        widgets::align_matrix(ui, &ziel.salt("ausrichtung"), &mut ah, &mut av);
    });

    ui.add_space(2.0);
    zeile("platzhalter");
    ui.label(theme::dim(jjcad_core::i18n::tr("gui.props.platzhalter_x_x_x_x")));

    // ---- Anwenden ----
    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.text_height = hoehe;
            app.options.text_font = schrift;
            app.options.text_angle = drehung;
            app.options.text_align = ah;
            app.options.text_valign = av;
            app.options.text_bold = stil.bold;
            app.options.text_italic = stil.italic;
            app.options.text_underline = stil.underline;
            app.options.text_superscript = stil.superscript;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                let alt = e.as_ref().and_then(|e| match &e.kind {
                    EntityKind::Text {
                        height,
                        angle,
                        align,
                        valign,
                        font,
                        ..
                    } => Some((*height, angle.to_degrees(), *align, *valign, *font)),
                    _ => None,
                });
                if let Some((h0, w0, ah0, av0, f0)) = alt {
                    if (hoehe - h0).abs() > 1e-9 {
                        map.insert("height".into(), serde_json::json!(unit.from_mm(hoehe)));
                        geaendert = true;
                    }
                    if (drehung - w0).abs() > 1e-9 {
                        map.insert("angle".into(), serde_json::json!(drehung));
                        geaendert = true;
                    }
                    if ah != ah0 {
                        map.insert("align".into(), serde_json::json!(ah.key()));
                        geaendert = true;
                    }
                    if av != av0 {
                        map.insert("valign".into(), serde_json::json!(av.key()));
                        geaendert = true;
                    }
                    if schrift != f0 {
                        map.insert("font".into(), serde_json::json!(schrift.key()));
                        geaendert = true;
                    }
                }
                if let Some((t, r)) = feld.aenderung.take() {
                    map.insert("text".into(), serde_json::json!(t));
                    map.insert(
                        "runs".into(),
                        serde_json::to_value(&r).unwrap_or(serde_json::json!([])),
                    );
                    geaendert = true;
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

fn setze_stil(stil: &mut RunStyle, schluessel: &str, wert: bool) {
    match schluessel {
        "bold" => stil.bold = wert,
        "italic" => stil.italic = wert,
        "underline" => stil.underline = wert,
        _ => stil.superscript = wert,
    }
}

/// Auswahlbereich im Textfeld: die Markierung, sonst der ganze Text.
fn zeichenbereich(markierung: Option<(usize, usize)>, laenge: usize) -> (usize, usize) {
    match markierung {
        Some((a, b)) if b > a => (a.min(laenge), b.min(laenge)),
        _ => (0, laenge),
    }
}

/// Was das Inhaltsfeld in diesem Bild geliefert hat.
#[derive(Default)]
struct Feld {
    /// Neuer Inhalt samt Läufen, wenn sich etwas geändert hat.
    aenderung: Option<(String, Vec<TextRun>)>,
    /// Markierung im Feld (Zeichenbereich), falls vorhanden.
    markierung: Option<(usize, usize)>,
    /// Länge des Textes in Zeichen — Grenze für den Zeichenbereich.
    laenge: usize,
    /// Auszeichnung an der Markierung; die Knöpfe zeigen sie.
    stil_am_cursor: RunStyle,
}

/// Textinhalt eines vorhandenen Textes mit gemischter Auszeichnung bearbeiten.
#[allow(clippy::too_many_arguments)]
fn textinhalt(
    app: &mut App,
    ui: &mut Ui,
    id: Id,
    text: &str,
    runs: &[TextRun],
    vorgabe: RunStyle,
    schrift: TextFont,
) -> Feld {
    let mut t = text.to_string();
    let pending = app
        .ui
        .text_format_next
        .filter(|(pid, _)| *pid == id)
        .map(|(_, st)| st);

    let text_vorher = text.to_string();
    let runs_vorher = runs.to_vec();
    let anzeige = jjcad_core::text::runs_for(
        &t,
        &jjcad_core::text::edit_runs(&text_vorher, &runs_vorher, &t, vorgabe, pending),
        vorgabe,
    );
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    let feld_farbe = ui.visuals().text_color();
    let satz_text = std::sync::Arc::new(text_vorher.clone());
    let satz_runs = std::sync::Arc::new(runs_vorher.clone());
    let mut setzer = move |ui: &Ui, text_jetzt: &str, wrap: f32| {
        let runs = jjcad_core::text::runs_for(
            text_jetzt,
            &jjcad_core::text::edit_runs(&satz_text, &satz_runs, text_jetzt, vorgabe, pending),
            vorgabe,
        );
        let mut job = crate::canvas::auszeichnungs_job(ui.ctx(), &runs, size, feld_farbe, schrift);
        job.wrap.max_width = wrap;
        ui.fonts(|f| f.layout_job(job))
    };
    // `anzeige` nur ableiten, damit die Läufe zum Feld passen; das Feld setzt
    // die Auszeichnung selbst.
    let _ = &anzeige;
    let ausgabe = egui::TextEdit::multiline(&mut t)
        .id(egui::Id::new("sel_text_inhalt"))
        .layouter(&mut setzer)
        .desired_rows(2)
        .desired_width(f32::INFINITY)
        .show(ui);

    // Markierung aus dem **gespeicherten** Zustand lesen: ein Klick auf einen
    // Umschalter nimmt dem Feld im selben Bild den Fokus, dann wäre
    // `cursor_range` schon leer.
    let feld_id = egui::Id::new("sel_text_inhalt");
    let markierung = egui::text_edit::TextEditState::load(ui.ctx(), feld_id)
        .and_then(|st| st.cursor.char_range())
        .map(|r| {
            let (p1, p2) = (r.primary.index, r.secondary.index);
            (p1.min(p2), p1.max(p2))
        })
        .or_else(|| {
            ausgabe.cursor_range.map(|cr| {
                let (p1, p2) = (cr.primary.ccursor.index, cr.secondary.ccursor.index);
                (p1.min(p2), p1.max(p2))
            })
        })
        .filter(|(a, b)| b > a);

    let stelle = markierung
        .map(|(a, _)| a)
        .unwrap_or(0)
        .min(text.chars().count().saturating_sub(1));
    let stil_am_cursor = jjcad_core::text::char_styles(text, runs, vorgabe)
        .get(stelle)
        .copied()
        .unwrap_or(vorgabe);

    let aenderung = if ausgabe.response.changed() {
        let frisch = jjcad_core::text::runs_for(
            &t,
            &jjcad_core::text::edit_runs(&text_vorher, &runs_vorher, &t, vorgabe, pending),
            vorgabe,
        );
        Some((t, frisch))
    } else {
        None
    };

    Feld {
        aenderung,
        markierung,
        laenge: text.chars().count(),
        stil_am_cursor,
    }
}

/// Panelwerte in die laufende Texteingabe spiegeln.
///
/// Das Panel schreibt in die Vorgaben (`Ziel::Neu`) bzw. über `modify.set` ins
/// Objekt; der Editor in der Zeichenfläche hält seine eigenen Werte, weil er
/// jeden Frame daraus zeichnet. Ohne diese Brücke bliebe die Vorschau hinter
/// einer Änderung im Panel zurück.
pub fn eingabe_spiegeln(app: &mut App) {
    let id = match app.ui.texteingabe.as_ref() {
        Some(t) => t.id,
        None => return,
    };
    // Erst lesen (unveränderlicher Borrow), dann schreiben.
    let werte = match id {
        None => Some((
            app.options.text_height,
            app.options.text_font,
            app.options.text_align,
            app.options.text_valign,
            app.options.text_angle.to_radians(),
        )),
        Some(id) => match app.doc().entity(id).map(|e| e.kind.clone()) {
            Some(EntityKind::Text {
                height,
                font,
                align,
                valign,
                angle,
                ..
            }) => Some((height, font, align, valign, angle)),
            _ => None,
        },
    };
    if let (Some((hoehe, schrift, ah, av, winkel)), Some(t)) =
        (werte, app.ui.texteingabe.as_mut())
    {
        t.height = hoehe;
        t.font = schrift;
        t.align = ah;
        t.valign = av;
        t.angle = winkel;
        // Größe und Drehung ändern die Lage des Feldes — neu zeichnen.
        app.ctx.request_repaint();
    }
}

// ---------------------------------------------------------------------------
// Wand
// ---------------------------------------------------------------------------

/// Alle Zeilen einer Wand — für Erzeugen und Bearbeiten dieselben.
pub fn wand(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let (layer, farbe, mut dicke, mut hoehe, mut ausrichtung, mut schraffur, mut fuellfarbe) =
        match &e {
            Some(e) => match &e.kind {
                EntityKind::Wall {
                    thickness,
                    height,
                    hatch,
                    align,
                    ..
                } => (
                    e.layer,
                    e.style.color,
                    *thickness,
                    *height,
                    *align,
                    *hatch,
                    e.style.fill,
                ),
                _ => return,
            },
            None => (
                app.doc().current_layer,
                app.ui.pending.color,
                app.options.wall_thickness,
                app.options.wall_height,
                app.options.wall_align,
                app.options.wall_hatch,
                app.ui.pending.fill,
            ),
        };

    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.wand"), "wall");
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        true,
        jjcad_core::i18n::tr("gui.props.linienfarbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "wall", jjcad_core::i18n::tr("gui.props.wand"));
    zeile("wandkopf");

    zeile("dicke");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.dicke"), |ui| {
        widgets::length_field(
            ui,
            &mut dicke,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });
    zeile("hoehe");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
        widgets::length_field(
            ui,
            &mut hoehe,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });
    zeile("ausrichtung");
    let align_items: Vec<(jjcad_core::style::WallAlign, String)> =
        jjcad_core::style::WallAlign::ALL
            .iter()
            .map(|c| (*c, c.text().to_string()))
            .collect();
    let mut picked_align = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.ausrichtung"), |ui| {
        picked_align = panels::pick_combo(
            ui,
            &ziel.salt("wand_ausrichtung"),
            ausrichtung,
            &align_items,
        );
    });
    if let Some(v) = picked_align {
        ausrichtung = v;
    }

    zeile("schraffur");
    let hatch_items: Vec<(jjcad_core::style::FillPattern, String)> =
        jjcad_core::style::FillPattern::ALL
            .iter()
            .map(|c| (*c, c.text().to_string()))
            .collect();
    let mut picked_hatch = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.schraffur"), |ui| {
        picked_hatch = panels::pick_combo(
            ui,
            &ziel.salt("wand_schraffur"),
            schraffur.unwrap_or(jjcad_core::style::FillPattern::Solid),
            &hatch_items,
        );
    });
    if let Some(hp) = picked_hatch {
        schraffur = if hp == jjcad_core::style::FillPattern::Solid {
            None
        } else {
            Some(hp)
        };
    }

    zeile("fuellfarbe");
    let fill_unset = fuellfarbe.is_none();
    let fill_cur = fuellfarbe.unwrap_or_else(|| farbe_gezeigt.mix(Color::WHITE, 0.74));
    let mut picked_fill: Option<Option<Color>> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fuellfarbe"), |ui| {
        picked_fill = panels::color_combo(
            ui,
            &ziel.salt("wand_fuellfarbe"),
            fill_cur,
            fill_unset,
            false,
            jjcad_core::i18n::tr("gui.props.standard"),
        );
    });
    if let Some(v) = picked_fill {
        fuellfarbe = v;
    }

    // Kennzahlen: beim Erzeugen noch nicht vorhanden.
    zeile("kennzahlen");
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Wall { a: pa, b: pb, .. } = &e.kind {
                ui.add_space(4.0);
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.laenge_x_achse_x_x_x_x", &[("wert1", format!("{}", unit.format(pa.dist(*pb), unit.decimals()))), ("wert2", format!("{:.2}", unit.from_mm(pa.x))), ("wert3", format!("{:.2}", unit.from_mm(pa.y))), ("wert4", format!("{:.2}", unit.from_mm(pb.x))), ("wert5", format!("{:.2}", unit.from_mm(pb.y)))])));
                // Relativer Abstand der Basispunkte — sichtbar ohne Scrollen.
                if let Some((dx, dy)) = crate::panels::delta_xy(e, unit) {
                    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.delta_x"), |ui| {
                        ui.label(theme::mono(dx.clone()));
                    });
                    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.delta_y"), |ui| {
                        ui.label(theme::mono(dy.clone()));
                    });
                }
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.volumen_x_m3", &[("wert1", format!("{:.3}", pa.dist(*pb) * dicke * hoehe / 1e9))])));
            }
        }
        _ => {
            ui.add_space(4.0);
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.laenge_und_volumen_erscheinen_am_platzierten_objekt")));
        }
    }

    // ---- Anwenden ----
    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.wall_thickness = dicke;
            app.options.wall_height = hoehe;
            app.options.wall_align = ausrichtung;
            app.options.wall_hatch = schraffur;
            if let Some(v) = picked_fill {
                app.ui.pending.fill = v;
            }
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Wall {
                        thickness,
                        height,
                        hatch,
                        align,
                        ..
                    } = &e.kind
                    {
                        if (dicke - *thickness).abs() > 1e-9 {
                            map.insert("thickness".into(), serde_json::json!(unit.from_mm(dicke)));
                            geaendert = true;
                        }
                        if (hoehe - *height).abs() > 1e-9 {
                            map.insert("height".into(), serde_json::json!(unit.from_mm(hoehe)));
                            geaendert = true;
                        }
                        if ausrichtung != *align {
                            map.insert(
                                "align".into(),
                                serde_json::json!(match ausrichtung {
                                    jjcad_core::style::WallAlign::Left => "left",
                                    jjcad_core::style::WallAlign::Center => "center",
                                    jjcad_core::style::WallAlign::Right => "right",
                                }),
                            );
                            geaendert = true;
                        }
                        if schraffur != *hatch {
                            map.insert(
                                "hatch".into(),
                                // Schlüssel aus `FillPattern::key` — dieselbe
                                // Quelle, die auch gelesen wird.
                                serde_json::json!(match schraffur {
                                    None => "none",
                                    Some(hp) => hp.key(),
                                }),
                            );
                            geaendert = true;
                        }
                    }
                    if fuellfarbe != e.style.fill {
                        map.insert(
                            "fill".into(),
                            serde_json::json!(match fuellfarbe {
                                Some(c) => c.to_hex(),
                                None => "none".to_string(),
                            }),
                        );
                        geaendert = true;
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tür
// ---------------------------------------------------------------------------

/// Alle Zeilen einer Tür — für Erzeugen und Bearbeiten dieselben.
pub fn tuer(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let (layer, farbe, mut breite, mut art, mut anschlag, mut blatt, mut basis) = match &e {
        Some(e) => match &e.kind {
            EntityKind::Door {
                width,
                kind,
                hinge,
                swing_in,
                base,
                ..
            } => (
                e.layer,
                e.style.color,
                *width,
                *kind,
                *hinge,
                *swing_in,
                *base,
            ),
            _ => return,
        },
        None => (
            app.doc().current_layer,
            app.ui.pending.color,
            app.options.door_width,
            app.options.door_kind,
            app.options.hinge,
            app.options.swing_in,
            app.options.door_base,
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.tuer"), "door");
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        true,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "door", jjcad_core::i18n::tr("gui.props.tuer"));
    zeile("tuerkopf");

    zeile("breite");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.breite"), |ui| {
        widgets::length_field(
            ui,
            &mut breite,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });

    zeile("typ");
    let typ_items: Vec<(DoorKind, String)> = DoorKind::ALL
        .iter()
        .map(|c| (*c, c.text().to_string()))
        .collect();
    let mut picked_typ = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.typ"), |ui| {
        picked_typ = panels::pick_combo(ui, &ziel.salt("tuer_typ"), art, &typ_items);
    });
    if let Some(v) = picked_typ {
        art = v;
    }

    zeile("anschlag");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.anschlag"), |ui| {
        if ui.selectable_value(&mut anschlag, 1.0, jjcad_core::i18n::tr("gui.props.rechts")).clicked() {}
        if ui.selectable_value(&mut anschlag, -1.0, jjcad_core::i18n::tr("gui.props.links")).clicked() {}
    });

    zeile("blatt");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.blatt"), |ui| {
        if ui
            .selectable_label(blatt, jjcad_core::i18n::tr("gui.props.seite_a"))
            .on_hover_text(jjcad_core::i18n::tr("gui.props.blatt_schlaegt_auf_seite_a_der_wand_auf"))
            .clicked()
        {
            blatt = true;
        }
        if ui
            .selectable_label(!blatt, jjcad_core::i18n::tr("gui.props.seite_b"))
            .on_hover_text(jjcad_core::i18n::tr("gui.props.blatt_schlaegt_auf_seite_b_der_wand_auf"))
            .clicked()
        {
            blatt = false;
        }
    });

    zeile("basispunkt");
    let basis_items: Vec<(DoorBase, String)> = DoorBase::ALL
        .iter()
        .map(|b| (*b, b.text().to_string()))
        .collect();
    let mut picked_basis = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.basispunkt"), |ui| {
        picked_basis = panels::pick_combo(ui, &ziel.salt("tuer_basis"), basis, &basis_items);
    });
    if let Some(v) = picked_basis {
        basis = v;
    }

    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Door { center, angle, .. } = &e.kind {
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.position_x_x_richtung_x", &[("wert1", format!("{:.2}", unit.from_mm(center.x))), ("wert2", format!("{:.2}", unit.from_mm(center.y))), ("wert3", format!("{:.0}", angle.to_degrees()))])));
            }
        }
        _ => {
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.sitzt_auf_der_vorderen_wandkante")));
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.r_dreht_die_oeffnung_f_spiegelt_sie")));
        }
    }

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.door_width = breite;
            app.options.door_kind = art;
            app.options.hinge = anschlag;
            app.options.swing_in = blatt;
            app.options.door_base = basis;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Door {
                        width,
                        kind,
                        hinge,
                        swing_in,
                        base,
                        ..
                    } = &e.kind
                    {
                        if (breite - *width).abs() > 1e-9 {
                            map.insert("width".into(), serde_json::json!(unit.from_mm(breite)));
                            geaendert = true;
                        }
                        if art != *kind {
                            map.insert(
                                "kind".into(),
                                serde_json::json!(match art {
                                    DoorKind::Hinge => "hinge",
                                    DoorKind::Sliding => "sliding",
                                    DoorKind::Double => "double",
                                    DoorKind::Opening => "opening",
                                }),
                            );
                            geaendert = true;
                        }
                        if (anschlag - *hinge).abs() > 1e-9 {
                            map.insert("hinge".into(), serde_json::json!(anschlag));
                            geaendert = true;
                        }
                        if blatt != *swing_in {
                            map.insert("swing_in".into(), serde_json::json!(blatt));
                            geaendert = true;
                        }
                        if basis != *base {
                            map.insert("base".into(), serde_json::json!(basis.key()));
                            geaendert = true;
                        }
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fenster
// ---------------------------------------------------------------------------

/// Alle Zeilen eines Fensters — für Erzeugen und Bearbeiten dieselben.
pub fn fenster(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let (layer, farbe, mut breite, mut bruestung, mut hoehe, mut lage) = match &e {
        Some(e) => match &e.kind {
            EntityKind::Window {
                width,
                sill,
                height,
                align,
                ..
            } => (
                e.layer,
                e.style.color,
                *width,
                *sill,
                *height,
                *align,
            ),
            _ => return,
        },
        None => (
            app.doc().current_layer,
            app.ui.pending.color,
            app.options.window_width,
            app.options.window_sill,
            app.options.window_height,
            app.options.window_align,
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.fenster"), "window");
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        true,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "window", jjcad_core::i18n::tr("gui.props.fenster"));
    zeile("fensterkopf");

    zeile("breite");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.breite"), |ui| {
        widgets::length_field(
            ui,
            &mut breite,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });
    zeile("bruestung");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.bruestung"), |ui| {
        widgets::length_field(ui, &mut bruestung, unit, ui.available_width(), 0.0, widgets::MASS_MAX_MM);
    });
    zeile("hoehe");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
        widgets::length_field(
            ui,
            &mut hoehe,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });
    zeile("ausrichtung");
    let lage_items: Vec<(jjcad_core::style::WallAlign, String)> =
        jjcad_core::style::WallAlign::ALL
            .iter()
            .map(|a| (*a, a.text().to_string()))
            .collect();
    let mut picked_lage = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.ausrichtung"), |ui| {
        picked_lage = panels::pick_combo(ui, &ziel.salt("fenster_lage"), lage, &lage_items);
    });
    if let Some(v) = picked_lage {
        lage = v;
    }

    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Window { center, angle, .. } = &e.kind {
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.position_x_x_richtung_x", &[("wert1", format!("{:.2}", unit.from_mm(center.x))), ("wert2", format!("{:.2}", unit.from_mm(center.y))), ("wert3", format!("{:.0}", angle.to_degrees()))])));
            }
        }
        _ => {
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.lage_in_der_wanddicke_links_mittig_rechts")));
        }
    }

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.window_width = breite;
            app.options.window_sill = bruestung;
            app.options.window_height = hoehe;
            app.options.window_align = lage;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Window {
                        width,
                        sill,
                        height,
                        align,
                        ..
                    } = &e.kind
                    {
                        if (breite - *width).abs() > 1e-9 {
                            map.insert("width".into(), serde_json::json!(unit.from_mm(breite)));
                            geaendert = true;
                        }
                        if (bruestung - *sill).abs() > 1e-9 {
                            map.insert("sill".into(), serde_json::json!(unit.from_mm(bruestung)));
                            geaendert = true;
                        }
                        if (hoehe - *height).abs() > 1e-9 {
                            map.insert("height".into(), serde_json::json!(unit.from_mm(hoehe)));
                            geaendert = true;
                        }
                        if lage != *align {
                            map.insert(
                                "align".into(),
                                serde_json::json!(match lage {
                                    jjcad_core::style::WallAlign::Left => "left",
                                    jjcad_core::style::WallAlign::Center => "center",
                                    jjcad_core::style::WallAlign::Right => "right",
                                }),
                            );
                            geaendert = true;
                        }
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Füllfläche
// ---------------------------------------------------------------------------

/// Alle Zeilen einer Füllfläche — für Erzeugen und Bearbeiten dieselben.
///
/// Die vier Erstellwege (Kontur, Umriss, Punktfolge, Farbeimer) teilen sich
/// Muster, Winkel und Abstand; nur die Umrandung merkt sich jeder Weg getrennt.
pub fn fuellflaeche(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let werkzeug = app
        .tool
        .as_ref()
        .map(|t| t.action.clone())
        .unwrap_or_default();

    let (layer, farbe, mut muster, mut winkel, mut abstand, mut rand, mut fuellfarbe) =
        match &e {
            Some(e) => match &e.kind {
                EntityKind::Fill {
                    pattern,
                    pattern_angle,
                    pattern_scale,
                    border,
                    ..
                } => (
                    e.layer,
                    e.style.color,
                    *pattern,
                    pattern_angle.to_degrees(),
                    *pattern_scale,
                    *border,
                    e.style.fill,
                ),
                _ => return,
            },
            None => (
                app.doc().current_layer,
                app.ui.pending.color,
                // Der Farbeimer hat sein eigenes Muster (startet mit
                // Vollfläche, wie ein Farbeimer im Malprogramm).
                if werkzeug == "draw.fill.paint" {
                    app.options.paint_pattern
                } else {
                    app.options.fill_pattern
                },
                app.options.fill_angle.to_degrees(),
                app.options.fill_scale,
                app.options.fill_border_for(&werkzeug),
                app.ui.pending.fill,
            ),
        };

    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.fuellflaeche"), "fill");
    reihenfolge(app, ui, ziel);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        0.35,
        jjcad_core::style::LineType::Solid,
        true,
        jjcad_core::i18n::tr("gui.props.linienfarbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "fill", jjcad_core::i18n::tr("gui.props.fuellflaeche"));
    zeile("fuellkopf");

    zeile("muster");
    let muster_items: Vec<(jjcad_core::style::FillPattern, String)> =
        jjcad_core::style::FillPattern::ALL
            .iter()
            .map(|c| (*c, c.text().to_string()))
            .collect();
    let mut picked = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.muster"), |ui| {
        picked = panels::pick_combo(ui, &ziel.salt("fuell_muster"), muster, &muster_items);
    });
    if let Some(v) = picked {
        muster = v;
    }

    zeile("winkel");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.winkel"), |ui| {
        ui.add(
            crate::widgets::zahl(&mut winkel, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX)
                .suffix("°"),
        );
    });

    zeile("abstand");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.abstand"), |ui| {
        widgets::length_field(
            ui,
            &mut abstand,
            unit,
            ui.available_width(),
            widgets::ABSTAND_MIN_MM,
            widgets::ABSTAND_MAX_MM,
        );
    });

    zeile("umrandung");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.umrandung"), |ui| {
        ui.checkbox(&mut rand, "");
    });

    zeile("fuellfarbe");
    let fill_unset = fuellfarbe.is_none();
    let fill_cur = fuellfarbe.unwrap_or_else(|| farbe_gezeigt.mix(Color::WHITE, 0.72));
    let mut picked_fill: Option<Option<Color>> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fuellfarbe"), |ui| {
        picked_fill = panels::color_combo(
            ui,
            &ziel.salt("fuell_farbe"),
            fill_cur,
            fill_unset,
            false,
            jjcad_core::i18n::tr("gui.props.standard"),
        );
    });
    if let Some(v) = picked_fill {
        fuellfarbe = v;
    }

    // Kennzahlen: beim Erzeugen gibt es noch keine Umgrenzung.
    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Fill {
                boundary, holes, ..
            } = &e.kind
            {
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_punkte", &[("wert1", format!("{}", boundary.len()))])));
                if !holes.is_empty() {
                    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_inseln", &[("wert1", format!("{}", holes.len()))])));
                }
            }
        }
        _ => {
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.punkte_flaeche_und_umriss_erscheinen_am_platzierten_obje")));
        }
    }

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            if werkzeug == "draw.fill.paint" {
                app.options.paint_pattern = muster;
            } else {
                app.options.fill_pattern = muster;
            }
            app.options.fill_angle = winkel.to_radians();
            app.options.fill_scale = abstand;
            if !werkzeug.is_empty() {
                *app.options.fill_border_for_mut(&werkzeug) = rand;
            }
            if let Some(v) = picked_fill {
                app.ui.pending.fill = v;
            }
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Fill {
                        pattern,
                        pattern_angle,
                        pattern_scale,
                        border,
                        ..
                    } = &e.kind
                    {
                        if muster != *pattern {
                            map.insert("pattern".into(), serde_json::json!(muster.key()));
                            geaendert = true;
                        }
                        if (winkel - pattern_angle.to_degrees()).abs() > 1e-9 {
                            map.insert("pattern_angle".into(), serde_json::json!(winkel));
                            geaendert = true;
                        }
                        if (abstand - *pattern_scale).abs() > 1e-9 {
                            map.insert(
                                "pattern_scale".into(),
                                serde_json::json!(unit.from_mm(abstand)),
                            );
                            geaendert = true;
                        }
                        if rand != *border {
                            map.insert("border".into(), serde_json::json!(rand));
                            geaendert = true;
                        }
                    }
                    if fuellfarbe != e.style.fill {
                        map.insert(
                            "fill".into(),
                            serde_json::json!(match fuellfarbe {
                                Some(c) => c.to_hex(),
                                None => "none".to_string(),
                            }),
                        );
                        geaendert = true;
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bemaßung
// ---------------------------------------------------------------------------

/// Alle Zeilen einer Bemaßung — für Erzeugen und Bearbeiten dieselben.
pub fn bemassung(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let unit = app.unit();
    // Abstand der Maßlinie zu den Basispunkten (vorzeichenbehaftet).
    let mut abstand = e
        .as_ref()
        .and_then(|e| jjcad_core::render::dim_offset(&e.kind));
    let (layer, farbe, mut achse, mut masstext) = match &e {
        Some(e) => match &e.kind {
            EntityKind::Dim {
                axis,
                override_text,
                ..
            } => (
                e.layer,
                e.style.color,
                Some(*axis),
                override_text.clone().unwrap_or_default(),
            ),
            _ => return,
        },
        None => (
            app.doc().current_layer,
            app.ui.pending.color,
            app.options.dim_axis,
            app.options.dim_text.clone(),
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.bemassung"), "dim");
    reihenfolge(app, ui, ziel);

    let (staerke, linientyp) = linienwerte(app, &e);
    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        true,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "dim", jjcad_core::i18n::tr("gui.props.bemassung"));
    zeile("dimkopf");

    zeile("richtung");
    // Beim Erzeugen steht zusätzlich „Automatisch“ zur Wahl (Achse aus der
    // längeren Richtung); ein vorhandenes Maß hat immer eine feste Achse.
    let axis_items: Vec<(Option<DimAxis>, String)> = std::iter::once((
        None,
        jjcad_core::i18n::tr("gui.props.automatisch_2").to_string(),
    ))
    .chain(
        DimAxis::ALL
            .iter()
            .map(|a| (Some(*a), a.text().to_string())),
    )
    .collect();
    let mut picked = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.richtung"), |ui| {
        picked = panels::pick_combo(ui, &ziel.salt("dim_achse"), achse, &axis_items);
    });
    if let Some(v) = picked {
        achse = v;
    }

    zeile("masstext");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.masstext"), |ui| {
        ui.add_sized(
            [120.0, 20.0],
            egui::TextEdit::singleline(&mut masstext).hint_text(jjcad_core::i18n::tr("gui.props.automatisch")),
        );
    });

    // Lage der Maßlinie: derselbe Wert, den die Raute an der Maßlinie zieht
    // (T0055). Beim Erzeugen bestimmen die Klicks die Lage.
    zeile("abstand");
    match abstand {
        Some(alt) => {
            let mut v = alt;
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.abstand"), |ui| {
                widgets::length_field(
                    ui,
                    &mut v,
                    unit,
                    110.0,
                    widgets::KOORD_MIN_MM,
                    widgets::KOORD_MAX_MM,
                );
            });
            abstand = Some(v);
        }
        None => gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.abstand"), jjcad_core::i18n::tr("gui.props.aus_den_klicks")),
    }

    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Dim { axis, a, b, .. } = &e.kind {
                let m = match axis {
                    DimAxis::X => (b.x - a.x).abs(),
                    DimAxis::Y => (b.y - a.y).abs(),
                    DimAxis::Aligned => a.dist(*b),
                };
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.mass_x_x", &[("wert1", format!("{}", jjcad_core::units::format_number(
                        app.doc().settings.dim_unit.from_mm(m),
                        app.doc().dim_style.decimals
                    ))), ("wert2", format!("{}", app.doc().settings.dim_unit.short()))])));
            }
        }
        _ => {
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.mass_und_masstext_erscheinen_am_platzierten_objekt")));
        }
    }

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.dim_axis = achse;
            app.options.dim_text = masstext;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Dim {
                        axis,
                        override_text,
                        ..
                    } = &e.kind
                    {
                        let alt_text = override_text.clone().unwrap_or_default();
                        if masstext != alt_text {
                            map.insert("text".into(), serde_json::json!(masstext));
                            geaendert = true;
                        }
                        if let Some(neu_achse) = achse {
                            if neu_achse != *axis {
                                map.insert(
                                    "axis".into(),
                                    serde_json::json!(match neu_achse {
                                        DimAxis::X => "x",
                                        DimAxis::Y => "y",
                                        DimAxis::Aligned => "aligned",
                                    }),
                                );
                                geaendert = true;
                            }
                        }
                        if let Some(v) = abstand {
                            if let Some(alt) = jjcad_core::render::dim_offset(&e.kind) {
                                if (v - alt).abs() > 1e-6 {
                                    map.insert("offset".into(), serde_json::json!(unit.from_mm(v)));
                                    geaendert = true;
                                }
                            }
                        }
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Objektarten, deren Maße aus den Klicks entstehen
// ---------------------------------------------------------------------------

/// Linie, Kreis, Bogen, Block … — alles, was beim Platzieren keine
/// Eingabewerte braucht, sondern aus den Klicks entsteht.
///
/// Diese Arten teilen sich **einen** Renderer: gleiche Zeilen beim Erzeugen und
/// beim Bearbeiten. Die Maße (Radius, Winkel, Länge) sind beim Erzeugen
/// gesperrt — sie stehen trotzdem da, mit dem Hinweis, woher sie kommen.
/// Sonst würde das Panel beim Wechsel zwischen Platzieren und Auswahl springen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Einfach {
    Punkt,
    Linie,
    Polylinie,
    Spline,
    Rechteck,
    Kreis,
    Bogen,
    Ellipse,
    /// Abschnitt einer Ellipse — entsteht beim Stutzen (T0084).
    Ellipsenbogen,
    Masskette,
    /// Grundriss-Kote: Kreis-Symbol, Höhe im Panel.
    Grundrisskote,
    /// Schnitt-Kote: Punktfolge, Höhen aus der Y-Differenz.
    Schnittkote,
    Block,
}

impl Einfach {
    /// Art aus dem Werkzeug, das gerade läuft.
    pub fn aus_werkzeug(id: &str) -> Option<Self> {
        Some(match id {
            "draw.point" => Self::Punkt,
            "draw.line" => Self::Linie,
            "draw.polyline" => Self::Polylinie,
            "draw.spline" => Self::Spline,
            "draw.rect" => Self::Rechteck,
            "draw.circle" => Self::Kreis,
            "draw.arc" => Self::Bogen,
            "draw.ellipse" => Self::Ellipse,
            "dim.chain" => Self::Masskette,
            "dim.elevation.plan" => Self::Grundrisskote,
            "dim.elevation.section" => Self::Schnittkote,
            "block.insert" => Self::Block,
            _ => return None,
        })
    }

    /// Art aus einem vorhandenen Objekt.
    pub fn aus_objekt(kind: &EntityKind) -> Option<Self> {
        Some(match kind {
            EntityKind::Point { .. } => Self::Punkt,
            EntityKind::Line { .. } => Self::Linie,
            EntityKind::Polyline { .. } => Self::Polylinie,
            EntityKind::Spline { .. } => Self::Spline,
            EntityKind::Circle { .. } => Self::Kreis,
            EntityKind::Arc { .. } => Self::Bogen,
            EntityKind::Ellipse { .. } => Self::Ellipse,
            EntityKind::EllipseArc { .. } => Self::Ellipsenbogen,
            EntityKind::DimChain { .. } => Self::Masskette,
            EntityKind::ElevationPlan { .. } => Self::Grundrisskote,
            EntityKind::ElevationSection { .. } => Self::Schnittkote,
            EntityKind::BlockRef { .. } => Self::Block,
            _ => return None,
        })
    }

    fn titel(self) -> &'static str {
        match self {
            Self::Punkt => jjcad_core::i18n::tr("gui.props.punkt"),
            Self::Linie => jjcad_core::i18n::tr("gui.props.linie"),
            Self::Polylinie => jjcad_core::i18n::tr("gui.props.polylinie"),
            Self::Spline => jjcad_core::i18n::tr("gui.props.spline"),
            Self::Rechteck => jjcad_core::i18n::tr("gui.props.rechteck"),
            Self::Kreis => jjcad_core::i18n::tr("gui.props.kreis"),
            Self::Bogen => jjcad_core::i18n::tr("gui.props.bogen"),
            Self::Ellipse => jjcad_core::i18n::tr("gui.props.ellipse"),
            Self::Ellipsenbogen => jjcad_core::i18n::tr("gui.props.ellipsenbogen"),
            Self::Masskette => jjcad_core::i18n::tr("gui.props.masskette"),
            Self::Grundrisskote => "Grundriss-Kote",
            Self::Schnittkote => "Schnitt-Kote",
            Self::Block => jjcad_core::i18n::tr("gui.props.block"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Punkt => "point",
            Self::Linie => "line",
            Self::Polylinie | Self::Rechteck => "polyline",
            Self::Spline => "spline",
            Self::Kreis => "circle",
            Self::Bogen => "arc",
            Self::Ellipse => "ellipse",
            Self::Ellipsenbogen => "arc",
            Self::Masskette => "dim",
            Self::Grundrisskote => "elevation",
            Self::Schnittkote => "elevation-section",
            Self::Block => "blocknew",
        }
    }

    /// Hat die Art eine Linie (Farbe, Stärke, Linientyp)?
    fn mit_linie(self) -> bool {
        !matches!(self, Self::Block)
    }
}

/// Raumstempel: Überschrift, Textgröße und die Kennzahlen der unsichtbaren
/// Fläche — dieselben Zeilen beim Erzeugen und beim Bearbeiten.
///
/// Die Lage des Textfeldes steht **nicht** hier: sie wird in der Zeichnung am
/// runden Griff verschoben (Fläche bleibt stehen, T0097). Die Fläche selbst hat
/// keine eigene Einstellung — sie ist die Umgrenzung, deren Ecken die Griffe
/// zeigen.
pub fn raumstempel(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let (layer, farbe, mut hoehe, mut kopf_text) = match &e {
        Some(e) => match &e.kind {
            EntityKind::Room { height, heading, .. } => {
                (e.layer, e.style.color, *height, heading.clone())
            }
            _ => return,
        },
        None => (
            app.doc().current_layer,
            app.ui.pending.color,
            app.options.room_height,
            app.options.room_heading.clone(),
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.raumstempel"), "room");
    reihenfolge(app, ui, ziel);
    // Ein Raumstempel hat keine Linie: Stärke und Linientyp entfallen in
    // beiden Fällen (wie beim Text).
    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        0.35,
        jjcad_core::style::LineType::Solid,
        false,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "room", jjcad_core::i18n::tr("gui.props.raumstempel"));
    zeile("artkopf");

    zeile("ueberschrift");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.ueberschrift"), |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut kopf_text)
                .id(egui::Id::new(ziel.salt("ueberschrift")))
                .desired_width(ui.available_width()),
        );
    });

    zeile("textgroesse");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.textgroesse"), |ui| {
        widgets::length_field(
            ui,
            &mut hoehe,
            unit,
            110.0,
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });

    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Room { boundary, .. } = &e.kind {
                let flaeche = jjcad_core::area::area_of(boundary, &[]);
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_punkte_flaeche_x", &[("wert1", format!("{}", boundary.len())), ("wert2", format!("{}", panels::area_text(unit, flaeche)))])));
            }
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.textlage_am_runden_griff_in_der_zeichnung")));
        }
        _ => {
            ui.label(theme::faint(
                jjcad_core::i18n::tr("gui.props.flaeche_und_text_entstehen_beim_abschluss_der_punktfolge"),
            ));
        }
    }

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.room_height = hoehe;
            app.options.room_heading = kopf_text;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(e) = &e {
                if let EntityKind::Room { height, heading, .. } = &e.kind {
                    if (hoehe - *height).abs() > 1e-9 {
                        params["height"] = serde_json::json!(unit.from_mm(hoehe));
                        geaendert = true;
                    }
                    if kopf_text != *heading {
                        params["text"] = serde_json::json!(kopf_text);
                        geaendert = true;
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

/// Alle Zeilen einer Klick-Art — für Erzeugen und Bearbeiten dieselben.
pub fn einfach(app: &mut App, ui: &mut Ui, ziel: Ziel, art: Einfach) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let (layer, farbe) = match &e {
        Some(e) => (e.layer, e.style.color),
        None => (app.doc().current_layer, app.ui.pending.color),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    let (staerke, linientyp) = linienwerte(app, &e);

    kopf(app, ui, ziel, art.titel(), art.icon());
    reihenfolge(app, ui, ziel);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        art.mit_linie(),
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, art.icon(), art.titel());
    zeile("artkopf");

    // ---- Art-eigene Zeilen ----
    let mut radius: Option<f64> = None;
    let mut halbachse_x: Option<f64> = None;
    let mut halbachse_y: Option<f64> = None;
    let mut startwinkel: Option<f64> = None;
    let mut endwinkel: Option<f64> = None;
    let mut hoehe: Option<f64> = None;
    let mut drehung: Option<f64> = None;
    let mut faktor: Option<f64> = None;
    // Lage der Maßlinie einer Maßkette (Abstand zu den Basispunkten).
    let mut abstand: Option<f64> = None;

    match art {
        Einfach::Kreis => {
            let wert = match &e {
                Some(e) => match &e.kind {
                    EntityKind::Circle { r, .. } => *r,
                    _ => 0.0,
                },
                None => 0.0,
            };
            radius = Some(wert);
            zeile("radius");
            if e.is_none() {
                gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.radius"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
            } else {
                widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.radius"), |ui| {
                    let mut v = wert;
                    if widgets::length_field(ui, &mut v, unit, 110.0, 0.1, widgets::MASS_MAX_MM) {
                        radius = Some(v);
                    }
                });
            }
        }
        Einfach::Bogen => {
            let (r0, a0, a1) = match &e {
                Some(e) => match &e.kind {
                    EntityKind::Arc { r, a0, a1, .. } => {
                        (*r, a0.to_degrees(), a1.to_degrees())
                    }
                    _ => (0.0, 0.0, 0.0),
                },
                None => (0.0, 0.0, 0.0),
            };
            radius = Some(r0);
            startwinkel = Some(a0);
            endwinkel = Some(a1);
            let gesperrt = e.is_none();
            for (key, label, wert) in [
                ("radius", jjcad_core::i18n::tr("gui.props.radius"), r0),
                ("startwinkel", jjcad_core::i18n::tr("gui.props.startwinkel"), a0),
                ("endwinkel", jjcad_core::i18n::tr("gui.props.endwinkel"), a1),
            ] {
                zeile(key);
                if gesperrt {
                    gesperrt_zeile(ui, label, jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
                    continue;
                }
                let mut v = wert;
                widgets::field_row(ui, label, |ui| {
                    if key == "radius" {
                        widgets::length_field(ui, &mut v, unit, 110.0, 0.1, widgets::MASS_MAX_MM);
                    } else {
                        ui.add(
                            crate::widgets::zahl(
                                &mut v,
                                1.0,
                                widgets::WINKEL_MIN..=widgets::WINKEL_MAX,
                            )
                            .suffix("°"),
                        );
                    }
                });
                match key {
                    "radius" => radius = Some(v),
                    "startwinkel" => startwinkel = Some(v),
                    _ => endwinkel = Some(v),
                }
            }
        }
        // Ellipsenbogen: beide Halbachsen, Drehlage und die Parameterwinkel.
        // Achsen und Drehung lassen sich hier ändern, die Winkel begrenzen den
        // Abschnitt — dieselben Werte, die das Stutzen erzeugt.
        Einfach::Ellipsenbogen => {
            let (rx0, ry0, rot0, w0, w1) = match &e {
                Some(e) => match &e.kind {
                    EntityKind::EllipseArc { rx, ry, rot, a0, a1, .. } => {
                        (*rx, *ry, rot.to_degrees(), a0.to_degrees(), a1.to_degrees())
                    }
                    _ => (0.0, 0.0, 0.0, 0.0, 0.0),
                },
                None => (0.0, 0.0, 0.0, 0.0, 0.0),
            };
            halbachse_x = Some(rx0);
            halbachse_y = Some(ry0);
            drehung = Some(rot0);
            startwinkel = Some(w0);
            endwinkel = Some(w1);
            let gesperrt = e.is_none();
            for (key, label, wert) in [
                ("halbachse_x", jjcad_core::i18n::tr("gui.props.halbachse_x"), rx0),
                ("halbachse_y", jjcad_core::i18n::tr("gui.props.halbachse_y"), ry0),
            ] {
                zeile(key);
                if gesperrt {
                    gesperrt_zeile(ui, label, jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
                    continue;
                }
                let mut v = wert;
                widgets::field_row(ui, label, |ui| {
                    widgets::length_field(ui, &mut v, unit, 110.0, 0.1, widgets::MASS_MAX_MM);
                });
                if key == "halbachse_x" {
                    halbachse_x = Some(v);
                } else {
                    halbachse_y = Some(v);
                }
            }
            for (key, label, wert) in [
                ("drehung", jjcad_core::i18n::tr("gui.props.drehung"), rot0),
                ("startwinkel", jjcad_core::i18n::tr("gui.props.startwinkel"), w0),
                ("endwinkel", jjcad_core::i18n::tr("gui.props.endwinkel"), w1),
            ] {
                zeile(key);
                if gesperrt {
                    gesperrt_zeile(ui, label, jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
                    continue;
                }
                let mut v = wert;
                widgets::field_row(ui, label, |ui| {
                    ui.add(
                        crate::widgets::zahl(&mut v, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX)
                            .suffix("°"),
                    );
                });
                match key {
                    "drehung" => drehung = Some(v),
                    "startwinkel" => startwinkel = Some(v),
                    _ => endwinkel = Some(v),
                }
            }
        }
        // Grundriss-Kote: die Höhe ist die einzige Eingabe des Nutzers.
        Einfach::Grundrisskote => {
            let z0 = match &e {
                Some(e) => match &e.kind {
                    EntityKind::ElevationPlan { z, .. } => *z,
                    _ => 0.0,
                },
                None => app.options.elevation_z,
            };
            hoehe = Some(z0);
            zeile("hoehe");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
                let mut v = z0;
                if widgets::length_field(
                    ui,
                    &mut v,
                    unit,
                    100.0,
                    widgets::KOORD_MIN_MM,
                    widgets::KOORD_MAX_MM,
                ) {
                    hoehe = Some(v);
                } else {
                    hoehe = Some(z0);
                }
            });
        }
        // Schnitt-Kote: keine Höheneingabe — der erste Punkt ist ±0,00, alle
        // weiteren Höhen folgen aus der Y-Differenz. Punktzahl und die sich
        // ergebenden Höhen stehen als Anzeige da.
        Einfach::Schnittkote => {
            let pts: Vec<jjcad_core::geom::Vec2> = match &e {
                Some(e) => match &e.kind {
                    EntityKind::ElevationSection { pts } => pts.clone(),
                    _ => Vec::new(),
                },
                // Beim Platzieren steht die Punktfolge im laufenden Werkzeug.
                None => app
                    .tool
                    .as_ref()
                    .filter(|t| t.action == "dim.elevation.section")
                    .map(|t| t.pts.clone())
                    .unwrap_or_default(),
            };
            zeile("punkte");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.punkte"), |ui| {
                ui.label(theme::mono(pts.len().to_string()));
            });
            let basis = pts.first().map(|p| p.y).unwrap_or(0.0);
            let liste = if pts.is_empty() {
                "—".to_string()
            } else {
                pts.iter()
                    .map(|p| jjcad_core::units::elevation_section_text(p.y - basis))
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            zeile("hoehen");
            // Die Wertespalte ist 150 px breit — die Höhen stehen deshalb als
            // umbrechende Zeile unter der Anzahl, sonst wäre die Liste
            // abgeschnitten.
            let kurz = if pts.is_empty() {
                "—".to_string()
            } else {
                format!("{} Werte", pts.len())
            };
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehen"), |ui| {
                ui.label(theme::mono(kurz.clone()));
            });
            ui.label(theme::dim(liste.clone()));
        }
        // Maßkette: jeder Basispunkt hängt am Griff in der Zeichnung, die
        // **Lage der Maßlinie** steht hier als Abstand (vorzeichenbehaftet,
        // positiv = links der Messrichtung) — dieselbe Zahl zieht die Raute
        // auf der Maßlinie (T0055).
        Einfach::Masskette => {
            let wert = e
                .as_ref()
                .and_then(|e| jjcad_core::render::dim_offset(&e.kind));
            zeile("abstand");
            match wert {
                Some(alt) => {
                    let mut v = alt;
                    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.abstand"), |ui| {
                        widgets::length_field(
                            ui,
                            &mut v,
                            unit,
                            110.0,
                            widgets::KOORD_MIN_MM,
                            widgets::KOORD_MAX_MM,
                        );
                    });
                    abstand = Some(v);
                }
                None => gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.abstand"), jjcad_core::i18n::tr("gui.props.aus_den_klicks")),
            }
        }
        Einfach::Block => {
            let (r0, s0) = match &e {
                Some(e) => match &e.kind {
                    EntityKind::BlockRef { rot, scale, .. } => (rot.to_degrees(), scale.x),
                    _ => (0.0, 1.0),
                },
                None => (app.options.block_rot, app.options.block_scale),
            };
            drehung = Some(r0);
            faktor = Some(s0);
            zeile("drehung");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.drehung"), |ui| {
                let mut v = r0;
                if ui
                    .add(
                        crate::widgets::zahl(&mut v, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX)
                            .suffix("°"),
                    )
                    .changed()
                {
                    drehung = Some(v);
                }
            });
            zeile("faktor");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.faktor"), |ui| {
                let mut v = s0;
                if ui.add(crate::widgets::zahl(&mut v, 0.01, 0.01..=1000.0)).changed() {
                    faktor = Some(v);
                }
            });
        }
        _ => {}
    }

    // Einfügemodus: eine Eigenschaft des **Werkzeugs**, keine des Objekts.
    // Er steht nur beim Platzieren im Panel und wird deshalb nicht als
    // Objektzeile protokolliert.
    if matches!(art, Einfach::Rechteck | Einfach::Ellipse | Einfach::Bogen) && ziel.ist_neu() {
        let tool_id = app
            .tool
            .as_ref()
            .map(|t| t.action.clone())
            .unwrap_or_default();
        if tool_id == "draw.arc" {
            // Eingabemodus des Bogens: drei Punkte oder Mittelpunkt, Start
            // und Ende (T0085). Wie beim Rechteck eine Werkzeugeinstellung.
            zeile_nur_neu("bogenmodus");
            let cur = app.options.arc_mode;
            let items: Vec<(crate::app::Bogenmodus, String)> = crate::app::Bogenmodus::ALL
                .iter()
                .map(|m| (*m, m.label().to_string()))
                .collect();
            let mut picked = None;
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.eingabemodus"), |ui| {
                picked = panels::pick_combo(ui, &ziel.salt("bogenmodus"), cur, &items);
            });
            if let Some(m) = picked {
                app.options.arc_mode = m;
            }
        } else if tool_id == "draw.rect" || tool_id == "draw.ellipse" {
            zeile_nur_neu("einfuegemodus");
            let cur = app.options.einfuegemodus(&tool_id);
            let items: Vec<(crate::app::Einfuegemodus, String)> = crate::app::Einfuegemodus::ALL
                .iter()
                .map(|m| (*m, m.label(&tool_id).to_string()))
                .collect();
            let mut picked = None;
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.einfuegemodus"), |ui| {
                picked = panels::pick_combo(ui, &ziel.salt("einfuegemodus"), cur, &items);
            });
            if let Some(m) = picked {
                *app.options.einfuegemodus_mut(&tool_id) = m;
            }
        }
    }

    // ---- Kennzahlen / Geometrie (nur Anzeige) ----
    zeile("geometrie");
    ui.add_space(4.0);
    match &e {
        Some(e) if !ziel.ist_neu() => {
            let geo = panels::geo_rows(e, unit);
            for (name, wert) in geo {
                widgets::field_row(ui, name, |ui| {
                    ui.label(theme::mono(wert.clone()));
                });
            }
            if let EntityKind::DimChain { pts, .. } = &e.kind {
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_basispunkte", &[("wert1", format!("{}", pts.len()))])));
            }
            if let EntityKind::BlockRef { block, .. } = &e.kind {
                let name = app
                    .doc()
                    .block(block)
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| block.clone());
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.block_x", &[("name", format!("{}", name))])));
            }
        }
        _ => {
            ui.label(theme::faint(match art {
                Einfach::Kreis => jjcad_core::i18n::tr("gui.props.radius_und_mittelpunkt_entstehen_aus_den_klicks"),
                Einfach::Bogen => jjcad_core::i18n::tr("gui.props.radius_und_winkel_entstehen_aus_den_klicks"),
                Einfach::Linie => jjcad_core::i18n::tr("gui.props.laenge_und_winkel_entstehen_aus_den_klicks"),
                Einfach::Polylinie | Einfach::Spline | Einfach::Rechteck => {
                    jjcad_core::i18n::tr("gui.props.laenge_flaeche_und_stuetzpunkte_entstehen_aus_den_klicks")
                }
                Einfach::Ellipse => jjcad_core::i18n::tr("gui.props.achsen_entstehen_aus_den_klicks"),
                Einfach::Punkt => jjcad_core::i18n::tr("gui.props.koordinate_entsteht_aus_dem_klick"),
                Einfach::Grundrisskote => jjcad_core::i18n::tr("gui.props.hoehe_steht_im_eigenschaften_panel"),
                Einfach::Schnittkote => {
                    jjcad_core::i18n::tr("gui.props.hoehen_entstehen_aus_der_y_differenz_zum_ersten_punkt")
                }
                Einfach::Block => jjcad_core::i18n::tr("gui.props.blocklage_entsteht_aus_dem_klick"),
                _ => jjcad_core::i18n::tr("gui.props.masse_entstehen_aus_den_klicks"),
            }));
        }
    }

    // ---- Anwenden ----
    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            if let Some(v) = hoehe {
                app.options.elevation_z = v;
            }
            if let Some(v) = drehung {
                app.options.block_rot = v;
            }
            if let Some(v) = faktor {
                app.options.block_scale = v;
            }
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    // Ellipsenbogen: Achsen, Drehlage und beide Parameterwinkel
                    // (T0084). Geht vor der gemeinsamen Schleife, weil hier fünf
                    // Werte zusammen gehören — keine andere Art nutzt sie.
                    if let EntityKind::EllipseArc { rx, ry, rot, a0, a1, .. } = &e.kind {
                        if let Some(v) = halbachse_x {
                            if (v - *rx).abs() > 1e-9 {
                                map.insert("rx".into(), serde_json::json!(unit.from_mm(v)));
                                geaendert = true;
                            }
                        }
                        if let Some(v) = halbachse_y {
                            if (v - *ry).abs() > 1e-9 {
                                map.insert("ry".into(), serde_json::json!(unit.from_mm(v)));
                                geaendert = true;
                            }
                        }
                        if let Some(v) = drehung {
                            if (v - rot.to_degrees()).abs() > 1e-9 {
                                map.insert("rot".into(), serde_json::json!(v));
                                geaendert = true;
                            }
                        }
                        if let Some(v) = startwinkel {
                            if (v - a0.to_degrees()).abs() > 1e-9 {
                                map.insert("a0".into(), serde_json::json!(v));
                                geaendert = true;
                            }
                        }
                        if let Some(v) = endwinkel {
                            if (v - a1.to_degrees()).abs() > 1e-9 {
                                map.insert("a1".into(), serde_json::json!(v));
                                geaendert = true;
                            }
                        }
                    }
                    match (&e.kind, radius, startwinkel, endwinkel, hoehe, drehung, faktor) {
                        (EntityKind::Circle { r, .. }, Some(v), _, _, _, _, _)
                            if (v - *r).abs() > 1e-9 =>
                        {
                            map.insert("r".into(), serde_json::json!(unit.from_mm(v)));
                            geaendert = true;
                        }
                        (
                            EntityKind::Arc { r, a0, a1, .. },
                            Some(rv),
                            Some(w0),
                            Some(w1),
                            _,
                            _,
                            _,
                        ) => {
                            if (rv - *r).abs() > 1e-9 {
                                map.insert("r".into(), serde_json::json!(unit.from_mm(rv)));
                                geaendert = true;
                            }
                            if (w0 - a0.to_degrees()).abs() > 1e-9 {
                                map.insert("a0".into(), serde_json::json!(w0));
                                geaendert = true;
                            }
                            if (w1 - a1.to_degrees()).abs() > 1e-9 {
                                map.insert("a1".into(), serde_json::json!(w1));
                                geaendert = true;
                            }
                        }
                        (EntityKind::ElevationPlan { z, .. }, _, _, _, Some(v), _, _)
                            if (v - *z).abs() > 1e-9 =>
                        {
                            map.insert("z".into(), serde_json::json!(unit.from_mm(v)));
                            geaendert = true;
                        }
                        (EntityKind::BlockRef { rot, scale, .. }, _, _, _, _, Some(rv), Some(sv)) => {
                            if (rv - rot.to_degrees()).abs() > 1e-9 {
                                map.insert("angle".into(), serde_json::json!(rv));
                                geaendert = true;
                            }
                            if (sv - scale.x).abs() > 1e-9 {
                                map.insert("scale".into(), serde_json::json!(sv));
                                geaendert = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Maßlinie verschieben: `offset` in der Eingabeeinheit, der Kern
            // rechnet ihn auf die Achse der Bemaßung um.
            if let (Some(e0), Some(v)) = (&e, abstand) {
                if let Some(alt) = jjcad_core::render::dim_offset(&e0.kind) {
                    if (v - alt).abs() > 1e-6 {
                        params["offset"] = serde_json::json!(unit.from_mm(v));
                        geaendert = true;
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Volumen
// ---------------------------------------------------------------------------

/// Alle Zeilen eines Volumens — für Erzeugen und Bearbeiten dieselben.
pub fn volumen(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let werkzeug = app
        .tool
        .as_ref()
        .map(|t| t.action.clone())
        .unwrap_or_default();

    let (layer, farbe, mut hoehe, mut muster, mut winkel, mut abstand, mut rand, mut fuellfarbe) =
        match &e {
            Some(e) => match &e.kind {
                EntityKind::Volume {
                    pattern,
                    pattern_angle,
                    pattern_scale,
                    border,
                    height,
                    ..
                } => (
                    e.layer,
                    e.style.color,
                    *height,
                    *pattern,
                    pattern_angle.to_degrees(),
                    *pattern_scale,
                    *border,
                    e.style.fill,
                ),
                _ => return,
            },
            None => (
                app.doc().current_layer,
                app.ui.pending.color,
                app.options.volume_height,
                app.options.volume_pattern,
                app.options.volume_angle.to_degrees(),
                app.options.volume_scale,
                app.options.fill_border_for(&werkzeug),
                app.ui.pending.fill,
            ),
        };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    let (staerke, linientyp) = linienwerte(app, &e);

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.volumen"), "volume");
    reihenfolge(app, ui, ziel);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        true,
        jjcad_core::i18n::tr("gui.props.linienfarbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "volume", jjcad_core::i18n::tr("gui.props.volumen"));
    zeile("volumenkopf");

    zeile("hoehe");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
        widgets::length_field(
            ui,
            &mut hoehe,
            unit,
            ui.available_width(),
            widgets::MASS_MIN_MM,
            widgets::MASS_MAX_MM,
        );
    });
    zeile("muster");
    let muster_items: Vec<(jjcad_core::style::FillPattern, String)> =
        jjcad_core::style::FillPattern::ALL
            .iter()
            .map(|c| (*c, c.text().to_string()))
            .collect();
    let mut picked = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.muster"), |ui| {
        picked = panels::pick_combo(ui, &ziel.salt("volumen_muster"), muster, &muster_items);
    });
    if let Some(v) = picked {
        muster = v;
    }
    zeile("winkel");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.winkel"), |ui| {
        ui.add(
            crate::widgets::zahl(&mut winkel, 1.0, widgets::WINKEL_MIN..=widgets::WINKEL_MAX)
                .suffix("°"),
        );
    });
    zeile("abstand");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.abstand"), |ui| {
        widgets::length_field(
            ui,
            &mut abstand,
            unit,
            ui.available_width(),
            widgets::ABSTAND_MIN_MM,
            widgets::ABSTAND_MAX_MM,
        );
    });
    zeile("umrandung");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.umrandung"), |ui| {
        ui.checkbox(&mut rand, "");
    });
    zeile("fuellfarbe");
    let fill_unset = fuellfarbe.is_none();
    let fill_cur = fuellfarbe
        .unwrap_or_else(|| farbe_gezeigt.mix(Color::WHITE, jjcad_core::render::WALL_FILL_MIX));
    let mut picked_fill: Option<Option<Color>> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fuellfarbe"), |ui| {
        picked_fill = panels::color_combo(
            ui,
            &ziel.salt("volumen_farbe"),
            fill_cur,
            fill_unset,
            false,
            jjcad_core::i18n::tr("gui.props.wandton"),
        );
    });
    if let Some(v) = picked_fill {
        fuellfarbe = v;
    }

    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Volume {
                boundary, holes, ..
            } = &e.kind
            {
                let flaeche = jjcad_core::area::area_of(boundary, holes);
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_punkte_flaeche_x", &[("wert1", format!("{}", boundary.len())), ("wert2", format!("{}", panels::area_text(unit, flaeche)))])));
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.volumen_x_m3_hoehe_x", &[("wert1", format!("{:.3}", flaeche * hoehe / 1e9)), ("wert2", format!("{}", unit.format(hoehe, unit.decimals())))])));
            }
        }
        _ => {
            ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.flaeche_und_volumen_erscheinen_am_platzierten_objekt")));
        }
    }

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.volume_height = hoehe;
            app.options.volume_pattern = muster;
            app.options.volume_angle = winkel.to_radians();
            app.options.volume_scale = abstand;
            if !werkzeug.is_empty() {
                *app.options.fill_border_for_mut(&werkzeug) = rand;
            }
            if let Some(v) = picked_fill {
                app.ui.pending.fill = v;
            }
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Volume {
                        height,
                        pattern,
                        pattern_angle,
                        pattern_scale,
                        border,
                        ..
                    } = &e.kind
                    {
                        if (hoehe - *height).abs() > 1e-9 {
                            map.insert("height".into(), serde_json::json!(unit.from_mm(hoehe)));
                            geaendert = true;
                        }
                        if muster != *pattern {
                            map.insert("pattern".into(), serde_json::json!(muster.key()));
                            geaendert = true;
                        }
                        if (winkel - pattern_angle.to_degrees()).abs() > 1e-9 {
                            map.insert("pattern_angle".into(), serde_json::json!(winkel));
                            geaendert = true;
                        }
                        if (abstand - *pattern_scale).abs() > 1e-9 {
                            map.insert(
                                "pattern_scale".into(),
                                serde_json::json!(unit.from_mm(abstand)),
                            );
                            geaendert = true;
                        }
                        if rand != *border {
                            map.insert("border".into(), serde_json::json!(rand));
                            geaendert = true;
                        }
                    }
                    if fuellfarbe != e.style.fill {
                        map.insert(
                            "fill".into(),
                            serde_json::json!(match fuellfarbe {
                                Some(c) => c.to_hex(),
                                None => "none".to_string(),
                            }),
                        );
                        geaendert = true;
                    }
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Treppe
// ---------------------------------------------------------------------------

/// Alle Zeilen einer Treppe — für Erzeugen und Bearbeiten dieselben.
///
/// Länge und Breite entstehen beim Platzieren aus den Klicks (Lauflinie +
/// dritter Klick); sie stehen beim Erzeugen gesperrt da. Höhe, Stufenzahl,
/// Schrittmaßregel und Schnitt lassen sich in beiden Fällen setzen. Die
/// Linien der beiden Treppenteile betreffen nur ein vorhandenes Objekt.
pub fn treppe(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let unit = app.unit();
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());

    let (
        layer,
        farbe,
        mut laenge,
        mut breite,
        mut hoehe,
        stuen,
        mut auto,
        mut schnitt,
    ) = match &e {
        Some(e) => match &e.kind {
            EntityKind::Stair {
                a,
                b,
                width,
                steps,
                height,
                auto_steps,
                cut,
                ..
            } => (
                e.layer,
                e.style.color,
                a.dist(*b),
                *width,
                *height,
                *steps,
                *auto_steps,
                *cut,
            ),
            _ => return,
        },
        None => (
            app.doc().current_layer,
            app.ui.pending.color,
            0.0,
            0.0,
            app.options.stair_height,
            app.options.stair_steps,
            app.options.stair_auto,
            app.options.stair_cut,
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    let (staerke, linientyp) = linienwerte(app, &e);
    // Ohne Länge (beim Erzeugen) gilt die Vorgabe der Action als Rechengrundlage.
    let rechen_laenge = if laenge > 1.0 { laenge } else { 4640.0 };
    let mass = jjcad_core::stair::treppenmass(rechen_laenge, hoehe, stuen, auto);

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.treppe"), "stair");
    reihenfolge(app, ui, ziel);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        true,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    ui.add_space(4.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    section_title(ui, "stair", jjcad_core::i18n::tr("gui.props.treppe"));
    zeile("treppenkopf");

    let gesperrt = e.is_none();
    zeile("laenge");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.laenge"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.laenge"), |ui| {
            widgets::length_field(ui, &mut laenge, unit, 100.0, 100.0, widgets::MASS_MAX_MM);
        });
    }
    zeile("breite");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.breite"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.breite"), |ui| {
            widgets::length_field(ui, &mut breite, unit, 100.0, 100.0, widgets::MASS_MAX_MM);
        });
    }
    zeile("hoehe");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
        widgets::length_field(ui, &mut hoehe, unit, 100.0, 100.0, widgets::MASS_MAX_MM);
    });
    zeile("stufen");
    let mut stufen_neu = mass.stufen;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.stufen"), |ui| {
        ui.add_enabled_ui(!auto, |ui| {
            ui.add(crate::widgets::zahl(
                &mut stufen_neu,
                1.0,
                jjcad_core::stair::STUFEN_MIN..=jjcad_core::stair::STUFEN_MAX,
            ));
        });
    });
    zeile("automatisch");
    let auto_vorher = auto;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.automatisch_2"), |ui| {
        ui.checkbox(&mut auto, jjcad_core::i18n::tr("gui.props.schrittmass"))
            .on_hover_text(jjcad_core::i18n::tr("gui.props.stufenzahl_aus_laenge_und_hoehe_nach_2_s_a_63_cm"));
    });
    if auto_vorher && !auto {
        // Beim Abschalten die berechnete Zahl übernehmen, damit die Treppe
        // nicht springt.
        stufen_neu = mass.stufen;
    }
    zeile("schnitt");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.schnitt"), |ui| {
        ui.checkbox(&mut schnitt, jjcad_core::i18n::tr("gui.props.zwei_laeufe"))
            .on_hover_text(jjcad_core::i18n::tr("gui.props.diagonalen_in_der_laufmitte_sie_trennen_oberen_und_unter"));
    });

    // Schrittmaß als Information — die Zahlen, die auch im Textfeld stehen.
    zeile("schrittmass");
    let cm = |mm: f64, stellen: usize| format!("{:.*} cm", stellen, mm / 10.0).replace(".", ",");
    ui.add_space(4.0);
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.auftritt"), |ui| {
        ui.label(theme::mono(cm(mass.auftritt, 1)));
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.steigung"), |ui| {
        ui.label(theme::mono(cm(mass.steigung, 1)));
    });
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.2_s_a"), |ui| {
        ui.label(theme::mono(cm(mass.regel(), 1)))
            .on_hover_text(jjcad_core::i18n::tr("gui.props.sollwert_nach_schrittmassregel_63_0_cm"));
    });
    if breite > 1.0 {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.auftrittsflaeche"), |ui| {
            ui.label(theme::mono(unit.format_area(mass.auftrittsflaeche(breite), 2)))
                .on_hover_text(jjcad_core::i18n::tr("gui.props.auftritt_treppenbreite"));
        });
    } else {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.auftrittsflaeche"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    }

    // Linien der beiden Teile — nur mit Schnitt gibt es oberen und unteren Teil.
    let mut teil_aenderung: Vec<(String, serde_json::Value)> = Vec::new();
    if schnitt {
        zeile("teile");
        let (unten, oben) = match &e {
            Some(e) => match &e.kind {
                EntityKind::Stair { unten, oben, .. } => (unten.clone(), oben.clone()),
                _ => (None, None),
            },
            None => (None, None),
        };
        let basis = e
            .as_ref()
            .map(|e| {
                let c = e
                    .style
                    .color
                    .unwrap_or_else(|| app.doc().effective_color(e));
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
            .unwrap_or_else(|| {
                (
                    jjcad_core::model::EntityStyle::default(),
                    farbe_gezeigt,
                    staerke,
                    linientyp,
                )
            });
        let (basis_style, basis_farbe, basis_w, basis_lt) = basis;
        ui.add_space(6.0);
        widgets::hline(ui);
        ui.add_space(4.0);
        for (name, praefix, teil) in [("unten", "unten", unten), ("oben", "oben", oben)] {
            let farbe_teil = teil.as_ref().and_then(|t| t.color).or(basis_style.color);
            let mut neue_farbe: Option<Option<Color>> = None;
            widgets::field_row(ui, &jjcad_core::i18n::t_args_owned("gui.props.farbe_x", &[("name", format!("{}", name))]), |ui| {
                ui.add_enabled_ui(!gesperrt, |ui| {
                    neue_farbe = panels::color_combo(
                        ui,
                        &ziel.salt(&format!("stair_col_{praefix}")),
                        farbe_teil.unwrap_or(basis_farbe),
                        farbe_teil.is_none(),
                        false,
                        jjcad_core::i18n::tr("gui.props.vom_layer"),
                    );
                });
            });
            if let Some(v) = neue_farbe {
                teil_aenderung.push((
                    format!("color_{praefix}"),
                    serde_json::json!(match v {
                        None => "bylayer".to_string(),
                        Some(c) => c.to_hex(),
                    }),
                ));
            }
            let lw = teil
                .as_ref()
                .and_then(|t| t.line_weight)
                .or(basis_style.line_weight)
                .unwrap_or(basis_w);
            let mut picked_w: Option<f64> = None;
            widgets::field_row(ui, &jjcad_core::i18n::t_args_owned("gui.props.staerke_x", &[("name", format!("{}", name))]), |ui| {
                ui.add_enabled_ui(!gesperrt, |ui| {
                    picked_w = panels::weight_combo(
                        ui,
                        &ziel.salt(&format!("stair_w_{praefix}")),
                        lw,
                        false,
                    );
                });
            });
            if let Some(v) = picked_w {
                teil_aenderung.push((format!("weight_{praefix}"), serde_json::json!(v)));
            }
            let lt = teil
                .as_ref()
                .and_then(|t| t.line_type)
                .or(basis_style.line_type)
                .unwrap_or(basis_lt);
            let mut picked_lt: Option<jjcad_core::style::LineType> = None;
            widgets::field_row(ui, &jjcad_core::i18n::t_args_owned("gui.props.typ_x", &[("name", format!("{}", name))]), |ui| {
                ui.add_enabled_ui(!gesperrt, |ui| {
                    picked_lt = panels::linetype_combo(
                        ui,
                        &ziel.salt(&format!("stair_lt_{praefix}")),
                        lt,
                        false,
                    );
                });
            });
            if let Some(v) = picked_lt {
                teil_aenderung.push((
                    format!("type_{praefix}"),
                    serde_json::json!(match v {
                        jjcad_core::style::LineType::Solid => "solid",
                        jjcad_core::style::LineType::Dashed => "dashed",
                        jjcad_core::style::LineType::Dotted => "dotted",
                        jjcad_core::style::LineType::DashDot => "dashdot",
                        jjcad_core::style::LineType::Hidden => "hidden",
                    }),
                ));
            }
            ui.add_space(4.0);
        }
    }

    zeile("beschriftung");
    ui.add_space(6.0);
    let klick = ui
        .add_enabled(
            !gesperrt,
            egui::Button::new("Beschriftung erzeugen").min_size(egui::Vec2::new(0.0, 24.0)),
        )
        .on_hover_text(jjcad_core::i18n::tr("gui.props.stufenzahl_und_schrittmass_als_textfeld_neben_die_treppe"))
        .clicked();
    if klick {
        if let Ziel::Auswahl(id) = ziel {
            let ebene = app.doc().current_layer;
            let _ = app
                .session_mut()
                .execute("draw.stair.label", &serde_json::json!({"id": id, "layer": ebene}));
            app.mark_changed();
        }
    }
    if gesperrt {
        ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.am_platzierten_objekt")));
    }

    // ---- Anwenden ----
    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.stair_height = hoehe;
            app.options.stair_steps = stufen_neu;
            app.options.stair_auto = auto;
            app.options.stair_cut = schnitt;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Stair {
                        a: pa,
                        b: pb,
                        width,
                        steps,
                        height,
                        auto_steps,
                        cut,
                        ..
                    } = &e.kind
                    {
                        if (laenge - pa.dist(*pb)).abs() > 1e-9 {
                            map.insert("len".into(), serde_json::json!(unit.from_mm(laenge)));
                            geaendert = true;
                        }
                        if (breite - *width).abs() > 1e-9 {
                            map.insert("width".into(), serde_json::json!(unit.from_mm(breite)));
                            geaendert = true;
                        }
                        if (hoehe - *height).abs() > 1e-9 {
                            map.insert("height".into(), serde_json::json!(unit.from_mm(hoehe)));
                            geaendert = true;
                        }
                        if auto != *auto_steps {
                            map.insert("auto_steps".into(), serde_json::json!(auto));
                            
                            geaendert = true;
                        }
                        if stufen_neu != *steps {
                            map.insert("steps".into(), serde_json::json!(stufen_neu));
                            geaendert = true;
                        }
                        if schnitt != *cut {
                            map.insert("cut".into(), serde_json::json!(schnitt));
                            geaendert = true;
                        }
                    }
                }
                for (k, v) in teil_aenderung {
                    map.insert(k, v);
                    geaendert = true;
                }
            }
            if geaendert {
                setzen(app, &params);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mehrfachauswahl
// ---------------------------------------------------------------------------

/// Eigenschaften einer Mehrfachauswahl.
///
/// Gezeichnet wird **je Objektart** genau der Abschnitt, den dieselbe Funktion
/// auch für ein einzelnes Objekt zeichnet (`wand`, `tuer`, `text`, …). Die
/// angezeigten Werte stammen vom ersten Objekt der Gruppe, das Ziel einer
/// Änderung ist die **ganze Gruppe**: [`setzen`] schickt jedes `modify.set` an
/// alle Kennungen in `App::mehrfach_ziel`.
///
/// Damit gilt jede Eigenschaft, die ein einzelnes Objekt hat — Dicke, Höhe,
/// Schraffur, Füllfarbe, Material, Ausrichtung, Breite, Muster, Stufen,
/// Schriftgröße, Auszeichnung —, ohne eigenen Code auch für die Auswahl. Wer
/// eine Eigenschaft ergänzt, ergänzt sie wie bisher **einmal** im Painter der
/// Art; für die Mehrfachauswahl ist nichts nachzutragen.
///
/// Eine Auswahl über mehrere Arten hinweg zeigt je Art einen Abschnitt;
/// geändert wird immer nur die Gruppe, zu der die Zeile gehört.
pub fn mehrfach(app: &mut App, ui: &mut Ui, ids: &[Id]) {
    let mut gruppen: Vec<(std::mem::Discriminant<EntityKind>, Vec<Id>)> = Vec::new();
    for id in ids {
        let Some(e) = app.doc().entity(*id) else { continue };
        let d = std::mem::discriminant(&e.kind);
        match gruppen.iter_mut().find(|(g, _)| *g == d) {
            Some((_, liste)) => liste.push(*id),
            None => gruppen.push((d, vec![*id])),
        }
    }
    if gruppen.is_empty() {
        return;
    }

    section_title(ui, "select", jjcad_core::i18n::tr("gui.props.mehrfachauswahl"));
    let anzahl: usize = gruppen.iter().map(|(_, l)| l.len()).sum();
    // Bei einer einzigen Art nennt der Kopf des Abschnitts die Anzahl — hier
    // stünde sie sonst zweimal untereinander.
    if gruppen.len() > 1 {
        ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.x_objekte_x_arten", &[("anzahl", format!("{}", anzahl)), ("wert1", format!("{}", gruppen.len()))])));
    }

    let vorher = std::mem::take(&mut app.mehrfach_ziel);
    for (i, (_, liste)) in gruppen.iter().enumerate() {
        if i > 0 {
            ui.add_space(8.0);
            widgets::hline(ui);
            ui.add_space(6.0);
        }
        app.mehrfach_ziel = liste.clone();
        let ziel = Ziel::Auswahl(liste[0]);
        let art = app.doc().entity(liste[0]).map(|e| e.kind.clone());
        match art {
            Some(EntityKind::Text { .. }) => text(app, ui, ziel),
            Some(EntityKind::Wall { .. }) => wand(app, ui, ziel),
            Some(EntityKind::Door { .. }) => tuer(app, ui, ziel),
            Some(EntityKind::Window { .. }) => fenster(app, ui, ziel),
            Some(EntityKind::Fill { .. }) => fuellflaeche(app, ui, ziel),
            Some(EntityKind::Dim { .. }) => bemassung(app, ui, ziel),
            Some(EntityKind::Volume { .. }) => volumen(app, ui, ziel),
            Some(EntityKind::Stair { .. }) => treppe(app, ui, ziel),
            Some(EntityKind::Room { .. }) => raumstempel(app, ui, ziel),
            Some(kind) => {
                if let Some(a) = Einfach::aus_objekt(&kind) {
                    einfach(app, ui, ziel, a)
                }
            }
            None => {}
        }
    }
    app.mehrfach_ziel = vorher;

    // ---- Auswertung über die ganze Auswahl ----
    let ents: Vec<Entity> = ids
        .iter()
        .filter_map(|i| app.doc().entity(*i).cloned())
        .collect();
    let unit = app.unit();
    ui.add_space(6.0);
    widgets::hline(ui);
    ui.add_space(4.0);
    let (mut total_len, mut total_area) = (0.0f64, 0.0f64);
    for e in &ents {
        total_len += crate::panels::entity_length(e);
        total_area += crate::panels::entity_area(e);
    }
    if total_len > 0.0 {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.gesamtlaenge"), |ui| {
            ui.label(theme::mono(unit.format(total_len, unit.decimals())));
        });
    }
    if total_area > 0.0 {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.gesamtflaeche"), |ui| {
            ui.label(theme::mono(crate::panels::area_text(unit, total_area)));
        });
    }

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button(jjcad_core::i18n::tr("gui.props.verschieben")).clicked() {
            app.start_tool("modify.move");
        }
        if ui.button(jjcad_core::i18n::tr("gui.props.kopieren_mit_basispunkt")).clicked() {
            app.start_tool("modify.copy");
        }
        if ui.button(jjcad_core::i18n::tr("gui.props.drehen")).clicked() {
            app.start_tool("modify.rotate");
        }
        if ui.button(jjcad_core::i18n::tr("gui.props.spiegeln")).clicked() {
            app.start_tool("modify.mirror");
        }
    });
}

// ---------------------------------------------------------------------------
// Tests: gleiche Zeilen in beiden Fällen
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jjcad_core::model::{EntityKind, TextAlignH, TextAlignV};
    use jjcad_core::style::Color;
    use serde_json::json;

    /// Zeichnet das Panel einmal und liefert die angemeldeten Zeilen.
    fn zeichne(app: &mut App) -> Vec<&'static str> {
        let ctx = app.ctx.clone();
        protokoll_start();
        let mut einmal = false;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if !einmal {
                    einmal = true;
                    crate::panels::properties(app, ui);
                }
            });
        });
        protokoll()
    }

    fn text_app() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1.0, 1.0], "text": "Hallo", "height": 0.25}),
            )
            .expect("Text anlegen");
        let id = app.doc().ents().last().expect("Text fehlt").id;
        (app, id)
    }

    #[test]
    fn text_hat_in_beiden_faellen_dieselben_zeilen() {
        let (mut app, id) = text_app();

        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);
        assert!(auswahl.contains(&"inhalt"), "Auswahl ohne Inhalt: {auswahl:?}");

        app.session_mut().selection.clear();
        app.start_tool("draw.text");
        let neu = zeichne(&mut app);
        assert!(app.tool.is_some(), "Werkzeug nicht gesetzt");

        assert_eq!(
            neu, auswahl,
            "Erzeugen und Bearbeiten zeigen unterschiedliche Zeilen"
        );
    }

    #[test]
    fn wand_hat_in_beiden_faellen_dieselben_zeilen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "draw.wall",
                &json!({"a": [0.0, 0.0], "b": [3.0, 0.0], "thickness": 0.24}),
            )
            .expect("Wand anlegen");
        let id = app.doc().ents().last().expect("Wand fehlt").id;

        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);

        app.session_mut().selection.clear();
        app.start_tool("draw.wall");
        let neu = zeichne(&mut app);

        assert_eq!(neu, auswahl, "Wand: Zeilen weichen ab");
    }

    fn wand_mit_oeffnung() -> (App, Id, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "draw.wall",
                &json!({"a": [0.0, 0.0], "b": [4.0, 0.0], "thickness": 0.24}),
            )
            .expect("Wand");
        let wand = app.doc().ents().last().expect("Wand fehlt").id;
        app.session_mut()
            .execute(
                "draw.window",
                &json!({"center": [2.0, 0.0], "angle": 0.0, "host": [wand],
                        "width": 1.2, "sill": 0.9, "height": 1.4}),
            )
            .expect("Fenster");
        let fenster = app.doc().ents().last().expect("Fenster fehlt").id;
        app.session_mut()
            .execute(
                "draw.door",
                &json!({"center": [1.0, 0.0], "angle": 0.0, "host": [wand], "width": 0.9}),
            )
            .expect("Tür");
        let tuer = app.doc().ents().last().expect("Tür fehlt").id;
        (app, tuer, fenster)
    }

    #[test]
    fn tuer_hat_in_beiden_faellen_dieselben_zeilen() {
        let (mut app, tuer, _) = wand_mit_oeffnung();
        app.session_mut().selection = vec![tuer];
        let auswahl = zeichne(&mut app);
        assert!(auswahl.contains(&"anschlag"), "Tür ohne Anschlag: {auswahl:?}");
        assert!(
            auswahl.contains(&"basispunkt"),
            "Tür ohne Basispunkt-Zeile: {auswahl:?}"
        );

        app.session_mut().selection.clear();
        app.start_tool("draw.door");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Tür: Zeilen weichen ab");
    }

    #[test]
    fn fenster_hat_in_beiden_faellen_dieselben_zeilen() {
        let (mut app, _, fenster) = wand_mit_oeffnung();
        app.session_mut().selection = vec![fenster];
        let auswahl = zeichne(&mut app);
        assert!(auswahl.contains(&"bruestung"), "Fenster ohne Brüstung: {auswahl:?}");

        app.session_mut().selection.clear();
        app.start_tool("draw.window");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Fenster: Zeilen weichen ab");
    }

    #[test]
    fn fuellflaeche_hat_in_beiden_faellen_dieselben_zeilen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let layer = app.doc().current_layer;
        let id = app.doc_mut().new_id();
        let e = Entity::new(
            id,
            layer,
            EntityKind::Fill {
                boundary: vec![
                    jjcad_core::geom::Vec2::ZERO,
                    jjcad_core::geom::v(2000.0, 0.0),
                    jjcad_core::geom::v(2000.0, 2000.0),
                ],
                holes: Vec::new(),
                pattern: jjcad_core::style::FillPattern::Lines,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: false,
            },
        );
        app.doc_mut().add_entity(e);

        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);
        assert!(auswahl.contains(&"umrandung"), "Fläche ohne Umrandung: {auswahl:?}");

        app.session_mut().selection.clear();
        app.start_tool("draw.fill");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Füllfläche: Zeilen weichen ab");
    }

    #[test]
    fn bemassung_hat_in_beiden_faellen_dieselben_zeilen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "dim.linear",
                &json!({"a": [0.0, 0.0], "b": [3.0, 0.0], "line_pos": [1.5, -0.4]}),
            )
            .expect("Maß anlegen");
        let id = app.doc().ents().last().expect("Maß fehlt").id;

        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);
        assert!(auswahl.contains(&"masstext"), "Maß ohne Maßtext: {auswahl:?}");

        app.session_mut().selection.clear();
        app.start_tool("dim.linear");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Bemaßung: Zeilen weichen ab");
    }

    /// Die Maßkette hat im Panel einen **Abstand** (Lage der Maßlinie) und
    /// zeigt ihn beim Platzieren wie bei der Auswahl (T0055).
    #[test]
    fn masskette_panel_zeigt_den_abstand() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "dim.chain",
                &json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                        "line_pos": [1.0, -0.4], "axis": "x"}),
            )
            .expect("Maßkette anlegen");
        let id = app.doc().ents().last().expect("Maßkette fehlt").id;

        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);
        assert!(auswahl.contains(&"abstand"), "Maßkette ohne Abstand: {auswahl:?}");

        app.session_mut().selection.clear();
        app.start_tool("dim.chain");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Maßkette: Zeilen weichen ab");
    }

    /// Ein laufendes Erstellwerkzeug verdrängt die Auswahl aus dem Panel —
    /// sonst stünden zwei Eigenschafts-Blöcke übereinander (gemeldet: Text
    /// beim Erzeugen, Wand darunter).
    #[test]
    fn werkzeug_verdraengt_die_auswahl_im_panel() {
        let (mut app, id) = text_app();
        app.session_mut().selection = vec![id];
        app.start_tool("draw.wall");
        let zeilen = zeichne(&mut app);
        assert!(
            zeilen.contains(&"wandkopf"),
            "Werkzeug-Panel fehlt: {zeilen:?}"
        );
        assert!(
            !zeilen.contains(&"textkopf"),
            "Auswahl-Panel steht weiterhin darunter: {zeilen:?}"
        );
    }

    /// Beim Tippen in der Zeichenfläche zeigt das Panel dieselben Zeilen wie
    /// beim Bearbeiten — kein zweites, kürzeres Textfenster mehr.
    #[test]
    fn text_im_zeichenbereich_zeigt_dasselbe_panel() {
        let (mut app, id) = text_app();
        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);

        app.session_mut().selection.clear();
        assert!(
            crate::textedit::start_entitaet(&mut app, id),
            "Texteditor startet nicht"
        );
        assert!(app.ui.texteingabe.is_some(), "keine laufende Eingabe");
        let beim_tippen = zeichne(&mut app);
        assert_eq!(
            beim_tippen, auswahl,
            "Tippen zeigt ein anderes Fenster als Bearbeiten"
        );
    }

    /// Auch für einen **neuen** Text gilt das Erzeugen-Panel.
    #[test]
    fn neuer_text_im_zeichenbereich_zeigt_das_erzeugen_panel() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        crate::textedit::start(&mut app, jjcad_core::geom::Vec2::new(1000.0, 1000.0));
        let beim_tippen = zeichne(&mut app);

        app.ui.texteingabe = None;
        app.start_tool("draw.text");
        let erzeugen = zeichne(&mut app);
        assert_eq!(beim_tippen, erzeugen, "Neuer Text: andere Zeilen beim Tippen");
    }

    /// Alle übrigen Erzeuger gegen ihr Objekt: gleiche Zeilen, gleiche
    /// Reihenfolge. Das ist die Tabelle, die den Umbau abgesichert hat.
    #[test]
    fn alle_erzeuger_zeigen_dieselben_zeilen_wie_das_objekt() {
        use jjcad_core::geom::v;
        let faelle: Vec<(&str, EntityKind)> = vec![
            (
                "draw.line",
                EntityKind::Line {
                    a: jjcad_core::geom::Vec2::ZERO,
                    b: v(1000.0, 0.0),
                },
            ),
            (
                "draw.polyline",
                EntityKind::Polyline {
                    verts: vec![
                        jjcad_core::geom::Vertex::new(jjcad_core::geom::Vec2::ZERO),
                        jjcad_core::geom::Vertex::new(v(1000.0, 0.0)),
                    ],
                    closed: false,
                },
            ),
            (
                "draw.spline",
                EntityKind::Spline {
                    verts: vec![
                        jjcad_core::geom::Vertex::new(jjcad_core::geom::Vec2::ZERO),
                        jjcad_core::geom::Vertex::new(v(1000.0, 0.0)),
                    ],
                    closed: false,
                },
            ),
            (
                // Ein Rechteck ist ein geschlossener Streckenzug.
                "draw.rect",
                EntityKind::Polyline {
                    verts: vec![
                        jjcad_core::geom::Vertex::new(jjcad_core::geom::Vec2::ZERO),
                        jjcad_core::geom::Vertex::new(v(1000.0, 0.0)),
                        jjcad_core::geom::Vertex::new(v(1000.0, 500.0)),
                    ],
                    closed: true,
                },
            ),
            (
                "draw.circle",
                EntityKind::Circle {
                    c: jjcad_core::geom::Vec2::ZERO,
                    r: 500.0,
                },
            ),
            (
                "draw.arc",
                EntityKind::Arc {
                    c: jjcad_core::geom::Vec2::ZERO,
                    r: 500.0,
                    a0: 0.0,
                    a1: 1.5,
                },
            ),
            (
                "draw.ellipse",
                EntityKind::Ellipse {
                    c: jjcad_core::geom::Vec2::ZERO,
                    rx: 800.0,
                    ry: 400.0,
                    rot: 0.0,
                },
            ),
            (
                "draw.point",
                EntityKind::Point {
                    p: jjcad_core::geom::Vec2::ZERO,
                },
            ),
            (
                "dim.chain",
                EntityKind::DimChain {
                    pts: vec![jjcad_core::geom::Vec2::ZERO, v(1000.0, 0.0)],
                    line_pos: v(500.0, -300.0),
                    axis: DimAxis::X,
                },
            ),
            (
                "dim.elevation.plan",
                EntityKind::ElevationPlan {
                    p: jjcad_core::geom::Vec2::ZERO,
                    z: 1200.0,
                },
            ),
            (
                "dim.elevation.section",
                EntityKind::ElevationSection {
                    pts: vec![jjcad_core::geom::Vec2::ZERO, v(0.0, 1200.0)],
                },
            ),
            (
                "dim.room",
                EntityKind::Room {
                    boundary: vec![
                        jjcad_core::geom::Vec2::ZERO,
                        v(4000.0, 0.0),
                        v(4000.0, 3000.0),
                        v(0.0, 3000.0),
                    ],
                    label: v(2000.0, 1500.0),
                    height: 250.0,
                    heading: "Zimmer".into(),
                },
            ),
            (
                "block.insert",
                EntityKind::BlockRef {
                    block: "Test".into(),
                    pos: jjcad_core::geom::Vec2::ZERO,
                    rot: 0.0,
                    scale: v(1.0, 1.0),
                    mirror: false,
                },
            ),
            (
                "draw.volume",
                EntityKind::Volume {
                    boundary: vec![
                        jjcad_core::geom::Vec2::ZERO,
                        v(2000.0, 0.0),
                        v(2000.0, 2000.0),
                    ],
                    holes: Vec::new(),
                    pattern: jjcad_core::style::FillPattern::Solid,
                    pattern_angle: 0.0,
                    pattern_scale: 100.0,
                    border: true,
                    height: 2750.0,
                },
            ),
            (
                "draw.stair",
                EntityKind::Stair {
                    a: jjcad_core::geom::Vec2::ZERO,
                    b: v(4640.0, 0.0),
                    width: 1000.0,
                    steps: 16,
                    height: 2750.0,
                    auto_steps: true,
                    cut: true,
                    unten: None,
                    oben: None,
                },
            ),
        ];

        for (tool, kind) in faelle {
            let mut app = App::with_ctx(egui::Context::default());
            app.session_mut()
                .execute("doc.settings", &json!({ "input_unit": "m" }))
                .expect("Einheit");
            let layer = app.doc().current_layer;
            let id = app.doc_mut().new_id();
            app.doc_mut().add_entity(Entity::new(id, layer, kind));

            app.session_mut().selection = vec![id];
            let auswahl = zeichne(&mut app);

            app.session_mut().selection.clear();
            app.start_tool(tool);
            assert!(app.tool.is_some(), "{tool}: Werkzeug startet nicht");
            let neu = zeichne(&mut app);

            assert_eq!(neu, auswahl, "{tool}: Zeilen weichen vom Objekt ab");
        }
    }

    #[test]
    fn neue_drehung_gilt_fuer_den_naechsten_text() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.start_tool("draw.text");
        zeichne(&mut app);
        // Zeile von Hand setzen: das Panel schreibt sie in die Vorgaben.
        app.options.text_angle = 30.0;
        app.options.text_height = 0.4;
        app.options.text_font = TextFont::default();
        assert!((app.options.text_angle - 30.0).abs() < 1e-9);

        // Der erzeugte Text übernimmt die Vorgaben der Action.
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [0.0, 0.0], "text": "X", "height": 0.4, "angle": 30.0}),
            )
            .expect("Text");
        match &app.doc().ents().last().unwrap().kind {
            EntityKind::Text { angle, .. } => {
                assert!((angle.to_degrees() - 30.0).abs() < 1e-6, "{angle}");
            }
            k => panic!("kein Text: {k:?}"),
        }
    }

    /// Wächter: **jede** Objektart hat ihr Panel in `props`.
    ///
    /// Der Test zählt alle `EntityKind`-Zweige auf. Kommt ein neuer dazu, lässt
    /// er sich nicht kompilieren, bis er hier steht — und dann verlangt die
    /// Zuordnung unten einen Renderer in dieser Datei. Das ist die Sperre
    /// gegen „neues Objekt, neues Panel, neue Unterschiede“.
    #[test]
    fn jede_objektart_hat_ein_panel() {
        /// Gibt es für diese Art eine Zeichnung in `props`?
        fn abgedeckt(kind: &EntityKind) -> bool {
            matches!(
                kind,
                EntityKind::Text { .. }
                    | EntityKind::Wall { .. }
                    | EntityKind::Door { .. }
                    | EntityKind::Window { .. }
                    | EntityKind::Fill { .. }
                    | EntityKind::Dim { .. }
                    | EntityKind::Volume { .. }
                    | EntityKind::Stair { .. }
                    | EntityKind::Room { .. }
            ) || Einfach::aus_objekt(kind).is_some()
        }

        let arten = [
            EntityKind::Point {
                p: jjcad_core::geom::Vec2::ZERO,
            },
            EntityKind::Line {
                a: jjcad_core::geom::Vec2::ZERO,
                b: jjcad_core::geom::Vec2::ZERO,
            },
            EntityKind::Polyline {
                verts: vec![],
                closed: false,
            },
            EntityKind::Spline {
                verts: vec![],
                closed: false,
            },
            EntityKind::Circle {
                c: jjcad_core::geom::Vec2::ZERO,
                r: 1.0,
            },
            EntityKind::Arc {
                c: jjcad_core::geom::Vec2::ZERO,
                r: 1.0,
                a0: 0.0,
                a1: 1.0,
            },
            EntityKind::Ellipse {
                c: jjcad_core::geom::Vec2::ZERO,
                rx: 1.0,
                ry: 1.0,
                rot: 0.0,
            },
            EntityKind::EllipseArc {
                c: jjcad_core::geom::Vec2::ZERO,
                rx: 1000.0,
                ry: 500.0,
                rot: 0.0,
                a0: 0.0,
                a1: 1.0,
            },
            EntityKind::Room {
                boundary: vec![],
                label: jjcad_core::geom::Vec2::ZERO,
                height: 250.0,
                heading: "Zimmer".into(),
            },
            EntityKind::Text {
                p: jjcad_core::geom::Vec2::ZERO,
                text: String::new(),
                height: 1.0,
                angle: 0.0,
                align: TextAlignH::Left,
                valign: TextAlignV::Middle,
                bold: false,
                italic: false,
                underline: false,
                superscript: false,
                runs: Vec::new(),
                font: TextFont::default(),
            },
            EntityKind::Fill {
                boundary: vec![],
                holes: vec![],
                pattern: jjcad_core::style::FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: false,
            },
            EntityKind::Volume {
                boundary: vec![],
                holes: vec![],
                pattern: jjcad_core::style::FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
                height: 2750.0,
            },
            EntityKind::Wall {
                a: jjcad_core::geom::Vec2::ZERO,
                b: jjcad_core::geom::Vec2::ZERO,
                thickness: 240.0,
                height: 2750.0,
                hatch: None,
                align: jjcad_core::style::WallAlign::Center,
                material: String::new(),
            },
            EntityKind::Door {
                center: jjcad_core::geom::Vec2::ZERO,
                angle: 0.0,
                width: 900.0,
                kind: DoorKind::Hinge,
                hinge: 1.0,
                swing_in: true,
                align: jjcad_core::style::WallAlign::Center,
                host: None,
                at: None,
                base: DoorBase::Left,
            },
            EntityKind::Window {
                center: jjcad_core::geom::Vec2::ZERO,
                angle: 0.0,
                width: 1200.0,
                align: jjcad_core::style::WallAlign::Center,
                sill: 900.0,
                height: 1400.0,
                host: None,
                at: None,
            },
            EntityKind::Stair {
                a: jjcad_core::geom::Vec2::ZERO,
                b: jjcad_core::geom::v(4640.0, 0.0),
                width: 1000.0,
                steps: 16,
                height: 2750.0,
                auto_steps: true,
                cut: true,
                unten: None,
                oben: None,
            },
            EntityKind::Dim {
                a: jjcad_core::geom::Vec2::ZERO,
                b: jjcad_core::geom::v(1000.0, 0.0),
                line_pos: jjcad_core::geom::v(500.0, -300.0),
                axis: DimAxis::X,
                override_text: None,
            },
            EntityKind::DimChain {
                pts: vec![],
                line_pos: jjcad_core::geom::Vec2::ZERO,
                axis: DimAxis::X,
            },
            EntityKind::ElevationPlan {
                p: jjcad_core::geom::Vec2::ZERO,
                z: 0.0,
            },
            EntityKind::ElevationSection {
                pts: vec![jjcad_core::geom::Vec2::ZERO],
            },
            EntityKind::BlockRef {
                block: "Test".into(),
                pos: jjcad_core::geom::Vec2::ZERO,
                rot: 0.0,
                scale: jjcad_core::geom::v(1.0, 1.0),
                mirror: false,
            },
        ];
        for k in &arten {
            let name = k.type_name();
            assert!(
                abgedeckt(k),
                "Objektart '{name}' hat kein Panel in props.rs"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Mehrfachauswahl (T0069)
    // -----------------------------------------------------------------------

    /// App mit zwei Wänden unterschiedlicher Dicke (in Metern gezeichnet,
    /// gespeichert in mm) — der Fall „mehrere Wände, eine Dicke für alle“.
    fn zwei_waende() -> (App, Id, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "draw.wall",
                &json!({"a": [0.0, 0.0], "b": [3.0, 0.0], "thickness": 0.24,
                        "hatch": "solid"}),
            )
            .expect("Wand 1");
        let a = app.doc().ents().last().expect("Wand 1 fehlt").id;
        app.session_mut()
            .execute(
                "draw.wall",
                &json!({"a": [0.0, 2.0], "b": [4.0, 2.0], "thickness": 0.365,
                        "hatch": "insulation"}),
            )
            .expect("Wand 2");
        let b = app.doc().ents().last().expect("Wand 2 fehlt").id;
        (app, a, b)
    }

    fn wand_dicke(app: &App, id: Id) -> f64 {
        match &app.doc().entity(id).expect("Wand").kind {
            EntityKind::Wall { thickness, .. } => *thickness,
            k => panic!("keine Wand: {k:?}"),
        }
    }

    fn wand_schraffur(app: &App, id: Id) -> Option<jjcad_core::style::FillPattern> {
        match &app.doc().entity(id).expect("Wand").kind {
            EntityKind::Wall { hatch, .. } => *hatch,
            k => panic!("keine Wand: {k:?}"),
        }
    }

    /// Eine Änderung aus dem Panel gilt für **alle** Objekte der Auswahl —
    /// nicht nur für das Objekt, dessen Werte angezeigt werden.
    #[test]
    fn mehrfachauswahl_aendert_alle_waende() {
        let (mut app, a, b) = zwei_waende();
        app.mehrfach_ziel = vec![a, b];
        setzen(
            &mut app,
            &json!({"ids": [a], "thickness": 0.30, "hatch": "insulation"}),
        );
        assert!((wand_dicke(&app, a) - 300.0).abs() < 1e-6, "Wand 1: {}", wand_dicke(&app, a));
        assert!((wand_dicke(&app, b) - 300.0).abs() < 1e-6, "Wand 2: {}", wand_dicke(&app, b));
        assert_eq!(
            wand_schraffur(&app, a),
            Some(jjcad_core::style::FillPattern::Insulation)
        );
        assert_eq!(
            wand_schraffur(&app, b),
            Some(jjcad_core::style::FillPattern::Insulation)
        );
    }

    /// Gegenprobe: ohne laufende Mehrfachauswahl bleibt es beim einen Objekt.
    #[test]
    fn ohne_mehrfachauswahl_aendert_sich_nur_das_objekt() {
        let (mut app, a, b) = zwei_waende();
        setzen(&mut app, &json!({"ids": [a], "thickness": 0.30}));
        assert!((wand_dicke(&app, a) - 300.0).abs() < 1e-6);
        assert!((wand_dicke(&app, b) - 365.0).abs() < 1e-6, "Wand 2: {}", wand_dicke(&app, b));
    }

    /// Das Panel der Mehrfachauswahl zeichnet **dieselben Zeilen** wie das
    /// Panel des einzelnen Objekts — für jede Objektart. Damit ist jede
    /// Eigenschaft, die ein Objekt hat, auch in der Auswahl bedienbar; ein
    /// zweiter Maler existiert nicht.
    #[test]
    fn mehrfachauswahl_zeigt_dieselben_zeilen_wie_das_objekt() {
        use jjcad_core::geom::{v, Vec2, Vertex};
        let arten: Vec<EntityKind> = vec![
            EntityKind::Wall {
                a: Vec2::ZERO,
                b: v(3000.0, 0.0),
                thickness: 240.0,
                height: 2500.0,
                align: jjcad_core::style::WallAlign::Center,
                hatch: None,
                material: String::new(),
            },
            EntityKind::Line { a: Vec2::ZERO, b: v(1000.0, 0.0) },
            EntityKind::Polyline {
                verts: vec![Vertex::new(Vec2::ZERO), Vertex::new(v(1000.0, 0.0))],
                closed: false,
            },
            EntityKind::Spline {
                verts: vec![Vertex::new(Vec2::ZERO), Vertex::new(v(1000.0, 0.0))],
                closed: false,
            },
            EntityKind::Circle { c: Vec2::ZERO, r: 500.0 },
            EntityKind::Arc { c: Vec2::ZERO, r: 500.0, a0: 0.0, a1: 1.5 },
            EntityKind::Ellipse { c: Vec2::ZERO, rx: 800.0, ry: 400.0, rot: 0.0 },
            EntityKind::Point { p: Vec2::ZERO },
            EntityKind::Text {
                p: Vec2::ZERO,
                text: "Hallo".into(),
                height: 250.0,
                angle: 0.0,
                align: TextAlignH::Left,
                valign: TextAlignV::Top,
                bold: false,
                italic: false,
                underline: false,
                superscript: false,
                runs: Vec::new(),
                font: TextFont::default(),
            },
            EntityKind::Fill {
                boundary: vec![Vec2::ZERO, v(1000.0, 0.0), v(1000.0, 1000.0)],
                holes: Vec::new(),
                pattern: jjcad_core::style::FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
            },
            EntityKind::Volume {
                boundary: vec![Vec2::ZERO, v(2000.0, 0.0), v(2000.0, 2000.0)],
                holes: Vec::new(),
                pattern: jjcad_core::style::FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
                height: 2750.0,
            },
            EntityKind::Stair {
                a: Vec2::ZERO,
                b: v(4640.0, 0.0),
                width: 1000.0,
                steps: 16,
                height: 2750.0,
                auto_steps: true,
                cut: true,
                unten: None,
                oben: None,
            },
        ];

        for (i, kind) in arten.into_iter().enumerate() {
            let mut app = App::with_ctx(egui::Context::default());
            app.session_mut()
                .execute("doc.settings", &json!({ "input_unit": "m" }))
                .expect("Einheit");
            let layer = app.doc().current_layer;
            let mut ids = Vec::new();
            for _ in 0..2 {
                let id = app.doc_mut().new_id();
                app.doc_mut().add_entity(Entity::new(id, layer, kind.clone()));
                ids.push(id);
            }

            // Einzelnes Objekt: sein eigenes Panel.
            app.session_mut().selection = vec![ids[0]];
            let einzeln = zeichne(&mut app);

            // Beide: der Abschnitt derselben Art in der Mehrfachauswahl.
            app.session_mut().selection = ids.clone();
            let mehrfach_zeilen = zeichne(&mut app);

            assert!(
                !einzeln.is_empty(),
                "Fall {i}: das Objekt hat gar keine Zeilen"
            );
            assert_eq!(
                mehrfach_zeilen, einzeln,
                "Fall {i} ({kind:?}): Mehrfachauswahl zeigt andere Zeilen als das Objekt"
            );
        }
    }

    /// Auswahl über mehrere Arten: je Art ein Abschnitt — die Zeilen beider
    /// Arten stehen im Panel, geändert wird je Gruppe.
    #[test]
    fn gemischte_auswahl_zeigt_je_art_einen_abschnitt() {
        let (mut app, a, b) = zwei_waende();
        let line = app
            .session_mut()
            .execute(
                "draw.line",
                &json!({"a": [0.0, 5.0], "b": [2.0, 5.0]}),
            )
            .expect("Linie")["created"][0]
            .as_u64()
            .expect("Linien-Id");
        app.session_mut().selection = vec![a, b, line];
        let zeilen = zeichne(&mut app);
        // Wandzeilen …
        assert!(zeilen.contains(&"dicke"), "Dicke fehlt: {zeilen:?}");
        assert!(zeilen.contains(&"schraffur"), "Schraffur fehlt: {zeilen:?}");
        // … und die Zeilen der Linie (aus `Einfach`, hier `geometrie`).
        assert!(
            zeilen.contains(&"geometrie"),
            "die Linie hat in der gemischten Auswahl keinen Abschnitt: {zeilen:?}"
        );
        // Der Abschnitt der Wand steht genau einmal (eine Gruppe), der der
        // Linie genau einmal — nicht doppelt.
        assert_eq!(
            zeilen.iter().filter(|z| **z == "dicke").count(),
            1,
            "Wandabschnitt doppelt: {zeilen:?}"
        );
    }

    /// Auch die gemeinsamen Zeilen (Farbe, Stärke, Linientyp) laufen über
    /// `setzen` — eine Stelle für alle Eigenschaften.
    #[test]
    fn gemeinsame_zeilen_aendern_alle_objekte() {
        let (mut app, a, b) = zwei_waende();
        app.mehrfach_ziel = vec![a, b];
        let mut aenderung = Gemein::default();
        aenderung.farbe = Some(Some(Color::rgb(200, 30, 30)));
        aenderung.staerke = Some(0.5);
        gemein_nach_objekt(&mut app, a, &aenderung);
        for id in [a, b] {
            let e = app.doc().entity(id).expect("Wand");
            assert_eq!(e.style.color, Some(Color::rgb(200, 30, 30)), "#{id}");
            assert_eq!(e.style.line_weight, Some(0.5), "#{id}");
        }
    }

    /// Das Ziel der Mehrfachauswahl gilt nur für das Bild, in dem das Panel
    /// gezeichnet wurde — danach schreibt jede Zeile wieder an ihr Objekt.
    #[test]
    fn mehrfachziel_wird_nach_dem_zeichnen_zurueckgesetzt() {
        let (mut app, a, b) = zwei_waende();
        app.session_mut().selection = vec![a, b];
        let _ = zeichne(&mut app);
        assert!(
            app.mehrfach_ziel.is_empty(),
            "mehrfach_ziel bleibt stehen: {:?}",
            app.mehrfach_ziel
        );
    }

    /// Ende-zu-Ende: ein **Zug am Feld „Dicke“** im Panel der
    /// Mehrfachauswahl setzt die Dicke bei **beiden** Wänden. Der Weg ist
    /// derselbe wie beim Nutzer: egui-Ereignisse auf das Feld, der Maler
    /// schickt `modify.set` über `setzen`.
    #[test]
    fn dickezug_im_panel_aendert_alle_ausgewaehlten_waende() {
        let (mut app, a, b) = zwei_waende();
        app.session_mut().selection = vec![a, b];
        let ctx = app.ctx.clone();
        let bild = |app: &mut App,
                    ereignisse: Vec<egui::Event>,
                    zeit: f64|
         -> egui::FullOutput {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1200.0, 950.0),
                )),
                time: Some(zeit),
                predicted_dt: 0.016,
                events: ereignisse,
                ..Default::default()
            };
            ctx.run(raw, |ctx| {
                egui::SidePanel::right("eigenschaften")
                    .exact_width(280.0)
                    .show(ctx, |ui| crate::panels::properties(app, ui));
            })
        };

        // Erstes Bild: Position des Dicke-Feldes aus dem gezeichneten Wert.
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = out
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text().trim() == "0,24 m" => Some(
                    t.pos + egui::Vec2::new(t.galley.size().x / 2.0, t.galley.size().y / 2.0),
                ),
                _ => None,
            })
            .unwrap_or_else(|| {
                let texte: Vec<String> = out
                    .shapes
                    .iter()
                    .filter_map(|cs| match &cs.shape {
                        egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                        _ => None,
                    })
                    .collect();
                panic!("Feld „Dicke“ nicht im Panel gefunden: {texte:?}")
            });

        let mitte = feld;
        let ziel = mitte + egui::Vec2::new(40.0, 0.0);
        let knopf = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        for (i, ev) in vec![
            vec![egui::Event::PointerMoved(mitte)],
            vec![knopf(mitte, true)],
            vec![egui::Event::PointerMoved(ziel)],
            vec![knopf(ziel, false)],
            Vec::new(),
        ]
        .into_iter()
        .enumerate()
        {
            let _ = bild(&mut app, ev, 1.1 + i as f64 * 0.1);
        }

        let da = wand_dicke(&app, a);
        let db = wand_dicke(&app, b);
        assert!(
            da > 250.0,
            "der Zug am Feld hat die Dicke nicht verändert: {da}"
        );
        assert!(
            (da - db).abs() < 1e-6,
            "die beiden Wände haben verschiedene Dicken: {da} / {db}"
        );
    }

    /// Weichen die Werte ab, sagt das Panel es — `*variiert*`.
    #[test]
    fn abweichende_werte_werden_als_variiert_gezeigt() {
        let (mut app, a, b) = zwei_waende();
        app.session_mut().selection = vec![a, b];
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1200.0, 950.0),
            )),
            time: Some(1.0),
            predicted_dt: 0.016,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| crate::panels::properties(&mut app, ui));
        });
        let texte: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(
            texte.iter().any(|t| t.contains("*variiert*")),
            "kein *variiert* bei zwei verschiedenen Wänden: {texte:?}"
        );
        assert!(
            texte.iter().any(|t| t.contains("2 Objekte")),
            "die Anzahl der Auswahl fehlt: {texte:?}"
        );
    }
    /// Zwei gleiche Füllflächen — etwa die zwei Teile eines Schnitts — zeigen
    /// **kein** „*variiert*“ in der Stärke. Der Vergleich läuft gegen das erste
    /// Objekt der Gruppe, nicht gegen den angezeigten Vorgabewert 0,35 mm
    /// (T0090).
    #[test]
    fn zwei_gleiche_flaechen_zeigen_kein_variiert_in_der_staerke() {
        let mut app = App::with_ctx(egui::Context::default());
        let mut ids = Vec::new();
        for pts in [
            [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
            [[4.0, 0.0], [8.0, 0.0], [8.0, 3.0], [4.0, 3.0]],
        ] {
            let v = app
                .session_mut()
                .execute("draw.fill.points", &serde_json::json!({"pts": pts}))
                .expect("Füllfläche");
            ids.push(v["created"][0].as_u64().expect("Id"));
        }
        app.session_mut().selection = ids;
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1200.0, 950.0),
            )),
            time: Some(1.0),
            predicted_dt: 0.016,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| crate::panels::properties(&mut app, ui));
        });
        let texte: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        // Einmal steht „*variiert*“ in der Kopfzeile (die Umgrenzungen sind
        // verschieden) — ein zweites Mal wäre die Stärkezeile.
        let anzahl = texte.iter().filter(|t| t.contains("*variiert*")).count();
        assert_eq!(
            anzahl, 1,
            "Stärke meldet *variiert* bei zwei gleichen Flächen: {texte:?}"
        );
    }

}
