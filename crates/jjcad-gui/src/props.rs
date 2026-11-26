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
//! # Gliederung
//!
//! Ganz oben steht der Objektkopf ([`kopf`]): Symbol und Bezeichnung, darunter
//! die Herkunft (Nummer und Layer) bzw. „gilt für das nächste Element“. Die
//! Bezeichnung erscheint damit **genau einmal** im Panel. Darunter gliedern
//! zwei Gruppen mit eigener Überschrift ([`abschnitt`]):
//!
//! * **Allgemein** — die Werte, die jedes Objekt hat (Reihenfolge, Layer,
//!   Farbe, Stärke, Linientyp).
//! * die **objekteigene Gruppe**, benannt nach ihrem Inhalt („Wandaufbau“,
//!   „Füllung“, „Maß“, „Geometrie“ …), nie nach dem Objekt selbst.
//!
//! Der Gruppenname steht als Schlüssel in `lang/de.toml` unter `gui.props`
//! (z. B. `gui.props.wandaufbau`); eine neue Objektart bringt ihren eigenen
//! mit. `tests::kopf_steht_oben_und_die_gruppen_haben_eigene_namen` prüft für
//! jede Art, dass der Kopf oben und genau einmal steht und die Gruppen anders
//! heißen.
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
    BlockGroesse, DimAxis, DoorBase, DoorKind, ElevationTextPos, Entity, EntityKind, Id, TextFont,
    TextRun,
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

/// Objektkopf: Symbol und Bezeichnung **ganz oben**, darunter die Herkunft
/// (Nummer und Layer) bzw. wofür die Werte gelten.
///
/// Die Bezeichnung steht damit genau einmal über dem Panel; die Gruppen
/// darunter nennen, was sie zeigen („Allgemein“ und die objekteigene Gruppe).
/// Vorher trug der Kopf die Bezeichnung und die zweite Gruppe denselben Titel
/// noch einmal — im Panel stand derselbe Name zweimal untereinander.
pub fn kopf(app: &App, ui: &mut Ui, ziel: Ziel, titel: &str, icon: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(20.0), egui::Sense::hover());
        crate::icons::paint(ui.painter(), r, icon, theme::Palette::accent());
        ui.label(theme::bold(titel, 14.5));
    });
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
    ui.add_space(2.0);
}

/// Gruppenüberschrift einer Eigenschaftsgruppe — leiser als der Objektkopf
/// und **ohne** dessen Bezeichnung.
///
/// Oben steht „Allgemein“ für die Werte, die jedes Objekt hat (Reihenfolge,
/// Layer, Farbe, Linie); darunter folgt die objekteigene Gruppe, deren Titel
/// nennt, was darin steht („Wandaufbau“, „Maß“, „Geometrie“ …). Damit ist die
/// Gliederung an der Art des Werts erkennbar und nicht am Objektnamen.
pub fn abschnitt(ui: &mut Ui, titel: &str) {
    ui.add_space(6.0);
    widgets::hline(ui);
    ui.add_space(5.0);
    ui.label(theme::bold(titel, 11.5).color(theme::Palette::text_dim()));
    ui.add_space(3.0);
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
fn linienwerte(
    app: &App,
    e: &Option<Entity>,
    ebene: &str,
) -> (f64, jjcad_core::style::LineType) {
    match e {
        Some(e) => (
            e.style
                .line_weight
                .unwrap_or_else(|| app.doc().effective_line_weight(e)),
            e.style
                .line_type
                .unwrap_or_else(|| app.doc().effective_line_type(e)),
        ),
        // T0125: „beim Erzeugen die Vorgabe“ ist die Breite, die das neue
        // Objekt wirklich bekommt — die seiner Ebene. Fehlt sie, gilt die
        // eingestellte Voreinstellung dieser Objektart (0,18 mm), nicht 0,35.
        None => (
            app.ui.pending.lineweight.unwrap_or_else(|| {
                app.doc()
                    .layer(app.doc().current_layer)
                    .map(|l| l.line_weight)
                    .filter(|w| *w > 0.0)
                    .unwrap_or_else(|| app.doc().standard_breite(ebene))
            }),
            app.ui
                .pending
                .line_type
                .unwrap_or(jjcad_core::style::LineType::Solid),
        ),
    }
}

/// Ein Auftrag an die Befehlsschicht aus der Zeile „Layer“ (T0245).
///
/// Der Layer und der Plan eines Objekts standen bisher in **einem** Eintrag
/// („Plan · Layer“) — bei mehreren Plänen wuchs die Liste mit jeder
/// Plan-Layer-Kombination, und mehrere Objekte auf verschiedenen Layern ließen
/// sich nicht gemeinsam auf einen anderen Plan verschieben. Jetzt sind es zwei
/// Abschnitte im selben Dropdown: ein Layername gilt im Plan **jedes**
/// Objekts, ein Planwechsel behält den **Layernamen** bei.
#[derive(Clone, Debug, PartialEq)]
pub enum EbenenZiel {
    /// Dieser Layername, gesucht im Plan jedes gewählten Objekts.
    Layer(String),
    /// Dieser Plan; die Objekte behalten ihren Layernamen. `name` ist der
    /// Name beim Erzeugen (dort gibt es noch kein Objekt, aus dem er käme).
    Plan { plan: Id, name: Option<String> },
}

/// Änderungen der gemeinsamen Zeilen — anwendbar auf Vorgaben oder Objekt.
#[derive(Default)]
pub struct Gemein {
    /// Neue Ebene bzw. neuer Plan (T0245) — zwei getrennte Aufträge an die
    /// Befehlsschicht, obwohl sie in **einem** Dropdown stehen.
    pub ebene: Option<EbenenZiel>,
    pub farbe: Option<Option<Color>>,
    pub staerke: Option<f64>,
    pub linientyp: Option<jjcad_core::style::LineType>,
    /// Dämmung: Bandhöhe H in mm
    pub daemmung_breite: Option<f64>,
    /// Dämmung: Schlaufenform r
    pub daemmung_form: Option<f64>,
}

/// Dämmungswerte (Breite H in mm, Schlaufenform r): am Objekt der tatsächliche
/// Wert, beim Erzeugen die Vorgabe. Bei Mehrfachauswahl gilt das erste Objekt —
/// wie bei allen Zeilen in [`gemein`].
fn daemmung_werte(app: &App, ziel: Ziel) -> (f64, f64) {
    let id = app.mehrfach_ziel.first().copied().or(ziel.id());
    match id.and_then(|i| app.doc().entity(i)) {
        Some(e) => (
            app.doc().effective_loop_width(e),
            app.doc().effective_loop_shape(e),
        ),
        None => (
            app.ui
                .pending
                .loop_width
                .unwrap_or(jjcad_core::daemmung::BREITE_MM),
            app.ui
                .pending
                .loop_shape
                .unwrap_or(jjcad_core::daemmung::FORM),
        ),
    }
}

/// Die drei Zeilen, die jedes Objekt hat: Layer, Farbe, Linie.
///
/// `mit_linie` ist für Text `false` — ein Text hat keine Linie; die Zeilen
/// Stärke und Linientyp erscheinen dann in **beiden** Fällen nicht.
#[allow(clippy::too_many_arguments)]
/// Dropdown „Layer“ mit **zwei Abschnitten** (T0245): oben jeder Layername
/// einmal, darunter die Pläne. Beide Abschnitte stehen im selben Menü, sind
/// aber getrennte Aufträge: ein Layername gilt im Plan jedes Objekts, ein
/// Planwechsel behält den Layernamen bei.
///
/// `aktueller_name` ist `None`, sobald die Layer der Auswahl auseinandergehen
/// — dann zeigt der Abschnitt „Layer“ keinen Haken, der Plan darunter aber
/// weiter den gemeinsamen Plan.
fn ebene_dropdown(
    ui: &mut Ui,
    salt: &str,
    beschriftung: &str,
    namen: &[String],
    plaene: &[(Id, String)],
    aktueller_name: Option<&str>,
    aktuelle_plaene: &[Id],
    name_fuer_plan: &str,
) -> Option<EbenenZiel> {
    panels::dropdown(ui, salt, beschriftung, None, |ui| {
        let mut out = None;
        ui.add_space(2.0);
        ui.label(theme::bold(jjcad_core::i18n::tr("gui.props.layer"), 11.0));
        // T0245: Luft unter der Überschrift — die Auswahlfläche der Zeile
        // darunter reichte sonst bis an den Text und verdeckte ihn.
        ui.add_space(4.0);
        for name in namen {
            let sel = aktueller_name
                .map(|a| a.eq_ignore_ascii_case(name))
                .unwrap_or(false);
            if ui.selectable_label(sel, name).clicked() {
                out = Some(EbenenZiel::Layer(name.clone()));
            }
        }
        ui.add_space(6.0);
        widgets::hline(ui);
        ui.add_space(4.0);
        ui.label(theme::bold(jjcad_core::i18n::tr("gui.props.plan"), 11.0));
        // T0245: derselbe Abstand unter „Plan“.
        ui.add_space(4.0);
        for (pid, pname) in plaene {
            let sel = aktuelle_plaene.contains(pid);
            if ui.selectable_label(sel, pname).clicked() {
                out = Some(EbenenZiel::Plan {
                    plan: *pid,
                    name: Some(name_fuer_plan.to_string()),
                });
            }
        }
        out
    })
}

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
    // angezeigte Vorgabewert: eine Füllfläche zeigt ihre Ebenenstärke, ihre wirksame
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

    // Layer und Plan — **ein** Dropdown mit zwei Abschnitten (T0245).
    zeile("layer");
    // Die Pläne, in denen die gewählten Objekte liegen; beim Erzeugen der Plan
    // der Zielebene. Nur dieser Plan steht im Feld, solange er für alle gilt.
    let plangeber: Vec<Id> = if viele.is_empty() {
        app.doc()
            .plan_of_layer(layer)
            .map(|p| p.id)
            .into_iter()
            .collect()
    } else {
        let mut liste: Vec<Id> = Vec::new();
        for id in &viele {
            let plan = app
                .doc()
                .entity(*id)
                .and_then(|e| app.doc().plan_of_layer(e.layer))
                .map(|p| p.id);
            if let Some(pid) = plan {
                if !liste.contains(&pid) {
                    liste.push(pid);
                }
            }
        }
        liste
    };
    let gleich_plan = plangeber.len() <= 1;
    let layer_name = app
        .doc()
        .layer(layer)
        .map(|l| l.name.clone())
        .unwrap_or_default();
    let plan_name = plangeber
        .first()
        .and_then(|p| app.doc().plan(*p))
        .map(|p| p.name.clone());
    // Das Feld nennt den Plan immer mit — so ist auch bei verschiedenen Layern
    // sichtbar, dass beide Objekte im selben Plan liegen.
    let beschriftung = if !gleich_plan {
        "*variiert*".to_string()
    } else {
        match &plan_name {
            Some(p) if gleich_layer => format!("{p} · {layer_name}"),
            Some(p) => format!("{p} · *variiert*"),
            None if gleich_layer => layer_name.clone(),
            None => "*variiert*".to_string(),
        }
    };
    // Oben jeder Layername **einmal** (in Zeichenreihenfolge der Datei).
    let mut namen: Vec<String> = Vec::new();
    for l in &app.doc().layers {
        if !namen.iter().any(|n| n.eq_ignore_ascii_case(&l.name)) {
            namen.push(l.name.clone());
        }
    }
    let plaene: Vec<(Id, String)> = app
        .doc()
        .plans
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    let mut wahl: Option<EbenenZiel> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.layer"), |ui| {
        wahl = ebene_dropdown(
            ui,
            &ziel.salt("layer"),
            &beschriftung,
            &namen,
            &plaene,
            if gleich_layer {
                Some(layer_name.as_str())
            } else {
                None
            },
            &plangeber,
            &layer_name,
        );
    });
    if let Some(w) = wahl {
        out.ebene = Some(w);
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

        // Dämmung: Breite (H) und Schlaufenform (r). Die Zeilen erscheinen nur
        // bei dieser Linienart — bei allen anderen bewirken sie nichts.
        if out.linientyp.unwrap_or(linientyp) == jjcad_core::style::LineType::Insulation {
            let (breite0, form0) = daemmung_werte(app, ziel);
            let (mut breite, mut form) = (breite0, form0);
            let einheit = app.unit();
            zeile("daemmung_breite");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.daemmung_breite"), |ui| {
                widgets::length_field(
                    ui,
                    &mut breite,
                    einheit,
                    150.0,
                    jjcad_core::daemmung::BREITE_MIN_MM,
                    jjcad_core::daemmung::BREITE_MAX_MM,
                );
            });
            zeile("daemmung_form");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.daemmung_form"), |ui| {
                ui.add(widgets::zahl(
                    &mut form,
                    0.01,
                    jjcad_core::daemmung::FORM_MIN..=jjcad_core::daemmung::FORM_MAX,
                ));
            });
            if (breite - breite0).abs() > 1e-9 {
                out.daemmung_breite = Some(breite);
            }
            if (form - form0).abs() > 1e-9 {
                out.daemmung_form = Some(form);
            }
        }
    }

    out
}

/// Gemeinsame Zeilen auf die Vorgaben anwenden (Erzeugen).
pub fn gemein_nach_optionen(app: &mut App, a: &Gemein) {
    if let Some(z) = &a.ebene {
        // T0202: Die Wahl im Panel ist die Ziel-Ebene des nächsten Elements —
        // sie verschiebt nicht mehr die „aktuelle Ebene“ (die folgt dem Typ).
        // T0245: Layername und Plan sind getrennt; beide enden in einer
        // konkreten Ebene des Zielplans.
        let plan = app
            .doc()
            .aktueller_plan()
            .or_else(|| app.doc().plans.first().map(|p| p.id))
            .unwrap_or(0);
        let (name, farbe) = match z {
            EbenenZiel::Layer(name) => (
                name.clone(),
                app.doc()
                    .layers
                    .iter()
                    .find(|l| l.plan == plan && l.name.eq_ignore_ascii_case(name))
                    .map(|l| l.color)
                    .unwrap_or(Color::BLACK),
            ),
            EbenenZiel::Plan { plan, name } => {
                let plan = *plan;
                let name = name.clone().unwrap_or_else(|| {
                    app.doc()
                        .layer(app.doc().current_layer)
                        .map(|l| l.name.clone())
                        .unwrap_or_default()
                });
                let farbe = app
                    .doc()
                    .layers
                    .iter()
                    .find(|l| l.plan == plan && l.name.eq_ignore_ascii_case(&name))
                    .map(|l| l.color)
                    .unwrap_or(Color::BLACK);
                (name, farbe)
            }
        };
        let lid = app.doc_mut().ensure_layer_in(&name, farbe, plan);
        app.ui.pending.layer = Some(lid);
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
    if let Some(b) = a.daemmung_breite {
        app.ui.pending.loop_width = Some(b);
    }
    if let Some(f) = a.daemmung_form {
        app.ui.pending.loop_shape = Some(f);
    }
}

/// Gemeinsame Zeilen als `modify.set` an die Auswahl schicken.
pub fn gemein_nach_objekt(app: &mut App, id: Id, a: &Gemein) {
    let mut params = serde_json::json!({"ids": [id]});
    if let Some(map) = params.as_object_mut() {
        if let Some(z) = &a.ebene {
            // T0245: Die Befehlsschicht löst den Namen je Objekt auf — so
            // behalten mehrere Objekte auf verschiedenen Layern ihre Struktur.
            match z {
                EbenenZiel::Layer(name) => {
                    map.insert("layer_name".into(), serde_json::json!(name));
                }
                EbenenZiel::Plan { plan, .. } => {
                    map.insert("plan".into(), serde_json::json!(plan));
                }
            }
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
            map.insert("linetype".into(), serde_json::json!(lt.key()));
        }
        if let Some(b) = a.daemmung_breite {
            // Länge in der Eingabeeinheit der Zeichnung (siehe `canvas::run`).
            map.insert(
                "loop_width".into(),
                serde_json::json!(app.unit().from_mm(b)),
            );
        }
        if let Some(f) = a.daemmung_form {
            map.insert("loop_shape".into(), serde_json::json!(f));
        }
    }
    if params.as_object().map(|m| m.len() > 1).unwrap_or(false) {
        setzen(app, &params);
    }
}

/// Übernimmt **alle** Eigenschaften eines vorhandenen Objekts in die Vorgaben
/// für das nächste Element (Schnellauswahl, T0169).
///
/// Die Schnellauswahl startet den Erzeuger der angeklickten Art; ohne diese
/// Übernahme entstand das neue Element in der zuletzt gewählten Farbe und
/// Stärke — eine rote Linie wurde zur schwarzen. Übernommen werden die
/// gemeinsamen Zeilen (Ebene, Farbe, Füllung, Stärke, Linientyp, Dämmung) und
/// die art-eigenen Werte, die im Eigenschaften-Panel beim Erzeugen stehen.
///
/// Die Werte kommen aus **derselben** Quelle wie die Anzeige bei Auswahl
/// (`Ziel::Auswahl`): die Felder des Objekts, nicht die wirksamen Werte der
/// Ebene. Ein Objekt mit `color: None` (vom Layer) setzt die Vorgabe deshalb
/// wieder auf „vom Layer“ — die neue Linie erbt dann die Farbe ihrer Ebene.
///
/// Liefert `false`, wenn das Objekt nicht (mehr) existiert.
pub fn vorgaben_aus_objekt(app: &mut App, id: Id) -> bool {
    let Some(e) = app.doc().entity(id).cloned() else {
        return false;
    };
    // Ebene: das neue Element soll auf derselben Ebene liegen. `canvas::run`
    // gibt `current_layer` an jede Erzeuger-Action weiter.
    app.doc_mut().current_layer = e.layer;
    // Gemeinsame Zeilen — dieselben Felder, die `gemein_nach_optionen` setzt.
    app.ui.pending.color = e.style.color;
    app.ui.pending.fill = e.style.fill;
    app.ui.pending.no_fill = e.style.no_fill;
    app.ui.pending.lineweight = e.style.line_weight;
    app.ui.pending.line_type = e.style.line_type;
    app.ui.pending.loop_width = e.style.loop_width;
    app.ui.pending.loop_shape = e.style.loop_shape;
    // Punktsymbole (T0210) gehören zu den übernommenen Eigenschaften: die
    // Schnellauswahl soll dieselbe Marke wieder erzeugen.
    app.options.symbol_start = e.start_symbol.clone();
    app.options.symbol_end = e.end_symbol.clone();

    // Art-eigene Werte — je Zweig dieselben Felder, die das Panel beim
    // Erzeugen in `ToolOptions` schreibt.
    match &e.kind {
        EntityKind::Wall {
            thickness,
            height,
            hatch,
            align,
            ..
        } => {
            app.options.wall_thickness = *thickness;
            app.options.wall_height = *height;
            app.options.wall_hatch = *hatch;
            app.options.wall_align = *align;
        }
        EntityKind::Door {
            width,
            kind,
            hinge,
            swing_in,
            base,
            ..
        } => {
            app.options.door_width = *width;
            app.options.door_kind = *kind;
            app.options.hinge = *hinge;
            app.options.swing_in = *swing_in;
            app.options.door_base = *base;
        }
        EntityKind::Window {
            width,
            sill,
            height,
            align,
            ..
        } => {
            app.options.window_width = *width;
            app.options.window_sill = *sill;
            app.options.window_height = *height;
            app.options.window_align = *align;
        }
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
            line_spacing,
            frame,
            frame_round,
            background,
            ..
        } => {
            app.options.text_height = *height;
            app.options.text_angle = angle.to_degrees();
            app.options.text_align = *align;
            app.options.text_valign = *valign;
            app.options.text_bold = *bold;
            app.options.text_italic = *italic;
            app.options.text_underline = *underline;
            app.options.text_superscript = *superscript;
            app.options.text_font = *font;
            app.options.text_line_spacing = *line_spacing;
            app.options.text_frame = *frame;
            app.options.text_frame_round = *frame_round;
            app.options.text_background = *background;
            // Der Text wird in der Zeichenfläche getippt; dessen Editor liest
            // die Farbe aus `text_color` (nicht aus `pending.color`).
            app.options.text_color = e.style.color;
        }
        EntityKind::Room { height, heading, .. } => {
            app.options.room_height = *height;
            app.options.room_heading = heading.clone();
        }
        EntityKind::Dim {
            angle,
            override_text,
            ..
        } => {
            app.options.dim_axis = Some(DimAxis::aus_winkel(*angle));
            app.options.dim_text = override_text.clone().unwrap_or_default();
        }
        EntityKind::ElevationPlan { z, text_pos, .. } => {
            app.options.elevation_z = *z;
            app.options.elevation_text_pos = *text_pos;
        }
        EntityKind::Fill {
            pattern,
            pattern_angle,
            pattern_scale,
            ..
        } => {
            app.options.fill_pattern = *pattern;
            app.options.fill_angle = *pattern_angle;
            app.options.fill_scale = *pattern_scale;
        }
        EntityKind::Volume {
            height,
            pattern,
            pattern_angle,
            pattern_scale,
            ..
        } => {
            app.options.volume_height = *height;
            app.options.volume_pattern = *pattern;
            app.options.volume_angle = *pattern_angle;
            app.options.volume_scale = *pattern_scale;
        }
        EntityKind::Stair {
            width,
            steps,
            height,
            auto_steps,
            cut,
            ..
        } => {
            app.options.stair_width = *width;
            app.options.stair_steps = *steps;
            app.options.stair_height = *height;
            app.options.stair_auto = *auto_steps;
            app.options.stair_cut = *cut;
        }
        EntityKind::BlockRef {
            block, rot, scale, ..
        } => {
            app.options.block_id = Some(block.clone());
            app.options.block_rot = rot.to_degrees();
            app.options.block_scale = scale.x;
        }
        EntityKind::Image {
            lock_ratio,
            name,
            data,
            ..
        } => {
            app.options.bild_lock = *lock_ratio;
            app.options.bild = Some(crate::app::BildQuelle {
                name: name.clone(),
                data: data.clone(),
            });
        }
        // Freie Geometrie, Koten und Ansichtsfenster tragen nur die
        // gemeinsamen Zeilen — die Maße entstehen aus den Klicks.
        _ => {}
    }
    true
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
    let (
        titel,
        layer,
        farbe,
        mut hoehe,
        mut schrift,
        mut zeilenabstand,
        mut umrandung,
        mut runde_umrandung,
        mut hintergrund,
        mut drehung,
        mut ah,
        mut av,
        mut stil,
    ) = match &e {
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
                    line_spacing,
                    frame,
                    frame_round,
                    background,
                    ..
                } => (
                    e.kind.type_name(),
                    e.layer,
                    e.style.color,
                    *height,
                    *font,
                    *line_spacing,
                    *frame,
                    *frame_round,
                    *background,
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
                app.werkzeug_ziel_ebene("Beschriftung"),
                app.options.text_color,
                app.options.text_height,
                app.options.text_font,
                app.options.text_line_spacing,
                app.options.text_frame,
                app.options.text_frame_round,
                app.options.text_background,
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
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);

    // Text hat keine Linie: Stärke und Linientyp erscheinen in beiden Fällen nicht.
    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        app.doc().standard_breite("Beschriftung"),
        jjcad_core::style::LineType::Solid,
        false,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.schrift_und_inhalt"));
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

    // Zeilenabstand: Vielfaches der Zeilenhöhe der Schrift. Mehrzeilige Texte
    // haben je Schriftart verschieden hohe Zeilen — hier lässt sich das
    // strecken oder stauchen, der Rahmen folgt (T0110). Angeboten werden die
    // typischen Werte aus Textprogrammen (Einfach … Doppelt) und ein freier
    // Wert (T0126).
    zeile("zeilenabstand");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.zeilenabstand"), |ui| {
        widgets::zeilenabstand_wahl(ui, &ziel.salt("zeilenabstand"), &mut zeilenabstand);
    });

    // Umrandung: derselbe Kasten, den die Bearbeitung als Auswahlrahmen zeigt
    // — dauerhaft in der Zeichnung. Auf Wunsch rund: Radius = halbe
    // Schrifthöhe, einzeilig also ein Oval aus zwei Halbkreisen (T0126).
    zeile("umrandung");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.umrandung"), |ui| {
        ui.checkbox(&mut umrandung, jjcad_core::i18n::tr("gui.props.anzeigen"))
            .on_hover_text(jjcad_core::i18n::tr("gui.props.umrandung_hinweis"));
    });
    zeile("runde_umrandung");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.runde_umrandung"), |ui| {
        ui.add_enabled_ui(umrandung, |ui| {
            ui.checkbox(
                &mut runde_umrandung,
                jjcad_core::i18n::tr("gui.props.abgerundet"),
            )
            .on_hover_text(jjcad_core::i18n::tr("gui.props.runde_umrandung_hinweis"));
        });
    });

    // Hintergrund: Fläche hinter dem Text, immer das **Rechteck** der
    // Umrandung — eine runde Fläche gibt es nicht (T0126).
    zeile("hintergrund");
    let mut hintergrund_neu = hintergrund;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hintergrund"), |ui| {
        let gezeigt = hintergrund.unwrap_or(Color::rgb(255, 255, 255));
        if let Some(v) = panels::color_combo(
            ui,
            &ziel.salt("hintergrund"),
            gezeigt,
            hintergrund.is_none(),
            false,
            jjcad_core::i18n::tr("gui.props.keine_flaeche"),
        ) {
            hintergrund_neu = v;
        }
    });
    hintergrund = hintergrund_neu;

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
            // Die Farbe des nächsten Textes steht in `text_color`: der Editor in
            // der Zeichenfläche liest sie dort (nicht aus `pending.color`), und
            // die Zeile oben zeigt sie von dort. Ohne diesen Schritt schrieb das
            // Panel die Farbe an eine Stelle, die der Text nicht liest (T0169).
            if let Some(v) = a.farbe {
                app.options.text_color = v;
            }
            app.options.text_height = hoehe;
            app.options.text_font = schrift;
            app.options.text_line_spacing = zeilenabstand;
            app.options.text_frame = umrandung;
            app.options.text_frame_round = runde_umrandung;
            app.options.text_background = hintergrund;
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
                        line_spacing,
                        frame,
                        frame_round,
                        background,
                        ..
                    } => Some((
                        *height,
                        angle.to_degrees(),
                        *align,
                        *valign,
                        *font,
                        *line_spacing,
                        *frame,
                        *frame_round,
                        *background,
                    )),
                    _ => None,
                });
                if let Some((h0, w0, ah0, av0, f0, a0, ra0, rr0, hg0)) = alt {
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
                    if (zeilenabstand - a0).abs() > 1e-9 {
                        map.insert("line_spacing".into(), serde_json::json!(zeilenabstand));
                        geaendert = true;
                    }
                    if umrandung != ra0 {
                        map.insert("frame".into(), serde_json::json!(umrandung));
                        geaendert = true;
                    }
                    if runde_umrandung != rr0 {
                        map.insert("frame_round".into(), serde_json::json!(runde_umrandung));
                        geaendert = true;
                    }
                    if hintergrund != hg0 {
                        map.insert(
                            "background".into(),
                            match hintergrund {
                                Some(c) => serde_json::json!(c.to_hex()),
                                None => serde_json::json!("none"),
                            },
                        );
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
            app.options.text_frame,
            app.options.text_frame_round,
            app.options.text_background,
        )),
        Some(id) => match app.doc().entity(id).map(|e| e.kind.clone()) {
            Some(EntityKind::Text {
                height,
                font,
                align,
                valign,
                angle,
                frame,
                frame_round,
                background,
                ..
            }) => Some((height, font, align, valign, angle, frame, frame_round, background)),
            _ => None,
        },
    };
    if let (Some((hoehe, schrift, ah, av, winkel, umrandung, rund, hintergrund)), Some(t)) =
        (werte, app.ui.texteingabe.as_mut())
    {
        t.height = hoehe;
        t.font = schrift;
        t.align = ah;
        t.valign = av;
        t.angle = winkel;
        // Umrandung und Hintergrund gelten auch während des Tippens (T0126).
        t.frame = umrandung;
        t.frame_round = rund;
        t.background = hintergrund;
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
    let (
        layer,
        farbe,
        mut dicke,
        mut hoehe,
        mut ausrichtung,
        mut schraffur,
        mut fuellfarbe,
        mut kein_fill,
    ) = match &e {
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
                e.style.no_fill,
            ),
            _ => return,
        },
        None => (
            app.werkzeug_ziel_ebene("Architektur"),
            app.ui.pending.color,
            app.options.wall_thickness,
            app.options.wall_height,
            app.options.wall_align,
            app.options.wall_hatch,
            app.ui.pending.fill,
            app.ui.pending.no_fill,
        ),
    };

    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.wand"), "wall");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e, "Architektur");

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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.wandaufbau"));
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
        // Muster und Fläche sind getrennt (T0242): „Keine Füllung“ nimmt nur
        // den Flächenton weg, das Muster bleibt.
    }

    zeile("fuellfarbe");
    // Drei Zustände (T0242): keine Füllung, Standard oder eine Farbe. Ohne
    // Füllfarbe zeichnet die Wand den Vorgabeton — das ist etwas anderes.
    let fill_cur = fuellfarbe.unwrap_or_else(|| farbe_gezeigt.mix(Color::WHITE, 0.74));
    let wahl = jjcad_core::style::Fuellung::aus_style(fuellfarbe, kein_fill);
    let mut picked_fill: Option<jjcad_core::style::Fuellung> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fuellfarbe"), |ui| {
        picked_fill = panels::fuellung_combo(
            ui,
            &ziel.salt("wand_fuellfarbe"),
            wahl,
            fill_cur,
            jjcad_core::i18n::tr("gui.props.standard"),
        );
    });
    if let Some(v) = picked_fill {
        (fuellfarbe, kein_fill) = v.in_style();
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
            app.ui.pending.fill = fuellfarbe;
            app.ui.pending.no_fill = kein_fill;
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
                    if fuellfarbe != e.style.fill || kein_fill != e.style.no_fill {
                        map.insert(
                            "fill".into(),
                            serde_json::json!(fuell_param(fuellfarbe, kein_fill)),
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

/// Füllung als Action-Parameter (T0242): „keine“, Farbwert oder „none“.
fn fuell_param(fill: Option<Color>, kein_fill: bool) -> String {
    if kein_fill {
        "keine".to_string()
    } else {
        match fill {
            Some(c) => c.to_hex(),
            None => "none".to_string(),
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
            app.werkzeug_ziel_ebene("Architektur"),
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
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e, "Architektur");

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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.tuerblatt"));
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
            app.werkzeug_ziel_ebene("Architektur"),
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
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e, "Architektur");

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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.fenstermasse"));
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

    let (
        layer,
        farbe,
        mut muster,
        mut winkel,
        mut abstand,
        mut rand,
        mut fuellfarbe,
        mut kein_fill,
    ) = match &e {
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
                e.style.no_fill,
            ),
            _ => return,
        },
        None => (
            app.werkzeug_ziel_ebene("Zeichnung"),
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
            app.ui.pending.no_fill,
        ),
    };

    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.fuellflaeche"), "fill");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);

    let a = gemein(
        app,
        ui,
        ziel,
        layer,
        farbe,
        farbe_gezeigt,
        app.doc().standard_breite("Zeichnung"),
        jjcad_core::style::LineType::Solid,
        true,
        jjcad_core::i18n::tr("gui.props.linienfarbe"),
    );

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.fuellung"));
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
        // Muster und Fläche sind getrennt (T0242): „Keine Füllung“ nimmt nur
        // den Flächenton weg, das Muster bleibt.
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
    // Drei Zustände (T0242): keine Füllung, Standard oder eine Farbe.
    let fill_cur = fuellfarbe.unwrap_or_else(|| farbe_gezeigt.mix(Color::WHITE, 0.72));
    let wahl = jjcad_core::style::Fuellung::aus_style(fuellfarbe, kein_fill);
    let mut picked_fill: Option<jjcad_core::style::Fuellung> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fuellfarbe"), |ui| {
        picked_fill = panels::fuellung_combo(
            ui,
            &ziel.salt("fuell_farbe"),
            wahl,
            fill_cur,
            jjcad_core::i18n::tr("gui.props.standard"),
        );
    });
    if let Some(v) = picked_fill {
        (fuellfarbe, kein_fill) = v.in_style();
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
            app.ui.pending.fill = fuellfarbe;
            app.ui.pending.no_fill = kein_fill;
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
                    if fuellfarbe != e.style.fill || kein_fill != e.style.no_fill {
                        map.insert(
                            "fill".into(),
                            serde_json::json!(fuell_param(fuellfarbe, kein_fill)),
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
    // Orientierung: gespeichert ist ein Winkel (T0166). Die Auswahl zeigt
    // daraus die nächstgelegene Eingabeweise, das Winkel-Feld den genauen Wert.
    let (layer, farbe, mut achse, mut winkel, mut masstext) = match &e {
        Some(e) => match &e.kind {
            EntityKind::Dim {
                angle,
                override_text,
                ..
            } => (
                e.layer,
                e.style.color,
                Some(DimAxis::aus_winkel(*angle)),
                *angle,
                override_text.clone().unwrap_or_default(),
            ),
            _ => return,
        },
        None => {
            let achse = app.options.dim_axis.unwrap_or(DimAxis::X);
            (
                app.werkzeug_ziel_ebene("Beschriftung"),
                app.ui.pending.color,
                Some(achse),
                achse.winkel(&[]).unwrap_or(0.0),
                app.options.dim_text.clone(),
            )
        }
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.bemassung"), "dim");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);

    let (staerke, linientyp) = linienwerte(app, &e, "Beschriftung");
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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.mass"));
    zeile("dimkopf");

    zeile("richtung");
    // **Eine** Zeile für beide Fälle: beim Erzeugen zeigt sie den Modus des
    // laufenden Werkzeugs (gewählt wird er über die drei Schaltflächen im
    // Reiter „Bemaßung“, T0154 Turn 3), bei der Auswahl ist sie die Achse des
    // vorhandenen Maßes. Die Zeilen bleiben damit gleich — nur die Werte
    // unterscheiden sich.
    if ziel.ist_neu() {
        let modus = app
            .tool
            .as_ref()
            .and_then(|t| crate::app::DimModus::aus_werkzeug(&t.action))
            .unwrap_or(crate::app::DimModus::Direkt);
        achse = Some(modus.axis());
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.richtung"), |ui| {
            ui.label(theme::mono(modus.label().to_string()));
        });
    } else {
        let axis_items: Vec<(Option<DimAxis>, String)> = DimAxis::ALL
            .iter()
            .map(|a| (Some(*a), a.text().to_string()))
            .collect();
        let mut picked = None;
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.richtung"), |ui| {
            picked = panels::pick_combo(ui, &ziel.salt("dim_achse"), achse, &axis_items);
        });
        if let Some(v) = picked {
            achse = v;
            // Die Auswahl setzt den Winkel der Kette — „Parallel“ nimmt die
            // Richtung der ersten Messstrecke (T0166).
            if let Some(e) = &e {
                if let EntityKind::Dim { pts, .. } = &e.kind {
                    if let Some(w) = v.and_then(|a| a.winkel(pts)) {
                        winkel = w;
                    }
                }
            }
        }
    }

    // Orientierung als Winkel: beim Erzeugen zeigt die Zeile den Winkel des
    // Werkzeugmodus (gesetzt wird er über die drei Schaltflächen), bei der
    // Auswahl lässt sie sich frei drehen — dieselbe Zeile in beiden Fällen.
    zeile("winkel");
    if ziel.ist_neu() {
        // Beim Erzeugen steht der Winkel erst nach den Messpunkten fest, wenn
        // das Werkzeug „Direkt“ läuft — dann zeigt die Zeile das an, statt
        // eine Zahl zu erfinden.
        let direkt = matches!(achse, Some(DimAxis::Aligned) | None);
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.winkel"), |ui| {
            if direkt {
                ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.aus_den_klicks")));
            } else {
                ui.label(theme::mono(format!(
                    "{}°",
                    jjcad_core::units::format_number(winkel.to_degrees(), 1)
                )));
            }
        });
    } else {
        let mut grad = winkel.to_degrees();
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.winkel"), |ui| {
            ui.add(widgets::zahl(
                &mut grad,
                1.0,
                widgets::WINKEL_MIN..=widgets::WINKEL_MAX,
            ));
        });
        if (grad - winkel.to_degrees()).abs() > 1e-9 {
            winkel = grad.to_radians();
        }
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

    // Verlängerung zu den Messpunkten (T0226): Maßhilfslinien von jedem
    // Messpunkt zur Maßlinie. Vorgabe aus — die Maßzahl steht allein an der
    // Maßlinie.
    let mut verlaengerung = e
        .as_ref()
        .map(|e| matches!(&e.kind, EntityKind::Dim { ext_lines: true, .. }))
        .unwrap_or(app.options.dim_ext);
    zeile("verlaengerung");
    widgets::field_row(
        ui,
        jjcad_core::i18n::tr("gui.props.verlaengerung"),
        |ui| {
            ui.checkbox(&mut verlaengerung, "")
                .on_hover_text(jjcad_core::i18n::tr("gui.props.verlaengerung_hinweis"));
        },
    );

    // Punktsymbol an den Messpunkten (T0227): die Marke, die jeder Messpunkt
    // trägt. Vorgabe ist der Schrägstrich; „Kein Symbol“ nimmt sie weg.
    let mut symbol = e
        .as_ref()
        .and_then(|e| match &e.kind {
            EntityKind::Dim { symbol, .. } => Some(symbol.clone()),
            _ => None,
        })
        .unwrap_or_else(|| app.options.dim_symbol.clone());
    zeile("symbol");
    let mut symbol_neu: Option<Option<String>> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.punktsymbol"), |ui| {
        symbol_neu = punktsymbol_wahl(app, ui, &ziel.salt("dim_symbol"), Some(&symbol));
    });
    if let Some(v) = symbol_neu {
        symbol = v.unwrap_or_default();
    }

    zeile("kennzahlen");
    ui.add_space(4.0);
    match (&e, ziel) {
        (Some(e), Ziel::Auswahl(_)) => {
            if let EntityKind::Dim { pts, angle, .. } = &e.kind {
                // Anzahl der Messpunkte und die Summe aller Segmente — beim
                // Einzelmaß ist das genau eine Strecke (T0154). Gemessen wird
                // der Anteil längs der Maßlinie (T0166).
                let (dir, _) = jjcad_core::render::dim_achse(*angle);
                let mut summe = 0.0;
                for w in pts.windows(2) {
                    summe += (w[1] - w[0]).dot(dir).abs();
                }
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned(
                    "gui.props.x_basispunkte",
                    &[("wert1", format!("{}", pts.len()))],
                )));
                ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.props.mass_x_x", &[("wert1", format!("{}", jjcad_core::units::format_number(
                        app.doc().settings.dim_unit.from_mm(summe),
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
            app.options.dim_ext = verlaengerung;
            app.options.dim_symbol = symbol;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::Dim {
                        angle,
                        override_text,
                        ..
                    } = &e.kind
                    {
                        let alt_text = override_text.clone().unwrap_or_default();
                        if masstext != alt_text {
                            map.insert("text".into(), serde_json::json!(masstext));
                            geaendert = true;
                        }
                        if (winkel - *angle).abs() > 1e-9 {
                            map.insert("angle".into(), serde_json::json!(winkel.to_degrees()));
                            geaendert = true;
                        }
                        let alt_ext = matches!(
                            &e.kind,
                            EntityKind::Dim { ext_lines: true, .. }
                        );
                        if verlaengerung != alt_ext {
                            map.insert("ext_lines".into(), serde_json::json!(verlaengerung));
                            geaendert = true;
                        }
                        // Punktsymbol an den Messpunkten (T0227).
                        let alt_sym = match &e.kind {
                            EntityKind::Dim { symbol, .. } => symbol.clone(),
                            _ => String::new(),
                        };
                        if symbol != alt_sym {
                            map.insert("symbol".into(), serde_json::json!(symbol));
                            geaendert = true;
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

/// Radiusbemaßung (T0252) — dieselben Zeilen beim Erzeugen und beim
/// Bearbeiten. Der Radius kommt aus dem angeklickten Kreis und ist deshalb
/// eine **Anzeige**; änderbar sind Maßtext und Richtung.
pub fn radiusbemassung(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let unit = app.unit();
    let (layer, farbe, mut winkel, radius, mut masstext) = match &e {
        Some(e) => match &e.kind {
            EntityKind::DimRadius {
                angle,
                r,
                override_text,
                ..
            } => (
                e.layer,
                e.style.color,
                angle.to_degrees(),
                *r,
                override_text.clone().unwrap_or_default(),
            ),
            _ => return,
        },
        None => (
            app.werkzeug_ziel_ebene("Beschriftung"),
            app.ui.pending.color,
            0.0,
            0.0,
            app.options.dim_text.clone(),
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.radiusmass"), "radius");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);
    let (staerke, linientyp) = linienwerte(app, &e, "Beschriftung");
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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.mass"));
    zeile("dimkopf");

    // Radius: beim Erzeugen kommt er aus dem Klick, beim Bearbeiten aus dem
    // gespeicherten Wert — geändert wird er am bemaßten Kreis, nicht hier.
    zeile("radius");
    if ziel.ist_neu() {
        gesperrt_zeile(
            ui,
            jjcad_core::i18n::tr("gui.props.radius"),
            jjcad_core::i18n::tr("gui.props.aus_den_klicks"),
        );
    } else {
        let (haupt, hoch) =
            jjcad_core::units::dim_zahl(radius, app.doc().settings.dim_unit);
        let txt = format!("R {haupt}{}", hoch.unwrap_or_default());
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.radius"), |ui| {
            ui.label(theme::mono(txt));
        });
    }

    // Richtung der Maßlinie — beim Erzeugen der Klick, beim Bearbeiten ein
    // Winkel (dieselbe Zeile in beiden Fällen).
    zeile("winkel");
    if ziel.ist_neu() {
        gesperrt_zeile(
            ui,
            jjcad_core::i18n::tr("gui.props.winkel"),
            jjcad_core::i18n::tr("gui.props.aus_den_klicks"),
        );
    } else {
        let mut grad = winkel;
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.winkel"), |ui| {
            ui.add(widgets::zahl(
                &mut grad,
                1.0,
                widgets::WINKEL_MIN..=widgets::WINKEL_MAX,
            ));
        });
        if (grad - winkel).abs() > 1e-9 {
            winkel = grad;
        }
    }

    zeile("masstext");
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.masstext"), |ui| {
        ui.add_sized(
            [120.0, 20.0],
            egui::TextEdit::singleline(&mut masstext)
                .hint_text(jjcad_core::i18n::tr("gui.props.automatisch")),
        );
    });

    match ziel {
        Ziel::Neu => {
            gemein_nach_optionen(app, &a);
            app.options.dim_text = masstext;
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    if let EntityKind::DimRadius {
                        angle,
                        override_text,
                        ..
                    } = &e.kind
                    {
                        let alt_text = override_text.clone().unwrap_or_default();
                        if masstext != alt_text {
                            map.insert("text".into(), serde_json::json!(masstext));
                            geaendert = true;
                        }
                        if (winkel.to_radians() - *angle).abs() > 1e-9 {
                            map.insert("angle".into(), serde_json::json!(winkel));
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
    /// Pfad: Kette aus Linien, Bögen, Ellipsenbögen und Splines (T0229).
    Pfad,
    Rechteck,
    Kreis,
    Bogen,
    Ellipse,
    /// Abschnitt einer Ellipse — entsteht beim Stutzen (T0084).
    Ellipsenbogen,
    /// Grundriss-Kote: Kreis-Symbol, Höhe im Panel.
    Grundrisskote,
    /// Schnitt-Kote: Punktfolge, Höhen aus der Y-Differenz.
    Schnittkote,
    Block,
    /// Bild: zwei Ecken spannen das Rechteck auf, Maße stehen als Anzeige da.
    Bild,
}

impl Einfach {
    /// Standardebene dieser Objektart — sie trägt die Linienstärke-Vorgabe
    /// aus den Einstellungen (T0125). Muss zu `actions::standard_ebene`
    /// passen, sonst zeigt das Panel eine andere Vorgabe als das erzeugte
    /// Objekt sie bekommt.
    pub fn ebene(self) -> &'static str {
        match self {
            Einfach::Grundrisskote | Einfach::Schnittkote => "Beschriftung",
            Einfach::Block => "Blöcke",
            Einfach::Bild => "Zeichnung",
            _ => "Zeichnung",
        }
    }

    /// Art aus dem Werkzeug, das gerade läuft.
    pub fn aus_werkzeug(id: &str) -> Option<Self> {
        Some(match id {
            "draw.point" => Self::Punkt,
            "draw.line" => Self::Linie,
            "draw.polyline" => Self::Polylinie,
            "draw.path" => Self::Pfad,
            "draw.spline" => Self::Spline,
            "draw.rect" => Self::Rechteck,
            "draw.circle" => Self::Kreis,
            "draw.arc" => Self::Bogen,
            "draw.ellipse" => Self::Ellipse,
            "dim.elevation.plan" => Self::Grundrisskote,
            "dim.elevation.section" => Self::Schnittkote,
            "block.insert" => Self::Block,
            "draw.image" => Self::Bild,
            _ => return None,
        })
    }

    /// Art aus einem vorhandenen Objekt.
    pub fn aus_objekt(kind: &EntityKind) -> Option<Self> {
        Some(match kind {
            EntityKind::Point { .. } => Self::Punkt,
            EntityKind::Line { .. } => Self::Linie,
            EntityKind::Polyline { .. } => Self::Polylinie,
            EntityKind::Path { .. } => Self::Pfad,
            EntityKind::Spline { .. } => Self::Spline,
            EntityKind::Circle { .. } => Self::Kreis,
            EntityKind::Arc { .. } => Self::Bogen,
            EntityKind::Ellipse { .. } => Self::Ellipse,
            EntityKind::EllipseArc { .. } => Self::Ellipsenbogen,
            EntityKind::ElevationPlan { .. } => Self::Grundrisskote,
            EntityKind::ElevationSection { .. } => Self::Schnittkote,
            EntityKind::BlockRef { .. } => Self::Block,
            EntityKind::Image { .. } => Self::Bild,
            _ => return None,
        })
    }

    fn titel(self) -> &'static str {
        match self {
            Self::Punkt => jjcad_core::i18n::tr("gui.props.punkt"),
            Self::Linie => jjcad_core::i18n::tr("gui.props.linie"),
            Self::Polylinie => jjcad_core::i18n::tr("gui.props.polylinie"),
            Self::Pfad => jjcad_core::i18n::tr("gui.props.pfad"),
            Self::Spline => jjcad_core::i18n::tr("gui.props.spline"),
            Self::Rechteck => jjcad_core::i18n::tr("gui.props.rechteck"),
            Self::Kreis => jjcad_core::i18n::tr("gui.props.kreis"),
            Self::Bogen => jjcad_core::i18n::tr("gui.props.bogen"),
            Self::Ellipse => jjcad_core::i18n::tr("gui.props.ellipse"),
            Self::Ellipsenbogen => jjcad_core::i18n::tr("gui.props.ellipsenbogen"),
            Self::Grundrisskote => "Grundriss-Kote",
            Self::Schnittkote => "Schnitt-Kote",
            Self::Block => jjcad_core::i18n::tr("gui.props.block"),
            Self::Bild => jjcad_core::i18n::tr("gui.props.bild"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Punkt => "point",
            Self::Linie => "line",
            Self::Polylinie | Self::Rechteck | Self::Pfad => "polyline",
            Self::Spline => "spline",
            Self::Kreis => "circle",
            Self::Bogen => "arc",
            Self::Ellipse => "ellipse",
            Self::Ellipsenbogen => "arc",
            Self::Grundrisskote => "elevation",
            Self::Schnittkote => "elevation-section",
            Self::Block => "blocknew",
            Self::Bild => "image",
        }
    }

    /// Hat die Art eine Linie (Farbe, Stärke, Linientyp)?
    fn mit_linie(self) -> bool {
        !matches!(self, Self::Block | Self::Bild)
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
            app.werkzeug_ziel_ebene("Beschriftung"),
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
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
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
        app.doc().standard_breite("Beschriftung"),
        jjcad_core::style::LineType::Solid,
        false,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.beschriftung"));
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

/// Pixelmaße eines eingebetteten Bildes als Anzeigetext — leer, wenn die
/// Daten nicht lesbar sind.
fn bild_pixel(data: &str) -> Option<String> {
    jjcad_core::bild::dekodiere(data).map(|b| format!("{} × {} px", b.breite, b.hoehe))
}

/// Auswahl eines Punktsymbols (T0210).
///
/// Die Liste kommt aus [`jjcad_core::model::Document::punktsymbol_bloecke`] —
/// **alle** Blöcke der Kategorie „Punkt Symbole“, gleich ob System-, Projekt-
/// oder Benutzerblock. „Kein Symbol“ steht oben und nimmt die Marke weg.
/// Rückgabe: `Some(None)` = kein Symbol, `Some(Some(id))` = gewählter Block.
fn punktsymbol_wahl(
    app: &App,
    ui: &mut Ui,
    salt: &str,
    cur: Option<&str>,
) -> Option<Option<String>> {
    let bloecke: Vec<(String, String)> = app
        .doc()
        .punktsymbol_bloecke()
        .iter()
        .map(|b| (b.id.clone(), b.name.clone()))
        .collect();
    if bloecke.is_empty() {
        ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.keine_punktsymbole")));
        return None;
    }
    let cap = cur
        .and_then(|id| {
            bloecke
                .iter()
                .find(|(b, _)| b == id)
                .map(|(_, n)| n.clone())
        })
        .unwrap_or_else(|| jjcad_core::i18n::tr("gui.props.kein_symbol").to_string());
    panels::dropdown(ui, salt, &cap, None, |ui| {
        let mut out = None;
        if ui
            .selectable_label(cur.is_none(), jjcad_core::i18n::tr("gui.props.kein_symbol"))
            .clicked()
        {
            out = Some(None);
        }
        for (id, name) in &bloecke {
            let gewaehlt = cur == Some(id.as_str());
            ui.horizontal(|ui| {
                // Kleine Vorschau: ein Symbol wählt man nach seiner Form, nicht
                // nach seinem Namen.
                symbol_vorschau(app, ui, id);
                if ui.selectable_label(gewaehlt, name).clicked() {
                    out = Some(Some(id.clone()));
                }
            });
        }
        out
    })
}

/// Zeichnet ein Punktsymbol als kleine Vorschau in die laufende Zeile.
fn symbol_vorschau(app: &App, ui: &mut Ui, id: &str) {
    let Some(def) = app.doc().block(id) else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 16.0), egui::Sense::hover());
    let ctx = jjcad_core::render::RenderCtx::new(app.doc());
    let mut dl = jjcad_core::render::DisplayList::default();
    for e in &def.entities {
        jjcad_core::render::render_entity(
            &ctx,
            e,
            jjcad_core::geom::Affine::IDENTITY,
            &mut dl,
            0,
        );
    }
    if dl.bbox.is_valid() {
        crate::inventory::draw_list_preview(ui.painter(), rect, dl.bbox, &dl, theme::Palette::text());
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
    let (staerke, linientyp) = linienwerte(app, &e, art.ebene());

    kopf(app, ui, ziel, art.titel(), art.icon());
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
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

    // Die Überschrift nennt, was die Zeilen zeigen: bei Block und Bild sind
    // es dessen Daten, bei den übrigen Arten Maße und Winkel.
    abschnitt(ui, jjcad_core::i18n::tr(match art {
        Einfach::Block => "gui.props.blockdaten",
        Einfach::Bild => "gui.props.bilddaten",
        Einfach::Punkt | Einfach::Linie => "gui.props.symbole",
        _ => "gui.props.geometrie",
    }));
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
    // Lage des Höhenwertes an der Grundriss-Kote (T0196).
    let mut text_pos: Option<ElevationTextPos> = None;
    // Lage der Maßlinie einer Maßkette (Abstand zu den Basispunkten).
    let mut abstand: Option<f64> = None;
    // Bild: Breite (untere Kante), Höhe und die Sperre des Seitenverhältnisses.
    let mut bild_breite: Option<f64> = None;
    let mut bild_hoehe: Option<f64> = None;
    let mut bild_lock: Option<bool> = None;
    // Punktsymbole (T0210): `Some(None)` = kein Symbol gewählt.
    let mut symbol_start: Option<Option<String>> = None;
    let mut symbol_end: Option<Option<String>> = None;

    match art {
        // Punktsymbole (T0210): ein Punkt trägt eines, eine Linie je eines an
        // Anfang und Ende. Die Auswahl kommt aus `Document::punktsymbol_bloecke`
        // — System-, Projekt- und Benutzerblöcke der Kategorie „Punkt Symbole"
        // stehen gleichberechtigt darin.
        Einfach::Punkt | Einfach::Linie => {
            let (cur_start, cur_end) = match &e {
                Some(e) => (e.start_symbol.clone(), e.end_symbol.clone()),
                None => (
                    app.options.symbol_start.clone(),
                    app.options.symbol_end.clone(),
                ),
            };
            zeile("symbol");
            let mut neu_start = None;
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.punktsymbol"), |ui| {
                neu_start = punktsymbol_wahl(app, ui, &ziel.salt("punkt_symbol"), cur_start.as_deref());
            });
            if neu_start.is_some() {
                symbol_start = neu_start;
            }
            if matches!(art, Einfach::Linie) {
                zeile("symbol_ende");
                let mut neu_ende = None;
                widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.punktsymbol_ende"), |ui| {
                    neu_ende = punktsymbol_wahl(app, ui, &ziel.salt("punkt_symbol_ende"), cur_end.as_deref());
                });
                if neu_ende.is_some() {
                    symbol_end = neu_ende;
                }
            }
        }
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
        // Grundriss-Kote: Höhe und Lage des Höhenwertes sind die Eingaben des
        // Nutzers (T0196).
        Einfach::Grundrisskote => {
            let (z0, pos0) = match &e {
                Some(e) => match &e.kind {
                    EntityKind::ElevationPlan { z, text_pos, .. } => (*z, *text_pos),
                    _ => (0.0, ElevationTextPos::default()),
                },
                None => (app.options.elevation_z, app.options.elevation_text_pos),
            };
            hoehe = Some(z0);
            text_pos = Some(pos0);
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
            // Lage des Höhenwertes am Symbol — dieselbe Zeile beim Platzieren
            // und bei der Auswahl.
            zeile("textposition");
            let items: Vec<(ElevationTextPos, String)> = ElevationTextPos::ALL
                .iter()
                .map(|p| (*p, p.text().to_string()))
                .collect();
            let mut picked = None;
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.textposition"), |ui| {
                picked = panels::pick_combo(ui, &ziel.salt("kote_textposition"), pos0, &items);
            });
            if let Some(v) = picked {
                text_pos = Some(v);
            }
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
        // Bild: Dateiname und die Größe, die aus den beiden Ecken folgt.
        // Beides ist Anzeige — die Größe bestimmt der aufgezogene Rahmen
        // (zwei Klicks), ein Zahlenfeld würde nur eine zweite Wahrheit bauen.
        Einfach::Bild => {
            let (name, pixel, breite, hoehe, gesperrt) = match &e {
                Some(e) => match &e.kind {
                    EntityKind::Image {
                        a,
                        b,
                        height,
                        lock_ratio,
                        name,
                        data,
                    } => {
                        let w = a.dist(*b);
                        let h = jjcad_core::bild::dekodiere(data)
                            .map(|bild| {
                                jjcad_core::bild::wirksame_hoehe(&bild, w, *height, *lock_ratio)
                            })
                            .unwrap_or(*height);
                        (name.clone(), bild_pixel(data), w, h, *lock_ratio)
                    }
                    _ => (String::new(), None, 0.0, 0.0, true),
                },
                None => match &app.options.bild {
                    Some(q) => (
                        q.name.clone(),
                        bild_pixel(&q.data),
                        0.0,
                        0.0,
                        app.options.bild_lock,
                    ),
                    None => (
                        jjcad_core::i18n::tr("gui.props.keine_datei").to_string(),
                        None,
                        0.0,
                        0.0,
                        app.options.bild_lock,
                    ),
                },
            };
            zeile("datei");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.datei"), |ui| {
                ui.label(theme::mono(name.clone()));
            });
            // Die Pixelgröße gehört zur Datei (Auflösung der Vorlage) und
            // steht deshalb als eigene, kleinere Zeile darunter — in der
            // Wertespalte wäre der Name abgeschnitten.
            if let Some(px) = &pixel {
                ui.label(theme::dim(px.clone()));
            }
            // Breite = Länge der unteren Kante. Sie ist die eine Eingabe; die
            // Höhe folgt bei gesperrtem Verhältnis daraus.
            zeile("breite");
            if e.is_none() {
                gesperrt_zeile(
                    ui,
                    jjcad_core::i18n::tr("gui.props.breite"),
                    jjcad_core::i18n::tr("gui.props.aus_den_klicks"),
                );
            } else {
                let mut v = breite;
                widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.breite"), |ui| {
                    if widgets::length_field(
                        ui,
                        &mut v,
                        unit,
                        110.0,
                        widgets::MASS_MIN_MM,
                        widgets::MASS_MAX_MM,
                    ) {
                        bild_breite = Some(v);
                    }
                });
            }
            // Höhe: bei gesperrtem Verhältnis nur Anzeige, sonst Eingabe.
            zeile("hoehe");
            if e.is_none() {
                gesperrt_zeile(
                    ui,
                    jjcad_core::i18n::tr("gui.props.hoehe"),
                    jjcad_core::i18n::tr("gui.props.aus_den_klicks"),
                );
            } else if gesperrt {
                widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
                    ui.label(theme::mono(unit.format(hoehe, unit.decimals())));
                });
            } else {
                let mut v = hoehe;
                widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.hoehe"), |ui| {
                    if widgets::length_field(
                        ui,
                        &mut v,
                        unit,
                        110.0,
                        widgets::MASS_MIN_MM,
                        widgets::MASS_MAX_MM,
                    ) {
                        bild_hoehe = Some(v);
                    }
                });
            }
            zeile("seitenverhaeltnis");
            let mut lock = gesperrt;
            widgets::field_row(
                ui,
                jjcad_core::i18n::tr("gui.props.seitenverhaeltnis"),
                |ui| {
                    if ui
                        .checkbox(&mut lock, jjcad_core::i18n::tr("gui.props.sperren"))
                        .on_hover_text(jjcad_core::i18n::tr(
                            "gui.props.hoehe_folgt_aus_breite_und_dateiverhaeltnis",
                        ))
                        .changed()
                    {
                        bild_lock = Some(lock);
                    }
                },
            );
        }
        Einfach::Block => {
            // T0204: Ein Block zeigt **seine** Eigenschaften — Name und Gruppe
            // der Blockdefinition, nicht nur Drehung und Faktor der Instanz.
            // Beim Platzieren stehen sie in den Vorgaben (`options.block_id`),
            // bei der Auswahl kommt die Kennung aus dem Objekt. Die Gruppe ist
            // die Überschrift in der Blockbibliothek.
            let block_id = match &e {
                Some(e) => match &e.kind {
                    EntityKind::BlockRef { block, .. } => Some(block.clone()),
                    _ => None,
                },
                None => app.options.block_id.clone(),
            };
            let definition = block_id.as_deref().and_then(|id| app.doc().block(id));
            let block_name = definition
                .map(|b| b.name.clone())
                .or_else(|| block_id.clone())
                .unwrap_or_else(|| "—".to_string());
            let block_gruppe = definition
                .map(|b| b.category.clone())
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| "—".to_string());
            zeile("blockname");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.blockname"), |ui| {
                ui.label(theme::mono(block_name.clone()));
            });
            zeile("gruppe");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.gruppe"), |ui| {
                ui.label(theme::mono(block_gruppe.clone()));
            });
            // T0231: die Größe gehört zur Definition — geändert wird sie im
            // Fenster „Block-Eigenschaften“ (Rechtsklick auf die Kachel).
            let block_groesse = definition.map(|b| b.groesse).unwrap_or_default();
            zeile("groesse");
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.groesse"), |ui| {
                ui.label(theme::mono(block_groesse.text()));
            });

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
        }
        _ => {
            ui.label(theme::faint(match art {
                Einfach::Kreis => jjcad_core::i18n::tr("gui.props.radius_und_mittelpunkt_entstehen_aus_den_klicks"),
                Einfach::Bogen => jjcad_core::i18n::tr("gui.props.radius_und_winkel_entstehen_aus_den_klicks"),
                Einfach::Linie => jjcad_core::i18n::tr("gui.props.laenge_und_winkel_entstehen_aus_den_klicks"),
                Einfach::Polylinie
                | Einfach::Spline
                | Einfach::Rechteck
                | Einfach::Pfad => {
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
            if let Some(v) = text_pos {
                app.options.elevation_text_pos = v;
            }
            if let Some(v) = drehung {
                app.options.block_rot = v;
            }
            if let Some(v) = faktor {
                app.options.block_scale = v;
            }
            if let Some(v) = bild_lock {
                app.options.bild_lock = v;
            }
            if let Some(v) = symbol_start {
                app.options.symbol_start = v;
            }
            if let Some(v) = symbol_end {
                app.options.symbol_end = v;
            }
        }
        Ziel::Auswahl(id) => {
            gemein_nach_objekt(app, id, &a);
            let mut params = serde_json::json!({"ids": [id]});
            let mut geaendert = false;
            if let Some(map) = params.as_object_mut() {
                if let Some(e) = &e {
                    // Punktsymbole (T0210): `null` nimmt das Symbol weg.
                    if let Some(v) = &symbol_start {
                        if v.as_deref() != e.start_symbol.as_deref() {
                            map.insert("start_symbol".into(), serde_json::json!(v));
                            geaendert = true;
                        }
                    }
                    if let Some(v) = &symbol_end {
                        if v.as_deref() != e.end_symbol.as_deref() {
                            map.insert("end_symbol".into(), serde_json::json!(v));
                            geaendert = true;
                        }
                    }
                    // Ellipsenbogen: Achsen, Drehlage und beide Parameterwinkel
                    // (T0084). Geht vor der gemeinsamen Schleife, weil hier fünf
                    // Werte zusammen gehören — keine andere Art nutzt sie.
                    // Grundriss-Kote: Lage des Höhenwertes (T0196). Steht vor
                    // der Werteschleife, weil sie kein Zahlenwert ist.
                    if let EntityKind::ElevationPlan { text_pos: alt, .. } = &e.kind {
                        if let Some(v) = text_pos {
                            if v != *alt {
                                map.insert("text_pos".into(), serde_json::json!(v.key()));
                                geaendert = true;
                            }
                        }
                    }
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
                        // Bild: Breite über die untere Kante, Höhe (nur ohne
                        // Sperre) und die Sperre selbst.
                        (
                            EntityKind::Image {
                                a,
                                b,
                                height,
                                lock_ratio,
                                ..
                            },
                            _,
                            _,
                            _,
                            _,
                            _,
                            _,
                        ) => {
                            if let Some(v) = bild_breite {
                                if (v - a.dist(*b)).abs() > 1e-6 {
                                    map.insert("width".into(), serde_json::json!(unit.from_mm(v)));
                                    geaendert = true;
                                }
                            }
                            if let Some(v) = bild_hoehe {
                                if (v - *height).abs() > 1e-6 {
                                    map.insert("height".into(), serde_json::json!(unit.from_mm(v)));
                                    geaendert = true;
                                }
                            }
                            if let Some(v) = bild_lock {
                                if v != *lock_ratio {
                                    map.insert("lock_ratio".into(), serde_json::json!(v));
                                    geaendert = true;
                                }
                            }
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

    let (
        layer,
        farbe,
        mut hoehe,
        mut muster,
        mut winkel,
        mut abstand,
        mut rand,
        mut fuellfarbe,
        mut kein_fill,
    ) = match &e {
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
                e.style.no_fill,
            ),
            _ => return,
        },
        None => (
            app.werkzeug_ziel_ebene("Architektur"),
            app.ui.pending.color,
            app.options.volume_height,
            app.options.volume_pattern,
            app.options.volume_angle.to_degrees(),
            app.options.volume_scale,
            app.options.fill_border_for(&werkzeug),
            app.ui.pending.fill,
            app.ui.pending.no_fill,
        ),
    };
    let farbe_gezeigt = farbe.unwrap_or_else(|| {
        app.doc()
            .layer(layer)
            .map(|l| l.color)
            .unwrap_or(Color::BLACK)
    });
    let (staerke, linientyp) = linienwerte(app, &e, "Architektur");

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.volumen"), "volume");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.koerper"));
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
        // Muster und Fläche sind getrennt (T0242): „Keine Füllung“ nimmt nur
        // den Flächenton weg, das Muster bleibt.
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
    // Drei Zustände (T0242): keine Füllung, Wandton oder eine Farbe.
    let fill_cur = fuellfarbe
        .unwrap_or_else(|| farbe_gezeigt.mix(Color::WHITE, jjcad_core::render::WALL_FILL_MIX));
    let wahl = jjcad_core::style::Fuellung::aus_style(fuellfarbe, kein_fill);
    let mut picked_fill: Option<jjcad_core::style::Fuellung> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fuellfarbe"), |ui| {
        picked_fill = panels::fuellung_combo(
            ui,
            &ziel.salt("volumen_farbe"),
            wahl,
            fill_cur,
            jjcad_core::i18n::tr("gui.props.wandton"),
        );
    });
    if let Some(v) = picked_fill {
        (fuellfarbe, kein_fill) = v.in_style();
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
            app.ui.pending.fill = fuellfarbe;
            app.ui.pending.no_fill = kein_fill;
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
                    if fuellfarbe != e.style.fill || kein_fill != e.style.no_fill {
                        map.insert(
                            "fill".into(),
                            serde_json::json!(fuell_param(fuellfarbe, kein_fill)),
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
            app.werkzeug_ziel_ebene("Architektur"),
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
    let (staerke, linientyp) = linienwerte(app, &e, "Architektur");
    // Ohne Länge (beim Erzeugen) gilt die Vorgabe der Action als Rechengrundlage.
    let rechen_laenge = if laenge > 1.0 { laenge } else { 4640.0 };
    let mass = jjcad_core::stair::treppenmass(rechen_laenge, hoehe, stuen, auto);

    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.treppe"), "stair");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
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

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.treppenlauf"));
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
                teil_aenderung.push((format!("type_{praefix}"), serde_json::json!(v.key())));
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
/// Ansichtsfenster im Papierraum (T0109).
///
/// Es ist ein normales Blattobjekt: dieselben Zeilen beim Aufziehen und beim
/// Bearbeiten. Die Papierlage steht in Millimetern (das Blatt ist in mm
/// gezeichnet), der Maßstab ist das Verhältnis Zeichnung : Papier.
pub fn viewport(app: &mut App, ui: &mut Ui, ziel: Ziel) {
    let e: Option<Entity> = ziel.id().and_then(|id| app.doc().entity(id).cloned());
    let vp = match &e {
        Some(e) => match &e.kind {
            EntityKind::Viewport(vp) => Some(vp.clone()),
            _ => None,
        },
        None => None,
    };
    kopf(app, ui, ziel, jjcad_core::i18n::tr("gui.props.ansichtsfenster"), "viewport");
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, ziel);

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.ausschnitt"));
    zeile("artkopf");

    let gesperrt = vp.is_none();
    let (mut name, mut scale, mut x, mut y, mut w, mut h, mut rahmen) = match &vp {
        Some(v) => (v.name.clone(), v.scale, v.x, v.y, v.w, v.h, v.show_frame),
        None => (String::new(), 100.0, 0.0, 0.0, 100.0, 100.0, true),
    };

    zeile("name");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.name"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.name"), |ui| {
            ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut name));
        });
    }

    zeile("massstab");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.massstab"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.massstab"), |ui| {
            ui.add_sized([70.0, 20.0], widgets::zahl(&mut scale, 1.0, 1.0..=1000.0));
        });
    }

    zeile("lage");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.x_y_mm"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.x_y_mm"), |ui| {
            ui.add_sized([52.0, 20.0], widgets::zahl(&mut x, 1.0, widgets::KOORD_MIN_MM..=widgets::KOORD_MAX_MM));
            ui.add_sized([52.0, 20.0], widgets::zahl(&mut y, 1.0, widgets::KOORD_MIN_MM..=widgets::KOORD_MAX_MM));
        });
    }

    zeile("groesse");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.b_h_mm"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.b_h_mm"), |ui| {
            ui.add_sized([52.0, 20.0], widgets::zahl(&mut w, 0.5, 10.0..=2000.0));
            ui.add_sized([52.0, 20.0], widgets::zahl(&mut h, 0.5, 10.0..=2000.0));
        });
    }

    zeile("rahmen");
    if gesperrt {
        gesperrt_zeile(ui, jjcad_core::i18n::tr("gui.props.fensterrahmen_zeigen"), jjcad_core::i18n::tr("gui.props.aus_den_klicks"));
    } else {
        widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.fensterrahmen_zeigen"), |ui| {
            ui.checkbox(&mut rahmen, "");
        });
    }

    // T0158: Ansichtsbereich einstellen — ein **Schalter**, kein einmaliger
    // Befehl. Solange er an ist, gehören Mausrad und Mitteltaste dem Fenster
    // (`canvas::vp_navigation`): Rad ändert den Maßstab, Mitteltaste schiebt
    // den gezeigten Zeichnungsausschnitt.
    zeile("bereich");
    if let Some(id) = ziel.id() {
        let an = app.ui.vp_edit == Some(id);
        let antwort = ui.add_sized(
            [ui.available_width(), 24.0],
            egui::SelectableLabel::new(
                an,
                jjcad_core::i18n::tr("gui.props.ansichtsbereich_anpassen"),
            ),
        );
        if antwort
            .on_hover_text(jjcad_core::i18n::tr("gui.props.ansichtsbereich_tip"))
            .clicked()
        {
            app.ui.vp_edit = if an { None } else { Some(id) };
        }
    } else {
        // Beim Aufziehen gibt es noch kein Fenster — der Schalter steht
        // deaktiviert an derselben Stelle (die Zeile fehlt nie).
        ui.add_enabled(
            false,
            egui::SelectableLabel::new(
                false,
                jjcad_core::i18n::tr("gui.props.ansichtsbereich_anpassen"),
            ),
        );
    }

    zeile("aktionen");
    if let Some(id) = ziel.id() {
        ui.horizontal(|ui| {
            if ui.button(jjcad_core::i18n::tr("gui.panels.auf_zeichnung_zentrieren")).clicked() {
                let c = app.doc().bbox();
                let u = app.unit();
                if c.is_valid() {
                    let cx = u.from_mm(c.center().x);
                    let cy = u.from_mm(c.center().y);
                    setzen(app, &serde_json::json!({"ids": [id], "center": [cx, cy]}));
                }
            }
            if ui.button(jjcad_core::i18n::tr("gui.panels.entfernen")).clicked() {
                let _ = app
                    .session_mut()
                    .execute("modify.erase", &serde_json::json!({"ids": [id]}));
                app.session_mut().clear_selection();
                app.mark_changed();
            }
        });
    } else {
        ui.label(theme::faint(jjcad_core::i18n::tr("gui.props.aus_den_klicks")));
    }

    // T0238: Angezeigte Layer. Eine Liste **aller** Layer der Zeichnung, nach
    // Plänen gruppiert, jede Zeile mit Häkchen. Die Checkbox eines Plans
    // schaltet alle seine Layer gemeinsam. `None` am Objekt heißt „alle
    // sichtbaren“ — die Liste zeigt dann überall ein Häkchen.
    abschnitt(ui, jjcad_core::i18n::tr("gui.props.layer"));
    zeile("layer");
    let alle_layer: Vec<Id> = app.doc().layers.iter().map(|l| l.id).collect();
    let gewaehlt = match &vp {
        Some(v) => viewport_layer(v, &alle_layer),
        // Beim Aufziehen gibt es noch kein Objekt: ein neues Fenster zeigt alle.
        None => alle_layer.clone(),
    };
    let zeilen: Vec<(String, Vec<(Id, String)>)> = app
        .doc()
        .plans
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                app.doc()
                    .layers_of_plan(p.id)
                    .iter()
                    .map(|l| (l.id, l.name.clone()))
                    .collect(),
            )
        })
        .collect();
    let mut wahl: Option<Vec<Id>> = None;
    // Keine eigene Scrollfläche: das Panel scrollt bereits, zwei ineinander
    // verschachtelte Radflächen wären eine Falle (T0222).
    for (plan_name, layer) in &zeilen {
        if layer.is_empty() {
            continue;
        }
        let plan_layer: Vec<Id> = layer.iter().map(|(id, _)| *id).collect();
        let mut an = plan_layer.iter().all(|id| gewaehlt.contains(id));
        let antwort = ui.add_enabled(
            !gesperrt,
            egui::Checkbox::new(&mut an, egui::RichText::new(plan_name).strong()),
        );
        if antwort.changed() {
            wahl = Some(plan_layer_umschalten(&gewaehlt, &plan_layer));
        }
        ui.indent(("vp_layer", plan_name.as_str()), |ui| {
            for (id, name) in layer {
                let mut an = gewaehlt.contains(id);
                let antwort =
                    ui.add_enabled(!gesperrt, egui::Checkbox::new(&mut an, name.as_str()));
                if antwort.changed() {
                    wahl = Some(layer_umschalten(&gewaehlt, *id));
                }
            }
        });
    }
    if let (Some(id), Some(neu)) = (ziel.id(), wahl) {
        // Zeigt das Fenster wieder alle Layer, steht `null` in der Datei —
        // so erscheinen später angelegte Layer von selbst.
        let wert = if neu.len() == alle_layer.len() {
            serde_json::Value::Null
        } else {
            serde_json::json!(neu)
        };
        setzen(app, &serde_json::json!({"ids": [id], "layers": wert}));
    }

    if let Some(id) = ziel.id() {
        let mut params = serde_json::json!({"ids": [id]});
        let mut geaendert = false;
        if let Some(v) = &vp {
            if name != v.name {
                params["name"] = serde_json::json!(name);
                geaendert = true;
            }
            if (scale - v.scale).abs() > 1e-9 {
                params["scale"] = serde_json::json!(scale);
                geaendert = true;
            }
            if (x - v.x).abs() > 1e-9 || (y - v.y).abs() > 1e-9 {
                params["x"] = serde_json::json!(x);
                params["y"] = serde_json::json!(y);
                geaendert = true;
            }
            if (w - v.w).abs() > 1e-9 || (h - v.h).abs() > 1e-9 {
                params["w"] = serde_json::json!(w);
                params["h"] = serde_json::json!(h);
                geaendert = true;
            }
            if rahmen != v.show_frame {
                params["show_frame"] = serde_json::json!(rahmen);
                geaendert = true;
            }
        }
        if geaendert {
            setzen(app, &params);
        }
    }
}

/// Angezeigte Layer eines Ansichtsfensters (T0238).
///
/// `Viewport::layers` mit `None` heißt „alle sichtbaren Layer“; für die
/// Häkchenliste wird daraus die Liste **aller** Layer der Zeichnung. Die
/// Reihenfolge folgt der Zeichnung, nicht der gespeicherten Liste.
pub fn viewport_layer(vp: &jjcad_core::model::Viewport, alle: &[Id]) -> Vec<Id> {
    match &vp.layers {
        Some(list) => alle.iter().copied().filter(|id| list.contains(id)).collect(),
        None => alle.to_vec(),
    }
}

/// Klick auf die Checkbox **eines Plans** im Ansichtsfenster (T0238): Sind
/// alle seine Layer gezeigt, nimmt der Klick sie alle heraus; sonst ergänzt er
/// die fehlenden. Ein Klick auf den Plan schaltet damit die ganze Gruppe.
pub fn plan_layer_umschalten(gewaehlt: &[Id], plan_layer: &[Id]) -> Vec<Id> {
    if plan_layer.iter().all(|id| gewaehlt.contains(id)) {
        gewaehlt
            .iter()
            .copied()
            .filter(|id| !plan_layer.contains(id))
            .collect()
    } else {
        let mut neu = gewaehlt.to_vec();
        for id in plan_layer {
            if !neu.contains(id) {
                neu.push(*id);
            }
        }
        neu
    }
}

/// Klick auf die Checkbox **eines Layers** im Ansichtsfenster (T0238).
pub fn layer_umschalten(gewaehlt: &[Id], id: Id) -> Vec<Id> {
    let mut neu = gewaehlt.to_vec();
    if let Some(pos) = neu.iter().position(|x| *x == id) {
        neu.remove(pos);
    } else {
        neu.push(id);
    }
    neu
}

/// Hat diese Objektart eine Linie (Farbe, Stärke, Linientyp)?
///
/// Text, Raumstempel, Block, Bild und Ansichtsfenster haben keine — bei ihnen
/// sähen Stärke und Linientyp aus, als ließen sie sich einstellen, ohne dass
/// sich am Bild etwas ändert.
fn hat_linie(kind: &EntityKind) -> bool {
    !matches!(
        kind,
        EntityKind::Text { .. }
            | EntityKind::Room { .. }
            | EntityKind::BlockRef { .. }
            | EntityKind::Image { .. }
            | EntityKind::Viewport { .. }
    )
}

/// Die Auswahl auf die Objekte **einer** Objektart setzen (T0194) — der Weg
/// des Art-Filters im Eigenschaften-Panel.
///
/// Die übrigen Arten fallen aus der Auswahl; das Panel zeigt danach den
/// Abschnitt dieser Art. `App::sync_space` zieht den Schalter „Ansichtsbereich
/// anpassen“ nach, der am ausgewählten Ansichtsfenster hängt.
pub fn auswahl_auf_art(app: &mut App, ids: Vec<Id>) {
    app.session_mut().select(ids);
    app.sync_space();
}

/// Eigenschaften einer Auswahl aus **mehreren Objektarten** (T0193).
///
/// Gezeigt wird nur, was **alle** Objekte gemeinsam haben: Reihenfolge, Layer
/// und Farbe, dazu Stärke und Linientyp, sofern jede Art eine Linie besitzt.
/// Die art-eigenen Werte (Dicke, Radius, Schraffur, Schrift …) bleiben weg —
/// für ein Objekt, das sie nicht hat, gäbe es nichts zu ändern, und die lange
/// Liste je Art war genau das, was hier gestört hat.
///
/// Geändert wird über [`gemein_nach_objekt`] → [`setzen`] an **alle** Kennungen:
/// dafür steht `App::mehrfach_ziel` während des Zeichnens auf der ganzen
/// Auswahl.
fn mehrfach_gemeinsam(
    app: &mut App,
    ui: &mut Ui,
    ids: &[Id],
    arten: &[(std::mem::Discriminant<EntityKind>, Vec<Id>)],
) {
    let vorher = std::mem::take(&mut app.mehrfach_ziel);
    app.mehrfach_ziel = ids.to_vec();

    section_title(ui, "select", jjcad_core::i18n::tr("gui.props.mehrfachauswahl"));
    ui.label(theme::dim(jjcad_core::i18n::t_args_owned(
        "gui.props.x_objekte_x_arten",
        &[
            ("anzahl", format!("{}", ids.len())),
            ("wert1", format!("{}", arten.len())),
        ],
    )));

    // T0194: Die Auswahl auf **eine** Objektart einschränken. Die Liste zeigt
    // genau die Arten, die gerade in der Auswahl liegen; die Wahl setzt die
    // Auswahl auf deren Objekte, die übrigen Arten fallen heraus. Danach
    // zeichnet das Panel den Abschnitt dieser Art (bei mehreren Objekten den
    // Mehrfachabschnitt, bei einem das Objekt selbst).
    zeile("art_filter");
    let namen: Vec<String> = arten
        .iter()
        .map(|(_, liste)| {
            app.doc()
                .entity(liste[0])
                .map(|e| e.kind.type_name().to_string())
                .unwrap_or_else(|| "—".to_string())
        })
        .collect();
    let mut gewaehlt: Option<usize> = None;
    widgets::field_row(ui, jjcad_core::i18n::tr("gui.props.art"), |ui| {
        gewaehlt = panels::dropdown(
            ui,
            "mehrfach_art",
            jjcad_core::i18n::tr("gui.props.alle_arten"),
            None,
            |ui| {
                let mut out = None;
                for (i, name) in namen.iter().enumerate() {
                    if ui.selectable_label(false, name).clicked() {
                        out = Some(i);
                    }
                }
                out
            },
        );
    });
    if let Some(i) = gewaehlt {
        if let Some((_, liste)) = arten.get(i) {
            auswahl_auf_art(app, liste.clone());
        }
    }
    ui.add_space(6.0);

    let erste = ids[0];
    let e: Option<Entity> = app.doc().entity(erste).cloned();
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
    let (staerke, linientyp) = linienwerte(app, &e, "Linie");
    // Nur wenn **jede** Art eine Linie hat, sind Stärke und Linientyp
    // gemeinsame Eigenschaften.
    let mit_linie = ids.iter().all(|id| {
        app.doc()
            .entity(*id)
            .map(|e| hat_linie(&e.kind))
            .unwrap_or(false)
    });

    abschnitt(ui, jjcad_core::i18n::tr("gui.props.allgemein"));
    reihenfolge(app, ui, Ziel::Auswahl(erste));
    let a = gemein(
        app,
        ui,
        Ziel::Auswahl(erste),
        layer,
        farbe,
        farbe_gezeigt,
        staerke,
        linientyp,
        mit_linie,
        jjcad_core::i18n::tr("gui.props.farbe"),
    );
    // `setzen` schickt an alle Kennungen in `mehrfach_ziel` — also an jedes
    // Objekt der Auswahl, unabhängig von seiner Art.
    gemein_nach_objekt(app, erste, &a);

    app.mehrfach_ziel = vorher;
}

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

    // Mehrere Arten: nur die gemeinsamen Zeilen (T0193).
    if gruppen.len() > 1 {
        mehrfach_gemeinsam(app, ui, ids, &gruppen);
    } else {
        section_title(ui, "select", jjcad_core::i18n::tr("gui.props.mehrfachauswahl"));

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
                Some(EntityKind::DimRadius { .. }) => radiusbemassung(app, ui, ziel),
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
    }

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

    /// Alle **gemalten** Texte eines Panelbildes in Malreihenfolge.
    ///
    /// Anders als [`zeichne`] prüft das die Anzeige statt nur das Protokoll:
    /// so lässt sich belegen, was der Nutzer im Panel liest und in welcher
    /// Reihenfolge es untereinander steht.
    fn texte(app: &mut App) -> Vec<String> {
        fn sammle(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(list) => {
                    for s in list {
                        sammle(s, out);
                    }
                }
                _ => {}
            }
        }
        let ctx = app.ctx.clone();
        let mut einmal = false;
        let ausgabe = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if !einmal {
                    einmal = true;
                    crate::panels::properties(app, ui);
                }
            });
        });
        let mut out = Vec::new();
        for gezeichnet in &ausgabe.shapes {
            sammle(&gezeichnet.shape, &mut out);
        }
        out
    }

    /// T0242: Der Zustand „keine Füllung“ steht als eigener Wert im Panel —
    /// und ist nicht mit „Standard“ (Vorgabeton) zu verwechseln.
    #[test]
    fn keine_fuellung_wird_im_panel_angezeigt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit Millimeter");
        let r = app
            .session_mut()
            .execute(
                "draw.wall",
                &json!({
                    "a": [0.0, 0.0],
                    "b": [4000.0, 0.0],
                    "thickness": 300.0
                }),
            )
            .expect("Wand anlegen");
        let id = r["created"][0].as_u64().expect("ID");
        app.session_mut().selection = vec![id];

        let vorher = texte(&mut app);
        assert!(
            vorher.iter().any(|t| t == "Standard"),
            "Vorgabe zeigt „Standard“: {vorher:?}"
        );

        app.session_mut()
            .execute("modify.set", &json!({ "ids": [id], "fill": "keine" }))
            .expect("keine Füllung setzen");

        let nachher = texte(&mut app);
        assert!(
            nachher.iter().any(|t| t == "Keine Füllung"),
            "Panel nennt den Zustand: {nachher:?}"
        );
        assert!(
            !nachher.iter().any(|t| t == "Standard"),
            "und nicht mehr „Standard“: {nachher:?}"
        );
    }

    /// Objekteigene Gruppe je Art — die erwartete Beschriftung, hier bewusst
    /// als eigener Text und nicht aus dem Panel abgeleitet.
    fn gruppentitel(kind: &EntityKind) -> &'static str {
        match kind {
            EntityKind::Wall { .. } => "Wandaufbau",
            EntityKind::Door { .. } => "Türblatt",
            EntityKind::Window { .. } => "Fenstermaße",
            EntityKind::Fill { .. } => "Füllung",
            EntityKind::Dim { .. } | EntityKind::DimRadius { .. } => "Maß",
            EntityKind::Room { .. } => "Beschriftung",
            EntityKind::Volume { .. } => "Körper",
            EntityKind::Stair { .. } => "Treppenlauf",
            EntityKind::Text { .. } => "Schrift und Inhalt",
            EntityKind::Viewport(_) => "Ausschnitt",
            EntityKind::Point { .. } | EntityKind::Line { .. } => "Symbole",
            EntityKind::BlockRef { .. } => "Blockdaten",
            EntityKind::Image { .. } => "Bilddaten",
            _ => "Geometrie",
        }
    }

    /// Prüft an einem gezeichneten Panel der Auswahl: Objektkopf ganz oben,
    /// danach „Allgemein“, danach die objekteigene Gruppe mit eigenem Namen.
    fn pruefe_kopf(app: &mut App, tool: &str, gruppe: &str) {
        let texte = texte(app);
        let ueberschrift = jjcad_core::i18n::tr("gui.panels.eigenschaften");

        // Der Kopf ist das erste, was unter der Panel-Überschrift steht.
        let pos_ueber = texte
            .iter()
            .position(|t| t == ueberschrift)
            .unwrap_or_else(|| panic!("{tool}: Panel-Überschrift fehlt: {texte:?}"));
        let name = texte
            .get(pos_ueber + 1)
            .cloned()
            .unwrap_or_else(|| panic!("{tool}: kein Objektkopf: {texte:?}"));
        assert_ne!(name, ueberschrift, "{tool}: Kopf fehlt: {texte:?}");
        let pos_kopf = pos_ueber + 1;
        assert_ne!(gruppe, name, "{tool}: Gruppe heißt wie der Objektkopf");

        let pos_allgemein = texte
            .iter()
            .position(|t| t == "Allgemein")
            .unwrap_or_else(|| panic!("{tool}: „Allgemein“ fehlt: {texte:?}"));
        // Der Kopfbereich (bis „Allgemein“) nennt das Objekt genau einmal:
        // Kopf, Herkunft, sonst nichts.
        assert_eq!(
            texte[..pos_allgemein].iter().filter(|t| **t == name).count(),
            1,
            "{tool}: „{name}“ steht mehrfach über den Gruppen: {texte:?}"
        );
        let pos_gruppe = texte
            .iter()
            .position(|t| t == gruppe)
            .unwrap_or_else(|| panic!("{tool}: Gruppe „{gruppe}“ fehlt: {texte:?}"));
        assert!(
            pos_kopf < pos_allgemein && pos_allgemein < pos_gruppe,
            "{tool}: Reihenfolge Kopf · Allgemein · Gruppe stimmt nicht: {texte:?}"
        );
    }

    /// T0214: Symbol und Bezeichnung stehen **einmal** ganz oben; darunter
    /// gliedern zwei Gruppen mit eigenen Namen — „Allgemein“ für die Werte
    /// jedes Objekts und die objekteigene Gruppe nach ihrem Inhalt. Vorher
    /// trug die zweite Gruppe denselben Titel wie der Kopf, im Panel stand der
    /// Objektname also zweimal untereinander.
    #[test]
    fn kopf_steht_oben_und_die_gruppen_haben_eigene_namen() {
        for (tool, kind) in erzeuger_faelle() {
            let gruppe = gruppentitel(&kind);
            let mut app = App::with_ctx(egui::Context::default());
            app.session_mut()
                .execute("doc.settings", &json!({ "input_unit": "m" }))
                .expect("Einheit");
            let layer = app.doc().current_layer;
            let id = app.doc_mut().new_id();
            app.doc_mut().add_entity(Entity::new(id, layer, kind));
            app.session_mut().selection = vec![id];
            pruefe_kopf(&mut app, tool, gruppe);
        }

        // Wand, Tür, Fenster, Fläche und Text stehen nicht in der Tabelle
        // (sie haben eigene Zeilentests), gehören aber zu derselben Zusage.
        let (mut app, tuer, fenster) = wand_mit_oeffnung();
        let wand = app
            .doc()
            .ents()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Wall { .. }))
            .map(|e| e.id)
            .expect("Wand fehlt");
        for (id, gruppe) in [(wand, "Wandaufbau"), (tuer, "Türblatt"), (fenster, "Fenstermaße")] {
            app.session_mut().selection = vec![id];
            pruefe_kopf(&mut app, "Wand mit Öffnung", gruppe);
        }

        let (mut app, text) = text_app();
        app.session_mut().selection = vec![text];
        pruefe_kopf(&mut app, "draw.text", "Schrift und Inhalt");

        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let flaeche = app
            .session_mut()
            .execute(
                "draw.fill",
                &json!({"boundary": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]], "border": true}),
            )
            .expect("Füllfläche")["created"][0]
            .as_u64()
            .expect("Id");
        app.session_mut().selection = vec![flaeche];
        pruefe_kopf(&mut app, "draw.fill", "Füllung");
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
        assert!(
            auswahl.contains(&"zeilenabstand"),
            "Auswahl ohne Zeilenabstand: {auswahl:?}"
        );
        for zeile in ["umrandung", "runde_umrandung", "hintergrund"] {
            assert!(
                auswahl.contains(&zeile),
                "Auswahl ohne {zeile}: {auswahl:?}"
            );
        }

        app.session_mut().selection.clear();
        app.start_tool("draw.text");
        let neu = zeichne(&mut app);
        assert!(app.tool.is_some(), "Werkzeug nicht gesetzt");

        assert_eq!(
            neu, auswahl,
            "Erzeugen und Bearbeiten zeigen unterschiedliche Zeilen"
        );
    }

    /// T0210: Punkt und Linie zeigen die Symbolzeilen — beim Erzeugen und bei
    /// der Auswahl dieselben.
    #[test]
    fn punkt_und_linie_zeigen_das_punktsymbol() {
        let mut app = App::with_ctx(egui::Context::default());
        let linie = app
            .session_mut()
            .execute(
                "draw.line",
                &json!({"a": [0.0, 0.0], "b": [2.0, 0.0], "end_symbol": "sys:pkt-pfeil"}),
            )
            .expect("Linie")["created"][0]
            .as_u64()
            .expect("Id");

        app.session_mut().selection = vec![linie];
        let auswahl = zeichne(&mut app);
        assert!(
            auswahl.contains(&"symbol") && auswahl.contains(&"symbol_ende"),
            "Linie ohne Symbolzeilen: {auswahl:?}"
        );

        app.session_mut().selection.clear();
        app.start_tool("draw.line");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Linie: Zeilen weichen beim Erzeugen ab");
        app.tool = None;

        let punkt = app
            .session_mut()
            .execute("draw.point", &json!({"p": [0.0, 3.0]}))
            .expect("Punkt")["created"][0]
            .as_u64()
            .expect("Id");
        app.session_mut().selection = vec![punkt];
        let punkt_auswahl = zeichne(&mut app);
        assert!(
            punkt_auswahl.contains(&"symbol"),
            "Punkt ohne Symbolzeile: {punkt_auswahl:?}"
        );
        assert!(
            !punkt_auswahl.contains(&"symbol_ende"),
            "ein Punkt hat kein Ende: {punkt_auswahl:?}"
        );
    }

    /// T0210: Die Schnellauswahl übernimmt die Symbole des angeklickten
    /// Objekts — sonst entstünde die nächste Linie ohne Marken.
    #[test]
    fn schnellauswahl_uebernimmt_das_punktsymbol() {
        let mut app = App::with_ctx(egui::Context::default());
        let id = app
            .session_mut()
            .execute(
                "draw.line",
                &json!({
                    "a": [0.0, 0.0],
                    "b": [2.0, 0.0],
                    "start_symbol": "sys:pkt-kreuz",
                    "end_symbol": "sys:pkt-pfeil",
                }),
            )
            .expect("Linie")["created"][0]
            .as_u64()
            .expect("Id");
        assert!(vorgaben_aus_objekt(&mut app, id));
        assert_eq!(app.options.symbol_start.as_deref(), Some("sys:pkt-kreuz"));
        assert_eq!(app.options.symbol_end.as_deref(), Some("sys:pkt-pfeil"));
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
        app.start_tool("dim.chain");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Bemaßung: Zeilen weichen ab");
    }

    /// Die Grundriss-Kote zeigt die Lage des Höhenwertes in beiden Fällen
    /// (T0196): beim Platzieren aus den Vorgaben, bei der Auswahl aus dem
    /// Objekt — und `vorgaben_aus_objekt` übernimmt sie mit.
    #[test]
    fn grundrisskote_zeigt_die_textposition() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.session_mut()
            .execute(
                "dim.elevation.plan",
                &json!({"p": [1.0, 1.0], "z": 0.0, "text_pos": "links"}),
            )
            .expect("Kote anlegen");
        let id = app.doc().ents().last().expect("Kote fehlt").id;

        app.session_mut().selection = vec![id];
        let auswahl = zeichne(&mut app);
        assert!(
            auswahl.contains(&"textposition"),
            "Grundriss-Kote ohne Textposition: {auswahl:?}"
        );

        app.session_mut().selection.clear();
        app.start_tool("dim.elevation.plan");
        let neu = zeichne(&mut app);
        assert_eq!(neu, auswahl, "Grundriss-Kote: Zeilen weichen ab");

        assert!(vorgaben_aus_objekt(&mut app, id), "Vorgaben nicht übernommen");
        assert_eq!(
            app.options.elevation_text_pos,
            ElevationTextPos::Left,
            "Vorgabe der Textposition nicht aus dem Objekt übernommen"
        );
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

    /// Alle Objektarten mit dem Werkzeug, das sie erzeugt — eine Tabelle
    /// für die Prüfungen, die über **jede** Art laufen müssen.
    fn erzeuger_faelle() -> Vec<(&'static str, EntityKind)> {
        use jjcad_core::geom::v;
        vec![
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
            EntityKind::Dim {
                pts: vec![jjcad_core::geom::Vec2::ZERO, v(1000.0, 0.0)],
                line_pos: v(500.0, -300.0),
                angle: 0.0,
                override_text: None,
                ext_lines: false,
                symbol: jjcad_core::model::DIM_SYMBOL_VORGABE.to_string(),
                heights: Vec::new(),
            },
        ),
        (
            "dim.radius",
            EntityKind::DimRadius {
                c: jjcad_core::geom::Vec2::ZERO,
                r: 500.0,
                angle: 0.0,
                override_text: None,
            },
        ),
        (
            "dim.elevation.plan",
            EntityKind::ElevationPlan {
                p: jjcad_core::geom::Vec2::ZERO,
                z: 1200.0,
                text_pos: jjcad_core::model::ElevationTextPos::default(),
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
            "draw.image",
            EntityKind::Image {
                a: jjcad_core::geom::Vec2::ZERO,
                b: v(1000.0, 0.0),
                height: 600.0,
                lock_ratio: false,
                name: "Bild".into(),
                data: String::new(),
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
        (
            // Ansichtsfenster: dieselben Zeilen beim Aufziehen und beim
            // Bearbeiten (T0109).
            "sheet.viewport_tool",
            EntityKind::Viewport(jjcad_core::model::Viewport {
                id: 1,
                name: "Ansicht".into(),
                x: 15.0,
                y: 83.0,
                w: 380.0,
                h: 200.0,
                center: jjcad_core::geom::Vec2::ZERO,
                scale: 100.0,
                show_frame: true,
                locked: false,
                layers: None,
            }),
        ),
        ]
    }

    /// Alle übrigen Erzeuger gegen ihr Objekt: gleiche Zeilen, gleiche
    /// Reihenfolge. Das ist die Tabelle, die den Umbau abgesichert hat.
    #[test]
    fn alle_erzeuger_zeigen_dieselben_zeilen_wie_das_objekt() {
        use jjcad_core::geom::v;
        let faelle = erzeuger_faelle();

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

    /// T0204: Ein ausgewählter Block zeigt **seine** Eigenschaften — Name und
    /// Gruppe der Blockdefinition stehen als Zeilen im Panel.
    #[test]
    fn blockpanel_zeigt_name_und_gruppe() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.doc_mut().blocks.push(jjcad_core::model::BlockDef {
            id: "usr:tisch".into(),
            name: "Tisch".into(),
            category: "Möbel".into(),
            source: jjcad_core::model::BlockSource::User,
            description: String::new(),
            entities: Vec::new(),
            icon: None,
            base: jjcad_core::geom::Vec2::ZERO,
            groesse: BlockGroesse::Fest,
        });
        let layer = app.doc().current_layer;
        let id = app.doc_mut().new_id();
        app.doc_mut().add_entity(Entity::new(
            id,
            layer,
            EntityKind::BlockRef {
                block: "usr:tisch".into(),
                pos: jjcad_core::geom::Vec2::ZERO,
                rot: 0.0,
                scale: jjcad_core::geom::v(1.0, 1.0),
                mirror: false,
            },
        ));
        app.session_mut().selection = vec![id];

        let zeilen = zeichne(&mut app);
        assert!(
            zeilen.contains(&"blockname"),
            "Block ohne Namenszeile: {zeilen:?}"
        );
        assert!(
            zeilen.contains(&"gruppe"),
            "Block ohne Gruppenzeile: {zeilen:?}"
        );
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
                    | EntityKind::DimRadius { .. }
                    | EntityKind::Volume { .. }
                    | EntityKind::Stair { .. }
                    | EntityKind::Room { .. }
                    | EntityKind::Viewport(_)
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
            EntityKind::Path {
                segments: vec![],
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
                line_spacing: 1.0,
            frame: false,                            frame_round: false,                            background: None,                            },
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
                pts: vec![
                    jjcad_core::geom::Vec2::ZERO,
                    jjcad_core::geom::v(1000.0, 0.0),
                ],
                line_pos: jjcad_core::geom::v(500.0, -300.0),
                angle: 0.0,
                override_text: None,
                ext_lines: false,
                symbol: jjcad_core::model::DIM_SYMBOL_VORGABE.to_string(),
                heights: Vec::new(),
            },
            EntityKind::DimRadius {
                c: jjcad_core::geom::Vec2::ZERO,
                r: 500.0,
                angle: 0.0,
                override_text: None,
            },
            EntityKind::ElevationPlan {
                p: jjcad_core::geom::Vec2::ZERO,
                z: 0.0,
                text_pos: jjcad_core::model::ElevationTextPos::default(),
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
            EntityKind::Image {
                a: jjcad_core::geom::Vec2::ZERO,
                b: jjcad_core::geom::v(1000.0, 0.0),
                height: 600.0,
                lock_ratio: false,
                name: "Bild".into(),
                data: String::new(),
            },
            EntityKind::Viewport(jjcad_core::model::Viewport {
                id: 1,
                name: "Ansicht".into(),
                x: 15.0,
                y: 83.0,
                w: 380.0,
                h: 200.0,
                center: jjcad_core::geom::Vec2::ZERO,
                scale: 100.0,
                show_frame: true,
                locked: false,
                layers: None,
            }),
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
            &json!({"ids": [a], "thickness": 0.30, "hatch": "mauerwerk"}),
        );
        assert!((wand_dicke(&app, a) - 300.0).abs() < 1e-6, "Wand 1: {}", wand_dicke(&app, a));
        assert!((wand_dicke(&app, b) - 300.0).abs() < 1e-6, "Wand 2: {}", wand_dicke(&app, b));
        assert_eq!(
            wand_schraffur(&app, a),
            Some(jjcad_core::style::FillPattern::Lines)
        );
        assert_eq!(
            wand_schraffur(&app, b),
            Some(jjcad_core::style::FillPattern::Lines)
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
                line_spacing: 1.0,
            frame: false,                            frame_round: false,                            background: None,                            },
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

    /// Auswahl über mehrere Arten: es stehen **nur** die gemeinsamen Zeilen im
    /// Panel (T0193) — Reihenfolge, Layer, Farbe, Stärke, Linientyp. Die
    /// art-eigenen Werte (Dicke, Schraffur, Geometrie der Linie) fehlen.
    #[test]
    fn gemischte_auswahl_zeigt_nur_gemeinsame_zeilen() {
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

        for gemeinsame in ["reihenfolge", "layer", "farbe", "staerke", "linientyp"] {
            assert!(
                zeilen.contains(&gemeinsame),
                "gemeinsame Zeile „{gemeinsame}“ fehlt: {zeilen:?}"
            );
        }
        // Nichts, was nur eine der beiden Arten hat.
        for eigene in ["dicke", "schraffur", "geometrie", "hoehe", "kennzahlen"] {
            assert!(
                !zeilen.contains(&eigene),
                "art-eigene Zeile „{eigene}“ in der gemischten Auswahl: {zeilen:?}"
            );
        }
        // Genau ein Abschnitt, jede gemeinsame Zeile genau einmal.
        for einmal in ["layer", "farbe", "staerke", "linientyp"] {
            assert_eq!(
                zeilen.iter().filter(|z| **z == einmal).count(),
                1,
                "Zeile „{einmal}“ doppelt: {zeilen:?}"
            );
        }
    }

    /// Hat eine der Arten keine Linie (Text), entfallen Stärke und Linientyp —
    /// Layer und Farbe bleiben.
    #[test]
    fn gemischte_auswahl_mit_text_ohne_linienzeilen() {
        let (mut app, a, b) = zwei_waende();
        let text = app
            .session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1.0, 1.0], "text": "Hallo", "height": 0.25}),
            )
            .expect("Text")["created"][0]
            .as_u64()
            .expect("Text-Id");
        app.session_mut().selection = vec![a, b, text];
        let zeilen = zeichne(&mut app);

        assert!(zeilen.contains(&"layer"), "Layer fehlt: {zeilen:?}");
        assert!(zeilen.contains(&"farbe"), "Farbe fehlt: {zeilen:?}");
        assert!(
            !zeilen.contains(&"staerke"),
            "Stärke steht, obwohl der Text keine Linie hat: {zeilen:?}"
        );
        assert!(
            !zeilen.contains(&"linientyp"),
            "Linientyp steht, obwohl der Text keine Linie hat: {zeilen:?}"
        );
    }

    /// Eine Änderung der gemeinsamen Zeilen trifft **alle** Arten der Auswahl —
    /// auch die, deren Werte nicht angezeigt werden.
    #[test]
    fn gemischte_auswahl_aenderung_trifft_alle_arten() {
        let (mut app, a, _b) = zwei_waende();
        let line = app
            .session_mut()
            .execute(
                "draw.line",
                &json!({"a": [0.0, 5.0], "b": [2.0, 5.0]}),
            )
            .expect("Linie")["created"][0]
            .as_u64()
            .expect("Linien-Id");
        app.mehrfach_ziel = vec![a, line];
        let mut aenderung = Gemein::default();
        aenderung.farbe = Some(Some(Color::rgb(10, 120, 200)));
        aenderung.staerke = Some(0.42);
        gemein_nach_objekt(&mut app, a, &aenderung);
        for id in [a, line] {
            let e = app.doc().entity(id).expect("Objekt");
            assert_eq!(e.style.color, Some(Color::rgb(10, 120, 200)), "#{id}");
            assert_eq!(e.style.line_weight, Some(0.42), "#{id}");
        }
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
    /// Objekt der Gruppe, nicht gegen den angezeigten Vorgabewert
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

    /// Die Linienart „Dämmung“ bringt zwei eigene Zeilen mit — Breite (H) und
    /// Schlaufenform (r). Bei jeder anderen Linienart fehlen sie, sonst stünden
    /// zwei wirkungslose Felder in jedem Panel (T0120).
    #[test]
    fn daemmung_zeigt_breite_und_schlaufenform() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let linie = |app: &mut App, typ: &str| -> Id {
            app.session_mut()
                .execute(
                    "draw.line",
                    &json!({
                        "a": [0.0, 0.0], "b": [3.0, 0.0],
                        "linetype": typ, "loop_width": 0.12, "loop_shape": 0.4
                    }),
                )
                .expect("Linie")["created"][0]
                .as_u64()
                .expect("Id")
        };

        let duenn = linie(&mut app, "insulation");
        app.session_mut().selection = vec![duenn];
        let zeilen = zeichne(&mut app);
        assert!(
            zeilen.contains(&"daemmung_breite"),
            "Breite fehlt bei Dämmung: {zeilen:?}"
        );
        assert!(
            zeilen.contains(&"daemmung_form"),
            "Schlaufenform fehlt bei Dämmung: {zeilen:?}"
        );

        let voll = linie(&mut app, "solid");
        app.session_mut().selection = vec![voll];
        let zeilen = zeichne(&mut app);
        assert!(
            !zeilen.contains(&"daemmung_breite") && !zeilen.contains(&"daemmung_form"),
            "Dämmungszeilen stehen an einer durchgezogenen Linie: {zeilen:?}"
        );
    }

    /// Die Wanddämmung ist kein Muster mehr (T0132): an der Wand gibt es keine
    /// Zeile „Schlaufenform“ — die Dämmung zeichnet die Linienart, dort steht
    /// das Feld (siehe `daemmungswerte_aus_dem_panel_gelten_...`).
    #[test]
    fn wand_panel_hat_keine_schlaufenform() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        for hatch in ["mauerwerk", "daemmung_schlaufen"] {
            let id = app
                .session_mut()
                .execute(
                    "draw.wall",
                    &json!({
                        "a": [0.0, 0.0], "b": [4.0, 0.0], "thickness": 0.24,
                        "hatch": hatch
                    }),
                )
                .expect("Wand")["created"][0]
                .as_u64()
                .expect("Id");
            app.session_mut().selection = vec![id];
            let zeilen = zeichne(&mut app);
            assert!(
                !zeilen.contains(&"daemmung_form") && !zeilen.contains(&"daemmung_breite"),
                "Dämmungszeile an der Wand (Schraffur „{hatch}“): {zeilen:?}"
            );
        }
    }

    /// Die beiden Zeilen gelten auch beim Erzeugen: die Vorgabe aus dem
    /// Eigenschaften-Panel landet am neuen Objekt (T0062-Regel).
    #[test]
    fn daemmungswerte_aus_dem_panel_gelten_fuer_das_naechste_element() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.ui.pending.line_type = Some(jjcad_core::style::LineType::Insulation);
        app.start_tool("draw.line");
        let zeilen = zeichne(&mut app);
        assert!(
            zeilen.contains(&"daemmung_breite") && zeilen.contains(&"daemmung_form"),
            "Dämmungszeilen fehlen beim Erzeugen: {zeilen:?}"
        );

        let mut a = Gemein::default();
        a.daemmung_breite = Some(180.0);
        a.daemmung_form = Some(0.35);
        gemein_nach_optionen(&mut app, &a);
        assert_eq!(app.ui.pending.loop_width, Some(180.0));
        assert_eq!(app.ui.pending.loop_shape, Some(0.35));
    }

    /// T0169: Die Schnellauswahl übernimmt auch die Werte, die **nur** in den
    /// Werkzeugvorgaben stehen. Beim Text sind das Höhe, Schrift, Auszeichnung
    /// und die Farbe — letztere liegt für den Editor in `text_color`, nicht in
    /// `pending.color`.
    #[test]
    fn vorgaben_aus_objekt_uebernimmt_textwerte() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let rot = Color::RED;
        let angelegt = app
            .session_mut()
            .execute(
                "draw.text",
                &json!({
                    "p": [1.0, 1.0],
                    "text": "Hallo",
                    "height": 0.35,
                    "color": rot.to_hex(),
                    "font": "din_mittel",
                    "bold": true,
                    "align": "center",
                }),
            )
            .expect("Text anlegen");
        let id = angelegt["created"][0].as_u64().expect("Text-Id");

        assert!(vorgaben_aus_objekt(&mut app, id));
        assert!(
            (app.options.text_height - 350.0).abs() < 1e-6,
            "Höhe: {}",
            app.options.text_height
        );
        assert_eq!(app.options.text_font, TextFont::DinMittel, "Schrift");
        assert!(app.options.text_bold, "Auszeichnung");
        assert_eq!(app.options.text_align, TextAlignH::Center, "Ausrichtung");
        assert_eq!(app.options.text_color, Some(rot), "Farbe für den Editor");
        assert_eq!(app.ui.pending.color, Some(rot), "Farbe für die Action");
    }

    /// Die Übernahme nimmt die **Felder** des Objekts, nicht die wirksamen
    /// Werte der Ebene: ein Objekt „vom Layer“ setzt die Vorgabe wieder auf
    /// „vom Layer“, damit die neue Linie die Farbe ihrer Ebene erbt.
    #[test]
    fn vorgaben_aus_objekt_setzt_bylayer_auf_bylayer() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.ui.pending.color = Some(Color::RED);
        let angelegt = app
            .session_mut()
            .execute(
                "draw.line",
                &json!({"a": [0.0, 0.0], "b": [2.0, 0.0], "color": "bylayer"}),
            )
            .expect("Linie anlegen");
        let id = angelegt["created"][0].as_u64().expect("Linien-Id");

        assert!(vorgaben_aus_objekt(&mut app, id));
        assert_eq!(app.ui.pending.color, None, "Vorgabe muss „vom Layer“ sein");
    }
}

/// Art-Filter der Mehrfachauswahl (T0194): Steht über der Auswahl mehr als
/// eine Objektart, listet ein Auswahlfeld unter „N Objekte · M Arten“ genau
/// diese Arten; eine Wahl lässt nur deren Objekte in der Auswahl.
#[cfg(test)]
mod tests_artfilter {
    use super::*;
    use serde_json::json;

    /// Zwei Wände und eine Linie, alle drei ausgewählt.
    fn gemischte_auswahl() -> (App, Vec<Id>, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let mut waende = Vec::new();
        for (a, b) in [([0.0, 0.0], [3.0, 0.0]), ([0.0, 2.0], [4.0, 2.0])] {
            let r = app
                .session_mut()
                .execute(
                    "draw.wall",
                    &json!({"a": a, "b": b, "thickness": 0.24, "hatch": "solid"}),
                )
                .expect("Wand");
            waende.push(r["created"][0].as_u64().expect("Wand-Id") as Id);
        }
        let r = app
            .session_mut()
            .execute("draw.line", &json!({"a": [0.0, 5.0], "b": [2.0, 5.0]}))
            .expect("Linie");
        let linie = r["created"][0].as_u64().expect("Linien-Id") as Id;
        let alle = vec![waende[0], waende[1], linie];
        app.session_mut().select(alle);
        (app, waende, linie)
    }

    /// Ein Bild des Eigenschaften-Panels; liefert die gezeichneten Formen.
    fn bild(app: &mut App, ereignisse: Vec<egui::Event>, zeit: f64) -> egui::FullOutput {
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
        app.ctx.clone().run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| crate::panels::properties(app, ui));
        })
    }

    /// Alle gezeichneten Texte mit ihrer Fläche — auch die im aufgeklappten
    /// Menü (eigene Ebene, teils in `Shape::Vec`).
    fn texte(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        fn lauf(s: &egui::Shape, acc: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    acc.push((t.galley.text().to_string(), r));
                }
                egui::Shape::Vec(list) => {
                    for s in list {
                        lauf(s, acc);
                    }
                }
                _ => {}
            }
        }
        let mut acc = Vec::new();
        for cs in &out.shapes {
            lauf(&cs.shape, &mut acc);
        }
        acc
    }

    fn flaeche(out: &egui::FullOutput, text: &str) -> egui::Rect {
        texte(out)
            .into_iter()
            .find(|(t, _)| t.trim() == text)
            .unwrap_or_else(|| {
                let alle: Vec<String> = texte(out).into_iter().map(|(t, _)| t).collect();
                panic!("Text „{text}“ nicht im Panel: {alle:?}")
            })
            .1
    }

    fn knopf(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    /// Ein Klick auf eine Fläche in zwei Bildern (Drücken, Loslassen).
    fn klick(app: &mut App, ziel: egui::Pos2, zeit: f64) {
        let _ = bild(app, vec![egui::Event::PointerMoved(ziel)], zeit);
        let _ = bild(app, vec![knopf(ziel, true)], zeit + 0.05);
        let _ = bild(app, vec![knopf(ziel, false)], zeit + 0.1);
        let _ = bild(app, Vec::new(), zeit + 0.15);
    }

    /// Die Wahl „Wand“ entfernt die Linie aus der Auswahl; das Panel zeigt
    /// danach den Abschnitt der Wand, nicht mehr den gemischten Kopf.
    #[test]
    fn artwahl_entfernt_die_uebrigen_arten_aus_der_auswahl() {
        let (mut app, waende, linie) = gemischte_auswahl();
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = flaeche(&out, "Alle Arten");
        klick(&mut app, feld.center(), 2.0);

        let out = bild(&mut app, Vec::new(), 3.0);
        let eintrag = flaeche(&out, "Wand");
        klick(&mut app, eintrag.center(), 4.0);

        let auswahl = app.session().selection.clone();
        assert_eq!(auswahl, waende, "die Auswahl ist nicht auf die Wände gefallen");
        assert!(!auswahl.contains(&linie), "die Linie steckt noch in der Auswahl");

        // Das Panel zeigt jetzt die Wand (Dicke), nicht mehr den gemischten Kopf.
        let mut app = app;
        let out = bild(&mut app, Vec::new(), 6.0);
        let alle: Vec<String> = texte(&out).into_iter().map(|(t, _)| t).collect();
        assert!(
            !alle.iter().any(|t| t.contains("Arten")),
            "der gemischte Kopf steht noch: {alle:?}"
        );
        assert!(
            alle.iter().any(|t| t.contains("2 Objekte")),
            "die Wand-Mehrfachauswahl fehlt: {alle:?}"
        );
    }

    /// Verlängerung (T0226): das Häkchen im Panel schreibt `ext_lines` an das
    /// Objekt, und ein zweiter Klick nimmt es zurück. Vorgabe ist **aus**.
    #[test]
    fn haekchen_schaltet_die_verlaengerung() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let r = app
            .session_mut()
            .execute(
                "dim.chain",
                &json!({"pts": [[0.0, 0.0], [3.0, 0.0], [6.0, 0.0]],
                        "line_pos": [0.0, -1.0], "angle": 0.0}),
            )
            .expect("Maßkette");
        let id = r["created"][0].as_u64().expect("Kennung") as Id;
        app.session_mut().select(vec![id]);

        let ext = |app: &App| match &app.doc().entity(id).expect("Maßkette").kind {
            EntityKind::Dim { ext_lines, .. } => *ext_lines,
            k => panic!("keine Maßkette: {k:?}"),
        };
        assert!(!ext(&app), "Vorgabe muss aus sein");

        // Die Häkchenfläche: Namensspalte 160 px, das Kästchen sitzt links in
        // der Wertspalte.
        let out = bild(&mut app, Vec::new(), 1.0);
        let zeile = flaeche(&out, "Verlängerung");
        let kaestchen = egui::pos2(zeile.left() + 166.0, zeile.center().y);

        klick(&mut app, kaestchen, 2.0);
        assert!(ext(&app), "Klick muss die Verlängerung einschalten");

        klick(&mut app, kaestchen, 4.0);
        assert!(!ext(&app), "zweiter Klick muss sie wieder ausschalten");
    }

    /// Beim Erzeugen gilt das Häkchen für das nächste Element: es schreibt in
    /// die Vorgaben des Werkzeugs (`ToolOptions::dim_ext`, T0226).
    #[test]
    fn haekchen_gilt_fuer_das_naechste_element() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        app.start_tool("dim.chain");
        assert!(!app.options.dim_ext, "Vorgabe muss aus sein");

        let out = bild(&mut app, Vec::new(), 1.0);
        let zeile = flaeche(&out, "Verlängerung");
        let kaestchen = egui::pos2(zeile.left() + 166.0, zeile.center().y);
        klick(&mut app, kaestchen, 2.0);

        assert!(app.options.dim_ext, "Vorgabe muss an sein");
    }

    /// Das Menü listet **nur** die Arten der Auswahl — Wand und Linie, nicht
    /// jede Objektart des Programms.
    #[test]
    fn artmenue_listet_nur_die_arten_der_auswahl() {
        let (mut app, _waende, _linie) = gemischte_auswahl();
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = flaeche(&out, "Alle Arten");
        klick(&mut app, feld.center(), 2.0);

        let out = bild(&mut app, Vec::new(), 3.0);
        let alle: Vec<String> = texte(&out).into_iter().map(|(t, _)| t).collect();
        for erwartet in ["Wand", "Linie"] {
            assert!(
                alle.iter().any(|t| t.trim() == erwartet),
                "„{erwartet}“ fehlt im Menü: {alle:?}"
            );
        }
        for fremd in ["Text", "Kreis", "Füllfläche", "Raumstempel"] {
            assert!(
                !alle.iter().any(|t| t.trim() == fremd),
                "„{fremd}“ steht im Menü, ist aber nicht in der Auswahl: {alle:?}"
            );
        }
    }
}


/// Punktsymbol der Maßkette (T0227): Das Auswahlfeld im Eigenschaften-Panel
/// zeigt die Vorgabe „Schrägstrich“ und schreibt die Wahl an das Objekt.
#[cfg(test)]
mod tests_masskette_symbol {
    use super::*;
    use serde_json::json;

    /// Ein Bild des Eigenschaften-Panels; liefert die gezeichneten Formen.
    fn bild(app: &mut App, ereignisse: Vec<egui::Event>, zeit: f64) -> egui::FullOutput {
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
        app.ctx.clone().run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| crate::panels::properties(app, ui));
        })
    }

    /// Alle gezeichneten Texte mit ihrer Fläche — auch die im aufgeklappten
    /// Menü (eigene Ebene, teils in `Shape::Vec`).
    fn texte(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        fn lauf(s: &egui::Shape, acc: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    acc.push((t.galley.text().to_string(), r));
                }
                egui::Shape::Vec(list) => {
                    for s in list {
                        lauf(s, acc);
                    }
                }
                _ => {}
            }
        }
        let mut acc = Vec::new();
        for cs in &out.shapes {
            lauf(&cs.shape, &mut acc);
        }
        acc
    }

    fn flaeche(out: &egui::FullOutput, text: &str) -> egui::Rect {
        texte(out)
            .into_iter()
            .find(|(t, _)| t.trim() == text)
            .unwrap_or_else(|| {
                let alle: Vec<String> = texte(out).into_iter().map(|(t, _)| t).collect();
                panic!("Text „{text}“ nicht im Panel: {alle:?}")
            })
            .1
    }

    fn knopf(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn klick(app: &mut App, ziel: egui::Pos2, zeit: f64) {
        let _ = bild(app, vec![egui::Event::PointerMoved(ziel)], zeit);
        let _ = bild(app, vec![knopf(ziel, true)], zeit + 0.05);
        let _ = bild(app, vec![knopf(ziel, false)], zeit + 0.1);
        let _ = bild(app, Vec::new(), zeit + 0.15);
    }

    /// Eine ausgewählte Maßkette mit zwei Messpunkten.
    fn kette() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let r = app
            .session_mut()
            .execute(
                "dim.chain",
                &json!({"pts": [[0.0, 0.0], [3.0, 0.0]],
                        "line_pos": [0.0, -1.0], "angle": 0.0}),
            )
            .expect("Maßkette");
        let id = r["created"][0].as_u64().expect("Kennung") as Id;
        app.session_mut().select(vec![id]);
        (app, id)
    }

    fn symbol(app: &App, id: Id) -> String {
        match &app.doc().entity(id).expect("Maßkette").kind {
            EntityKind::Dim { symbol, .. } => symbol.clone(),
            k => panic!("keine Maßkette: {k:?}"),
        }
    }

    /// Das Feld zeigt die Vorgabe — ohne eigene Wahl steht der Schrägstrich da.
    #[test]
    fn feld_zeigt_den_schraegstrich() {
        let (mut app, id) = kette();
        assert_eq!(
            symbol(&app, id),
            jjcad_core::model::DIM_SYMBOL_VORGABE,
            "Vorgabe"
        );
        let out = bild(&mut app, Vec::new(), 1.0);
        // Die Zeile heißt „Symbol“ (nicht zweimal „Punktsymbol“ im Panel) und
        // zeigt den Namen der Vorgabe.
        let alle: Vec<String> = texte(&out).into_iter().map(|(t, _)| t).collect();
        assert!(
            alle.iter().any(|t| t.trim() == "Symbol"),
            "die Zeile „Symbol“ fehlt: {alle:?}"
        );
        assert!(
            alle.iter().any(|t| t.trim() == "Schrägstrich"),
            "das Feld zeigt nicht die Vorgabe: {alle:?}"
        );
    }

    /// Ein Klick auf einen Eintrag schreibt den Block an das Objekt.
    #[test]
    fn auswahl_setzt_ein_anderes_symbol() {
        let (mut app, id) = kette();
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = flaeche(&out, "Schrägstrich");
        klick(&mut app, feld.center(), 2.0);

        let out = bild(&mut app, Vec::new(), 3.0);
        let eintrag = flaeche(&out, "Raute");
        klick(&mut app, eintrag.center(), 4.0);

        assert_eq!(symbol(&app, id), "sys:pkt-raute", "die Wahl muss ankommen");
        // Das geschlossene Feld zeigt jetzt den gewählten Namen.
        let out = bild(&mut app, Vec::new(), 5.0);
        let alle: Vec<String> = texte(&out).into_iter().map(|(t, _)| t).collect();
        assert!(
            alle.iter().any(|t| t.trim() == "Raute"),
            "das Feld zeigt die Wahl nicht: {alle:?}"
        );
    }

    /// „Kein Symbol“ nimmt die Marke von den Messpunkten.
    #[test]
    fn kein_symbol_nimmt_die_marke_weg() {
        let (mut app, id) = kette();
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = flaeche(&out, "Schrägstrich");
        klick(&mut app, feld.center(), 2.0);

        let out = bild(&mut app, Vec::new(), 3.0);
        let eintrag = flaeche(&out, "Kein Symbol");
        klick(&mut app, eintrag.center(), 4.0);

        assert_eq!(symbol(&app, id), "", "leer heißt kein Symbol");
    }
}

/// Angezeigte Layer im Ansichtsfenster (T0238): Die Häkchenliste im
/// Eigenschaften-Panel schreibt `Viewport::layers`. Ein Klick auf den Plan
/// schaltet alle seine Layer, ein Klick auf eine Zeile genau einen.
#[cfg(test)]
mod tests_ansichtsfenster_layer {
    use super::*;

    fn bild(app: &mut App, ereignisse: Vec<egui::Event>, zeit: f64) -> egui::FullOutput {
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
        app.ctx.clone().run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| crate::panels::properties(app, ui));
        })
    }

    fn texte(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        fn lauf(s: &egui::Shape, acc: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    acc.push((t.galley.text().to_string(), r));
                }
                egui::Shape::Vec(list) => {
                    for s in list {
                        lauf(s, acc);
                    }
                }
                _ => {}
            }
        }
        let mut acc = Vec::new();
        for cs in &out.shapes {
            lauf(&cs.shape, &mut acc);
        }
        acc
    }

    fn flaeche(out: &egui::FullOutput, text: &str) -> egui::Rect {
        texte(out)
            .into_iter()
            .find(|(t, _)| t.trim() == text)
            .unwrap_or_else(|| {
                let alle: Vec<String> = texte(out).into_iter().map(|(t, _)| t).collect();
                panic!("Text „{text}“ nicht im Panel: {alle:?}")
            })
            .1
    }

    fn knopf(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn klick(app: &mut App, ziel: egui::Pos2, zeit: f64) {
        let _ = bild(app, vec![egui::Event::PointerMoved(ziel)], zeit);
        let _ = bild(app, vec![knopf(ziel, true)], zeit + 0.05);
        let _ = bild(app, vec![knopf(ziel, false)], zeit + 0.1);
        let _ = bild(app, Vec::new(), zeit + 0.15);
    }

    /// App mit ausgewähltem Ansichtsfenster auf dem ersten Blatt.
    fn app_mit_fenster() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        let vid = app
            .doc()
            .sheets[0]
            .viewports()
            .next()
            .map(|(e, _)| e.id)
            .expect("Blatt hat ein Ansichtsfenster");
        // Das Panel liest `doc.ents()` — im Modell läge das Ansichtsfenster
        // nicht darin, es ist ein Blattobjekt. Der Raumwechsel leert die
        // Auswahl, sie wird deshalb danach gesetzt.
        let sid = app.doc().sheets[0].id;
        app.session_mut().set_space(Some(sid));
        app.session_mut().selection = vec![vid];
        (app, vid)
    }

    fn layer(app: &App, vid: Id) -> Option<Vec<Id>> {
        app.doc().sheets[0].viewport(vid).map(|vp| vp.layers.clone()).flatten()
    }

    /// Die Liste aller Layer, wie sie das Panel zeigt.
    #[test]
    fn panel_zeigt_alle_layer_mit_haken() {
        let (mut app, _) = app_mit_fenster();
        let out = bild(&mut app, Vec::new(), 1.0);
        let alle = texte(&out);
        let namen: Vec<&str> = alle.iter().map(|(t, _)| t.trim()).collect();
        for erwartet in ["Plan 1", "Zeichnung", "Architektur", "Beschriftung", "Blöcke"] {
            assert!(
                namen.contains(&erwartet),
                "„{erwartet}“ fehlt in der Layerliste: {namen:?}"
            );
        }
    }

    /// Ein Klick auf die Checkbox des Plans nimmt alle seine Layer heraus;
    /// ein zweiter Klick holt sie zurück (`null` = alle).
    #[test]
    fn plan_haken_schaltet_alle_seine_layer() {
        let (mut app, vid) = app_mit_fenster();
        let alle_layer = app.doc().layers.len();
        assert_eq!(layer(&app, vid), None, "ein neues Fenster zeigt alle Layer");

        let out = bild(&mut app, Vec::new(), 1.0);
        let plan = flaeche(&out, "Plan 1");
        klick(&mut app, plan.center(), 2.0);
        assert_eq!(
            layer(&app, vid),
            Some(Vec::new()),
            "der Planhaken muss alle seine Layer abwählen"
        );

        let out = bild(&mut app, Vec::new(), 4.0);
        let plan = flaeche(&out, "Plan 1");
        klick(&mut app, plan.center(), 5.0);
        assert_eq!(
            layer(&app, vid),
            None,
            "sind wieder alle Layer gewählt, steht `null` in der Datei ({alle_layer} Layer)"
        );
    }

    /// Ein Klick auf eine einzelne Layerzeile lässt die übrigen stehen.
    #[test]
    fn layer_haken_schaltet_genau_einen_layer() {
        let (mut app, vid) = app_mit_fenster();
        let architektur = app
            .doc()
            .layers
            .iter()
            .find(|l| l.name == "Architektur")
            .map(|l| l.id)
            .expect("Layer „Architektur“");

        let out = bild(&mut app, Vec::new(), 1.0);
        let zeile = flaeche(&out, "Architektur");
        klick(&mut app, zeile.center(), 2.0);

        let wahl = layer(&app, vid).expect("nach dem Abwählen steht eine Liste");
        assert!(!wahl.contains(&architektur), "„Architektur“ ist noch gewählt");
        assert_eq!(
            wahl.len(),
            app.doc().layers.len() - 1,
            "nur ein Layer darf fehlen: {wahl:?}"
        );
    }
}

#[cfg(test)]
mod tests_layer_plan {
    //! T0245: Layer und Plan sind zwei Abschnitte in **einem** Dropdown.
    //!
    //! Abnahme aus dem Ticket: Linie (Zeichnung) und Text (Beschriftung) im
    //! Plan 1 — bei gemeinsamer Auswahl steht „Plan 1 · *variiert*“ im Feld,
    //! und ein Klick auf „Plan 2“ verschiebt beide, jeder auf seinen Layer.

    use super::*;
    use serde_json::json;

    fn app_mit_linie_und_text() -> (App, Id, Id, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit");
        let linie = app
            .session_mut()
            .execute("draw.line", &json!({"a": [0.0, 0.0], "b": [4.0, 0.0]}))
            .expect("Linie")["created"][0]
            .as_u64()
            .expect("Linien-Id") as Id;
        let text = app
            .session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1.0, 1.0], "text": "Hallo", "height": 0.35}),
            )
            .expect("Text")["created"][0]
            .as_u64()
            .expect("Text-Id") as Id;
        let plan2 = app
            .session_mut()
            .execute("plan.create", &json!({"name": "Plan 2"}))
            .expect("Plan 2")["id"]
            .as_u64()
            .expect("Plan-Id") as Id;
        (app, linie, text, plan2)
    }

    fn bild(app: &mut App, ereignisse: Vec<egui::Event>, zeit: f64) -> egui::FullOutput {
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
        app.ctx.clone().run(raw, |ctx| {
            egui::SidePanel::right("eigenschaften")
                .exact_width(280.0)
                .show(ctx, |ui| crate::panels::properties(app, ui));
        })
    }

    /// Alle gemalten Texte mit ihrer Fläche — auch die im aufgeklappten Menü.
    fn texte(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        fn lauf(s: &egui::Shape, acc: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    acc.push((t.galley.text().to_string(), r));
                }
                egui::Shape::Vec(list) => {
                    for s in list {
                        lauf(s, acc);
                    }
                }
                _ => {}
            }
        }
        let mut acc = Vec::new();
        for cs in &out.shapes {
            lauf(&cs.shape, &mut acc);
        }
        acc
    }

    fn flaeche(out: &egui::FullOutput, text: &str) -> egui::Rect {
        texte(out)
            .into_iter()
            .find(|(t, _)| t.trim() == text)
            .unwrap_or_else(|| {
                let alle: Vec<String> = texte(out).into_iter().map(|(t, _)| t).collect();
                panic!("Text „{text}“ nicht im Panel: {alle:?}")
            })
            .1
    }

    fn knopf(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn klick(app: &mut App, ziel: egui::Pos2, zeit: f64) {
        let _ = bild(app, vec![egui::Event::PointerMoved(ziel)], zeit);
        let _ = bild(app, vec![knopf(ziel, true)], zeit + 0.05);
        let _ = bild(app, vec![knopf(ziel, false)], zeit + 0.1);
        let _ = bild(app, Vec::new(), zeit + 0.15);
    }

    fn layername(app: &App, id: Id) -> String {
        app.doc()
            .layer(app.doc().entity(id).expect("Objekt").layer)
            .expect("Layer")
            .name
            .clone()
    }

    /// Das Feld nennt den gemeinsamen Plan und meldet die Layer als variiert;
    /// das Menü listet jeden Layernamen **einmal** und darunter die Pläne.
    #[test]
    fn feld_zeigt_plan_und_variierte_layer() {
        let (mut app, linie, text, _plan2) = app_mit_linie_und_text();
        app.session_mut().select(vec![linie, text]);
        let out = bild(&mut app, Vec::new(), 1.0);
        // Die Beschriftung des Felds — der Plan steht darin, der Layer variiert.
        let feld = flaeche(&out, "Plan 1 · *variiert*");

        // Menü öffnen: beide Abschnitte sichtbar, jeder Layername einmal.
        klick(&mut app, feld.center(), 2.0);
        let out = bild(&mut app, Vec::new(), 3.0);
        let alle: Vec<String> = texte(&out).into_iter().map(|(t, _)| t).collect();
        for name in ["Zeichnung", "Architektur", "Beschriftung", "Blöcke"] {
            assert_eq!(
                alle.iter().filter(|t| t.trim() == name).count(),
                1,
                "Layer „{name}“ steht nicht genau einmal im Menü: {alle:?}"
            );
        }
        assert!(
            alle.iter().any(|t| t.trim() == "Plan 2"),
            "Plan 2 fehlt im Menü: {alle:?}"
        );
        // Abschnittsüberschriften „Layer“ und „Plan“ — „Plan 1“ gibt es nicht.
        assert!(alle.iter().any(|t| t.trim() == "Layer"), "Abschnitt Layer fehlt");
        assert!(alle.iter().any(|t| t.trim() == "Plan"), "Abschnitt Plan fehlt");
    }

    /// Unter den Überschriften „Layer“ und „Plan“ steht Luft: die
    /// Auswahlfläche der ersten Zeile darunter reichte sonst bis an den Text
    /// und verdeckte ihn, sobald die Zeile gewählt war.
    #[test]
    fn ueberschriften_haben_abstand_zur_ersten_zeile() {
        let (mut app, linie, text, _plan2) = app_mit_linie_und_text();
        app.session_mut().select(vec![linie, text]);
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = flaeche(&out, "Plan 1 · *variiert*");
        klick(&mut app, feld.center(), 2.0);
        let out = bild(&mut app, Vec::new(), 3.0);

        let alle = texte(&out);
        // „Layer“ steht auch als Zeilenname links am Feld — die Überschrift im
        // Menü ist die untere der beiden.
        let kopf_layer = alle
            .iter()
            .filter(|(t, _)| t.trim() == "Layer")
            .map(|(_, r)| *r)
            .max_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("Überschrift Layer");
        let erste_zeile = alle
            .iter()
            .filter(|(t, _)| t.trim() == "Zeichnung")
            .map(|(_, r)| *r)
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("Layer Zeichnung im Menü");
        let kopf_plan = alle
            .iter()
            .find(|(t, _)| t.trim() == "Plan")
            .map(|(_, r)| *r)
            .expect("Überschrift Plan");
        let erster_plan = alle
            .iter()
            .find(|(t, _)| t.trim() == "Plan 1")
            .map(|(_, r)| *r)
            .expect("Plan 1 im Menü");

        let luecke_layer = erste_zeile.top() - kopf_layer.bottom();
        let luecke_plan = erster_plan.top() - kopf_plan.bottom();
        // Ohne den Abstand sind es 6 px (gemessen) — die Auswahlfläche der
        // Zeile reicht darüber hinaus und deckte den Text zu. Mit sind es 10.
        assert!(
            luecke_layer >= 8.0,
            "zu wenig Luft unter „Layer“: {luecke_layer:.1} px"
        );
        assert!(
            luecke_plan >= 8.0,
            "zu wenig Luft unter „Plan“: {luecke_plan:.1} px"
        );
    }

    /// Abnahmefall aus dem Ticket: Klick auf „Plan 2“ verschiebt beide
    /// Objekte, der Text auf „Beschriftung“, die Linie auf „Zeichnung“.
    #[test]
    fn planwahl_verschiebt_alle_objekte_und_behaelt_den_layer() {
        let (mut app, linie, text, plan2) = app_mit_linie_und_text();
        app.session_mut().select(vec![linie, text]);
        let out = bild(&mut app, Vec::new(), 1.0);
        let feld = flaeche(&out, "Plan 1 · *variiert*");
        klick(&mut app, feld.center(), 2.0);

        let out = bild(&mut app, Vec::new(), 3.0);
        let eintrag = flaeche(&out, "Plan 2");
        klick(&mut app, eintrag.center(), 4.0);

        assert_eq!(layername(&app, text), "Beschriftung");
        assert_eq!(layername(&app, linie), "Zeichnung");
        for id in [linie, text] {
            let plan = app
                .doc()
                .plan_of_layer(app.doc().entity(id).expect("Objekt").layer)
                .expect("Plan");
            assert_eq!(plan.id, plan2, "Objekt {id} liegt nicht in Plan 2");
        }
    }

    /// Der obere Abschnitt setzt den Layer im Plan **jedes** Objekts; die Pläne
    /// bleiben dabei getrennt.
    #[test]
    fn layerwahl_aendert_den_namen_und_laesst_die_plaene_stehen() {
        let (mut app, linie, text, plan2) = app_mit_linie_und_text();
        // Den Text vorher allein nach Plan 2 schieben.
        app.mehrfach_ziel = vec![text];
        app.session_mut()
            .execute("modify.set", &json!({"ids": [text], "plan": plan2}))
            .expect("Text nach Plan 2");
        app.session_mut().select(vec![linie, text]);
        let out = bild(&mut app, Vec::new(), 1.0);
        // Pläne verschieden → Feld sagt nur *variiert*.
        let feld = flaeche(&out, "*variiert*");
        klick(&mut app, feld.center(), 2.0);

        let out = bild(&mut app, Vec::new(), 3.0);
        let eintrag = flaeche(&out, "Architektur");
        klick(&mut app, eintrag.center(), 4.0);

        assert_eq!(layername(&app, linie), "Architektur");
        assert_eq!(layername(&app, text), "Architektur");
        let plan_linie = app
            .doc()
            .plan_of_layer(app.doc().entity(linie).expect("Linie").layer)
            .expect("Plan")
            .id;
        let plan_text = app
            .doc()
            .plan_of_layer(app.doc().entity(text).expect("Text").layer)
            .expect("Plan")
            .id;
        assert_ne!(plan_linie, plan_text, "die Pläne bleiben getrennt");
    }
}
