#!/usr/bin/env bash
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$TEST_DIR/../.." && pwd)"
if [ -f "$HOME/ndrx_home" ]; then
    export CDPATH="${CDPATH:-}"
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}"
    export DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-}"
    . "$HOME/ndrx_home"
fi
cd "$TEST_DIR"
mkdir -p log
xadmin provision -d -vaddubf=../ubftab/test.fd -vtimeout=10 >/dev/null
cd conf
. ./settest1
cd "$TEST_DIR"
export FLDTBLDIR="$PROJECT_DIR/tests/ubftab"
export FIELDTBLS=test.fd
export NDRX_DEBUG_STR="file=$TEST_DIR/log/tpscript.log ndrx=2 ubf=2"

plugin="${ENDUROX_TPSCRIPT_PLUGIN:-}"
if [ -z "$plugin" ]; then
    plugin="$(python3 "$TEST_DIR/find_plugin.py")"
fi
if [ -z "$plugin" ]; then
    for candidate in "$PROJECT_DIR"/../endurox-python/build/lib.*/endurox/_endurox*.so \
        "$PROJECT_DIR"/../endurox-python/src/endurox/_endurox*.so; do
        if [ -f "$candidate" ]; then plugin="$candidate"; break; fi
    done
fi

run_mode() {
    cargo build --manifest-path "$TEST_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target/tpscript-it" "$@"
    NDRX_PLUGINS= "$PROJECT_DIR/target/tpscript-it/debug/rs-it-tpscript" noplugin
    if [ -n "$plugin" ]; then
        echo "Scripting plugin: $plugin"
        package_dir="$(cd "$(dirname "$plugin")/.." && pwd)"
        PYTHONPATH="$package_dir${PYTHONPATH:+:$PYTHONPATH}" NDRX_PLUGINS="$plugin" \
            "$PROJECT_DIR/target/tpscript-it/debug/rs-it-tpscript"
    else
        echo "SKIPPED: Python scripting plugin unavailable; set ENDUROX_TPSCRIPT_PLUGIN"
    fi
}
run_mode
run_mode --features ctx-send
