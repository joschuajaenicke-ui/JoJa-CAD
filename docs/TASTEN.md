# Tastenbelegung — die drei Schichten

Diese Datei ist verbindlich. Jede Taste in JoJa-CAD gehört genau einer der drei
Schichten unten. Wer eine Funktion, ein Kürzel oder eine Taste ergänzt, ordnet
sie zuerst hier ein — nicht „irgendwie" in den Code.

## Die Regel

1. **Ziffern `1` … `0` — Hotbar.**
   Die Belegung der Zifferntasten wird **über die Hotbar** gesteuert: ein Slot
   trägt eine Funktion, die Ziffer startet ihn. Geändert wird das im Inventar
   (Funktion auf den Slot ziehen), im Rechtsklickmenü eines Slots
   (Leeren / Verknüpfen / Bearbeiten) oder durch Anlegen einer Hotbar-Vorlage.
   Die Tastaturbelegung fasst die Ziffern nicht an — im Tastenbild sind sie
   ausgegraut. Die `0` gehört dem **zehnten (letzten)** Slot: `1` … `9` sind die
   Slots eins bis neun (T0065).

2. **Buchstaben und Symbole — Tastaturbelegung.**
   Alle Buchstaben und Symboltasten (`Q`, `W`, `Ü`, `,`, `.`, `-`, `+` …) werden
   **über die Tastaturbelegung** gesteuert (Ribbon ▸ Ansicht ▸ „Tasten" bzw.
   Werkzeugleiste unten links) und sind **dynamisch vom Benutzer anpassbar**.
   Zeichen, die auf einer deutschen Tastatur nur als zweite Ebene einer anderen
   Taste liegen (`#`, `<`, `´` = Umschalt/AltGr), kommen nie als eigener
   Tastendruck an — sie sind im Tastenbild grau und nicht zuweisbar. Gespeichert wird die Belegung in `~/.config/jjcad/prefs.json`
   (`bindings`). Jede Taste trägt **genau eine** Funktion; eine neue Zuweisung
   nimmt der vorherigen Funktion die Taste weg.
   Jeder Buchstabe hat zusätzlich eine **Umschalt-Ebene** (`Umschalt+I`,
   `Umschalt+V`), die ebenfalls frei belegbar ist.
   Ausgewertet wird das **getippte Zeichen**, nicht die physische Taste: `ä`,
   `ö`, `ü` und jedes weitere Sonderzeichen einer fremden Belegung starten ihre
   Funktion genauso wie ein Buchstabe (T0121). `egui::Key` kennt nur die
   US-Lage — die Taste `ä` kommt dort als `Key::Quote` an, das Zeichen nur als
   `Event::Text`; deshalb liest `App::handle_shortcuts` für alles, was nicht in
   `EINZELTASTEN` steht, den Text. Ein ausdrücklich belegtes Sonderzeichen
   schlägt dabei die Taste, über die es getippt wird (`AltGr+E` = `€`, nicht
   `e`). Die Umschalt-Ebene gilt auch hier (`Umschalt+ä`).

3. **Übrige Tasten und Tastenkombinationen — hardcodiert.**
   Die Funktionsweise aller weiteren Tasten (`Esc`, `Tab`, `Caps`, `Umschalt`,
   `Strg`, `Alt`, `AltGr`, `Enter`, `Rücktaste`, `Leertaste`, `Entf`, `F1` …
   `F12`) sowie aller **Tastenkombinationen** (z. B. `Strg+C`, `Strg+S`,
   `Strg+Z`, `Strg+Umschalt+C`, `Umschalt+F3`) ist **im Programm fest verdrahtet**
   und vom Benutzer **nicht änderbar**. Sie stehen in `App::handle_shortcuts`
   (`crates/jjcad-gui/src/app.rs`) und sind im Tastenbild ausgegraut bzw. in der
   Funktionsliste als „fest" gekennzeichnet.

Kurz: **Ziffern = Hotbar, Buchstaben/Symbole = Tastaturbelegung, alles andere =
fest.** Keine Taste gehört zwei Schichten.

## Was daraus folgt (Stand T0056)

- **`W` `A` `S` `D` schwenken die Ansicht** — als Funktionen
  `view.pan_up`, `view.pan_left`, `view.pan_down`, `view.pan_right`
  (Register in `crates/jjcad-core/src/actions.rs`). Sie tragen ein eigenes Icon
  und stehen damit sichtbar im Tastenbild der Tastaturbelegung. Vorher schwenkte
  `canvas::handle_navigation` fest auf `Key::W`/`A`/`S`/`D` — die Taste ließ
  sich nicht umbelegen und war im Tastenbild unsichtbar.
  Sie sind **gewöhnliche Funktionen** (T0089): der Aufruf kommt gleich ob von der
  Taste, aus der Hotbar oder aus dem Ribbon. Was er bewirkt, entscheidet die
  Einstellung **Bedienung ▸ „Verschieben"** (T0101): **Sprunghaft** rückt je Druck
  um `SCHWENK_SCHRITT_PX` weiter, **Kontinuierlich** (Vorgabe) hält die Ansicht in
  Bewegung, solange die Funktion gedrückt bleibt (`App::schwenk_fortschritt`,
  `SCHWENK_TEMPO_PX_PRO_S` mal Bilddauer). Ein **Schritt neben** dem Dauerlauf ist
  ein Fehler — er zählte die Tastenwiederholung des Betriebssystems doppelt und
  machte die Ansicht schneller als das eingestellte Tempo. Mit `Strg` schweigt der
  Dauerlauf (sonst schöbe `Strg+S` die Ansicht). Nimmt ein anderes Fenster den
  Fokus — der Systemdialog bei `Strg+S` und `Strg+O` —, gilt **keine** Taste mehr
  als gedrückt: das Betriebssystem liefert das Loslassen nicht nach, sonst liefe
  der Dauerlauf endlos weiter (T0140).
- **`S` gehört dem Schwenk, nicht dem Stutzen** — das Stutzen liegt seit T0079
  auf `C`. Eine alte `prefs.json` mit `Stutzen = S` wird beim Laden bereinigt.
- **`C` = Stutzen, `T` = Text, `Z` = Zerlegen** (T0079). Der Farbeimer lag
  vorher auf `C` und weicht deshalb auf `Umschalt+C`; seine alte Belegung wird
  beim Laden umgeschrieben. `T` lag zwischenzeitlich beim Chat-Protokoll — das
  sitzt seit T0009 auf `^` (Taste unter ESC).
- **`V` gehört dem Verschieben** (`modify.move`, so steht es in der Bedienung);
  das Volumen liegt auf `Umschalt+V`. Auch das wird beim Laden bereinigt.
- **`Umschalt+F3` schaltet die Leistungsanzeige** (T0068): Abstand und Rechenzeit
  je Bild, Perzentile und die Aufteilung der Rechenzeit in Abschnitte. F3 allein
  ist nicht belegt. Die Kombination ist fest verdrahtet, weil die Messwerte auch
  bei laufendem Werkzeug erreichbar sein müssen.
- **Drehen und Spiegeln der Einfüge-Vorschau** hängen an den Funktionen
  **„Drehen"** (`modify.rotate`, Vorgabe `R`) und **„Spiegeln"**
  (`modify.mirror`, Vorgabe `F`) — nicht an festen Tasten. Wer „Drehen" auf `Q`
  legt, dreht die Vorschau mit `Q` und die Auswahl mit `Q`. `Umschalt` dreht
  gegen den Uhrzeigersinn.

## Funktionen direkt auf eine Taste legen (T0064)

Der Weg wie bei der Hotbar: **Funktion im Ribbon anklicken, dann im Fenster
„Tastaturbelegung" eine leere Taste anklicken** — die Taste übernimmt sie sofort.
Das Fenster dunkelt den Hintergrund dafür nicht mehr ab; das Ribbon bleibt
bedienbar. Eine bereits belegte Taste wird dabei nur ausgewählt (ihre Funktion
bleibt), fest verdrahtete Funktionen (`Strg+S`, `F1` …) lassen sich so nicht
umbelegen.

Welche Funktion bereitliegt, merkt sich das Programm, solange das Fenster offen
ist (`UiState::keymap_funktion`): jede über `App::run_action` gestartete Funktion
aus Ribbon, Hotbar oder Tastatur. Beim Schließen wird der Merker geleert.

**Eine Funktion auf eine Taste legen (T0072):** Eine freie Taste anklicken —
das Inventar öffnet sich im Reiter **Funktionen**, die gewählte Funktion liegt
danach auf der Taste. Dieselbe Bedienung wie ein leerer Hotbar-Slot; auch eine
belegte Taste lässt sich so umbelegen. Eine eigene Funktionsliste im Fenster
gibt es nicht — und seit T0076 auch keine Knopfzeile darunter; fest verdrahtete
Funktionen stehen im Inventar ausgegraut mit „· fest".

**Mehrere Tasten je Funktion:** Eine Funktion darf auf beliebig vielen Tasten
liegen (`UserPrefs::bindings`: Action → Liste von Tasten). Ein zweiter Klick auf
eine freie Taste **ergänzt** die Belegung und ersetzt sie nicht; die
Funktionsliste zeigt alle Tasten in Klammern. Umgekehrt trägt eine Taste
weiterhin genau eine Funktion — wer eine Funktion auf eine belegte Taste legt,
nimmt sie der anderen weg. Eine Funktion verschieben heißt deshalb: neue Taste
anklicken, alte mit der **mittleren Maustaste** leeren.

Die Bedienung steht als Tooltip am **i** in der oberen Leiste des Fensters —
linksbündig, im Fenster selbst kein langer Hinweistext. Das Fenster ist genau so
breit wie das Tastenbild (716 px); unter dem Tastenbild steht nur die
Layoutzeile.

## Layouts speichern, wechseln, aus- und einlesen (T0076)

Unter dem Tastenbild steht die Layoutzeile: **Layout** (Auswahl), **Speichern …**,
**Importieren …**, **Exportieren …**, **Löschen**. Sie ersetzt den früheren
Knopf „Werkseinstellungen".

- Die **ersten vier Layouts sind eingebaute Vorlagen** (T0079): *Vorlage
  JoJa-CAD*, *Vorlage Allplan*, *Vorlage ArchiCAD*, *Vorlage AutoCAD*. Sie
  tragen immer die unten aufgeführten Belegungen, lassen sich nicht
  überschreiben (die Namen sind belegt) und nicht löschen — sie sind der
  Rückweg auf eine bekannte Belegung. Eigene Layouts stehen dahinter.
  Der frühere Name *Werkseinstellung* bleibt ebenfalls gesperrt und wird beim
  Laden durch *Vorlage JoJa-CAD* ersetzt.
- **Speichern …** legt die aktuelle Belegung unter einem Namen ab; ein
  vorhandener Name wird überschrieben. Ein Layout ist eine Momentaufnahme: wer
  danach eine Taste ändert, hat „Eigene Belegung", bis er wieder ein Layout
  wählt.
- **Wechseln** über die Auswahl: die Belegung des gewählten Layouts ersetzt die
  aktuelle sofort. Die Auswahl zeigt, welches Layout **ausdrücklich** gilt
  (`UserPrefs::keymap_aktiv`) — auch wenn zwei Layouts dieselben Tasten tragen.
  Nach dem Speichern steht dort der gespeicherte Name, nach einer Änderung von
  Hand „Eigene Belegung". Ohne gemerkten Eintrag (alte `prefs.json`) gilt das
  Layout, dessen Belegung der aktuellen entspricht.
- Das **Auswahlmenü wächst mit der Zahl der Layouts** (bis rund 15 Zeilen,
  `panels::DROPDOWN_MAX_H`); reicht der Platz unter dem Feld nicht, öffnet es
  nach oben. Darüber wird gerollt.
- **Wechsel aus einer ungespeicherten Belegung** fragt nach: *Abbrechen*
  (nichts ändern), *Verwerfen* (wechseln, Änderung fällt weg) oder *Speichern*
  (Belegung sichern, danach wechseln — die Namensabfrage merkt sich das Ziel).
  Ungespeichert heißt: die Belegung steht in **keinem** Layout
  (`UserPrefs::keymap_ungespeichert`).
- **Löschen** entfernt das in der Auswahl stehende gespeicherte Layout; die
  Tasten bleiben gültig, danach gilt „Eigene Belegung". Die eingebauten
  Vorlagen lassen sich nicht löschen.
- **Exportieren …** schreibt die **aktuelle** Belegung als `.json`
  (`{"name": …, "bindings": {aktion: [tasten]}}`), **Importieren …** liest sie
  wieder ein und wendet sie sofort an. Ein Import mit nackter Belegung
  (`{"draw.line": ["l"]}`) ist erlaubt, der Name kommt dann vom Dateinamen.
  Gespeichert wird in `prefs.json` unter `keymap_presets`.

## Die vier Vorlagen (T0079)

Ausgangspunkt jeder Vorlage ist die Belegung von JoJa-CAD; die fremden Vorlagen
tragen nur die Tasten, in denen sich Allplan, ArchiCAD und AutoCAD unterscheiden
(`persist::VORLAGE_*`, Quellen in den Kommentaren dort). Tastenkombinationen
(`Strg+L` …) sind in JoJa-CAD fest verdrahtet — wo ein fremdes Programm eine
Kombination vorgibt, nimmt die Vorlage den Buchstaben dahinter bzw. die
Umschalt-Ebene.

| Funktion | JoJa-CAD | Allplan | ArchiCAD | AutoCAD |
|---|---|---|---|---|
| Linie | `L` | `L` | `Umschalt+L` | `L` |
| Polylinie | `O` | `O` | `L` | `P` |
| Kreis | `K` | `K` | `C` | `C` |
| Bogen | – | – | – | `A` |
| Text | `T` | `Umschalt+T` | `K` | `T` |
| Maß | `M` | `M` | `J` | `D` |
| Wand | `G` | `W` | `W` | `G` |
| Tür | `H` | `T` | `M` | `Umschalt+H` |
| Fenster | `J` | `F` | `,` | `J` |
| Treppe | `B` | `B` | `.` | – (Block `B`) |
| Farbeimer | `Umschalt+C` | `Umschalt+C` | `H` | `H` |
| Verschieben | `V` | `V` | `D` | `M` |
| Kopieren (Basispunkt) | `Strg+Umschalt+C` | `Strg+Umschalt+C` | `Umschalt+C` | `Umschalt+C` |
| Drehen | `R` | `R` | `Umschalt+E` | `R` |
| Spiegeln | `F` | `Umschalt+F` | `Umschalt+M` | `Umschalt+M` |
| Stutzen | `C` | `C` | – | `Umschalt+T` |
| Verlängern | `X` | `X` | `X` | `Umschalt+E` |
| Versetzen | – | – | `O` | `O` |
| Zerlegen | `Z` | `Z` | `Z` | `X` |
| Skalieren | – | – | – | `S` |
| Zoom | – | – | `+` / `-` | `Z` (Alles zeigen) |
| Ansichtsschwenk | `W` `A` `S` `D` | `Umschalt+…` | `Umschalt+…` | `Umschalt+…` |

Quellen: Allplan-Kurzanleitung „Shortcuts" (deutsch: Linie `Strg+L`, Maßlinie
`Strg+M`, Wand `Strg+W`, Decke `Strg+D`, Tür `Strg+T`, Fenster `Strg+F`);
„ARCHICAD Default Keyboard Shortcuts" von Graphisoft (Wand `W`, Tür `M`,
Fenster `,`, Polylinie `L`, Linie `Umschalt+L`, Text `K`, Maß `J`, Versetzen
`O`); AutoCAD-Befehlskürzel (`LINE`=L, `PLINE`=P, `ARC`=A, `CIRCLE`=C,
`MOVE`=M, `OFFSET`=O, `SCALE`=S, `EXPLODE`=X, `HATCH`=H, `DIM`=D, `ZOOM`=Z).
Zeichen, die JoJa-CAD nicht auswertet (`;`, `=`, `’`), sind bewusst nicht
übernommen.

## Für Entwickler

- **Neue Funktion mit Kürzel:** Action im Register (`actions.rs`) mit `key`
  anlegen. Liegt der Schlüssel in Schicht 2 (ein Buchstabe/Symbol), erscheint er
  automatisch in der Tastaturbelegung und wird ausgewertet — vorausgesetzt, die
  Taste steht in `app::EINZELTASTEN`. **Diese Liste ist die eine Quelle** für
  Kürzel-Auswertung und Ansichtsschwenk; wer sie erweitert, prüft
  `persist::tests::jede_taste_traegt_genau_eine_funktion`.
- **Neue Taste in Schicht 3** (Steuerungstaste, Funktionstaste, Kombination):
  Auswertung in `App::handle_shortcuts` ergänzen **und** die Taste in
  `persist::fest_belegt` eintragen. Dann ist sie gesperrt, ausgegraut und in der
  Funktionsliste als „fest" markiert. Eine Funktion mit solcher Taste lässt sich
  nicht umbelegen (`UserPrefs::belegung_aenderbar`).
- **Kein zweiter Weg:** keine Taste zusätzlich im Code abfragen
  (`ctx.input(|i| i.key_pressed(...))`), wenn sie schon über die Belegung läuft.
  Genau daraus entstand das Durcheinander: `S` schwenkte *und* stutzte, `V`
  startete Volumen statt Verschieben.
- **Symbole:** `+` allein ist ein Zeichen (frei belegbar); erst `ctrl+…`/`alt+…`
  gilt als Kombination. `Umschalt+<Zeichen>` ist die zweite Ebene des
  Buchstabens und bleibt frei belegbar.
- Belegung zurücksetzen: Einstellungen → Bedienung → „Tastenbelegung
  zurücksetzen" (`UserPrefs::reset_bindings`).

## Prüfungen

| Was | Test |
|---|---|
| Drei Schichten, gesperrte Tasten | `persist::tests::drei_schichten_der_tastatur` |
| Schwenk liegt auf W/A/S/D, Stutzen nicht auf S | `persist::tests::ansicht_schwenk_ist_belegbar` |
| Eine Taste = eine Funktion | `persist::tests::jede_taste_traegt_genau_eine_funktion` |
| Kombinationen bleiben fest | `persist::tests::feste_kombinationen_lassen_sich_nicht_umbelegen` |
| Schwenk folgt der Belegung (Verhalten) | `app::tests_tastenbelegung::*` |
| Icons der Schwenkrichtungen | `icons::tests_pan_icons` |
| Leere Taste übernimmt die Ribbon-Funktion | `dialogs::tests_tastatur::leere_taste_uebernimmt_die_funktion_aus_dem_ribbon` |
| Belegte Taste bleibt belegt | `dialogs::tests_tastatur::belegte_taste_behaelt_ihre_funktion` |
| Fest verdrahtete Funktion landet nicht auf einer Taste | `dialogs::tests_tastatur::fest_verdrahtete_funktion_landet_nicht_auf_einer_taste` |
| Kein Abdunkeln beim offenen Fenster | `dialogs::tests_tastatur::tastaturbelegung_dunkelt_nicht_ab` |
| Mittlere Maustaste leert eine Taste | `dialogs::tests_tastatur::mittlere_maustaste_entfernt_die_belegung` |
| Funktion liegt auf mehreren Tasten | `dialogs::tests_tastatur::funktion_laesst_sich_mehrfach_platzieren` |
| Bedienung nur als Tooltip am i | `dialogs::tests_tastatur::bedienung_steht_als_tooltip_am_info_symbol` |
| Werkseinstellung ist das erste Layout | `persist::tests::werkseinstellung_ist_das_erste_layout` |
| Vier Vorlagen stehen vorn | `persist::tests::vier_vorlagen_stehen_vorn` |
| Werkstasten `C`/`T`/`Z` | `persist::tests::vorlage_jjcad_belegt_c_t_z` |
| Eine Taste = eine Funktion in jeder Vorlage | `persist::tests::jede_vorlage_belegt_jede_taste_genau_einmal` |
| Vorlagen belegen nur erreichbare Tasten | `persist::tests::vorlagen_belegen_nur_erreichbare_tasten` |
| Fremde Vorgaben (Stichproben) | `persist::tests::vorlagen_folgen_den_vorgaben_der_fremden_programme` |
| Alte Belegung bekommt `C`/`T`/`Z` nach | `persist::tests::alte_belegung_uebernimmt_die_neuen_werkstasten` |
| Eigene Layouts bleiben hinter den Vorlagen | `persist::tests::eigene_layouts_bleiben_hinter_den_vorlagen` |
| Menü zeigt die vier Vorlagen | `dialogs::tests_tastatur::layoutmenue_zeigt_die_vier_vorlagen` |
| Eingebaute Vorlage nicht löschbar | `dialogs::tests_tastatur::eingebaute_vorlage_laesst_sich_nicht_loeschen` |
| Speichern/Anwenden/Wechseln | `persist::tests::layout_wird_gespeichert_und_wieder_angewendet` |
| Export/Import (auch nackte Belegung) | `persist::tests::layout_wird_exportiert_und_importiert`, `…::import_nimmt_auch_eine_nackte_belegung` |
| Fenster: keine Funktionsauswahl, Layoutzeile da | `dialogs::tests_tastatur::tastaturfenster_hat_keine_funktionsauswahl_mehr` |
| Auswahlfeld und Knöpfe gleich hoch | `dialogs::tests_tastatur::layoutzeile_hat_fuer_alle_dieselbe_hoehe` |
| Auswahlmenü wächst mit der Zahl der Layouts | `dialogs::tests_tastatur::layoutmenue_waechst_mit_der_zahl_der_layouts` |
| Rückfrage: Abbrechen/Verwerfen | `dialogs::tests_tastatur::rueckfrage_abbrechen_und_verwerfen` |
| Rückfrage: Speichern sichert und wechselt | `dialogs::tests_tastatur::rueckfrage_speichern_sichert_und_wechselt` |
| Ungespeichert erkannt | `persist::tests::ungespeicherte_belegung_wird_erkannt` |
| Gespeicherter Name steht in der Auswahl | `persist::tests::gespeichertes_layout_steht_auch_bei_gleicher_belegung_in_der_auswahl` |
| Löschen über die Layoutzeile | `dialogs::tests_tastatur::loeschen_entfernt_das_gewaehlte_layout` |
| Breite = Tastenbild, i links oben | `dialogs::tests_tastatur::fenster_ist_so_breit_wie_das_tastenbild`, `…::infosymbol_steht_links_in_der_oberen_leiste` |
| Belegung: Liste je Funktion, alte Form wird gelesen | `persist::tests::funktion_laesst_sich_mehrfach_belegen`, `…::alte_einzelne_belegung_wird_als_liste_gelesen` |
