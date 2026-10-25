#!/usr/bin/env bash
# Liefert die Arbeit EINER Karte aus: Karten-Zweig -> master -> Testkanal.
#
#   ./scripts/ship-card.sh t_1234abcd
#
# Der Worker ruft das nach dem letzten Commit selbst auf; er bekommt die
# Build-Nummer, die der Nutzer testet, direkt als Ausgabe. Damit gilt:
#   - kein Build aus einem Feature-Zweig (es wird immer aus master gebaut),
#   - jeder Build enthaelt alle vorherigen Fixes (Vorfahren-Pruefung),
#   - kein Code auf master, dessen Tests rot sind (merge-card prueft),
#   - die Karte darf erst nach dieser Ausgabe nach `review`.
#
# Voraussetzungen: Arbeit ist im Worktree der Karte committet, der Zweig
# enthaelt den aktuellen master, das Hauptverzeichnis ist sauber.
set -uo pipefail
# Immer im HAUPTVERZEICHNIS arbeiten: aus einem Worktree heraus aufgerufen
# liefert --git-common-dir das .git des Hauptbaums (dort liegt master).
cd "$(dirname "$0")/.." || exit 1
MAIN="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"
cd "$MAIN" || exit 1

ID="${1:-}"
if ! [[ "$ID" =~ ^t_[0-9a-f]+$ ]]; then
    echo "Aufruf: ./scripts/ship-card.sh t_1234abcd" >&2
    exit 1
fi

# 1. Zweig der Karte bestimmen: Worktree, sonst wt/ bzw. fix/
BRANCH=""
WT=""
while IFS=$'\t' read -r w b; do
    case "$w" in "*/$ID") BRANCH="$b"; WT="$w" ;; esac
done < <(git worktree list --porcelain | awk '
    /^worktree / { w = substr($0, 10) }
    /^branch /   { b = substr($0, 8); sub("refs/heads/", "", b); print w "\t" b }')
if [ -z "$BRANCH" ]; then
    for cand in "wt/$ID" "fix/$ID"; do
        if git rev-parse --verify --quiet "$cand" >/dev/null; then BRANCH="$cand"; break; fi
    done
fi
if [ -z "$BRANCH" ]; then
    echo "Kein Zweig fuer $ID gefunden (weder Worktree noch wt/$ID bzw. fix/$ID)." >&2
    exit 2
fi
if [ "$BRANCH" = "master" ]; then
    echo "Auf master wird nicht ausgeliefert: erst eigenen Zweig anlegen und dort committen." >&2
    exit 2
fi

# 2. Arbeitsstand der Karte muss committet und auf master-Stand sein
if [ -n "$WT" ] && [ -n "$(git -C "$WT" status --porcelain 2>/dev/null)" ]; then
    echo "Im Worktree von $ID liegen ungespeicherte Aenderungen — erst committen." >&2
    git -C "$WT" status --short | head -5 >&2
    exit 3
fi
if ! git merge-base --is-ancestor master "$BRANCH"; then
    echo "ABGELEHNT: $BRANCH enthaelt den aktuellen master nicht." >&2
    echo "Erst im Worktree 'git merge master' (Konflikte loesen, Tests gruen), dann erneut." >&2
    exit 4
fi
if [ "$(git rev-list --count "master..$BRANCH")" = "0" ]; then
    echo "Auf $BRANCH liegt kein Commit, der nicht schon in master steckt — nichts auszuliefern." >&2
    exit 5
fi

# 3. Merge nach master (Tests gruen, sonst automatischer Rueckbau)
BEFORE="$(git rev-parse master)"
if ! MERGE_OUT="$(./scripts/merge-card.sh "$BRANCH" 2>&1)"; then
    printf '%s\n' "$MERGE_OUT" >&2
    echo "Auslieferung abgebrochen: Merge/Tests nicht in Ordnung." >&2
    exit 6
fi

# 4. Build aus master fuer den Testkanal
export JJCAD_KEEP_BRANCH="$BRANCH"
if ! BUILD_OUT="$(./scripts/build-live.sh 2>&1)"; then
    printf '%s\n' "$BUILD_OUT" >&2
    git reset --hard -q "$BEFORE"
    echo "Build fehlgeschlagen — master auf $(git rev-parse --short "$BEFORE") zurueckgerollt." >&2
    exit 7
fi
NAME="$(printf '%s' "$BUILD_OUT" | grep -oE 'jjcad-[0-9.]+-s[0-9]+-[0-9a-f]+' | head -1)"
if [ -z "$NAME" ]; then
    printf '%s\n' "$BUILD_OUT" >&2
    echo "Build lief, aber die Build-Nummer war nicht lesbar. Karte NICHT auf review setzen." >&2
    exit 8
fi
SEQ="$(printf '%s' "$NAME" | sed -E 's/.*-s([0-9]+)-[0-9a-f]+$/\1/')"
HASH="${NAME##*-}"

cat <<EOF
AUSGELIEFERT: ${NAME}
Pfad:         ${HOME}/jjcad-live/${NAME}
Symlink:      ${HOME}/jjcad-live/jjcad
master:       $(git rev-parse --short master)

So in die Karte schreiben (juengster Kommentar, erste Zeile beginnt mit TEST):

TEST s${SEQ}-${HASH} — <ein Satz: was jetzt anders ist>
Umgesetzt: <2-4 Stichpunkte, was genau geaendert wurde>
Testen: <2-4 konkrete Schritte in der App>
Erwartung: <was man sehen muss>
EOF
exit 0
