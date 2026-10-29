#!/usr/bin/env bash
# Abnahme des PDF-Rundlaufs (docs/PDF-IMPORT.md, Abschnitt 6).
#
#   1. Tests: Import (Grundriss.pdf), Textformatierung, Rundlauf (Struktur,
#      Text, Pixel) — die Zahlen stehen im Protokoll.
#   2. Am erzeugten PDF selbst: Schrift eingebettet (pdffonts), Text als Text
#      lesbar (pdftotext), Titel mit Gedankenstrich (pdfinfo).
#
# Aufruf: ./scripts/pdf-roundtrip.sh            (baut im Projektverzeichnis)
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

fehler=0
rot() { printf '\033[31m%s\033[0m\n' "$*"; }
gruen() { printf '\033[32m%s\033[0m\n' "$*"; }

echo "== 1/3 Tests =="
cargo test -p jjcad-core --test pdf_import --test pdf_roundtrip --test text_runs \
    -- --nocapture 2>&1 | tee /tmp/jjcad-pdf-tests.log | grep -E "^(test |Pixel|Objekte|Texte|Grundriss|running|test result)" 
if ! grep -q "test result: ok" /tmp/jjcad-pdf-tests.log; then
    rot "Tests rot — siehe /tmp/jjcad-pdf-tests.log"
    fehler=1
fi
# Auch bei mehreren Testdateien: keine Fehlschläge erlaubt.
if grep -q "FAILED" /tmp/jjcad-pdf-tests.log; then
    rot "Mindestens ein Test ist fehlgeschlagen."
    fehler=1
fi

PDF=$(ls -t "${TMPDIR:-/tmp}"/jjcad-rt.pdf 2>/dev/null | head -1)
if [ -z "$PDF" ]; then
    rot "Kein Test-PDF gefunden (jjcad-rt.pdf) — Testausgabe prüfen."
    exit 1
fi

echo
echo "== 2/3 Schriften im PDF: ${PDF} =="
if command -v pdffonts >/dev/null; then
    pdffonts "$PDF"
    if pdffonts "$PDF" | awk 'NR>2 {print $4}' | grep -qi "no"; then
        rot "Mindestens eine Schrift ist NICHT eingebettet."
        fehler=1
    else
        gruen "Alle Schriften eingebettet."
    fi
else
    echo "pdffonts fehlt (poppler-utils) — übersprungen."
fi

echo
echo "== 3/3 Text und Titel =="
if command -v pdftotext >/dev/null; then
    if pdftotext "$PDF" - | grep -q "WOHNEN"; then
        gruen "Raumnamen stehen als Text im PDF."
    else
        rot "Raumnamen fehlen in pdftotext — Text ist nur Grafik."
        fehler=1
    fi
    if command -v pdfinfo >/dev/null; then
        titel=$(pdfinfo "$PDF" | sed -n 's/^Title:[[:space:]]*//p')
        echo "Titel: $titel"
        case "$titel" in
            *"?"*) rot "Titel enthält '?' — Zeichen außerhalb WinAnsi verloren."; fehler=1 ;;
            *"—"*|*"–"*) gruen "Titel mit Gedankenstrich (UTF-16) korrekt." ;;
            *) echo "Titel ohne Gedankenstrich — in Ordnung, wenn die Quelle keinen hat." ;;
        esac
    fi
else
    echo "pdftotext fehlt (poppler-utils) — übersprungen."
fi

echo
if [ "$fehler" -eq 0 ]; then
    gruen "Rundlauf in Ordnung."
else
    rot "Rundlauf mit Befunden — siehe oben."
fi
exit "$fehler"
