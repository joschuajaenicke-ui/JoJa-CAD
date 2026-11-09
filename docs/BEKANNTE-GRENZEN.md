# Bekannte Grenzen — derzeit nicht lösbar

Stand: 9. November 2026 · JoJa-CAD 0.2

Hier steht, was **bekannt** ist, **geprüft** wurde und **jetzt nicht gelöst
werden kann** — weil eine Bibliothek es nicht hergibt, weil die Lösung dem
Zweck widerspricht oder weil der Preis zu hoch ist. Wer eines dieser Themen
anfasst, liest zuerst den Abschnitt. Sonst läuft dieselbe Diskussion wieder
von vorn.

| Nr. | Grenze | Ursache liegt bei | Bedingung für eine Lösung |
| --- | --- | --- | --- |
| 1 | Datei-Drop unter Wayland kommt nicht an | winit 0.30 (Fensterbibliothek) | egui/eframe auf winit 0.31 |
| 2 | Text zerlegen: nur bestimmte Schriften | Systemschriften, Modell | mitgelieferte Schriften für alle Familien |
| 3 | Text hat eine maximale Auflösung | egui-Schriftatlas | Vektor- oder SDF-Text auf der Zeichenfläche |
| 4 | Farbeimer ist eine Näherung | gewähltes Rasterverfahren | vektorielles Flächennetz (Arrangement) |
| 5 | Splines lassen sich nicht stutzen | Modell/Geometrie | numerischer Schnitt + Teilstück-Extraktion |
| 6 | Bei großer Displayskalierung wird die Oberfläche abgeschnitten | Mindestfenstergröße in logischen Punkten | umbrechende/scrollende Ribbon-Gruppen |
| 7 | Fensterposition lässt sich unter Wayland nicht wiederherstellen | Wayland (`xdg-shell`) und winit | `xdg-session-management-v1` in KWin und winit |

---

## 1. Datei-Drop unter Wayland

**Symptom.** Eine Datei aus dem Dateimanager auf das Fenster ziehen: nichts
passiert, keine Meldung.

**Ursache.** winit 0.30 — und damit jedes egui/eframe bis 0.37 — bindet im
Wayland-Zweig kein `wl_data_device`. `HoveredFile`/`DroppedFile` entstehen nur
im X11-Zweig; `RawInput::dropped_files` bleibt leer. Der Programmcode aus T0115
ist korrekt und bekommt nur nie etwas zu sehen.

**Warum jetzt nicht lösbar.** Der Weg über XWayland funktioniert, wurde aber
verworfen: das Programm wirkt dort spürbar träger und weniger scharf. Ein
eigener winit-Fork (~300 Zeilen) wäre möglich, wird aber nicht gepflegt.

**Bedingung.** egui/eframe auf winit 0.31 (dort gibt es Wayland-Drops und ein
asynchrones `data_transfer`-API). Der egui-Port läuft upstream.

**Behelf.** Datei über *Öffnen* wählen. Vollständiger Befund mit Belegstellen
und Prüfskript: [`WAYLAND-DATEI-DROP.md`](WAYLAND-DATEI-DROP.md).

---

## 2. Text zerlegen: nur bestimmte Schriften

**Symptom (Nutzerbeobachtung).** „Zum Zerlegen von Text gibt es nur eine
Schriftart."

**Was der Code tut.** `modify.explode` zerlegt einen Text in die Umrisslinien
**seiner eigenen** Schrift (`actions.rs`, `explode_text` →
`fonts::ttf_bytes(font, style)`). Abgelehnt wird nur die runde Zierschrift
(Quicksand): `TextFont::explodierbar()` in `model.rs` — „Zerlegen ergäbe sehr
viele kleine Linien". Gemessen an derselben Zeichenfolge (5 m Höhe) ergeben
sich unterschiedliche Breiten, das Zerlegen folgt also der Schrift:

| Schrift | Linien | Breite |
| --- | --- | --- |
| Sans | 13 | 26,7 m |
| Serif | 13 | 24,5 m |
| Mono | 13 | 29,2 m |
| Narrow | 13 | 25,7 m |
| DIN Eng | 12 | 20,4 m |
| DIN Mittel | 12 | 25,0 m |
| DIN Breit | 12 | 31,3 m |
| Rund | — | abgelehnt (Meldung) |

**Die eigentliche Grenze.** Die fünf Gebrauchsschriften (Sans, Serif, Mono,
Narrow, Rund) werden **zur Laufzeit aus dem System** geladen
(`fonts.rs`, `bases()`); nur DIN 1451 und EN ISO 3098 liegen im Programm.
Fehlt eine Systemschrift, liefert `ttf_bytes` `None` — das Zerlegen überspringt
die Zeichen stillschweigend und meldet am Ende „Der Text enthält keine
zeichenbaren Umrisse". Auf einem Rechner ohne Liberation/Quicksand ist das
Zerlegen dieser Familien also nicht möglich, ohne dass man es vorher sieht.

**Bedingung.** Alle Familien mitliefern (Lizenz und Programmgröße) oder beim
Zerlegen vorher prüfen und klar melden, welche Schrift fehlt.

**Offen.** Die Beobachtung „nur eine Schriftart" ist mit dem Code nicht
reproduzierbar (siehe Tabelle). Zu klären ist, welcher Fall genau gemeint war —
vermutlich eine fehlende Systemschrift oder ein Text mit gemischten Läufen.

---

## 3. Text hat eine maximale Auflösung

**Symptom.** Bei starkem Zoom wird große Schrift weich statt scharf.

**Ursache.** egui rastert jede Glyphe in **genau der angefragten Pixelgröße** in
einen gemeinsamen Schriftatlas (8192 × 8192). Ein 250-mm-Text bei 600 % wäre
1980 px hoch; ein einzelnes „A" belegt dann rund 3,6 Mio. Atlas-Pixel, der Atlas
läuft über, egui baut ihn neu (268 MB kopieren) und das Bild friert ein.
Deshalb gibt es einen **Rasterdeckel** (`textgroesse.rs`): gerastert wird
höchstens in 128/256/512/1024 px (Vorgabe 256), darüber wird die fertige
Galley gestreckt — scharf ist das nicht mehr.

**Warum jetzt nicht lösbar.** Der Deckel ist keine Einstellung, die man
aufdrehen könnte: er schützt vor dem Atlas-Überlauf. Ohne Deckel friert das
Programm bei großem Zoom ein.

**Bedingung.** Text, der nicht durch den Atlas muss: Umrisslinien auf der
Zeichenfläche (die gibt es bereits — `modify.explode` erzeugt sie) oder
SDF-Text. Beides ist ein eigener Zeichenweg, kein Schalter.

**Behelf.** Für große Beschriftungen den Text zerlegen (dann sind es Linien,
die beliebig scharf skalieren) oder die Rasterauflösung in den Einstellungen
auf 512/1024 px stellen — das kostet Atlas, nicht Schärfe.

---

## 4. Farbeimer ist eine Näherung

**Symptom.** Die mit dem Farbeimer erzeugte Fläche folgt der Kontur nicht
exakt; Ecken sind leicht abgestumpft, Kanten liegen bis zu etwa einen halben
Millimeter daneben.

**Ursache.** Das Verfahren ist bewusst **rasterbasiert** (`boundary.rs`):
Bilevel-Raster mit 1 px = 0,5 mm, Flutfüllung, danach Konturverfolgung und
Douglas-Peucker mit 0,5 mm Toleranz (Vorgabe). Die Begründung steht im
Modulkopf: Schnittpunkte, Bögen, Gehrungen und Wandkörper sind in einem
Bilevel-Bild von selbst dicht; eine rein vektorielle Flächensuche scheitert an
jedem Ende, das nicht auf den Bruchteil eines Millimeters genau passt.

**Warum jetzt nicht lösbar.** Exakt heißt: ebenes Arrangement aller Linien
(Knoten, Kanten, Flächen) mit Toleranzbehandlung für Haarrisse. Das ist ein
eigener Rechenkern — und die Rasterlösung ist genau deshalb entstanden, weil
der Vektorweg an realen Plänen scheitert.

**Bedingung.** Vektorielles Flächennetz mit sauberer Toleranzbehandlung
(Snap der Endpunkte vor dem Aufbau des Netzes). Danach wären Flächen exakt und
zusätzlich als Objekt editierbar.

**Behelf.** Für maßhaltige Flächen die Kontur als Polylinie zeichnen und die
Füllfläche über *Kontur anklicken* (`draw.fill`, Eingabe „Kontur") erzeugen —
dann ist der Umriss die gezeichnete Kontur selbst. Die Rastertoleranz des
Farbeimers lässt sich innerhalb des Verfahrens nicht unterschreiten.

---

## 5. Splines lassen sich nicht stutzen

**Symptom.** *Stutzen* auf einen Spline: „Stutzen wird für diesen Objekttyp
nicht unterstützt".

**Ursache.** `actions.rs` behandelt beim Stutzen Linie, Polylinie, Kreis,
Bogen, Ellipse und Ellipsenbogen; alles andere fällt in den Sammelzweig mit
dieser Meldung. Ein Spline ist eine **zentripetale Catmull-Rom-Kurve** durch
seine Stützpunkte (`geom::Spline`, `catmull_rom_point`); sie hat keine
analytische Schnittformel, und ein Teilstück ist nicht einfach „Punkte
wegschneiden" — die Tangentialität hängt an den Nachbarpunkten.

**Warum jetzt nicht lösbar.** Es fehlen zwei Bausteine: ein numerischer Schnitt
(Kurve gegen Linie/Kreis, Nullstellensuche auf dem Parameter) und die
Extraktion eines Teilstücks als **neuer** Spline mit neu berechneten
Stützpunkten. Beides ist lösbar, aber ein eigener Rechenweg mit eigenen Tests —
kein Nebenprodukt des vorhandenen Stutzens.

**Bedingung.** Numerischer Schnitt + Teilstück-Extraktion.

**Behelf.** Den Spline **zerlegen** (`modify.explode`): das ergibt eine
Polylinie mit der abgeflachten Kurve (geprüft: 4 Stützpunkte → 96 Punkte), und
die lässt sich stutzen. Der Preis ist die Glättung — es sind danach viele
kurze Strecken statt einer Kurve.

---

## 6. Bei großer Displayskalierung wird die Oberfläche abgeschnitten

**Symptom.** Bei hoher Skalierung (z. B. 200 %) verschwinden Teile der
Oberfläche rechts oder unten aus dem Fenster; das Fenster lässt sich nicht
kleiner ziehen.

**Ursache.** Die Mindestgröße des Fensters steht in **logischen Punkten**:
`with_min_inner_size([1080.0, 680.0])` (`main.rs`). Bei Skalierung 2,0 sind das
2160 × 1360 **Gerätepixel** — mehr, als ein 1920 × 1200-Bildschirm hergibt. Das
Fenster ist dann größer als der Bildschirm, und was rechts oder unten liegt,
ist nicht erreichbar. Dazu kommen feste Breiten in Punkten (Seitenleisten 196,
Eigenschaften 280/330) und eine Ribbon, die laut Test
(`reiter_ansicht_passt_in_die_mindestbreite`) rund 1080 Punkte Inhalt braucht.

**Warum jetzt nicht lösbar.** Kleiner ziehen geht nicht, ohne dass die Ribbon
abgeschnitten wird — die Mindestbreite ist die Summe der Gruppen, nicht eine
Willkür. Eine umbrechende oder scrollende Ribbon ist ein Umbau der obersten
Bedienebene.

**Bedingung.** Ribbon-Gruppen, die bei Platzmangel umbrechen oder waagerecht
scrollen, plus eine Mindestgröße, die sich an der Bildschirmgröße orientiert
(nie größer als der verfügbare Bereich).

**Behelf.** Skalierung auf 100–150 % stellen oder das Fenster im Vollbild
(F11) benutzen — dort steht die volle Bildschirmfläche zur Verfügung.

---

## 7. Fensterposition unter Wayland

**Symptom.** Unter Wayland (z. B. KDE Plasma) merkt sich das Programm die
Fenstergröße, das Fenster erscheint aber dort, wo die Fensterverwaltung es
hinstellt — nicht an der Stelle des letzten Laufs.

**Ursache.** `xdg-shell` kennt weder eine Abfrage noch eine Vorgabe der
Fensterposition: `xdg_toplevel` hat keine Koordinaten, und winit liefert für
`inner_position()`/`outer_position()` `NotSupported` (`set_outer_position` ist
im Wayland-Zweig eine leere Funktion). Das trifft jedes Wayland-Programm; KDE
schreibt es selbst so aus („apps aren’t allowed to position themselves“).
Nachweis im Code: `winit-0.30.13/src/platform_impl/linux/wayland/window/mod.rs`.

**Was das Programm trotzdem tut (T0141).** `sitzung.json` merkt Größe, Lage und
Maximiert-Zustand (`sitzung::Fensterlage`). Fehlt die Lage (Wayland), steht
`mit_lage: false` in der Datei und die **Größe** kommt beim Start zurück; unter
X11, Windows und macOS kommt auch die Position zurück (`main.rs` setzt
`with_position` nur, wenn `Fensterlage::lage()` eine Lage liefert). `x`/`y`
bleiben dabei Zahlen, weil eine `null` ältere Fassungen beim Laden der Sitzung
scheitern ließe. Belege: `jjcad-core`-Test
`sitzung::tests::fensterlage_ohne_position_ist_brauchbar`,
`app::tests_fensterlage` (Umrechnung mit und ohne Fensterrechtecke) und ein Lauf
unter Xvfb — Fenster auf 1240 × 780 bei (320, 180) gezogen, beendet, neu
gestartet: dieselbe Geometrie, `sitzung.json` trägt `x: 320.0, y: 180.0`,
`mit_lage: true`.

**Warum jetzt nicht lösbar.** Ein Ausweichen auf XWayland ist verworfen (Grenze 1:
träger und unschärfer). Die Programmseite kann eine Position nicht erzwingen.

**Bedingung.** `xdg-session-management-v1`, das KWin umsetzen und winit
durchreichen müsste (winit führt das Thema als Issue #4636) — dann darf ein
Programm seinen Zustand samt Position wieder anfordern.

**Behelf.** Fensterregel in KWin (Systemeinstellungen → Fensterverwaltung →
Fensterregeln): Eigenschaft „Position“ auf „Erzwingen“ für `jjcad`. Die Regel
setzt KWin selbst, deshalb wirkt sie auch unter Wayland.

---

## Wie ein Eintrag hier landet

1. Symptom am laufenden Programm beobachtet, nicht vermutet.
2. Ursache im Code belegt (Datei und Funktion, nicht „irgendwo in egui").
3. Geprüft, dass es **nicht** an einer Einstellung liegt.
4. Bedingung benannt, unter der es gelöst werden kann — sonst ist es kein
   Eintrag für diese Liste, sondern ein Ticket.

Fällt eine Bedingung weg (neue Bibliotheksversion, neuer Rechenkern), wird der
Eintrag gelöscht und die Arbeit als Ticket aufgenommen.
