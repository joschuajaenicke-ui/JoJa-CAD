#!/usr/bin/env bash
# Baut die Windows-Fassung von JoJa-CAD (x86_64-pc-windows-gnu) auf Linux und
# stellt daraus ein portables ZIP zusammen. Braucht kein root:
# MinGW-w64 wird als entpackte Debian-Pakete unter ~/.local/mingw abgelegt.
#
#   ./scripts/build-windows.sh [Zielordner]
#
# Standardziel: ~/JoJa-CAD/dist-win (außerhalb des Repos, damit `git status`
# sauber bleibt).
set -euo pipefail

PROJ=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$HOME/JoJa-CAD/dist-win}
MINGW=$HOME/.local/mingw
TRIPLE=x86_64-pc-windows-gnu
CC=x86_64-w64-mingw32-gcc-posix

cd "$PROJ"
VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
NAME="JoJa-CAD-$VERSION-win64-portable"
PKG="$OUT/$NAME"

# 1) Rust-Ziel ------------------------------------------------------------
if ! rustup target list --installed | grep -qx "$TRIPLE"; then
    echo "== Rust-Ziel $TRIPLE wird nachinstalliert"
    rustup target add "$TRIPLE"
fi

# 2) MinGW-w64 ohne root --------------------------------------------------
if [ ! -x "$MINGW/usr/bin/$CC" ]; then
    echo "== MinGW-w64 fehlt — entpacke Debian-Pakete nach $MINGW"
    DL=$(mktemp -d)
    ( cd "$DL"
      apt-get download gcc-mingw-w64-x86-64-posix gcc-mingw-w64-base \
          gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64 \
          mingw-w64-x86-64-dev mingw-w64-common
      mkdir -p "$MINGW"
      for f in ./*.deb; do dpkg -x "$f" "$MINGW"; done )
    rm -rf "$DL"
fi

# 3) Bauen ----------------------------------------------------------------
echo "== Baue $VERSION für Windows"
export PATH="$MINGW/usr/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$CC
cargo build --release -p jjcad-gui -p jjcad-mcp --target "$TRIPLE"

# 4) Portables Paket ------------------------------------------------------
# Schriften: Windows bringt Liberation und Quicksand nicht mit; der Kern sucht
# sie zuerst in "fonts/" neben dem Programm (crates/jjcad-core/src/fonts.rs).
FONTDIRS=(/usr/share/fonts/truetype/liberation /usr/share/fonts/truetype/quicksand)
for d in "${FONTDIRS[@]}"; do
    [ -d "$d" ] || { echo "!! Schriftordner fehlt: $d (fonts-liberation/-quicksand installieren)"; exit 1; }
done

rm -rf "$PKG"
mkdir -p "$PKG/fonts" "$PKG/beispiele"
cp "target/$TRIPLE/release/jjcad.exe" "target/$TRIPLE/release/jjcad-mcp.exe" "$PKG/"
cp "${FONTDIRS[0]}"/*.ttf "${FONTDIRS[1]}"/*.ttf "$PKG/fonts/"
cp samples/Wohnhaus.jjcad samples/Textfelder.jjcad "$PKG/beispiele/"
cp LICENSE "$PKG/LIZENZ-JoJa-CAD.txt"
cat /usr/share/doc/fonts-liberation/copyright      > "$PKG/fonts/LIZENZ-Liberation.txt"
cat /usr/share/doc/fonts-quicksand/copyright       > "$PKG/fonts/LIZENZ-Quicksand.txt"
cp "$PROJ/scripts/windows-readme.txt" "$PKG/LIESMICH.txt"

( cd "$OUT" && rm -f "$NAME.zip" && zip -qr "$NAME.zip" "$NAME" )
echo "== Fertig: $OUT/$NAME.zip  ($(du -h "$OUT/$NAME.zip" | cut -f1))"
