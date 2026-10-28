# JoJa-CAD v0.2 — Arbeitsstand

Kurzer Übergabestand für neue Sitzungen. Nach jeder Änderung fortschreiben.
Stand: 28.10.2026

## Build & Start

```
cargo build --release -p jjcad-gui      # Release
cargo build -p jjcad-gui                # Debug
./target/release/jjcad samples/Wohnhaus.jjcad
```

Nach jedem Build **laufende Instanz schließen und neu starten** — eine bereits
offene Fensterinstanz zeigt weiterhin den alten Stand.

## Erledigt (verifiziert)

- **Wandanschlüsse neu aufgebaut** (`crates/jjcad-core/src/wall.rs`, ersetzt den
  alten Beschnitt-Apparat in `render.rs` ersatzlos):
  - Eine Wand ist ein Band um die verschobene Achse. Verbunden = **Basispunkte**
    zusammen (0,5 mm). An einem verbundenen Ende wird die Wand an der
    **Anschlussnaht** abgeschnitten: Gerade durch die beiden Gehrungspunkte
    (Schnitt Außenkante↔Außenkante, Innenkante↔Innenkante). Bei gleich dicken,
    mittigen Wänden ist das die Winkelhalbierende; sonst die dicken- und
    ausrichtungsabhängige Halbierende. Beide Wände rechnen **dieselben zwei
    Punkte** — keine zwei Nähte mehr nebeneinander.
  - Geschnitten wird nur, wenn es **beidseitig gedeckt** ist (der abgeschnittene
    Teil liegt im verlängerten Band des Nachbarn, die Naht schneidet beide
    Bänder, Länge ≤ 6·h_min für beide gleich). Sonst Stumpfstoß. Ein Ende, das
    am Knoten gerade durchläuft (fluchtender Nachbar), wird nicht geschnitten.
  - Gezeichnet wird die **Vereinigung**: eine Randkante nur, wo außen keine
    andere Wand liegt. Damit sind Gehrung, Stumpfstoß, Stich, Kreuz und
    Mehrfachknoten keine Sonderfälle mehr; Nähte im Wandinneren entfallen.
  - Fang nutzt dieselbe Geometrie (`wall_contour_segments`), Füllband an der
    Naht 0,6 mm keilförmig (Haarstrich weg).
  - Abnahme: `crates/jjcad-core/tests/wandanschluss.rs` — 8 Winkel × 4
    Dickenpaare × 5 Ausrichtungen (160 Ecken), geprüft auf „keine Lücke"
    (Vektor **und** Pixelbild), „keine Naht im Körper", „jede Füllkante
    umrandet"; gemeinsame Gehrung gleich dicker Wände; drei Wände in einem
    Punkt ohne Splitter; Stich/Kreuzung; Reihenfolge der Wände ohne Einfluss.
    Messhilfen: `examples/wand_probe.rs` (Fälle als PNG + Kennzahlen),
    `examples/zeichnung_png.rs <datei.jjcad> [ziel] [x0 y0 x1 y1]`.
  - **Offen:** bei schrägen Ecken, wo der Nachbar den abgeschnittenen Zipfel
    nicht deckt (45° mittig/rechts, 135° 240/360, 20° 365/115), bleibt die
    ehrliche Vereinigung stehen — die schräge Stirnfläche spitzt über den
    Nachbarn hinaus. Ein „Zipfel abschneiden + freie Nahtkante nachziehen"
    (wie das alte `miter_patch`, aber konsistent) ist ein eigenes Ticket.
- **Wandschraffur**: `hatch` an der Wand (Vollfläche, 45°, Kreuz, Punkte,
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
   oder wenn der Haarstrich an der Naht auch bei starkem Zoom weg soll: an der
   Ecke stoßen zwei Füllungen aneinander (gleiche Farbe fällt nicht auf, das
   Füllband deckt die Naht).
4. **Zipfel an schrägen, ungleich dicken Ecken** — schneidet der Nachbar den von
   der Naht abgeschnittenen Zipfel nicht, wird heute nicht geschnitten (sonst
   Loch im Körper); sichtbar spitzt die schräge Stirnfläche über den Nachbarn
   hinaus. Lösung: Zipfel abschneiden, Restkante als Linie nachziehen.
5. Bemaßung: Griffe an Maßketten fehlen.
6. ~~Wandfang-Schalter in der Oberfläche~~ — erledigt: Checkbox „Wandkanten" im
   Einstellungen-Dialog, Ribbon-Gruppe Fangen, Statusmenü Fang (inkl. „Alle an/aus").

## Arbeitsweise (Lessons)

- Rust-Änderungen mit dem **patch-Tool und exakten Ankern** machen, nicht mit
  Python-String-Ersetzungen — die haben mehrfach Code zerschossen.
- Nach jeder Änderung `cargo build`, sonst prüft die Abnahme den alten Stand.
- Abnahme möglichst **messbar** machen: gespeicherte Datei prüfen
  (`align`, Koordinaten) oder Pixel messen, nicht nur „sieht gut aus".

## Paralleles Arbeiten (Test durch Joschua, Entwicklung durch Agent)

**Regel 1 — getrennte Binaries.** Der Agent baut/startet nur eigene Kopien
(`/tmp/jjcad-agent`, eigener Xvfb-Display) und beendet **nie** den Prozess des
Nutzers. Kein `pkill -x jjcad` in Testskripten.

**Regel 2 — Testkanal.** Der Nutzer testet ausschließlich

```
~/JoJa-CAD/JoJa-CAD_v0.2/scripts/build-live.sh    # baut + kopiert nach ~/jjcad-live/jjcad
~/JoJa-CAD/JoJa-CAD_v0.2/scripts/start-live.sh [datei.jjcad]
```

`build-live.sh` gibt die Uhrzeit aus; passt sie nicht zur laufenden Instanz,
ist es der alte Stand (Fenster schließen, neu starten).

**Regel 3 — Tickets statt Zurufe.** Board `joja-cad` (`hermes kanban …`,
Slash `/kanban`, Dashboard):

```
hermes kanban create "Titel" --body "Beschreibung" --workspace dir:$PWD
hermes kanban list
hermes kanban show t_xxxx
hermes kanban comment t_xxxx "Details/Screenshot-Pfad"
hermes kanban request-review t_xxxx      # fertig zum Testen
hermes kanban request-changes t_xxxx     # Test negativ -> zurueck an die Entwicklung
hermes kanban complete t_xxxx            # abgenommen
```

**Regel 4 — Git pro Ticket.** Ein Zweig je Ticket (`fix/t_xxxx`), Merge nach
`main` erst nach Abnahme. Zwei gleichzeitig offene Tickets → `git worktree`
mit eigenem `CARGO_TARGET_DIR`, damit sich Builds nicht behindern.

**Ablauf pro Runde:** Karte anlegen → Agent claimt, implementiert, baut,
`build-live.sh`, Kommentar „bereit" + `request-review` → Nutzer testet die
Live-Binary → `complete` oder `request-changes` mit Befund.
