Manto.RustPad - the editor for files that don't fit in RAM
==========================================================

Nothing to install: extract and run.

  mrp-tui     the editor in the terminal: ./mrp-tui file...  (F1 = help, F4 = edit)
  mrp-gui     the same editor in a window: ./mrp-gui file...  (needs a desktop)

English is embedded; the other six languages are in lang/ next to the
programs. While lang/ sits next to the programs, the settings and the saved
session stay in this same folder and nothing is written to your profile.

Want more? install.sh (same folder) publishes both as user commands
(~/.local/bin - make sure it is in your PATH) and drops the six languages
in ~/.config/mrp/lang.

  ./install.sh --associa   also makes mrp-gui the default for text files
                           (.txt and .log are both text/plain): it writes
                           ~/.local/share/applications/mrp-gui.desktop and
                           calls xdg-mime. On Linux the installer can set
                           the default itself.

Verify the download: the .sha256 file that rides next to this archive.

Note: the terminal editor and the engine have been tested on Linux; the
window (mrp-gui) on Linux has not been tried yet.

License: AGPL-3.0 - see LICENSE. The full story (measurements, architecture)
lives in the docs/ folder of the repository.
