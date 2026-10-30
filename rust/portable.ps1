# Prepara una cartella PORTATILE di mrp: mrp.exe + lang\*.json. Si copia dove si vuole (chiavetta, altro PC) e si
# usa senza installare: con la cartella `lang` accanto a mrp.exe, lingue e impostazioni (settings.txt) stanno li',
# niente nel profilo utente.
#
# Uso (dopo `cargo build --release -p mrp-tui`):
#   .\portable.ps1                      # crea .\mrp-portable
#   .\portable.ps1 -Out D:\strumenti\mrp
param([string]$Out = (Join-Path $PSScriptRoot 'mrp-portable'))
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'target\release\mrp.exe'
$lang = Join-Path $PSScriptRoot 'crates\mrp-core\lang'
if (-not (Test-Path $exe)) { throw "manca ${exe}: prima 'cargo build --release -p mrp-tui'" }
New-Item -ItemType Directory -Force -Path (Join-Path $Out 'lang') | Out-Null
Copy-Item $exe $Out -Force
Copy-Item (Join-Path $lang '*.json') (Join-Path $Out 'lang') -Force
Write-Host "Cartella portatile pronta: $Out"
Write-Host "  mrp.exe + lang\ ($((Get-ChildItem (Join-Path $Out 'lang') -Filter *.json).Count) lingue); le impostazioni si salveranno in $Out\settings.txt"
Write-Host "Prova: & '$Out\mrp.exe' --help"
