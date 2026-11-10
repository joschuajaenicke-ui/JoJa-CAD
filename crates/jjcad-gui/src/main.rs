//! JoJa-CAD — Desktop-Anwendung.
//!
//! 2D-CAD für Architektur und Bauantragszeichnungen.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bildablage;
mod canvas;
mod chat;
mod diag;
mod dialogs;
mod ghost;
mod hotbar;
mod handles;
mod icon;
mod icons;
mod inventory;
mod panels;
mod perf;
mod props;
mod systemdialog;
mod textedit;
mod textgroesse;
mod theme;
mod widgets;

use jjcad_core::daten;
use jjcad_core::instanz;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (daten_arg, datei) = argumente(&args);

    // T0127: Die Wurzel steht vor allem anderen fest — Einstellungen, Blöcke,
    // Sicherungen und Protokolle liegen darin.
    let wurzel = daten::wurzel_ermitteln_aus_umgebung(daten_arg.as_deref());
    daten::setze_wurzel(wurzel);
    if let Err(e) = daten::vorbereiten() {
        eprintln!("Datenordner: {e}");
    }

    // T0135: Läuft schon eine Instanz auf dieser Wurzel, übernimmt sie die
    // Arbeit — Zeichnung als Reiter öffnen, sonst Fenster nach vorn holen.
    // Der zweite Prozess endet sofort, statt eine Meldung zu zeigen.
    let auftrag = match &datei {
        Some(d) => instanz::Auftrag::Oeffnen(PathBuf::from(d)),
        None => instanz::Auftrag::Zeigen,
    };
    // Prüfläufe (`JJCAD_DEMO_UI`) sind keine Arbeit des Benutzers: sie übergeben
    // nichts an ein laufendes Fenster, sondern starten wie bisher.
    if !app::ist_demo_start() && instanz::melden(&daten::wurzel(), &auftrag) {
        return Ok(());
    }

    // Bleibt die Sperre: eine Fassung ohne Kanal (alt oder abgestürzt) würde
    // Sitzung und Sicherungs-Manifest überschreiben — sie bekommt den Hinweis.
    match daten::sperre_nehmen() {
        Ok(sperre) => starte_app(datei, sperre),
        Err(konflikt) => starte_hinweis(konflikt),
    }
}

/// Zerlegt die Kommandozeile: `--daten-dir <pfad>` (T0127) und die optional
/// übergebene Zeichnung. Alles andere bleibt unberücksichtigt.
fn argumente(args: &[String]) -> (Option<PathBuf>, Option<String>) {
    let mut daten: Option<PathBuf> = None;
    let mut datei: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--daten-dir" {
            if let Some(p) = args.get(i + 1) {
                daten = Some(PathBuf::from(p));
                i += 2;
                continue;
            }
        } else if let Some(p) = a.strip_prefix("--daten-dir=") {
            if !p.is_empty() {
                daten = Some(PathBuf::from(p));
            }
        } else if !a.starts_with("--") && datei.is_none() {
            datei = Some(a.clone());
        }
        i += 1;
    }
    (daten, datei)
}

fn starte_app(file: Option<String>, sperre: daten::Sperre) -> eframe::Result<()> {
    // T0098: Der Fensterrahmen steht schon vor dem Fenster fest — ob der
    // Fenstermanager eine Titelleiste zeichnet, entscheidet die Erzeugung
    // (`with_decorations`), nicht ein späterer Befehl.
    let prefs = jjcad_core::persist::UserPrefs::load();
    let titelleiste = !prefs.fensterrahmen.ohne_titelleiste();

    // T0130: Größe und Lage des Fensters stehen ebenfalls vor dem Fenster fest;
    // sie kommen aus `sitzung.json`. Fehlt die Datei, bleibt die Vorgabe.
    let sitzung = if app::ist_demo_start() {
        // Prüfbild: die Reiter des Nutzers bleiben unangetastet.
        None
    } else {
        match jjcad_core::sitzung::laden(&daten::sitzung_datei()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e} — es wird mit einer leeren Zeichnung gestartet");
                None
            }
        }
    };

    // T0147: Ohne Titelleiste zeichnet die Anwendung den Fensterschatten
    // selbst. Dafür bleibt ein Streifen am Fensterrand durchsichtig — das
    // Fenster braucht einen Alphakanal (`with_transparent`), und die
    // Mindestgröße wächst um den Streifen, damit die Oberfläche darin noch
    // ihre Mindestbreite hat.
    let schatten = if titelleiste { 0.0 } else { 2.0 * app::SCHATTEN_RAND };
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1500.0 + schatten, 950.0 + schatten])
        .with_min_inner_size([1080.0 + schatten, 680.0 + schatten])
        .with_title(app::window_title())
        // Ohne Titelleiste: Knöpfe und Ziehen zeichnet die Anwendung selbst
        .with_decorations(titelleiste)
        // T0147: Der Fensterschatten liegt im durchsichtigen Rand. Der
        // Alphakanal wird **immer** angefordert, nicht nur im Modus „Ohne
        // Titelleiste“: winit kann die Transparenz im Betrieb nicht
        // nachrüsten, ein Wechsel der Einstellung wirkte sonst erst nach einem
        // Neustart. Mit Titelleiste deckt die Oberfläche das Fenster ganz ab.
        .with_transparent(true)
        // Dateien dürfen ins Fenster gezogen werden (T0115). Unter
        // Windows ist das standardmäßig aus, deshalb ausdrücklich an.
        .with_drag_and_drop(true)
        // Fenstername für Taskleiste und Startmenü (KDE/GNOME ordnen das
        // Fenster über den .desktop-Eintrag „jjcad“ zu)
        .with_app_id("jjcad")
        .with_icon(icon::fenster_icon());
    if let Some(f) = sitzung.as_ref().and_then(|s| s.fenster) {
        if f.brauchbar() {
            // Die Mindestgröße gilt auch für einen gespeicherten Stand: ein von
            // Hand verkleinertes Fenster soll die Oberfläche nicht zerlegen.
            viewport = viewport.with_inner_size([
                f.breite.max(1080.0 + schatten),
                f.hoehe.max(680.0 + schatten),
            ]);
            // Die Lage setzt nur ein Fenstersystem, das globale Fensterkoordinaten
            // annimmt (X11, Windows, macOS). Unter Wayland meldet die Sitzung
            // keine Lage — dort kennt xdg-shell keine Fensterposition, `winit`
            // verwirft `with_position` still. Die Größe kommt trotzdem zurück.
            if let Some((x, y)) = f.lage() {
                viewport = viewport.with_position([x, y]);
            }
            // Vollbild und maximiert sind zwei Zustände: das Vollbild ist
            // rahmenlos und bildschirmfüllend, maximiert behält den Rahmen.
            if f.vollbild {
                viewport = viewport.with_fullscreen(true);
            } else if f.maximiert {
                viewport = viewport.with_maximized(true);
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        vsync: crate::diag::vsync(),
        ..Default::default()
    };

    eframe::run_native(
        "JoJa-CAD",
        options,
        Box::new(move |cc| {
            let mut app = app::App::new(cc);
            // Sperrdatei lebt so lange wie die Anwendung.
            app.sperre = Some(sperre);
            // T0141: die gespeicherte Fenstergröße gilt als „normaler“ Stand —
            // ein Start im Vollbild behält sie beim Beenden.
            app.setze_fensterlage(sitzung.as_ref().and_then(|s| s.fenster));
            // T0135: Kanal für eine zweite Instanz.
            app.instanzdienst_starten(&cc.egui_ctx);
            match file.clone() {
                // Eine ausdrücklich übergebene Zeichnung hat Vorrang — dann
                // wird nicht zusätzlich die Sitzung geöffnet.
                Some(f) => app.load_document(&std::path::PathBuf::from(f)),
                None => {
                    if let Some(s) = &sitzung {
                        let pool_dir = app.prefs.sicherungen_dir();
                        let meldungen = app.sitzung_wiederherstellen(s, &pool_dir);
                        for m in meldungen {
                            app.toast(m);
                        }
                    }
                }
            }
            // Startzustand für Screenshots/automatisierte Tests (JJCAD_DEMO_UI)
            app.apply_demo_ui();
            Ok(Box::new(app))
        }),
    )
}

/// Hinweisfenster, wenn eine andere Instanz die Wurzel hält: keine zweite
/// Schreibinstanz, aber der Weg zu einem anderen Datenordner.
struct SperrFenster {
    konflikt: daten::Sperrkonflikt,
}

impl eframe::App for SperrFenster {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading(jjcad_core::i18n::tr("gui.start.sperre_titel"));
            ui.add_space(6.0);
            let pid = if self.konflikt.pid == 0 {
                "?".to_string()
            } else {
                self.konflikt.pid.to_string()
            };
            ui.label(jjcad_core::i18n::t_args_owned(
                "gui.start.sperre_text",
                &[
                    // Der Nutzer soll den Ordner sehen, nicht die Sperrdatei.
                    ("pfad", jjcad_core::daten::pfad_text(&jjcad_core::daten::wurzel())),
                    ("pid", pid),
                ],
            ))
            .on_hover_text(self.konflikt.pfad.display().to_string());
            ui.add_space(4.0);
            ui.label(jjcad_core::i18n::tr("gui.start.sperre_hinweis"));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(jjcad_core::i18n::tr("gui.start.andere_wurzel"))
                    .clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::process::Command::new(exe)
                                .arg("--daten-dir")
                                .arg(dir)
                                .spawn();
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                if ui
                    .button(jjcad_core::i18n::tr("gui.start.beenden"))
                    .clicked()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.add_space(8.0);
            ui.label(jjcad_core::i18n::tr("gui.start.kommandozeile"));
        });
    }
}

fn starte_hinweis(konflikt: daten::Sperrkonflikt) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 260.0])
            .with_resizable(false)
            .with_title("JoJa-CAD — Datenordner belegt")
            .with_app_id("jjcad")
            .with_icon(icon::fenster_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "JoJa-CAD — Datenordner belegt",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(SperrFenster { konflikt }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::argumente;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    /// T0127: `--daten-dir` wird abgetrennt, die Zeichnung bleibt das erste
    /// freie Argument; beide Schreibweisen sind erlaubt.
    #[test]
    fn daten_dir_wird_abgetrennt() {
        let (d, f) = argumente(&s(&["--daten-dir", "/tmp/wurzel", "plan.jjcad"]));
        assert_eq!(d.as_deref(), Some(std::path::Path::new("/tmp/wurzel")));
        assert_eq!(f.as_deref(), Some("plan.jjcad"));

        let (d, f) = argumente(&s(&["plan.jjcad", "--daten-dir=/tmp/wurzel"]));
        assert_eq!(d.as_deref(), Some(std::path::Path::new("/tmp/wurzel")));
        assert_eq!(f.as_deref(), Some("plan.jjcad"));

        let (d, f) = argumente(&s(&["plan.jjcad"]));
        assert!(d.is_none());
        assert_eq!(f.as_deref(), Some("plan.jjcad"));

        let (d, f) = argumente(&s(&[]));
        assert!(d.is_none());
        assert!(f.is_none());
    }
}
