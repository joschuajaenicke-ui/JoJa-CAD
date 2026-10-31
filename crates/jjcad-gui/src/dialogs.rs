//! Dialoge: Dateien, Text, Einstellungen, Tastaturbelegung, Hilfe, Projekt.

use crate::app::App;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::geom::{Vec2 as V2};
use jjcad_core::persist::{fest_belegt, DragMode};
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
    ExportPdf,
    ExportDxf,
    ExportPng,
}

impl FileMode {
    pub fn ext(self) -> &'static str {
        match self {
            FileMode::Open | FileMode::SaveJjcad => "jjcad",
            FileMode::ImportDxf | FileMode::ExportDxf => "dxf",
            FileMode::ImportPdf => "pdf",
            FileMode::ExportPdf => "pdf",
            FileMode::ExportPng => "png",
        }
    }
    fn title(self) -> &'static str {
        match self {
            FileMode::Open => "Zeichnung öffnen",
            FileMode::SaveJjcad => "Zeichnung speichern",
            FileMode::ImportDxf => "DXF importieren",
            FileMode::ImportPdf => "PDF einfügen",
            FileMode::ExportPdf => "Als PDF exportieren",
            FileMode::ExportDxf => "Als DXF exportieren",
            FileMode::ExportPng => "Als PNG exportieren",
        }
    }
    fn action_label(self) -> &'static str {
        match self {
            FileMode::Open => "Öffnen",
            FileMode::ImportDxf => "Importieren",
            _ => "Speichern",
        }
    }
    fn is_save(self) -> bool {
        matches!(self, FileMode::SaveJjcad | FileMode::ExportPdf | FileMode::ExportDxf | FileMode::ExportPng)
    }
    /// Anzeigename des Dateifilters (ohne Endung) für den Systemdialog.
    fn filter_name(self) -> &'static str {
        match self {
            FileMode::Open | FileMode::SaveJjcad => "Zeichnung",
            FileMode::ImportDxf | FileMode::ExportDxf => "DXF-Zeichnung",
            FileMode::ImportPdf => "PDF-Dokument",
            FileMode::ExportPdf => "PDF-Dokument",
            FileMode::ExportPng => "PNG-Bild",
        }
    }
    /// Anfrage an den Systemdialog des Betriebssystems.
    pub fn anfrage(self, start_dir: std::path::PathBuf, base: &str) -> crate::systemdialog::Anfrage {
        let name = if base.trim().is_empty() { "Zeichnung" } else { base.trim() };
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
        let base = if base.trim().is_empty() { "Zeichnung" } else { base };
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
                    if widgets::icon_button(ui, "chevron_right", 26.0, false, "Übergeordneter Ordner").clicked() {
                        if let Some(p) = self.dir.parent() {
                            self.dir = p.to_path_buf();
                            self.dirty = true;
                        }
                    }
                    if ui.button("Home").clicked() {
                        if let Ok(h) = std::env::var("HOME") {
                            self.dir = std::path::PathBuf::from(h);
                            self.dirty = true;
                        }
                    }
                    if ui.button("Projekte").clicked() {
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
                                egui::SelectableLabel::new(false, "..  (übergeordnet)"),
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
                    ui.label(theme::dim("Dateiname"));
                    let resp = ui.add_sized(
                        [420.0, 24.0],
                        egui::TextEdit::singleline(&mut self.name).hint_text(format!("name.{}", self.mode.ext())),
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
                    ui.label(RichText::new(err).color(Palette::DANGER).size(11.5));
                }
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.mode.action_label()).clicked() {
                        if let Some(p) = self.resolve_path() {
                            result = Some(p);
                        }
                    }
                    if ui.button("Abbrechen").clicked() {
                        close = true;
                    }
                    if self.mode == FileMode::Open {
                        for (label, dir) in [
                            ("Dokumente", dirs::documents()),
                            ("Schreibtisch", dirs::desktop()),
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
            self.error = Some("Bitte einen Dateinamen angeben".into());
            return None;
        }
        if self.mode.is_save() || self.mode == FileMode::ImportDxf {
            let ext = self.mode.ext();
            if !n.to_lowercase().ends_with(&format!(".{ext}")) {
                n = format!("{n}.{ext}");
            }
        }
        let p = self.dir.join(n);
        if self.mode == FileMode::Open && !p.exists() {
            self.error = Some("Datei existiert nicht".into());
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
        FileMode::SaveJjcad => match app.session_mut().save(&path) {
            Ok(_) => {
                app.prefs.add_recent(&path.to_string_lossy());
                app.prefs.last_dir =
                    path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let _ = app.prefs.save();
                app.toast(format!("Datei · gespeichert: {}", path.display()));
            }
            Err(e) => app.toast(e),
        },
        FileMode::ImportDxf => app.import_dxf_from(path),
        FileMode::ImportPdf => app.pdf_import_oeffnen(path),
        FileMode::ExportPdf => app.export_pdf_to(path),
        FileMode::ExportDxf => app.export_dxf_to(path),
        FileMode::ExportPng => app.export_png_to(path),
    }
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
                        "Der Dateidialog des Systems ist geöffnet. Datei wählen oder abbrechen.",
                    ));
                    ui.label(theme::faint("ESC schließt den Hinweis, wenn das System nicht antwortet."));
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

    // Neuer Block
    if app.ui.block_name_dialog.is_some() {
        dim_background(ctx);
        // Fenster-Flag: startet offen, false = über das X geschlossen.
        let mut win_open = true;
        let mut close = false;
        let mut btn_close = false;
        let mut create = false;
        if let Some((name, _)) = app.ui.block_name_dialog.as_mut() {
            modal_window(ctx, "Block aus Auswahl", 420.0, &mut win_open).show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(theme::dim("Die Auswahl wird zur Blockdefinition und als Instanz eingefügt."));
                ui.add_space(4.0);
                let resp = ui.add_sized([380.0, 24.0], egui::TextEdit::singleline(name).hint_text("Blockname"));
                resp.request_focus();
                ui.add_space(8.0);
                button_row(ui, |ui| {
                    if ui.button("Anlegen").clicked() {
                        create = true;
                    }
                    if ui.button("Abbrechen").clicked() {
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
                    match app.session_mut().execute("block.create", &json!({"name": name, "ids": ids})) {
                        Ok(_) => {
                            app.mark_changed();
                            app.toast(format!("Block '{name}' angelegt"));
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
        modal_window(ctx, "Hotbar-Vorlage", 400.0, &mut win_close).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(theme::dim(if idx.is_some() {
                "Vorlage umbenennen"
            } else {
                "Neue Vorlage aus der aktuellen anlegen"
            }));
            ui.add_space(4.0);
            let resp = ui.add_sized(
                [360.0, 24.0],
                egui::TextEdit::singleline(&mut buf).hint_text("Name der Vorlage"),
            );
            resp.request_focus();
            ui.add_space(8.0);
            button_row(ui, |ui| {
                if ui.button("OK").clicked() {
                    ok = true;
                }
                if ui.button("Abbrechen").clicked() {
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
            app.toast("Vorlage gespeichert");
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
        modal_window(ctx, "Einstellungen", 960.0, &mut open)
            .default_height(660.0)
            .show(ctx, |ui| {
                ui.set_width(940.0);
                settings_body(app, ui);
            });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.settings_open = false;
        }
    }

    // Tastatur
    if app.ui.keymap_open {
        dim_background(ctx);
        let mut open = true;
        modal_window(ctx, "Tastaturbelegung", 860.0, &mut open).show(ctx, |ui| {
            ui.set_width(840.0);
            keyboard_body(app, ui);
            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Werkseinstellungen").clicked() {
                    app.prefs.reset_bindings();
                    let _ = app.prefs.save();
                    app.toast("Tastenbelegung zurückgesetzt");
                }
            });
        });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.keymap_open = false;
        }
    }

    // Hilfe
    if app.ui.help_open {
        dim_background(ctx);
        let mut open = true;
        modal_window(ctx, "Hilfe", 760.0, &mut open).show(ctx, |ui| {
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
        modal_window(ctx, "Projektdaten", 520.0, &mut open).show(ctx, |ui| {
            ui.set_width(500.0);
            ui.label(theme::dim("Diese Angaben füllen das Schriftfeld und dynamische Textfelder."));
            ui.add_space(6.0);
            let mut changed = false;
            for (label, field) in [
                ("Projekt", &mut p.project),
                ("Bauherr", &mut p.client),
                ("Planung / Büro", &mut p.planner),
                ("Adresse", &mut p.address),
                ("Plan-Nr.", &mut p.plan_no),
                ("Planinhalt", &mut p.plan_title),
                ("Revision", &mut p.revision),
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
        modal_window(ctx, "PDF einfügen", 470.0, &mut win_open).show(ctx, |ui| {
            ui.set_width(450.0);
            let name = wahl
                .pfad
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| wahl.pfad.display().to_string());
            ui.label(theme::bold(&name, 13.0));
            ui.label(theme::dim(format!(
                "{} Seite{} · Text, Linien und Flächen werden übernommen",
                wahl.seiten,
                if wahl.seiten == 1 { "" } else { "n" }
            )));
            ui.add_space(8.0);
            widgets::field_row(ui, "Seite", |ui| {
                if wahl.seiten > 1 {
                    let mut s = seite as i64;
                    if ui.add(crate::widgets::zahl(&mut s, 1.0, 1..=wahl.seiten as i64)).changed() {
                        seite = s.clamp(1, wahl.seiten as i64) as usize;
                    }
                    ui.label(theme::dim(format!("von {}", wahl.seiten)));
                } else {
                    ui.label("1");
                }
            });
            ui.add_space(4.0);
            ui.checkbox(&mut als_block, "Als ein Element einfügen (Block, zerlegbar)");
            ui.add_space(6.0);
            ui.label(theme::dim(
                "Bilder, Transparenz, Schattierungen, Muster und Beschnitt übernimmt JoJa-CAD nicht — sie werden gezählt und gemeldet.",
            ));
            ui.add_space(8.0);
            button_row(ui, |ui| {
                if ui.button("Einfügen").clicked() {
                    einfuegen = true;
                }
                if ui.button("Abbrechen").clicked() {
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

    // Neue Zeichnung bestätigen
    if app.ui.confirm_new {
        dim_background(ctx);
        let mut close = false;
        // Fenster-Flag: startet offen, false = über das X geschlossen.
        let mut win_close = true;
        let mut confirmed = false;
        modal_window(ctx, "Neue Zeichnung", 420.0, &mut win_close).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label("Neue Zeichnung anlegen?");
            ui.label(theme::dim("Nicht gespeicherte Änderungen der aktuellen Zeichnung bleiben in ihrem Reiter erhalten."));
            ui.add_space(8.0);
            button_row(ui, |ui| {
                if ui.button("Anlegen").clicked() {
                    confirmed = true;
                }
                if ui.button("Abbrechen").clicked() {
                    close = true;
                }
            });
        });
        if !win_close {
            close = true; // X im Fensterrahmen
        }
        if confirmed {
            app.new_document();
            app.ui.confirm_new = false;
        }
        if close {
            app.ui.confirm_new = false;
        }
    }

    // Info
    if app.ui.about_open {
        dim_background(ctx);
        let mut open = true;
        modal_window(ctx, "Über JoJa-CAD", 480.0, &mut open).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(theme::bold("JoJa-CAD", 20.0));
            ui.label(theme::dim(format!("Version {}", env!("CARGO_PKG_VERSION"))));
            ui.label(theme::mono(crate::app::build_label()));
            ui.add_space(8.0);
            ui.label("2D-CAD für Architektur und Bauantragszeichnungen.");
            ui.add_space(6.0);
            ui.label(theme::dim("Rust · egui · eigener PDF- und DXF-Export · MCP-Schnittstelle für KI-Agenten"));
            ui.add_space(6.0);
            ui.label(theme::dim("Lizenz: MIT · Format: .jjcad (JSON, offen dokumentiert)"));
            ui.add_space(8.0);
            ui.label(theme::dim(format!(
                "Dokument: {} · {}",
                app.doc().name,
                app.doc().summary()
            )));
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

/// Eine Kategorie der Einstellungen: Name, Icon und die Namen der
/// Einstellungen darin.
///
/// Die Stichworte sind die **eine** Quelle der Suche: wer hier eine Einstellung
/// ergänzt, trägt ihren Namen in der Liste der Kategorie ein — sonst findet die
/// Suche die Kategorie nicht und die Einstellung bleibt versteckt (T0061).
pub(crate) struct Einstellungskategorie {
    pub name: &'static str,
    /// Iconname aus `icons::paint`
    icon: &'static str,
    stichworte: &'static [&'static str],
}

impl Einstellungskategorie {
    /// Passt die Kategorie zum (klein geschriebenen) Suchbegriff? Gesucht wird
    /// im Kategorienamen und in den Namen aller Einstellungen der Kategorie.
    fn passt(&self, suche: &str) -> bool {
        if suche.is_empty() {
            return true;
        }
        self.name.to_lowercase().contains(suche)
            || self
                .stichworte
                .iter()
                .any(|w| w.to_lowercase().contains(suche))
    }
}

/// Die Kategorien des Einstellungsfensters in der Reihenfolge der Liste links.
pub(crate) const EINSTELLUNGSKATEGORIEN: &[Einstellungskategorie] = &[
    Einstellungskategorie {
        name: "Einheiten und Maßstab",
        icon: "staerke",
        stichworte: &["Eingabe", "Einheit", "Nachkommastellen", "Maßstab"],
    },
    Einstellungskategorie {
        name: "Raster",
        icon: "grid",
        stichworte: &["Rasterweite", "Raster anzeigen", "Rasterfang", "Raster"],
    },
    Einstellungskategorie {
        name: "Fangen",
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
        name: "Architektur",
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
        name: "Bemaßung",
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
        name: "Bedienung",
        icon: "move",
        stichworte: &["Ziehen", "Maus", "Dateidialog", "Bedienung"],
    },
    Einstellungskategorie {
        name: "Darstellung",
        icon: "eye",
        stichworte: &[
            "Linienstärken",
            "Layer-Panel",
            "Eigenschaften-Panel",
            "Fangmarker",
            "Lineale",
            "Darstellung",
        ],
    },
];

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
            p.rect_filled(rect, Rounding::same(6.0), Palette::SELECT.gamma_multiply(0.85));
        } else if antwort.hovered() {
            p.rect_filled(rect, Rounding::same(6.0), Palette::PANEL_ALT);
        }
        let farbe = if gewaehlt {
            Color32::WHITE
        } else if antwort.hovered() {
            Palette::TEXT
        } else {
            Palette::TEXT_DIM
        };
        let icon = Rect::from_center_size(
            egui::pos2(rect.left() + 18.0, rect.center().y),
            Vec2::splat(16.0),
        );
        crate::icons::paint(p, icon, kat.icon, farbe);
        p.text(
            egui::pos2(rect.left() + 34.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            kat.name,
            FontId::proportional(13.0),
            farbe,
        );
    }
    antwort.clicked()
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
    let mut drag_mode = app.ui.drag_mode;
    let mut show_layers = app.ui.layers_open;
    let mut show_props = app.ui.props_open;
    let mut marker = app.prefs.snap_marker;
    let mut dateidialog_system = app.prefs.dateidialog_system();

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
                    .hint_text("Einstellungen durchsuchen …"),
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
                            ui.label(theme::dim("Keine Einstellung gefunden"));
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
        let (pane_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), SETTINGS_HOEHE), Sense::hover());
        app.ui.last_settings_pane = Some(pane_rect);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(pane_rect)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.label(theme::bold(EINSTELLUNGSKATEGORIEN[kat].name, 18.0));
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .id_salt("einstellungen_seite")
                    .max_height(SETTINGS_HOEHE - 40.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if treffer.is_empty() {
                            ui.label(theme::dim(
                                "Keine Einstellung gefunden — Suchbegriff löschen, um alle \
                                 Kategorien zu sehen.",
                            ));
                            return;
                        }
                        match kat {
                            // ------------------------ Einheiten -------------------
                            0 => {
                                abschnitt(&suche, ui, "Einheiten");
                                zeile(&suche, ui, "Eingabe", |ui| {
                                    for u in Unit::ALL {
                                        ui.selectable_value(&mut input, u, u.short());
                                    }
                                });
                                zeile_mit(
                                    &suche,
                                    ui,
                                    "Dezimalstellen",
                                    &["Nachkommastellen"],
                                    |ui| {
                                        ui.add(crate::widgets::zahl(&mut decimals, 1.0, 0..=4));
                                    },
                                );
                                if suche.is_empty() {
                                    ui.add(
                                        egui::Label::new(theme::faint(
                                            "Maßeinheit, Maßstab, Textgröße und Bogensymbole \
                                             stehen in der Kategorie Bemaßung.",
                                        ))
                                        .wrap(),
                                    );
                                }
                            }
                            // -------------------------- Raster --------------------
                            1 => {
                                abschnitt(&suche, ui, "Raster");
                                zeile(&suche, ui, "Rasterweite", |ui| {
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
                                zeile(&suche, ui, "Raster anzeigen", |ui| {
                                    if ui.checkbox(&mut show_grid, "").changed() {
                                        snap.grid = show_grid;
                                    }
                                });
                                zeile(&suche, ui, "Rasterfang", |ui| {
                                    ui.checkbox(&mut snap.grid, "");
                                });
                            }
                            // -------------------------- Fangen --------------------
                            2 => {
                                abschnitt(&suche, ui, "Fangen");
                                for (label, field) in [
                                    ("Endpunkt", &mut snap.endpoint),
                                    ("Wandkanten", &mut snap.wall_edges),
                                    ("Mittelpunkt", &mut snap.midpoint),
                                    ("Zentrum", &mut snap.center),
                                    ("Schnittpunkt", &mut snap.intersection),
                                    ("Lot", &mut snap.perpendicular),
                                    ("Nächster Punkt", &mut snap.nearest),
                                    ("Ortho", &mut snap.ortho),
                                    ("Polar", &mut snap.polar),
                                ] {
                                    if !suche.is_empty() && !label.to_lowercase().contains(&suche) {
                                        continue;
                                    }
                                    widgets::field_row(ui, label, |ui| {
                                        ui.checkbox(field, "");
                                    });
                                }
                                zeile(&suche, ui, "Polarwinkel", |ui| {
                                    ui.add(
                                        crate::widgets::zahl(&mut polar_step, 1.0, 1.0..=180.0)
                                            .suffix("°"),
                                    );
                                    ui.label(theme::dim("Architektur: 45°"));
                                });
                                zeile(&suche, ui, "Polarfenster", |ui| {
                                    ui.add(
                                        crate::widgets::zahl(&mut polar_tol, 0.5, 0.0..=45.0)
                                            .suffix("°"),
                                    );
                                    ui.label(theme::dim(
                                        "nur innerhalb dieser Abweichung rastet Polar ein",
                                    ));
                                });
                                zeile(&suche, ui, "Fangradius", |ui| {
                                    ui.add(
                                        crate::widgets::zahl(&mut tol_px, 1.0, 1.0..=60.0)
                                            .suffix(" px"),
                                    );
                                });
                            }
                            // ------------------------ Architektur -----------------
                            3 => {
                                abschnitt(&suche, ui, "Architektur");
                                for (label, val, def) in [
                                    ("Wandstärke", &mut wall, 240.0),
                                    ("Türbreite", &mut door, 885.0),
                                    ("Fensterbreite", &mut win, 1200.0),
                                    ("Treppenbreite", &mut stair, 1000.0),
                                    ("Texthöhe", &mut th, 250.0),
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
                                            .add(egui::Button::new("Standard").small())
                                            .on_hover_text(format!("{:.0} mm", def))
                                            .clicked()
                                        {
                                            *val = def;
                                        }
                                    });
                                }
                            }
                            // -------------------------- Bemaßung ------------------
                            4 => {
                                abschnitt(&suche, ui, "Bemaßung");
                                zeile(&suche, ui, "Pfeilgröße", |ui| {
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
                                    "Dezimalstellen",
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
                                abschnitt(&suche, ui, "Bedienung");
                                zeile(&suche, ui, "Ziehen", |ui| {
                                    for m in DragMode::ALL {
                                        ui.selectable_value(&mut drag_mode, m, m.label())
                                            .on_hover_text(m.hint());
                                    }
                                });
                                if suche.is_empty() {
                                    ui.label(theme::faint("Gilt für Auswahl und Griffe"));
                                }
                                zeile(&suche, ui, "Dateidialog", |ui| {
                                    let moeglich = crate::systemdialog::verfuegbar();
                                    ui.selectable_value(&mut dateidialog_system, true, "System")
                                        .on_hover_text(
                                            "Dialog des Betriebssystems (Dolphin, Dateien, Windows)",
                                        );
                                    ui.selectable_value(
                                        &mut dateidialog_system,
                                        false,
                                        "Eingebaut",
                                    )
                                    .on_hover_text("Dialog von JoJa-CAD");
                                    if !moeglich {
                                        ui.label(theme::dim("kein Systemdialog verfügbar"));
                                    }
                                });
                            }
                            // ------------------------ Darstellung -----------------
                            _ => {
                                abschnitt(&suche, ui, "Darstellung");
                                zeile(&suche, ui, "Linienstärken", |ui| {
                                    let mut m = if lw_scaled { 1 } else { 0 };
                                    ui.selectable_value(&mut m, 0, "px");
                                    ui.selectable_value(&mut m, 1, "DPI");
                                    lw_scaled = m == 1;
                                });
                                // Häkchen mit vollem Text — die Labelspalte der
                                // Zeilen (96 px) schnitte „Eigenschaften-Panel“ ab.
                                for (name, wert) in [
                                    ("Layer-Panel", &mut show_layers),
                                    ("Eigenschaften-Panel", &mut show_props),
                                    ("Fangmarker", &mut marker),
                                    ("Lineale einblenden", &mut rulers),
                                ] {
                                    if !suche.is_empty() && !name.to_lowercase().contains(&suche) {
                                        continue;
                                    }
                                    ui.add_space(2.0);
                                    ui.checkbox(wert, name);
                                }
                            }
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
    let dialog_neu = if dateidialog_system { "system" } else { "intern" };
    if app.prefs.file_dialog != dialog_neu {
        app.prefs.file_dialog = dialog_neu.to_string();
        let _ = app.prefs.save();
    }
    app.prefs.lineweights_scaled = lw_scaled;
    app.ui.layers_open = show_layers;
    app.ui.props_open = show_props;
    app.prefs.snap_marker = marker;
    if show_grid != app.ui.show_grid {
        app.ui.show_grid = show_grid;
    }

    let settings_changed = input != cur.input_unit
        || decimals as usize != cur.decimals
        || (grid_step - cur.snap.grid_step).abs() > 1e-9
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
/// Fläche gesperrter Tasten: flaches Grau, deutlich heller als die freien
/// Tasten (`BG`) — der Unterschied ist die Fläche, nicht nur die Schrift.
const GESPERRT_FLAECHE: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x36);
/// Beschriftung gesperrter Tasten: schwächer als `TEXT_FAINT`.
const GESPERRT_TEXT: Color32 = Color32::from_rgb(0x5C, 0x64, 0x6E);

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

fn keyboard_body(app: &mut App, ui: &mut Ui) {
    // Der Hinweis nennt die drei Schichten aus `docs/TASTEN.md`. Er läuft über
    // die ganze Breite und bricht um — in einer Zeile ragte er über den Rand
    // des Dialogs hinaus.
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::dim(
            "Taste anklicken, dann Funktion aus der Liste zuweisen. Buchstaben und \
             Symbole sind frei belegbar; Ziffern gehören der Hotbar; Steuerungstasten \
             und Kombinationen sind fest verdrahtet und ausgegraut.",
        ));
    });
    ui.add_space(6.0);

    let (rahmen, _) = ui.allocate_exact_size(tastatur_flaeche(), Sense::hover());
    let tasten = tasten_rechtecke(rahmen.min);
    app.ui.last_tasten = Some(tasten.clone());
    for (i, (key, label, zeile, spalte, breite, unten)) in TASTEN.iter().enumerate() {
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
                Palette::ACCENT_SOFT
            } else if gesperrt.is_some() {
                // Gesperrte Tasten sind eine eigene graue Fläche — nicht nur
                // eine schwächere Schrift. Das Schloss sagt warum.
                GESPERRT_FLAECHE
            } else if hovered {
                Palette::PANEL_ALT
            } else {
                Palette::BG
            };
            let strich = Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected {
                    Palette::ACCENT
                } else if action.is_some() {
                    Palette::BORDER
                } else {
                    Palette::BORDER_SOFT
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
                    GESPERRT_TEXT
                } else {
                    Palette::TEXT_FAINT
                },
            );
            if let Some(a) = &action {
                if let Some(def) = actions::action(a) {
                    crate::icons::paint(
                        p,
                        Rect::from_center_size(
                            rect.center() - Vec2::new(0.0, 3.0),
                            Vec2::splat(16.0),
                        ),
                        def.icon,
                        if selected { Palette::ACCENT } else { Palette::TEXT_DIM },
                    );
                }
            } else if gesperrt.is_some() {
                crate::icons::paint(
                    p,
                    Rect::from_center_size(rect.center() - Vec2::new(0.0, 3.0), Vec2::splat(14.0)),
                    "lock",
                    GESPERRT_TEXT,
                );
            }
        }
        if resp.clicked() && gesperrt.is_none() {
            app.ui.keymap_selected = Some(key.to_string());
        }
        match gesperrt {
            // Hinweis auf den Grund: Ziffern gehören der Hotbar, die übrigen
            // Steuerungstasten dem Ablauf (Esc/Enter/Rücktaste usw.).
            Some(grund) => {
                resp.on_hover_text(grund.grund());
            }
            None => {
                if let Some(a) = action {
                    resp.on_hover_text(
                        actions::action(&a)
                            .map(|d| format!("{} — {}", d.label, d.hint))
                            .unwrap_or(a),
                    );
                }
            }
        }
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if let Some(key) = app.ui.keymap_selected.clone() {
            ui.label(theme::mono(format!("Taste: {key}")));
            if ui.button("Belegung dieser Taste entfernen").clicked() {
                if let Some(a) = app.prefs.action_for_key(&key) {
                    app.prefs.bindings.remove(&a);
                    let _ = app.prefs.save();
                }
            }
            ui.add_space(10.0);
            ui.add_sized(
                [200.0, 22.0],
                egui::TextEdit::singleline(&mut app.ui.search).hint_text("Funktion suchen …"),
            );
        } else {
            ui.label(theme::dim("Noch keine Taste gewählt."));
        }
    });
    ui.add_space(4.0);
    if let Some(key) = app.ui.keymap_selected.clone() {
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            let search = app.ui.search.to_lowercase();
            for def in actions::ACTIONS {
                if !search.is_empty()
                    && !def.label.to_lowercase().contains(&search)
                    && !def.id.contains(&search)
                {
                    continue;
                }
                let current = app.prefs.key_for(def.id);
                // Funktionen mit fest verdrahteter Taste (Strg+S, F1 …) sind
                // nicht umbelegbar — sie stehen als Hinweis in der Liste, aber
                // ohne Klickziel (T0056, `docs/TASTEN.md`).
                let aenderbar = app.prefs.belegung_aenderbar(def.id);
                let label = format!(
                    "{}{}",
                    def.label,
                    current
                        .as_ref()
                        .map(|k| format!("   ({})", k.to_uppercase()))
                        .unwrap_or_default()
                );
                if !aenderbar {
                    ui.add_enabled(
                        false,
                        egui::SelectableLabel::new(false, format!("{label}   · fest")),
                    )
                    .on_disabled_hover_text(
                        "Tastenkombination oder Funktionstaste — im Programm fest vergeben",
                    );
                    continue;
                }
                if ui
                    .selectable_label(
                        app.prefs.action_for_key(&key).as_deref() == Some(def.id),
                        label,
                    )
                    .clicked()
                {
                    app.prefs.bind(def.id, &key);
                    let _ = app.prefs.save();
                    app.toast(format!("{} → {}", def.label, key.to_uppercase()));
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Hilfe
// ---------------------------------------------------------------------------

fn help_body(app: &mut App, ui: &mut Ui) {
    ui.label(theme::bold("Bedienung", 15.0));
    ui.add_space(4.0);
    for (k, v) in [
        ("Zeichnen", "Werkzeug wählen (Ribbon, Hotbar, Taste) und Punkte klicken. ESC bricht ab."),
        ("Navigation", "Mausrad zoomt, mittlere Maustaste oder Leertaste+Ziehen schwenkt. Die Tasten zum Schwenken (Vorgabe W/A/S/D) sind Funktionen der Tastaturbelegung."),
        ("Fangen", "Endpunkte, Mittelpunkte, Schnittpunkte und Raster werden automatisch gefangen (Statuszeile zeigt den Fangtyp)."),
        ("Eingabe", "Nach dem ersten Klick Länge tippen und Enter – oder \"x;y\" für absolute Koordinaten (Semikolon als Trenner, Komma ist Dezimalzeichen)."),
        ("Hotbar", "Tasten 1–0 starten die Slots. Aktives Werkzeug + Klick auf leeren Slot = Verknüpfung."),
        ("Tasten", "Drei Schichten: Ziffern 1–0 = Hotbar, Buchstaben und Symbole = Tastaturbelegung (frei änderbar), Steuerungstasten und Kombinationen (Esc, F1–F12, Strg+C) = fest verdrahtet."),
        ("Inventar", "E öffnet und schließt Lager, Blockbibliothek und Funktionsliste. Alles ist per Drag & Drop auf die Hotbar ziehbar."),
        ("Auswahl", "Klick wählt, Ziehen von links nach rechts = Fenster, von rechts nach links = Querfenster, Shift ergänzt."),
        ("Blätter", "Reiter oben wechseln zwischen Modell und Blättern; PDF exportiert das aktuelle Blatt."),
    ] {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(110.0, 18.0), Layout::left_to_right(Align::Center), |ui| {
                ui.label(theme::bold(k, 11.5));
            });
            ui.label(theme::dim(v));
        });
    }
    ui.add_space(10.0);
    ui.label(theme::bold("Tastenbelegung (Werkseinstellung)", 15.0));
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
                        .color(Palette::TEXT_FAINT),
                );
                ui.label("");
                ui.end_row();
                for a in actions::actions_in_group(g) {
                    if let Some(k) = app.prefs.key_for(a.id) {
                        ui.label(theme::mono(k.to_uppercase()));
                        ui.label(theme::dim(a.label));
                        ui.end_row();
                    }
                }
            }
        });
    ui.add_space(10.0);
    ui.label(theme::dim(format!(
        "JoJa-CAD {} · {} Aktionen · {} Blöcke in der Bibliothek",
        env!("CARGO_PKG_VERSION"),
        actions::ACTIONS.len(),
        app.doc().blocks.len()
    )));
    ui.add_space(6.0);
    if ui.button("Über JoJa-CAD").clicked() {
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
    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| show(app, ctx));
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

        app.ui.confirm_new = true;
        frame(&ctx, &mut app, vec![]);
        assert!(fenster(&ctx, "Neue Zeichnung"));
        app.ui.confirm_new = false;
        frame(&ctx, &mut app, vec![]);
        assert!(!fenster(&ctx, "Neue Zeichnung"));
        assert!(!abdunklung(&ctx));
    }
}

#[cfg(test)]
mod tests_tastatur {
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
    fn naechste_zeit() -> f64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ZAEHLER: AtomicU64 = AtomicU64::new(0);
        ZAEHLER.fetch_add(1, Ordering::Relaxed) as f64 * 0.25
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

    fn app_mit_tastatur() -> (App, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
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
        assert_eq!(farbe("7"), Some(GESPERRT_TEXT), "Ziffer 7 ist nicht ausgegraut");
        assert_eq!(farbe("Esc"), Some(GESPERRT_TEXT), "Esc ist nicht ausgegraut");
        assert_eq!(
            farbe("Q"),
            Some(Palette::TEXT_FAINT),
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
            Some(GESPERRT_FLAECHE),
            "Ziffer 7 hat keine eigene Fläche"
        );
        assert_eq!(flaeche(esc), Some(GESPERRT_FLAECHE), "Esc hat keine eigene Fläche");
        assert_eq!(flaeche(q), Some(Palette::BG), "freie Taste hat eine fremde Fläche");
        // Der Unterschied ist auch als Helligkeit messbar, nicht nur im Namen.
        let hell = |c: Color32| c.r() as i32 + c.g() as i32 + c.b() as i32;
        assert!(
            hell(GESPERRT_FLAECHE) - hell(Palette::BG) > 40,
            "die graue Fläche hebt sich zu wenig von der freien Taste ab"
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

    #[test]
    fn jede_kategorie_hat_name_icon_und_stichworte() {
        assert_eq!(EINSTELLUNGSKATEGORIEN.len(), 7, "Kategorienzahl");
        for k in EINSTELLUNGSKATEGORIEN {
            assert!(!k.name.is_empty(), "Kategorie ohne Namen");
            assert!(!k.icon.is_empty(), "Kategorie „{}“ ohne Icon", k.name);
            assert!(
                k.stichworte.len() >= 2,
                "Kategorie „{}“ hat zu wenige Stichworte — die Suche findet sie nicht",
                k.name
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
                .map(|k| k.name)
                .collect()
        };
        assert_eq!(treffer(""), EINSTELLUNGSKATEGORIEN.iter().map(|k| k.name).collect::<Vec<_>>());
        assert_eq!(treffer("polarwinkel"), vec!["Fangen"]);
        assert_eq!(treffer("Wandstärke"), vec!["Architektur"]);
        assert_eq!(treffer("dateidialog"), vec!["Bedienung"]);
        assert!(treffer("gibtesnicht").is_empty(), "Unbekannter Begriff liefert Treffer");
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
            EINSTELLUNGSKATEGORIEN[app.ui.settings_kat].name,
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
}
