# MCP-Server: JoJa-CAD für KI-Agenten

Der MCP-Server (`jjcad-mcp`) macht die gesamte CAD-Funktionalität **ohne GUI**
steuerbar. Er spricht JSON-RPC 2.0 über **stdio** und implementiert das Model
Context Protocol (Protokollversion `2024-11-05`).

Damit kann ein Agent Zeichnungen erstellen, prüfen, ändern und als PDF/DXF/PNG
ausgeben — für automatisierte Planerstellung, Batch-Verarbeitung und Tests.

## Starten

```bash
cargo build --release -p jjcad-mcp
./target/release/jjcad-mcp                 # stdio-Server
./target/release/jjcad-mcp zeichnung.jjcad # startet mit geöffneter Datei
./target/release/jjcad-mcp --selftest      # kompletten Durchlauf prüfen
./target/release/jjcad-mcp --tools         # Werkzeugliste als JSON
```

Logs gehen ausschließlich nach **stderr** — stdout bleibt dem Protokoll vorbehalten.

### Konfiguration (Claude Desktop, Cursor, …)

```json
{
  "mcpServers": {
    "joja-cad": {
      "command": "/home/joschua/JoJa-CAD/JoJa-CAD_v0.2/target/release/jjcad-mcp",
      "args": []
    }
  }
}
```

## Werkzeuge

| Werkzeug | Zweck |
|---|---|
| `jjcad_document` | `new`, `open`, `save`, `save_as`, `info`, `list`, `close`, `units` |
| `jjcad_execute` | Eine Aktion ausführen (IDs siehe `jjcad_actions`) |
| `jjcad_batch` | Viele Aktionen in einem Aufruf (mit Fehlerkontext) |
| `jjcad_query` | Objekte nach ID, Layer, Typ oder Bereich abfragen; geschlossene Objekte tragen `area_m2` |
| `jjcad_export` | PDF, DXF oder PNG schreiben |
| `jjcad_render` | PNG als **Bildinhalt** zurückgeben (visuelles Feedback) |
| `jjcad_undo` / `jjcad_redo` | Historie |
| `jjcad_actions` | Aktionskatalog (ID, Gruppe, Hinweis) |
| `jjcad_sheet` | Blätter und Ansichtsfenster verwalten |

Mehrere Dokumente werden über den optionalen Parameter `doc` (Name aus
`jjcad_document list`) adressiert.

**Flächen:** `jjcad_query` liefert bei geschlossenen Objekten (Polylinie/Rechteck,
Kreis, Ellipse, geschlossene Füllfläche/Volumen, Wandkörper) `area_m2` als
deutsche Zahl mit zwei Nachkommastellen (`"12,00"`) und `area_mm2`; offene
Linienzüge, Texte und Maße tragen das Feld nicht. Für Raumgruppen summiert
`jjcad_execute {"action":"select.area","params":{"ids":[…]}}` die Flächen
(`sum_m2`) und nennt Objekte ohne Fläche in `without_area`.

## Typischer Ablauf

1. `jjcad_actions` — Katalog lesen (oder `docs/BEFEHLE.md`).
2. `jjcad_document {"action":"new","units":"m","project":{...}}`.
3. `jjcad_batch` mit den Zeichenbefehlen.
4. `jjcad_render` — Bild prüfen, bei Bedarf korrigieren.
5. `jjcad_export {"format":"pdf","path":"…/A-01.pdf"}` und
   `jjcad_document {"action":"save","path":"…/haus.jjcad"}`.

## Beispielsitzung

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "protocolVersion":"2024-11-05","capabilities":{},
  "clientInfo":{"name":"agent","version":"1"}}}
```

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
  "name":"jjcad_batch","arguments":{
    "units":"m",
    "steps":[
      {"action":"draw.wall","params":{"a":[0,0],"b":[8,0],"thickness":0.3}},
      {"action":"draw.wall","params":{"a":[8,0],"b":[8,6],"thickness":0.3}},
      {"action":"draw.wall","params":{"a":[8,6],"b":[0,6],"thickness":0.3}},
      {"action":"draw.wall","params":{"a":[0,6],"b":[0,0],"thickness":0.3}},
      {"action":"draw.door","params":{"center":[2,0],"width":1.0}},
      {"action":"draw.window","params":{"center":[5,0],"width":1.6}},
      {"action":"dim.chain","params":{"pts":[[0,0],[4,0],[8,0]],"line_pos":[0,-1.2]}}
    ]}}}
```

Antwort (gekürzt):

```json
{"ok": true, "executed": 7, "errors": 0,
 "results": [{"index":0,"action":"draw.wall","ok":true,"result":{"created":[13],"length_mm":8000.0}}],
 "state": {"entities": 7, "bbox": {"min": [-150, -150], "max": [8150, 6150]}}}
```

## Bilder prüfen

`jjcad_render` liefert zwei Inhaltsteile:

```json
{"content": [
  {"type":"image","data":"iVBORw0KGgo…","mimeType":"image/png"},
  {"type":"text","text":"PNG 1400×1000 (… Bytes)"}
]}
```

Agenten mit Bildverständnis können damit kontrollieren, ob Wände geschlossen,
Türen richtig angeschlagen und Maße plausibel sind — der gleiche Renderer wie in
der GUI und im PDF sorgt für Übereinstimmung.

## Hinweise für Agenten

- Koordinaten sind standardmäßig in **Metern** (`units:"mm"` für Millimeter).
- Wände sind **Achsen mit Dicke**; Ecken werden beim Rendern geschlossen.
- Türen/Fenster finden ihre Wand selbst (`host` wird automatisch gesetzt).
- Maßketten brauchen `line_pos` (Punkt auf der Maßlinie) — sonst wird ein
  Standardabstand gesetzt.
- `modify.trim` erwartet `at` auf dem **zu entfernenden** Abschnitt.
- Nach Änderungen liefert jede Antwort den aktuellen Zustand (`state`) mit.
- Ein Schritt = eine Undo-Einheit; `jjcad_batch` ist daher gutmütig zu korrigieren.

## Ressourcen

| URI | Inhalt |
|---|---|
| `jjcad://document` | Zustand, Layer und Projektdaten der aktiven Zeichnung |
| `jjcad://document/<n>` | dasselbe für Dokument `n` |

## Grenzen

- Kein Netzwerkzugriff, keine Authentifizierung — der Server läuft lokal.
- Ein Serverprozess hält seine Dokumente im Speicher; Persistenz über `save`.
- 3D, Renderings und IFC sind nicht Teil des Funktionsumfangs.
