//! Der MCP-Weg des Blatts (Ticket T0037).
//!
//! Der Defekt saß in `sheet_tool`: das Objekt `viewport` wurde unverändert an
//! `sheet.update_viewport` weitergereicht, ein `units` des Aufrufers aber nicht.
//! In einem Meter-Dokument wurde `center: [5618, 7076]` damit zu 5618 m — das
//! Blatt blieb leer, die Antwort sagte `ok: true`.
//!
//! Geprüft wird hier über stdio, also auf dem Weg, den ein Agent wirklich geht.

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

/// Alle Anfragen in **einem** Serverprozess ausführen (der Zustand lebt dort).
fn rpc(anfragen: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_jjcad-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("jjcad-mcp startet nicht");
    {
        let stdin = child.stdin.as_mut().unwrap();
        for a in anfragen {
            writeln!(stdin, "{a}").unwrap();
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn werkzeug(id: u64, name: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": name, "arguments": args}})
}

/// Inhalt eines `tools/call`-Ergebnisses als JSON (die Werkzeuge liefern Text).
fn inhalt(antwort: &Value) -> Value {
    let text = antwort["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("kein Text in der Antwort: {antwort}"));
    serde_json::from_str(text).unwrap_or_else(|e| panic!("kein JSON: {e} — {text}"))
}

fn nah(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn einheiten_und_einpassen_ueber_mcp() {
    let anfragen = vec![
        // 1 — Zeichnung in Metern, 13 m × 15 m
        werkzeug(1, "jjcad_document", json!({"action": "new", "units": "m"})),
        werkzeug(
            2,
            "jjcad_batch",
            json!({"units": "m", "steps": [
                {"action": "draw.rect", "params": {"a": [0, 0], "b": [13, 15]}}
            ]}),
        ),
        // 2 — Der Reproduktionsfall: `center` in Millimetern, ausdrücklich als
        // solche angesagt. Vorher wurde daraus 5618 m.
        werkzeug(
            3,
            "jjcad_sheet",
            json!({"action": "add_viewport", "sheet": 0, "units": "mm",
                   "viewport": {"rect_mm": [20, 70, 380, 210], "scale": 100,
                                "center": [5618, 7076]}}),
        ),
        // 3 — Ohne Einheit gilt die Dokumenteinheit (Meter).
        werkzeug(
            4,
            "jjcad_sheet",
            json!({"action": "add_viewport", "sheet": 0,
                   "viewport": {"rect_mm": [20, 70, 380, 210], "scale": 100,
                                "center": [5.618, 7.076]}}),
        ),
        // 4 — Einpassen (nimmt das erste Fenster des Blatts)
        werkzeug(5, "jjcad_sheet", json!({"action": "fit_viewport", "sheet": 0})),
        // 5 — Anzeige
        werkzeug(6, "jjcad_sheet", json!({"action": "list"})),
    ];
    let antworten = rpc(&anfragen);
    assert_eq!(antworten.len(), 6, "Antworten: {antworten:#?}");

    let add_mm = inhalt(&antworten[2]);
    // `units` in der Antwort ist die Einheit der **Zeichnung** — sie sagt, in
    // welcher Einheit `center` gelesen wird; `center_mm` ist immer Millimeter.
    assert_eq!(add_mm["viewport_info"]["units"], "m");
    assert!(
        nah(add_mm["viewport_info"]["center_mm"][0].as_f64().unwrap(), 5618.0, 1e-6),
        "units:\"mm\" + center 5618 muss 5618 mm ergeben, war {}",
        add_mm["viewport_info"]["center_mm"]
    );
    assert!(nah(
        add_mm["viewport_info"]["center_mm"][1].as_f64().unwrap(),
        7076.0,
        1e-6
    ));
    // rect_mm bleibt Papier-Millimeter, unabhängig von `units`
    assert!(nah(
        add_mm["viewport_info"]["rect_mm"][2].as_f64().unwrap(),
        380.0,
        1e-9
    ));

    let add_m = inhalt(&antworten[3]);
    assert_eq!(add_m["viewport_info"]["units"], "m");
    assert!(nah(
        add_m["viewport_info"]["center_mm"][0].as_f64().unwrap(),
        5618.0,
        1e-6
    ));

    let fit = inhalt(&antworten[4]);
    assert_eq!(fit["ok"], true, "fit_viewport: {fit}");
    assert_eq!(fit["scale"].as_f64().unwrap(), 100.0);
    // BBox-Mitte (6500, 7500) mm — in Metern 6,5 / 7,5
    assert!(nah(
        fit["viewport_info"]["center_mm"][0].as_f64().unwrap(),
        6500.0,
        1e-6
    ));
    assert!(nah(
        fit["viewport_info"]["center_mm"][1].as_f64().unwrap(),
        7500.0,
        1e-6
    ));

    let liste = inhalt(&antworten[5]);
    let blatt = &liste["sheets"][0];
    assert_eq!(blatt["units"], "m", "sheet list nennt die Einheit nicht");
    let vp = &blatt["viewports"][0];
    // Gelesen wird in derselben Einheit, in die man schreibt.
    assert!(nah(vp["center"][0].as_f64().unwrap(), 6.5, 1e-9));
    assert!(nah(vp["center"][1].as_f64().unwrap(), 7.5, 1e-9));
    assert!(nah(vp["center_mm"][0].as_f64().unwrap(), 6500.0, 1e-6));
    assert!(nah(vp["scale"].as_f64().unwrap(), 100.0, 1e-9));
}

#[test]
fn schema_nennt_name_und_einheit() {
    let out = Command::new(env!("CARGO_BIN_EXE_jjcad-mcp"))
        .arg("--tools")
        .output()
        .unwrap();
    let liste: Value = serde_json::from_slice(&out.stdout).unwrap();
    let sheet = liste
        .as_array()
        .expect("--tools liefert eine Liste")
        .iter()
        .find(|t| t["name"] == "jjcad_sheet")
        .expect("jjcad_sheet fehlt im Schema");
    let props = &sheet["inputSchema"]["properties"];
    assert!(props.get("name").is_some(), "`name` fehlt im Schema");
    assert!(props.get("units").is_some(), "`units` fehlt im Schema");
    let aktionen = sheet["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert!(aktionen.contains(&"fit_viewport"));
}
