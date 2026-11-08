//! Sicherung: Stände unabhängig vom Speichern (T0129).
//!
//! Die Sicherung legt während der Arbeit Stände im **Pool** ab und dünnt sie
//! nach Alter aus. Sie schreibt **nie** in die Zieldatei — gespeichert wird
//! ausschließlich vom Benutzer. Vorgabe und Begründung: `docs/SICHERUNG.md`.
//!
//! Aufbau des Pools (ein Pool für alle Dateien, ein Projektordner):
//!
//! ```text
//! <Sicherungen>/manifest.json            Zähler n und die Liste der Stände
//! <Sicherungen>/stand-<n>-<uuid>.jjcad   eine Datei je Stand
//! ```
//!
//! Der **Pin** (neuester Stand je Dokument) ist abgeleitet, nicht gespeichert:
//! so kann er nicht veralten. Die **Dichtezone** sind die letzten
//! [`DICHTEZONE`] Zyklen; sie werden nie ausgedünnt.

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
/// Fassung des Manifests.
pub const MANIFEST_VERSION: u32 = 1;
/// Dichtezone in Zyklen: die letzten `D` Stände liegen lückenlos vor.
pub const DICHTEZONE: u64 = 10;
/// Grenzen der Plätze im Pool (Einstellung „Plätze im Pool“).
pub const PLAETZE_MIN: u32 = 30;
pub const PLAETZE_MAX: u32 = 2000;
pub const PLAETZE_VORGABE: u32 = 100;
/// Vorgabe für „Reicht zurück“ in Arbeitsstunden (Quartal).
pub const STUNDEN_VORGABE: u32 = 480;
/// Die vier Wahlen für „Reicht zurück“ in Arbeitsstunden.
pub const STUNDEN_WAHL: [u32; 4] = [40, 160, 480, 1900];
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
    /// Dateiname im Pool (ohne Verzeichnis). Leer = aus `name` und `zeit`
    /// ableiten — so heißen Stände aus älteren Pools noch.
    #[serde(default)]
    pub datei_name: String,
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

    /// Obergrenze des Pools: `max(N, Anzahl Dateien)`.
    pub fn plaetze_effektiv(&self, dateien: usize) -> usize {
        (self.plaetze as usize).max(dateien)
    }

    /// Abstand der Stände als relative Lücke (Formel §3 der Vorgabe).
    ///
    /// `1 − (D / T) ^ (1 / (Plätze − Dateien))`. Ohne Platz für Verlauf
    /// (Plätze ≤ Dateien oder `T ≤ D`) ist der Abstand 1 (kein Verlauf).
    pub fn abstand(&self, dateien: usize) -> f64 {
        let t = self.zyklen() as f64;
        let d = DICHTEZONE as f64;
        let plaetze = self.plaetze_effektiv(dateien) as f64;
        let frei = plaetze - dateien as f64;
        if frei < 1.0 || t <= d {
            return 1.0;
        }
        1.0 - (d / t).powf(1.0 / frei)
    }
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
        self.ablegen(doc, einst, false, false)
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
        self.ablegen(doc, einst, true, true)
    }

    fn ablegen(
        &mut self,
        doc: &mut Document,
        einst: &Einstellungen,
        erzwingen: bool,
        gesichert: bool,
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
        // nicht gelöscht — die Einstellung verhindert nur neue.
        if !einst.verlauf {
            if let Some(alt) = self.neuester(&doc.id).cloned() {
                let _ = std::fs::remove_file(self.stand_pfad(&alt));
                self.manifest.staende.retain(|s| s.zyklus != alt.zyklus);
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
            datei_name: String::new(),
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
    pub fn ausduennen(&mut self, einst: &Einstellungen) {
        let t = einst.zyklen();
        let n = self.manifest.zyklus;
        let pins = self.pin_zyklen();

        // 1. Fenster: nichts außer Pins älter als T.
        let mut entfernt: Vec<Stand> = Vec::new();
        let mut behalten: Vec<Stand> = Vec::with_capacity(self.manifest.staende.len());
        for s in self.manifest.staende.drain(..) {
            let alter = n.saturating_sub(s.zyklus);
            if pins.contains(&s.zyklus) || alter <= t {
                behalten.push(s);
            } else {
                entfernt.push(s);
            }
        }
        self.manifest.staende = behalten;

        // 2. Verlauf aus: nichts löschen. Die Einstellung verhindert nur neue
        //    Stände (§10.5) — vorhandene bleiben, bis Fenster und Obergrenze
        //    sie ausdünnen. Ein Haken darf keine Sicherungen vernichten.

        // 3. Obergrenze: ausdünnen, solange der Pool voll ist.
        let plaetze = einst.plaetze_effektiv(self.dateien());
        while self.manifest.staende.len() > plaetze {
            let Some(pos) = self.kuerzester_kandidat(&pins, t) else {
                break;
            };
            entfernt.push(self.manifest.staende.remove(pos));
        }

        for s in &entfernt {
            let _ = std::fs::remove_file(self.stand_pfad(s));
        }
        if !entfernt.is_empty() {
            let _ = self.manifest_schreiben();
        }
    }

    /// Zyklusnummern der Pins: je Dokument der neueste Stand.
    fn pin_zyklen(&self) -> Vec<u64> {
        let mut neueste: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
        for s in &self.manifest.staende {
            let e = neueste.entry(s.datei.as_str()).or_insert(s.zyklus);
            if s.zyklus > *e {
                *e = s.zyklus;
            }
        }
        neueste.into_values().collect()
    }

    /// Kandidat mit den kleinsten Kosten (Regel 10.4). `None`, wenn kein Stand
    /// ausgedünnt werden darf (nur Pins und Dichtezone übrig).
    fn kuerzester_kandidat(&self, pins: &[u64], t: u64) -> Option<usize> {
        let n = self.manifest.zyklus;
        // Nach Alter aufsteigend: Index 0 = neuester Stand.
        let mut liste: Vec<(usize, u64)> = self
            .manifest
            .staende
            .iter()
            .enumerate()
            .map(|(i, s)| (i, n.saturating_sub(s.zyklus)))
            .collect();
        liste.sort_by_key(|(_, alter)| *alter);
        let alter: Vec<u64> = liste.iter().map(|(_, a)| *a).collect();
        let mut bestes: Option<(usize, f64)> = None;
        for (pos, (idx, a)) in liste.iter().enumerate() {
            if *a < DICHTEZONE || pins.contains(&self.manifest.staende[*idx].zyklus) {
                continue;
            }
            let vorher = if pos == 0 { 0 } else { alter[pos - 1] };
            let nachher = if pos + 1 >= alter.len() { t } else { alter[pos + 1] };
            if nachher == 0 {
                continue;
            }
            let kosten = (nachher as f64 - vorher as f64) / nachher as f64;
            if bestes.map(|(_, k)| kosten < k).unwrap_or(true) {
                bestes = Some((*idx, kosten));
            }
        }
        bestes.map(|(i, _)| i)
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
    std::fs::rename(&tmp, ziel).map_err(|e| {
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

    /// §13.3: Jede Datei behält ihren Pin, auch nach dem Dreifachen von T.
    #[test]
    fn jede_datei_behaelt_einen_stand() {
        let dir = test_dir("pin");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            stunden: 1,
            intervall_min: 1,
            ..Einstellungen::default()
        };
        let mut alt = doc_mit("Alt", "aaaaaaaa-0000-4000-8000-000000000004");
        pool.sichern(&mut alt, &einst).unwrap();
        let mut neu = doc_mit("Neu", "aaaaaaaa-0000-4000-8000-000000000005");
        for i in 0..200 {
            neu.name = format!("Neu {i}");
            pool.sichern(&mut neu, &einst).unwrap();
        }
        assert!(
            pool.neuester(&alt.id).is_some(),
            "der Pin der ruhenden Datei ist verschwunden"
        );
    }

    /// §13.4: Kein Stand außer Pins ist älter als T.
    #[test]
    fn kein_stand_ausser_pins_aelter_als_t() {
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
        let pins = pool.pin_zyklen();
        for s in pool.manifest().staende.iter() {
            if pins.contains(&s.zyklus) {
                continue;
            }
            assert!(
                n - s.zyklus <= einst.zyklen(),
                "Stand {} ist älter als T ({})",
                s.zyklus,
                einst.zyklen()
            );
        }
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

    /// §13.6: Die letzten D Zyklen liegen lückenlos vor.
    #[test]
    fn dichtezone_ist_lueckenlos() {
        let dir = test_dir("dichtezone");
        let mut pool = Sicherung::oeffnen(&dir);
        let einst = Einstellungen {
            plaetze: 30,
            ..Einstellungen::default()
        };
        let mut doc = doc_mit("Plan", "aaaaaaaa-0000-4000-8000-000000000008");
        for i in 0..100 {
            doc.name = format!("Plan {i}");
            pool.sichern(&mut doc, &einst).unwrap();
        }
        let n = pool.manifest().zyklus;
        for k in 0..DICHTEZONE {
            let zyklus = n - k;
            assert!(
                pool.manifest().staende.iter().any(|s| s.zyklus == zyklus),
                "Zyklus {zyklus} fehlt in der Dichtezone"
            );
        }
    }

    /// §13.7: Die Zahl der Stände hängt nicht an der Zahl der Dateien.
    #[test]
    fn pool_groesse_haengt_nicht_an_der_dateizahl() {
        let einst = Einstellungen {
            plaetze: 60,
            ..Einstellungen::default()
        };
        let mut zahlen = Vec::new();
        // So viele Stände je Datei, dass der Pool in jedem Fall voll ist
        // (5 × 12 = 60, 20 × 3 = 60, 50 × 2 = 100 → 60).
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
            zahlen.push(pool.manifest().staende.len());
        }
        assert_eq!(zahlen[0], zahlen[1], "5 gegen 20 Dateien: {zahlen:?}");
        assert_eq!(zahlen[1], zahlen[2], "20 gegen 50 Dateien: {zahlen:?}");
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

    /// Die Abstandsformel der Vorgabe: 10 Dateien, 100 Plätze, Quartal → 8,5 %.
    #[test]
    fn abstand_folgt_der_formel() {
        let einst = Einstellungen::default();
        assert_eq!(einst.zyklen(), 28_800);
        let g = einst.abstand(10);
        assert!((g - 0.085).abs() < 0.001, "Abstand war {g}");
        // Intervall 5 min: gleiche Reichweite, ein Fünftel der Zyklen.
        let langsam = Einstellungen {
            intervall_min: 5,
            ..Einstellungen::default()
        };
        assert_eq!(langsam.zyklen(), 5_760);
        // Ohne Platz für Verlauf: kein Verlauf.
        assert_eq!(einst.abstand(100), 1.0);
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
            datei_name: String::new(),
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
            datei_name: String::new(),
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
