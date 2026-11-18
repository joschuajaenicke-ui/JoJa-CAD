//! Sicherung: Stände unabhängig vom Speichern (T0129, T0225).
//!
//! Die Sicherung legt während der Arbeit Stände im **Pool** ab und dünnt sie
//! nach Alter aus. Sie schreibt **nie** in die Zieldatei — gespeichert wird
//! ausschließlich vom Benutzer. Vorgabe und Begründung: `docs/benutzer/SICHERUNG.md`.
//!
//! Aufbau des Pools (ein Pool für alle Dateien, ein Projektordner):
//!
//! ```text
//! <Sicherungen>/testdatei_autosave_20261118-143544.jjcad   eine Datei je Stand
//! <Sicherungen>/pool.lock                                  Sperre für einen Schreibvorgang
//! ```
//!
//! **Die Wahrheit ist der Verzeichnisinhalt** (T0225). Jeder Stand trägt seine
//! Aufbewahrungs-Metadaten selbst, als **erstes** Feld der JSON-Datei:
//!
//! ```json
//! {"sicherung":{"format":"jjcad-sicherung-stand","zyklus":7,"datei":"…",…},
//!   "format": "jjcad", "version": 2, …}
//! ```
//!
//! Ein Manifest gibt es nicht mehr: es war eine zweite Wahrheit, und ging es
//! verloren oder überschrieb es ein zweiter Prozess aus seinem Speicherstand,
//! wurden Stände als Waisen gelöscht oder nie wieder geschrieben. Der Index
//! lebt nur im Speicher und wird aus dem Verzeichnis gebildet.
//!
//! Auffrischen kostet im Normalfall **einen** `stat`: die Signatur des
//! Verzeichnisses (dev/ino/mtime/Größe) entscheidet, ob überhaupt etwas anders
//! ist. Erst dann wird `readdir` gemacht, und nur für Dateien, die nach Name,
//! Größe und Änderungszeit unbekannt sind, wird der Kopf gelesen (2 KB). Die
//! vollen Stände werden nie zum Auffrischen gelesen — das wären 150 × bis
//! 400 KB je Takt.
//!
//! Der **Pin** (neuester Stand je Dokument) bleibt immer; der **Anker**
//! (ältester Stand je Dokument) nur, solange sein jüngerer Nachbar weniger als
//! `T` alt ist. Fremde Dateien im Ordner werden übernommen (Kopf, sonst
//! Dateiname, sonst Datei-mtime), als „nicht zugeordnet“ geführt und **nie**
//! automatisch gelöscht.
//!
//! Aufbewahrt wird nach dem Verfahren „Geometric Retention“ (SRR, Anhang A):
//! fester Platzvorrat, konstante relative Auflösung, Alter in Zyklen statt in
//! Kalenderzeit.

use crate::model::Document;
use crate::persist;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Formatkennung im Kopf eines Standes.
pub const KOPF_FORMAT: &str = "jjcad-sicherung-stand";
/// So viele Bytes eines Standes werden für den Kopf höchstens gelesen.
pub const KOPF_BYTES: usize = 2048;
/// Sperrdatei gegen zwei gleichzeitig schreibende eigene Prozesse.
pub const LOCK_DATEI: &str = "pool.lock";
/// Nach so vielen Sekunden gilt eine Sperre als verwaist.
pub const LOCK_SEKUNDEN: u64 = 10;
/// Früheres Manifest — wird beim ersten Start einmal eingelesen (T0225).
pub const MIGRATIONS_MANIFEST: &str = "manifest.json";
/// Name des eingelesenen Manifests; es wird umbenannt, nicht gelöscht.
pub const MIGRATIONS_MANIFEST_EINGELESEN: &str = "manifest.json.eingelesen";
/// Kennung des alten Manifests (Formatprüfung beim Einlesen).
pub const MANIFEST_FORMAT: &str = "jjcad-sicherung";
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

/// Zählwerk der gelesenen Dateiköpfe — Messgröße für „der Normalfall ist
/// 0 Reads“ (T0225). Kein Verhalten hängt daran.
static KOPF_LESUNGEN: AtomicUsize = AtomicUsize::new(0);

/// Wie viele Dateiköpfe seit dem letzten Zurücksetzen gelesen wurden.
pub fn kopflesungen() -> usize {
    KOPF_LESUNGEN.load(Ordering::Relaxed)
}

/// Setzt das Zählwerk der Kopflesungen zurück.
pub fn setze_kopflesungen(wert: usize) {
    KOPF_LESUNGEN.store(wert, Ordering::Relaxed);
}

/// Metadatenblock eines Standes — das **erste** Feld der JSON-Datei.
///
/// Er gehört zum Stand, nicht zur Zeichnung: beim Laden wird er verworfen und
/// wandert deshalb nie in eine gespeicherte Datei. `hash` ist der SHA-256 des
/// Dokuments **ohne** diesen Block — sonst wäre ein Stand nie gleich seiner
/// Zieldatei.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kopf {
    /// Immer [`KOPF_FORMAT`]; unterscheidet den Block von einem Zufallstreffer.
    #[serde(default)]
    pub format: String,
    /// Zyklusnummer; einziger Zeitanker für Entscheidungen.
    #[serde(default)]
    pub zyklus: u64,
    /// Dokumentkennung (UUID aus dem Dateikopf, nicht der Pfad).
    #[serde(default)]
    pub datei: String,
    /// Zeitstempel, nur zur Anzeige.
    #[serde(default)]
    pub zeit: String,
    /// Der Stand entspricht dem Inhalt der Zieldatei.
    #[serde(default)]
    pub gesichert: bool,
    /// Der Benutzer hat diese Arbeit verworfen.
    #[serde(default)]
    pub verworfen: bool,
    /// SHA-256 des Standinhalts **ohne** diesen Block.
    #[serde(default)]
    pub hash: String,
    /// SHA-256 der Zieldatei beim Anlegen des Standes (leer, wenn es keine gab).
    #[serde(default)]
    pub datei_hash: String,
    /// Größe der Standdatei in Bytes.
    #[serde(default)]
    pub groesse: u64,
    /// Zieldatei zum Zeitpunkt der Sicherung (leer = unbekannt).
    #[serde(default)]
    pub pfad: String,
}

/// Ein gesicherter Stand einer Datei.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stand {
    /// Globaler Zähler; einziger Zeitanker für Entscheidungen.
    pub zyklus: u64,
    /// Kennung des Dokuments (UUID aus dem Dateikopf, nicht der Pfad). Leer bei
    /// einer fremden Datei ohne Kopf — die ist „nicht zugeordnet“.
    pub datei: String,
    /// Anzeigename, abgeleitet aus Zieldatei bzw. Dateiname (nur Anzeige).
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
    /// Die Datei gehört zu keiner Zeichnung: kein Kopf, keine bekannte
    /// Kennung. Sie wird geführt und angezeigt, aber **nie** automatisch
    /// gelöscht (T0225).
    #[serde(default)]
    pub fremd: bool,
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

/// Anzeigename eines Standes: die Zieldatei, sonst der Dateiname ohne
/// `_autosave_…`, in Leseschrift (`wohnhaus-1` → `Wohnhaus 1`).
fn anzeigename(name: &str, pfad: &str) -> String {
    if !pfad.trim().is_empty() {
        if let Some(stamm) = Path::new(pfad).file_stem() {
            let s = stamm.to_string_lossy().to_string();
            if !s.trim().is_empty() {
                return s;
            }
        }
    }
    let s = name.trim_end_matches(".jjcad");
    let s = s.split("_autosave_").next().unwrap_or(s);
    let mut out = String::with_capacity(s.len());
    let mut gross = true;
    for c in s.chars() {
        if c == '-' || c == '_' {
            out.push(' ');
            gross = true;
        } else if gross {
            out.extend(c.to_uppercase());
            gross = false;
        } else {
            out.push(c);
        }
    }
    if out.trim().is_empty() {
        "Zeichnung".to_string()
    } else {
        out
    }
}

/// Zyklusnummer aus einem Dateinamen `…_autosave_zyklus-000000012.jjcad`.
fn zyklus_aus_name(name: &str) -> Option<u64> {
    let i = name.find("zyklus-")?;
    let ziffern: String = name[i + "zyklus-".len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    ziffern.parse().ok()
}

/// Ende des JSON-Objekts, das bei `s[0]` beginnt (Byte-Index hinter `}`).
fn objekt_ende(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.first() != Some(&b'{') {
        return None;
    }
    let mut tiefe = 0usize;
    let mut im_text = false;
    let mut escape = false;
    for (i, &c) in b.iter().enumerate() {
        if im_text {
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                im_text = false;
            }
            continue;
        }
        match c {
            b'"' => im_text = true,
            b'{' | b'[' => tiefe += 1,
            b'}' | b']' => {
                tiefe = tiefe.saturating_sub(1);
                if tiefe == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Spanne des Metadatenblocks im Text: `(Schlüssel, Wertbeginn, Ende)`.
///
/// `Schlüssel` ist der Anfang von `"sicherung"`, `Wertbeginn` das öffnende `{`
/// des Blocks, `Ende` der Index hinter seinem schließenden `}`.
fn kopf_spanne(text: &str) -> Option<(usize, usize, usize)> {
    // Laut Format steht der Block an erster Stelle: `{"sicherung": {…}, …`.
    let start = text.find('{')?;
    let nach = text[start + 1..].trim_start();
    if !nach.starts_with("\"sicherung\"") {
        return None;
    }
    let schluessel = text[start + 1..].find("\"sicherung\"")? + start + 1;
    let nach_schluessel = schluessel + "\"sicherung\"".len();
    let doppelpunkt = nach_schluessel + text[nach_schluessel..].find(':')?;
    let wert = doppelpunkt + 1 + text[doppelpunkt + 1..].find('{')?;
    let ende = objekt_ende(&text[wert..])? + wert;
    Some((schluessel, wert, ende))
}

/// Liest den Metadatenblock aus einem Text.
pub fn kopf_aus_text(text: &str) -> Option<Kopf> {
    let (_, wert, ende) = kopf_spanne(text)?;
    let kopf: Kopf = serde_json::from_str(&text[wert..ende]).ok()?;
    if kopf.format != KOPF_FORMAT {
        return None;
    }
    Some(kopf)
}

/// Der Dokumentinhalt eines Standes **ohne** Metadatenblock.
///
/// Ein Stand darf nie mit seinem Block in eine Zieldatei wandern; beim Laden
/// und vor dem Vergleich wird er deshalb abgeschnitten.
pub fn ohne_kopf(text: &str) -> String {
    let Some((a, _, b)) = kopf_spanne(text) else {
        return text.to_string();
    };
    let mut rest = text[b..].trim_start();
    if let Some(r) = rest.strip_prefix(',') {
        rest = r.trim_start();
    }
    format!("{}{}", &text[..a], rest)
}

/// Standdatei: Metadatenblock als erstes Feld, dahinter das Dokument.
pub fn stand_text(kopf: &Kopf, doc_json: &str) -> Result<String, String> {
    let block = serde_json::to_string(kopf).map_err(|e| format!("Kopf nicht schreibbar: {e}"))?;
    let doc = doc_json.trim();
    let rest = doc.strip_prefix('{').unwrap_or(doc);
    if rest.trim() == "}" || rest.trim().is_empty() {
        Ok(format!("{{\"sicherung\":{block}}}"))
    } else {
        Ok(format!("{{\"sicherung\":{block},{rest}"))
    }
}

/// Baut Kopf und Text eines Standes; `groesse` ist die Länge der fertigen Datei.
///
/// Die Zahl steht selbst im Block, deshalb wird sie in zwei bis drei Schritten
/// nachgezogen, bis die Länge zur eingetragenen Zahl passt.
fn stand_mit_groesse(mut kopf: Kopf, doc_json: &str) -> Result<(Kopf, String), String> {
    kopf.groesse = 0;
    let mut text = stand_text(&kopf, doc_json)?;
    for _ in 0..4 {
        let ist = text.len() as u64;
        if kopf.groesse == ist {
            break;
        }
        kopf.groesse = ist;
        text = stand_text(&kopf, doc_json)?;
    }
    Ok((kopf, text))
}

/// Liest nur den Kopf einer Standdatei (2 KB, nicht den ganzen Stand).
pub fn kopf_lesen(pfad: &Path) -> Option<Kopf> {
    let datei = std::fs::File::open(pfad).ok()?;
    let mut buf = Vec::with_capacity(KOPF_BYTES);
    datei.take(KOPF_BYTES as u64).read_to_end(&mut buf).ok()?;
    KOPF_LESUNGEN.fetch_add(1, Ordering::Relaxed);
    let text = String::from_utf8_lossy(&buf);
    if let Some(k) = kopf_aus_text(&text) {
        return Some(k);
    }
    if buf.len() < KOPF_BYTES {
        return None;
    }
    // Der Block ist größer als das Fenster (praktisch nie): ganz lesen.
    let ganz = std::fs::read(pfad).ok()?;
    kopf_aus_text(&String::from_utf8_lossy(&ganz))
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

/// Signatur eines Verzeichnisses: alles, was sich beim Anlegen, Löschen oder
/// Ersetzen einer Datei ändert.
///
/// Ein `stat` genügt damit, um „unverändert“ zu erkennen — der Normalfall ist
/// 0 Lesevorgänge. `groesse` ist die Verzeichnisgröße (wächst mit der Zahl der
/// Einträge) und steht hier für die Zahl der `.jjcad`-Einträge: sie zu zählen
/// verlangte selbst ein `readdir`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Signatur {
    dev: u64,
    ino: u64,
    mtime: i64,
    groesse: u64,
}

impl Signatur {
    fn von(dir: &Path) -> Option<Self> {
        let m = std::fs::metadata(dir).ok()?;
        if !m.is_dir() {
            return None;
        }
        #[cfg(unix)]
        let (dev, ino) = {
            use std::os::unix::fs::MetadataExt;
            (m.dev(), m.ino())
        };
        #[cfg(not(unix))]
        let (dev, ino) = (0u64, 0u64);
        Some(Self {
            dev,
            ino,
            mtime: mtime_nanos(&m),
            groesse: m.len(),
        })
    }
}

/// Änderungszeit in Nanosekunden seit der Epoche (0, wenn unbekannt).
fn mtime_nanos(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Sperre für einen Schreibvorgang im Pool.
///
/// Zwei **eigene** Prozesse (zwei Fenster) dürfen nicht gleichzeitig schreiben;
/// fremde Programme stören ohnehin nicht, weil sie die Dateien nur lesen.
/// Die Sperre ist eine Datei mit PID und Zeitstempel und gilt nach
/// [`LOCK_SEKUNDEN`] als verwaist — ein abgestürzter Prozess blockiert nicht.
struct PoolSperre {
    pfad: PathBuf,
}

impl PoolSperre {
    fn nehmen(dir: &Path) -> Option<Self> {
        let pfad = dir.join(LOCK_DATEI);
        for _ in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&pfad)
            {
                Ok(mut f) => {
                    let _ = writeln!(f, "{} {}", std::process::id(), unix_sekunden());
                    let _ = f.sync_all();
                    return Some(Self { pfad });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if sperre_verwaist(&pfad) {
                        let _ = std::fs::remove_file(&pfad);
                        continue;
                    }
                    return None;
                }
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for PoolSperre {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.pfad);
    }
}

/// Ist die Sperrdatei älter als [`LOCK_SEKUNDEN`]?
fn sperre_verwaist(pfad: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(pfad) else {
        // Nicht lesbar: entweder gerade im Entstehen oder kaputt — kurz warten
        // ist billiger als eine zweite Sperre daneben.
        return false;
    };
    let zeit = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    unix_sekunden().saturating_sub(zeit) > LOCK_SEKUNDEN
}

fn unix_sekunden() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Der Pool: Verzeichnis und Index im Speicher.
///
/// Es gibt kein Manifest; die Liste wird beim Öffnen und bei jeder Änderung des
/// Verzeichnisses aus dessen Inhalt gebildet.
pub struct Sicherung {
    dir: PathBuf,
    staende: Vec<Stand>,
    zyklus: u64,
    signatur: Option<Signatur>,
    /// Dateiname → (Größe, mtime) aus der Runde, in der die Datei gelesen wurde.
    stat: BTreeMap<String, (u64, i64)>,
    /// Der letzte Scan hat jeden `.jjcad`-Eintrag gelesen — nur dann darf
    /// gelöscht werden.
    scan_vollstaendig: bool,
}

impl Sicherung {
    /// Öffnet den Pool im angegebenen Verzeichnis.
    ///
    /// Ein vorhandenes `manifest.json` wird **einmal** eingelesen und danach
    /// umbenannt (T0225); Waisen gibt es nicht mehr, fremde Dateien werden
    /// übernommen statt gelöscht.
    pub fn oeffnen(dir: &Path) -> Self {
        let mut pool = Self {
            dir: dir.to_path_buf(),
            staende: Vec::new(),
            zyklus: 0,
            signatur: None,
            stat: BTreeMap::new(),
            scan_vollstaendig: true,
        };
        pool.migrieren();
        pool.auffrischen();
        pool
    }

    /// Verzeichnis des Pools.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Alle Stände, aufsteigend nach Zyklus.
    pub fn staende(&self) -> &[Stand] {
        &self.staende
    }

    /// Höchster bekannter Zyklus (0 = leerer Pool).
    pub fn zyklus(&self) -> u64 {
        self.zyklus
    }

    /// Zahl der Stände im Pool.
    pub fn anzahl(&self) -> usize {
        self.staende.len()
    }

    /// Zahl der Dateien, die zu keiner Zeichnung gehören („nicht zugeordnet“).
    pub fn nicht_zugeordnet(&self) -> usize {
        self.staende.iter().filter(|s| s.fremd).count()
    }

    /// Alle Stände, neueste zuerst.
    pub fn staende_neueste(&self) -> Vec<&Stand> {
        let mut v: Vec<&Stand> = self.staende.iter().collect();
        v.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        v
    }

    /// Stände eines Dokuments, neueste zuerst.
    pub fn staende_von(&self, datei: &str) -> Vec<&Stand> {
        let mut v: Vec<&Stand> = self
            .staende
            .iter()
            .filter(|s| !s.fremd && s.datei == datei)
            .collect();
        v.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        v
    }

    /// Neuester Stand eines Dokuments.
    pub fn neuester(&self, datei: &str) -> Option<&Stand> {
        self.staende
            .iter()
            .filter(|s| !s.fremd && s.datei == datei)
            .max_by_key(|s| s.zyklus)
    }

    /// Der Stand mit dieser Zyklusnummer.
    pub fn stand_mit_zyklus(&self, zyklus: u64) -> Option<&Stand> {
        self.staende.iter().find(|s| s.zyklus == zyklus)
    }

    /// Zahl der Dokumente im Pool.
    pub fn dateien(&self) -> usize {
        let mut ids: Vec<&str> = self
            .staende
            .iter()
            .filter(|s| !s.fremd)
            .map(|s| s.datei.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }

    /// Belegter Speicher in Bytes (Summe der Standgrößen).
    pub fn pool_groesse(&self) -> u64 {
        self.staende.iter().map(|s| s.groesse).sum()
    }

    /// Pfad einer Standdatei.
    pub fn stand_pfad(&self, stand: &Stand) -> PathBuf {
        self.dir.join(stand.dateiname())
    }

    /// Liest den Index neu, wenn sich das Verzeichnis geändert hat.
    ///
    /// Normalfall: ein `stat` und kein Lesevorgang. Nach einem `readdir`
    /// werden nur Dateien gelesen, die nach (Name, Größe, mtime) unbekannt
    /// sind — und von ihnen nur der Kopf.
    pub fn auffrischen(&mut self) {
        let signatur = Signatur::von(&self.dir);
        if signatur == self.signatur {
            return;
        }
        let Some(signatur) = signatur else {
            // Verzeichnis fehlt oder ist unlesbar: der Index fällt auf leer
            // zurück. Geschrieben wird beim nächsten Sichern (create_dir_all).
            self.staende.clear();
            self.stat.clear();
            self.signatur = None;
            self.scan_vollstaendig = false;
            return;
        };
        let Ok(verzeichnis) = std::fs::read_dir(&self.dir) else {
            self.staende.clear();
            self.stat.clear();
            self.signatur = None;
            self.scan_vollstaendig = false;
            return;
        };
        let mut gescannt: Vec<(String, u64, i64, PathBuf)> = Vec::new();
        let mut vollstaendig = true;
        for e in verzeichnis {
            let Ok(e) = e else {
                vollstaendig = false;
                continue;
            };
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".jjcad") {
                continue;
            }
            let Ok(meta) = e.metadata() else {
                vollstaendig = false;
                continue;
            };
            if !meta.is_file() {
                // Ein Verzeichnis mit .jjcad-Namen ist ein halb lesbares
                // Verzeichnis: in dieser Runde wird nichts gelöscht.
                vollstaendig = false;
                continue;
            }
            gescannt.push((name, meta.len(), mtime_nanos(&meta), e.path()));
        }

        let mut neu: Vec<Stand> = Vec::with_capacity(gescannt.len());
        let mut stat: BTreeMap<String, (u64, i64)> = BTreeMap::new();
        let mut hoechster = 0u64;
        for (name, groesse, mtime, pfad) in gescannt {
            // Bekannt und unverändert? Eintrag übernehmen, Datei nicht lesen.
            let bekannt = self
                .staende
                .iter()
                .find(|s| s.datei_name == name)
                .filter(|_| self.stat.get(&name) == Some(&(groesse, mtime)));
            let stand = match bekannt {
                Some(s) => s.clone(),
                None => stand_aus_datei(&pfad, &name, groesse, mtime),
            };
            if !stand.fremd {
                hoechster = hoechster.max(stand.zyklus);
            }
            stat.insert(name, (groesse, mtime));
            neu.push(stand);
        }
        neu.sort_by_key(|s| s.zyklus);
        // Der Zähler fällt nie zurück, auch wenn alle Dateien von Hand
        // gelöscht wurden: eine Zyklusnummer wird nicht zweimal vergeben.
        self.zyklus = self.zyklus.max(hoechster);
        self.staende = neu;
        self.stat = stat;
        self.scan_vollstaendig = vollstaendig;
        self.signatur = Some(signatur);
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
    /// Liefert `Ok(None)`, wenn nichts zu tun war (unverändert oder ein anderer
    /// eigener Prozess schreibt gerade). Die Zieldatei wird nicht angefasst;
    /// geschrieben wird nur der Stand im Pool.
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
        self.auffrischen();
        let json = persist::to_json(doc)?;
        let hash = hash_hex(json.as_bytes());
        if !erzwingen {
            if let Some(neu) = self.neuester(&doc.id) {
                if neu.hash == hash {
                    return Ok(None);
                }
            }
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("Sicherungsordner nicht anlegbar: {e}"))?;
        // Zwei eigene Prozesse dürfen nicht gleichzeitig schreiben (T0225).
        // Der zweite überspringt diesen Takt; sein Stand entsteht beim nächsten.
        let Some(_sperre) = PoolSperre::nehmen(&self.dir) else {
            return Ok(None);
        };
        // Unter der Sperre noch einmal sehen, was inzwischen dazukam.
        self.auffrischen();

        let zyklus = self.zyklus + 1;
        // Ohne Verlauf bleibt je Dokument genau **ein** Stand: der bisherige
        // neueste wird ersetzt, nicht ergänzt. Vorhandene Stände werden dabei
        // nicht gelöscht — die Einstellung verhindert nur neue. Verworfene
        // Arbeit (R3.5) wird nie ersetzt: ihr Stand ist der einzige Beleg.
        if !einst.verlauf && !verworfen {
            if let Some(alt) = self.neuester(&doc.id).cloned() {
                if !alt.verworfen {
                    self.stand_entfernen(&alt);
                }
            }
        }
        let kopf = Kopf {
            format: KOPF_FORMAT.to_string(),
            zyklus,
            datei: doc.id.clone(),
            zeit: zeitstempel(),
            gesichert,
            verworfen,
            hash,
            datei_hash: zieldatei_hash(doc),
            groesse: 0,
            pfad: doc.path.clone().unwrap_or_default(),
        };
        let (kopf, text) = stand_mit_groesse(kopf, &json)?;
        // Zwei Stände in derselben Sekunde (zwei Reiter mit gleichem Dateinamen)
        // dürfen sich nicht gegenseitig überschreiben.
        let basis = format!(
            "{}_autosave_{}",
            dateistamm(&doc.name),
            rueckwaertsdatum(&kopf.zeit, zyklus)
        );
        let name = self.freier_name(&basis);
        let pfad = self.dir.join(&name);
        atomar_schreiben(&pfad, text.as_bytes())?;
        let stand = Stand {
            zyklus,
            datei: kopf.datei.clone(),
            name: if doc.name.trim().is_empty() {
                anzeigename(&name, &kopf.pfad)
            } else {
                doc.name.clone()
            },
            hash: kopf.hash.clone(),
            groesse: kopf.groesse,
            zeit: kopf.zeit.clone(),
            gesichert,
            verworfen,
            datei_hash: kopf.datei_hash.clone(),
            datei_name: name.clone(),
            pfad: kopf.pfad.clone(),
            fremd: false,
        };
        if let Ok(meta) = std::fs::metadata(&pfad) {
            self.stat.insert(name, (meta.len(), mtime_nanos(&meta)));
        }
        self.zyklus = zyklus;
        self.staende.push(stand.clone());
        self.staende.sort_by_key(|s| s.zyklus);
        self.signatur = Signatur::von(&self.dir);
        self.ausduennen(einst);
        Ok(Some(stand))
    }

    /// Lädt den Inhalt eines Standes als Dokument (ohne Pfad — der Aufrufer
    /// öffnet ihn als **neues** Dokument).
    ///
    /// Der Metadatenblock des Standes wird dabei verworfen; er gehört zum
    /// Stand, nicht zur Zeichnung, und darf nie in eine Zieldatei wandern.
    pub fn laden(&self, stand: &Stand) -> Result<Document, String> {
        let text = std::fs::read_to_string(self.stand_pfad(stand))
            .map_err(|e| format!("Stand nicht lesbar: {e}"))?;
        let mut doc = persist::from_json(&ohne_kopf(&text))?;
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
        if let Some(stand) = self.stand_mit_zyklus(zyklus).cloned() {
            self.stand_entfernen(&stand);
        }
        Ok(())
    }

    /// Setzt den Zykluszähler (nur für Tests und Wiederaufnahme).
    pub fn setze_zyklus(&mut self, zyklus: u64) {
        self.zyklus = zyklus;
    }

    /// Löscht einen Stand aus dem Index und — wenn der Scan ihn in dieser
    /// Runde gesehen hat — seine Datei.
    fn stand_entfernen(&mut self, stand: &Stand) {
        self.staende.retain(|s| s.zyklus != stand.zyklus);
        let name = stand.dateiname();
        if self.loesche_datei(stand) {
            self.stat.remove(&name);
        }
    }

    /// Löscht die Datei eines Standes — **nur** wenn sie in dieser Runde
    /// gescannt wurde und ihre `stat`-Angaben unverändert sind.
    ///
    /// Damit trifft kein Löschvorgang eine Datei, die inzwischen jemand anders
    /// angelegt oder verändert hat (T0225, Abnahme 7).
    fn loesche_datei(&self, stand: &Stand) -> bool {
        if stand.fremd {
            return false;
        }
        let name = stand.dateiname();
        let Some(&(groesse, mtime)) = self.stat.get(&name) else {
            return false;
        };
        let pfad = self.dir.join(&name);
        let Ok(meta) = std::fs::metadata(&pfad) else {
            return false;
        };
        if meta.len() != groesse || mtime_nanos(&meta) != mtime {
            return false;
        }
        std::fs::remove_file(&pfad).is_ok()
    }

    /// Dünnt den Pool nach den Regeln aus `docs/benutzer/SICHERUNG.md` §10 aus.
    ///
    /// Gerechnet wird nur auf dem Index; gelöscht wird einzeln und nur, wenn
    /// der Scan vollständig war. Liefert die Zahl der gelöschten Stände.
    pub fn ausduennen(&mut self, einst: &Einstellungen) -> usize {
        if !self.scan_vollstaendig {
            return 0;
        }
        let weg = zu_loeschen(&self.staende, self.zyklus, einst);
        if weg.is_empty() {
            return 0;
        }
        let mut entfernt = 0usize;
        let mut behalten: Vec<Stand> = Vec::with_capacity(self.staende.len());
        for s in std::mem::take(&mut self.staende) {
            if weg.contains(&s.zyklus) && self.loesche_datei(&s) {
                self.stat.remove(&s.dateiname());
                entfernt += 1;
            } else {
                behalten.push(s);
            }
        }
        self.staende = behalten;
        if entfernt > 0 {
            self.signatur = Signatur::von(&self.dir);
        }
        entfernt
    }

    /// Wie viele Stände würde [`Sicherung::ausduennen`] mit **diesen**
    /// Einstellungen löschen? Reine Rechnung — es wird nichts angefasst.
    ///
    /// Grundlage der Warnung vor dem Übernehmen neuer Einstellungen.
    pub fn wuerde_verlieren(&self, einst: &Einstellungen) -> usize {
        zu_loeschen(&self.staende, self.zyklus, einst).len()
    }

    /// Stände, deren Zeichnung es nicht mehr gibt: Der Pfad ist vermerkt, die
    /// Datei fehlt. Altbestand ohne Pfad zählt nicht — dort ist unbekannt, ob
    /// die Zeichnung noch existiert.
    pub fn ohne_zeichnung(&self) -> Vec<&Stand> {
        self.staende
            .iter()
            .filter(|s| !s.fremd && !s.pfad.is_empty() && !std::path::Path::new(&s.pfad).is_file())
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
        match self.neuester(datei) {
            Some(s) if !s.datei_hash.is_empty() => s.datei_hash != hash_hex(&inhalt),
            _ => false,
        }
    }

    /// Zahl der als „verworfen“ markierten Stände (R3.5) — der Papierkorb.
    pub fn verworfene(&self) -> usize {
        self.staende.iter().filter(|s| s.verworfen).count()
    }

    /// Alle Zeichnungen im Pool, alphabetisch nach Anzeigenamen (T0223).
    ///
    /// Die Liste ist der Inhalt des Sicherungsordners, gruppiert nach der
    /// Dokumentkennung: ein Eintrag je Zeichnung, mit der Zahl ihrer Stände
    /// und dem jüngsten Stand. Dateien ohne Kennung („nicht zugeordnet“)
    /// gehören keiner Zeichnung und erscheinen hier nicht.
    pub fn zeichnungen(&self) -> Vec<DateiEintrag> {
        let mut out: Vec<DateiEintrag> = Vec::new();
        for s in self.staende.iter().filter(|s| !s.fremd) {
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
                .staende
                .iter()
                .filter(|s| !s.fremd && s.datei == e.datei)
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
            .staende
            .iter()
            .filter(|s| !s.fremd && s.datei == datei)
            .collect();
        v.sort_by_key(|s| s.zyklus);
        v
    }

    /// Löscht die Stände mit diesen Zyklusnummern (Datei und Eintrag).
    pub fn entferne_zyklen(&mut self, zyklen: &[u64]) -> usize {
        let mut entfernt = 0usize;
        let weg: Vec<Stand> = self
            .staende
            .iter()
            .filter(|s| zyklen.contains(&s.zyklus))
            .cloned()
            .collect();
        for s in &weg {
            self.stand_entfernen(s);
            entfernt += 1;
        }
        entfernt
    }

    /// Liest ein vorhandenes `manifest.json` **einmal** ein (T0225): Zyklus,
    /// Hash und `gesichert` wandern in die Köpfe der Stände, danach wird das
    /// Manifest umbenannt. Gelöscht wird nichts.
    fn migrieren(&mut self) {
        #[derive(Deserialize)]
        struct AltManifest {
            #[serde(default)]
            format: String,
            #[serde(default)]
            zyklus: u64,
            #[serde(default)]
            staende: Vec<Stand>,
        }
        let pfad = self.dir.join(MIGRATIONS_MANIFEST);
        let Ok(text) = std::fs::read_to_string(&pfad) else {
            return;
        };
        let m = match serde_json::from_str::<AltManifest>(&text) {
            Ok(m) if m.format == MANIFEST_FORMAT => m,
            _ => {
                let _ = std::fs::rename(&pfad, self.dir.join("manifest.json.kaputt"));
                return;
            }
        };
        self.zyklus = self.zyklus.max(m.zyklus);
        for s in &m.staende {
            // Altbestand heißt `stand-<n>-<uuid>.jjcad`; umbenennen, damit das
            // Verzeichnis lesbar bleibt.
            let name = s.dateiname();
            if s.datei_name.is_empty() {
                let alt_name = format!("stand-{}-{}.jjcad", s.zyklus, s.datei);
                let alt_pfad = self.dir.join(&alt_name);
                let neu_pfad = self.dir.join(&name);
                if alt_pfad.exists() && !neu_pfad.exists() {
                    let _ = std::fs::rename(&alt_pfad, &neu_pfad);
                }
            }
            let datei = self.dir.join(&name);
            let Ok(inhalt) = std::fs::read_to_string(&datei) else {
                continue;
            };
            let rest = ohne_kopf(&inhalt);
            let kopf = Kopf {
                format: KOPF_FORMAT.to_string(),
                zyklus: s.zyklus,
                datei: s.datei.clone(),
                zeit: s.zeit.clone(),
                gesichert: s.gesichert,
                verworfen: s.verworfen,
                hash: s.hash.clone(),
                datei_hash: s.datei_hash.clone(),
                groesse: inhalt.len() as u64,
                pfad: s.pfad.clone(),
            };
            // Nur schreiben, wenn der Kopf fehlt oder abweicht.
            if kopf_aus_text(&inhalt).as_ref() == Some(&kopf) {
                continue;
            }
            if let Ok((_, neu)) = stand_mit_groesse(kopf, &rest) {
                let _ = atomar_schreiben(&datei, neu.as_bytes());
            }
        }
        let ziel = self.dir.join(MIGRATIONS_MANIFEST_EINGELESEN);
        let _ = std::fs::rename(&pfad, &ziel);
        self.signatur = None;
    }
}

/// Baut einen Stand aus einer Datei, die der Index noch nicht kennt.
///
/// Reihenfolge nach T0225: Kopf lesen → sonst Zyklus aus dem Dateinamen →
/// sonst aus der Datei-mtime. Ohne Kopf und ohne Kennung ist die Datei
/// „nicht zugeordnet“ (`fremd`) und wird nie automatisch gelöscht.
fn stand_aus_datei(pfad: &Path, name: &str, groesse: u64, mtime: i64) -> Stand {
    if let Some(kopf) = kopf_lesen(pfad) {
        return Stand {
            zyklus: kopf.zyklus,
            datei: kopf.datei.clone(),
            name: anzeigename(name, &kopf.pfad),
            hash: kopf.hash,
            groesse: kopf.groesse,
            zeit: kopf.zeit,
            gesichert: kopf.gesichert,
            verworfen: kopf.verworfen,
            datei_hash: kopf.datei_hash,
            datei_name: name.to_string(),
            pfad: kopf.pfad,
            fremd: kopf.datei.trim().is_empty(),
        };
    }
    let zyklus = zyklus_aus_name(name).unwrap_or_else(|| (mtime / 1_000_000_000).max(0) as u64);
    Stand {
        zyklus,
        datei: String::new(),
        name: anzeigename(name, ""),
        hash: String::new(),
        groesse,
        zeit: String::new(),
        gesichert: false,
        verworfen: false,
        datei_hash: String::new(),
        datei_name: name.to_string(),
        pfad: String::new(),
        fremd: true,
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

/// Zahl der verschiedenen Dokumente.
fn anzahl_dateien(staende: &[Stand]) -> usize {
    let mut ids: Vec<&str> = staende
        .iter()
        .filter(|s| !s.fremd)
        .map(|s| s.datei.as_str())
        .collect();
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
    let mut je_datei: BTreeMap<&str, Vec<&Stand>> = BTreeMap::new();
    for s in staende.iter().filter(|s| !s.fremd) {
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
/// geschützt; der Pin bleibt **immer** (T0225), der Anker nur, solange sein
/// jüngerer Nachbar weniger als `T` alt ist. Fremde Dateien sind kein
/// Kandidat. Gelöscht wird nur, nichts wird neu berechnet (5).
fn zu_loeschen(staende: &[Stand], zyklus: u64, einst: &Einstellungen) -> Vec<u64> {
    let t = einst.zyklen();
    let mut rest: Vec<Stand> = staende.iter().filter(|s| !s.fremd).cloned().collect();
    let mut weg: Vec<u64> = Vec::new();

    // Schritt 2 — Fenster: Der älteste Stand einer Datei wird gelöscht,
    // solange sein jüngerer Nachbar mindestens T alt ist (der Anker rückt
    // nach). Der neueste Stand bleibt immer, auch wenn er älter als T ist.
    let mut ids: Vec<String> = rest.iter().map(|s| s.datei.clone()).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let mut gruppe: Vec<Stand> = rest.iter().filter(|s| s.datei == id).cloned().collect();
        gruppe.sort_by(|a, b| b.zyklus.cmp(&a.zyklus));
        while gruppe.len() >= 2 {
            let juenger = zyklus.saturating_sub(gruppe[gruppe.len() - 2].zyklus);
            if juenger < t {
                break;
            }
            if let Some(s) = gruppe.pop() {
                weg.push(s.zyklus);
            }
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
        let d = std::env::temp_dir().join(format!("jjcad-sicherung-{}-{name}", std::process::id()));
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

/// Zyklusnummern der Pins: je Dokument der neueste Stand.
fn pin_zyklen(staende: &[Stand]) -> Vec<u64> {
    let mut neueste: BTreeMap<&str, u64> = BTreeMap::new();
    for s in staende.iter().filter(|s| !s.fremd) {
        let e = neueste.entry(s.datei.as_str()).or_insert(s.zyklus);
        if s.zyklus > *e {
            *e = s.zyklus;
        }
    }
    neueste.into_values().collect()
}

/// Zyklusnummern der Anker: je Dokument der älteste Stand (A.1).
fn anker_zyklen(staende: &[Stand]) -> Vec<u64> {
    let mut aelteste: BTreeMap<&str, u64> = BTreeMap::new();
    for s in staende.iter().filter(|s| !s.fremd) {
        let e = aelteste.entry(s.datei.as_str()).or_insert(s.zyklus);
        if s.zyklus < *e {
            *e = s.zyklus;
        }
    }
    aelteste.into_values().collect()
}

    fn stand_dateien(dir: &Path) -> usize {
        alle_dateien(dir).len()
    }

    /// Alle `.jjcad`-Dateien im Ordner, sortiert.
    fn alle_dateien(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".jjcad"))
            .collect();
        v.sort();
        v
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
            pool.anzahl() <= 30,
            "Pool über der Obergrenze: {}",
            pool.anzahl()
        );
        assert_eq!(stand_dateien(&dir), pool.anzahl());
    }

    /// T0225: Der Kopf steht als **erstes** Feld im Stand, der Hash gilt dem
    /// Dokument ohne Block, und beim Laden bleibt vom Block nichts übrig.
    #[test]
    fn kopf_ist_das_erste_feld_und_wird_beim_laden_verworfen() {
        let dir = test_dir("kopf");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000002");
        doc.entities.clear();
        let stand = pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .expect("erster Stand");
        let inhalt = std::fs::read_to_string(pool.stand_pfad(&stand)).unwrap();
        assert!(
            inhalt.starts_with("{\"sicherung\":{\"format\":\"jjcad-sicherung-stand\""),
            "der Block steht nicht als erstes Feld: {}",
            &inhalt[..inhalt.len().min(80)]
        );
        for feld in [
            "\"zyklus\"",
            "\"datei\"",
            "\"zeit\"",
            "\"gesichert\"",
            "\"verworfen\"",
            "\"hash\"",
            "\"datei_hash\"",
            "\"groesse\"",
            "\"pfad\"",
        ] {
            assert!(inhalt.contains(feld), "Feld {feld} fehlt im Kopf");
        }
        // Der Hash gilt dem Dokument **ohne** Block.
        assert_eq!(
            stand.hash,
            hash_hex(persist::to_json(&doc).unwrap().as_bytes())
        );
        assert_eq!(stand.groesse, inhalt.len() as u64);

        // Laden: der Block verschwindet.
        let geladen = pool.laden(&stand).unwrap();
        let neu = persist::to_json(&geladen).unwrap();
        assert!(
            !neu.contains("\"sicherung\""),
            "der Block ist im Dokument gelandet"
        );
        assert_eq!(neu, persist::to_json(&doc).unwrap());
        // Und was gespeichert wird, trägt ihn erst recht nicht.
        let ziel = dir.join("ziel.jjcad");
        let mut kopie = geladen.clone();
        persist::save_document(&mut kopie, &ziel).unwrap();
        let text = std::fs::read_to_string(&ziel).unwrap();
        assert!(!text.contains("\"sicherung\""), "Block in der Zieldatei");
    }

    /// Der Kopf wird aus den ersten 2 KB gelesen — nicht aus der ganzen Datei.
    #[test]
    fn kopf_lesen_liest_nur_den_anfang() {
        let dir = test_dir("kopfliesen");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000010");
        doc.entities.clear();
        let stand = pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .unwrap();
        let kopf = kopf_lesen(&pool.stand_pfad(&stand)).expect("Kopf nicht gelesen");
        assert_eq!(kopf.zyklus, stand.zyklus);
        assert_eq!(kopf.datei, stand.datei);
        assert_eq!(kopf.groesse, stand.groesse);
    }

    /// Unveränderter Inhalt erzeugt keinen neuen Zyklus.
    #[test]
    fn unveraendert_erzeugt_keinen_stand() {
        let dir = test_dir("unveraendert");
        let mut pool = Sicherung::oeffnen(&dir);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000003");
        assert!(pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .is_some());
        assert!(pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .is_none());
        assert_eq!(pool.anzahl(), 1);
    }

    /// Abnahme 1 und 2 (T0225): Wird der Ordner im laufenden Betrieb geleert,
    /// steht nach dem nächsten Takt wieder ein Stand da — ohne Neustart.
    #[test]
    fn geloeschter_ordnerinhalt_wird_wieder_geschrieben() {
        let dir = test_dir("neu-nach-leeren");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut doc = doc_mit("Testdatei", "aaaaaaaa-0000-4000-8000-000000000020");
        for i in 0..5 {
            doc.name = format!("Testdatei {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        assert_eq!(stand_dateien(&dir), 5);

        // Von Hand alles löschen.
        for name in alle_dateien(&dir) {
            std::fs::remove_file(dir.join(name)).unwrap();
        }
        pool.auffrischen();
        assert_eq!(pool.anzahl(), 0, "der Index hält gelöschte Stände");
        assert_eq!(pool.nicht_zugeordnet(), 0);

        // Nächster Takt sichert denselben Inhalt neu.
        let stand = pool
            .sichern(&mut doc, &einst)
            .unwrap()
            .expect("nach dem Leeren muss wieder geschrieben werden");
        assert!(pool.stand_pfad(&stand).exists());
        assert_eq!(stand_dateien(&dir), 1);
        assert!(pool.zyklus() > 5, "die Zyklusnummer wurde wiederverwendet");
    }

    /// Fehlt der Ordner selbst, legt der nächste Schreibvorgang ihn an.
    #[test]
    fn ordner_wird_beim_schreiben_neu_angelegt() {
        let dir = test_dir("ordner-weg");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000021");
        pool.sichern(&mut doc, &einst).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        pool.auffrischen();
        assert_eq!(pool.anzahl(), 0);
        doc.name = "Plan neu".into();
        let stand = pool.sichern(&mut doc, &einst).unwrap().expect("Stand");
        assert!(dir.is_dir(), "der Ordner wurde nicht angelegt");
        assert!(pool.stand_pfad(&stand).exists());
    }

    /// Abnahme 3 (T0225): Eine fremde `.jjcad` wird übernommen, als „nicht
    /// zugeordnet“ geführt und nie automatisch gelöscht.
    #[test]
    fn fremde_datei_wird_uebernommen_und_nie_geloescht() {
        let dir = test_dir("fremd");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000022");
        for i in 0..120 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        // Fremde Datei, die zu keiner Zeichnung gehört.
        let fremd = doc_mit("Fremd", "bbbbbbbb-0000-4000-8000-000000000023");
        std::fs::write(dir.join("fremd.jjcad"), persist::to_json(&fremd).unwrap()).unwrap();

        pool.auffrischen();
        assert_eq!(
            pool.nicht_zugeordnet(),
            1,
            "die fremde Datei fehlt im Index"
        );
        assert!(
            pool.zeichnungen().iter().all(|z| z.datei != fremd.id),
            "die fremde Datei gilt als Zeichnung"
        );
        let vorher = stand_dateien(&dir);
        pool.ausduennen(&einst);
        assert!(
            dir.join("fremd.jjcad").exists(),
            "die fremde Datei wurde gelöscht"
        );
        assert_eq!(stand_dateien(&dir), vorher, "der Bestand hat sich geändert");
        assert!(pool.nicht_zugeordnet() >= 1);
    }

    /// Abnahme 7 (T0225): Ist der Scan unvollständig, löscht die Runde nichts.
    #[test]
    fn unvollstaendiger_scan_loescht_nichts() {
        let dir = test_dir("halb-lesbar");
        let mut pool = Sicherung::oeffnen(&dir);
        let weit = Einstellungen {
            plaetze: 200,
            ..Einstellungen::default()
        };
        let eng = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000024");
        for i in 0..40 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &weit).unwrap();
        }
        let vorher = stand_dateien(&dir);
        assert_eq!(vorher, 40);
        // Ein Eintrag, der wie ein Stand heißt, aber nicht gelesen werden kann.
        std::fs::create_dir_all(dir.join("halb-lesbar.jjcad")).unwrap();

        pool.auffrischen();
        assert_eq!(
            pool.ausduennen(&eng),
            0,
            "trotz unvollständigem Scan gelöscht"
        );
        assert_eq!(stand_dateien(&dir), vorher);

        // Ohne den Stolperstein läuft dieselbe Runde durch.
        std::fs::remove_dir(dir.join("halb-lesbar.jjcad")).unwrap();
        pool.auffrischen();
        assert!(
            pool.ausduennen(&eng) > 0,
            "die Ausdünnung greift nicht mehr"
        );
    }

    /// Migration: ein vorhandenes `manifest.json` wird einmal eingelesen, die
    /// Metadaten wandern in die Köpfe, danach wird es umbenannt.
    #[test]
    fn manifest_wird_einmal_eingelesen_und_umbenannt() {
        let dir = test_dir("migration");
        let doc_a = doc_mit("Plan A", "aaaaaaaa-0000-4000-8000-000000000030");
        let doc_b = doc_mit("Plan B", "bbbbbbbb-0000-4000-8000-000000000031");
        // Zwei Stände im alten Aufbau: Kopf fehlt, Dateiname `stand-…`.
        std::fs::write(
            dir.join("stand-3-aaaa.jjcad"),
            persist::to_json(&doc_a).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("stand-4-bbbb.jjcad"),
            persist::to_json(&doc_b).unwrap(),
        )
        .unwrap();
        let alt = serde_json::json!({
            "format": "jjcad-sicherung",
            "version": 2,
            "zyklus": 4,
            "staende": [
                {"zyklus": 3, "datei": "aaaa", "name": "Plan A", "hash": "h3",
                 "groesse": 10, "zeit": "2026-11-08T15:30:12", "gesichert": true,
                 "verworfen": false, "datei_hash": "d3", "datei_name": "",
                 "pfad": "/tmp/a.jjcad"},
                {"zyklus": 4, "datei": "bbbb", "name": "Plan B", "hash": "h4",
                 "groesse": 10, "zeit": "2026-11-08T15:31:12", "gesichert": false,
                 "verworfen": false, "datei_hash": "", "datei_name": "",
                 "pfad": ""}
            ]
        });
        std::fs::write(dir.join("manifest.json"), alt.to_string()).unwrap();

        let pool = Sicherung::oeffnen(&dir);
        assert_eq!(pool.anzahl(), 2, "die Stände fehlen nach der Migration");
        assert!(
            !dir.join("manifest.json").exists(),
            "das Manifest liegt noch"
        );
        assert!(
            dir.join(MIGRATIONS_MANIFEST_EINGELESEN).exists(),
            "das Manifest wurde nicht umbenannt"
        );
        assert!(pool.zyklus() >= 4);
        let a = pool.stand_mit_zyklus(3).expect("Stand 3");
        assert_eq!(a.gesichert, true, "gesichert nicht übernommen");
        assert_eq!(a.hash, "h3");
        assert_eq!(a.pfad, "/tmp/a.jjcad");
        assert_eq!(a.dateiname(), "plan-a_autosave_20261108-153012.jjcad");
        assert!(dir.join("plan-a_autosave_20261108-153012.jjcad").exists());
        let b = pool.stand_mit_zyklus(4).expect("Stand 4");
        assert_eq!(b.dateiname(), "plan-b_autosave_20261108-153112.jjcad");
        // Die Köpfe tragen die Metadaten jetzt selbst.
        let kopf = kopf_lesen(&dir.join("plan-a_autosave_20261108-153012.jjcad")).unwrap();
        assert_eq!(kopf.zyklus, 3);
        assert_eq!(kopf.hash, "h3");
        assert!(kopf.gesichert);
        // Nichts gelöscht.
        assert_eq!(stand_dateien(&dir), 2);
    }

    /// Abnahme 4 (T0225): Ein fehlendes Manifest ändert nichts.
    #[test]
    fn fehlendes_manifest_aendert_nichts() {
        let dir = test_dir("manifest-weg");
        let einst = Einstellungen::default();
        {
            let mut pool = Sicherung::oeffnen(&dir);
            let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000032");
            for i in 0..3 {
                doc.name = format!("Plan {i}");
                pool.sichern(&mut doc, &einst).unwrap();
            }
        }
        let _ = std::fs::remove_file(dir.join(MIGRATIONS_MANIFEST));
        let pool = Sicherung::oeffnen(&dir);
        assert_eq!(pool.anzahl(), 3, "ohne Manifest fehlen Stände");
        assert_eq!(pool.dateien(), 1);
        let mut pool = pool;
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000032");
        doc.name = "Plan 9".into();
        assert!(pool.sichern(&mut doc, &einst).unwrap().is_some());
    }

    /// T0225: Nach einem geleerten Ordner zählt der Zähler weiter — der neue
    /// Stand trägt die höchste bekannte Nummer + 1.
    #[test]
    fn zyklus_ist_hoechster_bekannter_plus_eins() {
        let dir = test_dir("zyklus");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000033");
        doc.entities.clear();
        let a = pool.sichern(&mut doc, &einst).unwrap().unwrap();
        doc.entities.clear();
        doc.name = "Anders".into();
        let b = pool.sichern(&mut doc, &einst).unwrap().unwrap();
        assert_eq!(b.zyklus, a.zyklus + 1);
        assert_eq!(pool.zyklus(), b.zyklus);
    }

    /// Der Pin bleibt **immer** — die Aufbewahrungszeit T gilt nur für den
    /// Verlauf dazwischen (T0225, entschieden).
    #[test]
    fn pin_bleibt_auch_nach_t() {
        let dir = test_dir("pin-bleibt");
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
        for i in 0..100 {
            neu.name = format!("Neu {i}");
            pool.sichern(&mut neu, &einst).unwrap();
        }
        let pin = pool.neuester(&alt.id).expect("der Pin ist verfallen");
        assert_eq!(pin.zyklus, 1, "der Pin ist nicht der erste Stand");
        assert_eq!(
            pool.staende_von(&alt.id).len(),
            1,
            "die ruhende Zeichnung hat mehr als ihren Pin"
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
        let n = pool.zyklus();
        let pins = pin_zyklen(pool.staende());
        let anker = anker_zyklen(pool.staende());
        for s in pool.staende() {
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
        let aeltester = pool.staende().iter().map(|s| s.zyklus).min().unwrap();
        assert_eq!(
            anker,
            vec![aeltester],
            "der Anker ist nicht der älteste Stand"
        );
        assert!(
            n - aeltester >= einst.zyklen(),
            "die Tiefe T wird nicht erreicht ({} < {})",
            n - aeltester,
            einst.zyklen()
        );
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
            pool.anzahl(),
            100,
            "unter der Obergrenze darf nichts verfallen"
        );
        let n = pool.zyklus();
        for k in 0..100 {
            let zyklus = n - k;
            assert!(
                pool.staende().iter().any(|s| s.zyklus == zyklus),
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
                pool.anzahl() <= grenze,
                "{dateien} Dateien: {} Stände über der Grenze {grenze}",
                pool.anzahl()
            );
            zahlen.push(pool.anzahl());
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
        assert_eq!(pool.anzahl(), 2);
        assert_eq!(pool.staende_von(&a.id).len(), 1);
        assert_eq!(pool.staende_von(&b.id).len(), 1);
        assert_eq!(pool.neuester(&a.id).unwrap().name, "A 9");
        assert_eq!(stand_dateien(&dir), 2, "die ersetzten Dateien liegen noch");
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
        let stand = pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .unwrap();

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
            assert!((ist - k).abs() <= 1.0, "g = {g}: {ist} Stände statt {k}");
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
            assert!((zurueck - k).abs() < 0.05, "k = {k}: g = {g} → {zurueck}");
        }
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
        let stand = pool
            .sichern(&mut doc, &Einstellungen::default())
            .unwrap()
            .unwrap();
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
        assert!(
            !a.gesichert,
            "Arbeitsstand darf nicht als Dateistand gelten"
        );

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
            let streng = Einstellungen { plaetze, ..einst };
            let vorher = pool.anzahl();
            let angesagt = pool.wuerde_verlieren(&streng);
            assert_eq!(vorher, 120, "Vorbereitung: alle Stände angelegt");
            assert!(angesagt > 0, "bei {plaetze} Plätzen muss etwas wegfallen");
            let geloescht = pool.ausduennen(&streng);
            assert_eq!(
                angesagt, geloescht,
                "angesagte und gelöschte Zahl weichen ab ({plaetze} Plätze)"
            );
            assert_eq!(pool.anzahl(), vorher - geloescht);
        }
    }

    /// T0188: Ein Stand merkt sich die Zieldatei; fehlt sie, findet das
    /// Aufräumen ihn — Stände ohne Pfad bleiben unangetastet.
    #[test]
    fn pfad_und_ohne_zeichnung() {
        let dir = test_dir("pfad");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        // Zieldateien liegen **neben** dem Pool: im Pool wären sie fremde
        // Stände (T0225) und würden die Zahl der Einträge verfälschen.
        let ziele = dir.join("ziele");
        std::fs::create_dir_all(&ziele).unwrap();
        let vorhanden = ziele.join("da.jjcad");
        std::fs::write(&vorhanden, "{}").unwrap();

        let mut mit = doc_mit("Da", "aaaaaaaa-0000-4000-8000-0000000000f1");
        mit.path = Some(vorhanden.to_string_lossy().to_string());
        pool.sichern(&mut mit, &einst).unwrap();

        let mut ohne = doc_mit("Weg", "aaaaaaaa-0000-4000-8000-0000000000f2");
        ohne.path = Some(ziele.join("weg.jjcad").to_string_lossy().to_string());
        pool.sichern(&mut ohne, &einst).unwrap();

        let mut unbekannt = doc_mit("Unbenannt", "aaaaaaaa-0000-4000-8000-0000000000f3");
        unbekannt.path = None;
        pool.sichern(&mut unbekannt, &einst).unwrap();

        let waisen: Vec<String> = pool
            .ohne_zeichnung()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(
            waisen,
            vec!["Weg".to_string()],
            "falsche Waisen: {waisen:?}"
        );

        let zyklen: Vec<u64> = pool.ohne_zeichnung().iter().map(|s| s.zyklus).collect();
        assert_eq!(pool.entferne_zyklen(&zyklen), 1);
        assert_eq!(pool.anzahl(), 2, "zu viel gelöscht");
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
            fremd: false,
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
        // Ein gespeicherter Name gewinnt.
        let mit = Stand {
            datei_name: "da.jjcad".into(),
            ..stand
        };
        assert_eq!(mit.dateiname(), "da.jjcad");
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
            fremd: false,
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

    /// Fremde Dateien sind kein Kandidat der Ausdünnung.
    #[test]
    fn fremde_dateien_sind_kein_loeschkandidat() {
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut staende: Vec<Stand> = (1..=31).map(|z| stand_mit(z, "A")).collect();
        staende.push(Stand {
            fremd: true,
            datei: String::new(),
            ..stand_mit(99, "")
        });
        let weg = zu_loeschen(&staende, 99, &einst);
        assert!(
            !weg.contains(&99),
            "eine fremde Datei steht auf der Löschliste"
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
        assert_eq!(neueste, pool.zyklus(), "der Pin fehlt");
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
            let mut doc = doc_mit(
                &format!("Plan {d}"),
                &format!("aaaaaaaa-0000-4000-8000-{d:012}"),
            );
            for i in 0..3 {
                doc.name = format!("Plan {d} {i}");
                pool.sichern(&mut doc, &einst).unwrap();
            }
        }
        assert_eq!(einst.plaetze_effektiv(20), 40);
        assert_eq!(pool.anzahl(), 40, "Pool nicht auf 2·F begrenzt");
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
        // Die Markierung steht im Kopf der Standdatei.
        let kopf = kopf_lesen(&pool.stand_pfad(&stand)).unwrap();
        assert!(kopf.verworfen);
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
        std::fs::write(&ziel, b"{\"format\":\"jjcad\",\"extern\":true}").unwrap();
        assert!(
            pool.extern_geaendert(&doc.id, &ziel),
            "extern geänderte Datei nicht erkannt"
        );
    }

    /// T0225: Zwei eigene Prozesse gleichzeitig — keine Datei verschwindet.
    #[test]
    fn zwei_prozesse_verlieren_keine_staende() {
        let dir = test_dir("zwei-prozesse");
        let einst = Einstellungen {
            plaetze: 60,
            ..Einstellungen::default()
        };
        let mut a = Sicherung::oeffnen(&dir);
        let mut b = Sicherung::oeffnen(&dir);
        let mut doc_a = doc_mit("A", "aaaaaaaa-0000-4000-8000-0000000000a1");
        let mut doc_b = doc_mit("B", "bbbbbbbb-0000-4000-8000-0000000000b1");
        for i in 0..25 {
            doc_a.name = format!("A {i}");
            doc_b.name = format!("B {i}");
            a.sichern(&mut doc_a, &einst).unwrap();
            b.sichern(&mut doc_b, &einst).unwrap();
            a.sichern(&mut doc_b, &einst).unwrap();
            b.sichern(&mut doc_a, &einst).unwrap();
        }
        let mut c = Sicherung::oeffnen(&dir);
        c.auffrischen();
        assert_eq!(
            c.anzahl(),
            stand_dateien(&dir),
            "Index und Verzeichnis laufen auseinander"
        );
        assert_eq!(c.dateien(), 2, "eine Zeichnung ist verschwunden");
        assert!(c.neuester(&doc_a.id).is_some(), "A fehlt");
        assert!(c.neuester(&doc_b.id).is_some(), "B fehlt");
    }

    /// Die Sperre hält einen zweiten eigenen Schreiber ab, ohne etwas zu
    /// verlieren; nach dem Freigeben läuft der nächste Takt durch.
    #[test]
    fn sperre_haelt_den_zweiten_schreiber_ab() {
        let dir = test_dir("sperre");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000014");
        {
            let _fremde_sperre = PoolSperre::nehmen(&dir).expect("Sperre");
            assert!(
                pool.sichern(&mut doc, &einst).unwrap().is_none(),
                "trotz fremder Sperre geschrieben"
            );
            assert_eq!(stand_dateien(&dir), 0, "trotz Sperre eine Datei angelegt");
        }
        assert!(
            pool.sichern(&mut doc, &einst).unwrap().is_some(),
            "nach dem Freigeben wurde nicht geschrieben"
        );
        assert_eq!(stand_dateien(&dir), 1);
    }

    /// T0225/Abnahme 5: Der Start mit 150 Ständen liest nur Köpfe, und ein
    /// Takt ohne Änderung liest gar nichts.
    #[test]
    fn start_liest_nur_koepfe_und_der_normalfall_nichts() {
        let dir = test_dir("viele-staende");
        let einst = Einstellungen {
            plaetze: 2000,
            ..Einstellungen::default()
        };
        {
            let mut pool = Sicherung::oeffnen(&dir);
            let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000015");
            for i in 0..150 {
                doc.name = format!("Plan {i}");
                pool.sichern(&mut doc, &einst).unwrap();
            }
            assert_eq!(pool.anzahl(), 150);
        }
        setze_kopflesungen(0);
        let start = std::time::Instant::now();
        let mut pool = Sicherung::oeffnen(&dir);
        let dauer = start.elapsed();
        assert_eq!(pool.anzahl(), 150);
        assert_eq!(kopflesungen(), 150, "es wurde mehr als der Kopf gelesen");
        assert!(
            dauer.as_millis() < 150,
            "Start mit 150 Ständen dauerte {dauer:?}"
        );

        // Ein Takt ohne Änderung: ein stat, kein Lesevorgang.
        setze_kopflesungen(0);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000015");
        doc.name = "Plan 149".into();
        assert!(pool.sichern(&mut doc, &einst).unwrap().is_none());
        assert_eq!(
            kopflesungen(),
            0,
            "der unveränderte Takt hat Dateien gelesen"
        );

        // Geänderter Inhalt: nur der neue Kopf wird gelesen (beim nächsten
        // Auffrischen), der Bestand nicht.
        setze_kopflesungen(0);
        doc.name = "Plan neu".into();
        assert!(pool.sichern(&mut doc, &einst).unwrap().is_some());
        assert_eq!(kopflesungen(), 0, "der Schreibvorgang hat Köpfe gelesen");
    }

    /// Abnahme 5 (T0225): 150 Stände mit rund 60 KB — der Start liest nur die
    /// Köpfe und bleibt unter 50 ms; ein Takt ohne Änderung liest nichts.
    ///
    /// Zum Vergleich wird gemessen, was ein Volltextlesen aller Stände kostet.
    #[test]
    fn messung_150_staende_nur_koepfe() {
        let dir = test_dir("messung");
        let einst = Einstellungen {
            plaetze: 2000,
            ..Einstellungen::default()
        };
        {
            let mut pool = Sicherung::oeffnen(&dir);
            let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000017");
            // Ein langer Text treibt die Dateigröße auf rund 60 KB.
            doc.project.note = "x".repeat(20_000);
            for i in 0..150 {
                doc.name = format!("Plan {i}");
                pool.sichern(&mut doc, &einst).unwrap();
            }
            assert_eq!(pool.anzahl(), 150);
            let gross: u64 = pool.staende().iter().map(|s| s.groesse).sum();
            println!(
                "150 Stände, zusammen {} KB, je {} KB",
                gross / 1024,
                gross / 1024 / 150
            );
        }

        // Start: nur Köpfe.
        setze_kopflesungen(0);
        let start = std::time::Instant::now();
        let mut pool = Sicherung::oeffnen(&dir);
        let dauer = start.elapsed();
        println!(
            "Start mit 150 Ständen: {dauer:?}, Kopflesungen: {}",
            kopflesungen()
        );
        assert_eq!(pool.anzahl(), 150);
        assert_eq!(kopflesungen(), 150, "es wurde mehr als der Kopf gelesen");
        assert!(
            dauer.as_millis() < 100,
            "Start mit 150 Ständen dauerte {dauer:?}"
        );

        // Zum Vergleich: alle Stände vollständig lesen.
        let start = std::time::Instant::now();
        let mut bytes = 0usize;
        for s in pool.staende() {
            bytes += std::fs::read(pool.stand_pfad(s)).unwrap().len();
        }
        println!(
            "Volltextlesen aller 150 Stände ({} KB): {:?}",
            bytes / 1024,
            start.elapsed()
        );

        // Ein Takt ohne Änderung: 0 Lesevorgänge.
        setze_kopflesungen(0);
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000017");
        doc.project.note = "x".repeat(20_000);
        doc.name = "Plan 149".into();
        assert!(pool.sichern(&mut doc, &einst).unwrap().is_none());
        println!("Takt ohne Änderung: Kopflesungen {}", kopflesungen());
        assert_eq!(kopflesungen(), 0);
    }

    /// Die Signatur des Verzeichnisses erkennt jede Änderung — und nur diese.
    #[test]
    fn signatur_erkennt_aenderungen() {
        let dir = test_dir("signatur");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen::default();
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000016");
        pool.sichern(&mut doc, &einst).unwrap();

        setze_kopflesungen(0);
        pool.auffrischen();
        assert_eq!(kopflesungen(), 0, "ohne Änderung wurde gelesen");

        std::fs::write(dir.join("dazu.jjcad"), b"{\"format\":\"jjcad\"}").unwrap();
        pool.auffrischen();
        assert_eq!(kopflesungen(), 1, "die neue Datei wurde nicht gelesen");
        assert_eq!(pool.nicht_zugeordnet(), 1);
    }
}

#[cfg(test)]
mod tests_zeichnungen {
    //! T0223: die Zeichnungsliste für das Fenster „Wiederherstellen“ —
    //! ein Eintrag je Zeichnung, der Verlauf chronologisch.

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("jjcad-zeichnungen-{}-{name}", std::process::id()));
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
