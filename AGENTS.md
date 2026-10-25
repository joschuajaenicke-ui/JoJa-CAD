# JoJa-CAD v0.2 — Projektkontext für Agenten

## Worum es geht
2D-CAD für Architektur- und Bauantragszeichnungen, Rust-Workspace:
`crates/jjcad-core` (Geometrie, Actions, Displayliste, PDF/DXF), `crates/jjcad-gui`
(eframe/egui 0.29), `crates/jjcad-mcp` (JSON-RPC). Interne Einheit Millimeter,
deutsche UI, eine Action-Schicht (Action-IDs) für GUI/Tastatur/MCP, ein
gemeinsamer Renderer für Bildschirm, PDF und PNG.

## Bauen und testen
```
cargo build --release -p jjcad-gui     # Release
cargo test                             # 30 Tests
./scripts/build-live.sh                # Testbinary versioniert nach ~/jjcad-live
./scripts/start-live.sh [datei.jjcad]  # Testversion starten (zeigt Build-Nummer)
```

## Regeln
- **Niemals `pkill -x jjcad`** — das beendet die Instanz des Nutzers. Der Agent
  testet nur eigene Kopien (z. B. `/tmp/jjcad-agent`) auf eigenem Display.
- Jede Änderung: `cargo build`, Build-Nummer prüfen, danach `scripts/build-live.sh`.
- Abnahme möglichst messbar: gespeicherte `.jjcad` prüfen oder Pixel messen,
  nicht „sieht gut aus".
- Rust-Änderungen mit exakten Ankern patchen, nicht per String-Ersetzung.
- Offene Punkte und Stand: `docs/STATUS.md`.
