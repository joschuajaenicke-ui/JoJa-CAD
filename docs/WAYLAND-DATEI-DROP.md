# Befund: Datei-Drop unter Wayland (T0115, geprüft in T0123)

Stand: 6. November 2026 · Umgebung: KDE Plasma 6.3.6, Wayland-Sitzung,
eframe/egui/egui-winit **0.29.1**, winit **0.30.13**, arboard 3.7.

## Ergebnis kurz

| Frage | Antwort |
| --- | --- |
| Funktioniert „Datei ins Fenster ziehen“ unter Wayland? | **Nein** — und zwar unabhängig vom Programmcode |
| Liegt der Fehler in JoJa-CAD? | **Nein.** Die Fensterbibliothek unter egui (winit 0.30) hat unter Wayland keinen Datei-Drop |
| Hilft ein egui-Upgrade? | **Nein.** Auch eframe 0.37 (Oktober 2026) hängt an `winit ^0.30.13` |
| Warum war die Abnahme in T0115 grün? | Geprüft wurde headless unter **Xvfb**, also im X11-Zweig von winit — dort kommen Drops an |
| Was funktioniert? | Fenster über **XWayland** (X11-Zweig erzwingen) — verworfen, siehe §4 |
| Wann geht es wirklich? | Sobald egui/eframe auf **winit 0.31** steht (dort gibt es Wayland-Drops) |

## 1. Symptom

Eine Datei aus dem Dateimanager (Dolphin) auf das JoJa-CAD-Fenster ziehen: es
passiert nichts. Kein Hinweis, kein Import, keine Fehlermeldung. Der Code aus
T0115 (`dateien_ablegen`, `schwebende_dateien`, `dialogs::dateien_fallen_gelassen`)
läuft dabei fehlerfrei — er bekommt nur nie eine Datei zu sehen.

## 2. Ursache

egui liest abgelegte Dateien aus `RawInput::dropped_files`. Gefüllt wird das
Feld ausschließlich von `egui-winit` aus den Fensterereignissen
`WindowEvent::HoveredFile` / `DroppedFile`:

```
egui-winit-0.29.1/src/lib.rs:419   WindowEvent::HoveredFile(path)      -> raw.hovered_files
egui-winit-0.29.1/src/lib.rs:429   WindowEvent::HoveredFileCancelled   -> raw.hovered_files.clear()
egui-winit-0.29.1/src/lib.rs:436   WindowEvent::DroppedFile(path)      -> raw.dropped_files
```

Diese Ereignisse erzeugt **winit** — und zwar nur im X11-Zweig:

```
winit-0.30.13/src/platform_impl/linux/x11/dnd.rs        XDND-Protokoll (XdndEnter/Position/Drop)
winit-0.30.13/src/platform_impl/linux/x11/event_processor.rs:454-574   Ereignisse daraus
winit-0.30.13/src/platform_impl/linux/wayland/          kein wl_data_device, kein DnD
```

Im Wayland-Zweig gibt es **keine** Bindung an `wl_data_device_manager` und
damit keine `wl_data_offer`-Verarbeitung; `text/uri-list` wird nie gelesen.
Ein `grep` nach `data_device`, `data_offer`, `Dnd` über
`platform_impl/linux/wayland/` liefert nichts. Das ist kein Konfigurations-
oder Einstellungsproblem, sondern fehlender Code in der Bibliothek.

Was **nicht** die Ursache ist (alles geprüft und ausgeschlossen):

- `with_drag_and_drop(true)` — ist gesetzt (`main.rs`) und wird von eframe
  durchgereicht (`egui-winit-0.29.1/src/lib.rs:1723`).
- Die Auswahl des Backends — winit nimmt Wayland, sobald `WAYLAND_DISPLAY`
  gesetzt ist (`winit-0.30.13/src/platform_impl/linux/mod.rs:737`); ein
  `WINIT_UNIX_BACKEND` gibt es in 0.30 nicht mehr.
- Der Programmcode aus T0115 — er ist durch Tests abgedeckt und reagiert
  korrekt auf `dropped_files`; das Feld bleibt nur leer.

Upstream-Stand:

- [winit#1881 „Support drag and drop on wayland“](https://github.com/rust-windowing/winit/issues/1881)
  — am 3. August 2026 als erledigt geschlossen, gelöst **in winit 0.31**
  (neue Ereignisse `DragEntered`/`DragMoved`/`DragDropped`/`DragLeft` plus
  asynchrones `data_transfer`-API).
- [egui#1563](https://github.com/emilk/egui/issues/1563),
  [egui#4105](https://github.com/emilk/egui/issues/4105) — dieselbe Ursache,
  seit 2022 offen.
- Der egui-Port auf winit 0.31 läuft: [emilk/egui#7731](https://github.com/emilk/egui/pull/7731),
  [emilk/egui#8767](https://github.com/emilk/egui/pull/8767). Ein
  Versionswechsel von winit allein genügt nicht — der Adapter muss die
  Übertragung asynchron annehmen und die URI-Liste lesen.

## 3. Warum die Abnahme in T0115 grün war

Der Worker hat am ausgelieferten Build headless geprüft: `Xvfb`, also ein
**X11**-Server, `WAYLAND_DISPLAY` entfernt. Dort läuft winit im X11-Zweig,
`xprop XdndAware` meldet 5, und ein Drop kommt an. Genau dieser Zweig ist in
der Wayland-Sitzung des Nutzers nicht aktiv.

**Merksatz:** `XdndAware` am Fenster beweist nur, dass der X11-Zweig läuft —
nicht, dass Datei-Drop in der Sitzung des Nutzers funktioniert. Die
Plattformschicht muss in der echten Sitzung geprüft werden
(`XDG_SESSION_TYPE`, und ob der Prozess `libwayland-client` geladen hat).

## 4. Was funktioniert hat — und warum es verworfen wurde

Der X11-Zweig lässt sich erzwingen: `eframe::NativeOptions::event_loop_builder`
(`eframe-0.29.1/src/epi.rs:322`) plus
`winit::platform::x11::EventLoopBuilderExtX11::with_x11()`
(`winit-0.30.13/src/platform/x11.rs:114`). Unter Wayland läuft das Fenster dann
über **XWayland**, und KWin übersetzt die Drops der Wayland-Programme in XDND.

Damit war der Drop nachweisbar funktionsfähig (synthetischer XDND-Zug, §6:
Datei geöffnet, Reiter und Statuszeile korrekt). **Trotzdem verworfen**, weil
das Programm am laufenden Fenster spürbar träger wirkte und weniger scharf
aussah — Urteil des Nutzers, und das ist hier maßgeblich. Die Messung
`pixels_per_point` (1,198 unter XWayland statt 1,0 unter Wayland) war **kein**
Beleg für Bildqualität; sie sagt nur, in welcher Auflösung gezeichnet wird.

Die beiden Commits liegen als Patch außerhalb des Repos:
`~/JoJa-CAD/_lose-dateien/0001-T0123-…patch` und `0002-T0123-…patch`.
Nicht wieder einbauen, ohne §5 zu lesen.

## 5. Bedingungen, unter denen Datei-Drop funktioniert

| Weg | Stand | Aufwand | Bewertung |
| --- | --- | --- | --- |
| **egui/eframe auf winit 0.31** | winit 0.31.0-beta.4 (24.10.2026), egui-Port offen | warten; danach egui-Upgrade (0.29 → 0.3x, großer Sprung) | **der vorgesehene Weg** |
| winit 0.30 lokal patchen/vendorn | Fremdberichte: ~300 Zeilen `DataDeviceHandler` auf `smithay-client-toolkit` (ist bei winit schon dabei), läuft auf niri und KDE | eigener winit-Fork, muss gepflegt werden | nur wenn es früher sein muss |
| Fenster über XWayland | funktioniert | klein | **verworfen** (träger, weicher) |
| Flatpak-Portal (`application/vnd.portal.filetransfer`) | nicht relevant, kein Flatpak | — | nur bei Flatpak nötig |

Beim Umstieg auf winit 0.31 ist zu prüfen, ob der egui-Adapter
`raw.dropped_files` weiterhin füllt. Wenn ja, ist am JoJa-CAD-Code **nichts**
zu ändern: `dateien_ablegen` und `schwebende_dateien` bleiben, wie sie sind.
Wenn nein, muss der Adapter die Übertragung annehmen und die URI-Liste lesen —
das gehört dann in egui, nicht in die Anwendung.

## 6. Wie man es ohne Maus prüft

`scripts/xdnd-drop.py` baut einen XDND-Zug als Quelle nach: `XdndEnter` →
`XdndPosition` → `SelectionRequest` mit `text/uri-list` bedienen →
`XdndDrop`. Damit lässt sich ein echter Datei-Drop headless auslösen.

```bash
# 1. eigener X-Server, damit nichts auf dem Desktop passiert
Xvfb :99 -screen 0 1600x1000x24 &
# 2. Programm so starten, dass es nur mit erzwungenem X11-Zweig läuft:
#    WAYLAND_DISPLAY zeigt auf einen nicht vorhandenen Socket
WAYLAND_DISPLAY=wayland-99 DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1 ./target/debug/jjcad &
# 3. Datei „ziehen“
DISPLAY=:99 python3 scripts/xdnd-drop.py "JoJa-CAD" samples/Bogenprobe.jjcad
# 4. Fensterinhalt prüfen (Reiter + Statuszeile)
DISPLAY=:99 import -window "$(DISPLAY=:99 xwininfo -root -children | grep JoJa-CAD | awk '{print $1}')" /tmp/drop.png
```

Voraussetzung: `python-xlib` (`pip install python-xlib`).

Was ein grüner Lauf beweist: der X11-Zweig nimmt Drops an und der
Importweg im Programm funktioniert. Was er **nicht** beweist: dass es in einer
Wayland-Sitzung ohne erzwungenes X11 funktioniert — dafür gibt es bis
winit 0.31 keinen Ersatz für den echten Zug mit der Maus.

## 7. Stand im Code

- Der T0115-Code bleibt vollständig drin (`dateien_ablegen`,
  `schwebende_dateien`, `dialogs::dateien_fallen_gelassen`, Tests in
  `app::tests_dragdrop`). Er schadet nicht und wird mit winit 0.31 von selbst
  lebendig.
- Es gibt **keine** Fenster-Backend-Wahl im Programm (bewusst): das Fenster
  läuft nativ unter Wayland.
- `docs/BEDIENUNG.md` Abschnitt 17 beschreibt den Drop; dort steht der
  Wayland-Hinweis.

## 8. Offen

1. egui-Release mit winit 0.31 beobachten (emilk/egui#7731, #8767). Danach
   egui-Upgrade als eigenes Ticket — der Sprung 0.29 → 0.3x ist groß
   (Widget-IDs, `Event::Copy/Cut/Paste`, eigene Menü-Area).
2. Nach dem Upgrade: echter Zug mit der Maus in der Wayland-Sitzung, plus
   `scripts/xdnd-drop.py` als Gegenprobe im X11-Zweig.
3. Bis dahin gilt: Datei-Drop unter Wayland ist **bekannt unmöglich**, kein
   Fehler im Programm. Nicht erneut als Ticket „funktioniert nicht“ aufnehmen,
   ohne diesen Befund zu lesen.
