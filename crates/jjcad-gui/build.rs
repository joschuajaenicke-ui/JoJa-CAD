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

    // Windows: Symbol und Versionsangaben in die .exe einbetten, damit die
    // Datei im Explorer und in der Taskleiste wie das Programm aussieht.
    //
    // Die .rc-Datei wird **hier erzeugt**, nicht gepflegt: die Version kommt aus
    // `CARGO_PKG_VERSION`. Eine handgepflegte .rc bleibt sonst still auf der
    // alten Version stehen — die .exe hieße 0.2.1 und behauptete im Explorer
    // 0.2.0.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/jjc-icon.ico");
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
        let teile: Vec<u32> = version
            .split('.')
            .map(|t| t.parse::<u32>().unwrap_or(0))
            .collect();
        let (a, b, c) = (
            *teile.first().unwrap_or(&0),
            *teile.get(1).unwrap_or(&0),
            *teile.get(2).unwrap_or(&0),
        );
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let icon = std::path::Path::new(&manifest).join("assets").join("jjc-icon.ico");
        // Im .rc sind Backslashes Fluchtzeichen (`\a` = Glocke) — der Pfad muss
        // verdoppelt werden, sonst sucht rc.exe in `\x07ssets\...`.
        let icon_pfad = icon.display().to_string().replace('\\', "\\\\");
        let rc = format!(
            "1 ICON \"{}\"\n\n\
             1 VERSIONINFO\n\
             FILEVERSION {a},{b},{c},0\n\
             PRODUCTVERSION {a},{b},{c},0\n\
             FILEOS 0x40004\n\
             FILETYPE 0x1\n\
             BEGIN\n\
             \x20 BLOCK \"StringFileInfo\"\n\
             \x20 BEGIN\n\
             \x20   BLOCK \"040704b0\"\n\
             \x20   BEGIN\n\
             \x20     VALUE \"CompanyName\", \"Joschua Jaenicke\"\n\
             \x20     VALUE \"FileDescription\", \"JoJa-CAD - 2D-CAD fuer Architektur und Bauantragszeichnungen\"\n\
             \x20     VALUE \"FileVersion\", \"{version}\"\n\
             \x20     VALUE \"InternalName\", \"jjcad\"\n\
             \x20     VALUE \"LegalCopyright\", \"Copyright (c) 2026 Joschua Jaenicke\"\n\
             \x20     VALUE \"OriginalFilename\", \"jjcad.exe\"\n\
             \x20     VALUE \"ProductName\", \"JoJa-CAD\"\n\
             \x20     VALUE \"ProductVersion\", \"{version}\"\n\
             \x20   END\n\
             \x20 END\n\
             \x20 BLOCK \"VarFileInfo\"\n\
             \x20 BEGIN\n\
             \x20   VALUE \"Translation\", 0x407, 1200\n\
             \x20 END\n\
             END\n",
            icon_pfad
        );
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
        let pfad = out.join("jjcad.rc");
        std::fs::write(&pfad, rc).expect("jjcad.rc schreiben");
        embed_resource::compile(&pfad, embed_resource::NONE);
    }
}
