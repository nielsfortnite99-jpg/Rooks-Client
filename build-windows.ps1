$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$releaseExe = Join-Path $root "release\Rook's Client.exe"
$runningRelease = Get-Process -Name "Rook's Client" -ErrorAction SilentlyContinue | Where-Object {
    $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq [IO.Path]::GetFullPath($releaseExe))
}
if ($runningRelease) { throw "Bitte schließe zuerst Rook's Client (PID $($runningRelease.Id)), bevor du den Release-Build erneut startest." }

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path -LiteralPath $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust/Cargo fehlt. Rust stable mit rustup installieren.'
}
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw 'Node.js/npm fehlt. Node.js 18 oder neuer installieren.'
}

$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio Installer wurde nicht gefunden.' }
$vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) { throw 'Im Visual Studio Installer den Workload "Desktopentwicklung mit C++" installieren.' }
$vsDevCmd = Join-Path $vsPath 'Common7\Tools\VsDevCmd.bat'
if (-not (Test-Path -LiteralPath $vsDevCmd)) { throw 'VsDevCmd.bat wurde nicht gefunden.' }

Write-Host "Installiere Rook's Client Build-Abhängigkeiten …"
Push-Location $root
try { npm install; if ($LASTEXITCODE -ne 0) { throw 'npm install ist fehlgeschlagen.' } }
finally { Pop-Location }

# The Windows resource compiler mishandles the apostrophe in the user's project path.
$stage = Join-Path $env:TEMP ('RooksClientBuild-' + [guid]::NewGuid().ToString('N'))
$stageTauri = Join-Path $stage 'src-tauri'
New-Item -ItemType Directory -Force -Path $stage, $stageTauri | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'package.json'), (Join-Path $root 'package-lock.json'), (Join-Path $root 'server.mjs') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'ui'), (Join-Path $root 'node_modules') -Destination $stage -Recurse
Copy-Item -LiteralPath (Join-Path $root 'src-tauri\Cargo.toml'), (Join-Path $root 'src-tauri\Cargo.lock'), (Join-Path $root 'src-tauri\build.rs'), (Join-Path $root 'src-tauri\tauri.conf.json') -Destination $stageTauri
Copy-Item -LiteralPath (Join-Path $root 'src-tauri\src'), (Join-Path $root 'src-tauri\capabilities'), (Join-Path $root 'src-tauri\icons') -Destination $stageTauri -Recurse

$targetDir = Join-Path $root 'src-tauri\target'
$env:CARGO_TARGET_DIR = $targetDir
Push-Location $stage
try {
    Write-Host 'Erzeuge Release-EXE und Windows-MSI-Installer …'
    $command = 'call "' + $vsDevCmd + '" -arch=x64 && npm run tauri:build -- --bundles msi'
    & $env:ComSpec /d /s /c $command
    if ($LASTEXITCODE -ne 0) { throw 'Tauri-Release-Build ist fehlgeschlagen.' }
}
finally { Pop-Location }

$exe = Join-Path $targetDir "release\Rook's Client.exe"
$bundleDir = Join-Path $targetDir 'release\bundle\msi'
$installer = Get-ChildItem -LiteralPath $bundleDir -Filter '*x64*.msi' -File | Select-Object -First 1
if (-not (Test-Path -LiteralPath $exe)) { throw "EXE fehlt: $exe" }
if (-not $installer) { throw "MSI-Installer fehlt: $bundleDir" }

$release = Join-Path $root 'release'
New-Item -ItemType Directory -Force -Path $release | Out-Null
$releaseExe = Join-Path $release "Rook's Client.exe"
$releaseInstaller = Join-Path $release 'RooksClient-Setup.msi'
Copy-Item -LiteralPath $exe -Destination $releaseExe -Force
Copy-Item -LiteralPath $installer.FullName -Destination $releaseInstaller -Force

$tempRoot = [IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\'
$stageFull = [IO.Path]::GetFullPath($stage)
if ($stageFull.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $stageFull -Leaf).StartsWith('RooksClientBuild-')) {
    Remove-Item -LiteralPath $stageFull -Recurse -Force
}

Get-Item -LiteralPath $releaseExe, $releaseInstaller | Select-Object FullName, Length, LastWriteTime
