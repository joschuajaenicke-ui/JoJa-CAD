//! Ermittelt beim Bauen die Build-Nummer aus der Git-Historie und reicht sie
//! als Compile-Zeit-Konstante an den Code weiter. Damit steht an jeder
//! laufenden Instanz und an jedem Testbinary eindeutig, welcher Stand drin ist.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir("../..")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let count = git(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".to_string());
    let hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "nogit".to_string());
    let dirty = git(&["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    println!("cargo:rustc-env=JJCAD_BUILD={}", count);
    println!(
        "cargo:rustc-env=JJCAD_HASH={}{}",
        hash,
        if dirty { "+" } else { "" }
    );

    // Neu bauen, wenn sich der Stand ändert
    // Auslieferungsnummer (vom build-live.sh gesetzt) — im Programm sichtbar
    let ship = std::env::var("JJCAD_SHIP").unwrap_or_default();
    println!("cargo:rustc-env=JJCAD_SHIP={}", ship);
    println!("cargo:rerun-if-env-changed=JJCAD_SHIP");

    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}
