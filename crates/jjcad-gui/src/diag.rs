//! Selbstdiagnose für Anzeigefehler auf fremden Rechnern.
//!
//! Auf Windows setzt der Rechner das Fenster anders zusammen als unter Linux
//! (DWM, Grafiktreiber, Skalierung). Ein Fehlerbild wie „weiße 1-px-Streifen
//! zwischen den Panels" lässt sich nur zuordnen, wenn man weiß, **wo** es
//! entsteht: im eigenen Bild, das das Programm gezeichnet hat, oder erst beim
//! Zusammensetzen des Fensters durch das System.
//!
//! Deshalb nimmt `F12` (oder der Start mit `JJCAD_UI_DIAG=1`) den Fensterinhalt
//! so auf, wie er zuletzt gezeichnet wurde, und legt Bild und Kennwerte neben
//! die Programmdatei:
//!
//! ```text
//! jjcad-diag-<zeitstempel>.png   Fensterinhalt 1:1
//! jjcad-diag-<zeitstempel>.txt   Plattform, Skalierung, Grafiktreiber,
//!                                erkannte helle Spalten
//! ```
//!
//! Stehen die Streifen im PNG, liegt der Fehler im Zeichnen; fehlen sie dort,
//! malt das System das Fenster falsch (Treiber, Zusammensetzung, Deckkraft).

use eframe::glow::HasContext;
use std::path::PathBuf;

/// Zustand der Selbstdiagnose.
pub struct Diagnose {
    /// Ein Bildschirmfoto wurde angefordert und ist noch nicht eingetroffen.
    angefragt: bool,
    /// `JJCAD_UI_DIAG=1`: einmalig von selbst aufnehmen.
    automatisch: bool,
    zeit: f64,
}

impl Default for Diagnose {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnose {
    pub fn new() -> Self {
        Self {
            angefragt: false,
            automatisch: std::env::var("JJCAD_UI_DIAG").is_ok(),
            zeit: 0.0,
        }
    }

    /// `F12`: Bildschirmfoto des eigenen Fensters anfordern.
    pub fn tasten(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.anfordern(ctx);
        }
    }

    /// Mit `JJCAD_UI_DIAG=1` ein Bild, sobald die Ansicht eingepasst ist
    /// (damit nicht ein leeres erstes Bild aufgenommen wird).
    pub fn von_selbst(&mut self, ctx: &egui::Context, zeit: f64) {
        if !self.automatisch || self.angefragt {
            return;
        }
        if self.zeit == 0.0 && zeit > 0.0 {
            self.zeit = zeit;
        }
        if zeit > self.zeit + 1.5 {
            self.anfordern(ctx);
        }
    }

    fn anfordern(&mut self, ctx: &egui::Context) {
        // egui liefert das Ergebnis als `Event::Screenshot` im nächsten Bild.
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        self.angefragt = true;
    }

    /// Das eingetroffene Bild auswerten und schreiben. `Some(Ok(pfad))` heißt
    /// gespeichert, `Some(Err(..))` fehlgeschlagen, `None` nichts eingetroffen.
    pub fn einsammeln(
        &mut self,
        ctx: &egui::Context,
        gl: Option<&std::sync::Arc<eframe::glow::Context>>,
    ) -> Option<Result<PathBuf, String>> {
        if !self.angefragt {
            return None;
        }
        let bild = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let bild = bild?;
        self.angefragt = false;
        Some(self.schreiben(ctx, gl, &bild))
    }

    fn schreiben(
        &self,
        ctx: &egui::Context,
        gl: Option<&std::sync::Arc<eframe::glow::Context>>,
        bild: &egui::ColorImage,
    ) -> Result<PathBuf, String> {
        let [w, h] = bild.size;
        let mut rgba = Vec::with_capacity(w * h * 4);
        for c in &bild.pixels {
            rgba.extend_from_slice(&[c.r(), c.g(), c.b(), c.a()]);
        }
        let png = jjcad_core::raster::rgba_png(w as u32, h as u32, &rgba)?;
        let text = self.kennwerte(ctx, gl, w, h, &rgba);
        let ordner = zielordner();
        let stempel = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let png_pfad = ordner.join(format!("jjcad-diag-{stempel}.png"));
        if let Err(e) = std::fs::write(&png_pfad, png) {
            // Zweiter Versuch im temporären Ordner (Programmordner schreibgeschützt).
            let ersatz = std::env::temp_dir();
            let pfad = ersatz.join(format!("jjcad-diag-{stempel}.png"));
            std::fs::write(&pfad, jjcad_core::raster::rgba_png(w as u32, h as u32, &rgba)?)
                .map_err(|e2| format!("{e} / {e2}"))?;
            std::fs::write(
                ersatz.join(format!("jjcad-diag-{stempel}.txt")),
                text,
            )
            .map_err(|e2| format!("{e} / {e2}"))?;
            return Ok(pfad);
        }
        std::fs::write(ordner.join(format!("jjcad-diag-{stempel}.txt")), text)
            .map_err(|e| format!("Kennwerte nicht geschrieben: {e}"))?;
        Ok(png_pfad)
    }

    fn kennwerte(
        &self,
        ctx: &egui::Context,
        gl: Option<&std::sync::Arc<eframe::glow::Context>>,
        w: usize,
        h: usize,
        rgba: &[u8],
    ) -> String {
        let mut s = String::new();
        s.push_str(jjcad_core::i18n::tr("gui.diag.joja_cad_selbstdiagnose_der_oberflaeche"));
        s.push_str("=======================================\n");
        s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.version_x_build_x_x", &[("wert1", format!("{}", env!("CARGO_PKG_VERSION"))), ("wert2", format!("{}", env!("JJCAD_BUILD"))), ("wert3", format!("{}", env!("JJCAD_HASH")))]));
        s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.plattform_x_x_windows_version_x", &[("wert1", format!("{}", std::env::consts::OS)), ("wert2", format!("{}", std::env::consts::ARCH)), ("wert3", format!("{}", windows_version()))]));
        s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.fenster_x_x_x_bildpunkte", &[("w", format!("{}", w)), ("h", format!("{}", h))]));
        s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.skalierung_pixels_per_point_x_anzeige_x", &[("wert1", format!("{:.4}", ctx.pixels_per_point())), ("wert2", format!("{:.0}", ctx.pixels_per_point() * 100.0))]));
        s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.ansichtsflaeche_x_x_x", &[("wert1", format!("{:.2}", ctx.screen_rect().width())), ("wert2", format!("{:.2}", ctx.screen_rect().height()))]));
        s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.bildrate_interval_vsync_x", &[("wert1", format!("{}", if vsync() { "an" } else { "aus" }))]));
        if let Some(gl) = gl {
            unsafe {
                s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.grafiktreiber_x_x_x", &[("wert1", format!("{}", gl.get_parameter_string(eframe::glow::VENDOR))), ("wert2", format!("{}", gl.get_parameter_string(eframe::glow::RENDERER))), ("wert3", format!("{}", gl.get_parameter_string(eframe::glow::VERSION)))]));
            }
        } else {
            s.push_str(jjcad_core::i18n::tr("gui.diag.grafiktreiber_unbekannt_kein_glow_kontext"));
        }

        s.push_str(jjcad_core::i18n::tr("gui.diag.helle_1_px_spalten_im_eigenen_bild"));
        s.push_str("---------------------------------\n");
        let (spalten, hinweis) = helle_spalten(rgba, w, h);
        s.push_str(&hinweis);
        if !spalten.is_empty() {
            s.push_str(jjcad_core::i18n::tr("gui.diag.die_streifen_stecken_also_im_bild_das_dieses_programm_ze"));
            for (x, anteil, farbe) in spalten.iter().take(24) {
                s.push_str(&jjcad_core::i18n::t_args_owned("gui.diag.spalte_x_x_x_der_zeilen_hell_farbe_xxx", &[("x", format!("{}", x)), ("wert1", format!("{:.0}", anteil * 100.0)), ("wert2", format!("{:02X}", farbe[0])), ("wert3", format!("{:02X}", farbe[1])), ("wert4", format!("{:02X}", farbe[2]))]));
            }
            s.push_str(
                jjcad_core::i18n::tr("gui.diag.bitte_das_png_nicht_das_betriebssystem_fenster_an_das_ti"),
            );
        }
        s
    }
}

/// Wo die Bilder abgelegt werden: neben der Programmdatei, sonst im
/// temporären Ordner.
fn zielordner() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            return d.to_path_buf();
        }
    }
    std::env::temp_dir()
}

/// `JJCAD_NO_VSYNC=1` schaltet die Bildsynchronisation in `main()` ab.
pub fn vsync() -> bool {
    std::env::var("JJCAD_NO_VSYNC").is_err()
}

/// Windows-Version im Klartext (nur dort sinnvoll, sonst leer).
fn windows_version() -> String {
    #[cfg(target_os = "windows")]
    {
        // RtlGetVersion liefert die echte Version — GetVersionEx wäre ab
        // Windows 8.1 nur noch die Kompatibilitätsangabe.
        let mut info = WinVersionInfo {
            dw_os_version_info_size: std::mem::size_of::<WinVersionInfo>() as u32,
            dw_major_version: 0,
            dw_minor_version: 0,
            dw_build_number: 0,
            dw_platform_id: 0,
            sz_csd_version: [0; 128],
            w_service_pack_major: 0,
            w_service_pack_minor: 0,
            w_suite_mask: 0,
            w_product_type: 0,
            w_reserved: 0,
            dw_extended_platform_id: 0,
        };
        extern "system" {
            fn RtlGetVersion(info: *mut WinVersionInfo) -> i32;
        }
        let status = unsafe { RtlGetVersion(&mut info) };
        if status == 0 {
            return format!(
                "{}.{} (Build {})",
                info.dw_major_version,
                info.dw_minor_version,
                info.dw_build_number & 0xFFFF
            );
        }
        "unbekannt".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        String::new()
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinVersionInfo {
    dw_os_version_info_size: u32,
    dw_major_version: u32,
    dw_minor_version: u32,
    dw_build_number: u32,
    dw_platform_id: u32,
    sz_csd_version: [u16; 128],
    w_service_pack_major: u16,
    w_service_pack_minor: u16,
    w_suite_mask: u16,
    w_product_type: u8,
    w_reserved: u8,
    dw_extended_platform_id: u32,
}

/// Sucht Spalten, die über einen großen Teil der Höhe heller sind als ihre
/// Nachbarn — das Muster dünner Fugen zwischen zwei Flächen. Text und Symbole
/// liefern ebenfalls helle Punkte, aber nie über viele Zeilen hinweg an
/// derselben Spalte.
fn helle_spalten(rgba: &[u8], w: usize, h: usize) -> (Vec<(usize, f32, [u8; 3])>, String) {
    if w < 3 || h < 3 {
        return (Vec::new(), "Bild zu klein für die Suche.\n".to_string());
    }
    let lum = |x: usize, y: usize| -> f32 {
        let i = (y * w + x) * 4;
        0.299 * rgba[i] as f32 + 0.587 * rgba[i + 1] as f32 + 0.114 * rgba[i + 2] as f32
    };
    // Zeilen oben und unten auslassen: Fensterrahmen und Titelzeile.
    let y0 = h / 50;
    let y1 = h - h / 50;
    let zeilen = (y1 - y0).max(1) as f32;
    let mut treffer: Vec<(usize, f32, [u8; 3])> = Vec::new();
    for x in 1..w - 1 {
        let mut hell = 0usize;
        let mut summe_rgb = [0u32; 3];
        for y in y0..y1 {
            let l = lum(x, y);
            if l > 170.0 && lum(x - 1, y) < 110.0 && lum(x + 1, y) < 110.0 {
                hell += 1;
                let i = (y * w + x) * 4;
                for k in 0..3 {
                    summe_rgb[k] += rgba[i + k] as u32;
                }
            }
        }
        let anteil = hell as f32 / zeilen;
        if hell > 0 && anteil > 0.15 {
            let farbe = [
                (summe_rgb[0] / hell as u32).min(255) as u8,
                (summe_rgb[1] / hell as u32).min(255) as u8,
                (summe_rgb[2] / hell as u32).min(255) as u8,
            ];
            treffer.push((x, anteil, farbe));
        }
    }
    treffer.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let hinweis = if treffer.is_empty() {
        "Keine durchgehenden hellen Spalten gefunden — die Streifen stehen nicht\nim eigenen Bild; sie entstehen beim Zusammensetzen des Fensters.\n"
            .to_string()
    } else {
        String::new()
    };
    (treffer, hinweis)
}