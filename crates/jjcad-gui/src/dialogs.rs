//! Dialoge: Dateien, Text, Einstellungen, Tastaturbelegung, Hilfe, Projekt.

use crate::app::App;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::geom::{Vec2 as V2};
use jjcad_core::persist::DragMode;
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
    ExportPdf,
    ExportDxf,
    ExportPng,
}

impl FileMode {
    pub fn ext(self) -> &'static str {
        match self {
            FileMode::Open | FileMode::SaveJjcad => "jjcad",
            FileMode::ImportDxf | FileMode::ExportDxf => "dxf",
            FileMode::ExportPdf => "pdf",
            FileMode::ExportPng => "png",
        }
    }
    fn title(self) -> &'static str {
        match self {
            FileMode::Open => "Zeichnung öffnen",
            FileMode::SaveJjcad => "Zeichnung speichern",
            FileMode::ImportDxf => "DXF importieren",
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
// Textdialog
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TextDialog {
    pub pos: V2,
    pub text: String,
    pub height: f64,
    pub angle: f64,
    pub layer: jjcad_core::model::Id,
    pub editing_id: Option<jjcad_core::model::Id>,
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

    // Text
    if app.ui.text_dialog.is_some() {
        dim_background(ctx);
        let mut done = false;
        // `win_open` steuert nur das X im Fensterrahmen (false = geschlossen).
        // Es muss offen starten: mit false zeichnet egui den Dialog überhaupt
        // nicht, sichtbar bleibt allein die Abdunklung des Hintergrunds.
        let mut win_open = true;
        let mut cancel = false;
        let mut btn_cancel = false;
        let mut apply: Option<(String, f64, f64)> = None;
        let text_unit = app.doc().settings.input_unit;
        if let Some(td) = app.ui.text_dialog.as_mut() {
            modal_window(ctx, "Text", 460.0, &mut win_open).show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(theme::dim("Text (Platzhalter wie {DATUM} werden ersetzt)"));
                let resp = ui.add_sized(
                    [440.0, 90.0],
                    egui::TextEdit::multiline(&mut td.text).hint_text("Text eingeben …"),
                );
                resp.request_focus();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(theme::dim("Höhe"));
                    widgets::length_input(ui, &mut td.height, text_unit, 90.0);
                    ui.add_space(8.0);
                    ui.label(theme::dim("Drehung"));
                    let mut a = td.angle.to_degrees();
                    if ui.add(crate::widgets::dv(&mut a).speed(1.0).suffix("°")).changed() {
                        td.angle = a.to_radians();
                    }
                });
                ui.add_space(8.0);
                button_row(ui, |ui| {
                    if ui.button("Einfügen").clicked() && !td.text.trim().is_empty() {
                        apply = Some((td.text.clone(), td.height, td.angle));
                        done = true;
                    }
                    if ui.button("Abbrechen").clicked() {
                        btn_cancel = true;
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command) && !td.text.trim().is_empty() {
                    apply = Some((td.text.clone(), td.height, td.angle));
                    done = true;
                }
            });
            if btn_cancel {
                cancel = true;
            }
        }
        if !win_open {
            cancel = true; // X im Fensterrahmen
        }
        if let Some((text, h, angle)) = apply {
            let unit = app.unit();
            let td = app.ui.text_dialog.clone();
            if let Some(td) = td {
                let neu = td.editing_id.is_none();
                let res = match td.editing_id {
                    Some(id) => app.session_mut().execute(
                        "modify.set",
                        &json!({"ids": [id], "text": text, "height": unit.from_mm(h)}),
                    ),
                    None => app.session_mut().execute(
                        "draw.text",
                        &json!({"p": [unit.from_mm(td.pos.x), unit.from_mm(td.pos.y)],
                                "text": text, "height": unit.from_mm(h), "angle": angle.to_degrees()}),
                    ),
                };
                match res {
                    Ok(_) => {
                        app.mark_changed();
                        // Chat-Schema: <Objekt> · <Zustand>
                        app.mitteilung(if neu { "Text · gesetzt" } else { "Text · geändert" });
                        app.options.text = text.clone();
                        app.options.text_height = h;
                    }
                    Err(e) => app.toast(e),
                }
            }
            app.ui.text_dialog = None;
        }
        if done || cancel {
            app.ui.text_dialog = None;
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
        modal_window(ctx, "Einstellungen", 880.0, &mut open)
            .default_height(640.0)
            .show(ctx, |ui| {
                ui.set_width(860.0);
                egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
                    settings_body(app, ui);
                });
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
    let mut lw = app.ui.lineweights;
    let mut lw_scaled = app.ui.lineweights_scaled;
    let mut drag_mode = app.ui.drag_mode;
    let mut show_layers = app.ui.layers_open;
    let mut show_props = app.ui.props_open;
    let mut marker = app.prefs.snap_marker;
    let mut dateidialog_system = app.prefs.dateidialog_system();

    ui.columns(2, |cols| {
        // ----------------------------- links -----------------------------
        {
            let ui = &mut cols[0];
            theme::section(ui, "Einheiten und Maßstab");
            widgets::field_row(ui, "Eingabe", |ui| {
                for u in Unit::ALL {
                    ui.selectable_value(&mut input, u, u.short());
                }
            });
            widgets::field_row(ui, "Nachkommastellen", |ui| {
                ui.add(crate::widgets::dv(&mut decimals).range(0..=4));
            });
            // Maßketten-Werte gibt es nur an einer Stelle: rechts im Abschnitt
            // „Bemaßung" dieses Fensters. Hier steht nur der Weg dorthin.
            ui.add(
                egui::Label::new(theme::faint(
                    "Maßeinheit, Maßstab, Textgröße und Bogensymbole stehen rechts \
                     im Abschnitt Bemaßung.",
                ))
                .wrap(),
            );

            theme::section(ui, "Raster");
            widgets::field_row(ui, "Rasterweite", |ui| {
                // Dieselben Grenzen wie im Eigenschaften-Panel (1 mm … 10 m).
                let mut g = grid_step;
                if widgets::length_input_between(
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
            widgets::field_row(ui, "Raster anzeigen", |ui| {
                if ui.checkbox(&mut show_grid, "").changed() {
                    snap.grid = show_grid;
                }
            });
            widgets::field_row(ui, "Rasterfang", |ui| {
                ui.checkbox(&mut snap.grid, "");
            });

            theme::section(ui, "Fangen");
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
                widgets::field_row(ui, label, |ui| {
                    ui.checkbox(field, "");
                });
            }
            widgets::field_row(ui, "Polarwinkel", |ui| {
                ui.add(
                    crate::widgets::dv(&mut polar_step)
                        .speed(1.0)
                        .range(1.0..=180.0)
                        .suffix("°"),
                );
                ui.label(theme::dim("Architektur: 45°"));
            });
            widgets::field_row(ui, "Polarfenster", |ui| {
                ui.add(
                    crate::widgets::dv(&mut polar_tol)
                        .speed(0.5)
                        .range(0.0..=45.0)
                        .suffix("°"),
                );
                ui.label(theme::dim("nur innerhalb dieser Abweichung rastet Polar ein"));
            });
            widgets::field_row(ui, "Fangradius", |ui| {
                ui.add(
                    crate::widgets::dv(&mut tol_px)
                        .speed(1.0)
                        .range(1.0..=60.0)
                        .suffix(" px"),
                );
            });
        }

        // ----------------------------- rechts ----------------------------
        {
            let ui = &mut cols[1];
            theme::section(ui, "Architektur");
            for (label, val, def) in [
                ("Wandstärke", &mut wall, 240.0),
                ("Türbreite", &mut door, 885.0),
                ("Fensterbreite", &mut win, 1200.0),
                ("Treppenbreite", &mut stair, 1000.0),
                ("Texthöhe", &mut th, 250.0),
            ] {
                widgets::field_row(ui, label, |ui| {
                    let mut v = *val;
                    if widgets::length_input(ui, &mut v, unit, 90.0) {
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

            theme::section(ui, "Bemaßung");
            widgets::field_row(ui, "Pfeilgröße", |ui| {
                if ui
                    .add(
                        crate::widgets::dv(&mut ds.arrow_size)
                            .speed(0.1)
                            .suffix(" mm"),
                    )
                    .changed()
                {
                    ds_changed = true;
                }
            });
            widgets::field_row(ui, "Nachkommastellen", |ui| {
                let mut d = ds.decimals as i32;
                if ui.add(crate::widgets::dv(&mut d).range(0..=4)).changed() {
                    ds.decimals = d.max(0) as usize;
                    ds_changed = true;
                }
            });
            // Maßeinheit, Maßstab, Textgröße und Bogensymbole: dieselbe
            // Funktion wie im Eigenschaften-Panel zuvor — ein Wert, eine Stelle.
            crate::panels::dim_fields(app, ui);

            theme::section(ui, "Bedienung");
            widgets::field_row(ui, "Ziehen", |ui| {
                for m in DragMode::ALL {
                    ui.selectable_value(&mut drag_mode, m, m.label())
                        .on_hover_text(m.hint());
                }
            });
            ui.label(theme::faint("Gilt für Auswahl und Griffe"));
            widgets::field_row(ui, "Dateidialog", |ui| {
                let moeglich = crate::systemdialog::verfuegbar();
                ui.selectable_value(&mut dateidialog_system, true, "System")
                    .on_hover_text("Dialog des Betriebssystems (Dolphin, Dateien, Windows)");
                ui.selectable_value(&mut dateidialog_system, false, "Eingebaut")
                    .on_hover_text("Dialog von JoJa-CAD");
                if !moeglich {
                    ui.label(theme::dim("kein Systemdialog verfügbar"));
                }
            });

            theme::section(ui, "Darstellung");
            widgets::field_row(ui, "Linienstärken", |ui| {
                let mut m = if lw_scaled { 1 } else { 0 };
                ui.selectable_value(&mut m, 0, "px");
                ui.selectable_value(&mut m, 1, "DPI");
                lw_scaled = m == 1;
            });
            ui.checkbox(&mut show_layers, "Layer-Panel");
            ui.checkbox(&mut show_props, "Eigenschaften-Panel");
            ui.checkbox(&mut marker, "Fangmarker");
            ui.checkbox(&mut rulers, "Lineale einblenden");
        }
    });

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

const ROWS: &[&[(&str, &str, f32)]] = &[
    &[
        ("esc", "Esc", 1.0),
        ("1", "1", 1.0),
        ("2", "2", 1.0),
        ("3", "3", 1.0),
        ("4", "4", 1.0),
        ("5", "5", 1.0),
        ("6", "6", 1.0),
        ("7", "7", 1.0),
        ("8", "8", 1.0),
        ("9", "9", 1.0),
        ("0", "0", 1.0),
        ("backspace", "Bksp", 1.6),
    ],
    &[
        ("tab", "Tab", 1.4),
        ("q", "Q", 1.0),
        ("w", "W", 1.0),
        ("e", "E", 1.0),
        ("r", "R", 1.0),
        ("t", "T", 1.0),
        ("z", "Z", 1.0),
        ("u", "U", 1.0),
        ("i", "I", 1.0),
        ("o", "O", 1.0),
        ("p", "P", 1.0),
        ("ü", "Ü", 1.0),
    ],
    &[
        ("caps", "Caps", 1.7),
        ("a", "A", 1.0),
        ("s", "S", 1.0),
        ("d", "D", 1.0),
        ("f", "F", 1.0),
        ("g", "G", 1.0),
        ("h", "H", 1.0),
        ("j", "J", 1.0),
        ("k", "K", 1.0),
        ("l", "L", 1.0),
        ("ö", "Ö", 1.0),
        ("ä", "Ä", 1.0),
    ],
    &[
        ("shift", "Shift", 2.0),
        ("y", "Y", 1.0),
        ("x", "X", 1.0),
        ("c", "C", 1.0),
        ("v", "V", 1.0),
        ("b", "B", 1.0),
        ("n", "N", 1.0),
        ("m", "M", 1.0),
        (",", ",", 1.0),
        (".", ".", 1.0),
        ("-", "-", 1.0),
    ],
    &[
        ("ctrl", "Strg", 1.6),
        ("alt", "Alt", 1.4),
        ("space", "Leertaste", 6.0),
        ("enter", "Enter", 1.8),
    ],
];

fn keyboard_body(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(theme::dim(
            "Taste anklicken, dann Funktion aus der Liste zuweisen. Jede Taste ist ein Item-Slot.",
        ));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(k) = app.ui.keymap_selected.clone() {
                ui.label(theme::mono(format!("Taste: {k}")));
            }
        });
    });
    ui.add_space(6.0);

    let key_w = 46.0;
    let key_h = 42.0;
    let gap = 4.0;
    for row in ROWS {
        ui.horizontal(|ui| {
            for (key, label, wf) in *row {
                let w = key_w * wf + gap * (wf - 1.0);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, key_h), Sense::click());
                let action = app.prefs.action_for_key(key);
                let selected = app.ui.keymap_selected.as_deref() == Some(*key);
                let hovered = resp.hovered();
                if ui.is_rect_visible(rect) {
                    let p = ui.painter();
                    p.rect_filled(
                        rect,
                        Rounding::same(6.0),
                        if selected {
                            Palette::ACCENT_SOFT
                        } else if hovered {
                            Palette::PANEL_ALT
                        } else {
                            Palette::BG
                        },
                    );
                    p.rect_stroke(
                        rect,
                        Rounding::same(6.0),
                        Stroke::new(
                            if selected { 1.5 } else { 1.0 },
                            if selected {
                                Palette::ACCENT
                            } else if action.is_some() {
                                Palette::BORDER
                            } else {
                                Palette::BORDER_SOFT
                            },
                        ),
                    );
                    p.text(
                        Pos2::new(rect.left() + 5.0, rect.top() + 4.0),
                        egui::Align2::LEFT_TOP,
                        *label,
                        FontId::proportional(9.0),
                        Palette::TEXT_FAINT,
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
                    }
                }
                if resp.clicked() {
                    app.ui.keymap_selected = Some(key.to_string());
                }
                if let Some(a) = action {
                    resp.on_hover_text(
                        actions::action(&a)
                            .map(|d| format!("{} — {}", d.label, d.hint))
                            .unwrap_or(a),
                    );
                }
            }
        });
        ui.add_space(gap);
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if let Some(key) = app.ui.keymap_selected.clone() {
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
                let label = format!(
                    "{}{}",
                    def.label,
                    current
                        .as_ref()
                        .map(|k| format!("   ({})", k.to_uppercase()))
                        .unwrap_or_default()
                );
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
        ("Navigation", "Mausrad zoomt, mittlere Maustaste oder Leertaste+Ziehen schwenkt, W/A/S/D schwenkt ebenfalls."),
        ("Fangen", "Endpunkte, Mittelpunkte, Schnittpunkte und Raster werden automatisch gefangen (Statuszeile zeigt den Fangtyp)."),
        ("Eingabe", "Nach dem ersten Klick Länge tippen und Enter – oder \"x;y\" für absolute Koordinaten (Semikolon als Trenner, Komma ist Dezimalzeichen)."),
        ("Hotbar", "Tasten 1–0 starten die Slots. Aktives Werkzeug + Klick auf leeren Slot = Verknüpfung."),
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
    fn textdialog_wird_gezeichnet_und_nimmt_einfuegen_an() {
        let (mut app, ctx) = leeres_app();
        let layer = app.doc().current_layer;
        app.ui.text_dialog = Some(TextDialog {
            pos: V2::new(10.0, 20.0),
            text: String::new(),
            height: 250.0,
            angle: 0.0,
            layer,
            editing_id: None,
        });

        frame(&ctx, &mut app, vec![]);
        assert!(
            fenster(&ctx, "Text"),
            "Textdialog muss im ersten Frame gezeichnet werden"
        );
        assert!(
            abdunklung(&ctx),
            "der modale Dialog dunkelt den Hintergrund ab"
        );
        assert!(
            ctx.memory(|m| m.focused()).is_some(),
            "das Textfeld im Dialog muss den Fokus bekommen"
        );
        // Das Fenster darf nicht Frame für Frame auf die halbe Bildschirmhöhe
        // wachsen (egui dehnt eine ungebundene Knopfzeile sonst aus).
        let mut hoehen = Vec::new();
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
            hoehen.push(ctx.memory(|m| m.area_rect(Id::new("Text"))).unwrap().height());
        }
        assert!(
            hoehen.windows(2).all(|w| (w[0] - w[1]).abs() < 1.0),
            "Dialoghöhe muss stabil bleiben ({hoehen:?})"
        );
        assert!(hoehen[0] < 400.0, "Textdialog bleibt kompakt ({hoehen:?})");

        // Zweimal hintereinander aus der Zwischenablage einfügen (Strg+V).
        frame(&ctx, &mut app, vec![egui::Event::Paste("HALLO".into())]);
        frame(&ctx, &mut app, vec![egui::Event::Paste(" WELT".into())]);
        let text = app.ui.text_dialog.as_ref().expect("Dialog bleibt offen").text.clone();
        assert_eq!(text, "HALLO WELT", "Einfügen landet im Textfeld des Dialogs");

        // Dialog schließen: Fenster und Abdunklung sind danach weg.
        app.ui.text_dialog = None;
        frame(&ctx, &mut app, vec![]);
        assert!(!fenster(&ctx, "Text"));
        assert!(!abdunklung(&ctx), "ohne Dialog bleibt keine Abdunklung stehen");
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
