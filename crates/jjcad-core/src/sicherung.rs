//! Sicherung: Stände unabhängig vom Speichern (T0129).
//!
//! Die Sicherung legt während der Arbeit Stände im **Pool** ab und dünnt sie
//! nach Alter aus. Sie schreibt **nie** in die Zieldatei — gespeichert wird
//! ausschließlich vom Benutzer. Vorgabe und Begründung: `docs/benutzer/SICHERUNG.md`.
//!
//! Aufbau des Pools (ein Pool für alle Dateien, ein Projektordner):
//!
//! ```text
//! <Sicherungen>/manifest.json            Zähler n und die Liste der Stände
//! <Sicherungen>/stand-<n>-<uuid>.jjcad   eine Datei je Stand
//! ```
//!
//! Der **Pin** (neuester Stand je Dokument) ist abgeleitet, nicht gespeichert:
//! so kann er nicht veralten. Ebenso abgeleitet ist der **Anker** (ältester
//! Stand je Dokument). Beide werden beim Ausdünnen nie angefasst; der Pin
//! verfällt, sobald er `T` Zyklen alt ist (Anhang A.2, Schritt 2), der Anker
//! nur, indem ein jüngerer Stand nachrückt.
//!
//! Aufbewahrt wird nach dem Verfahren „Geometric Retention“ (SRR, Anhang A):
//! fester Platzvorrat, konstante relative Auflösung, Alter in Zyklen statt in
//! Kalenderzeit. Vorgabe und Begründung: `docs/benutzer/SICHERUNG.md`.

use crate::model::Document;
use crate::persist;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Name des Manifests im Pool.
pub const MANIFEST_DATEI: &str = "manifest.json";
/// Kennung des Manifests (Formatprüfung beim Laden).
pub const MANIFEST_FORMAT: &str = "jjcad-sicherung";
/// Fassung des Manifests. 2 = Geometric Retention (SRR), Stand kann
/// „verworfen“ sein; Fassung 1 kannte nur Fenster, Obergrenze und Dichtezone.
pub const MANIFEST_VERSION: u32 = 2;
/// Grenzen der Plätze im Pool (Einstellung „Plätze im Pool“).
pub const PLAETZE_MIN: u32 = 30;
pub const PLAETZE_MAX: u32 = 2000;
pub const PLAETZE_VORGABE: u32 = 150;
/// Vorgabe für „Reicht zurück“ in Arbeitsstunden (eine Woche).
pub const STUNDEN_VORGABE: u32 = 40;
/// Die vier Wahlen für „Reicht zurück“: Tag, Woche, Monat, Quartal.
pub const STUNDEN_WAHL: [u32; 4] = [8, 40, 160, 480];
/// Grenzen des Intervalls in Minuten.
pub const INTERVALL_MIN: u32 = 1;
pub const INTERVALL_MAX: u32 = 240;
pub const INTERVALL_VORGABE: u32 = 1;

/// Ein gesicherter Stand einer Datei.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stand {
    /// Globaler Zähler; einziger Zeitanker für Entscheidungen.
    pub zyklus: u64,
    /// Kennung des Dokuments (UUID aus dem Dateikopf, nicht der Pfad).
    pub datei: String,
    /// Anzeigename des Dokuments zum Zeitpunkt der Sicherung.
    #[serde(default)]
    pub name: String,
    /// SHA-256 des gesicherten Inhalts (Hex).
    pub hash: String,
    /// Größe des Standes in Bytes.
    pub groesse: u64,
    /// Zeitstempel, nur zur Anzeige.
    #[serde(default)]
    pub zeit: String,
    /// Der Stand entspricht dem Inhalt der **Zieldatei** (geschrieben nach dem
    /// Speichern bzw. nach „Verwerfen“). Beim Wiederherstellen gewinnt dann die
    /// Datei; nur ein Stand mit `gesichert = false` trägt ungespeicherte Arbeit.
    #[serde(default)]
    pub gesichert: bool,
    /// R3.5: Der Benutzer hat diese Arbeit verworfen (Reiter ohne Speichern
    /// geschlossen). Der Stand bleibt bis zu seinem Verfall liegen, wird aber
    /// **nie** automatisch wiederhergestellt.
    #[serde(default)]
    pub verworfen: bool,
    /// SHA-256 der **Zieldatei**, wie sie beim Anlegen des Standes aussah
    /// (leer, wenn es keine Zieldatei gab). Damit erkennt R2.3, ob eine fremde
    /// Anwendung die Datei seither verändert hat — der Inhalt des Standes
    /// selbst taugt dafür nicht, weil er den Pfad im Kopf trägt.
    #[serde(default)]
    pub datei_hash: String,
    /// Dateiname im Pool (ohne Verzeichnis). Leer = aus `name` und `zeit`
    /// ableiten — so heißen Stände aus älteren Pools noch.
    #[serde(default)]
    pub datei_name: String,
    /// Zieldatei zum Zeitpunkt der Sicherung (leer = unbekannt, z. B. bei
    /// Altbestand oder einer nie gespeicherten Zeichnung). Damit findet das
    /// Aufräumen Stände, deren Zeichnung es nicht mehr gibt.
    #[serde(default)]
    pub pfad: String,
}

impl Stand {
    /// Dateiname des Standes im Pool:
    /// `<Zieldatei>_autosave_<Rückwärtsdatum>.jjcad`, z. B.
    /// `wohnhaus_autosave_20261108-153012.jjcad`.
    ///
    /// Das Datum steht Jahr–Monat–Tag–Stunde–Minute–Sekunde, also rückwärts
    /// gegenüber der deutschen Schreibweise; alphabetisch sortiert ist das
    /// Verzeichnis damit chronologisch.
    pub fn dateiname(&self) -> String {
        if !self.datei_name.is_empty() {
            return self.datei_name.clone();
        }
        format!(
            "{}_autosave_{}.jjcad",
            dateistamm(&self.name),
            rueckwaertsdatum(&self.zeit, self.zyklus)
        )
    }
}

/// Eine Zeichnung im Sicherungsordner mit ihren Ständen (T0223).
///
/// Ein Eintrag je Dokumentkennung: die Liste im Fenster „Wiederherstellen“
/// zeigt damit die Zeichnungen, nicht die einzelnen Standdateien — deren
/// Verlauf erscheint erst bei Auswahl als Diagramm.
#[derive(Clone, Debug, PartialEq)]
pub struct DateiEintrag {
    /// Kennung des Dokuments (UUID aus dem Dateikopf).
    pub datei: String,
    /// Anzeigename aus dem jüngsten Stand.
    pub name: String,
    /// Zieldatei aus dem jüngsten Stand (leer, wenn nie gespeichert).
    pub pfad: String,
    /// Zahl der Stände dieser Zeichnung.
    pub staende: usize,
    /// Zyklus des jüngsten Standes.
    pub neuester_zyklus: u64,
    /// Zyklus des ältesten Standes.
    pub aeltester_zyklus: u64,
    /// Zeitstempel des jüngsten Standes (nur Anzeige).
    pub neueste_zeit: String,
}

/// Stamm für den Standnamen: klein, ohne Umlaute, ohne Sonderzeichen.
fn dateistamm(name: &str) -> String {
    let s = crate::bloecke::slug(name);
    if s.is_empty() {
        "zeichnung".to_string()
    } else {
        s
    }
}

/// `2026-11-08T15:30:12` → `20261108-153012`.
///
/// Fehlt der Zeitstempel (Altbestand), steht die Zyklusnummer — sie ist
/// ebenfalls monoton und damit sortierbar.
fn rueckwaertsdatum(zeit: &str, zyklus: u64) -> String {
    let ziffern: String = zeit
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(14)
        .collect();
    if ziffern.len() == 14 {
        format!("{}-{}", &ziffern[..8], &ziffern[8..])
    } else {
        format!("zyklus-{zyklus:09}")
    }
}

/// Inhalt von `manifest.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    /// Globaler Zykluszähler `n`.
    #[serde(default)]
    pub zyklus: u64,
    #[serde(default)]
    pub staende: Vec<Stand>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            format: MANIFEST_FORMAT.to_string(),
            version: MANIFEST_VERSION,
            zyklus: 0,
            staende: Vec::new(),
        }
    }
}

/// Die Einstellungen der Sicherung (aus `prefs.json`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Einstellungen {
    /// „Verlauf sichern“: aus = nur der neueste Stand je Dokument.
    pub verlauf: bool,
    /// Plätze im Pool (Obergrenze, wird auf die Dateizahl angehoben).
    pub plaetze: u32,
    /// „Reicht zurück“ in Arbeitsstunden.
    pub stunden: u32,
    /// Intervall zwischen zwei Sicherungen in Minuten.
    pub intervall_min: u32,
}

impl Default for Einstellungen {
    fn default() -> Self {
        Self {
            verlauf: true,
            plaetze: PLAETZE_VORGABE,
            stunden: STUNDEN_VORGABE,
            intervall_min: INTERVALL_VORGABE,
        }
    }
}

impl Einstellungen {
    /// Fenster `T` in Zyklen: `Arbeitsstunden × 60 / Intervall`.
    pub fn zyklen(&self) -> u64 {
        let intervall = self.intervall_min.max(1) as u64;
        ((self.stunden as u64) * 60 / intervall).max(1)
    }

    /// Obergrenze des Pools: `max(N, 2 · Anzahl Dateien)`.
    ///
    /// Der Faktor 2 lässt jeder Datei Pin **und** Anker — erst dann ist die
    /// Kostenrechnung aus A.2 Schritt 4 überhaupt anwendbar, weil jeder
    /// Kandidat zwei Nachbarn in derselben Datei hat.
    pub fn plaetze_effektiv(&self, dateien: usize) -> usize {
        (self.plaetze as usize).max(2 * dateien)
    }

    /// Relative Lücke `g`, die eine Datei braucht, um mit `staende` Ständen
    /// den Zeitraum `T` abzudecken (Anhang A.3, Umkehrung von
    /// [`staende_fuer_abstand`]).
    ///
    /// Ohne Verlauf (weniger als drei Plätze je Datei) gibt es keine Lücke,
    /// die eine Verteilung beschreibt — dann 1,0 (alles eine Lücke).
    pub fn abstand(&self, dateien: usize) -> f64 {
        let dateien = dateien.max(1);
        let je_datei = self.plaetze_effektiv(dateien) as f64 / dateien as f64;
        abstand_fuer_staende(je_datei, self.zyklen() as f64)
    }
}

/// Stände, die eine Datei für den relativen Abstand `g` braucht (A.3):
///
/// ```text
/// k(g) = 1/g + ln(g·T) / ln(1/(1−g))
/// ```
///
/// Bis zum Alter `1/g` liegen alle Stände lückenlos, danach wächst der Abstand
/// geometrisch. `T` geht nur logarithmisch ein.
pub fn staende_fuer_abstand(g: f64, t: f64) -> f64 {
    if !(0.0 < g && g < 1.0) || t <= 0.0 {
        return f64::INFINITY;
    }
    let zaehler = (g * t).ln();
    let nenner = (1.0 / (1.0 - g)).ln();
    if zaehler <= 0.0 || nenner <= 0.0 {
        // g·T ≤ 1: der Zeitraum ist mit einem Stand erreicht.
        return 1.0 / g;
    }
    1.0 / g + zaehler / nenner
}

/// Umkehrung von [`staende_fuer_abstand`]: der Abstand `g`, den `staende`
/// Stände bei der Tiefe `t` ergeben.
///
/// `k(g)` fällt streng monoton in `g`; die Lösung wird durch Bisektion
/// gesucht (kein geschlossener Ausdruck). Liegt `staende` unter dem Bedarf von
/// `g = 1` (also unter einem Stand), ist die Lücke 1,0.
pub fn abstand_fuer_staende(staende: f64, t: f64) -> f64 {
    if staende <= 1.0 || t <= 0.0 {
        return 1.0;
    }
    let (mut lo, mut hi) = (1e-6_f64, 0.999_999_f64);
    // 80 Halbierungen sind weit unter jeder Anzeigegenauigkeit.
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if staende_fuer_abstand(mid, t) > staende {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Der Pool: Manifest und Verzeichnis.
pub struct Sicherung {
    dir: PathBuf,
    manifest: Manifest,
}

impl Sicherung {
    /// Öffnet den Pool im angegebenen Verzeichnis.
    ///
    /// Ein unlesbares Manifest wird als `manifest.json.kaputt` beiseitegelegt
    /// und mit einem leeren Pool begonnen — besser als stillschweigend alles
    /// zu verlieren. Waisen (Standdatei ohne Eintrag, Eintrag ohne Datei) und
    /// Reste eines abgebrochenen Schreibens werden entfernt.
    pub fn oeffnen(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let pfad = dir.join(MANIFEST_DATEI);
        let mut manifest = match std::fs::read_to_string(&pfad) {
            Ok(text) => match serde_json::from_str::<Manifest>(&text) {
                Ok(m) if m.format == MANIFEST_FORMAT => m,
                _ => {
                    let _ = std::fs::rename(&pfad, dir.join("manifest.json.kaputt"));
                    Manifest::default()
                }
            },
            Err(_) => Manifest::default(),
        };
        manifest.staende.sort_by_key(|s| s.zyklus);
        let mut pool = Self {
            dir: dir.to_path_buf(),
            manifest,
        };
        pool.altbestand_umbenennen();
        pool.waisen_entfernen();
        pool
    }

    /// Stände aus einem älteren Pool heißen noch `stand-<n>-<uuid>.jjcad`.
    /// Sie werden auf den neuen Namen umgezogen, damit das Verzeichnis lesbar
    /// ist und `stand_pfad` sie wiederfindet — sonst würde `waisen_entfernen`
    /// sie als „Eintrag ohne Datei“ verwerfen.
    fn altbestand_umbenennen(&mut self) {
        let dir = self.dir.clone();
        for s in &mut self.manifest.staende {
            if !s.datei_name.is_empty() {
                continue;
            }
            let neu = dir.join(s.dateiname());
            if neu.exists() {
                continue;
            }
            let alt = dir.join(format!("stand-{}-{}.jjcad", s.zyklus, s.datei));
            if alt.exists() {
                let _ = std::fs::rename(&alt, &neu);
            }
        }
    }

    /// Verzeichnis des Pools.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Das Manifest (nur lesend).
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Alle Stände, neueste zuerst.
    pub fn staende_neueste(&self) -> Vec<&Stand> {
        let mut v: Vec<&Stand> = self.manifest.staende.iter().collect();
        v.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        v
    }

    /// Stände eines Dokuments, neueste zuerst.
    pub fn staende_von(&self, datei: &str) -> Vec<&Stand> {
        let mut v: Vec<&Stand> = self
            .manifest
            .staende
            .iter()
            .filter(|s| s.datei == datei)
            .collect();
        v.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        v
    }

    /// Neuester Stand eines Dokuments.
    pub fn neuester(&self, datei: &str) -> Option<&Stand> {
        self.manifest
            .staende
            .iter()
            .filter(|s| s.datei == datei)
            .max_by_key(|s| s.zyklus)
    }

    /// Zahl der Dokumente im Pool.
    pub fn dateien(&self) -> usize {
        let mut ids: Vec<&str> = self.manifest.staende.iter().map(|s| s.datei.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }

    /// Belegter Speicher in Bytes (Summe der Standgrößen).
    pub fn pool_groesse(&self) -> u64 {
        self.manifest.staende.iter().map(|s| s.groesse).sum()
    }

    /// Pfad einer Standdatei.
    pub fn stand_pfad(&self, stand: &Stand) -> PathBuf {
        self.dir.join(stand.dateiname())
    }

    /// Freier Dateiname für einen neuen Stand: `basis.jjcad`, sonst
    /// `basis-2.jjcad`, `basis-3.jjcad` …
    ///
    /// Nötig, weil zwei Reiter mit gleichem Dateinamen im selben Zyklus
    /// denselben Zeitstempel bekommen und sich sonst überschreiben würden.
    fn freier_name(&self, basis: &str) -> String {
        let stamm = basis.trim_end_matches(".jjcad").to_string();
        let mut kandidat = format!("{stamm}.jjcad");
        let mut n = 2;
        while self.dir.join(&kandidat).exists() {
            kandidat = format!("{stamm}-{n}.jjcad");
            n += 1;
        }
        kandidat
    }

    /// Sichert den aktuellen Inhalt des Dokuments, wenn er sich seit dem
    /// neuesten Stand geändert hat.
    ///
    /// Liefert `Ok(None)`, wenn nichts zu tun war (unverändert). Die Zieldatei
    /// wird nicht angefasst; geschrieben wird nur der Stand im Pool.
    pub fn sichern(
        &mut self,
        doc: &mut Document,
        einst: &Einstellungen,
    ) -> Result<Option<Stand>, String> {
        self.ablegen(doc, einst, false, false, false)
    }

    /// Stand, der dem Inhalt der **Zieldatei** entspricht (T0129): nach dem
    /// Speichern und nach „Verwerfen“.
    ///
    /// Er wird **immer** geschrieben, auch bei gleichem Hash — er muss der
    /// neueste Stand sein, damit das Wiederherstellen die Datei gewinnen lässt
    /// und verworfene Änderungen nicht zurückkehren.
    pub fn sichern_dateistand(
        &mut self,
        doc: &mut Document,
        einst: &Einstellungen,
    ) -> Result<Option<Stand>, String> {
        self.ablegen(doc, einst, true, true, false)
    }

    /// R3.5: Den letzten Zustand **verworfener** Arbeit als Stand ablegen.
    ///
    /// Der Stand wird als `verworfen` markiert: er bleibt bis zu seinem
    /// Verfall im Pool und ist einzeln zu öffnen, aber weder das
    /// Wiederherstellen noch das Öffnen wählen ihn von selbst.
    pub fn sichern_verworfen(
        &mut self,
        doc: &mut Document,
        einst: &Einstellungen,
    ) -> Result<Option<Stand>, String> {
        self.ablegen(doc, einst, true, false, true)
    }

    fn ablegen(
        &mut self,
        doc: &mut Document,
        einst: &Einstellungen,
        erzwingen: bool,
        gesichert: bool,
        verworfen: bool,
    ) -> Result<Option<Stand>, String> {
        if doc.id.trim().is_empty() {
            doc.id = neue_uuid();
        }
        let json = persist::to_json(doc)?;
        let hash = hash_hex(json.as_bytes());
        if !erzwingen {
            if let Some(neu) = self.neuester(&doc.id) {
                if neu.hash == hash {
                    return Ok(None);
                }
            }
        }
        let zyklus = self.manifest.zyklus + 1;
        // Ohne Verlauf bleibt je Dokument genau **ein** Stand: der bisherige
        // neueste wird ersetzt, nicht ergänzt. Vorhandene Stände werden dabei
        // nicht gelöscht — die Einstellung verhindert nur neue. Verworfene
        // Arbeit (R3.5) wird nie ersetzt: ihr Stand ist der einzige Beleg.
        if !einst.verlauf && !verworfen {
            if let Some(alt) = self.neuester(&doc.id).cloned() {
                if !alt.verworfen {
                    let _ = std::fs::remove_file(self.stand_pfad(&alt));
                    self.manifest.staende.retain(|s| s.zyklus != alt.zyklus);
                }
            }
        }
        let mut stand = Stand {
            zyklus,
            datei: doc.id.clone(),
            name: doc.name.clone(),
            hash,
            groesse: json.len() as u64,
            zeit: zeitstempel(),
            gesichert,
            verworfen,
            datei_hash: zieldatei_hash(doc),
            datei_name: String::new(),
            pfad: doc.path.clone().unwrap_or_default(),
        };
        // Zwei Stände in derselben Sekunde (zwei Reiter mit gleichem Dateinamen)
        // dürfen sich nicht gegenseitig überschreiben.
        let basis = format!(
            "{}_autosave_{}",
            dateistamm(&doc.name),
            rueckwaertsdatum(&stand.zeit, stand.zyklus)
        );
        stand.datei_name = self.freier_name(&basis);
        atomar_schreiben(&self.stand_pfad(&stand), json.as_bytes())?;
        self.manifest.zyklus = zyklus;
        self.manifest.staende.push(stand.clone());
        self.manifest.staende.sort_by_key(|s| s.zyklus);
        self.manifest_schreiben()?;
        self.ausduennen(einst);
        Ok(Some(stand))
    }

    /// Lädt den Inhalt eines Standes als Dokument (ohne Pfad — der Aufrufer
    /// öffnet ihn als **neues** Dokument).
    pub fn laden(&self, stand: &Stand) -> Result<Document, String> {
        let text = std::fs::read_to_string(self.stand_pfad(stand))
            .map_err(|e| format!("Stand nicht lesbar: {e}"))?;
        let mut doc = persist::from_json(&text)?;
        // Der Stand gehört zu genau dieser Kennung — sie gewinnt, falls der
        // Inhalt keine (mehr) trägt.
        if doc.id.trim().is_empty() {
            doc.id = stand.datei.clone();
        }
        doc.path = None;
        doc.modified = false;
        Ok(doc)
    }

    /// Entfernt einen Stand samt Datei.
    pub fn entfernen(&mut self, zyklus: u64) -> Result<(), String> {
        if let Some(pos) = self.manifest.staende.iter().position(|s| s.zyklus == zyklus) {
            let stand = self.manifest.staende.remove(pos);
            let _ = std::fs::remove_file(self.stand_pfad(&stand));
            self.manifest_schreiben()?;
        }
        Ok(())
    }

    /// Setzt den Zykluszähler (nur für Tests und Wiederaufnahme).
    pub fn setze_zyklus(&mut self, zyklus: u64) {
        self.manifest.zyklus = zyklus;
    }

    /// Dünnt den Pool nach den Regeln aus `docs/SICHERUNG.md` §10 aus.
    ///
    /// Liefert die Zahl der gelöschten Stände.
    pub fn ausduennen(&mut self, einst: &Einstellungen) -> usize {
        let weg = zu_loeschen(&self.manifest.staende, self.manifest.zyklus, einst);
        if weg.is_empty() {
            return 0;
        }
        let mut entfernt: Vec<Stand> = Vec::new();
        self.manifest.staende.retain(|s| {
            if weg.contains(&s.zyklus) {
                entfernt.push(s.clone());
                false
            } else {
                true
            }
        });
        for s in &entfernt {
            let _ = std::fs::remove_file(self.stand_pfad(s));
        }
        let _ = self.manifest_schreiben();
        entfernt.len()
    }

    /// Wie viele Stände würde [`Sicherung::ausduennen`] mit **diesen**
    /// Einstellungen löschen? Reine Rechnung — es wird nichts angefasst.
    ///
    /// Grundlage der Warnung vor dem Übernehmen neuer Einstellungen.
    pub fn wuerde_verlieren(&self, einst: &Einstellungen) -> usize {
        zu_loeschen(&self.manifest.staende, self.manifest.zyklus, einst).len()
    }

    /// Stände, deren Zeichnung es nicht mehr gibt: Der Pfad ist vermerkt, die
    /// Datei fehlt. Altbestand ohne Pfad zählt nicht — dort ist unbekannt, ob
    /// die Zeichnung noch existiert.
    pub fn ohne_zeichnung(&self) -> Vec<&Stand> {
        self.manifest
            .staende
            .iter()
            .filter(|s| !s.pfad.is_empty() && !std::path::Path::new(&s.pfad).is_file())
            .collect()
    }

    /// R2.3: Wurde die Zieldatei seit dem letzten Stand außerhalb von
    /// JoJa-CAD verändert?
    ///
    /// Maßstab ist der Hash der Zieldatei, den der jüngste Stand festgehalten
    /// hat. Kennt er ihn nicht (Stand aus einer älteren Fassung, Datei damals
    /// nicht vorhanden), ist die Frage nicht entscheidbar und die Antwort
    /// `false`.
    pub fn extern_geaendert(&self, datei: &str, pfad: &Path) -> bool {
        let Ok(inhalt) = std::fs::read(pfad) else {
            return false;
        };
        let letzter = self
            .manifest
            .staende
            .iter()
            .filter(|s| s.datei == datei)
            .max_by_key(|s| s.zyklus);
        match letzter {
            Some(s) if !s.datei_hash.is_empty() => s.datei_hash != hash_hex(&inhalt),
            _ => false,
        }
    }

    /// Zahl der als „verworfen“ markierten Stände (R3.5) — der Papierkorb.
    pub fn verworfene(&self) -> usize {
        self.manifest.staende.iter().filter(|s| s.verworfen).count()
    }

    /// Alle Zeichnungen im Pool, alphabetisch nach Anzeigenamen (T0223).
    ///
    /// Die Liste ist der Inhalt des Sicherungsordners, gruppiert nach der
    /// Dokumentkennung: ein Eintrag je Zeichnung, mit der Zahl ihrer Stände
    /// und dem jüngsten Stand. Grundlage ist das Manifest — `oeffnen` hat
    /// Waisen (Standdatei ohne Eintrag) vorher entfernt, Datei und Liste
    /// stimmen also überein.
    pub fn zeichnungen(&self) -> Vec<DateiEintrag> {
        let mut out: Vec<DateiEintrag> = Vec::new();
        for s in &self.manifest.staende {
            match out.iter_mut().find(|e| e.datei == s.datei) {
                Some(eintrag) => {
                    eintrag.staende += 1;
                    if s.zyklus > eintrag.neuester_zyklus {
                        eintrag.neuester_zyklus = s.zyklus;
                        eintrag.neueste_zeit = s.zeit.clone();
                        // Name und Pfad folgen dem jüngsten Stand — er trägt
                        // den zuletzt benutzten Namen und Speicherort.
                        if !s.name.trim().is_empty() {
                            eintrag.name = s.name.clone();
                        }
                        if !s.pfad.trim().is_empty() {
                            eintrag.pfad = s.pfad.clone();
                        }
                    }
                }
                None => out.push(DateiEintrag {
                    datei: s.datei.clone(),
                    name: s.name.clone(),
                    pfad: s.pfad.clone(),
                    staende: 1,
                    neuester_zyklus: s.zyklus,
                    neueste_zeit: s.zeit.clone(),
                    aeltester_zyklus: s.zyklus,
                }),
            }
        }
        for e in &mut out {
            if e.name.trim().is_empty() {
                e.name = e
                    .pfad
                    .rsplit(['/', '\\'])
                    .next()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or("Zeichnung")
                    .to_string();
            }
            if let Some(alt) = self
                .manifest
                .staende
                .iter()
                .filter(|s| s.datei == e.datei)
                .min_by_key(|s| s.zyklus)
            {
                e.aeltester_zyklus = alt.zyklus;
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    /// Stände einer Zeichnung, **älteste zuerst** — der Zeitverlauf des
    /// Diagramms im Fenster „Wiederherstellen“ (T0223).
    pub fn staende_chronologisch(&self, datei: &str) -> Vec<&Stand> {
        let mut v: Vec<&Stand> = self
            .manifest
            .staende
            .iter()
            .filter(|s| s.datei == datei)
            .collect();
        v.sort_by_key(|s| s.zyklus);
        v
    }

    /// Löscht die Stände mit diesen Zyklusnummern (Datei und Eintrag).
    pub fn entferne_zyklen(&mut self, zyklen: &[u64]) -> usize {
        let mut entfernt: Vec<Stand> = Vec::new();
        self.manifest.staende.retain(|s| {
            if zyklen.contains(&s.zyklus) {
                entfernt.push(s.clone());
                false
            } else {
                true
            }
        });
        for s in &entfernt {
            let _ = std::fs::remove_file(self.stand_pfad(s));
        }
        if !entfernt.is_empty() {
            let _ = self.manifest_schreiben();
        }
        entfernt.len()
    }

    /// Entfernt Standdateien ohne Eintrag, Einträge ohne Datei und Reste
    /// abgebrochener Schreibvorgänge.
    fn waisen_entfernen(&mut self) {
        let bekannt: std::collections::BTreeSet<String> = self
            .manifest
            .staende
            .iter()
            .map(|s| s.dateiname())
            .collect();
        let mut vorhanden: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if let Ok(eintraege) = std::fs::read_dir(&self.dir) {
            for e in eintraege.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                // Standdateien heißen `<datei>_autosave_<datum>.jjcad`; der alte
                // Name `stand-<n>-<uuid>.jjcad` zählt mit, bis er umbenannt ist.
                let stand_datei = name.ends_with(".jjcad")
                    && (name.contains("_autosave_") || name.starts_with("stand-"));
                if stand_datei {
                    if bekannt.contains(&name) {
                        vorhanden.insert(name);
                    } else {
                        // Stand ohne Eintrag: Absturz zwischen Datei und Manifest.
                        let _ = std::fs::remove_file(e.path());
                    }
                } else if name.contains(".tmp-") {
                    // Rest eines abgebrochenen Schreibens.
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        // Eintrag ohne Datei: der Stand ist nicht mehr wiederherstellbar.
        self.manifest.staende.retain(|s| vorhanden.contains(&s.dateiname()));
        let _ = self.manifest_schreiben();
    }

    fn manifest_schreiben(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&self.manifest)
            .map_err(|e| format!("Manifest nicht schreibbar: {e}"))?;
        atomar_schreiben(&self.dir.join(MANIFEST_DATEI), text.as_bytes())
    }
}

/// Hash der Zieldatei eines Dokuments, wie sie gerade auf der Platte liegt.
/// Leer, wenn das Dokument keinen Pfad hat oder die Datei nicht lesbar ist.
fn zieldatei_hash(doc: &Document) -> String {
    let Some(pfad) = doc.path.as_deref() else {
        return String::new();
    };
    match std::fs::read(pfad) {
        Ok(b) => hash_hex(&b),
        Err(_) => String::new(),
    }
}

/// Zyklusnummern der Pins: je Dokument der neueste Stand.
fn pin_zyklen(staende: &[Stand]) -> Vec<u64> {
    let mut neueste: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for s in staende {
        let e = neueste.entry(s.datei.as_str()).or_insert(s.zyklus);
        if s.zyklus > *e {
            *e = s.zyklus;
        }
    }
    neueste.into_values().collect()
}

/// Zyklusnummern der Anker: je Dokument der älteste Stand (A.1).
fn anker_zyklen(staende: &[Stand]) -> Vec<u64> {
    let mut aelteste: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for s in staende {
        let e = aelteste.entry(s.datei.as_str()).or_insert(s.zyklus);
        if s.zyklus < *e {
            *e = s.zyklus;
        }
    }
    aelteste.into_values().collect()
}

/// Zahl der verschiedenen Dokumente.
fn anzahl_dateien(staende: &[Stand]) -> usize {
    let mut ids: Vec<&str> = staende.iter().map(|s| s.datei.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.len()
}

/// Kandidat mit den kleinsten Kosten (A.2 Schritt 4); `None`, wenn es keinen
/// gibt (jede Datei hat nur noch Pin und Anker).
///
/// ```text
/// Kosten(i) = (a_älter − a_jünger) / a_älter
/// ```
///
/// `a_älter` und `a_jünger` sind die Alter der **beiden Nachbarn in derselben
/// Datei**. Genommen wird der Stand, dessen Löschung die kleinste relative
/// Lücke hinterlässt — dort fällt sie am wenigsten auf.
fn kandidat_mit_kleinsten_kosten(staende: &[Stand], zyklus: u64) -> Option<u64> {
    let mut je_datei: std::collections::BTreeMap<&str, Vec<&Stand>> =
        std::collections::BTreeMap::new();
    for s in staende {
        je_datei.entry(s.datei.as_str()).or_default().push(s);
    }
    let mut bestes: Option<(u64, f64)> = None;
    for (_, mut gruppe) in je_datei {
        // Neueste zuerst: Index 0 = Pin, letzter = Anker; beide sind geschützt.
        gruppe.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        if gruppe.len() < 3 {
            continue;
        }
        for i in 1..gruppe.len() - 1 {
            let a_juenger = zyklus.saturating_sub(gruppe[i - 1].zyklus) as f64;
            let a_aelter = zyklus.saturating_sub(gruppe[i + 1].zyklus) as f64;
            if a_aelter <= 0.0 {
                continue;
            }
            let kosten = (a_aelter - a_juenger) / a_aelter;
            if bestes.map(|(_, k)| kosten < k).unwrap_or(true) {
                bestes = Some((gruppe[i].zyklus, kosten));
            }
        }
    }
    bestes.map(|(z, _)| z)
}

/// Welche Stände würde die Ausdünnung mit diesen Einstellungen löschen?
///
/// Reine Rechnung auf einer Kopie — dieselbe Funktion entscheidet beim
/// Ausdünnen **und** bei der Warnung vor neuen Einstellungen, damit die
/// angezeigte Zahl nicht von der tatsächlichen abweichen kann.
///
/// Ablauf nach Anhang A.2: (2) Fenster je Datei, (3) Obergrenze
/// `max(N, 2·Dateien)`, (4) Ausdünnen über die Kosten. Pin und Anker sind
/// geschützt; gelöscht wird nur, nichts wird neu berechnet (5).
fn zu_loeschen(staende: &[Stand], zyklus: u64, einst: &Einstellungen) -> Vec<u64> {
    let t = einst.zyklen();
    let mut rest: Vec<Stand> = staende.to_vec();
    let mut weg: Vec<u64> = Vec::new();

    // Schritt 2 — Fenster: Der älteste Stand einer Datei wird gelöscht,
    // solange sein jüngerer Nachbar mindestens T alt ist (der Anker rückt
    // nach). Ist der Pin selbst mindestens T alt, verfällt auch er und die
    // Datei verlässt den Pool.
    let mut ids: Vec<String> = rest.iter().map(|s| s.datei.clone()).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let mut gruppe: Vec<Stand> = rest.iter().filter(|s| s.datei == id).cloned().collect();
        gruppe.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        loop {
            if gruppe.len() >= 2 {
                let juenger = zyklus.saturating_sub(gruppe[gruppe.len() - 2].zyklus);
                if juenger >= t {
                    if let Some(s) = gruppe.pop() {
                        weg.push(s.zyklus);
                    }
                    continue;
                }
            } else if gruppe.len() == 1 {
                if zyklus.saturating_sub(gruppe[0].zyklus) >= t {
                    if let Some(s) = gruppe.pop() {
                        weg.push(s.zyklus);
                    }
                }
            }
            break;
        }
    }
    rest.retain(|s| !weg.contains(&s.zyklus));

    // Schritt 3 und 4 — Obergrenze, dann nach Kosten ausdünnen.
    let plaetze = einst.plaetze_effektiv(anzahl_dateien(&rest));
    while rest.len() > plaetze {
        let Some(z) = kandidat_mit_kleinsten_kosten(&rest, zyklus) else {
            break;
        };
        weg.push(z);
        rest.retain(|s| s.zyklus != z);
    }

    // Schritt 2 kann Zyklen doppelt melden, wenn zwei Regeln denselben Stand
    // treffen; doppelte Nummern würden beim Zählen doppelt wirken.
    weg.sort_unstable();
    weg.dedup();
    weg
}

/// Schreibt eine Datei atomar: temp → fsync → rename, danach das Verzeichnis
/// fsyncen. Ein Absturz hinterlässt so nie eine halbe Datei.
pub fn atomar_schreiben(ziel: &Path, inhalt: &[u8]) -> Result<(), String> {
    let name = ziel
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "datei".into());
    let tmp = ziel.with_file_name(format!(".{name}.tmp-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)
            .map_err(|e| format!("{} nicht anlegbar: {e}", tmp.display()))?;
        f.write_all(inhalt)
            .map_err(|e| format!("{} nicht schreibbar: {e}", tmp.display()))?;
        f.sync_all()
            .map_err(|e| format!("{} nicht sicherbar: {e}", tmp.display()))?;
    }
    // Windows ersetzt ein vorhandenes Ziel bei `rename` nicht; dort wird es
    // vorher entfernt. Die Lücke zwischen Entfernen und Umbenennen ist der
    // Preis — die temporäre Datei bleibt vollständig, ein Absturz dazwischen
    // hinterlässt also eine lesbare Sicherung statt einer halben Datei.
    let mut versuch = std::fs::rename(&tmp, ziel);
    if versuch.is_err() && ziel.exists() {
        let _ = std::fs::remove_file(ziel);
        versuch = std::fs::rename(&tmp, ziel);
    }
    versuch.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{} nicht ersetzbar: {e}", ziel.display())
    })?;
    #[cfg(unix)]
    if let Some(dir) = ziel.parent() {
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

/// SHA-256 als Hex.
pub fn hash_hex(inhalt: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(inhalt);
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Neue Dokumentkennung (UUID, Version 4).
///
/// Ohne eigene Kiste: Zeit, Prozess, Zähler und eine Stapeladresse werden
/// gehasht. Die Kennung muss nur eindeutig sein, nicht geheim.
pub fn neue_uuid() -> String {
    static ZAEHLER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let zaehler = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let lokal = 0u8;
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(zaehler.to_le_bytes());
    h.update((&lokal as *const u8 as usize).to_le_bytes());
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Zeitstempel für die Anzeige (Ortszeit, `JJJJ-MM-TTTHH:MM:SS`).
///
/// Unter Unix über `localtime_r`; auf anderen Systemen in UTC — der Wert dient
/// nur der Anzeige, gerechnet wird in Zyklen.
pub fn zeitstempel() -> String {
    let sekunden = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    zeitstempel_aus_sekunden(sekunden)
}

#[cfg(unix)]
fn zeitstempel_aus_sekunden(sekunden: i64) -> String {
    let t = sekunden as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    if !ok {
        return format!("{sekunden}");
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(not(unix))]
fn zeitstempel_aus_sekunden(sekunden: i64) -> String {
    // Ohne Ortszeit-Umrechnung: UTC, nur zur Anzeige.
    let tage = sekunden.div_euclid(86_400);
    let rest = sekunden.rem_euclid(86_400);
    let (jahr, monat, tag) = ziviles_datum(tage);
    format!(
        "{jahr:04}-{monat:02}-{tag:02}T{:02}:{:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

#[cfg(not(unix))]
fn ziviles_datum(tage: i64) -> (i64, u32, u32) {
    // Howard Hinnants Umrechnung Tage → Kalenderdatum.
    let z = tage + 719_468;
    let aera = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + aera * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Document;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-sicherung-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn doc_mit(name: &str, id: &str) -> Document {
        let mut d = Document::new();
        d.name = name.into();
        d.id = id.into();
        d
    }

    fn stand_dateien(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.ends_with(".jjcad") && n.contains("_autosave_")
            })
            .count()
    }

    /// §13.1: Stände ≤ max(N, Dateien).
    #[test]
    fn staende_bleiben_unter_der_obergrenze() {
        let dir = test_dir("obergrenze");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000001");
        for i in 0..120 {
            doc.entities.clear();
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        assert!(
            pool.manifest().staende.len() <= 30,
            "Pool über der Obergrenze: {}",
            pool.manifest().staende.len()
        );
        assert_eq!(stand_dateien(&dir), pool.manifest().staende.len());
    }

    /// §13.2: Der neueste Stand ist byte-identisch zum gesicherten Inhalt.
    #[test]
    fn neuester_stand_ist_byte_gleich() {
        let dir = test_dir("bytegleich");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000002");
        let stand = pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .expect("erster Stand");
        let inhalt = std::fs::read_to_string(pool.stand_pfad(&stand)).unwrap();
        assert_eq!(inhalt, persist::to_json(&doc).unwrap());
        assert_eq!(stand.hash, hash_hex(inhalt.as_bytes()));
        assert_eq!(stand.groesse, inhalt.len() as u64);
    }

    /// Unveränderter Inhalt erzeugt keinen neuen Zyklus.
    #[test]
    fn unveraendert_erzeugt_keinen_stand() {
        let dir = test_dir("unveraendert");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000003");
        assert!(pool.sichern(&mut doc, &Einstellungen::default()).unwrap().is_some());
        assert!(pool.sichern(&mut doc, &Einstellungen::default()).unwrap().is_none());
        assert_eq!(pool.manifest().staende.len(), 1);
    }

    /// R3.4 / A.3: Der neueste Stand (Pin) bleibt bis `T` erhalten; danach
    /// verfällt er samt Verlauf, die Datei verlässt den Pool.
    #[test]
    fn pin_bleibt_bis_t_und_verfaellt_danach() {
        let dir = test_dir("pin-verfall");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            stunden: 1,
            intervall_min: 1,
            ..Einstellungen::default()
        };
        assert_eq!(einst.zyklen(), 60, "T bei 1 Stunde und 1 Minute");
        let mut alt = doc_mit("Alt", "aaaaaaaa-0000-4000-8000-000000000004");
        pool.sichern(&mut alt, &einst).unwrap();
        let mut neu = doc_mit("Neu", "aaaaaaaa-0000-4000-8000-000000000005");
        for i in 0..30 {
            neu.name = format!("Neu {i}");
            pool.sichern(&mut neu, &einst).unwrap();
        }
        assert!(
            pool.neuester(&alt.id).is_some(),
            "vor T muss der Pin der ruhenden Datei bleiben"
        );
        for i in 0..60 {
            neu.name = format!("Neu weiter {i}");
            pool.sichern(&mut neu, &einst).unwrap();
        }
        assert!(
            pool.neuester(&alt.id).is_none(),
            "nach T muss der Pin verfallen (A.2 Schritt 2)"
        );
        assert!(
            !pool.manifest().staende.iter().any(|s| s.datei == alt.id),
            "die ruhende Datei belegt weiter Plätze"
        );
    }

    /// A.2 Schritt 2: Außer Pin und Anker ist kein Stand älter als `T`; der
    /// Anker rückt nach, sobald sein jüngerer Nachbar `T` erreicht hat.
    #[test]
    fn kein_stand_ausser_pin_und_anker_aelter_als_t() {
        let dir = test_dir("fenster");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 200,
            stunden: 1,
            intervall_min: 1,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000006");
        for i in 0..300 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        let n = pool.manifest().zyklus;
        let pins = pin_zyklen(pool.manifest().staende.as_slice());
        let anker = anker_zyklen(pool.manifest().staende.as_slice());
        for s in pool.manifest().staende.iter() {
            if pins.contains(&s.zyklus) || anker.contains(&s.zyklus) {
                continue;
            }
            assert!(
                n - s.zyklus <= einst.zyklen(),
                "Stand {} ist älter als T ({})",
                s.zyklus,
                einst.zyklen()
            );
        }
        // Der Anker ist der älteste Stand und mindestens T alt.
        let aeltester = pool.manifest().staende.iter().map(|s| s.zyklus).min().unwrap();
        assert_eq!(anker, vec![aeltester], "der Anker ist nicht der älteste Stand");
        assert!(
            n - aeltester >= einst.zyklen(),
            "die Tiefe T wird nicht erreicht ({} < {})",
            n - aeltester,
            einst.zyklen()
        );
    }

    /// §13.5: Ein abgebrochenes Schreiben hinterlässt keine halbe Datei und
    /// keinen Pool über der Obergrenze; Waisen verschwinden beim Start.
    #[test]
    fn absturz_hinterlaesst_keine_halbe_datei() {
        let dir = test_dir("absturz");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000007");
        for i in 0..40 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        // Absturz mitten im Schreiben: halbe Standdatei ohne Eintrag und ein
        // temp-Rest.
        std::fs::write(dir.join("stand-9999-aaaaaaaa.jjcad"), b"{\"format\":\"jjc").unwrap();
        std::fs::write(dir.join(".manifest.json.tmp-1"), b"{\"format\":").unwrap();

        let wieder = Sicherung::oeffnen(&dir);
        assert!(
            !dir.join("stand-9999-aaaaaaaa.jjcad").exists(),
            "Waise muss beim Start entfernt werden"
        );
        assert!(
            !dir.join(".manifest.json.tmp-1").exists(),
            "temp-Rest muss beim Start entfernt werden"
        );
        assert!(wieder.manifest().staende.len() <= 30);
        assert_eq!(stand_dateien(&dir), wieder.manifest().staende.len());
    }

    /// Solange der Pool Platz hat, wird nichts gelöscht — nahe am Jetzt
    /// liegen die Stände damit lückenlos (A.2, Anmerkung zu den Kosten).
    #[test]
    fn unterhalb_der_obergrenze_wird_nichts_geloescht() {
        let dir = test_dir("dichtezone");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 150,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000008");
        for i in 0..100 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        assert_eq!(
            pool.manifest().staende.len(),
            100,
            "unter der Obergrenze darf nichts verfallen"
        );
        let n = pool.manifest().zyklus;
        for k in 0..100 {
            let zyklus = n - k;
            assert!(
                pool.manifest().staende.iter().any(|s| s.zyklus == zyklus),
                "Zyklus {zyklus} fehlt"
            );
        }
    }

    /// A.2 Schritt 3: Der Pool bleibt bei jeder Dateizahl an oder unter
    /// `max(N, 2 · Dateien)` — er wächst nicht mit der Nutzungsdauer.
    #[test]
    fn pool_bleibt_unter_der_obergrenze() {
        let einst = Einstellungen {
            plaetze: 60,
            ..Einstellungen::default()
        };
        let mut zahlen = Vec::new();
        // So viele Stände je Datei, dass der Pool in jedem Fall voll ist.
        for (dateien, je) in [(5usize, 12usize), (20, 3), (50, 2)] {
            let dir = test_dir(&format!("dateien-{dateien}"));
            let mut pool = Sicherung::oeffnen(&dir);
            for d in 0..dateien {
                let mut doc = doc_mit(
                    &format!("Plan {d}"),
                    &format!("aaaaaaaa-0000-4000-8000-{d:012}"),
                );
                for i in 0..je {
                    doc.name = format!("Plan {d} {i}");
                    pool.sichern(&mut doc, &einst).unwrap();
                }
            }
            let grenze = einst.plaetze_effektiv(dateien);
            assert!(
                pool.manifest().staende.len() <= grenze,
                "{dateien} Dateien: {} Stände über der Grenze {grenze}",
                pool.manifest().staende.len()
            );
            zahlen.push(pool.manifest().staende.len());
        }
        assert!(zahlen[0] <= 60, "5 Dateien: {zahlen:?}");
        assert!(zahlen[2] <= 100, "50 Dateien: {zahlen:?}");
    }

    /// §13.8: Die Zieldatei bleibt bei jedem Autosave unverändert.
    #[test]
    fn zieldatei_bleibt_unveraendert() {
        let dir = test_dir("zieldatei");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000009");
        let ziel = dir.join("Plan.jjcad");
        persist::save_document(&mut doc, &ziel).unwrap();
        let vorher = std::fs::read(&ziel).unwrap();
        for i in 0..5 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &Einstellungen::default()).unwrap();
        }
        let nachher = std::fs::read(&ziel).unwrap();
        assert_eq!(vorher, nachher, "die Sicherung hat die Zieldatei verändert");
    }

    /// §13.9: „Verlauf sichern“ aus → genau ein Stand je Dokument.
    #[test]
    fn verlauf_aus_haelt_einen_stand_je_dokument() {
        let dir = test_dir("verlauf-aus");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            verlauf: false,
            ..Einstellungen::default()
        };
        let mut a = doc_mit("A", "aaaaaaaa-0000-4000-8000-00000000000a");
        let mut b = doc_mit("B", "aaaaaaaa-0000-4000-8000-00000000000b");
        for i in 0..10 {
            a.name = format!("A {i}");
            b.name = format!("B {i}");
            pool.sichern(&mut a, &einst).unwrap();
            pool.sichern(&mut b, &einst).unwrap();
        }
        assert_eq!(pool.manifest().staende.len(), 2);
        assert_eq!(pool.staende_von(&a.id).len(), 1);
        assert_eq!(pool.staende_von(&b.id).len(), 1);
        assert_eq!(pool.neuester(&a.id).unwrap().name, "A 9");
    }

    /// „Verlauf aus“ verhindert nur **neue** Stände — vorhandene Sicherungen
    /// werden nicht gelöscht (ein Haken darf nichts vernichten).
    #[test]
    fn verlauf_aus_loescht_keinen_vorhandenen_verlauf() {
        let dir = test_dir("verlauf-aus-kein-loeschen");
        let mut pool = Sicherung::oeffnen(&dir);
        let an = Einstellungen::default();
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-00000000000d");
        for i in 0..5 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &an).unwrap();
        }
        assert_eq!(pool.staende_von(&doc.id).len(), 5);

        let aus = Einstellungen {
            verlauf: false,
            ..Einstellungen::default()
        };
        doc.name = "Plan neu".into();
        pool.sichern(&mut doc, &aus).unwrap();
        assert_eq!(
            pool.staende_von(&doc.id).len(),
            5,
            "das Abschalten hat vorhandene Stände gelöscht"
        );
        assert_eq!(pool.neuester(&doc.id).unwrap().name, "Plan neu");
    }

    /// §13.10: Ein Stand lässt sich als neues Dokument laden; die Zieldatei
    /// bleibt unberührt.
    #[test]
    fn wiederherstellen_oeffnet_neues_dokument() {
        let dir = test_dir("wiederherstellen");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-00000000000c");
        let ziel = dir.join("Plan.jjcad");
        persist::save_document(&mut doc, &ziel).unwrap();
        let vorher = std::fs::read(&ziel).unwrap();
        doc.name = "Plan geändert".into();
        let stand = pool.sichern(&mut doc, &Einstellungen::default()).unwrap().unwrap();

        let neu = pool.laden(&stand).unwrap();
        assert_eq!(neu.name, "Plan geändert");
        assert!(neu.path.is_none(), "der Stand darf keinen Pfad tragen");
        assert_eq!(std::fs::read(&ziel).unwrap(), vorher);
    }

    /// Die Formel aus A.3 trifft die Tabelle der Vorgabe.
    #[test]
    fn staende_fuer_abstand_folgt_der_tabelle() {
        // Tabelle: Woche = 40 h = 2400 Zyklen bei I = 1 min.
        let woche = 2400.0;
        let faelle = [(0.10, 62.0), (0.15, 43.0), (0.20, 33.0)];
        for (g, k) in faelle {
            let ist = staende_fuer_abstand(g, woche);
            assert!(
                (ist - k).abs() <= 1.0,
                "g = {g}: {ist} Stände statt {k}"
            );
        }
        // Monat (160 h) und Quartal (480 h): T geht nur logarithmisch ein.
        let monat = staende_fuer_abstand(0.15, 9600.0);
        assert!((monat - 51.0).abs() <= 1.0, "Monat: {monat}");
        let quartal = staende_fuer_abstand(0.15, 28800.0);
        assert!((quartal - 58.0).abs() <= 1.0, "Quartal: {quartal}");
    }

    /// Hin und zurück: zu `k` Ständen gehört ein Abstand, der wieder `k`
    /// Stände braucht. Ohne diese Gegenprobe wäre die Bisektion blind.
    #[test]
    fn abstand_ist_die_umkehrung_von_staende_fuer_abstand() {
        let t = 2400.0;
        for k in [5.0, 12.0, 33.0, 62.0, 150.0] {
            let g = abstand_fuer_staende(k, t);
            let zurueck = staende_fuer_abstand(g, t);
            assert!(
                (zurueck - k).abs() < 0.05,
                "k = {k}: g = {g} → {zurueck}"
            );
        }
        // Ohne Platz für Verlauf gibt es keine Lücke zu verteilen.
        assert_eq!(abstand_fuer_staende(1.0, t), 1.0);
        assert_eq!(abstand_fuer_staende(0.5, t), 1.0);
    }

    /// Die Vorgaben der Vorgabe (SRR A.1/A.4): N = 150, T = eine Woche,
    /// I = 1 Minute.
    #[test]
    fn vorgaben_entsprechen_der_spezifikation() {
        let e = Einstellungen::default();
        assert_eq!(e.plaetze, 150);
        assert_eq!(e.stunden, 40);
        assert_eq!(e.intervall_min, 1);
        assert_eq!(e.zyklen(), 2400);
        assert_eq!(
            STUNDEN_WAHL,
            [8, 40, 160, 480],
            "Tag, Woche, Monat, Quartal"
        );
    }


    /// Die Kennung ist eindeutig und hat die Form einer UUID.
    #[test]
    fn kennungen_sind_eindeutig() {
        let a = neue_uuid();
        let b = neue_uuid();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'4');
    }

    /// Ein Dokument ohne Kennung bekommt beim Sichern eine.
    #[test]
    fn dokument_ohne_kennung_bekommt_eine() {
        let dir = test_dir("ohne-kennung");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = Document::new();
        doc.id.clear();
        let stand = pool.sichern(&mut doc, &Einstellungen::default()).unwrap().unwrap();
        assert!(!doc.id.is_empty());
        assert_eq!(stand.datei, doc.id);
    }

    /// Ein Arbeitsstand trägt `gesichert = false` (ungespeicherte Arbeit), ein
    /// Dateistand `true` — und der Dateistand wird auch bei gleichem Inhalt
    /// geschrieben, damit er der neueste bleibt.
    #[test]
    fn dateistand_wird_auch_bei_gleichem_inhalt_geschrieben() {
        let dir = test_dir("dateistand");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut doc = Document::new();

        let a = pool.sichern(&mut doc, &einst).unwrap().unwrap();
        assert!(!a.gesichert, "Arbeitsstand darf nicht als Dateistand gelten");

        // Gleicher Inhalt: der normale Weg schweigt …
        assert!(pool.sichern(&mut doc, &einst).unwrap().is_none());

        // … der Dateistand nicht, und er ist danach der neueste.
        let b = pool.sichern_dateistand(&mut doc, &einst).unwrap().unwrap();
        assert!(b.gesichert);
        assert_eq!(b.zyklus, a.zyklus + 1);
        assert!(pool.neuester(&doc.id).unwrap().gesichert);
    }

    /// T0188: Die Vorausrechnung nennt genau so viele Stände, wie die
    /// Ausdünnung danach wirklich löscht.
    #[test]
    fn wuerde_verlieren_zaehlt_wie_ausduennen() {
        for plaetze in [30u32, 60, 100] {
            let dir = test_dir(&format!("voraus-{plaetze}"));
            let mut pool = Sicherung::oeffnen(&dir);
            let einst = Einstellungen {
                plaetze: 200,
                ..Einstellungen::default()
            };
            let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-0000000000e1");
            for i in 0..120 {
                doc.name = format!("Plan {i}");
                pool.sichern(&mut doc, &einst).unwrap();
            }
            let streng = Einstellungen {
                plaetze,
                ..einst
            };
            let vorher = pool.manifest().staende.len();
            let angesagt = pool.wuerde_verlieren(&streng);
            assert_eq!(vorher, 120, "Vorbereitung: alle Stände angelegt");
            assert!(angesagt > 0, "bei {plaetze} Plätzen muss etwas wegfallen");
            let geloescht = pool.ausduennen(&streng);
            assert_eq!(
                angesagt, geloescht,
                "angesagte und gelöschte Zahl weichen ab ({plaetze} Plätze)"
            );
            assert_eq!(pool.manifest().staende.len(), vorher - geloescht);
        }
    }

    /// T0188: Ein Stand merkt sich die Zieldatei; fehlt sie, findet das
    /// Aufräumen ihn — Stände ohne Pfad bleiben unangetastet.
    #[test]
    fn pfad_und_ohne_zeichnung() {
        let dir = test_dir("pfad");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let vorhanden = dir.join("da.jjcad");
        std::fs::write(&vorhanden, "{}").unwrap();

        let mut mit = doc_mit("Da", "aaaaaaaa-0000-4000-8000-0000000000f1");
        mit.path = Some(vorhanden.to_string_lossy().to_string());
        pool.sichern(&mut mit, &einst).unwrap();

        let mut ohne = doc_mit("Weg", "aaaaaaaa-0000-4000-8000-0000000000f2");
        ohne.path = Some(dir.join("weg.jjcad").to_string_lossy().to_string());
        pool.sichern(&mut ohne, &einst).unwrap();

        let mut unbekannt = doc_mit("Unbenannt", "aaaaaaaa-0000-4000-8000-0000000000f3");
        unbekannt.path = None;
        pool.sichern(&mut unbekannt, &einst).unwrap();

        let waisen: Vec<String> = pool
            .ohne_zeichnung()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(waisen, vec!["Weg".to_string()], "falsche Waisen: {waisen:?}");

        let zyklen: Vec<u64> = pool.ohne_zeichnung().iter().map(|s| s.zyklus).collect();
        assert_eq!(pool.entferne_zyklen(&zyklen), 1);
        assert_eq!(pool.manifest().staende.len(), 2, "zu viel gelöscht");
    }

    /// Der Name eines Standes: Zieldatei + `autosave` + Rückwärtsdatum.
    #[test]
    fn standname_aus_datei_autosave_und_rueckwaertsdatum() {
        let stand = Stand {
            zyklus: 7,
            datei: "egal".into(),
            name: "Wohnhaus Musterstraße".into(),
            hash: "h".into(),
            groesse: 1,
            zeit: "2026-11-08T15:30:12".into(),
            gesichert: false,
            verworfen: false,
            datei_hash: String::new(),
            datei_name: String::new(),
            pfad: String::new(),
        };
        assert_eq!(
            stand.dateiname(),
            "wohnhaus-musterstrasse_autosave_20261108-153012.jjcad"
        );

        // Ohne Zeitstempel steht die Zyklusnummer — auch sortierbar.
        let alt = Stand {
            zeit: String::new(),
            ..stand.clone()
        };
        assert_eq!(
            alt.dateiname(),
            "wohnhaus-musterstrasse_autosave_zyklus-000000007.jjcad"
        );
    }

    /// Zwei Stände mit gleichem Namen in derselben Sekunde bekommen eine
    /// Nummer, statt sich zu überschreiben.
    #[test]
    fn gleicher_name_in_derselben_sekunde_bekommt_eine_nummer() {
        let dir = test_dir("namenskollision");
        let pool = Sicherung::oeffnen(&dir);
        let basis = "plan_autosave_20261108-153012";
        assert_eq!(pool.freier_name(basis), format!("{basis}.jjcad"));
        std::fs::write(dir.join(format!("{basis}.jjcad")), b"{}").unwrap();
        assert_eq!(pool.freier_name(basis), format!("{basis}-2.jjcad"));
        std::fs::write(dir.join(format!("{basis}-2.jjcad")), b"{}").unwrap();
        assert_eq!(pool.freier_name(basis), format!("{basis}-3.jjcad"));
    }

    fn stand_mit(zyklus: u64, datei: &str) -> Stand {
        Stand {
            zyklus,
            datei: datei.into(),
            name: "Plan".into(),
            hash: "h".into(),
            groesse: 1,
            zeit: String::new(),
            gesichert: false,
            verworfen: false,
            datei_hash: String::new(),
            datei_name: String::new(),
            pfad: String::new(),
        }
    }

    /// A.2 Schritt 4: Gelöscht wird der Kandidat mit der kleinsten relativen
    /// Lücke. Bei gleichmäßigem Abstand liegt sie direkt hinter dem Anker —
    /// nahe am Jetzt ist ein einzelner Zyklus schon eine große Lücke.
    #[test]
    fn kosten_waehlen_die_kleinste_relative_luecke() {
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let staende: Vec<Stand> = (1..=31).map(|z| stand_mit(z, "A")).collect();
        let weg = zu_loeschen(&staende, 31, &einst);
        assert_eq!(weg.len(), 1, "genau ein Stand muss wegfallen: {weg:?}");
        assert_eq!(
            weg[0], 2,
            "hinter dem Anker ist die relative Lücke am kleinsten"
        );
    }

    /// Pin und Anker überleben das Ausdünnen; alles dazwischen darf fallen.
    #[test]
    fn ausduennen_schuetzt_pin_und_anker() {
        let dir = test_dir("pin-anker");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000011");
        for i in 0..120 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        let staende = pool.staende_von(&doc.id);
        assert_eq!(staende.len(), 30, "Obergrenze nicht eingehalten");
        let neueste = staende.first().unwrap().zyklus;
        let aelteste = staende.last().unwrap().zyklus;
        assert_eq!(neueste, pool.manifest().zyklus, "der Pin fehlt");
        assert_eq!(aelteste, 1, "der Anker fehlt");
        // Zweimal ausdünnen ändert nichts mehr (Monotonie, A.2 Schritt 5).
        assert_eq!(pool.ausduennen(&einst), 0);
    }

    /// A.2 Schritt 3: Die Obergrenze ist `max(N, 2 · Dateien)` — jede Datei
    /// behält Platz für Pin und Anker.
    #[test]
    fn obergrenze_laesst_zwei_plaetze_je_datei() {
        let dir = test_dir("zwei-je-datei");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        for d in 0..20 {
            let mut doc =
                doc_mit(&format!("Plan {d}"), &format!("aaaaaaaa-0000-4000-8000-{d:012}"));
            for i in 0..3 {
                doc.name = format!("Plan {d} {i}");
                pool.sichern(&mut doc, &einst).unwrap();
            }
        }
        assert_eq!(einst.plaetze_effektiv(20), 40);
        assert_eq!(pool.manifest().staende.len(), 40, "Pool nicht auf 2·F begrenzt");
        for d in 0..20 {
            let id = format!("aaaaaaaa-0000-4000-8000-{d:012}");
            assert_eq!(
                pool.staende_von(&id).len(),
                2,
                "Datei {d} hat nicht Pin und Anker"
            );
        }
    }

    /// R3.5: Verworfene Arbeit bleibt als Stand liegen, ist als `verworfen`
    /// markiert und wird nicht vom Verlauf-aus-Modus ersetzt.
    #[test]
    fn verworfen_ist_markiert_und_bleibt_liegen() {
        let dir = test_dir("verworfen");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            verlauf: false,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000012");
        doc.name = "verworfene Arbeit".into();
        let stand = pool.sichern_verworfen(&mut doc, &einst).unwrap().unwrap();
        assert!(stand.verworfen, "Stand nicht als verworfen markiert");
        assert!(!stand.gesichert, "verworfene Arbeit ist kein Dateistand");
        // Dateistand danach: der verworfene Stand bleibt, der Dateistand kommt
        // dazu — auch bei abgeschaltetem Verlauf.
        doc.name = "Datei".into();
        pool.sichern_dateistand(&mut doc, &einst).unwrap();
        let alle = pool.staende_von(&doc.id);
        assert_eq!(alle.len(), 2, "der verworfene Stand wurde ersetzt");
        assert_eq!(alle.iter().filter(|s| s.verworfen).count(), 1);
        assert_eq!(pool.neuester(&doc.id).unwrap().name, "Datei");
    }

    /// R2.3: Eine extern geänderte Zieldatei wird erkannt.
    #[test]
    fn extern_geaenderte_datei_wird_erkannt() {
        let dir = test_dir("extern");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let ziel = dir.join("plan.jjcad");
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000013");
        doc.path = Some(ziel.to_string_lossy().to_string());
        persist::save_document(&mut doc, &ziel).unwrap();
        pool.sichern_dateistand(&mut doc, &einst).unwrap();

        assert!(
            !pool.extern_geaendert(&doc.id, &ziel),
            "unveränderte Datei darf keinen Konflikt melden"
        );
        // Fremder Schreiber: anderer Inhalt.
        std::fs::write(&ziel, b"{\"format\":\"jjcad\",\"extern\":true}").unwrap();
        assert!(
            pool.extern_geaendert(&doc.id, &ziel),
            "extern geänderte Datei nicht erkannt"
        );
    }


    /// Stände aus einem älteren Pool (`stand-<n>-<uuid>.jjcad`) werden beim
    /// Öffnen umbenannt statt als Waise verworfen.
    #[test]
    fn altbestand_wird_umbenannt() {
        let dir = test_dir("altbestand");
        let mut m = Manifest::default();
        m.zyklus = 3;
        m.staende.push(Stand {
            zyklus: 3,
            datei: "abc".into(),
            name: "Plan".into(),
            hash: "h".into(),
            groesse: 2,
            zeit: "2026-11-08T15:30:12".into(),
            gesichert: false,
            verworfen: false,
            datei_hash: String::new(),
            datei_name: String::new(),
            pfad: String::new(),
        });
        std::fs::write(dir.join("manifest.json"), serde_json::to_string(&m).unwrap()).unwrap();
        std::fs::write(dir.join("stand-3-abc.jjcad"), "{}").unwrap();

        let pool = Sicherung::oeffnen(&dir);
        assert_eq!(pool.manifest().staende.len(), 1, "Stand wurde verworfen");
        assert!(
            dir.join("plan_autosave_20261108-153012.jjcad").exists(),
            "Stand nicht umbenannt"
        );
        assert!(!dir.join("stand-3-abc.jjcad").exists(), "alter Name liegt noch");
    }
}

#[cfg(test)]
mod tests_zeichnungen {
    //! T0223: die Zeichnungsliste für das Fenster „Wiederherstellen“ —
    //! ein Eintrag je Zeichnung, der Verlauf chronologisch.

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-zeichnungen-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn doc(name: &str, id: &str) -> Document {
        let mut d = Document::new();
        d.name = name.into();
        d.id = id.into();
        d
    }

    #[test]
    fn liste_gruppiert_nach_dokument_und_zaehlt_die_staende() {
        let dir = test_dir("gruppe");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut haus = doc("Wohnhaus", "aaaaaaaa-0000-4000-8000-00000000000a");
        let mut garage = doc("Garage", "bbbbbbbb-0000-4000-8000-00000000000b");
        for i in 0..3 {
            haus.name = format!("Wohnhaus {i}");
            pool.sichern(&mut haus, &einst).unwrap().unwrap();
        }
        for i in 0..2 {
            garage.name = format!("Garage {i}");
            pool.sichern(&mut garage, &einst).unwrap().unwrap();
        }

        let liste = pool.zeichnungen();
        assert_eq!(liste.len(), 2, "eine Zeile je Zeichnung: {liste:?}");
        // Alphabetisch nach Anzeigenamen.
        assert_eq!(liste[0].name, "Garage 1");
        assert_eq!(liste[0].staende, 2);
        assert_eq!(liste[1].name, "Wohnhaus 2");
        assert_eq!(liste[1].staende, 3);
        // Der jüngste Stand bestimmt Name, Zeit und Speicherort.
        assert_eq!(
            liste[1].neuester_zyklus,
            pool.staende_chronologisch(&haus.id).last().unwrap().zyklus
        );
        assert!(liste[1].aeltester_zyklus < liste[1].neuester_zyklus);
    }

    #[test]
    fn verlauf_ist_chronologisch() {
        let dir = test_dir("verlauf");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut plan = doc("Plan", "cccccccc-0000-4000-8000-00000000000c");
        for i in 0..4 {
            plan.name = format!("Plan {i}");
            pool.sichern(&mut plan, &einst).unwrap().unwrap();
        }
        let verlauf = pool.staende_chronologisch(&plan.id);
        assert_eq!(verlauf.len(), 4);
        assert!(
            verlauf.windows(2).all(|w| w[0].zyklus < w[1].zyklus),
            "die Stände stehen nicht in der Zeitfolge"
        );
        assert_eq!(verlauf.last().unwrap().name, "Plan 3");
        // Eine unbekannte Kennung liefert eine leere Liste, keinen Fehler.
        assert!(pool.staende_chronologisch("gibt-es-nicht").is_empty());
        assert!(pool.zeichnungen().iter().all(|e| e.staende > 0));
    }
}
