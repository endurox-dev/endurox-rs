#!/usr/bin/env bash

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_DIR="$THIS_DIR/conf"
BIN_DIR="$THIS_DIR/bin"
PROJECT_DIR="$(cd "$THIS_DIR/../.." && pwd)"
RUN_ID="$((($$ % 9000) + 1000))"

export NDRX_RS_EXT_QPREFIX="/nre${RUN_ID}"
export NDRX_RS_EXT_DPID="/tmp/ndrxd-rs-ext-${RUN_ID}.pid"
export NDRX_RS_EXT_RNDK="rsext${RUN_ID}"
export NDRX_RS_EXT_IPCKEY="$((60000 + RUN_ID))"

mkdir -p "$BIN_DIR"

pushd "$CONF_DIR" >/dev/null
. ./setndrx
popd >/dev/null

cargo build --manifest-path "$PROJECT_DIR/Cargo.toml" --bin rs_it_ext_server --bin rs_it_ext_client
cp "$PROJECT_DIR/target/debug/rs_it_ext_server" "$BIN_DIR/rs_it_ext_server"
chmod +x "$BIN_DIR/rs_it_ext_server"

cleanup() {
    xadmin stop -c -y >/dev/null 2>&1 || true
}

dump_logs() {
    for f in /tmp/xadmin-rs-ext.log /tmp/ndrxd-rs-ext.log /tmp/ndrx-rs-ext.log /tmp/rs_it_ext_server.log; do
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

if ! "$PROJECT_DIR/target/debug/rs_it_ext_client"; then
    dump_logs
    exit 1
fi

xadmin psc
xadmin stop -c -y
trap - EXIT

echo "Test OK: xatmi server extensions"
