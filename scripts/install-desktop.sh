#!/usr/bin/env bash
# Legt Markenzeichen und Startmenü-Eintrag für JoJa-CAD an (T0083).
#
# Warum das nötig ist: unter Wayland überträgt winit kein Fenstericon — KDE und
# GNOME holen das Symbol einer Anwendung aus dem .desktop-Eintrag, dessen Name
# zur App-ID passt ("jjcad", gesetzt in `crates/jjcad-gui/src/main.rs`). Unter
# X11, Windows und macOS trägt das Programm das Icon selbst (dieselbe PNG-Datei
# aus `crates/jjcad-gui/assets/`).
#
# Aufruf (idempotent, kein root, schreibt nur in das eigene Benutzerverzeichnis):
#
#   ./scripts/install-desktop.sh [Pfad zum Programm]
#
# Ohne Argument wird ~/jjcad-live/jjcad genommen (Testkanal), sonst
# target/release/jjcad des Arbeitsverzeichnisses.
set -euo pipefail
cd "$(dirname "$0")/.."

PROJ="$(pwd)"
BIN="${1:-}"
if [ -z "$BIN" ]; then
    for kandidat in "$HOME/jjcad-live/jjcad" "$PROJ/target/release/jjcad"; do
        if [ -x "$kandidat" ]; then BIN="$kandidat"; break; fi
    done
fi
# Den Symlink des Testkanals **nicht** auflösen: `~/jjcad-live/jjcad` zeigt
# immer auf die neueste Auslieferung, ein aufgelöster Pfad würde den
# Startmenü-Eintrag dagegen auf den Build festnageln, der beim Aufruf dieses
# Skripts gerade aktuell war (gesehen: Exec zeigte nach der nächsten
# Auslieferung noch auf s213).
[ -n "$BIN" ] && [ -x "$BIN" ] || {
    printf 'install-desktop: kein Programm gefunden (erst bauen oder Pfad angeben).\n' >&2
    exit 1
}

DATEN="${XDG_DATA_HOME:-$HOME/.local/share}"
ICONS="$DATEN/icons/hicolor"
APPS="$DATEN/applications"

# Alle Größen, die das Markenzeichen mitbringt (T0113). Die Taskleiste fragt je
# nach Skalierung 16/24/32/48/64 px an, das Startmenü 256/512 — fehlt eine
# Größe, skaliert die Oberfläche eine andere hoch und das Symbol wird unscharf.
# Die Dateien sind dieselben wie im Programm (`crates/jjcad-gui/assets/`), es
# gibt also keine zweite Bildquelle.
GROESSEN="16 24 32 48 64 128 256 512"

# Der Symbolname trägt einen Abdruck des Bildes (T0113, Turn 5). KDE hält
# geladene Symbole im Speicher und liest die Datei nicht erneut, solange der
# Name gleich bleibt: nach dem neuen Markenzeichen zeigte die Taskleiste
# weiterhin das alte Bild, obwohl Datei und Programm längst das neue trugen
# (plasmashell lief seit dem Anmelden, die Symbol-Dateien waren ersetzt). Ein
# Name, der sich mit dem Bild ändert, kann keinen alten Eintrag treffen — und
# beim nächsten Markenzeichen stimmt es von selbst wieder.
MARKE="$(md5sum crates/jjcad-gui/assets/jjc-icon-512.png | cut -c1-8)"
ICON_NAME="joja-cad-$MARKE"

mkdir -p "$APPS"
# Alte Abdruck-Namen wegräumen, damit im Symbolpfad nicht mehrere Fassungen
# desselben Zeichens liegen.
rm -f "$ICONS"/*/apps/joja-cad-*.png 2>/dev/null || true

for s in $GROESSEN; do
    quelle="crates/jjcad-gui/assets/jjc-icon-$s.png"
    if [ ! -f "$quelle" ]; then
        printf 'install-desktop: %s fehlt (Markenzeichen unvollstaendig).\n' "$quelle" >&2
        exit 1
    fi
    mkdir -p "$ICONS/${s}x${s}/apps"
    cp "$quelle" "$ICONS/${s}x${s}/apps/$ICON_NAME.png"
    # Zweitname = App-ID: KDE nimmt ihn, wenn es ein Fenster keinem
    # .desktop-Eintrag zuordnen kann. Dieselbe Datei, kein zweites Bild.
    cp "$quelle" "$ICONS/${s}x${s}/apps/jjcad.png"
done

cat > "$APPS/jjcad.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=JoJa-CAD
GenericName=CAD-Zeichenprogramm
Comment=2D-CAD für Architektur- und Bauantragszeichnungen
Exec="$BIN" %f
Icon=$ICON_NAME
Terminal=false
Categories=Graphics;Engineering;
MimeType=application/x-jjcad;
StartupWMClass=jjcad
EOF

# Caches der Oberfläche neu bauen, damit das Symbol sofort erscheint. KDE führt
# zusätzlich eine eigene Symbol- und Startmenü-Datenbank (`kbuildsycoca`) und
# einen Symbol-Cache (`icon-cache.kcache`) — ohne beides zeigt die Taskleiste
# nach einem Icon-Wechsel weiter das alte Bild. `JJCAD_INSTALL_OHNE_CACHE=1`
# überspringt diesen Teil (der Test schreibt in ein Wegwerf-Verzeichnis und darf
# die Caches des angemeldeten Benutzers nicht anfassen).
if [ "${JJCAD_INSTALL_OHNE_CACHE:-}" != "1" ]; then
    command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
    command -v gtk-update-icon-cache  >/dev/null && gtk-update-icon-cache -f -t "$ICONS" 2>/dev/null || true
    rm -f "${XDG_CACHE_HOME:-$HOME/.cache}/icon-cache.kcache" 2>/dev/null || true
    for sycoca in kbuildsycoca6 kbuildsycoca5; do
        if command -v "$sycoca" >/dev/null; then "$sycoca" >/dev/null 2>&1 || true; break; fi
    done
fi

printf 'Markenzeichen: %s (%s px)\n' "$ICONS/<n>x<n>/apps/$ICON_NAME.png" "$GROESSEN"
printf 'Startmenue:    %s\n' "$APPS/jjcad.desktop"
printf 'Programm:      %s\n' "$BIN"
