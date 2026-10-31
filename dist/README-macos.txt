Manto.RustPad - the editor for files that don't fit in RAM
==========================================================

Nothing to install: extract and run.

  mrp     the editor in the terminal: ./mrp file...  (F1 = help, F4 = edit)
  mrpg    the same editor in a window: ./mrpg file...

English is embedded; the other six languages are in lang/ next to the
programs. While lang/ sits next to the programs, the settings and the saved
session stay in this same folder and nothing is written to your profile.

The programs are not signed: if macOS blocks the first run, clear the
quarantine flag once with  xattr -d com.apple.quarantine mrp mrpg
(install.sh does it for you).

Want more? install.sh (same folder) publishes both as user commands
(~/.local/bin - make sure it is in your PATH) and drops the six languages
in ~/.config/mrp/lang.

Verify the download: the .sha256 file that rides next to this archive.

Note: macOS is untried so far; the CI builds and tests it at every push.

License: AGPL-3.0 - see LICENSE. The full story (measurements, architecture)
lives in the docs/ folder of the repository.
