# JoJa-CAD

**2D-CAD für Architektur- und Bauantragszeichnungen.** Bewusst minimal, kostenlos
und quelloffen — für Linux und Windows.

![JoJa-CAD Oberfläche](samples/ui.png)

*Beispielzeichnung, die mit dem Programm ausgeliefert wird — Platzhalter, nicht
fachlich geprüft.*

## Die Idee

Für einen Bauantrag braucht man kein Modellierprogramm mit tausend Werkzeugen,
sondern ein sauberes Werkzeug für Grundriss, Bemaßung und Blatt — und sonst nichts.

- **Fokus Bauantragszeichnung**: Wände mit Öffnungen, Bemaßung nach DIN, Blätter
  mit Rahmen und Schriftfeld, Linienstärken wie im Druck.
- **Bewusst minimal**: nur das, was eine Bauvorlage braucht. Kein 3D, kein BIM,
  kein DWG.
- **Kostenlos und quelloffen** (GPL-3.0-or-later), offenes Dateiformat (JSON), keine Cloud,
  kein Konto.
- **Linux und Windows.**
- **Oberfläche auf Deutsch**, auf Englisch umstellbar.
- **Vollständig über KI steuerbar** durch einen MCP-Server.

## Wichtigste Funktionen

| | |
|---|---|
| **Grundelemente** | Punkt, Linie, Polylinie, Rechteck, Kreis, Bogen, Ellipse, Schraffur, Text |
| **Architektur** | Wand mit Öffnungen, Tür, Fenster, Treppe, Volumen — die Bauteile verbinden sich dynamisch: Gehrung an Anschlüssen, Öffnungen schneiden die Wand, Türen sitzen auf der Blattseite |
| **Bemaßung** | Einzelmaß, Maßkette, Grundriss- und Schnitt-Kote nach DIN 406/1356; Maßtext in mm/cm/m |
| **Blöcke** | Systembibliothek (Sanitär, Küche, Möbel, Symbole) und eigene Projektblöcke, erweiterbar über Inventar und Hotbar |
| **Austausch** | PDF (vektoriell) und PNG schreiben, PDF und DXF importieren und exportieren |
| **KI-Steuerung** | MCP-Server mit 10 Werkzeugen, vollständig headless, inklusive Bild-Rückgabe |

## Hinweis: dieses Projekt ist vollständig KI-generiert

**Der gesamte Code wurde von einer KI geschrieben — keine Zeile wurde von Hand
getippt.** Entwickelt wurde er mit Hermes Agent und dem Modell **DeepSeek V4.1
Flash 1017**. Der Autor hat Entscheidungen getroffen, das laufende Programm
getestet und Fehler kritisiert.

KI-generierter Code ist zu Recht umstritten. Deshalb steht das hier offen statt
im Kleingedruckten: die automatisierten Tests und messbaren Nachweise sind ein
Sicherheitsnetz, kein Nachweis fachlicher Richtigkeit. Wer das Programm für echte
Bauanträge einsetzt, sollte Code und Ergebnisse prüfen.

## Installation

**Es gibt noch keine fertigen Pakete.** Das Projekt steht bei Version 0.2; die
Installation ist für die 1.0 geplant. Wer bis dahin testen will, baut aus dem
Quellcode (siehe unten).

### Linux *(Platzhalter)*

Geplant: AppImage bzw. .deb über die GitHub-Releases.

### Windows *(Platzhalter)*

Geplant: Installer bzw. portable ZIP über die GitHub-Releases.

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
- [`docs/ARCHITEKTUR.md`](docs/ARCHITEKTUR.md) — Aufbau, Datenfluss, Entscheidungen
- [`docs/FORMAT.md`](docs/FORMAT.md) — Dateiformat `.jjcad` (JSON, offen)
- [`docs/BEFEHLE.md`](docs/BEFEHLE.md) — alle Aktionen mit Parametern
- [`docs/MCP.md`](docs/MCP.md) — KI-Steuerung, Werkzeuge, Beispiele
- [`docs/GLOSSAR.md`](docs/GLOSSAR.md) — Begriffe, verbindlich für Oberfläche und Code
- [`docs/STATUS.md`](docs/STATUS.md) — Arbeitsstand und offene Punkte

## Lizenz

GPL-3.0-or-later — vollständiger Text in [`LICENSE`](LICENSE). Wer JoJa-CAD weitergibt, gibt es unter derselben Lizenz weiter.

## Entwickeln

Rust 1.88 oder neuer (`rustup`). Getestet ist der Linux-Build (X11 und Wayland);
unter Debian/Ubuntu zusätzlich:

```bash
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev
```

```bash
cargo run --release -p jjcad-gui   # Anwendung starten
cargo test
```

## Beitragen

**Keine fremden Commits, keine Pull Requests.** Willkommen sind Fehlerberichte
und Feature-Wünsche als Issue; umgesetzt werden sie im eigenen Ablauf mit dem
eigenen KI-Agenten, mit Test und Nachweis.

Ein brauchbarer Fehlerbericht enthält die **Build-Nummer aus dem Fenstertitel**,
die `.jjcad`-Datei oder die Klickfolge und Erwartung gegen tatsächliches
Verhalten — bei sichtbaren Fehlern mit Screenshot.

Konventionen für Änderungen stehen in [`AGENTS.md`](AGENTS.md), die Begriffe in
[`docs/GLOSSAR.md`](docs/GLOSSAR.md). Forken und selbst weiterbauen ist mit der
GPL ausdrücklich erlaubt.
