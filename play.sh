#!/bin/bash
# Builds and starts Skate 3 Rust Engine with your own Xbox 360 copy of Skate 3.
#
#   ./play.sh                          first run: choose the game, then play
#   ./play.sh "/path/to/Skate 3.iso"   set up from a game folder or .iso, then play
#   ./play.sh --map path/to/map.skate  options are passed to the game
#
# The first run offers to install rustup if it is missing, fetches the other
# tools, converts the game into data/ and builds the engine. Later runs rebuild
# what changed, refresh data/ when its conversion changed, and start the game.
# Every Mac gets the same tools: rust-toolchain.toml pins Rust, and
# tools/pinned_tool.sh pins uv, Python, its packages and vgmstream.
set -euo pipefail

repo=$(cd "$(dirname "$0")" && pwd)
cd "$repo"
audio=.local/skating-audio
exe=${CARGO_TARGET_DIR:-target}/debug/skate3rust

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
fail() { printf '\033[31m%s\033[0m\n' "$*" >&2; exit 1; }
# Yes unless the player declines; with no terminal there is nobody to ask.
confirm() {
    [ -t 0 ] || return 1
    local reply
    read -r -p "$1 [Y/n] " reply
    [[ ! $reply =~ ^[Nn] ]]
}

choose_game() {
    local choice
    # A Finder dialog in a desktop session; otherwise ask in the terminal.
    if choice=$(osascript 2>&1 <<'OSA'
set kind to button returned of (display dialog "Choose your own copy of Skate 3 for Xbox 360: its extracted game folder (with default.xex) or its .iso disc image." with title "Skate 3" buttons {"Cancel", "Disc image…", "Game folder…"} default button "Game folder…" cancel button "Cancel")
if kind is "Game folder…" then return POSIX path of (choose folder with prompt "Select the Skate 3 folder that contains default.xex")
return POSIX path of (choose file with prompt "Select the Skate 3 .iso disc image" of type {"iso", "public.iso-image"})
OSA
    ); then
        printf '%s' "${choice%/}"
        return
    fi
    [[ $choice != *-128* ]] || fail "No game selected." # Cancel
    [ -t 0 ] || fail "Pass your game: ./play.sh \"/path/to/Skate 3\" (a folder or .iso)."
    read -r -p "Drag your Skate 3 folder or .iso here, then press Return: " choice
    # Terminal escapes dropped paths; undo that without evaluating anything.
    [ -e "$choice" ] || choice=$(printf '%s' "$choice" | xargs printf '%s' 2>/dev/null || true)
    printf '%s' "${choice%/}"
}

game=
if [ $# -gt 0 ] && [[ $1 != -* ]]; then
    game=${1%/}
    shift
fi

[ "$(uname -s)-$(uname -m)" = Darwin-arm64 ] || fail "This fork needs a Mac with Apple Silicon."

if ! xcode-select -p >/dev/null 2>&1; then
    xcode-select --install >/dev/null 2>&1 || true
    fail "Install the Xcode Command Line Tools in the window that opened, then run ./play.sh again."
fi

# rustup installs the Rust version of rust-toolchain.toml when cargo first runs.
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v rustup >/dev/null; then
    confirm "Building the game needs rustup, Rust's official installer. Install it now?" ||
        fail "Install Rust with rustup (https://rustup.rs), then run ./play.sh again."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain none
fi

setup_python=$(tools/pinned_tool.sh python)
python() { PYTHONDONTWRITEBYTECODE=1 "$setup_python" "$@"; }

state=$(python -m tools.asset_pipeline.versions --status data)
if [ -z "$game" ] && [ "$state" != current ]; then
    if [ "$state" = stale ]; then
        game=$(plutil -extract source raw -o - data/installation.json 2>/dev/null || true)
        [ -e "$game" ] || { game=; echo "This version converts the game differently; select your Skate 3 again."; }
    else
        echo "Welcome! Choose your own copy of Skate 3 for Xbox 360 to convert it for the engine."
    fi
    [ -n "$game" ] || game=$(choose_game)
fi
[ -z "$game" ] || [ -e "$game" ] || fail "Not found: $game"

step "Building the game"
[ -x "$exe" ] || echo "The first build compiles the engine and its libraries; it takes several minutes."
cargo build --locked -p skate-game --bin skate3rust

if [ -n "$game" ]; then
    step "Converting your copy of Skate 3"
    echo "This takes a few minutes. Later runs only redo what changes."
    # Setup decodes the original skating sounds once, with the pinned vgmstream.
    [ -d "$audio" ] || tools/pinned_tool.sh vgmstream >/dev/null
    mkdir -p target/native
    rustc --edition 2024 --crate-type cdylib -C opt-level=3 -C panic=abort \
        tools/asset_pipeline/refpack_native.rs -o target/native/librefpack.dylib
    # Validation runs of the game have no window, and setup keeps their output,
    # so they skip the crash supervisor and its report dialog.
    SKATE_REPORT_CHILD=1 python tools/prepare_assets.py --game-root "$game" --output data \
        --game-exe "$exe" --skating-audio "$audio"
fi

step "Starting Skate 3"
if [ -z "${SKATE3_AUDIO+x}" ] && [ -d "$audio" ]; then export SKATE3_AUDIO="$repo/$audio"; fi
export SKATE3_MODS="$repo/mods"
exec "$exe" --assets "$repo/data/$(plutil -extract directory raw -o - data/installation.json)/assets" "$@"
