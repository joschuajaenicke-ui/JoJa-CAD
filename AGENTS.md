# JoJa-CAD v0.2 — Projektkontext für Agenten

**Lies dieses Dokument vollständig, bevor du eine Karte bearbeitest.** Es enthält
die verbindlichen Konventionen. Abweichungen gelten als Fehler.
(Gleicher Inhalt als Kopie: `docs/AGENTEN.md`.)

**Begriffe:** `docs/GLOSSAR.md` ist verbindlich. Ein Begriff, eine Bedeutung —
Tickets, Kommentare, UI-Texte und Code-Kommentare benutzen die dortige linke
Spalte. Abweichende Wörter gehören nicht in die Oberfläche.

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

Das Binary trägt eine Build-Nummer in Fenstertitel und Statusleiste
(`crates/jjcad-gui/build.rs`).

### Build-Identität (verbindlich)

Ein Buildname besteht aus Version, **Auslieferungszähler** `s<N>` und
Commit-Kurzform:

```
~/jjcad-live/jjcad-0.2.0-s4-ab12cd3      <- Zähler + Commit
~/jjcad-live/jjcad                       <- Symlink auf den neuesten
~/jjcad-live/shipped.last                <- zuletzt ausgelieferter Commit + Zähler
```

Nicht der Commit-Zähler: zwei verschiedene Stände ergäben sonst dieselbe Nummer.

`build-live.sh` bricht ab, wenn
- der Zweig nicht `master` ist,
- das Arbeitsverzeichnis ungesaubert ist,
- ein Build mit gleichem Namen schon existiert,
- der Stand **kein Nachfolger** des zuletzt ausgelieferten ist (Vorfahren-Prüfung;
  verhindert, dass ein älterer Zweig gelöste Fehler zurückbringt).

### Ausliefern ist Teil der Karte (genau ein Befehl)

Jeder Build entsteht **aus `master`** — nur so enthält er alle vorherigen Fixes.
Deshalb liefert der Worker seine Karte selbst aus:

```bash
./scripts/ship-card.sh t_1234abcd
```

Das Skript
- verweigert die Auslieferung, wenn der Zweig den aktuellen `master` nicht enthält
  (dann erst im Worktree `git merge master`),
- merged den Karten-Zweig nach `master` (`--no-ff`; Tests müssen grün sein, sonst
  automatischer Rückbau),
- baut `master` und legt den Build in `~/jjcad-live/` ab,
- gibt die Kennung `s<N>-<hash>` aus, die in den Review-Kommentar gehört.

Ohne diese Kennung geht keine Karte nach `review`. `build-live.sh` nie direkt
aufrufen, nie aus einem Feature-Zweig bauen.


## Wasserdichte Regeln (aus Schaden gelernt)

Diese fünf Regeln verhindern, dass gelöste Fehler wieder auftauchen:

1. **Nur ein Worker gleichzeitig.** Erzwungen vom Dispatcher über
   `kanban.max_in_progress = 1` (gatewayweit, gilt für alle Boards). Zwei parallele
   Worker haben schon einmal verschiedene Stände gleichzeitig gebaut und dieselbe
   Datei überschrieben.
2. **Isolierter Zweig pro Karte.** Jede Karte arbeitet in einem eigenen Worktree
   (`wt/t_<id>`). Nie im Hauptverzeichnis, nie auf `master` committen:
   `git checkout -b fix/t_<id>`.
3. **Merge nur über `scripts/ship-card.sh <karte>`** — es merged mit `--no-ff`,
   prüft die Tests und rollt bei Rot zurück. Kein `git merge` von Hand auf
   `master`, kein `git checkout master` im Hauptverzeichnis.
4. **Jeder Fix bekommt einen Test.** Ohne Test kann ein Fehler unbemerkt
   zurückkommen; mit Test bricht der nächste Merge sofort. Das ist der einzige
   echte Regressionsschutz.
5. **Testkanal nur aus `master` und nur mit grünen Tests.** `build-live.sh` läuft
   `cargo test` und liefert bei Rot nicht aus; Builds aus Zweigen sind gesperrt
   (Vorfahren-Prüfung, Commit-Kurzform im Namen).

## Regeln für parallele Arbeit

1. **Erster Schritt: auf den aktuellen master ziehen.** Bevor du eine Zeile
   änderst: `git merge master` (oder `git rebase master`) im eigenen Workspace.
   Ein Worktree kann von einem früheren Lauf stammen und dann auf veraltetem
   Stand stehen — dann baust du alte Fehler wieder ein. Prüfe danach:
   `git merge-base --is-ancestor master HEAD && echo aktuell`.
   Dann **nur im eigenen Workspace** arbeiten, nie in einem fremden Worktree.
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
7. **Handoff zuerst, Politur danach.** Ein Worker hat ein festes
   Iterationsbudget (150). Sobald der Fix committet ist und `cargo test` grün
   läuft: **sofort** `request-review` mit dem Bericht nach Vorlage. Screenshots,
   Doku oder Gegenproben danach als Kommentar nachreichen. Sonst geht das Budget
   für Nebensächliches drauf und der Lauf endet ohne Übergabe (schon passiert:
   t_9e6eb560, 150/150 Iterationen, Arbeit fertig, Handoff fehlte).

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

Der Nutzer testet jedes Ergebnis selbst und will **keinen Roman**. Der
Abschluss-Kommentar an der Karte hat genau diese Form, maximal 6 Zeilen, und
beginnt mit der Kennung, die `scripts/ship-card.sh` ausgegeben hat:

```
TEST s<N>-<hash> — <eine Zeile: was jetzt anders ist>
Umgesetzt: <2–4 Stichpunkte, was genau geändert wurde>
Testen: <2–4 konkrete Schritte in der App>
Erwartung: <was man sehen muss>
```

Das Wort `TEST` am Anfang ist Pflicht: daran erkennt der Mensch, dass die Karte
in der Review-Spalte auf ihn wartet.

Reihenfolge, nicht verhandelbar:

1. Arbeit committen, `cargo test` grün.
2. `./scripts/ship-card.sh <karte>` → Build liegt in `~/jjcad-live/`, Kennung notieren.
3. **Erst dann** `request-review` mit obigem Kommentar.

Eine Karte in `review` ohne `TEST s<N>`-Stempel ist ein Fehler: sie ist dann in
keinem Build und der Nutzer würde ins Leere testen. **Nur der jüngste Kommentar
zählt** — nach einer Nacharbeit ist der alte Stempel ungültig; dann neu ausliefern
und `GÜLTIG: TEST s<N>-<hash>` schreiben.

Beispiel:

```
TEST s12-4f2a9cd — Tür sitzt auf der vorderen Wandkante, Dropdown entfernt.
Umgesetzt: Türanker auf Blattseite, Auswahlfeld "Ausrichtung" entfallen, Test tuer_kante.
Testen: Wohnhaus.jjcad öffnen → Tür anklicken → Blatt Seite A/B umschalten.
Erwartung: Tür springt auf die jeweilige Wandaußenseite, kein Feld "Ausrichtung".
```

Keine Implementierungserklärungen, keine Zwischenschritte, keine Testprotokolle
in Prosa. Zahlen statt Adjektive.

**Jeder TEST-Kommentar muss einen Prüfweg enthalten, den der Mensch in zwei
Minuten gehen kann.** Bei sichtbaren Änderungen: Klickschritte. Bei rein
technischen Änderungen ohne Bedienelement (Speicherformat, Rendering-Grundlage):
eine **Beispieldatei im Repo** (`samples/…jjcad`) oder ein MCP-Befehl, mit dem
man das Ergebnis sieht. Ohne Prüfweg ist der Bericht wertlos — „fertig" ist dann
nicht überprüfbar.

## Abnahme-System (verbindlich)

Jede Spalte hat **genau eine** Bedeutung:

| Spalte | Bedeutung | Wer arbeitet |
|---|---|---|
| `triage` | Neue Meldung, noch nicht arbeitsfähig | Automatik (`specify`), alle 5 min |
| `todo` / `ready` | Warteschlange | Dispatcher, **immer nur eine Karte** |
| `running` | Ein Agent arbeitet | Agent |
| `review` | **Wartet auf den Menschen**: Build ist ausgeliefert, Joschua testet in der App | niemand automatisch |
| `blocked` | Echter Blocker: Entscheidung nötig, Repro fehlt, 2 Fehlversuche | Mensch |
| `done` | Abgeschlossen | — |

Daraus folgen die Regeln:

- **Strikt seriell.** `kanban.max_in_progress = 1` im Dispatcher: höchstens eine
  Karte in `running`, egal wie viele in `ready` warten. Nicht per Skript und nicht
  durch Zuweisungs-Trickserei „optimieren".

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
  (`Nicht gelöst: <Symptom>, Build s<N>-<hash>`), dann dort weiterarbeiten und den
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
4. Kommentar an der Karte: Build-Kennung `s<N>-<hash>`, Dateien, Nachweis, Testanleitung.

## Testergebnis des Nutzers

Der Nutzer testet `~/jjcad-live/jjcad` (Build-Nummer im Fenstertitel). Sein
Feedback kommt als Kommentar an die Karte oder als neue Karte. Bei Rückfragen
zur Geometrie hilft oft ein Screenshot mit eingezeichneter Hilfslinie.
