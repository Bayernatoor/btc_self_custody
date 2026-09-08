#!/usr/bin/env bash
# Restack the branch queue after master moves.
#
# Every merge into master rewrites the merged commits' SHAs (the repo
# rebase-merges to keep master linear), so each queued branch is left parented
# on a commit that no longer exists upstream and must be replayed.
#
# Doing that by hand went wrong three times in one session, each differently:
#   - plain `git rebase <newbase>` replays the OLD base too, because the merge
#     base is further back than you think. Always `--onto`.
#   - re-pointing the base branch inside the loop orphans the commit you just
#     amended.
#   - creating the next branch from whatever HEAD happened to be, rather than
#     from the intended tip.
# So this reads every parent from git BEFORE moving anything, then replays each
# branch onto the previous one's new tip.
#
# RUN IT FROM A COPY OUTSIDE THE WORKTREE. This script's own branch is in the
# queue below, so executing it in place means git rewrites the file while bash
# is still reading it, and bash reads scripts incrementally rather than all at
# once. Extract first:
#
#   git show chore/restack-script:scripts/restack.sh > /tmp/restack.sh
#   bash /tmp/restack.sh --list     # show the queue and what would happen
#   bash /tmp/restack.sh            # do it
#
# Signing is disabled per-command because rebase cannot sign in a sandbox.
set -euo pipefail

LOG="${TMPDIR:-/tmp}/restack.log"

# A dirty tree makes `git rebase` refuse, but it would refuse partway through
# the loop with some branches already moved and others not. Fail before
# touching anything instead.
if [ -n "$(git status --porcelain)" ]; then
  echo "working tree is not clean; commit or stash first:" >&2
  git status --short >&2
  exit 1
fi

# Merge order. Edit as branches land; drop the ones already in master.
BRANCHES=(
  fix/price-refresh-gap
  fix/dead-input-value
  fix/almanac-data-accuracy
  fix/archives-accuracy
  fix/input-validation
  fix/chart-zoom-axis
  docs/local-verification-norm
  fix/price-history-diagnosis
  chore/restack-script
  chore/drop-value-flow-chart
  fix/stamps-accuracy
)
# Branches that hang off something other than the previous entry, as
# name:parent-branch. Empty now: fix/stamps-accuracy used to sit off
# chore/chart-cleanup while it was held for a decision, and moved into the
# queue proper once that decision was made. Keep the mechanism, because the
# next held branch will want it.
SIBLINGS=()

live=() ; for b in "${BRANCHES[@]}"; do
  git rev-parse --verify -q "$b" >/dev/null || { echo "skip (gone): $b"; continue; }
  if git merge-base --is-ancestor "$b" master 2>/dev/null; then
    echo "skip (already in master): $b"; continue
  fi
  live+=("$b")
done

declare -A PARENT
for b in "${live[@]}"; do PARENT[$b]=$(git rev-parse "$b^"); done
for s in "${SIBLINGS[@]}"; do
  n="${s%%:*}"
  git rev-parse --verify -q "$n" >/dev/null && PARENT[$n]=$(git rev-parse "$n^")
done

if [ "${1:-}" = "--list" ]; then
  echo "master: $(git log --oneline -1 master | cut -c1-60)"
  base=master
  for b in "${live[@]}"; do
    printf "  %-34s replay %s onto %s\n" "$b" "$(git rev-parse --short "$b")" "$base"
    base=$b
  done
  for s in "${SIBLINGS[@]}"; do
    printf "  %-34s replay onto %s (sibling)\n" "${s%%:*}" "${s##*:}"
  done
  exit 0
fi

start=$(git rev-parse --abbrev-ref HEAD)
failed=0
base=master
for b in "${live[@]}"; do
  git checkout -q "$b"
  if git -c commit.gpgsign=false rebase --onto "$base" "${PARENT[$b]}" "$b" >"$LOG" 2>&1; then
    printf "  %-34s -> %s\n" "$b" "$(git rev-parse --short "$b")"
  else
    echo "CONFLICT restacking $b:"; grep -i conflict "$LOG" | head -5
    git rebase --abort; git checkout -q "$start"
    echo "aborted; nothing moved from $b onward"; exit 1
  fi
  base=$b
done

for s in "${SIBLINGS[@]}"; do
  n="${s%%:*}"; p="${s##*:}"
  git rev-parse --verify -q "$n" >/dev/null || continue
  git checkout -q "$n"
  if git -c commit.gpgsign=false rebase --onto "$p" "${PARENT[$n]}" "$n" >"$LOG" 2>&1; then
    printf "  %-34s -> %s (onto %s)\n" "$n" "$(git rev-parse --short "$n")" "$p"
  else
    echo "CONFLICT restacking sibling $n"; git rebase --abort; failed=1
  fi
done

git checkout -q "$start"
if [ "$failed" != 0 ]; then
  echo "one or more siblings did not restack; fix those before gating" >&2
  exit 1
fi
echo
echo "restacked. now gate the tip:"
echo "  cargo fmt --check && cargo clippy --features ssr --all-targets -- -D warnings"
echo "  cargo test --features ssr"
echo "  cargo check --no-default-features --features hydrate --target wasm32-unknown-unknown --lib"
