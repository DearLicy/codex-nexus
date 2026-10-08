$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

cargo build --release --manifest-path native/Cargo.toml
$manifest = Get-Content native/Cargo.toml -Raw
$version = [regex]::Match($manifest, 'version\s*=\s*"([^"]+)"').Groups[1].Value
$staging = Join-Path $repoRoot ("native\target\package\Codex Nexus-$version")
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Copy-Item "native\target\release\codex-nexus-native.exe" (Join-Path $staging "Codex Nexus.exe") -Force
Copy-Item "README.md" (Join-Path $staging "README.md") -Force

$archive = Join-Path $repoRoot ("native\target\package\Codex-Nexus-$version-windows.zip")
if (Test-Path $archive) { Remove-Item $archive -Force }
Compress-Archive -Path $staging -DestinationPath $archive
Write-Output "Created $archive"
