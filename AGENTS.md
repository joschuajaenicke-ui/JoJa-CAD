# JoJa-CAD v0.2 — Projektkontext für Agenten

**Lies dieses Dokument vollständig, bevor du eine Karte bearbeitest.** Es enthält
die verbindlichen Konventionen. Abweichungen gelten als Fehler.
(Gleicher Inhalt als Kopie: `docs/AGENTEN.md`.)

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
cargo test                          # muss grün sein
./scripts/build-live.sh             # Testbinary für den Nutzer, versioniert
```

Das Binary trägt eine Build-Nummer (Git-Commit-Zähler, `crates/jjcad-gui/build.rs`)
in Fenstertitel und Statusleiste.

### Build-Identität (verbindlich)

Die Build-Nummer ist der Commit-Zähler **von `master`** — auf einem Zweig hat sie
keine Bedeutung. Deshalb trägt jeder ausgelieferte Build zusätzlich die
Commit-Kurzform:

```
~/jjcad-live/jjcad-0.2.0-b27-81ec0c2      <- Name = Nummer + Commit
~/jjcad-live/jjcad                        <- Symlink auf den neuesten
~/jjcad-live/shipped.last                 <- zuletzt ausgelieferter Commit
```

`build-live.sh` bricht ab, wenn
- der Zweig nicht `master` ist,
- das Arbeitsverzeichnis ungesaubert ist,
- ein Build mit gleichem Namen schon existiert,
- der Stand **kein Nachfolger** des zuletzt ausgelieferten ist (Vorfahren-Prüfung;
  verhindert, dass ein älterer Zweig gelöste Fehler zurückbringt).

Testbuilds von Workern heißen nie so — sie bauen nur `cargo build --release` in
ihrem Worktree.

**`~/jjcad-live` gehört dem Manager, nicht den Workern.** Ein Worker baut seinen
Zweig mit `cargo build --release` und testet ihn headless; er ruft **nie**
`scripts/build-live.sh` auf. Sonst bekommt der Nutzer Builds aus einem
Feature-Zweig, in dem andere Fixes fehlen (schon passiert: b26/b27 aus
Worker-Zweigen). `build-live.sh` verweigert deshalb den Dienst außerhalb von
`master`.

## Wasserdichte Regeln (aus Schaden gelernt)

Diese fünf Regeln verhindern, dass gelöste Fehler wieder auftauchen:

1. **Nur ein Worker gleichzeitig.** `kanban.max_in_progress = 1`. Zwei parallele
   Worker haben schon einmal verschiedene Stände gleichzeitig gebaut und
   dieselbe Datei überschrieben.
2. **Isolierter Zweig pro Karte.** Jede Karte arbeitet in einem eigenen Worktree.
   Nie im Hauptverzeichnis, nie auf `master`: `git checkout -b fix/t_<id>`.
3. **Merge nur durch den Manager**, immer mit `scripts/merge-card.sh <zweig>` —
   das Skript rollt den Merge zurück, wenn `cargo test` rot wird.
4. **Jeder Fix bekommt einen Test.** Ohne Test kann ein Fehler unbemerkt
   zurückkommen; mit Test bricht der nächste Merge sofort. Das ist der einzige
   echte Regressionsschutz.
5. **Testkanal nur mit grünen Tests.** `scripts/build-live.sh` läuft `cargo test`
   und liefert bei Rot nicht aus; Builds aus Zweigen sind gesperrt (Vorfahren-
   Prüfung, Commit-Hash im Namen).

## Regeln für parallele Arbeit

1. **Nur im eigenen Workspace arbeiten** (Worktree bzw. zugewiesenes Verzeichnis).
   Nie in einen fremden Worktree schreiben.
2. **Niemals die GUI auf dem Bildschirm des Nutzers starten** (kein
   `DISPLAY=:0`, kein Start ohne Display-Angabe). Jedes Fenster, das auf seinem
   Schreibtisch aufpoppt, stört ihn beim Testen. Immer auf einem eigenen,
   unsichtbaren Xvfb-Display arbeiten:

   ```bash
   python3 scripts/qa-shot.py out.png          # startet Xvfb + App, macht Screenshot
   ./scripts/gui-headless.sh --shot out.png    # dito als Shell-Variante
   ```

   Auf dem Bildschirm des Nutzers erscheint ausschließlich, was er selbst über
   `scripts/start-live.sh` startet.
3. **Niemals `pkill -x jjcad`** oder ähnliche Muster-Kills — das beendet seine
   Instanz. Nur selbst gestartete Prozesse per PID beenden.
4. **Nicht mergen, nicht direkt auf `master` committen.** Arbeitest du im
   Hauptverzeichnis (Workspace = Projektordner), lege zuerst einen eigenen Zweig
   an: `git checkout -b fix/t_<karten-id>`. Arbeitest du in einem Worktree,
   committest du auf dessen Zweig.
5. **Kleine, prüfbare Schritte**: bauen, testen, dann committen. Kein Commit
   mit Compile-Fehlern.
6. **Debug-Dateien löschen** (`examples/dbg_*.rs`, temporäre Testdateien),
   bevor du abgibst.
7. Abschluss: Karte auf `request-review` setzen und den Kommentar **exakt nach
   der Vorlage** unten schreiben (Build, Testen, Erwartung).

## Verbindliche Designentscheidungen

Mit dem Nutzer abgestimmt — nicht „verbessern":

- **Türen** sitzen immer auf der *vorderen Wandkante* (Blattseite), nicht in der
  Wandmitte. Kein Auswahlfeld für die Tür-Lage. Fenster dürfen ihre Lage in
  der Wanddicke wählen (Links/Mittig/Rechts).
- **Dropdowns**: immer `panels::pick_combo` bzw. `panels::dropdown`
  (150 px Wertespalte, gleicher Pfeil). **Kein `egui::ComboBox`** — schließt bei
  jedem Klick im Popup und sieht anders aus.
- **Linienenden sind Halbkreise** (Kapsel-Rendering in `canvas.rs`), nicht Rechtecke.
- **Linienstärken**: zwei Modi, `px` (bildschirmkonstant) und `DPI`
  (maßstabsgetreu wie im Druck). Strichmuster folgt demselben Schalter.
- **Wandgehrung** über die *Innenwinkel*-Halbierende (`away - outward`);
  Füllungen vor allen Linien zeichnen, damit keine Umrandung überdeckt wird.
- **Auswahl** färbt nur blau ein — kein zusätzlicher Auswahlrahmen.
- **Raster-Chip** schaltet Anzeige *und* Rasterfang gemeinsam.
- **Ansichtsfenster** wie Rechtecke, beweglich nur mit `V`, exakte Eingabe X/Y/B/H.
- **Mauszeiger** ist ein durchgehendes Fadenkreuz bis zum Bildschirmrand.
- **Einfügen** (Block/Zwischenablage): Vorschau oben rechts am Zeiger, `R` dreht,
  `S` spiegelt, Klick bestätigt.
- **Rechtsklickmenüs** bleiben offen, wenn der Zeiger in die Zeichenfläche kommt.
- **Eigenschaften-Panel**: gleiche Zeilen (96 px Label, 150 px Wert) beim Platzieren
  *und* bei Auswahl; bei Mehrfachauswahl nur gemeinsame Felder, abweichende Werte
  als `*variiert*`; nicht editierbare Werte (Länge, Fläche) anzeigen.
- **Polarfang** 45° (Toleranz 6°); Fangmodi END, SCHN, MIT, ZEN, QUA, LOT, NÄC, RAS, POL.
- Wände: Farbe (Linie) und Füllung (Fläche) getrennt; Schraffuren inklusive
  **Dämmung** (Mäander); Ausrichtung Links/Mittig/Rechts.
- **Auswahl über Rechteck** prüft echte Geometrie (Segmente, Wandkörper),
  nicht die achsenparallele Bounding-Box.
- **Griffe** haben 8–12 px Trefferradius, geprüft gegen die *rohe* Zeigerposition
  (nicht den Fangpunkt).
- **Zieh-Modi**: Einstellungen → Bedienung → „Ziehen" (Gedrückthalten / Zwei
  Klicks), gespeichert in `prefs.json`, gilt für Auswahl *und* Griffe.

## Bericht an den Nutzer (Review)

Der Nutzer testet jedes Ergebnis selbst und will **keinen Roman**. Jeder
Abschluss-Kommentar an der Karte hat genau diese Form, maximal 6 Zeilen:

```
TEST b<N> — <eine Zeile: was jetzt anders ist>
Testen: <2–4 konkrete Schritte in der App>
Erwartung: <was man sehen muss>
```

Das Wort `TEST` am Anfang ist Pflicht: daran erkennt der Mensch, dass die Karte
in der Review-Spalte auf ihn wartet.

Beispiel:

```
Build b12 — Tür sitzt auf der vorderen Wandkante, Dropdown entfernt.
Testen: Wohnhaus.jjcad öffnen → Tür anklicken → Blatt Seite A/B umschalten.
Erwartung: Tür springt auf die jeweilige Wandaußenseite, kein Feld "Ausrichtung".
```

Keine Implementierungserklärungen, keine Zwischenschritte, keine Testprotokolle
in Prosa. Zahlen statt Adjektive.

## Abnahme-System (verbindlich)

Jede Spalte hat **genau eine** Bedeutung:

| Spalte | Bedeutung | Wer arbeitet |
|---|---|---|
| `todo` / `ready` | Warteschlange | Dispatcher |
| `running` | Ein Agent arbeitet | Agent |
| `review` | **Wartet auf den Menschen** (Joschua testet in der App) | niemand automatisch |
| `blocked` | Echter Blocker: Entscheidung nötig, Repro fehlt, 2 Fehlversuche | Mensch |
| `done` | Abgeschlossen | — |

Daraus folgen die Regeln:

- **`review` ist ausschließlich die Warteschlange des Menschen.** Karten dort
  werden von Joschua getestet. Kein Agent darf sie von dort wegbewegen — weder
  auf `done` noch auf `blocked`. Ein Prüf-Agent schreibt höchstens einen
  Kommentar (`Technisch geprüft: …`) und lässt den Status stehen.
- **`blocked` niemals als „wartet auf Test" verwenden.** `blocked` erzeugt eine
  Störung auf dem Board und muss manuell aufgelöst werden.
- **Selbst auf `done` setzen ist nur bei trivialen, eindeutig prüfbaren
  Änderungen erlaubt.** Zulässig:
  - Umbenennen von Beschriftungen, Menüeinträgen, Tooltips
  - Tippfehler, Kommentare, Dokumentation, Formatierung
  - Entfernen von Debug-Ausgaben
  - ein Wert/Text, den ein Test direkt prüft

  **Nicht** zulässig (immer `request-review`): Geometrie, Fang, Treffer-Tests,
  Rendering, Persistenz, Verhalten von Werkzeugen, Änderungen an der
  Bedienoberfläche, alles was man sehen oder anfassen muss.
- Bei Selbst-Abnahme: Karte auf `done` **und** ein Satz im Kommentar
  (`Selbst abgenommen: reine Umbenennung, Test X grün`) — damit nachvollziehbar
  bleibt, warum kein Mensch geprüft hat.

## Umgang mit Rückmeldungen
- **Nicht gelöst:** Kommentar an der *gleichen* Karte
  (`Nicht gelöst: <Symptom>, Build b<N>`), dann dort weiterarbeiten und den
  nächsten Build liefern.
- **Folgeproblem** (durch den Fix neu entstanden): gleiche Karte, Kommentar
  `Folgeproblem: <Symptom>`; Ursache neu suchen statt am alten Fix zu flicken.
- **Anderes Thema:** neue Karte, nicht in die laufende mischen.
- Nach `failure_limit` Fehlversuchen blockiert der Dispatcher die Karte — dann
  Ursache und offene Frage im Kommentar festhalten, nicht weiterprobieren.

## Abnahme (Definition of Done)

„Sieht gut aus" ist kein Nachweis. Jede Karte braucht:

1. `cargo build --release` ohne Fehler, `cargo test` grün.
2. Einen **messbaren** Nachweis: Test, geprüftes Feld in der gespeicherten
   `.jjcad`-Datei (z. B. `align: "left"`, Koordinaten) oder Pixelmaß.
3. Screenshot bei sichtbaren Änderungen, erzeugt mit
   `python3 scripts/qa-shot.py --demo "<zustand>" out.png` — niemals die Instanz
   des Nutzers dafür verwenden.
4. Kommentar an der Karte: Build-Nummer, Dateien, Nachweis, Testanleitung.

## Testergebnis des Nutzers

Der Nutzer testet `~/jjcad-live/jjcad` (Build-Nummer im Fenstertitel). Sein
Feedback kommt als Kommentar an die Karte oder als neue Karte. Bei Rückfragen
zur Geometrie hilft oft ein Screenshot mit eingezeichneter Hilfslinie.
