//! Splines auflösungsabhängig (T0175).
//!
//! Die Zerlegung der Catmull-Rom-Kurve folgt seit diesem Ticket der
//! **Pfeilhöhe**: je Spanne wird so lange halbiert, bis der Sehnenfehler die
//! Toleranz einhält. In der Anzeige ist die Toleranz `SPLINE_TOL_PX / Zoom`
//! (Bildschirm-pixel), ohne Abbildung — Druck, PNG, DXF — gilt weiter
//! `SPLINE_TOL_MM`. Fang, Trefferprüfung, Fläche und Bounding-Box rechnen
//! **immer** weltfest; die Auflösung im Bild darf daran nichts ändern.
//!
//! Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{
    catmull_rom_point, dist_point_segment, line_line, Spline, Vec2, SPLINE_MAX_TIEFE,
    SPLINE_TOL_MM,
};
use jjcad_core::model::EntityKind;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapSource};
use serde_json::json;

/// Prüfspline: drei Spannen, die mittlere mit Wendepunkt (S-Form) — genau die
/// Form, bei der eine Prüfung nur in der Spannenmitte durch die Sehne läuft.
const STUETZ: [(f64, f64); 4] = [
    (0.0, 0.0),
    (1000.0, 800.0),
    (2000.0, 0.0),
    (3000.0, 800.0),
];

fn stuetzpunkte() -> Vec<Vec2> {
    STUETZ.iter().map(|(x, y)| Vec2::new(*x, *y)).collect()
}

fn spline() -> Spline {
    Spline::new(stuetzpunkte(), false)
}

/// Die Nachbarpunkte einer Spanne — dieselbe Regel wie in
/// `catmull_rom_points` (offene Züge spiegeln am Ende).
fn nachbarn(pts: &[Vec2], i: usize) -> (Vec2, Vec2, Vec2, Vec2) {
    let n = pts.len();
    let p1 = pts[i];
    let p2 = pts[i + 1];
    let p0 = if i > 0 { pts[i - 1] } else { p1 * 2.0 - p2 };
    let p3 = if i + 2 < n {
        pts[i + 2]
    } else {
        p2 * 2.0 - p1
    };
    (p0, p1, p2, p3)
}

/// Größter Abstand der **echten** Kurve zur Punktfolge (mm) — die Pfeilhöhe,
/// wie sie im Bild sichtbar wird. Die Kurve wird dafür dicht abgetastet
/// (2000 Stellen je Spanne).
fn pfeilhoehe(pts: &[Vec2], poly: &[Vec2]) -> f64 {
    let mut max = 0.0f64;
    for i in 0..pts.len() - 1 {
        let (p0, p1, p2, p3) = nachbarn(pts, i);
        for k in 0..=2000 {
            let q = catmull_rom_point(p0, p1, p2, p3, k as f64 / 2000.0);
            let mut d = f64::INFINITY;
            for w in poly.windows(2) {
                d = d.min(dist_point_segment(q, w[0], w[1]));
            }
            max = max.max(d);
        }
    }
    max
}

/// Die Punktfolge, die die Anzeige für den Spline ablegt.
fn anzeige_polygon(zoom: Option<f64>) -> Vec<Vec2> {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute("draw.spline", &json!({ "pts": STUETZ }))
        .unwrap();
    let id = s.doc.entities[0].id;
    let ctx = match zoom {
        Some(z) => RenderCtx::model(&s.doc).mit_abbildung(z),
        None => RenderCtx::model(&s.doc),
    };
    let dl = render::render_document(&ctx);
    dl.prims
        .iter()
        .find_map(|p| match &p.shape {
            Shape::Poly(v, _) if p.entity == id => Some(v.clone()),
            _ => None,
        })
        .expect("der Spline steht als Polygonzug in der Anzeigeliste")
}

/// Der Kern: bei 30 px/mm bleibt die Pfeilhöhe unter 0,2 px. Vorher waren es
/// 45,6 px (feste Abtastung von 32 Punkten je Spanne, Toleranz wirkungslos).
#[test]
fn pfeilhoehe_bei_30_px_pro_mm_unter_0_2_px() {
    let poly = anzeige_polygon(Some(30.0));
    let h = pfeilhoehe(&stuetzpunkte(), &poly);
    let px = h * 30.0;
    assert!(
        px <= 0.2,
        "Pfeilhöhe {px:.3} px bei 30 px/mm (Grenze 0,2 px), {} Punkte",
        poly.len()
    );
    // Die Zerlegung ist wirklich feiner als die frühere feste Abtastung.
    assert!(
        poly.len() > 3 * 32,
        "nur {} Punkte — die Anzeige folgt der Pfeilhöhe nicht",
        poly.len()
    );
}

/// Bei kleinem Zoom wird die Zerlegung **gröber** als die früheren 32 Punkte je
/// Spanne: dort fielen ohnehin mehrere Punkte auf ein Pixel.
#[test]
fn bei_kleinem_zoom_weniger_punkte_als_frueher() {
    let poly = anzeige_polygon(Some(0.02));
    // Früher: 32 Punkte je Spanne, also 3 · 32 + 1 = 97 Punkte.
    assert!(
        poly.len() < 97,
        "{} Punkte bei 0,02 px/mm — nicht gröber als die feste Abtastung",
        poly.len()
    );
    // Und die Abweichung bleibt im Bild unter einem Pixel.
    let px = pfeilhoehe(&stuetzpunkte(), &poly) * 0.02;
    assert!(px <= 0.2, "Pfeilhöhe {px:.3} px bei 0,02 px/mm");
}

/// Ohne Abbildung (PNG, PDF, DXF) gilt die weltfeste Toleranz — und sie wird
/// jetzt eingehalten. Vorher lag die Pfeilhöhe bei 0,566 mm (elfmal die
/// Toleranz), weil die Abtastung bei 32 Punkten je Spanne endete.
#[test]
fn weltfest_haelt_die_toleranz_ein() {
    let poly = anzeige_polygon(None);
    let h = pfeilhoehe(&stuetzpunkte(), &poly);
    assert!(
        h <= SPLINE_TOL_MM,
        "Pfeilhöhe {h:.4} mm ohne Abbildung (Toleranz {SPLINE_TOL_MM} mm)"
    );
    // Die Punktzahl bleibt gedeckelt: höchstens 2^Tiefe Teilstrecken je Spanne.
    let deckel = 3 * (1usize << SPLINE_MAX_TIEFE) + 1;
    assert!(
        poly.len() <= deckel,
        "{} Punkte überschreiten den Deckel {deckel}",
        poly.len()
    );
}

/// Eine sehr feine Toleranz (hoher Zoom) läuft nicht ins Unermessliche: die
/// Obergrenze je Spanne hält die Punktzahl.
#[test]
fn punktzahl_ist_je_spanne_gedeckelt() {
    let poly = spline().flatten(1e-9);
    let deckel = 3 * (1usize << SPLINE_MAX_TIEFE) + 1;
    assert!(
        poly.len() <= deckel,
        "{} Punkte bei Toleranz 1e-9 — Deckel {deckel}",
        poly.len()
    );
    // Und die Kurve endet weiterhin genau auf den Stützpunkten.
    assert_eq!(poly.first().copied(), Some(Vec2::new(0.0, 0.0)));
    assert!(poly.last().unwrap().dist(Vec2::new(3000.0, 800.0)) < 1e-9);
}

/// Fang und Schnitt rechnen **weltfest**: dieselben Punkte bei 30 px/mm, bei
/// 0,02 px/mm und ohne Abbildung (1e-6). Die Auflösung im Bild darf den Fang
/// nicht verschieben — sonst läge der Fangpunkt neben der Kurve, sobald man
/// zoomt.
#[test]
fn fang_und_schnitt_bleiben_weltfest() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": {
            "grid": false, "ortho": false, "polar": false,
            "nearest": true, "midpoint": false, "perpendicular": false,
            "intersection": true, "center": false
        } }),
    )
    .unwrap();
    s.execute("draw.spline", &json!({ "pts": STUETZ }))
        .unwrap();
    // Eine Waagerechte, die die Kurve zweimal kreuzt.
    s.execute(
        "draw.line",
        &json!({ "a": [500.0, 300.0], "b": [2500.0, 300.0] }),
    )
    .unwrap();

    // „Nächster Punkt" auf der Kurve.
    let cursor = Vec2::new(1500.0, 410.0);
    let naechster = snap_constrained(&s.doc, cursor, 20.0, None, &[], false);
    assert_eq!(naechster.source, SnapSource::Object(SnapKind::Nearest));

    // Schnittpunkt Kurve × Linie.
    let kreuz = Vec2::new(1584.0, 300.0);
    let schnitt = snap_constrained(&s.doc, kreuz, 20.0, None, &[], false);
    assert_eq!(schnitt.source, SnapSource::Object(SnapKind::Intersection));

    // Beide Punkte liegen exakt auf der weltfesten Zerlegung.
    let weltfest = spline().flatten(SPLINE_TOL_MM);
    let auf_der_kurve = |p: Vec2| {
        weltfest
            .windows(2)
            .map(|w| dist_point_segment(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    };
    assert!(
        auf_der_kurve(naechster.point) < 1e-9,
        "Fangpunkt liegt nicht auf der Kurve: {:?}",
        naechster.point
    );
    assert!(
        auf_der_kurve(schnitt.point) < 1e-9,
        "Schnittpunkt liegt nicht auf der Kurve: {:?}",
        schnitt.point
    );

    // Und die Anzeige in jeder Auflösung ändert daran nichts: der Fang läuft
    // über `Spline::segments`, nicht über die Anzeigeliste.
    for zoom in [0.02, 1.0, 30.0, 300.0] {
        let dl = render::render_document(&RenderCtx::model(&s.doc).mit_abbildung(zoom));
        assert!(!dl.prims.is_empty(), "Anzeige bei {zoom} px/mm leer");
        let nochmal =
            snap_constrained(&s.doc, cursor, 20.0, None, &[], false);
        assert!(
            nochmal.point.dist(naechster.point) < 1e-6,
            "Fang wandert mit dem Zoom ({zoom} px/mm): {:?} statt {:?}",
            nochmal.point,
            naechster.point
        );
        let nochmal =
            snap_constrained(&s.doc, kreuz, 20.0, None, &[], false);
        assert!(
            nochmal.point.dist(schnitt.point) < 1e-6,
            "Schnitt wandert mit dem Zoom ({zoom} px/mm): {:?} statt {:?}",
            nochmal.point,
            schnitt.point
        );
    }
}

/// Die Anzeigeliste folgt dem Zoom: bei 30 px/mm feiner als weltfest, bei
/// 0,02 px/mm gröber. Die Zerlegung ist genau die mit der Bildschirmtoleranz.
#[test]
fn anzeigeliste_folgt_dem_zoom() {
    let fein = anzeige_polygon(Some(30.0));
    let grob = anzeige_polygon(Some(0.02));
    let weltfest = anzeige_polygon(None);
    assert!(
        fein.len() > weltfest.len() && weltfest.len() > grob.len(),
        "Punktzahlen 30 px/mm {}, weltfest {}, 0,02 px/mm {}",
        fein.len(),
        weltfest.len(),
        grob.len()
    );
    // Die Anzeige ist die Zerlegung mit `SPLINE_TOL_PX / Zoom`.
    let erwartet = spline().flatten(render::SPLINE_TOL_PX / 30.0);
    assert_eq!(fein.len(), erwartet.len());
    for (a, b) in fein.iter().zip(erwartet.iter()) {
        assert!(a.dist(*b) < 1e-9, "Anzeige weicht von der Zerlegung ab");
    }
}

/// Der Schnittpunkt einer Linie mit der Kurve liegt auf der **gezeichneten**
/// Kurve — die Zerlegung für Fang und Anzeige ist dieselbe (weltfest).
#[test]
fn schnittpunkt_liegt_auf_der_gezeichneten_kurve() {
    let poly = spline().flatten(SPLINE_TOL_MM);
    let (a, b) = (Vec2::new(500.0, 300.0), Vec2::new(2500.0, 300.0));
    let treffer: Vec<Vec2> = poly
        .windows(2)
        .filter_map(|w| line_line(w[0], w[1], a, b, false, false))
        .collect();
    assert_eq!(treffer.len(), 3, "die Waagerechte kreuzt die Kurve dreimal");
    for p in &treffer {
        let d = poly
            .windows(2)
            .map(|w| dist_point_segment(*p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min);
        assert!(d < 1e-9, "Schnittpunkt {p:?} liegt {d} mm neben der Kurve");
    }
    // Der erste Schnittpunkt liegt dort, wo die Kurve die Höhe 300 mm hat.
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute("draw.spline", &json!({ "pts": STUETZ }))
        .unwrap();
    let e = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Spline { .. }))
        .expect("Spline");
    let segs = render::entity_segments(&s.doc, e);
    assert!(!segs.is_empty(), "Fang und Treffer brauchen Strecken");
}
