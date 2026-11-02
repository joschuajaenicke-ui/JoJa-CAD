JoJa-CAD — Windows-Fassung (portabel)
=====================================

Start: jjcad.exe per Doppelklick. Keine Installation, keine Registry, keine
Administratorrechte. Der Ordner darf beliebig verschoben oder auf einen
USB-Stick kopiert werden. Voraussetzung ist ein OpenGL-2.0-Treiber, wie ihn
jeder Windows-Rechner mit Grafikkartentreiber mitbringt.

Inhalt
  jjcad.exe ........ Zeichenprogramm (Oberfläche deutsch)
  jjcad-mcp.exe .... Headless-Server über stdin/stdout (Model Context Protocol),
                     Selbsttest:  jjcad-mcp.exe --selftest
  fonts\ ........... Liberation Sans/Serif/Mono/Narrow und Quicksand, samt
                     Lizenztexten. Windows hat diese Schriften nicht; der Kern
                     sucht sie zuerst in "fonts\" neben dem Programm, danach im
                     System.
  beispiele\ ....... Wohnhaus.jjcad, Textfelder.jjcad
  LIZENZ-JoJa-CAD.txt

Wichtig
  * Der Ordner "fonts" muss neben jjcad.exe liegen bleiben. Ohne ihn fehlen
    die Beschriftungen auf der Zeichenfläche und im PDF-Export.
  * Zeichnungsdateien (.jjcad) sind reines JSON und zwischen Linux und Windows
    ohne Umweg austauschbar. Die technischen Schriften (DIN 1451, EN ISO 3098)
    sind im Programm eingebettet und brauchen die Systemschriften nicht.

Anzeigefehler melden
--------------------
Tritt ein Fehlerbild auf (helle Fugen, falsche Flächen), **F12 drücken**. Das
Programm legt dann neben jjcad.exe zwei Dateien ab:

  jjcad-diag-<zeit>.png   den Fensterinhalt genau so, wie ihn das Programm
                          selbst gezeichnet hat (1 Bildpunkt = 1 Bildpunkt)
  jjcad-diag-<zeit>.txt   Plattform, Windows-Version, Skalierung in Prozent,
                          Grafiktreiber (Hersteller/Renderer/Version) und die
                          Bildspalten, in denen eine durchgehende helle Linie
                          steckt

Damit steht in einem Schritt fest, ob der Fehler im gezeichneten Bild steckt
(dann steht er im PNG) oder erst beim Zusammensetzen des Fensters durch Windows
entsteht (dann fehlt er im PNG). Beide Dateien bitte an das Ticket hängen.

Notausgänge ohne neuen Build (in der Eingabeaufforderung setzen, dann starten)
  set JJCAD_UI_FEATHER=0   Flächen ohne weiche Kante zeichnen — hilft, wenn ein
                           Treiber halbtransparente Kanten hell darstellt.
  set JJCAD_NO_VSYNC=1     ohne Bildsynchronisation zeichnen — hilft bei
                           Streifen, die vom Zusammensetzen des Fensters kommen.
  set JJCAD_UI_DIAG=1      Diagnosebild automatisch kurz nach dem Start ablegen
                           (statt F12).

Änderungen gegenüber der ersten Testfassung
  * Deckender Fenstergrund in der Oberflächenfarbe statt des halbtransparenten
    Standardgraus: bleibt zwischen zwei Flächen eine Fuge, ist sie unsichtbar.
  * Panelhöhen und -breiten werden auf ganze Bildpunkte gerundet; bei 125/150/175 %
    Skalierung entstehen dadurch keine halben Pixelkanten mehr.
  * F12-Selbstdiagnose und die drei Schalter oben sind neu.

Stand dieser Testfassung
  Gebaut auf Linux mit der GNU-Toolchain (Ziel x86_64-pc-windows-gnu), also
  nicht auf Windows kompiliert. Geprüft unter Wine 10 bei 100 %, 125 % und
  150 % Skalierung: Start, Zeichnen einer Wand, PDF-/PNG-/DXF-Export,
  Wiederladen und die Selbstdiagnose (F12).

  Noch nicht geprüft: nativer Start unter echtem Windows, der Windows-
  Dateidialog (Öffnen/Speichern) und HiDPI an einem echten Bildschirm.