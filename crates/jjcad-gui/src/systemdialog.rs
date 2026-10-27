//! Systemweiter Dateidialog des Betriebssystems.
//!
//! Statt eines eigenen Dateidialogs wird der Dialog der Arbeitsumgebung benutzt:
//! unter Linux läuft er über das XDG-Desktop-Portal (KDE → Dolphin, GNOME →
//! Dateien, XFCE → GTK-Dialog), unter Windows über den Win32-Dateidialog und
//! unter macOS über NSOpenPanel. Das erledigt die Kiste `rfd`, damit wir keine
//! Oberfläche je System selbst nachbauen.
//!
//! Der Dialog läuft in einem **eigenen Thread**: das Systemfenster gehört dem
//! Portal, die Zeichenfläche von JoJa-CAD bleibt bedienbar, und die Oberfläche
//! muss nicht auf einen fremden Ereignis-Kreislauf warten. Das Ergebnis kommt
//! über einen Kanal zurück und wird im nächsten Frame abgeholt
//! (`Stand::Laeuft`/`Gewaehlt`/`Abgebrochen`).
//!
//! Ist kein Systemdialog verfügbar (kein D-Bus, kein Portal, reine
//! Testumgebung), meldet `verfuegbar()` false; der Aufrufer nimmt dann den
//! eigenen Dateidialog (`dialogs::FileDialog`). Dieselbe Umschaltung gibt es
//! als Einstellung („Dateidialog“: System / Eingebaut).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// Was der Systemdialog anzeigen soll — unabhängig von der Dialogkiste, damit
/// sich der Aufruf ohne Fenster prüfen lässt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anfrage {
    /// Fenstertitel des Dialogs (`Zeichnung öffnen`).
    pub titel: String,
    /// Anzeigename des Dateifilters ohne Endung (`Zeichnung`).
    pub filter: String,
    /// Endung ohne Punkt (`jjcad`, `dxf`, `pdf`, `png`).
    pub endung: String,
    pub verzeichnis: Option<PathBuf>,
    /// Vorgeschlagener Dateiname beim Speichern (`Grundriss.jjcad`).
    pub dateiname: Option<String>,
    pub speichern: bool,
}

impl Anfrage {
    pub fn neu(
        titel: impl Into<String>,
        filter: impl Into<String>,
        endung: impl Into<String>,
        verzeichnis: Option<PathBuf>,
        dateiname: Option<String>,
        speichern: bool,
    ) -> Self {
        Self {
            titel: titel.into(),
            filter: filter.into(),
            endung: endung.into(),
            verzeichnis,
            dateiname,
            speichern,
        }
    }
}

/// Zustand einer laufenden Abfrage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stand {
    /// Der Dialog steht noch offen.
    Laeuft,
    /// Der Nutzer hat eine Datei gewählt.
    Gewaehlt(PathBuf),
    /// Der Nutzer hat abgebrochen (oder das System konnte keinen Dialog
    /// zeigen).
    Abgebrochen,
}

/// Rückfrage beim System; im Test durch eine eigene Funktion ersetzbar.
pub type Abfrage = fn(&Anfrage) -> Option<PathBuf>;

/// Laufende Abfrage beim Systemdialog.
pub struct Systemdialog {
    rx: Receiver<Option<PathBuf>>,
    ergebnis: Option<Stand>,
}

impl Systemdialog {
    /// Öffnet den Systemdialog im Hintergrund-Thread.
    pub fn starten(anfrage: Anfrage) -> Self {
        Self::mit(system_abfrage, anfrage)
    }

    /// Wie `starten`, aber mit austauschbarem Rückweg (Tests).
    pub fn mit(abfrage: Abfrage, anfrage: Anfrage) -> Self {
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new()
            .name("jjcad-systemdialog".into())
            .spawn(move || {
                let _ = tx.send(abfrage(&anfrage));
            });
        Self { rx, ergebnis: None }
    }

    /// Fragt den Stand ab; das Ergebnis bleibt nach dem ersten Abholen stehen,
    /// damit ein Aufruf pro Frame nicht auf ein leeres Ergebnis läuft.
    pub fn abfragen(&mut self) -> Stand {
        if let Some(s) = &self.ergebnis {
            return s.clone();
        }
        let stand = match self.rx.try_recv() {
            Ok(Some(p)) => Stand::Gewaehlt(p),
            Ok(None) | Err(TryRecvError::Disconnected) => Stand::Abgebrochen,
            Err(TryRecvError::Empty) => return Stand::Laeuft,
        };
        self.ergebnis = Some(stand.clone());
        stand
    }
}

/// Ist der Systemdialog auf diesem Rechner benutzbar?
///
/// Unter Linux braucht das XDG-Portal eine Sitzungs-D-Bus-Adresse und eine
/// Anzeige; Windows und macOS bringen ihren Dialog mit.
pub fn verfuegbar() -> bool {
    verfuegbar_auf(
        std::env::consts::OS,
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
        std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some(),
    )
}

/// Rechenregel von `verfuegbar`, ohne Umgebungsvariablen (prüfbar).
pub fn verfuegbar_auf(os: &str, dbus: bool, anzeige: bool) -> bool {
    match os {
        "windows" | "macos" => true,
        // Linux und BSDs: XDG-Desktop-Portal über den Sitzungsbus.
        _ => dbus && anzeige,
    }
}

/// Endung an einen gewählten Namen hängen — nur beim Speichern und nur, wenn
/// sie fehlt. Bei „Öffnen“ bleibt der Name, wie das System ihn liefert.
pub fn endung_ergaenzen(pfad: PathBuf, endung: &str, speichern: bool) -> PathBuf {
    if !speichern || endung.is_empty() {
        return pfad;
    }
    let hat_endung = pfad
        .extension()
        .map(|e| e.to_string_lossy().eq_ignore_ascii_case(endung))
        .unwrap_or(false);
    if hat_endung {
        return pfad;
    }
    let mut name = pfad
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    name.push('.');
    name.push_str(endung);
    match pfad.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.join(name),
        _ => PathBuf::from(name),
    }
}

/// Der echte Dialog über `rfd`.
fn system_abfrage(a: &Anfrage) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(&a.titel);
    if !a.endung.is_empty() {
        let name = if a.filter.is_empty() {
            format!("*.{}", a.endung)
        } else {
            format!("{} (*.{})", a.filter, a.endung)
        };
        d = d.add_filter(name, &[a.endung.as_str()]);
    }
    if let Some(dir) = &a.verzeichnis {
        if dir.is_dir() {
            d = d.set_directory(dir);
        }
    }
    if let Some(name) = &a.dateiname {
        d = d.set_file_name(name);
    }
    let gewaehlt = if a.speichern { d.save_file() } else { d.pick_file() };
    gewaehlt.map(|p| endung_ergaenzen(p, &a.endung, a.speichern))
}

/// Hilfsfunktion für Tests: Verzeichnis eines Pfades als Startordner.
#[allow(dead_code)]
pub fn startordner(p: &Path) -> Option<PathBuf> {
    p.parent().map(|x| x.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn anfrage(speichern: bool) -> Anfrage {
        Anfrage::neu(
            "Zeichnung speichern",
            "Zeichnung",
            "jjcad",
            Some(PathBuf::from("/tmp")),
            Some("Grundriss.jjcad".into()),
            speichern,
        )
    }

    /// Wartet bis zu zwei Sekunden auf das Ergebnis der Abfrage.
    fn warte(d: &mut Systemdialog) -> Stand {
        let ende = Instant::now() + Duration::from_secs(2);
        loop {
            match d.abfragen() {
                Stand::Laeuft if Instant::now() < ende => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                s => return s,
            }
        }
    }

    fn liefert_datei(_: &Anfrage) -> Option<PathBuf> {
        Some(PathBuf::from("/tmp/grundriss.jjcad"))
    }

    fn liefert_nichts(_: &Anfrage) -> Option<PathBuf> {
        None
    }

    #[test]
    fn gewaehlte_datei_kommt_aus_dem_thread() {
        let mut d = Systemdialog::mit(liefert_datei, anfrage(false));
        assert_eq!(
            warte(&mut d),
            Stand::Gewaehlt(PathBuf::from("/tmp/grundriss.jjcad"))
        );
        // Ergebnis bleibt stehen (ein Aufruf je Frame)
        assert_eq!(
            d.abfragen(),
            Stand::Gewaehlt(PathBuf::from("/tmp/grundriss.jjcad"))
        );
    }

    #[test]
    fn keine_datei_meldet_abbruch() {
        let mut d = Systemdialog::mit(liefert_nichts, anfrage(false));
        assert_eq!(warte(&mut d), Stand::Abgebrochen);
    }

    #[test]
    fn anfrage_traegt_titel_filter_und_startordner() {
        let a = anfrage(true);
        assert_eq!(a.titel, "Zeichnung speichern");
        assert_eq!(a.filter, "Zeichnung");
        assert_eq!(a.endung, "jjcad");
        assert_eq!(a.verzeichnis, Some(PathBuf::from("/tmp")));
        assert_eq!(a.dateiname.as_deref(), Some("Grundriss.jjcad"));
        assert!(a.speichern);
    }

    #[test]
    fn endung_wird_nur_beim_speichern_ergaenzt() {
        // Speichern ohne Endung → Endung dran
        assert_eq!(
            endung_ergaenzen(PathBuf::from("/tmp/Haus"), "jjcad", true),
            PathBuf::from("/tmp/Haus.jjcad")
        );
        // Endung schon da (auch groß geschrieben) → unverändert
        assert_eq!(
            endung_ergaenzen(PathBuf::from("/tmp/Haus.JJCAD"), "jjcad", true),
            PathBuf::from("/tmp/Haus.JJCAD")
        );
        // Öffnen → nie ergänzen
        assert_eq!(
            endung_ergaenzen(PathBuf::from("/tmp/Haus"), "jjcad", false),
            PathBuf::from("/tmp/Haus")
        );
        // Andere Endung als Filter → die gewählte bleibt
        assert_eq!(
            endung_ergaenzen(PathBuf::from("/tmp/Haus.txt"), "jjcad", true),
            PathBuf::from("/tmp/Haus.txt.jjcad")
        );
    }

    #[test]
    fn verfuegbarkeit_haengt_am_sitzungsbus() {
        // Linux ohne D-Bus (z. B. Xvfb): eingebauter Dialog
        assert!(!verfuegbar_auf("linux", false, true));
        // Linux ohne Anzeige: kein Dialog möglich
        assert!(!verfuegbar_auf("linux", true, false));
        // KDE/GNOME-Sitzung: XDG-Portal
        assert!(verfuegbar_auf("linux", true, true));
        // Windows und macOS bringen den Dialog selbst mit
        assert!(verfuegbar_auf("windows", false, false));
        assert!(verfuegbar_auf("macos", false, false));
    }
}
