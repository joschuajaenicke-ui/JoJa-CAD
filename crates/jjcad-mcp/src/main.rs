//! JoJa-CAD MCP-Server — headless Steuerung des CAD-Kerns über stdio.
//!
//! Protokoll: JSON-RPC 2.0 / Model Context Protocol (stdio, zeilenweise).
//! Alle Werkzeuge arbeiten auf derselben Befehls-Schicht wie die GUI.

use jjcad_core::actions;
use jjcad_core::export::RasterOptions;
use jjcad_core::session::Session;
use jjcad_core::units::Unit;
use jjcad_core::{dxf, pdf, raster};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const PROTOCOL_VERSION: &str = "2024-11-05";

// ---------------------------------------------------------------------------
// Serverzustand
// ---------------------------------------------------------------------------

struct DocEntry {
    name: String,
    session: Session,
}

struct Server {
    docs: Vec<DocEntry>,
    active: usize,
}

impl Server {
    fn new() -> Self {
        Self {
            docs: vec![DocEntry {
                name: "default".into(),
                session: Session::new(),
            }],
            active: 0,
        }
    }

    fn active_mut(&mut self) -> &mut Session {
        &mut self.docs[self.active].session
    }

    fn active(&self) -> &Session {
        &self.docs[self.active].session
    }

    fn select(&mut self, name: Option<&str>) -> Result<(), String> {
        let Some(name) = name else { return Ok(()) };
        if name.is_empty() {
            return Ok(());
        }
        if let Some(i) = self
            .docs
            .iter()
            .position(|d| d.name == name || d.session.doc.name == name)
        {
            self.active = i;
            Ok(())
        } else {
            Err(format!("Dokument '{name}' nicht gefunden"))
        }
    }

    fn find_or_open(&mut self, path: &Path) -> Result<usize, String> {
        let ps = path.to_string_lossy().to_string();
        if let Some(i) = self
            .docs
            .iter()
            .position(|d| d.session.doc.path.as_deref() == Some(ps.as_str()))
        {
            return Ok(i);
        }
        let s = Session::open(path)?;
        let name = path
            .file_stem()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("doc{}", self.docs.len() + 1));
        self.docs.push(DocEntry { name, session: s });
        Ok(self.docs.len() - 1)
    }
}

// ---------------------------------------------------------------------------
// Basis64 (für PNG-Rückgaben)
// ---------------------------------------------------------------------------

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        out.push(T[(b[0] >> 2) as usize] as char);
        out.push(T[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(b[2] & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn ok_text(s: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": s.into()}], "isError": false})
}

fn err_text(s: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": s.into()}], "isError": true})
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

// ---------------------------------------------------------------------------
// Werkzeugliste
// ---------------------------------------------------------------------------

fn tool_list() -> Value {
    json!([
        {
            "name": "jjcad_document",
            "description": "Dokumentverwaltung: new, open, save, save_as, info, list, close, units. Ohne 'doc' wird das aktive Dokument verwendet. Projektmetadaten über 'project' (project, client, planner, plan_no, plan_title, address, revision) – sie füllen das Schriftfeld.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["new","open","save","save_as","info","list","close","units"]},
                    "path": {"type": "string", "description": "Dateipfad (.jjcad)"},
                    "name": {"type": "string", "description": "Dokumentname für spätere Aufrufe"},
                    "doc": {"type": "string", "description": "Dokumentname (optional)"},
                    "units": {"type": "string", "enum": ["mm","cm","m"]},
                    "project": {"type": "object", "description": "Projektdaten"}
                },
                "required": ["action"]
            }
        },
        {
            "name": "jjcad_execute",
            "description": "Führt eine CAD-Aktion aus (IDs siehe jjcad_actions). Koordinaten in der Dokument-Einheit (Standard Meter), alternativ 'units' setzen. Beispiele: draw.wall {a:[0,0], b:[5,0], thickness:0.24}; draw.door {center:[1.5,0], width:0.885}; dim.linear {a:[0,0],b:[5,0],line_pos:[2.5,-1]}; block.insert {block:'WC', pos:[1,2], rot:90}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "Action-ID, z. B. draw.wall"},
                    "params": {"type": "object", "description": "Parameter der Aktion"},
                    "doc": {"type": "string"},
                    "units": {"type": "string", "enum": ["mm","cm","m"]}
                },
                "required": ["action"]
            }
        },
        {
            "name": "jjcad_batch",
            "description": "Führt mehrere Aktionen nacheinander aus (eine Undo-Einheit pro Schritt). ACHTUNG: Der Batch bricht beim ersten fehlerhaften Schritt ab (Vorgabe stop_on_error=true) — alle folgenden Schritte werden NICHT ausgeführt. Ein Tippfehler in Schritt 12 verschluckt die Schritte 13–80, deshalb Batches in kleinen Gruppen (etwa 10–20 Schritte) schicken und nach jedem Batch das Ergebnis prüfen. Antwort: executed = erfolgreich ausgeführte Schritte, errors = Fehlschläge, aborted_at = Index des Fehlschlags (null ohne Abbruch), nicht_ausgefuehrt = Anzahl der nicht mehr ausgeführten Schritte.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "steps": {"type": "array", "items": {"type": "object"}},
                    "doc": {"type": "string"},
                    "units": {"type": "string", "enum": ["mm","cm","m"]},
                    "stop_on_error": {"type": "boolean", "default": true}
                },
                "required": ["steps"]
            }
        },
        {
            "name": "jjcad_query",
            "description": "Fragt Objekte ab: alle, nach IDs, Layer, Typ oder Bereich. Liefert Typ, Layer, Farbe, typspezifische Parameter und bei geschlossenen Objekten die Fläche area_m2 (m², deutsches Komma).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ids": {"type": "array", "items": {"type": "integer"}},
                    "layer": {"type": "string"},
                    "type": {"type": "string", "description": "z. B. Wand, Tür, Linie"},
                    "bbox": {"type": "array", "items": {"type": "number"}, "description": "[x0,y0,x1,y1] in mm"},
                    "limit": {"type": "integer", "default": 200},
                    "doc": {"type": "string"}
                }
            }
        },
        {
            "name": "jjcad_export",
            "description": "Exportiert PDF (Blatt mit Schriftfeld), DXF oder PNG (headless gerendert).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "format": {"type": "string", "enum": ["pdf","dxf","png"]},
                    "path": {"type": "string"},
                    "sheet": {"type": "integer", "description": "Blattindex (0-basiert)"},
                    "width": {"type": "integer", "default": 2000},
                    "height": {"type": "integer", "default": 1400},
                    "doc": {"type": "string"}
                },
                "required": ["format", "path"]
            }
        },
        {
            "name": "jjcad_render",
            "description": "Rendert Zeichnung oder Blatt als PNG und gibt das Bild zurück (visuelles Feedback).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sheet": {"type": "integer"},
                    "width": {"type": "integer", "default": 1400},
                    "height": {"type": "integer", "default": 1000},
                    "doc": {"type": "string"}
                }
            }
        },
        {
            "name": "jjcad_undo",
            "description": "Macht die letzte Änderung rückgängig.",
            "inputSchema": {"type": "object", "properties": {"doc": {"type": "string"}}}
        },
        {
            "name": "jjcad_redo",
            "description": "Wiederholt die zuletzt rückgängig gemachte Änderung.",
            "inputSchema": {"type": "object", "properties": {"doc": {"type": "string"}}}
        },
        {
            "name": "jjcad_actions",
            "description": "Listet alle verfügbaren Aktionen mit ID, Gruppe und Kurzhinweis.",
            "inputSchema": {
                "type": "object",
                "properties": {"group": {"type": "string"}}
            }
        },
        {
            "name": "jjcad_sheet",
            "description": "Blattverwaltung: list, create, update, remove_viewport, add_viewport, update_viewport, fit_viewport. Ein Ansichtsfenster mischt zwei Räume: x/y/w/h bzw. rect_mm sind Papier-Millimeter, center ist eine Modellkoordinate in der Einheit units (Vorgabe: Dokumenteinheit), center_mm dieselbe Lage in Millimetern. fit_viewport passt das Fenster auf die Zeichnung ein.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list","create","update","remove_viewport","add_viewport","update_viewport","fit_viewport"]},
                    "sheet": {"type": "integer", "description": "Blattindex (0 = erstes Blatt)"},
                    "name": {"type": "string", "description": "Blattname bei create"},
                    "paper": {"type": "string", "enum": ["A4","A3","A2","A1","A0"]},
                    "scale": {"type": "number", "description": "Maßstab 1:scale — Blatt bei update, Ansichtsfenster bei den Ansichts-Aktionen"},
                    "units": {"type": "string", "enum": ["mm","cm","m"], "description": "Einheit für center; Vorgabe: Dokumenteinheit"},
                    "margin_mm": {"type": "number", "description": "Rand in Papier-Millimetern für fit_viewport (Vorgabe 10)"},
                    "viewport": {
                        "type": "object",
                        "description": "Ansichtsfenster: id, x/y/w/h oder rect_mm (Papier-mm), center (in units) oder center_mm, scale, name, show_frame",
                        "properties": {
                            "id": {"type": "integer"},
                            "name": {"type": "string"},
                            "x": {"type": "number"}, "y": {"type": "number"},
                            "w": {"type": "number"}, "h": {"type": "number"},
                            "rect_mm": {"type": "array", "items": {"type": "number"}},
                            "center": {"type": "array", "items": {"type": "number"}},
                            "center_mm": {"type": "array", "items": {"type": "number"}},
                            "scale": {"type": "number"},
                            "show_frame": {"type": "boolean"}
                        }
                    },
                    "doc": {"type": "string"}
                },
                "required": ["action"]
            }
        }
    ])
}

fn action_catalog(group: Option<&str>) -> Value {
    let list: Vec<Value> = actions::ACTIONS
        .iter()
        .filter(|a| {
            group
                .map(|g| a.group.label().eq_ignore_ascii_case(g))
                .unwrap_or(true)
        })
        .map(|a| {
            json!({
                "id": a.id,
                "label": a.label(),
                "group": a.group.label(),
                "hint": a.hint(),
                "tool": a.tool,
                "key": a.key,
            })
        })
        .collect();
    json!({"count": list.len(), "actions": list})
}

// ---------------------------------------------------------------------------
// Werkzeugausführung
// ---------------------------------------------------------------------------

fn call_tool(server: &mut Server, name: &str, args: &Value) -> Result<Value, String> {
    let doc_name = args.get("doc").and_then(|v| v.as_str());
    let _ = server.select(doc_name);

    match name {
        "jjcad_document" => document_tool(server, args),
        "jjcad_execute" => {
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or("Parameter 'action' fehlt")?;
            let mut params = args.get("params").cloned().unwrap_or_else(|| json!({}));
            if let (Some(u), Value::Object(m)) =
                (args.get("units").and_then(|v| v.as_str()), &mut params)
            {
                m.insert("units".into(), json!(u));
            }
            let res = server
                .active_mut()
                .execute(action, &params)
                .map_err(|e| format!("{e}"))?;
            Ok(ok_text(pretty(&json!({
                "ok": true,
                "action": action,
                "result": res,
                "state": server.active().info(),
            }))))
        }
        "jjcad_batch" => {
            let steps = args
                .get("steps")
                .and_then(|v| v.as_array())
                .ok_or("Parameter 'steps' fehlt")?;
            let stop = args
                .get("stop_on_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let units = args.get("units").and_then(|v| v.as_str());
            let mut results = Vec::new();
            // Index des Schritts, an dem der Batch abgebrochen wurde. `None` =
            // kein Abbruch (alle Schritte gelaufen oder `stop_on_error: false`).
            let mut abgebrochen: Option<usize> = None;
            for (i, step) in steps.iter().enumerate() {
                let action = match step.get("action").and_then(|v| v.as_str()) {
                    Some(a) => a.to_string(),
                    None => {
                        results.push(json!({"index": i, "ok": false, "error": "action fehlt"}));
                        if stop {
                            abgebrochen = Some(i);
                            break;
                        }
                        continue;
                    }
                };
                let mut params = step.get("params").cloned().unwrap_or_else(|| json!({}));
                if let (Some(u), Value::Object(m)) = (units, &mut params) {
                    m.insert("units".into(), json!(u));
                }
                match server.active_mut().execute(&action, &params) {
                    Ok(res) => {
                        results.push(json!({"index": i, "action": action, "ok": true, "result": res}))
                    }
                    Err(e) => {
                        results.push(json!({"index": i, "action": action, "ok": false, "error": e}));
                        if stop {
                            abgebrochen = Some(i);
                            break;
                        }
                    }
                }
            }
            let errors = results
                .iter()
                .filter(|r| r.get("ok").and_then(|v| v.as_bool()) == Some(false))
                .count();
            // `executed` zählt nur **erfolgreiche** Schritte — vorher zählte es
            // auch den Fehlschlag mit, dann war der Zähler als Erfolgsmaß
            // unbrauchbar (`executed - errors` stimmte nur zufällig).
            let executed = results.len() - errors;
            // Restliche Schritte hat der Batch gar nicht mehr angefasst; ihre
            // Indizes sind `aborted_at + 1 … aborted_at + nicht_ausgefuehrt`.
            let nicht_ausgefuehrt = steps.len() - results.len();
            Ok(ok_text(pretty(&json!({
                "ok": errors == 0,
                "executed": executed,
                "errors": errors,
                "aborted_at": abgebrochen,
                "nicht_ausgefuehrt": nicht_ausgefuehrt,
                "results": results,
                "state": server.active().info(),
            }))))
        }
        "jjcad_query" => {
            let mut params = json!({"ids": args.get("ids")});
            if let Some(l) = args.get("layer") {
                params["layer"] = l.clone();
            }
            if let Some(t) = args.get("type") {
                params["type"] = t.clone();
            }
            if let Some(b) = args.get("bbox").and_then(|v| v.as_array()) {
                if b.len() == 4 {
                    params["a"] = json!([b[0], b[1]]);
                    params["b"] = json!([b[2], b[3]]);
                    params["units"] = json!("mm");
                }
            }
            let mut res = server.active().query(&params);
            if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                if let Some(arr) = res.get_mut("entities").and_then(|v| v.as_array_mut()) {
                    arr.truncate(limit as usize);
                }
            }
            Ok(ok_text(pretty(&res)))
        }
        "jjcad_export" => {
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .ok_or("Parameter 'format' fehlt")?;
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Parameter 'path' fehlt")?;
            let path = PathBuf::from(path);
            let sheet = args.get("sheet").and_then(|v| v.as_u64()).map(|v| v as usize);
            match format {
                "pdf" => {
                    let res = match sheet {
                        Some(i) => pdf::export_pdf(&server.active().doc, &path, Some(i)),
                        None => pdf::export_pdf(&server.active().doc, &path, Some(0)),
                    };
                    res?;
                }
                "dxf" => dxf::export_dxf(&server.active().doc, &path)?,
                "png" => {
                    let opts = RasterOptions {
                        width_px: args.get("width").and_then(|v| v.as_u64()).unwrap_or(2000) as u32,
                        height_px: args
                            .get("height")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1400) as u32,
                        sheet,
                        ..Default::default()
                    };
                    raster::render_png(&server.active().doc, &opts, &path)?;
                }
                other => return Err(format!("Unbekanntes Format '{other}'")),
            }
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(ok_text(pretty(&json!({
                "ok": true, "format": format, "path": path.to_string_lossy(), "bytes": size,
            }))))
        }
        "jjcad_render" => {
            let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1400) as u32;
            let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(1000) as u32;
            let sheet = args.get("sheet").and_then(|v| v.as_u64()).map(|v| v as usize);
            let opts = RasterOptions {
                width_px: width,
                height_px: height,
                sheet,
                ..Default::default()
            };
            let bytes = raster::png_bytes(&server.active().doc, &opts)?;
            Ok(json!({
                "content": [
                    {"type": "image", "data": base64(&bytes), "mimeType": "image/png"},
                    {"type": "text", "text": format!("PNG {}×{} ({} Bytes)", width, height, bytes.len())}
                ],
                "isError": false
            }))
        }
        "jjcad_undo" => {
            let ok = server.active_mut().undo();
            Ok(ok_text(pretty(&json!({"ok": ok, "state": server.active().info()}))))
        }
        "jjcad_redo" => {
            let ok = server.active_mut().redo();
            Ok(ok_text(pretty(&json!({"ok": ok, "state": server.active().info()}))))
        }
        "jjcad_actions" => {
            let group = args.get("group").and_then(|v| v.as_str());
            Ok(ok_text(pretty(&action_catalog(group))))
        }
        "jjcad_sheet" => sheet_tool(server, args),
        other => Err(format!("Unbekanntes Werkzeug '{other}'")),
    }
}

fn document_tool(server: &mut Server, args: &Value) -> Result<Value, String> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or("Parameter 'action' fehlt")?;
    match action {
        "new" => {
            let mut s = Session::new();
            if let Some(u) = args.get("units").and_then(|v| v.as_str()) {
                let _ = s.execute("doc.settings", &json!({"input_unit": u}));
            }
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("doc{}", server.docs.len() + 1));
            s.doc.name = name.clone();
            if let Some(p) = args.get("project") {
                let _ = s.execute("doc.project", p);
            }
            server.docs.push(DocEntry {
                name: name.clone(),
                session: s,
            });
            server.active = server.docs.len() - 1;
            Ok(ok_text(pretty(&json!({
                "ok": true, "document": name, "state": server.active().info()
            }))))
        }
        "open" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Parameter 'path' fehlt")?;
            let i = server.find_or_open(Path::new(path))?;
            server.active = i;
            let name = server.docs[i].name.clone();
            if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
                server.docs[i].name = n.to_string();
            }
            Ok(ok_text(pretty(&json!({
                "ok": true, "document": name, "state": server.active().info()
            }))))
        }
        "save" | "save_as" => {
            let existing = server.active().doc.path.clone();
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or(existing)
                .ok_or("Kein Pfad bekannt – 'path' angeben")?;
            let p = PathBuf::from(&path);
            server.active_mut().save(&p)?;
            let name = server.active().doc.name.clone();
            if let Some(e) = server.docs.get_mut(server.active) {
                e.name = name;
            }
            Ok(ok_text(pretty(&json!({
                "ok": true, "path": path, "state": server.active().info()
            }))))
        }
        "info" => Ok(ok_text(pretty(&server.active().info()))),
        "list" => {
            let docs: Vec<Value> = server
                .docs
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    json!({
                        "index": i,
                        "name": d.name,
                        "title": d.session.doc.name,
                        "path": d.session.doc.path,
                        "entities": d.session.doc.entities.len(),
                        "active": i == server.active,
                    })
                })
                .collect();
            Ok(ok_text(pretty(&json!({"documents": docs}))))
        }
        "close" => {
            if server.docs.len() <= 1 {
                return Err("Das letzte Dokument kann nicht geschlossen werden".into());
            }
            server.docs.remove(server.active);
            server.active = 0;
            Ok(ok_text(pretty(&json!({"ok": true, "state": server.active().info()}))))
        }
        "units" => {
            let u = args
                .get("units")
                .and_then(|v| v.as_str())
                .and_then(Unit::from_str_loose)
                .ok_or("Parameter 'units' (mm|cm|m) fehlt")?;
            server
                .active_mut()
                .execute("doc.settings", &json!({"input_unit": u.short()}))?;
            Ok(ok_text(pretty(&json!({"ok": true, "units": u.short()}))))
        }
        other => Err(format!("Unbekannte Dokumentaktion '{other}'")),
    }
}

fn sheet_tool(server: &mut Server, args: &Value) -> Result<Value, String> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or("Parameter 'action' fehlt")?;
    match action {
        "list" => {
            let sheets: Vec<Value> = server
                .active()
                .doc
                .sheets
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let einheit = server.active().doc.settings.input_unit;
                    json!({
                        "index": i,
                        "id": s.id,
                        "name": s.name,
                        "paper": s.paper.label(),
                        "landscape": s.landscape,
                        "scale": s.scale,
                        "units": einheit.short(),
                        "viewports": s.viewports()
                            .map(|(_, v)| actions::viewport_info(einheit, v))
                            .collect::<Vec<_>>(),
                    })
                })
                .collect();
            Ok(ok_text(pretty(&json!({"sheets": sheets}))))
        }
        "create" => {
            let mut params = json!({
                "paper": args.get("paper").and_then(|v| v.as_str()).unwrap_or("A3"),
                "scale": args.get("scale").and_then(|v| v.as_f64()).unwrap_or(100.0),
            });
            if let Some(n) = args.get("name") {
                params["name"] = n.clone();
            }
            let res = server.active_mut().execute("sheet.new", &params)?;
            Ok(ok_text(pretty(&res)))
        }
        "update" => {
            let idx = args.get("sheet").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let sid = server
                .active()
                .doc
                .sheets
                .get(idx)
                .map(|s| s.id)
                .ok_or("Blatt nicht gefunden")?;
            let mut params = json!({"sheet": sid});
            if let Some(p) = args.get("paper") {
                params["paper"] = p.clone();
            }
            if let Some(s) = args.get("scale") {
                params["scale"] = s.clone();
            }
            let res = server.active_mut().execute("sheet.update", &params)?;
            Ok(ok_text(pretty(&res)))
        }
        "add_viewport" | "update_viewport" | "fit_viewport" | "remove_viewport" => {
            let idx = args.get("sheet").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let sid = server
                .active()
                .doc
                .sheets
                .get(idx)
                .map(|s| s.id)
                .ok_or("Blatt nicht gefunden")?;
            let mut params = json!({"sheet": sid});
            // Das Ansichtsfenster-Objekt wird unverändert durchgereicht; die
            // Einheit seiner `center`-Angabe steht als `units` daneben — wie bei
            // `jjcad_execute`/`jjcad_batch`. Ohne diesen Durchgriff deutete der
            // Kern `center: [5618, 7076]` in einem Meter-Dokument als 5618 m,
            // das Blatt blieb leer und die Antwort sagte trotzdem `ok: true`.
            if let Some(vp) = args.get("viewport") {
                if let Some(obj) = vp.as_object() {
                    for (k, v) in obj {
                        params[k] = v.clone();
                    }
                } else if let Some(idn) = vp.as_u64() {
                    // `viewport: 47` adressiert ein vorhandenes Fenster.
                    params["viewport"] = json!(idn);
                }
            }
            let mut durch = |k: &str| {
                if let Some(v) = args.get(k) {
                    params[k] = v.clone();
                }
            };
            durch("units");
            durch("margin_mm");
            durch("scale");
            let ziel = match action {
                "add_viewport" => "sheet.add_viewport",
                "update_viewport" => "sheet.update_viewport",
                "fit_viewport" => "sheet.fit_viewport",
                _ => "sheet.remove_viewport",
            };
            let res = server.active_mut().execute(ziel, &params)?;
            Ok(ok_text(pretty(&res)))
        }
        other => Err(format!("Unbekannte Blattaktion '{other}'")),
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC
// ---------------------------------------------------------------------------

fn handle(server: &mut Server, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));

    if id.is_none() {
        eprintln!("[jjcad-mcp] notification: {method}");
        return None;
    }
    let id = id.unwrap();

    let result: Result<Value, String> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {"listChanged": false},
                "resources": {"subscribe": false, "listChanged": false},
            },
            "serverInfo": {
                "name": "jjcad-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": "JoJa-CAD steuert 2D-Architekturzeichnungen (Bauantrag). Zuerst jjcad_actions lesen, dann jjcad_execute/jjcad_batch; Koordinaten standardmäßig in Metern (einstellbar über units). Mit jjcad_render visuell prüfen, mit jjcad_export PDF/DXF erzeugen."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_list()})),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(server, &name, &args) {
                Ok(v) => Ok(v),
                Err(e) => Ok(err_text(e)),
            }
        }
        "resources/list" => {
            let mut res: Vec<Value> = vec![json!({
                "uri": "jjcad://document",
                "name": "Aktive Zeichnung",
                "description": "Dokumentzustand als JSON",
                "mimeType": "application/json"
            })];
            for (i, d) in server.docs.iter().enumerate() {
                res.push(json!({
                    "uri": format!("jjcad://document/{i}"),
                    "name": d.name,
                    "mimeType": "application/json"
                }));
            }
            Ok(json!({"resources": res}))
        }
        "resources/read" => {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            let idx = uri
                .rsplit('/')
                .next()
                .and_then(|x| x.parse::<usize>().ok())
                .unwrap_or(server.active);
            if let Some(d) = server.docs.get(idx) {
                Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": pretty(&json!({
                            "info": d.session.info(),
                            "layers": d.session.doc.layers.iter().map(|l| json!({
                                "id": l.id, "name": l.name, "color": l.color.to_hex(),
                                "visible": l.visible, "locked": l.locked,
                                "effektiv_sichtbar": d.session.doc.layer_sichtbar(l.id),
                                "plan": l.plan
                            })).collect::<Vec<_>>(),
                            "plans": d.session.doc.plans.iter().map(|p| json!({
                                "id": p.id, "name": p.name, "visible": p.visible, "locked": p.locked
                            })).collect::<Vec<_>>(),
                            "project": d.session.doc.project,
                        }))
                    }]
                }))
            } else {
                Err("Ressource nicht gefunden".into())
            }
        }
        "prompts/list" => Ok(json!({"prompts": []})),
        other => Err(format!("Unbekannte Methode '{other}'")),
    };

    Some(match result {
        Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
        Err(e) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32603, "message": e}
        }),
    })
}

// ---------------------------------------------------------------------------
// Selbsttest ohne MCP-Client
// ---------------------------------------------------------------------------

fn selftest() -> Result<(), String> {
    let mut server = Server::new();
    let steps = json!([
        {"action": "doc.settings", "params": {"input_unit": "m", "annotation_scale": 100}},
        {"action": "draw.wall", "params": {"a": [0,0], "b": [8,0], "thickness": 0.3, "layer": "Wände"}},
        {"action": "draw.wall", "params": {"a": [8,0], "b": [8,6], "thickness": 0.3, "layer": "Wände"}},
        {"action": "draw.wall", "params": {"a": [8,6], "b": [0,6], "thickness": 0.3, "layer": "Wände"}},
        {"action": "draw.wall", "params": {"a": [0,6], "b": [0,0], "thickness": 0.3, "layer": "Wände"}},
        {"action": "draw.door", "params": {"center": [2,0], "width": 1.0, "layer": "Türen"}},
        {"action": "draw.window", "params": {"center": [5,0], "width": 1.6, "layer": "Fenster"}},
        {"action": "dim.chain", "params": {"pts": [[0,0],[4,0],[8,0]], "line_pos": [0,-1.2], "axis": "x"}},
        {"action": "block.insert", "params": {"block": "Tisch", "pos": [4,3]}},
        {"action": "draw.text", "params": {"p": [1,4], "text": "WOHNEN", "height": 0.3}}
    ]);
    let res = call_tool(&mut server, "jjcad_batch", &json!({"steps": steps}))?;
    let text = res
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    println!("Batch: {}", &text.chars().take(240).collect::<String>());

    let out = PathBuf::from(std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into()))
        .join("jjcad-selftest");
    std::fs::create_dir_all(&out).ok();
    let pdf_path = out.join("selftest.pdf");
    let png_path = out.join("selftest.png");
    let dxf_path = out.join("selftest.dxf");
    let jjc_path = out.join("selftest.jjcad");
    pdf::export_pdf(&server.active().doc, &pdf_path, Some(0))?;
    raster::render_png(
        &server.active().doc,
        &RasterOptions {
            width_px: 1200,
            height_px: 850,
            ..Default::default()
        },
        &png_path,
    )?;
    dxf::export_dxf(&server.active().doc, &dxf_path)?;
    server.active_mut().save(&jjc_path)?;

    println!("{}", pretty(&server.active().info()));
    println!("PDF: {}", pdf_path.display());
    println!("PNG: {}", png_path.display());
    println!("DXF: {}", dxf_path.display());
    println!("JJCad: {}", jjc_path.display());

    let reloaded = jjcad_core::persist::load_document(&jjc_path)?;
    println!(
        "Reload: {} Objekte, {} Layer, {} Blätter",
        reloaded.entities.len(),
        reloaded.layers.len(),
        reloaded.sheets.len()
    );
    if reloaded.entities.len() != server.active().doc.entities.len() {
        return Err("Reload ergibt andere Objektzahl".into());
    }

    // T0038: die beiden Repros aus dem Ticket dürfen nie wieder still
    // „ok" melden — senkrechte Maßkette ohne `axis` misst senkrecht, eine Tür
    // ohne Wand ist ein Fehler.
    document_tool(
        &mut server,
        &json!({"action": "new", "units": "m", "name": "t0038"}),
    )?;
    let res = call_tool(
        &mut server,
        "jjcad_execute",
        &json!({"action": "dim.chain",
                "params": {"pts": [[0,0],[0,6.15],[0,8.76]], "line_pos": [-1.0, 0]}}),
    )?;
    let kette = ergebnis_text(&res);
    if !kette.contains("\"ok\": true") {
        return Err(format!("Maßkette ohne Achse abgelehnt: {kette}"));
    }
    // Eine abgelehnte Action kommt als Fehler aus `call_tool` (im MCP-Protokoll
    // wird daraus `isError: true`) — genau das ist hier gewollt.
    let tuer = match call_tool(
        &mut server,
        "jjcad_execute",
        &json!({"action": "draw.door", "params": {"center": [50,50], "width": 1.0}}),
    ) {
        Ok(v) => return Err(format!("Tür ohne Wand meldet keinen Fehler: {v}")),
        Err(e) => e,
    };
    println!("T0038: Maßkette senkrecht ok, Tür ohne Wand → Fehler: {tuer}");
    Ok(())
}

/// Textinhalt eines MCP-Werkzeugergebnisses (`content[0].text`).
fn ergebnis_text(v: &Value) -> String {
    v.get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Werkzeugergebnis als JSON (die Antwort steckt als Text in `content[0]`).
    fn antwort(res: &Value) -> Value {
        serde_json::from_str(&ergebnis_text(res)).expect("Antwort ist kein JSON")
    }

    fn neues_dokument(server: &mut Server) {
        document_tool(server, &json!({"action": "new", "units": "m"})).expect("neues Dokument");
    }

    /// Repro aus dem Ticket: drei Schritte, Schritt 2 ist unbekannt.
    #[test]
    fn batch_bricht_ab_und_zaehlt_nur_erfolge() {
        let mut server = Server::new();
        neues_dokument(&mut server);
        let steps = json!([
            {"action": "draw.line", "params": {"a": [0, 0], "b": [1, 0]}},
            {"action": "gibt.es.nicht", "params": {}},
            {"action": "draw.line", "params": {"a": [0, 1], "b": [1, 1]}},
        ]);
        let a = antwort(
            &call_tool(&mut server, "jjcad_batch", &json!({"steps": steps})).expect("Batch"),
        );
        assert_eq!(a["ok"], json!(false), "Fehlschlag muss gemeldet werden");
        // 1 Schritt erfolgreich, 1 fehlerhaft — vorher stand hier 2.
        assert_eq!(a["executed"], json!(1));
        assert_eq!(a["errors"], json!(1));
        assert_eq!(a["aborted_at"], json!(1));
        assert_eq!(a["nicht_ausgefuehrt"], json!(1));
        assert_eq!(a["results"].as_array().map(|r| r.len()), Some(2));
        // Der dritte Schritt wurde wirklich nicht ausgeführt.
        assert_eq!(server.active().doc.entities.len(), 1);
    }

    /// `stop_on_error: false` führt weiter aus — dann gibt es keinen Abbruch.
    #[test]
    fn batch_ohne_stop_zaehlt_alle_erfolge() {
        let mut server = Server::new();
        neues_dokument(&mut server);
        let steps = json!([
            {"action": "draw.line", "params": {"a": [0, 0], "b": [1, 0]}},
            {"action": "gibt.es.nicht", "params": {}},
            {"action": "draw.line", "params": {"a": [0, 1], "b": [1, 1]}},
        ]);
        let a = antwort(
            &call_tool(
                &mut server,
                "jjcad_batch",
                &json!({"steps": steps, "stop_on_error": false}),
            )
            .expect("Batch"),
        );
        assert_eq!(a["executed"], json!(2));
        assert_eq!(a["errors"], json!(1));
        assert_eq!(a["aborted_at"], Value::Null);
        assert_eq!(a["nicht_ausgefuehrt"], json!(0));
        assert_eq!(server.active().doc.entities.len(), 2);
    }

    /// Repro aus dem Ticket: leere Zeichnung meldet keine Blöcke als Bestand.
    #[test]
    fn leere_zeichnung_meldet_null_bloecke() {
        let mut server = Server::new();
        neues_dokument(&mut server);
        let info = antwort(
            &call_tool(&mut server, "jjcad_document", &json!({"action": "info"}))
                .expect("info"),
        );
        assert_eq!(info["summary"], json!("0 Objekte · 0 Wände · 0 Türen · 0 Fenster · 0 Blockinstanzen · 0 Sonstige"));
        assert_eq!(info["zaehlung"]["blockinstanzen"], json!(0));
        // Die Systembibliothek ist Programmausstattung und steht getrennt.
        assert!(
            info["systembloecke"].as_u64().unwrap_or(0) > 0,
            "Systemblockbibliothek fehlt: {info}"
        );
        assert_eq!(info["blocks"], info["systembloecke"]);
    }

    /// Die Teilsummen der Zählung müssen die Gesamtzahl ergeben.
    #[test]
    fn summary_teilsummen_ergeben_die_objektzahl() {
        let mut server = Server::new();
        neues_dokument(&mut server);
        let steps = json!([
            {"action": "draw.wall", "params": {"a": [0,0], "b": [8,0], "thickness": 0.3, "layer": "Wände"}},
            {"action": "draw.wall", "params": {"a": [8,0], "b": [8,6], "thickness": 0.3, "layer": "Wände"}},
            {"action": "draw.wall", "params": {"a": [8,6], "b": [0,6], "thickness": 0.3, "layer": "Wände"}},
            {"action": "draw.door", "params": {"center": [2,0], "width": 1.0, "layer": "Türen"}},
            {"action": "draw.window", "params": {"center": [5,0], "width": 1.6, "layer": "Fenster"}},
            {"action": "block.insert", "params": {"block": "Tisch", "pos": [4,3]}},
            {"action": "draw.text", "params": {"p": [1,4], "text": "WOHNEN", "height": 0.3}},
            {"action": "draw.line", "params": {"a": [0,0], "b": [8,6]}}
        ]);
        let a = antwort(
            &call_tool(&mut server, "jjcad_batch", &json!({"steps": steps})).expect("Batch"),
        );
        assert_eq!(a["ok"], json!(true), "Batch: {a}");
        assert_eq!(a["executed"], json!(8));

        let info = antwort(
            &call_tool(&mut server, "jjcad_document", &json!({"action": "info"}))
                .expect("info"),
        );
        let z = &info["zaehlung"];
        let n = |k: &str| z[k].as_u64().unwrap_or_else(|| panic!("Feld {k}: {z}"));
        let summe = n("waende") + n("tueren") + n("fenster") + n("blockinstanzen") + n("sonstige");
        assert_eq!(Some(summe), info["entities"].as_u64());
        assert_eq!(Some(n("objekte")), info["entities"].as_u64());
        assert_eq!(n("waende"), 3);
        assert_eq!(n("tueren"), 1);
        assert_eq!(n("fenster"), 1);
        assert_eq!(n("blockinstanzen"), 1);
        // Der Text in der summary nennt dieselben Zahlen.
        let text = info["summary"].as_str().unwrap_or_default();
        assert!(text.starts_with("8 Objekte · 3 Wände · 1 Türen · 1 Fenster · 1 Blockinstanzen · 2 Sonstige"), "{text}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--selftest") {
        if let Err(e) = selftest() {
            eprintln!("Selbsttest fehlgeschlagen: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|a| a == "--tools") {
        println!("{}", pretty(&tool_list()));
        return;
    }

    eprintln!(
        "[jjcad-mcp] JoJa-CAD MCP-Server {} gestartet (stdio)",
        env!("CARGO_PKG_VERSION")
    );
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut server = Server::new();
    if let Some(path) = args.iter().skip(1).find(|a| !a.starts_with("--")) {
        match server.find_or_open(Path::new(path)) {
            Ok(i) => {
                server.active = i;
                eprintln!("[jjcad-mcp] geladen: {path}");
            }
            Err(e) => eprintln!("[jjcad-mcp] konnte '{path}' nicht laden: {e}"),
        }
    }

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0", "id": Value::Null,
                    "error": {"code": -32700, "message": format!("Parse-Fehler: {e}")}
                });
                let _ = writeln!(stdout, "{resp}");
                let _ = stdout.flush();
                continue;
            }
        };
        let messages: Vec<Value> = match msg {
            Value::Array(a) => a,
            other => vec![other],
        };
        let mut responses = Vec::new();
        for m in &messages {
            if let Some(r) = handle(&mut server, m) {
                responses.push(r);
            }
        }
        if responses.is_empty() {
            continue;
        }
        let out = if responses.len() == 1 {
            responses.remove(0)
        } else {
            Value::Array(responses)
        };
        if writeln!(stdout, "{out}").is_err() {
            break;
        }
        let _ = stdout.flush();
    }
    eprintln!("[jjcad-mcp] beendet");
}
