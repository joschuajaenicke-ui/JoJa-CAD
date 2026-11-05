# Versionen und Veröffentlichung

Wie JoJa-CAD versioniert wird, was ein Release enthält und wie der Weg von der
geschlossenen Entwicklung zur ersten öffentlichen Fassung aussieht.

## Die Stufen

| Version | Bedeutung | Sichtbarkeit |
|---|---|---|
| **0.1** | Prototyp. Hat nie richtig funktioniert, wird nicht veröffentlicht und nicht getaggt. | — |
| **0.2** | Geschlossene Entwicklung. `0.2.x` sind Zwischenstände für den eigenen Test. | privat |
| **0.3** | Erste öffentliche Fassung — veröffentlicht, um Feedback einzuholen. | öffentlich |
| **1.0** | Grundfunktionalität vollständig. | öffentlich |

## Regeln

- **Eine Quelle für die Version:** `version` in `Cargo.toml` (`[workspace.package]`).
  Sie steht im Fenstertitel und in der Statusleiste.
- **`0.2.x` zählt Releases hoch**, nicht Commits: `0.2.1`, `0.2.2`, … Ein Release
  entsteht, wenn eine **Menge von Funktionen vollständig** ist — nicht für einen
  einzelnen Fix.
- **`0.3.0` ist der Schnitt zur Öffentlichkeit.** Ab dann: `0.3.x` = Fehlerbehebungen,
  `0.4.0` = neue Funktionen, `1.0.0` = Grundfunktionalität vollständig.
- **Die Dateiformat-Version ist unabhängig** (`"version": 2` in der `.jjcad`). Sie
  steigt nur bei einer inkompatiblen Änderung — dann mit Migration und einem
  Hinweis in den Patchnotes.
- **Tags** heißen `v0.2.1`, sind annotiert, liegen nur auf `master` und entstehen
  nur nach eigener Abnahme im Testkanal.
- **Push ist kein Release.** Ein Push ist eine Sicherung: einmal täglich, manuell,
  wenn etwas geschafft ist — `git push origin master`. Keine Automatik, keine
  Entwicklungszweige (`fix/…`, `alt/…`, `wt/…`), kein `--all`/`--mirror`.

## Was ein Release enthält

1. **Tag** `v0.2.x` auf `master`.
2. **GitHub-Release** mit Patchnotes aus [`CHANGELOG.md`](../CHANGELOG.md).
3. **Binaries**: Linux (AppImage bzw. .deb) und Windows (Installer bzw. ZIP) —
   Windows erst, wenn der Windows-Build in der CI grün ist. Ein Release, der
   Windows nennt, ohne dass es gebaut wurde, ist ein Fehler.
4. **Prüfung**: Release-Seite öffnen, beide Pakete herunterladen, starten,
   eine Beispielzeichnung öffnen.

## Ablauf eines Releases

```bash
# 1. Stand ist abgenommen (Testkanal, eigener Test)
# 2. Version erhöhen: Cargo.toml -> version = "0.2.1"
# 3. CHANGELOG.md: Abschnitt 0.2.1 mit Neu / Geändert / Behoben
cargo test && cargo build --release
git commit -am "Version 0.2.1"
git tag -a v0.2.1 -m "0.2.1"
git push origin master --follow-tags
gh release create v0.2.1 --title "0.2.1" --notes-file <(sed -n '/## 0.2.1/,/## 0.2.0/p' CHANGELOG.md) ./dist/*
```

## Weg von 0.2 nach 0.3

Sechs Blöcke. Ein Block ist fertig, wenn er im Testkanal abgenommen ist — nicht,
wenn der Code geschrieben ist.

**A — Zeichnen vollständig**
- Tür-/Fenster-Ausrichtung nachträglich im Eigenschaften-Panel änderbar
- Öffnungsposition als Wandparameter (Abstand längs, Lage quer), damit Öffnungen
  beim Verschieben der Wand folgen
- Griffe an Maßketten
- Stumpf-statt-Gehrung bei entgegengesetzter Ausrichtung: Entscheidung treffen
  (Modellfrage, kein Rechenfehler)

**B — Ausgabe**
- PDF/DXF-Rundlauf mit den Beispieldateien ohne Abweichung
- Blätter A4–A0 mit Ansichtsfenstern, Maßstab, Schriftfeld
- Druck-PDF aus dem Papierraum, Linienstärken wie im Druck

**C — Bedienung**
- Undo/Redo über alle Aktionen
- Tastaturbelegung vollständig und änderbar
- Oberfläche Deutsch und Englisch, Katalog vollständig

**D — Plattformen**
- Windows-Build in der CI (`windows-latest`), Startfähigkeit geprüft
- Linux-Paket (AppImage oder .deb) baut reproduzierbar

**E — Veröffentlichung**
- README, Lizenz, Screenshots, Issue-Vorlagen stehen
- Entscheidung zur Historie: Verlauf umschreiben oder frisches öffentliches Repo
  ab 0.3
- Repo auf öffentlich stellen, Release `v0.3.0` mit beiden Paketen

**F — Abnahme für 0.3**

Ein vollständiger Grundriss von der leeren Datei bis zum fertigen PDF, ohne
Handarbeit und ohne Absturz: Wände, Öffnungen, Treppe, Bemaßung, Blatt mit
Schriftfeld, Export als PDF und DXF — auf Linux und Windows. Dazu drei
Beispielpläne, die diesen Weg abdecken.
