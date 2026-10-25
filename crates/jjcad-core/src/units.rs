//! Einheiten, Maßstäbe und Zahlenformatierung.
//!
//! Intern wird ausschließlich in **Millimetern** gerechnet. Die Einheit in den
//! Einstellungen betrifft nur Eingabe und Anzeige.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Unit {
    #[default]
    Mm,
    Cm,
    M,
}

impl Unit {
    pub const ALL: [Unit; 3] = [Unit::Mm, Unit::Cm, Unit::M];

    /// Millimeter pro Einheit.
    pub fn mm(self) -> f64 {
        match self {
            Unit::Mm => 1.0,
            Unit::Cm => 10.0,
            Unit::M => 1000.0,
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::M => "m",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Unit::Mm => "Millimeter",
            Unit::Cm => "Zentimeter",
            Unit::M => "Meter",
        }
    }

    /// Sinnvolle Nachkommastellen für Anzeige.
    pub fn decimals(self) -> usize {
        match self {
            Unit::Mm => 0,
            Unit::Cm => 1,
            Unit::M => 2,
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Unit> {
        match s.trim().to_lowercase().as_str() {
            "mm" | "millimeter" => Some(Unit::Mm),
            "cm" | "zentimeter" => Some(Unit::Cm),
            "m" | "meter" => Some(Unit::M),
            _ => None,
        }
    }

    /// mm → Anzeigeeinheit
    pub fn from_mm(self, mm: f64) -> f64 {
        mm / self.mm()
    }

    /// Anzeigeeinheit → mm
    pub fn to_mm(self, val: f64) -> f64 {
        val * self.mm()
    }

    /// Zahl mit Einheit, z. B. "3,50 m".
    pub fn format(self, mm: f64, decimals: usize) -> String {
        format!("{} {}", format_number(self.from_mm(mm), decimals), self.short())
    }

    pub fn format_plain(self, mm: f64, decimals: usize) -> String {
        format_number(self.from_mm(mm), decimals)
    }

    /// Längenwert für Eingabefelder: mindestens die Stellenzahl der Einheit
    /// (mm 0, cm 1, m 2), mehr nur wenn der Wert sie wirklich braucht.
    ///
    /// Eingabefelder dürfen keine Genauigkeit verschweigen: mit fester
    /// Stellenzahl stand für 0,24 m nur „0 m“ im Feld. Umgekehrt soll ein
    /// mm-Feld nicht „240,0“ zeigen. Deshalb: Nachkommastellen der Einheit
    /// immer, zwei Reserve-Stellen nur bei echtem Bedarf (0,2405 m), keine
    /// Tausenderpunkte — die wären beim Wiedereinlesen nicht eindeutig
    /// („12.500“ könnte 12500 oder 12,5 heißen).
    pub fn format_length_auto(self, mm: f64) -> String {
        if !mm.is_finite() {
            return "–".to_string();
        }
        let min = self.decimals();
        let mut s = format!("{:.*}", min + 2, self.from_mm(mm)).replace('.', ",");
        if let Some(pos) = s.rfind(',') {
            let mut dec = s.len() - pos - 1;
            while dec > min && s.ends_with('0') {
                s.pop();
                dec -= 1;
            }
            if dec == 0 {
                s.pop();
            }
        }
        if s == "-0" {
            s = "0".to_string();
        }
        s
    }

    /// Fläche (mm²) in der Einheit², z. B. "12,5 m²".
    pub fn format_area(self, mm2: f64, decimals: usize) -> String {
        let f = self.mm() * self.mm();
        format!("{} {}²", format_number(mm2 / f, decimals), self.short())
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short())
    }
}

/// Zahl mit deutschem Dezimalkomma und Tausenderpunkt.
pub fn format_number(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return "–".to_string();
    }
    let neg = v < -0.0;
    let s = format!("{:.*}", decimals, v.abs());
    let (int_part, dec_part) = match s.split_once('.') {
        Some((a, b)) => (a.to_string(), Some(b.to_string())),
        None => (s.clone(), None),
    };
    // Tausenderpunkte
    let mut out = String::new();
    let bytes: Vec<char> = int_part.chars().collect();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(*c);
    }
    if let Some(d) = dec_part {
        if decimals > 0 {
            out.push(',');
            out.push_str(&d);
        }
    }
    if neg {
        out.insert(0, '-');
    }
    out
}

/// Länge parsen: "3,5", "3.5", "350 mm", "3,50m", "1,20 m".
/// Rückgabe in Millimetern.
pub fn parse_length(input: &str, default_unit: Unit) -> Option<f64> {
    let s = input.trim().replace('\u{00a0}', " ");
    if s.is_empty() {
        return None;
    }
    let lower = s.to_lowercase();
    let mut unit = default_unit;
    let mut num = lower.as_str();
    for (suffix, u) in [("mm", Unit::Mm), ("cm", Unit::Cm), ("m", Unit::M)] {
        if lower.ends_with(suffix) {
            let candidate = lower[..lower.len() - suffix.len()].trim();
            // "3,50 m" -> ok; aber "1.500" endet nicht auf m
            if !candidate.is_empty() && candidate.chars().any(|c| c.is_ascii_digit()) {
                unit = u;
                num = candidate;
                break;
            }
        }
    }
    let cleaned: String = num
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-' || *c == '+')
        .collect();
    let cleaned = cleaned.replace(',', ".");
    let val: f64 = cleaned.parse().ok()?;
    Some(unit.to_mm(val))
}

/// Maßstab parsen: "1:100", "100", "1/50".
pub fn parse_scale(s: &str) -> Option<f64> {
    let t = s.trim();
    let part = t.split([':', '/']).last()?.trim();
    let v: f64 = part.replace(',', ".").parse().ok()?;
    if v > 0.0 {
        Some(v)
    } else {
        None
    }
}

pub fn format_scale(scale: f64) -> String {
    if (scale - scale.round()).abs() < 1e-9 {
        format!("1:{}", scale.round() as i64)
    } else {
        format!("1:{:.1}", scale)
    }
}

/// Maßtext für Bemaßung: in der Anzeigeeinheit, ohne Einheitensuffix,
/// mit führender Null bei Werten < 1 (DIN-üblich: 0,85).
pub fn dim_text(mm: f64, unit: Unit, decimals: usize) -> String {
    format_number(unit.from_mm(mm), decimals)
}

/// Höhenkote formatieren: ±0,00 / +3,50 / -1,20 (immer in Metern, 2 Stellen).
pub fn elevation_text(z_mm: f64) -> String {
    let m = z_mm / 1000.0;
    if m.abs() < 0.0005 {
        "±0,00".to_string()
    } else if m > 0.0 {
        format!("+{:.2}", m).replace('.', ",")
    } else {
        format!("{:.2}", m).replace('.', ",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_variants() {
        assert_eq!(parse_length("3,50 m", Unit::Mm), Some(3500.0));
        assert_eq!(parse_length("3.5", Unit::M), Some(3500.0));
        assert_eq!(parse_length("350", Unit::Cm), Some(3500.0));
        assert_eq!(parse_length("350mm", Unit::M), Some(350.0));
        assert_eq!(parse_length("-1,2", Unit::M), Some(-1200.0));
    }

    #[test]
    fn number_format() {
        assert_eq!(format_number(1234567.891, 2), "1.234.567,89");
        assert_eq!(format_number(-0.5, 2), "-0,50");
        assert_eq!(format_number(0.0, 2), "0,00");
    }

    #[test]
    fn laengenfeld_format() {
        // Metern: 240 mm Wanddicke und 2750 mm Höhe waren als „0 m“/„3 m“ sichtbar
        assert_eq!(Unit::M.format_length_auto(240.0), "0,24");
        assert_eq!(Unit::M.format_length_auto(2750.0), "2,75");
        assert_eq!(Unit::M.format_length_auto(1000.0), "1,00");
        assert_eq!(Unit::M.format_length_auto(0.0), "0,00");
        // Millimeter: keine überflüssigen Nullen, aber echte Genauigkeit bleibt
        assert_eq!(Unit::Mm.format_length_auto(240.0), "240");
        assert_eq!(Unit::Mm.format_length_auto(240.5), "240,5");
        // Zentimeter: eine Stelle wie die Einheit vorgibt
        assert_eq!(Unit::Cm.format_length_auto(2750.0), "275,0");
        assert_eq!(Unit::Cm.format_length_auto(2755.0), "275,5");
        // kein Tausenderpunkt: sonst wäre „12.500“ beim Wiedereinlesen 12,5
        assert_eq!(Unit::Mm.format_length_auto(12500.0), "12500");
        // mehr als Einheitenstellenzahl nur bei Bedarf
        assert_eq!(Unit::M.format_length_auto(240.5), "0,2405");
        // negative Werte
        assert_eq!(Unit::M.format_length_auto(-1200.0), "-1,20");
        assert_eq!(Unit::Mm.format_length_auto(-0.5), "-0,5");
    }

    #[test]
    fn elev() {
        assert_eq!(elevation_text(0.0), "±0,00");
        assert_eq!(elevation_text(3500.0), "+3,50");
        assert_eq!(elevation_text(-1200.0), "-1,20");
    }
}

#[cfg(test)]
mod zahlentests {
    use super::*;

    #[test]
    fn komma_und_punkt_werden_akzeptiert() {
        assert_eq!(parse_length("0,5 m", Unit::M), Some(500.0));
        assert_eq!(parse_length("0.5 m", Unit::M), Some(500.0));
        assert_eq!(parse_length("3,25", Unit::M), Some(3250.0));
        assert_eq!(parse_length("3.25", Unit::M), Some(3250.0));
        assert_eq!(parse_length("2,40 m", Unit::M), Some(2400.0));
        assert_eq!(parse_length("2.40 m", Unit::M), Some(2400.0));
    }
}
