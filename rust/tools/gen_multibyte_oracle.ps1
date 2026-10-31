# Prima meta' dell'oracolo delle codifiche multibyte: come .NET (cioe' Windows) decodifica ogni sequenza di 1 e 2 byte
# (e un campione delle sequenze di 4 byte di GB18030). Una riga per sequenza valida: "<byte in esadecimale> <punti di codice>".
# La seconda meta' (gen_multibyte_oracle.py) incrocia questo con i codec di Python e scrive il file dei test.
# Uso (da rust/):  pwsh -NoProfile -File tools/gen_multibyte_oracle.ps1 -OutDir <cartella>
param([string]$OutDir)
[Text.Encoding]::RegisterProvider([Text.CodePagesEncodingProvider]::Instance)
$ids = [ordered]@{ 'shift-jis' = 932; 'euc-jp' = 51932; 'gbk' = 936; 'gb18030' = 54936; 'big5' = 950; 'euc-kr' = 949 }

function Decode($e, [byte[]]$b) {
    try {
        $s = $e.GetString($b)
        if ($s.Length -eq 0) { return $null }
        $cp = @()
        for ($i = 0; $i -lt $s.Length; $i++) {
            if ([char]::IsHighSurrogate($s[$i]) -and ($i + 1) -lt $s.Length) { $cp += [char]::ConvertToUtf32($s[$i], $s[$i + 1]); $i++ }
            else { $cp += [int]$s[$i] }
        }
        if ($cp -contains 0xFFFD) { return $null }
        return ($cp | ForEach-Object { '{0:X}' -f $_ }) -join '+'
    } catch { return $null }
}

foreach ($nome in $ids.Keys) {
    $e = [Text.Encoding]::GetEncoding($ids[$nome], [Text.EncoderFallback]::ExceptionFallback, [Text.DecoderFallback]::ExceptionFallback)
    $righe = New-Object System.Collections.Generic.List[string]
    for ($a = 0x80; $a -le 0xFF; $a++) {
        $r = Decode $e ([byte[]]@($a))
        if ($r) { $righe.Add(('{0:X2} {1}' -f $a, $r)) }
        if ($a -ge 0x81 -and $a -le 0xFE) {
            for ($b = 0x30; $b -le 0xFE; $b++) {
                $r = Decode $e ([byte[]]@($a, $b))
                if ($r) { $righe.Add(('{0:X2}{1:X2} {2}' -f $a, $b, $r)) }
            }
        }
    }
    if ($nome -eq 'gb18030') {
        # le sequenze di 4 byte: un campione (1 su 97) del piano base, e tutte quelle dei piani sopra U+FFFF
        $n = 0
        for ($a = 0x81; $a -le 0x84; $a++) { for ($b = 0x30; $b -le 0x39; $b++) { for ($c = 0x81; $c -le 0xFE; $c++) { for ($d = 0x30; $d -le 0x39; $d++) {
            $n++
            if ($n % 97 -ne 0) { continue }
            $r = Decode $e ([byte[]]@($a, $b, $c, $d))
            if ($r) { $righe.Add(('{0:X2}{1:X2}{2:X2}{3:X2} {4}' -f $a, $b, $c, $d, $r)) }
        } } } }
        for ($b = 0x30; $b -le 0x39; $b++) { for ($c = 0x81; $c -le 0xFE; $c += 7) { for ($d = 0x30; $d -le 0x39; $d++) {
            $r = Decode $e ([byte[]]@(0x90, $b, $c, $d))
            if ($r) { $righe.Add(('{0:X2}{1:X2}{2:X2}{3:X2} {4}' -f 0x90, $b, $c, $d, $r)) }
        } } }
    }
    $dest = Join-Path $OutDir "net_$nome.txt"
    [IO.File]::WriteAllText($dest, (($righe -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false)))
    "$nome : $($righe.Count) righe"
}
