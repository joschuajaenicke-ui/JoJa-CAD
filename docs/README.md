# Dokumentation — wer liest was

Diese Datei ist der Wegweiser: **wofür** es die Dokumentation gibt, **wo** ein
Thema hingehört und **welche Form** eine Datei hat.

## Drei Leser, drei Aufgaben

Die Dokumentation hat drei klar getrennte Aufgaben. Vermischt man sie, findet
keiner mehr, was er sucht.

| Leser | Frage | Bereich |
|---|---|---|
| **Anwender** | „Wie zeichne ich eine Wand? Wo liegen meine Sicherungen?“ | [`benutzer/`](benutzer/) |
| **Entwickler und Agent** | „Wie ist das gebaut, welche Regeln gelten, wie ändere ich es?“ | [`technik/`](technik/) |
| **Suchender** (auch: der Autor in zwei Jahren) | „Warum ist das so gelöst? Was wurde gemessen, was verworfen?“ | [`wissen/`](wissen/) |

## benutzer/ — Bedienung

Was man tun muss, ohne das Programm zu verstehen. Keine Begründungen, keine
Dateinamen, keine Ticket-Kennungen. Jede Anleitung ist in fünf Minuten durch.

| Datei | Inhalt |
|---|---|
| [`BEDIENUNG.md`](benutzer/BEDIENUNG.md) | Werkzeuge, Panels, erste Schritte, Fangmodi |
| [`TASTEN.md`](benutzer/TASTEN.md) | Tastenbelegung in drei Schichten |
| [`SICHERUNG.md`](benutzer/SICHERUNG.md) | Sicherung, Aufbewahrung, Wiederherstellen |
| [`GLOSSAR.md`](benutzer/GLOSSAR.md) | Begriffe — verbindlich für Oberfläche **und** Code |
| [`BEKANNTE-GRENZEN.md`](benutzer/BEKANNTE-GRENZEN.md) | was derzeit nicht geht, und was es lösen würde |

## technik/ — Bau und Regeln

Was man wissen muss, um den Code zu ändern — und was ein Agent lesen muss, bevor
er die erste Zeile anfasst. Die verbindlichen Regeln stehen in
[`../AGENTS.md`](../AGENTS.md): `technik/` erklärt, `AGENTS.md` verpflichtet.

| Datei | Inhalt |
|---|---|
| [`ARCHITEKTUR.md`](technik/ARCHITEKTUR.md) | Aufbau, Datenfluss, Entscheidungen |
| [`FORMAT.md`](technik/FORMAT.md) | Dateiformat `.jjcad` (JSON, offen) |
| [`BEFEHLE.md`](technik/BEFEHLE.md) | Action-Referenz: jede Aktion mit Parametern |
| [`MCP.md`](technik/MCP.md) | Schnittstelle für KI-Steuerung |
| [`I18N.md`](technik/I18N.md) | Sprachen, Katalog, Prüfskript |
| [`VERSIONEN.md`](technik/VERSIONEN.md) | Versionierung und Release-Ablauf |

## wissen/ — Herleitungen und Messungen

Der wertvollste Teil und der, der am häufigsten fehlt: **warum** etwas so gelöst
ist, welche Variante verworfen wurde und was gemessen wurde. Hier stehen die
Probleme, die beim Bauen wirklich wehgetan haben.

| Datei | Frage |
|---|---|
| [`WANDANSCHLUSS.md`](wissen/WANDANSCHLUSS.md) | Wo liegen die Nahtpunkte `S_l`, `S_r` bei beliebigen Wandwinkeln? |
| [`BOGEN_BASELINE.md`](wissen/BOGEN_BASELINE.md) | Wie glatt muss ein Bogen sein, und wie wird das gemessen? |
| [`DAEMMUNG-SCHLAUFEN.md`](wissen/DAEMMUNG-SCHLAUFEN.md) | Welche Kurve ergibt gleich große Schlaufen bei jeder Wanddicke? |
| [`PDF-IMPORT.md`](wissen/PDF-IMPORT.md) | Wie wird ein PDF-Grundriss zur Zeichnung? (noch Umbau vom Plan zur Erklärung) |
| [`WAYLAND-DATEI-DROP.md`](wissen/WAYLAND-DATEI-DROP.md) | Warum kommt unter Wayland kein Datei-Drop an? |
| [`TEST-WANDFANG.md`](wissen/TEST-WANDFANG.md) | Wie wird Punktfang auf Wandkanten geprüft? |

Belege liegen im Unterordner der jeweiligen Datei: `bogen-baseline/` und
`bogen-glaettung/` gehören zu `BOGEN_BASELINE.md` (Messreihen, Vorher/Nachher-
Bilder), `test-evidence/` zu `TEST-WANDFANG.md`.

Jede Datei in `wissen/` folgt derselben Form, damit sie zitierbar bleibt:

1. **Frage** — welches Problem, in einem Satz.
2. **Lösung** — was gerechnet oder gebaut wird, mit Formeln und Bildern.
3. **Warum nicht anders** — die verworfenen Wege und was an ihnen scheiterte.
4. **Grenzen** — wo die Lösung nicht gilt.
5. **Beleg** — Messwerte, Vorher/Nachher-Bilder, Testname.

Ohne Punkt 5 ist es eine Behauptung, keine Herleitung.

## Arbeitsmittel — nicht veröffentlicht

`STATUS.md` (Arbeitsstand), `TODO-0.3.md` und `ROADMAP-0.3.md` (Planung) stehen
über `.gitignore` außerhalb des Repos. Sie enthalten Ticket-Kennungen,
Halbfertiges und Absichten: im öffentlichen Repo sind sie Lärm und morgen falsch.

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
