# PDF-Import — Plan für die Umsetzung in einem Rutsch

**Ziel:** Ein PDF wird als **ein Element** (ganzes Blatt) eingefügt und lässt sich
in seine Bestandteile **zerlegen**. Übernommen werden Text, Linien und Flächen —
soweit PDF sie hergibt. Alles andere wird gezählt und gemeldet, nicht stillschweigend
verschluckt.

**Abnahmekriterium (verbindlich):**
Export als PDF → wieder importieren → zerlegen → **der Inhalt sieht identisch aus**.
Rundung in PDF-Koordinaten ist erlaubt; Toleranzen stehen in Abschnitt 7.

**Startbefehl für eine frische Sitzung** (wörtlich einfügbar):

> Lies `docs/PDF-IMPORT.md` vollständig und setze die Etappen 0–6 um. Arbeite die
> Etappen durch, ohne nach jeder Etappe zu fragen; melde nur Abweichungen vom Plan,
> fehlende Entscheidungen und am Ende einen knappen Bericht mit den Messwerten aus
> Abschnitt 7. Ein Ticket, ein Zweig, eine Auslieferung am Ende.

---

## 1. Warum das günstig liegt: was schon existiert

Der Import ist zu drei Vierteln „nur" ein Leser. Die Zielstruktur steht bereits:

| Baustein | Ort | Bedeutung für den Plan |
|---|---|---|
| `EntityKind::Line/Polyline/Circle/Arc/Ellipse/Fill/Text` | `core/src/model.rs` | Deckt die PDF-Primitive m/l/c/re/S/f ab, nichts Neues nötig |
| `Shape::Fill { outer, holes }` + Even-Odd | `core/src/render.rs:1100` | PDF-Füllregel passt 1:1 |
| `EntityKind::BlockRef` + Blockdefinition | `model.rs`, `actions.rs:2217` | **Das Blatt-Element** ist ein BlockRef |
| `modify.explode` / `block.explode` | `actions.rs:174`, `explode_entity` 3084 | Zerlegen rendert die Entity in die Displayliste und baut daraus Einzelentitäten — inkl. `Shape::Fill` (3131) und `Shape::Text` (3147) |
| `Affine`-Transformation im `render_entity` | `core/src/render.rs` | Skalierung/Drehung des Blattes beim Zerlegen ist erledigt |
| Bogen-/Spline-Abflachung | `tessellate.rs`, `geom.rs:955` | Kurven aus `c`/`v`/`y` müssen nicht neu erfunden werden |
| `dxf/import.rs` | 700+ Zeilen | Muster für einen Importer (Fremdrecords → Entities, Blockauflösung, Fehlerliste) |

**Folge:** „Zerlegen" ist geschenkt, sobald das Blatt ein BlockRef auf einen Block mit
echten Entities ist. Die ganze Arbeit steckt im **PDF-Lesen**.

## 2. Abhängigkeit: Textformatierung

Text-Round-Trip ist nur so gut wie das Textmodell. Heute kennt `EntityKind::Text` nur
`bold: bool`. Der PDF-Export wird (laut Textkonzept) Runs mit Fett/Kursiv/Unterstrichen/
Hochstellung und eingebettete Schriften schreiben.

- **Ist das Run-Modell schon da** → Text wird über Runs importiert, alles identisch.
- **Noch nicht** → Import erzeugt `bold`-only-Texte; Kursiv/Unterstrichen/Hochstellung
  gehen beim Re-Import verloren. Das Kriterium „identisch" ist dann für Text **nicht**
  erfüllt. Dann im Bericht klar benennen, nicht schönreden.

Empfehlung: Textkonzept (Runs + Schrifteinbettung) zuerst, PDF-Import danach. Der
PDF-Import soll aber **nicht** darauf warten — Etappe 1–3 sind unabhängig.

## 3. Technischer Entwurf

### 3.1 Abhängigkeit

```toml
# crates/jjcad-core/Cargo.toml
lopdf = { version = "0.46", default-features = false }
```

Gemessen (gleiches Release-Profil, `strip`, `lto=thin`, `codegen-units=1`):
**+881 KB roh, +363 KB zstd** — also ~+9 % Binary, ~+0,36 MB Download.

**MSRV:** lopdf 0.46 verlangt rustc 1.88, der Workspace deklariert `1.80`. Installiert
ist 1.99 → `rust-version = "1.88"` im Workspace anheben (eine Zeile) **oder** lopdf auf
0.3x pinnen. Nicht beides halb.

FlateDecode ist bei lopdf 0.46 harte Abhängigkeit (kein Feature) — echte PDFs sind fast
immer komprimiert, das muss drin sein. Nach dem Bauen prüfen: `cargo tree -p jjcad-core | grep flate2`.

### 3.2 Pipeline

```
Datei → lopdf::Document
      → Seite(n): MediaBox/CropBox, /Rotate, /Resources (Font, XObject, ExtGState)
      → Content-Stream(s) (dekodiert, ggf. mehrere, in Reihenfolge)
      → Tokenizer (Operanden + Operatoren)
      → Interpreter (Grafikzustand, CTM, Pfad, Textzustand)
      → Primitivstrom (Pfad/Strich/Füllung/Textlauf in Welt-mm)
      → Entities → Block "<Dateiname> S.<n>" + BlockRef auf Lageblatt
```

Neue Dateien: `core/src/pdf_read/{mod,token,interp,path,font,style}.rs`,
`core/src/pdf_read.rs` als Fassade. `pdf.rs` (Schreiber) bleibt unangetastet.

### 3.3 Grafikzustand

Stapel für `q`/`Q`. Felder: `ctm: Affine`, `line_width` (Papier-pt), `dash`,
`stroke: Color`, `fill: Color`, `font: FontSel`, `font_size`, `char_space`,
`word_space`, `h_scale`, `leading`, `rise`, `fill_rule` (nonzero/even-odd),
`clip: Option<Vec<Vec2>>` (nur zählen, siehe 6).

Operatoren: `cm`, `w`, `d`, `J`, `j`, `M`, `rg`, `RG`, `g`, `G`, `k`, `K`, `sc`, `scn`,
`SC`, `SCN`, `gs` (nur `/ca`/`CA` lesen → Warnung), `q`, `Q`, `re`, `m`, `l`, `c`, `v`,
`y`, `h`, `S`, `s`, `f`, `F`, `f*`, `B`, `B*`, `b`, `b*`, `n`, `W`, `W*`, `BT`, `ET`,
`Tf`, `Td`, `TD`, `Tm`, `T*`, `TL`, `Tc`, `Tw`, `Tz`, `Ts`, `Tj`, `TJ`, `'`, `"`, `Do`.

### 3.4 Pfad → Entities

| PDF | JoJa |
|---|---|
| `m`+`l`+`S` mit genau 2 Punkten | `Line` |
| `m`+`l`…+`S`/`s` | `Polyline { closed }` |
| `c`, `v`, `y` | abgeflacht → `Polyline`; **Kreiserkennung**: 3 Punkte + konstanter Radius (Toleranz 1 %) → `Arc`/`Circle` |
| `re` | geschlossene 4-Punkt-`Polyline` (Umriss) |
| `f`/`F`/`f*`/`B`/`B*`/`b`/`b*` | `Fill { boundary, holes, pattern: Solid, border: <B…> }` |

**Abflachtoleranz in Welt-mm**, nicht in Papier-pt: 0,15 mm auf dem Papier sind bei
1:100 fünfzehn Millimeter in der Zeichnung. Die Toleranz muss deshalb aus dem Maßstab
abgeleitet werden (`tolerance_world = 0.15 mm / scale`) oder fest 0,05 mm Welt — sonst
wird zerlegte Geometrie sichtbar eckig. **Das ist der Punkt, an dem ein naiver
Importer scheitert.**

### 3.5 Farbe, Linienstärke, Strichart

- Farbe: PDF 0…1 float → `Color { r,g,b: u8 }` (`(v*255).round()`). CMYK über die
  übliche Näherung `r=255(1-c)(1-k)` usw., Graustufen `rgba(v,v,v)`.
- Linienstärke: `w` in pt → mm ×0,352778, auf ≥0,05 begrenzen (wie `explode_entity`).
- `d`-Muster: Vergleich gegen die vorhandenen `LineType`-Muster (durchgezogen,
  gestrichelt, gepunktet, Strichpunkt). Kein Treffer → durchgezogen + Warnung zählen.

### 3.6 Text — der schwierige Teil

Textzustand: `Tm`/`Tlm`, `Tf`, `Ts` (Rise), `Tz` (Breite), `Tc`/`Tw` (Abstände),
`TL`/`T*` (Zeilenwechsel). Endposition eines Zeichens = `Tm · (Tf-Größe, Rise)`.

**Wir müssen beide Richtungen können:**

1. **Unser eigenes PDF** (Ziel des Abnahmekriteriums): `/Subtype /TrueType` mit
   `/FontFile2` — die eingebettete Teilmenge mit `ab_glyph` laden und **exakt** damit
   setzen. Das ist der Grund, warum die Textformatierung zuerst Schriften einbettet:
   dann ist der Rückweg bitgenau.
2. **Fremde PDFs, Base-14** (`/BaseFont /Helvetica|Helvetica-Bold|Helvetica-Oblique|
   Times-*|Courier-*`): auf die eingebetteten Familien abbilden. Liberation Sans ist
   metrisch identisch mit Helvetica → Lage und Breite stimmen.
3. **Fremde PDFs, Type0/CID (Identity-H)**: `/ToUnicode`-CMap lesen (Code → Unicode).
   Fehlt sie: Glyphen direkt aus dem eingebetteten CIDFontType2 setzen → **sieht richtig
   aus, ist aber kein Text mehr** (nur Konturen). Das ist der ehrliche Fallback; im
   Bericht als „N Textläufe ohne Zeichenzuordnung" ausweisen.

**Run-Zusammenfassung:** aufeinanderfolgende `Tj`/`TJ` mit gleicher Grundlinie, gleicher
Größe und Lücke < 0,6 × Größe werden zu **einem** `Text` zusammengefasst; Wechsel von
Größe/Rise/Face erzeugen Runs (Hochstellung, Fett/Kursiv). Ohne diese Zusammenfassung
entstehen pro Wort eigene Textentitäten — optisch identisch, aber unbrauchbar.

`TJ`-Zahlen sind Kerning in Tausendstel Em: Verschiebung `-n/1000 × Tf × Tz`. Wer sie
ignoriert, bekommt auseinandergezogene Wörter.

### 3.7 Einfügen als ein Element

- Action `file.import.pdf` mit Parametern `path`, `page` (1-basiert), `scale`
  (Standard 1:1 = Papier-maßstäblich), `layer`, `as_block` (Standard `true`).
- Blockname `"PDF <Dateiname> S.<n>"`; der BlockRef sitzt am Zeichnungsnullpunkt des
  Lageblatts, Skalierung 1,0, keine Drehung.
- Neue Ebene `PDF-Import` (Farbe wie importierte Objekte), damit der Nutzer sie
  ausblenden/sperren kann.
- Seitenmaß: `MediaBox` − untere linke Ecke, × 0,352778 → mm; A3 = 420 × 297 mm.
- GUI: Dateidialog über das vorhandene `rfd` (`systemdialog.rs`), Seitenauswahl,
  danach Hinweiszeile „N Objekte übernommen, M nicht darstellbar".
- MCP: dieselbe Action, damit der Import kopflos prüfbar ist (Projektregel: eine
  Befehlsschicht).

## 4. Was nicht geht (und wie es gemeldet wird)

| Feature | v1 |
|---|---|
| Bilder (`/Subtype /Image`) | **nicht** übernommen, gezählt und gemeldet |
| Transparenz (`/ca`, `/CA`, Softmasken, `/SMask`) | ignoriert, gezählt |
| Schattierungen (`sh`, `/Shading`) | ignoriert, gezählt |
| Muster (`/Pattern`) | ignoriert, gezählt |
| Clipping (`W`/`W*`) | ignoriert, gezählt — Folgen: überlaufende Muster/Kacheln |
| Formulare, Anmerkungen, OCG-Ebenen | ignoriert |
| Verschlüsselte PDFs | klare Fehlermeldung, kein stiller Teilimport |

Nach dem Import eine Warnliste in der Konsole und als Action-Ergebnis
(`{"ok": true, "created": […], "warnings": [{"art": "Bild", "anzahl": 3}]}`).

## 5. Etappen (in dieser Reihenfolge, ohne Zwischenfrage)

| # | Inhalt | Dateien | Fertig wenn |
|---|---|---|---|
| **0** | Vorprüfung | — | `explode_entity` mit `Shape::Fill` geprüft (pattern/border erhalten?); `cargo test --workspace` grün als Ausgangsstand notiert |
| **1** | Leser-Kern | `pdf_read/{token,interp,path}.rs` | `samples/Grundriss.pdf` liefert Linien/Flächen/Text in Welt-mm; Unit-Tests mit handgeschriebenen Content-Streams (`m/l/S`, `re f`, `q/Q/cm`, `d`, `w`) |
| **2** | Text | `pdf_read/{font,text}.rs` | Grundriss-PDF: Texte kommen als `Text`-Entities mit richtiger Lage/Höhe zurück, Umlaute korrekt (`Straße`) |
| **3** | Block + Action | `actions.rs`, `pdf_read.rs` | `file.import.pdf` erzeugt Block + BlockRef; `modify.explode` darauf erzeugt Einzelentitäten |
| **4** | GUI + Warnliste | `jjcad-gui/src/*` | Datei-Dialog, Seitenauswahl, Hinweiszeile; Import über MCP genauso möglich |
| **5** | Abnahme-Harness | `crates/jjcad-core/tests/pdf_roundtrip.rs`, `scripts/pdf-roundtrip.sh` | Kriterien aus Abschnitt 7 automatisch geprüft, Ergebnis als Textbericht |
| **6** | Härtung | — | Fremd-PDF-Proben (aus LibreOffice/Inkscape/Scanner-Export), Fehlerfälle, Bericht |

## 6. Abnahme-Harness (Abschnitt 7 macht es messbar)

`crates/jjcad-core/tests/pdf_roundtrip.rs`:

1. `samples/Wohnhaus.jjcad` laden, PDF nach `$TMPDIR/rt.pdf` exportieren.
2. PDF importieren (`file.import.pdf`, `as_block: true`), BlockRef zerlegen
   (`modify.explode`).
3. **Vergleich A — Struktur:** Anzahl Entities je Typ, Bounding-Box des Ganzen.
4. **Vergleich B — Text:** Liste der Textinhalte (Strings sortiert) identisch;
   Höhe ±0,05 mm, Lage ±0,3 mm.
5. **Vergleich C — Pixel:** beide Zustände (Original / re-importiert+zerlegt) mit dem
   eigenen Renderer als PNG in gleicher Auflösung rendern, Byte-Differenz zählen.
   Kein neues Test-Crate: `tiny-skia`-Pixmaps liegen schon vor.
6. Zusätzlich außerhalb des Tests: `pdftotext rt.pdf -` muss die Raumnamen enthalten,
   `pdffonts` die eingebetteten Schriften zeigen.

Toleranzen (PDF rundet, deshalb):

- Bounding-Box: **±0,5 mm**
- Textlage: **±0,3 mm**, Höhe **±0,05 mm**
- Entity-Anzahl: **±10 %** (siehe Fallstrick 5 — nicht Anzahl, sondern Bild zählt)
- Pixel-Differenz bei 300 dpi: **≤0,5 %** der Pixel, kein zusammenhängender
  Fehlbereich > 1 mm²

## 7. Fallstricke (aus dem Code und aus PDF-Erfahrung)

1. **Abflachtoleranz im Weltmaß** — sonst eckige Kurven bei 1:100 (Abschnitt 3.4).
2. **`TJ`-Kerning** ignorieren verschiebt Wörter.
3. **`Tz` (horizontale Skalierung)** verändert Breiten; entweder als Faktor übernehmen
   oder Text als Warnung markieren — nicht stillschweigend weglassen.
4. **`f` vs. `f*`**: bei nonzero entscheidet der Umlaufsinn über Löcher. Ringe nach
   Fläche sortieren und Enthaltensein per Punkt-in-Polygon prüfen → `outer` + `holes`.
5. **Strichweise Duplikate**: PDF zeichnet Rechtecke oft als vier einzelne `m/l/S`.
   Nach dem Import sind das vier `Line` statt einer `Polyline` — die Entity-Anzahl
   steigt, das Bild bleibt gleich. Deshalb Pixel-Vergleich als Hauptkriterium, nicht
   Anzahl. (Optional in Etappe 6: kollineare Nachbarlinien zusammenfassen.)
6. **Eigenes PDF hat keine `/Widths`** (`pdf.rs` schreibt sie nicht). Der Importer darf
   sich nicht darauf verlassen — Vorschübe aus der geladenen Schrift nehmen.
7. **`/Rotate` und CropBox**: Seite kann gedreht und beschnitten sein; erst CropBox,
   dann Drehung, dann Skalierung anwenden.
8. **Nullpunkt**: `MediaBox`-Ursprung liegt selten bei (0,0) — untere linke Ecke
   abziehen, sonst sitzt das Blatt neben dem Zeichnungsnullpunkt.
9. **Mehrere Content-Streams** je Seite (`/Contents [.. ..]`) müssen konkateniert
   werden, ohne Zustand zurückzusetzen.
10. **Form-XObjects** (`Do` mit `/Subtype /Form`): Matrix × CTM, dann Rekursion in den
    eigenen Stream. Kommt in Fremd-PDFs häufig vor, ist billig (ca. 40 Zeilen) — mit
    Tiefenbegrenzung gegen Endlosschleifen.
11. **Dateigröße**: ein importiertes A3-Blatt als Block kann 5.000–50.000 Entities
    bedeuten. Obergrenze je Seite (z. B. 50.000) mit klarer Meldung, sonst hängt die
    GUI. Block bleibt trotzdem gespeichert (für Zerlegen nötig) — Größe der `.jjcad`
    im Bericht nennen.
12. **Nebenfund am Rande**: `samples/Grundriss.pdf` hat im Info-Dictionary
    `/Title (Wohnhaus ? Blatt 1)` — ein Zeichen außerhalb WinAnsi wurde zu `?`. Beim
    PDF-Feinschliff mit erledigen (Titel als UTF-16BE-String mit BOM schreiben).

## 8. Definition of Done

- `cargo test --workspace` grün (Ausgangsstand 502 Tests + neue).
- `cargo build --release` ohne Warnungen im neuen Code.
- Round-Trip-Test grün mit den Zahlen aus Abschnitt 6 im Testprotokoll.
- Import von `samples/Grundriss.pdf` in der GUI sichtbar korrekt, Zerlegen liefert
  wählbare Einzelobjekte.
- Warnliste funktioniert (ein PDF mit Bild → Meldung „Bild nicht übernommen").
- Auslieferung: `./scripts/ship-card.sh <ticket>`; Bericht mit: gemessene Toleranzen,
  Pixel-Differenz, Binary-Größe vorher/nachher, Zahl der Entities, offene Punkte.

---

## 9. Umsetzungsstand (T0031)

Alle Etappen 0–6 sind umgesetzt. Die Zahlen stammen aus
`./scripts/pdf-roundtrip.sh` (Testprotokoll) und sind reproduzierbar.

### Messwerte der Abnahme

| Kriterium | Grenze | Gemessen |
|---|---|---|
| Pixel-Differenz (4 px/mm, A3) | ≤ 0,5 % | **0,03 %** (627 von 1.995.840) |
| Textinhalte | identisch | **19 von 19** |
| Texthöhe | ± 0,05 mm | eingehalten (Test bricht sonst ab) |
| Textlage (Umriss) | ± 0,3 mm | eingehalten |
| Objektzahl | ± 10 % (nicht maßgeblich) | 36 → 823 (Schraffuren/Muster als Einzelstriche) |
| Schriften | eingebettet | `pdffonts`: LiberationSans + -Bold, `emb yes`, `uni yes` |
| Titel | UTF-16 mit BOM | `Wohnhaus — Blatt 1` (vorher `Wohnhaus ? Blatt 1`) |
| Dateigröße | — | 437 KB (A3-Blatt, zwei eingebettete Schnitte) |

### Abweichungen vom Plan (bewusst, mit Begründung)

1. **MSRV 1.88** statt 1.80 — `lopdf` 0.46 verlangt es (Plan 3.1 erlaubt den
   Weg ausdrücklich).
2. **Tokenizer:** `pdf_read/token.rs` legt eine eigene, schmale Schnittstelle
   (`Operand`/`Op`) vor den Inhaltsparser von `lopdf` (`Content::decode_iter`).
   Ein eigener Parser wäre eine zweite Fehlerquelle ohne Gewinn.
3. **Keine Schrift-Teilmenge:** eingebettet wird die vollständige TrueType-
   Datei des benutzten Schnitts (`/FontFile2`), nicht eine Teilmenge. Eine
   echte Teilmenge müsste `cmap`, `loca` und `glyf` neu schreiben — dafür
   steht der Aufwand in keinem Verhältnis. Folge: rund 350 KB je **benutztem**
   Schnitt (nicht benutzte werden weggelassen).
4. **`Tz` (horizontale Skalierung)** wird auf 1 normalisiert und als Warnung
   gezählt — eine Breitenskalierung hat im Textmodell keine Entsprechung.
   `Tc`/`Tw` fließen in die Position ein, nicht in die Läufe.
5. **`TJ`-Kerning** wird für die Position übernommen (Textmatrix), aber nicht
   als Auszeichnung gespeichert: ein fremder Lauf mit starkem Kerning kann
   dadurch wenige Zehntelmillimeter anders laufen. Unser eigener Export
   schreibt kein `TJ`, der Rundlauf ist davon nicht betroffen.
6. **Unterstreichung:** PDF kennt sie nicht. Der Export schreibt ein dünnes
   Viereck 12 % unter der Grundlinie (6 % der Größe dick), der Import erkennt
   es wieder und macht daraus die Auszeichnung — und zwar nur für die Läufe,
   die über dem Viereck liegen.
7. **Maßstab der Seite: Vorgabe ist der Zeichnungsmaßstab** (`settings.annotation_scale`,
   Standard 1:100), nicht 1:1 wie in Abschnitt 3.7 vorgesehen. Grund: Linien sind
   in JoJa-CAD **Papier-Millimeter** und werden bei der Anzeige mit dem
   Zeichnungsmaßstab gerechnet. Ein Blatt in Papier-Millimetern (420 mm breit)
   hätte in einer 1:100-Zeichnung 100-fach zu dicke Striche. Mit dem
   Zeichnungsmaßstab liegt das eingefügte A3-Blatt bei 42,0 × 29,7 m und
   erscheint gedruckt genau so groß wie das Original. `scale: 1` importiert
   maßstäblich in Papier-Millimetern (so prüft es der Rundlauf-Test).
8. **Zerlegen verliert `Fill.pattern`/`Fill.border`** (Etappe 0 geprüft):
   Schraffuren liegen als eigene Musterlinien in der Displayliste, die
   Umrandung als eigene Linien. Das Bild bleibt gleich, nur die Objektzahl
   wächst.

### Was der Leser kann (Etappe 6)

- Base-14-Schriften (Helvetica/Times/Courier) mit `WinAnsiEncoding`,
  `MacRomanEncoding` und `/Differences`-Namen; Breiten aus `/Widths`.
- `Type0`/`Identity-H` mit `/ToUnicode`; fehlt die Zuordnung, wird sie über
  die eingebettete Schrift zurückgewonnen (`Glyphennummer → Zeichen`). Bleibt
  auch das erfolglos, wird der Lauf als „Text ohne Zeichenzuordnung" gezählt.
- Form-XObjects (`Do` mit `/Subtype /Form`) samt `/Matrix`, mit
  Tiefenbegrenzung.
- Mehrere Inhaltsströme je Seite, `CropBox`, `/Rotate` (90/180/270),
  Seitenursprung ungleich (0,0).
- Kurven (`c`, `v`, `y`) werden im **Zielmaß** abgeflacht (0,03 mm), Kreise und
  Bögen erkannt (1 % Toleranz) und wieder zu `Circle`/`Arc`.
- Flächen: Ringe werden nach Fläche sortiert und per Punkt-in-Polygon zu
  Außenring + Löchern (`Even-Odd`-Bild).

### Was nicht übernommen wird (gezählt und gemeldet)

Bilder, Transparenz (`/ca`, `/CA`, Softmasken), Schattierungen (`sh`),
Muster (`/Pattern`), Beschnitt (`W`/`W*`), Formulare/Anmerkungen/OCG-Ebenen.
Verschlüsselte PDFs werden mit klarer Meldung abgelehnt.

### Offene Punkte

- **Schrift-Teilmengen** (siehe Abweichung 3) — spart je Datei ~300 KB.
- **`Annotation`-Maßstab in der GUI:** die Seitenvorschau vor dem Einfügen
  fehlt; die Seitenzahl wird angezeigt, das Blatt erst nach dem Einfügen.
- **Bilder** bleiben außen vor (Plan 4); ein späterer Weg wäre `/Subtype
  /Image` als eingebettetes Raster im Block.
