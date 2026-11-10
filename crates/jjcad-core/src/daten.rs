//! Speicherorte aller Nutzdaten (T0127).
//!
//! Alle Nutzdaten liegen in **einem** Wurzelverzeichnis: `prefs.json`,
//! `sitzung.json`, `blocks/`, `sicherungen/`, `log/`. Die Zeichnungen selbst
//! bleiben, wo der Benutzer sie ablegt — das Programm legt dort nichts an.
//!
//! Die Wurzel wird bestimmt durch, erste Übereinstimmung gewinnt:
//!
//! 1. `--daten-dir <pfad>` (Kommandozeile),
//! 2. `$JJCAD_DATA_DIR`,
//! 3. `portable.txt` neben der Programmdatei → `<exe>/data`,
//! 4. Plattformstandard: Linux `$XDG_DATA_HOME/JoJa-CAD` (sonst
//!    `~/.local/share/JoJa-CAD`), Windows `%APPDATA%\JoJa-CAD`, macOS
//!    `~/Library/Application Support/JoJa-CAD`.
//!
//! Die Unterordner `blocks/`, `sicherungen/` und `log/` sind in den
//! Einstellungen änderbar — relativ zur Wurzel oder absolut
//! (`UserPrefs::blocks_pfad` & Co.).

use crate::model::BlockDef;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Umgebungsvariable, die die Wurzel setzt.
pub const WURZEL_UMGEBUNG: &str = "JJCAD_DATA_DIR";
/// Schalterdatei neben der Programmdatei — ihr Vorhandensein macht die
/// Installation portabel (`<exe>/data`).
pub const PORTABLE_DATEI: &str = "portable.txt";
/// Einstellungen in der Wurzel (nicht änderbar).
pub const PREFS_DATEI: &str = "prefs.json";
/// Sitzung in der Wurzel (nicht änderbar) — Reiter, Ansichten und Fensterlage
/// für den nächsten Start, siehe [`crate::sitzung`].
pub const SITZUNG_DATEI: &str = "sitzung.json";
/// Sperrdatei der laufenden Instanz (PID).
pub const SPERRE_DATEI: &str = "sperre.lock";

/// Unterordner der Wurzel, die das Programm anlegt.
pub const UNTERORDNER: [&str; 3] = ["blocks", "sicherungen", "log"];

static WURZEL: OnceLock<PathBuf> = OnceLock::new();

/// Plattformstandard, wenn weder Argument noch Umgebung noch `portable.txt`
/// etwas sagen.
pub fn plattform_standard() -> PathBuf {
    if cfg!(windows) {
        let basis = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| heim().join("AppData").join("Roaming"));
        return basis.join("JoJa-CAD");
    }
    if cfg!(target_os = "macos") {
        return heim()
            .join("Library")
            .join("Application Support")
            .join("JoJa-CAD");
    }
    // Linux und alles Übrige: XDG-Datenverzeichnis.
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("JoJa-CAD");
        }
    }
    heim().join(".local").join("share").join("JoJa-CAD")
}

fn heim() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Wurzel aus den vier Quellen — die erste gesetzte gewinnt. Reine Funktion,
/// damit die Reihenfolge ohne Umgebungsvariablen prüfbar ist.
pub fn wurzel_ermitteln(
    argument: Option<&Path>,
    umgebung: Option<&Path>,
    exe_dir: &Path,
    portable: bool,
) -> PathBuf {
    if let Some(p) = argument.filter(|p| !p.as_os_str().is_empty()) {
        return p.to_path_buf();
    }
    if let Some(p) = umgebung.filter(|p| !p.as_os_str().is_empty()) {
        return p.to_path_buf();
    }
    if portable {
        return exe_dir.join("data");
    }
    plattform_standard()
}

/// Läuft diese Fassung portabel? Dann liegt `portable.txt` neben der
/// Programmdatei, und alle Nutzdaten gehören nach `<exe>/data`.
///
/// Das ist die **einzige** Unterscheidung zwischen portabel und installiert —
/// es gibt keine Kennung im Programm und keinen Schalter in den Einstellungen.
/// Das portable Paket muss die Datei also mitliefern; fehlt sie, landet alles
/// im Plattformstandard (`%APPDATA%\JoJa-CAD` bzw. `~/.local/share/JoJa-CAD`).
pub fn ist_portabel() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(PORTABLE_DATEI).is_file()))
        .unwrap_or(false)
}

/// Wurzel aus der laufenden Umgebung (Kommandozeile, `$JJCAD_DATA_DIR`,
/// `portable.txt` neben der Programmdatei, Plattformstandard).
pub fn wurzel_ermitteln_aus_umgebung(argument: Option<&Path>) -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let umgebung = std::env::var_os(WURZEL_UMGEBUNG)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    match exe_dir {
        Some(dir) => {
            let portable = dir.join(PORTABLE_DATEI).is_file();
            wurzel_ermitteln(argument, umgebung.as_deref(), &dir, portable)
        }
        // Ohne Programmdatei (sollte nicht vorkommen) bleiben Argument,
        // Umgebung und Plattformstandard.
        None => wurzel_ermitteln(
            argument,
            umgebung.as_deref(),
            Path::new("."),
            false,
        ),
    }
}

/// Setzt die Wurzel für diesen Prozess. Ein zweiter Aufruf ändert nichts —
/// die Wurzel steht für die Laufzeit fest (ein Wechsel wäre ein Neustart,
/// siehe `--daten-dir`).
pub fn setze_wurzel(pfad: impl Into<PathBuf>) {
    let _ = WURZEL.set(pfad.into());
}

/// Die Wurzel dieses Prozesses; ohne ausdrücklichen Aufruf aus der Umgebung.
pub fn wurzel() -> PathBuf {
    WURZEL
        .get_or_init(|| wurzel_ermitteln_aus_umgebung(None))
        .clone()
}

/// `prefs.json` in der Wurzel — nicht änderbar.
pub fn prefs_datei() -> PathBuf {
    wurzel().join(PREFS_DATEI)
}

/// `sitzung.json` in der Wurzel — nicht änderbar.
pub fn sitzung_datei() -> PathBuf {
    wurzel().join(SITZUNG_DATEI)
}

/// Sperrdatei der laufenden Instanz.
pub fn sperre_datei() -> PathBuf {
    wurzel().join(SPERRE_DATEI)
}

/// Löst eine Pfadangabe aus den Einstellungen auf: leer = Vorgabe in der
/// Wurzel, relativ = an der Wurzel, absolut = wie angegeben. Ein führendes
/// `~/` wird zum Benutzerverzeichnis.
pub fn pfad_in_wurzel(angabe: &str, vorgabe: &str) -> PathBuf {
    pfad_in_wurzel_basis(&wurzel(), angabe, vorgabe)
}

/// Wie [`pfad_in_wurzel`], aber mit ausdrücklicher Wurzel (Tests).
pub fn pfad_in_wurzel_basis(w: &Path, angabe: &str, vorgabe: &str) -> PathBuf {
    let text = angabe.trim();
    if text.is_empty() {
        return w.join(vorgabe);
    }
    if text == "~" {
        return heim();
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return heim().join(rest);
    }
    let p = PathBuf::from(text);
    if p.is_absolute() {
        p
    } else {
        w.join(p)
    }
}

/// Anzeigetext eines aufgelösten Pfads (für das Einstellungsfenster).
pub fn pfad_text(p: &Path) -> String {
    p.display().to_string()
}

/// Stabile Kennung für eine Zeichnung, die im Dateikopf keine `id` trägt.
///
/// Aus dem kanonischen Pfad abgeleitet (UUID-Form, Version 5), damit **dieselbe**
/// Datei über Sitzungen hinweg dieselbe Kennung behält. Eine zufällige Kennung je
/// Öffnen würde dem Sicherungspool bei jedem Start einen neuen Pin bescheren und
/// den Verlauf einer Zeichnung in mehrere Dokumente zerfallen lassen.
///
/// Unter Windows werden Groß- und Kleinschreibung vereinheitlicht — dort ist
/// `C:\Bau\Haus.jjcad` dieselbe Datei wie `c:\bau\haus.jjcad`.
pub fn kennung_fuer_pfad(pfad: &Path) -> String {
    let echt = std::fs::canonicalize(pfad).unwrap_or_else(|_| pfad.to_path_buf());
    let mut text = echt.to_string_lossy().to_string();
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    // Version 5: aus einem Namen abgeleitet, nicht zufällig.
    b[6] = (b[6] & 0x0f) | 0x50;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Schreibprobe: legt das Verzeichnis an (falls nötig) und schreibt eine
/// Probedatei hinein. Nicht beschreibbar → `Err` mit lesbarem Grund; ein
/// angebotener Pfad wird dann **nicht** übernommen.
pub fn pruefe_verzeichnis(pfad: &Path) -> Result<(), String> {
    if pfad.as_os_str().is_empty() {
        return Err("leerer Pfad".into());
    }
    std::fs::create_dir_all(pfad)
        .map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let probe = pfad.join(format!(".schreibprobe-{}", std::process::id()));
    std::fs::write(&probe, b"JoJa-CAD")
        .map_err(|e| format!("nicht beschreibbar: {e}"))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Legt die Wurzel samt Unterordnern an und übernimmt beim ersten Start die
/// alte `prefs.json` aus `~/.config/jjcad` (bzw. `%APPDATA%\JoJa-CAD`).
/// Die alte Datei bleibt liegen.
pub fn vorbereiten() -> Result<(), String> {
    let w = wurzel();
    if ist_portabel() {
        // Portabel heißt eigenständig: ein USB-Stick übernimmt nicht die
        // Einstellungen des Rechners, an dem er gerade steckt. Als Quelle wird
        // die Wurzel selbst genannt — dann findet keine Übernahme statt.
        return vorbereiten_in(&w, &w);
    }
    vorbereiten_in(&w, &crate::persist::config_dir())
}

/// Wie [`vorbereiten`], aber mit ausdrücklicher Wurzel und ausdrücklichem
/// altem Konfigurationsverzeichnis (Tests).
pub fn vorbereiten_in(w: &Path, altes_config: &Path) -> Result<(), String> {
    std::fs::create_dir_all(w)
        .map_err(|e| format!("Datenordner {} nicht anlegbar: {e}", w.display()))?;
    for u in UNTERORDNER {
        let d = w.join(u);
        std::fs::create_dir_all(&d).map_err(|e| format!("{} nicht anlegbar: {e}", d.display()))?;
    }
    prefs_uebernehmen_aus(w, altes_config);
    Ok(())
}

/// Kopiert die Einstellungen vom alten Ort in die Wurzel, wenn dort noch keine
/// liegen. Der alte Ort ist das frühere Konfigurationsverzeichnis
/// (`crate::persist::config_dir()`); die alte Datei wird nicht angefasst.
pub fn prefs_uebernehmen() -> bool {
    prefs_uebernehmen_aus(&wurzel(), &crate::persist::config_dir())
}

/// Wie [`prefs_uebernehmen`], aber mit ausdrücklicher Wurzel und Quelle.
pub fn prefs_uebernehmen_aus(w: &Path, altes_config: &Path) -> bool {
    let ziel = w.join(PREFS_DATEI);
    if ziel.exists() {
        return false;
    }
    let alt = altes_config.join(PREFS_DATEI);
    if !alt.is_file() || alt == ziel {
        return false;
    }
    match std::fs::copy(&alt, &ziel) {
        Ok(_) => true,
        Err(e) => {
            eprintln!(
                "Einstellungen konnten nicht aus {} übernommen werden: {e}",
                alt.display()
            );
            false
        }
    }
}

/// Eine zweite Instanz auf derselben Wurzel.
#[derive(Clone, Debug)]
pub struct Sperrkonflikt {
    /// PID aus der Sperrdatei (0 = unbekannt).
    pub pid: u32,
    /// Die Sperrdatei.
    pub pfad: PathBuf,
}

/// Hält die Sperrdatei `<Wurzel>/sperre.lock` für die Laufzeit des Prozesses.
/// Beim Ablegen wird sie nur entfernt, wenn sie noch die eigene PID trägt.
#[derive(Debug)]
pub struct Sperre {
    pfad: PathBuf,
    pid: u32,
}

impl Sperre {
    /// PID, die in der Sperrdatei steht (eigene).
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn pfad(&self) -> &Path {
        &self.pfad
    }
}

impl Drop for Sperre {
    fn drop(&mut self) {
        if let Ok(s) = std::fs::read_to_string(&self.pfad) {
            if s.trim().parse::<u32>().ok() == Some(self.pid) {
                let _ = std::fs::remove_file(&self.pfad);
            }
        }
    }
}

/// Nimmt die Sperre der Wurzel. Ist sie von einem **lebenden** Prozess
/// belegt, kommt ein [`Sperrkonflikt`] zurück; eine Sperrdatei eines
/// abgestürzten Prozesses wird übernommen.
pub fn sperre_nehmen() -> Result<Sperre, Sperrkonflikt> {
    sperre_nehmen_in(&wurzel())
}

/// Wie [`sperre_nehmen`], aber mit ausdrücklicher Wurzel (Tests).
pub fn sperre_nehmen_in(w: &Path) -> Result<Sperre, Sperrkonflikt> {
    let pfad = w.join(SPERRE_DATEI);
    if let Some(parent) = pfad.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let pid = std::process::id();
    for versuch in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pfad)
        {
            Ok(mut datei) => {
                use std::io::Write;
                let _ = write!(datei, "{pid}");
                return Ok(Sperre { pfad, pid });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let fremd = std::fs::read_to_string(&pfad)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                // Auch die eigene PID ist ein Konflikt: dieselbe Wurzel darf
                // nur eine Sperre tragen.
                if prozess_lebt(fremd) {
                    return Err(Sperrkonflikt { pid: fremd, pfad });
                }
                // Alte Sperre eines beendeten Prozesses: weg damit und noch
                // einmal versuchen.
                if versuch == 0 {
                    let _ = std::fs::remove_file(&pfad);
                }
            }
            Err(e) => {
                return Err(Sperrkonflikt { pid: 0, pfad: pfad.clone() })
                    .map_err(|k| {
                        eprintln!("Sperrdatei {} nicht schreibbar: {e}", k.pfad.display());
                        k
                    });
            }
        }
    }
    Err(Sperrkonflikt { pid: 0, pfad })
}

/// Lebt der Prozess mit dieser PID?
#[cfg(unix)]
fn prozess_lebt(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if Path::new("/proc").is_dir() {
        return Path::new(&format!("/proc/{pid}")).exists();
    }
    // macOS und andere Unixe ohne /proc: `kill -0` fragt den Kernel.
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn prozess_lebt(pid: u32) -> bool {
    // Ohne billige Abfrage gilt eine vorhandene Sperre als lebend — lieber
    // einmal zu viel melden als zwei Schreiber auf derselben Wurzel.
    pid != 0
}

/// Blöcke aus der Benutzerbibliothek — ohne Meldungen (ältere Aufrufer).
///
/// Siehe [`lade_benutzerbloecke_mit_meldungen`].
pub fn lade_benutzerbloecke(dir: &Path) -> Vec<BlockDef> {
    lade_benutzerbloecke_mit_meldungen(dir).0
}

/// Blöcke aus der Benutzerbibliothek `<blocks>/`.
///
/// Zwei Formen liegen dort: die **eigene Blockdatei** `<slug>.jjblk` (eine
/// Datei je Block, T0128) und die ältere Form `.jjcad` — eine Zeichnung, deren
/// **nicht** mitgelieferte Blöcke übernommen werden. Die Kennung des
/// Systembestands bleibt beim Programm. Unlesbare Dateien werden gemeldet und
/// übersprungen; sie reißen die Bibliothek nicht mit.
pub fn lade_benutzerbloecke_mit_meldungen(dir: &Path) -> (Vec<BlockDef>, Vec<String>) {
    let (mut out, meldungen) = crate::bloecke::lade_bloecke(dir);
    let mut dateien: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(eintraege) => eintraege
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("jjcad"))
                        .unwrap_or(false)
            })
            .collect(),
        Err(_) => return (out, meldungen),
    };
    dateien.sort();
    let mut meldungen = meldungen;
    for datei in dateien {
        let doc = match crate::persist::load_document(&datei) {
            Ok(doc) => doc,
            Err(grund) => {
                meldungen.push(format!("{}: {grund}", datei.display()));
                continue;
            }
        };
        for mut b in doc.blocks {
            if b.source == crate::model::BlockSource::System {
                continue;
            }
            if out.iter().any(|x| x.id == b.id) {
                continue;
            }
            b.source = crate::model::BlockSource::User;
            out.push(b);
        }
    }
    (out, meldungen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-daten-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Erste Übereinstimmung gewinnt: Argument vor Umgebung vor `portable.txt`
    /// vor Plattformstandard.
    #[test]
    fn wurzel_reihenfolge() {
        let exe = Path::new("/opt/jjcad");
        let arg = Path::new("/daten/argument");
        let env = Path::new("/daten/umgebung");

        assert_eq!(wurzel_ermitteln(Some(arg), Some(env), exe, true), arg);
        assert_eq!(wurzel_ermitteln(None, Some(env), exe, true), env);
        assert_eq!(
            wurzel_ermitteln(None, None, exe, true),
            PathBuf::from("/opt/jjcad/data")
        );
        assert_eq!(wurzel_ermitteln(None, None, exe, false), plattform_standard());
        // Ein leerer Eintrag zählt nicht als Vorgabe.
        assert_eq!(
            wurzel_ermitteln(Some(Path::new("")), Some(env), exe, false),
            env
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn plattformstandard_ist_xdg_datenverzeichnis() {
        let p = plattform_standard();
        assert!(
            p.ends_with("JoJa-CAD"),
            "Wurzel muss auf JoJa-CAD enden: {}",
            p.display()
        );
        let s = p.to_string_lossy().to_string();
        assert!(
            s.contains(".local/share") || std::env::var_os("XDG_DATA_HOME").is_some(),
            "Linux-Vorgabe muss unter ~/.local/share liegen: {s}"
        );
    }

    /// Relative Angaben hängen an der Wurzel, absolute bleiben, leer ergibt
    /// den Unterordner der Wurzel.
    #[test]
    fn pfadangaben_werden_aufgeloest() {
        let w = test_dir("pfade");
        assert_eq!(pfad_in_wurzel_basis(&w, "", "blocks"), w.join("blocks"));
        assert_eq!(
            pfad_in_wurzel_basis(&w, "eigene/bloecke", "blocks"),
            w.join("eigene/bloecke")
        );
        assert_eq!(
            pfad_in_wurzel_basis(&w, "/var/lib/bloecke", "blocks"),
            PathBuf::from("/var/lib/bloecke")
        );
        assert_eq!(
            pfad_in_wurzel_basis(&w, "  /var/lib/bloecke  ", "blocks"),
            PathBuf::from("/var/lib/bloecke")
        );
    }

    /// Die Schreibprobe nimmt einen beschreibbaren Ordner an und lehnt einen
    /// ab, unter dem sich keine Datei anlegen lässt.
    #[test]
    fn schreibprobe_lehnt_unbeschreibbar_ab() {
        let w = test_dir("probe");
        let gut = w.join("blocks");
        assert!(pruefe_verzeichnis(&gut).is_ok());
        assert!(gut.is_dir(), "Ordner muss angelegt sein");

        // Eine Datei an der Stelle des Ordners: create_dir_all scheitert.
        let datei = w.join("datei");
        std::fs::write(&datei, b"x").unwrap();
        let schlecht = datei.join("unterordner");
        assert!(
            pruefe_verzeichnis(&schlecht).is_err(),
            "Pfad unter einer Datei muss abgelehnt werden"
        );
    }

    /// Beim ersten Start wandert die alte `prefs.json` in die Wurzel; die
    /// alte Datei bleibt liegen.
    #[test]
    fn alte_prefs_werden_kopiert() {
        let w = test_dir("migration");
        let alt = w.join("altes-config");
        let alt_datei = alt.join(PREFS_DATEI);
        std::fs::create_dir_all(&alt).unwrap();
        std::fs::write(&alt_datei, r#"{"last_dir":"/alt"}"#).unwrap();

        assert!(vorbereiten_in(&w, &alt).is_ok());
        let neu = w.join(PREFS_DATEI);
        assert!(neu.is_file(), "prefs.json muss in der Wurzel liegen");
        assert_eq!(
            std::fs::read_to_string(&neu).unwrap(),
            r#"{"last_dir":"/alt"}"#
        );
        assert!(alt_datei.is_file(), "alte Datei bleibt liegen");

        // Ein zweiter Lauf überschreibt die Wurzel nicht.
        std::fs::write(&neu, r#"{"last_dir":"/neu"}"#).unwrap();
        assert!(!prefs_uebernehmen_aus(&w, &alt));
        assert_eq!(
            std::fs::read_to_string(&neu).unwrap(),
            r#"{"last_dir":"/neu"}"#
        );

        let _ = std::fs::remove_file(&alt_datei);
    }

    /// Eine zweite Sperre auf derselben Wurzel meldet den Konflikt; nach dem
    /// Ablegen ist die Wurzel wieder frei.
    /// Portabel übernimmt keine Einstellungen vom Rechner — die Quelle ist die
    /// Wurzel selbst, also gibt es nichts zu kopieren.
    #[test]
    fn portabel_uebernimmt_keine_fremden_einstellungen() {
        let dir = test_dir("portabel-import");
        let w = dir.join("data");
        std::fs::create_dir_all(&w).unwrap();
        std::fs::write(w.join(PREFS_DATEI), "{}").unwrap();
        std::fs::remove_file(w.join(PREFS_DATEI)).unwrap();

        vorbereiten_in(&w, &w).unwrap();
        assert!(
            !w.join(PREFS_DATEI).exists(),
            "portabler Lauf hat fremde Einstellungen übernommen"
        );
        assert!(w.join("blocks").is_dir(), "Unterordner fehlen");
    }

    #[test]
    fn zweite_instanz_meldet_sich() {
        let w = test_dir("sperre");
        let erste = sperre_nehmen_in(&w).expect("erste Sperre");
        assert!(w.join(SPERRE_DATEI).is_file());
        let konflikt = sperre_nehmen_in(&w).expect_err("zweite Sperre muss abgelehnt werden");
        assert_eq!(konflikt.pid, std::process::id());
        drop(erste);
        assert!(
            !w.join(SPERRE_DATEI).exists(),
            "Sperre muss beim Ablegen verschwinden"
        );
        let wieder = sperre_nehmen_in(&w).expect("nach dem Ablegen wieder frei");
        drop(wieder);
    }

    /// Eine Sperrdatei eines beendeten Prozesses wird übernommen, nicht
    /// gemeldet.
    #[test]
    fn verwaiste_sperre_wird_uebernommen() {
        let w = test_dir("sperre-alt");
        // PID 999999 gibt es praktisch nicht.
        std::fs::write(w.join(SPERRE_DATEI), b"999999").unwrap();
        let s = sperre_nehmen_in(&w).expect("verwaiste Sperre muss übernommen werden");
        assert_eq!(
            std::fs::read_to_string(w.join(SPERRE_DATEI)).unwrap().trim(),
            std::process::id().to_string()
        );
        drop(s);
    }

    /// Blöcke aus der Benutzerbibliothek tragen danach die Herkunft „Benutzer“.
    #[test]
    fn benutzerbloecke_kommen_aus_dem_ordner() {
        let w = test_dir("bloecke");
        let dir = w.join("blocks");
        std::fs::create_dir_all(&dir).unwrap();
        let mut doc = crate::model::Document::new();
        let mut b = doc.blocks[0].clone();
        b.id = "benutzer-tuer".into();
        b.name = "Eigene Tür".into();
        b.source = crate::model::BlockSource::Project;
        doc.blocks.push(b);
        crate::persist::save_document(&mut doc, &dir.join("eigene.jjcad")).unwrap();

        let geladen = lade_benutzerbloecke(&dir);
        assert!(
            geladen.iter().any(|b| b.id == "benutzer-tuer"),
            "Block aus der Datei fehlt"
        );
        assert!(
            geladen.iter().all(|b| b.source != crate::model::BlockSource::System),
            "Systemblöcke gehören nicht in die Benutzerbibliothek"
        );
        assert!(geladen
            .iter()
            .find(|b| b.id == "benutzer-tuer")
            .map(|b| b.source == crate::model::BlockSource::User)
            .unwrap_or(false));
    }

    /// Eine Zeichnung ohne Kennung im Kopf muss über Sitzungen hinweg
    /// **dieselbe** Kennung behalten — sonst legt der Sicherungspool bei jedem
    /// Öffnen einen neuen Pin für dieselbe Datei an.
    #[test]
    fn kennung_ohne_kopfzeile_bleibt_gleich() {
        let dir = test_dir("kennung");
        let a = dir.join("a.jjcad");
        let b = dir.join("b.jjcad");
        std::fs::write(&a, "{}").unwrap();
        std::fs::write(&b, "{}").unwrap();

        assert_eq!(
            kennung_fuer_pfad(&a),
            kennung_fuer_pfad(&a),
            "gleicher Pfad muss dieselbe Kennung ergeben"
        );
        assert_ne!(
            kennung_fuer_pfad(&a),
            kennung_fuer_pfad(&b),
            "zwei Dateien dürfen nicht dieselbe Kennung bekommen"
        );

        let k = kennung_fuer_pfad(&a);
        assert_eq!(k.len(), 36, "Kennung in UUID-Form: {k}");
        assert_eq!(k.matches('-').count(), 4, "Kennung in UUID-Form: {k}");
    }
}
