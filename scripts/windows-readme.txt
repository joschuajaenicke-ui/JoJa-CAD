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

Stand dieser Testfassung
  Gebaut auf Linux mit der GNU-Toolchain (Ziel x86_64-pc-windows-gnu), also
  nicht auf Windows kompiliert. Geprüft ist die Fassung unter Wine 10:
  Start, Zeichnen einer Wand, PDF-/PNG-/DXF-Export und Wiederladen.

  Noch nicht geprüft: nativer Start unter echtem Windows, der Windows-
  Dateidialog (Öffnen/Speichern) und HiDPI-Skalierung an einem echten
  Bildschirm. Rückmeldungen dazu gern als Ticket.
