//! Das Markenzeichen der Anwendung — Rollvorlage mit Haus, orange auf dunkel.
//!
//! Die Bilddateien liegen in `crates/jjcad-gui/assets/` (Kopie aus
//! `branding/` im Projektordner) und kommen per `include_bytes!` mit ins
//! Programm: der Testkanal besteht aus einer einzigen Datei, eine Suche im
//! Projektordner fände dort nichts. Dasselbe Muster wie bei den technischen
//! Schriften in `jjcad-core` (`fonts.rs`).
//!
//! Zwei Stellen benutzen das Markenzeichen: den Fenstertitel (`main`,
//! [`fenster_icon`]) und den Über-Dialog (`dialogs`, [`textur`]). Wer eine
//! dritte braucht, nimmt diese beiden Funktionen — keinen zweiten Satz
//! Bilddaten und kein gezeichnetes Ersatzicon (das war der alte Zustand:
//! 32 px, im Code gemalt, ohne Bezug zum Markenzeichen).

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};

/// Das Markenzeichen in 512 × 512 Bildpunkten (RGBA, Hintergrund durchsichtig).
pub const ICON_512: &[u8] = include_bytes!("../assets/jjc-icon-512.png");

/// Dieselbe Marke in 32 × 32 — für die kleine Darstellung in der Kopfzeile
/// gerastert, damit die weißen Linien dort nicht zu Grau verwaschen (T0083).
pub const ICON_32: &[u8] = include_bytes!("../assets/jjc-icon-32.png");

/// 128 × 128 für das Fenstericon (`_NET_WM_ICON`). Das ist **nicht** die
/// 512er-Datei: 512 × 512 × 4 Byte ergibt eine X11-Eigenschaft von 1 MB, 256 px
/// schon 262 144 Byte — beides über der Höchstlänge einer X-Anfrage (65 535
/// Wörter = 262 140 Byte ohne BIG-REQUESTS), und winit verwirft den `BadLength`
/// still: im Fenster stand dann **gar kein** Icon (gemessen: `xprop
/// _NET_WM_ICON` leer bei 512 und 256, „Icon (128 x 128)" bei 128). Nachgemessen
/// mit `xprop -id <fenster> _NET_WM_ICON`, nicht am Bild.
pub const ICON_128: &[u8] = include_bytes!("../assets/jjc-icon-128.png");

/// Dunkelton des Markenzeichens — Rückfall, falls das Bild nicht lesbar ist.
const DUNKEL: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);

/// Dekodiert ein eingebettetes PNG in unmultiplizierte RGBA-Bytes.
///
/// Gibt `(Breite, Höhe, Bytes)` zurück; die Bytes sind immer vier je Bildpunkt,
/// auch wenn das PNG eine Palette oder Graustufen trägt (`Transformations`).
fn rgba(png: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("Markenzeichen nicht lesbar: {e}"))?;
    let mut puffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut puffer)
        .map_err(|e| format!("Markenzeichen nicht lesbar: {e}"))?;
    let bytes = puffer[..info.buffer_size()].to_vec();
    if bytes.len() != (info.width as usize) * (info.height as usize) * 4 {
        return Err("Markenzeichen hat eine unerwartete Puffergröße".into());
    }
    Ok((info.width as usize, info.height as usize, bytes))
}

/// Das Markenzeichen als egui-Bild (für Texturen und Vergleiche).
pub fn bild(png: &[u8]) -> Result<ColorImage, String> {
    let (w, h, bytes) = rgba(png)?;
    Ok(ColorImage::from_rgba_unmultiplied([w, h], &bytes))
}

/// Fenstericon für Taskleiste und Fenstertitel.
///
/// winit erwartet unmultiplizierte RGBA-Bytes. Ist das Bild nicht lesbar, kommt
/// ein einzelner dunkler Bildpunkt statt eines Absturzes beim Start — das
/// Fenster soll auch dann aufgehen, wenn am Markenzeichen etwas kaputt ist.
/// `tests::fenstericon_ist_das_markenzeichen` hält den Normalfall fest.
pub fn fenster_icon() -> egui::IconData {
    match rgba(ICON_128) {
        Ok((w, h, bytes)) => egui::IconData {
            rgba: bytes,
            width: w as u32,
            height: h as u32,
        },
        Err(_) => egui::IconData {
            rgba: vec![DUNKEL.r(), DUNKEL.g(), DUNKEL.b(), 0xFF],
            width: 1,
            height: 1,
        },
    }
}

/// Textur des Markenzeichens, einmal je egui-Kontext angelegt.
pub fn textur(ctx: &egui::Context) -> TextureHandle {
    textur_aus(ctx, "jjcad-markenzeichen", ICON_512)
}

/// Dieselbe Marke in der kleinen Rasterung (Kopfzeile, 30 px).
pub fn textur_klein(ctx: &egui::Context) -> TextureHandle {
    textur_aus(ctx, "jjcad-markenzeichen-klein", ICON_32)
}

/// Eine Textur je Name und Kontext — `load_texture` bei jedem Bild würde bei
/// offenem Dialog jede Frame eine neue Textur anlegen.
fn textur_aus(ctx: &egui::Context, name: &str, png: &[u8]) -> TextureHandle {
    let id = egui::Id::new(name);
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return t;
    }
    let bild = bild(png).unwrap_or_else(|_| ColorImage::new([1, 1], DUNKEL));
    let t = ctx.load_texture(name, bild, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die mitgelieferte Datei ist das Markenzeichen in groß, nicht das alte
    /// gezeichnete 32-px-Platzhalterbild (T0083).
    #[test]
    fn eingebettetes_bild_ist_das_markenzeichen() {
        let b = bild(ICON_512).expect("Markenzeichen muss lesbar sein");
        assert_eq!(b.size, [512, 512], "Größe aus branding/jjc-icon-512.png");

        let pixel = |x: usize, y: usize| b.pixels[y * b.size[0] + x];
        // Ecken durchsichtig (kein Kasten dahinter) …
        assert_eq!(pixel(0, 0).a(), 0, "Ecke links oben ist durchsichtig");
        assert_eq!(pixel(511, 511).a(), 0, "Ecke rechts unten ist durchsichtig");
        // … die Rollvorlage in Orange (227, 162, 61) …
        let orange = b
            .pixels
            .iter()
            .filter(|p| p.r() > 200 && (120..200).contains(&p.g()) && p.b() < 100)
            .count();
        assert!(
            orange > 20_000,
            "orange Rollvorlage fehlt (nur {orange} Bildpunkte)"
        );
        // … und der dunkle Grund des Blattes.
        let dunkel = b
            .pixels
            .iter()
            .filter(|p| p.a() == 255 && p.r() < 40 && p.g() < 40 && p.b() < 40)
            .count();
        assert!(dunkel > 20_000, "dunkler Grund fehlt (nur {dunkel})");
    }

    #[test]
    fn fenstericon_ist_das_markenzeichen() {
        let i = fenster_icon();
        assert_eq!((i.width, i.height), (128, 128));
        assert_eq!(i.rgba.len(), 128 * 128 * 4);
    }

    /// Die Textur kommt einmal und bleibt (kein Neuanlegen je Bild).
    #[test]
    fn textur_wird_einmal_angelegt() {
        let ctx = egui::Context::default();
        let a = textur(&ctx);
        let b = textur(&ctx);
        assert_eq!(a.id(), b.id());
        assert_eq!(a.size(), [512, 512]);
    }

    /// Das 32-px-Bild gehört zum Startmenü-Eintrag (`scripts/install-desktop.sh`)
    /// und muss ebenfalls lesbar sein — sonst zeigt die Taskleiste ein Loch.
    #[test]
    fn kleines_icon_fuer_das_startmenue_ist_lesbar() {
        let png = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/jjc-icon-32.png"
        ))
        .expect("assets/jjc-icon-32.png fehlt");
        let b = bild(&png).expect("32-px-Icon muss lesbar sein");
        assert_eq!(b.size, [32, 32]);
    }
}