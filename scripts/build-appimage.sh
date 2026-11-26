#!/usr/bin/env bash
# Baut ein AppImage (portable Linux-Fassung, x86_64) aus dem Release-Build.
#
#   ./scripts/build-appimage.sh [Zielordner]
#
# Voraussetzung: `cargo build --release -p jjcad-gui -p jjcad-mcp` ist gelaufen
# und `appimagetool` liegt im PATH oder in $APPIMAGETOOL.
# Standardziel: ~/JoJa-CAD/dist (außerhalb des Repos, damit `git status` sauber
# bleibt).
set -euo pipefail

PROJ="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$HOME/JoJa-CAD/dist}"
TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"

if [ -z "$TOOL" ] || [ ! -x "$TOOL" ]; then
    echo "appimagetool fehlt. Herunterladen:" >&2
    echo "  curl -L -o ~/.local/bin/appimagetool \\" >&2
    echo "    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" >&2
    echo "  chmod +x ~/.local/bin/appimagetool" >&2
    exit 1
fi

VERSION=$(grep -m1 '^version = ' "$PROJ/Cargo.toml" | cut -d'"' -f2)
BIN="$PROJ/target/release"
for f in jjcad jjcad-mcp; do
    [ -x "$BIN/$f" ] || { echo "target/release/$f fehlt — erst bauen" >&2; exit 1; }
done

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
APPDIR="$WORK/JoJa-CAD.AppDir"

mkdir -p "$APPDIR/usr/bin/fonts" "$APPDIR/usr/share/icons/hicolor"
cp "$BIN/jjcad" "$BIN/jjcad-mcp" "$APPDIR/usr/bin/"
# Schriften neben das Programm: dort sucht sie der Kern zuerst.
cp "$PROJ"/packaging/fonts/*.ttf "$APPDIR/usr/bin/fonts/"

# Lizenztext und README gehören mit ins Paket: die GPL verlangt den Lizenztext
# bei jeder Weitergabe der Binärfassung, und das AppImage ist eine einzelne Datei.
mkdir -p "$APPDIR/usr/share/doc/joja-cad"
cp "$PROJ/LICENSE" "$PROJ/README.md" "$APPDIR/usr/share/doc/joja-cad/"

for s in 16 24 32 48 64 128 256 512; do
    mkdir -p "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps"
    cp "$PROJ/crates/jjcad-gui/assets/jjc-icon-$s.png" \
       "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps/jjcad.png"
done
# Das Symbol im Wurzelverzeichnis ist Pflicht für appimagetool.
cp "$PROJ/crates/jjcad-gui/assets/jjc-icon-256.png" "$APPDIR/jjcad.png"

cat > "$APPDIR/jjcad.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=JoJa-CAD
Comment=2D-CAD für Architektur und Bauantragszeichnungen
Exec=jjcad
Icon=jjcad
Categories=Graphics;
Terminal=false
DESKTOP

# Der MCP-Server steckt mit im Paket; `--mcp` startet ihn statt der Oberfläche.
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
if [ "$1" = "--mcp" ]; then
    shift
    exec "$HERE/usr/bin/jjcad-mcp" "$@"
fi
exec "$HERE/usr/bin/jjcad" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

mkdir -p "$OUT"
ZIEL="$OUT/JoJa-CAD-$VERSION-x86_64.AppImage"
ARCH=x86_64 "$TOOL" --no-appstream "$APPDIR" "$ZIEL"
echo "AppImage: $ZIEL ($(du -h "$ZIEL" | cut -f1))"
