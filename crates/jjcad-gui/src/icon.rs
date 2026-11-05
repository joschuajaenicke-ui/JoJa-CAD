//! Das Markenzeichen der Anwendung — zwei übereinanderliegende Blätter auf
//! einer Kachel, „Jo“ im Akzentton, „Ja“ in der Schriftfarbe (T0113).
//!
//! Die Bilddateien liegen in `crates/jjcad-gui/assets/` (Kopie aus
//! `branding/icon/` im Projektordner) und kommen per `include_bytes!` mit ins
//! Programm: der Testkanal besteht aus einer einzigen Datei, eine Suche im
//! Projektordner fände dort nichts. Dasselbe Muster wie bei den technischen
//! Schriften in `jjcad-core` (`fonts.rs`).
//!
//! **Zwei Aufgaben, zwei Sätze Bilddaten:**
//!
//! * Die **ausführbare Datei** trägt fest Bernstein/dunkel — `jjc-icon-512.png`
//!   (Startmenü, `scripts/install-desktop.sh`), `jjc-icon-128.png`
//!   (Fenstericon) und `jjc-icon-32.png` (kleine Rasterung). Ein Symbol, das
//!   mit der Einstellung des Nutzers wechselt, wäre in Taskleiste und
//!   Startmenü falsch.
//! * **Kopfzeile und Über-Dialog** folgen Akzent und Design aus `theme.rs`:
//!   `a-<akzent>-<design>-512.png` bzw. `-32.png`, je Wahl eine eigene Datei.
//!   Die Textur wird über den Dateinamen gecacht, ein Wechsel von Akzent oder
//!   Design greift damit im nächsten Bild — nichts wird beim Start
//!   eingefroren.
//!
//! Wer eine dritte Stelle braucht, nimmt [`textur`]/[`textur_klein`] bzw.
//! [`fenster_icon`] — keinen zweiten Satz Bilddaten und kein gezeichnetes
//! Ersatzicon (das war der alte Zustand: 32 px, im Code gemalt, ohne Bezug
//! zum Markenzeichen).

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use jjcad_core::persist::Farbakzent;

/// Das Markenzeichen der ausführbaren Datei in 512 × 512 Bildpunkten
/// (Bernstein/dunkel, Hintergrund durchsichtig).
pub const ICON_512: &[u8] = include_bytes!("../assets/jjc-icon-512.png");

/// Dieselbe Marke in 32 × 32 — für Startmenü und Taskleiste gerastert, damit
/// die weißen Linien dort nicht zu Grau verwaschen (T0083).
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

/// Die Bilddatei zu Akzent, Design und Größe.
///
/// Gibt `(Dateiname ohne Endung, PNG-Bytes)` zurück. Der Dateiname ist
/// zugleich der Schlüssel der Textur — eine zweite Zuordnung (Akzent → Kürzel)
/// gibt es damit nicht.
fn datei(akzent: Farbakzent, hell: bool, gross: bool) -> (&'static str, &'static [u8]) {
    match (akzent, hell, gross) {
        (Farbakzent::Bernstein, false, true) => (
            "a-bernstein-dunkel-512",
            include_bytes!("../assets/a-bernstein-dunkel-512.png"),
        ),
        (Farbakzent::Bernstein, false, false) => (
            "a-bernstein-dunkel-32",
            include_bytes!("../assets/a-bernstein-dunkel-32.png"),
        ),
        (Farbakzent::Bernstein, true, true) => (
            "a-bernstein-hell-512",
            include_bytes!("../assets/a-bernstein-hell-512.png"),
        ),
        (Farbakzent::Bernstein, true, false) => (
            "a-bernstein-hell-32",
            include_bytes!("../assets/a-bernstein-hell-32.png"),
        ),
        (Farbakzent::Saphir, false, true) => (
            "a-saphir-dunkel-512",
            include_bytes!("../assets/a-saphir-dunkel-512.png"),
        ),
        (Farbakzent::Saphir, false, false) => (
            "a-saphir-dunkel-32",
            include_bytes!("../assets/a-saphir-dunkel-32.png"),
        ),
        (Farbakzent::Saphir, true, true) => (
            "a-saphir-hell-512",
            include_bytes!("../assets/a-saphir-hell-512.png"),
        ),
        (Farbakzent::Saphir, true, false) => (
            "a-saphir-hell-32",
            include_bytes!("../assets/a-saphir-hell-32.png"),
        ),
        (Farbakzent::Smaragd, false, true) => (
            "a-smaragd-dunkel-512",
            include_bytes!("../assets/a-smaragd-dunkel-512.png"),
        ),
        (Farbakzent::Smaragd, false, false) => (
            "a-smaragd-dunkel-32",
            include_bytes!("../assets/a-smaragd-dunkel-32.png"),
        ),
        (Farbakzent::Smaragd, true, true) => (
            "a-smaragd-hell-512",
            include_bytes!("../assets/a-smaragd-hell-512.png"),
        ),
        (Farbakzent::Smaragd, true, false) => (
            "a-smaragd-hell-32",
            include_bytes!("../assets/a-smaragd-hell-32.png"),
        ),
        (Farbakzent::Rubin, false, true) => (
            "a-rubin-dunkel-512",
            include_bytes!("../assets/a-rubin-dunkel-512.png"),
        ),
        (Farbakzent::Rubin, false, false) => (
            "a-rubin-dunkel-32",
            include_bytes!("../assets/a-rubin-dunkel-32.png"),
        ),
        (Farbakzent::Rubin, true, true) => (
            "a-rubin-hell-512",
            include_bytes!("../assets/a-rubin-hell-512.png"),
        ),
        (Farbakzent::Rubin, true, false) => (
            "a-rubin-hell-32",
            include_bytes!("../assets/a-rubin-hell-32.png"),
        ),
        (Farbakzent::Amethyst, false, true) => (
            "a-amethyst-dunkel-512",
            include_bytes!("../assets/a-amethyst-dunkel-512.png"),
        ),
        (Farbakzent::Amethyst, false, false) => (
            "a-amethyst-dunkel-32",
            include_bytes!("../assets/a-amethyst-dunkel-32.png"),
        ),
        (Farbakzent::Amethyst, true, true) => (
            "a-amethyst-hell-512",
            include_bytes!("../assets/a-amethyst-hell-512.png"),
        ),
        (Farbakzent::Amethyst, true, false) => (
            "a-amethyst-hell-32",
            include_bytes!("../assets/a-amethyst-hell-32.png"),
        ),
    }
}

/// Dekodiert ein eingebettetes PNG in unmultiplizierte RGBA-Bytes.
///
/// Gibt `(Breite, Höhe, Bytes)` zurück; die Bytes sind immer vier je Bildpunkt,
/// auch wenn das PNG eine Palette oder Graustufen trägt (`Transformations`).
fn rgba(png: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder
        .read_info()
        .map_err(|e| jjcad_core::i18n::t_args_owned("gui.icon.markenzeichen_nicht_lesbar", &[("fehler", format!("{}", e))]))?;
    let mut puffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut puffer)
        .map_err(|e| jjcad_core::i18n::t_args_owned("gui.icon.markenzeichen_nicht_lesbar", &[("fehler", format!("{}", e))]))?;
    let bytes = puffer[..info.buffer_size()].to_vec();
    if bytes.len() != (info.width as usize) * (info.height as usize) * 4 {
        return Err(jjcad_core::i18n::tr("gui.icon.markenzeichen_puffergroesse").into());
    }
    Ok((info.width as usize, info.height as usize, bytes))
}

/// Das Markenzeichen als egui-Bild (für Texturen und Vergleiche).
pub fn bild(png: &[u8]) -> Result<ColorImage, String> {
    let (w, h, bytes) = rgba(png)?;
    Ok(ColorImage::from_rgba_unmultiplied([w, h], &bytes))
}

/// Fenstericon für Taskleiste und Fenstertitel — fest Bernstein/dunkel.
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

/// Textur des Markenzeichens in 512 px (Über-Dialog) — folgt Akzent und Design.
pub fn textur(ctx: &egui::Context) -> TextureHandle {
    textur_aus(ctx, true)
}

/// Dieselbe Marke in der kleinen Rasterung (Kopfzeile, 30 px) — folgt Akzent
/// und Design.
pub fn textur_klein(ctx: &egui::Context) -> TextureHandle {
    textur_aus(ctx, false)
}

/// Eine Textur je Datei und Kontext — `load_texture` bei jedem Bild würde bei
/// offenem Dialog jede Frame eine neue Textur anlegen. Der Name enthält Akzent,
/// Design und Größe; ein Wechsel legt die passende Textur an, statt die alte
/// weiterzureichen.
fn textur_aus(ctx: &egui::Context, gross: bool) -> TextureHandle {
    let (name, png) = datei(crate::theme::aktueller_akzent(), crate::theme::ist_hell(), gross);
    let id = egui::Id::new(("jjcad-markenzeichen", name));
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

    /// Zählt Bildpunkte, die zum Bernstein-Schriftzug „Jo" gehören.
    fn bernstein(b: &ColorImage) -> usize {
        b.pixels
            .iter()
            .filter(|p| p.a() == 255 && p.r() > 190 && (120..200).contains(&p.g()) && p.b() < 110)
            .count()
    }

    /// Zählt die dunklen Bildpunkte der Kachel.
    fn kachel_dunkel(b: &ColorImage) -> usize {
        b.pixels
            .iter()
            .filter(|p| p.a() == 255 && p.r() < 45 && p.g() < 45 && p.b() < 50)
            .count()
    }

    /// Die mitgelieferte Datei ist das neue Markenzeichen (T0113): 512 × 512,
    /// durchsichtige Ecken, dunkle Kachel, Bernstein-Schriftzug „Jo" — als
    /// Pixelprobe am geladenen Bild, nicht als Farbkonstante behauptet.
    #[test]
    fn eingebettetes_bild_ist_das_markenzeichen() {
        let b = bild(ICON_512).expect("Markenzeichen muss lesbar sein");
        assert_eq!(b.size, [512, 512], "Größe aus branding/icon/a-bernstein-dunkel-512.png");

        let pixel = |x: usize, y: usize| b.pixels[y * b.size[0] + x];
        // Ecken durchsichtig (abgerundete Kachel, kein Kasten dahinter) …
        assert_eq!(pixel(0, 0).a(), 0, "Ecke links oben ist durchsichtig");
        assert_eq!(pixel(511, 0).a(), 0, "Ecke rechts oben ist durchsichtig");
        assert_eq!(pixel(0, 511).a(), 0, "Ecke links unten ist durchsichtig");
        assert_eq!(pixel(511, 511).a(), 0, "Ecke rechts unten ist durchsichtig");
        // … die dunkle Kachel …
        let dunkel = kachel_dunkel(&b);
        assert!(dunkel > 20_000, "dunkle Kachel fehlt (nur {dunkel} Bildpunkte)");
        // … und der Bernstein-Schriftzug „Jo" auf dem vorderen Blatt.
        let amber = bernstein(&b);
        assert!(amber > 3_000, "Bernstein-Schriftzug fehlt (nur {amber} Bildpunkte)");
    }

    #[test]
    fn fenstericon_ist_das_markenzeichen() {
        let i = fenster_icon();
        assert_eq!((i.width, i.height), (128, 128));
        assert_eq!(i.rgba.len(), 128 * 128 * 4);
    }

    /// Die Textur kommt einmal und bleibt (kein Neuanlegen je Bild).
    ///
    /// Akzent und Design werden im **eigenen Faden** festgelegt: die Wahl ist
    /// prozessweit, und ein parallel laufender Test, der sie umstellt, ließe
    /// zwischen den beiden Aufrufen eine andere Datei (und damit eine andere
    /// Textur) entstehen — genau die Flakiness aus T0099.
    #[test]
    fn textur_wird_einmal_angelegt() {
        let ctx = egui::Context::default();
        crate::theme::set_akzent(Farbakzent::Bernstein, &ctx);
        crate::theme::set_design(jjcad_core::persist::Design::Dunkel, &ctx);
        let a = textur(&ctx);
        let b = textur(&ctx);
        assert_eq!(a.id(), b.id());
        assert_eq!(a.size(), [512, 512]);
    }

    /// Jede Wahl aus Akzent und Design ist eingebettet, in beiden Größen und
    /// ohne Hochskalierung: 512 px für den Über-Dialog, 32 px für die Kopfzeile.
    #[test]
    fn alle_akzente_und_designs_sind_eingebettet() {
        for akzent in Farbakzent::ALL {
            for hell in [false, true] {
                for (gross, kante) in [(true, 512), (false, 32)] {
                    let (name, png) = datei(akzent, hell, gross);
                    let b = bild(png).unwrap_or_else(|e| panic!("{name}: {e}"));
                    assert_eq!(b.size, [kante, kante], "{name} hat die falsche Größe");
                    assert_eq!(
                        b.pixels[0].a(),
                        0,
                        "{name}: Ecke links oben muss durchsichtig sein"
                    );
                }
            }
        }
    }

    /// Das Symbol folgt Akzent und Design: Bernstein/dunkel trägt den
    /// Bernstein-Schriftzug, Saphir nicht; das helle Design hat keine dunkle
    /// Kachel. Die Textur wechselt im nächsten Bild, weil ihr Name die Wahl
    /// enthält.
    #[test]
    fn symbol_folgt_akzent_und_design() {
        let ctx = egui::Context::default();
        crate::theme::set_akzent(Farbakzent::Bernstein, &ctx);
        crate::theme::set_design(jjcad_core::persist::Design::Dunkel, &ctx);
        let bernstein_tex = textur(&ctx);
        let bernstein_bild = bild(datei(Farbakzent::Bernstein, false, true).1).unwrap();
        assert!(bernstein(&bernstein_bild) > 3_000, "Bernstein fehlt im dunklen Design");

        crate::theme::set_akzent(Farbakzent::Saphir, &ctx);
        let saphir_tex = textur(&ctx);
        assert_ne!(
            bernstein_tex.id(),
            saphir_tex.id(),
            "die Textur muss beim Akzentwechsel neu angelegt werden"
        );
        let saphir_bild = bild(datei(Farbakzent::Saphir, false, true).1).unwrap();
        assert_eq!(bernstein(&saphir_bild), 0, "Saphir trägt keinen Bernstein-Schriftzug");

        crate::theme::set_design(jjcad_core::persist::Design::Hell, &ctx);
        let hell_tex = textur(&ctx);
        assert_ne!(saphir_tex.id(), hell_tex.id(), "helles Design ist eine eigene Textur");
        let hell_bild = bild(datei(Farbakzent::Saphir, true, true).1).unwrap();
        assert_eq!(kachel_dunkel(&hell_bild), 0, "helles Design hat keine dunkle Kachel");

        // Ausgangslage für die übrigen Tests wiederherstellen.
        crate::theme::set_akzent(Farbakzent::Bernstein, &ctx);
        crate::theme::set_design(jjcad_core::persist::Design::Dunkel, &ctx);
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
