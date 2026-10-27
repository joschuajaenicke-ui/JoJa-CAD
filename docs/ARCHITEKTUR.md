# Architektur

## Ziel

Ein 2D-CAD, das für Bauantragszeichnungen ausreicht, sich wie ein Office-Tool
bedient und sich vollständig von einem KI-Agenten steuern lässt. Daraus folgen
drei harte Architekturentscheidungen:

1. **Kern ohne GUI** — Dokument, Geometrie, Befehle und Exporte liegen in
   `jjcad-core` und funktionieren headless.
2. **Eine Befehls-Schicht für alles** — GUI, Tastatur, Hotbar und MCP rufen
   dieselben Action-IDs auf.
3. **Ein Renderer für alle Ausgaben** — Displayliste statt drei Zeichenwegen.

## Crates

```
jjcad-core     Geometrie, Modell, Renderer, Aktionen, Persistenz, PDF/DXF/PNG
   ▲     ▲
   │     └── jjcad-mcp     JSON-RPC/MCP über stdio (headless)
   └──────── jjcad-gui     eframe/egui-Oberfläche
```

`jjcad-core` hängt nur von `serde`, `tiny-skia` und `ab_glyph` ab. Kein GUI-Code,
keine globalen Zustände.

## Datenfluss

```
            ┌──────────── Aktion (JSON) ────────────┐
            │                                       │
   GUI-Klick │                            MCP-Aufruf│
            ▼                                       ▼
      jjcad_core::actions::execute(&mut Document, id, params)
            │
            ├─► Document (Entities, Layer, Blöcke, Blätter)
            │        │
            │        └─► render::render_document → DisplayList
            │                    ├─► egui-Canvas   (Bildschirm)
            │                    ├─► pdf::export   (Vektor-PDF)
            │                    └─► raster::render (PNG, headless)
            │
            └─► Session (Undo-Snapshots, Auswahl, letzte Aktion)
```

**Displayliste:** `Prim { shape, style, entity, layer }` mit analytischen Formen
(Linie, Polylinie, Kreis, Bogen, Ellipse, gefülltes Polygon, Text). Jede Ausgabe
interpretiert dieselben Primitive — Wände, Türen, Bemaßung und Blöcke werden also
überall identisch aufgebaut.

## Wichtige Entscheidungen

### Einheit Millimeter, überall

Das Dokument rechnet intern in **mm**. `settings.input_unit` betrifft nur Eingabe
und Anzeige; Aktionen nehmen Koordinaten in dieser Einheit entgegen und rechnen
zentral um (`Params::pt`). Damit ist das Dateiformat unabhängig von der
Benutzereinstellung und MCP-Aufrufe sind reproduzierbar.

### Wände als Achse mit Dicke

Eine Wand speichert nur `a`, `b`, `thickness`, `height`, `material`. Darstellung
(parallele Kanten, Füllung, Laibungen, Ecken) entsteht im Renderer:

- Enden werden an Anschlüssen um die halbe Dicke der Nachbarwand verlängert,
  damit Ecken geschlossen sind.
- Wandflächen werden gegen andere Wände beschnitten (keine Kreuzlinien in Ecken).
- Öffnungen kommen aus Türen/Fenstern mit `host`; der Renderer schneidet sie aus
  und zeichnet Laibungen.

Folge: Verschieben, Drehen, Stutzen und Verlängern funktionieren mit derselben
Logik wie bei Linien — genau wie in der Spezifikation gefordert.

### Bemaßung in Papier-Millimetern

`DimStyle` ist in Papier-mm definiert und wird mit dem Zeichnungsmaßstab
multipliziert. Im Modell ist das `settings.annotation_scale`, im Ansichtsfenster
der Fenstermaßstab. Dadurch stimmt die Strichstärke auf jedem Blatt, und
`{MASSTAB}` im Schriftfeld zeigt den Blattmaßstab.

### Undo über Snapshots

`Session` klont das Dokument vor jeder verändernden Aktion (max. 80 Schritte).
Bei Zeichnungsgrößen im Bauantragsbereich ist das unproblematisch und robuster
als Delta-Patches. `modify.set` wird zusammengefasst, damit Schieberegler nicht
die Historie fluten.

### Blöcke mit Namensräumen

Block-IDs sind präfixiert (`sys:`, `prj:`, `dxf:`). Beim Import kollidierender
Definitionen wird umbenannt, Instanzen werden mitgezogen. Blockinhalte liegen im
Blockkoordinatensystem, Instanzen tragen nur die Transformation.

### Text und dynamische Felder

`{DATUM}`, `{PROJEKT}`, `{BAUHERR}`, `{MASSTAB}` … werden beim Rendern aufgelöst —
nicht beim Zeichnen. Das Schriftfeld bleibt dadurch aktuell, ohne dass Objekte
bearbeitet werden müssen.

## Oberfläche

- **egui/eframe** mit eigener Themen- und Icon-Schicht (Vektor-Icons, keine
  Bilddateien).
- Feste Struktur: Kopfzeile mit Ribbon-Reitern, Zeichnungs-/Blattreiter,
  Werkzeugoptionsleiste, Canvas, Hotbar, Statuszeile; Layer links,
  Eigenschaften rechts.
- **Item-Slots** wie in der Spezifikation: Hotbar (1–0), Inventar, Tastatur-Editor;
  aktives Werkzeug + Klick auf leeren Slot verknüpft, Rechtsklick bietet
  Leeren/Verknüpfen/Bearbeiten, Mitteltaste leert, Ziehen sortiert um.
- Der Zeichnungs-Cache wird über `rev` (Änderungszähler) und Zoom invalidiert.
- **Dateidialog:** der Dialog des Betriebssystems (`rfd`, Modul `systemdialog`) —
  Linux über das XDG-Desktop-Portal (KDE → Dolphin, GNOME → Dateien), Windows über
  den Win32-Dialog, macOS über NSOpenPanel. Er läuft in einem eigenen Thread, die
  Oberfläche bleibt bedienbar. Ohne verfügbaren Systemdialog (kein Sitzungsbus,
  Testumgebung) oder auf Wunsch („Dateidialog: Eingebaut“) greift der eigene Dialog.

## Ausgabe

- **PDF:** eigener Schreiber (kein Fremdpaket): Content-Streams, Helvetica mit
  WinAnsi-Oktalescapes (Umlaute), Ansichtsfenster mit Clip-Rechteck, Papier in mm.
- **DXF:** Export als R12-ASCII (Layer, Linientypen, Blöcke, INSERT); Import für
  LINE, LWPOLYLINE, POLYLINE/VERTEX, CIRCLE, ARC, TEXT/MTEXT, INSERT, SOLID,
  ELLIPSE, SPLINE-Näherung. Nicht unterstützte Typen werden gemeldet, nicht
  verschluckt.
- **PNG:** tiny-skia + ab_glyph, gleiche Displayliste, für Agenten-Feedback und
  Dokumentation.

## Tests

`cargo test` prüft Geometrie (Schnitte, Offset, Spiegelung), Einheiten, Aktionen
(Zeichnen, Architektur, Ändern, Trim/Extend, Blöcke), Undo, Persistenz-Roundtrip,
DXF-Roundtrip, PDF/PNG-Signaturen, Fang und Schriftfeld-Maßstab.

## Offene Punkte

- DWG-Import bleibt bewusst außen vor (proprietär).
- Schraffuren sind Musterlinien, keine frei definierbaren Muster.
- Splines werden nur genähert importiert.
- Druck/Plot-Einstellungen (Randabstände, Papierfach) sind nicht Teil der
  PDF-Ausgabe.
