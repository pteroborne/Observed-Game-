param(
    [string]$Destination = (Join-Path $env:APPDATA "TrenchBroom\games\Observed 2")
)

$source = Join-Path $PSScriptRoot "Observed2"
if (-not (Test-Path -LiteralPath $source)) {
    throw "Observed 2 TrenchBroom configuration is missing: $source"
}

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item -LiteralPath (Join-Path $source "GameConfig.cfg") -Destination $Destination -Force
Copy-Item -LiteralPath (Join-Path $source "Observed2.fgd") -Destination $Destination -Force
Write-Host "Installed Observed 2 TrenchBroom configuration to $Destination"
Write-Host "In TrenchBroom, set the Observed 2 game path to this repository's assets\tiles directory."
