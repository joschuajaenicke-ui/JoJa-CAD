# Dämmungs-Schlaufenkurve

> **Im Programm:** die Linienart **Dämmung** (`LineType::Insulation`). Die
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
- Gezeichnet wird ein Polygonzug; die Stützpunkte folgen der Kurve (rund 25 je
  Periode, mindestens 12, höchstens 6000 Punkte je Linie). Weil das Band die
  Anzeigegeometrie ist, folgen Trefferfläche, Fang und DXF-Export mit.
- Schraffuren bleiben außen vor: eine Wand in „Dämmung“ zeichnet ihre Kanten als
  Band, ihr Muster aber als dünne durchgezogene Striche.
