//! Vektoricons — mit dem Painter gezeichnet, damit sie in jeder Größe scharf sind.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

pub struct IconPainter<'a> {
    p: &'a Painter,
    r: Rect,
    color: Color32,
    w: f32,
}

impl<'a> IconPainter<'a> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        let s = self.r.width() / 24.0;
        Pos2::new(self.r.min.x + x * s, self.r.min.y + y * s)
    }

    fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.p.line_segment(
            [self.pt(x1, y1), self.pt(x2, y2)],
            Stroke::new(self.w, self.color),
        );
    }

    fn poly(&self, pts: &[(f32, f32)], close: bool) {
        let mut v: Vec<Pos2> = pts.iter().map(|(x, y)| self.pt(*x, *y)).collect();
        if close {
            v.push(self.pt(pts[0].0, pts[0].1));
        }
        for pair in v.windows(2) {
            self.p
                .line_segment([pair[0], pair[1]], Stroke::new(self.w, self.color));
        }
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) {
        self.p.rect_stroke(
            Rect::from_min_size(self.pt(x, y), Vec2::new(w, h) * (self.r.width() / 24.0)),
            egui::Rounding::same(1.0),
            Stroke::new(self.w, self.color),
        );
    }

    fn circle(&self, cx: f32, cy: f32, r: f32) {
        self.p.circle_stroke(
            self.pt(cx, cy),
            r * (self.r.width() / 24.0),
            Stroke::new(self.w, self.color),
        );
    }

    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let n = 18;
        let mut pts = Vec::new();
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
        self.poly(&pts, false);
    }

    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(
            self.pt(x, y),
            r * (self.r.width() / 24.0),
            self.color,
        );
    }

    fn filled(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.pt(*x, *y)).collect();
        self.p.add(egui::Shape::convex_polygon(
            v,
            self.color,
            Stroke::NONE,
        ));
    }

    /// Pfeil aus Schaft und gefülltem Kopf — kräftig genug für die 16 px
    /// Icons im Tastenbild. `arrow_head` hat einen weiten Öffnungswinkel und
    /// verschwindet dort fast (T0056).
    fn pfeil(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.line(x1, y1, x2, y2);
        let ang = (y2 - y1).atan2(x2 - x1);
        let groesse = 7.5;
        let halb = 0.6;
        self.filled(&[
            (x2, y2),
            (
                x2 - groesse * (ang - halb).cos(),
                y2 - groesse * (ang - halb).sin(),
            ),
            (
                x2 - groesse * (ang + halb).cos(),
                y2 - groesse * (ang + halb).sin(),
            ),
        ]);
    }

    fn arrow_head(&self, x: f32, y: f32, ang: f32, size: f32) {
        let a1 = ang + 2.6;
        let a2 = ang - 2.6;
        self.filled(&[
            (x, y),
            (x + size * a1.cos(), y + size * a1.sin()),
            (x + size * a2.cos(), y + size * a2.sin()),
        ]);
    }
}

/// Zeichnet ein Icon in `rect`.
pub fn paint(painter: &Painter, rect: Rect, name: &str, color: Color32) {
    let w = (rect.width() / 13.0).clamp(1.1, 2.0);
    let i = IconPainter {
        p: painter,
        r: rect,
        color,
        w,
    };
    match name {
        "line" => {
            i.line(4.0, 19.0, 20.0, 5.0);
            i.dot(4.0, 19.0, 1.6);
            i.dot(20.0, 5.0, 1.6);
        }
        "polyline" => {
            i.poly(&[(3.0, 18.0), (9.0, 8.0), (15.0, 15.0), (21.0, 6.0)], false);
            i.dot(3.0, 18.0, 1.3);
            i.dot(21.0, 6.0, 1.3);
        }
        "spline" => {
            // Kurve durch vier Stützpunkte — Gegenstück zum Polygonzug-Icon.
            let stoff = [(3.0f32, 18.0f32), (9.0, 7.0), (15.0, 16.0), (21.0, 6.0)];
            let mut pts: Vec<(f32, f32)> = Vec::new();
            for k in 0..=24 {
                let t = k as f32 / 24.0 * 3.0;
                let i = (t.floor() as usize).min(2);
                let f = t - i as f32;
                let p0 = stoff[i.saturating_sub(1)];
                let p1 = stoff[i];
                let p2 = stoff[i + 1];
                let p3 = stoff[(i + 2).min(3)];
                let h00 = 2.0 * f * f * f - 3.0 * f * f + 1.0;
                let h10 = f * f * f - 2.0 * f * f + f;
                let h01 = -2.0 * f * f * f + 3.0 * f * f;
                let h11 = f * f * f - f * f;
                pts.push((
                    h00 * p1.0 + h10 * (p2.0 - p0.0) * 0.5 + h01 * p2.0 + h11 * (p3.0 - p1.0) * 0.5,
                    h00 * p1.1 + h10 * (p2.1 - p0.1) * 0.5 + h01 * p2.1 + h11 * (p3.1 - p1.1) * 0.5,
                ));
            }
            i.poly(&pts, false);
            for (x, y) in stoff {
                i.dot(x, y, 1.2);
            }
        }
        "circle" => {
            i.circle(12.0, 12.0, 7.5);
            i.dot(12.0, 12.0, 1.1);
        }
        "arc" => {
            i.arc(12.0, 15.0, 8.0, 3.34, 6.08);
            i.dot(4.6, 12.5, 1.2);
            i.dot(19.4, 12.5, 1.2);
        }
        "ellipse" => {
            i.p.add(egui::Shape::line(
                (0..=28)
                    .map(|k| {
                        let a = k as f32 / 28.0 * std::f32::consts::TAU;
                        let s = i.r.width() / 24.0;
                        Pos2::new(i.r.min.x + 12.0 * s + 9.0 * a.cos() * s, i.r.min.y + 12.0 * s + 6.5 * a.sin() * s)
                    })
                    .collect::<Vec<_>>(),
                Stroke::new(i.w, color),
            ));
        }
        "rect" => i.rect(4.0, 6.0, 16.0, 12.0),
        "point" => {
            i.dot(12.0, 12.0, 2.2);
            i.line(12.0, 4.0, 12.0, 8.0);
            i.line(12.0, 16.0, 12.0, 20.0);
            i.line(4.0, 12.0, 8.0, 12.0);
            i.line(16.0, 12.0, 20.0, 12.0);
        }
        // Füllflächen und Schraffuren: je drei Eingabemodi, unterschieden durch
        // die Fuellung (voll) bzw. Schraffur (Striche) im Inneren.
        // Füllflächen (Vollton) und Schraffuren (Striche): je drei Eingabemodi.
        "fill-contour" | "hatch-contour" => {
            let quad = [(4.0, 19.0), (4.0, 6.0), (20.0, 6.0), (20.0, 19.0)];
            if name.starts_with("hatch") {
                i.poly(&quad, true);
                for k in 0..4 {
                    i.line(5.5 + k as f32 * 4.0, 18.5, 9.5 + k as f32 * 4.0, 6.5);
                }
            } else {
                i.filled(&quad);
                i.poly(&quad, true);
            }
            // Kontur anklicken: Ring am linken Rand
            i.circle(4.0, 19.0, 3.0);
        }
        "fill-points" | "hatch-points" => {
            let quad = [(4.0, 19.0), (4.0, 6.0), (20.0, 6.0), (20.0, 19.0)];
            if name.starts_with("hatch") {
                i.poly(&quad, true);
                for k in 0..4 {
                    i.line(5.5 + k as f32 * 4.0, 18.5, 9.5 + k as f32 * 4.0, 6.5);
                }
            } else {
                i.filled(&quad);
                i.poly(&quad, true);
            }
            for (x, y) in [(4.0, 19.0), (4.0, 6.0), (20.0, 6.0)] {
                i.dot(x, y, 2.0);
            }
        }
        "fill-paint" | "hatch-paint" => {
            // Wie Kontur/Punkte: die Fläche im Hintergrund (Vollton bzw.
            // Schraffur) — unten links sitzt ein kleiner Farbeimer als Abzeichen.
            let quad = [(4.0, 19.0), (4.0, 6.0), (20.0, 6.0), (20.0, 19.0)];
            if name.starts_with("hatch") {
                i.poly(&quad, true);
                for k in 0..4 {
                    i.line(5.5 + k as f32 * 4.0, 18.5, 9.5 + k as f32 * 4.0, 6.5);
                }
            } else {
                i.filled(&quad);
                i.poly(&quad, true);
            }
            // Abzeichen gross genug, um bei 22 px lesbar zu sein: Kontrastplatte
            // unten links, darauf ein Eimer mit Buegel
            let plate = IconPainter {
                p: i.p,
                r: i.r,
                color: crate::theme::Palette::BG,
                w: i.w,
            };
            plate.filled(&[(1.0, 22.0), (13.5, 22.0), (13.5, 10.0), (1.0, 10.0)]);
            let body = [(3.0, 21.0), (12.0, 21.0), (11.0, 14.5), (4.0, 14.5)];
            i.filled(&body);
            i.poly(&body, true);
            // Buegel
            i.line(4.6, 14.5, 7.5, 10.6);
            i.line(7.5, 10.6, 10.4, 14.5);
            // Griffstrich im Koerper
            i.line(4.6, 17.6, 10.4, 17.6);
        }
        "fill" | "hatch" => {
            i.rect(4.0, 5.0, 16.0, 14.0);
            for k in 0..5 {
                i.line(5.0 + k as f32 * 3.4, 18.0, 10.0 + k as f32 * 3.4, 6.0);
            }
        }
        // Volumen: 3D-Würfel — Körper, nicht Fläche.
        "volume" => {
            // Deckfläche (gefüllt, damit der Körper plastisch wirkt)
            i.filled(&[(6.0, 7.5), (12.0, 4.0), (18.0, 7.5), (12.0, 11.0)]);
            i.poly(&[(6.0, 7.5), (12.0, 4.0), (18.0, 7.5), (12.0, 11.0)], true);
            // Seitenflächen
            i.poly(&[(6.0, 7.5), (12.0, 11.0), (12.0, 19.5), (6.0, 16.0)], true);
            i.poly(&[(18.0, 7.5), (12.0, 11.0), (12.0, 19.5), (18.0, 16.0)], true);
        }
        "text" => {
            i.line(5.0, 6.0, 19.0, 6.0);
            i.line(12.0, 6.0, 12.0, 19.0);
            i.line(8.5, 19.0, 15.5, 19.0);
        }
        "wall" => {
            i.poly(&[(3.0, 8.0), (21.0, 8.0), (21.0, 16.0), (3.0, 16.0)], true);
            for k in 0..4 {
                i.line(5.0 + k as f32 * 4.5, 16.0, 9.5 + k as f32 * 4.5, 8.0);
            }
        }
        "door" => {
            i.line(4.0, 19.0, 4.0, 6.0);
            i.arc(4.0, 19.0, 12.0, 4.71, 6.28);
            i.line(4.0, 19.0, 16.0, 19.0);
            i.dot(4.0, 19.0, 1.2);
        }
        "window" => {
            i.rect(3.0, 7.0, 18.0, 10.0);
            i.line(3.0, 12.0, 21.0, 12.0);
            i.line(12.0, 7.0, 12.0, 17.0);
        }
        "stair" => {
            i.poly(&[(4.0, 19.0), (4.0, 15.0), (8.0, 15.0), (8.0, 11.0), (12.0, 11.0), (12.0, 7.0), (16.0, 7.0), (16.0, 4.0)], false);
            i.line(2.0, 21.0, 22.0, 21.0);
        }
        "dim" => {
            i.line(3.0, 15.0, 21.0, 15.0);
            i.line(3.0, 11.0, 3.0, 19.0);
            i.line(21.0, 11.0, 21.0, 19.0);
            i.line(3.0, 15.0, 5.0, 13.0);
            i.line(21.0, 15.0, 19.0, 13.0);
            i.line(8.0, 10.0, 16.0, 10.0);
        }
        "dimchain" => {
            i.line(3.0, 14.0, 21.0, 14.0);
            for x in [3.0, 9.0, 15.0, 21.0] {
                i.line(x, 10.5, x, 18.0);
            }
            i.line(3.0, 14.0, 4.5, 12.5);
            i.line(15.0, 14.0, 16.5, 12.5);
        }
        "elevation" => {
            i.line(4.0, 17.0, 20.0, 17.0);
            i.poly(&[(12.0, 17.0), (9.5, 20.0), (14.5, 20.0)], true);
            i.line(12.0, 17.0, 12.0, 8.0);
            i.line(12.0, 8.0, 20.0, 8.0);
        }
        "style" => {
            i.circle(9.0, 9.0, 5.0);
            i.line(12.0, 12.0, 20.0, 20.0);
            i.line(17.0, 17.0, 20.0, 14.0);
        }
        "move" => {
            i.line(12.0, 3.0, 12.0, 21.0);
            i.line(3.0, 12.0, 21.0, 12.0);
            i.arrow_head(12.0, 3.0, -1.57, 3.4);
            i.arrow_head(12.0, 21.0, 1.57, 3.4);
            i.arrow_head(3.0, 12.0, 3.14, 3.4);
            i.arrow_head(21.0, 12.0, 0.0, 3.4);
        }
        "copy" => {
            i.rect(3.0, 3.0, 12.0, 12.0);
            i.rect(9.0, 9.0, 12.0, 12.0);
        }
        "copy-base" => {
            // Kopieren mit Basispunkt: dasselbe Blattpaar, unten links der
            // gesetzte Basispunkt.
            i.rect(5.0, 2.0, 11.0, 11.0);
            i.rect(10.0, 7.0, 11.0, 11.0);
            i.dot(5.0, 19.5, 2.2);
        }
        "rotate" => {
            i.arc(12.0, 12.0, 8.0, 0.6, 4.4);
            i.arrow_head(12.0 + 8.0 * 0.6f32.cos(), 12.0 + 8.0 * 0.6f32.sin(), 0.6 + 2.2, 3.6);
            i.dot(12.0, 12.0, 1.2);
        }
        "mirror" => {
            i.line(12.0, 2.0, 12.0, 22.0);
            i.poly(&[(9.0, 5.0), (3.0, 12.0), (9.0, 19.0)], true);
            i.poly(&[(15.0, 5.0), (21.0, 12.0), (15.0, 19.0)], true);
        }
        "scale" => {
            i.rect(3.0, 3.0, 11.0, 11.0);
            i.rect(3.0, 3.0, 18.0, 18.0);
            i.line(14.0, 14.0, 21.0, 21.0);
        }
        "offset" => {
            i.poly(&[(5.0, 5.0), (13.0, 5.0), (13.0, 13.0), (5.0, 13.0)], true);
            i.poly(&[(10.0, 2.0), (19.0, 2.0), (19.0, 19.0), (2.0, 19.0), (2.0, 10.0)], false);
        }
        "trim" => {
            i.poly(&[(3.0, 18.0), (21.0, 6.0)], false);
            i.line(8.0, 4.0, 8.0, 20.0);
            i.line(16.0, 4.0, 16.0, 20.0);
            i.p.extend(egui::Shape::dashed_line(
                &[i.pt(9.0, 14.0), i.pt(15.0, 10.0)],
                Stroke::new(1.0, color.gamma_multiply(0.4)),
                3.0,
                3.0,
            ));
        }
        "extend" => {
            i.line(3.0, 18.0, 13.0, 18.0);
            i.line(19.0, 4.0, 19.0, 21.0);
            i.line(13.0, 18.0, 17.0, 18.0);
            i.arrow_head(17.5, 18.0, 0.0, 3.2);
        }
        "erase" => {
            i.poly(&[(5.0, 20.0), (14.0, 4.0), (21.0, 9.0), (13.0, 20.0)], true);
            i.line(5.0, 20.0, 21.0, 20.0);
            i.line(10.0, 12.0, 18.0, 16.0);
        }
        "explode" => {
            i.poly(&[(3.0, 8.0), (10.0, 8.0), (10.0, 15.0), (3.0, 15.0)], true);
            i.poly(&[(14.0, 4.0), (21.0, 4.0), (21.0, 11.0), (14.0, 11.0)], true);
            i.poly(&[(14.0, 14.0), (21.0, 14.0), (21.0, 21.0), (14.0, 21.0)], true);
            i.line(10.0, 8.0, 14.0, 4.0);
        }
        "block" => {
            i.rect(6.0, 6.0, 12.0, 12.0);
            i.line(6.0, 6.0, 12.0, 1.5);
            i.line(18.0, 6.0, 12.0, 1.5);
            i.line(12.0, 1.5, 12.0, 6.0);
        }
        "blocknew" => {
            i.rect(3.0, 7.0, 11.0, 11.0);
            i.line(18.0, 4.0, 18.0, 12.0);
            i.line(14.0, 8.0, 22.0, 8.0);
        }
        "library" => {
            i.rect(3.0, 3.0, 8.0, 8.0);
            i.rect(13.0, 3.0, 8.0, 8.0);
            i.rect(3.0, 13.0, 8.0, 8.0);
            i.rect(13.0, 13.0, 8.0, 8.0);
        }
        "select" => {
            i.poly(&[(5.0, 3.0), (18.0, 12.0), (12.0, 13.5), (15.0, 20.0), (12.5, 21.0), (9.5, 14.5), (5.0, 18.0)], true);
        }
        "eyedropper" => {
            i.line(4.0, 20.0, 11.0, 13.0);
            i.poly(&[(9.0, 11.0), (13.0, 7.0), (17.0, 11.0), (13.0, 15.0)], true);
            i.line(14.0, 6.0, 18.0, 2.0);
            i.dot(4.0, 20.0, 1.4);
        }
        "new" => {
            i.poly(&[(6.0, 2.0), (15.0, 2.0), (19.0, 6.0), (19.0, 22.0), (6.0, 22.0)], true);
            i.line(15.0, 2.0, 15.0, 6.0);
            i.line(15.0, 6.0, 19.0, 6.0);
        }
        "open" => {
            i.poly(&[(3.0, 6.0), (10.0, 6.0), (12.0, 9.0), (21.0, 9.0), (18.0, 20.0), (3.0, 20.0)], true);
            i.line(3.0, 6.0, 3.0, 20.0);
        }
        "save" => {
            i.rect(4.0, 3.0, 16.0, 18.0);
            i.rect(8.0, 3.0, 8.0, 6.0);
            i.rect(7.0, 13.0, 10.0, 8.0);
        }
        "pdf" => {
            i.poly(&[(6.0, 2.0), (15.0, 2.0), (19.0, 6.0), (19.0, 22.0), (6.0, 22.0)], true);
            i.line(9.0, 13.0, 16.0, 13.0);
            i.line(9.0, 17.0, 14.0, 17.0);
        }
        "dxf" => {
            i.rect(3.0, 4.0, 18.0, 16.0);
            i.line(7.0, 15.0, 11.0, 9.0);
            i.line(11.0, 9.0, 15.0, 15.0);
            i.line(9.0, 12.5, 13.0, 12.5);
            i.line(16.0, 9.0, 16.0, 15.0);
            i.line(16.0, 9.0, 19.0, 9.0);
            i.line(16.0, 15.0, 19.0, 15.0);
        }
        "image" => {
            i.rect(3.0, 5.0, 18.0, 14.0);
            i.circle(8.0, 10.0, 1.8);
            i.poly(&[(5.0, 18.0), (10.0, 12.5), (14.0, 16.0), (16.5, 13.5), (19.0, 18.0)], false);
        }
        "import" => {
            i.poly(&[(12.0, 3.0), (12.0, 14.0)], false);
            i.arrow_head(12.0, 15.0, 1.57, 3.4);
            i.poly(&[(3.0, 14.0), (3.0, 21.0), (21.0, 21.0), (21.0, 14.0)], false);
        }
        "undo" => {
            i.arc(12.0, 13.0, 7.5, 3.2, 6.2);
            i.arrow_head(4.8, 11.0, -2.0, 3.6);
        }
        "redo" => {
            i.arc(12.0, 13.0, 7.5, 3.1, 6.1);
            i.arrow_head(19.2, 11.0, -1.14, 3.6);
        }
        "zoomfit" => {
            i.rect(3.0, 3.0, 18.0, 18.0);
            i.rect(8.0, 8.0, 8.0, 8.0);
        }
        "zoomin" => {
            i.circle(10.5, 10.5, 6.5);
            i.line(15.0, 15.0, 21.0, 21.0);
            i.line(7.5, 10.5, 13.5, 10.5);
            i.line(10.5, 7.5, 10.5, 13.5);
        }
        "zoomout" => {
            i.circle(10.5, 10.5, 6.5);
            i.line(15.0, 15.0, 21.0, 21.0);
            i.line(7.5, 10.5, 13.5, 10.5);
        }
        // Umsetzen der Ansicht (T0056): ein Pfeil in der Schieberichtung, die
        // Zeichnung wandert mit — deshalb zeigt der Pfeil zur Seite, in die
        // die Ansicht geschoben wird.
        "pan-left" => i.pfeil(21.0, 12.0, 4.0, 12.0),
        "pan-right" => i.pfeil(3.0, 12.0, 20.0, 12.0),
        "pan-up" => i.pfeil(12.0, 21.0, 12.0, 4.0),
        "pan-down" => i.pfeil(12.0, 3.0, 12.0, 20.0),
        "fullscreen" => {
            // Vier Eckwinkel — das Zeichen für „Fenster ohne Rahmen".
            i.line(3.0, 8.0, 3.0, 3.0);
            i.line(3.0, 3.0, 8.0, 3.0);
            i.line(16.0, 3.0, 21.0, 3.0);
            i.line(21.0, 3.0, 21.0, 8.0);
            i.line(3.0, 16.0, 3.0, 21.0);
            i.line(3.0, 21.0, 8.0, 21.0);
            i.line(16.0, 21.0, 21.0, 21.0);
            i.line(21.0, 21.0, 21.0, 16.0);
        }
        "inventory" => {
            i.rect(3.0, 3.0, 18.0, 18.0);
            i.line(3.0, 9.0, 21.0, 9.0);
            i.line(3.0, 15.0, 21.0, 15.0);
            i.line(9.0, 3.0, 9.0, 21.0);
            i.line(15.0, 3.0, 15.0, 21.0);
        }
        "layers" => {
            i.poly(&[(12.0, 3.0), (21.0, 8.0), (12.0, 13.0), (3.0, 8.0)], true);
            i.poly(&[(3.0, 12.0), (12.0, 17.0), (21.0, 12.0)], false);
            i.poly(&[(3.0, 16.0), (12.0, 21.0), (21.0, 16.0)], false);
        }
        "keyboard" => {
            i.rect(2.0, 6.0, 20.0, 12.0);
            for row in 0..2 {
                for k in 0..5 {
                    i.dot(5.5 + k as f32 * 3.2, 10.0 + row as f32 * 4.0, 0.9);
                }
            }
            i.line(7.0, 16.0, 17.0, 16.0);
        }
        "settings" => {
            // Zahnrad: Innenkreis plus acht rechteckige Zähne
            i.circle(12.0, 12.0, 6.2);
            i.circle(12.0, 12.0, 2.6);
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                let (sa, ca) = a.sin_cos();
                let (x0, y0) = (12.0 + 6.0 * ca, 12.0 + 6.0 * sa);
                let (x1, y1) = (12.0 + 9.2 * ca, 12.0 + 9.2 * sa);
                // Zahn als kurzer, breiter Strich quer zur Achse
                let (px, py) = (-sa * 1.5, ca * 1.5);
                i.line(x0 + px, y0 + py, x0 - px, y0 - py);
                i.line(x1 + px, y1 + py, x1 - px, y1 - py);
                i.line(x0 + px, y0 + py, x1 + px, y1 + py);
                i.line(x0 - px, y0 - py, x1 - px, y1 - py);
            }
        }
        "hotbar" => {
            for k in 0..4 {
                i.rect(2.5 + k as f32 * 5.2, 9.0, 4.0, 6.0);
            }
            i.line(4.5, 11.0, 4.5, 13.0);
        }
        "help" => {
            i.circle(12.0, 12.0, 8.5);
            i.arc(12.0, 10.0, 3.2, 3.4, 5.6);
            i.line(12.0, 13.4, 12.0, 15.0);
            i.dot(12.0, 17.6, 1.0);
        }
        "sheet" => {
            i.rect(3.0, 3.0, 18.0, 18.0);
            i.rect(11.0, 13.0, 7.0, 5.0);
            i.line(3.0, 8.0, 21.0, 8.0);
        }
        "viewport" => {
            i.rect(2.5, 4.0, 19.0, 16.0);
            i.p.extend(egui::Shape::dashed_line(
                &[i.pt(7.0, 8.0), i.pt(17.0, 8.0), i.pt(17.0, 16.0), i.pt(7.0, 16.0), i.pt(7.0, 8.0)],
                Stroke::new(1.0, color),
                3.0,
                2.5,
            ));
        }
        "properties" => {
            i.line(3.0, 7.0, 21.0, 7.0);
            i.line(3.0, 12.0, 21.0, 12.0);
            i.line(3.0, 17.0, 21.0, 17.0);
            i.circle(8.0, 7.0, 1.8);
            i.circle(15.0, 12.0, 1.8);
            i.circle(10.0, 17.0, 1.8);
        }
        "repeat" => {
            i.arc(12.0, 12.0, 7.5, 0.4, 5.6);
            i.arrow_head(19.0, 9.0, 0.4, 3.4);
        }
        "info" => {
            i.circle(12.0, 12.0, 8.5);
            i.line(12.0, 11.0, 12.0, 17.0);
            i.dot(12.0, 8.0, 1.1);
        }
        "close" => {
            i.line(6.0, 6.0, 18.0, 18.0);
            i.line(18.0, 6.0, 6.0, 18.0);
        }
        "plus" => {
            i.line(12.0, 5.0, 12.0, 19.0);
            i.line(5.0, 12.0, 19.0, 12.0);
        }
        "eye" => {
            i.poly(&[(2.0, 12.0), (7.0, 7.0), (17.0, 7.0), (22.0, 12.0), (17.0, 17.0), (7.0, 17.0)], true);
            i.circle(12.0, 12.0, 2.6);
        }
        "eye_off" => {
            i.poly(&[(2.0, 12.0), (7.0, 7.0), (17.0, 7.0), (22.0, 12.0), (17.0, 17.0), (7.0, 17.0)], true);
            i.line(4.0, 20.0, 20.0, 4.0);
        }
        "lock" => {
            i.rect(5.0, 11.0, 14.0, 9.0);
            i.arc(12.0, 11.0, 4.0, 3.14, 6.28);
            i.line(7.0, 11.0, 7.0, 9.5);
            i.line(17.0, 11.0, 17.0, 9.5);
        }
        "unlock" => {
            i.rect(5.0, 11.0, 14.0, 9.0);
            i.arc(16.0, 11.0, 4.0, 3.14, 6.28);
        }
        "chevron_down" => {
            i.poly(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)], false);
        }
        "chevron_right" => {
            i.poly(&[(10.0, 7.0), (15.0, 12.0), (10.0, 17.0)], false);
        }
        "chevron_left" => {
            i.poly(&[(14.0, 7.0), (9.0, 12.0), (14.0, 17.0)], false);
        }
        "chevron_up" => {
            i.poly(&[(7.0, 14.0), (12.0, 9.0), (17.0, 14.0)], false);
        }
        "grid" => {
            for k in 0..4 {
                i.line(3.0 + k as f32 * 6.0, 3.0, 3.0 + k as f32 * 6.0, 21.0);
                i.line(3.0, 3.0 + k as f32 * 6.0, 21.0, 3.0 + k as f32 * 6.0);
            }
        }
        "staerke" => {
            // Drei Linien zunehmender Stärke — Zeichen für die Linienstärke
            for (k, w) in [1.0f32, 2.0, 3.4].iter().enumerate() {
                let y = 7.0 + k as f32 * 5.0;
                i.p.line_segment(
                    [i.pt(3.0, y), i.pt(21.0, y)],
                    Stroke::new(*w, color),
                );
            }
        }
        "snap" => {
            i.circle(12.0, 12.0, 4.0);
            i.line(12.0, 2.0, 12.0, 7.0);
            i.line(12.0, 17.0, 12.0, 22.0);
            i.line(2.0, 12.0, 7.0, 12.0);
            i.line(17.0, 12.0, 22.0, 12.0);
        }
        // Zeichenreihenfolge: zwei Blätter, das hervorgehobene (gefüllte) liegt
        // vorn bzw. hinten; ein Pfeil zeigt die Richtung an.
        "order-front" => {
            // Das gefüllte Blatt ist die Auswahl: oben rechts, davor liegend;
            // der Pfeil zeigt die Richtung (ganz nach vorn).
            i.rect(2.0, 10.0, 10.0, 10.0);
            i.filled(&[(7.0, 3.0), (17.0, 3.0), (17.0, 13.0), (7.0, 13.0)]);
            i.filled(&[(19.5, 18.0), (21.3, 18.0), (21.3, 10.0), (19.5, 10.0)]);
            i.filled(&[(20.4, 4.0), (24.0, 10.3), (16.8, 10.3)]);
        }
        "order-back" => {
            i.rect(7.0, 3.0, 10.0, 10.0);
            i.filled(&[(2.0, 10.0), (12.0, 10.0), (12.0, 20.0), (2.0, 20.0)]);
            i.filled(&[(19.5, 5.0), (21.3, 5.0), (21.3, 13.0), (19.5, 13.0)]);
            i.filled(&[(20.4, 19.0), (24.0, 12.7), (16.8, 12.7)]);
        }
        // Vor/Hinter Element: das gestrichelte Band ist das Bezugselement,
        // der gefüllte Block die Auswahl darüber bzw. darunter.
        "order-above" => {
            i.filled(&[(4.0, 3.0), (20.0, 3.0), (20.0, 11.0), (4.0, 11.0)]);
            i.p.extend(egui::Shape::dashed_line(
                &[i.pt(2.0, 16.0), i.pt(22.0, 16.0)],
                Stroke::new(i.w, color),
                3.0,
                2.0,
            ));
        }
        "order-below" => {
            i.p.extend(egui::Shape::dashed_line(
                &[i.pt(2.0, 8.0), i.pt(22.0, 8.0)],
                Stroke::new(i.w, color),
                3.0,
                2.0,
            ));
            i.filled(&[(4.0, 13.0), (20.0, 13.0), (20.0, 21.0), (4.0, 21.0)]);
        }
        _ => {
            i.circle(12.0, 12.0, 7.0);
        }
    }
}

#[cfg(test)]
mod tests_pan_icons {
    //! T0056: Das Umsetzen der Ansicht sind Funktionen mit eigenem Icon; im
    //! Tastenbild stehen sie damit sichtbar auf W/A/S/D.
    //!
    //! Ein unbekannter Icon-Name fällt nicht auf: `paint` zeichnet dafür still
    //! einen Kreis (`_ =>`). Deshalb wird der Name am Quelltext geprüft.

    #[test]
    fn jede_schwenkrichtung_hat_ein_eigenes_icon() {
        let quelle = include_str!("icons.rs");
        let quelle = &quelle[..quelle.find("mod tests_pan_icons").unwrap_or(quelle.len())];
        let mut namen: Vec<&str> = Vec::new();
        for id in [
            "view.pan_left",
            "view.pan_right",
            "view.pan_up",
            "view.pan_down",
        ] {
            let def = jjcad_core::actions::action(id).expect("Action fehlt");
            assert!(
                quelle.contains(&format!("\"{}\" =>", def.icon)),
                "Icon \"{}\" für {id} fehlt in `paint` — gezeichnet würde ein Kreis",
                def.icon
            );
            assert!(!namen.contains(&def.icon), "Icon {} doppelt", def.icon);
            namen.push(def.icon);
        }
    }
}
