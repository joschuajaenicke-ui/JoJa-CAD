# JoJa-CAD — Sicherung: Stände und Aufbewahrung

**Stand: umgesetzt.** Diese Datei beschreibt, was der Anwender von der Sicherung
sieht und was bei der Umsetzung einzuhalten ist. Maßgeblich ist die Spezifikation
**SRR (Save – Resume – Recover), Fassung 0.4** (Entwurf vom 14.11.2026,
`~/Schreibtisch/srr-spezifikation-v0.4-entwurf.md`); aufbewahrt wird nach ihrem
Referenzverfahren **Geometric Retention** (Anhang A). Die acht Prüfkriterien der
Spezifikation stehen in Abschnitt 13.

Grundsatz: In die Zieldatei (die vom Benutzer gewählte .jjcad) schreibt nur der Benutzer; automatische Stände landen ausschließlich im Sicherungsordner (Pool). Alle Zeitangaben der Aufbewahrung rechnen in Zyklen, nicht in Kalenderzeit.

Die Sicherung legt während der Arbeit automatisch Stände an und hält sie mit einer festen Zahl von Plätzen: vorne dicht, hinten grob, Tiefe mindestens der eingestellte Zeitraum. Der Speicherbedarf bleibt dadurch fest. Gespeichert wird die Zeichnung weiterhin nur vom Benutzer.

## Teil A — Bedienung

### 1. Was automatisch passiert

Ändert sich die Zeichnung, entsteht einmal je Intervall ein Stand im Pool. Ohne Änderung entsteht nichts; der Vergleich erfolgt über SHA-256.

Die Zieldatei wird von der Sicherung nie angefasst; sie ändert sich nur durch „Speichern“. Das Speichern selbst ist atomar: temporäre Datei, fsync, rename. Ein Absturz hinterlässt keine halbe Datei.

Beim Beenden wird sofort gesichert, unabhängig vom Intervall; die Sitzung (Reiter, Ansichten, Fensterlage) wird dabei mitgeschrieben.

Ein Zyklus ist ein Autosave mit Änderung. Der Zähler n ist global über alle Zeichnungen; Uhrzeit und Zeitumstellung spielen für Entscheidungen keine Rolle, der Zeitstempel eines Standes dient nur der Anzeige.

Nach einem Absturz ist höchstens ein Intervall Arbeit verloren.

### 2. Die Einstellungen

| Einstellung | Bedeutung | Vorgabe |
|---|---|---|
| Verlauf aufbewahren | an = jeder Stand bleibt und wird nach Alter ausgedünnt; aus = je Zeichnung bleibt nur der neueste Stand, der beim nächsten Sichern ersetzt wird. Abschalten löscht nichts. | an |
| Anzahl der Sicherungen (N) | Wie viele Stände insgesamt im Pool liegen, für alle Zeichnungen zusammen. Bereich 30 bis 2000. | 150 |
| Aufbewahrungszeitraum (T) | Wie weit der Verlauf zurückreicht; Wahlen Tag (8 h), Woche (40 h), Monat (160 h), Quartal (480 h). Länger als ein Quartal ist nicht vorgesehen. | Woche |
| Sicherung alle (I) | Abstand zwischen zwei Sicherungen in Minuten. Bereich 1 bis 240. | 1 |

Die Seite arbeitet auf einem Entwurf; darunter liegen „Übernehmen“ und „Verwerfen“. Erst „Übernehmen“ speichert. Vorher wird gerechnet, wie viele Stände die neuen Werte kosten; ginge etwas verloren, erscheint ein Fenster mit dem Text „Mit diesen Einstellungen werden X von Y Sicherungen endgültig gelöscht“ und den Knöpfen „Abbrechen“ und „Endgültig löschen und übernehmen“. Ohne Verlust wird sofort übernommen und ausgedünnt.

Die Zeile „Verworfen“ zeigt die Zahl der Stände, die als verworfen markiert sind (Papierkorb). Sie bleiben bis zum Verfall liegen und kehren nicht von selbst zurück.

Die Zeile „Aufräumen“ enthält den Knopf „N ohne Zeichnung löschen“ für Stände, deren vermerkte Zieldatei fehlt; eine Rückfrage nennt die Zahl. Stände ohne vermerkten Pfad bleiben.

Erscheint der Hinweis in Rot, wächst der Pool wegen der Zeichnungszahl über N: Je Zeichnung sind zwei Plätze reserviert (neuester und ältester Stand); die Obergrenze ist max(N, 2 × Zeichnungen).

Der Aufbewahrungszeitraum ist in Arbeitsstunden angegeben, nicht in Kalendertagen:

| Auswahl | Arbeitsstunden | Zyklen bei 1 Minute | bei Dauerbetrieb |
|---|---|---|---|
| Tag | 8 | 480 | 1 Arbeitstag |
| Woche | 40 | 2 400 | 5 Arbeitstage |
| Monat | 160 | 9 600 | 20 Arbeitstage |
| Quartal | 480 | 28 800 | 60 Arbeitstage |

Formel: `T = Arbeitsstunden × 60 / Intervall`. Bei 5 Minuten Intervall sind es für ein Quartal 5 760 Zyklen statt 28 800; die Reichweite in Arbeit bleibt gleich, der Speicherbedarf sinkt.

### 3. Der Abstand der Stände

Der Dialog zeigt den mittleren relativen Abstand g an; einstellbar ist er nicht. Bei 150 Plätzen, drei Zeichnungen und einer Woche liegt g bei etwa 12,7 % — das sind rund 8 Minuten bei einem Stand von vor einer Stunde, rund 61 Minuten bei einem von vor einem Arbeitstag und rund 5 Stunden bei einem von vor einer Woche.

```
k(g) = 1/g + ln(g·T) / ln(1/(1−g))
```

k ist die Zahl der Stände je Zeichnung, T die Tiefe in Zyklen. Bis zum Alter 1/g liegen alle Stände lückenlos, danach wächst der Abstand geometrisch. Zu gegebenem k wird g numerisch gelöst; k ergibt sich aus N geteilt durch die Zahl der aktiven Zeichnungen.

Stände `k` je aktive Zeichnung für den Zielabstand `g` bei `I = 1` Minute:

| g | Tag (8 h) | Woche (40 h) | Monat (160 h) | Quartal (480 h) |
|---|---|---|---|---|
| 10 % | 47 | 62 | 75 | 86 |
| 15 % | 33 | 43 | 51 | 58 |
| 20 % | 26 | 33 | 39 | 44 |

T geht nur logarithmisch in den Platzbedarf ein. Zwölffache Tiefe (Woche gegen Quartal) kostet bei gleichem Abstand etwa 15 Stände mehr; entscheidend sind der Abstand und die Zahl der aktiven Zeichnungen.

### 4. Was garantiert ist

1. Nach einem Absturz fehlt höchstens die Arbeit eines Intervalls.
2. Der Pool bleibt an oder unter max(N, 2 × Zeichnungen) und wächst nicht mit der Nutzungsdauer.
3. Der älteste Stand einer Zeichnung (Anker) ist mindestens T alt und höchstens T/(1−g).
4. Der neueste Stand einer Zeichnung (Pin) bleibt bis T erhalten. Ist er T alt geworden, verfällt er samt Verlauf, und die Zeichnung belegt keine Plätze mehr.
5. Stände liegen im Format der Zieldatei und sind einzeln zu öffnen.

### 5. Viele Zeichnungen

Jede Zeichnung belegt zwei Plätze: neuester und ältester Stand. Erst dadurch hat jeder Löschkandidat zwei Nachbarn in derselben Zeichnung.

Zur Planung gilt: N etwa gleich Zahl der aktiven Zeichnungen × k(g), mindestens 30 Stände je aktive Zeichnung.

| Zeichnungen | Plätze je Zeichnung bei N = 150 | Abstand g bei einer Woche |
|---|---|---|
| 1 | 150 | 3,6 % |
| 2 | 75 | 8,1 % |
| 3 | 50 | 12,7 % |
| 4 | 38 | 17,3 % |
| 6 | 25 | 26,2 % |
| 10 | 15 | 42,2 % |

Ruhige Zeichnungen geben ungenutzte Plätze an aktive ab. Weil die Nachbarn je Zeichnung gebildet werden, erreichen alle Zeichnungen denselben relativen Abstand, soweit sie genug Stände haben. Kein Verfahren schafft Speicher: Wachsen die aktiven Zeichnungen bei gleichem N, sinkt die Auflösung je Zeichnung.

### 6. Wiederherstellen

Es gibt zwei Wege; beide lesen denselben Pool.

Beim Start (Sitzung): Die Sitzung merkt sich Reiter mit Kennung und Speicherort. Für jeden Reiter gilt:

1. Trägt der neueste Stand ungespeicherte Arbeit (nicht als Dateistand und nicht als verworfen markiert) und weicht vom Inhalt der Zieldatei ab, wird sein Inhalt geladen, der Speicherort bleibt, und der Reiter gilt als geändert. „Speichern“ überschreibt dann die Zieldatei.
2. Sonst gilt die Zieldatei von der Platte.
3. Fehlt die Zieldatei, kommt der neueste Stand als unbenannte Zeichnung ohne Speicherort zurück.
4. Fehlt beides, entfällt der Reiter mit einer Meldung.

Konflikt (R2.3): Wurde die Zieldatei seit dem letzten Stand von einer anderen Anwendung verändert, während im Pool ungespeicherte Arbeit liegt, entscheidet der Benutzer. Das Fenster „Datei außerhalb geändert“ teilt mit, dass die Datei nach dem letzten Sichern außerhalb von JoJa-CAD verändert wurde und im Sicherungsordner ungespeicherte Arbeit mit Zeitangabe liegt; die Knöpfe lauten „Datei verwenden“ und „Stand von <Zeit> verwenden“. Bis zur Entscheidung gilt die Datei, überschrieben wird nichts, die andere Fassung bleibt als Stand erhalten. Wird die Datei gewählt, wird ihr Inhalt als neuester Stand festgehalten, damit der nächste Start nicht erneut fragt. Das Schließen des Fensters über das Kreuz heißt: die Datei gilt.

Ausdrückliches Öffnen (Doppelklick, Datei als Argument): Der Stand hat Vorrang, damit ein Doppelklick die Arbeit von eben nicht verwirft. Wurde die Datei inzwischen von außen geändert, gilt die geöffnete Datei.

Aus dem Ordner: Einstellungen → Sicherung → „Sicherungsordner“ öffnet den Ordner im Dateimanager. Jeder Stand ist eine gewöhnliche .jjcad-Datei und lässt sich öffnen; ein so geöffneter Stand trägt keinen Speicherort, gespeichert wird er erst durch „Speichern unter“.

Verworfen (R3.5): Wählt der Benutzer beim Schließen eines Reiters „Verwerfen“, wird der letzte Zustand der Arbeit als Stand abgelegt und als „verworfen“ markiert; danach wird der Dateiinhalt als neuester Stand geschrieben. Die verworfene Arbeit kehrt beim Start nicht zurück, bleibt aber bis zum Verfall im Sicherungsordner und ist in den Einstellungen als Zahl sichtbar. Sie unterliegt derselben Obergrenze und demselben Verfall wie jeder Stand.

### 7. Was die Sicherung nicht ist

- Kein Archiv: Nach dem Aufbewahrungszeitraum ist ein Stand endgültig weg. Dauerhafte Meilensteine sind eine eigene Funktion.
- Kein Ersatz für eine Sicherung gegen Medienverlust: Platte defekt, Ordner gelöscht oder verschlüsselt — dagegen hilft nur eine Kopie außerhalb des Rechners. Der Sicherungsordner gehört deshalb auf ein Netzlaufwerk mit redundanter Sicherung (Einstellungen → Speicherorte → Sicherungen).
- Keine Versionskontrolle: Stände entstehen automatisch und verfallen.

---

## Teil B — Umsetzung

### 8. Begriffe

| Begriff | Bedeutung |
| --- | --- |
| Zyklus | Autosave mit Änderung; globaler Zähler n über alle Zeichnungen |
| Stand | Gesicherte Fassung einer Zeichnung, mit Zyklusnummer und Hash |
| Pool | Alle Stände eines Sicherungsordners zusammen |
| Arbeitskopie | Neuester Stand einer Zeichnung im Pool, atomar geschrieben |
| Pin | Neuester Stand je Zeichnung; wird beim Ausdünnen nie angefasst und verfällt, sobald er T Zyklen alt ist |
| Anker | Ältester Stand je Zeichnung; wird beim Ausdünnen nie angefasst und nur ersetzt, indem ein jüngerer Stand nachrückt |
| Verworfen | Stand, dessen Arbeit der Benutzer beim Schließen verworfen hat; bleibt liegen, wird nie automatisch zurückgeholt |
| Verlauf | Alle Stände außer Pin und Anker; wird ausgedünnt |
| Alter | a = n − Zyklus eines Standes, in Zyklen |

### 9. Datenmodell

Es gibt einen Pool für alle Zeichnungen eines Projektordners, damit dateiübergreifend ausgedünnt werden kann.

Der Pool besteht aus zwei Arten von Dateien: einem Manifest mit dem globalen Zähler n und der Liste der Stände sowie einer Datei je Stand. Die Standdatei trägt den Namen der Zieldatei, das Suffix `_autosave_` und ein Rückwärtsdatum:

```text
<Sicherungen>/manifest.json
<Sicherungen>/<Zieldatei>_autosave_<JJJJMMTT-HHMMSS>.jjcad
```

Das Manifest führt je Stand folgende Felder:

| Feld | Bedeutung |
| --- | --- |
| `zyklus` | Zyklusnummer des Standes; einziger Zeitanker für Entscheidungen |
| `datei` | Dokumentkennung, UUID aus dem Dateikopf, nicht der Pfad |
| `name` | Anzeigename |
| `hash` | SHA-256 des Standinhalts |
| `groesse` | Größe in Bytes |
| `zeit` | Nur Anzeige, Ortszeit |
| `gesichert` | true = der Stand entspricht dem Inhalt der Zieldatei, geschrieben nach dem Speichern |
| `verworfen` | true = die Arbeit wurde verworfen |
| `datei_hash` | SHA-256 der Zieldatei in dem Zustand, in dem sie beim Anlegen des Standes war; leer, wenn es keine Zieldatei gab. Grundlage der Konflikterkennung, weil der Standinhalt selbst den Pfad im Kopf trägt und deshalb nie byte-gleich zur Datei ist |
| `datei_name` | Dateiname im Pool, einzige Quelle für den Pfad |
| `pfad` | Zieldatei zum Zeitpunkt der Sicherung, leer bei nie gespeicherten Zeichnungen |

Das Manifest trägt `format „jjcad-sicherung“` und `version 2`. Fassung 1 kannte nur Fenster, Obergrenze und Dichtezone; sie wird beim Öffnen übernommen, fehlende Felder gelten als false beziehungsweise leer.

Namensregeln: Die Zieldatei wird klein geschrieben, ohne Umlaute und Sonderzeichen (Wohnhaus Musterstraße → `wohnhaus-musterstrasse`), und ohne den Namen `zeichnung`. Das Rückwärtsdatum JJJJMMTT-HHMMSS sorgt dafür, dass alphabetische Sortierung chronologisch ist. Fehlt der Zeitstempel, steht `zyklus-000000123`; eine Kollision in derselben Sekunde bekommt `-2`, `-3`. Altbestand `stand-<n>-<uuid>.jjcad` wird beim Öffnen umbenannt.

Kennung: Trägt eine Zeichnung im Kopf keine Kennung, wird beim Laden eine aus dem kanonischen Pfad abgeleitete vergeben (`daten::kennung_fuer_pfad`, UUID v5, unter Windows kleingeschrieben). Eine zufällige Kennung je Öffnen würde je Start einen neuen Pin anlegen.

Zeit: Die Zyklusnummer ist der einzige Zeitanker für Entscheidungen — monoton, DST-fest, unabhängig von der Systemuhr. Pin und Anker sind abgeleitet, nicht gespeichert; so können sie nicht veralten.

### 10. Regeln (Geometric Retention, Anhang A.2)

1. Sichern: n um 1 erhöhen, neuen Stand anlegen.
2. Fenster: Der älteste Stand einer Zeichnung wird gelöscht, solange sein jüngerer Nachbar mindestens T alt ist (der Anker rückt nach). Ist der Pin selbst mindestens T alt, wird auch er gelöscht, und die Zeichnung verlässt den Pool.
3. Obergrenze: max(N, 2F) Plätze, mit F = Zahl der Zeichnungen im Pool. Übersteigt der Pool sie, wird ausgedünnt.
4. Ausdünnen, bis der Pool die Obergrenze einhält: Kandidaten sind alle Stände außer Pin und Anker. Gelöscht wird der Kandidat mit den kleinsten Kosten.

   ```text
   Kosten(i) = (a_älter − a_jünger) / a_älter
   ```

   a_älter und a_jünger sind die Alter der beiden Nachbarn von i in derselben Zeichnung. Beide existieren immer, weil Pin und Anker geschützt sind. Die Kosten sind die relative Lücke, die nach dem Löschen zwischen den Nachbarn entstünde; gelöscht wird dort, wo sie am wenigsten auffällt. Nahe am Jetzt bleiben die Stände von selbst lückenlos, weil dort schon ein einzelner Zyklus eine große relative Lücke ist.

5. Monotonie: Es wird nur gelöscht, nie etwas wiederhergestellt oder neu berechnet. Nach einem Absturz genügt die aktuelle Liste, um weiterzumachen.

Eine Dichtezone gibt es nicht mehr. Ohne Schritt 2 bliebe der Anker ewig und die Auflösung sänke mit der Nutzungsdauer; Schritt 2 hält die Tiefe bei T. Zeichnungen ohne neue Stände verschwinden nach T aus dem Pool, verwaiste Zeichnungen belegen also keine Plätze. Weil n nur bei Arbeit zählt, läuft die Frist in Arbeitszeit, nicht in Kalenderzeit: eine Pause lässt nichts verfallen.

Beim Ausschalten des Verlaufs entsteht kein zusätzlicher Stand — der bisherige neueste je Zeichnung wird durch den neuen ersetzt. Vorhandene Stände werden nicht gelöscht; sie verfallen nur nach Schritt 2 und 3. Ein verworfen markierter Stand wird nie ersetzt.

### 11. Parameter

| Größe | Wert | Herkunft |
| --- | --- | --- |
| N | Vorgabe 150, Bereich 30 … 2000 | Einstellung „Anzahl der Sicherungen“ |
| T | Vorgabe 2 400 Zyklen (40 Arbeitsstunden bei 1 Minute) | Einstellung „Aufbewahrungszeitraum“, T = Arbeitsstunden × 60 / Intervall |
| I | Vorgabe 1 Minute, Bereich 1 … 240 | |
| F | Zahl der Zeichnungen im Pool | ergibt sich |
| Verlauf | an | Einstellung „Verlauf aufbewahren“ |

Abgeleitet:

```text
k(g) = 1/g + ln(g·T) / ln(1/(1−g))
g = Lösung von k(g) = N / F        (Bisektion, kein geschlossener Ausdruck)
Tiefe = mindestens T, höchstens T / (1 − g)
```

### 12. Ablauf eines Autosave

```text
wenn Inhalt geändert (Hash-Vergleich):
    n += 1
    Stand atomar schreiben             # temp → fsync → rename
    Pool-Eintrag anhängen, Manifest fsync
    ausdünnen(n)                       # Regeln 10.2 bis 10.4
```

Beim Beenden läuft derselbe Ablauf sofort für jeden Reiter mit ungespeicherten Änderungen, ohne Rücksicht auf das Intervall.

Ein Dateistand (nach „Speichern“) überspringt den Hash-Vergleich und wird immer geschrieben, damit er der neueste ist; er hält zugleich den Hash der Zieldatei fest.

„Verwerfen“ schreibt zuerst den verworfenen Arbeitsstand, dann den Dateistand.

Die Zieldatei wird dabei nicht angefasst. Ausgedünnt wird nach dem erfolgreichen Schreiben; die Spitze liegt kurzzeitig bei Plätze + 1. Waisen (Standdatei ohne Eintrag) und Reste abgebrochener Schreibvorgänge werden beim Start entfernt. Aufwand: O(N) je Autosave.

### 13. Abnahmekriterien

1. P1: Der Hash der Zieldatei ist vor und nach Autosave, Beenden und Start gleich (R1.2).
2. P2: Ändern, Beenden, Starten — der Reiter kommt mit Inhalt, altem Speicherort und als geändert zurück, die Zieldatei ist unverändert (R2.1, R2.2).
3. P3: Die Zieldatei extern ändern, dann wiederherstellen — es kommt die Konfliktabfrage (R2.3).
4. P4: Verwerfen — die Arbeit kehrt beim Start nicht zurück, ihr letzter Zustand liegt als verworfen markierter Stand vor (R3.5).
5. P5: SIGKILL beim Schreiben — keine halbe Datei, Verlust höchstens ein Intervall (R1.3, R3.1).
6. P6: Dauerbetrieb über viele Intervalle — die Zahl der Stände bleibt an oder unter der Obergrenze (R3.2).
7. P7: Eine Zeichnung über viele Intervalle nicht bearbeiten — der neueste Stand ist bis T noch da, danach ist die Zeichnung aus dem Pool verschwunden (R3.4).
8. P8: Eine Einstellung, die Stände löscht, verlangt eine Bestätigung mit Anzahl; vorher bleibt alles stehen (R3.6).
9. Stände im Sicherungsordner höchstens max(N, 2 × Zeichnungen), geprüft per Dateizählung.
10. Neuester Stand jeder Zeichnung nach jedem Autosave byte-identisch zum gesicherten Inhalt (Hash-Vergleich).
11. Pin und Anker überleben das Ausdünnen; zweimaliges Ausdünnen ändert nichts mehr.
12. Die Formel k(g) trifft die Tabelle aus Anhang A.4 und ist in beide Richtungen konsistent.
13. Standdateien heißen wie in Abschnitt 9 und kollidieren nicht.
14. Eine Einstellung ändern löscht nichts und wirkt nicht sofort; erst „Übernehmen“ speichert sie.
15. „Aufräumen“ löscht nur Stände mit vermerktem Pfad, deren Datei fehlt.
16. Der Testlauf schreibt keine Stände in den Datenordner des Benutzers.

### 14. Randfälle

| Fall | Verhalten |
| --- | --- |
| Zeichnung unverändert | Kein Zyklus, kein Stand |
| Beenden mit Änderungen | Sofort ein Stand, unabhängig vom Intervall |
| „Verwerfen“ beim Reiter | Letzter Arbeitsstand als verworfen markiert, danach Dateistand; die Arbeit kehrt nicht zurück |
| Zeichnung ausdrücklich geöffnet (Datei-Argument) | Der Stand hat Vorrang, der Reiter gilt als geändert; wurde die Datei extern geändert, gilt die Datei |
| Zieldatei extern geändert | Konfliktabfrage beim Start, bis dahin gilt die Datei |
| Uhrzeit, Zeitumstellung, Zeitsprung | Ohne Wirkung, gerechnet wird in Zyklen |
| Speichern unter, umbenennen | Stände folgen der Dokumentkennung, nicht dem Pfad |
| Zeichnung ohne Kennung im Kopf | Kennung aus dem Pfad abgeleitet, über Sitzungen stabil |
| Datei gelöscht | Der Pin bleibt bis T; „Aufräumen“ löscht Stände mit vermerktem, verschwundenem Pfad nach Rückfrage |
| N, T oder Intervall geändert | Gilt erst mit „Übernehmen“; kostet es Stände, kommt vorher die Warnung mit der Zahl |
| Verlauf aus | Je Zeichnung nur der neueste Stand; vorhandene Stände bleiben |
| Netzlaufwerk | Atomar über temporäre Datei und rename, keine Hardlinks |
| Absturz zwischen Stand und Manifest | Waisen werden beim Start entfernt |
| Zeichnung länger als T nicht bearbeitet | Pin und Verlauf verfallen, die Zeichnung belegt keine Plätze mehr |

### 15. Offene Punkte

1. Pin-Tiefe: derzeit ein Stand je Zeichnung; zwei kosten einen Platz je Zeichnung.
2. Zählt Leerlauf als Zyklus? Dieses Konzept sagt nein.
3. Verworfene Stände sind nur über den Ordner und die Zahl in den Einstellungen sichtbar; eine eigene Liste mit Wiederherstellen-Knopf fehlt.
4. Große Zeichnungen (Bild, Video): Stände sind vollständige Fassungen. Unter etwa 20 Ständen je aktive Zeichnung lohnt das Verfahren kaum; dann Intervall erhöhen, T verkleinern oder Stände als Deltas speichern — das widerspricht aber der Anforderung, dass ein Stand im Format der Zieldatei vorliegt.
