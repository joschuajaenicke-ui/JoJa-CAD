# JoJa-CAD v0.2 — Arbeitsstand

Kurzer Übergabestand für neue Sitzungen. Nach jeder Änderung fortschreiben.
Stand: 25.10.2026, 11:50

## Build & Start

```
cargo build --release -p jjcad-gui      # Release
cargo build -p jjcad-gui                # Debug
./target/release/jjcad samples/Wohnhaus.jjcad
```

Nach jedem Build **laufende Instanz schließen und neu starten** — eine bereits
offene Fensterinstanz zeigt weiterhin den alten Stand.

## Erledigt (verifiziert)

- **Wandanschlüsse**: Füllungen werden vor allen Linien gezeichnet; Gehrung über
  die Innenwinkel-Halbierende, Knotenpunkt = Schnitt der *verschobenen* Achsen
  (`wall_end_corners`). Spitze/stumpfe Ecken, T- und X-Anschlüsse geschlossen.
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
- **Punktfang auf Wandkontur** (`snap.wall_edges`, Standard an): Die umgrenzenden
  Wandlinien (Längskanten, Gehrungs-/Stirnecken, Laibungen an Öffnungen) liefern
  Fangpunkte — Endpunkt, Mittelpunkt, Nächster Punkt und Schnittpunkte mit anderen
  Wänden. EINE Geometriequelle: `render::WallLayout::of` versorgt Darstellung
  (`render_wall`) und Fang (`render::wall_contour_segments`); Kandidaten werden in
  `snap::snap_candidates` entdoppelt (0,1 mm). Nachweis: Wand 0,0→5000,0, 300 mm —
  Zeiger 5 px neben der Ecke → Status „X 0,00 Y 0,15 · Endpunkt", Ring auf der
  Ecke; `wall_edges=false` → (-0,02; 0,17) ohne Fang, Achse bleibt fangbar.

## Offen

1. **Tür/Fenster-Ausrichtung im Eigenschaften-Panel** — beim Anlegen gesetzt,
   noch nicht nachträglich änderbar (braucht Wanddicke im Änderungspfad).
2. **Öffnungsposition als Wandparameter** (Abstand längs + Lage quer) statt
   absolutem Mittelpunkt — nötig, damit Öffnungen beim Verschieben der Wand folgen.
3. **Wand-Union**: letzter Feinschliff an extrem spitzen Ecken (Millimeter).
4. Bemaßung: Griffe an Maßketten fehlen.
5. **Wandfang-Schalter in der Oberfläche**: `snap.wall_edges` existiert im Kern
   (Setting mit `#[serde(default)]`, `doc.settings {"snap":{"wall_edges":…}}`,
   Wirkung in `snap.rs`) — der Eintrag im Fang-Chip-Menü (`app.rs`) und im
   Einstellungen-Dialog (`dialogs.rs`) fehlt noch (Karte `t_89c0a0a1`).

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
