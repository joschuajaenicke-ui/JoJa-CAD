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
   Läuft das **Einfügen eines Blocks**, nimmt der angeklickte Slot den Block auf (der Slot
   zeigt dann die Miniaturaansicht des Blocks, sein Tooltip den Blocknamen); die Taste `1`–`0`
   fügt ihn ein. Dasselbe gilt für eine Taste im Fenster **Tastaturbelegung** und für einen
   Lagerplatz: Klick bei laufender Vorschau legt den Block dorthin.
   Rechtsklick auf einen Slot: Leeren / Verknüpfen / Bearbeiten. Mittlere Taste leert.
5. **Inventar** (`E`): Lager, Blockbibliothek und alle Funktionen — per Drag & Drop auf die Hotbar.
   Ein Block lässt sich auch per Klick auf einen Slot oder Lagerplatz legen, solange seine
   Einfüge-Vorschau läuft.
6. **Navigieren**: Mausrad zoomt, mittlere Taste oder `Leertaste`+Ziehen schwenkt.
   `W A S D` schwenkt ebenfalls — als Funktionen „Ansicht oben/links/unten/rechts",
   die in der Tastaturbelegung stehen und umbelegt werden können. Wie sich eine
   Funktion bewegt, steht in den Einstellungen unter **Bedienung** in der Zeile
   **„Verschieben"**: **Kontinuierlich** (Vorgabe) hält die Ansicht in Bewegung,
   solange die Taste oder der Hotbar-Slot gedrückt bleibt; **Sprunghaft** rückt je
   Druck um einen Schritt weiter. Das Tempo steht darunter in der Zeile
   **„Geschwindigkeit der Ansicht"** als Faktor (0,1× bis 5×, Vorgabe 1×) — er gilt für
   beide Modi. Die 1× sind das Grundtempo des Programms: ein Sprung rückt 90 px
   weiter, der Dauerlauf fährt 720 px/s (T0146, 50 % mehr als vorher).
7. **Länge tippen**: nach dem ersten Klick Ziffern eingeben, `Enter` übernimmt den Abstand —
   oder `x;y` für absolute Koordinaten (Semikolon trennt, Komma ist Dezimalzeichen: `2,5;3,0`).
8. **Auswählen**: Klick wählt ein Objekt, `Umschalt`+Klick ergänzt. Ziehen von links nach rechts
   wählt vollständig umschlossene Objekte (Fenster), von rechts nach links auch geschnittene
   (Querfenster). `Strg+C` / `Strg+X` / `Strg+V` kopieren, schneiden aus und fügen 20 mm versetzt ein.
   `Strg+Umschalt+V` legt die Zwischenablage als **Block** ab: der Block entsteht ohne
   Namensdialog als „Unbenannt 1“, „Unbenannt 2“, … (kollisionsfrei) in der
   Benutzerbibliothek und steht sofort als Einfüge-Vorschau am Zeiger.
9. **Statusleiste**: `Raster`, `Ortho` und `Fang` schalten per Linksklick; **Rechtsklick** öffnet
   ein Menü mit den einzelnen Fangmodi (Endpunkt, Mittelpunkt, Zentrum, Schnittpunkt, Lot,
   nächster Punkt) bzw. Raster- und Polaroptionen. Ganz rechts schaltet `Stärke`
   zwischen **Bildschirm (konstant)** (Linienbreite immer gleich viele Bildschirmpixel,
   unabhängig vom Zoom; ein Faktor in Einstellungen ▸ Darstellung skaliert sie) und
   **Papier (maßstabsgetreu)** (Breite wie im exportierten PDF: `Breite_mm × Pixel pro
   Papier-mm`, ohne Obergrenze — beim Hineinzoomen werden die Linien im selben Verhältnis
   dicker wie die Zeichnung). Im Papier-Modus bestimmt der **Maßstab** der Zeichnung
   (Eigenschaften der Zeichnung, 1:N) die Strichbreite. Die Wahl wird gespeichert. Links davon stehen zwei weitere Schalter:
   **Verschieben** (Vierfach-Pfeil) schaltet **„Maus verschiebt"** ein und aus — dieselbe
   Einstellung wie Einstellungen ▸ Bedienung; ausgeschaltet wandert nur noch das Werkzeug
   „Verschieben". **Drehpunkt** (Fadenkreuz im Quadrat) wechselt beim Einfügen zwischen
   **Basispunkt** (Zeichen `B`) und **Mittelpunkt** (Zeichen `M`) — ebenfalls dieselbe
   Einstellung wie in den Einstellungen, ein Wechsel wirkt sofort auf eine laufende Vorschau.
10. **Rechtsklick in die Zeichnung** öffnet das Objektmenü: Verschieben, Kopieren, Drehen,
    Spiegeln, Zerlegen, Löschen und Eigenschaften für das angeklickte Element. Bei einer
    Schraffur genügt der Rechtsklick **irgendwo in die Fläche**. Auf freier
    Fläche erscheint das Flächenmenü (Einfügen, Alles auswählen, Auswahl aufheben, Ansicht
    einpassen).
    **Zerlegen** löst ein Objekt in seine Einzelteile auf. Ohne Auswahl ist es ein
    Klick-Werkzeug, und bei einer Schraffur genügt der Klick **irgendwo in die Fläche** —
    nicht nur auf den Rand: die Musterlinien werden zu einzelnen Linien, die Musterpunkte
    (Sand, Putz) zu kleinen gefüllten Kreisen in der Musterfarbe. Das Bild bleibt dabei gleich.
11. **Ansichtsfenster**: auf einem Blatt über `Blatt ▸ Ansicht` als Rechteck aufziehen. Es ist
    ein normales Blattobjekt: per Klick oder Auswahlfenster wählen, an den vier Eckgriffen
    aufziehen, mit der Maus verschieben (Einstellungen ▸ Bedienung ▸ „Maus verschiebt") oder
    mit dem Werkzeug „Verschieben" über einen Richtungsvektor, mit `Entf` löschen. `X`/`Y`/`B`/`H`
    stehen im Eigenschaften-Panel.
12. **Einstellungen** (Zahnrad) — links eine Tabelle der Kategorien (Einheiten und Maßstab,
    Raster, Fangen, Architektur, Bemaßung, Bedienung, Darstellung, Voreinstellungen), darüber die Suche, rechts die
    Einstellungen der gewählten Kategorie. Die Suche filtert Kategorien **und** Zeilen und springt
    auf die erste passende Kategorie.
13. **Platzieren mit Vorschau**: Block aus dem Inventar wählen oder `Strg+V` drücken — die Vorschau
    steht mit ihrer **unteren linken Ecke** am Mauszeiger (also oben rechts davon), folgt der Maus
    und wird mit Klick gesetzt. `R` (die Funktion „Drehen") dreht in 90°-Schritten im
    Uhrzeigersinn (`Umschalt+R` gegen den Uhrzeigersinn), `F` (die Funktion
    „Spiegeln") spiegelt, `ESC` bricht ab. Beide Tasten folgen der
    Tastaturbelegung: wer „Drehen" umlegt, dreht auch die Vorschau mit der neuen
    Taste. Der Einfügepunkt ist exakt der Ankerpunkt der Vorschau.
    Läuft das Einfügen aus der Zwischenablage und wird ein Hotbar-Slot (oder ein
    Lagerplatz bzw. eine Taste der Tastaturbelegung) angeklickt, entsteht daraus
    ebenfalls ein Block „Unbenannt N“ — der Slot trägt ihn danach.
    Wird während der Vorschau eine **andere Funktion** aktiviert (Ribbon, Hotbar,
    Inventar oder die Schnellauswahl), bricht das Einfügen ab — Vorschau und
    Werkzeug laufen nie gleichzeitig. Die Schnellauswahl wirkt dabei nur als
    Abbruch und wird nicht selbst aktiv.
    Beim Einfügen aus der **Zwischenablage** bestimmt die Einstellung **Bedienung ▸
    „Drehpunkt"**, welcher Punkt dabei am Fadenkreuz steht: **„Mittelpunkt"** (Vorgabe)
    hält die untere linke Ecke der gedrehten Auswahl am Fadenkreuz, die Vorschau bleibt
    also immer oben rechts davon; **„Basispunkt"** hält den mitkopierten Basispunkt dort —
    die Vorschau dreht um ihren Mittelpunkt und springt deshalb beim Drehen um das
    Fadenkreuz. Ein Wechsel in den Einstellungen wirkt sofort, auch auf eine bereits
    laufende Vorschau.
14. **Bogen**: der Pfeil am Ribbon-Knopf „Bogen" öffnet die beiden Eingabemodi — **„Drei Punkte"**
    (Start, Ende, Punkt auf dem Bogen, Vorgabe) und **„Mittelpunkt, Start, Ende"**: erst der
    Mittelpunkt, dann der Startpunkt, der zugleich den **Radius** festlegt, zuletzt der Endpunkt.
    Der Bogen läuft vom Startpunkt im mathematisch positiven Drehsinn zum Endpunkt. Derselbe
    Modus steht beim Platzieren im Eigenschaften-Panel unter **„Eingabemodus"**.
15. **Flächen schneiden und vereinigen** (Ribbon ▸ Zeichnen ▸ Gruppe „Füllen"):
    **„Schneiden"** fragt Punkte für die Schnittlinie ab — `Enter` (oder `ESC`) teilt die
    Füllfläche in zwei Teilflächen. Die Linie darf außerhalb der Fläche beginnen und enden;
    sie muss den Rand an genau zwei Stellen übertreten. **„Vereinigen"** wirkt sofort auf die
    Auswahl: zwei oder mehr **angrenzende** Füllflächen werden eine, Muster, Randlinie und
    Ebene kommen von der ersten. Getrennte Flächen melden einen Fehler, statt still zwei
    Flächen zu erzeugen.

16. **Bild einfügen** (Ribbon ▸ Start ▸ Ausgabe ▸ „Bild einfügen"): PNG oder JPEG auswählen,
    dann die **untere Kante** klicken — Anfang und Ende bestimmen Lage, Drehung und Breite.
    Die Höhe folgt aus dem **Seitenverhältnis der Datei** und der Breite; das Bild wird also
    nie verzerrt. Im Eigenschaften-Panel steht dafür die Zeile **„Seitenverhältnis"** mit dem
    Häkchen „sperren" (Vorgabe an): ohne Häkchen lässt sich die Höhe frei eingeben, und die
    Breite bleibt unabhängig. Die Bilddatei wird **in die Zeichnung eingebettet** (Base64),
    die `.jjcad`-Datei bleibt also gültig, wenn die Vorlage verschoben oder gelöscht wird.
    Das Bild liegt als Hintergrund hinter Flächen und Linien (eigene Ebene „Bilder", soweit
    die Ebene nicht selbst gewählt wurde); seine vier Rahmenecken sind Griffe — die unteren
    verschieben die Kante (Lage, Drehung, Breite), die oberen ziehen die Höhe (bei gesperrtem
    Verhältnis wächst die Breite mit). Ein Bild lässt sich nicht zerlegen.

17. **Dateien ins Fenster ziehen** (Drag an Drop): eine Datei aus dem Dateimanager auf das
    Programmfenster ziehen. Solange sie schwebt, nennt ein Hinweis oben in der Mitte die Datei
    und was beim Loslassen geschieht — oder dass sich das Format nicht importieren lässt.
    Beim Ablegen entscheidet die **Endung**: `.jjcad` öffnet die Zeichnung in einem Reiter,
    `.dxf` wird in die laufende Zeichnung importiert, `.pdf` öffnet die Seitenwahl
    („PDF einfügen"), `.png`/`.jpg`/`.jpeg` startet das Bildwerkzeug mit der Datei. Jede andere
    Endung wird mit „Datei · Format nicht unterstützt" abgelehnt und ändert nichts. Der Weg ist
    derselbe wie nach einer Auswahl im Dateidialog.

    **Unter Linux mit Wayland kommt der Drop derzeit nicht an** — die Fensterbibliothek unter
    egui (winit 0.30) kennt ihn dort nicht. Das ist kein Fehler des Programms; Ursache,
    Bedingungen und Prüfweg stehen in [`docs/WAYLAND-DATEI-DROP.md`](WAYLAND-DATEI-DROP.md).

Der Zeiger ist über der Zeichenfläche ein durchgehendes Fadenkreuz; der Systemzeiger wird dort
ausgeblendet.

## Oberfläche

- **Eigenschaften-Panel** (rechts): tabellarische Liste mit Layer, Farbe, Stärke, Linientyp und
  nicht editierbaren Angaben (Länge, Radius, Fläche). Läuft ein Werkzeug, stehen oben die
  Eigenschaften für das **nächste** Element — einmal gesetzt gilt das für alle folgenden Elemente.
  Bei Mehrfachauswahl erscheint **je Objektart** ein Abschnitt mit genau den Feldern des
  einzelnen Objekts (z. B. Dicke, Höhe, Schraffur, Füllfarbe bei Wänden); abweichende Werte
  meldet die Kopfzeile als `*variiert*`, jede Änderung wirkt auf alle Objekte dieser Art in
  der Auswahl.
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
  Der Polarfang greift nur in einem **Band** um den Strahl: seitlich höchstens der Fangradius
  (Vorgabe 12 px, bildschirmkonstant), unabhängig vom Abstand. Ohne diese Grenze wäre der
  Fangbereich ein Kegel, der mit dem Abstand breiter wird — ein 6°-Fenster ist auf 20 m über
  2 m breit, und der Punkt rastete weit neben der gemeinten Richtung ein.
  Auch der **erweiterte Punktfang** arbeitet dabei: bleibt der Zeiger auf einem Fangpunkt
  stehen, wird dieser gehalten und seine Hilfslinien fangen den gezogenen Punkt.
- **Linien** werden als Kapsel gezeichnet: Rechteck plus Halbkreis an beiden Enden. Dadurch sind
  auch die Übergänge in Polylinien, Bögen und an Wandanschlüssen rund statt eckig.
- **Ribbon** lässt sich über den Pfeil rechts in der Kopfzeile einklappen; die Reiter bleiben sichtbar.
- **Linienstärke neuer Objekte** (Einstellungen ▸ Voreinstellungen): eine Zeile je Objektart
  (Wände, Türen, Fenster, Treppen, Bemaßung, Text, Schraffur, Möbel, Achsen, Hilfslinien) mit
  der Stärken-Liste nach DIN EN ISO 128-2 und freiem Wert. Programm-Vorgabe ist **0,18 mm** für
  alle; „Vorgaben zurücksetzen" stellt jede Zeile darauf zurück. Die Werte gelten für **neue**
  Zeichnungen und für neu angelegte Standardebenen — die Ebenen einer geöffneten Zeichnung
  behalten ihre Breiten. Eine noch leere Zeichnung übernimmt die Änderung sofort, damit die
  Wirkung ohne „Neu" sichtbar ist. Ein neues Objekt erbt die Stärke seiner Ebene; das
  Eigenschaften-Panel zeigt beim Erzeugen genau diesen Wert. Gespeichert in `prefs.json` (`layer_weights`); die Farbe je Ebene
  bleibt unberührt.
- **Helles und dunkles Design** (Einstellungen ▸ Darstellung ▸ „Design"): **System** ist der
  Programmstandard und folgt dem hellen oder dunklen Farbschema des Betriebssystems (unter KDE
  dessen Fensterhintergrund, unter GTK „prefer dark"), **Hell** und **Dunkel** überstimmen es.
  Die Wahl wirkt sofort und steht in `prefs.json` (`design: system | hell | dunkel`). Die
  Zeichenfläche bleibt in beiden Designs Papier — es wechselt nur die Bedienoberfläche.
- **Fenster ohne Titelleiste** (Einstellungen ▸ Bedienung ▸ „Fensterrahmen", *experimentell*): meldet
  dem Fenstermanager „keine Dekoration" — unter KDE/X11 über `_MOTIF_WM_HINTS`, unter
  KDE/Wayland über die Anfrage nach *client-seitiger* Dekoration, unter Windows als rahmenloses
  Fenster. Dann fehlen die Knöpfe der Titelleiste, und JoJa-CAD übernimmt sie selbst: ganz oben
  rechts **Minimieren**, **Vergrößern/Wiederherstellen** und **Schließen**; ein Zug auf die
  Kopfzeile verschiebt das Fenster, ein Doppelklick vergrößert bzw. stellt wieder her. Die
  **Größe** ändert ein Zug an den acht Pixel breiten Zonen am Fensterrand: der Zeiger wird dort
  sofort zum Größenpfeil, die Kante leuchtet nach kurzem Verweilen auf (0,1 s — ein Zeiger, der
  nur über den Rand streicht, soll nichts aufblitzen lassen). Während die Größe gezogen wird,
  bleibt sie stehen und verschwindet kurz nach dem letzten Schritt — oder sofort, wenn der
  Zeiger wieder ins Fenster wandert. In der Ecke unten rechts steht ein sichtbarer Griff. Die Ecke oben rechts gehört den Fensterknöpfen (kein Größe-Ziehen, sonst wäre
  „Schließen“ nicht zu treffen). Die Wahl steht in `prefs.json`
  (`fensterrahmen: system | ohne_titelleiste`) und wirkt beim Umschalten sofort, ohne Neustart.
- **Dateireiter** (Streifen unter dem Ribbon, **zwei getrennte Bereiche**): links auf dunklem
  Band jede geöffnete Zeichnung als Reiter mit Dateisymbol, der aktive als Karte mit
  Akzentlinie; rechts in einer abgesetzten Fläche die Blätter der aktiven Datei als kleinere
  Chips (mit Abstand zueinander). „Neu“ hängt einen Reiter an, statt die aktuelle Datei zu
  schließen — der bisherige Stand bleibt in seinem Reiter stehen. `×` schließt einen Reiter;
  bei ungespeicherten Änderungen fragt „Datei schließen“ nach: „Speichern und schließen“,
  „Verwerfen“ oder „Abbrechen“. Ohne Speicherort fragt „Speichern und schließen“ im
  Dateidialog nach dem Ziel und legt den Reiter erst nach dem Sichern ab. Rechts vom Trenner stehen
  die **Blattreiter** der aktiven Datei: `Modell`, die Blätter und `+` für ein neues Blatt.
- **Linkes Panel** (`Layers`) und **rechtes Panel** (`Eigenschaften`) klappen über den Pfeil in ihrer
  Kopfzeile zu einer schmalen Leiste zusammen — ein Klick auf den Pfeil holt sie zurück.
- Das rechte Panel zeigt die **Eigenschaften der Auswahl**; ohne Auswahl die der Datei bzw. des
  Blatts. Die Werte stehen als Tabelle mit fester Namensspalte und Zebra-Streifen untereinander.
- **Hotbar-Vorlagen** wählt man über das Dropdown unten links; dort lassen sich neue Vorlagen
  anlegen (Kopie der aktuellen), umbenennen und löschen.
- **Leistungsanzeige** (Umschalt+F3, kleines Feld oben rechts in der Zeichenfläche): Abstand und
  Rechenzeit der gezeichneten Bilder mit Perzentilen (p50 · p95 · p99) sowie die Aufteilung der
  Rechenzeit in Abschnitte (Oberfläche, Displayliste, Fangen, Zeichnen, Bedienung, Fenster) mit
  Anteil und den Posten Füllungen/Linien/Text. Darunter Zeichnungsgröße, Grafikelemente,
  Trefferquote des Zwischenspeichers, Zoom, Speicher und Threads. Eine Bilder-pro-Sekunde-Zahl
  gibt es bewusst nicht: JoJa-CAD zeichnet nur, wenn ein Bild gebraucht wird, Pausen sagen
  nichts über die Zeichenzeit. Die Anzeige nimmt keine Klicks an.

## Daten, Sicherung und Sitzung

### Wo die Daten liegen

Alle Nutzdaten liegen in **einem** Ordner: `prefs.json` (Einstellungen),
`sitzung.json` (offene Reiter), `blocks/` (eigene Blöcke), `sicherungen/`
(Stände) und `log/`. Die Zeichnungen selbst bleiben, wo Sie sie speichern.

Den Ordner bestimmt die erste zutreffende Angabe:

1. `jjcad --daten-dir <Pfad>`
2. die Umgebungsvariable `JJCAD_DATA_DIR`
3. eine Datei `portable.txt` neben dem Programm → `<Programmordner>/data`
4. der Plattformstandard (Linux `~/.local/share/JoJa-CAD`, Windows
   `%APPDATA%\JoJa-CAD`, macOS `~/Library/Application Support/JoJa-CAD`)

Der Knopf **Datenordner** (Menüband *Datei*, Gruppe *Projekt*) öffnet ihn im
Dateimanager; sein Tooltip zeigt den Pfad. Die drei Unterordner lassen sich
unter Einstellungen → **Speicherorte** ändern — relativ zum Datenordner oder
absolut, je Feld mit **Zurücksetzen**. Ein nicht beschreibbarer Pfad wird beim
Übernehmen abgelehnt, nicht still gesetzt.

Läuft bereits eine Instanz auf demselben Ordner, wird **nicht** gemeckert: eine
zweite Zeichnung, die Sie von außen öffnen (Doppelklick, Datei als Argument),
landet als Reiter im laufenden Fenster und der zweite Prozess beendet sich; ein
Start ohne Datei holt das laufende Fenster nur nach vorn. Zwei Instanzen auf
einer Wurzel würden sich Sitzung und Sicherung gegenseitig überschreiben —
deshalb gibt es nur eine. Wer bewusst getrennt arbeiten will, startet mit einem
anderen Datenordner (`--daten-dir`).

### Speichern und Sitzung

Gespeichert wird **nur** auf Befehl (`Strg+S`, „Speichern unter"). Die Sicherung
schreibt nie in die Zieldatei.

Beim Beenden wird **nicht** nach dem Speichern gefragt: Der Zustand — offene
Reiter in ihrer Reihenfolge, je Reiter Ansicht und aktives Blatt, aktiver Reiter,
Fenstergröße, Vollbild, „maximiert“ und -lage — steht in `sitzung.json` und wird
beim nächsten Start wiederhergestellt. Unter Wayland kommt alles außer der Lage
zurück: dort darf kein Programm seine Fensterposition setzen
(`docs/BEKANNTE-GRENZEN.md`, Grenze 7). **Ungespeicherte Änderungen gehen dabei nicht verloren:** die
Sicherung legt sie beim Beenden als Stand ab, und der nächste Start holt sie
zurück — mit demselben Speicherort, aber als **geändert** markiert. Erst
„Speichern“ schreibt in die Zieldatei. Auch eine Zeichnung, die nie gespeichert
wurde, kommt als „Unbenannt“ zurück. **Nicht** wiederhergestellt werden Auswahl,
Undo-Verlauf, Werkzeugwahl und Zwischenablage.

Das Schließen eines **Reiters** (`×`) fragt dagegen nach, sobald der Inhalt von
der Zieldatei abweicht: „Speichern und schließen", „Verwerfen", „Abbrechen".
Ohne Speicherort fragt „Speichern und schließen" nach dem Ziel und legt den
Reiter erst nach dem Sichern ab.

### Sicherung

Ändert sich eine Zeichnung, legt die Sicherung im eingestellten Intervall einen
**Stand** im Sicherungsordner ab und dünnt die Stände nach Alter aus: vorne
dicht, hinten grob. Vorgabe: 1 Minute, 100 Plätze, Reichweite ein Quartal. Ohne
Änderung entsteht kein Stand.

Einstellungen → **Sicherung**: *Verlauf aufbewahren* (aus = je Zeichnung nur der
neueste Stand, vorhandene Sicherungen bleiben), *Anzahl der Sicherungen*
(30–2000), *Aufbewahrungszeitraum* (Woche bis Jahr), *Sicherung alle* (1–240
Minuten), dazu der mittlere Abstand der Stände, der belegte Speicher und der
Hinweis zum Netzlaufwerk. Jede Zeile erklärt sich über ihr Infozeichen; über der
Tabelle steht das Konzept in drei Sätzen.

**Sicherungen ansehen**: Einstellungen → Sicherung → **Sicherungsordner** öffnet
den Ordner im Dateimanager (dieselbe Funktion wie *Sicherungen öffnen* in
Inventar und Hotbar). Jeder Stand ist eine gewöhnliche `.jjcad`-Datei — öffnen
per Doppelklick oder in das Fenster ziehen. Ein so geöffneter Stand trägt keinen
Speicherort; gespeichert wird er erst über „Speichern unter“.

Die Sicherung schützt gegen Absturz und vergessene Speicherung, **nicht** gegen
einen Plattenausfall. Dafür den Sicherungsordner (Einstellungen → Speicherorte →
*Sicherungen*) auf ein Netzlaufwerk mit redundanter Sicherung legen.

### Eigene Blöcke

„Block aus Auswahl" im Blockmanager legt den Block **dauerhaft** an: eine Datei
`blocks/<name>.jjblk` im Datenordner. Nach dem nächsten Start steht er wieder im
Blockmanager, Untertitel **Benutzer**; Löschen im Blockmanager entfernt auch die
Datei. Das Verzeichnis ist die Wahrheit — es gibt keinen Index. Eine unlesbare
Datei wird gemeldet und übersprungen, sie hält den Start nicht auf.

## Nachweis für den Papier-Modus

`cargo run -p jjcad-core --example lw_test -- /tmp/lw.jjcad` erzeugt zwei parallele Linien
(0,5 mm Strich, 100 mm Abstand, Maßstab 1:100) — im Papier-Modus muss der weiße Streifen
zwischen ihnen genauso breit sein wie die Linien selbst.
