#!/bin/zsh
# stop.sh: stop the demo's agents (by stopping its service), the app and the Vite server. Keeps $OTAQ_DEMO on disk
# so a reshoot can reuse the board; `rm -rf /tmp/otaqdemo` removes it for good.
DEMO=${OTAQ_DEMO:-/tmp/otaqdemo}
REPO=$(git -C ${0:A:h} rev-parse --show-toplevel)
OTAQ_HOME=$DEMO/home $REPO/target/debug/otaq daemon stop
pkill -f "$REPO/target/debug/otaq-app" 2>/dev/null
pkill -f "$REPO/crates/otaq-desktop/node_modules/.bin/vite" 2>/dev/null
true
