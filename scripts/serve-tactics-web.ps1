[CmdletBinding()]
param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 8080,
    [string]$BindAddress = "0.0.0.0",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$webRoot = Join-Path $repoRoot "web-dist\tactics-lab"
if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot "build-tactics-web.ps1") -OutputDirectory $webRoot
    if ($LASTEXITCODE -ne 0) {
        throw "The tactics web build failed."
    }
}
if (-not (Test-Path -LiteralPath (Join-Path $webRoot "index.html") -PathType Leaf)) {
    throw "No web bundle exists at $webRoot. Build it first or omit -SkipBuild."
}
if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    throw "Python is required for the local static server."
}

$addresses = Get-NetIPAddress -AddressFamily IPv4 |
    Where-Object {
        $_.AddressState -eq "Preferred" -and
        $_.IPAddress -notlike "127.*" -and
        $_.InterfaceAlias -notlike "vEthernet*"
    } |
    Select-Object -ExpandProperty IPAddress

Write-Host "Serving the tactical lab. Keep this window open; Ctrl+C stops it."
Write-Host "Desktop: http://localhost:$Port/"
foreach ($address in $addresses) {
    Write-Host "Mobile:  http://${address}:$Port/"
}
Write-Host "Use only on a trusted private network; do not router-forward this port."
python (Join-Path $PSScriptRoot "serve-tactics-web.py") --port $Port --bind $BindAddress --directory $webRoot
