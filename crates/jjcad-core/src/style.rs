//! Darstellungsstile: Farben, Linientypen, Bemaßungsstil, Papierformate.

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

// ---------------------------------------------------------------------------
// Farbe
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Color = Color::rgb(28, 31, 36);
    pub const WHITE: Color = Color::rgb(250, 250, 247);
    pub const GREY: Color = Color::rgb(140, 145, 152);
    pub const LIGHT_GREY: Color = Color::rgb(205, 208, 212);
    pub const RED: Color = Color::rgb(200, 60, 55);
    pub const BLUE: Color = Color::rgb(45, 105, 190);
    pub const GREEN: Color = Color::rgb(60, 140, 95);
    pub const ORANGE: Color = Color::rgb(214, 138, 45);
    pub const PURPLE: Color = Color::rgb(130, 90, 170);
    pub const CYAN: Color = Color::rgb(50, 150, 160);

    pub fn from_hex(s: &str) -> Option<Color> {
        let t = s.trim().trim_start_matches('#');
        if t.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&t[0..2], 16).ok()?;
        let g = u8::from_str_radix(&t[2..4], 16).ok()?;
        let b = u8::from_str_radix(&t[4..6], 16).ok()?;
        Some(Color { r, g, b })
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn mix(self, o: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
        Color {
            r: f(self.r, o.r),
            g: f(self.g, o.g),
            b: f(self.b, o.b),
        }
    }

    pub fn luminance(self) -> f64 {
        (0.2126 * self.r as f64 + 0.7152 * self.g as f64 + 0.0722 * self.b as f64) / 255.0
    }

    /// Gut lesbare Textfarbe auf dieser Hintergrundfarbe.
    pub fn contrast_text(self) -> Color {
        if self.luminance() > 0.55 {
            Color::BLACK
        } else {
            Color::WHITE
        }
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

struct ColorVisitor;

impl<'de> Visitor<'de> for ColorVisitor {
    type Value = Color;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Farbe als Hex-String (#RRGGBB)")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Color, E> {
        Color::from_hex(v).ok_or_else(|| E::custom(format!("ungültige Farbe: {v}")))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        d.deserialize_str(ColorVisitor)
    }
}

// ---------------------------------------------------------------------------
// Linientyp
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineType {
    #[default]
    Solid,
    Dashed,
    Dotted,
    DashDot,
    Hidden,
}

impl LineType {
    pub const ALL: [LineType; 5] = [
        LineType::Solid,
        LineType::Dashed,
        LineType::Dotted,
        LineType::DashDot,
        LineType::Hidden,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LineType::Solid => "Durchgezogen",
            LineType::Dashed => "Gestrichelt",
            LineType::Dotted => "Gepunktet",
            LineType::DashDot => "Strichpunkt",
            LineType::Hidden => "Verdeckt",
        }
    }

    /// Muster in Papier-Millimetern (Strich, Lücke, …). Leer = durchgezogen.
    pub fn pattern(self) -> &'static [f64] {
        match self {
            LineType::Solid => &[],
            LineType::Dashed => &[4.0, 2.0],
            LineType::Dotted => &[0.4, 1.4],
            LineType::DashDot => &[6.0, 1.6, 0.4, 1.6],
            LineType::Hidden => &[2.0, 1.2],
        }
    }
}

// ---------------------------------------------------------------------------
// Bemaßungsstil (DIN 406 / DIN 1356)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ArrowKind {
    /// 45°-Schrägstrich (DIN 1356, Bauzeichnung)
    #[default]
    Slash,
    /// Gefüllte Pfeilspitze (DIN 406)
    Arrow,
    /// Punkt
    Dot,
    /// Ohne Markierung (nur Maßlinie)
    None,
}

impl ArrowKind {
    pub const ALL: [ArrowKind; 4] = [
        ArrowKind::Slash,
        ArrowKind::Arrow,
        ArrowKind::Dot,
        ArrowKind::None,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ArrowKind::Slash => "Schrägstrich (DIN 1356)",
            ArrowKind::Arrow => "Pfeil (DIN 406)",
            ArrowKind::Dot => "Punkt",
            ArrowKind::None => "Ohne",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DimTextPos {
    /// Über der Maßlinie (DIN-Standard)
    #[default]
    Above,
    /// In die Maßlinie eingeschrieben (unterbrochen)
    Inline,
}

/// Alle Maße in Papier-Millimetern; der Renderer skaliert mit dem
/// Zeichnungsmaßstab (`annotation_scale`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DimStyle {
    pub text_height: f64,
    pub arrow_size: f64,
    pub arrow: ArrowKind,
    /// Abstand Maßlinie → Maßtext.
    pub text_gap: f64,
    /// Abstand Bauteil → Beginn Hilfslinie.
    pub ext_gap: f64,
    /// Überstand Hilfslinie über Maßlinie.
    pub ext_overshoot: f64,
    /// Abstand zwischen zwei Maßketten (automatisch).
    pub chain_gap: f64,
    pub text_pos: DimTextPos,
    /// Nachkommastellen der Maßzahl.
    pub decimals: usize,
    /// Maßzahl in cm statt m anzeigen (Bauzeichnung üblich).
    pub unit_divisor: f64,
}

impl Default for DimStyle {
    fn default() -> Self {
        Self {
            text_height: 2.5,
            arrow_size: 2.0,
            arrow: ArrowKind::Slash,
            text_gap: 0.8,
            ext_gap: 1.5,
            ext_overshoot: 1.5,
            chain_gap: 8.0,
            text_pos: DimTextPos::Above,
            decimals: 2,
            unit_divisor: 1000.0, // Meter
        }
    }
}

// ---------------------------------------------------------------------------
// Schraffur
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HatchPattern {
    #[default]
    Solid,
    Lines,
    Cross,
    Dots,
    /// Dämmung: Mäanderlinie (Mineralwolle) — typisch für Bauzeichnungen
    Insulation,
}

impl HatchPattern {
    pub const ALL: [HatchPattern; 5] = [
        HatchPattern::Solid,
        HatchPattern::Lines,
        HatchPattern::Cross,
        HatchPattern::Dots,
        HatchPattern::Insulation,
    ];
    pub fn label(self) -> &'static str {
        match self {
            HatchPattern::Solid => "Vollfläche",
            HatchPattern::Lines => "Schraffur 45°",
            HatchPattern::Cross => "Kreuzschraffur",
            HatchPattern::Dots => "Punktraster",
            HatchPattern::Insulation => "Dämmung",
        }
    }
}

// ---------------------------------------------------------------------------
// Papier
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PaperSize {
    A4,
    #[default]
    A3,
    A2,
    A1,
    A0,
}

impl PaperSize {
    pub const ALL: [PaperSize; 5] = [
        PaperSize::A4,
        PaperSize::A3,
        PaperSize::A2,
        PaperSize::A1,
        PaperSize::A0,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::A3 => "A3",
            PaperSize::A2 => "A2",
            PaperSize::A1 => "A1",
            PaperSize::A0 => "A0",
        }
    }

    /// Maße in mm (Hochformat).
    pub fn size(self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A2 => (420.0, 594.0),
            PaperSize::A1 => (594.0, 841.0),
            PaperSize::A0 => (841.0, 1189.0),
        }
    }

    pub fn dims(self, landscape: bool) -> (f64, f64) {
        let (w, h) = self.size();
        if landscape {
            (h, w)
        } else {
            (w, h)
        }
    }
}

// ---------------------------------------------------------------------------
// Layer-Standardfarben (Architektur)
// ---------------------------------------------------------------------------

pub const LAYER_DEFAULTS: &[(&str, Color, f64, LineType)] = &[
    ("Wände", Color::rgb(35, 38, 44), 0.50, LineType::Solid),
    ("Türen", Color::rgb(120, 85, 55), 0.25, LineType::Solid),
    ("Fenster", Color::rgb(45, 105, 190), 0.25, LineType::Solid),
    ("Treppen", Color::rgb(60, 140, 95), 0.25, LineType::Solid),
    ("Bemaßung", Color::rgb(190, 70, 60), 0.18, LineType::Solid),
    ("Text", Color::rgb(40, 45, 52), 0.18, LineType::Solid),
    ("Schraffur", Color::rgb(150, 155, 160), 0.13, LineType::Solid),
    ("Möbel", Color::rgb(110, 115, 122), 0.18, LineType::Solid),
    ("Achsen", Color::rgb(200, 120, 40), 0.18, LineType::DashDot),
    ("Hilfslinien", Color::rgb(150, 160, 175), 0.13, LineType::Dashed),
];

// ---------------------------------------------------------------------------
// Wandausrichtung
// ---------------------------------------------------------------------------

/// Lage des Wandkörpers zur Achse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WallAlign {
    /// Achse links, Körper rechts davon
    Left,
    /// Achse mittig (Standard)
    #[default]
    Center,
    /// Achse rechts, Körper links davon
    Right,
}

impl WallAlign {
    pub const ALL: [WallAlign; 3] = [WallAlign::Left, WallAlign::Center, WallAlign::Right];
    pub fn label(self) -> &'static str {
        match self {
            WallAlign::Left => "Links",
            WallAlign::Center => "Mittig",
            WallAlign::Right => "Rechts",
        }
    }
    /// Versatz des Körpers in Richtung der Wandnormalen (in halben Dicken)
    pub fn offset(self) -> f64 {
        match self {
            WallAlign::Left => 1.0,
            WallAlign::Center => 0.0,
            WallAlign::Right => -1.0,
        }
    }
}
