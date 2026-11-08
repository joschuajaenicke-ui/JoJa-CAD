# JoJa-CAD

**2D-CAD für Architektur- und Bauantragszeichnungen.** Bewusst minimal, kostenlos
und quelloffen — für Linux und Windows.

![JoJa-CAD Oberfläche](samples/ui.png)

*Beispielzeichnung, die mit dem Programm ausgeliefert wird — Platzhalter, nicht
fachlich geprüft.*

## Die Idee

Für einen Bauantrag braucht man kein 3D Modellierprogramm mit tausend Werkzeugen,
sondern ein sauberes Werkzeug für Grundriss, Bemaßung und Blatt — und sonst nichts.

- **Fokus Bauantragszeichnung**: Wände mit Öffnungen, Bemaßung nach DIN, Blätter
  mit Rahmen und Schriftfeld, Linienstärken wie im Druck.
- **Bewusst minimal**: nur das, was eine Bauvorlage braucht. Kein 3D, kein BIM,
  kein IoT, kein DWG.
- **Kostenlos und quelloffen** (GPL-3.0-or-later), offenes Dateiformat (JSON), keine Cloud,
  kein Konto.
- **Linux und Windows.** macOS ist geplant.
- **Oberfläche auf Deutsch**, auf Englisch umstellbar, weitere Sprachen möglich.
- **Vollständig über KI steuerbar** durch einen MCP-Server.

## Wichtigste Funktionen

| | |
|---|---|
| **Grundelemente** | Punkt, Linie, Polylinie, Rechteck, Kreis, Bogen, Ellipse, Schraffur, Text |
| **Architektur** | Wand mit Öffnungen, Tür, Fenster, Treppe, Volumen — die Bauteile verbinden sich dynamisch: Gehrung an Anschlüssen, Öffnungen schneiden die Wand, Türen sitzen auf der Blattseite |
| **Bemaßung** | Einzelmaß, Maßkette, Grundriss- und Schnitt-Kote nach DIN 406/1356; Maßtext in mm/cm/m |
| **Blöcke** | Systembibliothek (Sanitär, Küche, Möbel, Symbole) und eigene Projektblöcke, erweiterbar |
| **Austausch** | Import und Export von PDF, PNG und DXF möglich |
| **KI-Steuerung** | MCP-Server mit 10 Werkzeugen, vollständig headless, inklusive Bild-Rückgabe |

## Hinweis: dieses Projekt ist vollständig KI-generiert

Der gesamte Sourcecode wurde von einer KI generiert — keine Zeile wurde von Hand getippt. Der Entwicklungsprozess wurde dazu umgestaltet, dass er im Wesentlichen nur noch aus dem Beschreiben von Features und Bugs besteht. Der Code wurde mit Hermes Agent und dem Modell DeepSeek V4.1 Flash 1017 entwickelt. Die menschliche Beteiligung beschränkte sich auf das Treffen von Entscheidungen, das Testen des laufenden Programms und die Kritik von Fehlern.

Dieses Projekt ist als Experiment entstanden, um zu sehen, wie weit KI-generierte Programmierung trägt. Der Code ist so schnell gewachsen, dass eine Prüfung im Detail unmöglich ist — was genau im Programm eingebaut wurde, lässt sich daher nicht vollständig wissen. KI-generierter Code ist zu Recht umstritten. Getestet wurde ausschließlich auf oberflächlicher Nutzungsebene; die Nutzung setzt folglich Vertrauen in die Korrektheit des KI-generierten Codes voraus.

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

## Wo die Daten liegen

Alle Nutzdaten liegen in **einem** Ordner: `prefs.json`, `sitzung.json`,
`blocks/` (Benutzerblöcke), `sicherungen/` und `log/`. Zeichnungen bleiben, wo
Sie sie speichern — das Programm legt in deren Ordner nichts an.

Den Ordner bestimmt die erste zutreffende Angabe:

1. `jjcad --daten-dir <Pfad>`
2. die Umgebungsvariable `JJCAD_DATA_DIR`
3. eine Datei `portable.txt` neben dem Programm → `<Programmordner>/data`
4. der Plattformstandard:
   - Linux `$XDG_DATA_HOME/JoJa-CAD`, sonst `~/.local/share/JoJa-CAD`
   - Windows `%APPDATA%\JoJa-CAD`
   - macOS `~/Library/Application Support/JoJa-CAD`

Beim ersten Start wird eine vorhandene `prefs.json` aus dem alten Ort
(`~/.config/jjcad`) in den neuen Ordner kopiert; die alte Datei bleibt liegen.
Die drei Unterordner lassen sich unter Einstellungen → Speicherorte ändern
(relativ zum Datenordner oder absolut, je Feld „Zurücksetzen“). Läuft bereits
eine Instanz auf demselben Ordner, meldet die zweite das und bietet an, mit
einem anderen Datenordner zu starten.

## Dokumentation

- [`docs/BEDIENUNG.md`](docs/BEDIENUNG.md) — Werkzeuge, Panels, erste Schritte
- [`docs/ARCHITEKTUR.md`](docs/ARCHITEKTUR.md) — Aufbau, Datenfluss, Entscheidungen
- [`docs/FORMAT.md`](docs/FORMAT.md) — Dateiformat `.jjcad` (JSON, offen)
- [`docs/SICHERUNG.md`](docs/SICHERUNG.md) — Autosave und Aufbewahrung: Einstellungen, Zusagen, Regeln
- [`docs/BEFEHLE.md`](docs/BEFEHLE.md) — alle Aktionen mit Parametern
- [`docs/MCP.md`](docs/MCP.md) — KI-Steuerung, Werkzeuge, Beispiele
- [`docs/GLOSSAR.md`](docs/GLOSSAR.md) — Begriffe, verbindlich für Oberfläche und Code
- [`docs/STATUS.md`](docs/STATUS.md) — Arbeitsstand und offene Punkte
- [`docs/WAYLAND-DATEI-DROP.md`](docs/WAYLAND-DATEI-DROP.md) — warum Datei-Drop unter Wayland nicht geht und wann er wieder kommt
- [`docs/BEKANNTE-GRENZEN.md`](docs/BEKANNTE-GRENZEN.md) — bekannte Grenzen, die derzeit nicht lösbar sind (und was sie lösen würde)
- [`docs/VERSIONEN.md`](docs/VERSIONEN.md) — Versionierung, Release-Ablauf, Weg zur 0.3
- [`CHANGELOG.md`](CHANGELOG.md) — was sich je Version geändert hat

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

Es werden keine Commits oder Pull Requests angenommen. Bei KI-generiertem Code ist eine klassische Code Review zu aufwendig und würde den Entwicklungsprozess ausbremsen. Willkommen sind stattdessen Fehlerberichte und Feature-Wünsche als GitHub-Issues: Da die Umsetzung über Prompts läuft, ist eine Prompt-Review deutlich einfacher als eine Code-Review, und eingereichte Vorschläge werden im eigenen Ablauf mit dem eigenen KI-Agenten umgesetzt.

So reichst du ein
Öffne ein Issue auf GitHub und wähle die passende Vorlage (Bug Report oder Feature Request).
Beschreibe das Anliegen so, wie man es einem Agenten geben würde: als direkte Anweisung mit klarem Ziel, nicht als Diskussion.
Folge dabei immer derselben Struktur:Ist-Zustand: Was tut das Programm aktuell? Wie lässt sich das Verhalten reproduzieren?
Fehler bzw. Problem: Was ist daran falsch oder unzureichend? (Bei Feature-Wünschen: Was kann das Programm noch nicht, was es können sollte?)
Soll-Zustand: Wie soll es stattdessen sein? Was genau soll der Agent tun?

Je präziser das Issue diese drei Punkte beschreibt, desto eher kann es direkt als Prompt übernommen und umgesetzt werden.
Wichtige Begriffe sind in [`docs/GLOSSAR.md`](docs/GLOSSAR.md) formuliert. 
Forken und selbst weiterbauen ist mit der GPL ausdrücklich erlaubt, das Github Projekt beinhaltet auch die notwendige Agent.md, damit direkt mit einem andern Agent weiter gearbeitet werden kann.
