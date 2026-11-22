# Änderungen

Alle nennenswerten Änderungen an JoJa-CAD. Format nach
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[`docs/technik/VERSIONEN.md`](docs/technik/VERSIONEN.md).

Abschnitte je Version: **Neu**, **Geändert**, **Behoben**. Ein Eintrag ist ein
Satz aus Nutzersicht — was man jetzt anders machen kann, nicht welche Datei
angefasst wurde.

## [Unveröffentlicht]

### Neu
- Wurde eine Zeichnung außerhalb von JoJa-CAD verändert, während ungespeicherte Arbeit im Sicherungsordner liegt, fragt der Programmstart, welche Fassung weiterverwendet werden soll; bis zur Entscheidung gilt die Datei, überschrieben wird nichts.
- Wird ein Reiter mit „Verwerfen“ geschlossen, bleibt der letzte Arbeitsstand als verworfener Stand im Sicherungsordner liegen; die Einstellungsseite zeigt die Zahl der verworfenen Stände an, zurückgeholt wird nichts.

### Geändert
- Die Sicherung bewahrt Stände nach dem Verfahren Geometric Retention auf: mit festem Platzvorrat, gleichbleibender Auflösung über die Nutzungsdauer und einem Alter in Zyklen statt in Kalenderzeit.
- Neue Vorgaben sind 150 Plätze, eine Reichweite von einer Woche und ein Intervall von einer Minute; der Aufbewahrungszeitraum ist als Tag, Woche, Monat oder Quartal wählbar, die Wahl „Jahr“ entfällt.
- Der neueste Stand einer Zeichnung bleibt bis zum Ende des Aufbewahrungszeitraums erhalten und verfällt danach mit dem Verlauf; der älteste Stand rückt nach, sobald der Verlauf tief genug ist.
- Die Obergrenze des Sicherungsordners beträgt max(Plätze, 2 × Zeichnungen), sodass jede Zeichnung Platz für ihren neuesten und ihren ältesten Stand behält.
- Die Anzeige „Abstand der Stände“ rechnet nach der Formel des neuen Verfahrens.
- Das Speichern der Zieldatei ist atomar über eine temporäre Datei, fsync und rename; ein Absturz mitten im Speichern hinterlässt keine halbe Datei mehr.

## [0.2.4] — 2026-11-22

Große Runde an Blöcken, Maßketten und Papier. Die Pakete sind jetzt wirklich
portabel: sie legen ihre Daten neben dem Programm ab.

### Blöcke

- Blöcke haben eine **Größe**: fest, in Papiergröße oder mit dem
  Zeichnungsmaßstab skalierend; sie tragen eine **Maßeinheit** und sitzen mit
  ihrem **Basispunkt** am Mauszeiger.
- Eigene Blöcke lassen sich im Panel **umbenennen**, per Rechtsklick
  **kopieren** und erst nach Rückfrage löschen — auf Wunsch mit Zerlegen der
  Instanzen.
- Systemblöcke gelten beim Laden in der Fassung der Bibliothek; Punktsymbole
  sitzen an Punkt, Linienenden und in den Symbolblöcken.

### Maßketten

- Jeder Abschnitt ist eine **Linie**, jeder Messpunkt trägt sein **Symbol**
  (Vorgabe Schrägstrich), die Marke liegt auf der Maßlinie, Begrenzungszeichen
  stehen nur, wo sie hingehören, und die Verlängerung zu den Messpunkten ist
  abschaltbar.

### Neu

- **Pfad-Werkzeug**: Linien und Bögen am Stück zeichnen, Verbinden aus der
  Auswahl.
- **PNG-Export eines Bereichs** über die Diagonale, mit Auflösung und
  Hintergrund.
- **Wiederherstellen-Fenster**: Zeichnungen und Stände aus dem Sicherungsordner
  mit Verlauf, Vorschau und Öffnen.
- **Planrahmen** als Block mit Abdeckfläche in A4 und A3 hoch; der Maßstab
  gehört zu jedem Plan.
- **Blattmanager** als Seitenpanel.
- **Raumstempel** aus zwei Punkten, Name direkt in der Zeichnung änderbar.
- **Hilfslinie** per Rechtsklick mitten in der Eingabe; Abstand zu einer
  bestehenden Wand per Rechtsklick und getippter Länge.
- **Lotfang** rechtwinklig auf vorhandene Linien und auf die Wandumgrenzung
  (Längs-, Stirn- und Laibungskanten).
- **Ansichtsfenster** zeigen nur die gewählten Layer; das
  Standard-Ansichtsfenster füllt das ganze Blatt.
- **Sprache** in den Einstellungen als Auswahlfeld.
- **Modell und Papierraum** mit eigener Eingabeeinheit; Text rastet auf die
  nächstgrößere Leiterstufe.

### Geändert

- **Die portablen Pakete legen ihre Daten im Programmordner ab** (Ordner
  `data/`). Bisher fehlte die Schalterdatei, deshalb landeten Einstellungen,
  eigene Blöcke und Sicherungen im Benutzerprofil. Wer eine ältere Fassung aus
  demselben Ordner benutzt hat, startet mit leeren Einstellungen — die alten
  liegen weiter im Benutzerprofil.
- Bögen und Ellipsen stehen im PDF als echte Bézier-Kurven.
- Der **Wandkörper** ist die Trefferfläche für Auswahl und Verlängern, nicht
  mehr die Basislinie; Öffnungen wandern beim Kopieren mit.
- Die Tangential-Fangmarke sitzt am Berührpunkt von Linie und Kreis.
- Die Tastenbelegung zeigt **Ebenen** (Umschalt, Strg, Alt).
- Seitenleiste immer sichtbar, eigenes Scrollen je Panel.
- Eigenschaften-Panel: Objektkopf einmal, dann allgemeine und objekteigene
  Gruppen.
- Nach dem Öffnen einer Datei holt sich JoJa-CAD in den Vordergrund.

### Behoben

- Ein Layer mit Objekten fragt vor dem Löschen nach.
- Das Umbenennen-Feld füllt die Zeile, ohne dass die Liste springt.
- Sicherungen ohne Manifest: Metadaten im Stand, Index im Speicher.

### Bekannt

- **Windows- und macOS-Pakete folgen mit der nächsten Fassung.** Der Build
  scheitert derzeit an einer Abhängigkeit der Bild-Zwischenablage, die nur für
  Linux deklariert ist; die Oberfläche selbst ist davon nicht betroffen.
- Die **macOS-Fassung ist gebaut, aber ungeprüft** — Rückmeldung willkommen.
- Die Windows-Fassung ist gebaut, aber nicht ausgiebig getestet.
- Unter Wayland wird die Fensterposition nicht gespeichert (siehe
  `docs/benutzer/BEKANNTE-GRENZEN.md`).
- Die Zeichnungen in `samples/` sind Platzhalter ohne fachliche Prüfung.

## [0.2.3] — 2026-11-17

Runde an Ebenen, Blöcken und Auswahl; dazu die erste macOS-Fassung.

### Neu

- **Parallele**: Linie oder Polylinie im gemerkten Abstand erzeugen.
- **Layer-Manager** überarbeitet: Pläne mit vier Standardlayern, Pläne
  einklappbar und sperrbar, Schalter je Plan.
- **Panels auf beiden Seiten**, Leisten mit denselben Symbolen.
- **Blöcke**: Name und Gruppe im Eigenschaften-Panel und im Fenster, Bearbeiten
  über das Rechtsklickmenü, Blockbibliothek in drei Reitern.
- **Zeichenfarben** in den Einstellungen anpassbar, in drei Gruppen und einer
  gemeinsamen Farbliste.
- **Grundriss-Kote**: Textposition Oben, Unten, Links, Rechts.
- **Auswahl**: Mehrfachauswahl lässt sich nach Objektart filtern; bei gemischter
  Auswahl zeigt das Panel nur die gemeinsamen Eigenschaften.
- **Sicherung**: Aufbewahrung nach Geometrie, Nachfrage bei Konflikten,
  verworfene Stände bleiben einsehbar.
- **Über-Dialog** mit Verweisen auf das Projekt und die Downloads.
- **Windows: die Schriften stecken in der `.exe`** — eine einzelne Datei, der
  Ordner `fonts/` entfällt.
- **macOS-Fassung** als `.app` (Apple Silicon und Intel), ungeprüft.

### Geändert

- Die Kurventoleranz gilt auch für Splines.
- Maßketten: Messpunkte bleiben nach dem Ziehen in Messreihenfolge und fangen
  aufeinander.
- Flächen decken die Umrandung darunterliegender Objekte zu.
- Bilder: nur die Kanten sind anklickbar; das Auswahlfenster prüft das schräge
  Rechteck.
- Fensterschatten dockt ohne Abstand an und ist abschaltbar.
- Rechtsklick in einer Mehrfachauswahl gilt für alle Objekte der Auswahl.
- Einstellungen aufgeräumt: Reihenfolge, Tooltips, Akzentfarbe.
- Auswahlfelder sind so hoch wie Eingabefelder.

### Behoben

- Tür und Fenster: die Laibung fängt nicht mehr; der Basispunkt von Türen bleibt
  auf der Wandkante.
- Zerlegen mehrerer Objekte aus einem Stand lässt die Umrandungen, wie sie
  gezeichnet sind.
- Der Selbst-Fang beim Bearbeiten ist vollständig entfernt.
- Das Rechtsklickmenü schließt bei Ansichtsbewegung und Werkzeugstart.
- Seiten-Panels verschieben die Zeichnung nicht mehr.
- Der Blockeditor speichert den Block, nicht die Zeichnung.
- Der Zeiger über einem Fenster gilt als außerhalb der Zeichenfläche.

### Bekannt

- Die **macOS-Fassung ist gebaut, aber ungeprüft** — Rückmeldung willkommen.
- Die Windows-Fassung ist gebaut, aber nicht ausgiebig getestet.
- Unter Wayland wird die Fensterposition nicht gespeichert (siehe
  `docs/benutzer/BEKANNTE-GRENZEN.md`).
- Die Zeichnungen in `samples/` sind Platzhalter ohne fachliche Prüfung.

## [0.2.2] — 2026-11-11

Erste Fassung mit fertigen Paketen: **AppImage** für Linux. Der größte Sprung
steckt in der Geschwindigkeit — Culling und die dynamische Glättung aller Kurven
machen große Zeichnungen spürbar flüssiger. Dazu die umfangreichste
Funktionsrunde bisher an Maßketten, Fang, Ansichtsfenstern und Einstellungen.

### Deutlich schneller

- **Culling**: Gezeichnet wird nur, was im Bild liegt — der Rest wird vor dem
  Malen verworfen. PDF, PNG und DXF bleiben vollständig.
- **Dynamische Glättung aller Kurven**: Bögen, Ellipsen, Splines und Dämmbänder
  werden nur so fein abgetastet, wie die Ansicht es braucht — herausgezoomt
  wenige Punkte, hineingezoomt feiner. Druck und Exporte behalten ihre feste
  Dichte.

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
