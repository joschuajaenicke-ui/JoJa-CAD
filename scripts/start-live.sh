#!/usr/bin/env bash
# Startet die Testversion. Der Agent baut/startet nur seine eigene Kopie
# (Name jjcad-agent), diese Instanz bleibt unangetastet.
set -euo pipefail
BIN="$HOME/jjcad-live/jjcad"
[ -x "$BIN" ] || { echo "Kein Testbinary - erst scripts/build-live.sh laufen lassen."; exit 1; }
exec "$BIN" "$@"
