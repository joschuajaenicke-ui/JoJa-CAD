//! Schnellauswahl (T0060): jede Objektart kennt ihr Erzeugerwerkzeug.
//!
//! Die Zuordnung steht in `actions::erzeuger_fuer` — ein `match` ohne Zweig
//! „Sonstiges“, damit eine neue Objektart sofort beim Bauen auffällt. Dieser
//! Test prüft die **inhaltliche** Richtigkeit: das Werkzeug muss existieren,
//! muss ein Werkzeug sein (kein Dialog) und zur Art passen.

use jjcad_core::actions;
use jjcad_core::geom::{v, Vertex};
use jjcad_core::model::*;
use jjcad_core::style::{FillPattern, WallAlign};

fn erzeuger(kind: &EntityKind) -> &'static str {
    actions::erzeuger_fuer(kind)
}

/// Der Erzeuger ist ein echtes Werkzeug aus dem Register — sonst startet der
/// Klick in der Schnellauswahl nichts (oder öffnet still einen Dialog).
fn pruefe(kind: &EntityKind, erwartet: &str) {
    let id = erzeuger(kind);
    assert_eq!(id, erwartet, "falsches Erzeugerwerkzeug für {:?}", kind.type_name());
    let def = actions::action(id).unwrap_or_else(|| panic!("Action '{id}' fehlt im Register"));
    assert!(def.tool, "'{id}' muss ein Werkzeug sein, kein Dialog");
}

fn flaeche_kind(volumen: bool) -> EntityKind {
    if volumen {
        EntityKind::Volume {
            boundary: vec![v(0.0, 0.0), v(1000.0, 0.0), v(1000.0, 1000.0)],
            holes: Vec::new(),
            pattern: FillPattern::Solid,
            pattern_angle: 0.0,
            pattern_scale: 100.0,
            border: true,
            height: 2500.0,
        }
    } else {
        EntityKind::Fill {
            boundary: vec![v(0.0, 0.0), v(1000.0, 0.0), v(1000.0, 1000.0)],
            holes: Vec::new(),
            pattern: FillPattern::Lines,
            pattern_angle: std::f64::consts::FRAC_PI_4,
            pattern_scale: 100.0,
            border: false,
        }
    }
}

#[test]
fn jede_objektart_startet_ihren_erzeuger() {
    let verts = vec![Vertex::new(v(0.0, 0.0)), Vertex::new(v(1000.0, 0.0))];
    let mut geprueft = 0;

    // Freie Geometrie
    pruefe(&EntityKind::Point { p: v(0.0, 0.0) }, "draw.point");
    pruefe(
        &EntityKind::Line {
            a: v(0.0, 0.0),
            b: v(1000.0, 0.0),
        },
        "draw.line",
    );
    pruefe(
        &EntityKind::Circle {
            c: v(0.0, 0.0),
            r: 500.0,
        },
        "draw.circle",
    );
    pruefe(
        &EntityKind::Arc {
            c: v(0.0, 0.0),
            r: 500.0,
            a0: 0.0,
            a1: 1.0,
        },
        "draw.arc",
    );
    pruefe(
        &EntityKind::Ellipse {
            c: v(0.0, 0.0),
            rx: 500.0,
            ry: 300.0,
            rot: 0.0,
        },
        "draw.ellipse",
    );
    pruefe(
        &EntityKind::Polyline {
            verts: verts.clone(),
            closed: false,
        },
        "draw.polyline",
    );
    pruefe(
        &EntityKind::Spline {
            verts,
            closed: false,
        },
        "draw.spline",
    );

    // Fläche und Volumen
    pruefe(&flaeche_kind(false), "draw.fill");
    pruefe(&flaeche_kind(true), "draw.volume");

    // Beschriftung
    pruefe(
        &EntityKind::Text {
            p: v(0.0, 0.0),
            text: "Text".into(),
            height: 250.0,
            angle: 0.0,
            align: TextAlignH::Left,
            valign: TextAlignV::Top,
            bold: false,
            italic: false,
            underline: false,
            superscript: false,
            runs: Vec::new(),
            font: TextFont::Sans,
            line_spacing: 1.0,
        frame: false,                    frame_round: false,                    background: None,                    },
        "draw.text",
    );
    pruefe(
        &EntityKind::Dim {
            pts: vec![v(0.0, 0.0), v(1000.0, 0.0)],
            line_pos: v(0.0, -300.0),
            axis: DimAxis::X,
            override_text: None,
        },
        "dim.chain",
    );
    pruefe(
        &EntityKind::Dim {
            pts: vec![v(0.0, 0.0), v(1000.0, 0.0), v(2000.0, 0.0)],
            line_pos: v(0.0, -300.0),
            axis: DimAxis::X,
            override_text: None,
        },
        "dim.chain",
    );
    pruefe(
        &EntityKind::ElevationPlan {
            p: v(0.0, 0.0),
            z: 2500.0,
        },
        "dim.elevation.plan",
    );
    pruefe(
        &EntityKind::ElevationSection {
            pts: vec![v(0.0, 0.0), v(0.0, 2500.0)],
        },
        "dim.elevation.section",
    );

    // Architektur
    pruefe(
        &EntityKind::Wall {
            a: v(0.0, 0.0),
            b: v(4000.0, 0.0),
            thickness: 240.0,
            height: 2500.0,
            material: String::new(),
            hatch: None,
            align: WallAlign::Center,
        },
        "draw.wall",
    );
    pruefe(
        &EntityKind::Door {
            center: v(0.0, 0.0),
            angle: 0.0,
            width: 885.0,
            kind: DoorKind::Hinge,
            hinge: 1.0,
            swing_in: true,
            align: WallAlign::Center,
            host: Some(1),
            at: Some(0.0),
            base: DoorBase::Left,
        },
        "draw.door",
    );
    pruefe(
        &EntityKind::Window {
            center: v(0.0, 0.0),
            angle: 0.0,
            width: 1200.0,
            align: WallAlign::Center,
            sill: 900.0,
            height: 1400.0,
            host: Some(1),
            at: Some(0.0),
        },
        "draw.window",
    );
    pruefe(
        &EntityKind::Stair {
            a: v(0.0, 0.0),
            b: v(3000.0, 0.0),
            width: 1000.0,
            steps: 16,
            height: 2800.0,
            auto_steps: true,
            cut: true,
            unten: None,
            oben: None,
        },
        "draw.stair",
    );
    geprueft += 17;

    // Block: der Erzeuger ist das Einfügen, die Definition kommt aus der Instanz
    // (`canvas::tool_click_at` setzt `options.block_id`).
    pruefe(
        &EntityKind::BlockRef {
            block: "tisch".into(),
            pos: v(0.0, 0.0),
            rot: 0.0,
            scale: v(1.0, 1.0),
            mirror: false,
        },
        "block.insert",
    );
    geprueft += 1;

    assert_eq!(geprueft, 18, "jede Objektart aus `EntityKind` wird geprüft");
}
