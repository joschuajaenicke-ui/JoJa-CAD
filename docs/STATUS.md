# JoJa-CAD v0.2 — Arbeitsstand

Fortlaufender Stand der Umsetzung. Stand: 29.10.2026

## Build & Start

```
cargo build --release -p jjcad-gui      # Release
cargo build -p jjcad-gui                # Debug
./target/release/jjcad samples/Wohnhaus.jjcad
```

Nach jedem Build **laufende Instanz schließen und neu starten** — eine bereits
offene Fensterinstanz zeigt weiterhin den alten Stand.

## Erledigt (verifiziert)

- **Fenster ohne Titelleiste (T0098)**: Einstellungen → Bedienung → Zeile
  „Fensterrahmen“ mit **System** (Vorgabe) und **Ohne Titelleiste** (experimentell,
  unter KDE geprüft). Gespeichert als `UserPrefs.fensterrahmen` (`system` /
  `ohne_titelleiste`) in `prefs.json`; die Erzeugung des Fensters liest sie schon
  vor dem Start (`ViewportBuilder::with_decorations`), ein Wechsel im Dialog
  schickt `ViewportCommand::Decorations` sofort nach. Ohne Titelleiste zeichnet
  die Kopfzeile oben rechts **Minimieren**, **Vergrößern/Wiederherstellen** und
  **Schließen**, ein Zug verschiebt das Fenster (`StartDrag`), ein Doppelklick
  schaltet die Größe um, die Kantenzonen (5 px) ändern sie (`BeginResize`).
  Tests: `app::tests_fensterrahmen` (7), `dialogs::tests_einstellungen::fensterrahmen_laesst_sich_umschalten`,
  `persist::tests::fensterrahmen_vorgabe_und_gespeichert`.
- **Drehpunkt der Einfüge-Vorschau (T0070)**: Einstellungen → Bedienung → Zeile
  „Drehpunkt" mit den Modi **Mittelpunkt** (Vorgabe seit T0087) und
  **Basispunkt**; gespeichert als `UserPrefs.paste_pivot` (`base_point`/`center`)
  in `prefs.json`. In beiden Modi dreht die Auswahl um ihren Mittelpunkt; der Modus
  entscheidet, welcher Punkt am Fadenkreuz steht (`ghost::clipboard_entities`).
  Mittelpunkt = untere linke Ecke der gedrehten Auswahl, sodass die Vorschau
  immer oben rechts vom Fadenkreuz bleibt. Vorschau und eingefügtes Ergebnis
  rechnen dieselbe Funktion. Ein Wechsel in den Einstellungen wirkt sofort, auch
  auf eine laufende Vorschau (`App::set_paste_pivot`, T0087). Tests: `ghost::tests_drehpunkt`,
  `app::tests_drehpunkt_einfuegen`, `dialogs::tests_einstellungen::bedienung_zeigt_den_drehpunkt`,
  `persist::tests::drehpunkt_der_einfuege_vorschau_wird_gespeichert`.
- **Wandanschlüsse: allgemeine Gehrung** (`crates/jjcad-core/src/wall.rs`,
  Herleitung in `docs/WANDANSCHLUSS.md`): An einem verbundenen Ende wird jede
  Wand an der **Anschlussnaht** abgeschnitten — der Geraden durch die beiden
  Gehrungspunkte `S_l` (Schnitt der linken Kanten) und `S_r` (Schnitt der
  rechten Kanten), jeweils bezogen auf die Laufrichtung am Knoten. Das gilt für
  beliebige Dicken, beliebigen Versatz (links/mittig/rechts) und beliebige
  Winkel; bei gleich dicken, mittigen Wänden ist es die Winkelhalbierende.
  Beide Wände rechnen **dieselben zwei Punkte** — keine zwei Nähte mehr
  nebeneinander (das war der Kern des alten Fehlers).
  - Verbunden = **Basispunkte** zusammen (0,5 mm). Der Körper wird am
    verbundenen Ende um die Reichweite der Naht verlängert, damit die Gehrung
    die Ecke **ergänzt** statt sie nur abzuschneiden.
  - Grenzen (alle **beidseitig** — dieselbe Entscheidung für beide Wände, sonst
    schneidet die eine und die andere nicht; genau daraus entstanden Finne und
    Überlappung): fluchtend/rückläufig → keine Naht (Wand läuft durch, Kreuz
    und Verlängerung bleiben gerade); **Öffnungswinkel < 40°**
    (`MIN_JOINT_ANGLE`) → stumpf, sonst schießt die Wand als meterlange Finne
    über den Knoten; Nahtlänge > 6 · Wandbreite der dünneren Wand
    (`MITER_RATIO`) → stumpf; nimmt der Schnitt mehr als ein Viertel einer
    Wandquerschnittsfläche weg, die der Nachbar nicht stellt (Wände auf
    entgegengesetzten Seiten ihrer Basislinien) → stumpf, sonst Loch; liegt ein
    Gehrungspunkt weiter vom Knoten als die **kürzere** Wand lang ist → stumpf.
  - Gezeichnet wird die **Vereinigung**: eine Randkante nur, wo außen keine
    andere Wand liegt. Gehrung, Stumpfstoß, Stich, Kreuz und Mehrfachknoten
    brauchen damit keinen Sonderfall.
  - Fang nutzt dieselbe Geometrie. Gegen den Antialiasing-Haarstrich sind die
    **Füllflächen** an der Naht um 0,6 mm (`FILL_BLEED`) in die Nachbarwand
    ausgebucht (`Body::fills`) und überlappen dort; die Geometrie (`pieces`),
    Umrandung und Fang bleiben exakt. Ein eigenes Bandpolygon taugt nicht: bei
    breitenabhängiger Tastung war es als Kante zu sehen und flackerte, bei
    0,4 mm Breite ist es unter einem Pixel und wird von der GUI verworfen —
    dann bleibt der Haarstrich stehen.
  - Körper werden **bei Bedarf** gerechnet (OnceCell) — der Treffer-Test fragt
    die Bounding-Box jeder Wand ab; `detect_boundary` auf dem Wohnhaus bleibt
    damit unter 100 ms (vorher 127 ms, mit eifriger Rechnung 222 ms).
  - Abnahme: `tests/wandanschluss.rs` — 8 Winkel × 4 Dickenpaare × 5
    Ausrichtungen (160 Ecken), geprüft auf „keine Lücke" (Vektor **und**
    Pixelbild), „keine Naht im Körper", „jede Füllkante umrandet"; gemeinsame
    Gehrung gleich dicker Wände; drei Wände in einem Punkt ohne Splitter;
    Stich/Kreuzung; Reihenfolge der Wände ohne Einfluss. Dazu die bestehenden
    Tests `wand_ecken.rs`, `wandgehrung_spitz.rs`, `snap_wandfang.rs`
    (dort jetzt: „Körper gefüllt **bis zur Naht**").
  - Messhilfen: `examples/wand_probe.rs` (Fälle als PNG + Kennzahlen),
    `examples/zeichnung_png.rs <datei.jjcad> [ziel] [x0 y0 x1 y1]`,
    `samples/Wandpruefung.jjcad`.
- **Volumen** (`crates/jjcad-core/src/model.rs`, `EntityKind::Volume`): ein
  Architekturelement zwischen Wand und Füllfläche. Es hat die Geometrie einer
  Füllfläche (Außenring + Inseln, Muster, Randlinie) **und eine Höhe**; in den
  Eigenschaften stehen Höhe, Fläche und Volumen in m³.
  - **Es verbindet sich mit den Wänden**: Wandkanten fallen aus, wo sie im
    Volumen liegen; die Umrandung des Volumens fällt aus, wo sie in einer Wand
    liegt. Beide Seiten prüfen dieselbe Regel, deshalb steht an der Naht keine
    Linie — ohne dass Geometrie verschmolzen wird. Kanten werden dafür an den
    Rändern der jeweils anderen Fläche unterteilt (`render::sichtbare_stuecke`),
    damit nur das verdeckte Stück wegfällt.
  - Die Fläche wird standardmäßig im **Wandton** (74 % Richtung Weiß) gefüllt,
    damit der Übergang auch als Farbe nicht auffällt (`render::WALL_FILL_MIX`).
  - Gedacht für Mauerwerk, das sich nicht als Rechteckwand zeichnen lässt:
    runde Wand, Pfeiler, Sonderform. Beispiel: `samples/Volumen.jjcad`
    (Halbkreisbogen auf zwei geraden Wänden, erzeugt von
    `examples/volumen_erzeugen.rs`).
  - Aktionen wie bei der Füllfläche: `draw.volume` (Kontur anklicken),
    `draw.volume.points` (Punkte setzen) und `draw.volume.paint` (Farbeimer) —
    erreichbar über die geteilte Schaltfläche **Architektur → Volumen**
    (Würfel-Symbol) mit Modus-Menü; `modify.set height` ändert die Höhe.
  - Wände tragen ihre Höhe schon als Metadatum; das Eigenschaften-Panel zeigt
    jetzt zusätzlich das Volumen (Länge × Dicke × Höhe, brutto).
  - Abnahme: `tests/volumen.rs` — Anschluss in beiden Richtungen ohne Linie
    (Vektor **und** Pixelbild), gleicher Ton wie die Wand, Höhe/Volumen in m³,
    Höhenänderung über `modify.set`, Speichern/Laden, alle drei Eingaben
    (`volumen_drei_eingaben`, Farbeimer über `detect_boundary`).
- **Schraffuren nach DIN 1356-1:2026-09, Tabelle 5** (T0095): alle 18
  Kennzeichnungen der Schnittflächen als Muster in `style::FillPattern`
  (`ALL` in der Reihenfolge der Norm); die früheren Muster heißen jetzt nach
  ihrem Baustoff (Mauerwerk, Kunststoffe, Sand, Dämmstoffe). Ein Generator
  (`render::push_muster`) zeichnet sie für Füllfläche, Volumen **und**
  Wandschraffur; Schlüssel und Bezeichnung finden jedes Muster wieder, die
  alten Dateinamen bleiben gültig. Prüfblatt `samples/Schraffuren.jjcad`
  (Erzeuger `examples/schraffuren_erzeugen.rs`), Test
  `crates/jjcad-core/tests/schraffuren_din.rs`.
  **Ein Raster für alle Muster** (`render::Raster`, Ursprung im Weltnullpunkt):
  Lage der Musterlinien `(i + versatz) · Abstand`, Unterteilungen (Striche,
  Gruppen) auf einem globalen Takt, Figuren in Rasterzellen. Zwei benachbarte
  Flächen zeigen dadurch dasselbe Strichbild ohne Sprung an der Kante; vorher
  begann jede Linie ihr Strichmuster am eigenen Beschnitt — die Unterteilungen
  lagen auf verschiedenen Höhen und wirkten zufällig (Turn 2).
- **Wandschraffur**: `hatch` an der Wand (alle Muster der Tabelle 5,
  **Dämmung = Mäander**), gezeichnet als Displaylisten-Primitive.
- **Wandausrichtung**: `WallAlign` (Links/Mittig/Rechts), Versatz ±Dicke/2,
  Tab wechselt beim Platzieren (Zyklus, `consume_key`), Wert landet in `draw.wall`.
- **Vorschau**: Wandkörper mit Ausrichtung + gestrichelter Achse; Tür/Fenster
  springen auf die nächste Wand und drehen sich in Wandrichtung.
- **Türen/Fenster**: Host-Bindung, Öffnung im Wandkörper (Ausrichtung der Wand
  berücksichtigt), R = Blattseite, F = Anschlag. Eigenschaft „Blatt: Seite A/B".
- **Farben/Stärken**: Vorgaben-Dropdowns (0,50/0,35/0,25 mm; 13 Farben),
  freier Wert inline, Komma und Punkt als Dezimalzeichen.
- **Eigenschaften-Panel**: einheitliche Zeilen (96 px Label, 150 px Wert),
  alle Auswahllisten gleiche Optik/Größe, Identisch während/nach dem Platzieren
  (Unterschied: Untertitel + Geometrie-Block).
- **Zieh-Modus als Enum**: `persist::DragMode` (`hold_and_drag` = Standard,
  `two_clicks`) liegt als `UserPrefs.drag_mode` in `prefs.json`; alte Dateien
  ohne Schlüssel ergeben `hold_and_drag`, das Altfeld `drag_two_clicks` aus
  Build b7 wird beim Laden übernommen. Nur Typ/Persistenz umgestellt — die
  Bedienung ist unverändert (Auswahlfeld-Optik und Zwei-Klick-Ablauf folgen).
- **Punktfang auf Wandkontur** (`snap.wall_edges`, Standard an): Die umgrenzenden
  Wandlinien (Längskanten, Gehrungs-/Stirnecken, Laibungen an Öffnungen) liefern
  Fangpunkte — Endpunkt, Mittelpunkt, Nächster Punkt und Schnittpunkte mit anderen
  Wänden. EINE Geometriequelle: `wall::WallIndex` (Körper + Randstücke) versorgt
  Darstellung (`render_wall`) und Fang (`render::wall_contour_segments`); Kandidaten werden in
  `snap::snap_candidates` entdoppelt (0,1 mm). Nachweis: Wand 0,0→5000,0, 300 mm —
  Zeiger 5 px neben der Ecke → Status „X 0,00 Y 0,15 · Endpunkt", Ring auf der
  Ecke; `wall_edges=false` → (-0,02; 0,17) ohne Fang, Achse bleibt fangbar.

- **Wandgehrung an extrem spitzen Ecken / ungleich dicken Wänden**: Der
  Beschnitt-Apparat von T0020 (`miter_patch`, `miter_dicht`, `keil_gedeckt`,
  `line_outside_pieces`, `partner_body`) ist **entfernt** — er hat je Wand eine
  eigene Naht gerechnet und bei versetzten Achsen zwei Nähte nebeneinander
  erzeugt. An seine Stelle tritt die gemeinsame Anschlussnaht in
  `crates/jjcad-core/src/wall.rs` (siehe erster Punkt). Nachweis:
  `tests/wandgehrung_spitz.rs` (7 Tests) und `tests/wandanschluss.rs` bleiben
  grün, ohne Patch und ohne Sonderfall.

## Offen

1. **Tür/Fenster-Ausrichtung im Eigenschaften-Panel** — beim Anlegen gesetzt,
   noch nicht nachträglich änderbar (braucht Wanddicke im Änderungspfad).
2. **Öffnungsposition als Wandparameter** (Abstand längs + Lage quer) statt
   absolutem Mittelpunkt — nötig, damit Öffnungen beim Verschieben der Wand folgen.
3. **Echte Polygon-Vereinigung der Wandfüllungen** — gezeichnet wird weiter je
   Wand (an der Naht beschnitten, Nebenseite über den Vereinigungstest
   ausgeblendet). Nötig erst, wenn Wände unterschiedliche *Füllfarben* bekommen
   — der Haarstrich an der Naht ist durch die überlappenden Füllflächen
   (`FILL_BLEED`) erledigt. Zwei verschiedene Füllfarben an einer Naht kann
   diese Überlappung aber nicht sauber trennen.
4. **Stumpf statt Gehrung bei entgegengesetzter Ausrichtung** — liegen beide
   Wände auf entgegengesetzten Seiten ihrer Basislinien (links/rechts) und ist
   der Winkel spitz, würde die Naht mehr als eine halbe Wandquerschnittsfläche
   wegnehmen; dort wird bewusst stumpf gestoßen (die Körper berühren sich nur,
   jede Gehrung risse ein Loch). Wer das anders will, braucht eine Entscheidung
   über die dann fehlende Fläche — kein Rechenfehler, eine Modellfrage.
5. Bemaßung: Griffe an Maßketten fehlen.
6. ~~Wandfang-Schalter in der Oberfläche~~ — erledigt: Checkbox „Wandkanten" im
   Einstellungen-Dialog, Ribbon-Gruppe Fangen, Statusmenü Fang (inkl. „Alle an/aus").
