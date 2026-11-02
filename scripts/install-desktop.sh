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
        if [ -x "$kandidat" ]; then BIN="$(readlink -f "$kandidat")"; break; fi
    done
fi
[ -n "$BIN" ] && [ -x "$BIN" ] || {
    printf 'install-desktop: kein Programm gefunden (erst bauen oder Pfad angeben).\n' >&2
    exit 1
}

DATEN="${XDG_DATA_HOME:-$HOME/.local/share}"
ICONS="$DATEN/icons/hicolor"
APPS="$DATEN/applications"

mkdir -p "$ICONS/512x512/apps" "$ICONS/32x32/apps" "$APPS"
cp crates/jjcad-gui/assets/jjc-icon-512.png "$ICONS/512x512/apps/jjcad.png"
cp crates/jjcad-gui/assets/jjc-icon-32.png  "$ICONS/32x32/apps/jjcad.png"

cat > "$APPS/jjcad.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=JoJa-CAD
GenericName=CAD-Zeichenprogramm
Comment=2D-CAD für Architektur- und Bauantragszeichnungen
Exec="$BIN" %f
Icon=jjcad
Terminal=false
Categories=Graphics;Engineering;
MimeType=application/x-jjcad;
StartupWMClass=jjcad
EOF

# Caches der Oberfläche neu bauen, damit das Symbol sofort erscheint.
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
command -v gtk-update-icon-cache  >/dev/null && gtk-update-icon-cache -f -t "$ICONS" 2>/dev/null || true

printf 'Markenzeichen: %s\n' "$ICONS/512x512/apps/jjcad.png"
printf 'Startmenue:    %s\n' "$APPS/jjcad.desktop"
printf 'Programm:      %s\n' "$BIN"