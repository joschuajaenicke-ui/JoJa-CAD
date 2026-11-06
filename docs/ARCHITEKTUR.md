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

### Zeichenrang: neue Elemente ordnen sich selbst ein (T0048)

Die Objektliste `entities` **ist** die Zeichenreihenfolge: Index 0 liegt ganz
hinten. Bisher hängte jedes neue Element am Ende der Liste und lag damit über
allem anderen — eine zuletzt gezeichnete Füllfläche deckte Wände und Linien zu.

Jede Objektart hat deshalb einen **Rang** (`model::EntityKind::zeichenrang`,
Konstanten `model::RANG_*`):

| Rang | Objektarten | liegt |
|---|---|---|
| 0 `RANG_FLAECHE` | Füllfläche (auch Schraffur) | ganz hinten |
| 1 `RANG_VOLUMEN` | Volumen | über den Flächen |
| 2 `RANG_BAUTEIL` | Wand, Tür, Fenster, Treppe, Block | über den Flächen, unter den Linien |
| 3 `RANG_LINIE` | Punkt, Linie, Polylinie, Spline, Rechteck, Kreis, Bogen, Ellipse | über den Bauteilen |
| 4 `RANG_BESCHRIFTUNG` | Text, Maß, Maßkette, Grundriss- und Schnitt-Kote | ganz vorn |

Eingefügt wird über `Document::insert_ranked` (jeder Erzeuger, jede Kopie, jede
Einfüge-Vorschau geht diesen Weg; `Document::add_entity` delegiert dorthin). Der
Platz ist das Minimum aus **erster Stelle mit höherem Rang** und **direkt hinter
dem letzten Objekt gleichen Rangs** (`Document::einfuege_index`):

- Nichts Neues liegt über einer Linie oder Wand: die Klassengrenze ist die harte
  Obergrenze.
- Innerhalb der eigenen Klasse liegt das Neue oben auf — zwei Füllflächen
  überdecken sich also nicht gegenseitig in der Zeichenreihenfolge.
- **Bestehende Objekte werden nie umsortiert**, sie rücken nur um eine Stelle.
  Eine von Hand verdrehte Reihenfolge (`modify.order`) bleibt damit erhalten; das
  neue Element ordnet sich an der Grenze seiner Klasse ein, auch in einer
  verdrehten Zeichnung (dann gewinnt „hinter höherem Rang" — nichts darf eine
  Linie überdecken).

Der Rang ist bewusst **keine** Geometrieprüfung (kein „berührt sich"): die Regel
ist ohne Sonderfälle erklärbar, in einem Durchgang über die Liste gerechnet und
in den meisten Fällen richtig. Wer eine Objektart ergänzt, ordnet sie in
`zeichenrang` einer Klasse zu — der Compiler erzwingt den Zweig.

### Dämmung als Linienart (T0120)

Die Linienart **Dämmung** (`LineType::Insulation`) ist kein Strichmuster: sie
zeichnet ein **Schlaufenband** entlang der Linie. Die Kurve ist die
dokumentierte parametrische Schlaufenkurve (`docs/DAEMMUNG-SCHLAUFEN.md`), die
Rechnung steht in `crates/jjcad-core/src/daemmung.rs`:

```
x(t) = L/(2πn)·(t + K·sin 2t),  y(t) = H/2·cos t,  n = max(½, round(2L/(r·H))/2)
```

`H` (Breite) ist die Dämmstärke in mm, `r` (Schlaufenform) das Verhältnis
Periodenlänge/Höhe. Bei `t = 2πn` ist `x = L` **exakt** — die Länge der Linie
bleibt unabhängig von `H` und `r` erhalten. Die Konstante `K = 2,3016694` muss
auf allen Stellen stehen, sonst ragt die Kurve über die Enden hinaus.

Übersetzt wird die Linienart **an einer Stelle**: `render::DisplayList::push`
ersetzt den Prim durch den Polygonzug des Bandes. Damit zeichnen Bildschirm,
PDF, PNG und DXF dieselbe Geometrie, und `render::trefferbild` (Trefferfläche,
Klick) sowie der DXF-Export folgen automatisch. Auf Polylinie, Bogen, Kreis und
Ellipse läuft die Kurve über die **Bogenlänge** (`x` = Weg, `y` = Versatz quer
dazu), sodass das Band mittig auf jedem Weg sitzt. Schraffurlinien tragen die
Linienart nicht — eine Wand in „Dämmung“ zeichnet ihre Kanten als Band, ihr
Muster als dünne Striche (`push_muster`).

Gespeichert wird sie wie jede Linienart (`style.line_type: "Insulation"`) plus
die zwei Werte `loop_width` (H, mm) und `loop_shape` (r); fehlen sie, gelten
100 mm und 0,5. Im Eigenschaften-Panel erscheinen „Breite“ und „Schlaufenform“
nur bei dieser Linienart.

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
- **Der Systemdialog ist modal:** solange er offen ist, sperrt `App::ui` die
  Tastenereignisse und eine deckende Fläche im Vordergrund (`dialogs::systemdialog_schleier`)
  nimmt alle Mausklicks an — die Zeichnung ist abgedunkelt, der Nutzer muss wählen
  oder abbrechen. Beide Sperren sind nötig: Knöpfe und Zeichenfläche lesen Klicks
  auch aus dem Zeigerzustand des Frames, nicht nur aus den Ereignissen. Die Kennung
  des eigenen Fensters (`systemdialog::Elternfenster`) geht an den Dialog, damit er
  als Fenster dieser Anwendung über dem Hauptfenster erscheint. `ESC` bleibt als
  Notausstieg, falls das Portal nicht antwortet.

- **Leistungsanzeige (T0068):** `crates/jjcad-gui/src/perf.rs` misst je Bild den
  Abstand zum vorigen Bild und die Rechenzeit, zerlegt in die Abschnitte der
  Anzeigekette: **Oberfläche** (Kopf, Ribbon, Reiter, Panels), **Displayliste**
  (Aufbau bzw. Kopie aus dem Zwischenspeicher), **Fangen** (Navigation, Fangen,
  Verweilfang), **Zeichnen** (Raster, Papier, Grafikelemente — nach Füllung,
  Linie und Text getrennt) und **Bedienung** (Vorschau, Griffe, Fadenkreuz,
  Eingabe); **Fenster** sind Dialoge, Menüs, Chat und die Anzeige selbst.
  Die Marken setzt `App::ui`/`canvas::show` über `Perf::abschnitt`; die Summe
  **muss** die Rechenzeit ergeben (`perf::tests::abschnitte_ergeben_die_rechenzeit`).
  Tessellierung und GPU sind von der Anwendung aus nicht messbar — sie erscheinen
  als „außerhalb“ (Abstand minus Rechenzeit). Perzentile laufen über einen Ring
  der letzten 240 Bilder; Pausen (Abstand über 600 ms) zählen nicht mit, weil in
  einer Pause nichts gerechnet wurde. Die Zerlegung je Grafikelement ist nur bei
  offener Anzeige aktiv (eine Uhr je Element).

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
