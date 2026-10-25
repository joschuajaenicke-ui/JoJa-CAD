//! Export-Schnittstelle (PDF, DXF, PNG) — gemeinsame Optionen.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PdfOptions {
    /// Blattindex (0-basiert); None = aktuelles Blatt
    pub sheet: Option<usize>,
    /// Zeichnung als PDF ohne Blatt (nur Modell)
    pub model_only: bool,
    /// Hintergrund weiß (Bauantrag) oder transparent
    pub white_background: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            sheet: None,
            model_only: false,
            white_background: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RasterOptions {
    pub width_px: u32,
    pub height_px: u32,
    /// Weltkoordinaten-Ausschnitt; None = gesamte Zeichnung
    pub bbox: Option<[f64; 4]>,
    /// Papierhintergrund
    pub paper: bool,
    /// Maßstab für Beschriftungsgrößen
    pub annot_scale: Option<f64>,
    /// Blatt rendern statt Modell
    pub sheet: Option<usize>,
}

impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            width_px: 1600,
            height_px: 1200,
            bbox: None,
            paper: true,
            annot_scale: None,
            sheet: None,
        }
    }
}
