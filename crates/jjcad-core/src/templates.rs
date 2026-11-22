//! Blattvorlagen: Schriftfeld (DIN 6771 angelehnt) und Standard-Layout.

use crate::geom::*;
use crate::model::*;
use crate::style::{Color, FillPattern, PaperSize};

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

/// Rechteck des Schriftfelds am unteren rechten Blattrand (Papier-mm).
///
/// **Eine** Quelle für die Felder des Schriftfelds und die weiße Abdeckfläche
/// dahinter (T0239) — sonst läge die Fläche um ein Feldmaß daneben.
/// `w`/`h` = Blattmaße, Rahmen: 10 mm Rand, 15 mm Heftrand links.
pub fn title_block_rect(w: f64, _h: f64) -> (f64, f64, f64, f64) {
    let bw = 180.0_f64.min(w - 40.0);
    let bh = 65.0;
    let x0 = w - 10.0 - bw;
    let y0 = 10.0;
    (x0, y0, x0 + bw, y0 + bh)
}

/// Schriftfeld am unteren rechten Blattrand.
/// `w`/`h` = Blattmaße, Rahmen: 10 mm Rand, 15 mm Heftrand links.
pub fn title_block(w: f64, h: f64) -> Vec<Entity> {
    let mut t = TB::new();
    let (x0, y0, x1, y1) = title_block_rect(w, h);
    let bw = x1 - x0;
    let bh = y1 - y0;

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

/// Weiße Abdeckfläche hinter dem Schriftfeld (T0239).
///
/// Sie liegt im Planrahmen **über** dem Inhalt der Ansichtsfenster und unter
/// den Linien und Texten des Schriftfelds: ein Ansichtsfenster darf damit das
/// ganze Blatt füllen, ohne dass die darunter liegende Zeichnung den Plankopf
/// unlesbar macht. Deckungsgleich mit [`title_block_rect`], sonst bliebe ein
/// Streifen der Zeichnung neben dem Schriftfeld stehen.
pub fn abdeckung(w: f64, h: f64) -> Entity {
    let (x0, y0, x1, y1) = title_block_rect(w, h);
    let mut e = Entity::new(0, 0, EntityKind::Fill {
        boundary: vec![v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)],
        holes: Vec::new(),
        pattern: FillPattern::Solid,
        pattern_angle: 0.0,
        pattern_scale: 100.0,
        border: false,
    });
    e.style.fill = Some(Color::WHITE);
    e.by_block = true;
    e
}

/// Der ganze Planrahmen als **ein** Objekt: Abdeckfläche, Blattrahmen und
/// Schriftfeld in dieser Reihenfolge (hinten nach vorn, T0239).
///
/// Die Kennungen laufen blocklokal von 1 aufwärts — dieselbe Form, die
/// [`crate::blocks_lib`] für seine Blöcke vergibt. Der Ursprung ist die **untere
/// linke Blattecke** (0,0), damit der Basispunkt des Blocks genau dort liegt
/// und das Einfügen ohne Versatz passt.
pub fn planrahmen_entities(w: f64, h: f64) -> Vec<Entity> {
    let mut out = Vec::new();
    out.push(abdeckung(w, h));
    out.extend(frame_entities(w, h));
    out.extend(title_block(w, h));
    for (i, e) in out.iter_mut().enumerate() {
        e.id = i as Id + 1;
    }
    out
}

/// Systemblock-Kennung des Planrahmens für ein Papierformat (`None` = für
/// dieses Format gibt es keinen Block, dann legt das Blatt lose Objekte an).
pub fn planrahmen_block(w: f64, h: f64) -> Option<&'static str> {
    let (bw, bh) = (297.0, 420.0); // A3 hoch
    let (qw, qh) = (420.0, 297.0); // A3 quer
    let (vw, vh) = (210.0, 297.0); // A4 hoch
    let nah = |a: f64, b: f64| (a - b).abs() < 0.5;
    if nah(w, qw) && nah(h, qh) {
        Some(crate::model::RAHMEN_A3_QUER)
    } else if nah(w, bw) && nah(h, bh) {
        Some(crate::model::RAHMEN_A3_HOCH)
    } else if nah(w, vw) && nah(h, vh) {
        Some(crate::model::RAHMEN_A4_HOCH)
    } else {
        None
    }
}

/// Neues Blatt inkl. Planrahmen (ein Block, T0239) und einem Ansichtsfenster.
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
    // Der ganze Planrahmen (Abdeckfläche, Blattrahmen, Schriftfeld) ist **ein**
    // Objekt (T0239): ein Block, dessen Basispunkt die untere linke Blattecke
    // ist. Er steht bei (0,0) und lässt sich als Ganzes wählen, verschieben
    // und löschen. Für Papierformate ohne Block bleiben lose Objekte.
    match planrahmen_block(w, h) {
        Some(block) => {
            let mut e = Entity::new(
                doc.new_id(),
                0,
                EntityKind::BlockRef {
                    block: block.to_string(),
                    pos: Vec2::ZERO,
                    rot: 0.0,
                    scale: Vec2::new(1.0, 1.0),
                    mirror: false,
                },
            );
            e.style.color = Some(Color::BLACK);
            sheet.entities.push(e);
        }
        None => {
            // Dieselben Objekte wie der Block, nur lose: eigene IDs aus dem
            // Dokument, damit sie sich eindeutig auswählen lassen.
            let mut rahmen = planrahmen_entities(w, h);
            for e in rahmen.iter_mut() {
                e.id = doc.new_id();
            }
            sheet.entities.extend(rahmen);
        }
    }

    // Ansichtsfenster über dem ganzen Blatt (T0239): Rahmen und Schriftfeld
    // liegen darüber, dessen weiße Abdeckfläche hält den Plankopf lesbar —
    // die Zeichnung darf deshalb bis in die Ecke laufen.
    let vp_x = 15.0;
    let vp_y = 10.0;
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
        // Über dem ganzen Blatt — der Plankopf liegt als Block darüber.
        x: 15.0,
        y: 10.0,
        w: w - 25.0,
        h: h - 20.0,
        center: Vec2::ZERO,
        scale: 100.0,
        show_frame: true,
        locked: false,
        layers: None,
    }
}
