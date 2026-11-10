//! Abnahmetests für die Fang-Kette bei der Polylinien-Eingabe (Kanban t_809d529d).
//!
//! Geprüft wird über die öffentliche Fang-Engine `snap::snap_constrained` — den
//! Weg, den die Zeichenfläche für Zeigeranzeige und Klickpunkt nimmt. Der Kern
//! der Karte: der Objektfang muss **vor** Ortho/Polar laufen, sonst liegt ein
//! vorhandenes Linienende neben der Zwangsachse und ist unerreichbar.
//!
//! Alle Koordinaten in Millimetern (`input_unit: mm`), damit Fangpunkt und
//! gespeicherter Punkt ohne Einheitenumrechnung verglichen werden können.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapResolution, SnapSource};
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    // Raster aus: sonst überlagert der Rasterfang die Messungen.
    s.execute("doc.settings", &json!({ "snap": { "grid": false } }))
        .unwrap();
    s
}

/// Sitzung mit Ortho-Zwang (Polar aus): die Zwangsrichtung, unter der der
/// Fehler auftrat.
fn mit_ortho() -> Session {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "ortho": true, "polar": false } }),
    )
    .unwrap();
    s
}

/// Sitzung mit Polarfang (Ortho aus) — der Standard des Programms.
fn mit_polar() -> Session {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "ortho": false, "polar": true } }),
    )
    .unwrap();
    s
}

/// Bestehende Linie 0,0 → 2000,0 (mm).
fn linie(s: &mut Session) {
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [2000.0, 0.0] }))
        .unwrap();
}

/// Fang ohne Raster und ohne Ausschluss — wie die Zeichenfläche bei
/// ausgeblendetem Raster zeichnet.
fn fang(s: &Session, cursor: Vec2, tol: f64, reference: Option<Vec2>) -> SnapResolution {
    snap_constrained(&s.doc, cursor, tol, reference, &[], &[], false)
}

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(
        ist.dist(soll) < 1e-9,
        "{was}: erwartet {soll:?}, ist {ist:?}"
    );
}

// ---------------------------------------------------------------------------
// Reihenfolge: Objektfang vor Ortho/Polar
// ---------------------------------------------------------------------------

/// Der Kernfehler: mit Ortho (Standard) wurde der Zeiger zuerst auf die Achse
/// projiziert und der Fang danach dort gesucht. Der Bezugspunkt liegt hier
/// 1400 mm neben der Achse des Linienendes — der projizierte Punkt (1990;1400)
/// ist 1400 mm vom Endpunkt entfernt, also außerhalb jedes Fangradius.
#[test]
fn objektfang_gewinnt_gegen_ortho() {
    let mut s = mit_ortho();
    linie(&mut s);
    // Bezugspunkt wie der zuletzt gesetzte Polylinien-Punkt, weit weg von der
    // X-Achse: Ortho würde (1980; 1400) liefern.
    let bezug = v(0.0, 1400.0);
    let r = fang(&s, v(1980.0, 30.0), 40.0, Some(bezug));

    assert_eq!(
        r.source,
        SnapSource::Object(SnapKind::Endpoint),
        "Linienende muss trotz Ortho fangen, war {:?}",
        r.source
    );
    assert_punkt(r.point, v(2000.0, 0.0), "Endpunkt");
    assert_eq!(r.label(), Some("Endpunkt"));
}

/// Gegenprobe: außerhalb des Fangradius bleibt die Ortho-Führung wirksam —
/// der Fang darf nicht jedes entfernte Element heranziehen.
#[test]
fn ohne_treffer_bleibt_ortho() {
    let mut s = mit_ortho();
    linie(&mut s);
    let bezug = v(0.0, 1400.0);
    let r = fang(&s, v(1500.0, 60.0), 40.0, Some(bezug));

    assert_eq!(r.source, SnapSource::Ortho, "Ortho bleibt Führung");
    assert_punkt(r.point, v(1500.0, 1400.0), "Ortho-Punkt");
    assert_eq!(r.label(), None, "Ortho ist keine Fangart");
}

/// Polarfang genauso: er darf den Objektfang nicht verdrängen.
#[test]
fn objektfang_gewinnt_gegen_polar() {
    let mut s = mit_polar();
    linie(&mut s);
    let r = fang(&s, v(1990.0, 25.0), 40.0, Some(v(0.0, 0.0)));

    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint));
    assert_punkt(r.point, v(2000.0, 0.0), "Endpunkt");
}

// ---------------------------------------------------------------------------
// Prioritäten und Fangradius
// ---------------------------------------------------------------------------

/// Endpunkt hat Vorrang vor der Kante, auch wenn die Kante näher liegt.
#[test]
fn endpunkt_vor_kante() {
    let mut s = s();
    linie(&mut s);
    // 6 mm vom Endpunkt, 4 mm von der Kante (Lotfuß 1988;0).
    let r = fang(&s, v(1996.0, 6.0), 30.0, None);
    assert_eq!(
        r.source,
        SnapSource::Object(SnapKind::Endpoint),
        "Endpunkt vor Nächster Punkt, war {:?}",
        r.source
    );
    assert_punkt(r.point, v(2000.0, 0.0), "Endpunkt");
}

/// Weit von jedem Endpunkt entfernt fängt die Kante — und zwar exakt auf ihr.
#[test]
fn kante_faengt_ohne_endpunkt_in_der_naehe() {
    let mut s = s();
    linie(&mut s);
    let r = fang(&s, v(700.0, 10.0), 30.0, None);
    assert_eq!(
        r.source,
        SnapSource::Object(SnapKind::Nearest),
        "Kante muss fangen, war {:?}",
        r.source
    );
    assert_punkt(r.point, v(700.0, 0.0), "Punkt auf der Kante");
}

/// Der Fangradius in Weltkoordinaten ist Pixel ÷ Zoom: derselbe Zeigerabstand
/// in Pixeln fängt bei jeder Zoomstufe, ein größerer nie.
#[test]
fn verhalten_unabhaengig_von_der_zoomstufe() {
    let mut s = s();
    linie(&mut s);
    let px = s.doc.settings.snap.tolerance_px;
    for zoom in [0.02, 0.1, 0.5, 2.0] {
        let tol = (px / zoom).max(0.5);
        // 0,4 × Radius diagonal daneben (Abstand ≈ 0,57 × Radius) → fängt.
        let nah = v(2000.0 - 0.4 * tol, 0.4 * tol);
        let r = fang(&s, nah, tol, None);
        assert_eq!(
            r.source,
            SnapSource::Object(SnapKind::Endpoint),
            "Zoom {zoom}: {nah:?} muss fangen"
        );
        assert_punkt(r.point, v(2000.0, 0.0), &format!("Zoom {zoom}"));

        // 1,5 × Radius daneben → kein Endpunktfang.
        let fern = v(2000.0 - 1.5 * tol, 1.5 * tol);
        let r = fang(&s, fern, tol, None);
        assert_ne!(
            r.source,
            SnapSource::Object(SnapKind::Endpoint),
            "Zoom {zoom}: {fern:?} darf nicht fangen"
        );
    }
}

// ---------------------------------------------------------------------------
// Übernommener Punkt == Fangpunkt (keine Rundung)
// ---------------------------------------------------------------------------

/// Der Polylinien-Punkt, den die Zeichenfläche übernimmt, wird unverändert
/// gespeichert: zwei Klicks 8 mm neben dem Linienende ergeben exakt dessen
/// Koordinaten — kein auf das Raster gerundeter Zwischenwert.
#[test]
fn gespeicherter_punkt_ist_der_fangpunkt() {
    let mut s = mit_ortho();
    linie(&mut s);
    let start = v(0.0, 2000.0);
    // 8 mm neben dem Linienende; Ortho würde daraus (1992; 2000) machen,
    // also 2000 mm neben dem Endpunkt.
    let r = fang(&s, v(1992.0, 8.0), 40.0, Some(start));
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint));

    s.execute(
        "draw.polyline",
        &json!({ "pts": [[start.x, start.y], [r.point.x, r.point.y]] }),
    )
    .unwrap();

    let verts = match &s.doc.entities.last().unwrap().kind {
        EntityKind::Polyline { verts, .. } => verts.clone(),
        other => panic!("keine Polylinie angelegt: {other:?}"),
    };
    assert_eq!(verts.len(), 2, "zwei Stützpunkte");
    assert_punkt(verts[1].p, r.point, "gespeicherter Punkt");
    assert_punkt(verts[1].p, v(2000.0, 0.0), "gespeicherter Punkt am Ende");
}

// ---------------------------------------------------------------------------
// Polarfang bleibt als Führung erhalten
// ---------------------------------------------------------------------------

/// Ohne Objekt in Reichweite rastet der Polarfang auf 45° — Länge bleibt,
/// Label `POL`, Punkt exakt auf dem Strahl.
#[test]
fn polarfuehrung_bleibt_erhalten() {
    let s = mit_polar();
    let bezug = v(0.0, 0.0);
    // 1° neben 45°, 600 mm entfernt: seitlich 10,5 mm — im Band von 12 mm (T0148).
    let cursor = Vec2::polar(44.0_f64.to_radians(), 600.0);
    let r = fang(&s, cursor, 12.0, Some(bezug));

    assert_eq!(r.source, SnapSource::Polar);
    assert_eq!(r.label(), Some("POL"));
    assert!(
        (r.point.dist(bezug) - 600.0).abs() < 1e-6,
        "Länge bleibt erhalten: {:?}",
        r.point
    );
    let winkel = (r.point - bezug).angle().to_degrees();
    assert!(
        (winkel - 45.0).abs() < 1e-9,
        "auf 45° eingerastet, ist {winkel}"
    );
}
