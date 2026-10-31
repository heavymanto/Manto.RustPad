# Oracolo indipendente: le tabelle delle code page come le decodifica .NET (cioe' Windows), non Python.
# Scrive una riga per code page: "<nome canonico> <128 unita' UTF-16 in esadecimale a 4 cifre>"; fffd = byte non definito.
param([string]$Out)
[Text.Encoding]::RegisterProvider([Text.CodePagesEncodingProvider]::Instance)
$ids = [ordered]@{
  'windows-1250'=1250; 'windows-1251'=1251; 'windows-1252'=1252; 'windows-1253'=1253; 'windows-1254'=1254
  'windows-1255'=1255; 'windows-1256'=1256; 'windows-1257'=1257; 'windows-1258'=1258
  'iso-8859-1'=28591; 'iso-8859-2'=28592; 'iso-8859-3'=28593; 'iso-8859-4'=28594; 'iso-8859-5'=28595
  'iso-8859-6'=28596; 'iso-8859-7'=28597; 'iso-8859-8'=28598; 'iso-8859-9'=28599; 'iso-8859-13'=28603; 'iso-8859-15'=28605
  'ibm437'=437; 'ibm737'=737; 'ibm850'=850; 'ibm852'=852; 'ibm858'=858; 'ibm866'=866
  'koi8-r'=20866; 'koi8-u'=21866; 'macintosh'=10000
  'us-ascii'=20127; 'windows-874'=874; 'ibm775'=775; 'ibm855'=855; 'ibm857'=857; 'ibm860'=860; 'ibm861'=861
  'ibm862'=862; 'ibm863'=863; 'ibm865'=865; 'ibm869'=869; 'ibm720'=720
  'mac-cyrillic'=10007; 'mac-centraleurope'=10029; 'mac-iceland'=10079
  'mac-turkish'=10081; 'mac-croatian'=10082; 'mac-romanian'=10010
}
$righe = foreach ($k in $ids.Keys) {
  $e = [Text.Encoding]::GetEncoding($ids[$k], [Text.EncoderFallback]::ExceptionFallback, [Text.DecoderFallback]::ExceptionFallback)
  $hex = for ($b = 0x80; $b -le 0xFF; $b++) {
    try {
      $s = $e.GetString([byte[]]@($b))
      if ($s.Length -ne 1) { throw 'non singolo' }
      '{0:x4}' -f [int][char]$s[0]
    } catch { 'fffd' }
  }
  "$k " + ($hex -join '')
}
[IO.File]::WriteAllText($Out, (($righe -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false)))
"scritte $($righe.Count) righe in $Out"
