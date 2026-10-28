# JoJa-CAD v0.2 — Projektkontext für Agenten

**Lies dieses Dokument vollständig, bevor du ein Ticket bearbeitest.** Es enthält
die verbindlichen Konventionen. Abweichungen gelten als Fehler.
(Gleicher Inhalt als Kopie: `docs/AGENTEN.md`.)

**Arbeitsablauf:** Tickets, Kategorien, Arbeitsplatz und Berichtsweg stehen in
`~/Ticketbox/AGENTS.md` — dieses Dokument ergänzt es um die Projektregeln.
Ticketbox löst Kanban ab; beides zusammen ist widerspruchsfrei.

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

### Ausliefern ist Teil des Tickets (genau ein Befehl)

Jeder Build entsteht **aus `master`** — nur so enthält er alle vorherigen Fixes.
Deshalb liefert der Worker sein Ticket selbst aus, in seinem Worktree:

```bash
./scripts/ship-card.sh T0042-cursor-polarfang
```

Das Skript läuft unter einer Sperre (`flock`): zwei Auslieferungen können sich
nicht verschränken. Kennungen beider Formen werden akzeptiert — `T0042-…` für
Ticketbox, `t_1234abcd` historisch für alte Karten.

Es
- verweigert die Auslieferung, wenn der Zweig den aktuellen `master` nicht enthält
  (dann erst im Worktree `git merge master`),
- merged den Ticket-Zweig nach `master` (`--no-ff`; Tests müssen grün sein, sonst
  automatischer Rückbau),
- baut `master` und legt den Build in `~/jjcad-live/` ab,
- gibt die Kennung `s<N>-<hash>` aus, die in `ergebnis.build` gehört.

Ohne diese Kennung geht kein Ticket nach „Überprüfung“: der Nutzer wüsste nicht,
welchen Build er testen soll. `build-live.sh` nie direkt aufrufen, nie aus einem
Feature-Zweig bauen.


## Wasserdichte Regeln (aus Schaden gelernt)

Diese fünf Regeln verhindern, dass gelöste Fehler wieder auftauchen:

1. **Nur ein Worker gleichzeitig.** Die Ticketbox startet genau einen Lauf,
   solange „Bearbeitung“ belegt ist (Auto-Modus). Zwei parallele Worker haben
   schon einmal verschiedene Stände gleichzeitig gebaut und dieselbe Datei
   überschrieben. Manuell in „Bearbeitung“ geschobene Tickets sind die einzige
   Ausnahme — dann gilt Regel 2 umso strenger.
2. **Isolierter Zweig pro Ticket.** Jedes Ticket arbeitet in seinem eigenen
   Worktree (`.worktrees/T0042-<slug>`, Zweig `fix/T0042-<slug>`), den die
   Ticketbox vor dem Start anlegt und auf den aktuellen `master` zieht. Nie im
   Hauptverzeichnis, nie auf `master` committen.
3. **Merge nur über `scripts/ship-card.sh <Ticket-Kennung>`** — es merged mit
   `--no-ff`, prüft die Tests und rollt bei Rot zurück. Kein `git merge` von Hand
   auf `master`, kein `git checkout master` im Hauptverzeichnis.
4. **Jeder Fix bekommt einen Test.** Ohne Test kann ein Fehler unbemerkt
   zurückkommen; mit Test bricht der nächste Merge sofort. Das ist der einzige
   echte Regressionsschutz.
5. **Testkanal nur aus `master` und nur mit grünen Tests.** `build-live.sh` läuft
   `cargo test` und liefert bei Rot nicht aus; Builds aus Zweigen sind gesperrt
   (Vorfahren-Prüfung, Commit-Kurzform im Namen).

## Regeln für parallele Arbeit

1. **Erster Schritt: Stand prüfen.** Bevor du eine Zeile änderst:
   `git merge-base --is-ancestor master HEAD && echo aktuell` im eigenen
   Worktree. Nicht aktuell → `git merge master` (Konflikte lösen, Tests grün),
   erst dann arbeiten. Die Ticketbox zieht den Worktree vor dem Start selbst
   nach; ein von Hand angelegter Arbeitsplatz kann trotzdem veraltet sein.
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
4. **Nicht mergen, nicht direkt auf `master` committen.** Du arbeitest immer in
   einem Ticket-Worktree und committest auf dessen Zweig. Ohne Worktree (z. B.
   bei einer Nachfrage im Hauptprofil): erst `git checkout -b fix/T<NNNN>-<slug>`.
5. **Kleine, prüfbare Schritte**: bauen, testen, dann committen. Kein Commit
   mit Compile-Fehlern.
6. **Debug-Dateien löschen** (`examples/dbg_*.rs`, temporäre Testdateien),
   bevor du abgibst.
7. **Bericht zuerst, Politur danach.** Sobald der Fix committet ist,
   `cargo test` grün läuft und ausgeliefert ist: **sofort** `ergebnis` in der
   `ticket.json` schreiben. Screenshots, Doku oder Gegenproben danach als
   Nacharbeit am Ticket. Sonst geht das Zeitbudget für Nebensächliches drauf und
   der Lauf endet ohne Übergabe (schon passiert: t_9e6eb560, 150/150
   Iterationen, Arbeit fertig, Übergabe fehlte).

## Verbindliche Designentscheidungen

Mit dem Nutzer abgestimmt — nicht „verbessern“:

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
- **Auswahl** färbt nur blau ein — kein zusätzlicher Auswahlrahmen. **Ausnahme Text**:
  der Text hat einen unsichtbaren Rahmen (derselbe Kasten wie in der Einfüge-Vorschau), der
  bei Auswahl gezeichnet wird und dessen Ecken/Kantenmitten fangen. Text wird **nicht** mit
  der gedrückten Maustaste verschoben — nur über das Werkzeug „Verschieben“. Der Kasten ist
  die Zeilenbox (`render::text_frame`: Höhe × 1,4, Breite nach Zeichenbreiten plus Rand), und
  die Vorschau des Textwerkzeugs läuft über `canvas::paint_text` — dieselbe Platzierung wie
  der eingefügte Text (links, mittig), nicht mit der Grundlinie am Zeiger.
- **Raster-Chip** schaltet Anzeige *und* Rasterfang gemeinsam.
- **Ansichtsfenster** wie Rechtecke, beweglich nur mit `V`, exakte Eingabe X/Y/B/H.
- **Druckvorschau = Papierraum**: gleiche Werkzeuge wie im Modell. Der Bearbeitungsraum
  steckt in `Document::space` (`None` = Modell, `Some(Blatt)` = Papierraum); alle Actions
  laufen über `doc.ents()`/`doc.ents_mut()`. Rahmen und Schriftfeld sind normale Linien und
  Textfelder in `sheet.entities` (kein Renderer-Sonderrahmen mehr, `Sheet::frame`/
  `show_title_block` sind nur noch Altlasten für die Migration). Die echten Blattecken
  (0,0)…(B,H) fangen im Papierraum als „Blattecke“.
- **Mauszeiger** ist ein durchgehendes Fadenkreuz bis zum Bildschirmrand.
- **Einfügen** (Block/Zwischenablage): Vorschau oben rechts am Zeiger, `R` dreht,
  `S` spiegelt, Klick bestätigt. Die Zwischenablage trägt ihren **Basispunkt** mit:
  „Kopieren mit Basispunkt" nimmt den geklickten Punkt, „Kopieren" automatisch die
  untere linke Ecke der Auswahl — beim Einfügen steht dieser Punkt am Zeiger.
  **Kopieren setzt nie sofort eine Kopie ab** (auch nicht mit Basispunkt): es füllt
  nur die Zwischenablage, platziert wird über die Einfüge-Vorschau.
- **Rechtsklickmenüs** bleiben offen, wenn der Zeiger in die Zeichenfläche kommt.
- **Eigenschaften-Panel**: gleiche Zeilen (96 px Label, 150 px Wert) beim Platzieren
  *und* bei Auswahl; bei Mehrfachauswahl nur gemeinsame Felder, abweichende Werte
  als `*variiert*`; nicht editierbare Werte (Länge, Fläche) anzeigen.
- **Polarfang** 45° (Toleranz 6°); Fangmodi END, SCHN, MIT, ZEN, QUA, LOT, NÄC, RAS, POL.
- **Erweiterter Punktfang**: bleibt der Zeiger 400 ms auf einem diskreten Fangpunkt stehen
  (ohne Klick), wird dieser *gehalten* und bekommt zwei Hilfslinien — bei einem Punkt auf
  einer Linie Verlängerung und Normale, sonst Waagerechte und Senkrechte. Auf ihnen fängt
  der nächste Punkt (`SnapKind::Tracking`, Statuszeile „Hilfslinie"), ebenso der
  **Schnittpunkt einer Hilfslinie mit einer bestehenden Linie** (Statuszeile „Schnittpunkt");
  höchstens zwei Haltepunkte, ein Klick löst sie. Diskrete Objektfänge (END, MIT, …) schlagen die
  Hilfslinie, die Hilfslinie schlägt Nächster Punkt/Lot sowie Ortho, Polar und Raster.
- **Chat**: Meldungen stehen unten links in der Zeichenfläche und verblassen
  nach einigen Sekunden; `^` (die Taste unter ESC) öffnet das Protokoll im
  selben Bild — Meldungen laufen nach oben, unten die Eingabezeile mit Fokus,
  Mausrad blättert, die Zeichnung wird leicht blass. `^`/`ESC` schließen es,
  Enter schickt die Eingabe ab und schließt ebenfalls (sie steht dann als Zeile
  `› …` im Chat). Eingaben des Nutzers werden nur protokolliert. Es gibt keinen eigenen Toast
  mehr; das Text-Werkzeug hat kein Kürzel. Bei offenem Protokoll bleibt der
  Mauszeiger sichtbar (in der Eingabezeile als Textcursor).
- **Chat-Schema** (eine Form für alle Funktionen): `<Werkzeug> · <Schritt>` als
  Aufforderung (`Wand · Ende klicken`), `<Objekt> · <Zustand>` als Ergebnis
  (`Wand · gesetzt`, `Auswahl · kopiert`), `<Werkzeug> · <Grund>` als Abbruch
  (`Tür · nur auf einer Wand`). Kein Satz, keine Erklärung, höchstens 48 Zeichen
  — `app::tests_chat_texte` prüft Aufforderung und Ergebnis für jedes Werkzeug.
- Wände: Farbe (Linie) und Füllung (Fläche) getrennt; Schraffuren inklusive
  **Dämmung** (Mäander); Ausrichtung Links/Mittig/Rechts.
- **Auswahl über Rechteck** prüft echte Geometrie (Segmente, Wandkörper),
  nicht die achsenparallele Bounding-Box.
- **Griffe** haben 8–12 px Trefferradius, geprüft gegen die *rohe* Zeigerposition
  (nicht den Fangpunkt).
- **Zieh-Modi**: Einstellungen → Bedienung → „Ziehen“ (Gedrückthalten / Zwei
  Klicks), gespeichert in `prefs.json`, gilt für Auswahl *und* Griffe.

## Bericht an den Nutzer (Ergebnis im Ticket)

Der Nutzer testet jedes Ergebnis selbst und will **keinen Roman**. Das Feld
`ergebnis` in `~/Ticketbox/tickets/<Ticket>/ticket.json` hat genau diese Form,
maximal 6 Zeilen, und beginnt mit der Kennung aus `ship-card.sh`:

```
TEST s<N>-<hash> — <eine Zeile: was jetzt anders ist>
Umgesetzt: <2–4 Stichpunkte, was genau geändert wurde>
Testen: <2–4 konkrete Schritte in der App>
Erwartung: <was man sehen muss>
```

Das Wort `TEST` am Anfang ist Pflicht: daran erkennt der Mensch, welchen Build er
in der App testet. Die volle Kennung (`jjcad-0.2.0-s<N>-<hash>`) gehört in
`ergebnis.build`.

Reihenfolge, nicht verhandelbar:

1. Arbeit committen, `cargo test` grün.
2. `./scripts/ship-card.sh <Ticket-Kennung>` → Build liegt in `~/jjcad-live/`,
   Kennung notieren.
3. **Erst dann** `ergebnis` schreiben (`build`, `gemacht`, `belege`, `offen`) und
   `fertig` setzen. Die Ticketbox schiebt das Ticket nach „Überprüfung“.

Ein Ticket in „Überprüfung“ ohne `ergebnis.build` ist ein Fehler: es ist in
keinem Build und der Nutzer würde ins Leere testen. **Nur das jüngste Ergebnis
zählt** — nach einer Nacharbeit ist der alte Stand ungültig; dann neu ausliefern
und den neuen Build nennen.

Beispiel:

```
TEST s12-4f2a9cd — Tür sitzt auf der vorderen Wandkante, Dropdown entfernt.
Umgesetzt: Türanker auf Blattseite, Auswahlfeld "Ausrichtung" entfallen, Test tuer_kante.
Testen: Wohnhaus.jjcad öffnen → Tür anklicken → Blatt Seite A/B umschalten.
Erwartung: Tür springt auf die jeweilige Wandaußenseite, kein Feld "Ausrichtung".
```

Keine Implementierungserklärungen, keine Zwischenschritte, keine Testprotokolle
in Prosa. Zahlen statt Adjektive.

**Jedes Ergebnis muss einen Prüfweg enthalten, den der Mensch in zwei Minuten
gehen kann.** Bei sichtbaren Änderungen: Klickschritte. Bei rein technischen
Änderungen ohne Bedienelement (Speicherformat, Rendering-Grundlage): eine
**Beispieldatei im Repo** (`samples/…jjcad`) oder ein MCP-Befehl, mit dem man
das Ergebnis sieht. Ohne Prüfweg ist der Bericht wertlos — „fertig“ ist dann
nicht überprüfbar.

## Abnahme-System (verbindlich)

Jede Kategorie hat **genau eine** Bedeutung:

| Kategorie | Bedeutung | Wer arbeitet |
|---|---|---|
| Eingang | Warteschlange | Ticketbox, **immer nur ein Ticket** (Auto-Modus) |
| Bearbeitung | Ein Agent arbeitet | Agent (genau ein Lauf) |
| Überprüfung | **Wartet auf den Menschen**: Build ist ausgeliefert, Joschua testet in der App | niemand automatisch |
| Abgeschlossen | Vom Nutzer abgenommen | — |

Daraus folgen die Regeln:

- **Strikt seriell.** Höchstens ein Ticket in „Bearbeitung“, egal wie viele im
  Eingang warten. Nicht per Skript und nicht durch Zuweisungs-Trickserei
  „optimieren“.
- **„Überprüfung“ ist ausschließlich die Warteschlange des Menschen.** Kein
  Agent bewegt ein Ticket von dort weg. Den Wechsel setzt die Ticketbox selbst,
  sobald Prozessende und `ergebnis`/`fertig` vorliegen.
- **Blockade ist kein Wartezustand.** Blockiert heißt:
  `ergebnis.offen = "blockiert: <Grund>"`; über Nacharbeit entscheidet der Nutzer.
- **Keine Selbstabnahme.** Nur der Nutzer entscheidet „Abschließen“ oder
  „Nachforderung“ — auch bei Umbenennungen und Tippfehlern.

## Umgang mit Rückmeldungen

- **Nicht gelöst:** Der Nutzer fordert Nacharbeit an (Prüfvermerk). Das Ticket
  kommt mit derselben Session zurück in „Bearbeitung“, der Vermerk steht in der
  Aufgabenstellung. Ursache neu suchen statt am alten Fix zu flicken.
- **Folgeproblem** (durch den Fix neu entstanden): ebenfalls Nacharbeit am selben
  Ticket, im Bericht als Folgeproblem benennen.
- **Anderes Thema:** neues Ticket, nicht in das laufende mischen.
- Nach dem zweiten erfolglosen Anlauf: `ergebnis.offen = "blockiert: <Ursache und
  offene Frage>"` statt weiterzuprobieren.

## Abnahme (Definition of Done)

„Sieht gut aus“ ist kein Nachweis. Jedes Ticket braucht:

1. `cargo build --release` ohne Fehler, `cargo test` grün.
2. Einen **messbaren** Nachweis: Test, geprüftes Feld in der gespeicherten
   `.jjcad`-Datei (z. B. `align: "left"`, Koordinaten) oder Pixelmaß.
3. Screenshot bei sichtbaren Änderungen, erzeugt mit
   `python3 scripts/qa-shot.py --demo "<zustand>" out.png` — niemals die Instanz
   des Nutzers dafür verwenden.
4. `ergebnis` im Ticket: Build-Kennung, betroffene Dateien, Nachweis, Testanleitung.

## Testergebnis des Nutzers

Der Nutzer testet `~/jjcad-live/jjcad` (Build-Nummer im Fenstertitel). Sein
Feedback kommt als Nacharbeit am Ticket (mit Prüfvermerk) oder als neues Ticket.
Bei Rückfragen zur Geometrie hilft oft ein Screenshot mit eingezeichneter
Hilfslinie.
