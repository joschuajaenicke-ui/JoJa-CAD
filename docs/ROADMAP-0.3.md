# Roadmap 0.3 — Veröffentlichung

Ziel: **Sonntagabend ist 0.3.0 öffentlich** und Freunde können es testen.
Kapazität: drei Tage, erfahrungsgemäß 30–40 Commits pro Tag. Die Schätzungen
unten sind in Commits, nicht in Stunden.

Zwei Tore, nicht eines:

1. **Freunde testen** — sie brauchen eine Datei. Bei privatem Repo können sie
   nichts herunterladen: Paket direkt schicken (Mail, Cloud) oder das Repo
   früher öffentlich stellen.
2. **Öffentlich** — Repo sichtbar, Release mit beiden Paketen, Issues offen.

## P0 — blockiert die Veröffentlichung

| # | Punkt | Warum | Aufwand |
|---|---|---|---|
| 1 | **README stimmt nicht mehr**: „Es gibt noch keine fertigen Pakete", Installation als Platzhalter | Das ist die erste Seite, die jeder liest — und sie ist falsch, seit 0.2.0 existiert | 1 |
| 2 | **Screenshots aus `samples/` lösen** (`docs/bilder/`) | `samples/` ist Debug-Material; die README-Bilder sollen nicht davon abhängen | 1 |
| 3 | **Autosave verdrahten** — `persist::autosave_path()` existiert, wird aber nirgends aufgerufen | Ein Tester, der eine Stunde zeichnet und beim Absturz alles verliert, testet nie wieder | 2–3 |
| 4 | **Panic-Hook**: Absturz zeigt eine Meldung mit Pfad zur Sicherung statt wortlos zu verschwinden | Ohne Meldung ist kein Fehlerbericht möglich | 1 |
| 5 | **Öffnungsposition als Wandparameter** (Abstand längs, Lage quer) | Heute bleiben Öffnungen stehen, wenn die Wand verschoben wird — der erste Fehler, den jeder Tester findet | 3–5 |
| 6 | **Tür-/Fenster-Ausrichtung nachträglich änderbar** | Beim Anlegen gesetzt, danach nicht mehr korrigierbar | 2–3 |
| 7 | **Fremde Dateien ohne Absturz**: kaputte PDF/DXF, falsche Endung, leere Datei | Tester öffnen alles, was sie finden | 2 |
| 8 | **Historie entscheiden**: umschreiben oder frisches öffentliches Repo ab 0.3 | Danach nur noch mit Aufwand möglich; die Historie enthält `/home/joschua`-Pfade und alte Prozessdateien | 1 + Entscheidung |
| 9 | **0.3.0 schneiden**: Version, CHANGELOG, Tag, Release mit Linux- und Windows-Paket | der eigentliche Akt | 2 |
| 10 | **Repo öffentlich**: Beschreibung, Themen, Social Preview, Issues an | Sichtbarkeit | 0 |

Summe P0: **15–20 Commits** — passt in einen Tag, wenn nichts dazwischenkommt.

## P1 — vor dem Zeigen an Freunde

| # | Punkt | Warum | Aufwand |
|---|---|---|---|
| 11 | **Testleitfaden** (`docs/TESTEN.md`): was ausprobieren, was melden, wie | Ohne Leitfaden kommt „geht nicht" statt eines Fehlerberichts | 1 |
| 12 | **Maßketten-Griffe** | Bemaßung lässt sich nicht nachziehen — fällt beim ersten Plan auf | 2 |
| 13 | **`.desktop`-Eintrag im Linux-Paket** (`scripts/install-desktop.sh` mitliefern, Hinweis in README) | Ohne Eintrag fehlt das Symbol in Taskleiste/Startmenü unter Wayland | 1 |
| 14 | **Bekannte Grenzen in der README** (was 0.3 nicht kann) | Erwartungen setzen, bevor jemand sie findet | 1 |
| 15 | **Englische Texte vollständig** (`scripts/i18n-check.py`) | Sprachumschaltung ist da, Lücken fallen auf | 1–2 |

## P2 — nach der Veröffentlichung

- Echte Polygon-Vereinigung der Wandfüllungen (nötig erst bei verschiedenen Füllfarben)
- Stumpf-statt-Gehrung bei entgegengesetzter Ausrichtung: Modellentscheidung treffen
- AppImage bzw. `.deb` statt Tar-Archiv
- GitHub-Account-Name, Projektseite
- Performance-Budget auf schwächeren Rechnern prüfen

## Tagesplan

**Freitag — Sicherheitsnetz und die zwei Funktionslücken**
1, 2, 3, 4, dann 5 (Öffnungen folgen der Wand). Abends: Test, dass eine
verschobene Wand ihre Öffnungen mitnimmt.

**Samstag — Robustheit und Feinschliff**
6, 7, dann P1 nach Lust und Zeit (11, 12, 13). Dazwischen: **Windows auf einem
echten Windows starten** — das ist der einzige Punkt, der sich nicht am
Schreibtisch erledigen lässt. Abends: 0.3.0 vorbereiten (Version, CHANGELOG).

**Sonntag — veröffentlichen**
Vormittag: 8 (Historie), 9 (Release bauen, beide Pakete, Smoke-Test).
Nachmittag: 10 (öffentlich stellen), README gegen die Wirklichkeit prüfen,
Release-Seite kontrollieren, Pakete erneut herunterladen und starten.
Abend: Freunden den Link schicken.

## Definition of Done (Sonntagabend)

- 0.3.0 ist öffentlich, zwei Pakete (Linux, Windows) sind herunterladbar, CI grün.
- Ein Fremder kann ohne Erklärung starten, zeichnen, als PDF ausgeben.
- Kein Absturz ohne Meldung; eine Sicherung existiert und wird gefunden.
- README beschreibt, was wirklich da ist — Installation, Screenshots, Grenzen.
- Ein Fehlerbericht ist möglich (Issues offen, Vorlagen vorhanden).

## Risiken

- **Windows ist ungetestet.** Gebaut ja, gestartet nie. Wenn es dort nicht läuft,
  ist die halbe Ankündigung falsch — deshalb Samstag, nicht Sonntag.
- **Historie umschreiben** kostet einen Force-Push und muss vor dem Öffentlich-
  stellen passieren. Frisches Repo ist der schnellere, sichere Weg.
- **Freunde ohne Zugang**: bei privatem Repo gibt es keinen Download-Link für
  sie. Entweder Datei direkt schicken oder früher öffentlich stellen.
- **Drei Tage sind knapp, wenn P0 Punkt 5 oder 6 klemmt** (beide berühren den
  Änderungspfad der Wände). Dann zuerst 5, 6 notfalls auf 0.3.1 verschieben.
