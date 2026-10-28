# Befehlsreferenz

Jede Funktion hat eine stabile **Action-ID**. Ribbon, Hotbar, Tastatur und
MCP-Server rufen dieselbe Aktion auf — es gibt keinen zweiten Weg an der
Befehls-Schicht vorbei.

**Konventionen**

- Koordinaten als `[x, y]` in der **Dokumenteinheit** (Standard Meter).
  Mit `"units": "mm"` (oder `"cm"`) im Parameterobjekt gilt die Angabe in dieser Einheit.
- Winkel in **Grad**.
- Längen wie `thickness`, `width`, `height` ebenfalls in der Dokumenteinheit.
- `layer` ist ein Layername (wird bei Bedarf angelegt) oder eine Layer-ID.
- Farbe: `"#RRGGBB"` oder `"bylayer"`.
- Rückgabe (MCP/JSON): `{"ok": true, "created": [ids], ...}` bzw. eine Fehlermeldung.

## Datei und Dokument

| ID | Parameter | Wirkung |
|---|---|---|
| `file.new` / `file.open` / `file.save` / `file.save_as` | — | Nur GUI (Dialoge) |
| `file.export_pdf` | — | Blatt-PDF über Dialog |
| `file.export_dxf` | — | DXF über Dialog |
| `doc.settings` | `input_unit`, `dim_unit`, `decimals`, `annotation_scale`, `grid_step`, `text_height`, `wall_thickness`, `door_width`, `window_width`, `stair_width`, `show_grid`, `snap{...}` | Einstellungen ändern |
| `doc.project` | `project`, `client`, `planner`, `plan_no`, `plan_title`, `address`, `note`, `date`, `revision` | Projektdaten / Schriftfeld |
| `dim.style` | `text_height`, `arrow_size`, `arrow` (`slash`/`arrow`/`dot`/`none`), `text_gap`, `ext_gap`, `ext_overshoot`, `decimals` | Bemaßungsstil |

## Zeichnen

| ID | Parameter | Beispiel |
|---|---|---|
| `draw.point` | `p` | `{"p":[1.2,3.4]}` |
| `draw.line` | `a`, `b` | `{"a":[0,0],"b":[5,0]}` |
| `draw.polyline` | `pts[]`, `closed` | `{"pts":[[0,0],[3,1],[5,0]],"closed":false}` |
| `draw.spline` | `pts[]`, `closed` | `{"pts":[[0,0],[3,1],[5,0]],"closed":false}` — Catmull-Rom durch die Punkte |
| `draw.rect` | `a`, `b`, optional `height` | `{"a":[0,0],"b":[3,2]}` — mit `height` ist `a`→`b` die Basislinie beliebiger Lage, `height` der senkrechte Abstand (Vorzeichen = Seite, positiv links der Richtung a→b): `{"a":[0,0],"b":[3,3],"height":1}` |
| `draw.circle` | `c`, `r` oder `through` | `{"c":[2,2],"r":0.75}` |
| `draw.arc` | `c`, `r`, `a0`, `a1` (Grad, CCW) | `{"c":[0,0],"r":2,"a0":0,"a1":90}` |
| `draw.ellipse` | `c`, `rx`, `ry`, `rot` (Grad, CCW) | `{"c":[0,0],"rx":1,"ry":0.5}`; Mittelachse + Breite = `c` = Achsenmitte, `rx` = halbe Achsenlänge, `ry` = halbe Breite, `rot` = Achsenwinkel |
| `draw.hatch` | `boundary[]`, `pattern` (`solid`/`lines`/`cross`/`dots`), `angle`, `spacing` | |
| `draw.text` | `p`, `text`, `height`, `angle`, `align`, `bold` | `{"p":[1,2],"text":"Wohnen","height":0.3}` |

## Architektur

| ID | Parameter | Hinweise |
|---|---|---|
| `draw.wall` | `a`, `b`, `thickness`, `height`, `material` | Achse mit Dicke; Ecken werden automatisch geschlossen |
| `draw.door` | `center`, `width`, `angle`, `kind`, `hinge`, `swing_in`, `host` | `kind`: `hinge` (Standard), `sliding`, `double`, `opening`; `hinge`: `1` rechts, `-1` links |
| `draw.window` | `center`, `width`, `sill`, `height`, `angle`, `host` | Öffnung wird aus der Wand geschnitten |
| `draw.stair` | `a`, `b`, `width`, `steps`, `direction` | `direction`: `up`, `down`, `both` |

Ohne `host` sucht `draw.door`/`draw.window` die nächste Wand im Umfeld und
übernimmt deren Richtung. Das ist derselbe Weg, den der GUI-Klick nutzt.

## Bemaßung

| ID | Parameter | Hinweise |
|---|---|---|
| `dim.linear` | `a`, `b`, `line_pos`, `axis`, `text` | `axis`: `x`, `y`, `aligned`; ohne Angabe automatisch nach Dominanz |
| `dim.chain` | `pts[]`, `line_pos`, `axis` | Eine Maßkette über mehrere Punkte |
| `dim.elevation` | `p`, `z`, `angle`, `text` | Höhenkote, `z` in mm über ±0,00 |

## Ändern

| ID | Parameter | Hinweise |
|---|---|---|
| `modify.move` | `ids[]`, `from`, `to` (oder `dx`,`dy`) | |
| `modify.copy` | wie `move`, `copy: true` | Kopieren mit Basispunkt; als Befehl mit `to` erzeugt es die Objekte sofort, in der Oberfläche füllt es nur die Zwischenablage |
| `modify.rotate` | `ids[]`, `center`, `angle` | `angle` in Grad, `copy` optional |
| `modify.mirror` | `ids[]`, `p1`, `p2`, `keep_original` | Achse durch zwei Punkte |
| `modify.scale` | `ids[]`, `center`, `factor` | Faktor > 0 |
| `modify.offset` | `ids[]`, `distance`, `side` | `side` = Punkt auf der Zielseite |
| `modify.trim` | `ids[]`, `at`, `cutters[]` | `at` = Punkt auf dem zu entfernenden Stück |
| `modify.extend` | `ids[]`, `at`, `cutters[]` | verlängert das nahe Ende bis zur nächsten Kante |
| `modify.erase` | `ids[]` | löschen |
| `modify.explode` | `ids[]` | Blöcke, Wände, Türen, Bemaßung → Grundelemente |
| `modify.set` | `ids[]`, `layer`, `color`, `linetype`, `lineweight`, `fill`, Typparameter | Eigenschaften ändern |

Typparameter für `modify.set`: `thickness`, `height`, `material` (Wand),
`width`, `kind`, `hinge`, `swing_in` (Tür), `width`, `sill` (Fenster),
`steps`, `direction` (Treppe), `text`, `angle`, `bold` (Text), `r` (Kreis),
`axis`, `text` (Maß), `z` (Höhenkote).

## Ebenen

| ID | Parameter |
|---|---|
| `layer.create` | `name`, `color`, `lineweight`, `linetype`, `visible`, `locked` |
| `layer.update` | `layer`, beliebige Eigenschaften |
| `layer.delete` | `layer` |
| `layer.set_current` | `layer` |
| `layer.list` | — |

## Blöcke

| ID | Parameter | Hinweise |
|---|---|---|
| `block.insert` | `block` (ID oder Name), `pos`, `rot`, `scale`, `scale_y`, `mirror` | |
| `block.create` | `name`, `ids[]`, `base`, `category` | Auswahl wird zur Definition, Instanz ersetzt sie |
| `block.update` | `block`, `ids[]`, `name` | Definition aus Auswahl ersetzen |
| `block.delete` | `block` | nur unbenutzte Blöcke |
| `block.list` | — | |

Systemblöcke: `WC`, `Waschbecken`, `Dusche`, `Badewanne`, `Herd`, `Spüle`,
`Kühlschrank`, `Bett 1600`, `Bett 900`, `Sofa`, `Tisch`, `Stuhl`, `Schrank`,
`Waschmaschine`, `Heizkörper`, `Nordpfeil`, `Schnittmarke`, `Raumstempel`,
`Platzhalter`.

## Blätter

| ID | Parameter |
|---|---|
| `sheet.new` | `paper` (`A4`…`A0`), `landscape`, `name`, `scale` |
| `sheet.update` | `sheet`, `name`, `paper`, `landscape`, `scale`, `frame`, `show_title_block` |
| `sheet.delete` | `sheet` |
| `sheet.add_viewport` | `sheet`, `x`, `y`, `w`, `h`, `scale`, `center`, `show_frame` |
| `sheet.update_viewport` | `sheet`, `viewport`, `x`, `y`, `w`, `h`, `scale`, `center`, `show_frame` |
| `sheet.remove_viewport` | `sheet`, `viewport` |
| `sheet.viewport_tool` | – (GUI-Werkzeug: Rechteck für ein neues Ansichtsfenster aufziehen) |

## Auswahl und Abfragen

| ID | Parameter | Hinweise |
|---|---|---|
| `select.all` | — | alle sichtbaren, nicht gesperrten Objekte |
| `select.query` | `layer`, `type`, `a`, `b` | Filter, liefert IDs |

## Beispiele

**Wohnung in einem Rutsch (MCP, Meter):**

```json
{"steps": [
  {"action":"draw.wall","params":{"a":[0,0],"b":[8,0],"thickness":0.3}},
  {"action":"draw.wall","params":{"a":[8,0],"b":[8,6],"thickness":0.3}},
  {"action":"draw.wall","params":{"a":[8,6],"b":[0,6],"thickness":0.3}},
  {"action":"draw.wall","params":{"a":[0,6],"b":[0,0],"thickness":0.3}},
  {"action":"draw.door","params":{"center":[2,0],"width":1.0}},
  {"action":"draw.window","params":{"center":[5,0],"width":1.6}},
  {"action":"dim.chain","params":{"pts":[[0,0],[4,0],[8,0]],"line_pos":[0,-1.2]}},
  {"action":"block.insert","params":{"block":"WC","pos":[1,5.6],"rot":270}}
]}
```

**Wand ändern und Tür verschieben:**

```json
{"action":"modify.set","params":{"ids":[12],"thickness":0.365}}
```

```json
{"action":"modify.move","params":{"ids":[18],"from":[0,0],"to":[0.35,0]}}
```

## Polylinie und Maßkette beenden

Beide Werkzeuge nehmen beliebig viele Punkte an. Abschließen mit:

- `Enter`
- Doppelklick
- Rechtsklick

`ESC` schließt ebenfalls ab — die bereits gesetzten Punkte bleiben als Objekt erhalten.
Nur wenn noch kein zweiter Punkt gesetzt wurde, wird das Werkzeug ohne Objekt verworfen.
Danach entfernt `Strg+Z` die ganze Polylinie in einem Schritt.
