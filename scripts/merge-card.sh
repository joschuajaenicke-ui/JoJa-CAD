#!/usr/bin/env bash
# Mergt den Zweig einer Karte nach master — und nimmt den Merge zurück, wenn die
# Tests rot werden. Nur der Manager ruft das auf.
#
#   scripts/merge-card.sh wt/t_1234abcd
#
set -euo pipefail
cd "$(dirname "$0")/.."

BRANCH="${1:?Zweig fehlt (z. B. wt/t_1234abcd)}"
git rev-parse --verify "$BRANCH" >/dev/null 2>&1 || { echo "Zweig '$BRANCH' gibt es nicht" >&2; exit 1; }

if [ -n "$(git status --porcelain)" ]; then
    echo "Arbeitsverzeichnis ist nicht sauber — erst aufräumen." >&2
    exit 1
fi

git checkout -q master
BEFORE="$(git rev-parse HEAD)"

if git merge --no-edit -m "Merge $BRANCH (Manager)" "$BRANCH"; then
    :
else
    echo "KONFLIKT in $BRANCH — Merge abgebrochen, Karte muss neu gebaut werden." >&2
    git merge --abort 2>/dev/null || true
    git reset --hard -q "$BEFORE"
    exit 2
fi

if cargo test -q >/tmp/merge-card-test.log 2>&1; then
    echo "OK: $BRANCH gemergt ($(git rev-parse --short HEAD)), Tests grün."
    exit 0
fi

echo "TESTS ROT nach dem Merge — zurückgerollt." >&2
tail -20 /tmp/merge-card-test.log >&2
git reset --hard -q "$BEFORE"
exit 3
