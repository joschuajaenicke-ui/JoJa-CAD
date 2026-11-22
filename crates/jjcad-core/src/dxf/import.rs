//! DXF-Import (ASCII und binär erkannte Dateien werden abgelehnt).
//!
//! Unterstützt: LINE, LWPOLYLINE, POLYLINE/VERTEX, CIRCLE, ARC, POINT, TEXT,
//! MTEXT (bereinigt), INSERT, SOLID, ELLIPSE, SPLINE (als Näherung),
//! Layer-Tabelle und Einheiten aus dem Header.

use crate::geom::*;
use crate::model::*;
use crate::style::{Color, LineType};
use std::path::Path;

#[derive(Default)]
pub struct ImportResult {
    pub entities: Vec<Entity>,
    pub layers: Vec<Layer>,
    pub blocks: Vec<BlockDef>,
    pub warnings: Vec<String>,
}

type Pairs = Vec<(i32, String)>;

fn parse_pairs(s: &str) -> Result<Pairs, String> {
    if s.trim_start().starts_with("AutoCAD Binary DXF") {
        return Err("Binäre DXF-Dateien werden nicht unterstützt (bitte als ASCII-DXF speichern)".into());
    }
    let mut out = Vec::new();
    let mut lines = s.lines();
    while let Some(code_line) = lines.next() {
        let code_str = code_line.trim();
        if code_str.is_empty() {
            continue;
        }
        let code: i32 = code_str
            .parse()
            .map_err(|_| format!("Ungültiger Group-Code: '{code_str}'"))?;
        let Some(val) = lines.next() else { break };
        out.push((code, val.trim_end_matches(['\r', '\n']).to_string()));
    }
    Ok(out)
}

fn find1<'a>(g: &'a [(i32, String)], code: i32) -> Option<&'a str> {
    g.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
}

fn find_f(g: &[(i32, String)], code: i32) -> Option<f64> {
    find1(g, code).and_then(|v| v.trim().replace(',', ".").parse().ok())
}

fn find_i(g: &[(i32, String)], code: i32) -> Option<i64> {
    find1(g, code).and_then(|v| v.trim().parse().ok())
}

fn aci_to_color(i: i64) -> Option<Color> {
    Some(match i.abs() {
        1 => Color::rgb(255, 0, 0),
        2 => Color::rgb(255, 255, 0),
        3 => Color::rgb(0, 255, 0),
        4 => Color::rgb(0, 255, 255),
        5 => Color::rgb(0, 0, 255),
        6 => Color::rgb(255, 0, 255),
        7 => Color::rgb(20, 20, 20),
        8 => Color::rgb(128, 128, 128),
        9 => Color::rgb(192, 192, 192),
        250..=255 => Color::rgb(90, 90, 90),
        _ => return None,
    })
}

fn ltype_from_name(n: &str) -> LineType {
    match n.to_uppercase().as_str() {
        "DASHED" | "DASHED2" | "DASHEDX2" => LineType::Dashed,
        "DOT" | "DOT2" | "DOTX2" => LineType::Dotted,
        "DASHDOT" | "DASHDOT2" => LineType::DashDot,
        "HIDDEN" | "HIDDEN2" => LineType::Hidden,
        _ => LineType::Solid,
    }
}

/// Bereinigt MTEXT-Steuercodes grob.
fn clean_mtext(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('P') => out.push('\n'),
                Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some('{') => out.push('{'),
                Some('}') => out.push('}'),
                Some('f') | Some('F') => {
                    // \fSchriftart|b0|i0|c0|p34;
                    while let Some(&n) = chars.peek() {
                        if n == ';' {
                            chars.next();
                            break;
                        }
                        chars.next();
                    }
                }
                Some('H') | Some('h') => {
                    while let Some(&n) = chars.peek() {
                        if n == ';' {
                            chars.next();
                            break;
                        }
                        chars.next();
                    }
                }
                Some(other) => out.push(other),
                None => {}
            },
            '{' | '}' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Parst eine DXF-Datei und liefert importierbare Objekte.
pub fn import_dxf(path: &Path) -> Result<ImportResult, String> {
    let raw = std::fs::read(path).map_err(|e| format!("DXF lesen fehlgeschlagen: {e}"))?;
    let text = String::from_utf8_lossy(&raw).to_string();
    let pairs = parse_pairs(&text)?;
    let mut res = ImportResult::default();

    // Einheiten
    let unit_factor = {
        let mut f = 1.0;
        for w in pairs.windows(2) {
            if w[0].0 == 9 && w[0].1 == "$INSUNITS" && w[1].0 == 70 {
                f = match w[1].1.trim() {
                    "1" => 25.4,
                    "2" => 304.8,
                    "4" => 1.0,
                    "5" => 10.0,
                    "6" => 1000.0,
                    _ => 1.0,
                };
            }
        }
        f
    };

    // Layer-Tabelle
    let mut layer_map: std::collections::HashMap<String, Id> = std::collections::HashMap::new();
    let mut next_layer_id = 1u64;
    if let Some(start) = find_section(&pairs, "TABLES") {
        let mut i = start;
        while i < pairs.len() {
            if pairs[i].0 == 0 && pairs[i].1 == "ENDTAB" {
                break;
            }
            if pairs[i].0 == 0 && pairs[i].1 == "LAYER" {
                let mut g = Vec::new();
                i += 1;
                while i < pairs.len() && pairs[i].0 != 0 {
                    g.push(pairs[i].clone());
                    i += 1;
                }
                if let Some(name) = find1(&g, 2) {
                    if name != "0" || !layer_map.contains_key("0") {
                        let mut l = Layer::new(
                            next_layer_id,
                            name,
                            aci_to_color(find_i(&g, 62).unwrap_or(7)).unwrap_or(Color::BLACK),
                        );
                        if let Some(lt) = find1(&g, 6) {
                            l.line_type = ltype_from_name(lt);
                        }
                        if let Some(v) = find_i(&g, 62) {
                            l.visible = v >= 0;
                        }
                        if let Some(lw) = find_i(&g, 370) {
                            l.line_weight = (lw.unsigned_abs() as f64) / 100.0;
                        }
                        layer_map.insert(name.to_string(), l.id);
                        res.layers.push(l);
                        next_layer_id += 1;
                    }
                }
                continue;
            }
            i += 1;
        }
    }
    if res.layers.is_empty() {
        res.layers.push(Layer::new(1, "0", Color::BLACK));
        next_layer_id = 2;
    }

    // Blöcke
    let mut block_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(start) = find_section(&pairs, "BLOCKS") {
        let mut i = start;
        while i < pairs.len() {
            if pairs[i].0 == 0 && pairs[i].1 == "ENDSEC" {
                break;
            }
            if pairs[i].0 == 0 && pairs[i].1 == "BLOCK" {
                i += 1;
                let mut g = Vec::new();
                while i < pairs.len() && pairs[i].0 != 0 {
                    g.push(pairs[i].clone());
                    i += 1;
                }
                let name = find1(&g, 2).unwrap_or("BLOCK").to_string();
                let base = v(find_f(&g, 10).unwrap_or(0.0), find_f(&g, 20).unwrap_or(0.0));
                // Blockinhalt bis ENDBLK
                let mut ents_raw: Vec<Vec<(i32, String)>> = Vec::new();
                while i < pairs.len() {
                    if pairs[i].0 == 0 && pairs[i].1 == "ENDBLK" {
                        i += 1;
                        break;
                    }
                    if pairs[i].0 == 0 {
                        let mut eg = vec![pairs[i].clone()];
                        i += 1;
                        while i < pairs.len() && pairs[i].0 != 0 {
                            eg.push(pairs[i].clone());
                            i += 1;
                        }
                        ents_raw.push(eg);
                    } else {
                        i += 1;
                    }
                }
                let bid = format!("dxf:{}", name.to_lowercase().replace(' ', "_"));
                let mut bid_next = 1u64;
                let ents = convert_entities(
                    &ents_raw,
                    unit_factor,
                    &mut bid_next,
                    &layer_map,
                    next_layer_id,
                    &mut res.warnings,
                );
                let mut bd = BlockDef {
                    id: bid.clone(),
                    name: name.clone(),
                    category: "Import".into(),
                    source: BlockSource::Project,
                    description: format!("DXF-Block {name}"),
                    entities: ents,
                    icon: None,
                    base,
                    groesse: BlockGroesse::Fest,
                };
                if let Some(desc) = find1(&g, 4) {
                    bd.description = desc.to_string();
                }
                block_map.insert(name.to_uppercase(), bid.clone());
                res.blocks.push(bd);
                continue;
            }
            i += 1;
        }
    }

    // Entitäten
    if let Some(start) = find_section(&pairs, "ENTITIES") {
        let mut raws: Vec<Vec<(i32, String)>> = Vec::new();
        let mut i = start;
        while i < pairs.len() {
            if pairs[i].0 == 0 && pairs[i].1 == "ENDSEC" {
                break;
            }
            if pairs[i].0 == 0 {
                let mut g = vec![pairs[i].clone()];
                i += 1;
                while i < pairs.len() && pairs[i].0 != 0 {
                    g.push(pairs[i].clone());
                    i += 1;
                }
                raws.push(g);
            } else {
                i += 1;
            }
        }
        let mut next_id = 1u64;
        let mut ents = convert_entities(
            &raws,
            unit_factor,
            &mut next_id,
            &layer_map,
            next_layer_id,
            &mut res.warnings,
        );
        // INSERTs mit unbekannten Blöcken auflösen
        for e in &mut ents {
            if let EntityKind::BlockRef { block, .. } = &mut e.kind {
                if let Some(mapped) = block_map.get(&block.to_uppercase()) {
                    *block = mapped.clone();
                }
            }
        }
        res.entities = ents;
    }

    Ok(res)
}

fn find_section(pairs: &Pairs, name: &str) -> Option<usize> {
    let mut i = 0;
    while i + 1 < pairs.len() {
        if pairs[i].0 == 0
            && pairs[i].1 == "SECTION"
            && pairs[i + 1].0 == 2
            && pairs[i + 1].1.eq_ignore_ascii_case(name)
        {
            return Some(i + 2);
        }
        i += 1;
    }
    None
}

fn convert_entities(
    raws: &[Vec<(i32, String)>],
    unit_factor: f64,
    next_id: &mut Id,
    layer_map: &std::collections::HashMap<String, Id>,
    _next_layer_id: Id,
    warnings: &mut Vec<String>,
) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut poly: Option<(Vec<Vertex>, bool, Id, Option<Color>, Option<LineType>)> = None;

    let mut push = |kind: EntityKind, layer: Id, color: Option<Color>, lt: Option<LineType>| {
        let id = *next_id;
        *next_id += 1;
        let mut e = Entity::new(id, layer, kind);
        e.style.color = color;
        e.style.line_type = lt;
        out.push(e);
    };

    for g in raws {
        let typ = g[0].1.to_uppercase();
        let layer_name = find1(g, 8).unwrap_or("0");
        let layer = *layer_map.get(layer_name).unwrap_or(&1);
        let color_code = find_i(g, 62).unwrap_or(256);
        let color = if color_code != 256 && color_code != 0 {
            aci_to_color(color_code)
        } else {
            None
        };
        let lt = find1(g, 6).filter(|s| !s.eq_ignore_ascii_case("BYLAYER")).map(|s| ltype_from_name(s));

        match typ.as_str() {
            "LINE" => {
                let a = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let b = v(
                    find_f(g, 11).unwrap_or(0.0) * unit_factor,
                    find_f(g, 21).unwrap_or(0.0) * unit_factor,
                );
                push(EntityKind::Line { a, b }, layer, color, lt);
            }
            "CIRCLE" => {
                let c = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let r = find_f(g, 40).unwrap_or(1.0) * unit_factor;
                push(EntityKind::Circle { c, r }, layer, color, lt);
            }
            "ARC" => {
                let c = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let r = find_f(g, 40).unwrap_or(1.0) * unit_factor;
                let a0 = deg(find_f(g, 50).unwrap_or(0.0));
                let a1 = deg(find_f(g, 51).unwrap_or(360.0));
                push(EntityKind::Arc { c, r, a0, a1 }, layer, color, lt);
            }
            "POINT" => {
                let p = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                push(EntityKind::Point { p }, layer, color, lt);
            }
            "LWPOLYLINE" => {
                let closed = find_i(g, 70).unwrap_or(0) & 1 == 1;
                let mut verts: Vec<Vertex> = Vec::new();
                let mut cur: Option<Vec2> = None;
                let mut bulge = 0.0;
                for (code, val) in g.iter() {
                    match code {
                        10 => {
                            if let Some(p) = cur {
                                verts.push(Vertex::arc(p, bulge));
                            }
                            cur = Some(v(val.parse().unwrap_or(0.0) * unit_factor, 0.0));
                            bulge = 0.0;
                        }
                        20 => {
                            if let Some(p) = &mut cur {
                                p.y = val.parse().unwrap_or(0.0) * unit_factor;
                            }
                        }
                        42 => bulge = val.parse().unwrap_or(0.0),
                        _ => {}
                    }
                }
                if let Some(p) = cur {
                    verts.push(Vertex::arc(p, bulge));
                }
                if verts.len() >= 2 {
                    push(EntityKind::Polyline { verts, closed }, layer, color, lt);
                }
            }
            "POLYLINE" => {
                let closed = find_i(g, 70).unwrap_or(0) & 1 == 1;
                poly = Some((Vec::new(), closed, layer, color, lt));
            }
            "VERTEX" => {
                if let Some((verts, _, _, _, _)) = &mut poly {
                    let p = v(
                        find_f(g, 10).unwrap_or(0.0) * unit_factor,
                        find_f(g, 20).unwrap_or(0.0) * unit_factor,
                    );
                    let bulge = find_f(g, 42).unwrap_or(0.0);
                    verts.push(Vertex::arc(p, bulge));
                }
            }
            "SEQEND" => {
                if let Some((verts, closed, layer, color, lt)) = poly.take() {
                    if verts.len() >= 2 {
                        push(EntityKind::Polyline { verts, closed }, layer, color, lt);
                    }
                }
            }
            "TEXT" => {
                let mut p = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let txt = find1(g, 1).unwrap_or("").to_string();
                let h = find_f(g, 40).unwrap_or(2.5) * unit_factor;
                let angle = deg(find_f(g, 50).unwrap_or(0.0));
                let h_align_raw = find_i(g, 72).unwrap_or(0);
                let align = match h_align_raw {
                    1 | 4 => TextAlignH::Center,
                    2 => TextAlignH::Right,
                    _ => TextAlignH::Left,
                };
                if h_align_raw != 0 {
                    if let (Some(x), Some(y)) = (find_f(g, 11), find_f(g, 21)) {
                        p = v(x * unit_factor, y * unit_factor);
                    }
                }
                if !txt.trim().is_empty() {
                    push(
                        EntityKind::Text {
                            p,
                            text: txt,
                            height: h,
                            angle,
                            align,
                            valign: TextAlignV::Middle,
                            bold: false,
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
                        layer,
                        color,
                        lt,
                    );
                }
            }
            "MTEXT" => {
                let p = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let mut txt = String::new();
                for (code, val) in g.iter() {
                    if *code == 3 || *code == 1 {
                        txt.push_str(val);
                    }
                }
                let txt = clean_mtext(&txt);
                let h = find_f(g, 40).unwrap_or(2.5) * unit_factor;
                let angle = deg(find_f(g, 50).unwrap_or(0.0));
                if !txt.trim().is_empty() {
                    push(
                        EntityKind::Text {
                            p,
                            text: txt,
                            height: h,
                            angle,
                            align: TextAlignH::Left,
                            valign: TextAlignV::Middle,
                            bold: false,
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
                        layer,
                        color,
                        lt,
                    );
                }
            }
            "INSERT" => {
                let name = find1(g, 2).unwrap_or("").to_string();
                let pos = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let sx = find_f(g, 41).unwrap_or(1.0);
                let sy = find_f(g, 42).unwrap_or(1.0);
                let rot = deg(find_f(g, 50).unwrap_or(0.0));
                let mirror = sx < 0.0;
                push(
                    EntityKind::BlockRef {
                        block: format!("dxf:{}", name.to_lowercase().replace(' ', "_")),
                        pos,
                        rot,
                        scale: v(sx.abs(), sy),
                        mirror,
                    },
                    layer,
                    color,
                    lt,
                );
            }
            "ELLIPSE" => {
                let c = v(
                    find_f(g, 10).unwrap_or(0.0) * unit_factor,
                    find_f(g, 20).unwrap_or(0.0) * unit_factor,
                );
                let major = v(find_f(g, 11).unwrap_or(1.0), find_f(g, 21).unwrap_or(0.0)) * unit_factor;
                let ratio = find_f(g, 40).unwrap_or(1.0);
                let rx = major.len();
                push(
                    EntityKind::Ellipse {
                        c,
                        rx,
                        ry: rx * ratio,
                        rot: major.angle(),
                    },
                    layer,
                    color,
                    lt,
                );
            }
            "SOLID" => {
                let mut pts = Vec::new();
                for (c1, c2) in [(10, 20), (11, 21), (12, 22), (13, 23)] {
                    if let Some(x) = find_f(g, c1) {
                        let y = find_f(g, c2).unwrap_or(0.0);
                        pts.push(v(x * unit_factor, y * unit_factor));
                    }
                }
                if pts.len() >= 3 {
                    push(
                        EntityKind::Fill {
                            boundary: pts,
                            holes: Vec::new(),
                            pattern: crate::style::FillPattern::Solid,
                            pattern_angle: 0.0,
                            pattern_scale: 100.0,
                            border: true,
                        },
                        layer,
                        color,
                        lt,
                    );
                }
            }
            "SPLINE" => {
                let mut pts = Vec::new();
                for (code, val) in g.iter() {
                    if *code == 11 || *code == 10 {
                        if let Ok(x) = val.parse::<f64>() {
                            pts.push(v(x * unit_factor, 0.0));
                        }
                    } else if (*code == 21 || *code == 20) && !pts.is_empty() {
                        if let Ok(y) = val.parse::<f64>() {
                            if let Some(last) = pts.last_mut() {
                                last.y = y * unit_factor;
                            }
                        }
                    }
                }
                if pts.len() >= 2 {
                    push(
                        EntityKind::Polyline {
                            verts: pts.into_iter().map(Vertex::new).collect(),
                            closed: false,
                        },
                        layer,
                        color,
                        lt,
                    );
                }
            }
            "DIMENSION" | "HATCH" | "LEADER" | "MLEADER" => {
                let msg = format!("{typ}-Objekte werden beim Import übersprungen");
                if !warnings.contains(&msg) {
                    warnings.push(msg);
                }
            }
            "ENDSEC" | "EOF" | "SECTION" | "TABLE" | "ENDTAB" | "BLOCK" | "ENDBLK" => {}
            other => {
                let msg = format!("Unbekannter DXF-Objekttyp '{other}' übersprungen");
                if !warnings.contains(&msg) {
                    warnings.push(msg);
                }
            }
        }
    }
    out
}

/// Importierte Objekte in ein bestehendes Dokument übernehmen.
pub fn merge_into(doc: &mut Document, res: ImportResult) -> ImportResult {
    let mut result = ImportResult {
        entities: Vec::new(),
        layers: Vec::new(),
        blocks: Vec::new(),
        warnings: res.warnings,
    };
    // Layer
    let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
    for l in res.layers {
        if let Some(existing) = doc.layer_by_name(&l.name) {
            map.insert(l.id, existing.id);
        } else {
            let id = doc.new_id();
            let mut nl = l.clone();
            nl.id = id;
            map.insert(l.id, id);
            result.layers.push(nl.clone());
            doc.layers.push(nl);
        }
    }
    // Blöcke
    let mut bmap: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for mut b in res.blocks {
        let mut id = b.id.clone();
        if doc.blocks.iter().any(|x| x.id == id) {
            id = format!("{}:{}", id, crate::util::short_id());
        }
        b.id = id.clone();
        bmap.insert(id.clone(), id.clone());
        for e in &mut b.entities {
            if let Some(nl) = map.get(&e.layer) {
                e.layer = *nl;
            }
            let child_block = match &e.kind {
                EntityKind::BlockRef { block, .. } => block.as_str(),
                _ => "",
            };
            if let Some(im) = bmap.get(child_block) {
                if let EntityKind::BlockRef { block, .. } = &mut e.kind {
                    *block = im.clone();
                }
            }
        }
        result.blocks.push(b.clone());
        doc.blocks.push(b);
    }
    // Entitäten
    for mut e in res.entities {
        if let Some(nl) = map.get(&e.layer) {
            e.layer = *nl;
        }
        e.id = doc.new_id();
        if let EntityKind::BlockRef { block, .. } = &mut e.kind {
            if let Some(nb) = bmap.get(block) {
                *block = nb.clone();
            }
        }
        result.entities.push(e.clone());
        doc.entities.push(e);
    }
    doc.modified = true;
    result
}
