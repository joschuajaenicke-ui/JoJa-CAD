# Das Projektformat `.jjcad`

Eine JoJa-CAD-Zeichnung ist eine **UTF-8-JSON-Datei** — lesbar, diffbar und ohne
Lock-in. Es gibt keine Binärdaten und keine eingebetteten Archive.

- **Interne Einheit: Millimeter.** Alle Koordinaten, Dicken und Höhen stehen als
  mm in der Datei, unabhängig davon, in welcher Einheit die Oberfläche rechnet.
- **Winkel in Bogenmaß** (Radiant), Nullrichtung +X, Drehsinn mathematisch positiv.
- **Farben** als `"#RRGGBB"`.
- **IDs** sind projektweit eindeutige Ganzzahlen, fortlaufend aus `next_id`.

## Kopfbereich

```json
{
  "format": "jjcad",
  "version": 2,
  "name": "Wohnhaus",
  "id": "6f1c2a4e-8b3d-4c7a-9e10-2d5b8f0a1c33",
  "project": {
    "project": "Wohnhaus Musterstraße 12",
    "client": "Bauherr Max Mustermann",
    "planner": "Planungsbüro JoJa",
    "plan_no": "A-01",
    "plan_title": "Grundriss Erdgeschoss",
    "address": "12345 Musterstadt",
    "note": "",
    "date": "",
    "revision": "0"
  },
  "settings": { "...": "siehe unten" },
  "layers": [ "...": "siehe unten" ],
  "current_layer": 1,
  "blocks": [ "...": "siehe unten" ],
  "entities": [ "...": "siehe unten" ],
  "sheets": [ "...": "siehe unten" ],
  "dim_style": { "...": "siehe unten" },
  "next_id": 87
}
```

Die Felder `project` und `settings` sind optional; fehlende Werte werden mit
Standardwerten ergänzt. `modified` und `path` werden nicht gespeichert.

`id` ist die **Kennung des Dokuments** (UUID, T0129). Die Sicherung folgt damit
der Datei, nicht dem Pfad — auch eine noch ungespeicherte Zeichnung hat eine.
Das Feld ist optional: ältere Dateien bekommen beim Laden eine Kennung und
schreiben sie beim nächsten Speichern mit. `version` bleibt deshalb 2.

## `sicherung` — Block einer Standdatei

Stände im Sicherungsordner sind gewöhnliche `.jjcad`-Dateien mit **einem
zusätzlichen ersten Feld**. Es beschreibt den Stand, nicht die Zeichnung:

```json
{
  "sicherung": {
    "format": "jjcad-sicherung-stand",
    "zyklus": 42,
    "datei": "6f1c2a4e-8b3d-4c7a-9e10-2d5b8f0a1c33",
    "zeit": "2026-11-18T14:35:44",
    "gesichert": false,
    "verworfen": false,
    "hash": "9f2c…",
    "datei_hash": "1a77…",
    "groesse": 60123,
    "pfad": "/home/joschua/Zeichnungen/Wohnhaus.jjcad"
  },
  "format": "jjcad",
  "version": 2,
  "name": "Wohnhaus",
  …
}
```

| Feld | Bedeutung |
| --- | --- |
| `format` | Immer `jjcad-sicherung-stand`; unterscheidet den Block von einem Zufallstreffer |
| `zyklus` | Zyklusnummer des Standes; einziger Zeitanker der Aufbewahrung |
| `datei` | Dokumentkennung (UUID aus dem Dateikopf). Leer = Datei gehört zu keiner Zeichnung („nicht zugeordnet“) |
| `zeit` | Zeitstempel, nur zur Anzeige |
| `gesichert` | Der Stand entspricht dem Inhalt der Zieldatei |
| `verworfen` | Der Benutzer hat diese Arbeit verworfen |
| `hash` | SHA-256 des Dokuments **ohne** diesen Block; der Block zählt nie mit, sonst wäre ein Stand nie gleich seiner Zieldatei |
| `datei_hash` | SHA-256 der Zieldatei beim Anlegen des Standes (leer, wenn es keine gab) |
| `groesse` | Größe der Standdatei in Bytes |
| `pfad` | Zieldatei zum Zeitpunkt der Sicherung (leer, wenn nie gespeichert) |

Regeln:

- Der Block steht **an erster Stelle**, damit die Sicherung ihn aus den ersten
  2 KB lesen kann, ohne den Stand zu laden.
- **Beim Laden wird er verworfen** (`sicherung::ohne_kopf`); eine gespeicherte
  Zieldatei trägt ihn nie. `persist::from_json` ignoriert ihn zusätzlich, weil
  `Document` das Feld nicht kennt.
- Es gibt **kein Manifest** mehr — der Verzeichnisinhalt ist die Wahrheit. Der
  frühere `manifest.json` wird beim ersten Start einmal eingelesen und danach in
  `manifest.json.eingelesen` umbenannt.

## `settings`

| Feld | Typ | Standard | Bedeutung |
|---|---|---|---|
| `input_unit` | `"Mm" \| "Cm" \| "M"` | `"M"` | Einheit für Eingaben (auch MCP-Koordinaten) |
| `dim_unit` | `"Mm" \| "Cm" \| "M"` | `"M"` | Einheit der Maßzahlen |
| `decimals` | Zahl | `2` | Nachkommastellen der Maßzahlen |
| `annotation_scale` | Zahl | — | **Altlast** (T0232): lag einmal hier. Beim Laden verteilt ihn `normalize_plaene` auf die Pläne; geschrieben wird er nicht mehr |
| `text_height` | mm | `250` | Standard-Texthöhe |
| `wall_thickness`, `door_width`, `window_width`, `stair_width` | mm | 240 / 885 / 1200 / 1000 | Werkzeugvorgaben |
| `show_grid`, `show_rulers` | bool | `true` | Anzeige |
| `snap` | Objekt | siehe unten | Fangoptionen |

`snap`: `endpoint`, `midpoint`, `center`, `intersection`, `perpendicular`,
`tangent`, `nearest`, `grid`, `ortho`, `polar`, `wall_edges`, `quadrant`,
`bisector` (bool), `grid_step` (mm, Standard 100), `tolerance_px` (Standard 12),
`polar_step` und `polar_tolerance` (Grad, Standard 45 / 6).

Erweiterter Punktfang (T0160): `hilfslinien` (bool, Standard `true`) schaltet
das Halten und die Hilfslinien ab, `verweil_ms` (Standard 400, 100…2000) ist die
Verweildauer bis zum Halten, `halte_max` (Standard 2, 0…4) die Höchstzahl
gehaltener Punkte.

Raster (T0144): `grid_kind` (`rechteckig` | `isometrisch` | `dimetrie` |
`trimetrie`, Standard `rechteckig`) bestimmt die Achsen von Anzeige **und**
Fang, `grid_mode` (`skaliert` | `fest`, Standard `skaliert`) ob die gezeichnete
Weite mit dem Zoom ausgedünnt wird, `grid_force` (bool, Standard `false`) ob
jede Eingabe ohne Fangradius im Raster liegt. Fehlende Felder gelten als
Vorgabe — alte Dateien laden unverändert.

## `plans`

```json
{ "id": 1, "name": "Plan 1", "visible": true, "locked": false, "collapsed": false,
  "scale": 100.0 }
```

Ein Plan ist die oberste Gliederungsebene des Layer-Managers (T0202); jeder
Layer gehört zu genau einem Plan (`Layer::plan`). `scale` ist der **Maßstab
dieses Plans** (1:N, T0232): An ihm hängen alle Papiergrößen seiner Objekte
(Strichstärke im Modus „Papier (maßstabsgetreu)“, Text- und Bemaßungshöhen,
Punktsymbole). Fehlt das Feld, gilt 1:100.

## `layers`

```json
{ "id": 1, "name": "Wände", "color": "#23262C", "line_weight": 0.5,
  "line_type": "Solid", "visible": true, "locked": false, "printable": true }
```

`line_type`: `Solid`, `Dashed`, `Dotted`, `DashDot`, `Hidden`.
`line_weight` ist die **Plotstärke in mm** (0,05–3,0).

## `entities`

Jedes Objekt hat `id`, `kind`, `layer` und optional `style`:

```json
{
  "id": 42,
  "layer": 1,
  "style": { "color": null, "line_type": null, "line_weight": null, "fill": null },
  "kind": { "type": "wall", "a": {"x": 0.0, "y": 0.0}, "b": {"x": 5000.0, "y": 0.0},
            "thickness": 300.0, "height": 2750.0, "material": "Mauerwerk" }
}
```

`null` in `style` bedeutet **ByLayer** (vom Layer erben). `by_block: true`
übernimmt die Eigenschaften der Blockinstanz.

### Objekttypen (`kind.type`)

| `type` | Felder | Hinweise |
|---|---|---|
| `point` | `p` | Punkt |
| `line` | `a`, `b` | Strecke |
| `circle` | `c`, `r` | Kreis |
| `arc` | `c`, `r`, `a0`, `a1` | Bogen gegen den Uhrzeigersinn |
| `ellipse` | `c`, `rx`, `ry`, `rot` | gedrehte Ellipse |
| `ellipse_arc` | `c`, `rx`, `ry`, `rot`, `a0`, `a1` | Abschnitt einer Ellipse (entsteht beim Stutzen); `a0`/`a1` sind **Parameterwinkel** im ungedrehten Ellipsenraum, der Bogen läuft gegen den Uhrzeigersinn |
| `polyline` | `verts[{p,bulge}]`, `closed` | `bulge = tan(θ/4)`, 0 = Gerade |
| `spline` | `verts[{p,bulge}]`, `closed` | Catmull-Rom-Kurve durch `verts`, zentripetal; `bulge` ungenutzt |
| `hatch` | `boundary[]`, `pattern`, `angle`, `spacing`, `filled` | Muster: `Solid`, `Lines`, `Cross`, `Dots` |
| `text` | `p`, `text`, `height`, `angle`, `align`, `valign`, `bold` | `{DATUM}` usw. werden ersetzt |
| `dim` | `pts[]`, `line_pos`, `angle`, `override_text` | Maßkette; `angle` = Richtung der Maßlinie in **Bogenmaß** (0 = waagerecht, π/2 = senkrecht). Ältere Dateien tragen `axis` (`X`, `Y`, `Aligned`) — das wird beim Laden in den Winkel umgerechnet |
| `dim_chain` | `pts[]`, `line_pos`, `axis` | Maßkette in älteren Dateien (wird beim Laden zu `dim` mit `angle`) |
| `elevation_plan` | `p`, `z`, `text_pos` | Grundriss-Kote: Kreis-Symbol; `z` in mm über ±0,00, `text_pos` = Lage des Höhenwertes am Symbol (`top` Vorgabe, `bottom`, `left`, `right`) |
| `elevation_section` | `pts[]` | Schnitt-Kote: Dreieck und Höhenangabe je Punkt; der **erste** Punkt ist ±0,00, alle weiteren Höhen folgen aus der Y-Differenz zu ihm (Modell-mm) |
| `wall` | `a`, `b`, `thickness`, `height`, `material` | Achse + Dicke; Öffnungen kommen aus Türen/Fenstern mit `host` |
| `door` | `center`, `angle`, `width`, `kind`, `hinge`, `swing_in`, `host` | `kind`: `Hinge`, `Sliding`, `Double`, `Opening` |
| `window` | `center`, `angle`, `width`, `sill`, `height`, `host` | |
| `stair` | `a`, `b`, `width`, `steps`, `height`, `auto_steps`, `cut`, `unten`, `oben` | `a`/`b` = Lauflinie (Mitte des Rechtecks), Breite symmetrisch; `height` = Treppenhöhe; `auto_steps` = Stufenzahl aus 2 S + A = 63 cm statt `steps`; `cut` = Treppenschnitt zeichnen (zwei Läufe übereinander; die Grenze zwischen unterem und oberem Teil läuft schräg mit ihm); `unten`/`oben` = eigene Linien der Treppenteile (`color`, `line_type`, `line_weight`), fehlt = Objektstil |
| `block_ref` | `block`, `pos`, `rot`, `scale`, `mirror` | verweist auf `blocks[].id` |
| `image` | `a`, `b`, `height`, `lock_ratio`, `name`, `data` | PNG oder JPEG, **eingebettet** als Base64 in `data` (die Zeichnung bleibt ohne die Bilddatei gültig). `a`→`b` ist die **untere Kante**: sie bestimmt Lage, Drehung und Breite; die Höhe steht senkrecht darauf (nach links). `lock_ratio` (Vorgabe `true`) hält die Höhe am Seitenverhältnis der Datei — dann ist `height` nur der zuletzt gerechnete Wert; bei `false` gilt `height`. Dateien aus der Zeit vor der Kante (zwei gegenüberliegende Ecken) werden beim Laden umgestellt: untere Kante aus der Box, Höhe als gespeicherter Wert, `lock_ratio: false`. Zeichenrang wie eine Fläche (hinten) |

**Wände verhalten sich wie Linien mit Breite:** `a`/`b` beschreiben die Achse; die
Darstellung (zwei parallele Kanten, Füllung, Laibungen) wird gerendert, nicht
gespeichert. Dadurch funktionieren Verschieben, Drehen, Stutzen und Verlängern
genau wie bei Linien.

**Türen und Fenster** sind eigenständige Objekte mit optionalem `host` (Wand-ID).
Fehlt `host`, sucht das Programm beim Zeichnen die nächste Wand. Der Wand-Renderer
schneidet die Öffnung aus der Wand und zeichnet die Laibungen.

## `blocks`

```json
{
  "id": "sys:wc",
  "name": "WC",
  "category": "Sanitär",
  "source": "System",
  "description": "Wand-WC, 380 × 600 mm",
  "entities": [ { "id": 1, "layer": 0, "by_block": true, "kind": { "...": "..." } } ],
  "icon": null,
  "base": { "x": 0.0, "y": 0.0 }
}
```

- `source`: `System` (mitgeliefert, schreibgeschützt), `Project` (in der Datei),
  `User` (Benutzerbibliothek im Profil, siehe unten).
- IDs sind namensraum-präfixiert (`sys:`, `prj:`, `dxf:`), damit beim Kopieren
  zwischen Dateien keine Kollisionen entstehen. Beim Import werden kollidierende
  IDs automatisch umbenannt.
- Blockinhalte liegen im **Blockkoordinatensystem**; die Instanz transformiert sie
  (Position, Rotation, Skalierung, Spiegelung).
- `groesse` (T0231) beantwortet die eine Frage, die ein Block beantworten muss:
  **feste Größe oder Papiergröße?** `"Fest"` = die Geometrie steht in
  Zeichnungs-Millimetern und wird maßstäblich (1:1) eingefügt — ein Bett bleibt 2 m
  lang. `"Papier"` = die Geometrie steht in **Papier-Millimetern, so groß wie
  gedruckt**, und wächst mit dem Zeichnungsmaßstab — eine Grundriss-Kote bleibt auf
  1:50, 1:100 und 1:200 gleich groß. Die Punktsymbole der Systembibliothek
  (`sys:pkt-*`) sind `"Papier"`; fehlt das Feld (ältere Dateien), gilt `"Fest"`
  (`"Millimeter"`, das frühere `einheit` aus Turn 2, und das Häkchen
  `annotative: true` schreibt `persist::migrate_block_groessen` beim Laden um).

### Eigene Blöcke: `<blocks>/<slug>.jjblk`

Ein Block des Benutzers liegt in **einer** eigenen Datei in der Datenwurzel
(`~/.local/share/JoJa-CAD/blocks`). Das Verzeichnis ist die Wahrheit — es gibt
keinen Index. Der Dateiname folgt dem Namen (Kleinbuchstaben, Umlaute
aufgelöst, z. B. `eigene-tuer.jjblk`); zwei Blöcke gleichen Namens bekommen
`…-2.jjblk`. Die Kennung `usr:…` steht im Inhalt und bleibt beim Umbenennen
gleich — umbenannt wird die Datei plus das Feld `name`.

```json
{ "format": "jjcad-block", "version": 1, "block": { "…": "BlockDef wie oben" } }
```

Eine unlesbare Datei wird beim Start gemeldet und übersprungen; sie reißt die
übrige Bibliothek nicht mit. Geschrieben wird sie von „Block aus Auswahl“ (Band
Blöcke), gelöscht beim Löschen im Blockmanager.

## `sheets`

```json
{
  "id": 60,
  "name": "Blatt 1",
  "paper": "A3",
  "landscape": true,
  "scale": 50.0,
  "frame": true,
  "show_title_block": true,
  "entities": [
    { "id": 61, "kind": { "type": "viewport", "id": 61, "name": "Ansicht",
        "x": 15.0, "y": 83.0, "w": 395.0, "h": 204.0,
        "center": { "x": 5250.0, "y": 4100.0 }, "scale": 50.0,
        "show_frame": false, "locked": false, "layers": null } },
    "...": "Schriftfeld im Papierraum (mm)"
  ]
}
```

- `entities` im Blatt sind normale Zeichenobjekte im Papierraum — das Schriftfeld
  ist damit frei gestaltbar und kann ersetzt werden.
- Ein **Ansichtsfenster** ist seit T0109 ebenfalls ein Objekt
  (`"type": "viewport"`). `x/y/w/h` sind **Papierkoordinaten in mm**, Ursprung
  unten links; `center` ist die Weltkoordinate in der Mitte des Fensters,
  `scale` der Fenstermaßstab (1:`scale`).
- Das frühere Feld `sheets[].viewports` wird nur noch **gelesen**: beim Laden
  wandert jedes Fenster als Objekt in `entities`
  (`persist::normalize_sheet_viewports`). Geschrieben wird es nicht mehr.

## `dim_style`

Alle Werte in **Papier-Millimetern**; der Renderer multipliziert sie mit dem
Zeichnungsmaßstab.

Einige Elemente der Bemaßung sind nach DIN 1356-1:2026-09, Bild 3/Bild 4
**Anteile der Texthöhe** und wachsen deshalb mit `text_height`
(`render::DIM_MARKE_H` = 1,2 h Begrenzungsstrich, `DIM_UEBERSTAND_H` = 1,2 h
Überstand der Maßlinie über die Maßhilfslinie, `DIM_TEXT_ABSTAND_H` = 0,6 h
Abstand der Maßzahl über der Maßlinie, `DIM_EXT_GAP_H` = 0,8 h Abstand am
Maßpunkt). Bei 2,5 mm Texthöhe: 3,0 mm Strich und Überstand, 1,5 mm
Textabstand, 2,0 mm Abstand am Maßpunkt. Die Maßhilfslinie wird **am
Maßpunkt** gekürzt (Abstand), nicht an der Maßlinie; dort gilt der Wert
`ext_overshoot` aus der Datei.

Die **Maßzahl** folgt DIN 1356-1, Tabelle 4, Zeile 2 („m und cm“): auf 5 mm
gerundet, unter 1 m in Zentimetern, der halbe Zentimeter als **Hochzahl** —
`11⁵` (11,5 cm), `1,12⁵` (1,125 m), `4,25`. Mit `dim_unit` = `cm` gilt Zeile 1
(`88,5`), mit `mm` Zeile 3 (`3 885`, Tausendertrennung als Leerzeichen).
Die Zahlen werden in DIN 1451 Mittelschrift gesetzt (§8.4).

| Feld | Standard | Bedeutung |
|---|---|---|
| `text_height` | 2.5 | Texthöhe |
| `arrow_size` | 2.0 | Größe von Pfeil und Punkt (der Schrägstrich folgt der Texthöhe) |
| `arrow` | `"Slash"` | `Slash` (DIN 1356), `Arrow`, `Dot`, `None` |
| `text_gap` | 0.8 | **Mindest**abstand Maßlinie → Maßzahl; unter 0,6 h greift der Normwert |
| `ext_gap` | 1.5 | Abstand Maßpunkt → Hilfslinie; unter 0,8 h gilt der Normwert |
| `ext_overshoot` | 1.5 | Überstand der Hilfslinie über die Maßlinie |
| `chain_gap` | 8.0 | Abstand zwischen Maßketten |
| `decimals` | 2 | ohne Wirkung auf die Maßzahl (Tabelle 4 gibt die Stellen vor) |

## Dynamische Textfelder

In `text`, `override_text` und im Schriftfeld:

| Platzhalter | Ersetzung |
|---|---|
| `{DATUM}` / `{DATE}` | aktuelles Datum (TT.MM.JJJJ) |
| `{ISO}` | aktuelles Datum (JJJJ-MM-TT) |
| `{PROJEKT}`, `{BAUHERR}`, `{PLANER}`, `{ADRESSE}` | Projektdaten |
| `{PLAN}`, `{PLANTITEL}`, `{REVISION}` | Plandaten |
| `{MASSTAB}` / `{MASSSTAB}` | Blattmaßstab (im Papierraum) bzw. Bezugsmaßstab |
| `{DATEI}` | Dokumentname |

## Kompatibilität

- Neue Felder werden mit Standardwerten ergänzt; unbekannte Felder werden ignoriert.
- Eine Datei mit höherer `version` wird mit Hinweis abgelehnt.
- Fehlende Layer werden beim Laden repariert, `next_id` wird nachgezogen.
