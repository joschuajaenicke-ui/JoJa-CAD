# JoJa-CAD — Bedienung

Die Oberfläche ist deutsch und an Office-Programmen orientiert: Ribbon oben,
Zeichnung in der Mitte, Layer links, Eigenschaften rechts, Hotbar unten.
Die vollständige Tastenbelegung ist im Programm über den Tastatur-Editor
einsehbar und dort frei änderbar (Ribbon ▸ Ansicht ▸ „Tasten").

**Drei Schichten von Tasten** (verbindlich, ausführlich in `docs/TASTEN.md`):

1. **Ziffern `1` … `0`** starten die Hotbar-Slots — ihre Belegung steuert die
   **Hotbar**, nicht die Tastaturbelegung.
2. **Buchstaben und Symbole** kommen aus der **Tastaturbelegung** und sind vom
   Benutzer frei und dynamisch änderbar.
3. **Alle übrigen Tasten** (`Esc`, `Tab`, `Strg`, `Enter`, `Entf`, `F1`–`F12` …)
   und **alle Tastenkombinationen** (`Strg+C`, `Strg+S`, `Strg+Z` …) sind
   **hardcodiert** und nicht änderbar.

## Erste Schritte

1. **Wand** (`G`): Start- und Endpunkt klicken — die Wand läuft als Kette weiter, `ESC` beendet.
2. **Tür** (`H`) / **Fenster** (`J`): in eine Wand klicken. Öffnung, Laibungen und Anschlag
   entstehen automatisch, die Tür richtet sich an der Wand aus.
3. **Maß** (`M`): zwei Punkte und die Maßlinie setzen. **Maßkette**: Punkte klicken, `Enter` beendet.
4. **Hotbar**: Tasten `1`–`0`. Aktives Werkzeug + Klick auf einen leeren Slot verknüpft es.
   Rechtsklick auf einen Slot: Leeren / Verknüpfen / Bearbeiten. Mittlere Taste leert.
5. **Inventar** (`E`): Lager, Blockbibliothek und alle Funktionen — per Drag & Drop auf die Hotbar.
6. **Navigieren**: Mausrad zoomt, mittlere Taste oder `Leertaste`+Ziehen schwenkt.
   `W A S D` schwenkt ebenfalls — als Funktionen „Ansicht oben/links/unten/rechts",
   die in der Tastaturbelegung stehen und umbelegt werden können.
7. **Länge tippen**: nach dem ersten Klick Ziffern eingeben, `Enter` übernimmt den Abstand —
   oder `x;y` für absolute Koordinaten (Semikolon trennt, Komma ist Dezimalzeichen: `2,5;3,0`).
8. **Auswählen**: Klick wählt ein Objekt, `Umschalt`+Klick ergänzt. Ziehen von links nach rechts
   wählt vollständig umschlossene Objekte (Fenster), von rechts nach links auch geschnittene
   (Querfenster). `Strg+C` / `Strg+X` / `Strg+V` kopieren, schneiden aus und fügen 20 mm versetzt ein.
9. **Statusleiste**: `Raster`, `Ortho` und `Fang` schalten per Linksklick; **Rechtsklick** öffnet
   ein Menü mit den einzelnen Fangmodi (Endpunkt, Mittelpunkt, Zentrum, Schnittpunkt, Lot,
   nächster Punkt) bzw. Raster- und Polaroptionen. Ganz rechts schaltet `Stärke`
   zwischen **px** (Linienbreite immer gleich viele Bildschirmpixel, unabhängig vom Zoom) und
   **DPI** (Breite wie im exportierten PDF: `Breite_mm × Pixel pro Papier-mm`, ohne Obergrenze —
   beim Hineinzoomen werden die Linien im selben Verhältnis dicker wie die Zeichnung).
   Die Wahl wird gespeichert.
10. **Rechtsklick in die Zeichnung** öffnet das Objektmenü: Verschieben, Kopieren, Drehen,
    Spiegeln, Zerlegen, Löschen und Eigenschaften für das angeklickte Element. Auf freier
    Fläche erscheint das Flächenmenü (Einfügen, Alles auswählen, Auswahl aufheben, Ansicht
    einpassen).
11. **Ansichtsfenster**: auf einem Blatt über `Blatt ▸ Ansicht` als Rechteck aufziehen, per Klick
    auswählen, mit `V` verschieben oder über `X`/`Y`/`B`/`H` im Panel exakt setzen.
12. **Einstellungen** (Zahnrad) — Einheiten, Rasterweite, Rasterfang, Fangmodi, Polarwinkel,
    Fangradius, Wandstärke, Tür-/Fenster-/Treppenbreite, Texthöhe, Bemaßungsstil und Darstellung.
13. **Platzieren mit Vorschau**: Block aus dem Inventar wählen oder `Strg+V` drücken — die Vorschau
    steht mit ihrer **unteren linken Ecke** am Mauszeiger (also oben rechts davon), folgt der Maus
    und wird mit Klick gesetzt. `R` (die Funktion „Drehen") dreht in 90°-Schritten im
    Uhrzeigersinn (`Umschalt+R` gegen den Uhrzeigersinn), `F` (die Funktion
    „Spiegeln") spiegelt, `ESC` bricht ab. Beide Tasten folgen der
    Tastaturbelegung: wer „Drehen" umlegt, dreht auch die Vorschau mit der neuen
    Taste. Der Einfügepunkt ist exakt der Ankerpunkt der Vorschau.

Der Zeiger ist über der Zeichenfläche ein durchgehendes Fadenkreuz; der Systemzeiger wird dort
ausgeblendet.

## Oberfläche

- **Eigenschaften-Panel** (rechts): tabellarische Liste mit Layer, Farbe, Stärke, Linientyp und
  nicht editierbaren Angaben (Länge, Radius, Fläche). Läuft ein Werkzeug, stehen oben die
  Eigenschaften für das **nächste** Element — einmal gesetzt gilt das für alle folgenden Elemente.
  Bei Mehrfachauswahl erscheinen nur gemeinsame Felder; abweichende Werte zeigen `*variiert*`,
  Änderungen wirken auf die ganze Auswahl.
- **Griffe**: Ausgewählte Elemente zeigen Griffe — Quadrate an Endpunkten und Ecken (Linie, Wand,
  Polylinie, Rechteck; Mittelpunkt bei Kreis/Bogen/Ellipse, **jeder Basispunkt** bei Maß und
  Maßkette) und Rauten in den Kantenmitten
  geschlossener Polygone sowie an der **Mitte der Maßlinie** eines Maßes oder einer Maßkette
  (zieht sie senkrecht zur Messrichtung, die Basispunkte bleiben stehen). Ziehen verschiebt den
  Punkt; bei einer Kante wird sie parallel
  verschoben und die Nachbarkanten folgen, sodass sich Breite/Höhe eines Rechtecks direkt ändern
  lassen. `ESC` bricht das Ziehen ab. Beim Ziehen gilt derselbe **Winkelfang** wie beim
  Erstellen: `Ortho` hält den Punkt waagerecht/senkrecht und der Polarfang rastet auf seinen
  Schrittweiten (45°) ein. Bezug sind die ruhenden Punkte des Elements — bei Linie und Wand das
  andere Ende, bei Polylinie, Spline, Rechteck, Fläche und Volumen **beide** Nachbarn (voriger
  und nächster Stützpunkt). Beide Achsen stehen gleichzeitig zur Verfügung: der **Schnittpunkt**
  zweier Achsen verschiedener Nachbarn ist ein eigenes Fangziel und wird mit doppeltem
  Fangradius gefangen — dort stehen beide angrenzenden Linien zugleich gerade. Daneben rastet
  jede Koordinate einzeln auf die nächstgelegene Achse. Beim **Polarfang** gilt dasselbe für
  alle 45°-Richtungen **beider** Nachbarn: jede Richtung rastet ein (nicht nur die des näheren
  Punkts), und die Kreuzung zweier Richtungen verschiedener Nachbarn ist ebenfalls ein Fangziel
  mit doppeltem Fangradius. Dasselbe gilt für das Verschieben mit der Maus.
  Auch der **erweiterte Punktfang** arbeitet dabei: bleibt der Zeiger auf einem Fangpunkt
  stehen, wird dieser gehalten und seine Hilfslinien fangen den gezogenen Punkt.
- **Linien** werden als Kapsel gezeichnet: Rechteck plus Halbkreis an beiden Enden. Dadurch sind
  auch die Übergänge in Polylinien, Bögen und an Wandanschlüssen rund statt eckig.
- **Ribbon** lässt sich über den Pfeil rechts in der Kopfzeile einklappen; die Reiter bleiben sichtbar.
- **Linkes Panel** (`Layers`) und **rechtes Panel** (`Eigenschaften`) klappen über den Pfeil in ihrer
  Kopfzeile zu einer schmalen Leiste zusammen — ein Klick auf den Pfeil holt sie zurück.
- Das rechte Panel zeigt die **Eigenschaften der Auswahl**; ohne Auswahl die der Datei bzw. des
  Blatts. Die Werte stehen als Tabelle mit fester Namensspalte und Zebra-Streifen untereinander.
- **Hotbar-Vorlagen** wählt man über das Dropdown unten links; dort lassen sich neue Vorlagen
  anlegen (Kopie der aktuellen), umbenennen und löschen.

## Nachweis für den DPI-Modus

`cargo run -p jjcad-core --example lw_test -- /tmp/lw.jjcad` erzeugt zwei parallele Linien
(0,5 mm Strich, 100 mm Abstand, Maßstab 1:100) — im DPI-Modus muss der weiße Streifen
zwischen ihnen genauso breit sein wie die Linien selbst.
