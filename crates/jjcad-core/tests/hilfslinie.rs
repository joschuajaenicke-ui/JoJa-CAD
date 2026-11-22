//! Hilfslinie beim Eingeben eines Punktes (T0230) — der Fang auf ihrer Strecke.
//!
//! Eine Hilfslinie verhält sich wie eine normale Linie: sie endet an zwei
//! Endpunkten, und der Zeiger rastet auf ihrem Fußpunkt ein. Anders als die
//! unbegrenzten Hilfslinien des erweiterten Punktfangs darf der Fußpunkt
//! deshalb nicht über die Enden hinauslaufen.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::snap::hilfslinien_fuss;

/// Senkrechter Abstand des Zeigers zur Strecke, gemessen am Fußpunkt.
fn fuss(a: Vec2, b: Vec2, cursor: Vec2, tol: f64) -> Option<Vec2> {
    hilfslinien_fuss(a, b, cursor, tol)
}

#[test]
fn fusspunkt_liegt_senkrecht_unter_dem_zeiger() {
    let a = v(0.0, 0.0);
    let b = v(2000.0, 0.0);
    // Zeiger 40 mm über der Streckenmitte: der Fußpunkt bleibt auf der
    // Strecke, 40 mm müssen im Fangradius (tol) liegen.
    let cursor = v(1000.0, 40.0);
    assert_eq!(fuss(a, b, cursor, 50.0), Some(v(1000.0, 0.0)));
    assert_eq!(fuss(a, b, cursor, 30.0), None, "außerhalb des Fangradius");
}

#[test]
fn fusspunkt_wird_auf_die_strecke_begrenzt() {
    let a = v(0.0, 0.0);
    let b = v(2000.0, 0.0);
    // Vor dem Anfang und hinter dem Ende zählt der jeweilige Endpunkt — eine
    // Strecke ist keine unbegrenzte Gerade.
    assert_eq!(fuss(a, b, v(-20.0, 5.0), 30.0), Some(a));
    assert_eq!(fuss(a, b, v(2020.0, 5.0), 30.0), Some(b));
    // 40 mm vor dem Anfang liegt der Endpunkt außerhalb des Fangradius.
    assert_eq!(fuss(a, b, v(-40.0, 0.0), 30.0), None);
}

#[test]
fn schraege_strecke_faengt_ihren_fusspunkt() {
    let a = v(0.0, 0.0);
    let b = v(2000.0, 2000.0);
    // Zeiger 20 mm seitlich der Diagonale, auf halber Strecke.
    let cursor = v(1000.0 - 20.0 * 0.70710678, 1000.0 + 20.0 * 0.70710678);
    let f = fuss(a, b, cursor, 30.0).expect("Fußpunkt der Diagonale");
    assert!(
        (f.x - 1000.0).abs() < 1e-6 && (f.y - 1000.0).abs() < 1e-6,
        "Fußpunkt auf der Strecke: {f:?}"
    );
    assert!((f.dist(cursor) - 20.0).abs() < 0.01, "Abstand: {}", f.dist(cursor));
}

#[test]
fn entartete_strecke_faengt_nichts() {
    // Ein Klick ohne Bewegung legt keine Richtung fest — die Strecke hat die
    // Länge null und darf nicht auf sich selbst ziehen.
    assert_eq!(fuss(v(100.0, 100.0), v(100.0, 100.0), v(100.0, 100.0), 50.0), None);
}
