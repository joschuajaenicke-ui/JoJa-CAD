# Änderungen

Alle nennenswerten Änderungen an JoJa-CAD. Format nach
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[`docs/VERSIONEN.md`](docs/VERSIONEN.md).

Abschnitte je Version: **Neu**, **Geändert**, **Behoben**. Ein Eintrag ist ein
Satz aus Nutzersicht — was man jetzt anders machen kann, nicht welche Datei
angefasst wurde.

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
