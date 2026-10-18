# Downloads the official, signed PawnIO installer that the Systana installer bundles.
# Run once before `npm run tauri build`. The file is not committed to git.
$ErrorActionPreference = "Stop"

$version = "2.2.0"
$url = "https://github.com/namazso/PawnIO.Setup/releases/download/$version/PawnIO_setup.exe"
$sha256 = "1F519A22E47187F70A1379A48CA604981C4FCF694F4E65B734AAA74A9FBA3032"
$dest = Join-Path $PSScriptRoot "..\src-tauri\vendor\PawnIO_setup.exe"

New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
if (-not (Test-Path $dest) -or (Get-FileHash $dest -Algorithm SHA256).Hash -ne $sha256) {
    Write-Host "Downloading PawnIO $version..."
    Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
}

$hash = (Get-FileHash $dest -Algorithm SHA256).Hash
if ($hash -ne $sha256) { throw "PawnIO_setup.exe hash mismatch: $hash" }
$sig = Get-AuthenticodeSignature $dest
if ($sig.Status -ne "Valid" -or $sig.SignerCertificate.Subject -notmatch "CN=namazso\.eu") {
    throw "PawnIO_setup.exe signature is not valid: $($sig.Status) $($sig.SignerCertificate.Subject)"
}
Write-Host "PawnIO_setup.exe OK ($hash)"
