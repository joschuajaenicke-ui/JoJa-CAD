# Änderungen

Alle nennenswerten Änderungen an JoJa-CAD. Format nach
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[`docs/technik/VERSIONEN.md`](docs/technik/VERSIONEN.md).

Abschnitte je Version: **Neu**, **Geändert**, **Behoben**. Ein Eintrag ist ein
Satz aus Nutzersicht — was man jetzt anders machen kann, nicht welche Datei
angefasst wurde.

## [0.2.2] — 2026-11-11

Erste Fassung mit fertigen Paketen: **AppImage** für Linux, dazu die
umfangreichste Funktionsrunde bisher — Maßketten, Fang, Ansichtsfenster und
Einstellungen sind deutlich weiter.

### Neu

- **AppImage** als dritte Downloadform: eine Datei, keine Installation, keine
  Administratorrechte; der MCP-Server steckt mit im Paket (`--mcp`).
- **Eigene Dateivorlage** für neue Zeichnungen.
- **Tangentialfang** für Linien an Kreis und Bogen, auch bei bewegten Kreisen.
- **Winkelhalbierender Fang** zwischen zwei Linien.
- **Maßkette**: drei Eingabemodi, Messpunkte nachträglich bearbeiten, Vorschau
  der neuen Kette beim Ändern, Orientierung dreht mit; Einzelmaß und Maßkette
  sind jetzt ein Element.
- **Ansichtsfenster**: eigener Ansichtsbereich einstellbar, Doppelklick springt
  in die Ansicht, `F5` zeigt die ganze Zeichnung.
- **Dämmung** in mehreren Darstellungsstufen für kleine Auflösungen.
- **Leistungseinstellungen**: Auflösung der Rundungen einstellbar, Objekte
  außerhalb der Ansicht werden nicht mehr gezeichnet, die Leistungsanzeige
  zählt die Unterteilungen mit.
- **Fang-Einstellungen** in Gruppen, mit allen Stellschrauben; die Wahl der
  Fangmarken wird gespeichert.
- **Schnellauswahl** übernimmt alle Eigenschaften des angeklickten Objekts.
- **Rasteroptionen**: Rasterarten, fester Modus und erzwungener Rasterfang.
- **Rotieren** nimmt einen getippten Winkel an; **Verlängern** dehnt auch
  Polylinien mit ihrem Stützbogen.
- Beim Einfügen erscheint eine **Miniatur am Zeiger**, solange die Vorschau die
  Zeichenfläche verlässt.

### Geändert

- Die Dokumentation ist nach Leser sortiert: `docs/benutzer/`, `docs/technik/`,
  `docs/wissen/` (Herleitungen und Messungen). Wegweiser: `docs/README.md`.
- Die Oberfläche setzt mit **Liberation** statt mit der erstbesten Systemschrift
  — gleiche Spaltenbreiten auf jedem Rechner.
- Dämmung ist in Wänden wieder **eine Linie** je Wand; Schraffuren werden als
  Polygonzug gezeichnet statt als Strecke je Stützpunkt.
- Splines und Vollellipsen folgen der eingestellten Pfeilhöhe.
- Ein Polylinienzug mit einem einzigen Element wird zur Linie.

### Behoben

- Rasterlinien fehlten bei hohem Zoom.
- Der Polarfang griff über ein ganzes Band statt nur in seiner Breite.
- Fangmarken hingen am Mauszeiger statt am Objekt und blieben doppelt stehen.
- Der Schließdialog bot kein Speichern an, wenn die Datei noch keinen Ort hatte.
- Rechtsklick auf eine Schraffur öffnete kein Objektmenü.
- Fensterschatten fehlte ohne Titelleiste.
- Die alte Maßkette blieb während der Bearbeitung der Messpunkte stehen.

### Bekannt

- Die Windows-Fassung ist gebaut, aber nicht ausgiebig getestet.
- Unter Wayland wird die Fensterposition nicht gespeichert (siehe
  `docs/benutzer/BEKANNTE-GRENZEN.md`).
- Die Zeichnungen in `samples/` sind Platzhalter ohne fachliche Prüfung.

## [0.2.1] — 2026-11-09

Sicherung und Sitzung: ungespeicherte Arbeit geht nicht mehr verloren. Dazu der
Blockeditor und ein Schwung Detailarbeit an Linienstärken, Text und Schraffuren.

### Neu

- **Sicherung**: Während der Arbeit legt das Programm automatisch Stände an —
  getrennt vom Speichern, die geöffnete Datei wird dabei nie angetastet.
  Einstellbar sind Intervall (1–240 min), Anzahl der Sicherungen (30–2000) und
  Aufbewahrungszeitraum (Woche bis Quartal); alte Stände werden ausgedünnt, der
  Speicherbedarf bleibt begrenzt. Nach einem Absturz ist höchstens das letzte
  Intervall verloren.
- **Sitzung**: Reiter, Fenstergröße und -lage werden gemerkt, der nächste Start
  stellt die Arbeit wieder her. Beim Beenden mit ungespeicherten Änderungen wird
  sofort gesichert.
- **Blockeditor**: Blöcke lassen sich in einem eigenen Bearbeitungsraum öffnen,
  ändern und zurückschreiben.
- **Eigene Blöcke als Datei** speichern und beim Start wieder laden.
- **Blöcke aus der Zwischenablage** erstellen.
- **Linienstärken nach DIN EN ISO 128-2**: neun Breiten von 0,13 bis 2,00 mm,
  Voreinstellung je Objektart, Umschalter „Bildschirm (konstant)“ / „Papier
  (maßstabsgetreu)“ und ein Stärke-Faktor.
- **Dämmung als Linienart**: Schlaufenband, auch an Wänden; Zerlegen ergibt eine
  Linie.
- **Text**: Umrandung dauerhaft anzeigen, Zeilenabstand einstellen,
  Texthintergrund, runde Umrandung auch bei mehrzeiligem Text.
- **Ein Datenordner** für alle Nutzdaten — Einstellungen, Blöcke, Sicherungen,
  Protokolle — frei wählbar und auch portabel neben dem Programm.
- **Zweite Instanz**: Eine von außen geöffnete Zeichnung erscheint als Reiter im
  laufenden Fenster, statt eine Fehlermeldung zu bringen.

### Geändert

- Schraffuren: fünf Muster entfernt, die in Wahrheit Linienarten waren; Zerlegen
  einer Schraffur trifft jetzt die Musterpunkte genau.
- Stutzen an Kreis, Bogen und Ellipse schneidet nur noch die angeklickte Stelle.
- Mittelpunkt von Kreis, Bogen und Ellipse wird nur bei Auswahl gezeichnet.
- Einstellungsfenster in allen Kategorien gleich breit.
- Schnitt-Kote: die Vorschau zeigt das fertige Objekt.

### Behoben

- Umlaute und Sonderzeichen lösten ihre Tastenfunktion nicht aus.
- Spline fing beim Manipulieren an beliebigen Stellen statt an den Stützpunkten.
- Kantengriffe erschienen auch an Objekten, die kein Rechteck sind.
- Der Schwenk lief nach Fokusverlust weiter.
- Fensterkanten ließen sich nur im wiederhergestellten Fenster ziehen.

### Bekannt

- Die Windows-Fassung ist gebaut, aber nicht ausgiebig getestet.
- Die Zeichnungen in `samples/` sind Platzhalter ohne fachliche Prüfung.

## [0.2.0] — 2026-11-05

Das Grundprogramm steht: zeichnen, Wände und Öffnungen, bemaßen, Blätter,
Ausgabe. Erste geschlossene Fassung — das Repository ist noch privat, die erste
öffentliche Fassung wird 0.3.0.

### Neu

- **Zeichnen**: Punkt, Linie, Polylinie, Rechteck, Kreis, Bogen, Ellipse,
  Schraffur und Text, mit Griffen, Auswahl und Undo/Redo
- **Fangen**: Endpunkt, Mittelpunkt, Zentrum, Schnittpunkt, Lot, nächster Punkt,
  Raster, Ortho und 45°-Polarfang, dazu Hilfslinien beim Verweilen auf einem Punkt
- **Architektur**: Wand mit Dicke, Ausrichtung und Öffnungen; Gehrung an
  Anschlüssen; Tür (Dreh-, Schiebe-, zweiflügelig, Öffnung), Fenster, Treppe,
  Volumen — die Bauteile verbinden sich dynamisch
- **Bemaßung**: Einzelmaß, Maßkette, Grundriss- und Schnitt-Kote nach
  DIN 406/1356, Maßtext in mm/cm/m
- **Blöcke**: Systembibliothek (Sanitär, Küche, Möbel, Symbole), eigene
  Projektblöcke, Hotbar mit Vorlagen, Inventar
- **Blätter**: A4–A0 mit Rahmen, Schriftfeld und mehreren Ansichtsfenstern
- **Ausgabe**: PDF (vektoriell), PNG und DXF; **Import** von PDF und DXF
- **Oberfläche**: Ribbon, Layer- und Eigenschaften-Panel, Statusleiste,
  helles und dunkles Design, Deutsch und Englisch
- **KI-Steuerung**: MCP-Server mit 10 Werkzeugen, vollständig headless

### Bekannt

- Die Windows-Fassung wird erstmals gebaut und ist nicht ausgiebig getestet.
- Die Zeichnungen in `samples/` sind Platzhalter ohne fachliche Prüfung.

## [0.1.0] — verworfen

Prototyp. Hat nie richtig funktioniert, wird nicht veröffentlicht und nicht
getaggt.
