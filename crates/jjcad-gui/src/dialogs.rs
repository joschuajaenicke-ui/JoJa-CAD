//! Dialoge: Dateien, Text, Einstellungen, Tastaturbelegung, Hilfe, Projekt.

use crate::app::App;
use crate::panels;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::geom::{Vec2 as V2};
use jjcad_core::persist::{
    fest_belegt, Design, DragMode, Farbakzent, Fensterrahmen, PastePivot, Textaufloesung,
};
use jjcad_core::units::Unit;
use serde_json::json;

// ---------------------------------------------------------------------------
// Dateidialog
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileMode {
    Open,
    SaveJjcad,
    ImportDxf,
    /// PDF einfügen (Plan `docs/PDF-IMPORT.md`)
    ImportPdf,
    /// Bild einfügen (PNG oder JPEG, T0100)
    ImportImage,
    ExportPdf,
    ExportDxf,
    ExportPng,
    /// Tastaturlayout als Datei einlesen (T0076)
    ImportKeymap,
    /// Tastaturlayout als Datei schreiben (T0076)
    ExportKeymap,
}

impl FileMode {
    pub fn ext(self) -> &'static str {
        match self {
            FileMode::Open | FileMode::SaveJjcad => "jjcad",
            FileMode::ImportDxf | FileMode::ExportDxf => "dxf",
            FileMode::ImportPdf => "pdf",
            // Beide Formate sind gemeint; der Filter unten lässt png/jpg/jpeg zu.
            FileMode::ImportImage => "png",
            FileMode::ExportPdf => "pdf",
            FileMode::ExportPng => "png",
            FileMode::ImportKeymap | FileMode::ExportKeymap => "json",
        }
    }
    fn title(self) -> &'static str {
        match self {
            FileMode::Open => jjcad_core::i18n::tr("gui.dialogs.zeichnung_oeffnen"),
            FileMode::SaveJjcad => jjcad_core::i18n::tr("gui.dialogs.zeichnung_speichern"),
            FileMode::ImportDxf => jjcad_core::i18n::tr("gui.dialogs.dxf_importieren"),
            FileMode::ImportPdf => jjcad_core::i18n::tr("gui.dialogs.pdf_einfuegen"),
            FileMode::ImportImage => jjcad_core::i18n::tr("gui.dialogs.bild_einfuegen"),
            FileMode::ExportPdf => jjcad_core::i18n::tr("gui.dialogs.als_pdf_exportieren"),
            FileMode::ExportDxf => jjcad_core::i18n::tr("gui.dialogs.als_dxf_exportieren"),
            FileMode::ExportPng => jjcad_core::i18n::tr("gui.dialogs.als_png_exportieren"),
            FileMode::ImportKeymap => jjcad_core::i18n::tr("gui.dialogs.tastaturlayout_importieren"),
            FileMode::ExportKeymap => jjcad_core::i18n::tr("gui.dialogs.tastaturlayout_exportieren"),
        }
    }
    fn action_label(self) -> &'static str {
        match self {
            FileMode::Open => jjcad_core::i18n::tr("gui.dialogs.oeffnen"),
            FileMode::ImportDxf | FileMode::ImportKeymap | FileMode::ImportImage => {
                jjcad_core::i18n::tr("gui.dialogs.importieren")
            }
            _ => jjcad_core::i18n::tr("gui.dialogs.speichern"),
        }
    }
    fn is_save(self) -> bool {
        matches!(
            self,
            FileMode::SaveJjcad
                | FileMode::ExportPdf
                | FileMode::ExportDxf
                | FileMode::ExportPng
                | FileMode::ExportKeymap
        )
    }
    /// Anzeigename des Dateifilters (ohne Endung) für den Systemdialog.
    fn filter_name(self) -> &'static str {
        match self {
            FileMode::Open | FileMode::SaveJjcad => jjcad_core::i18n::tr("gui.dialogs.zeichnung"),
            FileMode::ImportDxf | FileMode::ExportDxf => jjcad_core::i18n::tr("gui.dialogs.dxf_zeichnung"),
            FileMode::ImportPdf => jjcad_core::i18n::tr("gui.dialogs.pdf_dokument"),
            FileMode::ImportImage => jjcad_core::i18n::tr("gui.dialogs.bild"),
            FileMode::ExportPdf => jjcad_core::i18n::tr("gui.dialogs.pdf_dokument"),
            FileMode::ExportPng => jjcad_core::i18n::tr("gui.dialogs.png_bild"),
            FileMode::ImportKeymap | FileMode::ExportKeymap => jjcad_core::i18n::tr("gui.dialogs.tastaturlayout"),
        }
    }
    /// Anfrage an den Systemdialog des Betriebssystems.
    pub fn anfrage(self, start_dir: std::path::PathBuf, base: &str) -> crate::systemdialog::Anfrage {
        let name = if base.trim().is_empty() { jjcad_core::i18n::tr("gui.dialogs.zeichnung") } else { base.trim() };
        let dateiname = if self.is_save() {
            Some(format!("{}.{}", name, self.ext()))
        } else {
            None
        };
        crate::systemdialog::Anfrage::neu(
            self.title(),
            self.filter_name(),
            self.ext(),
            Some(start_dir),
            dateiname,
            self.is_save(),
        )
    }

    /// Welcher Import gehört zu dieser Datei? `None` = das Format lässt sich
    /// nicht importieren (T0115).
    ///
    /// Die **eine** Tabelle für „Datei fällt ins Fenster“ — der Vergleich
    /// ignoriert Groß- und Kleinschreibung. Wer ein Format ergänzt, ergänzt es
    /// hier und nicht an der Ablagestelle.
    pub fn import_aus_datei(pfad: &std::path::Path) -> Option<FileMode> {
        let endung = pfad.extension()?.to_string_lossy().to_ascii_lowercase();
        match endung.as_str() {
            "jjcad" => Some(FileMode::Open),
            "dxf" => Some(FileMode::ImportDxf),
            "pdf" => Some(FileMode::ImportPdf),
            // `bild_importieren` prüft die Datei selbst; die Endung genügt hier.
            "png" | "jpg" | "jpeg" => Some(FileMode::ImportImage),
            _ => None,
        }
    }
}

struct Entry {
    name: String,
    is_dir: bool,
    size: u64,
}

pub struct FileDialog {
    pub mode: FileMode,
    pub dir: std::path::PathBuf,
    pub name: String,
    pub error: Option<String>,
    entries: Vec<Entry>,
    dirty: bool,
    focus_name: bool,
}

impl FileDialog {
    pub fn new(mode: FileMode, dir: std::path::PathBuf, base: &str) -> Self {
        let base = if base.trim().is_empty() { jjcad_core::i18n::tr("gui.dialogs.zeichnung") } else { base };
        let name = if mode.is_save() {
            format!("{}.{}", base, mode.ext())
        } else {
            String::new()
        };
        Self {
            mode,
            dir,
            name,
            error: None,
            entries: Vec::new(),
            dirty: true,
            focus_name: true,
        }
    }

    fn scan(&mut self) {
        self.entries.clear();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let path = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let is_dir = path.is_dir();
                if !is_dir {
                    let ext = path
                        .extension()
                        .map(|x| x.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if self.mode == FileMode::Open && ext != "jjcad" {
                        continue;
                    }
                    if self.mode == FileMode::ImportDxf && ext != "dxf" {
                        continue;
                    }
                    if self.mode == FileMode::ImportKeymap && ext != "json" {
                        continue;
                    }
                    // Bilder: PNG und JPEG (beide Schreibweisen der Endung).
                    if self.mode == FileMode::ImportImage
                        && !matches!(ext.as_str(), "png" | "jpg" | "jpeg")
                    {
                        continue;
                    }
                }
                let size = if is_dir {
                    0
                } else {
                    e.metadata().map(|m| m.len()).unwrap_or(0)
                };
                self.entries.push(Entry { name, is_dir, size });
            }
        }
        self.entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
        self.dirty = false;
    }

    fn show(&mut self, ctx: &egui::Context) -> Option<std::path::PathBuf> {
        if self.dirty {
            self.scan();
        }
        let mut result = None;
        let mut close = false;
        let mut win_open = true;
        egui::Window::new(self.mode.title())
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(720.0, 500.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut win_open)
            .frame(theme::dialog_frame())
            .show(ctx, |ui| {
                ui.set_width(700.0);
                // Pfadzeile
                ui.horizontal(|ui| {
                    if widgets::icon_button(ui, "chevron_right", 26.0, false, jjcad_core::i18n::tr("gui.dialogs.uebergeordneter_ordner")).clicked() {
                        if let Some(p) = self.dir.parent() {
                            self.dir = p.to_path_buf();
                            self.dirty = true;
                        }
                    }
                    if ui.button(jjcad_core::i18n::tr("gui.dialogs.home")).clicked() {
                        if let Ok(h) = std::env::var("HOME") {
                            self.dir = std::path::PathBuf::from(h);
                            self.dirty = true;
                        }
                    }
                    if ui.button(jjcad_core::i18n::tr("gui.dialogs.projekte")).clicked() {
                        self.dir = std::env::current_dir().unwrap_or_else(|_| self.dir.clone());
                        self.dirty = true;
                    }
                    let path_text = self.dir.to_string_lossy().to_string();
                    ui.add_sized(
                        [ui.available_width(), 22.0],
                        egui::Label::new(theme::mono(path_text)).truncate(),
                    );
                });
                ui.add_space(4.0);
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(330.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.dir.parent().is_some() {
                            let resp = ui.add_sized(
                                [ui.available_width(), 24.0],
                                egui::SelectableLabel::new(false, jjcad_core::i18n::tr("gui.dialogs.uebergeordnet")),
                            );
                            if resp.double_clicked() || resp.clicked() {
                                if let Some(p) = self.dir.parent() {
                                    self.dir = p.to_path_buf();
                                    self.dirty = true;
                                }
                            }
                        }
                        let mut enter: Option<std::path::PathBuf> = None;
                        for e in &self.entries {
                            let label = if e.is_dir {
                                format!("[DIR]  {}", e.name)
                            } else {
                                format!("      {}   ({})", e.name, human_size(e.size))
                            };
                            let resp = ui.add_sized(
                                [ui.available_width(), 24.0],
                                egui::SelectableLabel::new(
                                    !e.is_dir
                                        && self.name == e.name,
                                    label,
                                ),
                            );
                            if resp.double_clicked() || (e.is_dir && resp.clicked()) {
                                if e.is_dir {
                                    enter = Some(self.dir.join(&e.name));
                                } else {
                                    self.name = e.name.clone();
                                    result = Some(self.dir.join(&self.name));
                                }
                            } else if resp.clicked() && !e.is_dir {
                                self.name = e.name.clone();
                            }
                        }
                        if let Some(p) = enter {
                            self.dir = p;
                            self.dirty = true;
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.dateiname")));
                    let resp = ui.add_sized(
                        [420.0, 24.0],
                        egui::TextEdit::singleline(&mut self.name).hint_text(jjcad_core::i18n::t_args_owned("gui.dialogs.name_x", &[("wert1", format!("{}", self.mode.ext()))])),
                    );
                    if resp.changed() {
                        self.error = None;
                    }
                    if self.focus_name {
                        resp.request_focus();
                        self.focus_name = false;
                    }
                    if self.mode.is_save() {
                        ui.label(theme::dim(format!(".{}", self.mode.ext())));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Some(p) = self.resolve_path() {
                            result = Some(p);
                        }
                    }
                });
                if let Some(err) = &self.error {
                    ui.label(RichText::new(err).color(Palette::danger()).size(11.5));
                }
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.mode.action_label()).clicked() {
                        if let Some(p) = self.resolve_path() {
                            result = Some(p);
                        }
                    }
                    if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                        close = true;
                    }
                    if self.mode == FileMode::Open {
                        for (label, dir) in [
                            (jjcad_core::i18n::tr("gui.dialogs.dokumente"), dirs::documents()),
                            (jjcad_core::i18n::tr("gui.dialogs.schreibtisch"), dirs::desktop()),
                        ] {
                            if let Some(d) = dir {
                                if ui.button(label).clicked() {
                                    self.dir = d;
                                    self.dirty = true;
                                }
                            }
                        }
                    }
                });
            });
        if close || !win_open {
            return None;
        }
        result
    }

    fn resolve_path(&mut self) -> Option<std::path::PathBuf> {
        let mut n = self.name.trim().to_string();
        if n.is_empty() {
            self.error = Some(jjcad_core::i18n::tr("gui.dialogs.bitte_einen_dateinamen_angeben").into());
            return None;
        }
        if self.mode.is_save()
            || self.mode == FileMode::ImportDxf
            || self.mode == FileMode::ImportKeymap
        {
            let ext = self.mode.ext();
            if !n.to_lowercase().ends_with(&format!(".{ext}")) {
                n = format!("{n}.{ext}");
            }
        }
        let p = self.dir.join(n);
        if self.mode == FileMode::Open && !p.exists() {
            self.error = Some(jjcad_core::i18n::tr("gui.dialogs.datei_existiert_nicht").into());
            return None;
        }
        Some(p)
    }
}

fn human_size(n: u64) -> String {
    if n > 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n > 1024 {
        format!("{:.0} kB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

/// Einfache Standardverzeichnisse (ohne zusätzliche Kisten).
mod dirs {
    use std::path::PathBuf;
    pub fn home() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
    pub fn documents() -> Option<PathBuf> {
        let h = home()?;
        let p = h.join("Documents");
        if p.exists() {
            Some(p)
        } else {
            let p = h.join("Dokumente");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        }
    }
    pub fn desktop() -> Option<PathBuf> {
        let h = home()?;
        for n in ["Desktop", "Schreibtisch"] {
            let p = h.join(n);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Gemeinsame Bausteine
// ---------------------------------------------------------------------------

fn dim_background(ctx: &egui::Context) {
    egui::Area::new(egui::Id::new("modal_bg"))
        .order(egui::Order::Background)
        .fixed_pos(ctx.screen_rect().min)
        .interactable(false)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                ctx.screen_rect(),
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 110),
            );
        });
}

/// Knopfzeile am rechten Rand eines modalen Dialogs. Die Höhe ist begrenzt,
/// weil `with_layout` den verfügbaren Platz sonst voll ausfüllt und das
/// modale Fenster dadurch Frame für Frame auf die halbe Bildschirmhöhe wächst.
fn button_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), 30.0),
        Layout::right_to_left(Align::Center),
        add,
    );
}

fn modal_window<'a>(
    _ctx: &'a egui::Context,
    title: &str,
    width: f32,
    open: &'a mut bool,
) -> egui::Window<'a> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .default_width(width)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .frame(theme::dialog_frame())
}

/// Was nach einer Dateiauswahl geschieht — für den eingebauten Dialog und den
/// Systemdialog gemeinsam.
pub fn datei_gewaehlt(app: &mut App, mode: FileMode, path: std::path::PathBuf) {
    match mode {
        FileMode::Open => app.load_document(&path),
        FileMode::SaveJjcad => {
            // T0143: Kam der Dialog aus „Speichern und schließen“, wird der
            // Reiter danach abgelegt. Er ist dafür aktiv geworden
            // (`App::close_tab_speichern_mit_ziel`), `session_mut` trifft also
            // genau ihn.
            let schliessen = app.ui.schliessen_nach_speichern.take();
            match app.session_mut().save(&path) {
                Ok(_) => {
                    app.prefs.add_recent(&path.to_string_lossy());
                    app.prefs.last_dir = path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let _ = app.prefs.save();
                    // T0129: der neueste Stand ist jetzt der Dateiinhalt.
                    let aktiv = app.active;
                    app.sicherung_dateistand(aktiv);
                    app.toast(jjcad_core::i18n::t_args_owned(
                        "gui.dialogs.datei_gespeichert_x",
                        &[("wert1", format!("{}", path.display()))],
                    ));
                    if let Some(idx) = schliessen {
                        app.close_tab(idx);
                    }
                }
                Err(e) => app.toast(e),
            }
        }
        FileMode::ImportDxf => app.import_dxf_from(path),
        FileMode::ImportPdf => app.pdf_import_oeffnen(path),
        FileMode::ImportImage => app.bild_importieren(path),
        FileMode::ExportPdf => app.export_pdf_to(path),
        FileMode::ExportDxf => app.export_dxf_to(path),
        FileMode::ExportPng => app.export_png_to(path),
        // Tastaturlayouts (T0076): Import übernimmt das Layout sofort, Export
        // schreibt die gerade gültige Belegung.
        FileMode::ImportKeymap => match app.prefs.keymap_importieren(&path) {
            Ok(name) => {
                let _ = app.prefs.save();
                app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.layout_importiert_x", &[("name", format!("{}", name))]));
            }
            Err(e) => app.toast(e),
        },
        FileMode::ExportKeymap => match app.prefs.keymap_exportieren_aktuell(&path) {
            Ok(name) => app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.layout_exportiert_x", &[("name", format!("{}", name))])),
            Err(e) => app.toast(e),
        },
    }
}

/// Dateien, die per Drag an Drop im Fenster abgelegt wurden (T0115).
///
/// Der Weg ist derselbe wie nach einer Auswahl im Dateidialog: die Endung
/// bestimmt über [`FileMode::import_aus_datei`] den Modus, ausgeführt wird über
/// [`datei_gewaehlt`]. Kein zweiter Importpfad neben dem bestehenden.
pub fn dateien_fallen_gelassen(app: &mut App, pfade: &[std::path::PathBuf]) {
    for pfad in pfade {
        match FileMode::import_aus_datei(pfad) {
            // Eine Seitenwahl steht schon offen: eine zweite PDF-Datei würde
            // sie still überschreiben.
            Some(FileMode::ImportPdf) if app.ui.pdf_import.is_some() => {
                app.toast(jjcad_core::i18n::tr("gui.dialogs.pdf_seitenwahl_offen"));
            }
            Some(mode) => datei_gewaehlt(app, mode, pfad.clone()),
            None => app.toast(jjcad_core::i18n::tr("gui.dialogs.datei_format_nicht_unterstuetzt")),
        }
    }
}

/// Hinweis, solange Dateien über dem Fenster schweben (T0115): er nennt die
/// Datei und was beim Loslassen geschieht — oder dass das Format nicht
/// importierbar ist. Ohne ihn bliebe der Zug bis zum Ablegen unsichtbar.
pub fn datei_hinweis(app: &App, ctx: &egui::Context) {
    let dateien = app.schwebende_dateien(ctx);
    let Some(erste) = dateien.first() else { return };
    let name = erste
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| erste.to_string_lossy().into_owned());
    let ziel = match FileMode::import_aus_datei(erste) {
        Some(mode) => mode.title().to_string(),
        None => jjcad_core::i18n::tr("gui.dialogs.format_nicht_unterstuetzt").to_string(),
    };
    egui::Area::new(egui::Id::new("datei_drop_hinweis"))
        .order(egui::Order::Tooltip)
        .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 120.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(Palette::panel())
                .stroke(Stroke::new(1.0, Palette::accent()))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.label(RichText::new(jjcad_core::i18n::tr("gui.dialogs.datei_loslassen")).strong());
                    ui.label(format!("{name} · {ziel}"));
                });
        });
}

/// Holt das Ergebnis des Systemdialogs ab. Der Dialog läuft in einem eigenen
/// Thread; ohne erneute Zeichnanforderung käme das Ergebnis erst beim nächsten
/// Mausklick an.
fn systemdialog_abholen(app: &mut App, ctx: &egui::Context) {
    let stand = match app.ui.systemdialog.as_mut() {
        Some((mode, dlg)) => Some((*mode, dlg.abfragen())),
        None => None,
    };
    match stand {
        Some((_, crate::systemdialog::Stand::Laeuft)) => {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        Some((mode, crate::systemdialog::Stand::Gewaehlt(path))) => {
            app.ui.systemdialog = None;
            datei_gewaehlt(app, mode, path);
        }
        Some((_, crate::systemdialog::Stand::Abgebrochen)) => {
            app.ui.systemdialog = None;
            // T0143: Abbruch heißt „nicht schließen“ — der Reiter bleibt offen.
            app.ui.schliessen_nach_speichern = None;
        }
        // Das System konnte keinen Dialog zeigen → eingebauten Dialog öffnen,
        // damit „Öffnen“ nie wirkungslos bleibt.
        Some((mode, crate::systemdialog::Stand::Fehlgeschlagen)) => {
            app.ui.systemdialog = None;
            app.open_own_file_dialog(mode);
        }
        None => {}
    }
}

/// Solange der Systemdialog offen ist, gehört die Anwendung ihm: die Zeichnung
/// wird abgedunkelt und ein Hinweis steht in der Mitte. Der Notausstieg (ESC)
/// steckt in `App::ui`.
pub fn systemdialog_schleier(app: &App, ctx: &egui::Context) {
    let Some((mode, _)) = app.ui.systemdialog.as_ref() else { return };
    // Volle Fläche über allem: dunkelt ab **und** nimmt jeden Mausklick an.
    // Nur Ereignisse zu verwerfen reicht nicht — Knöpfe und Zeichenfläche
    // lesen Klicks auch aus dem Zeigerzustand des Frames.
    egui::Area::new(egui::Id::new("systemdialog_schleier"))
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.screen_rect().min)
        .show(ctx, |ui| {
            let (rect, _antwort) = ui.allocate_exact_size(
                ctx.screen_rect().size(),
                Sense::click_and_drag(),
            );
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 110),
            );
        });
    egui::Area::new(egui::Id::new("systemdialog_hinweis"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            theme::dialog_frame().show(ui, |ui| {
                ui.set_max_width(420.0);
                ui.vertical_centered(|ui| {
                    ui.label(theme::bold(mode.title(), 15.0));
                    ui.add_space(2.0);
                    ui.label(theme::dim(
                        jjcad_core::i18n::tr("gui.dialogs.der_dateidialog_des_systems_ist_geoeffnet_datei_waehlen_"),
                    ));
                    ui.label(theme::faint(jjcad_core::i18n::tr("gui.dialogs.esc_schliesst_den_hinweis_wenn_das_system_nicht_antworte")));
                });
            });
        });
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    // Systemdialog des Betriebssystems (KDE/Dolphin, GNOME, Windows, macOS)
    systemdialog_abholen(app, ctx);

    // Dateidialog
    if app.ui.file_dialog.is_some() {
        dim_background(ctx);
        let mut res: Option<(FileMode, std::path::PathBuf)> = None;
        if let Some(fd) = app.ui.file_dialog.as_mut() {
            if let Some(path) = fd.show(ctx) {
                res = Some((fd.mode, path));
            }
        }
        if let Some((mode, path)) = res {
            app.ui.file_dialog = None;
            datei_gewaehlt(app, mode, path);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.file_dialog = None;
        }
    }
    // T0143: Der Merker „nach dem Speichern schließen“ gilt nur, solange ein
    // Dateidialog offen ist. Wird er abgebrochen, bleibt der Reiter stehen —
    // ohne dieses Aufräumen schlösse ihn das nächste Speichern.
    if app.ui.schliessen_nach_speichern.is_some()
        && app.ui.file_dialog.is_none()
        && app.ui.systemdialog.is_none()
    {
        app.ui.schliessen_nach_speichern = None;
    }

    // Neuer Block
    if app.ui.block_name_dialog.is_some() {
        dim_background(ctx);
        // Fenster-Flag: startet offen, false = über das X geschlossen.
        let mut win_open = true;
        let mut close = false;
        let mut btn_close = false;
        let mut create = false;
        if let Some((name, _)) = app.ui.block_name_dialog.as_mut() {
            modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.block_aus_auswahl"), 420.0, &mut win_open).show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.die_auswahl_wird_zur_blockdefinition_und_als_instanz_ein")));
                ui.add_space(4.0);
                let resp = ui.add_sized([380.0, 24.0], egui::TextEdit::singleline(name).hint_text(jjcad_core::i18n::tr("gui.dialogs.blockname")));
                resp.request_focus();
                ui.add_space(8.0);
                button_row(ui, |ui| {
                    if ui.button(jjcad_core::i18n::tr("gui.dialogs.anlegen")).clicked() {
                        create = true;
                    }
                    if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                        btn_close = true;
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
                    create = true;
                }
            });
            if btn_close {
                close = true;
            }
        }
        if !win_open {
            close = true; // X im Fensterrahmen
        }
        if create {
            if let Some((name, _)) = app.ui.block_name_dialog.clone() {
                if !name.trim().is_empty() {
                    let ids = app.selection().to_vec();
                    // T0128: „Block aus Auswahl“ schreibt die Blockdatei in die
                    // Benutzerbibliothek (`<Wurzel>/blocks/<slug>.jjblk`).
                    let dir = app.prefs.blocks_dir().to_string_lossy().to_string();
                    match app.session_mut().execute(
                        "block.create",
                        &json!({"name": name, "ids": ids, "dir": dir}),
                    ) {
                        Ok(_) => {
                            app.mark_changed();
                            app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.block_x_angelegt", &[("name", format!("{}", name))]));
                        }
                        Err(e) => app.toast(e),
                    }
                }
            }
            app.ui.block_name_dialog = None;
        }
        if close {
            app.ui.block_name_dialog = None;
        }
    }

    // Hotbar-Vorlage anlegen / umbenennen
    if let Some((idx, name)) = app.ui.preset_dialog.clone() {
        dim_background(ctx);
        // Fenster-Flag: startet offen, false = über das X geschlossen.
        let mut win_close = true;
        let mut btn_close = false;
        let mut ok = false;
        let mut buf = name.clone();
        modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.hotbar_vorlage"), 400.0, &mut win_close).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(theme::dim(if idx.is_some() {
                jjcad_core::i18n::tr("gui.dialogs.vorlage_umbenennen")
            } else {
                jjcad_core::i18n::tr("gui.dialogs.neue_vorlage_aus_der_aktuellen_anlegen")
            }));
            ui.add_space(4.0);
            let resp = ui.add_sized(
                [360.0, 24.0],
                egui::TextEdit::singleline(&mut buf).hint_text(jjcad_core::i18n::tr("gui.dialogs.name_der_vorlage")),
            );
            resp.request_focus();
            ui.add_space(8.0);
            button_row(ui, |ui| {
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.ok")).clicked() {
                    ok = true;
                }
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                    btn_close = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                ok = true;
            }
        });
        let close = btn_close || !win_close;
        if ok {
            let nm = if buf.trim().is_empty() {
                format!("Vorlage {}", app.prefs.hotbar_presets.len() + 1)
            } else {
                buf.trim().to_string()
            };
            match idx {
                Some(i) => {
                    if let Some(p) = app.prefs.hotbar_presets.get_mut(i) {
                        p.name = nm;
                    }
                }
                None => {
                    let mut p = jjcad_core::persist::HotbarPreset::new(&nm);
                    p.slots = app.prefs.slots();
                    app.prefs.hotbar_presets.push(p);
                    app.prefs.active_preset = app.prefs.hotbar_presets.len() - 1;
                }
            }
            let _ = app.prefs.save();
            app.ui.preset_dialog = None;
            app.toast(jjcad_core::i18n::tr("gui.dialogs.vorlage_gespeichert"));
        } else if close {
            app.ui.preset_dialog = None;
        }
    }

    // Einstellungen
    if app.ui.settings_open {
        dim_background(ctx);
        let mut open = true;
        // Aufbau wie in der KDE-Systemsteuerung (T0061): Suche und Kategorien
        // links, die Einstellungen der gewählten Kategorie rechts.
        // T0135: feste Größe. Vorher wuchs das Fenster in „Speicherorte“
        // Bild für Bild, weil die Pfadzeile ihre Breite aus
        // `ui.available_width()` rechnete und das Fenster mitwuchs. Die
        // eigentliche Ursache ist dort behoben (feste Breite der Seite,
        // Knopfzeile rechts nach links); `fixed_size` hält die Sollgröße fest,
        // und `tests_einstellungen::einstellungsfenster_gleich_gross_in_allen_kategorien`
        // prüft, dass keine Kategorie darüber hinauswächst.
        let fenster = modal_window(ctx, jjcad_core::i18n::tr("settings.title"), SETTINGS_BREITE, &mut open)
            .fixed_size([SETTINGS_BREITE, SETTINGS_HOEHE_GESAMT])
            .show(ctx, |ui| {
                ui.set_width(SETTINGS_BREITE_INNEN);
                settings_body(app, ui);
            });
        app.ui.last_settings_fenster = fenster.map(|f| f.response.rect);
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.settings_open = false;
            // T0127: der Pfadpuffer wird beim nächsten Öffnen neu gefüllt.
            app.ui.speicherorte_geladen = false;
        }
    }

    // Tastatur — bewusst **ohne** Abdunklung (T0064): das Ribbon bleibt
    // bedienbar, damit eine Funktion angeklickt und gleich auf eine Taste
    // gelegt werden kann.
    if app.ui.keymap_open {
        keymap_window(app, ctx);
    }
    keymap_wechsel_dialog(app, ctx);
    keymap_name_dialog(app, ctx);

    // Hilfe
    if app.ui.help_open {
        dim_background(ctx);
        let mut open = true;
        modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.hilfe"), 760.0, &mut open).show(ctx, |ui| {
            ui.set_width(740.0);
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                help_body(app, ui);
            });
        });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.help_open = false;
        }
    }

    // Projekt
    if app.ui.project_dialog {
        dim_background(ctx);
        let mut open = true;
        let mut p = app.doc().project.clone();
        modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.projektdaten"), 520.0, &mut open).show(ctx, |ui| {
            ui.set_width(500.0);
            ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.diese_angaben_fuellen_das_schriftfeld_und_dynamische_tex")));
            ui.add_space(6.0);
            let mut changed = false;
            for (label, field) in [
                (jjcad_core::i18n::tr("gui.dialogs.projekt"), &mut p.project),
                (jjcad_core::i18n::tr("gui.dialogs.bauherr"), &mut p.client),
                (jjcad_core::i18n::tr("gui.dialogs.planung_buero"), &mut p.planner),
                (jjcad_core::i18n::tr("gui.dialogs.adresse"), &mut p.address),
                ("Plan-Nr.", &mut p.plan_no),
                (jjcad_core::i18n::tr("gui.dialogs.planinhalt"), &mut p.plan_title),
                (jjcad_core::i18n::tr("gui.dialogs.revision"), &mut p.revision),
            ] {
                widgets::field_row(ui, label, |ui| {
                    if ui.add_sized([330.0, 22.0], egui::TextEdit::singleline(field)).changed() {
                        changed = true;
                    }
                });
            }
            if changed {
                let _ = app.session_mut().execute(
                    "doc.project",
                    &json!({
                        "project": p.project, "client": p.client, "planner": p.planner,
                        "address": p.address, "plan_no": p.plan_no,
                        "plan_title": p.plan_title, "revision": p.revision,
                    }),
                );
                app.mark_changed();
            }
        });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.project_dialog = false;
        }
    }

    // PDF einfügen: Seitenwahl (docs/PDF-IMPORT.md)
    if let Some(wahl) = app.ui.pdf_import.clone() {
        dim_background(ctx);
        // Fenster-Flag: startet offen, false = über das X geschlossen.
        let mut win_open = true;
        let mut seite = wahl.seite;
        let mut als_block = wahl.als_block;
        let mut einfuegen = false;
        let mut abbrechen = false;
        modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.pdf_einfuegen"), 470.0, &mut win_open).show(ctx, |ui| {
            ui.set_width(450.0);
            let name = wahl
                .pfad
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| wahl.pfad.display().to_string());
            ui.label(theme::bold(&name, 13.0));
            ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.dialogs.x_seitex_text_linien_und_flaechen_werden_uebernommen", &[("wert1", format!("{}", wahl.seiten)), ("wert2", format!("{}", if wahl.seiten == 1 { "" } else { "n" }))])));
            ui.add_space(8.0);
            widgets::field_row(ui, jjcad_core::i18n::tr("gui.dialogs.seite"), |ui| {
                if wahl.seiten > 1 {
                    let mut s = seite as i64;
                    if ui.add(crate::widgets::zahl(&mut s, 1.0, 1..=wahl.seiten as i64)).changed() {
                        seite = s.clamp(1, wahl.seiten as i64) as usize;
                    }
                    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.dialogs.von_x", &[("wert1", format!("{}", wahl.seiten))])));
                } else {
                    ui.label("1");
                }
            });
            ui.add_space(4.0);
            ui.checkbox(&mut als_block, jjcad_core::i18n::tr("gui.dialogs.als_ein_element_einfuegen_block_zerlegbar"));
            ui.add_space(6.0);
            ui.label(theme::dim(
                jjcad_core::i18n::tr("gui.dialogs.bilder_transparenz_schattierungen_muster_und_beschnitt_u"),
            ));
            ui.add_space(8.0);
            button_row(ui, |ui| {
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.einfuegen")).clicked() {
                    einfuegen = true;
                }
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                    abbrechen = true;
                }
            });
        });
        let mut wahl = wahl;
        wahl.seite = seite;
        wahl.als_block = als_block;
        if einfuegen {
            app.pdf_import_einfuegen(&wahl);
            app.ui.pdf_import = None;
        } else if !win_open || abbrechen {
            app.ui.pdf_import = None;
        } else {
            app.ui.pdf_import = Some(wahl);
        }
    }

    // Datei-Reiter schließen (T0073): ungespeicherte Änderungen fragen nach.
    if let Some(idx) = app.ui.confirm_close {
        dim_background(ctx);
        let mut close = false;
        // Fenster-Flag: startet offen, false = über das X geschlossen.
        let mut win_close = true;
        let mut verwerfen = false;
        let mut speichern = false;
        let name = app
            .tabs
            .get(idx)
            .map(|t| t.session.doc.name.clone())
            .unwrap_or_default();
        let hat_speicherort = app
            .tabs
            .get(idx)
            .map(|t| t.session.doc.path.is_some())
            .unwrap_or(false);
        modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.datei_schliessen"), 470.0, &mut win_close).show(ctx, |ui| {
            ui.set_width(450.0);
            ui.label(jjcad_core::i18n::t_args_owned("gui.dialogs.x_ist_nicht_gespeichert", &[("name", format!("{}", name))]));
            if hat_speicherort {
                ui.label(theme::dim(
                    jjcad_core::i18n::tr("gui.dialogs.speichern_und_schliessen_legt_den_reiter_ab_verwerfen_ve"),
                ));
            } else {
                ui.label(theme::dim(
                    jjcad_core::i18n::tr("gui.dialogs.speichern_fragt_nach_einem_speicherort_und_legt_den_reiter_dana"),
                ));
            }
            ui.add_space(8.0);
            button_row(ui, |ui| {
                // T0143: „Speichern“ steht immer — ohne Speicherort fragt der
                // Dateidialog nach dem Ziel (`App::close_tab_speichern_mit_ziel`).
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.speichern_und_schliessen")).clicked() {
                    speichern = true;
                }
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.verwerfen")).clicked() {
                    verwerfen = true;
                }
                if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                    close = true;
                }
            });
        });
        if !win_close {
            close = true; // X im Fensterrahmen
        }
        if speichern {
            app.ui.confirm_close = None;
            app.close_tab_speichern_mit_ziel(idx);
        } else if verwerfen {
            // T0129: verworfene Änderungen dürfen beim nächsten Start nicht
            // zurückkehren — der neueste Stand wird der Dateiinhalt.
            app.verwerfen_stand(idx);
            app.close_tab(idx);
            app.ui.confirm_close = None;
        } else if close {
            app.ui.confirm_close = None;
        }
    }

    // Info
    if app.ui.about_open {
        dim_background(ctx);
        let mut open = true;
        modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.ueber_joja_cad"), 480.0, &mut open).show(ctx, |ui| {
            ui.set_width(460.0);
            // Kopf mit Markenzeichen (T0083) — dasselbe Bild wie das Fenstericon.
            ui.horizontal(|ui| {
                let marke = crate::icon::textur(ui.ctx());
                let kante = egui::vec2(64.0, 64.0);
                ui.add(egui::Image::new(egui::load::SizedTexture::new(marke.id(), kante)));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(theme::bold(jjcad_core::i18n::tr("gui.dialogs.joja_cad"), 20.0));
                    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.dialogs.version_x", &[("wert1", format!("{}", env!("CARGO_PKG_VERSION")))])));
                    ui.label(theme::mono(crate::app::build_label()));
                });
            });
            ui.add_space(8.0);
            ui.label(jjcad_core::i18n::tr("gui.dialogs.2d_cad_fuer_architektur_und_bauantragszeichnungen"));
            ui.add_space(6.0);
            ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.rust_egui_eigener_pdf_und_dxf_export_mcp_schnittstelle_f")));
            ui.add_space(6.0);
            ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.lizenz_mit_format_jjcad_json_offen_dokumentiert")));
            ui.add_space(8.0);
            ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.dialogs.dokument_x_x", &[("wert1", format!("{}", app.doc().name)), ("wert2", format!("{}", app.doc().summary()))])));
            if let Some(p) = &app.doc().path {
                ui.label(theme::mono(p.clone()));
            }
            ui.add_space(8.0);
        });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.about_open = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Einstellungen
// ---------------------------------------------------------------------------

/// Breite der Kategorienspalte im Einstellungsfenster (Vorbild: KDE, T0061).
const SETTINGS_NAV_W: f32 = 220.0;
/// Höhe einer Zeile der Kategorienliste.
const SETTINGS_ZEILE_H: f32 = 30.0;
/// Höhe des Einstellungsfensters unter dem Fenstertitel.
const SETTINGS_HOEHE: f32 = 580.0;
/// Breite des Einstellungsfensters samt Rahmen (T0135). Sie ist in **allen**
/// Kategorien dieselbe — ein selbstgroßes Fenster wuchs in „Speicherorte“, weil
/// die Pfadzeile ihre Breite aus dem verfügbaren Platz rechnete.
const SETTINGS_BREITE: f32 = 960.0;
/// Innenbreite des Fensters (Rahmen links und rechts je 10 px).
const SETTINGS_BREITE_INNEN: f32 = SETTINGS_BREITE - 20.0;
/// Höhe des Fensters samt Titelzeile und Rahmen.
const SETTINGS_HOEHE_GESAMT: f32 = 660.0;
/// Höchstbreite der Einstellungsseite rechts neben der Kategorienspalte.
/// Sie wird **nicht** aus dem verfügbaren Platz genommen: sonst wächst das
/// Fenster mit jeder Zeile, die sich nach der verfügbaren Breite richtet (T0135).
const SETTINGS_SEITE_W: f32 = SETTINGS_BREITE_INNEN - SETTINGS_NAV_W - 32.0;

/// Eine Kategorie der Einstellungen: Name, Icon und die Namen der
/// Einstellungen darin.
///
/// Die Stichworte sind die **eine** Quelle der Suche: wer hier eine Einstellung
/// ergänzt, trägt ihren Namen in der Liste der Kategorie ein — sonst findet die
/// Suche die Kategorie nicht und die Einstellung bleibt versteckt (T0061).
pub(crate) struct Einstellungskategorie {
    /// Katalogschlüssel des Namens (T0103) — der Text folgt der Sprache.
    pub key: &'static str,
    /// Iconname aus `icons::paint`
    icon: &'static str,
    stichworte: &'static [&'static str],
}

impl Einstellungskategorie {
    /// Name der Kategorie in der eingestellten Sprache.
    pub fn name(&self) -> &'static str {
        jjcad_core::i18n::tr(self.key)
    }

    /// Passt die Kategorie zum (klein geschriebenen) Suchbegriff? Gesucht wird
    /// im Kategorienamen und in den Namen aller Einstellungen der Kategorie.
    fn passt(&self, suche: &str) -> bool {
        if suche.is_empty() {
            return true;
        }
        self.name().to_lowercase().contains(suche)
            || self
                .stichworte
                .iter()
                .any(|w| w.to_lowercase().contains(suche))
    }
}

/// Die Kategorien des Einstellungsfensters in der Reihenfolge der Liste links.
pub(crate) const EINSTELLUNGSKATEGORIEN: &[Einstellungskategorie] = &[
    Einstellungskategorie {
        key: "settings.category_einheiten",
        icon: "staerke",
        stichworte: &["Eingabe", "Einheit", "Nachkommastellen", "Maßstab"],
    },
    Einstellungskategorie {
        key: "settings.category_raster",
        icon: "grid",
        stichworte: &[
            "Rasterweite",
            "Raster anzeigen",
            "Rasterfang",
            "Raster",
            "Rasterart",
            "Isometrisch",
            "Dimetrie",
            "Trimetrie",
            "Fest",
            "erzwingen",
        ],
    },
    Einstellungskategorie {
        key: "settings.category_fangen",
        icon: "snap",
        stichworte: &[
            "Endpunkt",
            "Wandkanten",
            "Mittelpunkt",
            "Zentrum",
            "Schnittpunkt",
            "Lot",
            "Nächster Punkt",
            "Ortho",
            "Polar",
            "Polarwinkel",
            "Polarfenster",
            "Fangradius",
            "Fangen",
        ],
    },
    Einstellungskategorie {
        key: "settings.category_architektur",
        icon: "wall",
        stichworte: &[
            "Wandstärke",
            "Türbreite",
            "Fensterbreite",
            "Treppenbreite",
            "Texthöhe",
            "Wand",
            "Tür",
            "Fenster",
            "Treppe",
        ],
    },
    Einstellungskategorie {
        key: "settings.category_bemassung",
        icon: "dim",
        stichworte: &[
            "Pfeilgröße",
            "Nachkommastellen",
            "Maßeinheit",
            "Textgröße",
            "Bogensymbole",
            "Maßstab",
            "Bemaßung",
        ],
    },
    Einstellungskategorie {
        key: "settings.category_bedienung",
        icon: "move",
        stichworte: &[
            "Ziehen",
            "Maus",
            "Maus verschiebt",
            "Verschieben",
            "Schwenk",
            "Bewegungsgeschwindigkeit",
            "Geschwindigkeit",
            "WASD",
            "W A S D",
            "Ansicht",
            "Ansicht verschieben",
            "Bewegungsart",
            "kontinuierlich",
            "sprunghaft",
            "Drehpunkt",
            "Einfügen",
            "Vorschau",
            "Rotieren",
            "Drehen",
            "Mittelpunkt",
            "Basispunkt",
            "Fadenkreuz",
            "Zwischenablage",
            "Dateidialog",
            "Fensterrahmen",
            "Titelleiste",
            "Minimieren",
            "Vergrößern",
            "Schließen",
            "Rahmen",
            "Fenster",
            "Sprache",
            "Englisch",
            "Language",
            "Bedienung",
        ],
    },
    Einstellungskategorie {
        key: "settings.category_darstellung",
        icon: "eye",
        stichworte: &[
            "Linienstärken",
            "Linienstärke",
            "Papier",
            "maßstabsgetreu",
            "Bildschirm",
            "konstant",
            "Faktor",
            "Skalierung",
            "Layer-Panel",
            "Eigenschaften-Panel",
            "Fangmarker",
            "Lineale",
            "Design",
            "Farbschema",
            "Erscheinungsbild",
            "Aussehen",
            "Hell",
            "Dunkel",
            "Nachtmodus",
            "System",
            "Betriebssystem",
            "Farbakzent",
            "Farbakzente",
            "Akzent",
            "Akzentfarbe",
            "Bernstein",
            "Orange",
            "Saphir",
            "Smaragd",
            "Rubin",
            "Amethyst",
            "Blau",
            "Grün",
            "Rot",
            "Violett",
            "Textauflösung",
            "Auflösung",
            "Schärfe",
            "Rastergröße",
            "Schrift",
            "Atlas",
            "Darstellung",
        ],
    },
    // T0125: Voreinstellungen der Linienstärke je Objektart.
    Einstellungskategorie {
        key: "settings.category_voreinstellungen",
        icon: "staerke",
        stichworte: &[
            "Linienstärke",
            "Linienbreite",
            "Linienstärken",
            "Voreinstellungen",
            "Objektart",
            "Wände",
            "Türen",
            "Fenster",
            "Treppen",
            "Bemaßung",
            "Text",
            "Schraffur",
            "Möbel",
            "Achsen",
            "Hilfslinien",
            "zurücksetzen",
            "0,18",
        ],
    },
    // T0127: eine Wurzel für alle Nutzdaten; die drei Unterordner sind
    // änderbar.
    Einstellungskategorie {
        key: "settings.category_speicherorte",
        icon: "datei",
        stichworte: &[
            "Speicherorte",
            "Datenordner",
            "Datenwurzel",
            "Ordner",
            "Blockbibliothek",
            "Blocks",
            "Sicherungen",
            "Protokolle",
            "Log",
            "Pfad",
            "portabel",
            "portable",
            "daten-dir",
            "JJCAD_DATA_DIR",
        ],
    },
    // T0129: Sicherung — Stände unabhängig vom Speichern.
    Einstellungskategorie {
        key: "settings.category_sicherung",
        icon: "save",
        stichworte: &[
            "Sicherung",
            "Sicherungen",
            "Zwischenstände",
            "Verlauf",
            "Anzahl",
            "Plätze",
            "Stände",
            "Wiederherstellen",
            "Pool",
            "Intervall",
            "Aufbewahrungszeitraum",
            "Reicht zurück",
            "Abstand",
            "Netzlaufwerk",
            "Autosave",
            "Sicherungsordner",
        ],
    },
];

/// Zahl mit deutschem Dezimalkomma (T0129) — die Oberfläche schreibt deutsch.
fn zahl_deutsch(wert: f64, stellen: usize) -> String {
    format!("{wert:.stellen$}").replace('.', ",")
}

/// Dateigröße für die Anzeige (T0129): `12,3 kB`, `1,2 MB`.
fn groesse_text(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1_000_000.0 {
        format!("{} MB", zahl_deutsch(b / 1_000_000.0, 1))
    } else if b >= 1_000.0 {
        format!("{} kB", zahl_deutsch(b / 1_000.0, 1))
    } else {
        format!("{bytes} B")
    }
}

/// Name einer Wahl für „Reicht zurück“ (Arbeitsstunden, T0129).
fn stunden_name(stunden: u32) -> String {
    match stunden {
        40 => jjcad_core::i18n::tr("settings.stunden_woche").to_string(),
        160 => jjcad_core::i18n::tr("settings.stunden_monat").to_string(),
        480 => jjcad_core::i18n::tr("settings.stunden_quartal").to_string(),
        1900 => jjcad_core::i18n::tr("settings.stunden_jahr").to_string(),
        _ => format!("{stunden} h"),
    }
}

/// Eine Zeile im Einstellungsfenster — während einer Suche nur, wenn ihr Name
/// den Suchbegriff enthält.
fn zeile(suche: &str, ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    zeile_mit(suche, ui, label, &[], add);
}

/// Wie `zeile`, aber mit weiteren Suchbegriffen: ein kurzer Zeilenname (die
/// Labelspalte ist 96 px breit) bleibt trotzdem über den vollen Begriff
/// auffindbar — „Dezimalstellen“ über „Nachkommastellen“.
fn zeile_mit(
    suche: &str,
    ui: &mut Ui,
    label: &str,
    aliase: &[&str],
    add: impl FnOnce(&mut Ui),
) {
    let passt = suche.is_empty()
        || label.to_lowercase().contains(suche)
        || aliase.iter().any(|a| a.to_lowercase().contains(suche));
    if passt {
        widgets::field_row(ui, label, add);
    }
}

/// Wie `zeile`, aber der Name trägt einen Hinweis (Tooltip) — für Einstellungen,
/// deren Name allein nicht verrät, was sie tut.
fn zeile_tip(
    suche: &str,
    ui: &mut Ui,
    label: &str,
    aliase: &[&str],
    tip: &str,
    add: impl FnOnce(&mut Ui),
) {
    let passt = suche.is_empty()
        || label.to_lowercase().contains(suche)
        || aliase.iter().any(|a| a.to_lowercase().contains(suche));
    if passt {
        widgets::field_row_tip(ui, label, tip, add);
    }
}

/// Abschnittsüberschrift; während einer Suche entfällt sie, damit nur die
/// Treffer übrig bleiben.
fn abschnitt(suche: &str, ui: &mut Ui, text: &str) {
    if suche.is_empty() {
        theme::section(ui, text);
    }
}

/// Eine Zeile der Kategorienliste links: Icon, Name, Auswahlfarbe.
/// Liefert `true`, wenn die Zeile angeklickt wurde.
fn kategorie_zeile(ui: &mut Ui, kat: &Einstellungskategorie, gewaehlt: bool) -> bool {
    let (rect, antwort) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), SETTINGS_ZEILE_H), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if gewaehlt {
            p.rect_filled(rect, Rounding::same(6.0), Palette::select().gamma_multiply(0.85));
        } else if antwort.hovered() {
            p.rect_filled(rect, Rounding::same(6.0), Palette::panel_alt());
        }
        let farbe = if gewaehlt {
            Color32::WHITE
        } else if antwort.hovered() {
            Palette::text()
        } else {
            Palette::text_dim()
        };
        let icon = Rect::from_center_size(
            egui::pos2(rect.left() + 18.0, rect.center().y),
            Vec2::splat(16.0),
        );
        crate::icons::paint(p, icon, kat.icon, farbe);
        p.text(
            egui::pos2(rect.left() + 34.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            kat.name(),
            FontId::proportional(13.0),
            farbe,
        );
    }
    antwort.clicked()
}

/// Beschriftung einer Rasterart (T0144).
pub(crate) fn rasterart_text(art: jjcad_core::gitter::Rasterart) -> &'static str {
    use jjcad_core::gitter::Rasterart;
    match art {
        Rasterart::Rechteckig => jjcad_core::i18n::tr("gui.dialogs.rasterart_rechteckig"),
        Rasterart::Isometrisch => jjcad_core::i18n::tr("gui.dialogs.rasterart_isometrisch"),
        Rasterart::Dimetrie => jjcad_core::i18n::tr("gui.dialogs.rasterart_dimetrie"),
        Rasterart::Trimetrie => jjcad_core::i18n::tr("gui.dialogs.rasterart_trimetrie"),
    }
}

/// Beschriftung des Rastermodus (T0144).
pub(crate) fn rastermodus_text(m: jjcad_core::gitter::Rastermodus) -> &'static str {
    use jjcad_core::gitter::Rastermodus;
    match m {
        Rastermodus::Skaliert => jjcad_core::i18n::tr("gui.dialogs.rastermodus_skaliert"),
        Rastermodus::Fest => jjcad_core::i18n::tr("gui.dialogs.rastermodus_fest"),
    }
}

fn settings_body(app: &mut App, ui: &mut Ui) {
    let unit = app.doc().settings.input_unit;

    // Ausgangswerte einlesen
    let cur = app.doc().settings.clone();
    let mut input = cur.input_unit;
    let mut decimals = cur.decimals as i32;
    let mut show_grid = app.ui.show_grid;
    let mut snap = cur.snap.clone();
    let mut grid_step = snap.grid_step;
    // T0144: Art, Modus und erzwungener Rasterfang.
    let mut rasterart = snap.grid_kind;
    let mut rastermodus = snap.grid_mode;
    let mut grid_force = snap.grid_force;
    let mut polar_step = snap.polar_step;
    let mut polar_tol = snap.polar_tolerance;
    let mut tol_px = snap.tolerance_px;
    let mut th = cur.text_height;
    let mut wall = cur.wall_thickness;
    let mut door = cur.door_width;
    let mut win = cur.window_width;
    let mut stair = cur.stair_width;
    let mut rulers = cur.show_rulers;
    let mut ds = app.doc().dim_style.clone();
    let mut ds_changed = false;
    let mut lw_scaled = app.ui.lineweights_scaled;
    // T0139: Faktor der Linienstärke im Modus „Bildschirm (konstant)“.
    let mut lw_factor = app.ui.lineweight_factor;
    let mut drag_mode = app.ui.drag_mode;
    // T0145: eigener Zieh-Modus für das Auswahlfenster.
    let mut select_mode = app.ui.select_mode;
    let mut show_layers = app.ui.layers_open;
    let mut show_props = app.ui.props_open;
    let mut marker = app.prefs.snap_marker;
    let mut mouse_move = app.prefs.mouse_move;
    let mut pan_speed = app.prefs.pan_speed;
    // T0101: Bewegungsart der Ansicht — Dauerlauf oder Einzelschritt.
    let mut pan_mode = app.prefs.pan_mode;
    let mut dateidialog_system = app.prefs.dateidialog_system();
    // T0070: Drehpunkt der Einfüge-Vorschau (Basispunkt oder Mittelpunkt).
    let mut paste_pivot = app.prefs.paste_pivot;
    // T0098: Fensterrahmen — experimentell auch ohne Titelleiste.
    let mut fensterrahmen = app.prefs.fensterrahmen;
    // T0102: Farbschema der Oberfläche (hell, dunkel oder wie das System).
    let mut design = app.prefs.design;
    // T0111: Farbakzent der Bedienung (Vorgabe Bernstein).
    let mut farbakzent = app.prefs.farbakzent;
    // T0114: höchste Rastergröße der Schrift (Vorgabe 256 px).
    let mut textaufloesung = app.prefs.textaufloesung;
    // T0129: Sicherung — Verlauf, Plätze, Reichweite, Intervall.
    let mut s_verlauf = app.prefs.sicherung_verlauf;
    let mut s_plaetze = app.prefs.sicherung_plaetze as i32;
    let mut s_stunden = app.prefs.sicherung_stunden;
    let mut s_intervall = app.prefs.sicherung_intervall as i32;

    // Suche: Kategorien, deren Name oder eine ihrer Einstellungen passt.
    let suche = app.ui.settings_suche.trim().to_lowercase();
    let treffer: Vec<usize> = EINSTELLUNGSKATEGORIEN
        .iter()
        .enumerate()
        .filter(|(_, k)| k.passt(&suche))
        .map(|(i, _)| i)
        .collect();
    let mut kat = app.ui.settings_kat.min(EINSTELLUNGSKATEGORIEN.len() - 1);
    if !treffer.contains(&kat) {
        if let Some(&erste) = treffer.first() {
            kat = erste;
        }
    }

    ui.horizontal_top(|ui| {
        // ------------------------ Suche und Kategorien ------------------------
        ui.vertical(|ui| {
            ui.set_width(SETTINGS_NAV_W);
            ui.spacing_mut().item_spacing.y = 0.0;
            let antwort = ui.add_sized(
                [SETTINGS_NAV_W, 26.0],
                egui::TextEdit::singleline(&mut app.ui.settings_suche)
                    .hint_text(jjcad_core::i18n::tr("settings.search")),
            );
            app.ui.last_settings_suche = Some(antwort.rect);
            ui.add_space(8.0);

            let (nav_rect, _) = ui.allocate_exact_size(
                Vec2::new(SETTINGS_NAV_W, SETTINGS_HOEHE - 34.0),
                Sense::hover(),
            );
            app.ui.last_settings_nav = Some(nav_rect);
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(nav_rect)
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    let mut gewaehlt = None;
                egui::ScrollArea::vertical()
                    .id_salt("einstellungen_kategorien")
                    .max_height(SETTINGS_HOEHE - 40.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for &i in &treffer {
                            if kategorie_zeile(ui, &EINSTELLUNGSKATEGORIEN[i], i == kat) {
                                gewaehlt = Some(i);
                            }
                        }
                        if treffer.is_empty() {
                            ui.add_space(6.0);
                            ui.label(theme::dim(jjcad_core::i18n::tr("settings.none_found")));
                        }
                    });
                    if let Some(i) = gewaehlt {
                        kat = i;
                    }
                },
            );
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        // --------------------------- Einstellungen ---------------------------
        // T0135: feste Breite statt `available_width()` — sonst rechnen die
        // Pfadzeilen der Kategorie „Speicherorte“ ihre Breite aus dem Fenster
        // und treiben es Bild für Bild weiter auf.
        let (pane_rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(SETTINGS_SEITE_W), SETTINGS_HOEHE),
            Sense::hover(),
        );
        app.ui.last_settings_pane = Some(pane_rect);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(pane_rect)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.label(theme::bold(EINSTELLUNGSKATEGORIEN[kat].name(), 18.0));
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .id_salt("einstellungen_seite")
                    .max_height(SETTINGS_HOEHE - 40.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if treffer.is_empty() {
                            ui.label(theme::dim(
                                jjcad_core::i18n::tr("gui.dialogs.keine_einstellung_gefunden_suchbegriff_loeschen_um_alle_"),
                            ));
                            return;
                        }
                        match kat {
                            // ------------------------ Einheiten -------------------
                            0 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.einheiten"));
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.eingabe"), |ui| {
                                    for u in Unit::ALL {
                                        ui.selectable_value(&mut input, u, u.short());
                                    }
                                });
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.dezimalstellen"),
                                    &["Nachkommastellen"],
                                    |ui| {
                                        ui.add(crate::widgets::zahl(&mut decimals, 1.0, 0..=4));
                                    },
                                );
                                if suche.is_empty() {
                                    ui.add(
                                        egui::Label::new(theme::faint(
                                            jjcad_core::i18n::tr("gui.dialogs.masseinheit_massstab_textgroesse_und_bogensymbole_stehen"),
                                        ))
                                        .wrap(),
                                    );
                                }
                            }
                            // -------------------------- Raster --------------------
                            1 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.raster"));
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.rasterweite"), |ui| {
                                    // Dieselben Grenzen wie im Eigenschaften-Panel (1 mm … 10 m).
                                    let mut g = grid_step;
                                    if widgets::length_field(
                                        ui,
                                        &mut g,
                                        unit,
                                        90.0,
                                        jjcad_core::model::GRID_STEP_MIN_MM,
                                        jjcad_core::model::GRID_STEP_MAX_MM,
                                    ) {
                                        grid_step = g;
                                    }
                                });
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.raster_anzeigen"), |ui| {
                                    if ui.checkbox(&mut show_grid, "").changed() {
                                        snap.grid = show_grid;
                                    }
                                });
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.rasterfang"), |ui| {
                                    ui.checkbox(&mut snap.grid, "");
                                });
                                // T0144: Art des Rasters — dieselben Achsen gelten
                                // für Anzeige und Fang.
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.rasterart"), |ui| {
                                    let cap = rasterart_text(rasterart);
                                    if let Some(neu) = panels::dropdown(
                                        ui,
                                        "rasterart",
                                        cap,
                                        None,
                                        |ui| {
                                            let mut out = None;
                                            for art in jjcad_core::gitter::Rasterart::ALL {
                                                if ui
                                                    .selectable_label(
                                                        art == rasterart,
                                                        rasterart_text(art),
                                                    )
                                                    .clicked()
                                                {
                                                    out = Some(art);
                                                }
                                            }
                                            out
                                        },
                                    ) {
                                        rasterart = neu;
                                    }
                                });
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr(
                                        "gui.dialogs.rasterart_hinweis",
                                    )));
                                }
                                // T0144: skaliert die gezeichnete Weite mit dem
                                // Zoom oder bleibt sie fest?
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.rastermodus"),
                                    &["Rasterweite folgt dem Zoom", "Modus", "Raster"],
                                    |ui| {
                                    let cap = rastermodus_text(rastermodus);
                                    if let Some(neu) = panels::dropdown(
                                        ui,
                                        "rastermodus",
                                        cap,
                                        None,
                                        |ui| {
                                            let mut out = None;
                                            for m in jjcad_core::gitter::Rastermodus::ALL {
                                                if ui
                                                    .selectable_label(m == rastermodus, rastermodus_text(m))
                                                    .clicked()
                                                {
                                                    out = Some(m);
                                                }
                                            }
                                            out
                                        },
                                    ) {
                                        rastermodus = neu;
                                    }
                                });
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr(
                                        "gui.dialogs.rastermodus_hinweis",
                                    )));
                                }
                                // T0144: jede Eingabe liegt im Raster.
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.raster_erzwingen"),
                                    &["Rasterfang erzwingen", "Raster", "Zwang"],
                                    |ui| {
                                        ui.checkbox(&mut grid_force, "");
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr(
                                        "gui.dialogs.raster_erzwingen_hinweis",
                                    )));
                                }
                            }
                            // -------------------------- Fangen --------------------
                            2 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.fangen"));
                                for (label, field) in [
                                    (jjcad_core::i18n::tr("gui.dialogs.endpunkt"), &mut snap.endpoint),
                                    (jjcad_core::i18n::tr("gui.dialogs.wandkanten"), &mut snap.wall_edges),
                                    (jjcad_core::i18n::tr("gui.dialogs.mittelpunkt"), &mut snap.midpoint),
                                    (jjcad_core::i18n::tr("gui.dialogs.zentrum"), &mut snap.center),
                                    (jjcad_core::i18n::tr("gui.dialogs.schnittpunkt"), &mut snap.intersection),
                                    (jjcad_core::i18n::tr("gui.dialogs.lot"), &mut snap.perpendicular),
                                    (jjcad_core::i18n::tr("gui.dialogs.naechster_punkt"), &mut snap.nearest),
                                    (jjcad_core::i18n::tr("gui.dialogs.ortho"), &mut snap.ortho),
                                    (jjcad_core::i18n::tr("gui.dialogs.polar"), &mut snap.polar),
                                ] {
                                    if !suche.is_empty() && !label.to_lowercase().contains(&suche) {
                                        continue;
                                    }
                                    widgets::field_row(ui, label, |ui| {
                                        ui.checkbox(field, "");
                                    });
                                }
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.polarwinkel"), |ui| {
                                    ui.add(
                                        crate::widgets::zahl(&mut polar_step, 1.0, 1.0..=180.0)
                                            .suffix("°"),
                                    );
                                    ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.architektur_45")));
                                });
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.polarfenster"), |ui| {
                                    ui.add(
                                        crate::widgets::zahl(&mut polar_tol, 0.5, 0.0..=45.0)
                                            .suffix("°"),
                                    );
                                    ui.label(theme::dim(
                                        jjcad_core::i18n::tr("gui.dialogs.nur_innerhalb_dieser_abweichung_rastet_polar_ein"),
                                    ));
                                });
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.fangradius"), |ui| {
                                    ui.add(
                                        crate::widgets::zahl(&mut tol_px, 1.0, 1.0..=60.0)
                                            .suffix(" px"),
                                    );
                                });
                            }
                            // ------------------------ Architektur -----------------
                            3 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.architektur"));
                                for (label, val, def) in [
                                    (jjcad_core::i18n::tr("gui.dialogs.wandstaerke"), &mut wall, 240.0),
                                    (jjcad_core::i18n::tr("gui.dialogs.tuerbreite"), &mut door, 885.0),
                                    (jjcad_core::i18n::tr("gui.dialogs.fensterbreite"), &mut win, 1200.0),
                                    (jjcad_core::i18n::tr("gui.dialogs.treppenbreite"), &mut stair, 1000.0),
                                    (jjcad_core::i18n::tr("gui.dialogs.texthoehe"), &mut th, 250.0),
                                ] {
                                    if !suche.is_empty() && !label.to_lowercase().contains(&suche) {
                                        continue;
                                    }
                                    widgets::field_row(ui, label, |ui| {
                                        let mut v = *val;
                                        if widgets::length_field(
                                            ui,
                                            &mut v,
                                            unit,
                                            90.0,
                                            widgets::MASS_MIN_MM,
                                            widgets::MASS_MAX_MM,
                                        ) {
                                            *val = v;
                                        }
                                        if ui
                                            .add(egui::Button::new(jjcad_core::i18n::tr("gui.props.standard")).small())
                                            .on_hover_text(jjcad_core::i18n::t_args_owned("gui.dialogs.x_mm", &[("wert1", format!("{:.0}", def))]))
                                            .clicked()
                                        {
                                            *val = def;
                                        }
                                    });
                                }
                            }
                            // -------------------------- Bemaßung ------------------
                            4 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.bemassung"));
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.pfeilgroesse"), |ui| {
                                    if ui
                                        .add(
                                            crate::widgets::zahl(
                                                &mut ds.arrow_size,
                                                0.5,
                                                0.1..=1000.0,
                                            )
                                            .suffix(" mm"),
                                        )
                                        .changed()
                                    {
                                        ds_changed = true;
                                    }
                                });
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.dezimalstellen"),
                                    &["Nachkommastellen"],
                                    |ui| {
                                        let mut d = ds.decimals as i32;
                                        if ui.add(crate::widgets::zahl(&mut d, 1.0, 0..=4)).changed()
                                        {
                                            ds.decimals = d.max(0) as usize;
                                            ds_changed = true;
                                        }
                                    },
                                );
                                // Maßeinheit, Maßstab, Textgröße und Bogensymbole: dieselbe
                                // Funktion wie im Eigenschaften-Panel — ein Wert, eine Stelle.
                                // Sie zeichnet mehrere Zeilen; während einer Suche bleibt der
                                // Block sichtbar, sobald eine seiner Zeilen passt.
                                let block_passt = suche.is_empty()
                                    || [
                                        "maßeinheit",
                                        "maßstab",
                                        "textgröße",
                                        "bogensymbole",
                                        "textgrösse",
                                    ]
                                    .iter()
                                    .any(|w| w.contains(&suche));
                                if block_passt {
                                    crate::panels::dim_fields(app, ui);
                                }
                            }
                            // ------------------------- Bedienung ------------------
                            5 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.bedienung"));
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.ziehen"), |ui| {
                                    for m in DragMode::ALL {
                                        ui.selectable_value(&mut drag_mode, m, m.text())
                                            .on_hover_text(m.tip());
                                    }
                                });
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr("gui.dialogs.gilt_fuer_griffe_und_objekte")));
                                }
                                // T0145: Das Auswahlfenster (Rechteck auf der freien
                                // Fläche) hat einen eigenen Schalter — vorher galt
                                // „Zwei Klicks“ nur für Griffe und Objekte.
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.auswahlfenster"), |ui| {
                                    for m in DragMode::ALL {
                                        ui.selectable_value(&mut select_mode, m, m.text())
                                            .on_hover_text(m.tip());
                                    }
                                });
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr("gui.dialogs.gilt_fuer_das_auswahlfenster")));
                                }
                                // T0063: Objekte mit der gedrückten Maustaste
                                // verschieben — für alle Objektarten gemeinsam.
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.maus_verschiebt"), |ui| {
                                    ui.checkbox(&mut mouse_move, "");
                                });
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("gui.dialogs.aus_objekte_wandern_nur_ueber_das_werkzeug_verschieben"),
                                    ));
                                }
                                // T0101: Bewegungsart der Ansicht. „Kontinuierlich“
                                // hält die Ansicht in Bewegung, solange die Funktion
                                // gedrückt bleibt; „Sprunghaft“ rückt je Druck um einen
                                // Schritt (bisheriges Verhalten).
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.ansicht_verschieben"),
                                    &[
                                        "Verschieben",
                                        "Ansicht verschieben",
                                        "Bewegung",
                                        "Bewegungsart",
                                        "kontinuierlich",
                                        "sprunghaft",
                                        "Schwenk",
                                        "WASD",
                                    ],
                                    |ui| {
                                        for m in jjcad_core::persist::PanMode::ALL {
                                            ui.selectable_value(&mut pan_mode, m, m.text())
                                                .on_hover_text(m.tip());
                                        }
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("gui.dialogs.gilt_fuer_die_schwenkfunktionen"),
                                    ));
                                }
                                // T0067: Tempo der Schwenk-Funktionen (`view.pan_*`,
                                // Vorgabe W/A/S/D) als Faktor auf das Grundtempo.
                                // T0146: Die Zeile heißt „Geschwindigkeit der
                                // Ansicht“; „Schwenk“ bleibt als Suchbegriff.
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.geschwindigkeit_der_ansicht"),
                                    &[
                                        "Schwenk",
                                        "Bewegungsgeschwindigkeit",
                                        "Geschwindigkeit",
                                        "WASD",
                                        "W A S D",
                                        "Tempo",
                                        "Ansicht",
                                    ],
                                    |ui| {
                                        let antwort = ui.add_sized(
                                            [widgets::VAL_W, 20.0],
                                            widgets::zahl(
                                                &mut pan_speed,
                                                0.05,
                                                jjcad_core::persist::SCHWENK_FAKTOR_MIN
                                                    ..=jjcad_core::persist::SCHWENK_FAKTOR_MAX,
                                            )
                                            .max_decimals(2)
                                            .suffix(" ×"),
                                        );
                                        antwort.on_hover_text(
                                            jjcad_core::i18n::tr("gui.dialogs.faktor_fuer_den_schwenk_mit_w_a_s_d_1_vorgabe"),
                                        );
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("gui.dialogs.gilt_fuer_den_schwenk_der_ansicht_w_a_s_d"),
                                    ));
                                }
                                // T0070: Drehpunkt beim Einfügen aus der Zwischenablage.
                                // „Basispunkt" ist das bisherige Verhalten, „Mittelpunkt"
                                // lässt die Vorschau oben rechts am Fadenkreuz stehen.
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.drehpunkt"),
                                    &[
                                        "Einfügen",
                                        "Vorschau",
                                        "Rotieren",
                                        "Drehen",
                                        "Mittelpunkt",
                                        "Basispunkt",
                                        "Fadenkreuz",
                                        "Zwischenablage",
                                    ],
                                    |ui| {
                                        for m in PastePivot::ALL {
                                            ui.selectable_value(&mut paste_pivot, m, m.text())
                                                .on_hover_text(m.tip());
                                        }
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("gui.dialogs.basispunkt_drehen_um_das_fadenkreuz_mittelpunkt_vorschau"),
                                    ));
                                }
                                // T0103: Sprache der Oberfläche — wirkt sofort, ohne Neustart.
                zeile_mit(
                    &suche,
                    ui,
                    jjcad_core::i18n::tr("gui.dialogs.sprache"),
                    &["Language", "Deutsch", "Englisch", "English", "Übersetzung"],
                    |ui| {
                        let mut gewaehlt = jjcad_core::i18n::language();
                        let vorher = gewaehlt;
                        for l in jjcad_core::i18n::Lang::ALL {
                            ui.selectable_value(&mut gewaehlt, l, l.name());
                        }
                        if gewaehlt != vorher {
                            jjcad_core::i18n::set_language(gewaehlt);
                            app.prefs.language = gewaehlt;
                            let _ = app.prefs.save();
                            // Der Wechsel gilt ab dem nächsten Bild — ohne
                            // Anforderung bliebe die Oberfläche stehen.
                            ui.ctx().request_repaint();
                        }
                    },
                );
                if suche.is_empty() {
                    ui.label(theme::faint(
                        jjcad_core::i18n::tr("settings.language_hint"),
                    ));
                }
                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.dateidialog"), |ui| {
                                    let moeglich = crate::systemdialog::verfuegbar();
                                    ui.selectable_value(&mut dateidialog_system, true, jjcad_core::i18n::tr("gui.dialogs.system"))
                                        .on_hover_text(
                                            jjcad_core::i18n::tr("gui.dialogs.dialog_des_betriebssystems_dolphin_dateien_windows"),
                                        );
                                    ui.selectable_value(
                                        &mut dateidialog_system,
                                        false,
                                        jjcad_core::i18n::tr("gui.dialogs.eingebaut"),
                                    )
                                    .on_hover_text(jjcad_core::i18n::tr("gui.dialogs.dialog_von_joja_cad"));
                                    if !moeglich {
                                        ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.kein_systemdialog_verfuegbar")));
                                    }
                                });
                                // T0098: Fenster ohne Titelleiste. Ohne Rahmen des
                                // Fenstermanagers zeichnet JoJa-CAD Minimieren,
                                // Vergrößern und Schließen selbst; die Kopfzeile
                                // verschiebt das Fenster.
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.fensterrahmen"),
                                    &[
                                        "Titelleiste",
                                        "Fenster",
                                        "Rahmen",
                                        "Minimieren",
                                        "Vergrößern",
                                        "Maximieren",
                                        "Schließen",
                                        "KDE",
                                        "Windows",
                                    ],
                                    |ui| {
                                        for m in Fensterrahmen::ALL {
                                            ui.selectable_value(&mut fensterrahmen, m, m.text())
                                                .on_hover_text(m.tip());
                                        }
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr(
                                            "gui.dialogs.ohne_titelleiste_minimieren_vergroessern_und_schliessen",
                                        ),
                                    ));
                                }
                            }
                            // ------------------------ Darstellung -----------------
                            6 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.darstellung"));
                                // T0102: Hell, Dunkel oder wie das Betriebssystem —
                                // die Wahl wirkt sofort (Palette und Stil neu).
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.design"),
                                    &[
                                        "Farbschema",
                                        "Erscheinungsbild",
                                        "Aussehen",
                                        "Hell",
                                        "Dunkel",
                                        "Nachtmodus",
                                        "System",
                                        "Betriebssystem",
                                    ],
                                    |ui| {
                                        let mut gewaehlt = design;
                                        for m in Design::ALL {
                                            ui.selectable_value(&mut gewaehlt, m, m.text())
                                                .on_hover_text(m.tip());
                                        }
                                        design = gewaehlt;
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("settings.design_hint"),
                                    ));
                                }
                                // T0111: Der Akzent färbt alle hervorgehobenen
                                // Stellen der Bedienung; jede Wahl wird in ihrem
                                // eigenen Ton gezeigt (Farbprobe).
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.farbakzente"),
                                    &[
                                        "Akzent",
                                        "Akzentfarbe",
                                        "Bernstein",
                                        "Orange",
                                        "Saphir",
                                        "Smaragd",
                                        "Rubin",
                                        "Amethyst",
                                        "Blau",
                                        "Grün",
                                        "Rot",
                                        "Violett",
                                        "Farbe",
                                    ],
                                    |ui| {
                                        // Auswahlfeld wie alle anderen (150 px
                                        // Wertespalte, gleicher Pfeil); die
                                        // geschlossene Zeile trägt eine Farbprobe
                                        // des gewählten Akzents.
                                        let probe = theme::akzent_probe(farbakzent);
                                        let icon = panels::BtnIcon::Color(
                                            jjcad_core::style::Color::rgb(
                                                probe.r(),
                                                probe.g(),
                                                probe.b(),
                                            ),
                                        );
                                        let cap = farbakzent.text().to_string();
                                        if let Some(neu) = panels::dropdown(
                                            ui,
                                            "farbakzent",
                                            &cap,
                                            Some(icon),
                                            |ui| {
                                                let mut out = None;
                                                for m in Farbakzent::ALL {
                                                    let text = egui::RichText::new(m.text())
                                                        .color(theme::akzent_probe(m));
                                                    if ui
                                                        .selectable_label(m == farbakzent, text)
                                                        .on_hover_text(m.tip())
                                                        .clicked()
                                                    {
                                                        out = Some(m);
                                                    }
                                                }
                                                out
                                            },
                                        ) {
                                            farbakzent = neu;
                                        }
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("settings.farbakzent_hint"),
                                    ));
                                }
                                // T0114: höchste Rastergröße der Schrift. Höher
                                // heißt schärfer im hohen Zoom, kostet aber
                                // Schriftatlas — deshalb nur vier feste Stufen.
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.textaufloesung"),
                                    &[
                                        "Maximale Textauflösung",
                                        "Auflösung",
                                        "Schärfe",
                                        "Rastergröße",
                                        "Schrift",
                                        "Atlas",
                                    ],
                                    |ui| {
                                        let cap = textaufloesung.text().to_string();
                                        if let Some(neu) = panels::dropdown(
                                            ui,
                                            "textaufloesung",
                                            &cap,
                                            None,
                                            |ui| {
                                                let mut out = None;
                                                for m in Textaufloesung::ALL {
                                                    if ui
                                                        .selectable_label(
                                                            m == textaufloesung,
                                                            m.text(),
                                                        )
                                                        .on_hover_text(m.tip())
                                                        .clicked()
                                                    {
                                                        out = Some(m);
                                                    }
                                                }
                                                out
                                            },
                                        ) {
                                            textaufloesung = neu;
                                        }
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(
                                        jjcad_core::i18n::tr("settings.textaufloesung_hint"),
                                    ));
                                }
                                zeile(&suche, ui, jjcad_core::i18n::tr("gui.dialogs.linienstaerken"), |ui| {
                                    // T0139: die beiden Modi heißen jetzt nach dem, was sie
                                    // zeigen — „Bildschirm (konstant)“ und „Papier
                                    // (maßstabsgetreu)“. Auswahlfeld statt zweier Wahlen,
                                    // weil die vollen Namen nicht nebeneinander passen.
                                    let cap = if lw_scaled {
                                        jjcad_core::i18n::tr("gui.dialogs.linienstaerke_papier")
                                    } else {
                                        jjcad_core::i18n::tr("gui.dialogs.linienstaerke_bildschirm")
                                    };
                                    if let Some(neu) = panels::dropdown(
                                        ui,
                                        "linienstaerken_modus",
                                        cap,
                                        None,
                                        |ui| {
                                            let mut out = None;
                                            for (wert, text) in [
                                                (
                                                    false,
                                                    jjcad_core::i18n::tr(
                                                        "gui.dialogs.linienstaerke_bildschirm",
                                                    ),
                                                ),
                                                (
                                                    true,
                                                    jjcad_core::i18n::tr(
                                                        "gui.dialogs.linienstaerke_papier",
                                                    ),
                                                ),
                                            ] {
                                                if ui.selectable_label(lw_scaled == wert, text).clicked() {
                                                    out = Some(wert);
                                                }
                                            }
                                            out
                                        },
                                    ) {
                                        lw_scaled = neu;
                                    }
                                });
                                // T0139: Faktor nur im Modus „Bildschirm (konstant)“. Im
                                // Papier-Modus bleibt die Zeile sichtbar, aber deaktiviert —
                                // dort bestimmt der Maßstab der Zeichnung die Breite.
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("gui.dialogs.linienstaerke_faktor"),
                                    &["Faktor", "Skalierung", "Stärke", "Bildschirm"],
                                    |ui| {
                                        ui.add_enabled_ui(!lw_scaled, |ui| {
                                            ui.add_sized(
                                                [ui.available_width(), 20.0],
                                                widgets::zahl(
                                                    &mut lw_factor,
                                                    0.01,
                                                    jjcad_core::persist::LINIENSTAERKE_FAKTOR_MIN
                                                        ..=jjcad_core::persist::LINIENSTAERKE_FAKTOR_MAX,
                                                )
                                                .suffix(" ×"),
                                            );
                                        });
                                    },
                                );
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr(
                                        "settings.linienstaerke_faktor_hint",
                                    )));
                                }
                                // Häkchen mit vollem Text — die Labelspalte der
                                // Zeilen (96 px) schnitte „Eigenschaften-Panel“ ab.
                                for (name, wert) in [
                                    ("Layer-Panel", &mut show_layers),
                                    ("Eigenschaften-Panel", &mut show_props),
                                    (jjcad_core::i18n::tr("gui.dialogs.fangmarker"), &mut marker),
                                    ("Lineale einblenden", &mut rulers),
                                ] {
                                    if !suche.is_empty() && !name.to_lowercase().contains(&suche) {
                                        continue;
                                    }
                                    ui.add_space(2.0);
                                    ui.checkbox(wert, name);
                                }
                            }
                            // ------------------- Voreinstellungen ----------------
                            // T0125: eine Zeile je Objektart, Liste aus
                            // `style::LAYER_DEFAULTS` — nicht ein zweites Mal
                            // getippt. Der Wert gilt für neue Zeichnungen und
                            // neu angelegte Standardebenen; bestehende Ebenen
                            // einer offenen Zeichnung bleiben unverändert.
                            7 => {
                                abschnitt(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.linienstaerken_neuer_objekte"),
                                );
                                for (name, _, _) in jjcad_core::style::LAYER_DEFAULTS {
                                    zeile_mit(&suche, ui, name, &[], |ui| {
                                        let breite = app.prefs.linienstaerke(name);
                                        if let Some(neu) = panels::weight_combo(
                                            ui,
                                            &format!("vorgabe_{name}"),
                                            breite,
                                            false,
                                        ) {
                                            app.prefs.set_linienstaerke(name, neu);
                                            let _ = app.prefs.save();
                                            app.voreinstellungen_uebernehmen();
                                        }
                                    });
                                }
                                if suche.is_empty() {
                                    ui.add_space(8.0);
                                    if ui
                                        .button(jjcad_core::i18n::tr(
                                            "settings.vorgaben_zuruecksetzen",
                                        ))
                                        .clicked()
                                    {
                                        app.prefs.linienstaerken_zuruecksetzen();
                                        let _ = app.prefs.save();
                                        app.voreinstellungen_uebernehmen();
                                        ui.ctx().request_repaint();
                                    }
                                    ui.label(theme::faint(jjcad_core::i18n::tr(
                                        "settings.linienstaerken_hint",
                                    )));
                                }
                            }
                            // ------------------------ Speicherorte (T0127) ---------
                            8 => {
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("settings.speicherorte"));
                                // Die Wurzel ist nicht änderbar — sie steht als
                                // eigener Abschnitt über den Pfadfeldern, weil
                                // ein voller Pfad nicht in die Wertespalte passt.
                                let wurzel_name = jjcad_core::i18n::tr("settings.datenwurzel");
                                if suche.is_empty() || wurzel_name.to_lowercase().contains(&suche) {
                                    ui.add_space(4.0);
                                    ui.label(theme::bold(wurzel_name, 14.0));
                                    ui.add(
                                        egui::Label::new(theme::dim(
                                            &jjcad_core::daten::pfad_text(
                                                &jjcad_core::daten::wurzel(),
                                            ),
                                        ))
                                        .wrap(),
                                    );
                                    if ui
                                        .small_button(jjcad_core::i18n::tr(
                                            "settings.ordner_oeffnen",
                                        ))
                                        .clicked()
                                    {
                                        app.datenordner_oeffnen();
                                    }
                                }
                                if suche.is_empty() {
                                    ui.label(theme::faint(jjcad_core::i18n::tr(
                                        "settings.speicherorte_hint",
                                    )));
                                    ui.add_space(6.0);
                                }
                                if !app.ui.speicherorte_geladen {
                                    for (i, feld) in jjcad_core::persist::SpeicherortFeld::ALL
                                        .iter()
                                        .enumerate()
                                    {
                                        app.ui.speicherorte[i] =
                                            app.prefs.speicherort(*feld).to_string();
                                    }
                                    app.ui.speicherorte_geladen = true;
                                }
                                let mut aktion: Option<(jjcad_core::persist::SpeicherortFeld, bool)> =
                                    None;
                                for (i, feld) in jjcad_core::persist::SpeicherortFeld::ALL
                                    .iter()
                                    .copied()
                                    .enumerate()
                                {
                                    let name = match feld {
                                        jjcad_core::persist::SpeicherortFeld::Bloecke => {
                                            jjcad_core::i18n::tr("settings.blockbibliothek")
                                        }
                                        jjcad_core::persist::SpeicherortFeld::Sicherungen => {
                                            jjcad_core::i18n::tr("settings.sicherungen")
                                        }
                                        jjcad_core::persist::SpeicherortFeld::Protokolle => {
                                            jjcad_core::i18n::tr("settings.protokolle")
                                        }
                                    };
                                    if !suche.is_empty() && !name.to_lowercase().contains(&suche) {
                                        continue;
                                    }
                                    ui.add_space(8.0);
                                    ui.label(theme::bold(name, 14.0));
                                    let mut uebernehmen = false;
                                    let mut zuruecksetzen = false;
                                    // T0135: Die Knöpfe bestimmen ihre Breite
                                    // selbst — rechts nach links gesetzt nimmt
                                    // das Pfadfeld den Rest. Ein fester Abzug
                                    // (früher 210 px) war zu klein: die
                                    // Knopfzeile ragte aus der Seite und trieb
                                    // das ganze Fenster auf.
                                    let zeilen_breite = ui.available_width();
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(zeilen_breite, 30.0),
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                        if ui
                                            .button(jjcad_core::i18n::tr(
                                                "settings.zuruecksetzen",
                                            ))
                                            .clicked()
                                        {
                                            zuruecksetzen = true;
                                        }
                                        if ui
                                            .button(jjcad_core::i18n::tr("settings.uebernehmen"))
                                            .clicked()
                                        {
                                            uebernehmen = true;
                                        }
                                        let breite = ui.available_width();
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut app.ui.speicherorte[i],
                                            )
                                            .desired_width(breite)
                                            .hint_text(feld.vorgabe()),
                                        )
                                        .on_hover_text(jjcad_core::i18n::tr(
                                            "settings.speicherort_feld_hint",
                                        ));
                                        },
                                    );
                                    let angezeigt = jjcad_core::daten::pfad_in_wurzel(
                                        &app.ui.speicherorte[i],
                                        feld.vorgabe(),
                                    );
                                    ui.label(theme::faint(&jjcad_core::i18n::t_args_owned(
                                        "settings.vorgabe_pfad",
                                        &[("pfad", jjcad_core::daten::pfad_text(&angezeigt))],
                                    )));
                                    if uebernehmen {
                                        aktion = Some((feld, false));
                                    }
                                    if zuruecksetzen {
                                        aktion = Some((feld, true));
                                    }
                                }
                                if let Some((feld, zuruecksetzen)) = aktion {
                                    let idx = jjcad_core::persist::SpeicherortFeld::ALL
                                        .iter()
                                        .position(|f| *f == feld)
                                        .unwrap_or(0);
                                    if zuruecksetzen {
                                        app.ui.speicherorte[idx].clear();
                                        app.prefs.setze_speicherort(feld, "");
                                        let _ = app.prefs.save();
                                    } else {
                                        let text = app.ui.speicherorte[idx].clone();
                                        let pfad = jjcad_core::daten::pfad_in_wurzel(
                                            &text,
                                            feld.vorgabe(),
                                        );
                                        // Schreibprobe: einen nicht beschreibbaren
                                        // Pfad nicht still übernehmen (T0127).
                                        match jjcad_core::daten::pruefe_verzeichnis(&pfad) {
                                            Ok(()) => {
                                                app.prefs.setze_speicherort(feld, &text);
                                                let _ = app.prefs.save();
                                            }
                                            Err(grund) => {
                                                app.ui.speicherorte[idx] =
                                                    app.prefs.speicherort(feld).to_string();
                                                app.toast(jjcad_core::i18n::t_args_owned(
                                                    "settings.nicht_beschreibbar",
                                                    &[("grund", grund)],
                                                ));
                                            }
                                        }
                                    }
                                    if feld == jjcad_core::persist::SpeicherortFeld::Bloecke {
                                        app.benutzerbloecke_einhaengen();
                                    }
                                }
                            }
                            // ------------------------ Sicherung (T0129) --------
                            9 => {
                                if suche.is_empty() {
                                    ui.add(
                                        egui::Label::new(theme::dim(jjcad_core::i18n::tr(
                                            "settings.sicherung_erklaerung",
                                        )))
                                        .wrap(),
                                    );
                                    ui.add_space(8.0);
                                }
                                abschnitt(&suche, ui, jjcad_core::i18n::tr("settings.sicherung"));
                                zeile_tip(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_zwischenstaende"),
                                    &["Verlauf", "Sicherungen", "an", "aus"],
                                    jjcad_core::i18n::tr("settings.sicherung_zwischenstaende_tip"),
                                    |ui| {
                                        ui.checkbox(&mut s_verlauf, "");
                                    },
                                );
                                zeile_tip(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_anzahl"),
                                    &["Plätze", "Pool", "Stände", "Sicherungen"],
                                    jjcad_core::i18n::tr("settings.sicherung_anzahl_tip"),
                                    |ui| {
                                        ui.add(crate::widgets::zahl(
                                            &mut s_plaetze,
                                            1.0,
                                            jjcad_core::sicherung::PLAETZE_MIN as i32
                                                ..=jjcad_core::sicherung::PLAETZE_MAX as i32,
                                        ));
                                    },
                                );
                                zeile_tip(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_zeitraum"),
                                    &["Reicht zurück", "Aufbewahrung", "Arbeitsstunden"],
                                    jjcad_core::i18n::tr("settings.sicherung_zeitraum_tip"),
                                    |ui| {
                                        let caption = stunden_name(s_stunden);
                                        if let Some(neu) = crate::panels::dropdown(
                                            ui,
                                            "sicherung_stunden",
                                            &caption,
                                            None,
                                            |ui| {
                                                let mut wahl = None;
                                                for st in jjcad_core::sicherung::STUNDEN_WAHL {
                                                    if ui
                                                        .selectable_label(st == s_stunden, stunden_name(st))
                                                        .clicked()
                                                    {
                                                        wahl = Some(st);
                                                    }
                                                }
                                                wahl
                                            },
                                        ) {
                                            s_stunden = neu;
                                        }
                                    },
                                );
                                zeile_tip(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_intervall"),
                                    &["Minuten", "Abstand", "Sicherung alle"],
                                    jjcad_core::i18n::tr("settings.sicherung_intervall_tip"),
                                    |ui| {
                                        ui.add(crate::widgets::zahl(
                                            &mut s_intervall,
                                            1.0,
                                            jjcad_core::sicherung::INTERVALL_MIN as i32
                                                ..=jjcad_core::sicherung::INTERVALL_MAX as i32,
                                        ));
                                        // Einzahl hat einen eigenen Text: „1 Minuten“
                                        // liest sich falsch.
                                        let schluessel = if s_intervall == 1 {
                                            "settings.sicherung_minute"
                                        } else {
                                            "settings.sicherung_minuten"
                                        };
                                        ui.label(theme::dim(jjcad_core::i18n::tr(schluessel)));
                                    },
                                );
                                // Kennzahlen des Pools: höchstens alle zwei
                                // Sekunden neu lesen (ein Netzlaufwerk soll
                                // nicht bei jedem Bild gefragt werden).
                                let jetzt = ui.input(|i| i.time);
                                let veraltet = app
                                    .ui
                                    .sicherung_info
                                    .map(|(t, _, _, _)| jetzt - t > 2.0)
                                    .unwrap_or(true);
                                if veraltet {
                                    let (bytes, staende, dateien) = {
                                        let pool = app.sicherung_pool();
                                        (
                                            pool.pool_groesse(),
                                            pool.manifest().staende.len(),
                                            pool.dateien(),
                                        )
                                    };
                                    app.ui.sicherung_info = Some((jetzt, bytes, staende, dateien));
                                }
                                let (bytes, staende, dateien) = app
                                    .ui
                                    .sicherung_info
                                    .map(|(_, b, s, d)| (b, s, d))
                                    .unwrap_or((0, 0, 0));
                                let einst = jjcad_core::sicherung::Einstellungen {
                                    verlauf: s_verlauf,
                                    plaetze: s_plaetze.max(0) as u32,
                                    stunden: s_stunden,
                                    intervall_min: s_intervall.max(1) as u32,
                                };
                                let abstand = einst.abstand(dateien);
                                zeile_tip(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_abstand"),
                                    &["Abstand der Stände", "Prozent"],
                                    jjcad_core::i18n::tr("settings.sicherung_abstand_tip"),
                                    |ui| {
                                        ui.label(theme::dim(&format!(
                                            "{} %",
                                            zahl_deutsch(abstand * 100.0, 1)
                                        )));
                                    },
                                );
                                if suche.is_empty() {
                                    // Konkretes Beispiel statt nur einer Prozentzahl:
                                    // dieselben Zahlen stehen in `docs/SICHERUNG.md`.
                                    ui.add(
                                        egui::Label::new(theme::faint(
                                            jjcad_core::i18n::t_args_owned(
                                                "settings.sicherung_abstand_beispiel",
                                                &[
                                                    ("stunde", zahl_deutsch(60.0 * abstand, 0)),
                                                    ("tag", zahl_deutsch(480.0 * abstand, 0)),
                                                ],
                                            ),
                                        ))
                                        .wrap(),
                                    );
                                    ui.add_space(6.0);
                                }
                                zeile_mit(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_pool"),
                                    &["Speicher", "MB", "Größe"],
                                    |ui| {
                                        // Einzahl hat einen eigenen Text: „1 Stände“
                                        // liest sich falsch.
                                        let schluessel = if staende == 1 {
                                            "settings.sicherung_pool_wert_eins"
                                        } else {
                                            "settings.sicherung_pool_wert"
                                        };
                                        ui.label(theme::dim(&jjcad_core::i18n::t_args_owned(
                                            schluessel,
                                            &[
                                                (
                                                    "groesse",
                                                    zahl_deutsch(
                                                        bytes as f64 / 1_000_000.0,
                                                        1,
                                                    ),
                                                ),
                                                ("staende", staende.to_string()),
                                            ],
                                        )));
                                    },
                                );
                                zeile(
                                    &suche,
                                    ui,
                                    jjcad_core::i18n::tr("settings.sicherung_ordner"),
                                    |ui| {
                                        if ui
                                            .button(jjcad_core::i18n::tr(
                                                "settings.sicherung_ordner_oeffnen",
                                            ))
                                            .clicked()
                                        {
                                            app.sicherungsordner_oeffnen();
                                        }
                                    },
                                );
                                if suche.is_empty() {
                                    ui.add(
                                        egui::Label::new(theme::faint(jjcad_core::i18n::tr(
                                            "settings.sicherung_hint",
                                        )))
                                        .wrap(),
                                    );
                                }
                            }
                            // Die Auswahl oben ist auf die Liste begrenzt; der
                            // Zweig hält den `match` vollständig.
                            _ => {}
                        }
                    });
            },
        );
    });

    app.ui.settings_kat = kat;

    // ------------------------------ anwenden -----------------------------
    app.ui.lineweights = true;
    app.ui.lineweights_scaled = lw_scaled;
    app.ui.drag_mode = drag_mode;
    app.prefs.drag_mode = drag_mode;
    if app.ui.select_mode != select_mode || app.prefs.select_mode != select_mode {
        // T0145: gilt sofort; ein offenes Zwei-Klick-Rechteck gehört dem alten
        // Modus und wird in der Zeichenfläche verworfen.
        app.ui.select_mode = select_mode;
        app.prefs.select_mode = select_mode;
        let _ = app.prefs.save();
    }
    if app.prefs.mouse_move != mouse_move {
        app.prefs.mouse_move = mouse_move;
        let _ = app.prefs.save();
    }
    if (app.prefs.pan_speed - pan_speed).abs() > 1e-6 {
        app.prefs.pan_speed = pan_speed.clamp(
            jjcad_core::persist::SCHWENK_FAKTOR_MIN,
            jjcad_core::persist::SCHWENK_FAKTOR_MAX,
        );
        let _ = app.prefs.save();
    }
    if app.prefs.pan_mode != pan_mode {
        // T0101: gilt sofort — der Dauerlauf liest die Einstellung je Bild.
        app.prefs.pan_mode = pan_mode;
        let _ = app.prefs.save();
    }
    if app.prefs.paste_pivot != paste_pivot {
        // T0087: sofort übernehmen — auch für eine bereits laufende Vorschau.
        app.set_paste_pivot(paste_pivot);
        let _ = app.prefs.save();
    }
    if app.prefs.fensterrahmen != fensterrahmen {
        // T0098: sofort umschalten — der Fenstermanager bekommt „keine
        // Dekoration“ über denselben Weg wie beim Start (`with_decorations`).
        app.prefs.fensterrahmen = fensterrahmen;
        let _ = app.prefs.save();
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Decorations(
                !fensterrahmen.ohne_titelleiste(),
            ));
        // Ohne diese Anforderung bliebe die Kopfzeile im alten Zustand stehen,
        // bis der nächste Klick ein Bild auslöst.
        ui.ctx().request_repaint();
    }
    if app.prefs.design != design {
        // T0102: sofort umschalten — Palette und Stil werden neu gesetzt,
        // die Wahl steht in `prefs.json`.
        app.set_design(design, ui.ctx());
    }
    if app.prefs.farbakzent != farbakzent {
        // T0111: ebenso der Akzent — Palette und Stil tragen den neuen Ton.
        app.set_akzent(farbakzent, ui.ctx());
    }
    if app.prefs.textaufloesung != textaufloesung {
        // T0114: gilt sofort — die Zeichenfläche liest die Rastergröße bei
        // jedem Bild. Ein Bild genügt, dann steht der Text in der neuen Schärfe.
        app.prefs.textaufloesung = textaufloesung;
        let _ = app.prefs.save();
        ui.ctx().request_repaint();
    }
    // T0129: Sicherung — die Werte gelten ab dem nächsten Zyklus.
    let s_plaetze = s_plaetze.clamp(
        jjcad_core::sicherung::PLAETZE_MIN as i32,
        jjcad_core::sicherung::PLAETZE_MAX as i32,
    ) as u32;
    let s_intervall = s_intervall.clamp(
        jjcad_core::sicherung::INTERVALL_MIN as i32,
        jjcad_core::sicherung::INTERVALL_MAX as i32,
    ) as u32;
    if app.prefs.sicherung_verlauf != s_verlauf
        || app.prefs.sicherung_plaetze != s_plaetze
        || app.prefs.sicherung_stunden != s_stunden
        || app.prefs.sicherung_intervall != s_intervall
    {
        app.prefs.sicherung_verlauf = s_verlauf;
        app.prefs.sicherung_plaetze = s_plaetze;
        app.prefs.sicherung_stunden = s_stunden;
        app.prefs.sicherung_intervall = s_intervall;
        let _ = app.prefs.save();
        // Die Ausdünnung greift sofort mit den neuen Werten.
        let einst = app.prefs.sicherung_einstellungen();
        app.sicherung_pool().ausduennen(&einst);
        app.ui.sicherung_info = None;
    }
    let dialog_neu = if dateidialog_system { "system" } else { "intern" };
    if app.prefs.file_dialog != dialog_neu {
        app.prefs.file_dialog = dialog_neu.to_string();
        let _ = app.prefs.save();
    }
    app.prefs.lineweights_scaled = lw_scaled;
    // T0139: der Faktor gilt sofort (die Zeichenfläche liest ihn je Bild).
    app.ui.lineweight_factor = lw_factor;
    if (app.prefs.lineweight_factor - lw_factor).abs() > 1e-6 {
        app.prefs.lineweight_factor = lw_factor;
        let _ = app.prefs.save();
        ui.ctx().request_repaint();
    }
    app.ui.layers_open = show_layers;
    app.ui.props_open = show_props;
    app.prefs.snap_marker = marker;
    if show_grid != app.ui.show_grid {
        app.ui.show_grid = show_grid;
    }

    let settings_changed = input != cur.input_unit
        || decimals as usize != cur.decimals
        || (grid_step - cur.snap.grid_step).abs() > 1e-9
        || rasterart != cur.snap.grid_kind
        || rastermodus != cur.snap.grid_mode
        || grid_force != cur.snap.grid_force
        || (polar_step - cur.snap.polar_step).abs() > 1e-9
        || (polar_tol - cur.snap.polar_tolerance).abs() > 1e-9
        || (tol_px - cur.snap.tolerance_px).abs() > 1e-9
        || snap.endpoint != cur.snap.endpoint
        || snap.midpoint != cur.snap.midpoint
        || snap.center != cur.snap.center
        || snap.intersection != cur.snap.intersection
        || snap.perpendicular != cur.snap.perpendicular
        || snap.nearest != cur.snap.nearest
        || snap.grid != cur.snap.grid
        || snap.ortho != cur.snap.ortho
        || snap.polar != cur.snap.polar
        || snap.wall_edges != cur.snap.wall_edges
        || show_grid != cur.show_grid
        || (th - cur.text_height).abs() > 1e-9
        || (wall - cur.wall_thickness).abs() > 1e-9
        || (door - cur.door_width).abs() > 1e-9
        || (win - cur.window_width).abs() > 1e-9
        || (stair - cur.stair_width).abs() > 1e-9
        || rulers != cur.show_rulers;

    if settings_changed {
        let p = json!({
            "input_unit": input.short(),
            "decimals": decimals,
            "show_grid": show_grid,
            "show_rulers": rulers,
            "text_height": unit.from_mm(th),
            "wall_thickness": unit.from_mm(wall),
            "door_width": unit.from_mm(door),
            "window_width": unit.from_mm(win),
            "stair_width": unit.from_mm(stair),
            "snap": {
                "endpoint": snap.endpoint, "midpoint": snap.midpoint, "center": snap.center,
                "intersection": snap.intersection, "perpendicular": snap.perpendicular,
                "nearest": snap.nearest, "grid": snap.grid, "ortho": snap.ortho,
                "polar": snap.polar, "wall_edges": snap.wall_edges,
                "grid_step": unit.from_mm(grid_step),
                "grid_kind": rasterart.key(),
                "grid_mode": rastermodus.key(),
                "grid_force": grid_force,
                "polar_step": polar_step,
                "polar_tolerance": polar_tol,
                "tolerance_px": tol_px
            }
        });
        let _ = app.session_mut().execute("doc.settings", &p);
        app.mark_changed();
        // Wandkontur-Fang auch app-weit merken (überlebt Neustart/neue Zeichnung).
        app.prefs.snap_wall_edges = snap.wall_edges;
        let _ = app.prefs.save();
    }

    // Textgröße und Bogensymbole stehen im Eigenschaften-Panel (Abschnitt
    // „Maßketten"); hier werden nur noch Pfeilgröße und Nachkommastellen gesetzt.
    if ds_changed {
        let _ = app.session_mut().execute(
            "dim.style",
            &json!({"arrow_size": ds.arrow_size, "decimals": ds.decimals}),
        );
        app.mark_changed();
    }
}

// ---------------------------------------------------------------------------
// Tastaturbelegung
// ---------------------------------------------------------------------------

/// Eine Tasteneinheit — die Breite einer normalen Taste ohne Zwischenraum.
const KEY_W: f32 = 44.0;
/// Höhe einer Tastenreihe.
const KEY_H: f32 = 42.0;
/// Zwischenraum zwischen zwei Tasten.
const KEY_GAP: f32 = 4.0;
/// Breite des Tastenbilds in Tasteneinheiten — jede Reihe schließt rechts damit ab.
const KEY_UNITS: f32 = 15.0;
/// Zahl der Tastenreihen.
const KEY_ROWS: usize = 5;
/// Fläche gesperrter Tasten: flaches Grau, deutlich abgesetzt von den freien
/// Tasten (`Palette::bg()`) — der Unterschied ist die Fläche, nicht nur die
/// Schrift. Der Wert kommt aus dem Design (T0102), damit die Sperre auch im
/// hellen Design als Sperre zu erkennen ist.
fn gesperrt_flaeche() -> Color32 {
    Palette::taste_gesperrt()
}
/// Beschriftung gesperrter Tasten: schwächer als `Palette::text_faint()`.
fn gesperrt_text() -> Color32 {
    Palette::taste_gesperrt_text()
}

/// Tastenbild einer deutschen Tastatur: (Taste, Beschriftung, Zeile, Spalte,
/// Breite, untere Breite). Zeile, Spalte und Breite rechnen in Tasteneinheiten
/// (`KEY_W` + `KEY_GAP`), damit die Reihen rechtsbündig schließen. `enter` ist
/// zwei Zeilen hoch und steht am rechten Rand über der QWERTZ- und der
/// Grundreihe — dort, wo die Enter-Taste einer deutschen Tastatur auch sitzt;
/// seine untere Hälfte ist schmaler (`0` = so breit wie oben), damit `#` eine
/// ganze Taste behält. Die Umschalt-Reihe läuft darunter bis zum Rand durch.
const TASTEN: &[(&str, &str, usize, f32, f32, f32)] = &[
    // Ziffernreihe — nach der Null folgt die Akut-Taste (´), dann Rücktaste
    ("esc", "Esc", 0, 0.0, 1.25, 0.0),
    ("1", "1", 0, 1.25, 1.0, 0.0),
    ("2", "2", 0, 2.25, 1.0, 0.0),
    ("3", "3", 0, 3.25, 1.0, 0.0),
    ("4", "4", 0, 4.25, 1.0, 0.0),
    ("5", "5", 0, 5.25, 1.0, 0.0),
    ("6", "6", 0, 6.25, 1.0, 0.0),
    ("7", "7", 0, 7.25, 1.0, 0.0),
    ("8", "8", 0, 8.25, 1.0, 0.0),
    ("9", "9", 0, 9.25, 1.0, 0.0),
    ("0", "0", 0, 10.25, 1.0, 0.0),
    ("´", "´", 0, 11.25, 1.0, 0.0),
    ("backspace", "Bksp", 0, 12.25, 2.75, 0.0),
    // QWERTZ-Reihe — die Plus-Taste ist eine normale Taste, der Rest der Reihe
    // gehört dem zwei Reihen hohen Enter
    ("tab", "Tab", 1, 0.0, 1.5, 0.0),
    ("q", "Q", 1, 1.5, 1.0, 0.0),
    ("w", "W", 1, 2.5, 1.0, 0.0),
    ("e", "E", 1, 3.5, 1.0, 0.0),
    ("r", "R", 1, 4.5, 1.0, 0.0),
    ("t", "T", 1, 5.5, 1.0, 0.0),
    ("z", "Z", 1, 6.5, 1.0, 0.0),
    ("u", "U", 1, 7.5, 1.0, 0.0),
    ("i", "I", 1, 8.5, 1.0, 0.0),
    ("o", "O", 1, 9.5, 1.0, 0.0),
    ("p", "P", 1, 10.5, 1.0, 0.0),
    ("ü", "Ü", 1, 11.5, 1.0, 0.0),
    ("+", "+", 1, 12.5, 1.0, 0.0),
    ("enter", "Enter", 1, 13.5, 1.5, 1.0),
    // Grundreihe — gegenüber der QWERTZ-Reihe um eine halbe Taste versetzt;
    // `#` schließt an das Enter an
    ("caps", "Caps", 2, 0.0, 2.0, 0.0),
    ("a", "A", 2, 2.0, 1.0, 0.0),
    ("s", "S", 2, 3.0, 1.0, 0.0),
    ("d", "D", 2, 4.0, 1.0, 0.0),
    ("f", "F", 2, 5.0, 1.0, 0.0),
    ("g", "G", 2, 6.0, 1.0, 0.0),
    ("h", "H", 2, 7.0, 1.0, 0.0),
    ("j", "J", 2, 8.0, 1.0, 0.0),
    ("k", "K", 2, 9.0, 1.0, 0.0),
    ("l", "L", 2, 10.0, 1.0, 0.0),
    ("ö", "Ö", 2, 11.0, 1.0, 0.0),
    ("ä", "Ä", 2, 12.0, 1.0, 0.0),
    ("#", "#", 2, 13.0, 1.0, 0.0),
    // Umschalt-Reihe mit der Taste `<` links neben Y; der rechte Umschalt
    // (`R-Shift`) reicht bis zum rechten Rand des Tastenbilds
    ("shift", "Shift", 3, 0.0, 1.5, 0.0),
    ("<", "<", 3, 1.5, 1.0, 0.0),
    ("y", "Y", 3, 2.5, 1.0, 0.0),
    ("x", "X", 3, 3.5, 1.0, 0.0),
    ("c", "C", 3, 4.5, 1.0, 0.0),
    ("v", "V", 3, 5.5, 1.0, 0.0),
    ("b", "B", 3, 6.5, 1.0, 0.0),
    ("n", "N", 3, 7.5, 1.0, 0.0),
    ("m", "M", 3, 8.5, 1.0, 0.0),
    (",", ",", 3, 9.5, 1.0, 0.0),
    (".", ".", 3, 10.5, 1.0, 0.0),
    ("-", "-", 3, 11.5, 1.0, 0.0),
    ("shift_r", "R-Shift", 3, 12.5, 2.5, 0.0),
    // Untere Reihe mit den rechten Steuerungstasten
    ("ctrl", "Strg", 4, 0.0, 1.75, 0.0),
    ("alt", "Alt", 4, 1.75, 1.75, 0.0),
    ("space", "Leertaste", 4, 3.5, 8.0, 0.0),
    ("altgr", "AltGr", 4, 11.5, 1.75, 0.0),
    ("ctrl_r", "Strg", 4, 13.25, 1.75, 0.0),
];

/// Wie viele Reihen eine Taste hoch ist — nur `enter` ist zwei Reihen hoch.
fn tasten_hoehe(key: &str) -> f32 {
    if key == "enter" { 2.0 } else { 1.0 }
}

/// Fläche des ganzen Tastenbilds.
fn tastatur_flaeche() -> Vec2 {
    Vec2::new(
        KEY_UNITS * KEY_W + (KEY_UNITS - 1.0) * KEY_GAP,
        KEY_ROWS as f32 * KEY_H + (KEY_ROWS as f32 - 1.0) * KEY_GAP,
    )
}

/// Rechteck einer Taste in Tasteneinheiten, bezogen auf `orig`.
fn taste_rect(orig: Pos2, zeile: f32, spalte: f32, breite: f32, hoehe: f32) -> Rect {
    let min = Pos2::new(
        orig.x + spalte * (KEY_W + KEY_GAP),
        orig.y + zeile * (KEY_H + KEY_GAP),
    );
    let size = Vec2::new(
        breite * KEY_W + (breite - 1.0) * KEY_GAP,
        hoehe * KEY_H + (hoehe - 1.0) * KEY_GAP,
    );
    Rect::from_min_size(min, size)
}

/// Die Flächen einer Taste. Eine L-förmige Taste (`unten` kleiner als `breite`)
/// liefert zwei Rechtecke: die obere Hälfte in voller Breite, die untere
/// rechtsbündig schmaler — so behält `#` seine ganze Taste neben dem Enter.
fn taste_flaechen(
    orig: Pos2,
    key: &str,
    zeile: usize,
    spalte: f32,
    breite: f32,
    unten: f32,
) -> Vec<Rect> {
    let hoehe = tasten_hoehe(key);
    if hoehe > 1.0 && unten > 0.0 && unten < breite {
        return vec![
            taste_rect(orig, zeile as f32, spalte, breite, 1.0),
            taste_rect(
                orig,
                zeile as f32 + 1.0,
                spalte + breite - unten,
                unten,
                1.0,
            ),
        ];
    }
    vec![taste_rect(orig, zeile as f32, spalte, breite, hoehe)]
}

/// Umriss einer L-förmigen Taste (sieben Ecken in einem Zug) — die Umrandung läuft in einem
/// Zug um beide Hälften, sonst stünde eine Naht quer über der Taste.
fn taste_umriss(flaechen: &[Rect]) -> Option<Vec<Pos2>> {
    let [oben, unten] = flaechen else {
        return None;
    };
    Some(vec![
        oben.left_top(),
        oben.right_top(),
        oben.right_bottom(),
        unten.right_bottom(),
        unten.left_bottom(),
        unten.left_top(),
        oben.left_bottom(),
    ])
}

/// Rechtecke aller Tasten, bezogen auf die linke obere Ecke `orig`.
/// **Eine** Quelle für Zeichnen und Prüfung (`tests_tastatur`); bei einer
/// L-förmigen Taste ist es die umschließende Fläche.
fn tasten_rechtecke(orig: Pos2) -> Vec<(&'static str, &'static str, Rect)> {
    TASTEN
        .iter()
        .map(|(key, label, zeile, spalte, breite, unten)| {
            let flaechen = taste_flaechen(orig, key, *zeile, *spalte, *breite, *unten);
            let rect = flaechen
                .iter()
                .skip(1)
                .fold(flaechen[0], |a, b| a.union(*b));
            (*key, *label, rect)
        })
        .collect()
}

/// Belegte Flächen je Taste — beim L-förmigen Enter zwei, sonst eine.
/// Grundlage der Überlappungsprüfung: die umschließende Fläche des Enter würde
/// die Ecke von `#` mit abdecken, obwohl dort `#` liegt.
fn tasten_belegung(orig: Pos2) -> Vec<(&'static str, Rect)> {
    TASTEN
        .iter()
        .flat_map(|(key, _, zeile, spalte, breite, unten)| {
            taste_flaechen(orig, key, *zeile, *spalte, *breite, *unten)
                .into_iter()
                .map(move |r| (*key, r))
        })
        .collect()
}

/// Legt eine Funktion auf eine Taste der Tastaturbelegung (T0064).
///
/// Derselbe Weg wie „Funktion im Inventar wählen" (T0072): `UserPrefs::bind`
/// prüft die Sperren und räumt eine alte Belegung derselben Taste weg. Liefert
/// `false`, wenn die Funktion fest verdrahtet ist (Strg+S, F1 …) — dann bleibt
/// die Belegung unverändert.
pub(crate) fn funktion_auf_taste(app: &mut App, funktion: &str, key: &str) -> bool {
    if !app.prefs.belegung_aenderbar(funktion) {
        return false;
    }
    let name = actions::action(funktion)
        .map(|d| d.label().to_string())
        .unwrap_or_else(|| funktion.to_string());
    app.prefs.bind(funktion, key);
    let _ = app.prefs.save();
    app.ui.keymap_funktion = None;
    app.ui.bind_key = None;
    app.ui.keymap_selected = Some(key.to_lowercase());
    // Die gewählte Funktion hat ihr Ziel erreicht — das Werkzeug endet wie beim
    // Klick auf einen leeren Hotbar-Slot.
    app.tool = None;
    // Chat-Schema `<Objekt> · <Zustand>` — der Pfeil fehlt in der Schrift des
    // Protokolls und erscheint dort als leeres Kästchen.
    app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.x_taste_x", &[("name", format!("{}", name)), ("wert1", format!("{}", key.to_uppercase()))]));
    true
}

/// Bedienung der Tastaturbelegung — steht im Tooltip am kleinen „i" über dem
/// Tastenbild (T0064). Die Bedienung ist selbsterklärend; ein langer Hinweistext
/// gehört nicht ins Fenster.
fn keymap_hilfe() -> &'static str {
    jjcad_core::i18n::tr("gui.dialogs.keymap_hilfe")
}

/// Das Fenster der Tastaturbelegung (T0076): eigene Kopfleiste mit dem
/// Informationssymbol **links** und dem Schließen rechts, darunter das
/// Tastenbild, unten die Layoutauswahl. Die Breite ist genau die Breite des
/// Tastenbilds — rechts bleibt kein leerer Rand.
fn keymap_window(app: &mut App, ctx: &egui::Context) {
    let breite = tastatur_flaeche().x;
    let mut offen = true;
    let mut zug: Option<Vec2> = None;
    let mut fenster = egui::Window::new("Tastaturbelegung")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .default_width(breite)
        .frame(theme::dialog_frame());
    fenster = match app.ui.keymap_pos {
        Some(pos) => fenster.current_pos(pos),
        None => fenster.anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO),
    };
    let ergebnis = fenster.show(ctx, |ui| {
        ui.set_width(breite);
        // Obere Leiste: „i" linksbündig, Titel, offene Funktion, Schließen.
        ui.horizontal(|ui| {
            let (rect, antwort) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            widgets::icon(ui, rect, "info", Palette::text_dim());
            antwort.on_hover_text(keymap_hilfe());
            app.ui.last_keymap_info = Some(rect);
            ui.add_space(4.0);
            let titel = ui.add(
                egui::Label::new(theme::bold(jjcad_core::i18n::tr("gui.dialogs.tastaturbelegung"), 14.0)).sense(Sense::drag()),
            );
            if titel.dragged() {
                zug = Some(titel.drag_delta());
            }
            // T0064: Die zuletzt gewählte Funktion steht in der Leiste, damit
            // sichtbar ist, was eine leere Taste gerade aufnimmt.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, "close", 20.0, false, jjcad_core::i18n::tr("gui.dialogs.schliessen")).clicked() {
                    offen = false;
                }
                if let Some(f) = app.ui.keymap_funktion.clone() {
                    let name = actions::action(&f).map(|d| d.label().to_string()).unwrap_or(f);
                    ui.label(theme::mono(jjcad_core::i18n::t_args_owned("gui.dialogs.bereit_x", &[("name", format!("{}", name))])));
                }
            });
        });
        ui.add_space(2.0);
        ui.separator();
        keyboard_body(app, ui);
    });
    if let Some(r) = &ergebnis {
        app.ui.last_keymap_rect = Some(r.response.rect);
        if let Some(d) = zug {
            app.ui.keymap_pos = Some(r.response.rect.min + d);
        }
    }
    // ESC gehört einer offenen Rückfrage (Wechsel/Speichern), nicht dem Fenster.
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && app.ui.keymap_wechsel.is_none()
        && app.ui.keymap_name.is_none();
    if !offen || esc {
        app.ui.keymap_open = false;
    }
}

/// Wechselt auf ein gespeichertes Layout (T0076). Ist die aktuelle Belegung in
/// keinem Layout gespeichert, kommt erst die Rückfrage „Abbrechen / Verwerfen /
/// Speichern" — sonst ginge die Änderung beim Wechsel still verloren.
fn layout_wechseln(app: &mut App, wahl: usize) {
    if Some(wahl) == app.prefs.keymap_aktiv() {
        return;
    }
    if app.prefs.keymap_ungespeichert() {
        app.ui.keymap_wechsel = Some(wahl);
        return;
    }
    if let Some(name) = app.prefs.keymap_anwenden(wahl) {
        let _ = app.prefs.save();
        app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.layout_x", &[("name", format!("{}", name))]));
    }
}

/// Rückfrage vor dem Wechsel aus einer ungespeicherten Belegung (T0076).
/// „Speichern" öffnet die Namensabfrage; der Wechsel wird danach ausgeführt.
fn keymap_wechsel_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(ziel) = app.ui.keymap_wechsel else {
        return;
    };
    // Während die Namensabfrage steht, wird diese Rückfrage nicht gezeichnet.
    if app.ui.keymap_name.is_some() {
        return;
    }
    let zielname = app
        .prefs
        .keymap_presets
        .get(ziel)
        .map(|p| p.name.clone())
        .unwrap_or_default();

    #[derive(Clone, Copy)]
    enum Wahl {
        Abbrechen,
        Verwerfen,
        Speichern,
    }
    let mut wahl = None;
    dim_background(ctx);
    let mut offen = true;
    modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.layout_wechseln"), 460.0, &mut offen).show(ctx, |ui| {
        ui.set_width(436.0);
        ui.label(theme::bold(jjcad_core::i18n::tr("gui.dialogs.belegung_nicht_gespeichert"), 13.0));
        ui.add_space(4.0);
        ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.dialogs.die_aktuelle_belegung_gehoert_zu_keinem_layout_beim_wech", &[("zielname", format!("{}", zielname))])));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                wahl = Some(Wahl::Abbrechen);
            }
            if ui.button(jjcad_core::i18n::tr("gui.dialogs.verwerfen")).clicked() {
                wahl = Some(Wahl::Verwerfen);
            }
            if ui.button(jjcad_core::i18n::tr("gui.dialogs.speichern")).clicked() {
                wahl = Some(Wahl::Speichern);
            }
        });
    });
    if !offen || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        wahl = Some(Wahl::Abbrechen);
    }
    match wahl {
        None => {}
        Some(Wahl::Abbrechen) => app.ui.keymap_wechsel = None,
        Some(Wahl::Verwerfen) => {
            app.ui.keymap_wechsel = None;
            if let Some(name) = app.prefs.keymap_anwenden(ziel) {
                let _ = app.prefs.save();
                app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.layout_x", &[("name", format!("{}", name))]));
            }
        }
        // Erst sichern, dann wechseln (die Namensabfrage merkt sich das Ziel).
        Some(Wahl::Speichern) => app.ui.keymap_name = Some(String::new()),
    }
}

/// Namen für ein neues Layout erfragen (T0076) — „Speichern …" in der
/// Layoutzeile.
fn keymap_name_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut buf) = app.ui.keymap_name.clone() else {
        return;
    };
    let mut offen = true;
    let mut speichern = false;
    let mut abbrechen = false;
    modal_window(ctx, jjcad_core::i18n::tr("gui.dialogs.layout_speichern"), 380.0, &mut offen).show(ctx, |ui| {
        ui.set_width(356.0);
        ui.label(theme::dim(
            jjcad_core::i18n::tr("gui.dialogs.name_des_layouts_ist_der_name_schon_vergeben_wird_er_ueb"),
        ));
        ui.add_space(6.0);
        let feld = ui.add_sized(
            [356.0, 24.0],
            egui::TextEdit::singleline(&mut buf).hint_text(jjcad_core::i18n::tr("gui.dialogs.z_b_architektur")),
        );
        feld.request_focus();
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            speichern = true;
        }
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(jjcad_core::i18n::tr("gui.dialogs.speichern")).clicked() {
                speichern = true;
            }
            if ui.button(jjcad_core::i18n::tr("gui.dialogs.abbrechen")).clicked() {
                abbrechen = true;
            }
        });
    });
    if !offen || abbrechen {
        app.ui.keymap_name = None;
        return;
    }
    if speichern {
        match app.prefs.keymap_speichern(&buf) {
            Ok(name) => {
                let _ = app.prefs.save();
                app.ui.keymap_name = None;
                app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.layout_gespeichert_x", &[("name", format!("{}", name))]));
                // War ein Wechsel offen, wird er jetzt ausgeführt — die
                // Änderung ist gesichert (T0076).
                if let Some(ziel) = app.ui.keymap_wechsel.take() {
                    if let Some(zielname) = app.prefs.keymap_anwenden(ziel) {
                        let _ = app.prefs.save();
                        app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.layout_x_2", &[("zielname", format!("{}", zielname))]));
                    }
                }
            }
            // Ungültiger Name: das Feld bleibt offen, die Meldung nennt den Grund.
            Err(e) => {
                app.toast(e);
                app.ui.keymap_name = Some(buf);
            }
        }
    } else {
        app.ui.keymap_name = Some(buf);
    }
}

/// Name eines Layouts, gekürzt auf die Breite der Dropdown-Wertespalte.
/// Anzeigename einer eingebauten Vorlage (T0106). Gespeichert und verglichen
/// wird der deutsche Name (`persist::VORLAGEN`) — nur die Oberfläche übersetzt.
fn preset_anzeige(name: &str) -> String {
    if jjcad_core::persist::ist_vorlage(name) {
        let kurz = name.trim_start_matches("Vorlage ").trim();
        return jjcad_core::i18n::t_args_owned("gui.dialogs.vorlage_x", &[("name", kurz.to_string())]);
    }
    name.to_string()
}

/// Beschriftung einer Taste im Tastenbild (T0106) — `Strg` und `Leertaste`
/// sind deutsche Tastennamen, die englische Oberfläche sagt Ctrl und Space.
fn taste_beschriftung(label: &'static str) -> &'static str {
    match label {
        "Strg" => jjcad_core::i18n::tr("gui.dialogs.key_ctrl"),
        "Leertaste" => jjcad_core::i18n::tr("gui.dialogs.key_space"),
        other => other,
    }
}

fn keymap_kurz(name: &str) -> String {
    const MAX: usize = 20;
    if name.chars().count() > MAX {
        let mut s: String = name.chars().take(MAX - 1).collect();
        s.push('…');
        s
    } else {
        name.to_string()
    }
}

/// Höhe aller Bedienelemente der Layoutzeile (T0076). Vorher war das
/// Auswahlfeld 20 px hoch und die Knöpfe daneben 28 px — die Zeile wirkte
/// schief. Die Höhe ist jetzt **die** Höhe eines Knopfes, nach derselben Regel,
/// die egui selbst anwendet (`Button`: Texthöhe + zweimal Rand, mindestens die
/// Bedienhöhe); das Auswahlfeld bekommt genau diesen Wert.
fn keymap_zeile_hoehe(ui: &Ui) -> f32 {
    (ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y)
        .max(ui.spacing().interact_size.y)
}

/// Knopf der Layoutzeile — natürliche Größe, also genau `keymap_zeile_hoehe`.
fn zeilen_knopf(ui: &mut Ui, text: &str) -> egui::Response {
    ui.button(text)
}

/// Layoutzeile unter dem Tastenbild (T0076/T0079): Auswahl der Layouts,
/// Speichern, Import und Export. Sie ersetzt den früheren Knopf
/// „Werkseinstellungen" — die erste Zeile ist die Vorlage JoJa-CAD, daneben
/// stehen die Vorlagen Allplan, ArchiCAD und AutoCAD.
fn keyboard_layouts(app: &mut App, ui: &mut Ui) {
    let mut zeile: Vec<(String, Rect)> = Vec::new();
    ui.horizontal(|ui| {
        ui.label(theme::dim(jjcad_core::i18n::tr("gui.dialogs.layout")));
        // Das **ausdrücklich** gewählte Layout (`keymap_aktiv`), nicht das erste
        // mit gleicher Belegung: sonst zeigte die Auswahl nach dem Speichern
        // wieder die erste Vorlage (T0076).
        let aktiv = app.prefs.keymap_aktiv();
        // Die Wertespalte des Auswahlfelds ist 150 px breit — lange Namen würden
        // in die Knöpfe daneben laufen.
        let mut eintraege: Vec<(usize, String)> = app
            .prefs
            .keymap_presets
            .iter()
            .enumerate()
            .map(|(i, p)| (i, keymap_kurz(&preset_anzeige(&p.name))))
            .collect();
        if aktiv.is_none() {
            eintraege.push((usize::MAX, jjcad_core::i18n::tr("gui.dialogs.eigene_belegung").to_string()));
        }
        let gezeigt = aktiv.unwrap_or(usize::MAX);
        let beschriftung = eintraege
            .iter()
            .find(|(i, _)| *i == gezeigt)
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        let zeile_h = keymap_zeile_hoehe(ui);
        // Das Menü wächst mit der Zahl der Layouts (bis DROPDOWN_MAX_H) —
        // vorher zeigte es nur einen kleinen Ausschnitt (T0076).
        let menue_h =
            (eintraege.len() as f32 * (zeile_h + 2.0) + 10.0)
                .min(crate::panels::DROPDOWN_MAX_H);
        let mut feld_flaeche = None;
        if let Some(wahl) = crate::panels::dropdown_mit_menue(
            ui,
            "keymap_layout",
            &beschriftung,
            None,
            zeile_h,
            Some(menue_h),
            &mut feld_flaeche,
            |ui| {
                let mut out = None;
                for (i, name) in &eintraege {
                    if ui.selectable_label(*i == gezeigt, name).clicked() {
                        out = Some(*i);
                    }
                }
                out
            },
        ) {
            if wahl != usize::MAX && Some(wahl) != aktiv {
                layout_wechseln(app, wahl);
            }
        }
        if let Some(r) = feld_flaeche {
            zeile.push((jjcad_core::i18n::tr("gui.dialogs.auswahl").to_string(), r));
        }
        let speichern = zeilen_knopf(ui, jjcad_core::i18n::tr("gui.dialogs.speichern_punkte"))
            .on_hover_text(jjcad_core::i18n::tr("gui.dialogs.die_aktuelle_belegung_unter_einem_namen_ablegen"));
        zeile.push((jjcad_core::i18n::tr("gui.dialogs.speichern").to_string(), speichern.rect));
        if speichern.clicked() {
            // Die eingebauten Vorlagen sind nicht überschreibbar — sie dürfen
            // deshalb nicht als Name im Feld stehen (das war der Grund, warum
            // „Speichern" scheinbar nichts tat).
            app.ui.keymap_name = Some(
                aktiv
                    .filter(|i| *i >= jjcad_core::persist::VORLAGEN.len())
                    .map(|i| app.prefs.keymap_presets[i].name.clone())
                    .unwrap_or_default(),
            );
        }
        let import = zeilen_knopf(ui, jjcad_core::i18n::tr("gui.dialogs.importieren_punkte"))
            .on_hover_text(jjcad_core::i18n::tr("gui.dialogs.ein_layout_aus_einer_datei_uebernehmen"));
        zeile.push((jjcad_core::i18n::tr("gui.dialogs.importieren").to_string(), import.rect));
        if import.clicked() {
            app.open_file_dialog(FileMode::ImportKeymap);
        }
        let export = zeilen_knopf(ui, jjcad_core::i18n::tr("gui.dialogs.exportieren_punkte"))
            .on_hover_text(jjcad_core::i18n::tr("gui.dialogs.die_aktuelle_belegung_als_datei_schreiben"));
        zeile.push((jjcad_core::i18n::tr("gui.dialogs.exportieren").to_string(), export.rect));
        if export.clicked() {
            app.open_file_dialog(FileMode::ExportKeymap);
        }
        // Löschen: das in der Auswahl stehende gespeicherte Layout. Die
        // eingebauten Vorlagen bleiben (T0079).
        let loeschbar = matches!(aktiv, Some(i) if i >= jjcad_core::persist::VORLAGEN.len());
        let loeschen = ui.add_enabled_ui(loeschbar, |ui| zeilen_knopf(ui, jjcad_core::i18n::tr("gui.dialogs.loeschen")));
        zeile.push((jjcad_core::i18n::tr("gui.dialogs.loeschen").to_string(), loeschen.response.rect));
        if loeschen.inner.clicked() {
            if let Some(i) = aktiv {
                if app.prefs.keymap_loeschen(i) {
                    let _ = app.prefs.save();
                    app.toast(jjcad_core::i18n::tr("gui.dialogs.layout_geloescht"));
                }
            }
        }
    });
    app.ui.last_keymap_zeile = Some(zeile);
}

fn keyboard_body(app: &mut App, ui: &mut Ui) {
    let (rahmen, _) = ui.allocate_exact_size(tastatur_flaeche(), Sense::hover());
    let tasten = tasten_rechtecke(rahmen.min);
    app.ui.last_tasten = Some(tasten.clone());
    for (i, (key, label, zeile, spalte, breite, unten)) in TASTEN.iter().enumerate() {
        let label = taste_beschriftung(label);
        let (_, _, rect) = tasten[i];
        let flaechen = taste_flaechen(rahmen.min, key, *zeile, *spalte, *breite, *unten);
        let gesperrt = fest_belegt(key);
        // Gesperrte Tasten zeigen keine Belegung und nehmen keinen Klick an.
        let action = if gesperrt.is_none() {
            app.prefs.action_for_key(key)
        } else {
            None
        };
        let selected = gesperrt.is_none() && app.ui.keymap_selected.as_deref() == Some(key);
        let sense = if gesperrt.is_none() {
            Sense::click()
        } else {
            Sense::hover()
        };
        let resp = ui.interact(rect, ui.id().with(("taste", key)), sense);
        let hovered = resp.hovered();
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            let flaeche = if selected {
                Palette::accent_soft()
            } else if gesperrt.is_some() {
                // Gesperrte Tasten sind eine eigene graue Fläche — nicht nur
                // eine schwächere Schrift. Das Schloss sagt warum.
                gesperrt_flaeche()
            } else if hovered {
                Palette::panel_alt()
            } else {
                Palette::bg()
            };
            let strich = Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected {
                    Palette::accent()
                } else if action.is_some() {
                    Palette::border()
                } else {
                    Palette::border_soft()
                },
            );
            match flaechen.as_slice() {
                // L-förmige Taste: beide Hälften ohne Rundung an der Naht
                // füllen, die Umrandung in einem Zug darum herum.
                [oben, unten_teil] => {
                    p.rect_filled(
                        *oben,
                        Rounding {
                            nw: 6.0,
                            ne: 6.0,
                            sw: 0.0,
                            se: 0.0,
                        },
                        flaeche,
                    );
                    p.rect_filled(
                        *unten_teil,
                        Rounding {
                            nw: 0.0,
                            ne: 0.0,
                            sw: 6.0,
                            se: 6.0,
                        },
                        flaeche,
                    );
                    if let Some(umriss) = taste_umriss(&flaechen) {
                        p.add(egui::Shape::closed_line(umriss, strich));
                    }
                }
                _ => {
                    p.rect_filled(rect, Rounding::same(6.0), flaeche);
                    p.rect_stroke(rect, Rounding::same(6.0), strich);
                }
            }
            p.text(
                Pos2::new(rect.left() + 5.0, rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                label,
                FontId::proportional(9.0),
                if gesperrt.is_some() {
                    gesperrt_text()
                } else {
                    Palette::text_faint()
                },
            );
            if let Some(a) = &action {
                let farbe = if selected { Palette::accent() } else { Palette::text_dim() };
                let ir = Rect::from_center_size(rect.center() - Vec2::new(0.0, 3.0), Vec2::splat(16.0));
                // T0116: Ein Block zeigt auf der Taste seine Miniaturaansicht.
                if let Some((doc, b)) = app.slot_block(a) {
                    crate::inventory::draw_block_preview(p, ir, doc, b, farbe);
                } else if let Some(def) = actions::action(a) {
                    crate::icons::paint(p, ir, def.icon, farbe);
                }
            } else if gesperrt.is_some() {
                crate::icons::paint(
                    p,
                    Rect::from_center_size(rect.center() - Vec2::new(0.0, 3.0), Vec2::splat(14.0)),
                    "lock",
                    gesperrt_text(),
                );
            }
        }
        if resp.clicked() && gesperrt.is_none() {
            // T0116: Läuft das Einfügen eines Blocks, nimmt die angeklickte
            // Taste den Block auf — dieselbe Geste wie ein Hotbar-Slot.
            if !app.bind_key_block(key) {
                // Wie die Hotbar (T0064): Ist eine Funktion gewählt und die
                // Taste frei, übernimmt die Taste sie sofort — der Umweg über
                // die Liste entfällt. Eine belegte Taste wird weiterhin nur
                // ausgewählt.
                match app.ui.keymap_funktion.clone() {
                    Some(f) if action.is_none() => {
                        if !funktion_auf_taste(app, &f, key) {
                            // Fest verdrahtete Funktionen (Strg+S, F1 …) lassen
                            // sich nicht umbelegen; sie bleiben im Inventar
                            // gesperrt.
                            app.toast(jjcad_core::i18n::tr("gui.dialogs.funktion_fest_belegt"));
                        }
                    }
                    _ => {
                        app.ui.keymap_selected = Some(key.to_string());
                        // T0072: Dieselbe Bedienung wie ein leerer Hotbar-Slot —
                        // freie Taste anklicken öffnet das Inventar im Reiter
                        // „Funktionen". Die Liste im Fenster gibt es nicht mehr.
                        if action.is_none() {
                            taste_belegen(app, key);
                        }
                    }
                }
            }
        }
        // Mittlere Maustaste: Belegung dieser Taste entfernen (T0064) — wie bei
        // den Slots der Hotbar.
        if resp.clicked_by(egui::PointerButton::Middle) && gesperrt.is_none() {
            match app.prefs.unbind_key(key) {
                Some(a) => {
                    let _ = app.prefs.save();
                    let name = app.slot_label(&a);
                    app.toast(jjcad_core::i18n::t_args_owned("gui.dialogs.x_taste_x_frei", &[("name", format!("{}", name)), ("wert1", format!("{}", key.to_uppercase()))]));
                }
                None => app.toast(jjcad_core::i18n::tr("gui.dialogs.taste_nicht_belegt")),
            }
        }
        match gesperrt {
            // Hinweis auf den Grund: Ziffern gehören der Hotbar, die übrigen
            // Steuerungstasten dem Ablauf (Esc/Enter/Rücktaste usw.).
            Some(grund) => {
                resp.on_hover_text(grund.grund());
            }
            None => {
                if let Some(a) = action {
                    let text = match actions::action(&a) {
                        Some(d) => format!("{} — {}", d.label(), d.hint()),
                        None => app.slot_label(&a),
                    };
                    resp.on_hover_text(text);
                }
            }
        }
    }

    ui.add_space(8.0);
    ui.separator();
    keyboard_layouts(app, ui);
}

/// Belegen vorbereiten (T0072): öffnet das Inventar im Reiter „Funktionen".
/// Dieselbe Bedienung wie ein leerer Hotbar-Slot — die Funktion wird dort
/// gewählt; `funktion_auf_taste` schreibt sie anschließend auf die Taste.
fn taste_belegen(app: &mut App, key: &str) {
    app.ui.bind_key = Some(key.to_string());
    app.ui.bind_slot = None;
    app.ui.keymap_selected = Some(key.to_string());
    app.ui.inventory_open = true;
    app.ui.inventory_tab = 2;
    app.ui.search.clear();
    app.toast(jjcad_core::i18n::tr("gui.dialogs.funktion_fuer_die_taste_waehlen"));
}

// ---------------------------------------------------------------------------
// Hilfe
// ---------------------------------------------------------------------------

fn help_body(app: &mut App, ui: &mut Ui) {
    ui.label(theme::bold(jjcad_core::i18n::tr("gui.dialogs.bedienung"), 15.0));
    ui.add_space(4.0);
    for (k, v) in [
        (jjcad_core::i18n::tr("gui.dialogs.zeichnen"), jjcad_core::i18n::tr("gui.dialogs.hilfe_zeichnen")),
        (jjcad_core::i18n::tr("gui.dialogs.navigation"), jjcad_core::i18n::tr("gui.dialogs.hilfe_navigation")),
        (jjcad_core::i18n::tr("gui.dialogs.fangen"), jjcad_core::i18n::tr("gui.dialogs.hilfe_fangen")),
        (jjcad_core::i18n::tr("gui.dialogs.eingabe"), jjcad_core::i18n::tr("gui.dialogs.hilfe_eingabe")),
        (jjcad_core::i18n::tr("gui.dialogs.hotbar"), jjcad_core::i18n::tr("gui.dialogs.hilfe_hotbar")),
        (jjcad_core::i18n::tr("gui.dialogs.tasten"), jjcad_core::i18n::tr("gui.dialogs.hilfe_tasten")),
        (jjcad_core::i18n::tr("gui.dialogs.inventar"), jjcad_core::i18n::tr("gui.dialogs.hilfe_inventar")),
        (jjcad_core::i18n::tr("gui.dialogs.auswahl"), jjcad_core::i18n::tr("gui.dialogs.hilfe_auswahl")),
        (jjcad_core::i18n::tr("gui.dialogs.blaetter"), jjcad_core::i18n::tr("gui.dialogs.hilfe_blaetter")),
        (jjcad_core::i18n::tr("gui.dialogs.leistung"), jjcad_core::i18n::tr("gui.dialogs.hilfe_leistung")),
    ] {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(110.0, 18.0), Layout::left_to_right(Align::Center), |ui| {
                ui.label(theme::bold(k, 11.5));
            });
            ui.label(theme::dim(v));
        });
    }
    ui.add_space(10.0);
    ui.label(theme::bold(jjcad_core::i18n::tr("gui.dialogs.tastenbelegung_vorlage_joja_cad"), 15.0));
    ui.add_space(4.0);
    egui::Grid::new("helpkeys")
        .num_columns(2)
        .spacing([18.0, 3.0])
        .show(ui, |ui| {
            for g in [
                actions::ActionGroup::Zeichnen,
                actions::ActionGroup::Architektur,
                actions::ActionGroup::Aendern,
                actions::ActionGroup::Bemassung,
                actions::ActionGroup::Ansicht,
            ] {
                ui.label(
                    RichText::new(g.label().to_uppercase())
                        .size(9.5)
                        .color(Palette::text_faint()),
                );
                ui.label("");
                ui.end_row();
                for a in actions::actions_in_group(g) {
                    if let Some(k) = app.prefs.key_for(a.id) {
                        ui.label(theme::mono(k.to_uppercase()));
                        ui.label(theme::dim(a.label()));
                        ui.end_row();
                    }
                }
            }
        });
    ui.add_space(10.0);
    ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.dialogs.joja_cad_x_x_aktionen_x_bloecke_in_der_bibliothek", &[("wert1", format!("{}", env!("CARGO_PKG_VERSION"))), ("wert2", format!("{}", actions::ACTIONS.len())), ("wert3", format!("{}", app.doc().blocks.len()))])));
    ui.add_space(6.0);
    if ui.button(jjcad_core::i18n::tr("gui.dialogs.ueber_joja_cad")).clicked() {
        app.ui.about_open = true;
    }
}

#[cfg(test)]
mod modaltests {
    //! Regression: modale Fenster müssen im ersten Frame gezeichnet werden.
    //! Vorher startete das Fenster-Flag (`egui::Window::open`) mit `false` —
    //! egui hält den Dialog dann für geschlossen, zeichnet nichts und es bleibt
    //! nur die Abdunklung stehen ("Einfügen dunkelt die App ab").
    use super::*;
    use egui::{Id, LayerId, Order};

    fn leeres_app() -> (App, egui::Context) {
        let ctx = egui::Context::default();
        let app = App::with_ctx(ctx.clone());
        (app, ctx)
    }

    /// Einen Frame ohne echtes Fenster zeichnen — wie im laufenden Programm
    /// wird nur die Dialogschicht aufgerufen.
    fn rahmen(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| show(app, ctx))
    }

    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let _ = rahmen(ctx, app, events);
    }

    /// Mitte der gezeichneten Beschriftung `label` (Knopf eines Dialogs).
    fn text_mitte(out: &egui::FullOutput, label: &str) -> Option<Pos2> {
        out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
    }

    /// Klick mit echten Ereignissen: Bewegen, Drücken, Loslassen.
    fn klick(ctx: &egui::Context, app: &mut App, pos: Pos2) {
        let knopf = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        frame(ctx, app, vec![knopf(true)]);
        frame(ctx, app, vec![knopf(false)]);
        frame(ctx, app, vec![]);
    }

    /// Wird die Fläche (Fenster oder Abdunklung) tatsächlich gezeichnet?
    fn sichtbar(ctx: &egui::Context, order: Order, name: &str) -> bool {
        let layer = LayerId::new(order, Id::new(name));
        ctx.memory(|m| m.areas().is_visible(&layer))
    }

    fn fenster(ctx: &egui::Context, titel: &str) -> bool {
        sichtbar(ctx, Order::Middle, titel)
    }

    fn abdunklung(ctx: &egui::Context) -> bool {
        sichtbar(ctx, Order::Background, "modal_bg")
    }

    #[test]
    fn weitere_module_werden_gezeichnet() {
        let (mut app, ctx) = leeres_app();

        app.ui.block_name_dialog = Some(("Block".into(), false));
        frame(&ctx, &mut app, vec![]);
        assert!(fenster(&ctx, "Block aus Auswahl"));
        app.ui.block_name_dialog = None;

        app.ui.preset_dialog = Some((None, "Vorlage".into()));
        frame(&ctx, &mut app, vec![]);
        assert!(fenster(&ctx, "Hotbar-Vorlage"));
        app.ui.preset_dialog = None;

        // T0073: Das Schließen einer geänderten Datei fragt nach.
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [0.0, 1000.0]}),
            )
            .expect("Linie anlegen");
        app.ui.confirm_close = Some(0);
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        assert!(fenster(&ctx, "Datei schließen"));
        app.ui.confirm_close = None;
        frame(&ctx, &mut app, vec![]);
        assert!(!fenster(&ctx, "Datei schließen"));
        assert!(!abdunklung(&ctx));
    }

    /// Zeichnung mit einer Linie und **ohne** Speicherort — der Fall, in dem
    /// der Schließdialog früher nur „Verwerfen“ anbot (T0143).
    fn app_ohne_speicherort() -> (App, egui::Context) {
        let (mut app, ctx) = leeres_app();
        // Ohne diese Wahl hinge der Test am Sitzungsbus des Rechners: mit
        // D-Bus und Anzeige öffnete er den Systemdialog.
        app.prefs.file_dialog = "intern".into();
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [0.0, 1000.0]}),
            )
            .expect("Linie anlegen");
        assert!(app.doc().path.is_none(), "die Zeichnung hat keinen Speicherort");
        (app, ctx)
    }

    /// T0143: Auch ohne Speicherort steht „Speichern und schließen“ im Dialog.
    #[test]
    fn schliessdialog_bietet_speichern_auch_ohne_speicherort() {
        let (mut app, ctx) = app_ohne_speicherort();
        app.ui.confirm_close = Some(0);
        frame(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        assert!(fenster(&ctx, "Datei schließen"), "der Dialog fehlt");
        assert!(
            text_mitte(&out, "Speichern und schließen").is_some(),
            "der Schließdialog bietet ohne Speicherort kein Speichern an"
        );
    }

    /// T0143: „Speichern und schließen“ ohne Speicherort fragt im Dateidialog
    /// nach dem Ziel; erst nach dem Sichern geht der Reiter zu.
    #[test]
    fn speichern_ohne_speicherort_fragt_nach_dem_ziel() {
        let (mut app, ctx) = app_ohne_speicherort();
        app.ui.confirm_close = Some(0);
        frame(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        let knopf = text_mitte(&out, "Speichern und schließen").expect("kein Speichern-Knopf");
        klick(&ctx, &mut app, knopf);

        assert_eq!(app.ui.confirm_close, None, "der Schließdialog bleibt stehen");
        assert_eq!(app.ui.schliessen_nach_speichern, Some(0), "der Reiter ist nicht gemerkt");
        let fd = app
            .ui
            .file_dialog
            .as_ref()
            .expect("der Dateidialog muss nach dem Ziel fragen");
        assert_eq!(fd.mode, FileMode::SaveJjcad, "der Dialog speichert nicht");
        assert_eq!(app.tabs.len(), 1, "noch ist der Reiter offen");

        let ziel = std::env::temp_dir().join(format!("jjcad-t0143-{}.jjcad", std::process::id()));
        let _ = std::fs::remove_file(&ziel);
        app.ui.file_dialog = None;
        datei_gewaehlt(&mut app, FileMode::SaveJjcad, ziel.clone());
        assert!(ziel.is_file(), "die Zeichnung wurde nicht geschrieben");
        assert_eq!(app.ui.schliessen_nach_speichern, None);
        assert!(
            app.doc().entities.is_empty(),
            "der Reiter ist zu — es steht die leere Zeichnung"
        );
        let _ = std::fs::remove_file(&ziel);
    }

    /// T0143: Der ganze Weg im GUI — Schließdialog, Zielabfrage im
    /// eingebauten Dateidialog, Sichern, Reiter zu.
    #[test]
    fn speichern_und_schliessen_ueber_den_dateidialog_schliesst_den_reiter() {
        let (mut app, ctx) = app_ohne_speicherort();
        let ziel_dir = std::env::temp_dir().join(format!("jjcad-t0143-dlg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ziel_dir);
        std::fs::create_dir_all(&ziel_dir).unwrap();

        app.ui.confirm_close = Some(0);
        frame(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        let knopf = text_mitte(&out, "Speichern und schließen").expect("kein Speichern-Knopf");
        klick(&ctx, &mut app, knopf);

        // Der eingebaute Dateidialog zeigt auf einen Wegwerfordner; der
        // Vorschlagsname „Unbenannt“ bleibt.
        {
            let fd = app.ui.file_dialog.as_mut().expect("Dateidialog fehlt");
            fd.dir = ziel_dir.clone();
            fd.name = "Plan.jjcad".into();
        }
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![]);
        }
        let out = rahmen(&ctx, &mut app, vec![]);
        let sichern = text_mitte(&out, "Speichern").expect("kein Speichern-Knopf im Dateidialog");
        klick(&ctx, &mut app, sichern);

        assert!(
            ziel_dir.join("Plan.jjcad").is_file(),
            "die Zeichnung wurde nicht geschrieben"
        );
        assert!(app.ui.file_dialog.is_none(), "der Dateidialog bleibt offen");
        assert!(
            app.doc().entities.is_empty(),
            "der Reiter ist nach dem Sichern nicht zu"
        );
        let _ = std::fs::remove_dir_all(&ziel_dir);
    }

    /// T0143: Bricht der Nutzer den Dateidialog ab, bleibt der Reiter offen —
    /// der Merker darf das nächste Speichern nicht treffen.
    #[test]
    fn abbruch_des_dateidialogs_laesst_den_reiter_offen() {
        let (mut app, ctx) = app_ohne_speicherort();
        app.ui.confirm_close = Some(0);
        frame(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        let knopf = text_mitte(&out, "Speichern und schließen").expect("kein Speichern-Knopf");
        klick(&ctx, &mut app, knopf);
        assert!(app.ui.file_dialog.is_some(), "der Dateidialog fehlt");

        // Abbruch: der Dialog ist weg, ohne dass ein Ziel gewählt wurde.
        app.ui.file_dialog = None;
        frame(&ctx, &mut app, vec![]);
        assert_eq!(
            app.ui.schliessen_nach_speichern, None,
            "der Merker bleibt nach dem Abbruch stehen"
        );
        assert_eq!(app.doc().entities.len(), 1, "der Reiter wurde trotzdem geschlossen");
    }
}

#[cfg(test)]
pub(crate) mod tests_tastatur {
    //! T0045: Das Tastenbild ist eine vollständige deutsche Tastatur, und die
    //! fest vergebenen Tasten (Hotbar-Ziffern, Steuerungstasten) sind ausgegraut
    //! und lassen sich nicht belegen.
    //!
    //! Geprüft wird jeweils das Bild bzw. der gezeichnete Zustand, nicht eine
    //! im Test nachgebaute Rechnung: die Rechtecke kommen aus
    //! `tasten_rechtecke` (dieselbe Quelle wie das Zeichnen), die Klicks laufen
    //! als echte egui-Ereignisse durch den Dialog.
    use super::*;

    /// Bildzähler: egui blendet Fenster ein. Ohne laufende Zeit bleiben die
    /// Farben mit Teil-Alpha stehen — der Vergleich mit der Palette bräuchte
    /// dann eine eigene Rechnung.
    ///
    /// Der Zähler ist **je Testfaden** (alle Tests laufen in eigenen Fäden):
    /// ein prozessweiter Zähler lief im Parallelauf so schnell weiter, dass
    /// zwischen Drücken und Loslassen eines synthetischen Klicks mehr als
    /// egui's `max_click_duration` (0,8 s) lag — der Klick wurde dann
    /// stillschweigend verworfen und die Klicktests im Tastenbild fielen
    /// zufällig aus.
    fn naechste_zeit() -> f64 {
        use std::cell::Cell;
        thread_local! {
            static ZAEHLER: Cell<u64> = const { Cell::new(0) };
        }
        let n = ZAEHLER.with(|z| {
            let n = z.get() + 1;
            z.set(n);
            n
        });
        n as f64 * 0.25
    }

    fn rahmen(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(naechste_zeit()),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| show(app, ctx))
    }

    /// Ein Bild der **ganzen** Anwendung (nicht nur des Fensters) — nötig,
    /// sobald eine Taste ausgewertet werden soll (`App::handle_shortcuts`).
    fn app_rahmen(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(naechste_zeit()),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    fn app_mit_tastatur() -> (App, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        // `App::with_ctx` liest die `prefs.json` **des Rechners**. Die Tests dieser
        // Gruppe prüfen die Werkseinstellung (eingebaute Vorlagen, feste Belegung);
        // steht beim Nutzer „Eigene Belegung“ (`keymap_aktiv = None`) oder ein
        // eigenes Layout, schlugen sie fehl — und `ship-card.sh` rollte `master`
        // zurück (T0083). Deshalb hier die Vorgaben selbst setzen.
        app.prefs = jjcad_core::persist::UserPrefs::default();
        app.ui.keymap_open = true;
        // mehrere Bilder: das Fenster setzt Größe und Lage erst nach dem
        // ersten Aufbau (sonst liegen die Tastenrechtecke noch falsch)
        for _ in 0..4 {
            rahmen(&ctx, &mut app, vec![]);
        }
        (app, ctx)
    }

    fn taste_mit<'a>(
        tasten: &'a [(&'static str, &'static str, Rect)],
        key: &str,
    ) -> &'a (&'static str, &'static str, Rect) {
        tasten
            .iter()
            .find(|(k, _, _)| *k == key)
            .unwrap_or_else(|| panic!("Taste {key} fehlt im Tastenbild"))
    }

    /// Klick mit echten Ereignissen: Bewegen, Drücken, Loslassen.
    fn klick(ctx: &egui::Context, app: &mut App, pos: Pos2) {
        let knopf = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        rahmen(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        rahmen(ctx, app, vec![knopf(true)]);
        rahmen(ctx, app, vec![knopf(false)]);
        rahmen(ctx, app, vec![]);
    }

    /// `UserPrefs::save` folgt `XDG_CONFIG_HOME`. Die Tests unten belegen echte
    /// Tasten und schreiben damit Voreinstellungen — ein Wegwerf-Verzeichnis
    /// hält `~/.config/jjcad/prefs.json` des Nutzers unberührt. Die Sperre
    /// serialisiert diese Tests, sonst überschreibt ein zweiter Test den
    /// Umweg des ersten wieder mit dem echten Verzeichnis.
    pub(crate) static KONFIG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) struct WegwerfKonfig(Option<std::ffi::OsString>, std::path::PathBuf);

    impl WegwerfKonfig {
        pub(crate) fn neu(name: &str) -> Self {
            let alt = std::env::var_os("XDG_CONFIG_HOME");
            let dir = std::env::temp_dir().join(format!(
                "jjcad-t0064-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            Self(alt, dir)
        }
    }

    impl Drop for WegwerfKonfig {
        fn drop(&mut self) {
            match self.0.take() {
                Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            let _ = std::fs::remove_dir_all(&self.1);
        }
    }

    /// Die Tastaturbelegung dunkelt den Hintergrund nicht mehr ab (T0064) —
    /// das Ribbon muss bedienbar bleiben, sonst lässt sich keine Funktion mehr
    /// anklicken.
    #[test]
    fn tastaturbelegung_dunkelt_nicht_ab() {
        use egui::{Id, LayerId};
        let (_app, ctx) = app_mit_tastatur();
        let layer = |order, name| LayerId::new(order, Id::new(name));
        assert!(
            ctx.memory(|m| m.areas().is_visible(&layer(egui::Order::Middle, "Tastaturbelegung"))),
            "das Fenster der Tastaturbelegung wird nicht gezeichnet"
        );
        assert!(
            !ctx.memory(|m| m.areas().is_visible(&layer(egui::Order::Background, "modal_bg"))),
            "die Tastaturbelegung dunkelt den Hintergrund weiterhin ab"
        );
    }

    /// T0064: Eine Funktion im Ribbon anklicken, dann eine leere Taste — die
    /// Taste übernimmt sie sofort, genau wie bei der Hotbar.
    #[test]
    fn leere_taste_uebernimmt_die_funktion_aus_dem_ribbon() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("belegen");
        let (mut app, ctx) = app_mit_tastatur();
        assert!(
            app.prefs.action_for_key(",").is_none(),
            "Testtaste Komma ist in der Vorlage JoJa-CAD belegt"
        );

        // Ribbon-Knopf „Wand" — derselbe Weg wie der Klick im Ribbon.
        app.run_action(&ctx, "draw.wall");
        assert_eq!(
            app.ui.keymap_funktion.as_deref(),
            Some("draw.wall"),
            "die gewählte Funktion wird nicht gemerkt"
        );

        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, t) = *taste_mit(&tasten, ",");
        klick(&ctx, &mut app, t.center());

        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die leere Taste hat die Funktion nicht übernommen"
        );
        assert_eq!(app.ui.keymap_selected.as_deref(), Some(","));
        assert!(app.ui.keymap_funktion.is_none(), "die Funktion bleibt gemerkt");
        assert!(app.tool.is_none(), "das Werkzeug läuft nach dem Belegen weiter");

        // Der Beleg steht in der Voreinstellungsdatei, nicht nur im Speicher.
        let gespeichert = jjcad_core::persist::UserPrefs::load();
        assert_eq!(
            gespeichert.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die Belegung wurde nicht gespeichert"
        );
    }

    /// Eine bereits belegte Taste behält ihre Funktion und wird nur ausgewählt —
    /// sonst überschriebe jeder Klick im Tastenbild eine fremde Belegung.
    #[test]
    fn belegte_taste_behaelt_ihre_funktion() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("belegt");
        let (mut app, ctx) = app_mit_tastatur();
        let vorher = app.prefs.action_for_key("g").expect("G ist in der Vorlage JoJa-CAD belegt");

        app.run_action(&ctx, "draw.door");
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, g) = *taste_mit(&tasten, "g");
        klick(&ctx, &mut app, g.center());

        assert_eq!(app.prefs.action_for_key("g"), Some(vorher), "belegte Taste wurde überschrieben");
        assert_eq!(app.ui.keymap_selected.as_deref(), Some("g"), "belegte Taste ist nicht ausgewählt");
    }

    /// Fest verdrahtete Funktionen (Strg+S, F1 …) lassen sich auch über den
    /// Klickweg nicht umbelegen — die Taste bleibt dann leer.
    #[test]
    fn fest_verdrahtete_funktion_landet_nicht_auf_einer_taste() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("fest");
        let (mut app, ctx) = app_mit_tastatur();
        app.ui.keymap_funktion = Some("file.save".to_string());

        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, t) = *taste_mit(&tasten, ",");
        klick(&ctx, &mut app, t.center());

        assert!(
            app.prefs.action_for_key(",").is_none(),
            "eine fest verdrahtete Funktion wurde auf eine Taste gelegt"
        );
        assert_eq!(
            app.prefs.key_for("file.save").as_deref(),
            Some("ctrl+s"),
            "die feste Belegung der Funktion wurde verändert"
        );
    }

    /// Ist die Tastaturbelegung zu, bleibt kein Merker zurück: ein späterer
    /// Klick auf eine leere Taste legt sonst eine vergessene Funktion ab.
    #[test]
    fn geschlossene_tastaturbelegung_vergisst_die_funktion() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("zu");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.keymap_open = true;
        app.run_action(&ctx, "draw.wall");
        assert!(app.ui.keymap_funktion.is_some());
        app.ui.keymap_open = false;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(naechste_zeit()),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
        assert!(
            app.ui.keymap_funktion.is_none(),
            "der Merker überlebt das Schließen der Tastaturbelegung"
        );
    }

    /// Klick mit der **mittleren** Maustaste (T0064: Belegung entfernen).
    fn klick_mittel(ctx: &egui::Context, app: &mut App, pos: Pos2) {
        let knopf = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Middle,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        rahmen(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        rahmen(ctx, app, vec![knopf(true)]);
        rahmen(ctx, app, vec![knopf(false)]);
        rahmen(ctx, app, vec![]);
    }

    /// T0064: Die mittlere Maustaste leert eine Taste — wie bei den Slots der
    /// Hotbar. Die Funktion selbst bleibt erhalten.
    #[test]
    fn mittlere_maustaste_entfernt_die_belegung() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("mitte");
        let (mut app, ctx) = app_mit_tastatur();
        let vorher = app.prefs.action_for_key("g").expect("G ist im Werkssatz belegt");

        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, g) = *taste_mit(&tasten, "g");
        klick_mittel(&ctx, &mut app, g.center());

        assert_eq!(
            app.prefs.action_for_key("g"),
            None,
            "die mittlere Maustaste hat die Taste nicht geleert"
        );
        assert_eq!(
            app.prefs.key_for(vorher.as_str()).as_deref(),
            None,
            "die Funktion darf nur ihre Taste verlieren, nicht selbst verschwinden"
        );
        assert_eq!(
            app.prefs.action_for_key(","),
            None,
            "eine fremde Taste wurde mitgeleert"
        );
        // Gesperrte Tasten lassen sich nicht leeren.
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, eins) = *taste_mit(&tasten, "1");
        klick_mittel(&ctx, &mut app, eins.center());
        assert!(app.prefs.unbind_key("1").is_none(), "Ziffer 1 war nie belegt");
    }

    /// T0064: Eine Funktion darf auf mehreren Tasten liegen — zweimal dieselbe
    /// Funktion wählen, zweimal eine freie Taste anklicken.
    #[test]
    fn funktion_laesst_sich_mehrfach_platzieren() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("mehrfach");
        let (mut app, ctx) = app_mit_tastatur();
        let frei = [",", "ä"];
        for key in frei {
            assert!(
                app.prefs.action_for_key(key).is_none(),
                "Testtaste {key} ist belegt: {:?}",
                app.prefs.action_for_key(key)
            );
            app.run_action(&ctx, "draw.wall");
            let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
            let (_, _, r) = *taste_mit(&tasten, key);
            klick(&ctx, &mut app, r.center());
            assert_eq!(
                app.prefs.action_for_key(key).as_deref(),
                Some("draw.wall"),
                "Taste {key} hat die Funktion nicht übernommen"
            );
        }
        assert_eq!(
            app.prefs.keys_for("draw.wall"),
            &["g".to_string(), ",".to_string(), "ä".to_string()],
            "die Funktion muss auf allen drei Tasten liegen"
        );
        // Die Werkstaste G bleibt erhalten, die Auswertung findet beide.
        assert_eq!(app.prefs.key_for("draw.wall").as_deref(), Some("g"));
    }

    /// T0064: Der lange Hinweistext steht nicht mehr im Fenster — er hängt als
    /// Tooltip am kleinen „i" über dem Tastenbild.
    #[test]
    fn bedienung_steht_als_tooltip_am_info_symbol() {
        let (mut app, ctx) = app_mit_tastatur();
        let out = rahmen(&ctx, &mut app, vec![]);
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        assert!(
            !texte.iter().any(|t| t.starts_with("Taste anklicken, dann Funktion")),
            "der lange Hinweistext steht noch im Fenster"
        );
        assert!(
            !texte.iter().any(|t| t.contains("Noch keine Funktion gewählt")),
            "der Hinweis auf die fehlende Funktion steht noch im Fenster"
        );
        // Der Tooltip erklärt die Bedienung vollständig.
        for wort in ["Ribbon", "Mittlere Maustaste", "mehreren Tasten", "Hotbar"] {
            assert!(
                keymap_hilfe().contains(wort),
                "der Tooltip nennt „{wort}“ nicht"
            );
        }
    }

    #[test]
    fn bild_ist_eine_vollstaendige_deutsche_tastatur() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        for key in [
            "+", "#", "<", "shift_r", "ctrl_r", "altgr", "enter", "ü", "ö", "ä", ",", ".", "-",
        ] {
            taste_mit(&tasten, key);
        }
        // Jede Taste genau einmal — ein doppelter Eintrag würde eine Taste
        // überdecken und wäre im Bild nicht zu sehen.
        let mut keys: Vec<&str> = tasten.iter().map(|(k, _, _)| *k).collect();
        let anzahl = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), anzahl, "Tastenbild enthält eine Taste doppelt");
    }

    #[test]
    fn tasten_stehen_im_raster_ohne_ueberlappung() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let flaeche = Rect::from_min_size(tasten[0].2.min, tastatur_flaeche());
        for (key, _, rect) in &tasten {
            assert!(
                flaeche.contains_rect(*rect),
                "Taste {key} liegt außerhalb des Tastenbilds"
            );
        }
        // Belegt sind die Flächen der Taste, nicht ihre umschließende Box —
        // das L-förmige Enter deckt die Ecke von `#` sonst nur scheinbar ab.
        let teile = tasten_belegung(tasten[0].2.min);
        for (i, (ka, a)) in teile.iter().enumerate() {
            for (kb, b) in teile.iter().skip(i + 1) {
                let schnitt = a.intersect(*b);
                assert!(
                    schnitt.width() < 0.5 || schnitt.height() < 0.5,
                    "Tasten {ka} und {kb} überlappen sich"
                );
            }
        }
    }

    /// Die Enter-Taste sitzt am rechten Rand über der QWERTZ- und der
    /// Grundreihe — dort, wo die Enter-Taste einer deutschen Tastatur auch
    /// sitzt. Den Platz dafür gibt die Plus-Taste ab: sie ist nur noch eine
    /// Einheit breit, der Rest der Reihe gehört dem Enter. Die untere Hälfte
    /// ist schmaler (L-Form), damit `#` eine ganze Taste behält.
    #[test]
    fn enter_steht_rechts_ueber_zwei_reihen() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let rechts = tasten[0].2.left() + tastatur_flaeche().x;
        let (_, _, enter) = *taste_mit(&tasten, "enter");
        let (_, _, raute) = *taste_mit(&tasten, "#");
        let (_, _, plus) = *taste_mit(&tasten, "+");
        let teile = taste_flaechen(tasten[0].2.min, "enter", 1, 13.5, 1.5, 1.0);
        assert_eq!(teile.len(), 2, "Enter ist nicht L-förmig");
        let (oben, unten) = (teile[0], teile[1]);
        assert!(
            (enter.right() - rechts).abs() < 0.5,
            "Enter endet bei {}, der rechte Rand liegt bei {rechts}",
            enter.right()
        );
        assert!(
            (enter.height() - (2.0 * KEY_H + KEY_GAP)).abs() < 0.5,
            "Enter ist {} px hoch, erwartet zwei Reihen",
            enter.height()
        );
        assert!(
            (oben.width() - (1.5 * KEY_W + 0.5 * KEY_GAP)).abs() < 0.5,
            "obere Hälfte des Enter ist {} px breit, erwartet 1,5 Tasten",
            oben.width()
        );
        assert!(
            (unten.left() - (raute.right() + KEY_GAP)).abs() < 0.5,
            "untere Hälfte des Enter steht nicht direkt rechts neben # ({} zu {})",
            unten.left(),
            raute.right()
        );
        assert!(
            (unten.right() - rechts).abs() < 0.5 && (oben.right() - rechts).abs() < 0.5,
            "Enter schließt nicht bündig am rechten Rand"
        );
        assert!(
            (enter.top() - plus.top()).abs() < 0.5,
            "Enter beginnt nicht in der Reihe der Plus-Taste"
        );
        assert!(
            (raute.top() - (enter.top() + KEY_H + KEY_GAP)).abs() < 0.5
                && (raute.bottom() - enter.bottom()).abs() < 0.5,
            "# steht nicht in der zweiten Zeile des Enter"
        );
        // Der rechte Umschalt liegt unter dem Enter und beginnt an dessen Naht.
        let (_, _, shift_r) = *taste_mit(&tasten, "shift_r");
        assert!(
            (shift_r.top() - (enter.bottom() + KEY_GAP)).abs() < 0.5,
            "rechter Umschalt sitzt nicht unter dem Enter"
        );
    }

    /// Die Plus-Taste ist eine normale, rechteckige Taste — kein breiter Klotz
    /// mehr; der frei gewordene Platz gehört dem Enter.
    #[test]
    fn plus_taste_ist_eine_normale_taste() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, plus) = *taste_mit(&tasten, "+");
        let (_, _, ue) = *taste_mit(&tasten, "ü");
        let (_, _, enter) = *taste_mit(&tasten, "enter");
        assert!(
            (plus.width() - KEY_W).abs() < 0.5,
            "Plus-Taste ist {} px breit, erwartet eine Tasteneinheit ({KEY_W} px)",
            plus.width()
        );
        assert!(
            (plus.left() - (ue.right() + KEY_GAP)).abs() < 0.5,
            "Plus-Taste schließt nicht an Ü an"
        );
        assert!(
            (enter.left() - (plus.right() + KEY_GAP)).abs() < 0.5,
            "Plus-Taste endet nicht direkt vor dem Enter"
        );
    }

    /// Die Akut-Taste (`´`) fehlte in der Ziffernreihe — sie steht zwischen der
    /// Null und der Rücktaste.
    #[test]
    fn akut_taste_steht_zwischen_null_und_ruecktaste() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, beschriftung, akut) = *taste_mit(&tasten, "´");
        let (_, _, null) = *taste_mit(&tasten, "0");
        let (_, _, bksp) = *taste_mit(&tasten, "backspace");
        assert_eq!(beschriftung, "´", "Akut-Taste trägt die falsche Beschriftung");
        assert!(
            (akut.width() - KEY_W).abs() < 0.5,
            "Akut-Taste ist {} px breit, erwartet eine Tasteneinheit",
            akut.width()
        );
        assert!(
            (akut.left() - (null.right() + KEY_GAP)).abs() < 0.5,
            "Akut-Taste schließt nicht an die Null an"
        );
        assert!(
            (akut.right() + KEY_GAP - bksp.left()).abs() < 0.5,
            "Akut-Taste endet nicht direkt vor der Rücktaste"
        );
        assert!(
            (akut.top() - null.top()).abs() < 0.5,
            "Akut-Taste steht nicht in der Ziffernreihe"
        );
    }

    /// Der rechte Umschalt heißt `R-Shift` und reicht bis zum rechten Rand.
    #[test]
    fn rechter_umschalt_heisst_r_shift_und_endet_am_rand() {
        let (mut app, ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, beschriftung, shift_r) = *taste_mit(&tasten, "shift_r");
        let rechts = tasten[0].2.left() + tastatur_flaeche().x;
        assert_eq!(beschriftung, "R-Shift", "rechter Umschalt trägt den falschen Namen");
        assert!(
            (shift_r.right() - rechts).abs() < 0.5,
            "R-Shift endet bei {}, der rechte Rand liegt bei {rechts}",
            shift_r.right()
        );
        let (_, _, strg) = *taste_mit(&tasten, "ctrl_r");
        assert!(
            shift_r.width() > strg.width(),
            "R-Shift ist nicht breiter als die rechte Strg-Taste"
        );
        // Die Beschriftung wird auch gezeichnet, nicht nur geführt.
        let out = rahmen(&ctx, &mut app, vec![]);
        assert!(
            out.shapes.iter().any(|cs| matches!(&cs.shape,
                egui::Shape::Text(t) if t.galley.text() == "R-Shift")),
            "R-Shift steht nicht im gezeichneten Tastenbild"
        );
    }

    #[test]
    fn gesperrte_tasten_nehmen_keinen_klick_an() {
        let (mut app, ctx) = app_mit_tastatur();
        for key in [
            "1", "2", "5", "7", "0", "esc", "tab", "caps", "shift", "shift_r", "ctrl", "ctrl_r",
            "enter", "backspace",
        ] {
            let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
            let (_, _, rect) = *taste_mit(&tasten, key);
            klick(&ctx, &mut app, rect.center());
            assert!(
                app.ui.keymap_selected.is_none(),
                "Taste {key} ist gesperrt, wurde aber ausgewählt"
            );
        }
        // Gegenprobe: eine freie Taste lässt sich weiterhin wählen.
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, q) = *taste_mit(&tasten, "q");
        klick(&ctx, &mut app, q.center());
        assert_eq!(
            app.ui.keymap_selected.as_deref(),
            Some("q"),
            "freie Taste lässt sich nicht mehr wählen"
        );
    }

    /// Gesperrte Tasten sind im Bild grauer als freie — die Ausgrauung ist eine
    /// Eigenschaft der gezeichneten Beschriftung, nicht nur der Logik.
    #[test]
    fn gesperrte_tasten_sind_ausgegraut() {
        let (mut app, ctx) = app_mit_tastatur();
        let out = rahmen(&ctx, &mut app, vec![]);
        let farbe = |text: &str| {
            out.shapes.iter().find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == text => Some(t.fallback_color),
                _ => None,
            })
        };
        assert_eq!(farbe("7"), Some(gesperrt_text()), "Ziffer 7 ist nicht ausgegraut");
        assert_eq!(farbe("Esc"), Some(gesperrt_text()), "Esc ist nicht ausgegraut");
        assert_eq!(
            farbe("Q"),
            Some(Palette::text_faint()),
            "freie Taste hat die Farbe einer gesperrten Taste"
        );
    }

    /// Die Taste `<` gehört links neben Y — sie fehlte im Tastenbild.
    #[test]
    fn taste_kleiner_als_steht_links_neben_y() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, y) = *taste_mit(&tasten, "y");
        let (_, _, kleiner) = *taste_mit(&tasten, "<");
        let (_, _, shift) = *taste_mit(&tasten, "shift");
        assert!(
            (kleiner.right() + KEY_GAP - y.left()).abs() < 0.5,
            "`<` endet bei {}, Y beginnt bei {}",
            kleiner.right(),
            y.left()
        );
        assert!(
            (kleiner.top() - y.top()).abs() < 0.5 && (kleiner.height() - KEY_H).abs() < 0.5,
            "`<` steht nicht in der Reihe von Y"
        );
        assert!(
            (kleiner.left() - (shift.right() + KEY_GAP)).abs() < 0.5,
            "`<` schließt nicht an den linken Umschalt an"
        );
    }

    /// Die Grundreihe (a s d …) ist gegenüber der QWERTZ-Reihe um eine halbe
    /// Taste versetzt — sonst stehen die Tasten genau untereinander.
    #[test]
    fn grundreihe_ist_um_eine_halbe_taste_versetzt() {
        let (app, _ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        for (oben, unten) in [("q", "a"), ("w", "s"), ("e", "d"), ("p", "ö")] {
            let (_, _, o) = *taste_mit(&tasten, oben);
            let (_, _, u) = *taste_mit(&tasten, unten);
            let versatz = u.left() - o.left();
            assert!(
                (versatz - 0.5 * (KEY_W + KEY_GAP)).abs() < 0.5,
                "{unten} steht {versatz} px neben {oben}, erwartet eine halbe Taste ({} px)",
                0.5 * (KEY_W + KEY_GAP)
            );
        }
    }

    /// Gesperrte Tasten haben eine eigene Fläche, nicht nur eine schwächere
    /// Beschriftung — der Unterschied ist im gezeichneten Bild messbar.
    #[test]
    fn gesperrte_tasten_haben_eigene_flaeche() {
        let (mut app, ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let out = rahmen(&ctx, &mut app, vec![]);
        let flaeche = |gesucht: Rect| {
            out.shapes.iter().find_map(|cs| match &cs.shape {
                egui::Shape::Rect(rs)
                    if (rs.rect.min - gesucht.min).length() < 0.5
                        && (rs.rect.max - gesucht.max).length() < 0.5 =>
                {
                    Some(rs.fill)
                }
                _ => None,
            })
        };
        let (_, _, sieben) = *taste_mit(&tasten, "7");
        let (_, _, q) = *taste_mit(&tasten, "q");
        let (_, _, esc) = *taste_mit(&tasten, "esc");
        assert_eq!(
            flaeche(sieben),
            Some(gesperrt_flaeche()),
            "Ziffer 7 hat keine eigene Fläche"
        );
        assert_eq!(flaeche(esc), Some(gesperrt_flaeche()), "Esc hat keine eigene Fläche");
        assert_eq!(flaeche(q), Some(Palette::bg()), "freie Taste hat eine fremde Fläche");
        // Der Unterschied ist auch als Helligkeit messbar, nicht nur im Namen.
        // Der Betrag gilt für beide Designs: im dunklen ist die Sperrfläche
        // heller, im hellen dunkler als die freie Taste (T0102).
        let hell = |c: Color32| c.r() as i32 + c.g() as i32 + c.b() as i32;
        assert!(
            (hell(gesperrt_flaeche()) - hell(Palette::bg())).abs() > 40,
            "die graue Fläche hebt sich zu wenig von der freien Taste ab"
        );
    }

    /// T0072: Eine freie Taste öffnet das Inventar im Reiter „Funktionen" —
    /// dieselbe Bedienung wie ein leerer Hotbar-Slot. Die eigene Liste im
    /// Fenster gibt es nicht mehr; die Auswahl im Inventar belegt die Taste.
    #[test]
    fn freie_taste_oeffnet_das_inventar_und_die_auswahl_belegt_sie() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("inventar");
        let (mut app, ctx) = app_mit_tastatur();
        assert!(
            app.prefs.action_for_key(",").is_none(),
            "Testtaste Komma ist in der Vorlage JoJa-CAD belegt"
        );

        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, t) = *taste_mit(&tasten, ",");
        klick(&ctx, &mut app, t.center());

        assert_eq!(app.ui.keymap_selected.as_deref(), Some(","));
        assert_eq!(
            app.ui.bind_key.as_deref(),
            Some(","),
            "die Taste wartet nicht auf eine Funktion aus dem Inventar"
        );
        assert!(app.ui.inventory_open, "das Inventar wurde nicht geöffnet");
        assert_eq!(
            app.ui.inventory_tab, 2,
            "das Inventar steht nicht im Reiter Funktionen"
        );

        // Der Klick auf eine Zeile im Reiter „Funktionen" läuft über diesen Weg.
        crate::inventory::funktion_gewaehlt(&mut app, "draw.wall");

        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die gewählte Funktion liegt nicht auf der Taste"
        );
        assert!(app.ui.bind_key.is_none(), "das Belegen bleibt offen");
        assert!(app.tool.is_none(), "das Werkzeug läuft nach dem Belegen weiter");
        let gespeichert = jjcad_core::persist::UserPrefs::load();
        assert_eq!(
            gespeichert.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die Belegung wurde nicht gespeichert"
        );
    }

    /// T0116: Läuft das Einfügen eines Blocks, nimmt die angeklickte Taste den
    /// Block auf — dieselbe Geste wie ein Hotbar-Slot. Die Taste wartet dabei
    /// **nicht** auf eine Funktion aus dem Inventar.
    #[test]
    fn taste_nimmt_den_laufenden_block_auf() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("t0116-taste-block");
        let (mut app, ctx) = app_mit_tastatur();
        let block = app
            .doc()
            .blocks
            .first()
            .map(|b| b.id.clone())
            .expect("die Zeichnung bringt Systemblöcke mit");
        app.start_ghost_block(block.clone());
        rahmen(&ctx, &mut app, vec![]);

        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, t) = *taste_mit(&tasten, ",");
        klick(&ctx, &mut app, t.center());

        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some(format!("block:{block}").as_str()),
            "die Taste hat den Block nicht aufgenommen"
        );
        assert!(app.ghost.is_none(), "nach dem Belegen läuft das Einfügen weiter");
        assert!(
            app.ui.bind_key.is_none() && !app.ui.inventory_open,
            "die Taste öffnete das Inventar statt den Block zu belegen"
        );
        assert_eq!(app.ui.keymap_selected.as_deref(), Some(","));
        let gespeichert = jjcad_core::persist::UserPrefs::load();
        assert_eq!(
            gespeichert.action_for_key(",").as_deref(),
            Some(format!("block:{block}").as_str()),
            "die Belegung wurde nicht gespeichert"
        );
    }

    /// T0116: Die so belegte Taste startet die Einfüge-Vorschau des Blocks —
    /// über denselben Weg wie jede andere Funktion (`action_for_key` →
    /// `App::run_action`).
    #[test]
    fn taste_mit_block_startet_die_einfuege_vorschau() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("t0116-taste-block-start");
        let (mut app, ctx) = app_mit_tastatur();
        let block = app
            .doc()
            .blocks
            .first()
            .map(|b| b.id.clone())
            .expect("die Zeichnung bringt Systemblöcke vor");
        app.prefs.bind(&format!("block:{block}"), ",");
        app.ui.keymap_open = false;

        let taste = |pressed| egui::Event::Key {
            key: egui::Key::Comma,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        // Tastendrücke wertet `App::ui` aus, nicht das Fenster allein —
        // deshalb hier ein voller Durchgang statt `rahmen`.
        for _ in 0..2 {
            app_rahmen(&ctx, &mut app, vec![]);
        }
        app_rahmen(&ctx, &mut app, vec![taste(true)]);
        app_rahmen(&ctx, &mut app, vec![taste(false)]);

        assert_eq!(
            app.block_einfuegen(),
            Some(block.as_str()),
            "die Taste startet nicht die Einfüge-Vorschau des Blocks"
        );
    }

    /// T0072: Fest verdrahtete Funktionen (Strg+S, F1 …) lassen sich auch aus
    /// dem Inventar nicht auf eine Taste legen.
    #[test]
    fn feste_funktion_aus_dem_inventar_belegt_keine_taste() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("inventar-fest");
        let (mut app, ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, t) = *taste_mit(&tasten, ",");
        klick(&ctx, &mut app, t.center());
        assert_eq!(app.ui.bind_key.as_deref(), Some(","));

        crate::inventory::funktion_gewaehlt(&mut app, "file.save");

        assert!(
            app.prefs.action_for_key(",").is_none(),
            "eine fest verdrahtete Funktion wurde auf eine Taste gelegt"
        );
        assert_eq!(
            app.prefs.key_for("file.save").as_deref(),
            Some("ctrl+s"),
            "die feste Belegung der Funktion wurde verändert"
        );
    }

    /// T0074: Die Funktionsliste im Inventar stand nur für **eine** Zuteilung
    /// offen — nach dem Zuweisen schließt sie sich (Hotbar-Slot, Taste, Ribbon).
    /// Ohne Zuteilung (fest verdrahtete Funktion) und beim Füllen des Lagers
    /// bleibt sie offen.
    #[test]
    fn funktionsauswahl_schliesst_sich_nach_dem_zuteilen() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("t0074-zuteilen");
        let (mut app, _ctx) = app_mit_tastatur();

        // Ein leerer Hotbar-Slot wurde angeklickt: die Liste steht im Reiter
        // „Funktionen" und wartet auf ein Element für Slot 3.
        app.ui.bind_key = None;
        app.ui.bind_slot = Some(3);
        app.ui.inventory_open = true;
        app.ui.inventory_tab = 2;
        crate::inventory::funktion_gewaehlt(&mut app, "draw.line");

        assert_eq!(
            app.prefs.slots().get(3).cloned().flatten().as_deref(),
            Some("draw.line"),
            "die Funktion wurde nicht auf den Slot gelegt"
        );
        assert!(
            !app.ui.inventory_open,
            "die Funktionsliste bleibt nach dem Zuteilen auf den Slot offen"
        );
        assert!(app.ui.bind_slot.is_none(), "der Slot wartet weiter auf ein Element");

        // Dasselbe für die Tastaturbelegung.
        app.ui.bind_key = Some(",".to_string());
        app.ui.bind_slot = None;
        app.ui.inventory_open = true;
        app.ui.inventory_tab = 2;
        crate::inventory::funktion_gewaehlt(&mut app, "draw.wall");

        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die Funktion liegt nicht auf der Taste"
        );
        assert!(
            !app.ui.inventory_open,
            "die Funktionsliste bleibt nach dem Zuteilen auf die Taste offen"
        );

        // Eine fest verdrahtete Funktion wird nicht zugeteilt — die Liste bleibt
        // stehen, damit der nächste Klick greifen kann.
        app.ui.bind_key = Some(",".to_string());
        app.ui.inventory_open = true;
        app.ui.inventory_tab = 2;
        crate::inventory::funktion_gewaehlt(&mut app, "file.save");
        assert!(
            app.ui.inventory_open,
            "ohne Zuteilung darf die Funktionsliste nicht zufallen"
        );
        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die feste Funktion hat die Taste überschrieben"
        );

        // Der Ribbon-Klick belegt den wartenden Slot ebenfalls und schließt die
        // Liste (`App::start_tool` nimmt den Slot-Bezug auf).
        app.ui.bind_key = None;
        app.ui.bind_slot = Some(4);
        app.ui.inventory_open = true;
        app.start_tool("draw.circle");
        assert_eq!(
            app.prefs.slots().get(4).cloned().flatten().as_deref(),
            Some("draw.circle"),
            "der Ribbon-Klick hat den Slot nicht belegt"
        );
        assert!(
            !app.ui.inventory_open,
            "die Funktionsliste bleibt nach dem Ribbon-Klick offen"
        );
        assert!(app.tool.is_none(), "das Werkzeug läuft nach dem Zuteilen weiter");
    }

    /// T0076: Unter dem Tastenbild steht **keine** Funktionsauswahl mehr —
    /// weder die gewählte Taste noch „Funktion wählen …" oder „Belegung dieser
    /// Taste entfernen". Belegt wird über einen Klick auf die Taste (Inventar),
    /// geleert über die mittlere Maustaste.
    #[test]
    fn tastaturfenster_hat_keine_funktionsauswahl_mehr() {
        let (mut app, ctx) = app_mit_tastatur();
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let (_, _, q) = *taste_mit(&tasten, "q");
        klick(&ctx, &mut app, q.center());
        let out = rahmen(&ctx, &mut app, vec![]);
        let texte: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        for weg in [
            "Taste: q",
            "Funktion wählen",
            "Belegung dieser Taste entfernen",
            "Werkseinstellungen",
        ] {
            assert!(
                !texte.iter().any(|t| t.contains(weg)),
                "„{weg}“ steht weiterhin im Fenster"
            );
        }
        assert!(
            !texte.iter().any(|t| t == "Wand"),
            "die alte Funktionsliste steht weiterhin im Fenster"
        );
        // Die Layoutzeile ist der Ersatz: Auswahl der gespeicherten Layouts.
        assert!(
            texte.iter().any(|t| t == "Layout"),
            "die Layoutauswahl fehlt unter dem Tastenbild"
        );
        // T0079: Die Auswahl zeigt die erste Vorlage („Vorlage JoJa-CAD") —
        // die fremden Vorlagen stehen im aufgeklappten Menü (eigener Test).
        assert!(
            texte.iter().any(|t| t.contains("Vorlage JoJa-CAD")),
            "die Vorlage JoJa-CAD steht nicht in der Auswahl"
        );
    }

    /// T0076: Das Fenster ist genau so breit wie das Tastenbild — rechts bleibt
    /// kein leerer Rand (vorher 860 statt 716 px).
    #[test]
    fn fenster_ist_so_breit_wie_das_tastenbild() {
        let (app, _ctx) = app_mit_tastatur();
        let fenster = app.ui.last_keymap_rect.expect("Fenster gezeichnet");
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        let bild = tastatur_flaeche();
        // Der Rahmen des Fensters kommt zur Inhaltsbreite hinzu; mehr als ein
        // schmaler Rahmen darf es nicht sein.
        let rand = fenster.width() - bild.x;
        assert!(
            (0.0..=40.0).contains(&rand),
            "Fenster ist {} px breit, das Tastenbild {} px — Rand {rand}",
            fenster.width(),
            bild.x
        );
        let rechts = tasten[0].2.left() + bild.x;
        assert!(
            (rechts - (fenster.right() - rand / 2.0)).abs() < 1.0,
            "das Tastenbild sitzt nicht mittig im Fenster"
        );
    }

    /// T0076: Das Informationssymbol steht in der oberen Leiste, linksbündig —
    /// nicht mehr über dem Tastenbild und nicht rechts.
    #[test]
    fn infosymbol_steht_links_in_der_oberen_leiste() {
        let (app, _ctx) = app_mit_tastatur();
        let fenster = app.ui.last_keymap_rect.expect("Fenster gezeichnet");
        let info = app.ui.last_keymap_info.expect("Infosymbol gezeichnet");
        let tasten = app.ui.last_tasten.clone().expect("Tastenbild gezeichnet");
        assert!(
            info.center().y < tasten[0].2.top(),
            "das Infosymbol steht nicht über dem Tastenbild"
        );
        assert!(
            info.center().y < fenster.top() + 40.0,
            "das Infosymbol steht nicht in der oberen Leiste"
        );
        assert!(
            info.left() < fenster.left() + 30.0,
            "das Infosymbol steht nicht linksbündig"
        );
    }

    /// T0076: Auswahlfeld und Knöpfe der Layoutzeile sind gleich hoch und
    /// stehen auf einer Zeile — vorher war das Feld 20 px und die Knöpfe 24 px.
    #[test]
    fn layoutzeile_hat_fuer_alle_dieselbe_hoehe() {
        let (app, _ctx) = app_mit_tastatur();
        let zeile = app
            .ui
            .last_keymap_zeile
            .clone()
            .expect("Layoutzeile gezeichnet");
        assert_eq!(zeile.len(), 5, "die Layoutzeile hat nicht fünf Bedienelemente");
        let (_, erste) = &zeile[0];
        for (name, r) in &zeile {
            assert!(
                (r.height() - erste.height()).abs() < 0.5,
                "„{name}“ ist {} px hoch, die Auswahl {:.1} px",
                r.height(),
                erste.height()
            );
            assert!(
                (r.center().y - erste.center().y).abs() < 1.0,
                "„{name}“ steht nicht auf derselben Zeile"
            );
        }
        assert!(
            erste.height() > 20.0,
            "die Zeile ist nur {:.1} px hoch",
            erste.height()
        );
    }

    /// Mitte der gezeichneten Beschriftung `label` (Knopf einer Rückfrage).
    fn text_mitte(out: &egui::FullOutput, label: &str) -> Option<Pos2> {
        out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(t.pos + t.galley.size() / 2.0)
            }
            _ => None,
        })
    }

    /// T0076: Das Auswahlmenü wächst mit der Zahl der Layouts — es zeigt alle
    /// Layouts (hier zehn) und bleibt dabei im Fenster.
    #[test]
    fn layoutmenue_waechst_mit_der_zahl_der_layouts() {
        let (mut app, ctx) = app_mit_tastatur();
        app.prefs.keymap_presets = (0..10)
            .map(|i| jjcad_core::persist::KeymapPreset {
                name: format!("Layout {i:02}"),
                bindings: Default::default(),
            })
            .collect();
        app.prefs.keymap_aktiv = Some(0);
        let zeile = rahmen(&ctx, &mut app, vec![]);
        let _ = zeile;
        let feld = app
            .ui
            .last_keymap_zeile
            .clone()
            .expect("Layoutzeile gezeichnet")[0]
            .1;
        klick(&ctx, &mut app, feld.center());
        let out = rahmen(&ctx, &mut app, vec![]);
        let mut ys = Vec::new();
        for i in 0..10 {
            let name = format!("Layout {i:02}");
            let mitte = text_mitte(&out, &name)
                .unwrap_or_else(|| panic!("„{name}“ steht nicht im Menü"));
            assert!(
                mitte.y > 0.0 && mitte.y < 950.0,
                "„{name}“ steht außerhalb des Fensters (y = {})",
                mitte.y
            );
            ys.push(mitte.y);
        }
        let spanne = ys.iter().cloned().fold(f32::MIN, f32::max)
            - ys.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spanne > 8.0 * 20.0,
            "das Menü zeigt nur einen Ausschnitt (Höhe der zehn Zeilen: {spanne:.0} px)"
        );
    }

    /// T0079: Das Auswahlmenü führt die vier eingebauten Vorlagen.
    #[test]
    fn layoutmenue_zeigt_die_vier_vorlagen() {
        let (mut app, ctx) = app_mit_tastatur();
        for _ in 0..2 {
            rahmen(&ctx, &mut app, vec![]);
        }
        let feld = app
            .ui
            .last_keymap_zeile
            .clone()
            .expect("Layoutzeile gezeichnet")[0]
            .1;
        klick(&ctx, &mut app, feld.center());
        let out = rahmen(&ctx, &mut app, vec![]);
        for name in jjcad_core::persist::VORLAGEN {
            let _ = text_mitte(&out, name)
                .unwrap_or_else(|| panic!("„{name}“ steht nicht im Menü"));
        }
    }

    /// T0079: Die eingebauten Vorlagen lassen sich nicht löschen — der Knopf
    /// ist ausgegraut und ein Klick ändert nichts.
    #[test]
    fn eingebaute_vorlage_laesst_sich_nicht_loeschen() {
        let (mut app, ctx) = app_mit_tastatur();
        for _ in 0..2 {
            rahmen(&ctx, &mut app, vec![]);
        }
        assert_eq!(app.prefs.keymap_aktiv(), Some(0));
        let zeile = app
            .ui
            .last_keymap_zeile
            .clone()
            .expect("Layoutzeile gezeichnet");
        let (_, loeschen) = zeile
            .iter()
            .find(|(n, _)| n == "Löschen")
            .expect("kein Löschen-Knopf");
        klick(&ctx, &mut app, loeschen.center());
        assert_eq!(
            app.prefs.keymap_presets.len(),
            jjcad_core::persist::VORLAGEN.len(),
            "eine eingebaute Vorlage wurde gelöscht"
        );
        assert_eq!(app.prefs.keymap_aktiv(), Some(0));
    }

    /// T0076: Wechsel aus einer ungespeicherten Belegung fragt nach, statt die
    /// Änderung still zu verwerfen.
    #[test]
    fn wechsel_aus_ungespeicherter_belegung_fragt_nach() {
        let (mut app, _ctx) = app_mit_tastatur();
        app.prefs.bind("draw.wall", ",");
        assert!(app.prefs.keymap_ungespeichert());
        layout_wechseln(&mut app, 0);
        assert_eq!(
            app.ui.keymap_wechsel,
            Some(0),
            "der Wechsel läuft ohne Rückfrage durch"
        );
        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die Belegung wurde schon verworfen"
        );
    }

    /// T0076: „Abbrechen" lässt alles stehen, „Verwerfen" wechselt ohne Sichern.
    #[test]
    fn rueckfrage_abbrechen_und_verwerfen() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("wechsel-rueckfrage");
        let (mut app, ctx) = app_mit_tastatur();
        app.prefs.bind("draw.wall", ",");
        layout_wechseln(&mut app, 0);
        // Ein frisches Fenster (Area) ist erst im zweiten Bild sichtbar.
        rahmen(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        // „Abbrechen": nichts ändert sich.
        let abbrechen = text_mitte(&out, "Abbrechen").expect("kein Abbrechen-Knopf");
        klick(&ctx, &mut app, abbrechen);
        assert!(app.ui.keymap_wechsel.is_none(), "die Rückfrage bleibt stehen");
        assert!(app.prefs.keymap_ungespeichert(), "die Änderung ging verloren");

        // „Verwerfen": das Ziel wird angewendet, die Änderung fällt weg.
        layout_wechseln(&mut app, 0);
        rahmen(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        let verwerfen = text_mitte(&out, "Verwerfen").expect("kein Verwerfen-Knopf");
        klick(&ctx, &mut app, verwerfen);
        assert!(app.ui.keymap_wechsel.is_none());
        assert_eq!(app.prefs.keymap_aktiv(), Some(0));
        assert!(
            app.prefs.action_for_key(",").is_none(),
            "die ungespeicherte Taste blieb belegt"
        );
    }

    /// T0076: „Speichern" sichert die Belegung und wechselt danach auf das Ziel.
    #[test]
    fn rueckfrage_speichern_sichert_und_wechselt() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("wechsel-speichern");
        let (mut app, ctx) = app_mit_tastatur();
        // Ein zweites, leeres Ziel anlegen, damit der Wechsel etwas zu tun hat.
        app.prefs.keymap_presets.push(jjcad_core::persist::KeymapPreset {
            name: "Ziel".to_string(),
            bindings: Default::default(),
        });
        app.prefs.keymap_aktiv = Some(0);
        app.prefs.bind("draw.wall", ",");
        layout_wechseln(&mut app, 1);
        rahmen(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        let sichern = text_mitte(&out, "Speichern").expect("kein Speichern-Knopf");
        klick(&ctx, &mut app, sichern);
        assert!(app.ui.keymap_name.is_some(), "die Namensabfrage fehlt");

        // Der Nutzer tippt einen Namen und bestätigt.
        app.ui.keymap_name = Some("Neu".to_string());
        rahmen(&ctx, &mut app, vec![]);
        rahmen(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            app.prefs.keymap_presets.iter().any(|p| p.name == "Neu"),
            "das Layout wurde nicht gesichert"
        );
        assert_eq!(
            app.prefs.keymap_aktiv(),
            Some(1),
            "nach dem Sichern wurde nicht auf das Ziel gewechselt"
        );
        assert!(app.ui.keymap_wechsel.is_none(), "der Wechsel bleibt offen");
    }

    /// T0076: „Speichern …" öffnet die Namensabfrage, Enter legt das Layout an —
    /// und der Name steht danach in der Auswahl der Layoutzeile.
    #[test]
    fn layout_speichern_ueber_die_namensabfrage() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("layout-speichern");
        let (mut app, ctx) = app_mit_tastatur();
        app.prefs.bind("draw.wall", ",");
        app.ui.keymap_name = Some("Büro".to_string());
        let eingabe = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        rahmen(&ctx, &mut app, vec![eingabe]);
        assert!(
            app.prefs.keymap_presets.iter().any(|p| p.name == "Büro"),
            "das Layout wurde nicht angelegt"
        );
        assert!(
            app.prefs.action_for_key(",").as_deref() == Some("draw.wall"),
            "die Belegung ging beim Speichern verloren"
        );
        assert!(app.ui.keymap_name.is_none(), "die Namensabfrage bleibt offen");

        // Der gespeicherte Name erscheint in der Auswahl — auch dann, wenn die
        // Belegung der Vorlage JoJa-CAD entspricht (das war der Fehler).
        rahmen(&ctx, &mut app, vec![]);
        let out = rahmen(&ctx, &mut app, vec![]);
        let texte: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            app.prefs
                .keymap_aktiv()
                .map(|i| app.prefs.keymap_presets[i].name.clone())
                .as_deref(),
            Some("Büro"),
            "die Auswahl steht nicht auf dem gespeicherten Layout"
        );
        assert!(
            texte.iter().any(|t| t == "Büro"),
            "der gespeicherte Name steht nicht in der Auswahl"
        );
    }

    /// T0076: „Löschen" in der Layoutzeile entfernt das gewählte Layout; die
    /// Taste, die es trug, bleibt belegt (danach „Eigene Belegung").
    #[test]
    fn loeschen_entfernt_das_gewaehlte_layout() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("layout-loeschen");
        let (mut app, ctx) = app_mit_tastatur();
        app.prefs.bind("draw.wall", ",");
        app.prefs.keymap_speichern("Büro").expect("Layout speichern");
        for _ in 0..2 {
            rahmen(&ctx, &mut app, vec![]);
        }
        let zeile = app
            .ui
            .last_keymap_zeile
            .clone()
            .expect("Layoutzeile gezeichnet");
        let (_, loeschen) = zeile
            .iter()
            .find(|(n, _)| n == "Löschen")
            .expect("kein Löschen-Knopf");
        klick(&ctx, &mut app, loeschen.center());

        assert!(
            !app.prefs.keymap_presets.iter().any(|p| p.name == "Büro"),
            "das Layout wurde nicht gelöscht"
        );
        assert_eq!(app.prefs.keymap_aktiv(), None);
        assert_eq!(
            app.prefs.action_for_key(",").as_deref(),
            Some("draw.wall"),
            "die Tasten des gelöschten Layouts gingen verloren"
        );
    }
}

// ---------------------------------------------------------------------------
// Einstellungen: Aufbau und Suche (T0061)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests_einstellungen {
    //! T0061: Das Einstellungsfenster ist eine Kategorienliste links (mit Suche
    //! darüber) und die Einstellungen der gewählten Kategorie rechts — Vorbild
    //! ist die KDE-Systemsteuerung. Gemessen werden die Rechtecke, die das
    //! Fenster selbst hinterlässt (`UiState::last_settings_*`).

    use super::tests_tastatur::{WegwerfKonfig, KONFIG};
    use super::*;

    fn frame(app: &mut App, ctx: &egui::Context) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Zeichnet das Einstellungsfenster ein paar Bilder lang (ein zentriertes,
    /// selbstgroßes Fenster wächst erst nach mehreren Bildern) und liefert die
    /// gemerkten Rechtecke.
    fn lage(suche: &str) -> (Rect, Rect, Rect) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_suche = suche.to_string();
        for _ in 0..4 {
            frame(&mut app, &ctx);
        }
        (
            app.ui.last_settings_suche.expect("Suchfeld gezeichnet"),
            app.ui.last_settings_nav.expect("Kategorienliste gezeichnet"),
            app.ui.last_settings_pane.expect("Einstellungsseite gezeichnet"),
        )
    }

    #[test]
    fn suche_steht_ueber_der_tabelle_und_einstellungen_rechts() {
        let (suche, nav, pane) = lage("");
        assert!(
            suche.bottom() <= nav.top() + 1.0,
            "Suchfeld endet bei y = {}, die Kategorienliste beginnt erst bei {} — \
             die Suche steht nicht darüber",
            suche.bottom(),
            nav.top()
        );
        // Das Textfeld meldet seinen Rahmen um 4 px eingerückt — Toleranz 12 px.
        assert!(
            (suche.left() - nav.left()).abs() < 12.0
                && (suche.width() - nav.width()).abs() < 12.0,
            "Suchfeld ({}…{}) und Kategorienliste ({}…{}) stehen nicht übereinander",
            suche.left(),
            suche.right(),
            nav.left(),
            nav.right()
        );
        assert!(
            nav.right() < pane.left(),
            "die Kategorienliste endet bei x = {}, die Einstellungen beginnen bei {} — \
             sie stehen nicht links neben den Einstellungen",
            nav.right(),
            pane.left()
        );
        assert!(
            (nav.width() - SETTINGS_NAV_W).abs() < 1.0,
            "Kategorienspalte ist {} px breit, erwartet {SETTINGS_NAV_W} px",
            nav.width()
        );
        assert!(
            nav.height() > 400.0 && (nav.bottom() - pane.bottom()).abs() < 1.5,
            "Kategorienliste ({} … {}) und Einstellungsseite ({} … {}) enden nicht auf              derselben Höhe",
            nav.top(),
            nav.bottom(),
            pane.top(),
            pane.bottom()
        );
        assert!(
            pane.width() > nav.width(),
            "die Einstellungsseite ({} px) ist nicht breiter als die Kategorienspalte ({} px)",
            pane.width(),
            nav.width()
        );
    }

    /// T0135: Das Einstellungsfenster hat in **allen** Kategorien dieselbe
    /// Größe. Vorher wuchs es in „Speicherorte“ Bild für Bild, weil die
    /// Pfadzeilen ihre Breite aus dem verfügbaren Platz rechneten und das
    /// Fenster mit jeder Zeile weiter aufging.
    #[test]
    fn einstellungsfenster_gleich_gross_in_allen_kategorien() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("fenstergroesse");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        // Jede Kategorie ein paar Bilder lang zeichnen (ein selbstgroßes
        // Fenster wächst erst nach mehreren Bildern) und die Fensterfläche
        // merken.
        let mut masse: Vec<(&'static str, f32, f32)> = Vec::new();
        for (i, kat) in EINSTELLUNGSKATEGORIEN.iter().enumerate() {
            app.ui.settings_kat = i;
            app.ui.settings_suche.clear();
            app.ui.last_settings_fenster = None;
            for _ in 0..4 {
                frame(&mut app, &ctx);
            }
            let r = app
                .ui
                .last_settings_fenster
                .unwrap_or_else(|| panic!("Kategorie „{}“ zeichnet kein Fenster", kat.name()));
            masse.push((kat.name(), r.width(), r.height()));
        }
        let (erste, breite, hoehe) = masse[0];
        for (name, b, h) in &masse {
            assert!(
                (b - breite).abs() < 0.5,
                "Kategorie „{name}“ ist {b} px breit, „{erste}“ {breite} px"
            );
            assert!(
                (h - hoehe).abs() < 0.5,
                "Kategorie „{name}“ ist {h} px hoch, „{erste}“ {hoehe} px"
            );
        }
        // Der Rahmen legt links und rechts je ~18 px zu (gemessen 976 px
        // statt 940 Inhalt) — entscheidend ist, dass keine Kategorie mehr
        // darüber hinauswächst.
        assert!(
            breite <= SETTINGS_BREITE + 20.0,
            "das Fenster ist {breite} px breit, höchstens {SETTINGS_BREITE} px Inhalt erwartet"
        );
    }

    #[test]
    fn jede_kategorie_hat_name_icon_und_stichworte() {
        assert_eq!(EINSTELLUNGSKATEGORIEN.len(), 10, "Kategorienzahl");
        for k in EINSTELLUNGSKATEGORIEN {
            assert!(!k.name().is_empty(), "Kategorie ohne Namen");
            assert!(!k.icon.is_empty(), "Kategorie „{}“ ohne Icon", k.name());
            assert!(
                k.stichworte.len() >= 2,
                "Kategorie „{}“ hat zu wenige Stichworte — die Suche findet sie nicht",
                k.name()
            );
        }
    }

    /// Die Suche findet die Kategorie über den Namen einer Einstellung —
    /// sonst bleibt eine Einstellung in einer anderen Kategorie versteckt.
    #[test]
    fn suche_findet_kategorien_ueber_ihre_einstellungen() {
        let treffer = |s: &str| -> Vec<&'static str> {
            EINSTELLUNGSKATEGORIEN
                .iter()
                .filter(|k| k.passt(&s.to_lowercase()))
                .map(|k| k.name())
                .collect()
        };
        assert_eq!(treffer(""), EINSTELLUNGSKATEGORIEN.iter().map(|k| k.name()).collect::<Vec<_>>());
        assert_eq!(treffer("polarwinkel"), vec!["Fangen"]);
        assert_eq!(treffer("Wandstärke"), vec!["Architektur"]);
        assert_eq!(treffer("dateidialog"), vec!["Bedienung"]);
        assert_eq!(treffer("maus verschiebt"), vec!["Bedienung"]);
        // T0067: Die Bewegungsgeschwindigkeit ist unter „Bedienung“ zu finden.
        assert_eq!(treffer("bewegungsgeschwindigkeit"), vec!["Bedienung"]);
        assert_eq!(treffer("schwenk"), vec!["Bedienung"]);
        // T0070: Der Drehpunkt der Einfüge-Vorschau steht unter „Bedienung“.
        assert_eq!(treffer("drehpunkt"), vec!["Bedienung"]);
        // T0111: Der Farbakzent steht unter „Darstellung“.
        assert_eq!(treffer("farbakzent"), vec!["Darstellung"]);
        // T0114, Turn 3: die maximale Textauflösung steht unter „Darstellung“.
        assert_eq!(treffer("textauflösung"), vec!["Darstellung"]);
        assert_eq!(treffer("auflösung"), vec!["Darstellung"]);
        // T0129: Die Sicherung ist eine eigene Kategorie.
        assert!(
            treffer("verlauf").contains(&"Sicherung"),
            "„Verlauf“ findet die Kategorie Sicherung nicht"
        );
        assert!(
            treffer("aufbewahrungszeitraum").contains(&"Sicherung"),
            "„Aufbewahrungszeitraum“ findet die Kategorie Sicherung nicht"
        );
        assert!(
            treffer("intervall").contains(&"Sicherung"),
            "„Intervall“ findet die Kategorie Sicherung nicht"
        );
        assert!(treffer("gibtesnicht").is_empty(), "Unbekannter Begriff liefert Treffer");
    }

    /// T0129: Zahlen der Sicherung stehen mit deutschem Dezimalkomma.
    #[test]
    fn sicherungszahlen_mit_komma() {
        assert_eq!(zahl_deutsch(7.66, 1), "7,7");
        assert_eq!(zahl_deutsch(0.0, 1), "0,0");
        assert_eq!(groesse_text(110_400), "110,4 kB");
        assert_eq!(groesse_text(1_500_000), "1,5 MB");
    }

    /// T0129: Die Kategorie „Sicherung“ zeigt Verlauf, Plätze, Reichweite,
    /// Intervall, Abstand und Poolgröße.
    #[test]
    fn sicherung_zeigt_ihre_einstellungen() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("sicherung");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        // Eigener Pool: der Datenordner des Nutzers bleibt unberührt.
        let dir = std::env::temp_dir().join(format!(
            "jjcad-sicherung-ui-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        app.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(&dir));
        let kat = EINSTELLUNGSKATEGORIEN
            .iter()
            .position(|k| k.name() == "Sicherung")
            .expect("Kategorie Sicherung");
        app.ui.settings_open = true;
        app.ui.settings_kat = kat;
        app.ui.settings_suche = String::new();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in [
            "Verlauf aufbewahren",
            "Anzahl der Sicherungen",
            "Aufbewahrungszeitraum",
            "Sicherung alle",
            "Abstand der Stände",
            "Belegter Speicher",
            "Sicherungsordner",
        ] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt in der Kategorie Sicherung"
            );
        }
    }

    /// T0144: Die Kategorie „Raster“ führt alle Rastereinstellungen — Weite,
    /// Anzeige, Fang, Art, Modus und den erzwungenen Rasterfang.
    #[test]
    fn raster_zeigt_art_modus_und_zwang() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("raster");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let kat = EINSTELLUNGSKATEGORIEN
            .iter()
            .position(|k| k.name() == "Raster")
            .expect("Kategorie Raster");
        app.ui.settings_open = true;
        app.ui.settings_kat = kat;
        app.ui.settings_suche = String::new();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in [
            "Rasterweite",
            "Raster anzeigen",
            "Rasterfang",
            "Rasterart",
            "Rechtwinklig",
            "Weite folgt",
            "Ansicht (skaliert)",
            "Erzwingen",
        ] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt in der Kategorie Raster"
            );
        }
    }

    /// Mitte der gezeichneten Beschriftung `label` (Knopf, Chip, Zeilenwert).
    fn text_mitte(out: &egui::FullOutput, label: &str) -> Option<Pos2> {
        out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
    }

    /// Ein linker Mausklick auf `pos` (Drücken und Loslassen in eigenen Bildern).
    fn klick(app: &mut App, ctx: &egui::Context, pos: Pos2) {
        let knopf = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![knopf(true)],
            vec![knopf(false)],
            vec![],
        ] {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
        }
    }

    /// T0102: Unter „Darstellung“ steht die Zeile „Design“ mit den drei Wahlen.
    /// Ein Klick auf „Hell“ stellt die Farben sofort um und schreibt die Wahl in
    /// die Voreinstellungen (`prefs.json`). Der Test stellt am Ende das dunkle
    /// Design wieder her — die Palette gilt prozessweit (wie die Sprache).
    #[test]
    fn design_zeile_schaltet_das_helle_design() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("design");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.set_design(Design::Dunkel, &ctx);
        let dunkel_bg = Palette::bg();
        assert!(!theme::ist_hell(), "Ausgangslage ist nicht dunkel");

        let kat = EINSTELLUNGSKATEGORIEN
            .iter()
            .position(|k| k.name() == "Darstellung")
            .expect("Kategorie Darstellung");
        app.ui.settings_open = true;
        app.ui.settings_kat = kat;
        app.ui.settings_suche = "Design".to_string();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in ["Design", "System", "Hell", "Dunkel"] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt im Einstellungsfenster"
            );
        }

        let hell = text_mitte(&out, "Hell").expect("„Hell“ steht nicht im Fenster");
        klick(&mut app, &ctx, hell);

        assert_eq!(
            app.prefs.design,
            Design::Hell,
            "die Wahl steht nicht in den Voreinstellungen"
        );
        assert!(theme::ist_hell(), "die Oberfläche blieb im dunklen Design");
        assert_ne!(Palette::bg(), dunkel_bg, "die Flächenfarbe blieb gleich");
        assert_eq!(
            jjcad_core::persist::UserPrefs::load().design,
            Design::Hell,
            "die Wahl wurde nicht gespeichert"
        );

        // Ausgangszustand für die übrigen Tests wiederherstellen.
        app.set_design(Design::Dunkel, &ctx);
        assert!(!theme::ist_hell());
        assert_eq!(Palette::bg(), dunkel_bg);
    }

    /// T0111: Unter „Darstellung“ steht die Zeile „Farbakzente“ mit sechs
    /// Wahlen. Ein Klick auf „Blau“ färbt die Akzentstellen sofort um und
    /// schreibt die Wahl in die Voreinstellungen; Bernstein bleibt die Vorgabe.
    #[test]
    fn farbakzent_zeile_faerbt_die_oberflaeche() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("farbakzent");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.set_design(Design::Dunkel, &ctx);
        app.set_akzent(Farbakzent::Bernstein, &ctx);
        let bernstein = Palette::accent();
        assert_eq!(bernstein, egui::Color32::from_rgb(0xE3, 0xA2, 0x3D));

        let kat = EINSTELLUNGSKATEGORIEN
            .iter()
            .position(|k| k.name() == "Darstellung")
            .expect("Kategorie Darstellung");
        app.ui.settings_open = true;
        app.ui.settings_kat = kat;
        app.ui.settings_suche = "Farbakzent".to_string();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in ["Farbakzente", "Bernstein"] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt im Einstellungsfenster"
            );
        }
        // Die Vorgabe bleibt das bisherige Bernstein-Orange.
        assert_eq!(Palette::accent(), bernstein, "die Vorgabe ist nicht Bernstein");

        // Auswahlfeld öffnen (es zeigt den gewählten Akzent) und „Saphir“ wählen.
        let feld = text_mitte(&out, "Bernstein").expect("„Bernstein“ steht nicht im Fenster");
        klick(&mut app, &ctx, feld);
        for _ in 0..2 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in ["Bernstein", "Saphir", "Smaragd", "Rubin", "Amethyst"] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt im Auswahlmenü"
            );
        }
        // T0111, Turn 2: Türkis ist entfallen — es darf nicht mehr auftauchen.
        assert!(
            text_mitte(&out, "Türkis").is_none(),
            "die entfallene Wahl „Türkis“ steht noch im Menü"
        );
        let saphir = text_mitte(&out, "Saphir").expect("„Saphir“ steht nicht im Menü");
        klick(&mut app, &ctx, saphir);

        assert_eq!(
            app.prefs.farbakzent,
            Farbakzent::Saphir,
            "die Wahl steht nicht in den Voreinstellungen"
        );
        assert_eq!(
            Palette::accent(),
            egui::Color32::from_rgb(0x4C, 0x9A, 0xE8),
            "die Akzentfarbe der Bedienung blieb gleich"
        );
        assert_eq!(
            jjcad_core::persist::UserPrefs::load().farbakzent,
            Farbakzent::Saphir,
            "die Wahl wurde nicht gespeichert"
        );

        // Ausgangszustand für die übrigen Tests wiederherstellen.
        app.set_akzent(Farbakzent::Bernstein, &ctx);
        assert_eq!(Palette::accent(), bernstein);
    }

    /// T0114, Turn 3: Unter „Darstellung“ steht die Zeile „Textauflösung“ mit
    /// den Stufen 128/256/512/1024 px. Eine Wahl wirkt sofort auf die
    /// Rastergröße der Schrift und landet in `prefs.json`.
    #[test]
    fn textaufloesung_zeile_setzt_die_rastergroesse() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("textaufloesung");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        assert_eq!(
            app.prefs.textaufloesung,
            Textaufloesung::Px256,
            "die Vorgabe ist nicht 256 px"
        );

        let kat = EINSTELLUNGSKATEGORIEN
            .iter()
            .position(|k| k.name() == "Darstellung")
            .expect("Kategorie Darstellung");
        app.ui.settings_open = true;
        app.ui.settings_kat = kat;
        app.ui.settings_suche = "Textauflösung".to_string();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let feld = text_mitte(&out, "256 px").expect("„256 px“ steht nicht im Fenster");

        // Auswahlfeld öffnen — alle vier Stufen stehen im Menü.
        klick(&mut app, &ctx, feld);
        for _ in 0..2 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in ["128 px", "256 px", "512 px", "1024 px"] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt im Auswahlmenü"
            );
        }
        let hoch = text_mitte(&out, "1024 px").expect("„1024 px“ steht nicht im Menü");
        klick(&mut app, &ctx, hoch);

        assert_eq!(
            app.prefs.textaufloesung,
            Textaufloesung::Px1024,
            "die Wahl steht nicht in den Voreinstellungen"
        );
        assert_eq!(
            jjcad_core::persist::UserPrefs::load().textaufloesung,
            Textaufloesung::Px1024,
            "die Wahl wurde nicht gespeichert"
        );
        // Die Rastergröße folgt der Wahl: 1980 px gewünscht (250 mm bei 600 %)
        // werden jetzt in 1024 px gerastert statt in 256 px.
        let (raster, faktor) =
            crate::textgroesse::raster_und_skalierung(1980.0, app.prefs.textaufloesung.px());
        // Die oberste Leiterstufe unter 1024 px ist 992,6 px (3 px × 1,25²⁶) —
        // sie muss deutlich über der alten Vorgabe von 256 px liegen.
        assert!(
            raster > 900.0,
            "die Schrift wird nur in {raster} px gerastert"
        );
        assert!(
            (raster * faktor - 1980.0).abs() < 1.0,
            "1024 px × {faktor} ≠ 1980 px"
        );

        // Zurück auf die Vorgabe — die Wegwerf-Einstellungen bleiben sauber.
        app.prefs.textaufloesung = Textaufloesung::Px256;
        let _ = app.prefs.save();
    }

    /// Ein Suchbegriff außerhalb der gewählten Kategorie springt auf die erste
    /// passende — sonst bliebe die rechte Seite leer.
    #[test]
    fn suche_waehlt_passende_kategorie_aus() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 0;
        app.ui.settings_suche = "Polar".to_string();
        for _ in 0..4 {
            frame(&mut app, &ctx);
        }
        assert_eq!(
            EINSTELLUNGSKATEGORIEN[app.ui.settings_kat].name(),
            "Fangen",
            "die Suche hat die Kategorie nicht auf den Treffer gezogen"
        );
    }

    /// Die Einstellungsseite ist kürzer als das Fenster: sie braucht eine eigene
    /// Bildlaufleiste, damit auch die große Zahl künftiger Einstellungen
    /// erreichbar bleibt.
    #[test]
    fn einstellungsseite_scrollt() {
        let quelle = include_str!("dialogs.rs");
        let anfang = quelle.find("fn settings_body").expect("settings_body");
        let ende = quelle[anfang..]
            .find("\nfn ")
            .map(|i| anfang + i)
            .unwrap_or(quelle.len());
        let body = &quelle[anfang..ende];
        assert!(
            body.matches("ScrollArea::vertical()").count() >= 2,
            "Kategorienliste und Einstellungsseite brauchen je eine eigene Bildlaufleiste"
        );
    }

    /// T0063: Unter „Bedienung“ steht die Einstellung „Maus verschiebt“ — sie
    /// wird gezeichnet (nicht nur im Code geführt), und die Vorgabe ist an
    /// (bisheriges Verhalten: die meisten Objekte folgten der Maus).
    #[test]
    fn bedienung_zeigt_die_einstellung_maus_verschiebt() {
        // Die Vorgabe wird geprüft, nicht der Rechner: `App::with_ctx` liest
        // `prefs.json`, und wer „Maus verschiebt“ abgeschaltet hat, machte den
        // Test rot (T0122, `ship-card.sh` läuft mit dem echten XDG_CONFIG_HOME).
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("maus-verschiebt");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        // Zentriertes Fenster: Größe und Zeilenlage stehen erst nach ein paar Bildern.
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        assert!(
            out.shapes
                .iter()
                .any(|cs| matches!(&cs.shape,
                    egui::Shape::Text(t) if t.galley.text() == "Maus verschiebt")),
            "Zeile „Maus verschiebt“ fehlt im Einstellungsfenster"
        );
        // T0127: `prefs.json` liegt in der Datenwurzel — `WegwerfKonfig` leitet
        // nur `XDG_CONFIG_HOME` um. Geprüft wird deshalb die Vorgabe selbst.
        assert!(
            jjcad_core::persist::UserPrefs::default().mouse_move,
            "Vorgabe: Maus verschiebt ist eingeschaltet"
        );
    }

    /// T0139: Unter „Darstellung“ heißen die beiden Modi nach dem, was sie
    /// zeigen („Bildschirm (konstant)“, „Papier (maßstabsgetreu)“), und der
    /// Modus „Bildschirm (konstant)“ hat eine Zeile für den Stärke-Faktor.
    #[test]
    fn darstellung_zeigt_linienstaerke_modi_und_faktor() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("linienstaerke");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 6;
        app.ui.lineweights_scaled = false;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        for erwartet in ["Bildschirm (konstant)", "Stärke-Faktor"] {
            assert!(
                texte.iter().any(|t| *t == erwartet),
                "„{erwartet}“ fehlt in der Darstellung (gezeichnet: {texte:?})"
            );
        }
    }

    /// T0125: „Voreinstellungen“ zeigt eine Zeile je Objektart — die Liste
    /// kommt aus `LAYER_DEFAULTS`, alle Zeilen tragen zunächst 0,18 mm.
    #[test]
    fn voreinstellungen_zeigen_eine_zeile_je_objektart() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("vorgaben");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 7;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        for (name, _, _) in jjcad_core::style::LAYER_DEFAULTS {
            assert!(
                texte.iter().any(|t| *t == *name),
                "Zeile „{name}“ fehlt in den Voreinstellungen (gezeichnet: {texte:?})"
            );
        }
        assert!(
            texte.iter().any(|t| *t == "0,18 mm"),
            "keine Zeile zeigt die Vorgabe 0,18 mm"
        );
        assert!(
            texte.iter().any(|t| *t == "Vorgaben zurücksetzen"),
            "Knopf „Vorgaben zurücksetzen“ fehlt"
        );
    }

    /// T0125: Eine Änderung in den Voreinstellungen wirkt sofort auf eine
    /// **leere** Zeichnung (noch kein Plan) — eine geöffnete Zeichnung behält
    /// ihre Ebenenbreiten.
    #[test]
    fn voreinstellungen_wirken_sofort_auf_eine_leere_zeichnung() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("vorgaben-sofort");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.set_linienstaerke("Wände", 0.50);
        app.voreinstellungen_uebernehmen();
        assert!(
            (app.doc().layer_by_name("Wände").unwrap().line_weight - 0.50).abs() < 1e-9,
            "leere Zeichnung muss die Voreinstellung sofort tragen"
        );

        // Mit Inhalt ist die Zeichnung ein Plan: ihre Ebenen bleiben stehen.
        app.session_mut()
            .execute("draw.line", &json!({"a": [0.0, 0.0], "b": [1.0, 0.0]}))
            .expect("Linie");
        app.prefs.set_linienstaerke("Wände", 0.13);
        app.voreinstellungen_uebernehmen();
        assert!(
            (app.doc().layer_by_name("Wände").unwrap().line_weight - 0.50).abs() < 1e-9,
            "bestehende Ebene darf sich nicht ändern"
        );
    }

    /// T0125: „Neu“ übernimmt die eingestellte Linienstärke in die
    /// Standardebenen der neuen Zeichnung.
    #[test]
    fn neue_zeichnung_uebernimmt_die_voreinstellungen() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("vorgaben-neu");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.set_linienstaerke("Wände", 0.50);
        app.prefs.set_linienstaerke("Hilfslinien", 0.13);
        app.new_document();
        let doc = app.doc();
        assert!((doc.layer_by_name("Wände").unwrap().line_weight - 0.50).abs() < 1e-9);
        assert!((doc.layer_by_name("Hilfslinien").unwrap().line_weight - 0.13).abs() < 1e-9);
        assert!((doc.layer_by_name("Fenster").unwrap().line_weight - 0.18).abs() < 1e-9);
    }

    /// T0067/T0146: Unter „Bedienung“ steht zusätzlich die Zeile
    /// **„Geschwindigkeit der Ansicht“** (früher „Schwenk“) für das Tempo der
    /// Schwenk-Funktionen (W A S D). Der Name passt ungekürzt in die
    /// Namensspalte — sonst stünde in der Oberfläche „…“.
    #[test]
    fn bedienung_zeigt_die_geschwindigkeit_der_ansicht() {
        // Der Test prüft die **Vorgabe** (1×). Ohne Wegwerf-Voreinstellungen
        // liest `App::with_ctx` die `prefs.json` des Nutzers und der Test prüft
        // dessen Schwenk-Wert (gesehen: 3,9 — rot, obwohl die Oberfläche stimmt).
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("schwenk");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let zeile = out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == "Geschwindigkeit der Ansicht" => Some(t),
            _ => None,
        });
        let zeile = zeile.unwrap_or_else(|| {
            let texte: Vec<&str> = out
                .shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) => Some(t.galley.text()),
                    _ => None,
                })
                .collect();
            panic!("Zeile „Geschwindigkeit der Ansicht“ fehlt im Einstellungsfenster (gezeichnet: {texte:?})")
        });
        assert!(
            zeile.galley.size().x <= crate::widgets::LABEL_W - 8.0,
            "der Name läuft in die Wertspalte: {} px breit, {} px Platz",
            zeile.galley.size().x,
            crate::widgets::LABEL_W - 8.0
        );
        // T0127: `prefs.json` liegt in der Datenwurzel, nicht mehr unter
        // `XDG_CONFIG_HOME` — `WegwerfKonfig` isoliert sie also nicht. Der Test
        // prüft deshalb die **Vorgabe** der Einstellung selbst, nicht den
        // geladenen Wert: der Nutzer stellt „Schwenk“ im laufenden Betrieb um,
        // und `ship-card.sh` testet mit seiner echten `prefs.json` (gesehen:
        // rot nach dem Merge, obwohl die Oberfläche stimmt).
        assert!(
            (jjcad_core::persist::UserPrefs::default().pan_speed - 1.0).abs() < 1e-6,
            "Vorgabe der Geschwindigkeit der Ansicht ist 1×"
        );
    }

    /// T0070: Unter „Bedienung“ steht die Zeile „Drehpunkt“ mit beiden Modi —
    /// gezeichnet, nicht nur im Code geführt; die Vorgabe ist der Mittelpunkt
    /// (T0087).
    #[test]
    fn bedienung_zeigt_den_drehpunkt() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("drehpunkt");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        for erwartet in ["Drehpunkt", "Basispunkt", "Mittelpunkt"] {
            assert!(
                texte.iter().any(|t| *t == erwartet),
                "„{erwartet}“ fehlt im Einstellungsfenster (gezeichnet: {texte:?})"
            );
        }
        // T0127: `prefs.json` liegt in der Datenwurzel — `WegwerfKonfig`
        // isoliert sie nicht mehr. Geprüft wird deshalb die **Vorgabe** des
        // Programms, nicht der geladene Wert des Nutzers (dieselbe Begründung
        // wie bei `bedienung_zeigt_die_geschwindigkeit_der_ansicht`).
        assert_eq!(
            jjcad_core::persist::UserPrefs::default().paste_pivot,
            PastePivot::Center,
            "Vorgabe: der Mittelpunkt ist der Programmstandard (T0087)"
        );
    }

    /// T0145: Unter „Bedienung“ steht ein **zweiter** Zieh-Modus für das
    /// Auswahlfenster. Beide Zeilen zeigen dieselben beiden Wahlen — wer nur
    /// eine Zeile einbaut, bekommt den Test nicht grün.
    #[test]
    fn bedienung_zeigt_einen_eigenen_modus_fuer_das_auswahlfenster() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("auswahlmodus");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        app.ui.settings_suche = String::new();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        for wort in [
            jjcad_core::i18n::tr("gui.dialogs.ziehen"),
            jjcad_core::i18n::tr("gui.dialogs.auswahlfenster"),
        ] {
            assert!(
                text_mitte(&out, wort).is_some(),
                "„{wort}“ fehlt in der Kategorie Bedienung"
            );
        }
        // Zwei Zeilen mit derselben Wahl „Zwei Klicks“: Griffe und Auswahlfenster.
        let zwei_klicks = jjcad_core::persist::DragMode::TwoClicks.text();
        let anzahl = out
            .shapes
            .iter()
            .filter(|cs| matches!(&cs.shape, egui::Shape::Text(t) if t.galley.text() == zwei_klicks))
            .count();
        assert_eq!(
            anzahl, 2,
            "„{zwei_klicks}“ muss in zwei Zeilen stehen (Ziehen und Auswahlfenster), gefunden: {anzahl}"
        );
    }

    /// T0103: Unter „Bedienung" steht die Sprachwahl. Der Test prüft die
    /// gezeichneten Texte und die gespeicherte Auswahl — die globale Sprache
    /// stellt er **nicht** um, sonst liefen die übrigen Tests dieser Datei
    /// (die deutsche Beschriftungen prüfen) kreuz und quer.
    #[test]
    fn bedienung_zeigt_die_sprachwahl() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("sprachwahl");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        for erwartet in ["Sprache", "Deutsch", "English"] {
            assert!(
                texte.iter().any(|t| *t == erwartet),
                "„{erwartet}“ fehlt in den Einstellungen (gezeichnet: {texte:?})"
            );
        }
        assert_eq!(
            app.prefs.language,
            jjcad_core::i18n::Lang::Deutsch,
            "Vorgabe ist Deutsch"
        );
    }

    /// T0087: Der Drehpunkt lässt sich in den Einstellungen in **beide**
    /// Richtungen stellen. Der Klick läuft über echte Ereignisse auf das
    /// gezeichnete Feld — einmal von „Mittelpunkt“ auf „Basispunkt“ und wieder
    /// zurück.
    #[test]
    fn drehpunkt_laesst_sich_umschalten() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("drehpunkt-umschalten");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.paste_pivot = PastePivot::Center;
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        // Der Wert eines gezeichneten Feldes als Klickstelle.
        let stelle = |out: &egui::FullOutput, text: &str| -> Pos2 {
            out.shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) if t.galley.text() == text => {
                        Some(t.pos + t.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("„{text}“ fehlt im Einstellungsfenster"))
        };
        let knopf = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut klick = |app: &mut App, pos: Pos2| {
            for events in [
                vec![egui::Event::PointerMoved(pos)],
                vec![knopf(pos, true)],
                vec![knopf(pos, false)],
                vec![],
            ] {
                let _ = ctx.run(
                    egui::RawInput {
                        events,
                        ..bild()
                    },
                    |ctx| app.ui(ctx),
                );
            }
        };

        klick(&mut app, stelle(&out, "Basispunkt"));
        assert_eq!(
            app.prefs.paste_pivot,
            PastePivot::BasePoint,
            "Klick auf „Basispunkt“ stellt den alten Drehpunkt nicht wieder her"
        );

        out = ctx.run(bild(), |ctx| app.ui(ctx));
        klick(&mut app, stelle(&out, "Mittelpunkt"));
        assert_eq!(
            app.prefs.paste_pivot,
            PastePivot::Center,
            "Klick auf „Mittelpunkt“ stellt den Standard nicht her"
        );
    }

    /// T0101: Unter „Bedienung“ steht die Zeile „Verschieben“ mit beiden
    /// Bewegungsarten; Vorgabe ist „Kontinuierlich“.
    #[test]
    fn bedienung_zeigt_die_bewegungsart_der_ansicht() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("bewegungsart");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        for erwartet in ["Verschieben", "Kontinuierlich", "Sprunghaft"] {
            assert!(
                texte.iter().any(|t| *t == erwartet),
                "„{erwartet}“ fehlt im Einstellungsfenster (gezeichnet: {texte:?})"
            );
        }
        // Beide Wahlen müssen in die Wertespalte passen (150 px) und sich nicht
        // überdecken — sonst läuft der zweite Knopf aus der Zeile.
        let kasten = |text: &str| -> Rect {
            out.shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) if t.galley.text() == text => {
                        Some(Rect::from_min_size(t.pos, t.galley.size()))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("„{text}“ fehlt im Einstellungsfenster"))
        };
        // Nicht nur gezeichnet, sondern **sichtbar**: die Zeile darf nicht
        // unterhalb der Bildlaufgrenze liegen (T0098) — der Beschnitt des
        // gezeichneten Textes muss seinen Kasten enthalten.
        let sichtbar = |text: &str| -> Rect {
            out.shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) if t.galley.text() == text => {
                        let r = Rect::from_min_size(t.pos, t.galley.size());
                        Some(r.intersect(cs.clip_rect))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("„{text}“ fehlt im Einstellungsfenster"))
        };
        for text in ["Verschieben", "Kontinuierlich", "Sprunghaft"] {
            let ganz = kasten(text);
            let sicht = sichtbar(text);
            assert!(
                sicht.width() > ganz.width() - 1.0 && sicht.height() > ganz.height() - 1.0,
                "„{text}“ liegt hinter der Bildlaufgrenze ({ganz:?} beschnitten auf {sicht:?})"
            );
        }
        let (a, b) = (kasten("Kontinuierlich"), kasten("Sprunghaft"));
        assert!(
            a.right() + 2.0 < b.left() || b.right() + 2.0 < a.left(),
            "die beiden Wahlen überdecken sich: {a:?} / {b:?}"
        );
        let pane = app.ui.last_settings_pane.expect("Fläche der Einstellungsseite");
        assert!(
            pane.contains_rect(a) && pane.contains_rect(b),
            "die Wahlen müssen in der Einstellungsseite liegen: {a:?} / {b:?} in {pane:?}"
        );
        assert_eq!(
            app.prefs.pan_mode,
            jjcad_core::persist::PanMode::Kontinuierlich,
            "Vorgabe: die Ansicht folgt der gedrückten Funktion (T0101)"
        );
    }

    /// T0101: Die Bewegungsart lässt sich in den Einstellungen in **beide**
    /// Richtungen stellen — der Klick läuft über echte Ereignisse auf das
    /// gezeichnete Feld (Muster aus T0087). Die Suche holt die Zeile nach oben,
    /// damit sie nicht unterhalb der Bildlaufgrenze liegt (T0098).
    #[test]
    fn bewegungsart_laesst_sich_umschalten() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("bewegungsart-umschalten");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        app.ui.settings_suche = "Ansicht verschieben".into();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let stelle = |out: &egui::FullOutput, text: &str| -> Pos2 {
            out.shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) if t.galley.text() == text => {
                        Some(t.pos + t.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("„{text}“ fehlt im Einstellungsfenster"))
        };
        let knopf = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut klick = |app: &mut App, pos: Pos2| {
            for events in [
                vec![egui::Event::PointerMoved(pos)],
                vec![knopf(pos, true)],
                vec![knopf(pos, false)],
                vec![],
            ] {
                let _ = ctx.run(
                    egui::RawInput {
                        events,
                        ..bild()
                    },
                    |ctx| app.ui(ctx),
                );
            }
        };

        klick(&mut app, stelle(&out, "Sprunghaft"));
        assert_eq!(
            app.prefs.pan_mode,
            jjcad_core::persist::PanMode::Sprunghaft,
            "Klick auf „Sprunghaft“ muss den Einzelschritt wählen"
        );

        out = ctx.run(bild(), |ctx| app.ui(ctx));
        klick(&mut app, stelle(&out, "Kontinuierlich"));
        assert_eq!(
            app.prefs.pan_mode,
            jjcad_core::persist::PanMode::Kontinuierlich,
            "Klick auf „Kontinuierlich“ muss den Dauerlauf wiederherstellen"
        );
    }

    /// T0098: Unter „Bedienung“ steht der Fensterrahmen. Der Klick auf
    /// „Ohne Titelleiste“ schaltet die Dekoration ab (`Decorations(false)`) und
    /// merkt sich die Wahl in den Voreinstellungen — dieselbe Umschaltung wie
    /// beim Start (`with_decorations`), nur ohne Neustart.
    #[test]
    fn fensterrahmen_laesst_sich_umschalten() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("fensterrahmen");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.fensterrahmen = Fensterrahmen::System;
        app.ui.settings_open = true;
        app.ui.settings_kat = 5;
        // Die Seite scrollt: mit dem Suchbegriff steht die Zeile oben im Bild
        // und ist anklickbar (Suchbegriff zugleich mitgeprüft).
        app.ui.settings_suche = "Fensterrahmen".to_string();
        let bild = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        let mut out = ctx.run(bild(), |ctx| app.ui(ctx));
        for _ in 0..3 {
            out = ctx.run(bild(), |ctx| app.ui(ctx));
        }
        let texte: Vec<&str> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        for erwartet in ["Fensterrahmen", "System", "Ohne Titelleiste"] {
            assert!(
                texte.iter().any(|t| *t == erwartet),
                "„{erwartet}“ fehlt im Einstellungsfenster (gezeichnet: {texte:?})"
            );
        }

        let stelle = |out: &egui::FullOutput, text: &str| -> Pos2 {
            out.shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) if t.galley.text() == text => {
                        Some(t.pos + t.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("„{text}“ fehlt im Einstellungsfenster"))
        };
        let knopf = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut cmds = Vec::new();
        for events in [
            vec![egui::Event::PointerMoved(stelle(&out, "Ohne Titelleiste"))],
            vec![knopf(stelle(&out, "Ohne Titelleiste"), true)],
            vec![knopf(stelle(&out, "Ohne Titelleiste"), false)],
            vec![],
        ] {
            let o = ctx.run(
                egui::RawInput {
                    events,
                    ..bild()
                },
                |ctx| app.ui(ctx),
            );
            cmds.extend(
                o.viewport_output
                    .get(&egui::ViewportId::ROOT)
                    .map(|v| v.commands.clone())
                    .unwrap_or_default(),
            );
        }
        assert_eq!(
            app.prefs.fensterrahmen,
            Fensterrahmen::OhneTitelleiste,
            "Klick auf „Ohne Titelleiste“ wurde nicht gespeichert"
        );
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Decorations(false))),
            "der Fenstermanager muss die Dekoration abbestellen (Befehle: {cmds:?})"
        );
    }
}

