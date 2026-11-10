//! Sitzung: was beim Beenden gemerkt und beim Start wiederhergestellt wird
//! (T0130).
//!
//! Gespeichert wird `<Wurzel>/sitzung.json`: die Reiter in ihrer Reihenfolge
//! (Kennung, Zieldatei oder keine, Name), je Reiter Ansicht und aktives Blatt,
//! dazu der aktive Reiter und die Lage des Fensters.
//!
//! **Nicht** Teil der Sitzung sind Auswahl, Undo-Verlauf, Werkzeugwahl und
//! Zwischenablage — sie gehören zur Arbeit, nicht zur Zeichnung.
//!
//! Die Kennung eines Reiters ist die UUID des Dokuments (`Document::id`). Über
//! sie findet die Sicherung ([`crate::sicherung`]) den neuesten Stand einer
//! ungespeicherten Zeichnung, ohne dass die Sitzung selbst einen Stand trägt.
//! Fehlt `sitzung.json`, startet das Programm mit einer leeren Zeichnung.

use crate::geom::Vec2;
use crate::model::Id;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Kennung des Sitzungsformats (Formatprüfung beim Laden).
pub const SITZUNG_FORMAT: &str = "jjcad-sitzung";
/// Fassung der Sitzung.
pub const SITZUNG_VERSION: u32 = 1;

/// Ansicht eines Reiters — dieselben drei Werte wie `View` in der Oberfläche.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Ansicht {
    /// Weltkoordinate in der Bildmitte (mm).
    #[serde(default = "default_mitte")]
    pub mitte: Vec2,
    /// Pixel je Millimeter.
    #[serde(default = "default_zoom")]
    pub zoom: f64,
    /// Drehung der Ansicht im Bogenmaß, mathematisch positiv.
    #[serde(default)]
    pub drehung: f64,
}

fn default_mitte() -> Vec2 {
    Vec2::new(5000.0, 4000.0)
}

fn default_zoom() -> f64 {
    0.08
}

impl Default for Ansicht {
    fn default() -> Self {
        Self {
            mitte: default_mitte(),
            zoom: default_zoom(),
            drehung: 0.0,
        }
    }
}

impl Ansicht {
    /// Taugt die Ansicht zum Wiederherstellen? (Zoom 0 oder unsinnig große
    /// Werte würden die Zeichnung unsichtbar machen.)
    pub fn brauchbar(&self) -> bool {
        self.mitte.x.is_finite()
            && self.mitte.y.is_finite()
            && self.zoom.is_finite()
            && self.zoom > 1e-6
            && self.zoom < 1e4
            && self.drehung.is_finite()
    }
}

/// Ein Reiter in der Reihenfolge des Reiterstreifens.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Reiter {
    /// Kennung des Dokuments (UUID) — der Schlüssel in den Sicherungspool.
    #[serde(default)]
    pub id: String,
    /// Zieldatei; `None` = ungespeicherte Zeichnung.
    #[serde(default)]
    pub datei: Option<String>,
    /// Anzeigename des Reiters.
    #[serde(default)]
    pub name: String,
    /// Ansicht dieses Reiters (Mitte, Zoom, Drehung).
    #[serde(default)]
    pub ansicht: Ansicht,
    /// Aktives Blatt; `None` = Modell.
    #[serde(default)]
    pub blatt: Option<Id>,
}

/// Lage und Größe des Fensters (in Punkten, wie egui sie meldet).
///
/// Die **Größe** meldet jede Plattform (`RawInput::screen_rect`, aus
/// `window.inner_size()`). Die **Lage** (`x`/`y`) meldet nur ein Fenstersystem
/// mit globalen Fensterkoordinaten; unter Wayland fehlt sie (`mit_lage: false`),
/// weil xdg-shell weder Abfrage noch Vorgabe einer Fensterposition kennt
/// (winit liefert dort `NotSupported`, `with_position` wird still verworfen).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Fensterlage {
    pub breite: f32,
    pub hoehe: f32,
    /// Linke Fensterkante in Punkten; nur gültig, wenn `mit_lage`.
    pub x: f32,
    /// Obere Fensterkante in Punkten; nur gültig, wenn `mit_lage`.
    pub y: f32,
    /// Die Plattform hat eine Lage gemeldet (X11, Windows, macOS).
    ///
    /// Bewusst ein Merker statt `Option<f32>`: eine `null` im Feld `x` bricht
    /// das Laden in **älteren** Fassungen (dort steht `f32`), und die verwerfen
    /// dann die ganze Sitzung samt Reitern. Ältere Dateien trugen immer eine
    /// echte Lage — daher ist `true` die Vorgabe beim Lesen.
    #[serde(default = "lage_gemeldet")]
    pub mit_lage: bool,
    /// Das Fenster war beim Beenden maximiert.
    #[serde(default)]
    pub maximiert: bool,
    /// Das Fenster war beim Beenden im Vollbild (T0141, Turn 2).
    ///
    /// Getrennt von `maximiert`: ein Vollbild ist rahmenlos und bildschirmfüllend,
    /// ein maximiertes Fenster behält Rahmen und Titelleiste. Beides über einen
    /// Merker wiederherzustellen gäbe den falschen Zustand.
    #[serde(default)]
    pub vollbild: bool,
}

/// Vorgabe für [`Fensterlage::mit_lage`] bei Dateien ohne dieses Feld (T0141).
fn lage_gemeldet() -> bool {
    true
}

impl Fensterlage {
    /// Brauchbar für `ViewportBuilder`: eine endliche Größe, die nicht unter
    /// der Mindestgröße des Fensters liegt. Eine fehlende Lage ist kein Grund,
    /// die ganze Lage zu verwerfen — dann kommt wenigstens die Größe zurück.
    pub fn brauchbar(&self) -> bool {
        self.breite.is_finite()
            && self.hoehe.is_finite()
            && self.breite >= 1.0
            && self.hoehe >= 1.0
    }

    /// Fensterposition (linke obere Ecke in Punkten), sofern gemeldet und
    /// endlich. Beide Kanten gehören zusammen: eine halbe Lage ist keine Lage.
    pub fn lage(&self) -> Option<(f32, f32)> {
        (self.mit_lage && self.x.is_finite() && self.y.is_finite())
            .then_some((self.x, self.y))
    }
}

/// Inhalt von `sitzung.json`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sitzung {
    pub format: String,
    pub version: u32,
    /// Fensterlage; `None`, wenn sie nicht ermittelbar war (headless).
    #[serde(default)]
    pub fenster: Option<Fensterlage>,
    /// Index des aktiven Reiters.
    #[serde(default)]
    pub aktiv: usize,
    /// Die Reiter in der Reihenfolge des Reiterstreifens.
    #[serde(default)]
    pub reiter: Vec<Reiter>,
}

impl Default for Sitzung {
    fn default() -> Self {
        Self {
            format: SITZUNG_FORMAT.to_string(),
            version: SITZUNG_VERSION,
            fenster: None,
            aktiv: 0,
            reiter: Vec::new(),
        }
    }
}

/// Liest `sitzung.json`.
///
/// Fehlt die Datei, ist das **kein Fehler**: `Ok(None)` heißt „leerer Start“
/// (so verhält es sich auch, wenn der Nutzer die Datei von Hand löscht).
/// Eine unlesbare oder fremde Datei liefert `Err` — der Aufrufer startet dann
/// leer und meldet es, statt eine halb verstandene Sitzung anzuwenden.
pub fn laden(pfad: &Path) -> Result<Option<Sitzung>, String> {
    let text = match std::fs::read_to_string(pfad) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Sitzung nicht lesbar: {e}")),
    };
    let s: Sitzung = serde_json::from_str(&text)
        .map_err(|e| format!("Sitzung ist keine gültige jjcad-Sitzung: {e}"))?;
    if s.format != SITZUNG_FORMAT {
        return Err(format!(
            "Unbekanntes Sitzungsformat '{}' (erwartet '{}')",
            s.format, SITZUNG_FORMAT
        ));
    }
    if s.version > SITZUNG_VERSION {
        return Err(format!(
            "Die Sitzung wurde mit einer neueren Version geschrieben ({} > {})",
            s.version, SITZUNG_VERSION
        ));
    }
    Ok(Some(s))
}

/// Schreibt `sitzung.json` atomar (temp → rename), damit ein Absturz mitten im
/// Schreiben keine halbe Sitzung hinterlässt.
pub fn speichern(pfad: &Path, sitzung: &Sitzung) -> Result<(), String> {
    if let Some(dir) = pfad.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
        }
    }
    let text = serde_json::to_string_pretty(sitzung)
        .map_err(|e| format!("Sitzung nicht schreibbar: {e}"))?;
    crate::sicherung::atomar_schreiben(pfad, text.as_bytes())
}

/// Entfernt `sitzung.json`. Eine fehlende Datei ist kein Fehler.
pub fn entfernen(pfad: &Path) -> Result<(), String> {
    match std::fs::remove_file(pfad) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Sitzung nicht entfernbar: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-sitzung-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sitzung_mit_drei() -> Sitzung {
        Sitzung {
            fenster: Some(Fensterlage {
                breite: 1600.0,
                hoehe: 1000.0,
                x: 120.0,
                y: 60.0,
                mit_lage: true,
                maximiert: false,
                vollbild: false,
            }),
            aktiv: 1,
            reiter: vec![
                Reiter {
                    id: "aaaaaaaa-0000-4000-8000-000000000001".into(),
                    datei: Some("/pläne/Haus.jjcad".into()),
                    name: "Haus".into(),
                    ansicht: Ansicht {
                        mitte: Vec2::new(1200.0, 800.0),
                        zoom: 0.25,
                        drehung: 0.0,
                    },
                    blatt: None,
                },
                Reiter {
                    id: "aaaaaaaa-0000-4000-8000-000000000002".into(),
                    datei: None,
                    name: "Unbenannt".into(),
                    ansicht: Ansicht {
                        mitte: Vec2::new(4000.0, 1500.0),
                        zoom: 0.08,
                        drehung: 0.5236,
                    },
                    blatt: Some(7),
                },
                Reiter {
                    id: "aaaaaaaa-0000-4000-8000-000000000003".into(),
                    datei: Some("/pläne/Schnitt.jjcad".into()),
                    name: "Schnitt".into(),
                    ansicht: Ansicht::default(),
                    blatt: None,
                },
            ],
            ..Sitzung::default()
        }
    }

    /// Rundlauf: Reihenfolge, Ansichten, Blatt, aktiver Reiter und Fensterlage
    /// kommen unverändert zurück.
    #[test]
    fn rundlauf_erhaelt_alles() {
        let dir = test_dir("rundlauf");
        let pfad = dir.join("sitzung.json");
        let s = sitzung_mit_drei();
        speichern(&pfad, &s).unwrap();
        let wieder = laden(&pfad).unwrap().expect("Sitzung muss gelesen werden");
        assert_eq!(wieder, s);
        assert_eq!(wieder.reiter.len(), 3);
        assert_eq!(wieder.aktiv, 1);
        assert_eq!(wieder.reiter[1].blatt, Some(7));
        assert_eq!(wieder.fenster.unwrap().breite, 1600.0);
    }

    /// Eine fehlende Datei ist der leere Start, kein Fehler.
    #[test]
    fn fehlende_datei_ist_der_leere_start() {
        let dir = test_dir("fehlt");
        assert!(laden(&dir.join("sitzung.json")).unwrap().is_none());
        // Auch das Entfernen einer fehlenden Datei ist in Ordnung.
        assert!(entfernen(&dir.join("sitzung.json")).is_ok());
    }

    /// Von Hand gelöscht → beim nächsten Laden wieder der leere Start.
    #[test]
    fn geloeschte_datei_startet_leer() {
        let dir = test_dir("geloescht");
        let pfad = dir.join("sitzung.json");
        speichern(&pfad, &sitzung_mit_drei()).unwrap();
        assert!(laden(&pfad).unwrap().is_some());
        entfernen(&pfad).unwrap();
        assert!(!pfad.exists(), "die Datei muss weg sein");
        assert!(laden(&pfad).unwrap().is_none());
    }

    /// Kaputte oder fremde Datei → Fehler, nicht eine halb verstandene Sitzung.
    #[test]
    fn kaputte_datei_meldet_sich() {
        let dir = test_dir("kaputt");
        let pfad = dir.join("sitzung.json");
        std::fs::write(&pfad, b"{ das ist kein json").unwrap();
        assert!(laden(&pfad).is_err());

        std::fs::write(&pfad, br#"{"format":"etwas-anderes","version":1}"#).unwrap();
        assert!(laden(&pfad).is_err());

        std::fs::write(&pfad, br#"{"format":"jjcad-sitzung","version":99}"#).unwrap();
        assert!(laden(&pfad).is_err());
    }

    /// Ein Schreibvorgang lässt keine temp-Datei zurück.
    #[test]
    fn schreiben_hinterlaesst_keine_reste() {
        let dir = test_dir("reste");
        let pfad = dir.join("sitzung.json");
        speichern(&pfad, &sitzung_mit_drei()).unwrap();
        let reste: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".tmp-"))
            .collect();
        assert!(reste.is_empty(), "temp-Reste: {reste:?}");
    }

    /// Unsinnige Ansichten und Fensterlagen werden als unbrauchbar erkannt.
    #[test]
    fn unsinnige_werte_sind_unbrauchbar() {
        assert!(Ansicht::default().brauchbar());
        assert!(!Ansicht {
            zoom: 0.0,
            ..Ansicht::default()
        }
        .brauchbar());
        assert!(!Ansicht {
            mitte: Vec2::new(f64::NAN, 0.0),
            ..Ansicht::default()
        }
        .brauchbar());
        assert!(Fensterlage {
            breite: 1600.0,
            hoehe: 1000.0,
            x: 0.0,
            y: 0.0,
            mit_lage: true,
            maximiert: false,
            vollbild: false,
        }
        .brauchbar());
        assert!(!Fensterlage {
            breite: 0.0,
            hoehe: 1000.0,
            x: 0.0,
            y: 0.0,
            mit_lage: true,
            maximiert: false,
            vollbild: false,
        }
        .brauchbar());
    }

    /// T0141: Ohne gemeldete Lage (Wayland) ist die Größe trotzdem brauchbar —
    /// sonst würde beim Beenden gar nichts gemerkt.
    #[test]
    fn fensterlage_ohne_position_ist_brauchbar() {
        let ohne = Fensterlage {
            breite: 1500.0,
            hoehe: 950.0,
            x: 0.0,
            y: 0.0,
            mit_lage: false,
            maximiert: false,
            vollbild: false,
        };
        assert!(ohne.brauchbar(), "die Größe allein ist brauchbar");
        assert_eq!(ohne.lage(), None, "ohne gemeldete Lage keine Position");

        let mit = Fensterlage {
            breite: 1500.0,
            hoehe: 950.0,
            x: 120.0,
            y: 60.0,
            mit_lage: true,
            maximiert: true,
            vollbild: false,
        };
        assert_eq!(mit.lage(), Some((120.0, 60.0)));
        assert!(mit.maximiert);
    }

    /// Ohne Merker zählt keine Position — auch keine unsinnige Zahl.
    #[test]
    fn lage_ohne_merker_oder_mit_nan_zaehlt_nicht() {
        let ohne_merker = Fensterlage {
            breite: 1500.0,
            hoehe: 950.0,
            x: 10.0,
            y: 20.0,
            mit_lage: false,
            maximiert: false,
            vollbild: false,
        };
        assert_eq!(ohne_merker.lage(), None);
        let nan = Fensterlage {
            breite: 1500.0,
            hoehe: 950.0,
            x: f32::NAN,
            y: 60.0,
            mit_lage: true,
            maximiert: false,
            vollbild: false,
        };
        assert_eq!(nan.lage(), None);
    }

    /// T0141: Die Datei trägt `x`/`y` **als Zahlen**, auch wenn keine Lage
    /// gemeldet wurde — ältere Fassungen lesen dort `f32` und würden an einer
    /// `null` die ganze Sitzung (samt Reitern) verwerfen.
    #[test]
    fn geschriebene_datei_traegt_zahlen_statt_null() {
        let dir = test_dir("zahlen");
        let pfad = dir.join("sitzung.json");
        let s = Sitzung {
            fenster: Some(Fensterlage {
                breite: 1500.0,
                hoehe: 950.0,
                x: 0.0,
                y: 0.0,
                mit_lage: false,
                maximiert: false,
                vollbild: false,
            }),
            ..Sitzung::default()
        };
        speichern(&pfad, &s).unwrap();
        let wert: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&pfad).unwrap()).unwrap();
        let f = &wert["fenster"];
        assert!(f["x"].is_number(), "x muss eine Zahl sein: {f}");
        assert!(f["y"].is_number(), "y muss eine Zahl sein: {f}");
        assert_eq!(f["mit_lage"], serde_json::json!(false));
        // Und der neue Leser versteht seine eigene Datei.
        let wieder = laden(&pfad).unwrap().unwrap();
        assert_eq!(wieder.fenster.unwrap().lage(), None);
    }

    /// Alte `sitzung.json` (T0130) trug `x`/`y` als Zahl und kannte
    /// `maximiert` nicht — sie muss weiterhin laden.
    #[test]
    fn alte_sitzung_ohne_maximiert_laedt() {
        let dir = test_dir("alt");
        let pfad = dir.join("sitzung.json");
        std::fs::write(
            &pfad,
            br#"{"format":"jjcad-sitzung","version":1,
                 "fenster":{"breite":1600.0,"hoehe":1000.0,"x":120.0,"y":60.0},
                 "aktiv":0,"reiter":[]}"#,
        )
        .unwrap();
        let s = laden(&pfad).unwrap().expect("alte Sitzung muss laden");
        let f = s.fenster.expect("Fensterlage");
        assert_eq!(f.lage(), Some((120.0, 60.0)));
        assert!(!f.maximiert, "unbekanntes Feld heißt nicht maximiert");
        assert!(!f.vollbild, "unbekanntes Feld heißt nicht Vollbild");
    }

    /// T0141, Turn 2: Vollbild und maximiert sind zwei Zustände und bleiben es
    /// auch in der Datei — sonst käme ein Vollbild als maximiertes Fenster
    /// (mit Rahmen) zurück.
    #[test]
    fn vollbild_und_maximiert_bleiben_getrennt() {
        let dir = test_dir("vollbild");
        let pfad = dir.join("sitzung.json");
        let s = Sitzung {
            fenster: Some(Fensterlage {
                breite: 1200.0,
                hoehe: 800.0,
                x: 40.0,
                y: 30.0,
                mit_lage: true,
                maximiert: false,
                vollbild: true,
            }),
            ..Sitzung::default()
        };
        speichern(&pfad, &s).unwrap();
        let wieder = laden(&pfad).unwrap().unwrap().fenster.unwrap();
        assert!(wieder.vollbild);
        assert!(!wieder.maximiert);
        assert_eq!((wieder.breite, wieder.hoehe), (1200.0, 800.0));
    }
}
