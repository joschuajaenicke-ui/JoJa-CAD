# JoJa-CAD

**2D-CAD für Architektur und Bauantragszeichnungen.** Bewusst minimal, kostenlos,
quelloffen — für Linux und Windows.

![JoJa-CAD Oberfläche](samples/ui.png)

*Die Demo-Zeichnung, die mit dem Programm ausgeliefert wird — Wände mit Öffnungen,
Treppe, Bemaßung, Möbelblöcke; links die Layer, rechts die Eigenschaften, unten die Hotbar.*

## Die Idee

Wer für einen Bauantrag zeichnet, braucht kein Modellierprogramm mit tausend Werkzeugen,
sondern ein sauberes Werkzeug für Grundriss, Schnitt und Blatt: Wände, Öffnungen,
Bemaßung, Blöcke, Papierformat — und sonst nichts.

- **Fokus Architektur und Bauantragszeichnung.** DIN-orientierte Bemaßung, Blätter mit
  Rahmen und Schriftfeld, Maßstäbe, Linienstärken, die im Druck stimmen.
- **Bewusst minimal.** Reduziert auf das, was eine Bauvorlage braucht. Keine Einstellungs-
  wüste, keine versteckten Modi, keine Funktion, die niemand erklären kann.
- **Kostenlos und quelloffen** — GPL-3.0-or-later, Dateiformat offen (JSON), kein Konto, keine
  Cloud, keine Abhängigkeit von einem Hersteller.
- **Linux und Windows.** Ein Rust-Programm ohne plattformspezifische Eigenheiten.
- **Offen für KI-Steuerung.** Der komplette Befehlssatz liegt als MCP-Server vor —
  ein KI-Agent kann zeichnen, ändern und exportieren, headless und ohne GUI.

**Was es bewusst nicht ist:** kein 3D, kein Rendering, kein BIM-/IFC-Datenmodell, kein
DWG-Import (proprietär). DXF ist der offene Weg — und der ist drin.

## Wichtigste Funktionen

| | |
|---|---|
| **Grundelemente** | Punkt, Linie, Polylinie, Rechteck, Kreis, Bogen, Ellipse, Schraffur, Text — mit Griffen, Fangen, Ortho und 45°-Polarfang |
| **Architektur** | Wand mit Dicke, Ausrichtung und Öffnungen; Tür (Dreh-, Schiebe-, zweiflügelig, Öffnung), Fenster, Treppe, Volumen — die Bauteile **verbinden sich dynamisch**: Gehrung an Wandanschlüssen, Öffnungen schneiden die Wand, Türen sitzen auf der Blattseite |
| **Bemaßung** | Einzelmaß, Maßkette, Grundriss- und Schnitt-Kote nach DIN 406/1356 — Schrägstrich- oder Pfeilmarken, Maßtext in mm/cm/m, Bezugsmaßstab 1:N frei einstellbar |
| **Blöcke** | umfangreiche Systembibliothek (Sanitär, Küche, Möbel, Symbole), eigene Projektblöcke aus der Auswahl, Instanzen mit Drehung/Spiegelung, Zerlegen, ByBlock/ByLayer — erweiterbar über Inventar und Hotbar-Vorlagen |
| **Austausch** | **PDF** (vektoriell, mit Schriftfeld) und **PNG** schreiben, **PDF importieren** (Grundriss als Zeichnung), **DXF** R12 lesen und schreiben |
| **KI-Steuerung** | MCP-Server mit 10 Werkzeugen (`jjcad_document`, `_execute`, `_batch`, `_query`, `_export`, `_render`, `_sheet`, `_actions`, `_undo`, `_redo`) — vollständig headless, inklusive Bild-Rückgabe; jede Funktion der Oberfläche ist auch über MCP erreichbar |

Ein Renderer für alles: Bildschirm, PDF und PNG entstehen aus derselben Displayliste —
Vorschau, Ausdruck und Zeichnung können nicht auseinanderlaufen.

## Hinweis zur Entstehung: dieses Projekt ist vollständig KI-generiert

> **Der gesamte Quelltext wurde zu 100 % von einer KI geschrieben** — „Vibe Coding“ im
> wörtlichen Sinn. Geschrieben hat ihn Hermes Agent mit dem Modell **DeepSeek V4.1 Flash 1017**.
> Der Mensch hinter dem Projekt hat keine Zeile selbst getippt: er hat das laufende Programm
> getestet, Fehler gemeldet, Entscheidungen getroffen und korrigiert.

Das steht hier absichtlich ganz oben und nicht im Kleingedruckten: KI-generierter Code ist
zu Recht umstritten. Er kann plausibel aussehen und trotzdem falsch sein, und niemand hat
jede Zeile „im Kopf“. Was dafür spricht, dass dieses Programm trotzdem benutzbar ist:

- **655 automatisierte Tests** (`cargo test`) prüfen Geometrie, Wandanschlüsse, Bemaßung,
  Dateiformat, PDF/DXF-Rundläufe und die Zahlenfelder der Oberfläche.
- **Jede Änderung wird am laufenden Programm nachgemessen** — gespeicherte Datei, Pixelmaß,
  Testfall —, nicht nur „sieht gut aus“.
- Die Architekturregeln und Designentscheidungen stehen offen in
  [`AGENTS.md`](AGENTS.md), damit Fehler nicht zufällig, sondern nachvollziehbar entstehen.

Trotzdem: wer dieses Programm produktiv für Bauanträge einsetzt, sollte den Code und vor
allem die Ergebnisse prüfen. Ein grüner Testlauf ist ein Sicherheitsnetz, kein Nachweis
fachlicher Richtigkeit — die fachliche Abnahme macht ein Mensch.

## Installation

**Es gibt noch keine fertigen Pakete.** Das Projekt ist derzeit bei Version 0.2 und noch
nicht öffentlich; die Installation ist erst mit der 1.0 vorgesehen. Wer jetzt schon
hineinschauen will, baut aus dem Quellcode — siehe [Entwickeln](#entwickeln).

### Linux — Platzhalter

```text
Noch kein Paket. Geplant: AppImage bzw. .deb, verteilt über die GitHub-Releases.
```

### Windows — Platzhalter

```text
Noch kein Paket. Geplant: Installer (.msi) bzw. portable .zip über die GitHub-Releases.
```

## Bedienung

Die Oberfläche ist deutsch und an Office-Programmen orientiert: Ribbon oben, Zeichnung in
der Mitte, Layer links, Eigenschaften rechts, Hotbar unten. Erste Schritte: **Wand** (`G`),
**Tür** (`H`), **Fenster** (`J`), **Maß** (`M`) — jeweils Start- und Endpunkt klicken,
`ESC` beendet das Werkzeug. Längen lassen sich nach dem ersten Klick tippen (`2,5` oder
`x;y`), der Maßstab steht unten rechts.

Alle Werkzeuge, Fangmodi, Panels und Tastenkürzel: [`docs/BEDIENUNG.md`](docs/BEDIENUNG.md).

### Sprache

Die Oberfläche ist auf Deutsch und lässt sich auf Englisch umstellen
(Einstellungen ▸ Bedienung ▸ „Sprache“, wirkt sofort und wird gespeichert). Die
Texte liegen nicht im Code, sondern in Sprachdateien unter
`crates/jjcad-core/lang/` — eine weitere Sprache ist damit eine Textdatei plus
sieben Zeilen in `crates/jjcad-core/src/i18n.rs`; fehlende Einträge fallen auf
Deutsch zurück. `scripts/i18n-check.py` prüft den Katalog gegen den Code
(fehlende, verwaiste und doppelte Schlüssel, Platzhalter, leere Werte) und läuft
in `cargo test` mit, damit er nicht unbemerkt verrottet. Schritt für Schritt:
[`docs/I18N.md`](docs/I18N.md).

## Stand

Version 0.2 — Kern und Oberfläche sind neu geschrieben, die Grundfunktionen stehen. Was
noch fehlt, steht offen in [`docs/STATUS.md`](docs/STATUS.md); die 1.0 gilt als erreicht,
wenn die Grundfunktionalität vollständig ist.

- Getestet ist der **Linux-Build** (X11 und Wayland); Windows ist vorgesehen, aber noch
  nicht geprüft.
- Schraffuren sind Musterschraffuren (Linien/Kreuz/Punkte), Splines werden beim DXF-Import
  als Polylinie genähert.
- Die Zeichnungen in `samples/` sind **Platzhalter**: KI-generiert, nicht fachlich geprüft,
  inhaltlich ohne baulichen Sinn. Sie zeigen, was das Programm kann — nicht, wie ein
  korrekter Plan aussieht.

## Aufbau des Repositories

```text
JoJa-CAD/
├── crates/
│   ├── jjcad-core/     Geometrie, Modell, Renderer, Aktionen, PDF/DXF/PNG (GUI-frei)
│   ├── jjcad-gui/      Desktop-Anwendung (egui/eframe)
│   └── jjcad-mcp/      MCP-Server über stdio
├── docs/               Architektur, Dateiformat, Befehle, MCP, Bedienung, Stand
├── samples/            Beispielzeichnungen, PDF, PNG, DXF (Platzhalter)
└── scripts/            Hilfsskripte (headless starten, Screenshot, Roundtrip-Test)
```

## Dokumentation

- [`docs/BEDIENUNG.md`](docs/BEDIENUNG.md) — Werkzeuge, Panels, erste Schritte
- [`docs/I18N.md`](docs/I18N.md) — Sprachen: Übersetzungen, neue Sprache anlegen, Prüfskript
- [`docs/TASTEN.md`](docs/TASTEN.md) — Tastenbelegung in drei Schichten (verbindlich)
- [`docs/ARCHITEKTUR.md`](docs/ARCHITEKTUR.md) — Aufbau, Datenfluss, Entscheidungen
- [`docs/FORMAT.md`](docs/FORMAT.md) — Dateiformat `.jjcad` (JSON, offen)
- [`docs/BEFEHLE.md`](docs/BEFEHLE.md) — alle Aktionen mit Parametern
- [`docs/MCP.md`](docs/MCP.md) — KI-Steuerung, Werkzeuge, Beispiele
- [`docs/GLOSSAR.md`](docs/GLOSSAR.md) — Begriffe, verbindlich für Oberfläche und Code
- [`docs/STATUS.md`](docs/STATUS.md) — Arbeitsstand und offene Punkte

## Lizenz

GPL-3.0-or-later — vollständiger Text in [`LICENSE`](LICENSE). Wer JoJa-CAD weitergibt, gibt es unter derselben Lizenz weiter.

## Entwickeln

Voraussetzungen: **Rust 1.88** oder neuer (`rustup`). Unter Linux zusätzlich die
Fenster- und GL-Bibliotheken (Debian/Ubuntu):

```bash
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev
```

```bash
cargo run --release -p jjcad-gui            # Anwendung starten
cargo run --release -p jjcad-gui -- datei.jjcad
cargo run -p jjcad-mcp -- --selftest        # Kern + Exporte prüfen
cargo test                                  # 655 Tests
```

Die Beispielzeichnungen lassen sich mit `cargo run -p jjcad-core --example demo_plan --
samples` neu erzeugen. Für Screenshots und automatisierte UI-Prüfungen setzt
`JJCAD_DEMO_UI=arch,inventar,blatt` einen Startzustand; `scripts/qa-shot.py` startet dafür
eine eigene Instanz auf einem unsichtbaren Display und macht ein Bild.

## Fehler melden und Wünsche äußern

Dieses Projekt nimmt **keine fremden Commits und keine Pull Requests** an. Der
Betreiber prüft keinen fremden Code — ein Programm, das Bauanträge zeichnet, soll
keine Stelle enthalten, die niemand verstanden hat. Willkommen sind dagegen:

- **Fehlerberichte** — was du gemacht hast, was passiert ist, was passieren sollte
- **Feature-Wünsche** — welches Problem du lösen willst, nicht nur welche Schaltfläche fehlt

Beides landet als Ticket in der eigenen Entwicklung und wird dort mit dem eigenen
KI-Agenten umgesetzt, samt Test und Nachweis. Ein guter Fehlerbericht enthält:

1. die **Build-Nummer** aus dem Fenstertitel (z. B. `0.2.0-s33-ab12cd3`) — ohne sie
   ist nicht feststellbar, welcher Stand bei dir lief,
2. die Datei (`.jjcad`) oder die genaue Klickfolge,
3. Erwartung und tatsächliches Verhalten, bei sichtbaren Fehlern mit Screenshot.

Forken und selbst weiterbauen ist mit der GPL ausdrücklich erlaubt — nur
eben nicht als Pull Request einsenden. Die Konventionen für Änderungen stehen in
[`AGENTS.md`](AGENTS.md), die Begriffe in [`docs/GLOSSAR.md`](docs/GLOSSAR.md):
`cargo test` muss grün sein, jede Änderung bekommt einen Test, Oberfläche und
Code-Kommentare sind deutsch, und ein Nachweis ist messbar (Datei, Pixelmaß, Test)
— nicht „sieht gut aus“.

Fachliches Gegenlesen hilft diesem Projekt mehr als jede weitere Funktion: ob eine
Bemaßung der DIN entspricht oder ein PDF/DXF wirklich brauchbar herauskommt, kann
nur beurteilen, wer damit arbeitet.
