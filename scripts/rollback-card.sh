#!/usr/bin/env bash
# Nimmt die Änderungen einer Karte wieder aus master heraus.
# Jede Karte wurde als ein Merge-Commit übernommen ("Merge <zweig> (Manager)"),
# deshalb genügt ein Revert dieses einen Commits.
#
#   scripts/rollback-card.sh t_1234abcd
#
set -uo pipefail
cd "$(dirname "$0")/.."

ID="${1:?Karten-ID fehlt}"
[[ "$ID" =~ ^t_[0-9a-f]+$ ]] || { echo "Ungültige Karten-ID: $ID" >&2; exit 1; }

if [ -n "$(git status --porcelain)" ]; then
    echo "Arbeitsverzeichnis ist nicht sauber — erst aufräumen." >&2
    exit 1
fi

git checkout -q master
MERGE="$(git log --merges --format='%H %s' master | grep -E "Merge (wt|fix)/${ID}(b)? " | head -1 | awk '{print $1}')"
if [ -z "$MERGE" ]; then
    # Frueher wurde per fast-forward gemergt: dann die Commits der Karte einzeln zuruecknehmen
    COMMITS="$(git log --format='%H %s' master | grep -E "(^| )$ID\b" | awk '{print $1}')"
    if [ -z "$COMMITS" ]; then
        echo "Kein Commit fuer $ID in master gefunden." >&2
        exit 2
    fi
    for c in $COMMITS; do
        printf 'Revert %s\n' "$(git log -1 --format='%h %s' "$c")"
        if ! git revert -m 1 --no-edit "$c" >/tmp/rollback.log 2>&1; then
            echo "Konflikt beim Revert von $c" >&2
            git revert --abort 2>/dev/null || true
            exit 3
        fi
    done
    if cargo test -q >/tmp/rollback-test.log 2>&1; then
        printf 'Zurueckgenommen (%s), Tests gruen. Ausliefern mit ./scripts/build-live.sh\n' "$(git rev-parse --short HEAD)"
        exit 0
    fi
    echo "Tests rot nach dem Revert — pruefen mit git log." >&2
    exit 4
fi

printf 'Revert von %s (%s)\n' "$(git log -1 --format='%h %s' "$MERGE")" "$ID"
if ! git revert -m 1 --no-edit "$MERGE" >/tmp/rollback.log 2>&1; then
    echo "Revert ging nicht sauber durch (Konflikt). Details: /tmp/rollback.log" >&2
    git revert --abort 2>/dev/null || true
    exit 3
fi

if cargo test -q >/tmp/rollback-test.log 2>&1; then
    printf 'Zurückgenommen, Tests grün (%s).\n' "$(git rev-parse --short HEAD)"
    printf 'Jetzt ausliefern:  ./scripts/build-live.sh\n'
else
    echo "Tests rot nach dem Revert — zurückgerollt." >&2
    tail -20 /tmp/rollback-test.log >&2
    git reset --hard -q HEAD~1
    exit 4
fi
