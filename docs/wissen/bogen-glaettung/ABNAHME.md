# Bogenglättung — Verifikation und Abnahme

Karte `t_aea1b20d` (Verifikation zu `t_e70e2b0a`). Geprüfter Stand: **`b3a3865`**
(Merge von `wt/t_cde0fdde`), Build **b102**, ausgeliefert als `s25-b3a3865`.
Verglichen mit dem Stand davor: **`29b020b`**, Build **b100**.

Alle Zahlen dieses Berichts sind mit `cargo test`/Messprogrammen erzeugt
(nachprüfbar, siehe „Reproduzieren“), nicht geschätzt.

**Messbasis:** Alle Messungen und Screenshots entstanden auf `b3a3865` (Build
b102). Der Bericht selbst wurde mit **s27-9c3009c** ausgeliefert, also nach dem
Merge der Ribbon-Karte `t_92b03440`; die Zeichenflächen-Geometrie ist davon
nicht betroffen, die Bogenrechnung läuft auf demselben Code (`cargo test` auf
`master` grün). Einzige Folge: `fit_view` kann durch die neue Reiterhöhe eine
andere Zoomstufe wählen, die Pixelvergleiche gelten deshalb für die genannten
Zoomstufen, nicht für beliebige Fensterhöhen.

---

## 1. Abnahme-Checkliste

Kriterien aus `docs/wissen/BOGEN_BASELINE.md` (Baseline-Karte `t_ea35d723`), gemessen
über den Weg, den die Anwendung wirklich geht (`tessellate::CurveQuality`).

| Nr. | Kriterium | Soll | Vorher | Nachher | Ergebnis |
|---|---|---|---|---|---|
| **K1** | Abstand Polygonzug ↔ idealer Bogen, 14 Messfälle, Zeichenfläche | ≤ 0,15 px | 0,40–0,60 px | **0,128–0,150 px** | **erfüllt** |
| **K2** | Knick zwischen Nachbarsegmenten, 14 Messfälle | ≤ 3,0° | 5,0–15,0° | **0,72–8,18°** | **nicht erfüllt** (11 von 14 Fällen über 3°) |
| **K3** | Segmente je Bogen ≤ 512; Wohnhaus ≤ 400 Segmente, ≤ 1,0 ms | erfüllt | 56 Segmente | **111 Segmente, 0,005 ms** | **erfüllt** |
| **K4** | Sichtprüfung: kein Knick/gerades Teilstück in `S1…S4_4x.png`; Rasterweg ≤ 0,1 px | erfüllt | Knicke sichtbar | siehe unten | **überwiegend erfüllt** (Einschränkung bei kleinen Radien) |

### K1 — erfüllt

Pfeilhöhe liegt in allen 14 Fällen bei 0,128–0,150 px (Soll ≤ 0,15 px), vorher
0,40–0,60 px. Das ist unter einem halben Pixel und damit unter der
Kantenglättung: der Zug deckt dieselben Bildpunkte wie der ideale Bogen.
Messung: `crates/jjcad-core/tests/bogen_verifikation.rs::k1_pfeilhoehe_und_k3_segmentzahl`.

### K2 — nicht erfüllt (bewusst, mit Begründung)

Die Baseline fordert ≤ 3,0° Knick je Fuge. Der umgesetzte Stand begrenzt die
**Pfeilhöhe** (0,15 px), nicht den **Knickwinkel**. Dadurch bleibt der Knick bei
kleinen Bildschirmradien grob: 8,18° bei 57 px Radius, 7,50° bei 60 px,
5,29° bei 125 px; erst ab etwa 300 px Radius wird 3° erreicht. Mittelpunkt der
Kritik ist die Sichtbarkeit — und die hängt an der Pfeilhöhe: 8 °Knick bei
0,15 px Abweichung und 8 px Sehnenlänge ist im Bild nicht als Ecke zu erkennen,
15 ° bei 0,43 px (alter Stand) schon.

**Wichtig:** K2 und K3 sind zusammen nicht erfüllbar. Ein Knickdeckel von 3°
verlangt im Wohnhaus (23 Bögen, 5 %) **690** Segmente statt 111 — K3 erlaubt
400. Die Analyse (`t_de39937a`) hatte 3° empfohlen, ohne diese Kollision zu
bemerken; die Umsetzung hat sich für die Pfeilhöhe entschieden und damit K3
gehalten. Wer K2 wörtlich will, muss K3 anheben (690 Segmente ≈ 0,02 ms,
leistungsmäßig unkritisch).

Nachweis des offenen Punkts als Test:
`cargo test -p jjcad-core --test bogen_verifikation -- --ignored k2_ --nocapture`
(listet alle 14 Fälle mit Knick und schlägt fehl; im normalen `cargo test`
übersprungen, damit die Suite grün bleibt).

### K3 — erfüllt

Höchstwert 125 Segmente je Bogen (Fall 1500 mm bei 500 %, Soll ≤ 512).
Wohnhaus bei 5 %: 23 Bögen, **111** Segmente (vorher 56), Unterteilung
**0,005 ms** je Bild (vorher 0,004 ms), Soll ≤ 400 Segmente / ≤ 1,0 ms.

### K4 — überwiegend erfüllt

* Raster/PNG-Weg: Pfeilhöhe 0,0462–0,0481 Papier-mm (Soll ≤ 0,05), vorher
  0,0714–0,0764 mm.
* Sichtprüfung: In `bilder/S1_klein_4x.png`, `S2_mittel_4x.png`,
  `S4_starken_4x.png` (jeweils links alt, rechts neu) ist der Bogen rechts
  durchgehend rund; die Knicke des alten Stands sind weg. Bei kleinem
  Bildschirmradius (< ~100 px) bleibt der Verlauf geringfügig eckiger als der
  Kreis daneben — das ist der in K2 beschriebene, bewusst offene Punkt.
* Grenze der Prüfung: `S3_gross_4x.png` des Baselinesatzes trifft nur ~150
  Rasterpunkte Tinte (der 20°-Bogen liegt am Bildrand des Ausschnitts) und ist
  als Sichtbeleg unbrauchbar; für S3 gilt der Vollbildvergleich.

---

## 2. Vergleich mit den Baseline-Artefakten

14 Messfälle der Baseline, Zeichenfläche. „Vorher“ aus
`docs/wissen/bogen-baseline/messwerte.md`, „Nachher“ aus `messwerte_nachher.md`.

| Fall | Bildschirmradius | Segmente vorher → nachher | Pfeilhöhe vorher → nachher | Knick vorher → nachher |
|---|---:|---|---|---|
| S1   60 mm /  90° / 95 % | 57,2 px | 6 → 11 | 0,489 → 0,146 px | 15,00° → 8,18° |
| S1  150 mm / 180° / 95 % | 142,9 px | 18 → 35 | 0,544 → 0,144 px | 10,00° → 5,14° |
| S1  400 mm /  90° / 95 % | 381,2 px | 14 → 28 | 0,600 → 0,150 px | 6,43° → 3,21° |
| S2  885 mm /  90° / 14 % | 124,8 px | 9 → 17 | 0,475 → 0,133 px | 10,00° → 5,29° |
| S2 2000 mm /  90° / 14 % | 282,0 px | 13 → 25 | 0,514 → 0,139 px | 6,92° → 3,60° |
| S2 3000 mm / 180° / 14 % | 423,0 px | 30 → 59 | 0,580 → 0,150 px | 6,00° → 3,05° |
| S3  8000 mm /  90° / 0,75 % | 60,0 px | 6 → 12 | 0,513 → 0,128 px | 15,00° → 7,50° |
| S3 25000 mm /  90° / 0,75 % | 187,5 px | 10 → 20 | 0,578 → 0,145 px | 9,00° → 4,50° |
| S3 60000 mm /  20° / 0,75 % | 450,0 px | 4 → 7 | 0,428 → 0,140 px | 5,00° → 2,86° |
| S4  400 mm /  90° / 38 % | 151,8 px | 9 → 18 | 0,578 → 0,144 px | 10,00° → 5,00° |
| S4  885 mm /  90° / 38 % | 335,9 px | 14 → 27 | 0,528 → 0,142 px | 6,43° → 3,33° |
| G  Vollkreis 1000 mm / 10 % | 100,0 px | 29 → 58 | 0,586 → 0,147 px | 12,41° → 6,21° |
| G  1500 mm /  90° / 500 % | 7500,0 px | 63 → 125 | 0,583 → 0,148 px | 1,43° → 0,72° |
| G  50 mm /  90° / 400 % | 200,0 px | 11 → 21 | 0,510 → 0,140 px | 8,18° → 4,29° |

Der Fehler liegt jetzt in **jedem** Fall an der Vorgabe (0,15 px) statt bei
0,4–0,6 px; die Segmentzahl ist rund verdoppelt. Vorher wuchs der Fehler mit dem
Radius (0,43 → 0,59 px), jetzt ist er über acht Zoomstufen konstant — genau die
Eigenschaft, die eine Pfeilhöhen-Vorgabe in Ausgabeeinheiten liefert.

---

## 3. Regressionsprüfung

### Bedienelemente und unbeteiligte Geometrie — pixelgleich

Vergleich der Screenshots alter/neuer Stand, gleiche Datei, gleiches Fenster
(1500×950), Pixelmaß mit `compare -metric AE`:

| Bereich | Bogenprobe | Wohnhaus | S1…S4 |
|---|---|---|---|
| Menüband (1500×140) | 0 px | 0 px | 0 px |
| linkes Panel (225×700) | 0 px | 0 px | 0 px |
| rechtes Panel (290×700) | 0 px | 0 px | 0 px |
| Statusleiste links (240×30) | 0 px | 0 px | 0 px |
| Kreis, gleicher Radius (Ausschnitt) | 0 px | 0 px | — |
| Linie, frei von Bögen (200×30) | 0 px | 0 px | — |

Die Statusleiste rechts unterscheidet sich nur in der Build-Kennung
(`b100 - 29b020b` gegen `b102 - b3a3865`). Layout, Panels, Raster, Maßketten,
Texte und Nicht-Bogen-Geometrie sind unverändert.

### Strichstärke, Enden, Auswahl

* Strichbreite im Raster: 4 px in beiden Ständen (Querschnitt durch den großen
  Bogen bei y = 700) — Kapseloptik bleibt, Enden weiter rund (zwei Kreise mit
  Radius w/2 ersetzen die Endkappen der Kapselkette).
* Auswahl/Hover nutzen denselben `paint_prim`-Zweig mit `Palette::SELECT` und
  der verdickten Breite (`base_w.max(1.6) + 0.6`); Farbe und Verdickung greifen
  unverändert, weil nur die Punktliste ausgetauscht wurde.
* Sehr kleine Bögen (r × Zoom < 0,4 px) werden als Punkt gezeichnet — neu, aber
  deckungsgleich mit dem, was vorher als Kapsel mit Länge < 0,01 px entstand.

### Alle Verbraucher der Bogenauflösung

| Stelle | Weg | Stand nach der Änderung |
|---|---|---|
| `canvas.rs:348` Zeichenfläche | Bildschirm | `CurveQuality::SCREEN` — neu |
| `canvas.rs:1254` Werkzeugvorschau | Bildschirm | `CurveQuality::SCREEN` — neu |
| `canvas.rs:2357` Einfüge-Vorschau/Ghost | Bildschirm | `CurveQuality::SCREEN` — neu |
| `inventory.rs:313` Inventar-Vorschau | Bildschirm | `CurveQuality::SCREEN` — neu |
| `pdf.rs:287` PDF | Druck | `CurveQuality::PRINT` (0,05 mm) — neu |
| `raster.rs:290` PNG | Druck | `CurveQuality::PRINT` (0,05 mm) — neu |
| `boundary.rs:550` Füllflächen/Farbgrenzen | Welt | unverändert `Arc::flatten(0,2 px)` |
| `snap.rs:364` Objektfang auf Bögen | Welt | unverändert `flatten(1,0 mm)` |
| `geom.rs` Polylinien/Bulge, Fläche | Welt | unverändert |
| `dxf/export.rs` DXF | exakt | unverändert (echte `ARC`-Entität) |

Kein Ausgabeweg ist schlechter geworden: PDF/PNG sind feiner (0,05 statt
0,08 Papier-mm bzw. 0,05 statt 0,1 px), Vorschauen und Inventar ziehen mit der
Zeichenfläche gleich. Füllflächen und Fang arbeiten auf Weltgeometrie und waren
von der Änderung nicht betroffen.

### Exporte

| Prüfung | Ergebnis |
|---|---|
| PNG-Export Wohnhaus (2000×1400) | 595 von 2 800 000 Pixeln abweichend — ausschließlich auf Bögen |
| PNG-Export Bogenprobe | 1097 von 2 800 000 Pixeln abweichend — ausschließlich auf Bögen |
| PDF-Export, 600 dpi gerastert | beide Dateien gültig (`pdftoppm`), Unterschied nur im Bogenbereich (941×2940 px von 7016×4961) |
| PDF-Operatoren | Linienoperatoren 561 → 627 (Wohnhaus), Bézieroperatoren unverändert 48 (Kreise) |

**Grenze:** Bögen bleiben im PDF ein Polygonzug (Kreise laufen dort schon als
kubische Bézier). Bei 600 dpi entspricht die Druckvorgabe 0,05 mm ≈ 1,2 px
Pfeilhöhe — im Druck bei starker Vergrößerung also weiter messbar eckig.
Echte Bézierbögen im PDF (Empfehlung der Analyse, Option b1, ~30 Zeilen in
`pdf.rs`) sind **nicht** umgesetzt.

---

## 4. Renderzeit vor/nach

Gemessen in Release, `crates/jjcad-gui/examples/mess_zeichnen.rs` (Epaint-
Tessellierung) und `crates/jjcad-core/examples/mess_bogen.rs` (Unterteilung).

| Fall | vorher | nachher | Änderung |
|---|---:|---:|---:|
| Bogen r = 1000 px, dünn (1 px): Formen / Scheitel / Zeit | 22 / 132 / 2,42 µs | 3 / 171 / 2,11 µs | **−13 %** |
| Bogen r = 1000 px, breit (3 px) | 22 / 792 / 13,43 µs | 3 / 216 / 2,22 µs | **−83 %** |
| Bogen r = 2400 px, dünn | 36 / 216 / 4,01 µs | 3 / 252 / 2,92 µs | **−27 %** |
| Bogen r = 2400 px, breit | 36 / 1296 / 21,55 µs | 3 / 324 / 3,16 µs | **−85 %** |
| Wohnhaus-Plan, 23 Bögen, dünn | 299 / 1794 / 42,6 µs | 69 / 2553 / 52,5 µs | **+23 %** |
| Unterteilung Wohnhaus (23 Bögen, Zoom 0,06) | 2,34 µs | 3,51 µs | +50 % |
| Lastfall 200 Bögen bis 5000 px, Zoom 8 | 126,6 µs | 239,3 µs | +89 % |

Lesart: **Zeichnen** wird deutlich billiger (ein Pfad statt einer Form je Sehne,
bei dicken Strichen Faktor 6), **Unterteilen** teurer (rund doppelt so viele
Punkte). Im echten Plan ergibt das +10 µs je Bild — 0,06 % eines 16,7-ms-Bildes,
nicht messbar. Der teuerste geprüfte Fall (200 große Bögen bei 8-facher
Vergrößerung) liegt bei 0,24 ms, also 1,4 % eines Bildes. Kein Engpass.

---

## 5. Gewählter Ansatz, geänderte Dateien, Vorgaben

**Ansatz:** Pfeilhöhe (Sagitta) in Ausgabeeinheiten als einzige Steuergröße,
zentral in `tessellate::CurveQuality`; Segmentzahl folgt aus Radius und
Bogenwinkel. Auf dem Bildschirm wird der Bogen als **ein** Pfad gezeichnet
(runde Enden als zwei Kreise), nicht mehr als Kapsel je Sehne — das entfernt die
doppelt geblendeten Stöße an den Fugen.

| Datei | Änderung |
|---|---|
| `crates/jjcad-core/src/tessellate.rs` (neu) | `CurveQuality` mit `SCREEN`/`PRINT`, `segments[_tol]`, `flatten[_tol]`, 9 Unit-Tests |
| `crates/jjcad-core/src/geom.rs` | `Arc::flatten` delegiert an `CurveQuality` (Rückwärtskompatibilität für Welt-Verbraucher) |
| `crates/jjcad-core/src/lib.rs` | Modul `tessellate` |
| `crates/jjcad-core/src/pdf.rs` | Bogen nutzt `PRINT` statt lokaler Toleranz |
| `crates/jjcad-core/src/raster.rs` | dito (PNG) |
| `crates/jjcad-gui/src/canvas.rs` | Zeichenfläche, Werkzeugvorschau, Ghost auf `SCREEN`; Bogen als ein Pfad + zwei Endkreise |
| `crates/jjcad-gui/src/inventory.rs` | Inventar-Vorschau auf `SCREEN` |
| `crates/jjcad-core/tests/bogen_glaettung.rs` (neu) | 4 Tests des Umsetzers |
| `samples/Bogenprobe.jjcad` (neu) | Prüfdatei (3 Bögen, 1 Kreis, 1 Linie) |

**Neue Vorgaben (Default):**

| Ausgabe | Pfeilhöhe | Segmentzahl | Datei |
|---|---|---|---|
| Bildschirm | 0,15 px | 4 … 4096, Mindestschritt 2π/4096 | `CurveQuality::SCREEN` |
| Druck (PDF/PNG) | 0,05 Papier-mm | 5 … 2048, Mindestschritt 2π/2048 | `CurveQuality::PRINT` |

---

## 6. Bekannte Grenzen und Folgearbeiten

1. **K2 offen** (Knick ≤ 3°): siehe Abschnitt 1 — Kollision mit K3. Entscheidung
   des Nutzers nötig: Kriterium fallen lassen oder K3 auf ~700 Segmente anheben.
2. **Ellipsen sind jetzt der gröbere Teil** — sie haben weiter feste 72 Punkte
   (5°, Pfeilhöhe 0,10 px bei r = 100 px, aber **2,28 px bei r = 2400 px**,
   das 15-fache des Bogens). Vorher fiel das nicht auf, weil Bögen genauso grob
   waren. Eigene Karte vorgeschlagen.
3. **PDF zeichnet Bögen als Polygonzug** (kein Bézier), bei 600 dpi ≈ 1,2 px
   Pfeilhöhe. Analyse-Empfehlung b1 ist offen.
4. **Bögen ignorieren den Linientyp** (gestrichelt erscheint durchgezogen, im
   PDF korrekt) — bestand vorher, unverändert, in `t_cde0fdde` als bekannte
   Lücke dokumentiert.
5. **Füllflächen entlang von Bögen** (`boundary.rs`) nutzen weiter den alten
   Polygonzug mit 0,2 px — feiner als die neue Bildschirmvorgabe, also
   unkritisch, aber ein zweiter Weg.
6. Die Kriterien gelten für das Fenster 1500×950 (dort entstehen die Bilder).

---

## 7. Baseline-Stand

Die Baseline aus `t_ea35d723` bleibt als **Vorher-Referenz** unverändert
(Szenen, Zeichnungen, `S*_4x.png`, `messwerte.md`). Der neue Stand liegt
daneben:

* `messwerte_nachher.md` — dieselben 14 Fälle, gemessen über den neuen Weg
  (ersetzt die Spalte „Heute“ in `docs/wissen/bogen-baseline/messwerte.md`),
* `bilder/` — Vergleichsausschnitte (links alt, rechts neu) und die
  Wohnhaus-Vollbilder beider Stände.

Damit ist die Baseline auf den neuen Stand fortgeschrieben, ohne die
Vorher-Artefakte zu überschreiben.

---

## 8. Reproduzieren


```bash
# Abnahmekriterien und Messwerte (14 Baseline-Fälle, Druckweg, Wohnhaus-Aufwand)
cargo test -p jjcad-core --test bogen_verifikation -- --nocapture --test-threads=1

# offener Punkt K2
cargo test -p jjcad-core --test bogen_verifikation -- --ignored k2_ --nocapture

# Rohmessungen (Pfeilhöhe/Segmente/Grenzfälle/Zeit, Zeichenkosten, Exporte)
cargo run --release -p jjcad-core --example mess_bogen
cargo run --release -p jjcad-gui  --example mess_zeichnen
cargo run --release -p jjcad-core --example mess_export -- samples/Wohnhaus.jjcad /tmp/pruef

# Szenenbilder beider Stände
python3 scripts/qa-shot.py /tmp/S2.png --file samples/bogen_baseline/S2_mittel.jjcad
```

Die Szenenbilder der Baseline liegen in `docs/wissen/bogen-baseline/` (Karte
`t_ea35d723`); die Vergleichsbilder dieses Berichts in `bilder/` (links alter
Stand, rechts neuer Stand).
