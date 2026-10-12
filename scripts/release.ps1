$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
npm run tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Output "Build output: src-tauri\target\release\bundle\nsis\"
