#!/usr/bin/env bash
# Nimmt eine Karte zurück in die Warteschlange — und entsorgt dabei ihren alten
# Worktree und Zweig. Damit startet der nächste Lauf garantiert auf dem
# aktuellen master und nicht auf einem veralteten Stand.
#
#   scripts/reopen-card.sh t_1234abcd "Grund / naechster Schritt"
#
set -uo pipefail
cd "$(dirname "$0")/.."

ID="${1:?Karten-ID fehlt}"
REASON="${2:-Zurueck in die Warteschlange}"

hermes kanban reopen-review "$1" --reason "$2" >/dev/null 2>&1 \
    || hermes kanban comment "$1" "$2" >/dev/null 2>&1

# Alten Arbeitsstand verwerfen: sonst arbeitet der naechste Lauf auf altem master
for w in $(git worktree list --porcelain | awk -v id="$1" '
        /^worktree / { w = substr($0, 10) }
        /^branch /   { b = substr($0, 8); sub("refs/heads/", "", b); if (w ~ "/"id"$") print w "\t" b }' | cut -f1); do
    echo "entferne veralteten Worktree: $w"
    git worktree remove --force "$w" 2>/dev/null || true
done

for b in $(git branch --format='%(refname:short)' | grep -E "(^|/)$1\b"); do
    printf 'Zweig %s wird verworfen (alter Stand)\n' "$b"
    git branch -D "$b" >/dev/null 2>&1 || true
done
git worktree prune

hermes kanban assign "$1" none >/dev/null 2>&1
printf '%s ist wieder in der Warteschlange; naechster Lauf startet auf master %s.\n' \
    "$1" "$(git rev-parse --short master)"
