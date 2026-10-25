#!/usr/bin/env bash
# Startet die Testversion. Zeigt vorher die Build-Nummer an, damit klar ist,
# welcher Stand laeuft.
set -euo pipefail
DEST="${HOME}/jjcad-live"
BIN="$DEST/jjcad"
[ -x "$BIN" ] || { echo "Kein Testbinary - erst scripts/build-live.sh laufen lassen."; exit 1; }
NAME="$(basename "$(readlink -f "$BIN")")"
printf 'Starte %s  (%s)\n' "$NAME" "$(date +%H:%M:%S)"
exec "$BIN" "$@"
