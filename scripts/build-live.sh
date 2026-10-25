#!/usr/bin/env bash
# Baut den Release-Stand und legt ihn als TESTBINARY ab.
# Nur dieses Binary starten, waehrend der Agent weiterentwickelt.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p jjcad-gui
mkdir -p "$HOME/jjcad-live"
cp target/release/jjcad "$HOME/jjcad-live/jjcad"
DATEI="$HOME/jjcad-live/jjcad"
printf 'Testbinary aktualisiert: %s  (%s)\n' "$DATEI" "$(date +%H:%M:%S)"
printf 'Starten mit:  %s/scripts/start-live.sh [datei.jjcad]\n' "$PWD"
