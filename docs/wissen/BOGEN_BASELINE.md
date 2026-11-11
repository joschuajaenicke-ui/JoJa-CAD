# Bogenglätte — Baseline, Ist-Stand und Abnahmekriterien

Stand: 26.10.2026.

| Stand | Commit / Build | Artefakte |
|---|---|---|
| **Vorher** (Baseline) | `29b020b`, Build b100 | `docs/wissen/bogen-baseline/` |
| **Ist** (nach der Glättung) | `a26e2f0` (Glättung aus `deeaf72`), Build b108 | `docs/wissen/bogen-baseline/nachher/` |

Diese Baseline ändert **keine** Renderlogik. Sie legt fest, wie „kantig" gemessen
wird, wie die Szenen reproduzierbar entstehen und welche Werte als abgenommen
gelten.

## Wozu

Bögen (Türbögen, Kreisbögen, Wandbögen) werden für Bildschirm, PNG und PDF als
Polygonzug gezeichnet. Wie fein dieser Zug ist, hängt von einer zulässigen
Pfeilhöhe in Ausgabeeinheiten ab. Diese Baseline hält für typische Ansichten
fest,

* wie stark der gezeichnete Zug vom idealen Kreisbogen abweicht (px),
* wie groß der Knick zwischen zwei Nachbsegmenten ist (Grad),
* wie viele Segmente und wie viel Rechenzeit dafür anfallen,
* und wie das im Bild aussieht (Vollbild + 4×-Ausschnitt).

Damit ist „glatter als vorher" nachprüfbar statt Geschmackssache.

## Szenen

Vier Zeichnungen unter `samples/bogen_baseline/`, jede nur aus Bögen. Die
Ansicht entsteht durch „Alles zeigen" (`fit_view`) im Fenster 1500×950 — kein
Klickweg nötig. Der Prozentwert in der Statusleiste gehört zum Beleg.

| Szene | Datei | Bögen (Radius, Öffnungswinkel) | Striche | Maßstab | Ansicht |
|---|---|---|---|---|---|
| S1 Detail | `S1_klein.jjcad` | 60 mm/90°, 150 mm/180°, 400 mm/90° | 0,35 / 0,50 / 0,70 mm | 1:10 | 95 % (0,953 px/mm) |
| S2 Bau-Maß | `S2_mittel.jjcad` | 885 mm/90° (Türbogen), 2000 mm/90°, 3000 mm/180° | 0,35 / 0,50 / 0,70 mm | 1:50 | 14 % (0,141 px/mm) |
| S3 Plan | `S3_gross.jjcad` | 8000 mm/90°, 25000 mm/90°, 60000 mm/20° | 0,50 / 0,70 / 1,00 mm | 1:100 | 0,75 % (0,0075 px/mm) |
| S4 Dick | `S4_starken.jjcad` | 400 mm/90° dreimal, 885 mm/90° | 0,13 / 0,50 / 1,00 / 2,00 mm | 1:20 | 38 % (0,3795 px/mm) |

Je Szene gibt es `<Name>.png` (ganzes Fenster, Build-Kennung im Titel) und
`<Name>_4x.png` (4×-Ausschnitt der Bogenmitte, Point-Sampling — zeigt Knicke
ohne Weichzeichnung).

Abgedeckt sind: kleiner und großer Radius, vier Zoomstufen (0,75 % bis 95 %),
dünne (0,13 mm) und dicke (2,00 mm) Striche, Viertel-, Halb- und
Dreiviertelbögen sowie ein flacher Bogen (20°). Der Modus „Papier (maßstabsgetreu)“ ist
aktiv, die Strichbreite skaliert also mit dem Zoom wie im Druck — so wie der
Nutzer zeichnet.

## Reproduzieren

```bash
./scripts/bogen-baseline.sh                     # Stand neu erzeugen (docs/wissen/bogen-baseline)
./scripts/bogen-baseline.sh docs/wissen/bogen-baseline/nachher   # anderer Zielordner
```

Das Skript schreibt in vier Schritten: Szenen-Zeichnungen, Messtabelle,
Release-Build, Screenshots + 4×-Ausschnitte. Einzelschritte:

```bash
# Szenen-Zeichnungen (samples/bogen_baseline/) neu schreiben, Ansichten ausgeben
cargo test -p jjcad-core --test bogen_baseline -- --ignored --nocapture

# Messwerte (Tabellen auf stdout)
cargo test -p jjcad-core --test bogen_baseline -- --nocapture

# Ein Bild, ohne dass etwas auf dem Bildschirm des Nutzers erscheint
python3 scripts/qa-shot.py /tmp/S2.png \
    --file samples/bogen_baseline/S2_mittel.jjcad --bin target/release/jjcad
```

`scripts/qa-shot.py` startet ein eigenes Xvfb und beendet nur den eigenen
Prozess; die Instanz des Nutzers bleibt unberührt.

## Messwerte

Vollständig je Messfall in `docs/wissen/bogen-baseline/messwerte.md` (bzw.
`nachher/messwerte.md`, identischer Inhalt). 14 Messfälle, Zeichenfläche:

| Kennzahl | Vorher (0,6 px Unterteilung) | Ist (`CurveQuality::SCREEN`, 0,15 px) |
|---|---|---|
| Abstand Polygonzug ↔ idealer Bogen | 0,43 – 0,60 px | **0,128 – 0,150 px** |
| Knick zwischen Nachbsegmenten | 5,0° – 15,0° | 0,7° – 8,2° |
| Segmente je Bogen | 4 – 30 | 7 – 125 |

Aufwand im echten Plan (`Wohnhaus.jjcad`, Ansicht 5 %):

| Kennzahl | Vorher | Ist |
|---|---:|---:|
| Bögen | 23 | 23 |
| Segmente aller Bögen | 56 | 111 |
| Bögen tessellieren | 0,003 ms | 0,005 ms |
| Displayliste bauen (267 Primitive) | 0,119 ms | 0,119 ms |

Im Bild: `S2_mittel_4x.png` zeigt vorher einen deutlichen Knick am Scheitel,
nachher eine durchgehende Rundung; `S3_gross_4x.png` zeigt vorher eine Kette
gerader Teilstücke, nachher eine gekrümmte Linie.

## Wo die Auflösung entsteht

Die Unterteilung kommt aus `crates/jjcad-core/src/tessellate.rs`
(`CurveQuality`): `h = r · (1 − cos(Δφ/2))` → `Δφ = 2 · acos(1 − h/r)`, also
Segmentzahl aus Pfeilhöhe, Radius und Bogenwinkel. Zwei benannte Vorgaben an
einer Stelle:

* `CurveQuality::SCREEN` — 0,15 px Pfeilhöhe, 4…4096 Segmente, Mindestschritt
  2π/4096. Benutzt von Zeichenfläche (`canvas.rs`), Inventar und Vorschau.
  Seit T0172 ist die Pfeilhöhe der Zeichenfläche einstellbar (Einstellungen →
  Grafik → Kurventoleranz, 0,05…2,0 px, Vorgabe 0,15); `CurveQuality::SCREEN`
  bleibt die Vorgabe, und `CurveQuality::mit_pfeilhoehe(px)` bildet die
  eingestellte Güte mit denselben Grenzen. Die Zahlen dieser Baseline gelten
  für die Vorgabe.
* `CurveQuality::PRINT` — 0,05 Papier-mm, 5…2048 Segmente. Benutzt von PNG
  (`raster.rs`) und PDF (`pdf.rs`).

Die **volle** Ellipse folgt derselben Regel (T0174): `CurveQuality::ellipse_punkte`
zerlegt sie mit der Pfeilhöhe der jeweiligen Vorgabe — vorher zeichneten
Zeichenfläche, PNG und PDF sie mit **festen 72 Segmenten** (bei 30 px/mm 2,85 px
Abweichung, an den Scheiteln als Ecken sichtbar).

`geom::Arc::flatten(tol)` (Weltmaß) bleibt als allgemeiner Weg bestehen und
bildet auf dieselbe Rechnung ab (512 Segmente, 0,02 rad als Untergrenze).

Die alte Zeichenflächen-Unterteilung — 0,6 px Pfeilhöhe, Mindestschritt
0,02 rad, höchstens 512 Segmente — ist im Messtest als `ALT_SCREEN`
nachgebaut, damit die „Vorher"-Zahlen reproduzierbar bleiben.

## Abnahmekriterien

Gemessen mit `cargo test -p jjcad-core --test bogen_baseline -- --nocapture`,
verglichen mit `docs/bogen-baseline*/messwerte.md`. Einheiten sind
Bildschirmpunkte (bei 100 % Fensterskalierung = Pixel).

| Nr. | Kriterium | Soll | Vorher | Ist |
|---|---|---|---|---|
| **K1** | Abstand Polygonzug ↔ idealer Bogen, alle 14 Messfälle | ≤ 0,15 px | 0,43 – 0,60 px | **0,128 – 0,150 px — erfüllt** |
| **K2** | Sichtprüfung: in `S1…S4_4x.png` weder Knick noch gerades Teilstück | kein Knick sichtbar | Knicke sichtbar | **erfüllt** (S2 Scheitel, S3 Teilstücke) |
| **K3** | Aufwand: ≤ 4096 Segmente je Bogen; Wohnhaus bei 5 % ≤ 400 Segmente gesamt und ≤ 1,0 ms Tessellierung | erfüllt | 56 / 0,003 ms | **125 max. / 111 / 0,005 ms — erfüllt** |
| **K4** | Ausgabewege und Grenzfälle: PNG/PDF ≤ 0,05 Papier-mm; Vollkreis bleibt geschlossen; Radius 0 und Bildschirmradius < 0,4 px zeichnen nichts Kaputtes | erfüllt | erfüllt (0,1 px) | **erfüllt** (0,049 px, Vollkreis 58 Segmente) |

**Warum der Knickwinkel kein eigenes Kriterium ist:** sichtbar ist nicht der
Winkel, sondern der Abstand zur Idealkurve. Bei 0,15 px Pfeilhöhe bleibt der
Knick zwangsläufig bei 8° an kleinen Bildschirmradien (57 px) — die Kurve weicht
dort aber nur 0,15 px ab, also unterhalb der Kantenglättung. Umgekehrt kann ein
kleiner Knickwinkel bei riesigem Radius mehrere Pixel Abweichung bedeuten. Die
Knickwerte stehen deshalb als Kennzahl in der Messtabelle, nicht als Grenze.

## Grenzen dieser Baseline

* Die Zahlen gelten für das Fenster 1500×950 (so entstehen die Screenshots).
  Bei anderer Fenstergröße ändert `fit_view` den Zoom; die Szenen bleiben
  gleich, die Ansicht verschiebt sich.
* Gemessen wird der Polygonzug aus `CurveQuality` — genau die Geometrie, die
  alle Ausgabewege zeichnen —, nicht das Antialiasing des Renderers. Ein
  glatter Zug kann durch Kantenglättung trotzdem leicht unscharf wirken.
* Die 4×-Ausschnitte werden über die Zeichenflächenmitte (717,5; 507,5) im
  Fenster 1500×950 berechnet. Ändert sich das Panel-Layout, sind die
  Ausschnitte nachzuziehen (`scripts/bogen-baseline.sh`); die Vollbilder
  bleiben davon unabhängig gültig.
* Die Betriebssystem-Skalierung (egui `pixels_per_point`) skaliert die
  gemessenen Punkte mit; die Kriterien sind deshalb in Punkten angegeben.
