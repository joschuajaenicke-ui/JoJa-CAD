//! Eigene Blöcke als einzelne Dateien (T0128).
//!
//! Ein Block des Benutzers liegt in **einer** Datei
//! `<blocks>/<slug>.jjblk`:
//!
//! ```json
//! {"format":"jjcad-block","version":1,"block":{ … }}
//! ```
//!
//! `block` ist ein [`BlockDef`] wie in der `.jjcad`-Datei. Das Verzeichnis ist
//! die Wahrheit — es gibt **keinen** Index, der auseinanderlaufen könnte. Der
//! Dateiname folgt dem Blocknamen (`slug`), die Kennung (`usr:…`) steht im
//! Inhalt und bleibt beim Umbenennen gleich: umbenannt wird die Datei plus das
//! Feld `name`.
//!
//! Eine unlesbare Datei wird beim Laden **gemeldet und übersprungen**; sie
//! reißt die übrige Bibliothek nicht mit. Zwei Blöcke gleichen Namens bekommen
//! zwei Dateien (`…-2.jjblk`), keiner überschreibt den anderen.

use crate::model::{BlockDef, BlockSource};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Kennzeichen im Dateikopf.
pub const FORMAT: &str = "jjcad-block";
/// Fassung des Dateiformats.
pub const VERSION: u32 = 1;
/// Endung der Blockdatei.
pub const ENDUNG: &str = "jjblk";

/// Aufbau einer `.jjblk`-Datei.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Blockdatei {
    pub format: String,
    pub version: u32,
    pub block: BlockDef,
}

/// Dateiname-Stamm aus dem Blocknamen: Kleinbuchstaben, Umlaute aufgelöst,
/// alles andere wird zum Bindestrich. Nie leer — ein Name ohne verwertbare
/// Zeichen heißt `block`.
pub fn slug(name: &str) -> String {
    let mut roh = String::new();
    for c in name.chars() {
        match c {
            'ä' | 'Ä' => roh.push_str("ae"),
            'ö' | 'Ö' => roh.push_str("oe"),
            'ü' | 'Ü' => roh.push_str("ue"),
            'ß' => roh.push_str("ss"),
            _ => roh.push(c),
        }
    }
    let mut out = String::new();
    for c in roh.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("block");
    }
    // Windows kennt keine Gerätenamen als Dateinamen.
    const GERAETE: [&str; 9] = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "lpt1",
    ];
    if GERAETE.contains(&out.as_str()) {
        out.insert_str(0, "block-");
    }
    out
}

/// Alle Blockdateien im Verzeichnis, nach Namen sortiert (feste Reihenfolge
/// beim Laden). Ein fehlendes Verzeichnis ist keine Bibliothek.
pub fn dateien(dir: &Path) -> Vec<PathBuf> {
    let Ok(eintraege) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = eintraege
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(ENDUNG))
                    .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}

/// Liest eine Blockdatei und prüft Kopf und Fassung.
pub fn lies(pfad: &Path) -> Result<Blockdatei, String> {
    let text = std::fs::read_to_string(pfad)
        .map_err(|e| format!("nicht lesbar: {e}"))?;
    let datei: Blockdatei = serde_json::from_str(&text)
        .map_err(|e| format!("kein gültiger Block (JSON): {e}"))?;
    if !datei.format.eq_ignore_ascii_case(FORMAT) {
        return Err(format!(
            "falsches Kennzeichen „{}“ statt „{FORMAT}“",
            datei.format
        ));
    }
    if datei.version > VERSION {
        return Err(format!(
            "Fassung {} ist neuer als die unterstützte {VERSION}",
            datei.version
        ));
    }
    Ok(datei)
}

/// Trägt die Datei die Kennung? Unlesbare Dateien gehören zu niemandem.
fn gehoert_zu(pfad: &Path, id: &str) -> bool {
    lies(pfad).map(|d| d.block.id == id).unwrap_or(false)
}

/// Erster freier Dateiname für `basis`. Eine vorhandene Datei mit **derselben**
/// Kennung darf überschrieben werden (Änderung an einem Block), eine fremde
/// wird nie angefasst — dann rückt der Name auf `basis-2`, `basis-3`, …
fn freier_pfad(dir: &Path, basis: &str, id: &str) -> PathBuf {
    let erst = dir.join(format!("{basis}.{ENDUNG}"));
    if !erst.exists() || gehoert_zu(&erst, id) {
        return erst;
    }
    for n in 2..1000u32 {
        let p = dir.join(format!("{basis}-{n}.{ENDUNG}"));
        if !p.exists() || gehoert_zu(&p, id) {
            return p;
        }
    }
    dir.join(format!("{basis}-{}.{}", crate::util::short_id(), ENDUNG))
}

/// Schreibt einen Block als `<blocks>/<slug>.jjblk` und gibt den Pfad zurück.
///
/// Die Datei trägt immer die Herkunft „Benutzer“. Liegt der Block schon (gleiche
/// Kennung) unter einem anderen Namen, wird die alte Datei nach dem Schreiben
/// entfernt — das ist der Weg des Umbenennens.
pub fn schreibe_block(dir: &Path, block: &BlockDef) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Ordner {} nicht anlegbar: {e}", dir.display()))?;
    let mut block = block.clone();
    block.source = BlockSource::User;
    let ziel = freier_pfad(dir, &slug(&block.name), &block.id);

    let datei = Blockdatei {
        format: FORMAT.to_string(),
        version: VERSION,
        block: block.clone(),
    };
    let text = serde_json::to_string_pretty(&datei)
        .map_err(|e| format!("Block nicht schreibbar: {e}"))?;
    std::fs::write(&ziel, format!("{text}\n"))
        .map_err(|e| format!("{} nicht schreibbar: {e}", ziel.display()))?;

    // Alte Datei desselben Blocks (Umbenennen) wegräumen.
    for alt in dateien(dir) {
        if alt != ziel && gehoert_zu(&alt, &block.id) {
            let _ = std::fs::remove_file(&alt);
        }
    }
    Ok(ziel)
}

/// Entfernt alle Dateien, die zu dieser Kennung gehören. `false`, wenn keine
/// Datei zu finden war (z. B. ein reiner Projektblock).
pub fn loesche_block(dir: &Path, id: &str) -> Result<bool, String> {
    let mut entfernt = false;
    for pfad in dateien(dir) {
        if gehoert_zu(&pfad, id) {
            std::fs::remove_file(&pfad)
                .map_err(|e| format!("{} nicht löschbar: {e}", pfad.display()))?;
            entfernt = true;
        }
    }
    Ok(entfernt)
}

/// Setzt den Namen eines gespeicherten Blocks: Datei umbenennen **und** das
/// Feld `name` schreiben — die Kennung bleibt unverändert.
pub fn umbenennen(dir: &Path, id: &str, neuer_name: &str) -> Result<Option<PathBuf>, String> {
    for pfad in dateien(dir) {
        if !gehoert_zu(&pfad, id) {
            continue;
        }
        let mut datei = lies(&pfad)?;
        datei.block.name = neuer_name.to_string();
        return schreibe_block(dir, &datei.block).map(Some);
    }
    Ok(None)
}

/// Liest die ganze Bibliothek: die Blöcke und je unlesbarer Datei eine Meldung.
///
/// Eine doppelte Kennung wird nur einmal genommen (die erste Datei gewinnt) und
/// gemeldet; eine Datei ohne Namen bekommt den Dateinamen als Namen.
pub fn lade_bloecke(dir: &Path) -> (Vec<BlockDef>, Vec<String>) {
    let mut out: Vec<BlockDef> = Vec::new();
    let mut meldungen: Vec<String> = Vec::new();
    for pfad in dateien(dir) {
        let mut datei = match lies(&pfad) {
            Ok(d) => d,
            Err(grund) => {
                meldungen.push(format!("{}: {grund}", pfad.display()));
                continue;
            }
        };
        if datei.block.id.trim().is_empty() {
            datei.block.id = format!("usr:{}", slug(&datei.block.name));
        }
        if datei.block.name.trim().is_empty() {
            datei.block.name = pfad
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Block")
                .to_string();
        }
        if out.iter().any(|b| b.id == datei.block.id) {
            meldungen.push(format!(
                "{}: Kennung „{}“ ist schon vergeben — Datei übersprungen",
                pfad.display(),
                datei.block.id
            ));
            continue;
        }
        datei.block.source = BlockSource::User;
        out.push(datei.block);
    }
    (out, meldungen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_loest_umlaute_und_sonderzeichen_auf() {
        assert_eq!(slug("Eigene Tür"), "eigene-tuer");
        assert_eq!(slug("  Küche / Spüle  "), "kueche-spuele");
        assert_eq!(slug("Größe"), "groesse");
        assert_eq!(slug("///"), "block");
        assert_eq!(slug("con"), "block-con");
    }
}
