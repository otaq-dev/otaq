#!/bin/zsh
# check_links.sh [README.md]: every relative link and image in the README must point at a file that git tracks
# (or that is staged). A file that exists only on this disk, like the untracked decisions/ or DESIGN.md, is a
# broken link on GitHub.
cd $(git rev-parse --show-toplevel)
f=${1:-README.md}
bad=0
for p in $(grep -oE '\]\([^)#]+\)|src="[^"]+"' $f | sed -E 's/^\]\(|\)$|^src="|"$//g' | grep -vE '^(https?:|mailto:)' | sort -u); do
  if git ls-files --error-unmatch "$p" >/dev/null 2>&1 || [[ -n $(git ls-files "$p" | head -1) ]] \
     || [[ -n $(git diff --cached --name-only -- "$p") ]]; then
    echo "ok       $p"
  else
    echo "MISSING  $p"; bad=1
  fi
done
exit $bad
