#!/usr/bin/env bash
# Räumt Worktrees ab, deren Zweig vollständig in master steckt.
# Jeder Worktree bringt ein eigenes target/ mit (~1 GB) — nach dem Merge unnötig.
set -uo pipefail
cd "$(dirname "$0")/.."

git worktree list --porcelain | awk '
    /^worktree / { w = substr($0, 10) }
    /^branch /   { b = substr($0, 8); sub("refs/heads/", "", b); print w "\t" b }
' | while IFS=$'\t' read -r w b; do
    [ -z "$b" ] && continue
    [ "$b" = "master" ] && continue
    case "$w" in
        */.worktrees/*) ;;
        *) continue ;;
    esac
    if git merge-base --is-ancestor "$b" master 2>/dev/null; then
        printf 'entferne %s (Zweig %s gemergt)\n' "$w" "$b"
        git worktree remove --force "$w" 2>/dev/null || true
        git branch -d "$b" >/dev/null 2>&1 || true
    fi
done
git worktree prune
du -sh .worktrees 2>/dev/null || echo "keine Worktrees mehr"
