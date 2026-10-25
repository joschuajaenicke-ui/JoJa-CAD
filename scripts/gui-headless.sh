#!/usr/bin/env bash
# Startet JoJa-CAD auf einem EIGENEN, unsichtbaren Display (Xvfb).
# Niemals auf dem Bildschirm des Nutzers starten — dort erscheint nur, was er
# selbst ueber scripts/start-live.sh startet.
#
#   scripts/gui-headless.sh [datei.jjcad] [--shot bild.png] [--demo "arch,door"] [--wait 7]
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

BIN="${BIN:-target/release/jjcad}"
DATEI="samples/Wohnhaus.jjcad"
SHOT=""
DEMO="${JJCAD_DEMO_UI:-}"
WAIT=7

while [ $# -gt 0 ]; do
    case "$1" in
        --shot) SHOT="$2"; shift 2 ;;
        --demo) DEMO="$2"; shift 2 ;;
        --wait) WAIT="$2"; shift 2 ;;
        *) DATEI="$1"; shift ;;
    esac
done

DISPLAY_NR="${JJCAD_DISPLAY:-:97}"
umask 077

# Xvfb starten, falls das Display noch nicht existiert
XVFB_PID=""
if ! DISPLAY="$DISPLAY_NR" xdpyinfo >/dev/null 2>&1; then
    Xvfb "$DISPLAY_NR" -screen 0 1600x1000x24 >/dev/null 2>&1 &
    XVFB_PID=$!
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        DISPLAY="$DISPLAY_NR" xdpyinfo >/dev/null 2>&1 && break
        sleep 0.3
    done
fi

cleanup() {
    [ -n "${APP_PID:-}" ] && kill "$APP_PID" 2>/dev/null
    [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null
}
trap cleanup EXIT

env -u WAYLAND_DISPLAY DISPLAY="$DISPLAY_NR" LIBGL_ALWAYS_SOFTWARE=1 \
    ${DEMO:+JJCAD_DEMO_UI="$DEMO"} "$BIN" "$DATEI" >/tmp/jjcad-headless.log 2>&1 &
APP_PID=$!

sleep "$WAIT"
if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "Start fehlgeschlagen:" >&2
    tail -5 /tmp/jjcad-headless.log >&2
    exit 1
fi

if [ -n "$SHOT" ]; then
    WID="$(DISPLAY="$DISPLAY_NR" xwininfo -root -children 2>/dev/null \
        | grep -oE '0x[0-9a-f]+ "JoJa[^"]*"' | head -1 | cut -d' ' -f1)"
    if [ -z "$WID" ]; then
        echo "Kein Fenster gefunden" >&2
        exit 1
    fi
    DISPLAY="$DISPLAY_NR" import -window "$WID" "$SHOT"
    echo "Screenshot: $SHOT (Display $DISPLAY_NR)"
else
    echo "Laeuft auf $DISPLAY_NR (PID $APP_PID) — sichtbar nur im Xvfb"
    sleep "${JJCAD_HOLD:-0}"
fi
