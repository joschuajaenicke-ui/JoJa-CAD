//! Gebrauchsschriften, die in der Windows-Fassung in der Programmdatei liegen
//! (T0205).
//!
//! Windows bringt Liberation Sans/Serif/Mono/Narrow und Quicksand nicht mit;
//! `C:\Windows\Fonts` kennt nur `arial.ttf`, `times.ttf` und Verwandte. Bisher
//! lieferte das ZIP deshalb einen Ordner `fonts/` mit. Damit eine einzelne
//! `jjcad.exe` genügt, stehen die Dateien aus `packaging/fonts/` hier über
//! `include_bytes!` im Programm — derselbe Weg wie bei den technischen
//! Schriften ([`super::gebuendelt`]).
//!
//! Eingebettet wird **nur** unter Windows oder wenn das Cargo-Merkmal
//! `schriften-eingebettet` gesetzt ist (Prüfweg auf Linux: damit läuft die
//! Vollständigkeitsprüfung der Tabelle auch dort). Auf Linux bleiben die
//! Gebrauchsschriften Systemschriften: das AppImage bringt sie als Dateien mit,
//! und Distro-Pakete dürfen keine Schriften bündeln, die es als Paket gibt.
//!
//! Der Dateiname ohne Endung ist zugleich der `/BaseFont` im PDF
//! (`LiberationSans-Regular`); er stimmt mit dem Namen überein, den die Suche
//! im Ordner `fonts/` neben dem Programm ergibt. Deshalb ändern sich
//! bestehende PDFs nicht.

use crate::fonts::FontStyle;
use crate::model::TextFont;

/// Eine eingebettete Schriftdatei.
pub(crate) struct Datei {
    /// Name wie in `packaging/fonts/`; ohne Endung steht er im PDF.
    pub dateiname: &'static str,
    /// Inhalt der `.ttf`.
    pub daten: &'static [u8],
}

/// Tabelle je Familie und Schnitt — die eine Quelle für die Einbettung.
///
/// Quicksand gibt es nur in Regular und Bold; die kursiven Schnitte greifen
/// wie bei der Suche im Dateisystem auf die entsprechende Grunddatei zurück
/// (`bases()` in `fonts.rs` nennt dieselbe Zuordnung).
static TABELLE: &[(TextFont, FontStyle, Datei)] = &[
    // Serifenlos (Liberation Sans) — metrisch wie Helvetica
    (
        TextFont::Sans,
        FontStyle::Regular,
        Datei {
            dateiname: "LiberationSans-Regular.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSans-Regular.ttf"),
        },
    ),
    (
        TextFont::Sans,
        FontStyle::Bold,
        Datei {
            dateiname: "LiberationSans-Bold.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSans-Bold.ttf"),
        },
    ),
    (
        TextFont::Sans,
        FontStyle::Italic,
        Datei {
            dateiname: "LiberationSans-Italic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSans-Italic.ttf"),
        },
    ),
    (
        TextFont::Sans,
        FontStyle::BoldItalic,
        Datei {
            dateiname: "LiberationSans-BoldItalic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSans-BoldItalic.ttf"),
        },
    ),
    // Serifen (Liberation Serif) — metrisch wie Times
    (
        TextFont::Serif,
        FontStyle::Regular,
        Datei {
            dateiname: "LiberationSerif-Regular.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSerif-Regular.ttf"),
        },
    ),
    (
        TextFont::Serif,
        FontStyle::Bold,
        Datei {
            dateiname: "LiberationSerif-Bold.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSerif-Bold.ttf"),
        },
    ),
    (
        TextFont::Serif,
        FontStyle::Italic,
        Datei {
            dateiname: "LiberationSerif-Italic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSerif-Italic.ttf"),
        },
    ),
    (
        TextFont::Serif,
        FontStyle::BoldItalic,
        Datei {
            dateiname: "LiberationSerif-BoldItalic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSerif-BoldItalic.ttf"),
        },
    ),
    // Nichtproportional (Liberation Mono) — metrisch wie Courier
    (
        TextFont::Mono,
        FontStyle::Regular,
        Datei {
            dateiname: "LiberationMono-Regular.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationMono-Regular.ttf"),
        },
    ),
    (
        TextFont::Mono,
        FontStyle::Bold,
        Datei {
            dateiname: "LiberationMono-Bold.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationMono-Bold.ttf"),
        },
    ),
    (
        TextFont::Mono,
        FontStyle::Italic,
        Datei {
            dateiname: "LiberationMono-Italic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationMono-Italic.ttf"),
        },
    ),
    (
        TextFont::Mono,
        FontStyle::BoldItalic,
        Datei {
            dateiname: "LiberationMono-BoldItalic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationMono-BoldItalic.ttf"),
        },
    ),
    // Schmal (Liberation Sans Narrow) — nicht die DIN 1451
    (
        TextFont::Narrow,
        FontStyle::Regular,
        Datei {
            dateiname: "LiberationSansNarrow-Regular.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSansNarrow-Regular.ttf"),
        },
    ),
    (
        TextFont::Narrow,
        FontStyle::Bold,
        Datei {
            dateiname: "LiberationSansNarrow-Bold.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSansNarrow-Bold.ttf"),
        },
    ),
    (
        TextFont::Narrow,
        FontStyle::Italic,
        Datei {
            dateiname: "LiberationSansNarrow-Italic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSansNarrow-Italic.ttf"),
        },
    ),
    (
        TextFont::Narrow,
        FontStyle::BoldItalic,
        Datei {
            dateiname: "LiberationSansNarrow-BoldItalic.ttf",
            daten: include_bytes!("../../../packaging/fonts/LiberationSansNarrow-BoldItalic.ttf"),
        },
    ),
    // Runde Zierschrift (Quicksand) — es gibt nur Regular und Bold
    (
        TextFont::Rund,
        FontStyle::Regular,
        Datei {
            dateiname: "Quicksand-Regular.ttf",
            daten: include_bytes!("../../../packaging/fonts/Quicksand-Regular.ttf"),
        },
    ),
    (
        TextFont::Rund,
        FontStyle::Bold,
        Datei {
            dateiname: "Quicksand-Bold.ttf",
            daten: include_bytes!("../../../packaging/fonts/Quicksand-Bold.ttf"),
        },
    ),
    (
        TextFont::Rund,
        FontStyle::Italic,
        Datei {
            dateiname: "Quicksand-Regular.ttf",
            daten: include_bytes!("../../../packaging/fonts/Quicksand-Regular.ttf"),
        },
    ),
    (
        TextFont::Rund,
        FontStyle::BoldItalic,
        Datei {
            dateiname: "Quicksand-Bold.ttf",
            daten: include_bytes!("../../../packaging/fonts/Quicksand-Bold.ttf"),
        },
    ),
];

/// Eingebettete Datei für Familie und Schnitt — mit Rückfall auf den
/// Grundschnitt, wie ihn die Suche im Dateisystem nimmt.
pub(crate) fn datei(font: TextFont, style: FontStyle) -> Option<&'static Datei> {
    let gleich = |f: &TextFont, s: &FontStyle| *f == font && *s == style;
    TABELLE
        .iter()
        .find(|(f, s, _)| gleich(f, s))
        .or_else(|| {
            TABELLE
                .iter()
                .find(|(f, s, _)| *f == font && *s == FontStyle::Regular)
        })
        .map(|(_, _, d)| d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{self, FontStyle};
    use crate::model::TextFont;

    /// Die Familien, die im ZIP steckten — genau diese müssen eingebettet sein.
    const GEBRAUCH: [TextFont; 5] = [
        TextFont::Sans,
        TextFont::Serif,
        TextFont::Mono,
        TextFont::Narrow,
        TextFont::Rund,
    ];

    /// Keine stille Lücke: jede Kombination aus Familie und Schnitt steht in der
    /// Tabelle, ist lesbar und trägt den Dateinamen als `/BaseFont`.
    #[test]
    fn jede_kombination_ist_eingebettet() {
        for font in GEBRAUCH {
            for style in FontStyle::ALL {
                let treffer = TABELLE
                    .iter()
                    .find(|(f, s, _)| *f == font && *s == style)
                    .unwrap_or_else(|| {
                        panic!("{} · {:?} fehlt in der Tabelle", font.label(), style)
                    });
                let d = &treffer.2;
                assert!(
                    d.daten.len() > 10_000,
                    "{} ist zu klein ({} Bytes)",
                    d.dateiname,
                    d.daten.len()
                );
                crate::fonts::TtfInfo::parse(d.daten)
                    .unwrap_or_else(|e| panic!("{} ist keine TrueType-Datei: {e}", d.dateiname));
                assert_eq!(
                    crate::fonts::ps_name(font, style),
                    d.dateiname.trim_end_matches(".ttf"),
                    "{} · {:?} schreibt einen anderen Namen ins PDF",
                    font.label(),
                    style
                );
            }
        }
        // 16 Dateien für die vier Liberation-Familien plus 2 für Quicksand.
        let dateien: std::collections::BTreeSet<&str> =
            TABELLE.iter().map(|(_, _, d)| d.dateiname).collect();
        assert_eq!(dateien.len(), 18, "die Tabelle deckt nicht alle 18 Dateien ab");
    }

    /// Ohne `fonts/` neben dem Programm gewinnt die eingebettete Schrift — der
    /// geladene Inhalt ist byte-gleich mit der Tabelle, nicht die Systemschrift.
    #[test]
    fn eingebettet_schlaegt_die_systemschrift() {
        for font in GEBRAUCH {
            let d = datei(font, FontStyle::Regular).expect("eingebettet");
            let geladen = fonts::ttf_bytes(font, FontStyle::Regular).expect("Schrift geladen");
            assert!(
                geladen == d.daten,
                "{} kommt nicht aus der Tabelle ({} Bytes gegen {} Bytes)",
                font.label(),
                geladen.len(),
                d.daten.len()
            );
            assert!(
                fonts::verfuegbar(font),
                "{} gilt als nicht verfügbar",
                font.label()
            );
        }
    }

    /// Die Einbettung ändert den PDF-Namen nicht: `/BaseFont` bleibt der
    /// Dateiname ohne Endung, die Schrift steckt als Teilsatz im PDF.
    #[test]
    fn pdf_traegt_den_unveraenderten_basefont_und_den_teilsatz() {
        use crate::model::{Document, Entity, EntityKind, TextAlignH, TextAlignV};
        let mut doc = Document::default();
        let layer = doc.current_layer;
        let id = doc.new_id();
        doc.add_entity(Entity::new(
            id,
            layer,
            EntityKind::Text {
                p: crate::geom::v(20.0, 30.0),
                text: "Wohnen 12,4".into(),
                height: 6.0,
                angle: 0.0,
                align: TextAlignH::Left,
                valign: TextAlignV::Bottom,
                bold: false,
                italic: false,
                underline: false,
                superscript: false,
                runs: Vec::new(),
                font: TextFont::Sans,
                line_spacing: 1.0,
                frame: false,
                frame_round: false,
                background: None,
            },
        ));
        let pfad = std::env::temp_dir().join("jjcad-t0205-basefont.pdf");
        let _ = std::fs::remove_file(&pfad);
        crate::pdf::export_model_pdf(&doc, &pfad, crate::style::PaperSize::A4)
            .expect("PDF schreiben");
        let roh = String::from_utf8_lossy(&std::fs::read(&pfad).expect("PDF lesen")).to_string();
        let _ = std::fs::remove_file(&pfad);
        assert!(
            roh.contains("/BaseFont /LiberationSans-Regular"),
            "/BaseFont weicht ab — bestehende PDFs würden sich ändern"
        );
        assert!(
            roh.contains("/FontFile2"),
            "die Schrift steckt nicht als Teilsatz im PDF"
        );
    }
}
