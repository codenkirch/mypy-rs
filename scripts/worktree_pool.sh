#!/usr/bin/env bash
# Reusable worktree pool for parallel lanes (issue #1680).
# Rationale, measurements and the why behind per-slot target dirs: see PR body.
set -euo pipefail

REPO=$(cd "$(dirname "$0")/.." && pwd)
POOL_ROOT="${POOL_ROOT:-/private/tmp/mypy-rs-pool}"
STATE="$POOL_ROOT/.state"

slot_path() { printf '%s/%s' "$POOL_ROOT" "$1"; }

cmd_init() {
    want="${1:-4}"
    mkdir -p "$POOL_ROOT" "$STATE"
    i=1
    while [ "$i" -le "$want" ]; do
        s="w$i"
        p="$(slot_path "$s")"
        if [ ! -d "$p" ]; then
            git -C "$REPO" worktree add --detach "$p" origin/main >/dev/null
            ln -sfn "$REPO/.venv" "$p/.venv"
            echo "created $s -> $p"
        else
            echo "exists  $s -> $p"
        fi
        i=$((i + 1))
    done
    echo "pool ready at $POOL_ROOT (state in $STATE)"
}

is_claimed() { [ -f "$STATE/$1.branch" ]; }

cmd_claim() {
    slot="${1:?usage: claim <slot> <branch>}"
    branch="${2:?usage: claim <slot> <branch>}"
    p="$(slot_path "$slot")"
    [ -d "$p" ] || { echo "no such slot: $slot" >&2; exit 1; }
    if is_claimed "$slot" && [ "$(cat "$STATE/$slot.branch")" != "$branch" ]; then
        echo "slot $slot is claimed by $(cat "$STATE/$slot.branch")" >&2
        exit 1
    fi
    git -C "$p" fetch origin -q
    git -C "$p" checkout -B "$branch" origin/main >/dev/null 2>&1
    git -C "$p" clean -xdfq -e target -e .venv
    printf '%s\n' "$branch" > "$STATE/$slot.branch"
    echo "$p"
}

cmd_release() {
    slot="${1:?usage: release <slot>}"
    p="$(slot_path "$slot")"
    [ -d "$p" ] || { echo "no such slot: $slot" >&2; exit 1; }
    if [ -n "$(git -C "$p" status --porcelain)" ]; then
        echo "refusing to release $slot: uncommitted changes present" >&2
        git -C "$p" status --short >&2
        exit 1
    fi
    rm -f "$STATE/$slot.branch"
    git -C "$p" checkout --detach origin/main >/dev/null 2>&1 || true
    echo "released $slot (target kept warm)"
}

cmd_status() {
    printf '%-4s %-9s %-34s %s\n' SLOT STATE BRANCH TARGET
    for d in "$POOL_ROOT"/w*; do
        [ -d "$d" ] || continue
        s="$(basename "$d")"
        if is_claimed "$s"; then
            st=claimed; br="$(cat "$STATE/$s.branch")"
        else
            st=free; br='-'
        fi
        sz=$(du -sh "$d/target" 2>/dev/null | cut -f1 || echo '-')
        printf '%-4s %-9s %-34s %s\n' "$s" "$st" "$br" "${sz:--}"
    done
}

cmd_prune() {
    for d in "$POOL_ROOT"/w*; do
        [ -d "$d" ] || continue
        if [ -n "$(git -C "$d" status --porcelain)" ]; then
            echo "skipping $(basename "$d"): uncommitted changes" >&2
            continue
        fi
        git -C "$REPO" worktree remove --force "$d"
        echo "removed $(basename "$d")"
    done
    git -C "$REPO" worktree prune
    rm -rf "$STATE"
    echo "pool pruned (POOL_ROOT kept: $POOL_ROOT)"
}

case "${1:-}" in
    init) shift; cmd_init "$@" ;;
    claim) shift; cmd_claim "$@" ;;
    release) shift; cmd_release "$@" ;;
    status) cmd_status ;;
    prune) cmd_prune ;;
    *) sed -n '2,3p' "$0"; exit 2 ;;
esac