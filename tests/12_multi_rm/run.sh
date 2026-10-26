#!/usr/bin/env bash
set -euo pipefail
export CDPATH="${CDPATH:-}"
THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_DIR="$THIS_DIR/conf"
BIN_DIR="$THIS_DIR/bin"
PROJECT_DIR="$(cd "$THIS_DIR/../.." && pwd)"
CRATE_FEATURE="${1:-}"
if [ -f ~/ndrx_home ]; then
    . ~/ndrx_home
fi
mkdir -p "$BIN_DIR" "$THIS_DIR/log"
rm -f "$CONF_DIR/app.ini" "$CONF_DIR/settest1"
find "$THIS_DIR/log" -type f -exec rm -f {} +
pushd "$THIS_DIR" >/dev/null
xadmin provision -d -vaddubf=test.fd -vtimeout=60 -vmsgmax=10 -vmsgsizemax=524288 -vndrxlev=2 -vubflev=2 -vtplev=2
popd >/dev/null
cp "$PROJECT_DIR/tests/ubftab/test.fd" "$THIS_DIR/ubftab/test.fd"
pushd "$CONF_DIR" >/dev/null
. ./settest1
popd >/dev/null
export NDRX_CCONFIG="$CONF_DIR"
export NDRX_CCTAG=RS_A/RS_B
case "$(uname -s)" in Darwin) TEST_LIBEXT=dylib; shared=(-dynamiclib);; *) TEST_LIBEXT=so; shared=(-shared);; esac
export TEST_RM_DRIVER="$BIN_DIR/librstest.$TEST_LIBEXT"
# The fixture supplies distinct provider-owned connection handles for each RM.
${CC:-cc} -fPIC "${shared[@]}" $(pkg-config --cflags atmisrvinteg) "$THIS_DIR/src/driver.c" \
    $(pkg-config --libs atmisrvinteg) -o "$TEST_RM_DRIVER"
mkdir -p "$THIS_DIR/RM1" "$THIS_DIR/RM2"
if [ -n "$CRATE_FEATURE" ]; then
    cargo build --manifest-path "$THIS_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target" --features "$CRATE_FEATURE"
else
    cargo build --manifest-path "$THIS_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target"
fi
cp "$PROJECT_DIR/target/debug/rs_rm_server" "$BIN_DIR/rs_rm_server"
cleanup() {
    xadmin stop -c -y >/dev/null 2>&1 || true
}
dump_logs() {
    for f in "$THIS_DIR"/log/*; do
        if [ -f "$f" ]; then tail -n 80 "$f"; fi
    done
}
trap cleanup EXIT
xadmin stop -c -y >/dev/null 2>&1 || true
if ! xadmin start -y; then dump_logs; exit 1; fi
echo "Running 12_multi_rm (${CRATE_FEATURE:-default}): ${NDRX_TEST_DURATATION:-10}s per scenario"
if ! "$PROJECT_DIR/target/debug/rs_rm_client"; then dump_logs; exit 1; fi
xadmin stop -c -y
trap - EXIT
if grep -R 'TESTERROR' "$THIS_DIR/log"; then dump_logs; exit 1; fi
echo "Test OK: 12_multi_rm (${CRATE_FEATURE:-default})"
