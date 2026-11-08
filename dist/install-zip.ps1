# install.ps1 (ZIP version) - publishes mrp-tui and mrp-gui as user commands.
# Same semantics as the repo's original (rust\install.ps1), with the ZIP's
# paths: the exes sit next to this script, languages in lang\.
# The installed copy is a snapshot: re-run after every new ZIP.
#
# Optional:  .\install.ps1 -Associate   (alias: -Associa)
#   also registers mrp-gui in the "Open with" list of .txt and .log and in
#   Settings > Default apps, for this user only (HKCU: no admin, nothing
#   machine-wide). The default itself stays yours: Windows does not let a
#   program take an extension silently, so the last step is one click -
#   right click a file, Open with, Choose another app, mrp-gui, "Always".
param([switch]$Associate, [switch]$Associa)
$ErrorActionPreference = 'Stop'
$binDir = Join-Path $env:USERPROFILE '.local\bin'
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
foreach ($name in 'mrp-tui', 'mrp-gui') {
    $src = Join-Path $PSScriptRoot "$name.exe"
    if (-not (Test-Path $src)) { throw "missing $src - the archive is incomplete" }
    $dst = Join-Path $binDir "$name.exe"
    Remove-Item "$dst.old" -Force -ErrorAction SilentlyContinue
    try {
        Copy-Item $src $dst -Force
    } catch {
        # exe in use: Windows won't overwrite but allows a rename
        Move-Item $dst "$dst.old" -Force
        Copy-Item $src $dst -Force
        Remove-Item "$dst.old" -Force -ErrorAction SilentlyContinue
    }
    "installed: $dst"
}

# PATH check: if ~/.local/bin is not in user PATH, add it (one time)
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$binDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$binDir", "User")
    "PATH updated: $binDir added (restart your terminal)"
}
# language registry: the JSON files from lang\ go to the user profile,
# where mrp-tui and mrp-gui discover them at runtime (English is already embedded).
$langSrc = Join-Path $PSScriptRoot 'lang'
if (-not (Test-Path $langSrc)) { throw "language folder missing: $langSrc" }
$langDir = Join-Path $env:APPDATA 'mrp\lang'
New-Item -ItemType Directory -Force -Path $langDir | Out-Null
Copy-Item (Join-Path $langSrc '*.json') $langDir -Force
"languages installed: $((Get-ChildItem $langDir -Filter *.json).Count) JSON in $langDir"
"self-test:"
& (Join-Path $binDir 'mrp-tui.exe') --help

# File associations (opt-in): mrp-gui in the "Open with" list of .txt and .log
# and in Settings > Default apps. All in HKCU - no admin. The default itself
# is the user's one click (see the header): Windows signs that choice itself.
if ($Associate -or $Associa) {
    $exe = Join-Path $binDir 'mrp-gui.exe'
    $progId = 'Manto.RustPad'
    New-Item -Path "HKCU:\Software\Classes\$progId" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId" -Name '(Default)' -Value 'Manto.RustPad'
    New-Item -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Name '(Default)' -Value "`"$exe`",0"
    New-Item -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Name '(Default)' -Value "`"$exe`" `"%1`""
    foreach ($ext in '.txt', '.log') {
        $key = "HKCU:\Software\Classes\$ext\OpenWithProgids"
        New-Item -Path $key -Force | Out-Null
        New-ItemProperty -Path $key -Name $progId -Value ([byte[]]@()) -PropertyType None -Force | Out-Null
    }
    # Settings > Default apps: the program appears there with its name and icon
    New-Item -Path 'HKCU:\Software\Manto\RustPad\Capabilities\FileAssociations' -Force | Out-Null
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities' -Name 'ApplicationName' -Value 'Manto.RustPad'
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities' -Name 'ApplicationDescription' -Value 'The editor for files that do not fit in RAM'
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities\FileAssociations' -Name '.txt' -Value $progId
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities\FileAssociations' -Name '.log' -Value $progId
    New-ItemProperty -Path 'HKCU:\Software\RegisteredApplications' -Name 'Manto.RustPad' -Value 'Software\Manto\RustPad\Capabilities' -PropertyType String -Force | Out-Null
    "file associations: mrp-gui is in the 'Open with' list of .txt and .log"
    "and in Settings > Default apps. The default is your one click:"
    "right click a .txt > Open with > Choose another app > mrp-gui > Always"
}
