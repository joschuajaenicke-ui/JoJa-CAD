# JoJa-CAD v0.2 — Arbeitsstand

Fortlaufender Stand der Umsetzung. Stand: 08.11.2026

## Build & Start

```
cargo build --release -p jjcad-gui      # Release
cargo build -p jjcad-gui                # Debug
./target/release/jjcad samples/Wohnhaus.jjcad
```

Nach jedem Build **laufende Instanz schließen und neu starten** — eine bereits
offene Fensterinstanz zeigt weiterhin den alten Stand.

## Erledigt (verifiziert)

- **Helles und dunkles Design (T0102)**: Einstellungen → Darstellung → Zeile „Design“
  mit **System** (Vorgabe), **Hell** und **Dunkel**; gespeichert als
  `UserPrefs.design` (`system`/`hell`/`dunkel`) in `prefs.json`, Wechsel wirkt
  sofort ohne Neustart. Die Farben stehen in zwei Tabellen in `theme.rs`; alle
  Zeichenwege lesen `Palette::…()`. „System“ folgt dem Betriebssystem
  (`ctx.system_theme()`, unter Linux `kdeglobals`/GTK-`settings.ini`/Umgebung) und
  zieht einen Wechsel höchstens alle 2 s nach. Papier und Raster bleiben in beiden
  Designs gleich. Tests: `theme::tests` (3), `persist::tests::design_wird_gespeichert_und_gelesen`,
  `dialogs::tests_einstellungen::design_zeile_schaltet_das_helle_design`.

- **Fenster ohne Titelleiste (T0098)**: Einstellungen → Bedienung → Zeile
  „Fensterrahmen“ mit **System** (Vorgabe) und **Ohne Titelleiste** (experimentell,
  unter KDE geprüft). Gespeichert als `UserPrefs.fensterrahmen` (`system` /
  `ohne_titelleiste`) in `prefs.json`; die Erzeugung des Fensters liest sie schon
  vor dem Start (`ViewportBuilder::with_decorations`), ein Wechsel im Dialog
  schickt `ViewportCommand::Decorations` sofort nach. Ohne Titelleiste zeichnet
  die Kopfzeile oben rechts **Minimieren**, **Vergrößern/Wiederherstellen** und
  **Schließen**, ein Zug verschiebt das Fenster (`StartDrag`), ein Doppelklick
  schaltet die Größe um, die Kantenzonen (8 px, Ecken 24 px) ändern sie
  (`BeginResize`): der Zeiger zeigt dort sofort den Größenpfeil, die Kante leuchtet
  nach 0,1 s Verweilen auf (gedämpft, 1,5 px bei 55 % — Turn 7: die volle
  Markierung blitzte bei jeder schnellen Bewegung über den Rand auf) und bleibt
  während des Ziehens stehen (Turn 9) und endet danach zuverlässig (Turn 11): sie
  hängt an der Größenänderung (Nachlauf 0,6 s, gerundete Pixel gegen Zappeln bei
  krummer Skalierung), endet sofort, wenn der Zeiger frisch bewegt mehr als 40 px
  von der Kante weg ist, und spätestens nach 20 s. Mit eigener Bildanforderung,
  sonst würde der Nachlauf im Ruhezustand nie ausgewertet. Unten rechts sitzt ein
  sichtbarer Griff. Die Ecke oben rechts gehört den
  Fensterknöpfen (Turn 5: 5 px waren unsichtbar und nicht auffindbar).
  Nach dem Zug beendet `Context::stop_dragging` egui's Zieh-Zustand: der
  Fenstermanager übernimmt den Zeiger und schickt das Loslassen an niemanden,
  sonst bliebe egui „am Ziehen“ und **jeder zweite** Zug verschöbe nichts (Turn 3).
  Tests: `app::tests_fensterrahmen` (17), `dialogs::tests_einstellungen::fensterrahmen_laesst_sich_umschalten`,
  `persist::tests::fensterrahmen_vorgabe_und_gespeichert`.
- **Drehpunkt der Einfüge-Vorschau (T0070)**: Einstellungen → Bedienung → Zeile
  „Drehpunkt" mit den Modi **Mittelpunkt** (Vorgabe seit T0087) und
  **Basispunkt**; gespeichert als `UserPrefs.paste_pivot` (`base_point`/`center`)
  in `prefs.json`. In beiden Modi dreht die Auswahl um ihren Mittelpunkt; der Modus
  entscheidet, welcher Punkt am Fadenkreuz steht (`ghost::clipboard_entities`).
  Mittelpunkt = untere linke Ecke der gedrehten Auswahl, sodass die Vorschau
  immer oben rechts vom Fadenkreuz bleibt. Vorschau und eingefügtes Ergebnis
  rechnen dieselbe Funktion. Ein Wechsel in den Einstellungen wirkt sofort, auch
  auf eine laufende Vorschau (`App::set_paste_pivot`, T0087). Tests: `ghost::tests_drehpunkt`,
  `app::tests_drehpunkt_einfuegen`, `dialogs::tests_einstellungen::bedienung_zeigt_den_drehpunkt`,
  `persist::tests::drehpunkt_der_einfuege_vorschau_wird_gespeichert`.
- **Wandanschlüsse: allgemeine Gehrung** (`crates/jjcad-core/src/wall.rs`,
  Herleitung in `docs/WANDANSCHLUSS.md`): An einem verbundenen Ende wird jede
  Wand an der **Anschlussnaht** abgeschnitten — der Geraden durch die beiden
  Gehrungspunkte `S_l` (Schnitt der linken Kanten) und `S_r` (Schnitt der
  rechten Kanten), jeweils bezogen auf die Laufrichtung am Knoten. Das gilt für
  beliebige Dicken, beliebigen Versatz (links/mittig/rechts) und beliebige
  Winkel; bei gleich dicken, mittigen Wänden ist es die Winkelhalbierende.
  Beide Wände rechnen **dieselben zwei Punkte** — keine zwei Nähte mehr
  nebeneinander (das war der Kern des alten Fehlers).
  - Verbunden = **Basispunkte** zusammen (0,5 mm). Der Körper wird am
    verbundenen Ende um die Reichweite der Naht verlängert, damit die Gehrung
    die Ecke **ergänzt** statt sie nur abzuschneiden.
  - Grenzen (alle **beidseitig** — dieselbe Entscheidung für beide Wände, sonst
    schneidet die eine und die andere nicht; genau daraus entstanden Finne und
    Überlappung): fluchtend/rückläufig → keine Naht (Wand läuft durch, Kreuz
    und Verlängerung bleiben gerade); **Öffnungswinkel < 40°**
    (`MIN_JOINT_ANGLE`) → stumpf, sonst schießt die Wand als meterlange Finne
    über den Knoten; Nahtlänge > 6 · Wandbreite der dünneren Wand
    (`MITER_RATIO`) → stumpf; nimmt der Schnitt mehr als ein Viertel einer
    Wandquerschnittsfläche weg, die der Nachbar nicht stellt (Wände auf
    entgegengesetzten Seiten ihrer Basislinien) → stumpf, sonst Loch; liegt ein
    Gehrungspunkt weiter vom Knoten als die **kürzere** Wand lang ist → stumpf.
  - Gezeichnet wird die **Vereinigung**: eine Randkante nur, wo außen keine
    andere Wand liegt. Gehrung, Stumpfstoß, Stich, Kreuz und Mehrfachknoten
    brauchen damit keinen Sonderfall.
  - Fang nutzt dieselbe Geometrie. Gegen den Antialiasing-Haarstrich sind die
    **Füllflächen** an der Naht um 0,6 mm (`FILL_BLEED`) in die Nachbarwand
    ausgebucht (`Body::fills`) und überlappen dort; die Geometrie (`pieces`),
    Umrandung und Fang bleiben exakt. Ein eigenes Bandpolygon taugt nicht: bei
    breitenabhängiger Tastung war es als Kante zu sehen und flackerte, bei
    0,4 mm Breite ist es unter einem Pixel und wird von der GUI verworfen —
    dann bleibt der Haarstrich stehen.
  - Körper werden **bei Bedarf** gerechnet (OnceCell) — der Treffer-Test fragt
    die Bounding-Box jeder Wand ab; `detect_boundary` auf dem Wohnhaus bleibt
    damit unter 100 ms (vorher 127 ms, mit eifriger Rechnung 222 ms).
  - Abnahme: `tests/wandanschluss.rs` — 8 Winkel × 4 Dickenpaare × 5
    Ausrichtungen (160 Ecken), geprüft auf „keine Lücke" (Vektor **und**
    Pixelbild), „keine Naht im Körper", „jede Füllkante umrandet"; gemeinsame
    Gehrung gleich dicker Wände; drei Wände in einem Punkt ohne Splitter;
    Stich/Kreuzung; Reihenfolge der Wände ohne Einfluss. Dazu die bestehenden
    Tests `wand_ecken.rs`, `wandgehrung_spitz.rs`, `snap_wandfang.rs`
    (dort jetzt: „Körper gefüllt **bis zur Naht**").
  - Messhilfen: `examples/wand_probe.rs` (Fälle als PNG + Kennzahlen),
    `examples/zeichnung_png.rs <datei.jjcad> [ziel] [x0 y0 x1 y1]`,
    `samples/Wandpruefung.jjcad`.
- **Volumen** (`crates/jjcad-core/src/model.rs`, `EntityKind::Volume`): ein
  Architekturelement zwischen Wand und Füllfläche. Es hat die Geometrie einer
  Füllfläche (Außenring + Inseln, Muster, Randlinie) **und eine Höhe**; in den
  Eigenschaften stehen Höhe, Fläche und Volumen in m³.
  - **Es verbindet sich mit den Wänden**: Wandkanten fallen aus, wo sie im
    Volumen liegen; die Umrandung des Volumens fällt aus, wo sie in einer Wand
    liegt. Beide Seiten prüfen dieselbe Regel, deshalb steht an der Naht keine
    Linie — ohne dass Geometrie verschmolzen wird. Kanten werden dafür an den
    Rändern der jeweils anderen Fläche unterteilt (`render::sichtbare_stuecke`),
    damit nur das verdeckte Stück wegfällt.
  - Die Fläche wird standardmäßig im **Wandton** (74 % Richtung Weiß) gefüllt,
    damit der Übergang auch als Farbe nicht auffällt (`render::WALL_FILL_MIX`).
  - Gedacht für Mauerwerk, das sich nicht als Rechteckwand zeichnen lässt:
    runde Wand, Pfeiler, Sonderform. Beispiel: `samples/Volumen.jjcad`
    (Halbkreisbogen auf zwei geraden Wänden, erzeugt von
    `examples/volumen_erzeugen.rs`).
  - Aktionen wie bei der Füllfläche: `draw.volume` (Kontur anklicken),
    `draw.volume.points` (Punkte setzen) und `draw.volume.paint` (Farbeimer) —
    erreichbar über die geteilte Schaltfläche **Architektur → Volumen**
    (Würfel-Symbol) mit Modus-Menü; `modify.set height` ändert die Höhe.
  - Wände tragen ihre Höhe schon als Metadatum; das Eigenschaften-Panel zeigt
    jetzt zusätzlich das Volumen (Länge × Dicke × Höhe, brutto).
  - Abnahme: `tests/volumen.rs` — Anschluss in beiden Richtungen ohne Linie
    (Vektor **und** Pixelbild), gleicher Ton wie die Wand, Höhe/Volumen in m³,
    Höhenänderung über `modify.set`, Speichern/Laden, alle drei Eingaben
    (`volumen_drei_eingaben`, Farbeimer über `detect_boundary`).
- **Schraffuren nach DIN 1356-1:2026-09, Tabelle 5** (T0095): 17 der 18
  Kennzeichnungen der Schnittflächen als Muster in `style::FillPattern`
  (`ALL` in der Reihenfolge der Norm); Nr. 15 (Abdichtungen) ist eine
  Linienart und deshalb kein Muster (T0132), Nr. 14 (Dämmung) ist beides —
  Linienart **und** Muster aus parallelen Schlaufenbändern (T0136). Die Muster
  heißen nach ihrem Baustoff (Mauerwerk, Kunststoffe, Sand). Ein Generator
  (`render::push_muster`) zeichnet sie für Füllfläche, Volumen **und**
  Wandschraffur; Schlüssel und Bezeichnung finden jedes Muster wieder, die
  alten Dateinamen bleiben gültig. Prüfblatt `samples/Schraffuren.jjcad`
  (Erzeuger `examples/schraffuren_erzeugen.rs`), Test
  `crates/jjcad-core/tests/schraffuren_din.rs`.
  **Ein Raster für alle Muster** (`render::Raster`, Ursprung im Weltnullpunkt):
  Lage der Musterlinien `(i + versatz) · Abstand`, Figuren in Rasterzellen.
  Die Unterteilungen (Striche, Gruppen) zählen **in der Höhe**: die Lücken
  liegen auf waagerechten Taktlinien, wie in der Norm. Vorher begann jede
  Linie ihr Strichmuster am eigenen Beschnitt (Turn 2), danach wanderte die
  Lücke längs der 45°-Linie von Reihe zu Reihe und bildete eine schiefe Naht
  (Turn 4).
- **Wandschraffur**: `hatch` an der Wand (alle Muster der Tabelle 5,
  **Dämmung = parallele Schlaufenbänder**), gezeichnet als
  Displaylisten-Primitive.
- **Wandausrichtung**: `WallAlign` (Links/Mittig/Rechts), Versatz ±Dicke/2,
  Tab wechselt beim Platzieren (Zyklus, `consume_key`), Wert landet in `draw.wall`.
- **Vorschau**: Wandkörper mit Ausrichtung + gestrichelter Achse; Tür/Fenster
  springen auf die nächste Wand und drehen sich in Wandrichtung.
- **Türen/Fenster**: Host-Bindung, Öffnung im Wandkörper (Ausrichtung der Wand
  berücksichtigt), R = Blattseite, F = Anschlag. Eigenschaft „Blatt: Seite A/B".
- **Farben/Stärken**: Vorgaben-Dropdowns (0,50/0,35/0,25 mm; 13 Farben),
  freier Wert inline, Komma und Punkt als Dezimalzeichen.
- **Eigenschaften-Panel**: einheitliche Zeilen (96 px Label, 150 px Wert),
  alle Auswahllisten gleiche Optik/Größe, Identisch während/nach dem Platzieren
  (Unterschied: Untertitel + Geometrie-Block).
- **Zieh-Modus als Enum**: `persist::DragMode` (`hold_and_drag` = Standard,
  `two_clicks`) liegt als `UserPrefs.drag_mode` in `prefs.json`; alte Dateien
  ohne Schlüssel ergeben `hold_and_drag`, das Altfeld `drag_two_clicks` aus
  Build b7 wird beim Laden übernommen. Nur Typ/Persistenz umgestellt — die
  Bedienung ist unverändert (Auswahlfeld-Optik und Zwei-Klick-Ablauf folgen).
- **Punktfang auf Wandkontur** (`snap.wall_edges`, Standard an): Die umgrenzenden
  Wandlinien (Längskanten, Gehrungs-/Stirnecken, Laibungen an Öffnungen) liefern
  Fangpunkte — Endpunkt, Mittelpunkt, Nächster Punkt und Schnittpunkte mit anderen
  Wänden. EINE Geometriequelle: `wall::WallIndex` (Körper + Randstücke) versorgt
  Darstellung (`render_wall`) und Fang (`render::wall_contour_segments`); Kandidaten werden in
  `snap::snap_candidates` entdoppelt (0,1 mm). Nachweis: Wand 0,0→5000,0, 300 mm —
  Zeiger 5 px neben der Ecke → Status „X 0,00 Y 0,15 · Endpunkt", Ring auf der
  Ecke; `wall_edges=false` → (-0,02; 0,17) ohne Fang, Achse bleibt fangbar.

- **Wandgehrung an extrem spitzen Ecken / ungleich dicken Wänden**: Der
  Beschnitt-Apparat von T0020 (`miter_patch`, `miter_dicht`, `keil_gedeckt`,
  `line_outside_pieces`, `partner_body`) ist **entfernt** — er hat je Wand eine
  eigene Naht gerechnet und bei versetzten Achsen zwei Nähte nebeneinander
  erzeugt. An seine Stelle tritt die gemeinsame Anschlussnaht in
  `crates/jjcad-core/src/wall.rs` (siehe erster Punkt). Nachweis:
  `tests/wandgehrung_spitz.rs` (7 Tests) und `tests/wandanschluss.rs` bleiben
  grün, ohne Patch und ohne Sonderfall.

- **Speicherorte (T0127)**: Alle Nutzdaten liegen in **einer** Wurzel
  (`prefs.json`, `sitzung.json`, `blocks/`, `sicherungen/`, `log/`); die
  Zeichnungen bleiben, wo der Benutzer sie ablegt. Auflösung in
  `daten::wurzel_ermitteln`: `--daten-dir` → `$JJCAD_DATA_DIR` → `portable.txt`
  neben der Programmdatei → Plattformstandard. Beim ersten Start wandert eine
  vorhandene `prefs.json` aus `~/.config/jjcad` in die Wurzel (die alte bleibt
  liegen). `sperre.lock` trägt die PID — eine zweite Instanz auf derselben
  Wurzel zeigt „Datenordner belegt" statt zu schreiben. Die Unterordner sind
  unter Einstellungen → **Speicherorte** änderbar (Schreibprobe lehnt einen
  nicht beschreibbaren Pfad ab), Menüband *Datei* → **Datenordner** öffnet die
  Wurzel. Nachweis: 8 Tests in `daten.rs`, `cargo test` grün, Kopf der
  `prefs.json` liegt nach dem Start in der Wurzel.

- **Eigene Blöcke als Datei (T0128)**: „Block aus Auswahl" schreibt
  `<Wurzel>/blocks/<slug>.jjblk` (`{"format":"jjcad-block","version":1,…}`),
  eine Datei je Block; die Kennung `usr:…` steht im Inhalt und bleibt beim
  Umbenennen. Der Start liest das Verzeichnis (kein Index); eine unlesbare Datei
  wird gemeldet und übersprungen. Blockmanager zeigt die Herkunft **Benutzer**.
  Nachweis: `tests/bloecke_datei.rs` (6 Tests).

- **Sicherung (T0129)**: Stände im Pool statt einer Autosave-Datei, **ohne** je
  in die Zieldatei zu schreiben — gespeichert wird nur auf Befehl. Zyklus nur
  bei geänderter Zeichnung (Hash), Ausdünnen nach Fenster/Obergrenze/Kosten,
  Pins je Dokument unantastbar, Waisen beim Start entfernt. Einstellungen →
  **Sicherung**: Verlauf an/aus, Plätze 30–2000 (Vorgabe 100), Reichweite,
  Intervall 1–240 min (Vorgabe 1), Abstandsanzeige, Poolgröße, Hinweis zum
  Netzlaufwerk. Wiederherstellen über `file.restore` öffnet ein **neues**
  Dokument. Doku: `docs/SICHERUNG.md` (korrigiert), `docs/FORMAT.md`
  (`Document.id`). Nachweis: 14 Tests in `sicherung.rs` (die Kriterien aus §13),
  `cargo test` grün; am laufenden Programm nach 70 s ein Stand, Zieldatei
  byte-gleich zum Git-Stand.

- **Sitzung (T0130)**: Beim Beenden wird der Zustand in
  `<Wurzel>/sitzung.json` gemerkt (Reiter in Reihenfolge, je Reiter Ansicht und
  Blatt, aktiver Reiter, Fensterlage) und beim Start wiederhergestellt; auch nie
  gespeicherte Zeichnungen kommen als „Unbenannt" aus dem Pool zurück. Beenden
  fragt nicht mehr nach dem Speichern; das Schließen eines Reiters fragt, sobald
  der Inhalt von der Zieldatei abweicht (Vergleich statt Merker). **Ungespeicherte
  Arbeit überlebt das Beenden**: sie wird als Stand abgelegt (unabhängig vom
  Intervall) und beim Start mit Speicherort, aber als geändert zurückgeholt; ein
  „Verwerfen“ setzt den neuesten Stand auf den Dateiinhalt. Auswahl,
  Undo-Verlauf, Werkzeugwahl und Zwischenablage bleiben draußen. Nachweis:
  8 Tests in `sitzung.rs`/`app.rs` (`ungespeicherte_arbeit_kommt_beim_start_zurueck`,
  `verworfene_arbeit_kommt_nicht_zurueck`), am Programm: ändern → beenden →
  starten zeigt 37 statt 36 Objekte, „Stand: geändert“, Zieldatei unverändert.

- **Fensterlage in der Sitzung (T0141)**: `sitzung.json` merkt beim Beenden
  Fenstergröße (jede Plattform), Maximiert-Zustand und — wo das Fenstersystem
  globale Koordinaten liefert — die Fensterposition. Bis hierher scheiterte das
  Merken unter Wayland **ganz**: `inner_rect`/`outer_rect` sind dort `None`
  (xdg-shell kennt keine Fensterposition), deshalb stand `fenster: null` in der
  Datei und auch die Größe kam nicht zurück. Jetzt liefert die Bildschirmfläche
  die Größe (`RawInput::screen_rect`, aus `window.inner_size()`), die
  Fensterrechtecke die Lage; fehlt die Lage, steht `mit_lage: false` statt die
  ganze Lage zu verwerfen (`x`/`y` bleiben Zahlen — eine `null` ließe ältere
  Fassungen an der Sitzung scheitern). Unter X11, Windows und macOS kommt auch
  die Position zurück; unter Wayland bleibt sie eine Grenze
  (`docs/BEKANNTE-GRENZEN.md`, Nr. 7).

  **Turn 2 — Vollbild.** Der Vollbild-Modus wird jetzt ebenfalls gemerkt
  (`Fensterlage::vollbild`, getrennt von `maximiert`: rahmenlos gegenüber
  Rahmen). Im Vollbild und maximiert schreibt die Sitzung die zuletzt bekannte
  **normale** Größe (`App::fenster_normal`, jedes Bild gepflegt), nicht die
  Bildschirmgröße — sonst stünde nach dem Verlassen des Vollbilds ein
  bildschirmgroßes Fenster da. Der Startzustand wird im ersten Bild gesetzt und
  bis zu 3 s wiederholt (`App::fenster_start_anwenden`): beim **Erzeugen** des
  Fensters verwirft KWin die `_NET_WM_STATE`-Nachricht (gemessen — das Vollbild
  klappt dort nur, weil winit es bis zur Sichtbarkeit zurückhält). Nachweis:
  `cargo test -p jjcad-gui fensterlage` (11 Tests), `cargo test -p jjcad-core
  --lib sitzung` (11), unter Xvfb mit kwin_x11: Vollbild beendet → Neustart mit
  `_NET_WM_STATE_FULLSCREEN` und gespeicherter Größe 1500 × 950; maximiert
  beendet → Neustart mit `_NET_WM_STATE_MAXIMIZED_VERT/HORZ`; F11 im
  Vollbild-Start führt zurück auf 1500 × 950. Die Lage bleibt unter Wayland
  eine Grenze (Nr. 7).

- **Eigene Vorlage für neue Dateien (T0162)**: Unter Einstellungen →
  **Speicherorte** steht unter den drei Dateipfaden die Zeile „Vorlage für neue
  Dateien“ (Feld, **Datei wählen …**, **Übernehmen**, **Zurücksetzen**). Der
  Pfad steht als `vorlage_pfad` in `prefs.json`, relativ zur Datenwurzel oder
  absolut; ein nicht vorhandener Pfad wird beim Übernehmen abgelehnt. Jede mit
  **Neu** angelegte Datei ist eine Kopie dieser Datei — Objekte, Ebenen,
  Blätter, Blöcke und Zeichnungseinstellungen kommen aus der Vorlage
  (`persist::lade_vorlage`), der Reiter bleibt neu (kein Pfad, nicht geändert,
  eigene Kennung). Eine unlesbare Vorlage meldet „Vorlage · nicht lesbar“ und
  die Datei beginnt leer. Nachweis: 4 Tests (`vorlage_wird_als_neue_zeichnung_geladen`,
  `vorlagenpfad_wird_gespeichert_und_aufgeloest`,
  `neue_datei_ist_kopie_der_vorlage`, `unlesbare_vorlage_beginnt_leer_und_meldet_sich`)
  und `speicherorte_zeigen_die_vorlage_und_oeffnen_den_dateidialog`.

## Offen

1. **Tür/Fenster-Ausrichtung im Eigenschaften-Panel** — beim Anlegen gesetzt,
   noch nicht nachträglich änderbar (braucht Wanddicke im Änderungspfad).
2. **Öffnungsposition als Wandparameter** (Abstand längs + Lage quer) statt
   absolutem Mittelpunkt — nötig, damit Öffnungen beim Verschieben der Wand folgen.
3. **Echte Polygon-Vereinigung der Wandfüllungen** — gezeichnet wird weiter je
   Wand (an der Naht beschnitten, Nebenseite über den Vereinigungstest
   ausgeblendet). Nötig erst, wenn Wände unterschiedliche *Füllfarben* bekommen
   — der Haarstrich an der Naht ist durch die überlappenden Füllflächen
   (`FILL_BLEED`) erledigt. Zwei verschiedene Füllfarben an einer Naht kann
   diese Überlappung aber nicht sauber trennen.
4. **Stumpf statt Gehrung bei entgegengesetzter Ausrichtung** — liegen beide
   Wände auf entgegengesetzten Seiten ihrer Basislinien (links/rechts) und ist
   der Winkel spitz, würde die Naht mehr als eine halbe Wandquerschnittsfläche
   wegnehmen; dort wird bewusst stumpf gestoßen (die Körper berühren sich nur,
   jede Gehrung risse ein Loch). Wer das anders will, braucht eine Entscheidung
   über die dann fehlende Fläche — kein Rechenfehler, eine Modellfrage.
5. Bemaßung: Griffe an Maßketten fehlen.
6. ~~Wandfang-Schalter in der Oberfläche~~ — erledigt: Checkbox „Wandkanten" im
   Einstellungen-Dialog, Ribbon-Gruppe Fangen, Statusmenü Fang (inkl. „Alle an/aus").
