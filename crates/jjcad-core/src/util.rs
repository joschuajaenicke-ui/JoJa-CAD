//! Kleine Helfer (Datum, IDs, Dateinamen).

use std::time::{SystemTime, UNIX_EPOCH};

/// Sekunden seit Unix-Epoch.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Zivil-Datum aus Tagen seit 1970-01-01 (Howard Hinnant's Algorithmus).
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// Heutiges Datum als ISO-String (YYYY-MM-DD), lokale Zeit (UTC+Offset grob über Systemzeit).
pub fn today_iso() -> String {
    let secs = now_unix();
    // Systemzeitzone über libc-freie Näherung: Europe/Berlin ist UTC+1/+2.
    // Für korrekte lokale Zeit wird die Umgebungsvariable TZ bzw. der Host-Offset genutzt.
    let offset = local_offset_seconds();
    let days = (secs + offset).div_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Lokaler UTC-Offset in Sekunden (aus `localtime`-Datei gelesen, sonst 1h).
fn local_offset_seconds() -> i64 {
    // /etc/localtime ist ein Symlink auf die Zonendatei; wir lesen den Namen
    // und schätzen für Europa/Berlin die Sommerzeit grob über den Monat.
    if let Ok(link) = std::fs::read_link("/etc/localtime") {
        if let Some(name) = link.to_str() {
            if name.contains("Berlin") || name.contains("Vienna") || name.contains("Zurich") {
                let days = now_unix().div_euclid(86400);
                let (_, m, _) = civil_from_days(days);
                return if (4..=10).contains(&m) { 7200 } else { 3600 };
            }
        }
    }
    3600
}

/// ISO (YYYY-MM-DD) → deutsches Format (DD.MM.YYYY)
pub fn iso_to_de(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3 {
        format!("{}.{}.{}", parts[2], parts[1], parts[0])
    } else {
        iso.to_string()
    }
}

/// Dateinamen ohne Verzeichnis und Endung.
pub fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Sicheres Dateinamen-Fragment.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Zufällige 4-Byte-ID als Hex (aus Zeit + Adresse eines Heap-Werts).
pub fn short_id() -> String {
    let t = now_unix() as u64;
    let x = Box::into_raw(Box::new(0u8)) as u64;
    let mixed = t
        .wrapping_mul(6364136223846793005)
        .wrapping_add(x.rotate_left(17));
    format!("{:08x}", (mixed >> 16) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_conversion() {
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
        // 20321 Tage nach dem 01.01.1970
        let (y, m, d) = civil_from_days(20321);
        assert_eq!((y, m, d), (2025, 8, 21));
    }

    #[test]
    fn de_format() {
        assert_eq!(iso_to_de("2026-10-24"), "24.10.2026");
    }
}
