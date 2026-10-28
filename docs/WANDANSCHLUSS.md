# Wandanschluss: Eckpunkte S_l und S_r in allgemeiner Lage

Diese Herleitung ist die Grundlage von
`crates/jjcad-core/src/wall.rs`: **eine** Naht je Wandpaar, unabhängig von
Dicke, Ausrichtung und Winkel.

## 1. Gegebene Größen

| Größe | Bedeutung |
|---|---|
| `X` | Verbindungspunkt der beiden Basislinien (Weltkoordinaten) |
| `θ` | Richtungswinkel der Basislinie von Wand A, gemessen von der x-Achse, Laufrichtung **auf X zu** |
| `α` | Winkel zwischen den Strahlen X→A und X→B |
| `a_l`, `a_r` | Abstand der linken / rechten Kante von Wand A zur Basislinie (> 0) |
| `b_l`, `b_r` | Abstand der linken / rechten Kante von Wand B zur Basislinie (> 0) |

„Links" und „rechts" beziehen sich auf die Laufrichtung der jeweiligen Wand
(A auf X zu, B von X weg). Aus Breite `w` und Versatz `o` ergeben sich die
Kantenabstände als `l = w/2 + o` und `r = w/2 − o`.

## 2. Lokales Koordinatensystem

- Ursprung `X`, x'-Achse in Laufrichtung von A: `d_A = (cos θ, sin θ)`
- y'-Achse nach links: `n_A = (−sin θ, cos θ)`
- Wand B im lokalen System: `t_B = (−cos α, −sin α)`,
  `n_B,l = (sin α, −cos α)`, `n_B,r = (−sin α, cos α)`

## 3. Schnittpunkte

**S_l** (Schnitt der linken Kanten): Kante A `(x', a_l)`, Kante B
`τ·t_B + b_l·n_B,l`. Die y'-Komponente liefert `τ = −(a_l + b_l·cos α)/sin α`,
eingesetzt in die x'-Komponente:

```
x'_l =  (b_l + a_l·cos α) / sin α
x'_r = −(b_r + a_r·cos α) / sin α
```

## 4. Allgemeine Lage

`S_l = X + x'_l·d_A + a_l·n_A` und `S_r = X + x'_r·d_A − a_r·n_A`, gleichwertig
mit der Drehmatrix `R(θ)` aus den lokalen Vektoren `(x'_l, a_l)` bzw.
`(x'_r, −a_r)`.

## 5. Winkel aus den Punkten

```
d_A = (X − P_A) / |X − P_A|      # Laufrichtung A, zeigt zu X
θ   = atan2(d_A.y, d_A.x)
u   = P_A − X                    # Strahl X→A
v   = P_B − X                    # Strahl X→B
α   = atan2(u.x·v.y − u.y·v.x, u·v)   # vorzeichenbehaftet von u nach v
```

Die Formeln gelten unverändert mit vorzeichenbehaftetem α — eine
Fallunterscheidung nach Drehrichtung ist nicht nötig.

## 6. Umsetzung im Code

- Kanten statt Winkel: `S_l = f.edge(−s_f) ∩ g.edge(+s_g)`,
  `S_r = f.edge(+s_f) ∩ g.edge(−s_g)` mit `s = sign(perp(d)·n)` als „linke
  Kante" der jeweiligen Laufrichtung `d` (vom Knoten in die Wand). Beide Wände
  rechnen dieselben zwei Punkte — die Naht kann nicht auseinanderlaufen.
- Geschnitten wird an dieser Naht; der Körper wird am verbundenen Ende um die
  Reichweite der Naht verlängert (`MITER_LIMIT · h`), damit die Gehrung die
  Ecke **ergänzt** und nicht nur abschneidet.
- Grenzen (alle **beidseitig** geprüft: dieselbe Entscheidung für beide Wände,
  sonst schneidet die eine und die andere nicht — genau das erzeugt Finne bzw.
  Überlappung):
  1. `α ≈ 0/π` (fluchtend oder rückläufig): keine Naht, Wand läuft durch.
  2. Öffnungswinkel `ψ` (Winkel zwischen den beiden Wänden am Knoten: 180° =
     fluchtend, 90° = Ecke) kleiner als `MIN_JOINT_ANGLE` (30°): keine Naht.
     Bei spitzerem Winkel liegt die Naht so weit vom Knoten, dass die Wand als
     meterlange Finne über ihn hinausschießt.
  3. Nahtlänge > `MITER_RATIO · 2·h_min` (Standard 6): stumpf stoßen.
  4. Nimmt der Schnitt mehr als ein Viertel einer Wandquerschnittsfläche weg,
     die der Nachbar nicht stellt (Wände auf entgegengesetzten Seiten ihrer
     Basislinien), wird nicht geschnitten — sonst entstünde ein Loch.
  5. Liegt ein Gehrungspunkt weiter vom Knoten als **die kürzere** der beiden
     Wände lang ist, wird ebenfalls nicht geschnitten (kurze Wand, spitzer
     Winkel, Punkt 3 der Herleitung).

## 7. Probe

- **θ = 0**: die Formeln reduzieren sich auf die lokale Lösung aus Abschnitt 3.
- **α = 90°, θ = 90°**: `XS_l = (−a_l, b_l)`.
