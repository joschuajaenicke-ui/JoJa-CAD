#!/usr/bin/env bash
# Liefert einen Build in den Testkanal des Nutzers.
#
# Regeln (organisatorisch, nicht verhandelbar):
#   1. NUR aus master — Worker bauen ihre Zweige selbst.
#   2. Kein Build mit ungespeicherten Aenderungen (nicht reproduzierbar).
#   3. Jeder Build traegt die Commit-Kurzform im Dateinamen. Zwei Agents koennen
#      so nie denselben Namen erzeugen; vorhandene Dateien werden nie ueberschrieben.
#   4. Ein Build muss Nachfolger des zuletzt ausgelieferten sein
#      (Vorfahren-Pruefung). Damit kann kein aelterer Zweig schon geloeste
#      Fehler zurueckschreiben.
set -euo pipefail
cd "$(dirname "$0")/.."

DEST="${HOME}/jjcad-live"
STATE="$DEST/shipped.last"

BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
if [ "${JJCAD_LIVE_FORCE:-0}" != "1" ] && [ "$BRANCH" != "master" ]; then
    printf 'build-live: nur aus master (aktuell: %s).\n' "$BRANCH" >&2
    exit 2
fi
if [ "${JJCAD_LIVE_FORCE:-0}" != "1" ] && [ -n "$(git status --porcelain)" ]; then
    printf 'build-live: Arbeitsverzeichnis ist nicht sauber — erst committen.\n' >&2
    git status --short | head -5 >&2
    exit 3
fi

# Torwaechter: ohne gruene Tests verlaesst kein Build das Haus
if [ "${JJCAD_LIVE_FORCE:-0}" != "1" ]; then
    if ! cargo test -q >/tmp/build-live-test.log 2>&1; then
        printf 'build-live: TESTS ROT — nicht ausgeliefert.\n' >&2
        tail -20 /tmp/build-live-test.log >&2
        exit 5
    fi
fi

cargo build --release -p jjcad-gui

COUNT="$(git rev-list --count HEAD)"
HASH="$(git rev-parse --short HEAD)"
VER="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
NAME="jjcad-${VER}-b${COUNT}-${HASH}"

# 4. Vorfahren-Pruefung gegen den zuletzt ausgelieferten Stand
if [ -f "$STATE" ]; then
    LAST="$(cut -d' ' -f1 "$STATE")"
    LASTNAME="$(cut -d' ' -f2- "$STATE")"
    if [ -n "$LAST" ] && ! git merge-base --is-ancestor "$LAST" HEAD 2>/dev/null; then
        printf 'build-live: ABGELEHNT. %s ist kein Nachfolger von %s (%s).\n' \
            "$HASH" "$LAST" "$LASTNAME" >&2
        printf 'Der Stand enthaelt den zuletzt ausgelieferten nicht — er wuerde\n' >&2
        printf 'geloeste Fehler zurueckbringen. Erst master nachziehen.\n' >&2
        exit 4
    fi
fi

mkdir -p "$DEST"
if [ -e "$DEST/$NAME" ]; then
    printf 'build-live: %s existiert bereits — nichts ueberschrieben.\n' "$NAME"
    ln -sfn "$NAME" "$DEST/jjcad"
    exit 0
fi

cp target/release/jjcad "$DEST/$NAME"
ln -sfn "$NAME" "$DEST/jjcad"
printf '%s %s\n' "$(git rev-parse HEAD)" "$NAME" > "$STATE"

# nur die letzten 10 Builds behalten
ls -1t "$DEST"/jjcad-* 2>/dev/null | tail -n +11 | xargs -r rm -f

printf 'Build %s bereit  (%s/%s)\n' "$COUNT" "$DEST" "$NAME"
printf 'Im Programm sichtbar als: %s\n' "$NAME"
printf 'Starten mit:  scripts/start-live.sh [datei.jjcad]\n'
