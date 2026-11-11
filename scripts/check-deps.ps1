# Controle qu'une version « autonome » tourne sur un Windows vierge : chaque DLL importee par un .exe/.dll du dossier doit
# etre dans le dossier, ou faire partie de Windows. Les runtimes Visual C++ ne sont PAS consideres comme presents sur le
# systeme (une machine neuve n'en a pas forcement) : ils doivent etre fournis dans le dossier.
#   .\scripts\check-deps.ps1 -Dir target\dist\VoiceClone-0.1.1-windows-x64
param([Parameter(Mandatory)][string]$Dir)
$ErrorActionPreference = 'Stop'
$dumpbin = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" | Sort-Object FullName | Select-Object -Last 1
if (-not $dumpbin) { throw 'dumpbin.exe introuvable (Visual Studio Build Tools).' }
$dir = (Resolve-Path $Dir).Path
$here = (Get-ChildItem $dir -File | ForEach-Object { $_.Name.ToLower() })
$system = "$env:SystemRoot\System32"
$notWindows = '^(vcruntime|msvcp|vcomp|concrt|vccorlib|mfc)'   # runtimes Visual C++ : a fournir
$optional = @{ 'vulkan-1.dll' = 'fourni par le pilote graphique (sans lui le calcul retombe sur le processeur)' }

$missing = @{}; $notes = @{}
foreach ($f in Get-ChildItem $dir -File | Where-Object { $_.Extension -in '.exe', '.dll' }) {
    $out = & $dumpbin.FullName /dependents $f.FullName | Out-String
    foreach ($m in [regex]::Matches($out, '(?im)^\s+(\S+\.dll)\s*$')) {
        $dep = $m.Groups[1].Value.ToLower()
        if ($here -contains $dep -or $dep -like 'api-ms-win-*' -or $dep -like 'ext-ms-win-*') { continue }
        if ($optional.ContainsKey($dep)) { $notes[$dep] = $optional[$dep]; continue }
        if ($dep -notmatch $notWindows -and (Test-Path "$system\$dep")) { continue }
        if (-not $missing.ContainsKey($dep)) { $missing[$dep] = @() }
        $missing[$dep] += $f.Name
    }
}
foreach ($k in $notes.Keys) { "optionnel : $k ($($notes[$k]))" }
if ($missing.Count) {
    foreach ($k in $missing.Keys | Sort-Object) { "MANQUANT : $k  <- $(($missing[$k] | Sort-Object -Unique) -join ', ')" }
    exit 1
}
"dépendances OK : $($here.Count) fichiers, rien ne manque hors Windows."
