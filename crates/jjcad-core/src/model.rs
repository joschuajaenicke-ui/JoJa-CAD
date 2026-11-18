//! Dokumentmodell: Zeichnung, Layer, Entitäten, Blöcke, Blätter.

use crate::geom::*;
use crate::style::*;
use crate::units::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Id = u64;

pub const FORMAT_NAME: &str = "jjcad";
pub const FORMAT_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------

/// Ein **Plan** ist die oberste und einzige Gliederungsebene des Layer-Managers
/// (T0202): Layers gehören zu genau einem Plan. Pläne sortieren Geschosse,
/// Schnitte, Ansichten und Lagepläne; sie lassen sich als Ganzes ausblenden,
/// sperren und im Panel einklappen.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub id: Id,
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    /// Einklappzustand im Layer-Manager — reine Anzeige, gehört aber zum
    /// Dokument, damit er beim Speichern erhalten bleibt.
    #[serde(default)]
    pub collapsed: bool,
}

impl Plan {
    pub fn new(id: Id, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            visible: true,
            locked: false,
            collapsed: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    pub color: Color,
    /// Plan, zu dem der Layer gehört (T0202). Ältere Dateien tragen 0; beim
    /// Laden ordnet `persist::normalize_plaene` sie dem ersten Plan zu.
    #[serde(default)]
    pub plan: Id,
    /// Linienstärke in mm (Plot).
    #[serde(default = "default_lw")]
    pub line_weight: f64,
    #[serde(default)]
    pub line_type: LineType,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_true")]
    pub printable: bool,
}

fn default_lw() -> f64 {
    0.25
}
fn default_true() -> bool {
    true
}

impl Layer {
    pub fn new(id: Id, name: &str, color: Color) -> Self {
        Self {
            id,
            name: name.to_string(),
            color,
            plan: 0,
            line_weight: 0.25,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            printable: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Entitäten
// ---------------------------------------------------------------------------

/// Eingabeweise einer Maßkette — **keine** Eigenschaft des Objekts (T0166).
///
/// Eine Maßkette hat eine Lage (`pts`, `line_pos`) und eine **Orientierung**
/// (`Dim::angle`, Bogenmaß). Waagerecht, senkrecht und parallel sind nur drei
/// bequeme Winkel derselben Sache: 0°, 90° und die Richtung der ersten
/// Messstrecke. Gespeichert wird immer der Winkel, damit sich jede Kette
/// beliebig drehen lässt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DimAxis {
    #[default]
    /// Waagerecht gemessen (0°)
    X,
    /// Senkrecht gemessen (90°)
    Y,
    /// Parallel zur Messstrecke
    Aligned,
}

impl DimAxis {
    pub const ALL: [DimAxis; 3] = [DimAxis::X, DimAxis::Y, DimAxis::Aligned];

    /// Richtungswinkel der Maßlinie in Bogenmaß (0 = waagerecht, π/2 =
    /// senkrecht). `Aligned` nimmt die Richtung der ersten Messstrecke; ohne
    /// zwei verschiedene Punkte gibt es keine Richtung (`None`).
    pub fn winkel(self, pts: &[Vec2]) -> Option<f64> {
        match self {
            DimAxis::X => Some(0.0),
            DimAxis::Y => Some(std::f64::consts::FRAC_PI_2),
            DimAxis::Aligned => {
                let a = *pts.first()?;
                let b = *pts.get(1)?;
                let d = b - a;
                if d.len() < 1e-9 {
                    None
                } else {
                    Some(d.angle())
                }
            }
        }
    }

    /// Nächstgelegene Eingabeweise zu einem gespeicherten Winkel — für die
    /// Anzeige im Eigenschaften-Panel. Alles, was nicht waagerecht oder
    /// senkrecht liegt, heißt „Parallel“ (die Kette läuft dann schräg).
    pub fn aus_winkel(angle: f64) -> DimAxis {
        let a = angle.rem_euclid(std::f64::consts::PI);
        let grad = a.to_degrees();
        if grad < 0.5 || grad > 179.5 {
            DimAxis::X
        } else if (grad - 90.0).abs() < 0.5 {
            DimAxis::Y
        } else {
            DimAxis::Aligned
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            DimAxis::X => "Waagerecht",
            DimAxis::Y => "Senkrecht",
            DimAxis::Aligned => "Parallel",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            DimAxis::X => crate::i18n::tr("ui.dimaxis.x"),
            DimAxis::Y => crate::i18n::tr("ui.dimaxis.y"),
            DimAxis::Aligned => crate::i18n::tr("ui.dimaxis.aligned"),
        }
    }


    /// Achse einer Maßkette aus der Dominanz der Punktdifferenz bestimmen.
    ///
    /// Maßgeblich ist die über alle Segmente zurückgelegte Strecke je Richtung;
    /// eine Achse zählt nur, wenn sie an **jedem** Segment einen Maßtext größer
    /// als null ergibt. `None` heißt: die Kette läuft gemischt (z. B. erst
    /// senkrecht, dann waagerecht) — dann muss der Aufrufer `axis` angeben,
    /// sonst entstünde bei einem Segment der Maßtext „0,0“ (T0038).
    pub fn aus_kette(pts: &[Vec2]) -> Option<DimAxis> {
        let mut dx = 0.0;
        let mut dy = 0.0;
        for w in pts.windows(2) {
            dx += (w[1].x - w[0].x).abs();
            dy += (w[1].y - w[0].y).abs();
        }
        // Achse mit der größeren Strecke zuerst probieren.
        let kandidaten = if dx >= dy {
            [DimAxis::X, DimAxis::Y]
        } else {
            [DimAxis::Y, DimAxis::X]
        };
        kandidaten.into_iter().find(|achse| {
            pts.windows(2).all(|w| {
                let d = w[1] - w[0];
                let len = d.len();
                if len < 1e-9 {
                    return true;
                }
                // Ein Anteil unter 1/1000 der Segmentlänge ist Rauschen
                // (Handklick), kein Beitrag zum Maßtext.
                let m = match achse {
                    DimAxis::X => d.x.abs(),
                    DimAxis::Y => d.y.abs(),
                    DimAxis::Aligned => len,
                };
                m > len * 1e-3
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DoorKind {
    #[default]
    Hinge,
    Sliding,
    Double,
    Opening,
}

impl DoorKind {
    pub const ALL: [DoorKind; 4] = [
        DoorKind::Hinge,
        DoorKind::Sliding,
        DoorKind::Double,
        DoorKind::Opening,
    ];
    pub fn label(self) -> &'static str {
        match self {
            DoorKind::Hinge => "Drehtür",
            DoorKind::Sliding => "Schiebetür",
            DoorKind::Double => "Zweiflügelig",
            DoorKind::Opening => "Öffnung",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            DoorKind::Hinge => crate::i18n::tr("ui.doorkind.hinge"),
            DoorKind::Sliding => crate::i18n::tr("ui.doorkind.sliding"),
            DoorKind::Double => crate::i18n::tr("ui.doorkind.double"),
            DoorKind::Opening => crate::i18n::tr("ui.doorkind.opening"),
        }
    }

}

/// Lage des Höhenwertes an der Grundriss-Kote (T0196).
///
/// Der Wert gehört zum Symbol und steht normalerweise **oben** darüber. Je
/// nach Platz in der Zeichnung lässt er sich auch unten, links oder rechts
/// daneben setzen; die Lage im Symbol selbst (Kreis, Fadenkreuz) ändert sich
/// dabei nicht.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ElevationTextPos {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl ElevationTextPos {
    pub const ALL: [ElevationTextPos; 4] = [
        ElevationTextPos::Top,
        ElevationTextPos::Bottom,
        ElevationTextPos::Left,
        ElevationTextPos::Right,
    ];
    /// Deutscher Name — Dateien und Action-Parameter benutzen `key()`.
    pub fn label(self) -> &'static str {
        match self {
            ElevationTextPos::Top => "Oben",
            ElevationTextPos::Bottom => "Unten",
            ElevationTextPos::Left => "Links",
            ElevationTextPos::Right => "Rechts",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106).
    pub fn text(self) -> &'static str {
        match self {
            ElevationTextPos::Top => crate::i18n::tr("ui.elevationtextpos.top"),
            ElevationTextPos::Bottom => crate::i18n::tr("ui.elevationtextpos.bottom"),
            ElevationTextPos::Left => crate::i18n::tr("ui.elevationtextpos.left"),
            ElevationTextPos::Right => crate::i18n::tr("ui.elevationtextpos.right"),
        }
    }
    /// Schlüssel in der Datei und in der Befehlsschicht.
    pub fn key(self) -> &'static str {
        match self {
            ElevationTextPos::Top => "top",
            ElevationTextPos::Bottom => "bottom",
            ElevationTextPos::Left => "left",
            ElevationTextPos::Right => "right",
        }
    }
    /// Aus dem Text der Befehlsschicht — auch die deutschen Namen.
    pub fn from_text(t: &str) -> Option<ElevationTextPos> {
        match t.trim().to_lowercase().as_str() {
            "top" | "oben" => Some(ElevationTextPos::Top),
            "bottom" | "unten" => Some(ElevationTextPos::Bottom),
            "left" | "links" => Some(ElevationTextPos::Left),
            "right" | "rechts" => Some(ElevationTextPos::Right),
            _ => None,
        }
    }
}

/// Lage des Basispunktes einer Tür längs ihrer Breite (T0091).
///
/// Der Basispunkt ist der Punkt, an dem eine **neue** Tür am Mauszeiger
/// hängt: „Links" setzt das linke Türblattende (in Wandrichtung) an den
/// Klickpunkt, „Mitte" die Türmitte, „Rechts" das rechte Ende. Vorgabe ist
/// „Links" — die Tür wächst vom Klickpunkt aus nach rechts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DoorBase {
    #[default]
    Left,
    Center,
    Right,
}

impl DoorBase {
    pub const ALL: [DoorBase; 3] = [DoorBase::Left, DoorBase::Center, DoorBase::Right];
    pub fn label(self) -> &'static str {
        match self {
            DoorBase::Left => "Links",
            DoorBase::Center => "Mitte",
            DoorBase::Right => "Rechts",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            DoorBase::Left => crate::i18n::tr("ui.doorbase.left"),
            DoorBase::Center => crate::i18n::tr("ui.doorbase.center"),
            DoorBase::Right => crate::i18n::tr("ui.doorbase.right"),
        }
    }

    /// Versatz des Basispunktes vom Türzentrum, in Türbreiten.
    pub fn offset(self) -> f64 {
        match self {
            DoorBase::Left => -0.5,
            DoorBase::Center => 0.0,
            DoorBase::Right => 0.5,
        }
    }
    /// Schlüssel für die Befehlsschicht (`base`).
    pub fn key(self) -> &'static str {
        match self {
            DoorBase::Left => "left",
            DoorBase::Center => "center",
            DoorBase::Right => "right",
        }
    }
    /// Aus dem Text der Befehlsschicht (`base`).
    pub fn from_text(t: &str) -> Option<DoorBase> {
        match t.trim().to_lowercase().as_str() {
            "left" | "links" => Some(DoorBase::Left),
            "center" | "centre" | "mitte" | "mittig" => Some(DoorBase::Center),
            "right" | "rechts" => Some(DoorBase::Right),
            _ => None,
        }
    }
}

/// Schriftart eines Textes.
///
/// Die **Gebrauchsschriften** kommen aus dem System (Liberation/Quicksand) —
/// das hält das Programm klein, und der PDF-Export bettet nur die tatsächlich
/// benutzte Schrift ein.
///
/// Die **technischen Schriften** DIN 1451 und EN ISO 3098 gibt es auf keinem
/// üblichen Rechner; sie liegen deshalb mit im Programm
/// (`crates/jjcad-core/fonts/`, OFL bzw. LGPL mit Schriftausnahme) und werden
/// über `include_bytes!` geladen — siehe [`crate::fonts`].
///
/// `explodierbar` ist eine Eigenschaft der Familie: konturierte Gebrauchs-
/// schriften lassen sich sauber in Linien zerlegen, runde Zierschriften
/// ergeben dabei sehr viele kleine Striche und sind deshalb ausgenommen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TextFont {
    /// Serifenlos (Liberation Sans) — metrisch wie Helvetica
    #[default]
    #[serde(rename = "sans")]
    Sans,
    /// Serifen (Liberation Serif)
    #[serde(rename = "serif")]
    Serif,
    /// Nichtproportional (Liberation Mono)
    #[serde(rename = "mono")]
    Mono,
    /// Schmal (Liberation Sans Narrow) — **nicht** die DIN 1451: die schmale
    /// Gebrauchsschrift des Systems, ohne Bezug zur technischen Normschrift
    #[serde(rename = "narrow")]
    Narrow,
    /// Runde Zierschrift (Quicksand) — wird **nicht** in Linien zerlegt
    #[serde(rename = "rund")]
    Rund,
    /// DIN 1451 Engschrift (mitgeliefert: D-DIN Condensed)
    #[serde(rename = "din_eng")]
    DinEng,
    /// DIN 1451 Mittelschrift (mitgeliefert: D-DIN)
    #[serde(rename = "din_mittel")]
    DinMittel,
    /// DIN 1451 Breitschrift (mitgeliefert: D-DIN Expanded)
    #[serde(rename = "din_breit")]
    DinBreit,
    /// EN ISO 3098 Typ A — dünne technische Schrift, Strichstärke d = h/14
    /// (mitgeliefert: osifont)
    #[serde(rename = "iso_a")]
    IsoA,
    /// EN ISO 3098 Typ B — Strichstärke d = h/10; formgleich mit der
    /// DIN-1451-Mittelschrift (mitgeliefert: D-DIN)
    #[serde(rename = "iso_b")]
    IsoB,
}

impl TextFont {
    pub const ALL: [TextFont; 10] = [
        TextFont::Sans,
        TextFont::Serif,
        TextFont::Mono,
        TextFont::Narrow,
        TextFont::Rund,
        TextFont::DinEng,
        TextFont::DinMittel,
        TextFont::DinBreit,
        TextFont::IsoA,
        TextFont::IsoB,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TextFont::Sans => "Serifenlos",
            TextFont::Serif => "Serifen",
            TextFont::Mono => "Nichtproportional",
            TextFont::Narrow => "Schmal",
            TextFont::Rund => "Rund",
            TextFont::DinEng => "DIN 1451 Engschrift",
            TextFont::DinMittel => "DIN 1451 Mittelschrift",
            TextFont::DinBreit => "DIN 1451 Breitschrift",
            TextFont::IsoA => "EN ISO 3098 Typ A",
            TextFont::IsoB => "EN ISO 3098 Typ B",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            TextFont::Sans => crate::i18n::tr("ui.textfont.sans"),
            TextFont::Serif => crate::i18n::tr("ui.textfont.serif"),
            TextFont::Mono => crate::i18n::tr("ui.textfont.mono"),
            TextFont::Narrow => crate::i18n::tr("ui.textfont.narrow"),
            TextFont::Rund => crate::i18n::tr("ui.textfont.rund"),
            TextFont::DinEng => crate::i18n::tr("ui.textfont.din_eng"),
            TextFont::DinMittel => crate::i18n::tr("ui.textfont.din_mittel"),
            TextFont::DinBreit => crate::i18n::tr("ui.textfont.din_breit"),
            TextFont::IsoA => crate::i18n::tr("ui.textfont.iso_a"),
            TextFont::IsoB => crate::i18n::tr("ui.textfont.iso_b"),
        }
    }


    pub fn key(self) -> &'static str {
        match self {
            TextFont::Sans => "sans",
            TextFont::Serif => "serif",
            TextFont::Mono => "mono",
            TextFont::Narrow => "narrow",
            TextFont::Rund => "rund",
            TextFont::DinEng => "din_eng",
            TextFont::DinMittel => "din_mittel",
            TextFont::DinBreit => "din_breit",
            TextFont::IsoA => "iso_a",
            TextFont::IsoB => "iso_b",
        }
    }

    /// Familie zu einem Schlüssel („din_eng") **oder** zu ihrer Bezeichnung
    /// („DIN 1451 Engschrift") — MCP und PDF-Import reichen beides durch.
    pub fn from_key(s: &str) -> Option<TextFont> {
        let roh = s.trim();
        if let Some(f) = TextFont::ALL
            .into_iter()
            .find(|f| f.key().eq_ignore_ascii_case(roh))
        {
            return Some(f);
        }
        let norm = |t: &str| -> String {
            t.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect()
        };
        let gesucht = norm(roh);
        TextFont::ALL
            .into_iter()
            .find(|f| norm(f.label()) == gesucht)
    }

    pub fn index(self) -> usize {
        match self {
            TextFont::Sans => 0,
            TextFont::Serif => 1,
            TextFont::Mono => 2,
            TextFont::Narrow => 3,
            TextFont::Rund => 4,
            TextFont::DinEng => 5,
            TextFont::DinMittel => 6,
            TextFont::DinBreit => 7,
            TextFont::IsoA => 8,
            TextFont::IsoB => 9,
        }
    }

    /// Lässt sich Text dieser Schrift in Linien zerlegen?
    pub fn explodierbar(self) -> bool {
        !matches!(self, TextFont::Rund)
    }

    /// Kurze Begründung für die Oberfläche.
    pub fn explode_hinweis(self) -> &'static str {
        match self {
            TextFont::Rund => {
                "Runde Zierschrift — Zerlegen ergäbe sehr viele kleine Linien"
            }
            _ => "lässt sich in Linien zerlegen",
        }
    }
}

/// Ein Lauf innerhalb eines Textes: ein Stück Text mit eigener Formatierung.
///
/// Mehrere Läufe in einem Objekt ergeben gemischte Auszeichnung (z. B.
/// „Raum **3,20 m²** mit *Küche*"). Der schlichte Text des Objekts bleibt die
/// Wahrheit für Suche und Bearbeitung; die Läufe beschreiben nur die Form.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub superscript: bool,
}

impl TextRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    /// Ohne jede Auszeichnung? Dann genügt der schlichte Text.
    pub fn is_plain(&self) -> bool {
        !self.bold && !self.italic && !self.underline && !self.superscript
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignH {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignV {
    Top,
    #[default]
    Middle,
    Bottom,
}

impl TextAlignH {
    pub const ALL: [TextAlignH; 3] = [TextAlignH::Left, TextAlignH::Center, TextAlignH::Right];
    /// Name in der Oberfläche (Glossar: Linksbündig / Zentriert / Rechtsbündig)
    pub fn label(self) -> &'static str {
        match self {
            TextAlignH::Left => "Linksbündig",
            TextAlignH::Center => "Zentriert",
            TextAlignH::Right => "Rechtsbündig",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            TextAlignH::Left => crate::i18n::tr("ui.textalignh.left"),
            TextAlignH::Center => crate::i18n::tr("ui.textalignh.center"),
            TextAlignH::Right => crate::i18n::tr("ui.textalignh.right"),
        }
    }

    /// Schlüssel in der Befehlsschicht (Parameter `align`)
    pub fn key(self) -> &'static str {
        match self {
            TextAlignH::Left => "left",
            TextAlignH::Center => "center",
            TextAlignH::Right => "right",
        }
    }
}

impl TextAlignV {
    pub const ALL: [TextAlignV; 3] = [TextAlignV::Top, TextAlignV::Middle, TextAlignV::Bottom];
    /// Name in der Oberfläche (Glossar: Oben / Mitte / Unten)
    pub fn label(self) -> &'static str {
        match self {
            TextAlignV::Top => "Oben",
            TextAlignV::Middle => "Mitte",
            TextAlignV::Bottom => "Unten",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            TextAlignV::Top => crate::i18n::tr("ui.textalignv.top"),
            TextAlignV::Middle => crate::i18n::tr("ui.textalignv.middle"),
            TextAlignV::Bottom => crate::i18n::tr("ui.textalignv.bottom"),
        }
    }

    /// Schlüssel in der Befehlsschicht (Parameter `valign`)
    pub fn key(self) -> &'static str {
        match self {
            TextAlignV::Top => "top",
            TextAlignV::Middle => "middle",
            TextAlignV::Bottom => "bottom",
        }
    }
}

/// Alle Maße in Millimetern, Winkel im Bogenmaß.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityKind {
    Point {
        p: Vec2,
    },
    Line {
        a: Vec2,
        b: Vec2,
    },
    Circle {
        c: Vec2,
        r: f64,
    },
    Arc {
        c: Vec2,
        r: f64,
        a0: f64,
        a1: f64,
    },
    Ellipse {
        c: Vec2,
        rx: f64,
        ry: f64,
        rot: f64,
    },
    /// Abschnitt einer Ellipse — entsteht beim Stutzen einer Ellipse, wie der
    /// Bogen beim Stutzen eines Kreises (T0084).
    ///
    /// `a0`/`a1` sind **Parameterwinkel** im ungedrehten Ellipsenraum (siehe
    /// [`crate::geom::Ellipse`]), keine Polarkoordinaten. Der Bogen läuft von
    /// `a0` gegen den Uhrzeigersinn bis `a1`.
    EllipseArc {
        c: Vec2,
        rx: f64,
        ry: f64,
        rot: f64,
        a0: f64,
        a1: f64,
    },
    Polyline {
        verts: Vec<Vertex>,
        closed: bool,
    },
    /// Glättende Punktfolge (Catmull-Rom) — Eingabe wie die Polylinie, die Kurve
    /// läuft durch alle Stützpunkte. Siehe `geom::Spline`.
    Spline {
        verts: Vec<Vertex>,
        closed: bool,
    },
    /// Füllfläche: Muster + Flächenfarbe + optionale Randlinie.
    ///
    /// Der einzige gefüllte Objekttyp — die frühere Schraffur (Typ
    /// `"hatch"`) wird beim Laden migriert (siehe `persist::from_json`).
    Fill {
        /// Geschlossene Kontur (mm, wie eingegeben).
        boundary: Vec<Vec2>,
        /// Inseln in der Fläche (z. B. Schornstein, Treppe, Pfeiler) — werden
        /// ausgespart, nicht übermalt. Leer bei Flächen ohne Insel und in
        /// Dateien von vor diesem Feld (`serde(default)`).
        #[serde(default)]
        holes: Vec<Vec<Vec2>>,
        #[serde(default)]
        pattern: FillPattern,
        /// Drehung des Musters (Bogenmaß, 0 = waagerecht)
        #[serde(default)]
        pattern_angle: f64,
        /// Musterabstand bzw. Rasterweite in mm
        #[serde(default = "default_pattern_scale")]
        pattern_scale: f64,
        /// Umrandung entlang der Umgrenzung zeichnen (Farbe aus `style.color`).
        /// Vorgabe je Erstellmodus: Punkte setzen an, Kontur anklicken und
        /// Farbeimer aus; im Eigenschaften-Panel umschaltbar.
        #[serde(default)]
        border: bool,
    },
    /// Volumen: dieselbe Fläche wie eine Füllfläche, aber ein **Körper** mit
    /// Höhe — gedacht für Mauerwerk, das sich nicht als Rechteckwand zeichnen
    /// lässt (runde Wand, Pfeiler, Sonderform).
    ///
    /// Optisch verhält es sich wie eine Füllfläche, nur dass es sich mit den
    /// Wänden **verbindet**: seine Umrandung fällt aus, wo sie in einer Wand
    /// liegt, und Wandkanten fallen aus, wo sie im Volumen liegen. Die Fläche
    /// wird standardmäßig im Wandton gefüllt, damit der Übergang nicht als
    /// Farbkante sichtbar wird.
    Volume {
        boundary: Vec<Vec2>,
        #[serde(default)]
        holes: Vec<Vec<Vec2>>,
        #[serde(default)]
        pattern: FillPattern,
        #[serde(default)]
        pattern_angle: f64,
        #[serde(default = "default_pattern_scale")]
        pattern_scale: f64,
        #[serde(default)]
        border: bool,
        /// Höhe des Körpers in mm (Metadaten; der Grundriss zeigt keine Höhe).
        #[serde(default = "default_wall_height")]
        height: f64,
    },
    Text {
        p: Vec2,
        text: String,
        height: f64,
        #[serde(default)]
        angle: f64,
        #[serde(default)]
        align: TextAlignH,
        #[serde(default)]
        valign: TextAlignV,
        #[serde(default)]
        bold: bool,
        /// Fett/Kursiv/Unterstrichen für den **ganzen** Text — gilt, solange
        /// keine Läufe gesetzt sind.
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        underline: bool,
        /// Hochstellung des ganzen Textes (Fußnotenzeichen, Einheit).
        #[serde(default)]
        superscript: bool,
        /// Gemischte Auszeichnung. Leer = die Felder oben gelten für alles.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        runs: Vec<TextRun>,
        /// Schriftart des ganzen Textes (Standard: serifenlos).
        #[serde(default)]
        font: TextFont,
        /// Zeilenabstand als Vielfaches der Zeilenhöhe der Schrift.
        ///
        /// 1 = die Zeilenhöhe der Schrift selbst (Aufstieg, Abstieg,
        /// Durchschuss der Datei). Mehrzeilige Texte bekommen damit je nach
        /// Schriftart verschieden hohe Zeilen; der Rahmen folgt (`render::text_box`).
        #[serde(default = "default_one")]
        line_spacing: f64,
        /// Umrandung dauerhaft anzeigen — derselbe Kasten, den die Bearbeitung
        /// als Auswahlrahmen zeigt (T0126).
        #[serde(default)]
        frame: bool,
        /// Runde Umrandung: Ecken mit dem Radius der halben Schrifthöhe
        /// abgerundet. Einzeilig ergibt das ein Oval aus zwei Halbkreisen.
        #[serde(default)]
        frame_round: bool,
        /// Hintergrundfläche hinter dem Text, in der Farbe dieses Feldes.
        ///
        /// Die Fläche ist immer das **Rechteck** der Umrandung; eine runde
        /// Hintergrundfläche gibt es nicht (T0126).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background: Option<Color>,
    },
    /// Raumstempel: unsichtbare Raumfläche mit verknüpfter Beschriftung.
    ///
    /// Die Umgrenzung ist die Fläche des Raums. Sie wird **nicht** gezeichnet
    /// und ist nicht anklickbar — sichtbar ist allein das Textfeld an `label`
    /// mit der Überschrift `heading` und dem Flächeninhalt. Der Text wird bei
    /// jeder Anzeige aus der Umgrenzung gerechnet (`room_text`), damit er nach
    /// dem Ziehen einer Ecke sofort stimmt. `label` ist von der Fläche
    /// unabhängig: das Textfeld lässt sich verschieben, ohne den Raum zu
    /// bewegen (T0097).
    Room {
        boundary: Vec<Vec2>,
        /// Ankerpunkt des Textfeldes (Mitte des Textes).
        label: Vec2,
        #[serde(default = "default_text_height")]
        height: f64,
        /// Überschrift über dem Flächeninhalt.
        #[serde(default = "default_room_heading")]
        heading: String,
    },
    /// Bemaßung: eine Maßkette über beliebig viele Messpunkte mit gemeinsamer
    /// Maßlinie (T0154).
    ///
    /// Früher gab es zwei Objektarten — das Einzelmaß (`a`, `b`) und die
    /// Maßkette (`pts`). Beide sind jetzt **ein** Element: das Einzelmaß ist
    /// eine Kette aus genau zwei Messpunkten. `pts` trägt immer mindestens
    /// zwei Punkte; die Befehle `dim.point.add`/`dim.point.remove` ändern die
    /// Anzahl, ohne unter zwei zu gehen.
    Dim {
        pts: Vec<Vec2>,
        /// Punkt auf der Maßlinie (bestimmt Lage der Maßlinie).
        line_pos: Vec2,
        /// Orientierung: Richtung der Maßlinie in **Bogenmaß** (0 = waagerecht,
        /// π/2 = senkrecht). Eine Maßkette ist damit ein Objekt mit Lage und
        /// Richtung — waagerecht und senkrecht sind nur zwei Winkel davon, und
        /// Drehen ändert den Winkel mit (T0166).
        #[serde(default)]
        angle: f64,
        /// Maßtext des **ersten** Segments überschreiben (z. B. "2,50").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        override_text: Option<String>,
    },
    /// Grundriss-Kote: Kreis-Symbol mit verknüpftem Höhenwert daneben.
    ///
    /// Der Kreismittelpunkt sitzt auf `p`; der Höhenwert steht rechts davon
    /// und gehört zum Symbol (kein eigenes Textfeld). Die Höhe gibt der
    /// Nutzer im Eigenschaften-Panel an.
    #[serde(rename = "elevation_plan", alias = "elevation")]
    ElevationPlan {
        p: Vec2,
        /// Höhe in mm über ±0,00.
        z: f64,
        /// Lage des Höhenwertes am Symbol (T0196), Vorgabe „Oben".
        #[serde(default)]
        text_pos: ElevationTextPos,
    },
    /// Schnitt-Kote: Punktfolge aus Dreieck und Höhenangabe je Punkt.
    ///
    /// Der **erste** Punkt ist die Bezugshöhe ±0,00; alle weiteren Höhen
    /// ergeben sich aus der Y-Differenz zum ersten Punkt (Modell-mm).
    #[serde(rename = "elevation_section")]
    ElevationSection {
        pts: Vec<Vec2>,
    },
    Wall {
        a: Vec2,
        b: Vec2,
        thickness: f64,
        #[serde(default = "default_wall_height")]
        height: f64,
        #[serde(default)]
        material: String,
        /// Schraffur der Wandfläche (None = Vollfläche in der Füllfarbe)
        #[serde(default)]
        hatch: Option<FillPattern>,
        /// Ausrichtung des Wandkörpers zur Achse
        #[serde(default)]
        align: crate::style::WallAlign,
    },
    Door {
        center: Vec2,
        /// Richtung der Wand (Öffnungsrichtung der Wand).
        angle: f64,
        width: f64,
        #[serde(default)]
        kind: DoorKind,
        /// +1 = Anschlag rechts, -1 = links
        #[serde(default = "default_one")]
        hinge: f64,
        /// Öffnet zur positiven Normalen der Wand
        #[serde(default = "default_true")]
        swing_in: bool,
        /// Lage in der Wanddicke (links/mittig/rechts)
        #[serde(default)]
        align: crate::style::WallAlign,
        /// Wand, in der die Tür sitzt (für Öffnungen).
        #[serde(default)]
        host: Option<Id>,
        /// Lage längs der Wand in mm ab Wandanfang — die Tür folgt der Wand.
        #[serde(default)]
        at: Option<f64>,
        /// Lage des Basispunktes längs der Türbreite (T0091).
        #[serde(default)]
        base: DoorBase,
    },
    Window {
        center: Vec2,
        angle: f64,
        width: f64,
        /// Lage in der Wanddicke (links/mittig/rechts)
        #[serde(default)]
        align: crate::style::WallAlign,
        #[serde(default = "default_sill")]
        sill: f64,
        #[serde(default = "default_window_height")]
        height: f64,
        #[serde(default)]
        host: Option<Id>,
        /// Lage längs der Wand in mm ab Wandanfang — das Fenster folgt der Wand.
        #[serde(default)]
        at: Option<f64>,
    },
    Stair {
        a: Vec2,
        b: Vec2,
        width: f64,
        #[serde(default = "default_steps")]
        steps: u32,
        /// Treppenhöhe (Geschosshöhe) in mm — Grundlage der Schrittmaßregel.
        #[serde(default = "default_stair_height")]
        height: f64,
        /// Stufenzahl folgt der Schrittmaßregel aus Länge und Höhe
        /// (`false` = vom Nutzer überschrieben). Ältere Dateien ohne dieses
        /// Feld behalten ihre gespeicherte Stufenzahl.
        #[serde(default)]
        auto_steps: bool,
        /// Treppenschnitt (zwei Diagonalen) zeichnen. Er symbolisiert **zwei
        /// Läufe übereinander**; ohne Schnitt ist es eine einzelne Treppe.
        #[serde(default = "default_true")]
        cut: bool,
        /// Linien des unteren Teils (vor dem Schnitt). `None` = Objektstil.
        #[serde(default)]
        unten: Option<EntityStyle>,
        /// Linien des oberen Teils (hinter dem Schnitt). `None` = Objektstil.
        #[serde(default)]
        oben: Option<EntityStyle>,
    },
    BlockRef {
        block: String,
        pos: Vec2,
        #[serde(default)]
        rot: f64,
        #[serde(default = "default_scale")]
        scale: Vec2,
        #[serde(default)]
        mirror: bool,
    },
    /// Eingebettetes Bild (PNG oder JPEG).
    ///
    /// `a`→`b` ist die **untere Kante**: Anfangspunkt, Richtung (Drehung) und
    /// Länge (Breite) stecken darin. Die Höhe folgt aus dem Seitenverhältnis
    /// der Datei und der Breite — solange `lock_ratio` an ist (Vorgabe). Ist
    /// die Sperre aus, gilt die gespeicherte `height` (mm).
    ///
    /// Die Bilddaten liegen als Base64 im Dokument, damit die Zeichnung ohne
    /// die ursprüngliche Bilddatei gilt. `name` ist nur die Anzeige.
    Image {
        a: Vec2,
        b: Vec2,
        /// Höhe in mm — nur wirksam, wenn `lock_ratio` aus ist.
        #[serde(default)]
        height: f64,
        /// Seitenverhältnis der Datei erzwingen (Vorgabe: an).
        #[serde(default = "default_true")]
        lock_ratio: bool,
        #[serde(default)]
        name: String,
        #[serde(default)]
        data: String,
    },
    /// Ansichtsfenster im Papierraum (T0109).
    ///
    /// Bis dahin lag es in einer eigenen Liste (`Sheet::viewports`) mit
    /// eigenem Auswahlfeld, eigenem Verschieben und ohne Griffe — es ließ sich
    /// weder über ein Auswahlfenster wählen noch löschen. Jetzt ist es ein
    /// **normales Blattobjekt**: Rechteck in Papier-Millimetern, vier
    /// Eckgriffe, verschiebbar und löschbar wie jede Linie. Der Inhalt
    /// (die Zeichnung im Maßstab) wird weiterhin aus dem Modell gerendert.
    Viewport(Viewport),
}

// ---------------------------------------------------------------------------
// Zeichenrang (T0048)
// ---------------------------------------------------------------------------
//
// Die Liste `entities` ist die Zeichenreihenfolge: Index 0 liegt ganz hinten.
// Damit ein neu eingefügtes Element ohne Zutun des Nutzers richtig liegt, hat
// jede Objektart einen **Rang**. Der Rang ordnet nur die Klassen, nicht die
// einzelnen Objekte: innerhalb einer Klasse bleibt die Reihenfolge die des
// Nutzers — auch eine von Hand verdrehte Zeichnung wird also nicht umsortiert,
// es wird nur der Platz des neuen Elements bestimmt.
//
// Die Regel ist bewusst einfach und ohne Geometrieprüfung:
//   Fläche < Volumen < Architekturbauteil < freie Linie < Beschriftung
// Flächen liegen damit hinter allem, Linien über Wänden; Text und Bemaßung
// sind vorn. Das gilt auch dann, wenn der Nutzer die Reihenfolge vorher
// durcheinandergebracht hat: das neue Element ordnet sich an der Grenze seiner
// Klasse ein (siehe `Document::einfuege_index`), bestehende Objekte rücken nur.

/// Rang der Fläche (Füllfläche, Schraffur): ganz hinten.
pub const RANG_FLAECHE: u8 = 0;
/// Rang des Volumens: Fläche mit Höhe, gehört über die Füllflächen.
pub const RANG_VOLUMEN: u8 = 1;
/// Rang der Architekturbauteile (Wand, Tür, Fenster, Treppe, Block).
pub const RANG_BAUTEIL: u8 = 2;
/// Rang der freien Linien (Punkt, Linie, Polylinie, Spline, Rechteck, Kreis,
/// Bogen, Ellipse) — sie sollen über Wänden liegen.
pub const RANG_LINIE: u8 = 3;
/// Rang der Beschriftung (Text, Maß, Höhenkote): immer vorn.
pub const RANG_BESCHRIFTUNG: u8 = 4;

impl EntityKind {
    /// Zeichenrang dieser Objektart (siehe Konstanten `RANG_*`).
    pub fn zeichenrang(&self) -> u8 {
        match self {
            EntityKind::Fill { .. } => RANG_FLAECHE,
            // Ein Bild ist ein Hintergrund: es liegt hinter Flächen und Linien
            // (eingescannte Pläne, Lagepläne), wie eine Füllfläche.
            EntityKind::Image { .. } => RANG_FLAECHE,
            // Ein Ansichtsfenster ist der Hintergrund des Blatts: es liegt
            // hinter Rahmen, Schriftfeld und allem, was darauf gezeichnet ist.
            EntityKind::Viewport(_) => RANG_FLAECHE,
            EntityKind::Volume { .. } => RANG_VOLUMEN,
            EntityKind::Wall { .. }
            | EntityKind::Door { .. }
            | EntityKind::Window { .. }
            | EntityKind::Stair { .. }
            | EntityKind::BlockRef { .. } => RANG_BAUTEIL,
            EntityKind::Point { .. }
            | EntityKind::Line { .. }
            | EntityKind::Circle { .. }
            | EntityKind::Arc { .. }
            | EntityKind::Ellipse { .. }
            | EntityKind::EllipseArc { .. }
            | EntityKind::Polyline { .. }
            | EntityKind::Spline { .. } => RANG_LINIE,
            EntityKind::Text { .. }
            | EntityKind::Room { .. }
            | EntityKind::Dim { .. }
            | EntityKind::ElevationPlan { .. }
            | EntityKind::ElevationSection { .. } => RANG_BESCHRIFTUNG,
        }
    }
}

impl Entity {
    /// Zeichenrang dieses Objekts (siehe `EntityKind::zeichenrang`).
    pub fn zeichenrang(&self) -> u8 {
        self.kind.zeichenrang()
    }
}

fn default_room_heading() -> String {
    "Zimmer".to_string()
}
fn default_pattern_scale() -> f64 {
    100.0
}
fn default_wall_height() -> f64 {
    2750.0
}
fn default_one() -> f64 {
    1.0
}
fn default_sill() -> f64 {
    900.0
}
fn default_window_height() -> f64 {
    1250.0
}
fn default_steps() -> u32 {
    16
}
fn default_stair_height() -> f64 {
    crate::stair::HOEHE_VORGABE_MM
}
fn default_scale() -> Vec2 {
    v(1.0, 1.0)
}

impl EntityKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            EntityKind::Point { .. } => "Punkt",
            EntityKind::Line { .. } => "Linie",
            EntityKind::Circle { .. } => "Kreis",
            EntityKind::Arc { .. } => "Bogen",
            EntityKind::Ellipse { .. } => "Ellipse",
            EntityKind::EllipseArc { .. } => "Ellipsenbogen",
            EntityKind::Polyline { .. } => "Polylinie",
            EntityKind::Spline { .. } => "Spline",
            EntityKind::Fill { .. } => "Füllfläche",
            EntityKind::Volume { .. } => "Volumen",
            EntityKind::Text { .. } => "Text",
            EntityKind::Room { .. } => "Raumstempel",
            EntityKind::Dim { .. } => "Maßkette",
            EntityKind::ElevationPlan { .. } => "Grundriss-Kote",
            EntityKind::ElevationSection { .. } => "Schnitt-Kote",
            EntityKind::Wall { .. } => "Wand",
            EntityKind::Door { .. } => "Tür",
            EntityKind::Window { .. } => "Fenster",
            EntityKind::Stair { .. } => "Treppe",
            EntityKind::BlockRef { .. } => "Block",
            EntityKind::Image { .. } => "Bild",
            EntityKind::Viewport(_) => "Ansichtsfenster",
        }
    }

    pub fn is_architecture(&self) -> bool {
        matches!(
            self,
            EntityKind::Wall { .. }
                | EntityKind::Door { .. }
                | EntityKind::Window { .. }
                | EntityKind::Stair { .. }
                | EntityKind::Volume { .. }
        )
    }

    pub fn is_dimension(&self) -> bool {
        matches!(
            self,
            EntityKind::Dim { .. }
                | EntityKind::ElevationPlan { .. }
                | EntityKind::ElevationSection { .. }
        )
    }
}

/// Beschriftung eines Raumstempels: Überschrift und Flächeninhalt.
///
/// Der Text ist **abgeleitet**, nie gespeichert: so stimmt er nach jeder
/// Änderung der Umgrenzung ohne Nachziehen. Eine Quelle für Anzeige,
/// Eigenschaften-Panel und Prüfungen — wer den Text woanders zusammenbaut,
/// zeigt bei der nächsten Änderung etwas anderes an.
pub fn room_text(
    heading: &str,
    boundary: &[Vec2],
    unit: crate::units::Unit,
    decimals: usize,
) -> String {
    let flaeche = crate::area::area_of(boundary, &[]);
    let wert = unit.format_area(flaeche, decimals);
    let kopf = heading.trim();
    if kopf.is_empty() {
        wert
    } else {
        format!("{kopf}\n{wert}")
    }
}

/// Weltkoordinaten-Epsilon für „derselbe Punkt" (Millimeter).
///
/// Ein Schlusspunkt innerhalb dieser Strecke vom Startpunkt gilt als Startpunkt:
/// die Polylinie ist dann geschlossen. Der Wert ist bewusst klein — er fängt
/// Rundungsreste aus der Tippeingabe ab, nicht eine bewusst gesetzte Kurzstrecke.
pub const POINT_EPS_MM: f64 = 0.5;

/// Fertige Punktfolge für einen Polygonzug.
///
/// Doppelte Stützpunkte fallen weg (zwei Klicks auf dieselbe Stelle erzeugen
/// sonst eine Strecke der Länge null). Liegt der Schlusspunkt innerhalb von
/// `tol` auf dem Startpunkt, wird er nicht gespeichert: der Zug gilt als
/// geschlossen und `closed` wird gesetzt — der doppelte Endpunkt existiert im
/// Modell nicht. Offene Züge bleiben unverändert offen; unter drei Punkten
/// umschließt nichts eine Fläche, dann bleibt der Zug offen.
pub fn polyline_points(pts: &[Vec2], closed: bool, tol: f64) -> (Vec<Vec2>, bool) {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().map(|q| q.dist(*p) <= tol).unwrap_or(false) {
            continue;
        }
        out.push(*p);
    }
    let mut geschlossen = closed;
    if out.len() >= 3 {
        let first = out[0];
        if out.last().map(|q| q.dist(first) <= tol).unwrap_or(false) {
            out.pop();
            geschlossen = true;
        }
    }
    if out.len() < 3 {
        geschlossen = false;
    }
    (out, geschlossen)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EntityStyle {
    /// None = ByLayer
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub line_type: Option<LineType>,
    #[serde(default)]
    pub line_weight: Option<f64>,
    /// Füllung (z. B. Wand-Poché); None = ByLayer bzw. kein Fill
    #[serde(default)]
    pub fill: Option<Color>,
    /// Dämmung (Linienart `Insulation`): Bandhöhe **H** in mm. `None` = Vorgabe
    /// ([`crate::daemmung::BREITE_MM`]). Nur diese Linienart liest den Wert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_width: Option<f64>,
    /// Dämmung: Schlaufenform **r** (Periodenlänge / Höhe). `None` = Vorgabe
    /// ([`crate::daemmung::FORM`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_shape: Option<f64>,
}

impl Default for EntityStyle {
    fn default() -> Self {
        Self {
            color: None,
            line_type: None,
            line_weight: None,
            fill: None,
            loop_width: None,
            loop_shape: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub id: Id,
    pub kind: EntityKind,
    #[serde(default)]
    pub layer: Id,
    #[serde(default)]
    pub style: EntityStyle,
    /// In Blöcken: "ByBlock" für Farbe/Linie/Stärke
    #[serde(default)]
    pub by_block: bool,
    /// Punktsymbol am **Anfang** (T0210) — Block-ID aus der Kategorie
    /// [`PUNKTSYMBOL_KATEGORIE`].
    ///
    /// Ein Punkt hat keinen Anfang und kein Ende; er trägt sein Symbol deshalb
    /// im Anfangsfeld. Bei einer Linie steht das Symbol am Basispunkt `a`,
    /// [`Entity::end_symbol`] am Endpunkt `b`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_symbol: Option<String>,
    /// Punktsymbol am **Ende** der Linie (T0210) — nur Linien lesen es.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_symbol: Option<String>,
}

impl Entity {
    pub fn new(id: Id, layer: Id, kind: EntityKind) -> Self {
        Self {
            id,
            kind,
            layer,
            style: EntityStyle::default(),
            by_block: false,
            start_symbol: None,
            end_symbol: None,
        }
    }

    /// Das Punktsymbol dieses Objekts am genannten Ende — `false` = Anfang.
    ///
    /// Für einen Punkt ist das immer der Anfang; ein Ende hat er nicht.
    pub fn symbol(&self, am_ende: bool) -> Option<&str> {
        let feld = if am_ende && matches!(self.kind, EntityKind::Line { .. }) {
            &self.end_symbol
        } else {
            &self.start_symbol
        };
        feld.as_deref()
    }

    /// Trägt dieser Objekttyp überhaupt Punktsymbole? Nur Punkt und Linie.
    pub fn kann_punktsymbol(&self) -> bool {
        matches!(self.kind, EntityKind::Point { .. } | EntityKind::Line { .. })
    }

    pub fn line(id: Id, layer: Id, a: Vec2, b: Vec2) -> Self {
        Self::new(id, layer, EntityKind::Line { a, b })
    }

    /// Außenring und Inseln eines gefüllten Objekts (Füllfläche oder Volumen).
    pub fn filled_rings(&self) -> Option<(&[Vec2], &[Vec<Vec2>])> {
        match &self.kind {
            EntityKind::Fill { boundary, holes, .. }
            | EntityKind::Volume { boundary, holes, .. } => Some((boundary, holes)),
            _ => None,
        }
    }

    /// Höhe des Körpers in mm — nur Volumen (und Wände) haben eine.
    pub fn body_height(&self) -> Option<f64> {
        match &self.kind {
            EntityKind::Wall { height, .. } | EntityKind::Volume { height, .. } => Some(*height),
            _ => None,
        }
    }

    /// Volumen in m³. Volumen: Grundfläche × Höhe. Wand: Länge × Dicke × Höhe
    /// (brutto, ohne Abzug der Öffnungen).
    pub fn volume_m3(&self) -> Option<f64> {
        match &self.kind {
            EntityKind::Volume {
                boundary,
                holes,
                height,
                ..
            } => Some(crate::area::area_of(boundary, holes) * height / 1e9),
            EntityKind::Wall {
                a,
                b,
                thickness,
                height,
                ..
            } => Some(a.dist(*b) * thickness * height / 1e9),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Blöcke
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlockSource {
    /// Mit dem Programm geliefert, schreibgeschützt
    #[default]
    System,
    /// In der Zeichnung definiert
    Project,
    /// Benutzerbibliothek (profilweit)
    User,
}

impl BlockSource {
    pub fn label(self) -> &'static str {
        match self {
            BlockSource::System => "System",
            BlockSource::Project => "Projekt",
            BlockSource::User => "Benutzer",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            BlockSource::System => crate::i18n::tr("ui.blocksource.system"),
            BlockSource::Project => crate::i18n::tr("ui.blocksource.project"),
            BlockSource::User => crate::i18n::tr("ui.blocksource.user"),
        }
    }

}

/// Kategorie der Punktsymbole (T0210).
///
/// Blöcke dieser Kategorie lassen sich als Symbol an einem Punkt und an den
/// Enden einer Linie wählen — auch Benutzer- und Projektblöcke, denn die
/// Zuordnung hängt allein an der Kategorie, nicht an der Herkunft.
pub const PUNKTSYMBOL_KATEGORIE: &str = "Punkt Symbole";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockDef {
    /// Stabile, namensraum-präfixierte ID, z. B. "sys:wc" oder "prj:3f2a".
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub source: BlockSource,
    #[serde(default)]
    pub description: String,
    pub entities: Vec<Entity>,
    /// Optionales eigenes Icon (PNG-Pfad), überschreibt die automatische Vorschau.
    #[serde(default)]
    pub icon: Option<String>,
    /// Einfügebasispunkt (meist 0,0)
    #[serde(default)]
    pub base: Vec2,
    /// **Mit dem Zeichnungsmaßstab skalieren** (T0210): die Blockgeometrie
    /// liegt dann in Papier-Millimetern und wird beim Zeichnen mit dem
    /// Beschriftungsmaßstab vervielfacht — wie die Begrenzungssymbole der
    /// Bemaßung. Ohne die Eigenschaft gilt die Geometrie maßstäblich in der
    /// Zeichnung (1:1).
    #[serde(default)]
    pub annotative: bool,
}

impl BlockDef {
    pub fn bbox(&self) -> BBox {
        let scratch = Document::blank();
        let mut b = BBox::EMPTY;
        for e in &self.entities {
            b.union(crate::render::entity_bbox(&scratch, e));
        }
        b
    }

    /// Gehört der Block zur Kategorie „Punkt Symbole“ (T0210)?
    ///
    /// Geprüft wird ohne Rücksicht auf Groß-/Kleinschreibung und Leerzeichen am
    /// Rand — eine Kategorie schreibt der Nutzer im Fenster von Hand
    /// („Block-Eigenschaften“, T0204), und „punkt symbole“ ist dieselbe Angabe.
    pub fn ist_punktsymbol(&self) -> bool {
        self.category.trim().eq_ignore_ascii_case(PUNKTSYMBOL_KATEGORIE)
    }
}

// ---------------------------------------------------------------------------
// Blätter / Layout
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    pub id: Id,
    pub name: String,
    /// Papierkoordinaten in mm, Ursprung unten links.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Weltkoordinate in der Mitte des Fensters.
    pub center: Vec2,
    /// Maßstab des Fensters (1:scale).
    pub scale: f64,
    #[serde(default)]
    pub show_frame: bool,
    #[serde(default)]
    pub locked: bool,
    /// Nur diese Layer zeigen (None = alle sichtbaren)
    #[serde(default)]
    pub layers: Option<Vec<Id>>,
}

impl Viewport {
    pub fn rect(&self) -> BBox {
        BBox::new(v(self.x, self.y), v(self.x + self.w, self.y + self.h))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sheet {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub paper: PaperSize,
    #[serde(default = "default_true")]
    pub landscape: bool,
    /// Blattmaßstab (Anzeige im Schriftfeld)
    #[serde(default = "default_hundred")]
    pub scale: f64,
    #[serde(default = "default_true")]
    pub frame: bool,
    #[serde(default)]
    pub show_title_block: bool,
    /// Altbestand: Ansichtsfenster lagen bis T0109 in einer eigenen Liste.
    /// Beim Laden werden sie zu Objekten (`persist::normalize_sheet_viewports`);
    /// geschrieben wird die Liste nicht mehr.
    #[serde(rename = "viewports", default, skip_serializing)]
    pub viewports_alt: Vec<Viewport>,
    /// Zeichnungselemente im Papierraum (Schriftfeld etc.)
    #[serde(default)]
    pub entities: Vec<Entity>,
}

fn default_hundred() -> f64 {
    100.0
}

impl Sheet {
    pub fn size(&self) -> (f64, f64) {
        self.paper.dims(self.landscape)
    }
    pub fn bbox(&self) -> BBox {
        let (w, h) = self.size();
        BBox::new(v(0.0, 0.0), v(w, h))
    }
    /// Ansichtsfenster des Blatts als Objekte, in Zeichenreihenfolge.
    pub fn viewports(&self) -> impl Iterator<Item = (&Entity, &Viewport)> {
        self.entities.iter().filter_map(|e| match &e.kind {
            EntityKind::Viewport(vp) => Some((e, vp)),
            _ => None,
        })
    }

    pub fn viewport(&self, id: Id) -> Option<&Viewport> {
        self.viewports().find(|(e, _)| e.id == id).map(|(_, vp)| vp)
    }
}

// ---------------------------------------------------------------------------
// Einstellungen
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SnapSettings {
    #[serde(default = "default_true")]
    pub endpoint: bool,
    #[serde(default = "default_true")]
    pub midpoint: bool,
    #[serde(default = "default_true")]
    pub center: bool,
    #[serde(default = "default_true")]
    pub intersection: bool,
    #[serde(default = "default_true")]
    pub perpendicular: bool,
    /// Tangentialfang (T0157): Linie tangential an Kreis/Bogen, bewegter
    /// Kreis/Bogen tangential an einer Linie.
    #[serde(default = "default_true")]
    pub tangent: bool,
    /// Lotfang (T0216): die neue bzw. gezogene Linie steht rechtwinklig auf
    /// einer vorhandenen, wenn ihr Bezugspunkt auf dieser Linie liegt.
    #[serde(default = "default_true")]
    pub lot: bool,
    #[serde(default = "default_true")]
    pub nearest: bool,
    #[serde(default = "default_true")]
    pub grid: bool,
    #[serde(default = "default_true")]
    pub ortho: bool,
    #[serde(default = "default_true")]
    pub polar: bool,
    /// Punktfang auf den umgrenzenden Linien der Wandkörper (Kanten, Ecken,
    /// Laibungen) — unabhängig von den übrigen Fangarten schaltbar.
    #[serde(default = "default_true")]
    pub wall_edges: bool,
    /// Quadrantenpunkte eines Kreises/Bogens (0°, 90°, 180°, 270°) als eigenes
    /// Fangziel (T0160). Vorher hingen sie am Endpunkt-Fang: wer die Quadranten
    /// nicht wollte, musste auch die Endpunkte abschalten.
    #[serde(default = "default_true")]
    pub quadrant: bool,
    /// Winkelhalbierende zwischen zwei Linien als Fangziel (T0150) — in der
    /// Fang-Kette fest eingebaut, hier schaltbar (T0160).
    #[serde(default = "default_true")]
    pub bisector: bool,
    /// Erweiterter Punktfang (T0014): bleibt der Zeiger auf einem diskreten
    /// Fangpunkt stehen, wird dieser gehalten und gibt Hilfslinien. Aus =
    /// kein Verweilen, keine Hilfslinien (T0160).
    #[serde(default = "default_true")]
    pub hilfslinien: bool,
    /// Verweildauer bis zum Halten in Millisekunden ([`VERWEIL_MIN_MS`] …
    /// [`VERWEIL_MAX_MS`], Vorgabe 400).
    #[serde(default = "default_verweil_ms")]
    pub verweil_ms: f64,
    /// Höchstzahl gleichzeitig gehaltener Punkte (0 … [`HALTE_MAX`], Vorgabe 2).
    #[serde(default = "default_halte_max")]
    pub halte_max: usize,
    /// Rasterweite in mm
    #[serde(default = "default_grid")]
    pub grid_step: f64,
    /// Art des Zeichenrasters (T0144): rechtwinklig, isometrisch, dimetrisch,
    /// trimetrisch. Bestimmt Achsen der Anzeige **und** des Fangs.
    #[serde(default)]
    pub grid_kind: crate::gitter::Rasterart,
    /// Ob die gezeichnete Rasterweite mit dem Zoom skaliert oder fest bleibt
    /// (T0144).
    #[serde(default)]
    pub grid_mode: crate::gitter::Rastermodus,
    /// Rasterfang erzwingen (T0144): jede Eingabe liegt im Raster, ohne
    /// Fangradius. Objektpunkte (Endpunkt, Schnitt …) bleiben exakt.
    #[serde(default)]
    pub grid_force: bool,
    /// Fangradius in Pixeln
    #[serde(default = "default_snap_px")]
    pub tolerance_px: f64,
    /// Schrittweite des Polarfangs in Grad (Architektur: 45°)
    #[serde(default = "default_polar_step")]
    pub polar_step: f64,
    /// Winkel-Fenster des Polarfangs in Grad: nur innerhalb dieser Abweichung rastet er ein
    #[serde(default = "default_polar_tol")]
    pub polar_tolerance: f64,
}

fn default_polar_tol() -> f64 {
    6.0
}

/// Verweildauer des erweiterten Punktfangs in Millisekunden (T0160).
pub fn default_verweil_ms() -> f64 {
    400.0
}

/// Vorgabe der gehaltenen Punkte des erweiterten Punktfangs (T0160).
pub const HALTE_VORGABE: usize = 2;

/// Höchstzahl gehaltener Punkte des erweiterten Punktfangs (T0160).
pub fn default_halte_max() -> usize {
    HALTE_VORGABE
}

/// Untere Grenze der Verweildauer: darunter hält schon ein flüchtiges
/// Überfahren des Punktes, der Fang wird unberechenbar.
pub const VERWEIL_MIN_MS: f64 = 100.0;
/// Obere Grenze der Verweildauer: darüber muss man auf den Fang warten.
pub const VERWEIL_MAX_MS: f64 = 2000.0;
/// Höchstzahl gehaltener Punkte — zwei genügen (x vom einen, y vom anderen);
/// mehr Linien machen den Fang nur unübersichtlich.
pub const HALTE_MAX: usize = 4;

fn default_polar_step() -> f64 {
    45.0
}

fn default_grid() -> f64 {
    100.0
}

/// Untere Grenze der Rasterweite in mm: feiner als 1 mm rastet nichts mehr sinnvoll.
pub const GRID_STEP_MIN_MM: f64 = 1.0;
/// Obere Grenze der Rasterweite in mm (10 m) — darüber ist ein Fangraster in einer
/// Bauzeichnung nicht mehr brauchbar, und beliebig große Werte waren der Fehler.
pub const GRID_STEP_MAX_MM: f64 = 10_000.0;

/// Rasterweite auf den brauchbaren Bereich begrenzen (mm).
///
/// Die Grenzen gelten überall dort, wo die Rasterweite ins Dokument kommt
/// (Action `doc.settings`, Datei laden, Bedienfelder): negative Werte schalten
/// den Rasterfang still ab (beide Snap-Pfade prüfen `> 0`), riesige Werte
/// rasten ins Nichts.
pub fn clamp_grid_step(mm: f64) -> f64 {
    if mm.is_finite() {
        mm.clamp(GRID_STEP_MIN_MM, GRID_STEP_MAX_MM)
    } else {
        GRID_STEP_MIN_MM
    }
}
fn default_snap_px() -> f64 {
    12.0
}

impl SnapSettings {
    /// Das Raster dieser Einstellungen (Art und Weite) — eine Quelle für
    /// Anzeige und Fang (T0144). Bei Weite 0 fängt das Raster nicht.
    pub fn raster(&self) -> crate::gitter::Raster {
        crate::gitter::Raster::neu(self.grid_kind, self.grid_step)
    }
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            endpoint: true,
            midpoint: true,
            center: true,
            intersection: true,
            perpendicular: true,
            tangent: true,
            lot: true,
            nearest: true,
            grid: true,
            ortho: false,
            polar: true,
            wall_edges: true,
            quadrant: true,
            bisector: true,
            hilfslinien: true,
            verweil_ms: default_verweil_ms(),
            halte_max: default_halte_max(),
            grid_step: 100.0,
            grid_kind: crate::gitter::Rasterart::Rechteckig,
            grid_mode: crate::gitter::Rastermodus::Skaliert,
            grid_force: false,
            tolerance_px: 12.0,
            polar_step: 45.0,
            polar_tolerance: 6.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub input_unit: Unit,
    #[serde(default = "default_dim_unit")]
    pub dim_unit: Unit,
    #[serde(default = "default_two")]
    pub decimals: usize,
    /// Bezugsmaßstab für Papiergrößen von Texten/Bemaßung (1:annotation_scale)
    #[serde(default = "default_hundred")]
    pub annotation_scale: f64,
    #[serde(default)]
    pub snap: SnapSettings,
    #[serde(default = "default_text_height")]
    pub text_height: f64,
    #[serde(default = "default_wall_thickness")]
    pub wall_thickness: f64,
    #[serde(default = "default_door_width")]
    pub door_width: f64,
    #[serde(default = "default_window_width")]
    pub window_width: f64,
    #[serde(default = "default_stair_width")]
    pub stair_width: f64,
    #[serde(default = "default_true")]
    pub show_grid: bool,
    #[serde(default = "default_true")]
    pub show_rulers: bool,
}

fn default_dim_unit() -> Unit {
    Unit::M
}
fn default_two() -> usize {
    2
}
fn default_text_height() -> f64 {
    250.0
}
fn default_wall_thickness() -> f64 {
    240.0
}
fn default_door_width() -> f64 {
    885.0
}
fn default_window_width() -> f64 {
    1200.0
}
fn default_stair_width() -> f64 {
    1000.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input_unit: Unit::M,
            dim_unit: Unit::M,
            decimals: 2,
            annotation_scale: 100.0,
            snap: SnapSettings::default(),
            text_height: 250.0,
            wall_thickness: 240.0,
            door_width: 885.0,
            window_width: 1200.0,
            stair_width: 1000.0,
            show_grid: true,
            show_rulers: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectMeta {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub planner: String,
    #[serde(default)]
    pub plan_no: String,
    #[serde(default)]
    pub plan_title: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub note: String,
    /// Datum ISO (leer = aktuelles Datum)
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub revision: String,
}

// ---------------------------------------------------------------------------
// Dokument
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub name: String,
    /// Kennung des Dokuments (T0129): Stände der Sicherung folgen der Datei,
    /// nicht dem Pfad — auch eine ungespeicherte Zeichnung braucht eine.
    /// Beim Anlegen erzeugt; ältere Dateien bekommen sie beim Laden.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub project: ProjectMeta,
    #[serde(default)]
    pub settings: Settings,
    /// Pläne des Layer-Managers (T0202) — die oberste Gliederungsebene.
    #[serde(default)]
    pub plans: Vec<Plan>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub current_layer: Id,
    #[serde(default)]
    pub blocks: Vec<BlockDef>,
    #[serde(default)]
    pub entities: Vec<Entity>,
    /// Bearbeitungsraum der Befehlsschicht: `None` = Modell, `Some(blatt)` =
    /// Papierraum (Druckvorschau) dieses Blatts. Reiner Laufzeitzustand — wird
    /// nicht gespeichert, nicht verglichen und nicht mitgeschrieben.
    #[serde(skip)]
    pub space: Option<Id>,
    /// Kennung des **Blockeditors** (T0137): Ist sie gesetzt, arbeiten alle
    /// Befehle auf den Objekten dieser Blockdefinition statt auf der Zeichnung.
    /// Der Blockeditor ist damit ein dritter Bearbeitungsraum neben Modell und
    /// Papierraum — ohne zweiten Eingabe- oder Action-Weg. Reiner
    /// Laufzeitzustand: wird nicht gespeichert und nicht verglichen.
    #[serde(skip)]
    pub block_space: Option<String>,
    #[serde(default)]
    pub sheets: Vec<Sheet>,
    #[serde(default)]
    pub dim_style: DimStyle,
    #[serde(default = "default_one_id")]
    pub next_id: Id,
    /// Voreinstellungen der Linienstärke je Standardebene (T0125), Name → mm.
    /// Laufzeitzustand aus `prefs.json` — wird **nicht** mit der Zeichnung
    /// gespeichert. Bestehende Ebenen bleiben unangetastet; der Wert gilt nur
    /// für frisch angelegte Standardebenen ([`Document::ensure_layer`]).
    #[serde(skip)]
    pub layer_weight_defaults: BTreeMap<String, f64>,
    #[serde(skip)]
    pub modified: bool,
    #[serde(skip)]
    pub path: Option<String>,
}

fn default_one_id() -> Id {
    1
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Leere Architekturzeichnung mit Standard-Layern, Systemblöcken und A3-Blatt.
    pub fn new() -> Self {
        let mut doc = Self {
            format: FORMAT_NAME.to_string(),
            version: FORMAT_VERSION,
            name: "Unbenannt".to_string(),
            id: crate::sicherung::neue_uuid(),
            project: ProjectMeta::default(),
            settings: Settings::default(),
            plans: Vec::new(),
            layers: Vec::new(),
            current_layer: 0,
            blocks: Vec::new(),
            entities: Vec::new(),
            space: None,
            block_space: None,
            sheets: Vec::new(),
            dim_style: DimStyle::default(),
            next_id: 1,
            layer_weight_defaults: BTreeMap::new(),
            modified: false,
            path: None,
        };
        // T0202: Eine neue Zeichnung hat **einen** Plan („Plan 1“) mit den vier
        // Standardlayern. Weitere Pläne legt der Nutzer im Layer-Manager an;
        // jeder neue Plan bekommt dieselben vier Layer (`actions::plan.create`).
        let plan_id = doc.new_id();
        doc.plans.push(Plan::new(plan_id, "Plan 1"));
        doc.add_standard_layers(plan_id);
        doc.current_layer = doc.layers.first().map(|l| l.id).unwrap_or(0);
        doc.blocks = crate::blocks_lib::system_blocks(&mut doc.next_id);
        let sheet = crate::templates::new_sheet(&mut doc, PaperSize::A3, true, "Blatt 1");
        doc.sheets.push(sheet);
        // Ebenen, Systemblöcke und Blatt vergeben nur Ids (`new_id` markiert
        // „geändert“) — eine neue Zeichnung ist aber unverändert. Sonst meldet
        // jeder frische Reiter ungespeicherte Änderungen, und eine Datei im
        // leeren Startreiter landet in einem zweiten statt darin (T0073).
        doc.modified = false;
        doc
    }

    /// Minimales Dokument ohne Layer/Blöcke (für Geometrie-Hilfsrechnungen).
    pub fn blank() -> Self {
        Self {
            format: FORMAT_NAME.to_string(),
            version: FORMAT_VERSION,
            name: String::new(),
            // Kein Kennungsbedarf: `blank` ist ein Rechengerüst (z. B. für
            // `Entity::bbox`) und wird in heißen Schleifen angelegt.
            id: String::new(),
            project: ProjectMeta::default(),
            settings: Settings::default(),
            plans: Vec::new(),
            layers: Vec::new(),
            current_layer: 0,
            blocks: Vec::new(),
            entities: Vec::new(),
            space: None,
            block_space: None,
            sheets: Vec::new(),
            dim_style: DimStyle::default(),
            next_id: 1,
            layer_weight_defaults: BTreeMap::new(),
            modified: false,
            path: None,
        }
    }

    /// Voreinstellung der Linienstärke für eine Standardebene (T0125):
    /// der eingestellte Wert, sonst [`LINIENSTAERKE_VORGABE`] (0,18 mm).
    /// Unbekannte Namen bekommen dieselbe Vorgabe.
    pub fn standard_breite(&self, ebene: &str) -> f64 {
        self.layer_weight_defaults
            .get(ebene)
            .copied()
            .filter(|w| w.is_finite() && *w > 0.0)
            .unwrap_or(LINIENSTAERKE_VORGABE)
    }

    /// Übernimmt die Voreinstellungen der Linienstärke (T0125): merkt sie für
    /// neue Standardebenen und **setzt** sie auf die vorhandenen
    /// Standardebenen. Nur für frische Zeichnungen gedacht — eine geöffnete
    /// Zeichnung darf ihre Ebenenbreiten behalten
    /// ([`Document::merke_standard_breiten`]).
    pub fn setze_standard_breiten(&mut self, breiten: &BTreeMap<String, f64>) {
        self.layer_weight_defaults = breiten.clone();
        let vorgabe = |name: &str| {
            breiten
                .get(name)
                .copied()
                .filter(|w| w.is_finite() && *w > 0.0)
                .unwrap_or(LINIENSTAERKE_VORGABE)
        };
        for l in &mut self.layers {
            if LAYER_DEFAULTS
                .iter()
                .any(|(name, _, _)| name.eq_ignore_ascii_case(&l.name))
            {
                l.line_weight = vorgabe(&l.name);
            }
        }
    }

    /// Merkt die Voreinstellungen nur für **neue** Standardebenen — die
    /// vorhandenen Ebenen einer geöffneten Zeichnung bleiben unverändert.
    pub fn merke_standard_breiten(&mut self, breiten: &BTreeMap<String, f64>) {
        self.layer_weight_defaults = breiten.clone();
    }

    pub fn new_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        self.modified = true;
        id
    }

    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn layer_by_name(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    pub fn layer_id_by_name(&self, name: &str) -> Option<Id> {
        self.layer_by_name(name).map(|l| l.id)
    }

    // ---------------- Pläne (T0202) ----------------

    pub fn plan(&self, id: Id) -> Option<&Plan> {
        self.plans.iter().find(|p| p.id == id)
    }

    pub fn plan_mut(&mut self, id: Id) -> Option<&mut Plan> {
        self.plans.iter_mut().find(|p| p.id == id)
    }

    /// Der Plan, zu dem ein Layer gehört (ältere Dateien: keiner).
    pub fn plan_of_layer(&self, layer: Id) -> Option<&Plan> {
        let l = self.layer(layer)?;
        self.plan(l.plan)
    }

    /// Die Layer eines Plans in Zeichenreihenfolge (Reihenfolge von
    /// `Document::layers`).
    pub fn layers_of_plan(&self, plan: Id) -> Vec<&Layer> {
        self.layers.iter().filter(|l| l.plan == plan).collect()
    }

    /// Sichtbar ist ein Layer nur, wenn er selbst **und** sein Plan sichtbar
    /// sind. Ein Layer ohne Plan (alte Datei, vor der Migration) zählt als
    /// sichtbar.
    pub fn layer_sichtbar(&self, layer: Id) -> bool {
        match self.layer(layer) {
            Some(l) => l.visible && self.plan(l.plan).map(|p| p.visible).unwrap_or(true),
            None => true,
        }
    }

    /// Zahl der Objekte auf einem Layer — Modell- und Papierraum zusammen
    /// (T0213). Ein Layer, der nur im Schriftfeld benutzt wird, ist nicht leer.
    pub fn layer_objekte(&self, layer: Id) -> usize {
        self.entities
            .iter()
            .chain(self.sheets.iter().flat_map(|s| s.entities.iter()))
            .filter(|e| e.layer == layer)
            .count()
    }

    /// Setzt alle Objekte eines Layers auf einen anderen Layer (T0213):
    /// Modell- und Papierraum. Blockdefinitionen bleiben unangetastet.
    /// Gibt die Zahl der verschobenen Objekte zurück.
    pub fn layer_objekte_verschieben(&mut self, von: Id, nach: Id) -> usize {
        let mut n = 0;
        for e in self.entities.iter_mut() {
            if e.layer == von {
                e.layer = nach;
                n += 1;
            }
        }
        for s in self.sheets.iter_mut() {
            for e in s.entities.iter_mut() {
                if e.layer == von {
                    e.layer = nach;
                    n += 1;
                }
            }
        }
        n
    }

    /// Gesperrt ist ein Layer, wenn er selbst oder sein Plan gesperrt ist —
    /// ausgeblendet zählt wie gesperrt (nicht wählbar, kein Fangpunkt).
    pub fn layer_gesperrt(&self, layer: Id) -> bool {
        match self.layer(layer) {
            Some(l) => {
                let plan = self.plan(l.plan);
                l.locked
                    || !l.visible
                    || plan.map(|p| p.locked || !p.visible).unwrap_or(false)
            }
            None => false,
        }
    }

    /// Layer für einen Namen holen oder neu anlegen — **innerhalb eines
    /// Plans** (T0202). Ein Layername ist je Plan eindeutig, nicht global:
    /// „Zeichnung“ gibt es in jedem Plan.
    pub fn ensure_layer_in(&mut self, name: &str, color: Color, plan: Id) -> Id {
        if let Some(l) = self
            .layers
            .iter()
            .find(|l| l.plan == plan && l.name.eq_ignore_ascii_case(name))
        {
            return l.id;
        }
        let id = self.new_id();
        let mut l = Layer::new(id, name, color);
        l.plan = plan;
        // T0125: eine neu angelegte Standardebene trägt die eingestellte
        // Voreinstellung; freie Ebenen behalten die Grundbreite.
        if let Some(w) = self.layer_weight_defaults.get(name) {
            l.line_weight = *w;
        }
        // Hinter den letzten Layer desselben Plans einsortieren, damit die
        // Anzeige gruppiert bleibt.
        let pos = self
            .layers
            .iter()
            .rposition(|x| x.plan == plan)
            .map(|i| i + 1)
            .unwrap_or(self.layers.len());
        self.layers.insert(pos, l);
        id
    }

    /// Layer für einen Namen im Plan des aktuellen Layers holen oder anlegen.
    pub fn ensure_layer(&mut self, name: &str, color: Color) -> Id {
        let plan = self
            .layer(self.current_layer)
            .map(|l| l.plan)
            .or_else(|| self.plans.first().map(|p| p.id))
            .unwrap_or(0);
        self.ensure_layer_in(name, color, plan)
    }

    /// Plan des aktuellen Layers — der Plan, in dem gezeichnet wird.
    pub fn aktueller_plan(&self) -> Option<Id> {
        self.layer(self.current_layer)
            .map(|l| l.plan)
            .or_else(|| self.plans.first().map(|p| p.id))
    }

    /// Legt die vier Standardlayer eines Plans an
    /// ([`crate::style::LAYER_DEFAULTS`], T0202). Vorhandene Namen im Plan
    /// bleiben, wie sie sind.
    pub fn add_standard_layers(&mut self, plan: Id) {
        for (name, color, lt) in LAYER_DEFAULTS.iter() {
            let vorhanden = self
                .layers
                .iter()
                .any(|l| l.plan == plan && l.name.eq_ignore_ascii_case(name));
            let id = self.ensure_layer_in(name, *color, plan);
            if !vorhanden {
                let breite = self.standard_breite(name);
                if let Some(l) = self.layer_mut(id) {
                    l.line_type = *lt;
                    l.line_weight = breite;
                }
            }
        }
    }

    pub fn block(&self, id: &str) -> Option<&BlockDef> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Alle Blöcke der Kategorie „Punkt Symbole“ (T0210), nach Namen sortiert.
    ///
    /// Das ist die **eine** Auswahlliste für das Eigenschaften-Panel — System-,
    /// Projekt- und Benutzerblöcke stehen gleichberechtigt darin, weil allein
    /// die Kategorie zählt. Wer eine zweite Liste daneben baut, lässt
    /// Benutzerblöcke wieder aus.
    pub fn punktsymbol_bloecke(&self) -> Vec<&BlockDef> {
        let mut liste: Vec<&BlockDef> = self
            .blocks
            .iter()
            .filter(|b| b.ist_punktsymbol())
            .collect();
        liste.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        liste
    }

    pub fn block_mut(&mut self, id: &str) -> Option<&mut BlockDef> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    pub fn sheet(&self, id: Id) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn sheet_mut(&mut self, id: Id) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }

    // ---- Bearbeitungsraum (Modell / Papierraum) ---------------------------
    // Die Befehlsschicht arbeitet immer auf "den Objekten des aktiven Raums".
    // Im Modell ist das `entities`, im Papierraum `sheets[..].entities`.
    // Dadurch gilt jeder Befehl (Zeichnen, Ändern, Reihenfolge, Auswahl) in
    // beiden Räumen gleich — ohne zweiten Weg an der Action-Schicht vorbei.

    /// Objekte des aktiven Bearbeitungsraums.
    ///
    /// Drei Räume, eine Quelle: Blockeditor (`block_space`) vor Papierraum
    /// (`space`) vor Modell. Der Blockeditor hat Vorrang, weil er immer
    /// ausdrücklich geöffnet wird.
    pub fn ents(&self) -> &[Entity] {
        if let Some(bid) = self.block_space.as_deref() {
            if let Some(b) = self.block(bid) {
                return &b.entities;
            }
        }
        match self.space.and_then(|sid| self.sheet(sid)) {
            Some(s) => &s.entities,
            None => &self.entities,
        }
    }

    /// Objekte des aktiven Bearbeitungsraums, veränderbar.
    pub fn ents_mut(&mut self) -> &mut Vec<Entity> {
        if let Some(bid) = self.block_space.clone() {
            if let Some(i) = self.blocks.iter().position(|b| b.id == bid) {
                return &mut self.blocks[i].entities;
            }
        }
        let Some(sid) = self.space.filter(|sid| self.sheets.iter().any(|s| s.id == *sid)) else {
            return &mut self.entities;
        };
        &mut self
            .sheets
            .iter_mut()
            .find(|s| s.id == sid)
            .expect("Blatt eben geprüft")
            .entities
    }

    /// Block des offenen Blockeditors (None = kein Blockeditor).
    pub fn block_edit(&self) -> Option<&BlockDef> {
        self.block_space.as_deref().and_then(|id| self.block(id))
    }

    /// Läuft die Bearbeitung im Blockeditor?
    pub fn in_block_space(&self) -> bool {
        self.block_edit().is_some()
    }

    /// Blockeditor öffnen. Nur **eigene** Blöcke (Projekt/Benutzer) sind
    /// bearbeitbar — Systemblöcke sind schreibgeschützt und werden abgelehnt;
    /// ein unbekannter Bezeichner ebenso. Ein Blockeditor ist kein Papierraum:
    /// `space` wird dabei geleert.
    pub fn set_block_space(&mut self, block: Option<&str>) -> bool {
        let ziel = block.and_then(|id| {
            self.blocks
                .iter()
                .find(|b| b.id == id && b.source != BlockSource::System)
                .map(|b| b.id.clone())
        });
        if block.is_some() && ziel.is_none() {
            return false;
        }
        self.block_space = ziel;
        if self.block_space.is_some() {
            self.space = None;
        }
        true
    }

    /// Blatt des aktiven Papierraums (None = Modell).
    pub fn space_sheet(&self) -> Option<&Sheet> {
        self.space.and_then(|sid| self.sheet(sid))
    }

    /// Bearbeitungsraum wechseln. Ein unbekanntes Blatt fällt auf das Modell
    /// zurück — nie auf ein Blatt, das es nicht gibt.
    pub fn set_space(&mut self, space: Option<Id>) {
        self.space = space.filter(|sid| self.sheets.iter().any(|s| s.id == *sid));
        // Modell/Papierraum und Blockeditor schließen sich aus.
        self.block_space = None;
    }

    /// Ist der Papierraum (Druckvorschau) der aktive Raum?
    pub fn in_paper_space(&self) -> bool {
        self.space_sheet().is_some()
    }

    /// Rasterweite des aktiven Raums in mm. Der Papierraum ist in Blatt-
    /// Millimetern gezeichnet, dort ist 10 mm die brauchbare Rasterweite.
    pub fn grid_step(&self) -> f64 {
        if self.in_paper_space() {
            10.0
        } else {
            self.settings.snap.grid_step
        }
    }

    /// Platz für ein neu eingefügtes Element mit diesem Zeichenrang.
    ///
    /// Grundsatz: unter **allen** Objekten mit höherem Rang (nichts darf eine
    /// Linie oder Wand überdecken), innerhalb des eigenen Rangs über den schon
    /// vorhandenen. Der Platz ist deshalb das Minimum aus
    /// - der ersten Stelle mit höherem Rang (Klassengrenze) und
    /// - direkt hinter dem letzten Objekt gleichen Rangs.
    ///
    /// Bestehende Objekte werden nie umsortiert; sie rücken nur um eine Stelle.
    /// Damit bleibt eine von Hand verdrehte Reihenfolge des Nutzers erhalten,
    /// und das neue Element landet trotzdem hinter Flächen bzw. vor Linien.
    pub fn einfuege_index(&self, rang: u8) -> usize {
        let ents = self.ents();
        let erster_hoeher = ents
            .iter()
            .position(|e| e.zeichenrang() > rang)
            .unwrap_or(ents.len());
        let letzter_gleich = ents.iter().rposition(|e| e.zeichenrang() == rang);
        match letzter_gleich {
            Some(i) => erster_hoeher.min(i + 1),
            None => erster_hoeher,
        }
    }

    /// Fügt ein neues Element ranggerecht ein und meldet Id und Platz
    /// (1-basiert wie in `modify.order`).
    pub fn insert_ranked(&mut self, mut e: Entity) -> (Id, usize) {
        if e.id == 0 {
            e.id = self.new_id();
        }
        if e.layer == 0 {
            e.layer = self.current_layer;
        }
        let id = e.id;
        let platz = self.einfuege_index(e.zeichenrang());
        self.ents_mut().insert(platz, e);
        self.modified = true;
        (id, platz)
    }

    /// Fügt ein neues Element ein. Der Platz kommt aus dem Zeichenrang
    /// (`insert_ranked`) — neu gezeichnete Objekte ordnen sich damit von
    /// selbst: Flächen nach hinten, Linien über Wände, Beschriftung nach vorn.
    pub fn add_entity(&mut self, e: Entity) -> Id {
        self.insert_ranked(e).0
    }

    pub fn remove_entity(&mut self, id: Id) -> Option<Entity> {
        let ents = self.ents_mut();
        let entfernt = ents.iter().position(|e| e.id == id).map(|pos| ents.remove(pos));
        if entfernt.is_some() {
            self.modified = true;
        }
        entfernt
    }

    pub fn entity(&self, id: Id) -> Option<&Entity> {
        self.ents().iter().find(|e| e.id == id)
    }

    pub fn entity_mut(&mut self, id: Id) -> Option<&mut Entity> {
        self.ents_mut().iter_mut().find(|e| e.id == id)
    }

    /// Alle sichtbaren Layer-IDs — ein ausgeblendeter **Plan** blendet seine
    /// Layer mit aus (T0202).
    pub fn visible_layers(&self) -> Vec<Id> {
        self.layers
            .iter()
            .filter(|l| self.layer_sichtbar(l.id))
            .map(|l| l.id)
            .collect()
    }

    pub fn effective_color(&self, e: &Entity) -> Color {
        if let Some(c) = e.style.color {
            return c;
        }
        self.layer(e.layer).map(|l| l.color).unwrap_or(Color::BLACK)
    }

    /// Bandhöhe der Dämmung in mm — eigener Wert, sonst die Vorgabe.
    pub fn effective_loop_width(&self, e: &Entity) -> f64 {
        e.style
            .loop_width
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(crate::daemmung::BREITE_MM)
    }

    /// Schlaufenform der Dämmung — eigener Wert, sonst die Vorgabe.
    pub fn effective_loop_shape(&self, e: &Entity) -> f64 {
        e.style
            .loop_shape
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(crate::daemmung::FORM)
    }

    pub fn effective_line_type(&self, e: &Entity) -> LineType {
        if let Some(lt) = e.style.line_type {
            return lt;
        }
        self.layer(e.layer)
            .map(|l| l.line_type)
            .unwrap_or(LineType::Solid)
    }

    pub fn effective_line_weight(&self, e: &Entity) -> f64 {
        if let Some(lw) = e.style.line_weight {
            return lw;
        }
        self.layer(e.layer).map(|l| l.line_weight).unwrap_or(0.25)
    }

    /// Gesamt-Bounding-Box aller Modellobjekte.
    pub fn bbox(&self) -> BBox {
        let mut b = BBox::EMPTY;
        for e in &self.entities {
            b.union(crate::render::entity_bbox(self, e));
        }
        b
    }

    /// Wand, in der eine Tür/ein Fenster sitzt.
    pub fn host_wall(&self, id: Option<Id>) -> Option<&Entity> {
        let id = id?;
        self.entities
            .iter()
            .find(|e| e.id == id && matches!(e.kind, EntityKind::Wall { .. }))
    }

    /// Öffnungen (Türen/Fenster) für eine Wand.
    pub fn wall_openings(&self, wall_id: Id) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for e in &self.entities {
            match &e.kind {
                EntityKind::Door {
                    center,
                    angle,
                    width,
                    host: Some(h),
                    ..
                }
                | EntityKind::Window {
                    center,
                    angle,
                    width,
                    host: Some(h),
                    ..
                } if *h == wall_id => {
                    out.push((*angle, *width));
                    let _ = center;
                }
                _ => {}
            }
        }
        out.into_iter().map(|(_, w)| (0.0, w)).collect()
    }

    /// Text mit dynamischen Feldern auflösen (Maßstab aus den Einstellungen).
    pub fn resolve_text(&self, text: &str) -> String {
        self.resolve_text_scaled(text, None)
    }

    /// Text mit dynamischen Feldern auflösen; `scale` überschreibt {MASSTAB}
    /// (im Papierraum: der Blattmaßstab).
    pub fn resolve_text_scaled(&self, text: &str, scale: Option<f64>) -> String {
        let mut s = text.to_string();
        let today = crate::util::today_iso();
        let today_de = crate::util::iso_to_de(&today);
        s = s.replace("{DATUM}", &today_de);
        s = s.replace("{DATE}", &today_de);
        s = s.replace("{ISO}", &today);
        s = s.replace("{PROJEKT}", &self.project.project);
        s = s.replace("{BAUHERR}", &self.project.client);
        s = s.replace("{PLANER}", &self.project.planner);
        s = s.replace("{PLAN}", &self.project.plan_no);
        s = s.replace("{PLANTITEL}", &self.project.plan_title);
        s = s.replace("{ADRESSE}", &self.project.address);
        s = s.replace("{REVISION}", &self.project.revision);
        s = s.replace("{DATEI}", &self.name);
        let sc = scale.unwrap_or(self.settings.annotation_scale);
        s = s.replace("{MASSTAB}", &crate::units::format_scale(sc));
        s = s.replace("{MASSSTAB}", &crate::units::format_scale(sc));
        s
    }

    /// Alle Objekte, die eine Blockdefinition referenzieren.
    pub fn block_instances(&self, block_id: &str) -> Vec<Id> {
        self.entities
            .iter()
            .filter(|e| matches!(&e.kind, EntityKind::BlockRef { block, .. } if block == block_id))
            .map(|e| e.id)
            .collect()
    }

    /// Prüft, ob Layer `id` gesperrt ist — der Plan zählt mit (T0202).
    pub fn layer_locked(&self, id: Id) -> bool {
        self.layer_gesperrt(id)
    }

    /// Kurzfassung für die Statuszeile — zählt den aktiven Raum.
    pub fn summary_short(&self) -> String {
        crate::i18n::t_args_owned(
            "gui.dialogs.bestand_x_objekte",
            &[("anzahl", format!("{}", self.ents().len()))],
        )
    }

    /// Objektzählung der Zeichnung — nur Modellobjekte, keine Blockbibliothek.
    pub fn zaehlung(&self) -> ObjektZaehlung {
        ObjektZaehlung::erhebe(&self.entities)
    }

    /// Anzahl der mitgelieferten Systemblöcke in der Blockbibliothek.
    ///
    /// Sie gehören zum Programm, nicht zur Zeichnung — deshalb zählt `summary`
    /// sie nicht als Objekte mit, sondern nennt sie getrennt.
    pub fn system_blocks(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| b.source == BlockSource::System)
            .count()
    }

    /// Bestand als eine Zeile: alle Teile ergeben zusammen die Objektzahl.
    pub fn summary(&self) -> String {
        let z = self.zaehlung();
        crate::i18n::t_args_owned(
            "gui.dialogs.bestand_zeile",
            &[
                ("objekte", format!("{}", z.objekte)),
                ("waende", format!("{}", z.waende)),
                ("tueren", format!("{}", z.tueren)),
                ("fenster", format!("{}", z.fenster)),
                ("bloecke", format!("{}", z.blockinstanzen)),
                ("sonstige", format!("{}", z.sonstige)),
            ],
        )
    }
}

/// Zählung der Objekte einer Zeichnung.
///
/// Die Teile ergeben zusammen `objekte` — jede Objektart fällt in genau eine
/// Zeile. Block**definitionen** (Bibliothek) sind kein Zeichnungsinhalt; gezählt
/// werden nur eingesetzte Blockinstanzen (`EntityKind::BlockRef`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjektZaehlung {
    pub objekte: usize,
    pub waende: usize,
    pub tueren: usize,
    pub fenster: usize,
    pub blockinstanzen: usize,
    /// Alles, was keine der vier Architekturarten ist: Linien, Maße, Texte, …
    pub sonstige: usize,
}

impl ObjektZaehlung {
    pub fn erhebe(ents: &[Entity]) -> Self {
        let mut z = Self {
            objekte: ents.len(),
            ..Self::default()
        };
        for e in ents {
            match &e.kind {
                EntityKind::Wall { .. } => z.waende += 1,
                EntityKind::Door { .. } => z.tueren += 1,
                EntityKind::Window { .. } => z.fenster += 1,
                EntityKind::BlockRef { .. } => z.blockinstanzen += 1,
                // Bilder sind keine Architekturart — sie zählen zu „Sonstige“.
                EntityKind::Image { .. } => z.sonstige += 1,
                _ => z.sonstige += 1,
            }
        }
        z
    }
}

// ---------------------------------------------------------------------------
// Attribut-Map für Blöcke (zukünftige Nutzung)
// ---------------------------------------------------------------------------

pub type Attributes = BTreeMap<String, String>;
