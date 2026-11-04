# JoJa-CAD v0.2 — Konventionen

Verbindliche Konventionen für Beiträge am Projekt. Abweichungen gelten als Fehler.

**Begriffe:** `docs/GLOSSAR.md` ist verbindlich. Ein Begriff, eine Bedeutung —
UI-Texte, Code-Kommentare und Dokumentation benutzen die dortige linke Spalte.
Abweichende Wörter gehören nicht in die Oberfläche.

## Worum es geht

2D-CAD für Architektur- und Bauantragszeichnungen, Rust-Workspace:

- `crates/jjcad-core` — Geometrie (`geom`), Modell (`model`), Befehle (`actions`),
  Displayliste (`render`), PDF/DXF-Schreiber, Datei-Ein-/Ausgabe (`persist`)
- `crates/jjcad-gui` — eframe/egui 0.29: Ribbon, Hotbar, Inventar, Panels, Canvas
- `crates/jjcad-mcp` — JSON-RPC-Server (MCP) auf derselben Befehlsschicht

Architekturregeln:

- **Eine Befehlsschicht**: jede Funktion ist eine Action (`draw.wall`, `modify.set`, …)
  in `actions.rs`. GUI, Tastatur, Hotbar und MCP rufen denselben Code auf — nie
  einen zweiten Weg an der Action-Schicht vorbei bauen.
- **Eine Displayliste** (`render::DisplayList`) ist die einzige Renderquelle für
  Bildschirm, PDF und PNG. Neue Linien/Füllungen dort erzeugen, nicht im Painter.
- Interne Einheit **Millimeter**, Winkel im Bogenmaß, Nullrichtung +X, mathematisch
  positiver Drehsinn. Ausgabe deutsch (Komma), `m²`.
- **Mehrere Dateien (T0073)**: Der Reiterstreifen hat **zwei getrennte Bereiche**: links die
  Dateien (`App::tabs`, aktiv über `App::active`) auf eigenem dunklem Band, volle
  Streifenhöhe, Dateisymbol, aktiver Reiter als Karte mit Akzentlinie; rechts in einer
  abgesetzten Fläche die Blattreiter der aktiven Datei als kleinere Chips mit 4 px Abstand
  (`doc_tabs`, `datei_reiter`, `blatt_chip`, `kreuz`, `BLATT_ABSTAND`). Keine Beschriftungen
  über den Zonen — sie waren überflüssig. Datei- und Blattreiter nie angleichen: die
  Unterscheidung ist der Sinn der zwei Zonen. „Neu“ hängt an (`App::new_document`), es ersetzt nichts; ein Wechsel läuft über
  `App::switch_doc`, das Werkzeug, Vorschau, Züge und laufende Texteingabe abschließt
  (`App::datei_nachziehen`, `textedit::beenden`) und Vorgaben/Raster/Raum aus der neuen
  Datei zieht. Eine Datei liegt nur einmal offen (`App::datei_reiter`, echter Pfad); ein
  frisches `Document` gilt **nicht** als geändert (sonst landet die geöffnete Datei in
  einem zweiten Reiter).
- **Oberfläche und Code-Kommentare auf Deutsch.**

## Bauen und testen

```bash
cargo build -p jjcad-gui            # Debug
cargo build --release -p jjcad-gui  # Release
cargo test                          # muss grün sein

./target/release/jjcad samples/Wohnhaus.jjcad
```

Das Binary trägt eine Build-Nummer in Fenstertitel und Statusleiste
(`crates/jjcad-gui/build.rs`); sie kommt aus der Git-Historie und ist ohne
Repository `0`/`nogit`.

Beispiele ohne GUI (schreiben ihre Bilder dorthin, wohin `JJCAD_OUT_DIR` zeigt,
sonst in das temporäre Verzeichnis):

```bash
cargo run --example zeichnung_png -- samples/Wohnhaus.jjcad /tmp/wohnhaus.png
cargo run --example volumen_erzeugen
cargo run --example wand_probe
```

## Regeln für Änderungen

1. **Jede Änderung bekommt einen Test.** Ohne Test kann ein Fehler unbemerkt
   zurückkommen; mit Test bricht der nächste Lauf sofort.
   Für das Eigenschaften-Panel heißt das: `props::tests` prüft je Objektart, dass
   Erzeugen und Bearbeiten **dieselben Zeilen in derselben Reihenfolge** zeigen
   (Protokoll der gezeichneten Zeilen). Eine neue Objektart ohne Panel-Eintrag
   lässt `jede_objektart_ist_eingetragen` fehlschlagen — bitte die Art dort
   eintragen **und** in `props.rs` Zeilen zeichnen, nicht nur den Test anpassen.
2. **Kleine, prüfbare Schritte**: bauen, testen, dann committen. Kein Commit
   mit Compile-Fehlern.
3. **Messbarer Nachweis statt „sieht gut aus“**: Test, geprüftes Feld in der
   gespeicherten `.jjcad`-Datei (z. B. `align: "left"`, Koordinaten) oder ein
   Pixelmaß.
4. **Debug-Dateien löschen** (`examples/dbg_*.rs`, temporäre Testdateien), bevor
   du abgibst.
5. **Nur die Dateien anfassen, die zur Aufgabe gehören.** Andere Dateien nur,
   wenn es zwingend nötig ist — und dann im Commit nennen.

## Verbindliche Designentscheidungen

Mit dem Nutzer abgestimmt — nicht „verbessern“:

- **Türen** sitzen immer auf der *vorderen Wandkante* (Blattseite), nicht in der
  Wandmitte. Kein Auswahlfeld für die Tür-Lage. Fenster dürfen ihre Lage in
  der Wanddicke wählen (Links/Mittig/Rechts).
- **Vorschau von Tür und Fenster** ist das fertige Objekt, kein Platzhalter:
  `render::preview_door`/`preview_window` liefern `OpeningPreview` (Symbol, Wand des
  Hosts, freigelegte Fläche) aus demselben Renderer wie der eingefügte Block — Tür auf
  der vorderen Wandkante (Blattseite folgt `swing_in`), Fenster in der Wanddicke; beide
  schneiden die Wand **gleich** aus. Das Loch wird in Papierfarbe freigelegt
  (`OpeningPreview::loch`), weil die Vorschau **über** der vorhandenen Wand liegt.
  Wandkanten der Vorschau müssen zeilengleich mit der Wand nach dem Einfügen sein.
- **Dropdowns**: immer `panels::pick_combo` bzw. `panels::dropdown`
  (150 px Wertespalte, gleicher Pfeil). **Kein `egui::ComboBox`** — schließt bei
  jedem Klick im Popup und sieht anders aus.
- **Linienenden sind Halbkreise** (Kapsel-Rendering in `canvas.rs`), nicht Rechtecke.
- **Linienstärken**: zwei Modi, `px` (bildschirmkonstant) und `DPI`
  (maßstabsgetreu wie im Druck). Strichmuster folgt demselben Schalter.
- **Wandgehrung** über die *Innenwinkel*-Halbierende (`away - outward`);
  Füllungen vor allen Linien zeichnen, damit keine Umrandung überdeckt wird.
- **Auswahl** färbt nur blau ein — kein zusätzlicher Auswahlrahmen. **Ausnahme Text**:
  der Text hat einen unsichtbaren Rahmen (derselbe Kasten wie in der Einfüge-Vorschau), der
  bei Auswahl gezeichnet wird und dessen Ecken/Kantenmitten fangen. Ob Text mit der
  gedrückten Maustaste wandert, entscheidet die Einstellung „Maus verschiebt“ (T0063);
  ausgeschaltet bleibt das Werkzeug „Verschieben“ der einzige Weg. Ein Doppelklick auf
  einen Text öffnet dessen Inhalt **immer**, auch mit eingeschaltetem Mausverschieben
  (`canvas::handle_input` verwirft dafür den eben gestarteten Zug, `App::verwerfe_move_drag`).
  Der Kasten ist
  die Zeilenbox (`render::text_frame`: Höhe × 1,4, Breite nach Zeichenbreiten plus Rand), und
  die Vorschau des Textwerkzeugs läuft über `canvas::paint_text` — dieselbe Platzierung wie
  der eingefügte Text, nicht mit der Grundlinie am Zeiger.
- **Textausrichtung** ist eine 3×3-Matrix im Eigenschaften-Panel: waagerecht
  Linksbündig/Zentriert/Rechtsbündig (`align`), senkrecht Oben/Mitte/Unten (`valign`), je Knopf
  ein Pfeil und ein Tooltip `<waagerecht> · <senkrecht>` (`widgets::align_matrix`, beim
  Platzieren **und** bei Auswahl). Der **Einfügepunkt** ist der Klickpunkt — er liegt in der
  Ecke/Kante/Mitte des Textkastens, die die Matrix zeigt; `render::text_corners` ist die eine
  Quelle für Anzeige, Rahmen, Fang und Vorschau. Beim Kopieren eines **einzelnen** Textfeldes
  ist dieser Einfügepunkt der Basispunkt: ein zentriert/mittig gesetzter Text landet beim
  Einfügen genau mittig auf dem Zielpunkt.
- **Schriftarten (T0059)**: Gebrauchsschriften kommen aus dem System (Serifenlos, Serifen, Nichtproportional, Schmal, Rund). Die technischen Schriften
  **DIN 1451** (Eng-, Mittel-, Breitschrift) und **EN ISO 3098** (Typ A, Typ B) liegen
  mit im Programm (`crates/jjcad-core/fonts/`, OFL bzw. LGPL mit Schriftausnahme,
  Herkunft in `fonts/LIESMICH.md`) — sie sind auf keinem Rechner installiert. Die
  **Breitschrift** entsteht beim Laden durch waagerechtes Strecken der Mittelschrift
  (`fonts::strecke`, × 1,25), weil DIN 1451 dieselben Formen in drei Breiten ist;
  gestreckt wird die **Schriftdatei**, nicht der Zeichner — so stimmen Zeichenfläche,
  Rasterausgabe, PDF und Zerlegen überein. „Schmal" ist Liberation Sans Narrow und
  **nicht** die DIN 1451. Die Auswahl `widgets::schrift_wahl` zeigt jede Zeile in ihrer
  eigenen Schrift und läuft über `panels::dropdown` (kein `egui::ComboBox`).
- **Raster-Chip** schaltet Anzeige *und* Rasterfang gemeinsam.
- **Ansichtsfenster** wie Rechtecke, beweglich nur mit `V`, exakte Eingabe X/Y/B/H.
- **Druckvorschau = Papierraum**: gleiche Werkzeuge wie im Modell. Der Bearbeitungsraum
  steckt in `Document::space` (`None` = Modell, `Some(Blatt)` = Papierraum); alle Actions
  laufen über `doc.ents()`/`doc.ents_mut()`. Rahmen und Schriftfeld sind normale Linien und
  Textfelder in `sheet.entities` (kein Renderer-Sonderrahmen mehr, `Sheet::frame`/
  `show_title_block` sind nur noch Altlasten für die Migration). Die echten Blattecken
  (0,0)…(B,H) fangen im Papierraum als „Blattecke“.
- **Mauszeiger** ist ein durchgehendes Fadenkreuz bis zum Bildschirmrand.
- **Einfügen** (Block/Zwischenablage): Vorschau oben rechts am Zeiger, `R` dreht,
  `S` spiegelt, Klick bestätigt. Beim Einfügen aus der Zwischenablage entscheidet
  Einstellungen → Bedienung → „Drehpunkt" (`prefs.paste_pivot`), welcher Punkt am
  Fadenkreuz steht: „Mittelpunkt" (Vorgabe seit T0087) hält die untere linke Ecke
  der gedrehten Auswahl am Fadenkreuz, sodass die Vorschau immer oben rechts davon
  bleibt; „Basispunkt" hält den mitkopierten Basispunkt dort — die Vorschau dreht um
  ihren Mittelpunkt und springt dabei um das Fadenkreuz (T0070). Ein Wechsel wirkt
  sofort, auch auf eine laufende Vorschau (`App::set_paste_pivot`). Die Zwischenablage trägt ihren **Basispunkt** mit:
  „Kopieren mit Basispunkt" nimmt den geklickten Punkt, „Kopieren" automatisch die
  untere linke Ecke der Auswahl — beim Einfügen steht dieser Punkt am Zeiger.
  **Kopieren setzt nie sofort eine Kopie ab** (auch nicht mit Basispunkt): es füllt
  nur die Zwischenablage, platziert wird über die Einfüge-Vorschau.
- **Tastenbelegung in drei Schichten (T0056, `docs/TASTEN.md` ist verbindlich)**:
  Ziffern `1`–`0` gehören der **Hotbar**, Buchstaben und Symbole der
  **Tastaturbelegung** (frei und dynamisch änderbar, je Taste genau eine
  Funktion, Umschalt-Ebene als `shift+<Zeichen>`), alle übrigen Tasten
  (`Esc`, `Strg`, `Enter`, `Entf`, `F1`–`F12` …) und Kombinationen (`Strg+C`,
  `Strg+S`) sind **hardcodiert**. Eine Funktion kommt wie bei der Hotbar ins
  Tastenbild (T0072): freie Taste anklicken öffnet das Inventar im Reiter
  „Funktionen" (`UiState::bind_key`) — es gibt **keine** eigene Funktionsliste
  im Fenster der Tastaturbelegung. Eine Taste darf nie in zwei Schichten liegen:
  der Ansichtsschwenk sind die Funktionen `view.pan_*` auf `W`/`A`/`S`/`D`
  (Icon im Tastenbild), Drehen und Spiegeln der Einfüge-Vorschau hängen an
  `modify.rotate`/`modify.mirror`. Kürzel nur über `app::EINZELTASTEN` und das
  Action-Register auswerten, nicht zusätzlich im Code abfragen.
- **Fenster „Tastaturbelegung" (T0076)**: eigene Kopfleiste statt Titelleiste —
  **i** linksbündig (Bedienung als Tooltip), Titel daneben, Schließen rechts;
  darunter das Tastenbild, darunter **nur** die Layoutzeile. Das Fenster ist
  genau so breit wie das Tastenbild (`tastatur_flaeche().x` = 716 px), keine
  Knopfzeile unter der Tastatur. Layouts liegen in `UserPrefs::keymap_presets`
  — die **ersten vier sind eingebaute Vorlagen** (T0079: „Vorlage JoJa-CAD",
  „Vorlage Allplan", „Vorlage ArchiCAD", „Vorlage AutoCAD"; nicht
  überschreibbar, nicht löschbar, eigene Layouts stehen dahinter), Wechsel
  über `keymap_anwenden`, Export/Import über `keymap_exportieren_aktuell` /
  `keymap_importieren`. Die Auswahl zeigt den passenden Namen oder „Eigene
  Belegung" (`keymap_passend`) — keinen gespeicherten Zustand raten.
- **Rechtsklickmenüs** bleiben offen, wenn der Zeiger in die Zeichenfläche kommt.
- **Eigenschaften-Panel**: gleiche Zeilen (96 px Label, 150 px Wert) beim Platzieren
  *und* bei Auswahl; bei Mehrfachauswahl nur gemeinsame Felder, abweichende Werte
  als `*variiert*`; nicht editierbare Werte (Länge, Fläche) anzeigen.
  Umgesetzt ist das in `crates/jjcad-gui/src/props.rs`: **eine** Funktion je
  Objektart zeichnet alle Zeilen, nur das Ziel der Änderung unterscheidet sich
  (`props::Ziel::Neu` schreibt in `ToolOptions` — „gilt für das nächste Element“;
  `props::Ziel::Auswahl(id)` schickt `modify.set` an das Objekt). Nie wieder eine
  zweite Zeichnung derselben Eigenschaften in `panels::entity_props` oder
  `panels::pending_props` anlegen. Zeilen, die ohne Objekt sinnlos sind
  (Reihenfolge, Textinhalt, Kennzahlen), werden **deaktiviert gezeigt, nicht
  weggelassen**. Ein **Erstellwerkzeug** (`draw.*`, `dim.*`) verdrängt die
  Auswahl aus dem Panel: es zeigt nur das neue Element, die Auswahl bleibt in der
  Zeichnung markiert. Und es gibt **kein eigenes Panel während der Texteingabe** —
  auch dort zeichnet `props::text`; `props::eingabe_spiegeln` reicht die
  Panelwerte an den Editor in der Zeichenfläche weiter.
  **Alle** Erzeuger und alle Objektarten laufen über `props`; `panels` zeichnet
  kein eigenes Eigenschaften-Panel mehr. Eine neue Objektart wird nach der
  Anleitung im Kopf von `props.rs` eingetragen (Zeichnen, Werte holen, Anwenden,
  Zeilen anmelden, Routing, Test) — `props::tests::jede_objektart_hat_ein_panel`
  und `…::alle_erzeuger_zeigen_dieselben_zeilen_wie_das_objekt` erzwingen das.
  Die **Mehrfachauswahl** (`props::mehrfach`) zeichnet **je Objektart** denselben
  Abschnitt wie ein einzelnes Objekt und schickt jede Änderung über `props::setzen`
  an **alle** Objekte der Gruppe (`App::mehrfach_ziel`). Damit gilt jede Eigenschaft
  eines Objekts automatisch auch für die Auswahl — wer eine Zeile ergänzt, hat sie
  in der Mehrfachauswahl schon. Abweichende Werte meldet die Kopfzeile als
  `*variiert*`; ein zweiter Maler existiert nicht mehr.
- **Markenzeichen (T0083)**: Das Icon aus `branding/` ist **eine** Bilddatei
  (`crates/jjcad-gui/assets/jjc-icon-512.png`, per `include_bytes!` im Programm wie die
  technischen Schriften). Drei Stellen zeigen es über `icon::textur`/`icon::textur_klein`/
  `icon::fenster_icon`: Fenstertitel (128 px, größer geht unter X11 nicht — siehe unten),
  Kopfzeile (30 px, dafür die 32-px-Rasterung; bei 24 px verwaschen die weißen Linien) und
  der Über-Dialog (64 px). Kein zweites gezeichnetes Ersatzlogo daneben.
  **`_NET_WM_ICON` ist höchstens 262 140 Byte lang** (65 535 Wörter einer X-Anfrage):
  256×256×4 = 262 144 Byte passt schon nicht mehr, winit verwirft den `BadLength` still und
  das Fenster bleibt ohne Icon. Größe mit `xprop -id <fenster> _NET_WM_ICON` prüfen. Unter **Wayland** überträgt winit kein Fenstericon: Taskleiste und
  Startmenü holen es aus `~/.local/share/applications/jjcad.desktop` (Name = App-ID `jjcad`),
  angelegt mit `scripts/install-desktop.sh`.
- **Zeichenrang (T0048)**: Neue Elemente ordnen sich beim Einfügen selbst in die
  Zeichenreihenfolge ein — Fläche (0) < Volumen (1) < Bauteil (2) < Linie (3) <
  Beschriftung (4), Konstanten `model::RANG_*`, Prüfung in
  `EntityKind::zeichenrang`. Eingefügt wird **nur** über
  `Document::insert_ranked` (bzw. `add_entity`, das dorthin delegiert); bestehende
  Objekte werden nie umsortiert, sie rücken nur. Wer eine Objektart ergänzt, ordnet
  sie dort einer Klasse zu. Details: `docs/ARCHITEKTUR.md` → „Zeichenrang“.
- **Flächen schneiden und vereinigen (T0090)**: `modify.area.split` teilt eine
  Füllfläche entlang einer Punktfolge in zwei Teilflächen, `modify.area.union`
  verschmilzt zwei oder mehr **angrenzende** Füllflächen zu einer. Die Geometrie
  steht in `crates/jjcad-core/src/flaeche.rs` (ebenes Netz: Kanten an Kreuzungen
  und Stößen teilen, gemeinsame und innenliegende Kanten fallen weg, Ringe neu
  zusammensetzen) — nicht in der Oberfläche nachbauen. Beide Aktionen ersetzen die
  Ausgangsflächen und übernehmen Muster, Randlinie, Stil und Ebene von der ersten.
  Getrennte Flächen, mehrfacher Randkontakt und eine gekreuzte Insel sind Fehler
  (kein stiller Erfolg). Ribbon: Zeichnen ▸ Füllen ▸ „Vereinigen" / „Schneiden".
- **Bemaßung beim Ändern**: Die Maßlinie behält ihre **Lage**. Ein waagerechtes
  Maß liegt auf *einer* Höhe (`line_pos.y` für alle Basispunkte), ein senkrechtes
  auf einer Breite — `render::dim_projiziert` **setzt** die Koordinate, es addiert
  keinen Versatz je Punkt. Ein Versatz je Basispunkt kippte die Maßlinie, sobald
  ein Punkt seitlich wanderte (T0055). Griffe: Quadrat je Basispunkt, Raute in der
  Mitte der Maßlinie (verschiebt sie senkrecht zur Messrichtung, `render::dim_line_move`);
  im Eigenschaften-Panel steht derselbe Wert als „Abstand“ (`render::dim_offset`).
- **Polarfang** 45° (Toleranz 6°); Fangmodi END, SCHN, MIT, ZEN, QUA, LOT, NÄC, RAS, POL.
- **Treppen** sind von der Kontur her ein **Rechteck**: erster und zweiter Klick
  liegen auf der **Außenkante** (Länge), der dritte setzt die Breite dorthin, wo
  der Zeiger steht. Gespeichert ist die Lauflinie (`a`→`b`, Mitte) plus Breite —
  die Anzeige rechnet daraus das Rechteck. Die vier Kantenmitten sind Griffe
  (Raute quer zur Kante): Ziehen verschiebt genau diese Kante **parallel**
  (`handles::apply_grip_tol`), die gegenüberliegende bleibt stehen;
  Länge/Breite/Höhe stehen im Eigenschaften-Panel. Beim Ziehen an einer Kante
  läuft die Maßangabe „Länge × Breite · n Stg“ am Zeiger mit
  (`canvas::draw_grip_mass`, derselbe Text wie in der Einfüge-Vorschau).
  Ein Klick in die Fläche verschiebt die Treppe nur, wenn die Einstellung
  „Maus verschiebt“ an ist (`App::maus_verschiebt`, T0063) — sonst bewegen sie
  nur die Kantengriffe und das Werkzeug „Verschieben“. Eine
  Laufrichtung (auf/ab) gibt es nicht: der Pfeil zeigt immer von `a` nach `b`. Die Höhe bestimmt über die Schrittmaßregel
  (2 S + A = 63 cm) die Stufenzahl: `n = (2 H + L) / 630`, gerundet; das Feld
  „Stufen“ überschreibt sie, das Häkchen „Schrittmaß“ schaltet die Regel zurück.
  Der **Treppenschnitt** (zwei Diagonalen in der Laufmitte, die Stufenlinien
  werden dort unterbrochen) ist abschaltbar — er symbolisiert **zwei Läufe
  übereinander**. Mit Schnitt haben unterer und oberer Teil **eigene Linien**
  (Farbe/Stärke/Typ; `unten`/`oben` im Modell, `None` = Objektstil); die Grenze
  dazwischen ist die **Parallele zwischen den beiden Diagonalen**, sie läuft
  also schräg über die Treppe (`render::schnitt_geometrie`). Der Knopf
  „Beschriftung erzeugen“ legt `16 Stg / 17,2 / 29` als Textfeld links neben den
  Lauf (`draw.stair.label`).
- **Erweiterter Punktfang**: bleibt der Zeiger 400 ms auf einem diskreten Fangpunkt stehen
  (ohne Klick), wird dieser *gehalten* und bekommt zwei Hilfslinien — bei einem Punkt auf
  einer Linie Verlängerung und Normale, sonst Waagerechte und Senkrechte. Auf ihnen fängt
  der nächste Punkt (`SnapKind::Tracking`, Statuszeile „Hilfslinie"), ebenso der
  **Schnittpunkt einer Hilfslinie mit einer bestehenden Linie** (Statuszeile „Schnittpunkt");
  höchstens zwei Haltepunkte, ein Klick löst sie. Diskrete Objektfänge (END, MIT, …) schlagen die
  Hilfslinie, die Hilfslinie schlägt Nächster Punkt/Lot und das Raster. **Ortho und Polar sind
  keine Alternative zur Hilfslinie, sondern eine Bedingung**: liegt eine Hilfslinie im Fangradius
  und greift zugleich Ortho bzw. Polar, wird der **Schnittpunkt beider** gefangen
  (`snap::zwangsachse`, `hilfslinien_snap_mit_zwang`) — die Achse bleibt, die Hilfslinie wählt nur
  die Lage auf ihr. Ohne das kippte die neue Linie je nach Abstand zur Hilfslinie aus der
  Zwangsrichtung (T0042).
- **Texteingabe** läuft direkt in der Zeichnung (kein Dialog, keine Eingabefläche): ein
  Klick mit dem Textwerkzeug setzt den Cursor an die Klickstelle, der Text erscheint sofort
  in der Zeichnung — gleiche Schrift, gleiche Lage, gleiche Farbe wie danach. Ein Doppelklick
  auf einen vorhandenen Text öffnet dessen Inhalt. **Esc übernimmt**, ein Klick daneben
  übernimmt ebenfalls, **Enter ist der Zeilenumbruch**, Rechtsklick bricht ab.
- **Chat**: Meldungen stehen unten links in der Zeichenfläche und verblassen
  nach einigen Sekunden; `^` (die Taste unter ESC) öffnet das Protokoll im
  selben Bild — Meldungen laufen nach oben, unten die Eingabezeile mit Fokus,
  Mausrad blättert, die Zeichnung wird leicht blass. `^`/`ESC` schließen es,
  Enter schickt die Eingabe ab und schließt ebenfalls (sie steht dann als Zeile
  `› …` im Chat). Eingaben des Nutzers werden nur protokolliert. Es gibt keinen eigenen Toast
  mehr; das Text-Werkzeug hat kein Kürzel. Bei offenem Protokoll bleibt der
  Mauszeiger sichtbar (in der Eingabezeile als Textcursor).
- **Chat-Schema** (eine Form für alle Funktionen): `<Werkzeug> · <Schritt>` als
  Aufforderung (`Wand · Ende klicken`), `<Objekt> · <Zustand>` als Ergebnis
  (`Wand · gesetzt`, `Auswahl · kopiert`), `<Werkzeug> · <Grund>` als Abbruch
  (`Tür · nur auf einer Wand`). Kein Satz, keine Erklärung, höchstens 48 Zeichen
  — `app::tests_chat_texte` prüft Aufforderung und Ergebnis für jedes Werkzeug.
- Wände: Farbe (Linie) und Füllung (Fläche) getrennt; Schraffuren inklusive
  **Dämmung** (Mäander); Ausrichtung Links/Mittig/Rechts.
- **Helles und dunkles Design (T0102)**: Die Farben stehen in **zwei** Tabellen in
  `theme.rs` (`DUNKEL`, `HELL`); `Palette::bg()` & Co. liefern die Werte des eingestellten
  Designs — im Zeichenweg wird **keine** Farbe festgehalten, sonst bleibt beim Umschalten ein
  Teil der Oberfläche im alten Design. Einstellungen ▸ Darstellung ▸ „Design“ bietet
  **System** (Programmstandard), **Hell**, **Dunkel**; gespeichert als `UserPrefs.design`
  (`system`/`hell`/`dunkel`), der Wechsel wirkt sofort (`App::set_design` →
  `theme::set_design`, Stil neu, Anzeigelisten verworfen). „System“ fragt zuerst
  `ctx.system_theme()` (Windows/macOS); unter Linux meldet winit nichts, dort entscheiden
  `kdeglobals` (`[Colors:Window] BackgroundNormal`, sonst `ColorScheme`), dann GTK
  `settings.ini`, dann `GTK_THEME`/`COLORFGBG`; ohne Auskunft bleibt es dunkel. Ein Wechsel
  am Betriebssystem wird höchstens alle 2 s nachgezogen (`theme::system_nachziehen`).
  **Papier und Raster bleiben in beiden Designs gleich** — die Zeichnung sieht aus wie im
  Druck, nur die Bedienoberfläche wechselt (auch die Beschriftungsbox am Zeiger,
  `Palette::hinweis_flaeche`).

- **Schraffuren (T0095)**: Die Muster sind die Kennzeichnungen der Schnittflächen nach
  DIN 1356-1:2026-09, Tabelle 5 (18 Zeilen, `style::FillPattern`, `ALL` in der
  Reihenfolge der Norm); die früheren Muster heißen nach ihrem Baustoff (Mauerwerk,
  Kunststoffe, Sand, Dämmstoffe). Gezeichnet werden **alle** Muster an einer Stelle
  (`render::push_muster`) — Füllfläche, Volumen und Wandschraffur teilen sie, keine
  zweite Musterzeichnung. Schlüssel über `FillPattern::key`/`from_key`, die alten
  Schlüssel `lines`/`cross`/`dots`/`insulation` bleiben gültig (alte Dateien).
  Ein neues Muster: Variante und `ALL` in `style.rs`, Zweig in `push_muster`,
  Bezeichnung in `TABELLE_5` des Tests `tests/schraffuren_din.rs` — der Test bricht,
  wenn ein Muster fehlt, nichts zeichnet oder wie ein anderes aussieht.
- **Auswahl über Rechteck** prüft echte Geometrie (Segmente, Wandkörper),
  nicht die achsenparallele Bounding-Box.
- **Griffe** haben 8–12 px Trefferradius, geprüft gegen die *rohe* Zeigerposition
  (nicht den Fangpunkt).
- **Zieh-Modi**: Einstellungen → Bedienung → „Ziehen“ (Gedrückthalten / Zwei
  Klicks), gespeichert in `prefs.json`, gilt für Auswahl *und* Griffe.
- **Maus verschiebt (T0063)**: Einstellungen → Bedienung → Häkchen „Maus verschiebt“
  (`prefs.mouse_move`, Vorgabe an). Es gilt für **alle** Objektarten gleich — Text und
  Treppe haben keine Sonderregel mehr. Ausgeschaltet wandert ausschließlich das Werkzeug
  „Verschieben“ (exakter Weg für professionelles Arbeiten). Die eine Auswertung ist
  `App::maus_verschiebt`; wer eine Objektart besonders behandeln will, tut das dort —
  nicht in einem zweiten `match` an der Zeichenfläche.
- **Einstellungsfenster (T0061)**: links die Kategorienliste
  (`dialogs::EINSTELLUNGSKATEGORIEN`, Icon + Name, Auswahl blau), darüber das Suchfeld,
  rechts die Einstellungen der gewählten Kategorie. Beide Spalten scrollen getrennt.
  Eine neue Einstellung wird in ihrer Kategorie über `dialogs::zeile`/`zeile_mit`
  gezeichnet **und** ihr Name in den `stichworte`n der Kategorie eingetragen — sonst
  findet die Suche sie nicht. Zeilennamen bleiben kurz (Labelspalte 96 px), lange
  Begriffe kommen als Alias in `zeile_mit`; Häkchen ohne Wert stehen mit vollem Text.
- **Schwenkgeschwindigkeit (T0067)**: Einstellungen → Bedienung → „Schwenk" ist ein
  Faktor (`prefs.pan_speed`, 0,1…5, Vorgabe 1) auf den Schritt der
  Schwenk-Funktionen (`app::SCHWENK_SCHRITT_PX` = 60 px). **Ein** Weg: die Funktion
  `view.pan_*` aus Taste, Hotbar oder Ribbon ruft `App::schwenk_schritt` — ein Druck
  ist ein Schritt (T0089). Keinen zweiten Dauerlauf auf `key_down` daneben bauen;
  der bewegte die Ansicht auch bei Strg+S und unabhängig von der Hotbar.

- **Höhenkoten (T0096)**: Zwei getrennte Objektarten ersetzen die frühere eine
  Höhenkote. Die **Grundriss-Kote** (`EntityKind::ElevationPlan`) ist ein
  Kreis-Symbol mit Fadenkreuz und zwei gefüllten Vierteln; der Höhenwert steht
  rechts daneben und gehört zum Symbol, die Höhe gibt der Nutzer im
  Eigenschaften-Panel an (`z` in mm, `elevation_text` mit zwei Stellen in
  Metern). Die **Schnitt-Kote** (`EntityKind::ElevationSection`) ist eine
  Punktfolge aus Dreieck und Höhenangabe je Punkt: der **erste** Punkt ist
  ±0,00 (offenes Dreieck, ±0,00 als Text), alle weiteren Höhen rechnet der
  Renderer aus der Y-Differenz zum ersten Punkt und schreibt sie dreistellig
  (`elevation_section_text`, z. B. `+1,200`). Werkzeuge:
  `dim.elevation.plan` (ein Objekt je Klick) und `dim.elevation.section`
  (Punkte klicken, Enter beendet). Trefferflächen beider Koten kommen aus
  `render::trefferbild` — über die Bounding-Box wäre die ganze Fläche zwischen
  den Punkten anklickbar. Alte Zeichnungen mit `elevation` werden über einen
  Serde-Alias als Grundriss-Kote gelesen; der frühere Zusatztext entfällt.

## Zahlenfelder: Grenzen und Zuggeschwindigkeit (verbindlich)

Jedes Zahlenfeld — Eigenschaften-Panel, Einstellungen, Statusleiste — ist ein
`egui::DragValue`: Klick und Ziehen ändert den Wert, ein weiterer Klick tippt
ihn. egui rechnet beim Ziehen `Wert += Punkte × speed` **in der Anzeigeeinheit**
und klemmt auf `range`. Zwei Fehler sind damit möglich, beide sind passiert
(T0029): `speed` aus der Einheit abgeleitet (Wandbreite in Metern sprang um
500 m je Pixel) und `range` vergessen (negative Wandbreiten, Werte bis in
Milliarden).

Deshalb gibt es **genau zwei Bausteine** in `crates/jjcad-gui/src/widgets.rs`:

| Baustein | Wofür | Grenzen |
|---|---|---|
| `widgets::length_field(ui, &mut mm, einheit, breite, min_mm, max_mm)` | jede Länge in mm | Pflichtargumente |
| `widgets::zahl(&mut wert, speed, range)` | Winkel, Stückzahlen, Faktoren, Papier-mm | Pflichtargumente |

Benannte Vorgaben (dieselbe Datei) statt Zahlen im Aufruf:

- `MASS_MIN_MM` = 1 mm, `MASS_MAX_MM` = 100 m — Abmessungen (Wanddicke, Breite,
  Höhe, Textgröße, Radius).
- `ABSTAND_MIN_MM` = 0,1 mm, `ABSTAND_MAX_MM` = 10 m — Abstände, Muster,
  Schraffurweiten.
- `KOORD_MIN_MM` / `KOORD_MAX_MM` = ±1 km — Lagewerte (X/Y, Ansichtsfenster,
  Höhenkoten); diese **dürfen negativ sein**.
- `WINKEL_MIN` / `WINKEL_MAX` = ±360° — Drehungen, Füll- und Bogenwinkel.
- Zuggeschwindigkeit von Längen: `ZUG_MM_JE_PUNKT` = 10 mm je Bildpunkt (1 cm;
  Umschalt gedrückt = 1 mm) — **in mm, cm und m gleich**. Die Einheit ändert nur
  die Anzeige, nicht das Ziehen; deshalb ist `speed` nie aus `unit.mm()` 
  abzuleiten.

Regeln:

1. Neue Zahlenfelder nur über diese zwei Bausteine. `egui::DragValue::new`
   außerhalb von `widgets.rs` ist ein Fehler;
   `widgets::zahltests::zahlenfelder_nur_ueber_widgets` bricht den Test, sobald
   ein Panel wieder selbst ein Feld baut.
2. Grenzen gehören zum Feld, nicht zur Aufrufstelle: `.speed(...)` und
   `.range(...)` werden außerhalb von `widgets.rs` nicht aufgerufen.
3. Untergrenze nur dann 0 oder kleiner, wenn der Wert fachlich negativ sein darf
   (Lage, Kote, Drehung). Breiten, Dicken und Höhen beginnen bei `MASS_MIN_MM`.
4. Ganzzahlige Felder (Stufenzahl, Nachkommastellen, Reihenfolge) mit
   ganzzahligen Grenzen; `zahl` setzt `.max_decimals(0)` selbst.
5. Eigene Grenzen nur, wenn der Bereich fachlich anders ist, mit Begründung im
   Kommentar daneben (Beispiel: Rasterweite `GRID_STEP_MIN_MM`…`MAX_MM`,
   Polarfenster 0…45°).

## Entwicklungsumgebung

Dieses Repository wird lokal mit einem Ticketsystem und einem eigenen Testkanal
bearbeitet; beides ist bewusst **nicht** Teil dieses Repos. Veröffentlicht wird
ausschließlich `master`. Arbeits- und Testkanal-Skripte wie `scripts/ship-card.sh`,
`scripts/build-live.sh` und `scripts/start-live.sh` werden dort gebraucht und
bleiben im Repo, setzen aber einen solchen Kanal voraus — für Beiträge von außen
sind `cargo build`, `cargo test` und `scripts/qa-shot.py` ausreichend.
