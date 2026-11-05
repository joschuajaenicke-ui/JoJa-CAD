# ToDo 0.3 — vollständige Liste

Jede Zeile ist ein Ticket. `P0` = ohne das wird nicht veröffentlicht, `P1` = vor
dem Zeigen an Freunde, `P2` = danach. Aufwand in Commits.

**Reihenfolge ist fix, das Datum nicht.** Sonntag ist ein Ziel, kein Termin:
wenn P0 nicht fertig ist, verschiebt sich die Veröffentlichung, nicht die Liste.

Stand: 2026-11-05 · 73 Punkte · P0: 45 · P1: 20 · P2: 8 · zusammen 124 Commits

---

## A — Datensicherung (P0, 9)

- [ ] **A1** Autosave verdrahten: alle 60 s bei Änderung, Pfad aus `persist::autosave_path()` (3)
- [ ] **A2** Autosave nach erfolgreichem Speichern und beim sauberen Beenden entfernen (1)
- [ ] **A3** Atomar speichern: in temporäre Datei schreiben, `fsync`, dann umbenennen (2)
- [ ] **A4** `.bak` der Vorgängerversion beim Überschreiben anlegen (1)
- [ ] **A5** Panic-Hook: Meldung mit Pfad zur Sicherung statt wortlosem Verschwinden (2)
- [ ] **A6** Nach Absturz beim Start Wiederherstellung anbieten (Autosave gefunden) (1)
- [ ] **A7** Warnung bei ungespeicherten Änderungen: Schließen, Neue Datei, Öffnen (1)
- [ ] **A8** Test: Absturz mitten im Zeichnen → Autosave da, wiederherstellbar (1)
- [ ] **A9** Test: Speichern auf volle oder gesperrte Platte → Meldung, Originaldatei unversehrt (1)

## B — Grundelemente und Interaktion (P0, 8)

- [ ] **B1** Bug Rotation — Reproduktion vom Nutzer, dann beheben (2)
- [ ] **B2** Bug 2-Klick-Auswahl — Reproduktion vom Nutzer, dann beheben (2)
- [ ] **B3** Bestandsaufnahme: jedes Grundelement durchklicken (Punkt, Linie, Polylinie, Rechteck, Kreis, Bogen, Ellipse, Schraffur, Text), Lückenliste (1)
- [ ] **B4** Lücken aus B3 schließen (4)
- [ ] **B5** Griffe für alle Objekttypen prüfen: Ziehen, Kanten, ESC (1)
- [ ] **B6** Auswahl prüfen: Fenster, Querfenster, Umschalt-Klick, Alles auswählen (1)
- [ ] **B7** Undo/Redo über alle Aktionen — Liste der Aktionen ohne Undo, dann schließen (2)
- [ ] **B8** Tastatureingabe prüfen: Länge, `x;y`, Komma/Semikolon, ESC/Enter (1)

## C — Bemaßung (P0, 6)

- [ ] **C1** Die drei DIN-Fehler benennen (Nutzer, je ein Satz) (0)
- [ ] **C2** Fehler 1 beheben (1)
- [ ] **C3** Fehler 2 beheben (1)
- [ ] **C4** Fehler 3 beheben (1)
- [ ] **C5** Griffe an Maßketten: nachziehen, Endpunkte verschieben (2)
- [ ] **C6** DIN-Abnahme durch den Nutzer: Screenshot mit Maßkette, Freigabe (0)

## D — Blöcke (P0, 5)

- [ ] **D1** Bestandsaufnahme Blockkonzept: Bibliothek, Einfügen, Instanz, Zerlegen, ByBlock/ByLayer, Drehen/Spiegeln, Hotbar, Inventar (1)
- [ ] **D2** Lücken aus D1 schließen (4)
- [ ] **D3** Blockbibliothek inhaltlich überarbeiten: Ordnung, fehlende Symbole (2)
- [ ] **D4** Blockbibliothek übersetzen (EN) (2)
- [ ] **D5** Test: Block einfügen → speichern → laden → identisch (1)

## E — Import und Export (P0, 7)

- [ ] **E1** PDF-Export: Rundlauf mit drei Zeichnungen, Maße und Linienstärken prüfen (2)
- [ ] **E2** DXF-Export R12: in einem Fremdprogramm öffnen (LibreCAD/QCAD) (1)
- [ ] **E3** DXF-Import: Rundlauf Export → Import → Vergleich (1)
- [ ] **E4** PDF-Import: Grundriss, Text, Linien, Maßstab (2)
- [ ] **E5** PNG-Export: Auflösung, Maßstab, Papier/Modell (1)
- [ ] **E6** Fehlerliste aus E1–E5 abarbeiten (3)
- [ ] **E7** Fremde und kaputte Dateien: leer, falsche Endung, abgeschnitten → Meldung statt Absturz (2)

## F — Veröffentlichung (P0, 10)

- [ ] **F1** README: echte Installation (Download, entpacken, starten), „keine Pakete" entfernen (1)
- [ ] **F2** Screenshots aus `samples/` nach `docs/bilder/` lösen (1)
- [ ] **F3** Echtes Beispiel-Wohnhaus zeichnen (plausibles EG) und als Screenshot verwenden (2)
- [ ] **F4** Bekannte Grenzen in die README (was 0.3 nicht kann) (1)
- [ ] **F5** Historie entscheiden und umsetzen: umschreiben oder frisches Repo ab 0.3 (1)
- [ ] **F6** Version 0.3.0 setzen, CHANGELOG-Abschnitt schreiben (1)
- [ ] **F7** Release bauen: Linux und Windows, beide Programme, `fonts/`, keine `samples/` (2)
- [ ] **F8** Repo öffentlich stellen, Beschreibung, Themen, Social Preview (0)
- [ ] **F9** Windows auf einem echten Windows starten (0)
- [ ] **F10** Gegenprobe: beide Pakete herunterladen, entpacken, starten (1)

## G — Bedienoberfläche (P1, 12)

- [ ] **G1** Layer Manager vollständig überarbeiten (5)
- [ ] **G2** Punktfang ausbauen und besser einstellbar machen (4)
- [ ] **G3** Ansicht editieren: Ansichtsfenster bearbeiten (3)
- [ ] **G4** Fensterskalierung: drei Kleinigkeiten (Nutzer benennt sie) (2)
- [ ] **G5** Löschen-Vorschau (1)
- [ ] **G6** Linienstärke anpassen (2)
- [ ] **G7** Einstellungen aufräumen (3)
- [ ] **G8** Ribbon-Ordnung anpassen (2)
- [ ] **G9** Hilfe und Über überarbeiten (2)
- [ ] **G10** Link zum GitHub-Fehlerbericht im Hilfe-Menü (1)
- [ ] **G11** Abstand-Dilemma: klären, dann beheben (3)
- [ ] **G12** Schraffuren nachbessern (3)

## H — MCP (P1, 4)

- [ ] **H1** Alle 10 Werkzeuge durchtesten (2)
- [ ] **H2** Fehler aus H1 beheben (2)
- [ ] **H3** Benchmark MCP: Antwortzeiten der Werkzeuge (1)
- [ ] **H4** `docs/MCP.md` gegen die Wirklichkeit prüfen (1)

## I — Testrunde (P1, 4)

- [ ] **I1** Fragebogen für Testpersonen (1)
- [ ] **I2** Testleitfaden `docs/TESTEN.md`: was ausprobieren, wie melden (1)
- [ ] **I3** Issue-Vorlagen auf Laien prüfen (1)
- [ ] **I4** Discord, Website, Domain: entscheiden (0)

## J — Performance (P2, 2)

- [ ] **J1** Benchmark: Start, Zeichnen, Zoom, PDF-Export auf schwachem Rechner (2)
- [ ] **J2** Zeitbudget der Tests an die schwächste Zielmaschine anpassen (1)

## K — Später (P2, 6)

- [ ] **K1** Export Manager für mehrseitige PDFs (5)
- [ ] **K2** Eigene Dateivorlage für neue Datei (3)
- [ ] **K3** Echte Polygon-Vereinigung der Wandfüllungen (5)
- [ ] **K4** Stumpf statt Gehrung: Modellentscheidung treffen (1)
- [ ] **K5** AppImage bzw. `.deb` (3)
- [ ] **K6** GitHub-Account-Name (0)

---

## Summen

| Stufe | Punkte | Commits |
|---|---|---|
| P0 | 45 | 64 |
| P1 | 20 | 40 |
| P2 | 8 | 20 |
| **gesamt** | **73** | **124** |

Bei 35–40 Commits pro Tag: P0 knapp zwei Tage, P1 einer, P2 ein halber. Alles
zusammen ist realistisch eine Woche — Sonntag wäre P0 plus ein Teil von P1.

## Was zuerst

1. **A** komplett — ohne Datensicherung wird nichts gezeigt.
2. **B1, B2** (die zwei Bugs) und **B3** (Bestandsaufnahme) — sie sagen, wie groß B4 wird.
3. **C1** und **G4**, **G11** brauchen je einen Satz vom Nutzer; ohne sie stehen die Tickets still.
4. **D1** und **E1–E5** sind Bestandsaufnahmen: erst messen, dann schätzen.
