# JoJa-CAD v0.2 — Konventionen

Verbindliche Konventionen für Beiträge am Projekt. Abweichungen gelten als Fehler.

**Begriffe:** `docs/benutzer/GLOSSAR.md` ist verbindlich. Ein Begriff, eine Bedeutung —
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

- **Fensterschatten ohne Titelleiste (T0147)**: Für ein Fenster ohne Dekoration
  zeichnet der Fenstermanager unter Wayland keinen Schatten — JoJa-CAD malt ihn
  selbst. Ein Streifen von `app::SCHATTEN_RAND` (16 px) am Fensterrand bleibt
  durchsichtig (Fenster mit `with_transparent`, `clear_color` mit Alpha 0), die
  Oberfläche liegt darin; den kleineren Bildschirmausschnitt setzt
  `App::raw_input_hook` (vor dem Bild, sonst bleiben die Panels auf der ganzen
  Fläche). Unter X11 bleibt es beim Schatten von KWin (gemessen: derselbe
  Schatten mit und ohne Titelleiste) — dort wäre der durchsichtige Rand ohne
  Compositor ein dunkler Rahmen. Im Vollbild und maximiert entfällt der Rand.
  **Andocken (T0177)**: Der Streifen gehört zum Fenster, das Fenstermanagement
  rechnet ihn mit — beim Andocken blieb ein Abstand in Schattenbreite und die
  Oberfläche war um ihn kleiner. Deshalb entfällt der Rand auch, wenn das
  Fenster am Bildschirmrand liegt: `app::fenster_angedockt` erkennt das an der
  **Größe** (halbe Bildschirmbreite bzw. -höhe, beides = Viertel; gemessen unter
  KWin 6.3: andocken links → 799 × 972 auf 1600 × 1000), weil winit unter
  Wayland weder „angedockt“ noch eine Fensterlage meldet. Abschaltbar in
  Einstellungen ▸ Darstellung ▸ „Schatten“ (`prefs.fensterschatten`, Vorgabe an);
  ohne ihn fordert `main.rs` den Streifen gar nicht erst an. `clear_color` ist
  nur dann durchsichtig, wenn `ui.fenster_aussen` in **diesem** Bild gesetzt ist.
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
- **Mittelpunkte (T0117)**: Das Kreuz am Mittelpunkt von Kreis, Bogen und Ellipse
  zeichnet `canvas::paint_prim` nur bei **Auswahl** — im Plan ist der Mittelpunkt
  kein Zeichnungselement. Fangen bleibt er trotzdem; wird er gefangen, zeigt
  `canvas::draw_cursor` das Kreuz in Akzentfarbe (Halbbreite 7 px) an der
  Fangstelle.
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
- **Linienstärke-Voreinstellungen (T0125)**: `LAYER_DEFAULTS` (`style.rs`) führt
  **keine** Stärke mehr — nur Name, Farbe, Linientyp. Die Breite je Objektart steht
  in `prefs.json` (`UserPrefs::layer_weights`, Vorgabe
  `style::LINIENSTAERKE_VORGABE` = 0,18 mm für alle) und wird über
  `Document::setze_standard_breiten` (frische Zeichnung) bzw.
  `Document::merke_standard_breiten` (geöffnete Zeichnung: nur für **neue**
  Standardebenen) übernommen. Bestehende Ebenen werden nie umgeschrieben. Das
  Eigenschaften-Panel zeigt als „beim Erzeugen die Vorgabe“ dieselbe Zahl
  (`Document::standard_breite`), nicht mehr 0,35.
- **Raster-Chip** schaltet Anzeige *und* Rasterfang gemeinsam.
- **Raster (T0144)**: Die Kategorie „Raster“ im Einstellungsfenster führt **alle**
  Rastereinstellungen (Weite, Anzeigen, Fang, Art, Modus, Erzwingen). Art und Weite
  stehen in `gitter::Raster` — **eine** Rechnung für Anzeige (`canvas::draw_grid`) und
  Fang (`snap`), kein zweites Rastermodell daneben. Arten: rechtwinklig (0°/90°),
  isometrisch (30°/150°/90°, dritte Familie entlang `u+v`), Dimetrie (7°/138°, zweite
  Achse halb verkürzt), Trimetrie (15°/140°, 0,6). Modus „Skaliert“ dünnt die
  gezeichneten Linien bei kleinem Zoom aus (bisheriges Verhalten), „Fest“ zeichnet
  immer die eingestellte Weite; gefangen wird in beiden Modi auf die eingestellte
  Weite. „Rasterfang erzwingen“ (`grid_force`) legt jede Eingabe **ohne** Fangradius
  ins Raster; Objektpunkte (Endpunkt, Schnitt, Mitte …) bleiben exakt, sonst wäre der
  Objektfang mit dieser Option unbrauchbar.
  **Die gezeichneten Linien liegen um die Mitte des sichtbaren Ausschnitts**
  (`canvas::raster_strecken`), nicht um den Weltnullpunkt: eine Linie durch den Ursprung
  deckte bei hohem Zoom nur die Umgebung von (0,0) ab, und das Raster verschwand aus dem
  Bild, sobald die Zeichnung weit davon entfernt war. Prüfweg:
  `cargo test -p jjcad-gui tests_raster` (Gegenprobe ist `basis = normale · (k · abstand)`).
- **Ansichtsfenster (T0109)** sind **normale Blattobjekte** (`EntityKind::Viewport` in
  `sheet.entities`, nicht mehr `Sheet::viewports`): vier Eckgriffe wie ein Rechteck, Auswahl per
  Klick und Auswahlfenster, Verschieben per Maus („Maus verschiebt") oder Werkzeug „Verschieben",
  Löschen mit `Entf`, Eigenschaften im Panel (`props::viewport`). Die alte Liste
  `Sheet::viewports_alt` wird nur noch beim Laden gelesen (`persist::normalize_sheet_viewports`).
  Den Inhalt (die Zeichnung im Maßstab) rendert weiterhin `render::render_viewport`; der Rahmen
  kommt aus `render_sheet`.
  **Ansichtsbereich anpassen (T0158)** ist ein Schalter im Eigenschaften-Panel
  (`UiState::vp_edit`): solange er an ist, gehören die Bewegungsfunktionen dem Fenster statt dem
  Blatt — Mitteltaste/Leertaste+Ziehen schieben den Ausschnitt (`canvas::viewport_ziehen`),
  Mausrad ändert den Maßstab und hält den Punkt unter dem Zeiger (`canvas::viewport_zoom`),
  Tastatur-Schwenk (`App::schwenk_schritt`) und „alles zeigen" (`App::fit_view` →
  `sheet.fit_viewport`) ebenso. Geschrieben wird immer über `App::vp_setzen` (`modify.set`), also
  derselbe Weg wie das Panel. Der Schalter hängt am ausgewählten Fenster und endet mit der
  Auswahl (`App::sync_space`); `App::vp_edit_fenster` ist die eine Abfrage. Ein **Doppelklick in
  die Ansicht** schaltet ihn ebenfalls (an/aus): `canvas::viewport_bei` trifft die ganze Fläche,
  nicht nur den Rahmen, und der Doppelklick-Zweig steht **vor** `laufender_drag`, weil der zweite
  Druck über dem Fenster einen Mauszug startet („Maus verschiebt“) — der wird wie beim
  Text-Doppelklick mit `App::verwerfe_move_drag` still verworfen.
- **Druckvorschau = Papierraum**: gleiche Werkzeuge wie im Modell. Der Bearbeitungsraum
  steckt in `Document::space` (`None` = Modell, `Some(Blatt)` = Papierraum); alle Actions
  laufen über `doc.ents()`/`doc.ents_mut()`. Rahmen und Schriftfeld sind normale Linien und
  Textfelder in `sheet.entities` (kein Renderer-Sonderrahmen mehr, `Sheet::frame`/
  `show_title_block` sind nur noch Altlasten für die Migration). Die echten Blattecken
  (0,0)…(B,H) fangen im Papierraum als „Blattecke“.
- **Blockeditor (T0137)**: Eigene Blöcke (Projekt/Benutzer — **nicht** System) werden
  über das Rechtsklickmenü der Blockbibliothek geöffnet. `Document::block_space` ist
  der **dritte Bearbeitungsraum** neben Modell und Papierraum: `ents()`/`ents_mut()`
  liefern die Blockdefinition, damit gelten alle Actions, Fang, Auswahl und Griffe
  unverändert. Der Reiter steht im Streifen — Blöcke **der Datei** in der Blattzone,
  **Benutzerblöcke** in einer eigenen Gruppe dahinter. Die Fläche hat einen eigenen
  Ton (`Palette::block_paper`), zeigt den Ursprung als Basispunkt (Kreuz + „Basispunkt“)
  und oben den Hinweis `Blockeditor · <Name>`. Ohne Auswahl zeigt das Panel Name,
  Gruppe, Beschreibung, Herkunft und „Speichern“ (schreibt bei Benutzerblöcken die
  `.jjblk` und schließt den Reiter); das Reiterkreuz verwirft den Stand vom Öffnen.
  `render::visible_bbox` liest `doc.ents()` — sonst passt die Ansicht im Blockeditor
  die Zeichnung statt den Block ein.
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
  **Miniatur am Zeiger (T0156)**: Die Vorschau wird nur **über der Zeichenfläche**
  gezeichnet (`canvas::show` prüft dafür die Zeigerlage) — verlässt der Zeiger
  die Fläche, gibt es dort keinen Punkt zum Setzen, und das eingefrorene Abbild am
  letzten Punkt sah aus, als hinge es in der Luft. Am Zeiger steht stattdessen die
  Miniatur des laufenden Inhalts (`canvas::draw_ghost_zeiger`, `GHOST_ZEIGER` = 30 px
  gegen 50 px Slot) aus **derselben** Anzeigeliste wie Blockkachel und Hotbar-Slot
  (`inventory::draw_list_preview` über `ghost::geometry`, also gedreht/gespiegelt wie
  die Vorschau) — damit ist sichtbar, was ein Klick auf einen Slot oder eine Taste
  ablegt. Prüfzustand `--demo blockvorschau`; Tests
  `cargo test -p jjcad-gui tests_ghost_zeiger`.
- **Tastenbelegung in drei Schichten (T0056, `docs/benutzer/TASTEN.md` ist verbindlich)**:
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
  Der Objekteintrag **„Eigenschaften im Panel“** öffnet das Eigenschaften-Panel
  (ausgeschaltet oder auf die Leiste eingeklappt) und wählt das Objekt aus; steht
  das Panel offen, wird der Eintrag gar nicht gezeichnet — die eine Abfrage ist
  `App::eigenschaften_panel_offen` (T0179).
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
  sie dort einer Klasse zu. Details: `docs/technik/ARCHITEKTUR.md` → „Zeichenrang“.
- **Flächen schneiden und vereinigen (T0090)**: `modify.area.split` teilt eine
  Füllfläche entlang einer Punktfolge in zwei Teilflächen, `modify.area.union`
  verschmilzt zwei oder mehr **angrenzende** Füllflächen zu einer. Die Geometrie
  steht in `crates/jjcad-core/src/flaeche.rs` (ebenes Netz: Kanten an Kreuzungen
  und Stößen teilen, gemeinsame und innenliegende Kanten fallen weg, Ringe neu
  zusammensetzen) — nicht in der Oberfläche nachbauen. Beide Aktionen ersetzen die
  Ausgangsflächen und übernehmen Muster, Randlinie, Stil und Ebene von der ersten.
  Getrennte Flächen, mehrfacher Randkontakt und eine gekreuzte Insel sind Fehler
  (kein stiller Erfolg). Ribbon: Zeichnen ▸ Füllen ▸ „Vereinigen" / „Schneiden".
- **Maßkette und Einzelmaß sind ein Element (T0154)**: `EntityKind::Dim` trägt
  `pts` (mindestens zwei **Messpunkte**), `line_pos`, `angle`, `override_text`;
  das frühere Einzelmaß (`a`, `b`) und `DimChain` gibt es nicht mehr. Alte
  Dateien und Blöcke schreibt `persist::migrate_legacy_dims` beim Laden um —
  **ein** Renderer (`render_dim_chain`) zeichnet jede Bemaßung, `dim.linear`
  bleibt nur als Befehl für MCP/ältere Skripte. Das Werkzeug `dim.chain` hat
  drei Eingabemodi (`app::DimModus`): **Waagerecht** und **Senkrecht** setzen
  zuerst die Lage der Maßlinie (erster Klick im Werkzeugpuffer, Vorschau =
  exemplarische Kette), danach Messpunkte; **Direkt** nimmt zwei Messpunkte,
  der **dritte** Klick legt die Maßlinie parallel fest, weitere Klicks hängen
  Messpunkte an. `canvas::dim_legt_masslinie`/`dim_messpunkte`/`dim_masslinie`
  sind die eine Auswertung für Klick, Vorschau, Fang und Chat-Text — nicht
  daneben nachbauen. `dim.point.add`/`dim.point.remove` ändern eine bestehende
  Kette (Einfügen hält die Reihenfolge entlang der Messrichtung, unter zwei
  Punkten ist Entfernen ein Fehler). Beim Positionieren fängt
  `snap::dim_parallel` die Maßlinie parallel zu einer vorhandenen im Abstand
  `DIM_PARALLEL_ABSTAND_MM` (0,75 m). **Je Modus eine eigene Schaltfläche**
  (`dim.chain.h`/`.v`/`.d`, eigenes Icon, kein Dropdown) — der Modus steckt im
  Werkzeug (`DimModus::werkzeug`/`aus_werkzeug`), nicht in einer Einstellung
  daneben; `dim.chain` bleibt der Befehl. **Messpunkte werden sortiert**:
  `render::dim_sortieren` ordnet sie beim Anlegen entlang der Messrichtung
  (beliebige Klickreihenfolge, kein Zickzack), `dim.point.add` fügt an der
  geordneten Stelle ein. Punkt hinzufügen/entfernen geht in **zwei Schritten**
  (`Tool::obj` hält die gewählte Maßkette): erst die Kette anklicken, dann den
  Messpunkt. **`Esc` bestätigt** die Eingabe (wie beim Text), statt sie zu
  verwerfen: `App::escape_pressed` schickt die Maßketten-Werkzeuge in
  `finish_tool` (ohne einen Klick bricht Esc ab), das Punktwerkzeug endet ohne
  die Auswahl zu leeren.
- **Orientierung der Maßkette (T0166)**: Eine Maßkette hat **eine** Orientierung
  — `Dim::angle` (Bogenmaß, Richtung der Maßlinie). Waagerecht (0°) und senkrecht
  (π/2) sind nur zwei Winkel desselben Objekts; `DimAxis` ist seitdem **keine
  Objekteigenschaft** mehr, sondern die bequeme Eingabeweise (`x`/`y`/`aligned`)
  und wandert über `DimAxis::winkel(pts)` in den Winkel. Drehen und Spiegeln
  ändern den Winkel mit (`transform::transform_entity`: `+ rotation()`, gespiegelt
  `rotation() − angle`), deshalb dreht die Rotationsfunktion die **ganze**
  Darstellung (Maßlinie, Hilfslinien, Maßtext) — vorher wanderten nur die
  Messpunkte und die Maßlinie blieb waagerecht stehen. Alle Rechnungen hängen an
  `render::dim_achse(angle)` (Richtung + Abstandsrichtung quer, positiv links der
  Messrichtung); `dim_projiziert`/`dim_sortieren`/`dim_offset` gibt es nur einmal
  für jeden Winkel. Alte Dateien rechnet `persist::migrate_legacy_dims` beim
  Laden um (`axis` → `angle`). Prüfweg: `cargo test -p jjcad-core --test
  masskette_rotieren`, Prüfbild `--demo masskettengedreht`.
- **Bemaßung beim Ändern**: Die Maßlinie behält ihre **Lage**. Ein waagerechtes
  Maß liegt auf *einer* Höhe (`line_pos.y` für alle Basispunkte), ein senkrechtes
  auf einer Breite — `render::dim_projiziert` **setzt** die Koordinate, es addiert
  keinen Versatz je Punkt. Ein Versatz je Basispunkt kippte die Maßlinie, sobald
  ein Punkt seitlich wanderte (T0055). Griffe: Quadrat je Basispunkt, Raute in der
  Mitte der Maßlinie (verschiebt sie senkrecht zur Messrichtung, `render::dim_line_move`);
  im Eigenschaften-Panel steht derselbe Wert als „Abstand“ (`render::dim_offset`).
- **Polarfang** 45° (Toleranz 6°); Fangmodi END, SCHN, MIT, ZEN, QUA, LOT, NÄC, RAS, POL.
  Das Winkel-Fenster **allein** wäre ein Kegel: die seitliche Abweichung wächst mit dem
  Abstand (bei 6° und 20 m über 2 m) und weit entfernte Punkte rasteten auf eine Richtung,
  die der Zeiger nicht meint (T0148). `Vec2::polar_snap(base, step, tol_deg, band)` prüft
  deshalb **beides** — Winkel **und** seitlichen Abstand `band`; `band` ist der Fangradius
  in Weltkoordinaten (`tol` des Fangs, Vorgabe 12 px), also bildschirmkonstant. Der
  Fangbereich ist damit ein Band mit einer Spitze am Bezugspunkt: nah am Punkt begrenzt der
  Winkel, fern die Breite. Eine reine Winkelprüfung (`polar_snap_from_tol`) gibt es nicht mehr.
- **Splines fangen nur an ihren Stützpunkten (T0134)**: Die gezeichnete Kurve ist
  eine Zerlegung in viele kurze Teilstrecken; deren Enden sind **keine** Fangpunkte.
  Beim Ziehen an einem Stützpunkt wandert die Kurve mit, ein Fang auf einem
  Zwischenpunkt zöge den Zeiger auf eine Stelle, die im nächsten Bild woanders
  liegt — die Linie flackert und zittert. `snap` überspringt End- und Mittelpunkt
  für `EntityKind::Spline` und bietet stattdessen die Stützpunkte als Endpunkte an;
  Lot und „Nächster Punkt" auf der Kurve bleiben erhalten. Prüfweg:
  `cargo test -p jjcad-core --test snap_spline` und
  `cargo test -p jjcad-gui spline_griffzug`.
- **Polylinie mit einem Element (T0149)**: Ein Zug aus **zwei Punkten** (ein
  Segment) ist keine Polylinie, sondern eine **Linie** (mit Stützbogen ein
  Bogen). Die Regel steht einmal in `actions::polyline_kind` und gilt beim
  Zeichnen (`draw.polyline`) und beim Stutzen (`modify.trim`, dort sind die
  Stücke immer offen) — nicht an einer zweiten Stelle nachbauen. Ältere Dateien
  dürfen noch Zwei-Punkt-Züge enthalten; sie werden erst beim Stutzen zur Linie.
  Prüfweg: `cargo test -p jjcad-core --test polylinie_geschlossen`.
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
- Wände: Farbe (Linie) und Füllung (Fläche) getrennt; Schraffuren nach Tabelle 5;
  Ausrichtung Links/Mittig/Rechts.
- **Eigene Blöcke als Datei (T0128)**: „Block aus Auswahl“ legt **eine** Datei
  `<Wurzel>/blocks/<slug>.jjblk` an (`{"format":"jjcad-block","version":1,"block":{…}}`,
  Code in `crates/jjcad-core/src/bloecke.rs`); Kennung `usr:…` im Inhalt, Dateiname
  folgt dem Namen (`slug`), Umbenennen ist `rename` plus Feld. Das Verzeichnis ist
  die Wahrheit — kein Index. Beim Start liest `daten::lade_benutzerbloecke_mit_meldungen`
  das Verzeichnis, unlesbare Dateien werden gemeldet und übersprungen.
  `block.create`/`block.update`/`block.delete` schreiben bzw. löschen nur mit dem
  Parameter `dir` (die Oberfläche übergibt `prefs.blocks_dir()`) — ohne `dir`
  bleibt das Benutzerverzeichnis unberührt (Tests, MCP ohne Datenwurzel).

- **Culling (T0167)**: Beim Zeichnen entscheidet `culling.rs` je Grafikelement,
  ob es den Bereich schneidet, in dem gezeichnet wird — was ganz außerhalb liegt,
  wird nicht gemalt. Die **Displayliste bleibt vollständig** (PDF/PNG unverändert);
  geprüft wird gegen die Bildschirmhülle der Welt-Box plus halbe Strichbreite, ein
  Fehler darf nur mehr zeichnen, nie weniger. Der Prüfbetrieb sitzt als eigener,
  **anklickbarer** Streifen über der Leistungsanzeige (die Anzeige selbst bleibt
  ohne Klick-Ebene, T0068): Haken „Bereich verkleinern“ → Bereich 12,5 % je Seite
  kleiner, lila Rechteck zeigt die Grenze, Zeile „Culling: x von y gezeichnet ·
  z verworfen“. Prüfweg `cargo test -p jjcad-gui tests_culling`, Prüfbild
  `--demo culling`.

- **Unterteilungen in der Leistungsanzeige (T0173)**: Die Zeile „Unterteilungen: N“
  nennt die Sehnenpunkte, die in diesem Bild entstehen — die Zahl, die über die
  Bildkosten der Kurven entscheidet. Gezählt wird **im Zeichenweg**
  (`canvas::paint_prim`), wo die Punkte ohnehin entstehen: Punkte − 1 je Bogen und
  Ellipsenbogen, die Segmentzahl je Vollellipse (`ELLIPSE_SEGMENTE` = 72) und je
  Kreis (`canvas::kreis_segmente`, dieselbe Pfeilhöhen-Regel wie `kreis_zug`), dazu
  die Punkte der Punktfolgen (`Shape::Poly`). Eine **Polylinie der Zeichnung** steht
  in der Anzeigeliste als einzelne Linien (`render::render_polyline`) — unter
  „Polylinien“ zählen deshalb die Punktfolgen (Spline, Schraffurzug, Dämmband), nicht
  die Modell-Polylinie. `perf::Zaehlung` sind drei `Cell<usize>` in `PaintCtx`: kein
  Allokieren, keine zweite Zerlegung, und Vorschauen wie Ansichtsfenster tragen
  `zaehlung: None`. Vom Culling Verworfene zählen nicht mit, weil `paint_prim` für
  sie nie läuft. Der Tooltip nennt die Aufteilung (Bögen / Ellipsen / Polylinien) und
  die wirksame Pfeilhöhe in px. Prüfweg `cargo test -p jjcad-gui unterteilungen`,
  Prüfbild `--demo "perf,zoom:100"` (Zoom in Prozent: 100 = 1 px/mm).

- **Splines folgen der Pfeilhöhe (T0175)**: `geom::catmull_rom_points` tastet nicht
  mehr fest 32 Punkte je Spanne ab, sondern halbiert jede Spanne, bis der
  Sehnenfehler die Toleranz einhält (geprüft an Mitte und beiden Viertelstellen —
  die Mitte allein läuft bei einer S-Form durch die Sehne; Deckel
  `SPLINE_MAX_TIEFE` = 1024 Teilstrecken je Spanne). Die **Anzeige** flacht mit
  `render::SPLINE_TOL_PX` (0,15 px) / Zoom ab, `RenderCtx::screen` trägt den Zoom
  (`mit_abbildung`, gesetzt nur von der Zeichenfläche); Druck, PNG, DXF und
  Ansichtsfenster lassen ihn leer und bleiben weltfest bei `SPLINE_TOL_MM`
  (0,05 mm). Fang, Trefferprüfung, Fläche und Bounding-Box rechnen **immer**
  weltfest (`Spline::segments`) — die Auflösung im Bild darf den Fang nicht
  verschieben. Weil die Liste damit am Zoom hängt, vergleicht `DrawCacheKey` den
  Zoom mit **Hysterese** (`app::ZOOM_HYSTERESE` = 1,25): neu gebaut wird erst,
  wenn sich der Zoom um mehr als ein Viertel ändert, sonst baute jedes Bild beim
  Zoomen die Liste neu. Prüfweg `cargo test -p jjcad-core --test
  spline_aufloesung` und `cargo test -p jjcad-gui anzeige_schluessel_zoom`.

- **Dämmung und Abdichtung sind Linienarten, keine Muster (T0132)**: Die Muster
  „Dämmstoffe“, „Dämmstoffe Schlaufen“ und die drei „Abdichtungen“ sind
  **entfernt**. Dämmung zeichnet die Linienart `LineType::Insulation` (Schlaufenband,
  `docs/wissen/DAEMMUNG-SCHLAUFEN.md`), auch auf einer Wand (deren Kanten). Alte
  Zeichnungen mit diesen Mustern laden über den Serde-Alias an
  `FillPattern::Lines` als Mauerwerk; ihre Schlüssel (`daemmstoffe`,
  `daemmung_schlaufen`, `abdichtung_*`) liefern `None`.
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
  DIN 1356-1:2026-09, Tabelle 5 (17 Muster — Nr. 15 ist eine Linienart, T0132;
  `style::FillPattern`, `ALL` in der Reihenfolge der Norm); die Muster
  heißen nach ihrem Baustoff (Mauerwerk, Kunststoffe, Sand).
  **Dämmung (Nr. 14, T0136)** ist beides: Linienart `LineType::Insulation` und
  Muster `FillPattern::InsulationHatch` — parallele Schlaufenbänder aus
  derselben Kurve (`daemmung::band_fest`), aber mit **fester** Schlaufengröße
  (`p = r · H`) und Phase am Weltnullpunkt. Der Musterabstand ist die
  Dämmstärke H. Nicht die gestauchte Linienart-Kurve nehmen: sie rechnet die
  Periode je Bande vom Beschnitt und die Schlaufen stünden von Reihe zu Reihe
  verschieden (`tests/schraffur_daemmung.rs` prüft das Raster). Gezeichnet werden
  **alle** Muster an einer Stelle
  (`render::push_muster`) — Füllfläche, Volumen und Wandschraffur teilen sie, keine
  zweite Musterzeichnung. Schlüssel über `FillPattern::key`/`from_key`, die alten
  Schlüssel `lines`/`cross`/`dots` bleiben gültig (alte Dateien).
  **Züge statt Strecken (T0164)**: Das Muster entsteht aus einer dichten
  Punktfolge; `render::muster_kurve_ablegen` legt einen zusammenhängenden Lauf
  als **einen** Polygonzug ab (der Beschnitt trennt nur an den Kanten). Eine
  eigene Strecke je Stützpunkt ergab bei 100 m² Dämmung 144 004 Primitiva und
  31 ms Bildzeit, als Zug sind es 49 Primitiva und 2,5 ms — bei gleicher
  Dämmung aus Linien 18 ms. `render::entity_bbox` nimmt für Füllfläche und
  Volumen die **Umgrenzung** statt das Muster zu rendern (vorher 6 ms je
  Aufruf, und Trefferprüfung und Auswahl fragen sie mehrmals je Bild); beim
  Raumstempel bleibt der Renderweg, weil er nur seine Beschriftung zeichnet.
  Die Anzeige malt dünne Züge (unter 2 px) als **eine** Form
  (`canvas::stroke_pts`), Kapseln je Strecke erst darüber.
  **Dämmung als Linie (T0168)**: Die Linienart `LineType::Insulation` legt je
  Linie ein Schlaufenband ab (10 m bei H = 200 mm sind 2500 Punkte). Drei
  Stellen waren darauf nicht vorbereitet: `render::entity_bbox` rechnet die Box
  aus dem **Weg** (`daemmung_huelle`: je Strecke um H/2 quer versetzt), nicht
  aus dem Band — Fang und Trefferprüfung fragen sie je Bild mehrfach
  (2,9 ms → 0,0 ms je Abfrage); `snap::hit_test` prüft erst die Ausdehnung,
  bevor es das Band eines Objekts rendert (7,4 → 0,1 ms je Klick); und
  `canvas::stroke_pts` malt einen **dichten** Zug (ab `DICHTER_ZUG_PUNKTE` =
  128 Punkten) auch bei breitem Strich als eine Form mit zwei Kreisen für die
  runden Enden — hervorgehoben oder ausgewählt entstand sonst je Strecke eine
  Kapsel (17,3 → 1,3 ms Bildzeit bei 100 m² Dämmung). Die Hülle wächst nur in
  Querrichtung, nicht rundum: eine runde Hülle machte die Zeichnung um H/2
  breiter, und `Document::bbox` bestimmt die Größe von PNG und PDF.
  Prüfweg: `cargo test -p jjcad-core --test daemmung_linie` und
  `cargo test -p jjcad-gui hervorgehobene_daemmungslinie`.
  **Ausnahme Wand (T0161)**: In einer Wand zeichnet die Schraffur „Dämmung“
  **kein** Muster, sondern **eine** Linie mit der Linienart „Dämmung“ auf der
  Wandmitte (Bandhöhe = Wanddicke, je Wandstück eine). Die parallelen Bänder
  gehören auf eine Fläche; in einer Wand füllten sie den Körper mit Reihen
  (`render::push_wall_hatch` → `render::wall_insulation_paths`).
  **Boden unverändert (T0163)** ist eine feste **Einheitszelle** von 8 × 8
  Musterabständen: vier durchgezogene Senkrechte, vier lange und vier kurze
  Waagerechte, vier Haken in S-Form (`muster_boden_unveraendert`). Die Zelle
  steht in der Zeichnung des Nutzers rechtwinklig; im Musterraster liegt sie
  **um 90° gedreht**, damit ihre Längsrichtung (die durchgezogenen Linien) bei
  Vorgabe-Musterwinkel (+45°) unter −45° läuft — so zeigt es die Vorlage aus
  T0152, mit der das Muster zellengleich übereinstimmt (Deckung 1,000).
  Ohne diesen Versatz steht das Muster spiegelverkehrt zur Vorlage; der
  Musterwinkel allein kann eine Spiegelung nicht ausgleichen.
  Prüfweg: `cargo test -p jjcad-core --test schraffuren_din
  boden_unveraendert` (vergleicht das gezeichnete Bild mit der Zelle aus der
  Zeichnung des Nutzers, bei Musterwinkel 0 und bei 45°).
  **Boden verändert (T0170)** ist ebenfalls eine **Einheitszelle** aus der
  Zeichnung des Nutzers: ein Schachbrett aus Feldern von 6 × 6 Musterabständen
  (Zelle 12 × 12), in jedem Feld **drei** Striche der Länge 6 s im Abstand 2 s,
  abwechselnd senkrecht und waagerecht (`muster_boden_veraendert`). Die Striche
  enden auf der Feldkante — sie kreuzen sich **nie**; der vorherige Stand
  zeichnete zwei freie Linienscharen, die sich überkreuzten und deshalb falsch
  aussahen. Als Schraffur steht die Zelle unter 45°; weil sie gegen Drehung um
  90° gleich ist (das Schachbrett verschiebt sich nur um ein Feld), ist die
  Drehrichtung beliebig. Prüfweg: `cargo test -p jjcad-core --test
  schraffuren_din boden_veraendert` (Zellvergleich bei 0° und 45°, dazu die
  Probe, dass sich keine zwei Striche kreuzen).
  Ein neues Muster: Variante und `ALL` in `style.rs`, Zweig in `push_muster`,
  Bezeichnung in `TABELLE_5` des Tests `tests/schraffuren_din.rs` — der Test bricht,
  wenn ein Muster fehlt, nichts zeichnet oder wie ein anderes aussieht.
  **Alle Muster hängen an einem Raster mit festem Ursprung** (`render::Raster`,
  Weltnullpunkt): Linienlage `(i + versatz) · Abstand`. Die Unterteilungen
  (Striche, Gruppen) zählen **in der Höhe** — die Lücken liegen auf
  waagerechten Taktlinien `y = k · Takt` (bei waagerechten Linien auf
  senkrechten), nicht längs der Linie. Längs gezählt wandert die Lücke von
  Reihe zu Reihe und bildet eine schiefe Naht (T0095 Turn 2/4); am Anfang des
  beschnittenen Stücks neu zu beginnen macht das Muster zufällig. Figuren
  (Kiesel, Schlaufen, Pfeile, Felder) sitzen in Rasterzellen und werden nur
  **ganz** gezeichnet. Prüfweg: `cargo test -p jjcad-core --test schraffuren_din`
  (`unterbrechungen_liegen_auf_waagerechten_linien`,
  `unterteilungen_haengen_am_globalen_raster`,
  `unterteilungen_sind_in_jeder_reihe_gleichmaessig`,
  `figuren_sitzen_auf_dem_raster`) — Gegenprobe ist in `muster_striche` die
  Bezugsachse auf die Linienrichtung zu setzen (`let (achse, start) = (1.0, 0.0)`).
- **Auswahl über Rechteck** prüft echte Geometrie (Segmente, Wandkörper),
  nicht die achsenparallele Bounding-Box.
- **Zerlegen trifft die Fläche (T0138)**: `snap::hit_test_flaeche` nimmt eine
  Füllfläche auch **innen** (Rückfall, wenn kein Objekt darüber liegt) — genutzt
  von Zerlegen (Werkzeug, Vorschau) **und vom Rechtsklick**: erst dadurch öffnet
  ein Klick in die Schraffur das Objektmenü mit „Zerlegen“ statt des
  Flächenmenüs. `snap::hit_test` bleibt beim Randtreffer, weil der Farbeimer
  dieselbe Funktion fragt. Ein Musterpunkt
  (`Shape::Dot`) wird beim Zerlegen zum gefüllten Kreis in der Musterfarbe
  (`style.fill`; Anzeige, Raster und PDF zeichnen einen gefüllten Kreis als
  Scheibe); ein Punktobjekt geriete über Linienstärke × Beschriftungsmaßstab
  auf das Neunfache.
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
- **Einstellungsfenster mit fester Größe (T0135)**: Das Fenster hat in **allen**
  Kategorien dieselbe Größe (`SETTINGS_BREITE`/`SETTINGS_HOEHE_GESAMT` in `dialogs.rs`).
  Eine Zeile, die ihre Breite aus `ui.available_width()` nimmt, treibt ein
  selbstgroßes egui-Fenster Bild für Bild auf — so wuchs „Speicherorte“ um 45 px,
  weil die Knopfzeile (2 × 101 px) breiter war als der feste Abzug von 210 px.
  Die Seite rechts neben der Kategorienliste ist deshalb auf `SETTINGS_SEITE_W`
  begrenzt, und die Pfadzeile setzt ihre Knöpfe **rechts nach links**
  (`allocate_ui_with_layout` mit fester Höhe — `with_layout` allein füllt den
  restlichen Platz und schiebt alle folgenden Zeilen nach unten). Prüfweg:
  `cargo test -p jjcad-gui einstellungsfenster_gleich_gross_in_allen_kategorien`
  (vergleicht die Fensterfläche je Kategorie; vor dem Fix 1021 px gegen 976 px).
- **Einstellungsfenster (T0061)**: links die Kategorienliste
  (`dialogs::EINSTELLUNGSKATEGORIEN`, Icon + Name), darüber das Suchfeld,
  rechts die Einstellungen der gewählten Kategorie. Beide Spalten scrollen getrennt.
  Eine neue Einstellung wird in ihrer Kategorie über `dialogs::zeile`/`zeile_mit`
  gezeichnet **und** ihr Name in den `stichworte`n der Kategorie eingetragen — sonst
  findet die Suche sie nicht. Zeilennamen bleiben kurz (Namensspalte 160 px), lange
  Begriffe kommen als Alias in `zeile_mit`; Häkchen ohne Wert stehen mit vollem Text.
- **Einstellungen aufgeräumt (T0178)**: Die Kategorien stehen in dieser Reihenfolge:
  Bedienung, Darstellung, Grafik, Raster, Fang, Voreinstellungen, Architektur,
  Bemaßung, Update (leerer Platzhalter), Speicherorte, Sicherung. Der Zweig in
  `settings_body` hängt am **Katalogschlüssel** (`EINSTELLUNGSKATEGORIEN[kat].key`),
  nicht am Index — mit Index zeigte eine Umsortierung die Einstellungen einer
  fremden Kategorie. Jede Beschreibung steht als **Tooltip am Infozeichen** der
  Zeile (`dialogs::zeile_tip` → `widgets::field_row_tip`), nicht als Absatz
  darunter; kein Zweig schreibt eigene Hinweistexte. **Auswahlfelder sind so hoch
  wie Eingabefelder**: maßgeblich ist `widgets::FELD_H` (24 px) — ein Zahlenfeld
  nimmt sich mindestens `interact_size.y` der Vorlage und lässt sich mit
  `add_sized` nicht kleiner ziehen (gemessen: 24 px gezeichnet, egal welche Höhe
  vorgegeben wird). `panels::DROPDOWN_H` ist deshalb `widgets::FELD_H + 1`
  (25 px): das Zahlenfeld zeichnet seinen Rahmen eine halbe Strichbreite über
  die Fläche hinaus, der Auswahlknopf innerhalb — mit gleicher Vorgabe wäre er
  sichtbar einen Pixel niedriger (gemessen 26 gegen 27 px im Bild). Auswahlfelder
  im Einstellungsfenster laufen über `dialogs::auswahl`, im Panel über
  `panels::dropdown`; `egui::ComboBox` ist überall ersetzt (niedriger als ein
  Feld, siehe unten). Die gewählte Kategorie trägt den
  **Farbakzent** (weiche Akzentfläche, 2-px-Streifen, Akzenttext), nicht Blau.
  „Einheiten und Maßstab“ ist entfallen — Einheit, Dezimalen, Rasterweite und
  Maßstab stehen im Eigenschaften-Panel der Zeichnung (`panels::doc_props`).
  „Lineale einblenden“ ist entfernt: `show_rulers` wurde nirgends gezeichnet.
- **Ansicht verschieben: zwei Bewegungsarten (T0101)**: Einstellungen → Bedienung →
  „Verschieben" wählt `prefs.pan_mode` — **Kontinuierlich** (Vorgabe) hält die
  Ansicht in Bewegung, solange die Schwenkfunktion gedrückt bleibt, **Sprunghaft**
  rückt je Druck um einen Schritt. Der eine Aufrufweg bleibt
  `App::schwenk_funktion` aus `view.pan_*` (Taste, Hotbar, Ribbon): im Sprungmodus
  ein `App::schwenk_schritt` (`SCHWENK_SCHRITT_PX` = 60 px), im Dauerlauf **kein**
  Schritt — den Weg rechnet allein `App::schwenk_fortschritt` je Bild aus
  `SCHWENK_TEMPO_PX_PRO_S` × Bilddauer (`stable_dt`, gedeckelt auf 0,1 s). Ein
  Schritt daneben zählte die Tastenwiederholung doppelt: gemessen 700–850 px/s
  statt 480. Den Dauerlauf nicht als zweiten `key_down`-Lauf neben die Funktion
  bauen (das war T0089): er liest die Belegung über `prefs.keys_for`, die
  Hotbar-Ziffern und einen gedrückt gehaltenen Slot (`UiState::schwenk_zeiger`) und
  schweigt bei Strg, offenem Protokoll und laufender Texteingabe.
- **Schwenkgeschwindigkeit (T0067)**: Einstellungen → Bedienung → „Schwenk" ist ein
  Faktor (`prefs.pan_speed`, 0,1…5, Vorgabe 1) auf den Schritt (`SCHWENK_SCHRITT_PX`
  = 60 px) **und** auf das Tempo des Dauerlaufs — ein Faktor für beide
  Bewegungsarten.

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
  **Vorschau (T0122)**: Beim Setzen zeigt `render::preview_elevation_section`
  das **fertige** Objekt für die gesetzten Punkte plus den Punkt am Zeiger —
  Dreiecke, Füllung ab dem zweiten Punkt und die Höhenangabe unter jedem
  Dreieck, aus demselben Renderer wie die eingefügte Kote. Kein eigener
  Platzhalter im Zeichner: der zeigte nur Umrisse, die Kote wurde blind
  gesetzt. Prüfweg: `cargo test -p jjcad-core --test schnittkote_vorschau`
  (Vorschau zeilengleich mit dem Objekt) und
  `canvas::tests_hoehenkoten::vorschau_der_schnitt_kote_zeigt_die_hoehen`;
  Prüfbild `--demo koteschnitt`.

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