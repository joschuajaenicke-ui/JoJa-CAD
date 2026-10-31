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

3. **Übrige Tasten und Tastenkombinationen — hardcodiert.**
   Die Funktionsweise aller weiteren Tasten (`Esc`, `Tab`, `Caps`, `Umschalt`,
   `Strg`, `Alt`, `AltGr`, `Enter`, `Rücktaste`, `Leertaste`, `Entf`, `F1` …
   `F12`) sowie aller **Tastenkombinationen** (z. B. `Strg+C`, `Strg+S`,
   `Strg+Z`, `Strg+Umschalt+C`) ist **im Programm fest verdrahtet** und vom
   Benutzer **nicht änderbar**. Sie stehen in `App::handle_shortcuts`
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
  Gehalten bewegt die Taste die Ansicht laufend; ein einzelner Aufruf (Hotbar,
  MCP-freier Weg über `App::run_action`) rückt sie um `SCHWENK_SCHRITT_PX` weiter.
- **`S` gehört dem Schwenk, nicht dem Stutzen.** `modify.trim` hat deshalb kein
  Werkkürzel mehr (erreichbar über Ribbon, Inventar, Hotbar, Tastaturbelegung).
  Eine alte `prefs.json` mit `Stutzen = S` wird beim Laden bereinigt.
- **`V` gehört dem Verschieben** (`modify.move`, so steht es in der Bedienung);
  das Volumen liegt auf `Umschalt+V`. Auch das wird beim Laden bereinigt.
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
aus Ribbon, Hotbar oder Tastatur. Beim Schließen wird der Merker geleert. Die
Alternative bleibt die Funktionsliste im Fenster (Taste wählen, Funktion
aus der Liste).

**Mehrere Tasten je Funktion:** Eine Funktion darf auf beliebig vielen Tasten
liegen (`UserPrefs::bindings`: Action → Liste von Tasten). Ein zweiter Klick auf
eine freie Taste **ergänzt** die Belegung und ersetzt sie nicht; die
Funktionsliste zeigt alle Tasten in Klammern. Umgekehrt trägt eine Taste
weiterhin genau eine Funktion — wer eine Funktion auf eine belegte Taste legt,
nimmt sie der anderen weg. Eine Funktion verschieben heißt deshalb: neue Taste
anklicken, alte mit der **mittleren Maustaste** leeren (oder „Belegung dieser
Taste entfernen").

Die Bedienung steht als Tooltip am kleinen **i** über dem Tastenbild — im
Fenster selbst kein langer Hinweistext.

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
| Belegung: Liste je Funktion, alte Form wird gelesen | `persist::tests::funktion_laesst_sich_mehrfach_belegen`, `…::alte_einzelne_belegung_wird_als_liste_gelesen` |
