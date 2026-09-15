#!/usr/bin/env bash
# Reusable worktree pool for parallel lanes (issue #1680).
# Rationale, measurements and the why behind per-slot target dirs: see PR body.
set -euo pipefail

SCRIPT_REPO=$(cd "$(dirname "$0")/.." && pwd)
MAIN=$(cd "$(git -C "$SCRIPT_REPO" rev-parse --git-common-dir)/.." && pwd)
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
            git -C "$MAIN" worktree add --detach "$p" origin/main >/dev/null
            ln -sfn "$MAIN/.venv" "$p/.venv"
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
    ln -sfn "$MAIN/.venv" "$p/.venv"
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
    br="$(cat "$STATE/$slot.branch" 2>/dev/null || true)"
    rm -f "$STATE/$slot.branch"
    git -C "$p" checkout --detach origin/main >/dev/null 2>&1 || true
    if [ -n "$br" ]; then
        if ! pushed="$(git -C "$p" rev-list --count "origin/$br..$br" 2>/dev/null)"; then
            pushed=1
        fi
        if ! git -C "$p" rev-parse --verify --quiet "refs/remotes/origin/$br" >/dev/null; then
            echo "kept local branch $br: no origin/$br to verify against" >&2
        elif [ "$pushed" != "0" ]; then
            echo "kept local branch $br: has unpushed commits" >&2
        elif git -C "$p" branch -D "$br" >/dev/null 2>&1; then
            echo "deleted local branch $br"
        else
            echo "kept local branch $br: checked out in another worktree" >&2
        fi
    fi
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
        git -C "$MAIN" worktree remove --force "$d"
        echo "removed $(basename "$d")"
    done
    git -C "$MAIN" worktree prune
    rm -rf "$STATE"
    echo "pool pruned (POOL_ROOT kept: $POOL_ROOT)"
}

case "${1:-}" in
    init) shift; cmd_init "$@" ;;
    claim) shift; cmd_claim "$@" ;;
    release) shift; cmd_release "$@" ;;
    status) cmd_status ;;
    prune) cmd_prune ;;
    *)
        cat >&2 <<'USAGE'
usage: worktree_pool.sh <command>

  init [count]           create pool slots (default 4)
  claim <slot> <branch>  claim a slot, reset to origin/main on <branch>
  release <slot>         release a slot (keeps target warm)
  status                 show slot, claim state, branch, target size
  prune                  remove every clean pool worktree
USAGE
        exit 2
        ;;
esac