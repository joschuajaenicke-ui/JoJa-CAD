//! T0105 — das Prüfskript `scripts/i18n-check.py` als Tor in `cargo test`.
//!
//! Ohne diesen Test prüft nur, wer das Skript von Hand aufruft; dann verrottet
//! der Katalog wieder unbemerkt. Fehlt Python 3 (Windows ohne Python, sehr
//! schlanke Container), wird übersprungen — der Aufruf von Hand steht in
//! `docs/technik/I18N.md`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// `scripts/i18n-check.py` in der Workspace-Wurzel.
fn skript() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Workspace-Wurzel")
        .join("scripts")
        .join("i18n-check.py")
}

/// Startet das Skript; `None`, wenn kein `python3` vorhanden ist.
fn starte(argumente: &[&str]) -> Option<Output> {
    let pfad = skript();
    assert!(pfad.is_file(), "Prüfskript fehlt: {}", pfad.display());
    match Command::new("python3").arg(&pfad).args(argumente).output() {
        Ok(ausgabe) => Some(ausgabe),
        Err(e) => {
            eprintln!("python3 nicht verfügbar ({e}) — i18n-Prüfung übersprungen");
            None
        }
    }
}

fn text(ausgabe: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&ausgabe.stdout),
        String::from_utf8_lossy(&ausgabe.stderr)
    )
}

/// Der Bestand ist sauber: Exitcode 0. Schlägt fehl, sobald ein Schlüssel
/// fehlt, verwaist ist, ein Platzhalter nicht passt oder eine Sprachdatei
/// nicht in `i18n.rs` eingetragen ist.
#[test]
fn katalog_besteht_die_pruefung() {
    let Some(ausgabe) = starte(&[]) else { return };
    assert!(
        ausgabe.status.success(),
        "scripts/i18n-check.py meldet Fehler:\n{}",
        text(&ausgabe)
    );
}

/// Das Skript findet die drei absichtlich eingebauten Fehler (fehlender
/// Schlüssel, falscher Platzhalter, leerer Wert) und meldet für den sauberen
/// Bestand 0 — die Gegenprobe zum Tor.
#[test]
fn pruefskript_findet_die_eingebauten_fehler() {
    let Some(ausgabe) = starte(&["--selbsttest"]) else { return };
    assert!(
        ausgabe.status.success(),
        "Selbsttest des Prüfskripts durchgefallen:\n{}",
        text(&ausgabe)
    );
}
