# JoJa-CAD v0.2 — Projektkontext für Agenten

**Lies dieses Dokument vollständig, bevor du eine Karte bearbeitest.** Es enthält
die verbindlichen Konventionen. Abweichungen gelten als Fehler.

## Worum es geht

2D-CAD für Architektur- und Bauantragszeichnungen, Rust-Workspace:

- `crates/jjcad-core` — Geometrie (`geom`), Modell (`model`), Befehle (`actions`),
  Displayliste (`render`), PDF/DXF-Schreiber, Datei-Ein-/Ausgabe (`persist`)
- `crates/jjcad-gui` — eframe/egui 0.29: Ribbon, Hotbar, Inventar, Panels, Canvas
- `crates/jjcad-mcp` — JSON-RPC-Server (MCP) auf derselben Befehlsschicht

Architekturregeln:

- **Eine Befehlsschicht**: jede Funktion ist eine Action (`draw.wall`, `modify.set`, …)
  in `actions.rs`. GUI, Tastatur, Hotbar und MCP rufen denselben Code auf — nie
  einen zweiten Weg an der Action-Schicht vorbei bauen.
- **Eine Displayliste** (`render::DisplayList`) ist die einzige Renderquelle für
  Bildschirm, PDF und PNG. Neue Linien/Füllungen dort erzeugen, nicht im Painter.
- Interne Einheit **Millimeter**, Winkel im Bogenmaß, Nullrichtung +X, mathematisch
  positiver Drehsinn. Ausgabe deutsch (Komma), `m²`.
- **Oberfläche und Code-Kommentare auf Deutsch.**

## Bauen, testen, ausliefern

```bash
cargo build -p jjcad-gui            # Debug
cargo build --release -p jjcad-gui  # Release
cargo test                          # muss gruen sein
./scripts/build-live.sh             # Testbinary fuer den Nutzer, versioniert
```

Das Binary traegt eine Build-Nummer (Git-Commit-Zaehler, `crates/jjcad-gui/build.rs`)
in Fenstertitel und Statusleiste. `build-live.sh` legt
`~/jjcad-live/jjcad-<version>-b<nummer>` an und aktualisiert den Symlink `jjcad`.

## Regeln fuer parallele Arbeit

1. **Nur im eigenen Workspace arbeiten** (Worktree bzw. zugewiesenes Verzeichnis).
   Nie in einen fremden Worktree schreiben.
2. **Niemals `pkill -x jjcad`** oder aehnliche Muster-Kills — das beendet die
   Instanz des Nutzers. Nur selbst gestartete Prozesse per PID beenden.
   Fuer Screenshots `python3 scripts/qa-shot.py` benutzen (eigenes Xvfb).
3. **Nicht mergen, nicht direkt auf `master` committen.** Arbeitest du im
   Hauptverzeichnis (Workspace = Projektordner), lege zuerst einen eigenen Zweig
   an: `git checkout -b fix/t_<karten-id>`. Arbeitest du in einem Worktree,
   committest du auf dessen Zweig. Ein Commit pro Karte, Nachricht mit Karten-ID:
   `t_xxxx: <was geaendert wurde>`.
4. **Kleine, pruefbare Schritte**: bauen, testen, dann committen. Kein Commit
   mit Compile-Fehlern.
5. **Debug-Dateien loeschen** (`examples/dbg_*.rs`, temporaere Testdateien),
   bevor du abgibst.
6. Abschluss: Karte kommentieren mit Build-Nummer, geaenderten Dateien,
   Nachweis und dem, was der Nutzer testen soll.

## Verbindliche Designentscheidungen

Mit dem Nutzer abgestimmt — nicht „verbessern":

- **Tueren** sitzen immer auf der *vorderen Wandkante* (Blattseite), nicht in der
  Wandmitte. Kein Auswahlfeld fuer die Tuer-Lage. Fenster duerfen ihre Lage in
  der Wanddicke waehlen (Links/Mittig/Rechts).
- **Dropdowns**: immer `panels::pick_combo` bzw. `panels::dropdown`
  (150 px Wertespalte, gleicher Pfeil). **Kein `egui::ComboBox`** — schliesst bei
  jedem Klick im Popup und sieht anders aus.
- **Linienenden sind Halbkreise** (Kapsel-Rendering in `canvas.rs`), nicht Rechtecke.
- **Linienstaerken**: zwei Modi, `px` (bildschirmkonstant) und `DPI`
  (maßstabsgetreu wie im Druck). Strichmuster folgt demselben Schalter.
- **Wandgehrung** ueber die *Innenwinkel*-Halbierende (`away - outward`);
  Fuellungen vor allen Linien zeichnen, damit keine Umrandung ueberdeckt wird.
- **Auswahl** faerbt nur blau ein — kein zusaetzlicher Auswahlrahmen.
- **Raster-Chip** schaltet Anzeige *und* Rasterfang gemeinsam.
- **Ansichtsfenster** wie Rechtecke, beweglich nur mit `V`, exakte Eingabe X/Y/B/H.
- **Mauszeiger** ist ein durchgehendes Fadenkreuz bis zum Bildschirmrand.
- **Einfuegen** (Block/Zwischenablage): Vorschau oben rechts am Zeiger, `R` dreht,
  `S` spiegelt, Klick bestaetigt.
- **Rechtsklickmenues** bleiben offen, wenn der Zeiger in die Zeichenflaeche kommt.
- **Eigenschaften-Panel**: gleiche Zeilen (96 px Label, 150 px Wert) beim Platzieren
  *und* bei Auswahl; bei Mehrfachauswahl nur gemeinsame Felder, abweichende Werte
  als `*variiert*`; nicht editierbare Werte (Laenge, Flaeche) anzeigen.
- **Polarfang** 45° (Toleranz 6°); Fangmodi END, SCHN, MIT, ZEN, QUA, LOT, NAEC, RAS, POL.
- Wande: Farbe (Linie) und Fuellung (Flaeche) getrennt; Schraffuren inklusive
  **Daemmung** (Maender); Ausrichtung Links/Mittig/Rechts.
- **Auswahl ueber Rechteck** prueft echte Geometrie (Segmente, Wandkoerper),
  nicht die achsenparallele Bounding-Box.
- **Griffe** haben 8–12 px Trefferradius, geprueft gegen die *rohe* Zeigerposition
  (nicht den Fangpunkt).

## Abnahme (Definition of Done)

„Sieht gut aus" ist kein Nachweis. Jede Karte braucht:

1. `cargo build --release` ohne Fehler, `cargo test` gruen.
2. Einen **messbaren** Nachweis: Test, geprueftes Feld in der gespeicherten
   `.jjcad`-Datei (z. B. `align: "left"`, Koordinaten) oder Pixelmaß.
3. Screenshot bei sichtbaren Aenderungen, erzeugt mit
   `python3 scripts/qa-shot.py --demo "<zustand>" out.png` — niemals die Instanz
   des Nutzers dafuer verwenden.
4. Kommentar an der Karte: Build-Nummer, Dateien, Nachweis, Testanleitung.

## Testergebnis des Nutzers

Der Nutzer testet `~/jjcad-live/jjcad` (Build-Nummer im Fenstertitel). Sein
Feedback kommt als Kommentar an die Karte oder als neue Karte. Bei Rueckfragen
zur Geometrie hilft oft ein Screenshot mit eingezeichneter Hilfslinie.
