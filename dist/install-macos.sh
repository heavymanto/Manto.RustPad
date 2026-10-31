#!/bin/bash
# install.sh (archive version) - publishes mrp-tui and mrp-gui as user commands.
# Same semantics as the repo's original (rust/install.ps1), for macOS:
# binaries go to ~/.local/bin, languages to ~/.config/mrp/lang (where mrp-tui looks
# for them on every system that is not Windows).
# The installed copy is a snapshot: re-run after every new tar.gz.
set -e

HERE="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$HOME/.local/bin"
LANG_DIR="$HOME/.config/mrp/lang"

mkdir -p "$BIN_DIR"

for name in mrp-tui mrp-gui; do
    SRC="$HERE/$name"
    if [ ! -f "$SRC" ]; then
        echo "missing $SRC - the archive is incomplete" >&2
        exit 1
    fi
    DST="$BIN_DIR/$name"
    cp -f "$SRC" "$DST"
    chmod +x "$DST"
    # a binary downloaded with a browser carries the quarantine flag: the first run
    # would be blocked by Gatekeeper (the programs are not signed)
    xattr -d com.apple.quarantine "$DST" 2>/dev/null || true
    echo "installed: $DST"
done

# language registry: the JSON files from lang/ go to the user profile,
# where mrp-tui and mrp-gui discover them at runtime (English is already embedded).
LANG_SRC="$HERE/lang"
if [ ! -d "$LANG_SRC" ]; then
    echo "language folder missing: $LANG_SRC" >&2
    exit 1
fi
mkdir -p "$LANG_DIR"
cp -f "$LANG_SRC"/*.json "$LANG_DIR/"
echo "languages installed: $(ls "$LANG_DIR"/*.json | wc -l | tr -d ' ') JSON in $LANG_DIR"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "NOTE: $BIN_DIR is not in your PATH; add it (or run mrp-tui with the full path)." ;;
esac

echo "self-test:"
"$BIN_DIR/mrp-tui" --help
