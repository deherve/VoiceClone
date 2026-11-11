# Passe a la version suivante dans tous les fichiers qui la portent (Cargo.toml de l'application et du coeur, tauri.conf.json).
#   .\scripts\bump-version.ps1            version suivante : on avance de 0.0.1, apres 9 on passe a la dizaine superieure
#                                         (0.1.8 -> 0.1.9 -> 0.2.0 -> ... -> 0.9.9 -> 1.0.0)
#   .\scripts\bump-version.ps1 -To 0.1.1  version imposee
param([ValidatePattern('^\d+\.\d+\.\d+$')][string]$To = '')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot

function Next-Version([string]$v) {
    $a = [int[]]($v -split '\.')
    $a[2]++
    if ($a[2] -gt 9) { $a[2] = 0; $a[1]++ }
    if ($a[1] -gt 9) { $a[1] = 0; $a[0]++ }
    "$($a[0]).$($a[1]).$($a[2])"
}
if ($MyInvocation.InvocationName -eq '.') { return }   # `. .\bump-version.ps1` : charge seulement Next-Version (tests)

$appToml = "$root\crates\app\Cargo.toml"
$current = (Select-String -Path $appToml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$new = if ($To) { $To } else { Next-Version $current }

# Edition par remplacement de la ligne `version = "..."` du paquet (premiere occurrence), sans toucher aux dependances.
foreach ($f in "$root\crates\app\Cargo.toml", "$root\crates\voiceclone\Cargo.toml") {
    $text = Get-Content $f -Raw -Encoding UTF8   # sans -Encoding, PowerShell 5.1 lit en ANSI et abime les accents
    $text = [regex]::Replace($text, '(?m)^version = "[^"]+"', "version = `"$new`"", 1)
    [IO.File]::WriteAllText($f, $text, (New-Object Text.UTF8Encoding $false))
}
$conf = "$root\crates\app\tauri.conf.json"
$text = [regex]::Replace((Get-Content $conf -Raw -Encoding UTF8), '(?m)^(\s*"version":\s*)"[^"]+"', "`${1}`"$new`"", 1)
[IO.File]::WriteAllText($conf, $text, (New-Object Text.UTF8Encoding $false))

cargo check --release 2>&1 | Out-Null   # remet Cargo.lock d'equerre
"$current -> $new"
