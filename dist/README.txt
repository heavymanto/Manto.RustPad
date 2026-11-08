Manto.RustPad - the editor for files that don't fit in RAM
==========================================================

Nothing to install: extract and run.

  mrp-tui.exe     the editor in the terminal: mrp-tui file...  (F1 = help, F4 = edit)
  mrp-gui.exe     the same editor in a window: mrp-gui file...  (no file: open dialog)

English is embedded; the other six languages are in lang\ next to the
programs. While lang\ sits next to the programs, the settings and the saved
session stay in this same folder and nothing is written to your profile.

Want more? install.ps1 (same folder) publishes both as user commands
(%USERPROFILE%\.local\bin, added to your user PATH if it is not there) and
drops the six languages in %APPDATA%\mrp\lang.

  .\install.ps1 -Associate   also puts mrp-gui in the "Open with" list of
                             .txt and .log, and in Settings > Default apps
                             (your user only, no admin). Making it the default
                             stays one click of yours: right click a file,
                             Open with, Choose another app, mrp-gui, "Always".

Verify the download: the .sha256 file that rides next to this ZIP.

License: AGPL-3.0 - see LICENSE. The full story (measurements, architecture)
lives in the docs/ folder of the repository.
