[CmdletBinding()]
param(
    [string]$OutputDirectory
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repoRoot "web-dist\tactics-lab"
}
$outputPath = [System.IO.Path]::GetFullPath($OutputDirectory)
$expectedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "web-dist"))
if (-not $outputPath.StartsWith($expectedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputDirectory must remain inside $expectedRoot"
}

$bindgen = Get-Command wasm-bindgen -ErrorAction SilentlyContinue
if (-not $bindgen) {
    throw "wasm-bindgen is required. Install the lockfile-matched CLI with: cargo install wasm-bindgen-cli --version 0.2.125 --locked"
}

Push-Location $repoRoot
try {
    cargo build -p tactics_lab --profile wasm-release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) {
        throw "The tactics WASM build failed."
    }
    $metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $wasm = Join-Path $metadata.target_directory "wasm32-unknown-unknown\wasm-release\tactics_lab.wasm"
    if (-not (Test-Path -LiteralPath $wasm -PathType Leaf)) {
        throw "Cargo completed but $wasm was not produced."
    }
    New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
    & $bindgen.Source --target web --no-typescript --out-dir $outputPath --out-name tactics_lab $wasm
    if ($LASTEXITCODE -ne 0) {
        throw "wasm-bindgen failed."
    }
    Copy-Item -LiteralPath (Join-Path $repoRoot "labs\tactics_lab\web\index.html") -Destination $outputPath -Force

    $bundle = Get-Item -LiteralPath (Join-Path $outputPath "tactics_lab_bg.wasm")
    Write-Host ("Tactics web bundle: {0:N1} MiB" -f ($bundle.Length / 1MB))
    Write-Host "Output: $outputPath"
}
finally {
    Pop-Location
}
