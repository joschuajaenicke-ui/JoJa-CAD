# Glossar JoJa-CAD

Ein Begriff, eine Bedeutung. Wer ein Ticket schreibt, benutzt die linke Spalte.
Steht rechts „nicht", dann ist das Wort im Projekt verpönt (auch in der Oberfläche).

## 1. Arbeit und Versionen

| Begriff | Bedeutung | nicht |
|---|---|---|
| **Karte** (Ticket) | Ein Arbeitsauftrag auf dem Board `joja-cad`, mit ID `t_…` | Task, Issue, Task-Karte |
| **Build `s<N>`** | Ausgelieferter Stand aus `master`, Nummer zählt bei jeder Auslieferung hoch (`s1`, `s2` …), dazu der Commit-Hash: `s7-9f08437` | „Version", „Version b26" |
| **Build-Nummer im Programm** | Fenstertitel und Statusleiste zeigen `s<N>` — nur so erkennst du, was läuft | — |
| **master** | Der einzige Zweig, aus dem Builds entstehen. Alles andere ist Arbeit. | Hauptzweig, Main |
| **Zweig** | Arbeitszweig einer Karte (`wt/t_xxxx`, `fix/t_xxxx`) | Branch, Branch-Zweig |
| **Worktree** | Eigenes Arbeitsverzeichnis einer Karte (`.worktrees/t_xxxx`), damit der Hauptbaum sauber bleibt | Unterordner, Klon |
| **Merge** | Übernahme eines Karten-Zweigs in `master`; genau ein Commit pro Karte | Zusammenführen, Einspielen |
| **Rückbau** (Rollback) | Rücknahme genau einer Karte aus `master` (`scripts/rollback-card.sh`) | Reset, Zurücksetzen |
| **Run** | Ein Arbeitslauf eines Workers an einer Karte; mehrere Runs pro Karte sind normal | Durchgang, Versuch |
| **TEST-Stempel** | Kommentar `TEST s<N>-<hash>` = in diesem Build enthalten, zum Prüfen freigegeben. Nur der **jüngste** Stempel gilt. | „fertig", „erledigt" |
| **Prüfweg** | Die 2–3 Schritte, mit denen der Mensch das Ergebnis sieht (Klicks oder Beispieldatei) | Testanleitung, „bitte testen" |

## 2. Board-Spalten

| Spalte | Bedeutung |
|---|---|
| **triage** | Neue Meldung, noch keine belastbare Aufgabe — wird zu einem Arbeitspaket geschärft |
| **todo / ready** | Warteschlange; `ready` = darf laufen |
| **running** | Ein Worker arbeitet (genau einer gleichzeitig) |
| **review** | **Wartet auf dich.** Mit `TEST`-Stempel = in einem Build enthalten |
| **blocked** | Echter Blocker: Entscheidung nötig, Repro fehlt, zwei Fehlversuche |
| **done** | Von dir abgenommen |

## 3. Zeichnung und Geometrie

| Begriff | Bedeutung | nicht |
|---|---|---|
| **Achse** (Wandachse) | Die Mittellinie einer Wand, auf der gezeichnet und gefangen wird | Mittellinie, Mittellinie der Wand |
| **Wandkante** / **Wandkontur** | Die beiden Längsflächen des Wandkörpers, senkrecht zur Dicke | Wandlinie, Rand |
| **Wandkörper** | Die gefüllte Fläche zwischen den Kanten (Dicke × Länge) | Wandfläche, Wandblock |
| **Gehrung** | Der geschlossene Übergang zweier Wände an einer Ecke; entsteht über die Winkelhalbierende | Ecke, Verbindung |
| **Winkelhalbierende** | Hilfslinie, die den Innenwinkel zweier Wände halbiert; **nie sichtbar** im Plan | Halbierende, Mittellinie des Winkels |
| **Ausrichtung** (Wand) | Lage des Körpers zur Achse: Links / Mittig / Rechts | Seite, Offset |
| **Dicke** | Wandstärke quer zur Achse (mm) | Breite (bei Wänden) |
| **Füllfläche** | Objekt, das eine geschlossene Fläche ausfüllt — Farbe oder Muster; **dasselbe Objekt** wie Schraffur | Fill, Fläche, Schraffur-Objekt |
| **Schraffur** | Füllfläche mit Muster (Linien, Kreuz, Punkte, Dämmung) — gleiches Objekt, nur anderes Muster | Hatch, Musterfläche |
| **Dämmung** | Schraffurmuster als Mäanderlinie (Sinus), Amplitude = Wanddicke | Isolierung, Dämm-Muster |
| **Insel** / **Loch** | Ausgesparte Fläche innerhalb einer Füllfläche (z. B. Stütze im Raum) | Lochung, Aussparung |
| **Umgrenzung** | Die Punktfolge, die eine Füllfläche definiert | Umriss, Kontur, Polygon (nur wenn wirklich gemeint) |
| **Umrandung** | Die gezeichnete Linie entlang der Umgrenzung einer Füllfläche; in den Eigenschaften an/aus, Vorgabe je Erstellmodus | Randlinie, Rahmen, Kontur |
| **Farbeimer** | Werkzeug: Klick in eine leere Fläche, Umgrenzung wird automatisch ermittelt | Füllwerkzeug, Zauberstab, Paint-Bucket |
| **Griff** | Anfasser zum Ziehen an Enden, Ecken und Kantenmitten eines ausgewählten Objekts | Handle, Punkt, Anker |
| **Fang** | Einrasten auf Punkten/Geometrie beim Zeichnen | Snap, Einrasten |
| **Objektfang** | Fang auf vorhandene Geometrie: END (Endpunkt), SCHN (Schnitt), MIT (Mitte), ZEN (Zentrum), QUA (Quadrant), LOT (Lot), NÄC (nächster Punkt), RAS (Raster), POL (Polar) | Fangmodus, Fangart |
| **Polarfang** | Fang auf feste Winkel (45°-Schritte, Toleranz 6°) | Winkelraster, Ortho (das ist etwas anderes) |
| **Erweiterter Punktfang** | Bleibt der Zeiger kurz auf einem Fangpunkt stehen, wird dieser gehalten und bekommt zwei Hilfslinien; auf ihnen fängt der nächste Punkt | Punktverfolgung, Tracking, OTRACK |
| **Hilfslinie (Punktfang)** | Eine der beiden Linien des erweiterten Punktfangs — **nicht** die Winkelhalbierende und nicht der Layer „Hilfslinien" | Fanglinie, Verfolgungslinie |
| **Ortho** | Sperre auf waagerecht/senkrecht | — |
| **Raster** | Zeichenraster; der Chip schaltet Anzeige **und** Rasterfang | Gitter |

## 4. Objekte

| Begriff | Bedeutung | nicht |
|---|---|---|
| **Tür** | Öffnung mit Blatt; sitzt immer auf der **vorderen Wandkante** | Türobjekt, Door |
| **Blatt** | Der Türflügel; „Blatt: Seite A/B" = auf welcher Wandseite er aufschlägt | Öffnungsrichtung, „nach innen" |
| **Anschlag** | Auf welcher Seite der Öffnung das Blatt angeschlagen ist: links / rechts | Band, Scharnierseite |
| **Fenster** | Öffnung mit Rahmen; Lage in der Wanddicke wählbar (Links/Mittig/Rechts) | Fensterobjekt |
| **Treppe** | Treppenobjekt mit Stufen, Breite, Richtung (auf/ab/beide) | Stiege |
| **Block** | Wiederverwendbare Zeichnung (Definition) | Symbol, Bauteil |
| **Blockinstanz** | Ein eingesetzter Block im Plan (mit Position, Drehung, Spiegelung) | Block (wenn die Instanz gemeint ist) |
| **Maßkette** | Mehrere Maße in einer Linie, fortlaufend | Bemaßungskette, Maßreihe |
| **Höhenkote** | Höhenangabe als Text mit Bezug (±0,00) | Höhenmaß, Kote |
| **Ansichtsfenster** | Fenster auf dem Blatt, das einen Ausschnitt der Zeichnung zeigt | Viewport, Ansicht |
| **Schriftfeld** | Rahmen mit Feldern für Plan-Nr., Maßstab, Bauherr | Titelblock, Plankopf |
| **Papierraum** | Bearbeitungsebene eines Blatts (Druckvorschau); enthält Schriftfeld, Texte und eigene Blattelemente | Layout, Paper Space |
| **Druckvorschau** | Ansicht des Blatts im Papierraum — zeichnen und ändern wie im Modell | Layout-Ansicht, Print Preview |
| **Blatt** | Papierformat (A4 … A0) mit Schriftfeld | Layout, Seite |

## 5. Bedienoberfläche

| Begriff | Bedeutung | nicht |
|---|---|---|
| **Ribbon** | Die Registerkartenleiste oben mit Gruppen und Schaltflächen | Menüleiste, Toolbar |
| **Registerkarte** | Zeichnen / Architektur / Bemassung / Blöcke / Blatt / Ansicht | Tab (das sind Dokumente) |
| **Hotbar** | Die Schnellzugriffleiste unten | Favoritenleiste |
| **Inventar** | Seitliche Sammlung von Blöcken und Funktionen (mit `E`) | Bibliothek, Palette |
| **Eigenschaften** | Das rechte Seitenpanel mit Feldern zum ausgewählten Objekt **oder** zum nächsten Element | Einstellungen (das ist der Dialog), Seitenleiste |
| **Einstellungen** | Der Dialog (`Datei → Einstellungen`) für globale Vorgaben | Optionen, Preferences |
| **Dateidialog** | Das Fenster zur Auswahl von Dateien beim Öffnen/Speichern/Exportieren. Vorgabe ist der **Systemdialog** des Betriebssystems; „Eingebaut“ meint den Dialog von JoJa-CAD | Dateiauswahl, File-Picker, Chooser |
| **Dropdown** | Auswahlfeld mit Klappe; im ganzen Programm gleich breit (150 px) und gleicher Pfeil | Auswahlliste, Combo |
| **Freier Wert** | Zahlenfeld neben einem Dropdown für einen selbst eingegebenen Wert | „eigener Wert" (verpönt) |
| **Statusleiste** | Die Zeile ganz unten: Koordinaten, Fang, Ortho, Raster, Maßstab, Build | Fußzeile |
| **Chip** | Kleine Schaltfläche in der Statusleiste | Button, Toggle |
| **Vorschau** (Ghost) | Halbtransparente Darstellung des Elements, das beim nächsten Klick entsteht | Ghost, Preview |
| **Kopieren** | Auswahl in die Zwischenablage legen (Strg+C); eingefügt wird mit Vorschau (Strg+V) | Kopie, Duplizieren |
| **Kopieren mit Basispunkt** | Kopie versetzt absetzen: Basispunkt, dann Zielpunkt (Strg+Umschalt+C); Icon mit Punkt unten links | Kopieren, Duplizieren |
| **Chat** | Meldungen der Anwendung unten links in der Zeichenfläche; verblassen nach einigen Sekunden, `^` (Taste unter ESC) öffnet das Protokoll | Log, Konsole |
| **Protokoll** | Der geöffnete Chat: alle Meldungen, scrollbar, Zeichnung blass | Verlauf, Historie |
| **Toast** | Kurze Meldung, die von selbst verschwindet — heute eine Zeile im Chat | Benachrichtigung, Popup |
| **Zieh-Modus** | Wie gezogen wird: „Gedrückthalten" oder „Zwei Klicks" | Drag-Modus, Drag and Drop |

## 6. Ausgabe

| Begriff | Bedeutung | nicht |
|---|---|---|
| **Displayliste** | Die eine Zwischendarstellung, aus der Bildschirm, PDF und PNG gezeichnet werden | Renderliste, Draw-List |
| **Linienstärke** | Strichdicke in mm (0,25 / 0,35 / 0,50); Anzeige in `px` oder `DPI` | Linienbreite, Strichstärke |
| **px-Modus** | Stärke bleibt auf dem Bildschirm konstant | Bildschirmmodus |
| **DPI-Modus** | Stärke wächst mit dem Zoom, wie im Druck | Druckmodus |
| **Maßstab** | Verhältnis Zeichnung : Papier (1:100) | Skalierung |
| **Papier-mm** | Koordinaten auf dem Blatt, unabhängig vom Maßstab | Blattkoordinaten |

## 7. Ticket schreiben — Vorlage

Ein Ticket ist arbeitsfähig, wenn diese vier Zeilen stehen:

```
Bereich:  Wände / Türen / Füllflächen / Auswahl / Ribbon / …
Symptom:  Was passiert (oder fehlt) — in Worten des Glossars
Repro:    Schritte oder Datei, mit der man es sieht
Soll:     Woran man erkennt, dass es stimmt (Zahl, Datei, Bild)
```

Beispiel:

```
Bereich:  Füllflächen
Symptom:  Nach dem Farbeimer bleibt zwischen Wand und Fläche ein weißer Spalt (~2 mm)
Repro:    samples/Wohnhaus.jjcad öffnen → C → in den Flur klicken
Soll:     Flächenrand deckungsgleich mit der Wandkante, Spalt < 0,5 mm
```

Unsichere Begriffe lieber hier nachschlagen als im Ticket umschreiben — das Glossar
ist die gemeinsame Sprache, nicht die Oberfläche.
