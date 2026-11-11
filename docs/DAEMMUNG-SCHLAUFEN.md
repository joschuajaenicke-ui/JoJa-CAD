# Dämmungs-Schlaufenkurve

> **Im Programm:** die Linienart **Dämmung** (`LineType::Insulation`) und seit
> T0136 das Schraffurmuster **Dämmung** (`FillPattern::InsulationHatch`). Die
> Rechnung steht in `crates/jjcad-core/src/daemmung.rs`, gezeichnet wird das
> Band in der Displayliste (`render::DisplayList::push`) — Bildschirm, PDF,
> PNG und DXF zeigen dieselbe Geometrie. Die beiden Parameter heißen in der
> Oberfläche **Breite** (`H`, mm) und **Schlaufenform** (`r`).

Parametrische Kurve zur vektoriellen Darstellung von Dämmung als Schlaufenband. Die Kurve beginnt bei x = 0 und endet exakt bei x = L. Sie bleibt dabei vollständig im Bereich 0 ≤ x ≤ L.

## Parameter

| Parameter | Bedeutung | Hinweis |
|-----------|-----------|---------|
| `H` | Dämmstärke (Gesamthöhe der Kurve) | Skaliert die Form gleichmäßig |
| `L` | Länge, x-Wert, bei dem die Kurve endet | Exakt, unabhängig von `H` und `r` |
| `r` | Sollverhältnis Periodenlänge / Höhe (`p / H`) | Sinnvoll: 0,2 bis 0,8, optimal 0,5 |

Abgeleitete Größen:

| Größe | Formel | Bedeutung |
|-------|--------|-----------|
| `n` | `max(1/2, round(2L/(r·H)) / 2)` | Anzahl Perioden, in Halbschritten |
| `p` | `L / n` | Tatsächliche Periodenlänge |

Da `n` in Halbschritten springt, weicht `p/H` je nach Länge etwas von `r` ab. Bei wenigen Perioden sind es einige Prozent, bei vielen kaum noch.

## Kurve

$$x(t) = \frac{L}{2\pi n}\,\bigl(t + K\,\sin 2t\bigr), \qquad y(t) = \frac{H}{2}\cos t, \qquad t \in [0,\ 2\pi n]$$

mit der Konstanten

$$K = 2{,}3016694$$

## GeoGebra

Schieberegler `H`, `r`, `L` anlegen, dann:

```
n = max(1/2, round(2 L/(r H))/2)
Kurve(L/(2π n) (t + 2.3016694 sin(2t)), H/2 cos(t), t, 0, 2π n)
```

## Konstante K

`K = |s| / 2` mit `s = -4,6033388`. Dieser Wert ist so gewählt, dass sich benachbarte Schlaufen gerade berühren. Er löst

$$\sqrt{s^2-1} - \arccos\!\left(\frac{1}{|s|}\right) = \pi$$

Die Konstante ist unabhängig von `H`, `L` und `r`.

Sie muss mit allen Stellen verwendet werden. Bei einem gröber gerundeten Wert (z. B. 2,3017) ragt die Kurve minimal über 0 bzw. `L` hinaus.

## Eigenschaften

- Ende bei `t = 2πn`: Dort ist `sin 2t = 0`, also `x = L` exakt.
- Die Minima der Schlaufen liegen bei x ≈ 0 und die Maxima bei x ≈ L. Die Kurve ragt nicht über die Grenzen hinaus.
- Start bei y = +H/2. Das Ende liegt oben oder unten, je nachdem, ob `n` ganzzahlig ist oder nicht.
- `H` skaliert nur die Höhe. Die Schlaufenform ändert sich nur über `r` (und minimal über die Rundung von `n`).

## Umsetzung im Programm

- **Breite** = `H` (Dämmstärke in mm), **Schlaufenform** = `r`. Beide stehen im
  Eigenschaften-Panel, sobald die Linienart „Dämmung“ gewählt ist, und in der
  Datei als `loop_width` / `loop_shape` (fehlen sie, gelten 100 mm und 0,5).
- Die Kurve läuft auf **jeder** Linie: Gerade, Polylinie, Bogen, Kreis und
  Ellipse werden über die **Bogenlänge** abgefahren — `x` ist der Weg längs der
  Linie, `y` der Versatz quer dazu. Damit sitzt das Band mittig auf der Linie,
  und die Länge stimmt auf jedem Weg, nicht nur auf der Geraden.
- Gezeichnet wird ein Polygonzug; die Stützpunkte folgen der Kurve. **Wie fein**,
  hängt an der Anzeige (T0176): ab 12 px je Periode nach Pfeilhöhe
  (`N = gerade(ceil(sqrt(52 · ppx)))`, höchstens 512), darunter feste Stufen mit
  10, 6 und 2 Punkten, unter 1,5 px eine Gerade in Bandmitte mit der Bandhöhe
  als Strichbreite. Immer gerade Punktzahl, Knoten auf den Periodengrenzen.
  Ohne Abbildung (Druck, PNG, DXF) bleibt es weltfest bei 12…40 Punkten je
  Periode (höchstens 6000 je Linie). Weil das Band die Anzeigegeometrie ist,
  folgen Trefferfläche, Fang und DXF-Export mit.
- Eine Wand in „Dämmung“ zeichnet ihre **Kanten** als Band; ihr Muster (die
  Wandschraffur) ist davon getrennt und wird — wie jedes Muster — als dünne
  Striche gezeichnet.

## Schraffur Dämmung (T0136)

Die Fläche, die eine Dämmung kennzeichnet, ist **eine Schraffur aus parallelen
Schlaufenbändern**: dieselbe Kurve, mehrfach quer zum Musterwinkel versetzt.

Der Unterschied zur Linienart ist die **Schlaufengröße**. Die Linienart passt
die Periodenlänge an die Länge der Linie an (`n` aus `L`, siehe oben) — die
Form wird dafür um einige Prozent gestaucht oder gestreckt, damit eine runde
Anzahl Schlaufen aufgeht. In einer Schraffur liegen viele Bänder nebeneinander,
und jedes endet an einer anderen Kante: mit der gestauchten Kurve stünde in
jeder Reihe ein anderes Schlaufenmaß, die Schlaufen stünden nicht übereinander.

Deshalb rechnet das Muster mit **fester** Periode:

| Größe | Wert | Bedeutung |
|-------|------|-----------|
| `H` | Musterabstand (`scale`/`spacing`, Vorgabe 100 mm) | Bandhöhe = Dämmstärke |
| `r` | `daemmung::FORM` = 0,5 | Schlaufenform, fest |
| `p` | `r · H` | Periodenlänge, **unabhängig von der Länge** |

Die Phase hängt am **Weltnullpunkt** (`x = k · p`), wie bei allen Mustern am
gemeinsamen Raster (`render::Raster`): zwei Flächen mit verschiedenem Rand
zeigen an der gemeinsamen Kante dasselbe Schlaufenbild. Die Bänder liegen um
`H` versetzt, ein Band schwingt zwischen ±`H`/2 — benachbarte Schlaufen
berühren sich (dafür ist die Konstante `K` gewählt).

Beschnitten wird an der Kontur, **nicht gestaucht**: an einer schrägen Kante
hört das Band auf. Die Rechnung steht in `daemmung::band_fest`/`periode`,
gezeichnet wird in `render::muster_daemmung`.

## Dämmung an Wänden

Die Dämmung an einer Wand ist **keine Schraffur**: die Wand trägt die Linienart
„Dämmung“ (`style.line_type`), und deren Band läuft auf den Wandkanten. Die
früheren Muster „Dämmstoffe“ und „Dämmstoffe Schlaufen“ gibt es nicht mehr
(T0132) — sie waren Linienarten in der Musterauswahl. Im Eigenschaften-Panel
stehen **Breite** (H) und **Schlaufenform** (r) deshalb nur bei der Linienart,
nicht mehr bei einer Wandschraffur.

**Wandschraffur „Dämmung“ (T0161)**: Wählt man bei einer Wand die Schraffur
„Dämmung“, zeichnet die Wand **eine** Linie mit der Linienart „Dämmung“ auf
ihrer Mitte — je Wandstück (zwischen Öffnungen) eine. Die Bandhöhe ist die
Wanddicke, das Band füllt damit den Wandkörper; die parallelen Schlaufenbänder
des Musters (`muster_daemmung`) gehören auf eine **Fläche** und werden in einer
Wand nicht gezeichnet. Das ist die Darstellung vor T0136/T0132, wieder
hergestellt in `render::push_wall_hatch` (`render::wall_insulation_paths`).

