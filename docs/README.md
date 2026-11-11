# Dokumentation — wer liest was

Diese Datei ist der Wegweiser. Sie beantwortet zwei Fragen: **wofür** es die
Dokumentation gibt und **wo** ein Thema hingehört.

## Drei Leser, drei Aufgaben

Die Dokumentation hat drei klar getrennte Aufgaben. Vermischt man sie, findet
keiner mehr, was er sucht — genau das ist der heutige Zustand.

| Leser | Frage | Bereich |
|---|---|---|
| **Anwender** | „Wie zeichne ich eine Wand? Wo liegen meine Sicherungen?“ | `benutzer/` |
| **Entwickler und Agent** | „Wie ist das gebaut, welche Regeln gelten, wie ändere ich es?“ | `technik/` |
| **Suchender** (auch: der Autor in zwei Jahren) | „Warum ist das so gelöst? Was wurde gemessen, was verworfen?“ | `wissen/` |

Dazu ein vierter Bereich, der **nicht** ins veröffentlichte Repo gehört:
Arbeitsmittel wie der aktuelle Arbeitsstand und die ToDo-Liste. Sie sind
Momentaufnahmen, veralten in Tagen und interessieren niemanden außerhalb.

## Die drei Bereiche

### `benutzer/` — Bedienung

Was man tun muss, ohne das Programm zu verstehen. Keine Begründungen, keine
Dateinamen, keine Tickets. Jede Anleitung ist in fünf Minuten durch.

Gehört hierher: Bedienung, Tastenkürzel, Sicherung und Aufbewahrung, Glossar,
bekannte Grenzen.

### `technik/` — Bau und Regeln

Was man wissen muss, um den Code zu ändern — und was ein Agent lesen muss, bevor
er die erste Zeile anfasst.

Gehört hierher: Architektur und Datenfluss, Dateiformat, Befehlsreferenz (die
Action-Schicht), MCP-Schnittstelle, Übersetzungen, Release-Ablauf.
Die verbindlichen Regeln stehen in [`../AGENTS.md`](../AGENTS.md) — `technik/`
erklärt, `AGENTS.md` verpflichtet.

### `wissen/` — Herleitungen und Messungen

Das ist der wertvollste Teil und der, der am häufigsten fehlt: **warum** etwas so
gelöst ist, welche Variante verworfen wurde und was gemessen wurde. Hier stehen
die Probleme, die beim Bauen wirklich wehgetan haben — Wandgehrung, Bogen­glättung,
Dämmungsschlaufen, PDF-Import, Wayland-Datei-Drop.

Jede Datei in `wissen/` folgt derselben Form, damit sie zitierbar bleibt:

1. **Frage** — welches Problem, in einem Satz.
2. **Lösung** — was gerechnet oder gebaut wird, mit den Formeln und Bildern.
3. **Warum nicht anders** — die verworfenen Wege und was an ihnen scheiterte.
4. **Grenzen** — wo die Lösung nicht gilt.
5. **Beleg** — Messwerte, Vorher/Nachher-Bilder, Testname. Bilder und Messreihen
   liegen im Unterordner der jeweiligen Datei.

Ohne Punkt 5 ist es eine Behauptung, keine Herleitung.

## Wo die vorhandenen Dateien hingehören

| Datei | Bereich | Anmerkung |
|---|---|---|
| `BEDIENUNG.md` | `benutzer/` | Werkzeuge, Panels, erste Schritte |
| `TASTEN.md` | `benutzer/` | Tastenbelegung in drei Schichten |
| `SICHERUNG.md` | `benutzer/` | Sicherung und Aufbewahrung; Anhang „Messungen“ nach `wissen/` |
| `GLOSSAR.md` | `benutzer/` | Begriffe; verbindlich für Oberfläche **und** Code |
| `BEKANNTE-GRENZEN.md` | `benutzer/` | was derzeit nicht geht, und was es lösen würde |
| `ARCHITEKTUR.md` | `technik/` | Aufbau, Datenfluss, Entscheidungen |
| `FORMAT.md` | `technik/` | Dateiformat `.jjcad` |
| `BEFEHLE.md` | `technik/` | Action-Referenz |
| `MCP.md` | `technik/` | Schnittstelle für KI-Steuerung (auch für Anwender mit Agent) |
| `I18N.md` | `technik/` | Sprachen, Katalog, Prüfskript |
| `VERSIONEN.md` | `technik/` | Versionierung und Release-Ablauf |
| `WANDANSCHLUSS.md` | `wissen/` | Herleitung der Nahtpunkte `S_l`, `S_r` |
| `BOGEN_BASELINE.md` | `wissen/` | Bogenglätte: Messwerte, Abnahmekriterien |
| `DAEMMUNG-SCHLAUFEN.md` | `wissen/` | Schlaufenkurve der Dämmung |
| `PDF-IMPORT.md` | `wissen/` | derzeit noch Umsetzungsplan — nach Abschluss umschreiben |
| `WAYLAND-DATEI-DROP.md` | `wissen/` | Befund zu winit 0.30 |
| `TEST-WANDFANG.md` | `wissen/` | Testbericht Punktfang; Messmethode ist der Wert |
| `bogen-baseline/`, `bogen-glaettung/`, `test-evidence/` | `wissen/` | Belege, bleiben als Unterordner ihrer Datei |

**Arbeitsmittel, nicht veröffentlichen:** `STATUS.md` (Arbeitsstand),
`TODO-0.3.md` und `ROADMAP-0.3.md` (Planung). Sie enthalten Ticket-Kennungen,
Halbfertiges und Absichten — im öffentlichen Repo sind sie Lärm und morgen
falsch. Vorschlag: lokal behalten (über `.gitignore` ausgenommen) oder beim
Öffentlichstellen entfernen.

## Regeln, die das Durcheinander verhindern

1. **Eine Datei beantwortet eine Frage.** Wird eine Datei zur Hälfte Plan und
   zur Hälfte Erklärung, ist sie zu teilen.
2. **Der Bereich steht im Pfad, nicht im Titel.** Wer `wissen/` öffnet, erwartet
   keine Klick-Anleitung.
3. **Keine Ticket-Kennungen** in `benutzer/` und `technik/` — in `wissen/` sind
   sie erlaubt, weil dort die Entstehung dazugehört.
4. **Bilder liegen bei ihrer Datei**, nicht in einem Sammelordner.
5. **Alles, was in einer Datei steht, ist entweder belegt oder als offen
   markiert.** „Sieht gut aus“ ist kein Beleg.

## Offen

Das Verschieben der Dateien berührt alle Querverweise (rund 40 Verlinkungen) und
kollidiert mit laufenden Arbeiten an denselben Dateien. Vorschlag: in einem Zug
erledigen, wenn kein Worker in `docs/` schreibt — mit anschließender Prüfung
aller Links. Bis dahin steht diese Tabelle als Zielbild.
