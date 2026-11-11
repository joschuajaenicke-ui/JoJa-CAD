# Änderungen

Alle nennenswerten Änderungen an JoJa-CAD. Format nach
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[`docs/technik/VERSIONEN.md`](docs/technik/VERSIONEN.md).

Abschnitte je Version: **Neu**, **Geändert**, **Behoben**. Ein Eintrag ist ein
Satz aus Nutzersicht — was man jetzt anders machen kann, nicht welche Datei
angefasst wurde.

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
