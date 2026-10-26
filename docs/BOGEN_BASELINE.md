# Bogenglätte — visuelle Baseline und Abnahmekriterien

Stand: 26.10.2026, `master` (= Build b100 im Bild), vor jeder Änderung an der
Bogenauflösung. Diese Baseline ändert **keine** Renderlogik; sie legt fest, wie
„kantig" gemessen wird und wie die Szenen reproduzierbar entstehen.

## Wozu

Die Bögen (Türbögen, Kreisbögen, Wandbögen) werden auf dem Bildschirm als
Polygonzug gezeichnet. Wie fein dieser Zug ist, hängt von einer Toleranz in
Bildschirmpixeln ab. Diese Baseline hält für typische Ansichten fest,

* wie stark der gezeichnete Zug vom idealen Kreisbogen abweicht (px),
* wie groß der Knick zwischen zwei Nachbsegmenten ist (Grad),
* wie viele Segmente und wie viel Rechenzeit dafür anfallen,
* und wie das im Bild aussieht (Vollbild + 4×-Ausschnitt).

Damit ist „glatter als vorher" nachprüfbar statt Geschmackssache.

## Szenen

Vier Zeichnungen unter `samples/bogen_baseline/`, jede nur aus Bögen. Die
Ansicht entsteht durch „Alles zeigen" (`fit_view`) im Fenster 1500×950 — kein
Klickweg nötig. Der Prozentwert in der Statusleiste gehört zum Beleg.

| Szene | Datei | Bögen (Radius, Öffnungswinkel) | Striche | Maßstab | Ansicht | Artefakte |
|---|---|---|---|---|---|---|
| S1 Detail | `S1_klein.jjcad` | 60 mm/90°, 150 mm/180°, 400 mm/90° | 0,35 / 0,50 / 0,70 mm | 1:10 | 95 % (0,953 px/mm) | `S1_klein.png`, `S1_klein_4x.png` |
| S2 Bau Maß | `S2_mittel.jjcad` | 885 mm/90° (Türbogen), 2000 mm/90°, 3000 mm/180° | 0,35 / 0,50 / 0,70 mm | 1:50 | 14 % (0,141 px/mm) | `S2_mittel.png`, `S2_mittel_4x.png` |
| S3 Plan | `S3_gross.jjcad` | 8000 mm/90°, 25000 mm/90°, 60000 mm/20° | 0,50 / 0,70 / 1,00 mm | 1:100 | 0,75 % (0,0075 px/mm) | `S3_gross.png`, `S3_gross_4x.png` |
| S4 Dick | `S4_starken.jjcad` | 400 mm/90° dreimal, 885 mm/90° | 0,13 / 0,50 / 1,00 / 2,00 mm | 1:20 | 38 % (0,3795 px/mm) | `S4_starken.png`, `S4_starken_4x.png` |

Abgedeckt sind damit: kleiner und großer Radius, vier Zoomstufen (0,75 % bis
95 %), dünne (0,13 mm) und dicke (2,00 mm) Striche, Viertel-, Halb- und
Dreiviertelbögen sowie ein flacher Bogen (20°). Der Modus „Stärke DPI" ist
aktiv, die Strichbreite skaliert also mit dem Zoom wie im Druck — so wie der
Nutzer zeichnet.

## Reproduzieren

```bash
./scripts/bogen-baseline.sh        # alles neu: Zeichnungen, Messwerte, Bilder
```

Einzelschritte, falls nur ein Teil gebraucht wird:

```bash
# Szenen-Zeichnungen (samples/bogen_baseline/) neu schreiben, Ansichten ausgeben
cargo test -p jjcad-core --test bogen_baseline -- --ignored --nocapture

# Messwerte (Tabelle auf stdout)
cargo test -p jjcad-core --test bogen_baseline -- --nocapture

# Ein Bild, ohne dass etwas auf dem Bildschirm des Nutzers erscheint
python3 scripts/qa-shot.py docs/bogen-baseline/S2_mittel.png \
    --file samples/bogen_baseline/S2_mittel.jjcad --bin target/release/jjcad
```

`scripts/qa-shot.py` startet ein eigenes Xvfb und beendet nur den eigenen
Prozess; die Instanz des Nutzers bleibt unberührt.

## Messwerte vor der Änderung

Vollständig in `docs/bogen-baseline/messwerte.md` (wird vom Skript neu
geschrieben). Kernzahlen bei Zeichenflächen-Toleranz 0,6 px:

| Kennzahl | Wert heute | Bereich über alle 14 Messfälle |
|---|---|---|
| Abstand Polygonzug ↔ idealer Bogen | 0,40 – 0,59 px | praktisch überall die Toleranz ausgeschöpft |
| Knick zwischen Nachbarsegmenten | 5,0° – 15,0° | großer Knick = sichtbare Ecke |
| Segmente je Bogen | 4 – 29 | — |
| Wohnhaus.jjcad bei 5 % | 23 Bögen, 56 Segmente | Tessellierung 0,004 ms, Displayliste 0,12 ms |

Der sichtbare Effekt ist in `S2_mittel_4x.png` (Knick am Scheitel) und
`S3_gross_4x.png` (Bogen zerfällt in gerade Teilstücke) direkt zu sehen.

## Wo die Auflösung entsteht

`geom::Arc::flatten(tol)` (`crates/jjcad-core/src/geom.rs:684`) erzeugt den
Polygonzug; `tol` ist der zugelassene Pfeilhöhenfehler. Die Aufrufer wählen ihn
in Pixeln und rechnen über den Zoom in Millimeter um:

| Weg | Toleranz | Stelle |
|---|---|---|
| Zeichenfläche | 0,6 px | `crates/jjcad-gui/src/canvas.rs:343` |
| Werkzeugvorschau Bogen | 0,5 px | `crates/jjcad-gui/src/canvas.rs:1237` |
| Vorschau/Ghost/Auswahl | 0,6 px | `crates/jjcad-gui/src/canvas.rs:2340` |
| PNG/Raster | 0,1 px | `crates/jjcad-core/src/raster.rs:288` |
| PDF | 0,08 mm Papier | `crates/jjcad-core/src/pdf.rs:291` |

Zwei harte Grenzen in `flatten` begrenzen die Glätte unabhängig von der
Toleranz (beide in der Messtabelle als Spalte „512er-Grenze" bzw. am Fall
„1500 mm / 500 %" sichtbar):

* **Mindestschritt 0,02 rad (1,15°)** — `max_step.max(0.02)`. Bei großem
  Bildschirmradius greift die Toleranz nicht mehr, der Knick bleibt bei 1,15°
  stehen (Fall 1500 mm bei 500 %: 0,37 px Fehler statt 0,1 px).
* **Höchstens 512 Segmente je Bogen** — `clamp(2, 512)`.

## Abnahmekriterien

Gemessen mit `cargo test -p jjcad-core --test bogen_baseline -- --nocapture`,
verglichen mit `docs/bogen-baseline/messwerte.md`. Alle Werte in
Bildschirmpunkten (bei 100 % Fensterskalierung = Pixel).

| Nr. | Kriterium | Soll | Heute |
|---|---|---|---|
| **K1** | Abstand Polygonzug ↔ idealer Bogen, alle 14 Messfälle, Zeichenflächentoleranz | **≤ 0,15 px** | 0,40 – 0,59 px → nicht erfüllt |
| **K2** | Knick zwischen zwei Nachbarsegmenten, alle 14 Messfälle | **≤ 3,0°** | 5,0 – 15,0° → nicht erfüllt |
| **K3** | Aufwand: Segmente je Bogen ≤ 512 (bestehende Obergrenze); im Wohnhaus bei 5 % ≤ 400 Segmente gesamt und ≤ 1,0 ms Tessellierung | **erfüllt** | 56 Segmente, 0,004 ms → erfüllt, Luft für die Glättung |
| **K4** | Sichtprüfung: in `S1…S4_4x.png` ist kein Knick und kein gerades Teilstück mehr zu erkennen; der PNG/Rasterweg wird nicht schlechter (≤ 0,1 px, Ausnahme 500 %-Fall wegen Mindestschritt) | **erfüllt** | Knicke sichtbar → nicht erfüllt |

Zusammengenommen legen K1 und K2 die Auflösung fest: 0,15 px Pfeilhöhe **und**
3,0° Knick. Der Knickwert ist der eigentliche Glätte-Maßstab (er erzeugt die
sichtbare Ecke); die Pfeilhöhe verhindert, dass ein Bogen bei kleinem Radius
abgeschnitten wirkt. Bei 3° Höchstschritt braucht ein Vollkreis höchstens 120
Segmente — deutlich weniger als die heutige Obergrenze von 512, K3 bleibt also
auch nach der Verfeinerung erreichbar.

## Grenzen dieser Baseline

* Die Zahlen gelten für das Fenster 1500×950 (so entstehen die Screenshots).
  Bei anderer Fenstergröße ändert `fit_view` den Zoom; die Kriterien gelten
  dann für die in der Tabelle genannten Zoomstufen entsprechend.
* Gemessen wird der Polygonzug aus `flatten` — also genau die Geometrie, die
  alle Ausgabewege zeichnen —, nicht das Antialiasing des Renderers. Ein
  Knick in der Messtabelle ist im Bild sichtbar, ein glatter Zug kann durch
  Kantenglättung trotzdem leicht unscharf wirken.
* Die 4×-Ausschnitte werden über die Zeichenflächenmitte (717,5; 507,5) im
  Fenster 1500×950 berechnet. Ändert sich das Panel-Layout, sind die
  Ausschnitte nachzuziehen (`scripts/bogen-baseline.sh`); die Vollbilder
  bleiben davon unabhängig gültig.
* Die Messtoleranzen 0,6 px (Zeichenfläche) und 0,1 px (Raster) sind in
  `crates/jjcad-core/tests/bogen_baseline.rs` als Konstanten wiederholt. Wer
  die Toleranz im Renderer ändert, ändert sie dort mit — der Diff zeigt das
  dann.
