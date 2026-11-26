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
  Fehlt er, legt jede Zeichenaktion ihr Objekt auf die **Standardebene ihres
  Typs** (siehe „Standardebenen“); ein ausdrücklich übergebenes `layer` gewinnt
  immer.
- Farbe: `"#RRGGBB"` oder `"bylayer"`.
- Rückgabe (MCP/JSON): `{"ok": true, "created": [ids], ...}` bzw. eine Fehlermeldung.

## Datei und Dokument

| ID | Parameter | Wirkung |
|---|---|---|
| `file.new` / `file.open` / `file.save` / `file.save_as` | — | Nur GUI (Dialoge) |
| `file.export_pdf` | — | Blatt-PDF über Dialog |
| `file.export_dxf` | — | DXF über Dialog |
| `file.restore` | — | Sicherungsordner im Dateimanager öffnen (die Stände sind `.jjcad`-Dateien und lassen sich direkt öffnen) |
| `doc.settings` | `input_unit`, `dim_unit`, `decimals`, `annotation_scale`, `grid_step`, `grid_kind` (`rechteckig`/`isometrisch`/`dimetrie`/`trimetrie`), `grid_mode` (`skaliert`/`fest`), `grid_force`, `text_height`, `wall_thickness`, `door_width`, `window_width`, `stair_width`, `show_grid`, `snap{...}` | Einstellungen ändern |
| `doc.project` | `project`, `client`, `planner`, `plan_no`, `plan_title`, `address`, `note`, `date`, `revision` | Projektdaten / Schriftfeld |
| `dim.style` | `text_height`, `arrow_size`, `arrow` (`slash`/`arrow`/`dot`/`none`), `text_gap`, `ext_gap`, `ext_overshoot`, `decimals` | Bemaßungsstil |

## Zeichnen

| ID | Parameter | Beispiel |
|---|---|---|
| `draw.point` | `p`, optional `symbol` | `{"p":[1.2,3.4]}` — `symbol` ist die Block-ID eines Punktsymbols (z. B. `sys:pkt-kreuz`) |
| `draw.line` | `a`, `b`, optional `start_symbol`/`end_symbol` | `{"a":[0,0],"b":[5,0]}` — Punktsymbole an Anfang und Ende (Block-IDs) |
| `draw.polyline` | `pts[]`, `closed` | `{"pts":[[0,0],[3,1],[5,0]],"closed":false}` |
| `draw.parallel` | `ids` (Linie oder Polylinie), `distance`, `side` | `{"ids":[7],"distance":0.3,"side":[2,1]}` — neue Linie/Polylinie im Abstand `distance`; `side` ist ein Punkt und bestimmt nur die **Seite**. Ebene, Farbe und Stärke erbt das neue Objekt von der Vorlage. Andere Objektarten sind ein Fehler |
| `draw.spline` | `pts[]`, `closed` | `{"pts":[[0,0],[3,1],[5,0]],"closed":false}` — Catmull-Rom durch die Punkte |
| `draw.rect` | `a`, `b`, optional `height` | `{"a":[0,0],"b":[3,2]}` — mit `height` ist `a`→`b` die Basislinie beliebiger Lage, `height` der senkrechte Abstand (Vorzeichen = Seite, positiv links der Richtung a→b): `{"a":[0,0],"b":[3,3],"height":1}` |
| `draw.circle` | `c`, `r` oder `through` | `{"c":[2,2],"r":0.75}` |
| `draw.arc` | `c`, `r`, `a0`, `a1` (Grad, CCW) | `{"c":[0,0],"r":2,"a0":0,"a1":90}` |
| `draw.ellipse` | `c`, `rx`, `ry`, `rot` (Grad, CCW) | `{"c":[0,0],"rx":1,"ry":0.5}`; Mittelachse + Breite = `c` = Achsenmitte, `rx` = halbe Achsenlänge, `ry` = halbe Breite, `rot` = Achsenwinkel |
| `draw.hatch` | `boundary[]`, `pattern`, `angle`, `spacing` | Muster nach DIN 1356-1:2026-09, Tabelle 5 — Schlüssel: `solid`, `mauerwerk` (45°-Linien), `kunststoffe` (Kreuz), `sand` (Punkte), `boden_unveraendert`, `boden_veraendert`, `kies`, `beton_unbewehrt`, `beton_bewehrt`, `wu_beton`, `textilbeton`, `vollholz_quer`, `vollholz_laengs`, `metall`, `putz`, `daemmung` (parallele Schlaufenbänder, Abstand = Dämmstärke), `dichtstoffe`, `glas`; die alten Schlüssel `lines`/`cross`/`dots` gelten weiter. **Abdichtung ist kein Muster** (T0132), sondern eine Linienart (`linetype: "insulation"`); die Schlüssel `daemmstoffe`, `daemmung_schlaufen`, `abdichtung_*` liefern nichts mehr |
| `draw.text` | `p`, `text`, `height`, `angle`, `align`, `valign`, `font`, `line_spacing`, `bold` | `{"p":[1,2],"text":"Wohnen","height":0.3,"font":"din_mittel","line_spacing":1.5}` |
| `draw.image` | `a`, `b`, `file` oder `data` (Base64), `name`, optional `height`, `lock_ratio` | `{"a":[0,0],"b":[4,0],"file":"Lageplan.png"}` — PNG oder JPEG; `a`→`b` ist die **untere Kante** (Lage, Drehung, Breite), die Höhe folgt aus dem Seitenverhältnis (`lock_ratio`, Vorgabe `true`), sonst aus `height`. Die Datei wird eingebettet |

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
| `dim.linear` | `a`, `b`, `line_pos`, `axis`, `angle`, `text` | Einzelmaß = Maßkette aus zwei Messpunkten; `angle` (Grad) ist die Richtung der Maßlinie, `axis` (`x`, `y`, `aligned`) die bequeme Angabe dafür, ohne beide automatisch nach Dominanz |
| `dim.chain` | `pts[]`, `line_pos`, `axis`, `angle` | Maßkette über mindestens zwei Messpunkte (dasselbe Element wie `dim.linear`); `angle` (Grad) setzt die Orientierung frei, die Punkte werden entlang der Maßlinie sortiert abgelegt |
| `dim.chain.h` / `.v` / `.d` | `pts[]`, `line_pos` | Werkzeuge der Oberfläche: Maßkette waagerecht, senkrecht oder direkt — die Achse kommt aus dem Werkzeug, ein mitgegebenes `axis` sticht sie nicht |
| `dim.point.add` | `ids`, `p` (oder `pos`), optional `index` | Fügt einer Maßkette einen Messpunkt hinzu; ohne `index` an der Stelle, die die Kette entlang der Messrichtung geordnet hält |
| `dim.point.remove` | `ids`, `index` oder `p` | Entfernt einen Messpunkt; unter zwei Punkten ist das ein Fehler |
| `dim.height` | — | Werkzeug der Oberfläche: Maßkette bzw. Abschnitt anklicken, dann die Höhenangabe tippen (Esc/Enter übernimmt) |
| `dim.height.set` | `ids`, `index`, `text` | Schreibt die Höhenangabe eines Abschnitts (`index` = Abschnitt zwischen `pts[i]` und `pts[i+1]`); leerer Text nimmt sie zurück, ein Index außerhalb der Kette ist ein Fehler |
| `dim.elevation.plan` | `p`, `z`, `text_pos` | Grundriss-Kote (Kreis-Symbol), `z` in mm über ±0,00; `text_pos` = `top` (Vorgabe) / `bottom` / `left` / `right` — Lage des Höhenwertes am Symbol |
| `dim.elevation.section` | `pts[]` | Schnitt-Kote; der erste Punkt ist ±0,00, alle weiteren Höhen rechnet der Renderer aus der Y-Differenz |

## Ändern

| ID | Parameter | Hinweise |
|---|---|---|
| `modify.move` | `ids[]`, `from`, `to` (oder `dx`,`dy`) | |
| `modify.copy` | wie `move`, `copy: true` | Kopieren mit Basispunkt; als Befehl mit `to` erzeugt es die Objekte sofort, in der Oberfläche füllt es nur die Zwischenablage |
| `modify.rotate` | `ids[]`, `center`, `angle` | `angle` in Grad, `copy` optional |
| `modify.mirror` | `ids[]`, `p1`, `p2`, `keep_original` | Achse durch zwei Punkte |
| `modify.scale` | `ids[]`, `center`, `factor` | Faktor > 0 |
| `modify.offset` | `ids[]`, `distance`, `side` | `side` = Punkt auf der Zielseite |
| `modify.trim` | `ids[]`, `at`, `cutters[]` | `at` = Punkt auf dem zu entfernenden Stück; Linie, Wand, Kreis (→ Bogen), Bogen, Ellipse (→ Ellipsenbogen), Ellipsenbogen, Polylinie. Schnittkanten sind Geraden **und** Bögen/Kreise |
| `modify.extend` | `ids[]`, `at`, `cutters[]` | verlängert das nahe Ende bis zur nächsten Kante; Linie, Wand, offene Polylinie (gerades Endsegment läuft in seiner Richtung, ein Endsegment mit Stützbogen auf seinem Kreis weiter). Eine geschlossene Polylinie hat kein Ende und liefert einen Fehler |
| `modify.intersect` | `ids[]` (zwei Linien oder Wände) | zieht beide Objekte auf ihren gemeinsamen Schnittpunkt — verlängert oder kürzt das jeweils nähere Ende |
| `modify.area.union` | `ids[]` (zwei oder mehr Füllflächen) | vereinigt **angrenzende** Flächen zu einer; Muster, Rand, Stil und Ebene kommen von der ersten. Getrennte Flächen sind ein Fehler |
| `modify.area.split` | `pts[]`, `ids[]` (optional) | teilt eine Füllfläche entlang der Punktfolge in zwei; ohne `ids` bestimmt die Linie die Fläche. Kreuzt sie den Rand mehrfach oder eine Insel, ist das ein Fehler |
| `modify.erase` | `ids[]` | löschen |
| `modify.explode` | `ids[]` | Blöcke, Wände, Türen, Bemaßung, Schraffuren → Grundelemente. Eine Schraffur zerfällt in ihre Musterlinien, Musterpunkte werden zu kleinen gefüllten Kreisen in der Musterfarbe; das Bild bleibt gleich |
| `modify.set` | `ids[]`, `layer`, `color`, `linetype`, `lineweight`, `fill`, Typparameter | Eigenschaften ändern |

`linetype` kennt `solid`, `dashed`, `dotted`, `dashdot`, `hidden` und
`insulation` (Dämmung). Die Dämmung bringt zwei eigene Parameter mit:
`loop_width` (Breite H, Länge in der Eingabeeinheit) und `loop_shape`
(Schlaufenform r, Verhältnis Periodenlänge/Höhe). Beide gelten beim Zeichnen
und in `modify.set`; ohne Angabe 100 mm und 0,5.

Typparameter für `modify.set`: `thickness`, `height`, `material` (Wand),
`width`, `kind`, `hinge`, `swing_in` (Tür), `width`, `sill` (Fenster),
`steps`, `direction` (Treppe), `text`, `angle`, `bold` (Text), `r` (Kreis),
`axis`, `angle`, `text` (Maß), `z`, `text_pos` (Grundriss-Kote), `pts`
(Schnitt-Kote).

## Standardebenen

Ohne `layer` zeichnet jede Aktion auf die Ebene ihres Typs. Die Ebene wird bei
Bedarf angelegt — ein Plan aus einem Batch hat damit seine Ebenenstruktur, ohne
dass jeder Schritt `layer` mitgeben muss.

Seit T0202 gibt es nur noch die vier Layer eines Plans: **Zeichnung**,
**Architektur**, **Beschriftung** und **Blöcke**. Die Ebene wird im Plan des
aktuellen Layers angelegt.

| Aktion | Standardebene |
|---|---|
| `draw.wall`, `draw.door`, `draw.window`, `draw.stair`, `draw.stair.label` | Architektur |
| `draw.volume*` | Architektur (Körper im Wandton) |
| `dim.linear`, `dim.chain*`, `dim.height*`, `dim.elevation.*`, `dim.room`, `draw.text` | Beschriftung |
| `block.insert` | Blöcke |
| `draw.fill*`, `draw.hatch`, `draw.image` | Zeichnung |
| `draw.point`, `draw.line`, `draw.polyline`, `draw.spline`, `draw.rect`, `draw.circle`, `draw.arc`, `draw.ellipse` | Zeichnung |

In der Oberfläche legt jedes Werkzeug sein Objekt ebenfalls auf die Ebene seines
Typs. Eine **eigene** Ebene (nicht eine der vier) sticht das: wer sie im
Layer-Panel wählt, zeichnet bewusst dorthin.

## Pläne und Ebenen

Ein **Plan** ist die oberste Stufe des Layer-Managers; Layer gehören zu einem
Plan. Ein Layername ist je Plan eindeutig.

| ID | Parameter |
|---|---|
| `plan.create` | `name` (Vorgabe „Plan N“), `scale` (Maßstab 1:N, Vorgabe 100) — legt den Plan mit den vier Standardlayern an |
| `plan.update` | `plan`, `name`, `visible`, `locked`, `collapsed`, `scale` |
| `plan.list` | — (Pläne mit ihren Layern) |
| `layer.create` | `name`, `plan`, `color`, `lineweight`, `linetype`, `visible`, `locked` |
| `layer.update` | `layer`, beliebige Eigenschaften, `plan` (verschieben) |
| `layer.delete` | `layer` |
| `layer.set_current` | `layer` |
| `layer.list` | — (je Layer auch `plan`, `sichtbar`, `gesperrt`) |

## Blöcke

| ID | Parameter | Hinweise |
|---|---|---|
| `block.insert` | `block` (ID oder Name), `pos`, `rot`, `scale`, `scale_y`, `mirror` | |
| `block.create` | `name`, `ids[]`, `base`, `category` | Auswahl wird zur Definition, Instanz ersetzt sie |
| `block.update` | `block`, `ids[]`, `name` | Definition aus Auswahl ersetzen |
| `block.delete` | `block`, `explode` | ohne `explode` nur unbenutzte Blöcke; mit `explode` werden vorher **alle** Verwendungen zerlegt (Modell, Blätter, andere Blöcke) |
| `block.copy` | `block` (ID oder Name), `name`, `dir` | Kopie als Benutzerblock (auch von Systemblöcken); `dir` schreibt die `.jjblk` |
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
| `sheet.add_viewport` | `sheet`, `x`, `y`, `w`, `h` (oder `rect_mm`), `scale`, `center` oder `center_mm`, `show_frame` |
| `sheet.update_viewport` | `sheet`, `viewport`, `x`, `y`, `w`, `h` (oder `rect_mm`), `scale`, `center` oder `center_mm`, `show_frame` |
| `sheet.fit_viewport` | `sheet`, `viewport` (Vorgabe: erstes Fenster), `margin_mm` (Vorgabe 10), `scale` |
| `sheet.remove_viewport` | `sheet`, `viewport` |
| `sheet.viewport_tool` | – (GUI-Werkzeug: Rechteck für ein neues Ansichtsfenster aufziehen) |

Ansichtsfenster mischen zwei Räume: `x`/`y`/`w`/`h` bzw. `rect_mm` sind
**Papier-Millimeter**, `center` ist eine **Modellkoordinate in der Einheit
`units`** (Vorgabe: Dokumenteinheit), `center_mm` dieselbe Lage in Millimetern.
`sheet.fit_viewport` setzt die Mitte auf die Zeichnungs-BBox und den Maßstab aus
der Fenstergröße (siehe `docs/technik/MCP.md`).

Ein Ansichtsfenster ist seit T0109 ein **normales Blattobjekt**
(`EntityKind::Viewport`): `modify.move`, `modify.erase` und `modify.set`
(`x`, `y`, `w`, `h`, `scale`, `center`, `show_frame`, `name`) arbeiten darauf wie
auf jeder Linie. Die Aktionen `sheet.*_viewport` bleiben für MCP und ältere
Aufrufer bestehen und ändern dasselbe Objekt.

## Auswahl und Abfragen

| ID | Parameter | Hinweise |
|---|---|---|
| `select.all` | — | alle sichtbaren, nicht gesperrten Objekte |
| `select.query` | `layer`, `type`, `a`, `b` | Filter, liefert IDs |
| `select.area` | `ids`, `layer`, `type`, `a`, `b` | Fläche je Objekt (`areas[].area_m2`) und Summe (`sum_m2`) in m², deutsches Komma; offene Objekte stehen in `without_area` |

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
Besteht der Zug aus nur **einem Element** (zwei Punkte), entsteht eine **Linie**
statt einer Polylinie — ein Segment ist kein Polygonzug.

Danach entfernt `Strg+Z` die ganze Polylinie in einem Schritt.