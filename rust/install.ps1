# install.ps1 - pubblica mrp-tui (l'editor da terminale) e mrp-gui (la finestra) come comandi utente
# Copia rust\target\release\mrp-tui.exe e mrp-gui.exe (workspace Cargo) in %USERPROFILE%\.local\bin
# e crates\mrp-core\lang\*.json (le sette lingue) in %APPDATA%\mrp\lang, il registro
# che mrp-tui scopre a runtime (l'inglese e' incorporato nell'exe: la cartella serve per
# le altre lingue e per l'override di en.json, che nel profilo sopravanza quello di
# fabbrica).
# Non tocca mai il PATH: se %USERPROFILE%\.local\bin non e' nel PATH lo dice soltanto.
# La copia installata e' uno snapshot: rieseguire questo script dopo ogni rebuild o
# dopo ogni cambio alle lingue. Le impostazioni dell'utente (%APPDATA%\mrp\settings.txt)
# non si toccano.
# Uso:  cargo build --release   (in rust\)
#       .\install.ps1
#       mrp-tui <file>        -> apre il file (lettura; F4 per modificare, F1 aiuto)
#       mrp-tui --help        -> le opzioni
# Opzionale:  .\install.ps1 -Associa   mette anche mrp-gui nell'elenco "Apri
#   con" di .txt e .log e in Impostazioni > App predefinite (solo utente, HKCU:
#   niente admin). Il default resta il tuo click: Windows non lascia che sia un
#   programma a prendersi un'estensione in silenzio.
param([switch]$Associa)
$ErrorActionPreference = 'Stop'
$binDir = Join-Path $env:USERPROFILE '.local\bin'
New-Item -ItemType Directory -Force -Path $binDir | Out-Null

$langSrc = Join-Path $PSScriptRoot 'crates\mrp-core\lang'
if (-not (Test-Path $langSrc)) { throw "cartella lingue mancante: $langSrc" }

# mrp-tui (terminale) e mrp-gui (finestra): mrp-tui e' obbligatorio, mrp-gui si installa se e' stato compilato
foreach ($nome in 'mrp-tui', 'mrp-gui') {
    $src = Join-Path $PSScriptRoot "target\release\$nome.exe"
    if (-not (Test-Path $src)) {
        if ($nome -eq 'mrp-tui') { throw "build mancante: $src (prima: cargo build --release in rust\)" }
        "saltato: $src non c'e' (cargo build --release -p mrp-gui)"
        continue
    }
    $dst = Join-Path $binDir "$nome.exe"
    Remove-Item "$dst.old" -Force -ErrorAction SilentlyContinue
    try {
        Copy-Item $src $dst -Force
    } catch {
        # exe in esecuzione (es. un file aperto in mrp-tui): Windows non consente la
        # sovrascrittura ma CONSENTE il rename di un exe in uso; spostiamo il vecchio
        # e mettiamo il nuovo al suo posto.
        Move-Item $dst "$dst.old" -Force
        Copy-Item $src $dst -Force
        # se il vecchio processo e' ancora attivo la Delete fallisce: ripulita al giro dopo
        Remove-Item "$dst.old" -Force -ErrorAction SilentlyContinue
    }
    "installato: $dst"
}
$dst = Join-Path $binDir 'mrp-tui.exe'

# i vecchi nomi dei comandi (mrp.exe, mrpg.exe) non esistono piu': una copia rimasta sarebbe una versione vecchia
foreach ($vecchio in 'mrp.exe', 'mrpg.exe') {
    $v = Join-Path $binDir $vecchio
    if (Test-Path $v) {
        try { Remove-Item $v -Force } catch { Move-Item $v "$v.old" -Force }
        "rimosso il vecchio nome: $v"
    }
}

# registro lingue: i JSON sorgente (il test di parita' del core li ha gia' verificati)
# finiscono nel profilo utente. Su una macchina nuova la cartella non esiste: la creiamo.
$langDir = Join-Path $env:APPDATA 'mrp\lang'
New-Item -ItemType Directory -Force -Path $langDir | Out-Null
Copy-Item (Join-Path $langSrc '*.json') $langDir -Force
"lingue installate: $((Get-ChildItem $langDir -Filter *.json).Count) JSON in $langDir"

if (-not (($env:PATH -split ';') -contains $binDir)) {
    "ATTENZIONE: $binDir non e' nel PATH; aggiungilo (o lancia mrp-tui col percorso completo)."
}

# auto-test senza terminale: --dump stampa lo schermo come testo. Un file di prova
# temporaneo, in italiano per provare anche il registro lingue appena installato.
$prova = Join-Path ([IO.Path]::GetTempPath()) ("mrp-install-" + [guid]::NewGuid().ToString('N') + '.txt')
try {
    [IO.File]::WriteAllText($prova, "riga uno`nriga due`nriga tre`n")
    "auto-test mrp-tui --help:"
    & $dst --help
    if ($LASTEXITCODE -ne 0) { throw "mrp-tui --help ha restituito $LASTEXITCODE" }
    "auto-test mrp-tui --dump 60x6 (inglese):"
    & $dst --dump 60x6 $prova
    if ($LASTEXITCODE -ne 0) { throw "mrp-tui --dump ha restituito $LASTEXITCODE" }
    "auto-test registro lingue (mrp-tui --lang it --dump 60x6):"
    & $dst --lang it --dump 60x6 $prova
    if ($LASTEXITCODE -ne 0) { throw "mrp-tui --lang it --dump ha restituito $LASTEXITCODE" }
} finally {
    Remove-Item $prova -Force -ErrorAction SilentlyContinue
}

# Associazioni file (opt-in): mrp-gui nell'elenco "Apri con" di .txt e .log e in
# Impostazioni > App predefinite. Tutto in HKCU, niente admin. Il default e'
# l'ultimo click dell'utente (vedi l'intestazione): quella firma la mette Windows.
if ($Associa) {
    $exe = Join-Path $binDir 'mrp-gui.exe'
    $progId = 'Manto.RustPad'
    New-Item -Path "HKCU:\Software\Classes\$progId" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId" -Name '(Default)' -Value 'Manto.RustPad'
    New-Item -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Name '(Default)' -Value "`"$exe`",0"
    New-Item -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Name '(Default)' -Value "`"$exe`" `"%1`""
    foreach ($est in '.txt', '.log') {
        $chiave = "HKCU:\Software\Classes\$est\OpenWithProgids"
        New-Item -Path $chiave -Force | Out-Null
        New-ItemProperty -Path $chiave -Name $progId -Value ([byte[]]@()) -PropertyType None -Force | Out-Null
    }
    # Impostazioni > App predefinite: il programma compare li', col nome e l'icona
    New-Item -Path 'HKCU:\Software\Manto\RustPad\Capabilities\FileAssociations' -Force | Out-Null
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities' -Name 'ApplicationName' -Value 'Manto.RustPad'
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities' -Name 'ApplicationDescription' -Value 'The editor for files that do not fit in RAM'
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities\FileAssociations' -Name '.txt' -Value $progId
    Set-ItemProperty -Path 'HKCU:\Software\Manto\RustPad\Capabilities\FileAssociations' -Name '.log' -Value $progId
    New-ItemProperty -Path 'HKCU:\Software\RegisteredApplications' -Name 'Manto.RustPad' -Value 'Software\Manto\RustPad\Capabilities' -PropertyType String -Force | Out-Null
    "associazioni file: mrp-gui e' nell'elenco 'Apri con' di .txt e .log"
    "e in Impostazioni > App predefinite. Il default e' il tuo click:"
    "click destro su un .txt > Apri con > Scegli un'altra app > mrp-gui > sempre"
}
