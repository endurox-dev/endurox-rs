#!/usr/bin/env bash

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_DIR="$THIS_DIR/conf"
BIN_DIR="$THIS_DIR/bin"
PROJECT_DIR="$(cd "$THIS_DIR/../.." && pwd)"
SCENARIO="${1:-tpcall}"
RUN_ID="$((($$ % 9000) + 1000))"

export NDRX_RS_IT_QPREFIX="/nri${RUN_ID}"
export NDRX_RS_IT_DPID="/tmp/ndrxd-rs-it-${RUN_ID}.pid"
export NDRX_RS_IT_RNDK="rsit${RUN_ID}"
export NDRX_RS_IT_IPCKEY="$((50000 + RUN_ID))"

mkdir -p "$BIN_DIR"

pushd "$CONF_DIR" >/dev/null
. ./setndrx
popd >/dev/null

cargo build --manifest-path "$PROJECT_DIR/Cargo.toml" --bin rs_it_server --bin rs_it_client
cp "$PROJECT_DIR/target/debug/rs_it_server" "$BIN_DIR/rs_it_server"
chmod +x "$BIN_DIR/rs_it_server"

cleanup() {
    xadmin stop -c -y >/dev/null 2>&1 || true
}

dump_logs() {
    for f in /tmp/xadmin-rs-it.log /tmp/ndrxd-rs-it.log /tmp/ndrx-rs-it.log /tmp/rs_it_server.log; do
        if [ -f "$f" ]; then
            echo "===== $f (tail) ====="
            tail -n 80 "$f" || true
        fi
    done
}

trap cleanup EXIT

xadmin stop -c -y >/dev/null 2>&1 || true

if ! xadmin start -y; then
    dump_logs
    exit 1
fi

sleep 2

if ! "$PROJECT_DIR/target/debug/rs_it_client" "$SCENARIO"; then
    dump_logs
    exit 1
fi

xadmin psc
xadmin stop -c -y
trap - EXIT

echo "Test OK: $SCENARIO"
