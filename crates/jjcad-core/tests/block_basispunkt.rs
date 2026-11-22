//! Basispunkt im Blockeditor (T0208).
//!
//! Im Blockeditor zeigt die Zeichenfläche am Weltursprung den Basispunkt des
//! Blocks als Kreuz. Er ist kein Objekt — ohne eigenen Fangpunkt war er mit dem
//! Zeiger nicht zu treffen. Diese Prüfung hält fest: er fängt im Blockeditor
//! (wie ein Endpunkt, mit Vorrang vor dem Raster), und nur dort.

use jjcad_core::geom::v;
use jjcad_core::model::{BlockDef, BlockEinheit, BlockSource, Document};
use jjcad_core::snap::{self, SnapKind};

/// Zeichnung mit einem bearbeitbaren Block im Blockeditor.
fn doc_im_blockeditor() -> Document {
    let mut doc = Document::new();
    doc.blocks.push(BlockDef {
        id: "prj:basispunkt".into(),
        name: "Basispunkt-Probe".into(),
        category: String::new(),
        source: BlockSource::Project,
        description: String::new(),
        entities: Vec::new(),
        icon: None,
        base: v(0.0, 0.0),
        einheit: BlockEinheit::Millimeter,
    });
    assert!(doc.set_block_space(Some("prj:basispunkt")), "Blockeditor öffnet nicht");
    doc
}

#[test]
fn basispunkt_faengt_im_blockeditor() {
    let doc = doc_im_blockeditor();
    let kandidaten = snap::snap_candidates(&doc, v(1.0, 1.0), 6.0, None, &[]);
    let bp = kandidaten
        .iter()
        .find(|k| k.kind == SnapKind::BlockBase)
        .expect("Basispunkt wird nicht gefangen");
    assert!(bp.point.dist(v(0.0, 0.0)) < 1e-9, "Basispunkt liegt woanders: {:?}", bp.point);
    assert_eq!(
        kandidaten[0].kind,
        SnapKind::BlockBase,
        "Basispunkt muss Vorrang vor dem Raster haben"
    );
    assert_eq!(SnapKind::BlockBase.label(), "Basispunkt");
    assert_eq!(SnapKind::BlockBase.short(), "BP");
    assert!(SnapKind::BlockBase.ist_diskret(), "Basispunkt ist ein Haltepunkt");
}

#[test]
fn basispunkt_faengt_nur_im_blockeditor() {
    // Modell und Papierraum haben keinen Block-Basispunkt.
    let doc = Document::new();
    let kandidaten = snap::snap_candidates(&doc, v(1.0, 1.0), 6.0, None, &[]);
    assert!(
        kandidaten.iter().all(|k| k.kind != SnapKind::BlockBase),
        "Basispunkt darf außerhalb des Blockeditors nicht fangen"
    );
    assert!(snap::block_basispunkt(&doc).is_none());
    assert_eq!(snap::block_basispunkt(&doc_im_blockeditor()), Some(v(0.0, 0.0)));
}

#[test]
fn basispunkt_haengt_am_endpunkt_schalter() {
    let mut doc = doc_im_blockeditor();
    doc.settings.snap.endpoint = false;
    let kandidaten = snap::snap_candidates(&doc, v(1.0, 1.0), 6.0, None, &[]);
    assert!(
        kandidaten.iter().all(|k| k.kind != SnapKind::BlockBase),
        "ohne Endpunktfang darf auch der Basispunkt nicht fangen"
    );
}
