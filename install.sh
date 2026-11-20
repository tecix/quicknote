#!/bin/sh
set -e
cd "$(dirname "$0")"
cargo build --release
install -Dm755 target/release/quicknote "$HOME/.local/bin/quicknote"
install -Dm644 data/dev.tecix.QuickNote.desktop "$HOME/.local/share/applications/dev.tecix.QuickNote.desktop"
echo "Installed. Run: quicknote --settings"
