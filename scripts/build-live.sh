#!/usr/bin/env bash
# Baut den Release-Stand und legt ihn versioniert als Testbinary ab.
#   ~/jjcad-live/jjcad-0.2.<build>   (echte Datei, Build-Nummer = Git-Commits)
#   ~/jjcad-live/jjcad               (Symlink auf den neuesten Build)
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p jjcad-gui
BUILD="$(git rev-list --count HEAD)"
VER="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
NAME="jjcad-${VER}-b${BUILD}"
DEST="${HOME}/jjcad-live"
mkdir -p "$DEST"
if ! cp target/release/jjcad "$DEST/$NAME" 2>/dev/null; then
    # Datei laeuft gerade (Nutzer testet) -> Inhalt ist identisch, nichts zu tun
    if [ -f "$DEST/$NAME" ]; then
        printf 'Build %s laeuft gerade und ist unveraendert (%s).\n' "$BUILD" "$DEST/$NAME"
        exit 0
    fi
    printf 'Kopie nach %s fehlgeschlagen.\n' "$DEST/$NAME" >&2
    exit 1
fi
ln -sfn "$NAME" "$DEST/jjcad"
# nur die letzten 10 Builds behalten
ls -1t "$DEST"/jjcad-* 2>/dev/null | tail -n +11 | xargs -r rm -f
printf 'Build %s bereit  (%s/%s)\n' "$BUILD" "$DEST" "$NAME"
printf 'Starten mit:  scripts/start-live.sh [datei.jjcad]\n'
