//! Systemblock-Bibliothek (mitgelieferte Symbole).
//!
//! Alle Maße in Millimetern, Ursprung im Objektmittelpunkt. Die Elemente sind
//! auf "ByBlock" gestellt, übernehmen also Farbe und Linienstärke der Instanz.

use crate::geom::*;
use crate::model::*;
use crate::style::{Color, FillPattern};

struct B {
    next: Id,
    ents: Vec<Entity>,
}

impl B {
    fn new() -> Self {
        Self {
            next: 1,
            ents: Vec::new(),
        }
    }

    fn id(&mut self) -> Id {
        let i = self.next;
        self.next += 1;
        i
    }

    fn push(&mut self, kind: EntityKind) -> &mut Self {
        let id = self.id();
        let mut e = Entity::new(id, 0, kind);
        e.by_block = true;
        self.ents.push(e);
        self
    }

    fn line(&mut self, a: Vec2, b: Vec2) {
        self.push(EntityKind::Line { a, b });
    }

    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let pts = vec![v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)];
        self.push(EntityKind::Polyline {
            verts: pts.into_iter().map(Vertex::new).collect(),
            closed: true,
        });
    }

    fn rect_arcs(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, r: f64) {
        let bulge = (std::f64::consts::FRAC_PI_2 / 4.0).tan();
        let verts = vec![
            Vertex::arc(v(x0 + r, y0), 0.0),
            Vertex::arc(v(x1 - r, y0), bulge),
            Vertex::arc(v(x1, y0 + r), 0.0),
            Vertex::arc(v(x1, y1 - r), bulge),
            Vertex::arc(v(x1 - r, y1), 0.0),
            Vertex::arc(v(x0 + r, y1), bulge),
            Vertex::arc(v(x0, y1 - r), 0.0),
            Vertex::arc(v(x0, y0 + r), bulge),
        ];
        self.push(EntityKind::Polyline {
            verts,
            closed: true,
        });
    }

    fn circle(&mut self, c: Vec2, r: f64) {
        self.push(EntityKind::Circle { c, r });
    }

    fn arc(&mut self, c: Vec2, r: f64, a0: f64, a1: f64) {
        self.push(EntityKind::Arc { c, r, a0, a1 });
    }

    fn text(&mut self, p: Vec2, txt: &str, h: f64) {
        self.push(EntityKind::Text {
            p,
            text: txt.to_string(),
            height: h,
            angle: 0.0,
            align: TextAlignH::Center,
            valign: TextAlignV::Middle,
            bold: false,
            italic: false,
            underline: false,
            superscript: false,
            runs: Vec::new(),
            font: TextFont::Sans,
            line_spacing: 1.0,
        });
    }

    fn fill(&mut self, pts: Vec<Vec2>, color: Color) {
        let id = self.id();
        let mut e = Entity::new(id, 0, EntityKind::Fill {
            boundary: pts,
            holes: Vec::new(),
            pattern: FillPattern::Solid,
            pattern_angle: 0.0,
            pattern_scale: 100.0,
            border: false,
        });
        e.by_block = true;
        e.style.fill = Some(color);
        self.ents.push(e);
    }
}

fn block(
    id: &str,
    name: &str,
    cat: &str,
    desc: &str,
    b: B,
) -> BlockDef {
    BlockDef {
        id: id.to_string(),
        name: name.to_string(),
        category: cat.to_string(),
        source: BlockSource::System,
        description: desc.to_string(),
        entities: b.ents,
        icon: None,
        base: Vec2::ZERO,
    }
}

/// Alle Systemblöcke.
pub fn system_blocks(_next_id: &mut Id) -> Vec<BlockDef> {
    let mut out = Vec::new();

    // ---------------- Sanitär ----------------
    let mut b = B::new();
    b.rect(-190.0, -300.0, 190.0, -120.0); // Spülkasten
    b.rect_arcs(-185.0, -300.0, 185.0, -120.0, 20.0);
    b.push(EntityKind::Ellipse {
        c: v(0.0, 45.0),
        rx: 175.0,
        ry: 215.0,
        rot: 0.0,
    });
    b.push(EntityKind::Ellipse {
        c: v(0.0, 45.0),
        rx: 130.0,
        ry: 170.0,
        rot: 0.0,
    });
    b.circle(v(0.0, -60.0), 18.0);
    out.push(block(
        "sys:wc",
        "WC",
        "Sanitär",
        "Wand-WC, 380 × 600 mm",
        b,
    ));

    let mut b = B::new();
    b.rect_arcs(-275.0, -210.0, 275.0, 210.0, 35.0);
    b.push(EntityKind::Ellipse {
        c: v(0.0, 10.0),
        rx: 200.0,
        ry: 140.0,
        rot: 0.0,
    });
    b.circle(v(0.0, 165.0), 22.0);
    b.circle(v(0.0, 165.0), 8.0);
    out.push(block(
        "sys:waschbecken",
        "Waschbecken",
        "Sanitär",
        "Handwaschbecken 550 × 420 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-450.0, -450.0, 450.0, 450.0);
    b.line(v(-450.0, -450.0), v(450.0, 450.0));
    b.line(v(-450.0, 450.0), v(450.0, -450.0));
    b.circle(v(0.0, 0.0), 55.0);
    b.circle(v(0.0, 0.0), 15.0);
    out.push(block(
        "sys:dusche",
        "Dusche",
        "Sanitär",
        "Duschfläche 900 × 900 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-850.0, -375.0, 850.0, 375.0);
    b.rect_arcs(-790.0, -315.0, 790.0, 315.0, 160.0);
    b.circle(v(650.0, 0.0), 35.0);
    b.circle(v(-760.0, 0.0), 25.0);
    out.push(block(
        "sys:badewanne",
        "Badewanne",
        "Sanitär",
        "Badewanne 1700 × 750 mm",
        b,
    ));

    // ---------------- Küche ----------------
    let mut b = B::new();
    b.rect(-300.0, -300.0, 300.0, 300.0);
    for (x, y, r) in [
        (-150.0, 150.0, 85.0),
        (150.0, 150.0, 70.0),
        (-150.0, -150.0, 70.0),
        (150.0, -150.0, 85.0),
    ] {
        b.circle(v(x, y), r);
    }
    out.push(block(
        "sys:herd",
        "Herd",
        "Küche",
        "Kochfeld 600 × 600 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-300.0, -250.0, 300.0, 250.0);
    b.rect_arcs(-255.0, -205.0, -15.0, 205.0, 25.0);
    b.rect_arcs(15.0, -205.0, 255.0, 205.0, 25.0);
    b.circle(v(0.0, -215.0), 20.0);
    out.push(block(
        "sys:spuele",
        "Spüle",
        "Küche",
        "Doppelspüle 600 × 500 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-300.0, -300.0, 300.0, 300.0);
    b.line(v(-300.0, 240.0), v(300.0, 240.0));
    b.text(v(0.0, 0.0), "KS", 90.0);
    out.push(block(
        "sys:kuehlschrank",
        "Kühlschrank",
        "Küche",
        "Kühlschrank 600 × 600 mm",
        b,
    ));

    // ---------------- Möbel ----------------
    let mut b = B::new();
    b.rect(-800.0, -1000.0, 800.0, 1000.0);
    b.line(v(-800.0, 780.0), v(800.0, 780.0));
    b.rect(-720.0, 760.0, -80.0, 930.0);
    b.rect(80.0, 760.0, 720.0, 930.0);
    b.line(v(-800.0, 500.0), v(800.0, 500.0));
    out.push(block(
        "sys:bett",
        "Bett 1600",
        "Möbel",
        "Doppelbett 1600 × 2000 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-450.0, -1000.0, 450.0, 1000.0);
    b.line(v(-450.0, 800.0), v(450.0, 800.0));
    b.rect(-380.0, 780.0, 380.0, 940.0);
    out.push(block(
        "sys:bett90",
        "Bett 900",
        "Möbel",
        "Einzelbett 900 × 2000 mm",
        b,
    ));

    let mut b = B::new();
    b.rect_arcs(-1100.0, -450.0, 1100.0, 450.0, 60.0);
    b.line(v(-1100.0, 280.0), v(1100.0, 280.0));
    b.line(v(-360.0, -450.0), v(-360.0, 280.0));
    b.line(v(360.0, -450.0), v(360.0, 280.0));
    out.push(block(
        "sys:sofa",
        "Sofa",
        "Möbel",
        "Sofa 2200 × 900 mm, 3-sitzig",
        b,
    ));

    let mut b = B::new();
    b.rect(-700.0, -400.0, 700.0, 400.0);
    out.push(block(
        "sys:tisch",
        "Tisch",
        "Möbel",
        "Tisch 1400 × 800 mm",
        b,
    ));

    let mut b = B::new();
    b.rect_arcs(-225.0, -225.0, 225.0, 225.0, 30.0);
    b.line(v(-225.0, 150.0), v(225.0, 150.0));
    out.push(block(
        "sys:stuhl",
        "Stuhl",
        "Möbel",
        "Stuhl 450 × 450 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-500.0, -300.0, 500.0, 300.0);
    b.line(v(0.0, -300.0), v(0.0, 300.0));
    b.line(v(-500.0, -300.0), v(0.0, 300.0));
    b.line(v(0.0, -300.0), v(500.0, 300.0));
    out.push(block(
        "sys:schrank",
        "Schrank",
        "Möbel",
        "Schrank 1000 × 600 mm",
        b,
    ));

    // ---------------- Technik ----------------
    let mut b = B::new();
    b.rect(-300.0, -300.0, 300.0, 300.0);
    b.circle(v(0.0, 0.0), 180.0);
    b.circle(v(0.0, 0.0), 140.0);
    b.text(v(0.0, 0.0), "WM", 90.0);
    out.push(block(
        "sys:waschmaschine",
        "Waschmaschine",
        "Technik",
        "Waschmaschine 600 × 600 mm",
        b,
    ));

    let mut b = B::new();
    b.rect(-400.0, -50.0, 400.0, 50.0);
    for i in 0..15 {
        let x = -400.0 + i as f64 * 53.0 + 26.0;
        b.line(v(x, -50.0), v(x, 50.0));
    }
    out.push(block(
        "sys:heizkoerper",
        "Heizkörper",
        "Technik",
        "Heizkörper 800 × 100 mm",
        b,
    ));

    // ---------------- Symbole ----------------
    let mut b = B::new();
    b.circle(Vec2::ZERO, 300.0);
    b.circle(Vec2::ZERO, 240.0);
    b.push(EntityKind::Polyline {
        verts: vec![
            Vertex::new(v(0.0, 270.0)),
            Vertex::new(v(-80.0, -120.0)),
            Vertex::new(v(0.0, -40.0)),
            Vertex::new(v(80.0, -120.0)),
        ],
        closed: true,
    });
    b.text(v(0.0, 420.0), "N", 200.0);
    out.push(block(
        "sys:nordpfeil",
        "Nordpfeil",
        "Symbole",
        "Nordpfeil mit Kreis",
        b,
    ));

    let mut b = B::new();
    b.line(v(-350.0, 0.0), v(350.0, 0.0));
    b.circle(v(-350.0, 0.0), 110.0);
    b.circle(v(350.0, 0.0), 110.0);
    b.text(v(-350.0, 0.0), "A", 120.0);
    out.push(block(
        "sys:schnitt",
        "Schnittmarke",
        "Symbole",
        "Schnittmarke A–A",
        b,
    ));

    let mut b = B::new();
    b.rect(-300.0, -150.0, 300.0, 150.0);
    b.text(v(0.0, 0.0), "RAUM", 100.0);
    out.push(block(
        "sys:raumstempel",
        "Raumstempel",
        "Symbole",
        "Raumbezeichnung (Text anpassen)",
        b,
    ));

    let mut b = B::new();
    b.rect(-250.0, -250.0, 250.0, 250.0);
    b.line(v(-250.0, -250.0), v(250.0, 250.0));
    b.line(v(-250.0, 250.0), v(250.0, -250.0));
    b.text(v(0.0, 0.0), "X", 90.0);
    out.push(block(
        "sys:leer",
        "Platzhalter",
        "Symbole",
        "Platzhalter 500 × 500 mm",
        b,
    ));

    out
}

/// Kategorien in Anzeigereihenfolge.
pub fn categories() -> Vec<&'static str> {
    vec!["Sanitär", "Küche", "Möbel", "Technik", "Symbole"]
}
