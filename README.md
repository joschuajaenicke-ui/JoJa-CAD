# JoJa-CAD

**Deutsch** · [English](README.en.md)

**2D-CAD für Architektur- und Bauantragszeichnungen.** Bewusst minimal, kostenlos
und quelloffen — für Linux und Windows.

![JoJa-CAD Oberfläche](samples/ui.png)

*Beispielzeichnung aus dem Repository — Platzhalter, nicht fachlich geprüft.*

## Die Idee

Für einen Bauantrag braucht man kein 3D-Modellierprogramm mit tausend Werkzeugen,
sondern ein sauberes Werkzeug für Grundriss, Bemaßung und Blatt — und sonst nichts.

- **Fokus Bauantragszeichnung**: Wände mit Öffnungen, Bemaßung nach DIN, Blätter
  mit Rahmen und Schriftfeld, Linienstärken wie im Druck.
- **Bewusst minimal**: nur das, was eine Bauvorlage braucht. Kein 3D, kein BIM,
  kein IFC, kein DWG.
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

Der gesamte Sourcecode wurde von einer KI generiert — keine Zeile wurde von Hand getippt. Der Entwicklungsprozess wurde so umgestaltet, dass er im Wesentlichen nur noch aus dem Beschreiben von Features und Bugs besteht. Der Code wurde mit Hermes Agent und dem Modell DeepSeek V4.1 Flash 1017 entwickelt. Die menschliche Beteiligung beschränkte sich auf das Treffen von Entscheidungen, das Testen des laufenden Programms und die Kritik von Fehlern.

Dieses Projekt ist als Experiment entstanden, um zu sehen, wie weit KI-generierte Programmierung trägt. Der Code ist so schnell gewachsen, dass eine Prüfung im Detail unmöglich ist — was genau im Programm eingebaut wurde, lässt sich daher nicht vollständig wissen. KI-generierter Code ist zu Recht umstritten. Getestet wurde ausschließlich auf oberflächlicher Nutzungsebene; die Nutzung setzt folglich Vertrauen in die Korrektheit des KI-generierten Codes voraus.

## Installation

Fertige Pakete liegen bei den **[Releases](../../releases)** — entpacken bzw.
starten, keine Installation, keine Administratorrechte, kein Konto.

### Linux

**AppImage** (empfohlen, eine Datei):

```bash
chmod +x JoJa-CAD-0.2.3-x86_64.AppImage
./JoJa-CAD-0.2.3-x86_64.AppImage
```

Wer mehrere AppImages verwaltet, bindet sie am besten mit
[Gear Lever](https://github.com/mijorus/gearlever) ein: es legt die
Startmenü-Verknüpfung an, aktualisiert beim neuen Release und hält die Dateien
an einem Ort statt im Download-Ordner. Startet ein System ohne FUSE die Datei
nicht, hilft `./JoJa-CAD-0.2.3-x86_64.AppImage --appimage-extract-and-run`.

**Tar-Archiv** (wenn du die Dateien lieber offen im Ordner hast): entpacken und
`./jjcad` starten. Die Schriften liegen im Ordner `fonts/` neben dem Programm.

Der **MCP-Server** steckt in beiden Linux-Paketen mit: im AppImage über
`--mcp` (z. B. `./JoJa-CAD-0.2.3-x86_64.AppImage --mcp --selftest`), im
Tar-Archiv als eigene Datei `jjcad-mcp`.

### Windows

**Portables ZIP**: entpacken (beliebiger Ordner, USB-Stick genügt) und
`jjcad.exe` starten. Keine Installation, keine Registry, keine
Administratorrechte; der Ordner darf beliebig verschoben werden. Die Schriften
stecken in der `.exe`, ein Ordner `fonts/` daneben ist nicht nötig;
`SCHRIFTEN-LIZENZ.txt` daneben nennt ihre Lizenz (SIL OFL 1.1).

#### Wenn Windows warnt

Die `jjcad.exe` ist **nicht signiert** (eine Code-Signatur setzt ein Zertifikat
mit geprüfter Identität voraus). Deshalb stufen SmartScreen und Defender eine
frisch heruntergeladene Datei als unbekannt ein. Drei Wege, je nach Meldung:

1. **SmartScreen** („Windows hat den Start dieser App verhindert"): auf
   **„Weitere Informationen"** klicken, dann auf **„Trotzdem ausführen"**.
2. **Defender meldet einen Fund**: im Schutzverlauf **„Zulassen"** bzw.
   **„Zulassen auf dem Gerät"** wählen — oder den Programmordner unter
   Viren- & Bedrohungsschutz → Einstellungen → Ausschlüsse eintragen.
3. **Am saubersten vorher entsperren**: Rechtsklick auf die **ZIP-Datei** →
   **Eigenschaften** → unten im Abschnitt „Sicherheit" das Kästchen
   **„Entsperren"** anhaken → Übernehmen → *danach* entpacken. Die Freigabe gilt
   dann für alles im Archiv. Die einzelne `.exe` hat dasselbe Kästchen unter
   Eigenschaften → Allgemein („Die Datei stammt von einem anderen Computer …").

### macOS (ungeprüft)

Die Fassung ist gebaut, aber noch nicht ausprobiert — Rückmeldung ist willkommen.
Zwei Pakete: `macos-arm64` für Apple Silicon, `macos-x64` für Intel-Rechner.

```bash
unzip JoJa-CAD-0.2.3-macos-arm64.zip
xattr -dr com.apple.quarantine JoJa-CAD-0.2.3.app
open JoJa-CAD-0.2.3.app
```

Das Programm ist **nicht signiert und nicht notarisiert** (dafür braucht es ein
Apple-Developer-Konto), deshalb blockiert macOS den ersten Start. Der
`xattr`-Befehl hebt die Quarantäne auf; alternativ Rechtsklick auf das Programm →
„Öffnen" → „Öffnen". Der MCP-Server liegt als
`JoJa-CAD-0.2.3.app/Contents/MacOS/jjcad-mcp` daneben, die Schriften unter
`Contents/MacOS/fonts/`.

Wer selbst bauen will: siehe [Entwickeln](#entwickeln).

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

`portable.txt` ist der **einzige** Schalter zwischen portabel und installiert:
liegt die Datei neben dem Programm, bleibt alles im Programmordner (`data/`);
fehlt sie, geht alles in den Plattformstandard. Das portable ZIP für Windows
liefert sie mit — sie darf nicht gelöscht werden. Ein portabler Lauf übernimmt
**keine** Einstellungen vom Rechner, an dem er steckt.

Beim ersten Start einer installierten Fassung wird eine vorhandene `prefs.json`
aus dem alten Ort (`~/.config/jjcad`) in den neuen Ordner kopiert; die alte
Datei bleibt liegen.
Die drei Unterordner lassen sich unter Einstellungen → Speicherorte ändern
(relativ zum Datenordner oder absolut; jedes Feld hat ein „Zurücksetzen“). Läuft bereits
eine Instanz auf demselben Ordner, übernimmt sie die Arbeit: eine von außen
geöffnete Zeichnung erscheint als Reiter im laufenden Fenster, ein Start ohne
Datei holt es nur nach vorn. Für getrennte Arbeit einen anderen Datenordner
angeben (`--daten-dir`).

## Dokumentation

Die Dokumentation ist nach **Leser** sortiert, nicht nach Thema — sonst sucht
jeder in der falschen Datei:

| Ordner | Für wen | Inhalt |
|---|---|---|
| [`docs/benutzer/`](docs/benutzer/) | Anwender | Bedienung, Tastenbelegung, Sicherung, Glossar, bekannte Grenzen |
| [`docs/technik/`](docs/technik/) | Entwickler und Agenten | Architektur, Dateiformat, Befehlsreferenz, MCP, Übersetzungen, Release-Ablauf |
| [`docs/wissen/`](docs/wissen/) | wer verstehen will, *warum* | Herleitungen und Messungen: Wandgehrung, Bogenglättung, Dämmungsschlaufen, PDF-Import, Wayland-Befund |

`docs/wissen/` ist der Teil, der über den Quelltext hinausgeht: dort steht, wie
ein Problem gelöst wurde, welche Variante verworfen wurde und mit welcher Messung
die Lösung belegt ist. Einstieg und Regeln für alle drei Bereiche:
[`docs/README.md`](docs/README.md).

Die wichtigsten Dateien:

- [`docs/benutzer/BEDIENUNG.md`](docs/benutzer/BEDIENUNG.md) — Werkzeuge, Panels, erste Schritte
- [`docs/benutzer/SICHERUNG.md`](docs/benutzer/SICHERUNG.md) — Autosave und Aufbewahrung: Einstellungen, Zusagen, Regeln
- [`docs/benutzer/GLOSSAR.md`](docs/benutzer/GLOSSAR.md) — Begriffe, verbindlich für Oberfläche und Code
- [`docs/benutzer/BEKANNTE-GRENZEN.md`](docs/benutzer/BEKANNTE-GRENZEN.md) — bekannte Grenzen und was sie lösen würde
- [`docs/technik/ARCHITEKTUR.md`](docs/technik/ARCHITEKTUR.md) — Aufbau, Datenfluss, Entscheidungen
- [`docs/technik/FORMAT.md`](docs/technik/FORMAT.md) — Dateiformat `.jjcad` (JSON, offen)
- [`docs/technik/BEFEHLE.md`](docs/technik/BEFEHLE.md) — alle Aktionen mit Parametern
- [`docs/technik/MCP.md`](docs/technik/MCP.md) — KI-Steuerung, Werkzeuge, Beispiele
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

Es werden keine Commits oder Pull Requests angenommen. Bei KI-generiertem Code ist eine klassische Code-Review zu aufwendig und würde den Entwicklungsprozess ausbremsen. Willkommen sind stattdessen Fehlerberichte und Feature-Wünsche als GitHub-Issues: Da die Umsetzung über Prompts läuft, ist eine Prompt-Review deutlich einfacher als eine Code-Review, und eingereichte Vorschläge werden im eigenen Ablauf mit dem eigenen KI-Agenten umgesetzt.

**So reichst du ein:**

1. Öffne ein Issue auf GitHub und wähle die passende Vorlage (Bug Report oder Feature Request).
2. Beschreibe das Anliegen so, wie man es einem Agenten geben würde: als direkte Anweisung mit klarem Ziel, nicht als Diskussion.
3. Halte dich an dieselbe Struktur:
   - **Ist-Zustand**: Was tut das Programm aktuell? Wie lässt sich das Verhalten reproduzieren?
   - **Fehler bzw. Problem**: Was ist daran falsch oder unzureichend? (Bei Feature-Wünschen: Was kann das Programm noch nicht, was es können sollte?)
   - **Soll-Zustand**: Wie soll es stattdessen sein? Was genau soll der Agent tun?

Je präziser das Issue diese drei Punkte beschreibt, desto eher kann es direkt als Prompt übernommen und umgesetzt werden. Wichtige Begriffe sind in [`docs/benutzer/GLOSSAR.md`](docs/benutzer/GLOSSAR.md) formuliert.

Forken und selbst weiterbauen ist mit der GPL ausdrücklich erlaubt; das GitHub-Projekt enthält auch die notwendige [`AGENTS.md`](AGENTS.md), damit direkt mit einem anderen Agenten weitergearbeitet werden kann.
