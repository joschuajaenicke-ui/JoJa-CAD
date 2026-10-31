# Mitgelieferte Schriften

Die technischen Schriften DIN 1451 und EN ISO 3098 gibt es auf keinem üblichen
Rechner; aus den Systemschriften (Liberation, Quicksand) lassen sie sich nicht
ableiten. Sie liegen deshalb hier und werden über `include_bytes!` in
`../src/fonts.rs` geladen. Alles andere bleibt Systemschrift.

| Datei | Schrift | Verwendung in JoJa-CAD |
|---|---|---|
| `D-DINCondensed.ttf` | D-DIN Condensed | **DIN 1451 Engschrift** |
| `D-DIN.ttf` | D-DIN | **DIN 1451 Mittelschrift**, **EN ISO 3098 Typ B** |
| `osifont-lgpl3fe.ttf` | Osifont | **EN ISO 3098 Typ A** (dünn, d = h/14) |

Die **DIN 1451 Breitschrift** entsteht zur Laufzeit aus der Mittelschrift: DIN 1451
ist eine Schrift in drei Breitenverhältnissen (rund 0,82 : 1,0 : 1,25), und die
Vorlage ist von der Mittelschrift nur 2 % breiter — zu wenig, um als eigene
Breitschrift zu gelten. `fonts::strecke` setzt dafür die Glyphen waagerecht
auseinander (× 1,25) und schreibt die Datei im Speicher neu; die gestreckte
Kopie trägt den eigenen Namen `JjcadDIN1451Breit`, weil die OFL den
reservierten Namen „D-DIN" für Abwandlungen verbietet.

## D-DIN — SIL Open Font License 1.1

Copyright (C) 2017 Datto Inc. — https://www.datto.com/fonts/d-din
Reservierte Namen: „D-DIN", „D-DIN Condensed", „D-DIN Expanded".
Vollständiger Lizenztext: `COPYING.txt`, Änderungsprotokoll: `FONTLOG.txt`.
Die Dateien sind unverändert übernommen.

## Osifont — LGPL-3.0 mit Schriftausnahme

Osifont, © Mihail Shakenov und Mitwirkende —
https://github.com/hikikomori82/osifont (Datei `osifont-lgpl3fe.ttf`).
Vollständiger Lizenztext: `LGPL-3.0.txt`, Änderungsprotokoll: `HISTORY`.
Die Datei ist unverändert übernommen.

Schriftausnahme des Projekts: Wer ein Schriftstück erstellt, das diese Schrift
benutzt, und die Schrift oder unveränderte Teile davon in das Schriftstück
einbettet, löst damit allein keine GPL-Pflichten für das Schriftstück aus —
das gilt auch für die PDF-Ausgabe von JoJa-CAD.
