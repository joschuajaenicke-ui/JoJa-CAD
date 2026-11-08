# JoJa-CAD — Sicherung: Stände und Aufbewahrung

**Stand: umgesetzt (T0129).** Diese Datei beschreibt, was der Anwender von der
Sicherung sieht und was bei der Umsetzung einzuhalten ist. Herleitung,
Messungen und Simulationscode folgen als Anhang.

Die Sicherung legt während der Arbeit automatisch Stände an und dünnt sie nach
Alter aus: vorne dicht (jedes Intervall), hinten grob (bis zu einem Quartal
Arbeit). So bleibt der Speicherbedarf fest, obwohl der Zeitraum weit
zurückreicht.

**Die Sicherung arbeitet getrennt vom Speichern und schreibt nie in die
Zieldatei.** Die Zeichnung speichert ausschließlich der Benutzer (Strg+S /
Speichern unter). Die Sicherung greift nur, wenn er es vergisst oder etwas
schiefgeht.

---

## Teil A — Bedienung

### 1. Was automatisch passiert

- Ändert sich die Zeichnung, wird **einmal je Intervall** ein Stand gesichert
  (Vorgabe 1 Minute, einstellbar — für Netzlaufwerke ist 1 Minute zu hoch).
- Ohne Änderung wird nichts gesichert — Pausen und reines Betrachten kosten
  weder Plätze noch Zeit.
- Gesichert wird in den **Pool** im Datenordner. Die **Arbeitskopie** ist der
  neueste Stand eines Dokuments im Pool; sie wird atomar geschrieben
  (temp → fsync → rename). Bei einem Absturz ist höchstens das letzte
  Intervall verloren.
- Die **Zieldatei** (die `.jjcad`, die der Benutzer geöffnet hat) wird von der
  Sicherung **nie** angefasst. Sie ändert sich nur durch „Speichern“.

Ein **Zyklus** ist ein solcher Autosave mit Änderung. Alle Angaben in dieser
Doku (Reichweite, Abstand der Stände) rechnen in Zyklen, nicht in
Kalenderzeit.

### 2. Die Einstellungen

Einstellungen → Sicherung:

| Einstellung | Bedeutung | Vorgabe |
|---|---|---|
| **Verlauf sichern** | an = Stände werden aufbewahrt und ausgedünnt; aus = nur der neueste Stand je Dokument (Pins bleiben) | an |
| **Plätze im Pool** | wie viele Stände **insgesamt** aufbewahrt werden — für alle Dateien zusammen | 100 (30 … 2000) |
| **Reicht zurück** | wie viel Arbeit rekonstruierbar sein soll | Quartal |
| **Intervall** | Abstand zwischen zwei Sicherungen in Minuten | 1 (1 … 240) |

„Reicht zurück“ ist in **Arbeitsstunden** angegeben, nicht in Kalendertagen:

| Auswahl | Arbeitsstunden | Zyklen bei 1 min | bei Dauerbetrieb |
|---|---|---|---|
| Woche | 40 | 2 400 | 5 Arbeitstage |
| Monat | 160 | 9 600 | 20 Arbeitstage |
| **Quartal** | **480** | **28 800** | **60 Arbeitstage** |
| Jahr | 1 900 | 114 000 | 238 Arbeitstage |

Die Zahl der Zyklen folgt aus Arbeitsstunden und Intervall:

```
T = Arbeitsstunden × 60 / Intervall
```

Bei 5 Minuten Intervall sind es für ein Quartal also 5 760 Zyklen statt
28 800 — die Reichweite in Arbeit bleibt gleich, der Speicherbedarf sinkt.

### 3. Die dritte Zahl: Abstand der Stände

Der Dialog zeigt sie an, einstellen kann man sie nicht:

> **Abstand der Stände: 8,5 %** — zwei benachbarte Stände liegen im Mittel
> 8,5 % ihres Alters auseinander: bei einer Stunde Rückstand ~5 Minuten, bei
> einem Arbeitstag ~41 Minuten, bei einem Monat ~1,7 Arbeitstage.

Der Abstand folgt aus den Einstellungen und der Zahl der Dateien:

```
Abstand = 1 − (D / T) ^ (1 / (Plätze − Dateien))        mit D = 10
```

Bei vielen Dateien wird er gröber, weil jede Datei einen Platz belegt
(Abschnitt 5).

### 4. Was garantiert ist

1. **Die letzten 10 Zyklen** liegen lückenlos vor (Dichtezone,
   `D = 10` Zyklen = 10 × Intervall).
2. **Jede Datei** hat immer ihren letzten Stand — auch nach Monaten Ruhezeit.
3. Kein Verlaufsstand ist älter als „Reicht zurück“.
4. Der Speicherbedarf ist fest: **Plätze × Dateigröße**, unabhängig davon, wie
   viele Dateien im Projekt liegen (bis Plätze = Dateien, siehe 5).

### 5. Viele Dateien

Jede Datei belegt einen Platz (ihren letzten Stand). Bleiben zu wenige Plätze
für den Verlauf, wird der Abstand grob; reicht es gar nicht, hebt das Programm
die Plätze selbst auf die Zahl der Dateien an und sagt es im Dialog.

| Dateien | Plätze | Abstand bei Quartal | Empfehlung |
|---|---|---|---|
| 10 | 100 | 8,5 % | Vorgabe passt |
| 20 | 100 | 9,5 % | Plätze 105 für 9 % |
| 50 | 100 | 14,7 % | Plätze 126 für 10 % |
| 200 | 100 | kein Verlauf | Plätze 276 für 10 % |

Faustformel für die Plätze: **Dateien + 90** ergibt bei Quartal etwa 8,5 %.

Der Verlauf gehört der Datei, an der gearbeitet wird: Bei 200 Dateien und 290
Plätzen haben die meistbenutzten 5 Stände, die übrigen 1. Eine ruhende Datei
behält nur ihren letzten Stand — ihr Verlauf verfällt (nach 1 000 Zyklen anderer
Arbeit sind es noch ~22 von 100 Ständen, nach 10 000 noch 4).

### 6. Wiederherstellen

Der Dialog zeigt eine Liste der Stände, neueste zuerst: Datum, Uhrzeit, Größe
und Vorschau. Die letzten zehn im Intervallraster, danach in wachsenden
Abständen. Ein Klick öffnet den Stand als **neues Dokument**; die Zieldatei und
der bisherige Stand bleiben unberührt. Gespeichert wird das neue Dokument erst
durch „Speichern unter“.

### 7. Was die Sicherung nicht ist

- **Kein Archiv.** Nach „Reicht zurück“ ist ein Verlaufsstand endgültig weg.
  Dauerhafte Meilensteine („Stand bei Baueingabe“) sind eine eigene Funktion.
- **Keine Sicherung gegen Verlust des Ordners.** Platte defekt, Ordner gelöscht
  oder verschlüsselt: dagegen hilft nur eine Kopie außerhalb des Rechners. Für
  Schutz gegen einen Plattenausfall gehört der Pool auf ein **Netzlaufwerk mit
  redundanter Sicherung** (Einstellungen → Speicherorte → Sicherungen).

---

## Teil B — Umsetzung

### 8. Begriffe

| Begriff | Bedeutung |
|---|---|
| **Zyklus** | Autosave mit Änderung; globaler Zähler `n` über alle Dateien |
| **Stand** | gesicherte Fassung einer Datei, mit Zyklusnummer und Hash |
| **Pool** | alle Stände eines Projektordners zusammen |
| **Arbeitskopie** | neuester Stand eines Dokuments im Pool; wird atomar geschrieben |
| **Pin** | neuester Stand je Datei; wird nie gelöscht und verfällt nicht |
| **Dichtezone** | die 10 neuesten Stände des Pools; nie Löschkandidat |
| **Verlauf** | alle Stände außer Pins und Dichtezone; wird ausgedünnt |
| **Alter** | `a = n − Zyklus` eines Standes, in Zyklen |

### 9. Datenmodell

**Ein Pool für alle Dateien** — ein Projektordner, kein Gruppenfeld. Sonst kann
nicht dateiübergreifend ausgedünnt werden. Je Stand: Zyklusnummer,
Datei-Kennung (UUID aus dem Dateikopf, nicht der Pfad), Hash, Größe,
Zeitstempel (nur Anzeige).

```
<Sicherungen>/manifest.json          Zähler n und die Liste der Stände
<Sicherungen>/stand-<n>-<uuid>.jjcad eine Datei je Stand
```

Die Zyklusnummer ist der einzige Zeitanker für Entscheidungen: monoton,
DST-fest, unabhängig von der Systemuhr. Der Pin ist **abgeleitet** (neuester
Stand je UUID), nicht gespeichert — so kann er nicht veralten.

### 10. Regeln

1. **Sichern:** `n += 1`, Stand anhängen, dann ausdünnen.
2. **Fenster:** Stände mit `Alter > T` werden gelöscht — **außer Pins**.
3. **Obergrenze:** `Plätze = max(N, Anzahl Dateien)`.
4. **Ausdünnen**, solange der Pool voll ist: Kandidaten sind alle Stände außer
   Pins und Dichtezone. Gelöscht wird der Kandidat mit den **kleinsten Kosten**

   ```
   Kosten(i) = (Alter[i+1] − Alter[i−1]) / Alter[i+1]
   ```

   wobei das Fensterende `T` als oberer Nachbar zählt. Kleine Kosten heißt:
   dort ist der Abstand nach dem Löschen am größten.
5. **Verlauf aus:** alle Stände außer Pins werden gelöscht — je Dokument bleibt
   genau der neueste.

Die Regel ist **monoton**: es wird nur gelöscht, nie wiederhergestellt. Sie
braucht keinen Verlauf außer der aktuellen Liste und ist damit nach einem
Absturz einfach fortsetzbar.

### 11. Parameter

| Größe | Wert | Herkunft |
|---|---|---|
| `N` | Vorgabe 100, Bereich 30 … 2000 | Einstellung „Plätze im Pool“ |
| `T` | Vorgabe 28 800 Zyklen | „Reicht zurück“ (Arbeitsstunden × 60) / Intervall |
| `D` | 10 | fest, Dichtezone (10 × Intervall) |
| `Verlauf` | an | Einstellung „Verlauf sichern“ |

Abgeleitet:

```
r = (T / D) ^ (1 / (N − Dateien))        Wachstumsfaktor der Altersstufen
g = 1 − 1 / r                             Abstand der Stände (relative Lücke)
N nötig = Dateien + ceil( ln(T/D) / ln(1/(1−g)) )
```

### 12. Ablauf eines Autosave

```
wenn Inhalt geändert (Hash-Vergleich):
    n += 1
    Stand atomar schreiben             # temp → fsync → rename
    Pool-Eintrag anhängen, Manifest fsync
    ausdünnen(n)                       # Regeln 10.2–10.5
```

Die Zieldatei wird dabei **nicht** angefasst. Ausgedünnt wird **nach** dem
erfolgreichen Schreiben; die Spitze liegt kurzzeitig bei `Plätze + 1`. Waisen
(Stand ohne Eintrag) werden beim Start entfernt. Aufwand: `O(N)` je Autosave.

### 13. Abnahmekriterien

1. Stände im Sicherungsordner ≤ `max(N, Dateien)`, geprüft per Dateizählung.
2. Neuester Stand jeder Datei ist nach jedem Autosave byte-identisch zum
   gesicherten Inhalt (Hash-Vergleich).
3. Jede Datei hat mindestens einen Stand — auch nach dem Dreifachen von `T`
   ohne Bearbeitung.
4. Kein Stand außer Pins ist älter als `T` (geprüft über die Zyklusnummer, nicht
   über das Dateidatum).
5. `kill -9` mitten im Schreiben hinterlässt keine halbe Datei und keinen Pool
   über der Obergrenze.
6. Die letzten `D` Stände liegen lückenlos vor.
7. Bei 5, 20 und 50 Dateien ist die Zahl der Stände gleich (Speicher skaliert
   nicht mit der Dateizahl).
8. Die Zieldatei bleibt bei jedem Autosave unverändert (Hash vor/nach).
9. „Verlauf sichern“ aus → genau ein Stand je Dokument.
10. Ein wiederhergestellter Stand öffnet ein **neues** Dokument; die Zieldatei
    bleibt unverändert.

### 14. Randfälle

| Fall | Verhalten |
|---|---|
| Datei unverändert | kein Zyklus, kein Stand |
| Uhrzeit/DST/Zeitsprung | ohne Wirkung — gerechnet wird in Zyklen |
| Speichern unter / umbenennen | Stände folgen der Datei-UUID, nicht dem Pfad |
| Datei gelöscht | Pin bleibt, bis die Datei 30 Tage unbekannt ist |
| `N`, `T` oder Intervall geändert | gilt ab der nächsten Ausdünnung; bestehende Stände behalten ihre Frist |
| Verlauf aus | nur der neueste Stand je Dokument bleibt |
| Netzlaufwerk | Kopie + fsync + rename, keine Hardlinks |
| Absturz zwischen Stand und Manifest | Waisen werden beim Start entfernt |

### 15. Offene Entscheidungen

1. Vorgabe für `N`: fest 100 oder „automatisch = Dateien + 90“.
2. Pin-Tiefe: derzeit 1 Stand je Datei; 2 kostet einen Platz je Datei.
3. Zählt Leerlauf als Zyklus? Dieses Konzept sagt nein.
