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
    /// Dämmung: statt eines Strichs wird ein **Schlaufenband** entlang der
    /// Linie gezeichnet (Breite `H`, Schlaufenform `r`). Die Länge bleibt
    /// exakt die der Linie — Rechnung in [`crate::daemmung`], Dokumentation
    /// in `docs/wissen/DAEMMUNG-SCHLAUFEN.md`.
    Insulation,
}

impl LineType {
    pub const ALL: [LineType; 6] = [
        LineType::Solid,
        LineType::Dashed,
        LineType::Dotted,
        LineType::DashDot,
        LineType::Hidden,
        LineType::Insulation,
    ];

    /// Kennung in Datei und Befehlsschicht — **eine** Quelle für alle
    /// Aufrufstellen (`props`, `canvas`, `actions`).
    pub fn key(self) -> &'static str {
        match self {
            LineType::Solid => "solid",
            LineType::Dashed => "dashed",
            LineType::Dotted => "dotted",
            LineType::DashDot => "dashdot",
            LineType::Hidden => "hidden",
            LineType::Insulation => "insulation",
        }
    }

    /// Kennung → Linienart; deutsche Namen gelten weiter (MCP-Eingaben).
    pub fn from_key(s: &str) -> Option<LineType> {
        let s = s.trim().to_lowercase();
        Some(match s.as_str() {
            "solid" | "durchgezogen" => LineType::Solid,
            "dashed" | "gestrichelt" => LineType::Dashed,
            "dotted" | "gepunktet" => LineType::Dotted,
            "dashdot" | "strichpunkt" => LineType::DashDot,
            "hidden" | "verdeckt" => LineType::Hidden,
            "insulation" | "daemmung" | "dämmung" | "schlaufen" => LineType::Insulation,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            LineType::Solid => "Durchgezogen",
            LineType::Dashed => "Gestrichelt",
            LineType::Dotted => "Gepunktet",
            LineType::DashDot => "Strichpunkt",
            LineType::Hidden => "Verdeckt",
            LineType::Insulation => "Dämmung",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            LineType::Solid => crate::i18n::tr("ui.linetype.solid"),
            LineType::Dashed => crate::i18n::tr("ui.linetype.dashed"),
            LineType::Dotted => crate::i18n::tr("ui.linetype.dotted"),
            LineType::DashDot => crate::i18n::tr("ui.linetype.dash_dot"),
            LineType::Hidden => crate::i18n::tr("ui.linetype.hidden"),
            LineType::Insulation => crate::i18n::tr("ui.linetype.insulation"),
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
            // Die Dämmung ist kein Strichmuster: sie wird als Schlaufenband
            // gezeichnet (siehe `daemmung`), nicht gestrichelt.
            LineType::Insulation => &[],
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
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            ArrowKind::Slash => crate::i18n::tr("ui.arrowkind.slash"),
            ArrowKind::Arrow => crate::i18n::tr("ui.arrowkind.arrow"),
            ArrowKind::Dot => crate::i18n::tr("ui.arrowkind.dot"),
            ArrowKind::None => crate::i18n::tr("ui.arrowkind.none"),
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
/// Maßstab des Plans (`Plan::scale`, T0232).
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
// Muster einer Füllfläche
// ---------------------------------------------------------------------------

/// Muster einer Füllfläche. Gilt für den Objekttyp `Fill` **und** für die
/// Wandschraffur (`Wall.hatch`) — ein Muster, eine Bedeutung.
///
/// Die Namen folgen DIN 1356-1:2026-09, Tabelle 5 „Kennzeichnung der
/// Schnittflächen von Baustoffen“ (18 Schraffuren). Die schon vorhandenen
/// Muster tragen seitdem den Namen der Zeile, die sie abbilden: `Lines` ist
/// Mauerwerk, `Cross` Kunststoffe, `Dots` Sand.
///
/// **Abdichtung (Nr. 15)** ist keine Schraffur, sondern eine Linienart (T0132).
/// **Dämmung (Nr. 14)** ist seit T0136 beides: als Linie das Schlaufenband
/// `LineType::Insulation`, als Fläche das Muster `InsulationHatch`, das aus
/// derselben Kurve parallele Schlaufenbänder zeichnet. Alte Zeichnungen mit den
/// früheren Dämm-Mustern laden über den Serde-Alias an `Lines` als Mauerwerk
/// weiter, statt beim Öffnen zu scheitern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FillPattern {
    /// Fläche in der Füllfarbe
    #[default]
    Solid,
    /// Parallele Linien im Musterwinkel (Vorgabe 45°) — Tabelle 5, Nr. 9
    /// „Mauerwerk“.
    ///
    /// Die 2026 entfernten Muster „Dämmstoffe“, „Dämmstoffe Schlaufen“ und die
    /// drei „Abdichtungen“ (T0132) tragen ihre alten Serde-Namen als Alias und
    /// laden deshalb weiter — als Mauerwerk, weil ein fehlendes Muster die
    /// Zeichnung nicht öffnen ließe.
    #[serde(
        alias = "Insulation",
        alias = "InsulationLoops",
        alias = "SealSingle",
        alias = "SealMulti",
        alias = "SealUpstand"
    )]
    Lines,
    /// Zwei Linienscharen im rechten Winkel — Nr. 17 „Kunststoffe“.
    Cross,
    /// Punktraster — Nr. 4 „Sand“.
    Dots,
    /// Nr. 1 „Boden/Gelände unverändert“: Flechtmuster aus Haken und
    /// Querstrichen (T0152).
    GroundPlain,
    /// Nr. 2 „Boden/Gelände verändert“: Flechtwerk aus langen Strichen beider
    /// Diagonalen (T0152).
    GroundChanged,
    /// Nr. 3 „Kies“: Kieselsteine als Kreise wechselnder Größe.
    Gravel,
    /// Nr. 5 „Beton (unbewehrt)“: gestrichelte Musterlinien, Reihen versetzt.
    ConcretePlain,
    /// Nr. 6 „Beton (bewehrt)“: durchgehende Linien mit kurzen Strichen
    /// dazwischen.
    ConcreteReinforced,
    /// Nr. 7 „WU-Beton“: durchgehende Linien mit langen Strichen dazwischen.
    ConcreteWatertight,
    /// Nr. 8 „Textilbeton“: Linien mit Pfeilen dazwischen.
    TextileConcrete,
    /// Nr. 10 „Vollholz, quer“: gewellte Linien (Faser quer zum Musterwinkel).
    WoodAcross,
    /// Nr. 11 „Vollholz, längs“: gewellte Linien waagerecht (längs der Faser).
    WoodAlong,
    /// Nr. 12 „Metall“: weite Musterlinien, dazwischen kurze Striche.
    Metal,
    /// Nr. 13 „Putz“: gröberes, unregelmäßiges Punktraster.
    Plaster,
    /// Nr. 14 „Dämmung“ (T0136): parallele Schlaufenbänder — dieselbe Kurve
    /// wie die Linienart [`LineType::Insulation`], aber mit **fester**
    /// Schlaufengröße. Der Musterabstand ist die Bandhöhe (Dämmstärke H).
    ///
    /// Die Linienart passt die Periodenlänge an die Linienlänge an (eine runde
    /// Anzahl Schlaufen); nebeneinander gelegt stünde dann an jeder Kante ein
    /// anderes Schlaufenmaß. Das Muster rechnet die Periode deshalb fest aus
    /// H und der Schlaufenform (`daemmung::periode`) und beschneidet das Band
    /// an der Kontur.
    ///
    /// Der Serde-Name `Insulation` bleibt beim alten Alias an [`FillPattern::Lines`]
    /// (Dämmstoffe von vor 2026): ein neues Muster braucht einen neuen Namen,
    /// sonst wären zwei Bedeutungen an einem Wort.
    InsulationHatch,
    /// Nr. 16 „Dichtstoffe“: enges Kreuzraster.
    Sealant,
    /// Nr. 18 „Glas“: Gruppen aus drei kurzen Strichen.
    Glass,
}

impl FillPattern {
    /// Alle Muster in der Reihenfolge der Norm (Tabelle 5, Nr. 1–13 und
    /// 16–18; Nr. 14 und 15 sind Linienarten, siehe [`FillPattern`]) —
    /// dieselbe Reihenfolge zeigt die Auswahl im Eigenschaften-Panel.
    pub const ALL: &'static [FillPattern] = &[
        FillPattern::Solid,
        FillPattern::GroundPlain,
        FillPattern::GroundChanged,
        FillPattern::Gravel,
        FillPattern::Dots,
        FillPattern::ConcretePlain,
        FillPattern::ConcreteReinforced,
        FillPattern::ConcreteWatertight,
        FillPattern::TextileConcrete,
        FillPattern::Lines,
        FillPattern::WoodAcross,
        FillPattern::WoodAlong,
        FillPattern::Metal,
        FillPattern::Plaster,
        FillPattern::InsulationHatch,
        FillPattern::Sealant,
        FillPattern::Cross,
        FillPattern::Glass,
    ];

    /// Bezeichnung nach DIN 1356-1:2026-09, Tabelle 5. Kurz genug für die
    /// 150-px-Wertespalte des Eigenschaften-Panels.
    pub fn label(self) -> &'static str {
        match self {
            FillPattern::Solid => "Vollfläche",
            FillPattern::Lines => "Mauerwerk",
            FillPattern::Cross => "Kunststoffe",
            FillPattern::Dots => "Sand",
            FillPattern::GroundPlain => "Boden unverändert",
            FillPattern::GroundChanged => "Boden verändert",
            FillPattern::Gravel => "Kies",
            FillPattern::ConcretePlain => "Beton unbewehrt",
            FillPattern::ConcreteReinforced => "Beton bewehrt",
            FillPattern::ConcreteWatertight => "WU-Beton",
            FillPattern::TextileConcrete => "Textilbeton",
            FillPattern::WoodAcross => "Vollholz quer",
            FillPattern::WoodAlong => "Vollholz längs",
            FillPattern::Metal => "Metall",
            FillPattern::Plaster => "Putz",
            FillPattern::InsulationHatch => "Dämmung",
            FillPattern::Sealant => "Dichtstoffe",
            FillPattern::Glass => "Glas",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            FillPattern::Solid => crate::i18n::tr("ui.fillpattern.solid"),
            FillPattern::Lines => crate::i18n::tr("ui.fillpattern.lines"),
            FillPattern::Cross => crate::i18n::tr("ui.fillpattern.cross"),
            FillPattern::Dots => crate::i18n::tr("ui.fillpattern.dots"),
            FillPattern::GroundPlain => crate::i18n::tr("ui.fillpattern.ground_plain"),
            FillPattern::GroundChanged => crate::i18n::tr("ui.fillpattern.ground_changed"),
            FillPattern::Gravel => crate::i18n::tr("ui.fillpattern.gravel"),
            FillPattern::ConcretePlain => crate::i18n::tr("ui.fillpattern.concrete_plain"),
            FillPattern::ConcreteReinforced => crate::i18n::tr("ui.fillpattern.concrete_reinforced"),
            FillPattern::ConcreteWatertight => crate::i18n::tr("ui.fillpattern.concrete_watertight"),
            FillPattern::TextileConcrete => crate::i18n::tr("ui.fillpattern.textile_concrete"),
            FillPattern::WoodAcross => crate::i18n::tr("ui.fillpattern.wood_across"),
            FillPattern::WoodAlong => crate::i18n::tr("ui.fillpattern.wood_along"),
            FillPattern::Metal => crate::i18n::tr("ui.fillpattern.metal"),
            FillPattern::Plaster => crate::i18n::tr("ui.fillpattern.plaster"),
            FillPattern::InsulationHatch => crate::i18n::tr("ui.fillpattern.insulation"),
            FillPattern::Sealant => crate::i18n::tr("ui.fillpattern.sealant"),
            FillPattern::Glass => crate::i18n::tr("ui.fillpattern.glass"),
        }
    }


    /// Schlüssel für Action-Parameter (`draw.fill`, `modify.set`) und für die
    /// gespeicherte Vorgabe in `prefs.json`.
    pub fn key(self) -> &'static str {
        match self {
            FillPattern::Solid => "solid",
            FillPattern::Lines => "mauerwerk",
            FillPattern::Cross => "kunststoffe",
            FillPattern::Dots => "sand",
            FillPattern::GroundPlain => "boden_unveraendert",
            FillPattern::GroundChanged => "boden_veraendert",
            FillPattern::Gravel => "kies",
            FillPattern::ConcretePlain => "beton_unbewehrt",
            FillPattern::ConcreteReinforced => "beton_bewehrt",
            FillPattern::ConcreteWatertight => "wu_beton",
            FillPattern::TextileConcrete => "textilbeton",
            FillPattern::WoodAcross => "vollholz_quer",
            FillPattern::WoodAlong => "vollholz_laengs",
            FillPattern::Metal => "metall",
            FillPattern::Plaster => "putz",
            FillPattern::InsulationHatch => "daemmung",
            FillPattern::Sealant => "dichtstoffe",
            FillPattern::Glass => "glas",
        }
    }

    /// Muster aus einem Action-Parameter lesen (deutsch und englisch, auch die
    /// alten Schlüssel `lines`/`cross`/`dots`). Die Schlüssel der entfernten
    /// Dämm- und Abdichtungsmuster gibt es nicht mehr (T0132) — sie liefern
    /// `None`, damit kein Aufruf still ein anderes Muster bekommt.
    pub fn from_key(s: &str) -> Option<FillPattern> {
        let k = s.trim().to_lowercase().replace([' ', '-', '/'], "_");
        Some(match k.as_str() {
            "solid" | "voll" | "vollflaeche" | "vollfläche" | "voll_fläche" => FillPattern::Solid,
            // Mauerwerk (Nr. 9) — hieß früher „Schraffur 45°“
            "mauerwerk" | "lines" | "line" | "linien" | "schraffur" | "45" => FillPattern::Lines,
            // Kunststoffe (Nr. 17) — hieß früher „Kreuzschraffur“
            "kunststoffe" | "cross" | "kreuz" | "kreuzschraffur" => FillPattern::Cross,
            // Sand (Nr. 4) — hieß früher „Punktraster“
            "sand" | "dots" | "punkte" | "punktraster" => FillPattern::Dots,
            "boden_unveraendert" | "boden_unverändert" | "boden" => {
                FillPattern::GroundPlain
            }
            "boden_veraendert" | "boden_verändert" => {
                FillPattern::GroundChanged
            }
            "kies" | "gravel" => FillPattern::Gravel,
            "beton_unbewehrt" | "beton" | "concrete" => FillPattern::ConcretePlain,
            "beton_bewehrt" | "stahlbeton" | "concrete_reinforced" => {
                FillPattern::ConcreteReinforced
            }
            "wu_beton" | "wasserdichter_beton" => FillPattern::ConcreteWatertight,
            "textilbeton" | "textile_concrete" => FillPattern::TextileConcrete,
            "vollholz_quer" | "holz_quer" | "wood_across" => FillPattern::WoodAcross,
            "vollholz_laengs" | "vollholz_längs" | "holz_laengs" | "wood_along" => {
                FillPattern::WoodAlong
            }
            "metall" | "metal" => FillPattern::Metal,
            "putz" | "plaster" => FillPattern::Plaster,
            // Dämmung als Flächenmuster (T0136) — die Linienart heißt im
            // Dateiformat `insulation` und bleibt davon getrennt.
            "daemmung" | "dämmung" | "insulation_hatch" => FillPattern::InsulationHatch,
            "dichtstoffe" | "sealant" => FillPattern::Sealant,
            "glas" | "glass" => FillPattern::Glass,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------
// Wahl der Füllung (T0242)
// ---------------------------------------------------------------------------

/// Füllung eines Objekts im Eigenschaften-Panel: **keine**, die Vorgabe des
/// Objekts oder eine Farbe.
///
/// Drei Zustände statt „Farbe oder nichts“: ohne Füllfarbe zeichnet eine Wand
/// den Vorgabeton (grau) und eine Vollfläche die aufgehellte Linienfarbe — die
/// Abwesenheit einer Farbe ist also **nicht** „keine Füllung“. Gespeichert
/// wird das im eigenen Feld [`crate::model::EntityStyle::no_fill`], die Farbe
/// bleibt in `fill`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fuellung {
    /// Füllfläche ganz aus — keine Fläche, kein Muster (nur die Umrandung).
    Keine,
    /// Vorgabe des Objekts: Wandton bzw. aufgehellte Linienfarbe.
    Standard,
    /// eine ausdrücklich gewählte Farbe
    Farbe(Color),
}

impl Fuellung {
    /// Die gespeicherte Füllung eines Objekts als Panel-Wahl.
    pub fn aus_style(fill: Option<Color>, no_fill: bool) -> Fuellung {
        if no_fill {
            Fuellung::Keine
        } else {
            match fill {
                Some(c) => Fuellung::Farbe(c),
                None => Fuellung::Standard,
            }
        }
    }

    /// Die beiden Felder, in die diese Wahl geschrieben wird.
    pub fn in_style(self) -> (Option<Color>, bool) {
        match self {
            Fuellung::Keine => (None, true),
            Fuellung::Standard => (None, false),
            Fuellung::Farbe(c) => (Some(c), false),
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
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            PaperSize::A4 => crate::i18n::tr("ui.papersize.a4"),
            PaperSize::A3 => crate::i18n::tr("ui.papersize.a3"),
            PaperSize::A2 => crate::i18n::tr("ui.papersize.a2"),
            PaperSize::A1 => crate::i18n::tr("ui.papersize.a1"),
            PaperSize::A0 => crate::i18n::tr("ui.papersize.a0"),
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

/// Linienstärke neuer Ebenen, wenn der Nutzer keine Voreinstellung gesetzt hat
/// (T0125). Eine Vorgabe für **alle** Objektarten — so ist es auch in Allplan;
/// die frühere Mischung (Wände 0,50, Türen 0,25 …) ist damit weg.
pub const LINIENSTAERKE_VORGABE: f64 = 0.18;

/// Farbe der Standardebenen einer neuen Zeichnung (T0249).
///
/// Vorher trug „Beschriftung“ ein Rot (`#BE463C`) und „Blöcke“ ein Grau
/// (`#6E737A`): in einer neuen Datei standen damit alle Maßzahlen, Texte und
/// Beschriftungen in Rot und jede Blockinstanz in Grau, obwohl der Plan in
/// Schwarz gezeichnet wird. Jetzt tragen **alle vier** Layer den einen
/// Schwarzwert des Programms ([`Color::BLACK`]) — dieselbe Farbe wie Rahmen
/// und Schriftfeld eines Blatts (`templates`).
pub const LAYER_FARBE: Color = Color::BLACK;

/// Die Standardebenen einer neuen Zeichnung: Name, Farbe, Linientyp.
///
/// Seit T0202 sind das die **vier Layer eines Plans**: Jeder Plan (auch jeder
/// neu angelegte) besteht aus Zeichnung, Architektur, Beschriftung und Blöcken.
/// Die Zuordnung der Objektarten steht in `actions::standard_ebene`.
///
/// Die **Linienstärke** steht bewusst nicht in dieser Liste — sie kommt aus
/// den Voreinstellungen (`UserPrefs::layer_weights`, Vorgabe
/// [`LINIENSTAERKE_VORGABE`]). So gibt es keine zweite, fest verdrahtete
/// Wahrheit, die von der Einstellung abweichen könnte (T0125).
pub const LAYER_DEFAULTS: &[(&str, Color, LineType)] = &[
    ("Zeichnung", LAYER_FARBE, LineType::Solid),
    ("Architektur", LAYER_FARBE, LineType::Solid),
    ("Beschriftung", LAYER_FARBE, LineType::Solid),
    ("Blöcke", LAYER_FARBE, LineType::Solid),
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
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            WallAlign::Left => crate::i18n::tr("ui.wallalign.left"),
            WallAlign::Center => crate::i18n::tr("ui.wallalign.center"),
            WallAlign::Right => crate::i18n::tr("ui.wallalign.right"),
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
