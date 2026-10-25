#!/usr/bin/env bash
# Entfernt Worktrees, deren Arbeit vollstaendig in master steckt und in denen
# nichts Ungespeichertes liegt. Jeder Worktree bringt ~1 GB target/ mit.
set -uo pipefail
cd "$(dirname "$0")/.."
git worktree list --porcelain | awk '
    /^worktree / { w = substr($0, 10) }
    /^branch /   { b = substr($0, 8); sub("refs/heads/", "", b); print w "\t" b }
' | while IFS=$'\t' read -r w b; do
    [ -z "$b" ] || [ "$b" = "master" ] && continue
    case "$w" in */.worktrees/*) ;; *) continue ;; esac
    # nichts Ungespeichertes?
    [ -n "$(git -C "$w" status --porcelain 2>/dev/null)" ] && continue
    # Inhalt schon in master (entweder Vorfahr oder kein Unterschied)?
    if git merge-base --is-ancestor "$b" master 2>/dev/null \
       || [ -z "$(git diff --name-only master..."$b" 2>/dev/null)" ]; then
        printf 'entferne %s (%s)\n' "$w" "$b"
        git worktree remove --force "$w" 2>/dev/null || true
        git branch -d "$b" >/dev/null 2>&1 || true
    fi
done
git worktree prune
