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

Der Server läuft **unabhängig vom Programm**: er nimmt keine Sperre auf den
Datenordner, liest weder `prefs.json` noch `sitzung.json` und schreibt nur die
Zeichnungen, die ihm genannt werden. Er kann also parallel zu einem geöffneten
JoJa-CAD laufen — auch auf derselben Datei; die zuletzt gespeicherte Fassung
gewinnt, und das geöffnete Fenster merkt eine von außen geänderte Datei nicht
von selbst.

### Konfiguration (Claude Desktop, Cursor, …)

```json
{
  "mcpServers": {
    "joja-cad": {
      "command": "/pfad/zu/jjcad/target/release/jjcad-mcp",
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
| `jjcad_batch` | Viele Aktionen in einem Aufruf — **bricht beim ersten Fehler ab** (siehe [Batches](#batches-abbruch-beim-ersten-fehler)) |
| `jjcad_query` | Objekte nach ID, Layer, Typ oder Bereich abfragen; geschlossene Objekte tragen `area_m2` |
| `jjcad_export` | PDF, DXF oder PNG schreiben |
| `jjcad_render` | PNG als **Bildinhalt** zurückgeben (visuelles Feedback) |
| `jjcad_undo` / `jjcad_redo` | Historie |
| `jjcad_actions` | Aktionskatalog (ID, Gruppe, Hinweis) |
| `jjcad_sheet` | Blätter und Ansichtsfenster verwalten (Einheiten und Einpassen: siehe unten) |

Mehrere Dokumente werden über den optionalen Parameter `doc` (Name aus
`jjcad_document list`) adressiert.

**Flächen:** `jjcad_query` liefert bei geschlossenen Objekten (Polylinie/Rechteck,
Kreis, Ellipse, geschlossene Füllfläche/Volumen, Wandkörper) `area_m2` als
deutsche Zahl mit zwei Nachkommastellen (`"12,00"`) und `area_mm2`; offene
Linienzüge, Texte und Maße tragen das Feld nicht. Für Raumgruppen summiert
`jjcad_execute {"action":"select.area","params":{"ids":[…]}}` die Flächen
(`sum_m2`) und nennt Objekte ohne Fläche in `without_area`.

## Typischer Ablauf

1. `jjcad_actions` — Katalog lesen (oder `docs/technik/BEFEHLE.md`).
2. `jjcad_document {"action":"new","units":"m","project":{...}}`.
3. `jjcad_batch` mit den Zeichenbefehlen — in kleinen Gruppen (10–20 Schritte),
   denn der Batch bricht beim ersten Fehler ab und schickt den Rest nicht mehr.
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
{"ok": true, "executed": 7, "errors": 0, "aborted_at": null, "nicht_ausgefuehrt": 0,
 "results": [{"index":0,"action":"draw.wall","ok":true,"result":{"created":[13],"length_mm":8000.0}}],
 "state": {"entities": 7, "bbox": {"min": [-150, -150], "max": [8150, 6150]}}}
```

## Batches: Abbruch beim ersten Fehler

`jjcad_batch` arbeitet die Schritte **der Reihe nach** ab und hält beim ersten
fehlerhaften Schritt an. Die folgenden Schritte werden **nicht** ausgeführt —
sie fehlen auch in `results`. Ein Tippfehler in Schritt 12 verschluckt also die
Schritte 13–80, ohne dass irgendetwas davon im Dokument landet.

Deshalb: **Batches in kleinen Gruppen schicken** (etwa 10–20 Schritte), das
Ergebnis jedes Batches prüfen und erst dann den nächsten schicken. Wer 80
Schritte in einem Aufruf schickt, erfährt sonst nur, dass irgendwo in der ersten
Hälfte etwas schiefging.

```json
{"name":"jjcad_batch","arguments":{"steps":[
  {"action":"draw.wall","params":{"a":[0,0],"b":[8,0],"thickness":0.3}},
  {"action":"gibtesnicht","params":{}},
  {"action":"draw.wall","params":{"a":[8,0],"b":[8,6],"thickness":0.3}}
]}}
```

Antwort:

```json
{"ok": false, "executed": 1, "errors": 1, "aborted_at": 1, "nicht_ausgefuehrt": 1,
 "results": [
   {"index":0,"action":"draw.wall","ok":true,"result":{"created":[13]}},
   {"index":1,"action":"gibtesnicht","ok":false,
    "error":"Aktion 'gibtesnicht' ist in dieser Version nicht implementiert"}
 ]}
```

| Feld | Bedeutung |
|---|---|
| `executed` | Anzahl **erfolgreich** ausgeführter Schritte (Fehlschläge zählen nicht mit) |
| `errors` | Anzahl fehlgeschlagener Schritte (ohne `stop_on_error: false` höchstens 1) |
| `aborted_at` | Index des Schritts, bei dem abgebrochen wurde; `null` ohne Abbruch |
| `nicht_ausgefuehrt` | Anzahl der Schritte, die der Batch gar nicht mehr angefasst hat |

Die nicht ausgeführten Schritte sind die Indizes `aborted_at + 1` bis
`aborted_at + nicht_ausgefuehrt`. Mit `"stop_on_error": false` läuft der Batch
weiter und meldet am Ende alle Fehler; dann ist `aborted_at` immer `null` und
`nicht_ausgefuehrt` immer 0. Ein Schritt = eine Undo-Einheit, ein Abbruch lässt
sich also schrittweise zurücknehmen.

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

## Blätter und Ansichtsfenster

`jjcad_sheet` verwaltet Blätter und die Ansichtsfenster darauf. Aktionen:
`list`, `create`, `update`, `remove_viewport`, `add_viewport`,
`update_viewport`, `fit_viewport`.

**Zwei Räume, zwei Einheiten.** Ein Ansichtsfenster steht auf dem Papier, zeigt
aber die Zeichnung. Deshalb steht die Einheit im Feldnamen:

| Feld | Einheit | Bedeutung |
|---|---|---|
| `x`, `y`, `w`, `h` bzw. `rect_mm` | **Papier-Millimeter** | Lage und Größe des Fensters auf dem Blatt |
| `center` | Einheit `units` (Vorgabe: Dokumenteinheit) | Modellpunkt in der Fenstermitte |
| `center_mm` | **Millimeter** | derselbe Punkt, unabhängig von der Dokumenteinheit |
| `scale` | — | Maßstab 1:`scale` |
| `margin_mm` | Papier-Millimeter | Rand beim Einpassen (Vorgabe 10) |

`units` wirkt wie bei `jjcad_execute`/`jjcad_batch`: es gilt für `center` des
Aufrufs, nicht für das Dokument. `sheet list` gibt `units` und **beide**
Schreibweisen des Mittelpunkts aus (`center` in der Dokumenteinheit,
`center_mm` in Millimetern) — was man liest, kann man unverändert
zurückschreiben. `center` und `center_mm` gleichzeitig ist ein Fehler.

```json
{"name":"jjcad_sheet","arguments":{
  "action":"update_viewport","sheet":0,
  "viewport":{"id":47,"rect_mm":[20,70,380,210],"scale":100,"center":[5618,7076]},
  "units":"mm"}}
```

Ohne `units` gilt die Dokumenteinheit: in einem Meter-Dokument wäre
`center: [5618, 7076]` 5618 m — das Fenster zeigte leere Fläche. Wer in
Millimetern denkt, nimmt `center_mm` und ist von der Dokumenteinheit unabhängig.

**Ansicht einpassen.** `fit_viewport` setzt den Mittelpunkt auf die Mitte der
Zeichnungs-BBox und wählt den Maßstab aus der Fenstergröße (nächstgrößerer
gängiger Maßstab, 5 % Luft):

```json
{"name":"jjcad_sheet","arguments":{"action":"fit_viewport","sheet":0,"margin_mm":10}}
```

Antwort (gekürzt):

```json
{"ok": true, "viewport": 51, "scale": 100,
 "bbox_mm": [[0,0],[13000,15000]],
 "gezeichnet_mm": [130, 150],
 "viewport_info": {"rect_mm":[15,83,395,204],"center":..., "center_mm":[6500,7500]}}
```

`gezeichnet_mm` ist die Größe der Zeichnung **auf dem Papier**
(BBox ÷ Maßstab) — passt sie nicht in `rect_mm`, ist das Blatt zu klein oder
der Maßstab zu klein gewählt. Ohne Ansichtsfenster oder bei leerer Zeichnung
meldet die Aktion einen Fehler statt still `ok`.

## Zustand und Bestand (`jjcad_document info`)

Jede Antwort trägt den Zustand des aktiven Dokuments mit. `info` liefert ihn
ausführlich; für den Bestand gilt:

```json
{"entities": 8,
 "summary": "8 Objekte · 3 Wände · 1 Türen · 1 Fenster · 1 Blockinstanzen · 2 Sonstige",
 "zaehlung": {"objekte": 8, "waende": 3, "tueren": 1, "fenster": 1,
              "blockinstanzen": 1, "sonstige": 2},
 "blocks": 19, "systembloecke": 19}
```

- `summary` zählt **nur Objekte der Zeichnung**. Die Teile (`Wände`, `Türen`,
  `Fenster`, `Blockinstanzen`, `Sonstige`) ergeben zusammen die Objektzahl;
  `Sonstige` sind Linien, Maße, Texte und alles andere ohne eigene Spalte.
- `blockinstanzen` sind **eingesetzte** Blöcke im Plan, nicht die Bibliothek.
- `blocks` ist die Zahl der Block**definitionen** in der Datei, `systembloecke`
  davon die mitgelieferten. Eine leere Zeichnung meldet `0 Blockinstanzen` und
  trotzdem 19 Systemblöcke — die Bibliothek ist Programmausstattung, kein
  Zeichnungsinhalt.
- `zaehlung` ist dieselbe Zahl maschinenlesbar; `summary` ist die Zeile für den
  Menschen (GUI „Über“/Zeichnungsinfo).

## Hinweise für Agenten

- Koordinaten sind standardmäßig in **Metern** (`units:"mm"` für Millimeter).
- Wände sind **Achsen mit Dicke**; Ecken werden beim Rendern geschlossen.
- Türen/Fenster finden ihre Wand selbst (`host` wird automatisch gesetzt).
- Maßketten brauchen `line_pos` (Punkt auf der Maßlinie) — sonst wird ein
  Standardabstand gesetzt.
- `modify.trim` erwartet `at` auf dem **zu entfernenden** Abschnitt.
- Nach Änderungen liefert jede Antwort den aktuellen Zustand (`state`) mit.
- Ein Schritt = eine Undo-Einheit; `jjcad_batch` ist daher gutmütig zu
  korrigieren — aber nur bis zum Abbruch: was hinter dem Fehlschlag steht, ist
  nicht gelaufen (siehe [Batches](#batches-abbruch-beim-ersten-fehler)).

## Ressourcen

| URI | Inhalt |
|---|---|
| `jjcad://document` | Zustand, Layer und Projektdaten der aktiven Zeichnung |
| `jjcad://document/<n>` | dasselbe für Dokument `n` |

## Grenzen

- Kein Netzwerkzugriff, keine Authentifizierung — der Server läuft lokal.
- Ein Serverprozess hält seine Dokumente im Speicher; Persistenz über `save`.
- 3D, Renderings und IFC sind nicht Teil des Funktionsumfangs.
