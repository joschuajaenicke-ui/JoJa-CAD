#!/usr/bin/env python3
"""i18n-Prüfskript (T0105) — prüft die Sprachkataloge gegen den Code.

Der Katalog liegt als Datendatei unter ``crates/jjcad-core/lang/<code>.toml``
und wird per ``include_str!`` einkompiliert (siehe ``docs/technik/I18N.md``). Ein
Tippfehler dort fällt sonst erst im Bild auf: dann steht der Schlüssel oder ein
leerer Text in der Oberfläche. Dieses Skript ist das Tor davor.

Geprüft wird:

* **fehlender Schlüssel** — im Quelltext benutzt, in ``de.toml`` nicht vorhanden
* **verwaister Schlüssel** — in ``de.toml``, im Quelltext nirgends benutzt
* **Platzhalter** — ``{name}`` fehlt, ist überzählig oder hat keinen Namen
* **doppelter Schlüssel** — derselbe Schlüssel zweimal in einer Datei
* **leerer Wert** — ``schluessel = ""``
* **Eintragung** — jede Sprachdatei steht in ``KATALOG_QUELLEN`` und ihr Code
  in ``Lang::ALL`` (sonst wird die Sprache nie angeboten)
* **Übersetzungen** — Schlüssel, den nur die Übersetzung kennt, ist ein Fehler;
  ein fehlender Eintrag ist eine Warnung (Rückfall auf Deutsch ist gewollt)

Aufruf::

    python3 scripts/i18n-check.py                  # Tor: 0 = sauber, 1 = Fehler
    python3 scripts/i18n-check.py --streng         # unvollständige Übersetzung = Fehler
    python3 scripts/i18n-check.py --vorlage        # Vorlage für eine neue Sprache (stdout)
    python3 scripts/i18n-check.py --vorlage PFAD   # ... in eine Datei schreiben
    python3 scripts/i18n-check.py --selbsttest     # Prüfskript selbst prüfen

Die Vorlage ``lang/xx.toml`` enthält **keinen** aktiven Eintrag (alle Zeilen
auskommentiert) und ist damit im Build unschädlich — sie wird nicht als Sprache
angeboten. Nach jeder neuen Katalogzeile neu erzeugen::

    python3 scripts/i18n-check.py --vorlage crates/jjcad-core/lang/xx.toml
"""

from __future__ import annotations

import argparse
import re
import shutil
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LANG_DIR = REPO / "crates" / "jjcad-core" / "lang"
I18N_RS = REPO / "crates" / "jjcad-core" / "src" / "i18n.rs"
CRATES = REPO / "crates"

#: Quellsprache und Rückfall (Dateiname ohne Endung).
QUELLSPRACHE = "de"
#: Dateiname der leeren Vorlage — wird nie als Sprache angeboten.
VORLAGE = "xx.toml"

#: Schlüssel, die absichtlich nicht im Katalog stehen (Rückfallprobe im
#: MCP-Selbsttest: ein unbekannter Schlüssel liefert den Schlüssel selbst).
ABSICHTLICH_UNBEKANNT = {"gibt.es.nicht"}

#: Katalogabschnitte, die keine Übersetzung brauchen (Vorrichtungen der Tests).
NICHT_ZU_UEBERSETZEN = {"test"}

#: Platzhalter dürfen Text nicht sprengen: ab diesem Verhältnis zur deutschen
#: Fassung warnt das Skript (Menüband ist 70 px breit, siehe docs/technik/I18N.md).
LAENGE_FAKTOR = 1.4
LAENGE_ZUSCHLAG = 8

PLATZHALTER = re.compile(r"\{([^{}]*)\}")
PLATZHALTER_NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
SCHLUESSEL = re.compile(r"^[a-z][a-z0-9_]*(\.[a-z0-9_]+)*$")
TABELLE = re.compile(r"^[a-z][a-z0-9_]*(\.[a-z0-9_]+)*$")

#: Eintrag in ``KATALOG_QUELLEN``: ``("en", "English", include_str!("../lang/en.toml"))``
QUELLE_EINTRAG = re.compile(
    r'\(\s*"([a-z]{2,3})"\s*,\s*"([^"]*)"\s*,\s*'
    r'include_str!\("\.\./lang/([A-Za-z0-9_-]+)\.toml"\)\s*\)'
)
#: ``pub const ALL: [Lang; 2] = [Lang::Deutsch, Lang::Englisch];``
ALLE_SPRACHEN = re.compile(r"pub const ALL:\s*\[Lang;\s*\d+\]\s*=\s*\[([^\]]*)\]")
#: Zweig in ``Lang::code()``: ``Lang::Englisch => "en",``
SPRACHCODE = re.compile(r'Lang::(\w+)\s*=>\s*"([a-z]{2,3})"')


# --------------------------------------------------------------------------
# Katalog lesen — bewusst mit denselben Regeln wie `i18n::lies` in Rust
# --------------------------------------------------------------------------


@dataclass
class Katalog:
    """Eine gelesene Sprachdatei samt Befunden."""

    pfad: Path
    werte: dict[str, str] = field(default_factory=dict)
    zeilen: dict[str, int] = field(default_factory=dict)
    reihenfolge: list[str] = field(default_factory=list)
    abschnitte: list[str] = field(default_factory=list)
    fehler: list[str] = field(default_factory=list)

    @property
    def name(self) -> str:
        return self.pfad.name

    @property
    def code(self) -> str:
        return self.pfad.stem


def entpacke(text: str) -> str:
    """Löst die Escapes auf, die ``i18n::entpacke`` kennt (n, t, ", \\)."""
    out: list[str] = []
    zeichen = iter(text)
    for c in zeichen:
        if c != "\\":
            out.append(c)
            continue
        naechstes = next(zeichen, None)
        if naechstes == "n":
            out.append("\n")
        elif naechstes == "t":
            out.append("\t")
        elif naechstes == '"':
            out.append('"')
        elif naechstes == "\\":
            out.append("\\")
        elif naechstes is None:
            out.append("\\")
        else:
            out.append("\\")
            out.append(naechstes)
    return "".join(out)


def platzhalter(text: str) -> list[str]:
    """Alle ``{name}`` eines Textes in der Reihenfolge des Auftretens."""
    return [m.group(1) for m in PLATZHALTER.finditer(text)]


def lies(pfad: Path) -> Katalog:
    """Liest eine Katalogdatei und sammelt ihre Befunde.

    Der Rust-Leser (``i18n::lies``) **überspringt** Zeilen, die er nicht
    versteht, still — deshalb ist hier jede unlesbare Zeile ein Fehler.
    """
    katalog = Katalog(pfad)
    try:
        roh = pfad.read_bytes()
    except OSError as e:
        katalog.fehler.append(f"{pfad.name}: nicht lesbar ({e})")
        return katalog
    try:
        text = roh.decode("utf-8")
    except UnicodeDecodeError as e:
        katalog.fehler.append(
            f"{pfad.name}: keine UTF-8-Datei ({e.reason} bei Byte {e.start}) — "
            "die Umlaute der Oberfläche kommen sonst als Fragezeichen an"
        )
        return katalog
    if text.startswith("\ufeff"):
        katalog.fehler.append(
            f"{pfad.name}:1: Byte-Order-Mark am Dateianfang — die erste Zeile "
            "ist damit keine Tabelle und wird übersprungen"
        )
        text = text[1:]

    tabelle = ""
    for nummer, zeile in enumerate(text.splitlines(), 1):
        z = zeile.strip()
        if not z or z.startswith("#"):
            continue
        if z.startswith("["):
            if not z.endswith("]"):
                katalog.fehler.append(
                    f"{pfad.name}:{nummer}: Tabellenkopf ohne „]“: {z}"
                )
                continue
            tabelle = z[1:-1].strip()
            if not TABELLE.match(tabelle):
                katalog.fehler.append(
                    f"{pfad.name}:{nummer}: unbrauchbarer Tabellenname „{tabelle}“"
                )
            if tabelle not in katalog.abschnitte:
                katalog.abschnitte.append(tabelle)
            continue
        if "=" not in z:
            katalog.fehler.append(
                f"{pfad.name}:{nummer}: keine Zuweisung „schlüssel = \"text\"“: {z}"
            )
            continue
        schluessel, _, wert = z.partition("=")
        schluessel = schluessel.strip()
        wert = wert.strip()
        voll = f"{tabelle}.{schluessel}" if tabelle else schluessel
        if not SCHLUESSEL.match(voll):
            katalog.fehler.append(
                f"{pfad.name}:{nummer}: unbrauchbarer Schlüssel „{voll}“"
            )
            continue
        if len(wert) < 2 or not (wert.startswith('"') and wert.endswith('"')):
            katalog.fehler.append(
                f"{pfad.name}:{nummer}: Wert von „{voll}“ steht nicht in "
                "Anführungszeichen — der Leser überspringt die Zeile still"
            )
            continue
        if voll in katalog.werte:
            katalog.fehler.append(
                f"{pfad.name}:{nummer}: doppelter Schlüssel „{voll}“ "
                f"(schon in Zeile {katalog.zeilen[voll]})"
            )
            continue
        inhalt = entpacke(wert[1:-1])
        katalog.werte[voll] = inhalt
        katalog.zeilen[voll] = nummer
        katalog.reihenfolge.append(voll)
        if not inhalt.strip():
            katalog.fehler.append(
                f"{pfad.name}:{nummer}: leerer Wert bei „{voll}“"
            )
        for name in platzhalter(inhalt):
            if not PLATZHALTER_NAME.match(name):
                katalog.fehler.append(
                    f"{pfad.name}:{nummer}: Platzhalter „{{{name}}}“ bei "
                    f"„{voll}“ hat keinen Namen — er bleibt als Klammer stehen"
                )
        ohne = PLATZHALTER.sub("", inhalt)
        if "{" in ohne or "}" in ohne:
            katalog.fehler.append(
                f"{pfad.name}:{nummer}: unvollständige Klammer bei „{voll}“: {inhalt}"
            )
    return katalog


# --------------------------------------------------------------------------
# Quelltext lesen
# --------------------------------------------------------------------------


def abschnitte_von(katalog: Katalog) -> list[str]:
    """Die Katalogabschnitte (erster Teil jedes Schlüssels), in Dateireihenfolge."""
    gefunden: list[str] = []
    for key in katalog.reihenfolge:
        kopf = key.split(".", 1)[0]
        if kopf not in gefunden:
            gefunden.append(kopf)
    return gefunden


def benutzte_schluessel(
    crates: Path, abschnitte: list[str]
) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    """Sammelt die im Quelltext benutzten Schlüssel.

    Zwei Wege, weil der Katalog an zwei Arten von Stellen steht:

    * als Zeichenkette direkt am Aufruf (``tr("gui.app.wand_gesetzt")``),
    * als Zeichenkette in einer Tabelle, die über eine Variable nachgeschlagen
      wird (``act!("draw.wall", "action.draw.wall.label", …)``,
      ``ActionGroup::Zeichnen => "group.zeichnen"``).

    Der zweite Weg wird über den Abschnittsnamen gefunden: jede Zeichenkette,
    die mit ``action.``, ``gui.`` … beginnt, gilt als Katalogschlüssel. Das ist
    genau der Unterschied zu einer Action-ID (``draw.wall``) — die Abschnitte
    sind eine eigene Namenswelt.

    Rückgabe: (gefundene Schlüssel → Fundstellen, Schlüssel außerhalb der
    bekannten Abschnitte → Fundstellen).
    """
    tabelle = re.compile(
        r'"((?:' + "|".join(re.escape(a) for a in sorted(abschnitte, key=len, reverse=True)) + r')\.[a-z0-9_.]+)"'
    )
    aufrufe = [
        re.compile(r'(?<![\w!])tr\(\s*"([^"]+)"'),
        re.compile(r'(?<![\w!])t!\(\s*"([^"]+)"'),
        re.compile(r'(?<![\w!])t_args(?:_owned)?\(\s*"([^"]+)"'),
        re.compile(r'(?<![\w!])rb\(\s*"([^"]+)"'),
    ]
    gefunden: dict[str, set[str]] = {}
    fremd: dict[str, set[str]] = {}
    for datei in sorted(crates.rglob("*.rs")):
        rel = datei.relative_to(REPO).as_posix()
        try:
            text = datei.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for nummer, zeile in enumerate(text.splitlines(), 1):
            # Kommentarzeilen überspringen: die Doc-Beispiele in `i18n.rs`
            # (`tr("menu.file")`) sind Erklärtext, kein Benutzer des Katalogs.
            if zeile.lstrip().startswith("//"):
                continue
            for m in tabelle.finditer(zeile):
                gefunden.setdefault(m.group(1), set()).add(f"{rel}:{nummer}")
            for muster in aufrufe:
                for m in muster.finditer(zeile):
                    key = m.group(1)
                    if key.split(".", 1)[0] in abschnitte:
                        gefunden.setdefault(key, set()).add(f"{rel}:{nummer}")
                    elif SCHLUESSEL.match(key) and key not in ABSICHTLICH_UNBEKANNT:
                        fremd.setdefault(key, set()).add(f"{rel}:{nummer}")
    return gefunden, fremd


def eintragung(i18n_rs: Path) -> tuple[dict[str, str], set[str], set[str], list[str]]:
    """Liest die Sprachliste aus ``i18n.rs``.

    Rückgabe: (Code → Dateiname aus ``KATALOG_QUELLEN``, Codes aus ``Lang::ALL``,
    Codes aus ``Lang::code()``, Fehler beim Lesen).
    """
    fehler: list[str] = []
    try:
        text = i18n_rs.read_text(encoding="utf-8")
    except OSError as e:
        return {}, set(), set(), [f"{i18n_rs.name}: nicht lesbar ({e})"]

    quellen = {m.group(1): m.group(3) for m in QUELLE_EINTRAG.finditer(text)}
    if not quellen:
        fehler.append(
            f"{i18n_rs.name}: KATALOG_QUELLEN nicht gefunden — das Skript sucht "
            'Einträge der Form ("en", "English", include_str!("../lang/en.toml"))'
        )

    treffer = ALLE_SPRACHEN.search(text)
    alle = set()
    if treffer:
        alle = set(re.findall(r"Lang::(\w+)", treffer.group(1)))
    else:
        fehler.append(f"{i18n_rs.name}: Lang::ALL nicht gefunden")

    codes = {m.group(2) for m in SPRACHCODE.finditer(text)}
    return quellen, alle, codes, fehler


# --------------------------------------------------------------------------
# Prüfen
# --------------------------------------------------------------------------


def pruefe(
    lang_dir: Path, crates: Path, i18n_rs: Path, eintragung_pruefen: bool = True
) -> tuple[list[str], list[str], list[Katalog]]:
    """Führt alle Prüfungen aus. Rückgabe: (Fehler, Warnungen, Kataloge)."""
    fehler: list[str] = []
    warnungen: list[str] = []

    dateien = sorted(p for p in lang_dir.glob("*.toml"))
    if not dateien:
        return [f"{lang_dir}: keine Sprachdatei (*.toml) gefunden"], [], []

    kataloge = {p.stem: lies(p) for p in dateien}
    for k in kataloge.values():
        fehler.extend(k.fehler)

    quelle = kataloge.get(QUELLSPRACHE)
    if quelle is None:
        fehler.append(
            f"{QUELLSPRACHE}.toml fehlt in {lang_dir} — die Quellsprache ist Pflicht"
        )
        return fehler, warnungen, list(kataloge.values())

    abschnitte = abschnitte_von(quelle)
    gefunden, fremd = benutzte_schluessel(crates, abschnitte)

    # 1) Im Code benutzt, in der Quellsprache nicht vorhanden.
    for key in sorted(set(gefunden) - set(quelle.werte)):
        stellen = ", ".join(sorted(gefunden[key])[:3])
        fehler.append(
            f"{QUELLSPRACHE}.toml: Schlüssel „{key}“ fehlt (benutzt in {stellen})"
        )
    # 2) In der Quellsprache, im Code nicht benutzt.
    for key in sorted(set(quelle.werte) - set(gefunden)):
        if key.split(".", 1)[0] in NICHT_ZU_UEBERSETZEN:
            continue
        fehler.append(
            f"{QUELLSPRACHE}.toml:{quelle.zeilen[key]}: Schlüssel „{key}“ wird "
            "nirgends benutzt (verwaist) — Text ändern oder Zeile löschen"
        )
    # 3) Schlüssel außerhalb der bekannten Abschnitte.
    for key in sorted(fremd):
        warnungen.append(
            f"Schlüssel „{key}“ liegt außerhalb der Katalogabschnitte "
            f"({', '.join(abschnitte)}) — benutzt in {', '.join(sorted(fremd[key])[:2])}"
        )

    # 4) Jede weitere Sprache gegen die Quellsprache.
    for code, katalog in sorted(kataloge.items()):
        if code in (QUELLSPRACHE, Path(VORLAGE).stem):
            continue
        offen = [k for k in quelle.reihenfolge if k not in katalog.werte]
        ueberzaehlig = [k for k in katalog.reihenfolge if k not in quelle.werte]
        for key in ueberzaehlig:
            fehler.append(
                f"{katalog.name}:{katalog.zeilen[key]}: Schlüssel „{key}“ steht "
                f"nicht in {QUELLSPRACHE}.toml (verwaist)"
            )
        for key, wert in katalog.werte.items():
            if key not in quelle.werte:
                continue
            soll = set(platzhalter(quelle.werte[key]))
            ist = set(platzhalter(wert))
            if soll != ist:
                fehlt = ", ".join(f"{{{n}}}" for n in sorted(soll - ist)) or "—"
                zuviel = ", ".join(f"{{{n}}}" for n in sorted(ist - soll)) or "—"
                fehler.append(
                    f"{katalog.name}:{katalog.zeilen[key]}: Platzhalter passen "
                    f"nicht zu {QUELLSPRACHE}.toml („{key}“): fehlt {fehlt}, "
                    f"überzählig {zuviel}"
                )
        if offen:
            kurz = ", ".join(offen[:5]) + (" …" if len(offen) > 5 else "")
            text = (
                f"{katalog.name}: {len(offen)} von {len(quelle.reihenfolge)} "
                f"Schlüsseln ohne Übersetzung (Rückfall Deutsch): {kurz}"
            )
            warnungen.append(text)
        # Länge: das Menüband ist eng, ein langer Text wird verkleinert oder
        # läuft in die Nachbargruppe.
        for key, wert in katalog.werte.items():
            if key not in quelle.werte:
                continue
            kurz = key.endswith(".label") or key.split(".", 1)[0] in ("ribbon", "group", "ui")
            if not kurz:
                continue
            deutsch = quelle.werte[key]
            if len(wert) > len(deutsch) * LAENGE_FAKTOR + LAENGE_ZUSCHLAG:
                warnungen.append(
                    f"{katalog.name}:{katalog.zeilen[key]}: „{wert}“ ist "
                    f"{len(wert)} Zeichen lang, deutsch {len(deutsch)} — im "
                    "Menüband (70 px) zu breit"
                )

    # 5) Eintragung in i18n.rs: eine Datei allein reicht nicht.
    if eintragung_pruefen:
        quellen, alle, codes, lesefehler = eintragung(i18n_rs)
        fehler.extend(lesefehler)
        eingetragen = set(quellen.values())
        for code, katalog in sorted(kataloge.items()):
            if code == Path(VORLAGE).stem:
                continue
            if code not in eingetragen:
                fehler.append(
                    f"{katalog.name}: nicht in KATALOG_QUELLEN eingetragen "
                    f"({i18n_rs.name}) — die Datei wird nie geladen"
                )
            elif code not in codes:
                fehler.append(
                    f"{katalog.name}: Sprachcode „{code}“ fehlt in Lang::code() "
                    "— die Sprache lässt sich nicht einstellen"
                )
        for code, datei in sorted(quellen.items()):
            if not (lang_dir / f"{datei}.toml").is_file():
                fehler.append(
                    f"{i18n_rs.name}: KATALOG_QUELLEN nennt „{code}“ mit "
                    f"{datei}.toml — die Datei fehlt in {lang_dir}"
                )
            if code not in codes:
                fehler.append(
                    f"{i18n_rs.name}: KATALOG_QUELLEN nennt „{code}“, "
                    "Lang::code() kennt den Code nicht"
                )
        # Sprache ohne Code-Zweig in Lang::ALL wird nie angeboten.
        for code in sorted(codes):
            if code not in quellen:
                fehler.append(
                    f"{i18n_rs.name}: Lang::code() kennt „{code}“, "
                    "KATALOG_QUELLEN lädt dafür aber keinen Katalog"
                )
        if len(alle) != len(quellen):
            fehler.append(
                f"{i18n_rs.name}: Lang::ALL hat {len(alle)} Sprachen, "
                f"KATALOG_QUELLEN {len(quellen)} — beide Listen müssen gleich sein"
            )

    # 6) Vorlage: sie muss alle Schlüssel der Quellsprache kennen (auskommentiert).
    vorlage_pfad = lang_dir / VORLAGE
    if vorlage_pfad.is_file():
        soll = vorlage_text(quelle)
        ist = vorlage_pfad.read_text(encoding="utf-8", errors="replace")
        if soll != ist:
            fehlend = [
                key for key in quelle.reihenfolge if f"# {key.split('.', 1)[1]} =" not in ist
            ]
            warnungen.append(
                f"{VORLAGE} ist veraltet ({len(fehlend)} Schlüssel fehlen, z. B. "
                f"{fehlend[0] if fehlend else '—'}) — neu erzeugen mit "
                "„python3 scripts/i18n-check.py --vorlage crates/jjcad-core/lang/xx.toml“"
            )

    return fehler, warnungen, [kataloge[c] for c in sorted(kataloge)]


# --------------------------------------------------------------------------
# Vorlage erzeugen
# --------------------------------------------------------------------------

VORLAGE_KOPF = """\
# JoJa-CAD — leere Vorlage für eine weitere Sprache (T0105)
#
# So wird daraus eine Sprache (ausführlich: docs/technik/I18N.md):
#   1. Diese Datei kopieren:  lang/fr.toml
#   2. Zeilen auskommentieren heißt übersetzen: das „# “ am Zeilenanfang
#      entfernen und den deutschen Text durch die Übersetzung ersetzen.
#      Nicht übersetzte Zeilen dürfen auskommentiert bleiben — dann erscheint
#      der deutsche Text (Rückfall).
#   3. Schlüssel NIE ändern, nur die Texte rechts vom „=“.
#   4. Prüfen:  python3 scripts/i18n-check.py
#   5. In crates/jjcad-core/src/i18n.rs eintragen (KATALOG_QUELLEN, Lang).
#
# English: copy this file to lang/<code>.toml, uncomment a line and replace the
# German text on the right of „=“. Never change a key. Missing entries fall back
# to German. Then run scripts/i18n-check.py and register the file in
# crates/jjcad-core/src/i18n.rs (KATALOG_QUELLEN and the Lang enum).
#
# Platzhalter wie {name} müssen erhalten bleiben — sie werden zur Laufzeit
# gefüllt; die Reihenfolge darf sich ändern, der Name nicht.
"""

ABSCHNITTS_TEXT = {
    "action": "Aktionen: Beschriftung (.label) und Kurzhinweis (.hint) von Ribbon,\n"
    "# Inventar und MCP. Der Hinweis steht als Tooltip und in der Statuszeile.",
    "group": "Gruppenüberschrift der Aktionen (ActionGroup::key) — kurz halten.",
    "ribbon": "Menüband: Reiter, Gruppen und einzelne Schaltflächen. Sehr kurze\n"
    "# Texte, die Schaltfläche ist 70 px breit.",
    "ui": "Auswahlwerte in Auswahllisten (Einfügemodus, Bogenmodus, Füllart …).",
    "settings": "Einstellungsfenster: Titel, Suchfeld, Kategorien.",
    "gui": "Meldungen und Beschriftungen der Oberfläche, je Quelldatei ein\n"
    "# Abschnitt (gui.app, gui.canvas, gui.panels …). Platzhalter wie {name}\n"
    "# und {wert1} stehen für Werte, die das Programm einsetzt.",
    "test": "Nur für die Tests des Sprachbaukastens — NICHT übersetzen.",
}


def vorlage_text(quelle: Katalog) -> str:
    """Erzeugt die Vorlage: jede Katalogzeile auskommentiert, nach Abschnitten."""
    zeilen: list[str] = [VORLAGE_KOPF]
    letzter_abschnitt = ""
    letzte_tabelle = ""
    for key in quelle.reihenfolge:
        tabelle, _, kurz = key.rpartition(".")
        abschnitt = key.split(".", 1)[0]
        if abschnitt != letzter_abschnitt:
            zeilen.append(f"#\n# --- {abschnitt} " + "-" * max(0, 60 - len(abschnitt)) + "\n#")
            if abschnitt in ABSCHNITTS_TEXT:
                zeilen.append(f"# {ABSCHNITTS_TEXT[abschnitt]}")
            letzter_abschnitt = abschnitt
            letzte_tabelle = ""
        if tabelle != letzte_tabelle:
            zeilen.append(f"#\n[{tabelle}]")
            letzte_tabelle = tabelle
        wert = quelle.werte[key].replace("\\", "\\\\").replace('"', '\\"')
        wert = wert.replace("\n", "\\n").replace("\t", "\\t")
        zeilen.append(f'# {kurz} = "{wert}"')
    return "\n".join(zeilen) + "\n"


# --------------------------------------------------------------------------
# Selbsttest
# --------------------------------------------------------------------------


def entferne_block(text: str, tabelle: str) -> str:
    """Entfernt den Abschnitt ``[tabelle]`` samt seiner Zeilen."""
    aus: list[str] = []
    drin = False
    for zeile in text.splitlines():
        z = zeile.strip()
        if z.startswith("[") and z.endswith("]"):
            drin = z[1:-1] == tabelle
            if drin:
                continue
        if not drin:
            aus.append(zeile)
    return "\n".join(aus) + "\n"


def setze_wert(text: str, tabelle: str, schluessel: str, wert: str) -> str:
    """Setzt ``schluessel`` in ``[tabelle]``; legt den Abschnitt sonst an."""
    zeilen = text.splitlines()
    start = None
    for i, zeile in enumerate(zeilen):
        z = zeile.strip()
        if z.startswith("[") and z.endswith("]"):
            if start is not None:
                # Abschnitt endet, ohne den Schlüssel zu enthalten.
                zeilen.insert(i, f"{schluessel} = {wert}")
                return "\n".join(zeilen) + "\n"
            if z[1:-1] == tabelle:
                start = i
            continue
        if start is not None and z.partition("=")[0].strip() == schluessel:
            zeilen[i] = f"{schluessel} = {wert}"
            return "\n".join(zeilen) + "\n"
    if start is None:
        zeilen.extend([f"\n[{tabelle}]", f"{schluessel} = {wert}"])
        return "\n".join(zeilen) + "\n"
    zeilen.insert(start + 1, f"{schluessel} = {wert}")
    return "\n".join(zeilen) + "\n"


def selbsttest() -> int:
    """Prüft das Prüfskript: sauberer Bestand = 0, drei eingebaute Fehler = 1."""
    print("i18n-Prüfskript — Selbsttest")
    with tempfile.TemporaryDirectory(prefix="i18n-selbsttest-") as tmp:
        ziel = Path(tmp)
        for name in (f"{QUELLSPRACHE}.toml", "en.toml"):
            quelle = LANG_DIR / name
            if not quelle.is_file():
                print(f"  FEHLER: {quelle} fehlt — Selbsttest nicht möglich")
                return 1
            shutil.copy(quelle, ziel / name)

        # 1) Sauberer Bestand: keine Fehler (Warnungen sind erlaubt).
        fehler, warnungen, _ = pruefe(ziel, CRATES, I18N_RS, eintragung_pruefen=False)
        if fehler:
            print(f"  FEHLER: sauberer Bestand meldet {len(fehler)} Fehler:")
            for f in fehler[:10]:
                print(f"    {f}")
            return 1
        print(f"  sauberer Bestand: 0 Fehler, {len(warnungen)} Warnungen — ok")

        # 2) Drei Fehler einbauen.
        de = (ziel / f"{QUELLSPRACHE}.toml").read_text(encoding="utf-8")
        en = (ziel / "en.toml").read_text(encoding="utf-8")

        # a) fehlender Schlüssel: eine Tabelle entfernen, die der Code benutzt
        fehlend_tabelle = "action.file.open"
        de = entferne_block(de, fehlend_tabelle)
        # b) leerer Wert
        leer_tabelle, leer_schluessel = "action.file.save", "label"
        en = setze_wert(en, leer_tabelle, leer_schluessel, '""')
        # c) falscher Platzhalter: ein Schlüssel mit benanntem Platzhalter
        kandidat = next(
            (
                k
                for k in lies(ziel / f"{QUELLSPRACHE}.toml").reihenfolge
                if platzhalter(lies(ziel / f"{QUELLSPRACHE}.toml").werte[k])
                and k.split(".", 1)[0] != "test"
            ),
            None,
        )
        if kandidat is None:
            print("  FEHLER: kein Schlüssel mit Platzhalter im Katalog gefunden")
            return 1
        tab, _, kurz = kandidat.rpartition(".")
        en = setze_wert(en, tab, kurz, '"übersetzt {falscher_name}"')

        (ziel / f"{QUELLSPRACHE}.toml").write_text(de, encoding="utf-8")
        (ziel / "en.toml").write_text(en, encoding="utf-8")

        fehler, _, _ = pruefe(ziel, CRATES, I18N_RS, eintragung_pruefen=False)
        text = "\n".join(fehler)
        geprueft = {
            "fehlender Schlüssel": f"{fehlend_tabelle}.label" in text,
            "leerer Wert": f"leerer Wert bei „{leer_tabelle}.{leer_schluessel}“" in text,
            "falscher Platzhalter": f"„{kandidat}“" in text and "Platzhalter" in text,
        }
        for name, ok in geprueft.items():
            print(f"  {name}: {'gefunden' if ok else 'NICHT gefunden'}")
        if not all(geprueft.values()) or not fehler:
            print("  SELBSTTEST: durchgefallen")
            for f in fehler[:10]:
                print(f"    {f}")
            return 1
    print(f"  SELBSTTEST: bestanden ({len(fehler)} Fehler gemeldet, Exitcode wäre 1)")
    return 0


# --------------------------------------------------------------------------
# Aufruf
# --------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    zerleger = argparse.ArgumentParser(
        description="Prüft die Sprachkataloge (lang/*.toml) gegen den Quelltext.",
        epilog="Exitcode 0 = sauber, 1 = Fehler. Siehe docs/technik/I18N.md.",
    )
    zerleger.add_argument(
        "--lang-dir",
        type=Path,
        default=LANG_DIR,
        help=f"Verzeichnis der Sprachdateien (Vorgabe: {LANG_DIR.relative_to(REPO)})",
    )
    zerleger.add_argument(
        "--crates",
        type=Path,
        default=CRATES,
        help="Quelltextverzeichnis für die Schlüsselsuche",
    )
    zerleger.add_argument(
        "--streng",
        action="store_true",
        help="unvollständige Übersetzungen als Fehler werten (Vorgabe: Warnung)",
    )
    zerleger.add_argument(
        "--vorlage",
        nargs="?",
        const="-",
        metavar="PFAD",
        help="Vorlage für eine neue Sprache ausgeben (ohne PFAD auf stdout)",
    )
    zerleger.add_argument(
        "--selbsttest",
        action="store_true",
        help="das Prüfskript selbst prüfen (drei eingebaute Fehler)",
    )
    args = zerleger.parse_args(argv)

    if args.selbsttest:
        return selbsttest()

    if args.vorlage is not None:
        quelle = lies(args.lang_dir / f"{QUELLSPRACHE}.toml")
        if quelle.fehler:
            for f in quelle.fehler:
                print(f"FEHLER: {f}", file=sys.stderr)
            return 1
        text = vorlage_text(quelle)
        if args.vorlage == "-":
            sys.stdout.write(text)
        else:
            pfad = Path(args.vorlage)
            pfad.write_text(text, encoding="utf-8")
            print(f"Vorlage geschrieben: {pfad} ({len(quelle.reihenfolge)} Schlüssel)")
        return 0

    fehler, warnungen, kataloge = pruefe(args.lang_dir, args.crates, I18N_RS)
    if args.streng:
        streng = [w for w in warnungen if "ohne Übersetzung" in w]
        warnungen = [w for w in warnungen if w not in streng]
        fehler.extend(streng)

    print(f"i18n-Prüfung — {args.lang_dir}")
    for k in kataloge:
        print(f"  {k.name:<10} {len(k.werte):>4} Schlüssel")
    if warnungen:
        print("\nWARNUNGEN")
        for w in warnungen:
            print(f"  {w}")
    if fehler:
        print("\nFEHLER")
        for f in fehler:
            print(f"  {f}")
        print(
            f"\n{len(fehler)} Fehler, {len(warnungen)} Warnungen — "
            "Katalog nicht in Ordnung (Exitcode 1)"
        )
        return 1
    print(f"\n0 Fehler, {len(warnungen)} Warnungen — Katalog in Ordnung")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
