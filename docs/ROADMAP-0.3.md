# Roadmap 0.3 — Veröffentlichung

Ziel: **Sonntagabend ist 0.3.0 öffentlich.** Kapazität: drei Tage, bisher
30–40 Commits pro Tag, also rund 100–120 Commits. Die Schätzungen unten sind in
Commits, nicht in Stunden.

Die vollständige, abhakbare Liste steht in [`TODO-0.3.md`](TODO-0.3.md) —
105 Punkte, 191 Commits. Dieses Dokument sagt, in welcher Reihenfolge und warum.

Zwei Tore, nicht eines:

1. **Freunde testen** — sie brauchen eine Datei. Bei privatem Repo können sie
   nichts herunterladen: Paket direkt schicken oder früher öffentlich stellen.
2. **Öffentlich** — Repo sichtbar, Release mit beiden Paketen, Issues offen.

## Die eine Regel

**Kein Datenverlust beim Benutzer.** Alles andere darf warten, das nicht. Ein
Tester, der eine Stunde zeichnet und beim Absturz alles verliert, testet nie
wieder — und erzählt es weiter.

## Stufe 1 — Blocker (P0)

Ohne diese Punkte wird nicht veröffentlicht.

### A. Datensicherung

| Punkt | Aufwand | Stand |
|---|---|---|
| Autosave verdrahten | 3 | `persist::autosave_path()` existiert, wird **nirgends aufgerufen** |
| Atomar speichern (temporär schreiben, dann umbenennen) + `.bak` der Vorgängerversion | 2 | heute wird direkt über die Datei geschrieben |
| Panic-Hook: Meldung mit Pfad zur Sicherung statt wortlosem Verschwinden | 2 | kein Hook vorhanden |
| Beim Schließen mit ungespeicherten Änderungen warnen | 1 | — |
| Wiederherstellung nach Absturz anbieten (Autosave gefunden → öffnen?) | 1 | — |

### B. Grundelemente und Interaktion

| Punkt | Aufwand | Anmerkung |
|---|---|---|
| Bug: Rotation | 1–2 | Reproduktion nötig (siehe unten) |
| Bug: 2-Klick-Auswahl | 1–2 | Reproduktion nötig |
| Grundelemente in der Bedienung voll funktionsfähig | 5–8 | Bestandsaufnahme zuerst: welche Werkzeuge haben Lücken? |

### C. Bemaßung

| Punkt | Aufwand | Anmerkung |
|---|---|---|
| Maßketten DIN-konform, 3 Fehler | 3–5 | die drei Fehler müssen benannt werden — DIN-Konformität kann nur der Fachmann abnehmen |

### D. Blöcke

| Punkt | Aufwand | Anmerkung |
|---|---|---|
| Grundkonzept vollständig funktionsfähig | 6–10 | erst Bestandsaufnahme (1 Commit), dann Lücken schließen |

### E. Import und Export

| Punkt | Aufwand | Anmerkung |
|---|---|---|
| PDF, DXF, PNG im Detail prüfen und reparieren | 5–8 | Rundlauf mit echten Dateien; Fehlerliste zuerst |

### F. Veröffentlichung

| Punkt | Aufwand | Anmerkung |
|---|---|---|
| README auf Wahrheit bringen (Installation, keine „keine Pakete"-Lüge) | 1 | — |
| Screenshots aus `samples/` nach `docs/bilder/` lösen | 1 | `samples/` ist Debug-Material |
| Historie entscheiden (umschreiben oder frisches Repo ab 0.3) | 1 | danach nur mit Aufwand |
| 0.3.0 schneiden: Version, CHANGELOG, Tag, Release mit beiden Paketen | 2 | — |
| Repo öffentlich, Beschreibung, Themen, Social Preview | 0 | — |
| **Windows auf echtem Windows starten** | 0 | gebaut ja, gestartet nie |

**Summe P0: 35–48 Commits** — ein guter Tag, wenn nichts klemmt.

## Stufe 2 — vor dem Zeigen an Freunde (P1)

| Punkt | Aufwand |
|---|---|
| Layer Manager vollständig überarbeiten | 4–6 |
| Punktfang ausbauen und besser einstellbar machen | 3–5 |
| MCP testen und überarbeiten | 3–5 |
| Blockbibliothek überarbeiten und übersetzen | 3–5 |
| Abstand-Dilemma lösen | 2–3 |
| Ansicht editieren (Ansichtsfenster) | 2–3 |
| Fensterskalierung: 3 Kleinigkeiten | 2 |
| Löschen-Vorschau | 1 |
| Linienstärke anpassen | 1–2 |
| Hilfe und Über überarbeiten | 1–2 |
| Verlinkung zum GitHub für Fehlerberichte (Hilfe-Menü) | 1 |
| Richtiges Beispiel-Wohnhaus für den Screenshot | 1–2 |
| Fragebogen für Testpersonen | 1 |
| Testleitfaden `docs/TESTEN.md` | 1 |
| `.desktop`-Eintrag ins Linux-Paket | 1 |
| Bekannte Grenzen in die README | 1 |
| Englische Texte vollständig (`scripts/i18n-check.py`) | 1–2 |

**Summe P1: 29–42 Commits.**

## Stufe 3 — nach der Veröffentlichung (P2)

Schraffuren nachbessern (2–3) · Benchmark Performance (1–2) · Benchmark MCP (1–2)
· Einstellungen aufräumen (2–3) · Ribbon-Ordnung (1–2) · Export Manager für
mehrseitige PDFs (4–6) · eigene Dateivorlage für neue Datei (2–3) · echte
Polygon-Vereinigung der Wandfüllungen · Stumpf-statt-Gehrung entscheiden ·
AppImage bzw. `.deb`.

## Stufe 4 — Entscheidungen, keine Commits

Discord-Server · Website · Domain sichern · GitHub-Account-Name.
Domain und Account-Name sind in zehn Minuten erledigt, wenn du dich entschieden
hast — der Rest wartet auf Feedback aus der Testrunde.

## Was ich streichen würde

- **Export Manager für mehrseitige PDFs** — großes Feature, für eine Testrunde
  nicht nötig. Ein Blatt pro PDF reicht zum Bewerten.
- **Schraffuren, Einstellungen, Ribbon-Ordnung** — Politur. Fällt Testern auf,
  aber sie schreiben es als Wunsch, nicht als Blocker.
- **Benchmarks** — nur, wenn Sonntag Luft bleibt.
- **Discord und Website** — erst wenn jemand außer dir mitliest.

## Tagesplan

**Freitag — Sicherheitsnetz und Kern**
A komplett (Datensicherung), dann B (Rotation, 2-Klick-Auswahl, Grundelemente).
Abends: Test, dass ein Absturz die Arbeit nicht mehr frisst.

**Samstag — Fachliches und Robustheit**
C (Maßketten), D (Blöcke), E (Import/Export). Dazwischen **Windows auf echtem
Windows starten**. Abends: P1-Kern (Layer Manager, Punktfang, MCP).

**Sonntag — veröffentlichen**
Vormittag: P1-Rest, README, Screenshots, Wohnhaus-Beispiel.
Nachmittag: Historie, 0.3.0 bauen, Release, öffentlich stellen, Pakete erneut
herunterladen und starten. Abend: Freunden den Link schicken.

## Was ich von dir brauche

Vier Punkte sind ohne dich nicht bearbeitbar:

1. **Abstand-Dilemma** — welcher Abstand ist gemeint? (Versetzen-Werkzeug,
   Abstandseingabe beim Zeichnen, Wandabstand?)
2. **Maßketten: die drei Fehler** — je ein Satz, was falsch ist.
3. **Fensterskalierung: die drei Kleinigkeiten** — was genau stört?
4. **Rotation und 2-Klick-Auswahl** — was passiert, was soll passieren?

Ohne diese vier Sätze schätze ich falsch und die Tickets laufen ins Leere.

## Definition of Done (Sonntagabend)

- 0.3.0 ist öffentlich, zwei Pakete herunterladbar, CI grün.
- Ein Fremder kann ohne Erklärung starten, zeichnen, als PDF ausgeben.
- **Kein Datenverlust**: Autosave, Sicherung, Meldung bei Absturz.
- README beschreibt, was wirklich da ist — Installation, Screenshots, Grenzen.
- Fehlerbericht möglich: Issues offen, Vorlagen da, Link im Hilfe-Menü.

## Risiken

- **Windows ist ungetestet.** Wenn es dort nicht läuft, ist die halbe
  Ankündigung falsch — deshalb Samstag, nicht Sonntag.
- **Blöcke und Import/Export sind unbekannte Größen.** Beide können doppelt so
  teuer werden wie geschätzt; dann rutschen P1-Punkte, nicht P0.
- **Historie umschreiben** kostet einen Force-Push. Frisches Repo ist der
  schnellere, sichere Weg.
- **Drei Tage sind knapp.** Realistisch: P0 vollständig, P1 zu etwa zwei
  Dritteln, P2 gar nicht. Das reicht für eine ehrliche Testrunde.
