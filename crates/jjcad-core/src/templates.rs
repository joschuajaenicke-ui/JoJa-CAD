//! Blattvorlagen: Schriftfeld (DIN 6771 angelehnt) und Standard-Layout.

use crate::geom::*;
use crate::model::*;
use crate::style::{Color, PaperSize};

/// Linienstärken im Schriftfeld (Papier-mm)
const LW_THIN: f64 = 0.18;
const LW_NORMAL: f64 = 0.25;
const LW_BOLD: f64 = 0.5;

struct TB {
    next: Id,
    ents: Vec<Entity>,
}

impl TB {
    fn new() -> Self {
        Self {
            next: 1,
            ents: Vec::new(),
        }
    }

    fn line_w(&mut self, a: Vec2, b: Vec2, w: f64) {
        let id = self.next;
        self.next += 1;
        let mut e = Entity::new(id, 0, EntityKind::Line { a, b });
        e.style.color = Some(Color::BLACK);
        e.style.line_weight = Some(w);
        self.ents.push(e);
    }

    fn rect_w(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, w: f64) {
        self.line_w(v(x0, y0), v(x1, y0), w);
        self.line_w(v(x1, y0), v(x1, y1), w);
        self.line_w(v(x1, y1), v(x0, y1), w);
        self.line_w(v(x0, y1), v(x0, y0), w);
    }

    fn text(&mut self, p: Vec2, txt: &str, h: f64, bold: bool) {
        let id = self.next;
        self.next += 1;
        let mut e = Entity::new(
            id,
            0,
            EntityKind::Text {
                p,
                text: txt.to_string(),
                height: h,
                angle: 0.0,
                align: TextAlignH::Left,
                valign: TextAlignV::Top,
                bold,
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
        );
        e.style.color = Some(Color::BLACK);
        e.style.line_weight = Some(if bold { LW_NORMAL } else { LW_THIN });
        self.ents.push(e);
    }
}

/// Schriftfeld am unteren rechten Blattrand.
/// `w`/`h` = Blattmaße, Rahmen: 10 mm Rand, 15 mm Heftrand links.
pub fn title_block(w: f64, h: f64) -> Vec<Entity> {
    let mut t = TB::new();
    let bw = 180.0_f64.min(w - 40.0);
    let bh = 65.0;
    let x0 = w - 10.0 - bw;
    let y0 = 10.0;
    let x1 = x0 + bw;
    let y1 = y0 + bh;

    // Außenrahmen
    t.rect_w(x0, y0, x1, y1, LW_BOLD);

    // Vertikale Trennungen
    let cx = x0 + bw * 0.58;
    t.line_w(v(cx, y0), v(cx, y1), LW_NORMAL);
    // Untere Zeile links dreiteilig
    let cy = y0 + 18.0;
    t.line_w(v(x0, cy), v(x1, cy), LW_NORMAL);
    t.line_w(v(x0, cy + 15.0), v(cx, cy + 15.0), LW_NORMAL);
    t.line_w(v(x0, cy + 30.0), v(cx, cy + 30.0), LW_NORMAL);
    // Rechte Spalte
    t.line_w(v(cx, cy + 15.0), v(x1, cy + 15.0), LW_NORMAL);
    t.line_w(v(cx, cy + 30.0), v(x1, cy + 30.0), LW_NORMAL);
    // Plan-Nr-Zelle
    let nx = x1 - 42.0;
    t.line_w(v(nx, y0), v(nx, cy), LW_NORMAL);
    // Revision
    let rx = x1 - 26.0;
    t.line_w(v(rx, cy), v(rx, y1), LW_THIN);

    // --- Feldtexte ---
    let pad = 2.0;
    // Projekt
    t.text(v(x0 + pad, y1 - pad), "PROJEKT", 1.8, false);
    t.text(v(x0 + pad, y1 - 2.0 - 4.6), "{PROJEKT}", 3.6, true);
    // Bauherr
    t.text(v(x0 + pad, cy + 30.0 - pad), "BAUHERR", 1.8, false);
    t.text(v(x0 + pad, cy + 30.0 - 4.2), "{BAUHERR}", 2.6, false);
    // Adresse
    t.text(v(x0 + pad, cy + 15.0 - pad), "ADRESSE / ORT", 1.8, false);
    t.text(v(x0 + pad, cy + 15.0 - 4.2), "{ADRESSE}", 2.4, false);
    // Planinhalt
    t.text(v(x0 + pad, cy - pad), "PLANINHALT", 1.8, false);
    t.text(v(x0 + pad, cy - 4.2), "{PLANTITEL}", 2.8, true);

    // Planer / Büro (Wert mit Abstand unter der Beschriftung — sonst kleben
    // beide Zeilen bei kleinem Zoom aneinander)
    t.text(v(cx + pad, y1 - pad), "PLANUNG / BÜRO", 1.8, false);
    t.text(v(cx + pad, y1 - 7.0), "{PLANER}", 2.4, false);
    t.text(v(cx + pad, y0 + bh - 12.0), "GEPRÜFT / FREIGABE", 1.5, false);

    // Maßstab
    t.text(v(cx + pad, cy + 30.0 - pad), "MASSSTAB", 1.8, false);
    t.text(v(cx + pad, cy + 30.0 - 4.6), "{MASSTAB}", 4.2, true);
    // Datum
    t.text(v(cx + pad, cy + 15.0 - pad), "DATUM", 1.8, false);
    t.text(v(cx + pad, cy + 15.0 - 4.2), "{DATUM}", 2.6, false);
    // Plan-Nr
    t.text(v(cx + pad, cy - pad), "PLAN-NR.", 1.8, false);
    t.text(v(nx + pad, y0 + 7.0), "{PLAN}", 4.4, true);
    // Revision
    t.text(v(rx + 1.2, y1 - pad), "REV.", 1.5, false);
    t.text(v(rx + 1.2, y1 - 4.4), "{REVISION}", 2.2, false);

    t.ents
}

/// Blattrahmen als vier normale Linien (10 mm Rand, 15 mm Heftrand links).
///
/// Der Rahmen war früher ein Sonderzeichen des Renderers (`Sheet::frame`) und
/// damit weder auswählbar noch bearbeitbar. Als Linien ist er ein Objekt wie
/// jedes andere: anklickbar, mit Griffen, im Eigenschaften-Panel änderbar.
pub fn frame_entities(w: f64, h: f64) -> Vec<Entity> {
    let m = 10.0;
    let left = m + 5.0; // Heftrand
    let ecken = [v(left, m), v(w - m, m), v(w - m, h - m), v(left, h - m)];
    let mut out = Vec::with_capacity(4);
    for i in 0..4 {
        // ID 0: der Aufrufer vergibt sie aus dem Dokument (eindeutig).
        let mut e = Entity::new(0, 0, EntityKind::Line { a: ecken[i], b: ecken[(i + 1) % 4] });
        e.style.color = Some(Color::BLACK);
        e.style.line_weight = Some(0.35);
        out.push(e);
    }
    out
}

/// Neues Blatt inkl. Schriftfeld und einem Ansichtsfenster.
pub fn new_sheet(doc: &mut Document, paper: PaperSize, landscape: bool, name: &str) -> Sheet {
    let id = doc.new_id();
    let (w, h) = paper.dims(landscape);
    let mut sheet = Sheet {
        id,
        name: name.to_string(),
        paper,
        landscape,
        scale: 100.0,
        // Der Rahmen ist ab T0004 ein Objekt (siehe `frame_entities`), kein
        // Renderer-Sonderfall mehr.
        frame: false,
        show_title_block: false,
        viewports_alt: Vec::new(),
        entities: Vec::new(),
        plansatz: false,
    };
    // Rahmen und Schriftfeld bestehen aus normalen Linien und Textfeldern —
    // dieselben Objekte wie in der Zeichnung, nur im Papierraum. Eigene IDs aus
    // dem Dokument, damit sie sich eindeutig auswählen und bearbeiten lassen.
    // Der Rahmen kommt zuerst: er liegt damit hinter dem Schriftfeld.
    sheet.entities = frame_entities(w, h);
    sheet.entities.extend(title_block(w, h));
    for e in sheet.entities.iter_mut() {
        e.id = doc.new_id();
    }

    // Ansichtsfenster über dem Schriftfeld
    let vp_x = 15.0;
    let vp_y = 10.0 + 65.0 + 8.0;
    let vp_w = w - 10.0 - vp_x;
    let vp_h = h - 10.0 - vp_y;
    let bbox = doc.bbox();
    let center = if bbox.is_valid() && bbox.width() > 1.0 {
        bbox.center()
    } else {
        Vec2::ZERO
    };
    let scale = if bbox.is_valid() && bbox.width() > 1.0 {
        // Maßstab so wählen, dass die Zeichnung gut ins Fenster passt
        let need = (bbox.width() / vp_w.max(1.0)).max(bbox.height() / vp_h.max(1.0));
        let nice = [20.0, 25.0, 50.0, 75.0, 100.0, 125.0, 150.0, 200.0, 250.0, 500.0];
        nice.iter()
            .copied()
            .find(|s| *s >= need * 1.05)
            .unwrap_or(500.0)
    } else {
        100.0
    };
    sheet.scale = scale;
    let vp = Viewport {
        id: doc.new_id(),
        name: "Ansicht".to_string(),
        x: vp_x,
        y: vp_y,
        w: vp_w,
        h: vp_h,
        center,
        scale,
        show_frame: false,
        locked: false,
        layers: None,
    };
    // Das Ansichtsfenster ist ein normales Blattobjekt (T0109) — es liegt
    // hinter Rahmen und Schriftfeld, damit beide sichtbar bleiben.
    let mut e = Entity::new(vp.id, 0, EntityKind::Viewport(vp));
    e.style.color = Some(Color::BLACK);
    sheet.entities.insert(0, e);
    sheet
}

/// Standard-Ansichtsfenster (für weitere Blätter).
pub fn default_viewport(doc: &mut Document, w: f64, h: f64) -> Viewport {
    Viewport {
        id: doc.new_id(),
        name: "Ansicht".to_string(),
        x: 15.0,
        y: 10.0 + 73.0,
        w: w - 35.0,
        h: h - 95.0,
        center: Vec2::ZERO,
        scale: 100.0,
        show_frame: true,
        locked: false,
        layers: None,
    }
}
