//! Bilder über die **System-Zwischenablage** austauschen (T0112).
//!
//! Zwei Richtungen, ein Weg:
//!
//! * **Hinein** — liegt in der Zwischenablage ein Bild (Screenshot, Foto aus
//!   einem anderen Programm), wird es als PNG eingebettet und das Werkzeug
//!   `draw.image` gestartet. Der Nutzer klickt wie nach einem Datei-Import die
//!   beiden Ecken.
//! * **Hinaus** — ist genau **ein** Bild ausgewählt, legt Strg+C dessen Pixel
//!   als reine Bilddaten in die Zwischenablage; andere Programme fügen es dort
//!   als Bild ein.
//!
//! egui selbst kennt nur Text (`Event::Paste(String)`), deshalb spricht diese
//! Kiste die Zwischenablage über `arboard` an. Unter Linux gehört der Inhalt
//! dem Programm, das ihn abgelegt hat: die Verbindung muss also leben, solange
//! das Programm läuft — sie steckt deshalb in [`BildAblage`] und wird nicht je
//! Aufruf neu geöffnet. Ohne Anzeige (Testlauf, `cargo test`) schlägt das
//! Öffnen fehl; alle Wege sind dann stille No-Ops und die Oberfläche bleibt
//! bedienbar.

use jjcad_core::bild::Bild;

/// Ein Bild aus der System-Zwischenablage.
pub struct AblageBild {
    pub breite: u32,
    pub hoehe: u32,
    /// RGBA, 4 Byte je Punkt, Zeile 0 oben.
    pub rgba: Vec<u8>,
    /// `true`, wenn es das Bild ist, das dieses Programm selbst abgelegt hat.
    /// Dann gilt die eigene Zwischenablage (mit Lage und Größe) statt eines
    /// neuen Bildes.
    pub eigen: bool,
}

/// Verbindung zur System-Zwischenablage, einmal je Programm.
#[derive(Default)]
pub struct BildAblage {
    clip: Option<arboard::Clipboard>,
    /// Das Öffnen ist gescheitert (keine Anzeige) — nicht bei jedem Tastendruck
    /// erneut versuchen.
    ohne_anzeige: bool,
    /// Kennung (Hash über die Pixel) des zuletzt von uns abgelegten Bildes.
    eigen: Option<u64>,
}

impl BildAblage {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Öffnet die Verbindung beim ersten Bedarf. `None` ohne Anzeige.
    fn verbindung(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clip.is_none() && !self.ohne_anzeige {
            match arboard::Clipboard::new() {
                Ok(clip) => self.clip = Some(clip),
                Err(_) => self.ohne_anzeige = true,
            }
        }
        self.clip.as_mut()
    }

    /// Legt die Pixel eines Bildes als **reine Bilddaten** ab.
    pub fn lege(&mut self, bild: &Bild) -> Result<(), String> {
        let kennung = jjcad_core::bild::kennung(&bild.rgba);
        let daten = arboard::ImageData {
            width: bild.breite as usize,
            height: bild.hoehe as usize,
            bytes: std::borrow::Cow::Borrowed(&bild.rgba),
        };
        let clip = self
            .verbindung()
            .ok_or_else(|| "Zwischenablage nicht verfügbar".to_string())?;
        clip.set_image(daten).map_err(|e| e.to_string())?;
        self.eigen = Some(kennung);
        Ok(())
    }

    /// Holt ein Bild aus der Zwischenablage — `None`, wenn keines vorliegt.
    pub fn hole(&mut self) -> Option<AblageBild> {
        let clip = self.verbindung()?;
        let bild = clip.get_image().ok()?;
        if bild.width == 0 || bild.height == 0 {
            return None;
        }
        let erwartet = bild.width * bild.height * 4;
        if bild.bytes.len() < erwartet {
            return None;
        }
        let rgba = bild.bytes[..erwartet].to_vec();
        let eigen = self.eigen == Some(jjcad_core::bild::kennung(&rgba));
        Some(AblageBild {
            breite: bild.width as u32,
            hoehe: bild.height as u32,
            rgba,
            eigen,
        })
    }

    /// Liegt zusätzlich **Text** in der Zwischenablage?
    ///
    /// Wichtig für Strg+V: enthält die Zwischenablage Text, meldet egui das
    /// Einfügen selbst (`Event::Paste`) — dann darf das Loslassen der Taste
    /// nicht ein zweites Mal einfügen.
    pub fn hat_text(&mut self) -> bool {
        match self.verbindung() {
            Some(clip) => clip.get_text().map(|t| !t.is_empty()).unwrap_or(false),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    /// T0112: Ohne das Merkmal `wayland-data-control` liest arboard unter
    /// Wayland über XWayland — und dort kommen Bilder aus anderen
    /// Wayland-Programmen nicht an (im Protokoll stand „Zwischenablage leer“).
    /// Die Zusicherung steht in der `Cargo.toml` dieses Pakets; wer sie
    /// entfernt, bricht diesen Test.
    #[test]
    fn arboard_liest_unter_wayland_nativ() {
        let manifest = include_str!("../Cargo.toml");
        assert!(
            manifest.contains("wayland-data-control"),
            "arboard braucht das Merkmal `wayland-data-control`, sonst bleibt das \
             Einfügen von Bildern aus anderen Programmen unter Wayland leer"
        );
    }
}
