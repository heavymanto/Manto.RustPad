# Prepara una cartella PORTATILE di mrp-tui: mrp-tui.exe + lang\*.json. Si copia dove si vuole (chiavetta, altro PC) e si
# usa senza installare: con la cartella `lang` accanto a mrp-tui.exe, lingue e impostazioni (settings.txt) stanno li',
# niente nel profilo utente.
#
# Uso (dopo `cargo build --release -p mrp-tui`):
#   .\portable.ps1                      # crea .\mrp-portable
#   .\portable.ps1 -Out D:\strumenti\mrp
param([string]$Out = (Join-Path $PSScriptRoot 'mrp-portable'))
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'target\release\mrp-tui.exe'
$lang = Join-Path $PSScriptRoot 'crates\mrp-core\lang'
if (-not (Test-Path $exe)) { throw "manca ${exe}: prima 'cargo build --release -p mrp-tui'" }
New-Item -ItemType Directory -Force -Path (Join-Path $Out 'lang') | Out-Null
Copy-Item $exe $Out -Force
# la finestra, se e' stata compilata
$gui = Join-Path $PSScriptRoot 'target\release\mrp-gui.exe'
if (Test-Path $gui) { Copy-Item $gui $Out -Force }
Copy-Item (Join-Path $lang '*.json') (Join-Path $Out 'lang') -Force
Write-Host "Cartella portatile pronta: $Out"
Write-Host "  mrp-tui.exe + mrp-gui.exe + lang\ ($((Get-ChildItem (Join-Path $Out 'lang') -Filter *.json).Count) lingue); le impostazioni si salveranno in $Out\settings.txt"
Write-Host "Prova: & '$Out\mrp-tui.exe' --help"
