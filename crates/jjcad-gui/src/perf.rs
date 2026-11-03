//! Leistungsanzeige (T0068) — Messwerte je Bild und ihre Aufteilung.
//!
//! JoJa-CAD zeichnet ein Bild **nur, wenn eines gebraucht wird** (Zeiger,
//! Eingabe, laufende Vorschau). Eine Bilder-pro-Sekunde-Zahl wäre deshalb
//! irreführend: sie zählt die Pausen mit. Gemessen werden stattdessen
//!
//! * der **Abstand** zwischen zwei gezeichneten Bildern (wie träge fühlt es
//!   sich an) und
//! * die **Rechenzeit eines Bildes**, zerlegt in die Abschnitte der
//!   Anzeigekette ([`Abschnitt`]).
//!
//! Was in keinem Abschnitt steckt — Tessellierung der Figuren, die GPU und das
//! Warten auf den Bildwechsel — erscheint als „außerhalb“: Abstand minus
//! Rechenzeit. Nach unten begrenzt (`max(0)`), weil Abstand und Rechenzeit aus
//! zwei verschiedenen Uhren kommen.
//!
//! Für den Verlauf gibt es Perzentile (p50 · p95 · p99) über den Ring der
//! letzten [`RING`] Bilder. **Pausen** (Abstand über [`PAUSE_MS`]) gehen nicht
//! ein: dort wurde nichts gerechnet, der Wert sagt nichts über die Zeichenzeit.
//!
//! Die Messung läuft immer mit (ein paar Uhrablesungen je Bild, kein
//! Speicherzuwachs). Die Zerlegung der einzelnen Grafikelemente kostet je
//! Element eine Uhr und läuft deshalb **nur bei offener Anzeige**.

use std::collections::VecDeque;
use std::time::Instant;

use egui::{Align, Color32, Id, Layout, Order, RichText, Stroke};
use jjcad_core::render;

use crate::app::App;
use crate::theme::Palette;

/// So viele Bilder werden für die Perzentile gemerkt.
pub const RING: usize = 240;

/// Bilder mit größerem Abstand zum Vorgänger sind Pausen (kein Takt) und gehen
/// nicht in die Perzentile ein.
pub const PAUSE_MS: f32 = 600.0;

/// Abschnitte der Anzeigekette — in dieser Reihenfolge läuft ein Bild.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Abschnitt {
    /// Kopfzeile, Ribbon, Reiter, Seitenpanels und Leisten (`App::ui` außer der
    /// Zeichenfläche)
    Oberflaeche,
    /// Aufbau der Displayliste und ihre Kopie aus dem Zwischenspeicher
    Displayliste,
    /// Navigation, Fangen und die Verweil-Erkennung des erweiterten Punktfangs
    Fangen,
    /// Raster, Papier und die Grafikelemente auf die Fläche bringen
    Zeichnen,
    /// Werkzeug-Vorschau, Griffe, Rahmen, Fadenkreuz und die Eingabe
    Bedienung,
    /// Dialoge, Menüs, Chatfenster und diese Anzeige
    Fenster,
}

/// Anzahl der Abschnitte.
pub const ANZAHL: usize = 6;

impl Abschnitt {
    /// Alle Abschnitte in der Reihenfolge eines Bildes.
    pub const ALLE: [Abschnitt; ANZAHL] = [
        Abschnitt::Oberflaeche,
        Abschnitt::Displayliste,
        Abschnitt::Fangen,
        Abschnitt::Zeichnen,
        Abschnitt::Bedienung,
        Abschnitt::Fenster,
    ];

    /// Platz in den Summen.
    fn index(self) -> usize {
        match self {
            Abschnitt::Oberflaeche => 0,
            Abschnitt::Displayliste => 1,
            Abschnitt::Fangen => 2,
            Abschnitt::Zeichnen => 3,
            Abschnitt::Bedienung => 4,
            Abschnitt::Fenster => 5,
        }
    }

    /// Beschriftung in der Anzeige.
    pub fn name(self) -> &'static str {
        match self {
            Abschnitt::Oberflaeche => jjcad_core::i18n::tr("gui.perf.oberflaeche"),
            Abschnitt::Displayliste => jjcad_core::i18n::tr("gui.perf.displayliste"),
            Abschnitt::Fangen => jjcad_core::i18n::tr("gui.perf.fangen"),
            Abschnitt::Zeichnen => jjcad_core::i18n::tr("gui.perf.zeichnen"),
            Abschnitt::Bedienung => jjcad_core::i18n::tr("gui.perf.bedienung"),
            Abschnitt::Fenster => jjcad_core::i18n::tr("gui.perf.fenster"),
        }
    }

    /// Kurze Erläuterung (Tooltip / Fußzeile).
    pub fn erklaerung(self) -> &'static str {
        match self {
            Abschnitt::Oberflaeche => jjcad_core::i18n::tr("gui.perf.ribbon_reiter_panels_leisten"),
            Abschnitt::Displayliste => "Displayliste aufbauen bzw. kopieren",
            Abschnitt::Fangen => jjcad_core::i18n::tr("gui.perf.navigation_fangen_verweilfang"),
            Abschnitt::Zeichnen => jjcad_core::i18n::tr("gui.perf.raster_papier_grafikelemente"),
            Abschnitt::Bedienung => jjcad_core::i18n::tr("gui.perf.vorschau_griffe_fadenkreuz_eingabe"),
            Abschnitt::Fenster => jjcad_core::i18n::tr("gui.perf.dialoge_menues_chat_diese_anzeige"),
        }
    }
}

/// Wozu ein Grafikelement beim Zeichnen zählt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MalKlasse {
    Fuellung,
    Linie,
    Text,
}

impl MalKlasse {
    fn index(self) -> usize {
        match self {
            MalKlasse::Fuellung => 0,
            MalKlasse::Linie => 1,
            MalKlasse::Text => 2,
        }
    }

    /// Wozu ein Grafikelement der Displayliste gehört. Flächen (Füllungen)
    /// werden verbunden und trianguliert, Texte über die Schrift
    /// gesetzt — das sind die drei Posten, die unterschiedlich teuer sind.
    pub fn von(shape: &render::Shape) -> MalKlasse {
        match shape {
            render::Shape::Fill { .. } => MalKlasse::Fuellung,
            render::Shape::Text(_) => MalKlasse::Text,
            _ => MalKlasse::Linie,
        }
    }
}

/// Messwerte eines abgeschlossenen Bildes.
#[derive(Clone, Copy, Debug)]
pub struct Bild {
    /// ms seit dem vorigen gezeichneten Bild
    pub abstand: f32,
    /// Rechenzeit dieses Bildes (Summe der Abschnitte)
    pub cpu: f32,
    /// ms je Abschnitt
    zeiten: [f32; ANZAHL],
    /// ms je Art beim Zeichnen der Grafikelemente
    malzeiten: [f32; 3],
    /// Displayliste in diesem Bild neu aufgebaut (sonst aus dem Zwischenspeicher)
    pub liste_neu: bool,
    /// Objekte der Zeichnung, die in die Displayliste gingen
    pub objekte: usize,
    /// Grafikelemente insgesamt und je Art
    pub grafikelemente: usize,
    pub fuellungen: usize,
    pub linien: usize,
    pub texte: usize,
}

impl Default for Bild {
    fn default() -> Self {
        Self {
            abstand: 0.0,
            cpu: 0.0,
            zeiten: [0.0; ANZAHL],
            malzeiten: [0.0; 3],
            liste_neu: false,
            objekte: 0,
            grafikelemente: 0,
            fuellungen: 0,
            linien: 0,
            texte: 0,
        }
    }
}

impl Bild {
    /// Zeit eines Abschnitts in ms.
    pub fn zeit(&self, a: Abschnitt) -> f32 {
        self.zeiten[a.index()]
    }

    /// Zeit zum Zeichnen einer Art von Grafikelement in ms.
    pub fn malzeit(&self, k: MalKlasse) -> f32 {
        self.malzeiten[k.index()]
    }

    /// Zeit, die in keinem Abschnitt steckt: Tessellierung, GPU und das Warten
    /// auf den Bildwechsel.
    pub fn ausserhalb(&self) -> f32 {
        (self.abstand - self.cpu).max(0.0)
    }

    /// Anteil eines Abschnitts an der Rechenzeit (0…1).
    pub fn anteil(&self, a: Abschnitt) -> f32 {
        if self.cpu <= 0.0 {
            0.0
        } else {
            self.zeit(a) / self.cpu
        }
    }
}

/// Messung und Verlauf der Bilder.
pub struct Perf {
    /// Anzeige offen (Umschalt+F3)
    pub an: bool,
    /// Laufendes Bild
    laufend: Bild,
    /// Zeitpunkt des letzten Abschnittswechsels
    marke: Instant,
    /// Offener Abschnitt — ihm wird die Zeit bis zum nächsten Wechsel
    /// zugeschlagen
    phase: Abschnitt,
    /// Beginn des vorigen Bildes (für den Abstand)
    voriger_start: Option<Instant>,
    /// Abgeschlossene Bilder für die Perzentile (die letzten [`RING`])
    pub ring: VecDeque<Bild>,
    /// Zuletzt abgeschlossenes Bild — bleibt auch dann stehen, wenn es als
    /// Pause nicht in den Ring ging (sonst zeigte die Anzeige bei einem sehr
    /// langsamen Bild gar nichts)
    letzte: Option<Bild>,
    /// Bilder, die als Pause nicht in den Ring gingen
    pub pausen: usize,
    /// Belegter Speicher in MB (nur Linux, einmal je Sekunde abgefragt)
    speicher_mb: f32,
    /// Threads des Prozesses (nur Linux)
    threads: usize,
    /// Zeitpunkt der letzten Speicherab Frage
    probe: Instant,
}

impl Default for Perf {
    fn default() -> Self {
        let jetzt = Instant::now();
        Self {
            an: false,
            laufend: Bild::default(),
            marke: jetzt,
            phase: Abschnitt::Oberflaeche,
            voriger_start: None,
            ring: VecDeque::with_capacity(RING),
            letzte: None,
            pausen: 0,
            speicher_mb: 0.0,
            threads: 0,
            probe: jetzt,
        }
    }
}

impl Perf {
    /// Ein Bild beginnt: Abstand zum vorigen Bild festhalten, Summen leeren.
    pub fn bild_beginnen(&mut self) {
        let jetzt = Instant::now();
        self.laufend = Bild::default();
        if let Some(v) = self.voriger_start {
            self.laufend.abstand = (jetzt - v).as_secs_f32() * 1000.0;
        }
        self.voriger_start = Some(jetzt);
        self.marke = jetzt;
        self.phase = Abschnitt::Oberflaeche;
    }

    /// Abschnitt wechseln. Die seit dem letzten Wechsel vergangene Zeit gehört
    /// dem **vorigen** Abschnitt — er war bis hierher offen.
    pub fn abschnitt(&mut self, neu: Abschnitt) {
        let jetzt = Instant::now();
        let dauer = (jetzt - self.marke).as_secs_f32() * 1000.0;
        self.laufend.zeiten[self.phase.index()] += dauer;
        self.marke = jetzt;
        self.phase = neu;
    }

    /// Ein Grafikelement wurde gezeichnet: zählen und seine Zeit der Art
    /// zuschlagen (nur bei offener Anzeige aufgerufen).
    pub fn grafikelement(&mut self, prim: &render::Prim, dauer: std::time::Duration) {
        self.laufend.grafikelemente += 1;
        let klasse = MalKlasse::von(&prim.shape);
        self.laufend.malzeiten[klasse.index()] += dauer.as_secs_f32() * 1000.0;
        match klasse {
            MalKlasse::Fuellung => self.laufend.fuellungen += 1,
            MalKlasse::Linie => self.laufend.linien += 1,
            MalKlasse::Text => self.laufend.texte += 1,
        }
    }

    /// Woher die Displayliste dieses Bildes kam und wie groß die Zeichnung ist.
    pub fn anzeigeliste(&mut self, neu: bool, objekte: usize) {
        self.laufend.liste_neu = neu;
        self.laufend.objekte = objekte;
    }

    /// Bild abschließen: offenen Abschnitt schließen, Summe bilden, merken.
    pub fn bild_beenden(&mut self) {
        self.abschnitt(self.phase);
        self.laufend.cpu = self.laufend.zeiten.iter().sum();
        self.merken(self.laufend);
        self.probe_speicher();
    }

    /// Ein Bild in den Ring aufnehmen — Pausen ausgenommen.
    fn merken(&mut self, bild: Bild) {
        self.letzte = Some(bild);
        if bild.abstand <= 0.0 {
            // Das erste Bild nach dem Start hat keinen Vorgänger.
            return;
        }
        // Pause = lange gewartet, aber selbst nichts gerechnet. Ein Bild, das
        // selbst länger als die Pausenschwelle rechnet (große Zeichnung), ist
        // keine Pause und muss im Verlauf stehen.
        if bild.abstand > PAUSE_MS && bild.cpu < PAUSE_MS {
            self.pausen += 1;
            return;
        }
        if self.ring.len() == RING {
            self.ring.pop_front();
        }
        self.ring.push_back(bild);
    }

    /// Speicher und Threads einmal je Sekunde nachsehen (Systemaufrufe sind
    /// billig, aber nicht umsonst).
    fn probe_speicher(&mut self) {
        if self.probe.elapsed().as_millis() < 1000 {
            return;
        }
        self.probe = Instant::now();
        self.speicher_mb = belegter_speicher_mb();
        self.threads = threadzahl();
    }

    /// Belegter Speicher in MB (0, wenn das System es nicht hergibt).
    pub fn speicher_mb(&self) -> f32 {
        self.speicher_mb
    }

    /// Threads des Prozesses (0, wenn unbekannt).
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// Perzentil eines Messwerts über den Ring (`f` wählt den Wert, 0…1).
    pub fn perzentil_von(&self, q: f32, f: fn(&Bild) -> f32) -> f32 {
        let mut werte: Vec<f32> = self.ring.iter().map(f).collect();
        perzentil(&mut werte, q)
    }

    /// Anteil der Bilder, deren Displayliste aus dem Zwischenspeicher kam.
    pub fn aus_dem_zwischenspeicher(&self) -> f32 {
        if self.ring.is_empty() {
            return 0.0;
        }
        let kopien = self.ring.iter().filter(|b| !b.liste_neu).count();
        kopien as f32 / self.ring.len() as f32
    }

    /// Letztes abgeschlossenes Bild (für die Anzeige).
    pub fn letztes(&self) -> Option<Bild> {
        self.letzte
    }

    /// Wurde die Displayliste in **diesem** Bild neu aufgebaut? Die Anzeige
    /// zeichnet vor `bild_beenden`, deshalb der laufende Stand — sonst stünde
    /// dort immer die Herkunft des vorigen Bildes.
    pub fn liste_jetzt_neu(&self) -> bool {
        self.laufend.liste_neu
    }
}

/// Perzentil einer Messreihe (0…1). Nächstgelegene Stichprobe, nicht
/// interpoliert — bei 240 Werten ist der Unterschied zur Interpolation
/// bedeutungslos und die Stichprobe ist ehrlicher.
pub fn perzentil(werte: &mut [f32], q: f32) -> f32 {
    if werte.is_empty() {
        return 0.0;
    }
    werte.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let i = ((werte.len() - 1) as f32 * q.clamp(0.0, 1.0)).round() as usize;
    werte[i.min(werte.len() - 1)]
}

// ---------------------------------------------------------------------------
// Systemwerte
// ---------------------------------------------------------------------------

/// Belegter Speicher des eigenen Prozesses in MB.
#[cfg(target_os = "linux")]
fn belegter_speicher_mb() -> f32 {
    // /proc/self/statm: Feld 2 = belegte Seiten. Die Seitengröße steht nicht in
    // der Datei; 4 KiB trifft x86_64 und die üblichen ARM-Kerne.
    let Ok(text) = std::fs::read_to_string("/proc/self/statm") else {
        return 0.0;
    };
    let seiten: f32 = text
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    seiten * 4096.0 / (1024.0 * 1024.0)
}

#[cfg(not(target_os = "linux"))]
fn belegter_speicher_mb() -> f32 {
    0.0
}

/// Threadzahl des eigenen Prozesses.
#[cfg(target_os = "linux")]
fn threadzahl() -> usize {
    let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    text.lines()
        .find_map(|l| l.strip_prefix("Threads:").and_then(|v| v.trim().parse().ok()))
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn threadzahl() -> usize {
    0
}

// ---------------------------------------------------------------------------
// Anzeige
// ---------------------------------------------------------------------------

/// Millisekunden mit deutschem Komma. Unter 1 ms zwei Nachkommastellen —
/// sonst steht bei den Unterposten überall „0,0“.
fn ms_text(v: f32) -> String {
    let text = if v < 0.995 {
        format!("{v:>6.2}")
    } else {
        format!("{v:>6.1}")
    };
    text.replace('.', ",")
}

/// Ganzzahl mit Tausenderpunkt (8.452).
fn zahl_text(n: usize) -> String {
    let roh = n.to_string();
    let mut out = String::with_capacity(roh.len() + roh.len() / 3);
    for (i, c) in roh.chars().enumerate() {
        if i > 0 && (roh.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Farbe für eine Rechenzeit: grün im 60-Bilder-Takt, gelb bis 33 ms,
/// darüber rot (dann ruckelt selbst das Ziehen).
fn zeit_farbe(ms: f32) -> Color32 {
    if ms <= 8.0 {
        Palette::OK
    } else if ms <= 16.7 {
        Palette::ACCENT
    } else {
        Palette::DANGER
    }
}

/// Feste Spaltenbreiten der Tabellen (px).
///
/// Ohne feste Breite bestimmt der **längste** Text einer Spalte ihre Breite —
/// und damit wandert die ganze Tabelle, sobald sich ein Text ändert. Genau das
/// war beim Wechsel „neu aufgebaut“ ↔ „aus dem Zwischenspeicher“ als Flackern
/// zu sehen (T0068, Turn 2).
const NAME_W: f32 = 92.0;
/// Breite einer Zahlenspalte (px).
const WERT_W: f32 = 50.0;
/// Zeilenhöhe einer Tabellenzeile (px).
const ZEILE_H: f32 = 14.0;

/// Eine Zelle fester Breite: der Inhalt bestimmt die Spalte nicht mehr.
fn zelle(ui: &mut egui::Ui, text: RichText, breite: f32) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(breite, ZEILE_H),
        Layout::left_to_right(Align::Center),
        |ui| ui.label(text),
    )
    .inner
}

/// Eine Zeile mit drei Zahlenspalten.
fn zeile(ui: &mut egui::Ui, name: &str, werte: [String; 3], farbe: Color32, hinweis: &str) {
    zelle(ui, theme_mono(name).color(Palette::TEXT_DIM), NAME_W).on_hover_text(hinweis);
    for w in werte {
        zelle(ui, theme_mono(&w).color(farbe), WERT_W);
    }
    ui.end_row();
}

fn theme_mono(text: &str) -> RichText {
    RichText::new(text.to_string()).monospace().size(10.5)
}

fn kopf(ui: &mut egui::Ui, text: &str) {
    zelle(
        ui,
        RichText::new(text)
            .monospace()
            .size(10.0)
            .color(Palette::TEXT_FAINT),
        NAME_W,
    );
}

/// Kopf einer Zahlenspalte.
fn kopf_wert(ui: &mut egui::Ui, text: &str) {
    zelle(
        ui,
        RichText::new(text)
            .monospace()
            .size(10.0)
            .color(Palette::TEXT_FAINT),
        WERT_W,
    );
}

/// Die Leistungsanzeige über der Zeichenfläche (Umschalt+F3).
///
/// Sie nimmt **keine Klicks an**: sie liegt über der Zeichnung, darf aber kein
/// Werkzeug blockieren und keinen Zeiger fangen.
pub fn overlay(app: &App, ctx: &egui::Context) {
    if !app.perf.an {
        return;
    }
    let flaeche = app.ui.last_canvas;
    let breite = 366.0_f32.min(flaeche.width() - 24.0);
    if breite < 260.0 || flaeche.height() < 240.0 {
        return;
    }
    let pos = egui::pos2(flaeche.right() - breite - 12.0, flaeche.top() + 12.0);
    egui::Area::new(Id::new("leistungsanzeige"))
        .order(Order::Foreground)
        .interactable(false)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(Palette::PANEL)
                .stroke(Stroke::new(1.0_f32, Palette::BORDER))
                .rounding(egui::Rounding::same(4.0))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(breite - 20.0);
                    inhalt(app, ui);
                });
        });
}

fn inhalt(app: &App, ui: &mut egui::Ui) {
    let perf = &app.perf;
    let letzt = perf.letztes();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(jjcad_core::i18n::tr("gui.perf.leistung"))
                .size(12.0)
                .color(Palette::ACCENT)
                .strong(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(jjcad_core::i18n::tr("gui.perf.umschalt_f3"))
                    .size(10.0)
                    .color(Palette::TEXT_FAINT),
            );
        });
    });

    ui.add_space(2.0);
    ui.label(
        RichText::new(jjcad_core::i18n::tr("gui.perf.bilder_entstehen_nur_bei_bedarf_kein_fester_takt"))
            .size(10.0)
            .color(Palette::TEXT_FAINT),
    );
    ui.add_space(4.0);

    // Perzentile über den Verlauf
    egui::Grid::new("leistung_bilder")
        .num_columns(4)
        .spacing([7.0, 2.0])
        .show(ui, |ui| {
            kopf(ui, jjcad_core::i18n::tr("gui.perf.bilder_ms"));
            kopf_wert(ui, "  p50");
            kopf_wert(ui, "  p95");
            kopf_wert(ui, "  p99");
            ui.end_row();
            let reihen: [(&str, fn(&Bild) -> f32, &str); 3] = [
                (
                    jjcad_core::i18n::tr("gui.perf.abstand"),
                    |b: &Bild| b.abstand,
                    jjcad_core::i18n::tr("gui.perf.hinweis_abstand"),
                ),
                (
                    jjcad_core::i18n::tr("gui.perf.rechnen"),
                    |b: &Bild| b.cpu,
                    jjcad_core::i18n::tr("gui.perf.hinweis_rechnen"),
                ),
                (
                    jjcad_core::i18n::tr("gui.perf.ausserhalb"),
                    |b: &Bild| b.ausserhalb(),
                    jjcad_core::i18n::tr("gui.perf.hinweis_ausserhalb"),
                ),
            ];
            for (name, f, hinweis) in reihen {
                let werte = [
                    ms_text(perf.perzentil_von(0.5, f)),
                    ms_text(perf.perzentil_von(0.95, f)),
                    ms_text(perf.perzentil_von(0.99, f)),
                ];
                let farbe = if name == "Rechnen" {
                    zeit_farbe(perf.perzentil_von(0.95, f))
                } else {
                    Palette::TEXT
                };
                zeile(ui, name, werte, farbe, hinweis);
            }
        });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(3.0);

    // Aufteilung der Rechenzeit des letzten Bildes
    egui::Grid::new("leistung_abschnitte")
        .num_columns(3)
        .spacing([7.0, 2.0])
        .show(ui, |ui| {
            kopf(ui, jjcad_core::i18n::tr("gui.perf.abschnitte"));
            kopf_wert(ui, jjcad_core::i18n::tr("gui.perf.jetzt"));
            kopf_wert(ui, jjcad_core::i18n::tr("gui.perf.anteil"));
            ui.end_row();
            for a in Abschnitt::ALLE {
                let (jetzt, anteil) = match letzt {
                    Some(b) => (
                        ms_text(b.zeit(a)),
                        format!("{:>4.0} %", b.anteil(a) * 100.0),
                    ),
                    None => ("     —".to_string(), "     —".to_string()),
                };
                zelle(ui, theme_mono(a.name()).color(Palette::TEXT_DIM), NAME_W)
                    .on_hover_text(a.erklaerung());
                zelle(ui, theme_mono(&jetzt).color(Palette::TEXT), WERT_W);
                zelle(
                    ui,
                    theme_mono(&anteil).color(Palette::TEXT_FAINT),
                    WERT_W,
                );
                ui.end_row();
                // Die Posten, in die „Zeichnen“ zerfällt.
                if a == Abschnitt::Zeichnen {
                    if let Some(b) = letzt {
                        for k in [MalKlasse::Fuellung, MalKlasse::Linie, MalKlasse::Text] {
                            let name = match k {
                                MalKlasse::Fuellung => jjcad_core::i18n::tr("gui.perf.fuellungen"),
                                MalKlasse::Linie => jjcad_core::i18n::tr("gui.perf.linien"),
                                MalKlasse::Text => jjcad_core::i18n::tr("gui.perf.text"),
                            };
                            zelle(ui, theme_mono(name).color(Palette::TEXT_FAINT), NAME_W);
                            zelle(
                                ui,
                                theme_mono(&ms_text(b.malzeit(k))).color(Palette::TEXT_FAINT),
                                WERT_W,
                            );
                            zelle(ui, theme_mono(""), WERT_W);
                            ui.end_row();
                        }
                    }
                }
            }
        });

    // Herkunft der Displayliste in einer **eigenen Zeile**: in einer
    // Tabellenspalte verschob der wechselnde Text die ganze Tabelle, und beim
    // häufigen Wechsel „neu aufgebaut“ ↔ „aus dem Zwischenspeicher“ sah das aus
    // wie ein Flackern (T0068, Turn 2). Die Zeile steht immer da, damit auch die
    // Höhe der Anzeige gleich bleibt.
    let herkunft = if perf.liste_jetzt_neu() {
        jjcad_core::i18n::tr("gui.perf.displayliste_neu_aufgebaut")
    } else {
        jjcad_core::i18n::tr("gui.perf.displayliste_aus_dem_zwischenspeicher")
    };
    ui.label(theme_mono(herkunft).color(Palette::TEXT_FAINT));

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(3.0);

    // Zeichnung und Prozess
    let objekte = app.doc().ents().len();
    let mut zeile1 = jjcad_core::i18n::t_args_owned("gui.perf.x_objekte", &[("wert1", zahl_text(objekte))]);
    if let Some(b) = letzt {
        if b.objekte > 0 && b.objekte != objekte {
            zeile1.push_str(&jjcad_core::i18n::t_args_owned("gui.perf.liste_x", &[("wert1", format!("{}", zahl_text(b.objekte)))]));
        }
        zeile1.push_str(&jjcad_core::i18n::t_args_owned("gui.perf.x_grafikelemente", &[("wert1", format!("{}", zahl_text(b.grafikelemente)))]));
    }
    ui.label(theme_mono(&zeile1).color(Palette::TEXT_DIM));
    if let Some(b) = letzt {
        ui.label(
            theme_mono(&jjcad_core::i18n::t_args_owned(
                "gui.perf.fuellungen_x_linien_x_text_x",
                &[
                    ("fuellungen", zahl_text(b.fuellungen).to_string()),
                    ("linien", zahl_text(b.linien).to_string()),
                    ("texte", zahl_text(b.texte).to_string()),
                ],
            ))
            .color(Palette::TEXT_FAINT),
        );
    }
    ui.label(
        theme_mono(&jjcad_core::i18n::t_args_owned(
            "gui.perf.displayliste_x_prozent_zwischenspeicher",
            &[(
                "anteil",
                format!("{:.0}", perf.aus_dem_zwischenspeicher() * 100.0),
            )],
        ))
        .color(Palette::TEXT_FAINT),
    );

    let zoom = app.current_view().zoom;
    // Bei weit herausgezoomten Zeichnungen sind zwei Nachkommastellen nur
    // Nullen — dann eine mehr.
    let mut zeile2 = if zoom < 0.095 {
        format!("Zoom {} px/mm", format!("{zoom:.3}").replace('.', ","))
    } else {
        format!("Zoom {} px/mm", format!("{zoom:.2}").replace('.', ","))
    };
    if perf.speicher_mb() > 0.0 {
        zeile2.push_str(&jjcad_core::i18n::t_args_owned("gui.perf.x_mb", &[("wert1", format!("{:.0}", perf.speicher_mb()))]));
    }
    if perf.threads() > 0 {
        zeile2.push_str(&jjcad_core::i18n::t_args_owned("gui.perf.x_threads", &[("wert1", format!("{}", perf.threads()))]));
    }
    ui.label(theme_mono(&zeile2).color(Palette::TEXT_FAINT));

    ui.add_space(2.0);
    ui.label(
        RichText::new(jjcad_core::i18n::t_args_owned("gui.perf.verlauf_x_bilder_x_pausen", &[("wert1", format!("{}", perf.ring.len())), ("wert2", format!("{}", perf.pausen))]))
        .size(9.5)
        .color(Palette::TEXT_FAINT),
    );
}

#[cfg(test)]
mod tests {
    //! T0068: Messkern der Leistungsanzeige.
    //!
    //! Geprüft werden Perzentile, der Ring der letzten Bilder (Pausen ausgenommen)
    //! und dass die Abschnitte die Rechenzeit eines Bildes **ergeben** — sonst
    //! verteilt sich die Zeit auf dem Papier auf mehr oder weniger als 100 %.

    use super::*;
    use std::time::Duration;

    fn bild(abstand: f32) -> Bild {
        Bild {
            abstand,
            ..Bild::default()
        }
    }

    #[test]
    fn perzentile_liegen_auf_stichproben() {
        let mut werte: Vec<f32> = (0..=100).map(|i| i as f32).collect();
        assert_eq!(perzentil(&mut werte, 0.5), 50.0);
        assert_eq!(perzentil(&mut werte, 0.95), 95.0);
        assert_eq!(perzentil(&mut werte, 0.99), 99.0);
        assert_eq!(perzentil(&mut werte, 0.0), 0.0);
        assert_eq!(perzentil(&mut werte, 1.0), 100.0);
        let mut leer: [f32; 0] = [];
        assert_eq!(perzentil(&mut leer, 0.5), 0.0, "ohne Messungen 0, nicht NaN");
    }

    #[test]
    fn ring_behaelt_die_letzten_bilder() {
        let mut p = Perf::default();
        for i in 0..(RING + 20) {
            p.merken(bild(10.0 + i as f32));
        }
        assert_eq!(p.ring.len(), RING, "der Ring ist auf {RING} Bilder begrenzt");
        assert_eq!(p.ring.front().unwrap().abstand, 30.0, "das älteste fällt heraus");
        assert_eq!(
            p.ring.back().unwrap().abstand,
            10.0 + (RING + 19) as f32
        );
        assert_eq!(p.pausen, 0);
    }

    #[test]
    fn pausen_gehen_nicht_in_die_perzentile() {
        let mut p = Perf::default();
        p.merken(bild(16.0));
        p.merken(bild(PAUSE_MS + 4000.0));
        assert_eq!(p.ring.len(), 1, "ein Pausenbild verfälscht den Verlauf nicht");
        assert_eq!(p.pausen, 1);
        // Das erste Bild eines Laufs hat keinen Vorgänger und wird nicht gemerkt.
        p.merken(bild(0.0));
        assert_eq!(p.ring.len(), 1);
    }

    #[test]
    fn langsames_bild_ist_keine_pause() {
        // Eine große Zeichnung braucht selbst mehrere Sekunden. Solche Bilder
        // müssen im Verlauf stehen, sonst zeigt die Anzeige genau dann nichts,
        // wenn man sie braucht.
        let mut p = Perf::default();
        let mut langsam = bild(PAUSE_MS + 3000.0);
        langsam.cpu = PAUSE_MS + 2500.0;
        p.merken(langsam);
        assert_eq!(p.pausen, 0, "ein Bild, das selbst lange rechnet, ist keine Pause");
        assert_eq!(p.ring.len(), 1, "es muss im Verlauf stehen");

        // Ein reines Pausenbild bleibt trotzdem als „letztes Bild“ sichtbar —
        // die Abschnittstabelle darf nicht leer werden.
        let mut p = Perf::default();
        p.merken(bild(5000.0));
        assert_eq!(p.ring.len(), 0);
        assert!(p.letztes().is_some(), "das letzte Bild gehört in die Anzeige");
    }

    #[test]
    fn abschnitte_ergeben_die_rechenzeit() {
        let mut p = Perf::default();
        // Erstes Bild: Oberfläche offen, dann Zeichnen; danach Bildende.
        p.bild_beginnen();
        std::thread::sleep(Duration::from_millis(4));
        p.abschnitt(Abschnitt::Zeichnen);
        std::thread::sleep(Duration::from_millis(4));
        p.bild_beenden();
        // Zweites Bild füllt den Ring (der Abstand zum Vorgänger ist jetzt > 0)
        // und muss dieselbe Aufteilung zeigen.
        p.bild_beginnen();
        std::thread::sleep(Duration::from_millis(4));
        p.abschnitt(Abschnitt::Zeichnen);
        std::thread::sleep(Duration::from_millis(4));
        p.bild_beenden();

        let b = p.letztes().expect("das zweite Bild muss im Ring liegen");
        assert!(
            b.zeit(Abschnitt::Oberflaeche) >= 3.5,
            "die Zeit bis zum Abschnittswechsel gehört dem vorigen Abschnitt (Oberfläche: {} ms)",
            b.zeit(Abschnitt::Oberflaeche)
        );
        assert!(
            b.zeit(Abschnitt::Zeichnen) >= 3.5,
            "der offene Abschnitt bekommt die Zeit bis zum Bildende (Zeichnen: {} ms)",
            b.zeit(Abschnitt::Zeichnen)
        );
        let summe: f32 = Abschnitt::ALLE.iter().map(|a| b.zeit(*a)).sum();
        assert!(
            (summe - b.cpu).abs() < 0.01,
            "die Abschnitte müssen die Rechenzeit ergeben ({summe} ms gegen {} ms)",
            b.cpu
        );
        let anteile: f32 = Abschnitt::ALLE.iter().map(|a| b.anteil(*a)).sum();
        assert!(
            (anteile - 1.0).abs() < 0.01,
            "die Anteile müssen sich zu 100 % addieren ({anteile})"
        );
        assert!(b.ausserhalb() >= 0.0, "außerhalb darf nicht negativ werden");
    }

    #[test]
    fn grafikelemente_zaehlen_nach_art() {
        // Füllung, Linie, Text — die drei Posten, in die „Zeichnen“ zerfällt.
        let mut p = Perf::default();
        p.bild_beginnen();
        let fuellung = render::Prim {
            shape: render::Shape::Fill {
                outer: vec![],
                holes: vec![],
            },
            style: render::PrimStyle::default(),
            entity: 1,
            layer: 1,
        };
        let linie = render::Prim {
            shape: render::Shape::Line(
                jjcad_core::geom::Vec2::new(0.0, 0.0),
                jjcad_core::geom::Vec2::new(10.0, 0.0),
            ),
            style: render::PrimStyle::default(),
            entity: 1,
            layer: 1,
        };
        for _ in 0..2 {
            p.grafikelement(&fuellung, Duration::from_micros(500));
        }
        for _ in 0..3 {
            p.grafikelement(&linie, Duration::from_micros(200));
        }
        let b = p.laufend;
        assert_eq!(b.grafikelemente, 5);
        assert_eq!((b.fuellungen, b.linien, b.texte), (2, 3, 0));
        assert!(b.malzeit(MalKlasse::Fuellung) > b.malzeit(MalKlasse::Linie));
    }

    #[test]
    fn zwischengespeicherte_liste_wird_gezaehlt() {
        let mut p = Perf::default();
        let mut neu = bild(16.0);
        neu.liste_neu = true;
        p.merken(neu);
        p.merken(bild(16.0));
        p.merken(bild(16.0));
        assert!(
            (p.aus_dem_zwischenspeicher() - 2.0 / 3.0).abs() < 0.01,
            "ein Neubau von drei Bildern heißt 67 % aus dem Zwischenspeicher, gemessen {}",
            p.aus_dem_zwischenspeicher()
        );
    }
}
